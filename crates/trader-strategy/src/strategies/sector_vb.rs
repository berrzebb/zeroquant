//! 섹터 변동성 돌파 전략 (Sector Volatility Breakout) v2.0
//!
//! 한국 섹터 ETF를 대상으로 하는 변동성 돌파 전략.
//! Larry Williams 변동성 돌파 전략을 섹터 ETF에 적용.
//!
//! # 핵심 로직
//! 1. 전일 가장 강한 섹터 선택 (수익률/모멘텀 기준)
//! 2. 당일 시가 + (전일 범위 × K) 돌파 시 매수
//! 3. 장 마감 전 청산 (일중 전략)
//!
//! # StrategyContext 연동 (v2.0)
//! - GlobalScore: 최소 점수 이상인 섹터만 대상
//! - RouteState: Attack/Armed 상태에서만 진입
//! - MarketRegime: 하락장에서는 진입 제한
//! - MarketBreadth: 섹터 로테이션 데이터 활용
//!
//! # 대상 섹터 ETF (한국)
//! - 091160: KODEX 반도체
//! - 091230: TIGER 반도체
//! - 305720: KODEX 2차전지산업
//! - 305540: TIGER 2차전지테마
//! - 091170: KODEX 은행
//! - 091220: TIGER 은행
//! - 102970: KODEX 철강
//! - 117460: KODEX 건설
//! - 091180: TIGER 자동차
//! - 102960: KODEX 기계장비

use crate::register_strategy;
use crate::strategies::common::deserialize_tickers;
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::domain::StrategyContext;
use trader_core::{
    MarketData, MarketDataType, MarketRegime, Order, Position, RouteState, Side, Signal,
};
use trader_strategy_macro::StrategyConfig;

use crate::strategies::common::{adjust_strength_by_score, ExitConfig};

// ============================================================================
// 상수 정의
// ============================================================================

/// 한국 시장 마감 시간 (KST)
const KST_MARKET_CLOSE_HOUR: u32 = 15;
const KST_MARKET_CLOSE_MINUTE: u32 = 30;

/// KST와 UTC 시차 (시간)
const KST_UTC_OFFSET_HOURS: i64 = 9;

// ============================================================================
// 설정
// ============================================================================

/// 섹터 변동성 돌파 전략 설정
#[derive(Debug, Clone, Deserialize, Serialize, StrategyConfig)]
#[strategy(
    id = "sector_vb",
    name = "섹터 변동성 돌파",
    description = "한국 섹터 ETF 대상 Larry Williams 변동성 돌파 전략",
    category = "Intraday"
)]
pub struct SectorVbConfig {
    /// 거래 대상 섹터 ETF 리스트
    #[serde(
        default = "default_sector_list",
        deserialize_with = "deserialize_tickers"
    )]
    #[schema(label = "대상 섹터 ETF", field_type = "symbols", section = "asset")]
    pub tickers: Vec<String>,

    /// 돌파 K 계수 (기본값: 0.5)
    #[serde(default = "default_k_factor")]
    #[schema(label = "K 계수", min = 0.1, max = 1.0, default = 0.5, section = "indicator")]
    pub k_factor: f64,

    /// 섹터 선정 기준 (기본값: "returns" - 전일 수익률)
    #[serde(default = "default_selection_method")]
    #[schema(label = "섹터 선정 기준", field_type = "select", options = ["returns", "momentum", "volume"], default = "returns", section = "indicator")]
    pub selection_method: String,

    /// 선택할 상위 섹터 수 (기본값: 1)
    #[serde(default = "default_top_n")]
    #[schema(label = "상위 섹터 수", min = 1, max = 10, default = 1, section = "sizing")]
    pub top_n: usize,

    /// 최소 전일 거래량 (기본값: 100000)
    #[serde(default = "default_min_volume")]
    #[schema(label = "최소 거래량", min = 10000, max = 10000000, default = 100000, section = "filter")]
    pub min_volume: u64,

    /// 장 마감 전 청산 시간 (분, 기본값: 10분 전)
    #[serde(default = "default_close_before_minutes")]
    #[schema(label = "마감 전 청산 (분)", min = 1, max = 60, default = 10, section = "timing")]
    pub close_before_minutes: u32,

    /// 손절 비율 (기본값: 2%)
    #[serde(default = "default_stop_loss_pct")]
    #[schema(label = "손절 비율 (%)", min = 0.5, max = 10, default = 2.0, section = "sizing")]
    pub stop_loss_pct: f64,

    /// 익절 비율 (기본값: 3%)
    #[serde(default = "default_take_profit_pct")]
    #[schema(label = "익절 비율 (%)", min = 0.5, max = 20, default = 3.0, section = "sizing")]
    pub take_profit_pct: f64,

    // ========== StrategyContext 연동 설정 (v2.0) ==========
    /// 최소 GlobalScore (기본값: 50.0)
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", min = 0, max = 100, default = 50, section = "filter")]
    pub min_global_score: f64,

    /// RouteState 필터 사용 여부 (기본값: true)
    #[serde(default = "default_use_route_filter")]
    #[schema(label = "RouteState 필터 사용", default = true, section = "filter")]
    pub use_route_filter: bool,

    /// MarketRegime 필터 사용 여부 (기본값: true)
    #[serde(default = "default_use_regime_filter")]
    #[schema(label = "MarketRegime 필터 사용", default = true, section = "filter")]
    pub use_regime_filter: bool,

    /// 하락장에서 진입 허용 여부 (기본값: false)
    #[serde(default)]
    #[schema(label = "하락장 진입 허용", default = false, section = "filter")]
    pub allow_downtrend_entry: bool,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

fn default_sector_list() -> Vec<String> {
    vec![
        "091160".to_string(), // KODEX 반도체
        "091230".to_string(), // TIGER 반도체
        "305720".to_string(), // KODEX 2차전지산업
        "305540".to_string(), // TIGER 2차전지테마
        "091170".to_string(), // KODEX 은행
        "091220".to_string(), // TIGER 은행
        "102970".to_string(), // KODEX 철강
        "117460".to_string(), // KODEX 건설
        "091180".to_string(), // TIGER 자동차
        "102960".to_string(), // KODEX 기계장비
    ]
}

fn default_k_factor() -> f64 {
    0.5
}
fn default_selection_method() -> String {
    "returns".to_string()
}
fn default_top_n() -> usize {
    1
}
fn default_min_volume() -> u64 {
    100_000
}
fn default_close_before_minutes() -> u32 {
    10
}
fn default_stop_loss_pct() -> f64 {
    2.0
}
fn default_take_profit_pct() -> f64 {
    3.0
}
fn default_min_global_score() -> f64 {
    50.0
}
fn default_use_route_filter() -> bool {
    true
}
fn default_use_regime_filter() -> bool {
    true
}

impl Default for SectorVbConfig {
    fn default() -> Self {
        Self {
            tickers: default_sector_list(),
            k_factor: 0.5,
            selection_method: "returns".to_string(),
            top_n: 1,
            min_volume: 100_000,
            close_before_minutes: 10,
            stop_loss_pct: 2.0,
            take_profit_pct: 3.0,
            min_global_score: 50.0,
            use_route_filter: true,
            use_regime_filter: true,
            allow_downtrend_entry: false,
            exit_config: ExitConfig::for_day_trading(),
        }
    }
}

// ============================================================================
// 내부 데이터 구조체
// ============================================================================

/// 섹터 데이터
#[derive(Debug, Clone)]
struct SectorData {
    ticker: String,
    prev_close: Decimal,
    prev_high: Decimal,
    prev_low: Decimal,
    prev_volume: Decimal,
    today_open: Option<Decimal>,
    today_high: Decimal,
    today_low: Decimal,
    current_price: Decimal,
    target_price: Option<Decimal>,
    returns: Decimal, // 전일 수익률 (%)
}

impl SectorData {
    fn new(ticker: String) -> Self {
        Self {
            ticker,
            prev_close: Decimal::ZERO,
            prev_high: Decimal::ZERO,
            prev_low: Decimal::ZERO,
            prev_volume: Decimal::ZERO,
            today_open: None,
            today_high: Decimal::ZERO,
            today_low: Decimal::MAX,
            current_price: Decimal::ZERO,
            target_price: None,
            returns: Decimal::ZERO,
        }
    }
}

/// 포지션 상태
#[derive(Debug, Clone)]
struct PositionState {
    ticker: String,
    entry_price: Decimal,
    stop_loss: Decimal,
    take_profit: Decimal,
    #[allow(dead_code)]
    entry_time: DateTime<Utc>,
}

/// 전략 상태
#[derive(Debug, Clone, PartialEq)]
enum StrategyState {
    Rest,      // 대기 (조건 불만족)
    Ready,     // 돌파 체크 준비
    Investing, // 투자 중
}

// ============================================================================
// 전략 구현
// ============================================================================

/// 섹터 변동성 돌파 전략 v2.0
pub struct SectorVbStrategy {
    config: Option<SectorVbConfig>,
    tickers: Vec<String>,
    context: Option<Arc<RwLock<StrategyContext>>>,

    /// 섹터별 데이터
    sector_data: HashMap<String, SectorData>,

    /// 선택된 섹터 (오늘 투자 대상)
    selected_sector: Option<String>,

    /// 전략 상태
    state: StrategyState,

    /// 현재 포지션
    position: Option<PositionState>,

    /// 오늘 날짜 (거래일 관리)
    today_date: Option<chrono::NaiveDate>,

    /// 통계
    trades_count: u32,
    wins: u32,
    total_pnl: Decimal,

    initialized: bool,
}

impl SectorVbStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            tickers: Vec::new(),
            context: None,
            sector_data: HashMap::new(),
            selected_sector: None,
            state: StrategyState::Rest,
            position: None,
            today_date: None,
            trades_count: 0,
            wins: 0,
            total_pnl: Decimal::ZERO,
            initialized: false,
        }
    }

    // ========================================================================
    // 시간 관련 헬퍼 (v2.0 - KST 명시적 처리)
    // ========================================================================

    /// UTC 시간을 KST 시간으로 변환 (시/분만 반환)
    fn utc_to_kst_time(utc_time: DateTime<Utc>) -> (u32, u32) {
        let kst_hour = (utc_time.hour() as i64 + KST_UTC_OFFSET_HOURS) % 24;
        (kst_hour as u32, utc_time.minute())
    }

    /// 장 마감 시간 근접 여부 확인 (KST 기준)
    fn is_near_market_close(&self, utc_time: DateTime<Utc>) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return false,
        };

        let (kst_hour, kst_minute) = Self::utc_to_kst_time(utc_time);

        // 청산 목표 시간: 15:30 - close_before_minutes
        let close_target_minute =
            KST_MARKET_CLOSE_MINUTE.saturating_sub(config.close_before_minutes);

        // 15:20 이후면 청산 준비 (close_before_minutes=10일 때)
        if kst_hour == KST_MARKET_CLOSE_HOUR {
            return kst_minute >= close_target_minute;
        }
        // 15:30 이후면 무조건 청산
        if kst_hour > KST_MARKET_CLOSE_HOUR {
            return true;
        }

        false
    }

    /// 새로운 거래일인지 확인
    fn is_new_trading_day(&self, current_time: DateTime<Utc>) -> bool {
        match self.today_date {
            Some(date) => current_time.date_naive() != date,
            None => true,
        }
    }

    /// 새 거래일 시작 처리
    fn on_new_day(&mut self, current_time: DateTime<Utc>) {
        self.today_date = Some(current_time.date_naive());
        self.state = StrategyState::Rest;
        self.selected_sector = None;

        // 전일 데이터 초기화 (today → prev)
        for data in self.sector_data.values_mut() {
            if data.today_open.is_some() {
                data.prev_close = data.current_price;
                data.prev_high = data.today_high;
                data.prev_low = data.today_low;
            }
            // 오늘 데이터 리셋
            data.today_open = None;
            data.today_high = Decimal::ZERO;
            data.today_low = Decimal::MAX;
            data.target_price = None;
        }

        info!(date = %current_time.date_naive(), "새 거래일 시작");
    }

    // ========================================================================
    // StrategyContext 연동 헬퍼 (v2.0)
    // ========================================================================

    /// StrategyContext에서 GlobalScore 조회
    async fn get_global_score(&self, ticker: &str) -> Option<f64> {
        let ctx = self.context.as_ref()?;
        let ctx_guard = ctx.read().await;
        ctx_guard.get_global_score(ticker).map(|gs| {
            use rust_decimal::prelude::ToPrimitive;
            gs.overall_score.to_f64().unwrap_or(0.0)
        })
    }

    /// StrategyContext에서 RouteState 조회
    async fn get_route_state(&self, ticker: &str) -> Option<RouteState> {
        let ctx = self.context.as_ref()?;
        let ctx_guard = ctx.read().await;
        ctx_guard.get_route_state(ticker).cloned()
    }

    /// StrategyContext에서 MarketRegime 조회
    async fn get_market_regime(&self, ticker: &str) -> Option<MarketRegime> {
        let ctx = self.context.as_ref()?;
        let ctx_guard = ctx.read().await;
        ctx_guard.get_market_regime(ticker).cloned()
    }

    /// GlobalScore 기반 신호 강도 계산.
    async fn get_adjusted_strength(&self, ticker: &str, base_strength: f64) -> f64 {
        if let Some(score) = self.get_global_score(ticker).await {
            adjust_strength_by_score(base_strength, Some(Decimal::try_from(score).unwrap_or_default()))
        } else {
            base_strength
        }
    }

    /// 섹터가 진입 가능한지 StrategyContext 기반으로 확인
    async fn can_enter_by_context(&self, ticker: &str) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return true,
        };

        // GlobalScore 체크
        if let Some(score) = self.get_global_score(ticker).await {
            if score < config.min_global_score {
                debug!(
                    ticker,
                    score,
                    min = config.min_global_score,
                    "GlobalScore 미달"
                );
                return false;
            }
        }

        // RouteState 체크
        if config.use_route_filter {
            if let Some(route) = self.get_route_state(ticker).await {
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

        // MarketRegime 체크
        if config.use_regime_filter {
            if let Some(regime) = self.get_market_regime(ticker).await {
                match regime {
                    MarketRegime::StrongUptrend | MarketRegime::BottomBounce => {
                        // 최적의 진입 환경
                    }
                    MarketRegime::Correction | MarketRegime::Sideways => {
                        // 진입 가능하지만 주의
                        debug!(ticker, ?regime, "MarketRegime 보통 - 주의 진입");
                    }
                    MarketRegime::Downtrend => {
                        if !config.allow_downtrend_entry {
                            debug!(ticker, ?regime, "MarketRegime 하락장 - 진입 거부");
                            return false;
                        }
                    }
                }
            }
        }

        true
    }

    // ========================================================================
    // 섹터 선택 및 신호 생성
    // ========================================================================

    /// 섹터 순위 계산 및 선택
    fn select_top_sectors(&mut self) {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return,
        };

        // 수익률 기준 정렬
        let mut ranked: Vec<_> = self
            .sector_data
            .values()
            .filter(|d| {
                d.prev_close > Decimal::ZERO && d.prev_volume >= Decimal::from(config.min_volume)
            })
            .collect();

        ranked.sort_by(|a, b| b.returns.cmp(&a.returns));

        if let Some(top) = ranked.first() {
            self.selected_sector = Some(top.ticker.clone());
            self.state = StrategyState::Ready;

            info!(
                sector = %top.ticker,
                returns = %top.returns,
                "상위 섹터 선택"
            );
        }
    }

    /// 돌파 목표가 계산
    fn calculate_target_price(&mut self, ticker: &str) {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return,
        };

        if let Some(data) = self.sector_data.get_mut(ticker) {
            if let Some(today_open) = data.today_open {
                let prev_range = data.prev_high - data.prev_low;
                let k = Decimal::from_f64_retain(config.k_factor).unwrap_or(dec!(0.5));
                data.target_price = Some(today_open + prev_range * k);

                debug!(
                    ticker = %ticker,
                    open = %today_open,
                    range = %prev_range,
                    k = %k,
                    target = ?data.target_price,
                    "돌파 목표가 계산"
                );
            }
        }
    }

    /// 신호 생성 (async - StrategyContext 조회 필요)
    async fn generate_signals(
        &mut self,
        ticker: &str,
        current_price: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Vec<Signal> {
        let config = match self.config.as_ref() {
            Some(c) => c.clone(),
            None => return Vec::new(),
        };

        let mut signals = Vec::new();

        // 선택된 섹터가 아니면 무시
        if self.selected_sector.as_ref() != Some(&ticker.to_string()) {
            return signals;
        }

        // 포지션 있을 때: 손절/익절/청산 확인
        if let Some(pos) = &self.position {
            if pos.ticker != ticker {
                return signals;
            }

            // 손절
            if current_price <= pos.stop_loss {
                let sym = self
                    .tickers
                    .iter()
                    .find(|s| s.starts_with(&format!("{}/", ticker)))
                    .cloned();
                if let Some(sym) = sym {
                    signals.push(
                        Signal::exit("sector_vb", sym, Side::Sell)
                            .with_strength(1.0)
                            .with_prices(Some(current_price), None, None)
                            .with_metadata("exit_reason", json!("stop_loss")),
                    );

                    let pnl = current_price - pos.entry_price;
                    self.total_pnl += pnl;
                    self.trades_count += 1;
                    self.position = None;
                    self.state = StrategyState::Rest;

                    info!(price = %current_price, pnl = %pnl, "손절 청산");
                }
                return signals;
            }

            // 익절
            if current_price >= pos.take_profit {
                let sym = self
                    .tickers
                    .iter()
                    .find(|s| s.starts_with(&format!("{}/", ticker)))
                    .cloned();
                if let Some(sym) = sym {
                    signals.push(
                        Signal::exit("sector_vb", sym, Side::Sell)
                            .with_strength(1.0)
                            .with_prices(Some(current_price), None, None)
                            .with_metadata("exit_reason", json!("take_profit")),
                    );

                    let pnl = current_price - pos.entry_price;
                    self.total_pnl += pnl;
                    self.trades_count += 1;
                    self.wins += 1;
                    self.position = None;
                    self.state = StrategyState::Rest;

                    info!(price = %current_price, pnl = %pnl, "익절 청산");
                }
                return signals;
            }

            // 장 마감 전 청산 (KST 15:20 이후)
            if self.is_near_market_close(timestamp) {
                let sym = self
                    .tickers
                    .iter()
                    .find(|s| s.starts_with(&format!("{}/", ticker)))
                    .cloned();
                if let Some(sym) = sym {
                    let (kst_hour, kst_minute) = Self::utc_to_kst_time(timestamp);

                    signals.push(
                        Signal::exit("sector_vb", sym, Side::Sell)
                            .with_strength(1.0)
                            .with_prices(Some(current_price), None, None)
                            .with_metadata("exit_reason", json!("market_close"))
                            .with_metadata(
                                "kst_time",
                                json!(format!("{:02}:{:02}", kst_hour, kst_minute)),
                            ),
                    );

                    let pnl = current_price - pos.entry_price;
                    self.total_pnl += pnl;
                    self.trades_count += 1;
                    if pnl > Decimal::ZERO {
                        self.wins += 1;
                    }
                    self.position = None;
                    self.state = StrategyState::Rest;

                    info!(
                        price = %current_price,
                        pnl = %pnl,
                        kst_time = format!("{:02}:{:02}", kst_hour, kst_minute),
                        "장마감 전 청산"
                    );
                }
                return signals;
            }

            return signals;
        }

        // 포지션 없을 때: 돌파 확인
        if self.state != StrategyState::Ready {
            return signals;
        }

        // StrategyContext 기반 진입 조건 확인 (v2.0)
        if !self.can_enter_by_context(ticker).await {
            return signals;
        }

        if let Some(data) = self.sector_data.get(ticker) {
            if let Some(target) = data.target_price {
                if current_price >= target {
                    // 돌파! 매수
                    let sym = self
                        .tickers
                        .iter()
                        .find(|s| s.starts_with(&format!("{}/", ticker)))
                        .cloned();
                    if let Some(sym) = sym {
                        let stop_loss = current_price
                            * (dec!(1)
                                - Decimal::from_f64_retain(config.stop_loss_pct / 100.0)
                                    .unwrap_or(dec!(0.02)));
                        let take_profit = current_price
                            * (dec!(1)
                                + Decimal::from_f64_retain(config.take_profit_pct / 100.0)
                                    .unwrap_or(dec!(0.03)));

                        let strength = self.get_adjusted_strength(ticker, 0.5).await;
                        signals.push(
                            Signal::entry("sector_vb", sym.clone(), Side::Buy)
                                .with_strength(strength)
                                .with_prices(
                                    Some(current_price),
                                    Some(stop_loss),
                                    Some(take_profit),
                                )
                                .with_metadata("target_price", json!(target.to_string()))
                                .with_metadata("sector", json!(ticker)),
                        );

                        self.position = Some(PositionState {
                            ticker: ticker.to_string(),
                            entry_price: current_price,
                            stop_loss,
                            take_profit,
                            entry_time: timestamp,
                        });
                        self.state = StrategyState::Investing;

                        info!(
                            sector = %ticker,
                            target = %target,
                            entry = %current_price,
                            stop_loss = %stop_loss,
                            take_profit = %take_profit,
                            "돌파 진입"
                        );
                    }
                }
            }
        }

        signals
    }
}

impl Default for SectorVbStrategy {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Strategy 트레이트 구현
// ============================================================================

#[async_trait]
impl Strategy for SectorVbStrategy {
    fn name(&self) -> &str {
        "Sector VB"
    }

    fn version(&self) -> &str {
        "2.0.0"
    }

    fn description(&self) -> &str {
        "섹터 변동성 돌파 전략 v2.0. 한국 섹터 ETF 중 전일 수익률이 가장 높은 섹터를 \
         선택하여 변동성 돌파 시 진입. StrategyContext 연동으로 GlobalScore/RouteState/MarketRegime 필터링."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let vb_config: SectorVbConfig = serde_json::from_value(config)?;

        info!(
            tickers = ?vb_config.tickers,
            k_factor = vb_config.k_factor,
            top_n = vb_config.top_n,
            min_global_score = vb_config.min_global_score,
            use_route_filter = vb_config.use_route_filter,
            use_regime_filter = vb_config.use_regime_filter,
            "섹터 변동성 돌파 전략 v2.0 초기화"
        );

        // 티커 생성
        for ticker_str in &vb_config.tickers {
            let ticker = format!("{}/KRW", ticker_str);
            self.tickers.push(ticker);
            self.sector_data
                .insert(ticker_str.clone(), SectorData::new(ticker_str.clone()));
        }

        self.config = Some(vb_config);
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

        // base 티커만 추출 (091160/KRW -> 091160)
        let ticker_str = data.ticker.clone();

        // 등록된 섹터인지 확인
        if !self.sector_data.contains_key(&ticker_str) {
            return Ok(vec![]);
        }

        // kline에서 OHLCV 추출
        let (open, high, low, close, volume, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (
                kline.open,
                kline.high,
                kline.low,
                kline.close,
                kline.volume,
                kline.open_time,
            ),
            _ => return Ok(vec![]),
        };

        // 새 거래일 확인
        if self.is_new_trading_day(timestamp) {
            self.on_new_day(timestamp);
        }

        // 섹터 데이터 업데이트
        let need_calc_target = if let Some(sector) = self.sector_data.get_mut(&ticker_str) {
            sector.current_price = close;

            // 오늘 첫 데이터인지 확인하고 처리
            let calc_target = if sector.today_open.is_none() {
                sector.today_open = Some(open);
                sector.today_high = high;
                sector.today_low = low;

                // 전일 수익률 계산
                if sector.prev_close > Decimal::ZERO {
                    sector.returns = (open - sector.prev_close) / sector.prev_close * dec!(100);
                }

                true // 돌파 목표가 계산 필요
            } else {
                // 고저 업데이트
                sector.today_high = sector.today_high.max(high);
                sector.today_low = sector.today_low.min(low);
                false
            };

            sector.prev_volume = volume;
            calc_target
        } else {
            false
        };

        // mutable borrow가 끝난 후 돌파 목표가 계산
        if need_calc_target {
            self.calculate_target_price(&ticker_str);
        }

        // 섹터 선택 (아직 안 됐으면)
        if self.state == StrategyState::Rest && self.selected_sector.is_none() {
            // 모든 섹터의 오늘 시가가 있는지 확인
            let all_have_open = self.sector_data.values().all(|d| d.today_open.is_some());
            if all_have_open {
                self.select_top_sectors();
            }
        }

        // 신호 생성 (async)
        let signals = self.generate_signals(&ticker_str, close, timestamp).await;

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
        let win_rate = if self.trades_count > 0 {
            (self.wins as f64 / self.trades_count as f64) * 100.0
        } else {
            0.0
        };

        info!(
            trades = self.trades_count,
            wins = self.wins,
            win_rate = %format!("{:.1}%", win_rate),
            total_pnl = %self.total_pnl,
            "섹터 변동성 돌파 전략 v2.0 종료"
        );

        Ok(())
    }

    fn get_state(&self) -> Value {
        json!({
            "version": "2.0.0",
            "initialized": self.initialized,
            "state": format!("{:?}", self.state),
            "selected_sector": self.selected_sector,
            "has_position": self.position.is_some(),
            "sector_count": self.sector_data.len(),
            "trades_count": self.trades_count,
            "wins": self.wins,
            "total_pnl": self.total_pnl.to_string(),
            "context_connected": self.context.is_some(),
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into SectorVb v2.0 strategy");
    }
}

// ============================================================================
// 테스트
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[tokio::test]
    async fn test_sector_vb_initialization() {
        let mut strategy = SectorVbStrategy::new();

        let config = json!({
            "tickers": ["091160", "091230", "305720"],
            "k_factor": 0.5,
            "top_n": 1,
            "min_global_score": 50.0
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
        assert_eq!(strategy.sector_data.len(), 3);
    }

    #[test]
    fn test_default_sector_list() {
        let sectors = default_sector_list();
        assert_eq!(sectors.len(), 10);
        assert!(sectors.contains(&"091160".to_string()));
    }

    #[test]
    fn test_utc_to_kst_conversion() {
        // UTC 06:20 = KST 15:20
        let utc_time = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 6, 20, 0).unwrap();
        let (kst_hour, kst_minute) = SectorVbStrategy::utc_to_kst_time(utc_time);
        assert_eq!(kst_hour, 15);
        assert_eq!(kst_minute, 20);
    }

    #[test]
    fn test_near_market_close() {
        let mut strategy = SectorVbStrategy::new();
        strategy.config = Some(SectorVbConfig {
            close_before_minutes: 10,
            ..Default::default()
        });

        // KST 15:19 = UTC 06:19 -> 아직 청산 시간 아님
        let utc_time_before = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 6, 19, 0).unwrap();
        assert!(!strategy.is_near_market_close(utc_time_before));

        // KST 15:20 = UTC 06:20 -> 청산 시간
        let utc_time_at = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 6, 20, 0).unwrap();
        assert!(strategy.is_near_market_close(utc_time_at));

        // KST 15:25 = UTC 06:25 -> 청산 시간
        let utc_time_after = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 6, 25, 0).unwrap();
        assert!(strategy.is_near_market_close(utc_time_after));
    }

    #[test]
    fn test_default_config() {
        let config = SectorVbConfig::default();
        assert_eq!(config.k_factor, 0.5);
        assert_eq!(config.min_global_score, 50.0);
        assert!(config.use_route_filter);
        assert!(config.use_regime_filter);
        assert!(!config.allow_downtrend_entry);
    }
}

// 전략 레지스트리에 자동 등록
register_strategy! {
    id: "sector_vb",
    aliases: ["sector_volatility", "sector_breakout"],
    name: "섹터 변동성 돌파",
    description: "섹터별 변동성 돌파 전략 v2.0. StrategyContext 연동.",
    timeframe: "5m",
    tickers: ["091160", "091170", "091180", "091220", "091230"],
    category: Intraday,
    markets: [Stock],
    type: SectorVbStrategy,
    config: SectorVbConfig
}
