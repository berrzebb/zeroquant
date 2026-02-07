//! 리스크 검증 및 제한.
//!
//! 이 모듈은 포지션 및 거래에 대한 리스크 검증을 수행합니다.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 리스크 검증 에러.
#[derive(Debug, Error)]
pub enum RiskCheckError {
    #[error("포지션 크기가 최대 한도를 초과했습니다: {current} > {max}")]
    ExceededMaxPosition { current: Decimal, max: Decimal },

    #[error("일일 손실 한도를 초과했습니다: {current} > {max}")]
    ExceededDailyLoss { current: Decimal, max: Decimal },

    #[error("총 리스크가 한도를 초과했습니다: {current} > {max}")]
    ExceededTotalRisk { current: Decimal, max: Decimal },

    #[error("레버리지가 허용 범위를 초과했습니다: {current} > {max}")]
    ExceededLeverage { current: Decimal, max: Decimal },
}

/// 리스크 파라미터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskParams {
    /// 최대 포지션 크기 (자본 대비 비율)
    pub max_position_ratio: Decimal,

    /// 일일 최대 손실 한도 (자본 대비 비율)
    pub max_daily_loss_ratio: Decimal,

    /// 손절가 비율 (진입가 대비)
    pub stop_loss_ratio: Decimal,

    /// 익절가 비율 (진입가 대비)
    pub take_profit_ratio: Decimal,

    /// 최대 레버리지
    pub max_leverage: Decimal,
}

impl Default for RiskParams {
    fn default() -> Self {
        Self {
            max_position_ratio: dec!(0.1),    // 10%
            max_daily_loss_ratio: dec!(0.03), // 3%
            stop_loss_ratio: dec!(0.03),      // 3%
            take_profit_ratio: dec!(0.05),    // 5%
            max_leverage: dec!(1),            // 1x (레버리지 없음)
        }
    }
}

/// 리스크 체커 trait.
pub trait RiskChecker: Send + Sync + std::fmt::Debug {
    /// 포지션 크기를 검증합니다.
    fn check_position_size(
        &self,
        position_value: Decimal,
        total_capital: Decimal,
        params: &RiskParams,
    ) -> Result<(), RiskCheckError>;

    /// 일일 손실을 검증합니다.
    fn check_daily_loss(
        &self,
        current_loss: Decimal,
        total_capital: Decimal,
        params: &RiskParams,
    ) -> Result<(), RiskCheckError>;

    /// 레버리지를 검증합니다.
    fn check_leverage(&self, leverage: Decimal, params: &RiskParams) -> Result<(), RiskCheckError>;
}

/// 기본 리스크 체커 구현.
#[derive(Debug, Default)]
pub struct DefaultRiskChecker;

impl RiskChecker for DefaultRiskChecker {
    fn check_position_size(
        &self,
        position_value: Decimal,
        total_capital: Decimal,
        params: &RiskParams,
    ) -> Result<(), RiskCheckError> {
        let max_allowed = total_capital * params.max_position_ratio;

        if position_value > max_allowed {
            return Err(RiskCheckError::ExceededMaxPosition {
                current: position_value,
                max: max_allowed,
            });
        }

        Ok(())
    }

    fn check_daily_loss(
        &self,
        current_loss: Decimal,
        total_capital: Decimal,
        params: &RiskParams,
    ) -> Result<(), RiskCheckError> {
        let max_allowed = total_capital * params.max_daily_loss_ratio;

        if current_loss > max_allowed {
            return Err(RiskCheckError::ExceededDailyLoss {
                current: current_loss,
                max: max_allowed,
            });
        }

        Ok(())
    }

    fn check_leverage(&self, leverage: Decimal, params: &RiskParams) -> Result<(), RiskCheckError> {
        if leverage > params.max_leverage {
            return Err(RiskCheckError::ExceededLeverage {
                current: leverage,
                max: params.max_leverage,
            });
        }

        Ok(())
    }
}

/// 리스크 관리자.
///
/// 여러 리스크 검증을 종합적으로 수행합니다.
#[derive(Debug)]
pub struct RiskManager {
    checker: Box<dyn RiskChecker>,
    params: RiskParams,
}

impl RiskManager {
    pub fn new(checker: Box<dyn RiskChecker>, params: RiskParams) -> Self {
        Self { checker, params }
    }

    pub fn with_default_params() -> Self {
        Self {
            checker: Box::new(DefaultRiskChecker),
            params: RiskParams::default(),
        }
    }

    /// 포지션 진입 전 종합 검증.
    pub fn validate_entry(
        &self,
        position_value: Decimal,
        total_capital: Decimal,
        current_daily_loss: Decimal,
        leverage: Decimal,
    ) -> Result<(), RiskCheckError> {
        // 포지션 크기 검증
        self.checker
            .check_position_size(position_value, total_capital, &self.params)?;

        // 일일 손실 검증
        self.checker
            .check_daily_loss(current_daily_loss, total_capital, &self.params)?;

        // 레버리지 검증
        self.checker.check_leverage(leverage, &self.params)?;

        Ok(())
    }

    /// 손절가 계산.
    pub fn calculate_stop_loss(&self, entry_price: Decimal, is_long: bool) -> Decimal {
        if is_long {
            entry_price * (dec!(1) - self.params.stop_loss_ratio)
        } else {
            entry_price * (dec!(1) + self.params.stop_loss_ratio)
        }
    }

    /// 익절가 계산.
    pub fn calculate_take_profit(&self, entry_price: Decimal, is_long: bool) -> Decimal {
        if is_long {
            entry_price * (dec!(1) + self.params.take_profit_ratio)
        } else {
            entry_price * (dec!(1) - self.params.take_profit_ratio)
        }
    }

    /// 리스크/보상 비율 계산.
    pub fn risk_reward_ratio(&self) -> Decimal {
        self.params.take_profit_ratio / self.params.stop_loss_ratio
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_position_size_ok() {
        let checker = DefaultRiskChecker;
        let params = RiskParams::default();

        let result = checker.check_position_size(dec!(1000), dec!(10000), &params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_position_size_exceeded() {
        let checker = DefaultRiskChecker;
        let params = RiskParams::default();

        let result = checker.check_position_size(dec!(2000), dec!(10000), &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_daily_loss_ok() {
        let checker = DefaultRiskChecker;
        let params = RiskParams::default();

        let result = checker.check_daily_loss(dec!(100), dec!(10000), &params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_daily_loss_exceeded() {
        let checker = DefaultRiskChecker;
        let params = RiskParams::default();

        let result = checker.check_daily_loss(dec!(400), dec!(10000), &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_risk_manager_validate_entry() {
        let manager = RiskManager::with_default_params();

        let result = manager.validate_entry(
            dec!(1000),  // 포지션 크기
            dec!(10000), // 총 자본
            dec!(100),   // 현재 일일 손실
            dec!(1),     // 레버리지
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_calculate_stop_loss() {
        let manager = RiskManager::with_default_params();

        let stop_long = manager.calculate_stop_loss(dec!(100), true);
        assert_eq!(stop_long, dec!(97)); // 100 * (1 - 0.03)

        let stop_short = manager.calculate_stop_loss(dec!(100), false);
        assert_eq!(stop_short, dec!(103)); // 100 * (1 + 0.03)
    }

    #[test]
    fn test_calculate_take_profit() {
        let manager = RiskManager::with_default_params();

        let tp_long = manager.calculate_take_profit(dec!(100), true);
        assert_eq!(tp_long, dec!(105)); // 100 * (1 + 0.05)

        let tp_short = manager.calculate_take_profit(dec!(100), false);
        assert_eq!(tp_short, dec!(95)); // 100 * (1 - 0.05)
    }

    #[test]
    fn test_risk_reward_ratio() {
        let manager = RiskManager::with_default_params();
        let ratio = manager.risk_reward_ratio();

        // 0.05 / 0.03 = 1.666...
        assert!(ratio > dec!(1.6) && ratio < dec!(1.7));
    }
}
