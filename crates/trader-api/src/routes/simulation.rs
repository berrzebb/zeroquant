//! 동적 백테스트 (시뮬레이션) API 엔드포인트
//!
//! 정적 백테스트와 동일한 전략 로직을 실행하되, 시간 흐름에 따라 점진적으로 진행합니다.
//! speed 파라미터로 배속을 조절할 수 있습니다.
//!
//! # 핵심 원칙
//!
//! - 정적 백테스트와 동일한 전략 실행 (`on_market_data` 호출)
//! - 동일한 신호 처리 (`process_signal`)
//! - 동일한 설정에서 동일한 시점에 동일한 신호/거래 발생
//!
//! # 엔드포인트
//!
//! - `POST /api/v1/simulation/start` - 시뮬레이션 시작 (자동 전략 실행)
//! - `POST /api/v1/simulation/stop` - 시뮬레이션 중지
//! - `POST /api/v1/simulation/pause` - 일시정지/재개 토글
//! - `GET /api/v1/simulation/status` - 현재 상태 조회
//! - `GET /api/v1/simulation/positions` - 포지션 조회
//! - `GET /api/v1/simulation/trades` - 거래 내역 조회
//! - `GET /api/v1/simulation/equity` - 자산 곡선 조회
//! - `GET /api/v1/simulation/signals` - 신호 마커 조회

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use utoipa::ToSchema;
use trader_core::{
    unrealized_pnl, Kline, Side, Signal, SignalMarker, SignalType, StrategyContext, Timeframe,
};
use trader_data::cache::CachedHistoricalDataProvider;
use trader_execution::{ProcessorConfig, SignalProcessor, SimulatedExecutor};
use trader_strategy::{Strategy, StrategyRegistry};
use trader_analytics::backtest::CandleProcessor;

use crate::state::AppState;

// ==================== 시뮬레이션 상태 ====================

/// 시뮬레이션 실행 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SimulationState {
    /// 중지됨
    Stopped,
    /// 실행 중
    Running,
    /// 일시 정지
    Paused,
}

/// 시뮬레이션 포지션
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SimulationPosition {
    /// 심볼
    pub symbol: String,
    /// 표시 이름
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 방향
    pub side: String,
    /// 수량
    pub quantity: Decimal,
    /// 평균 진입가
    pub entry_price: Decimal,
    /// 현재가
    pub current_price: Decimal,
    /// 미실현 손익
    pub unrealized_pnl: Decimal,
    /// 수익률 (%)
    pub return_pct: Decimal,
    /// 진입 시간
    pub entry_time: DateTime<Utc>,
}

/// 시뮬레이션 거래 내역
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SimulationTrade {
    /// 거래 ID
    pub id: String,
    /// 심볼
    pub symbol: String,
    /// 표시 이름
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 방향 (Buy/Sell)
    pub side: String,
    /// 수량
    pub quantity: Decimal,
    /// 체결가
    pub price: Decimal,
    /// 수수료
    pub commission: Decimal,
    /// 실현 손익 (청산 거래인 경우)
    pub realized_pnl: Option<Decimal>,
    /// 거래 시간 (시뮬레이션 시간)
    pub timestamp: DateTime<Utc>,
}

/// 자산 곡선 포인트
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EquityPoint {
    /// 시간 (시뮬레이션 시간)
    pub timestamp: DateTime<Utc>,
    /// 총 자산
    pub equity: Decimal,
    /// 낙폭 (%)
    pub drawdown_pct: Decimal,
}

/// 시뮬레이션 엔진 상태
///
/// # 아키텍처
///
/// ```text
/// SimulationEngine (데이터 제공 + 진행 관리)
///        │
///        ▼
/// SimulatedExecutor (Signal 처리 - 포지션/잔고 관리)
/// ```
///
/// 포지션 관리는 SimulatedExecutor에 위임됩니다.
/// 이를 통해 백테스트/시뮬레이션/실거래가 동일한 로직을 사용합니다.
pub struct SimulationEngine {
    // === 상태 ===
    /// 현재 상태
    pub state: SimulationState,
    /// 전략 ID
    pub strategy_id: Option<String>,
    /// 전략 인스턴스 (비동기 전략 실행용)
    strategy: Option<Box<dyn Strategy>>,

    // === Signal 처리기 (포지션/잔고/거래 관리) ===
    executor: SimulatedExecutor,

    // === 자산 (초기값) ===
    /// 초기 잔고
    pub initial_balance: Decimal,

    // === 신호 기록 ===
    /// 신호 마커
    pub signal_markers: Vec<SignalMarker>,
    /// 자산 곡선
    pub equity_curve: Vec<EquityPoint>,

    // === 진행 상황 ===
    /// 로드된 캔들 데이터
    klines: Vec<Kline>,
    /// 현재 캔들 인덱스
    current_kline_index: usize,
    /// 현재 시뮬레이션 시간
    current_simulation_time: Option<DateTime<Utc>>,

    // === 설정 ===
    /// 시뮬레이션 속도 (1.0 = 1초에 1캔들)
    pub speed: f64,
    /// 수수료율 (설정 보관용 - executor에 전달)
    pub commission_rate: Decimal,
    /// 슬리피지율 (설정 보관용 - executor에 전달)
    pub slippage_rate: Decimal,

    // === 통계 ===
    /// 최고 자산 (낙폭 계산용)
    peak_equity: Decimal,

    // === 공통 캔들 처리 (BacktestEngine과 동일 로직) ===
    /// StrategyContext (전략 지표 데이터)
    context: Option<Arc<RwLock<StrategyContext>>>,
    /// 캔들 프로세서 (공통 캔들 처리 로직)
    candle_processor: Option<CandleProcessor>,
    /// 메인 티커
    ticker: Option<String>,

    // === 백그라운드 태스크 ===
    /// 실제 시작 시간
    pub started_at: Option<DateTime<Utc>>,
}

impl Default for SimulationEngine {
    fn default() -> Self {
        let initial_balance = dec!(10_000_000);
        let config = ProcessorConfig {
            commission_rate: dec!(0.001),     // 0.1%
            slippage_rate: dec!(0.0005),      // 0.05%
            max_position_size_pct: dec!(0.2), // 20%
            max_positions: 10,
            allow_short: false,
            min_strength: 0.0,
            auto_stop_loss: false,
            auto_take_profit: false,
            stop_loss_pct: dec!(0.05),
            take_profit_pct: dec!(0.10),
        };

        Self {
            state: SimulationState::Stopped,
            strategy_id: None,
            strategy: None,
            executor: SimulatedExecutor::new(config, initial_balance),
            initial_balance,
            signal_markers: Vec::new(),
            equity_curve: Vec::new(),
            klines: Vec::new(),
            current_kline_index: 0,
            current_simulation_time: None,
            speed: 1.0,
            commission_rate: dec!(0.001), // 0.1%
            slippage_rate: dec!(0.0005),  // 0.05%
            peak_equity: initial_balance,
            context: None,
            candle_processor: None,
            ticker: None,
            started_at: None,
        }
    }
}

impl SimulationEngine {
    /// 새로운 시뮬레이션 엔진 생성
    pub fn new(initial_balance: Decimal) -> Self {
        let config = ProcessorConfig {
            commission_rate: dec!(0.001),
            slippage_rate: dec!(0.0005),
            max_position_size_pct: dec!(0.2),
            max_positions: 10,
            allow_short: false,
            min_strength: 0.0,
            auto_stop_loss: false,
            auto_take_profit: false,
            stop_loss_pct: dec!(0.05),
            take_profit_pct: dec!(0.10),
        };

        Self {
            executor: SimulatedExecutor::new(config, initial_balance),
            initial_balance,
            peak_equity: initial_balance,
            ..Default::default()
        }
    }

    // === 위임 메서드 (executor에서) ===

    /// 현재 잔고 (executor에서 위임)
    pub fn current_balance(&self) -> Decimal {
        self.executor.balance()
    }

    /// 총 수수료 (executor에서 위임)
    pub fn total_commission(&self) -> Decimal {
        self.executor.total_commission()
    }

    /// 총 실현 손익 (executor에서 위임)
    pub fn total_realized_pnl(&self) -> Decimal {
        self.executor.realized_pnl()
    }

    /// 포지션 수 (executor에서 위임)
    pub fn positions_count(&self) -> usize {
        self.executor.positions().len()
    }

    /// 거래 수 (executor에서 위임)
    pub fn trades_count(&self) -> usize {
        self.executor.trades().len()
    }

    /// 미실현 손익 계산
    pub fn unrealized_pnl(&self) -> Decimal {
        // 현재 가격 맵 생성
        // 주의:
        // 1. position의 symbol(실제 ticker)을 키로 사용해야 함
        // 2. 현재 시뮬레이션 시점까지의 캔들에서만 가격 조회 (미래 가격 방지)
        let current_prices: HashMap<String, Decimal> = self
            .executor
            .positions()
            .values()
            .filter_map(|position| {
                self.klines
                    .iter()
                    .take(self.current_kline_index + 1)  // 현재 시점까지만
                    .rev()
                    .find(|k| k.ticker == position.symbol)
                    .map(|k| (position.symbol.clone(), k.close))
            })
            .collect();

        self.executor.unrealized_pnl(&current_prices)
    }

    /// 포지션을 API 응답 타입으로 변환
    pub fn get_positions(&self) -> Vec<SimulationPosition> {
        self.executor
            .positions()
            .values()
            .map(|p| {
                // 현재 시뮬레이션 시점까지의 캔들에서 가격 조회 (미래 가격 방지)
                let current_price = self
                    .klines
                    .iter()
                    .take(self.current_kline_index + 1)  // 현재 시점까지만
                    .rev()
                    .find(|k| k.ticker == p.symbol)
                    .map(|k| k.close)
                    .unwrap_or(p.entry_price);

                let side_str = match p.side {
                    Side::Buy => "Long",
                    Side::Sell => "Short",
                };

                let unrealized = unrealized_pnl(p.entry_price, current_price, p.quantity, p.side);
                let return_pct = if p.entry_price > Decimal::ZERO {
                    unrealized / (p.entry_price * p.quantity) * dec!(100)
                } else {
                    Decimal::ZERO
                };

                SimulationPosition {
                    symbol: p.symbol.clone(),
                    display_name: None,
                    side: side_str.to_string(),
                    quantity: p.quantity,
                    entry_price: p.entry_price,
                    current_price,
                    unrealized_pnl: unrealized,
                    return_pct,
                    entry_time: p.entry_time,
                }
            })
            .collect()
    }

    /// 거래 내역을 API 응답 타입으로 변환
    pub fn get_trades(&self) -> Vec<SimulationTrade> {
        self.executor
            .trades()
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let side_str = match t.side {
                    Side::Buy => "Buy",
                    Side::Sell => "Sell",
                };

                SimulationTrade {
                    id: format!("sim-{}", i),
                    symbol: t.symbol.clone(),
                    display_name: None,
                    side: side_str.to_string(),
                    quantity: t.quantity,
                    price: t.price,
                    commission: t.commission,
                    realized_pnl: t.realized_pnl,
                    timestamp: t.timestamp,
                }
            })
            .collect()
    }

    /// 시뮬레이션 초기화 (전략 + 데이터 로드)
    #[allow(clippy::too_many_arguments)]
    pub async fn initialize(
        &mut self,
        strategy_id: &str,
        parameters: Option<serde_json::Value>,
        symbols: &[String],
        start_date: &str,
        end_date: &str,
        initial_balance: Decimal,
        speed: f64,
        commission_rate: Decimal,
        slippage_rate: Decimal,
        data_provider: &CachedHistoricalDataProvider,
    ) -> Result<(), String> {
        // 1. 전략 타입 결정
        // strategy_id가 인스턴스 ID (예: "grid_936a29e6")일 경우 엔진에서 타입 조회
        // 그렇지 않으면 직접 타입으로 간주 (예: "grid")
        let strategy_type = if StrategyRegistry::find(strategy_id).is_some() {
            // strategy_id가 이미 유효한 타입이면 그대로 사용
            strategy_id.to_string()
        } else {
            // 인스턴스 ID에서 타입 추출 (형식: {type}_{uuid})
            strategy_id
                .rsplit('_')
                .skip(1)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("_")
        };

        // 2. 전략 메타 조회
        let meta = StrategyRegistry::find(&strategy_type)
            .ok_or_else(|| format!("Unknown strategy type: {} (from id: {})", strategy_type, strategy_id))?;

        // 2. 심볼 결정 (전략 초기화 전에 parameters에서 직접 추출)
        // 우선순위: 1) symbols 파라미터 → 2) parameters.ticker → 3) default_tickers
        let symbol = if !symbols.is_empty() {
            symbols[0].clone()
        } else if let Some(ticker) = parameters
            .as_ref()
            .and_then(|p| p.get("ticker"))
            .and_then(|v| v.as_str())
        {
            ticker.to_string()
        } else {
            meta.default_tickers
                .first()
                .ok_or_else(|| "심볼이 지정되지 않았습니다. 전략 설정에서 ticker를 지정하거나 symbols를 전달해주세요.".to_string())?
                .to_string()
        };

        // 3. 전략 인스턴스 생성
        let mut strategy = (meta.factory)();

        // 4. 전략 초기화 (파라미터 적용)
        // parameters가 None이면 빈 객체로 초기화 (기본값 사용)
        let init_params = parameters.unwrap_or_else(|| serde_json::json!({}));

        // =========================================================================
        // 전략 파라미터에서 Executor 설정값 추출 (하드코딩 방지)
        // =========================================================================

        // max_positions 추출 (전략 파라미터 → 기본값 10)
        let max_positions = init_params
            .get("max_positions")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            // Grid 전략은 levels 필드 사용
            .or_else(|| {
                init_params
                    .get("levels")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
            })
            // InfinityBot은 max_rounds 사용
            .or_else(|| {
                init_params
                    .get("max_rounds")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
            })
            .unwrap_or(10); // 기본값

        // exit_config에서 리스크 관리 파라미터 추출
        let exit_config = init_params.get("exit_config");

        let stop_loss_enabled = exit_config
            .and_then(|c| c.get("stop_loss_enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false); // 기본값: 비활성화

        let stop_loss_pct = exit_config
            .and_then(|c| c.get("stop_loss_pct"))
            .and_then(|v| v.as_f64())
            .map(|v| Decimal::from_f64_retain(v / 100.0).unwrap_or(dec!(0.05))) // % → 비율 변환
            .unwrap_or(dec!(0.05)); // 기본값: 5%

        let take_profit_enabled = exit_config
            .and_then(|c| c.get("take_profit_enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false); // 기본값: 비활성화

        let take_profit_pct = exit_config
            .and_then(|c| c.get("take_profit_pct"))
            .and_then(|v| v.as_f64())
            .map(|v| Decimal::from_f64_retain(v / 100.0).unwrap_or(dec!(0.10))) // % → 비율 변환
            .unwrap_or(dec!(0.10)); // 기본값: 10%

        // min_strength 추출 (Signal 필터링)
        let min_strength = init_params
            .get("min_strength")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        // allow_short 추출
        let allow_short = init_params
            .get("allow_short")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // max_position_size_pct 추출 (기본 20%)
        let max_position_size_pct = init_params
            .get("max_position_size_pct")
            .and_then(|v| v.as_f64())
            .map(|v| Decimal::from_f64_retain(v / 100.0).unwrap_or(dec!(0.2)))
            .unwrap_or(dec!(0.2));

        tracing::info!(
            max_positions = max_positions,
            max_position_size_pct = %max_position_size_pct,
            stop_loss_enabled = stop_loss_enabled,
            stop_loss_pct = %stop_loss_pct,
            take_profit_enabled = take_profit_enabled,
            take_profit_pct = %take_profit_pct,
            min_strength = min_strength,
            allow_short = allow_short,
            "전략 파라미터에서 Executor 설정 추출"
        );

        strategy
            .initialize(init_params)
            .await
            .map_err(|e| format!("전략 초기화 실패: {}", e))?;

        // 5. 캔들 데이터 로드 (공유 data_provider 사용 - Redis 3계층 캐시)
        let start = NaiveDate::parse_from_str(start_date, "%Y-%m-%d")
            .map_err(|e| format!("시작일 파싱 실패: {}", e))?;
        let end = NaiveDate::parse_from_str(end_date, "%Y-%m-%d")
            .map_err(|e| format!("종료일 파싱 실패: {}", e))?;

        // 타임프레임 (전략 기본 → 폴백: 가장 빠른 가용 데이터)
        // 우선순위: 전략 기본 → 1m → 5m → 15m → 1h → 1d
        let timeframe_priority = [
            meta.default_timeframe,
            "1m", "5m", "15m", "30m", "1h", "4h", "1d",
        ];

        let mut klines = Vec::new();
        let mut selected_timeframe = Timeframe::D1;

        for tf_str in &timeframe_priority {
            if let Ok(tf) = tf_str.parse::<Timeframe>() {
                if let Ok(data) = data_provider.get_klines_range(&symbol, tf, start, end).await {
                    if !data.is_empty() {
                        klines = data;
                        selected_timeframe = tf;
                        tracing::info!(
                            "시뮬레이션 타임프레임: {} (전략 기본: {})",
                            tf, meta.default_timeframe
                        );
                        break;
                    }
                }
            }
        }

        if klines.is_empty() {
            return Err(format!(
                "기간 {} ~ {}에 대한 {} 데이터가 없습니다 (시도한 타임프레임: {:?})",
                start_date, end_date, symbol, timeframe_priority
            ));
        }

        let _ = selected_timeframe; // 향후 사용 예정

        // 5. StrategyContext 생성 및 전략에 주입 (BacktestEngine과 동일)
        let context = Arc::new(RwLock::new(StrategyContext::default()));
        strategy.set_context(context.clone());

        // 6. 상태 초기화
        self.state = SimulationState::Running;
        self.strategy_id = Some(strategy_id.to_string());
        self.strategy = Some(strategy);
        self.initial_balance = initial_balance;
        self.context = Some(context);
        self.candle_processor = Some(CandleProcessor::new());
        self.ticker = Some(symbol.clone());
        self.peak_equity = initial_balance;
        self.signal_markers.clear();
        self.equity_curve.clear();
        self.klines = klines;
        self.current_kline_index = 0;
        self.current_simulation_time = None;
        self.speed = speed;
        self.commission_rate = commission_rate;
        self.slippage_rate = slippage_rate;
        self.started_at = Some(Utc::now());

        // Executor 설정 및 초기화 (전략 파라미터 반영)
        let config = ProcessorConfig {
            commission_rate,
            slippage_rate,
            max_position_size_pct,
            max_positions,
            allow_short,
            min_strength,
            auto_stop_loss: stop_loss_enabled,
            auto_take_profit: take_profit_enabled,
            stop_loss_pct,
            take_profit_pct,
        };
        self.executor = SimulatedExecutor::new(config, initial_balance);

        Ok(())
    }

    /// 다음 캔들 처리 (CandleProcessor를 통해 BacktestEngine과 동일한 로직 실행)
    ///
    /// CandleProcessor가 StrategyContext 업데이트, 시그널 생성, 포지션 동기화를 수행하여
    /// BacktestEngine과 동일한 결과를 보장합니다.
    pub async fn process_next_candle(&mut self) -> Result<bool, String> {
        if self.state != SimulationState::Running {
            return Ok(false);
        }

        // 현재 캔들 가져오기
        let idx = self.current_kline_index;
        let kline = match self.klines.get(idx) {
            Some(k) => k.clone(),
            None => return Ok(false), // 데이터 끝
        };

        // 현재 시뮬레이션 시간 업데이트
        self.current_simulation_time = Some(kline.close_time);

        let exchange_name = "simulation";

        // CandleProcessor, StrategyContext, Strategy 존재 확인
        let context = self.context.as_ref()
            .ok_or_else(|| "StrategyContext가 초기화되지 않았습니다".to_string())?
            .clone();
        let ticker = self.ticker.as_ref()
            .ok_or_else(|| "티커가 설정되지 않았습니다".to_string())?
            .clone();

        // 1. StrategyContext 업데이트 + 시그널 생성 (공통 로직)
        let candle_processor = self.candle_processor.as_mut()
            .ok_or_else(|| "CandleProcessor가 초기화되지 않았습니다".to_string())?;

        let historical_klines = &self.klines[..=idx];
        let strategy = self.strategy.as_mut()
            .ok_or_else(|| "전략이 초기화되지 않았습니다".to_string())?;

        let ctx = trader_analytics::ProcessCandleContext {
            idx,
            kline: &kline,
            historical_klines,
            context: &context,
            ticker: &ticker,
            exchange_name,
            screening_calculator: None,
        };

        let signals = candle_processor
            .process_candle(ctx, strategy.as_mut())
            .await
            .map_err(|e| format!("캔들 처리 오류: {}", e))?;

        // 2. 시그널 처리 (Entry 먼저, Exit 나중에)
        for signal in &signals.entry_signals {
            self.process_signal(signal, &kline).await?;
        }
        for signal in &signals.exit_signals {
            self.process_signal(signal, &kline).await?;
        }

        // 3. 포지션 동기화 (공통 로직)
        let candle_processor = self.candle_processor.as_ref()
            .ok_or_else(|| "CandleProcessor가 초기화되지 않았습니다".to_string())?;
        let strategy = self.strategy.as_mut()
            .ok_or_else(|| "전략이 초기화되지 않았습니다".to_string())?;
        candle_processor
            .sync_positions(strategy.as_mut(), self.executor.positions(), &kline, exchange_name, &ticker)
            .await
            .map_err(|e| format!("포지션 동기화 오류: {}", e))?;

        // 4. 자산 곡선 업데이트
        self.update_equity_curve(&kline);

        // 다음 캔들로 이동
        self.current_kline_index += 1;

        Ok(self.current_kline_index < self.klines.len())
    }

    /// 신호 처리 (executor에 위임)
    ///
    /// BacktestEngine과 동일한 패턴: SignalProcessor trait 구현체에 위임
    async fn process_signal(&mut self, signal: &Signal, kline: &Kline) -> Result<(), String> {
        let price = signal.suggested_price.unwrap_or(kline.close);

        // SignalMarker 저장
        let marker = SignalMarker::from_signal(signal, price, kline.open_time, &signal.strategy_id);
        self.signal_markers.push(marker);

        // Alert는 실행하지 않고 마커만 기록
        if signal.signal_type == SignalType::Alert {
            return Ok(());
        }

        // Executor에 신호 처리 위임
        let result = self
            .executor
            .process_signal(signal, price, kline.close_time)
            .await
            .map_err(|e| format!("Signal 처리 실패: {}", e))?;

        // 거래 결과 로깅 (디버그용)
        if let Some(trade) = result {
            tracing::debug!(
                "거래 체결: {} {} {} @ {}",
                trade.symbol,
                trade.side,
                trade.quantity,
                trade.price
            );
        }

        Ok(())
    }

    /// 현재 가격 맵 업데이트 (자산 계산용)
    fn update_current_prices(&self, kline: &Kline) -> HashMap<String, Decimal> {
        let mut prices = HashMap::new();
        prices.insert(kline.ticker.clone(), kline.close);
        prices
    }

    /// 자산 곡선 업데이트
    fn update_equity_curve(&mut self, kline: &Kline) {
        let current_prices = self.update_current_prices(kline);
        let equity = self.executor.total_equity(&current_prices);

        // 최고점 갱신
        if equity > self.peak_equity {
            self.peak_equity = equity;
        }

        // 낙폭 계산
        let drawdown_pct = if self.peak_equity > Decimal::ZERO {
            (self.peak_equity - equity) / self.peak_equity * dec!(100)
        } else {
            Decimal::ZERO
        };

        self.equity_curve.push(EquityPoint {
            timestamp: kline.close_time,
            equity,
            drawdown_pct,
        });
    }

    /// 총 자산 계산 (executor에 위임)
    pub fn total_equity(&self) -> Decimal {
        // 현재 가격 맵 생성 (캔들 데이터에서)
        // 주의:
        // 1. position의 symbol(실제 ticker)을 키로 사용해야 함
        // 2. 현재 시뮬레이션 시점까지의 캔들에서만 가격 조회 (미래 가격 방지)
        let current_prices: HashMap<String, Decimal> = self
            .executor
            .positions()
            .values()
            .filter_map(|position| {
                // 현재 시뮬레이션 시점까지의 캔들에서 가격 찾기
                self.klines
                    .iter()
                    .take(self.current_kline_index + 1)  // 현재 시점까지만
                    .rev()
                    .find(|k| k.ticker == position.symbol)
                    .map(|k| (position.symbol.clone(), k.close))
            })
            .collect();

        self.executor.total_equity(&current_prices)
    }

    /// 시뮬레이션 중지
    pub fn stop(&mut self) {
        self.state = SimulationState::Stopped;
    }

    /// 모든 미청산 포지션 강제 청산 (시뮬레이션 종료 시)
    ///
    /// 시뮬레이션이 완료되거나 중지될 때 남아있는 모든 포지션을
    /// 현재 가격으로 청산하여 최종 손익을 확정합니다.
    pub fn close_all_positions(&mut self) -> usize {
        // 현재 가격 맵 생성 (캔들 데이터에서)
        // 주의: position의 symbol(실제 ticker)을 키로 사용해야 함
        let current_prices: HashMap<String, Decimal> = self
            .executor
            .positions()
            .values()  // keys() 대신 values() 사용
            .filter_map(|position| {
                self.klines
                    .iter()
                    .rev()
                    .find(|k| k.ticker == position.symbol)  // 실제 ticker로 검색
                    .map(|k| (position.symbol.clone(), k.close))
            })
            .collect();

        // 현재 시뮬레이션 시간 (없으면 마지막 캔들 시간)
        let timestamp = self.current_simulation_time.unwrap_or_else(|| {
            self.klines.last().map(|k| k.close_time).unwrap_or_else(Utc::now)
        });

        // 모든 포지션 청산
        let results = self.executor.close_all_positions(&current_prices, timestamp);
        let closed_count = results.len();

        // 청산 거래 로깅
        for trade in &results {
            tracing::info!(
                "미청산 포지션 청산: {} {} {} @ {} (PnL: {:?})",
                trade.symbol,
                trade.side,
                trade.quantity,
                trade.price,
                trade.realized_pnl
            );
        }

        if closed_count > 0 {
            tracing::info!("총 {} 개 미청산 포지션 청산 완료", closed_count);
        }

        closed_count
    }

    /// 일시 정지
    pub fn pause(&mut self) {
        if self.state == SimulationState::Running {
            self.state = SimulationState::Paused;
        }
    }

    /// 재개
    pub fn resume(&mut self) {
        if self.state == SimulationState::Paused {
            self.state = SimulationState::Running;
        }
    }

    /// 진행률 (%)
    pub fn progress_pct(&self) -> f64 {
        if self.klines.is_empty() {
            return 0.0;
        }
        (self.current_kline_index as f64 / self.klines.len() as f64) * 100.0
    }
}

/// 공유 가능한 시뮬레이션 엔진 타입
pub type SharedSimulationEngine = Arc<RwLock<SimulationEngine>>;

/// 새로운 공유 시뮬레이션 엔진 생성
pub fn create_simulation_engine() -> SharedSimulationEngine {
    Arc::new(RwLock::new(SimulationEngine::default()))
}

// ==================== 요청/응답 타입 ====================

/// 시뮬레이션 시작 요청
#[derive(Debug, Deserialize, ToSchema)]
pub struct SimulationStartRequest {
    /// 전략 ID
    pub strategy_id: String,
    /// 전략 파라미터 (JSON)
    #[serde(default)]
    pub parameters: Option<serde_json::Value>,
    /// 심볼 목록 (비어있으면 전략 기본 심볼 사용)
    #[serde(default)]
    pub symbols: Vec<String>,
    /// 초기 잔고
    #[serde(default = "default_initial_balance")]
    pub initial_balance: Decimal,
    /// 시뮬레이션 속도 (1.0 = 1초에 1캔들)
    #[serde(default = "default_speed")]
    pub speed: f64,
    /// 시작 날짜 (YYYY-MM-DD)
    pub start_date: String,
    /// 종료 날짜 (YYYY-MM-DD)
    pub end_date: String,
    /// 수수료율 (기본 0.001 = 0.1%)
    #[serde(default = "default_commission_rate")]
    pub commission_rate: Decimal,
    /// 슬리피지율 (기본 0.0005 = 0.05%)
    #[serde(default = "default_slippage_rate")]
    pub slippage_rate: Decimal,
}

fn default_initial_balance() -> Decimal {
    dec!(10_000_000)
}

fn default_speed() -> f64 {
    1.0
}

fn default_commission_rate() -> Decimal {
    dec!(0.001)
}

fn default_slippage_rate() -> Decimal {
    dec!(0.0005)
}

/// 시뮬레이션 시작 응답
#[derive(Debug, Serialize, ToSchema)]
pub struct SimulationStartResponse {
    /// 성공 여부
    pub success: bool,
    /// 메시지
    pub message: String,
    /// 시작 시간
    pub started_at: DateTime<Utc>,
    /// 총 캔들 수
    pub total_candles: usize,
}

/// 시뮬레이션 중지 응답
#[derive(Debug, Serialize, ToSchema)]
pub struct SimulationStopResponse {
    /// 성공 여부
    pub success: bool,
    /// 메시지
    pub message: String,
    /// 최종 자산
    pub final_equity: Decimal,
    /// 총 수익률 (%)
    pub total_return_pct: Decimal,
    /// 총 거래 횟수
    pub total_trades: usize,
}

/// 시뮬레이션 상태 응답
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SimulationStatusResponse {
    /// 현재 상태
    pub state: SimulationState,
    /// 전략 ID
    pub strategy_id: Option<String>,
    /// 초기 잔고
    pub initial_balance: Decimal,
    /// 현재 잔고
    pub current_balance: Decimal,
    /// 총 자산
    pub total_equity: Decimal,
    /// 미실현 손익
    pub unrealized_pnl: Decimal,
    /// 실현 손익
    pub realized_pnl: Decimal,
    /// 수익률 (%)
    pub return_pct: Decimal,
    /// 포지션 수
    pub position_count: usize,
    /// 거래 횟수
    pub trade_count: usize,
    /// 실제 시작 시간
    pub started_at: Option<DateTime<Utc>>,
    /// 시뮬레이션 속도
    pub speed: f64,
    /// 현재 시뮬레이션 시간
    pub current_simulation_time: Option<DateTime<Utc>>,
    /// 진행률 (%)
    pub progress_pct: f64,
    /// 현재 캔들 인덱스
    pub current_candle_index: usize,
    /// 총 캔들 수
    pub total_candles: usize,
}

/// 포지션 목록 응답
#[derive(Debug, Serialize, ToSchema)]
pub struct SimulationPositionsResponse {
    /// 포지션 목록
    pub positions: Vec<SimulationPosition>,
    /// 총 미실현 손익
    pub total_unrealized_pnl: Decimal,
}

/// 거래 내역 응답
#[derive(Debug, Serialize, ToSchema)]
pub struct SimulationTradesResponse {
    /// 거래 목록
    pub trades: Vec<SimulationTrade>,
    /// 총 거래 수
    pub total: usize,
    /// 총 실현 손익
    pub total_realized_pnl: Decimal,
    /// 총 수수료
    pub total_commission: Decimal,
}

/// 자산 곡선 응답
#[derive(Debug, Serialize, ToSchema)]
pub struct SimulationEquityResponse {
    /// 자산 곡선
    pub equity_curve: Vec<EquityPoint>,
    /// 최대 낙폭 (%)
    pub max_drawdown_pct: Decimal,
}

/// 신호 마커 응답
#[derive(Debug, Serialize, ToSchema)]
pub struct SimulationSignalsResponse {
    /// 신호 마커 목록
    pub signals: Vec<SignalMarker>,
    /// 총 신호 수
    pub total: usize,
}

/// API 에러 응답
#[derive(Debug, Serialize, ToSchema)]
pub struct SimulationApiError {
    /// 에러 코드
    pub code: String,
    /// 에러 메시지
    pub message: String,
}

impl SimulationApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

// ==================== 전역 시뮬레이션 엔진 ====================

lazy_static::lazy_static! {
    /// 전역 시뮬레이션 엔진
    static ref SIMULATION_ENGINE: SharedSimulationEngine = create_simulation_engine();
    /// 백그라운드 러너 핸들
    static ref RUNNER_HANDLE: Arc<RwLock<Option<JoinHandle<()>>>> = Arc::new(RwLock::new(None));
}

// ==================== 백그라운드 러너 ====================

/// 시뮬레이션 백그라운드 러너
async fn simulation_runner(engine: SharedSimulationEngine, speed: f64) {
    // 1초에 처리할 캔들 수 = speed
    // 따라서 캔들당 대기 시간 = 1/speed 초
    let delay_per_candle = std::time::Duration::from_secs_f64(1.0 / speed);

    loop {
        // 다음 캔들 처리
        let should_continue = {
            let mut engine = engine.write().await;

            // 일시정지 상태면 대기
            if engine.state == SimulationState::Paused {
                drop(engine);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }

            // 중지 상태면 종료
            if engine.state == SimulationState::Stopped {
                break;
            }

            // 다음 캔들 처리
            match engine.process_next_candle().await {
                Ok(has_more) => has_more,
                Err(e) => {
                    tracing::error!("시뮬레이션 오류: {}", e);
                    engine.stop();
                    false
                }
            }
        };

        if !should_continue {
            // 시뮬레이션 완료
            let mut engine = engine.write().await;

            // 미청산 포지션 모두 청산 (최종 손익 확정)
            let closed_count = engine.close_all_positions();
            if closed_count > 0 {
                tracing::info!("시뮬레이션 종료 전 {} 개 포지션 청산", closed_count);
            }

            engine.stop();
            tracing::info!("시뮬레이션 완료");
            break;
        }

        // 속도에 따른 대기
        tokio::time::sleep(delay_per_candle).await;
    }
}

// ==================== 핸들러 ====================

/// 시뮬레이션 시작
///
/// POST /api/v1/simulation/start
#[utoipa::path(
    post,
    path = "/api/v1/simulation/start",
    tag = "simulation",
    request_body = SimulationStartRequest,
    responses(
        (status = 200, description = "시뮬레이션 시작 성공", body = SimulationStartResponse),
        (status = 400, description = "잘못된 요청", body = SimulationApiError),
        (status = 409, description = "이미 실행 중", body = SimulationApiError),
        (status = 503, description = "데이터 제공자 미연결", body = SimulationApiError),
    )
)]
pub async fn start_simulation(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SimulationStartRequest>,
) -> Result<Json<SimulationStartResponse>, (StatusCode, Json<SimulationApiError>)> {
    // 입력 검증
    if request.speed <= 0.0 || request.speed > 1000.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(SimulationApiError::new(
                "INVALID_SPEED",
                "속도는 0.1 ~ 1000 사이여야 합니다",
            )),
        ));
    }

    // 기존 러너 중지
    {
        let mut handle = RUNNER_HANDLE.write().await;
        if let Some(h) = handle.take() {
            h.abort();
        }
    }

    // 엔진 초기화
    let (started_at, total_candles) = {
        let mut engine = SIMULATION_ENGINE.write().await;

        // 이미 실행 중인지 확인
        if engine.state == SimulationState::Running {
            return Err((
                StatusCode::CONFLICT,
                Json(SimulationApiError::new(
                    "ALREADY_RUNNING",
                    "시뮬레이션이 이미 실행 중입니다. 먼저 중지해주세요.",
                )),
            ));
        }

        // 공유 data_provider 확인 (Redis 3계층 캐시 포함)
        let data_provider = state.data_provider.as_ref().ok_or_else(|| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(SimulationApiError::new(
                    "DATA_PROVIDER_UNAVAILABLE",
                    "Data provider가 연결되어 있지 않습니다",
                )),
            )
        })?;

        // 전략 파라미터 결정:
        // 1. request.parameters가 있으면 사용
        // 2. 없으면 strategy_engine에서 조회
        let parameters = if request.parameters.is_some() {
            request.parameters.clone()
        } else {
            // 전략 엔진에서 설정 조회 시도
            let strategy_engine = state.strategy_engine.read().await;
            match strategy_engine.get_strategy_config(&request.strategy_id).await {
                Ok(config) => {
                    tracing::info!(
                        strategy_id = %request.strategy_id,
                        "엔진에서 전략 설정 조회 성공"
                    );
                    Some(config)
                }
                Err(_) => {
                    tracing::debug!(
                        strategy_id = %request.strategy_id,
                        "엔진에 전략 설정 없음, 기본값 사용"
                    );
                    None
                }
            }
        };

        // 시뮬레이션 초기화
        engine
            .initialize(
                &request.strategy_id,
                parameters,
                &request.symbols,
                &request.start_date,
                &request.end_date,
                request.initial_balance,
                request.speed,
                request.commission_rate,
                request.slippage_rate,
                data_provider,
            )
            .await
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(SimulationApiError::new("INIT_FAILED", e)),
                )
            })?;

        let started_at = engine.started_at.unwrap_or_else(Utc::now);
        let total_candles = engine.klines.len();

        (started_at, total_candles)
    };

    // 백그라운드 러너 시작
    let engine_clone = SIMULATION_ENGINE.clone();
    let handle = tokio::spawn(simulation_runner(engine_clone, request.speed));

    {
        let mut runner = RUNNER_HANDLE.write().await;
        *runner = Some(handle);
    }

    Ok(Json(SimulationStartResponse {
        success: true,
        message: format!("시뮬레이션이 시작되었습니다 ({} 캔들)", total_candles),
        started_at,
        total_candles,
    }))
}

/// 시뮬레이션 중지
///
/// POST /api/v1/simulation/stop
#[utoipa::path(
    post,
    path = "/api/v1/simulation/stop",
    tag = "simulation",
    responses(
        (status = 200, description = "시뮬레이션 중지 성공", body = SimulationStopResponse),
        (status = 400, description = "실행 중이 아님", body = SimulationApiError),
    )
)]
pub async fn stop_simulation(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<SimulationStopResponse>, (StatusCode, Json<SimulationApiError>)> {
    // 러너 중지
    {
        let mut handle = RUNNER_HANDLE.write().await;
        if let Some(h) = handle.take() {
            h.abort();
        }
    }

    // 엔진 중지 및 결과 추출
    let (final_equity, initial_balance, total_trades, closed_positions) = {
        let mut engine = SIMULATION_ENGINE.write().await;

        if engine.state == SimulationState::Stopped && engine.started_at.is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(SimulationApiError::new(
                    "NOT_RUNNING",
                    "시뮬레이션이 실행 중이 아닙니다",
                )),
            ));
        }

        // 미청산 포지션 모두 청산 (최종 손익 확정)
        let closed_count = engine.close_all_positions();

        let final_equity = engine.total_equity();
        let initial_balance = engine.initial_balance;
        let total_trades = engine.trades_count();
        engine.stop();

        (final_equity, initial_balance, total_trades, closed_count)
    };

    if closed_positions > 0 {
        tracing::info!("시뮬레이션 중지 시 {} 개 포지션 청산", closed_positions);
    }

    let total_return_pct = if initial_balance > Decimal::ZERO {
        (final_equity - initial_balance) / initial_balance * dec!(100)
    } else {
        Decimal::ZERO
    };

    Ok(Json(SimulationStopResponse {
        success: true,
        message: "시뮬레이션이 중지되었습니다".to_string(),
        final_equity,
        total_return_pct,
        total_trades,
    }))
}

/// 시뮬레이션 일시정지/재개
///
/// POST /api/v1/simulation/pause
#[utoipa::path(
    post,
    path = "/api/v1/simulation/pause",
    tag = "simulation",
    responses(
        (status = 200, description = "일시정지/재개 토글 성공", body = inline(serde_json::Value)),
    )
)]
pub async fn pause_simulation(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut engine = SIMULATION_ENGINE.write().await;

    match engine.state {
        SimulationState::Running => {
            engine.pause();
            Json(serde_json::json!({
                "success": true,
                "state": "paused",
                "message": "시뮬레이션이 일시정지되었습니다"
            }))
        }
        SimulationState::Paused => {
            engine.resume();
            Json(serde_json::json!({
                "success": true,
                "state": "running",
                "message": "시뮬레이션이 재개되었습니다"
            }))
        }
        SimulationState::Stopped => Json(serde_json::json!({
            "success": false,
            "state": "stopped",
            "message": "시뮬레이션이 실행 중이 아닙니다"
        })),
    }
}

/// 시뮬레이션 상태 조회
///
/// GET /api/v1/simulation/status
#[utoipa::path(
    get,
    path = "/api/v1/simulation/status",
    tag = "simulation",
    responses(
        (status = 200, description = "시뮬레이션 현재 상태", body = SimulationStatusResponse),
    )
)]
pub async fn get_simulation_status(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let engine = SIMULATION_ENGINE.read().await;

    let total_equity = engine.total_equity();
    let unrealized = engine.unrealized_pnl();

    let return_pct = if engine.initial_balance > Decimal::ZERO {
        (total_equity - engine.initial_balance) / engine.initial_balance * dec!(100)
    } else {
        Decimal::ZERO
    };

    Json(SimulationStatusResponse {
        state: engine.state,
        strategy_id: engine.strategy_id.clone(),
        initial_balance: engine.initial_balance,
        current_balance: engine.current_balance(),
        total_equity,
        unrealized_pnl: unrealized,
        realized_pnl: engine.total_realized_pnl(),
        return_pct,
        position_count: engine.positions_count(),
        trade_count: engine.trades_count(),
        started_at: engine.started_at,
        speed: engine.speed,
        current_simulation_time: engine.current_simulation_time,
        progress_pct: engine.progress_pct(),
        current_candle_index: engine.current_kline_index,
        total_candles: engine.klines.len(),
    })
}

/// 포지션 목록 조회
///
/// GET /api/v1/simulation/positions
#[utoipa::path(
    get,
    path = "/api/v1/simulation/positions",
    tag = "simulation",
    responses(
        (status = 200, description = "시뮬레이션 포지션 목록", body = SimulationPositionsResponse),
    )
)]
pub async fn get_simulation_positions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let engine = SIMULATION_ENGINE.read().await;

    let mut positions = engine.get_positions();
    let total_unrealized_pnl: Decimal = positions.iter().map(|p| p.unrealized_pnl).sum();

    // display_name 설정
    let symbols: Vec<String> = positions.iter().map(|p| p.symbol.clone()).collect();
    let display_names = state.get_display_names(&symbols, false).await;
    for pos in positions.iter_mut() {
        if let Some(name) = display_names.get(&pos.symbol) {
            pos.display_name = Some(name.clone());
        }
    }

    Json(SimulationPositionsResponse {
        positions,
        total_unrealized_pnl,
    })
}

/// 거래 내역 조회
///
/// GET /api/v1/simulation/trades
#[utoipa::path(
    get,
    path = "/api/v1/simulation/trades",
    tag = "simulation",
    responses(
        (status = 200, description = "시뮬레이션 거래 내역", body = SimulationTradesResponse),
    )
)]
pub async fn get_simulation_trades(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let engine = SIMULATION_ENGINE.read().await;

    let mut trades = engine.get_trades();
    let total = trades.len();

    // display_name 설정
    let symbols: Vec<String> = trades.iter().map(|t| t.symbol.clone()).collect();
    let display_names = state.get_display_names(&symbols, false).await;
    for trade in trades.iter_mut() {
        if let Some(name) = display_names.get(&trade.symbol) {
            trade.display_name = Some(name.clone());
        }
    }

    Json(SimulationTradesResponse {
        trades,
        total,
        total_realized_pnl: engine.total_realized_pnl(),
        total_commission: engine.total_commission(),
    })
}

/// 자산 곡선 조회
///
/// GET /api/v1/simulation/equity
#[utoipa::path(
    get,
    path = "/api/v1/simulation/equity",
    tag = "simulation",
    responses(
        (status = 200, description = "시뮬레이션 자산 곡선", body = SimulationEquityResponse),
    )
)]
pub async fn get_simulation_equity(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let engine = SIMULATION_ENGINE.read().await;

    let max_drawdown_pct = engine
        .equity_curve
        .iter()
        .map(|e| e.drawdown_pct)
        .max()
        .unwrap_or(Decimal::ZERO);

    Json(SimulationEquityResponse {
        equity_curve: engine.equity_curve.clone(),
        max_drawdown_pct,
    })
}

/// 신호 마커 조회
///
/// GET /api/v1/simulation/signals
#[utoipa::path(
    get,
    path = "/api/v1/simulation/signals",
    tag = "simulation",
    responses(
        (status = 200, description = "시뮬레이션 신호 마커 목록", body = SimulationSignalsResponse),
    )
)]
pub async fn get_simulation_signals(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let engine = SIMULATION_ENGINE.read().await;

    Json(SimulationSignalsResponse {
        signals: engine.signal_markers.clone(),
        total: engine.signal_markers.len(),
    })
}

/// 시뮬레이션 리셋
///
/// POST /api/v1/simulation/reset
#[utoipa::path(
    post,
    path = "/api/v1/simulation/reset",
    tag = "simulation",
    responses(
        (status = 200, description = "시뮬레이션 초기화 성공", body = inline(serde_json::Value)),
    )
)]
pub async fn reset_simulation(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    // 러너 중지
    {
        let mut handle = RUNNER_HANDLE.write().await;
        if let Some(h) = handle.take() {
            h.abort();
        }
    }

    // 엔진 리셋
    let mut engine = SIMULATION_ENGINE.write().await;
    *engine = SimulationEngine::default();

    Json(serde_json::json!({
        "success": true,
        "message": "시뮬레이션이 초기화되었습니다"
    }))
}

// ==================== 라우터 ====================

/// 시뮬레이션 라우터 생성
pub fn simulation_router() -> Router<Arc<AppState>> {
    Router::new()
        // 시뮬레이션 제어
        .route("/start", post(start_simulation))
        .route("/stop", post(stop_simulation))
        .route("/pause", post(pause_simulation))
        .route("/reset", post(reset_simulation))
        // 상태 조회
        .route("/status", get(get_simulation_status))
        .route("/positions", get(get_simulation_positions))
        .route("/trades", get(get_simulation_trades))
        .route("/equity", get(get_simulation_equity))
        .route("/signals", get(get_simulation_signals))
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simulation_engine_default() {
        let engine = SimulationEngine::default();
        assert_eq!(engine.state, SimulationState::Stopped);
        assert_eq!(engine.initial_balance, dec!(10_000_000));
        assert_eq!(engine.current_balance(), dec!(10_000_000));
    }

    #[tokio::test]
    async fn test_simulation_engine_new() {
        let engine = SimulationEngine::new(dec!(5_000_000));
        assert_eq!(engine.initial_balance, dec!(5_000_000));
        assert_eq!(engine.current_balance(), dec!(5_000_000));
        assert_eq!(engine.peak_equity, dec!(5_000_000));
    }

    #[test]
    fn test_simulation_api_error() {
        let error = SimulationApiError::new("TEST_ERROR", "테스트 에러");
        assert_eq!(error.code, "TEST_ERROR");
        assert_eq!(error.message, "테스트 에러");
    }
}
