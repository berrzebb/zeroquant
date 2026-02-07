//! 평균회귀 전략 그룹
//!
//! 가격이 평균으로 돌아올 것을 기대하는 전략들입니다.
//!
//! # 지원 변형
//!
//! - `Rsi`: RSI 과매도/과매수 기반 평균회귀
//! - `Bollinger`: 볼린저 밴드 이탈 후 복귀
//!
//! # 공통 로직
//!
//! - `RouteState`: 진입 가능 여부 판단 (Armed, Attack만 허용)
//! - `GlobalScore`: 종목 품질 필터링
//! - 손절/익절: 설정된 비율로 자동 청산
//!
//! # Grid/MagicSplit 분리 안내
//!
//! Grid Trading, MagicSplit, InfinityBot은 `dca.rs`로 이동했습니다.
//! 이들은 스프레드 기반 전략으로, 레벨별 독립 포지션 관리가 필요합니다.

use crate::strategies::common::{adjust_strength_by_score, ExitConfig};
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::domain::{RouteState, StrategyContext};
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, SignalType};
use trader_strategy_macro::StrategyConfig;

// ================================================================================================
// 설정 타입
// ================================================================================================

/// 전략 변형
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MeanReversionVariant {
    /// RSI 평균회귀
    #[default]
    Rsi,
    /// 볼린저 밴드
    Bollinger,
}

// ================================================================================================
// 전략별 UI Config (SDUI용)
// ================================================================================================

/// RSI 평균회귀 전략 설정
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "rsi",
    name = "RSI 평균회귀",
    description = "RSI 과매수/과매도 구간에서 평균회귀 매매",
    category = "Intraday"
)]
pub struct RsiConfig {
    /// 거래 티커
    #[serde(default = "default_ticker")]
    #[schema(label = "거래 종목", field_type = "symbol", default = "005930", section = "asset")]
    pub ticker: String,

    /// 거래 금액
    #[serde(default = "default_amount")]
    #[schema(label = "거래 금액", field_type = "number", min = 10000, max = 100000000, default = 1000000, section = "asset")]
    pub amount: Decimal,

    /// RSI 기간
    #[serde(default = "default_rsi_period")]
    #[schema(label = "RSI 기간", field_type = "integer", min = 2, max = 100, default = 7, section = "indicator")]
    pub rsi_period: usize,

    /// 과매도 임계값
    #[serde(default = "default_oversold")]
    #[schema(label = "과매도 임계값", field_type = "number", min = 0, max = 50, default = 45, section = "indicator")]
    pub oversold: Decimal,

    /// 과매수 임계값
    #[serde(default = "default_overbought")]
    #[schema(label = "과매수 임계값", field_type = "number", min = 50, max = 100, default = 55, section = "indicator")]
    pub overbought: Decimal,

    /// 청산 설정
    #[serde(default = "ExitConfig::for_mean_reversion")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,

    /// 최대 포지션 수
    #[serde(default = "default_max_positions")]
    #[schema(label = "최대 포지션 수", field_type = "integer", min = 1, max = 10, default = 3, section = "filter")]
    pub max_positions: usize,

    /// 최소 GlobalScore
    #[serde(default = "default_min_score")]
    #[schema(label = "최소 GlobalScore", field_type = "number", min = 0, max = 100, default = 0, section = "filter")]
    pub min_global_score: Decimal,
}

/// 볼린저 밴드 전략 설정
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "bollinger",
    name = "볼린저 밴드",
    description = "볼린저 밴드 상/하단 터치 시 평균회귀 매매",
    category = "Intraday"
)]
pub struct BollingerConfig {
    /// 거래 티커
    #[serde(default = "default_ticker")]
    #[schema(label = "거래 종목", field_type = "symbol", default = "005930", section = "asset")]
    pub ticker: String,

    /// 거래 금액
    #[serde(default = "default_amount")]
    #[schema(label = "거래 금액", field_type = "number", min = 10000, max = 100000000, default = 1000000, section = "asset")]
    pub amount: Decimal,

    /// 볼린저 밴드 기간
    #[serde(default = "default_bb_period")]
    #[schema(label = "기간", field_type = "integer", min = 5, max = 100, default = 10, section = "indicator")]
    pub period: usize,

    /// 표준편차 배수
    #[serde(default = "default_std_multiplier")]
    #[schema(label = "표준편차 배수", field_type = "number", min = 0.5, max = 5, default = 1.5, section = "indicator")]
    pub std_multiplier: Decimal,

    /// RSI 확인 사용 여부
    #[serde(default = "default_false")]
    #[schema(label = "RSI 확인 사용", field_type = "boolean", default = false, section = "indicator")]
    pub use_rsi_confirmation: bool,

    /// 최소 밴드폭 (%)
    #[serde(default = "default_min_bandwidth")]
    #[schema(label = "최소 밴드폭 (%)", field_type = "number", min = 0, max = 10, default = 0, section = "indicator")]
    pub min_bandwidth_pct: Decimal,

    /// 청산 설정
    #[serde(default = "ExitConfig::for_mean_reversion")]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,

    /// 최대 포지션 수
    #[serde(default = "default_max_positions")]
    #[schema(label = "최대 포지션 수", field_type = "integer", min = 1, max = 10, default = 3, section = "filter")]
    pub max_positions: usize,

    /// 최소 GlobalScore
    #[serde(default = "default_min_score")]
    #[schema(label = "최소 GlobalScore", field_type = "number", min = 0, max = 100, default = 0, section = "filter")]
    pub min_global_score: Decimal,
}

// ================================================================================================
// 기본값 함수
// ================================================================================================

fn default_ticker() -> String {
    "005930".to_string()
}

fn default_amount() -> Decimal {
    dec!(100000)
}

fn default_rsi_period() -> usize {
    14
}

fn default_oversold() -> Decimal {
    dec!(30)
}

fn default_overbought() -> Decimal {
    dec!(70)
}

fn default_bb_period() -> usize {
    20
}

fn default_std_multiplier() -> Decimal {
    dec!(2)
}

fn default_false() -> bool {
    false
}

fn default_min_bandwidth() -> Decimal {
    dec!(1)
}

fn default_max_positions() -> usize {
    1
}

fn default_min_score() -> Decimal {
    dec!(50)
}

fn default_cooldown() -> usize {
    5
}

// ================================================================================================
// 내부 설정 타입
// ================================================================================================

/// 평균회귀 내부 통합 설정
#[derive(Debug, Clone)]
pub struct MeanReversionConfig {
    pub variant: MeanReversionVariant,
    pub ticker: String,
    pub amount: Decimal,
    pub exit_config: ExitConfig,
    pub max_positions: usize,
    pub min_global_score: Decimal,
    pub cooldown_candles: usize,

    // RSI 전용
    pub rsi_period: usize,
    pub oversold: Decimal,
    pub overbought: Decimal,

    // Bollinger 전용
    pub bb_period: usize,
    pub std_multiplier: Decimal,
    pub use_rsi_confirmation: bool,
    pub min_bandwidth_pct: Decimal,
}

impl From<RsiConfig> for MeanReversionConfig {
    fn from(cfg: RsiConfig) -> Self {
        Self {
            variant: MeanReversionVariant::Rsi,
            ticker: cfg.ticker,
            amount: cfg.amount,
            exit_config: cfg.exit_config,
            max_positions: cfg.max_positions,
            min_global_score: cfg.min_global_score,
            cooldown_candles: default_cooldown(),
            rsi_period: cfg.rsi_period,
            oversold: cfg.oversold,
            overbought: cfg.overbought,
            bb_period: 0,
            std_multiplier: Decimal::ZERO,
            use_rsi_confirmation: false,
            min_bandwidth_pct: Decimal::ZERO,
        }
    }
}

impl From<BollingerConfig> for MeanReversionConfig {
    fn from(cfg: BollingerConfig) -> Self {
        Self {
            variant: MeanReversionVariant::Bollinger,
            ticker: cfg.ticker,
            amount: cfg.amount,
            exit_config: cfg.exit_config,
            max_positions: cfg.max_positions,
            min_global_score: cfg.min_global_score,
            cooldown_candles: default_cooldown(),
            rsi_period: 14, // RSI 확인용
            oversold: dec!(30),
            overbought: dec!(70),
            bb_period: cfg.period,
            std_multiplier: cfg.std_multiplier,
            use_rsi_confirmation: cfg.use_rsi_confirmation,
            min_bandwidth_pct: cfg.min_bandwidth_pct,
        }
    }
}

// ================================================================================================
// 내부 상태
// ================================================================================================

/// 포지션 상태
#[derive(Debug, Clone, Default)]
struct PositionState {
    side: Option<Side>,
    entry_price: Decimal,
    quantity: Decimal,
    #[allow(dead_code)]
    entry_time: Option<DateTime<Utc>>,
}

/// RSI 계산기
#[derive(Debug, Clone, Default)]
struct RsiCalculator {
    gains: VecDeque<Decimal>,
    losses: VecDeque<Decimal>,
    prev_close: Option<Decimal>,
    period: usize,
}

impl RsiCalculator {
    fn new(period: usize) -> Self {
        Self {
            gains: VecDeque::with_capacity(period + 1),
            losses: VecDeque::with_capacity(period + 1),
            prev_close: None,
            period,
        }
    }

    fn update(&mut self, close: Decimal) -> Option<Decimal> {
        if let Some(prev) = self.prev_close {
            let change = close - prev;
            let gain = if change > Decimal::ZERO { change } else { Decimal::ZERO };
            let loss = if change < Decimal::ZERO { -change } else { Decimal::ZERO };

            self.gains.push_back(gain);
            self.losses.push_back(loss);

            while self.gains.len() > self.period {
                self.gains.pop_front();
            }
            while self.losses.len() > self.period {
                self.losses.pop_front();
            }
        }
        self.prev_close = Some(close);

        if self.gains.len() < self.period {
            return None;
        }

        let avg_gain: Decimal = self.gains.iter().sum::<Decimal>() / Decimal::from(self.period);
        let avg_loss: Decimal = self.losses.iter().sum::<Decimal>() / Decimal::from(self.period);

        if avg_loss == Decimal::ZERO {
            return Some(dec!(100));
        }

        let rs = avg_gain / avg_loss;
        let rsi = dec!(100) - (dec!(100) / (dec!(1) + rs));
        Some(rsi)
    }
}

// ================================================================================================
// 전략 구현
// ================================================================================================

/// 평균회귀 전략
pub struct MeanReversionStrategy {
    /// 초기 variant (팩토리 메서드에서 설정, name() 반환용)
    initial_variant: MeanReversionVariant,
    config: Option<MeanReversionConfig>,
    context: Option<Arc<RwLock<StrategyContext>>>,

    // 공통 상태
    prices: VecDeque<Decimal>,
    position: PositionState,
    cooldown_counter: usize,
    initialized: bool,

    // RSI 상태
    rsi_calculator: RsiCalculator,
    prev_rsi: Option<Decimal>,
}

impl MeanReversionStrategy {
    pub fn new() -> Self {
        Self {
            initial_variant: MeanReversionVariant::Rsi, // 기본값
            config: None,
            context: None,
            prices: VecDeque::new(),
            position: PositionState::default(),
            cooldown_counter: 0,
            initialized: false,
            rsi_calculator: RsiCalculator::new(14),
            prev_rsi: None,
        }
    }

    /// RSI 평균회귀 전략 팩토리
    pub fn rsi() -> Self {
        Self {
            initial_variant: MeanReversionVariant::Rsi,
            ..Self::new()
        }
    }

    /// 볼린저 밴드 전략 팩토리
    pub fn bollinger() -> Self {
        Self {
            initial_variant: MeanReversionVariant::Bollinger,
            ..Self::new()
        }
    }

    // ========================================================================
    // StrategyContext 헬퍼
    // ========================================================================

    fn get_rsi_from_context(&self, ticker: &str) -> Option<Decimal> {
        let ctx = self.context.as_ref()?;
        let ctx_lock = ctx.try_read().ok()?;
        let features = ctx_lock.structural_features.get(ticker)?;
        Some(features.rsi)
    }

    fn get_bollinger_from_context(&self, ticker: &str) -> Option<(Decimal, Decimal, Decimal, Decimal)> {
        let ctx = self.context.as_ref()?;
        let ctx_lock = ctx.try_read().ok()?;
        let features = ctx_lock.structural_features.get(ticker)?;
        Some((features.bb_lower, features.bb_middle, features.bb_upper, features.bb_width))
    }

    // ========================================================================
    // 공통 헬퍼
    // ========================================================================

    fn can_enter(&self) -> bool {
        let Some(config) = self.config.as_ref() else {
            return false;
        };

        // 단일 티커 전략: GlobalScore는 스크리닝용이므로 체크 불필요
        // StructuralFeatures(RSI 등)는 generate_*_signals()에서 직접 활용
        // RouteState는 시장 상황 판단에 활용 (선택적)

        let Some(ctx) = self.context.as_ref() else {
            return true;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            return true;
        };

        // RouteState 체크 - 지표 기반 전략에서 시장 과열 시 진입 제한
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

    fn is_in_cooldown(&self) -> bool {
        self.cooldown_counter > 0
    }

    fn start_cooldown(&mut self) {
        if let Some(config) = &self.config {
            self.cooldown_counter = config.cooldown_candles;
        }
    }

    fn tick_cooldown(&mut self) {
        if self.cooldown_counter > 0 {
            self.cooldown_counter -= 1;
        }
    }

    fn has_position(&self) -> bool {
        self.position.side.is_some() && self.position.quantity > Decimal::ZERO
    }

    // ========================================================================
    // RSI 로직
    // ========================================================================

    fn generate_rsi_signals(&mut self, price: Decimal) -> Vec<Signal> {
        let Some(config) = self.config.as_ref() else {
            return vec![];
        };

        let rsi = match self.get_rsi_from_context(&config.ticker) {
            Some(r) => r,
            None => {
                debug!(ticker = %config.ticker, "StrategyContext에서 RSI를 가져올 수 없음");
                return vec![];
            }
        };

        let mut signals = vec![];

        // 진입 체크
        if !self.has_position() && !self.is_in_cooldown() && self.can_enter() {
            if rsi < config.oversold {
                let base_strength = ((config.oversold - rsi) / config.oversold).to_f64().unwrap_or(0.5);
                let strength = self.get_adjusted_strength(base_strength);
                signals.push(
                    Signal::new("mean_reversion", config.ticker.clone(), Side::Buy, SignalType::Entry)
                        .with_strength(strength)
                        .with_prices(Some(price), None, None)
                        .with_metadata("variant", json!("rsi"))
                        .with_metadata("rsi", json!(rsi.to_string())),
                );
            }
        }

        // 청산 체크
        if self.has_position() && self.position.side == Some(Side::Buy) {
            let entry = self.position.entry_price;

            // 손절
            if let Some(sl_pct) = config.exit_config.stop_loss() {
                let stop_price = entry * (dec!(1) - sl_pct / dec!(100));
                if price <= stop_price {
                    signals.push(
                        Signal::new("mean_reversion", config.ticker.clone(), Side::Sell, SignalType::Exit)
                            .with_strength(1.0)
                            .with_prices(Some(price), None, None)
                            .with_metadata("reason", json!("stop_loss")),
                    );
                    self.prev_rsi = Some(rsi);
                    return signals;
                }
            }

            // 익절
            if let Some(tp_pct) = config.exit_config.take_profit() {
                let target_price = entry * (dec!(1) + tp_pct / dec!(100));
                if price >= target_price {
                    signals.push(
                        Signal::new("mean_reversion", config.ticker.clone(), Side::Sell, SignalType::Exit)
                            .with_strength(1.0)
                            .with_prices(Some(price), None, None)
                            .with_metadata("reason", json!("take_profit")),
                    );
                    self.prev_rsi = Some(rsi);
                    return signals;
                }
            }

            // RSI 과매수 청산
            if rsi > config.overbought {
                signals.push(
                    Signal::new("mean_reversion", config.ticker.clone(), Side::Sell, SignalType::Exit)
                        .with_strength(0.8)
                        .with_prices(Some(price), None, None)
                        .with_metadata("reason", json!("rsi_overbought")),
                );
            }
        }

        self.prev_rsi = Some(rsi);
        signals
    }

    // ========================================================================
    // Bollinger 로직
    // ========================================================================

    fn generate_bollinger_signals(&mut self, price: Decimal) -> Vec<Signal> {
        let Some(config) = self.config.as_ref() else {
            return vec![];
        };

        let (lower, middle, _upper, bandwidth) = match self.get_bollinger_from_context(&config.ticker) {
            Some(bb) => bb,
            None => {
                debug!(ticker = %config.ticker, "StrategyContext에서 볼린저 밴드를 가져올 수 없음");
                return vec![];
            }
        };

        // 밴드폭 체크
        if bandwidth < config.min_bandwidth_pct {
            debug!(bandwidth = %bandwidth, "볼린저 스퀴즈 - 대기");
            return vec![];
        }

        let mut signals = vec![];

        // 진입 체크
        if !self.has_position() && !self.is_in_cooldown() && self.can_enter() {
            if price <= lower {
                let rsi_ok = if config.use_rsi_confirmation {
                    self.prev_rsi.map(|r| r < dec!(30)).unwrap_or(false)
                } else {
                    true
                };

                if rsi_ok {
                    let strength = self.get_adjusted_strength(0.8);
                    signals.push(
                        Signal::new("mean_reversion", config.ticker.clone(), Side::Buy, SignalType::Entry)
                            .with_strength(strength)
                            .with_prices(Some(price), None, None)
                            .with_metadata("variant", json!("bollinger"))
                            .with_metadata("lower_band", json!(lower.to_string())),
                    );
                }
            }
        }

        // 청산 체크
        if self.has_position() && self.position.side == Some(Side::Buy) {
            let entry = self.position.entry_price;

            // 손절
            if let Some(sl_pct) = config.exit_config.stop_loss() {
                let stop_price = entry * (dec!(1) - sl_pct / dec!(100));
                if price <= stop_price {
                    signals.push(
                        Signal::new("mean_reversion", config.ticker.clone(), Side::Sell, SignalType::Exit)
                            .with_strength(1.0)
                            .with_prices(Some(price), None, None)
                            .with_metadata("reason", json!("stop_loss")),
                    );
                    return signals;
                }
            }

            // 익절
            if let Some(tp_pct) = config.exit_config.take_profit() {
                let target_price = entry * (dec!(1) + tp_pct / dec!(100));
                if price >= target_price {
                    signals.push(
                        Signal::new("mean_reversion", config.ticker.clone(), Side::Sell, SignalType::Exit)
                            .with_strength(1.0)
                            .with_prices(Some(price), None, None)
                            .with_metadata("reason", json!("take_profit")),
                    );
                    return signals;
                }
            }

            // 중간밴드 청산
            if config.exit_config.exit_on_opposite_signal && price >= middle {
                signals.push(
                    Signal::new("mean_reversion", config.ticker.clone(), Side::Sell, SignalType::Exit)
                        .with_strength(0.7)
                        .with_prices(Some(price), None, None)
                        .with_metadata("reason", json!("middle_band")),
                );
            }
        }

        signals
    }
}

impl Default for MeanReversionStrategy {
    fn default() -> Self {
        Self::new()
    }
}

// ================================================================================================
// Strategy Trait 구현
// ================================================================================================

#[async_trait]
impl Strategy for MeanReversionStrategy {
    fn name(&self) -> &str {
        // 초기화 전에도 올바른 이름 반환을 위해 initial_variant 사용
        match self.initial_variant {
            MeanReversionVariant::Rsi => "MeanReversion-RSI",
            MeanReversionVariant::Bollinger => "MeanReversion-Bollinger",
        }
    }

    fn version(&self) -> &str {
        "2.0.0"
    }

    fn description(&self) -> &str {
        "평균회귀 전략 (RSI, Bollinger)"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // config에 variant가 없으면 팩토리에서 설정한 initial_variant 사용
        let variant = config
            .get("variant")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "rsi" | "Rsi" => Some(MeanReversionVariant::Rsi),
                "bollinger" | "Bollinger" => Some(MeanReversionVariant::Bollinger),
                _ => None,
            })
            .unwrap_or(self.initial_variant);

        let mr_config: MeanReversionConfig = match variant {
            MeanReversionVariant::Rsi => {
                let cfg: RsiConfig = serde_json::from_value(config)?;
                cfg.into()
            }
            MeanReversionVariant::Bollinger => {
                let cfg: BollingerConfig = serde_json::from_value(config)?;
                cfg.into()
            }
        };

        info!(
            variant = ?mr_config.variant,
            ticker = %mr_config.ticker,
            "[MeanReversion] 전략 초기화"
        );

        self.rsi_calculator = RsiCalculator::new(mr_config.rsi_period);
        self.config = Some(mr_config);
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

        // 가격 히스토리 업데이트
        self.prices.push_back(price);
        if self.prices.len() > 300 {
            self.prices.pop_front();
        }

        // 쿨다운 감소
        self.tick_cooldown();

        // RSI 업데이트
        let _ = self.rsi_calculator.update(price);

        // 볼린저에서 RSI 확인용
        if matches!(variant, MeanReversionVariant::Bollinger) {
            if let Some(rsi) = self.get_rsi_from_context(&ticker) {
                self.prev_rsi = Some(rsi);
            }
        }

        let signals = match variant {
            MeanReversionVariant::Rsi => self.generate_rsi_signals(price),
            MeanReversionVariant::Bollinger => self.generate_bollinger_signals(price),
        };

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "[MeanReversion] 주문 체결: {:?} {} @ {:?}",
            order.side, order.quantity, order.average_fill_price
        );

        if let Some(fill_price) = order.average_fill_price {
            match order.side {
                Side::Buy => {
                    self.position.side = Some(Side::Buy);
                    self.position.entry_price = fill_price;
                    self.position.quantity += order.quantity;
                    self.position.entry_time = Some(chrono::Utc::now());
                }
                Side::Sell => {
                    self.position.quantity -= order.quantity;
                    if self.position.quantity <= Decimal::ZERO {
                        self.position = PositionState::default();
                        self.start_cooldown();
                    }
                }
            }
        }

        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(config) = self.config.as_ref() else {
            return Ok(());
        };

        if position.ticker != config.ticker {
            return Ok(());
        }

        if position.quantity > Decimal::ZERO {
            self.position.side = Some(position.side);
            self.position.entry_price = position.entry_price;
            self.position.quantity = position.quantity;
        } else {
            self.position = PositionState::default();
        }

        info!(
            "[MeanReversion] 포지션 업데이트: {} = {}",
            position.ticker, position.quantity
        );

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("[MeanReversion] 전략 종료");
        Ok(())
    }

    fn get_state(&self) -> Value {
        let mut state = json!({
            "name": self.name(),
            "version": self.version(),
            "variant": self.config.as_ref().map(|c| format!("{:?}", c.variant)),
            "initialized": self.initialized,
            "has_position": self.has_position(),
            "position": {
                "side": self.position.side.map(|s| format!("{:?}", s)),
                "entry_price": self.position.entry_price.to_string(),
                "quantity": self.position.quantity.to_string(),
            },
            "cooldown": self.cooldown_counter,
            "prev_rsi": self.prev_rsi.map(|r| r.to_string()),
        });

        // config.ticker 반환 (시뮬레이션에서 사용)
        if let Some(config) = &self.config {
            state["config"] = json!({
                "ticker": config.ticker,
                "variant": format!("{:?}", config.variant),
            });
        }

        state
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("[MeanReversion] StrategyContext 주입 완료");
    }
}

// ================================================================================================
// 전략 레지스트리 등록
// ================================================================================================

use crate::register_strategy;

// RSI 평균회귀 전략
register_strategy! {
    id: "rsi",
    aliases: ["rsi_mean_reversion", "rsi_strategy"],
    name: "RSI 평균회귀",
    description: "RSI 과매수/과매도 구간에서 평균회귀 매매",
    timeframe: "15m",
    tickers: [],
    category: Intraday,
    markets: [Crypto, Stock],
    factory: MeanReversionStrategy::rsi,
    config: RsiConfig
}

// 볼린저 밴드 전략
register_strategy! {
    id: "bollinger",
    aliases: ["bollinger_bands", "bb_strategy"],
    name: "볼린저 밴드",
    description: "볼린저 밴드 상/하단 터치 시 평균회귀 매매",
    timeframe: "15m",
    tickers: [],
    category: Intraday,
    markets: [Crypto, Stock],
    factory: MeanReversionStrategy::bollinger,
    config: BollingerConfig
}
