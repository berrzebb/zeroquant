//! Compound Momentum 전략 구현.
//!
//! TQQQ/SCHD/PFIX/TMF 기반 모멘텀 자산배분 전략.
//!
//! # 전략 개요
//!
//! - **공격 자산**: TQQQ (나스닥 3배 레버리지) - 50%
//! - **배당 자산**: SCHD (배당 성장 ETF) - 20%
//! - **금리 헤지**: PFIX (금리 상승 헤지) - 15%
//! - **채권 레버리지**: TMF (장기채 3배) - 15%
//!
//! # 모멘텀 필터
//!
//! 각 자산에 대해 MA130 기반 모멘텀 필터 적용:
//! 1. 전일 종가 < MA130 → 비중 50% 감소
//! 2. MA130 하락 추세 → 비중 추가 50% 감소
//! 3. 두 조건 모두 충족 시 PFIX/TMF는 완전 청산
//!
//! # 대체 로직
//!
//! PFIX/TMF 중 하나만 청산되면 다른 자산에 2배 배분
//!
//! # 리밸런싱
//!
//! 월간 리밸런싱 (매월 초)

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use trader_strategy_macro::StrategyConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use trader_core::domain::{RouteState, StrategyContext};
use trader_core::types::Timeframe;

use crate::strategies::common::rebalance::{
    PortfolioPosition, RebalanceCalculator, RebalanceConfig, RebalanceOrderSide, TargetAllocation,
};
use crate::strategies::common::{adjust_strength_by_score, ExitConfig};
use crate::traits::Strategy;
use trader_core::{MarketData, Order, Position, Side, Signal, SignalType};

/// Simple Power 전략 설정.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "compound_momentum",
    name = "Simple Power",
    description = "TQQQ/SCHD/PFIX/TMF 기반 모멘텀 자산배분 전략",
    category = "Monthly"
)]
pub struct CompoundMomentumConfig {
    /// 시장 타입 (US/KR)
    #[serde(default)]
    #[schema(label = "시장 타입", field_type = "select", options = ["US", "KR"], default = "US", section = "asset")]
    pub market: MarketType,

    /// 공격 자산 (기본: TQQQ)
    #[serde(default = "default_aggressive_asset")]
    #[schema(label = "공격 자산", field_type = "symbol", default = "TQQQ", section = "asset")]
    pub aggressive_asset: String,
    /// 공격 자산 기본 비중
    #[serde(default = "default_aggressive_weight")]
    #[schema(label = "공격 자산 비중", min = 0, max = 1, default = 0.5, section = "sizing")]
    pub aggressive_weight: Decimal,

    /// 배당 자산 (기본: SCHD)
    #[serde(default = "default_dividend_asset")]
    #[schema(label = "배당 자산", field_type = "symbol", default = "SCHD", section = "asset")]
    pub dividend_asset: String,
    /// 배당 자산 비중
    #[serde(default = "default_dividend_weight")]
    #[schema(label = "배당 자산 비중", min = 0, max = 1, default = 0.2, section = "sizing")]
    pub dividend_weight: Decimal,

    /// 금리 헤지 자산 (기본: PFIX)
    #[serde(default = "default_rate_hedge_asset")]
    #[schema(label = "금리 헤지 자산", field_type = "symbol", default = "PFIX", section = "asset")]
    pub rate_hedge_asset: String,
    /// 금리 헤지 비중
    #[serde(default = "default_rate_hedge_weight")]
    #[schema(label = "금리 헤지 비중", min = 0, max = 1, default = 0.15, section = "sizing")]
    pub rate_hedge_weight: Decimal,

    /// 채권 레버리지 자산 (기본: TMF)
    #[serde(default = "default_bond_leverage_asset")]
    #[schema(label = "채권 레버리지 자산", field_type = "symbol", default = "TMF", section = "asset")]
    pub bond_leverage_asset: String,
    /// 채권 레버리지 비중
    #[serde(default = "default_bond_leverage_weight")]
    #[schema(label = "채권 레버리지 비중", min = 0, max = 1, default = 0.15, section = "sizing")]
    pub bond_leverage_weight: Decimal,

    /// MA 기간 (기본: 130일)
    #[serde(default = "default_ma_period")]
    #[schema(label = "MA 기간", min = 20, max = 300, default = 130, section = "indicator")]
    pub ma_period: usize,

    /// 리밸런싱 주기 (월 단위)
    #[serde(default = "default_rebalance_interval")]
    #[schema(label = "리밸런싱 주기 (월)", min = 1, max = 12, default = 1, section = "timing")]
    pub rebalance_interval_months: u32,

    /// 투자 비율 (총 자산 대비)
    #[serde(default = "default_invest_rate")]
    #[schema(label = "투자 비율", min = 0, max = 1, default = 1.0, section = "sizing")]
    pub invest_rate: Decimal,

    /// 리밸런싱 임계값 (비중 편차)
    #[serde(default = "default_rebalance_threshold")]
    #[schema(label = "리밸런싱 임계값", min = 0.01, max = 0.2, default = 0.03, section = "timing")]
    pub rebalance_threshold: Decimal,

    /// 최소 GlobalScore (기본값: 60)
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", min = 0, max = 100, default = 60, section = "filter")]
    pub min_global_score: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

// 기본값 함수들
fn default_aggressive_asset() -> String {
    "TQQQ".to_string()
}
fn default_aggressive_weight() -> Decimal {
    dec!(0.5)
}
fn default_dividend_asset() -> String {
    "SCHD".to_string()
}
fn default_dividend_weight() -> Decimal {
    dec!(0.2)
}
fn default_rate_hedge_asset() -> String {
    "PFIX".to_string()
}
fn default_rate_hedge_weight() -> Decimal {
    dec!(0.15)
}
fn default_bond_leverage_asset() -> String {
    "TMF".to_string()
}
fn default_bond_leverage_weight() -> Decimal {
    dec!(0.15)
}
fn default_ma_period() -> usize {
    130
}
fn default_rebalance_interval() -> u32 {
    1
}
fn default_invest_rate() -> Decimal {
    dec!(1.0)
}
fn default_rebalance_threshold() -> Decimal {
    dec!(0.03)
}

/// 기본 최소 GlobalScore.
fn default_min_global_score() -> Decimal {
    dec!(60)
}

/// 시장 타입.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MarketType {
    /// 미국 시장
    #[default]
    US,
    /// 한국 시장
    KR,
}

impl Default for CompoundMomentumConfig {
    fn default() -> Self {
        Self::us_default()
    }
}

impl CompoundMomentumConfig {
    /// 미국 시장 기본 설정 (V2).
    pub fn us_default() -> Self {
        Self {
            market: MarketType::US,
            aggressive_asset: "TQQQ".to_string(),
            aggressive_weight: dec!(0.5),
            dividend_asset: "SCHD".to_string(),
            dividend_weight: dec!(0.2),
            rate_hedge_asset: "PFIX".to_string(),
            rate_hedge_weight: dec!(0.15),
            bond_leverage_asset: "TMF".to_string(),
            bond_leverage_weight: dec!(0.15),
            ma_period: 130,
            rebalance_interval_months: 1,
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(0.03),
            min_global_score: dec!(60),
            exit_config: ExitConfig::for_momentum(),
        }
    }

    /// 한국 시장 설정 (ETF 기반).
    ///
    /// 한국에서 유사한 ETF로 대체:
    /// - TQQQ 대체: KODEX 미국나스닥100레버리지 (409820)
    /// - SCHD 대체: KODEX 미국배당프리미엄액티브 (441640) 또는 TIGER 미국S&P500배당귀족
    /// - PFIX 대체: 단기채권 ETF
    /// - TMF 대체: KODEX 미국채울트라30년선물(H) (304660)
    pub fn kr_default() -> Self {
        Self {
            market: MarketType::KR,
            aggressive_asset: "409820".to_string(), // KODEX 미국나스닥100레버리지
            aggressive_weight: dec!(0.5),
            dividend_asset: "441640".to_string(), // KODEX 미국배당프리미엄액티브
            dividend_weight: dec!(0.2),
            rate_hedge_asset: "453850".to_string(), // KODEX CD금리액티브(합성)
            rate_hedge_weight: dec!(0.15),
            bond_leverage_asset: "304660".to_string(), // KODEX 미국채울트라30년선물(H)
            bond_leverage_weight: dec!(0.15),
            ma_period: 130,
            rebalance_interval_months: 1,
            invest_rate: dec!(1.0),
            rebalance_threshold: dec!(0.03),
            min_global_score: dec!(60),
            exit_config: ExitConfig::for_momentum(),
        }
    }

    /// 모든 자산 티커 가져오기.
    pub fn all_assets(&self) -> Vec<String> {
        vec![
            self.aggressive_asset.clone(),
            self.dividend_asset.clone(),
            self.rate_hedge_asset.clone(),
            self.bond_leverage_asset.clone(),
        ]
    }

    /// 기본 비중 맵 가져오기.
    pub fn base_weights(&self) -> HashMap<String, Decimal> {
        let mut weights = HashMap::new();
        weights.insert(self.aggressive_asset.clone(), self.aggressive_weight);
        weights.insert(self.dividend_asset.clone(), self.dividend_weight);
        weights.insert(self.rate_hedge_asset.clone(), self.rate_hedge_weight);
        weights.insert(self.bond_leverage_asset.clone(), self.bond_leverage_weight);
        weights
    }
}

/// 자산별 모멘텀 상태.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct AssetMomentumState {
    /// 현재 MA130
    ma_current: Option<Decimal>,
    /// 전일 MA130
    ma_previous: Option<Decimal>,
    /// 전일 종가
    prev_close: Option<Decimal>,
    /// 모멘텀 컷 카운트 (0, 1, 2)
    cut_count: u8,
    /// 조정된 비중 배율 (1.0, 0.5, 0.25, 0.0)
    rate_multiplier: Decimal,
    /// 아웃 상태 (완전 청산)
    is_out: bool,
}

impl Default for AssetMomentumState {
    fn default() -> Self {
        Self {
            ma_current: None,
            ma_previous: None,
            prev_close: None,
            cut_count: 0,
            rate_multiplier: dec!(1.0), // 기본값은 100% 투자
            is_out: false,
        }
    }
}

/// Simple Power 전략.
pub struct CompoundMomentumStrategy {
    config: Option<CompoundMomentumConfig>,
    /// StrategyContext (RouteState, GlobalScore, Klines 조회용)
    context: Option<Arc<RwLock<StrategyContext>>>,
    /// 자산별 모멘텀 상태
    momentum_states: HashMap<String, AssetMomentumState>,
    /// 현재 포지션
    positions: HashMap<String, Decimal>,
    /// 마지막 리밸런싱 년월 (YYYY_MM)
    last_rebalance_ym: Option<String>,
    /// 리밸런싱 계산기
    rebalance_calculator: RebalanceCalculator,
    /// 현재 현금 잔고
    cash_balance: Decimal,
}

impl CompoundMomentumStrategy {
    /// 새 전략 생성.
    pub fn new() -> Self {
        Self {
            config: None,
            context: None,
            momentum_states: HashMap::new(),
            positions: HashMap::new(),
            last_rebalance_ym: None,
            rebalance_calculator: RebalanceCalculator::new(RebalanceConfig::us_market()),
            cash_balance: Decimal::ZERO,
        }
    }

    /// 설정으로 전략 생성.
    pub fn with_config(config: CompoundMomentumConfig) -> Self {
        let rebalance_config = match config.market {
            MarketType::US => RebalanceConfig::us_market(),
            MarketType::KR => RebalanceConfig::korean_market(),
        };

        Self {
            config: Some(config),
            context: None,
            momentum_states: HashMap::new(),
            positions: HashMap::new(),
            last_rebalance_ym: None,
            rebalance_calculator: RebalanceCalculator::new(rebalance_config),
            cash_balance: Decimal::ZERO,
        }
    }

    // ========================================================================
    // StrategyContext 헬퍼
    // ========================================================================

    /// StrategyContext에서 가격 히스토리 가져오기 (최신 가격이 앞에)
    fn get_price_history(&self, ticker: &str) -> Vec<Decimal> {
        let ctx = match self.context.as_ref() {
            Some(c) => c,
            None => return vec![],
        };
        let ctx_lock = match ctx.try_read() {
            Ok(l) => l,
            Err(_) => return vec![],
        };
        let klines = ctx_lock.get_klines(ticker, Timeframe::D1);
        // 최신 가격이 앞에 오도록 역순으로 변환
        klines.iter().rev().map(|k| k.close).collect()
    }

    /// 충분한 데이터가 있는지 확인
    fn has_sufficient_data(&self) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return false,
        };
        // 적어도 공격 자산의 MA 기간 + 3일 이상 필요
        let prices = self.get_price_history(&config.aggressive_asset);
        prices.len() >= config.ma_period + 3
    }

    /// 이동평균 계산 (최신 가격이 앞에 있는 배열 기준).
    fn calculate_ma(&self, prices: &[Decimal], period: usize, offset: usize) -> Option<Decimal> {
        if prices.len() < period + offset {
            return None;
        }

        let start = offset;
        let end = start + period;
        if end > prices.len() {
            return None;
        }

        let sum: Decimal = prices[start..end].iter().sum();
        Some(sum / Decimal::from(period))
    }

    /// 모멘텀 상태 계산 (StrategyContext 기반).
    fn calculate_momentum_state(
        &self,
        ticker: &str,
        config: &CompoundMomentumConfig,
    ) -> AssetMomentumState {
        let prices = self.get_price_history(ticker);
        if prices.len() < config.ma_period + 3 {
            return AssetMomentumState::default();
        }

        // 전일 종가 (index 1, 오늘은 index 0)
        let prev_close = prices.get(1).copied();

        // 현재 MA130 (전일 기준, index 1에서 시작)
        let ma_current = self.calculate_ma(&prices, config.ma_period, 1);

        // 전일 MA130 (2일 전 기준, index 2에서 시작)
        let ma_previous = self.calculate_ma(&prices, config.ma_period, 2);

        let mut cut_count: u8 = 0;
        let mut rate = dec!(1.0);

        if let (Some(ma), Some(close)) = (ma_current, prev_close) {
            // 조건 1: 전일 종가 < MA130
            if ma > close {
                rate *= dec!(0.5);
                cut_count += 1;
            }
        }

        if let (Some(ma_curr), Some(ma_prev)) = (ma_current, ma_previous) {
            // 조건 2: MA130 하락 추세
            if ma_prev > ma_curr {
                rate *= dec!(0.5);
                cut_count += 1;
            }
        }

        // PFIX/TMF는 두 조건 모두 충족 시 완전 청산
        let is_hedge_asset =
            ticker == config.rate_hedge_asset || ticker == config.bond_leverage_asset;
        let is_out = is_hedge_asset && cut_count == 2;

        if is_out {
            rate = Decimal::ZERO;
        }

        AssetMomentumState {
            ma_current,
            ma_previous,
            prev_close,
            cut_count,
            rate_multiplier: rate,
            is_out,
        }
    }

    /// 조정된 목표 비중 계산.
    fn calculate_adjusted_weights(
        &mut self,
        config: &CompoundMomentumConfig,
    ) -> Vec<TargetAllocation> {
        // 각 자산의 모멘텀 상태 계산
        for asset in config.all_assets() {
            let state = self.calculate_momentum_state(&asset, config);
            self.momentum_states.insert(asset.clone(), state);
        }

        let base_weights = config.base_weights();
        let mut adjusted_weights: HashMap<String, Decimal> = HashMap::new();

        // 기본 비중에 모멘텀 필터 적용
        for (asset, base_weight) in &base_weights {
            let state = self.momentum_states.get(asset).cloned().unwrap_or_default();
            adjusted_weights.insert(asset.clone(), *base_weight * state.rate_multiplier);
        }

        // PFIX/TMF 대체 로직
        let pfix_state = self
            .momentum_states
            .get(&config.rate_hedge_asset)
            .cloned()
            .unwrap_or_default();
        let tmf_state = self
            .momentum_states
            .get(&config.bond_leverage_asset)
            .cloned()
            .unwrap_or_default();

        if pfix_state.is_out && !tmf_state.is_out {
            // PFIX 아웃 → TMF에 2배 배분
            if let Some(weight) = adjusted_weights.get_mut(&config.bond_leverage_asset) {
                *weight *= dec!(2.0);
                info!(
                    "PFIX 청산 → {} 비중 2배: {:.1}%",
                    config.bond_leverage_asset,
                    (*weight * dec!(100))
                );
            }
        } else if tmf_state.is_out && !pfix_state.is_out {
            // TMF 아웃 → PFIX에 2배 배분
            if let Some(weight) = adjusted_weights.get_mut(&config.rate_hedge_asset) {
                *weight *= dec!(2.0);
                info!(
                    "TMF 청산 → {} 비중 2배: {:.1}%",
                    config.rate_hedge_asset,
                    (*weight * dec!(100))
                );
            }
        }

        // 로그 출력
        for (asset, weight) in &adjusted_weights {
            let state = self.momentum_states.get(asset).cloned().unwrap_or_default();
            info!(
                "{} → 투자 비중: {:.1}% (cut_count: {}, rate: {:.2})",
                asset,
                (*weight * dec!(100)),
                state.cut_count,
                state.rate_multiplier
            );
        }

        // TargetAllocation으로 변환
        adjusted_weights
            .into_iter()
            .map(|(ticker, weight)| TargetAllocation::new(ticker, weight))
            .collect()
    }

    /// RouteState 기반 진입 조건 체크.
    /// 미국 ETF 전략이므로 GlobalScore 체크 불필요 (자체 모멘텀 계산 사용).
    fn can_enter(&self, ticker: &str) -> bool {
        let Some(_config) = self.config.as_ref() else {
            return false;
        };

        let Some(ctx) = self.context.as_ref() else {
            // Context가 없으면 진입 허용 (하위 호환성)
            debug!("StrategyContext not available - allowing entry by default");
            return true;
        };

        let Ok(ctx_lock) = ctx.try_read() else {
            warn!("Failed to acquire context lock - entry blocked");
            return false;
        };

        // RouteState 체크 - Overheat 시만 진입 제한
        if let Some(route_state) = ctx_lock.get_route_state(ticker) {
            match route_state {
                RouteState::Overheat => {
                    debug!(
                        ticker = %ticker,
                        route_state = ?route_state,
                        "시장 과열 - 진입 제한"
                    );
                    return false;
                }
                // 나머지 상태는 진입 허용
                _ => {}
            }
        }

        true
    }

    /// GlobalScore 기반 동적 강도 계산.
    fn get_adjusted_strength(&self, ticker: &str, base_strength: f64) -> f64 {
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

    /// 리밸런싱 필요 여부 확인.
    fn should_rebalance(&self, current_time: DateTime<Utc>) -> bool {
        let current_ym = format!("{}_{}", current_time.year(), current_time.month());

        match &self.last_rebalance_ym {
            None => true,                            // 첫 리밸런싱
            Some(last_ym) => last_ym != &current_ym, // 달이 바뀌었으면 리밸런싱
        }
    }

    /// 리밸런싱 신호 생성.
    fn generate_rebalance_signals(
        &mut self,
        config: &CompoundMomentumConfig,
        current_time: DateTime<Utc>,
    ) -> Vec<Signal> {
        if !self.should_rebalance(current_time) {
            return Vec::new();
        }

        // 조정된 목표 비중 계산
        let target_allocations = self.calculate_adjusted_weights(config);

        // 현재 포지션을 PortfolioPosition으로 변환
        let mut portfolio_positions: Vec<PortfolioPosition> = Vec::new();

        for (ticker, quantity) in &self.positions {
            let prices = self.get_price_history(ticker);
            if let Some(current_price) = prices.first() {
                portfolio_positions.push(PortfolioPosition::new(
                    ticker,
                    *quantity,
                    *current_price,
                ));
            }
        }

        // 현금 포지션 추가
        let cash_ticker = match config.market {
            MarketType::US => "USD",
            MarketType::KR => "KRW",
        };
        portfolio_positions.push(PortfolioPosition::cash(self.cash_balance, cash_ticker));

        // 리밸런싱 계산
        let result = self
            .rebalance_calculator
            .calculate_orders_with_cash_constraint(&portfolio_positions, &target_allocations);

        // 신호 변환
        let mut signals = Vec::new();

        for order in result.orders {
            let side = match order.side {
                RebalanceOrderSide::Buy => Side::Buy,
                RebalanceOrderSide::Sell => Side::Sell,
            };

            // BUY 신호의 경우 can_enter() 체크
            if side == Side::Buy && !self.can_enter(&order.ticker) {
                debug!(
                    ticker = %order.ticker,
                    "Skipping BUY signal due to RouteState/GlobalScore filter"
                );
                continue;
            }

            // USD를 quote로 사용 (미국 시장)
            let quote_currency = match config.market {
                MarketType::US => "USD",
                MarketType::KR => "KRW",
            };

            // GlobalScore 기반 동적 강도 적용 (BUY 신호에만)
            let base_strength = if side == Side::Buy { 0.7 } else { 0.9 };
            let strength = self.get_adjusted_strength(&order.ticker, base_strength);

            // Signal 빌더 패턴으로 생성
            let signal = Signal::new(
                self.name(),
                format!("{}/{}", order.ticker, quote_currency),
                side,
                SignalType::Scale, // 리밸런싱은 Scale 타입 사용
            )
            .with_strength(strength)
            .with_metadata("current_weight", json!(order.current_weight.to_string()))
            .with_metadata("target_weight", json!(order.target_weight.to_string()))
            .with_metadata("amount", json!(order.amount.to_string()))
            .with_metadata("quantity", json!(order.quantity.to_string()))
            .with_metadata("reason", json!("monthly_rebalance"));

            signals.push(signal);
        }

        // 리밸런싱 시간 기록
        if !signals.is_empty() {
            self.last_rebalance_ym =
                Some(format!("{}_{}", current_time.year(), current_time.month()));
            info!("Simple Power 리밸런싱 완료: {} 주문 생성", signals.len());
        }

        signals
    }
}

impl Default for CompoundMomentumStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for CompoundMomentumStrategy {
    fn name(&self) -> &str {
        "Simple Power"
    }

    fn version(&self) -> &str {
        "2.0.0"
    }

    fn description(&self) -> &str {
        "TQQQ/SCHD/PFIX/TMF 기반 모멘텀 자산배분 전략. MA130 필터로 비중 조정, 월간 리밸런싱."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let parsed_config: CompoundMomentumConfig = serde_json::from_value(config.clone())?;

        // 시장에 맞는 리밸런싱 설정
        let rebalance_config = match parsed_config.market {
            MarketType::US => RebalanceConfig::us_market(),
            MarketType::KR => RebalanceConfig::korean_market(),
        };
        self.rebalance_calculator = RebalanceCalculator::new(rebalance_config);

        // initial_capital 또는 amount가 있으면 cash_balance로 설정
        let capital_value = config.get("initial_capital").or_else(|| config.get("amount"));
        if let Some(capital_str) = capital_value {
            let capital_opt = capital_str.as_str()
                .and_then(|s| s.parse::<Decimal>().ok())
                .or_else(|| capital_str.as_i64().map(Decimal::from));
            
            if let Some(capital_dec) = capital_opt {
                self.cash_balance = capital_dec;
                info!("[Simple Power] 초기 자본금 설정: {}", capital_dec);
            }
        }

        info!(
            "[Simple Power] 전략 초기화 - 시장: {:?}, 자산: {:?}, 초기자본: {}",
            parsed_config.market,
            parsed_config.all_assets(),
            self.cash_balance
        );

        self.config = Some(parsed_config);
        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        let config = match &self.config {
            Some(c) => c.clone(),
            None => return Ok(Vec::new()),
        };

        let ticker = data.ticker.clone();

        // 관심 자산이 아니면 무시
        if !config.all_assets().contains(&ticker) {
            return Ok(Vec::new());
        }

        // StrategyContext에 충분한 데이터가 있는지 확인
        if !self.has_sufficient_data() {
            return Ok(Vec::new());
        }

        // 리밸런싱 신호 생성
        let signals = self.generate_rebalance_signals(&config, data.timestamp);

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "[Simple Power] 주문 체결: {:?} {} {} @ {:?}",
            order.side, order.quantity, order.ticker, order.average_fill_price
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ticker = position.ticker.clone();
        self.positions.insert(ticker.clone(), position.quantity);
        info!(
            "[Simple Power] 포지션 업데이트: {} = {} (PnL: {})",
            ticker, position.quantity, position.unrealized_pnl
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("[Simple Power] 전략 종료");
        Ok(())
    }

    fn get_state(&self) -> Value {
        let momentum_info: HashMap<String, Value> = self
            .momentum_states
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    json!({
                        "cut_count": v.cut_count,
                        "rate_multiplier": v.rate_multiplier.to_string(),
                        "is_out": v.is_out,
                        "ma_current": v.ma_current.map(|d| d.to_string()),
                        "prev_close": v.prev_close.map(|d| d.to_string()),
                    }),
                )
            })
            .collect();

        json!({
            "name": self.name(),
            "version": self.version(),
            "last_rebalance_ym": self.last_rebalance_ym,
            "momentum_states": momentum_info,
            "positions": self.positions.iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect::<HashMap<_, _>>(),
            "cash_balance": self.cash_balance.to_string(),
        })
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into Simple Power strategy");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_us_default() {
        let config = CompoundMomentumConfig::us_default();
        assert_eq!(config.market, MarketType::US);
        assert_eq!(config.aggressive_asset, "TQQQ");
        assert_eq!(config.dividend_asset, "SCHD");
        assert_eq!(config.rate_hedge_asset, "PFIX");
        assert_eq!(config.bond_leverage_asset, "TMF");
        assert_eq!(config.aggressive_weight, dec!(0.5));
        assert_eq!(config.ma_period, 130);
    }

    #[test]
    fn test_config_kr_default() {
        let config = CompoundMomentumConfig::kr_default();
        assert_eq!(config.market, MarketType::KR);
        assert_eq!(config.aggressive_asset, "409820");
        assert_eq!(config.ma_period, 130);
    }

    #[test]
    fn test_config_all_assets() {
        let config = CompoundMomentumConfig::us_default();
        let assets = config.all_assets();
        assert_eq!(assets.len(), 4);
        assert!(assets.contains(&"TQQQ".to_string()));
        assert!(assets.contains(&"SCHD".to_string()));
        assert!(assets.contains(&"PFIX".to_string()));
        assert!(assets.contains(&"TMF".to_string()));
    }

    #[test]
    fn test_base_weights_sum() {
        let config = CompoundMomentumConfig::us_default();
        let weights = config.base_weights();
        let sum: Decimal = weights.values().sum();
        assert_eq!(sum, dec!(1.0));
    }

    #[test]
    fn test_strategy_creation() {
        let strategy = CompoundMomentumStrategy::new();
        assert_eq!(strategy.name(), "Simple Power");
        assert_eq!(strategy.version(), "2.0.0");
    }

    #[test]
    fn test_calculate_ma() {
        let strategy = CompoundMomentumStrategy::new();
        let prices: Vec<Decimal> = (0..150).map(|i| dec!(100) + Decimal::from(i)).collect();

        // MA5 at offset 0
        let ma = strategy.calculate_ma(&prices, 5, 0);
        assert!(ma.is_some());
        // MA of [100, 101, 102, 103, 104] = 102
        assert_eq!(ma.unwrap(), dec!(102));
    }

    #[test]
    fn test_should_rebalance_first_time() {
        let strategy = CompoundMomentumStrategy::new();
        let now = Utc::now();
        assert!(strategy.should_rebalance(now));
    }

    #[test]
    fn test_should_rebalance_same_month() {
        let mut strategy = CompoundMomentumStrategy::new();
        let now = Utc::now();
        strategy.last_rebalance_ym = Some(format!("{}_{}", now.year(), now.month()));
        assert!(!strategy.should_rebalance(now));
    }

    #[test]
    fn test_momentum_state_no_context() {
        // StrategyContext 없이 모멘텀 상태 계산 - 기본값 반환
        let strategy = CompoundMomentumStrategy::new();
        let config = CompoundMomentumConfig::us_default();
        let state = strategy.calculate_momentum_state("TQQQ", &config);

        // Context 없으면 기본값
        assert_eq!(state.cut_count, 0);
        assert_eq!(state.rate_multiplier, dec!(1.0));
        assert!(!state.is_out);
    }

    #[test]
    fn test_has_sufficient_data_without_context() {
        // StrategyContext 없이는 데이터 부족
        let strategy = CompoundMomentumStrategy::new();
        assert!(!strategy.has_sufficient_data());
    }

    #[test]
    fn test_get_state() {
        let strategy = CompoundMomentumStrategy::new();
        let state = strategy.get_state();

        assert_eq!(state["name"], "Simple Power");
        assert_eq!(state["version"], "2.0.0");
    }
}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "compound_momentum",
    aliases: [],
    name: "Compound Momentum",
    description: "복합 모멘텀 자산배분 전략입니다.",
    timeframe: "1d",
    tickers: ["TQQQ", "SCHD", "PFIX", "TMF"],
    category: Monthly,
    markets: [Stock],
    type: CompoundMomentumStrategy,
    config: CompoundMomentumConfig
}
