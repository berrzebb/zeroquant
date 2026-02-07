//! 분할매수(DCA) 전략 그룹
//!
//! 레벨/라운드별 독립 포지션을 관리하는 스프레드 기반 전략들입니다.
//!
//! # 지원 변형
//!
//! - `Grid`: 가격 대역 분할 자동 거래 (그리드 트레이딩)
//! - `MagicSplit`: 단계적 분할 매수
//! - `InfinityBot`: 피라미드 물타기 + 익절
//!
//! # 공통 특징
//!
//! - **position_id**: 레벨/라운드별 독립 포지션 식별
//! - **group_id**: 세션 전체 관리용 그룹 ID
//! - 스프레드 기반 수익 추구
//!
//! # 아키텍처
//!
//! ```text
//! DcaStrategy
//! ├── DcaVariant::Grid → 그리드 레벨별 독립 매수/매도
//! ├── DcaVariant::MagicSplit → 분할 레벨별 독립 매수/청산
//! └── DcaVariant::InfinityBot → 라운드별 물타기/익절
//! ```

use crate::strategies::common::{adjust_strength_by_score, ExitConfig};
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::domain::{MarketRegime, RouteState, StrategyContext};
use trader_core::types::Timeframe;
use trader_core::{Kline, MarketData, MarketDataType, Order, Position, Side, Signal, SignalType};
use trader_strategy_macro::StrategyConfig;

// ================================================================================================
// 전략 변형
// ================================================================================================

/// DCA 전략 변형
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DcaVariant {
    /// 그리드 트레이딩
    #[default]
    Grid,
    /// 매직 분할매수
    MagicSplit,
    /// 인피니티봇 (무한매수)
    InfinityBot,
}

// ================================================================================================
// 설정 타입
// ================================================================================================

/// 분할 매수 레벨 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitLevel {
    /// 추가 매수 트리거 손실률 (예: -3%)
    pub trigger_rate: Decimal,
    /// 목표 수익률 (예: 5%)
    pub target_rate: Decimal,
    /// 투자 금액
    pub amount: Decimal,
}

// ================================================================================================
// 전략별 UI Config (SDUI용)
// ================================================================================================

/// 그리드 트레이딩 전략 설정
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "grid",
    name = "그리드 트레이딩",
    description = "일정 가격 간격으로 매수/매도 주문 배치",
    category = "Realtime"
)]
pub struct GridTradingConfig {
    /// 거래 티커
    #[serde(default = "default_ticker")]
    #[schema(label = "거래 종목", field_type = "symbol", default = "005930")]
    pub ticker: String,

    /// 거래 금액
    #[serde(default = "default_amount")]
    #[schema(label = "거래 금액", field_type = "number", min = 10000, max = 100000000, default = 1000000)]
    pub amount: Decimal,

    /// 그리드 간격 (%)
    #[serde(default = "default_grid_spacing")]
    #[schema(label = "그리드 간격 (%)", field_type = "number", min = 0.1, max = 10, default = 0.2)]
    pub spacing_pct: Decimal,

    /// 그리드 레벨 수
    #[serde(default = "default_grid_levels")]
    #[schema(label = "그리드 레벨 수", field_type = "integer", min = 1, max = 20, default = 15)]
    pub levels: usize,

    /// ATR 기반 동적 간격 사용
    #[serde(default)]
    #[schema(label = "ATR 동적 간격", field_type = "boolean", default = false)]
    pub use_atr: bool,

    /// ATR 기간
    #[serde(default = "default_atr_period")]
    #[schema(label = "ATR 기간", field_type = "integer", min = 5, max = 50, default = 14)]
    pub atr_period: usize,

    /// 청산 설정
    #[serde(default = "ExitConfig::for_grid_trading")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,

    /// 최대 포지션 수
    #[serde(default = "default_grid_max_positions")]
    #[schema(label = "최대 포지션 수", field_type = "integer", min = 1, max = 20, default = 15)]
    pub max_positions: usize,

    /// 그리드 재설정 임계값 (%)
    /// 현재 가격이 그리드 기준 가격에서 이 비율 이상 벗어나면 그리드 재초기화
    #[serde(default = "default_reset_threshold")]
    #[schema(label = "그리드 재설정 임계값 (%)", min = 5, max = 50, default = 10, section = "indicator")]
    pub reset_threshold_pct: Decimal,

    /// 워밍업 캔들 수 (초기 관찰 기간)
    /// 이 기간 동안은 그리드를 설정만 하고 실제 거래는 하지 않음
    #[serde(default = "default_warmup_candles")]
    #[schema(label = "워밍업 캔들 수", field_type = "integer", min = 0, max = 50, default = 5, section = "timing")]
    pub warmup_candles: usize,
}

/// 매직 분할매수 전략 설정
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "magic_split",
    name = "매직 분할매수",
    description = "가격 구간별 분할 매수 및 목표 수익 시 청산",
    category = "Daily"
)]
pub struct MagicSplitConfig {
    /// 거래 티커
    #[serde(default = "default_ticker")]
    #[schema(label = "거래 종목", field_type = "symbol", default = "005930")]
    pub ticker: String,

    /// 분할 매수 레벨
    #[serde(default = "default_split_levels")]
    #[schema(label = "분할 레벨", skip)]
    pub levels: Vec<SplitLevel>,

    /// 청산 설정
    #[serde(default = "ExitConfig::for_grid_trading")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,

    /// 최대 포지션 수
    #[serde(default = "default_split_max_positions")]
    #[schema(label = "최대 포지션 수", field_type = "integer", min = 1, max = 10, default = 5)]
    pub max_positions: usize,
}

/// 인피니티봇 전략 설정
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "infinity_bot",
    name = "무한매수봇",
    description = "피라미드 구조로 하락 시 분할 매수하고 평균 단가 대비 목표 수익률 달성 시 익절",
    category = "Daily"
)]
pub struct InfinityBotConfig {
    /// 대상 티커
    #[serde(default = "default_ticker")]
    #[schema(label = "대상 티커", field_type = "symbol", default = "005930", section = "asset")]
    pub ticker: String,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    #[schema(label = "총 투자 금액", min = 100000, max = 1000000000, default = 10000000, section = "asset")]
    pub total_amount: Decimal,

    /// 최대 라운드 수
    #[serde(default = "default_max_rounds")]
    #[schema(label = "최대 라운드 수", min = 1, max = 100, default = 50, section = "sizing")]
    pub max_rounds: usize,

    /// 라운드당 투자 비율 (%)
    #[serde(default = "default_round_pct")]
    #[schema(label = "라운드당 투자 비율 (%)", min = 0.5, max = 20, default = 2, section = "sizing")]
    pub round_pct: Decimal,

    /// 추가 매수 트리거 하락률 (%)
    #[serde(default = "default_dip_trigger")]
    #[schema(label = "추가 매수 트리거 하락률 (%)", min = 0.5, max = 20, default = 2, section = "indicator")]
    pub dip_trigger_pct: Decimal,

    /// 익절 목표 수익률 (%)
    #[serde(default = "default_take_profit")]
    #[schema(label = "익절 목표 수익률 (%)", min = 0.5, max = 50, default = 3, section = "indicator")]
    pub take_profit_pct: Decimal,

    /// 이동평균 기간
    #[serde(default = "default_ma_period")]
    #[schema(label = "이동평균 기간", min = 5, max = 200, default = 20, section = "indicator")]
    pub ma_period: usize,

    /// 최소 GlobalScore
    #[serde(default = "default_min_score")]
    #[schema(label = "최소 GlobalScore", min = 0, max = 100, default = 50, section = "filter")]
    pub min_global_score: Decimal,

    /// 청산 설정
    #[serde(default = "ExitConfig::for_grid_trading")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

// ================================================================================================
// 기본값 함수
// ================================================================================================

fn default_ticker() -> String {
    "005930".to_string()
}

fn default_amount() -> Decimal {
    dec!(1000000)
}

fn default_grid_spacing() -> Decimal {
    dec!(1)
}

fn default_grid_levels() -> usize {
    5
}

fn default_atr_period() -> usize {
    14
}

fn default_grid_max_positions() -> usize {
    10
}

fn default_reset_threshold() -> Decimal {
    dec!(10) // 10% - 가격이 그리드 기준에서 10% 이상 벗어나면 재설정
}

fn default_warmup_candles() -> usize {
    5 // 5캔들 동안 시장 관찰 후 거래 시작
}

fn default_split_levels() -> Vec<SplitLevel> {
    vec![
        SplitLevel { trigger_rate: dec!(0), target_rate: dec!(10), amount: dec!(100000) },
        SplitLevel { trigger_rate: dec!(-3), target_rate: dec!(8), amount: dec!(150000) },
        SplitLevel { trigger_rate: dec!(-5), target_rate: dec!(6), amount: dec!(200000) },
        SplitLevel { trigger_rate: dec!(-7), target_rate: dec!(5), amount: dec!(250000) },
        SplitLevel { trigger_rate: dec!(-10), target_rate: dec!(4), amount: dec!(300000) },
    ]
}

fn default_split_max_positions() -> usize {
    5
}

fn default_total_amount() -> Decimal {
    dec!(10000000)
}

fn default_max_rounds() -> usize {
    50
}

fn default_round_pct() -> Decimal {
    dec!(2)
}

fn default_dip_trigger() -> Decimal {
    dec!(2)
}

fn default_take_profit() -> Decimal {
    dec!(3)
}

fn default_ma_period() -> usize {
    20
}

fn default_min_score() -> Decimal {
    dec!(50)
}

// ================================================================================================
// 내부 설정 타입
// ================================================================================================

/// DCA 내부 통합 설정
#[derive(Debug, Clone)]
pub struct DcaConfig {
    pub variant: DcaVariant,
    pub ticker: String,
    pub amount: Decimal,
    pub exit_config: ExitConfig,
    pub max_positions: usize,
    pub min_global_score: Decimal,

    // Grid 전용
    pub grid_spacing_pct: Decimal,
    pub grid_levels: usize,
    pub use_atr: bool,
    pub atr_period: usize,
    pub reset_threshold_pct: Decimal, // 그리드 재설정 임계값
    pub warmup_candles: usize, // 워밍업 캔들 수

    // MagicSplit 전용
    pub split_levels: Vec<SplitLevel>,

    // InfinityBot 전용
    pub total_amount: Decimal,
    pub max_rounds: usize,
    pub round_pct: Decimal,
    pub dip_trigger_pct: Decimal,
    pub take_profit_pct: Decimal,
    pub ma_period: usize,
}

impl From<GridTradingConfig> for DcaConfig {
    fn from(cfg: GridTradingConfig) -> Self {
        Self {
            variant: DcaVariant::Grid,
            ticker: cfg.ticker,
            amount: cfg.amount,
            exit_config: cfg.exit_config,
            max_positions: cfg.max_positions,
            min_global_score: Decimal::ZERO, // Grid는 GlobalScore 필터 미사용
            grid_spacing_pct: cfg.spacing_pct,
            grid_levels: cfg.levels,
            use_atr: cfg.use_atr,
            atr_period: cfg.atr_period,
            reset_threshold_pct: cfg.reset_threshold_pct,
            warmup_candles: cfg.warmup_candles, // 워밍업 캔들 수
            split_levels: vec![],
            total_amount: Decimal::ZERO,
            max_rounds: 0,
            round_pct: Decimal::ZERO,
            dip_trigger_pct: Decimal::ZERO,
            take_profit_pct: Decimal::ZERO,
            ma_period: 0,
        }
    }
}

impl From<MagicSplitConfig> for DcaConfig {
    fn from(cfg: MagicSplitConfig) -> Self {
        let amount = cfg.levels.first().map(|l| l.amount).unwrap_or(dec!(100000));
        Self {
            variant: DcaVariant::MagicSplit,
            ticker: cfg.ticker,
            amount,
            exit_config: cfg.exit_config,
            max_positions: cfg.max_positions,
            min_global_score: Decimal::ZERO, // MagicSplit은 단일 티커 전략이므로 GlobalScore 필터 미사용 (강도 조정만)
            grid_spacing_pct: Decimal::ZERO,
            grid_levels: 0,
            use_atr: false,
            atr_period: 0,
            reset_threshold_pct: Decimal::ZERO, // MagicSplit은 그리드 재설정 미사용
            warmup_candles: 0, // MagicSplit은 워밍업 미사용
            split_levels: cfg.levels,
            total_amount: Decimal::ZERO,
            max_rounds: 0,
            round_pct: Decimal::ZERO,
            dip_trigger_pct: Decimal::ZERO,
            take_profit_pct: Decimal::ZERO,
            ma_period: 0,
        }
    }
}

impl From<InfinityBotConfig> for DcaConfig {
    fn from(cfg: InfinityBotConfig) -> Self {
        Self {
            variant: DcaVariant::InfinityBot,
            ticker: cfg.ticker,
            amount: cfg.total_amount * cfg.round_pct / dec!(100),
            exit_config: cfg.exit_config,
            max_positions: cfg.max_rounds,
            min_global_score: Decimal::ZERO, // InfinityBot은 GlobalScore 필터 미사용 (강도 조정만)
            grid_spacing_pct: Decimal::ZERO,
            grid_levels: 0,
            use_atr: false,
            atr_period: 0,
            reset_threshold_pct: Decimal::ZERO, // InfinityBot은 그리드 재설정 미사용
            warmup_candles: 0, // InfinityBot은 워밍업 미사용 (즉시 진입)
            split_levels: vec![],
            total_amount: cfg.total_amount,
            max_rounds: cfg.max_rounds,
            round_pct: cfg.round_pct,
            dip_trigger_pct: cfg.dip_trigger_pct,
            take_profit_pct: cfg.take_profit_pct,
            ma_period: cfg.ma_period,
        }
    }
}

// ================================================================================================
// 내부 상태
// ================================================================================================

/// 그리드 레벨 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GridLevelState {
    /// 매수 대기
    WaitingBuy,
    /// 매도 대기
    WaitingSell,
}

/// 그리드 레벨
#[derive(Debug, Clone)]
struct GridLevel {
    buy_price: Decimal,
    sell_price: Decimal,
    state: GridLevelState,
}

/// 분할 레벨 상태
#[derive(Debug, Clone, Default)]
struct SplitLevelState {
    is_bought: bool,
    entry_price: Decimal,
    quantity: Decimal,
}

/// 분할 매수 액션
#[derive(Debug, Clone, Copy)]
enum SplitAction {
    Buy,
    TakeProfit,
    StopLoss,
}

/// 인피니티봇 라운드 정보
#[derive(Debug, Clone)]
struct RoundInfo {
    round: usize,
    entry_price: Decimal,
    #[allow(dead_code)]
    quantity: Decimal,
    #[allow(dead_code)]
    timestamp: i64,
}

/// 인피니티봇 상태
#[derive(Debug, Clone, Default)]
struct InfinityBotState {
    current_round: usize,
    rounds: Vec<RoundInfo>,
    avg_price: Option<Decimal>,
    total_quantity: Decimal,
    invested_amount: Decimal,
}

impl InfinityBotState {
    fn calculate_avg_price(&self) -> Option<Decimal> {
        if self.total_quantity.is_zero() {
            return None;
        }
        Some(self.invested_amount / self.total_quantity)
    }

    fn current_return(&self, current_price: Decimal) -> Option<Decimal> {
        let avg = self.avg_price?;
        if avg.is_zero() {
            return None;
        }
        Some((current_price - avg) / avg * dec!(100))
    }
}

// ================================================================================================
// 전략 구현
// ================================================================================================

/// 분할매수(DCA) 전략
pub struct DcaStrategy {
    /// 초기 variant (팩토리 메서드에서 설정, name() 반환용)
    initial_variant: DcaVariant,
    config: Option<DcaConfig>,
    context: Option<Arc<RwLock<StrategyContext>>>,
    initialized: bool,

    // Grid 상태
    grid_levels: Vec<GridLevel>,
    grid_base_price: Decimal,
    grid_group_id: Option<String>,
    candles_processed: usize, // 워밍업용 캔들 카운터

    // MagicSplit 상태
    split_states: Vec<SplitLevelState>,
    split_entry_date: Option<String>,
    split_group_id: Option<String>,

    // InfinityBot 상태
    infinity_state: InfinityBotState,
    infinity_group_id: Option<String>,
    last_entry_price: Option<Decimal>,
}

impl DcaStrategy {
    pub fn new() -> Self {
        Self {
            initial_variant: DcaVariant::Grid, // 기본값
            config: None,
            context: None,
            initialized: false,
            grid_levels: Vec::new(),
            grid_base_price: Decimal::ZERO,
            grid_group_id: None,
            candles_processed: 0, // 워밍업용 캔들 카운터 초기화
            split_states: Vec::new(),
            split_entry_date: None,
            split_group_id: None,
            infinity_state: InfinityBotState::default(),
            infinity_group_id: None,
            last_entry_price: None,
        }
    }

    /// 그리드 전략 팩토리
    pub fn grid() -> Self {
        Self {
            initial_variant: DcaVariant::Grid,
            ..Self::new()
        }
    }

    /// 매직 분할 전략 팩토리
    pub fn magic_split() -> Self {
        Self {
            initial_variant: DcaVariant::MagicSplit,
            ..Self::new()
        }
    }

    /// 인피니티봇 전략 팩토리
    pub fn infinity_bot() -> Self {
        Self {
            initial_variant: DcaVariant::InfinityBot,
            ..Self::new()
        }
    }

    // ========================================================================
    // StrategyContext 헬퍼
    // ========================================================================

    fn get_klines(&self) -> Option<Vec<Kline>> {
        let config = self.config.as_ref()?;
        let ctx = self.context.as_ref()?;
        let ctx_lock = ctx.try_read().ok()?;
        let klines = ctx_lock.get_klines(&config.ticker, Timeframe::D1);
        if klines.is_empty() {
            return None;
        }
        Some(klines.to_vec())
    }


    // ========================================================================
    // 공통 헬퍼
    // ========================================================================

    fn can_enter(&self) -> bool {
        let Some(config) = self.config.as_ref() else {
            return false;
        };

        // 모든 DCA 변형은 단일 티커 전략
        // GlobalScore는 스크리닝용이므로 체크 불필요
        // RouteState는 시장 상황 판단에 활용 (Grid는 순수 가격 기반이므로 스킵)

        // Grid 전략: 순수 가격 기반 → RouteState도 체크 불필요
        if config.variant == DcaVariant::Grid {
            return true;
        }

        // MagicSplit, InfinityBot: RouteState만 체크 (시장 과열 시 진입 제한)
        let Some(ctx) = self.context.as_ref() else {
            return true;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            return true;
        };

        if let Some(route_state) = ctx_lock.get_route_state(&config.ticker) {
            match route_state {
                RouteState::Overheat => {
                    debug!(
                        ticker = %config.ticker,
                        route_state = ?route_state,
                        "시장 과열 - 진입 제한"
                    );
                    return false;
                }
                // Wait, Neutral, Armed, Attack: 진입 허용
                _ => {}
            }
        }

        true
    }

    fn get_adjusted_strength(&self, base_strength: f64) -> f64 {
        let Some(config) = self.config.as_ref() else {
            return base_strength;
        };

        let Some(ctx) = self.context.as_ref() else {
            return base_strength;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            return base_strength;
        };

        if let Some(score) = ctx_lock.get_global_score(&config.ticker) {
            adjust_strength_by_score(base_strength, Some(score.overall_score))
        } else {
            base_strength
        }
    }

    fn get_regime(&self) -> Option<MarketRegime> {
        let config = self.config.as_ref()?;
        let ctx = self.context.as_ref()?;
        let ctx_lock = ctx.try_read().ok()?;
        ctx_lock.get_market_regime(&config.ticker).copied()
    }

    fn get_route_state(&self) -> Option<RouteState> {
        let config = self.config.as_ref()?;
        let ctx = self.context.as_ref()?;
        let ctx_lock = ctx.try_read().ok()?;
        ctx_lock.get_route_state(&config.ticker).copied()
    }

    // ========================================================================
    // Grid 로직
    // ========================================================================

    fn initialize_grid(&mut self, base_price: Decimal) {
        let config = match &self.config {
            Some(c) => c,
            None => return,
        };

        self.grid_base_price = base_price;
        self.grid_levels.clear();

        // 새 그리드 세션 시작 - 고유 그룹 ID 생성
        self.grid_group_id = Some(format!(
            "grid_{}_{}",
            base_price.to_string().replace(".", "_"),
            chrono::Utc::now().timestamp_millis()
        ));

        let spacing = base_price * config.grid_spacing_pct / dec!(100);

        for i in 1..=config.grid_levels {
            let buy_price = base_price - spacing * Decimal::from(i as i32);
            let sell_price = base_price - spacing * Decimal::from(i as i32 - 1);

            self.grid_levels.push(GridLevel {
                buy_price,
                sell_price,
                state: GridLevelState::WaitingBuy,
            });
        }

        info!(
            base_price = %base_price,
            spacing_pct = %config.grid_spacing_pct,
            levels = config.grid_levels,
            "그리드 초기화"
        );
    }

    fn generate_grid_signals(&mut self, price: Decimal) -> Vec<Signal> {
        let config = match &self.config {
            Some(c) => c.clone(),
            None => return vec![],
        };

        // 캔들 카운터 증가
        self.candles_processed += 1;

        // 그리드 초기화
        if self.grid_base_price == Decimal::ZERO {
            self.initialize_grid(price);

            // 워밍업 기간 중이면 그리드만 초기화하고 신호 발생 안함
            if self.candles_processed <= config.warmup_candles {
                info!(
                    candles_processed = self.candles_processed,
                    warmup_candles = config.warmup_candles,
                    current_price = %price,
                    "Grid 워밍업 기간 - 그리드 초기화만 수행"
                );
                return vec![];
            }
        } else {
            // 동적 그리드 재설정 체크
            let spacing = self.grid_base_price * config.grid_spacing_pct / dec!(100);
            let grid_upper = self.grid_base_price;
            let grid_lower = self.grid_base_price - spacing * Decimal::from(config.grid_levels as i32);
            let threshold = config.reset_threshold_pct / dec!(100);

            // 가격이 그리드 상단을 threshold% 이상 벗어났거나
            // 그리드 하단을 threshold% 이상 벗어났을 때 재설정
            let upper_breach = price > grid_upper * (Decimal::ONE + threshold);
            let lower_breach = price < grid_lower * (Decimal::ONE - threshold);

            if upper_breach || lower_breach {
                // 기존 포지션 수 카운트 (로깅용)
                let existing_positions = self.grid_levels.iter()
                    .filter(|l| l.state == GridLevelState::WaitingSell)
                    .count();

                info!(
                    current_price = %price,
                    grid_upper = %grid_upper,
                    grid_lower = %grid_lower,
                    threshold_pct = %config.reset_threshold_pct,
                    breach = if upper_breach { "상단" } else { "하단" },
                    existing_positions = existing_positions,
                    "그리드 범위 이탈 - 그리드만 재설정 (기존 포지션 유지)"
                );

                // 기존 포지션 정보 저장 (진입가격 목록)
                // 재설정 후에도 해당 가격 레벨에서 익절 가능하도록
                let held_entry_prices: Vec<Decimal> = self.grid_levels.iter()
                    .filter(|l| l.state == GridLevelState::WaitingSell)
                    .map(|l| l.buy_price)
                    .collect();

                // 새 그리드 초기화
                self.initialize_grid(price);

                // 기존 포지션의 진입가격에 가장 가까운 레벨을 WaitingSell로 복원
                // 이렇게 하면 기존 포지션도 익절 조건 도달 시 청산 가능
                for entry_price in held_entry_prices {
                    // 새 그리드에서 진입가격에 가장 가까운 레벨 찾기
                    if let Some((idx, _)) = self.grid_levels.iter().enumerate()
                        .filter(|(_, l)| l.state == GridLevelState::WaitingBuy)
                        .min_by_key(|(_, l)| {
                            let diff = if l.buy_price > entry_price {
                                l.buy_price - entry_price
                            } else {
                                entry_price - l.buy_price
                            };
                            diff
                        })
                    {
                        // 해당 레벨을 WaitingSell로 변경 (진입가격은 원래 값 유지)
                        self.grid_levels[idx].state = GridLevelState::WaitingSell;
                        // 익절가는 원래 진입가 기준으로 계산
                        let spacing = self.grid_base_price * config.grid_spacing_pct / dec!(100);
                        self.grid_levels[idx].buy_price = entry_price;
                        self.grid_levels[idx].sell_price = entry_price + spacing;
                    }
                }

                info!(
                    new_base_price = %price,
                    restored_positions = self.grid_levels.iter().filter(|l| l.state == GridLevelState::WaitingSell).count(),
                    "그리드 재설정 완료 - 기존 포지션 복원"
                );
            }
        }

        // 워밍업 기간 동안 거래 신호 발생 안함 (그리드 재설정/청산은 허용)
        if self.candles_processed <= config.warmup_candles {
            info!(
                candles_processed = self.candles_processed,
                warmup_candles = config.warmup_candles,
                current_price = %price,
                "Grid 워밍업 기간 - 거래 신호 없음"
            );
            return vec![];
        }

        let mut signals = vec![];
        // (level_index, side, price, exit_reason) - exit_reason: None=entry, Some("take_profit"), Some("stop_loss")
        let mut updates: Vec<(usize, Side, Decimal, Option<&'static str>)> = vec![];

        for (i, level) in self.grid_levels.iter().enumerate() {
            match level.state {
                GridLevelState::WaitingBuy => {
                    if price <= level.buy_price && self.can_enter() {
                        updates.push((i, Side::Buy, level.buy_price, None));
                    }
                }
                GridLevelState::WaitingSell => {
                    // 1. 익절 조건
                    if price >= level.sell_price {
                        updates.push((i, Side::Sell, level.sell_price, Some("take_profit")));
                    }
                    // 2. 손절 조건 (exit_config 활용)
                    else if let Some(sl_pct) = config.exit_config.stop_loss() {
                        if level.buy_price > Decimal::ZERO {
                            let loss_pct = (level.buy_price - price) / level.buy_price * dec!(100);
                            if loss_pct >= sl_pct {
                                info!(
                                    ticker = %config.ticker,
                                    level = i,
                                    entry_price = %level.buy_price,
                                    current_price = %price,
                                    loss_pct = %loss_pct,
                                    stop_loss_pct = %sl_pct,
                                    "Grid 손절 조건 충족"
                                );
                                updates.push((i, Side::Sell, level.buy_price, Some("stop_loss")));
                            }
                        }
                    }
                }
            }
        }

        for (i, side, level_price, exit_reason) in updates {
            let signal_type = if side == Side::Buy {
                SignalType::Entry
            } else {
                SignalType::Exit
            };

            let position_id = format!("{}_grid_L{}", config.ticker, i);
            let strength = self.get_adjusted_strength(0.7);

            let mut signal = Signal::new("dca", config.ticker.clone(), side, signal_type)
                .with_position_id(position_id)
                .with_strength(strength)
                .with_prices(Some(price), None, None)
                .with_metadata("variant", json!("grid"))
                .with_metadata("grid_level", json!(level_price.to_string()))
                .with_metadata("grid_level_index", json!(i));

            // 청산 사유 추가
            if let Some(reason) = exit_reason {
                signal = signal.with_metadata("exit_reason", json!(reason));
            }

            if let Some(ref group_id) = self.grid_group_id {
                signal = signal.with_group_id(group_id.clone());
            }
            signals.push(signal);

            // 상태 전환
            match side {
                Side::Buy => {
                    self.grid_levels[i].state = GridLevelState::WaitingSell;
                }
                Side::Sell => {
                    self.grid_levels[i].state = GridLevelState::WaitingBuy;
                }
            }
        }

        signals
    }

    // ========================================================================
    // MagicSplit 로직
    // ========================================================================

    fn generate_split_signals(&mut self, price: Decimal, timestamp: DateTime<Utc>) -> Vec<Signal> {
        let config = match &self.config {
            Some(c) => c.clone(),
            None => return vec![],
        };

        // 상태 초기화
        if self.split_states.is_empty() {
            self.split_states = vec![SplitLevelState::default(); config.split_levels.len()];
            self.split_group_id = Some(format!(
                "split_{}_{}",
                config.ticker,
                chrono::Utc::now().timestamp_millis()
            ));
        }

        // 당일 체크
        let today = format!("{}", timestamp.format("%Y-%m-%d"));
        if let Some(ref entry_date) = self.split_entry_date {
            if entry_date == &today && self.all_split_sold() {
                return vec![];
            }
        }

        if !self.can_enter() {
            return vec![];
        }

        let mut actions: Vec<(usize, SplitAction, Decimal)> = vec![];

        for (i, level) in config.split_levels.iter().enumerate() {
            let is_bought = self.split_states[i].is_bought;
            let entry_price = self.split_states[i].entry_price;

            if !is_bought {
                let should_buy = if i == 0 {
                    true
                } else {
                    let prev_is_bought = self.split_states[i - 1].is_bought;
                    let prev_entry_price = self.split_states[i - 1].entry_price;
                    if prev_is_bought && prev_entry_price > Decimal::ZERO {
                        let loss_rate = (price - prev_entry_price) / prev_entry_price * dec!(100);
                        loss_rate <= level.trigger_rate
                    } else {
                        false
                    }
                };

                if should_buy {
                    actions.push((i, SplitAction::Buy, level.amount));
                }
            } else if entry_price > Decimal::ZERO {
                let profit_rate = (price - entry_price) / entry_price * dec!(100);
                // 1. 익절 조건
                if profit_rate >= level.target_rate {
                    actions.push((i, SplitAction::TakeProfit, profit_rate));
                }
                // 2. 손절 조건 (exit_config 활용)
                else if let Some(sl_pct) = config.exit_config.stop_loss() {
                    let loss_rate = -profit_rate; // profit_rate가 음수이므로 부호 변환
                    if loss_rate >= sl_pct {
                        info!(
                            ticker = %config.ticker,
                            level = i,
                            entry_price = %entry_price,
                            current_price = %price,
                            loss_rate = %loss_rate,
                            stop_loss_pct = %sl_pct,
                            "MagicSplit 손절 조건 충족"
                        );
                        actions.push((i, SplitAction::StopLoss, loss_rate));
                    }
                }
            }
        }

        let mut signals = vec![];
        for (i, action, value) in actions {
            let position_id = format!("{}_split_L{}", config.ticker, i);

            match action {
                SplitAction::Buy => {
                    let strength = self.get_adjusted_strength(0.8);
                    let mut signal = Signal::new("dca", config.ticker.clone(), Side::Buy, SignalType::Entry)
                        .with_position_id(position_id)
                        .with_strength(strength)
                        .with_prices(Some(price), None, None)
                        .with_metadata("variant", json!("split"))
                        .with_metadata("level", json!(i + 1))
                        .with_metadata("amount", json!(value.to_string()));

                    if let Some(ref group_id) = self.split_group_id {
                        signal = signal.with_group_id(group_id.clone());
                    }
                    signals.push(signal);

                    self.split_states[i].is_bought = true;
                    self.split_states[i].entry_price = price;
                    self.split_states[i].quantity = value / price;

                    if i == 0 {
                        self.split_entry_date = Some(today.clone());
                    }
                }
                SplitAction::TakeProfit => {
                    let mut signal = Signal::new("dca", config.ticker.clone(), Side::Sell, SignalType::Exit)
                        .with_position_id(position_id)
                        .with_strength(0.9)
                        .with_prices(Some(price), None, None)
                        .with_metadata("variant", json!("split"))
                        .with_metadata("level", json!(i + 1))
                        .with_metadata("exit_reason", json!("take_profit"))
                        .with_metadata("profit_rate", json!(value.to_string()));

                    if let Some(ref group_id) = self.split_group_id {
                        signal = signal.with_group_id(group_id.clone());
                    }
                    signals.push(signal);

                    self.split_states[i].is_bought = false;
                    self.split_states[i].entry_price = Decimal::ZERO;
                    self.split_states[i].quantity = Decimal::ZERO;
                }
                SplitAction::StopLoss => {
                    let mut signal = Signal::new("dca", config.ticker.clone(), Side::Sell, SignalType::Exit)
                        .with_position_id(position_id)
                        .with_strength(1.0) // 손절은 우선순위 높음
                        .with_prices(Some(price), None, None)
                        .with_metadata("variant", json!("split"))
                        .with_metadata("level", json!(i + 1))
                        .with_metadata("exit_reason", json!("stop_loss"))
                        .with_metadata("loss_rate", json!(value.to_string()));

                    if let Some(ref group_id) = self.split_group_id {
                        signal = signal.with_group_id(group_id.clone());
                    }
                    signals.push(signal);

                    self.split_states[i].is_bought = false;
                    self.split_states[i].entry_price = Decimal::ZERO;
                    self.split_states[i].quantity = Decimal::ZERO;
                }
            }
        }

        signals
    }

    fn all_split_sold(&self) -> bool {
        self.split_states.iter().all(|s| !s.is_bought)
    }

    // ========================================================================
    // InfinityBot 로직
    // ========================================================================

    fn calculate_ma(&self) -> Option<Decimal> {
        let config = self.config.as_ref()?;
        let klines = self.get_klines()?;

        if klines.len() < config.ma_period {
            return None;
        }

        let sum: Decimal = klines
            .iter()
            .rev()
            .take(config.ma_period)
            .map(|k| k.close)
            .sum();
        Some(sum / Decimal::from(config.ma_period))
    }

    fn is_above_ma(&self, price: Decimal) -> bool {
        // MA 계산 불가 시 true 반환 (데이터 없으면 진입 허용)
        // InfinityBot은 DCA 전략이므로 기본적으로 진입을 허용해야 함
        self.calculate_ma().map(|ma| price > ma).unwrap_or(true)
    }

    fn has_positive_momentum(&self) -> bool {
        let klines = match self.get_klines() {
            Some(k) => k,
            // klines 없으면 true 반환 (DCA 전략이므로 기본 진입 허용)
            None => return true,
        };

        if klines.len() < 6 {
            return true; // 데이터 부족 시에도 진입 허용
        }

        let len = klines.len();
        let current = klines[len - 1].close;
        let past = klines[len - 6].close;

        if past.is_zero() {
            return true;
        }

        current > past
    }

    fn can_enter_infinity(&self, price: Decimal) -> bool {
        // RouteState 우선 체크 - Overheat이면 진입 금지
        if let Some(route) = self.get_route_state() {
            if route == RouteState::Overheat {
                return false; // 과열 상태에서는 진입 금지
            }
            if route == RouteState::Attack || route == RouteState::Armed {
                return true; // 공격/준비 상태에서는 항상 허용
            }
        }

        let regime = self.get_regime();

        match regime {
            // 상승장: 항상 진입 가능
            Some(MarketRegime::StrongUptrend) | Some(MarketRegime::BottomBounce) => true,
            // 조정장: MA 상단이면 진입
            Some(MarketRegime::Correction) => self.is_above_ma(price),
            // 박스장: 모멘텀 긍정이면 진입
            Some(MarketRegime::Sideways) => self.has_positive_momentum(),
            // 하락장: InfinityBot은 DCA 전략이므로 진입 허용
            // 단, 라운드 수와 stop_loss_pct가 리스크 관리
            Some(MarketRegime::Downtrend) => true,
            // 레짐 없음: MA 기준
            None => self.is_above_ma(price),
        }
    }

    fn can_add_position(&self, current_price: Decimal) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        if self.infinity_state.current_round >= config.max_rounds {
            return false;
        }

        if let Some(last_price) = self.last_entry_price {
            if last_price.is_zero() {
                return false;
            }
            let drop_pct = (last_price - current_price) / last_price * dec!(100);
            drop_pct >= config.dip_trigger_pct
        } else {
            true
        }
    }

    fn should_take_profit(&self, current_price: Decimal) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        if let Some(return_pct) = self.infinity_state.current_return(current_price) {
            return_pct >= config.take_profit_pct
        } else {
            false
        }
    }

    fn round_amount(&self) -> Decimal {
        let config = match &self.config {
            Some(c) => c,
            None => return Decimal::ZERO,
        };

        config.total_amount * config.round_pct / dec!(100)
    }

    fn generate_infinity_signals(&mut self, price: Decimal, timestamp: i64) -> Vec<Signal> {
        let config = match &self.config {
            Some(c) => c.clone(),
            None => return vec![],
        };

        // 그룹 ID 초기화
        if self.infinity_group_id.is_none() {
            self.infinity_group_id = Some(format!(
                "infinity_{}_{}",
                config.ticker,
                chrono::Utc::now().timestamp_millis()
            ));
        }

        let mut signals = vec![];

        // 1. 익절 조건 확인
        if !self.infinity_state.total_quantity.is_zero() && self.should_take_profit(price) {
            let return_pct = self.infinity_state.current_return(price).unwrap_or(Decimal::ZERO);

            info!(
                ticker = %config.ticker,
                return_pct = %return_pct,
                rounds = self.infinity_state.current_round,
                "익절 조건 충족"
            );

            // 모든 라운드 청산 - 각 라운드별로 개별 청산 신호
            let total_rounds = self.infinity_state.current_round;
            for round_info in &self.infinity_state.rounds {
                let position_id = format!("{}_infinity_R{}", config.ticker, round_info.round);
                let mut signal = Signal::new("dca", config.ticker.clone(), Side::Sell, SignalType::Exit)
                    .with_position_id(position_id)
                    .with_strength(1.0)
                    .with_metadata("action", json!("take_profit"))
                    .with_metadata("return_pct", json!(return_pct.to_string()))
                    .with_metadata("round", json!(round_info.round))
                    .with_metadata("rounds", json!(total_rounds));

                if let Some(ref group_id) = self.infinity_group_id {
                    signal = signal.with_group_id(group_id.clone());
                }
                signals.push(signal);
            }

            // 상태 초기화
            self.infinity_state = InfinityBotState::default();
            self.last_entry_price = None;
            self.infinity_group_id = None;

            return signals;
        }

        // 1-2. 손절 조건 확인 (exit_config 활용)
        if !self.infinity_state.total_quantity.is_zero() {
            if let Some(sl_pct) = config.exit_config.stop_loss() {
                if let Some(return_pct) = self.infinity_state.current_return(price) {
                    if return_pct <= -sl_pct {
                        info!(
                            ticker = %config.ticker,
                            return_pct = %return_pct,
                            stop_loss_pct = %sl_pct,
                            rounds = self.infinity_state.current_round,
                            "InfinityBot 손절 조건 충족"
                        );

                        // 모든 라운드 손절 - 각 라운드별로 개별 청산 신호
                        let total_rounds = self.infinity_state.current_round;
                        for round_info in &self.infinity_state.rounds {
                            let position_id = format!("{}_infinity_R{}", config.ticker, round_info.round);
                            let mut signal = Signal::new("dca", config.ticker.clone(), Side::Sell, SignalType::Exit)
                                .with_position_id(position_id)
                                .with_strength(1.0)
                                .with_metadata("action", json!("stop_loss"))
                                .with_metadata("return_pct", json!(return_pct.to_string()))
                                .with_metadata("round", json!(round_info.round))
                                .with_metadata("rounds", json!(total_rounds));

                            if let Some(ref group_id) = self.infinity_group_id {
                                signal = signal.with_group_id(group_id.clone());
                            }
                            signals.push(signal);
                        }

                        // 상태 초기화
                        self.infinity_state = InfinityBotState::default();
                        self.last_entry_price = None;
                        self.infinity_group_id = None;

                        return signals;
                    }
                }
            }
        }

        // 2. 진입/물타기 조건 (단일 티커 전략 - GlobalScore 필터 사용 안함)
        if self.can_add_position(price) && self.can_enter_infinity(price) {
            let round = self.infinity_state.current_round + 1;
            let amount = self.round_amount();
            let quantity = if price.is_zero() {
                Decimal::ZERO
            } else {
                amount / price
            };

            // 라운드별 고유 포지션 ID
            let position_id = format!("{}_infinity_R{}", config.ticker, round);

            // 상태 업데이트
            self.infinity_state.current_round = round;
            self.infinity_state.rounds.push(RoundInfo {
                round,
                entry_price: price,
                quantity,
                timestamp,
            });
            self.infinity_state.total_quantity += quantity;
            self.infinity_state.invested_amount += amount;
            self.infinity_state.avg_price = self.infinity_state.calculate_avg_price();
            self.last_entry_price = Some(price);

            info!(
                ticker = %config.ticker,
                round,
                price = %price,
                quantity = %quantity,
                avg_price = ?self.infinity_state.avg_price,
                "라운드 진입"
            );

            let strength = self.get_adjusted_strength(1.0);
            let mut signal = Signal::new("dca", config.ticker.clone(), Side::Buy, SignalType::Entry)
                .with_position_id(position_id)
                .with_strength(strength)
                .with_metadata("action", json!("round_entry"))
                .with_metadata("round", json!(round))
                .with_metadata("quantity", json!(quantity.to_string()))
                .with_metadata("avg_price", json!(self.infinity_state.avg_price.map(|d| d.to_string())));

            if let Some(ref group_id) = self.infinity_group_id {
                signal = signal.with_group_id(group_id.clone());
            }
            signals.push(signal);
        }

        signals
    }
}

impl Default for DcaStrategy {
    fn default() -> Self {
        Self::new()
    }
}

// ================================================================================================
// Strategy Trait 구현
// ================================================================================================

#[async_trait]
impl Strategy for DcaStrategy {
    fn name(&self) -> &str {
        // 초기화 전에도 올바른 이름 반환을 위해 initial_variant 사용
        match self.initial_variant {
            DcaVariant::Grid => "DCA-Grid",
            DcaVariant::MagicSplit => "DCA-MagicSplit",
            DcaVariant::InfinityBot => "DCA-InfinityBot",
        }
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "분할매수 전략 (Grid, MagicSplit, InfinityBot)"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let variant = config
            .get("variant")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "grid" | "Grid" => Some(DcaVariant::Grid),
                "magic_split" | "MagicSplit" => Some(DcaVariant::MagicSplit),
                "infinity_bot" | "InfinityBot" => Some(DcaVariant::InfinityBot),
                _ => None,
            })
            .unwrap_or_default();

        let dca_config: DcaConfig = match variant {
            DcaVariant::Grid => {
                let cfg: GridTradingConfig = serde_json::from_value(config)?;
                cfg.into()
            }
            DcaVariant::MagicSplit => {
                let cfg: MagicSplitConfig = serde_json::from_value(config)?;
                cfg.into()
            }
            DcaVariant::InfinityBot => {
                let cfg: InfinityBotConfig = serde_json::from_value(config)?;
                cfg.into()
            }
        };

        info!(
            variant = ?dca_config.variant,
            ticker = %dca_config.ticker,
            "[DCA] 전략 초기화"
        );

        // 변형별 초기화
        match dca_config.variant {
            DcaVariant::Grid => {
                self.grid_levels = Vec::with_capacity(dca_config.grid_levels);
            }
            DcaVariant::MagicSplit => {
                self.split_states = vec![SplitLevelState::default(); dca_config.split_levels.len()];
            }
            DcaVariant::InfinityBot => {
                self.infinity_state = InfinityBotState::default();
            }
        }

        self.config = Some(dca_config);
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

        let (ticker, variant) = match self.config.as_ref() {
            Some(config) => (config.ticker.clone(), config.variant),
            None => return Ok(vec![]),
        };

        if data.ticker != ticker {
            return Ok(vec![]);
        }

        let price = match &data.data {
            MarketDataType::Kline(kline) => kline.close,
            MarketDataType::Ticker(ticker_data) => ticker_data.last,
            MarketDataType::Trade(trade) => trade.price,
            _ => return Ok(vec![]),
        };

        let signals = match variant {
            DcaVariant::Grid => self.generate_grid_signals(price),
            DcaVariant::MagicSplit => self.generate_split_signals(price, data.timestamp),
            DcaVariant::InfinityBot => self.generate_infinity_signals(price, data.timestamp.timestamp()),
        };

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "[DCA] 주문 체결: {:?} {} @ {:?}",
            order.side, order.quantity, order.average_fill_price
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            "[DCA] 포지션 업데이트: {} = {}",
            position.ticker, position.quantity
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("[DCA] 전략 종료");
        Ok(())
    }

    fn get_state(&self) -> Value {
        // 기본 상태
        let mut state = json!({
            "name": self.name(),
            "version": self.version(),
            "variant": self.config.as_ref().map(|c| format!("{:?}", c.variant)),
            "initialized": self.initialized,
            "grid_levels": self.grid_levels.len(),
            "split_states": self.split_states.len(),
            "infinity_rounds": self.infinity_state.current_round,
        });

        // 모든 variant에서 config.ticker 반환 (시뮬레이션에서 사용)
        if let Some(config) = &self.config {
            // 기본 config 정보 (ticker는 모든 variant에서 필요)
            state["config"] = json!({
                "ticker": config.ticker,
                "variant": format!("{:?}", config.variant),
            });

            // Grid variant일 때 상세 상태 추가
            if config.variant == DcaVariant::Grid {
                let grid_levels_json: Vec<Value> = self.grid_levels.iter().map(|level| {
                    json!({
                        "buy_price": level.buy_price.to_string(),
                        "sell_price": level.sell_price.to_string(),
                        "state": format!("{:?}", level.state),
                    })
                }).collect();

                state["state"] = json!({
                    "grid": {
                        "base_price": self.grid_base_price.to_string(),
                        "group_id": self.grid_group_id.clone(),
                        "levels": grid_levels_json,
                    }
                });
            }

            // InfinityBot variant일 때 상세 상태 추가
            if config.variant == DcaVariant::InfinityBot {
                // klines_count 계산 (context에서)
                let klines_count = self.context.as_ref().and_then(|ctx| {
                    ctx.try_read().ok().map(|c| {
                        c.get_klines(&config.ticker, Timeframe::D1).len() as i64
                    })
                }).unwrap_or(0);

                // 마지막 진입가 (rounds 배열에서)
                let last_entry_price = self.infinity_state.rounds.last()
                    .map(|r| r.entry_price);

                state["klines_count"] = json!(klines_count);
                state["config"] = json!({
                    "ticker": config.ticker,
                    "max_rounds": config.max_rounds,
                    "total_amount": config.total_amount.to_string(),
                    "round_pct": config.round_pct.to_string(),
                    "dip_trigger_pct": config.dip_trigger_pct.to_string(),
                    "take_profit_pct": config.take_profit_pct.to_string(),
                    "ma_period": config.ma_period,
                });
                state["state"] = json!({
                    "current_round": self.infinity_state.current_round,
                    "avg_price": match &self.infinity_state.avg_price {
                        Some(p) if !p.is_zero() => json!(p.to_string()),
                        _ => Value::Null,
                    },
                    "total_quantity": self.infinity_state.total_quantity.to_string(),
                    "last_entry_price": match last_entry_price {
                        Some(p) if !p.is_zero() => json!(p.to_string()),
                        _ => Value::Null,
                    },
                });
            }
        }

        state
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("[DCA] StrategyContext 주입 완료");
    }
}

// ================================================================================================
// 전략 레지스트리 등록
// ================================================================================================

use crate::register_strategy;

// 그리드 전략
register_strategy! {
    id: "grid",
    aliases: ["grid_trading", "grid_strategy"],
    name: "그리드 트레이딩",
    description: "일정 가격 간격으로 매수/매도 주문 배치",
    timeframe: "1m",
    tickers: [],
    category: Realtime,
    markets: [Crypto, Stock],
    factory: DcaStrategy::grid,
    config: GridTradingConfig
}

// 매직 분할 전략
register_strategy! {
    id: "magic_split",
    aliases: ["split_entry", "pyramid"],
    name: "매직 분할매수",
    description: "가격 구간별 분할 매수 및 목표 수익 시 청산",
    timeframe: "1d",
    tickers: [],
    category: Daily,
    markets: [Crypto, Stock],
    factory: DcaStrategy::magic_split,
    config: MagicSplitConfig
}

// 인피니티봇 전략
register_strategy! {
    id: "infinity_bot",
    aliases: ["무한매수봇", "infinity"],
    name: "인피니티봇",
    description: "피라미드 물타기 + MarketRegime 기반 진입 전략",
    timeframe: "1d",
    tickers: ["005930"],
    category: Daily,
    markets: [Stock],
    factory: DcaStrategy::infinity_bot,
    config: InfinityBotConfig
}

// ================================================================================================
// 테스트
// ================================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_config() {
        let config = GridTradingConfig {
            ticker: "BTCUSDT".to_string(),
            amount: dec!(100000),
            spacing_pct: dec!(1),
            levels: 5,
            use_atr: false,
            atr_period: 14,
            exit_config: ExitConfig::for_grid_trading(),
            max_positions: 10,
            reset_threshold_pct: dec!(10),
            warmup_candles: 5,
        };
        let dca_config: DcaConfig = config.into();
        assert_eq!(dca_config.variant, DcaVariant::Grid);
        assert_eq!(dca_config.grid_levels, 5);
    }

    #[test]
    fn test_magic_split_config() {
        let config = MagicSplitConfig {
            ticker: "005930".to_string(),
            levels: default_split_levels(),
            exit_config: ExitConfig::for_grid_trading(),
            max_positions: 5,
        };
        let dca_config: DcaConfig = config.into();
        assert_eq!(dca_config.variant, DcaVariant::MagicSplit);
        assert_eq!(dca_config.split_levels.len(), 5);
    }

    #[test]
    fn test_infinity_bot_config() {
        let config = InfinityBotConfig {
            ticker: "005930".to_string(),
            total_amount: dec!(10000000),
            max_rounds: 50,
            round_pct: dec!(2),
            dip_trigger_pct: dec!(2),
            take_profit_pct: dec!(3),
            ma_period: 20,
            min_global_score: dec!(50),
            exit_config: ExitConfig::for_grid_trading(),
        };
        let dca_config: DcaConfig = config.into();
        assert_eq!(dca_config.variant, DcaVariant::InfinityBot);
        assert_eq!(dca_config.max_rounds, 50);
    }

    #[test]
    fn test_infinity_state() {
        let mut state = InfinityBotState::default();
        state.invested_amount = dec!(10000);
        state.total_quantity = dec!(10);
        state.avg_price = state.calculate_avg_price();
        assert_eq!(state.avg_price, Some(dec!(1000)));

        let ret = state.current_return(dec!(1100));
        assert_eq!(ret, Some(dec!(10)));
    }
}
