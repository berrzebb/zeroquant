//! 시장 데이터 타입 및 구조체.
//!
//! 이 모듈은 시장 데이터 관련 타입을 정의합니다:
//! - `Kline` - OHLCV 캔들스틱 데이터
//! - `Ticker` - 실시간 시세 데이터
//! - `OrderBook` - 호가창 데이터
//! - `TradeTick` - 체결 틱 데이터
//! - `MarketData` - 통합 시장 데이터

use crate::domain::order::Side;
use crate::types::{Price, Quantity, Timeframe};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// OHLCV 캔들스틱 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kline {
    /// 거래 심볼
    pub ticker: String,
    /// 타임프레임
    pub timeframe: Timeframe,
    /// 캔들 시작 시간
    pub open_time: DateTime<Utc>,
    /// 시가
    pub open: Price,
    /// 고가
    pub high: Price,
    /// 저가
    pub low: Price,
    /// 종가
    pub close: Price,
    /// 거래량 (기준 자산 단위)
    pub volume: Quantity,
    /// 캔들 종료 시간
    pub close_time: DateTime<Utc>,
    /// 거래대금 (호가 자산 단위)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_volume: Option<Decimal>,
    /// 체결 건수
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_trades: Option<u32>,
}

impl Kline {
    /// 새 캔들을 생성합니다.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ticker: String,
        timeframe: Timeframe,
        open_time: DateTime<Utc>,
        open: Price,
        high: Price,
        low: Price,
        close: Price,
        volume: Quantity,
        close_time: DateTime<Utc>,
    ) -> Self {
        Self {
            ticker,
            timeframe,
            open_time,
            open,
            high,
            low,
            close,
            volume,
            close_time,
            quote_volume: None,
            num_trades: None,
        }
    }

    /// 캔들 몸통 크기(절대값)를 반환합니다.
    pub fn body_size(&self) -> Decimal {
        (self.close - self.open).abs()
    }

    /// 캔들 범위(고가 - 저가)를 반환합니다.
    pub fn range(&self) -> Decimal {
        self.high - self.low
    }

    /// 양봉(종가 > 시가)인지 확인합니다.
    pub fn is_bullish(&self) -> bool {
        self.close > self.open
    }

    /// 음봉(종가 < 시가)인지 확인합니다.
    pub fn is_bearish(&self) -> bool {
        self.close < self.open
    }

    /// 대표가(고가+저가+종가 평균)를 반환합니다.
    pub fn typical_price(&self) -> Decimal {
        (self.high + self.low + self.close) / Decimal::from(3)
    }

    /// 이 캔들의 VWAP를 반환합니다.
    pub fn vwap(&self) -> Option<Decimal> {
        if self.volume.is_zero() {
            return None;
        }
        self.quote_volume.map(|qv| qv / self.volume)
    }
}

/// 실시간 시세 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticker {
    /// 거래 심볼 (ticker)
    pub ticker: String,
    /// 최우선 매수 호가
    pub bid: Price,
    /// 최우선 매도 호가
    pub ask: Price,
    /// 최근 체결가
    pub last: Price,
    /// 24시간 거래량
    pub volume_24h: Quantity,
    /// 24시간 최고가
    pub high_24h: Price,
    /// 24시간 최저가
    pub low_24h: Price,
    /// 24시간 가격 변동
    pub change_24h: Decimal,
    /// 24시간 변동률(%)
    pub change_24h_percent: Decimal,
    /// 타임스탬프
    pub timestamp: DateTime<Utc>,
}

impl Ticker {
    /// 매수/매도 스프레드를 반환합니다.
    pub fn spread(&self) -> Decimal {
        self.ask - self.bid
    }

    /// 스프레드를 백분율로 반환합니다.
    pub fn spread_pct(&self) -> Decimal {
        if self.bid.is_zero() {
            return Decimal::ZERO;
        }
        (self.spread() / self.bid) * Decimal::from(100)
    }

    /// 중간 가격을 반환합니다.
    pub fn mid_price(&self) -> Decimal {
        (self.bid + self.ask) / Decimal::from(2)
    }
}

/// 호가창 가격 레벨.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookLevel {
    /// 가격
    pub price: Price,
    /// 수량
    pub quantity: Quantity,
}

/// 호가창 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    /// 거래 심볼 (ticker)
    pub ticker: String,
    /// 매수 호가 - 가격 내림차순 정렬
    pub bids: Vec<OrderBookLevel>,
    /// 매도 호가 - 가격 오름차순 정렬
    pub asks: Vec<OrderBookLevel>,
    /// 마지막 업데이트 타임스탬프
    pub timestamp: DateTime<Utc>,
}

impl OrderBook {
    /// 최우선 매수 호가를 반환합니다.
    pub fn best_bid(&self) -> Option<Price> {
        self.bids.first().map(|l| l.price)
    }

    /// 최우선 매도 호가를 반환합니다.
    pub fn best_ask(&self) -> Option<Price> {
        self.asks.first().map(|l| l.price)
    }

    /// 최우선 매수 호가 레벨을 반환합니다.
    pub fn best_bid_level(&self) -> Option<&OrderBookLevel> {
        self.bids.first()
    }

    /// 최우선 매도 호가 레벨을 반환합니다.
    pub fn best_ask_level(&self) -> Option<&OrderBookLevel> {
        self.asks.first()
    }

    /// 스프레드를 반환합니다.
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }

    /// 중간 가격을 반환합니다.
    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some((bid + ask) / Decimal::from(2)),
            _ => None,
        }
    }

    /// 지정 가격까지의 총 매수 물량을 반환합니다.
    pub fn bid_volume_to_price(&self, price: Price) -> Quantity {
        self.bids
            .iter()
            .filter(|l| l.price >= price)
            .map(|l| l.quantity)
            .sum()
    }

    /// 지정 가격까지의 총 매도 물량을 반환합니다.
    pub fn ask_volume_to_price(&self, price: Price) -> Quantity {
        self.asks
            .iter()
            .filter(|l| l.price <= price)
            .map(|l| l.quantity)
            .sum()
    }
}

/// 체결 틱 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeTick {
    /// 거래 심볼 (ticker)
    pub ticker: String,
    /// 체결 ID
    pub id: String,
    /// 가격
    pub price: Price,
    /// 수량
    pub quantity: Quantity,
    /// 체결 방향 (매수 또는 매도)
    pub side: Side,
    /// 타임스탬프
    pub timestamp: DateTime<Utc>,
}

/// 다양한 데이터 유형을 위한 시장 데이터 래퍼.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MarketDataType {
    /// 캔들스틱 데이터
    Kline(Kline),
    /// 시세 데이터
    Ticker(Ticker),
    /// 호가창 스냅샷
    OrderBook(OrderBook),
    /// 체결 틱
    Trade(TradeTick),
}

/// 통합 시장 데이터 구조체.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    /// 거래소 이름
    pub exchange: String,
    /// 거래 ticker
    pub ticker: String,
    /// 데이터 타임스탬프
    pub timestamp: DateTime<Utc>,
    /// 데이터 내용
    pub data: MarketDataType,
}

impl MarketData {
    /// 캔들로부터 시장 데이터를 생성합니다.
    pub fn from_kline(exchange: impl Into<String>, kline: Kline) -> Self {
        Self {
            exchange: exchange.into(),
            ticker: kline.ticker.clone(),
            timestamp: kline.open_time,
            data: MarketDataType::Kline(kline),
        }
    }

    /// 시세로부터 시장 데이터를 생성합니다.
    pub fn from_ticker(exchange: impl Into<String>, ticker: Ticker) -> Self {
        Self {
            exchange: exchange.into(),
            ticker: ticker.ticker.clone(),
            timestamp: ticker.timestamp,
            data: MarketDataType::Ticker(ticker),
        }
    }

    /// 호가창으로부터 시장 데이터를 생성합니다.
    pub fn from_order_book(exchange: impl Into<String>, order_book: OrderBook) -> Self {
        Self {
            exchange: exchange.into(),
            ticker: order_book.ticker.clone(),
            timestamp: order_book.timestamp,
            data: MarketDataType::OrderBook(order_book),
        }
    }

    /// 이 시장 데이터에서 현재 가격을 추출합니다.
    pub fn get_price(&self) -> Option<Price> {
        match &self.data {
            MarketDataType::Kline(k) => Some(k.close),
            MarketDataType::Ticker(t) => Some(t.last),
            MarketDataType::OrderBook(ob) => ob.mid_price(),
            MarketDataType::Trade(t) => Some(t.price),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_kline() {
        let symbol = "BTC/USDT".to_string();
        let now = Utc::now();
        let kline = Kline::new(
            symbol,
            Timeframe::H1,
            now,
            dec!(50000),
            dec!(51000),
            dec!(49500),
            dec!(50500),
            dec!(100),
            now,
        );

        assert!(kline.is_bullish());
        assert_eq!(kline.body_size(), dec!(500));
        assert_eq!(kline.range(), dec!(1500));
    }

    #[test]
    fn test_order_book() {
        let ticker = "ETH/USDT".to_string();
        let ob = OrderBook {
            ticker,
            bids: vec![
                OrderBookLevel {
                    price: dec!(2000),
                    quantity: dec!(10),
                },
                OrderBookLevel {
                    price: dec!(1999),
                    quantity: dec!(20),
                },
            ],
            asks: vec![
                OrderBookLevel {
                    price: dec!(2001),
                    quantity: dec!(15),
                },
                OrderBookLevel {
                    price: dec!(2002),
                    quantity: dec!(25),
                },
            ],
            timestamp: Utc::now(),
        };

        assert_eq!(ob.best_bid(), Some(dec!(2000)));
        assert_eq!(ob.best_ask(), Some(dec!(2001)));
        assert_eq!(ob.spread(), Some(dec!(1)));
        assert_eq!(ob.mid_price(), Some(dec!(2000.5)));
    }

    #[test]
    fn test_ticker_spread() {
        let ticker_str = "BTC/USDT".to_string();
        let ticker = Ticker {
            ticker: ticker_str,
            bid: dec!(50000),
            ask: dec!(50010),
            last: dec!(50005),
            volume_24h: dec!(1000),
            high_24h: dec!(51000),
            low_24h: dec!(49000),
            change_24h: dec!(500),
            change_24h_percent: dec!(1.0),
            timestamp: Utc::now(),
        };

        assert_eq!(ticker.spread(), dec!(10));
        assert_eq!(ticker.mid_price(), dec!(50005));
    }
}
