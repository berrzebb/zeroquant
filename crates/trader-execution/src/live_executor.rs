//! 실거래 실행기.
//!
//! 실제 거래소에 주문을 제출하는 실행기입니다.
//! `SignalProcessor` trait을 구현하여 `SimulatedExecutor`와 동일한 인터페이스를 제공합니다.
//!
//! # 설계 원칙
//!
//! - **거래소 추상화**: `OrderExecutionProvider` trait을 통해 거래소를 주입받음
//! - **포지션 추적**: 내부 HashMap으로 포지션 상태를 관리 (거래소 상태와 동기화)
//! - **position_id/group_id 지원**: 스프레드/그리드 전략의 분할 매매 구조 완전 지원
//! - **브라켓 주문**: SL/TP 주문을 자동으로 생성하여 거래소에 제출

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

use trader_core::{OrderExecutionProvider, OrderRequest, OrderType, Side, Signal, SignalType, TimeInForce};

use crate::executor::{BracketOrderManager, ConversionConfig};
use crate::signal_processor::{
    apply_slippage, build_add_trade, build_entry_trade, build_exit_trade,
    calculate_position_size, calculate_realized_pnl, determine_close_quantity,
    update_position_average, validate_funds, ProcessorConfig, ProcessorPosition,
    SignalProcessor, SignalProcessorError, TradeResult,
};

/// 실거래 실행기.
///
/// 실제 거래소에 주문을 제출하며, `SignalProcessor` trait을 구현합니다.
/// `SimulatedExecutor`와 동일한 포지션 관리 구조를 사용하여
/// 동일한 전략 코드로 실거래/시뮬레이션 전환이 가능합니다.
///
/// # 구성 요소
///
/// - `order_provider`: 거래소 주문 실행 추상화 (KIS, Binance 등)
/// - `converter`: Signal → OrderRequest 변환기
/// - `bracket_manager`: 브라켓 주문 (SL/TP OCO) 관리자
///
/// # 사용 예시
///
/// ```ignore
/// let kis_provider = Arc::new(KisExchangeProvider::new(client));
/// let executor = LiveExecutor::new(
///     ProcessorConfig::default(),
///     Decimal::from(10_000_000),
///     kis_provider,
/// );
///
/// let result = executor.process_signal(&signal, dec!(50000), Utc::now()).await?;
/// ```
pub struct LiveExecutor {
    // === 공통 필드 (SimulatedExecutor와 동일) ===
    /// Signal 처리 설정 (수수료율, 슬리피지율, 최대 포지션 등)
    config: ProcessorConfig,
    /// 현재 잔고 (내부 추적)
    balance: Decimal,
    /// 초기 잔고
    initial_balance: Decimal,
    /// 포지션 목록 (position_key → ProcessorPosition)
    positions: HashMap<String, ProcessorPosition>,
    /// 거래 기록
    trades: Vec<TradeResult>,
    /// 총 수수료
    total_commission: Decimal,
    /// 총 슬리피지
    total_slippage: Decimal,
    /// 총 주문 수
    total_orders: usize,

    // === LiveExecutor 전용 필드 ===
    /// 거래소 주문 실행 제공자 (거래소 추상화)
    order_provider: Arc<dyn OrderExecutionProvider>,
    /// 브라켓 주문 관리자 (SL/TP OCO)
    bracket_manager: BracketOrderManager,
    /// Signal 변환 설정
    conversion_config: ConversionConfig,
}

impl LiveExecutor {
    /// 새로운 실거래 실행기 생성.
    pub fn new(
        config: ProcessorConfig,
        initial_balance: Decimal,
        order_provider: Arc<dyn OrderExecutionProvider>,
    ) -> Self {
        Self {
            config,
            balance: initial_balance,
            initial_balance,
            positions: HashMap::new(),
            trades: Vec::new(),
            total_commission: Decimal::ZERO,
            total_slippage: Decimal::ZERO,
            total_orders: 0,
            order_provider,
            bracket_manager: BracketOrderManager::new(),
            conversion_config: ConversionConfig::default(),
        }
    }

    /// 변환 설정과 함께 생성.
    pub fn with_conversion_config(
        config: ProcessorConfig,
        initial_balance: Decimal,
        order_provider: Arc<dyn OrderExecutionProvider>,
        conversion_config: ConversionConfig,
    ) -> Self {
        Self {
            config,
            balance: initial_balance,
            initial_balance,
            positions: HashMap::new(),
            trades: Vec::new(),
            total_commission: Decimal::ZERO,
            total_slippage: Decimal::ZERO,
            total_orders: 0,
            order_provider,
            bracket_manager: BracketOrderManager::new(),
            conversion_config,
        }
    }

    /// 설정 조회.
    pub fn config(&self) -> &ProcessorConfig {
        &self.config
    }

    /// 총 슬리피지.
    pub fn total_slippage(&self) -> Decimal {
        self.total_slippage
    }

    /// 총 주문 수.
    pub fn total_orders(&self) -> usize {
        self.total_orders
    }

    /// 거래소 이름.
    pub fn exchange_name(&self) -> &str {
        self.order_provider.exchange_name()
    }

    /// 모든 포지션 강제 청산.
    ///
    /// 실거래에서 모든 보유 포지션에 대해 시장가 청산 주문을 제출합니다.
    ///
    /// # Arguments
    /// * `prices` - 각 심볼의 현재 가격 맵 (슬리피지 계산용)
    /// * `timestamp` - 청산 시간
    ///
    /// # Returns
    /// 청산된 거래 결과 목록
    pub async fn close_all_positions(
        &mut self,
        prices: &HashMap<String, Decimal>,
        timestamp: DateTime<Utc>,
    ) -> Vec<TradeResult> {
        let mut results = Vec::new();

        // 포지션 키 목록 복사 (빌림 충돌 방지)
        let position_keys: Vec<String> = self.positions.keys().cloned().collect();

        for key in position_keys {
            let position = match self.positions.get(&key) {
                Some(p) => p.clone(),
                None => continue,
            };

            let current_price = prices
                .get(&position.symbol)
                .copied()
                .unwrap_or(position.entry_price);

            // 반대 방향으로 청산 주문 생성
            let exit_side = if position.side == Side::Buy {
                Side::Sell
            } else {
                Side::Buy
            };

            // 거래소에 시장가 청산 주문 제출
            let order_request = OrderRequest {
                ticker: position.symbol.clone(),
                side: exit_side,
                order_type: OrderType::Market,
                quantity: position.quantity,
                price: None,
                stop_price: None,
                time_in_force: TimeInForce::GTC,
                client_order_id: Some(format!("close_all_{}", key)),
                strategy_id: None,
            };

            let execution_price = match self.order_provider.place_order(&order_request).await {
                Ok(_response) => {
                    // 거래소 체결가를 사용해야 하지만, 현재 OrderResponse에는 체결가가 없음
                    // 현재가에 슬리피지를 적용하여 추정
                    apply_slippage(current_price, self.config.slippage_rate, exit_side)
                }
                Err(e) => {
                    warn!("청산 주문 실패: {} - {}", key, e);
                    continue;
                }
            };

            // 수수료 계산
            let close_value = execution_price * position.quantity;
            let commission = close_value * self.config.commission_rate;

            // 실현 손익 계산 (공통 유틸리티)
            let realized_pnl = calculate_realized_pnl(
                position.entry_price,
                execution_price,
                position.quantity,
                commission,
                position.side,
            );

            // 잔고 업데이트
            self.balance += close_value - commission;
            self.total_commission += commission;
            self.total_orders += 1;

            // 포지션 제거
            self.positions.remove(&key);

            // 거래 기록 생성
            let trade = TradeResult {
                symbol: position.symbol.clone(),
                side: exit_side,
                signal_type: SignalType::Exit,
                quantity: position.quantity,
                price: execution_price,
                commission,
                slippage: Decimal::ZERO,
                timestamp,
                realized_pnl: Some(realized_pnl),
                is_partial: false,
                metadata: {
                    let mut map = HashMap::new();
                    map.insert("reason".to_string(), "close_all".to_string());
                    if let Some(pos_id) = &position.position_id {
                        map.insert("position_id".to_string(), pos_id.clone());
                    }
                    map
                },
            };

            info!(
                "[{}] 전체 청산: {} {} @ {} (PnL: {:?})",
                self.order_provider.exchange_name(),
                trade.symbol,
                trade.side,
                trade.price,
                trade.realized_pnl
            );

            self.trades.push(trade.clone());
            results.push(trade);
        }

        results
    }

    /// 포지션 열기 (내부 메서드).
    ///
    /// Signal을 OrderRequest로 변환하여 거래소에 제출하고,
    /// 체결 후 내부 포지션 상태를 업데이트합니다.
    async fn open_position_internal(
        &mut self,
        signal: &Signal,
        current_price: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<TradeResult>, SignalProcessorError> {
        let key = signal.position_key();

        // 이미 포지션이 있는 경우 - 분할 매수 처리
        if self.positions.contains_key(&key) {
            if signal.signal_type == SignalType::AddToPosition {
                return self
                    .add_to_position_internal(signal, current_price, timestamp)
                    .await;
            }
            // 일반 Entry 신호는 무시 (이미 포지션이 있음)
            return Ok(None);
        }

        // 최대 포지션 수 확인
        if self.positions.len() >= self.config.max_positions {
            return Err(SignalProcessorError::MaxPositionsExceeded {
                max: self.config.max_positions,
            });
        }

        // 포지션 크기 계산 (공통 유틸리티)
        let price = signal.suggested_price.unwrap_or(current_price);
        let (position_amount, quantity) = calculate_position_size(
            self.balance,
            self.config.max_position_size_pct,
            signal.strength,
            price,
        );

        // 자금 검증 (공통 유틸리티)
        let _ = validate_funds(
            position_amount,
            self.config.commission_rate,
            self.balance,
        )?;

        // Signal → OrderRequest 변환 후 거래소에 제출
        let order_request = OrderRequest {
            ticker: signal.ticker.clone(),
            side: signal.side,
            order_type: if self.conversion_config.use_market_orders {
                OrderType::Market
            } else {
                OrderType::Limit
            },
            quantity,
            price: if self.conversion_config.use_market_orders {
                None
            } else {
                Some(price)
            },
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: Some(format!("sig_{}", signal.id)),
            strategy_id: Some(signal.strategy_id.clone()),
        };

        let _order_response = self
            .order_provider
            .place_order(&order_request)
            .await
            .map_err(|e| SignalProcessorError::ExchangeError(e.to_string()))?;

        // 체결 가격 추정 (거래소 체결가를 사용해야 하지만 OrderResponse에 체결가 없음)
        let execution_price = apply_slippage(price, self.config.slippage_rate, signal.side);

        // 실제 수수료 계산
        let actual_amount = execution_price * quantity;
        let commission = actual_amount * self.config.commission_rate;

        // 잔고 차감
        self.balance -= actual_amount + commission;
        self.total_commission += commission;
        let slippage_amount = (execution_price - price).abs() * quantity;
        self.total_slippage += slippage_amount;
        self.total_orders += 1;

        // 포지션 생성
        self.positions.insert(
            key.clone(),
            ProcessorPosition {
                symbol: signal.ticker.clone(),
                side: signal.side,
                quantity,
                entry_price: execution_price,
                entry_time: timestamp,
                fees: commission,
                position_id: signal.position_id.clone(),
                group_id: signal.group_id.clone(),
            },
        );

        // 브라켓 주문 생성 (SL/TP 자동 설정 시)
        if self.conversion_config.auto_stop_loss || self.conversion_config.auto_take_profit {
            self.create_bracket_orders(signal, execution_price, quantity)
                .await;
        }

        // 거래 기록 생성 (공통 유틸리티)
        let trade = build_entry_trade(
            signal, quantity, execution_price, commission, slippage_amount, timestamp,
        );

        info!(
            "[{}] 진입: {} {:?} {} @ {} (잔고: {})",
            self.order_provider.exchange_name(),
            signal.ticker,
            signal.side,
            quantity,
            execution_price,
            self.balance
        );

        self.trades.push(trade.clone());
        Ok(Some(trade))
    }

    /// 포지션 추가 (분할 매수).
    async fn add_to_position_internal(
        &mut self,
        signal: &Signal,
        current_price: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<TradeResult>, SignalProcessorError> {
        let key = signal.position_key();
        let price = signal.suggested_price.unwrap_or(current_price);

        // 포지션 크기 계산 (공통 유틸리티)
        let (position_amount, add_quantity) = calculate_position_size(
            self.balance,
            self.config.max_position_size_pct,
            signal.strength,
            price,
        );

        // 자금 검증 (공통 유틸리티)
        let commission = validate_funds(
            position_amount,
            self.config.commission_rate,
            self.balance,
        )?;

        // 거래소에 주문 제출
        let order_request = OrderRequest {
            ticker: signal.ticker.clone(),
            side: signal.side,
            order_type: if self.conversion_config.use_market_orders {
                OrderType::Market
            } else {
                OrderType::Limit
            },
            quantity: add_quantity,
            price: if self.conversion_config.use_market_orders {
                None
            } else {
                Some(price)
            },
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: Some(format!("sig_add_{}", signal.id)),
            strategy_id: Some(signal.strategy_id.clone()),
        };

        self.order_provider
            .place_order(&order_request)
            .await
            .map_err(|e| SignalProcessorError::ExchangeError(e.to_string()))?;

        let execution_price = apply_slippage(price, self.config.slippage_rate, signal.side);

        // 평균 단가 재계산 (공통 유틸리티)
        if let Some(existing) = self.positions.get_mut(&key) {
            update_position_average(existing, add_quantity, execution_price, commission);
        }

        // 잔고 차감
        self.balance -= position_amount + commission;
        self.total_commission += commission;
        self.total_orders += 1;

        // 거래 기록 생성 (공통 유틸리티)
        let trade = build_add_trade(signal, add_quantity, execution_price, commission, timestamp);

        info!(
            "[{}] 추가 매수: {} {} @ {} (총 수량 업데이트)",
            self.order_provider.exchange_name(),
            signal.ticker,
            add_quantity,
            execution_price
        );

        self.trades.push(trade.clone());
        Ok(Some(trade))
    }

    /// 포지션 닫기 (내부 메서드).
    async fn close_position_internal(
        &mut self,
        signal: &Signal,
        current_price: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<TradeResult>, SignalProcessorError> {
        let key = signal.position_key();

        let position = match self.positions.get(&key) {
            Some(p) => p.clone(),
            None => {
                // 포지션이 없으면 무시 (에러가 아님)
                return Ok(None);
            }
        };

        let price = signal.suggested_price.unwrap_or(current_price);

        // 청산 수량 결정 (공통 유틸리티)
        let close_quantity = determine_close_quantity(signal, position.quantity);

        // 거래소에 청산 주문 제출
        let order_request = OrderRequest {
            ticker: signal.ticker.clone(),
            side: signal.side,
            order_type: OrderType::Market, // 청산은 시장가
            quantity: close_quantity,
            price: None,
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: Some(format!("sig_exit_{}", signal.id)),
            strategy_id: Some(signal.strategy_id.clone()),
        };

        self.order_provider
            .place_order(&order_request)
            .await
            .map_err(|e| SignalProcessorError::ExchangeError(e.to_string()))?;

        let execution_price = apply_slippage(price, self.config.slippage_rate, signal.side);

        // 청산 금액 계산
        let close_value = execution_price * close_quantity;
        let commission = close_value * self.config.commission_rate;

        // 실현 손익 계산 (공통 유틸리티)
        let realized_pnl = calculate_realized_pnl(
            position.entry_price,
            execution_price,
            close_quantity,
            commission,
            position.side,
        );

        // 잔고 업데이트
        self.balance += close_value - commission;
        self.total_commission += commission;
        self.total_orders += 1;

        // 포지션 업데이트 또는 제거
        if close_quantity >= position.quantity {
            self.positions.remove(&key);
        } else if let Some(pos) = self.positions.get_mut(&key) {
            pos.quantity -= close_quantity;
        }

        // 거래 기록 생성 (공통 유틸리티)
        let trade = build_exit_trade(
            &position.symbol,
            signal,
            close_quantity,
            execution_price,
            commission,
            realized_pnl,
            position.quantity,
            timestamp,
        );

        info!(
            "[{}] 청산: {} {:?} {} @ {} (PnL: {:?})",
            self.order_provider.exchange_name(),
            position.symbol,
            signal.side,
            close_quantity,
            execution_price,
            realized_pnl
        );

        self.trades.push(trade.clone());
        Ok(Some(trade))
    }

    /// 브라켓 주문 생성 (SL/TP 자동 생성).
    async fn create_bracket_orders(
        &mut self,
        signal: &Signal,
        entry_price: Decimal,
        quantity: Decimal,
    ) {
        let exit_side = if signal.side == Side::Buy {
            Side::Sell
        } else {
            Side::Buy
        };

        // 손절 주문 생성
        let stop_loss = if self.conversion_config.auto_stop_loss {
            let sl_price = if signal.side == Side::Buy {
                entry_price * (Decimal::ONE - Decimal::new(5, 2)) // 기본 5% 손절
            } else {
                entry_price * (Decimal::ONE + Decimal::new(5, 2))
            };

            let sl_order = OrderRequest {
                ticker: signal.ticker.clone(),
                side: exit_side,
                order_type: OrderType::StopLoss,
                quantity,
                price: None,
                stop_price: Some(sl_price),
                time_in_force: TimeInForce::GTC,
                client_order_id: Some(format!("sl_{}", signal.id)),
                strategy_id: Some(signal.strategy_id.clone()),
            };

            // 거래소에 SL 주문 제출
            match self.order_provider.place_order(&sl_order).await {
                Ok(response) => {
                    debug!("SL 주문 제출 완료: {} @ {}", response.order_no, sl_price);
                    Some(sl_order)
                }
                Err(e) => {
                    warn!("SL 주문 제출 실패: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // 익절 주문 생성
        let take_profit = if self.conversion_config.auto_take_profit {
            let tp_price = if signal.side == Side::Buy {
                entry_price * (Decimal::ONE + Decimal::new(10, 2)) // 기본 10% 익절
            } else {
                entry_price * (Decimal::ONE - Decimal::new(10, 2))
            };

            let tp_order = OrderRequest {
                ticker: signal.ticker.clone(),
                side: exit_side,
                order_type: OrderType::Limit,
                quantity,
                price: Some(tp_price),
                stop_price: None,
                time_in_force: TimeInForce::GTC,
                client_order_id: Some(format!("tp_{}", signal.id)),
                strategy_id: Some(signal.strategy_id.clone()),
            };

            // 거래소에 TP 주문 제출
            match self.order_provider.place_order(&tp_order).await {
                Ok(response) => {
                    debug!("TP 주문 제출 완료: {} @ {}", response.order_no, tp_price);
                    Some(tp_order)
                }
                Err(e) => {
                    warn!("TP 주문 제출 실패: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // 브라켓 등록 (SL 또는 TP 중 하나라도 있으면)
        if stop_loss.is_some() || take_profit.is_some() {
            self.bracket_manager
                .register_bracket(uuid::Uuid::new_v4(), stop_loss, take_profit);
        }
    }
}

#[async_trait]
impl SignalProcessor for LiveExecutor {
    async fn process_signal(
        &mut self,
        signal: &Signal,
        current_price: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<TradeResult>, SignalProcessorError> {
        // min_strength 필터
        if signal.strength < self.conversion_config.min_strength {
            debug!(
                "Signal 강도 부족: {:.2} < {:.2} ({})",
                signal.strength, self.conversion_config.min_strength, signal.ticker
            );
            return Ok(None);
        }

        match signal.signal_type {
            SignalType::Entry | SignalType::AddToPosition => {
                // 숏 포지션 확인
                if signal.side == Side::Sell && !self.config.allow_short {
                    return Err(SignalProcessorError::ShortNotAllowed);
                }
                self.open_position_internal(signal, current_price, timestamp)
                    .await
            }
            SignalType::Exit | SignalType::ReducePosition => {
                self.close_position_internal(signal, current_price, timestamp)
                    .await
            }
            SignalType::Scale => {
                // 스케일 신호는 현재 포지션에 따라 처리
                let key = signal.position_key();
                if self.positions.contains_key(&key) {
                    self.close_position_internal(signal, current_price, timestamp)
                        .await
                } else {
                    if signal.side == Side::Sell && !self.config.allow_short {
                        return Err(SignalProcessorError::ShortNotAllowed);
                    }
                    self.open_position_internal(signal, current_price, timestamp)
                        .await
                }
            }
            SignalType::Alert => {
                // Alert는 실행하지 않음
                Ok(None)
            }
        }
    }

    fn balance(&self) -> Decimal {
        self.balance
    }

    fn positions(&self) -> &HashMap<String, ProcessorPosition> {
        &self.positions
    }

    fn trades(&self) -> &[TradeResult] {
        &self.trades
    }

    fn total_commission(&self) -> Decimal {
        self.total_commission
    }

    fn reset(&mut self, initial_balance: Decimal) {
        self.balance = initial_balance;
        self.initial_balance = initial_balance;
        self.positions.clear();
        self.trades.clear();
        self.total_commission = Decimal::ZERO;
        self.total_slippage = Decimal::ZERO;
        self.total_orders = 0;
        self.bracket_manager = BracketOrderManager::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use trader_core::{OrderResponse, ProviderError};

    /// 테스트용 Mock 주문 제공자.
    struct MockOrderProvider {
        should_fail: bool,
    }

    #[async_trait]
    impl OrderExecutionProvider for MockOrderProvider {
        async fn place_order(
            &self,
            _request: &OrderRequest,
        ) -> Result<OrderResponse, ProviderError> {
            if self.should_fail {
                return Err(ProviderError::Api("Mock: 주문 실패".to_string()));
            }
            Ok(OrderResponse {
                order_no: "MOCK_001".to_string(),
                order_time: "090000".to_string(),
            })
        }

        async fn cancel_order(&self, _order_id: &str, _ticker: &str) -> Result<(), ProviderError> {
            Ok(())
        }

        async fn modify_order(
            &self,
            _order_id: &str,
            _ticker: &str,
            _quantity: Option<Decimal>,
            _price: Option<Decimal>,
        ) -> Result<OrderResponse, ProviderError> {
            Ok(OrderResponse {
                order_no: "MOCK_002".to_string(),
                order_time: "090001".to_string(),
            })
        }

        fn exchange_name(&self) -> &str {
            "MockExchange"
        }
    }

    fn create_test_signal(ticker: &str, side: Side, signal_type: SignalType) -> Signal {
        Signal::new("test_strategy", ticker.to_string(), side, signal_type)
    }

    fn create_mock_executor(should_fail: bool) -> LiveExecutor {
        let provider = Arc::new(MockOrderProvider { should_fail });
        // min_strength를 0.0으로 설정하여 모든 신호 통과
        let conversion_config = ConversionConfig {
            min_strength: 0.0,
            auto_stop_loss: false,
            auto_take_profit: false,
            ..ConversionConfig::default()
        };
        LiveExecutor::with_conversion_config(
            ProcessorConfig::default(),
            dec!(10_000_000),
            provider,
            conversion_config,
        )
    }

    #[tokio::test]
    async fn test_open_position() {
        let mut executor = create_mock_executor(false);

        let signal = create_test_signal("005930", Side::Buy, SignalType::Entry).with_strength(0.5);
        let result = executor
            .process_signal(&signal, dec!(50000), Utc::now())
            .await;

        assert!(result.is_ok());
        let trade = result.unwrap();
        assert!(trade.is_some());
        assert_eq!(executor.positions().len(), 1);
    }

    #[tokio::test]
    async fn test_close_position() {
        let mut executor = create_mock_executor(false);

        // 매수
        let buy_signal =
            create_test_signal("005930", Side::Buy, SignalType::Entry).with_strength(0.5);
        executor
            .process_signal(&buy_signal, dec!(50000), Utc::now())
            .await
            .unwrap();

        assert_eq!(executor.positions().len(), 1);

        // 매도
        let sell_signal = create_test_signal("005930", Side::Sell, SignalType::Exit);
        let result = executor
            .process_signal(&sell_signal, dec!(51000), Utc::now())
            .await;

        assert!(result.is_ok());
        let trade = result.unwrap().unwrap();
        assert!(trade.realized_pnl.is_some());
        assert!(executor.positions().is_empty());
    }

    #[tokio::test]
    async fn test_position_id_independent() {
        let mut executor = create_mock_executor(false);

        // 그리드 레벨 1 진입
        let signal1 = create_test_signal("005930", Side::Buy, SignalType::Entry)
            .with_strength(0.3)
            .with_position_id("005930_grid_L1".to_string())
            .with_group_id("grid_session_1".to_string());

        executor
            .process_signal(&signal1, dec!(50000), Utc::now())
            .await
            .unwrap();

        // 그리드 레벨 2 진입
        let signal2 = create_test_signal("005930", Side::Buy, SignalType::Entry)
            .with_strength(0.3)
            .with_position_id("005930_grid_L2".to_string())
            .with_group_id("grid_session_1".to_string());

        executor
            .process_signal(&signal2, dec!(49000), Utc::now())
            .await
            .unwrap();

        assert_eq!(executor.positions().len(), 2);

        // 그룹 조회
        let group_positions = executor.positions_by_group("grid_session_1");
        assert_eq!(group_positions.len(), 2);

        // 레벨 1만 청산
        let exit_signal = create_test_signal("005930", Side::Sell, SignalType::Exit)
            .with_position_id("005930_grid_L1".to_string());

        executor
            .process_signal(&exit_signal, dec!(51000), Utc::now())
            .await
            .unwrap();

        assert_eq!(executor.positions().len(), 1);
        assert!(executor.has_position("005930_grid_L2"));
    }

    #[tokio::test]
    async fn test_exchange_error() {
        let mut executor = create_mock_executor(true); // 실패 모드

        let signal = create_test_signal("005930", Side::Buy, SignalType::Entry).with_strength(0.5);
        let result = executor
            .process_signal(&signal, dec!(50000), Utc::now())
            .await;

        assert!(matches!(
            result,
            Err(SignalProcessorError::ExchangeError(_))
        ));
        assert!(executor.positions().is_empty());
    }

    #[tokio::test]
    async fn test_min_strength_filter() {
        let provider = Arc::new(MockOrderProvider { should_fail: false });
        let conversion_config = ConversionConfig {
            min_strength: 0.5,
            ..ConversionConfig::default()
        };
        let mut executor = LiveExecutor::with_conversion_config(
            ProcessorConfig::default(),
            dec!(10_000_000),
            provider,
            conversion_config,
        );

        // 강도 부족 (0.3 < 0.5)
        let signal = create_test_signal("005930", Side::Buy, SignalType::Entry).with_strength(0.3);
        let result = executor
            .process_signal(&signal, dec!(50000), Utc::now())
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none()); // 필터링됨
        assert!(executor.positions().is_empty());
    }

    #[tokio::test]
    async fn test_short_not_allowed() {
        let mut executor = create_mock_executor(false);

        let signal = create_test_signal("005930", Side::Sell, SignalType::Entry).with_strength(0.5);
        let result = executor
            .process_signal(&signal, dec!(50000), Utc::now())
            .await;

        assert!(matches!(result, Err(SignalProcessorError::ShortNotAllowed)));
    }

    #[tokio::test]
    async fn test_close_all_positions() {
        let mut executor = create_mock_executor(false);

        // 2개 포지션 열기
        let signal1 =
            create_test_signal("005930", Side::Buy, SignalType::Entry).with_strength(0.3);
        executor
            .process_signal(&signal1, dec!(50000), Utc::now())
            .await
            .unwrap();

        let signal2 =
            create_test_signal("035720", Side::Buy, SignalType::Entry).with_strength(0.3);
        executor
            .process_signal(&signal2, dec!(100000), Utc::now())
            .await
            .unwrap();

        assert_eq!(executor.positions().len(), 2);

        // 전체 청산
        let mut prices = HashMap::new();
        prices.insert("005930".to_string(), dec!(52000));
        prices.insert("035720".to_string(), dec!(105000));

        let results = executor.close_all_positions(&prices, Utc::now()).await;

        assert_eq!(results.len(), 2);
        assert!(executor.positions().is_empty());
    }

    #[tokio::test]
    async fn test_total_equity() {
        let mut executor = create_mock_executor(false);

        let signal =
            create_test_signal("005930", Side::Buy, SignalType::Entry).with_strength(0.5);
        executor
            .process_signal(&signal, dec!(50000), Utc::now())
            .await
            .unwrap();

        // 가격 상승 시 총 자산 확인
        let mut prices = HashMap::new();
        prices.insert("005930".to_string(), dec!(55000)); // 10% 상승

        let equity = executor.total_equity(&prices);
        assert!(equity > dec!(10_000_000)); // 수익 발생
    }

    #[tokio::test]
    async fn test_add_to_position() {
        let mut executor = create_mock_executor(false);

        // 첫 매수
        let signal1 =
            create_test_signal("005930", Side::Buy, SignalType::Entry).with_strength(0.3);
        executor
            .process_signal(&signal1, dec!(50000), Utc::now())
            .await
            .unwrap();

        let initial_price = executor.positions().get("005930").unwrap().entry_price;

        // 분할 매수
        let signal2 =
            create_test_signal("005930", Side::Buy, SignalType::AddToPosition).with_strength(0.3);
        executor
            .process_signal(&signal2, dec!(48000), Utc::now())
            .await
            .unwrap();

        // 여전히 1개 포지션
        assert_eq!(executor.positions().len(), 1);

        // 평균 단가 변경 확인
        let avg_price = executor.positions().get("005930").unwrap().entry_price;
        assert!(avg_price < initial_price); // 낮은 가격에 추가 매수 → 평균 단가 하락
    }
}
