//! 시뮬레이션 실행기.
//!
//! 백테스트와 페이퍼 트레이딩에서 사용하는 가상 체결 실행기입니다.
//! SignalProcessor trait을 구현하여 실거래와 동일한 인터페이스를 제공합니다.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;

use trader_core::{Side, Signal, SignalType};

use crate::signal_processor::{
    apply_slippage, build_add_trade, build_entry_trade, build_exit_trade,
    calculate_position_size, calculate_realized_pnl, determine_close_quantity,
    update_position_average, validate_funds, ProcessorConfig, ProcessorPosition,
    SignalProcessor, SignalProcessorError, TradeResult,
};

/// 브라켓 주문 시뮬레이션 정보.
///
/// 시뮬레이션에서 SL/TP 가격 도달 시 자동 청산을 처리하기 위한 내부 추적용입니다.
#[derive(Debug, Clone)]
pub struct BracketSimulation {
    /// 손절 가격 (None이면 SL 미설정)
    pub stop_loss_price: Option<Decimal>,
    /// 익절 가격 (None이면 TP 미설정)
    pub take_profit_price: Option<Decimal>,
    /// 포지션 방향
    pub side: Side,
}

/// 시뮬레이션 실행기
///
/// 백테스트와 페이퍼 트레이딩에서 가상 체결을 수행합니다.
/// 실제 거래소 API를 호출하지 않고, 내부 상태만 업데이트합니다.
#[derive(Debug)]
pub struct SimulatedExecutor {
    /// 설정
    config: ProcessorConfig,
    /// 현재 잔고
    balance: Decimal,
    /// 초기 잔고
    initial_balance: Decimal,
    /// 포지션 목록
    positions: HashMap<String, ProcessorPosition>,
    /// 거래 기록
    trades: Vec<TradeResult>,
    /// 총 수수료
    total_commission: Decimal,
    /// 총 슬리피지
    total_slippage: Decimal,
    /// 총 주문 수
    total_orders: usize,
    /// 브라켓 주문 추적 (position_key → (SL가격, TP가격))
    /// 시뮬레이션에서 SL/TP 트리거를 확인하기 위한 내부 추적용
    bracket_orders: HashMap<String, BracketSimulation>,
}

impl SimulatedExecutor {
    /// 새로운 시뮬레이션 실행기 생성
    pub fn new(config: ProcessorConfig, initial_balance: Decimal) -> Self {
        Self {
            config,
            balance: initial_balance,
            initial_balance,
            positions: HashMap::new(),
            trades: Vec::new(),
            total_commission: Decimal::ZERO,
            total_slippage: Decimal::ZERO,
            total_orders: 0,
            bracket_orders: HashMap::new(),
        }
    }

    /// 기본 설정으로 생성
    pub fn with_balance(initial_balance: Decimal) -> Self {
        Self::new(ProcessorConfig::default(), initial_balance)
    }

    /// 설정 조회
    pub fn config(&self) -> &ProcessorConfig {
        &self.config
    }

    /// 총 슬리피지
    pub fn total_slippage(&self) -> Decimal {
        self.total_slippage
    }

    /// 총 주문 수
    pub fn total_orders(&self) -> usize {
        self.total_orders
    }

    /// 모든 포지션 강제 청산 (시뮬레이션/백테스트 종료 시 사용)
    ///
    /// 시뮬레이션이나 백테스트가 종료될 때 남아있는 모든 포지션을
    /// 주어진 가격으로 청산합니다.
    ///
    /// # Arguments
    /// * `prices` - 각 심볼의 현재 가격 맵
    /// * `timestamp` - 청산 시간
    ///
    /// # Returns
    /// 청산된 거래 결과 목록
    pub fn close_all_positions(
        &mut self,
        prices: &HashMap<String, Decimal>,
        timestamp: DateTime<Utc>,
    ) -> Vec<TradeResult> {
        let mut results = Vec::new();

        // 포지션 키 목록 복사 (빌림 충돌 방지)
        let position_keys: Vec<String> = self.positions.keys().cloned().collect();

        for key in position_keys {
            // 포지션 정보 복사
            let position = match self.positions.get(&key) {
                Some(p) => p.clone(),
                None => continue,
            };

            // 현재 가격 조회 (없으면 진입가 사용)
            let current_price = prices
                .get(&position.symbol)
                .copied()
                .unwrap_or(position.entry_price);

            // 슬리피지 적용
            let exit_side = if position.side == Side::Buy {
                Side::Sell
            } else {
                Side::Buy
            };
            let execution_price =
                apply_slippage(current_price, self.config.slippage_rate, exit_side);

            // 청산 금액 및 수수료 계산
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
                signal_type: SignalType::Exit, // 시뮬레이션 종료 시 강제 청산
                quantity: position.quantity,
                price: execution_price,
                commission,
                slippage: Decimal::ZERO,
                timestamp,
                realized_pnl: Some(realized_pnl),
                is_partial: false,
                metadata: {
                    let mut map = HashMap::new();
                    map.insert("reason".to_string(), "simulation_end".to_string());
                    if let Some(pos_id) = &position.position_id {
                        map.insert("position_id".to_string(), pos_id.clone());
                    }
                    map
                },
            };

            self.trades.push(trade.clone());
            results.push(trade);
        }

        results
    }

    /// 포지션 열기 (내부 메서드)
    fn open_position_internal(
        &mut self,
        signal: &Signal,
        current_price: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<TradeResult>, SignalProcessorError> {
        let key = signal.position_key();

        // 실행 가격 계산 (슬리피지 적용)
        let price = signal.suggested_price.unwrap_or(current_price);
        let execution_price = apply_slippage(price, self.config.slippage_rate, signal.side);

        // 유효하지 않은 가격 체크
        if execution_price <= Decimal::ZERO {
            return Err(SignalProcessorError::InvalidPrice {
                price: execution_price,
            });
        }

        // 이미 포지션이 있는 경우 - AddToPosition만 허용
        if self.positions.contains_key(&key) {
            if signal.signal_type == SignalType::AddToPosition {
                return self.add_to_position_internal(signal, execution_price, timestamp);
            }
            return Ok(None);
        }

        // 최대 포지션 수 확인
        if self.positions.len() >= self.config.max_positions {
            return Err(SignalProcessorError::MaxPositionsExceeded {
                max: self.config.max_positions,
            });
        }

        // 포지션 크기 계산 (공통 유틸리티)
        let (position_amount, quantity) = calculate_position_size(
            self.balance,
            self.config.max_position_size_pct,
            signal.strength,
            execution_price,
        );

        // 자금 검증 (공통 유틸리티)
        let commission = validate_funds(
            position_amount,
            self.config.commission_rate,
            self.balance,
        )?;

        // 잔고 차감
        let required = position_amount + commission;
        self.balance -= required;
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

        // 거래 기록 생성 (공통 유틸리티)
        let trade = build_entry_trade(
            signal, quantity, execution_price, commission, slippage_amount, timestamp,
        );
        self.trades.push(trade.clone());

        // 브라켓 주문 생성 (SL/TP 시뮬레이션)
        if self.config.auto_stop_loss || self.config.auto_take_profit {
            let sl_price = if self.config.auto_stop_loss {
                Some(if signal.side == Side::Buy {
                    execution_price * (Decimal::ONE - self.config.stop_loss_pct)
                } else {
                    execution_price * (Decimal::ONE + self.config.stop_loss_pct)
                })
            } else {
                None
            };

            let tp_price = if self.config.auto_take_profit {
                Some(if signal.side == Side::Buy {
                    execution_price * (Decimal::ONE + self.config.take_profit_pct)
                } else {
                    execution_price * (Decimal::ONE - self.config.take_profit_pct)
                })
            } else {
                None
            };

            self.bracket_orders.insert(
                key,
                BracketSimulation {
                    stop_loss_price: sl_price,
                    take_profit_price: tp_price,
                    side: signal.side,
                },
            );
        }

        Ok(Some(trade))
    }

    /// 포지션 추가 (분할 매수)
    fn add_to_position_internal(
        &mut self,
        signal: &Signal,
        execution_price: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<TradeResult>, SignalProcessorError> {
        let key = signal.position_key();

        // 포지션 크기 계산 (공통 유틸리티)
        let (position_amount, add_quantity) = calculate_position_size(
            self.balance,
            self.config.max_position_size_pct,
            signal.strength,
            execution_price,
        );

        // 자금 검증 (공통 유틸리티)
        let commission = validate_funds(
            position_amount,
            self.config.commission_rate,
            self.balance,
        )?;

        // 평균 단가 재계산 (공통 유틸리티)
        if let Some(existing) = self.positions.get_mut(&key) {
            update_position_average(existing, add_quantity, execution_price, commission);
        }

        // 잔고 차감
        let required = position_amount + commission;
        self.balance -= required;
        self.total_commission += commission;
        self.total_orders += 1;

        // 거래 기록 생성 (공통 유틸리티)
        let trade = build_add_trade(signal, add_quantity, execution_price, commission, timestamp);
        self.trades.push(trade.clone());

        Ok(Some(trade))
    }

    /// 포지션 닫기 (내부 메서드)
    fn close_position_internal(
        &mut self,
        signal: &Signal,
        current_price: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<TradeResult>, SignalProcessorError> {
        let key = signal.position_key();

        let position = match self.positions.get(&key) {
            Some(p) => p.clone(),
            None => return Ok(None),
        };

        // 실행 가격 계산 (슬리피지 적용)
        let price = signal.suggested_price.unwrap_or(current_price);
        let execution_price = apply_slippage(price, self.config.slippage_rate, signal.side);

        if execution_price <= Decimal::ZERO {
            return Err(SignalProcessorError::InvalidPrice {
                price: execution_price,
            });
        }

        // 청산 수량 결정 (공통 유틸리티)
        let close_quantity = determine_close_quantity(signal, position.quantity);

        // 청산 금액 및 수수료 계산
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
            self.bracket_orders.remove(&key);
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
        self.trades.push(trade.clone());

        Ok(Some(trade))
    }

    /// 브라켓 주문 트리거 확인.
    ///
    /// 현재 가격을 기준으로 SL/TP 도달 여부를 확인하여
    /// 자동 청산 Signal 목록을 반환합니다.
    ///
    /// # Arguments
    /// * `current_prices` - 각 심볼의 현재 가격 맵
    ///
    /// # Returns
    /// 트리거된 청산 Signal 목록 (호출자가 process_signal에 전달해야 함)
    pub fn check_bracket_triggers(
        &self,
        current_prices: &HashMap<String, Decimal>,
    ) -> Vec<(String, String)> {
        let mut triggered = Vec::new();

        for (key, bracket) in &self.bracket_orders {
            let position = match self.positions.get(key) {
                Some(p) => p,
                None => continue,
            };

            let current_price = match current_prices.get(&position.symbol) {
                Some(p) => *p,
                None => continue,
            };

            // SL 트리거 확인
            if let Some(sl_price) = bracket.stop_loss_price {
                let triggered_sl = match bracket.side {
                    Side::Buy => current_price <= sl_price,  // 롱: 가격 하락 시 SL
                    Side::Sell => current_price >= sl_price, // 숏: 가격 상승 시 SL
                };
                if triggered_sl {
                    triggered.push((key.clone(), "stop_loss".to_string()));
                    continue; // SL이 트리거되면 TP는 무시 (OCO)
                }
            }

            // TP 트리거 확인
            if let Some(tp_price) = bracket.take_profit_price {
                let triggered_tp = match bracket.side {
                    Side::Buy => current_price >= tp_price,  // 롱: 가격 상승 시 TP
                    Side::Sell => current_price <= tp_price, // 숏: 가격 하락 시 TP
                };
                if triggered_tp {
                    triggered.push((key.clone(), "take_profit".to_string()));
                }
            }
        }

        triggered
    }
}

#[async_trait]
impl SignalProcessor for SimulatedExecutor {
    async fn process_signal(
        &mut self,
        signal: &Signal,
        current_price: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<TradeResult>, SignalProcessorError> {
        // min_strength 필터
        if self.config.min_strength > 0.0 && signal.strength < self.config.min_strength {
            return Ok(None);
        }

        match signal.signal_type {
            SignalType::Entry | SignalType::AddToPosition => {
                // 숏 포지션 확인
                if signal.side == Side::Sell && !self.config.allow_short {
                    return Err(SignalProcessorError::ShortNotAllowed);
                }
                self.open_position_internal(signal, current_price, timestamp)
            }
            SignalType::Exit | SignalType::ReducePosition => {
                self.close_position_internal(signal, current_price, timestamp)
            }
            SignalType::Scale => {
                // 스케일 신호는 현재 포지션에 따라 처리
                let key = signal.position_key();
                if self.positions.contains_key(&key) {
                    self.close_position_internal(signal, current_price, timestamp)
                } else {
                    if signal.side == Side::Sell && !self.config.allow_short {
                        return Err(SignalProcessorError::ShortNotAllowed);
                    }
                    self.open_position_internal(signal, current_price, timestamp)
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
        self.bracket_orders.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_signal(ticker: &str, side: Side, signal_type: SignalType) -> Signal {
        Signal::new("test_strategy", ticker.to_string(), side, signal_type)
    }

    #[tokio::test]
    async fn test_open_position() {
        let config = ProcessorConfig::default();
        let mut executor = SimulatedExecutor::new(config, dec!(10_000_000));

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
    async fn test_add_to_position() {
        let config = ProcessorConfig::default();
        let mut executor = SimulatedExecutor::new(config, dec!(10_000_000));

        // 첫 매수
        let signal1 = create_test_signal("005930", Side::Buy, SignalType::Entry)
            .with_strength(0.5);
        executor
            .process_signal(&signal1, dec!(50000), Utc::now())
            .await
            .unwrap();

        // 분할 매수 (AddToPosition 사용)
        let signal2 = create_test_signal("005930", Side::Buy, SignalType::AddToPosition)
            .with_strength(0.5);
        let result = executor
            .process_signal(&signal2, dec!(49000), Utc::now())
            .await;

        assert!(result.is_ok());
        assert_eq!(executor.positions().len(), 1); // 여전히 1개 포지션

        // 평균 단가가 변경되었는지 확인
        let pos = executor.positions().get("005930").unwrap();
        assert!(pos.entry_price < dec!(50000)); // 평균 단가가 낮아졌어야 함
    }

    #[tokio::test]
    async fn test_duplicate_entry_ignored() {
        let config = ProcessorConfig::default();
        let mut executor = SimulatedExecutor::new(config, dec!(10_000_000));

        // 첫 매수
        let signal1 = create_test_signal("005930", Side::Buy, SignalType::Entry)
            .with_strength(0.5);
        executor
            .process_signal(&signal1, dec!(50000), Utc::now())
            .await
            .unwrap();

        // 같은 ticker로 Entry 시도 → 무시됨
        let signal2 = create_test_signal("005930", Side::Buy, SignalType::Entry)
            .with_strength(0.5);
        let result = executor
            .process_signal(&signal2, dec!(49000), Utc::now())
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none()); // 무시됨
        assert_eq!(executor.positions().len(), 1);
    }

    #[tokio::test]
    async fn test_close_position() {
        let config = ProcessorConfig::default();
        let mut executor = SimulatedExecutor::new(config, dec!(10_000_000));

        // 매수
        let buy_signal =
            create_test_signal("005930", Side::Buy, SignalType::Entry).with_strength(0.5);
        executor
            .process_signal(&buy_signal, dec!(50000), Utc::now())
            .await
            .unwrap();

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
    async fn test_short_not_allowed() {
        let config = ProcessorConfig {
            allow_short: false,
            ..Default::default()
        };
        let mut executor = SimulatedExecutor::new(config, dec!(10_000_000));

        let signal = create_test_signal("005930", Side::Sell, SignalType::Entry);
        let result = executor
            .process_signal(&signal, dec!(50000), Utc::now())
            .await;

        assert!(matches!(result, Err(SignalProcessorError::ShortNotAllowed)));
    }

    #[tokio::test]
    async fn test_total_equity() {
        let config = ProcessorConfig::default();
        let mut executor = SimulatedExecutor::new(config, dec!(10_000_000));

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
}
