//! OHLCV 데이터 수집 모듈.
//!
//! OHLCV 데이터 수집과 동시에 분석 지표(RouteState, MarketRegime, TTM Squeeze)를
//! 계산하여 symbol_fundamental 테이블에 저장합니다.
//!
//! GlobalScore는 별도 워크플로우(global_score_sync)에서 계산합니다.
//!
//! # 데이터 소스 이원화
//!
//! - **국내 (KR)**: KRX API 우선 사용, 실패 시 Yahoo Finance fallback
//! - **해외 (US, JP 등)**: Yahoo Finance 사용

use crate::{CollectionStats, CollectorConfig, Result};
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

use super::watchlist_helper;
use trader_analytics::{
    indicators::IndicatorEngine, MarketRegimeCalculator, RouteStateCalculator,
};
use trader_core::{CredentialEncryptor, Kline, Timeframe};
use trader_data::cache::historical::CachedHistoricalDataProvider;
use trader_data::provider::krx_api::KrxApiClient;
use uuid::Uuid;

use super::checkpoint::{self, CheckpointStatus};
use super::utils::{calculate_ttm_squeeze, to_screaming_snake_case};

/// ETA 및 시장별 진행률을 추적하는 트래커.
struct ProgressTracker {
    overall_start: Instant,
    market_totals: HashMap<String, usize>,
    market_completed: HashMap<String, usize>,
    recent_durations: Vec<Duration>,
    window_size: usize,
    last_log_time: Instant,
    completed: usize,
    total: usize,
}

impl ProgressTracker {
    /// 심볼 목록에서 시장별 카운트를 초기화.
    fn new(symbols: &[(Uuid, String, String)]) -> Self {
        let mut market_totals: HashMap<String, usize> = HashMap::new();
        for (_, _, market) in symbols {
            *market_totals.entry(market.clone()).or_insert(0) += 1;
        }
        let total = symbols.len();
        let now = Instant::now();
        Self {
            overall_start: now,
            market_totals,
            market_completed: HashMap::new(),
            recent_durations: Vec::with_capacity(50),
            window_size: 50,
            last_log_time: now,
            completed: 0,
            total,
        }
    }

    /// 완료된 심볼 기록 및 이동 평균 업데이트.
    fn record_completion(&mut self, market: &str, duration: Duration) {
        self.completed += 1;
        *self.market_completed.entry(market.to_string()).or_insert(0) += 1;

        if self.recent_durations.len() >= self.window_size {
            self.recent_durations.remove(0);
        }
        self.recent_durations.push(duration);
    }

    /// 이동 평균 기반 남은 시간 추정.
    fn estimated_remaining(&self) -> Option<Duration> {
        if self.recent_durations.is_empty() || self.completed == 0 {
            return None;
        }
        let avg: Duration = self.recent_durations.iter().sum::<Duration>()
            / self.recent_durations.len() as u32;
        let remaining = self.total.saturating_sub(self.completed);
        Some(avg * remaining as u32)
    }

    /// 10개마다 또는 1분마다 로그를 출력할지 결정.
    fn should_log(&self) -> bool {
        self.completed % 10 == 0
            || self.completed == self.total
            || self.last_log_time.elapsed() >= Duration::from_secs(60)
    }

    /// 진행률 로그 출력.
    fn log_progress(&mut self, ticker: &str, market: &str) {
        if !self.should_log() {
            return;
        }
        self.last_log_time = Instant::now();

        let percent = if self.total > 0 {
            (self.completed * 100) / self.total
        } else {
            0
        };
        let elapsed = self.overall_start.elapsed();
        let eta_str = self.estimated_remaining()
            .map(format_duration)
            .unwrap_or_else(|| "계산 중".to_string());

        // 시장별 진행률 문자열 생성
        let market_progress: Vec<String> = self.market_totals.iter().map(|(m, total)| {
            let done = self.market_completed.get(m).copied().unwrap_or(0);
            format!("{}: {}/{}", m, done, total)
        }).collect();

        tracing::info!(
            "[{}/{}] ({}%) | ETA: {} | 경과: {} | {} | 현재: {} ({})",
            self.completed,
            self.total,
            percent,
            eta_str,
            format_duration(elapsed),
            market_progress.join(", "),
            ticker,
            market,
        );
    }
}

/// Duration을 사람이 읽기 쉬운 문자열로 변환.
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// OHLCV 데이터 수집 및 지표 동시 업데이트
///
/// OHLCV 데이터를 수집하고, 성공한 심볼에 대해 즉시 분석 지표를 계산합니다.
/// - RouteState (매매 단계)
/// - MarketRegime (시장 레짐)
/// - TTM Squeeze (에너지 응축)
///
/// # Arguments
///
/// * `pool` - DB 연결 풀
/// * `config` - 수집 설정
/// * `symbols` - 특정 심볼 지정 (쉼표 구분), None이면 전체
/// * `stale_hours` - 이 시간보다 오래된 심볼만 수집 (증분 수집), None이면 전체
pub async fn collect_ohlcv(
    pool: &PgPool,
    config: &CollectorConfig,
    symbols: Option<String>,
    stale_hours: Option<u32>,
) -> Result<CollectionStats> {
    let start = Instant::now();
    let mut stats = CollectionStats::new();

    tracing::info!("OHLCV 수집 및 지표 업데이트 시작");

    // 지표 계산기 초기화
    let route_state_calc = RouteStateCalculator::new();
    let market_regime_calc = MarketRegimeCalculator::new();
    let indicator_engine = IndicatorEngine::new();

    // 수집할 심볼 목록 결정 (symbol_info_id, ticker, market 포함)
    let target_symbols = match symbols {
        Some(ref s) => {
            // 쉼표로 구분된 심볼 파싱
            let tickers: Vec<&str> = s.split(',').map(|s| s.trim()).collect();
            let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
                "SELECT id, ticker, market FROM symbol_info
                 WHERE ticker = ANY($1)
                   AND is_active = true",
            )
            .bind(&tickers)
            .fetch_all(pool)
            .await?;
            tracing::info!(count = rows.len(), "특정 심볼 수집");
            rows
        }
        None => {
            // DB에서 활성화된 심볼 조회 (STOCK, ETF만)
            // target_markets가 설정된 경우 해당 시장만, 아니면 전체 시장
            // stale_hours가 지정되면 해당 시간 이전에 업데이트된 심볼만 선택 (증분 수집)
            let target_markets = &config.ohlcv_collect.target_markets;
            let market_filter = if target_markets.is_empty() {
                None
            } else {
                Some(target_markets.clone())
            };

            // 증분 수집: OHLCV 테이블의 최신 데이터 기준
            // stale_threshold 이후 1d OHLCV가 없는 심볼만 선택
            let rows: Vec<(Uuid, String, String)> = if let Some(hours) = stale_hours {
                let stale_threshold = Utc::now() - chrono::Duration::hours(hours as i64);
                if let Some(ref markets) = market_filter {
                    sqlx::query_as(
                        r#"
                        SELECT si.id, si.ticker, si.market
                        FROM symbol_info si
                        WHERE si.is_active = true
                          AND si.symbol_type IN ('STOCK', 'ETF')
                          AND si.market = ANY($1)
                          AND NOT EXISTS (
                              SELECT 1 FROM ohlcv o
                              WHERE o.symbol = si.ticker
                                AND o.timeframe = '1d'
                                AND o.open_time >= $2
                          )
                        ORDER BY
                          CASE si.market WHEN 'KR' THEN 1 WHEN 'US' THEN 2 ELSE 3 END,
                          si.ticker
                        "#,
                    )
                    .bind(markets)
                    .bind(stale_threshold)
                    .fetch_all(pool)
                    .await?
                } else {
                    sqlx::query_as(
                        r#"
                        SELECT si.id, si.ticker, si.market
                        FROM symbol_info si
                        WHERE si.is_active = true
                          AND si.symbol_type IN ('STOCK', 'ETF')
                          AND NOT EXISTS (
                              SELECT 1 FROM ohlcv o
                              WHERE o.symbol = si.ticker
                                AND o.timeframe = '1d'
                                AND o.open_time >= $1
                          )
                        ORDER BY si.market, si.ticker
                        "#,
                    )
                    .bind(stale_threshold)
                    .fetch_all(pool)
                    .await?
                }
            } else if let Some(ref markets) = market_filter {
                sqlx::query_as(
                    r#"
                    SELECT id, ticker, market FROM symbol_info
                    WHERE is_active = true
                      AND symbol_type IN ('STOCK', 'ETF')
                      AND market = ANY($1)
                    ORDER BY
                      CASE market WHEN 'KR' THEN 1 WHEN 'US' THEN 2 ELSE 3 END,
                      ticker
                    "#,
                )
                .bind(markets)
                .fetch_all(pool)
                .await?
            } else {
                sqlx::query_as(
                    r#"
                    SELECT id, ticker, market FROM symbol_info
                    WHERE is_active = true
                      AND symbol_type IN ('STOCK', 'ETF')
                    ORDER BY market, ticker
                    "#,
                )
                .fetch_all(pool)
                .await?
            };

            let market_desc = if target_markets.is_empty() {
                "전체 시장".to_string()
            } else {
                target_markets.join(", ")
            };

            if stale_hours.is_some() {
                tracing::info!(
                    count = rows.len(),
                    stale_hours = stale_hours,
                    markets = %market_desc,
                    "증분 수집: 업데이트 필요한 심볼 조회 완료"
                );
            } else {
                tracing::info!(count = rows.len(), markets = %market_desc, "활성 심볼 조회 완료 (STOCK/ETF)");
            }
            rows
        }
    };

    // 관심종목 우선 처리: watchlist 심볼을 앞으로 이동
    let target_symbols = if config.prioritize_watchlist && symbols.is_none() {
        match watchlist_helper::fetch_all_priority_tickers(pool).await {
            Ok(wl_tickers) if !wl_tickers.is_empty() => {
                let wl_set = watchlist_helper::to_hashset(&wl_tickers);
                let mut watchlist_first: Vec<(Uuid, String, String)> = Vec::with_capacity(target_symbols.len());
                let mut rest: Vec<(Uuid, String, String)> = Vec::with_capacity(target_symbols.len());

                for item in target_symbols {
                    if wl_set.contains(&item.1) {
                        watchlist_first.push(item);
                    } else {
                        rest.push(item);
                    }
                }
                tracing::info!(
                    watchlist = watchlist_first.len(),
                    others = rest.len(),
                    "관심종목 우선 처리 적용"
                );
                watchlist_first.extend(rest);
                watchlist_first
            }
            _ => target_symbols,
        }
    } else {
        target_symbols
    };

    if target_symbols.is_empty() {
        tracing::warn!("수집할 심볼이 없습니다");
        stats.elapsed = start.elapsed();
        return Ok(stats);
    }

    // 수집할 타임프레임 목록
    let timeframes = if config.ohlcv_collect.timeframes.is_empty() {
        vec!["1d".to_string()]
    } else {
        config.ohlcv_collect.timeframes.clone()
    };

    // 기본 타임프레임 (D1) 기준 날짜 범위 계산
    let primary_timeframe = timeframes.first().map(|s| s.as_str()).unwrap_or("1d");
    let (start_date, end_date) = determine_date_range(config, primary_timeframe);

    tracing::info!(
        timeframes = ?timeframes,
        start = %start_date,
        end = %end_date,
        "타임프레임별 수집 설정"
    );

    // 시장별 심볼 분류
    let kr_symbols: Vec<_> = target_symbols
        .iter()
        .filter(|(_, _, m)| m == "KR")
        .collect();
    let foreign_symbols: Vec<_> = target_symbols
        .iter()
        .filter(|(_, _, m)| m != "KR")
        .collect();

    tracing::info!(
        total = target_symbols.len(),
        kr = kr_symbols.len(),
        foreign = foreign_symbols.len(),
        start_date = ?start_date,
        end_date = ?end_date,
        "수집 범위 설정 완료 (시장별 분류)"
    );

    // 데이터 제공자 초기화
    // Yahoo Finance (해외 + KR fallback)
    let yahoo_provider = CachedHistoricalDataProvider::new(pool.clone());

    // KRX API 클라이언트 (국내 전용) - 설정에서 활성화된 경우에만
    let krx_client = if config.providers.krx_api_enabled {
        init_krx_client(pool).await
    } else {
        tracing::info!("KRX API 비활성화됨 (PROVIDER_KRX_API_ENABLED=false)");
        None
    };

    // =========================================================================
    // KRX API 일괄 수집 (국내 전 종목)
    // =========================================================================
    // KRX API가 활성화된 경우, 먼저 전 종목 일괄 수집 후 개별 fallback
    let mut kr_collected_tickers: HashSet<String> = HashSet::new();

    if let Some(ref client) = krx_client {
        // KRX API는 T+1 데이터 제공 (당일 데이터 없음)
        // 따라서 전일 날짜로 조회해야 데이터가 존재함
        let krx_query_date = end_date - chrono::Duration::days(1);
        let base_date = krx_query_date.format("%Y%m%d").to_string();
        tracing::info!(
            base_date = %base_date,
            original_end_date = %end_date,
            kr_symbols = kr_symbols.len(),
            "KRX API 일괄 수집 시작 (KOSPI + KOSDAQ, T-1 날짜 사용)"
        );

        match client.fetch_all_daily_trades(&base_date).await {
            Ok(daily_trades) => {
                tracing::info!(
                    count = daily_trades.len(),
                    "KRX API 일괄 조회 완료 - 배치 저장 시작"
                );

                // 종목코드 → symbol_info_id 매핑 생성
                let kr_ticker_map: std::collections::HashMap<String, Uuid> = kr_symbols
                    .iter()
                    .map(|(id, ticker, _)| (ticker.clone(), *id))
                    .collect();

                // 배치 저장용 벡터에 데이터 수집
                let mut ohlcv_rows: Vec<KrxOhlcvRow> = Vec::with_capacity(daily_trades.len());
                let mut market_info_rows: Vec<KrxMarketInfoRow> = Vec::new();

                for trade in &daily_trades {
                    // 6자리 단축코드 추출 (표준코드에서)
                    let short_code = if trade.code.len() >= 6 {
                        trade.code.chars().take(6).collect::<String>()
                    } else {
                        trade.code.clone()
                    };

                    // symbol_info에 등록된 종목만 처리
                    if let Some(&symbol_info_id) = kr_ticker_map.get(&short_code) {
                        // OHLCV 데이터 수집
                        if let (Some(open), Some(high), Some(low)) = (trade.open, trade.high, trade.low) {
                            let open_time = trade.date.and_hms_opt(0, 0, 0)
                                .map(|dt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
                                .unwrap_or_else(Utc::now);

                            ohlcv_rows.push(KrxOhlcvRow {
                                ticker: short_code.clone(),
                                symbol_info_id,
                                open_time,
                                open,
                                high,
                                low,
                                close: trade.close,
                                volume: trade.volume,
                            });
                            kr_collected_tickers.insert(short_code.clone());
                        }

                        // 시가총액, 상장주식수
                        if trade.market_cap.is_some() || trade.shares_outstanding.is_some() {
                            market_info_rows.push(KrxMarketInfoRow {
                                symbol_info_id,
                                market_cap: trade.market_cap,
                                shares_outstanding: trade.shares_outstanding,
                            });
                        }
                    }
                }

                // 배치 UPSERT 실행
                let ohlcv_count = ohlcv_rows.len();
                match save_krx_ohlcv_batch(pool, &ohlcv_rows).await {
                    Ok(affected) => {
                        stats.total_klines += ohlcv_count;
                        tracing::info!(
                            rows = ohlcv_count,
                            affected = affected,
                            "KRX OHLCV 배치 저장 완료"
                        );
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "KRX OHLCV 배치 저장 실패");
                    }
                }

                if !market_info_rows.is_empty() {
                    if let Err(e) = update_market_info_batch(pool, &market_info_rows).await {
                        tracing::error!(error = %e, "KRX 시가총액 배치 업데이트 실패");
                    }
                }

                tracing::info!(
                    ohlcv = ohlcv_count,
                    market_info = market_info_rows.len(),
                    unique_tickers = kr_collected_tickers.len(),
                    "KRX API 일괄 배치 수집 완료"
                );
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "KRX API 일괄 수집 실패 - 개별 수집으로 fallback"
                );
            }
        }
    }

    tracing::info!(
        total = target_symbols.len(),
        "OHLCV 수집 시작 - 총 {}개 심볼", target_symbols.len()
    );

    // 심볼별 수집 (KRX API로 이미 수집된 종목은 fallback 대상에서 제외)
    // 소유권 이전으로 Send 제약 충족 (buffer_unordered용)
    let fallback_symbols: Vec<(Uuid, String, String)> = target_symbols
        .into_iter()
        .filter(|(_, ticker, market)| {
            // KR 종목 중 이미 KRX API로 수집된 종목은 스킵
            !(market == "KR" && kr_collected_tickers.contains(ticker))
        })
        .collect();

    let fallback_count = fallback_symbols.len();
    if !kr_collected_tickers.is_empty() {
        tracing::info!(
            kr_collected = kr_collected_tickers.len(),
            fallback_needed = fallback_count,
            "KRX 일괄 수집 완료 - 나머지 종목 개별 수집"
        );
    }

    // =========================================================================
    // 사전 필터링: 일괄 날짜 범위 조회 → 메모리에서 수집 대상 결정
    // =========================================================================
    let fallback_tickers: Vec<String> = fallback_symbols.iter().map(|(_, t, _)| t.clone()).collect();
    let existing_ranges = get_all_existing_ranges(pool, &fallback_tickers, primary_timeframe).await;

    // 우선순위 종목 세트 (watchlist + 전략 관심 종목)
    let priority_set: HashSet<String> = if config.ohlcv_collect.max_gap_days_non_priority > 0 {
        watchlist_helper::fetch_all_priority_tickers(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect()
    } else {
        HashSet::new()
    };

    let max_gap = config.ohlcv_collect.max_gap_days_non_priority;

    // 메모리에서 사전 필터링 — 수집이 필요한 심볼만 추출
    let mut gap_skipped = 0usize;
    let symbols_needing_collection: Vec<_> = fallback_symbols.into_iter().filter(|(_, ticker, _)| {
        let (existing_start, existing_end) = existing_ranges
            .get(ticker).copied().unwrap_or((None, None));
        let (past, future) = calculate_missing_ranges(start_date, end_date, existing_start, existing_end);

        // 수집할 것이 없으면 스킵
        if past.is_none() && future.is_none() {
            return false;
        }

        // 비우선순위 종목의 대규모 갭 백필 제한
        if max_gap > 0 && !priority_set.contains(ticker) {
            if let Some((ps, pe)) = &past {
                let gap_days = (*pe - *ps).num_days();
                if gap_days > max_gap {
                    tracing::debug!(
                        ticker = ticker,
                        gap_days = gap_days,
                        max_gap = max_gap,
                        "비우선순위 종목 대규모 갭 스킵"
                    );
                    gap_skipped += 1;
                    return false;
                }
            }
        }

        true
    }).collect();

    let pre_filter_skipped = fallback_tickers.len() - symbols_needing_collection.len();
    tracing::info!(
        total = fallback_tickers.len(),
        needs_collection = symbols_needing_collection.len(),
        skipped = pre_filter_skipped,
        gap_skipped = gap_skipped,
        max_gap_days = max_gap,
        priority_count = priority_set.len(),
        "사전 필터링 완료 — 수집 대상만 루프 진입"
    );

    // 동시 수집을 위한 Semaphore 기반 동시성 제한
    let concurrent_limit = config.ohlcv_collect.concurrent_limit.unwrap_or(5);
    let semaphore = Arc::new(Semaphore::new(concurrent_limit));
    let collection_count = symbols_needing_collection.len();

    tracing::info!(
        concurrent_limit = concurrent_limit,
        collection_count = collection_count,
        "Yahoo fallback 동시 수집 시작"
    );

    let request_delay = config.ohlcv_collect.request_delay();

    // 진행률 트래커 초기화
    let mut progress = ProgressTracker::new(&symbols_needing_collection);

    // 체크포인트: 수집 시작
    let _ = checkpoint::save_checkpoint(pool, "ohlcv_collect", "", 0, CheckpointStatus::Running).await;

    for (idx, (symbol_info_id, ticker, market)) in symbols_needing_collection.iter().enumerate() {
        stats.total += 1;
        let symbol_start = Instant::now();

        // Semaphore로 동시 실행 수 제한 (permit 획득까지 대기)
        let _permit = semaphore.clone().acquire_owned().await.expect("세마포어 획득 실패");

        // 일괄 조회 결과에서 기존 데이터 범위 가져오기 (개별 DB 쿼리 불필요)
        let (existing_start, existing_end) = existing_ranges
            .get(ticker).copied().unwrap_or((None, None));

        // 누락 구간 계산
        let (past_range, future_range) = calculate_missing_ranges(
            start_date, end_date, existing_start, existing_end,
        );

        // 누락 구간이 없으면 스킵 (사전 필터링 후에도 안전 장치)
        if past_range.is_none() && future_range.is_none() {
            tracing::debug!(ticker = ticker, "이미 수집된 데이터 - 스킵");
            stats.success += 1;
            progress.record_completion(market, symbol_start.elapsed());
            progress.log_progress(ticker, market);
            continue;
        }

        // 대규모 과거 데이터 수집 감지 (1년 이상 갭)
        if let Some((ps, pe)) = &past_range {
            let gap_days = (*pe - *ps).num_days();
            if gap_days > 365 {
                tracing::info!(ticker = ticker, gap_days, "대규모 과거 데이터 수집 ({}년)", gap_days / 365);
            }
        }

        // 수집할 구간 결정 (과거 방향 우선)
        let (fetch_start, fetch_end) = if let Some((ps, pe)) = past_range {
            tracing::info!(ticker = ticker, range = format!("{} ~ {}", ps, pe), "과거 방향 증분 수집");
            (ps, pe)
        } else if let Some((fs, fe)) = future_range {
            tracing::debug!(ticker = ticker, range = format!("{} ~ {}", fs, fe), "최신 방향 증분 수집");
            (fs, fe)
        } else {
            continue;
        };

        for tf_str in &timeframes {
            let timeframe = match tf_str.as_str() {
                "1m" => Timeframe::M1,
                "5m" => Timeframe::M5,
                "15m" => Timeframe::M15,
                "30m" => Timeframe::M30,
                "1h" => Timeframe::H1,
                "1d" | "d1" => Timeframe::D1,
                "1w" | "w1" => Timeframe::W1,
                _ => continue,
            };

            let (tf_start, tf_end) = match timeframe {
                Timeframe::M1 | Timeframe::M5 | Timeframe::M15 | Timeframe::M30 | Timeframe::H1 => {
                    let max_days = 55;
                    let intraday_start = end_date - chrono::Duration::days(max_days);
                    let adjusted_start = if fetch_start > intraday_start { fetch_start } else { intraday_start };
                    (adjusted_start, fetch_end)
                }
                _ => (fetch_start, fetch_end),
            };

            let klines_result = match timeframe {
                Timeframe::D1 => {
                    if market == "KR" {
                        fetch_kr_klines(&krx_client, &yahoo_provider, ticker, tf_start, tf_end).await
                    } else {
                        yahoo_provider.get_klines_range(ticker, Timeframe::D1, tf_start, tf_end)
                            .await.map_err(|e| e.to_string())
                    }
                }
                _ => {
                    yahoo_provider.get_klines_range(ticker, timeframe, tf_start, tf_end)
                        .await.map_err(|e| e.to_string())
                }
            };

            match klines_result {
                Ok(klines) if !klines.is_empty() => {
                    stats.total_klines += klines.len();

                    if timeframe == Timeframe::D1 && klines.len() >= 40 {
                        stats.success += 1;
                        update_indicators_for_symbol(
                            pool, *symbol_info_id, ticker, market, &klines,
                            &route_state_calc, &market_regime_calc, &indicator_engine,
                        ).await;
                    }
                    tracing::info!(ticker = ticker, timeframe = tf_str, klines = klines.len(), "수집 완료");
                }
                Ok(_) => {
                    if timeframe == Timeframe::D1 { stats.empty += 1; }
                }
                Err(e) => {
                    let error_str = e.to_string();
                    if timeframe == Timeframe::D1
                        && (error_str.contains("may be delisted")
                            || error_str.contains("No data found")
                            || error_str.contains("empty data set"))
                    {
                        stats.errors += 1;
                        tracing::warn!(ticker = ticker, "상장폐지 감지 - 자동 비활성화");
                        let _ = sqlx::query(
                            "UPDATE symbol_info SET is_active = false, updated_at = NOW() WHERE id = $1"
                        )
                        .bind(symbol_info_id)
                        .execute(pool)
                        .await;
                        break;
                    } else {
                        if timeframe == Timeframe::D1 { stats.errors += 1; }
                        tracing::error!(ticker = ticker, timeframe = tf_str, error = %e, "조회 실패");
                    }
                }
            }

            // Rate limiting
            let delay = if matches!(timeframe, Timeframe::D1 | Timeframe::W1) {
                request_delay
            } else {
                std::time::Duration::from_millis(100)
            };
            tokio::time::sleep(delay).await;
        }

        // 진행률 기록 및 출력
        progress.record_completion(market, symbol_start.elapsed());
        progress.log_progress(ticker, market);

        // 체크포인트: 100개마다 갱신
        if (idx + 1) % 100 == 0 {
            let _ = checkpoint::save_checkpoint(
                pool, "ohlcv_collect", ticker, (idx + 1) as i32, CheckpointStatus::Running,
            ).await;
        }
    }

    // 체크포인트: 수집 완료
    let _ = checkpoint::save_checkpoint(
        pool, "ohlcv_collect", "", collection_count as i32, CheckpointStatus::Completed,
    ).await;

    stats.elapsed = start.elapsed();
    Ok(stats)
}

/// 개별 심볼의 지표 계산 및 DB 업데이트 (RouteState, MarketRegime, TTM Squeeze)
///
/// GlobalScore는 별도 워크플로우(global_score_sync)에서 계산합니다.
async fn update_indicators_for_symbol(
    pool: &PgPool,
    symbol_info_id: Uuid,
    ticker: &str,
    _market: &str,
    candles: &[Kline],
    route_state_calc: &RouteStateCalculator,
    market_regime_calc: &MarketRegimeCalculator,
    indicator_engine: &IndicatorEngine,
) {
    // RouteState 계산
    let route_state = match route_state_calc.calculate(candles) {
        Ok(state) => Some(format!("{:?}", state).to_uppercase()),
        Err(e) => {
            tracing::debug!(ticker = ticker, error = %e, "RouteState 계산 실패");
            None
        }
    };

    // MarketRegime 계산 (70개 이상 필요)
    let regime = if candles.len() >= 70 {
        match market_regime_calc.calculate(candles) {
            Ok(result) => {
                let regime_str = format!("{:?}", result.regime);
                Some(to_screaming_snake_case(&regime_str))
            }
            Err(e) => {
                tracing::debug!(ticker = ticker, error = %e, "MarketRegime 계산 실패");
                None
            }
        }
    } else {
        None
    };

    // TTM Squeeze 계산 (20개 이상 필요)
    let (ttm_squeeze, ttm_squeeze_cnt) = if candles.len() >= 20 {
        calculate_ttm_squeeze(indicator_engine, candles)
    } else {
        (None, None)
    };

    // symbol_fundamental DB 업데이트
    if let Err(e) = sqlx::query(
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
    .bind(route_state.as_deref())
    .bind(regime.as_deref())
    .bind(ttm_squeeze)
    .bind(ttm_squeeze_cnt)
    .execute(pool)
    .await
    {
        tracing::warn!(ticker = ticker, error = %e, "지표 DB 업데이트 실패");
    }
}

// to_screaming_snake_case, calculate_ttm_squeeze는 utils.rs로 이동됨

/// 타임프레임별 기본 수집 기간 (일 단위)
fn get_default_retention_days(timeframe: &str) -> i64 {
    match timeframe.to_lowercase().as_str() {
        "1m" | "m1" => 7,       // 1분봉: 7일
        "5m" | "m5" => 14,      // 5분봉: 14일
        "15m" | "m15" => 30,    // 15분봉: 30일
        "1h" | "h1" => 90,      // 1시간봉: 90일
        "4h" | "h4" => 180,     // 4시간봉: 180일
        "1d" | "d1" => 365 * 3, // 일봉: 3년
        "1w" | "w1" => 365 * 5, // 주봉: 5년
        _ => 365,               // 기본: 1년
    }
}

/// 날짜 범위 결정 (타임프레임 기반)
fn determine_date_range(config: &CollectorConfig, timeframe: &str) -> (NaiveDate, NaiveDate) {
    let end_date = match &config.ohlcv_collect.end_date {
        Some(date) => NaiveDate::parse_from_str(date, "%Y%m%d")
            .unwrap_or_else(|_| Utc::now().date_naive()),
        None => Utc::now().date_naive(),
    };

    let start_date = match &config.ohlcv_collect.start_date {
        Some(date) => NaiveDate::parse_from_str(date, "%Y%m%d")
            .unwrap_or_else(|_| end_date - chrono::Duration::days(get_default_retention_days(timeframe))),
        None => {
            // 타임프레임별 기본 수집 기간 적용
            let retention_days = get_default_retention_days(timeframe);
            // 최대 보존 기간 제한 (config에서 설정)
            let max_days = config.ohlcv_collect.max_retention_years as i64 * 365;
            let actual_days = retention_days.min(max_days);
            end_date - chrono::Duration::days(actual_days)
        }
    };

    (start_date, end_date)
}

// ============================================================================
// 데이터 소스 이원화 헬퍼 함수
// ============================================================================

/// KRX API 클라이언트 초기화 (credential 시스템 사용).
///
/// credential이 없으면 None 반환 (Yahoo fallback 사용).
async fn init_krx_client(pool: &PgPool) -> Option<KrxApiClient> {
    let master_key = match std::env::var("ENCRYPTION_MASTER_KEY") {
        Ok(key) => key,
        Err(_) => {
            tracing::debug!("ENCRYPTION_MASTER_KEY 없음 - KRX API 비활성화");
            return None;
        }
    };

    let encryptor = match CredentialEncryptor::new(&master_key) {
        Ok(e) => e,
        Err(e) => {
            tracing::debug!(error = %e, "암호화키 초기화 실패 - KRX API 비활성화");
            return None;
        }
    };

    match KrxApiClient::from_credential(pool, &encryptor).await {
        Ok(Some(client)) => {
            tracing::info!("KRX API 클라이언트 초기화 성공 (국내 데이터 이원화 활성화)");
            Some(client)
        }
        Ok(None) => {
            tracing::debug!("KRX credential 미등록 - Yahoo fallback 사용");
            None
        }
        Err(e) => {
            tracing::warn!(error = %e, "KRX API 클라이언트 초기화 실패 - Yahoo fallback 사용");
            None
        }
    }
}

/// 국내(KR) 시장 OHLCV 데이터 수집.
///
/// KRX API를 먼저 시도하고, 실패하거나 데이터가 없으면 Yahoo Finance로 fallback.
async fn fetch_kr_klines(
    krx_client: &Option<KrxApiClient>,
    yahoo_provider: &CachedHistoricalDataProvider,
    ticker: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> std::result::Result<Vec<Kline>, String> {
    // KRX API가 활성화된 경우 먼저 시도
    if let Some(client) = krx_client {
        let start_str = start_date.format("%Y%m%d").to_string();
        let end_str = end_date.format("%Y%m%d").to_string();

        match client.fetch_daily_ohlcv(ticker, &start_str, &end_str).await {
            Ok(krx_data) if !krx_data.is_empty() => {
                // KRX 데이터를 Kline으로 변환
                let klines: Vec<Kline> = krx_data
                    .into_iter()
                    .map(|k| Kline {
                        ticker: ticker.to_string(),
                        timeframe: Timeframe::D1,
                        open_time: k.date.and_hms_opt(0, 0, 0).unwrap().and_utc(),
                        open: k.open,
                        high: k.high,
                        low: k.low,
                        close: k.close,
                        volume: Decimal::from(k.volume),
                        close_time: k.date.and_hms_opt(23, 59, 59).unwrap().and_utc(),
                        quote_volume: k.trading_value,
                        num_trades: None,
                    })
                    .collect();

                tracing::debug!(
                    ticker = ticker,
                    source = "KRX",
                    count = klines.len(),
                    "국내 데이터 수집 성공"
                );
                return Ok(klines);
            }
            Ok(_) => {
                tracing::debug!(ticker = ticker, "KRX API 데이터 없음 - Yahoo fallback");
            }
            Err(e) => {
                tracing::debug!(
                    ticker = ticker,
                    error = %e,
                    "KRX API 실패 - Yahoo fallback"
                );
            }
        }
    }

    // Yahoo Finance fallback
    yahoo_provider
        .get_klines_range(ticker, Timeframe::D1, start_date, end_date)
        .await
        .map_err(|e| e.to_string())
}

// ============================================================================
// 증분 수집 헬퍼 함수
// ============================================================================

/// 심볼별 기존 OHLCV 데이터 범위 조회
///
/// ohlcv 테이블에서 해당 심볼의 가장 오래된/최신 캔들 날짜를 반환합니다.
/// 데이터가 없으면 (None, None)을 반환합니다.
///
/// 일괄 수집에서는 `get_all_existing_ranges()`를 사용하므로 현재 미사용이지만,
/// 단일 심볼 조회 용도로 유지합니다.
#[allow(dead_code)]
async fn get_existing_date_range(
    pool: &PgPool,
    ticker: &str,
    timeframe: &str,
) -> (Option<NaiveDate>, Option<NaiveDate>) {
    let result: Option<(Option<chrono::DateTime<Utc>>, Option<chrono::DateTime<Utc>>)> = sqlx::query_as(
        r#"
        SELECT MIN(open_time), MAX(open_time)
        FROM ohlcv
        WHERE symbol = $1 AND timeframe = $2
        "#,
    )
    .bind(ticker)
    .bind(timeframe)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    match result {
        Some((Some(min), Some(max))) => (Some(min.date_naive()), Some(max.date_naive())),
        _ => (None, None),
    }
}

/// 모든 심볼의 기존 OHLCV 데이터 범위를 일괄 조회.
///
/// ohlcv_metadata 테이블 활용 (트리거로 자동 관리, 심볼당 1행).
/// 개별 쿼리 N회 대신 단일 쿼리로 전체 범위를 가져옵니다.
async fn get_all_existing_ranges(
    pool: &PgPool,
    tickers: &[String],
    timeframe: &str,
) -> std::collections::HashMap<String, (Option<NaiveDate>, Option<NaiveDate>)> {
    let mut result_map = std::collections::HashMap::new();

    if tickers.is_empty() {
        return result_map;
    }

    let rows: Vec<(String, Option<chrono::DateTime<Utc>>, Option<chrono::DateTime<Utc>>)> =
        sqlx::query_as(
            r#"
            SELECT symbol, first_cached_time, last_cached_time
            FROM ohlcv_metadata
            WHERE symbol = ANY($1) AND timeframe = $2
            "#,
        )
        .bind(tickers)
        .bind(timeframe)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    for (symbol, first_time, last_time) in rows {
        result_map.insert(
            symbol,
            (
                first_time.map(|t| t.date_naive()),
                last_time.map(|t| t.date_naive()),
            ),
        );
    }

    result_map
}

/// 증분 수집 구간 계산
///
/// 요청 범위와 기존 데이터 범위를 비교하여 수집해야 할 구간을 반환합니다.
///
/// # 반환
/// - `past_range`: 과거 방향 누락 구간 (요청 시작일 ~ 기존 데이터 시작일 - 1일)
/// - `future_range`: 최신 방향 누락 구간 (기존 데이터 종료일 + 1일 ~ 요청 종료일)
/// - `gaps`: 중간 갭 (현재 미구현)
fn calculate_missing_ranges(
    requested_start: NaiveDate,
    requested_end: NaiveDate,
    existing_start: Option<NaiveDate>,
    existing_end: Option<NaiveDate>,
) -> (Option<(NaiveDate, NaiveDate)>, Option<(NaiveDate, NaiveDate)>) {
    match (existing_start, existing_end) {
        (None, None) => {
            // 데이터 없음 - 전체 구간 수집 필요
            (Some((requested_start, requested_end)), None)
        }
        (Some(ex_start), Some(ex_end)) => {
            let mut past_range = None;
            let mut future_range = None;

            // 1. 과거 방향 누락 (요청 시작일 < 기존 시작일)
            if requested_start < ex_start {
                past_range = Some((requested_start, ex_start - chrono::Duration::days(1)));
            }

            // 2. 최신 방향 누락 (요청 종료일 > 기존 종료일)
            if requested_end > ex_end {
                future_range = Some((ex_end + chrono::Duration::days(1), requested_end));
            }

            (past_range, future_range)
        }
        _ => (Some((requested_start, requested_end)), None),
    }
}


// ============================================================================
// KRX API 일괄 수집 헬퍼 함수
// ============================================================================



/// KRX OHLCV 배치 데이터 저장을 위한 구조체
struct KrxOhlcvRow {
    ticker: String,
    symbol_info_id: Uuid,
    open_time: chrono::DateTime<Utc>,
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    volume: i64,
}

/// KRX API에서 수집한 OHLCV 데이터를 배치로 DB에 저장.
///
/// `BATCH_SIZE`개씩 묶어서 한 번의 쿼리로 처리하여 DB 왕복을 최소화합니다.
/// 2,500건 기준: 개별 INSERT 2,500회 → 배치 5회로 감소.
async fn save_krx_ohlcv_batch(
    pool: &PgPool,
    rows: &[KrxOhlcvRow],
) -> std::result::Result<u64, sqlx::Error> {
    if rows.is_empty() {
        return Ok(0);
    }

    const BATCH_SIZE: usize = 500;
    let mut total_affected = 0u64;

    for chunk in rows.chunks(BATCH_SIZE) {
        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO ohlcv (symbol, symbol_info_id, timeframe, open_time, open, high, low, close, volume) "
        );

        query_builder.push_values(chunk, |mut b, row| {
            b.push_bind(&row.ticker)
                .push_bind(row.symbol_info_id)
                .push("'1d'")
                .push_bind(row.open_time)
                .push_bind(row.open)
                .push_bind(row.high)
                .push_bind(row.low)
                .push_bind(row.close)
                .push_bind(row.volume);
        });

        query_builder.push(
            " ON CONFLICT (symbol, timeframe, open_time) DO UPDATE SET \
             open = EXCLUDED.open, \
             high = EXCLUDED.high, \
             low = EXCLUDED.low, \
             close = EXCLUDED.close, \
             volume = EXCLUDED.volume, \
             updated_at = NOW()"
        );

        let result = query_builder.build().execute(pool).await?;
        total_affected += result.rows_affected();
    }

    Ok(total_affected)
}

/// KRX 시가총액/상장주식수 배치 업데이트를 위한 구조체
struct KrxMarketInfoRow {
    symbol_info_id: Uuid,
    market_cap: Option<Decimal>,
    shares_outstanding: Option<i64>,
}

/// KRX API에서 수집한 시가총액, 상장주식수를 배치로 업데이트.
async fn update_market_info_batch(
    pool: &PgPool,
    rows: &[KrxMarketInfoRow],
) -> std::result::Result<u64, sqlx::Error> {
    if rows.is_empty() {
        return Ok(0);
    }

    const BATCH_SIZE: usize = 500;
    let mut total_affected = 0u64;

    for chunk in rows.chunks(BATCH_SIZE) {
        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO symbol_fundamental (symbol_info_id, market_cap, shares_outstanding, fetched_at) "
        );

        query_builder.push_values(chunk, |mut b, row| {
            b.push_bind(row.symbol_info_id)
                .push_bind(row.market_cap)
                .push_bind(row.shares_outstanding)
                .push("NOW()");
        });

        query_builder.push(
            " ON CONFLICT (symbol_info_id) DO UPDATE SET \
             market_cap = COALESCE(EXCLUDED.market_cap, symbol_fundamental.market_cap), \
             shares_outstanding = COALESCE(EXCLUDED.shares_outstanding, symbol_fundamental.shares_outstanding), \
             updated_at = NOW()"
        );

        let result = query_builder.build().execute(pool).await?;
        total_affected += result.rows_affected();
    }

    Ok(total_affected)
}


