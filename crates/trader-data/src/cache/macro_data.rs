//! MacroDataProvider - 매크로 경제 지표 데이터 제공자.
//!
//! Yahoo Finance API를 통해 주요 지수와 환율을 조회합니다.
//!
//! # 지원 심볼
//!
//! - **KOSPI**: "^KS11"
//! - **KOSDAQ**: "^KQ11"
//! - **USD/KRW**: "KRW=X"
//! - **VIX**: "^VIX"
//! - **NASDAQ**: "^IXIC"
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! use trader_data::cache::MacroDataProvider;
//!
//! let provider = MacroDataProvider::new()?;
//! let data = provider.fetch_macro_data().await?;
//!
//! println!("USD/KRW: {} ({:+.2}%)", data.usd_krw, data.usd_change_pct);
//! println!("NASDAQ: {:+.2}%", data.nasdaq_change_pct);
//! ```

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error};
use yahoo_finance_api as yahoo;

use crate::RedisCache;

/// 매크로 데이터 캐시 키
const MACRO_DATA_CACHE_KEY: &str = "macro:data";
/// 매크로 데이터 캐시 TTL (10분)
/// 동기화 주기(5분)보다 충분히 길게 설정하여 캐시 갱신 전 만료 방지
const MACRO_DATA_CACHE_TTL_SECS: u64 = 600;

/// 매크로 데이터 조회 에러.
#[derive(Debug, Error)]
pub enum MacroDataError {
    #[error("Yahoo Finance 연결 실패: {0}")]
    ConnectionError(String),

    #[error("API 요청 실패 ({symbol}): {message}")]
    ApiError { symbol: String, message: String },

    #[error("데이터 파싱 실패: {0}")]
    ParseError(String),

    #[error("데이터 없음: {0}")]
    NoData(String),
}

/// 매크로 경제 지표 데이터.
///
/// # 필드
///
/// - `kospi`: KOSPI 지수
/// - `kosdaq`: KOSDAQ 지수
/// - `usd_krw`: USD/KRW 환율
/// - `vix`: VIX 변동성 지수
/// - `nasdaq`: NASDAQ 지수
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroData {
    /// 현재 KOSPI 지수
    pub kospi_close: Decimal,

    /// 전일 KOSPI 종가
    pub kospi_prev_close: Decimal,

    /// 전일 대비 KOSPI 변동률 (%)
    pub kospi_change_pct: f64,

    /// 현재 KOSDAQ 지수
    pub kosdaq_close: Decimal,

    /// 전일 KOSDAQ 종가
    pub kosdaq_prev_close: Decimal,

    /// 전일 대비 KOSDAQ 변동률 (%)
    pub kosdaq_change_pct: f64,

    /// 현재 USD/KRW 환율
    pub usd_krw: Decimal,

    /// 전일 USD/KRW 종가
    pub usd_prev_close: Decimal,

    /// 전일 대비 환율 변동률 (%)
    pub usd_change_pct: f64,

    /// 현재 VIX 지수
    pub vix_close: Decimal,

    /// 전일 VIX 종가
    pub vix_prev_close: Decimal,

    /// 전일 대비 VIX 변동률 (%)
    pub vix_change_pct: f64,

    /// 현재 나스닥 지수
    pub nasdaq_close: Decimal,

    /// 전일 나스닥 종가
    pub nasdaq_prev_close: Decimal,

    /// 전일 대비 나스닥 변동률 (%)
    pub nasdaq_change_pct: f64,
}

impl MacroData {
    /// 변동률 계산 (%)
    fn calculate_change_pct(current: Decimal, previous: Decimal) -> f64 {
        if previous.is_zero() {
            return 0.0;
        }

        let change = current - previous;
        let pct = (change / previous) * Decimal::from(100);

        pct.to_string().parse::<f64>().unwrap_or(0.0)
    }
}

/// 매크로 데이터 제공자 트레잇.
#[async_trait]
pub trait MacroDataProviderTrait: Send + Sync {
    /// 매크로 경제 지표 데이터 조회.
    async fn fetch_macro_data(&self) -> Result<MacroData, MacroDataError>;
}

/// Yahoo Finance 기반 매크로 데이터 제공자.
pub struct MacroDataProvider {
    connector: yahoo::YahooConnector,
}

impl MacroDataProvider {
    /// 새로운 MacroDataProvider 생성.
    pub fn new() -> Result<Self, MacroDataError> {
        let connector = yahoo::YahooConnector::new()
            .map_err(|e| MacroDataError::ConnectionError(format!("{}", e)))?;

        Ok(Self { connector })
    }

    /// 심볼의 최근 2일 데이터 조회 (현재가 + 전일 종가).
    async fn fetch_quotes(&self, symbol: &str) -> Result<Vec<yahoo::Quote>, MacroDataError> {
        debug!("매크로 데이터 조회: {}", symbol);

        // 최근 5일 데이터 조회 (주말 등을 고려하여 여유 있게)
        let response = self
            .connector
            .get_quote_range(symbol, "1d", "5d")
            .await
            .map_err(|e| MacroDataError::ApiError {
                symbol: symbol.to_string(),
                message: format!("{}", e),
            })?;

        let quotes = response
            .quotes()
            .map_err(|e| MacroDataError::ParseError(format!("{}", e)))?;

        if quotes.is_empty() {
            return Err(MacroDataError::NoData(format!(
                "심볼 {} 데이터 없음",
                symbol
            )));
        }

        debug!("{} 캔들 {} 개 수신", symbol, quotes.len());
        Ok(quotes)
    }

    /// USD/KRW 환율 데이터 조회.
    async fn fetch_usd_krw(&self) -> Result<(Decimal, Decimal), MacroDataError> {
        let quotes = self.fetch_quotes("KRW=X").await?;

        if quotes.len() < 2 {
            return Err(MacroDataError::NoData(
                "USD/KRW 이전 데이터 부족 (최소 2개 필요)".to_string(),
            ));
        }

        // 최근 2개 데이터 추출 (마지막이 최신)
        let current = &quotes[quotes.len() - 1];
        let previous = &quotes[quotes.len() - 2];

        let current_price = Decimal::from_f64_retain(current.close)
            .ok_or_else(|| MacroDataError::ParseError("USD/KRW 현재가 변환 실패".to_string()))?;

        let prev_price = Decimal::from_f64_retain(previous.close)
            .ok_or_else(|| MacroDataError::ParseError("USD/KRW 전일가 변환 실패".to_string()))?;

        debug!("USD/KRW: {} (전일: {})", current_price, prev_price);
        Ok((current_price, prev_price))
    }

    /// 나스닥 지수 데이터 조회.
    async fn fetch_nasdaq(&self) -> Result<(Decimal, Decimal), MacroDataError> {
        let quotes = self.fetch_quotes("^IXIC").await?;

        if quotes.len() < 2 {
            return Err(MacroDataError::NoData(
                "나스닥 이전 데이터 부족 (최소 2개 필요)".to_string(),
            ));
        }

        // 최근 2개 데이터 추출
        let current = &quotes[quotes.len() - 1];
        let previous = &quotes[quotes.len() - 2];

        let current_price = Decimal::from_f64_retain(current.close)
            .ok_or_else(|| MacroDataError::ParseError("나스닥 현재가 변환 실패".to_string()))?;

        let prev_price = Decimal::from_f64_retain(previous.close)
            .ok_or_else(|| MacroDataError::ParseError("나스닥 전일가 변환 실패".to_string()))?;

        debug!("NASDAQ: {} (전일: {})", current_price, prev_price);
        Ok((current_price, prev_price))
    }

    /// KOSPI 지수 데이터 조회.
    async fn fetch_kospi(&self) -> Result<(Decimal, Decimal), MacroDataError> {
        let quotes = self.fetch_quotes("^KS11").await?;

        if quotes.len() < 2 {
            return Err(MacroDataError::NoData(
                "KOSPI 이전 데이터 부족 (최소 2개 필요)".to_string(),
            ));
        }

        let current = &quotes[quotes.len() - 1];
        let previous = &quotes[quotes.len() - 2];

        let current_price = Decimal::from_f64_retain(current.close)
            .ok_or_else(|| MacroDataError::ParseError("KOSPI 현재가 변환 실패".to_string()))?;

        let prev_price = Decimal::from_f64_retain(previous.close)
            .ok_or_else(|| MacroDataError::ParseError("KOSPI 전일가 변환 실패".to_string()))?;

        debug!("KOSPI: {} (전일: {})", current_price, prev_price);
        Ok((current_price, prev_price))
    }

    /// KOSDAQ 지수 데이터 조회.
    async fn fetch_kosdaq(&self) -> Result<(Decimal, Decimal), MacroDataError> {
        let quotes = self.fetch_quotes("^KQ11").await?;

        if quotes.len() < 2 {
            return Err(MacroDataError::NoData(
                "KOSDAQ 이전 데이터 부족 (최소 2개 필요)".to_string(),
            ));
        }

        let current = &quotes[quotes.len() - 1];
        let previous = &quotes[quotes.len() - 2];

        let current_price = Decimal::from_f64_retain(current.close)
            .ok_or_else(|| MacroDataError::ParseError("KOSDAQ 현재가 변환 실패".to_string()))?;

        let prev_price = Decimal::from_f64_retain(previous.close)
            .ok_or_else(|| MacroDataError::ParseError("KOSDAQ 전일가 변환 실패".to_string()))?;

        debug!("KOSDAQ: {} (전일: {})", current_price, prev_price);
        Ok((current_price, prev_price))
    }

    /// VIX 변동성 지수 데이터 조회.
    async fn fetch_vix(&self) -> Result<(Decimal, Decimal), MacroDataError> {
        let quotes = self.fetch_quotes("^VIX").await?;

        if quotes.len() < 2 {
            return Err(MacroDataError::NoData(
                "VIX 이전 데이터 부족 (최소 2개 필요)".to_string(),
            ));
        }

        let current = &quotes[quotes.len() - 1];
        let previous = &quotes[quotes.len() - 2];

        let current_price = Decimal::from_f64_retain(current.close)
            .ok_or_else(|| MacroDataError::ParseError("VIX 현재가 변환 실패".to_string()))?;

        let prev_price = Decimal::from_f64_retain(previous.close)
            .ok_or_else(|| MacroDataError::ParseError("VIX 전일가 변환 실패".to_string()))?;

        debug!("VIX: {} (전일: {})", current_price, prev_price);
        Ok((current_price, prev_price))
    }
}

#[async_trait]
impl MacroDataProviderTrait for MacroDataProvider {
    async fn fetch_macro_data(&self) -> Result<MacroData, MacroDataError> {
        debug!("매크로 경제 지표 데이터 수집 시작");

        // KOSPI 지수 조회
        let (kospi_close, kospi_prev_close) = self.fetch_kospi().await.unwrap_or_else(|e| {
            error!("KOSPI 조회 실패: {}", e);
            (Decimal::ZERO, Decimal::ZERO)
        });
        let kospi_change_pct = MacroData::calculate_change_pct(kospi_close, kospi_prev_close);

        // KOSDAQ 지수 조회
        let (kosdaq_close, kosdaq_prev_close) = self.fetch_kosdaq().await.unwrap_or_else(|e| {
            error!("KOSDAQ 조회 실패: {}", e);
            (Decimal::ZERO, Decimal::ZERO)
        });
        let kosdaq_change_pct = MacroData::calculate_change_pct(kosdaq_close, kosdaq_prev_close);

        // USD/KRW 환율 조회
        let (usd_krw, usd_prev_close) = self.fetch_usd_krw().await?;
        let usd_change_pct = MacroData::calculate_change_pct(usd_krw, usd_prev_close);

        // VIX 지수 조회
        let (vix_close, vix_prev_close) = self.fetch_vix().await.unwrap_or_else(|e| {
            error!("VIX 조회 실패: {}", e);
            (Decimal::ZERO, Decimal::ZERO)
        });
        let vix_change_pct = MacroData::calculate_change_pct(vix_close, vix_prev_close);

        // 나스닥 지수 조회
        let (nasdaq_close, nasdaq_prev_close) = self.fetch_nasdaq().await?;
        let nasdaq_change_pct = MacroData::calculate_change_pct(nasdaq_close, nasdaq_prev_close);

        let data = MacroData {
            kospi_close,
            kospi_prev_close,
            kospi_change_pct,
            kosdaq_close,
            kosdaq_prev_close,
            kosdaq_change_pct,
            usd_krw,
            usd_prev_close,
            usd_change_pct,
            vix_close,
            vix_prev_close,
            vix_change_pct,
            nasdaq_close,
            nasdaq_prev_close,
            nasdaq_change_pct,
        };

        debug!(
            "매크로 데이터 수집 완료: KOSPI {} ({:+.2}%), KOSDAQ {} ({:+.2}%), USD/KRW {} ({:+.2}%), VIX {} ({:+.2}%), NASDAQ {:+.2}%",
            data.kospi_close, data.kospi_change_pct,
            data.kosdaq_close, data.kosdaq_change_pct,
            data.usd_krw, data.usd_change_pct,
            data.vix_close, data.vix_change_pct,
            data.nasdaq_change_pct
        );

        Ok(data)
    }
}

/// Redis 캐시를 활용한 매크로 데이터 제공자.
///
/// 캐시 히트 시 Redis에서 즉시 반환하고,
/// 캐시 미스 시 Yahoo Finance API를 호출하여 데이터를 가져온 후 캐시에 저장합니다.
pub struct CachedMacroDataProvider {
    provider: MacroDataProvider,
    cache: Arc<RedisCache>,
}

impl CachedMacroDataProvider {
    /// 새로운 CachedMacroDataProvider 생성.
    pub fn new(cache: Arc<RedisCache>) -> Result<Self, MacroDataError> {
        let provider = MacroDataProvider::new()?;
        Ok(Self { provider, cache })
    }

    /// 기존 MacroDataProvider를 래핑하여 생성.
    pub fn with_provider(provider: MacroDataProvider, cache: Arc<RedisCache>) -> Self {
        Self { provider, cache }
    }
}

#[async_trait]
impl MacroDataProviderTrait for CachedMacroDataProvider {
    async fn fetch_macro_data(&self) -> Result<MacroData, MacroDataError> {
        // 1. Redis 캐시 확인
        match self.cache.get::<MacroData>(MACRO_DATA_CACHE_KEY).await {
            Ok(Some(cached)) => {
                debug!("매크로 데이터 캐시 히트");
                return Ok(cached);
            }
            Ok(None) => {
                debug!("매크로 데이터 캐시 미스");
            }
            Err(e) => {
                error!("매크로 데이터 캐시 조회 실패: {}", e);
            }
        }

        // 2. Yahoo Finance API 호출
        let data = self.provider.fetch_macro_data().await?;

        // 3. 캐시 저장 (TTL: 5분)
        if let Err(e) = self
            .cache
            .set_with_ttl(MACRO_DATA_CACHE_KEY, &data, MACRO_DATA_CACHE_TTL_SECS)
            .await
        {
            error!("매크로 데이터 캐시 저장 실패: {}", e);
        }

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_change_pct() {
        // 정상 상승
        let current = Decimal::from(1350);
        let previous = Decimal::from(1300);
        let pct = MacroData::calculate_change_pct(current, previous);
        assert!((pct - 3.846).abs() < 0.01); // ~3.846%

        // 정상 하락
        let current = Decimal::from(1250);
        let previous = Decimal::from(1300);
        let pct = MacroData::calculate_change_pct(current, previous);
        assert!((pct + 3.846).abs() < 0.01); // ~-3.846%

        // 변동 없음
        let current = Decimal::from(1300);
        let previous = Decimal::from(1300);
        let pct = MacroData::calculate_change_pct(current, previous);
        assert_eq!(pct, 0.0);

        // 0으로 나누기 방지
        let current = Decimal::from(100);
        let previous = Decimal::ZERO;
        let pct = MacroData::calculate_change_pct(current, previous);
        assert_eq!(pct, 0.0);
    }

    #[tokio::test]
    #[ignore] // 실제 API 호출 필요
    async fn test_fetch_macro_data_integration() {
        let provider = MacroDataProvider::new().expect("Provider 생성 실패");
        let result = provider.fetch_macro_data().await;

        match result {
            Ok(data) => {
                println!("KOSPI: {} ({:+.2}%)", data.kospi_close, data.kospi_change_pct);
                println!("KOSDAQ: {} ({:+.2}%)", data.kosdaq_close, data.kosdaq_change_pct);
                println!("USD/KRW: {} ({:+.2}%)", data.usd_krw, data.usd_change_pct);
                println!("VIX: {} ({:+.2}%)", data.vix_close, data.vix_change_pct);
                println!("NASDAQ: {} ({:+.2}%)", data.nasdaq_close, data.nasdaq_change_pct);
                assert!(data.usd_krw > Decimal::ZERO);
                assert!(data.nasdaq_close > Decimal::ZERO);
            }
            Err(e) => {
                eprintln!("API 호출 실패: {}", e);
            }
        }
    }
}
