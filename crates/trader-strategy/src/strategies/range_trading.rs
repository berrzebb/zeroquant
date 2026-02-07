//! Range Trading 전략 v2.0
//!
//! ## 핵심 아이디어
//!
//! 가격대를 여러 구간으로 나누어 구간 변동 시 매매하는 장기 투자 전략.
//!
//! ## 진입 조건
//!
//! 1. **구간 상승**: 가격이 상위 구간으로 진입 시 매수
//! 2. **구간 하락**: 가격이 하위 구간으로 진입 시 매도
//! 3. **MA 필터**: 매수 시 MA20 상회, 매도 시 MA5 하회 조건
//!
//! ## 스크리닝 연동
//!
//! - `GlobalScore`: 최소 점수 필터
//! - `MarketRegime`: Downtrend 시 매수 금지
//! - `RouteState`: Attack/Armed 시 적극 매수

use crate::strategies::common::ExitConfig;
use crate::Strategy;
use async_trait::async_trait;
use trader_strategy_macro::StrategyConfig;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::{
    domain::{MarketRegime, RouteState, StrategyContext},
    types::Timeframe,
    Kline, MarketData, MarketDataType, Order, Position, Side, Signal, SignalType,
};

// ============================================================================
// 설정 (Config)
// ============================================================================

/// 구간분할 전략 설정
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "range_trading",
    name = "구간분할 매매 전략",
    description = "가격대 구간별 분할 매수/매도 전략",
    category = "Daily"
)]
pub struct RangeTradingConfig {
    /// 대상 티커
    #[serde(default = "default_ticker")]
    #[schema(label = "대상 종목", field_type = "symbol", default = "005930", section = "asset")]
    pub ticker: String,

    /// 구간 분할 수 (기본: 15)
    #[serde(default = "default_div_num")]
    #[schema(label = "구간 분할 수", min = 5, max = 50, default = 15, section = "indicator")]
    pub div_num: usize,

    /// 구간 계산 기간 (기본: 20일)
    #[serde(default = "default_target_period")]
    #[schema(label = "구간 계산 기간 (일)", min = 5, max = 100, default = 20, section = "indicator")]
    pub target_period: usize,

    /// MA 필터 사용 여부
    #[serde(default = "default_use_ma_filter")]
    #[schema(label = "MA 필터 사용", default = true, section = "filter")]
    pub use_ma_filter: bool,

    /// 매수 MA 기간 (기본: 20)
    #[serde(default = "default_buy_ma_period")]
    #[schema(label = "매수 MA 기간", min = 5, max = 60, default = 20, section = "indicator")]
    pub buy_ma_period: usize,

    /// 매도 MA 기간 (기본: 5)
    #[serde(default = "default_sell_ma_period")]
    #[schema(label = "매도 MA 기간", min = 3, max = 30, default = 5, section = "indicator")]
    pub sell_ma_period: usize,

    /// 최소 GlobalScore
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
fn default_div_num() -> usize {
    15
}
fn default_target_period() -> usize {
    20
}
fn default_use_ma_filter() -> bool {
    true
}
fn default_buy_ma_period() -> usize {
    20
}
fn default_sell_ma_period() -> usize {
    5
}
fn default_min_global_score() -> Decimal {
    dec!(50)
}

impl Default for RangeTradingConfig {
    fn default() -> Self {
        Self {
            ticker: "005930".to_string(),
            div_num: default_div_num(),
            target_period: default_target_period(),
            use_ma_filter: default_use_ma_filter(),
            buy_ma_period: default_buy_ma_period(),
            sell_ma_period: default_sell_ma_period(),
            min_global_score: default_min_global_score(),
            exit_config: ExitConfig::for_mean_reversion(),
        }
    }
}

// ============================================================================
// 전략 상태
// ============================================================================

/// 전략 상태
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RangeTradingState {
    /// 현재 구간 (1 ~ div_num)
    pub current_zone: Option<usize>,
    /// 이전 구간
    pub prev_zone: Option<usize>,
    /// 구간 상한
    pub zone_high: Option<Decimal>,
    /// 구간 하한
    pub zone_low: Option<Decimal>,
    /// 구간 간격
    pub zone_gap: Option<Decimal>,
    /// 거래 횟수
    pub trades_count: u32,
}

// ============================================================================
// 전략 구현
// ============================================================================

/// Range Trading Strategy
pub struct RangeTradingStrategy {
    config: Option<RangeTradingConfig>,
    state: RangeTradingState,
    context: Option<Arc<RwLock<StrategyContext>>>,
}

impl RangeTradingStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            state: RangeTradingState::default(),
            context: None,
        }
    }

    // ========================================================================
    // StrategyContext 헬퍼
    // ========================================================================

    /// StrategyContext에서 klines 가져오기
    fn get_klines(&self) -> Vec<Kline> {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return vec![],
        };
        let ctx = match self.context.as_ref() {
            Some(c) => c,
            None => return vec![],
        };
        let ctx_lock = match ctx.try_read() {
            Ok(l) => l,
            Err(_) => return vec![],
        };
        ctx_lock.get_klines(&config.ticker, Timeframe::D1).to_vec()
    }

    /// 충분한 데이터가 있는지 확인
    fn has_sufficient_data(&self) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return false,
        };
        let klines = self.get_klines();
        let required_len = config.target_period.max(config.buy_ma_period);
        klines.len() >= required_len
    }

    // ========================================================================
    // 스크리닝 연동 헬퍼
    // ========================================================================

    /// GlobalScore 확인
    fn check_global_score(&self) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return true,
        };

        let ctx = match self.context.as_ref() {
            Some(c) => c,
            None => return true,
        };

        let ctx_lock = match ctx.try_read() {
            Ok(l) => l,
            Err(_) => return true,
        };

        if let Some(score) = ctx_lock.get_global_score(&config.ticker) {
            if score.overall_score < config.min_global_score {
                debug!(
                    ticker = %config.ticker,
                    score = %score.overall_score,
                    min = %config.min_global_score,
                    "GlobalScore 미달"
                );
                return false;
            }
        }
        true
    }

    /// MarketRegime 확인
    fn get_regime(&self) -> Option<MarketRegime> {
        let config = self.config.as_ref()?;
        let ctx = self.context.as_ref()?;
        let ctx_lock = ctx.try_read().ok()?;
        ctx_lock.get_market_regime(&config.ticker).copied()
    }

    /// RouteState 확인
    fn get_route_state(&self) -> Option<RouteState> {
        let config = self.config.as_ref()?;
        let ctx = self.context.as_ref()?;
        let ctx_lock = ctx.try_read().ok()?;
        ctx_lock.get_route_state(&config.ticker).copied()
    }

    /// 매수 가능 여부 (스크리닝 기반)
    fn can_buy(&self) -> bool {
        // GlobalScore 체크
        if !self.check_global_score() {
            return false;
        }

        // MarketRegime 체크 - Downtrend면 매수 금지
        if let Some(regime) = self.get_regime() {
            if regime == MarketRegime::Downtrend {
                debug!(regime = ?regime, "Downtrend - 매수 금지");
                return false;
            }
        }

        // RouteState가 Attack/Armed면 적극 매수
        if let Some(route) = self.get_route_state() {
            match route {
                RouteState::Attack | RouteState::Armed => {
                    debug!(route = ?route, "적극 매수 모드");
                    return true;
                }
                RouteState::Overheat | RouteState::Wait => {
                    debug!(route = ?route, "매수 대기");
                    return false;
                }
                _ => {}
            }
        }

        true
    }

    // ========================================================================
    // 핵심 로직
    // ========================================================================

    /// 이동평균 계산 (StrategyContext 기반)
    fn calculate_ma(&self, period: usize) -> Option<Decimal> {
        let klines = self.get_klines();
        if klines.len() < period {
            return None;
        }
        // 최신 데이터부터 period개 사용
        let sum: Decimal = klines.iter().rev().take(period).map(|k| k.close).sum();
        Some(sum / Decimal::from(period))
    }

    /// 구간 정보 업데이트 (최근 target_period일 기준, StrategyContext 기반)
    fn update_zone_info(&mut self) {
        let config = match &self.config {
            Some(c) => c.clone(),
            None => return,
        };

        let klines = self.get_klines();
        if klines.len() < config.target_period {
            return;
        }

        // 최근 target_period일의 최고가/최저가
        let recent: Vec<&Kline> = klines.iter().rev().take(config.target_period).collect();
        let high = recent.iter().map(|k| k.high).max();
        let low = recent.iter().map(|k| k.low).min();

        if let (Some(h), Some(l)) = (high, low) {
            if h > l {
                let gap = (h - l) / Decimal::from(config.div_num);
                self.state.zone_high = Some(h);
                self.state.zone_low = Some(l);
                self.state.zone_gap = Some(gap);
            }
        }
    }

    /// 현재 가격의 구간 계산 (1 ~ div_num)
    ///
    /// **수정됨**: 경계값 포함 (`<=` 사용)
    fn get_current_zone(&self, price: Decimal) -> Option<usize> {
        let config = self.config.as_ref()?;
        let zone_low = self.state.zone_low?;
        let zone_gap = self.state.zone_gap?;

        if zone_gap.is_zero() {
            return None;
        }

        // 구간 계산 (경계값 포함: <=)
        for step in 1..=config.div_num {
            let zone_upper = zone_low + zone_gap * Decimal::from(step);
            // 상한 경계 포함 (<=)
            if price <= zone_upper {
                return Some(step);
            }
        }

        // 최고가 초과 시 최대 구간
        Some(config.div_num)
    }

    /// MA 필터 확인 (매수/매도, StrategyContext 기반)
    fn check_ma_filter(&self, is_buy: bool) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return true,
        };

        if !config.use_ma_filter {
            return true;
        }

        let klines = self.get_klines();
        let current_price = match klines.last() {
            Some(k) => k.close,
            None => return false,
        };

        if is_buy {
            // 매수: 가격 > MA(buy_ma_period)
            if let Some(ma) = self.calculate_ma(config.buy_ma_period) {
                return current_price > ma;
            }
        } else {
            // 매도: 가격 < MA(sell_ma_period)
            if let Some(ma) = self.calculate_ma(config.sell_ma_period) {
                return current_price < ma;
            }
        }

        false
    }
}

impl Default for RangeTradingStrategy {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Strategy Trait 구현
// ============================================================================

#[async_trait]
impl Strategy for RangeTradingStrategy {
    fn name(&self) -> &str {
        "RangeTrading"
    }

    fn version(&self) -> &str {
        "2.0.0"
    }

    fn description(&self) -> &str {
        "구간분할 매매 전략 (스크리닝 연동)"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let cfg: RangeTradingConfig = serde_json::from_value(config)?;

        info!(
            ticker = %cfg.ticker,
            div_num = cfg.div_num,
            target_period = cfg.target_period,
            "전략 초기화"
        );

        self.config = Some(cfg);
        self.state = RangeTradingState::default();

        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        let config = match &self.config {
            Some(c) => c.clone(),
            None => return Ok(vec![]),
        };

        // 해당 티커인지 확인
        if !data.ticker.starts_with(&config.ticker) {
            return Ok(vec![]);
        }

        // 가격 추출 (마켓 데이터에서)
        let price = match &data.data {
            MarketDataType::Kline(k) => k.close,
            MarketDataType::Ticker(t) => t.last,
            MarketDataType::Trade(t) => t.price,
            _ => return Ok(vec![]),
        };

        // StrategyContext에 충분한 데이터가 있는지 확인
        if !self.has_sufficient_data() {
            return Ok(vec![]);
        }

        // 구간 정보 업데이트
        self.update_zone_info();

        // 현재 구간 계산
        let current_zone = match self.get_current_zone(price) {
            Some(z) => z,
            None => return Ok(vec![]),
        };

        // 이전 구간과 비교
        let prev_zone = self.state.current_zone.unwrap_or(current_zone);
        let zone_change = current_zone as i32 - prev_zone as i32;

        // 구간 상태 업데이트
        self.state.prev_zone = self.state.current_zone;
        self.state.current_zone = Some(current_zone);

        if zone_change == 0 {
            return Ok(vec![]);
        }

        let mut signals = vec![];

        // 구간 상승 → 매수
        if zone_change > 0 {
            // 스크리닝 필터
            if !self.can_buy() {
                debug!("스크리닝 필터 - 매수 스킵");
                return Ok(vec![]);
            }

            // MA 필터
            if !self.check_ma_filter(true) {
                debug!("MA 필터 미충족 - 매수 스킵");
                return Ok(vec![]);
            }

            self.state.trades_count += 1;

            info!(
                ticker = %config.ticker,
                prev_zone = prev_zone,
                current_zone = current_zone,
                zone_change = zone_change,
                "구간 상승 - 매수"
            );

            let signal = Signal::new(
                "range_trading",
                config.ticker.clone(),
                Side::Buy,
                SignalType::Entry,
            )
            .with_strength((zone_change as f64) / (config.div_num as f64))
            .with_metadata("action", json!("zone_up"))
            .with_metadata("prev_zone", json!(prev_zone))
            .with_metadata("current_zone", json!(current_zone))
            .with_metadata("zone_change", json!(zone_change));

            signals.push(signal);
        }
        // 구간 하락 → 매도
        else if zone_change < 0 {
            // MA 필터
            if !self.check_ma_filter(false) {
                debug!("MA 필터 미충족 - 매도 스킵");
                return Ok(vec![]);
            }

            self.state.trades_count += 1;

            info!(
                ticker = %config.ticker,
                prev_zone = prev_zone,
                current_zone = current_zone,
                zone_change = zone_change,
                "구간 하락 - 매도"
            );

            let signal = Signal::new(
                "range_trading",
                config.ticker.clone(),
                Side::Sell,
                SignalType::Exit,
            )
            .with_strength((zone_change.abs() as f64) / (config.div_num as f64))
            .with_metadata("action", json!("zone_down"))
            .with_metadata("prev_zone", json!(prev_zone))
            .with_metadata("current_zone", json!(current_zone))
            .with_metadata("zone_change", json!(zone_change));

            signals.push(signal);
        }

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            ticker = %order.ticker,
            side = ?order.side,
            qty = %order.quantity,
            "주문 체결"
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            ticker = %position.ticker,
            qty = %position.quantity,
            "포지션 업데이트"
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(trades = self.state.trades_count, "전략 종료");
        Ok(())
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        debug!("StrategyContext 주입 완료");
    }

    fn exit_config(&self) -> Option<&ExitConfig> {
        self.config.as_ref().map(|c| &c.exit_config)
    }

    fn get_state(&self) -> Value {
        let klines_count = self.get_klines().len();
        json!({
            "config": self.config,
            "state": self.state,
            "has_context": self.context.is_some(),
            "klines_count": klines_count,
        })
    }
}

// ============================================================================
// 레지스트리 등록
// ============================================================================

use crate::register_strategy;

register_strategy! {
    id: "range_trading",
    aliases: [],
    name: "Range Trading",
    description: "구간분할 매매 전략 - 가격대 구간별 분할 매수/매도",
    timeframe: "1d",
    tickers: ["005930"],
    category: Daily,
    markets: [Stock],
    type: RangeTradingStrategy,
    config: RangeTradingConfig
}

// ============================================================================
// 테스트
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = RangeTradingConfig::default();
        assert_eq!(config.div_num, 15);
        assert_eq!(config.target_period, 20);
        assert_eq!(config.min_global_score, dec!(50));
    }

    #[test]
    fn test_zone_calculation() {
        let mut strategy = RangeTradingStrategy::new();
        strategy.config = Some(RangeTradingConfig::default());

        // 구간 정보 설정 (zone_low=100, zone_high=115, gap=1)
        strategy.state.zone_low = Some(dec!(100));
        strategy.state.zone_high = Some(dec!(115));
        strategy.state.zone_gap = Some(dec!(1));

        // 경계값 테스트 (수정된 로직: <= 사용)
        assert_eq!(strategy.get_current_zone(dec!(100)), Some(1));
        assert_eq!(strategy.get_current_zone(dec!(100.5)), Some(1));
        assert_eq!(strategy.get_current_zone(dec!(101)), Some(1)); // 경계 포함
        assert_eq!(strategy.get_current_zone(dec!(101.01)), Some(2));
        assert_eq!(strategy.get_current_zone(dec!(115)), Some(15));
        assert_eq!(strategy.get_current_zone(dec!(120)), Some(15)); // 초과
    }

    #[tokio::test]
    async fn test_initialization() {
        let mut strategy = RangeTradingStrategy::new();
        let config = json!({
            "ticker": "005930",
            "div_num": 10,
            "target_period": 15
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.config.is_some());
        assert_eq!(strategy.state.current_zone, None);
    }
}
