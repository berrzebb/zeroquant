//! 소형주 퀀트 전략 (Small Cap Quant).
//!
//! 코스닥 소형지수의 20일 이동평균선을 기준으로
//! 소형주 팩터(시가총액, 영업이익, ROE, PBR, PER)를 활용한 퀀트 전략.
//!
//! # 전략 개요
//!
//! ## 필터 조건
//! - 시가총액 50억 이상 (너무 작은 종목 제외)
//! - 금융 섹터 제외
//! - 영업이익 > 0 (흑자 기업)
//! - ROE >= 5% (자본 수익성)
//! - EPS > 0, BPS > 0
//! - PBR >= 0.2, PER >= 2 (극단적 저평가 제외)
//!
//! ## 정렬 및 선택
//! - 시가총액 오름차순 정렬 (소형주 우선)
//! - 상위 N개 종목 선택 (기본 20개)
//!
//! ## 매매 로직
//! - 코스닥 소형지수가 20일 MA 위 → 매수 유지
//! - 코스닥 소형지수가 20일 MA 아래 → 전량 매도
//! - 월간 리밸런싱

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::traits::Strategy;
use trader_core::domain::{RouteState, StrategyContext};
use trader_core::{Kline, MarketData, MarketDataType, Order, Position, Side, Signal, Timeframe};
use trader_strategy_macro::StrategyConfig;

use crate::strategies::common::ExitConfig;

/// 소형주 퀀트 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "small_cap_quant",
    name = "소형주 퀀트",
    description = "코스닥 소형지수 MA 기반 소형주 팩터 퀀트 전략",
    category = "Daily"
)]
pub struct SmallCapQuantConfig {
    /// 선택할 종목 수
    #[serde(default = "default_target_count")]
    #[schema(label = "선택 종목 수", min = 5, max = 50, default = 20, section = "sizing")]
    pub target_count: usize,

    /// 이동평균 기간
    #[serde(default = "default_ma_period")]
    #[schema(label = "이동평균 기간", min = 5, max = 200, default = 20, section = "indicator")]
    pub ma_period: usize,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    #[schema(label = "총 투자 금액", min = 1000000, max = 1000000000, default = 10000000, section = "asset")]
    pub total_amount: Decimal,

    /// 최소 시가총액 (억원)
    #[serde(default = "default_min_market_cap")]
    #[schema(label = "최소 시가총액 (억원)", min = 10, max = 1000, default = 50, section = "filter")]
    pub min_market_cap: f64,

    /// 최소 ROE (%)
    #[serde(default = "default_min_roe")]
    #[schema(label = "최소 ROE (%)", min = 0, max = 50, default = 5, section = "filter")]
    pub min_roe: f64,

    /// 최소 PBR
    #[serde(default = "default_min_pbr")]
    #[schema(label = "최소 PBR", min = 0.1, max = 5, default = 0.2, section = "filter")]
    pub min_pbr: f64,

    /// 최소 PER
    #[serde(default = "default_min_per")]
    #[schema(label = "최소 PER", min = 1, max = 50, default = 2, section = "filter")]
    pub min_per: f64,

    /// 기준 지수 티커 (기본: 코스닥150 ETF)
    #[serde(default = "default_index_ticker")]
    #[schema(label = "기준 지수 티커", field_type = "symbol", default = "229200", section = "asset")]
    pub index_ticker: String,

    /// 최소 글로벌 스코어 (기본값: 60)
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", min = 0, max = 100, default = 60, section = "filter")]
    pub min_global_score: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

fn default_target_count() -> usize {
    20
}
fn default_ma_period() -> usize {
    20
}
fn default_total_amount() -> Decimal {
    dec!(10000000)
}
fn default_min_market_cap() -> f64 {
    50.0
} // 50억
fn default_min_roe() -> f64 {
    5.0
}
fn default_min_pbr() -> f64 {
    0.2
}
fn default_min_per() -> f64 {
    2.0
}
fn default_index_ticker() -> String {
    "229200".to_string()
} // 코스닥150 ETF
fn default_min_global_score() -> Decimal {
    dec!(60)
}

impl Default for SmallCapQuantConfig {
    fn default() -> Self {
        Self {
            target_count: default_target_count(),
            ma_period: default_ma_period(),
            total_amount: default_total_amount(),
            min_market_cap: default_min_market_cap(),
            min_roe: default_min_roe(),
            min_pbr: default_min_pbr(),
            min_per: default_min_per(),
            index_ticker: default_index_ticker(),
            min_global_score: default_min_global_score(),
            exit_config: ExitConfig::for_momentum(),
        }
    }
}

/// 종목 재무 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockFundamentals {
    pub ticker: String,
    pub market_cap: f64,       // 시가총액 (억원)
    pub sector: String,        // 섹터
    pub operating_profit: f64, // 영업이익
    pub roe: f64,              // ROE (%)
    pub eps: f64,              // EPS
    pub bps: f64,              // BPS
    pub pbr: f64,              // PBR
    pub per: f64,              // PER
}

impl StockFundamentals {
    /// 필터 조건 통과 여부.
    pub fn passes_filter(&self, config: &SmallCapQuantConfig) -> bool {
        // 시가총액 필터
        if self.market_cap < config.min_market_cap {
            return false;
        }

        // 금융 섹터 제외
        let sector_lower = self.sector.to_lowercase();
        if sector_lower.contains("금융")
            || sector_lower.contains("은행")
            || sector_lower.contains("보험")
            || sector_lower.contains("증권")
        {
            return false;
        }

        // 영업이익 > 0
        if self.operating_profit <= 0.0 {
            return false;
        }

        // ROE >= 최소값
        if self.roe < config.min_roe {
            return false;
        }

        // EPS, BPS > 0
        if self.eps <= 0.0 || self.bps <= 0.0 {
            return false;
        }

        // PBR, PER 최소값
        if self.pbr < config.min_pbr || self.per < config.min_per {
            return false;
        }

        true
    }
}

/// 종목별 데이터.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct StockData {
    ticker: String,
    current_price: Decimal,
    current_holdings: Decimal,
}

/// 지수 데이터 (현재 가격만 저장, 히스토리는 StrategyContext에서 가져옴).
#[derive(Debug, Clone, Default)]
struct IndexData {
    current_price: Decimal,
}

impl IndexData {
    fn new() -> Self {
        Self {
            current_price: Decimal::ZERO,
        }
    }

    fn update_price(&mut self, price: Decimal) {
        self.current_price = price;
    }
}

/// 시장 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarketState {
    /// 지수가 MA 위 (매수 가능)
    AboveMA,
    /// 지수가 MA 아래 (매도 필요)
    BelowMA,
    /// 알 수 없음 (데이터 부족)
    Unknown,
}

/// 소형주 퀀트 전략.
pub struct SmallCapQuantStrategy {
    config: Option<SmallCapQuantConfig>,
    tickers: Vec<String>,
    stock_data: HashMap<String, StockData>,
    index_data: IndexData,

    /// 현재 보유 종목
    holdings: Vec<String>,

    /// 이전 시장 상태
    prev_market_state: MarketState,

    /// 현재 시장 상태
    current_market_state: MarketState,

    /// 마지막 리밸런싱 월
    last_rebalance_month: Option<u32>,

    /// 통계
    trades_count: u32,
    total_pnl: Decimal,

    initialized: bool,

    /// 전략 컨텍스트 (RouteState, GlobalScore 등)
    context: Option<Arc<RwLock<StrategyContext>>>,
}

impl SmallCapQuantStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            tickers: Vec::new(),
            stock_data: HashMap::new(),
            index_data: IndexData::new(),
            holdings: Vec::new(),
            prev_market_state: MarketState::Unknown,
            current_market_state: MarketState::Unknown,
            last_rebalance_month: None,
            trades_count: 0,
            total_pnl: Decimal::ZERO,
            initialized: false,
            context: None,
        }
    }

    // ========================================================================
    // StrategyContext 연동 헬퍼
    // ========================================================================

    /// StrategyContext에서 지수 klines 가져오기
    fn get_index_klines(&self) -> Vec<Kline> {
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
        ctx_lock.get_klines(&config.index_ticker, Timeframe::D1).to_vec()
    }

    /// 지수가 MA 위에 있는지 확인 (StrategyContext 기반)
    fn is_index_above_ma(&self, period: usize) -> Option<bool> {
        let klines = self.get_index_klines();
        if klines.len() < period {
            return None;
        }

        let sum: Decimal = klines.iter().rev().take(period).map(|k| k.close).sum();
        let ma = sum / Decimal::from(period);
        let current = klines.last()?.close;

        Some(current > ma)
    }

    /// 시장 상태 업데이트.
    fn update_market_state(&mut self) {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return,
        };

        self.prev_market_state = self.current_market_state;

        match self.is_index_above_ma(config.ma_period) {
            Some(true) => self.current_market_state = MarketState::AboveMA,
            Some(false) => self.current_market_state = MarketState::BelowMA,
            None => self.current_market_state = MarketState::Unknown,
        }
    }

    /// 리밸런싱 필요 여부.
    fn needs_rebalance(&self, timestamp: DateTime<Utc>) -> bool {
        // MA 상태 변화 감지
        let state_changed = match (self.prev_market_state, self.current_market_state) {
            (MarketState::AboveMA, MarketState::BelowMA) => true, // 하락 전환
            (MarketState::BelowMA, MarketState::AboveMA) => true, // 상승 전환
            (MarketState::Unknown, MarketState::AboveMA) => true, // 첫 진입
            _ => false,
        };

        if state_changed {
            return true;
        }

        // 월간 리밸런싱 체크 (MA 위에 있을 때만)
        if self.current_market_state == MarketState::AboveMA {
            let current_month = timestamp.month();
            if let Some(last) = self.last_rebalance_month {
                if current_month != last {
                    return true;
                }
            }
        }

        false
    }

    /// RouteState와 GlobalScore 기반 진입 조건 체크
    #[allow(dead_code)]
    fn can_enter(&self, ticker: &str) -> bool {
        let context = match &self.context {
            Some(ctx) => ctx,
            None => return true,
        };

        let config = match &self.config {
            Some(cfg) => cfg,
            None => return true,
        };

        let ctx = match context.try_read() {
            Ok(ctx) => ctx,
            Err(_) => return true,
        };

        // RouteState 체크
        if let Some(route) = ctx.get_route_state(ticker) {
            match route {
                RouteState::Wait | RouteState::Overheat => {
                    debug!("[SmallCapQuant] RouteState가 {:?}이므로 진입 불가", route);
                    return false;
                }
                _ => {}
            }
        }

        // GlobalScore 체크
        if let Some(score) = ctx.get_global_score(ticker) {
            if score.overall_score < config.min_global_score {
                debug!(
                    "[SmallCapQuant] {} GlobalScore {} < {} 기준 미달",
                    ticker, score.overall_score, config.min_global_score
                );
                return false;
            }
        }

        true
    }

    /// 매도 시그널 생성 (모든 보유 종목).
    fn generate_sell_all_signals(&mut self) -> Vec<Signal> {
        let mut signals = Vec::new();

        for ticker_str in &self.holdings {
            if let Some(data) = self.stock_data.get(ticker_str) {
                if data.current_holdings > Decimal::ZERO {
                    let ticker = format!("{}/KRW", ticker_str);
                    let signal = Signal::exit("small_cap_quant", ticker, Side::Sell)
                        .with_strength(1.0)
                        .with_prices(Some(data.current_price), None, None)
                        .with_metadata("reason", json!("below_ma"));

                    signals.push(signal);
                }
            }
        }

        info!(count = signals.len(), "20일선 하향 돌파 - 전체 매도");

        signals
    }

    /// 매수 시그널 생성 (MA 상향 돌파 시).
    fn generate_buy_signals(&mut self) -> Vec<Signal> {
        let config = match &self.config {
            Some(c) => c.clone(),
            None => return Vec::new(),
        };

        let mut signals = Vec::new();

        // stock_data에서 지수가 아닌 종목들 중 가격이 있는 것들 선택
        let candidates: Vec<_> = self.stock_data
            .iter()
            .filter(|(ticker, data)| {
                *ticker != &config.index_ticker && data.current_price > Decimal::ZERO
            })
            .take(config.target_count)
            .collect();

        if candidates.is_empty() {
            return Vec::new();
        }

        // 균등 비중으로 매수 시그널 생성
        let weight = 1.0 / candidates.len() as f64;

        for (ticker_str, data) in candidates {
            let ticker = format!("{}/KRW", ticker_str);
            let signal = Signal::entry("small_cap_quant", ticker, Side::Buy)
                .with_strength(weight)
                .with_prices(Some(data.current_price), None, None)
                .with_metadata("reason", json!("above_ma"));

            signals.push(signal);
        }

        info!(count = signals.len(), "20일선 상향 돌파 - 균등 비중 매수");

        signals
    }


}

impl Default for SmallCapQuantStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for SmallCapQuantStrategy {
    fn name(&self) -> &str {
        "Small Cap Quant"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "소형주 퀀트 전략. 코스닥 소형지수의 20일 이동평균선 위에서 \
         재무 필터(시총, ROE, PBR, PER)를 통과한 소형주 상위 N개에 투자합니다."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let scq_config: SmallCapQuantConfig = serde_json::from_value(config)?;

        info!(
            target_count = scq_config.target_count,
            ma_period = scq_config.ma_period,
            index = %scq_config.index_ticker,
            "소형주 퀀트 전략 초기화"
        );

        // 기준 지수 티커 추가
        let index_ticker = format!("{}/KRW", scq_config.index_ticker);
        self.tickers.push(index_ticker);

        self.config = Some(scq_config);
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

        let config = match self.config.as_ref() {
            Some(c) => c.clone(),
            None => return Ok(vec![]),
        };

        let ticker_str = data.ticker.clone();

        // Kline 데이터에서 종가 추출
        let (close, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (kline.close, kline.open_time),
            _ => return Ok(vec![]),
        };

        // 기준 지수 데이터 업데이트 (StrategyContext에서 klines 조회하므로 현재가만 저장)
        if ticker_str == config.index_ticker {
            self.index_data.update_price(close);
            self.update_market_state();

            debug!(
                index = %ticker_str,
                price = %close,
                state = ?self.current_market_state,
                "지수 데이터 업데이트"
            );

            // 리밸런싱 필요 여부 체크
            if self.needs_rebalance(timestamp) {
                let signals = match self.current_market_state {
                    MarketState::BelowMA => {
                        // MA 아래로 하락 → 전량 매도
                        self.generate_sell_all_signals()
                    }
                    MarketState::AboveMA => {
                        // MA 위 → 매수 (stock_data에 있는 종목 중 선택)
                        self.generate_buy_signals()
                    }
                    _ => Vec::new(),
                };

                if !signals.is_empty() {
                    self.last_rebalance_month = Some(timestamp.month());
                    return Ok(signals);
                }
            }
        } else {
            // 개별 종목 데이터 업데이트
            if let Some(stock) = self.stock_data.get_mut(&ticker_str) {
                stock.current_price = close;
            } else {
                self.stock_data.insert(
                    ticker_str.clone(),
                    StockData {
                        ticker: ticker_str,
                        current_price: close,
                        current_holdings: Decimal::ZERO,
                    },
                );
            }
        }

        Ok(vec![])
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ticker_str = order.ticker.clone();

        if let Some(stock) = self.stock_data.get_mut(&ticker_str) {
            match order.side {
                Side::Buy => {
                    stock.current_holdings += order.quantity;
                    if !self.holdings.contains(&ticker_str) {
                        self.holdings.push(ticker_str.clone());
                    }
                }
                Side::Sell => {
                    stock.current_holdings -= order.quantity;
                    if stock.current_holdings <= Decimal::ZERO {
                        stock.current_holdings = Decimal::ZERO;
                        self.holdings.retain(|s| s != &ticker_str);
                    }
                }
            }
            self.trades_count += 1;
        }

        debug!(
            ticker = %order.ticker,
            side = ?order.side,
            quantity = %order.quantity,
            "주문 체결"
        );

        Ok(())
    }

    async fn on_position_update(
        &mut self,
        _position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            trades = self.trades_count,
            holdings = self.holdings.len(),
            total_pnl = %self.total_pnl,
            "소형주 퀀트 전략 종료"
        );
        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "initialized": self.initialized,
            "market_state": format!("{:?}", self.current_market_state),
            "holdings_count": self.holdings.len(),
            "trades_count": self.trades_count,
            "last_rebalance_month": self.last_rebalance_month,
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into SmallCapQuant strategy");
    }

    fn exit_config(&self) -> Option<&ExitConfig> {
        self.config.as_ref().map(|c| &c.exit_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_small_cap_quant_initialization() {
        let mut strategy = SmallCapQuantStrategy::new();

        let config = json!({
            "target_count": 20,
            "ma_period": 20
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
    }

    #[test]
    fn test_stock_filter() {
        let config = SmallCapQuantConfig::default();

        // 통과하는 종목
        let good_stock = StockFundamentals {
            ticker: "123456".to_string(),
            market_cap: 100.0,
            sector: "IT".to_string(),
            operating_profit: 100.0,
            roe: 10.0,
            eps: 1000.0,
            bps: 5000.0,
            pbr: 0.5,
            per: 10.0,
        };
        assert!(good_stock.passes_filter(&config));

        // 금융 섹터 제외
        let finance_stock = StockFundamentals {
            ticker: "234567".to_string(),
            market_cap: 100.0,
            sector: "금융업".to_string(),
            operating_profit: 100.0,
            roe: 10.0,
            eps: 1000.0,
            bps: 5000.0,
            pbr: 0.5,
            per: 10.0,
        };
        assert!(!finance_stock.passes_filter(&config));

        // 시총 미달
        let small_stock = StockFundamentals {
            ticker: "345678".to_string(),
            market_cap: 30.0, // 50억 미만
            sector: "IT".to_string(),
            operating_profit: 100.0,
            roe: 10.0,
            eps: 1000.0,
            bps: 5000.0,
            pbr: 0.5,
            per: 10.0,
        };
        assert!(!small_stock.passes_filter(&config));
    }

}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "small_cap_quant",
    aliases: [],
    name: "소형주 퀀트",
    description: "소형주 대상 퀀트 전략입니다.",
    timeframe: "1d",
    tickers: [],
    category: Daily,
    markets: [Stock],
    type: SmallCapQuantStrategy,
    config: SmallCapQuantConfig
}
