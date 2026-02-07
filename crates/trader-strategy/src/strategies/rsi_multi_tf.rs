//! 다중 타임프레임 RSI 전략.
//!
//! 3개의 타임프레임(일봉, 1시간봉, 5분봉)을 조합하여 RSI 기반 진입 타이밍을 찾습니다.
//!
//! # 전략 조건
//!
//! **매수 조건 (모든 조건 충족 시):**
//! 1. 일봉 RSI > 50 (상승 추세 확인)
//! 2. 1시간봉 RSI < 30 (과매도 구간)
//! 3. 5분봉 RSI 반등 (30 이하에서 30 이상으로 교차)
//!
//! **매도 조건 (청산):**
//! 1. 5분봉 RSI > 70 (과매수)
//! 2. 또는 1시간봉 RSI > 70 (더 큰 타임프레임에서 과매수)
//!
//! # 다중 타임프레임 활용
//!
//! - **일봉 (D1)**: 장기 추세 방향 확인 - "필터" 역할
//! - **1시간봉 (H1)**: 중기 과매도/과매수 구간 식별
//! - **5분봉 (M5)**: 정확한 진입 타이밍 (Primary 타임프레임)
//!
//! # 예시
//!
//! ```rust,ignore
//! use trader_strategy::strategies::rsi_multi_tf::RsiMultiTfStrategy;
//!
//! let mut strategy = RsiMultiTfStrategy::new();
//! // 백테스트 엔진에서 run_multi_timeframe() 호출 시 자동으로
//! // on_multi_timeframe_data()가 호출됩니다.
//! ```

use crate::strategies::common::{deserialize_ticker, ExitConfig};
use crate::{register_strategy, Strategy};
use trader_strategy_macro::StrategyConfig;
use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::{
    domain::{MultiTimeframeConfig, StrategyContext},
    Kline, MarketData, MarketDataType, Order, Position, RouteState, Side, Signal, Timeframe,
};

/// 다중 타임프레임 RSI 전략 설정.
#[derive(Debug, Clone, Deserialize, Serialize, StrategyConfig)]
#[strategy(
    id = "rsi_multi_tf",
    name = "다중 타임프레임 RSI 전략",
    description = "일봉/1시간봉/5분봉 RSI 조합 전략",
    category = "Intraday"
)]
pub struct RsiMultiTfConfig {
    /// 거래할 티커
    #[serde(deserialize_with = "deserialize_ticker")]
    #[schema(label = "거래 종목", field_type = "symbol", default = "005930", section = "asset")]
    pub ticker: String,

    /// 거래 금액 (호가 통화 기준)
    #[schema(label = "거래 금액", min = 100, max = 100000000, default = 1000000, section = "asset")]
    pub amount: Decimal,

    /// 일봉 RSI 추세 필터 임계값 (기본: 50)
    #[serde(default = "default_daily_trend_threshold")]
    #[schema(label = "일봉 RSI 임계값", min = 30, max = 70, default = 50, section = "indicator")]
    pub daily_trend_threshold: Decimal,

    /// 1시간봉 과매도 임계값 (기본: 35)
    #[serde(default = "default_h1_oversold")]
    #[schema(label = "1시간봉 과매도 임계값", min = 10, max = 40, default = 35, section = "indicator")]
    pub h1_oversold_threshold: Decimal,

    /// 5분봉 과매도 임계값 (기본: 35)
    #[serde(default = "default_m5_oversold")]
    #[schema(label = "5분봉 과매도 임계값", min = 10, max = 40, default = 35, section = "indicator")]
    pub m5_oversold_threshold: Decimal,

    /// 과매수 청산 임계값 (기본: 65)
    #[serde(default = "default_overbought")]
    #[schema(label = "과매수 청산 임계값", min = 60, max = 90, default = 65, section = "indicator")]
    pub overbought_threshold: Decimal,

    /// RSI 기간 (기본: 7)
    #[serde(default = "default_rsi_period")]
    #[schema(label = "RSI 기간", min = 5, max = 50, default = 7, section = "indicator")]
    pub rsi_period: usize,

    /// 손절 비율 (%)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(label = "손절 비율 (%)", min = 0.5, max = 20.0, section = "sizing")]
    pub stop_loss_pct: Option<Decimal>,

    /// 익절 비율 (%)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(label = "익절 비율 (%)", min = 1.0, max = 50.0, section = "sizing")]
    pub take_profit_pct: Option<Decimal>,

    /// 거래 후 쿨다운 기간 (Primary 캔들 수)
    #[serde(default = "default_cooldown")]
    #[schema(label = "쿨다운 캔들 수", min = 0, max = 20, default = 3, section = "timing")]
    pub cooldown_candles: usize,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,

    /// 최소 GlobalScore (기본값: 0)
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", min = 0, max = 100, default = 0, section = "filter")]
    pub min_global_score: Decimal,

    /// RouteState 필터 사용 여부 (기본값: false)
    #[serde(default = "default_use_route_filter")]
    #[schema(label = "RouteState 필터 사용", default = false, section = "filter")]
    pub use_route_filter: bool,
}

fn default_min_global_score() -> Decimal {
    dec!(0)
}

fn default_use_route_filter() -> bool {
    false
}

fn default_daily_trend_threshold() -> Decimal {
    dec!(50)
}
fn default_h1_oversold() -> Decimal {
    dec!(35)
}
fn default_m5_oversold() -> Decimal {
    dec!(35)
}
fn default_overbought() -> Decimal {
    dec!(65)
}
fn default_rsi_period() -> usize {
    7
}
fn default_cooldown() -> usize {
    3
}

impl Default for RsiMultiTfConfig {
    fn default() -> Self {
        Self {
            ticker: "BTC/USDT".to_string(),
            amount: dec!(100),
            daily_trend_threshold: dec!(50),
            h1_oversold_threshold: dec!(35),
            m5_oversold_threshold: dec!(35),
            overbought_threshold: dec!(65),
            rsi_period: 7,
            stop_loss_pct: Some(dec!(2)),   // 2% 손절
            take_profit_pct: Some(dec!(4)), // 4% 익절
            cooldown_candles: 3,
            exit_config: ExitConfig::for_momentum(),
            min_global_score: default_min_global_score(),
            use_route_filter: default_use_route_filter(),
        }
    }
}

/// 포지션 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionState {
    /// 포지션 없음
    Flat,
    /// 롱 포지션
    Long,
}

/// 타임프레임별 RSI 값
#[derive(Debug, Clone, Default)]
struct MultiTfRsiValues {
    /// 일봉 RSI
    daily: Option<Decimal>,
    /// 1시간봉 RSI
    hourly: Option<Decimal>,
    /// 5분봉 RSI
    m5: Option<Decimal>,
    /// 5분봉 이전 RSI (교차 감지용)
    m5_prev: Option<Decimal>,
}

/// 다중 타임프레임 RSI 전략.
///
/// 일봉/1시간봉/5분봉의 RSI를 조합하여 더 정교한 진입 타이밍을 찾습니다.
pub struct RsiMultiTfStrategy {
    /// 전략 설정
    config: Option<RsiMultiTfConfig>,

    /// 거래 중인 티커
    ticker: Option<String>,

    /// 전략 실행 컨텍스트
    context: Option<Arc<RwLock<StrategyContext>>>,

    /// 현재 포지션 상태
    position_state: PositionState,

    /// 진입 가격
    entry_price: Option<Decimal>,

    /// 현재 가격
    current_price: Option<Decimal>,

    /// 타임프레임별 RSI 값
    rsi_values: MultiTfRsiValues,

    /// 타임프레임별 최근 캔들 (RSI 계산용)
    candle_history: HashMap<Timeframe, Vec<Decimal>>, // close 가격만 저장

    /// 쿨다운 카운터
    cooldown_counter: usize,

    /// 총 실행된 거래 수
    trades_count: u32,

    /// 승리 횟수
    wins: u32,

    /// 손실 횟수
    losses: u32,

    /// 총 손익
    total_pnl: Decimal,

    /// 초기화 플래그
    initialized: bool,
}

impl RsiMultiTfStrategy {
    /// 새로운 다중 타임프레임 RSI 전략 인스턴스를 생성합니다.
    pub fn new() -> Self {
        Self {
            config: None,
            ticker: None,
            context: None,
            position_state: PositionState::Flat,
            entry_price: None,
            current_price: None,
            rsi_values: MultiTfRsiValues::default(),
            candle_history: HashMap::new(),
            cooldown_counter: 0,
            trades_count: 0,
            wins: 0,
            losses: 0,
            total_pnl: Decimal::ZERO,
            initialized: false,
        }
    }

    /// StrategyContext에서 RouteState 조회
    async fn get_route_state(&self, ticker: &str) -> Option<RouteState> {
        let ctx = self.context.as_ref()?;
        let ctx_guard = ctx.read().await;
        ctx_guard.get_route_state(ticker).cloned()
    }

    /// StrategyContext에서 GlobalScore 조회
    async fn get_global_score(&self, ticker: &str) -> Option<f64> {
        use rust_decimal::prelude::ToPrimitive;
        let ctx = self.context.as_ref()?;
        let ctx_guard = ctx.read().await;
        ctx_guard.get_global_score(ticker).map(|gs| {
            gs.overall_score.to_f64().unwrap_or(0.0)
        })
    }

    /// 컨텍스트 기반 진입 조건 확인 (RouteState + GlobalScore)
    async fn check_context_conditions(&self, config: &RsiMultiTfConfig) -> bool {
        let ticker = match &self.ticker {
            Some(t) => t.clone(),
            None => return false,
        };

        // GlobalScore 체크
        if let Some(score) = self.get_global_score(&ticker).await {
            use rust_decimal::prelude::ToPrimitive;
            let min_score = config.min_global_score.to_f64().unwrap_or(50.0);
            if score < min_score {
                debug!(
                    ticker,
                    score,
                    min_score,
                    "GlobalScore 미달 - 진입 거부"
                );
                return false;
            }
        }

        // RouteState 체크
        if config.use_route_filter {
            if let Some(route) = self.get_route_state(&ticker).await {
                match route {
                    RouteState::Attack | RouteState::Armed => {
                        // 진입 허용
                    }
                    RouteState::Neutral => {
                        debug!(ticker, ?route, "RouteState Neutral - 진입 보류");
                        return false;
                    }
                    RouteState::Wait | RouteState::Overheat => {
                        debug!(ticker, ?route, "RouteState 비호의적 - 진입 거부");
                        return false;
                    }
                }
            }
        }

        true
    }

    /// RSI 계산 (Wilder's Smoothing 사용)
    fn calculate_rsi(&self, closes: &[Decimal], period: usize) -> Option<Decimal> {
        if closes.len() < period + 1 {
            return None;
        }

        let mut gains = Vec::new();
        let mut losses = Vec::new();

        for i in 1..closes.len() {
            let change = closes[i] - closes[i - 1];
            if change > Decimal::ZERO {
                gains.push(change);
                losses.push(Decimal::ZERO);
            } else {
                gains.push(Decimal::ZERO);
                losses.push(change.abs());
            }
        }

        if gains.len() < period {
            return None;
        }

        // 첫 번째 평균
        let initial_avg_gain: Decimal =
            gains.iter().take(period).sum::<Decimal>() / Decimal::from(period);
        let initial_avg_loss: Decimal =
            losses.iter().take(period).sum::<Decimal>() / Decimal::from(period);

        // Wilder's Smoothing
        let mut avg_gain = initial_avg_gain;
        let mut avg_loss = initial_avg_loss;

        for i in period..gains.len() {
            avg_gain = (avg_gain * Decimal::from(period - 1) + gains[i]) / Decimal::from(period);
            avg_loss = (avg_loss * Decimal::from(period - 1) + losses[i]) / Decimal::from(period);
        }

        if avg_loss == Decimal::ZERO {
            return Some(dec!(100));
        }

        let rs = avg_gain / avg_loss;
        let rsi = dec!(100) - (dec!(100) / (Decimal::ONE + rs));

        Some(rsi)
    }

    /// Secondary 데이터에서 RSI 업데이트
    fn update_secondary_rsi(
        &mut self,
        secondary_data: &HashMap<Timeframe, Vec<Kline>>,
        period: usize,
    ) {
        // 일봉 RSI
        if let Some(daily_klines) = secondary_data.get(&Timeframe::D1) {
            if !daily_klines.is_empty() {
                let closes: Vec<Decimal> = daily_klines.iter().map(|k| k.close).collect();
                self.rsi_values.daily = self.calculate_rsi(&closes, period);
            }
        }

        // 1시간봉 RSI
        if let Some(hourly_klines) = secondary_data.get(&Timeframe::H1) {
            if !hourly_klines.is_empty() {
                let closes: Vec<Decimal> = hourly_klines.iter().map(|k| k.close).collect();
                self.rsi_values.hourly = self.calculate_rsi(&closes, period);
            }
        }
    }

    /// 매수 조건 확인
    fn should_enter_long(&self, config: &RsiMultiTfConfig) -> bool {
        let (daily, hourly, m5, m5_prev) = (
            self.rsi_values.daily,
            self.rsi_values.hourly,
            self.rsi_values.m5,
            self.rsi_values.m5_prev,
        );

        // 모든 RSI 값이 있어야 함
        let (daily_rsi, hourly_rsi, m5_rsi, m5_prev_rsi) = match (daily, hourly, m5, m5_prev) {
            (Some(d), Some(h), Some(m), Some(mp)) => (d, h, m, mp),
            _ => {
                debug!(
                    "RSI 값 부족: daily={:?}, hourly={:?}, m5={:?}, m5_prev={:?}",
                    daily, hourly, m5, m5_prev
                );
                return false;
            }
        };

        // 조건 1: 일봉 RSI > 50 (상승 추세)
        let daily_trend_ok = daily_rsi > config.daily_trend_threshold;

        // 조건 2: 1시간봉 RSI < 30 (과매도)
        let hourly_oversold = hourly_rsi < config.h1_oversold_threshold;

        // 조건 3: 5분봉 RSI 반등 (30 이하에서 30 이상으로 교차)
        let m5_bounce =
            m5_prev_rsi <= config.m5_oversold_threshold && m5_rsi > config.m5_oversold_threshold;

        debug!(
            "매수 조건 체크: daily_trend={} ({}), h1_oversold={} ({}), m5_bounce={} ({} -> {})",
            daily_trend_ok, daily_rsi, hourly_oversold, hourly_rsi, m5_bounce, m5_prev_rsi, m5_rsi
        );

        daily_trend_ok && hourly_oversold && m5_bounce
    }

    /// 청산 조건 확인
    fn should_exit(&self, config: &RsiMultiTfConfig) -> bool {
        let (hourly, m5) = (self.rsi_values.hourly, self.rsi_values.m5);

        // 5분봉 과매수
        if let Some(m5_rsi) = m5 {
            if m5_rsi > config.overbought_threshold {
                debug!("5분봉 과매수 청산: RSI={}", m5_rsi);
                return true;
            }
        }

        // 1시간봉 과매수
        if let Some(hourly_rsi) = hourly {
            if hourly_rsi > config.overbought_threshold {
                debug!("1시간봉 과매수 청산: RSI={}", hourly_rsi);
                return true;
            }
        }

        // 손절/익절 확인
        if let (Some(entry), Some(current)) = (self.entry_price, self.current_price) {
            let pnl_pct = (current - entry) / entry * dec!(100);

            if let Some(sl_pct) = config.stop_loss_pct {
                if pnl_pct < -sl_pct {
                    debug!("손절 청산: PnL={}%", pnl_pct);
                    return true;
                }
            }

            if let Some(tp_pct) = config.take_profit_pct {
                if pnl_pct > tp_pct {
                    debug!("익절 청산: PnL={}%", pnl_pct);
                    return true;
                }
            }
        }

        false
    }

    /// 전략 통계 반환
    pub fn get_stats(&self) -> Value {
        let win_rate = if self.trades_count > 0 {
            Decimal::from(self.wins) / Decimal::from(self.trades_count) * dec!(100)
        } else {
            Decimal::ZERO
        };

        json!({
            "position_state": format!("{:?}", self.position_state),
            "entry_price": self.entry_price,
            "current_price": self.current_price,
            "rsi": {
                "daily": self.rsi_values.daily,
                "hourly": self.rsi_values.hourly,
                "m5": self.rsi_values.m5,
                "m5_prev": self.rsi_values.m5_prev
            },
            "trades_count": self.trades_count,
            "wins": self.wins,
            "losses": self.losses,
            "win_rate": win_rate,
            "total_pnl": self.total_pnl,
            "cooldown_remaining": self.cooldown_counter
        })
    }
}

impl Default for RsiMultiTfStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for RsiMultiTfStrategy {
    fn name(&self) -> &str {
        "RsiMultiTf"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "다중 타임프레임 RSI 전략 (일봉/1시간봉/5분봉)"
    }

    /// 다중 타임프레임 설정 반환
    fn multi_timeframe_config(&self) -> Option<MultiTimeframeConfig> {
        Some(
            MultiTimeframeConfig::new()
                .with_primary(Timeframe::M5)
                .with_timeframe(Timeframe::M5, 100)   // Primary: 5분봉 100개
                .with_timeframe(Timeframe::H1, 24)    // Secondary: 1시간봉 24개
                .with_timeframe(Timeframe::D1, 30), // Secondary: 일봉 30개
        )
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let parsed_config: RsiMultiTfConfig = serde_json::from_value(config)?;

        info!(
            "[{}] 전략 초기화: ticker={}, daily_threshold={}, h1_oversold={}, m5_oversold={}, overbought={}",
            self.name(),
            parsed_config.ticker,
            parsed_config.daily_trend_threshold,
            parsed_config.h1_oversold_threshold,
            parsed_config.m5_oversold_threshold,
            parsed_config.overbought_threshold
        );

        self.ticker = Some(parsed_config.ticker.clone());
        self.config = Some(parsed_config);
        self.position_state = PositionState::Flat;
        self.entry_price = None;
        self.current_price = None;
        self.rsi_values = MultiTfRsiValues::default();
        self.candle_history.clear();
        self.cooldown_counter = 0;
        self.initialized = true;

        Ok(())
    }

    /// 단일 타임프레임 데이터 처리 (기본 동작)
    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        // 다중 타임프레임 전략이지만, 단일 TF로 호출되면 Primary만 처리
        let config = match &self.config {
            Some(c) => c.clone(),
            None => return Ok(vec![]),
        };

        let kline = match &data.data {
            MarketDataType::Kline(k) => k.clone(),
            _ => return Ok(vec![]),
        };

        // Primary 캔들 히스토리 업데이트
        {
            let history = self.candle_history.entry(Timeframe::M5).or_default();
            history.push(kline.close);
            if history.len() > 100 {
                history.remove(0);
            }
        }

        // 현재 가격 업데이트
        self.current_price = Some(kline.close);

        // M5 RSI 계산 (히스토리를 clone하여 borrow 충돌 방지)
        let history_clone = self
            .candle_history
            .get(&Timeframe::M5)
            .cloned()
            .unwrap_or_default();
        self.rsi_values.m5_prev = self.rsi_values.m5;
        self.rsi_values.m5 = self.calculate_rsi(&history_clone, config.rsi_period);

        // 쿨다운 감소
        if self.cooldown_counter > 0 {
            self.cooldown_counter -= 1;
            return Ok(vec![]);
        }

        // 신호 없음 (Secondary 데이터 없이는 진입하지 않음)
        debug!("단일 타임프레임 모드: Secondary 데이터 없이 신호 생성하지 않음");
        Ok(vec![])
    }

    /// 다중 타임프레임 데이터 처리
    async fn on_multi_timeframe_data(
        &mut self,
        primary_data: &MarketData,
        secondary_data: &HashMap<Timeframe, Vec<Kline>>,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        let config = match &self.config {
            Some(c) => c.clone(),
            None => return Ok(vec![]),
        };

        let kline = match &primary_data.data {
            MarketDataType::Kline(k) => k.clone(),
            _ => return Ok(vec![]),
        };

        // Primary 캔들 히스토리 업데이트
        {
            let history = self.candle_history.entry(Timeframe::M5).or_default();
            history.push(kline.close);
            if history.len() > 100 {
                history.remove(0);
            }
        }

        // 현재 가격 업데이트
        self.current_price = Some(kline.close);

        // M5 RSI 업데이트 (히스토리를 clone하여 borrow 충돌 방지)
        let history_clone = self
            .candle_history
            .get(&Timeframe::M5)
            .cloned()
            .unwrap_or_default();
        self.rsi_values.m5_prev = self.rsi_values.m5;
        self.rsi_values.m5 = self.calculate_rsi(&history_clone, config.rsi_period);

        // Secondary RSI 업데이트
        self.update_secondary_rsi(secondary_data, config.rsi_period);

        // 쿨다운 감소
        if self.cooldown_counter > 0 {
            self.cooldown_counter -= 1;
            return Ok(vec![]);
        }

        let mut signals = vec![];
        let ticker = self.ticker.clone().unwrap_or_else(|| kline.ticker.clone());

        match self.position_state {
            PositionState::Flat => {
                // 컨텍스트 조건 체크 (RouteState + GlobalScore)
                if !self.check_context_conditions(&config).await {
                    return Ok(vec![]);
                }

                if self.should_enter_long(&config) {
                    info!(
                        "[{}] 매수 신호: ticker={}, price={}, daily_rsi={:?}, h1_rsi={:?}, m5_rsi={:?}",
                        self.name(),
                        ticker,
                        kline.close,
                        self.rsi_values.daily,
                        self.rsi_values.hourly,
                        self.rsi_values.m5
                    );

                    signals.push(
                        Signal::entry(self.name(), ticker.clone(), Side::Buy)
                            .with_prices(Some(kline.close), None, None)
                            .with_strength(1.0)
                            .with_metadata(
                                "reason",
                                serde_json::json!(format!(
                                    "Multi-TF RSI Entry: D1={:.1}, H1={:.1}, M5={:.1}",
                                    self.rsi_values.daily.unwrap_or_default(),
                                    self.rsi_values.hourly.unwrap_or_default(),
                                    self.rsi_values.m5.unwrap_or_default()
                                )),
                            ),
                    );

                    self.position_state = PositionState::Long;
                    self.entry_price = Some(kline.close);
                    self.cooldown_counter = config.cooldown_candles;
                }
            }
            PositionState::Long => {
                if self.should_exit(&config) {
                    let pnl = if let Some(entry) = self.entry_price {
                        kline.close - entry
                    } else {
                        Decimal::ZERO
                    };

                    info!(
                        "[{}] 청산 신호: ticker={}, price={}, pnl={}",
                        self.name(),
                        ticker,
                        kline.close,
                        pnl
                    );

                    signals.push(
                        Signal::exit(self.name(), ticker.clone(), Side::Sell)
                            .with_prices(Some(kline.close), None, None)
                            .with_metadata(
                                "reason",
                                serde_json::json!(format!(
                                    "Multi-TF RSI Exit: H1={:.1}, M5={:.1}",
                                    self.rsi_values.hourly.unwrap_or_default(),
                                    self.rsi_values.m5.unwrap_or_default()
                                )),
                            ),
                    );

                    // 통계 업데이트
                    self.trades_count += 1;
                    if pnl > Decimal::ZERO {
                        self.wins += 1;
                    } else {
                        self.losses += 1;
                    }
                    self.total_pnl += pnl;

                    self.position_state = PositionState::Flat;
                    self.entry_price = None;
                    self.cooldown_counter = config.cooldown_candles;
                }
            }
        }

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        _order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 포지션 상태 동기화
        if position.quantity > Decimal::ZERO {
            self.position_state = PositionState::Long;
            if self.entry_price.is_none() {
                self.entry_price = Some(position.entry_price);
            }
        } else {
            self.position_state = PositionState::Flat;
            self.entry_price = None;
        }
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "[{}] 전략 종료: trades={}, wins={}, losses={}, total_pnl={}",
            self.name(),
            self.trades_count,
            self.wins,
            self.losses,
            self.total_pnl
        );
        Ok(())
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
    }

    fn exit_config(&self) -> Option<&ExitConfig> {
        self.config.as_ref().map(|c| &c.exit_config)
    }

    fn get_state(&self) -> Value {
        self.get_stats()
    }

    fn save_state(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let state = json!({
            "position_state": format!("{:?}", self.position_state),
            "entry_price": self.entry_price,
            "trades_count": self.trades_count,
            "wins": self.wins,
            "losses": self.losses,
            "total_pnl": self.total_pnl.to_string(),
            "rsi_values": {
                "m5_prev": self.rsi_values.m5_prev.map(|v| v.to_string())
            }
        });
        Ok(serde_json::to_vec(&state)?)
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let state: Value = serde_json::from_slice(data)?;

        if let Some(pos_str) = state.get("position_state").and_then(|v| v.as_str()) {
            self.position_state = match pos_str {
                "Long" => PositionState::Long,
                _ => PositionState::Flat,
            };
        }

        if let Some(price) = state.get("entry_price").and_then(|v| v.as_str()) {
            self.entry_price = price.parse().ok();
        }

        if let Some(count) = state.get("trades_count").and_then(|v| v.as_u64()) {
            self.trades_count = count as u32;
        }

        if let Some(wins) = state.get("wins").and_then(|v| v.as_u64()) {
            self.wins = wins as u32;
        }

        if let Some(losses) = state.get("losses").and_then(|v| v.as_u64()) {
            self.losses = losses as u32;
        }

        if let Some(pnl) = state.get("total_pnl").and_then(|v| v.as_str()) {
            self.total_pnl = pnl.parse().unwrap_or(Decimal::ZERO);
        }

        if let Some(rsi_obj) = state.get("rsi_values") {
            if let Some(m5_prev) = rsi_obj.get("m5_prev").and_then(|v| v.as_str()) {
                self.rsi_values.m5_prev = m5_prev.parse().ok();
            }
        }

        Ok(())
    }
}

// 전략 등록 (다중 타임프레임 매크로 패턴 사용)
register_strategy! {
    id: "rsi_multi_tf",
    aliases: ["rsi_mtf", "multi_rsi"],
    name: "다중 타임프레임 RSI",
    description: "일봉/1시간봉/5분봉 RSI 조합 전략",
    timeframe: "5m",
    secondary_timeframes: ["1h", "1d"],
    tickers: [],
    category: Intraday,
    markets: [Crypto, Stock],
    type: RsiMultiTfStrategy,
    config: RsiMultiTfConfig
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RsiMultiTfConfig::default();
        assert_eq!(config.daily_trend_threshold, dec!(50));
        assert_eq!(config.h1_oversold_threshold, dec!(35));
        assert_eq!(config.m5_oversold_threshold, dec!(35));
        assert_eq!(config.overbought_threshold, dec!(65));
        assert_eq!(config.rsi_period, 7);
    }

    #[test]
    fn test_multi_timeframe_config() {
        let strategy = RsiMultiTfStrategy::new();
        let mtf_config = strategy.multi_timeframe_config();

        assert!(mtf_config.is_some());
        let config = mtf_config.unwrap();
        assert_eq!(config.get_primary_timeframe(), Some(Timeframe::M5));
        assert!(config.timeframes.contains_key(&Timeframe::H1));
        assert!(config.timeframes.contains_key(&Timeframe::D1));
        assert_eq!(config.get_candle_count(Timeframe::H1), 24);
        assert_eq!(config.get_candle_count(Timeframe::D1), 30);
    }

    #[test]
    fn test_rsi_calculation() {
        let strategy = RsiMultiTfStrategy::new();

        // 상승 추세 데이터
        let closes = vec![
            dec!(100),
            dec!(101),
            dec!(102),
            dec!(103),
            dec!(104),
            dec!(105),
            dec!(106),
            dec!(107),
            dec!(108),
            dec!(109),
            dec!(110),
            dec!(111),
            dec!(112),
            dec!(113),
            dec!(114),
            dec!(115),
        ];

        let rsi = strategy.calculate_rsi(&closes, 14);
        assert!(rsi.is_some());
        assert!(rsi.unwrap() > dec!(70)); // 상승 추세면 RSI > 70
    }

    #[tokio::test]
    async fn test_strategy_initialization() {
        let mut strategy = RsiMultiTfStrategy::new();

        let config = serde_json::json!({
            "ticker": "ETH/USDT",
            "amount": "500"
        });

        let result = strategy.initialize(config).await;
        assert!(result.is_ok());
        assert_eq!(strategy.ticker.as_deref(), Some("ETH/USDT"));
        assert!(strategy.initialized);
    }
}
