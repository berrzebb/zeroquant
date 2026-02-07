//! 종목 스크리닝 Repository
//!
//! Fundamental 데이터와 OHLCV 데이터를 조합하여
//! 다양한 조건으로 종목을 필터링합니다.

use chrono::{DateTime, Duration, Utc};
use futures::future::join_all;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, PgPool, QueryBuilder};
use tracing::debug;
use utoipa::ToSchema;
use uuid::Uuid;

// 구조적 피처 계산을 위한 import
use trader_analytics::indicators::IndicatorEngine;
use trader_analytics::StructuralFeaturesCalculator;
use trader_core::{types::Timeframe, Kline};
use trader_data::cache::CachedHistoricalDataProvider;
use trader_data::RedisCache;

/// 스크리닝 결과 캐시 TTL (2시간).
/// 일중 변동이 없으므로 장 마감까지 유효.
const SCREENING_CACHE_TTL_SECS: u64 = 7200;

/// 스크리닝 결과 레코드
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ScreeningResult {
    // 심볼 기본 정보
    pub id: Uuid,
    pub ticker: String,
    pub name: String,
    pub market: String,
    pub exchange: Option<String>,
    pub sector: Option<String>,
    pub yahoo_symbol: Option<String>,

    // Fundamental 지표
    pub market_cap: Option<Decimal>,
    pub per: Option<Decimal>,
    pub pbr: Option<Decimal>,
    pub roe: Option<Decimal>,
    pub roa: Option<Decimal>,
    pub eps: Option<Decimal>,
    pub bps: Option<Decimal>,
    pub dividend_yield: Option<Decimal>,
    pub operating_margin: Option<Decimal>,
    pub debt_ratio: Option<Decimal>,
    pub revenue_growth_yoy: Option<Decimal>,
    pub earnings_growth_yoy: Option<Decimal>,

    // 가격 정보 (OHLCV 기반)
    pub current_price: Option<Decimal>,
    pub price_change_1d: Option<Decimal>,
    pub price_change_5d: Option<Decimal>,
    pub price_change_20d: Option<Decimal>,
    pub volume_ratio: Option<Decimal>,
    pub week_52_high: Option<Decimal>,
    pub week_52_low: Option<Decimal>,
    pub distance_from_52w_high: Option<Decimal>,
    pub distance_from_52w_low: Option<Decimal>,

    // 구조적 피처 (계산 결과)
    pub low_trend: Option<f64>,
    pub vol_quality: Option<f64>,
    pub range_pos: Option<f64>,
    pub dist_ma20: Option<f64>,
    pub bb_width: Option<f64>,
    pub rsi_14: Option<f64>,
    pub breakout_score: Option<f64>,

    // MACD 지표
    pub macd: Option<f64>,
    pub macd_signal: Option<f64>,
    pub macd_histogram: Option<f64>,
    pub macd_cross: Option<String>,

    // RouteState (매매 단계)
    pub route_state: Option<String>,

    // MarketRegime (시장 레짐)
    pub regime: Option<String>,

    // Sector RS (섹터 상대강도)
    pub sector_rs: Option<Decimal>,
    pub sector_rank: Option<i32>,

    // TTM Squeeze (에너지 응축 지표)
    pub ttm_squeeze: Option<bool>,
    pub ttm_squeeze_cnt: Option<i32>,

    // TRIGGER (진입 트리거)
    pub trigger_score: Option<f64>,
    pub trigger_label: Option<String>,

    // GlobalScore (종합 점수)
    pub overall_score: Option<Decimal>,
    pub grade: Option<String>,
    pub confidence: Option<String>,
}

/// 스크리닝 필터 조건
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScreeningFilter {
    // 시장/거래소 필터
    pub market: Option<String>,
    pub exchange: Option<String>,
    pub sector: Option<String>,

    // 시가총액 필터
    pub min_market_cap: Option<Decimal>,
    pub max_market_cap: Option<Decimal>,

    // 밸류에이션 필터
    pub min_per: Option<Decimal>,
    pub max_per: Option<Decimal>,
    pub min_pbr: Option<Decimal>,
    pub max_pbr: Option<Decimal>,
    pub min_psr: Option<Decimal>,
    pub max_psr: Option<Decimal>,

    // 수익성 필터
    pub min_roe: Option<Decimal>,
    pub max_roe: Option<Decimal>,
    pub min_roa: Option<Decimal>,
    pub max_roa: Option<Decimal>,
    pub min_operating_margin: Option<Decimal>,
    pub max_operating_margin: Option<Decimal>,

    // 배당 필터
    pub min_dividend_yield: Option<Decimal>,
    pub max_dividend_yield: Option<Decimal>,

    // 안정성 필터
    pub max_debt_ratio: Option<Decimal>,
    pub min_current_ratio: Option<Decimal>,

    // 성장성 필터
    pub min_revenue_growth: Option<Decimal>,
    pub min_earnings_growth: Option<Decimal>,

    // 가격/기술적 필터
    pub min_price_change_1d: Option<Decimal>,
    pub max_price_change_1d: Option<Decimal>,
    pub min_price_change_5d: Option<Decimal>,
    pub max_price_change_5d: Option<Decimal>,
    pub min_price_change_20d: Option<Decimal>,
    pub max_price_change_20d: Option<Decimal>,

    // 거래량 필터
    pub min_volume_ratio: Option<Decimal>, // 평균 대비 거래량 배율 (예: 2.0 = 평균의 2배)
    pub min_avg_volume: Option<i64>,       // 최소 평균 거래량

    // 52주 고/저가 대비
    pub max_distance_from_52w_high: Option<Decimal>, // 52주 고가 대비 하락률 (예: 10 = 10% 이내)
    pub min_distance_from_52w_low: Option<Decimal>,  // 52주 저가 대비 상승률

    // 구조적 피처 필터
    pub min_low_trend: Option<f64>,   // Higher Low 강도 (-1.0 ~ 1.0)
    pub min_vol_quality: Option<f64>, // 거래량 품질 (-1.0 ~ 1.0)
    pub min_breakout_score: Option<f64>, // 돌파 가능성 점수 (0 ~ 100)
    pub only_alive_consolidation: Option<bool>, // "살아있는 횡보"만 필터링

    // RouteState 필터
    pub filter_route_state: Option<String>, // ATTACK, ARMED, WAIT, OVERHEAT, NEUTRAL

    // MarketRegime 필터
    pub filter_regime: Option<String>, // STRONG_UPTREND, CORRECTION, SIDEWAYS, BOTTOM_BOUNCE, DOWNTREND

    // TTM Squeeze 필터
    pub filter_ttm_squeeze: Option<bool>, // true: squeeze 상태인 종목만
    pub min_ttm_squeeze_cnt: Option<i32>, // 최소 squeeze 카운트 (에너지 응축 기간)

    // 종목 유형 필터
    pub exclude_etf: Option<bool>, // true: ETF/ETN 제외 (기본값: true)

    // 정렬 및 제한
    pub sort_by: Option<String>, // market_cap, per, pbr, roe, price_change_1d, volume_ratio
    pub sort_order: Option<String>, // asc, desc
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// 스크리닝 Repository
pub struct ScreeningRepository;

impl ScreeningRepository {
    /// 종합 스크리닝 실행
    ///
    /// Fundamental 데이터와 최근 OHLCV 데이터를 조합하여 스크리닝합니다.
    pub async fn screen(
        pool: &PgPool,
        data_provider: &CachedHistoricalDataProvider,
        filter: &ScreeningFilter,
    ) -> Result<Vec<ScreeningResult>, sqlx::Error> {
        // 기본 쿼리: Fundamental 뷰 + Materialized View (최신 가격)
        // mv_latest_prices 사용으로 DISTINCT ON 쿼리 제거 → 성능 ~10x 향상
        let mut builder: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            r#"
            SELECT
                sf.id,
                sf.ticker,
                sf.name,
                sf.market,
                sf.exchange,
                sf.sector,
                sf.yahoo_symbol,
                sf.market_cap,
                sf.per,
                sf.pbr,
                sf.roe,
                sf.roa,
                sf.eps,
                sf.bps,
                sf.dividend_yield,
                sf.operating_margin,
                sf.debt_ratio,
                sf.revenue_growth_yoy,
                sf.earnings_growth_yoy,
                lp.close as current_price,
                NULL::decimal as price_change_1d,
                NULL::decimal as price_change_5d,
                NULL::decimal as price_change_20d,
                NULL::decimal as volume_ratio,
                sf.week_52_high,
                sf.week_52_low,
                CASE WHEN sf.week_52_high > 0 AND lp.close IS NOT NULL
                    THEN ((sf.week_52_high - lp.close) / sf.week_52_high) * 100
                    ELSE NULL END as distance_from_52w_high,
                CASE WHEN sf.week_52_low > 0 AND lp.close IS NOT NULL
                    THEN ((lp.close - sf.week_52_low) / sf.week_52_low) * 100
                    ELSE NULL END as distance_from_52w_low,
                NULL::double precision as low_trend,
                NULL::double precision as vol_quality,
                NULL::double precision as range_pos,
                NULL::double precision as dist_ma20,
                NULL::double precision as bb_width,
                NULL::double precision as rsi_14,
                NULL::double precision as breakout_score,
                NULL::double precision as macd,
                NULL::double precision as macd_signal,
                NULL::double precision as macd_histogram,
                NULL::varchar as macd_cross,
                sf.route_state::varchar as route_state,
                sf.regime as regime,
                NULL::decimal as sector_rs,
                NULL::integer as sector_rank,
                sf.ttm_squeeze as ttm_squeeze,
                sf.ttm_squeeze_cnt as ttm_squeeze_cnt,
                NULL::double precision as trigger_score,
                NULL::varchar as trigger_label,
                sgs.overall_score,
                sgs.grade,
                sgs.confidence
            FROM v_symbol_with_fundamental sf
            LEFT JOIN mv_latest_prices lp ON lp.symbol = sf.ticker
            LEFT JOIN symbol_global_score sgs ON sgs.symbol_info_id = sf.id
            WHERE sf.is_active = true
            "#,
        );

        // 동적 WHERE 조건 추가
        Self::add_filter_conditions(&mut builder, filter);

        // 정렬
        let sort_by = filter.sort_by.as_deref().unwrap_or("market_cap");
        let sort_order = filter.sort_order.as_deref().unwrap_or("desc");

        builder.push(" ORDER BY ");
        match sort_by {
            "per" => builder.push("sf.per"),
            "pbr" => builder.push("sf.pbr"),
            "roe" => builder.push("sf.roe"),
            "dividend_yield" => builder.push("sf.dividend_yield"),
            "price_change_1d" => builder.push("lp.close"), // TODO: 실제 변동률로 변경
            _ => builder.push("sf.market_cap"),
        };

        if sort_order.to_lowercase() == "asc" {
            builder.push(" ASC NULLS LAST");
        } else {
            builder.push(" DESC NULLS LAST");
        }

        // LIMIT/OFFSET
        let limit = filter.limit.unwrap_or(50).min(500);
        let offset = filter.offset.unwrap_or(0);
        builder.push(" LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let query = builder.build_query_as::<ScreeningResult>();
        let results = query.fetch_all(pool).await?;

        // 구조적 피처 필터링 적용 (7단계, 공유 data_provider 사용)
        let filtered = Self::apply_structural_filter(data_provider, results, filter).await?;

        debug!("스크리닝 완료: {} 종목 반환", filtered.len());
        Ok(filtered)
    }

    /// 동적 WHERE 조건 추가
    fn add_filter_conditions(builder: &mut QueryBuilder<sqlx::Postgres>, filter: &ScreeningFilter) {
        // 시장 필터 (KR-KOSPI 형식 지원)
        if let Some(ref market) = filter.market {
            // "KR-KOSPI", "KR-KOSDAQ" 등 하이픈 구분 형식 파싱
            if let Some((market_code, exchange_code)) = market.split_once('-') {
                builder.push(" AND sf.market = ");
                builder.push_bind(market_code.to_string());
                builder.push(" AND sf.exchange = ");
                builder.push_bind(exchange_code.to_string());
            } else {
                // 단순 시장 코드 (KR, US 등)
                builder.push(" AND sf.market = ");
                builder.push_bind(market.clone());
            }
        }
        // 별도 거래소 필터 (market에 하이픈이 없을 때만 적용)
        if filter.market.as_ref().map_or(true, |m| !m.contains('-')) {
            if let Some(ref exchange) = filter.exchange {
                builder.push(" AND sf.exchange = ");
                builder.push_bind(exchange.clone());
            }
        }
        if let Some(ref sector) = filter.sector {
            builder.push(" AND sf.sector ILIKE ");
            builder.push_bind(format!("%{}%", sector));
        }

        // ETF/ETN 제외 필터 (기본값: true = ETF 제외)
        if filter.exclude_etf.unwrap_or(true) {
            builder.push(" AND (sf.symbol_type IS NULL OR sf.symbol_type = 'STOCK')");
        }

        // 시가총액 필터
        if let Some(v) = filter.min_market_cap {
            builder.push(" AND sf.market_cap >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.max_market_cap {
            builder.push(" AND sf.market_cap <= ");
            builder.push_bind(v);
        }

        // PER 필터
        if let Some(v) = filter.min_per {
            builder.push(" AND sf.per >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.max_per {
            builder.push(" AND sf.per <= ");
            builder.push_bind(v);
        }

        // PBR 필터
        if let Some(v) = filter.min_pbr {
            builder.push(" AND sf.pbr >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.max_pbr {
            builder.push(" AND sf.pbr <= ");
            builder.push_bind(v);
        }

        // ROE 필터
        if let Some(v) = filter.min_roe {
            builder.push(" AND sf.roe >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.max_roe {
            builder.push(" AND sf.roe <= ");
            builder.push_bind(v);
        }

        // ROA 필터
        if let Some(v) = filter.min_roa {
            builder.push(" AND sf.roa >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.max_roa {
            builder.push(" AND sf.roa <= ");
            builder.push_bind(v);
        }

        // 배당수익률 필터
        if let Some(v) = filter.min_dividend_yield {
            builder.push(" AND sf.dividend_yield >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.max_dividend_yield {
            builder.push(" AND sf.dividend_yield <= ");
            builder.push_bind(v);
        }

        // Operating Margin 필터
        if let Some(v) = filter.min_operating_margin {
            builder.push(" AND sf.operating_margin >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.max_operating_margin {
            builder.push(" AND sf.operating_margin <= ");
            builder.push_bind(v);
        }

        // 부채비율 필터
        if let Some(v) = filter.max_debt_ratio {
            builder.push(" AND sf.debt_ratio <= ");
            builder.push_bind(v);
        }

        // 성장성 필터
        if let Some(v) = filter.min_revenue_growth {
            builder.push(" AND sf.revenue_growth_yoy >= ");
            builder.push_bind(v);
        }
        if let Some(v) = filter.min_earnings_growth {
            builder.push(" AND sf.earnings_growth_yoy >= ");
            builder.push_bind(v);
        }

        // 52주 고저가 필터
        if let Some(v) = filter.max_distance_from_52w_high {
            builder.push(
                " AND CASE WHEN sf.week_52_high > 0 AND lp.close IS NOT NULL
                  THEN ((sf.week_52_high - lp.close) / sf.week_52_high) * 100
                  ELSE NULL END <= ",
            );
            builder.push_bind(v);
        }
        if let Some(v) = filter.min_distance_from_52w_low {
            builder.push(
                " AND CASE WHEN sf.week_52_low > 0 AND lp.close IS NOT NULL
                  THEN ((lp.close - sf.week_52_low) / sf.week_52_low) * 100
                  ELSE NULL END >= ",
            );
            builder.push_bind(v);
        }

        // RouteState 필터 (DB 캐시 값 사용)
        if let Some(ref state) = filter.filter_route_state {
            builder.push(" AND sf.route_state::text = ");
            builder.push_bind(state.clone());
        }

        // MarketRegime 필터 (DB 캐시 값 사용)
        if let Some(ref regime) = filter.filter_regime {
            builder.push(" AND sf.regime = ");
            builder.push_bind(regime.clone());
        }

        // TTM Squeeze 필터 (DB 캐시 값 사용)
        if let Some(squeeze) = filter.filter_ttm_squeeze {
            builder.push(" AND sf.ttm_squeeze = ");
            builder.push_bind(squeeze);
        }
        if let Some(min_cnt) = filter.min_ttm_squeeze_cnt {
            builder.push(" AND sf.ttm_squeeze_cnt >= ");
            builder.push_bind(min_cnt);
        }
    }

    /// 구조적 피처 계산 및 필터링 적용 (배치 병렬 처리)
    ///
    /// 모든 스크리닝 결과에 대해 기술적 지표(RSI, MACD 등)를 계산하고,
    /// 구조적 필터가 있으면 필터링도 적용합니다.
    ///
    /// **성능 최적화**: N+1 문제 해결을 위해 배치 병렬 처리 적용
    async fn apply_structural_filter(
        data_provider: &CachedHistoricalDataProvider,
        candidates: Vec<ScreeningResult>,
        filter: &ScreeningFilter,
    ) -> Result<Vec<ScreeningResult>, sqlx::Error> {
        use std::collections::HashMap;

        let has_structural_filter = filter.min_low_trend.is_some()
            || filter.min_vol_quality.is_some()
            || filter.min_breakout_score.is_some()
            || filter.only_alive_consolidation.unwrap_or(false);

        let total_count = candidates.len();
        debug!(
            "구조적 피처 계산: {} 종목 (필터 적용: {}) - 배치 병렬 처리",
            total_count, has_structural_filter
        );

        if candidates.is_empty() {
            return Ok(vec![]);
        }

        // 1단계: 모든 심볼의 캔들 데이터 병렬 조회 (N+1 → 1번의 병렬 배치)
        let symbols: Vec<String> = candidates.iter().map(|c| c.ticker.clone()).collect();
        let fetch_futures: Vec<_> = symbols
            .iter()
            .map(|symbol| {
                let symbol = symbol.clone();
                let provider = data_provider;
                async move {
                    let result = provider.get_klines_readonly(&symbol, Timeframe::D1, 50).await;
                    (symbol, result)
                }
            })
            .collect();

        // 병렬 실행
        let fetch_results = join_all(fetch_futures).await;

        // 결과를 HashMap으로 변환
        let candles_map: HashMap<String, Vec<Kline>> = fetch_results
            .into_iter()
            .filter_map(|(symbol, result)| result.ok().map(|candles| (symbol, candles)))
            .collect();

        debug!("캔들 데이터 조회 완료: {}/{} 성공", candles_map.len(), total_count);

        // 2단계: 피처 계산 및 필터링 (동기 처리, CPU 바운드)
        let indicator_engine = IndicatorEngine::new();
        let trigger_calculator = trader_analytics::TriggerCalculator::new();
        let mut filtered_results = Vec::with_capacity(candidates.len());

        for mut candidate in candidates {
            let symbol = &candidate.ticker;

            // 캔들 데이터 조회 (HashMap에서)
            let candles = match candles_map.get(symbol) {
                Some(c) if c.len() >= 40 => c,
                Some(_) => {
                    debug!("데이터 부족 ({}): 40개 미만", symbol);
                    continue;
                }
                None => {
                    debug!("캔들 조회 실패 ({})", symbol);
                    continue;
                }
            };

            // 피처 계산
            let features = match StructuralFeaturesCalculator::from_candles(symbol, candles, &indicator_engine) {
                Ok(f) => f,
                Err(e) => {
                    debug!("피처 계산 실패 ({}): {}", symbol, e);
                    continue;
                }
            };

            // 필터 조건 매칭
            let mut pass = true;

            if let Some(min_lt) = filter.min_low_trend {
                if features.low_trend.to_f64().unwrap_or(0.0) < min_lt {
                    pass = false;
                }
            }

            if let Some(min_vq) = filter.min_vol_quality {
                if features.vol_quality.to_f64().unwrap_or(0.0) < min_vq {
                    pass = false;
                }
            }

            if let Some(min_bs) = filter.min_breakout_score {
                if features.breakout_score().to_f64().unwrap_or(0.0) < min_bs {
                    pass = false;
                }
            }

            if filter.only_alive_consolidation.unwrap_or(false)
                && !features.is_alive_consolidation()
            {
                pass = false;
            }

            if !pass {
                continue;
            }

            // 피처를 결과에 추가 (Decimal → f64 변환)
            candidate.low_trend = Some(features.low_trend.to_f64().unwrap_or(0.0));
            candidate.vol_quality = Some(features.vol_quality.to_f64().unwrap_or(0.0));
            candidate.range_pos = Some(features.range_pos.to_f64().unwrap_or(0.0));
            candidate.dist_ma20 = Some(features.dist_ma20.to_f64().unwrap_or(0.0));
            candidate.bb_width = Some(features.bb_width.to_f64().unwrap_or(0.0));
            candidate.rsi_14 = Some(features.rsi.to_f64().unwrap_or(0.0));
            candidate.breakout_score = Some(features.breakout_score().to_f64().unwrap_or(0.0));

            // MACD 계산 (이미 캔들 데이터가 있으므로 추가 조회 불필요)
            if candles.len() >= 35 {
                let closes: Vec<Decimal> = candles.iter().map(|c| c.close).collect();
                let macd_params = trader_analytics::MacdParams::default();

                if let Ok(macd_results) = indicator_engine.macd(&closes, macd_params) {
                    if let Some(latest) = macd_results.last() {
                        candidate.macd = latest.macd.map(|d| d.to_f64().unwrap_or(0.0));
                        candidate.macd_signal = latest.signal.map(|d| d.to_f64().unwrap_or(0.0));
                        candidate.macd_histogram = latest.histogram.map(|d| d.to_f64().unwrap_or(0.0));

                        // 골든크로스/데드크로스 감지
                        if macd_results.len() >= 2 {
                            let prev = &macd_results[macd_results.len() - 2];
                            if let (Some(curr_macd), Some(curr_sig), Some(prev_macd), Some(prev_sig)) =
                                (latest.macd, latest.signal, prev.macd, prev.signal)
                            {
                                if prev_macd < prev_sig && curr_macd > curr_sig {
                                    candidate.macd_cross = Some("golden".to_string());
                                } else if prev_macd > prev_sig && curr_macd < curr_sig {
                                    candidate.macd_cross = Some("dead".to_string());
                                }
                            }
                        }
                    }
                }
            }

            // TRIGGER 계산 (block_on 제거 - 이미 캔들 데이터 있음)
            if let Ok(trigger) = trigger_calculator.calculate(candles) {
                candidate.trigger_score = Some(trigger.score);
                candidate.trigger_label = Some(trigger.label);
            } else {
                candidate.trigger_score = None;
                candidate.trigger_label = None;
            }

            // Sector RS는 별도 계산
            candidate.sector_rs = None;
            candidate.sector_rank = None;

            filtered_results.push(candidate);
        }

        debug!(
            "구조적 필터링 완료: {} → {} 종목 (병렬 처리)",
            total_count,
            filtered_results.len()
        );

        Ok(filtered_results)
    }

    /// 섹터별 RS (상대강도) 계산
    ///
    /// 시장 대비 초과수익으로 진짜 주도 섹터를 발굴합니다.
    /// 계산 공식: 섹터 점수 = RS * 0.6 + 단순수익 * 0.4
    ///
    /// # Arguments
    /// * `pool` - Database pool
    /// * `market` - 시장 필터 (옵션)
    /// * `days` - 계산 기간 (기본: 20일)
    ///
    /// # Returns
    /// 섹터별 RS 점수와 순위 목록
    pub async fn calculate_sector_rs(
        pool: &PgPool,
        market: Option<&str>,
        days: i32,
    ) -> Result<Vec<SectorRsResult>, sqlx::Error> {
        // MV 빠른 경로: 기본 20일 기간이면 사전계산된 MV에서 조회
        if days == 20 {
            return Self::calculate_sector_rs_from_mv(pool, market).await;
        }

        // 커스텀 기간: 원본 CTE 쿼리 사용 (폴백)
        let lookback_date = Utc::now() - Duration::days(days.into());

        // KR-KOSPI 형식 파싱
        let (market_code, exchange_code) = match market {
            Some(m) if m.contains('-') => {
                let parts: Vec<&str> = m.split('-').collect();
                (Some(parts[0].to_string()), Some(parts[1].to_string()))
            }
            Some(m) => (Some(m.to_string()), None),
            None => (None, None),
        };

        // 동적 market/exchange 조건
        let market_condition = match (&market_code, &exchange_code) {
            (Some(_), Some(_)) => "AND sf.market = $2 AND sf.exchange = $3",
            (Some(_), None) => "AND sf.market = $2",
            _ => "",
        };

        let query = format!(
            r#"
            WITH sector_prices AS (
                -- 섹터별 종목의 시작/종료 가격 계산
                SELECT
                    sf.sector,
                    sf.ticker,
                    sf.yahoo_symbol,
                    sf.market_cap,
                    first_value(o.close) OVER (
                        PARTITION BY sf.ticker
                        ORDER BY o.open_time ASC
                    ) as start_price,
                    first_value(o.close) OVER (
                        PARTITION BY sf.ticker
                        ORDER BY o.open_time DESC
                    ) as end_price
                FROM v_symbol_with_fundamental sf
                JOIN ohlcv o ON o.symbol = sf.ticker
                WHERE o.timeframe = '1d'
                  AND o.open_time >= $1
                  AND sf.sector IS NOT NULL
                  AND sf.sector != ''
                  {}
            ),
            sector_returns AS (
                -- 섹터별 종목 수익률 계산 (중복 제거)
                SELECT DISTINCT ON (sector, ticker)
                    sector,
                    ticker,
                    market_cap,
                    CASE
                        WHEN start_price > 0
                        THEN ((end_price - start_price) / start_price) * 100
                        ELSE 0
                    END as return_pct
                FROM sector_prices
            ),
            sector_avg_returns AS (
                -- 섹터별 평균 수익률 및 총 시가총액
                SELECT
                    sector,
                    COUNT(*) as symbol_count,
                    AVG(return_pct) as avg_return_pct,
                    SUM(market_cap) as total_market_cap
                FROM sector_returns
                GROUP BY sector
                HAVING COUNT(*) >= 3  -- 최소 3종목 이상
            ),
            market_avg AS (
                -- 시장 전체 평균 수익률
                SELECT AVG(avg_return_pct) as market_return
                FROM sector_avg_returns
            )
            SELECT
                s.sector,
                s.symbol_count,
                s.avg_return_pct,
                m.market_return,
                -- ROUND: PostgreSQL NUMERIC의 긴 소수점을 rust_decimal이 처리 가능한 범위로 제한
                ROUND(CASE
                    WHEN m.market_return > 0
                    THEN s.avg_return_pct / m.market_return
                    ELSE 1.0
                END, 8) as relative_strength,
                -- 종합 점수 = RS * 0.6 + 단순수익 * 0.4
                ROUND(CASE
                    WHEN m.market_return > 0
                    THEN (s.avg_return_pct / m.market_return) * 0.6 + (s.avg_return_pct / 10.0) * 0.4
                    ELSE s.avg_return_pct / 10.0
                END, 8) as composite_score,
                -- 추가 필드 (시각화 컴포넌트용)
                NULL::DECIMAL as avg_return_5d_pct,  -- collector에서 제공 예정
                s.total_market_cap
            FROM sector_avg_returns s
            CROSS JOIN market_avg m
            ORDER BY composite_score DESC
        "#,
            market_condition
        );

        // 파라미터 바인딩 (market/exchange 조건에 따라 다름)
        let results = match (&market_code, &exchange_code) {
            (Some(m), Some(e)) => {
                sqlx::query_as::<_, SectorRsResult>(&query)
                    .bind(lookback_date)
                    .bind(m)
                    .bind(e)
                    .fetch_all(pool)
                    .await?
            }
            (Some(m), None) => {
                sqlx::query_as::<_, SectorRsResult>(&query)
                    .bind(lookback_date)
                    .bind(m)
                    .fetch_all(pool)
                    .await?
            }
            _ => {
                sqlx::query_as::<_, SectorRsResult>(&query)
                    .bind(lookback_date)
                    .fetch_all(pool)
                    .await?
            }
        };

        // 순위 추가
        let ranked: Vec<SectorRsResult> = results
            .into_iter()
            .enumerate()
            .map(|(idx, mut r)| {
                r.rank = (idx + 1) as i32;
                r
            })
            .collect();

        debug!("섹터 RS 계산 완료: {} 섹터", ranked.len());
        Ok(ranked)
    }

    /// 사전계산된 Materialized View에서 섹터 RS 조회 (빠른 경로)
    ///
    /// mv_sector_rs MV는 Collector에서 주기적으로 갱신됩니다.
    /// 20일 기간 기준 사전계산 데이터를 사용하여 CTE 계산을 건너뜁니다.
    async fn calculate_sector_rs_from_mv(
        pool: &PgPool,
        market: Option<&str>,
    ) -> Result<Vec<SectorRsResult>, sqlx::Error> {
        // KR-KOSPI 형식 파싱
        let (market_code, exchange_code) = match market {
            Some(m) if m.contains('-') => {
                let parts: Vec<&str> = m.split('-').collect();
                (Some(parts[0].to_string()), Some(parts[1].to_string()))
            }
            Some(m) => (Some(m.to_string()), None),
            None => (None, None),
        };

        let results = match (&market_code, &exchange_code) {
            (Some(m), Some(e)) => {
                sqlx::query_as::<_, SectorRsResult>(
                    r#"SELECT sector, symbol_count, avg_return_pct, market_return,
                       relative_strength, composite_score,
                       NULL::DECIMAL as avg_return_5d_pct, total_market_cap
                    FROM mv_sector_rs
                    WHERE market = $1 AND exchange = $2
                    ORDER BY composite_score DESC"#,
                )
                .bind(m)
                .bind(e)
                .fetch_all(pool)
                .await?
            }
            (Some(m), None) => {
                sqlx::query_as::<_, SectorRsResult>(
                    r#"SELECT sector, symbol_count, avg_return_pct, market_return,
                       relative_strength, composite_score,
                       NULL::DECIMAL as avg_return_5d_pct, total_market_cap
                    FROM mv_sector_rs
                    WHERE market = $1
                    ORDER BY composite_score DESC"#,
                )
                .bind(m)
                .fetch_all(pool)
                .await?
            }
            _ => {
                sqlx::query_as::<_, SectorRsResult>(
                    r#"SELECT sector, symbol_count, avg_return_pct, market_return,
                       relative_strength, composite_score,
                       NULL::DECIMAL as avg_return_5d_pct, total_market_cap
                    FROM mv_sector_rs
                    ORDER BY composite_score DESC"#,
                )
                .fetch_all(pool)
                .await?
            }
        };

        // 순위 추가
        let ranked: Vec<SectorRsResult> = results
            .into_iter()
            .enumerate()
            .map(|(idx, mut r)| {
                r.rank = (idx + 1) as i32;
                r
            })
            .collect();

        debug!("섹터 RS (MV) 조회 완료: {} 섹터", ranked.len());
        Ok(ranked)
    }

    /// 스크리닝 결과에 섹터 RS 정보 추가
    ///
    /// 기존 스크리닝 결과에 섹터별 RS 점수와 순위를 추가합니다.
    pub async fn enrich_with_sector_rs(
        pool: &PgPool,
        mut results: Vec<ScreeningResult>,
        market: Option<&str>,
    ) -> Result<Vec<ScreeningResult>, sqlx::Error> {
        if results.is_empty() {
            return Ok(results);
        }

        // 섹터 RS 계산
        let sector_rs_map = Self::calculate_sector_rs(pool, market, 20)
            .await?
            .into_iter()
            .map(|r| (r.sector.clone(), (r.composite_score, r.rank)))
            .collect::<std::collections::HashMap<String, (Decimal, i32)>>();

        // 각 종목에 섹터 RS 정보 추가
        for result in &mut results {
            if let Some(ref sector) = result.sector {
                if let Some((score, rank)) = sector_rs_map.get(sector) {
                    result.sector_rs = Some(*score);
                    result.sector_rank = Some(*rank);
                }
            }
        }

        Ok(results)
    }

    /// 사전 정의된 스크리닝 프리셋 실행
    pub async fn screen_preset(
        pool: &PgPool,
        data_provider: &CachedHistoricalDataProvider,
        preset: &str,
        market: Option<&str>,
    ) -> Result<Vec<ScreeningResult>, sqlx::Error> {
        let filter = match preset {
            // 가치주: 저PER + 저PBR + 적정 ROE
            "value" => ScreeningFilter {
                market: market.map(String::from),
                max_per: Some(Decimal::from(15)),
                max_pbr: Some(Decimal::from(1)),
                min_roe: Some(Decimal::from(5)),
                limit: Some(50),
                sort_by: Some("pbr".to_string()),
                sort_order: Some("asc".to_string()),
                ..Default::default()
            },
            // 고배당주: 배당수익률 높은 종목
            "dividend" => ScreeningFilter {
                market: market.map(String::from),
                min_dividend_yield: Some(Decimal::from(3)),
                min_roe: Some(Decimal::from(5)),
                max_debt_ratio: Some(Decimal::from(100)),
                limit: Some(50),
                sort_by: Some("dividend_yield".to_string()),
                sort_order: Some("desc".to_string()),
                ..Default::default()
            },
            // 성장주: 높은 매출/이익 성장률
            "growth" => ScreeningFilter {
                market: market.map(String::from),
                min_revenue_growth: Some(Decimal::from(20)),
                min_earnings_growth: Some(Decimal::from(15)),
                min_roe: Some(Decimal::from(10)),
                limit: Some(50),
                sort_by: Some("revenue_growth_yoy".to_string()),
                sort_order: Some("desc".to_string()),
                ..Default::default()
            },
            // 스노우볼: 저PBR + 고배당 + 안정성
            "snowball" => ScreeningFilter {
                market: market.map(String::from),
                max_pbr: Some(Decimal::from(1)),
                min_dividend_yield: Some(Decimal::from(3)),
                max_debt_ratio: Some(Decimal::from(80)),
                min_roe: Some(Decimal::from(8)),
                limit: Some(30),
                sort_by: Some("dividend_yield".to_string()),
                sort_order: Some("desc".to_string()),
                ..Default::default()
            },
            // 대형주: 시가총액 상위
            "large_cap" => ScreeningFilter {
                market: market.map(String::from),
                min_market_cap: Some(Decimal::from(10_000_000_000_000i64)), // 10조 이상
                limit: Some(50),
                sort_by: Some("market_cap".to_string()),
                sort_order: Some("desc".to_string()),
                ..Default::default()
            },
            // 52주 신저가 근접 (바닥 매수 전략)
            "near_52w_low" => ScreeningFilter {
                market: market.map(String::from),
                min_distance_from_52w_low: Some(Decimal::from(0)),
                max_distance_from_52w_high: Some(Decimal::from(50)), // 고가 대비 50% 이상 하락
                min_roe: Some(Decimal::from(5)),                     // 기본 수익성 보장
                limit: Some(50),
                sort_by: Some("pbr".to_string()),
                sort_order: Some("asc".to_string()),
                ..Default::default()
            },
            // 전체: 필터 없이 모든 종목 조회 (basic, all, 또는 알 수 없는 값)
            _ => ScreeningFilter {
                market: market.map(String::from),
                limit: Some(100),
                ..Default::default()
            },
        };

        Self::screen(pool, data_provider, &filter).await
    }

    /// 가격 변동률 기반 모멘텀 스크리닝
    ///
    /// OHLCV 데이터를 직접 분석하여 급등주, 급락주 등을 찾습니다.
    pub async fn screen_momentum(
        pool: &PgPool,
        market: Option<&str>,
        days: i32,
        min_change_pct: Decimal,
        min_volume_ratio: Option<Decimal>,
        limit: i32,
    ) -> Result<Vec<MomentumScreenResult>, sqlx::Error> {
        let lookback_date = Utc::now() - Duration::days(days.into());

        // SQL Injection 방지: 파라미터화된 쿼리 사용
        // $3 = market (NULL이면 필터 무시)
        // $4 = min_volume_ratio (NULL이면 필터 무시)
        let results = sqlx::query_as::<_, MomentumScreenResult>(
            r#"
            WITH start_prices AS (
                -- 기간 시작 시점의 가격 (DISTINCT ON으로 심볼별 첫 번째 레코드)
                SELECT DISTINCT ON (symbol)
                    symbol,
                    close as start_price
                FROM ohlcv
                WHERE timeframe = '1d'
                  AND open_time >= $1
                ORDER BY symbol, open_time ASC
            ),
            end_prices AS (
                -- 기간 종료 시점의 가격 (심볼별 마지막 레코드)
                SELECT DISTINCT ON (symbol)
                    symbol,
                    close as end_price,
                    volume as current_volume
                FROM ohlcv
                WHERE timeframe = '1d'
                  AND open_time >= $1
                ORDER BY symbol, open_time DESC
            ),
            avg_volumes AS (
                -- 기간 내 평균 거래량
                SELECT
                    symbol,
                    AVG(volume) as avg_volume
                FROM ohlcv
                WHERE timeframe = '1d'
                  AND open_time >= $1
                GROUP BY symbol
            ),
            momentum AS (
                SELECT
                    sp.symbol,
                    sp.start_price,
                    ep.end_price,
                    CASE WHEN sp.start_price > 0
                        THEN ((ep.end_price - sp.start_price) / sp.start_price) * 100
                        ELSE 0 END as change_pct,
                    COALESCE(av.avg_volume, 0) as avg_volume,
                    ep.current_volume,
                    CASE WHEN av.avg_volume > 0
                        THEN ep.current_volume / av.avg_volume
                        ELSE 0 END as volume_ratio
                FROM start_prices sp
                JOIN end_prices ep ON ep.symbol = sp.symbol
                LEFT JOIN avg_volumes av ON av.symbol = sp.symbol
            )
            SELECT
                m.symbol,
                COALESCE(si.name, m.symbol) as name,
                COALESCE(si.market, 'UNKNOWN') as market,
                si.exchange,
                m.start_price,
                m.end_price,
                m.change_pct,
                m.avg_volume,
                m.current_volume,
                m.volume_ratio
            FROM momentum m
            LEFT JOIN symbol_info si ON (si.yahoo_symbol = m.symbol OR si.ticker = m.symbol)
            WHERE (si.is_active = true OR si.id IS NULL)
              AND m.change_pct >= $2
              AND ($3::text IS NULL OR si.market = $3)
              AND ($4::numeric IS NULL OR m.volume_ratio >= $4)
            ORDER BY m.change_pct DESC
            LIMIT $5
            "#,
        )
        .bind(lookback_date)
        .bind(min_change_pct)
        .bind(market)
        .bind(min_volume_ratio)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        debug!(
            "모멘텀 스크리닝 완료: {}일간 {}% 이상 상승, {} 종목",
            days,
            min_change_pct,
            results.len()
        );
        Ok(results)
    }

    /// 사용 가능한 프리셋 목록 반환
    pub fn available_presets() -> Vec<ScreeningPreset> {
        vec![
            ScreeningPreset {
                id: "basic".to_string(),
                name: "전체".to_string(),
                description: "필터 없이 모든 종목 조회".to_string(),
            },
            ScreeningPreset {
                id: "value".to_string(),
                name: "가치주".to_string(),
                description: "저PER, 저PBR, 적정 ROE를 가진 저평가 종목".to_string(),
            },
            ScreeningPreset {
                id: "dividend".to_string(),
                name: "고배당주".to_string(),
                description: "배당수익률 3% 이상, 안정적인 수익성".to_string(),
            },
            ScreeningPreset {
                id: "growth".to_string(),
                name: "성장주".to_string(),
                description: "매출/이익 20% 이상 성장, 높은 ROE".to_string(),
            },
            ScreeningPreset {
                id: "snowball".to_string(),
                name: "스노우볼".to_string(),
                description: "저PBR + 고배당 + 낮은 부채비율의 안정 성장주".to_string(),
            },
            ScreeningPreset {
                id: "large_cap".to_string(),
                name: "대형주".to_string(),
                description: "시가총액 10조원 이상 우량 대형주".to_string(),
            },
            ScreeningPreset {
                id: "near_52w_low".to_string(),
                name: "52주 신저가 근접".to_string(),
                description: "52주 저가 근처에서 거래되는 수익성 있는 종목".to_string(),
            },
        ]
    }

    // ==================== 캐시 래퍼 함수 ====================

    /// 프리셋 스크리닝 결과 조회 (Redis 캐시 적용).
    ///
    /// TTL: 2시간 (일중 변동 없음)
    pub async fn screen_preset_cached(
        pool: &PgPool,
        cache: Option<&RedisCache>,
        data_provider: &CachedHistoricalDataProvider,
        preset: &str,
        market: Option<&str>,
    ) -> Result<Vec<ScreeningResult>, sqlx::Error> {
        // 캐시 없으면 DB 직접 조회
        let Some(cache) = cache else {
            return Self::screen_preset(pool, data_provider, preset, market).await;
        };

        // 캐시 키 생성: screening:preset:{preset}:{market}
        let cache_key = format!(
            "screening:preset:{}:{}",
            preset,
            market.unwrap_or("ALL")
        );

        // 캐시에서 조회 시도
        if let Ok(Some(cached)) = cache.get::<Vec<ScreeningResult>>(&cache_key).await {
            debug!(preset = preset, "프리셋 스크리닝 캐시 히트");
            return Ok(cached);
        }

        // DB 조회
        let results = Self::screen_preset(pool, data_provider, preset, market).await?;

        // 결과 캐시에 저장
        if !results.is_empty() {
            if let Err(e) = cache
                .set_with_ttl(&cache_key, &results, SCREENING_CACHE_TTL_SECS)
                .await
            {
                debug!(error = %e, "스크리닝 캐시 저장 실패");
            }
        }

        Ok(results)
    }

    /// 모멘텀 스크리닝 결과 조회 (Redis 캐시 적용).
    pub async fn screen_momentum_cached(
        pool: &PgPool,
        cache: Option<&RedisCache>,
        market: Option<&str>,
        days: i32,
        min_change_pct: Decimal,
        min_volume_ratio: Option<Decimal>,
        limit: i32,
    ) -> Result<Vec<MomentumScreenResult>, sqlx::Error> {
        // 캐시 없으면 DB 직접 조회
        let Some(cache) = cache else {
            return Self::screen_momentum(pool, market, days, min_change_pct, min_volume_ratio, limit)
                .await;
        };

        // 캐시 키 생성
        let cache_key = format!(
            "screening:momentum:{}:{}:{}:{}:{}",
            market.unwrap_or("ALL"),
            days,
            min_change_pct,
            min_volume_ratio.map(|d| d.to_string()).unwrap_or_default(),
            limit
        );

        // 캐시에서 조회 시도
        if let Ok(Some(cached)) = cache.get::<Vec<MomentumScreenResult>>(&cache_key).await {
            debug!(days = days, "모멘텀 스크리닝 캐시 히트");
            return Ok(cached);
        }

        // DB 조회
        let results =
            Self::screen_momentum(pool, market, days, min_change_pct, min_volume_ratio, limit)
                .await?;

        // 결과 캐시에 저장
        if !results.is_empty() {
            if let Err(e) = cache
                .set_with_ttl(&cache_key, &results, SCREENING_CACHE_TTL_SECS)
                .await
            {
                debug!(error = %e, "모멘텀 스크리닝 캐시 저장 실패");
            }
        }

        Ok(results)
    }
}

/// 모멘텀 스크리닝 결과
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MomentumScreenResult {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub exchange: Option<String>,
    pub start_price: Decimal,
    pub end_price: Decimal,
    pub change_pct: Decimal,
    pub avg_volume: Decimal,
    pub current_volume: Decimal,
    pub volume_ratio: Decimal,
}

/// 섹터 상대강도 결과
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SectorRsResult {
    /// 섹터명
    pub sector: String,
    /// 섹터 내 종목 수
    pub symbol_count: i64,
    /// 섹터 평균 수익률 (%)
    pub avg_return_pct: Decimal,
    /// 시장 평균 수익률 (%)
    pub market_return: Decimal,
    /// 상대강도 (RS = 섹터수익률 / 시장수익률)
    pub relative_strength: Decimal,
    /// 종합 점수 (RS * 0.6 + 단순수익 * 0.4)
    pub composite_score: Decimal,
    /// 순위
    #[sqlx(default)]
    pub rank: i32,
    /// 5일 평균 수익률 (%) - SectorMomentumBar 용
    #[sqlx(default)]
    pub avg_return_5d_pct: Option<Decimal>,
    /// 섹터 총 시가총액 - SectorTreemap 용
    #[sqlx(default)]
    pub total_market_cap: Option<Decimal>,
}

/// 스크리닝 프리셋 정보 (레거시 - 하드코딩 프리셋용)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScreeningPreset {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// DB에 저장된 스크리닝 프리셋 레코드
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ScreeningPresetRecord {
    pub id: Uuid,
    pub name: String,
    #[sqlx(default)]
    pub description: Option<String>,
    pub filters: Value,
    #[sqlx(default)]
    pub is_default: Option<bool>,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 새 프리셋 생성 요청
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreatePresetRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub filters: Value,
}

impl ScreeningRepository {
    /// 모든 프리셋 조회 (DB + 하드코딩)
    pub async fn get_all_presets(pool: &PgPool) -> Result<Vec<ScreeningPresetRecord>, sqlx::Error> {
        let records = sqlx::query_as::<_, ScreeningPresetRecord>(
            r#"
            SELECT * FROM screening_preset
            ORDER BY sort_order, name
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 프리셋 저장
    pub async fn save_preset(
        pool: &PgPool,
        request: CreatePresetRequest,
    ) -> Result<ScreeningPresetRecord, sqlx::Error> {
        // 현재 최대 sort_order 조회
        let max_order: Option<i32> =
            sqlx::query_scalar("SELECT MAX(sort_order) FROM screening_preset")
                .fetch_one(pool)
                .await?;

        let next_order = max_order.unwrap_or(-1) + 1;

        let record = sqlx::query_as::<_, ScreeningPresetRecord>(
            r#"
            INSERT INTO screening_preset (name, description, filters, is_default, sort_order)
            VALUES ($1, $2, $3, false, $4)
            RETURNING *
            "#,
        )
        .bind(&request.name)
        .bind(&request.description)
        .bind(&request.filters)
        .bind(next_order)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    /// 프리셋 삭제 (기본 프리셋은 삭제 불가)
    pub async fn delete_preset(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        // 기본 프리셋 여부 확인
        let is_default: Option<bool> =
            sqlx::query_scalar("SELECT is_default FROM screening_preset WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?
                .flatten();

        if is_default == Some(true) {
            return Ok(false); // 기본 프리셋은 삭제 불가
        }

        let result =
            sqlx::query("DELETE FROM screening_preset WHERE id = $1 AND is_default = false")
                .bind(id)
                .execute(pool)
                .await?;

        Ok(result.rows_affected() > 0)
    }

    /// 이름으로 프리셋 조회
    pub async fn get_preset_by_name(
        pool: &PgPool,
        name: &str,
    ) -> Result<Option<ScreeningPresetRecord>, sqlx::Error> {
        let record = sqlx::query_as::<_, ScreeningPresetRecord>(
            "SELECT * FROM screening_preset WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }
}

// =====================================================
// Materialized View 관리
// =====================================================

/// 최신 가격 Materialized View 갱신.
///
/// 새 가격 데이터가 입력된 후 호출하여 스크리닝 성능을 유지합니다.
/// CONCURRENTLY 옵션으로 읽기 차단 없이 갱신됩니다.
///
/// # 호출 시점
/// - 트레이딩 시간 종료 후
/// - 일봉 데이터 배치 입력 후
/// - 수동 갱신 요청 시
pub async fn refresh_latest_prices(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY mv_latest_prices")
        .execute(pool)
        .await?;

    debug!("mv_latest_prices 갱신 완료");
    Ok(())
}

/// Materialized View 존재 여부 확인.
pub async fn check_latest_prices_view_exists(pool: &PgPool) -> Result<bool, sqlx::Error> {
    let result: Option<(i32,)> =
        sqlx::query_as("SELECT 1 FROM pg_matviews WHERE matviewname = 'mv_latest_prices'")
            .fetch_optional(pool)
            .await?;

    Ok(result.is_some())
}
