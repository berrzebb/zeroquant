//! 전략 포지션 상태 동기화 유틸리티.
//!
//! 거래소로부터 주문 체결 정보를 받아 전략의 내부 포지션 상태를 동기화합니다.
//! 거래소 중립적으로 설계되어 모든 거래소에서 동일하게 작동합니다.
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! use trader_strategy::strategies::common::position_sync::{PositionSync, SyncedPosition};
//!
//! struct MyStrategy {
//!     position_sync: PositionSync,
//!     // ... 기타 필드
//! }
//!
//! impl Strategy for MyStrategy {
//!     async fn on_order_filled(&mut self, order: &Order) -> Result<...> {
//!         let result = self.position_sync.process_fill(order);
//!         match result {
//!             FillResult::PositionOpened { position } => { /* 신규 진입 */ }
//!             FillResult::PositionClosed { pnl, .. } => { /* 포지션 종료 */ }
//!             FillResult::PositionUpdated { position } => { /* 포지션 업데이트 */ }
//!             FillResult::NoChange => { /* 변화 없음 */ }
//!         }
//!         Ok(())
//!     }
//! }
//! ```

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use trader_core::{unrealized_pnl, Order, OrderStatusType, Side};

/// 동기화된 포지션 상태.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncedPosition {
    /// 포지션 심볼.
    pub ticker: String,
    /// 포지션 방향.
    pub side: Side,
    /// 현재 수량.
    pub quantity: Decimal,
    /// 평균 진입가.
    pub entry_price: Decimal,
    /// 손절가 (전략에서 설정).
    pub stop_loss: Option<Decimal>,
    /// 익절가 (전략에서 설정).
    pub take_profit: Option<Decimal>,
    /// 실현 손익 (청산된 부분).
    pub realized_pnl: Decimal,
    /// 총 진입 비용 (수량 * 평균가).
    pub total_cost: Decimal,
}

impl SyncedPosition {
    /// 새 포지션 생성.
    pub fn new(ticker: String, side: Side, quantity: Decimal, entry_price: Decimal) -> Self {
        Self {
            ticker,
            side,
            quantity,
            entry_price,
            stop_loss: None,
            take_profit: None,
            realized_pnl: Decimal::ZERO,
            total_cost: quantity * entry_price,
        }
    }

    /// 손절/익절 설정.
    pub fn with_stops(mut self, stop_loss: Option<Decimal>, take_profit: Option<Decimal>) -> Self {
        self.stop_loss = stop_loss;
        self.take_profit = take_profit;
        self
    }

    /// 미실현 손익 계산.
    pub fn unrealized_pnl(&self, current_price: Decimal) -> Decimal {
        unrealized_pnl(self.entry_price, current_price, self.quantity, self.side)
    }

    /// 포지션 추가 (물타기/추가 진입).
    pub fn add(&mut self, quantity: Decimal, price: Decimal) {
        let new_total_cost = self.total_cost + (quantity * price);
        let new_quantity = self.quantity + quantity;
        self.entry_price = if new_quantity > Decimal::ZERO {
            new_total_cost / new_quantity
        } else {
            Decimal::ZERO
        };
        self.quantity = new_quantity;
        self.total_cost = new_total_cost;
    }

    /// 포지션 감소 (부분 청산).
    pub fn reduce(&mut self, quantity: Decimal, exit_price: Decimal) -> Decimal {
        let reduce_qty = quantity.min(self.quantity);
        let pnl = match self.side {
            Side::Buy => (exit_price - self.entry_price) * reduce_qty,
            Side::Sell => (self.entry_price - exit_price) * reduce_qty,
        };
        self.realized_pnl += pnl;
        self.quantity -= reduce_qty;
        self.total_cost = self.quantity * self.entry_price;
        pnl
    }
}

/// 체결 처리 결과.
#[derive(Debug, Clone)]
pub enum FillResult {
    /// 새 포지션 진입.
    PositionOpened { position: SyncedPosition },
    /// 포지션 완전 청산.
    PositionClosed {
        /// 실현 손익.
        pnl: Decimal,
        /// 진입가.
        entry_price: Decimal,
        /// 청산가.
        exit_price: Decimal,
    },
    /// 포지션 업데이트 (추가 진입 또는 부분 청산).
    PositionUpdated {
        position: SyncedPosition,
        /// 부분 청산 시 실현된 손익.
        partial_pnl: Option<Decimal>,
    },
    /// 변화 없음 (관련 없는 주문 또는 아직 미체결).
    NoChange,
}

/// 포지션 동기화 관리자.
///
/// 전략의 내부 포지션 상태를 거래소 체결 정보와 동기화합니다.
/// 거래소 중립적으로 설계되어 어떤 거래소에서도 동일하게 작동합니다.
#[derive(Debug, Clone, Default)]
pub struct PositionSync {
    /// 현재 포지션.
    position: Option<SyncedPosition>,
    /// 대상 심볼 (특정 심볼만 추적).
    target_ticker: Option<String>,
}

impl PositionSync {
    /// 새 PositionSync 생성.
    pub fn new() -> Self {
        Self::default()
    }

    /// 특정 심볼만 추적하도록 설정.
    pub fn with_ticker(mut self, ticker: String) -> Self {
        self.target_ticker = Some(ticker);
        self
    }

    /// 현재 포지션 조회.
    pub fn position(&self) -> Option<&SyncedPosition> {
        self.position.as_ref()
    }

    /// 포지션 보유 여부.
    pub fn has_position(&self) -> bool {
        self.position
            .as_ref()
            .map(|p| p.quantity > Decimal::ZERO)
            .unwrap_or(false)
    }

    /// 포지션 직접 설정 (외부 동기화용).
    pub fn set_position(&mut self, position: Option<SyncedPosition>) {
        self.position = position;
    }

    /// 손절/익절가 설정.
    pub fn set_stops(&mut self, stop_loss: Option<Decimal>, take_profit: Option<Decimal>) {
        if let Some(ref mut pos) = self.position {
            pos.stop_loss = stop_loss;
            pos.take_profit = take_profit;
        }
    }

    /// 주문 체결 처리.
    ///
    /// 체결된 주문을 분석하여 포지션 상태를 업데이트합니다.
    /// 거래소 중립적으로 동작하며, Order 구조체의 정보만 사용합니다.
    pub fn process_fill(&mut self, order: &Order) -> FillResult {
        // 완전히 체결되지 않은 주문은 무시
        if order.status != OrderStatusType::Filled {
            debug!(
                order_id = %order.id,
                status = ?order.status,
                "아직 완전 체결되지 않은 주문, 건너뜀"
            );
            return FillResult::NoChange;
        }

        // 대상 심볼 필터링
        if let Some(ref target) = self.target_ticker {
            if &order.ticker != target {
                debug!(
                    order_ticker = %order.ticker,
                    target_ticker = %target,
                    "대상 심볼 불일치, 건너뜀"
                );
                return FillResult::NoChange;
            }
        }

        // 체결 가격 결정 (평균 체결가 우선, 없으면 지정가 사용)
        let fill_price = order
            .average_fill_price
            .or(order.price)
            .unwrap_or(Decimal::ZERO);

        if fill_price == Decimal::ZERO {
            warn!(
                order_id = %order.id,
                "체결 가격을 결정할 수 없음"
            );
            return FillResult::NoChange;
        }

        let fill_qty = order.filled_quantity;
        if fill_qty == Decimal::ZERO {
            return FillResult::NoChange;
        }

        match (&mut self.position, order.side) {
            // 포지션 없는 상태에서 매수 → 롱 포지션 진입
            (None, Side::Buy) => {
                let position =
                    SyncedPosition::new(order.ticker.clone(), Side::Buy, fill_qty, fill_price);
                info!(
                    ticker = %order.ticker,
                    side = "Buy",
                    quantity = %fill_qty,
                    price = %fill_price,
                    "롱 포지션 진입"
                );
                self.position = Some(position.clone());
                FillResult::PositionOpened { position }
            }

            // 포지션 없는 상태에서 매도 → 숏 포지션 진입
            (None, Side::Sell) => {
                let position =
                    SyncedPosition::new(order.ticker.clone(), Side::Sell, fill_qty, fill_price);
                info!(
                    ticker = %order.ticker,
                    side = "Sell",
                    quantity = %fill_qty,
                    price = %fill_price,
                    "숏 포지션 진입"
                );
                self.position = Some(position.clone());
                FillResult::PositionOpened { position }
            }

            // 롱 포지션에서 매수 → 추가 진입 (물타기)
            (Some(ref mut pos), Side::Buy) if pos.side == Side::Buy => {
                pos.add(fill_qty, fill_price);
                info!(
                    ticker = %order.ticker,
                    new_quantity = %pos.quantity,
                    avg_price = %pos.entry_price,
                    "롱 포지션 추가"
                );
                FillResult::PositionUpdated {
                    position: pos.clone(),
                    partial_pnl: None,
                }
            }

            // 롱 포지션에서 매도 → 청산 (전체 또는 부분)
            (Some(ref mut pos), Side::Sell) if pos.side == Side::Buy => {
                let entry_price = pos.entry_price;
                let pnl = pos.reduce(fill_qty, fill_price);

                if pos.quantity <= Decimal::ZERO {
                    info!(
                        ticker = %order.ticker,
                        entry = %entry_price,
                        exit = %fill_price,
                        pnl = %pnl,
                        "롱 포지션 완전 청산"
                    );
                    self.position = None;
                    FillResult::PositionClosed {
                        pnl,
                        entry_price,
                        exit_price: fill_price,
                    }
                } else {
                    info!(
                        ticker = %order.ticker,
                        remaining = %pos.quantity,
                        partial_pnl = %pnl,
                        "롱 포지션 부분 청산"
                    );
                    FillResult::PositionUpdated {
                        position: pos.clone(),
                        partial_pnl: Some(pnl),
                    }
                }
            }

            // 숏 포지션에서 매도 → 추가 진입
            (Some(ref mut pos), Side::Sell) if pos.side == Side::Sell => {
                pos.add(fill_qty, fill_price);
                info!(
                    ticker = %order.ticker,
                    new_quantity = %pos.quantity,
                    avg_price = %pos.entry_price,
                    "숏 포지션 추가"
                );
                FillResult::PositionUpdated {
                    position: pos.clone(),
                    partial_pnl: None,
                }
            }

            // 숏 포지션에서 매수 → 청산 (전체 또는 부분)
            (Some(ref mut pos), Side::Buy) if pos.side == Side::Sell => {
                let entry_price = pos.entry_price;
                let pnl = pos.reduce(fill_qty, fill_price);

                if pos.quantity <= Decimal::ZERO {
                    info!(
                        ticker = %order.ticker,
                        entry = %entry_price,
                        exit = %fill_price,
                        pnl = %pnl,
                        "숏 포지션 완전 청산"
                    );
                    self.position = None;
                    FillResult::PositionClosed {
                        pnl,
                        entry_price,
                        exit_price: fill_price,
                    }
                } else {
                    info!(
                        ticker = %order.ticker,
                        remaining = %pos.quantity,
                        partial_pnl = %pnl,
                        "숏 포지션 부분 청산"
                    );
                    FillResult::PositionUpdated {
                        position: pos.clone(),
                        partial_pnl: Some(pnl),
                    }
                }
            }

            _ => FillResult::NoChange,
        }
    }

    /// 외부 Position 정보로 동기화.
    ///
    /// 거래소에서 직접 조회한 포지션 정보로 내부 상태를 동기화합니다.
    /// 불일치 발생 시 외부 정보를 신뢰합니다.
    pub fn sync_from_external(&mut self, external_position: &trader_core::Position) {
        // 수량이 0이면 포지션 없음으로 처리
        if external_position.quantity == Decimal::ZERO {
            if self.position.is_some() {
                warn!(
                    ticker = %external_position.ticker,
                    "외부 동기화: 포지션이 예상과 달리 없음, 내부 상태 초기화"
                );
                self.position = None;
            }
            return;
        }

        match &mut self.position {
            Some(ref mut pos) => {
                // 기존 포지션과 비교 후 업데이트
                if pos.quantity != external_position.quantity
                    || pos.entry_price != external_position.entry_price
                {
                    warn!(
                        internal_qty = %pos.quantity,
                        external_qty = %external_position.quantity,
                        internal_price = %pos.entry_price,
                        external_price = %external_position.entry_price,
                        "포지션 불일치 감지, 외부 정보로 동기화"
                    );
                    pos.quantity = external_position.quantity;
                    pos.entry_price = external_position.entry_price;
                    pos.total_cost = pos.quantity * pos.entry_price;
                }
            }
            None => {
                // 내부 포지션 없는데 외부에 있음 → 생성
                warn!(
                    ticker = %external_position.ticker,
                    quantity = %external_position.quantity,
                    "외부 동기화: 예상치 못한 포지션 발견, 내부 상태 생성"
                );
                self.position = Some(SyncedPosition::new(
                    external_position.ticker.clone(),
                    external_position.side,
                    external_position.quantity,
                    external_position.entry_price,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use trader_core::{OrderType, TimeInForce};
    use uuid::Uuid;

    fn create_filled_order(ticker: &str, side: Side, qty: Decimal, price: Decimal) -> Order {
        // 테스트용 심볼 파싱 (예: "BTCUSDT" -> BTC/USDT)
        let (base, quote) = if ticker.ends_with("USDT") {
            (ticker.strip_suffix("USDT").unwrap(), "USDT")
        } else if ticker.ends_with("USD") {
            (ticker.strip_suffix("USD").unwrap(), "USD")
        } else {
            (ticker, "USD")
        };

        Order {
            id: Uuid::new_v4(),
            exchange: "test".to_string(),
            exchange_order_id: Some("123".to_string()),
            ticker: base.to_string(),
            side,
            order_type: OrderType::Market,
            quantity: qty,
            price: Some(price),
            stop_price: None,
            status: OrderStatusType::Filled,
            filled_quantity: qty,
            average_fill_price: Some(price),
            time_in_force: TimeInForce::GTC,
            client_order_id: None,
            strategy_id: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            metadata: serde_json::Value::Null,
        }
    }

    #[test]
    fn test_open_long_position() {
        let mut sync = PositionSync::new();
        let order = create_filled_order("BTCUSDT", Side::Buy, dec!(1), dec!(50000));

        let result = sync.process_fill(&order);

        match result {
            FillResult::PositionOpened { position } => {
                assert_eq!(position.side, Side::Buy);
                assert_eq!(position.quantity, dec!(1));
                assert_eq!(position.entry_price, dec!(50000));
            }
            _ => panic!("Expected PositionOpened"),
        }
        assert!(sync.has_position());
    }

    #[test]
    fn test_close_long_position() {
        let mut sync = PositionSync::new();

        // 진입
        let buy_order = create_filled_order("BTCUSDT", Side::Buy, dec!(1), dec!(50000));
        sync.process_fill(&buy_order);

        // 청산
        let sell_order = create_filled_order("BTCUSDT", Side::Sell, dec!(1), dec!(55000));
        let result = sync.process_fill(&sell_order);

        match result {
            FillResult::PositionClosed { pnl, .. } => {
                assert_eq!(pnl, dec!(5000)); // 55000 - 50000 = 5000 이익
            }
            _ => panic!("Expected PositionClosed"),
        }
        assert!(!sync.has_position());
    }

    #[test]
    fn test_add_to_position() {
        let mut sync = PositionSync::new();

        // 첫 진입
        let order1 = create_filled_order("BTCUSDT", Side::Buy, dec!(1), dec!(50000));
        sync.process_fill(&order1);

        // 추가 진입
        let order2 = create_filled_order("BTCUSDT", Side::Buy, dec!(1), dec!(48000));
        let result = sync.process_fill(&order2);

        match result {
            FillResult::PositionUpdated { position, .. } => {
                assert_eq!(position.quantity, dec!(2));
                // 평균가: (50000 + 48000) / 2 = 49000
                assert_eq!(position.entry_price, dec!(49000));
            }
            _ => panic!("Expected PositionUpdated"),
        }
    }

    #[test]
    fn test_partial_close() {
        let mut sync = PositionSync::new();

        // 진입
        let buy_order = create_filled_order("BTCUSDT", Side::Buy, dec!(2), dec!(50000));
        sync.process_fill(&buy_order);

        // 부분 청산
        let sell_order = create_filled_order("BTCUSDT", Side::Sell, dec!(1), dec!(55000));
        let result = sync.process_fill(&sell_order);

        match result {
            FillResult::PositionUpdated {
                position,
                partial_pnl,
            } => {
                assert_eq!(position.quantity, dec!(1));
                assert_eq!(partial_pnl, Some(dec!(5000)));
            }
            _ => panic!("Expected PositionUpdated"),
        }
        assert!(sync.has_position());
    }

    #[test]
    fn test_ticker_filter() {
        let mut sync = PositionSync::new().with_ticker("BTC/USDT".to_string());

        // 다른 심볼 주문은 무시
        let eth_order = create_filled_order("ETHUSDT", Side::Buy, dec!(10), dec!(3000));
        let result = sync.process_fill(&eth_order);

        assert!(matches!(result, FillResult::NoChange));
        assert!(!sync.has_position());
    }
}
