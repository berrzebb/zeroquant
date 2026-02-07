//! 연금 자동화 전략 (Pension Bot).
//!
//! 개인연금 계좌용 정적+동적 자산배분 조합 전략입니다.
//! 13612W 가중 모멘텀 스코어와 평균 모멘텀을 활용하여
//! 자산 유형별 비중을 동적으로 조절합니다.
//!
//! ## 핵심 개념
//!
//! - **13612W 모멘텀 스코어**: 12×1M + 4×3M + 2×6M + 1×12M
//! - **평균 모멘텀**: 10개월 평균으로 비중 조절
//! - **자산 유형**: 주식(STOCK), 안전자산(SAFE), 원자재(MAT), 현금(CASH)
//! - **남은 현금 분배**: 45% 단기자금, 45% TOP12 모멘텀 보너스, 10% 현금
//!
//! ## 월간 리밸런싱 로직
//!
//! 1. 각 자산의 모멘텀 스코어 계산
//! 2. 평균 모멘텀으로 목표 비중 조절
//! 3. 남은 현금을 단기자금과 상위 모멘텀 종목에 분배
//! 4. 목표 비중에 맞게 리밸런싱

use async_trait::async_trait;
use chrono::{Datelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::domain::{RouteState, StrategyContext};
use trader_strategy_macro::StrategyConfig;

use crate::strategies::common::rebalance::{
    PortfolioPosition, RebalanceCalculator, RebalanceConfig, RebalanceOrderSide, TargetAllocation,
};
use crate::strategies::common::ExitConfig;
use crate::traits::Strategy;
use trader_core::{MarketData, MarketDataType, Order, Position, Side, Signal};

/// 자산 유형
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
#[derive(Default)]
pub enum PensionAssetType {
    /// 주식 (공격적)
    #[default]
    Stock,
    /// 안전자산 (채권 등)
    Safe,
    /// 원자재 (금, 원유 등)
    Mat,
    /// 현금/단기자금
    Cash,
}


/// 포트폴리오 자산 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PensionAsset {
    /// 종목 코드
    pub ticker: String,
    /// 자산 유형
    pub asset_type: PensionAssetType,
    /// 목표 비중 (%)
    pub target_rate: Decimal,
}

impl PensionAsset {
    pub fn new(
        ticker: impl Into<String>,
        asset_type: PensionAssetType,
        target_rate: Decimal,
    ) -> Self {
        Self {
            ticker: ticker.into(),
            asset_type,
            target_rate,
        }
    }

    pub fn stock(ticker: impl Into<String>, target_rate: Decimal) -> Self {
        Self::new(ticker, PensionAssetType::Stock, target_rate)
    }

    pub fn safe(ticker: impl Into<String>, target_rate: Decimal) -> Self {
        Self::new(ticker, PensionAssetType::Safe, target_rate)
    }

    pub fn mat(ticker: impl Into<String>, target_rate: Decimal) -> Self {
        Self::new(ticker, PensionAssetType::Mat, target_rate)
    }

    pub fn cash(ticker: impl Into<String>) -> Self {
        Self::new(ticker, PensionAssetType::Cash, dec!(0))
    }
}

/// 자산별 모멘텀 결과
#[derive(Debug, Clone)]
struct AssetMomentum {
    ticker: String,
    asset_type: PensionAssetType,
    base_target_rate: Decimal,
    momentum_score: Decimal,
    avg_momentum: Decimal,
    adjusted_rate: Decimal,
    current_price: Decimal,
    candles: Vec<Decimal>, // 종가 데이터 저장
}

impl AssetMomentum {
    fn new(ticker: String, asset_type: PensionAssetType, base_rate: Decimal) -> Self {
        Self {
            ticker,
            asset_type,
            base_target_rate: base_rate,
            momentum_score: dec!(0),
            avg_momentum: dec!(0.5),
            adjusted_rate: base_rate,
            current_price: dec!(0),
            candles: Vec::new(),
        }
    }

    /// 13612W 모멘텀 스코어 계산
    fn calculate_momentum_score(&mut self) {
        if self.candles.len() < 240 {
            return;
        }

        let now_price = *self.candles.last().unwrap();
        let len = self.candles.len();

        let one_month_idx = len.saturating_sub(20);
        let three_month_idx = len.saturating_sub(60);
        let six_month_idx = len.saturating_sub(120);
        let twelve_month_idx = len.saturating_sub(240);

        let one_price = self.candles[one_month_idx];
        let three_price = self.candles[three_month_idx];
        let six_price = self.candles[six_month_idx];
        let twelve_price = self.candles[twelve_month_idx];

        // 수익률 계산
        let ret_1m = (now_price - one_price) / one_price;
        let ret_3m = (now_price - three_price) / three_price;
        let ret_6m = (now_price - six_price) / six_price;
        let ret_12m = (now_price - twelve_price) / twelve_price;

        self.momentum_score = ret_1m * dec!(12) + ret_3m * dec!(4) + ret_6m * dec!(2) + ret_12m;
    }

    /// 평균 모멘텀 계산 (10개월 중 현재가 > N개월전 가격인 비율)
    fn calculate_avg_momentum(&mut self, period: usize) {
        let now_price = match self.candles.last() {
            Some(&p) => p,
            None => {
                self.avg_momentum = dec!(0.5);
                return;
            }
        };

        // 데이터가 충분하지 않은 경우
        if self.candles.len() < period * 20 {
            if self.candles.len() < 10 {
                self.avg_momentum = dec!(0.5);
                return;
            }

            let cell_val = self.candles.len() / period;
            if cell_val == 0 {
                self.avg_momentum = dec!(0.5);
                return;
            }

            let mut up_count = 0;
            for i in 1..=period {
                let idx = self.candles.len().saturating_sub(cell_val * i);
                if self.candles[idx] <= now_price {
                    up_count += 1;
                }
            }

            self.avg_momentum = Decimal::from(up_count) / Decimal::from(period);
            return;
        }

        // 정상적인 평균 모멘텀 계산 (20거래일 = 1개월)
        let mut up_count = 0;
        for i in 1..=period {
            let idx = self.candles.len().saturating_sub(20 * i);
            if self.candles[idx] <= now_price {
                up_count += 1;
            }
        }

        self.avg_momentum = Decimal::from(up_count) / Decimal::from(period);
    }

    /// 비중 조절
    fn adjust_rate(&mut self) {
        self.adjusted_rate = self.base_target_rate * self.avg_momentum;
    }
}

/// Pension Bot 설정
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "pension_bot",
    name = "연금 자동화",
    description = "13612W 모멘텀 + 평균 모멘텀 기반 연금 계좌 운용 전략",
    category = "Monthly"
)]
pub struct PensionBotConfig {
    /// 포트폴리오 자산 목록
    #[serde(default = "default_pension_portfolio")]
    #[schema(label = "포트폴리오 자산", skip)]
    pub portfolio: Vec<PensionAsset>,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    #[schema(label = "총 투자 금액", min = 1000000, max = 1000000000, default = 10000000, section = "asset")]
    pub total_amount: Decimal,

    /// 평균 모멘텀 계산 기간 (개월)
    #[serde(default = "default_avg_momentum_period")]
    #[schema(label = "평균 모멘텀 기간 (개월)", min = 3, max = 24, default = 10, section = "indicator")]
    pub avg_momentum_period: usize,

    /// 모멘텀 보너스 상위 종목 수
    #[serde(default = "default_top_bonus_count")]
    #[schema(label = "모멘텀 보너스 상위 종목 수", min = 1, max = 30, default = 12, section = "filter")]
    pub top_bonus_count: usize,

    /// 남은 현금 중 단기자금 비율 (0.0~1.0)
    #[serde(default = "default_cash_to_short_term")]
    #[schema(label = "단기자금 비율", min = 0, max = 1, default = 0.45, section = "sizing")]
    pub cash_to_short_term_rate: Decimal,

    /// 남은 현금 중 모멘텀 보너스 비율 (0.0~1.0)
    #[serde(default = "default_cash_to_bonus")]
    #[schema(label = "모멘텀 보너스 비율", min = 0, max = 1, default = 0.45, section = "sizing")]
    pub cash_to_bonus_rate: Decimal,

    /// 리밸런싱 임계값 (%)
    #[serde(default = "default_rebalance_threshold")]
    #[schema(label = "리밸런싱 임계값 (%)", min = 1, max = 20, default = 3, section = "timing")]
    pub rebalance_threshold: Decimal,

    /// 최소 거래 금액
    #[serde(default = "default_min_trade_amount")]
    #[schema(label = "최소 거래 금액", min = 10000, max = 1000000, default = 50000, section = "sizing")]
    pub min_trade_amount: Decimal,

    /// 최소 GlobalScore (기본값: 60)
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", min = 0, max = 100, default = 60, section = "filter")]
    pub min_global_score: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

fn default_pension_portfolio() -> Vec<PensionAsset> {
    vec![
        // 주식 자산 (STOCK) - 44%
        PensionAsset::stock("448290", dec!(10)), // SOL 미국배당 다우존스
        PensionAsset::stock("379780", dec!(10)), // KODEX 미국S&P500TR
        PensionAsset::stock("294400", dec!(6)),  // TIGER 미국나스닥100
        PensionAsset::stock("200250", dec!(4)),  // KOSEF 인도Nifty50
        PensionAsset::stock("283580", dec!(4)),  // KODEX 일본TOPIX100
        PensionAsset::stock("195970", dec!(6)),  // ARIRANG 신흥국MSCI
        PensionAsset::stock("161510", dec!(2)),  // TIGER 베트남VN30
        PensionAsset::stock("445910", dec!(2)),  // TIGER 필리핀MSCI
        // 안전자산 (SAFE) - 30%
        PensionAsset::safe("305080", dec!(9)), // TIGER 미국달러단기채권액티브
        PensionAsset::safe("148070", dec!(9)), // KOSEF 달러선물
        PensionAsset::safe("385560", dec!(3)), // KODEX 미국30년국채커버드콜
        PensionAsset::safe("304660", dec!(3)), // KODEX 미국채울트라30년선물(H)
        PensionAsset::safe("114470", dec!(3)), // KOSEF 국고채10년
        PensionAsset::safe("329750", dec!(3)), // TIGER 미국채10년선물
        // 원자재 (MAT) - 14%
        PensionAsset::mat("319640", dec!(6)), // TIGER 골드선물(H)
        PensionAsset::mat("276000", dec!(2)), // KBSTAR 원유선물
        PensionAsset::mat("261220", dec!(2)), // KODEX 원유선물
        PensionAsset::mat("139310", dec!(2)), // TIGER 금속선물
        PensionAsset::mat("137610", dec!(2)), // TIGER 농산물선물
        // 현금 (CASH) - 0% (남은 비중으로 자동 조절)
        PensionAsset::cash("130730"), // KOSEF 단기자금
    ]
}

fn default_total_amount() -> Decimal {
    dec!(10000000)
}
fn default_avg_momentum_period() -> usize {
    10
}
fn default_top_bonus_count() -> usize {
    12
}
fn default_cash_to_short_term() -> Decimal {
    dec!(0.45)
}
fn default_cash_to_bonus() -> Decimal {
    dec!(0.45)
}
fn default_rebalance_threshold() -> Decimal {
    dec!(3)
}
fn default_min_trade_amount() -> Decimal {
    dec!(50000)
}
fn default_min_global_score() -> Decimal {
    dec!(60)
}

impl Default for PensionBotConfig {
    fn default() -> Self {
        Self {
            portfolio: default_pension_portfolio(),
            total_amount: default_total_amount(),
            avg_momentum_period: default_avg_momentum_period(),
            top_bonus_count: default_top_bonus_count(),
            cash_to_short_term_rate: default_cash_to_short_term(),
            cash_to_bonus_rate: default_cash_to_bonus(),
            rebalance_threshold: default_rebalance_threshold(),
            min_trade_amount: default_min_trade_amount(),
            min_global_score: default_min_global_score(),
            exit_config: ExitConfig::for_rebalancing(),
        }
    }
}

/// Pension Bot 전략
pub struct PensionBotStrategy {
    config: Option<PensionBotConfig>,
    asset_data: HashMap<String, AssetMomentum>,
    last_rebalance_month: Option<u32>,
    context: Option<Arc<RwLock<StrategyContext>>>,
}

impl PensionBotStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            asset_data: HashMap::new(),
            last_rebalance_month: None,
            context: None,
        }
    }

    pub fn with_config(config: PensionBotConfig) -> Self {
        let mut strategy = Self::new();
        strategy.init_from_config(&config);
        strategy.config = Some(config);
        strategy
    }

    fn init_from_config(&mut self, config: &PensionBotConfig) {
        for asset in &config.portfolio {
            let momentum =
                AssetMomentum::new(asset.ticker.clone(), asset.asset_type, asset.target_rate);
            self.asset_data.insert(asset.ticker.clone(), momentum);
        }
    }

    /// 남은 현금 분배
    fn distribute_remaining_cash(&mut self) {
        let config = match self.config.as_ref() {
            Some(c) => c.clone(),
            None => return,
        };

        let total_rate: Decimal = self.asset_data.values().map(|m| m.adjusted_rate).sum();

        let remaining_rate = dec!(100) - total_rate;

        if remaining_rate <= dec!(0) {
            return;
        }

        info!("Remaining cash rate: {:.2}%", remaining_rate);

        // 1. 45%는 단기자금(CASH)에 배분
        let to_short_term = remaining_rate * config.cash_to_short_term_rate;
        let cash_ticker = self
            .asset_data
            .values()
            .find(|m| m.asset_type == PensionAssetType::Cash)
            .map(|m| m.ticker.clone());

        if let Some(ticker) = cash_ticker {
            if let Some(momentum) = self.asset_data.get_mut(&ticker) {
                momentum.adjusted_rate += to_short_term;
                debug!("Added {:.2}% to CASH asset {}", to_short_term, ticker);
            }
        }

        // 2. 45%는 모멘텀 스코어 상위 N개에 차등 분배
        let to_bonus = remaining_rate * config.cash_to_bonus_rate;

        // 모멘텀 스코어로 정렬
        let mut sorted: Vec<_> = self
            .asset_data
            .values()
            .filter(|m| m.asset_type != PensionAssetType::Cash && m.momentum_score > dec!(0))
            .map(|m| (m.ticker.clone(), m.momentum_score))
            .collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        // 상위 12개에 차등 보너스 (3-3-3-3-2-2-2-2-1-1-1-1 = 24등분)
        let bonus_tickers: Vec<String> = sorted
            .iter()
            .take(config.top_bonus_count)
            .map(|(s, _)| s.clone())
            .collect();

        let rate_cell = to_bonus / dec!(24);
        let bonus_weights = [3, 3, 3, 3, 2, 2, 2, 2, 1, 1, 1, 1];

        for (idx, ticker) in bonus_tickers.iter().enumerate() {
            if idx >= bonus_weights.len() {
                break;
            }
            let weight = Decimal::from(bonus_weights[idx]);
            let bonus = rate_cell * weight;

            if let Some(momentum) = self.asset_data.get_mut(ticker) {
                momentum.adjusted_rate += bonus;
                debug!("Added {:.2}% bonus to {}", bonus, ticker);
            }
        }
    }

    /// 목표 배분 계산
    fn calculate_target_allocations(&self) -> Vec<TargetAllocation> {
        let total_rate: Decimal = self.asset_data.values().map(|m| m.adjusted_rate).sum();

        if total_rate <= dec!(0) {
            return vec![];
        }

        self.asset_data
            .values()
            .filter(|m| m.adjusted_rate > dec!(0))
            .map(|m| {
                let weight = m.adjusted_rate / total_rate;
                TargetAllocation::new(m.ticker.clone(), weight)
            })
            .collect()
    }

    /// 리밸런싱 필요 여부
    fn should_rebalance(&self, current_month: u32) -> bool {
        match self.last_rebalance_month {
            None => true,
            Some(last) => current_month != last,
        }
    }

    /// 특정 티커에 대해 진입 가능 여부 확인.
    /// 미국 ETF 전략이므로 GlobalScore 체크 불필요 (자체 모멘텀 계산 사용).
    fn can_enter(&self, ticker: &str) -> bool {
        let Some(_config) = self.config.as_ref() else {
            return false;
        };

        let Some(ctx) = self.context.as_ref() else {
            // Context가 없으면 진입 허용 (기존 동작 유지)
            debug!("[PensionBot] StrategyContext not available - entry allowed");
            return true;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            debug!("[PensionBot] Failed to acquire context lock - entry blocked");
            return false;
        };

        // RouteState 체크 - Overheat 시만 진입 제한
        if let Some(route_state) = ctx_lock.get_route_state(ticker) {
            match route_state {
                RouteState::Overheat => {
                    debug!(
                        "[PensionBot] 시장 과열 - {} 진입 제한",
                        ticker
                    );
                    return false;
                }
                // 나머지 상태는 진입 허용
                _ => {}
            }
        }

        true
    }

    /// 리밸런싱 시그널 생성
    fn generate_rebalance_signals(&mut self) -> Vec<Signal> {
        let config = match self.config.as_ref() {
            Some(c) => c.clone(),
            None => return Vec::new(),
        };

        // 모든 자산 모멘텀 계산
        for data in self.asset_data.values_mut() {
            data.calculate_momentum_score();
            data.calculate_avg_momentum(config.avg_momentum_period);
            data.adjust_rate();
        }

        // 남은 현금 분배
        self.distribute_remaining_cash();

        // 목표 배분 계산
        let target_allocations = self.calculate_target_allocations();

        // 현재 포지션 구성 (빈 포지션 - 실제 구현에서는 외부에서 전달)
        let current_positions: Vec<PortfolioPosition> = Vec::new();

        // 리밸런싱 계산
        let rebalance_config = RebalanceConfig::korean_market();
        let calculator = RebalanceCalculator::new(rebalance_config);
        let result = calculator.calculate_orders(&current_positions, &target_allocations);

        // 시그널 생성
        let mut signals = Vec::new();

        for order in result.orders {
            // BUY 시그널의 경우 can_enter() 체크
            if order.side == RebalanceOrderSide::Buy
                && !self.can_enter(&order.ticker) {
                    debug!(
                        "[PensionBot] Skipping BUY signal for {} - entry not allowed",
                        order.ticker
                    );
                    continue;
                }

            let ticker = format!("{}/KRW", order.ticker);

            let side = match order.side {
                RebalanceOrderSide::Buy => Side::Buy,
                RebalanceOrderSide::Sell => Side::Sell,
            };

            let price = if order.quantity > Decimal::ZERO {
                order.amount / order.quantity
            } else {
                Decimal::ZERO
            };

            let signal = if order.side == RebalanceOrderSide::Buy {
                Signal::entry("pension_bot", ticker, side)
                    .with_strength(0.5)
                    .with_prices(Some(price), None, None)
                    .with_metadata("reason", json!("rebalance"))
            } else {
                Signal::exit("pension_bot", ticker, side)
                    .with_strength(0.5)
                    .with_prices(Some(price), None, None)
                    .with_metadata("reason", json!("rebalance"))
            };

            signals.push(signal);
        }

        let current_month = Utc::now().month();
        self.last_rebalance_month = Some(current_month);

        info!(signals = signals.len(), "Pension Bot: 리밸런싱 시그널 생성");

        signals
    }
}

impl Default for PensionBotStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for PensionBotStrategy {
    fn name(&self) -> &str {
        "pension_bot"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "개인연금 자동화 전략 - 정적+동적 자산배분 조합"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let cfg: PensionBotConfig = serde_json::from_value(config)?;
        self.init_from_config(&cfg);
        self.config = Some(cfg);
        info!("Pension Bot 전략 초기화 완료");
        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        let ticker_str = data.ticker.clone();

        // Kline 데이터에서 종가 추출
        let (close, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (kline.close, kline.open_time),
            _ => return Ok(vec![]),
        };

        // 자산 데이터 업데이트
        if let Some(momentum) = self.asset_data.get_mut(&ticker_str) {
            momentum.candles.push(close);
            momentum.current_price = close;

            // 최대 300개 유지 (메모리 누수 방지)
            if momentum.candles.len() > 300 {
                momentum.candles.drain(0..momentum.candles.len() - 300);
            }
        }

        // 월간 리밸런싱 체크
        let current_month = timestamp.month();
        if !self.should_rebalance(current_month) {
            return Ok(vec![]);
        }

        // 모든 자산의 데이터가 충분한지 확인 (최소 20개 캔들)
        let all_ready = self.asset_data.values().all(|m| m.candles.len() >= 20);
        if !all_ready {
            return Ok(vec![]);
        }

        // 리밸런싱 시그널 생성
        let signals = self.generate_rebalance_signals();
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
        _position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Pension Bot 전략 종료");
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "name": self.name(),
            "last_rebalance_month": self.last_rebalance_month,
            "asset_count": self.asset_data.len(),
            "assets": self.asset_data.values()
                .map(|m| json!({
                    "ticker": m.ticker,
                    "asset_type": format!("{:?}", m.asset_type),
                    "momentum_score": m.momentum_score.to_string(),
                    "avg_momentum": m.avg_momentum.to_string(),
                    "adjusted_rate": m.adjusted_rate.to_string(),
                }))
                .collect::<Vec<_>>(),
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into Pension Bot strategy");
    }

    fn exit_config(&self) -> Option<&ExitConfig> {
        self.config.as_ref().map(|c| &c.exit_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pension_bot_initialization() {
        let config = PensionBotConfig::default();
        let strategy = PensionBotStrategy::with_config(config.clone());

        assert_eq!(strategy.name(), "pension_bot");
        assert!(!strategy.asset_data.is_empty());
        assert_eq!(config.avg_momentum_period, 10);
        assert_eq!(config.top_bonus_count, 12);
    }

    #[test]
    fn test_momentum_score_calculation() {
        let mut momentum =
            AssetMomentum::new("TEST".to_string(), PensionAssetType::Stock, dec!(10));

        // 상승 추세 데이터 생성 (250일)
        for i in 0..250 {
            momentum.candles.push(Decimal::from(100 + i));
        }

        momentum.calculate_momentum_score();
        assert!(
            momentum.momentum_score > dec!(0),
            "상승 추세에서 모멘텀 스코어는 양수"
        );
    }

    #[test]
    fn test_avg_momentum_calculation() {
        let mut momentum =
            AssetMomentum::new("TEST".to_string(), PensionAssetType::Stock, dec!(10));

        // 꾸준히 상승하는 데이터 (200일)
        for i in 0..200 {
            momentum.candles.push(Decimal::from(100 + i));
        }

        momentum.calculate_avg_momentum(10);
        assert!(
            momentum.avg_momentum > dec!(0.5),
            "상승 추세에서 평균 모멘텀은 0.5 이상"
        );
    }

    #[test]
    fn test_should_rebalance() {
        let strategy = PensionBotStrategy::new();

        // 첫 실행에서는 리밸런싱 필요
        assert!(strategy.should_rebalance(1));
    }

    #[test]
    fn test_asset_types() {
        assert_eq!(PensionAssetType::default(), PensionAssetType::Stock);

        let stock = PensionAsset::stock("TEST", dec!(10));
        assert_eq!(stock.asset_type, PensionAssetType::Stock);

        let safe = PensionAsset::safe("TEST", dec!(10));
        assert_eq!(safe.asset_type, PensionAssetType::Safe);

        let mat = PensionAsset::mat("TEST", dec!(10));
        assert_eq!(mat.asset_type, PensionAssetType::Mat);

        let cash = PensionAsset::cash("TEST");
        assert_eq!(cash.asset_type, PensionAssetType::Cash);
        assert_eq!(cash.target_rate, dec!(0));
    }

    #[test]
    fn test_default_portfolio() {
        let portfolio = default_pension_portfolio();
        assert!(!portfolio.is_empty());

        // 자산 유형별 개수 확인
        let stock_count = portfolio
            .iter()
            .filter(|a| a.asset_type == PensionAssetType::Stock)
            .count();
        let safe_count = portfolio
            .iter()
            .filter(|a| a.asset_type == PensionAssetType::Safe)
            .count();
        let mat_count = portfolio
            .iter()
            .filter(|a| a.asset_type == PensionAssetType::Mat)
            .count();
        let cash_count = portfolio
            .iter()
            .filter(|a| a.asset_type == PensionAssetType::Cash)
            .count();

        assert!(stock_count > 0, "주식 자산이 있어야 함");
        assert!(safe_count > 0, "안전 자산이 있어야 함");
        assert!(mat_count > 0, "원자재 자산이 있어야 함");
        assert!(cash_count > 0, "현금 자산이 있어야 함");
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "pension_bot",
    aliases: ["pension"],
    name: "연금 자동화",
    description: "장기 연금 계좌 운용 전략입니다.",
    timeframe: "1d",
    tickers: ["SPY", "IWM", "VEA", "VWO", "TLT", "IEF", "TIP", "BIL"],
    category: Monthly,
    markets: [Stock],
    type: PensionBotStrategy,
    config: PensionBotConfig
}
