//! GlobalScore Repository
//!
//! GlobalScore 계산 및 랭킹 조회를 담당합니다.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use tracing::{debug, info, warn};
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

/// Fundamental 데이터 행 타입 (복잡한 타입 alias)
type FundamentalRow = (Uuid, Option<i64>, Option<Decimal>, Option<Decimal>);
/// Fundamental 데이터 맵 타입 (복잡한 타입 alias)
type FundamentalData = (Option<i64>, Option<Decimal>, Option<Decimal>);

use trader_analytics::{
    GlobalScorer, GlobalScorerParams, IndicatorEngine, SevenFactorCalculator,
    SevenFactorInput, SevenFactorScores,
};
use trader_core::types::{MarketType, Symbol, Timeframe};
use trader_data::cache::CachedHistoricalDataProvider;
use trader_data::RedisCache;

/// 글로벌 스코어 캐시 TTL (6시간).
/// 장 마감 후 계산되며 다음 마감까지 유효.
const GLOBAL_SCORE_CACHE_TTL_SECS: u64 = 21600;

/// 7Factor 분석 캐시 TTL (2시간).
const SEVEN_FACTOR_CACHE_TTL_SECS: u64 = 7200;

// ================================================================================================
// Types
// ================================================================================================

/// GlobalScore 계산 결과 레코드 (DB)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GlobalScoreRecord {
    pub id: Uuid,
    pub symbol_info_id: Uuid,
    pub overall_score: Decimal,
    pub grade: String,
    pub confidence: Option<String>,
    pub component_scores: JsonValue,
    pub penalties: Option<JsonValue>,
    pub market: String,
    pub ticker: String,
    pub calculated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// GlobalScore 랭킹 응답용 (JOIN with symbol_info)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema, TS)]
#[ts(export, export_to = "ranking/")]
pub struct RankedSymbol {
    pub ticker: String,
    pub name: String,
    pub market: String,
    pub exchange: Option<String>,
    /// 종합 점수 (0-100) - JSON에서 숫자로 직렬화
    #[serde(with = "rust_decimal::serde::float")]
    #[ts(type = "number")]
    pub overall_score: Decimal,
    pub grade: String,
    pub confidence: Option<String>,
    #[ts(type = "Record<string, number>")]
    pub component_scores: JsonValue,
    #[ts(type = "Record<string, number> | null")]
    pub penalties: Option<JsonValue>,
    #[ts(type = "string")]
    pub calculated_at: DateTime<Utc>,
    /// RouteState (실시간 계산됨, DB 조회 시 None)
    #[sqlx(skip)]
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[ts(optional)]
    pub route_state: Option<String>,
}

/// 7Factor 응답용 타입
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "ranking/")]
pub struct SevenFactorResponse {
    pub ticker: String,
    pub name: String,
    pub market: String,
    /// 7개 정규화 팩터 (0-100)
    pub factors: SevenFactorData,
    /// 종합 점수 (0-100) - JSON에서 숫자로 직렬화
    #[serde(with = "rust_decimal::serde::float")]
    #[ts(type = "number")]
    pub composite_score: Decimal,
    /// 기존 GlobalScore 정보 - JSON에서 숫자로 직렬화
    #[serde(default, with = "rust_decimal::serde::float_option")]
    #[ts(type = "number | null")]
    pub global_score: Option<Decimal>,
    pub grade: Option<String>,
    /// 계산 시각
    #[ts(type = "string")]
    pub calculated_at: chrono::DateTime<Utc>,
}

/// 7Factor 데이터
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "ranking/")]
pub struct SevenFactorData {
    /// 모멘텀 (0-100)
    #[ts(type = "number")]
    pub norm_momentum: Decimal,
    /// 가치 (0-100)
    #[ts(type = "number")]
    pub norm_value: Decimal,
    /// 품질 (0-100)
    #[ts(type = "number")]
    pub norm_quality: Decimal,
    /// 변동성 (0-100, 낮은 변동성 = 높은 점수)
    #[ts(type = "number")]
    pub norm_volatility: Decimal,
    /// 유동성 (0-100)
    #[ts(type = "number")]
    pub norm_liquidity: Decimal,
    /// 성장성 (0-100)
    #[ts(type = "number")]
    pub norm_growth: Decimal,
    /// 시장 심리 (0-100)
    #[ts(type = "number")]
    pub norm_sentiment: Decimal,
}

impl From<SevenFactorScores> for SevenFactorData {
    fn from(scores: SevenFactorScores) -> Self {
        Self {
            norm_momentum: scores.norm_momentum,
            norm_value: scores.norm_value,
            norm_quality: scores.norm_quality,
            norm_volatility: scores.norm_volatility,
            norm_liquidity: scores.norm_liquidity,
            norm_growth: scores.norm_growth,
            norm_sentiment: scores.norm_sentiment,
        }
    }
}

/// 랭킹 필터
#[derive(Debug, Clone, Default)]
pub struct RankingFilter {
    pub market: Option<String>,
    pub grade: Option<String>,
    pub min_score: Option<Decimal>,
    pub limit: Option<i64>,
    /// 오프셋 (무한 스크롤용)
    pub offset: Option<i64>,
    /// RouteState 필터 (ATTACK, ARMED, WATCH, REST)
    pub route_state: Option<String>,
}

// ================================================================================================
// Repository
// ================================================================================================

/// GlobalScore Repository
pub struct GlobalScoreRepository;

impl GlobalScoreRepository {
    /// 모든 활성 심볼에 대해 GlobalScore 계산.
    ///
    /// # 인자
    ///
    /// * `pool` - PostgreSQL 연결 풀
    ///
    /// # 반환
    ///
    /// 처리된 종목 수
    ///
    /// # 에러
    ///
    /// DB 조회/삽입 실패 시
    pub async fn calculate_all(
        pool: &PgPool,
        data_provider: &CachedHistoricalDataProvider,
    ) -> Result<i32, sqlx::Error> {
        // 1. 활성 심볼 목록 조회
        let symbols = sqlx::query!(
            r#"
            SELECT id, ticker, name, market, exchange
            FROM symbol_info
            WHERE is_active = true
            ORDER BY ticker
            "#
        )
        .fetch_all(pool)
        .await?;

        info!("GlobalScore 계산 시작: {} 종목", symbols.len());

        // 2. Fundamental 데이터 배치 조회 (N+1 제거)
        let symbol_ids: Vec<Uuid> = symbols.iter().map(|s| s.id).collect();
        let fundamentals_rows: Vec<FundamentalRow> =
            sqlx::query_as(
                r#"
                SELECT symbol_info_id, avg_volume_10d, week_52_high, week_52_low
                FROM symbol_fundamental
                WHERE symbol_info_id = ANY($1)
                "#,
            )
            .bind(&symbol_ids)
            .fetch_all(pool)
            .await?;

        let fundamentals: HashMap<Uuid, FundamentalData> =
            fundamentals_rows
                .into_iter()
                .map(|(id, vol, high, low)| (id, (vol, high, low)))
                .collect();

        // 3. 시장별 Volume Percentile 사전 계산 (N+1 제거)
        let volume_percentiles_rows: Vec<(Uuid, f64)> = sqlx::query_as(
            r#"
            SELECT
                sf.symbol_info_id,
                PERCENT_RANK() OVER (
                    PARTITION BY si.market
                    ORDER BY sf.avg_volume_10d
                ) as percentile
            FROM symbol_fundamental sf
            JOIN symbol_info si ON sf.symbol_info_id = si.id
            WHERE sf.avg_volume_10d IS NOT NULL
              AND si.is_active = true
            "#,
        )
        .fetch_all(pool)
        .await?;

        let volume_percentiles: HashMap<Uuid, f32> = volume_percentiles_rows
            .into_iter()
            .map(|(id, pct)| (id, pct as f32))
            .collect();

        let scorer = GlobalScorer::new();
        let mut processed = 0;

        // 4. 각 심볼에 대해 GlobalScore 계산 (사전 조회된 데이터 전달)
        for sym in symbols.iter() {
            let fundamental = fundamentals.get(&sym.id).copied();
            let volume_pct = volume_percentiles.get(&sym.id).copied();

            match Self::calculate_single(
                pool,
                &scorer,
                data_provider,
                sym.id,
                &sym.ticker,
                &sym.market,
                fundamental,
                volume_pct,
            )
            .await
            {
                Ok(_) => {
                    processed += 1;
                    if processed % 100 == 0 {
                        debug!("진행률: {}/{}", processed, symbols.len());
                    }
                }
                Err(e) => {
                    warn!("GlobalScore 계산 실패 ({}): {}", sym.ticker, e);
                }
            }
        }

        info!(
            "GlobalScore 계산 완료: {}/{} 종목",
            processed,
            symbols.len()
        );

        Ok(processed)
    }

    /// 단일 심볼에 대해 GlobalScore 계산 및 저장.
    ///
    /// fundamental과 volume_percentile은 배치 조회된 데이터를 전달받습니다.
    async fn calculate_single(
        pool: &PgPool,
        scorer: &GlobalScorer,
        data_provider: &CachedHistoricalDataProvider,
        symbol_info_id: Uuid,
        ticker: &str,
        market: &str,
        fundamental: Option<(Option<i64>, Option<Decimal>, Option<Decimal>)>,
        volume_percentile: Option<f32>,
    ) -> Result<(), sqlx::Error> {
        // 1. Market 문자열을 MarketType으로 변환
        let market_type = match market {
            "KR" => MarketType::Stock,
            "US" => MarketType::Stock,
            "CRYPTO" => MarketType::Crypto,
            "FOREX" => MarketType::Forex,
            "FUTURES" => MarketType::Futures,
            _ => MarketType::Stock,
        };

        let symbol = Symbol::new(ticker, "", market_type);

        // 2. OHLCV 데이터 조회 (60일치) - 읽기 전용 (외부 API 호출 없음)
        let candles = data_provider
            .get_klines_readonly(ticker, Timeframe::D1, 60)
            .await
            .map_err(|e| sqlx::Error::Protocol(format!("OHLCV 조회 실패: {}", e)))?;

        if candles.len() < 30 {
            debug!("{}: 캔들 부족으로 스킵 ({}/60)", ticker, candles.len());
            return Ok(()); // 스킵 (데이터 없는 심볼)
        }

        // 3. 파라미터 계산
        let current_price = candles.last().unwrap().close;

        // 3-1. 기술적 지표 기반 목표가/손절가 계산
        let highs: Vec<Decimal> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<Decimal> = candles.iter().map(|c| c.low).collect();

        // 20일 최고/최저를 목표가/손절가로 사용
        let recent_high = highs.iter().rev().take(20).max().copied().unwrap_or(current_price);
        let recent_low = lows.iter().rev().take(20).min().copied().unwrap_or(current_price);

        // 52주 고저가 있으면 목표가/손절가에 반영
        let (target_price, stop_price) = if let Some((_, w52_high, w52_low)) = fundamental {
            let target = w52_high.unwrap_or(recent_high);
            let stop = w52_low.unwrap_or(recent_low);
            // 목표가는 현재가보다 높아야 함, 손절가는 현재가보다 낮아야 함
            (
                if target > current_price { Some(target) } else { Some(recent_high) },
                if stop < current_price { Some(stop) } else { Some(recent_low) },
            )
        } else {
            (Some(recent_high), Some(recent_low))
        };

        // 3-2. StructuralFeatures 계산 (타입 통합으로 변환 불필요)
        let indicator_engine = trader_analytics::IndicatorEngine::new();
        let structural_features = trader_analytics::StructuralFeaturesCalculator::from_candles(
            ticker, &candles, &indicator_engine,
        ).ok();

        // 5. GlobalScore 계산
        let params = GlobalScorerParams {
            symbol: Some(symbol.to_string()),
            market_type: Some(market_type),
            entry_price: Some(current_price), // 현재가 = 진입가
            target_price,
            stop_price,
            volume_percentile,
            structural_features,
            ..Default::default()
        };

        let result = scorer
            .calculate(&candles, params)
            .map_err(|e| sqlx::Error::Protocol(format!("GlobalScore 계산 실패: {}", e)))?;

        // 4. DB 저장 (UPSERT)
        // component_scores를 복사하여 penalties 추출
        let mut component_scores_map = result.component_scores.clone();
        let penalties_value = component_scores_map
            .remove("penalties")
            .unwrap_or(Decimal::ZERO);

        let component_scores = serde_json::to_value(&component_scores_map)
            .map_err(|e| sqlx::Error::Protocol(format!("JSON 변환 실패: {}", e)))?;

        // penalties를 JSONB로 변환 (단일 값을 객체로 감싸기)
        let penalties = serde_json::json!({ "total": penalties_value.to_string() });

        // grade는 recommendation 필드 사용
        let grade = &result.recommendation;

        // confidence는 Decimal이므로 HIGH/MEDIUM/LOW 문자열로 변환
        let confidence_str = if result.confidence >= dec!(0.8) {
            Some("HIGH".to_string())
        } else if result.confidence >= dec!(0.6) {
            Some("MEDIUM".to_string())
        } else {
            Some("LOW".to_string())
        };

        sqlx::query(
            r#"
            SELECT upsert_global_score($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(symbol_info_id)
        .bind(result.overall_score)
        .bind(grade)
        .bind(confidence_str)
        .bind(component_scores)
        .bind(penalties)
        .bind(market)
        .bind(ticker)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// 상위 랭킹 조회.
    ///
    /// # 인자
    ///
    /// * `pool` - PostgreSQL 연결 풀
    /// * `data_provider` - 캐싱된 OHLCV 데이터 제공자 (Redis 3계층 캐시)
    /// * `filter` - 필터 조건
    ///
    /// # 반환
    ///
    /// 상위 N개 종목 (overall_score DESC)
    ///
    /// # 참고
    ///
    /// route_state 필터는 Collector가 사전 계산한 symbol_fundamental.route_state를 사용합니다.
    pub async fn get_top_ranked(
        pool: &PgPool,
        _data_provider: &CachedHistoricalDataProvider,
        filter: RankingFilter,
    ) -> Result<Vec<RankedSymbol>, sqlx::Error> {
        let db_limit = filter.limit.unwrap_or(50).min(500);

        // RouteState 필터 시 symbol_fundamental JOIN 추가
        let has_route_state_filter = filter.route_state.is_some();

        let mut query_builder: QueryBuilder<Postgres> = if has_route_state_filter {
            QueryBuilder::new(
                r#"
                SELECT
                    si.ticker,
                    si.name,
                    sgs.market,
                    si.exchange,
                    sgs.overall_score,
                    sgs.grade,
                    sgs.confidence,
                    sgs.component_scores,
                    sgs.penalties,
                    sgs.calculated_at
                FROM symbol_global_score sgs
                INNER JOIN symbol_info si ON sgs.symbol_info_id = si.id
                INNER JOIN symbol_fundamental sf ON sf.symbol_info_id = si.id
                WHERE 1=1
                "#,
            )
        } else {
            QueryBuilder::new(
                r#"
                SELECT
                    si.ticker,
                    si.name,
                    sgs.market,
                    si.exchange,
                    sgs.overall_score,
                    sgs.grade,
                    sgs.confidence,
                    sgs.component_scores,
                    sgs.penalties,
                    sgs.calculated_at
                FROM symbol_global_score sgs
                INNER JOIN symbol_info si ON sgs.symbol_info_id = si.id
                WHERE 1=1
                "#,
            )
        };

        // RouteState 필터 — DB 레벨에서 처리 (Collector 사전 계산 값 사용)
        if let Some(ref target_state) = filter.route_state {
            query_builder.push(" AND sf.route_state::text = ");
            query_builder.push_bind(target_state.to_uppercase());
        }

        // 시장 필터 (KR-KOSPI 형식 지원)
        if let Some(ref market) = filter.market {
            if let Some((market_code, exchange_code)) = market.split_once('-') {
                query_builder.push(" AND sgs.market = ");
                query_builder.push_bind(market_code.to_string());
                query_builder.push(" AND si.exchange = ");
                query_builder.push_bind(exchange_code.to_string());
            } else {
                query_builder.push(" AND sgs.market = ");
                query_builder.push_bind(market.clone());
            }
        }

        if let Some(ref grade) = filter.grade {
            query_builder.push(" AND sgs.grade = ");
            query_builder.push_bind(grade);
        }

        if let Some(min_score) = filter.min_score {
            query_builder.push(" AND sgs.overall_score >= ");
            query_builder.push_bind(min_score);
        }

        // 정렬 및 제한
        query_builder.push(" ORDER BY sgs.overall_score DESC, si.ticker ASC");
        query_builder.push(" LIMIT ");
        query_builder.push_bind(db_limit);

        if let Some(offset) = filter.offset {
            query_builder.push(" OFFSET ");
            query_builder.push_bind(offset);
        }

        let mut results = query_builder
            .build_query_as::<RankedSymbol>()
            .fetch_all(pool)
            .await?;

        // RouteState 필터 시 route_state 필드 채우기
        if let Some(ref target_state) = filter.route_state {
            for symbol in results.iter_mut() {
                symbol.route_state = Some(target_state.to_uppercase());
            }
        }

        Ok(results)
    }

    /// 특정 심볼의 GlobalScore 조회.
    ///
    /// # 인자
    ///
    /// * `pool` - PostgreSQL 연결 풀
    /// * `ticker` - 티커 코드
    /// * `market` - 시장 (KR, US 등)
    ///
    /// # 반환
    ///
    /// GlobalScore 레코드 (없으면 None)
    pub async fn get_by_ticker(
        pool: &PgPool,
        ticker: &str,
        market: &str,
    ) -> Result<Option<RankedSymbol>, sqlx::Error> {
        let result = sqlx::query_as::<_, RankedSymbol>(
            r#"
            SELECT
                si.ticker,
                si.name,
                sgs.market,
                si.exchange,
                sgs.overall_score,
                sgs.grade,
                sgs.confidence,
                sgs.component_scores,
                sgs.penalties,
                sgs.calculated_at
            FROM symbol_global_score sgs
            INNER JOIN symbol_info si ON sgs.symbol_info_id = si.id
            WHERE si.ticker = $1 AND sgs.market = $2
            "#,
        )
        .bind(ticker)
        .bind(market)
        .fetch_optional(pool)
        .await?;

        Ok(result)
    }

    /// 특정 심볼의 7Factor 데이터 조회.
    ///
    /// GlobalScore + Fundamental 데이터를 조합하여 7Factor 점수를 계산합니다.
    ///
    /// # 인자
    ///
    /// * `pool` - PostgreSQL 연결 풀
    /// * `data_provider` - 캐싱된 OHLCV 데이터 제공자 (Redis 3계층 캐시)
    /// * `ticker` - 티커 코드
    /// * `market` - 시장 (KR, US 등)
    ///
    /// # 반환
    ///
    /// 7Factor 응답 (데이터 부족 시 None)
    #[allow(clippy::field_reassign_with_default)]
    pub async fn get_seven_factor(
        pool: &PgPool,
        data_provider: &CachedHistoricalDataProvider,
        ticker: &str,
        market: &str,
    ) -> Result<Option<SevenFactorResponse>, sqlx::Error> {
        // 1. GlobalScore + Fundamental 데이터 조회
        let row = sqlx::query!(
            r#"
            SELECT
                si.id as symbol_info_id,
                si.ticker,
                si.name,
                sgs.market,
                sgs.overall_score,
                sgs.grade,
                sgs.component_scores,
                sgs.calculated_at,
                -- Fundamental 데이터
                sf.per,
                sf.pbr,
                sf.psr,
                sf.roe,
                sf.roa,
                sf.operating_margin,
                sf.net_profit_margin,
                sf.revenue_growth_yoy,
                sf.earnings_growth_yoy,
                sf.week_52_high,
                sf.week_52_low,
                sf.avg_volume_10d
            FROM symbol_info si
            LEFT JOIN symbol_global_score sgs ON sgs.symbol_info_id = si.id
            LEFT JOIN symbol_fundamental sf ON sf.symbol_info_id = si.id
            WHERE si.ticker = $1 AND (sgs.market = $2 OR sgs.market IS NULL)
            "#,
            ticker,
            market
        )
        .fetch_optional(pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        // 2. OHLCV 데이터에서 기술 지표 계산 (읽기 전용 - Redis/DB에서만 조회)
        let candles = data_provider
            .get_klines_readonly(ticker, Timeframe::D1, 60)
            .await
            .map_err(|e| sqlx::Error::Protocol(format!("OHLCV 조회 실패: {}", e)))?;

        // 3. 7Factor 입력 데이터 구성
        let mut input = SevenFactorInput::default();

        // Fundamental 데이터 매핑
        input.per = row.per;
        input.pbr = row.pbr;
        input.psr = row.psr;
        input.roe = row.roe;
        input.roa = row.roa;
        input.operating_margin = row.operating_margin;
        input.net_profit_margin = row.net_profit_margin;
        input.revenue_growth_yoy = row.revenue_growth_yoy;
        input.earnings_growth_yoy = row.earnings_growth_yoy;
        input.week_52_high = row.week_52_high;
        input.week_52_low = row.week_52_low;

        // 거래량 데이터
        if let Some(vol) = row.avg_volume_10d {
            input.avg_volume_amount = Some(Decimal::from(vol));
        }

        // 기술 지표 계산 (캔들 데이터 있는 경우)
        if candles.len() >= 20 {
            let indicator = IndicatorEngine::new();

            // 종가, 고가, 저가 배열 추출
            let closes: Vec<Decimal> = candles.iter().map(|c| c.close).collect();
            let highs: Vec<Decimal> = candles.iter().map(|c| c.high).collect();
            let lows: Vec<Decimal> = candles.iter().map(|c| c.low).collect();

            // RSI - 가장 최근 값 사용
            use trader_analytics::{AtrParams, RsiParams};
            if let Ok(rsi_values) = indicator.rsi(&closes, RsiParams::default()) {
                if let Some(Some(last_rsi)) = rsi_values.last() {
                    input.rsi = Some(*last_rsi);
                }
            }

            // ATR% - 가장 최근 값 사용
            if let Ok(atr_values) = indicator.atr(&highs, &lows, &closes, AtrParams::default()) {
                if let Some(Some(last_atr)) = atr_values.last() {
                    let current_price = candles.last().map(|c| c.close).unwrap_or(Decimal::ONE);
                    if current_price > Decimal::ZERO {
                        input.atr_pct = Some(*last_atr / current_price * dec!(100));
                    }
                }
            }

            // 현재가
            if let Some(last) = candles.last() {
                input.current_price = Some(last.close);

                // 5일/20일 수익률
                if candles.len() >= 5 {
                    let price_5d_ago = candles[candles.len() - 5].close;
                    if price_5d_ago > Decimal::ZERO {
                        input.return_5d =
                            Some((last.close - price_5d_ago) / price_5d_ago * dec!(100));
                    }
                }
                if candles.len() >= 20 {
                    let price_20d_ago = candles[candles.len() - 20].close;
                    if price_20d_ago > Decimal::ZERO {
                        input.return_20d =
                            Some((last.close - price_20d_ago) / price_20d_ago * dec!(100));
                    }
                }
            }
        }

        // 4. 7Factor 계산
        let scores = SevenFactorCalculator::calculate(&input);
        let composite = scores.composite_score();

        // SevenFactorResponse 생성
        // sqlx::query! 타입 (스키마 NOT NULL 기반, non-Option):
        // - row.market: String, row.overall_score: Decimal, row.grade: String
        // - row.calculated_at: DateTime<Utc>
        // SevenFactorResponse의 global_score와 grade는 Option이므로 Some()으로 감싸기
        Ok(Some(SevenFactorResponse {
            ticker: row.ticker,
            name: row.name,
            market: row.market.clone(),
            factors: scores.into(),
            composite_score: composite,
            global_score: Some(row.overall_score),
            grade: Some(row.grade.clone()),
            calculated_at: row.calculated_at,
        }))
    }

    /// 여러 심볼의 7Factor 데이터 일괄 조회.
    pub async fn get_seven_factor_batch(
        pool: &PgPool,
        data_provider: &CachedHistoricalDataProvider,
        tickers: &[String],
        market: &str,
    ) -> Result<Vec<SevenFactorResponse>, sqlx::Error> {
        let mut results = Vec::with_capacity(tickers.len());

        for ticker in tickers {
            if let Some(factor) = Self::get_seven_factor(pool, data_provider, ticker, market).await?
            {
                results.push(factor);
            }
        }

        Ok(results)
    }

    // ==================== 캐시 래퍼 함수 ====================

    /// 상위 랭킹 심볼 조회 (Redis 캐시 적용).
    ///
    /// TTL: 6시간 (장 마감 후 계산, 다음 마감까지 유효)
    pub async fn get_top_ranked_cached(
        pool: &PgPool,
        cache: Option<&RedisCache>,
        data_provider: &CachedHistoricalDataProvider,
        filter: &RankingFilter,
    ) -> Result<Vec<RankedSymbol>, sqlx::Error> {
        // 캐시 없으면 DB 직접 조회
        let Some(cache) = cache else {
            return Self::get_top_ranked(pool, data_provider, filter.clone()).await;
        };

        // 캐시 키 생성: ranking:top:{market}:{grade}:{limit}:{offset}
        let cache_key = format!(
            "ranking:top:{}:{}:{}:{}",
            filter.market.as_deref().unwrap_or("ALL"),
            filter.grade.as_deref().unwrap_or("ALL"),
            filter.limit.unwrap_or(50),
            filter.offset.unwrap_or(0)
        );

        // 캐시에서 조회 시도
        if let Ok(Some(cached)) = cache.get::<Vec<RankedSymbol>>(&cache_key).await {
            debug!(cache_key = cache_key, "랭킹 캐시 히트");
            return Ok(cached);
        }

        // DB 조회
        let results = Self::get_top_ranked(pool, data_provider, filter.clone()).await?;

        // 결과 캐시에 저장
        if !results.is_empty() {
            if let Err(e) = cache
                .set_with_ttl(&cache_key, &results, GLOBAL_SCORE_CACHE_TTL_SECS)
                .await
            {
                warn!(error = %e, "랭킹 캐시 저장 실패");
            }
        }

        Ok(results)
    }

    /// 티커별 글로벌 스코어 조회 (Redis 캐시 적용).
    pub async fn get_by_ticker_cached(
        pool: &PgPool,
        cache: Option<&RedisCache>,
        ticker: &str,
        market: &str,
    ) -> Result<Option<RankedSymbol>, sqlx::Error> {
        // 캐시 없으면 DB 직접 조회
        let Some(cache) = cache else {
            return Self::get_by_ticker(pool, ticker, market).await;
        };

        let cache_key = format!("ranking:score:{}:{}", ticker.to_uppercase(), market);

        // 캐시에서 조회 시도
        if let Ok(Some(cached)) = cache.get::<RankedSymbol>(&cache_key).await {
            debug!(ticker = ticker, "글로벌 스코어 캐시 히트");
            return Ok(Some(cached));
        }

        // DB 조회
        let result = Self::get_by_ticker(pool, ticker, market).await?;

        // 결과 캐시에 저장
        if let Some(ref record) = result {
            if let Err(e) = cache
                .set_with_ttl(&cache_key, record, GLOBAL_SCORE_CACHE_TTL_SECS)
                .await
            {
                warn!(error = %e, "글로벌 스코어 캐시 저장 실패");
            }
        }

        Ok(result)
    }

    /// 7Factor 분석 조회 (Redis 캐시 적용).
    pub async fn get_seven_factor_cached(
        pool: &PgPool,
        cache: Option<&RedisCache>,
        data_provider: &CachedHistoricalDataProvider,
        ticker: &str,
        market: &str,
    ) -> Result<Option<SevenFactorResponse>, sqlx::Error> {
        // 캐시 없으면 DB 직접 조회
        let Some(cache) = cache else {
            return Self::get_seven_factor(pool, data_provider, ticker, market).await;
        };

        let cache_key = format!("ranking:7factor:{}:{}", ticker.to_uppercase(), market);

        // 캐시에서 조회 시도
        if let Ok(Some(cached)) = cache.get::<SevenFactorResponse>(&cache_key).await {
            debug!(ticker = ticker, "7Factor 캐시 히트");
            return Ok(Some(cached));
        }

        // DB 조회
        let result = Self::get_seven_factor(pool, data_provider, ticker, market).await?;

        // 결과 캐시에 저장
        if let Some(ref factor) = result {
            if let Err(e) = cache
                .set_with_ttl(&cache_key, factor, SEVEN_FACTOR_CACHE_TTL_SECS)
                .await
            {
                warn!(error = %e, "7Factor 캐시 저장 실패");
            }
        }

        Ok(result)
    }
}
