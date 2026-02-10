use std::sync::Arc;
use std::str::FromStr;
use std::time::Duration;
use futures::{SinkExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use serde_json::json;
use trader_core::{OrderBook, OrderBookLevel, ProviderError, QuoteData};
use rust_decimal::Decimal;
use chrono::Utc;
use tracing::{info, error, debug};

const LS_WS_URL: &str = "wss://openapi.ls-sec.co.kr:9443/websocket";
const MAX_RECONNECT_ATTEMPTS: u32 = 5;
const RECONNECT_DELAY: Duration = Duration::from_secs(5);
/// 구독 등록 간격 (밀리초). 거래소 규정: 건당 0.2초 이상 간격 권장.
const SUBSCRIBE_INTERVAL_MS: u64 = 200;

#[derive(Debug, Clone)]
pub enum LsWsMessage {
    Trade(QuoteData),
    Orderbook(OrderBook),
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

        // 접속 안정화 대기 (서버 초기화 완료 대기)
        tokio::time::sleep(Duration::from_millis(SUBSCRIBE_INTERVAL_MS)).await;

        // 구독 복원 (건당 0.2초 간격 준수)
        let subs = self.subscriptions.read().await;
        for (i, (tr_cd, tr_key)) in subs.iter().enumerate() {
            if i > 0 {
                tokio::time::sleep(Duration::from_millis(SUBSCRIBE_INTERVAL_MS)).await;
            }
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
                            // 구독 등록 간격 준수 (0.2초)
                            tokio::time::sleep(Duration::from_millis(SUBSCRIBE_INTERVAL_MS)).await;
                        }
                        LsWsCommand::Unsubscribe { tr_cd, tr_key } => {
                            let mut subs = self.subscriptions.write().await;
                            subs.retain(|(c, k)| c != &tr_cd || k != &tr_key);
                            drop(subs);
                            self.send_sub_msg(&mut ws_tx, &tr_cd, &tr_key, false).await?;
                            // 구독 해제 간격 준수 (0.2초)
                            tokio::time::sleep(Duration::from_millis(SUBSCRIBE_INTERVAL_MS)).await;
                        }
                    }
                }
                _ = ping_interval.tick() => {
                    let _ = ws_tx.send(Message::Ping(vec![])).await;
                }
            }
        }

        // 접속 해제 전 구독 해제 시도 (best-effort)
        {
            let subs = self.subscriptions.read().await;
            let subs_list: Vec<_> = subs.clone();
            drop(subs);
            if !subs_list.is_empty() {
                debug!("LS 접속 해제 전 구독 해제 시도: {} 건", subs_list.len());
                for (i, (tr_cd, tr_key)) in subs_list.iter().enumerate() {
                    if i > 0 {
                        tokio::time::sleep(Duration::from_millis(SUBSCRIBE_INTERVAL_MS)).await;
                    }
                    let _ = self.send_sub_msg(&mut ws_tx, tr_cd, tr_key, false).await;
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
        // LS WebSocket 프로토콜: 파이프(|) 구분자
        // 형식: 헤더|TR코드|데이터길이|데이터
        let parts: Vec<&str> = text.split('|').collect();
        if parts.len() < 4 {
            debug!("LS WS Control message: {}", text);
            return;
        }

        let tr_cd = parts[1];
        let data = parts[3];

        match tr_cd {
            "S3_" => {
                // 실시간 체결가 (국내)
                if let Some(quote) = self.parse_trade(tr_cd, data) {
                    let _ = self.tx.send(LsWsMessage::Trade(quote)).await;
                }
            }
            "HDF" => {
                // 실시간 체결가 (해외)
                if let Some(quote) = self.parse_trade(tr_cd, data) {
                    let _ = self.tx.send(LsWsMessage::Trade(quote)).await;
                }
            }
            "H1_" | "H2_" => {
                // 실시간 호가 (H1_: 10호가, H2_: 호가잔량)
                if let Some(ob) = self.parse_orderbook(data) {
                    let _ = self.tx.send(LsWsMessage::Orderbook(ob)).await;
                }
            }
            _ => {
                debug!("LS WS unknown tr_cd: {}", tr_cd);
            }
        }
    }

    fn parse_trade(&self, _tr_cd: &str, data: &str) -> Option<QuoteData> {
        let fields: Vec<&str> = data.split('^').collect();
        // LS 체결가 데이터 (간략화된 파싱)
        // 필드: [symbol, time, price, sign, change, rate, volume, ...]
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

    /// 실시간 호가 데이터 파싱
    ///
    /// LS 호가 데이터는 캐럿(^) 구분자로 필드가 구분됩니다.
    /// 일반적인 필드 구조 (실제 스펙에 따라 조정 필요):
    /// [종목코드, 시간, 매도10호가~매도1호가, 매도10잔량~매도1잔량, 매수1호가~매수10호가, 매수1잔량~매수10잔량, ...]
    fn parse_orderbook(&self, data: &str) -> Option<OrderBook> {
        let fields: Vec<&str> = data.split('^').collect();

        // 최소 필드 수 확인 (종목코드 + 호가/잔량 데이터)
        // 10호가 기준: 종목코드(1) + 시간(1) + 매도호가(10) + 매도잔량(10) + 매수호가(10) + 매수잔량(10) = 42개 이상
        if fields.len() < 42 {
            debug!("LS 호가 데이터 필드 부족: {} 개", fields.len());
            return None;
        }

        let symbol = fields[0].to_string();

        let mut asks = Vec::new();
        let mut bids = Vec::new();

        // LS 호가 필드 매핑 (일반적인 패턴, 실제 데이터에 따라 조정 필요):
        // [0]=종목코드, [1]=시간
        // [2]~[11]=매도10호가~매도1호가 (가격 오름차순)
        // [12]~[21]=매도10잔량~매도1잔량
        // [22]~[31]=매수1호가~매수10호가 (가격 내림차순)
        // [32]~[41]=매수1잔량~매수10잔량

        // 매도 호가 파싱 (5호가만 사용, 실제로는 10호가 모두 사용 가능)
        for i in 0..5 {
            let ask_price_idx = 7 + i; // 매도5~1호가 (필드 인덱스 조정)
            let ask_qty_idx = 17 + i;  // 매도5~1잔량

            if ask_price_idx < fields.len() && ask_qty_idx < fields.len() {
                if let (Ok(price), Ok(qty)) = (
                    Decimal::from_str(fields[ask_price_idx]),
                    Decimal::from_str(fields[ask_qty_idx])
                ) {
                    if price > Decimal::ZERO && qty > Decimal::ZERO {
                        asks.push(OrderBookLevel { price, quantity: qty });
                    }
                }
            }
        }

        // 매수 호가 파싱 (5호가만 사용)
        for i in 0..5 {
            let bid_price_idx = 22 + i; // 매수1~5호가
            let bid_qty_idx = 32 + i;   // 매수1~5잔량

            if bid_price_idx < fields.len() && bid_qty_idx < fields.len() {
                if let (Ok(price), Ok(qty)) = (
                    Decimal::from_str(fields[bid_price_idx]),
                    Decimal::from_str(fields[bid_qty_idx])
                ) {
                    if price > Decimal::ZERO && qty > Decimal::ZERO {
                        bids.push(OrderBookLevel { price, quantity: qty });
                    }
                }
            }
        }

        // 매도 호가: 가격 오름차순 정렬
        asks.sort_by(|a, b| a.price.cmp(&b.price));

        // 매수 호가: 가격 내림차순 정렬
        bids.sort_by(|a, b| b.price.cmp(&a.price));

        // 유효한 호가가 없으면 None 반환
        if asks.is_empty() && bids.is_empty() {
            return None;
        }

        Some(OrderBook {
            ticker: symbol,
            bids,
            asks,
            timestamp: Utc::now(),
        })
    }
}
