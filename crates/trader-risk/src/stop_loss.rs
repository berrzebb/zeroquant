//! 손절매 및 이익실현 관리.
//!
//! 제공 기능:
//! - 고정 손절매 주문 생성
//! - 이익실현 주문 생성
//! - 동적 조정이 가능한 추적 손절매
//! - 변동성 조정 리스크 관리를 위한 ATR 기반 스탑

use crate::config::RiskConfig;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use trader_core::{OrderRequest, OrderType, Position, Side, TimeInForce};

/// 정수 연산을 사용하여 가격에 백분율 조정을 적용.
/// 예시: apply_pct(50000, -2.0) = 49000 (2% 감소)
/// 예시: apply_pct(50000, 5.0) = 52500 (5% 증가)
fn apply_pct(price: Decimal, pct: f64) -> Decimal {
    // 백분율을 정수로 스케일링 (백분율에서 소수점 4자리까지 지원)
    // pct는 변화 백분율 (증가는 양수, 감소는 음수)
    // 공식: price * (1 + pct/100) = price * (100 + pct) / 100
    let scaled_factor = ((100.0 + pct) * 10000.0).round() as i64;
    (price * Decimal::from(scaled_factor)) / Decimal::from(1_000_000)
}

/// 스탑 주문 유형 열거형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopType {
    /// 특정 가격의 고정 손절매
    FixedStopLoss,
    /// 특정 가격의 고정 이익실현
    FixedTakeProfit,
    /// 가격과 함께 움직이는 추적 손절매
    TrailingStop,
    /// ATR 기반 손절매
    AtrStop,
}

/// 스탑 주문 명세.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopOrder {
    /// 스탑 주문 유형
    pub stop_type: StopType,
    /// 스탑 주문 심볼
    pub symbol: String,
    /// 방향 (청산 시 포지션 방향의 반대)
    pub side: Side,
    /// 청산 수량
    pub quantity: Decimal,
    /// 트리거 가격
    pub trigger_price: Decimal,
    /// 지정가 (스탑-리밋 주문용)
    pub limit_price: Option<Decimal>,
    /// 원래 진입 가격 (참조용)
    pub entry_price: Decimal,
    /// 연관된 포지션 ID
    pub position_id: Option<String>,
    /// 감소 전용 주문 여부
    pub reduce_only: bool,
}

impl StopOrder {
    /// 새 손절매 주문 생성.
    pub fn stop_loss(
        symbol: String,
        position_side: Side,
        quantity: Decimal,
        trigger_price: Decimal,
        entry_price: Decimal,
    ) -> Self {
        Self {
            stop_type: StopType::FixedStopLoss,
            symbol,
            side: position_side.opposite(),
            quantity,
            trigger_price,
            limit_price: None,
            entry_price,
            position_id: None,
            reduce_only: true,
        }
    }

    /// 새 이익실현 주문 생성.
    pub fn take_profit(
        symbol: String,
        position_side: Side,
        quantity: Decimal,
        trigger_price: Decimal,
        entry_price: Decimal,
    ) -> Self {
        Self {
            stop_type: StopType::FixedTakeProfit,
            symbol,
            side: position_side.opposite(),
            quantity,
            trigger_price,
            limit_price: None,
            entry_price,
            position_id: None,
            reduce_only: true,
        }
    }

    /// 추적 손절매 주문 생성.
    pub fn trailing_stop(
        symbol: String,
        position_side: Side,
        quantity: Decimal,
        trigger_price: Decimal,
        entry_price: Decimal,
    ) -> Self {
        Self {
            stop_type: StopType::TrailingStop,
            symbol,
            side: position_side.opposite(),
            quantity,
            trigger_price,
            limit_price: None,
            entry_price,
            position_id: None,
            reduce_only: true,
        }
    }

    /// 스탑-리밋 주문의 지정가 설정.
    pub fn with_limit_price(mut self, limit_price: Decimal) -> Self {
        self.limit_price = Some(limit_price);
        self
    }

    /// 포지션 ID 설정.
    pub fn with_position_id(mut self, position_id: impl Into<String>) -> Self {
        self.position_id = Some(position_id.into());
        self
    }

    /// OrderRequest로 변환.
    pub fn to_order_request(&self) -> OrderRequest {
        match self.limit_price {
            Some(limit) => OrderRequest {
                ticker: self.symbol.clone(),
                side: self.side,
                order_type: OrderType::StopLossLimit,
                quantity: self.quantity,
                price: Some(limit),
                stop_price: Some(self.trigger_price),
                time_in_force: TimeInForce::GTC,
                client_order_id: None,
                strategy_id: None,
            },
            None => OrderRequest {
                ticker: self.symbol.clone(),
                side: self.side,
                order_type: OrderType::StopLoss,
                quantity: self.quantity,
                price: None,
                stop_price: Some(self.trigger_price),
                time_in_force: TimeInForce::GTC,
                client_order_id: None,
                strategy_id: None,
            },
        }
    }

    /// 트리거 시 손익 금액 계산.
    pub fn calculate_pnl(&self) -> Decimal {
        let price_diff = match self.side.opposite() {
            // 원래 포지션 방향
            Side::Buy => self.trigger_price - self.entry_price,
            Side::Sell => self.entry_price - self.trigger_price,
        };
        price_diff * self.quantity
    }

    /// 트리거 시 손익 백분율 계산.
    pub fn calculate_pnl_pct(&self) -> f64 {
        if self.entry_price.is_zero() {
            return 0.0;
        }
        let pnl = self.calculate_pnl();
        let notional = self.entry_price * self.quantity;
        (pnl / notional * Decimal::from(100))
            .to_f64()
            .unwrap_or(0.0)
    }

    /// 손절매 여부 확인.
    pub fn is_stop_loss(&self) -> bool {
        matches!(
            self.stop_type,
            StopType::FixedStopLoss | StopType::TrailingStop | StopType::AtrStop
        )
    }

    /// 이익실현 여부 확인.
    pub fn is_take_profit(&self) -> bool {
        matches!(self.stop_type, StopType::FixedTakeProfit)
    }
}

/// 추적 및 업데이트를 위한 추적 손절매 상태.
#[derive(Debug, Clone)]
pub struct TrailingStopState {
    /// 현재 트리거 가격
    pub trigger_price: Decimal,
    /// 관측된 최적 가격 (롱은 최고가, 숏은 최저가)
    pub best_price: Decimal,
    /// 추적 거리 (절대값 또는 백분율)
    pub trail_distance: Decimal,
    /// trail_distance가 백분율인지 여부
    pub is_percentage: bool,
    /// 포지션 방향
    pub position_side: Side,
    /// 스탑 활성화 여부 (가격이 유리한 방향으로 이동)
    pub activated: bool,
}

impl TrailingStopState {
    /// 새 추적 손절매 상태 생성.
    pub fn new(
        initial_trigger: Decimal,
        trail_distance: Decimal,
        is_percentage: bool,
        position_side: Side,
    ) -> Self {
        Self {
            trigger_price: initial_trigger,
            best_price: match position_side {
                Side::Buy => initial_trigger + trail_distance,
                Side::Sell => initial_trigger - trail_distance,
            },
            trail_distance,
            is_percentage,
            position_side,
            activated: false,
        }
    }

    /// 새 가격을 기반으로 추적 손절매 업데이트.
    ///
    /// 트리거 가격이 업데이트되면 true 반환.
    pub fn update(&mut self, current_price: Decimal) -> bool {
        match self.position_side {
            Side::Buy => {
                // 롱 포지션: 최고가 아래에서 추적
                if current_price > self.best_price {
                    self.best_price = current_price;
                    let new_trigger = if self.is_percentage {
                        current_price * (Decimal::ONE - self.trail_distance / Decimal::from(100))
                    } else {
                        current_price - self.trail_distance
                    };
                    if new_trigger > self.trigger_price {
                        self.trigger_price = new_trigger;
                        self.activated = true;
                        return true;
                    }
                }
            }
            Side::Sell => {
                // 숏 포지션: 최저가 위에서 추적
                if current_price < self.best_price {
                    self.best_price = current_price;
                    let new_trigger = if self.is_percentage {
                        current_price * (Decimal::ONE + self.trail_distance / Decimal::from(100))
                    } else {
                        current_price + self.trail_distance
                    };
                    if new_trigger < self.trigger_price {
                        self.trigger_price = new_trigger;
                        self.activated = true;
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 스탑이 트리거되어야 하는지 확인.
    pub fn should_trigger(&self, current_price: Decimal) -> bool {
        match self.position_side {
            Side::Buy => current_price <= self.trigger_price,
            Side::Sell => current_price >= self.trigger_price,
        }
    }
}

/// 손절매 및 이익실현 주문 생성기.
#[derive(Debug, Clone)]
pub struct StopOrderGenerator {
    config: RiskConfig,
}

impl StopOrderGenerator {
    /// 새 스탑 주문 생성기 생성.
    pub fn new(config: RiskConfig) -> Self {
        Self { config }
    }

    /// 포지션에 대한 손절매 주문 생성.
    ///
    /// # 인자
    /// * `position` - 보호할 포지션
    /// * `stop_loss_pct` - 선택적 사용자 정의 손절매 백분율 (None이면 설정 기본값 사용)
    pub fn generate_stop_loss(&self, position: &Position, stop_loss_pct: Option<f64>) -> StopOrder {
        let symbol_str = &position.ticker;
        let pct = stop_loss_pct.unwrap_or_else(|| self.config.get_stop_loss_pct(symbol_str));

        let trigger_price = match position.side {
            Side::Buy => {
                // 롱 포지션: 진입가 아래에 손절매
                apply_pct(position.entry_price, -pct)
            }
            Side::Sell => {
                // 숏 포지션: 진입가 위에 손절매
                apply_pct(position.entry_price, pct)
            }
        };

        StopOrder::stop_loss(
            position.ticker.clone(),
            position.side,
            position.quantity,
            trigger_price,
            position.entry_price,
        )
    }

    /// 포지션에 대한 이익실현 주문 생성.
    ///
    /// # 인자
    /// * `position` - 이익실현을 설정할 포지션
    /// * `take_profit_pct` - 선택적 사용자 정의 이익실현 백분율 (None이면 설정 기본값 사용)
    pub fn generate_take_profit(
        &self,
        position: &Position,
        take_profit_pct: Option<f64>,
    ) -> StopOrder {
        let symbol_str = &position.ticker;
        let pct = take_profit_pct.unwrap_or_else(|| self.config.get_take_profit_pct(symbol_str));

        let trigger_price = match position.side {
            Side::Buy => {
                // 롱 포지션: 진입가 위에 이익실현
                apply_pct(position.entry_price, pct)
            }
            Side::Sell => {
                // 숏 포지션: 진입가 아래에 이익실현
                apply_pct(position.entry_price, -pct)
            }
        };

        StopOrder::take_profit(
            position.ticker.clone(),
            position.side,
            position.quantity,
            trigger_price,
            position.entry_price,
        )
    }

    /// 포지션에 대한 손절매 및 이익실현 주문 모두 생성.
    pub fn generate_bracket_orders(
        &self,
        position: &Position,
        stop_loss_pct: Option<f64>,
        take_profit_pct: Option<f64>,
    ) -> (StopOrder, StopOrder) {
        let sl = self.generate_stop_loss(position, stop_loss_pct);
        let tp = self.generate_take_profit(position, take_profit_pct);
        (sl, tp)
    }

    /// 포지션에 대한 추적 손절매 생성.
    ///
    /// # 인자
    /// * `position` - 보호할 포지션
    /// * `trail_pct` - 백분율로 표시된 추적 거리
    /// * `current_price` - 현재 시장 가격
    pub fn generate_trailing_stop(
        &self,
        position: &Position,
        trail_pct: f64,
        current_price: Decimal,
    ) -> (StopOrder, TrailingStopState) {
        let trigger_price = match position.side {
            Side::Buy => apply_pct(current_price, -trail_pct),
            Side::Sell => apply_pct(current_price, trail_pct),
        };

        let order = StopOrder::trailing_stop(
            position.ticker.clone(),
            position.side,
            position.quantity,
            trigger_price,
            position.entry_price,
        );

        // 정밀도를 위해 trail_pct를 정수로 스케일링 (소수점 4자리까지 지원)
        let trail_decimal =
            Decimal::from((trail_pct * 10000.0).round() as i64) / Decimal::from(10000);

        let state = TrailingStopState::new(trigger_price, trail_decimal, true, position.side);

        (order, state)
    }

    /// ATR 기반 손절매 가격 계산.
    ///
    /// # 인자
    /// * `entry_price` - 진입 가격
    /// * `atr` - Average True Range 값
    /// * `multiplier` - ATR 승수 (예: 2x ATR은 2.0)
    /// * `side` - 포지션 방향
    pub fn calculate_atr_stop(
        &self,
        entry_price: Decimal,
        atr: Decimal,
        multiplier: f64,
        side: Side,
    ) -> Decimal {
        let atr_distance = atr * Decimal::from_f64_retain(multiplier).unwrap_or(Decimal::from(2));

        match side {
            Side::Buy => entry_price - atr_distance,
            Side::Sell => entry_price + atr_distance,
        }
    }

    /// ATR 기반 손절매 주문 생성.
    ///
    /// # 인자
    /// * `position` - 보호할 포지션
    /// * `atr` - 현재 ATR 값
    /// * `multiplier` - ATR 승수 (기본값: 2.0)
    pub fn generate_atr_stop(
        &self,
        position: &Position,
        atr: Decimal,
        multiplier: Option<f64>,
    ) -> StopOrder {
        let mult = multiplier.unwrap_or(2.0);
        let trigger_price = self.calculate_atr_stop(position.entry_price, atr, mult, position.side);

        let mut order = StopOrder::stop_loss(
            position.ticker.clone(),
            position.side,
            position.quantity,
            trigger_price,
            position.entry_price,
        );
        order.stop_type = StopType::AtrStop;
        order
    }

    /// 손익비 계산.
    pub fn calculate_risk_reward(
        entry_price: Decimal,
        stop_loss_price: Decimal,
        take_profit_price: Decimal,
        side: Side,
    ) -> f64 {
        let risk = match side {
            Side::Buy => entry_price - stop_loss_price,
            Side::Sell => stop_loss_price - entry_price,
        };

        let reward = match side {
            Side::Buy => take_profit_price - entry_price,
            Side::Sell => entry_price - take_profit_price,
        };

        if risk.is_zero() || risk < Decimal::ZERO {
            return 0.0;
        }

        (reward / risk).to_f64().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use trader_core::Symbol;

    fn create_test_position(side: Side, quantity: Decimal, entry_price: Decimal) -> Position {
        let symbol = "BTC/USDT".to_string();
        Position::new("test_exchange", symbol, side, quantity, entry_price)
    }

    #[test]
    fn test_stop_loss_order_creation() {
        let symbol = "BTC/USDT".to_string();
        let order = StopOrder::stop_loss(
            symbol.clone(),
            Side::Buy,
            dec!(0.1),
            dec!(49000),
            dec!(50000),
        );

        assert_eq!(order.stop_type, StopType::FixedStopLoss);
        assert_eq!(order.side, Side::Sell); // 포지션 방향의 반대
        assert_eq!(order.trigger_price, dec!(49000));
        assert!(order.reduce_only);
    }

    #[test]
    fn test_take_profit_order_creation() {
        let symbol = "BTC/USDT".to_string();
        let order = StopOrder::take_profit(
            symbol.clone(),
            Side::Buy,
            dec!(0.1),
            dec!(55000),
            dec!(50000),
        );

        assert_eq!(order.stop_type, StopType::FixedTakeProfit);
        assert_eq!(order.side, Side::Sell);
        assert_eq!(order.trigger_price, dec!(55000));
    }

    #[test]
    fn test_stop_order_pnl_calculation() {
        let symbol = "BTC/USDT".to_string();

        // 롱 포지션 손절매
        let sl = StopOrder::stop_loss(
            symbol.clone(),
            Side::Buy,
            dec!(0.1),
            dec!(49000), // 진입가 $1000 아래
            dec!(50000),
        );

        let pnl = sl.calculate_pnl();
        assert_eq!(pnl, dec!(-100)); // $100 손실

        // 롱 포지션 이익실현
        let tp = StopOrder::take_profit(
            symbol.clone(),
            Side::Buy,
            dec!(0.1),
            dec!(55000), // 진입가 $5000 위
            dec!(50000),
        );

        let pnl = tp.calculate_pnl();
        assert_eq!(pnl, dec!(500)); // $500 수익
    }

    #[test]
    fn test_trailing_stop_state_long() {
        let mut state = TrailingStopState::new(
            dec!(49000), // 초기 트리거
            dec!(2),     // 2% 추적
            true,
            Side::Buy,
        );

        // 가격 상승 - 추적 손절매 업데이트 필요
        assert!(state.update(dec!(52000)));
        assert!(state.trigger_price > dec!(49000));
        assert!(state.activated);

        // 가격 하락 - 추적 손절매 업데이트 불필요
        let prev_trigger = state.trigger_price;
        assert!(!state.update(dec!(51000)));
        assert_eq!(state.trigger_price, prev_trigger);
    }

    #[test]
    fn test_trailing_stop_state_short() {
        let mut state = TrailingStopState::new(
            dec!(51000), // 초기 트리거
            dec!(2),     // 2% 추적
            true,
            Side::Sell,
        );

        // 가격 하락 - 추적 손절매 업데이트 필요
        assert!(state.update(dec!(48000)));
        assert!(state.trigger_price < dec!(51000));

        // 가격 상승 - 추적 손절매 업데이트 불필요
        let prev_trigger = state.trigger_price;
        assert!(!state.update(dec!(49000)));
        assert_eq!(state.trigger_price, prev_trigger);
    }

    #[test]
    fn test_trailing_stop_trigger_detection() {
        let state = TrailingStopState::new(dec!(49000), dec!(2), true, Side::Buy);

        assert!(!state.should_trigger(dec!(50000))); // 트리거 가격 위
        assert!(state.should_trigger(dec!(49000))); // 트리거 가격
        assert!(state.should_trigger(dec!(48000))); // 트리거 가격 아래
    }

    #[test]
    fn test_generator_stop_loss_long() {
        let config = RiskConfig::default(); // 2% 기본 손절매
        let generator = StopOrderGenerator::new(config);

        let position = create_test_position(Side::Buy, dec!(0.1), dec!(50000));
        let order = generator.generate_stop_loss(&position, None);

        assert_eq!(order.stop_type, StopType::FixedStopLoss);
        assert_eq!(order.side, Side::Sell);
        // 50000의 2% 아래 = 49000
        assert_eq!(order.trigger_price, dec!(49000));
    }

    #[test]
    fn test_generator_stop_loss_short() {
        let config = RiskConfig::default();
        let generator = StopOrderGenerator::new(config);

        let position = create_test_position(Side::Sell, dec!(0.1), dec!(50000));
        let order = generator.generate_stop_loss(&position, None);

        assert_eq!(order.side, Side::Buy);
        // 50000의 2% 위 = 51000
        assert_eq!(order.trigger_price, dec!(51000));
    }

    #[test]
    fn test_generator_take_profit_long() {
        let config = RiskConfig::default(); // 5% 기본 이익실현
        let generator = StopOrderGenerator::new(config);

        let position = create_test_position(Side::Buy, dec!(0.1), dec!(50000));
        let order = generator.generate_take_profit(&position, None);

        assert_eq!(order.stop_type, StopType::FixedTakeProfit);
        assert_eq!(order.side, Side::Sell);
        // 50000의 5% 위 = 52500
        assert_eq!(order.trigger_price, dec!(52500));
    }

    #[test]
    fn test_generator_bracket_orders() {
        let config = RiskConfig::default();
        let generator = StopOrderGenerator::new(config);

        let position = create_test_position(Side::Buy, dec!(0.1), dec!(50000));
        let (sl, tp) = generator.generate_bracket_orders(&position, None, None);

        assert!(sl.is_stop_loss());
        assert!(tp.is_take_profit());
        assert!(sl.trigger_price < position.entry_price);
        assert!(tp.trigger_price > position.entry_price);
    }

    #[test]
    fn test_generator_atr_stop() {
        let config = RiskConfig::default();
        let generator = StopOrderGenerator::new(config);

        let position = create_test_position(Side::Buy, dec!(0.1), dec!(50000));
        let atr = dec!(1000); // ATR $1000
        let order = generator.generate_atr_stop(&position, atr, Some(2.0));

        assert_eq!(order.stop_type, StopType::AtrStop);
        // 2x ATR = 진입가 아래 $2000 = $48000
        assert_eq!(order.trigger_price, dec!(48000));
    }

    #[test]
    fn test_risk_reward_calculation() {
        let rr = StopOrderGenerator::calculate_risk_reward(
            dec!(50000), // 진입가
            dec!(49000), // 손절매 ($1000 리스크)
            dec!(52000), // 이익실현 ($2000 보상)
            Side::Buy,
        );

        assert!((rr - 2.0).abs() < 0.01); // 2:1 손익비
    }

    #[test]
    fn test_stop_order_to_order_request() {
        let symbol = Symbol::crypto("BTC", "USDT");
        let symbol_str = symbol.to_string();
        let order = StopOrder::stop_loss(
            symbol_str.clone(),
            Side::Buy,
            dec!(0.1),
            dec!(49000),
            dec!(50000),
        );

        let request = order.to_order_request();

        assert_eq!(request.ticker, symbol_str);
        assert_eq!(request.side, Side::Sell);
        assert_eq!(request.quantity, dec!(0.1));
    }

    #[test]
    fn test_custom_stop_percentages() {
        let config = RiskConfig::default();
        let generator = StopOrderGenerator::new(config);

        let position = create_test_position(Side::Buy, dec!(0.1), dec!(50000));

        // 사용자 정의 1% 손절매
        let order = generator.generate_stop_loss(&position, Some(1.0));
        assert_eq!(order.trigger_price, dec!(49500)); // 50000의 1% 아래

        // 사용자 정의 10% 이익실현
        let order = generator.generate_take_profit(&position, Some(10.0));
        assert_eq!(order.trigger_price, dec!(55000)); // 50000의 10% 위
    }
}
