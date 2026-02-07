//! 시뮬레이션된 시장 및 사용자 데이터 스트림.

#![allow(dead_code)] // 스트림 생명주기 관리 메서드

use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use trader_core::Timeframe;

use crate::traits::{ExchangeResult, MarketEvent, MarketStream, UserEvent, UserStream};

/// 시뮬레이션된 시장 데이터 스트림.
///
/// SimulatedExchange로부터 이벤트를 수신하여 구독자에게 전달합니다.
pub struct SimulatedMarketStream {
    /// 이벤트 수신기
    event_rx: mpsc::Receiver<MarketEvent>,
    /// 티커 구독 심볼
    ticker_subscriptions: HashSet<String>,
    /// Kline 구독 심볼 (타임프레임 포함)
    kline_subscriptions: HashSet<(String, Timeframe)>,
    /// 호가창 구독 심볼
    order_book_subscriptions: HashSet<String>,
    /// 거래 구독 심볼
    trade_subscriptions: HashSet<String>,
}

impl SimulatedMarketStream {
    /// 새로운 시뮬레이션 시장 스트림을 생성합니다.
    pub fn new(event_rx: mpsc::Receiver<MarketEvent>) -> Self {
        Self {
            event_rx,
            ticker_subscriptions: HashSet::new(),
            kline_subscriptions: HashSet::new(),
            order_book_subscriptions: HashSet::new(),
            trade_subscriptions: HashSet::new(),
        }
    }

    /// 이벤트가 구독과 일치하는지 확인합니다.
    fn is_subscribed(&self, event: &MarketEvent) -> bool {
        match event {
            MarketEvent::Ticker(ticker) => self.ticker_subscriptions.contains(&ticker.ticker),
            MarketEvent::Kline(kline) => {
                // 간단하게 심볼만 확인 (타임프레임 매칭은 추가 가능)
                self.kline_subscriptions
                    .iter()
                    .any(|(symbol, _)| symbol == &kline.ticker)
            }
            MarketEvent::OrderBook(ob) => self.order_book_subscriptions.contains(&ob.ticker),
            MarketEvent::Trade(trade) => self.trade_subscriptions.contains(&trade.ticker),
            MarketEvent::Connected | MarketEvent::Disconnected | MarketEvent::Error(_) => true,
        }
    }
}

#[async_trait]
impl MarketStream for SimulatedMarketStream {
    async fn subscribe_ticker(&mut self, symbol: &str) -> ExchangeResult<()> {
        self.ticker_subscriptions.insert(symbol.to_string());
        Ok(())
    }

    async fn subscribe_kline(&mut self, symbol: &str, timeframe: Timeframe) -> ExchangeResult<()> {
        self.kline_subscriptions
            .insert((symbol.to_string(), timeframe));
        Ok(())
    }

    async fn subscribe_order_book(&mut self, symbol: &str) -> ExchangeResult<()> {
        self.order_book_subscriptions.insert(symbol.to_string());
        Ok(())
    }

    async fn subscribe_trades(&mut self, symbol: &str) -> ExchangeResult<()> {
        self.trade_subscriptions.insert(symbol.to_string());
        Ok(())
    }

    async fn unsubscribe(&mut self, symbol: &str) -> ExchangeResult<()> {
        self.ticker_subscriptions.remove(symbol);
        self.kline_subscriptions.retain(|(s, _)| s != symbol);
        self.order_book_subscriptions.remove(symbol);
        self.trade_subscriptions.remove(symbol);
        Ok(())
    }

    async fn next_event(&mut self) -> Option<MarketEvent> {
        loop {
            match self.event_rx.recv().await {
                Some(event) => {
                    if self.is_subscribed(&event) {
                        return Some(event);
                    }
                    // 구독하지 않은 이벤트 건너뛰기
                }
                None => return None,
            }
        }
    }
}

/// 시뮬레이션된 사용자 데이터 스트림.
///
/// SimulatedExchange로부터 사용자 이벤트(주문 업데이트, 잔고 업데이트)를 수신합니다.
pub struct SimulatedUserStream {
    /// 이벤트 수신기
    event_rx: mpsc::Receiver<UserEvent>,
    /// 실행 상태
    running: bool,
}

impl SimulatedUserStream {
    /// 새로운 시뮬레이션 사용자 스트림을 생성합니다.
    pub fn new(event_rx: mpsc::Receiver<UserEvent>) -> Self {
        Self {
            event_rx,
            running: false,
        }
    }
}

#[async_trait]
impl UserStream for SimulatedUserStream {
    async fn start(&mut self) -> ExchangeResult<()> {
        self.running = true;
        Ok(())
    }

    async fn stop(&mut self) -> ExchangeResult<()> {
        self.running = false;
        Ok(())
    }

    async fn next_event(&mut self) -> Option<UserEvent> {
        if !self.running {
            return None;
        }
        self.event_rx.recv().await
    }
}

/// 시뮬레이션 거래소를 위한 이벤트 브로드캐스터.
///
/// 거래소가 여러 구독자에게 이벤트를 전송할 수 있게 합니다.
pub struct EventBroadcaster<T: Clone + Send> {
    /// 각 구독자의 송신기
    senders: Arc<RwLock<Vec<mpsc::Sender<T>>>>,
}

impl<T: Clone + Send> EventBroadcaster<T> {
    /// 새로운 이벤트 브로드캐스터를 생성합니다.
    pub fn new() -> Self {
        Self {
            senders: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 이벤트를 구독하고 수신기를 가져옵니다.
    pub async fn subscribe(&self, buffer_size: usize) -> mpsc::Receiver<T> {
        let (tx, rx) = mpsc::channel(buffer_size);
        self.senders.write().await.push(tx);
        rx
    }

    /// 모든 구독자에게 이벤트를 브로드캐스트합니다.
    pub async fn broadcast(&self, event: T) {
        let senders = self.senders.read().await;
        for sender in senders.iter() {
            // 전송 오류 무시 (구독자가 삭제되었을 수 있음)
            let _ = sender.send(event.clone()).await;
        }
    }

    /// 연결이 끊긴 구독자를 제거합니다.
    pub async fn cleanup(&self) {
        let mut senders = self.senders.write().await;
        senders.retain(|sender| !sender.is_closed());
    }
}

impl<T: Clone + Send> Default for EventBroadcaster<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use trader_core::{Symbol, Ticker};

    fn create_test_symbol() -> Symbol {
        Symbol::crypto("BTC", "USDT")
    }

    fn create_test_ticker() -> Ticker {
        Ticker {
            ticker: "BTC/USDT".to_string(),
            last: dec!(50000),
            bid: dec!(49999),
            ask: dec!(50001),
            high_24h: dec!(51000),
            low_24h: dec!(49000),
            volume_24h: dec!(1000),
            change_24h: dec!(500),
            change_24h_percent: dec!(1.0),
            timestamp: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_market_stream_subscription() {
        let (tx, rx) = mpsc::channel(100);
        let mut stream = SimulatedMarketStream::new(rx);

        stream.subscribe_ticker("BTC/USDT").await.unwrap();

        // 티커 이벤트 전송
        let ticker = create_test_ticker();
        tx.send(MarketEvent::Ticker(ticker.clone())).await.unwrap();

        // 이벤트를 수신해야 함
        let event = stream.next_event().await;
        assert!(matches!(event, Some(MarketEvent::Ticker(_))));
    }

    #[tokio::test]
    async fn test_market_stream_filter() {
        let (tx, rx) = mpsc::channel(100);
        let mut stream = SimulatedMarketStream::new(rx);

        // BTC만 구독
        stream.subscribe_ticker("BTC/USDT").await.unwrap();

        // ETH 티커 전송 (필터링되어야 함)
        let eth_ticker = Ticker {
            ticker: "ETH/USDT".to_string(),
            ..create_test_ticker()
        };
        tx.send(MarketEvent::Ticker(eth_ticker)).await.unwrap();

        // BTC 티커 전송
        let btc_ticker = create_test_ticker();
        tx.send(MarketEvent::Ticker(btc_ticker)).await.unwrap();

        // 스트림 종료를 위해 송신기 삭제
        drop(tx);

        // BTC 이벤트만 수신해야 함
        let event = stream.next_event().await;
        if let Some(MarketEvent::Ticker(ticker)) = event {
            assert_eq!(ticker.ticker, "BTC/USDT");
        } else {
            panic!("Expected BTC ticker");
        }
    }

    #[tokio::test]
    async fn test_event_broadcaster() {
        let broadcaster: EventBroadcaster<i32> = EventBroadcaster::new();

        let mut rx1 = broadcaster.subscribe(10).await;
        let mut rx2 = broadcaster.subscribe(10).await;

        broadcaster.broadcast(42).await;

        assert_eq!(rx1.recv().await, Some(42));
        assert_eq!(rx2.recv().await, Some(42));
    }
}
