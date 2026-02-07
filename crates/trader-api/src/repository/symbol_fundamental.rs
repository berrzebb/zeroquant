//! 심볼 Fundamental 정보 저장소.
//!
//! symbol_info와 연계하여 펀더멘털 데이터를 관리합니다.
//! KRX 또는 Yahoo Finance에서 데이터를 조회하여 저장합니다.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tracing::debug;
use uuid::Uuid;

use trader_data::RedisCache;

/// 펀더멘털 데이터 캐시 TTL (1시간).
/// 펀더멘털 데이터는 7일마다 갱신되므로 긴 TTL 가능.
const FUNDAMENTAL_CACHE_TTL_SECS: u64 = 3600;

/// Fundamental 정보 레코드.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SymbolFundamental {
    pub id: Uuid,
    pub symbol_info_id: Uuid,

    // 시장 데이터
    pub market_cap: Option<Decimal>,
    pub shares_outstanding: Option<i64>,
    pub float_shares: Option<i64>,

    // 가격 관련
    pub week_52_high: Option<Decimal>,
    pub week_52_low: Option<Decimal>,
    pub avg_volume_10d: Option<i64>,
    pub avg_volume_3m: Option<i64>,

    // 밸류에이션
    pub per: Option<Decimal>,
    pub forward_per: Option<Decimal>,
    pub pbr: Option<Decimal>,
    pub psr: Option<Decimal>,
    pub pcr: Option<Decimal>,
    pub ev_ebitda: Option<Decimal>,

    // 주당 지표
    pub eps: Option<Decimal>,
    pub bps: Option<Decimal>,
    pub dps: Option<Decimal>,
    pub sps: Option<Decimal>,

    // 배당
    pub dividend_yield: Option<Decimal>,
    pub dividend_payout_ratio: Option<Decimal>,
    pub ex_dividend_date: Option<NaiveDate>,

    // 재무제표 요약
    pub revenue: Option<Decimal>,
    pub operating_income: Option<Decimal>,
    pub net_income: Option<Decimal>,
    pub total_assets: Option<Decimal>,
    pub total_liabilities: Option<Decimal>,
    pub total_equity: Option<Decimal>,

    // 수익성 지표
    pub roe: Option<Decimal>,
    pub roa: Option<Decimal>,
    pub operating_margin: Option<Decimal>,
    pub net_profit_margin: Option<Decimal>,
    pub gross_margin: Option<Decimal>,

    // 안정성 지표
    pub debt_ratio: Option<Decimal>,
    pub current_ratio: Option<Decimal>,
    pub quick_ratio: Option<Decimal>,
    pub interest_coverage: Option<Decimal>,

    // 성장성 지표
    pub revenue_growth_yoy: Option<Decimal>,
    pub earnings_growth_yoy: Option<Decimal>,
    pub revenue_growth_3y: Option<Decimal>,
    pub earnings_growth_3y: Option<Decimal>,

    // 메타데이터
    pub data_source: Option<String>,
    pub fiscal_year_end: Option<String>,
    pub currency: Option<String>,

    pub fetched_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 새 Fundamental 정보 삽입/업데이트용.
#[derive(Debug, Clone, Default)]
pub struct NewSymbolFundamental {
    pub symbol_info_id: Uuid,

    pub market_cap: Option<Decimal>,
    pub shares_outstanding: Option<i64>,
    pub float_shares: Option<i64>,

    pub week_52_high: Option<Decimal>,
    pub week_52_low: Option<Decimal>,
    pub avg_volume_10d: Option<i64>,
    pub avg_volume_3m: Option<i64>,

    pub per: Option<Decimal>,
    pub forward_per: Option<Decimal>,
    pub pbr: Option<Decimal>,
    pub psr: Option<Decimal>,
    pub pcr: Option<Decimal>,
    pub ev_ebitda: Option<Decimal>,

    pub eps: Option<Decimal>,
    pub bps: Option<Decimal>,
    pub dps: Option<Decimal>,
    pub sps: Option<Decimal>,

    pub dividend_yield: Option<Decimal>,
    pub dividend_payout_ratio: Option<Decimal>,
    pub ex_dividend_date: Option<NaiveDate>,

    pub revenue: Option<Decimal>,
    pub operating_income: Option<Decimal>,
    pub net_income: Option<Decimal>,
    pub total_assets: Option<Decimal>,
    pub total_liabilities: Option<Decimal>,
    pub total_equity: Option<Decimal>,

    pub roe: Option<Decimal>,
    pub roa: Option<Decimal>,
    pub operating_margin: Option<Decimal>,
    pub net_profit_margin: Option<Decimal>,
    pub gross_margin: Option<Decimal>,

    pub debt_ratio: Option<Decimal>,
    pub current_ratio: Option<Decimal>,
    pub quick_ratio: Option<Decimal>,
    pub interest_coverage: Option<Decimal>,

    pub revenue_growth_yoy: Option<Decimal>,
    pub earnings_growth_yoy: Option<Decimal>,
    pub revenue_growth_3y: Option<Decimal>,
    pub earnings_growth_3y: Option<Decimal>,

    pub data_source: Option<String>,
    pub fiscal_year_end: Option<String>,
    pub currency: Option<String>,
}

/// 심볼 + Fundamental 통합 정보 (뷰 조회용).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SymbolWithFundamental {
    pub id: Uuid,
    pub ticker: String,
    pub name: String,
    pub name_en: Option<String>,
    pub market: String,
    pub exchange: Option<String>,
    pub sector: Option<String>,
    pub yahoo_symbol: Option<String>,
    pub is_active: Option<bool>,

    // Fundamental
    pub market_cap: Option<Decimal>,
    pub per: Option<Decimal>,
    pub pbr: Option<Decimal>,
    pub eps: Option<Decimal>,
    pub bps: Option<Decimal>,
    pub dividend_yield: Option<Decimal>,
    pub roe: Option<Decimal>,
    pub roa: Option<Decimal>,
    pub operating_margin: Option<Decimal>,
    pub debt_ratio: Option<Decimal>,
    pub week_52_high: Option<Decimal>,
    pub week_52_low: Option<Decimal>,
    pub avg_volume_10d: Option<i64>,
    pub revenue: Option<Decimal>,
    pub operating_income: Option<Decimal>,
    pub net_income: Option<Decimal>,
    pub revenue_growth_yoy: Option<Decimal>,
    pub earnings_growth_yoy: Option<Decimal>,
    pub fundamental_source: Option<String>,
    pub fundamental_fetched_at: Option<DateTime<Utc>>,
    pub fundamental_updated_at: Option<DateTime<Utc>>,
}

/// Fundamental 정보 저장소.
pub struct SymbolFundamentalRepository;

impl SymbolFundamentalRepository {
    /// symbol_info_id로 Fundamental 정보 조회.
    pub async fn get_by_symbol_id(
        pool: &PgPool,
        symbol_info_id: Uuid,
    ) -> Result<Option<SymbolFundamental>, sqlx::Error> {
        sqlx::query_as::<_, SymbolFundamental>(
            "SELECT * FROM symbol_fundamental WHERE symbol_info_id = $1",
        )
        .bind(symbol_info_id)
        .fetch_optional(pool)
        .await
    }

    /// 티커로 통합 정보 조회 (뷰 사용).
    pub async fn get_with_fundamental_by_ticker(
        pool: &PgPool,
        ticker: &str,
        market: Option<&str>,
    ) -> Result<Option<SymbolWithFundamental>, sqlx::Error> {
        let mut query =
            String::from("SELECT * FROM v_symbol_with_fundamental WHERE UPPER(ticker) = UPPER($1)");

        if market.is_some() {
            query.push_str(" AND market = $2");
        }
        query.push_str(" LIMIT 1");

        if let Some(m) = market {
            sqlx::query_as::<_, SymbolWithFundamental>(&query)
                .bind(ticker)
                .bind(m)
                .fetch_optional(pool)
                .await
        } else {
            sqlx::query_as::<_, SymbolWithFundamental>(&query)
                .bind(ticker)
                .fetch_optional(pool)
                .await
        }
    }

    /// Fundamental 정보 삽입/업데이트 (upsert).
    pub async fn upsert(pool: &PgPool, data: &NewSymbolFundamental) -> Result<Uuid, sqlx::Error> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO symbol_fundamental (
                symbol_info_id,
                market_cap, shares_outstanding, float_shares,
                week_52_high, week_52_low, avg_volume_10d, avg_volume_3m,
                per, forward_per, pbr, psr, pcr, ev_ebitda,
                eps, bps, dps, sps,
                dividend_yield, dividend_payout_ratio, ex_dividend_date,
                revenue, operating_income, net_income,
                total_assets, total_liabilities, total_equity,
                roe, roa, operating_margin, net_profit_margin, gross_margin,
                debt_ratio, current_ratio, quick_ratio, interest_coverage,
                revenue_growth_yoy, earnings_growth_yoy, revenue_growth_3y, earnings_growth_3y,
                data_source, fiscal_year_end, currency, fetched_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
                $31, $32, $33, $34, $35, $36, $37, $38, $39, $40,
                $41, $42, $43, NOW()
            )
            ON CONFLICT (symbol_info_id) DO UPDATE SET
                market_cap = EXCLUDED.market_cap,
                shares_outstanding = EXCLUDED.shares_outstanding,
                float_shares = EXCLUDED.float_shares,
                week_52_high = EXCLUDED.week_52_high,
                week_52_low = EXCLUDED.week_52_low,
                avg_volume_10d = EXCLUDED.avg_volume_10d,
                avg_volume_3m = EXCLUDED.avg_volume_3m,
                per = EXCLUDED.per,
                forward_per = EXCLUDED.forward_per,
                pbr = EXCLUDED.pbr,
                psr = EXCLUDED.psr,
                pcr = EXCLUDED.pcr,
                ev_ebitda = EXCLUDED.ev_ebitda,
                eps = EXCLUDED.eps,
                bps = EXCLUDED.bps,
                dps = EXCLUDED.dps,
                sps = EXCLUDED.sps,
                dividend_yield = EXCLUDED.dividend_yield,
                dividend_payout_ratio = EXCLUDED.dividend_payout_ratio,
                ex_dividend_date = EXCLUDED.ex_dividend_date,
                revenue = EXCLUDED.revenue,
                operating_income = EXCLUDED.operating_income,
                net_income = EXCLUDED.net_income,
                total_assets = EXCLUDED.total_assets,
                total_liabilities = EXCLUDED.total_liabilities,
                total_equity = EXCLUDED.total_equity,
                roe = EXCLUDED.roe,
                roa = EXCLUDED.roa,
                operating_margin = EXCLUDED.operating_margin,
                net_profit_margin = EXCLUDED.net_profit_margin,
                gross_margin = EXCLUDED.gross_margin,
                debt_ratio = EXCLUDED.debt_ratio,
                current_ratio = EXCLUDED.current_ratio,
                quick_ratio = EXCLUDED.quick_ratio,
                interest_coverage = EXCLUDED.interest_coverage,
                revenue_growth_yoy = EXCLUDED.revenue_growth_yoy,
                earnings_growth_yoy = EXCLUDED.earnings_growth_yoy,
                revenue_growth_3y = EXCLUDED.revenue_growth_3y,
                earnings_growth_3y = EXCLUDED.earnings_growth_3y,
                data_source = EXCLUDED.data_source,
                fiscal_year_end = EXCLUDED.fiscal_year_end,
                currency = EXCLUDED.currency,
                fetched_at = NOW(),
                updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(data.symbol_info_id)
        .bind(data.market_cap)
        .bind(data.shares_outstanding)
        .bind(data.float_shares)
        .bind(data.week_52_high)
        .bind(data.week_52_low)
        .bind(data.avg_volume_10d)
        .bind(data.avg_volume_3m)
        .bind(data.per)
        .bind(data.forward_per)
        .bind(data.pbr)
        .bind(data.psr)
        .bind(data.pcr)
        .bind(data.ev_ebitda)
        .bind(data.eps)
        .bind(data.bps)
        .bind(data.dps)
        .bind(data.sps)
        .bind(data.dividend_yield)
        .bind(data.dividend_payout_ratio)
        .bind(data.ex_dividend_date)
        .bind(data.revenue)
        .bind(data.operating_income)
        .bind(data.net_income)
        .bind(data.total_assets)
        .bind(data.total_liabilities)
        .bind(data.total_equity)
        .bind(data.roe)
        .bind(data.roa)
        .bind(data.operating_margin)
        .bind(data.net_profit_margin)
        .bind(data.gross_margin)
        .bind(data.debt_ratio)
        .bind(data.current_ratio)
        .bind(data.quick_ratio)
        .bind(data.interest_coverage)
        .bind(data.revenue_growth_yoy)
        .bind(data.earnings_growth_yoy)
        .bind(data.revenue_growth_3y)
        .bind(data.earnings_growth_3y)
        .bind(&data.data_source)
        .bind(&data.fiscal_year_end)
        .bind(&data.currency)
        .fetch_one(pool)
        .await?;

        Ok(row)
    }

    /// 스크리닝: 조건에 맞는 심볼 조회.
    ///
    /// 예: PER < 10 AND ROE > 15 인 종목
    pub async fn screen(
        pool: &PgPool,
        market: Option<&str>,
        min_market_cap: Option<Decimal>,
        max_per: Option<Decimal>,
        min_roe: Option<Decimal>,
        min_dividend_yield: Option<Decimal>,
        limit: i64,
    ) -> Result<Vec<SymbolWithFundamental>, sqlx::Error> {
        let mut conditions = vec!["1=1".to_string()];
        let mut param_idx = 1;

        if market.is_some() {
            conditions.push(format!("market = ${}", param_idx));
            param_idx += 1;
        }
        if min_market_cap.is_some() {
            conditions.push(format!("market_cap >= ${}", param_idx));
            param_idx += 1;
        }
        if max_per.is_some() {
            conditions.push(format!("per <= ${}", param_idx));
            param_idx += 1;
        }
        if min_roe.is_some() {
            conditions.push(format!("roe >= ${}", param_idx));
            param_idx += 1;
        }
        if min_dividend_yield.is_some() {
            conditions.push(format!("dividend_yield >= ${}", param_idx));
            param_idx += 1;
        }

        let query = format!(
            r#"
            SELECT * FROM v_symbol_with_fundamental
            WHERE {}
            ORDER BY market_cap DESC NULLS LAST
            LIMIT ${}
            "#,
            conditions.join(" AND "),
            param_idx
        );

        // 동적 바인딩을 위해 수동 쿼리 빌드
        let mut q = sqlx::query_as::<_, SymbolWithFundamental>(&query);

        if let Some(m) = market {
            q = q.bind(m);
        }
        if let Some(v) = min_market_cap {
            q = q.bind(v);
        }
        if let Some(v) = max_per {
            q = q.bind(v);
        }
        if let Some(v) = min_roe {
            q = q.bind(v);
        }
        if let Some(v) = min_dividend_yield {
            q = q.bind(v);
        }
        q = q.bind(limit);

        q.fetch_all(pool).await
    }

    /// Fundamental 데이터가 오래된 심볼 목록 조회.
    ///
    /// fetched_at이 지정된 시간보다 오래된 경우.
    /// CRYPTO 심볼은 Yahoo Finance에서 지원하지 않으므로 제외.
    ///
    /// 반환 값: (symbol_info_id, ticker, market, yahoo_symbol)
    pub async fn get_stale_symbols(
        pool: &PgPool,
        older_than: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<(Uuid, String, String, Option<String>)>, sqlx::Error> {
        // (symbol_info_id, ticker, market, yahoo_symbol)
        // CRYPTO 심볼 제외: Yahoo Finance가 암호화폐 Fundamental 데이터를 제공하지 않음
        sqlx::query_as::<_, (Uuid, String, String, Option<String>)>(
            r#"
            SELECT si.id, si.ticker, si.market, si.yahoo_symbol
            FROM symbol_info si
            LEFT JOIN symbol_fundamental sf ON si.id = sf.symbol_info_id
            WHERE si.is_active = true
              AND si.market != 'CRYPTO'
              AND (sf.fetched_at IS NULL OR sf.fetched_at < $1)
            ORDER BY sf.fetched_at NULLS FIRST
            LIMIT $2
            "#,
        )
        .bind(older_than)
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// symbol_info_id로 삭제.
    pub async fn delete_by_symbol_id(
        pool: &PgPool,
        symbol_info_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM symbol_fundamental WHERE symbol_info_id = $1")
            .bind(symbol_info_id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// 분석 지표 업데이트 (RouteState, MarketRegime, TTM Squeeze).
    ///
    /// symbol_fundamental 레코드가 없으면 새로 생성합니다.
    pub async fn update_indicators(
        pool: &PgPool,
        update: &IndicatorUpdate,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO symbol_fundamental (symbol_info_id, route_state, regime, ttm_squeeze, ttm_squeeze_cnt, fetched_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            ON CONFLICT (symbol_info_id) DO UPDATE SET
                route_state = COALESCE(EXCLUDED.route_state, symbol_fundamental.route_state),
                regime = COALESCE(EXCLUDED.regime, symbol_fundamental.regime),
                ttm_squeeze = COALESCE(EXCLUDED.ttm_squeeze, symbol_fundamental.ttm_squeeze),
                ttm_squeeze_cnt = COALESCE(EXCLUDED.ttm_squeeze_cnt, symbol_fundamental.ttm_squeeze_cnt),
                updated_at = NOW()
            "#,
        )
        .bind(update.symbol_info_id)
        .bind(&update.route_state)
        .bind(&update.regime)
        .bind(update.ttm_squeeze)
        .bind(update.ttm_squeeze_cnt)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// 분석 지표가 오래된 심볼 목록 조회.
    ///
    /// route_state 또는 regime이 없거나 updated_at이 오래된 경우.
    /// CRYPTO 심볼 제외.
    ///
    /// 반환 값: (symbol_info_id, ticker, market, yahoo_symbol)
    pub async fn get_stale_indicator_symbols(
        pool: &PgPool,
        older_than: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<(Uuid, String, String, Option<String>)>, sqlx::Error> {
        sqlx::query_as::<_, (Uuid, String, String, Option<String>)>(
            r#"
            SELECT si.id, si.ticker, si.market, si.yahoo_symbol
            FROM symbol_info si
            LEFT JOIN symbol_fundamental sf ON si.id = sf.symbol_info_id
            WHERE si.is_active = true
              AND si.market != 'CRYPTO'
              AND (
                  sf.route_state IS NULL
                  OR sf.regime IS NULL
                  OR sf.updated_at IS NULL
                  OR sf.updated_at < $1
              )
            ORDER BY sf.updated_at NULLS FIRST
            LIMIT $2
            "#,
        )
        .bind(older_than)
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    // ==================== 캐시 래퍼 함수 ====================

    /// 티커별 심볼 + 펀더멘털 정보 조회 (Redis 캐시 적용).
    ///
    /// TTL: 1시간 (펀더멘털은 7일마다 갱신)
    pub async fn get_with_fundamental_by_ticker_cached(
        pool: &PgPool,
        cache: Option<&RedisCache>,
        ticker: &str,
        market: Option<&str>,
    ) -> Result<Option<SymbolWithFundamental>, sqlx::Error> {
        // 캐시 없으면 DB 직접 조회
        let Some(cache) = cache else {
            return Self::get_with_fundamental_by_ticker(pool, ticker, market).await;
        };

        let cache_key = format!(
            "fundamental:{}:{}",
            ticker.to_uppercase(),
            market.unwrap_or("ALL")
        );

        // 캐시에서 조회 시도
        if let Ok(Some(cached)) = cache.get::<SymbolWithFundamental>(&cache_key).await {
            debug!(ticker = ticker, "펀더멘털 캐시 히트");
            return Ok(Some(cached));
        }

        // DB 조회
        let result = Self::get_with_fundamental_by_ticker(pool, ticker, market).await?;

        // 결과 캐시에 저장
        if let Some(ref data) = result {
            if let Err(e) = cache
                .set_with_ttl(&cache_key, data, FUNDAMENTAL_CACHE_TTL_SECS)
                .await
            {
                debug!(error = %e, "펀더멘털 캐시 저장 실패");
            }
        }

        Ok(result)
    }
}

/// 분석 지표 업데이트 요청.
#[derive(Debug, Clone, Default)]
pub struct IndicatorUpdate {
    /// 심볼 ID
    pub symbol_info_id: Uuid,
    /// RouteState (Overheat, Attack, Armed, Wait, Neutral)
    pub route_state: Option<String>,
    /// MarketRegime (StrongUptrend, Correction, Sideways, BottomBounce, Downtrend)
    pub regime: Option<String>,
    /// TTM Squeeze 상태 (true = squeeze 진행 중)
    pub ttm_squeeze: Option<bool>,
    /// TTM Squeeze 연속 기간 (일)
    pub ttm_squeeze_cnt: Option<i32>,
}
