//! DB증권 실시간 시세 WebSocket 클라이언트.
//!
//! DB금융투자(현 DB증권) WebSocket API를 통해 실시간 체결가와 호가를 수신합니다.
//!
//! # 지원 채널
//!
//! - `V60`: 실시간 체결가
//! - `V20`: 실시간 호가
//!
//! # 사용 예제
//!
//! ```rust,ignore
//! use trader_exchange::connector::db_investment::DbInvestmentWebSocket;
//!
//! let mut ws = DbInvestmentWebSocket::new(access_token, true);
//!
//! // 삼성전자(005930) 실시간 체결가 구독
//! ws.subscribe_trade("005930").await?;
//!
//! // 메시지 수신
//! while let Some(msg) = ws.recv().await {
//!     println!("Received: {:?}", msg);
//! }
//! ```

use crate::ExchangeError;
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde_json::json;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use trader_core::{OrderBook, OrderBookLevel, QuoteData};
use tracing::{debug, error, info, warn};

/// 연결 중 동적 구독/해제를 위한 명령.
#[derive(Debug)]
pub enum DbWsCommand {
    /// 실시간 구독 추가 (tr_cd, tr_key)
    Subscribe { tr_cd: String, tr_key: String },
    /// 실시간 구독 해제 (tr_cd, tr_key)
    Unsubscribe { tr_cd: String, tr_key: String },
}

/// DB증권 WebSocket 메시지 타입.
#[derive(Debug, Clone)]
pub enum DbWsMessage {
    /// 체결가
    Trade(QuoteData),
    /// 호가
    Orderbook(OrderBook),
    /// 연결 상태 변경
    ConnectionStatus(bool),
    /// 에러
    Error(String),
}

/// 재연결 최대 시도 횟수.
const MAX_RECONNECT_ATTEMPTS: u32 = 3;

/// 재연결 대기 시간 (초).
const RECONNECT_DELAY_SECS: u64 = 5;

/// Ping 간격 (초).
const PING_INTERVAL_SECS: u64 = 30;

/// DB증권 실시간 WebSocket 클라이언트.
///
/// 동적 구독을 지원합니다. `command_tx`를 통해 연결 중에도 구독/해제가 가능합니다.
pub struct DbInvestmentWebSocket {
    /// 접근 토큰 (향후 WebSocket 인증에 사용 가능)
    #[allow(dead_code)]
    access_token: String,
    /// 운영/모의투자 구분 (true: 운영, false: 모의투자)
    is_prod: bool,
    /// 메시지 전송용 채널
    tx: mpsc::Sender<DbWsMessage>,
    /// 메시지 수신용 채널
    rx: Option<mpsc::Receiver<DbWsMessage>>,
    /// 동적 구독/해제 명령 전송용
    command_tx: mpsc::Sender<DbWsCommand>,
    /// 동적 구독/해제 명령 수신용 (connect 루프 내부에서 사용)
    command_rx: Option<mpsc::Receiver<DbWsCommand>>,
    /// 구독 중인 체결가 종목
    subscribed_trades: Vec<String>,
    /// 구독 중인 호가 종목
    subscribed_orderbooks: Vec<String>,
}

impl DbInvestmentWebSocket {
    /// 새로운 DB증권 WebSocket 클라이언트 생성.
    ///
    /// # Arguments
    /// * `access_token` - OAuth2 접근 토큰
    /// * `is_prod` - 운영/모의투자 구분 (true: 운영, false: 모의투자)
    pub fn new(access_token: String, is_prod: bool) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        Self {
            access_token,
            is_prod,
            tx,
            rx: Some(rx),
            command_tx: cmd_tx,
            command_rx: Some(cmd_rx),
            subscribed_trades: Vec::new(),
            subscribed_orderbooks: Vec::new(),
        }
    }

    /// 메시지 수신 채널 가져오기.
    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<DbWsMessage>> {
        self.rx.take()
    }

    /// 동적 구독 명령 전송 채널 가져오기.
    pub fn command_sender(&self) -> mpsc::Sender<DbWsCommand> {
        self.command_tx.clone()
    }

    /// WebSocket 연결 및 메시지 수신 시작.
    ///
    /// 이 메서드는 별도 태스크에서 실행해야 합니다.
    /// 재연결 로직이 포함되어 있습니다.
    pub async fn connect(&mut self) -> Result<(), ExchangeError> {
        let mut reconnect_attempts = 0;

        loop {
            match self.run_session().await {
                Ok(_) => {
                    // 정상 종료
                    info!("DB증권 WebSocket 연결 종료");
                    break;
                }
                Err(e) => {
                    error!("DB증권 WebSocket 에러: {}", e);
                    reconnect_attempts += 1;

                    if reconnect_attempts > MAX_RECONNECT_ATTEMPTS {
                        error!(
                            "최대 재연결 시도 횟수 초과 ({}회)",
                            MAX_RECONNECT_ATTEMPTS
                        );
                        let _ = self
                            .tx
                            .send(DbWsMessage::Error(format!(
                                "최대 재연결 시도 횟수 초과: {}",
                                e
                            )))
                            .await;
                        return Err(e);
                    }

                    warn!(
                        "{}초 후 재연결 시도 ({}/{})",
                        RECONNECT_DELAY_SECS, reconnect_attempts, MAX_RECONNECT_ATTEMPTS
                    );
                    tokio::time::sleep(Duration::from_secs(RECONNECT_DELAY_SECS)).await;
                }
            }
        }

        Ok(())
    }

    /// 내부 연결 로직.
    ///
    /// command channel을 통해 연결 중 동적 구독/해제 명령을 수신합니다.
    async fn run_session(&mut self) -> Result<(), ExchangeError> {
        // WebSocket URL 선택
        let ws_url = if self.is_prod {
            "wss://openapi.dbsec.co.kr:7070/websocket"
        } else {
            "wss://openapi.dbsec.co.kr:17070/websocket"
        };

        info!("DB증권 WebSocket 연결 중: {}", ws_url);

        // WebSocket 연결
        let (ws_stream, _) = connect_async(ws_url)
            .await
            .map_err(|e| ExchangeError::NetworkError(format!("WebSocket 연결 실패: {}", e)))?;

        let (mut write, mut read) = ws_stream.split();

        // 연결 성공 알림
        let _ = self.tx.send(DbWsMessage::ConnectionStatus(true)).await;
        info!("DB증권 WebSocket 연결 성공");

        // command_rx를 take하여 이 연결 세션에서 사용
        let mut cmd_rx = self.command_rx.take().unwrap_or_else(|| {
            let (tx, rx) = mpsc::channel(64);
            self.command_tx = tx;
            rx
        });

        // 기존 구독 복원
        let trades = self.subscribed_trades.clone();
        let orderbooks = self.subscribed_orderbooks.clone();

        for symbol in &trades {
            let msg = json!({
                "tr_cd": "V60",
                "tr_key": symbol
            });
            write
                .send(Message::Text(msg.to_string()))
                .await
                .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;
            debug!("체결가 구독 복원: {}", symbol);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        for symbol in &orderbooks {
            let msg = json!({
                "tr_cd": "V20",
                "tr_key": symbol
            });
            write
                .send(Message::Text(msg.to_string()))
                .await
                .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;
            debug!("호가 구독 복원: {}", symbol);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Ping 타이머
        let mut ping_interval = interval(Duration::from_secs(PING_INTERVAL_SECS));

        // 메시지 수신 루프 (동적 구독 명령도 처리)
        loop {
            tokio::select! {
                // WebSocket 메시지 수신
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Err(e) = self.handle_message(&text).await {
                                error!("메시지 파싱 실패: {}", e);
                            }
                        }
                        Some(Ok(Message::Ping(data))) => {
                            debug!("Ping 수신, Pong 응답");
                            let _ = write.send(Message::Pong(data)).await;
                        }
                        Some(Ok(Message::Close(_))) => {
                            warn!("서버에서 연결 종료 요청");
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WebSocket 수신 에러: {}", e);
                            break;
                        }
                        None => {
                            warn!("WebSocket 스트림 종료");
                            break;
                        }
                        _ => {}
                    }
                }
                // 동적 구독/해제 명령 수신
                Some(cmd) = cmd_rx.recv() => {
                    match cmd {
                        DbWsCommand::Subscribe { tr_cd, tr_key } => {
                            let msg = json!({
                                "tr_cd": tr_cd,
                                "tr_key": tr_key
                            });
                            if let Err(e) = write.send(Message::Text(msg.to_string())).await {
                                error!("동적 구독 전송 실패 ({}/{}): {}", tr_cd, tr_key, e);
                            } else {
                                info!("동적 구독 성공: {}/{}", tr_cd, tr_key);
                                if tr_cd == "V60" {
                                    self.add_trade_subscription(&tr_key);
                                } else if tr_cd == "V20" {
                                    self.add_orderbook_subscription(&tr_key);
                                }
                            }
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                        DbWsCommand::Unsubscribe { tr_cd, tr_key } => {
                            let msg = json!({
                                "tr_cd": tr_cd,
                                "tr_key": tr_key,
                                "unsubscribe": true
                            });
                            if let Err(e) = write.send(Message::Text(msg.to_string())).await {
                                error!("동적 구독 해제 전송 실패 ({}/{}): {}", tr_cd, tr_key, e);
                            } else {
                                info!("동적 구독 해제 성공: {}/{}", tr_cd, tr_key);
                                if tr_cd == "V60" {
                                    self.remove_trade_subscription(&tr_key);
                                } else if tr_cd == "V20" {
                                    self.remove_orderbook_subscription(&tr_key);
                                }
                            }
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
                // Ping 전송
                _ = ping_interval.tick() => {
                    debug!("Ping 전송");
                    if let Err(e) = write.send(Message::Ping(vec![])).await {
                        error!("Ping 전송 실패: {}", e);
                        break;
                    }
                }
            }
        }

        // 접속 해제 전 구독 해제 시도 (best-effort)
        {
            let all_trades = self.subscribed_trades.clone();
            let all_orderbooks = self.subscribed_orderbooks.clone();
            let total = all_trades.len() + all_orderbooks.len();
            if total > 0 {
                debug!("접속 해제 전 구독 해제 시도: {} 건", total);
                for symbol in &all_trades {
                    let msg = json!({
                        "tr_cd": "V60",
                        "tr_key": symbol,
                        "unsubscribe": true
                    });
                    let _ = write.send(Message::Text(msg.to_string())).await;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                for symbol in &all_orderbooks {
                    let msg = json!({
                        "tr_cd": "V20",
                        "tr_key": symbol,
                        "unsubscribe": true
                    });
                    let _ = write.send(Message::Text(msg.to_string())).await;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        // 연결 종료 시 command_rx를 복원하여 재연결에서 재사용
        self.command_rx = Some(cmd_rx);

        // 연결 종료 알림
        let _ = self.tx.send(DbWsMessage::ConnectionStatus(false)).await;

        Err(ExchangeError::NetworkError("연결 끊김".to_string()))
    }

    /// 수신 메시지 처리.
    async fn handle_message(&self, text: &str) -> Result<(), ExchangeError> {
        let value: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| ExchangeError::ParseError(format!("JSON 파싱 실패: {}", e)))?;

        let tr_cd = value
            .get("tr_cd")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ExchangeError::ParseError("tr_cd 필드 없음".into()))?;

        match tr_cd {
            "V60" => {
                // 실시간 체결가
                let quote = self.parse_trade(&value)?;
                let _ = self.tx.send(DbWsMessage::Trade(quote)).await;
            }
            "V20" => {
                // 실시간 호가
                let orderbook = self.parse_orderbook(&value)?;
                let _ = self.tx.send(DbWsMessage::Orderbook(orderbook)).await;
            }
            _ => {
                debug!("알 수 없는 tr_cd: {}", tr_cd);
            }
        }

        Ok(())
    }

    /// 체결 데이터 파싱.
    fn parse_trade(&self, value: &serde_json::Value) -> Result<QuoteData, ExchangeError> {
        let ticker = value
            .get("tr_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ExchangeError::ParseError("tr_key 필드 없음".into()))?
            .to_string();

        // 현재가 (Prpr 필드)
        let price_str = value
            .get("Prpr")
            .and_then(|v| {
                v.as_str()
                    .map(String::from)
                    .or_else(|| v.as_f64().map(|f| f.to_string()))
            })
            .ok_or_else(|| ExchangeError::ParseError("Prpr 필드 없음".into()))?;

        let current_price = Decimal::from_str(&price_str)
            .map_err(|e| ExchangeError::ParseError(format!("가격 파싱 실패: {}", e)))?;

        // 거래량 (VolumeQty 필드)
        let volume = value
            .get("VolumeQty")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let volume_decimal = Decimal::from(volume);

        // 거래대금 (VolumeValue 필드)
        let trading_value = value
            .get("VolumeValue")
            .and_then(|v| v.as_f64())
            .map(Decimal::from_f64_retain)
            .unwrap_or(Some(Decimal::ZERO))
            .unwrap_or(Decimal::ZERO);

        // 변화율 (ChangeRate 필드, %)
        let change_percent = value
            .get("ChangeRate")
            .and_then(|v| v.as_f64())
            .map(Decimal::from_f64_retain)
            .unwrap_or(Some(Decimal::ZERO))
            .unwrap_or(Decimal::ZERO);

        // 전일대비 가격 변동 (현재가 * 변화율 / 100)
        let price_change = current_price * change_percent / Decimal::from(100);

        // 전일 종가 계산
        let prev_close = current_price - price_change;

        Ok(QuoteData {
            symbol: ticker,
            current_price,
            price_change,
            change_percent,
            high: current_price, // 실시간 체결에는 일일 고저가 없으므로 현재가 사용
            low: current_price,
            open: current_price,
            prev_close,
            volume: volume_decimal,
            trading_value,
            timestamp: Utc::now(),
        })
    }

    /// 호가 데이터 파싱.
    fn parse_orderbook(&self, value: &serde_json::Value) -> Result<OrderBook, ExchangeError> {
        let ticker = value
            .get("tr_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ExchangeError::ParseError("tr_key 필드 없음".into()))?
            .to_string();

        let mut asks = Vec::new();
        let mut bids = Vec::new();

        // 5호가 파싱
        for i in 1..=5 {
            // 매도호가
            let ask_price_key = format!("AskPrice{}", i);
            let ask_qty_key = format!("AskQty{}", i);
            if let (Some(price_val), Some(qty)) = (
                value.get(&ask_price_key),
                value.get(&ask_qty_key).and_then(|v| v.as_i64()),
            ) {
                let price_str = price_val
                    .as_str()
                    .map(String::from)
                    .or_else(|| price_val.as_f64().map(|f| f.to_string()))
                    .ok_or_else(|| {
                        ExchangeError::ParseError(format!("AskPrice{} 파싱 실패", i))
                    })?;

                let price = Decimal::from_str(&price_str)
                    .map_err(|e| ExchangeError::ParseError(format!("가격 파싱 실패: {}", e)))?;
                let quantity = Decimal::from(qty);

                asks.push(OrderBookLevel { price, quantity });
            }

            // 매수호가
            let bid_price_key = format!("BidPrice{}", i);
            let bid_qty_key = format!("BidQty{}", i);
            if let (Some(price_val), Some(qty)) = (
                value.get(&bid_price_key),
                value.get(&bid_qty_key).and_then(|v| v.as_i64()),
            ) {
                let price_str = price_val
                    .as_str()
                    .map(String::from)
                    .or_else(|| price_val.as_f64().map(|f| f.to_string()))
                    .ok_or_else(|| {
                        ExchangeError::ParseError(format!("BidPrice{} 파싱 실패", i))
                    })?;

                let price = Decimal::from_str(&price_str)
                    .map_err(|e| ExchangeError::ParseError(format!("가격 파싱 실패: {}", e)))?;
                let quantity = Decimal::from(qty);

                bids.push(OrderBookLevel { price, quantity });
            }
        }

        Ok(OrderBook {
            ticker,
            asks,
            bids,
            timestamp: Utc::now(),
        })
    }

    /// 실시간 체결가 구독.
    pub async fn subscribe_trade(&self, ticker: &str) -> Result<(), ExchangeError> {
        self.command_tx
            .send(DbWsCommand::Subscribe {
                tr_cd: "V60".to_string(),
                tr_key: ticker.to_string(),
            })
            .await
            .map_err(|e| ExchangeError::NetworkError(format!("구독 명령 전송 실패: {}", e)))?;
        Ok(())
    }

    /// 실시간 호가 구독.
    pub async fn subscribe_orderbook(&self, ticker: &str) -> Result<(), ExchangeError> {
        self.command_tx
            .send(DbWsCommand::Subscribe {
                tr_cd: "V20".to_string(),
                tr_key: ticker.to_string(),
            })
            .await
            .map_err(|e| ExchangeError::NetworkError(format!("구독 명령 전송 실패: {}", e)))?;
        Ok(())
    }

    /// 체결가 구독 추가 (내부용).
    fn add_trade_subscription(&mut self, symbol: &str) {
        if !self.subscribed_trades.contains(&symbol.to_string()) {
            self.subscribed_trades.push(symbol.to_string());
        }
    }

    /// 호가 구독 추가 (내부용).
    fn add_orderbook_subscription(&mut self, symbol: &str) {
        if !self.subscribed_orderbooks.contains(&symbol.to_string()) {
            self.subscribed_orderbooks.push(symbol.to_string());
        }
    }

    /// 체결가 구독 제거 (내부용).
    fn remove_trade_subscription(&mut self, symbol: &str) {
        self.subscribed_trades.retain(|s| s != symbol);
    }

    /// 호가 구독 제거 (내부용).
    fn remove_orderbook_subscription(&mut self, symbol: &str) {
        self.subscribed_orderbooks.retain(|s| s != symbol);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_trade_data() {
        let ws = DbInvestmentWebSocket::new("test_token".to_string(), true);

        let json = json!({
            "tr_cd": "V60",
            "tr_key": "005930",
            "Prpr": "70500",
            "VolumeQty": 1500,
            "VolumeValue": 105750000.0,
            "ChangeRate": 1.23
        });

        let result = ws.parse_trade(&json);
        assert!(result.is_ok());

        let quote = result.unwrap();
        assert_eq!(quote.symbol, "005930");
        assert_eq!(quote.current_price, Decimal::new(70500, 0));
        assert_eq!(quote.volume, Decimal::from(1500));
    }

    #[test]
    fn test_parse_orderbook_data() {
        let ws = DbInvestmentWebSocket::new("test_token".to_string(), true);

        let json = json!({
            "tr_cd": "V20",
            "tr_key": "005930",
            "AskPrice1": "70600",
            "AskQty1": 100,
            "AskPrice2": "70700",
            "AskQty2": 200,
            "BidPrice1": "70500",
            "BidQty1": 150,
            "BidPrice2": "70400",
            "BidQty2": 250
        });

        let result = ws.parse_orderbook(&json);
        assert!(result.is_ok());

        let orderbook = result.unwrap();
        assert_eq!(orderbook.ticker, "005930");
        assert_eq!(orderbook.asks.len(), 2);
        assert_eq!(orderbook.bids.len(), 2);
        assert_eq!(orderbook.asks[0].price, Decimal::new(70600, 0));
        assert_eq!(orderbook.bids[0].quantity, Decimal::from(150));
    }

    #[test]
    fn test_subscribe_message_format() {
        let _ws = DbInvestmentWebSocket::new("test_token".to_string(), true);

        let msg = json!({
            "tr_cd": "V60",
            "tr_key": "005930"
        });

        let msg_str = msg.to_string();
        assert!(msg_str.contains("V60"));
        assert!(msg_str.contains("005930"));
    }

    #[test]
    fn test_unsubscribe_message_format() {
        let msg = json!({
            "tr_cd": "V60",
            "tr_key": "005930",
            "unsubscribe": true
        });

        let msg_str = msg.to_string();
        assert!(msg_str.contains("unsubscribe"));
        assert!(msg_str.contains("true"));
    }
}
