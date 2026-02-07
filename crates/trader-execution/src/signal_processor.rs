//! Signal 처리 추상화.
//!
//! 전략이 발행하는 Signal을 처리하는 공통 인터페이스를 제공합니다.
//! 실거래와 시뮬레이션 모두 동일한 인터페이스를 사용합니다.
//!
//! # 아키텍처
//!
//! ```text
//! ┌─────────────┐
//! │   전략      │ → Signal 발행
//! └─────────────┘
//!        │
//!        ▼
//! ╔═══════════════════════════════════════╗
//! ║  SignalProcessor (trait)              ║
//! ╠═══════════════════════════════════════╣
//! ║  SimulatedExecutor  │  LiveExecutor   ║
//! ║  (가상 체결)        │  (실제 주문)    ║
//! ╚═══════════════════════════════════════╝
//! ```
//!
//! # 사용 예시
//!
//! ```ignore
//! use trader_execution::{SignalProcessor, SimulatedExecutor};
//!
//! // 시뮬레이션 모드
//! let mut executor = SimulatedExecutor::new(config);
//! executor.process_signal(&signal, current_price, timestamp)?;
//!
//! // 실거래 모드
//! let mut executor = OrderExecutor::new(config, exchange);
//! executor.process_signal(&signal, current_price, timestamp)?;
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use trader_core::{Side, Signal, SignalType};

/// Signal 처리 에러
#[derive(Debug, Clone, Error)]
pub enum SignalProcessorError {
    #[error("자금 부족: 필요 {required}, 보유 {available}")]
    InsufficientFunds {
        required: Decimal,
        available: Decimal,
    },
    #[error("포지션을 찾을 수 없음: {symbol}")]
    PositionNotFound { symbol: String },
    #[error("최대 포지션 수 초과: {max}")]
    MaxPositionsExceeded { max: usize },
    #[error("유효하지 않은 가격: {price}")]
    InvalidPrice { price: Decimal },
    #[error("숏 포지션 비허용")]
    ShortNotAllowed,
    #[error("거래소 에러: {0}")]
    ExchangeError(String),
    #[error("주문 실패: {0}")]
    OrderFailed(String),
}

/// 거래 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeResult {
    /// 심볼
    pub symbol: String,
    /// 방향
    pub side: Side,
    /// 신호 유형 (Entry, Exit, AddToPosition 등)
    pub signal_type: SignalType,
    /// 체결 수량
    pub quantity: Decimal,
    /// 체결 가격
    pub price: Decimal,
    /// 수수료
    pub commission: Decimal,
    /// 슬리피지 금액
    pub slippage: Decimal,
    /// 체결 시간
    pub timestamp: DateTime<Utc>,
    /// 실현 손익 (청산 거래인 경우)
    pub realized_pnl: Option<Decimal>,
    /// 분할 매수/매도 여부
    pub is_partial: bool,
    /// 메타데이터
    pub metadata: HashMap<String, String>,
}

/// 포지션 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorPosition {
    /// 심볼
    pub symbol: String,
    /// 방향 (Long/Short)
    pub side: Side,
    /// 수량
    pub quantity: Decimal,
    /// 평균 진입가
    pub entry_price: Decimal,
    /// 진입 시간
    pub entry_time: DateTime<Utc>,
    /// 누적 수수료
    pub fees: Decimal,
    /// 포지션 ID (스프레드/그리드 전략용)
    /// None이면 symbol이 키, Some이면 position_id가 키
    pub position_id: Option<String>,
    /// 그룹 ID (관련 포지션 묶음)
    /// 그룹 단위 청산, 손익 추적용
    pub group_id: Option<String>,
}

/// Signal 처리 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorConfig {
    /// 수수료율 (예: 0.001 = 0.1%)
    pub commission_rate: Decimal,
    /// 슬리피지율 (예: 0.0005 = 0.05%)
    pub slippage_rate: Decimal,
    /// 최대 포지션 크기 비율 (예: 0.2 = 20%)
    pub max_position_size_pct: Decimal,
    /// 최대 포지션 수
    pub max_positions: usize,
    /// 숏 허용 여부
    pub allow_short: bool,
    /// Signal 최소 강도 (0.0 ~ 1.0, 기본 0.0)
    /// 이 값보다 낮은 강도의 Signal은 무시됩니다.
    #[serde(default)]
    pub min_strength: f64,
    /// 자동 손절 생성 여부 (기본 false)
    #[serde(default)]
    pub auto_stop_loss: bool,
    /// 자동 익절 생성 여부 (기본 false)
    #[serde(default)]
    pub auto_take_profit: bool,
    /// 손절 비율 (기본 0.05 = 5%)
    #[serde(default = "default_stop_loss_pct")]
    pub stop_loss_pct: Decimal,
    /// 익절 비율 (기본 0.10 = 10%)
    #[serde(default = "default_take_profit_pct")]
    pub take_profit_pct: Decimal,
}


fn default_stop_loss_pct() -> Decimal {
    Decimal::new(5, 2) // 5%
}

fn default_take_profit_pct() -> Decimal {
    Decimal::new(10, 2) // 10%
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            commission_rate: Decimal::new(1, 3),     // 0.1%
            slippage_rate: Decimal::new(5, 4),       // 0.05%
            max_position_size_pct: Decimal::new(2, 1), // 20%
            max_positions: 10,
            allow_short: false,
            min_strength: 0.0,
            auto_stop_loss: false,
            auto_take_profit: false,
            stop_loss_pct: Decimal::new(5, 2),  // 5%
            take_profit_pct: Decimal::new(10, 2), // 10%
        }
    }
}

/// Signal 처리 trait
///
/// 전략이 발행하는 Signal을 처리하는 공통 인터페이스입니다.
/// 실거래(LiveExecutor)와 시뮬레이션(SimulatedExecutor) 모두 이 trait을 구현합니다.
#[async_trait]
pub trait SignalProcessor: Send + Sync {
    /// Signal 처리
    ///
    /// Entry/Exit/AddToPosition/ReducePosition/Scale 신호를 처리합니다.
    /// 거래가 발생하면 TradeResult를 반환합니다.
    async fn process_signal(
        &mut self,
        signal: &Signal,
        current_price: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<TradeResult>, SignalProcessorError>;

    /// 현재 잔고 조회
    fn balance(&self) -> Decimal;

    /// 포지션 목록 조회
    fn positions(&self) -> &HashMap<String, ProcessorPosition>;

    /// 특정 포지션 조회
    fn get_position(&self, symbol: &str) -> Option<&ProcessorPosition> {
        self.positions().get(symbol)
    }

    /// 포지션 보유 여부
    fn has_position(&self, symbol: &str) -> bool {
        self.positions().contains_key(symbol)
    }

    /// 거래 기록 조회
    fn trades(&self) -> &[TradeResult];

    /// 총 수수료
    fn total_commission(&self) -> Decimal;

    /// 미실현 손익 계산
    fn unrealized_pnl(&self, current_prices: &HashMap<String, Decimal>) -> Decimal {
        self.positions()
            .values()
            .map(|p| {
                let price = current_prices.get(&p.symbol).copied().unwrap_or(p.entry_price);
                if p.side == Side::Buy {
                    (price - p.entry_price) * p.quantity
                } else {
                    (p.entry_price - price) * p.quantity
                }
            })
            .sum()
    }

    /// 실현 손익 합계
    fn realized_pnl(&self) -> Decimal {
        self.trades()
            .iter()
            .filter_map(|t| t.realized_pnl)
            .sum()
    }

    /// 총 자산 (잔고 + 포지션 평가액)
    fn total_equity(&self, current_prices: &HashMap<String, Decimal>) -> Decimal {
        let position_value: Decimal = self
            .positions()
            .values()
            .map(|p| {
                let price = current_prices.get(&p.symbol).copied().unwrap_or(p.entry_price);
                price * p.quantity
            })
            .sum();

        self.balance() + position_value
    }

    /// 상태 초기화
    fn reset(&mut self, initial_balance: Decimal);

    /// 그룹별 포지션 조회
    ///
    /// 동일한 group_id를 가진 모든 포지션을 반환합니다.
    fn positions_by_group(&self, group_id: &str) -> Vec<&ProcessorPosition> {
        self.positions()
            .values()
            .filter(|p| p.group_id.as_deref() == Some(group_id))
            .collect()
    }

    /// 그룹별 미실현 손익 계산
    fn group_unrealized_pnl(
        &self,
        group_id: &str,
        current_prices: &HashMap<String, Decimal>,
    ) -> Decimal {
        self.positions_by_group(group_id)
            .iter()
            .map(|p| {
                let price = current_prices.get(&p.symbol).copied().unwrap_or(p.entry_price);
                if p.side == Side::Buy {
                    (price - p.entry_price) * p.quantity
                } else {
                    (p.entry_price - price) * p.quantity
                }
            })
            .sum()
    }

    /// 그룹에 속한 포지션 ID 목록 반환 (그룹 청산용)
    fn position_keys_by_group(&self, group_id: &str) -> Vec<String> {
        self.positions()
            .iter()
            .filter(|(_, p)| p.group_id.as_deref() == Some(group_id))
            .map(|(key, _)| key.clone())
            .collect()
    }
}

/// 슬리피지 적용 가격 계산
pub fn apply_slippage(price: Decimal, slippage_rate: Decimal, side: Side) -> Decimal {
    let slippage = price * slippage_rate;
    match side {
        Side::Buy => price + slippage,  // 매수는 높은 가격
        Side::Sell => price - slippage, // 매도는 낮은 가격
    }
}

/// 포지션 크기 계산.
///
/// 잔고, 최대 비율, Signal 강도를 기반으로 주문 수량을 계산합니다.
/// SimulatedExecutor와 LiveExecutor에서 공통으로 사용합니다.
///
/// # Returns
/// `(position_amount, quantity)` - 포지션 금액과 주문 수량
pub fn calculate_position_size(
    balance: Decimal,
    max_position_size_pct: Decimal,
    strength: f64,
    price: Decimal,
) -> (Decimal, Decimal) {
    let max_amount = balance * max_position_size_pct;
    let strength_dec = rust_decimal::prelude::FromPrimitive::from_f64(strength)
        .unwrap_or(Decimal::ONE);
    let position_amount = max_amount * strength_dec;
    let quantity = position_amount / price;
    (position_amount, quantity)
}

/// 자금 검증.
///
/// 주문에 필요한 금액(포지션 금액 + 수수료)이 잔고를 초과하는지 확인합니다.
pub fn validate_funds(
    position_amount: Decimal,
    commission_rate: Decimal,
    balance: Decimal,
) -> Result<Decimal, SignalProcessorError> {
    let commission = position_amount * commission_rate;
    let required = position_amount + commission;
    if required > balance {
        return Err(SignalProcessorError::InsufficientFunds {
            required,
            available: balance,
        });
    }
    Ok(commission)
}

/// 실현 손익 계산.
///
/// 진입가, 청산가, 수량, 수수료, 포지션 방향을 기반으로 PnL을 계산합니다.
pub fn calculate_realized_pnl(
    entry_price: Decimal,
    exit_price: Decimal,
    quantity: Decimal,
    commission: Decimal,
    side: Side,
) -> Decimal {
    if side == Side::Buy {
        (exit_price - entry_price) * quantity - commission
    } else {
        (entry_price - exit_price) * quantity - commission
    }
}

/// 청산 수량 결정.
///
/// position_id 기반 전략은 전량 청산, 레거시 ReducePosition은 분할 청산합니다.
pub fn determine_close_quantity(
    signal: &Signal,
    position_quantity: Decimal,
) -> Decimal {
    if signal.position_id.is_some() {
        // 개별 position_id 사용 시: 해당 포지션 전량 청산
        position_quantity
    } else if signal.signal_type == SignalType::ReducePosition {
        // ReducePosition: 분할 청산 (레거시 호환)
        let levels = signal
            .metadata
            .get("grid_levels")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;
        position_quantity / Decimal::from(levels)
    } else {
        // 일반 Exit: 전량 청산
        position_quantity
    }
}

/// 포지션 추가 시 평균 단가 재계산.
///
/// 기존 포지션에 추가 매수 시 가중 평균 단가를 계산합니다.
pub fn update_position_average(
    existing: &mut ProcessorPosition,
    add_quantity: Decimal,
    add_price: Decimal,
    commission: Decimal,
) {
    let existing_value = existing.quantity * existing.entry_price;
    let new_value = add_quantity * add_price;
    let new_quantity = existing.quantity + add_quantity;
    existing.entry_price = (existing_value + new_value) / new_quantity;
    existing.quantity = new_quantity;
    existing.fees += commission;
}

/// Signal metadata를 TradeResult metadata로 변환.
///
/// JSON Value의 문자열 값만 추출하여 HashMap<String, String>으로 변환합니다.
pub fn convert_signal_metadata(signal: &Signal) -> HashMap<String, String> {
    signal
        .metadata
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect()
}

/// 진입 거래 기록 생성.
pub fn build_entry_trade(
    signal: &Signal,
    quantity: Decimal,
    execution_price: Decimal,
    commission: Decimal,
    slippage: Decimal,
    timestamp: DateTime<Utc>,
) -> TradeResult {
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
        metadata: convert_signal_metadata(signal),
    }
}

/// 청산 거래 기록 생성.
pub fn build_exit_trade(
    symbol: &str,
    signal: &Signal,
    close_quantity: Decimal,
    execution_price: Decimal,
    commission: Decimal,
    realized_pnl: Decimal,
    position_quantity: Decimal,
    timestamp: DateTime<Utc>,
) -> TradeResult {
    TradeResult {
        symbol: symbol.to_string(),
        side: signal.side,
        signal_type: signal.signal_type,
        quantity: close_quantity,
        price: execution_price,
        commission,
        slippage: Decimal::ZERO,
        timestamp,
        realized_pnl: Some(realized_pnl),
        is_partial: close_quantity < position_quantity,
        metadata: convert_signal_metadata(signal),
    }
}

/// 추가 매수 거래 기록 생성.
pub fn build_add_trade(
    signal: &Signal,
    add_quantity: Decimal,
    execution_price: Decimal,
    commission: Decimal,
    timestamp: DateTime<Utc>,
) -> TradeResult {
    TradeResult {
        symbol: signal.ticker.clone(),
        side: signal.side,
        signal_type: signal.signal_type,
        quantity: add_quantity,
        price: execution_price,
        commission,
        slippage: Decimal::ZERO,
        timestamp,
        realized_pnl: None,
        is_partial: true,
        metadata: convert_signal_metadata(signal),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_processor_config_default() {
        let config = ProcessorConfig::default();
        assert_eq!(config.commission_rate, dec!(0.001));
        assert_eq!(config.slippage_rate, dec!(0.0005));
        assert_eq!(config.max_position_size_pct, dec!(0.2));
        assert_eq!(config.max_positions, 10);
        assert!(!config.allow_short);
    }

    #[test]
    fn test_apply_slippage() {
        let price = dec!(10000);
        let slippage_rate = dec!(0.001); // 0.1%

        let buy_price = apply_slippage(price, slippage_rate, Side::Buy);
        let sell_price = apply_slippage(price, slippage_rate, Side::Sell);

        assert_eq!(buy_price, dec!(10010));  // 10000 + 10
        assert_eq!(sell_price, dec!(9990)); // 10000 - 10
    }
}
