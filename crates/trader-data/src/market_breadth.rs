//! Market Breadth 계산 모듈.
//!
//! 시장별 20일 이동평균선 상회 종목 비율을 계산합니다.

use crate::error::{DataError, Result};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, info, instrument};
use trader_core::domain::MarketBreadth;

/// Market Breadth 계산기.
///
/// DB에서 종목별 최신 가격과 20일선을 조회하여 시장 온도를 계산합니다.
pub struct MarketBreadthCalculator {
    pool: PgPool,
}

impl MarketBreadthCalculator {
    /// 새로운 계산기 생성.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 전체 시장 Breadth 계산.
    ///
    /// KOSPI와 KOSDAQ을 모두 포함한 전체 시장의 20일선 상회 비율을 계산합니다.
    ///
    /// # Returns
    ///
    /// MarketBreadth 인스턴스
    ///
    /// # Errors
    ///
    /// - DB 연결 실패
    /// - 데이터 부족 (종목 수 < 10)
    #[instrument(skip(self), level = "info")]
    pub async fn calculate(&self) -> Result<MarketBreadth> {
        let (all_ratio, kospi_ratio, kosdaq_ratio) = tokio::try_join!(
            self.calculate_market_ratio(None),
            self.calculate_market_ratio(Some("KOSPI")),
            self.calculate_market_ratio(Some("KOSDAQ"))
        )?;

        info!(
            all = %all_ratio,
            kospi = %kospi_ratio,
            kosdaq = %kosdaq_ratio,
            "Market Breadth 계산 완료"
        );

        Ok(MarketBreadth::new(all_ratio, kospi_ratio, kosdaq_ratio))
    }

    /// 특정 시장의 Above_MA20 비율 계산.
    ///
    /// # Arguments
    ///
    /// * `market` - 시장 필터 (None=전체, Some("KOSPI"), Some("KOSDAQ"))
    ///
    /// # Returns
    ///
    /// 20일선 상회 종목 비율 (0.0 ~ 1.0)
    #[instrument(skip(self), level = "debug")]
    async fn calculate_market_ratio(&self, market: Option<&str>) -> Result<Decimal> {
        // 35일 전 날짜 계산 (20 영업일을 커버하기 위해 주말/휴일 고려)
        let thirty_five_days_ago = Utc::now() - Duration::days(35);

        // 시장별 필터 조건
        let market_filter = match market {
            Some("KOSPI") => "AND si.exchange = 'KOSPI'",
            Some("KOSDAQ") => "AND si.exchange = 'KOSDAQ'",
            _ => "", // 전체 시장
        };

        // 20일선 상회 종목 수 조회
        // OHLCV 테이블에서 최근 20일 데이터를 기반으로 계산
        let query = format!(
            r#"
            WITH recent_prices AS (
                -- 종목별 최근 20일 종가 데이터
                SELECT
                    o.symbol,
                    o.close,
                    ROW_NUMBER() OVER (PARTITION BY o.symbol ORDER BY o.open_time DESC) as rn
                FROM ohlcv o
                JOIN symbol_info si ON o.symbol = si.ticker
                WHERE o.timeframe = '1d'
                  AND o.open_time >= $1
                  AND si.is_active = true
                  AND si.market = 'KR'
                  AND si.symbol_type = 'STOCK'
                  {}
            ),
            ma20_calc AS (
                -- 종목별 20일 이동평균 계산
                SELECT
                    symbol,
                    AVG(close) as ma20,
                    MAX(CASE WHEN rn = 1 THEN close END) as current_price
                FROM recent_prices
                WHERE rn <= 20
                GROUP BY symbol
                HAVING COUNT(*) >= 20  -- 20일 데이터가 모두 있는 종목만
            )
            SELECT
                COUNT(*) as total_stocks,
                COUNT(CASE WHEN current_price > ma20 THEN 1 END) as above_ma20
            FROM ma20_calc
            "#,
            market_filter
        );

        debug!(market = ?market, "Market Breadth 쿼리 실행");

        let row: (i64, i64) = sqlx::query_as(&query)
            .bind(thirty_five_days_ago)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DataError::QueryError(format!("Market Breadth 계산 실패: {}", e)))?;

        let (total_stocks, above_ma20) = row;

        // 종목 수가 너무 적으면 에러
        if total_stocks < 10 {
            return Err(DataError::InvalidData(format!(
                "종목 수 부족 (market={:?}, count={}). 최소 10개 필요",
                market, total_stocks
            )));
        }

        // 비율 계산
        let ratio = if total_stocks > 0 {
            Decimal::from(above_ma20) / Decimal::from(total_stocks)
        } else {
            Decimal::from_f32_retain(0.5).unwrap() // 데이터 없으면 중립값
        };

        debug!(
            market = ?market,
            total = total_stocks,
            above_ma20 = above_ma20,
            ratio = %ratio,
            "Market Breadth 비율 계산 완료"
        );

        Ok(ratio)
    }

    /// 특정 거래소의 Above_MA20 비율 계산 (레거시 호환).
    ///
    /// # Deprecated
    ///
    /// `calculate_market_ratio` 사용을 권장합니다.
    pub async fn calculate_exchange_ratio(&self, exchange: &str) -> Result<Decimal> {
        self.calculate_market_ratio(Some(exchange)).await
    }
}

#[cfg(test)]
mod tests {


    // 통합 테스트는 실제 DB가 필요하므로 건너뜁니다.
    // 실제 테스트는 integration test로 작성하세요.

    #[test]
    fn test_module_exists() {
        // 모듈 컴파일 확인용 더미 테스트
        assert!(true);
    }
}
