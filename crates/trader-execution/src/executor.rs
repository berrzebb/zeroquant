//! 주문 executor 구현.
//!
//! 제공 기능:
//! - Signal을 OrderRequest로 변환
//! - 주문 라우팅 및 실행
//! - OrderManager를 통한 주문 생명주기 관리
//! - PositionTracker를 통한 포지션 추적
//! - 브라켓 주문 (손절/익절) 자동 관리
//! - OCO(One-Cancels-Other) 주문 관리
//! - 실행 추적 및 보고

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use trader_core::{
    Order, OrderRequest, OrderStatus, OrderStatusType, OrderType, Position, Side, Signal,
    SignalType, TimeInForce,
};
use trader_risk::RiskManager;
use uuid::Uuid;

use crate::order_manager::{OrderFill, OrderManager};
use crate::position_tracker::PositionTracker;

/// 실행 오류 유형.
#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("Risk check failed: {0}")]
    RiskCheckFailed(String),

    #[error("Invalid signal: {0}")]
    InvalidSignal(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Exchange error: {0}")]
    ExchangeError(String),

    #[error("Insufficient balance")]
    InsufficientBalance,

    #[error("Position not found: {0}")]
    PositionNotFound(String),

    #[error("Bracket order error: {0}")]
    BracketOrderError(String),
}

// ==================== 브라켓 주문 관리 ====================

/// 브라켓 주문 (메인 주문 + 손절/익절).
///
/// 메인 주문 체결 시 손절과 익절 주문이 자동으로 제출되며,
/// 둘 중 하나가 체결되면 다른 하나는 자동 취소됩니다 (OCO).
#[derive(Debug, Clone)]
pub struct BracketOrder {
    /// 메인 주문 ID.
    pub parent_order_id: Uuid,
    /// 손절 주문 요청.
    pub stop_loss: Option<OrderRequest>,
    /// 손절 주문 ID (제출 후 설정).
    pub stop_loss_id: Option<Uuid>,
    /// 익절 주문 요청.
    pub take_profit: Option<OrderRequest>,
    /// 익절 주문 ID (제출 후 설정).
    pub take_profit_id: Option<Uuid>,
    /// 메인 주문 체결 여부.
    pub parent_filled: bool,
    /// 브라켓 주문 활성 상태.
    pub active: bool,
}

impl BracketOrder {
    /// 새로운 브라켓 주문 생성.
    pub fn new(
        parent_order_id: Uuid,
        stop_loss: Option<OrderRequest>,
        take_profit: Option<OrderRequest>,
    ) -> Self {
        Self {
            parent_order_id,
            stop_loss,
            stop_loss_id: None,
            take_profit,
            take_profit_id: None,
            parent_filled: false,
            active: true,
        }
    }

    /// 손절 또는 익절 주문 여부 확인.
    pub fn has_child_orders(&self) -> bool {
        self.stop_loss.is_some() || self.take_profit.is_some()
    }

    /// 주문 ID가 이 브라켓의 자식 주문인지 확인.
    pub fn is_child_order(&self, order_id: Uuid) -> bool {
        self.stop_loss_id == Some(order_id) || self.take_profit_id == Some(order_id)
    }

    /// 손절 주문 ID 설정.
    pub fn set_stop_loss_id(&mut self, id: Uuid) {
        self.stop_loss_id = Some(id);
    }

    /// 익절 주문 ID 설정.
    pub fn set_take_profit_id(&mut self, id: Uuid) {
        self.take_profit_id = Some(id);
    }

    /// 브라켓 비활성화.
    pub fn deactivate(&mut self) {
        self.active = false;
    }
}

/// 브라켓 주문 관리자.
///
/// 메인 주문과 연결된 손절/익절 주문을 추적하고,
/// OCO(One-Cancels-Other) 로직을 관리합니다.
#[derive(Debug, Default)]
pub struct BracketOrderManager {
    /// 메인 주문 ID -> 브라켓 주문 매핑.
    brackets: HashMap<Uuid, BracketOrder>,
    /// 자식 주문 ID -> 부모 주문 ID 역참조.
    child_to_parent: HashMap<Uuid, Uuid>,
}

impl BracketOrderManager {
    /// 새로운 브라켓 주문 관리자 생성.
    pub fn new() -> Self {
        Self::default()
    }

    /// 브라켓 주문 등록.
    ///
    /// # Arguments
    /// * `parent_order_id` - 메인 주문 ID
    /// * `stop_loss` - 손절 주문 요청
    /// * `take_profit` - 익절 주문 요청
    pub fn register_bracket(
        &mut self,
        parent_order_id: Uuid,
        stop_loss: Option<OrderRequest>,
        take_profit: Option<OrderRequest>,
    ) {
        if stop_loss.is_none() && take_profit.is_none() {
            return; // 손절/익절 없으면 등록하지 않음
        }

        let bracket = BracketOrder::new(parent_order_id, stop_loss, take_profit);
        self.brackets.insert(parent_order_id, bracket);

        debug!(
            parent_order_id = %parent_order_id,
            "브라켓 주문 등록됨"
        );
    }

    /// 메인 주문 체결 처리.
    ///
    /// 메인 주문이 체결되면 손절/익절 주문을 반환하여 제출할 수 있게 합니다.
    ///
    /// # Returns
    /// * `Some((stop_loss, take_profit))` - 제출할 손절/익절 주문
    /// * `None` - 브라켓이 없거나 이미 처리됨
    pub fn on_parent_filled(
        &mut self,
        parent_order_id: Uuid,
    ) -> Option<(Option<OrderRequest>, Option<OrderRequest>)> {
        let bracket = self.brackets.get_mut(&parent_order_id)?;

        if bracket.parent_filled || !bracket.active {
            return None;
        }

        bracket.parent_filled = true;

        info!(
            parent_order_id = %parent_order_id,
            has_stop_loss = bracket.stop_loss.is_some(),
            has_take_profit = bracket.take_profit.is_some(),
            "메인 주문 체결, 브라켓 주문 제출 준비"
        );

        Some((bracket.stop_loss.clone(), bracket.take_profit.clone()))
    }

    /// 자식 주문 ID 등록 (제출 후 호출).
    pub fn register_child_order(
        &mut self,
        parent_order_id: Uuid,
        child_order_id: Uuid,
        is_stop_loss: bool,
    ) {
        if let Some(bracket) = self.brackets.get_mut(&parent_order_id) {
            if is_stop_loss {
                bracket.set_stop_loss_id(child_order_id);
            } else {
                bracket.set_take_profit_id(child_order_id);
            }
            self.child_to_parent.insert(child_order_id, parent_order_id);
        }
    }

    /// 자식 주문 체결 시 OCO 처리.
    ///
    /// 손절 또는 익절 중 하나가 체결되면, 다른 하나의 주문 ID를 반환하여 취소할 수 있게 합니다.
    ///
    /// # Returns
    /// * `Some(order_id_to_cancel)` - 취소할 상대 주문 ID
    /// * `None` - OCO 대상 없음
    pub fn on_child_filled(&mut self, child_order_id: Uuid) -> Option<Uuid> {
        let parent_id = self.child_to_parent.get(&child_order_id).copied()?;
        let bracket = self.brackets.get_mut(&parent_id)?;

        if !bracket.active {
            return None;
        }

        // 체결된 것이 손절인지 익절인지 확인
        let (filled_type, other_id) = if bracket.stop_loss_id == Some(child_order_id) {
            ("손절", bracket.take_profit_id)
        } else if bracket.take_profit_id == Some(child_order_id) {
            ("익절", bracket.stop_loss_id)
        } else {
            return None;
        };

        bracket.deactivate();

        info!(
            parent_order_id = %parent_id,
            filled_type = filled_type,
            cancel_order_id = ?other_id,
            "브라켓 {} 체결, OCO 취소 처리", filled_type
        );

        // 상대 주문이 있으면 취소 대상으로 반환
        other_id
    }

    /// 특정 부모 주문의 브라켓 조회.
    pub fn get_bracket(&self, parent_order_id: Uuid) -> Option<&BracketOrder> {
        self.brackets.get(&parent_order_id)
    }

    /// 특정 자식 주문의 부모 주문 ID 조회.
    pub fn get_parent_id(&self, child_order_id: Uuid) -> Option<Uuid> {
        self.child_to_parent.get(&child_order_id).copied()
    }

    /// 브라켓 주문 제거.
    pub fn remove_bracket(&mut self, parent_order_id: Uuid) {
        if let Some(bracket) = self.brackets.remove(&parent_order_id) {
            if let Some(sl_id) = bracket.stop_loss_id {
                self.child_to_parent.remove(&sl_id);
            }
            if let Some(tp_id) = bracket.take_profit_id {
                self.child_to_parent.remove(&tp_id);
            }
        }
    }

    /// 활성 브라켓 수 조회.
    pub fn active_count(&self) -> usize {
        self.brackets.values().filter(|b| b.active).count()
    }
}

/// 브라켓 체결 처리 결과.
///
/// `handle_fill_with_brackets()` 호출 후 반환되며,
/// 호출자가 적절한 거래소에 주문을 제출하거나 취소해야 합니다.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum BracketFillResult {
    /// 추가 작업 없음.
    None,

    /// 브라켓 주문 제출 필요 (메인 주문 체결 시).
    SubmitBracket {
        /// 메인 주문 ID.
        parent_order_id: Uuid,
        /// 손절 주문 (있는 경우).
        stop_loss: Option<OrderRequest>,
        /// 익절 주문 (있는 경우).
        take_profit: Option<OrderRequest>,
    },

    /// OCO 주문 취소 필요 (손절 또는 익절 체결 시).
    CancelOther {
        /// 체결된 주문 ID.
        filled_order_id: Uuid,
        /// 취소할 상대 주문 ID.
        cancel_order_id: Uuid,
    },
}

impl BracketFillResult {
    /// 브라켓 제출이 필요한지 확인.
    pub fn needs_bracket_submit(&self) -> bool {
        matches!(self, BracketFillResult::SubmitBracket { .. })
    }

    /// OCO 취소가 필요한지 확인.
    pub fn needs_cancel(&self) -> bool {
        matches!(self, BracketFillResult::CancelOther { .. })
    }

    /// 추가 작업이 필요한지 확인.
    pub fn has_action(&self) -> bool {
        !matches!(self, BracketFillResult::None)
    }
}

/// Signal 변환 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionConfig {
    /// 실행할 최소 신호 강도 (0.0 ~ 1.0)
    pub min_strength: f64,
    /// 지정되지 않은 경우 기본 주문 수량
    pub default_quantity: Decimal,
    /// 진입 시 시장가 주문 사용
    pub use_market_orders: bool,
    /// 기본 슬리피지 허용 비율
    pub slippage_tolerance_pct: f64,
    /// 손절 주문 자동 생성
    pub auto_stop_loss: bool,
    /// 익절 주문 자동 생성
    pub auto_take_profit: bool,
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            min_strength: 0.5,
            default_quantity: Decimal::ZERO,
            use_market_orders: true,
            slippage_tolerance_pct: 0.1,
            auto_stop_loss: true,
            auto_take_profit: true,
        }
    }
}

/// Signal에 대한 실행 결과.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// 원본 신호 ID
    pub signal_id: Uuid,
    /// 내부 주문 ID (주문이 생성된 경우)
    pub order_id: Option<Uuid>,
    /// 생성된 주문 요청 (성공한 경우)
    pub order: Option<OrderRequest>,
    /// 손절 주문 (생성된 경우)
    pub stop_loss: Option<OrderRequest>,
    /// 익절 주문 (생성된 경우)
    pub take_profit: Option<OrderRequest>,
    /// 실행 성공 여부
    pub success: bool,
    /// 오류 메시지 (실패한 경우)
    pub error: Option<String>,
    /// 실행 노트/경고
    pub notes: Vec<String>,
}

impl ExecutionResult {
    /// 성공 결과 생성.
    pub fn success(signal_id: Uuid, order: OrderRequest) -> Self {
        Self {
            signal_id,
            order_id: None,
            order: Some(order),
            stop_loss: None,
            take_profit: None,
            success: true,
            error: None,
            notes: vec![],
        }
    }

    /// 실패 결과 생성.
    pub fn failure(signal_id: Uuid, error: impl Into<String>) -> Self {
        Self {
            signal_id,
            order_id: None,
            order: None,
            stop_loss: None,
            take_profit: None,
            success: false,
            error: Some(error.into()),
            notes: vec![],
        }
    }

    /// 내부 주문 ID 추가.
    pub fn with_order_id(mut self, order_id: Uuid) -> Self {
        self.order_id = Some(order_id);
        self
    }

    /// 손절 주문 추가.
    pub fn with_stop_loss(mut self, order: OrderRequest) -> Self {
        self.stop_loss = Some(order);
        self
    }

    /// 익절 주문 추가.
    pub fn with_take_profit(mut self, order: OrderRequest) -> Self {
        self.take_profit = Some(order);
        self
    }

    /// 노트 추가.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// 내부 주문 ID 조회.
    pub fn order_id(&self) -> Option<Uuid> {
        self.order_id
    }
}

/// Signal을 주문 요청으로 변환하는 Signal 변환기.
#[derive(Debug, Clone)]
pub struct SignalConverter {
    config: ConversionConfig,
}

impl SignalConverter {
    /// 새로운 Signal 변환기 생성.
    pub fn new(config: ConversionConfig) -> Self {
        Self { config }
    }

    /// 기본 설정으로 생성.
    pub fn default_config() -> Self {
        Self::new(ConversionConfig::default())
    }

    /// Signal을 주문 요청으로 변환.
    ///
    /// # 인자
    /// * `signal` - 변환할 트레이딩 신호
    /// * `current_price` - 해당 심볼의 현재 시장 가격
    /// * `quantity` - 주문 수량 (필수, 일반적으로 PositionSizer에서 제공)
    pub fn convert(
        &self,
        signal: &Signal,
        current_price: Decimal,
        quantity: Option<Decimal>,
    ) -> Result<OrderRequest, ExecutionError> {
        // Alert 신호는 실행하지 않음
        if signal.signal_type == SignalType::Alert {
            return Err(ExecutionError::InvalidSignal(
                "Alert signals are not executable".to_string(),
            ));
        }

        // 신호 강도 검증
        if signal.strength < self.config.min_strength {
            return Err(ExecutionError::InvalidSignal(format!(
                "Signal strength {:.2} below minimum {:.2}",
                signal.strength, self.config.min_strength
            )));
        }

        // 수량 결정 (Signal에는 수량이 없음 - 전달받거나 기본값 사용)
        let qty = quantity.unwrap_or(self.config.default_quantity);

        if qty <= Decimal::ZERO {
            return Err(ExecutionError::InvalidSignal(
                "Order quantity must be positive".to_string(),
            ));
        }

        // 신호 유형에 따라 주문 유형과 가격 결정
        let (order_type, price, stop_price) = match signal.signal_type {
            SignalType::Entry | SignalType::AddToPosition => {
                if self.config.use_market_orders {
                    (OrderType::Market, None, None)
                } else {
                    // 제안 가격 또는 슬리피지가 적용된 현재 가격으로 지정가 주문 사용
                    let limit_price = signal
                        .suggested_price
                        .unwrap_or_else(|| self.apply_slippage(current_price, signal.side));
                    (OrderType::Limit, Some(limit_price), None)
                }
            }
            SignalType::Exit | SignalType::ReducePosition => {
                // 청산은 체결 보장을 위해 일반적으로 시장가 주문 사용
                (OrderType::Market, None, None)
            }
            SignalType::Scale => {
                // 스케일 인/아웃은 시장가 주문 사용
                (OrderType::Market, None, None)
            }
            SignalType::Alert => {
                // Alert는 이미 위에서 필터링됨, 여기 도달 불가
                unreachable!("Alert signals should be filtered out before reaching this point")
            }
        };

        // 주문 요청 구성
        let order = OrderRequest {
            ticker: signal.ticker.clone(),
            side: signal.side,
            order_type,
            quantity: qty,
            price,
            stop_price,
            time_in_force: TimeInForce::GTC,
            client_order_id: Some(format!("sig_{}", signal.id)),
            strategy_id: Some(signal.strategy_id.clone()),
        };

        Ok(order)
    }

    /// 가격에 슬리피지 허용치 적용.
    fn apply_slippage(&self, price: Decimal, side: Side) -> Decimal {
        let slippage = Decimal::from_f64_retain(self.config.slippage_tolerance_pct / 100.0)
            .unwrap_or(Decimal::ZERO);

        match side {
            // 매수의 경우 슬리피지 추가 (약간 더 높은 가격 지불 의향)
            Side::Buy => price * (Decimal::ONE + slippage),
            // 매도의 경우 슬리피지 차감 (약간 더 낮은 가격 수령 의향)
            Side::Sell => price * (Decimal::ONE - slippage),
        }
    }

    /// 신호 유형이 진입인지 확인.
    pub fn is_entry_signal(signal_type: &SignalType) -> bool {
        matches!(signal_type, SignalType::Entry | SignalType::AddToPosition)
    }

    /// 신호 유형이 청산인지 확인.
    pub fn is_exit_signal(signal_type: &SignalType) -> bool {
        matches!(signal_type, SignalType::Exit | SignalType::ReducePosition)
    }

    /// 매매 방향이 일반적으로 진입을 나타내는지 확인.
    ///
    /// 참고: 이것은 휴리스틱임 - 매수 방향은 일반적으로 롱 포지션의 진입.
    /// 숏의 경우 매도가 진입이 됨. 주의해서 사용할 것.
    pub fn is_entry_signal_from_side(side: Side) -> bool {
        matches!(side, Side::Buy)
    }
}

/// 신호 처리 및 실행 관리를 위한 주문 executor.
///
/// 다음을 통합하는 핵심 컴포넌트:
/// - SignalConverter: 트레이딩 신호를 주문 요청으로 변환
/// - RiskManager: 리스크 한도에 대해 주문 검증
/// - OrderManager: 주문 생명주기 추적 (생성 -> 체결 -> 완료)
/// - PositionTracker: 포지션 관리 및 손익 계산
/// - BracketOrderManager: 브라켓 주문 (손절/익절) 관리
///
/// # 거래소 중립성
/// 이 executor는 거래소에 독립적으로 설계되었습니다.
/// 실제 주문 제출은 호출자가 적절한 거래소 커넥터를 통해 수행해야 합니다.
/// `handle_fill_with_brackets()`는 제출할 브라켓 주문을 반환합니다.
pub struct OrderExecutor {
    /// Signal 변환기
    converter: SignalConverter,
    /// 리스크 관리자
    risk_manager: Arc<RwLock<RiskManager>>,
    /// 주문 추적을 위한 주문 관리자
    order_manager: Arc<RwLock<OrderManager>>,
    /// 포지션 관리를 위한 포지션 추적기
    position_tracker: Arc<RwLock<PositionTracker>>,
    /// 브라켓 주문 관리자
    bracket_manager: Arc<RwLock<BracketOrderManager>>,
    /// 실행 설정
    config: ConversionConfig,
    /// 거래소 식별자
    exchange: String,
}

impl OrderExecutor {
    /// 모든 의존성을 가진 새로운 주문 executor 생성.
    pub fn new(
        risk_manager: Arc<RwLock<RiskManager>>,
        order_manager: Arc<RwLock<OrderManager>>,
        position_tracker: Arc<RwLock<PositionTracker>>,
        config: ConversionConfig,
        exchange: String,
    ) -> Self {
        Self {
            converter: SignalConverter::new(config.clone()),
            risk_manager,
            order_manager,
            position_tracker,
            bracket_manager: Arc::new(RwLock::new(BracketOrderManager::new())),
            config,
            exchange,
        }
    }

    /// 기본 설정으로 생성.
    pub fn with_risk_manager(risk_manager: Arc<RwLock<RiskManager>>, exchange: &str) -> Self {
        let order_manager = Arc::new(RwLock::new(OrderManager::new()));
        let position_tracker = Arc::new(RwLock::new(PositionTracker::new(exchange)));
        Self::new(
            risk_manager,
            order_manager,
            position_tracker,
            ConversionConfig::default(),
            exchange.to_string(),
        )
    }

    /// 모든 관리자를 포함한 완전한 executor 생성.
    pub fn new_complete(
        risk_manager: RiskManager,
        exchange: &str,
        config: ConversionConfig,
    ) -> Self {
        Self::new(
            Arc::new(RwLock::new(risk_manager)),
            Arc::new(RwLock::new(OrderManager::new())),
            Arc::new(RwLock::new(PositionTracker::new(exchange))),
            config,
            exchange.to_string(),
        )
    }

    /// Signal을 처리하고 실행 결과 생성.
    ///
    /// Order를 생성하고, 리스크 관리자로 검증한 후,
    /// OrderManager에 등록함. 실제 거래소 제출은
    /// `submit_order()`를 통해 수행해야 함.
    ///
    /// # 인자
    /// * `signal` - 처리할 트레이딩 신호
    /// * `current_price` - 현재 시장 가격
    pub async fn process_signal(&self, signal: &Signal, current_price: Decimal) -> ExecutionResult {
        // Signal을 주문 요청으로 변환
        let order_request = match self.converter.convert(signal, current_price, None) {
            Ok(o) => o,
            Err(e) => return ExecutionResult::failure(signal.id, e.to_string()),
        };

        // PositionTracker에서 현재 포지션 조회
        let positions: Vec<Position> = {
            let tracker = self.position_tracker.read().await;
            tracker.get_open_positions().into_iter().cloned().collect()
        };

        // 리스크 관리자로 검증
        let mut risk_manager = self.risk_manager.write().await;

        let validation =
            match risk_manager.validate_order(&order_request, &positions, current_price) {
                Ok(v) => v,
                Err(e) => return ExecutionResult::failure(signal.id, e.to_string()),
            };

        if !validation.is_valid {
            // 수정된 주문 제안이 있는지 확인
            if let Some(modified) = validation.modified_order {
                return ExecutionResult::failure(signal.id, validation.messages.join("; "))
                    .with_note(format!("Suggested adjusted order: {:?}", modified));
            }
            return ExecutionResult::failure(signal.id, validation.messages.join("; "));
        }

        drop(risk_manager);

        // OrderRequest에서 Order를 생성하고 OrderManager에 등록
        let order = Order::from_request(order_request.clone(), &self.exchange);
        let order_id = order.id;

        {
            let mut order_manager = self.order_manager.write().await;
            if let Err(e) = order_manager.add_order(order) {
                return ExecutionResult::failure(signal.id, e.to_string());
            }
        }

        // 성공 결과 구성
        let mut result =
            ExecutionResult::success(signal.id, order_request.clone()).with_order_id(order_id);

        // 경고가 있으면 추가
        for msg in validation.messages {
            result = result.with_note(msg);
        }

        // 진입 신호의 경우 설정에 따라 손절 및 익절 생성
        if SignalConverter::is_entry_signal(&signal.signal_type)
            && (self.config.auto_stop_loss || self.config.auto_take_profit) {
                // 브라켓 주문 생성을 위한 임시 포지션 생성
                let mock_position = Position::new(
                    "temp",
                    signal.ticker.clone(),
                    signal.side,
                    order_request.quantity,
                    current_price,
                );

                let risk_manager = self.risk_manager.read().await;

                if self.config.auto_stop_loss {
                    let sl_order = risk_manager.generate_stop_loss(&mock_position, None);
                    result = result.with_stop_loss(sl_order.to_order_request());
                }

                if self.config.auto_take_profit {
                    let tp_order = risk_manager.generate_take_profit(&mock_position, None);
                    result = result.with_take_profit(tp_order.to_order_request());
                }
            }

        // 브라켓 주문 등록 (손절/익절이 있는 경우)
        if let Some(order_id) = result.order_id {
            if result.stop_loss.is_some() || result.take_profit.is_some() {
                let mut bracket_manager = self.bracket_manager.write().await;
                bracket_manager.register_bracket(
                    order_id,
                    result.stop_loss.clone(),
                    result.take_profit.clone(),
                );
            }
        }

        result
    }

    /// 거래소에 주문 제출.
    ///
    /// OrderManager의 주문 상태를 업데이트하며,
    /// 일반적으로 거래소 커넥터에 주문을 전송함.
    ///
    /// # 인자
    /// * `order_id` - 내부 주문 ID
    /// * `exchange_order_id` - 거래소에서 할당한 주문 ID
    pub async fn submit_order(
        &self,
        order_id: Uuid,
        exchange_order_id: String,
    ) -> Result<(), ExecutionError> {
        let mut order_manager = self.order_manager.write().await;

        // 업데이트용 OrderStatus 생성 (거래소 제출됨 = Open)
        let status = OrderStatus {
            order_id: exchange_order_id.clone(),
            client_order_id: None,
            ticker: None,
            side: None,
            quantity: None,
            price: None,
            status: OrderStatusType::Open,
            filled_quantity: Decimal::ZERO,
            average_price: None,
            updated_at: chrono::Utc::now(),
        };

        // 주문 상태 업데이트
        order_manager
            .update_status(order_id, &status)
            .map_err(|e| ExecutionError::ExecutionFailed(e.to_string()))?;

        Ok(())
    }

    /// 거래소로부터 주문 체결 처리.
    ///
    /// 체결 정보로 OrderManager를 업데이트하고
    /// PositionTracker도 함께 업데이트함.
    ///
    /// # 인자
    /// * `order_id` - 내부 주문 ID
    /// * `fill` - 거래소로부터의 체결 정보
    /// * `is_complete` - 이 체결이 주문을 완료하는지 여부
    pub async fn handle_fill(
        &self,
        order_id: Uuid,
        fill: OrderFill,
        _is_complete: bool,
    ) -> Result<(), ExecutionError> {
        // 업데이트 전 주문 조회
        let order = {
            let order_manager = self.order_manager.read().await;
            order_manager.get_order(order_id).cloned().ok_or_else(|| {
                ExecutionError::ExecutionFailed(format!("Order {} not found", order_id))
            })?
        };

        // OrderManager에 체결 기록
        // 참고: record_fill은 완전 체결 시 자동으로 주문 상태를 Filled로 업데이트
        {
            let mut order_manager = self.order_manager.write().await;
            order_manager
                .record_fill(fill.clone())
                .map_err(|e| ExecutionError::ExecutionFailed(e.to_string()))?;
        }

        // 체결에 따라 PositionTracker 업데이트
        {
            let mut position_tracker = self.position_tracker.write().await;

            // apply_fill은 새 포지션과 기존 포지션 모두 처리
            if let Err(e) = position_tracker.apply_fill(&order, &fill) {
                // 오류 로그만 남기고 실패 처리하지 않음 - 포지션이 이미 청산되었을 수 있음
                warn!("Failed to apply fill to position: {}", e);
            }
        }

        Ok(())
    }

    /// 거래소로부터 주문 체결 처리 (브라켓 주문 포함).
    ///
    /// 기본 `handle_fill`에 추가로 브라켓 주문 관리를 수행합니다.
    /// 메인 주문 체결 시 손절/익절 주문을 반환하고,
    /// 손절/익절 체결 시 OCO 취소 대상을 반환합니다.
    ///
    /// # Returns
    /// * `BracketFillResult` - 제출할 브라켓 주문 또는 취소할 주문 정보
    ///
    /// # 거래소 중립성
    /// 반환된 주문은 호출자가 적절한 거래소 커넥터를 통해 제출/취소해야 합니다.
    pub async fn handle_fill_with_brackets(
        &self,
        order_id: Uuid,
        fill: OrderFill,
        is_complete: bool,
    ) -> Result<BracketFillResult, ExecutionError> {
        // 기본 체결 처리
        self.handle_fill(order_id, fill.clone(), is_complete)
            .await?;

        // 완전 체결이 아니면 브라켓 처리 안 함
        if !is_complete {
            return Ok(BracketFillResult::None);
        }

        // 브라켓 주문 확인
        let mut bracket_manager = self.bracket_manager.write().await;

        // 1. 메인 주문 체결인지 확인
        if let Some((stop_loss, take_profit)) = bracket_manager.on_parent_filled(order_id) {
            info!(
                order_id = %order_id,
                has_stop_loss = stop_loss.is_some(),
                has_take_profit = take_profit.is_some(),
                "메인 주문 체결, 브라켓 주문 제출 필요"
            );
            return Ok(BracketFillResult::SubmitBracket {
                parent_order_id: order_id,
                stop_loss,
                take_profit,
            });
        }

        // 2. 자식 주문(손절/익절) 체결인지 확인 -> OCO 처리
        if let Some(cancel_order_id) = bracket_manager.on_child_filled(order_id) {
            info!(
                filled_order_id = %order_id,
                cancel_order_id = %cancel_order_id,
                "브라켓 주문 체결, OCO 취소 필요"
            );
            return Ok(BracketFillResult::CancelOther {
                filled_order_id: order_id,
                cancel_order_id,
            });
        }

        Ok(BracketFillResult::None)
    }

    /// 브라켓 자식 주문 ID 등록.
    ///
    /// 손절/익절 주문을 거래소에 제출한 후 호출하여
    /// 내부 주문 ID를 등록합니다.
    pub async fn register_bracket_child(
        &self,
        parent_order_id: Uuid,
        child_order_id: Uuid,
        is_stop_loss: bool,
    ) {
        let mut bracket_manager = self.bracket_manager.write().await;
        bracket_manager.register_child_order(parent_order_id, child_order_id, is_stop_loss);
    }

    /// 활성 브라켓 주문 수 조회.
    pub async fn active_bracket_count(&self) -> usize {
        let bracket_manager = self.bracket_manager.read().await;
        bracket_manager.active_count()
    }

    /// 주문 취소.
    ///
    /// # 인자
    /// * `order_id` - 내부 주문 ID
    /// * `reason` - 취소 사유 (선택)
    pub async fn cancel_order(
        &self,
        order_id: Uuid,
        reason: Option<String>,
    ) -> Result<(), ExecutionError> {
        let mut order_manager = self.order_manager.write().await;
        order_manager
            .cancel_order(order_id, reason)
            .map_err(|e| ExecutionError::ExecutionFailed(e.to_string()))
    }

    /// 모든 포지션의 시장 가격 업데이트.
    ///
    /// # 인자
    /// * `prices` - 심볼별 현재 가격 맵
    pub async fn update_market_prices(&self, prices: &std::collections::HashMap<String, Decimal>) {
        let mut position_tracker = self.position_tracker.write().await;
        position_tracker.update_prices(prices);
    }

    /// 모든 포지션의 총 미실현 손익 조회.
    pub async fn get_unrealized_pnl(&self) -> Decimal {
        let position_tracker = self.position_tracker.read().await;
        position_tracker.total_unrealized_pnl()
    }

    /// 총 실현 손익 조회.
    pub async fn get_realized_pnl(&self) -> Decimal {
        let position_tracker = self.position_tracker.read().await;
        position_tracker.total_realized_pnl()
    }

    /// 모든 활성 주문 조회.
    pub async fn get_active_orders(&self) -> Vec<Order> {
        let order_manager = self.order_manager.read().await;
        order_manager
            .get_active_orders()
            .into_iter()
            .cloned()
            .collect()
    }

    /// 모든 열린 포지션 조회.
    pub async fn get_open_positions(&self) -> Vec<Position> {
        let position_tracker = self.position_tracker.read().await;
        position_tracker
            .get_open_positions()
            .into_iter()
            .cloned()
            .collect()
    }

    /// ID로 주문 조회.
    pub async fn get_order(&self, order_id: Uuid) -> Option<Order> {
        let order_manager = self.order_manager.read().await;
        order_manager.get_order(order_id).cloned()
    }

    /// 심볼로 포지션 조회.
    pub async fn get_position(&self, symbol: &str) -> Option<Position> {
        let position_tracker = self.position_tracker.read().await;
        position_tracker.get_position_for_symbol(symbol).cloned()
    }

    /// 주문 관리자 참조 조회.
    pub fn order_manager(&self) -> &Arc<RwLock<OrderManager>> {
        &self.order_manager
    }

    /// 포지션 추적기 참조 조회.
    pub fn position_tracker(&self) -> &Arc<RwLock<PositionTracker>> {
        &self.position_tracker
    }

    /// 여러 신호 처리.
    pub async fn process_signals(
        &self,
        signals: &[Signal],
        prices: &std::collections::HashMap<String, Decimal>,
    ) -> Vec<ExecutionResult> {
        let mut results = Vec::with_capacity(signals.len());

        for signal in signals {
            let symbol_str = signal.ticker.clone();
            if let Some(&price) = prices.get(&symbol_str) {
                results.push(self.process_signal(signal, price).await);
            } else {
                results.push(ExecutionResult::failure(
                    signal.id,
                    format!("No price data for symbol: {}", symbol_str),
                ));
            }
        }

        results
    }

    /// 리스크 관리자 잔액 업데이트.
    pub async fn update_balance(&self, balance: Decimal) {
        let mut rm = self.risk_manager.write().await;
        rm.update_balance(balance);
    }

    /// 거래 손익 기록.
    pub async fn record_pnl(&self, symbol: &str, amount: Decimal) {
        let mut rm = self.risk_manager.write().await;
        rm.record_pnl(symbol, amount);
    }

    /// 거래 허용 여부 확인.
    pub async fn can_trade(&self) -> bool {
        let mut rm = self.risk_manager.write().await;
        rm.can_trade()
    }

    /// 거래소 식별자 조회.
    pub fn exchange(&self) -> &str {
        &self.exchange
    }

    /// 실행 설정 조회.
    pub fn config(&self) -> &ConversionConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::prelude::FromPrimitive;

    use trader_risk::RiskConfig;

    /// 정수로부터 Decimal을 생성하는 헬퍼 매크로
    macro_rules! dec {
        ($val:expr) => {
            Decimal::from_f64($val as f64).unwrap()
        };
    }

    fn create_test_signal(side: Side, signal_type: SignalType) -> Signal {
        Signal::new("test_strategy", "BTC/USDT".to_string(), side, signal_type).with_strength(0.8)
    }

    fn create_test_executor(default_quantity: Decimal) -> OrderExecutor {
        let config = RiskConfig::default();
        let risk_manager = RiskManager::new(config, dec!(10000));

        let mut exec_config = ConversionConfig::default();
        exec_config.default_quantity = default_quantity;

        OrderExecutor::new_complete(risk_manager, "test_exchange", exec_config)
    }

    #[test]
    fn test_signal_converter_market_order() {
        let converter = SignalConverter::default_config();
        let signal = create_test_signal(Side::Buy, SignalType::Entry);

        let order = converter
            .convert(&signal, dec!(50000), Some(dec!(0.1)))
            .unwrap();

        assert_eq!(order.side, Side::Buy);
        assert_eq!(order.order_type, OrderType::Market);
        assert_eq!(order.quantity, dec!(0.1));
        assert!(order.price.is_none());
    }

    #[test]
    fn test_signal_converter_limit_order() {
        let mut config = ConversionConfig::default();
        config.use_market_orders = false;
        config.slippage_tolerance_pct = 0.1;

        let converter = SignalConverter::new(config);
        let signal = create_test_signal(Side::Buy, SignalType::Entry);

        let order = converter
            .convert(&signal, dec!(50000), Some(dec!(0.1)))
            .unwrap();

        assert_eq!(order.order_type, OrderType::Limit);
        assert!(order.price.is_some());
        // 매수 주문은 현재가보다 약간 높은 가격이어야 함 (슬리피지 적용)
        assert!(order.price.unwrap() > dec!(50000));
    }

    #[test]
    fn test_signal_converter_exit_order() {
        let converter = SignalConverter::default_config();
        let signal = create_test_signal(Side::Sell, SignalType::Exit);

        let order = converter
            .convert(&signal, dec!(50000), Some(dec!(0.1)))
            .unwrap();

        assert_eq!(order.side, Side::Sell);
        assert_eq!(order.order_type, OrderType::Market);
    }

    #[test]
    fn test_signal_converter_scale_order() {
        let converter = SignalConverter::default_config();
        let signal = create_test_signal(Side::Sell, SignalType::Scale);

        let order = converter
            .convert(&signal, dec!(50000), Some(dec!(0.1)))
            .unwrap();

        assert_eq!(order.side, Side::Sell);
        assert_eq!(order.order_type, OrderType::Market);
    }

    #[test]
    fn test_signal_converter_low_strength_rejected() {
        let mut config = ConversionConfig::default();
        config.min_strength = 0.7;

        let converter = SignalConverter::new(config);
        let mut signal = create_test_signal(Side::Buy, SignalType::Entry);
        signal = signal.with_strength(0.5); // 임계값 미만

        let result = converter.convert(&signal, dec!(50000), Some(dec!(0.1)));

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExecutionError::InvalidSignal(_)
        ));
    }

    #[test]
    fn test_signal_converter_zero_quantity_rejected() {
        let converter = SignalConverter::default_config();
        let signal = create_test_signal(Side::Buy, SignalType::Entry);

        let result = converter.convert(&signal, dec!(50000), Some(Decimal::ZERO));

        assert!(result.is_err());
    }

    #[test]
    fn test_execution_result_builder() {
        let signal_id = Uuid::new_v4();
        let order = OrderRequest::market_buy("BTC/USDT".to_string(), dec!(0.1));
        let order_id = Uuid::new_v4();

        let result = ExecutionResult::success(signal_id, order)
            .with_order_id(order_id)
            .with_note("Test note")
            .with_stop_loss(OrderRequest::market_sell("BTC/USDT".to_string(), dec!(0.1)));

        assert!(result.success);
        assert!(result.order.is_some());
        assert!(result.order_id.is_some());
        assert!(result.stop_loss.is_some());
        assert!(!result.notes.is_empty());
    }

    #[tokio::test]
    async fn test_order_executor_process_signal() {
        let executor = create_test_executor(dec!(0.01));
        let signal = create_test_signal(Side::Buy, SignalType::Entry);

        let result = executor.process_signal(&signal, dec!(50000)).await;

        assert!(result.success);
        assert!(result.order.is_some());
        assert!(result.order_id.is_some()); // 주문이 등록되어야 함

        let order = result.order.unwrap();
        assert_eq!(order.side, Side::Buy);
        assert_eq!(order.quantity, dec!(0.01));
    }

    #[tokio::test]
    async fn test_order_executor_auto_bracket_orders() {
        let config = RiskConfig::default();
        let risk_manager = RiskManager::new(config, dec!(10000));

        let mut exec_config = ConversionConfig::default();
        exec_config.default_quantity = dec!(0.01);
        exec_config.auto_stop_loss = true;
        exec_config.auto_take_profit = true;

        let executor = OrderExecutor::new_complete(risk_manager, "test", exec_config);
        let signal = create_test_signal(Side::Buy, SignalType::Entry);

        let result = executor.process_signal(&signal, dec!(50000)).await;

        assert!(result.success);
        assert!(result.stop_loss.is_some());
        assert!(result.take_profit.is_some());
    }

    #[tokio::test]
    async fn test_order_executor_risk_check_failure() {
        // 포지션 한도를 초과하는 큰 기본 수량 ($10000의 10% = $1000)
        let executor = create_test_executor(dec!(1.0)); // $50000 상당, 10% 한도 초과

        // 과다 매수 시도 (포지션 한도 초과)
        let signal = create_test_signal(Side::Buy, SignalType::Entry);

        let result = executor.process_signal(&signal, dec!(50000)).await;

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_order_executor_order_tracking() {
        let executor = create_test_executor(dec!(0.01));
        let signal = create_test_signal(Side::Buy, SignalType::Entry);

        let result = executor.process_signal(&signal, dec!(50000)).await;
        assert!(result.success);

        let order_id = result.order_id.unwrap();

        // 주문이 추적 가능해야 함
        let order = executor.get_order(order_id).await;
        assert!(order.is_some());

        let order = order.unwrap();
        assert_eq!(order.status, OrderStatusType::Pending);
    }

    #[tokio::test]
    async fn test_order_executor_submit_and_fill() {
        let executor = create_test_executor(dec!(0.01));
        let signal = create_test_signal(Side::Buy, SignalType::Entry);

        // 신호 처리
        let result = executor.process_signal(&signal, dec!(50000)).await;
        assert!(result.success);

        let order_id = result.order_id.unwrap();

        // 주문 제출
        executor
            .submit_order(order_id, "EX123".to_string())
            .await
            .unwrap();

        // 주문이 이제 Open 상태인지 확인
        let order = executor.get_order(order_id).await.unwrap();
        assert_eq!(order.status, OrderStatusType::Open);
        assert_eq!(order.exchange_order_id, Some("EX123".to_string()));

        // 체결 처리
        let fill = OrderFill {
            order_id,
            quantity: dec!(0.01),
            price: dec!(50000),
            commission: Some(dec!(0.5)),
            commission_asset: None,
            timestamp: chrono::Utc::now(),
        };

        executor.handle_fill(order_id, fill, true).await.unwrap();

        // 주문이 Filled 상태인지 확인
        let order = executor.get_order(order_id).await.unwrap();
        assert_eq!(order.status, OrderStatusType::Filled);

        // 포지션이 생성되어야 함
        let position = executor.get_position("BTC/USDT").await;
        assert!(position.is_some());
    }

    #[tokio::test]
    async fn test_order_executor_cancel_order() {
        let executor = create_test_executor(dec!(0.01));
        let signal = create_test_signal(Side::Buy, SignalType::Entry);

        let result = executor.process_signal(&signal, dec!(50000)).await;
        let order_id = result.order_id.unwrap();

        // 주문 제출
        executor
            .submit_order(order_id, "EX123".to_string())
            .await
            .unwrap();

        // 주문 취소
        executor
            .cancel_order(order_id, Some("User requested".to_string()))
            .await
            .unwrap();

        // 주문이 Cancelled 상태인지 확인
        let order = executor.get_order(order_id).await.unwrap();
        assert_eq!(order.status, OrderStatusType::Cancelled);
    }

    #[tokio::test]
    async fn test_order_executor_active_orders() {
        let executor = create_test_executor(dec!(0.01));

        // 여러 주문 생성
        for _ in 0..3 {
            let signal = create_test_signal(Side::Buy, SignalType::Entry);
            let result = executor.process_signal(&signal, dec!(50000)).await;
            let order_id = result.order_id.unwrap();
            executor
                .submit_order(order_id, format!("EX{}", order_id))
                .await
                .unwrap();
        }

        let active_orders = executor.get_active_orders().await;
        assert_eq!(active_orders.len(), 3);
    }

    #[test]
    fn test_is_entry_exit_signal() {
        assert!(SignalConverter::is_entry_signal(&SignalType::Entry));
        assert!(SignalConverter::is_entry_signal(&SignalType::AddToPosition));
        assert!(!SignalConverter::is_entry_signal(&SignalType::Exit));

        assert!(SignalConverter::is_exit_signal(&SignalType::Exit));
        assert!(SignalConverter::is_exit_signal(&SignalType::ReducePosition));
        assert!(!SignalConverter::is_exit_signal(&SignalType::Entry));
    }

    #[test]
    fn test_is_entry_from_side() {
        assert!(SignalConverter::is_entry_signal_from_side(Side::Buy));
        assert!(!SignalConverter::is_entry_signal_from_side(Side::Sell));
    }
}
