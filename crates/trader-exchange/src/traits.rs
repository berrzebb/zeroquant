//! 거래소 trait 정의.

use async_trait::async_trait;
use trader_core::{Kline, OrderBook, OrderStatus, Position, Ticker, Timeframe, TradeTick};

use crate::ExchangeError;

/// 거래소 작업을 위한 Result 타입.
pub type ExchangeResult<T> = Result<T, ExchangeError>;

/// 자산의 잔고 정보.
#[derive(Debug, Clone)]
pub struct Balance {
    /// 자산 이름 (예: "BTC", "USDT")
    pub asset: String,
    /// 사용 가능한 잔고
    pub free: rust_decimal::Decimal,
    /// 주문에 묶인 잔고
    pub locked: rust_decimal::Decimal,
}

impl Balance {
    /// 총 잔고 반환 (사용 가능 + 묶인 잔고).
    pub fn total(&self) -> rust_decimal::Decimal {
        self.free + self.locked
    }
}

/// 거래소의 계좌 정보.
#[derive(Debug, Clone)]
pub struct AccountInfo {
    /// 계좌 잔고
    pub balances: Vec<Balance>,
    /// 거래 가능 여부
    pub can_trade: bool,
    /// 출금 가능 여부
    pub can_withdraw: bool,
    /// 입금 가능 여부
    pub can_deposit: bool,
}

// Exchange trait은 제거됨 (v0.7.5).
// ExchangeProvider trait (trader_core::domain::exchange_provider)이 대체합니다.
// BinanceClient, SimulatedExchange에서는 동일 메서드가 inherent impl로 전환되었습니다.

/// 시장 데이터 스트림 이벤트.
#[derive(Debug, Clone)]
pub enum MarketEvent {
    /// 시세 업데이트
    Ticker(Ticker),
    /// 캔들스틱 업데이트
    Kline(Kline),
    /// 호가창 업데이트
    OrderBook(OrderBook),
    /// 체결 틱
    Trade(TradeTick),
    /// 연결 상태 변경
    Connected,
    /// 연결 해제
    Disconnected,
    /// 에러 발생
    Error(String),
}

/// 사용자 데이터 스트림 이벤트.
#[derive(Debug, Clone)]
pub enum UserEvent {
    /// 주문 업데이트
    OrderUpdate(OrderStatus),
    /// 잔고 업데이트
    BalanceUpdate(Balance),
    /// 포지션 업데이트 (선물)
    PositionUpdate(Position),
}

/// WebSocket 스트림 구독.
#[async_trait]
pub trait MarketStream: Send + Sync {
    /// WebSocket 연결 시작.
    ///
    /// 연결이 필요 없는 스트림(예: SimulatedMarketStream)은
    /// 기본 구현(no-op)을 사용합니다.
    async fn start(&mut self) -> ExchangeResult<()> {
        Ok(())
    }

    /// 연결 시작 여부 확인.
    fn is_started(&self) -> bool {
        true
    }

    /// 시세 업데이트 구독.
    async fn subscribe_ticker(&mut self, symbol: &str) -> ExchangeResult<()>;

    /// 캔들스틱 업데이트 구독.
    async fn subscribe_kline(&mut self, symbol: &str, timeframe: Timeframe) -> ExchangeResult<()>;

    /// 호가창 업데이트 구독.
    async fn subscribe_order_book(&mut self, symbol: &str) -> ExchangeResult<()>;

    /// 체결 업데이트 구독.
    async fn subscribe_trades(&mut self, symbol: &str) -> ExchangeResult<()>;

    /// 심볼 구독 해제.
    async fn unsubscribe(&mut self, symbol: &str) -> ExchangeResult<()>;

    /// 다음 시장 이벤트 반환.
    async fn next_event(&mut self) -> Option<MarketEvent>;
}

/// 사용자 데이터 스트림 구독.
#[async_trait]
pub trait UserStream: Send + Sync {
    /// 사용자 데이터 스트림 시작.
    async fn start(&mut self) -> ExchangeResult<()>;

    /// 사용자 데이터 스트림 중지.
    async fn stop(&mut self) -> ExchangeResult<()>;

    /// 다음 사용자 이벤트 반환.
    async fn next_event(&mut self) -> Option<UserEvent>;
}
