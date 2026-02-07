//! 머신러닝 및 AI 기능.
//!
//! 이 모듈은 트레이딩을 위한 ML 기반 분석 도구를 제공합니다:
//!
//! - **가격 예측**: 가격 움직임 예측을 위한 ONNX Runtime 기반 모델
//! - **패턴 인식**: 캔들스틱 및 차트 패턴 감지
//! - **Feature Engineering**: ML 입력을 위한 기술 지표 추출
//! - **통합 서비스**: MlService로 모든 기능 통합
//!
//! # 아키텍처
//!
//! ```text
//! Market Data (Klines)
//!        │
//!        ▼
//! ┌─────────────────┐
//! │ Feature Engine  │ ← 기술 지표 추출
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐     ┌──────────────────┐
//! │ Price Predictor │     │ Pattern Detector │
//! │ (ONNX Runtime)  │     │ (규칙 기반)      │
//! └────────┬────────┘     └────────┬─────────┘
//!          │                       │
//!          └───────────┬───────────┘
//!                      ▼
//!              ┌───────────────┐
//!              │  ML Service   │ ← prediction 집계
//!              └───────────────┘
//!                      │
//!                      ▼
//!              Signal.metadata
//! ```
//!
//! # 예제
//!
//! ```ignore
//! use trader_analytics::ml::{MlService, MlServiceConfig};
//!
//! // 기본 설정으로 서비스 생성
//! let service = MlService::with_defaults()?;
//!
//! // 패턴 감지
//! let patterns = service.detect_patterns("BTC/USDT", "1h", &klines);
//! println!("전체 신호: {}", patterns.overall_signal);
//!
//! // 전체 분석
//! let analysis = service.analyze("BTC/USDT", &klines).await?;
//! for pattern in &analysis.candlestick_patterns {
//!     println!("감지: {:?} (신뢰도: {:.1}%)", pattern.pattern_type, pattern.confidence * 100.0);
//! }
//! ```

pub mod error;
pub mod features;
pub mod pattern;
pub mod predictor;
pub mod service;
pub mod types;

// 자주 사용되는 타입 재내보내기
pub use error::{MlError, MlResult};
pub use features::{FeatureConfig, FeatureExtractor};
#[cfg(feature = "ml")]
pub use predictor::OnnxPredictor;
pub use predictor::{MockPredictor, PredictionResult, PredictorConfig, PricePredictor};
pub use types::{ConfidenceLevel, FeatureVector, Prediction, PredictionDirection};

// 패턴 인식 타입 재내보내기
pub use pattern::{
    CandlestickPattern, CandlestickPatternType, ChartPattern, ChartPatternType, PatternConfig,
    PatternPoint, PatternRecognizer, Trendline,
};

// 서비스 타입 재내보내기
pub use service::{
    CandlestickPatternInfo, ChartPatternInfo, FeatureSummary, MlAnalysisResult, MlService,
    MlServiceConfig, PatternDetectionResult,
};
