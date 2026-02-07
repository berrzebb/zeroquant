//! 거래 체결 기록.
//!
//! 이 모듈은 주문 체결 관련 타입을 정의합니다:
//! - `Trade` - 개별 체결 기록
//! - `TradeStats` - 거래 통계

use crate::domain::{OrderStatusType, Side};
use crate::types::{Price, Quantity};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 체결된 주문을 나타내는 거래 기록.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    /// 내부 거래 ID
    pub id: Uuid,
    /// 관련 주문 ID
    pub order_id: Uuid,
    /// 거래소 이름
    pub exchange: String,
    /// 거래소 거래 ID
    pub exchange_trade_id: String,
    /// 거래 심볼 (ticker)
    pub ticker: String,
    /// 거래 방향
    pub side: Side,
    /// 체결 수량
    pub quantity: Quantity,
    /// 체결 가격
    pub price: Price,
    /// 수수료
    pub fee: Decimal,
    /// 수수료 통화
    pub fee_currency: String,
    /// 체결 타임스탬프
    pub executed_at: DateTime<Utc>,
    /// 메이커 여부 (메이커 = true, 테이커 = false)
    pub is_maker: bool,
    /// 추가 메타데이터
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl Trade {
    /// 새 거래 기록을 생성합니다.
    pub fn new(
        order_id: Uuid,
        exchange: impl Into<String>,
        exchange_trade_id: impl Into<String>,
        ticker: String,
        side: Side,
        quantity: Quantity,
        price: Price,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            order_id,
            exchange: exchange.into(),
            exchange_trade_id: exchange_trade_id.into(),
            ticker,
            side,
            quantity,
            price,
            fee: Decimal::ZERO,
            fee_currency: "USDT".to_string(),
            executed_at: Utc::now(),
            is_maker: false,
            metadata: serde_json::Value::Null,
        }
    }

    /// 수수료를 설정합니다.
    pub fn with_fee(mut self, fee: Decimal, currency: impl Into<String>) -> Self {
        self.fee = fee;
        self.fee_currency = currency.into();
        self
    }

    /// 메이커/테이커 플래그를 설정합니다.
    pub fn with_maker(mut self, is_maker: bool) -> Self {
        self.is_maker = is_maker;
        self
    }

    /// 체결 타임스탬프를 설정합니다.
    pub fn with_executed_at(mut self, executed_at: DateTime<Utc>) -> Self {
        self.executed_at = executed_at;
        self
    }

    /// 메타데이터를 설정합니다.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// 거래의 명목 가치를 반환합니다.
    pub fn notional_value(&self) -> Decimal {
        self.price * self.quantity
    }

    /// 수수료 차감 후 순가치를 반환합니다 (매수: 음수, 매도: 양수).
    pub fn net_value(&self) -> Decimal {
        let notional = self.notional_value();
        match self.side {
            Side::Buy => -(notional + self.fee),
            Side::Sell => notional - self.fee,
        }
    }
}

/// 체결 내역 기록 (거래소 중립적).
///
/// 주문의 체결 결과를 나타내는 거래소 중립적 타입입니다.
/// 각 거래소 커넥터는 자체 응답 타입을 이 타입으로 변환합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// 거래소 이름
    pub exchange: String,
    /// 거래소 주문 ID
    pub order_id: String,
    /// 원주문 ID (정정/취소 시)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_order_id: Option<String>,
    /// 거래 심볼
    pub symbol: String,
    /// 종목명/자산명
    pub asset_name: String,
    /// 주문 방향
    pub side: Side,
    /// 주문 유형 (지정가, 시장가 등)
    pub order_type: String,
    /// 주문 수량
    pub order_qty: Quantity,
    /// 주문 가격
    pub order_price: Price,
    /// 체결 수량
    pub filled_qty: Quantity,
    /// 체결 평균가
    pub filled_price: Price,
    /// 체결 금액
    pub filled_amount: Decimal,
    /// 주문 상태 (체결, 미체결, 취소 등)
    pub status: OrderStatusType,
    /// 취소 여부
    pub is_cancelled: bool,
    /// 정정/취소 구분
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modify_type: Option<String>,
    /// 주문 일시
    pub ordered_at: DateTime<Utc>,
    /// 추가 메타데이터
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl ExecutionRecord {
    /// 완전 체결 여부
    pub fn is_fully_filled(&self) -> bool {
        self.filled_qty >= self.order_qty
    }

    /// 부분 체결 여부
    pub fn is_partially_filled(&self) -> bool {
        self.filled_qty > Decimal::ZERO && self.filled_qty < self.order_qty
    }

    /// 체결률 (%)
    pub fn fill_rate(&self) -> Decimal {
        if self.order_qty.is_zero() {
            return Decimal::ZERO;
        }
        (self.filled_qty / self.order_qty) * Decimal::from(100)
    }
}

/// 체결 내역 조회 결과 (거래소 중립적).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionHistory {
    /// 체결 내역 목록
    pub records: Vec<ExecutionRecord>,
    /// 추가 데이터 존재 여부
    pub has_more: bool,
    /// 다음 페이지 조회용 커서 (거래소별 상이)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_trade_creation() {
        let symbol = "BTC/USDT".to_string();
        let trade = Trade::new(
            Uuid::new_v4(),
            "binance".to_string(),
            "12345".to_string(),
            symbol,
            Side::Buy,
            dec!(0.1),
            dec!(50000),
        )
        .with_fee(dec!(5), "USDT");

        assert_eq!(trade.notional_value(), dec!(5000));
        assert_eq!(trade.fee, dec!(5));
    }
}
