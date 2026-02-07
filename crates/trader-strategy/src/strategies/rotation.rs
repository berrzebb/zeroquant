//! 로테이션 전략 (Rotation Strategy)
//!
//! 섹터 모멘텀, 종목 로테이션, 시가총액 상위 전략을 통합한 그룹 전략입니다.
//!
//! # 지원 변형
//!
//! - **SectorMomentum**: 섹터 ETF 모멘텀 순위 기반 투자
//! - **StockMomentum**: 개별 종목 모멘텀 순위 기반 투자
//! - **MarketCapTop**: 시가총액 상위 종목 투자 (선택적 모멘텀 필터)
//!
//! # 공통 로직
//!
//! 1. 유니버스 내 자산들의 순위 계산 (모멘텀/시총 등)
//! 2. 상위 N개 자산 선택
//! 3. 비중 배분 (균등/모멘텀비례/역변동성)
//! 4. 정기 리밸런싱 (월간/일간)
//!
//! # 예시
//!
//! ```rust,ignore
//! // 섹터 모멘텀 전략
//! let config = RotationConfig::sector_momentum_default();
//! let strategy = RotationStrategy::new(config);
//!
//! // 종목 로테이션 전략
//! let config = RotationConfig::stock_rotation_default();
//! let strategy = RotationStrategy::new(config);
//!
//! // 시가총액 상위 전략
//! let config = RotationConfig::market_cap_top_default();
//! let strategy = RotationStrategy::new(config);
//! ```

use crate::strategies::common::rebalance::{
    PortfolioPosition, RebalanceCalculator, RebalanceConfig, RebalanceOrderSide, TargetAllocation,
};
use crate::strategies::common::{adjust_strength_by_score, ExitConfig};
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use trader_strategy_macro::StrategyConfig;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use trader_core::domain::{RouteState, StrategyContext};
use trader_core::types::Timeframe;
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, SignalType};

// ============================================================================
// 전략 변형 (Strategy Variant)
// ============================================================================

/// 로테이션 전략 변형.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[derive(Default)]
pub enum RotationVariant {
    /// 섹터 모멘텀 - 섹터 ETF 모멘텀 순위 기반
    #[default]
    SectorMomentum,
    /// 종목 모멘텀 - 개별 종목 모멘텀 순위 기반
    StockMomentum,
    /// 시가총액 상위 - 시총 순위 기반 (선택적 모멘텀 필터)
    MarketCapTop,
}


// ============================================================================
// 시장 타입 (Market Type)
// ============================================================================

/// 시장 타입.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MarketType {
    /// 미국 시장
    #[default]
    US,
    /// 한국 시장
    KR,
}

impl MarketType {
    /// Quote 통화 반환.
    pub fn quote_currency(&self) -> &str {
        match self {
            MarketType::US => "USD",
            MarketType::KR => "KRW",
        }
    }

    /// RebalanceConfig 반환.
    pub fn rebalance_config(&self) -> RebalanceConfig {
        match self {
            MarketType::US => RebalanceConfig::us_market(),
            MarketType::KR => RebalanceConfig::korean_market(),
        }
    }
}

// ============================================================================
// 랭킹 메트릭 (Ranking Metric)
// ============================================================================

/// 순위 결정 방식.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RankingMetric {
    /// 다중 기간 모멘텀 (섹터 모멘텀용)
    /// - short/medium/long 기간과 가중치 지정
    MultiPeriodMomentum {
        /// 단기 기간 (일)
        short_period: usize,
        /// 중기 기간 (일)
        medium_period: usize,
        /// 장기 기간 (일)
        long_period: usize,
        /// 단기 가중치
        short_weight: f64,
        /// 중기 가중치
        medium_weight: f64,
        /// 장기 가중치
        long_weight: f64,
    },
    /// 평균 기간 모멘텀 (종목 로테이션용)
    /// - 1M, 3M, 6M, 12M 평균
    AverageMomentum {
        /// 사용할 기간들 (일 단위)
        periods: Vec<usize>,
    },
    /// 단일 기간 모멘텀 (시가총액용)
    /// - 단순 ROC
    SinglePeriodMomentum {
        /// 모멘텀 계산 기간 (일)
        period: usize,
    },
    /// 순위 없음 (정적 유니버스)
    None,
}

impl Default for RankingMetric {
    fn default() -> Self {
        Self::MultiPeriodMomentum {
            short_period: 20,
            medium_period: 60,
            long_period: 120,
            short_weight: 0.5,
            medium_weight: 0.3,
            long_weight: 0.2,
        }
    }
}

// ============================================================================
// 비중 배분 방식 (Weighting Method)
// ============================================================================

/// 비중 배분 방식.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WeightingMethod {
    /// 동일 비중
    #[default]
    Equal,
    /// 모멘텀 비례 비중
    MomentumProportional,
    /// 역변동성 비중
    InverseVolatility,
}

// ============================================================================
// 리밸런싱 빈도 (Rebalance Frequency)
// ============================================================================

/// 리밸런싱 빈도.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub enum RebalanceFrequency {
    /// 월간 (매월 초)
    #[default]
    Monthly,
    /// 일수 기반
    Days(u32),
}


// ============================================================================
// 자산 정보 (Asset Info)
// ============================================================================

/// 유니버스 내 자산 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetInfo {
    /// 티커 (기본 심볼, e.g., "XLK", "AAPL")
    pub ticker: String,
    /// 자산명
    pub name: String,
    /// 섹터 (선택)
    pub sector: Option<String>,
}

impl AssetInfo {
    /// 새 자산 정보 생성.
    pub fn new(ticker: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            ticker: ticker.into(),
            name: name.into(),
            sector: None,
        }
    }

    /// 섹터와 함께 생성.
    pub fn with_sector(mut self, sector: impl Into<String>) -> Self {
        self.sector = Some(sector.into());
        self
    }
}

// ============================================================================
// 로테이션 설정 (Rotation Config)
// ============================================================================

/// 로테이션 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "rotation",
    name = "로테이션 전략",
    description = "섹터/종목/시총 기반 로테이션 투자 전략",
    category = "Monthly"
)]
pub struct RotationConfig {
    /// 전략 변형
    #[serde(default)]
    #[schema(label = "전략 변형")]
    pub variant: RotationVariant,

    /// 시장 타입
    #[serde(default)]
    #[schema(label = "시장 타입")]
    pub market: MarketType,

    /// 유니버스 (자산 목록) - 빈 값이면 기본 유니버스 사용
    #[serde(default)]
    #[schema(label = "투자 유니버스", skip)]
    pub universe: Vec<AssetInfo>,

    /// 상위 N개 선택
    #[serde(default = "default_top_n")]
    #[schema(label = "상위 종목 수", min = 1, max = 20)]
    pub top_n: usize,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    #[schema(label = "투자 금액", min = 100000, max = 1000000000)]
    pub total_amount: Decimal,

    /// 순위 결정 방식
    #[serde(default)]
    #[schema(label = "순위 결정 방식", skip)]
    pub ranking_metric: RankingMetric,

    /// 비중 배분 방식
    #[serde(default)]
    #[schema(label = "비중 배분 방식")]
    pub weighting_method: WeightingMethod,

    /// 리밸런싱 빈도
    #[serde(default)]
    #[schema(label = "리밸런싱 빈도", skip)]
    pub rebalance_frequency: RebalanceFrequency,

    /// 리밸런싱 허용 오차 (%)
    #[serde(default = "default_rebalance_threshold")]
    #[schema(label = "리밸런싱 허용 오차 (%)", min = 1, max = 20)]
    pub rebalance_threshold: Decimal,

    /// 최소 모멘텀 (이 이하면 투자 안 함)
    #[serde(default)]
    #[schema(label = "최소 모멘텀")]
    pub min_momentum: Option<Decimal>,

    /// 현금 보유 비율 (0.0 ~ 1.0)
    #[serde(default)]
    #[schema(label = "현금 보유 비율", min = 0, max = 1)]
    pub cash_reserve_rate: Decimal,

    /// 모멘텀 필터 사용 여부 (MarketCapTop용)
    #[serde(default)]
    #[schema(label = "모멘텀 필터 사용")]
    pub use_momentum_filter: bool,

    /// 최소 GlobalScore
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", min = 0, max = 100)]
    pub min_global_score: Decimal,
}

fn default_top_n() -> usize {
    5
}

fn default_total_amount() -> Decimal {
    dec!(10000000)
}

fn default_rebalance_threshold() -> Decimal {
    dec!(5)
}

fn default_min_global_score() -> Decimal {
    dec!(60)
}

fn default_min_momentum() -> Decimal {
    dec!(0)
}

fn default_cash_reserve_rate() -> Decimal {
    dec!(0)
}

fn default_sector_top_n() -> usize {
    3
}

fn default_kr_sector_top_n() -> usize {
    2
}

fn default_market_cap_top_n() -> usize {
    10
}

fn default_false() -> bool {
    false
}

impl Default for RotationConfig {
    fn default() -> Self {
        Self::sector_momentum_default()
    }
}

// ================================================================================================
// 전략별 UI Config (SDUI용)
// ================================================================================================

/// 섹터 모멘텀 (미국) 설정 (UI용).
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "sector_momentum",
    name = "섹터 모멘텀",
    description = "미국 섹터 ETF 모멘텀 순위 기반 투자 전략",
    category = "Monthly"
)]
pub struct SectorMomentumConfig {
    /// 상위 N개 선택
    #[serde(default = "default_sector_top_n")]
    #[schema(label = "상위 섹터 수", field_type = "integer", min = 1, max = 11, default = "3", section = "filter")]
    pub top_n: usize,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    #[schema(label = "투자 금액", field_type = "number", min = 100000, max = 1000000000, default = "10000000", section = "asset")]
    pub total_amount: Decimal,

    /// 리밸런싱 허용 오차 (%)
    #[serde(default = "default_rebalance_threshold")]
    #[schema(label = "리밸런싱 허용 오차 (%)", field_type = "number", min = 1, max = 20, default = "5", section = "filter")]
    pub rebalance_threshold: Decimal,

    /// 최소 모멘텀 (이 이하면 투자 안 함)
    #[serde(default = "default_min_momentum")]
    #[schema(label = "최소 모멘텀", field_type = "number", min = -100, max = 100, default = "0", section = "indicator")]
    pub min_momentum: Decimal,

    /// 현금 보유 비율 (0.0 ~ 1.0)
    #[serde(default = "default_cash_reserve_rate")]
    #[schema(label = "현금 보유 비율", field_type = "number", min = 0, max = 1, default = "0", section = "filter")]
    pub cash_reserve_rate: Decimal,

    /// 최소 GlobalScore
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", field_type = "number", min = 0, max = 100, default = "60", section = "indicator")]
    pub min_global_score: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default = "ExitConfig::for_rebalancing")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

impl From<SectorMomentumConfig> for RotationConfig {
    fn from(cfg: SectorMomentumConfig) -> Self {
        let mut base = Self::sector_momentum_default();
        base.top_n = cfg.top_n;
        base.total_amount = cfg.total_amount;
        base.rebalance_threshold = cfg.rebalance_threshold;
        base.min_momentum = Some(cfg.min_momentum);
        base.cash_reserve_rate = cfg.cash_reserve_rate;
        base.min_global_score = cfg.min_global_score;
        base
    }
}

/// 섹터 모멘텀 (한국) 설정 (UI용).
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "sector_momentum_kr",
    name = "섹터 모멘텀 (KR)",
    description = "한국 섹터 ETF 모멘텀 순위 기반 투자 전략",
    category = "Monthly"
)]
pub struct SectorMomentumKrConfig {
    /// 상위 N개 선택
    #[serde(default = "default_kr_sector_top_n")]
    #[schema(label = "상위 섹터 수", field_type = "integer", min = 1, max = 10, default = "2", section = "filter")]
    pub top_n: usize,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    #[schema(label = "투자 금액", field_type = "number", min = 100000, max = 1000000000, default = "10000000", section = "asset")]
    pub total_amount: Decimal,

    /// 리밸런싱 허용 오차 (%)
    #[serde(default = "default_rebalance_threshold")]
    #[schema(label = "리밸런싱 허용 오차 (%)", field_type = "number", min = 1, max = 20, default = "5", section = "filter")]
    pub rebalance_threshold: Decimal,

    /// 최소 모멘텀
    #[serde(default = "default_min_momentum")]
    #[schema(label = "최소 모멘텀", field_type = "number", min = -100, max = 100, default = "0", section = "indicator")]
    pub min_momentum: Decimal,

    /// 현금 보유 비율
    #[serde(default = "default_cash_reserve_rate")]
    #[schema(label = "현금 보유 비율", field_type = "number", min = 0, max = 1, default = "0", section = "filter")]
    pub cash_reserve_rate: Decimal,

    /// 최소 GlobalScore
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", field_type = "number", min = 0, max = 100, default = "60", section = "indicator")]
    pub min_global_score: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default = "ExitConfig::for_rebalancing")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

impl From<SectorMomentumKrConfig> for RotationConfig {
    fn from(cfg: SectorMomentumKrConfig) -> Self {
        let mut base = Self::sector_momentum_kr();
        base.top_n = cfg.top_n;
        base.total_amount = cfg.total_amount;
        base.rebalance_threshold = cfg.rebalance_threshold;
        base.min_momentum = Some(cfg.min_momentum);
        base.cash_reserve_rate = cfg.cash_reserve_rate;
        base.min_global_score = cfg.min_global_score;
        base
    }
}

/// 종목 로테이션 (미국) 설정 (UI용).
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "stock_rotation",
    name = "종목 로테이션",
    description = "미국 개별 종목 모멘텀 순위 기반 투자 전략",
    category = "Monthly"
)]
pub struct StockRotationConfig {
    /// 상위 N개 선택
    #[serde(default = "default_top_n")]
    #[schema(label = "상위 종목 수", field_type = "integer", min = 1, max = 20, default = "5", section = "filter")]
    pub top_n: usize,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    #[schema(label = "투자 금액", field_type = "number", min = 100000, max = 1000000000, default = "10000000", section = "asset")]
    pub total_amount: Decimal,

    /// 리밸런싱 허용 오차 (%)
    #[serde(default = "default_rebalance_threshold")]
    #[schema(label = "리밸런싱 허용 오차 (%)", field_type = "number", min = 1, max = 20, default = "5", section = "filter")]
    pub rebalance_threshold: Decimal,

    /// 최소 모멘텀
    #[serde(default = "default_min_momentum")]
    #[schema(label = "최소 모멘텀", field_type = "number", min = -100, max = 100, default = "0", section = "indicator")]
    pub min_momentum: Decimal,

    /// 현금 보유 비율
    #[serde(default = "default_cash_reserve_rate")]
    #[schema(label = "현금 보유 비율", field_type = "number", min = 0, max = 1, default = "0", section = "filter")]
    pub cash_reserve_rate: Decimal,

    /// 최소 GlobalScore
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", field_type = "number", min = 0, max = 100, default = "60", section = "indicator")]
    pub min_global_score: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default = "ExitConfig::for_rebalancing")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

impl From<StockRotationConfig> for RotationConfig {
    fn from(cfg: StockRotationConfig) -> Self {
        let mut base = Self::stock_rotation_default();
        base.top_n = cfg.top_n;
        base.total_amount = cfg.total_amount;
        base.rebalance_threshold = cfg.rebalance_threshold;
        base.min_momentum = Some(cfg.min_momentum);
        base.cash_reserve_rate = cfg.cash_reserve_rate;
        base.min_global_score = cfg.min_global_score;
        base
    }
}

/// 종목 로테이션 (한국) 설정 (UI용).
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "stock_rotation_kr",
    name = "종목 로테이션 (KR)",
    description = "한국 대형주 모멘텀 순위 기반 투자 전략",
    category = "Monthly"
)]
pub struct StockRotationKrConfig {
    /// 상위 N개 선택
    #[serde(default = "default_top_n")]
    #[schema(label = "상위 종목 수", field_type = "integer", min = 1, max = 20, default = "5", section = "filter")]
    pub top_n: usize,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    #[schema(label = "투자 금액", field_type = "number", min = 100000, max = 1000000000, default = "10000000", section = "asset")]
    pub total_amount: Decimal,

    /// 리밸런싱 허용 오차 (%)
    #[serde(default = "default_rebalance_threshold")]
    #[schema(label = "리밸런싱 허용 오차 (%)", field_type = "number", min = 1, max = 20, default = "5", section = "filter")]
    pub rebalance_threshold: Decimal,

    /// 최소 모멘텀
    #[serde(default = "default_min_momentum")]
    #[schema(label = "최소 모멘텀", field_type = "number", min = -100, max = 100, default = "0", section = "indicator")]
    pub min_momentum: Decimal,

    /// 현금 보유 비율
    #[serde(default = "default_cash_reserve_rate")]
    #[schema(label = "현금 보유 비율", field_type = "number", min = 0, max = 1, default = "0", section = "filter")]
    pub cash_reserve_rate: Decimal,

    /// 최소 GlobalScore
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", field_type = "number", min = 0, max = 100, default = "60", section = "indicator")]
    pub min_global_score: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default = "ExitConfig::for_rebalancing")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

impl From<StockRotationKrConfig> for RotationConfig {
    fn from(cfg: StockRotationKrConfig) -> Self {
        let mut base = Self::stock_rotation_kr();
        base.top_n = cfg.top_n;
        base.total_amount = cfg.total_amount;
        base.rebalance_threshold = cfg.rebalance_threshold;
        base.min_momentum = Some(cfg.min_momentum);
        base.cash_reserve_rate = cfg.cash_reserve_rate;
        base.min_global_score = cfg.min_global_score;
        base
    }
}

/// 시총 상위 전략 설정 (UI용).
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "market_cap_top",
    name = "시총 상위",
    description = "미국 시총 상위 종목 균등/시총 비중 투자",
    category = "Monthly"
)]
pub struct MarketCapTopConfig {
    /// 상위 N개 선택
    #[serde(default = "default_market_cap_top_n")]
    #[schema(label = "상위 종목 수", field_type = "integer", min = 1, max = 30, default = "10", section = "filter")]
    pub top_n: usize,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    #[schema(label = "투자 금액", field_type = "number", min = 100000, max = 1000000000, default = "10000000", section = "asset")]
    pub total_amount: Decimal,

    /// 리밸런싱 허용 오차 (%)
    #[serde(default = "default_rebalance_threshold")]
    #[schema(label = "리밸런싱 허용 오차 (%)", field_type = "number", min = 1, max = 20, default = "5", section = "filter")]
    pub rebalance_threshold: Decimal,

    /// 모멘텀 필터 사용 여부
    #[serde(default = "default_false")]
    #[schema(label = "모멘텀 필터 사용", field_type = "boolean", default = "false", section = "filter")]
    pub use_momentum_filter: bool,

    /// 현금 보유 비율
    #[serde(default = "default_cash_reserve_rate")]
    #[schema(label = "현금 보유 비율", field_type = "number", min = 0, max = 1, default = "0", section = "filter")]
    pub cash_reserve_rate: Decimal,

    /// 최소 GlobalScore
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", field_type = "number", min = 0, max = 100, default = "60", section = "indicator")]
    pub min_global_score: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default = "ExitConfig::for_rebalancing")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

impl From<MarketCapTopConfig> for RotationConfig {
    fn from(cfg: MarketCapTopConfig) -> Self {
        let mut base = Self::market_cap_top_default();
        base.top_n = cfg.top_n;
        base.total_amount = cfg.total_amount;
        base.rebalance_threshold = cfg.rebalance_threshold;
        base.use_momentum_filter = cfg.use_momentum_filter;
        base.cash_reserve_rate = cfg.cash_reserve_rate;
        base.min_global_score = cfg.min_global_score;
        base
    }
}

impl RotationConfig {
    // ========================================================================
    // 섹터 모멘텀 기본 설정
    // ========================================================================

    /// 섹터 모멘텀 US 기본 설정.
    pub fn sector_momentum_default() -> Self {
        Self {
            variant: RotationVariant::SectorMomentum,
            market: MarketType::US,
            universe: Self::us_sector_universe(),
            top_n: 3,
            total_amount: default_total_amount(),
            ranking_metric: RankingMetric::MultiPeriodMomentum {
                short_period: 20,
                medium_period: 60,
                long_period: 120,
                short_weight: 0.5,
                medium_weight: 0.3,
                long_weight: 0.2,
            },
            weighting_method: WeightingMethod::Equal,
            rebalance_frequency: RebalanceFrequency::Monthly,
            rebalance_threshold: default_rebalance_threshold(),
            min_momentum: None,
            cash_reserve_rate: Decimal::ZERO,
            use_momentum_filter: false,
            min_global_score: default_min_global_score(),
        }
    }

    /// 섹터 모멘텀 KR 기본 설정.
    pub fn sector_momentum_kr() -> Self {
        let mut config = Self::sector_momentum_default();
        config.market = MarketType::KR;
        config.universe = Self::kr_sector_universe();
        config
    }

    // ========================================================================
    // 종목 로테이션 기본 설정
    // ========================================================================

    /// 종목 로테이션 US 기본 설정.
    pub fn stock_rotation_default() -> Self {
        Self {
            variant: RotationVariant::StockMomentum,
            market: MarketType::US,
            universe: Self::us_mega_cap_universe(),
            top_n: 5,
            total_amount: default_total_amount(),
            ranking_metric: RankingMetric::AverageMomentum {
                periods: vec![20, 60, 120, 240], // 1M, 3M, 6M, 12M
            },
            weighting_method: WeightingMethod::Equal,
            rebalance_frequency: RebalanceFrequency::Monthly,
            rebalance_threshold: dec!(3),
            min_momentum: None,
            cash_reserve_rate: Decimal::ZERO,
            use_momentum_filter: false,
            min_global_score: default_min_global_score(),
        }
    }

    /// 종목 로테이션 KR 기본 설정.
    pub fn stock_rotation_kr() -> Self {
        let mut config = Self::stock_rotation_default();
        config.market = MarketType::KR;
        config.universe = Self::kr_large_cap_universe();
        config
    }

    // ========================================================================
    // 시가총액 상위 기본 설정
    // ========================================================================

    /// 시가총액 상위 기본 설정.
    pub fn market_cap_top_default() -> Self {
        Self {
            variant: RotationVariant::MarketCapTop,
            market: MarketType::US,
            universe: Self::us_mega_cap_universe(),
            top_n: 10,
            total_amount: default_total_amount(),
            ranking_metric: RankingMetric::SinglePeriodMomentum { period: 252 },
            weighting_method: WeightingMethod::Equal,
            rebalance_frequency: RebalanceFrequency::Days(30),
            rebalance_threshold: default_rebalance_threshold(),
            min_momentum: None,
            cash_reserve_rate: Decimal::ZERO,
            use_momentum_filter: false,
            min_global_score: default_min_global_score(),
        }
    }

    // ========================================================================
    // 유니버스 정의
    // ========================================================================

    /// US 섹터 ETF 유니버스.
    pub fn us_sector_universe() -> Vec<AssetInfo> {
        vec![
            AssetInfo::new("XLK", "기술").with_sector("Technology"),
            AssetInfo::new("XLF", "금융").with_sector("Financials"),
            AssetInfo::new("XLV", "헬스케어").with_sector("Healthcare"),
            AssetInfo::new("XLY", "경기소비재").with_sector("Consumer Discretionary"),
            AssetInfo::new("XLP", "필수소비재").with_sector("Consumer Staples"),
            AssetInfo::new("XLE", "에너지").with_sector("Energy"),
            AssetInfo::new("XLI", "산업재").with_sector("Industrials"),
            AssetInfo::new("XLB", "소재").with_sector("Materials"),
            AssetInfo::new("XLU", "유틸리티").with_sector("Utilities"),
            AssetInfo::new("XLRE", "부동산").with_sector("Real Estate"),
            AssetInfo::new("XLC", "통신").with_sector("Communication Services"),
        ]
    }

    /// KR 섹터 ETF 유니버스.
    pub fn kr_sector_universe() -> Vec<AssetInfo> {
        vec![
            AssetInfo::new("091160", "KODEX 반도체").with_sector("IT"),
            AssetInfo::new("091230", "TIGER 반도체").with_sector("IT"),
            AssetInfo::new("305720", "KODEX 2차전지산업").with_sector("IT"),
            AssetInfo::new("305540", "TIGER 2차전지테마").with_sector("IT"),
            AssetInfo::new("091170", "KODEX 은행").with_sector("Financials"),
            AssetInfo::new("091220", "TIGER 은행").with_sector("Financials"),
            AssetInfo::new("102970", "KODEX 철강").with_sector("Materials"),
            AssetInfo::new("117460", "KODEX 건설").with_sector("Industrials"),
            AssetInfo::new("091180", "TIGER 자동차").with_sector("Consumer"),
            AssetInfo::new("102960", "KODEX 기계장비").with_sector("Industrials"),
        ]
    }

    /// US 메가캡 유니버스.
    pub fn us_mega_cap_universe() -> Vec<AssetInfo> {
        vec![
            AssetInfo::new("AAPL", "Apple Inc.").with_sector("Technology"),
            AssetInfo::new("MSFT", "Microsoft Corporation").with_sector("Technology"),
            AssetInfo::new("GOOGL", "Alphabet Inc.").with_sector("Technology"),
            AssetInfo::new("AMZN", "Amazon.com Inc.").with_sector("Consumer"),
            AssetInfo::new("NVDA", "NVIDIA Corporation").with_sector("Technology"),
            AssetInfo::new("META", "Meta Platforms Inc.").with_sector("Technology"),
            AssetInfo::new("TSLA", "Tesla Inc.").with_sector("Consumer"),
            AssetInfo::new("BRK.B", "Berkshire Hathaway").with_sector("Financials"),
            AssetInfo::new("JPM", "JPMorgan Chase & Co.").with_sector("Financials"),
            AssetInfo::new("V", "Visa Inc.").with_sector("Financials"),
            AssetInfo::new("UNH", "UnitedHealth Group").with_sector("Healthcare"),
            AssetInfo::new("JNJ", "Johnson & Johnson").with_sector("Healthcare"),
            AssetInfo::new("XOM", "Exxon Mobil").with_sector("Energy"),
            AssetInfo::new("PG", "Procter & Gamble").with_sector("Consumer Staples"),
            AssetInfo::new("MA", "Mastercard").with_sector("Financials"),
        ]
    }

    /// KR 대형주 유니버스.
    pub fn kr_large_cap_universe() -> Vec<AssetInfo> {
        vec![
            AssetInfo::new("005930", "삼성전자").with_sector("IT"),
            AssetInfo::new("000660", "SK하이닉스").with_sector("IT"),
            AssetInfo::new("373220", "LG에너지솔루션").with_sector("IT"),
            AssetInfo::new("207940", "삼성바이오로직스").with_sector("Healthcare"),
            AssetInfo::new("005380", "현대차").with_sector("Consumer"),
            AssetInfo::new("006400", "삼성SDI").with_sector("IT"),
            AssetInfo::new("035420", "NAVER").with_sector("IT"),
            AssetInfo::new("000270", "기아").with_sector("Consumer"),
            AssetInfo::new("035720", "카카오").with_sector("IT"),
            AssetInfo::new("105560", "KB금융").with_sector("Financials"),
        ]
    }

    /// 유니버스의 모든 티커 반환.
    pub fn all_tickers(&self) -> Vec<String> {
        self.universe.iter().map(|a| a.ticker.clone()).collect()
    }
}

// ============================================================================
// 자산 데이터 (Asset Data)
// ============================================================================

/// 자산별 내부 데이터.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct AssetData {
    /// 티커
    ticker: String,
    /// 자산명
    name: String,
    /// 현재 가격 (StrategyContext에서 업데이트)
    current_price: Decimal,
    /// 모멘텀 스코어
    momentum_score: Decimal,
    /// 현재 보유 수량
    holdings: Decimal,
}

impl AssetData {
    fn new(ticker: String, name: String) -> Self {
        Self {
            ticker,
            name,
            current_price: Decimal::ZERO,
            momentum_score: Decimal::ZERO,
            holdings: Decimal::ZERO,
        }
    }

    /// 현재 가격 업데이트.
    fn update_price(&mut self, price: Decimal) {
        self.current_price = price;
    }

    /// 모멘텀 점수 업데이트 (GlobalScore에서 가져온 값).
    fn update_momentum(&mut self, score: Decimal) {
        self.momentum_score = score;
    }

    /// 가격 기반 변동성 계산.
    fn calculate_volatility(prices: &[Decimal], period: usize) -> Decimal {
        if prices.len() < period + 1 {
            return Decimal::ZERO;
        }

        let returns: Vec<Decimal> = prices
            .windows(2)
            .take(period)
            .filter_map(|w| {
                if w[1] > Decimal::ZERO {
                    Some((w[0] - w[1]) / w[1])
                } else {
                    None
                }
            })
            .collect();

        if returns.is_empty() {
            return Decimal::ZERO;
        }

        let mean: Decimal = returns.iter().copied().sum::<Decimal>() / Decimal::from(returns.len());
        let variance: Decimal = returns
            .iter()
            .map(|r| {
                let diff = *r - mean;
                diff * diff
            })
            .sum::<Decimal>()
            / Decimal::from(returns.len());

        // 제곱근 근사 (Newton-Raphson)
        sqrt_approx(variance)
    }
}

/// 제곱근 근사.
fn sqrt_approx(x: Decimal) -> Decimal {
    if x <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    let mut guess = x / dec!(2);
    for _ in 0..10 {
        guess = (guess + x / guess) / dec!(2);
    }
    guess
}

// ============================================================================
// 순위 정보 (Ranking Info)
// ============================================================================

/// 자산 순위 정보.
#[derive(Debug, Clone)]
struct RankedAsset {
    ticker: String,
    score: Decimal,
    rank: usize,
}

// ============================================================================
// 로테이션 전략 (Rotation Strategy)
// ============================================================================

/// 로테이션 전략.
///
/// 섹터 모멘텀, 종목 로테이션, 시가총액 상위 전략을 통합한 그룹 전략입니다.
pub struct RotationStrategy {
    /// 설정
    config: Option<RotationConfig>,

    /// 전략 컨텍스트 (RouteState, GlobalScore 조회용)
    context: Option<Arc<RwLock<StrategyContext>>>,

    /// 자산별 데이터
    asset_data: HashMap<String, AssetData>,

    /// 현재 보유 자산
    current_holdings: HashSet<String>,

    /// 포지션 정보 (ticker -> quantity)
    positions: HashMap<String, Decimal>,

    /// 마지막 리밸런싱 정보 (월: YYYY_MM, 일: day_of_year)
    last_rebalance: Option<String>,

    /// 현재 날짜 (day of year)
    current_day: u32,

    /// 리밸런싱 계산기
    rebalance_calculator: Option<RebalanceCalculator>,

    /// 현금 잔고
    cash_balance: Decimal,

    /// 초기화 여부
    initialized: bool,

    /// 통계: 거래 횟수
    trades_count: u32,
}

impl RotationStrategy {
    /// 새 전략 생성 (기본값).
    ///
    /// config는 `initialize()`에서 설정됩니다.
    pub fn new() -> Self {
        Self {
            config: None,
            context: None,
            asset_data: HashMap::new(),
            current_holdings: HashSet::new(),
            positions: HashMap::new(),
            last_rebalance: None,
            current_day: 0,
            rebalance_calculator: None,
            cash_balance: Decimal::ZERO,
            initialized: false,
            trades_count: 0,
        }
    }

    /// 설정으로 전략 생성.
    pub fn with_config(config: RotationConfig) -> Self {
        let rebalance_config = config.market.rebalance_config();

        Self {
            config: Some(config),
            context: None,
            asset_data: HashMap::new(),
            current_holdings: HashSet::new(),
            positions: HashMap::new(),
            last_rebalance: None,
            current_day: 0,
            rebalance_calculator: Some(RebalanceCalculator::new(rebalance_config)),
            cash_balance: Decimal::ZERO,
            initialized: false,
            trades_count: 0,
        }
    }

    // ========================================================================
    // 팩토리 메서드
    // ========================================================================

    /// 섹터 모멘텀 전략 (US).
    pub fn sector_momentum() -> Self {
        Self::with_config(RotationConfig::sector_momentum_default())
    }

    /// 섹터 모멘텀 전략 (KR).
    pub fn sector_momentum_kr() -> Self {
        Self::with_config(RotationConfig::sector_momentum_kr())
    }

    /// 종목 로테이션 전략 (US).
    pub fn stock_rotation() -> Self {
        Self::with_config(RotationConfig::stock_rotation_default())
    }

    /// 종목 로테이션 전략 (KR).
    pub fn stock_rotation_kr() -> Self {
        Self::with_config(RotationConfig::stock_rotation_kr())
    }

    /// 시가총액 상위 전략.
    pub fn market_cap_top() -> Self {
        Self::with_config(RotationConfig::market_cap_top_default())
    }

    // ========================================================================
    // 진입 조건 체크
    // ========================================================================

    /// RouteState 기반 진입 조건 체크.
    /// GlobalScore는 한국 시장(KR)에서만 사용 (한국 주식 스크리닝용).
    /// 미국 시장(US)은 자체 모멘텀 계산을 사용하므로 GlobalScore 불필요.
    fn can_enter(&self, ticker: &str) -> bool {
        let Some(config) = self.config.as_ref() else {
            return false;
        };

        let Some(ctx) = self.context.as_ref() else {
            // Context 없으면 진입 허용 (하위 호환성)
            return true;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            return true;
        };

        // RouteState 체크 - Overheat 시만 진입 제한
        if let Some(route_state) = ctx_lock.get_route_state(ticker) {
            match route_state {
                RouteState::Overheat => {
                    debug!(
                        ticker = %ticker,
                        route_state = ?route_state,
                        "[Rotation] 시장 과열 - 진입 제한"
                    );
                    return false;
                }
                // 나머지 상태는 진입 허용
                _ => {}
            }
        }

        // GlobalScore 체크 - 한국 시장에서만 (한국 주식 스크리닝용)
        if config.market == MarketType::KR {
            if let Some(score) = ctx_lock.get_global_score(ticker) {
                if score.overall_score < config.min_global_score {
                    debug!(
                        ticker = %ticker,
                        score = %score.overall_score,
                        min_score = %config.min_global_score,
                        "[Rotation] GlobalScore 미달 - 진입 제한"
                    );
                    return false;
                }
            }
        }

        true
    }

    /// GlobalScore 기반 신호 강도 계산.
    fn get_adjusted_strength(&self, ticker: &str, base_strength: f64) -> f64 {
        let Some(ctx) = self.context.as_ref() else {
            return base_strength;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            return base_strength;
        };

        if let Some(score) = ctx_lock.get_global_score(ticker) {
            adjust_strength_by_score(base_strength, Some(score.overall_score))
        } else {
            base_strength
        }
    }

    // ========================================================================
    // 모멘텀 계산
    // ========================================================================

    /// 모든 자산의 모멘텀 업데이트 (GlobalScore에서 가져옴).
    fn calculate_all_momentum(&mut self) {
        let Some(ctx) = self.context.as_ref() else {
            warn!("[Rotation] StrategyContext 없음 - 모멘텀 계산 불가");
            return;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            warn!("[Rotation] StrategyContext 잠금 실패");
            return;
        };

        // GlobalScore에서 각 자산의 모멘텀 점수 가져오기
        for data in self.asset_data.values_mut() {
            if let Some(score) = ctx_lock.get_global_score(&data.ticker) {
                // component_scores에서 momentum 가져오기
                let momentum = score
                    .component_scores
                    .get("momentum")
                    .copied()
                    .unwrap_or(Decimal::ZERO);
                data.update_momentum(momentum);

                // 현재 가격도 klines에서 업데이트
                let klines = ctx_lock.get_klines(&data.ticker, Timeframe::D1);
                if let Some(last_kline) = klines.last() {
                    data.update_price(last_kline.close);
                }

                debug!(
                    ticker = %data.ticker,
                    momentum = %momentum,
                    overall_score = %score.overall_score,
                    "[Rotation] GlobalScore에서 모멘텀 가져옴"
                );
            } else {
                debug!(
                    ticker = %data.ticker,
                    "[Rotation] GlobalScore 없음"
                );
            }
        }
    }

    // ========================================================================
    // 순위 계산
    // ========================================================================

    /// 모든 자산의 순위 계산.
    fn rank_assets(&self) -> Vec<RankedAsset> {
        let Some(config) = self.config.as_ref() else {
            return Vec::new();
        };

        let mut assets: Vec<RankedAsset> = self
            .asset_data
            .values()
            .filter_map(|data| {
                // 최소 모멘텀 필터
                if let Some(min_mom) = config.min_momentum {
                    if data.momentum_score < min_mom {
                        debug!(
                            ticker = %data.ticker,
                            score = %data.momentum_score,
                            min = %min_mom,
                            "[Rotation] 최소 모멘텀 미달 제외"
                        );
                        return None;
                    }
                }

                // 양의 모멘텀만 (모멘텀 필터 사용 시)
                if config.use_momentum_filter && data.momentum_score <= Decimal::ZERO {
                    return None;
                }

                Some(RankedAsset {
                    ticker: data.ticker.clone(),
                    score: data.momentum_score,
                    rank: 0,
                })
            })
            .collect();

        // 모멘텀 내림차순 정렬
        assets.sort_by(|a, b| b.score.cmp(&a.score));

        // 순위 부여
        for (i, asset) in assets.iter_mut().enumerate() {
            asset.rank = i + 1;
        }

        assets
    }

    // ========================================================================
    // 비중 계산
    // ========================================================================

    /// 목표 비중 계산.
    fn calculate_target_weights(&self, ranked_assets: &[RankedAsset]) -> Vec<TargetAllocation> {
        let Some(config) = self.config.as_ref() else {
            return Vec::new();
        };

        let top_n = config.top_n.min(ranked_assets.len());
        if top_n == 0 {
            return Vec::new();
        }

        let investable_rate = dec!(1) - config.cash_reserve_rate;
        let mut allocations = Vec::new();

        match config.weighting_method {
            WeightingMethod::Equal => {
                let weight = investable_rate / Decimal::from(top_n);
                for asset in ranked_assets.iter().take(top_n) {
                    allocations.push(TargetAllocation::new(asset.ticker.clone(), weight));
                    debug!(
                        ticker = %asset.ticker,
                        rank = asset.rank,
                        score = %asset.score,
                        weight = %weight,
                        "[Rotation] 균등 비중 할당"
                    );
                }
            }
            WeightingMethod::MomentumProportional => {
                let total_score: Decimal = ranked_assets
                    .iter()
                    .take(top_n)
                    .map(|a| a.score.max(Decimal::ZERO))
                    .sum();

                for asset in ranked_assets.iter().take(top_n) {
                    let weight = if total_score > Decimal::ZERO {
                        (asset.score.max(Decimal::ZERO) / total_score) * investable_rate
                    } else {
                        investable_rate / Decimal::from(top_n)
                    };
                    allocations.push(TargetAllocation::new(asset.ticker.clone(), weight));
                    debug!(
                        ticker = %asset.ticker,
                        rank = asset.rank,
                        score = %asset.score,
                        weight = %weight,
                        "[Rotation] 모멘텀 비례 비중 할당"
                    );
                }
            }
            WeightingMethod::InverseVolatility => {
                let mut inv_vols: Vec<(String, Decimal)> = Vec::new();
                let mut total_inv_vol = Decimal::ZERO;

                // StrategyContext에서 klines 가져와서 변동성 계산
                let ctx_opt = self.context.as_ref().and_then(|c| c.try_read().ok());

                for asset in ranked_assets.iter().take(top_n) {
                    let vol = if let Some(ref ctx_lock) = ctx_opt {
                        let klines = ctx_lock.get_klines(&asset.ticker, Timeframe::D1);
                        let prices: Vec<Decimal> = klines.iter().map(|k| k.close).collect();
                        AssetData::calculate_volatility(&prices, 20)
                    } else {
                        Decimal::ZERO
                    };

                    if vol > Decimal::ZERO {
                        let inv_vol = Decimal::ONE / vol;
                        inv_vols.push((asset.ticker.clone(), inv_vol));
                        total_inv_vol += inv_vol;
                    }
                }

                if total_inv_vol > Decimal::ZERO {
                    for (ticker, inv_vol) in inv_vols {
                        let weight = (inv_vol / total_inv_vol) * investable_rate;
                        allocations.push(TargetAllocation::new(ticker.clone(), weight));
                        debug!(
                            ticker = %ticker,
                            weight = %weight,
                            "[Rotation] 역변동성 비중 할당"
                        );
                    }
                } else {
                    // 데이터 부족 시 균등 비중
                    let weight = investable_rate / Decimal::from(top_n);
                    for asset in ranked_assets.iter().take(top_n) {
                        allocations.push(TargetAllocation::new(asset.ticker.clone(), weight));
                    }
                }
            }
        }

        allocations
    }

    // ========================================================================
    // 리밸런싱 체크
    // ========================================================================

    /// 리밸런싱 필요 여부 확인.
    fn should_rebalance(&self, timestamp: DateTime<Utc>) -> bool {
        let Some(config) = self.config.as_ref() else {
            return false;
        };

        match &config.rebalance_frequency {
            RebalanceFrequency::Monthly => {
                let current_ym = format!("{}_{}", timestamp.year(), timestamp.month());
                match &self.last_rebalance {
                    None => true,
                    Some(last) => last != &current_ym,
                }
            }
            RebalanceFrequency::Days(days) => match &self.last_rebalance {
                None => true,
                Some(last) => {
                    if let Ok(last_day) = last.parse::<u32>() {
                        let days_passed = if self.current_day >= last_day {
                            self.current_day - last_day
                        } else {
                            365 - last_day + self.current_day
                        };
                        days_passed >= *days
                    } else {
                        true
                    }
                }
            },
        }
    }

    /// 리밸런싱 정보 업데이트.
    fn update_rebalance_info(&mut self, timestamp: DateTime<Utc>) {
        let Some(config) = self.config.as_ref() else {
            return;
        };

        match &config.rebalance_frequency {
            RebalanceFrequency::Monthly => {
                self.last_rebalance = Some(format!("{}_{}", timestamp.year(), timestamp.month()));
            }
            RebalanceFrequency::Days(_) => {
                self.last_rebalance = Some(self.current_day.to_string());
            }
        }
    }

    // ========================================================================
    // 교체 대상 계산
    // ========================================================================

    /// 교체 대상 종목 계산.
    fn calculate_rotation(&self, ranked_assets: &[RankedAsset]) -> (Vec<String>, Vec<String>) {
        let Some(config) = self.config.as_ref() else {
            return (Vec::new(), Vec::new());
        };

        // 상위 N개 종목
        let new_top_n: HashSet<String> = ranked_assets
            .iter()
            .take(config.top_n)
            .map(|a| a.ticker.clone())
            .collect();

        // 매도 대상: 현재 보유 중인데 상위 N에서 빠진 종목
        let to_sell: Vec<String> = self
            .current_holdings
            .iter()
            .filter(|t| !new_top_n.contains(*t))
            .cloned()
            .collect();

        // 매수 대상: 상위 N에 있는데 현재 미보유
        let to_buy: Vec<String> = new_top_n
            .iter()
            .filter(|t| !self.current_holdings.contains(*t))
            .cloned()
            .collect();

        (to_sell, to_buy)
    }

    // ========================================================================
    // 신호 생성
    // ========================================================================

    /// 리밸런싱 신호 생성.
    fn generate_rebalance_signals(&mut self, timestamp: DateTime<Utc>) -> Vec<Signal> {
        let Some(config) = self.config.clone() else {
            return Vec::new();
        };

        if !self.should_rebalance(timestamp) {
            return Vec::new();
        }

        // 모멘텀 계산
        self.calculate_all_momentum();

        // 순위 계산
        let ranked_assets = self.rank_assets();

        if ranked_assets.is_empty() {
            warn!("[Rotation] 순위 계산 가능한 자산 없음");
            return Vec::new();
        }

        // 교체 대상 계산
        let (to_sell, to_buy) = self.calculate_rotation(&ranked_assets);

        if !to_sell.is_empty() {
            info!(
                variant = ?config.variant,
                to_sell = ?to_sell,
                "[Rotation] 매도 예정 (순위 이탈)"
            );
        }
        if !to_buy.is_empty() {
            info!(
                variant = ?config.variant,
                to_buy = ?to_buy,
                "[Rotation] 매수 예정 (순위 진입)"
            );
        }

        // 목표 비중 계산
        let target_allocations = self.calculate_target_weights(&ranked_assets);

        if target_allocations.is_empty() {
            return Vec::new();
        }

        // 현재 포지션 구성
        let quote_currency = config.market.quote_currency();
        let mut portfolio_positions: Vec<PortfolioPosition> = self
            .asset_data
            .values()
            .filter(|d| d.holdings > Decimal::ZERO)
            .map(|d| PortfolioPosition::new(&d.ticker, d.holdings, d.current_price))
            .collect();

        // 현금 포지션 추가
        let invested: Decimal = portfolio_positions.iter().map(|p| p.market_value).sum();
        let cash_available = config.total_amount - invested + self.cash_balance;
        if cash_available > Decimal::ZERO {
            portfolio_positions.push(PortfolioPosition::cash(cash_available, quote_currency));
        }

        // 리밸런싱 계산
        let Some(calculator) = self.rebalance_calculator.as_ref() else {
            return Vec::new();
        };

        let result = calculator.calculate_orders(&portfolio_positions, &target_allocations);

        // 신호 변환
        let mut signals = Vec::new();

        for order in result.orders {
            let side = match order.side {
                RebalanceOrderSide::Buy => Side::Buy,
                RebalanceOrderSide::Sell => Side::Sell,
            };

            // 매수 신호의 경우 can_enter() 체크
            if side == Side::Buy && !self.can_enter(&order.ticker) {
                debug!(
                    ticker = %order.ticker,
                    "[Rotation] RouteState/GlobalScore 조건 미충족, 매수 신호 스킵"
                );
                continue;
            }

            // 교체 사유 결정
            let rotation_type = if to_sell.contains(&order.ticker) {
                "rank_exit"
            } else if to_buy.contains(&order.ticker) {
                "rank_enter"
            } else {
                "rebalance"
            };

            let signal_type = if side == Side::Buy {
                SignalType::Entry
            } else {
                SignalType::Exit
            };

            // ticker는 그대로 사용 (quote currency 붙이지 않음)
            let strength = self.get_adjusted_strength(&order.ticker, 0.5);
            let signal = Signal::new(self.name(), order.ticker.clone(), side, signal_type)
                .with_strength(strength)
                .with_metadata("variant", json!(format!("{:?}", config.variant)))
                .with_metadata("rotation_type", json!(rotation_type))
                .with_metadata("current_weight", json!(order.current_weight.to_string()))
                .with_metadata("target_weight", json!(order.target_weight.to_string()))
                .with_metadata("amount", json!(order.amount.to_string()))
                .with_metadata("quantity", json!(order.quantity.to_string()));

            signals.push(signal);
        }

        // 리밸런싱 정보 업데이트
        if !signals.is_empty() {
            self.update_rebalance_info(timestamp);

            // 보유 종목 업데이트
            for ticker in &to_sell {
                self.current_holdings.remove(ticker);
            }
            for ticker in &to_buy {
                self.current_holdings.insert(ticker.clone());
            }

            info!(
                variant = ?config.variant,
                signals_count = signals.len(),
                sell_count = to_sell.len(),
                buy_count = to_buy.len(),
                "[Rotation] 리밸런싱 완료"
            );
        }

        signals
    }
}

impl Default for RotationStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for RotationStrategy {
    fn name(&self) -> &str {
        match self.config.as_ref().map(|c| &c.variant) {
            Some(RotationVariant::SectorMomentum) => "Sector Momentum",
            Some(RotationVariant::StockMomentum) => "Stock Rotation",
            Some(RotationVariant::MarketCapTop) => "Market Cap Top",
            None => "Rotation",
        }
    }

    fn version(&self) -> &str {
        "2.0.0"
    }

    fn description(&self) -> &str {
        match self.config.as_ref().map(|c| &c.variant) {
            Some(RotationVariant::SectorMomentum) => {
                "섹터 ETF의 모멘텀을 분석하여 상위 N개 섹터에 투자합니다."
            }
            Some(RotationVariant::StockMomentum) => {
                "개별 종목의 모멘텀 순위 기반으로 상위 N개 종목을 보유하고 순위 변동 시 교체합니다."
            }
            Some(RotationVariant::MarketCapTop) => {
                "시가총액 상위 종목에 투자하는 패시브 전략입니다."
            }
            None => "모멘텀/시총 기반 로테이션 전략입니다.",
        }
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 1. config에서 variant와 market 확인 (테스트 등에서 직접 JSON 전달 시)
        // 2. self.config에서 확인 (팩토리 메서드 사용 시)
        // 3. 기본값: SectorMomentum, US
        let variant = config
            .get("variant")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "SectorMomentum" => Some(RotationVariant::SectorMomentum),
                "StockMomentum" => Some(RotationVariant::StockMomentum),
                "MarketCapTop" => Some(RotationVariant::MarketCapTop),
                _ => None,
            })
            .or_else(|| self.config.as_ref().map(|c| c.variant))
            .unwrap_or(RotationVariant::SectorMomentum);

        let market = config
            .get("market")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "US" => Some(MarketType::US),
                "KR" => Some(MarketType::KR),
                _ => None,
            })
            .or_else(|| self.config.as_ref().map(|c| c.market))
            .unwrap_or(MarketType::US);

        // variant와 market에 따라 적절한 Config 타입으로 파싱
        let mut rotation_config: RotationConfig = match (variant, market) {
            (RotationVariant::SectorMomentum, MarketType::US) => {
                let cfg: SectorMomentumConfig = serde_json::from_value(config.clone())?;
                cfg.into()
            }
            (RotationVariant::SectorMomentum, MarketType::KR) => {
                let cfg: SectorMomentumKrConfig = serde_json::from_value(config.clone())?;
                cfg.into()
            }
            (RotationVariant::StockMomentum, MarketType::US) => {
                let cfg: StockRotationConfig = serde_json::from_value(config.clone())?;
                cfg.into()
            }
            (RotationVariant::StockMomentum, MarketType::KR) => {
                let cfg: StockRotationKrConfig = serde_json::from_value(config.clone())?;
                cfg.into()
            }
            (RotationVariant::MarketCapTop, _) => {
                let cfg: MarketCapTopConfig = serde_json::from_value(config.clone())?;
                cfg.into()
            }
        };

        // 빈 유니버스인 경우 variant와 market에 따라 기본 유니버스 설정
        if rotation_config.universe.is_empty() {
            rotation_config.universe = match (&rotation_config.variant, &rotation_config.market) {
                (RotationVariant::SectorMomentum, MarketType::US) => {
                    RotationConfig::us_sector_universe()
                }
                (RotationVariant::SectorMomentum, MarketType::KR) => {
                    RotationConfig::kr_sector_universe()
                }
                (RotationVariant::StockMomentum, MarketType::US) => {
                    RotationConfig::us_mega_cap_universe()
                }
                (RotationVariant::StockMomentum, MarketType::KR) => {
                    RotationConfig::kr_large_cap_universe()
                }
                (RotationVariant::MarketCapTop, _) => RotationConfig::us_mega_cap_universe(),
            };
            info!(
                variant = ?rotation_config.variant,
                market = ?rotation_config.market,
                "[Rotation] 기본 유니버스 사용"
            );
        }

        info!(
            variant = ?rotation_config.variant,
            market = ?rotation_config.market,
            top_n = rotation_config.top_n,
            universe_size = rotation_config.universe.len(),
            "[Rotation] 전략 초기화"
        );

        // 리밸런싱 계산기 설정
        let rebalance_config = rotation_config.market.rebalance_config();
        self.rebalance_calculator = Some(RebalanceCalculator::new(rebalance_config));

        // 자산 데이터 초기화
        self.asset_data.clear();
        for asset in &rotation_config.universe {
            self.asset_data.insert(
                asset.ticker.clone(),
                AssetData::new(asset.ticker.clone(), asset.name.clone()),
            );
        }

        // 초기 자본금 설정
        if let Some(capital_val) = config.get("initial_capital") {
            if let Some(capital_str) = capital_val.as_str() {
                if let Ok(capital) = capital_str.parse::<Decimal>() {
                    self.cash_balance = capital;
                    info!(capital = %capital, "[Rotation] 초기 자본금 설정");
                }
            }
        }

        self.config = Some(rotation_config);
        self.initialized = true;

        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        if !self.initialized {
            return Ok(vec![]);
        }

        let Some(config) = self.config.as_ref() else {
            return Ok(vec![]);
        };

        let ticker = data.ticker.clone();

        // 유니버스에 없는 종목이면 무시
        if !config.all_tickers().contains(&ticker) {
            return Ok(vec![]);
        }

        // 가격 및 시간 추출
        let (price, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (kline.close, kline.open_time),
            MarketDataType::Ticker(t) => (t.last, data.timestamp),
            MarketDataType::Trade(trade) => (trade.price, trade.timestamp),
            MarketDataType::OrderBook(_) => return Ok(vec![]),
        };

        // 날짜 업데이트
        self.current_day = timestamp.ordinal();

        // 가격 업데이트
        if let Some(asset_data) = self.asset_data.get_mut(&ticker) {
            asset_data.update_price(price);
        }

        // 리밸런싱 신호 생성 (GlobalScore가 있을 때만)
        let has_global_scores = self.context.as_ref().map_or(false, |ctx| {
            ctx.try_read().map_or(false, |lock| {
                self.asset_data
                    .keys()
                    .any(|ticker| lock.get_global_score(ticker).is_some())
            })
        });

        if has_global_scores {
            let signals = self.generate_rebalance_signals(timestamp);
            return Ok(signals);
        }

        Ok(vec![])
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ticker = order.ticker.clone();

        if let Some(data) = self.asset_data.get_mut(&ticker) {
            match order.side {
                Side::Buy => {
                    data.holdings += order.quantity;
                    self.current_holdings.insert(ticker.clone());
                }
                Side::Sell => {
                    data.holdings -= order.quantity;
                    if data.holdings <= Decimal::ZERO {
                        data.holdings = Decimal::ZERO;
                        self.current_holdings.remove(&ticker);
                    }
                }
            }
            self.trades_count += 1;
        }

        info!(
            ticker = %ticker,
            side = ?order.side,
            quantity = %order.quantity,
            "[Rotation] 주문 체결"
        );

        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ticker = position.ticker.clone();

        if position.quantity > Decimal::ZERO {
            self.positions.insert(ticker.clone(), position.quantity);
            self.current_holdings.insert(ticker.clone());

            if let Some(data) = self.asset_data.get_mut(&ticker) {
                data.holdings = position.quantity;
            }
        } else {
            self.positions.remove(&ticker);
            self.current_holdings.remove(&ticker);

            if let Some(data) = self.asset_data.get_mut(&ticker) {
                data.holdings = Decimal::ZERO;
            }
        }

        debug!(
            ticker = %ticker,
            quantity = %position.quantity,
            pnl = %position.unrealized_pnl,
            "[Rotation] 포지션 업데이트"
        );

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            trades_count = self.trades_count,
            holdings_count = self.current_holdings.len(),
            "[Rotation] 전략 종료"
        );
        self.initialized = false;
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "variant": self.config.as_ref().map(|c| format!("{:?}", c.variant)),
            "market": self.config.as_ref().map(|c| format!("{:?}", c.market)),
            "initialized": self.initialized,
            "holdings_count": self.current_holdings.len(),
            "current_holdings": self.current_holdings.iter().collect::<Vec<_>>(),
            "last_rebalance": self.last_rebalance,
            "trades_count": self.trades_count,
            "cash_balance": self.cash_balance.to_string(),
            "positions": self.positions.iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect::<HashMap<_, _>>(),
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("[Rotation] StrategyContext 주입 완료");
    }
}

// ============================================================================
// 전략 레지스트리 등록
// ============================================================================

use crate::register_strategy;

// 섹터 모멘텀 전략 (미국)
register_strategy! {
    id: "sector_momentum",
    aliases: ["sector_rotation", "sector_etf"],
    name: "섹터 모멘텀",
    description: "섹터 ETF 모멘텀 순위 기반 투자 전략",
    timeframe: "1d",
    tickers: ["XLK", "XLF", "XLV", "XLE", "XLI", "XLY", "XLP", "XLB", "XLU", "XLRE", "XLC"],
    category: Monthly,
    markets: [Stock],
    factory: RotationStrategy::sector_momentum,
    config: SectorMomentumConfig
}

// 섹터 모멘텀 전략 (한국)
register_strategy! {
    id: "sector_momentum_kr",
    aliases: ["sector_rotation_kr"],
    name: "섹터 모멘텀 (KR)",
    description: "한국 섹터 ETF 모멘텀 순위 기반 투자 전략",
    timeframe: "1d",
    tickers: ["091160", "091170", "091180", "117700", "227540"],
    category: Monthly,
    markets: [Stock],
    factory: RotationStrategy::sector_momentum_kr,
    config: SectorMomentumKrConfig
}

// 종목 로테이션 전략 (미국)
register_strategy! {
    id: "stock_rotation",
    aliases: ["stock_momentum"],
    name: "종목 로테이션",
    description: "개별 종목 모멘텀 순위 기반 투자 전략",
    timeframe: "1d",
    tickers: ["AAPL", "MSFT", "GOOGL", "AMZN", "NVDA", "META", "TSLA", "BRK.B", "UNH", "JPM"],
    category: Monthly,
    markets: [Stock],
    factory: RotationStrategy::stock_rotation,
    config: StockRotationConfig
}

// 종목 로테이션 전략 (한국)
register_strategy! {
    id: "stock_rotation_kr",
    aliases: ["stock_momentum_kr"],
    name: "종목 로테이션 (KR)",
    description: "한국 대형주 모멘텀 순위 기반 투자 전략",
    timeframe: "1d",
    tickers: ["005930", "000660", "035420", "051910", "006400"],
    category: Monthly,
    markets: [Stock],
    factory: RotationStrategy::stock_rotation_kr,
    config: StockRotationKrConfig
}

// 시총 상위 전략
register_strategy! {
    id: "market_cap_top",
    aliases: ["mega_cap", "top_holdings"],
    name: "시총 상위",
    description: "미국 시총 상위 종목 균등/시총 비중 투자",
    timeframe: "1d",
    tickers: ["AAPL", "MSFT", "GOOGL", "AMZN", "NVDA", "META", "BRK.B", "TSLA", "UNH", "JPM"],
    category: Monthly,
    markets: [Stock],
    factory: RotationStrategy::market_cap_top,
    config: MarketCapTopConfig
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotation_config_sector_momentum() {
        let config = RotationConfig::sector_momentum_default();
        assert_eq!(config.variant, RotationVariant::SectorMomentum);
        assert_eq!(config.market, MarketType::US);
        assert_eq!(config.top_n, 3);
        assert_eq!(config.universe.len(), 11); // US 섹터 11개
    }

    #[test]
    fn test_rotation_config_stock_rotation() {
        let config = RotationConfig::stock_rotation_default();
        assert_eq!(config.variant, RotationVariant::StockMomentum);
        assert_eq!(config.top_n, 5);
    }

    #[test]
    fn test_rotation_config_market_cap_top() {
        let config = RotationConfig::market_cap_top_default();
        assert_eq!(config.variant, RotationVariant::MarketCapTop);
        assert_eq!(config.top_n, 10);
    }

    #[test]
    fn test_asset_info_creation() {
        let asset = AssetInfo::new("XLK", "Technology");
        assert_eq!(asset.ticker, "XLK");
        assert_eq!(asset.name, "Technology");
        assert!(asset.sector.is_none());

        let with_sector = asset.with_sector("Technology");
        assert_eq!(with_sector.sector, Some("Technology".to_string()));
    }

    #[test]
    fn test_strategy_factory_methods() {
        let sector = RotationStrategy::sector_momentum();
        assert_eq!(sector.name(), "Sector Momentum");

        let stock = RotationStrategy::stock_rotation();
        assert_eq!(stock.name(), "Stock Rotation");

        let market_cap = RotationStrategy::market_cap_top();
        assert_eq!(market_cap.name(), "Market Cap Top");
    }

    #[test]
    fn test_market_type_quote_currency() {
        assert_eq!(MarketType::US.quote_currency(), "USD");
        assert_eq!(MarketType::KR.quote_currency(), "KRW");
    }
}
