//! 전략 실행 엔진.
//!
//! 엔진은 전략 생명주기를 관리하고, 시장 데이터를 전략에 라우팅하며,
//! 전략으로부터 트레이딩 신호를 수집합니다.

use crate::Strategy;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, warn};
use trader_core::{
    domain::{MarketDataType, SignalConflictError, StrategyContext},
    Kline, MarketData, Order, Position, Signal, SignalType, Timeframe,
};

/// Signal 충돌 이벤트.
///
/// 전략이 발행한 Signal이 충돌로 인해 필터링되었을 때 발생합니다.
/// WebSocket이나 로깅을 통해 사용자에게 알림을 보낼 수 있습니다.
#[derive(Debug, Clone, Serialize)]
pub struct SignalConflictEvent {
    /// 전략 ID
    pub strategy_id: String,
    /// 티커
    pub ticker: String,
    /// 신호 유형
    pub signal_type: SignalType,
    /// 충돌 유형 (에러 문자열)
    pub conflict_type: String,
    /// 충돌 메시지
    pub message: String,
    /// 타임스탬프
    pub timestamp: DateTime<Utc>,
}

impl SignalConflictEvent {
    /// SignalConflictError에서 이벤트 생성.
    pub fn from_error(
        strategy_id: &str,
        signal: &Signal,
        error: &SignalConflictError,
    ) -> Self {
        let conflict_type = match error {
            SignalConflictError::PendingOrderExists { .. } => "pending_order",
            SignalConflictError::DuplicatePosition { .. } => "duplicate_position",
            SignalConflictError::InsufficientBalance { .. } => "insufficient_balance",
            SignalConflictError::NoPositionToExit { .. } => "no_position",
        };

        Self {
            strategy_id: strategy_id.to_string(),
            ticker: signal.ticker.clone(),
            signal_type: signal.signal_type.clone(),
            conflict_type: conflict_type.to_string(),
            message: error.to_string(),
            timestamp: Utc::now(),
        }
    }
}

/// 전략 엔진 에러.
#[derive(Error, Debug)]
pub enum EngineError {
    #[error("전략을 찾을 수 없음: {0}")]
    StrategyNotFound(String),

    #[error("전략이 이미 존재함: {0}")]
    StrategyAlreadyExists(String),

    #[error("전략 초기화 실패: {0}")]
    InitializationFailed(String),

    #[error("전략이 실행 중이 아님: {0}")]
    NotRunning(String),

    #[error("전략이 이미 실행 중: {0}")]
    AlreadyRunning(String),

    #[error("채널 에러: {0}")]
    ChannelError(String),

    #[error("내부 에러: {0}")]
    InternalError(String),
}

/// 전략 인스턴스 래퍼.
pub struct StrategyInstance {
    /// 전략 구현체
    strategy: Box<dyn Strategy>,
    /// 전략 설정
    config: Value,
    /// 전략 실행 중 여부
    running: bool,
    /// 전략 통계
    stats: StrategyStats,
    /// 사용자 지정 이름 (없으면 전략 기본 이름 사용)
    custom_name: Option<String>,
    /// 전략 컨텍스트 (다중 타임프레임 데이터 등)
    context: Arc<RwLock<StrategyContext>>,
}

/// 전략 통계.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StrategyStats {
    /// 생성된 신호 수
    pub signals_generated: u64,
    /// 체결된 주문 수
    pub orders_filled: u64,
    /// 처리된 시장 데이터 이벤트 수
    pub market_data_processed: u64,
    /// 마지막 신호 시간
    pub last_signal_time: Option<DateTime<Utc>>,
    /// 마지막 에러 메시지
    pub last_error: Option<String>,
    /// 전략 시작 시간
    pub started_at: Option<DateTime<Utc>>,
    /// 총 실행 시간(초)
    pub total_runtime_secs: u64,
}

/// 전략 상태.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyStatus {
    /// 전략 이름
    pub name: String,
    /// 전략 버전
    pub version: String,
    /// 전략 설명
    pub description: String,
    /// 전략 실행 중 여부
    pub running: bool,
    /// 전략 통계
    pub stats: StrategyStats,
    /// 현재 전략 상태
    pub state: Value,
}

/// 엔진 설정.
#[derive(Debug, Clone, Deserialize)]
pub struct EngineConfig {
    /// 최대 동시 전략 수
    #[serde(default = "default_max_strategies")]
    pub max_strategies: usize,

    /// 신호 버퍼 크기
    #[serde(default = "default_signal_buffer")]
    pub signal_buffer_size: usize,

    /// 시장 데이터 브로드캐스트 버퍼 크기
    #[serde(default = "default_broadcast_buffer")]
    pub broadcast_buffer_size: usize,

    /// 신호 중복 제거 활성화
    #[serde(default = "default_true")]
    pub deduplicate_signals: bool,

    /// 신호 중복 제거 윈도우(밀리초)
    #[serde(default = "default_dedup_window")]
    pub dedup_window_ms: u64,
}

fn default_max_strategies() -> usize {
    usize::MAX // 전략 수 제한 없음
}
fn default_signal_buffer() -> usize {
    1000
}
fn default_broadcast_buffer() -> usize {
    1000
}
fn default_true() -> bool {
    true
}
fn default_dedup_window() -> u64 {
    1000
}

/// 전략 config JSON에서 관심 종목(ticker) 목록을 추출.
///
/// 다양한 전략 config 형식을 지원합니다:
/// - `ticker`: 단일 종목 (예: "005930")
/// - `tickers`: 고정 종목 배열 (예: ["005930", "035420"])
/// - `universe`: 유니버스 배열 (예: ["XLK", "XLF", ...])
/// - `symbols`: 심볼 배열 (일부 전략에서 사용)
pub fn extract_tickers_from_config(config: &Value) -> Vec<String> {
    let mut tickers = Vec::new();

    // 단일 ticker
    if let Some(ticker) = config.get("ticker").and_then(|v| v.as_str()) {
        if !ticker.is_empty() {
            tickers.push(ticker.to_string());
        }
    }

    // 복수 tickers 배열
    if let Some(arr) = config.get("tickers").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(t) = item.as_str() {
                if !t.is_empty() && !tickers.contains(&t.to_string()) {
                    tickers.push(t.to_string());
                }
            }
        }
    }

    // 유니버스 배열 (로테이션/자산배분 전략)
    if let Some(arr) = config.get("universe").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(t) = item.as_str() {
                if !t.is_empty() && !tickers.contains(&t.to_string()) {
                    tickers.push(t.to_string());
                }
            }
        }
    }

    // symbols 배열 (일부 전략)
    if let Some(arr) = config.get("symbols").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(t) = item.as_str() {
                if !t.is_empty() && !tickers.contains(&t.to_string()) {
                    tickers.push(t.to_string());
                }
            }
        }
    }

    tickers
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_strategies: default_max_strategies(),
            signal_buffer_size: default_signal_buffer(),
            broadcast_buffer_size: default_broadcast_buffer(),
            deduplicate_signals: default_true(),
            dedup_window_ms: default_dedup_window(),
        }
    }
}

/// 전략 실행 엔진.
///
/// 다수의 트레이딩 전략을 관리하고, 시장 데이터를 라우팅하며, 신호를 수집합니다.
pub struct StrategyEngine {
    /// 엔진 설정
    config: EngineConfig,

    /// 등록된 전략들
    strategies: Arc<RwLock<HashMap<String, StrategyInstance>>>,

    /// 시장 데이터 브로드캐스터
    market_data_tx: broadcast::Sender<MarketData>,

    /// 신호 출력 채널
    signal_tx: mpsc::Sender<Signal>,

    /// 신호 수신기 (소비자용)
    signal_rx: Option<mpsc::Receiver<Signal>>,

    /// 충돌 이벤트 출력 채널
    conflict_tx: mpsc::Sender<SignalConflictEvent>,

    /// 충돌 이벤트 수신기 (소비자용)
    conflict_rx: Option<mpsc::Receiver<SignalConflictEvent>>,

    /// 엔진 실행 상태
    running: Arc<RwLock<bool>>,

    /// 중복 제거를 위한 최근 신호 (signal_id -> timestamp)
    recent_signals: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
}

impl StrategyEngine {
    /// 새 전략 엔진 생성.
    pub fn new(config: EngineConfig) -> Self {
        let (market_data_tx, _) = broadcast::channel(config.broadcast_buffer_size);
        let (signal_tx, signal_rx) = mpsc::channel(config.signal_buffer_size);
        let (conflict_tx, conflict_rx) = mpsc::channel(100); // 충돌 이벤트 버퍼

        Self {
            config,
            strategies: Arc::new(RwLock::new(HashMap::new())),
            market_data_tx,
            signal_tx,
            signal_rx: Some(signal_rx),
            conflict_tx,
            conflict_rx: Some(conflict_rx),
            running: Arc::new(RwLock::new(false)),
            recent_signals: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 신호 수신기 가져오기 (한 번만 호출 가능).
    pub fn take_signal_receiver(&mut self) -> Option<mpsc::Receiver<Signal>> {
        self.signal_rx.take()
    }

    /// 충돌 이벤트 수신기 가져오기 (한 번만 호출 가능).
    pub fn take_conflict_receiver(&mut self) -> Option<mpsc::Receiver<SignalConflictEvent>> {
        self.conflict_rx.take()
    }

    /// 엔진에 데이터를 공급하기 위한 시장 데이터 전송기 반환.
    pub fn market_data_sender(&self) -> broadcast::Sender<MarketData> {
        self.market_data_tx.clone()
    }

    /// 전략 등록.
    ///
    /// # Arguments
    /// * `id` - 전략 고유 ID
    /// * `strategy` - 전략 구현체
    /// * `config` - 전략 설정 (JSON)
    /// * `custom_name` - 사용자 지정 이름 (없으면 전략 기본 이름 사용)
    /// * `shared_context` - 공유 StrategyContext (None이면 독립 컨텍스트 생성)
    pub async fn register_strategy(
        &self,
        id: impl Into<String>,
        mut strategy: Box<dyn Strategy>,
        config: Value,
        custom_name: Option<String>,
        shared_context: Option<Arc<RwLock<StrategyContext>>>,
    ) -> Result<(), EngineError> {
        let id = id.into();
        let mut strategies = self.strategies.write().await;

        if strategies.len() >= self.config.max_strategies {
            return Err(EngineError::InternalError(format!(
                "Maximum number of strategies ({}) reached",
                self.config.max_strategies
            )));
        }

        if strategies.contains_key(&id) {
            return Err(EngineError::StrategyAlreadyExists(id));
        }

        let display_name = custom_name
            .clone()
            .unwrap_or_else(|| strategy.name().to_string());

        info!(
            strategy_id = %id,
            strategy_name = %display_name,
            "Registering strategy"
        );

        // 공유 컨텍스트가 있으면 사용, 없으면 독립 컨텍스트 생성
        let context = shared_context
            .unwrap_or_else(|| Arc::new(RwLock::new(StrategyContext::default())));
        strategy.set_context(Arc::clone(&context));

        strategies.insert(
            id,
            StrategyInstance {
                strategy,
                config,
                running: false,
                stats: StrategyStats::default(),
                custom_name,
                context,
            },
        );

        Ok(())
    }

    /// 전략 등록 해제.
    pub async fn unregister_strategy(&self, id: &str) -> Result<(), EngineError> {
        let mut strategies = self.strategies.write().await;

        if let Some(instance) = strategies.get(id) {
            if instance.running {
                return Err(EngineError::AlreadyRunning(format!(
                    "Cannot unregister running strategy: {}",
                    id
                )));
            }
        }

        strategies
            .remove(id)
            .ok_or_else(|| EngineError::StrategyNotFound(id.to_string()))?;

        info!(strategy_id = %id, "Unregistered strategy");
        Ok(())
    }

    /// 전략 시작.
    ///
    /// 전략 초기화 후, config에서 관심 종목(ticker/tickers/universe)을 추출하여
    /// 공유 StrategyContext의 watched_tickers에 등록합니다.
    pub async fn start_strategy(&self, id: &str) -> Result<(), EngineError> {
        let mut strategies = self.strategies.write().await;

        let instance = strategies
            .get_mut(id)
            .ok_or_else(|| EngineError::StrategyNotFound(id.to_string()))?;

        if instance.running {
            return Err(EngineError::AlreadyRunning(id.to_string()));
        }

        // 전략 초기화
        instance
            .strategy
            .initialize(instance.config.clone())
            .await
            .map_err(|e| EngineError::InitializationFailed(e.to_string()))?;

        // config에서 관심 종목 추출 → watched_tickers에 등록
        let tickers = extract_tickers_from_config(&instance.config);
        if !tickers.is_empty() {
            let mut ctx = instance.context.write().await;
            ctx.add_watched_tickers(&tickers);
            info!(
                strategy_id = %id,
                ticker_count = tickers.len(),
                "전략 관심 종목 등록 완료"
            );
        }

        instance.running = true;
        instance.stats.started_at = Some(Utc::now());

        info!(
            strategy_id = %id,
            strategy_name = instance.strategy.name(),
            "Started strategy"
        );

        Ok(())
    }

    /// 전략 중지.
    pub async fn stop_strategy(&self, id: &str) -> Result<(), EngineError> {
        let mut strategies = self.strategies.write().await;

        let instance = strategies
            .get_mut(id)
            .ok_or_else(|| EngineError::StrategyNotFound(id.to_string()))?;

        if !instance.running {
            return Err(EngineError::NotRunning(id.to_string()));
        }

        // 전략 종료
        if let Err(e) = instance.strategy.shutdown().await {
            warn!(
                strategy_id = %id,
                error = %e,
                "Error during strategy shutdown"
            );
        }

        instance.running = false;

        // 실행 시간 업데이트
        if let Some(started) = instance.stats.started_at {
            let runtime = Utc::now().signed_duration_since(started);
            instance.stats.total_runtime_secs += runtime.num_seconds() as u64;
        }

        info!(strategy_id = %id, "Stopped strategy");
        Ok(())
    }

    /// 전략 상태 조회.
    pub async fn get_strategy_status(&self, id: &str) -> Result<StrategyStatus, EngineError> {
        let strategies = self.strategies.read().await;

        let instance = strategies
            .get(id)
            .ok_or_else(|| EngineError::StrategyNotFound(id.to_string()))?;

        // 커스텀 이름이 있으면 커스텀 이름 사용, 없으면 전략 기본 이름 사용
        let display_name = instance
            .custom_name
            .clone()
            .unwrap_or_else(|| instance.strategy.name().to_string());

        Ok(StrategyStatus {
            name: display_name,
            version: instance.strategy.version().to_string(),
            description: instance.strategy.description().to_string(),
            running: instance.running,
            stats: instance.stats.clone(),
            state: instance.strategy.get_state(),
        })
    }

    /// 전략 설정 조회.
    pub async fn get_strategy_config(&self, id: &str) -> Result<Value, EngineError> {
        let strategies = self.strategies.read().await;

        let instance = strategies
            .get(id)
            .ok_or_else(|| EngineError::StrategyNotFound(id.to_string()))?;

        Ok(instance.config.clone())
    }

    /// 전략 타입 조회 (전략 ID에서 추출).
    pub async fn get_strategy_type(&self, id: &str) -> Result<String, EngineError> {
        let strategies = self.strategies.read().await;

        if !strategies.contains_key(id) {
            return Err(EngineError::StrategyNotFound(id.to_string()));
        }

        // ID 형식: {strategy_type}_{uuid}
        let strategy_type = id
            .rsplit('_')
            .skip(1)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("_");

        Ok(strategy_type)
    }

    /// 모든 전략 목록.
    pub async fn list_strategies(&self) -> Vec<String> {
        let strategies = self.strategies.read().await;
        strategies.keys().cloned().collect()
    }

    /// 모든 전략 상태 조회.
    pub async fn get_all_statuses(&self) -> HashMap<String, StrategyStatus> {
        let strategies = self.strategies.read().await;
        let mut statuses = HashMap::new();

        for (id, instance) in strategies.iter() {
            // 커스텀 이름이 있으면 커스텀 이름 사용, 없으면 전략 기본 이름 사용
            let display_name = instance
                .custom_name
                .clone()
                .unwrap_or_else(|| instance.strategy.name().to_string());

            statuses.insert(
                id.clone(),
                StrategyStatus {
                    name: display_name,
                    version: instance.strategy.version().to_string(),
                    description: instance.strategy.description().to_string(),
                    running: instance.running,
                    stats: instance.stats.clone(),
                    state: instance.strategy.get_state(),
                },
            );
        }

        statuses
    }

    // =========================================================================
    // 다중 타임프레임 지원 메서드 (Phase 1.4)
    // =========================================================================

    /// 전략의 다중 타임프레임 설정 조회.
    ///
    /// # 반환
    ///
    /// 전략이 다중 타임프레임을 사용하면 `Some(config)`, 아니면 `None`
    pub async fn get_strategy_multi_tf_config(
        &self,
        id: &str,
    ) -> Result<Option<trader_core::domain::MultiTimeframeConfig>, EngineError> {
        let strategies = self.strategies.read().await;

        let instance = strategies
            .get(id)
            .ok_or_else(|| EngineError::StrategyNotFound(id.to_string()))?;

        Ok(instance.strategy.multi_timeframe_config())
    }

    /// 전략 컨텍스트 참조 반환.
    ///
    /// 외부에서 전략의 컨텍스트를 업데이트할 때 사용합니다.
    /// 예: 전략 시작 전 히스토리 데이터 로드
    pub async fn get_strategy_context(
        &self,
        id: &str,
    ) -> Result<Arc<RwLock<StrategyContext>>, EngineError> {
        let strategies = self.strategies.read().await;

        let instance = strategies
            .get(id)
            .ok_or_else(|| EngineError::StrategyNotFound(id.to_string()))?;

        Ok(Arc::clone(&instance.context))
    }

    /// 전략 컨텍스트에 캔들 데이터 업데이트.
    ///
    /// 여러 타임프레임의 캔들 데이터를 한 번에 업데이트합니다.
    ///
    /// # 인자
    ///
    /// * `id` - 전략 ID
    /// * `ticker` - 심볼
    /// * `data` - (타임프레임, 캔들 목록) 튜플 벡터
    pub async fn update_strategy_klines(
        &self,
        id: &str,
        ticker: &str,
        data: Vec<(Timeframe, Vec<Kline>)>,
    ) -> Result<(), EngineError> {
        let context = self.get_strategy_context(id).await?;
        let mut ctx = context.write().await;

        ctx.update_multi_timeframe_klines(ticker, data);

        debug!(
            strategy_id = %id,
            ticker = %ticker,
            "Updated strategy klines"
        );

        Ok(())
    }

    /// 시장 데이터 처리 및 모든 실행 중 전략에 라우팅.
    ///
    /// 다중 타임프레임 전략의 경우:
    /// - Secondary TF 데이터: 컨텍스트만 업데이트, 전략 재평가 안 함
    /// - Primary TF 데이터: 모든 TF 데이터와 함께 `on_multi_timeframe_data()` 호출
    pub async fn process_market_data(&self, data: MarketData) -> Result<Vec<Signal>, EngineError> {
        let mut all_signals = Vec::new();
        let mut strategies = self.strategies.write().await;

        for (id, instance) in strategies.iter_mut() {
            if !instance.running {
                continue;
            }

            // 다중 타임프레임 전략 처리
            let signals_result =
                if let Some(mtf_config) = instance.strategy.multi_timeframe_config() {
                    self.process_multi_timeframe_data(instance, &data, &mtf_config)
                        .await
                } else {
                    // 일반 전략: 기존 방식대로 처리
                    instance.strategy.on_market_data(&data).await
                };

            match signals_result {
                Ok(signals) => {
                    instance.stats.market_data_processed += 1;

                    // StrategyContext를 통한 Signal 충돌 검증
                    let ctx = instance.context.read().await;
                    let (valid_signals, conflicts) = ctx.filter_valid_signals(&signals);

                    // 충돌 로깅 및 이벤트 전송
                    for (signal, error) in conflicts {
                        warn!(
                            strategy_id = %id,
                            ticker = %signal.ticker,
                            signal_type = ?signal.signal_type,
                            error = %error,
                            "Signal conflict detected - filtered out"
                        );

                        // 충돌 이벤트 채널로 전송 (WebSocket 알림용)
                        let event = SignalConflictEvent::from_error(id, signal, &error);
                        if let Err(e) = self.conflict_tx.try_send(event) {
                            debug!(error = %e, "Failed to send conflict event (channel full or closed)");
                        }
                    }

                    for signal in valid_signals {
                        instance.stats.signals_generated += 1;
                        instance.stats.last_signal_time = Some(Utc::now());

                        debug!(
                            strategy_id = %id,
                            signal_type = %signal.signal_type,
                            ticker = %signal.ticker,
                            side = ?signal.side,
                            "Strategy generated signal"
                        );

                        all_signals.push(signal.clone());
                    }
                }
                Err(e) => {
                    instance.stats.last_error = Some(e.to_string());
                    error!(
                        strategy_id = %id,
                        error = %e,
                        "Strategy error processing market data"
                    );
                }
            }
        }

        // 활성화된 경우 신호 중복 제거
        if self.config.deduplicate_signals {
            all_signals = self.deduplicate_signals(all_signals).await;
        }

        // 출력 채널로 신호 전송
        for signal in &all_signals {
            if let Err(e) = self.signal_tx.send(signal.clone()).await {
                error!(error = %e, "Failed to send signal to channel");
            }
        }

        // 시장 데이터도 브로드캐스트
        let _ = self.market_data_tx.send(data);

        Ok(all_signals)
    }

    /// 다중 타임프레임 데이터 처리.
    ///
    /// - Kline 데이터가 아니면 기존 방식대로 `on_market_data()` 호출
    /// - Kline 데이터면:
    ///   - 컨텍스트에 캔들 데이터 업데이트
    ///   - Primary TF인 경우에만 `on_multi_timeframe_data()` 호출
    ///   - Secondary TF인 경우 빈 벡터 반환 (캐시만 업데이트)
    async fn process_multi_timeframe_data(
        &self,
        instance: &mut StrategyInstance,
        data: &MarketData,
        mtf_config: &trader_core::domain::MultiTimeframeConfig,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        // Kline 데이터가 아니면 일반 처리
        let kline = match &data.data {
            MarketDataType::Kline(k) => k.clone(),
            _ => return instance.strategy.on_market_data(data).await,
        };

        let incoming_tf = kline.timeframe;
        let ticker = &kline.ticker;

        // 컨텍스트에 캔들 데이터 업데이트
        {
            let mut ctx = instance.context.write().await;
            let candle_count = mtf_config.get_candle_count(incoming_tf);

            // 기존 캔들 목록 가져오기
            let mut klines = ctx.get_klines(ticker, incoming_tf).to_vec();

            // 새 캔들 추가 (시간순 정렬 유지)
            if klines.last().map(|k| k.open_time) != Some(kline.open_time) {
                klines.push(kline.clone());
            } else {
                // 같은 시간의 캔들이면 업데이트
                if let Some(last) = klines.last_mut() {
                    *last = kline.clone();
                }
            }

            // 캔들 수 제한 (오래된 것부터 제거)
            if klines.len() > candle_count {
                klines.drain(0..klines.len() - candle_count);
            }

            ctx.update_klines(ticker, incoming_tf, klines);
        }

        // Primary TF 확인
        let primary_tf = mtf_config.get_primary_timeframe();

        // Primary TF가 아니면 캐시만 업데이트하고 신호 생성 안 함
        if primary_tf.is_some() && Some(incoming_tf) != primary_tf {
            debug!(
                ticker = %ticker,
                incoming_tf = ?incoming_tf,
                primary_tf = ?primary_tf,
                "Secondary TF 데이터 캐시 업데이트 (전략 재평가 스킵)"
            );
            return Ok(vec![]);
        }

        // Primary TF 데이터 → 모든 타임프레임 데이터와 함께 전략 호출
        let secondary_data = {
            let ctx = instance.context.read().await;
            let mut data_map: HashMap<Timeframe, Vec<Kline>> = HashMap::new();

            for &tf in mtf_config.timeframes.keys() {
                if Some(tf) != primary_tf {
                    let klines = ctx.get_klines(ticker, tf).to_vec();
                    data_map.insert(tf, klines);
                }
            }

            data_map
        };

        debug!(
            ticker = %ticker,
            primary_tf = ?primary_tf,
            secondary_tfs = ?secondary_data.keys().collect::<Vec<_>>(),
            "Primary TF 완료 - 전략 재평가 실행"
        );

        instance
            .strategy
            .on_multi_timeframe_data(data, &secondary_data)
            .await
    }

    /// 중복 제거 윈도우 내 신호 중복 제거.
    async fn deduplicate_signals(&self, signals: Vec<Signal>) -> Vec<Signal> {
        let mut recent = self.recent_signals.write().await;
        let now = Utc::now();
        let window_ms = self.config.dedup_window_ms as i64;

        // 오래된 항목 정리
        recent.retain(|_, ts| now.signed_duration_since(*ts).num_milliseconds() < window_ms);

        let mut unique_signals = Vec::new();

        for signal in signals {
            let key = format!(
                "{}:{}:{}:{:?}",
                signal.strategy_id, signal.ticker, signal.signal_type, signal.side
            );

            use std::collections::hash_map::Entry;
            match recent.entry(key) {
                Entry::Vacant(entry) => {
                    entry.insert(now);
                    unique_signals.push(signal);
                }
                Entry::Occupied(_) => {
                    debug!(
                        strategy_id = %signal.strategy_id,
                        ticker = %signal.ticker,
                        "Duplicate signal filtered"
                    );
                }
            }
        }

        unique_signals
    }

    /// 전략에 주문 체결 알림.
    pub async fn notify_order_filled(&self, order: &Order) -> Result<(), EngineError> {
        let mut strategies = self.strategies.write().await;

        for (id, instance) in strategies.iter_mut() {
            if !instance.running {
                continue;
            }

            if let Err(e) = instance.strategy.on_order_filled(order).await {
                instance.stats.last_error = Some(e.to_string());
                error!(
                    strategy_id = %id,
                    error = %e,
                    "Strategy error handling order fill"
                );
            } else {
                instance.stats.orders_filled += 1;
            }
        }

        Ok(())
    }

    /// 전략에 포지션 업데이트 알림.
    pub async fn notify_position_update(&self, position: &Position) -> Result<(), EngineError> {
        let mut strategies = self.strategies.write().await;

        for (id, instance) in strategies.iter_mut() {
            if !instance.running {
                continue;
            }

            if let Err(e) = instance.strategy.on_position_update(position).await {
                instance.stats.last_error = Some(e.to_string());
                error!(
                    strategy_id = %id,
                    error = %e,
                    "Strategy error handling position update"
                );
            }
        }

        Ok(())
    }

    /// 엔진 메인 루프 시작.
    pub async fn run(&self) -> Result<(), EngineError> {
        {
            let mut running = self.running.write().await;
            if *running {
                return Err(EngineError::AlreadyRunning("Engine".to_string()));
            }
            *running = true;
        }

        info!("Strategy engine started");

        let mut market_data_rx = self.market_data_tx.subscribe();

        loop {
            tokio::select! {
                result = market_data_rx.recv() => {
                    match result {
                        Ok(data) => {
                            if let Err(e) = self.process_market_data(data).await {
                                error!(error = %e, "Error processing market data");
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(skipped = n, "Market data receiver lagged");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Market data channel closed");
                            break;
                        }
                    }
                }
                _ = self.wait_for_shutdown() => {
                    info!("Strategy engine shutdown requested");
                    break;
                }
            }
        }

        // 모든 실행 중 전략 중지
        self.stop_all_strategies().await;

        {
            let mut running = self.running.write().await;
            *running = false;
        }

        info!("Strategy engine stopped");
        Ok(())
    }

    /// 종료 신호 대기.
    async fn wait_for_shutdown(&self) {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let running = self.running.read().await;
            if !*running {
                break;
            }
        }
    }

    /// 엔진 종료 요청.
    pub async fn shutdown(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }

    /// 모든 실행 중 전략 중지.
    pub async fn stop_all_strategies(&self) {
        let strategy_ids: Vec<String> = {
            let strategies = self.strategies.read().await;
            strategies
                .iter()
                .filter(|(_, inst)| inst.running)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for id in strategy_ids {
            if let Err(e) = self.stop_strategy(&id).await {
                error!(strategy_id = %id, error = %e, "Error stopping strategy");
            }
        }
    }

    /// 모든 등록된 전략 시작.
    pub async fn start_all_strategies(&self) -> Result<(), EngineError> {
        let strategy_ids: Vec<String> = {
            let strategies = self.strategies.read().await;
            strategies
                .iter()
                .filter(|(_, inst)| !inst.running)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for id in strategy_ids {
            self.start_strategy(&id).await?;
        }

        Ok(())
    }

    /// 엔진 통계 조회.
    pub async fn get_engine_stats(&self) -> EngineStats {
        let strategies = self.strategies.read().await;

        let mut total_signals = 0u64;
        let mut total_orders = 0u64;
        let mut total_data_processed = 0u64;
        let mut running_strategies = 0usize;

        for instance in strategies.values() {
            total_signals += instance.stats.signals_generated;
            total_orders += instance.stats.orders_filled;
            total_data_processed += instance.stats.market_data_processed;
            if instance.running {
                running_strategies += 1;
            }
        }

        EngineStats {
            total_strategies: strategies.len(),
            running_strategies,
            total_signals_generated: total_signals,
            total_orders_filled: total_orders,
            total_market_data_processed: total_data_processed,
        }
    }

    /// 전략 설정 업데이트 (핫 리로드).
    pub async fn update_strategy_config(&self, id: &str, config: Value) -> Result<(), EngineError> {
        let mut strategies = self.strategies.write().await;

        let instance = strategies
            .get_mut(id)
            .ok_or_else(|| EngineError::StrategyNotFound(id.to_string()))?;

        // config에서 name 필드 추출하여 custom_name으로 저장
        let mut config_for_strategy = config.clone();
        if let Some(obj) = config_for_strategy.as_object_mut() {
            if let Some(name_value) = obj.remove("name") {
                if let Some(name_str) = name_value.as_str() {
                    instance.custom_name = Some(name_str.to_string());
                    info!(strategy_id = %id, name = %name_str, "Updated strategy custom name");
                }
            }
        }

        // 새 설정 저장 (name 필드 제외)
        instance.config = config_for_strategy.clone();

        // 실행 중이면 전략 재초기화
        if instance.running {
            info!(strategy_id = %id, "Hot reloading strategy configuration");

            instance
                .strategy
                .initialize(config_for_strategy)
                .await
                .map_err(|e| EngineError::InitializationFailed(e.to_string()))?;
        }

        Ok(())
    }
}

/// 엔진 통계.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStats {
    /// 등록된 전략 총 수
    pub total_strategies: usize,
    /// 현재 실행 중인 전략 수
    pub running_strategies: usize,
    /// 모든 전략에서 생성된 총 신호 수
    pub total_signals_generated: u64,
    /// 총 체결된 주문 수
    pub total_orders_filled: u64,
    /// 처리된 총 시장 데이터 이벤트 수
    pub total_market_data_processed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    /// 간단한 테스트 전략.
    struct TestStrategy {
        name: String,
        signal_count: u32,
    }

    impl TestStrategy {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                signal_count: 0,
            }
        }
    }

    #[async_trait]
    impl Strategy for TestStrategy {
        fn name(&self) -> &str {
            &self.name
        }

        fn version(&self) -> &str {
            "1.0.0"
        }

        fn description(&self) -> &str {
            "Test strategy"
        }

        async fn initialize(
            &mut self,
            _config: Value,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        async fn on_market_data(
            &mut self,
            data: &MarketData,
        ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
            self.signal_count += 1;

            // 10개 데이터 포인트마다 신호 생성
            if self.signal_count % 10 == 0 {
                let signal = Signal::entry(&self.name, data.ticker.clone(), trader_core::Side::Buy);
                Ok(vec![signal])
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
            serde_json::json!({
                "signal_count": self.signal_count
            })
        }
    }

    #[tokio::test]
    async fn test_engine_register_strategy() {
        let engine = StrategyEngine::new(EngineConfig::default());

        let strategy = Box::new(TestStrategy::new("test"));
        engine
            .register_strategy("test1", strategy, serde_json::json!({}), None, None)
            .await
            .unwrap();

        let strategies = engine.list_strategies().await;
        assert_eq!(strategies.len(), 1);
        assert!(strategies.contains(&"test1".to_string()));
    }

    #[tokio::test]
    async fn test_engine_register_strategy_with_custom_name() {
        let engine = StrategyEngine::new(EngineConfig::default());

        let strategy = Box::new(TestStrategy::new("test"));
        engine
            .register_strategy(
                "test1",
                strategy,
                serde_json::json!({}),
                Some("나의 커스텀 전략".to_string()),
                None,
            )
            .await
            .unwrap();

        let status = engine.get_strategy_status("test1").await.unwrap();
        assert_eq!(status.name, "나의 커스텀 전략");
    }

    #[tokio::test]
    async fn test_engine_start_stop_strategy() {
        let engine = StrategyEngine::new(EngineConfig::default());

        let strategy = Box::new(TestStrategy::new("test"));
        engine
            .register_strategy("test1", strategy, serde_json::json!({}), None, None)
            .await
            .unwrap();

        // 전략 시작
        engine.start_strategy("test1").await.unwrap();

        let status = engine.get_strategy_status("test1").await.unwrap();
        assert!(status.running);

        // 전략 중지
        engine.stop_strategy("test1").await.unwrap();

        let status = engine.get_strategy_status("test1").await.unwrap();
        assert!(!status.running);
    }

    #[tokio::test]
    async fn test_duplicate_strategy_error() {
        let engine = StrategyEngine::new(EngineConfig::default());

        let strategy1 = Box::new(TestStrategy::new("test"));
        let strategy2 = Box::new(TestStrategy::new("test"));

        engine
            .register_strategy("test1", strategy1, serde_json::json!({}), None, None)
            .await
            .unwrap();

        let result = engine
            .register_strategy("test1", strategy2, serde_json::json!({}), None, None)
            .await;

        assert!(matches!(result, Err(EngineError::StrategyAlreadyExists(_))));
    }
}
