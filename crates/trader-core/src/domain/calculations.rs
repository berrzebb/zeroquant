//! 매매 손익 및 비용 계산 공통 로직.
//!
//! Journal과 Backtest에서 공유하는 P&L 계산 함수를 제공합니다.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use super::order::Side;
use crate::types::Quantity;

/// 비용 기준 계산 방법.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CostMethod {
    /// FIFO (First In, First Out) - 선입선출
    Fifo,
    /// 가중평균 매입가
    WeightedAverage,
    /// 최종 평가 (현재가 기준)
    LastPrice,
}

/// 거래 진입 정보 (비용 기준 계산용).
#[derive(Debug, Clone)]
pub struct TradeEntry {
    /// 진입 가격
    pub price: Decimal,
    /// 진입 수량
    pub quantity: Quantity,
    /// 진입 시각
    pub timestamp: DateTime<Utc>,
}

impl TradeEntry {
    /// 새로운 진입 정보 생성.
    pub fn new(price: Decimal, quantity: Quantity, timestamp: DateTime<Utc>) -> Self {
        Self {
            price,
            quantity,
            timestamp,
        }
    }

    /// 명목 가치 계산 (가격 × 수량).
    pub fn notional(&self) -> Decimal {
        self.price * self.quantity
    }
}

/// 비용 기준 계산.
///
/// 여러 진입 거래의 평균 매입가를 계산합니다.
///
/// # Arguments
///
/// * `entries` - 진입 거래 목록
/// * `method` - 계산 방법 (FIFO, 가중평균 등)
///
/// # Returns
///
/// 계산된 비용 기준 (평균 매입가)
///
/// # Examples
///
/// ```ignore
/// use trader_core::domain::{calculations::*, Quantity};
/// use rust_decimal_macros::dec;
///
/// let entries = vec![
///     TradeEntry::new(dec!(100), dec!(10)), Utc::now()),
///     TradeEntry::new(dec!(110), dec!(5)), Utc::now()),
/// ];
///
/// let avg_price = cost_basis(&entries, CostMethod::WeightedAverage);
/// // (100*10 + 110*5) / (10+5) = 103.33
/// ```
pub fn cost_basis(entries: &[TradeEntry], method: CostMethod) -> Decimal {
    if entries.is_empty() {
        return Decimal::ZERO;
    }

    match method {
        CostMethod::Fifo => {
            // FIFO: 가장 오래된 진입의 가격
            entries[0].price
        }
        CostMethod::WeightedAverage => {
            // 가중평균: (Σ 가격×수량) / Σ수량
            let total_cost: Decimal = entries.iter().map(|e| e.notional()).sum();
            let total_qty: Decimal = entries.iter().map(|e| e.quantity).sum();

            if total_qty > Decimal::ZERO {
                total_cost / total_qty
            } else {
                Decimal::ZERO
            }
        }
        CostMethod::LastPrice => {
            // 최종 가격: 가장 최근 진입의 가격
            entries.last().map(|e| e.price).unwrap_or(Decimal::ZERO)
        }
    }
}

/// 실현 손익 계산 (수수료 제외).
///
/// 진입가와 청산가의 차이로 손익을 계산합니다.
///
/// # Arguments
///
/// * `entry_price` - 진입 가격
/// * `exit_price` - 청산 가격
/// * `quantity` - 거래 수량
/// * `side` - 포지션 방향 (Buy=롱, Sell=숏)
///
/// # Returns
///
/// 실현 손익 (수수료 제외)
///
/// # Examples
///
/// ```ignore
/// use trader_core::domain::{calculations::*, Side, Quantity};
/// use rust_decimal_macros::dec;
///
/// // 롱 포지션: 100에 매수 → 110에 매도, 수량 10
/// let pnl = realized_pnl(dec!(100), dec!(110), dec!(10)), Side::Buy);
/// assert_eq!(pnl, dec!(100));  // (110-100) * 10 = 100
///
/// // 숏 포지션: 110에 매도 → 100에 매수, 수량 10
/// let pnl = realized_pnl(dec!(110), dec!(100), dec!(10)), Side::Sell);
/// assert_eq!(pnl, dec!(100));  // (110-100) * 10 = 100
/// ```
pub fn realized_pnl(
    entry_price: Decimal,
    exit_price: Decimal,
    quantity: Quantity,
    side: Side,
) -> Decimal {
    match side {
        Side::Buy => {
            // 롱 포지션: (청산가 - 진입가) × 수량
            (exit_price - entry_price) * quantity
        }
        Side::Sell => {
            // 숏 포지션: (진입가 - 청산가) × 수량
            (entry_price - exit_price) * quantity
        }
    }
}

/// 수수료 차감 후 순손익 계산.
///
/// # Arguments
///
/// * `gross_pnl` - 총손익 (수수료 제외)
/// * `fees` - 진입 + 청산 수수료 합계
///
/// # Returns
///
/// 순손익 (수수료 차감 후)
pub fn net_pnl(gross_pnl: Decimal, fees: Decimal) -> Decimal {
    gross_pnl - fees
}

/// 수익률 계산 (백분율).
///
/// # Arguments
///
/// * `pnl` - 손익 (수수료 차감 후)
/// * `cost_basis` - 비용 기준 (진입 시 투입 자본)
///
/// # Returns
///
/// 수익률 (백분율, 예: 10.5 = 10.5%)
///
/// # Examples
///
/// ```ignore
/// use trader_core::domain::calculations::*;
/// use rust_decimal_macros::dec;
///
/// let pnl = dec!(50);        // 50 수익
/// let cost = dec!(1000);     // 1000 투입
/// let return_pct = return_pct(pnl, cost);
/// assert_eq!(return_pct, dec!(5));  // 5% 수익
/// ```
pub fn return_pct(pnl: Decimal, cost_basis: Decimal) -> Decimal {
    if cost_basis > Decimal::ZERO {
        (pnl / cost_basis) * dec!(100)
    } else {
        Decimal::ZERO
    }
}

/// 미실현 손익 계산.
///
/// 현재 보유 중인 포지션의 평가 손익을 계산합니다.
///
/// # Arguments
///
/// * `entry_price` - 진입 가격 (평균 매입가)
/// * `current_price` - 현재 시장 가격
/// * `quantity` - 보유 수량
/// * `side` - 포지션 방향
///
/// # Returns
///
/// 미실현 손익 (현재 시점 평가)
///
/// # Examples
///
/// ```ignore
/// use trader_core::domain::{calculations::*, Side, Quantity};
/// use rust_decimal_macros::dec;
///
/// // 롱 포지션: 100에 매수, 현재가 105, 수량 10
/// let unrealized = unrealized_pnl(dec!(100), dec!(105), dec!(10)), Side::Buy);
/// assert_eq!(unrealized, dec!(50));  // (105-100) * 10 = 50
/// ```
pub fn unrealized_pnl(
    entry_price: Decimal,
    current_price: Decimal,
    quantity: Quantity,
    side: Side,
) -> Decimal {
    match side {
        Side::Buy => (current_price - entry_price) * quantity,
        Side::Sell => (entry_price - current_price) * quantity,
    }
}

/// 명목 가치 계산 (포지션 크기).
///
/// # Arguments
///
/// * `price` - 가격
/// * `quantity` - 수량
///
/// # Returns
///
/// 명목 가치 (가격 × 수량)
pub fn notional_value(price: Decimal, quantity: Quantity) -> Decimal {
    price * quantity
}

/// 평균 진입가 계산 (여러 부분 진입).
///
/// # Arguments
///
/// * `entries` - 진입 거래 목록
///
/// # Returns
///
/// 가중평균 진입가
pub fn average_entry_price(entries: &[TradeEntry]) -> Decimal {
    cost_basis(entries, CostMethod::WeightedAverage)
}

/// 총 진입 비용 계산.
///
/// # Arguments
///
/// * `entries` - 진입 거래 목록
/// * `fees` - 진입 수수료 합계
///
/// # Returns
///
/// 총 비용 (진입 금액 + 수수료)
pub fn total_entry_cost(entries: &[TradeEntry], fees: Decimal) -> Decimal {
    let notional: Decimal = entries.iter().map(|e| e.notional()).sum();
    notional + fees
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_cost_basis_weighted_average() {
        let entries = vec![
            TradeEntry::new(dec!(100), dec!(10), Utc::now()),
            TradeEntry::new(dec!(110), dec!(5), Utc::now()),
        ];

        let avg = cost_basis(&entries, CostMethod::WeightedAverage);
        // (100*10 + 110*5) / 15 = 1550 / 15 = 103.333...
        let expected = dec!(1550) / dec!(15);
        assert!((avg - expected).abs() < dec!(0.0001));
    }

    #[test]
    fn test_cost_basis_fifo() {
        let entries = vec![
            TradeEntry::new(dec!(100), dec!(10), Utc::now()),
            TradeEntry::new(dec!(110), dec!(5), Utc::now()),
        ];

        let fifo = cost_basis(&entries, CostMethod::Fifo);
        assert_eq!(fifo, dec!(100));
    }

    #[test]
    fn test_cost_basis_last_price() {
        let entries = vec![
            TradeEntry::new(dec!(100), dec!(10), Utc::now()),
            TradeEntry::new(dec!(110), dec!(5), Utc::now()),
        ];

        let last = cost_basis(&entries, CostMethod::LastPrice);
        assert_eq!(last, dec!(110));
    }

    #[test]
    fn test_realized_pnl_long() {
        let pnl = realized_pnl(dec!(100), dec!(110), dec!(10), Side::Buy);
        assert_eq!(pnl, dec!(100));
    }

    #[test]
    fn test_realized_pnl_short() {
        let pnl = realized_pnl(dec!(110), dec!(100), dec!(10), Side::Sell);
        assert_eq!(pnl, dec!(100));
    }

    #[test]
    fn test_net_pnl() {
        let gross = dec!(100);
        let fees = dec!(5);
        let net = net_pnl(gross, fees);
        assert_eq!(net, dec!(95));
    }

    #[test]
    fn test_return_pct() {
        let pnl = dec!(50);
        let cost = dec!(1000);
        let ret = return_pct(pnl, cost);
        assert_eq!(ret, dec!(5)); // 5%
    }

    #[test]
    fn test_unrealized_pnl_long() {
        let unrealized = unrealized_pnl(dec!(100), dec!(105), dec!(10), Side::Buy);
        assert_eq!(unrealized, dec!(50));
    }

    #[test]
    fn test_unrealized_pnl_short() {
        let unrealized = unrealized_pnl(dec!(110), dec!(105), dec!(10), Side::Sell);
        assert_eq!(unrealized, dec!(50));
    }

    #[test]
    fn test_notional_value() {
        let notional = notional_value(dec!(100), dec!(10));
        assert_eq!(notional, dec!(1000));
    }

    #[test]
    fn test_total_entry_cost() {
        let entries = vec![
            TradeEntry::new(dec!(100), dec!(10), Utc::now()),
            TradeEntry::new(dec!(110), dec!(5), Utc::now()),
        ];
        let fees = dec!(10);

        let total = total_entry_cost(&entries, fees);
        // (100*10 + 110*5) + 10 = 1550 + 10 = 1560
        assert_eq!(total, dec!(1560));
    }
}
