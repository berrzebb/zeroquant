//! 구조적 피처 re-export.
//!
//! 실제 정의는 `trader_core::domain::StructuralFeatures`에 있습니다.
//! 계산 로직은 `StructuralFeaturesCalculator::from_candles()`을 사용하세요.
//!
//! # 사용 예시
//!
//! ```ignore
//! use trader_analytics::{IndicatorEngine, StructuralFeaturesCalculator};
//! use trader_analytics::indicators::StructuralFeatures;
//!
//! let engine = IndicatorEngine::new();
//! let features = StructuralFeaturesCalculator::from_candles("005930", &candles, &engine)?;
//! ```

pub use trader_core::domain::StructuralFeatures;

/// 최소 필요 캔들 개수.
pub const MIN_CANDLES: usize = 40;
