//! Mock 거래소 ExchangeProvider 구현.
//!
//! UI에서 등록하여 전략의 실제 동작을 검증하는 가상 거래소입니다.
//! 실시간 시세(Yahoo Finance)와 DB 영속성을 지원합니다.
//!
//! # 아키텍처
//!
//! ```text
//! MockExchangeProvider
//! ├── ExchangeProvider 구현 (계정정보 조회)
//! ├── SignalProcessor 위임 (SimulatedExecutor)
//! ├── 실시간 시세 조회 (Yahoo Finance)
//! └── DB 상태 영속성 (PostgreSQL)
//! ```
//!
//! # 거래소 중립성
//!
//! Mock 거래소는 다른 실제 거래소(KIS, Binance 등)와 동일한 인터페이스를 제공합니다.
//! 이를 통해 전략 코드는 거래소 종류와 무관하게 동일한 방식으로 동작합니다.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::simulated::EventBroadcaster;
use crate::traits::MarketEvent;
use trader_core::Ticker;

use crate::historical::HistoricalDataProvider;
use crate::yahoo::YahooFinanceProvider;
use trader_core::domain::{
    ExchangeProvider, ExecutionHistoryRequest, ExecutionHistoryResponse, OrderExecutionProvider,
    OrderRequest, OrderResponse, PendingOrder, ProviderError, Side, StrategyAccountInfo,
    StrategyPositionInfo, Trade,
};
use trader_core::Timeframe;
use trader_execution::{ProcessorPosition, TradeResult};

/// Mock 거래소 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockConfig {
    /// 초기 자금
    pub initial_balance: Decimal,
    /// 수수료율 (기본 0.015%)
    pub commission_rate: Decimal,
    /// 슬리피지율 (기본 0.01%)
    pub slippage_rate: Decimal,
    /// 시장 유형 ("stock_kr", "stock_us", "crypto")
    pub market_type: String,
    /// 통화
    pub currency: String,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            initial_balance: dec!(10_000_000), // 1천만원
            commission_rate: dec!(0.00015),    // 0.015%
            slippage_rate: dec!(0.0001),       // 0.01%
            market_type: "stock_kr".to_string(),
            currency: "KRW".to_string(),
        }
    }
}

impl MockConfig {
    /// US 주식 기본 설정
    pub fn stock_us() -> Self {
        Self {
            initial_balance: dec!(10_000),
            commission_rate: dec!(0.0),      // 무료 수수료
            slippage_rate: dec!(0.0001),
            market_type: "stock_us".to_string(),
            currency: "USD".to_string(),
        }
    }

    /// 암호화폐 기본 설정
    pub fn crypto() -> Self {
        Self {
            initial_balance: dec!(10_000),
            commission_rate: dec!(0.001),    // 0.1%
            slippage_rate: dec!(0.0005),     // 0.05%
            market_type: "crypto".to_string(),
            currency: "USDT".to_string(),
        }
    }

    /// 초기 잔고 설정 (빌더 패턴).
    pub fn with_balance(mut self, balance: Decimal) -> Self {
        self.initial_balance = balance;
        self
    }

    /// 수수료율 설정 (빌더 패턴).
    pub fn with_commission_rate(mut self, rate: Decimal) -> Self {
        self.commission_rate = rate;
        self
    }

    /// 슬리피지율 설정 (빌더 패턴).
    pub fn with_slippage_rate(mut self, rate: Decimal) -> Self {
        self.slippage_rate = rate;
        self
    }
}

/// Mock 거래소 전략별 상태 (메모리)
#[derive(Debug, Clone)]
struct StrategyState {
    /// 현재 잔고
    balance: Decimal,
    /// 포지션 목록
    positions: HashMap<String, ProcessorPosition>,
    /// 거래 기록
    trades: Vec<TradeResult>,
    /// 총 수수료
    total_commission: Decimal,
    /// 초기 잔고
    initial_balance: Decimal,
}

impl StrategyState {
    fn new(initial_balance: Decimal) -> Self {
        Self {
            balance: initial_balance,
            positions: HashMap::new(),
            trades: Vec::new(),
            total_commission: Decimal::ZERO,
            initial_balance,
        }
    }

    fn reset(&mut self) {
        self.balance = self.initial_balance;
        self.positions.clear();
        self.trades.clear();
        self.total_commission = Decimal::ZERO;
    }
}

/// Mock 거래소 상태 (전략별 관리)
///
/// 하나의 Mock 계정에서 여러 전략이 독립적인 잔고/포지션을 가집니다.
#[derive(Debug, Clone, Default)]
struct MockState {
    /// 전략별 상태 (strategy_id -> StrategyState)
    strategies: HashMap<String, StrategyState>,
}

impl MockState {
    fn new() -> Self {
        Self {
            strategies: HashMap::new(),
        }
    }

    /// 전략 상태 조회 또는 생성.
    fn get_or_create_strategy(&mut self, strategy_id: &str, initial_balance: Decimal) -> &mut StrategyState {
        self.strategies
            .entry(strategy_id.to_string())
            .or_insert_with(|| StrategyState::new(initial_balance))
    }

    /// 전략 상태 조회 (읽기 전용).
    fn get_strategy(&self, strategy_id: &str) -> Option<&StrategyState> {
        self.strategies.get(strategy_id)
    }

    /// 전략 상태 조회 (쓰기).
    fn get_strategy_mut(&mut self, strategy_id: &str) -> Option<&mut StrategyState> {
        self.strategies.get_mut(strategy_id)
    }

    /// 전략 목록 조회.
    fn strategy_ids(&self) -> Vec<String> {
        self.strategies.keys().cloned().collect()
    }

    /// 전체 잔고 합계.
    fn total_balance(&self) -> Decimal {
        self.strategies.values().map(|s| s.balance).sum()
    }

    /// 전체 포지션 수.
    fn total_position_count(&self) -> usize {
        self.strategies.values().map(|s| s.positions.len()).sum()
    }
}

/// Mock 거래소 ExchangeProvider.
///
/// UI에서 등록하여 전략의 실제 동작을 검증합니다.
/// 실시간 시세는 Yahoo Finance에서 가져오며, 상태는 DB에 영속화됩니다.
///
/// # 전략별 독립 관리
///
/// 하나의 Mock 계정(credential_id)에서 여러 전략이 실행될 수 있습니다.
/// 각 전략은 독립적인 잔고와 포지션을 가지며, 스트림 원천만 공유합니다.
///
/// ```text
/// MockExchangeProvider (credential_id)
/// ├── 스트림 원천 (EventBroadcaster) - 공유
/// ├── 전략 A 상태 (잔고, 포지션) - 독립
/// ├── 전략 B 상태 (잔고, 포지션) - 독립
/// └── 전략 C 상태 (잔고, 포지션) - 독립
/// ```
///
/// # 실시간 스트리밍
///
/// Paper Trading 시 실제 거래소처럼 가격 데이터를 스트리밍합니다:
/// - `start_streaming()`: 지정된 심볼들의 가격 데이터 발행 시작
/// - `stop_streaming()`: 스트리밍 중지
/// - `create_market_stream()`: 구독자용 스트림 생성
pub struct MockExchangeProvider {
    /// Credential ID (DB 연동용)
    credential_id: Uuid,
    /// 설정
    config: MockConfig,
    /// 전략별 상태 (RwLock으로 동시 접근 보호)
    state: Arc<RwLock<MockState>>,
    /// DB 연결 풀
    db_pool: PgPool,
    /// Yahoo Finance Provider (실시간 시세) - Arc로 감싸 백그라운드 태스크에서 공유
    yahoo: Option<Arc<YahooFinanceProvider>>,
    /// 시장 이벤트 브로드캐스터
    market_broadcaster: Arc<EventBroadcaster<MarketEvent>>,
    /// 스트리밍 실행 플래그
    streaming_active: Arc<AtomicBool>,
    /// 스트리밍 중지 채널
    streaming_stop_tx: Arc<RwLock<Option<mpsc::Sender<()>>>>,
}

impl MockExchangeProvider {
    /// 새 Mock Provider 생성.
    pub async fn new(
        credential_id: Uuid,
        config: MockConfig,
        db_pool: PgPool,
    ) -> Result<Self, ProviderError> {
        let state = Arc::new(RwLock::new(MockState::new()));

        // Yahoo Finance 초기화 (실패해도 계속 진행)
        let yahoo = match YahooFinanceProvider::new() {
            Ok(y) => Some(Arc::new(y)),
            Err(e) => {
                warn!("Yahoo Finance 초기화 실패 (시세 조회 불가): {:?}", e);
                None
            }
        };

        let provider = Self {
            credential_id,
            config,
            state,
            db_pool,
            yahoo,
            market_broadcaster: Arc::new(EventBroadcaster::new()),
            streaming_active: Arc::new(AtomicBool::new(false)),
            streaming_stop_tx: Arc::new(RwLock::new(None)),
        };

        // DB에서 상태 복원
        if let Err(e) = provider.load_state().await {
            info!("Mock 상태 복원 실패 (신규 계정): {:?}", e);
        }

        Ok(provider)
    }

    /// 전략 초기화 (Paper Trading 시작 시 호출).
    ///
    /// 전략별 초기 잔고를 설정합니다. 이미 존재하면 기존 상태 유지.
    pub async fn init_strategy(&self, strategy_id: &str, initial_balance: Decimal) {
        let mut state = self.state.write().await;
        if !state.strategies.contains_key(strategy_id) {
            state.strategies.insert(strategy_id.to_string(), StrategyState::new(initial_balance));
            info!("전략 초기화: {} (잔고: {})", strategy_id, initial_balance);
        }
    }

    /// 전략 목록 조회.
    pub async fn strategy_ids(&self) -> Vec<String> {
        self.state.read().await.strategy_ids()
    }

    /// DB에서 전략별 상태 복원.
    pub async fn load_state(&self) -> Result<(), ProviderError> {
        // paper_trading_sessions에서 전략별 잔고 복원
        let session_rows = sqlx::query!(
            r#"
            SELECT strategy_id, current_balance, initial_balance
            FROM paper_trading_sessions
            WHERE credential_id = $1
            "#,
            self.credential_id
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| ProviderError::Other(format!("DB 에러: {}", e)))?;

        let mut state = self.state.write().await;
        for row in &session_rows {
            let strategy_state = state.get_or_create_strategy(&row.strategy_id, row.initial_balance);
            strategy_state.balance = row.current_balance;
        }

        // 전략별 포지션 복원
        let position_rows = sqlx::query!(
            r#"
            SELECT strategy_id, symbol, side, quantity, entry_price, entry_time
            FROM mock_positions
            WHERE credential_id = $1
            "#,
            self.credential_id
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| ProviderError::Other(format!("DB 에러: {}", e)))?;

        for row in position_rows {
            if let Some(strategy_id) = &row.strategy_id {
                let side = match row.side.as_str() {
                    "Buy" | "buy" => Side::Buy,
                    "Sell" | "sell" => Side::Sell,
                    _ => Side::Buy,
                };

                let position = ProcessorPosition {
                    symbol: row.symbol.clone(),
                    side,
                    quantity: row.quantity,
                    entry_price: row.entry_price,
                    entry_time: row.entry_time,
                    fees: Decimal::ZERO,
                    position_id: None,
                    group_id: None,
                };

                if let Some(strategy_state) = state.get_strategy_mut(strategy_id) {
                    strategy_state.positions.insert(row.symbol, position);
                }
            }
        }

        debug!("Mock 상태 복원 완료 (전략: {}, 총 포지션: {})",
            state.strategies.len(),
            state.total_position_count()
        );
        Ok(())
    }

    /// 전략별 상태를 DB에 저장.
    pub async fn save_strategy_state(&self, strategy_id: &str) -> Result<(), ProviderError> {
        let state = self.state.read().await;
        let strategy_state = state.get_strategy(strategy_id).ok_or_else(|| {
            ProviderError::Other(format!("전략 상태 없음: {}", strategy_id))
        })?;

        // 1. 세션 잔고 업데이트
        sqlx::query!(
            r#"
            UPDATE paper_trading_sessions
            SET current_balance = $1, updated_at = NOW()
            WHERE strategy_id = $2
            "#,
            strategy_state.balance,
            strategy_id
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| ProviderError::Other(format!("DB 에러: {}", e)))?;

        // 2. 포지션 저장 (기존 삭제 후 재삽입)
        sqlx::query!(
            r#"DELETE FROM mock_positions WHERE strategy_id = $1"#,
            strategy_id
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| ProviderError::Other(format!("DB 에러: {}", e)))?;

        for (_, position) in strategy_state.positions.iter() {
            let side_str = match position.side {
                Side::Buy => "Buy",
                Side::Sell => "Sell",
            };

            sqlx::query!(
                r#"
                INSERT INTO mock_positions (credential_id, strategy_id, symbol, side, quantity, entry_price, entry_time)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                self.credential_id,
                strategy_id,
                position.symbol,
                side_str,
                position.quantity,
                position.entry_price,
                position.entry_time
            )
            .execute(&self.db_pool)
            .await
            .map_err(|e| ProviderError::Other(format!("DB 에러: {}", e)))?;
        }

        debug!("전략 {} 상태 저장 완료 (잔고: {}, 포지션: {})",
            strategy_id,
            strategy_state.balance,
            strategy_state.positions.len()
        );
        Ok(())
    }

    /// 체결 내역 DB에 저장 (전략별).
    async fn save_execution(&self, trade: &TradeResult, strategy_id: &str) -> Result<(), ProviderError> {
        let side_str = match trade.side {
            Side::Buy => "Buy",
            Side::Sell => "Sell",
        };

        sqlx::query!(
            r#"
            INSERT INTO mock_executions (credential_id, strategy_id, symbol, side, quantity, price, commission, realized_pnl, executed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            self.credential_id,
            strategy_id,
            trade.symbol,
            side_str,
            trade.quantity,
            trade.price,
            trade.commission,
            trade.realized_pnl,
            trade.timestamp
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| ProviderError::Other(format!("DB 에러: {}", e)))?;

        Ok(())
    }

    /// Yahoo Finance에서 현재가 조회.
    pub async fn get_current_price(&self, symbol: &str) -> Result<Decimal, ProviderError> {
        let yahoo = self.yahoo.as_ref().ok_or_else(|| {
            ProviderError::Network("Yahoo Finance 미초기화".to_string())
        })?;

        // 심볼을 Yahoo Finance 형식으로 변환
        let yahoo_symbol = self.to_yahoo_symbol(symbol);

        // 최근 1일 데이터에서 마지막 종가 사용
        let klines = yahoo
            .get_klines(&yahoo_symbol, Timeframe::D1, 1)
            .await
            .map_err(|e| ProviderError::Api(format!("Yahoo Finance 오류: {:?}", e)))?;

        klines
            .last()
            .map(|k| k.close)
            .ok_or_else(|| ProviderError::Api("시세 데이터 없음".to_string()))
    }

    /// 심볼을 Yahoo Finance 형식으로 변환.
    fn to_yahoo_symbol(&self, symbol: &str) -> String {
        match self.config.market_type.as_str() {
            "stock_kr" => {
                // 이미 .KS/.KQ 형식이면 그대로, 아니면 .KS 추가
                if symbol.ends_with(".KS") || symbol.ends_with(".KQ") {
                    symbol.to_string()
                } else {
                    format!("{}.KS", symbol)
                }
            }
            _ => symbol.to_string(),
        }
    }

    /// 전략별 잔고 조회.
    pub async fn balance(&self, strategy_id: &str) -> Decimal {
        self.state.read().await
            .get_strategy(strategy_id)
            .map(|s| s.balance)
            .unwrap_or(Decimal::ZERO)
    }

    /// 전략별 포지션 목록 조회.
    pub async fn positions(&self, strategy_id: &str) -> HashMap<String, ProcessorPosition> {
        self.state.read().await
            .get_strategy(strategy_id)
            .map(|s| s.positions.clone())
            .unwrap_or_default()
    }

    /// 전략별 거래 기록 조회.
    pub async fn trades(&self, strategy_id: &str) -> Vec<TradeResult> {
        self.state.read().await
            .get_strategy(strategy_id)
            .map(|s| s.trades.clone())
            .unwrap_or_default()
    }

    /// 전체 잔고 합계 (모든 전략).
    pub async fn total_balance(&self) -> Decimal {
        self.state.read().await.total_balance()
    }

    /// 전체 포지션 수 (모든 전략).
    pub async fn total_position_count(&self) -> usize {
        self.state.read().await.total_position_count()
    }

    /// Signal 처리 (체결) - 전략별 독립 상태.
    ///
    /// Signal의 strategy에서 전략 ID를 추출하여 해당 전략의 상태를 업데이트합니다.
    /// 수량은 metadata에서 추출하거나 기본값 1을 사용합니다.
    pub async fn process_signal(
        &self,
        signal: &trader_core::Signal,
        current_price: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<TradeResult>, ProviderError> {
        use trader_core::SignalType;

        let strategy_id = &signal.strategy_id;
        let mut state = self.state.write().await;

        // 전략 상태 확인 (없으면 에러)
        let strategy_state = state.get_strategy_mut(strategy_id).ok_or_else(|| {
            ProviderError::Other(format!("전략 상태 없음: {}. init_strategy()를 먼저 호출하세요.", strategy_id))
        })?;

        // 슬리피지 적용
        let slippage = current_price * self.config.slippage_rate;
        let execution_price = match signal.side {
            Side::Buy => current_price + slippage,
            Side::Sell => current_price - slippage,
        };

        // 수량은 metadata에서 추출하거나 기본값 사용
        let quantity = signal
            .metadata
            .get("quantity")
            .and_then(|v| v.as_f64())
            .map(|v| Decimal::from_f64_retain(v).unwrap_or(Decimal::ONE))
            .unwrap_or(Decimal::ONE);

        let trade_value = execution_price * quantity;
        let commission = trade_value * self.config.commission_rate;

        let trade = match signal.signal_type {
            SignalType::Entry => {
                // 진입: 잔고 확인 및 포지션 생성
                let required = trade_value + commission;
                if strategy_state.balance < required {
                    return Err(ProviderError::Other(format!(
                        "[{}] 자금 부족: 필요 {}, 보유 {}",
                        strategy_id, required, strategy_state.balance
                    )));
                }

                strategy_state.balance -= required;
                let position = ProcessorPosition {
                    symbol: signal.ticker.clone(),
                    side: signal.side,
                    quantity,
                    entry_price: execution_price,
                    entry_time: timestamp,
                    fees: commission,
                    position_id: signal.position_id.clone(),
                    group_id: signal.group_id.clone(),
                };
                strategy_state.positions.insert(signal.ticker.clone(), position);

                TradeResult {
                    symbol: signal.ticker.clone(),
                    side: signal.side,
                    signal_type: signal.signal_type,
                    quantity,
                    price: execution_price,
                    commission,
                    slippage,
                    timestamp,
                    realized_pnl: None,
                    is_partial: false,
                    metadata: HashMap::new(),
                }
            }
            SignalType::Exit => {
                // 청산: 포지션 확인 및 제거
                let position = strategy_state.positions.remove(&signal.ticker).ok_or_else(|| {
                    ProviderError::Other(format!("[{}] 포지션 없음: {}", strategy_id, signal.ticker))
                })?;

                // 실현 손익 계산
                let realized_pnl = if position.side == Side::Buy {
                    (execution_price - position.entry_price) * position.quantity - commission
                } else {
                    (position.entry_price - execution_price) * position.quantity - commission
                };

                let exit_value = execution_price * position.quantity;
                strategy_state.balance += exit_value - commission;

                TradeResult {
                    symbol: signal.ticker.clone(),
                    side: signal.side,
                    signal_type: signal.signal_type,
                    quantity: position.quantity,
                    price: execution_price,
                    commission,
                    slippage,
                    timestamp,
                    realized_pnl: Some(realized_pnl),
                    is_partial: false,
                    metadata: HashMap::new(),
                }
            }
            SignalType::AddToPosition => {
                // 추가 매수
                let required = trade_value + commission;
                if strategy_state.balance < required {
                    return Err(ProviderError::Other(format!(
                        "[{}] 자금 부족: 필요 {}, 보유 {}",
                        strategy_id, required, strategy_state.balance
                    )));
                }

                strategy_state.balance -= required;

                if let Some(position) = strategy_state.positions.get_mut(&signal.ticker) {
                    // 평균 단가 재계산
                    let total_value = position.entry_price * position.quantity + execution_price * quantity;
                    let total_quantity = position.quantity + quantity;
                    position.entry_price = total_value / total_quantity;
                    position.quantity = total_quantity;
                    position.fees += commission;
                } else {
                    // 신규 포지션 생성
                    let position = ProcessorPosition {
                        symbol: signal.ticker.clone(),
                        side: signal.side,
                        quantity,
                        entry_price: execution_price,
                        entry_time: timestamp,
                        fees: commission,
                        position_id: signal.position_id.clone(),
                        group_id: signal.group_id.clone(),
                    };
                    strategy_state.positions.insert(signal.ticker.clone(), position);
                }

                TradeResult {
                    symbol: signal.ticker.clone(),
                    side: signal.side,
                    signal_type: signal.signal_type,
                    quantity,
                    price: execution_price,
                    commission,
                    slippage,
                    timestamp,
                    realized_pnl: None,
                    is_partial: true,
                    metadata: HashMap::new(),
                }
            }
            SignalType::ReducePosition => {
                // 일부 청산 - 먼저 포지션 정보 복사
                let (pos_side, pos_qty, pos_entry_price) = {
                    let position = strategy_state.positions.get(&signal.ticker).ok_or_else(|| {
                        ProviderError::Other(format!("[{}] 포지션 없음: {}", strategy_id, signal.ticker))
                    })?;
                    (position.side, position.quantity, position.entry_price)
                };

                let reduce_qty = quantity.min(pos_qty);
                let realized_pnl = if pos_side == Side::Buy {
                    (execution_price - pos_entry_price) * reduce_qty - commission
                } else {
                    (pos_entry_price - execution_price) * reduce_qty - commission
                };

                let remaining_qty = pos_qty - reduce_qty;
                let should_remove = remaining_qty <= Decimal::ZERO;

                // 포지션 업데이트 또는 제거
                if should_remove {
                    strategy_state.positions.remove(&signal.ticker);
                } else if let Some(pos) = strategy_state.positions.get_mut(&signal.ticker) {
                    pos.quantity = remaining_qty;
                }

                strategy_state.balance += execution_price * reduce_qty - commission;

                TradeResult {
                    symbol: signal.ticker.clone(),
                    side: signal.side,
                    signal_type: signal.signal_type,
                    quantity: reduce_qty,
                    price: execution_price,
                    commission,
                    slippage,
                    timestamp,
                    realized_pnl: Some(realized_pnl),
                    is_partial: true,
                    metadata: HashMap::new(),
                }
            }
            _ => {
                // 기타 신호는 무시
                return Ok(None);
            }
        };

        // 거래 기록 추가
        strategy_state.trades.push(trade.clone());
        strategy_state.total_commission += commission;

        // strategy_id 복사 (borrow 해제를 위해)
        let strategy_id_owned = strategy_id.to_string();

        // 상태 저장을 위해 락 해제 후 저장
        drop(state);
        self.save_strategy_state(&strategy_id_owned).await?;
        self.save_execution(&trade, &strategy_id_owned).await?;

        info!(
            "[{}] Mock 체결: {} {} {} @ {} (PnL: {:?})",
            strategy_id_owned, trade.symbol, trade.side, trade.quantity, trade.price, trade.realized_pnl
        );

        Ok(Some(trade))
    }

    /// 전략별 상태 초기화 (리셋).
    pub async fn reset_strategy(&self, strategy_id: &str) -> Result<(), ProviderError> {
        let mut state = self.state.write().await;
        if let Some(strategy_state) = state.get_strategy_mut(strategy_id) {
            strategy_state.reset();
        }
        drop(state);

        // DB에서 전략별 상태 삭제
        sqlx::query!(
            r#"DELETE FROM mock_positions WHERE strategy_id = $1"#,
            strategy_id
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| ProviderError::Other(format!("DB 에러: {}", e)))?;

        sqlx::query!(
            r#"DELETE FROM mock_executions WHERE strategy_id = $1"#,
            strategy_id
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| ProviderError::Other(format!("DB 에러: {}", e)))?;

        // 세션 잔고 초기화
        sqlx::query!(
            r#"
            UPDATE paper_trading_sessions
            SET current_balance = initial_balance, status = 'stopped', updated_at = NOW()
            WHERE strategy_id = $1
            "#,
            strategy_id
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| ProviderError::Other(format!("DB 에러: {}", e)))?;

        info!("전략 {} 상태 초기화 완료", strategy_id);
        Ok(())
    }

    /// 전체 상태 초기화 (모든 전략).
    pub async fn reset_all(&self) -> Result<(), ProviderError> {
        let mut state = self.state.write().await;
        state.strategies.clear();
        drop(state);

        // DB에서도 상태 삭제
        sqlx::query!(
            r#"DELETE FROM mock_positions WHERE credential_id = $1"#,
            self.credential_id
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| ProviderError::Other(format!("DB 에러: {}", e)))?;

        sqlx::query!(
            r#"DELETE FROM mock_executions WHERE credential_id = $1"#,
            self.credential_id
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| ProviderError::Other(format!("DB 에러: {}", e)))?;

        info!("Mock 전체 상태 초기화 완료");
        Ok(())
    }
}

#[async_trait]
impl ExchangeProvider for MockExchangeProvider {
    /// 계정 정보 조회 (거래소 중립적 형식).
    ///
    /// 모든 전략의 잔고 합계를 반환합니다.
    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
        let state = self.state.read().await;

        // 모든 전략의 잔고 합계
        let total_balance = state.total_balance();

        // 모든 전략의 포지션 평가액 합계
        let position_value: Decimal = state
            .strategies
            .values()
            .flat_map(|s| s.positions.values())
            .map(|p| p.entry_price * p.quantity)
            .sum();

        Ok(StrategyAccountInfo {
            total_balance: total_balance + position_value,
            available_balance: total_balance,
            margin_used: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO, // 실시간 계산 비용 절약
            currency: self.config.currency.clone(),
        })
    }

    /// 포지션 목록 조회 (거래소 중립적 형식).
    ///
    /// 모든 전략의 포지션을 반환합니다.
    async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        let state = self.state.read().await;

        let positions: Vec<StrategyPositionInfo> = state
            .strategies
            .values()
            .flat_map(|s| s.positions.values())
            .map(|p| {
                StrategyPositionInfo::new(
                    p.symbol.clone(),
                    p.side,
                    p.quantity,
                    p.entry_price,
                )
            })
            .collect();

        Ok(positions)
    }

    /// 미체결 주문 조회.
    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError> {
        // Mock 거래소는 즉시 체결되므로 미체결 주문 없음
        Ok(Vec::new())
    }

    /// 거래소 이름 반환.
    fn exchange_name(&self) -> &str {
        "Mock Exchange"
    }

    /// 체결 내역 조회 (거래소 중립적 형식).
    async fn fetch_execution_history(
        &self,
        _request: &ExecutionHistoryRequest,
    ) -> Result<ExecutionHistoryResponse, ProviderError> {
        // DB에서 체결 내역 조회 (최근 100건)
        let rows = sqlx::query!(
            r#"
            SELECT symbol, side, quantity, price, commission, realized_pnl, executed_at
            FROM mock_executions
            WHERE credential_id = $1
            ORDER BY executed_at DESC
            LIMIT 100
            "#,
            self.credential_id
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| ProviderError::Other(format!("DB 에러: {}", e)))?;

        let trades = rows
            .into_iter()
            .map(|row| {
                let side = match row.side.as_str() {
                    "Buy" | "buy" => Side::Buy,
                    _ => Side::Sell,
                };

                Trade::new(
                    Uuid::new_v4(), // order_id (mock)
                    "mock",
                    Uuid::new_v4().to_string(), // exchange_trade_id
                    row.symbol,
                    side,
                    row.quantity,
                    row.price,
                )
                .with_fee(row.commission, &self.config.currency)
                .with_executed_at(row.executed_at)
            })
            .collect();

        Ok(ExecutionHistoryResponse {
            trades,
            next_cursor: None,
        })
    }
}

// =============================================================================
// MarketDataProvider 구현
// =============================================================================

use trader_core::domain::{MarketDataProvider, QuoteData};

#[async_trait]
impl MarketDataProvider for MockExchangeProvider {
    async fn get_quote(&self, symbol: &str) -> Result<QuoteData, ProviderError> {
        let yahoo = self.yahoo.as_ref().ok_or_else(|| {
            ProviderError::Network("Yahoo Finance 미초기화".to_string())
        })?;

        // 심볼을 Yahoo Finance 형식으로 변환
        let yahoo_symbol = self.to_yahoo_symbol(symbol);

        // 최근 1일 데이터에서 OHLCV 조회
        let klines = yahoo
            .get_klines(&yahoo_symbol, Timeframe::D1, 2) // 오늘 + 전일 (변동 계산용)
            .await
            .map_err(|e| ProviderError::Api(format!("Yahoo Finance 오류: {:?}", e)))?;

        if klines.is_empty() {
            return Err(ProviderError::Api(format!("시세 데이터 없음: {}", symbol)));
        }

        let today = klines.last().unwrap();
        let prev_close = if klines.len() > 1 {
            klines[klines.len() - 2].close
        } else {
            today.open // 전일 데이터 없으면 시가 사용
        };

        let price_change = today.close - prev_close;
        let change_percent = if !prev_close.is_zero() {
            (price_change / prev_close) * dec!(100)
        } else {
            Decimal::ZERO
        };

        Ok(QuoteData {
            symbol: symbol.to_string(),
            current_price: today.close,
            price_change,
            change_percent,
            high: today.high,
            low: today.low,
            open: today.open,
            prev_close,
            volume: today.volume,
            trading_value: today.quote_volume.unwrap_or(today.volume * today.close),
            timestamp: Utc::now(),
        })
    }

    fn provider_name(&self) -> &str {
        "Mock Exchange (Yahoo Finance)"
    }
}

// =============================================================================
// 실시간 스트리밍 구현
// =============================================================================

use crate::simulated::SimulatedMarketStream;
use std::time::Duration;

/// Mock 거래소용 MarketStream.
///
/// SimulatedMarketStream을 재사용하며, Yahoo Finance에서 실시간 가격을 가져와
/// 주기적으로 MarketEvent를 발행합니다.
pub struct MockMarketStream {
    /// 내부 스트림 (SimulatedMarketStream)
    inner: SimulatedMarketStream,
}

impl MockMarketStream {
    /// 새 Mock 스트림 생성 (EventBroadcaster에서 구독).
    pub fn new(rx: mpsc::Receiver<MarketEvent>) -> Self {
        Self {
            inner: SimulatedMarketStream::new(rx),
        }
    }
}

use crate::traits::MarketStream;

#[async_trait]
impl MarketStream for MockMarketStream {
    async fn subscribe_ticker(&mut self, symbol: &str) -> crate::traits::ExchangeResult<()> {
        self.inner.subscribe_ticker(symbol).await
    }

    async fn subscribe_kline(
        &mut self,
        symbol: &str,
        timeframe: trader_core::Timeframe,
    ) -> crate::traits::ExchangeResult<()> {
        self.inner.subscribe_kline(symbol, timeframe).await
    }

    async fn subscribe_order_book(&mut self, symbol: &str) -> crate::traits::ExchangeResult<()> {
        self.inner.subscribe_order_book(symbol).await
    }

    async fn subscribe_trades(&mut self, symbol: &str) -> crate::traits::ExchangeResult<()> {
        self.inner.subscribe_trades(symbol).await
    }

    async fn unsubscribe(&mut self, symbol: &str) -> crate::traits::ExchangeResult<()> {
        self.inner.unsubscribe(symbol).await
    }

    async fn next_event(&mut self) -> Option<MarketEvent> {
        self.inner.next_event().await
    }
}

impl MockExchangeProvider {
    /// 시장 스트림 생성.
    ///
    /// 구독자가 시장 이벤트를 수신할 수 있는 스트림을 반환합니다.
    /// 실제 거래소의 WebSocket 스트림과 동일한 인터페이스입니다.
    pub async fn create_market_stream(&self) -> MockMarketStream {
        let rx = self.market_broadcaster.subscribe(1000).await;
        MockMarketStream::new(rx)
    }

    /// 스트리밍 시작.
    ///
    /// 지정된 심볼들의 실시간 가격 데이터를 주기적으로 발행합니다.
    /// Paper Trading 시작 시 호출됩니다.
    ///
    /// # Arguments
    ///
    /// * `symbols` - 가격을 스트리밍할 심볼 목록
    /// * `interval_secs` - 가격 조회 간격 (초)
    pub async fn start_streaming(
        &self,
        symbols: Vec<String>,
        interval_secs: u64,
    ) -> Result<(), ProviderError> {
        if self.streaming_active.load(Ordering::SeqCst) {
            info!("스트리밍이 이미 활성화되어 있습니다");
            return Ok(());
        }

        self.streaming_active.store(true, Ordering::SeqCst);
        info!("Mock 거래소 스트리밍 시작: {:?}, 간격: {}초", symbols, interval_secs);

        // 중지 채널 생성
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);
        {
            let mut tx_guard = self.streaming_stop_tx.write().await;
            *tx_guard = Some(stop_tx);
        }

        // 백그라운드 태스크 실행
        let broadcaster = Arc::clone(&self.market_broadcaster);
        let yahoo = self.yahoo.clone();
        let config = self.config.clone();
        let streaming_flag = Arc::clone(&self.streaming_active);

        tokio::spawn(async move {
            let interval = Duration::from_secs(interval_secs);

            loop {
                // 중지 신호 확인
                tokio::select! {
                    _ = stop_rx.recv() => {
                        info!("Mock 스트리밍 중지 신호 수신");
                        break;
                    }
                    _ = tokio::time::sleep(interval) => {
                        // 가격 조회 및 발행
                        if !streaming_flag.load(Ordering::SeqCst) {
                            break;
                        }

                        for symbol in &symbols {
                            if let Some(ref yahoo_provider) = yahoo {
                                // Yahoo Finance에서 가격 조회
                                let yahoo_symbol = match config.market_type.as_str() {
                                    "stock_kr" => {
                                        if symbol.ends_with(".KS") || symbol.ends_with(".KQ") {
                                            symbol.clone()
                                        } else {
                                            format!("{}.KS", symbol)
                                        }
                                    }
                                    _ => symbol.clone(),
                                };

                                match yahoo_provider.get_klines(&yahoo_symbol, Timeframe::D1, 1).await {
                                    Ok(klines) => {
                                        if let Some(kline) = klines.last() {
                                            // Ticker 이벤트 생성
                                            let ticker = Ticker {
                                                ticker: symbol.clone(),
                                                last: kline.close,
                                                bid: kline.close * dec!(0.9999),
                                                ask: kline.close * dec!(1.0001),
                                                high_24h: kline.high,
                                                low_24h: kline.low,
                                                volume_24h: kline.volume,
                                                change_24h: kline.close - kline.open,
                                                change_24h_percent: if kline.open > Decimal::ZERO {
                                                    ((kline.close - kline.open) / kline.open) * dec!(100)
                                                } else {
                                                    Decimal::ZERO
                                                },
                                                timestamp: Utc::now(),
                                            };

                                            debug!("Mock 가격 발행: {} @ {}", symbol, ticker.last);
                                            broadcaster.broadcast(MarketEvent::Ticker(ticker)).await;
                                        }
                                    }
                                    Err(e) => {
                                        error!("Yahoo Finance 가격 조회 실패 ({}): {:?}", symbol, e);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            streaming_flag.store(false, Ordering::SeqCst);
            info!("Mock 스트리밍 종료");
        });

        Ok(())
    }

    /// 스트리밍 중지.
    pub async fn stop_streaming(&self) {
        if !self.streaming_active.load(Ordering::SeqCst) {
            return;
        }

        info!("Mock 거래소 스트리밍 중지 요청");
        self.streaming_active.store(false, Ordering::SeqCst);

        // 중지 신호 전송
        let tx_guard = self.streaming_stop_tx.read().await;
        if let Some(ref tx) = *tx_guard {
            let _ = tx.send(()).await;
        }
    }

    /// 스트리밍 활성화 여부.
    pub fn is_streaming(&self) -> bool {
        self.streaming_active.load(Ordering::SeqCst)
    }

    /// 가격 이벤트 직접 발행 (테스트/수동 사용).
    pub async fn emit_ticker(&self, ticker: Ticker) {
        self.market_broadcaster.broadcast(MarketEvent::Ticker(ticker)).await;
    }
}

// =============================================================================
// OrderExecutionProvider 구현 (Mock 거래소 주문 시뮬레이션)
// =============================================================================

#[async_trait]
impl OrderExecutionProvider for MockExchangeProvider {
    async fn place_order(&self, request: &OrderRequest) -> Result<OrderResponse, ProviderError> {
        // 즉시 체결 시뮬레이션 (슬리피지는 LiveExecutor 레벨에서 적용)
        let order_no = Uuid::new_v4().to_string();
        let order_time = Utc::now().format("%H%M%S").to_string();

        debug!(
            "[Mock] 주문 체결: {} {:?} {} @ {:?}",
            request.ticker, request.side, request.quantity, request.price
        );

        Ok(OrderResponse {
            order_no,
            order_time,
        })
    }

    async fn cancel_order(&self, order_id: &str, _ticker: &str) -> Result<(), ProviderError> {
        debug!("[Mock] 주문 취소: {}", order_id);
        Ok(())
    }

    async fn modify_order(
        &self,
        order_id: &str,
        _ticker: &str,
        _quantity: Option<Decimal>,
        _price: Option<Decimal>,
    ) -> Result<OrderResponse, ProviderError> {
        debug!("[Mock] 주문 정정: {}", order_id);
        Ok(OrderResponse {
            order_no: order_id.to_string(),
            order_time: Utc::now().format("%H%M%S").to_string(),
        })
    }

    fn exchange_name(&self) -> &str {
        "Mock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_mock_config_default() {
        let config = MockConfig::default();
        assert_eq!(config.initial_balance, dec!(10_000_000));
        assert_eq!(config.commission_rate, dec!(0.00015));
        assert_eq!(config.market_type, "stock_kr");
        assert_eq!(config.currency, "KRW");
    }

    #[test]
    fn test_mock_config_us() {
        let config = MockConfig::stock_us();
        assert_eq!(config.initial_balance, dec!(10_000));
        assert_eq!(config.commission_rate, dec!(0.0));
        assert_eq!(config.currency, "USD");
    }

    #[test]
    fn test_mock_state_new() {
        let mut state = MockState::new();
        assert!(state.strategies.is_empty());
        assert_eq!(state.total_balance(), dec!(0));

        // 전략 추가
        state.get_or_create_strategy("test_strategy", dec!(1_000_000));
        assert_eq!(state.strategies.len(), 1);
        assert_eq!(state.total_balance(), dec!(1_000_000));
    }

    #[test]
    fn test_strategy_state_new() {
        let strategy_state = StrategyState::new(dec!(1_000_000));
        assert_eq!(strategy_state.balance, dec!(1_000_000));
        assert_eq!(strategy_state.initial_balance, dec!(1_000_000));
        assert!(strategy_state.positions.is_empty());
        assert!(strategy_state.trades.is_empty());
    }

    #[test]
    fn test_strategy_state_reset() {
        let mut strategy_state = StrategyState::new(dec!(1_000_000));
        strategy_state.balance = dec!(500_000);
        strategy_state.total_commission = dec!(1000);

        strategy_state.reset();

        assert_eq!(strategy_state.balance, dec!(1_000_000));
        assert_eq!(strategy_state.total_commission, dec!(0));
    }
}
