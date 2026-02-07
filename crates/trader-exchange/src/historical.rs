//! 거래소 중립적 과거 데이터 제공자.
//!
//! 다양한 거래소에서 과거 캔들(OHLCV) 데이터를 조회하는 통합 인터페이스를 제공합니다.
//!
//! # 사용 예제
//!
//! ```rust,ignore
//! use trader_exchange::historical::{HistoricalDataProvider, UnifiedHistoricalProvider};
//! use trader_core::Timeframe;
//!
//! let provider = UnifiedHistoricalProvider::new(client);
//! let klines = provider.get_klines("005930", Timeframe::D1, 100).await?;
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use tracing::{debug, info, warn};

use trader_core::{Kline, Symbol, Timeframe};

use crate::connector::kis::{KisClient, KrMinuteOhlcv, KrOhlcv, UsOhlcv};
use crate::ExchangeError;

/// 거래소 중립적 과거 데이터 제공자 trait.
#[async_trait]
pub trait HistoricalDataProvider: Send + Sync {
    /// 캔들스틱 데이터 조회.
    ///
    /// # 인자
    /// * `symbol` - 심볼 (예: "005930", "AAPL", "SPY")
    /// * `timeframe` - 타임프레임
    /// * `limit` - 최대 데이터 개수
    async fn get_klines(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>, ExchangeError>;
}

/// KIS API를 사용하는 통합 과거 데이터 제공자.
///
/// 심볼 형식을 자동으로 분석하여 국내/해외 시장을 구분합니다:
/// - 6자리 숫자 (예: "005930"): 국내 주식
/// - 영문 (예: "AAPL", "SPY"): 해외 주식
pub struct UnifiedHistoricalProvider {
    client: Arc<KisClient>,
}

impl UnifiedHistoricalProvider {
    /// 새로운 통합 과거 데이터 제공자 생성.
    ///
    /// Arc로 래핑된 통합 KisClient를 받아서 사용합니다.
    pub fn new(client: Arc<KisClient>) -> Self {
        Self { client }
    }

    /// 심볼이 국내 주식인지 확인.
    pub fn is_korean_symbol(symbol: &str) -> bool {
        // 6자리 숫자인 경우 국내 주식
        symbol.len() == 6 && symbol.chars().all(|c| c.is_ascii_digit())
    }

    /// 타임프레임을 KIS API 기간 코드로 변환.
    fn timeframe_to_period(timeframe: Timeframe) -> &'static str {
        match timeframe {
            Timeframe::D1 => "D",
            Timeframe::W1 => "W",
            Timeframe::MN1 => "M",
            _ => "D", // 기본값: 일봉
        }
    }

    /// 타임프레임을 분 단위로 변환 (분봉 전용).
    fn timeframe_to_minutes(timeframe: Timeframe) -> u32 {
        match timeframe {
            Timeframe::M1 => 1,
            Timeframe::M3 => 3,
            Timeframe::M5 => 5,
            Timeframe::M15 => 15,
            Timeframe::M30 => 30,
            Timeframe::H1 => 60,
            Timeframe::H2 => 120,
            Timeframe::H4 => 240,
            Timeframe::H6 => 360,
            Timeframe::H8 => 480,
            Timeframe::H12 => 720,
            _ => 1, // 일/주/월봉은 분봉 API를 사용하지 않음
        }
    }

    /// 분봉이 필요한 타임프레임인지 확인.
    fn needs_minute_data(timeframe: Timeframe) -> bool {
        matches!(
            timeframe,
            Timeframe::M1
                | Timeframe::M3
                | Timeframe::M5
                | Timeframe::M15
                | Timeframe::M30
                | Timeframe::H1
                | Timeframe::H2
                | Timeframe::H4
                | Timeframe::H6
                | Timeframe::H8
                | Timeframe::H12
        )
    }

    /// 타임프레임에 따른 캔들 지속 시간 계산.
    fn timeframe_duration(timeframe: Timeframe) -> Duration {
        match timeframe {
            Timeframe::M1 => Duration::minutes(1),
            Timeframe::M3 => Duration::minutes(3),
            Timeframe::M5 => Duration::minutes(5),
            Timeframe::M15 => Duration::minutes(15),
            Timeframe::M30 => Duration::minutes(30),
            Timeframe::H1 => Duration::hours(1),
            Timeframe::H2 => Duration::hours(2),
            Timeframe::H4 => Duration::hours(4),
            Timeframe::H6 => Duration::hours(6),
            Timeframe::H8 => Duration::hours(8),
            Timeframe::H12 => Duration::hours(12),
            Timeframe::D1 => Duration::days(1),
            Timeframe::D3 => Duration::days(3),
            Timeframe::W1 => Duration::weeks(1),
            Timeframe::MN1 => Duration::days(30),
        }
    }

    /// 국내 주식 캔들 데이터 조회.
    async fn get_kr_klines(
        &self,
        stock_code: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>, ExchangeError> {
        let symbol = Symbol::stock(stock_code, "KRW");

        if Self::needs_minute_data(timeframe) {
            // 분봉 조회
            let minutes = Self::timeframe_to_minutes(timeframe);
            let minute_data = self.client.kr().get_minute_chart(stock_code, minutes).await?;

            let klines: Vec<Kline> = minute_data
                .into_iter()
                .take(limit)
                .map(|m| self.minute_to_kline(&symbol, timeframe, m))
                .collect();

            Ok(klines)
        } else {
            // 일/주/월봉 조회
            let period = Self::timeframe_to_period(timeframe);
            let end_date = chrono::Utc::now().format("%Y%m%d").to_string();

            // 과거 데이터 범위 계산 (limit에 따라)
            let days_back = match timeframe {
                Timeframe::D1 => limit as i64,
                Timeframe::W1 => limit as i64 * 7,
                Timeframe::MN1 => limit as i64 * 30,
                _ => limit as i64 * 365, // 년봉
            };
            let start_date = (chrono::Utc::now() - Duration::days(days_back))
                .format("%Y%m%d")
                .to_string();

            let daily_data = self
                .client
                .kr()
                .get_daily_price(stock_code, period, &start_date, &end_date, true)
                .await?;

            let klines: Vec<Kline> = daily_data
                .into_iter()
                .take(limit)
                .map(|d| self.daily_kr_to_kline(&symbol, timeframe, d))
                .collect();

            Ok(klines)
        }
    }

    /// 해외 주식 캔들 데이터 조회.
    async fn get_us_klines(
        &self,
        ticker: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>, ExchangeError> {
        let symbol = Symbol::stock(ticker, "USD");

        // 해외 주식은 현재 일/주/월봉만 지원
        // 분봉이 필요한 경우 일봉으로 대체 (재귀 대신 직접 처리)
        let effective_timeframe = if Self::needs_minute_data(timeframe) {
            warn!(
                "US minute data not supported for {}, falling back to daily",
                ticker
            );
            Timeframe::D1
        } else {
            timeframe
        };

        let period = Self::timeframe_to_period(effective_timeframe);
        let end_date = chrono::Utc::now().format("%Y%m%d").to_string();

        // 과거 데이터 범위 계산
        let days_back = match effective_timeframe {
            Timeframe::D1 => limit as i64,
            Timeframe::W1 => limit as i64 * 7,
            Timeframe::MN1 => limit as i64 * 30,
            _ => limit as i64 * 365,
        };
        let start_date = (chrono::Utc::now() - Duration::days(days_back))
            .format("%Y%m%d")
            .to_string();

        let daily_data = self
            .client
            .us()
            .get_daily_price(ticker, period, &start_date, &end_date, None)
            .await?;

        let klines: Vec<Kline> = daily_data
            .into_iter()
            .take(limit)
            .map(|d| self.daily_us_to_kline(&symbol, effective_timeframe, d))
            .collect();

        Ok(klines)
    }

    /// 국내 분봉 데이터를 Kline으로 변환.
    fn minute_to_kline(&self, symbol: &Symbol, timeframe: Timeframe, data: KrMinuteOhlcv) -> Kline {
        // HHMMSS 형식에서 시간 파싱
        let today = chrono::Utc::now().date_naive();
        let time_str = format!(
            "{}:{}:{}",
            &data.time[0..2],
            &data.time[2..4],
            &data.time[4..6]
        );
        let time = chrono::NaiveTime::parse_from_str(&time_str, "%H:%M:%S")
            .unwrap_or_else(|_| chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());

        // KST를 UTC로 변환 (-9시간)
        let datetime = today.and_time(time);
        let open_time =
            DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc) - Duration::hours(9);
        let close_time = open_time + Self::timeframe_duration(timeframe);

        Kline {
            ticker: symbol.to_string(),
            timeframe,
            open_time,
            open: data.open,
            high: data.high,
            low: data.low,
            close: data.close,
            volume: data.volume,
            close_time,
            quote_volume: None,
            num_trades: None,
        }
    }

    /// 국내 일봉 데이터를 Kline으로 변환.
    fn daily_kr_to_kline(&self, symbol: &Symbol, timeframe: Timeframe, data: KrOhlcv) -> Kline {
        // YYYYMMDD 형식에서 날짜 파싱
        let date = NaiveDate::parse_from_str(&data.date, "%Y%m%d")
            .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
        let datetime = date.and_hms_opt(0, 0, 0).unwrap();
        let open_time = DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc);
        let close_time = open_time + Self::timeframe_duration(timeframe);

        Kline {
            ticker: symbol.to_string(),
            timeframe,
            open_time,
            open: data.open,
            high: data.high,
            low: data.low,
            close: data.close,
            volume: data.volume,
            close_time,
            quote_volume: Some(data.trading_value),
            num_trades: None,
        }
    }

    /// 해외 일봉 데이터를 Kline으로 변환.
    fn daily_us_to_kline(&self, symbol: &Symbol, timeframe: Timeframe, data: UsOhlcv) -> Kline {
        // YYYYMMDD 형식에서 날짜 파싱
        let date = NaiveDate::parse_from_str(&data.date, "%Y%m%d")
            .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
        let datetime = date.and_hms_opt(0, 0, 0).unwrap();
        let open_time = DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc);
        let close_time = open_time + Self::timeframe_duration(timeframe);

        Kline {
            ticker: symbol.to_string(),
            timeframe,
            open_time,
            open: data.open,
            high: data.high,
            low: data.low,
            close: data.close,
            volume: data.volume,
            close_time,
            quote_volume: None,
            num_trades: None,
        }
    }
}

#[async_trait]
impl HistoricalDataProvider for UnifiedHistoricalProvider {
    async fn get_klines(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>, ExchangeError> {
        info!(
            "Fetching {} klines for {} (limit: {})",
            timeframe, symbol, limit
        );

        if Self::is_korean_symbol(symbol) {
            debug!("Symbol {} detected as Korean stock", symbol);
            self.get_kr_klines(symbol, timeframe, limit).await
        } else {
            debug!("Symbol {} detected as US stock", symbol);
            self.get_us_klines(symbol, timeframe, limit).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_korean_symbol() {
        assert!(UnifiedHistoricalProvider::is_korean_symbol("005930"));
        assert!(UnifiedHistoricalProvider::is_korean_symbol("000660"));
        assert!(!UnifiedHistoricalProvider::is_korean_symbol("AAPL"));
        assert!(!UnifiedHistoricalProvider::is_korean_symbol("SPY"));
        assert!(!UnifiedHistoricalProvider::is_korean_symbol("12345")); // 5자리
        assert!(!UnifiedHistoricalProvider::is_korean_symbol("1234567")); // 7자리
    }

    #[test]
    fn test_timeframe_to_period() {
        assert_eq!(
            UnifiedHistoricalProvider::timeframe_to_period(Timeframe::D1),
            "D"
        );
        assert_eq!(
            UnifiedHistoricalProvider::timeframe_to_period(Timeframe::W1),
            "W"
        );
        assert_eq!(
            UnifiedHistoricalProvider::timeframe_to_period(Timeframe::MN1),
            "M"
        );
    }

    #[test]
    fn test_needs_minute_data() {
        assert!(UnifiedHistoricalProvider::needs_minute_data(Timeframe::M1));
        assert!(UnifiedHistoricalProvider::needs_minute_data(Timeframe::M5));
        assert!(UnifiedHistoricalProvider::needs_minute_data(Timeframe::M15));
        assert!(UnifiedHistoricalProvider::needs_minute_data(Timeframe::H1));
        assert!(!UnifiedHistoricalProvider::needs_minute_data(Timeframe::D1));
        assert!(!UnifiedHistoricalProvider::needs_minute_data(Timeframe::W1));
    }
}
