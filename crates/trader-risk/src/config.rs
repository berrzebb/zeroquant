//! 리스크 관리 설정.
//!
//! 리스크 한도, 포지션 사이징, 보호 주문(손절/익절)을 위한
//! 설정 구조체를 정의합니다.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 전역 리스크 관리 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    /// 계좌 잔고 대비 최대 포지션 크기 비율 (기본값: 10%)
    #[serde(default = "default_max_position_pct")]
    pub max_position_pct: f64,

    /// 계좌 잔고 대비 최대 일일 손실 비율 (기본값: 3%)
    /// 이 한도에 도달하면 거래가 중지됩니다
    #[serde(default = "default_max_daily_loss_pct")]
    pub max_daily_loss_pct: f64,

    /// 계좌 잔고 대비 최대 총 노출 비율 (기본값: 50%)
    /// 모든 열린 포지션의 합이 이를 초과하지 않아야 합니다
    #[serde(default = "default_max_total_exposure_pct")]
    pub max_total_exposure_pct: f64,

    /// 거래가 일시 중지되는 변동성 임계값 (ATR 비율) (기본값: 5%)
    #[serde(default = "default_volatility_threshold")]
    pub volatility_threshold: f64,

    /// 진입가 대비 기본 손절 비율 (기본값: 2%)
    #[serde(default = "default_stop_loss_pct")]
    pub default_stop_loss_pct: f64,

    /// 진입가 대비 기본 익절 비율 (기본값: 5%)
    #[serde(default = "default_take_profit_pct")]
    pub default_take_profit_pct: f64,

    /// 호가 통화 기준 최소 주문 크기 (기본값: 10.0)
    #[serde(default = "default_min_order_size")]
    pub min_order_size: Decimal,

    /// 최대 동시 포지션 수 (기본값: 10)
    #[serde(default = "default_max_concurrent_positions")]
    pub max_concurrent_positions: usize,

    /// 트레일링 손절 활성화 여부 (기본값: false)
    #[serde(default)]
    pub enable_trailing_stop: bool,

    /// 트레일링 손절 거리 비율 (기본값: 1.5%)
    #[serde(default = "default_trailing_stop_pct")]
    pub trailing_stop_pct: f64,

    /// 심볼별 리스크 설정 (전역 설정을 재정의함)
    #[serde(default)]
    pub symbol_configs: HashMap<String, SymbolRiskConfig>,
}

/// 심볼별 리스크 설정.
/// 여기의 값들은 특정 심볼에 대해 전역 RiskConfig를 재정의합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRiskConfig {
    /// 이 심볼의 최대 포지션 크기 (전역 설정 재정의)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_position_pct: Option<f64>,

    /// 이 심볼의 손절 비율 (전역 설정 재정의)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss_pct: Option<f64>,

    /// 이 심볼의 익절 비율 (전역 설정 재정의)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit_pct: Option<f64>,

    /// 이 심볼의 변동성 임계값 (전역 설정 재정의)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volatility_threshold: Option<f64>,

    /// 이 심볼의 거래 활성화 여부 (기본값: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
}

// 기본값 함수들
fn default_max_position_pct() -> f64 {
    10.0
}

fn default_max_daily_loss_pct() -> f64 {
    3.0
}

fn default_max_total_exposure_pct() -> f64 {
    50.0
}

fn default_volatility_threshold() -> f64 {
    5.0
}

fn default_stop_loss_pct() -> f64 {
    2.0
}

fn default_take_profit_pct() -> f64 {
    5.0
}

fn default_min_order_size() -> Decimal {
    Decimal::from(10)
}

fn default_max_concurrent_positions() -> usize {
    10
}

fn default_trailing_stop_pct() -> f64 {
    1.5
}

fn default_true() -> bool {
    true
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_pct: default_max_position_pct(),
            max_daily_loss_pct: default_max_daily_loss_pct(),
            max_total_exposure_pct: default_max_total_exposure_pct(),
            volatility_threshold: default_volatility_threshold(),
            default_stop_loss_pct: default_stop_loss_pct(),
            default_take_profit_pct: default_take_profit_pct(),
            min_order_size: default_min_order_size(),
            max_concurrent_positions: default_max_concurrent_positions(),
            enable_trailing_stop: false,
            trailing_stop_pct: default_trailing_stop_pct(),
            symbol_configs: HashMap::new(),
        }
    }
}

impl Default for SymbolRiskConfig {
    fn default() -> Self {
        Self {
            max_position_pct: None,
            stop_loss_pct: None,
            take_profit_pct: None,
            volatility_threshold: None,
            enabled: true,
        }
    }
}

impl RiskConfig {
    /// 기본값으로 새 RiskConfig를 생성합니다.
    pub fn new() -> Self {
        Self::default()
    }

    /// 보수적인 리스크 설정을 생성합니다 (낮은 한도).
    pub fn conservative() -> Self {
        Self {
            max_position_pct: 5.0,
            max_daily_loss_pct: 1.5,
            max_total_exposure_pct: 30.0,
            volatility_threshold: 3.0,
            default_stop_loss_pct: 1.5,
            default_take_profit_pct: 3.0,
            min_order_size: Decimal::from(10),
            max_concurrent_positions: 5,
            enable_trailing_stop: true,
            trailing_stop_pct: 1.0,
            symbol_configs: HashMap::new(),
        }
    }

    /// 공격적인 리스크 설정을 생성합니다 (높은 한도).
    pub fn aggressive() -> Self {
        Self {
            max_position_pct: 20.0,
            max_daily_loss_pct: 5.0,
            max_total_exposure_pct: 80.0,
            volatility_threshold: 8.0,
            default_stop_loss_pct: 3.0,
            default_take_profit_pct: 8.0,
            min_order_size: Decimal::from(10),
            max_concurrent_positions: 20,
            enable_trailing_stop: false,
            trailing_stop_pct: 2.0,
            symbol_configs: HashMap::new(),
        }
    }

    /// 심볼에 대한 유효 손절 비율을 가져옵니다.
    /// 심볼별 값이 설정되어 있으면 해당 값을 반환하고, 그렇지 않으면 전역 기본값을 반환합니다.
    pub fn get_stop_loss_pct(&self, symbol: &str) -> f64 {
        self.symbol_configs
            .get(symbol)
            .and_then(|c| c.stop_loss_pct)
            .unwrap_or(self.default_stop_loss_pct)
    }

    /// 심볼에 대한 유효 익절 비율을 가져옵니다.
    pub fn get_take_profit_pct(&self, symbol: &str) -> f64 {
        self.symbol_configs
            .get(symbol)
            .and_then(|c| c.take_profit_pct)
            .unwrap_or(self.default_take_profit_pct)
    }

    /// 심볼에 대한 유효 최대 포지션 비율을 가져옵니다.
    pub fn get_max_position_pct(&self, symbol: &str) -> f64 {
        self.symbol_configs
            .get(symbol)
            .and_then(|c| c.max_position_pct)
            .unwrap_or(self.max_position_pct)
    }

    /// 심볼에 대한 유효 변동성 임계값을 가져옵니다.
    pub fn get_volatility_threshold(&self, symbol: &str) -> f64 {
        self.symbol_configs
            .get(symbol)
            .and_then(|c| c.volatility_threshold)
            .unwrap_or(self.volatility_threshold)
    }

    /// 심볼에 대해 거래가 활성화되어 있는지 확인합니다.
    pub fn is_symbol_enabled(&self, symbol: &str) -> bool {
        self.symbol_configs
            .get(symbol)
            .map(|c| c.enabled)
            .unwrap_or(true)
    }

    /// 심볼별 설정을 추가하거나 업데이트합니다.
    pub fn set_symbol_config(&mut self, symbol: impl Into<String>, config: SymbolRiskConfig) {
        self.symbol_configs.insert(symbol.into(), config);
    }

    /// 설정 값을 검증합니다.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if self.max_position_pct <= 0.0 || self.max_position_pct > 100.0 {
            return Err(ConfigValidationError::InvalidValue(
                "max_position_pct must be between 0 and 100".into(),
            ));
        }

        if self.max_daily_loss_pct <= 0.0 || self.max_daily_loss_pct > 100.0 {
            return Err(ConfigValidationError::InvalidValue(
                "max_daily_loss_pct must be between 0 and 100".into(),
            ));
        }

        if self.default_stop_loss_pct <= 0.0 || self.default_stop_loss_pct > 50.0 {
            return Err(ConfigValidationError::InvalidValue(
                "default_stop_loss_pct must be between 0 and 50".into(),
            ));
        }

        if self.default_take_profit_pct <= 0.0 {
            return Err(ConfigValidationError::InvalidValue(
                "default_take_profit_pct must be greater than 0".into(),
            ));
        }

        if self.min_order_size <= Decimal::ZERO {
            return Err(ConfigValidationError::InvalidValue(
                "min_order_size must be greater than 0".into(),
            ));
        }

        Ok(())
    }
}

/// 설정 검증 오류.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigValidationError {
    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),

    #[error("Missing required field: {0}")]
    MissingField(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RiskConfig::default();

        assert_eq!(config.max_position_pct, 10.0);
        assert_eq!(config.max_daily_loss_pct, 3.0);
        assert_eq!(config.default_stop_loss_pct, 2.0);
        assert_eq!(config.default_take_profit_pct, 5.0);
        assert_eq!(config.max_concurrent_positions, 10);
        assert!(!config.enable_trailing_stop);
    }

    #[test]
    fn test_conservative_config() {
        let config = RiskConfig::conservative();

        assert_eq!(config.max_position_pct, 5.0);
        assert_eq!(config.max_daily_loss_pct, 1.5);
        assert!(config.enable_trailing_stop);
    }

    #[test]
    fn test_aggressive_config() {
        let config = RiskConfig::aggressive();

        assert_eq!(config.max_position_pct, 20.0);
        assert_eq!(config.max_daily_loss_pct, 5.0);
    }

    #[test]
    fn test_symbol_specific_config() {
        let mut config = RiskConfig::default();

        config.set_symbol_config(
            "BTC/USDT",
            SymbolRiskConfig {
                max_position_pct: Some(15.0),
                stop_loss_pct: Some(3.0),
                take_profit_pct: Some(8.0),
                volatility_threshold: None,
                enabled: true,
            },
        );

        // 심볼별 값
        assert_eq!(config.get_max_position_pct("BTC/USDT"), 15.0);
        assert_eq!(config.get_stop_loss_pct("BTC/USDT"), 3.0);
        assert_eq!(config.get_take_profit_pct("BTC/USDT"), 8.0);

        // 설정되지 않은 값은 전역 설정으로 폴백
        assert_eq!(
            config.get_volatility_threshold("BTC/USDT"),
            config.volatility_threshold
        );

        // 알려지지 않은 심볼은 전역 기본값 사용
        assert_eq!(config.get_max_position_pct("ETH/USDT"), 10.0);
        assert_eq!(config.get_stop_loss_pct("ETH/USDT"), 2.0);
    }

    #[test]
    fn test_config_validation() {
        let config = RiskConfig::default();
        assert!(config.validate().is_ok());

        // 유효하지 않은 max_position_pct
        let mut invalid = RiskConfig::default();
        invalid.max_position_pct = 150.0;
        assert!(invalid.validate().is_err());

        // 유효하지 않은 stop_loss_pct
        let mut invalid = RiskConfig::default();
        invalid.default_stop_loss_pct = -1.0;
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_config_serialization() {
        let config = RiskConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RiskConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.max_position_pct, deserialized.max_position_pct);
        assert_eq!(config.max_daily_loss_pct, deserialized.max_daily_loss_pct);
    }

    #[test]
    fn test_symbol_enabled() {
        let mut config = RiskConfig::default();

        // 기본적으로 모든 심볼 활성화
        assert!(config.is_symbol_enabled("BTC/USDT"));

        // 심볼 비활성화
        config.set_symbol_config(
            "RISKY/USDT",
            SymbolRiskConfig {
                enabled: false,
                ..Default::default()
            },
        );

        assert!(!config.is_symbol_enabled("RISKY/USDT"));
        assert!(config.is_symbol_enabled("BTC/USDT")); // 다른 심볼은 여전히 활성화
    }
}
