use std::sync::Arc;
use std::str::FromStr;
use std::time::Duration;
use futures::{SinkExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use serde_json::json;

use trader_core::ProviderError;
use trader_core::QuoteData;
use rust_decimal::Decimal;
use chrono::Utc;
use tracing::{info, error};

const BITHUMB_WS_URL: &str = "wss://api.bithumb.com/pub/ws";
const MAX_RECONNECT_ATTEMPTS: u32 = 5;
const RECONNECT_DELAY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub enum BithumbWsMessage {
    Ticker(QuoteData),
    Error(String),
}

#[derive(Debug)]
pub enum BithumbWsCommand {
    SubscribeTicker(Vec<String>),
    UnsubscribeTicker(Vec<String>),
}

pub struct BithumbWebSocket {
    tx: mpsc::Sender<BithumbWsMessage>,
    rx: Option<mpsc::Receiver<BithumbWsMessage>>,
    command_tx: mpsc::Sender<BithumbWsCommand>,
    command_rx: Option<mpsc::Receiver<BithumbWsCommand>>,
    subscribed_tickers: Arc<RwLock<Vec<String>>>,
}

impl Default for BithumbWebSocket {
    fn default() -> Self {
        Self::new()
    }
}

impl BithumbWebSocket {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(100);
        let (cmd_tx, cmd_rx) = mpsc::channel(10);
        Self {
            tx,
            rx: Some(rx),
            command_tx: cmd_tx,
            command_rx: Some(cmd_rx),
            subscribed_tickers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<BithumbWsMessage>> {
        self.rx.take()
    }

    pub fn command_sender(&self) -> mpsc::Sender<BithumbWsCommand> {
        self.command_tx.clone()
    }

    pub async fn connect(&mut self) {
        let mut attempts = 0;
        let mut cmd_rx = self.command_rx.take().expect("command_rx already taken");

        loop {
            match self.run_session(&mut cmd_rx).await {
                Ok(_) => {
                    info!("Bithumb WebSocket session ended normally.");
                    break;
                }
                Err(e) => {
                    attempts += 1;
                    error!("Bithumb WebSocket error (attempt {}): {:?}", attempts, e);
                    if attempts >= MAX_RECONNECT_ATTEMPTS {
                        let _ = self.tx.send(BithumbWsMessage::Error("Max reconnect attempts reached".into())).await;
                        break;
                    }
                    tokio::time::sleep(RECONNECT_DELAY).await;
                }
            }
        }
    }

    async fn run_session(&self, cmd_rx: &mut mpsc::Receiver<BithumbWsCommand>) -> Result<(), ProviderError> {
        let (ws_stream, _) = connect_async(BITHUMB_WS_URL).await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        
        let (mut ws_tx, mut ws_rx) = ws_stream.split();
        info!("Connected to Bithumb WebSocket");

        // 접속 안정화 대기 (서버 초기화 완료 대기)
        tokio::time::sleep(Duration::from_millis(200)).await;

        self.send_subscription(&mut ws_tx).await?;

        let mut ping_interval = interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                Some(msg) = ws_rx.next() => {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                if val["type"] == "ticker" {
                                    if let Some(quote) = self.parse_ticker(&val["content"]) {
                                        let _ = self.tx.send(BithumbWsMessage::Ticker(quote)).await;
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
                        BithumbWsCommand::SubscribeTicker(codes) => {
                            let mut subs = self.subscribed_tickers.write().await;
                            for code in codes {
                                if !subs.contains(&code) {
                                    subs.push(code);
                                }
                            }
                            drop(subs);
                            self.send_subscription(&mut ws_tx).await?;
                        }
                        BithumbWsCommand::UnsubscribeTicker(codes) => {
                            let mut subs = self.subscribed_tickers.write().await;
                            subs.retain(|c| !codes.contains(c));
                            drop(subs);
                            self.send_subscription(&mut ws_tx).await?;
                        }
                    }
                }
                _ = ping_interval.tick() => {
                    let _ = ws_tx.send(Message::Ping(vec![])).await;
                }
            }
        }
        Ok(())
    }

    async fn send_subscription<S>(&self, ws_tx: &mut S) -> Result<(), ProviderError> 
    where S: futures::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin
    {
        let codes = self.subscribed_tickers.read().await;
        if codes.is_empty() { return Ok(()); }

        let msg = json!({
            "type": "ticker",
            "symbols": *codes,
            "tickTypes": ["24H"]
        });

        ws_tx.send(Message::Text(msg.to_string())).await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        
        Ok(())
    }

    fn parse_ticker(&self, content: &serde_json::Value) -> Option<QuoteData> {
        let symbol = content["symbol"].as_str()?.to_string();
        let current_price = Decimal::from_str(content["closePrice"].as_str()?).unwrap_or_default();
        
        Some(QuoteData {
            symbol,
            current_price,
            price_change: Decimal::from_str(content["chgAmt"].as_str().unwrap_or("0")).unwrap_or_default(),
            change_percent: Decimal::from_str(content["chgRate"].as_str().unwrap_or("0")).unwrap_or_default(),
            high: Decimal::from_str(content["highPrice"].as_str().unwrap_or("0")).unwrap_or_default(),
            low: Decimal::from_str(content["lowPrice"].as_str().unwrap_or("0")).unwrap_or_default(),
            open: Decimal::from_str(content["openPrice"].as_str().unwrap_or("0")).unwrap_or_default(),
            prev_close: Decimal::from_str(content["prevClosePrice"].as_str().unwrap_or("0")).unwrap_or_default(),
            volume: Decimal::from_str(content["volume"].as_str().unwrap_or("0")).unwrap_or_default(),
            trading_value: Decimal::from_str(content["value"].as_str().unwrap_or("0")).unwrap_or_default(),
            timestamp: Utc::now(),
        })
    }
}
