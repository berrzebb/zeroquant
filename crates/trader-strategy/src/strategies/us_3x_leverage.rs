//! 미국 3배 레버리지/인버스 조합 전략 (US 3X Leverage) v2.0
//!
//! 3배 레버리지 ETF와 인버스 ETF를 조합하여 양방향 수익을 추구하는 전략.
//!
//! # 핵심 로직
//! 1. 상승장: 레버리지 ETF 비중 확대 (TQQQ, SOXL)
//! 2. 하락장: 인버스 ETF 비중 확대 (SQQQ, SOXS)
//! 3. 위기 상황: 전량 현금화 또는 인버스 집중
//!
//! # StrategyContext 연동 (v2.0)
//! - GlobalScore: 최소 점수 이상인 ETF만 매수
//! - RouteState: Wait/Overheat 상태에서 진입 제한
//! - MarketRegime: 시장 상태에 따른 동적 비중 조절
//! - MacroEnvironment: 매크로 위험 수준에 따른 방어 전환
//!
//! # 대상 ETF
//! - **레버리지**: TQQQ (나스닥 3배), SOXL (반도체 3배)
//! - **인버스**: SQQQ (나스닥 인버스 3배), SOXS (반도체 인버스 3배)

use crate::register_strategy;
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use trader_core::domain::StrategyContext;
use trader_core::types::Timeframe;
use trader_core::{
    MacroRisk, MarketData, MarketDataType, MarketRegime, Order, Position, RouteState, Side, Signal,
};
use trader_strategy_macro::StrategyConfig;

use crate::strategies::common::ExitConfig;

// ============================================================================
// 설정
// ============================================================================

/// 개별 ETF 배분 설정
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EtfAllocation {
    /// ETF 티커
    pub ticker: String,
    /// 목표 비율 (0.0 ~ 1.0)
    pub target_ratio: f64,
    /// 레버리지/인버스 타입
    #[serde(default)]
    pub etf_type: EtfType,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[derive(Default)]
pub enum EtfType {
    #[serde(rename = "leverage")]
    #[default]
    Leverage,
    #[serde(rename = "inverse")]
    Inverse,
}


/// 미국 3배 레버리지 전략 설정 v2.0
#[derive(Debug, Clone, Deserialize, Serialize, StrategyConfig)]
#[strategy(
    id = "us_3x_leverage",
    name = "미국 3배 레버리지",
    description = "미국 레버리지/인버스 ETF 조합으로 양방향 수익을 추구하는 전략",
    category = "Daily"
)]
pub struct Us3xLeverageConfig {
    /// ETF 배분 리스트
    #[serde(default = "default_allocations")]
    #[schema(label = "ETF 배분", skip)]
    pub allocations: Vec<EtfAllocation>,

    /// 리밸런싱 임계값 (비율 이탈 %, 기본값: 5%)
    #[serde(default = "default_rebalance_threshold")]
    #[schema(label = "리밸런싱 임계값 (%)", min = 1, max = 20, default = 5, section = "timing")]
    pub rebalance_threshold: f64,

    /// 리밸런싱 주기 (일, 기본값: 30일)
    #[serde(default = "default_rebalance_period_days")]
    #[schema(label = "리밸런싱 주기 (일)", min = 1, max = 90, default = 30, section = "timing")]
    pub rebalance_period_days: u32,

    /// 레버리지 MA 기간 (기본값: 20)
    #[serde(default = "default_ma_period")]
    #[schema(label = "이동평균 기간", min = 5, max = 200, default = 20, section = "indicator")]
    pub ma_period: usize,

    /// 하락장 인버스 최대 비중 (기본값: 60%)
    #[serde(default = "default_max_inverse_ratio")]
    #[schema(label = "인버스 최대 비중 (%)", min = 0, max = 100, default = 60, section = "sizing")]
    pub max_inverse_ratio: f64,

    /// 레버리지 최대 손실 시 전량 매도 (기본값: 30%)
    #[serde(default = "default_max_drawdown")]
    #[schema(label = "최대 손실률 (%)", min = 5, max = 50, default = 30, section = "sizing")]
    pub max_drawdown_pct: f64,

    // ========== StrategyContext 연동 설정 (v2.0) ==========
    /// 최소 글로벌 스코어 (기본값: 55.0)
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", min = 0, max = 100, default = 55, section = "filter")]
    pub min_global_score: f64,

    /// RouteState 필터 사용 여부 (기본값: true)
    #[serde(default = "default_use_route_filter")]
    #[schema(label = "RouteState 필터 사용", default = true, section = "filter")]
    pub use_route_filter: bool,

    /// MarketRegime 기반 동적 배분 사용 여부 (기본값: true)
    #[serde(default = "default_use_regime_allocation")]
    #[schema(label = "MarketRegime 동적 배분", default = true, section = "filter")]
    pub use_regime_allocation: bool,

    /// MacroRisk 기반 방어 전환 사용 여부 (기본값: true)
    #[serde(default = "default_use_macro_risk")]
    #[schema(label = "MacroRisk 방어 전환", default = true, section = "filter")]
    pub use_macro_risk: bool,

    /// 위기 상황 시 전량 현금화 (기본값: false)
    #[serde(default)]
    #[schema(label = "위기 시 현금화", default = false, section = "filter")]
    pub cash_out_on_crisis: bool,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

fn default_allocations() -> Vec<EtfAllocation> {
    vec![
        EtfAllocation {
            ticker: "TQQQ".to_string(),
            target_ratio: 0.35,
            etf_type: EtfType::Leverage,
        },
        EtfAllocation {
            ticker: "SOXL".to_string(),
            target_ratio: 0.35,
            etf_type: EtfType::Leverage,
        },
        EtfAllocation {
            ticker: "SQQQ".to_string(),
            target_ratio: 0.15,
            etf_type: EtfType::Inverse,
        },
        EtfAllocation {
            ticker: "SOXS".to_string(),
            target_ratio: 0.15,
            etf_type: EtfType::Inverse,
        },
    ]
}

fn default_rebalance_threshold() -> f64 {
    5.0
}
fn default_rebalance_period_days() -> u32 {
    30
}
fn default_ma_period() -> usize {
    20
}
fn default_max_inverse_ratio() -> f64 {
    0.6
}
fn default_max_drawdown() -> f64 {
    30.0
}
fn default_min_global_score() -> f64 {
    55.0
}
fn default_use_route_filter() -> bool {
    true
}
fn default_use_regime_allocation() -> bool {
    true
}
fn default_use_macro_risk() -> bool {
    true
}

impl Default for Us3xLeverageConfig {
    fn default() -> Self {
        Self {
            allocations: default_allocations(),
            rebalance_threshold: 5.0,
            rebalance_period_days: 30,
            ma_period: 20,
            max_inverse_ratio: 0.6,
            max_drawdown_pct: 30.0,
            min_global_score: 55.0,
            use_route_filter: true,
            use_regime_allocation: true,
            use_macro_risk: true,
            cash_out_on_crisis: false,
            exit_config: ExitConfig::for_leverage(),
        }
    }
}

// ============================================================================
// 내부 데이터 구조체
// ============================================================================

/// ETF 데이터 (가격 히스토리는 StrategyContext에서 가져옴)
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct EtfData {
    ticker: String,
    etf_type: EtfType,
    base_target_ratio: f64, // 설정 파일의 기본 비율
    target_ratio: f64,      // 현재 적용 비율 (동적 조절 가능)
    current_ratio: f64,
    current_price: Decimal,
    holdings: Decimal,
    entry_price: Decimal,
    ma_period: usize,
}

impl EtfData {
    fn new(ticker: String, target_ratio: f64, etf_type: EtfType, ma_period: usize) -> Self {
        Self {
            ticker,
            etf_type,
            base_target_ratio: target_ratio,
            target_ratio,
            current_ratio: 0.0,
            current_price: Decimal::ZERO,
            holdings: Decimal::ZERO,
            entry_price: Decimal::ZERO,
            ma_period,
        }
    }

    fn update_price(&mut self, price: Decimal) {
        self.current_price = price;
    }
}

/// 시장 환경 상태
#[derive(Debug, Clone, Copy, PartialEq)]
enum MarketEnvironment {
    Bullish, // 강세장 - 레버리지 70%+
    Neutral, // 중립 - 기본 배분
    Bearish, // 약세장 - 인버스 비중 증가
    Crisis,  // 위기 - 전량 현금화 또는 인버스 집중
}

// ============================================================================
// 전략 구현
// ============================================================================

/// 미국 3배 레버리지 전략 v2.0
pub struct Us3xLeverageStrategy {
    config: Option<Us3xLeverageConfig>,
    tickers: Vec<String>,
    context: Option<Arc<RwLock<StrategyContext>>>,

    /// ETF별 데이터
    etf_data: HashMap<String, EtfData>,

    /// 총 포트폴리오 가치
    total_value: Decimal,

    /// 마지막 리밸런싱 날짜
    last_rebalance_date: Option<chrono::NaiveDate>,

    /// 현재 날짜
    current_date: Option<chrono::NaiveDate>,

    /// 포트폴리오 고점
    portfolio_high: Decimal,

    /// 현재 시장 환경
    market_env: MarketEnvironment,

    /// 초기화 완료
    started: bool,

    /// 통계
    rebalance_count: u32,
    total_pnl: Decimal,

    initialized: bool,
}

impl Us3xLeverageStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            tickers: Vec::new(),
            context: None,
            etf_data: HashMap::new(),
            total_value: Decimal::ZERO,
            last_rebalance_date: None,
            current_date: None,
            portfolio_high: Decimal::ZERO,
            market_env: MarketEnvironment::Neutral,
            started: false,
            rebalance_count: 0,
            total_pnl: Decimal::ZERO,
            initialized: false,
        }
    }

    // ========================================================================
    // StrategyContext 연동 헬퍼 (v2.0)
    // ========================================================================

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

    /// StrategyContext에서 MacroRisk 조회
    async fn get_macro_risk(&self) -> Option<MacroRisk> {
        let ctx = self.context.as_ref()?;
        let ctx_guard = ctx.read().await;
        ctx_guard
            .get_macro_environment()
            .map(|m| m.risk_level)
    }

    /// StrategyContext에서 klines를 가져와 MA 위에 있는지 확인
    async fn is_etf_above_ma(&self, ticker: &str, ma_period: usize, current_price: Decimal) -> bool {
        let ctx = match self.context.as_ref() {
            Some(c) => c,
            None => return false,
        };
        let ctx_guard = ctx.read().await;
        let klines = ctx_guard.get_klines(ticker, Timeframe::D1);

        if klines.len() < ma_period {
            return false;
        }

        // 최근 ma_period개의 종가 평균
        let sum: Decimal = klines.iter().rev().take(ma_period).map(|k| k.close).sum();
        let ma = sum / Decimal::from(ma_period);

        current_price > ma
    }

    /// 종목별 진입 가능 여부 확인 (v2.0 - async).
    /// 미국 ETF 전략이므로 GlobalScore 체크 불필요.
    async fn can_enter(&self, ticker: &str) -> bool {
        let config = match &self.config {
            Some(cfg) => cfg,
            None => return true,
        };

        // RouteState 체크 - Overheat 시만 진입 제한
        if config.use_route_filter {
            if let Some(route) = self.get_route_state(ticker).await {
                match route {
                    RouteState::Overheat => {
                        debug!(ticker, ?route, "시장 과열 - 진입 제한");
                        return false;
                    }
                    _ => {}
                }
            }
        }

        true
    }

    // ========================================================================
    // 시장 환경 판단 (v2.0)
    // ========================================================================

    /// 시장 환경 업데이트 (MarketRegime + MacroRisk 기반)
    async fn update_market_environment(&mut self) {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return,
        };

        // 1. MacroRisk 확인 (최우선)
        if config.use_macro_risk {
            if let Some(risk) = self.get_macro_risk().await {
                match risk {
                    MacroRisk::Critical => {
                        self.market_env = MarketEnvironment::Crisis;
                        info!("매크로 위험 Critical - 위기 모드 전환");
                        return;
                    }
                    MacroRisk::High => {
                        self.market_env = MarketEnvironment::Bearish;
                        info!("매크로 위험 High - 약세장 모드 전환");
                        return;
                    }
                    MacroRisk::Normal => {
                        // MacroRisk 정상 - MarketRegime으로 판단
                    }
                }
            }
        }

        // 2. MarketRegime 기반 판단
        if config.use_regime_allocation {
            // 대표 레버리지 ETF의 MarketRegime 확인 (TQQQ)
            if let Some(regime) = self.get_market_regime("TQQQ").await {
                match regime {
                    MarketRegime::StrongUptrend => {
                        self.market_env = MarketEnvironment::Bullish;
                        debug!("MarketRegime StrongUptrend - 강세장");
                        return;
                    }
                    MarketRegime::BottomBounce => {
                        self.market_env = MarketEnvironment::Neutral;
                        debug!("MarketRegime BottomBounce - 중립");
                        return;
                    }
                    MarketRegime::Sideways => {
                        self.market_env = MarketEnvironment::Neutral;
                        debug!("MarketRegime Sideways - 중립");
                        return;
                    }
                    MarketRegime::Correction => {
                        self.market_env = MarketEnvironment::Bearish;
                        debug!("MarketRegime Correction - 약세장");
                        return;
                    }
                    MarketRegime::Downtrend => {
                        self.market_env = MarketEnvironment::Bearish;
                        debug!("MarketRegime Downtrend - 약세장");
                        return;
                    }
                }
            }
        }

        // 3. 폴백: MA 기반 판단
        let mut bullish_count = 0;

        // borrow 문제 회피를 위해 필요한 데이터 미리 추출
        let leverage_etfs: Vec<_> = self
            .etf_data
            .values()
            .filter(|d| d.etf_type == EtfType::Leverage)
            .map(|d| (d.ticker.clone(), d.ma_period, d.current_price))
            .collect();

        let total_leverage = leverage_etfs.len();

        for (ticker, ma_period, current_price) in leverage_etfs {
            if self.is_etf_above_ma(&ticker, ma_period, current_price).await {
                bullish_count += 1;
            }
        }

        if total_leverage > 0 {
            if bullish_count >= total_leverage {
                self.market_env = MarketEnvironment::Bullish;
            } else if bullish_count == 0 {
                self.market_env = MarketEnvironment::Bearish;
            } else {
                self.market_env = MarketEnvironment::Neutral;
            }
        }

        debug!(
            bullish_count,
            total_leverage,
            ?self.market_env,
            "시장 환경 업데이트 (MA 기반)"
        );
    }

    /// 목표 비율 조정 (시장 환경 기반)
    fn adjust_target_ratios(&mut self) {
        let config = match self.config.as_ref() {
            Some(c) => c.clone(),
            None => return,
        };

        match self.market_env {
            MarketEnvironment::Bullish => {
                // 강세장: 레버리지 비중 최대화
                let leverage_ratio = 0.8;
                let inverse_ratio = 0.2;
                self.set_etf_ratios(leverage_ratio, inverse_ratio);
                debug!(
                    "강세장 비중: 레버리지 {:.0}%, 인버스 {:.0}%",
                    leverage_ratio * 100.0,
                    inverse_ratio * 100.0
                );
            }
            MarketEnvironment::Neutral => {
                // 중립: 기본 비율로 복귀
                for alloc in &config.allocations {
                    if let Some(data) = self.etf_data.get_mut(&alloc.ticker) {
                        data.target_ratio = data.base_target_ratio;
                    }
                }
                debug!("중립 비중: 기본값 복귀");
            }
            MarketEnvironment::Bearish => {
                // 약세장: 인버스 비중 증가
                let inverse_ratio = config.max_inverse_ratio;
                let leverage_ratio = 1.0 - inverse_ratio;
                self.set_etf_ratios(leverage_ratio, inverse_ratio);
                debug!(
                    "약세장 비중: 레버리지 {:.0}%, 인버스 {:.0}%",
                    leverage_ratio * 100.0,
                    inverse_ratio * 100.0
                );
            }
            MarketEnvironment::Crisis => {
                if config.cash_out_on_crisis {
                    // 전량 현금화 (모든 비율 0으로 - 신호 생성 시 청산)
                    for data in self.etf_data.values_mut() {
                        data.target_ratio = 0.0;
                    }
                    warn!("위기 모드: 전량 현금화 예정");
                } else {
                    // 인버스 집중
                    self.set_etf_ratios(0.1, 0.9);
                    warn!("위기 모드: 인버스 90% 집중");
                }
            }
        }
    }

    /// 레버리지/인버스 비율 설정 헬퍼
    fn set_etf_ratios(&mut self, leverage_total: f64, inverse_total: f64) {
        let leverage_count = self
            .etf_data
            .values()
            .filter(|d| d.etf_type == EtfType::Leverage)
            .count();
        let inverse_count = self
            .etf_data
            .values()
            .filter(|d| d.etf_type == EtfType::Inverse)
            .count();

        for data in self.etf_data.values_mut() {
            data.target_ratio = match data.etf_type {
                EtfType::Leverage if leverage_count > 0 => leverage_total / leverage_count as f64,
                EtfType::Inverse if inverse_count > 0 => inverse_total / inverse_count as f64,
                _ => 0.0,
            };
        }
    }

    // ========================================================================
    // 기타 헬퍼
    // ========================================================================

    /// 새로운 날인지 확인
    fn is_new_day(&self, current_time: DateTime<Utc>) -> bool {
        match self.current_date {
            Some(date) => current_time.date_naive() != date,
            None => true,
        }
    }

    /// 리밸런싱 필요 여부 확인
    fn needs_rebalancing(&self, current_time: DateTime<Utc>) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return false,
        };

        // 주기적 리밸런싱
        if let Some(last_date) = self.last_rebalance_date {
            let days_since = (current_time.date_naive() - last_date).num_days();
            if days_since >= config.rebalance_period_days as i64 {
                return true;
            }
        }

        // 비율 이탈 체크
        for data in self.etf_data.values() {
            let diff = (data.current_ratio - data.target_ratio).abs() * 100.0;
            if diff >= config.rebalance_threshold {
                return true;
            }
        }

        false
    }

    /// 현재 비율 계산
    fn calculate_current_ratios(&mut self) {
        if self.total_value <= Decimal::ZERO {
            return;
        }

        for data in self.etf_data.values_mut() {
            let value = data.holdings * data.current_price;
            data.current_ratio = (value / self.total_value).to_f64().unwrap_or(0.0);
        }
    }

    /// 리밸런싱 신호 생성
    async fn generate_rebalance_signals(&mut self, timestamp: DateTime<Utc>) -> Vec<Signal> {
        let mut signals = Vec::new();

        self.calculate_current_ratios();
        self.update_market_environment().await;
        self.adjust_target_ratios();

        for (ticker, data) in &self.etf_data {
            let target = data.target_ratio;
            let current = data.current_ratio;
            let diff = target - current;

            if diff.abs() < 0.01 {
                continue; // 1% 미만 차이는 무시
            }

            let full_ticker = match self
                .tickers
                .iter()
                .find(|s| s.starts_with(&format!("{}/", ticker)))
            {
                Some(s) => s.clone(),
                None => continue,
            };

            if diff > 0.0 {
                // 매수 필요 - StrategyContext 기반 진입 체크
                if !self.can_enter(ticker).await {
                    debug!(ticker, "리밸런싱 매수 스킵 - can_enter() false");
                    continue;
                }

                info!(
                    ticker,
                    target = %format!("{:.1}%", target * 100.0),
                    current = %format!("{:.1}%", current * 100.0),
                    ?self.market_env,
                    "리밸런싱 매수"
                );

                signals.push(
                    Signal::entry("us_3x_leverage", full_ticker, Side::Buy)
                        .with_strength(diff.abs())
                        .with_prices(Some(data.current_price), None, None)
                        .with_metadata("action", json!("rebalance_buy"))
                        .with_metadata("target_ratio", json!(target))
                        .with_metadata("current_ratio", json!(current))
                        .with_metadata("market_env", json!(format!("{:?}", self.market_env))),
                );
            } else {
                // 매도 필요
                info!(
                    ticker,
                    target = %format!("{:.1}%", target * 100.0),
                    current = %format!("{:.1}%", current * 100.0),
                    ?self.market_env,
                    "리밸런싱 매도"
                );

                signals.push(
                    Signal::exit("us_3x_leverage", full_ticker, Side::Sell)
                        .with_strength(diff.abs())
                        .with_prices(Some(data.current_price), None, None)
                        .with_metadata("action", json!("rebalance_sell"))
                        .with_metadata("target_ratio", json!(target))
                        .with_metadata("current_ratio", json!(current))
                        .with_metadata("market_env", json!(format!("{:?}", self.market_env))),
                );
            }
        }

        if !signals.is_empty() {
            self.last_rebalance_date = Some(timestamp.date_naive());
            self.rebalance_count += 1;
        }

        signals
    }

    /// 초기 진입 신호 생성
    async fn generate_initial_signals(&mut self) -> Vec<Signal> {
        let mut signals = Vec::new();

        // 초기 진입 전 시장 환경 확인
        self.update_market_environment().await;
        self.adjust_target_ratios();

        for (ticker, data) in &self.etf_data {
            if data.target_ratio <= 0.0 {
                continue;
            }

            // StrategyContext 기반 진입 체크
            if !self.can_enter(ticker).await {
                debug!(ticker, "초기 진입 스킵 - can_enter() false");
                continue;
            }

            let full_ticker = match self
                .tickers
                .iter()
                .find(|s| s.starts_with(&format!("{}/", ticker)))
            {
                Some(s) => s.clone(),
                None => continue,
            };

            info!(
                ticker,
                ratio = %format!("{:.1}%", data.target_ratio * 100.0),
                ?self.market_env,
                "초기 매수"
            );

            signals.push(
                Signal::entry("us_3x_leverage", full_ticker, Side::Buy)
                    .with_strength(data.target_ratio)
                    .with_prices(Some(data.current_price), None, None)
                    .with_metadata("action", json!("initial_buy"))
                    .with_metadata("target_ratio", json!(data.target_ratio))
                    .with_metadata("market_env", json!(format!("{:?}", self.market_env))),
            );
        }

        self.started = true;
        signals
    }

    /// 드로다운 체크
    fn check_drawdown(&mut self) -> Option<Vec<Signal>> {
        let config = self.config.as_ref()?;

        if self.portfolio_high <= Decimal::ZERO {
            return None;
        }

        let drawdown = ((self.portfolio_high - self.total_value) / self.portfolio_high * dec!(100))
            .to_f64()
            .unwrap_or(0.0);

        if drawdown >= config.max_drawdown_pct {
            // 레버리지 ETF 전량 청산
            let mut signals = Vec::new();

            for (ticker, data) in &self.etf_data {
                if data.holdings <= Decimal::ZERO {
                    continue;
                }

                if data.etf_type == EtfType::Leverage {
                    let full_ticker = match self
                        .tickers
                        .iter()
                        .find(|s| s.starts_with(&format!("{}/", ticker)))
                    {
                        Some(s) => s.clone(),
                        None => continue,
                    };

                    warn!(
                        ticker,
                        drawdown = %format!("{:.1}%", drawdown),
                        "최대 드로다운 도달 - 레버리지 청산"
                    );

                    signals.push(
                        Signal::exit("us_3x_leverage", full_ticker, Side::Sell)
                            .with_strength(1.0)
                            .with_prices(Some(data.current_price), None, None)
                            .with_metadata("action", json!("drawdown_exit"))
                            .with_metadata("drawdown_pct", json!(drawdown)),
                    );
                }
            }

            return Some(signals);
        }

        None
    }
}

impl Default for Us3xLeverageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Strategy 트레이트 구현
// ============================================================================

#[async_trait]
impl Strategy for Us3xLeverageStrategy {
    fn name(&self) -> &str {
        "US 3X Leverage"
    }

    fn version(&self) -> &str {
        "2.0.0"
    }

    fn description(&self) -> &str {
        "미국 3배 레버리지/인버스 조합 전략 v2.0. StrategyContext 연동으로 \
         MarketRegime/MacroRisk 기반 동적 비중 조절."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let lev_config: Us3xLeverageConfig = serde_json::from_value(config)?;

        info!(
            allocations = ?lev_config.allocations.iter().map(|a| &a.ticker).collect::<Vec<_>>(),
            rebalance_days = lev_config.rebalance_period_days,
            use_regime_allocation = lev_config.use_regime_allocation,
            use_macro_risk = lev_config.use_macro_risk,
            min_global_score = lev_config.min_global_score,
            "미국 3배 레버리지 전략 v2.0 초기화"
        );

        // ETF 데이터 초기화
        for alloc in &lev_config.allocations {
            let ticker = format!("{}/USD", alloc.ticker);
            self.tickers.push(ticker);
            self.etf_data.insert(
                alloc.ticker.clone(),
                EtfData::new(
                    alloc.ticker.clone(),
                    alloc.target_ratio,
                    alloc.etf_type.clone(),
                    lev_config.ma_period,
                ),
            );
        }

        self.config = Some(lev_config);
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

        // base 티커만 추출 (TQQQ/USD -> TQQQ)
        let ticker_str = data.ticker.clone();

        // 등록된 ETF인지 확인
        if !self.etf_data.contains_key(&ticker_str) {
            return Ok(vec![]);
        }

        // kline에서 데이터 추출
        let (close, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (kline.close, kline.open_time),
            _ => return Ok(vec![]),
        };

        // 새 날짜 확인
        let new_day = self.is_new_day(timestamp);
        if new_day {
            self.current_date = Some(timestamp.date_naive());
        }

        // ETF 데이터 업데이트
        if let Some(etf) = self.etf_data.get_mut(&ticker_str) {
            etf.update_price(close);
        }

        // 총 가치 계산
        self.total_value = self
            .etf_data
            .values()
            .map(|d| d.holdings * d.current_price)
            .sum();

        // 포트폴리오 고점 업데이트
        if self.total_value > self.portfolio_high {
            self.portfolio_high = self.total_value;
        }

        // 아직 시작 안 했으면 초기 진입
        if !self.started {
            // 모든 ETF 가격이 있는지 확인
            let all_have_price = self
                .etf_data
                .values()
                .all(|d| d.current_price > Decimal::ZERO);
            if all_have_price {
                return Ok(self.generate_initial_signals().await);
            }
            return Ok(vec![]);
        }

        // 드로다운 체크
        if let Some(signals) = self.check_drawdown() {
            return Ok(signals);
        }

        // 리밸런싱 체크 (새 날에만)
        if new_day && self.needs_rebalancing(timestamp) {
            return Ok(self.generate_rebalance_signals(timestamp).await);
        }

        Ok(vec![])
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ticker = order.ticker.to_string();

        if let Some(etf) = self.etf_data.get_mut(&ticker) {
            match order.side {
                Side::Buy => {
                    let old_value = etf.holdings * etf.entry_price;
                    let new_value = order.quantity * order.price.unwrap_or(etf.current_price);
                    let total_qty = etf.holdings + order.quantity;

                    if total_qty > Decimal::ZERO {
                        etf.entry_price = (old_value + new_value) / total_qty;
                    }
                    etf.holdings += order.quantity;
                }
                Side::Sell => {
                    let pnl = order.quantity
                        * (order.price.unwrap_or(etf.current_price) - etf.entry_price);
                    self.total_pnl += pnl;
                    etf.holdings -= order.quantity;

                    if etf.holdings <= Decimal::ZERO {
                        etf.holdings = Decimal::ZERO;
                        etf.entry_price = Decimal::ZERO;
                    }
                }
            }
        }

        debug!(
            ticker,
            side = ?order.side,
            quantity = %order.quantity,
            "주문 체결"
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ticker = position.ticker.to_string();

        if let Some(etf) = self.etf_data.get_mut(&ticker) {
            etf.holdings = position.quantity;
            if position.quantity > Decimal::ZERO {
                etf.entry_price = position.entry_price;
            }
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            rebalance_count = self.rebalance_count,
            total_pnl = %self.total_pnl,
            final_value = %self.total_value,
            final_env = ?self.market_env,
            "미국 3배 레버리지 전략 v2.0 종료"
        );

        Ok(())
    }

    fn get_state(&self) -> Value {
        let holdings: HashMap<_, _> = self
            .etf_data
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    json!({
                        "holdings": v.holdings.to_string(),
                        "current_ratio": format!("{:.1}%", v.current_ratio * 100.0),
                        "target_ratio": format!("{:.1}%", v.target_ratio * 100.0),
                        "etf_type": format!("{:?}", v.etf_type),
                    }),
                )
            })
            .collect();

        json!({
            "version": "2.0.0",
            "initialized": self.initialized,
            "started": self.started,
            "market_env": format!("{:?}", self.market_env),
            "total_value": self.total_value.to_string(),
            "portfolio_high": self.portfolio_high.to_string(),
            "rebalance_count": self.rebalance_count,
            "total_pnl": self.total_pnl.to_string(),
            "holdings": holdings,
            "context_connected": self.context.is_some(),
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into Us3xLeverage v2.0 strategy");
    }
}

// ============================================================================
// 테스트
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_us_3x_leverage_initialization() {
        let mut strategy = Us3xLeverageStrategy::new();

        let config = json!({
            "allocations": [
                { "ticker": "TQQQ", "target_ratio": 0.35, "etf_type": "leverage" },
                { "ticker": "SQQQ", "target_ratio": 0.15, "etf_type": "inverse" }
            ],
            "rebalance_period_days": 30,
            "min_global_score": 55.0
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
        assert_eq!(strategy.etf_data.len(), 2);
    }

    #[test]
    fn test_default_allocations() {
        let allocs = default_allocations();
        assert_eq!(allocs.len(), 4);

        let total_ratio: f64 = allocs.iter().map(|a| a.target_ratio).sum();
        assert!((total_ratio - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_default_config() {
        let config = Us3xLeverageConfig::default();
        assert_eq!(config.min_global_score, 55.0);
        assert!(config.use_route_filter);
        assert!(config.use_regime_allocation);
        assert!(config.use_macro_risk);
        assert!(!config.cash_out_on_crisis);
    }

    #[test]
    fn test_market_environment_variants() {
        assert_ne!(MarketEnvironment::Bullish, MarketEnvironment::Bearish);
        assert_ne!(MarketEnvironment::Neutral, MarketEnvironment::Crisis);
    }
}

// 전략 레지스트리에 자동 등록
register_strategy! {
    id: "us_3x_leverage",
    aliases: ["us_leverage", "leverage_3x"],
    name: "미국 3배 레버리지",
    description: "미국 3배 레버리지 ETF 전략 v2.0. StrategyContext 연동.",
    timeframe: "1d",
    tickers: ["TQQQ", "SQQQ", "SOXL", "SOXS"],
    category: Daily,
    markets: [Stock],
    type: Us3xLeverageStrategy,
    config: Us3xLeverageConfig
}
