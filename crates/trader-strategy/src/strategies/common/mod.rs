//! 전략을 위한 공통 유틸리티 및 계산기.
//!
//! 이 모듈은 트레이딩 전략 구축을 위한 재사용 가능한 컴포넌트를 제공합니다:
//!
//! - **defaults**: 전략 기본 상수 (지표, 리스크, 그리드, 모멘텀, 배분)
//! - **indicators**: 기술적 지표 계산 (RSI, SMA, EMA, BB, MACD, ATR)
//! - **position_sizing**: 포지션 크기 계산 (Kelly, FixedRatio, ATR 기반)
//! - **risk_checks**: 리스크 검증 및 관리
//! - **signal_filters**: 신호 필터링 및 확인
//! - **모멘텀**: 자산 배분 전략을 위한 다기간 모멘텀 스코어링
//! - **리밸런싱**: 포트폴리오 리밸런싱 계산
//! - **serde_helpers**: SDUI와 전략 설정 간 타입 변환
//! - **position_sync**: 거래소 중립 포지션 상태 동기화
//! - **global_score_utils**: GlobalScore 기반 종목 선택 및 포지션 가중치 계산
//! - **screening_integration**: 스크리닝 결과 및 RouteState 전략 연동

pub mod defaults;
pub mod exit_config;
pub mod global_score_utils;
pub mod indicators;
pub mod momentum;
pub mod position_sizing;
pub mod position_sync;
pub mod rebalance;
pub mod risk_checks;
pub mod screening_integration;
pub mod serde_helpers;
pub mod signal_filters;

pub use momentum::{
    MomentumCalculator, MomentumConfig, MomentumResult, MomentumScore, WeightedMomentumConfig,
};

pub use position_sync::{FillResult, PositionSync, SyncedPosition};

pub use rebalance::{
    PortfolioPosition, RebalanceCalculator, RebalanceConfig, RebalanceOrder, RebalanceOrderSide,
    RebalanceResult, TargetAllocation,
};

pub use serde_helpers::{deserialize_ticker, deserialize_ticker_opt, deserialize_tickers};

pub use defaults::{
    AllocationDefaults, GridDefaults, IndicatorDefaults, MomentumDefaults, RiskDefaults,
};

pub use indicators::{
    calculate_atr, calculate_bollinger_bands, calculate_ema, calculate_macd, calculate_rsi,
    calculate_sma, BollingerBands, MacdResult,
};

pub use position_sizing::{
    AtrPositionSizer, FixedRatioSizer, GlobalScorePositionSizer, KellyPositionSizer, PositionSize,
    PositionSizer,
};

pub use global_score_utils::{
    adjust_strength_by_score, calculate_risk_adjustment, calculate_score_weight,
    calculate_signal_strength, calculate_weighted_average, get_score, select_top_tickers,
    ScoreFilterOptions,
};

pub use screening_integration::{
    get_tickers_by_global_score, get_tickers_by_route_state, get_tickers_by_state_and_score,
    get_top_tickers_per_sector, ScreeningAware,
};

pub use risk_checks::{DefaultRiskChecker, RiskCheckError, RiskChecker, RiskManager, RiskParams};

pub use signal_filters::{
    CompositeFilter, ConfirmationPattern, FilteredSignal, SignalContext, SignalFilter,
    SignalStrength, TrendFilter, ValidationResult, VolumeFilter,
    can_execute_signal, validate_signals_with_context,
};

pub use exit_config::ExitConfig;
