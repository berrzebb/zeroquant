//! 공통 유틸리티 함수.
//!
//! 여러 모듈에서 사용되는 공통 함수를 통합합니다.

use rust_decimal::Decimal;
use trader_analytics::{IndicatorEngine, indicators::TtmSqueezeParams};
use trader_core::types::MarketType;
use trader_core::Kline;

/// CamelCase 문자열을 SCREAMING_SNAKE_CASE로 변환.
///
/// # 예시
/// - "UpTrend" → "UP_TREND"
/// - "StrongUptrend" → "STRONG_UPTREND"
pub fn to_screaming_snake_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_uppercase());
    }
    result
}

/// 시장 코드를 MarketType으로 변환.
///
/// # 인자
/// - `market`: 시장 코드 (예: "KR", "US", "CRYPTO")
///
/// # 반환
/// 해당하는 MarketType (기본값: Stock)
pub fn market_to_market_type(market: &str) -> MarketType {
    match market {
        "KR" => MarketType::Stock,
        "US" => MarketType::Stock,
        "CRYPTO" => MarketType::Crypto,
        "FOREX" => MarketType::Forex,
        "FUTURES" => MarketType::Futures,
        _ => MarketType::Stock,
    }
}

/// TTM Squeeze 지표 계산.
///
/// 볼린저 밴드와 켈트너 채널을 비교하여 스퀴즈 상태를 판단합니다.
///
/// # 인자
/// - `engine`: 지표 계산 엔진
/// - `candles`: OHLCV 캔들 데이터 (최소 20개)
///
/// # 반환
/// (squeeze 상태, 연속 스퀴즈 횟수)
pub fn calculate_ttm_squeeze(
    engine: &IndicatorEngine,
    candles: &[Kline],
) -> (Option<bool>, Option<i32>) {
    let high: Vec<Decimal> = candles.iter().map(|c| c.high).collect();
    let low: Vec<Decimal> = candles.iter().map(|c| c.low).collect();
    let close: Vec<Decimal> = candles.iter().map(|c| c.close).collect();
    let params = TtmSqueezeParams::default();

    match engine.ttm_squeeze(&high, &low, &close, params) {
        Ok(results) if !results.is_empty() => {
            let latest = results.last().unwrap();
            (Some(latest.is_squeeze), Some(latest.squeeze_count as i32))
        }
        Ok(_) => (None, None),
        Err(_) => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_screaming_snake_case() {
        assert_eq!(to_screaming_snake_case("UpTrend"), "UP_TREND");
        assert_eq!(to_screaming_snake_case("StrongUptrend"), "STRONG_UPTREND");
        assert_eq!(to_screaming_snake_case("REST"), "REST");
        assert_eq!(to_screaming_snake_case("correction"), "CORRECTION");
    }

    #[test]
    fn test_market_to_market_type() {
        assert_eq!(market_to_market_type("KR"), MarketType::Stock);
        assert_eq!(market_to_market_type("US"), MarketType::Stock);
        assert_eq!(market_to_market_type("CRYPTO"), MarketType::Crypto);
        assert_eq!(market_to_market_type("UNKNOWN"), MarketType::Stock);
    }
}
