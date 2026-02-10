//! 통합 자산 배분 전략.
//!
//! HAA, XAA, BAA 등 자산 배분 전략을 통합한 베이스 전략입니다.
//! 설정을 통해 각 전략의 특성을 구현할 수 있습니다.
//!
//! # 지원 전략 유형
//!
//! - **HAA (Hybrid Asset Allocation)**: 카나리아 기반 공격/방어 전환
//! - **XAA (Extended Asset Allocation)**: 채권 모멘텀 기반 비중 조절
//! - **BAA (Balanced Asset Allocation)**: 가중 모멘텀, 버전 선택
//!
//! # 공통 로직
//!
//! 1. 카나리아 자산으로 시장 상태 판단
//! 2. 공격 또는 방어 모드 결정
//! 3. 모멘텀 기준 상위 N개 자산 선택
//! 4. 월간 리밸런싱 실행
//!
//! # 예시
//!
//! ```rust,ignore
//! use trader_strategy::strategies::asset_allocation::{
//!     AssetAllocationConfig, AssetAllocationStrategy, StrategyVariant
//! };
//!
//! // HAA 스타일 전략
//! let config = AssetAllocationConfig::haa_default();
//! let strategy = AssetAllocationStrategy::new(config);
//!
//! // XAA 스타일 전략
//! let config = AssetAllocationConfig::xaa_default();
//! let strategy = AssetAllocationStrategy::new(config);
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use trader_strategy_macro::StrategyConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use trader_core::domain::{RouteState, StrategyContext};
use trader_core::types::Timeframe;
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, SignalType};

use super::common::{
    ExitConfig, MomentumCalculator, MomentumConfig, MomentumResult, PortfolioPosition,
    RebalanceCalculator, RebalanceConfig, TargetAllocation,
};
use crate::traits::Strategy;

// ================================================================================================
// 전략 변형 열거형
// ================================================================================================

/// 자산 배분 전략 변형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum StrategyVariant {
    /// HAA (Hybrid Asset Allocation)
    #[default]
    Haa,
    /// XAA (Extended Asset Allocation)
    Xaa,
    /// BAA (Balanced Asset Allocation)
    Baa,
    /// All Weather (올웨더)
    AllWeather,
    /// Dual Momentum (듀얼 모멘텀)
    DualMomentum,
    /// 사용자 정의
    Custom,
}


/// 포트폴리오 모드.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum PortfolioMode {
    /// 공격 모드 (주식 위주)
    Offensive,
    /// 방어 모드 (채권/현금 위주)
    #[default]
    Defensive,
}


/// 모멘텀 선택 방식.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MomentumMethod {
    /// 단순 평균 (HAA 스타일): 1/3/6/12개월 평균
    SimpleAverage { periods_months: Vec<usize> },
    /// 가중 모멘텀 (BAA 스타일)
    Weighted {
        period_weights: Vec<(usize, Decimal)>,
    },
    /// XAA 스타일: 기본 + 6개월 모멘텀 별도 계산
    Extended {
        base_periods_months: Vec<usize>,
        bond_period_months: usize,
    },
}

impl Default for MomentumMethod {
    fn default() -> Self {
        Self::SimpleAverage {
            periods_months: vec![1, 3, 6, 12],
        }
    }
}

// ================================================================================================
// 자산 정의
// ================================================================================================

/// 자산 유형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetCategory {
    /// 카나리아 자산 (시장 상태 판단용)
    Canary,
    /// 공격 자산 (주식 등)
    Offensive,
    /// 방어 자산 (채권 등)
    Defensive,
    /// 채권 자산 (XAA 전용)
    Bond,
    /// 안전 자산 (XAA 전용)
    Safe,
    /// 현금/단기 채권
    Cash,
}

/// 자산 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetDefinition {
    /// 티커 심볼
    pub ticker: String,
    /// 자산 카테고리
    pub category: AssetCategory,
    /// 설명
    pub description: Option<String>,
}

impl AssetDefinition {
    /// 새 자산 정의 생성.
    pub fn new(ticker: impl Into<String>, category: AssetCategory) -> Self {
        Self {
            ticker: ticker.into(),
            category,
            description: None,
        }
    }

    /// 설명 추가.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

// ================================================================================================
// 설정
// ================================================================================================

/// 자산 배분 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "asset_allocation",
    name = "자산 배분",
    description = "HAA/XAA/BAA/AllWeather 기반 자산 배분 전략",
    category = "Monthly"
)]
pub struct AssetAllocationConfig {
    /// 전략 변형
    #[schema(label = "전략 변형")]
    pub variant: StrategyVariant,

    /// 자산 목록
    #[schema(label = "자산 목록")]
    pub assets: Vec<AssetDefinition>,

    /// 모멘텀 계산 방식
    #[schema(label = "모멘텀 방식")]
    pub momentum_method: MomentumMethod,

    /// 공격 자산 선택 수
    #[schema(label = "공격 자산 선택 수", min = 1, max = 10)]
    pub offensive_top_n: usize,

    /// 방어 자산 선택 수
    #[schema(label = "방어 자산 선택 수", min = 1, max = 10)]
    pub defensive_top_n: usize,

    /// 현금 티커
    #[schema(label = "현금 티커")]
    pub cash_ticker: String,

    /// 투자 비율 (0.0 ~ 1.0)
    #[schema(label = "투자 비율", min = 0, max = 1)]
    pub invest_rate: Decimal,

    /// 리밸런싱 임계값 (%)
    #[schema(label = "리밸런싱 임계값 (%)", min = 1, max = 20)]
    pub rebalance_threshold: Decimal,

    /// 최소 Global Score
    #[schema(label = "최소 GlobalScore")]
    pub min_global_score: Option<Decimal>,

    /// 카나리아 임계값 (양수 모멘텀 비율)
    #[schema(label = "카나리아 임계값", min = 0, max = 1)]
    pub canary_threshold: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default = "ExitConfig::for_rebalancing")]
    pub exit_config: ExitConfig,
}

impl Default for AssetAllocationConfig {
    fn default() -> Self {
        Self::haa_default()
    }
}

// ================================================================================================
// 전략별 UI Config (SDUI용)
// ================================================================================================

/// HAA 자산 배분 설정 (UI용).
///
/// Hybrid Asset Allocation: 카나리아 자산으로 시장 상태 판단 후
/// 공격/방어 모드 전환하는 전략.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "haa",
    name = "HAA 자산배분",
    description = "계층적 자산 배분 전략 (카나리아 기반 공격/방어 모드 전환)",
    category = "Monthly"
)]
pub struct HaaConfig {
    /// 현금 티커
    #[schema(label = "현금 티커", section = "asset", default = "BIL")]
    pub cash_ticker: String,

    /// 공격 자산 선택 수
    #[schema(label = "공격 자산 선택 수", section = "filter", field_type = "integer", min = 1, max = 10, default = "2")]
    pub offensive_top_n: usize,

    /// 방어 자산 선택 수
    #[schema(label = "방어 자산 선택 수", section = "filter", field_type = "integer", min = 1, max = 10, default = "2")]
    pub defensive_top_n: usize,

    /// 투자 비율 (0.0 ~ 1.0)
    #[schema(label = "투자 비율", section = "timing", field_type = "number", min = 0, max = 1, default = "1.0")]
    pub invest_rate: Decimal,

    /// 리밸런싱 임계값 (%)
    #[schema(label = "리밸런싱 임계값 (%)", section = "timing", field_type = "number", min = 0.01, max = 20, default = "0.01")]
    pub rebalance_threshold: Decimal,

    /// 최소 Global Score
    #[schema(label = "최소 GlobalScore", section = "filter", field_type = "number", min = 0, max = 100, default = "0")]
    pub min_global_score: Decimal,

    /// 카나리아 임계값 (양수 모멘텀 비율)
    #[schema(label = "카나리아 임계값", section = "filter", field_type = "number", min = 0, max = 1, default = "0")]
    pub canary_threshold: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default = "ExitConfig::for_rebalancing")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

impl From<HaaConfig> for AssetAllocationConfig {
    fn from(cfg: HaaConfig) -> Self {
        let mut base = Self::haa_default();
        base.cash_ticker = cfg.cash_ticker;
        base.offensive_top_n = cfg.offensive_top_n;
        base.defensive_top_n = cfg.defensive_top_n;
        base.invest_rate = cfg.invest_rate;
        base.rebalance_threshold = cfg.rebalance_threshold;
        base.min_global_score = Some(cfg.min_global_score);
        base.canary_threshold = cfg.canary_threshold;
        base.exit_config = cfg.exit_config;
        base
    }
}

/// XAA 자산 배분 설정 (UI용).
///
/// Extended Asset Allocation: 채권 최적화가 포함된 확장 자산 배분.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "xaa",
    name = "XAA 자산배분",
    description = "확장 자산 배분 전략 (채권 최적화 포함)",
    category = "Monthly"
)]
pub struct XaaConfig {
    /// 현금 티커
    #[schema(label = "현금 티커", section = "asset", default = "BIL")]
    pub cash_ticker: String,

    /// 공격 자산 선택 수
    #[schema(label = "공격 자산 선택 수", section = "filter", field_type = "integer", min = 1, max = 10, default = "2")]
    pub offensive_top_n: usize,

    /// 방어 자산 선택 수
    #[schema(label = "방어 자산 선택 수", section = "filter", field_type = "integer", min = 1, max = 10, default = "2")]
    pub defensive_top_n: usize,

    /// 채권 모멘텀 기간 (개월)
    #[schema(label = "채권 모멘텀 기간", section = "indicator", field_type = "integer", min = 1, max = 24, default = "3")]
    pub bond_momentum_months: usize,

    /// 투자 비율 (0.0 ~ 1.0)
    #[schema(label = "투자 비율", section = "timing", field_type = "number", min = 0, max = 1, default = "1.0")]
    pub invest_rate: Decimal,

    /// 리밸런싱 임계값 (%)
    #[schema(label = "리밸런싱 임계값 (%)", section = "timing", field_type = "number", min = 0.01, max = 20, default = "0.01")]
    pub rebalance_threshold: Decimal,

    /// 최소 Global Score
    #[schema(label = "최소 GlobalScore", section = "filter", field_type = "number", min = 0, max = 100, default = "0")]
    pub min_global_score: Decimal,

    /// 카나리아 임계값
    #[schema(label = "카나리아 임계값", section = "filter", field_type = "number", min = 0, max = 1, default = "0")]
    pub canary_threshold: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default = "ExitConfig::for_rebalancing")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

impl From<XaaConfig> for AssetAllocationConfig {
    fn from(cfg: XaaConfig) -> Self {
        let mut base = Self::xaa_default();
        base.cash_ticker = cfg.cash_ticker;
        base.offensive_top_n = cfg.offensive_top_n;
        base.defensive_top_n = cfg.defensive_top_n;
        base.momentum_method = MomentumMethod::Extended {
            base_periods_months: vec![1, 3, 6, 12],
            bond_period_months: cfg.bond_momentum_months,
        };
        base.invest_rate = cfg.invest_rate;
        base.rebalance_threshold = cfg.rebalance_threshold;
        base.min_global_score = Some(cfg.min_global_score);
        base.canary_threshold = cfg.canary_threshold;
        base.exit_config = cfg.exit_config;
        base
    }
}

/// BAA 자산 배분 설정 (UI용).
///
/// Balanced Asset Allocation: 가중 모멘텀 기반 균형 자산 배분.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "baa",
    name = "BAA 자산배분",
    description = "균형 자산 배분 전략 (가중 모멘텀 기반)",
    category = "Monthly"
)]
pub struct BaaConfig {
    /// 현금 티커
    #[schema(label = "현금 티커", section = "asset", default = "BIL")]
    pub cash_ticker: String,

    /// 공격 자산 선택 수 (BAA는 보통 1개)
    #[schema(label = "공격 자산 선택 수", section = "filter", field_type = "integer", min = 1, max = 5, default = "1")]
    pub offensive_top_n: usize,

    /// 방어 자산 선택 수
    #[schema(label = "방어 자산 선택 수", section = "filter", field_type = "integer", min = 1, max = 10, default = "2")]
    pub defensive_top_n: usize,

    /// 투자 비율 (0.0 ~ 1.0)
    #[schema(label = "투자 비율", section = "timing", field_type = "number", min = 0, max = 1, default = "1.0")]
    pub invest_rate: Decimal,

    /// 리밸런싱 임계값 (%)
    #[schema(label = "리밸런싱 임계값 (%)", section = "timing", field_type = "number", min = 0.01, max = 20, default = "0.01")]
    pub rebalance_threshold: Decimal,

    /// 최소 Global Score
    #[schema(label = "최소 GlobalScore", section = "filter", field_type = "number", min = 0, max = 100, default = "0")]
    pub min_global_score: Decimal,

    /// 카나리아 임계값 (BAA는 75% 권장)
    #[schema(label = "카나리아 임계값", section = "filter", field_type = "number", min = 0, max = 1, default = "0")]
    pub canary_threshold: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default = "ExitConfig::for_rebalancing")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

impl From<BaaConfig> for AssetAllocationConfig {
    fn from(cfg: BaaConfig) -> Self {
        let mut base = Self::baa_default();
        base.cash_ticker = cfg.cash_ticker;
        base.offensive_top_n = cfg.offensive_top_n;
        base.defensive_top_n = cfg.defensive_top_n;
        base.invest_rate = cfg.invest_rate;
        base.rebalance_threshold = cfg.rebalance_threshold;
        base.min_global_score = Some(cfg.min_global_score);
        base.canary_threshold = cfg.canary_threshold;
        base.exit_config = cfg.exit_config;
        base
    }
}

/// All Weather 설정 (UI용).
///
/// 레이 달리오의 올웨더 포트폴리오: 정적 자산 배분으로
/// 카나리아 로직이나 글로벌 스코어 필터 없이 운영.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "all_weather",
    name = "All Weather",
    description = "레이 달리오 올웨더 포트폴리오 (정적 자산 배분)",
    category = "Monthly"
)]
pub struct AllWeatherConfig {
    /// 현금 티커
    #[schema(label = "현금 티커", section = "asset", default = "BIL")]
    pub cash_ticker: String,

    /// 투자 비율 (0.0 ~ 1.0)
    #[schema(label = "투자 비율", section = "timing", field_type = "number", min = 0, max = 1, default = "1.0")]
    pub invest_rate: Decimal,

    /// 리밸런싱 임계값 (%)
    #[schema(label = "리밸런싱 임계값 (%)", section = "timing", field_type = "number", min = 1, max = 20, default = "5")]
    pub rebalance_threshold: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default = "ExitConfig::for_rebalancing")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

impl From<AllWeatherConfig> for AssetAllocationConfig {
    fn from(cfg: AllWeatherConfig) -> Self {
        let mut base = Self::all_weather_default();
        base.cash_ticker = cfg.cash_ticker;
        base.invest_rate = cfg.invest_rate;
        base.rebalance_threshold = cfg.rebalance_threshold;
        base.exit_config = cfg.exit_config;
        // AllWeather는 min_global_score와 canary 미사용
        base
    }
}

/// Dual Momentum 설정 (UI용).
///
/// 듀얼 모멘텀: 절대 모멘텀(BIL 대비)과 상대 모멘텀을 조합.
/// 카나리아 임계값이 1.0으로 모든 카나리아가 양수여야 공격 모드.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "dual_momentum",
    name = "Dual Momentum",
    description = "듀얼 모멘텀 전략 (절대/상대 모멘텀 조합)",
    category = "Monthly"
)]
pub struct DualMomentumConfig {
    /// 현금 티커
    #[schema(label = "현금 티커", section = "asset", default = "BIL")]
    pub cash_ticker: String,

    /// 투자 비율 (0.0 ~ 1.0)
    #[schema(label = "투자 비율", section = "timing", field_type = "number", min = 0, max = 1, default = "1.0")]
    pub invest_rate: Decimal,

    /// 리밸런싱 임계값 (%)
    #[schema(label = "리밸런싱 임계값 (%)", section = "timing", field_type = "number", min = 1, max = 20, default = "5")]
    pub rebalance_threshold: Decimal,

    /// 최소 Global Score
    #[schema(label = "최소 GlobalScore", section = "filter", field_type = "number", min = 0, max = 100, default = "50")]
    pub min_global_score: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default = "ExitConfig::for_rebalancing")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

impl From<DualMomentumConfig> for AssetAllocationConfig {
    fn from(cfg: DualMomentumConfig) -> Self {
        let mut base = Self::dual_momentum_default();
        base.cash_ticker = cfg.cash_ticker;
        base.invest_rate = cfg.invest_rate;
        base.rebalance_threshold = cfg.rebalance_threshold;
        base.min_global_score = Some(cfg.min_global_score);
        base.exit_config = cfg.exit_config;
        // DualMomentum의 canary_threshold는 1.0 고정
        base
    }
}

impl AssetAllocationConfig {
    /// HAA 기본 설정 (미국 시장).
    pub fn haa_default() -> Self {
        Self {
            variant: StrategyVariant::Haa,
            assets: vec![
                // 카나리아
                AssetDefinition::new("VWO", AssetCategory::Canary).with_description("이머징"),
                AssetDefinition::new("BND", AssetCategory::Canary).with_description("채권"),
                // 공격
                AssetDefinition::new("SPY", AssetCategory::Offensive).with_description("S&P 500"),
                AssetDefinition::new("VEA", AssetCategory::Offensive).with_description("선진국"),
                AssetDefinition::new("VWO", AssetCategory::Offensive).with_description("이머징"),
                AssetDefinition::new("AGG", AssetCategory::Offensive).with_description("미국 채권"),
                // 방어
                AssetDefinition::new("SHY", AssetCategory::Defensive).with_description("단기 채권"),
                AssetDefinition::new("IEF", AssetCategory::Defensive).with_description("중기 채권"),
                AssetDefinition::new("LQD", AssetCategory::Defensive).with_description("회사채"),
                // 현금
                AssetDefinition::new("BIL", AssetCategory::Cash).with_description("현금"),
            ],
            momentum_method: MomentumMethod::SimpleAverage {
                periods_months: vec![1, 3, 6, 12],
            },
            offensive_top_n: 4,
            defensive_top_n: 3,
            cash_ticker: "BIL".to_string(),
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(5.0),
            min_global_score: Some(dec!(55)),
            canary_threshold: dec!(0.5), // 50% 이상 양수 모멘텀
            exit_config: ExitConfig::for_rebalancing(),
        }
    }

    /// XAA 기본 설정 (미국 시장).
    pub fn xaa_default() -> Self {
        Self {
            variant: StrategyVariant::Xaa,
            assets: vec![
                // 카나리아
                AssetDefinition::new("VWO", AssetCategory::Canary),
                AssetDefinition::new("BND", AssetCategory::Canary),
                // 공격
                AssetDefinition::new("SPY", AssetCategory::Offensive),
                AssetDefinition::new("VEA", AssetCategory::Offensive),
                AssetDefinition::new("VWO", AssetCategory::Offensive),
                AssetDefinition::new("BND", AssetCategory::Offensive),
                // 채권
                AssetDefinition::new("LQD", AssetCategory::Bond),
                AssetDefinition::new("HYG", AssetCategory::Bond),
                AssetDefinition::new("EMB", AssetCategory::Bond),
                // 안전
                AssetDefinition::new("SHY", AssetCategory::Safe),
                AssetDefinition::new("IEF", AssetCategory::Safe),
                AssetDefinition::new("TLT", AssetCategory::Safe),
                // 현금
                AssetDefinition::new("BIL", AssetCategory::Cash),
            ],
            momentum_method: MomentumMethod::Extended {
                base_periods_months: vec![1, 3, 6, 12],
                bond_period_months: 6,
            },
            offensive_top_n: 4,
            defensive_top_n: 3,
            cash_ticker: "BIL".to_string(),
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(5.0),
            min_global_score: Some(dec!(55)),
            canary_threshold: dec!(0.5),
            exit_config: ExitConfig::for_rebalancing(),
        }
    }

    /// BAA 기본 설정.
    pub fn baa_default() -> Self {
        Self {
            variant: StrategyVariant::Baa,
            assets: vec![
                // 카나리아
                AssetDefinition::new("SPY", AssetCategory::Canary),
                AssetDefinition::new("VEA", AssetCategory::Canary),
                AssetDefinition::new("VWO", AssetCategory::Canary),
                AssetDefinition::new("AGG", AssetCategory::Canary),
                // 공격
                AssetDefinition::new("QQQ", AssetCategory::Offensive),
                AssetDefinition::new("VEA", AssetCategory::Offensive),
                AssetDefinition::new("VWO", AssetCategory::Offensive),
                AssetDefinition::new("AGG", AssetCategory::Offensive),
                // 방어
                AssetDefinition::new("SHY", AssetCategory::Defensive),
                AssetDefinition::new("IEF", AssetCategory::Defensive),
                AssetDefinition::new("LQD", AssetCategory::Defensive),
                // 현금
                AssetDefinition::new("BIL", AssetCategory::Cash),
            ],
            momentum_method: MomentumMethod::Weighted {
                period_weights: vec![
                    (21 * 12, dec!(0.4)), // 12개월
                    (21 * 3, dec!(0.3)),  // 3개월
                    (21, dec!(0.3)),      // 1개월
                ],
            },
            offensive_top_n: 1,
            defensive_top_n: 3,
            cash_ticker: "BIL".to_string(),
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(5.0),
            min_global_score: Some(dec!(55)),
            canary_threshold: dec!(0.75), // 75% 이상 양수
            exit_config: ExitConfig::for_rebalancing(),
        }
    }

    /// All Weather 기본 설정 (레이 달리오 올웨더).
    pub fn all_weather_default() -> Self {
        Self {
            variant: StrategyVariant::AllWeather,
            assets: vec![
                // 공격 자산 (주식)
                AssetDefinition::new("SPY", AssetCategory::Offensive).with_description("S&P 500"),
                // 채권
                AssetDefinition::new("TLT", AssetCategory::Bond).with_description("장기 국채"),
                AssetDefinition::new("IEF", AssetCategory::Bond).with_description("중기 국채"),
                // 금/원자재
                AssetDefinition::new("GLD", AssetCategory::Defensive).with_description("금"),
                AssetDefinition::new("DBC", AssetCategory::Defensive).with_description("원자재"),
                // 현금
                AssetDefinition::new("BIL", AssetCategory::Cash).with_description("현금"),
            ],
            momentum_method: MomentumMethod::SimpleAverage {
                periods_months: vec![12],
            },
            offensive_top_n: 1, // 주식 1개
            defensive_top_n: 4, // 채권2 + 금 + 원자재
            cash_ticker: "BIL".to_string(),
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(5.0),
            min_global_score: None,      // 정적 배분이므로 스코어 필터 없음
            canary_threshold: dec!(0.0), // 카나리아 없음
            exit_config: ExitConfig::for_rebalancing(),
        }
    }

    /// Dual Momentum 기본 설정 (듀얼 모멘텀).
    pub fn dual_momentum_default() -> Self {
        Self {
            variant: StrategyVariant::DualMomentum,
            assets: vec![
                // 카나리아 (절대 모멘텀 확인용)
                AssetDefinition::new("BIL", AssetCategory::Canary).with_description("현금"),
                // 공격 자산 (주식)
                AssetDefinition::new("SPY", AssetCategory::Offensive).with_description("S&P 500"),
                AssetDefinition::new("VEU", AssetCategory::Offensive)
                    .with_description("미국 제외 선진국"),
                // 방어 자산 (채권)
                AssetDefinition::new("AGG", AssetCategory::Defensive).with_description("미국 채권"),
                // 현금
                AssetDefinition::new("BIL", AssetCategory::Cash).with_description("현금"),
            ],
            momentum_method: MomentumMethod::SimpleAverage {
                periods_months: vec![12],
            },
            offensive_top_n: 1, // 상위 1개 주식
            defensive_top_n: 1, // 채권 1개
            cash_ticker: "BIL".to_string(),
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(5.0),
            min_global_score: Some(dec!(50)),
            canary_threshold: dec!(1.0), // 모든 카나리아 양수여야 공격 모드
            exit_config: ExitConfig::for_rebalancing(),
        }
    }

    /// 모든 티커 목록.
    pub fn all_tickers(&self) -> Vec<String> {
        let mut tickers: Vec<String> = self.assets.iter().map(|a| a.ticker.clone()).collect();
        tickers.sort();
        tickers.dedup();
        tickers
    }

    /// 카테고리별 자산 필터.
    pub fn assets_by_category(&self, category: AssetCategory) -> Vec<&AssetDefinition> {
        self.assets
            .iter()
            .filter(|a| a.category == category)
            .collect()
    }
}

// ================================================================================================
// 전략 구현
// ================================================================================================

/// 통합 자산 배분 전략.
pub struct AssetAllocationStrategy {
    config: Option<AssetAllocationConfig>,
    context: Option<Arc<RwLock<StrategyContext>>>,
    positions: HashMap<String, Decimal>,
    last_rebalance_ym: Option<String>,
    rebalance_calculator: RebalanceCalculator,
    momentum_calculator: MomentumCalculator,
    current_mode: PortfolioMode,
    cash_balance: Decimal,
}

impl AssetAllocationStrategy {
    /// 새 전략 생성.
    pub fn new() -> Self {
        Self {
            config: None,
            context: None,
            positions: HashMap::new(),
            last_rebalance_ym: None,
            rebalance_calculator: RebalanceCalculator::new(RebalanceConfig::us_market()),
            momentum_calculator: MomentumCalculator::standard(),
            current_mode: PortfolioMode::Defensive,
            cash_balance: Decimal::ZERO,
        }
    }

    /// 설정으로 초기화된 전략 생성.
    pub fn with_config(config: AssetAllocationConfig) -> Self {
        let mut strategy = Self::new();
        strategy.init_momentum_calculator(&config);
        strategy.config = Some(config);
        strategy
    }

    /// HAA 전략 팩토리.
    pub fn haa() -> Self {
        Self::with_config(AssetAllocationConfig::haa_default())
    }

    /// XAA 전략 팩토리.
    pub fn xaa() -> Self {
        Self::with_config(AssetAllocationConfig::xaa_default())
    }

    /// BAA 전략 팩토리.
    pub fn baa() -> Self {
        Self::with_config(AssetAllocationConfig::baa_default())
    }

    /// All Weather 전략 팩토리.
    pub fn all_weather() -> Self {
        Self::with_config(AssetAllocationConfig::all_weather_default())
    }

    /// Dual Momentum 전략 팩토리.
    pub fn dual_momentum() -> Self {
        Self::with_config(AssetAllocationConfig::dual_momentum_default())
    }

    /// 설정으로 모멘텀 계산기 초기화.
    fn init_momentum_calculator(&mut self, config: &AssetAllocationConfig) {
        let momentum_config = match &config.momentum_method {
            MomentumMethod::SimpleAverage { periods_months } => MomentumConfig {
                lookback_periods: periods_months.iter().map(|m| m * 21).collect(),
                equal_weights: true,
                min_data_points: None,
            },
            MomentumMethod::Weighted { period_weights } => MomentumConfig {
                lookback_periods: period_weights.iter().map(|(p, _)| *p).collect(),
                equal_weights: false,
                min_data_points: None,
            },
            MomentumMethod::Extended {
                base_periods_months,
                ..
            } => MomentumConfig {
                lookback_periods: base_periods_months.iter().map(|m| m * 21).collect(),
                equal_weights: true,
                min_data_points: None,
            },
        };
        self.momentum_calculator = MomentumCalculator::new(momentum_config);
    }

    /// StrategyContext에서 가격 히스토리 가져오기.
    fn get_price_history(&self, ticker: &str) -> Option<Vec<Decimal>> {
        let ctx = self.context.as_ref()?;
        let ctx_lock = ctx.try_read().ok()?;
        let klines = ctx_lock.get_klines(ticker, Timeframe::D1);

        if klines.is_empty() {
            return None;
        }

        // klines를 가격 벡터로 변환 (최신이 앞에)
        let prices: Vec<Decimal> = klines.iter().rev().map(|k| k.close).collect();
        Some(prices)
    }

    /// 자산별 모멘텀 계산 (StrategyContext에서 가격 가져옴).
    fn calculate_asset_momentum(&self, ticker: &str) -> Option<MomentumResult> {
        let prices = self.get_price_history(ticker)?;
        if prices.len() < 21 {
            return None;
        }
        Some(self.momentum_calculator.calculate(&prices))
    }

    /// 카나리아 자산 확인.
    fn check_canary_assets(&self, config: &AssetAllocationConfig) -> PortfolioMode {
        let canary_assets = config.assets_by_category(AssetCategory::Canary);
        if canary_assets.is_empty() {
            return PortfolioMode::Offensive; // 카나리아 없으면 공격 모드
        }

        let mut positive_count = 0;
        let mut total_count = 0;

        for asset in canary_assets {
            if let Some(result) = self.calculate_asset_momentum(&asset.ticker) {
                total_count += 1;
                if result.is_valid && result.score > Decimal::ZERO {
                    positive_count += 1;
                }
            }
        }

        if total_count == 0 {
            return PortfolioMode::Defensive;
        }

        let positive_ratio = Decimal::from(positive_count) / Decimal::from(total_count);
        if positive_ratio >= config.canary_threshold {
            info!(
                "[AssetAllocation] 카나리아 양수 비율 {:.2}% >= 임계값 {:.2}% → 공격 모드",
                positive_ratio * dec!(100),
                config.canary_threshold * dec!(100)
            );
            PortfolioMode::Offensive
        } else {
            info!(
                "[AssetAllocation] 카나리아 양수 비율 {:.2}% < 임계값 {:.2}% → 방어 모드",
                positive_ratio * dec!(100),
                config.canary_threshold * dec!(100)
            );
            PortfolioMode::Defensive
        }
    }

    /// StrategyContext를 통해 진입 가능 여부 확인.
    /// 자산배분 전략은 자체 모멘텀 계산을 하므로 GlobalScore 체크 불필요.
    /// 미국 ETF를 사용하므로 GlobalScore(한국 주식용)가 제공되지 않음.
    fn can_enter(&self, ticker: &str, _config: &AssetAllocationConfig) -> bool {
        let ctx = match self.context.as_ref() {
            Some(c) => c,
            None => return true, // 컨텍스트 없으면 기본 허용
        };

        let ctx_lock = match ctx.try_read() {
            Ok(lock) => lock,
            Err(_) => return true, // 락 실패 시 기본 허용
        };

        // RouteState 확인 - Overheat 시만 진입 제한
        if let Some(state) = ctx_lock.get_route_state(ticker) {
            if state == &RouteState::Overheat {
                debug!(
                    ticker = ticker,
                    route_state = ?state,
                    "시장 과열 - 진입 제한"
                );
                return false;
            }
        }

        true
    }

    /// 리밸런싱 필요 여부 확인.
    fn should_rebalance(&self, current_time: DateTime<Utc>) -> bool {
        let current_ym = format!("{}_{}", current_time.year(), current_time.month());

        match &self.last_rebalance_ym {
            None => true,
            Some(last_ym) => last_ym != &current_ym,
        }
    }

    /// 목표 비중 계산.
    fn calculate_target_weights(&self, config: &AssetAllocationConfig) -> Vec<TargetAllocation> {
        let mut allocations: Vec<TargetAllocation> = Vec::new();

        let assets_for_mode = match self.current_mode {
            PortfolioMode::Offensive => config.assets_by_category(AssetCategory::Offensive),
            PortfolioMode::Defensive => config.assets_by_category(AssetCategory::Defensive),
        };

        // 모멘텀 순위 계산
        let mut ranked: Vec<(String, Decimal)> = assets_for_mode
            .iter()
            .filter_map(|a| {
                self.calculate_asset_momentum(&a.ticker)
                    .filter(|r| r.is_valid)
                    .map(|r| (a.ticker.clone(), r.score))
            })
            .collect();

        ranked.sort_by(|a, b| b.1.cmp(&a.1));

        let top_n = match self.current_mode {
            PortfolioMode::Offensive => config.offensive_top_n.min(ranked.len()),
            PortfolioMode::Defensive => config.defensive_top_n.min(ranked.len()),
        };

        // 균등 비중 계산
        let base_weight = if top_n > 0 {
            config.invest_rate / Decimal::from(top_n)
        } else {
            Decimal::ZERO
        };

        let mut allocated_weight = Decimal::ZERO;

        // 상위 N개 자산 처리
        for (ticker, momentum) in ranked.iter().take(top_n) {
            if *momentum > Decimal::ZERO && self.can_enter(ticker, config) {
                allocations.push(TargetAllocation::new(ticker.clone(), base_weight));
                allocated_weight += base_weight;
                info!(
                    "[AssetAllocation] {:?} 자산: {} (모멘텀: {:.4}, 비중: {:.1}%)",
                    self.current_mode,
                    ticker,
                    momentum,
                    base_weight * dec!(100)
                );
            } else {
                info!(
                    "[AssetAllocation] {:?} 자산: {} 스킵 (모멘텀: {:.4})",
                    self.current_mode, ticker, momentum
                );
            }
        }

        // 잔여 비중은 현금으로
        let cash_weight = config.invest_rate - allocated_weight;
        if cash_weight > Decimal::ZERO {
            allocations.push(TargetAllocation::new(
                config.cash_ticker.clone(),
                cash_weight,
            ));
            info!(
                "[AssetAllocation] 현금: {} (비중: {:.1}%)",
                config.cash_ticker,
                cash_weight * dec!(100)
            );
        }

        allocations
    }

    /// 리밸런싱 신호 생성.
    fn generate_rebalance_signals(
        &mut self,
        config: &AssetAllocationConfig,
        current_time: DateTime<Utc>,
    ) -> Vec<Signal> {
        if !self.should_rebalance(current_time) {
            return Vec::new();
        }

        // 모드 결정
        self.current_mode = self.check_canary_assets(config);

        // 목표 비중 계산
        let target_allocations = self.calculate_target_weights(config);
        debug!(targets = ?target_allocations, "목표 비중 계산 완료");

        // 현재 포지션을 PortfolioPosition으로 변환
        let current_positions: Vec<PortfolioPosition> = self
            .positions
            .iter()
            .filter_map(|(ticker, &qty)| {
                let prices = self.get_price_history(ticker)?;
                let price = prices.first()?;
                Some(PortfolioPosition::new(ticker.clone(), qty, *price))
            })
            .collect();

        // 현금 포지션 추가
        let mut all_positions = current_positions;
        all_positions.push(PortfolioPosition::cash(self.cash_balance, "USD"));

        // 리밸런싱 주문 계산
        let result = self
            .rebalance_calculator
            .calculate_orders(&all_positions, &target_allocations);

        if !result.rebalance_needed {
            debug!(
                "리밸런싱 불필요: 최대 편차 {:.2}%",
                result.max_weight_deviation * dec!(100)
            );
            return Vec::new();
        }

        // 신호 생성
        let variant_name = match config.variant {
            StrategyVariant::Haa => "asset_allocation_haa",
            StrategyVariant::Xaa => "asset_allocation_xaa",
            StrategyVariant::Baa => "asset_allocation_baa",
            StrategyVariant::AllWeather => "asset_allocation_all_weather",
            StrategyVariant::DualMomentum => "asset_allocation_dual_momentum",
            StrategyVariant::Custom => "asset_allocation_custom",
        };

        let signals: Vec<Signal> = result
            .orders
            .iter()
            .map(|order| {
                let side = match order.side {
                    super::common::rebalance::RebalanceOrderSide::Buy => Side::Buy,
                    super::common::rebalance::RebalanceOrderSide::Sell => Side::Sell,
                };

                let current_price = self
                    .get_price_history(&order.ticker)
                    .and_then(|p: Vec<Decimal>| p.first().copied())
                    .unwrap_or(Decimal::ZERO);

                let signal_type = if side == Side::Buy {
                    SignalType::Entry
                } else {
                    SignalType::Exit
                };

                Signal::new(variant_name, order.ticker.clone(), side, signal_type)
                    .with_strength(0.8)
                    .with_prices(Some(current_price), None, None)
                    .with_metadata("mode", json!(format!("{:?}", self.current_mode)))
                    .with_metadata("current_weight", json!(order.current_weight.to_string()))
                    .with_metadata("target_weight", json!(order.target_weight.to_string()))
                    .with_metadata("quantity", json!(order.quantity.to_string()))
            })
            .collect();

        // 리밸런싱 시간 기록
        self.last_rebalance_ym = Some(format!("{}_{}", current_time.year(), current_time.month()));
        info!(
            "[AssetAllocation] 리밸런싱 완료: {} 신호 생성",
            signals.len()
        );

        signals
    }
}

impl Default for AssetAllocationStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for AssetAllocationStrategy {
    fn name(&self) -> &str {
        match self.config.as_ref().map(|c| c.variant) {
            Some(StrategyVariant::Haa) => "AssetAllocation-HAA",
            Some(StrategyVariant::Xaa) => "AssetAllocation-XAA",
            Some(StrategyVariant::Baa) => "AssetAllocation-BAA",
            Some(StrategyVariant::AllWeather) => "AssetAllocation-AllWeather",
            Some(StrategyVariant::DualMomentum) => "AssetAllocation-DualMomentum",
            Some(StrategyVariant::Custom) => "AssetAllocation-Custom",
            None => "AssetAllocation",
        }
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "통합 자산배분 전략. HAA/XAA/BAA 스타일 지원, 카나리아 기반 공격/방어 전환, 월간 리밸런싱."
    }

    async fn initialize(
        &mut self,
        config_value: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 1. config_value에서 variant 확인 (테스트 등에서 직접 JSON 전달 시)
        // 2. self.config에서 variant 확인 (팩토리 메서드 사용 시)
        // 3. 기본값: Haa
        let variant = config_value
            .get("variant")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "Haa" => Some(StrategyVariant::Haa),
                "Xaa" => Some(StrategyVariant::Xaa),
                "Baa" => Some(StrategyVariant::Baa),
                "AllWeather" => Some(StrategyVariant::AllWeather),
                "DualMomentum" => Some(StrategyVariant::DualMomentum),
                "Custom" => Some(StrategyVariant::Custom),
                _ => None,
            })
            .or_else(|| self.config.as_ref().map(|c| c.variant))
            .unwrap_or(StrategyVariant::Haa);

        // variant에 따라 적절한 Config 타입으로 파싱
        let config: AssetAllocationConfig = match variant {
            StrategyVariant::Haa => {
                let cfg: HaaConfig = serde_json::from_value(config_value.clone())?;
                cfg.into()
            }
            StrategyVariant::Xaa => {
                let cfg: XaaConfig = serde_json::from_value(config_value.clone())?;
                cfg.into()
            }
            StrategyVariant::Baa => {
                let cfg: BaaConfig = serde_json::from_value(config_value.clone())?;
                cfg.into()
            }
            StrategyVariant::AllWeather => {
                let cfg: AllWeatherConfig = serde_json::from_value(config_value.clone())?;
                cfg.into()
            }
            StrategyVariant::DualMomentum => {
                let cfg: DualMomentumConfig = serde_json::from_value(config_value.clone())?;
                cfg.into()
            }
            StrategyVariant::Custom => {
                // Custom은 직접 AssetAllocationConfig 파싱
                serde_json::from_value(config_value.clone())?
            }
        };

        self.init_momentum_calculator(&config);

        // initial_capital 또는 amount가 있으면 cash_balance로 설정
        let capital_value = config_value.get("initial_capital").or_else(|| config_value.get("amount"));
        if let Some(capital_v) = capital_value {
            let capital_opt = capital_v.as_str()
                .and_then(|s| s.parse::<Decimal>().ok())
                .or_else(|| capital_v.as_i64().map(Decimal::from));

            if let Some(capital_dec) = capital_opt {
                self.cash_balance = capital_dec;
                info!("[AssetAllocation] 초기 자본금 설정: {}", capital_dec);
            }
        }

        info!(
            "[AssetAllocation] 초기화 - 변형: {:?}, 공격자산: {}개, 방어자산: {}개",
            config.variant,
            config.assets_by_category(AssetCategory::Offensive).len(),
            config.assets_by_category(AssetCategory::Defensive).len()
        );

        self.config = Some(config);
        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        let config = match &self.config {
            Some(c) => c.clone(),
            None => return Ok(Vec::new()),
        };

        let ticker = data.ticker.clone();

        // 관심 자산이 아니면 무시
        if !config.all_tickers().contains(&ticker) {
            return Ok(Vec::new());
        }

        // 가격 추출
        let price = match &data.data {
            MarketDataType::Kline(kline) => Some(kline.close),
            MarketDataType::Ticker(ticker_data) => Some(ticker_data.last),
            MarketDataType::Trade(trade) => Some(trade.price),
            MarketDataType::OrderBook(_) => None,
        };

        // 리밸런싱 신호 생성 (가격은 StrategyContext에서 가져옴)
        // price 변수가 있으면 데이터 유효성 확인용으로만 사용
        if price.is_none() {
            return Ok(vec![]);
        }

        let signals = self.generate_rebalance_signals(&config, data.timestamp);

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "[AssetAllocation] 주문 체결: {:?} {} {} @ {:?}",
            order.side, order.quantity, order.ticker, order.average_fill_price
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ticker = position.ticker.clone();
        self.positions.insert(ticker.clone(), position.quantity);
        info!(
            "[AssetAllocation] 포지션 업데이트: {} = {} (PnL: {})",
            ticker, position.quantity, position.unrealized_pnl
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("[AssetAllocation] 전략 종료");
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "name": self.name(),
            "version": self.version(),
            "variant": self.config.as_ref().map(|c| format!("{:?}", c.variant)),
            "current_mode": format!("{:?}", self.current_mode),
            "last_rebalance_ym": self.last_rebalance_ym,
            "positions": self.positions.iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect::<HashMap<_, _>>(),
            "cash_balance": self.cash_balance.to_string(),
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("[AssetAllocation] StrategyContext 주입 완료");
    }

    fn exit_config(&self) -> Option<&ExitConfig> {
        self.config.as_ref().map(|c| &c.exit_config)
    }
}

// ================================================================================================
// 전략 레지스트리 등록
// ================================================================================================

use crate::register_strategy;

// HAA 자산배분 전략
register_strategy! {
    id: "haa",
    aliases: ["hierarchical_asset_allocation"],
    name: "HAA 자산배분",
    description: "계층적 자산 배분 전략 (카나리아 기반 공격/방어 모드 전환)",
    timeframe: "1M",
    tickers: ["SPY", "VEA", "VWO", "AGG", "SHY", "IEF", "LQD", "BIL"],
    category: Monthly,
    markets: [Stock],
    factory: AssetAllocationStrategy::haa,
    config: HaaConfig
}

// XAA 자산배분 전략
register_strategy! {
    id: "xaa",
    aliases: ["extended_asset_allocation"],
    name: "XAA 자산배분",
    description: "확장 자산 배분 전략 (채권 최적화 포함)",
    timeframe: "1M",
    tickers: ["SPY", "VEA", "VWO", "BND", "LQD", "HYG", "EMB", "SHY", "IEF", "TLT", "BIL"],
    category: Monthly,
    markets: [Stock],
    factory: AssetAllocationStrategy::xaa,
    config: XaaConfig
}

// BAA 자산배분 전략
register_strategy! {
    id: "baa",
    aliases: ["balanced_asset_allocation"],
    name: "BAA 자산배분",
    description: "균형 자산 배분 전략 (가중 모멘텀 기반)",
    timeframe: "1M",
    tickers: ["SPY", "VEA", "VWO", "AGG", "QQQ", "SHY", "IEF", "LQD", "BIL"],
    category: Monthly,
    markets: [Stock],
    factory: AssetAllocationStrategy::baa,
    config: BaaConfig
}

// All Weather 전략
register_strategy! {
    id: "all_weather",
    aliases: ["allweather", "ray_dalio"],
    name: "All Weather",
    description: "레이 달리오 올웨더 포트폴리오 (정적 자산 배분)",
    timeframe: "1M",
    tickers: ["SPY", "TLT", "IEF", "GLD", "DBC", "BIL"],
    category: Monthly,
    markets: [Stock],
    factory: AssetAllocationStrategy::all_weather,
    config: AllWeatherConfig
}

// Dual Momentum 전략
register_strategy! {
    id: "dual_momentum",
    aliases: ["dualmomentum", "gaa"],
    name: "Dual Momentum",
    description: "듀얼 모멘텀 전략 (절대/상대 모멘텀 조합)",
    timeframe: "1M",
    tickers: ["SPY", "VEU", "AGG", "BIL"],
    category: Monthly,
    markets: [Stock],
    factory: AssetAllocationStrategy::dual_momentum,
    config: DualMomentumConfig
}

// 통합 테스트는 tests/asset_allocation_test.rs에서 수행
