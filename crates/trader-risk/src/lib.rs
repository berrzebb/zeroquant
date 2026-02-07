//! 리스크 관리 시스템.
//!
//! 이 crate는 다음 기능을 제공합니다:
//! - 리스크 한도에 대한 주문 검증
//! - 포지션 사이징
//! - Stop-loss/Take-profit 관리
//! - 일일 손실 한도
//! - 변동성 필터
//!
//! # 예제
//!
//! ```rust,ignore
//! use trader_risk::{RiskManager, RiskConfig};
//!
//! let config = RiskConfig::default();
//! let manager = RiskManager::new(config);
//!
//! // 주문 검증
//! let validation = manager.validate_order(&order, &positions)?;
//! if validation.is_valid {
//!     // 주문 진행
//! }
//! ```

pub mod config;
pub mod limits;
pub mod manager;
pub mod position_sizing;
pub mod stop_loss;
pub mod trailing_stop;

// 주요 타입 재내보내기
pub use config::{ConfigValidationError, RiskConfig, SymbolRiskConfig};
pub use limits::{DailyLimitStatus, DailyLossTracker, PnLRecord, RiskLimits};
pub use manager::{RiskManager, RiskValidation};
pub use position_sizing::{PositionSizer, SizingValidation};
pub use stop_loss::{StopOrder, StopOrderGenerator, StopType, TrailingStopState};
pub use trailing_stop::{
    EnhancedTrailingStop, ProfitLevel, StepTrailingStopBuilder, TrailingStopMode, TrailingStopStats,
};
