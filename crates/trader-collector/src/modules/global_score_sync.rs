//! Global Score 동기화 모듈.
//!
//! 모든 활성 심볼에 대해 GlobalScore를 계산하여 symbol_global_score 테이블에 저장합니다.

use rust_decimal::Decimal;
use sqlx::{PgPool, QueryBuilder, Postgres};
use std::time::Instant;
use tracing::{debug, info, warn};
use uuid::Uuid;

use trader_analytics::{GlobalScorer, GlobalScorerParams, IndicatorEngine, StructuralFeaturesCalculator};
use trader_analytics::indicators::AtrParams;
use trader_core::{Symbol, Timeframe};
use trader_data::cache::historical::CachedHistoricalDataProvider;

use super::checkpoint::{self, CheckpointStatus};
use super::utils::market_to_market_type;
use super::watchlist_helper;
use crate::config::CollectorConfig;
use crate::error::CollectorError;
use crate::stats::CollectionStats;
use crate::Result;

/// GlobalScore 동기화 옵션
#[derive(Debug, Default)]
pub struct GlobalScoreSyncOptions {
    /// 중단점부터 재개
    pub resume: bool,
    /// N시간 이내 업데이트된 심볼 스킵
    pub stale_hours: Option<u32>,
    /// 배치 크기 오버라이드 (None이면 config 기본값 사용, 0이면 제한 없음)
    pub batch_size: Option<i64>,
}

/// Global Score 동기화 실행.
///
/// # 동작
/// 1. 활성 심볼 목록 조회
/// 2. 각 심볼에 대해 OHLCV 데이터 조회 (60일)
/// 3. GlobalScorer로 점수 계산
/// 4. symbol_global_score 테이블에 UPSERT
///
/// # 인자
/// * `pool` - 데이터베이스 연결 풀
/// * `config` - Collector 설정
/// * `symbols` - 특정 심볼만 처리 (None이면 전체)
pub async fn sync_global_scores(
    pool: &PgPool,
    config: &CollectorConfig,
    symbols: Option<String>,
) -> Result<CollectionStats> {
    let options = GlobalScoreSyncOptions::default();
    sync_global_scores_with_options(pool, config, symbols, options).await
}

/// Global Score 동기화 실행 (옵션 포함).
pub async fn sync_global_scores_with_options(
    pool: &PgPool,
    config: &CollectorConfig,
    symbols: Option<String>,
    options: GlobalScoreSyncOptions,
) -> Result<CollectionStats> {
    let start = Instant::now();
    let mut stats = CollectionStats::new();

    // GlobalScorer 및 IndicatorEngine 초기화 (루프 밖에서 1회만 생성)
    let scorer = GlobalScorer::new();
    let indicator_engine = IndicatorEngine::new();
    let data_provider = CachedHistoricalDataProvider::new(pool.clone());

    // 체크포인트 로드 (resume 모드)
    let resume_ticker = if options.resume {
        match checkpoint::load_checkpoint(pool, "global_score_sync").await? {
            Some(t) => {
                info!(last_ticker = %t, "중단점부터 재개");
                Some(t)
            }
            None => {
                info!("이전 중단점 없음, 처음부터 시작");
                None
            }
        }
    } else {
        None
    };

    // 대상 심볼 결정 (관심종목 우선 처리)
    let target_symbols = if let Some(ref tickers) = symbols {
        let ticker_list: Vec<&str> = tickers.split(',').map(|s| s.trim()).collect();
        get_symbols_by_tickers(pool, &ticker_list).await?
    } else {
        // Phase 1: 관심종목 우선 처리 (체크포인트 무시)
        let (watchlist_symbols, wl_tickers) = if config.prioritize_watchlist {
            match watchlist_helper::fetch_all_priority_tickers(pool).await {
                Ok(wl) if !wl.is_empty() => {
                    let wl_syms = get_active_symbols_with_options(
                        pool,
                        wl.len() as i64,
                        None, // 체크포인트 무시
                        options.stale_hours,
                        Some(wl.as_slice()), // only: watchlist 심볼만
                        None,
                    )
                    .await?;
                    if !wl_syms.is_empty() {
                        tracing::info!(
                            count = wl_syms.len(),
                            "관심종목 우선 처리 (GlobalScore)"
                        );
                    }
                    (wl_syms, wl)
                }
                _ => (Vec::new(), Vec::new()),
            }
        } else {
            (Vec::new(), Vec::new())
        };

        // Phase 2: 나머지 심볼 (watchlist 제외)
        let exclude = if wl_tickers.is_empty() {
            None
        } else {
            Some(wl_tickers.as_slice())
        };
        // batch_size: 옵션에서 오버라이드 (0이면 제한 없음 = i64::MAX)
        let effective_batch_size = match options.batch_size {
            Some(0) => i64::MAX,
            Some(n) => n,
            None => config.fundamental_collect.batch_size,
        };
        let remaining = get_active_symbols_with_options(
            pool,
            effective_batch_size,
            resume_ticker.as_deref(),
            options.stale_hours,
            None,    // only: 전체
            exclude, // exclude: watchlist 심볼 제외
        )
        .await?;

        // 합치기: watchlist 먼저 + 나머지
        let mut combined = watchlist_symbols;
        combined.extend(remaining);
        combined
    };

    if target_symbols.is_empty() {
        info!("동기화할 심볼이 없습니다");
        checkpoint::save_checkpoint(
            pool,
            "global_score_sync",
            "",
            0,
            CheckpointStatus::Completed,
        )
        .await?;
        stats.elapsed = start.elapsed();
        return Ok(stats);
    }

    info!("GlobalScore 동기화 시작: {} 심볼", target_symbols.len());
    stats.total = target_symbols.len();

    // 시작 상태 저장
    checkpoint::save_checkpoint(pool, "global_score_sync", "", 0, CheckpointStatus::Running)
        .await?;

    for (idx, (symbol_info_id, ticker, market)) in target_symbols.iter().enumerate() {
        // 체크포인트 저장 (100개마다)
        if (idx + 1) % 100 == 0 {
            info!(
                progress = format!("{}/{}", idx + 1, stats.total),
                "GlobalScore 동기화 진행 중"
            );
            checkpoint::save_checkpoint(
                pool,
                "global_score_sync",
                ticker,
                (idx + 1) as i32,
                CheckpointStatus::Running,
            )
            .await?;
        }

        debug!(ticker = %ticker, market = %market, "GlobalScore 계산 중");

        match calculate_and_save(
            pool,
            &scorer,
            &data_provider,
            &indicator_engine,
            *symbol_info_id,
            ticker,
            market,
        )
        .await
        {
            Ok(true) => {
                stats.success += 1;
            }
            Ok(false) => {
                // 데이터 부족으로 스킵
                stats.skipped += 1;
            }
            Err(e) => {
                warn!(ticker = %ticker, error = %e, "GlobalScore 계산 실패");
                stats.errors += 1;
            }
        }

    }

    // 완료 상태 저장
    checkpoint::save_checkpoint(
        pool,
        "global_score_sync",
        "",
        stats.total as i32,
        CheckpointStatus::Completed,
    )
    .await?;

    stats.elapsed = start.elapsed();
    info!(
        "GlobalScore 동기화 완료: {}/{} 성공, {} 스킵, {} 오류",
        stats.success, stats.total, stats.skipped, stats.errors
    );

    Ok(stats)
}

/// 단일 심볼에 대해 GlobalScore 계산 및 저장.
async fn calculate_and_save(
    pool: &PgPool,
    scorer: &GlobalScorer,
    data_provider: &CachedHistoricalDataProvider,
    indicator_engine: &IndicatorEngine,
    symbol_info_id: Uuid,
    ticker: &str,
    market: &str,
) -> Result<bool> {
    // 1. MarketType 변환 (utils.rs 사용)
    let market_type = market_to_market_type(market);

    let symbol = Symbol::new(ticker, "", market_type);

    // 2. OHLCV 데이터 조회 (60일)
    let candles = data_provider
        .get_klines(ticker, Timeframe::D1, 60)
        .await
        .map_err(|e| CollectorError::Other(Box::new(e)))?;

    if candles.len() < 30 {
        debug!(ticker = %ticker, count = candles.len(), "데이터 부족 (최소 30개 필요)");
        return Ok(false);
    }

    // 3. 가격/거래량 데이터 추출
    let highs: Vec<Decimal> = candles.iter().map(|c| c.high).collect();
    let lows: Vec<Decimal> = candles.iter().map(|c| c.low).collect();
    let closes: Vec<Decimal> = candles.iter().map(|c| c.close).collect();
    
    let current_price = closes.last().copied();

    // 4. 거래대금 계산 (유동성 점수용)
    let avg_volume_amount = {
        let total_amount: Decimal = candles
            .iter()
            .map(|c| c.volume * c.close)
            .sum();
        Some(total_amount / Decimal::from(candles.len()))
    };

    // 5. ATR 기반 목표가/손절가 계산 (2 ATR 목표, 1 ATR 손절)
    let atr_params = AtrParams { period: 14 };
    let atr_result = indicator_engine.atr(&highs, &lows, &closes, atr_params);
    
    let (target_price, stop_price) = if let Some(price) = current_price {
        let latest_atr = atr_result
            .ok()
            .and_then(|v| v.last().copied().flatten())
            .unwrap_or(price * Decimal::new(2, 2)); // 기본 2%
        let target = Some(price + latest_atr * Decimal::from(2)); // +2 ATR
        let stop = Some(price - latest_atr);                       // -1 ATR
        (target, stop)
    } else {
        (None, None)
    };

    // 6. StructuralFeatures 계산 (ERS 점수용)
    let structural_features = StructuralFeaturesCalculator::from_candles(ticker, &candles, indicator_engine).ok();

    // 7. 거래대금 퍼센타일 계산 (시장 전체 기준)
    // 일단 거래대금 기반으로 대략적 퍼센타일 추정
    // 거래대금 1억 이하: 0.1, 10억: 0.3, 100억: 0.5, 1000억: 0.7, 1조 이상: 0.9
    let volume_percentile = avg_volume_amount.map(|amt| {
        let amt_f64 = amt.to_string().parse::<f64>().unwrap_or(0.0);
        if amt_f64 >= 1_000_000_000_000.0 { 0.95 }      // 1조 이상
        else if amt_f64 >= 100_000_000_000.0 { 0.8 }    // 1000억 이상
        else if amt_f64 >= 10_000_000_000.0 { 0.6 }     // 100억 이상
        else if amt_f64 >= 1_000_000_000.0 { 0.4 }      // 10억 이상
        else if amt_f64 >= 100_000_000.0 { 0.2 }        // 1억 이상
        else { 0.1 }                                     // 1억 미만
    });

    // 8. GlobalScore 계산
    let params = GlobalScorerParams {
        symbol: Some(symbol.to_string()),
        market_type: Some(market_type),
        entry_price: current_price,
        target_price,
        stop_price,
        avg_volume_amount,
        volume_percentile,
        structural_features,
    };

    let result = scorer
        .calculate(&candles, params)
        .map_err(|e| CollectorError::Other(Box::new(e)))?;

    // 9. DB 저장 (UPSERT)
    let mut component_scores_map = result.component_scores.clone();
    let penalties_value = component_scores_map
        .remove("penalties")
        .unwrap_or(Decimal::ZERO);

    let component_scores = serde_json::to_value(&component_scores_map)
        .map_err(|e| CollectorError::Other(Box::new(e)))?;

    let penalties = serde_json::json!({ "total": penalties_value.to_string() });

    // 추천 등급 (BUY, WATCH, HOLD)
    let grade = &result.recommendation;

    let confidence_str = if result.confidence >= Decimal::new(8, 1) {
        Some("HIGH".to_string())
    } else if result.confidence >= Decimal::new(6, 1) {
        Some("MEDIUM".to_string())
    } else {
        Some("LOW".to_string())
    };

    sqlx::query(r#"SELECT upsert_global_score($1, $2, $3, $4, $5, $6, $7, $8)"#)
        .bind(symbol_info_id)
        .bind(result.overall_score)
        .bind(grade)
        .bind(confidence_str.clone())
        .bind(&component_scores)
        .bind(&penalties)
        .bind(market)
        .bind(ticker)
        .execute(pool)
        .await
        .map_err(CollectorError::Database)?;

    // 10. Score History 저장 (일별 히스토리)
    // route_state는 symbol_fundamental에서 조회
    let route_state: Option<String> = sqlx::query_scalar(
        r#"SELECT route_state::text FROM symbol_fundamental WHERE symbol_info_id = $1"#
    )
    .bind(symbol_info_id)
    .fetch_optional(pool)
    .await
    .map_err(CollectorError::Database)?
    .flatten();

    let today = chrono::Utc::now().date_naive();
    sqlx::query(r#"
        INSERT INTO score_history (score_date, symbol, global_score, route_state, component_scores)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (score_date, symbol) DO UPDATE SET
            global_score = EXCLUDED.global_score,
            route_state = EXCLUDED.route_state,
            component_scores = EXCLUDED.component_scores
    "#)
    .bind(today)
    .bind(ticker)
    .bind(result.overall_score)
    .bind(route_state.as_deref())
    .bind(&component_scores)
    .execute(pool)
    .await
    .map_err(CollectorError::Database)?;

    debug!(
        ticker = %ticker,
        score = %result.overall_score,
        grade = %grade,
        "GlobalScore 저장 완료 (히스토리 포함)"
    );

    Ok(true)
}

/// 특정 티커로 심볼 조회.
async fn get_symbols_by_tickers(
    pool: &PgPool,
    tickers: &[&str],
) -> Result<Vec<(Uuid, String, String)>> {
    let results = sqlx::query_as::<_, (Uuid, String, String)>(
        r#"
        SELECT id, ticker, market
        FROM symbol_info
        WHERE ticker = ANY($1)
          AND is_active = true
        "#,
    )
    .bind(tickers)
    .fetch_all(pool)
    .await
    .map_err(CollectorError::Database)?;

    Ok(results)
}



/// 활성 심볼 조회 (resume, stale_hours, 티커 필터링 지원).
/// QueryBuilder 사용으로 SQL 주입 방지.
///
/// # 인자
/// * `only_tickers` - Some이면 해당 티커만 포함 (Phase 1: watchlist)
/// * `exclude_tickers` - Some이면 해당 티커 제외 (Phase 2: watchlist 제외)
async fn get_active_symbols_with_options(
    pool: &PgPool,
    limit: i64,
    resume_ticker: Option<&str>,
    stale_hours: Option<u32>,
    only_tickers: Option<&[String]>,
    exclude_tickers: Option<&[String]>,
) -> Result<Vec<(Uuid, String, String)>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
        SELECT si.id, si.ticker, si.market
        FROM symbol_info si
        LEFT JOIN symbol_global_score sgs ON si.id = sgs.symbol_info_id
        INNER JOIN ohlcv_metadata om ON om.symbol = si.ticker
          AND om.timeframe = '1d' AND om.total_candles >= 50
        WHERE si.is_active = true
          AND si.market != 'CRYPTO'
        "#,
    );

    if let Some(t) = resume_ticker {
        qb.push(" AND si.ticker > ");
        qb.push_bind(t.to_string());
    }

    if let Some(hours) = stale_hours {
        qb.push(" AND (sgs.updated_at IS NULL OR sgs.updated_at < NOW() - INTERVAL '");
        qb.push(hours.to_string());
        qb.push(" hours')");
    }

    // only 필터 (Phase 1: watchlist 심볼만)
    if let Some(tickers) = only_tickers {
        if !tickers.is_empty() {
            qb.push(" AND si.ticker = ANY(");
            qb.push_bind(tickers.to_vec());
            qb.push(")");
        }
    }

    // exclude 필터 (Phase 2: watchlist 심볼 제외)
    if let Some(tickers) = exclude_tickers {
        if !tickers.is_empty() {
            qb.push(" AND si.ticker != ALL(");
            qb.push_bind(tickers.to_vec());
            qb.push(")");
        }
    }

    qb.push(" ORDER BY si.ticker LIMIT ");
    qb.push_bind(limit);

    let results = qb
        .build_query_as::<(Uuid, String, String)>()
        .fetch_all(pool)
        .await
        .map_err(CollectorError::Database)?;

    Ok(results)
}
