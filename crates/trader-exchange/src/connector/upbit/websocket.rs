use std::sync::Arc;

use std::time::Duration;
use futures::{SinkExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use serde_json::json;
use trader_core::{OrderBook, OrderBookLevel, ProviderError, QuoteData, Side, TradeTick};
use rust_decimal::Decimal;
use chrono::Utc;
use tracing::{info, error};

const UPBIT_WS_URL: &str = "wss://api.upbit.com/websocket/v1";
const MAX_RECONNECT_ATTEMPTS: u32 = 5;
const RECONNECT_DELAY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub enum UpbitWsMessage {
    Ticker(QuoteData),
    Orderbook(OrderBook),
    Trade(TradeTick),
    Error(String),
}

#[derive(Debug)]
pub enum UpbitWsCommand {
    SubscribeTicker(Vec<String>),
    SubscribeOrderbook(Vec<String>),
    SubscribeTrade(Vec<String>),
    UnsubscribeTicker(Vec<String>),
}

pub struct UpbitWebSocket {
    tx: mpsc::Sender<UpbitWsMessage>,
    rx: Option<mpsc::Receiver<UpbitWsMessage>>,
    command_tx: mpsc::Sender<UpbitWsCommand>,
    command_rx: Option<mpsc::Receiver<UpbitWsCommand>>,
    subscribed_tickers: Arc<RwLock<Vec<String>>>,
    subscribed_orderbooks: Arc<RwLock<Vec<String>>>,
    subscribed_trades: Arc<RwLock<Vec<String>>>,
}

impl Default for UpbitWebSocket {
    fn default() -> Self {
        Self::new()
    }
}

impl UpbitWebSocket {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(100);
        let (cmd_tx, cmd_rx) = mpsc::channel(10);
        Self {
            tx,
            rx: Some(rx),
            command_tx: cmd_tx,
            command_rx: Some(cmd_rx),
            subscribed_tickers: Arc::new(RwLock::new(Vec::new())),
            subscribed_orderbooks: Arc::new(RwLock::new(Vec::new())),
            subscribed_trades: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<UpbitWsMessage>> {
        self.rx.take()
    }

    pub fn command_sender(&self) -> mpsc::Sender<UpbitWsCommand> {
        self.command_tx.clone()
    }

    pub async fn connect(&mut self) {
        let mut attempts = 0;
        let mut cmd_rx = self.command_rx.take().expect("command_rx already taken");

        loop {
            match self.run_session(&mut cmd_rx).await {
                Ok(_) => {
                    info!("Upbit WebSocket session ended normally.");
                    break;
                }
                Err(e) => {
                    attempts += 1;
                    error!("Upbit WebSocket error (attempt {}): {:?}", attempts, e);
                    if attempts >= MAX_RECONNECT_ATTEMPTS {
                        let _ = self.tx.send(UpbitWsMessage::Error("Max reconnect attempts reached".into())).await;
                        break;
                    }
                    tokio::time::sleep(RECONNECT_DELAY).await;
                }
            }
        }
    }

    async fn run_session(&self, cmd_rx: &mut mpsc::Receiver<UpbitWsCommand>) -> Result<(), ProviderError> {
        let (ws_stream, _) = connect_async(UPBIT_WS_URL).await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        
        let (mut ws_tx, mut ws_rx) = ws_stream.split::<Message>();
        info!("Connected to Upbit WebSocket");

        // 접속 안정화 대기 (서버 초기화 완료 대기)
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Initial subscription
        self.send_subscription(&mut ws_tx).await?;

        let mut ping_interval = interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                Some(msg) = ws_rx.next() => {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                if val["type"] == "ticker" {
                                    if let Some(quote) = self.parse_ticker(&val) {
                                        let _ = self.tx.send(UpbitWsMessage::Ticker(quote)).await;
                                    }
                                } else if val["type"] == "orderbook" {
                                    if let Some(ob) = self.parse_orderbook(&val) {
                                        let _ = self.tx.send(UpbitWsMessage::Orderbook(ob)).await;
                                    }
                                } else if val["type"] == "trade" {
                                    if let Some(tick) = self.parse_trade(&val) {
                                        let _ = self.tx.send(UpbitWsMessage::Trade(tick)).await;
                                    }
                                }
                            }
                        }
                        Ok(Message::Ping(_)) => {
                            let _ = ws_tx.send(Message::Pong(vec![])).await;
                        }
                        Ok(Message::Close(_)) => break,
                        Err(e) => return Err(ProviderError::Network(e.to_string())),
                        _ => {}
                    }
                }
                Some(cmd) = cmd_rx.recv() => {
                    match cmd {
                        UpbitWsCommand::SubscribeTicker(codes) => {
                            let mut subs = self.subscribed_tickers.write().await;
                            for code in codes {
                                if !subs.contains(&code) {
                                    subs.push(code);
                                }
                            }
                            drop(subs);
                            self.send_subscription(&mut ws_tx).await?;
                        }
                        UpbitWsCommand::SubscribeOrderbook(codes) => {
                            let mut subs = self.subscribed_orderbooks.write().await;
                            for code in codes {
                                if !subs.contains(&code) {
                                    subs.push(code);
                                }
                            }
                            drop(subs);
                            self.send_subscription(&mut ws_tx).await?;
                        }
                        UpbitWsCommand::SubscribeTrade(codes) => {
                            let mut subs = self.subscribed_trades.write().await;
                            for code in codes {
                                if !subs.contains(&code) {
                                    subs.push(code);
                                }
                            }
                            drop(subs);
                            self.send_subscription(&mut ws_tx).await?;
                        }
                        UpbitWsCommand::UnsubscribeTicker(codes) => {
                            let mut subs = self.subscribed_tickers.write().await;
                            subs.retain(|c| !codes.contains(c));
                            drop(subs);
                            self.send_subscription(&mut ws_tx).await?;
                        }
                    }
                }
                _ = ping_interval.tick() => {
                    // Upbit doesn't strictly need client-sent pings, but it doesn't hurt
                    let _ = ws_tx.send(Message::Ping(vec![])).await;
                }
            }
        }
        Ok(())
    }

    async fn send_subscription<S>(&self, ws_tx: &mut S) -> Result<(), ProviderError>
    where S: futures::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin
    {
        let ticker_codes = self.subscribed_tickers.read().await;
        let ob_codes = self.subscribed_orderbooks.read().await;
        let trade_codes = self.subscribed_trades.read().await;

        if ticker_codes.is_empty() && ob_codes.is_empty() && trade_codes.is_empty() {
            return Ok(());
        }

        let mut msg_array = vec![
            json!({"ticket": "upbit-ws-orderbook"}),
        ];

        if !ticker_codes.is_empty() {
            msg_array.push(json!({
                "type": "ticker",
                "codes": *ticker_codes,
                "isOnlyRealtime": true
            }));
        }

        if !ob_codes.is_empty() {
            msg_array.push(json!({
                "type": "orderbook",
                "codes": *ob_codes,
                "isOnlyRealtime": true
            }));
        }

        if !trade_codes.is_empty() {
            msg_array.push(json!({
                "type": "trade",
                "codes": *trade_codes,
                "isOnlyRealtime": true
            }));
        }

        let msg_str = serde_json::to_string(&msg_array)
            .map_err(|e| ProviderError::Parse(e.to_string()))?;

        ws_tx.send(Message::Text(msg_str)).await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        Ok(())
    }

    fn parse_ticker(&self, val: &serde_json::Value) -> Option<QuoteData> {
        let symbol = val["code"].as_str()?.to_string();
        let current_price = Decimal::from_f64_retain(val["trade_price"].as_f64()?).unwrap_or_default();

        Some(QuoteData {
            symbol,
            current_price,
            price_change: Decimal::from_f64_retain(val["change_price"].as_f64().unwrap_or(0.0)).unwrap_or_default(),
            change_percent: Decimal::from_f64_retain(val["change_rate"].as_f64().unwrap_or(0.0)).unwrap_or_default(),
            high: Decimal::from_f64_retain(val["high_price"].as_f64().unwrap_or(0.0)).unwrap_or_default(),
            low: Decimal::from_f64_retain(val["low_price"].as_f64().unwrap_or(0.0)).unwrap_or_default(),
            open: Decimal::from_f64_retain(val["opening_price"].as_f64().unwrap_or(0.0)).unwrap_or_default(),
            prev_close: Decimal::from_f64_retain(val["prev_closing_price"].as_f64().unwrap_or(0.0)).unwrap_or_default(),
            volume: Decimal::from_f64_retain(val["acc_trade_volume_24h"].as_f64().unwrap_or(0.0)).unwrap_or_default(),
            trading_value: Decimal::from_f64_retain(val["acc_trade_price_24h"].as_f64().unwrap_or(0.0)).unwrap_or_default(),
            timestamp: Utc::now(),
        })
    }

    /// Upbit orderbook JSON 메시지를 OrderBook 구조체로 파싱
    fn parse_orderbook(&self, val: &serde_json::Value) -> Option<OrderBook> {
        let code = val["code"].as_str()?.to_string();
        let units = val["orderbook_units"].as_array()?;

        let mut bids = Vec::new();
        let mut asks = Vec::new();

        for unit in units {
            if let (Some(ask_price), Some(bid_price), Some(ask_size), Some(bid_size)) = (
                unit["ask_price"].as_f64(),
                unit["bid_price"].as_f64(),
                unit["ask_size"].as_f64(),
                unit["bid_size"].as_f64(),
            ) {
                asks.push(OrderBookLevel {
                    price: Decimal::from_f64_retain(ask_price).unwrap_or_default(),
                    quantity: Decimal::from_f64_retain(ask_size).unwrap_or_default(),
                });
                bids.push(OrderBookLevel {
                    price: Decimal::from_f64_retain(bid_price).unwrap_or_default(),
                    quantity: Decimal::from_f64_retain(bid_size).unwrap_or_default(),
                });
            }
        }

        // 매도 호가는 가격 오름차순 정렬 (낮은 가격부터)
        asks.sort_by(|a, b| a.price.cmp(&b.price));
        // 매수 호가는 가격 내림차순 정렬 (높은 가격부터)
        bids.sort_by(|a, b| b.price.cmp(&a.price));

        Some(OrderBook {
            ticker: code,
            bids,
            asks,
            timestamp: Utc::now(),
        })
    }

    /// Upbit trade JSON 메시지를 TradeTick 구조체로 파싱
    fn parse_trade(&self, val: &serde_json::Value) -> Option<TradeTick> {
        let symbol = val["code"].as_str()?.to_string();

        // 가격 파싱
        let price = val["trade_price"].as_f64()
            .and_then(Decimal::from_f64_retain)?;

        // 수량 파싱
        let quantity = val["trade_volume"].as_f64()
            .and_then(Decimal::from_f64_retain)?;

        // 체결 방향 파싱 (ASK = 매도, BID = 매수)
        let side = match val["ask_bid"].as_str()? {
            "ASK" => Side::Sell,
            "BID" => Side::Buy,
            _ => return None,
        };

        // 체결 ID (sequential_id를 문자열로 변환)
        let id = val["sequential_id"]
            .as_u64()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "0".to_string());

        Some(TradeTick {
            ticker: symbol,
            id,
            price,
            quantity,
            side,
            timestamp: Utc::now(),
        })
    }
}
