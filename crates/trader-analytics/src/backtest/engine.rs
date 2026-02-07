//! 백테스팅 엔진
//!
//! 과거 데이터로 트레이딩 전략을 시뮬레이션하고 성과를 분석합니다.
//!
//! # 주요 기능
//!
//! - **전략 시뮬레이션**: 과거 시장 데이터로 전략의 신호 생성 및 실행
//! - **주문 체결 시뮬레이션**: 슬리피지, 수수료 등 현실적인 체결 모델
//! - **성과 분석**: PerformanceTracker와 통합된 상세한 성과 지표
//! - **자산 곡선**: 시간에 따른 자산 가치 변화 추적
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! use trader_analytics::backtest::{BacktestConfig, BacktestEngine};
//! use trader_strategy::Strategy;
//! use rust_decimal_macros::dec;
//!
//! // 백테스트 설정
//! let config = BacktestConfig::new(dec!(10_000_000))
//!     .with_commission_rate(dec!(0.001))  // 0.1% 수수료
//!     .with_slippage_rate(dec!(0.0005));  // 0.05% 슬리피지
//!
//! // 백테스트 엔진 생성
//! let mut engine = BacktestEngine::new(config);
//!
//! // 백테스트 실행
//! let result = engine.run(&mut strategy, &historical_klines).await?;
//!
//! // 결과 분석
//! println!("총 수익률: {}%", result.metrics.total_return_pct);
//! println!("샤프 비율: {}", result.metrics.sharpe_ratio);
//! println!("최대 낙폭: {}%", result.metrics.max_drawdown_pct);
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use trader_core::{
    unrealized_pnl, Kline, MarketData, ScreeningCalculator, Side, Signal, SignalMarker, SignalType, StrategyContext, Trade,
};
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::backtest::slippage::SlippageModel;
use crate::backtest::candle_processor::CandleProcessor;
use crate::performance::{EquityPoint, PerformanceMetrics, PerformanceTracker, RoundTrip};
use trader_execution::{ProcessorConfig, SignalProcessor, SimulatedExecutor, TradeResult};

/// 백테스트 오류
#[derive(Debug, Error)]
pub enum BacktestError {
    /// 설정 오류
    #[error("백테스트 설정 오류: {0}")]
    ConfigError(String),

    /// 데이터 오류
    #[error("데이터 오류: {0}")]
    DataError(String),

    /// 전략 오류
    #[error("전략 실행 오류: {0}")]
    StrategyError(String),

    /// 실행 오류
    #[error("실행 오류: {0}")]
    ExecutionError(String),

    /// 자금 부족
    #[error("자금 부족: 필요={required}, 가용={available}")]
    InsufficientFunds {
        required: Decimal,
        available: Decimal,
    },
}

/// 백테스트 결과 타입
pub type BacktestResult<T> = Result<T, BacktestError>;

/// 백테스트 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    /// 초기 자본금
    #[serde(default = "default_initial_capital")]
    pub initial_capital: Decimal,

    /// 거래 수수료율 (예: 0.001 = 0.1%)
    #[serde(default = "default_commission_rate")]
    pub commission_rate: Decimal,

    /// 슬리피지율 (예: 0.0005 = 0.05%)
    ///
    /// 참고: slippage_model이 설정되면 무시됩니다.
    #[serde(default = "default_slippage_rate")]
    pub slippage_rate: Decimal,

    /// 동적 슬리피지 모델 (Optional)
    ///
    /// 설정되면 slippage_rate 대신 이 모델을 사용합니다.
    /// Fixed, Linear, VolatilityBased, Tiered 모델 지원.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slippage_model: Option<SlippageModel>,

    /// 최대 동시 포지션 수
    #[serde(default = "default_max_positions")]
    pub max_positions: usize,

    /// 포지션당 최대 자본 비율 (예: 0.1 = 10%)
    #[serde(default = "default_max_position_size_pct")]
    pub max_position_size_pct: Decimal,

    /// 무위험 이자율 (연율화 계산용)
    #[serde(default = "default_risk_free_rate")]
    pub risk_free_rate: f64,

    /// 거래소 이름 (시뮬레이션용)
    #[serde(default = "default_exchange_name")]
    pub exchange_name: String,

    /// 틱 데이터 사용 여부 (캔들 내 가격 변동 시뮬레이션)
    #[serde(default)]
    pub use_tick_simulation: bool,

    /// 마진 거래 허용 여부
    #[serde(default)]
    pub allow_margin: bool,

    /// 숏 포지션 허용 여부
    #[serde(default)]
    pub allow_short: bool,

    // === 리스크 관리 (exit_config에서 추출) ===
    /// 자동 손절 활성화
    #[serde(default)]
    pub auto_stop_loss: bool,

    /// 자동 익절 활성화
    #[serde(default)]
    pub auto_take_profit: bool,

    /// 손절 비율 (예: 0.05 = 5%)
    #[serde(default = "default_stop_loss_pct")]
    pub stop_loss_pct: Decimal,

    /// 익절 비율 (예: 0.10 = 10%)
    #[serde(default = "default_take_profit_pct")]
    pub take_profit_pct: Decimal,

    /// 최소 신호 강도 (기본값: 0.0 = 모든 신호 허용)
    #[serde(default)]
    pub min_strength: f64,
}

// 설정 기본값 함수들 (serde default용)
fn default_initial_capital() -> Decimal {
    Decimal::new(10_000_000, 0)
}
fn default_commission_rate() -> Decimal {
    Decimal::new(1, 3)
} // 0.1%
fn default_slippage_rate() -> Decimal {
    Decimal::new(5, 4)
} // 0.05%
fn default_max_positions() -> usize {
    10
}
fn default_max_position_size_pct() -> Decimal {
    Decimal::new(2, 1)
} // 20%
fn default_risk_free_rate() -> f64 {
    0.05
} // 5%
fn default_exchange_name() -> String {
    "backtest".to_string()
}
fn default_stop_loss_pct() -> Decimal {
    Decimal::new(5, 2)
} // 5%
fn default_take_profit_pct() -> Decimal {
    Decimal::new(10, 2)
} // 10%

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: default_initial_capital(),
            commission_rate: default_commission_rate(),
            slippage_rate: default_slippage_rate(),
            slippage_model: None,
            max_positions: default_max_positions(),
            max_position_size_pct: default_max_position_size_pct(),
            risk_free_rate: default_risk_free_rate(),
            exchange_name: default_exchange_name(),
            use_tick_simulation: false,
            allow_margin: false,
            allow_short: false,
            auto_stop_loss: false,
            auto_take_profit: false,
            stop_loss_pct: default_stop_loss_pct(),
            take_profit_pct: default_take_profit_pct(),
            min_strength: 0.0,
        }
    }
}

impl BacktestConfig {
    /// 새로운 백테스트 설정을 생성합니다.
    pub fn new(initial_capital: Decimal) -> Self {
        Self {
            initial_capital,
            ..Default::default()
        }
    }

    /// 수수료율 설정
    pub fn with_commission_rate(mut self, rate: Decimal) -> Self {
        self.commission_rate = rate;
        self
    }

    /// 슬리피지율 설정 (고정 비율)
    ///
    /// 참고: with_slippage_model()로 동적 모델을 설정하면 무시됩니다.
    pub fn with_slippage_rate(mut self, rate: Decimal) -> Self {
        self.slippage_rate = rate;
        self
    }

    /// 동적 슬리피지 모델 설정
    ///
    /// 설정되면 slippage_rate 대신 이 모델을 사용합니다.
    ///
    /// # 예시
    /// ```rust,ignore
    /// use trader_analytics::backtest::{BacktestConfig, SlippageModel};
    ///
    /// // 변동성 기반 모델 사용
    /// let config = BacktestConfig::default()
    ///     .with_slippage_model(SlippageModel::volatility_based(0.5));
    ///
    /// // 구간별 차등 모델 사용
    /// let config = BacktestConfig::default()
    ///     .with_slippage_model(SlippageModel::tiered(vec![
    ///         (dec!(10000), dec!(0.0003)),
    ///         (dec!(100000), dec!(0.0005)),
    ///         (dec!(1000000), dec!(0.001)),
    ///     ]));
    /// ```
    pub fn with_slippage_model(mut self, model: SlippageModel) -> Self {
        self.slippage_model = Some(model);
        self
    }

    /// 최대 포지션 수 설정
    pub fn with_max_positions(mut self, max: usize) -> Self {
        self.max_positions = max;
        self
    }

    /// 포지션 크기 제한 설정
    pub fn with_max_position_size_pct(mut self, pct: Decimal) -> Self {
        self.max_position_size_pct = pct;
        self
    }

    /// 무위험 이자율 설정
    pub fn with_risk_free_rate(mut self, rate: f64) -> Self {
        self.risk_free_rate = rate;
        self
    }

    /// 숏 포지션 허용 설정
    pub fn with_allow_short(mut self, allow: bool) -> Self {
        self.allow_short = allow;
        self
    }

    /// 자동 손절 설정
    pub fn with_stop_loss(mut self, enabled: bool, pct: Decimal) -> Self {
        self.auto_stop_loss = enabled;
        self.stop_loss_pct = pct;
        self
    }

    /// 자동 익절 설정
    pub fn with_take_profit(mut self, enabled: bool, pct: Decimal) -> Self {
        self.auto_take_profit = enabled;
        self.take_profit_pct = pct;
        self
    }

    /// 최소 신호 강도 설정
    pub fn with_min_strength(mut self, strength: f64) -> Self {
        self.min_strength = strength;
        self
    }

    /// 설정 검증
    pub fn validate(&self) -> BacktestResult<()> {
        if self.initial_capital <= Decimal::ZERO {
            return Err(BacktestError::ConfigError(
                "초기 자본은 0보다 커야 합니다".to_string(),
            ));
        }
        if self.commission_rate < Decimal::ZERO {
            return Err(BacktestError::ConfigError(
                "수수료율은 0 이상이어야 합니다".to_string(),
            ));
        }
        if self.slippage_rate < Decimal::ZERO {
            return Err(BacktestError::ConfigError(
                "슬리피지율은 0 이상이어야 합니다".to_string(),
            ));
        }
        Ok(())
    }
}

/// 백테스트 실행 리포트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestReport {
    /// 설정 정보
    pub config: BacktestConfig,

    /// 성과 지표
    pub metrics: PerformanceMetrics,

    /// 완료된 거래 (라운드트립)
    pub trades: Vec<RoundTrip>,

    /// 자산 곡선
    pub equity_curve: Vec<EquityPoint>,

    /// 총 거래 횟수 (진입 + 청산)
    pub total_orders: usize,

    /// 총 수수료
    pub total_commission: Decimal,

    /// 총 슬리피지 비용
    pub total_slippage: Decimal,

    /// 백테스트 기간 시작
    pub start_time: DateTime<Utc>,

    /// 백테스트 기간 종료
    pub end_time: DateTime<Utc>,

    /// 데이터 포인트 수
    pub data_points: usize,

    /// 심볼별 성과
    pub performance_by_symbol: HashMap<String, PerformanceMetrics>,

    /// 신호 마커 (차트 표시 및 분석용)
    pub signal_markers: Vec<SignalMarker>,

    /// 캔들 데이터 (차트 표시용)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub klines: Vec<Kline>,

    /// 주 심볼
    #[serde(default)]
    pub symbol: String,

    /// 모든 거래 기록 (매수/매도 포함) - 매매일지용
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub all_trades: Vec<TradeResult>,
}

impl BacktestReport {
    /// 요약 문자열 반환
    pub fn summary(&self) -> String {
        let duration_days = (self.end_time - self.start_time).num_days();

        format!(
            "백테스트 결과 요약\n\
             ═══════════════════════════════════════\n\
             기간: {} → {} ({} 일)\n\
             데이터 포인트: {}\n\
             ───────────────────────────────────────\n\
             초기 자본: {}\n\
             최종 자산: {:.2}\n\
             순수익: {:.2}\n\
             총 수익률: {:.2}%\n\
             연율화 수익률: {:.2}%\n\
             ───────────────────────────────────────\n\
             총 거래: {}\n\
             승률: {:.1}%\n\
             프로핏 팩터: {:.2}\n\
             ───────────────────────────────────────\n\
             샤프 비율: {:.2}\n\
             소르티노 비율: {:.2}\n\
             최대 낙폭: {:.2}%\n\
             칼마 비율: {:.2}\n\
             ───────────────────────────────────────\n\
             총 수수료: {:.2}\n\
             총 슬리피지: {:.2}\n\
             ═══════════════════════════════════════",
            self.start_time.format("%Y-%m-%d"),
            self.end_time.format("%Y-%m-%d"),
            duration_days,
            self.data_points,
            self.config.initial_capital,
            self.config.initial_capital + self.metrics.net_profit,
            self.metrics.net_profit,
            self.metrics.total_return_pct,
            self.metrics.annualized_return_pct,
            self.metrics.total_trades,
            self.metrics.win_rate_pct,
            self.metrics.profit_factor,
            self.metrics.sharpe_ratio,
            self.metrics.sortino_ratio,
            self.metrics.max_drawdown_pct,
            self.metrics.calmar_ratio,
            self.total_commission,
            self.total_slippage,
        )
    }
}

/// 백테스팅 엔진
///
/// 과거 데이터로 전략을 시뮬레이션하고 성과를 분석합니다.
///
/// # 아키텍처
///
/// ```text
/// BacktestEngine (데이터 제공 + 성과 추적)
///        │
///        ▼
/// SimulatedExecutor (Signal 처리 - 포지션/잔고 관리)
/// ```
///
/// 포지션 관리는 SimulatedExecutor에 위임됩니다.
/// 이를 통해 백테스트/시뮬레이션/실거래가 동일한 로직을 사용합니다.
pub struct BacktestEngine {
    /// 설정
    config: BacktestConfig,

    /// Signal 처리기 (포지션/잔고 관리)
    executor: SimulatedExecutor,

    /// 성과 추적기
    tracker: PerformanceTracker,

    /// 현재 시뮬레이션 시각
    current_time: DateTime<Utc>,

    /// 현재 가격 (심볼별)
    current_prices: HashMap<String, Decimal>,

    /// 신호 마커 (차트 표시 및 분석용)
    signal_markers: Vec<SignalMarker>,

    /// 총 슬리피지 (executor와 별도 추적 - 기존 호환성)
    total_slippage: Decimal,
}

impl BacktestEngine {
    /// 새로운 백테스트 엔진을 생성합니다.
    pub fn new(config: BacktestConfig) -> Self {
        // 백테스트용 트래커: 과거 데이터 자산 곡선 삭제 방지
        let tracker = PerformanceTracker::new(config.initial_capital)
            .with_risk_free_rate(config.risk_free_rate)
            .without_equity_history_limit();

        // Signal 처리기 설정 (BacktestConfig에서 변환)
        let executor_config = ProcessorConfig {
            commission_rate: config.commission_rate,
            slippage_rate: config.slippage_rate,
            max_position_size_pct: config.max_position_size_pct,
            max_positions: config.max_positions,
            allow_short: config.allow_short,
            min_strength: config.min_strength,
            auto_stop_loss: config.auto_stop_loss,
            auto_take_profit: config.auto_take_profit,
            stop_loss_pct: config.stop_loss_pct,
            take_profit_pct: config.take_profit_pct,
        };
        let executor = SimulatedExecutor::new(executor_config, config.initial_capital);

        Self {
            config,
            executor,
            tracker,
            current_time: Utc::now(),
            current_prices: HashMap::new(),
            signal_markers: Vec::new(),
            total_slippage: Decimal::ZERO,
        }
    }

    // === 위임 메서드 (기존 API 호환성 유지) ===

    /// 현재 잔고 조회 (executor에서 위임)
    pub fn balance(&self) -> Decimal {
        self.executor.balance()
    }

    /// 포지션 수 조회 (executor에서 위임)
    pub fn positions_count(&self) -> usize {
        self.executor.positions().len()
    }

    /// 총 수수료 조회 (executor에서 위임)
    pub fn total_commission(&self) -> Decimal {
        self.executor.total_commission()
    }

    /// 총 주문 수 조회 (executor에서 위임)
    pub fn total_orders(&self) -> usize {
        self.executor.total_orders()
    }

    /// StrategyContext 연동 백테스트 실행.
    ///
    /// 각 캔들 시점마다 StructuralFeatures를 재계산하여 StrategyContext에 업데이트합니다.
    /// 이를 통해 실거래와 동일한 방식으로 전략이 지표 데이터에 접근할 수 있습니다.
    ///
    /// # 다중 심볼 지원
    ///
    /// 다중 심볼 전략의 경우, 호출 전에 StrategyContext에 모든 심볼의 klines를
    /// `context.update_klines(symbol, timeframe, klines)` 메서드로 등록해야 합니다.
    /// BacktestEngine은 메인 루프에서 각 시점마다 등록된 모든 심볼의 klines를
    /// 현재 시점까지 필터링하여 업데이트합니다.
    ///
    /// # 인자
    ///
    /// * `strategy` - 전략 인스턴스
    /// * `klines` - 메인 티커의 과거 캔들 데이터
    /// * `context` - StrategyContext (지표 데이터 업데이트 대상, 다중 심볼 klines 포함)
    /// * `ticker` - 메인 종목 티커
    /// * `screening_calculator` - 스크리닝 계산기 (동적 유니버스 전략용, None이면 스크리닝 비활성화)
    pub async fn run<S>(
        &mut self,
        strategy: &mut S,
        klines: &[Kline],
        context: Arc<RwLock<StrategyContext>>,
        ticker: &str,
        screening_calculator: Option<&dyn ScreeningCalculator>,
    ) -> BacktestResult<BacktestReport>
    where
        S: trader_strategy::Strategy + ?Sized,
    {
        // 설정 검증
        self.config.validate()?;

        if klines.is_empty() {
            return Err(BacktestError::DataError(
                "캔들 데이터가 비어있습니다".to_string(),
            ));
        }

        // 시간순 정렬 확인
        for window in klines.windows(2) {
            if window[0].open_time > window[1].open_time {
                return Err(BacktestError::DataError(
                    "캔들 데이터가 시간순으로 정렬되어 있지 않습니다".to_string(),
                ));
            }
        }

        let start_time = klines.first().unwrap().open_time;
        let end_time = klines.last().unwrap().close_time;
        let data_points = klines.len();

        // 백테스트 시작 시간으로 equity curve 초기 timestamp 설정
        self.tracker.set_initial_timestamp(start_time);

        // 공통 캔들 프로세서 (SimulationEngine과 동일한 로직 공유)
        let mut candle_processor = CandleProcessor::new();
        let exchange_name = self.config.exchange_name.clone();

        // 각 캔들에 대해 시뮬레이션
        for (idx, kline) in klines.iter().enumerate() {
            // 1. StrategyContext 업데이트 (공통: 지표, klines, 스크리닝)
            let historical_klines = &klines[..=idx];
            candle_processor.update_context(
                idx, kline, historical_klines, &context, ticker, screening_calculator,
            ).await?;

            // 가격/시간 동기화 (BacktestEngine 내부 메서드용)
            self.current_time = candle_processor.current_time();
            self.current_prices.clone_from(candle_processor.current_prices());

            // 2. 시그널 생성 (공통: 멀티 심볼/멀티 TF + Entry/Exit 파티셔닝)
            let signals = candle_processor.generate_signals(
                strategy, kline, &context, ticker, &exchange_name,
            ).await?;

            // 3. 시그널 처리 (BacktestEngine 고유: PerformanceTracker/SignalMarker 기록)
            for signal in &signals.entry_signals {
                self.process_signal(signal, kline).await?;
            }
            for signal in &signals.exit_signals {
                self.process_signal(signal, kline).await?;
            }

            // 4. 포지션 동기화 (공통: 전략에 현재 포지션 상태 알림)
            candle_processor.sync_positions(
                strategy, self.executor.positions(), kline, &exchange_name, ticker,
            ).await?;

            // 5. 미실현 손익 반영하여 자산 업데이트 (BacktestEngine 고유)
            let equity = self.calculate_equity(kline);
            self.tracker.update_equity(kline.close_time, equity);
        }

        // 미청산 포지션 강제 청산
        let last_kline = klines.last().unwrap();
        self.close_all_positions(last_kline).await?;

        // 강제 청산 후 최종 자산 업데이트 (실현 손익 반영)
        let final_equity = self.calculate_equity(last_kline);
        self.tracker.update_equity(last_kline.close_time, final_equity);

        // 심볼별 성과 계산
        let performance_by_symbol = self.calculate_performance_by_symbol();

        // 결과 생성
        // PerformanceMetrics는 완료된 거래(RoundTrip) 기준으로 계산
        // MDD는 equity curve 기반으로 교체
        let mut metrics = self.tracker.get_metrics();
        metrics.max_drawdown_pct = self.tracker.max_drawdown_pct();

        Ok(BacktestReport {
            config: self.config.clone(),
            metrics,
            trades: self.tracker.get_round_trips().to_vec(),
            equity_curve: self.tracker.get_equity_curve().to_vec(),
            total_orders: self.total_orders(),
            total_commission: self.total_commission(),
            total_slippage: self.total_slippage,
            start_time,
            end_time,
            data_points,
            performance_by_symbol,
            signal_markers: self.signal_markers.clone(),
            klines: klines.to_vec(),
            symbol: ticker.to_string(),
            all_trades: self.executor.trades().to_vec(),
        })
    }

    /// 신호를 처리합니다.
    ///
    /// SimulatedExecutor에 위임하여 포지션을 관리합니다.
    async fn process_signal(&mut self, signal: &Signal, kline: &Kline) -> BacktestResult<()> {
        // 실행 가격 결정
        let current_price = self.get_price_for_signal(signal, kline);

        // Alert는 실행하지 않음 - marker만 저장
        if signal.signal_type == SignalType::Alert {
            let marker = SignalMarker::from_signal(
                signal,
                current_price,
                kline.open_time,
                &signal.strategy_id,
            );
            self.signal_markers.push(marker);
            return Ok(());
        }

        // SimulatedExecutor에 Signal 처리 위임
        let result = self.executor
            .process_signal(signal, current_price, kline.close_time)
            .await
            .map_err(|e| BacktestError::ExecutionError(e.to_string()))?;

        // SignalMarker 생성 (실행 결과 반영)
        let executed = result.is_some();
        let marker = SignalMarker::from_signal(
            signal,
            current_price,
            kline.open_time,
            &signal.strategy_id,
        ).with_executed(executed);
        self.signal_markers.push(marker);

        // 거래가 발생한 경우 tracker에 기록
        if let Some(trade_result) = result {
            self.record_trade_result(&trade_result, signal)?;

            // 슬리피지 별도 추적 (기존 호환성)
            self.total_slippage += trade_result.slippage;
        }

        Ok(())
    }

    /// Signal에 대한 현재 가격 조회
    ///
    /// 다중 자산 전략에서는 신호 심볼과 현재 kline 심볼이 다를 수 있음:
    /// 1. signal.suggested_price가 있으면 사용
    /// 2. current_prices에서 해당 심볼의 가격 사용
    /// 3. fallback: kline.close (단일 자산 전략)
    fn get_price_for_signal(&self, signal: &Signal, kline: &Kline) -> Decimal {
        let key = &signal.ticker;
        // 티커 형식 정규화: "TLT/USD" → "TLT"
        let base_ticker = key.split('/').next().unwrap_or(key);

        signal.suggested_price
            .or_else(|| self.current_prices.get(key).copied())
            .or_else(|| self.current_prices.get(base_ticker).copied())
            .unwrap_or(kline.close)
    }

    /// TradeResult를 PerformanceTracker에 기록
    fn record_trade_result(&mut self, trade_result: &TradeResult, signal: &Signal) -> BacktestResult<()> {
        // 기존 create_trade 헬퍼 사용
        let trade = self.create_trade(
            signal,
            trade_result.price,
            trade_result.quantity,
            trade_result.commission,
            trade_result.realized_pnl.is_none(), // is_entry: PnL이 없으면 진입
        );

        let is_entry = matches!(signal.signal_type, SignalType::Entry | SignalType::AddToPosition);

        // 디버그: 거래 기록 추적
        tracing::debug!(
            symbol = %trade.ticker,
            side = ?trade.side,
            signal_type = ?signal.signal_type,
            is_entry = is_entry,
            price = %trade.price,
            quantity = %trade.quantity,
            "record_trade_result: 거래 기록"
        );

        self.tracker
            .record_trade(&trade, is_entry, Some(signal.strategy_id.clone()))
            .map_err(|e| BacktestError::ExecutionError(e.to_string()))?;

        Ok(())
    }

    /// 모든 포지션을 청산합니다.
    ///
    /// executor에서 포지션 정보를 가져와 각각에 대해 Exit Signal을 처리합니다.
    /// position_id가 있는 포지션(그리드 등)은 해당 ID로 청산합니다.
    async fn close_all_positions(&mut self, kline: &Kline) -> BacktestResult<()> {
        // executor에서 포지션 정보 가져오기 (symbol, position_id 포함)
        let positions: Vec<_> = self.executor.positions()
            .iter()
            .map(|(_key, pos)| (pos.symbol.clone(), pos.position_id.clone(), pos.side, pos.quantity))
            .collect();

        if !positions.is_empty() {
            tracing::info!(
                "백테스트 종료: {} 개 포지션 강제 청산",
                positions.len()
            );
        }

        for (symbol, position_id, side, quantity) in positions {
            let exit_side = match side {
                Side::Buy => Side::Sell,
                Side::Sell => Side::Buy,
            };

            // 청산 가격 조회
            let base_ticker = symbol.split('/').next().unwrap_or(&symbol);
            let close_price = self.current_prices
                .get(&symbol)
                .or_else(|| self.current_prices.get(base_ticker))
                .copied()
                .unwrap_or(kline.close);

            tracing::debug!(
                symbol = %symbol,
                position_id = ?position_id,
                side = ?exit_side,
                quantity = %quantity,
                price = %close_price,
                "강제 청산 처리"
            );

            // 청산 사유를 포함한 Signal 생성
            let mut signal = Signal::exit("backtest_cleanup", symbol, exit_side);
            // position_id가 있으면 해당 포지션만 청산
            if let Some(pid) = position_id {
                signal = signal.with_position_id(pid);
            }
            signal.metadata.insert(
                "reason".to_string(),
                serde_json::Value::String("백테스트 종료 (강제 청산)".to_string()),
            );
            self.process_signal(&signal, kline).await?;
        }

        // 청산 후 남은 포지션 확인
        let remaining = self.executor.positions().len();
        if remaining > 0 {
            tracing::warn!(
                "강제 청산 후에도 {} 개 포지션이 남아있음",
                remaining
            );
        }

        Ok(())
    }

    /// 현재 자산 가치를 계산합니다.
    ///
    /// executor에서 잔고와 포지션 정보를 가져와 총 자산을 계산합니다.
    fn calculate_equity(&self, kline: &Kline) -> Decimal {
        let mut equity = self.executor.balance();

        for (symbol, position) in self.executor.positions().iter() {
            // 티커 형식 정규화: "TLT/USD" → "TLT"
            let base_ticker = symbol.split('/').next().unwrap_or(symbol);
            let current_price = self
                .current_prices
                .get(symbol)
                .or_else(|| self.current_prices.get(base_ticker))
                .copied()
                .unwrap_or(kline.close);

            let position_value = match position.side {
                Side::Buy => current_price * position.quantity,
                Side::Sell => {
                    // 숏 포지션: 원금 + 미실현 손익
                    let entry_value = position.entry_price * position.quantity;
                    let pnl = unrealized_pnl(
                        position.entry_price,
                        current_price,
                        position.quantity,
                        position.side,
                    );
                    entry_value + pnl
                }
            };

            equity += position_value;
        }

        equity
    }

    /// Trade 객체를 생성합니다.
    fn create_trade(
        &self,
        signal: &Signal,
        price: Decimal,
        quantity: Decimal,
        fee: Decimal,
        _is_entry: bool,
    ) -> Trade {
        // Signal.metadata에서 reason 추출하여 Trade.metadata에 저장
        let metadata = if let Some(reason) = signal.metadata.get("reason") {
            serde_json::json!({ "reason": reason })
        } else {
            // reason이 없으면 signal_type과 variant로 자동 생성
            let signal_type = format!("{:?}", signal.signal_type);
            let variant = signal
                .metadata
                .get("variant")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            serde_json::json!({
                "reason": format!("{} ({})", signal_type, variant)
            })
        };

        Trade::new(
            Uuid::new_v4(),
            &self.config.exchange_name,
            Uuid::new_v4().to_string(),
            signal.ticker.clone(),
            signal.side,
            quantity,
            price,
        )
        .with_fee(fee, "USDT")
        .with_executed_at(self.current_time)
        .with_metadata(metadata)
    }

    /// 심볼별 성과를 계산합니다.
    fn calculate_performance_by_symbol(&self) -> HashMap<String, PerformanceMetrics> {
        let mut by_symbol: HashMap<String, Vec<RoundTrip>> = HashMap::new();

        for rt in self.tracker.get_round_trips() {
            by_symbol
                .entry(rt.symbol.clone())
                .or_default()
                .push(rt.clone());
        }

        by_symbol
            .into_iter()
            .map(|(symbol, trades)| {
                let metrics = PerformanceMetrics::from_round_trips(
                    &trades,
                    self.config.initial_capital,
                    Some(self.config.risk_free_rate),
                );
                (symbol, metrics)
            })
            .collect()
    }

    /// 다중 타임프레임 백테스트를 실행합니다.
    ///
    /// Primary 타임프레임 캔들과 Secondary 타임프레임 캔들을 함께 사용하여
    /// 전략의 `on_multi_timeframe_data()` 메서드를 호출합니다.
    ///
    /// # 매개변수
    ///
    /// * `strategy` - 테스트할 전략 (Strategy trait 구현체)
    /// * `primary_klines` - Primary 타임프레임 캔들 데이터 (시간순 정렬 필수)
    /// * `secondary_klines` - Secondary 타임프레임별 캔들 데이터
    ///
    /// # Look-Ahead Bias 방지
    ///
    /// Secondary 데이터는 `TimeframeAligner`를 통해 각 Primary 캔들의 `close_time`
    /// 기준으로 완료된 캔들만 전략에 전달됩니다.
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// use std::collections::HashMap;
    /// use trader_core::Timeframe;
    ///
    /// let primary_klines = /* 5분봉 데이터 */;
    /// let mut secondary_klines = HashMap::new();
    /// secondary_klines.insert(Timeframe::H1, /* 1시간봉 데이터 */);
    /// secondary_klines.insert(Timeframe::D1, /* 일봉 데이터 */);
    ///
    /// let report = engine.run_multi_timeframe(
    ///     &mut strategy,
    ///     &primary_klines,
    ///     &secondary_klines,
    /// ).await?;
    /// ```
    pub async fn run_multi_timeframe<S>(
        &mut self,
        strategy: &mut S,
        primary_klines: &[Kline],
        secondary_klines: &HashMap<trader_core::Timeframe, Vec<Kline>>,
    ) -> BacktestResult<BacktestReport>
    where
        S: trader_strategy::Strategy,
    {
        use crate::timeframe_alignment::TimeframeAligner;

        // 설정 검증
        self.config.validate()?;

        if primary_klines.is_empty() {
            return Err(BacktestError::DataError(
                "Primary 캔들 데이터가 비어있습니다".to_string(),
            ));
        }

        // 시간순 정렬 확인
        for window in primary_klines.windows(2) {
            if window[0].open_time > window[1].open_time {
                return Err(BacktestError::DataError(
                    "Primary 캔들 데이터가 시간순으로 정렬되어 있지 않습니다".to_string(),
                ));
            }
        }

        let start_time = primary_klines.first().unwrap().open_time;
        let end_time = primary_klines.last().unwrap().close_time;
        let data_points = primary_klines.len();

        // 백테스트 시작 시간으로 equity curve 초기 timestamp 설정
        self.tracker.set_initial_timestamp(start_time);

        // 전략이 다중 타임프레임을 지원하는지 확인
        let is_multi_tf_strategy = strategy.multi_timeframe_config().is_some();

        // 각 Primary 캔들에 대해 시뮬레이션
        for kline in primary_klines {
            // 캔들 완성 시점으로 현재 시간 설정 (데이터 누수 방지)
            self.current_time = kline.close_time;
            self.current_prices
                .insert(kline.ticker.to_string(), kline.close);

            // 시장 데이터 생성
            let market_data = MarketData::from_kline(&self.config.exchange_name, kline.clone());

            // 전략에 데이터 전달
            let signals = if is_multi_tf_strategy {
                // 다중 타임프레임 전략: TimeframeAligner로 유효한 Secondary 데이터만 전달
                let aligned_secondary =
                    TimeframeAligner::align_multi_timeframe(secondary_klines, kline.close_time);

                strategy
                    .on_multi_timeframe_data(&market_data, &aligned_secondary)
                    .await
                    .map_err(|e| BacktestError::StrategyError(e.to_string()))?
            } else {
                // 단일 타임프레임 전략: 기존 메서드 호출
                strategy
                    .on_market_data(&market_data)
                    .await
                    .map_err(|e| BacktestError::StrategyError(e.to_string()))?
            };

            // 신호 처리
            for signal in signals {
                self.process_signal(&signal, kline).await?;
            }

            // 미실현 손익 반영하여 자산 업데이트
            let equity = self.calculate_equity(kline);
            self.tracker.update_equity(kline.close_time, equity);
        }

        // 미청산 포지션 강제 청산
        let last_kline = primary_klines.last().unwrap();
        self.close_all_positions(last_kline).await?;

        // 강제 청산 후 최종 자산 업데이트 (실현 손익 반영)
        let final_equity = self.calculate_equity(last_kline);
        self.tracker.update_equity(last_kline.close_time, final_equity);

        // 심볼별 성과 계산
        let performance_by_symbol = self.calculate_performance_by_symbol();

        // 결과 생성
        // PerformanceMetrics는 완료된 거래(RoundTrip) 기준으로 계산
        // MDD는 equity curve 기반으로 교체
        let mut metrics = self.tracker.get_metrics();
        metrics.max_drawdown_pct = self.tracker.max_drawdown_pct();

        Ok(BacktestReport {
            config: self.config.clone(),
            metrics,
            trades: self.tracker.get_round_trips().to_vec(),
            equity_curve: self.tracker.get_equity_curve().to_vec(),
            total_orders: self.total_orders(),
            total_commission: self.total_commission(),
            total_slippage: self.total_slippage,
            start_time,
            end_time,
            data_points,
            performance_by_symbol,
            signal_markers: self.signal_markers.clone(),
            klines: primary_klines.to_vec(),
            symbol: primary_klines
                .first()
                .map(|k| k.ticker.to_string())
                .unwrap_or_default(),
            all_trades: self.executor.trades().to_vec(),
        })
    }
}

/// 간단한 테스트용 전략
#[cfg(test)]
pub mod test_strategies {
    use super::*;
    use async_trait::async_trait;
    use serde_json::Value;
    use trader_core::{MarketDataType, Order, Position};

    /// 단순 이동평균 크로스오버 전략 (테스트용)
    pub struct SimpleSmaStrategy {
        short_period: usize,
        long_period: usize,
        prices: Vec<Decimal>,
        position_open: bool,
    }

    impl SimpleSmaStrategy {
        pub fn new(short_period: usize, long_period: usize) -> Self {
            Self {
                short_period,
                long_period,
                prices: Vec::new(),
                position_open: false,
            }
        }

        fn calculate_sma(&self, period: usize) -> Option<Decimal> {
            if self.prices.len() < period {
                return None;
            }

            let sum: Decimal = self.prices.iter().rev().take(period).sum();
            Some(sum / Decimal::from(period))
        }
    }

    #[async_trait]
    impl trader_strategy::Strategy for SimpleSmaStrategy {
        fn name(&self) -> &str {
            "SimpleSMA"
        }

        fn version(&self) -> &str {
            "1.0.0"
        }

        fn description(&self) -> &str {
            "단순 이동평균 크로스오버 전략"
        }

        async fn initialize(
            &mut self,
            _config: Value,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.prices.clear();
            self.position_open = false;
            Ok(())
        }

        async fn on_market_data(
            &mut self,
            data: &MarketData,
        ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
            let price = match &data.data {
                MarketDataType::Kline(k) => k.close,
                _ => return Ok(vec![]),
            };

            self.prices.push(price);

            let short_sma = match self.calculate_sma(self.short_period) {
                Some(sma) => sma,
                None => return Ok(vec![]),
            };

            let long_sma = match self.calculate_sma(self.long_period) {
                Some(sma) => sma,
                None => return Ok(vec![]),
            };

            let mut signals = vec![];

            // 골든 크로스 (단기 > 장기)
            if short_sma > long_sma && !self.position_open {
                signals.push(Signal::entry("SimpleSMA", data.ticker.clone(), Side::Buy));
                self.position_open = true;
            }
            // 데드 크로스 (단기 < 장기)
            else if short_sma < long_sma && self.position_open {
                signals.push(Signal::exit("SimpleSMA", data.ticker.clone(), Side::Sell));
                self.position_open = false;
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
            _position: &Position,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn get_state(&self) -> Value {
            serde_json::json!({
                "prices_count": self.prices.len(),
                "position_open": self.position_open
            })
        }
    }

    /// 항상 매수하는 전략 (테스트용)
    pub struct AlwaysBuyStrategy {
        bought: bool,
    }

    impl AlwaysBuyStrategy {
        pub fn new() -> Self {
            Self { bought: false }
        }
    }

    #[async_trait]
    impl trader_strategy::Strategy for AlwaysBuyStrategy {
        fn name(&self) -> &str {
            "AlwaysBuy"
        }

        fn version(&self) -> &str {
            "1.0.0"
        }

        fn description(&self) -> &str {
            "항상 매수하는 테스트 전략"
        }

        async fn initialize(
            &mut self,
            _config: Value,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.bought = false;
            Ok(())
        }

        async fn on_market_data(
            &mut self,
            data: &MarketData,
        ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
            if !self.bought {
                self.bought = true;
                Ok(vec![Signal::entry(
                    "AlwaysBuy",
                    data.ticker.clone(),
                    Side::Buy,
                )])
            } else {
                Ok(vec![])
            }
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
            Ok(())
        }

        fn get_state(&self) -> Value {
            serde_json::json!({ "bought": self.bought })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use rust_decimal_macros::dec;
    use trader_core::Timeframe;

    fn create_test_klines(count: usize, start_price: Decimal, trend: Decimal) -> Vec<Kline> {
        let ticker = "BTC/USDT".to_string();
        let base_time = Utc::now() - Duration::days(count as i64);

        (0..count)
            .map(|i| {
                let price = start_price + trend * Decimal::from(i);
                let high = price * dec!(1.01);
                let low = price * dec!(0.99);
                let open_time = base_time + Duration::hours(i as i64);
                let close_time = open_time + Duration::hours(1);

                Kline::new(
                    ticker.clone(),
                    Timeframe::H1,
                    open_time,
                    price,
                    high,
                    low,
                    price,
                    dec!(100),
                    close_time,
                )
            })
            .collect()
    }

    /// 테스트용 StrategyContext 생성 헬퍼
    fn create_test_context() -> Arc<RwLock<StrategyContext>> {
        Arc::new(RwLock::new(StrategyContext::default()))
    }

    #[test]
    fn test_config_creation() {
        let config = BacktestConfig::new(dec!(10000));
        assert_eq!(config.initial_capital, dec!(10000));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let config = BacktestConfig::new(dec!(-1000));
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_engine_creation() {
        let config = BacktestConfig::new(dec!(10000));
        let engine = BacktestEngine::new(config);
        assert_eq!(engine.balance(), dec!(10000));
        assert_eq!(engine.positions_count(), 0);
    }

    #[tokio::test]
    async fn test_backtest_empty_data() {
        let config = BacktestConfig::new(dec!(10000));
        let mut engine = BacktestEngine::new(config);
        let mut strategy = test_strategies::AlwaysBuyStrategy::new();
        let context = create_test_context();

        let result = engine.run(&mut strategy, &[], context, "BTC/USDT", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_backtest_always_buy() {
        let config = BacktestConfig::new(dec!(100000))
            .with_commission_rate(dec!(0.001))
            .with_slippage_rate(dec!(0.0005));

        let mut engine = BacktestEngine::new(config);
        let mut strategy = test_strategies::AlwaysBuyStrategy::new();
        let context = create_test_context();

        // 상승 추세 데이터
        let klines = create_test_klines(10, dec!(50000), dec!(100));

        let result = engine.run(&mut strategy, &klines, context, "BTC/USDT", None).await;
        assert!(result.is_ok());

        let report = result.unwrap();
        assert_eq!(report.data_points, 10);
        assert!(report.total_commission > Decimal::ZERO);
    }

    #[tokio::test]
    async fn test_backtest_sma_strategy() {
        let config = BacktestConfig::new(dec!(1000000))
            .with_commission_rate(dec!(0.001))
            .with_slippage_rate(dec!(0.0));

        let mut engine = BacktestEngine::new(config);
        let mut strategy = test_strategies::SimpleSmaStrategy::new(5, 20);
        let context = create_test_context();

        // 상승 후 하락 데이터
        let mut klines = create_test_klines(30, dec!(50000), dec!(100));
        klines.extend(create_test_klines(30, dec!(53000), dec!(-100)));

        // 시간 조정
        let base_time = Utc::now() - Duration::days(60);
        for (i, k) in klines.iter_mut().enumerate() {
            k.open_time = base_time + Duration::hours(i as i64);
            k.close_time = k.open_time + Duration::hours(1);
        }

        let result = engine.run(&mut strategy, &klines, context, "BTC/USDT", None).await;
        assert!(result.is_ok());

        let report = result.unwrap();
        assert_eq!(report.data_points, 60);
        println!("{}", report.summary());
    }

    #[tokio::test]
    async fn test_backtest_report() {
        let config = BacktestConfig::new(dec!(100000));
        let mut engine = BacktestEngine::new(config);
        let mut strategy = test_strategies::AlwaysBuyStrategy::new();
        let context = create_test_context();

        let klines = create_test_klines(20, dec!(50000), dec!(50));

        let result = engine.run(&mut strategy, &klines, context, "BTC/USDT", None).await.unwrap();

        // 리포트 확인
        assert!(!result.equity_curve.is_empty());
        assert!(!result.summary().is_empty());
    }

    #[test]
    fn test_default_config() {
        let config = BacktestConfig::default();
        assert_eq!(config.initial_capital, dec!(10000000));
        assert!(config.validate().is_ok());
    }
}
