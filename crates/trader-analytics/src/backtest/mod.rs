//! 백테스팅 모듈
//!
//! 과거 데이터로 트레이딩 전략을 시뮬레이션하고 성과를 분석합니다.
//!
//! # 주요 구성요소
//!
//! - [`BacktestConfig`]: 백테스트 설정 (초기 자본, 수수료, 슬리피지 등)
//! - [`BacktestEngine`]: 백테스트 실행 엔진
//! - [`BacktestReport`]: 백테스트 결과 리포트
//! - [`SlippageModel`]: 동적 슬리피지 모델 (Fixed/Linear/VolatilityBased/Tiered)
//! - [`BacktestScreeningProvider`]: 백테스트용 스크리닝 결과 제공자

pub mod engine;
pub mod screening_provider;
pub mod slippage;

pub use engine::{BacktestConfig, BacktestEngine, BacktestError, BacktestReport, BacktestResult};
pub use screening_provider::{BacktestScreeningConfig, BacktestScreeningProvider, MIN_CANDLES_FOR_SCREENING};
pub use slippage::{SlippageModel, SlippageResult, SlippageTier};

// Re-export core types for convenience
pub use trader_core::{ScreeningCalculator, ScreeningCalculatorConfig, ScreeningUpdateFrequency};
