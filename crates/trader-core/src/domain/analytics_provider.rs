//! 분석 결과 제공자 trait 및 관련 타입.
//!
//! 이 모듈은 전략에서 분석 결과를 조회하기 위한 추상화 계층을 제공합니다.
//! 실제 분석 로직(GlobalScorer, RouteStateAnalyzer 등)은 Phase 1에서 구현됩니다.

use crate::types::MarketType;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;

use super::market_data::Kline;

// Re-export RouteState from route_state module for convenience
pub use super::route_state::RouteState;
// Re-export MarketRegime, MacroEnvironment, MarketBreadth for convenience
pub use super::macro_environment::MacroEnvironment;
pub use super::market_breadth::MarketBreadth;
pub use super::market_regime::MarketRegime;

// ================================================================================================
// Error Types
// ================================================================================================

/// AnalyticsProvider 에러 타입.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalyticsError {
    /// 데이터 조회 실패
    DataFetch(String),
    /// 계산 오류
    Calculation(String),
    /// 지원하지 않는 기능
    Unsupported(String),
    /// 기타 오류
    Other(String),
}

impl fmt::Display for AnalyticsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalyticsError::DataFetch(msg) => write!(f, "Data fetch error: {}", msg),
            AnalyticsError::Calculation(msg) => write!(f, "Calculation error: {}", msg),
            AnalyticsError::Unsupported(msg) => write!(f, "Unsupported: {}", msg),
            AnalyticsError::Other(msg) => write!(f, "Analytics error: {}", msg),
        }
    }
}

impl StdError for AnalyticsError {}

// ================================================================================================
// Core Types
// ================================================================================================

/// Global Score 결과.
///
/// 시장 전체 또는 종목별 종합 점수를 나타냅니다.
/// 실제 계산 로직은 Phase 1에서 구현됩니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalScoreResult {
    /// 종목 티커 (종목별 점수인 경우)
    pub ticker: Option<String>,
    /// 시장 유형 (시장별 점수인 경우)
    pub market_type: Option<MarketType>,
    /// 종합 점수 (0.0 ~ 100.0)
    pub overall_score: Decimal,
    /// 컴포넌트별 점수 (예: "momentum": 75.0, "trend": 80.0)
    pub component_scores: HashMap<String, Decimal>,
    /// 추천 방향 (BUY/SELL/HOLD)
    pub recommendation: String,
    /// 신뢰도 (0.0 ~ 1.0)
    pub confidence: Decimal,
    /// 계산 시각
    pub timestamp: DateTime<Utc>,
}

/// 스크리닝 결과.
///
/// 특정 프리셋을 통과한 종목의 스크리닝 결과를 나타냅니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreeningResult {
    /// 종목 티커
    pub ticker: String,
    /// 프리셋 이름
    pub preset_name: String,
    /// 통과 여부
    pub passed: bool,
    /// 종합 점수 (0.0 ~ 100.0)
    pub overall_score: Decimal,
    /// 경로 상태
    pub route_state: RouteState,
    /// 조건별 결과 (조건명 -> 통과 여부)
    pub criteria_results: HashMap<String, bool>,
    /// 계산 시각
    pub timestamp: DateTime<Utc>,
    /// 섹터 상대강도 점수
    pub sector_rs: Option<Decimal>,
    /// 섹터 순위
    pub sector_rank: Option<i32>,
    /// 진입 트리거 점수 (0~100, 높을수록 강한 신호)
    pub trigger_score: Option<f64>,
    /// 진입 트리거 라벨 (예: "🚀스퀴즈 해제, 📊거래량 폭증")
    pub trigger_label: Option<String>,
}

/// 스크리닝 프리셋.
///
/// 스크리닝 조건 세트를 정의합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreeningPreset {
    /// 프리셋 이름
    pub name: String,
    /// 설명
    pub description: String,
    /// 시장 유형 필터
    pub market_types: Vec<MarketType>,
    /// 활성화된 조건 목록 (조건명)
    pub enabled_criteria: Vec<String>,
    /// 조건별 임계값 (조건명 -> 값)
    pub thresholds: HashMap<String, Decimal>,
    /// 최소 점수
    pub min_score: Decimal,
}

impl ScreeningPreset {
    /// 기본 프리셋 생성.
    pub fn default_preset() -> Self {
        Self {
            name: "default".to_string(),
            description: "Default screening preset".to_string(),
            market_types: vec![],
            enabled_criteria: vec![],
            thresholds: HashMap::new(),
            min_score: Decimal::ZERO,
        }
    }
}

/// 구조적 피처.
///
/// "살아있는 횡보"와 "죽은 횡보"를 구분하여 돌파 가능성을 예측합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralFeatures {
    /// 종목 티커
    pub ticker: String,
    /// Higher Low 강도 (-1.0 ~ 1.0, 양수=저점 상승)
    pub low_trend: Decimal,
    /// 매집/이탈 판별 (0 ~ 5, 2.0 이상=매집, -2.0 이하=이탈)
    pub vol_quality: Decimal,
    /// 박스권 위치 (0.0 ~ 1.0, 0=하단, 1=상단)
    pub range_pos: Decimal,
    /// MA20 이격도 (%, -20 ~ +20)
    pub dist_ma20: Decimal,
    /// 볼린저 밴드 폭 (%, 0 ~ 50)
    pub bb_width: Decimal,
    /// 볼린저 밴드 상단
    pub bb_upper: Decimal,
    /// 볼린저 밴드 중간 (SMA20)
    pub bb_middle: Decimal,
    /// 볼린저 밴드 하단
    pub bb_lower: Decimal,
    /// RSI 14일 (0 ~ 100)
    pub rsi: Decimal,
    /// 계산 시각
    pub timestamp: DateTime<Utc>,
}

impl StructuralFeatures {
    /// 돌파 가능성 점수 계산 (0 ~ 100).
    ///
    /// 가중치 기반:
    /// - low_trend: 30%
    /// - vol_quality: 25%
    /// - range_pos: 20%
    /// - bb_width: 15% (좁을수록 가산)
    /// - dist_ma20: 10%
    pub fn breakout_score(&self) -> Decimal {
        let low_trend_score =
            (self.low_trend * Decimal::new(3, 1) + Decimal::new(3, 1)) * Decimal::new(50, 0);
        let vol_quality_score =
            (self.vol_quality * Decimal::new(25, 2) + Decimal::new(25, 2)) * Decimal::new(50, 0);
        let range_score = self.range_pos * Decimal::new(2, 1) * Decimal::new(100, 0);
        let bb_ratio = (self.bb_width / Decimal::new(20, 0)).min(Decimal::ONE);
        let bb_score = (Decimal::ONE - bb_ratio) * Decimal::new(15, 2) * Decimal::new(100, 0);
        let ma_ratio = (self.dist_ma20.abs() / Decimal::new(10, 0)).min(Decimal::ONE);
        let ma_score = ma_ratio * Decimal::new(1, 1) * Decimal::new(100, 0);

        (low_trend_score + vol_quality_score + range_score + bb_score + ma_score)
            .max(Decimal::ZERO)
            .min(Decimal::new(100, 0))
    }

    /// "살아있는 횡보" 판정.
    ///
    /// 조건:
    /// - low_trend > 0.2 (저가 상승)
    /// - vol_quality > 0.1 (매집 패턴)
    /// - bb_width < 3.0 (변동성 수축)
    pub fn is_alive_consolidation(&self) -> bool {
        self.low_trend > Decimal::new(2, 1)
            && self.vol_quality > Decimal::new(1, 1)
            && self.bb_width < Decimal::new(3, 0)
    }
}

impl Default for StructuralFeatures {
    fn default() -> Self {
        Self {
            ticker: String::new(),
            low_trend: Decimal::ZERO,
            vol_quality: Decimal::ZERO,
            range_pos: Decimal::new(5, 1),
            dist_ma20: Decimal::ZERO,
            bb_width: Decimal::ZERO,
            bb_upper: Decimal::ZERO,
            bb_middle: Decimal::ZERO,
            bb_lower: Decimal::ZERO,
            rsi: Decimal::new(50, 0),
            timestamp: Utc::now(),
        }
    }
}

// ================================================================================================
// AnalyticsProvider Trait
// ================================================================================================

/// 분석 결과 제공자.
///
/// 전략에서 분석 결과를 조회하기 위한 추상화 계층입니다.
/// 실제 구현체는 Phase 1에서 제공됩니다.
#[async_trait]
pub trait AnalyticsProvider: Send + Sync {
    /// Global Score 조회 (시장별).
    ///
    /// 특정 시장의 종합 점수를 조회합니다.
    ///
    /// # Arguments
    /// * `market_type` - 조회할 시장 유형
    ///
    /// # Returns
    /// GlobalScoreResult 리스트
    async fn fetch_global_scores(
        &self,
        market_type: MarketType,
    ) -> Result<Vec<GlobalScoreResult>, AnalyticsError>;

    /// RouteState 조회 (종목별).
    ///
    /// 특정 종목들의 경로 상태를 조회합니다.
    ///
    /// # Arguments
    /// * `tickers` - 조회할 종목 티커 목록
    ///
    /// # Returns
    /// ticker -> RouteState 매핑
    async fn fetch_route_states(
        &self,
        tickers: &[&str],
    ) -> Result<HashMap<String, RouteState>, AnalyticsError>;

    /// 스크리닝 결과 조회.
    ///
    /// 특정 프리셋으로 스크리닝한 결과를 조회합니다.
    ///
    /// # Arguments
    /// * `preset` - 스크리닝 프리셋
    ///
    /// # Returns
    /// ScreeningResult 리스트
    async fn fetch_screening(
        &self,
        preset: ScreeningPreset,
    ) -> Result<Vec<ScreeningResult>, AnalyticsError>;

    /// 구조적 피처 조회.
    ///
    /// 특정 종목들의 구조적 특징을 조회합니다.
    ///
    /// # Arguments
    /// * `tickers` - 조회할 종목 티커 목록
    ///
    /// # Returns
    /// ticker -> StructuralFeatures 매핑
    async fn fetch_features(
        &self,
        tickers: &[&str],
    ) -> Result<HashMap<String, StructuralFeatures>, AnalyticsError>;

    /// MarketRegime 조회 (종목별).
    ///
    /// 특정 종목들의 시장 레짐(추세 단계)을 조회합니다.
    ///
    /// # Arguments
    /// * `tickers` - 조회할 종목 티커 목록
    ///
    /// # Returns
    /// ticker -> MarketRegime 매핑
    async fn fetch_market_regimes(
        &self,
        tickers: &[&str],
    ) -> Result<HashMap<String, MarketRegime>, AnalyticsError>;

    /// MacroEnvironment 조회.
    ///
    /// 현재 매크로 환경(환율, 나스닥 등)을 조회합니다.
    ///
    /// # Returns
    /// 현재 MacroEnvironment
    async fn fetch_macro_environment(&self) -> Result<MacroEnvironment, AnalyticsError>;

    /// MarketBreadth 조회.
    ///
    /// 현재 시장 폭(20일선 상회 비율 등)을 조회합니다.
    ///
    /// # Returns
    /// 현재 MarketBreadth
    async fn fetch_market_breadth(&self) -> Result<MarketBreadth, AnalyticsError>;
}

// ================================================================================================
// ScreeningCalculator Trait (백테스트용 스크리닝 계산 추상화)
// ================================================================================================

/// 스크리닝 업데이트 주기.
///
/// 백테스트에서 스크리닝 결과를 얼마나 자주 갱신할지 결정합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScreeningUpdateFrequency {
    /// 매 캔들마다 (성능 저하 주의)
    EveryCandle,
    /// 일간 (일봉 기준 매일)
    Daily,
    /// 주간 (월요일)
    Weekly,
    /// 월간 (1일)
    #[default]
    Monthly,
    /// 사용자 정의 캔들 수마다
    Custom(usize),
}


/// 스크리닝 계산 요청 설정.
///
/// 전략이 백테스트에서 필요로 하는 스크리닝 설정을 정의합니다.
/// 이 타입은 trader-core에 정의되어 순환 의존성 없이 사용할 수 있습니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreeningCalculatorConfig {
    /// 스크리닝 프리셋 이름 (전략에서 사용)
    pub preset_name: String,
    /// 스크리닝 업데이트 주기
    pub update_frequency: ScreeningUpdateFrequency,
    /// 최소 GlobalScore 임계값 (passed 판정 기준)
    pub min_score: Decimal,
}

impl Default for ScreeningCalculatorConfig {
    fn default() -> Self {
        Self {
            preset_name: "backtest".to_string(),
            update_frequency: ScreeningUpdateFrequency::Monthly,
            min_score: Decimal::from(60),
        }
    }
}

impl ScreeningCalculatorConfig {
    /// 새 스크리닝 설정 생성
    pub fn new(preset_name: impl Into<String>, update_frequency: ScreeningUpdateFrequency, min_score: Decimal) -> Self {
        Self {
            preset_name: preset_name.into(),
            update_frequency,
            min_score,
        }
    }

    /// 월간 업데이트 스크리닝 설정
    pub fn monthly(preset_name: impl Into<String>, min_score: Decimal) -> Self {
        Self::new(preset_name, ScreeningUpdateFrequency::Monthly, min_score)
    }

    /// 주간 업데이트 스크리닝 설정
    pub fn weekly(preset_name: impl Into<String>, min_score: Decimal) -> Self {
        Self::new(preset_name, ScreeningUpdateFrequency::Weekly, min_score)
    }
}

/// 스크리닝 계산 trait.
///
/// 백테스트에서 캔들 데이터 기반으로 스크리닝 결과를 계산하는 추상화 계층입니다.
/// 실제 구현체(`BacktestScreeningProvider`)는 `trader-analytics`에서 제공됩니다.
///
/// # 의존성 역전 원칙 (DIP)
///
/// - 이 trait은 상위 모듈(`trader-core`)에 정의
/// - 구체 구현체는 하위 모듈(`trader-analytics`)에서 제공
/// - `BacktestEngine`이 이 trait을 주입받아 사용
///
/// # 사용 예시
///
/// ```ignore
/// // BacktestEngine에서 스크리닝 계산기 주입
/// let screening_calculator: Box<dyn ScreeningCalculator> = ...;
/// engine.run_with_screening(strategy, klines, context, screening_calculator).await?;
/// ```
pub trait ScreeningCalculator: Send + Sync {
    /// 캔들 데이터 기반 스크리닝 결과 생성.
    ///
    /// # 인자
    ///
    /// * `all_klines` - 종목별 캔들 데이터 (ticker → Vec<Kline>)
    /// * `current_time` - 현재 시점 (Look-Ahead Bias 방지용)
    ///
    /// # 반환
    ///
    /// 점수 순으로 정렬된 스크리닝 결과 벡터
    fn calculate_from_klines(
        &self,
        all_klines: &HashMap<String, Vec<Kline>>,
        current_time: DateTime<Utc>,
    ) -> Vec<ScreeningResult>;

    /// 스크리닝 설정 조회.
    fn config(&self) -> &ScreeningCalculatorConfig;

    /// 스크리닝 업데이트 필요 여부 판단.
    ///
    /// # 인자
    ///
    /// * `idx` - 현재 캔들 인덱스
    /// * `current_time` - 현재 시점
    /// * `last_update` - 마지막 업데이트 시점 (None이면 첫 업데이트)
    ///
    /// # 반환
    ///
    /// 업데이트 필요 여부
    fn should_update(
        &self,
        idx: usize,
        current_time: DateTime<Utc>,
        last_update: Option<DateTime<Utc>>,
    ) -> bool;
}
