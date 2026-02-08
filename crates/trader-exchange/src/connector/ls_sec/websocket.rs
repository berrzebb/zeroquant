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
use tracing::{info, error, debug};

const LS_WS_URL: &str = "wss://openapi.ls-sec.co.kr:9443/websocket";
const MAX_RECONNECT_ATTEMPTS: u32 = 5;
const RECONNECT_DELAY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub enum LsWsMessage {
    Trade(QuoteData),
    Orderbook(serde_json::Value),
    Error(String),
}

#[derive(Debug)]
pub enum LsWsCommand {
    Subscribe { tr_cd: String, tr_key: String },
    Unsubscribe { tr_cd: String, tr_key: String },
}

pub struct LsSecWebSocket {
    token: String,
    tx: mpsc::Sender<LsWsMessage>,
    rx: Option<mpsc::Receiver<LsWsMessage>>,
    command_tx: mpsc::Sender<LsWsCommand>,
    command_rx: Option<mpsc::Receiver<LsWsCommand>>,
    subscriptions: Arc<RwLock<Vec<(String, String)>>>,
}

impl LsSecWebSocket {
    pub fn new(token: String) -> Self {
        let (tx, rx) = mpsc::channel(100);
        let (cmd_tx, cmd_rx) = mpsc::channel(10);
        Self {
            token,
            tx,
            rx: Some(rx),
            command_tx: cmd_tx,
            command_rx: Some(cmd_rx),
            subscriptions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<LsWsMessage>> {
        self.rx.take()
    }

    pub fn command_sender(&self) -> mpsc::Sender<LsWsCommand> {
        self.command_tx.clone()
    }

    pub async fn connect(&mut self) {
        let mut attempts = 0;
        let mut cmd_rx = self.command_rx.take().expect("command_rx already taken");

        loop {
            match self.run_session(&mut cmd_rx).await {
                Ok(_) => {
                    info!("LS WebSocket session ended normally.");
                    break;
                }
                Err(e) => {
                    attempts += 1;
                    error!("LS WebSocket error (attempt {}): {:?}", attempts, e);
                    if attempts >= MAX_RECONNECT_ATTEMPTS {
                        let _ = self.tx.send(LsWsMessage::Error("Max reconnect attempts reached".into())).await;
                        break;
                    }
                    tokio::time::sleep(RECONNECT_DELAY).await;
                }
            }
        }
    }

    async fn run_session(&self, cmd_rx: &mut mpsc::Receiver<LsWsCommand>) -> Result<(), ProviderError> {
        let (ws_stream, _) = connect_async(LS_WS_URL).await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        
        let (mut ws_tx, mut ws_rx) = ws_stream.split::<Message>();
        info!("Connected to LS Securities WebSocket");

        // Restore subscriptions
        let subs = self.subscriptions.read().await;
        for (tr_cd, tr_key) in subs.iter() {
            self.send_sub_msg(&mut ws_tx, tr_cd, tr_key, true).await?;
        }
        drop(subs);

        let mut ping_interval = interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                Some(msg) = ws_rx.next() => {
                    match msg {
                        Ok(Message::Text(text)) => {
                            self.handle_message(&text).await;
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
                        LsWsCommand::Subscribe { tr_cd, tr_key } => {
                            let mut subs = self.subscriptions.write().await;
                            if !subs.iter().any(|(c, k)| c == &tr_cd && k == &tr_key) {
                                subs.push((tr_cd.clone(), tr_key.clone()));
                            }
                            drop(subs);
                            self.send_sub_msg(&mut ws_tx, &tr_cd, &tr_key, true).await?;
                        }
                        LsWsCommand::Unsubscribe { tr_cd, tr_key } => {
                            let mut subs = self.subscriptions.write().await;
                            subs.retain(|(c, k)| c != &tr_cd || k != &tr_key);
                            drop(subs);
                            self.send_sub_msg(&mut ws_tx, &tr_cd, &tr_key, false).await?;
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

    async fn send_sub_msg<S>(&self, ws_tx: &mut S, tr_cd: &str, tr_key: &str, subscribe: bool) -> Result<(), ProviderError>
    where S: futures::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin
    {
        let msg = json!({
            "header": {
                "token": self.token,
                "tr_cd": tr_cd,
                "tr_type": if subscribe { "1" } else { "2" }
            },
            "body": {
                "tr_key": tr_key
            }
        });

        ws_tx.send(Message::Text(msg.to_string())).await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        
        Ok(())
    }

    async fn handle_message(&self, text: &str) {
        // LS uses pipes '|' for data fields, but JSON for headers?
        // Actually LS WebSocket often sends raw bytes or specialized format.
        // For simplicity, we'll try to parse if it's JSON or log it.
        // Based on KIS pattern, we should parse the pipe-separated values.
        let parts: Vec<&str> = text.split('|').collect();
        if parts.len() < 4 {
            debug!("LS WS Control message: {}", text);
            return;
        }

        let tr_cd = parts[1];
        let data = parts[3];

        match tr_cd {
            "H1_" | "HDF" => {
                // KR Trade or US Trade
                if let Some(quote) = self.parse_trade(tr_cd, data) {
                    let _ = self.tx.send(LsWsMessage::Trade(quote)).await;
                }
            }
            _ => {
                // Other (Orderbook etc)
            }
        }
    }

    fn parse_trade(&self, _tr_cd: &str, data: &str) -> Option<QuoteData> {
        let fields: Vec<&str> = data.split('^').collect();
        // Very simplified parsing as exact field mapping is complex
        // tr_cd H1_ fields: [symbol, time, price, sign, change, rate, ...]
        if fields.len() < 10 { return None; }

        Some(QuoteData {
            symbol: fields[0].to_string(),
            current_price: Decimal::from_str(fields[2]).unwrap_or_default(),
            price_change: Decimal::from_str(fields[4]).unwrap_or_default(),
            change_percent: Decimal::from_str(fields[5]).unwrap_or_default(),
            high: Decimal::ZERO,
            low: Decimal::ZERO,
            open: Decimal::ZERO,
            prev_close: Decimal::ZERO,
            volume: Decimal::from_str(fields[fields.len()-1]).unwrap_or_default(),
            trading_value: Decimal::ZERO,
            timestamp: Utc::now(),
        })
    }
}
