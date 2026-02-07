//! 거래소 중립 타입 정의.
//!
//! 다양한 거래소(KIS KR/US, Binance 등)의 데이터를
//! 통일된 형식으로 표현하기 위한 중립 타입입니다.
//!
//! 각 거래소의 serde 타입은 거래소 크레이트 내부에 유지되며,
//! `From<T>` 변환을 통해 이 중립 타입으로 변환됩니다.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// =============================================================================
// OHLCV (캔들 데이터)
// =============================================================================

/// 거래소 중립 OHLCV 캔들 데이터.
///
/// KIS 국내(일봉/분봉), KIS 해외(일봉), Binance Kline 등을
/// 통일된 형식으로 표현합니다.
///
/// # 필드 설명
///
/// - `datetime`: 일봉은 YYYYMMDD, 분봉은 HHMMSS 형식
/// - `trading_value`, `change`, `change_rate`: 일부 거래소/시간대에서는 제공하지 않음
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhlcvBar {
    /// 날짜/시간 (YYYYMMDD 또는 HHMMSS)
    pub datetime: String,
    /// 시가
    pub open: Decimal,
    /// 고가
    pub high: Decimal,
    /// 저가
    pub low: Decimal,
    /// 종가
    pub close: Decimal,
    /// 거래량
    pub volume: Decimal,
    /// 거래대금 (일부 거래소에서 미제공)
    pub trading_value: Option<Decimal>,
    /// 전일 대비 변동 (일부 거래소에서 미제공)
    pub change: Option<Decimal>,
    /// 등락률 (%, 일부 거래소에서 미제공)
    pub change_rate: Option<Decimal>,
}

// =============================================================================
// 호가 (OrderBook)
// =============================================================================

/// 거래소 중립 최우선 호가 데이터.
///
/// REST API에서 조회하는 최우선 호가(1호가) 기준의 간략한 호가 정보입니다.
/// 전체 호가창 데이터는 `market_data::OrderBook`을 사용합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleOrderBook {
    /// 매도 1호가
    pub ask_price: Decimal,
    /// 매도 1호가 잔량
    pub ask_qty: Decimal,
    /// 매수 1호가
    pub bid_price: Decimal,
    /// 매수 1호가 잔량
    pub bid_qty: Decimal,
    /// 총 매도 잔량 (일부 거래소에서 미제공)
    pub total_ask_qty: Option<Decimal>,
    /// 총 매수 잔량 (일부 거래소에서 미제공)
    pub total_bid_qty: Option<Decimal>,
}

// =============================================================================
// 매수 가능 금액
// =============================================================================

/// 거래소 중립 매수 가능 금액 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuyPower {
    /// 최대 매수 가능 수량
    pub max_quantity: Decimal,
    /// 주문 가능 현금
    pub orderable_cash: Decimal,
    /// 주문 가능 금액
    pub orderable_amount: Decimal,
}

// =============================================================================
// 주문 응답
// =============================================================================

/// 거래소 중립 주문 응답.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    /// 주문번호
    pub order_no: String,
    /// 주문시간 (HHMMSS 등)
    pub order_time: String,
}

// =============================================================================
// 체결 내역
// =============================================================================

/// 거래소 중립 체결 내역.
///
/// KIS 국내/해외 주문체결 내역을 통일된 형식으로 표현합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderExecution {
    /// 주문일자 (YYYYMMDD)
    pub order_date: String,
    /// 주문번호
    pub order_no: String,
    /// 원주문번호
    pub original_order_no: String,
    /// 주문시각 (HHMMSS)
    pub order_time: String,
    /// 매수/매도 구분 ("buy" 또는 "sell")
    pub side: String,
    /// 종목코드
    pub symbol: String,
    /// 종목명
    pub name: String,
    /// 주문수량
    pub order_qty: Decimal,
    /// 주문단가
    pub order_price: Decimal,
    /// 체결수량
    pub filled_qty: Decimal,
    /// 체결평균가
    pub avg_price: Decimal,
    /// 체결금액
    pub filled_amount: Decimal,
    /// 주문유형명 (지정가, 시장가 등)
    pub order_type: String,
    /// 취소 여부
    pub is_cancelled: bool,
}

// =============================================================================
// 보유 종목
// =============================================================================

/// 거래소 중립 보유 종목 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountHolding {
    /// 종목코드
    pub symbol: String,
    /// 종목명
    pub name: String,
    /// 보유수량
    pub quantity: Decimal,
    /// 매입평균가
    pub avg_price: Decimal,
    /// 현재가
    pub current_price: Decimal,
}

// =============================================================================
// 계좌 요약
// =============================================================================

/// 거래소 중립 계좌 요약 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSummary {
    /// 예수금 (현금 잔고, 일부 거래소에서 미제공)
    pub cash_balance: Option<Decimal>,
    /// 총 평가금액
    pub total_eval_amount: Decimal,
    /// 총 평가손익
    pub total_profit_loss: Decimal,
}

// =============================================================================
// 테스트
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_ohlcv_bar_creation() {
        let bar = OhlcvBar {
            datetime: "20260101".to_string(),
            open: dec!(100),
            high: dec!(110),
            low: dec!(95),
            close: dec!(105),
            volume: dec!(1000000),
            trading_value: Some(dec!(105000000)),
            change: Some(dec!(5)),
            change_rate: Some(dec!(5.0)),
        };
        assert_eq!(bar.close, dec!(105));
        assert!(bar.trading_value.is_some());
    }

    #[test]
    fn test_ohlcv_bar_without_optional_fields() {
        let bar = OhlcvBar {
            datetime: "20260101".to_string(),
            open: dec!(100),
            high: dec!(110),
            low: dec!(95),
            close: dec!(105),
            volume: dec!(1000000),
            trading_value: None,
            change: None,
            change_rate: None,
        };
        assert!(bar.trading_value.is_none());
    }

    #[test]
    fn test_simple_order_book_creation() {
        let book = SimpleOrderBook {
            ask_price: dec!(100),
            ask_qty: dec!(500),
            bid_price: dec!(99),
            bid_qty: dec!(300),
            total_ask_qty: Some(dec!(5000)),
            total_bid_qty: Some(dec!(3000)),
        };
        assert_eq!(book.ask_price, dec!(100));
    }

    #[test]
    fn test_buy_power_creation() {
        let power = BuyPower {
            max_quantity: dec!(100),
            orderable_cash: dec!(10000000),
            orderable_amount: dec!(10000000),
        };
        assert_eq!(power.max_quantity, dec!(100));
    }

    #[test]
    fn test_order_response_creation() {
        let resp = OrderResponse {
            order_no: "0000123456".to_string(),
            order_time: "093015".to_string(),
        };
        assert_eq!(resp.order_no, "0000123456");
    }

    #[test]
    fn test_order_execution_creation() {
        let exec = OrderExecution {
            order_date: "20260101".to_string(),
            order_no: "0000123456".to_string(),
            original_order_no: "0000123456".to_string(),
            order_time: "093015".to_string(),
            side: "buy".to_string(),
            symbol: "005930".to_string(),
            name: "삼성전자".to_string(),
            order_qty: dec!(10),
            order_price: dec!(70000),
            filled_qty: dec!(10),
            avg_price: dec!(70000),
            filled_amount: dec!(700000),
            order_type: "지정가".to_string(),
            is_cancelled: false,
        };
        assert_eq!(exec.side, "buy");
        assert!(!exec.is_cancelled);
    }

    #[test]
    fn test_account_holding_creation() {
        let holding = AccountHolding {
            symbol: "005930".to_string(),
            name: "삼성전자".to_string(),
            quantity: dec!(100),
            avg_price: dec!(70000),
            current_price: dec!(72000),
        };
        assert_eq!(holding.quantity, dec!(100));
    }

    #[test]
    fn test_account_summary_creation() {
        let summary = AccountSummary {
            cash_balance: Some(dec!(5000000)),
            total_eval_amount: dec!(15000000),
            total_profit_loss: dec!(500000),
        };
        assert!(summary.cash_balance.is_some());
    }

    #[test]
    fn test_account_summary_without_cash() {
        let summary = AccountSummary {
            cash_balance: None,
            total_eval_amount: dec!(15000000),
            total_profit_loss: dec!(500000),
        };
        assert!(summary.cash_balance.is_none());
    }
}
