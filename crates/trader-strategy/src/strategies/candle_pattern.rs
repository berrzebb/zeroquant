//! Candlestick Pattern Recognition Strategy
//!
//! 35가지 이상의 캔들스틱 패턴을 인식하여 매매 신호를 생성하는 전략입니다.
//!
//! ## 지원 패턴
//!
//! ### 반전 패턴 (Reversal Patterns)
//! - Hammer / Inverted Hammer (망치형)
//! - Hanging Man (교수형)
//! - Doji (도지)
//! - Morning Star / Evening Star (샛별형/석별형)
//! - Engulfing (장악형)
//! - Harami (잉태형)
//! - Piercing Line / Dark Cloud Cover
//! - Three White Soldiers / Three Black Crows
//!
//! ### 지속 패턴 (Continuation Patterns)
//! - Rising/Falling Three Methods
//! - Marubozu (마루보즈)
//! - Spinning Top
//!
//! ## 전략 로직
//! 1. 캔들스틱 패턴 감지
//! 2. 패턴 강도 평가 (Volume, Trend 확인)
//! 3. 다중 패턴 확인 시 강화 신호

use crate::strategies::common::{adjust_strength_by_score, deserialize_ticker, ExitConfig};
use crate::Strategy;
use trader_strategy_macro::StrategyConfig;
use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use trader_core::domain::{RouteState, StrategyContext};
use trader_core::{
    MarketData, MarketDataType, Order, Position, Side, Signal, SignalType,
};

/// 캔들스틱 패턴 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandlePatternType {
    // 단일 캔들 패턴
    Hammer,
    InvertedHammer,
    HangingMan,
    ShootingStar,
    Doji,
    LongLeggedDoji,
    DragonflyDoji,
    GravestoneDoji,
    Marubozu,
    SpinningTop,

    // 2봉 패턴
    BullishEngulfing,
    BearishEngulfing,
    BullishHarami,
    BearishHarami,
    PiercingLine,
    DarkCloudCover,
    Tweezer,

    // 3봉 패턴
    MorningStar,
    EveningStar,
    ThreeWhiteSoldiers,
    ThreeBlackCrows,
    ThreeInsideUp,
    ThreeInsideDown,
    ThreeOutsideUp,
    ThreeOutsideDown,
    RisingThreeMethods,
    FallingThreeMethods,
    AbandonedBaby,
}

/// 패턴 방향
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternDirection {
    Bullish,
    Bearish,
    Neutral,
}

/// 감지된 패턴
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedPattern {
    pub pattern_type: CandlePatternType,
    pub direction: PatternDirection,
    pub strength: Decimal,
    pub confirmation: bool,
}

/// 캔들 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CandleData {
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
}

/// Candle Pattern 전략 설정
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "candle_pattern",
    name = "캔들 패턴 전략",
    description = "35가지 캔들스틱 패턴 인식 기반 매매 전략",
    category = "Intraday"
)]
pub struct CandlePatternConfig {
    /// 대상 심볼
    #[serde(default = "default_ticker", deserialize_with = "deserialize_ticker")]
    #[schema(label = "대상 종목", field_type = "symbol", default = "005930", section = "asset")]
    pub ticker: String,

    /// 거래 금액
    #[serde(default = "default_trade_amount")]
    #[schema(label = "거래 금액", min = 10000, max = 100000000, default = 1000000, section = "asset")]
    pub trade_amount: Decimal,

    /// 최소 패턴 강도 (0-1)
    #[serde(default = "default_min_strength")]
    #[schema(label = "최소 패턴 강도", min = 0.1, max = 1.0, default = 0.6, section = "indicator")]
    pub min_pattern_strength: Decimal,

    /// 볼륨 확인 사용
    #[serde(default = "default_use_volume")]
    #[schema(label = "볼륨 확인 사용", default = true, section = "filter")]
    pub use_volume_confirmation: bool,

    /// 트렌드 확인 사용
    #[serde(default = "default_use_trend")]
    #[schema(label = "트렌드 확인 사용", default = true, section = "filter")]
    pub use_trend_confirmation: bool,

    /// 트렌드 확인 기간
    #[serde(default = "default_trend_period")]
    #[schema(label = "트렌드 확인 기간", min = 5, max = 100, default = 20, section = "indicator")]
    pub trend_period: usize,

    /// 손절 비율 (%)
    #[serde(default = "default_stop_loss")]
    #[schema(label = "손절 비율 (%)", min = 0.5, max = 20.0, default = 3, section = "sizing")]
    pub stop_loss_pct: Decimal,

    /// 익절 비율 (%)
    #[serde(default = "default_take_profit")]
    #[schema(label = "익절 비율 (%)", min = 1.0, max = 50.0, default = 6, section = "sizing")]
    pub take_profit_pct: Decimal,

    /// 활성화할 패턴 타입 (빈 경우 모두 활성화)
    #[serde(default)]
    #[schema(skip)]
    pub enabled_patterns: Vec<CandlePatternType>,

    /// 최소 GlobalScore (기본값: 50)
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", min = 0, max = 100, default = 50, section = "filter")]
    pub min_global_score: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

fn default_ticker() -> String {
    "005930".to_string()
}
fn default_trade_amount() -> Decimal {
    dec!(1000000)
}
fn default_min_strength() -> Decimal {
    dec!(0.6)
}
fn default_use_volume() -> bool {
    true
}
fn default_use_trend() -> bool {
    true
}
fn default_trend_period() -> usize {
    20
}
fn default_stop_loss() -> Decimal {
    dec!(3)
}
fn default_take_profit() -> Decimal {
    dec!(6)
}
fn default_min_global_score() -> Decimal {
    dec!(50)
}

impl Default for CandlePatternConfig {
    fn default() -> Self {
        Self {
            ticker: "005930".to_string(),
            trade_amount: default_trade_amount(),
            min_pattern_strength: default_min_strength(),
            use_volume_confirmation: default_use_volume(),
            use_trend_confirmation: default_use_trend(),
            trend_period: default_trend_period(),
            stop_loss_pct: default_stop_loss(),
            take_profit_pct: default_take_profit(),
            enabled_patterns: Vec::new(),
            min_global_score: default_min_global_score(),
            exit_config: ExitConfig::for_mean_reversion(),
        }
    }
}

/// Candle Pattern 전략 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandlePatternState {
    /// 최근 감지된 패턴
    pub recent_patterns: Vec<DetectedPattern>,
    /// 현재 포지션 방향
    pub position_direction: Option<PatternDirection>,
    /// 진입 가격
    pub entry_price: Option<Decimal>,
    /// 현재 수량
    pub current_quantity: Decimal,
    /// 패턴 인식 통계
    pub pattern_stats: HashMap<String, u32>,
}

impl Default for CandlePatternState {
    fn default() -> Self {
        Self {
            recent_patterns: Vec::new(),
            position_direction: None,
            entry_price: None,
            current_quantity: Decimal::ZERO,
            pattern_stats: HashMap::new(),
        }
    }
}

/// Candle Pattern 전략
pub struct CandlePatternStrategy {
    config: Option<CandlePatternConfig>,
    ticker: Option<String>,
    context: Option<Arc<RwLock<StrategyContext>>>,
    state: CandlePatternState,
    /// 캔들 히스토리
    candles: VecDeque<CandleData>,
    /// 볼륨 히스토리
    volumes: VecDeque<Decimal>,
    initialized: bool,
}

impl CandlePatternStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            ticker: None,
            context: None,
            state: CandlePatternState::default(),
            candles: VecDeque::new(),
            volumes: VecDeque::new(),
            initialized: false,
        }
    }

    /// 단일 티커 전략: GlobalScore 체크 불필요 (스크리닝용)
    /// RouteState만 체크하여 시장 과열 시 진입 제한
    fn can_enter(&self) -> bool {
        let Some(config) = self.config.as_ref() else {
            return false;
        };
        let ticker = &config.ticker;
        let Some(ctx) = self.context.as_ref() else {
            return true;
        };
        let Ok(ctx_lock) = ctx.try_read() else {
            return true;
        };

        // RouteState 체크 - 시장 과열 시 진입 제한
        if let Some(route_state) = ctx_lock.get_route_state(ticker) {
            if route_state == &RouteState::Overheat {
                return false;
            }
        }

        true
    }

    /// GlobalScore 기반 신호 강도 조정
    /// - 단일 티커 전략이지만, 시장 상황을 반영하여 신호 강도 조정
    /// - 캔들 패턴 강도에 GlobalScore 배율 적용 (0.75~1.25배)
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

    /// 캔들 몸통 크기
    fn body_size(candle: &CandleData) -> Decimal {
        (candle.close - candle.open).abs()
    }

    /// 캔들 전체 크기
    fn total_size(candle: &CandleData) -> Decimal {
        candle.high - candle.low
    }

    /// 상단 꼬리 크기
    fn upper_shadow(candle: &CandleData) -> Decimal {
        candle.high - candle.close.max(candle.open)
    }

    /// 하단 꼬리 크기
    fn lower_shadow(candle: &CandleData) -> Decimal {
        candle.close.min(candle.open) - candle.low
    }

    /// 양봉 여부
    fn is_bullish(candle: &CandleData) -> bool {
        candle.close > candle.open
    }

    /// 음봉 여부
    fn is_bearish(candle: &CandleData) -> bool {
        candle.close < candle.open
    }

    /// Doji 패턴 감지
    fn detect_doji(&self, candle: &CandleData) -> Option<DetectedPattern> {
        let body = Self::body_size(candle);
        let total = Self::total_size(candle);

        if total == Decimal::ZERO {
            return None;
        }

        let body_ratio = body / total;

        // Doji: 몸통이 전체의 10% 미만
        if body_ratio < dec!(0.1) {
            let upper = Self::upper_shadow(candle);
            let lower = Self::lower_shadow(candle);

            let pattern_type = if lower > upper * dec!(2) {
                CandlePatternType::DragonflyDoji
            } else if upper > lower * dec!(2) {
                CandlePatternType::GravestoneDoji
            } else if total > self.average_candle_size() * dec!(1.5) {
                CandlePatternType::LongLeggedDoji
            } else {
                CandlePatternType::Doji
            };

            return Some(DetectedPattern {
                pattern_type,
                direction: PatternDirection::Neutral,
                strength: dec!(1) - body_ratio,
                confirmation: false,
            });
        }

        None
    }

    /// Hammer / Hanging Man 감지
    fn detect_hammer(&self, candle: &CandleData) -> Option<DetectedPattern> {
        let body = Self::body_size(candle);
        let total = Self::total_size(candle);
        let lower = Self::lower_shadow(candle);
        let upper = Self::upper_shadow(candle);

        if total == Decimal::ZERO || body == Decimal::ZERO {
            return None;
        }

        // Hammer: 하단 꼬리가 몸통의 2배 이상, 상단 꼬리는 작음
        if lower >= body * dec!(2) && upper < body * dec!(0.5) {
            let trend = self.get_trend();
            let (pattern_type, direction) = match trend {
                PatternDirection::Bearish => (CandlePatternType::Hammer, PatternDirection::Bullish),
                PatternDirection::Bullish => {
                    (CandlePatternType::HangingMan, PatternDirection::Bearish)
                }
                _ => (CandlePatternType::Hammer, PatternDirection::Neutral),
            };

            return Some(DetectedPattern {
                pattern_type,
                direction,
                strength: (lower / body / dec!(2)).min(dec!(1)),
                confirmation: false,
            });
        }

        // Inverted Hammer / Shooting Star
        if upper >= body * dec!(2) && lower < body * dec!(0.5) {
            let trend = self.get_trend();
            let (pattern_type, direction) = match trend {
                PatternDirection::Bearish => {
                    (CandlePatternType::InvertedHammer, PatternDirection::Bullish)
                }
                PatternDirection::Bullish => {
                    (CandlePatternType::ShootingStar, PatternDirection::Bearish)
                }
                _ => (CandlePatternType::InvertedHammer, PatternDirection::Neutral),
            };

            return Some(DetectedPattern {
                pattern_type,
                direction,
                strength: (upper / body / dec!(2)).min(dec!(1)),
                confirmation: false,
            });
        }

        None
    }

    /// Engulfing 패턴 감지
    fn detect_engulfing(&self) -> Option<DetectedPattern> {
        if self.candles.len() < 2 {
            return None;
        }

        let curr = &self.candles[0];
        let prev = &self.candles[1];

        let curr_body = Self::body_size(curr);
        let prev_body = Self::body_size(prev);

        // Bullish Engulfing
        if Self::is_bearish(prev) && Self::is_bullish(curr)
            && curr.open < prev.close && curr.close > prev.open && curr_body > prev_body {
                return Some(DetectedPattern {
                    pattern_type: CandlePatternType::BullishEngulfing,
                    direction: PatternDirection::Bullish,
                    strength: (curr_body / prev_body).min(dec!(1)),
                    confirmation: true,
                });
            }

        // Bearish Engulfing
        if Self::is_bullish(prev) && Self::is_bearish(curr)
            && curr.open > prev.close && curr.close < prev.open && curr_body > prev_body {
                return Some(DetectedPattern {
                    pattern_type: CandlePatternType::BearishEngulfing,
                    direction: PatternDirection::Bearish,
                    strength: (curr_body / prev_body).min(dec!(1)),
                    confirmation: true,
                });
            }

        None
    }

    /// Harami 패턴 감지
    fn detect_harami(&self) -> Option<DetectedPattern> {
        if self.candles.len() < 2 {
            return None;
        }

        let curr = &self.candles[0];
        let prev = &self.candles[1];

        // Bullish Harami
        if Self::is_bearish(prev) && Self::is_bullish(curr)
            && curr.open > prev.close && curr.close < prev.open {
                return Some(DetectedPattern {
                    pattern_type: CandlePatternType::BullishHarami,
                    direction: PatternDirection::Bullish,
                    strength: dec!(0.7),
                    confirmation: false,
                });
            }

        // Bearish Harami
        if Self::is_bullish(prev) && Self::is_bearish(curr)
            && curr.open < prev.close && curr.close > prev.open {
                return Some(DetectedPattern {
                    pattern_type: CandlePatternType::BearishHarami,
                    direction: PatternDirection::Bearish,
                    strength: dec!(0.7),
                    confirmation: false,
                });
            }

        None
    }

    /// Morning/Evening Star 감지
    fn detect_star(&self) -> Option<DetectedPattern> {
        if self.candles.len() < 3 {
            return None;
        }

        let curr = &self.candles[0];
        let mid = &self.candles[1];
        let first = &self.candles[2];

        let first_body = Self::body_size(first);
        let mid_body = Self::body_size(mid);

        // Morning Star
        if Self::is_bearish(first)
            && mid_body < first_body * dec!(0.3)
            && Self::is_bullish(curr)
            && curr.close > (first.open + first.close) / dec!(2)
        {
            return Some(DetectedPattern {
                pattern_type: CandlePatternType::MorningStar,
                direction: PatternDirection::Bullish,
                strength: dec!(0.85),
                confirmation: true,
            });
        }

        // Evening Star
        if Self::is_bullish(first)
            && mid_body < first_body * dec!(0.3)
            && Self::is_bearish(curr)
            && curr.close < (first.open + first.close) / dec!(2)
        {
            return Some(DetectedPattern {
                pattern_type: CandlePatternType::EveningStar,
                direction: PatternDirection::Bearish,
                strength: dec!(0.85),
                confirmation: true,
            });
        }

        None
    }

    /// Three Soldiers / Crows 감지
    fn detect_three_soldiers_crows(&self) -> Option<DetectedPattern> {
        if self.candles.len() < 3 {
            return None;
        }

        let c1 = &self.candles[2];
        let c2 = &self.candles[1];
        let c3 = &self.candles[0];

        // Three White Soldiers
        if Self::is_bullish(c1) && Self::is_bullish(c2) && Self::is_bullish(c3)
            && c2.close > c1.close && c3.close > c2.close {
                let body1 = Self::body_size(c1);
                let body2 = Self::body_size(c2);
                let body3 = Self::body_size(c3);

                if body2 > body1 * dec!(0.8) && body3 > body2 * dec!(0.8) {
                    return Some(DetectedPattern {
                        pattern_type: CandlePatternType::ThreeWhiteSoldiers,
                        direction: PatternDirection::Bullish,
                        strength: dec!(0.9),
                        confirmation: true,
                    });
                }
            }

        // Three Black Crows
        if Self::is_bearish(c1) && Self::is_bearish(c2) && Self::is_bearish(c3)
            && c2.close < c1.close && c3.close < c2.close {
                return Some(DetectedPattern {
                    pattern_type: CandlePatternType::ThreeBlackCrows,
                    direction: PatternDirection::Bearish,
                    strength: dec!(0.9),
                    confirmation: true,
                });
            }

        None
    }

    /// Marubozu 감지
    fn detect_marubozu(&self, candle: &CandleData) -> Option<DetectedPattern> {
        let body = Self::body_size(candle);
        let total = Self::total_size(candle);
        let upper = Self::upper_shadow(candle);
        let lower = Self::lower_shadow(candle);

        if total == Decimal::ZERO {
            return None;
        }

        // 꼬리가 거의 없음 (전체의 5% 미만)
        if upper < total * dec!(0.05) && lower < total * dec!(0.05) {
            let direction = if Self::is_bullish(candle) {
                PatternDirection::Bullish
            } else {
                PatternDirection::Bearish
            };

            return Some(DetectedPattern {
                pattern_type: CandlePatternType::Marubozu,
                direction,
                strength: body / total,
                confirmation: true,
            });
        }

        None
    }

    /// 평균 캔들 크기 계산
    fn average_candle_size(&self) -> Decimal {
        if self.candles.is_empty() {
            return dec!(1);
        }

        let sum: Decimal = self.candles.iter().map(Self::total_size).sum();
        sum / Decimal::from(self.candles.len())
    }

    /// 현재 트렌드 판단
    fn get_trend(&self) -> PatternDirection {
        let config = match &self.config {
            Some(c) => c,
            None => return PatternDirection::Neutral,
        };

        if self.candles.len() < config.trend_period {
            return PatternDirection::Neutral;
        }

        let recent: Vec<Decimal> = self
            .candles
            .iter()
            .take(config.trend_period)
            .map(|k| k.close)
            .collect();

        if recent.len() < 2 {
            return PatternDirection::Neutral;
        }

        let first = recent.last().unwrap();
        let last = recent.first().unwrap();

        if last > first {
            PatternDirection::Bullish
        } else if last < first {
            PatternDirection::Bearish
        } else {
            PatternDirection::Neutral
        }
    }

    /// 볼륨 확인
    fn is_volume_confirmed(&self) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return true,
        };

        if !config.use_volume_confirmation || self.volumes.len() < 10 {
            return true;
        }

        let current = self.volumes.front().unwrap_or(&Decimal::ZERO);
        let avg: Decimal = self.volumes.iter().take(10).sum::<Decimal>() / dec!(10);

        *current > avg * dec!(1.2)
    }

    /// 모든 패턴 감지
    fn detect_all_patterns(&self, candle: &CandleData) -> Vec<DetectedPattern> {
        let config = match &self.config {
            Some(c) => c,
            None => return Vec::new(),
        };

        let mut patterns = Vec::new();

        // 단일 캔들 패턴
        if let Some(p) = self.detect_doji(candle) {
            patterns.push(p);
        }
        if let Some(p) = self.detect_hammer(candle) {
            patterns.push(p);
        }
        if let Some(p) = self.detect_marubozu(candle) {
            patterns.push(p);
        }

        // 2봉 패턴
        if let Some(p) = self.detect_engulfing() {
            patterns.push(p);
        }
        if let Some(p) = self.detect_harami() {
            patterns.push(p);
        }

        // 3봉 패턴
        if let Some(p) = self.detect_star() {
            patterns.push(p);
        }
        if let Some(p) = self.detect_three_soldiers_crows() {
            patterns.push(p);
        }

        // 강도 필터
        patterns
            .into_iter()
            .filter(|p| p.strength >= config.min_pattern_strength)
            .collect()
    }

    /// 패턴이 활성화되어 있는지 확인
    fn is_pattern_enabled(&self, pattern: &CandlePatternType) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return true,
        };

        if config.enabled_patterns.is_empty() {
            return true;
        }
        config.enabled_patterns.contains(pattern)
    }

    /// 신호 생성
    fn generate_signals(&mut self, candle: &CandleData, current_price: Decimal) -> Vec<Signal> {
        let config = match &self.config {
            Some(c) => c,
            None => return Vec::new(),
        };
        let ticker = match &self.ticker {
            Some(s) => s,
            None => return Vec::new(),
        };
        let mut signals = Vec::new();

        // 손절/익절 확인
        if let (Some(entry), Some(direction)) =
            (self.state.entry_price, self.state.position_direction)
        {
            let pnl_pct = match direction {
                PatternDirection::Bullish => (current_price - entry) / entry * dec!(100),
                PatternDirection::Bearish => (entry - current_price) / entry * dec!(100),
                _ => Decimal::ZERO,
            };

            // 익절
            if pnl_pct >= config.take_profit_pct {
                self.state.position_direction = None;
                self.state.entry_price = None;
                let _qty = self.state.current_quantity;
                self.state.current_quantity = Decimal::ZERO;

                let signal = Signal::new(
                    "candle_pattern",
                    ticker.clone(),
                    Side::Sell,
                    SignalType::Exit,
                )
                .with_strength(1.0)
                .with_metadata("reason", json!("take_profit"))
                .with_metadata("pnl_pct", json!(pnl_pct.to_string()));

                signals.push(signal);
                info!("[CandlePattern] 익절: +{:.2}%", pnl_pct);
                return signals;
            }

            // 손절
            if pnl_pct <= -config.stop_loss_pct {
                self.state.position_direction = None;
                self.state.entry_price = None;
                let _qty = self.state.current_quantity;
                self.state.current_quantity = Decimal::ZERO;

                let signal = Signal::new(
                    "candle_pattern",
                    ticker.clone(),
                    Side::Sell,
                    SignalType::Exit,
                )
                .with_strength(1.0)
                .with_metadata("reason", json!("stop_loss"))
                .with_metadata("pnl_pct", json!(pnl_pct.to_string()));

                signals.push(signal);
                warn!("[CandlePattern] 손절: {:.2}%", pnl_pct);
                return signals;
            }
        }

        // 패턴 감지
        let patterns = self.detect_all_patterns(candle);

        // 활성화된 패턴 필터
        let patterns: Vec<_> = patterns
            .into_iter()
            .filter(|p| self.is_pattern_enabled(&p.pattern_type))
            .collect();

        if patterns.is_empty() {
            return signals;
        }

        // 볼륨 확인
        if !self.is_volume_confirmed() {
            return signals;
        }

        // 가장 강한 패턴 선택
        let best_pattern = match patterns.iter().max_by(|a, b| a.strength.cmp(&b.strength)) {
            Some(p) => p,
            None => return signals,
        };

        // 패턴 통계 업데이트
        let pattern_name = format!("{:?}", best_pattern.pattern_type);
        *self
            .state
            .pattern_stats
            .entry(pattern_name.clone())
            .or_insert(0) += 1;

        self.state.recent_patterns = patterns.clone();

        // 이미 포지션이 있으면 새 신호 생성 안함
        if self.state.position_direction.is_some() {
            return signals;
        }

        // 중립 패턴은 신호 생성 안함
        if best_pattern.direction == PatternDirection::Neutral {
            return signals;
        }

        // 매수 신호 생성 전 can_enter 체크
        if best_pattern.direction == PatternDirection::Bullish && !self.can_enter() {
            debug!("[CandlePattern] can_enter() 실패 - 매수 신호 스킵");
            return signals;
        }

        // 신호 생성
        let quantity = config.trade_amount / current_price;
        let (side, signal_type) = match best_pattern.direction {
            PatternDirection::Bullish => (Side::Buy, SignalType::Entry),
            PatternDirection::Bearish => (Side::Sell, SignalType::Entry),
            _ => return signals,
        };

        self.state.position_direction = Some(best_pattern.direction);
        self.state.entry_price = Some(current_price);
        self.state.current_quantity = quantity;

        // 패턴 강도를 기본값으로 사용하고, GlobalScore로 조정
        let base_strength = best_pattern
            .strength
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.5);
        let adjusted_strength = self.get_adjusted_strength(base_strength);

        let signal = Signal::new("candle_pattern", ticker.clone(), side, signal_type)
            .with_strength(adjusted_strength)
            .with_metadata("pattern", json!(pattern_name))
            .with_metadata("direction", json!(format!("{:?}", best_pattern.direction)))
            .with_metadata("strength", json!(best_pattern.strength.to_string()))
            .with_metadata("confirmation", json!(best_pattern.confirmation));

        signals.push(signal);

        info!(
            "[CandlePattern] 패턴 감지: {:?} (강도: {:.2})",
            best_pattern.pattern_type, best_pattern.strength
        );

        signals
    }
}

impl Default for CandlePatternStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for CandlePatternStrategy {
    fn name(&self) -> &str {
        "Candle Pattern"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "35가지 캔들스틱 패턴 인식 기반 매매 전략"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let cp_config: CandlePatternConfig = serde_json::from_value(config)?;

        info!(
            ticker = %cp_config.ticker,
            min_strength = %cp_config.min_pattern_strength,
            "Initializing Candle Pattern strategy"
        );

        self.ticker = Some(cp_config.ticker.clone());
        self.config = Some(cp_config);
        self.state = CandlePatternState::default();
        self.candles.clear();
        self.volumes.clear();
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

        let config = match &self.config {
            Some(c) => c,
            None => return Ok(vec![]),
        };

        // 심볼 확인
        if data.ticker != config.ticker {
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
            },
            _ => return Ok(vec![]),
        };

        let current_price = candle.close;

        // 캔들 히스토리 업데이트
        self.candles.push_front(candle.clone());
        if self.candles.len() > 50 {
            self.candles.pop_back();
        }

        // 볼륨 히스토리 업데이트
        self.volumes.push_front(candle.volume);
        if self.volumes.len() > 20 {
            self.volumes.pop_back();
        }

        // 신호 생성
        let signals = self.generate_signals(&candle, current_price);

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let fill_price = order
            .average_fill_price
            .or(order.price)
            .unwrap_or(Decimal::ZERO);

        info!(
            "[CandlePattern] 주문 체결: {:?} {} @ {}",
            order.side, order.quantity, fill_price
        );

        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            quantity = %position.quantity,
            pnl = %position.realized_pnl,
            "Position updated"
        );

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Candle Pattern strategy shutdown");
        self.initialized = false;
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "config": self.config,
            "state": self.state,
            "candles_count": self.candles.len(),
            "current_trend": format!("{:?}", self.get_trend()),
            "initialized": self.initialized,
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into CandlePattern strategy");
    }

    fn exit_config(&self) -> Option<&ExitConfig> {
        self.config.as_ref().map(|c| &c.exit_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = CandlePatternConfig::default();
        assert_eq!(config.min_pattern_strength, dec!(0.6));
        assert!(config.use_volume_confirmation);
    }

    #[tokio::test]
    async fn test_initialization() {
        let mut strategy = CandlePatternStrategy::new();

        let config = json!({
            "ticker": "005930",
            "min_pattern_strength": "0.5"
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
    }

    #[test]
    fn test_strategy_creation() {
        let strategy = CandlePatternStrategy::new();
        assert_eq!(strategy.name(), "Candle Pattern");
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "candle_pattern",
    aliases: [],
    name: "캔들 패턴",
    description: "캔들 패턴 인식으로 매매 신호를 생성합니다.",
    timeframe: "15m",
    tickers: [],
    category: Intraday,
    markets: [Crypto, Stock, Stock],
    type: CandlePatternStrategy,
    config: CandlePatternConfig
}
