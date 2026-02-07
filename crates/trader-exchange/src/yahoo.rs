//! Yahoo Finance 과거 데이터 제공자.
//!
//! Yahoo Finance API를 사용하여 과거 캔들(OHLCV) 데이터를 조회합니다.
//!
//! # 지원 간격
//!
//! - **분봉**: 1m, 2m, 5m, 15m, 30m (최근 60일 제한)
//! - **시간봉**: 1h (최근 60일 제한)
//! - **일봉 이상**: 1d, 1wk, 1mo (수년간 데이터 가능)
//!
//! # 심볼 형식
//!
//! 모든 심볼은 Yahoo Finance 형식으로 전달되어야 합니다:
//! - 한국 주식: "005930.KS" (코스피) 또는 "124560.KQ" (코스닥)
//! - 미국 주식: "AAPL", "GOOGL"
//! - ETF: "SPY", "QQQ"
//!
//! # 사용 예제
//!
//! ```rust,ignore
//! use trader_exchange::yahoo::YahooFinanceProvider;
//! use trader_core::Timeframe;
//!
//! let provider = YahooFinanceProvider::new();
//! let klines = provider.get_klines("AAPL", Timeframe::D1, 100).await?;
//! ```

#![allow(dead_code)] // 향후 확장을 위한 헬퍼 메서드

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use tracing::{debug, info, warn};
use yahoo_finance_api as yahoo;

use crate::historical::HistoricalDataProvider;
use crate::ExchangeError;
use trader_core::{Kline, Symbol, Timeframe};

/// Yahoo Finance 과거 데이터 제공자.
///
/// KIS API 대신 Yahoo Finance를 사용하여 차트 데이터를 조회합니다.
/// 백테스트와 라이브 트레이딩에서 동일한 데이터셋을 사용할 수 있습니다.
pub struct YahooFinanceProvider {
    connector: yahoo::YahooConnector,
}

impl YahooFinanceProvider {
    /// 새로운 Yahoo Finance 제공자 생성.
    pub fn new() -> Result<Self, ExchangeError> {
        let connector = yahoo::YahooConnector::new()
            .map_err(|e| ExchangeError::NetworkError(format!("Yahoo Finance 연결 실패: {}", e)))?;

        Ok(Self { connector })
    }

    /// 타임프레임을 Yahoo Finance 간격 문자열로 변환.
    pub fn timeframe_to_interval(timeframe: Timeframe) -> &'static str {
        match timeframe {
            Timeframe::M1 => "1m",
            Timeframe::M3 => "5m", // Yahoo는 3분봉이 없으므로 5분봉 사용
            Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1 => "1h",
            Timeframe::H2 => "1h", // Yahoo는 2시간봉이 없으므로 1시간봉 사용
            Timeframe::H4 => "1h", // Yahoo는 4시간봉이 없으므로 1시간봉 사용
            Timeframe::H6 => "1h",
            Timeframe::H8 => "1h",
            Timeframe::H12 => "1h",
            Timeframe::D1 => "1d",
            Timeframe::D3 => "1d", // Yahoo는 3일봉이 없음
            Timeframe::W1 => "1wk",
            Timeframe::MN1 => "1mo",
        }
    }

    /// 타임프레임과 limit에 따른 조회 기간 문자열 반환.
    ///
    /// Yahoo Finance API의 range 파라미터 형식:
    /// - "1d", "5d", "1mo", "3mo", "6mo", "1y", "2y", "5y", "10y", "ytd", "max"
    pub fn calculate_range_string(timeframe: Timeframe, limit: usize) -> &'static str {
        match timeframe {
            // 분봉/시간봉: 최대 60일이므로 1mo~2mo 범위 사용
            Timeframe::M1 | Timeframe::M3 | Timeframe::M5 | Timeframe::M15 | Timeframe::M30 => {
                if limit <= 100 {
                    "5d"
                } else if limit <= 500 {
                    "1mo"
                } else {
                    "3mo"
                } // 60일 제한이 있으므로 더 늘려도 의미 없음
            }

            Timeframe::H1
            | Timeframe::H2
            | Timeframe::H4
            | Timeframe::H6
            | Timeframe::H8
            | Timeframe::H12 => {
                if limit <= 50 {
                    "5d"
                } else if limit <= 200 {
                    "1mo"
                } else {
                    "3mo"
                }
            }

            // 일봉 이상: 제한 없음
            Timeframe::D1 => {
                if limit <= 5 {
                    "5d"
                } else if limit <= 20 {
                    "1mo"
                } else if limit <= 60 {
                    "3mo"
                } else if limit <= 120 {
                    "6mo"
                } else if limit <= 250 {
                    "1y"
                } else if limit <= 500 {
                    "2y"
                } else if limit <= 1250 {
                    "5y"
                } else {
                    "10y"
                }
            }
            Timeframe::D3 => {
                if limit <= 10 {
                    "1mo"
                } else if limit <= 30 {
                    "3mo"
                } else if limit <= 60 {
                    "6mo"
                } else {
                    "1y"
                }
            }
            Timeframe::W1 => {
                if limit <= 4 {
                    "1mo"
                } else if limit <= 12 {
                    "3mo"
                } else if limit <= 26 {
                    "6mo"
                } else if limit <= 52 {
                    "1y"
                } else if limit <= 104 {
                    "2y"
                } else {
                    "5y"
                }
            }
            Timeframe::MN1 => {
                if limit <= 3 {
                    "3mo"
                } else if limit <= 6 {
                    "6mo"
                } else if limit <= 12 {
                    "1y"
                } else if limit <= 24 {
                    "2y"
                } else if limit <= 60 {
                    "5y"
                } else {
                    "10y"
                }
            }
        }
    }

    /// 분봉/시간봉이 필요한 타임프레임인지 확인.
    pub fn is_intraday(timeframe: Timeframe) -> bool {
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

    /// 타임프레임의 지속 시간 계산.
    fn timeframe_duration(timeframe: Timeframe) -> chrono::Duration {
        match timeframe {
            Timeframe::M1 => chrono::Duration::minutes(1),
            Timeframe::M3 => chrono::Duration::minutes(3),
            Timeframe::M5 => chrono::Duration::minutes(5),
            Timeframe::M15 => chrono::Duration::minutes(15),
            Timeframe::M30 => chrono::Duration::minutes(30),
            Timeframe::H1 => chrono::Duration::hours(1),
            Timeframe::H2 => chrono::Duration::hours(2),
            Timeframe::H4 => chrono::Duration::hours(4),
            Timeframe::H6 => chrono::Duration::hours(6),
            Timeframe::H8 => chrono::Duration::hours(8),
            Timeframe::H12 => chrono::Duration::hours(12),
            Timeframe::D1 => chrono::Duration::days(1),
            Timeframe::D3 => chrono::Duration::days(3),
            Timeframe::W1 => chrono::Duration::weeks(1),
            Timeframe::MN1 => chrono::Duration::days(30),
        }
    }

    /// Yahoo Quote를 Kline으로 변환.
    fn quote_to_kline(&self, symbol: &Symbol, timeframe: Timeframe, quote: &yahoo::Quote) -> Kline {
        // Unix timestamp를 DateTime으로 변환
        let open_time = Utc
            .timestamp_opt(quote.timestamp, 0)
            .single()
            .unwrap_or_else(Utc::now);
        let close_time = open_time + Self::timeframe_duration(timeframe);
        Kline {
            ticker: symbol.to_string(),
            timeframe,
            open_time,
            open: Decimal::from_f64_retain(quote.open).unwrap_or_default(),
            high: Decimal::from_f64_retain(quote.high).unwrap_or_default(),
            low: Decimal::from_f64_retain(quote.low).unwrap_or_default(),
            close: Decimal::from_f64_retain(quote.close).unwrap_or_default(),
            volume: Decimal::from(quote.volume),
            close_time,
            quote_volume: None,
            num_trades: None,
        }
    }

    /// 심볼의 통화 코드 추정.
    fn guess_currency(symbol: &str) -> &'static str {
        if symbol.ends_with(".KS") || symbol.ends_with(".KQ") {
            "KRW"
        } else if symbol.ends_with(".T") {
            "JPY"
        } else if symbol.ends_with(".L") {
            "GBP"
        } else {
            "USD"
        }
    }
}

// NOTE: Default 트레잇 구현 제거됨
// `new()`가 `Result`를 반환하므로 Default 트레잇은 적합하지 않습니다.
// 대신 `YahooFinanceProvider::new()?`를 사용하세요.

#[async_trait]
impl HistoricalDataProvider for YahooFinanceProvider {
    async fn get_klines(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>, ExchangeError> {
        // symbol은 이미 Yahoo Finance 형식이어야 함 (예: "005930.KS", "AAPL")
        let yahoo_symbol = symbol;
        let interval = Self::timeframe_to_interval(timeframe);
        let range = Self::calculate_range_string(timeframe, limit);

        info!(
            "Yahoo Finance: {} klines for {} (interval: {}, range: {})",
            limit, yahoo_symbol, interval, range
        );

        // Yahoo Finance API 호출 (get_quote_range 사용)
        let response = self
            .connector
            .get_quote_range(yahoo_symbol, interval, range)
            .await
            .map_err(|e| ExchangeError::ApiError {
                code: 0,
                message: format!("Yahoo Finance API 오류 ({}): {}", yahoo_symbol, e),
            })?;

        let quotes = response
            .quotes()
            .map_err(|e| ExchangeError::ParseError(format!("Quote 파싱 오류: {}", e)))?;

        if quotes.is_empty() {
            warn!("Yahoo Finance: {} 데이터 없음", yahoo_symbol);
            return Ok(Vec::new());
        }

        debug!(
            "Yahoo Finance: {} 캔들 {} 개 수신",
            yahoo_symbol,
            quotes.len()
        );

        // 통화 코드 추정
        let currency = Self::guess_currency(yahoo_symbol);
        let symbol_obj = Symbol::stock(symbol, currency);

        // Quote를 Kline으로 변환
        let klines: Vec<Kline> = quotes
            .iter()
            .map(|q| self.quote_to_kline(&symbol_obj, timeframe, q))
            .collect();

        // 시간순 정렬 (오래된 것부터)
        let mut sorted_klines = klines;
        sorted_klines.sort_by_key(|k| k.open_time);

        // 최근 limit개만 반환 (뒤에서부터)
        if sorted_klines.len() > limit {
            let skip = sorted_klines.len() - limit;
            sorted_klines = sorted_klines.into_iter().skip(skip).collect();
        }

        Ok(sorted_klines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeframe_to_interval() {
        assert_eq!(
            YahooFinanceProvider::timeframe_to_interval(Timeframe::M1),
            "1m"
        );
        assert_eq!(
            YahooFinanceProvider::timeframe_to_interval(Timeframe::M5),
            "5m"
        );
        assert_eq!(
            YahooFinanceProvider::timeframe_to_interval(Timeframe::H1),
            "1h"
        );
        assert_eq!(
            YahooFinanceProvider::timeframe_to_interval(Timeframe::D1),
            "1d"
        );
        assert_eq!(
            YahooFinanceProvider::timeframe_to_interval(Timeframe::W1),
            "1wk"
        );
        assert_eq!(
            YahooFinanceProvider::timeframe_to_interval(Timeframe::MN1),
            "1mo"
        );
    }

    #[test]
    fn test_is_intraday() {
        assert!(YahooFinanceProvider::is_intraday(Timeframe::M1));
        assert!(YahooFinanceProvider::is_intraday(Timeframe::H1));
        assert!(!YahooFinanceProvider::is_intraday(Timeframe::D1));
        assert!(!YahooFinanceProvider::is_intraday(Timeframe::W1));
    }

    #[test]
    fn test_guess_currency() {
        assert_eq!(YahooFinanceProvider::guess_currency("005930.KS"), "KRW");
        assert_eq!(YahooFinanceProvider::guess_currency("AAPL"), "USD");
        assert_eq!(YahooFinanceProvider::guess_currency("7203.T"), "JPY");
    }
}
