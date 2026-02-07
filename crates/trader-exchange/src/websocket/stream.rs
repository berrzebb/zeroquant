//! Binance용 WebSocket 스트림 구현.
//!
//! WebSocket 연결을 통해 실시간 시장 데이터 스트리밍을 제공합니다.

#![allow(dead_code)] // WebSocket 연결 상태 추적 필드

use crate::connector::binance::BinanceConfig;
use crate::traits::{ExchangeResult, MarketEvent, MarketStream};
use crate::ExchangeError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, info};
use trader_core::{
    Kline, MarketType, OrderBook, OrderBookLevel, Side, Symbol, Ticker, Timeframe, TradeTick,
};

// ============================================================================
// WebSocket 메시지 타입
// ============================================================================

/// Binance WebSocket 구독 메시지.
#[derive(Debug, Serialize)]
struct SubscribeMessage {
    method: String,
    params: Vec<String>,
    id: u64,
}

/// Binance 티커 스트림 이벤트.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WsTicker {
    #[serde(rename = "e")]
    event_type: String,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "c")]
    close: String,
    #[serde(rename = "b")]
    bid: String,
    #[serde(rename = "a")]
    ask: String,
    #[serde(rename = "v")]
    volume: String,
    #[serde(rename = "h")]
    high: String,
    #[serde(rename = "l")]
    low: String,
    #[serde(rename = "p")]
    price_change: String,
    #[serde(rename = "P")]
    price_change_percent: String,
}

/// Binance 캔들(kline) 스트림 이벤트.
#[derive(Debug, Deserialize)]
struct WsKlineEvent {
    #[serde(rename = "e")]
    event_type: String,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "k")]
    kline: WsKline,
}

#[derive(Debug, Deserialize)]
struct WsKline {
    #[serde(rename = "t")]
    open_time: i64,
    #[serde(rename = "T")]
    close_time: i64,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "i")]
    interval: String,
    #[serde(rename = "o")]
    open: String,
    #[serde(rename = "h")]
    high: String,
    #[serde(rename = "l")]
    low: String,
    #[serde(rename = "c")]
    close: String,
    #[serde(rename = "v")]
    volume: String,
    #[serde(rename = "q")]
    quote_volume: String,
    #[serde(rename = "n")]
    num_trades: u32,
    #[serde(rename = "x")]
    is_closed: bool,
}

/// Binance 체결 스트림 이벤트.
#[derive(Debug, Deserialize)]
struct WsTrade {
    #[serde(rename = "e")]
    event_type: String,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "t")]
    trade_id: i64,
    #[serde(rename = "p")]
    price: String,
    #[serde(rename = "q")]
    quantity: String,
    #[serde(rename = "T")]
    timestamp: i64,
    #[serde(rename = "m")]
    is_buyer_maker: bool,
}

/// Binance 호가창(order book) 스트림 이벤트.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WsDepth {
    #[serde(rename = "e")]
    event_type: String,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "b")]
    bids: Vec<[String; 2]>,
    #[serde(rename = "a")]
    asks: Vec<[String; 2]>,
}

// ============================================================================
// Binance 시장 스트림
// ============================================================================

/// Binance WebSocket 시장 데이터 스트림.
pub struct BinanceMarketStream {
    config: BinanceConfig,
    ws: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    subscriptions: HashSet<String>,
    event_rx: Option<mpsc::Receiver<MarketEvent>>,
    event_tx: Option<mpsc::Sender<MarketEvent>>,
    message_id: u64,
}

impl BinanceMarketStream {
    /// 새로운 Binance 시장 스트림을 생성합니다.
    pub fn new(config: BinanceConfig) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            config,
            ws: None,
            subscriptions: HashSet::new(),
            event_rx: Some(rx),
            event_tx: Some(tx),
            message_id: 1,
        }
    }

    /// WebSocket 서버에 연결합니다.
    pub async fn connect(&mut self) -> ExchangeResult<()> {
        let url = self.config.ws_base_url();
        info!("Connecting to Binance WebSocket: {}", url);

        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| ExchangeError::WebSocket(e.to_string()))?;

        self.ws = Some(ws_stream);
        info!("Connected to Binance WebSocket");

        Ok(())
    }

    /// WebSocket 서버와의 연결을 해제합니다.
    pub async fn disconnect(&mut self) -> ExchangeResult<()> {
        if let Some(mut ws) = self.ws.take() {
            ws.close(None)
                .await
                .map_err(|e| ExchangeError::WebSocket(e.to_string()))?;
        }
        self.subscriptions.clear();
        info!("Disconnected from Binance WebSocket");
        Ok(())
    }

    /// 구독 메시지를 전송합니다.
    async fn send_subscribe(&mut self, streams: Vec<String>) -> ExchangeResult<()> {
        let msg = SubscribeMessage {
            method: "SUBSCRIBE".to_string(),
            params: streams.clone(),
            id: self.message_id,
        };
        self.message_id += 1;

        let json =
            serde_json::to_string(&msg).map_err(|e| ExchangeError::ParseError(e.to_string()))?;

        if let Some(ws) = &mut self.ws {
            ws.send(Message::Text(json))
                .await
                .map_err(|e| ExchangeError::WebSocket(e.to_string()))?;

            for stream in streams {
                self.subscriptions.insert(stream);
            }
        } else {
            return Err(ExchangeError::Disconnected("Not connected".to_string()));
        }

        Ok(())
    }

    /// 구독 해제 메시지를 전송합니다.
    async fn send_unsubscribe(&mut self, streams: Vec<String>) -> ExchangeResult<()> {
        let msg = SubscribeMessage {
            method: "UNSUBSCRIBE".to_string(),
            params: streams.clone(),
            id: self.message_id,
        };
        self.message_id += 1;

        let json =
            serde_json::to_string(&msg).map_err(|e| ExchangeError::ParseError(e.to_string()))?;

        if let Some(ws) = &mut self.ws {
            ws.send(Message::Text(json))
                .await
                .map_err(|e| ExchangeError::WebSocket(e.to_string()))?;

            for stream in streams {
                self.subscriptions.remove(&stream);
            }
        }

        Ok(())
    }

    /// 티커 스트림 이름을 반환합니다.
    fn ticker_stream(ticker: &str) -> String {
        format!("{}@ticker", Self::format_symbol(ticker))
    }

    /// 캔들(kline) 스트림 이름을 반환합니다.
    fn kline_stream(ticker: &str, timeframe: Timeframe) -> String {
        format!(
            "{}@kline_{}",
            Self::format_symbol(ticker),
            timeframe.to_binance_interval()
        )
    }

    /// 호가창 스트림 이름을 반환합니다.
    fn depth_stream(ticker: &str) -> String {
        format!("{}@depth@100ms", Self::format_symbol(ticker))
    }

    /// 체결 스트림 이름을 반환합니다.
    fn trade_stream(ticker: &str) -> String {
        format!("{}@trade", Self::format_symbol(ticker))
    }

    /// Binance WebSocket용 심볼 형식으로 변환합니다. (BTC/USDT -> btcusdt)
    fn format_symbol(ticker: &str) -> String {
        ticker.replace("/", "").to_lowercase()
    }

    /// Binance 형식에서 심볼을 파싱합니다.
    fn parse_symbol(binance_symbol: &str) -> Symbol {
        let quotes = ["usdt", "busd", "btc", "eth", "bnb", "usdc"];
        let lower = binance_symbol.to_lowercase();

        for quote in quotes {
            if lower.ends_with(quote) {
                let base = &binance_symbol[..binance_symbol.len() - quote.len()];
                return Symbol::new(
                    base.to_uppercase(),
                    quote.to_uppercase(),
                    MarketType::Crypto,
                );
            }
        }

        Symbol::new(binance_symbol.to_uppercase(), "USDT", MarketType::Crypto)
    }

    /// 문자열에서 소수점 숫자를 파싱합니다.
    fn parse_decimal(s: &str) -> Decimal {
        s.parse().unwrap_or(Decimal::ZERO)
    }

    /// WebSocket 메시지를 MarketEvent로 파싱합니다.
    fn parse_message(text: &str) -> Option<MarketEvent> {
        // 다양한 이벤트 타입으로 파싱 시도
        if let Ok(ticker) = serde_json::from_str::<WsTicker>(text) {
            if ticker.event_type == "24hrTicker" {
                let symbol = Self::parse_symbol(&ticker.symbol);
                return Some(MarketEvent::Ticker(Ticker {
                    ticker: symbol.to_string(),
                    bid: Self::parse_decimal(&ticker.bid),
                    ask: Self::parse_decimal(&ticker.ask),
                    last: Self::parse_decimal(&ticker.close),
                    volume_24h: Self::parse_decimal(&ticker.volume),
                    high_24h: Self::parse_decimal(&ticker.high),
                    low_24h: Self::parse_decimal(&ticker.low),
                    change_24h: Self::parse_decimal(&ticker.price_change),
                    change_24h_percent: Self::parse_decimal(&ticker.price_change_percent),
                    timestamp: Utc::now(),
                }));
            }
        }

        if let Ok(kline_event) = serde_json::from_str::<WsKlineEvent>(text) {
            if kline_event.event_type == "kline" {
                let k = &kline_event.kline;
                let symbol = Self::parse_symbol(&k.symbol);
                let timeframe =
                    Timeframe::from_binance_interval(&k.interval).unwrap_or(Timeframe::M1);

                return Some(MarketEvent::Kline(Kline {
                    ticker: symbol.to_string(),
                    timeframe,
                    open_time: DateTime::from_timestamp_millis(k.open_time)
                        .unwrap_or_else(Utc::now),
                    open: Self::parse_decimal(&k.open),
                    high: Self::parse_decimal(&k.high),
                    low: Self::parse_decimal(&k.low),
                    close: Self::parse_decimal(&k.close),
                    volume: Self::parse_decimal(&k.volume),
                    close_time: DateTime::from_timestamp_millis(k.close_time)
                        .unwrap_or_else(Utc::now),
                    quote_volume: Some(Self::parse_decimal(&k.quote_volume)),
                    num_trades: Some(k.num_trades),
                }));
            }
        }

        if let Ok(trade) = serde_json::from_str::<WsTrade>(text) {
            if trade.event_type == "trade" {
                let symbol = Self::parse_symbol(&trade.symbol);
                return Some(MarketEvent::Trade(TradeTick {
                    ticker: symbol.to_string(),
                    id: trade.trade_id.to_string(),
                    price: Self::parse_decimal(&trade.price),
                    quantity: Self::parse_decimal(&trade.quantity),
                    side: if trade.is_buyer_maker {
                        Side::Sell
                    } else {
                        Side::Buy
                    },
                    timestamp: DateTime::from_timestamp_millis(trade.timestamp)
                        .unwrap_or_else(Utc::now),
                }));
            }
        }

        if let Ok(depth) = serde_json::from_str::<WsDepth>(text) {
            if depth.event_type == "depthUpdate" {
                let symbol = Self::parse_symbol(&depth.symbol);
                let bids = depth
                    .bids
                    .into_iter()
                    .map(|[p, q]| OrderBookLevel {
                        price: Self::parse_decimal(&p),
                        quantity: Self::parse_decimal(&q),
                    })
                    .collect();
                let asks = depth
                    .asks
                    .into_iter()
                    .map(|[p, q]| OrderBookLevel {
                        price: Self::parse_decimal(&p),
                        quantity: Self::parse_decimal(&q),
                    })
                    .collect();

                return Some(MarketEvent::OrderBook(OrderBook {
                    ticker: symbol.to_string(),
                    bids,
                    asks,
                    timestamp: Utc::now(),
                }));
            }
        }

        None
    }

    /// 메시지 처리 루프를 시작합니다.
    pub async fn run(&mut self) -> ExchangeResult<()> {
        let tx = self
            .event_tx
            .take()
            .ok_or_else(|| ExchangeError::Unknown("Event sender not available".to_string()))?;

        let ws = self
            .ws
            .take()
            .ok_or_else(|| ExchangeError::Disconnected("Not connected".to_string()))?;

        let (_write, mut read) = ws.split();

        // 수신 메시지를 처리하는 태스크 생성
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Some(event) = Self::parse_message(&text) {
                            if tx.send(event).await.is_err() {
                                error!("Failed to send event to channel");
                                break;
                            }
                        }
                    }
                    Ok(Message::Ping(_data)) => {
                        debug!("Received ping");
                        // Pong은 tungstenite에서 자동으로 처리됨
                    }
                    Ok(Message::Close(_)) => {
                        info!("WebSocket closed by server");
                        let _ = tx.send(MarketEvent::Disconnected).await;
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        let _ = tx.send(MarketEvent::Error(e.to_string())).await;
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }
}

#[async_trait]
impl MarketStream for BinanceMarketStream {
    async fn start(&mut self) -> ExchangeResult<()> {
        self.connect().await
    }

    fn is_started(&self) -> bool {
        self.ws.is_some()
    }

    async fn subscribe_ticker(&mut self, symbol: &str) -> ExchangeResult<()> {
        let stream = Self::ticker_stream(symbol);
        info!("Subscribing to ticker: {}", stream);
        self.send_subscribe(vec![stream]).await
    }

    async fn subscribe_kline(&mut self, symbol: &str, timeframe: Timeframe) -> ExchangeResult<()> {
        let stream = Self::kline_stream(symbol, timeframe);
        info!("Subscribing to kline: {}", stream);
        self.send_subscribe(vec![stream]).await
    }

    async fn subscribe_order_book(&mut self, symbol: &str) -> ExchangeResult<()> {
        let stream = Self::depth_stream(symbol);
        info!("Subscribing to depth: {}", stream);
        self.send_subscribe(vec![stream]).await
    }

    async fn subscribe_trades(&mut self, symbol: &str) -> ExchangeResult<()> {
        let stream = Self::trade_stream(symbol);
        info!("Subscribing to trades: {}", stream);
        self.send_subscribe(vec![stream]).await
    }

    async fn unsubscribe(&mut self, symbol: &str) -> ExchangeResult<()> {
        let streams: Vec<String> = self
            .subscriptions
            .iter()
            .filter(|s| s.starts_with(&Self::format_symbol(symbol)))
            .cloned()
            .collect();

        if !streams.is_empty() {
            info!("Unsubscribing from: {:?}", streams);
            self.send_unsubscribe(streams).await?;
        }

        Ok(())
    }

    async fn next_event(&mut self) -> Option<MarketEvent> {
        if let Some(rx) = &mut self.event_rx {
            rx.recv().await
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_symbol() {
        let ticker = "BTC/USDT";
        assert_eq!(BinanceMarketStream::format_symbol(ticker), "btcusdt");
    }

    #[test]
    fn test_parse_symbol() {
        let symbol = BinanceMarketStream::parse_symbol("ETHUSDT");
        assert_eq!(symbol.base, "ETH");
        assert_eq!(symbol.quote, "USDT");
    }

    #[test]
    fn test_stream_names() {
        let ticker = "BTC/USDT";

        assert_eq!(BinanceMarketStream::ticker_stream(ticker), "btcusdt@ticker");
        assert_eq!(
            BinanceMarketStream::kline_stream(ticker, Timeframe::H1),
            "btcusdt@kline_1h"
        );
        assert_eq!(
            BinanceMarketStream::depth_stream(ticker),
            "btcusdt@depth@100ms"
        );
        assert_eq!(BinanceMarketStream::trade_stream(ticker), "btcusdt@trade");
    }
}
