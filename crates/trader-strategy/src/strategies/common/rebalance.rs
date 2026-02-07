//! 포트폴리오 리밸런싱 계산기.
//!
//! 이 모듈은 포트폴리오 리밸런싱 주문 계산을 위한 유틸리티를 제공합니다.
//! 지원 기능:
//!
//! - 목표 비중 정규화
//! - 목표 배분 달성을 위한 주문 계산 (매수/매도)
//! - 최소 거래 금액 필터링
//! - 수수료 및 세금 고려
//!
//! # 예제
//!
//! ```rust,ignore
//! use trader_strategy::strategies::common::rebalance::*;
//! use rust_decimal_macros::dec;
//!
//! // 현재 포트폴리오 포지션
//! let positions = vec![
//!     PortfolioPosition::new("SPY", dec!(100), dec!(450.0)),
//!     PortfolioPosition::new("TLT", dec!(50), dec!(100.0)),
//!     PortfolioPosition::new("CASH", dec!(10000), dec!(1.0)),
//! ];
//!
//! // 목표 배분
//! let targets = vec![
//!     TargetAllocation::new("SPY", dec!(0.6)),
//!     TargetAllocation::new("TLT", dec!(0.4)),
//! ];
//!
//! let config = RebalanceConfig::default();
//! let calculator = RebalanceCalculator::new(config);
//! let result = calculator.calculate_orders(&positions, &targets);
//! ```

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 리밸런싱 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceConfig {
    /// 최소 거래 금액 (통화 단위).
    /// 이 임계값 미만의 주문은 필터링됩니다.
    pub min_trade_amount: Decimal,

    /// 거래 수수료율 (예: 0.001 = 0.1%).
    pub fee_rate: Decimal,

    /// 매도 세율 (예: 한국 주식의 경우 0.0015 = 0.15%).
    pub sell_tax_rate: Decimal,

    /// 슬리피지 허용치 (예: 0.001 = 0.1%).
    pub slippage_rate: Decimal,

    /// 리밸런싱 비중 임계값 (예: 0.05 = 5%).
    /// 비중 편차가 이 임계값을 초과할 때만 리밸런싱합니다.
    pub rebalance_threshold: Decimal,

    /// 현금 심볼 (예: "CASH", "KRW", "USD").
    pub cash_ticker: String,
}

impl Default for RebalanceConfig {
    fn default() -> Self {
        Self {
            min_trade_amount: dec!(10000),   // 10,000 KRW or $10
            fee_rate: dec!(0.00015),         // 0.015% (typical for Korean ETFs)
            sell_tax_rate: dec!(0.0),        // 0% for ETFs (varies by market)
            slippage_rate: dec!(0.001),      // 0.1%
            rebalance_threshold: dec!(0.03), // 3% deviation threshold
            cash_ticker: "CASH".to_string(),
        }
    }
}

impl RebalanceConfig {
    /// 한국 시장용 설정 생성.
    pub fn korean_market() -> Self {
        Self {
            min_trade_amount: dec!(10000), // 10,000 KRW
            fee_rate: dec!(0.00015),       // 0.015%
            sell_tax_rate: dec!(0.0),      // ETF의 경우 0%
            slippage_rate: dec!(0.001),    // 0.1%
            rebalance_threshold: dec!(0.03),
            cash_ticker: "KRW".to_string(),
        }
    }

    /// 미국 시장용 설정 생성.
    pub fn us_market() -> Self {
        Self {
            min_trade_amount: dec!(10), // $10
            fee_rate: dec!(0.0),        // 대부분의 브로커에서 수수료 무료
            sell_tax_rate: dec!(0.0),   // 거래세 없음
            slippage_rate: dec!(0.001), // 0.1%
            rebalance_threshold: dec!(0.03),
            cash_ticker: "USD".to_string(),
        }
    }
}

/// 현재 포트폴리오 포지션.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioPosition {
    /// 자산 심볼 (예: "SPY", "069500").
    pub ticker: String,

    /// 보유 수량.
    pub quantity: Decimal,

    /// 단위당 현재 가격.
    pub current_price: Decimal,

    /// 현재 시장 가치 (수량 * 가격).
    pub market_value: Decimal,
}

impl PortfolioPosition {
    /// 새 포트폴리오 포지션 생성.
    pub fn new(ticker: impl Into<String>, quantity: Decimal, current_price: Decimal) -> Self {
        let market_value = quantity * current_price;
        Self {
            ticker: ticker.into(),
            quantity,
            current_price,
            market_value,
        }
    }

    /// 현금 포지션 생성.
    pub fn cash(amount: Decimal, ticker: impl Into<String>) -> Self {
        Self {
            ticker: ticker.into(),
            quantity: amount,
            current_price: dec!(1),
            market_value: amount,
        }
    }
}

/// 자산에 대한 목표 배분.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetAllocation {
    /// 자산 심볼.
    pub ticker: String,

    /// 목표 비중 (0.0 ~ 1.0).
    pub weight: Decimal,
}

impl TargetAllocation {
    /// 새 목표 배분 생성.
    pub fn new(ticker: impl Into<String>, weight: Decimal) -> Self {
        Self {
            ticker: ticker.into(),
            weight,
        }
    }
}

/// 리밸런싱을 위한 주문 방향.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebalanceOrderSide {
    Buy,
    Sell,
}

/// 단일 리밸런싱 주문.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceOrder {
    /// 자산 ticker.
    pub ticker: String,

    /// 주문 방향 (매수 또는 매도).
    pub side: RebalanceOrderSide,

    /// 거래할 수량.
    pub quantity: Decimal,

    /// 예상 거래 금액 (수수료 제외).
    pub amount: Decimal,

    /// 예상 수수료.
    pub estimated_fee: Decimal,

    /// 예상 세금 (매도 주문의 경우).
    pub estimated_tax: Decimal,

    /// 리밸런싱 전 현재 비중.
    pub current_weight: Decimal,

    /// 리밸런싱 후 목표 비중.
    pub target_weight: Decimal,

    /// 비중 편차 (목표 - 현재).
    pub weight_deviation: Decimal,
}

/// 리밸런싱 계산 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceResult {
    /// 리밸런싱 전 총 포트폴리오 가치.
    pub total_portfolio_value: Decimal,

    /// 총 가용 현금.
    pub available_cash: Decimal,

    /// 실행할 주문 (정렬: 매도 먼저, 그 다음 매수).
    pub orders: Vec<RebalanceOrder>,

    /// 총 매수 금액.
    pub total_buy_amount: Decimal,

    /// 총 매도 금액.
    pub total_sell_amount: Decimal,

    /// 총 예상 수수료.
    pub total_fees: Decimal,

    /// 총 예상 세금.
    pub total_taxes: Decimal,

    /// 필터링된 주문 (최소 거래 금액 미만).
    pub filtered_orders: Vec<RebalanceOrder>,

    /// 임계값 기준 리밸런싱 필요 여부.
    pub rebalance_needed: bool,

    /// 포트폴리오의 최대 비중 편차.
    pub max_weight_deviation: Decimal,
}

impl RebalanceResult {
    /// 실행할 주문이 있는지 확인.
    pub fn has_orders(&self) -> bool {
        !self.orders.is_empty()
    }

    /// 매수 주문만 반환.
    pub fn buy_orders(&self) -> Vec<&RebalanceOrder> {
        self.orders
            .iter()
            .filter(|o| o.side == RebalanceOrderSide::Buy)
            .collect()
    }

    /// 매도 주문만 반환.
    pub fn sell_orders(&self) -> Vec<&RebalanceOrder> {
        self.orders
            .iter()
            .filter(|o| o.side == RebalanceOrderSide::Sell)
            .collect()
    }

    /// 순 현금 흐름 (양수 = 현금 증가, 음수 = 현금 감소).
    pub fn net_cash_flow(&self) -> Decimal {
        self.total_sell_amount - self.total_buy_amount - self.total_fees - self.total_taxes
    }
}

/// 포트폴리오 리밸런싱 계산기.
#[derive(Debug, Clone)]
pub struct RebalanceCalculator {
    config: RebalanceConfig,
}

impl RebalanceCalculator {
    /// 새 리밸런싱 계산기 생성.
    pub fn new(config: RebalanceConfig) -> Self {
        Self { config }
    }

    /// 기본 설정으로 계산기 생성.
    pub fn with_defaults() -> Self {
        Self::new(RebalanceConfig::default())
    }

    /// 목표 비중을 합계 1.0으로 정규화.
    pub fn normalize_weights(&self, targets: &[TargetAllocation]) -> Vec<TargetAllocation> {
        let total_weight: Decimal = targets.iter().map(|t| t.weight).sum();

        if total_weight.is_zero() {
            return targets.to_vec();
        }

        targets
            .iter()
            .map(|t| TargetAllocation {
                ticker: t.ticker.clone(),
                weight: t.weight / total_weight,
            })
            .collect()
    }

    /// 리밸런싱 주문 계산.
    ///
    /// # 인수
    ///
    /// * `positions` - 현재 포트폴리오 포지션
    /// * `targets` - 목표 배분 (정규화됨)
    ///
    /// # 반환값
    ///
    /// 실행할 주문이 포함된 리밸런싱 결과.
    pub fn calculate_orders(
        &self,
        positions: &[PortfolioPosition],
        targets: &[TargetAllocation],
    ) -> RebalanceResult {
        // Normalize target weights
        let normalized_targets = self.normalize_weights(targets);
        let target_map: HashMap<&str, Decimal> = normalized_targets
            .iter()
            .map(|t| (t.ticker.as_str(), t.weight))
            .collect();

        // Calculate total portfolio value
        let total_value: Decimal = positions.iter().map(|p| p.market_value).sum();

        // Find cash position
        let cash_position = positions
            .iter()
            .find(|p| p.ticker == self.config.cash_ticker);
        let available_cash = cash_position.map(|p| p.market_value).unwrap_or(dec!(0));

        // Build position map (excluding cash)
        let position_map: HashMap<&str, &PortfolioPosition> = positions
            .iter()
            .filter(|p| p.ticker != self.config.cash_ticker)
            .map(|p| (p.ticker.as_str(), p))
            .collect();

        // Calculate orders for each target
        let mut orders = Vec::new();
        let mut filtered_orders = Vec::new();
        let mut max_deviation = dec!(0);

        for target in &normalized_targets {
            // Skip cash in target allocation
            if target.ticker == self.config.cash_ticker {
                continue;
            }

            let current_value = position_map
                .get(target.ticker.as_str())
                .map(|p| p.market_value)
                .unwrap_or(dec!(0));

            let current_price = position_map
                .get(target.ticker.as_str())
                .map(|p| p.current_price)
                .unwrap_or(dec!(1));

            let current_weight = if total_value.is_zero() {
                dec!(0)
            } else {
                current_value / total_value
            };

            let target_value = total_value * target.weight;
            let value_diff = target_value - current_value;
            let weight_deviation = target.weight - current_weight;

            // Update max deviation
            if weight_deviation.abs() > max_deviation {
                max_deviation = weight_deviation.abs();
            }

            // Skip if no significant change needed
            if value_diff.abs() < self.config.min_trade_amount {
                continue;
            }

            let (side, amount) = if value_diff > dec!(0) {
                (RebalanceOrderSide::Buy, value_diff)
            } else {
                (RebalanceOrderSide::Sell, value_diff.abs())
            };

            // Calculate quantity (round down for buys, round up for sells)
            let quantity = if current_price.is_zero() {
                dec!(0)
            } else {
                let raw_qty = amount / current_price;
                match side {
                    RebalanceOrderSide::Buy => raw_qty.floor(),
                    RebalanceOrderSide::Sell => raw_qty.ceil(),
                }
            };

            // Skip if quantity is zero
            if quantity.is_zero() {
                continue;
            }

            // Recalculate amount based on rounded quantity
            let actual_amount = quantity * current_price;

            // Calculate fees and taxes
            let fee = actual_amount * self.config.fee_rate;
            let tax = if side == RebalanceOrderSide::Sell {
                actual_amount * self.config.sell_tax_rate
            } else {
                dec!(0)
            };

            let order = RebalanceOrder {
                ticker: target.ticker.clone(),
                side,
                quantity,
                amount: actual_amount,
                estimated_fee: fee,
                estimated_tax: tax,
                current_weight,
                target_weight: target.weight,
                weight_deviation,
            };

            // Filter orders below minimum trade amount
            if actual_amount < self.config.min_trade_amount {
                filtered_orders.push(order);
            } else {
                orders.push(order);
            }
        }

        // Check for positions not in target (should be sold)
        for (ticker, position) in &position_map {
            if !target_map.contains_key(*ticker)
                && position.market_value > self.config.min_trade_amount
            {
                let current_weight = if total_value.is_zero() {
                    dec!(0)
                } else {
                    position.market_value / total_value
                };

                let fee = position.market_value * self.config.fee_rate;
                let tax = position.market_value * self.config.sell_tax_rate;

                let order = RebalanceOrder {
                    ticker: ticker.to_string(),
                    side: RebalanceOrderSide::Sell,
                    quantity: position.quantity,
                    amount: position.market_value,
                    estimated_fee: fee,
                    estimated_tax: tax,
                    current_weight,
                    target_weight: dec!(0),
                    weight_deviation: -current_weight,
                };

                if position.market_value >= self.config.min_trade_amount {
                    orders.push(order);
                    if current_weight > max_deviation {
                        max_deviation = current_weight;
                    }
                } else {
                    filtered_orders.push(order);
                }
            }
        }

        // Sort orders: sells first (to free up cash), then buys
        orders.sort_by(|a, b| {
            match (&a.side, &b.side) {
                (RebalanceOrderSide::Sell, RebalanceOrderSide::Buy) => std::cmp::Ordering::Less,
                (RebalanceOrderSide::Buy, RebalanceOrderSide::Sell) => std::cmp::Ordering::Greater,
                _ => b.amount.cmp(&a.amount), // Larger amounts first within same side
            }
        });

        // Calculate totals
        let total_buy_amount: Decimal = orders
            .iter()
            .filter(|o| o.side == RebalanceOrderSide::Buy)
            .map(|o| o.amount)
            .sum();

        let total_sell_amount: Decimal = orders
            .iter()
            .filter(|o| o.side == RebalanceOrderSide::Sell)
            .map(|o| o.amount)
            .sum();

        let total_fees: Decimal = orders.iter().map(|o| o.estimated_fee).sum();
        let total_taxes: Decimal = orders.iter().map(|o| o.estimated_tax).sum();

        // Check if rebalancing is needed based on threshold
        let rebalance_needed = max_deviation > self.config.rebalance_threshold;

        RebalanceResult {
            total_portfolio_value: total_value,
            available_cash,
            orders,
            total_buy_amount,
            total_sell_amount,
            total_fees,
            total_taxes,
            filtered_orders,
            rebalance_needed,
            max_weight_deviation: max_deviation,
        }
    }

    /// 현금 제약 조건을 적용한 주문 계산.
    ///
    /// 이 버전은 매수 주문이 가용 현금과
    /// 매도 주문의 수익금을 초과하지 않도록 합니다.
    pub fn calculate_orders_with_cash_constraint(
        &self,
        positions: &[PortfolioPosition],
        targets: &[TargetAllocation],
    ) -> RebalanceResult {
        let mut result = self.calculate_orders(positions, targets);

        // Calculate available funds (cash + sell proceeds - costs)
        let sell_proceeds: Decimal = result
            .orders
            .iter()
            .filter(|o| o.side == RebalanceOrderSide::Sell)
            .map(|o| o.amount - o.estimated_fee - o.estimated_tax)
            .sum();

        let available_funds = result.available_cash + sell_proceeds;

        // Adjust buy orders if they exceed available funds
        let mut remaining_funds = available_funds;
        let mut adjusted_orders = Vec::new();
        let mut filtered = Vec::new();

        // Keep all sell orders
        for order in result
            .orders
            .iter()
            .filter(|o| o.side == RebalanceOrderSide::Sell)
        {
            adjusted_orders.push(order.clone());
        }

        // Process buy orders with cash constraint
        for order in result
            .orders
            .iter()
            .filter(|o| o.side == RebalanceOrderSide::Buy)
        {
            let cost = order.amount + order.estimated_fee;

            if cost <= remaining_funds {
                adjusted_orders.push(order.clone());
                remaining_funds -= cost;
            } else if remaining_funds > self.config.min_trade_amount {
                // Partial order with remaining funds
                let adjusted_amount = remaining_funds - (remaining_funds * self.config.fee_rate);
                let adjusted_qty = (adjusted_amount / (order.amount / order.quantity)).floor();

                if adjusted_qty > dec!(0) {
                    let actual_amount = adjusted_qty * (order.amount / order.quantity);
                    let mut adjusted_order = order.clone();
                    adjusted_order.quantity = adjusted_qty;
                    adjusted_order.amount = actual_amount;
                    adjusted_order.estimated_fee = actual_amount * self.config.fee_rate;
                    adjusted_orders.push(adjusted_order);
                    remaining_funds = dec!(0);
                } else {
                    filtered.push(order.clone());
                }
            } else {
                filtered.push(order.clone());
            }
        }

        // Recalculate totals
        result.orders = adjusted_orders;
        result.filtered_orders.extend(filtered);

        result.total_buy_amount = result
            .orders
            .iter()
            .filter(|o| o.side == RebalanceOrderSide::Buy)
            .map(|o| o.amount)
            .sum();

        result.total_fees = result.orders.iter().map(|o| o.estimated_fee).sum();

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_weights() {
        let calculator = RebalanceCalculator::with_defaults();
        let targets = vec![
            TargetAllocation::new("SPY", dec!(60)),
            TargetAllocation::new("TLT", dec!(40)),
        ];

        let normalized = calculator.normalize_weights(&targets);

        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0].weight, dec!(0.6));
        assert_eq!(normalized[1].weight, dec!(0.4));
    }

    #[test]
    fn test_calculate_orders_basic() {
        let config = RebalanceConfig {
            min_trade_amount: dec!(100),
            fee_rate: dec!(0.001),
            sell_tax_rate: dec!(0),
            slippage_rate: dec!(0),
            rebalance_threshold: dec!(0.03),
            cash_ticker: "CASH".to_string(),
        };
        let calculator = RebalanceCalculator::new(config);

        // Portfolio: 50% SPY, 50% cash -> Target: 60% SPY, 40% TLT
        let positions = vec![
            PortfolioPosition::new("SPY", dec!(100), dec!(50)), // $5,000
            PortfolioPosition::cash(dec!(5000), "CASH"),        // $5,000
        ];

        let targets = vec![
            TargetAllocation::new("SPY", dec!(0.6)), // 60%
            TargetAllocation::new("TLT", dec!(0.4)), // 40%
        ];

        let result = calculator.calculate_orders(&positions, &targets);

        assert_eq!(result.total_portfolio_value, dec!(10000));
        assert_eq!(result.available_cash, dec!(5000));
        assert!(result.has_orders());

        // Should have buy orders for SPY (need $1000 more) and TLT (need $4000)
        let buy_orders = result.buy_orders();
        assert_eq!(buy_orders.len(), 2);
    }

    #[test]
    fn test_rebalance_threshold() {
        let config = RebalanceConfig {
            min_trade_amount: dec!(100),
            rebalance_threshold: dec!(0.05), // 5%
            ..Default::default()
        };
        let calculator = RebalanceCalculator::new(config);

        // Portfolio already close to target
        let positions = vec![
            PortfolioPosition::new("SPY", dec!(58), dec!(100)), // 58%
            PortfolioPosition::new("TLT", dec!(42), dec!(100)), // 42%
        ];

        let targets = vec![
            TargetAllocation::new("SPY", dec!(0.6)),
            TargetAllocation::new("TLT", dec!(0.4)),
        ];

        let result = calculator.calculate_orders(&positions, &targets);

        // Max deviation is 2%, below 5% threshold
        assert!(!result.rebalance_needed);
    }

    #[test]
    fn test_sell_untracked_positions() {
        let config = RebalanceConfig {
            min_trade_amount: dec!(100),
            ..Default::default()
        };
        let calculator = RebalanceCalculator::new(config);

        // Has position not in target
        let positions = vec![
            PortfolioPosition::new("SPY", dec!(100), dec!(50)),
            PortfolioPosition::new("OLD_STOCK", dec!(50), dec!(100)), // Not in target
            PortfolioPosition::cash(dec!(0), "CASH"),
        ];

        let targets = vec![
            TargetAllocation::new("SPY", dec!(1.0)), // 100% SPY
        ];

        let result = calculator.calculate_orders(&positions, &targets);

        // Should sell OLD_STOCK
        let sell_orders = result.sell_orders();
        assert!(sell_orders.iter().any(|o| o.ticker == "OLD_STOCK"));
    }

    #[test]
    fn test_order_sorting() {
        let calculator = RebalanceCalculator::with_defaults();

        let positions = vec![
            PortfolioPosition::new("SELL_ME", dec!(100), dec!(100)),
            PortfolioPosition::new("SPY", dec!(10), dec!(100)),
            PortfolioPosition::cash(dec!(5000), "CASH"),
        ];

        let targets = vec![
            TargetAllocation::new("SPY", dec!(0.5)),
            TargetAllocation::new("TLT", dec!(0.5)),
        ];

        let result = calculator.calculate_orders(&positions, &targets);

        // Sell orders should come before buy orders
        if result.orders.len() >= 2 {
            let first_is_sell = result
                .orders
                .first()
                .map(|o| o.side == RebalanceOrderSide::Sell)
                .unwrap_or(false);
            assert!(first_is_sell || result.sell_orders().is_empty());
        }
    }
}
