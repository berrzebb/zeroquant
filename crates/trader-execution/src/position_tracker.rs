//! 포지션 추적 및 관리.
//!
//! 제공 기능:
//! - 주문 체결에 따른 실시간 포지션 업데이트
//! - 손익(PnL) 추적 및 계산
//! - 포지션 조회 및 집계

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use trader_core::{Order, Position, PositionSummary, Side};
use uuid::Uuid;

use crate::order_manager::OrderFill;

/// 포지션 트래커 에러 타입.
#[derive(Debug, Error)]
pub enum PositionTrackerError {
    #[error("Position not found: {0}")]
    PositionNotFound(Uuid),

    #[error("Position not found for symbol: {0}")]
    SymbolPositionNotFound(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Insufficient quantity: have {0}, need {1}")]
    InsufficientQuantity(Decimal, Decimal),
}

/// 포지션 이벤트 타입.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PositionEvent {
    /// 포지션 오픈
    Opened {
        position_id: Uuid,
        symbol: String,
        side: Side,
        quantity: Decimal,
        price: Decimal,
        timestamp: DateTime<Utc>,
    },
    /// 포지션 증가
    Increased {
        position_id: Uuid,
        quantity: Decimal,
        price: Decimal,
        new_total: Decimal,
        timestamp: DateTime<Utc>,
    },
    /// 포지션 감소
    Decreased {
        position_id: Uuid,
        quantity: Decimal,
        price: Decimal,
        realized_pnl: Decimal,
        remaining: Decimal,
        timestamp: DateTime<Utc>,
    },
    /// 포지션 종료
    Closed {
        position_id: Uuid,
        final_pnl: Decimal,
        timestamp: DateTime<Utc>,
    },
    /// 가격 업데이트
    PriceUpdated {
        position_id: Uuid,
        old_price: Decimal,
        new_price: Decimal,
        unrealized_pnl: Decimal,
        timestamp: DateTime<Utc>,
    },
}

impl PositionEvent {
    /// 이벤트에서 포지션 ID를 가져온다.
    pub fn position_id(&self) -> Uuid {
        match self {
            PositionEvent::Opened { position_id, .. } => *position_id,
            PositionEvent::Increased { position_id, .. } => *position_id,
            PositionEvent::Decreased { position_id, .. } => *position_id,
            PositionEvent::Closed { position_id, .. } => *position_id,
            PositionEvent::PriceUpdated { position_id, .. } => *position_id,
        }
    }

    /// 이벤트의 타임스탬프를 가져온다.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            PositionEvent::Opened { timestamp, .. } => *timestamp,
            PositionEvent::Increased { timestamp, .. } => *timestamp,
            PositionEvent::Decreased { timestamp, .. } => *timestamp,
            PositionEvent::Closed { timestamp, .. } => *timestamp,
            PositionEvent::PriceUpdated { timestamp, .. } => *timestamp,
        }
    }
}

/// 모든 포지션을 관리하는 포지션 트래커.
#[derive(Debug)]
pub struct PositionTracker {
    /// ID별 모든 포지션
    positions: HashMap<Uuid, Position>,
    /// 심볼별 오픈 포지션 (symbol -> position_id)
    positions_by_symbol: HashMap<String, Uuid>,
    /// 전략별 포지션
    positions_by_strategy: HashMap<String, Vec<Uuid>>,
    /// 포지션 히스토리 (종료된 포지션)
    closed_positions: Vec<Position>,
    /// 포지션 이벤트
    events: Vec<PositionEvent>,
    /// 거래소 이름
    exchange: String,
    /// 최대 히스토리 크기
    max_history_size: usize,
}

impl PositionTracker {
    /// 새 포지션 트래커를 생성한다.
    pub fn new(exchange: impl Into<String>) -> Self {
        Self {
            positions: HashMap::new(),
            positions_by_symbol: HashMap::new(),
            positions_by_strategy: HashMap::new(),
            closed_positions: Vec::new(),
            events: Vec::new(),
            exchange: exchange.into(),
            max_history_size: 10000,
        }
    }

    /// 커스텀 히스토리 크기로 생성한다.
    pub fn with_history_size(exchange: impl Into<String>, max_history_size: usize) -> Self {
        Self {
            max_history_size,
            ..Self::new(exchange)
        }
    }

    // ==================== 포지션 생성 ====================

    /// 새 포지션을 오픈한다.
    pub fn open_position(
        &mut self,
        symbol: String,
        side: Side,
        quantity: Decimal,
        price: Decimal,
        strategy_id: Option<String>,
    ) -> Result<Position, PositionTrackerError> {
        let symbol_str = symbol.to_string();

        // 이 심볼에 대한 포지션이 이미 존재하는지 확인
        if self.positions_by_symbol.contains_key(&symbol_str) {
            return Err(PositionTrackerError::InvalidOperation(format!(
                "Position already exists for {}. Use add_to_position instead.",
                symbol_str
            )));
        }

        let mut position = Position::new(&self.exchange, symbol, side, quantity, price);

        if let Some(ref strat_id) = strategy_id {
            position = position.with_strategy(strat_id.clone());
        }

        let position_id = position.id;
        let now = Utc::now();

        // 포지션 저장
        self.positions.insert(position_id, position.clone());
        self.positions_by_symbol
            .insert(symbol_str.clone(), position_id);

        // 전략별 인덱싱
        if let Some(strat_id) = strategy_id {
            self.positions_by_strategy
                .entry(strat_id)
                .or_default()
                .push(position_id);
        }

        // 이벤트 기록
        self.events.push(PositionEvent::Opened {
            position_id,
            symbol: symbol_str,
            side,
            quantity,
            price,
            timestamp: now,
        });

        self.trim_history();

        Ok(position)
    }

    /// 체결된 주문을 기반으로 포지션을 오픈하거나 추가한다.
    pub fn apply_fill(
        &mut self,
        order: &Order,
        fill: &OrderFill,
    ) -> Result<Position, PositionTrackerError> {
        let symbol_str = order.ticker.to_string();

        // 포지션이 존재하는지 확인
        if let Some(&pos_id) = self.positions_by_symbol.get(&symbol_str) {
            // 포지션이 존재함 - 추가하거나 감소
            let position = self
                .positions
                .get_mut(&pos_id)
                .ok_or(PositionTrackerError::PositionNotFound(pos_id))?;

            if position.side == order.side {
                // 같은 방향 - 포지션 증가
                self.increase_position_internal(pos_id, fill.quantity, fill.price)?;
            } else {
                // 반대 방향 - 포지션 감소
                self.reduce_position_internal(pos_id, fill.quantity, fill.price)?;
            }

            self.positions
                .get(&pos_id)
                .cloned()
                .ok_or(PositionTrackerError::PositionNotFound(pos_id))
        } else {
            // 포지션 없음 - 새로 오픈
            self.open_position(
                order.ticker.clone(),
                order.side,
                fill.quantity,
                fill.price,
                order.strategy_id.clone(),
            )
        }
    }

    // ==================== 포지션 업데이트 ====================

    /// 기존 포지션에 추가한다.
    pub fn add_to_position(
        &mut self,
        symbol: &str,
        quantity: Decimal,
        price: Decimal,
    ) -> Result<Position, PositionTrackerError> {
        let pos_id = self
            .positions_by_symbol
            .get(symbol)
            .copied()
            .ok_or_else(|| PositionTrackerError::SymbolPositionNotFound(symbol.to_string()))?;

        self.increase_position_internal(pos_id, quantity, price)?;
        self.positions
            .get(&pos_id)
            .cloned()
            .ok_or(PositionTrackerError::PositionNotFound(pos_id))
    }

    /// 포지션을 감소시킨다.
    pub fn reduce_position(
        &mut self,
        symbol: &str,
        quantity: Decimal,
        price: Decimal,
    ) -> Result<(Position, Decimal), PositionTrackerError> {
        let pos_id = self
            .positions_by_symbol
            .get(symbol)
            .copied()
            .ok_or_else(|| PositionTrackerError::SymbolPositionNotFound(symbol.to_string()))?;

        let pnl = self.reduce_position_internal(pos_id, quantity, price)?;
        // 열린 포지션에서 먼저 찾고, 없으면 종료된 포지션에서 찾기
        let position = self
            .positions
            .get(&pos_id)
            .cloned()
            .or_else(|| {
                self.closed_positions
                    .iter()
                    .find(|p| p.id == pos_id)
                    .cloned()
            })
            .ok_or(PositionTrackerError::PositionNotFound(pos_id))?;
        Ok((position, pnl))
    }

    /// 포지션을 완전히 종료한다.
    pub fn close_position(
        &mut self,
        symbol: &str,
        price: Decimal,
    ) -> Result<(Position, Decimal), PositionTrackerError> {
        let pos_id = self
            .positions_by_symbol
            .get(symbol)
            .copied()
            .ok_or_else(|| PositionTrackerError::SymbolPositionNotFound(symbol.to_string()))?;

        let position = self
            .positions
            .get(&pos_id)
            .ok_or(PositionTrackerError::PositionNotFound(pos_id))?;

        let quantity = position.quantity;
        self.reduce_position(symbol, quantity, price)
    }

    fn increase_position_internal(
        &mut self,
        position_id: Uuid,
        quantity: Decimal,
        price: Decimal,
    ) -> Result<(), PositionTrackerError> {
        let position = self
            .positions
            .get_mut(&position_id)
            .ok_or(PositionTrackerError::PositionNotFound(position_id))?;

        position.add(quantity, price);
        let new_total = position.quantity;
        let now = Utc::now();

        self.events.push(PositionEvent::Increased {
            position_id,
            quantity,
            price,
            new_total,
            timestamp: now,
        });

        self.trim_history();
        Ok(())
    }

    fn reduce_position_internal(
        &mut self,
        position_id: Uuid,
        quantity: Decimal,
        price: Decimal,
    ) -> Result<Decimal, PositionTrackerError> {
        let position = self
            .positions
            .get_mut(&position_id)
            .ok_or(PositionTrackerError::PositionNotFound(position_id))?;

        if quantity > position.quantity {
            return Err(PositionTrackerError::InsufficientQuantity(
                position.quantity,
                quantity,
            ));
        }

        let pnl = position.reduce(quantity, price);
        let remaining = position.quantity;
        let now = Utc::now();

        if position.is_closed() {
            // 포지션 완전 종료
            let final_pnl = position.realized_pnl;
            let symbol_str = position.ticker.to_string();

            // 종료된 포지션으로 이동
            let closed_position = self
                .positions
                .remove(&position_id)
                .ok_or(PositionTrackerError::PositionNotFound(position_id))?;
            self.closed_positions.push(closed_position);
            self.positions_by_symbol.remove(&symbol_str);

            // 전략 인덱스 업데이트
            if let Some(strat_id) = self
                .closed_positions
                .last()
                .and_then(|p| p.strategy_id.clone())
            {
                if let Some(ids) = self.positions_by_strategy.get_mut(&strat_id) {
                    ids.retain(|id| *id != position_id);
                }
            }

            self.events.push(PositionEvent::Closed {
                position_id,
                final_pnl,
                timestamp: now,
            });
        } else {
            self.events.push(PositionEvent::Decreased {
                position_id,
                quantity,
                price,
                realized_pnl: pnl,
                remaining,
                timestamp: now,
            });
        }

        self.trim_history();
        Ok(pnl)
    }

    /// 포지션의 가격을 업데이트한다.
    pub fn update_price(
        &mut self,
        symbol: &str,
        new_price: Decimal,
    ) -> Result<(), PositionTrackerError> {
        let pos_id = self
            .positions_by_symbol
            .get(symbol)
            .copied()
            .ok_or_else(|| PositionTrackerError::SymbolPositionNotFound(symbol.to_string()))?;

        let position = self
            .positions
            .get_mut(&pos_id)
            .ok_or(PositionTrackerError::PositionNotFound(pos_id))?;

        let old_price = position.current_price;
        position.update_price(new_price);
        let unrealized_pnl = position.unrealized_pnl;

        self.events.push(PositionEvent::PriceUpdated {
            position_id: pos_id,
            old_price,
            new_price,
            unrealized_pnl,
            timestamp: Utc::now(),
        });

        self.trim_history();
        Ok(())
    }

    /// 모든 포지션의 가격을 업데이트한다.
    pub fn update_prices(&mut self, prices: &HashMap<String, Decimal>) {
        for (symbol, price) in prices {
            let _ = self.update_price(symbol, *price);
        }
    }

    // ==================== 조회 ====================

    /// ID로 포지션을 가져온다.
    pub fn get_position(&self, position_id: Uuid) -> Option<&Position> {
        self.positions.get(&position_id)
    }

    /// 심볼로 포지션을 가져온다.
    pub fn get_position_for_symbol(&self, symbol: &str) -> Option<&Position> {
        self.positions_by_symbol
            .get(symbol)
            .and_then(|id| self.positions.get(id))
    }

    /// 모든 오픈 포지션을 가져온다.
    pub fn get_open_positions(&self) -> Vec<&Position> {
        self.positions.values().filter(|p| p.is_open()).collect()
    }

    /// 전략에 대한 포지션들을 가져온다.
    pub fn get_positions_for_strategy(&self, strategy_id: &str) -> Vec<&Position> {
        self.positions_by_strategy
            .get(strategy_id)
            .map(|ids| ids.iter().filter_map(|id| self.positions.get(id)).collect())
            .unwrap_or_default()
    }

    /// 종료된 포지션들을 가져온다.
    pub fn get_closed_positions(&self) -> &[Position] {
        &self.closed_positions
    }

    /// 심볼에 대한 오픈 포지션이 있는지 확인한다.
    pub fn has_position(&self, symbol: &str) -> bool {
        self.positions_by_symbol.contains_key(symbol)
    }

    /// 오픈 포지션의 총 개수를 가져온다.
    pub fn open_position_count(&self) -> usize {
        self.positions.len()
    }

    /// 모든 포지션을 벡터로 가져온다.
    pub fn get_all_positions(&self) -> Vec<&Position> {
        self.positions.values().collect()
    }

    // ==================== 통계 ====================

    /// 포지션 요약을 가져온다.
    pub fn get_summary(&self) -> PositionSummary {
        let positions: Vec<_> = self.positions.values().cloned().collect();
        PositionSummary::from_positions(&positions)
    }

    /// 총 미실현 손익을 가져온다.
    pub fn total_unrealized_pnl(&self) -> Decimal {
        self.positions.values().map(|p| p.unrealized_pnl).sum()
    }

    /// 총 실현 손익을 가져온다.
    pub fn total_realized_pnl(&self) -> Decimal {
        let open_realized: Decimal = self.positions.values().map(|p| p.realized_pnl).sum();
        let closed_realized: Decimal = self.closed_positions.iter().map(|p| p.realized_pnl).sum();
        open_realized + closed_realized
    }

    /// 총 명목 익스포저를 가져온다.
    pub fn total_exposure(&self) -> Decimal {
        self.positions.values().map(|p| p.notional_value()).sum()
    }

    /// 전략별 포트폴리오 손익을 가져온다.
    pub fn pnl_by_strategy(&self) -> HashMap<String, (Decimal, Decimal)> {
        let mut result: HashMap<String, (Decimal, Decimal)> = HashMap::new();

        for position in self.positions.values() {
            if let Some(ref strategy_id) = position.strategy_id {
                let entry = result
                    .entry(strategy_id.clone())
                    .or_insert((Decimal::ZERO, Decimal::ZERO));
                entry.0 += position.unrealized_pnl;
                entry.1 += position.realized_pnl;
            }
        }

        for position in &self.closed_positions {
            if let Some(ref strategy_id) = position.strategy_id {
                let entry = result
                    .entry(strategy_id.clone())
                    .or_insert((Decimal::ZERO, Decimal::ZERO));
                entry.1 += position.realized_pnl;
            }
        }

        result
    }

    /// 포지션 이벤트들을 가져온다.
    pub fn get_events(&self) -> &[PositionEvent] {
        &self.events
    }

    /// 포지션에 대한 이벤트들을 가져온다.
    pub fn get_position_events(&self, position_id: Uuid) -> Vec<&PositionEvent> {
        self.events
            .iter()
            .filter(|e| e.position_id() == position_id)
            .collect()
    }

    // ==================== 내부 ====================

    fn trim_history(&mut self) {
        if self.events.len() > self.max_history_size {
            let drain_count = self.events.len() - self.max_history_size;
            self.events.drain(0..drain_count);
        }
        if self.closed_positions.len() > self.max_history_size {
            let drain_count = self.closed_positions.len() - self.max_history_size;
            self.closed_positions.drain(0..drain_count);
        }
    }

    /// 오래된 종료 포지션을 정리한다.
    pub fn cleanup_old_positions(&mut self, older_than: DateTime<Utc>) {
        self.closed_positions
            .retain(|p| p.closed_at.map(|t| t >= older_than).unwrap_or(true));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::prelude::FromPrimitive;

    /// Decimal 생성을 위한 헬퍼 매크로
    macro_rules! dec {
        ($val:expr) => {
            Decimal::from_f64($val as f64).unwrap()
        };
    }

    fn create_test_symbol() -> String {
        "BTC/USDT".to_string()
    }

    #[test]
    fn test_open_position() {
        let mut tracker = PositionTracker::new("binance");
        let symbol = create_test_symbol();

        let position = tracker
            .open_position(
                symbol,
                Side::Buy,
                dec!(0.1),
                dec!(50000),
                Some("grid".to_string()),
            )
            .unwrap();

        assert_eq!(position.quantity, dec!(0.1));
        assert_eq!(position.entry_price, dec!(50000));
        assert_eq!(position.side, Side::Buy);
        assert_eq!(tracker.open_position_count(), 1);
    }

    #[test]
    fn test_duplicate_position_fails() {
        let mut tracker = PositionTracker::new("binance");
        let symbol = create_test_symbol();

        tracker
            .open_position(symbol.clone(), Side::Buy, dec!(0.1), dec!(50000), None)
            .unwrap();

        let result = tracker.open_position(symbol, Side::Buy, dec!(0.1), dec!(51000), None);
        assert!(matches!(
            result,
            Err(PositionTrackerError::InvalidOperation(_))
        ));
    }

    #[test]
    fn test_add_to_position() {
        let mut tracker = PositionTracker::new("binance");
        let symbol = create_test_symbol();

        tracker
            .open_position(symbol, Side::Buy, dec!(0.1), dec!(50000), None)
            .unwrap();

        let position = tracker
            .add_to_position("BTC/USDT", dec!(0.1), dec!(52000))
            .unwrap();

        assert_eq!(position.quantity, dec!(0.2));
        // 평균가: (50000*0.1 + 52000*0.1) / 0.2 = 51000
        assert_eq!(position.entry_price, dec!(51000));
    }

    #[test]
    fn test_reduce_position() {
        let mut tracker = PositionTracker::new("binance");
        let symbol = create_test_symbol();

        tracker
            .open_position(symbol, Side::Buy, dec!(0.2), dec!(50000), None)
            .unwrap();

        let (position, pnl) = tracker
            .reduce_position("BTC/USDT", dec!(0.1), dec!(55000))
            .unwrap();

        // 손익: (55000 - 50000) * 0.1 = 500
        assert_eq!(pnl, dec!(500));
        assert_eq!(position.quantity, dec!(0.1));
        assert_eq!(position.realized_pnl, dec!(500));
        assert!(tracker.has_position("BTC/USDT"));
    }

    #[test]
    fn test_close_position() {
        let mut tracker = PositionTracker::new("binance");
        let symbol = create_test_symbol();

        tracker
            .open_position(symbol, Side::Buy, dec!(0.1), dec!(50000), None)
            .unwrap();

        let (position, pnl) = tracker.close_position("BTC/USDT", dec!(55000)).unwrap();

        assert_eq!(pnl, dec!(500));
        assert!(position.is_closed());
        assert!(!tracker.has_position("BTC/USDT"));
        assert_eq!(tracker.open_position_count(), 0);
        assert_eq!(tracker.get_closed_positions().len(), 1);
    }

    #[test]
    fn test_short_position_pnl() {
        let mut tracker = PositionTracker::new("binance");
        let symbol = create_test_symbol();

        tracker
            .open_position(symbol, Side::Sell, dec!(0.1), dec!(50000), None)
            .unwrap();

        // 가격 하락 - 숏 포지션 수익
        let (_, pnl) = tracker.close_position("BTC/USDT", dec!(48000)).unwrap();

        // 손익: (50000 - 48000) * 0.1 = 200
        assert_eq!(pnl, dec!(200));
    }

    #[test]
    fn test_update_price() {
        let mut tracker = PositionTracker::new("binance");
        let symbol = create_test_symbol();

        tracker
            .open_position(symbol, Side::Buy, dec!(0.1), dec!(50000), None)
            .unwrap();

        tracker.update_price("BTC/USDT", dec!(55000)).unwrap();

        let position = tracker.get_position_for_symbol("BTC/USDT").unwrap();
        assert_eq!(position.current_price, dec!(55000));
        assert_eq!(position.unrealized_pnl, dec!(500));
    }

    #[test]
    fn test_total_pnl() {
        let mut tracker = PositionTracker::new("binance");

        // BTC 포지션 오픈
        tracker
            .open_position(
                "BTC/USDT".to_string(),
                Side::Buy,
                dec!(0.1),
                dec!(50000),
                None,
            )
            .unwrap();

        // ETH 포지션 오픈
        tracker
            .open_position(
                "ETH/USDT".to_string(),
                Side::Buy,
                dec!(1.0),
                dec!(3000),
                None,
            )
            .unwrap();

        // 가격 업데이트
        let mut prices = HashMap::new();
        prices.insert("BTC/USDT".to_string(), dec!(55000));
        prices.insert("ETH/USDT".to_string(), dec!(3200));
        tracker.update_prices(&prices);

        // 총 미실현 손익: 500 (BTC) + 200 (ETH) = 700
        assert_eq!(tracker.total_unrealized_pnl(), dec!(700));
    }

    #[test]
    fn test_position_summary() {
        let mut tracker = PositionTracker::new("binance");

        tracker
            .open_position(
                "BTC/USDT".to_string(),
                Side::Buy,
                dec!(0.1),
                dec!(50000),
                None,
            )
            .unwrap();

        tracker
            .open_position(
                "ETH/USDT".to_string(),
                Side::Sell,
                dec!(1.0),
                dec!(3000),
                None,
            )
            .unwrap();

        let summary = tracker.get_summary();
        assert_eq!(summary.total_positions, 2);
        assert_eq!(summary.long_count, 1);
        assert_eq!(summary.short_count, 1);
        // 명목가치: 5000 (BTC) + 3000 (ETH) = 8000
        assert_eq!(summary.total_notional_value, dec!(8000));
    }

    #[test]
    fn test_pnl_by_strategy() {
        let mut tracker = PositionTracker::new("binance");

        // Grid 전략 포지션
        tracker
            .open_position(
                "BTC/USDT".to_string(),
                Side::Buy,
                dec!(0.1),
                dec!(50000),
                Some("grid".to_string()),
            )
            .unwrap();

        // RSI 전략 포지션
        tracker
            .open_position(
                "ETH/USDT".to_string(),
                Side::Buy,
                dec!(1.0),
                dec!(3000),
                Some("rsi".to_string()),
            )
            .unwrap();

        // 가격 업데이트
        tracker.update_price("BTC/USDT", dec!(55000)).unwrap();
        tracker.update_price("ETH/USDT", dec!(2800)).unwrap();

        let pnl_map = tracker.pnl_by_strategy();

        assert_eq!(pnl_map.get("grid").unwrap().0, dec!(500)); // 미실현 손익
        assert_eq!(pnl_map.get("rsi").unwrap().0, dec!(-200)); // 미실현 손익 (손실)
    }

    #[test]
    fn test_position_events() {
        let mut tracker = PositionTracker::new("binance");
        let symbol = create_test_symbol();

        tracker
            .open_position(symbol, Side::Buy, dec!(0.2), dec!(50000), None)
            .unwrap();

        tracker
            .add_to_position("BTC/USDT", dec!(0.1), dec!(51000))
            .unwrap();

        tracker.update_price("BTC/USDT", dec!(52000)).unwrap();

        let (_, _) = tracker
            .reduce_position("BTC/USDT", dec!(0.1), dec!(52000))
            .unwrap();

        let events = tracker.get_events();
        assert_eq!(events.len(), 4); // 오픈, 증가, 가격업데이트, 감소
    }

    #[test]
    fn test_insufficient_quantity() {
        let mut tracker = PositionTracker::new("binance");
        let symbol = create_test_symbol();

        tracker
            .open_position(symbol, Side::Buy, dec!(0.1), dec!(50000), None)
            .unwrap();

        let result = tracker.reduce_position("BTC/USDT", dec!(0.2), dec!(51000));
        assert!(matches!(
            result,
            Err(PositionTrackerError::InsufficientQuantity(_, _))
        ));
    }

    #[test]
    fn test_apply_fill_new_position() {
        let mut tracker = PositionTracker::new("binance");

        let order = trader_core::Order::from_request(
            trader_core::OrderRequest::market_buy("BTC/USDT".to_string(), dec!(0.1))
                .with_strategy("grid"),
            "binance",
        );

        let fill = OrderFill {
            order_id: order.id,
            quantity: dec!(0.1),
            price: dec!(50000),
            commission: None,
            commission_asset: None,
            timestamp: Utc::now(),
        };

        let position = tracker.apply_fill(&order, &fill).unwrap();

        assert_eq!(position.quantity, dec!(0.1));
        assert_eq!(position.entry_price, dec!(50000));
        assert_eq!(position.strategy_id, Some("grid".to_string()));
    }

    #[test]
    fn test_apply_fill_increase_position() {
        let mut tracker = PositionTracker::new("binance");
        let ticker = "BTC/USDT".to_string();

        // 초기 포지션
        tracker
            .open_position(ticker.clone(), Side::Buy, dec!(0.1), dec!(50000), None)
            .unwrap();

        // 추가 매수 주문
        let order = trader_core::Order::from_request(
            trader_core::OrderRequest::market_buy(ticker.clone(), dec!(0.1)),
            "binance",
        );

        let fill = OrderFill {
            order_id: order.id,
            quantity: dec!(0.1),
            price: dec!(51000),
            commission: None,
            commission_asset: None,
            timestamp: Utc::now(),
        };

        let position = tracker.apply_fill(&order, &fill).unwrap();

        assert_eq!(position.quantity, dec!(0.2));
    }

    #[test]
    fn test_apply_fill_reduce_position() {
        let mut tracker = PositionTracker::new("binance");
        let symbol = "BTC/USDT".to_string();

        // 초기 롱 포지션
        tracker
            .open_position(symbol.clone(), Side::Buy, dec!(0.2), dec!(50000), None)
            .unwrap();

        // 매도 주문 (포지션 감소)
        let order = trader_core::Order::from_request(
            trader_core::OrderRequest::market_sell(symbol.clone(), dec!(0.1)),
            "binance",
        );

        let fill = OrderFill {
            order_id: order.id,
            quantity: dec!(0.1),
            price: dec!(55000),
            commission: None,
            commission_asset: None,
            timestamp: Utc::now(),
        };

        let position = tracker.apply_fill(&order, &fill).unwrap();

        assert_eq!(position.quantity, dec!(0.1));
        assert_eq!(position.realized_pnl, dec!(500)); // 손익: (55000-50000)*0.1
    }
}
