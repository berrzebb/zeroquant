//! 분석 지표 동기화 모듈.
//!
//! RouteState, MarketRegime, TTM Squeeze 지표를 계산하여 symbol_fundamental 테이블에 저장합니다.

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sqlx::{PgPool, QueryBuilder, Postgres};
use std::time::Instant;
use tracing::{debug, info, warn};
use uuid::Uuid;

use trader_analytics::{
    indicators::IndicatorEngine,
    MarketRegimeCalculator, RouteStateCalculator,
};
use trader_core::{Kline, Timeframe};

use super::checkpoint::{self, CheckpointStatus};
use super::utils::{calculate_ttm_squeeze, to_screaming_snake_case};
use super::watchlist_helper;
use crate::config::CollectorConfig;
use crate::error::CollectorError;
use crate::stats::CollectionStats;
use crate::Result;

/// 지표 동기화 옵션
#[derive(Debug, Default)]
pub struct IndicatorSyncOptions {
    /// 중단점부터 재개
    pub resume: bool,
    /// N시간 이내 업데이트된 심볼 스킵
    pub stale_hours: Option<u32>,
    /// 배치 크기 오버라이드 (None이면 config 기본값 사용, 0이면 제한 없음)
    pub batch_size: Option<i64>,
}

/// 분석 지표 동기화 실행.
///
/// # 동작
/// 1. 지표가 오래된 심볼 목록 조회
/// 2. 각 심볼에 대해 OHLCV 데이터 조회
/// 3. RouteState, MarketRegime, TTM Squeeze 계산
/// 4. DB에 저장
///
/// # 인자
/// * `pool` - 데이터베이스 연결 풀
/// * `config` - Collector 설정
/// * `symbols` - 특정 심볼만 처리 (None이면 전체)
pub async fn sync_indicators(
    pool: &PgPool,
    config: &CollectorConfig,
    symbols: Option<String>,
) -> Result<CollectionStats> {
    let options = IndicatorSyncOptions::default();
    sync_indicators_with_options(pool, config, symbols, options).await
}

/// 분석 지표 동기화 실행 (옵션 포함).
pub async fn sync_indicators_with_options(
    pool: &PgPool,
    config: &CollectorConfig,
    symbols: Option<String>,
    options: IndicatorSyncOptions,
) -> Result<CollectionStats> {
    let start = Instant::now();
    let mut stats = CollectionStats::new();

    // 계산기 초기화
    let route_state_calc = RouteStateCalculator::new();
    let market_regime_calc = MarketRegimeCalculator::new();
    let indicator_engine = IndicatorEngine::new();

    // 체크포인트 로드 (resume 모드)
    let resume_ticker = if options.resume {
        match checkpoint::load_checkpoint(pool, "indicator_sync").await? {
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
        // 특정 심볼 지정
        let ticker_list: Vec<&str> = tickers.split(',').map(|s| s.trim()).collect();
        get_symbols_by_tickers(pool, &ticker_list).await?
    } else {
        let stale_threshold = if let Some(hours) = options.stale_hours {
            Utc::now() - Duration::hours(hours as i64)
        } else {
            Utc::now() - Duration::days(config.fundamental_collect.stale_days)
        };

        // Phase 1: 관심종목 우선 처리 (체크포인트 무시)
        let (watchlist_symbols, wl_tickers) = if config.prioritize_watchlist {
            match watchlist_helper::fetch_all_priority_tickers(pool).await {
                Ok(wl) if !wl.is_empty() => {
                    // watchlist 심볼 중 stale 조건에 해당하는 것만 조회
                    let wl_syms = get_stale_indicator_symbols_with_resume(
                        pool,
                        stale_threshold,
                        wl.len() as i64,
                        None, // 체크포인트 무시
                        Some(wl.as_slice()), // only: watchlist 심볼만
                        None,
                    )
                    .await?;
                    if !wl_syms.is_empty() {
                        tracing::info!(
                            count = wl_syms.len(),
                            "관심종목 우선 처리 (Indicator)"
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
        let remaining = get_stale_indicator_symbols_with_resume(
            pool,
            stale_threshold,
            effective_batch_size,
            resume_ticker.as_deref(),
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
        checkpoint::save_checkpoint(pool, "indicator_sync", "", 0, CheckpointStatus::Completed)
            .await?;
        stats.elapsed = start.elapsed();
        return Ok(stats);
    }

    info!("지표 동기화 시작: {} 심볼", target_symbols.len());
    stats.total = target_symbols.len();

    // 시작 상태 저장
    checkpoint::save_checkpoint(pool, "indicator_sync", "", 0, CheckpointStatus::Running).await?;

    for (idx, (symbol_info_id, ticker, market, yahoo_symbol)) in target_symbols.iter().enumerate() {
        // 체크포인트 저장 (100개마다)
        if (idx + 1) % 100 == 0 {
            info!(
                progress = format!("{}/{}", idx + 1, stats.total),
                "지표 동기화 진행 중"
            );
            checkpoint::save_checkpoint(
                pool,
                "indicator_sync",
                ticker,
                (idx + 1) as i32,
                CheckpointStatus::Running,
            )
            .await?;
        }

        let ticker = ticker.clone();
        let market = market.clone();
        let yahoo_symbol = yahoo_symbol.clone();
        debug!(ticker = %ticker, market = %market, yahoo_symbol = ?yahoo_symbol, "지표 계산 중");

        // OHLCV 데이터 조회 (80개 - MarketRegime용 70개 + 여유분)
        // yahoo_symbol이 있으면 우선 사용, 없으면 ticker로 조회
        let candles = match get_candles(pool, &ticker, yahoo_symbol.as_deref(), 80).await {
            Ok(c) if c.len() >= 40 => c,
            Ok(c) => {
                debug!(
                    ticker = %ticker,
                    count = c.len(),
                    "캔들 데이터 부족 (최소 40개 필요)"
                );
                stats.skipped += 1;
                continue;
            }
            Err(e) => {
                warn!(ticker = %ticker, error = %e, "캔들 조회 실패");
                stats.errors += 1;
                continue;
            }
        };

        // RouteState 계산 (DB ENUM은 대문자)
        let route_state = match route_state_calc.calculate(&candles) {
            Ok(state) => Some(format!("{:?}", state).to_uppercase()),
            Err(e) => {
                debug!(ticker = %ticker, error = %e, "RouteState 계산 실패");
                None
            }
        };

        // MarketRegime 계산 (70개 이상 필요)
        // 값 형식: StrongUptrend → STRONG_UPTREND, BottomBounce → BOTTOM_BOUNCE
        let regime = if candles.len() >= 70 {
            match market_regime_calc.calculate(&candles) {
                Ok(result) => {
                    let regime_str = format!("{:?}", result.regime);
                    // CamelCase → SNAKE_CASE 변환
                    Some(to_screaming_snake_case(&regime_str))
                }
                Err(e) => {
                    debug!(ticker = %ticker, error = %e, "MarketRegime 계산 실패");
                    None
                }
            }
        } else {
            None
        };

        // TTM Squeeze 계산 (20개 이상 필요)
        let (ttm_squeeze, ttm_squeeze_cnt) = if candles.len() >= 20 {
            calculate_ttm_squeeze(&indicator_engine, &candles)
        } else {
            (None, None)
        };

        // DB 업데이트
        match update_indicators(
            pool,
            *symbol_info_id,
            route_state.as_deref(),
            regime.as_deref(),
            ttm_squeeze,
            ttm_squeeze_cnt,
        )
        .await
        {
            Ok(_) => {
                debug!(
                    ticker = %ticker,
                    route_state = ?route_state,
                    regime = ?regime,
                    ttm_squeeze = ?ttm_squeeze,
                    ttm_squeeze_cnt = ?ttm_squeeze_cnt,
                    "지표 업데이트 완료"
                );
                stats.success += 1;
            }
            Err(e) => {
                warn!(ticker = %ticker, error = %e, "지표 DB 업데이트 실패");
                stats.errors += 1;
            }
        }

    }

    // 완료 상태 저장
    checkpoint::save_checkpoint(
        pool,
        "indicator_sync",
        "",
        stats.total as i32,
        CheckpointStatus::Completed,
    )
    .await?;

    stats.elapsed = start.elapsed();
    Ok(stats)
}

// to_screaming_snake_case, calculate_ttm_squeeze는 utils.rs로 이동됨

/// 특정 티커로 심볼 조회.
async fn get_symbols_by_tickers(
    pool: &PgPool,
    tickers: &[&str],
) -> Result<Vec<(Uuid, String, String, Option<String>)>> {
    let results = sqlx::query_as::<_, (Uuid, String, String, Option<String>)>(
        r#"
        SELECT id, ticker, market, yahoo_symbol
        FROM symbol_info
        WHERE ticker = ANY($1)
          AND is_active = true
          AND market != 'CRYPTO'
        "#,
    )
    .bind(tickers)
    .fetch_all(pool)
    .await
    .map_err(CollectorError::Database)?;

    Ok(results)
}



/// 지표가 오래된 심볼 조회 (resume 지원, 티커 필터링).
/// QueryBuilder 사용으로 SQL 주입 방지.
///
/// # 인자
/// * `pool` - DB 연결 풀
/// * `older_than` - 이 시점 이전에 업데이트된 심볼만 조회
/// * `limit` - 최대 조회 수
/// * `resume_ticker` - 이 티커 이후부터 조회 (체크포인트 재개)
/// * `only_tickers` - Some이면 해당 티커만 포함 (Phase 1: watchlist)
/// * `exclude_tickers` - Some이면 해당 티커 제외 (Phase 2: watchlist 제외)
async fn get_stale_indicator_symbols_with_resume(
    pool: &PgPool,
    older_than: chrono::DateTime<Utc>,
    limit: i64,
    resume_ticker: Option<&str>,
    only_tickers: Option<&[String]>,
    exclude_tickers: Option<&[String]>,
) -> Result<Vec<(Uuid, String, String, Option<String>)>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
        SELECT si.id, si.ticker, si.market, si.yahoo_symbol
        FROM symbol_info si
        LEFT JOIN symbol_fundamental sf ON si.id = sf.symbol_info_id
        INNER JOIN ohlcv_metadata om ON om.symbol = si.ticker
          AND om.timeframe = '1d' AND om.total_candles >= 40
        WHERE si.is_active = true
          AND si.market != 'CRYPTO'
          AND (
              sf.route_state IS NULL
              OR sf.regime IS NULL
              OR sf.updated_at IS NULL
              OR sf.updated_at < "#,
    );
    qb.push_bind(older_than);
    qb.push(")");

    // resume_ticker 조건 (파라미터 바인딩으로 SQL 주입 방지)
    if let Some(t) = resume_ticker {
        qb.push(" AND si.ticker > ");
        qb.push_bind(t.to_string());
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
        .build_query_as::<(Uuid, String, String, Option<String>)>()
        .fetch_all(pool)
        .await
        .map_err(CollectorError::Database)?;

    Ok(results)
}

/// OHLCV 캔들 데이터 조회.
/// ohlcv 테이블의 symbol 컬럼은 순수 ticker만 저장합니다.
/// yahoo_symbol은 더 이상 사용되지 않습니다 (레거시 파라미터).
async fn get_candles(
    pool: &PgPool,
    ticker: &str,
    _yahoo_symbol: Option<&str>, // 미사용 (ticker로 통일됨)
    limit: i64,
) -> Result<Vec<Kline>> {
    // ticker로만 조회 (OHLCV 테이블은 순수 ticker만 저장)
    let rows = sqlx::query_as::<
        _,
        (
            chrono::DateTime<Utc>,
            Decimal,
            Decimal,
            Decimal,
            Decimal,
            Decimal,
            Option<chrono::DateTime<Utc>>,
        ),
    >(
        r#"
        SELECT open_time, open, high, low, close, volume, close_time
        FROM ohlcv
        WHERE symbol = $1 AND timeframe = '1d'
        ORDER BY open_time DESC
        LIMIT $2
        "#,
    )
    .bind(ticker)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(CollectorError::Database)?;

    if rows.is_empty() {
        return Ok(vec![]);
    }

    // 시간순 정렬 (DESC로 가져왔으므로 reverse)
    let mut candles: Vec<Kline> = rows
        .into_iter()
        .map(
            |(open_time, open, high, low, close, volume, close_time)| Kline {
                ticker: ticker.to_string(),
                timeframe: Timeframe::D1,
                open_time,
                open,
                high,
                low,
                close,
                volume,
                close_time: close_time.unwrap_or(open_time),
                quote_volume: None,
                num_trades: None,
            },
        )
        .collect();

    candles.reverse();
    Ok(candles)
}

/// DB에 지표 업데이트.
/// route_state는 PostgreSQL ENUM 타입이므로 명시적 캐스팅이 필요합니다.
async fn update_indicators(
    pool: &PgPool,
    symbol_info_id: Uuid,
    route_state: Option<&str>,
    regime: Option<&str>,
    ttm_squeeze: Option<bool>,
    ttm_squeeze_cnt: Option<i32>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO symbol_fundamental (symbol_info_id, route_state, regime, ttm_squeeze, ttm_squeeze_cnt, fetched_at)
        VALUES ($1, $2::route_state, $3, $4, $5, NOW())
        ON CONFLICT (symbol_info_id) DO UPDATE SET
            route_state = COALESCE(EXCLUDED.route_state, symbol_fundamental.route_state),
            regime = COALESCE(EXCLUDED.regime, symbol_fundamental.regime),
            ttm_squeeze = COALESCE(EXCLUDED.ttm_squeeze, symbol_fundamental.ttm_squeeze),
            ttm_squeeze_cnt = COALESCE(EXCLUDED.ttm_squeeze_cnt, symbol_fundamental.ttm_squeeze_cnt),
            updated_at = NOW()
        "#,
    )
    .bind(symbol_info_id)
    .bind(route_state)
    .bind(regime)
    .bind(ttm_squeeze)
    .bind(ttm_squeeze_cnt)
    .execute(pool)
    .await
    .map_err(CollectorError::Database)?;

    Ok(())
}
