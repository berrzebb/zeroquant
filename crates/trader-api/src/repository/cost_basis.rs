//! 비용 기준(Cost Basis) 및 실현 손익 계산 모듈.
//!
//! 거래소 중립적으로 포지션의 평균 매입가와 FIFO 기반 실현 손익을 계산합니다.
//!
//! # 주요 기능
//!
//! - **가중평균 매입가**: 물타기(추가 매수) 시 자동 계산
//! - **FIFO 실현손익**: 선입선출 방식으로 매도 시 실현 손익 계산
//! - **로트(Lot) 추적**: 개별 매수 건별 추적으로 정확한 비용 기준 관리
//!
//! # 예시
//!
//! ```ignore
//! let mut tracker = CostBasisTracker::new();
//!
//! // 100주 @ $50 매수
//! tracker.add_lot(lot1);
//!
//! // 50주 @ $45 추가 매수 (물타기)
//! tracker.add_lot(lot2);
//!
//! // 평균 매입가: (100*50 + 50*45) / 150 = $48.33
//! assert_eq!(tracker.average_cost(), dec!(48.33));
//!
//! // 80주 @ $55 매도 (FIFO 기준 실현손익 계산)
//! let result = tracker.sell(80, dec!(55));
//! // 실현손익: 80 * (55 - 50) = $400
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use trader_core::{Side, TradeInfo};
use uuid::Uuid;

/// 매수 로트(Lot).
///
/// 개별 매수 건을 추적하여 FIFO 계산에 사용합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lot {
    /// 로트 ID (추적용)
    pub id: Uuid,
    /// 매수 수량 (남은 수량)
    pub quantity: Decimal,
    /// 매수 가격
    pub price: Decimal,
    /// 수수료 (비용에 포함)
    pub fee: Decimal,
    /// 매수 시간
    pub acquired_at: DateTime<Utc>,
    /// 원본 매수 수량 (일부 매도 후에도 원본 추적)
    pub original_quantity: Decimal,
    /// 관련 체결 ID
    pub execution_id: Option<Uuid>,
}

impl Lot {
    /// 새 로트 생성.
    pub fn new(
        quantity: Decimal,
        price: Decimal,
        fee: Decimal,
        acquired_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            quantity,
            price,
            fee,
            acquired_at,
            original_quantity: quantity,
            execution_id: None,
        }
    }

    /// 체결 ID 설정.
    pub fn with_execution_id(mut self, execution_id: Uuid) -> Self {
        self.execution_id = Some(execution_id);
        self
    }

    /// 로트의 총 비용 (수량 * 가격 + 수수료).
    pub fn total_cost(&self) -> Decimal {
        self.quantity * self.price + self.fee
    }

    /// 수량당 비용 (수수료 포함).
    pub fn cost_per_unit(&self) -> Decimal {
        if self.quantity > Decimal::ZERO {
            (self.quantity * self.price + self.fee) / self.quantity
        } else {
            self.price
        }
    }
}

/// FIFO 매도 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FifoSaleResult {
    /// 매도 수량
    pub quantity_sold: Decimal,
    /// 매도 가격
    pub sale_price: Decimal,
    /// 매도 금액
    pub proceeds: Decimal,
    /// 비용 기준 (FIFO 기반)
    pub cost_basis: Decimal,
    /// 실현 손익
    pub realized_pnl: Decimal,
    /// 실현 손익률 (%)
    pub realized_pnl_pct: Decimal,
    /// 사용된 로트 정보
    pub lots_used: Vec<LotUsage>,
}

/// 매도에 사용된 로트 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LotUsage {
    /// 로트 ID
    pub lot_id: Uuid,
    /// 사용 수량
    pub quantity_used: Decimal,
    /// 해당 로트의 매수 가격
    pub purchase_price: Decimal,
    /// 보유 기간 (일)
    pub holding_days: i64,
    /// 개별 손익
    pub pnl: Decimal,
}

/// 비용 기준 추적기.
///
/// FIFO 방식으로 로트를 관리하고 가중평균 매입가를 계산합니다.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostBasisTracker {
    /// 심볼
    pub symbol: String,
    /// 로트 큐 (FIFO 순서)
    lots: VecDeque<Lot>,
    /// 누적 실현 손익
    pub total_realized_pnl: Decimal,
    /// 총 실현 매도 건수
    pub total_sales: u32,
    /// 누적 수수료
    pub total_fees: Decimal,
}

impl CostBasisTracker {
    /// 새 추적기 생성.
    pub fn new(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            lots: VecDeque::new(),
            total_realized_pnl: Decimal::ZERO,
            total_sales: 0,
            total_fees: Decimal::ZERO,
        }
    }

    /// 매수 로트 추가 (물타기).
    ///
    /// 가중평균 매입가가 자동으로 재계산됩니다.
    pub fn add_lot(&mut self, lot: Lot) {
        self.total_fees += lot.fee;
        self.lots.push_back(lot);
    }

    /// 간편 매수 추가.
    pub fn buy(
        &mut self,
        quantity: Decimal,
        price: Decimal,
        fee: Decimal,
        acquired_at: DateTime<Utc>,
    ) {
        let lot = Lot::new(quantity, price, fee, acquired_at);
        self.add_lot(lot);
    }

    /// FIFO 기반 매도.
    ///
    /// 선입선출 방식으로 가장 오래된 로트부터 매도 처리합니다.
    ///
    /// # Returns
    /// - `Ok(FifoSaleResult)`: 매도 성공 시 실현 손익 정보
    /// - `Err(String)`: 보유 수량 부족 시 에러
    pub fn sell(
        &mut self,
        quantity: Decimal,
        sale_price: Decimal,
        sale_fee: Decimal,
        sold_at: DateTime<Utc>,
    ) -> Result<FifoSaleResult, String> {
        let current_qty = self.total_quantity();
        if quantity > current_qty {
            return Err(format!(
                "보유 수량 부족: 매도 요청 {}, 보유 {}",
                quantity, current_qty
            ));
        }

        let mut remaining = quantity;
        let mut cost_basis = Decimal::ZERO;
        let mut lots_used = Vec::new();

        while remaining > Decimal::ZERO {
            let lot = self.lots.front_mut().ok_or("로트가 없습니다")?;

            let used_qty = remaining.min(lot.quantity);
            let lot_cost = used_qty * lot.price;

            // 수수료 비례 배분
            let fee_portion = if lot.original_quantity > Decimal::ZERO {
                lot.fee * (used_qty / lot.original_quantity)
            } else {
                Decimal::ZERO
            };

            cost_basis += lot_cost + fee_portion;

            let holding_days = (sold_at - lot.acquired_at).num_days();
            let pnl = used_qty * (sale_price - lot.price) - fee_portion;

            lots_used.push(LotUsage {
                lot_id: lot.id,
                quantity_used: used_qty,
                purchase_price: lot.price,
                holding_days,
                pnl,
            });

            lot.quantity -= used_qty;
            remaining -= used_qty;

            // 로트가 소진되면 제거
            if lot.quantity <= Decimal::ZERO {
                self.lots.pop_front();
            }
        }

        let proceeds = quantity * sale_price - sale_fee;
        let realized_pnl = proceeds - cost_basis;
        let realized_pnl_pct = if cost_basis > Decimal::ZERO {
            (realized_pnl / cost_basis) * dec!(100)
        } else {
            Decimal::ZERO
        };

        self.total_realized_pnl += realized_pnl;
        self.total_sales += 1;
        self.total_fees += sale_fee;

        Ok(FifoSaleResult {
            quantity_sold: quantity,
            sale_price,
            proceeds,
            cost_basis,
            realized_pnl,
            realized_pnl_pct,
            lots_used,
        })
    }

    /// 현재 총 보유 수량.
    pub fn total_quantity(&self) -> Decimal {
        self.lots.iter().map(|l| l.quantity).sum()
    }

    /// 현재 총 비용 기준.
    pub fn total_cost_basis(&self) -> Decimal {
        self.lots.iter().map(|l| l.total_cost()).sum()
    }

    /// 가중평균 매입가 (물타기 반영).
    ///
    /// `(총 비용) / (총 수량)` 으로 계산합니다.
    pub fn average_cost(&self) -> Decimal {
        let total_qty = self.total_quantity();
        if total_qty > Decimal::ZERO {
            self.total_cost_basis() / total_qty
        } else {
            Decimal::ZERO
        }
    }

    /// 수수료 제외 가중평균 매입가.
    pub fn average_price(&self) -> Decimal {
        let total_qty = self.total_quantity();
        if total_qty > Decimal::ZERO {
            let total_value: Decimal = self.lots.iter().map(|l| l.quantity * l.price).sum();
            total_value / total_qty
        } else {
            Decimal::ZERO
        }
    }

    /// 미실현 손익 계산.
    pub fn unrealized_pnl(&self, current_price: Decimal) -> Decimal {
        let total_qty = self.total_quantity();
        let market_value = total_qty * current_price;
        market_value - self.total_cost_basis()
    }

    /// 미실현 손익률 (%).
    pub fn unrealized_pnl_pct(&self, current_price: Decimal) -> Decimal {
        let cost_basis = self.total_cost_basis();
        if cost_basis > Decimal::ZERO {
            (self.unrealized_pnl(current_price) / cost_basis) * dec!(100)
        } else {
            Decimal::ZERO
        }
    }

    /// 로트 개수.
    pub fn lot_count(&self) -> usize {
        self.lots.len()
    }

    /// 모든 로트 조회 (불변 참조).
    pub fn lots(&self) -> &VecDeque<Lot> {
        &self.lots
    }

    /// 가장 오래된 로트 조회.
    pub fn oldest_lot(&self) -> Option<&Lot> {
        self.lots.front()
    }

    /// 가장 최근 로트 조회.
    pub fn newest_lot(&self) -> Option<&Lot> {
        self.lots.back()
    }

    /// 평균 보유 기간 (일).
    pub fn average_holding_days(&self, as_of: DateTime<Utc>) -> f64 {
        if self.lots.is_empty() {
            return 0.0;
        }

        let total_qty = self.total_quantity();
        if total_qty <= Decimal::ZERO {
            return 0.0;
        }

        let weighted_days: Decimal = self
            .lots
            .iter()
            .map(|l| {
                let days = (as_of - l.acquired_at).num_days();
                l.quantity * Decimal::from(days)
            })
            .sum();

        (weighted_days / total_qty)
            .to_string()
            .parse()
            .unwrap_or(0.0)
    }

    /// 포지션 클리어 (전량 매도 후 리셋).
    pub fn clear(&mut self) {
        self.lots.clear();
    }

    /// 상태 요약.
    pub fn summary(&self, current_price: Option<Decimal>) -> CostBasisSummary {
        let total_quantity = self.total_quantity();
        let average_cost = self.average_cost();
        let average_price = self.average_price();
        let total_cost_basis = self.total_cost_basis();

        let (market_value, unrealized_pnl, unrealized_pnl_pct) = match current_price {
            Some(price) => {
                let mv = total_quantity * price;
                let upnl = mv - total_cost_basis;
                let upnl_pct = if total_cost_basis > Decimal::ZERO {
                    (upnl / total_cost_basis) * dec!(100)
                } else {
                    Decimal::ZERO
                };
                (Some(mv), Some(upnl), Some(upnl_pct))
            }
            None => (None, None, None),
        };

        CostBasisSummary {
            symbol: self.symbol.clone(),
            total_quantity,
            average_cost,
            average_price,
            total_cost_basis,
            market_value,
            unrealized_pnl,
            unrealized_pnl_pct,
            total_realized_pnl: self.total_realized_pnl,
            total_sales: self.total_sales,
            total_fees: self.total_fees,
            lot_count: self.lots.len(),
        }
    }
}

/// 비용 기준 요약.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBasisSummary {
    /// 심볼
    pub symbol: String,
    /// 총 보유 수량
    pub total_quantity: Decimal,
    /// 가중평균 매입가 (수수료 포함)
    pub average_cost: Decimal,
    /// 가중평균 매입가 (수수료 제외)
    pub average_price: Decimal,
    /// 총 비용 기준
    pub total_cost_basis: Decimal,
    /// 시장 가치 (현재가 기준)
    pub market_value: Option<Decimal>,
    /// 미실현 손익
    pub unrealized_pnl: Option<Decimal>,
    /// 미실현 손익률 (%)
    pub unrealized_pnl_pct: Option<Decimal>,
    /// 누적 실현 손익
    pub total_realized_pnl: Decimal,
    /// 총 매도 건수
    pub total_sales: u32,
    /// 총 수수료
    pub total_fees: Decimal,
    /// 로트 개수
    pub lot_count: usize,
}

/// 체결 내역에서 비용 기준 추적기 생성.
///
/// 거래소에서 가져온 체결 내역을 분석하여 현재 포지션의 비용 기준을 계산합니다.
#[derive(Debug, Clone)]
pub struct TradeExecution {
    pub id: Uuid,
    pub symbol: String,
    pub side: Side,
    pub quantity: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
    pub executed_at: DateTime<Utc>,
}

impl TradeInfo for TradeExecution {
    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn pnl(&self) -> Option<Decimal> {
        // TradeExecution은 단일 체결 내역이므로 pnl이 없음.
        // 실현 손익은 매도 시 CostBasisTracker가 계산함.
        None
    }

    fn fees(&self) -> Decimal {
        self.fee
    }

    fn entry_time(&self) -> DateTime<Utc> {
        self.executed_at
    }

    fn exit_time(&self) -> Option<DateTime<Utc>> {
        // TradeExecution은 단일 체결이므로 청산 시각이 없음.
        // 매도 체결도 진입으로 간주 (FIFO 계산용).
        None
    }
}

/// 체결 내역으로부터 비용 기준 추적기 빌드.
pub fn build_tracker_from_executions(
    symbol: &str,
    executions: Vec<TradeExecution>,
) -> CostBasisTracker {
    let mut tracker = CostBasisTracker::new(symbol);

    // 시간순 정렬
    let mut sorted_executions = executions;
    sorted_executions.sort_by_key(|e| e.executed_at);

    for exec in sorted_executions {
        match exec.side {
            Side::Buy => {
                let lot = Lot::new(exec.quantity, exec.price, exec.fee, exec.executed_at)
                    .with_execution_id(exec.id);
                tracker.add_lot(lot);
            }
            Side::Sell => {
                // 매도 시 FIFO 처리 (실패해도 계속 진행)
                let _ = tracker.sell(exec.quantity, exec.price, exec.fee, exec.executed_at);
            }
        }
    }

    tracker
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    #[test]
    fn test_single_lot() {
        let mut tracker = CostBasisTracker::new("AAPL");
        tracker.buy(dec!(100), dec!(150.00), dec!(10.00), now());

        assert_eq!(tracker.total_quantity(), dec!(100));
        assert_eq!(tracker.average_price(), dec!(150.00));
        // 평균 비용: (100 * 150 + 10) / 100 = 150.10
        assert_eq!(tracker.average_cost(), dec!(150.10));
    }

    #[test]
    fn test_averaging_down() {
        let mut tracker = CostBasisTracker::new("AAPL");

        // 첫 매수: 100주 @ $50
        tracker.buy(dec!(100), dec!(50.00), dec!(0), now());

        // 물타기: 50주 @ $40
        tracker.buy(dec!(50), dec!(40.00), dec!(0), now());

        // 총 150주
        assert_eq!(tracker.total_quantity(), dec!(150));

        // 가중평균: (100*50 + 50*40) / 150 = 7000/150 = 46.666...
        let avg = tracker.average_price();
        assert!(avg > dec!(46.66) && avg < dec!(46.67));
    }

    #[test]
    fn test_fifo_sale() {
        let mut tracker = CostBasisTracker::new("AAPL");

        // 첫 매수: 100주 @ $50
        let t1 = Utc::now() - chrono::Duration::days(30);
        tracker.buy(dec!(100), dec!(50.00), dec!(0), t1);

        // 두 번째 매수: 50주 @ $60
        let t2 = Utc::now() - chrono::Duration::days(10);
        tracker.buy(dec!(50), dec!(60.00), dec!(0), t2);

        // 80주 매도 @ $70 (FIFO: 첫 로트에서 80주)
        let sale_time = Utc::now();
        let result = tracker
            .sell(dec!(80), dec!(70.00), dec!(5.00), sale_time)
            .unwrap();

        // 비용기준: 80 * 50 = 4000
        assert_eq!(result.cost_basis, dec!(4000));
        // 수익금: 80 * 70 - 5 = 5595
        assert_eq!(result.proceeds, dec!(5595));
        // 실현손익: 5595 - 4000 = 1595
        assert_eq!(result.realized_pnl, dec!(1595));

        // 남은 수량: 20(첫 로트) + 50(두 번째 로트) = 70
        assert_eq!(tracker.total_quantity(), dec!(70));
    }

    #[test]
    fn test_fifo_multiple_lots() {
        let mut tracker = CostBasisTracker::new("TSLA");

        // 로트1: 50주 @ $100
        let t1 = Utc::now() - chrono::Duration::days(60);
        tracker.buy(dec!(50), dec!(100.00), dec!(0), t1);

        // 로트2: 30주 @ $120
        let t2 = Utc::now() - chrono::Duration::days(30);
        tracker.buy(dec!(30), dec!(120.00), dec!(0), t2);

        // 로트3: 20주 @ $110
        let t3 = Utc::now() - chrono::Duration::days(10);
        tracker.buy(dec!(20), dec!(110.00), dec!(0), t3);

        // 70주 매도 (로트1 전체 + 로트2의 20주 사용)
        let result = tracker
            .sell(dec!(70), dec!(130.00), dec!(0), Utc::now())
            .unwrap();

        // 비용기준: 50*100 + 20*120 = 5000 + 2400 = 7400
        assert_eq!(result.cost_basis, dec!(7400));

        // 로트 사용 확인
        assert_eq!(result.lots_used.len(), 2);
        assert_eq!(result.lots_used[0].quantity_used, dec!(50)); // 로트1 전체
        assert_eq!(result.lots_used[1].quantity_used, dec!(20)); // 로트2 일부

        // 남은 수량: 10(로트2) + 20(로트3) = 30
        assert_eq!(tracker.total_quantity(), dec!(30));
    }

    #[test]
    fn test_insufficient_quantity() {
        let mut tracker = CostBasisTracker::new("AAPL");
        tracker.buy(dec!(50), dec!(100.00), dec!(0), now());

        let result = tracker.sell(dec!(100), dec!(110.00), dec!(0), now());
        assert!(result.is_err());
    }

    #[test]
    fn test_unrealized_pnl() {
        let mut tracker = CostBasisTracker::new("AAPL");
        tracker.buy(dec!(100), dec!(50.00), dec!(10.00), now());

        // 시장가 $60일 때 미실현 손익
        let current_price = dec!(60.00);
        let unrealized = tracker.unrealized_pnl(current_price);

        // 시장가치: 100 * 60 = 6000
        // 비용기준: 100 * 50 + 10 = 5010
        // 미실현손익: 6000 - 5010 = 990
        assert_eq!(unrealized, dec!(990));
    }

    #[test]
    fn test_build_from_executions() {
        let executions = vec![
            TradeExecution {
                id: Uuid::new_v4(),
                symbol: "AAPL".to_string(),
                side: Side::Buy,
                quantity: dec!(100),
                price: dec!(150.00),
                fee: dec!(5.00),
                executed_at: Utc::now() - chrono::Duration::days(30),
            },
            TradeExecution {
                id: Uuid::new_v4(),
                symbol: "AAPL".to_string(),
                side: Side::Buy,
                quantity: dec!(50),
                price: dec!(140.00),
                fee: dec!(5.00),
                executed_at: Utc::now() - chrono::Duration::days(20),
            },
            TradeExecution {
                id: Uuid::new_v4(),
                symbol: "AAPL".to_string(),
                side: Side::Sell,
                quantity: dec!(80),
                price: dec!(160.00),
                fee: dec!(8.00),
                executed_at: Utc::now() - chrono::Duration::days(10),
            },
        ];

        let tracker = build_tracker_from_executions("AAPL", executions);

        // 남은 수량: 100 + 50 - 80 = 70
        assert_eq!(tracker.total_quantity(), dec!(70));
        // 실현손익 발생
        assert!(tracker.total_realized_pnl != Decimal::ZERO);
    }

    #[test]
    fn test_summary() {
        let mut tracker = CostBasisTracker::new("AAPL");
        tracker.buy(dec!(100), dec!(150.00), dec!(10.00), now());
        tracker.buy(dec!(50), dec!(140.00), dec!(5.00), now());

        let summary = tracker.summary(Some(dec!(160.00)));

        assert_eq!(summary.total_quantity, dec!(150));
        assert_eq!(summary.lot_count, 2);
        assert!(summary.market_value.is_some());
        assert!(summary.unrealized_pnl.is_some());
    }
}
