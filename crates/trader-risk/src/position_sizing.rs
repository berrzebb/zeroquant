//! 포지션 크기 계산 및 검증.
//!
//! 제공 기능:
//! - 계좌 잔고 기반 최대 허용 포지션 크기 계산
//! - 리스크 한도 대비 주문 크기 검증
//! - 다양한 방법(고정 비율, Kelly)을 사용한 최적 포지션 크기 계산

use crate::config::RiskConfig;
use crate::manager::RiskValidation;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use trader_core::{OrderRequest, Position};

/// 정밀도를 위해 정수 연산을 사용하여 퍼센트를 금액으로 변환.
/// 예시: pct_to_amount(1000, 10.0) = 100 (1000의 10%)
fn pct_to_amount(amount: Decimal, pct: f64) -> Decimal {
    // 퍼센트를 정수로 스케일링 (퍼센트의 소수점 4자리까지 지원)
    // 예: 10.5% -> 105000, 그 후 1_000_000으로 나눔
    let scaled_pct = (pct * 10000.0).round() as i64;
    (amount * Decimal::from(scaled_pct)) / Decimal::from(1_000_000)
}

/// 포지션 크기 계산 및 검증을 위한 포지션 사이저.
#[derive(Debug, Clone)]
pub struct PositionSizer {
    config: RiskConfig,
}

/// 상세 정보가 포함된 포지션 크기 검증 결과.
#[derive(Debug, Clone)]
pub struct SizingValidation {
    /// 크기가 유효한지 여부
    pub is_valid: bool,
    /// 계산된 최대 허용 크기
    pub max_allowed_size: Decimal,
    /// 요청된 크기
    pub requested_size: Decimal,
    /// 현재 총 노출
    pub current_exposure: Decimal,
    /// 검증 메시지
    pub messages: Vec<String>,
}

impl SizingValidation {
    /// 유효한 결과를 생성.
    pub fn valid(max_allowed: Decimal, requested: Decimal, current_exposure: Decimal) -> Self {
        Self {
            is_valid: true,
            max_allowed_size: max_allowed,
            requested_size: requested,
            current_exposure,
            messages: vec![],
        }
    }

    /// 유효하지 않은 결과를 생성.
    pub fn invalid(
        reason: impl Into<String>,
        max_allowed: Decimal,
        requested: Decimal,
        current_exposure: Decimal,
    ) -> Self {
        Self {
            is_valid: false,
            max_allowed_size: max_allowed,
            requested_size: requested,
            current_exposure,
            messages: vec![reason.into()],
        }
    }

    /// RiskValidation으로 변환.
    pub fn to_risk_validation(&self) -> RiskValidation {
        if self.is_valid {
            RiskValidation::valid()
        } else {
            let mut validation = RiskValidation::invalid(
                self.messages
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Position size validation failed".to_string()),
            );
            for msg in self.messages.iter().skip(1) {
                validation = validation.with_warning(msg.clone());
            }
            validation
        }
    }
}

impl PositionSizer {
    /// 주어진 설정으로 새 포지션 사이저를 생성.
    pub fn new(config: RiskConfig) -> Self {
        Self { config }
    }

    /// 단일 거래에 대한 최대 허용 포지션 크기를 계산.
    ///
    /// # 인자
    /// * `balance` - 기준 통화로 된 총 계좌 잔고
    /// * `symbol` - 거래 심볼 (심볼별 한도용)
    ///
    /// # 반환값
    /// 기준 통화로 된 최대 포지션 크기
    pub fn calculate_max_size(&self, balance: Decimal, symbol: &str) -> Decimal {
        let max_pct = self.config.get_max_position_pct(symbol);
        pct_to_amount(balance, max_pct)
    }

    /// 허용되는 최대 총 노출을 계산.
    ///
    /// # 인자
    /// * `balance` - 기준 통화로 된 총 계좌 잔고
    ///
    /// # 반환값
    /// 기준 통화로 된 최대 총 노출
    pub fn calculate_max_exposure(&self, balance: Decimal) -> Decimal {
        pct_to_amount(balance, self.config.max_total_exposure_pct)
    }

    /// 기존 포지션에서 현재 총 노출을 계산.
    ///
    /// # 인자
    /// * `positions` - 현재 열린 포지션 목록
    ///
    /// # 반환값
    /// 기준 통화로 된 총 노출 (모든 포지션 명목 가치의 합)
    pub fn calculate_current_exposure(&self, positions: &[Position]) -> Decimal {
        positions
            .iter()
            .filter(|p| p.is_open())
            .map(|p| p.notional_value())
            .sum()
    }

    /// 특정 심볼에 대한 노출을 계산.
    pub fn calculate_symbol_exposure(&self, positions: &[Position], symbol: &str) -> Decimal {
        positions
            .iter()
            .filter(|p| p.is_open() && p.ticker == symbol)
            .map(|p| p.notional_value())
            .sum()
    }

    /// 포지션 크기 한도에 대해 주문을 검증.
    ///
    /// # 인자
    /// * `order` - 검증할 주문
    /// * `positions` - 현재 열린 포지션
    /// * `balance` - 총 계좌 잔고
    /// * `current_price` - 주문 심볼의 현재 시장 가격
    ///
    /// # 반환값
    /// 상세 정보가 포함된 검증 결과
    pub fn validate_order(
        &self,
        order: &OrderRequest,
        positions: &[Position],
        balance: Decimal,
        current_price: Decimal,
    ) -> SizingValidation {
        let symbol = order.ticker.clone();

        // 심볼이 활성화되어 있는지 확인
        if !self.config.is_symbol_enabled(&symbol) {
            return SizingValidation::invalid(
                format!("Trading disabled for symbol: {}", symbol),
                Decimal::ZERO,
                order.quantity * current_price,
                self.calculate_current_exposure(positions),
            );
        }

        // 주문 가치 계산
        let order_value = order.quantity * current_price;
        let max_single_size = self.calculate_max_size(balance, &symbol);
        let current_exposure = self.calculate_current_exposure(positions);

        // 검사 1: 단일 주문 크기 한도
        if order_value > max_single_size {
            return SizingValidation::invalid(
                format!(
                    "Order size {} exceeds maximum allowed {} ({:.1}% of balance)",
                    order_value,
                    max_single_size,
                    self.config.get_max_position_pct(&symbol)
                ),
                max_single_size,
                order_value,
                current_exposure,
            );
        }

        // 검사 2: 최소 주문 크기
        if order_value < self.config.min_order_size {
            return SizingValidation::invalid(
                format!(
                    "Order size {} is below minimum {}",
                    order_value, self.config.min_order_size
                ),
                max_single_size,
                order_value,
                current_exposure,
            );
        }

        // 검사 3: 총 노출 한도
        let max_exposure = self.calculate_max_exposure(balance);
        let new_total_exposure = current_exposure + order_value;

        if new_total_exposure > max_exposure {
            return SizingValidation::invalid(
                format!(
                    "Total exposure {} would exceed maximum {} ({:.1}% of balance)",
                    new_total_exposure, max_exposure, self.config.max_total_exposure_pct
                ),
                max_exposure - current_exposure,
                order_value,
                current_exposure,
            );
        }

        // 검사 4: 최대 동시 포지션 (새 포지션에만 해당)
        let is_new_position = !positions
            .iter()
            .any(|p| p.is_open() && p.ticker == symbol && p.side == order.side);

        if is_new_position {
            let open_position_count = positions.iter().filter(|p| p.is_open()).count();
            if open_position_count >= self.config.max_concurrent_positions {
                return SizingValidation::invalid(
                    format!(
                        "Maximum concurrent positions ({}) reached",
                        self.config.max_concurrent_positions
                    ),
                    max_single_size,
                    order_value,
                    current_exposure,
                );
            }
        }

        SizingValidation::valid(max_single_size, order_value, current_exposure)
    }

    /// 고정 비율 방법을 사용하여 최적 포지션 크기를 계산.
    ///
    /// # 인자
    /// * `balance` - 총 계좌 잔고
    /// * `risk_per_trade_pct` - 거래당 위험에 노출할 잔고 비율
    /// * `entry_price` - 예상 진입 가격
    /// * `stop_loss_price` - 손절 가격
    ///
    /// # 반환값
    /// 권장 포지션 수량
    pub fn calculate_fixed_fractional(
        &self,
        balance: Decimal,
        risk_per_trade_pct: f64,
        entry_price: Decimal,
        stop_loss_price: Decimal,
    ) -> Decimal {
        if entry_price == stop_loss_price {
            return Decimal::ZERO;
        }

        let risk_amount = pct_to_amount(balance, risk_per_trade_pct);
        let price_risk = (entry_price - stop_loss_price).abs();

        if price_risk.is_zero() {
            return Decimal::ZERO;
        }

        risk_amount / price_risk
    }

    /// Kelly 기준을 사용하여 포지션 크기를 계산.
    ///
    /// # 인자
    /// * `balance` - 총 계좌 잔고
    /// * `win_rate` - 과거 승률 (0.0에서 1.0)
    /// * `avg_win` - 평균 수익 거래 금액
    /// * `avg_loss` - 평균 손실 거래 금액
    /// * `current_price` - 변환용 현재 가격
    ///
    /// # 반환값
    /// 기준 통화로 된 권장 포지션 크기 (최대 허용량으로 제한)
    pub fn calculate_kelly(
        &self,
        balance: Decimal,
        win_rate: f64,
        avg_win: Decimal,
        avg_loss: Decimal,
        symbol: &str,
    ) -> Decimal {
        if avg_loss.is_zero() || win_rate <= 0.0 || win_rate >= 1.0 {
            return Decimal::ZERO;
        }

        let loss_rate = 1.0 - win_rate;
        let win_loss_ratio = avg_win / avg_loss;

        // Kelly 공식: f = W - (1-W)/R, 여기서 W = 승률, R = 승/패 비율
        let kelly_pct = win_rate - (loss_rate / win_loss_ratio.to_f64().unwrap_or(1.0));

        if kelly_pct <= 0.0 {
            return Decimal::ZERO;
        }

        // 안전을 위해 half-Kelly 사용 (100 대신 50을 곱함)
        let kelly_size = pct_to_amount(balance, kelly_pct * 50.0);

        // 최대 허용 크기로 제한
        let max_size = self.calculate_max_size(balance, symbol);
        kelly_size.min(max_size)
    }

    /// 한도 내에 맞는 조정된 주문 크기를 제안.
    ///
    /// # 인자
    /// * `order` - 원래 주문
    /// * `positions` - 현재 포지션
    /// * `balance` - 계좌 잔고
    /// * `current_price` - 현재 시장 가격
    ///
    /// # 반환값
    /// 한도 내에 맞는 제안 수량, 불가능한 경우 None
    pub fn suggest_adjusted_size(
        &self,
        order: &OrderRequest,
        positions: &[Position],
        balance: Decimal,
        current_price: Decimal,
    ) -> Option<Decimal> {
        let symbol = order.ticker.clone();

        if !self.config.is_symbol_enabled(&symbol) {
            return None;
        }

        let current_exposure = self.calculate_current_exposure(positions);
        let max_exposure = self.calculate_max_exposure(balance);
        let max_single = self.calculate_max_size(balance, &symbol);

        // 새 포지션을 위한 가용 여유
        let available_exposure = max_exposure - current_exposure;
        if available_exposure <= Decimal::ZERO {
            return None;
        }

        // 단일 포지션 한도와 가용 노출 중 작은 값 선택
        let max_order_value = available_exposure.min(max_single);

        if max_order_value < self.config.min_order_size {
            return None;
        }

        // 다시 수량으로 변환
        let suggested_qty = max_order_value / current_price;
        Some(suggested_qty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use trader_core::{Side, Symbol};

    fn create_test_position(symbol: &Symbol, quantity: Decimal, price: Decimal) -> Position {
        Position::new(
            "test_exchange",
            symbol.to_string(),
            Side::Buy,
            quantity,
            price,
        )
    }

    #[test]
    fn test_calculate_max_size() {
        let config = RiskConfig::default(); // max_position_pct = 10%
        let sizer = PositionSizer::new(config);

        let balance = dec!(10000);
        let max_size = sizer.calculate_max_size(balance, "BTC/USDT");

        assert_eq!(max_size, dec!(1000)); // 10% of 10000
    }

    #[test]
    fn test_calculate_max_exposure() {
        let config = RiskConfig::default(); // max_total_exposure_pct = 50%
        let sizer = PositionSizer::new(config);

        let balance = dec!(10000);
        let max_exposure = sizer.calculate_max_exposure(balance);

        assert_eq!(max_exposure, dec!(5000)); // 50% of 10000
    }

    #[test]
    fn test_validate_order_within_limits() {
        let config = RiskConfig::default();
        let sizer = PositionSizer::new(config);

        let symbol = Symbol::crypto("BTC", "USDT");
        let order = OrderRequest::market_buy(symbol.to_string(), dec!(0.01));
        let positions: Vec<Position> = vec![];
        let balance = dec!(10000);
        let price = dec!(50000); // Order value = 0.01 * 50000 = 500

        let validation = sizer.validate_order(&order, &positions, balance, price);

        assert!(validation.is_valid);
        assert_eq!(validation.requested_size, dec!(500));
    }

    #[test]
    fn test_validate_order_exceeds_single_limit() {
        let config = RiskConfig::default(); // max_position_pct = 10%
        let sizer = PositionSizer::new(config);

        let symbol = Symbol::crypto("BTC", "USDT");
        let order = OrderRequest::market_buy(symbol.to_string(), dec!(1.0));
        let positions: Vec<Position> = vec![];
        let balance = dec!(10000);
        let price = dec!(50000); // Order value = 1.0 * 50000 = 50000 (500% of balance)

        let validation = sizer.validate_order(&order, &positions, balance, price);

        assert!(!validation.is_valid);
        assert!(validation.messages[0].contains("exceeds maximum"));
    }

    #[test]
    fn test_validate_order_exceeds_total_exposure() {
        let config = RiskConfig::default(); // max_total_exposure_pct = 50%
        let sizer = PositionSizer::new(config);

        let symbol = Symbol::crypto("BTC", "USDT");
        let eth_symbol = Symbol::crypto("ETH", "USDT");

        // Existing position worth 4500 (45% of balance)
        // So any new position > 500 (5%) will exceed 50% total exposure
        let existing = create_test_position(&eth_symbol, dec!(2.25), dec!(2000));

        let positions = vec![existing];

        // New order worth 800 (8% of balance) - under single limit (10%)
        // But total would be 45% + 8% = 53% > 50%
        let order = OrderRequest::market_buy(symbol.to_string(), dec!(0.016));
        let balance = dec!(10000);
        let price = dec!(50000); // Order value = 0.016 * 50000 = 800

        let validation = sizer.validate_order(&order, &positions, balance, price);

        assert!(!validation.is_valid);
        assert!(
            validation.messages[0].contains("Total exposure"),
            "Expected 'Total exposure' error, got: {}",
            validation.messages[0]
        );
    }

    #[test]
    fn test_validate_order_below_minimum() {
        let config = RiskConfig::default(); // min_order_size = 10
        let sizer = PositionSizer::new(config);

        let symbol = Symbol::crypto("BTC", "USDT");
        let order = OrderRequest::market_buy(symbol.to_string(), dec!(0.0001));
        let positions: Vec<Position> = vec![];
        let balance = dec!(10000);
        let price = dec!(50000); // Order value = 0.0001 * 50000 = 5

        let validation = sizer.validate_order(&order, &positions, balance, price);

        assert!(!validation.is_valid);
        assert!(validation.messages[0].contains("below minimum"));
    }

    #[test]
    fn test_validate_max_concurrent_positions() {
        let mut config = RiskConfig::default();
        config.max_concurrent_positions = 2;
        let sizer = PositionSizer::new(config);

        let btc_symbol = Symbol::crypto("BTC", "USDT");
        let eth_symbol = Symbol::crypto("ETH", "USDT");
        let sol_symbol = Symbol::crypto("SOL", "USDT");

        // 2 existing positions
        let positions = vec![
            create_test_position(&btc_symbol, dec!(0.01), dec!(50000)),
            create_test_position(&eth_symbol, dec!(0.1), dec!(2000)),
        ];

        // Try to add a third
        let order = OrderRequest::market_buy(sol_symbol.to_string(), dec!(1.0));
        let balance = dec!(100000);
        let price = dec!(100);

        let validation = sizer.validate_order(&order, &positions, balance, price);

        assert!(!validation.is_valid);
        assert!(validation.messages[0].contains("Maximum concurrent positions"));
    }

    #[test]
    fn test_fixed_fractional_sizing() {
        let config = RiskConfig::default();
        let sizer = PositionSizer::new(config);

        let balance = dec!(10000);
        let risk_pct = 1.0; // 1% risk per trade = $100
        let entry_price = dec!(50000);
        let stop_loss = dec!(49000); // $1000 risk per BTC

        let qty = sizer.calculate_fixed_fractional(balance, risk_pct, entry_price, stop_loss);

        // Expected: $100 / $1000 = 0.1 BTC
        assert_eq!(qty, dec!(0.1));
    }

    #[test]
    fn test_suggest_adjusted_size() {
        let config = RiskConfig::default();
        let sizer = PositionSizer::new(config);

        let symbol = Symbol::crypto("BTC", "USDT");
        let order = OrderRequest::market_buy(symbol.to_string(), dec!(1.0)); // Too large
        let positions: Vec<Position> = vec![];
        let balance = dec!(10000);
        let price = dec!(50000);

        let suggested = sizer.suggest_adjusted_size(&order, &positions, balance, price);

        assert!(suggested.is_some());
        let qty = suggested.unwrap();
        // Max single position = 1000, so qty = 1000/50000 = 0.02
        assert_eq!(qty, dec!(0.02));
    }

    #[test]
    fn test_symbol_specific_limits() {
        let mut config = RiskConfig::default();
        config.set_symbol_config(
            "BTC/USDT",
            crate::config::SymbolRiskConfig {
                max_position_pct: Some(20.0), // Higher limit for BTC
                ..Default::default()
            },
        );
        let sizer = PositionSizer::new(config);

        let balance = dec!(10000);

        // BTC has 20% limit
        let btc_max = sizer.calculate_max_size(balance, "BTC/USDT");
        assert_eq!(btc_max, dec!(2000));

        // ETH uses default 10%
        let eth_max = sizer.calculate_max_size(balance, "ETH/USDT");
        assert_eq!(eth_max, dec!(1000));
    }

    #[test]
    fn test_disabled_symbol() {
        let mut config = RiskConfig::default();
        config.set_symbol_config(
            "RISKY/USDT",
            crate::config::SymbolRiskConfig {
                enabled: false,
                ..Default::default()
            },
        );
        let sizer = PositionSizer::new(config);

        let symbol = Symbol::new("RISKY", "USDT", trader_core::MarketType::Crypto);
        let order = OrderRequest::market_buy(symbol.to_string(), dec!(1.0));
        let positions: Vec<Position> = vec![];
        let balance = dec!(10000);
        let price = dec!(100);

        let validation = sizer.validate_order(&order, &positions, balance, price);

        assert!(!validation.is_valid);
        assert!(validation.messages[0].contains("Trading disabled"));
    }
}
