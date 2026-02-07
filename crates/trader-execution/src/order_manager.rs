//! 주문 상태 관리.
//!
//! 제공 기능:
//! - 주문 생명주기 추적
//! - 주문 장부 유지 관리
//! - 주문 이벤트 처리
//! - 조회 기능

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use trader_core::{Order, OrderRequest, OrderStatus, OrderStatusType, Side};
use uuid::Uuid;

/// 주문 관리자 에러 타입.
#[derive(Debug, Error)]
pub enum OrderManagerError {
    #[error("Order not found: {0}")]
    OrderNotFound(Uuid),

    #[error("Order already exists: {0}")]
    OrderAlreadyExists(Uuid),

    #[error("Invalid state transition: {0} -> {1}")]
    InvalidStateTransition(String, String),

    #[error("Order is in final state: {0}")]
    OrderFinalized(Uuid),
}

/// 변경 사항 추적을 위한 주문 이벤트 타입.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderEvent {
    /// 주문 생성됨
    Created {
        order_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    /// 거래소에 주문 제출됨
    Submitted {
        order_id: Uuid,
        exchange_order_id: String,
        timestamp: DateTime<Utc>,
    },
    /// 주문 부분 체결됨
    PartialFill {
        order_id: Uuid,
        filled_qty: Decimal,
        fill_price: Decimal,
        timestamp: DateTime<Utc>,
    },
    /// 주문 완전 체결됨
    Filled {
        order_id: Uuid,
        avg_price: Decimal,
        timestamp: DateTime<Utc>,
    },
    /// 주문 취소됨
    Cancelled {
        order_id: Uuid,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },
    /// 주문 거부됨
    Rejected {
        order_id: Uuid,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    /// 주문 만료됨
    Expired {
        order_id: Uuid,
        timestamp: DateTime<Utc>,
    },
}

impl OrderEvent {
    /// 이벤트에서 주문 ID를 가져온다.
    pub fn order_id(&self) -> Uuid {
        match self {
            OrderEvent::Created { order_id, .. } => *order_id,
            OrderEvent::Submitted { order_id, .. } => *order_id,
            OrderEvent::PartialFill { order_id, .. } => *order_id,
            OrderEvent::Filled { order_id, .. } => *order_id,
            OrderEvent::Cancelled { order_id, .. } => *order_id,
            OrderEvent::Rejected { order_id, .. } => *order_id,
            OrderEvent::Expired { order_id, .. } => *order_id,
        }
    }

    /// 이벤트의 타임스탬프를 가져온다.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            OrderEvent::Created { timestamp, .. } => *timestamp,
            OrderEvent::Submitted { timestamp, .. } => *timestamp,
            OrderEvent::PartialFill { timestamp, .. } => *timestamp,
            OrderEvent::Filled { timestamp, .. } => *timestamp,
            OrderEvent::Cancelled { timestamp, .. } => *timestamp,
            OrderEvent::Rejected { timestamp, .. } => *timestamp,
            OrderEvent::Expired { timestamp, .. } => *timestamp,
        }
    }
}

/// 주문 체결 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFill {
    /// 주문 ID
    pub order_id: Uuid,
    /// 체결 수량
    pub quantity: Decimal,
    /// 체결 가격
    pub price: Decimal,
    /// 지불한 수수료
    pub commission: Option<Decimal>,
    /// 수수료 자산
    pub commission_asset: Option<String>,
    /// 타임스탬프
    pub timestamp: DateTime<Utc>,
}

/// 모든 주문을 추적하는 주문 관리자.
#[derive(Debug)]
pub struct OrderManager {
    /// ID별 모든 주문
    orders: HashMap<Uuid, Order>,
    /// 활성 주문 (최종 상태가 아닌 주문)
    active_orders: HashMap<Uuid, Order>,
    /// 심볼별 주문
    orders_by_symbol: HashMap<String, Vec<Uuid>>,
    /// 전략별 주문
    orders_by_strategy: HashMap<String, Vec<Uuid>>,
    /// 거래소 주문 ID에서 내부 ID로의 매핑
    exchange_id_map: HashMap<String, Uuid>,
    /// 주문 이벤트 이력
    events: Vec<OrderEvent>,
    /// 체결 이력
    fills: Vec<OrderFill>,
    /// 최대 이력 크기
    max_history_size: usize,
}

impl Default for OrderManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderManager {
    /// 새로운 주문 관리자를 생성한다.
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
            active_orders: HashMap::new(),
            orders_by_symbol: HashMap::new(),
            orders_by_strategy: HashMap::new(),
            exchange_id_map: HashMap::new(),
            events: Vec::new(),
            fills: Vec::new(),
            max_history_size: 10000,
        }
    }

    /// 사용자 정의 이력 크기로 생성한다.
    pub fn with_history_size(max_history_size: usize) -> Self {
        Self {
            max_history_size,
            ..Self::new()
        }
    }

    // ==================== 주문 생성 ====================

    /// 요청으로부터 새 주문을 생성하고 추적한다.
    pub fn create_order(
        &mut self,
        request: OrderRequest,
        exchange: &str,
    ) -> Result<Order, OrderManagerError> {
        let order = Order::from_request(request, exchange);
        self.add_order(order.clone())?;
        Ok(order)
    }

    /// 기존 주문을 추적에 추가한다.
    pub fn add_order(&mut self, order: Order) -> Result<(), OrderManagerError> {
        if self.orders.contains_key(&order.id) {
            return Err(OrderManagerError::OrderAlreadyExists(order.id));
        }

        let order_id = order.id;
        let symbol = order.ticker.to_string();
        let strategy = order.strategy_id.clone();

        // 메인 저장소에 추가
        self.orders.insert(order_id, order.clone());

        // 최종 상태가 아니면 활성 주문에 추가
        if order.status.is_active() {
            self.active_orders.insert(order_id, order);
        }

        // 심볼별 인덱스
        self.orders_by_symbol
            .entry(symbol)
            .or_default()
            .push(order_id);

        // 전략별 인덱스
        if let Some(strategy_id) = strategy {
            self.orders_by_strategy
                .entry(strategy_id)
                .or_default()
                .push(order_id);
        }

        // 이벤트 기록
        self.record_event(OrderEvent::Created {
            order_id,
            timestamp: Utc::now(),
        });

        Ok(())
    }

    // ==================== 주문 업데이트 ====================

    /// 거래소로부터 주문 상태를 업데이트한다.
    pub fn update_status(
        &mut self,
        order_id: Uuid,
        status: &OrderStatus,
    ) -> Result<(), OrderManagerError> {
        // 먼저 주문이 존재하는지 확인하고 이전 상태를 가져옴
        let (old_status, needs_exchange_id_update) = {
            let order = self
                .orders
                .get(&order_id)
                .ok_or(OrderManagerError::OrderNotFound(order_id))?;

            if order.status.is_final() {
                return Err(OrderManagerError::OrderFinalized(order_id));
            }

            (order.status, order.exchange_order_id.is_none())
        };

        let now = Utc::now();

        // 주문 업데이트 (위에서 존재 확인 후 가져오므로 논리적으로 안전)
        {
            let order = self
                .orders
                .get_mut(&order_id)
                .ok_or(OrderManagerError::OrderNotFound(order_id))?;
            order.status = status.status;
            order.filled_quantity = status.filled_quantity;
            order.average_fill_price = status.average_price;
            order.updated_at = now;

            if needs_exchange_id_update {
                order.exchange_order_id = Some(status.order_id.clone());
            }
        }

        // 거래소 ID 맵 업데이트
        if needs_exchange_id_update {
            self.exchange_id_map
                .insert(status.order_id.clone(), order_id);
        }

        // 적절한 이벤트 생성 (가변 빌림이 해제되어 호출 가능)
        match status.status {
            OrderStatusType::Open => {
                if old_status == OrderStatusType::Pending {
                    self.record_event(OrderEvent::Submitted {
                        order_id,
                        exchange_order_id: status.order_id.clone(),
                        timestamp: now,
                    });
                }
            }
            OrderStatusType::PartiallyFilled => {
                self.record_event(OrderEvent::PartialFill {
                    order_id,
                    filled_qty: status.filled_quantity,
                    fill_price: status.average_price.unwrap_or(Decimal::ZERO),
                    timestamp: now,
                });
            }
            OrderStatusType::Filled => {
                self.record_event(OrderEvent::Filled {
                    order_id,
                    avg_price: status.average_price.unwrap_or(Decimal::ZERO),
                    timestamp: now,
                });
                self.active_orders.remove(&order_id);
            }
            OrderStatusType::Cancelled => {
                self.record_event(OrderEvent::Cancelled {
                    order_id,
                    reason: None,
                    timestamp: now,
                });
                self.active_orders.remove(&order_id);
            }
            OrderStatusType::Rejected => {
                self.record_event(OrderEvent::Rejected {
                    order_id,
                    reason: "Rejected by exchange".to_string(),
                    timestamp: now,
                });
                self.active_orders.remove(&order_id);
            }
            OrderStatusType::Expired => {
                self.record_event(OrderEvent::Expired {
                    order_id,
                    timestamp: now,
                });
                self.active_orders.remove(&order_id);
            }
            OrderStatusType::Pending => {}
        }

        // 활성 주문 업데이트
        if let Some(order) = self.orders.get(&order_id) {
            if let Some(active_order) = self.active_orders.get_mut(&order_id) {
                *active_order = order.clone();
            }
        }

        Ok(())
    }

    /// 주문에 대한 체결을 기록한다.
    pub fn record_fill(&mut self, fill: OrderFill) -> Result<(), OrderManagerError> {
        // 주문 존재 여부 확인
        if !self.orders.contains_key(&fill.order_id) {
            return Err(OrderManagerError::OrderNotFound(fill.order_id));
        }

        // 업데이트 후 이벤트에 필요한 데이터 수집
        let (new_filled, avg_price, is_fully_filled, was_partially_filled, order_quantity);

        // 주문 업데이트 (위에서 contains_key 확인 후 가져오므로 논리적으로 안전)
        {
            let order = self
                .orders
                .get_mut(&fill.order_id)
                .ok_or(OrderManagerError::OrderNotFound(fill.order_id))?;

            // 주문 체결 수량 및 평균 가격 업데이트
            let old_filled = order.filled_quantity;
            new_filled = old_filled + fill.quantity;

            // 새로운 평균 가격 계산
            if let Some(old_avg) = order.average_fill_price {
                let total_value = old_avg * old_filled + fill.price * fill.quantity;
                order.average_fill_price = Some(total_value / new_filled);
            } else {
                order.average_fill_price = Some(fill.price);
            }

            avg_price = order.average_fill_price.unwrap_or(Decimal::ZERO);
            order.filled_quantity = new_filled;
            order.updated_at = fill.timestamp;
            order_quantity = order.quantity;
            was_partially_filled = order.status == OrderStatusType::PartiallyFilled;

            // 완전 체결 여부 확인
            is_fully_filled = new_filled >= order_quantity;
            if is_fully_filled {
                order.status = OrderStatusType::Filled;
            } else if new_filled > Decimal::ZERO && !was_partially_filled {
                order.status = OrderStatusType::PartiallyFilled;
            }
        }

        // 이벤트 기록 (주문 빌림이 해제되어 안전)
        if is_fully_filled {
            self.active_orders.remove(&fill.order_id);
            self.record_event(OrderEvent::Filled {
                order_id: fill.order_id,
                avg_price,
                timestamp: fill.timestamp,
            });
        } else if new_filled > Decimal::ZERO && !was_partially_filled {
            self.record_event(OrderEvent::PartialFill {
                order_id: fill.order_id,
                filled_qty: new_filled,
                fill_price: fill.price,
                timestamp: fill.timestamp,
            });
        }

        // 활성 주문 업데이트
        if let Some(order) = self.orders.get(&fill.order_id) {
            if let Some(active_order) = self.active_orders.get_mut(&fill.order_id) {
                *active_order = order.clone();
            }
        }

        // 체결 저장
        self.fills.push(fill);
        self.trim_history();

        Ok(())
    }

    /// 주문을 취소한다.
    pub fn cancel_order(
        &mut self,
        order_id: Uuid,
        reason: Option<String>,
    ) -> Result<(), OrderManagerError> {
        let order = self
            .orders
            .get_mut(&order_id)
            .ok_or(OrderManagerError::OrderNotFound(order_id))?;

        if order.status.is_final() {
            return Err(OrderManagerError::OrderFinalized(order_id));
        }

        order.status = OrderStatusType::Cancelled;
        order.updated_at = Utc::now();

        self.active_orders.remove(&order_id);

        self.record_event(OrderEvent::Cancelled {
            order_id,
            reason,
            timestamp: Utc::now(),
        });

        Ok(())
    }

    /// 주문을 거부한다.
    pub fn reject_order(
        &mut self,
        order_id: Uuid,
        reason: impl Into<String>,
    ) -> Result<(), OrderManagerError> {
        let order = self
            .orders
            .get_mut(&order_id)
            .ok_or(OrderManagerError::OrderNotFound(order_id))?;

        if order.status.is_final() {
            return Err(OrderManagerError::OrderFinalized(order_id));
        }

        order.status = OrderStatusType::Rejected;
        order.updated_at = Utc::now();

        self.active_orders.remove(&order_id);

        let reason_str = reason.into();
        self.record_event(OrderEvent::Rejected {
            order_id,
            reason: reason_str,
            timestamp: Utc::now(),
        });

        Ok(())
    }

    // ==================== 조회 ====================

    /// ID로 주문을 가져온다.
    pub fn get_order(&self, order_id: Uuid) -> Option<&Order> {
        self.orders.get(&order_id)
    }

    /// 거래소 주문 ID로 주문을 가져온다.
    pub fn get_order_by_exchange_id(&self, exchange_order_id: &str) -> Option<&Order> {
        self.exchange_id_map
            .get(exchange_order_id)
            .and_then(|id| self.orders.get(id))
    }

    /// 모든 활성 주문을 가져온다.
    pub fn get_active_orders(&self) -> Vec<&Order> {
        self.active_orders.values().collect()
    }

    /// 심볼에 대한 활성 주문을 가져온다.
    pub fn get_active_orders_for_symbol(&self, symbol: &str) -> Vec<&Order> {
        self.orders_by_symbol
            .get(symbol)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.active_orders.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 심볼에 대한 주문을 가져온다 (모든 상태).
    pub fn get_orders_for_symbol(&self, symbol: &str) -> Vec<&Order> {
        self.orders_by_symbol
            .get(symbol)
            .map(|ids| ids.iter().filter_map(|id| self.orders.get(id)).collect())
            .unwrap_or_default()
    }

    /// 전략에 대한 주문을 가져온다.
    pub fn get_orders_for_strategy(&self, strategy_id: &str) -> Vec<&Order> {
        self.orders_by_strategy
            .get(strategy_id)
            .map(|ids| ids.iter().filter_map(|id| self.orders.get(id)).collect())
            .unwrap_or_default()
    }

    /// 상태별로 주문을 가져온다.
    pub fn get_orders_by_status(&self, status: OrderStatusType) -> Vec<&Order> {
        self.orders
            .values()
            .filter(|o| o.status == status)
            .collect()
    }

    /// 총 주문 수를 가져온다.
    pub fn total_orders(&self) -> usize {
        self.orders.len()
    }

    /// 활성 주문 수를 가져온다.
    pub fn active_order_count(&self) -> usize {
        self.active_orders.len()
    }

    /// 주문 이벤트를 가져온다.
    pub fn get_events(&self) -> &[OrderEvent] {
        &self.events
    }

    /// 특정 주문의 이벤트를 가져온다.
    pub fn get_order_events(&self, order_id: Uuid) -> Vec<&OrderEvent> {
        self.events
            .iter()
            .filter(|e| e.order_id() == order_id)
            .collect()
    }

    /// 주문의 체결 내역을 가져온다.
    pub fn get_order_fills(&self, order_id: Uuid) -> Vec<&OrderFill> {
        self.fills
            .iter()
            .filter(|f| f.order_id == order_id)
            .collect()
    }

    // ==================== 통계 ====================

    /// 심볼에 대한 통계를 가져온다.
    pub fn get_symbol_stats(&self, symbol: &str) -> OrderStats {
        let orders = self.get_orders_for_symbol(symbol);
        self.calculate_stats(&orders)
    }

    /// 전략에 대한 통계를 가져온다.
    pub fn get_strategy_stats(&self, strategy_id: &str) -> OrderStats {
        let orders = self.get_orders_for_strategy(strategy_id);
        self.calculate_stats(&orders)
    }

    /// 전체 통계를 가져온다.
    pub fn get_overall_stats(&self) -> OrderStats {
        let orders: Vec<&Order> = self.orders.values().collect();
        self.calculate_stats(&orders)
    }

    fn calculate_stats(&self, orders: &[&Order]) -> OrderStats {
        let total = orders.len();
        let filled = orders
            .iter()
            .filter(|o| o.status == OrderStatusType::Filled)
            .count();
        let cancelled = orders
            .iter()
            .filter(|o| o.status == OrderStatusType::Cancelled)
            .count();
        let rejected = orders
            .iter()
            .filter(|o| o.status == OrderStatusType::Rejected)
            .count();
        let active = orders.iter().filter(|o| o.status.is_active()).count();

        let buy_orders = orders.iter().filter(|o| o.side == Side::Buy).count();
        let sell_orders = orders.iter().filter(|o| o.side == Side::Sell).count();

        let total_volume: Decimal = orders
            .iter()
            .filter(|o| o.status == OrderStatusType::Filled)
            .map(|o| o.filled_quantity)
            .sum();

        let total_notional: Decimal = orders
            .iter()
            .filter(|o| o.status == OrderStatusType::Filled)
            .filter_map(|o| o.average_fill_price.map(|p| p * o.filled_quantity))
            .sum();

        OrderStats {
            total_orders: total,
            filled_orders: filled,
            cancelled_orders: cancelled,
            rejected_orders: rejected,
            active_orders: active,
            buy_orders,
            sell_orders,
            total_volume,
            total_notional,
            fill_rate: if total > 0 {
                filled as f64 / total as f64
            } else {
                0.0
            },
        }
    }

    // ==================== 내부 ====================

    fn record_event(&mut self, event: OrderEvent) {
        self.events.push(event);
        self.trim_history();
    }

    fn trim_history(&mut self) {
        if self.events.len() > self.max_history_size {
            let drain_count = self.events.len() - self.max_history_size;
            self.events.drain(0..drain_count);
        }
        if self.fills.len() > self.max_history_size {
            let drain_count = self.fills.len() - self.max_history_size;
            self.fills.drain(0..drain_count);
        }
    }

    /// 특정 기간보다 오래된 완료된 주문을 정리한다 (메모리 관리용).
    pub fn cleanup_old_orders(&mut self, older_than: DateTime<Utc>) {
        // 제거할 주문 찾기
        let orders_to_remove: Vec<Uuid> = self
            .orders
            .iter()
            .filter(|(_, o)| o.status.is_final() && o.updated_at < older_than)
            .map(|(id, _)| *id)
            .collect();

        for order_id in orders_to_remove {
            if let Some(order) = self.orders.remove(&order_id) {
                // 심볼 인덱스에서 제거
                if let Some(ids) = self.orders_by_symbol.get_mut(&order.ticker.to_string()) {
                    ids.retain(|id| *id != order_id);
                }

                // 전략 인덱스에서 제거
                if let Some(strategy_id) = &order.strategy_id {
                    if let Some(ids) = self.orders_by_strategy.get_mut(strategy_id) {
                        ids.retain(|id| *id != order_id);
                    }
                }

                // 거래소 ID 맵에서 제거
                if let Some(exchange_id) = &order.exchange_order_id {
                    self.exchange_id_map.remove(exchange_id);
                }
            }
        }
    }
}

/// 주문 통계.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderStats {
    pub total_orders: usize,
    pub filled_orders: usize,
    pub cancelled_orders: usize,
    pub rejected_orders: usize,
    pub active_orders: usize,
    pub buy_orders: usize,
    pub sell_orders: usize,
    pub total_volume: Decimal,
    pub total_notional: Decimal,
    pub fill_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::prelude::FromPrimitive;
    use trader_core::OrderRequest;

    /// Decimal 생성을 위한 헬퍼 매크로
    macro_rules! dec {
        ($val:expr) => {
            Decimal::from_f64($val as f64).unwrap()
        };
    }

    fn create_test_order(side: Side) -> Order {
        let request = match side {
            Side::Buy => OrderRequest::market_buy("BTC/USDT".to_string(), dec!(0.1)),
            Side::Sell => OrderRequest::market_sell("BTC/USDT".to_string(), dec!(0.1)),
        };
        Order::from_request(request.with_strategy("test"), "binance")
    }

    #[test]
    fn test_create_order() {
        let mut manager = OrderManager::new();
        let request =
            OrderRequest::market_buy("BTC/USDT".to_string(), dec!(0.1)).with_strategy("grid");

        let order = manager.create_order(request, "binance").unwrap();

        assert_eq!(order.status, OrderStatusType::Pending);
        assert_eq!(manager.total_orders(), 1);
        assert_eq!(manager.active_order_count(), 1);
    }

    #[test]
    fn test_add_duplicate_order_fails() {
        let mut manager = OrderManager::new();
        let order = create_test_order(Side::Buy);
        let order_id = order.id;

        manager.add_order(order.clone()).unwrap();
        let result = manager.add_order(order);

        assert!(matches!(
            result,
            Err(OrderManagerError::OrderAlreadyExists(id)) if id == order_id
        ));
    }

    #[test]
    fn test_update_status_to_filled() {
        let mut manager = OrderManager::new();
        let order = create_test_order(Side::Buy);
        let order_id = order.id;
        manager.add_order(order).unwrap();

        let status = OrderStatus {
            order_id: "BINANCE123".to_string(),
            client_order_id: None,
            ticker: None,
            side: None,
            quantity: None,
            price: None,
            status: OrderStatusType::Filled,
            filled_quantity: dec!(0.1),
            average_price: Some(dec!(50000)),
            updated_at: Utc::now(),
        };

        manager.update_status(order_id, &status).unwrap();

        let updated = manager.get_order(order_id).unwrap();
        assert_eq!(updated.status, OrderStatusType::Filled);
        assert_eq!(updated.filled_quantity, dec!(0.1));
        assert_eq!(updated.average_fill_price, Some(dec!(50000)));
        assert_eq!(manager.active_order_count(), 0);
    }

    #[test]
    fn test_record_fill() {
        let mut manager = OrderManager::new();
        let order = create_test_order(Side::Buy);
        let order_id = order.id;
        manager.add_order(order).unwrap();

        // 첫 번째 부분 체결
        let fill1 = OrderFill {
            order_id,
            quantity: dec!(0.05),
            price: dec!(49000),
            commission: Some(dec!(0.005)),
            commission_asset: Some("BNB".to_string()),
            timestamp: Utc::now(),
        };
        manager.record_fill(fill1).unwrap();

        let updated = manager.get_order(order_id).unwrap();
        assert_eq!(updated.status, OrderStatusType::PartiallyFilled);
        assert_eq!(updated.filled_quantity, dec!(0.05));

        // 두 번째 체결 - 주문 완료
        let fill2 = OrderFill {
            order_id,
            quantity: dec!(0.05),
            price: dec!(51000),
            commission: None,
            commission_asset: None,
            timestamp: Utc::now(),
        };
        manager.record_fill(fill2).unwrap();

        let final_order = manager.get_order(order_id).unwrap();
        assert_eq!(final_order.status, OrderStatusType::Filled);
        assert_eq!(final_order.filled_quantity, dec!(0.1));
        // 평균 가격은 (49000*0.05 + 51000*0.05) / 0.1 = 50000 이어야 함
        assert_eq!(final_order.average_fill_price, Some(dec!(50000)));
    }

    #[test]
    fn test_cancel_order() {
        let mut manager = OrderManager::new();
        let order = create_test_order(Side::Buy);
        let order_id = order.id;
        manager.add_order(order).unwrap();

        manager
            .cancel_order(order_id, Some("User cancelled".to_string()))
            .unwrap();

        let cancelled = manager.get_order(order_id).unwrap();
        assert_eq!(cancelled.status, OrderStatusType::Cancelled);
        assert_eq!(manager.active_order_count(), 0);
    }

    #[test]
    fn test_cannot_update_finalized_order() {
        let mut manager = OrderManager::new();
        let order = create_test_order(Side::Buy);
        let order_id = order.id;
        manager.add_order(order).unwrap();

        // 주문 취소
        manager.cancel_order(order_id, None).unwrap();

        // 다시 업데이트 시도
        let status = OrderStatus {
            order_id: "BINANCE123".to_string(),
            client_order_id: None,
            ticker: None,
            side: None,
            quantity: None,
            price: None,
            status: OrderStatusType::Filled,
            filled_quantity: dec!(0.1),
            average_price: Some(dec!(50000)),
            updated_at: Utc::now(),
        };

        let result = manager.update_status(order_id, &status);
        assert!(matches!(
            result,
            Err(OrderManagerError::OrderFinalized(id)) if id == order_id
        ));
    }

    #[test]
    fn test_get_orders_by_symbol() {
        let mut manager = OrderManager::new();

        let btc_order1 = create_test_order(Side::Buy);
        let btc_order2 = create_test_order(Side::Sell);
        manager.add_order(btc_order1).unwrap();
        manager.add_order(btc_order2).unwrap();

        let eth_ticker = "ETH/USDT".to_string();
        let eth_request = OrderRequest::market_buy(eth_ticker, dec!(1.0));
        manager.create_order(eth_request, "binance").unwrap();

        let btc_orders = manager.get_orders_for_symbol("BTC/USDT");
        assert_eq!(btc_orders.len(), 2);

        let eth_orders = manager.get_orders_for_symbol("ETH/USDT");
        assert_eq!(eth_orders.len(), 1);
    }

    #[test]
    fn test_get_orders_by_strategy() {
        let mut manager = OrderManager::new();

        let ticker = "BTC/USDT".to_string();

        let grid_request =
            OrderRequest::market_buy(ticker.clone(), dec!(0.1)).with_strategy("grid");
        let rsi_request = OrderRequest::market_sell(ticker, dec!(0.2)).with_strategy("rsi");

        manager.create_order(grid_request, "binance").unwrap();
        manager.create_order(rsi_request, "binance").unwrap();

        let grid_orders = manager.get_orders_for_strategy("grid");
        assert_eq!(grid_orders.len(), 1);

        let rsi_orders = manager.get_orders_for_strategy("rsi");
        assert_eq!(rsi_orders.len(), 1);
    }

    #[test]
    fn test_get_by_exchange_id() {
        let mut manager = OrderManager::new();
        let order = create_test_order(Side::Buy);
        let order_id = order.id;
        manager.add_order(order).unwrap();

        // 거래소 ID로 업데이트
        let status = OrderStatus {
            order_id: "BINANCE_ORDER_123".to_string(),
            client_order_id: None,
            ticker: None,
            side: None,
            quantity: None,
            price: None,
            status: OrderStatusType::Open,
            filled_quantity: Decimal::ZERO,
            average_price: None,
            updated_at: Utc::now(),
        };
        manager.update_status(order_id, &status).unwrap();

        let found = manager.get_order_by_exchange_id("BINANCE_ORDER_123");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, order_id);
    }

    #[test]
    fn test_order_stats() {
        let mut manager = OrderManager::new();

        // 매수 주문 생성 및 체결
        let order1 = create_test_order(Side::Buy);
        let order1_id = order1.id;
        manager.add_order(order1).unwrap();

        let status = OrderStatus {
            order_id: "1".to_string(),
            client_order_id: None,
            ticker: None,
            side: None,
            quantity: None,
            price: None,
            status: OrderStatusType::Filled,
            filled_quantity: dec!(0.1),
            average_price: Some(dec!(50000)),
            updated_at: Utc::now(),
        };
        manager.update_status(order1_id, &status).unwrap();

        // 취소된 주문 생성
        let order2 = create_test_order(Side::Sell);
        let order2_id = order2.id;
        manager.add_order(order2).unwrap();
        manager.cancel_order(order2_id, None).unwrap();

        // 활성 주문 생성
        let order3 = create_test_order(Side::Buy);
        manager.add_order(order3).unwrap();

        let stats = manager.get_overall_stats();
        assert_eq!(stats.total_orders, 3);
        assert_eq!(stats.filled_orders, 1);
        assert_eq!(stats.cancelled_orders, 1);
        assert_eq!(stats.active_orders, 1);
        assert_eq!(stats.buy_orders, 2);
        assert_eq!(stats.sell_orders, 1);
        assert_eq!(stats.total_volume, dec!(0.1));
        assert_eq!(stats.total_notional, dec!(5000)); // 0.1 * 50000
    }

    #[test]
    fn test_order_events() {
        let mut manager = OrderManager::new();
        let order = create_test_order(Side::Buy);
        let order_id = order.id;
        manager.add_order(order).unwrap();

        // 첫 번째 업데이트: Open 상태로 (거래소에 제출됨)
        let status_open = OrderStatus {
            order_id: "123".to_string(),
            client_order_id: None,
            ticker: None,
            side: None,
            quantity: None,
            price: None,
            status: OrderStatusType::Open,
            filled_quantity: Decimal::ZERO,
            average_price: None,
            updated_at: Utc::now(),
        };
        manager.update_status(order_id, &status_open).unwrap();

        // 그 다음 Filled 상태로 업데이트
        let status_filled = OrderStatus {
            order_id: "123".to_string(),
            client_order_id: None,
            ticker: None,
            side: None,
            quantity: None,
            price: None,
            status: OrderStatusType::Filled,
            filled_quantity: dec!(0.1),
            average_price: Some(dec!(50000)),
            updated_at: Utc::now(),
        };
        manager.update_status(order_id, &status_filled).unwrap();

        let events = manager.get_order_events(order_id);
        // Created (add_order), Submitted (Pending->Open), Filled (Open->Filled) 이벤트
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_cleanup_old_orders() {
        use chrono::Duration;

        let mut manager = OrderManager::new();

        // 오래된 주문 생성 및 체결
        let mut order = create_test_order(Side::Buy);
        order.updated_at = Utc::now() - Duration::days(10);
        order.status = OrderStatusType::Filled;
        let old_id = order.id;
        manager.orders.insert(old_id, order.clone());
        manager
            .orders_by_symbol
            .entry("BTC/USDT".to_string())
            .or_default()
            .push(old_id);

        // 최근 활성 주문 생성
        let recent_order = create_test_order(Side::Buy);
        manager.add_order(recent_order).unwrap();

        assert_eq!(manager.total_orders(), 2);

        // 5일보다 오래된 주문 정리
        manager.cleanup_old_orders(Utc::now() - Duration::days(5));

        assert_eq!(manager.total_orders(), 1);
    }
}
