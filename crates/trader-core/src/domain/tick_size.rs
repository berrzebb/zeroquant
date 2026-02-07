//! 거래소별 호가 단위(Tick Size) 처리 모듈.
//!
//! 각 거래소는 고유한 호가 단위 규칙을 가지고 있으며,
//! 이 모듈은 가격을 호가 단위에 맞게 라운딩하는 기능을 제공합니다.

use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;

/// 호가 단위 라운딩 방법
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundMethod {
    /// 일반 반올림 (기본)
    Round,
    /// 내림 (보수적, 매수 시 유리)
    Floor,
    /// 올림 (공격적, 매도 시 유리)
    Ceil,
}

/// 거래소별 호가 단위 제공자 trait
pub trait TickSizeProvider: Send + Sync {
    /// 주어진 가격에 대한 호가 단위를 반환합니다.
    ///
    /// # Arguments
    /// * `price` - 기준 가격
    ///
    /// # Returns
    /// 해당 가격 구간의 호가 단위
    fn tick_size(&self, price: Decimal) -> Decimal;

    /// 가격을 호가 단위로 라운딩합니다.
    ///
    /// # Arguments
    /// * `price` - 라운딩할 가격
    /// * `method` - 라운딩 방법
    ///
    /// # Returns
    /// 호가 단위로 라운딩된 가격
    fn round_to_tick(&self, price: Decimal, method: RoundMethod) -> Decimal {
        let tick = self.tick_size(price);
        if tick.is_zero() {
            return price;
        }

        let ticks = price / tick;
        let rounded_ticks = match method {
            RoundMethod::Round => ticks.round(),
            RoundMethod::Floor => ticks.floor(),
            RoundMethod::Ceil => ticks.ceil(),
        };

        rounded_ticks * tick
    }

    /// 가격이 호가 단위에 맞는지 검증합니다.
    ///
    /// # Arguments
    /// * `price` - 검증할 가격
    ///
    /// # Returns
    /// 유효하면 true, 아니면 false
    fn is_valid_price(&self, price: Decimal) -> bool {
        let tick = self.tick_size(price);
        if tick.is_zero() {
            return true;
        }

        let remainder = price % tick;
        remainder.is_zero()
    }
}

/// KRX (한국거래소) 호가 단위 제공자
///
/// 한국 주식시장의 7단계 호가 규칙을 구현합니다:
/// - 100원 미만: 1원
/// - 100원 이상 ~ 1,000원 미만: 5원
/// - 1,000원 이상 ~ 10,000원 미만: 10원
/// - 10,000원 이상 ~ 50,000원 미만: 50원
/// - 50,000원 이상 ~ 100,000원 미만: 100원
/// - 100,000원 이상 ~ 500,000원 미만: 500원
/// - 500,000원 이상: 1,000원
#[derive(Debug, Clone)]
pub struct KrxTickSize;

impl KrxTickSize {
    pub fn new() -> Self {
        Self
    }
}

impl Default for KrxTickSize {
    fn default() -> Self {
        Self::new()
    }
}

impl TickSizeProvider for KrxTickSize {
    fn tick_size(&self, price: Decimal) -> Decimal {
        use rust_decimal_macros::dec;

        if price < dec!(100) {
            dec!(1)
        } else if price < dec!(1_000) {
            dec!(5)
        } else if price < dec!(10_000) {
            dec!(10)
        } else if price < dec!(50_000) {
            dec!(50)
        } else if price < dec!(100_000) {
            dec!(100)
        } else if price < dec!(500_000) {
            dec!(500)
        } else {
            dec!(1_000)
        }
    }
}

/// 미국 주식 시장 호가 단위 제공자
///
/// 미국 주식은 고정 $0.01 호가 단위를 사용합니다.
#[derive(Debug, Clone)]
pub struct UsEquityTickSize;

impl UsEquityTickSize {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UsEquityTickSize {
    fn default() -> Self {
        Self::new()
    }
}

impl TickSizeProvider for UsEquityTickSize {
    fn tick_size(&self, _price: Decimal) -> Decimal {
        rust_decimal_macros::dec!(0.01)
    }
}

/// Binance 거래소 호가 단위 제공자
///
/// Binance는 심볼별로 다른 tick_size를 사용합니다.
/// Exchange Info API에서 심볼 정보를 가져와 캐싱합니다.
#[derive(Debug, Clone)]
pub struct BinanceTickSize {
    /// 심볼별 tick_size 캐시
    tick_sizes: Arc<HashMap<String, Decimal>>,
    /// 기본 tick_size (심볼 정보가 없을 때)
    default_tick_size: Decimal,
}

impl BinanceTickSize {
    /// 새로운 Binance tick_size 제공자를 생성합니다.
    ///
    /// # Arguments
    /// * `tick_sizes` - 심볼별 tick_size 맵
    /// * `default_tick_size` - 기본 tick_size (옵션)
    pub fn new(tick_sizes: HashMap<String, Decimal>, default_tick_size: Option<Decimal>) -> Self {
        Self {
            tick_sizes: Arc::new(tick_sizes),
            default_tick_size: default_tick_size.unwrap_or(rust_decimal_macros::dec!(0.01)),
        }
    }

    /// 심볼에 대한 tick_size를 설정합니다.
    pub fn with_symbol(mut self, symbol: &str, tick_size: Decimal) -> Self {
        let mut tick_sizes = (*self.tick_sizes).clone();
        tick_sizes.insert(symbol.to_string(), tick_size);
        self.tick_sizes = Arc::new(tick_sizes);
        self
    }

    /// 특정 심볼의 tick_size를 조회합니다.
    pub fn get_tick_size_for_symbol(&self, symbol: &str) -> Decimal {
        self.tick_sizes
            .get(symbol)
            .copied()
            .unwrap_or(self.default_tick_size)
    }
}

impl Default for BinanceTickSize {
    fn default() -> Self {
        Self::new(HashMap::new(), None)
    }
}

impl TickSizeProvider for BinanceTickSize {
    fn tick_size(&self, _price: Decimal) -> Decimal {
        // 기본값 반환 (심볼별 조회는 get_tick_size_for_symbol 사용)
        self.default_tick_size
    }
}

// 거래소별 팩토리 함수는 trader-exchange 크레이트에서 제공합니다.
// trader-core는 거래소 중립적인 trait과 구현체만 제공합니다.

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_krx_tick_size() {
        let provider = KrxTickSize::new();

        // 각 구간 테스트
        assert_eq!(provider.tick_size(dec!(50)), dec!(1));
        assert_eq!(provider.tick_size(dec!(99)), dec!(1));
        assert_eq!(provider.tick_size(dec!(100)), dec!(5));
        assert_eq!(provider.tick_size(dec!(999)), dec!(5));
        assert_eq!(provider.tick_size(dec!(1_000)), dec!(10));
        assert_eq!(provider.tick_size(dec!(9_999)), dec!(10));
        assert_eq!(provider.tick_size(dec!(10_000)), dec!(50));
        assert_eq!(provider.tick_size(dec!(49_999)), dec!(50));
        assert_eq!(provider.tick_size(dec!(50_000)), dec!(100));
        assert_eq!(provider.tick_size(dec!(99_999)), dec!(100));
        assert_eq!(provider.tick_size(dec!(100_000)), dec!(500));
        assert_eq!(provider.tick_size(dec!(499_999)), dec!(500));
        assert_eq!(provider.tick_size(dec!(500_000)), dec!(1_000));
        assert_eq!(provider.tick_size(dec!(1_000_000)), dec!(1_000));
    }

    #[test]
    fn test_krx_round_to_tick() {
        let provider = KrxTickSize::new();

        // 35,432원 -> 50원 단위 (35,450원)
        assert_eq!(
            provider.round_to_tick(dec!(35_432), RoundMethod::Round),
            dec!(35_450)
        );

        // 35,432원 -> 내림 (35,400원)
        assert_eq!(
            provider.round_to_tick(dec!(35_432), RoundMethod::Floor),
            dec!(35_400)
        );

        // 35,432원 -> 올림 (35,450원)
        assert_eq!(
            provider.round_to_tick(dec!(35_432), RoundMethod::Ceil),
            dec!(35_450)
        );

        // 120,750원 -> 500원 단위 (반올림 시 121,000원)
        assert_eq!(
            provider.round_to_tick(dec!(120_750), RoundMethod::Round),
            dec!(121_000)
        );
    }

    #[test]
    fn test_krx_is_valid_price() {
        let provider = KrxTickSize::new();

        // 유효한 가격들
        assert!(provider.is_valid_price(dec!(35_450))); // 50원 단위
        assert!(provider.is_valid_price(dec!(120_500))); // 500원 단위
        assert!(provider.is_valid_price(dec!(99))); // 1원 단위

        // 무효한 가격들
        assert!(!provider.is_valid_price(dec!(35_432))); // 50원 단위 아님
        assert!(!provider.is_valid_price(dec!(120_750))); // 500원 단위 아님
    }

    #[test]
    fn test_us_equity_tick_size() {
        let provider = UsEquityTickSize::new();

        // 모든 가격에 대해 $0.01
        assert_eq!(provider.tick_size(dec!(1.0)), dec!(0.01));
        assert_eq!(provider.tick_size(dec!(100.0)), dec!(0.01));
        assert_eq!(provider.tick_size(dec!(1000.0)), dec!(0.01));

        // 라운딩 테스트
        assert_eq!(
            provider.round_to_tick(dec!(123.456), RoundMethod::Round),
            dec!(123.46)
        );
        assert_eq!(
            provider.round_to_tick(dec!(123.456), RoundMethod::Floor),
            dec!(123.45)
        );
    }

    #[test]
    fn test_binance_tick_size() {
        let mut tick_sizes = HashMap::new();
        tick_sizes.insert("BTCUSDT".to_string(), dec!(0.01));
        tick_sizes.insert("ETHUSDT".to_string(), dec!(0.01));
        tick_sizes.insert("DOGEUSDT".to_string(), dec!(0.00001));

        let provider = BinanceTickSize::new(tick_sizes, Some(dec!(0.01)));

        // 심볼별 tick_size
        assert_eq!(provider.get_tick_size_for_symbol("BTCUSDT"), dec!(0.01));
        assert_eq!(provider.get_tick_size_for_symbol("DOGEUSDT"), dec!(0.00001));

        // 없는 심볼은 기본값
        assert_eq!(provider.get_tick_size_for_symbol("UNKNOWN"), dec!(0.01));
    }

    #[test]
    fn test_round_method() {
        let provider = KrxTickSize::new();
        let price = dec!(35_432); // 50원 단위 구간

        // Round: 35,432 -> 35,450 (가까운 쪽)
        assert_eq!(
            provider.round_to_tick(price, RoundMethod::Round),
            dec!(35_450)
        );

        // Floor: 35,432 -> 35,400 (내림)
        assert_eq!(
            provider.round_to_tick(price, RoundMethod::Floor),
            dec!(35_400)
        );

        // Ceil: 35,432 -> 35,450 (올림)
        assert_eq!(
            provider.round_to_tick(price, RoundMethod::Ceil),
            dec!(35_450)
        );
    }
}
