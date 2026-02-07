//! 분석 및 백테스팅 엔진.
//!
//! 이 크레이트는 다음을 제공합니다:
//! - 성과 지표 계산
//! - 포트폴리오 분석 (자산 곡선, 차트)
//! - 백테스팅 엔진
//! - ML/AI 모델 추론 (ONNX) - `ml` feature 필요
//! - 기술적 지표
//!
//! # Re-exports
//!
//! - [`performance`]: 성과 지표 계산 (PerformanceMetrics, RollingMetrics 등)
//! - [`portfolio`]: 포트폴리오 분석 (EquityCurve, PortfolioCharts 등)
//! - [`backtest`]: 백테스팅 엔진 - `backtest` feature 필요
//! - [`ml`]: ML/AI 기능 (패턴 인식, 피처 추출, 예측) - `ml` feature 필요

pub mod analytics_provider_impl;
#[cfg(feature = "backtest")]
pub mod backtest;
pub mod correlation;
pub mod global_scorer;
pub mod indicators;
pub mod journal_integration;
pub mod liquidity_gate;
pub mod market_regime_calculator;
#[cfg(feature = "ml")]
pub mod ml;
pub mod multi_timeframe_helpers;
pub mod performance;
pub mod portfolio;
pub mod route_state_calculator;
pub mod sector_rs;
pub mod seven_factor;
pub mod structural_features;
pub mod survival;
pub mod timeframe_alignment;
pub mod trigger_calculator;
pub mod volume_profile;

// Performance 모듈 re-exports
pub use performance::metrics::{
    PerformanceMetrics, RollingMetrics, RoundTrip, DEFAULT_RISK_FREE_RATE, TRADING_DAYS_PER_YEAR,
};
pub use performance::tracker::{PerformanceEvent, PerformanceThresholds, PerformanceTracker};

// Portfolio 모듈 re-exports
pub use portfolio::charts::{
    ChartPoint, MonthlyReturnCell, PerformanceSummary, PeriodPerformance, PortfolioCharts,
};
pub use portfolio::equity_curve::{
    DrawdownPeriod, EquityCurve, EquityCurveBuilder, EquityPoint, TimeFrame,
};

// Journal Integration 모듈 re-exports
pub use journal_integration::{
    export_backtest_to_journal, export_backtest_trades, JournalTradeInput,
};

// Indicators 모듈 re-exports
pub use indicators::{
    AtrParams,
    // 변동성 지표
    BollingerBandsParams,
    BollingerBandsResult,
    EmaParams,
    IndicatorEngine,
    IndicatorError,
    IndicatorResult,
    // Keltner Channel
    KeltnerChannelParams,
    KeltnerChannelResult,
    MacdParams,
    MacdResult,
    MomentumCalculator,
    // OBV
    ObvIndicator,
    ObvParams,
    ObvResult,
    // 모멘텀 지표
    RsiParams,
    // 추세 지표
    SmaParams,
    StochasticParams,
    StochasticResult,
    // 구조적 피처
    StructuralFeatures,
    // SuperTrend
    SuperTrendIndicator,
    SuperTrendParams,
    SuperTrendResult,
    TrendIndicators,
    VolatilityIndicators,
    // VWAP
    VwapIndicator,
    VwapParams,
    VwapResult,
};

// RouteState 계산기 re-export
pub use route_state_calculator::RouteStateCalculator;

// StructuralFeatures 계산기 re-export
pub use structural_features::StructuralFeaturesCalculator;

// MarketRegime 계산기 re-export
pub use market_regime_calculator::{MarketRegimeCalculator, MarketRegimeResult};

// Trigger 계산기 re-export
pub use trigger_calculator::{TriggerCalculator, TriggerError};

// Global Scorer re-export
pub use global_scorer::{GlobalScorer, GlobalScorerError, GlobalScorerParams, GlobalScorerResult};

// Liquidity Gate re-export
pub use liquidity_gate::{LiquidityGate, LiquidityLevel};

// 7Factor re-export
pub use seven_factor::{SevenFactorCalculator, SevenFactorInput, SevenFactorScores};

// Sector RS re-export
pub use sector_rs::{
    enrich_screening_with_sector_rs, SectorRsCalculator, SectorRsInput, SectorRsResult,
    TickerSectorRs,
};

// Survival Tracker re-export
pub use survival::{
    get_survival_days_map, DailyRanking, DailyRankingBuilder, SurvivalResult, SurvivalTracker,
};

// Volume Profile re-export
pub use volume_profile::{
    calculate_volume_profile, PriceLevel, VolumeProfile, VolumeProfileCalculator,
};

// Correlation re-export
pub use correlation::{
    calculate_correlation, calculate_correlation_matrix, calculate_correlation_matrix_decimal,
    CorrelationMatrix,
};

// AnalyticsProvider 구현체 re-export
pub use analytics_provider_impl::AnalyticsProviderImpl;

// Multi-timeframe helpers re-export
pub use multi_timeframe_helpers::{
    analyze_trend, combine_signals, default_weights, detect_divergence, CombinedSignal,
    DivergenceType, SignalDirection, TrendAnalysis, TrendDirection,
};

// Timeframe Alignment re-export
pub use timeframe_alignment::TimeframeAligner;

// ML 모듈 re-exports (ml feature 필요)
#[cfg(feature = "ml")]
pub use ml::{
    // 패턴 인식
    CandlestickPattern,
    // 통합 서비스
    CandlestickPatternInfo,
    CandlestickPatternType,
    ChartPattern,
    ChartPatternInfo,
    ChartPatternType,
    // 예측
    ConfidenceLevel,
    // 피처 추출
    FeatureConfig,
    FeatureExtractor,
    FeatureSummary,
    FeatureVector,
    MlAnalysisResult,
    // 에러 타입
    MlError,
    MlResult,
    MlService,
    MlServiceConfig,
    MockPredictor,
    OnnxPredictor,
    PatternConfig,
    PatternDetectionResult,
    PatternRecognizer,
    Prediction,
    PredictionDirection,
    PredictionResult,
    PredictorConfig,
    PricePredictor,
};
