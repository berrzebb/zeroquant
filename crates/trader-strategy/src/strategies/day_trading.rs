//! 일간 모멘텀 통합 전략 (Day Trading Strategy)
//!
//! 변동성 돌파, SMA 크로스오버, 거래량 급증 전략을 통합한 일간 트레이딩 전략입니다.
//!
//! # 전략 변형 (Variant)
//!
//! - **Breakout**: 래리 윌리엄스 변동성 돌파 전략
//!   - 시가 + (전일 레인지 × K) 돌파 시 진입
//!   - K 계수로 신호 강도 조절
//!   - 장 마감 전 청산 옵션
//!
//! - **Crossover**: 이동평균 크로스오버 전략
//!   - 단기 MA가 장기 MA 상향 돌파 시 매수
//!   - 하향 돌파 시 매도
//!   - 추세 추종 전략
//!
//! - **VolumeSurge**: 거래량 급증 모멘텀 전략
//!   - 거래량 2배 이상 급증
//!   - N개 연속 상승봉
//!   - RSI 과열 아님
//!   - 트레일링 스톱 청산
//!
//! # StrategyContext 연동
//!
//! - `RouteState`: Attack/Armed에서만 진입
//! - `GlobalScore`: 최소 점수 이상일 때만 진입
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! // 변동성 돌파 설정
//! let vb = DayTradingStrategy::breakout();
//!
//! // SMA 크로스오버 설정
//! let sma = DayTradingStrategy::crossover();
//!
//! // 거래량 급증 설정
//! let volume = DayTradingStrategy::volume_surge();
//!
//! // 커스텀 설정
//! let custom = DayTradingStrategy::with_config(DayTradingConfig {
//!     variant: DayTradingVariant::Breakout,
//!     ticker: "BTC/USDT".to_string(),
//!     breakout_config: Some(BreakoutConfig {
//!         k_factor: dec!(0.6),
//!         ..Default::default()
//!     }),
//!     ..Default::default()
//! });
//! ```

use crate::strategies::common::adjust_strength_by_score;
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use trader_strategy_macro::StrategyConfig;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::domain::{RouteState, StrategyContext};
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal, Timeframe};

/// 일간 트레이딩 전략 변형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum DayTradingVariant {
    /// 변동성 돌파 (래리 윌리엄스)
    #[default]
    Breakout,
    /// 이동평균 크로스오버
    Crossover,
    /// 거래량 급증 모멘텀
    VolumeSurge,
}


/// 돌파 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakoutConfig {
    /// 돌파 K 계수 (기본값: 0.5)
    #[serde(default = "default_k_factor")]
    pub k_factor: Decimal,

    /// ATR 사용 여부 (기본값: false)
    #[serde(default)]
    pub use_atr: bool,

    /// ATR 기간 (기본값: 14)
    #[serde(default = "default_atr_period")]
    pub atr_period: usize,

    /// 룩백 기간 (기본값: 1)
    #[serde(default = "default_lookback")]
    pub lookback_period: usize,

    /// 최소 레인지 비율 (%) (기본값: 0.5)
    #[serde(default = "default_min_range_pct")]
    pub min_range_pct: Decimal,

    /// 최대 레인지 비율 (%) (기본값: 10.0)
    #[serde(default = "default_max_range_pct")]
    pub max_range_pct: Decimal,

    /// 양방향 거래 여부 (기본값: true)
    #[serde(default = "default_trade_both_directions")]
    pub trade_both_directions: bool,

    /// 기간 종료 시 청산 (기본값: true)
    #[serde(default = "default_exit_at_period_close")]
    pub exit_at_period_close: bool,
}

fn default_k_factor() -> Decimal {
    dec!(0.5)
}
fn default_atr_period() -> usize {
    14
}
fn default_lookback() -> usize {
    1
}
fn default_min_range_pct() -> Decimal {
    dec!(0.5)
}
fn default_max_range_pct() -> Decimal {
    dec!(10.0)
}
fn default_trade_both_directions() -> bool {
    true
}
fn default_exit_at_period_close() -> bool {
    true
}

impl Default for BreakoutConfig {
    fn default() -> Self {
        Self {
            k_factor: default_k_factor(),
            use_atr: false,
            atr_period: default_atr_period(),
            lookback_period: default_lookback(),
            min_range_pct: default_min_range_pct(),
            max_range_pct: default_max_range_pct(),
            trade_both_directions: default_trade_both_directions(),
            exit_at_period_close: default_exit_at_period_close(),
        }
    }
}

/// 크로스오버 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossoverConfig {
    /// 단기 이동평균 기간 (기본값: 10)
    #[serde(default = "default_short_period")]
    pub short_period: usize,

    /// 장기 이동평균 기간 (기본값: 20)
    #[serde(default = "default_long_period")]
    pub long_period: usize,
}

fn default_short_period() -> usize {
    10
}
fn default_long_period() -> usize {
    20
}

impl Default for CrossoverConfig {
    fn default() -> Self {
        Self {
            short_period: default_short_period(),
            long_period: default_long_period(),
        }
    }
}

/// 거래량 급증 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeSurgeConfig {
    /// 거래량 급증 배수 (기본값: 2.0)
    #[serde(default = "default_volume_multiplier")]
    pub volume_multiplier: Decimal,

    /// 거래량 평균 기간 (기본값: 20)
    #[serde(default = "default_volume_period")]
    pub volume_period: usize,

    /// 연속 상승봉 수 (기본값: 3)
    #[serde(default = "default_consecutive_up")]
    pub consecutive_up_candles: usize,

    /// RSI 과열 기준 (기본값: 80)
    #[serde(default = "default_rsi_overbought")]
    pub rsi_overbought: Decimal,

    /// RSI 기간 (기본값: 14)
    #[serde(default = "default_rsi_period")]
    pub rsi_period: usize,

    /// 최대 보유 시간 (분) (기본값: 120)
    #[serde(default = "default_max_hold_minutes")]
    pub max_hold_minutes: u32,
}

fn default_volume_multiplier() -> Decimal {
    dec!(2.0)
}
fn default_volume_period() -> usize {
    20
}
fn default_consecutive_up() -> usize {
    3
}
fn default_rsi_overbought() -> Decimal {
    dec!(80)
}
fn default_rsi_period() -> usize {
    14
}
fn default_max_hold_minutes() -> u32 {
    120
}

impl Default for VolumeSurgeConfig {
    fn default() -> Self {
        Self {
            volume_multiplier: default_volume_multiplier(),
            volume_period: default_volume_period(),
            consecutive_up_candles: default_consecutive_up(),
            rsi_overbought: default_rsi_overbought(),
            rsi_period: default_rsi_period(),
            max_hold_minutes: default_max_hold_minutes(),
        }
    }
}

/// 청산 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitConfig {
    /// 손절 비율 (%) (기본값: 2.0)
    #[serde(default = "default_stop_loss_pct")]
    pub stop_loss_pct: Decimal,

    /// 익절 비율 (%) (기본값: 3.0)
    #[serde(default = "default_take_profit_pct")]
    pub take_profit_pct: Decimal,

    /// 트레일링 스톱 비율 (%) (기본값: 1.5)
    #[serde(default = "default_trailing_stop_pct")]
    pub trailing_stop_pct: Decimal,

    /// 반대 신호 시 청산 (기본값: true)
    #[serde(default = "default_exit_on_opposite")]
    pub exit_on_opposite_signal: bool,
}

fn default_stop_loss_pct() -> Decimal {
    dec!(2.0)
}
fn default_take_profit_pct() -> Decimal {
    dec!(3.0)
}
fn default_trailing_stop_pct() -> Decimal {
    dec!(1.5)
}
fn default_exit_on_opposite() -> bool {
    true
}

impl Default for ExitConfig {
    fn default() -> Self {
        Self {
            stop_loss_pct: default_stop_loss_pct(),
            take_profit_pct: default_take_profit_pct(),
            trailing_stop_pct: default_trailing_stop_pct(),
            exit_on_opposite_signal: default_exit_on_opposite(),
        }
    }
}

// ================================================================================================
// 전략별 UI Config (SDUI용)
// ================================================================================================

/// 변동성 돌파 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "volatility_breakout",
    name = "변동성 돌파",
    description = "Larry Williams 변동성 돌파 전략 (당일 시가 + 전일 범위 × K)",
    category = "Daily"
)]
pub struct VolatilityBreakoutConfig {
    /// 대상 티커.
    #[serde(default = "default_day_ticker")]
    #[schema(label = "거래 종목", field_type = "symbol", default = "005930", section = "asset")]
    pub ticker: String,

    /// 거래 금액.
    #[serde(default = "default_trade_amount")]
    #[schema(label = "거래 금액", field_type = "number", min = 100000, max = 100000000, default = 1000000, section = "asset")]
    pub trade_amount: Decimal,

    /// 돌파 K 계수.
    #[serde(default = "default_k_factor")]
    #[schema(label = "K 계수", field_type = "number", min = 0.1, max = 1, default = 0.1, section = "indicator")]
    pub k_factor: Decimal,

    /// ATR 사용 여부.
    #[serde(default)]
    #[schema(label = "ATR 사용", field_type = "boolean", default = false, section = "indicator")]
    pub use_atr: bool,

    /// ATR 기간.
    #[serde(default = "default_atr_period")]
    #[schema(label = "ATR 기간", field_type = "integer", min = 5, max = 50, default = 14, section = "indicator")]
    pub atr_period: usize,

    /// 룩백 기간.
    #[serde(default = "default_lookback")]
    #[schema(label = "룩백 기간", field_type = "integer", min = 1, max = 10, default = 1, section = "indicator")]
    pub lookback_period: usize,

    /// 양방향 거래 여부.
    #[serde(default = "default_trade_both_directions")]
    #[schema(label = "양방향 거래", field_type = "boolean", default = true, section = "trading")]
    pub trade_both_directions: bool,

    /// 기간 종료 시 청산.
    #[serde(default = "default_exit_at_period_close")]
    #[schema(label = "장 마감 청산", field_type = "boolean", default = false, section = "trading")]
    pub exit_at_period_close: bool,

    /// 청산 설정.
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,

    /// 최소 GlobalScore.
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", field_type = "number", min = 0, max = 100, default = 0, section = "filter")]
    pub min_global_score: Decimal,
}

fn default_day_ticker() -> String {
    "005930".to_string()
}

impl From<VolatilityBreakoutConfig> for DayTradingConfig {
    fn from(cfg: VolatilityBreakoutConfig) -> Self {
        Self {
            variant: DayTradingVariant::Breakout,
            ticker: cfg.ticker,
            breakout_config: BreakoutConfig {
                k_factor: cfg.k_factor,
                use_atr: cfg.use_atr,
                atr_period: cfg.atr_period,
                lookback_period: cfg.lookback_period,
                trade_both_directions: cfg.trade_both_directions,
                exit_at_period_close: cfg.exit_at_period_close,
                ..Default::default()
            },
            crossover_config: CrossoverConfig::default(),
            volume_surge_config: VolumeSurgeConfig::default(),
            exit_config: cfg.exit_config,
            min_global_score: cfg.min_global_score,
            trade_amount: cfg.trade_amount,
        }
    }
}

/// SMA 크로스오버 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "sma_crossover",
    name = "SMA 크로스오버",
    description = "단기/장기 이동평균 교차 매매 전략",
    category = "Daily"
)]
pub struct SmaCrossoverConfig {
    /// 대상 티커.
    #[serde(default = "default_day_ticker")]
    #[schema(label = "거래 종목", field_type = "symbol", default = "005930", section = "asset")]
    pub ticker: String,

    /// 거래 금액.
    #[serde(default = "default_trade_amount")]
    #[schema(label = "거래 금액", field_type = "number", min = 100000, max = 100000000, default = 1000000, section = "asset")]
    pub trade_amount: Decimal,

    /// 단기 이동평균 기간.
    #[serde(default = "default_short_period")]
    #[schema(label = "단기 MA 기간", field_type = "integer", min = 2, max = 50, default = 3, section = "indicator")]
    pub short_period: usize,

    /// 장기 이동평균 기간.
    #[serde(default = "default_long_period")]
    #[schema(label = "장기 MA 기간", field_type = "integer", min = 5, max = 200, default = 7, section = "indicator")]
    pub long_period: usize,

    /// 청산 설정.
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,

    /// 최소 GlobalScore.
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", field_type = "number", min = 0, max = 100, default = 0, section = "filter")]
    pub min_global_score: Decimal,
}

impl From<SmaCrossoverConfig> for DayTradingConfig {
    fn from(cfg: SmaCrossoverConfig) -> Self {
        Self {
            variant: DayTradingVariant::Crossover,
            ticker: cfg.ticker,
            breakout_config: BreakoutConfig::default(),
            crossover_config: CrossoverConfig {
                short_period: cfg.short_period,
                long_period: cfg.long_period,
            },
            volume_surge_config: VolumeSurgeConfig::default(),
            exit_config: cfg.exit_config,
            min_global_score: cfg.min_global_score,
            trade_amount: cfg.trade_amount,
        }
    }
}

/// 거래량 급증 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "volume_surge",
    name = "거래량 급증",
    description = "거래량 급증 + 연속 상승봉 패턴 포착 전략",
    category = "Daily"
)]
pub struct VolumeSurgeStrategyConfig {
    /// 대상 티커.
    #[serde(default = "default_day_ticker")]
    #[schema(label = "거래 종목", field_type = "symbol", default = "005930", section = "asset")]
    pub ticker: String,

    /// 거래 금액.
    #[serde(default = "default_trade_amount")]
    #[schema(label = "거래 금액", field_type = "number", min = 100000, max = 100000000, default = 1000000, section = "asset")]
    pub trade_amount: Decimal,

    /// 거래량 급증 배수.
    #[serde(default = "default_volume_multiplier")]
    #[schema(label = "거래량 배수", field_type = "number", min = 1, max = 10, default = 1.1, section = "indicator")]
    pub volume_multiplier: Decimal,

    /// 거래량 평균 기간.
    #[serde(default = "default_volume_period")]
    #[schema(label = "거래량 평균 기간", field_type = "integer", min = 5, max = 60, default = 5, section = "indicator")]
    pub volume_period: usize,

    /// 연속 상승봉 수.
    #[serde(default = "default_consecutive_up")]
    #[schema(label = "연속 상승봉 수", field_type = "integer", min = 1, max = 10, default = 1, section = "indicator")]
    pub consecutive_up_candles: usize,

    /// RSI 과열 기준.
    #[serde(default = "default_rsi_overbought")]
    #[schema(label = "RSI 과열 기준", field_type = "number", min = 60, max = 100, default = 90, section = "indicator")]
    pub rsi_overbought: Decimal,

    /// RSI 기간.
    #[serde(default = "default_rsi_period")]
    #[schema(label = "RSI 기간", field_type = "integer", min = 5, max = 50, default = 5, section = "indicator")]
    pub rsi_period: usize,

    /// 최대 보유 시간 (분).
    #[serde(default = "default_max_hold_minutes")]
    #[schema(label = "최대 보유 시간 (분)", field_type = "integer", min = 10, max = 480, default = 120, section = "trading")]
    pub max_hold_minutes: u32,

    /// 청산 설정.
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,

    /// 최소 GlobalScore.
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", field_type = "number", min = 0, max = 100, default = 0, section = "filter")]
    pub min_global_score: Decimal,
}

impl From<VolumeSurgeStrategyConfig> for DayTradingConfig {
    fn from(cfg: VolumeSurgeStrategyConfig) -> Self {
        Self {
            variant: DayTradingVariant::VolumeSurge,
            ticker: cfg.ticker,
            breakout_config: BreakoutConfig::default(),
            crossover_config: CrossoverConfig::default(),
            volume_surge_config: VolumeSurgeConfig {
                volume_multiplier: cfg.volume_multiplier,
                volume_period: cfg.volume_period,
                consecutive_up_candles: cfg.consecutive_up_candles,
                rsi_overbought: cfg.rsi_overbought,
                rsi_period: cfg.rsi_period,
                max_hold_minutes: cfg.max_hold_minutes,
            },
            exit_config: cfg.exit_config,
            min_global_score: cfg.min_global_score,
            trade_amount: cfg.trade_amount,
        }
    }
}

// ================================================================================================
// 내부용 통합 설정 (런타임에서 사용)
// ================================================================================================

/// 일간 트레이딩 전략 설정 (내부용).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayTradingConfig {
    /// 전략 변형.
    pub variant: DayTradingVariant,

    /// 대상 티커.
    pub ticker: String,

    /// 돌파 전략 설정.
    pub breakout_config: BreakoutConfig,

    /// 크로스오버 전략 설정.
    pub crossover_config: CrossoverConfig,

    /// 거래량 급증 전략 설정.
    pub volume_surge_config: VolumeSurgeConfig,

    /// 청산 설정.
    pub exit_config: ExitConfig,

    /// 최소 GlobalScore.
    pub min_global_score: Decimal,

    /// 거래 금액.
    pub trade_amount: Decimal,
}

fn default_min_global_score() -> Decimal {
    dec!(50)
}
fn default_trade_amount() -> Decimal {
    dec!(1000000)
}

impl Default for DayTradingConfig {
    fn default() -> Self {
        Self {
            variant: DayTradingVariant::default(),
            ticker: "BTC/USDT".to_string(),
            breakout_config: BreakoutConfig::default(),
            crossover_config: CrossoverConfig::default(),
            volume_surge_config: VolumeSurgeConfig::default(),
            exit_config: ExitConfig::default(),
            min_global_score: default_min_global_score(),
            trade_amount: default_trade_amount(),
        }
    }
}

/// 캔들 데이터.
#[derive(Debug, Clone)]
struct CandleData {
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    volume: Decimal,
    timestamp: DateTime<Utc>,
}

/// 포지션 상태.
#[derive(Debug, Clone)]
struct PositionState {
    side: Side,
    entry_price: Decimal,
    entry_time: DateTime<Utc>,
    highest_price: Decimal,
    lowest_price: Decimal,
}

/// 일간 트레이딩 통합 전략.
pub struct DayTradingStrategy {
    config: Option<DayTradingConfig>,
    ticker: Option<String>,
    context: Option<Arc<RwLock<StrategyContext>>>,

    // 캔들 데이터
    candles: VecDeque<CandleData>,
    current_candle: Option<CandleData>,

    // 포지션 상태
    position: Option<PositionState>,

    // 지표 데이터
    prices: VecDeque<Decimal>,
    volumes: VecDeque<Decimal>,
    tr_history: VecDeque<Decimal>,

    // 계산된 값들
    current_atr: Option<Decimal>,
    prev_range: Option<Decimal>,
    prev_short_sma: Option<Decimal>,
    prev_long_sma: Option<Decimal>,

    // 돌파 레벨 (Breakout용)
    upper_breakout: Option<Decimal>,
    lower_breakout: Option<Decimal>,
    triggered_this_period: bool,

    // 통계
    trades_count: u32,
    wins: u32,
    losses_count: u32,
    total_pnl: Decimal,

    initialized: bool,
}

impl DayTradingStrategy {
    /// 기본 생성자 (매크로 호환용).
    pub fn new() -> Self {
        Self {
            config: None,
            ticker: None,
            context: None,
            candles: VecDeque::new(),
            current_candle: None,
            position: None,
            prices: VecDeque::new(),
            volumes: VecDeque::new(),
            tr_history: VecDeque::new(),
            current_atr: None,
            prev_range: None,
            prev_short_sma: None,
            prev_long_sma: None,
            upper_breakout: None,
            lower_breakout: None,
            triggered_this_period: false,
            trades_count: 0,
            wins: 0,
            losses_count: 0,
            total_pnl: Decimal::ZERO,
            initialized: false,
        }
    }

    /// 설정으로 생성.
    pub fn with_config(config: DayTradingConfig) -> Self {
        let mut strategy = Self::new();
        strategy.ticker = Some(config.ticker.clone());
        strategy.config = Some(config);
        strategy
    }

    /// 변동성 돌파 전략 생성.
    pub fn breakout() -> Self {
        Self::with_config(DayTradingConfig {
            variant: DayTradingVariant::Breakout,
            ticker: "BTC/USDT".to_string(),
            ..Default::default()
        })
    }

    /// SMA 크로스오버 전략 생성.
    pub fn crossover() -> Self {
        Self::with_config(DayTradingConfig {
            variant: DayTradingVariant::Crossover,
            ticker: "BTC/USDT".to_string(),
            ..Default::default()
        })
    }

    /// 거래량 급증 전략 생성.
    pub fn volume_surge() -> Self {
        Self::with_config(DayTradingConfig {
            variant: DayTradingVariant::VolumeSurge,
            ticker: "005930".to_string(),
            ..Default::default()
        })
    }

    // ====== 공통 메서드 ======

    /// StrategyContext 기반 진입 가능 여부 체크.
    /// 단일 티커 전략: GlobalScore 체크 불필요 (스크리닝용)
    fn can_enter(&self) -> bool {
        let Some(config) = self.config.as_ref() else {
            return false;
        };
        let ticker = &config.ticker;

        let Some(ctx) = self.context.as_ref() else {
            return true; // Context 없으면 진입 허용
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            return true;
        };

        // RouteState 체크 - 시장 과열 시 진입 제한
        if let Some(route_state) = ctx_lock.get_route_state(ticker) {
            match route_state {
                RouteState::Overheat => {
                    debug!(ticker = %ticker, route_state = ?route_state, "시장 과열 - 진입 제한");
                    return false;
                }
                // Wait, Neutral, Armed, Attack: 진입 허용
                _ => {}
            }
        }

        true
    }


    /// GlobalScore 기반 신호 강도 계산.
    ///
    /// 기본 강도를 GlobalScore에 따라 조정합니다.
    /// - 90점 이상: 1.25배
    /// - 80~90점: 1.15배
    /// - 70~80점: 1.0배 (기본)
    /// - 60~70점: 0.85배
    /// - 60점 미만: 0.75배
    fn get_adjusted_strength(&self, base_strength: f64) -> f64 {
        let Some(config) = self.config.as_ref() else {
            return base_strength;
        };
        let ticker = &config.ticker;

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

    /// SMA 계산.
    fn calculate_sma(&self, period: usize) -> Option<Decimal> {
        if self.prices.len() < period {
            return None;
        }
        let sum: Decimal = self.prices.iter().take(period).sum();
        Some(sum / Decimal::from(period))
    }

    /// True Range 계산.
    fn calculate_true_range(
        &self,
        high: Decimal,
        low: Decimal,
        prev_close: Option<Decimal>,
    ) -> Decimal {
        let hl = high - low;
        match prev_close {
            Some(pc) => {
                let hc = (high - pc).abs();
                let lc = (low - pc).abs();
                hl.max(hc).max(lc)
            }
            None => hl,
        }
    }

    /// ATR 계산.
    fn calculate_atr(&mut self) {
        let Some(config) = self.config.as_ref() else {
            return;
        };
        let atr_period = config.breakout_config.atr_period;

        if self.tr_history.len() >= atr_period {
            let sum: Decimal = self.tr_history.iter().take(atr_period).sum();
            self.current_atr = Some(sum / Decimal::from(atr_period));
        }
    }

    /// 평균 거래량 계산.
    fn calculate_avg_volume(&self) -> Option<Decimal> {
        let config = self.config.as_ref()?;
        let period = config.volume_surge_config.volume_period;

        if self.volumes.len() <= period {
            return None;
        }

        // 현재 봉 제외
        let sum: Decimal = self.volumes.iter().skip(1).take(period).sum();
        Some(sum / Decimal::from(period))
    }

    /// RSI 계산.
    fn calculate_rsi(&self) -> Option<Decimal> {
        let config = self.config.as_ref()?;
        let period = config.volume_surge_config.rsi_period;

        if self.candles.len() < period + 1 {
            return None;
        }

        let mut gains = Decimal::ZERO;
        let mut losses = Decimal::ZERO;

        for i in 0..period {
            let current = &self.candles[i];
            let prev = &self.candles[i + 1];
            let change = current.close - prev.close;

            if change > Decimal::ZERO {
                gains += change;
            } else {
                losses += change.abs();
            }
        }

        let p = Decimal::from(period);
        let avg_gain = gains / p;
        let avg_loss = losses / p;

        if avg_loss == Decimal::ZERO {
            return Some(dec!(100));
        }

        let rs = avg_gain / avg_loss;
        Some(dec!(100) - (dec!(100) / (dec!(1) + rs)))
    }

    /// 연속 상승봉 확인.
    fn has_consecutive_up_candles(&self) -> bool {
        let Some(config) = self.config.as_ref() else {
            return false;
        };
        let n = config.volume_surge_config.consecutive_up_candles;

        if self.candles.len() < n {
            return false;
        }

        self.candles.iter().take(n).all(|c| c.close > c.open)
    }

    /// 새 기간 시작 여부 확인.
    fn is_new_period(&self, current_time: DateTime<Utc>) -> bool {
        match &self.current_candle {
            Some(candle) => {
                // 날짜가 다르면 새 기간
                if current_time.date_naive() != candle.timestamp.date_naive() {
                    return true;
                }
                // 같은 날이면 시간이 다른지 확인
                current_time.hour() != candle.timestamp.hour()
            }
            None => true,
        }
    }

    /// 기간 종료 처리.
    fn on_period_close(&mut self) {
        if let Some(candle) = self.current_candle.take() {
            let (atr_period, lookback) = {
                let config = match self.config.as_ref() {
                    Some(c) => c,
                    None => return,
                };
                (
                    config.breakout_config.atr_period,
                    config.breakout_config.lookback_period,
                )
            };

            // True Range 계산 및 저장
            let prev_close = self.candles.front().map(|c| c.close);
            let tr = self.calculate_true_range(candle.high, candle.low, prev_close);

            self.tr_history.push_front(tr);
            while self.tr_history.len() > atr_period + 1 {
                self.tr_history.pop_back();
            }

            // 거래량 저장
            self.volumes.push_front(candle.volume);
            while self.volumes.len() > 25 {
                self.volumes.pop_back();
            }

            // ATR 계산
            self.calculate_atr();

            // 단순 레인지 저장
            self.prev_range = Some(candle.high - candle.low);

            // 캔들 저장
            self.candles.push_front(candle);
            while self.candles.len() > lookback + 20 {
                self.candles.pop_back();
            }

            // 트리거 플래그 리셋
            self.triggered_this_period = false;
        }
    }

    /// StrategyContext에서 레인지와 시가 가져오기.
    ///
    /// 전일 캔들의 (high - low)를 레인지로, 오늘 캔들의 open을 시가로 반환합니다.
    fn get_range_from_context(&self, ticker: &str) -> Option<(Decimal, Decimal)> {
        let ctx = self.context.as_ref()?;
        let ctx_lock = ctx.try_read().ok()?;

        // D1 타임프레임 캔들 가져오기
        let klines = ctx_lock.get_klines(ticker, Timeframe::D1);

        if klines.len() < 2 {
            debug!(ticker = %ticker, klines_len = klines.len(), "캔들 데이터 부족");
            return None;
        }

        let today_kline = &klines[0];      // 오늘 캔들 (가장 최근)
        let prev_kline = &klines[1];       // 전일 캔들

        // 레인지: 전일 고가 - 전일 저가
        let range = prev_kline.high - prev_kline.low;
        // 시가: 오늘 캔들의 시가
        let period_open = today_kline.open;

        debug!(
            ticker = %ticker,
            prev_high = %prev_kline.high,
            prev_low = %prev_kline.low,
            range = %range,
            today_open = %period_open,
            "StrategyContext에서 캔들 데이터 로드"
        );

        Some((range, period_open))
    }


    /// 레인지 유효성 검증.
    fn is_range_valid(&self, range: Decimal, current_price: Decimal) -> bool {
        let Some(config) = self.config.as_ref() else {
            return false;
        };

        if current_price == Decimal::ZERO {
            return false;
        }

        let range_pct = range / current_price * dec!(100);
        range_pct >= config.breakout_config.min_range_pct
            && range_pct <= config.breakout_config.max_range_pct
    }

    // ====== 신호 생성 ======

    /// 돌파 신호 생성.
    fn generate_breakout_signals(&mut self, candle: &CandleData) -> Vec<Signal> {
        let Some(config) = self.config.as_ref() else {
            return Vec::new();
        };
        let Some(ticker) = self.ticker.as_ref() else {
            return Vec::new();
        };
        let mut signals = Vec::new();

        // 기존 포지션 처리 (borrow 충돌 방지를 위해 복사)
        if let Some(pos) = self.position.clone() {
            let exit_result = self.check_exit_conditions(candle, &pos);
            if let Some((signal, pnl)) = exit_result {
                self.record_trade(pnl);
                signals.push(signal);
                self.position = None;
                return signals;
            }
            return signals;
        }

        // 이미 이번 기간에 트리거됨
        if self.triggered_this_period {
            return signals;
        }

        // 진입 가능 여부 확인
        if !self.can_enter() {
            return signals;
        }

        // StrategyContext에서 레인지와 시가 가져오기
        let (range, period_open) = match self.get_range_from_context(ticker) {
            Some(data) => data,
            None => {
                debug!(ticker = %ticker, "StrategyContext에서 캔들 데이터를 가져올 수 없음");
                return signals;
            }
        };

        // 레인지 유효성 검증
        if !self.is_range_valid(range, candle.close) {
            return signals;
        }

        // 돌파 레벨 계산 (period_open은 StrategyContext에서 가져옴)
        let k = config.breakout_config.k_factor;
        let upper = period_open + range * k;
        let lower = period_open - range * k;

        self.upper_breakout = Some(upper);
        self.lower_breakout = Some(lower);

        // 롱 돌파
        if candle.close >= upper {
            let stop = candle.close - range;
            let tp = candle.close + range * dec!(2);

            let strength = self.get_adjusted_strength(0.5);
            signals.push(
                Signal::entry("day_trading", ticker.clone(), Side::Buy)
                    .with_strength(strength)
                    .with_prices(Some(candle.close), Some(stop), Some(tp))
                    .with_metadata("variant", json!("breakout"))
                    .with_metadata("breakout_level", json!(upper.to_string()))
                    .with_metadata("range", json!(range.to_string())),
            );

            self.position = Some(PositionState {
                side: Side::Buy,
                entry_price: candle.close,
                entry_time: candle.timestamp,
                highest_price: candle.high,
                lowest_price: candle.low,
            });

            self.triggered_this_period = true;
            info!(price = %candle.close, breakout = %upper, range = %range, "롱 돌파 신호");
        }

        // 숏 돌파
        if config.breakout_config.trade_both_directions
            && candle.close <= lower
            && self.position.is_none()
        {
            let stop = candle.close + range;
            let tp = candle.close - range * dec!(2);

            let strength = self.get_adjusted_strength(0.5);
            signals.push(
                Signal::entry("day_trading", ticker.clone(), Side::Sell)
                    .with_strength(strength)
                    .with_prices(Some(candle.close), Some(stop), Some(tp))
                    .with_metadata("variant", json!("breakout"))
                    .with_metadata("breakout_level", json!(lower.to_string()))
                    .with_metadata("range", json!(range.to_string())),
            );

            self.position = Some(PositionState {
                side: Side::Sell,
                entry_price: candle.close,
                entry_time: candle.timestamp,
                highest_price: candle.high,
                lowest_price: candle.low,
            });

            self.triggered_this_period = true;
            info!(price = %candle.close, breakout = %lower, range = %range, "숏 돌파 신호");
        }

        signals
    }

    /// 크로스오버 신호 생성.
    fn generate_crossover_signals(&mut self, candle: &CandleData) -> Vec<Signal> {
        let Some(config) = self.config.as_ref() else {
            return Vec::new();
        };
        let Some(ticker) = self.ticker.as_ref() else {
            return Vec::new();
        };
        let mut signals = Vec::new();

        // 가격 히스토리 업데이트
        self.prices.push_front(candle.close);
        let max_len = config.crossover_config.long_period + 1;
        while self.prices.len() > max_len {
            self.prices.pop_back();
        }

        // SMA 계산
        let short_sma = match self.calculate_sma(config.crossover_config.short_period) {
            Some(s) => s,
            None => return signals,
        };
        let long_sma = match self.calculate_sma(config.crossover_config.long_period) {
            Some(s) => s,
            None => return signals,
        };

        // 크로스오버 감지
        if let (Some(prev_short), Some(prev_long)) = (self.prev_short_sma, self.prev_long_sma) {
            let golden_cross = prev_short <= prev_long && short_sma > long_sma;
            let death_cross = prev_short >= prev_long && short_sma < long_sma;
            let exit_on_opposite = config.exit_config.exit_on_opposite_signal;

            // 기존 포지션이 있으면 반대 크로스에서 청산 (borrow 충돌 방지를 위해 데이터 먼저 추출)
            let exit_info = self.position.as_ref().and_then(|pos| {
                let should_exit = match pos.side {
                    Side::Buy => death_cross,
                    Side::Sell => golden_cross,
                };

                if should_exit && exit_on_opposite {
                    let exit_side = match pos.side {
                        Side::Buy => Side::Sell,
                        Side::Sell => Side::Buy,
                    };
                    let pnl = match pos.side {
                        Side::Buy => candle.close - pos.entry_price,
                        Side::Sell => pos.entry_price - candle.close,
                    };
                    Some((exit_side, pnl))
                } else {
                    None
                }
            });

            if let Some((exit_side, pnl)) = exit_info {
                signals.push(
                    Signal::exit("day_trading", ticker.clone(), exit_side)
                        .with_strength(1.0)
                        .with_prices(Some(candle.close), None, None)
                        .with_metadata("variant", json!("crossover"))
                        .with_metadata("exit_reason", json!("opposite_cross")),
                );

                self.record_trade(pnl);
                self.position = None;

                info!(short_sma = %short_sma, long_sma = %long_sma, "크로스오버 청산");
            } else if self.position.is_none() && golden_cross && self.can_enter() {
                // 골든 크로스 - 매수
                let stop = candle.close * (dec!(1) - config.exit_config.stop_loss_pct / dec!(100));
                let tp = candle.close * (dec!(1) + config.exit_config.take_profit_pct / dec!(100));

                let strength = self.get_adjusted_strength(1.0);
                signals.push(
                    Signal::entry("day_trading", ticker.clone(), Side::Buy)
                        .with_strength(strength)
                        .with_prices(Some(candle.close), Some(stop), Some(tp))
                        .with_metadata("variant", json!("crossover"))
                        .with_metadata("short_sma", json!(short_sma.to_string()))
                        .with_metadata("long_sma", json!(long_sma.to_string())),
                );

                self.position = Some(PositionState {
                    side: Side::Buy,
                    entry_price: candle.close,
                    entry_time: candle.timestamp,
                    highest_price: candle.high,
                    lowest_price: candle.low,
                });

                info!(short_sma = %short_sma, long_sma = %long_sma, price = %candle.close, "골든 크로스 - 매수");
            }
        }

        // 이전 SMA 저장
        self.prev_short_sma = Some(short_sma);
        self.prev_long_sma = Some(long_sma);

        signals
    }

    /// 거래량 급증 신호 생성.
    fn generate_volume_surge_signals(&mut self, candle: &CandleData) -> Vec<Signal> {
        let Some(config) = self.config.as_ref() else {
            return Vec::new();
        };
        let Some(ticker) = self.ticker.as_ref() else {
            return Vec::new();
        };
        let mut signals = Vec::new();

        // 거래량 히스토리 업데이트
        self.volumes.push_front(candle.volume);
        while self.volumes.len() > config.volume_surge_config.volume_period + 5 {
            self.volumes.pop_back();
        }

        // 캔들 히스토리 업데이트
        self.candles.push_front(candle.clone());
        while self.candles.len() > 50 {
            self.candles.pop_back();
        }

        // 기존 포지션 처리 (borrow 충돌 방지를 위해 복사)
        if let Some(mut pos) = self.position.clone() {
            // 최고가/최저가 업데이트
            if candle.high > pos.highest_price {
                pos.highest_price = candle.high;
            }
            if candle.low < pos.lowest_price {
                pos.lowest_price = candle.low;
            }
            // 업데이트된 pos를 다시 저장
            self.position = Some(pos.clone());

            // 청산 조건 체크
            if let Some((signal, pnl)) = self.check_volume_surge_exit(candle, &pos) {
                self.record_trade(pnl);
                signals.push(signal);
                self.position = None;
                return signals;
            }
            return signals;
        }

        // 진입 조건 확인
        if !self.can_enter() {
            return signals;
        }

        // 1. 거래량 급증 확인
        let avg_volume = match self.calculate_avg_volume() {
            Some(v) => v,
            None => return signals,
        };

        let is_volume_surge =
            candle.volume >= avg_volume * config.volume_surge_config.volume_multiplier;
        if !is_volume_surge {
            return signals;
        }

        // 2. 연속 상승봉 확인
        if !self.has_consecutive_up_candles() {
            return signals;
        }

        // 3. RSI 과열 아닌지 확인
        if let Some(rsi) = self.calculate_rsi() {
            if rsi >= config.volume_surge_config.rsi_overbought {
                return signals;
            }
        }

        // 매수 신호 생성
        let stop = candle.close * (dec!(1) - config.exit_config.stop_loss_pct / dec!(100));
        let tp = candle.close * (dec!(1) + config.exit_config.take_profit_pct / dec!(100));

        let strength = self.get_adjusted_strength(1.0);
        signals.push(
            Signal::entry("day_trading", ticker.clone(), Side::Buy)
                .with_strength(strength)
                .with_prices(Some(candle.close), Some(stop), Some(tp))
                .with_metadata("variant", json!("volume_surge"))
                .with_metadata("volume", json!(candle.volume.to_string()))
                .with_metadata("avg_volume", json!(avg_volume.to_string()))
                .with_metadata("rsi", json!(self.calculate_rsi())),
        );

        self.position = Some(PositionState {
            side: Side::Buy,
            entry_price: candle.close,
            entry_time: candle.timestamp,
            highest_price: candle.high,
            lowest_price: candle.low,
        });

        info!(
            volume = %candle.volume,
            avg_volume = %avg_volume,
            consecutive = config.volume_surge_config.consecutive_up_candles,
            "거래량 급증 - 매수"
        );

        signals
    }

    /// 일반 청산 조건 확인 (Breakout/Crossover용).
    /// 반환: (Signal, PnL) - 호출자가 record_trade를 처리해야 함.
    fn check_exit_conditions(
        &self,
        candle: &CandleData,
        pos: &PositionState,
    ) -> Option<(Signal, Decimal)> {
        let config = self.config.as_ref()?;
        let ticker = self.ticker.as_ref()?;

        let profit_pct = match pos.side {
            Side::Buy => (candle.close - pos.entry_price) / pos.entry_price * dec!(100),
            Side::Sell => (pos.entry_price - candle.close) / pos.entry_price * dec!(100),
        };

        let pnl = match pos.side {
            Side::Buy => candle.close - pos.entry_price,
            Side::Sell => pos.entry_price - candle.close,
        };

        // 손절
        if profit_pct <= -config.exit_config.stop_loss_pct {
            let exit_side = match pos.side {
                Side::Buy => Side::Sell,
                Side::Sell => Side::Buy,
            };

            return Some((
                Signal::exit("day_trading", ticker.clone(), exit_side)
                    .with_strength(1.0)
                    .with_prices(Some(candle.close), None, None)
                    .with_metadata("exit_reason", json!("stop_loss"))
                    .with_metadata("pnl_pct", json!(profit_pct.to_string())),
                pnl,
            ));
        }

        // 익절
        if profit_pct >= config.exit_config.take_profit_pct {
            let exit_side = match pos.side {
                Side::Buy => Side::Sell,
                Side::Sell => Side::Buy,
            };

            return Some((
                Signal::exit("day_trading", ticker.clone(), exit_side)
                    .with_strength(1.0)
                    .with_prices(Some(candle.close), None, None)
                    .with_metadata("exit_reason", json!("take_profit"))
                    .with_metadata("pnl_pct", json!(profit_pct.to_string())),
                pnl,
            ));
        }

        None
    }

    /// 거래량 급증 전략용 청산 조건 확인.
    /// 반환: (Signal, PnL) - 호출자가 record_trade를 처리해야 함.
    fn check_volume_surge_exit(
        &self,
        candle: &CandleData,
        pos: &PositionState,
    ) -> Option<(Signal, Decimal)> {
        let config = self.config.as_ref()?;
        let ticker = self.ticker.as_ref()?;

        let profit_pct = (candle.close - pos.entry_price) / pos.entry_price * dec!(100);
        let pnl = candle.close - pos.entry_price;

        // 1. 익절
        if profit_pct >= config.exit_config.take_profit_pct {
            return Some((
                Signal::exit("day_trading", ticker.clone(), Side::Sell)
                    .with_strength(1.0)
                    .with_prices(Some(candle.close), None, None)
                    .with_metadata("exit_reason", json!("take_profit"))
                    .with_metadata("pnl_pct", json!(profit_pct.to_string())),
                pnl,
            ));
        }

        // 2. 손절
        if profit_pct <= -config.exit_config.stop_loss_pct {
            return Some((
                Signal::exit("day_trading", ticker.clone(), Side::Sell)
                    .with_strength(1.0)
                    .with_prices(Some(candle.close), None, None)
                    .with_metadata("exit_reason", json!("stop_loss"))
                    .with_metadata("pnl_pct", json!(profit_pct.to_string())),
                pnl,
            ));
        }

        // 3. 트레일링 스톱 (수익 중일 때만)
        if profit_pct > Decimal::ZERO && pos.highest_price > Decimal::ZERO {
            let drop_from_high = (pos.highest_price - candle.close) / pos.highest_price * dec!(100);
            if drop_from_high >= config.exit_config.trailing_stop_pct {
                return Some((
                    Signal::exit("day_trading", ticker.clone(), Side::Sell)
                        .with_strength(1.0)
                        .with_prices(Some(candle.close), None, None)
                        .with_metadata("exit_reason", json!("trailing_stop"))
                        .with_metadata("drop_pct", json!(drop_from_high.to_string())),
                    pnl,
                ));
            }
        }

        // 4. 최대 보유 시간
        let hold_seconds = (candle.timestamp - pos.entry_time).num_seconds();
        let hold_minutes = (hold_seconds / 60) as u32;
        if hold_minutes >= config.volume_surge_config.max_hold_minutes {
            return Some((
                Signal::exit("day_trading", ticker.clone(), Side::Sell)
                    .with_strength(1.0)
                    .with_prices(Some(candle.close), None, None)
                    .with_metadata("exit_reason", json!("max_hold_time"))
                    .with_metadata("hold_minutes", json!(hold_minutes)),
                pnl,
            ));
        }

        // 5. 모멘텀 약화 (연속 2개 음봉 + 수익 중)
        if self.candles.len() >= 2 && profit_pct > Decimal::ZERO {
            let last_two_bearish = self.candles.iter().take(2).all(|c| c.close < c.open);
            if last_two_bearish {
                return Some((
                    Signal::exit("day_trading", ticker.clone(), Side::Sell)
                        .with_strength(1.0)
                        .with_prices(Some(candle.close), None, None)
                        .with_metadata("exit_reason", json!("momentum_weakening")),
                    pnl,
                ));
            }
        }

        None
    }

    /// 거래 기록.
    fn record_trade(&mut self, pnl: Decimal) {
        self.trades_count += 1;
        if pnl > Decimal::ZERO {
            self.wins += 1;
        } else {
            self.losses_count += 1;
        }
        self.total_pnl += pnl;
    }
}

impl Default for DayTradingStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for DayTradingStrategy {
    fn name(&self) -> &str {
        "Day Trading"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "변동성 돌파, SMA 크로스오버, 거래량 급증 전략을 통합한 일간 트레이딩 전략"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 1. config에서 variant 확인 (테스트 등에서 직접 JSON 전달 시)
        // 2. self.config에서 variant 확인 (팩토리 메서드 사용 시)
        // 3. 기본값: Breakout
        let variant = config
            .get("variant")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "Breakout" => Some(DayTradingVariant::Breakout),
                "Crossover" => Some(DayTradingVariant::Crossover),
                "VolumeSurge" => Some(DayTradingVariant::VolumeSurge),
                _ => None,
            })
            .or_else(|| self.config.as_ref().map(|c| c.variant))
            .unwrap_or(DayTradingVariant::Breakout);

        // variant에 따라 적절한 Config 타입으로 파싱
        let dt_config: DayTradingConfig = match variant {
            DayTradingVariant::Breakout => {
                let cfg: VolatilityBreakoutConfig = serde_json::from_value(config)?;
                cfg.into()
            }
            DayTradingVariant::Crossover => {
                let cfg: SmaCrossoverConfig = serde_json::from_value(config)?;
                cfg.into()
            }
            DayTradingVariant::VolumeSurge => {
                let cfg: VolumeSurgeStrategyConfig = serde_json::from_value(config)?;
                cfg.into()
            }
        };

        info!(
            variant = ?dt_config.variant,
            ticker = %dt_config.ticker,
            "DayTrading 전략 초기화"
        );

        self.ticker = Some(dt_config.ticker.clone());
        self.config = Some(dt_config);
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

        // borrow 충돌 방지를 위해 필요한 값 먼저 추출
        let (ticker, variant) = match &self.config {
            Some(c) => (c.ticker.clone(), c.variant),
            None => return Ok(vec![]),
        };

        // 티커 확인
        if data.ticker != ticker {
            return Ok(vec![]);
        }

        // 캔들 데이터 추출
        let candle = match &data.data {
            MarketDataType::Kline(kline) => CandleData {
                open: kline.open,
                high: kline.high,
                low: kline.low,
                close: kline.close,
                volume: kline.volume,
                timestamp: kline.open_time,
            },
            _ => return Ok(vec![]),
        };

        // 새 기간 확인 (Breakout 전략용)
        if variant == DayTradingVariant::Breakout && self.is_new_period(candle.timestamp) {
            self.on_period_close();
            self.current_candle = Some(candle.clone());
        } else if variant == DayTradingVariant::Breakout {
            // 현재 기간 업데이트
            if let Some(c) = &mut self.current_candle {
                c.high = c.high.max(candle.high);
                c.low = c.low.min(candle.low);
                c.close = candle.close;
                c.volume += candle.volume;
            }
        }

        // 변형에 따른 신호 생성
        let signals = match variant {
            DayTradingVariant::Breakout => self.generate_breakout_signals(&candle),
            DayTradingVariant::Crossover => self.generate_crossover_signals(&candle),
            DayTradingVariant::VolumeSurge => self.generate_volume_surge_signals(&candle),
        };

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!(side = ?order.side, quantity = %order.quantity, "주문 체결");
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!(quantity = %position.quantity, "포지션 업데이트");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let win_rate = if self.trades_count > 0 {
            (self.wins as f64 / self.trades_count as f64) * 100.0
        } else {
            0.0
        };

        info!(
            trades = self.trades_count,
            wins = self.wins,
            losses = self.losses_count,
            win_rate = %format!("{:.1}%", win_rate),
            total_pnl = %self.total_pnl,
            "DayTrading 전략 종료"
        );

        Ok(())
    }

    fn get_state(&self) -> Value {
        let variant = self.config.as_ref().map(|c| format!("{:?}", c.variant));

        json!({
            "variant": variant,
            "initialized": self.initialized,
            "has_position": self.position.is_some(),
            "position_side": self.position.as_ref().map(|p| format!("{:?}", p.side)),
            "trades_count": self.trades_count,
            "wins": self.wins,
            "losses": self.losses_count,
            "total_pnl": self.total_pnl.to_string(),
            "current_atr": self.current_atr.map(|v| v.to_string()),
            "prev_range": self.prev_range.map(|v| v.to_string()),
            "short_sma": self.prev_short_sma.map(|v| v.to_string()),
            "long_sma": self.prev_long_sma.map(|v| v.to_string()),
            "upper_breakout": self.upper_breakout.map(|v| v.to_string()),
            "lower_breakout": self.lower_breakout.map(|v| v.to_string()),
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into DayTrading strategy");
    }
}

// ============================================================================
// 전략 레지스트리 등록
// ============================================================================

use crate::register_strategy;

// 변동성 돌파 전략
register_strategy! {
    id: "volatility_breakout",
    aliases: ["vb", "breakout", "larry_williams"],
    name: "변동성 돌파",
    description: "Larry Williams 변동성 돌파 전략 (당일 시가 + 전일 범위 × K)",
    timeframe: "1d",
    tickers: [],
    category: Daily,
    markets: [Crypto, Stock],
    factory: DayTradingStrategy::breakout,
    config: VolatilityBreakoutConfig
}

// SMA 크로스오버 전략
register_strategy! {
    id: "sma_crossover",
    aliases: ["sma", "ma_crossover", "golden_cross"],
    name: "SMA 크로스오버",
    description: "단기/장기 이동평균 교차 매매 전략",
    timeframe: "1d",
    tickers: [],
    category: Daily,
    markets: [Crypto, Stock],
    factory: DayTradingStrategy::crossover,
    config: SmaCrossoverConfig
}

// 거래량 급증 전략
register_strategy! {
    id: "volume_surge",
    aliases: ["market_interest_day", "volume_spike"],
    name: "거래량 급증",
    description: "거래량 급증 + 연속 상승봉 패턴 포착 전략",
    timeframe: "1d",
    tickers: [],
    category: Daily,
    markets: [Crypto, Stock],
    factory: DayTradingStrategy::volume_surge,
    config: VolumeSurgeStrategyConfig
}
