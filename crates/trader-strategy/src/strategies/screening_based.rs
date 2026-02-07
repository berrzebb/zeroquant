//! 스크리닝 기반 전략 (Screening-Based Strategy)
//!
//! 동적 유니버스 전략의 기반 그룹 전략입니다.
//! 스크리닝 결과에서 종목을 동적으로 선택하고 리밸런싱합니다.
//!
//! # 핵심 차별점
//!
//! - **티커 목록 없음**: 고정된 종목 대신 스크리닝 결과에서 동적 선택
//! - **스크리닝 프리셋**: GlobalScore, RouteState 기반 필터링
//! - **자동 리밸런싱**: 주기적으로 상위 종목 재선정
//!
//! # 지원 변형
//!
//! - **SmallCapQuant**: 소형주 퀀트 (재무 필터 + GlobalScore)
//! - **PensionBot**: 연금 자동화 (모멘텀 + 자산 배분)
//! - **DynamicUniverse**: 일반 동적 유니버스 (스크리닝만 활용)
//!
//! # 예시
//!
//! ```rust,ignore
//! // 소형주 퀀트 전략
//! let config = ScreeningBasedConfig::small_cap_quant_default();
//! let mut strategy = ScreeningBasedStrategy::new();
//! strategy.initialize(serde_json::to_value(config)?).await?;
//!
//! // 연금 자동화 전략
//! let config = ScreeningBasedConfig::pension_bot_default();
//! let mut strategy = ScreeningBasedStrategy::new();
//! strategy.initialize(serde_json::to_value(config)?).await?;
//! ```

use crate::strategies::common::rebalance::{
    PortfolioPosition, RebalanceCalculator, RebalanceConfig, RebalanceOrderSide, TargetAllocation,
};
use crate::strategies::common::{adjust_strength_by_score, ExitConfig};
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use trader_core::domain::{RouteState, ScreeningResult, StrategyContext};
use trader_core::{MarketData, Order, Position, Side, Signal, SignalType, Timeframe};
use trader_strategy_macro::StrategyConfig;

// ============================================================================
// 전략 변형 (Strategy Variant)
// ============================================================================

/// 스크리닝 기반 전략 변형.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ScreeningVariant {
    /// 소형주 퀀트 - 재무 필터 + GlobalScore 기반
    #[default]
    SmallCapQuant,
    /// 연금 자동화 - 모멘텀 + 자산 배분 기반
    PensionBot,
    /// 일반 동적 유니버스 - 스크리닝 결과만 활용
    DynamicUniverse,
}

// ============================================================================
// 시장 타입 (Market Type)
// ============================================================================

/// 시장 타입.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MarketType {
    /// 한국 시장
    #[default]
    KR,
    /// 미국 시장
    US,
}

impl MarketType {
    /// Quote 통화 반환.
    pub fn quote_currency(&self) -> &str {
        match self {
            MarketType::KR => "KRW",
            MarketType::US => "USD",
        }
    }

    /// RebalanceConfig 반환.
    pub fn rebalance_config(&self) -> RebalanceConfig {
        match self {
            MarketType::US => RebalanceConfig::us_market(),
            MarketType::KR => RebalanceConfig::korean_market(),
        }
    }
}

// ============================================================================
// 리밸런싱 빈도 (Rebalance Frequency)
// ============================================================================

/// 리밸런싱 빈도.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum RebalanceFrequency {
    /// 월간 (매월 초)
    #[default]
    Monthly,
    /// 주간 (매주 월요일)
    Weekly,
    /// 일수 기반
    Days(u32),
}

// ============================================================================
// 비중 배분 방식 (Weighting Method)
// ============================================================================

/// 비중 배분 방식.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WeightingMethod {
    /// 동일 비중
    #[default]
    Equal,
    /// GlobalScore 비례 비중
    ScoreProportional,
}

// ============================================================================
// 스크리닝 기반 설정 (Screening-Based Config)
// ============================================================================

/// 스크리닝 기반 전략 설정.
///
/// **핵심**: 티커 목록 대신 스크리닝 프리셋 사용
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "screening_based",
    name = "스크리닝 기반 전략",
    description = "동적 유니버스 기반 투자 전략 (소형주 퀀트, 연금 자동화 등)",
    category = "Monthly"
)]
pub struct ScreeningBasedConfig {
    /// 전략 변형
    #[serde(default)]
    #[schema(label = "전략 변형")]
    pub variant: ScreeningVariant,

    /// 시장 타입
    #[serde(default)]
    #[schema(label = "시장 타입")]
    pub market: MarketType,

    /// 스크리닝 프리셋 이름 (StrategyContext.screening_results 키)
    #[serde(default = "default_preset_name")]
    #[schema(label = "스크리닝 프리셋")]
    pub preset_name: String,

    /// 최소 GlobalScore (필터링 기준)
    #[serde(default = "default_min_score")]
    #[schema(label = "최소 점수", default = "60")]
    pub min_score: Decimal,

    /// 선택할 상위 종목 수
    #[serde(default = "default_top_n")]
    #[schema(label = "상위 종목 수", default = "10")]
    pub top_n: usize,

    /// 비중 배분 방식
    #[serde(default)]
    #[schema(label = "비중 배분")]
    pub weighting_method: WeightingMethod,

    /// 리밸런싱 빈도
    #[serde(default)]
    #[schema(label = "리밸런싱 주기")]
    pub rebalance_frequency: RebalanceFrequency,

    /// 리밸런싱 임계값 (비중 차이가 이 값 이상일 때만 리밸런싱)
    #[serde(default = "default_rebalance_threshold")]
    #[schema(label = "리밸런싱 임계값", default = "0.05")]
    pub rebalance_threshold: Decimal,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    #[schema(label = "투자 금액", default = "10000000")]
    pub total_amount: Decimal,

    /// RouteState 필터 사용 여부
    #[serde(default = "default_use_route_filter")]
    #[schema(label = "RouteState 필터")]
    pub use_route_filter: bool,

    /// 청산 설정
    #[serde(default)]
    #[schema(label = "청산 설정", skip)]
    pub exit_config: ExitConfig,
}

fn default_preset_name() -> String {
    "screening_based".to_string()
}

fn default_min_score() -> Decimal {
    dec!(60)
}

fn default_top_n() -> usize {
    10
}

fn default_rebalance_threshold() -> Decimal {
    dec!(0.05)
}

fn default_total_amount() -> Decimal {
    dec!(10_000_000)
}

fn default_use_route_filter() -> bool {
    true
}

impl Default for ScreeningBasedConfig {
    fn default() -> Self {
        Self {
            variant: ScreeningVariant::default(),
            market: MarketType::default(),
            preset_name: default_preset_name(),
            min_score: default_min_score(),
            top_n: default_top_n(),
            weighting_method: WeightingMethod::default(),
            rebalance_frequency: RebalanceFrequency::default(),
            rebalance_threshold: default_rebalance_threshold(),
            total_amount: default_total_amount(),
            use_route_filter: default_use_route_filter(),
            exit_config: ExitConfig::default(),
        }
    }
}

impl ScreeningBasedConfig {
    /// 소형주 퀀트 기본 설정.
    pub fn small_cap_quant_default() -> Self {
        Self {
            variant: ScreeningVariant::SmallCapQuant,
            market: MarketType::KR,
            preset_name: "small_cap_quant".to_string(),
            min_score: dec!(60),
            top_n: 20,
            weighting_method: WeightingMethod::Equal,
            rebalance_frequency: RebalanceFrequency::Monthly,
            rebalance_threshold: dec!(0.05),
            total_amount: dec!(10_000_000),
            use_route_filter: false, // 소형주는 RouteState 필터 사용 안함
            exit_config: ExitConfig::default(),
        }
    }

    /// 연금 자동화 기본 설정.
    pub fn pension_bot_default() -> Self {
        Self {
            variant: ScreeningVariant::PensionBot,
            market: MarketType::KR,
            preset_name: "pension_bot".to_string(),
            min_score: dec!(70),
            top_n: 10,
            weighting_method: WeightingMethod::ScoreProportional,
            rebalance_frequency: RebalanceFrequency::Weekly,
            rebalance_threshold: dec!(0.03),
            total_amount: dec!(10_000_000),
            use_route_filter: true,
            exit_config: ExitConfig::default(),
        }
    }

    /// 동적 유니버스 기본 설정.
    pub fn dynamic_universe_default() -> Self {
        Self {
            variant: ScreeningVariant::DynamicUniverse,
            market: MarketType::KR,
            preset_name: "dynamic".to_string(),
            min_score: dec!(50),
            top_n: 15,
            weighting_method: WeightingMethod::Equal,
            rebalance_frequency: RebalanceFrequency::Monthly,
            rebalance_threshold: dec!(0.05),
            total_amount: dec!(10_000_000),
            use_route_filter: true,
            exit_config: ExitConfig::default(),
        }
    }
}

// ============================================================================
// 종목 상태 (Asset State)
// ============================================================================

/// 스크리닝 결과에서 선택된 종목의 상태.
#[derive(Debug, Clone)]
struct SelectedAsset {
    /// 티커
    ticker: String,
    /// GlobalScore
    score: Decimal,
    /// RouteState
    #[allow(dead_code)]
    route_state: RouteState,
    /// 현재 가격
    current_price: Decimal,
    /// 목표 비중
    target_weight: Decimal,
}

// ============================================================================
// 스크리닝 기반 전략 (Screening-Based Strategy)
// ============================================================================

/// 스크리닝 기반 전략.
///
/// 스크리닝 결과에서 동적으로 종목을 선택하고 리밸런싱합니다.
pub struct ScreeningBasedStrategy {
    /// 설정
    config: Option<ScreeningBasedConfig>,

    /// 현재 선택된 종목들
    selected_assets: Vec<SelectedAsset>,

    /// 현재 보유 종목 (ticker → 보유 수량)
    holdings: HashMap<String, Decimal>,

    /// 마지막 리밸런싱 시각
    last_rebalance_time: Option<DateTime<Utc>>,

    /// 마지막 리밸런싱 월
    last_rebalance_month: Option<u32>,

    /// 초기화 여부
    initialized: bool,

    /// 전략 컨텍스트
    context: Option<Arc<RwLock<StrategyContext>>>,

    /// 거래 횟수
    trades_count: u32,
}

impl ScreeningBasedStrategy {
    /// 새 전략 인스턴스 생성.
    pub fn new() -> Self {
        Self {
            config: None,
            selected_assets: Vec::new(),
            holdings: HashMap::new(),
            last_rebalance_time: None,
            last_rebalance_month: None,
            initialized: false,
            context: None,
            trades_count: 0,
        }
    }

    /// 스크리닝 결과에서 종목 선택.
    async fn select_assets_from_screening(&mut self) -> Vec<SelectedAsset> {
        let config = match &self.config {
            Some(c) => c,
            None => return Vec::new(),
        };

        let context = match &self.context {
            Some(c) => c,
            None => {
                warn!("StrategyContext가 설정되지 않았습니다");
                return Vec::new();
            }
        };

        let ctx_read = context.read().await;

        // 스크리닝 결과 조회
        let screening_results = match ctx_read.screening_results.get(&config.preset_name) {
            Some(results) => results,
            None => {
                debug!(preset = %config.preset_name, "스크리닝 결과 없음");
                return Vec::new();
            }
        };

        // 필터링 및 정렬
        let mut filtered: Vec<_> = screening_results
            .iter()
            .filter(|r| {
                // 최소 점수 필터
                if r.overall_score < config.min_score {
                    return false;
                }

                // RouteState 필터 (옵션)
                if config.use_route_filter {
                    if !matches!(r.route_state, RouteState::Attack | RouteState::Armed) {
                        return false;
                    }
                }

                true
            })
            .collect();

        // overall_score 기준 내림차순 정렬
        filtered.sort_by(|a, b| b.overall_score.cmp(&a.overall_score));

        // 상위 N개 선택
        let selected: Vec<_> = filtered.into_iter().take(config.top_n).collect();

        if selected.is_empty() {
            debug!("선택된 종목이 없습니다");
            return Vec::new();
        }

        // 비중 계산
        let weights = self.calculate_weights(&selected);

        // SelectedAsset 변환
        selected
            .into_iter()
            .zip(weights)
            .map(|(result, weight)| {
                // 현재 가격 조회 (klines_by_timeframe에서 마지막 가격)
                // 구조: ticker → (timeframe → klines)
                let current_price = ctx_read
                    .klines_by_timeframe
                    .get(&result.ticker)
                    .and_then(|tf_map| tf_map.get(&Timeframe::D1))
                    .and_then(|klines| klines.last())
                    .map(|k| k.close)
                    .unwrap_or(Decimal::ZERO);

                SelectedAsset {
                    ticker: result.ticker.clone(),
                    score: result.overall_score,
                    route_state: result.route_state.clone(),
                    current_price,
                    target_weight: weight,
                }
            })
            .collect()
    }

    /// 비중 계산.
    fn calculate_weights(&self, results: &[&ScreeningResult]) -> Vec<Decimal> {
        let config = match &self.config {
            Some(c) => c,
            None => return Vec::new(),
        };

        let count = results.len();
        if count == 0 {
            return Vec::new();
        }

        match config.weighting_method {
            WeightingMethod::Equal => {
                // 동일 비중
                let weight = Decimal::ONE / Decimal::from(count);
                vec![weight; count]
            }
            WeightingMethod::ScoreProportional => {
                // GlobalScore 비례 비중
                let total_score: Decimal = results.iter().map(|r| r.overall_score).sum();
                if total_score == Decimal::ZERO {
                    return vec![Decimal::ONE / Decimal::from(count); count];
                }
                results
                    .iter()
                    .map(|r| r.overall_score / total_score)
                    .collect()
            }
        }
    }

    /// 리밸런싱 필요 여부 확인.
    fn should_rebalance(&self, current_time: DateTime<Utc>) -> bool {
        let config = match &self.config {
            Some(c) => c,
            None => return false,
        };

        match &config.rebalance_frequency {
            RebalanceFrequency::Monthly => {
                let current_month = current_time.month();
                match self.last_rebalance_month {
                    Some(last_month) => current_month != last_month,
                    None => true, // 첫 리밸런싱
                }
            }
            RebalanceFrequency::Weekly => {
                // 월요일인지 확인
                current_time.weekday() == chrono::Weekday::Mon
                    && self.last_rebalance_time.map_or(true, |t| {
                        // 같은 주가 아닌지 확인
                        (current_time - t).num_days() >= 5
                    })
            }
            RebalanceFrequency::Days(days) => {
                self.last_rebalance_time.map_or(true, |t| {
                    (current_time - t).num_days() >= *days as i64
                })
            }
        }
    }

    /// 리밸런싱 신호 생성.
    fn generate_rebalance_signals(&mut self, current_time: DateTime<Utc>) -> Vec<Signal> {
        let config = match &self.config {
            Some(c) => c.clone(),
            None => return Vec::new(),
        };

        let mut signals = Vec::new();

        // 목표 포트폴리오 생성
        let mut target_allocations: Vec<TargetAllocation> = Vec::new();
        for asset in &self.selected_assets {
            target_allocations.push(TargetAllocation {
                ticker: asset.ticker.clone(),
                weight: asset.target_weight,
            });
        }

        // 현재 포트폴리오 생성
        let mut current_positions: Vec<PortfolioPosition> = Vec::new();
        for (ticker, quantity) in &self.holdings {
            let price = self
                .selected_assets
                .iter()
                .find(|a| &a.ticker == ticker)
                .map(|a| a.current_price)
                .unwrap_or(Decimal::ZERO);

            let market_value = *quantity * price;
            current_positions.push(PortfolioPosition {
                ticker: ticker.clone(),
                quantity: *quantity,
                current_price: price,
                market_value,
            });
        }

        // 현금 포지션 추가 (리밸런싱 계산을 위해)
        let total_holdings_value: Decimal = current_positions.iter().map(|p| p.market_value).sum();
        let cash_amount = config.total_amount - total_holdings_value;
        if cash_amount > Decimal::ZERO {
            let rebalance_cfg = config.market.rebalance_config();
            current_positions.push(PortfolioPosition {
                ticker: rebalance_cfg.cash_ticker.clone(),
                quantity: cash_amount,
                current_price: Decimal::ONE,
                market_value: cash_amount,
            });
        }

        // 리밸런싱 계산
        let rebalance_config = config.market.rebalance_config();
        let calculator = RebalanceCalculator::new(rebalance_config);
        let result = calculator.calculate_orders(&current_positions, &target_allocations);
        let orders = result.orders;

        // 주문을 시그널로 변환
        for order in orders {
            let (signal_type, side) = match order.side {
                RebalanceOrderSide::Buy => (SignalType::Entry, Side::Buy),
                RebalanceOrderSide::Sell => (SignalType::Exit, Side::Sell),
            };

            let ticker_with_quote = format!("{}/{}", order.ticker, config.market.quote_currency());
            let strength = self
                .selected_assets
                .iter()
                .find(|a| a.ticker == order.ticker)
                .map(|a| {
                    // GlobalScore 기반 신호 강도 조정
                    adjust_strength_by_score(0.5, Some(a.score))
                })
                .unwrap_or(0.5);

            let signal = match signal_type {
                SignalType::Entry => {
                    Signal::entry("screening_based", ticker_with_quote.clone(), side)
                        .with_strength(strength)
                        .with_metadata("quantity", json!(order.quantity.to_string()))
                        .with_metadata("reason", json!("rebalance"))
                        .with_metadata("variant", json!(format!("{:?}", config.variant)))
                }
                SignalType::Exit => {
                    Signal::exit("screening_based", ticker_with_quote.clone(), side)
                        .with_strength(strength)
                        .with_metadata("quantity", json!(order.quantity.to_string()))
                        .with_metadata("reason", json!("rebalance"))
                }
                _ => continue,
            };

            signals.push(signal);
            self.trades_count += 1;
        }

        // 리밸런싱 시각 업데이트
        self.last_rebalance_time = Some(current_time);
        self.last_rebalance_month = Some(current_time.month());

        info!(
            variant = ?config.variant,
            selected_count = self.selected_assets.len(),
            signals_count = signals.len(),
            "리밸런싱 신호 생성 완료"
        );

        signals
    }
}

impl ScreeningBasedStrategy {
    /// 소형주 퀀트 팩토리.
    pub fn small_cap_quant() -> Self {
        Self::new()
    }

    /// 연금 자동화 팩토리.
    pub fn pension_bot() -> Self {
        Self::new()
    }

    /// 동적 유니버스 팩토리.
    pub fn dynamic_universe() -> Self {
        Self::new()
    }
}

impl Default for ScreeningBasedStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for ScreeningBasedStrategy {
    fn name(&self) -> &str {
        "ScreeningBasedStrategy"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "동적 유니버스 기반 투자 전략 (스크리닝 결과에서 종목 선택)"
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let parsed_config: ScreeningBasedConfig = serde_json::from_value(config)?;

        info!(
            variant = ?parsed_config.variant,
            preset = %parsed_config.preset_name,
            min_score = %parsed_config.min_score,
            top_n = parsed_config.top_n,
            "스크리닝 기반 전략 초기화"
        );

        self.config = Some(parsed_config);
        self.initialized = true;

        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        if !self.initialized {
            return Ok(Vec::new());
        }

        let current_time = data.timestamp;

        // 리밸런싱 시점 확인
        if !self.should_rebalance(current_time) {
            return Ok(Vec::new());
        }

        // 스크리닝 결과에서 종목 선택
        self.selected_assets = self.select_assets_from_screening().await;

        if self.selected_assets.is_empty() {
            debug!("선택된 종목이 없어 리밸런싱 건너뜀");
            return Ok(Vec::new());
        }

        // 리밸런싱 신호 생성
        let signals = self.generate_rebalance_signals(current_time);

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 보유 수량 업데이트
        let ticker = order.ticker.split('/').next().unwrap_or(&order.ticker);
        let entry = self.holdings.entry(ticker.to_string()).or_insert(Decimal::ZERO);

        match order.side {
            Side::Buy => *entry += order.filled_quantity,
            Side::Sell => *entry -= order.filled_quantity,
        }

        // 0 이하면 제거
        if *entry <= Decimal::ZERO {
            self.holdings.remove(ticker);
        }

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
            "스크리닝 기반 전략 종료"
        );
        Ok(())
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
    }

    fn get_state(&self) -> Value {
        let config = self.config.as_ref();
        json!({
            "initialized": self.initialized,
            "variant": config.map(|c| format!("{:?}", c.variant)),
            "preset_name": config.map(|c| c.preset_name.clone()),
            "selected_assets_count": self.selected_assets.len(),
            "holdings_count": self.holdings.len(),
            "trades_count": self.trades_count,
            "last_rebalance_time": self.last_rebalance_time.map(|t| t.to_rfc3339()),
        })
    }
}

// ============================================================================
// UI Config 변형들 (SDUI용)
// ============================================================================

/// 소형주 퀀트 UI Config.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "small_cap_quant_v2",
    name = "소형주 퀀트 (스크리닝 기반)",
    description = "스크리닝 결과 기반 소형주 퀀트 전략",
    category = "Monthly"
)]
pub struct SmallCapQuantV2Config {
    /// 최소 GlobalScore
    #[serde(default = "default_min_score")]
    #[schema(label = "최소 점수", default = "60")]
    pub min_score: Decimal,

    /// 선택할 상위 종목 수
    #[serde(default = "default_top_n_small_cap")]
    #[schema(label = "상위 종목 수", default = "20")]
    pub top_n: usize,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    #[schema(label = "투자 금액", default = "10000000")]
    pub total_amount: Decimal,
}

fn default_top_n_small_cap() -> usize {
    20
}

impl From<SmallCapQuantV2Config> for ScreeningBasedConfig {
    fn from(cfg: SmallCapQuantV2Config) -> Self {
        Self {
            variant: ScreeningVariant::SmallCapQuant,
            market: MarketType::KR,
            preset_name: "small_cap_quant".to_string(),
            min_score: cfg.min_score,
            top_n: cfg.top_n,
            weighting_method: WeightingMethod::Equal,
            rebalance_frequency: RebalanceFrequency::Monthly,
            rebalance_threshold: dec!(0.05),
            total_amount: cfg.total_amount,
            use_route_filter: false,
            exit_config: ExitConfig::default(),
        }
    }
}

/// 연금 자동화 UI Config.
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "pension_bot_v2",
    name = "연금 자동화 (스크리닝 기반)",
    description = "스크리닝 결과 기반 연금 자동화 전략",
    category = "Daily"
)]
pub struct PensionBotV2Config {
    /// 최소 GlobalScore
    #[serde(default = "default_min_score_pension")]
    #[schema(label = "최소 점수", default = "70")]
    pub min_score: Decimal,

    /// 선택할 상위 종목 수
    #[serde(default = "default_top_n_pension")]
    #[schema(label = "상위 종목 수", default = "10")]
    pub top_n: usize,

    /// 총 투자 금액
    #[serde(default = "default_total_amount")]
    #[schema(label = "투자 금액", default = "10000000")]
    pub total_amount: Decimal,

    /// RouteState 필터 사용 여부
    #[serde(default = "default_use_route_filter")]
    #[schema(label = "RouteState 필터", default = "true")]
    pub use_route_filter: bool,
}

fn default_min_score_pension() -> Decimal {
    dec!(70)
}

fn default_top_n_pension() -> usize {
    10
}

impl From<PensionBotV2Config> for ScreeningBasedConfig {
    fn from(cfg: PensionBotV2Config) -> Self {
        Self {
            variant: ScreeningVariant::PensionBot,
            market: MarketType::KR,
            preset_name: "pension_bot".to_string(),
            min_score: cfg.min_score,
            top_n: cfg.top_n,
            weighting_method: WeightingMethod::ScoreProportional,
            rebalance_frequency: RebalanceFrequency::Weekly,
            rebalance_threshold: dec!(0.03),
            total_amount: cfg.total_amount,
            use_route_filter: cfg.use_route_filter,
            exit_config: ExitConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = ScreeningBasedConfig::default();
        assert_eq!(config.min_score, dec!(60));
        assert_eq!(config.top_n, 10);
        assert!(config.use_route_filter);
    }

    #[test]
    fn test_small_cap_quant_config() {
        let config = ScreeningBasedConfig::small_cap_quant_default();
        assert_eq!(config.variant, ScreeningVariant::SmallCapQuant);
        assert_eq!(config.preset_name, "small_cap_quant");
        assert_eq!(config.top_n, 20);
        assert!(!config.use_route_filter);
    }

    #[test]
    fn test_pension_bot_config() {
        let config = ScreeningBasedConfig::pension_bot_default();
        assert_eq!(config.variant, ScreeningVariant::PensionBot);
        assert_eq!(config.preset_name, "pension_bot");
        assert_eq!(config.min_score, dec!(70));
        assert!(config.use_route_filter);
    }

    #[test]
    fn test_ui_config_conversion() {
        let ui_config = SmallCapQuantV2Config {
            min_score: dec!(65),
            top_n: 15,
            total_amount: dec!(5_000_000),
        };

        let config: ScreeningBasedConfig = ui_config.into();
        assert_eq!(config.variant, ScreeningVariant::SmallCapQuant);
        assert_eq!(config.min_score, dec!(65));
        assert_eq!(config.top_n, 15);
    }
}

// ============================================================================
// 전략 등록 (Strategy Registration)
// ============================================================================

use crate::register_strategy;

// 소형주 퀀트 V2 (스크리닝 기반)
// 티커 목록은 동적으로 스크리닝 결과에서 선택됨
register_strategy! {
    id: "small_cap_quant_v2",
    aliases: ["screening_small_cap"],
    name: "소형주 퀀트 (스크리닝 기반)",
    description: "스크리닝 결과 기반 소형주 퀀트 전략",
    timeframe: "1d",
    tickers: [],
    category: Monthly,
    markets: [Stock],
    factory: ScreeningBasedStrategy::small_cap_quant,
    config: SmallCapQuantV2Config
}

// 연금 자동화 V2 (스크리닝 기반)
// 주간 리밸런싱이지만 Daily 카테고리 사용 (주기는 config에서 설정)
register_strategy! {
    id: "pension_bot_v2",
    aliases: ["screening_pension"],
    name: "연금 자동화 (스크리닝 기반)",
    description: "스크리닝 결과 기반 연금 자동화 전략",
    timeframe: "1d",
    tickers: [],
    category: Daily,
    markets: [Stock],
    factory: ScreeningBasedStrategy::pension_bot,
    config: PensionBotV2Config
}

// 동적 유니버스 전략
register_strategy! {
    id: "dynamic_universe",
    aliases: ["screening_dynamic"],
    name: "동적 유니버스",
    description: "스크리닝 결과 기반 동적 종목 선택 전략",
    timeframe: "1d",
    tickers: [],
    category: Monthly,
    markets: [Stock],
    factory: ScreeningBasedStrategy::dynamic_universe,
    config: ScreeningBasedConfig
}
