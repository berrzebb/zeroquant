//! 정밀한 금융 계산을 위한 Decimal 유틸리티.
//!
//! 이 모듈은 금융 계산에 필요한 정밀 소수점 타입 및 유틸리티를 제공합니다.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 금융 정밀도를 위한 가격 타입.
pub type Price = Decimal;

/// 주문 수량을 위한 타입.
pub type Quantity = Decimal;

/// 퍼센트 타입 (0.01 = 1%).
pub type Percentage = Decimal;

/// Decimal 연산을 위한 확장 트레이트.
pub trait DecimalExt {
    /// 양수인지 확인합니다.
    fn is_positive(&self) -> bool;

    /// 음수인지 확인합니다.
    fn is_negative(&self) -> bool;

    /// 절대값을 반환합니다.
    fn abs(&self) -> Decimal;

    /// 퍼센트 문자열로 변환합니다 (예: "5.25%").
    fn to_percentage_string(&self) -> String;

    /// 지정된 소수점 자릿수로 반올림합니다.
    fn round_dp(&self, dp: u32) -> Decimal;
}

impl DecimalExt for Decimal {
    fn is_positive(&self) -> bool {
        *self > Decimal::ZERO
    }

    fn is_negative(&self) -> bool {
        *self < Decimal::ZERO
    }

    fn abs(&self) -> Decimal {
        if self.is_sign_negative() {
            -*self
        } else {
            *self
        }
    }

    fn to_percentage_string(&self) -> String {
        let pct = *self * Decimal::from(100);
        format!("{:.2}%", pct)
    }

    fn round_dp(&self, dp: u32) -> Decimal {
        self.round_dp_with_strategy(dp, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
    }
}

/// 통화가 포함된 금액.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    /// 금액
    pub amount: Decimal,
    /// 통화
    pub currency: String,
}

impl Money {
    /// 새 금액을 생성합니다.
    pub fn new(amount: Decimal, currency: impl Into<String>) -> Self {
        Self {
            amount,
            currency: currency.into().to_uppercase(),
        }
    }

    /// USDT 금액을 생성합니다.
    pub fn usdt(amount: Decimal) -> Self {
        Self::new(amount, "USDT")
    }

    /// USD 금액을 생성합니다.
    pub fn usd(amount: Decimal) -> Self {
        Self::new(amount, "USD")
    }

    /// KRW 금액을 생성합니다.
    pub fn krw(amount: Decimal) -> Self {
        Self::new(amount, "KRW")
    }
}

impl std::fmt::Display for Money {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.amount, self.currency)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_decimal_ext() {
        let d = dec!(0.0525);
        assert_eq!(d.to_percentage_string(), "5.25%");
    }

    #[test]
    fn test_money() {
        let m = Money::usdt(dec!(1000.50));
        assert_eq!(m.to_string(), "1000.50 USDT");
    }
}
