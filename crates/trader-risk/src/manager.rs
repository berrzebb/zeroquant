//! 리스크 매니저 구현.
//!
//! 모든 리스크 관리 작업을 위한 통합 인터페이스 제공:
//! - 포지션 크기 제한에 대한 주문 검증
//! - 일일 손실 한도 추적
//! - Stop-loss/Take-profit 주문 생성
//! - 변동성 필터링

use crate::config::RiskConfig;
use crate::limits::DailyLossTracker;
use crate::position_sizing::PositionSizer;
use crate::stop_loss::{StopOrder, StopOrderGenerator, TrailingStopState};
use rust_decimal::Decimal;
use std::collections::HashMap;
use trader_core::{OrderRequest, Position, TraderResult};

/// 리스크 검증 결과.
#[derive(Debug, Clone)]
pub struct RiskValidation {
    /// 주문이 리스크 검사를 통과했는지 여부
    pub is_valid: bool,
    /// 검증 메시지/경고
    pub messages: Vec<String>,
    /// 수정된 주문 (조정이 이루어진 경우)
    pub modified_order: Option<OrderRequest>,
}

impl RiskValidation {
    /// 유효한 결과 생성.
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            messages: vec![],
            modified_order: None,
        }
    }

    /// 무효한 결과 생성.
    pub fn invalid(reason: impl Into<String>) -> Self {
        Self {
            is_valid: false,
            messages: vec![reason.into()],
            modified_order: None,
        }
    }

    /// 경고 메시지 추가.
    pub fn with_warning(mut self, message: impl Into<String>) -> Self {
        self.messages.push(message.into());
        self
    }

    /// 수정된 주문 설정.
    pub fn with_modified_order(mut self, order: OrderRequest) -> Self {
        self.modified_order = Some(order);
        self
    }
}

/// 심볼의 변동성 데이터.
#[derive(Debug, Clone)]
pub struct VolatilityData {
    /// 현재 변동성 백분율
    pub current_volatility: f64,
    /// 과거 평균 변동성
    pub average_volatility: f64,
    /// 마지막 업데이트 타임스탬프
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// 주문 검증 및 리스크 관리를 위한 리스크 매니저.
pub struct RiskManager {
    /// 리스크 설정
    config: RiskConfig,
    /// 포지션 크기 계산기
    position_sizer: PositionSizer,
    /// 일일 손실 추적기
    daily_tracker: DailyLossTracker,
    /// Stop 주문 생성기
    stop_generator: StopOrderGenerator,
    /// 계좌 잔고
    balance: Decimal,
    /// 심볼별 변동성 데이터
    volatility_data: HashMap<String, VolatilityData>,
    /// 활성 Trailing Stop (position_id -> state)
    trailing_stops: HashMap<String, TrailingStopState>,
}

impl RiskManager {
    /// 설정과 시작 잔고로 새 리스크 매니저 생성.
    pub fn new(config: RiskConfig, starting_balance: Decimal) -> Self {
        let position_sizer = PositionSizer::new(config.clone());
        let daily_tracker = DailyLossTracker::from_config(&config, starting_balance);
        let stop_generator = StopOrderGenerator::new(config.clone());

        Self {
            config,
            position_sizer,
            daily_tracker,
            stop_generator,
            balance: starting_balance,
            volatility_data: HashMap::new(),
            trailing_stops: HashMap::new(),
        }
    }

    /// 기본 설정으로 생성.
    pub fn with_balance(starting_balance: Decimal) -> Self {
        Self::new(RiskConfig::default(), starting_balance)
    }

    /// 계좌 잔고 업데이트.
    pub fn update_balance(&mut self, balance: Decimal) {
        self.balance = balance;
        self.daily_tracker.update_starting_balance(balance);
    }

    /// 현재 잔고 조회.
    pub fn balance(&self) -> Decimal {
        self.balance
    }

    /// 설정 참조 조회.
    pub fn config(&self) -> &RiskConfig {
        &self.config
    }

    // ==================== Order Validation ====================

    /// 모든 리스크 한도에 대해 주문 검증.
    ///
    /// # Arguments
    /// * `order` - 검증할 주문
    /// * `positions` - 현재 열린 포지션들
    /// * `current_price` - 해당 심볼의 현재 시장 가격
    ///
    /// # Returns
    /// 상세 결과가 포함된 RiskValidation
    pub fn validate_order(
        &mut self,
        order: &OrderRequest,
        positions: &[Position],
        current_price: Decimal,
    ) -> TraderResult<RiskValidation> {
        let symbol = order.ticker.clone();
        let mut warnings = Vec::new();

        // Check 1: Daily loss limit
        if !self.daily_tracker.can_trade() {
            return Ok(RiskValidation::invalid(
                "Trading paused: Daily loss limit reached",
            ));
        }

        // Check 2: Symbol enabled
        if !self.config.is_symbol_enabled(&symbol) {
            return Ok(RiskValidation::invalid(format!(
                "Trading disabled for symbol: {}",
                symbol
            )));
        }

        // Check 3: Volatility filter
        if let Some(volatility) = self.volatility_data.get(&symbol) {
            if volatility.current_volatility > self.config.volatility_threshold {
                return Ok(RiskValidation::invalid(format!(
                    "High volatility: {:.1}% exceeds threshold {:.1}%",
                    volatility.current_volatility, self.config.volatility_threshold
                )));
            }

            // Warning for elevated volatility
            if volatility.current_volatility > self.config.volatility_threshold * 0.7 {
                warnings.push(format!(
                    "Elevated volatility: {:.1}%",
                    volatility.current_volatility
                ));
            }
        }

        // Check 4: Position sizing limits
        let sizing_result =
            self.position_sizer
                .validate_order(order, positions, self.balance, current_price);

        if !sizing_result.is_valid {
            let mut validation = sizing_result.to_risk_validation();

            // Try to suggest adjusted size
            if let Some(suggested_qty) = self.position_sizer.suggest_adjusted_size(
                order,
                positions,
                self.balance,
                current_price,
            ) {
                let mut adjusted_order = order.clone();
                adjusted_order.quantity = suggested_qty;
                validation = validation.with_modified_order(adjusted_order);
                validation
                    .messages
                    .push(format!("Suggested adjusted quantity: {}", suggested_qty));
            }

            return Ok(validation);
        }

        // Check 5: Daily limit status warning
        let daily_status = self.daily_tracker.get_status();
        if let Some(warning) = daily_status.warning {
            warnings.push(warning);
        }

        // All checks passed
        let mut result = RiskValidation::valid();
        for warning in warnings {
            result = result.with_warning(warning);
        }

        Ok(result)
    }

    /// 거래 가능 여부 빠른 확인.
    pub fn can_trade(&mut self) -> bool {
        self.daily_tracker.can_trade()
    }

    // ==================== Daily Loss Tracking ====================

    /// 수익 또는 손실 기록.
    pub fn record_pnl(&mut self, symbol: &str, amount: Decimal) {
        if amount >= Decimal::ZERO {
            self.daily_tracker.record_profit(symbol, amount);
        } else {
            self.daily_tracker.record_loss(symbol, amount.abs());
        }
    }

    /// 일일 PnL 상태 조회.
    pub fn daily_status(&mut self) -> crate::limits::DailyLimitStatus {
        self.daily_tracker.get_status()
    }

    /// 현재 일일 PnL 조회.
    pub fn daily_pnl(&mut self) -> Decimal {
        self.daily_tracker.daily_pnl()
    }

    /// 일일 손실 한도 도달 여부 확인.
    pub fn is_daily_limit_reached(&mut self) -> bool {
        !self.daily_tracker.can_trade()
    }

    /// 일일 추적 강제 리셋 (관리자 기능).
    pub fn reset_daily_tracking(&mut self) {
        self.daily_tracker.force_reset();
    }

    // ==================== Stop Orders ====================

    /// 포지션에 대한 Stop-loss 주문 생성.
    pub fn generate_stop_loss(&self, position: &Position, custom_pct: Option<f64>) -> StopOrder {
        self.stop_generator.generate_stop_loss(position, custom_pct)
    }

    /// 포지션에 대한 Take-profit 주문 생성.
    pub fn generate_take_profit(&self, position: &Position, custom_pct: Option<f64>) -> StopOrder {
        self.stop_generator
            .generate_take_profit(position, custom_pct)
    }

    /// Stop-loss와 Take-profit 주문 모두 생성.
    pub fn generate_bracket_orders(
        &self,
        position: &Position,
        stop_loss_pct: Option<f64>,
        take_profit_pct: Option<f64>,
    ) -> (StopOrder, StopOrder) {
        self.stop_generator
            .generate_bracket_orders(position, stop_loss_pct, take_profit_pct)
    }

    /// ATR 기반 Stop-loss 생성.
    pub fn generate_atr_stop(
        &self,
        position: &Position,
        atr: Decimal,
        multiplier: Option<f64>,
    ) -> StopOrder {
        self.stop_generator
            .generate_atr_stop(position, atr, multiplier)
    }

    /// 포지션에 대한 Trailing Stop 초기화.
    pub fn init_trailing_stop(
        &mut self,
        position: &Position,
        trail_pct: f64,
        current_price: Decimal,
    ) -> StopOrder {
        let (order, state) =
            self.stop_generator
                .generate_trailing_stop(position, trail_pct, current_price);

        // Trailing Stop 상태 저장
        self.trailing_stops.insert(position.id.to_string(), state);

        order
    }

    /// 새 가격으로 Trailing Stop 업데이트.
    ///
    /// # Returns
    /// 업데이트된 경우 Some(new_trigger_price), 변경 없으면 None
    pub fn update_trailing_stop(
        &mut self,
        position_id: &str,
        current_price: Decimal,
    ) -> Option<Decimal> {
        if let Some(state) = self.trailing_stops.get_mut(position_id) {
            if state.update(current_price) {
                return Some(state.trigger_price);
            }
        }
        None
    }

    /// Trailing Stop 발동 여부 확인.
    pub fn should_trigger_trailing_stop(&self, position_id: &str, current_price: Decimal) -> bool {
        self.trailing_stops
            .get(position_id)
            .map(|state| state.should_trigger(current_price))
            .unwrap_or(false)
    }

    /// 포지션의 Trailing Stop 추적 제거.
    pub fn remove_trailing_stop(&mut self, position_id: &str) {
        self.trailing_stops.remove(position_id);
    }

    // ==================== Volatility ====================

    /// 심볼의 변동성 데이터 업데이트.
    pub fn update_volatility(&mut self, symbol: &str, current: f64, average: f64) {
        self.volatility_data.insert(
            symbol.to_string(),
            VolatilityData {
                current_volatility: current,
                average_volatility: average,
                last_updated: chrono::Utc::now(),
            },
        );
    }

    /// 거래에 적합한 변동성인지 확인.
    pub fn check_volatility(&self, symbol: &str) -> TraderResult<bool> {
        if let Some(data) = self.volatility_data.get(symbol) {
            Ok(data.current_volatility <= self.config.volatility_threshold)
        } else {
            // 데이터 없음 = 허용 가능으로 간주
            Ok(true)
        }
    }

    /// 심볼의 변동성 데이터 조회.
    pub fn get_volatility(&self, symbol: &str) -> Option<&VolatilityData> {
        self.volatility_data.get(symbol)
    }

    // ==================== Position Sizing ====================

    /// 심볼의 최대 포지션 크기 계산.
    pub fn calculate_max_size(&self, symbol: &str) -> Decimal {
        self.position_sizer.calculate_max_size(self.balance, symbol)
    }

    /// 현재 총 노출 계산.
    pub fn calculate_exposure(&self, positions: &[Position]) -> Decimal {
        self.position_sizer.calculate_current_exposure(positions)
    }

    /// 남은 노출 용량 계산.
    pub fn remaining_exposure(&self, positions: &[Position]) -> Decimal {
        let max = self.position_sizer.calculate_max_exposure(self.balance);
        let current = self.position_sizer.calculate_current_exposure(positions);
        (max - current).max(Decimal::ZERO)
    }

    /// 고정 비율 방식으로 최적 포지션 크기 계산.
    pub fn calculate_position_size(
        &self,
        risk_per_trade_pct: f64,
        entry_price: Decimal,
        stop_loss_price: Decimal,
    ) -> Decimal {
        self.position_sizer.calculate_fixed_fractional(
            self.balance,
            risk_per_trade_pct,
            entry_price,
            stop_loss_price,
        )
    }

    /// 리스크-보상 비율 계산.
    pub fn calculate_risk_reward(
        &self,
        entry_price: Decimal,
        stop_loss_price: Decimal,
        take_profit_price: Decimal,
        side: trader_core::Side,
    ) -> f64 {
        StopOrderGenerator::calculate_risk_reward(
            entry_price,
            stop_loss_price,
            take_profit_price,
            side,
        )
    }
}

impl Default for RiskManager {
    fn default() -> Self {
        Self::new(RiskConfig::default(), Decimal::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use trader_core::{Side, Symbol};

    fn create_test_position(
        symbol: &Symbol,
        side: Side,
        quantity: Decimal,
        entry_price: Decimal,
    ) -> Position {
        Position::new(
            "test_exchange",
            symbol.to_string(),
            side,
            quantity,
            entry_price,
        )
    }

    #[test]
    fn test_risk_manager_creation() {
        let config = RiskConfig::default();
        let mut manager = RiskManager::new(config, dec!(10000));

        assert_eq!(manager.balance(), dec!(10000));
        assert!(manager.can_trade());
    }

    #[test]
    fn test_validate_order_within_limits() {
        let config = RiskConfig::default();
        let mut manager = RiskManager::new(config, dec!(10000));

        let symbol = Symbol::crypto("BTC", "USDT");
        let order = OrderRequest::market_buy(symbol.to_string(), dec!(0.01));
        let positions: Vec<Position> = vec![];
        let price = dec!(50000); // Order value = $500

        let result = manager.validate_order(&order, &positions, price).unwrap();

        assert!(result.is_valid);
        assert!(result.messages.is_empty());
    }

    #[test]
    fn test_validate_order_exceeds_limit() {
        let config = RiskConfig::default();
        let mut manager = RiskManager::new(config, dec!(10000));

        let symbol = Symbol::crypto("BTC", "USDT");
        let order = OrderRequest::market_buy(symbol.to_string(), dec!(1.0));
        let positions: Vec<Position> = vec![];
        let price = dec!(50000); // Order value = $50000 (500% of balance)

        let result = manager.validate_order(&order, &positions, price).unwrap();

        assert!(!result.is_valid);
        assert!(result.messages[0].contains("exceeds maximum"));
    }

    #[test]
    fn test_daily_loss_tracking() {
        let config = RiskConfig::default(); // 3% daily loss limit
        let mut manager = RiskManager::new(config, dec!(10000));

        // Record losses
        manager.record_pnl("BTC/USDT", dec!(-100));
        manager.record_pnl("ETH/USDT", dec!(-100));

        assert!(manager.can_trade());
        assert_eq!(manager.daily_pnl(), dec!(-200));

        // Exceed limit
        manager.record_pnl("SOL/USDT", dec!(-200)); // Total = $400 > 3% of $10000 = $300

        assert!(!manager.can_trade());
        assert!(manager.is_daily_limit_reached());
    }

    #[test]
    fn test_generate_bracket_orders() {
        let config = RiskConfig::default();
        let manager = RiskManager::new(config, dec!(10000));

        let symbol = Symbol::crypto("BTC", "USDT");
        let position = create_test_position(&symbol, Side::Buy, dec!(0.1), dec!(50000));

        let (sl, tp) = manager.generate_bracket_orders(&position, None, None);

        assert!(sl.is_stop_loss());
        assert!(tp.is_take_profit());
        assert!(sl.trigger_price < dec!(50000)); // Stop below entry
        assert!(tp.trigger_price > dec!(50000)); // TP above entry
    }

    #[test]
    fn test_volatility_filter() {
        let mut config = RiskConfig::default();
        config.volatility_threshold = 5.0; // 5% threshold
        let mut manager = RiskManager::new(config, dec!(10000));

        // No volatility data = allowed
        assert!(manager.check_volatility("BTC/USDT").unwrap());

        // Low volatility = allowed
        manager.update_volatility("BTC/USDT", 3.0, 4.0);
        assert!(manager.check_volatility("BTC/USDT").unwrap());

        // High volatility = blocked
        manager.update_volatility("BTC/USDT", 6.0, 4.0);
        assert!(!manager.check_volatility("BTC/USDT").unwrap());
    }

    #[test]
    fn test_validate_order_high_volatility() {
        let mut config = RiskConfig::default();
        config.volatility_threshold = 5.0;
        let mut manager = RiskManager::new(config, dec!(10000));

        // Set high volatility
        manager.update_volatility("BTC/USDT", 8.0, 4.0);

        let symbol = Symbol::crypto("BTC", "USDT");
        let order = OrderRequest::market_buy(symbol.to_string(), dec!(0.01));

        let result = manager.validate_order(&order, &[], dec!(50000)).unwrap();

        assert!(!result.is_valid);
        assert!(result.messages[0].contains("volatility"));
    }

    #[test]
    fn test_trailing_stop_management() {
        let config = RiskConfig::default();
        let mut manager = RiskManager::new(config, dec!(10000));

        let symbol = Symbol::crypto("BTC", "USDT");
        let position = create_test_position(&symbol, Side::Buy, dec!(0.1), dec!(50000));
        let position_id = position.id.to_string();

        // Initialize trailing stop at current price $50000 with 2% trail
        let order = manager.init_trailing_stop(&position, 2.0, dec!(50000));

        assert_eq!(order.trigger_price, dec!(49000)); // 2% below $50000

        // Price goes up - trailing stop should update
        let new_trigger = manager.update_trailing_stop(&position_id, dec!(52000));
        assert!(new_trigger.is_some());
        assert!(new_trigger.unwrap() > dec!(49000));

        // Price goes down - should not update
        let new_trigger = manager.update_trailing_stop(&position_id, dec!(51000));
        assert!(new_trigger.is_none());
    }

    #[test]
    fn test_calculate_position_size() {
        let config = RiskConfig::default();
        let manager = RiskManager::new(config, dec!(10000));

        // Risk 1% ($100) with $1000 price risk per unit
        let qty = manager.calculate_position_size(
            1.0,         // 1% risk
            dec!(50000), // Entry
            dec!(49000), // Stop (1000 risk per unit)
        );

        assert_eq!(qty, dec!(0.1)); // $100 / $1000 = 0.1
    }

    #[test]
    fn test_remaining_exposure() {
        let config = RiskConfig::default(); // 50% max exposure
        let manager = RiskManager::new(config, dec!(10000));

        let symbol = Symbol::crypto("BTC", "USDT");
        let position = create_test_position(&symbol, Side::Buy, dec!(0.05), dec!(50000));
        // Position value = $2500 (25% of balance)

        let remaining = manager.remaining_exposure(&[position]);
        assert_eq!(remaining, dec!(2500)); // 50% - 25% = 25% = $2500
    }

    #[test]
    fn test_validate_order_with_suggested_adjustment() {
        let config = RiskConfig::default();
        let mut manager = RiskManager::new(config, dec!(10000));

        let symbol = Symbol::crypto("BTC", "USDT");
        // Try to order $2000 worth (20%) when limit is 10%
        let order = OrderRequest::market_buy(symbol.to_string(), dec!(0.04));
        let price = dec!(50000); // Order value = $2000

        let result = manager.validate_order(&order, &[], price).unwrap();

        assert!(!result.is_valid);
        assert!(result.modified_order.is_some());

        let suggested = result.modified_order.unwrap();
        // Should suggest 0.02 (max $1000 at $50000)
        assert_eq!(suggested.quantity, dec!(0.02));
    }

    #[test]
    fn test_daily_reset() {
        let config = RiskConfig::default();
        let mut manager = RiskManager::new(config, dec!(10000));

        // Exceed daily limit
        manager.record_pnl("BTC/USDT", dec!(-500));
        assert!(!manager.can_trade());

        // Reset
        manager.reset_daily_tracking();

        assert!(manager.can_trade());
        assert_eq!(manager.daily_pnl(), dec!(0));
    }
}
