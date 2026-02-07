//! 거래소 중립적 시장 데이터 스트림.
//!
//! 다양한 거래소의 WebSocket 연결을 `MarketStream` trait으로 래핑하여
//! 통합된 인터페이스를 제공합니다.
//!
//! # 동적 구독 지원
//!
//! `start()` 호출 전후 모두 `subscribe_*` / `unsubscribe` 가능합니다.
//! - 연결 전: 내부 큐에 추가 (연결 시 일괄 구독)
//! - 연결 후: command channel을 통해 실시간 구독/해제
//!
//! # 사용 예제
//!
//! ```rust,ignore
//! use trader_exchange::stream::KisKrMarketStream;
//! use trader_exchange::connector::kis::KisOAuth;
//!
//! let mut stream = KisKrMarketStream::new(oauth);
//!
//! // 연결 전 구독 설정
//! stream.subscribe_ticker("005930").await?;
//!
//! // 연결 시작
//! stream.start().await?;
//!
//! // 연결 후에도 동적 구독 가능
//! stream.subscribe_ticker("035420").await?;
//!
//! // 이벤트 수신
//! while let Some(event) = stream.next_event().await {
//!     match event {
//!         MarketEvent::Ticker(ticker) => println!("Ticker: {:?}", ticker),
//!         MarketEvent::OrderBook(book) => println!("Orderbook: {:?}", book),
//!         _ => {}
//!     }
//! }
//! ```

use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use trader_core::{OrderBook, OrderBookLevel, Side, Symbol, Ticker, Timeframe, TradeTick};

use crate::connector::kis::{
    tr_id, KisKrWebSocket, KisOAuth, KisUsClient, KisUsWebSocket, KrRealtimeMessage,
    KrRealtimeOrderbook, KrRealtimeTrade, UsRealtimeMessage, UsRealtimeOrderbook, UsRealtimeTrade,
    WsCommand, UsWsCommand,
};
use crate::traits::{ExchangeResult, MarketEvent, MarketStream};
use crate::ExchangeError;

// ============================================================================
// KIS 국내 MarketStream
// ============================================================================

/// KIS 국내 주식용 MarketStream 구현.
///
/// `KisKrWebSocket`을 래핑하여 `MarketStream` trait을 구현합니다.
///
/// # 동적 구독
///
/// `start()` 전후 모두 구독/해제 가능합니다.
/// - 연결 전: WebSocket 큐에 추가 (연결 시 일괄 전송)
/// - 연결 후: command channel을 통해 실시간 전송
pub struct KisKrMarketStream {
    ws: Arc<RwLock<KisKrWebSocket>>,
    rx: Option<mpsc::Receiver<KrRealtimeMessage>>,
    /// 동적 구독을 위한 command sender (연결 후 사용)
    cmd_tx: mpsc::Sender<WsCommand>,
    subscribed_symbols: HashMap<String, SubscriptionType>,
    started: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum SubscriptionType {
    Trade,
    Orderbook,
    Both,
}

impl KisKrMarketStream {
    /// 새로운 KIS 국내 MarketStream 생성.
    pub fn new(oauth: KisOAuth) -> Self {
        let mut ws = KisKrWebSocket::new(oauth);
        let rx = ws.take_receiver();
        let cmd_tx = ws.command_sender();

        Self {
            ws: Arc::new(RwLock::new(ws)),
            rx,
            cmd_tx,
            subscribed_symbols: HashMap::new(),
            started: false,
        }
    }



    /// 종목코드에서 Symbol 생성 (국내).
    #[allow(dead_code)]
    fn code_to_symbol(code: &str) -> Symbol {
        Symbol::stock(code, "KRW")
    }

    /// KrRealtimeTrade를 Ticker로 변환.
    fn trade_to_ticker(trade: &KrRealtimeTrade) -> Ticker {
        let change_percent = if trade.change_rate != Decimal::ZERO {
            trade.change_rate
        } else {
            dec!(0)
        };

        Ticker {
            ticker: trade.symbol.clone(),
            bid: trade.price - dec!(10), // 근사값 (실제로는 호가 데이터 필요)
            ask: trade.price + dec!(10), // 근사값
            last: trade.price,
            volume_24h: Decimal::from(trade.acc_volume),
            high_24h: trade.price, // KIS 실시간에서 미제공 - 현재가로 대체
            low_24h: trade.price,  // KIS 실시간에서 미제공 - 현재가로 대체
            change_24h: trade.change,
            change_24h_percent: change_percent,
            timestamp: Utc::now(),
        }
    }

    /// KrRealtimeTrade를 TradeTick으로 변환.
    #[allow(dead_code)]
    fn trade_to_tick(trade: &KrRealtimeTrade) -> TradeTick {
        // KIS에서는 체결 방향을 직접 제공하지 않음 - sign 필드로 추정
        let side = match trade.sign.as_str() {
            "1" | "2" => Side::Buy,  // 상한, 상승
            "4" | "5" => Side::Sell, // 하한, 하락
            _ => Side::Buy,          // 보합 등 기타 - 기본값
        };

        TradeTick {
            ticker: trade.symbol.clone(),
            id: trade.trade_time.clone(), // 체결시간을 ID로 사용
            price: trade.price,
            quantity: Decimal::from(trade.volume),
            side,
            timestamp: Utc::now(),
        }
    }

    /// KrRealtimeOrderbook을 OrderBook으로 변환.
    fn orderbook_to_book(ob: &KrRealtimeOrderbook) -> OrderBook {
        let bids: Vec<OrderBookLevel> = ob
            .bid_prices
            .iter()
            .zip(ob.bid_volumes.iter())
            .map(|(price, volume)| OrderBookLevel {
                price: *price,
                quantity: Decimal::from(*volume),
            })
            .collect();

        let asks: Vec<OrderBookLevel> = ob
            .ask_prices
            .iter()
            .zip(ob.ask_volumes.iter())
            .map(|(price, volume)| OrderBookLevel {
                price: *price,
                quantity: Decimal::from(*volume),
            })
            .collect();

        OrderBook {
            ticker: ob.symbol.clone(),
            bids,
            asks,
            timestamp: Utc::now(),
        }
    }
}

#[async_trait]
impl MarketStream for KisKrMarketStream {
    async fn start(&mut self) -> ExchangeResult<()> {
        if self.started {
            return Ok(());
        }

        let ws = self.ws.clone();
        self.started = true;

        tokio::spawn(async move {
            let mut ws_guard = ws.write().await;
            if let Err(e) = ws_guard.connect().await {
                error!("KIS KR WebSocket 연결 실패: {}", e);
            }
        });

        info!("KIS KR MarketStream 시작됨");
        Ok(())
    }

    fn is_started(&self) -> bool {
        self.started
    }

    async fn subscribe_ticker(&mut self, symbol: &str) -> ExchangeResult<()> {
        let code = symbol.to_string();

        if self.started {
            // 연결 후: command channel을 통해 실시간 구독
            self.cmd_tx
                .send(WsCommand::Subscribe {
                    tr_id: tr_id::WS_KR_TRADE.to_string(),
                    tr_key: code.clone(),
                })
                .await
                .map_err(|e| ExchangeError::NetworkError(format!("동적 구독 전송 실패: {}", e)))?;
            info!("KR 티커 동적 구독: {}", code);
        } else {
            // 연결 전: 내부 큐에 추가
            let mut ws = self.ws.write().await;
            ws.add_trade_subscription(&code);
            info!("KR 티커 구독 설정: {}", code);
        }

        self.subscribed_symbols
            .entry(code)
            .and_modify(|t| {
                if *t == SubscriptionType::Orderbook {
                    *t = SubscriptionType::Both;
                }
            })
            .or_insert(SubscriptionType::Trade);

        Ok(())
    }

    async fn subscribe_kline(
        &mut self,
        _symbol: &str,
        _timeframe: Timeframe,
    ) -> ExchangeResult<()> {
        // KIS WebSocket은 실시간 캔들스틱을 지원하지 않음
        warn!("KIS는 실시간 캔들스틱을 지원하지 않습니다");
        Err(ExchangeError::NotSupported(
            "KIS does not support real-time kline streaming".to_string(),
        ))
    }

    async fn subscribe_order_book(&mut self, symbol: &str) -> ExchangeResult<()> {
        let code = symbol.to_string();

        if self.started {
            // 연결 후: command channel을 통해 실시간 구독
            self.cmd_tx
                .send(WsCommand::Subscribe {
                    tr_id: tr_id::WS_KR_ORDERBOOK.to_string(),
                    tr_key: code.clone(),
                })
                .await
                .map_err(|e| ExchangeError::NetworkError(format!("동적 호가 구독 전송 실패: {}", e)))?;
            info!("KR 호가 동적 구독: {}", code);
        } else {
            // 연결 전: 내부 큐에 추가
            let mut ws = self.ws.write().await;
            ws.add_orderbook_subscription(&code);
            info!("KR 호가 구독 설정: {}", code);
        }

        self.subscribed_symbols
            .entry(code)
            .and_modify(|t| {
                if *t == SubscriptionType::Trade {
                    *t = SubscriptionType::Both;
                }
            })
            .or_insert(SubscriptionType::Orderbook);

        Ok(())
    }

    async fn subscribe_trades(&mut self, symbol: &str) -> ExchangeResult<()> {
        // 체결 구독 = Ticker 구독과 동일
        self.subscribe_ticker(symbol).await
    }

    async fn unsubscribe(&mut self, symbol: &str) -> ExchangeResult<()> {
        let code = symbol.to_string();

        if self.started {
            // 연결 후: command channel을 통해 실시간 구독 해제
            if let Some(sub_type) = self.subscribed_symbols.remove(&code) {
                match sub_type {
                    SubscriptionType::Trade | SubscriptionType::Both => {
                        self.cmd_tx
                            .send(WsCommand::Unsubscribe {
                                tr_id: tr_id::WS_KR_TRADE.to_string(),
                                tr_key: code.clone(),
                            })
                            .await
                            .map_err(|e| ExchangeError::NetworkError(format!("구독 해제 전송 실패: {}", e)))?;
                    }
                    SubscriptionType::Orderbook => {}
                }
                if sub_type == SubscriptionType::Both || sub_type == SubscriptionType::Orderbook {
                    self.cmd_tx
                        .send(WsCommand::Unsubscribe {
                            tr_id: tr_id::WS_KR_ORDERBOOK.to_string(),
                            tr_key: code.clone(),
                        })
                        .await
                        .map_err(|e| ExchangeError::NetworkError(format!("호가 구독 해제 전송 실패: {}", e)))?;
                }
                info!("KR 동적 구독 해제: {}", code);
            }
        } else {
            // 연결 전: 내부 큐에서 제거
            let mut ws = self.ws.write().await;
            if let Some(sub_type) = self.subscribed_symbols.remove(&code) {
                match sub_type {
                    SubscriptionType::Trade | SubscriptionType::Both => {
                        ws.remove_trade_subscription(&code);
                    }
                    SubscriptionType::Orderbook => {
                        ws.remove_orderbook_subscription(&code);
                    }
                }
                if sub_type == SubscriptionType::Both {
                    ws.remove_orderbook_subscription(&code);
                }
            }
        }

        Ok(())
    }

    async fn next_event(&mut self) -> Option<MarketEvent> {
        let rx = self.rx.as_mut()?;

        match rx.recv().await {
            Some(KrRealtimeMessage::Trade(trade)) => {
                debug!("KR Trade: {} @ {}", trade.symbol, trade.price);
                Some(MarketEvent::Ticker(Self::trade_to_ticker(&trade)))
            }
            Some(KrRealtimeMessage::Orderbook(ob)) => {
                debug!("KR Orderbook: {}", ob.symbol);
                Some(MarketEvent::OrderBook(Self::orderbook_to_book(&ob)))
            }
            Some(KrRealtimeMessage::ConnectionStatus(connected)) => {
                if connected {
                    info!("KIS KR WebSocket 연결됨");
                    Some(MarketEvent::Connected)
                } else {
                    warn!("KIS KR WebSocket 연결 끊김");
                    Some(MarketEvent::Disconnected)
                }
            }
            Some(KrRealtimeMessage::Error(msg)) => {
                error!("KIS KR WebSocket 에러: {}", msg);
                Some(MarketEvent::Error(msg))
            }
            None => None,
        }
    }
}

// ============================================================================
// KIS 해외 MarketStream
// ============================================================================

/// US 구독 정보 (거래소 코드 포함).
#[derive(Clone)]
struct UsSubscriptionInfo {
    sub_type: SubscriptionType,
    exchange_code: String,
}

/// KIS 해외 주식용 MarketStream 구현.
///
/// # 동적 구독
///
/// `start()` 전후 모두 구독/해제 가능합니다.
/// - 연결 전: WebSocket 큐에 추가 (연결 시 일괄 전송)
/// - 연결 후: command channel을 통해 실시간 전송
pub struct KisUsMarketStream {
    ws: Arc<RwLock<KisUsWebSocket>>,
    rx: Option<mpsc::Receiver<UsRealtimeMessage>>,
    /// 동적 구독을 위한 command sender (연결 후 사용)
    cmd_tx: mpsc::Sender<UsWsCommand>,
    subscribed_symbols: HashMap<String, UsSubscriptionInfo>,
    started: bool,
}

impl KisUsMarketStream {
    /// 새로운 KIS 해외 MarketStream 생성.
    pub fn new(oauth: KisOAuth) -> Self {
        let mut ws = KisUsWebSocket::new(oauth);
        let rx = ws.take_receiver();
        let cmd_tx = ws.command_sender();

        Self {
            ws: Arc::new(RwLock::new(ws)),
            rx,
            cmd_tx,
            subscribed_symbols: HashMap::new(),
            started: false,
        }
    }



    /// 티커에서 Symbol 생성 (해외).
    #[allow(dead_code)]
    fn ticker_to_symbol(ticker: &str) -> Symbol {
        Symbol::stock(ticker, "USD")
    }

    /// UsRealtimeTrade를 Ticker로 변환.
    fn trade_to_ticker(trade: &UsRealtimeTrade) -> Ticker {
        Ticker {
            ticker: trade.symbol.clone(),
            bid: trade.price - dec!(0.01),
            ask: trade.price + dec!(0.01),
            last: trade.price,
            volume_24h: Decimal::from(trade.volume), // 체결량 (누적거래량 미제공)
            high_24h: trade.price,                   // KIS 실시간에서 미제공 - 현재가로 대체
            low_24h: trade.price,                    // KIS 실시간에서 미제공 - 현재가로 대체
            change_24h: trade.change,
            change_24h_percent: trade.change_rate,
            timestamp: Utc::now(),
        }
    }

    /// UsRealtimeOrderbook을 OrderBook으로 변환.
    fn orderbook_to_book(ob: &UsRealtimeOrderbook) -> OrderBook {
        // US는 단일 호가만 제공
        let bids = vec![OrderBookLevel {
            price: ob.bid_price,
            quantity: Decimal::from(ob.bid_volume),
        }];

        let asks = vec![OrderBookLevel {
            price: ob.ask_price,
            quantity: Decimal::from(ob.ask_volume),
        }];

        OrderBook {
            ticker: ob.symbol.clone(),
            bids,
            asks,
            timestamp: Utc::now(),
        }
    }
}

#[async_trait]
impl MarketStream for KisUsMarketStream {
    async fn start(&mut self) -> ExchangeResult<()> {
        if self.started {
            return Ok(());
        }

        let ws = self.ws.clone();
        self.started = true;

        tokio::spawn(async move {
            let mut ws_guard = ws.write().await;
            if let Err(e) = ws_guard.connect().await {
                error!("KIS US WebSocket 연결 실패: {}", e);
            }
        });

        info!("KIS US MarketStream 시작됨");
        Ok(())
    }

    fn is_started(&self) -> bool {
        self.started
    }

    async fn subscribe_ticker(&mut self, symbol: &str) -> ExchangeResult<()> {
        let ticker = symbol.to_string();
        let exchange_code = KisUsClient::get_exchange_code(symbol).to_string();
        // US WebSocket의 tr_key 형식: D{거래소코드}{심볼}
        let tr_key = format!("D{}{}", exchange_code, ticker);

        if self.started {
            // 연결 후: command channel을 통해 실시간 구독
            self.cmd_tx
                .send(UsWsCommand::Subscribe {
                    tr_id: tr_id::WS_US_TRADE.to_string(),
                    tr_key: tr_key.clone(),
                })
                .await
                .map_err(|e| ExchangeError::NetworkError(format!("US 동적 구독 전송 실패: {}", e)))?;
            info!("US 티커 동적 구독: {} ({})", ticker, exchange_code);
        } else {
            // 연결 전: 내부 큐에 추가
            let mut ws = self.ws.write().await;
            ws.add_trade_subscription(&ticker, &exchange_code);
            info!("US 티커 구독 설정: {} ({})", ticker, exchange_code);
        }

        self.subscribed_symbols
            .entry(ticker)
            .and_modify(|info| {
                if info.sub_type == SubscriptionType::Orderbook {
                    info.sub_type = SubscriptionType::Both;
                }
            })
            .or_insert(UsSubscriptionInfo {
                sub_type: SubscriptionType::Trade,
                exchange_code: exchange_code.clone(),
            });

        Ok(())
    }

    async fn subscribe_kline(
        &mut self,
        _symbol: &str,
        _timeframe: Timeframe,
    ) -> ExchangeResult<()> {
        warn!("KIS는 실시간 캔들스틱을 지원하지 않습니다");
        Err(ExchangeError::NotSupported(
            "KIS does not support real-time kline streaming".to_string(),
        ))
    }

    async fn subscribe_order_book(&mut self, symbol: &str) -> ExchangeResult<()> {
        let ticker = symbol.to_string();
        let exchange_code = KisUsClient::get_exchange_code(symbol).to_string();
        let tr_key = format!("D{}{}", exchange_code, ticker);

        if self.started {
            // 연결 후: command channel을 통해 실시간 구독
            self.cmd_tx
                .send(UsWsCommand::Subscribe {
                    tr_id: tr_id::WS_US_ORDERBOOK.to_string(),
                    tr_key: tr_key.clone(),
                })
                .await
                .map_err(|e| ExchangeError::NetworkError(format!("US 호가 동적 구독 전송 실패: {}", e)))?;
            info!("US 호가 동적 구독: {} ({})", ticker, exchange_code);
        } else {
            // 연결 전: 내부 큐에 추가
            let mut ws = self.ws.write().await;
            ws.add_orderbook_subscription(&ticker, &exchange_code);
            info!("US 호가 구독 설정: {} ({})", ticker, exchange_code);
        }

        self.subscribed_symbols
            .entry(ticker)
            .and_modify(|info| {
                if info.sub_type == SubscriptionType::Trade {
                    info.sub_type = SubscriptionType::Both;
                }
            })
            .or_insert(UsSubscriptionInfo {
                sub_type: SubscriptionType::Orderbook,
                exchange_code: exchange_code.clone(),
            });

        Ok(())
    }

    async fn subscribe_trades(&mut self, symbol: &str) -> ExchangeResult<()> {
        self.subscribe_ticker(symbol).await
    }

    async fn unsubscribe(&mut self, symbol: &str) -> ExchangeResult<()> {
        let ticker = symbol.to_string();

        if self.started {
            // 연결 후: command channel을 통해 실시간 구독 해제
            if let Some(info) = self.subscribed_symbols.remove(&ticker) {
                let exchange_code = &info.exchange_code;
                let tr_key = format!("D{}{}", exchange_code, ticker);

                match info.sub_type {
                    SubscriptionType::Trade | SubscriptionType::Both => {
                        self.cmd_tx
                            .send(UsWsCommand::Unsubscribe {
                                tr_id: tr_id::WS_US_TRADE.to_string(),
                                tr_key: tr_key.clone(),
                            })
                            .await
                            .map_err(|e| ExchangeError::NetworkError(format!("US 구독 해제 전송 실패: {}", e)))?;
                    }
                    SubscriptionType::Orderbook => {}
                }
                if info.sub_type == SubscriptionType::Both || info.sub_type == SubscriptionType::Orderbook {
                    self.cmd_tx
                        .send(UsWsCommand::Unsubscribe {
                            tr_id: tr_id::WS_US_ORDERBOOK.to_string(),
                            tr_key,
                        })
                        .await
                        .map_err(|e| ExchangeError::NetworkError(format!("US 호가 구독 해제 전송 실패: {}", e)))?;
                }
                info!("US 동적 구독 해제: {}", ticker);
            }
        } else {
            // 연결 전: 내부 큐에서 제거
            let mut ws = self.ws.write().await;
            if let Some(info) = self.subscribed_symbols.remove(&ticker) {
                match info.sub_type {
                    SubscriptionType::Trade | SubscriptionType::Both => {
                        ws.remove_trade_subscription(&ticker, &info.exchange_code);
                    }
                    SubscriptionType::Orderbook => {
                        ws.remove_orderbook_subscription(&ticker, &info.exchange_code);
                    }
                }
                if info.sub_type == SubscriptionType::Both {
                    ws.remove_orderbook_subscription(&ticker, &info.exchange_code);
                }
            }
        }

        Ok(())
    }

    async fn next_event(&mut self) -> Option<MarketEvent> {
        let rx = self.rx.as_mut()?;

        match rx.recv().await {
            Some(UsRealtimeMessage::Trade(trade)) => {
                debug!("US Trade: {} @ {}", trade.symbol, trade.price);
                Some(MarketEvent::Ticker(Self::trade_to_ticker(&trade)))
            }
            Some(UsRealtimeMessage::Orderbook(ob)) => {
                debug!("US Orderbook: {}", ob.symbol);
                Some(MarketEvent::OrderBook(Self::orderbook_to_book(&ob)))
            }
            Some(UsRealtimeMessage::ConnectionStatus(connected)) => {
                if connected {
                    info!("KIS US WebSocket 연결됨");
                    Some(MarketEvent::Connected)
                } else {
                    warn!("KIS US WebSocket 연결 끊김");
                    Some(MarketEvent::Disconnected)
                }
            }
            Some(UsRealtimeMessage::Error(msg)) => {
                error!("KIS US WebSocket 에러: {}", msg);
                Some(MarketEvent::Error(msg))
            }
            None => None,
        }
    }
}

// ============================================================================
// 통합 MarketStream (여러 거래소 지원)
// ============================================================================

/// Bridge 태스크로 전달되는 스트림 제어 명령.
#[derive(Debug)]
enum StreamCommand {
    /// 심볼 구독 추가
    Subscribe { symbol: String },
    /// 심볼 구독 해제
    Unsubscribe { symbol: String },
    /// 호가 구독 추가
    SubscribeOrderBook { symbol: String },
}

/// 여러 거래소를 통합하는 MarketStream.
///
/// 국내(KR)와 해외(US) 시장을 모두 지원하며,
/// 심볼에 따라 적절한 스트림으로 라우팅합니다.
///
/// # Bridge Task 패턴
///
/// `start()` 시 KR/US 스트림을 각각 별도 tokio 태스크로 분리하고,
/// 이벤트를 하나의 `mpsc` 채널로 통합하여 동시 수신합니다.
/// 동적 구독은 command 채널을 통해 bridge 태스크로 전파됩니다.
///
/// # Mock 스트림 지원
///
/// Paper Trading 시 Mock 거래소의 스트림도 통합 지원합니다.
/// Mock 모드가 활성화되면 실제 거래소 스트림 대신 Mock 스트림을 사용합니다.
pub struct UnifiedMarketStream {
    /// 연결 전 KR 스트림 (start 시 bridge 태스크로 소유권 이동)
    kr_stream: Option<Box<dyn MarketStream>>,
    /// 연결 전 US 스트림 (start 시 bridge 태스크로 소유권 이동)
    us_stream: Option<Box<dyn MarketStream>>,
    /// Mock 거래소 스트림 (Paper Trading용, bridge 태스크로 이동)
    mock_stream: Option<Box<dyn MarketStream>>,
    /// Mock 모드 활성화 여부
    mock_mode: bool,
    /// bridge 태스크에서 통합된 이벤트를 수신하는 채널
    event_rx: Option<mpsc::Receiver<MarketEvent>>,
    /// KR bridge 태스크에 명령을 보내는 채널
    kr_cmd_tx: Option<mpsc::Sender<StreamCommand>>,
    /// US bridge 태스크에 명령을 보내는 채널
    us_cmd_tx: Option<mpsc::Sender<StreamCommand>>,
    /// Mock bridge 태스크에 명령을 보내는 채널
    mock_cmd_tx: Option<mpsc::Sender<StreamCommand>>,
    started: bool,
}

impl UnifiedMarketStream {
    /// 새로운 통합 MarketStream 생성.
    pub fn new() -> Self {
        Self {
            kr_stream: None,
            us_stream: None,
            mock_stream: None,
            mock_mode: false,
            event_rx: None,
            kr_cmd_tx: None,
            us_cmd_tx: None,
            mock_cmd_tx: None,
            started: false,
        }
    }

    /// 국내 시장 스트림 추가.
    pub fn with_kr_stream(mut self, stream: impl MarketStream + 'static) -> Self {
        self.kr_stream = Some(Box::new(stream));
        self
    }

    /// 해외 시장 스트림 추가.
    pub fn with_us_stream(mut self, stream: impl MarketStream + 'static) -> Self {
        self.us_stream = Some(Box::new(stream));
        self
    }

    /// Mock 스트림 추가 (Paper Trading용).
    pub fn with_mock_stream(mut self, stream: impl MarketStream + 'static) -> Self {
        self.mock_stream = Some(Box::new(stream));
        self
    }

    /// Mock 모드 활성화/비활성화.
    ///
    /// Mock 모드가 활성화되면 실제 거래소 스트림 대신 Mock 스트림을 사용합니다.
    pub fn set_mock_mode(&mut self, enabled: bool) {
        self.mock_mode = enabled;
        info!("UnifiedMarketStream Mock 모드: {}", if enabled { "활성화" } else { "비활성화" });
    }

    /// Mock 모드 여부 확인.
    pub fn is_mock_mode(&self) -> bool {
        self.mock_mode
    }

    /// 심볼이 국내인지 해외인지 판단.
    fn is_korean_symbol(ticker: &str) -> bool {
        // 숫자로만 구성된 코드 = 국내 주식 (예: "005930", "005930/KRW")
        let code = ticker.split('/').next().unwrap_or(ticker);
        !code.is_empty() && code.chars().all(|c| c.is_ascii_digit())
    }
}

impl Default for UnifiedMarketStream {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MarketStream for UnifiedMarketStream {
    async fn start(&mut self) -> ExchangeResult<()> {
        if self.started {
            return Ok(());
        }

        // 모든 이벤트를 하나의 채널로 통합
        let (event_tx, event_rx) = mpsc::channel::<MarketEvent>(1024);

        if self.mock_mode {
            // Mock 모드: Mock 스트림만 bridge 태스크로 시작
            if let Some(mut mock) = self.mock_stream.take() {
                let tx = event_tx.clone();
                let (cmd_tx, mut cmd_rx) = mpsc::channel::<StreamCommand>(64);

                tokio::spawn(async move {
                    if let Err(e) = mock.start().await {
                        error!("Mock MarketStream 시작 실패: {}", e);
                        return;
                    }

                    loop {
                        tokio::select! {
                            event = mock.next_event() => {
                                match event {
                                    Some(ev) => {
                                        if tx.send(ev).await.is_err() {
                                            info!("Mock bridge: 이벤트 채널 종료");
                                            break;
                                        }
                                    }
                                    None => {
                                        info!("Mock bridge: 스트림 종료");
                                        break;
                                    }
                                }
                            }
                            Some(cmd) = cmd_rx.recv() => {
                                match cmd {
                                    StreamCommand::Subscribe { symbol } => {
                                        if let Err(e) = mock.subscribe_ticker(&symbol).await {
                                            warn!("Mock 동적 구독 실패 {}: {}", symbol, e);
                                        }
                                    }
                                    StreamCommand::SubscribeOrderBook { symbol } => {
                                        if let Err(e) = mock.subscribe_order_book(&symbol).await {
                                            warn!("Mock 동적 호가 구독 실패 {}: {}", symbol, e);
                                        }
                                    }
                                    StreamCommand::Unsubscribe { symbol } => {
                                        if let Err(e) = mock.unsubscribe(&symbol).await {
                                            warn!("Mock 동적 구독 해제 실패 {}: {}", symbol, e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                });

                self.mock_cmd_tx = Some(cmd_tx);
                info!("UnifiedMarketStream 시작됨 (Mock 모드, bridge task)");
            }
        } else {
            // 실거래 모드: KR/US 각각 독립적인 bridge 태스크로 시작

            // KR bridge 태스크
            if let Some(mut kr) = self.kr_stream.take() {
                let tx = event_tx.clone();
                let (cmd_tx, mut cmd_rx) = mpsc::channel::<StreamCommand>(64);

                tokio::spawn(async move {
                    if let Err(e) = kr.start().await {
                        error!("KR MarketStream 시작 실패: {}", e);
                        return;
                    }

                    loop {
                        tokio::select! {
                            event = kr.next_event() => {
                                match event {
                                    Some(ev) => {
                                        if tx.send(ev).await.is_err() {
                                            info!("KR bridge: 이벤트 채널 종료");
                                            break;
                                        }
                                    }
                                    None => {
                                        info!("KR bridge: 스트림 종료");
                                        break;
                                    }
                                }
                            }
                            Some(cmd) = cmd_rx.recv() => {
                                match cmd {
                                    StreamCommand::Subscribe { symbol } => {
                                        if let Err(e) = kr.subscribe_ticker(&symbol).await {
                                            warn!("KR 동적 구독 실패 {}: {}", symbol, e);
                                        }
                                    }
                                    StreamCommand::SubscribeOrderBook { symbol } => {
                                        if let Err(e) = kr.subscribe_order_book(&symbol).await {
                                            warn!("KR 동적 호가 구독 실패 {}: {}", symbol, e);
                                        }
                                    }
                                    StreamCommand::Unsubscribe { symbol } => {
                                        if let Err(e) = kr.unsubscribe(&symbol).await {
                                            warn!("KR 동적 구독 해제 실패 {}: {}", symbol, e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                });

                self.kr_cmd_tx = Some(cmd_tx);
            }

            // US bridge 태스크
            if let Some(mut us) = self.us_stream.take() {
                let tx = event_tx.clone();
                let (cmd_tx, mut cmd_rx) = mpsc::channel::<StreamCommand>(64);

                tokio::spawn(async move {
                    if let Err(e) = us.start().await {
                        error!("US MarketStream 시작 실패: {}", e);
                        return;
                    }

                    loop {
                        tokio::select! {
                            event = us.next_event() => {
                                match event {
                                    Some(ev) => {
                                        if tx.send(ev).await.is_err() {
                                            info!("US bridge: 이벤트 채널 종료");
                                            break;
                                        }
                                    }
                                    None => {
                                        info!("US bridge: 스트림 종료");
                                        break;
                                    }
                                }
                            }
                            Some(cmd) = cmd_rx.recv() => {
                                match cmd {
                                    StreamCommand::Subscribe { symbol } => {
                                        if let Err(e) = us.subscribe_ticker(&symbol).await {
                                            warn!("US 동적 구독 실패 {}: {}", symbol, e);
                                        }
                                    }
                                    StreamCommand::SubscribeOrderBook { symbol } => {
                                        if let Err(e) = us.subscribe_order_book(&symbol).await {
                                            warn!("US 동적 호가 구독 실패 {}: {}", symbol, e);
                                        }
                                    }
                                    StreamCommand::Unsubscribe { symbol } => {
                                        if let Err(e) = us.unsubscribe(&symbol).await {
                                            warn!("US 동적 구독 해제 실패 {}: {}", symbol, e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                });

                self.us_cmd_tx = Some(cmd_tx);
            }

            info!("UnifiedMarketStream 시작됨 (bridge tasks)");
        }

        self.event_rx = Some(event_rx);
        self.started = true;
        Ok(())
    }

    fn is_started(&self) -> bool {
        self.started
    }

    async fn subscribe_ticker(&mut self, symbol: &str) -> ExchangeResult<()> {
        if self.mock_mode {
            if self.started {
                // bridge 태스크에 명령 전송
                if let Some(ref cmd_tx) = self.mock_cmd_tx {
                    cmd_tx
                        .send(StreamCommand::Subscribe { symbol: symbol.to_string() })
                        .await
                        .map_err(|e| ExchangeError::NetworkError(format!("Mock 구독 명령 전송 실패: {}", e)))?;
                    return Ok(());
                }
            } else if let Some(ref mut mock) = self.mock_stream {
                return mock.subscribe_ticker(symbol).await;
            }
            return Err(ExchangeError::NotSupported("Mock 스트림이 설정되지 않았습니다".to_string()));
        }

        let cmd = StreamCommand::Subscribe { symbol: symbol.to_string() };

        if self.started {
            // bridge 태스크에 명령 전송
            if Self::is_korean_symbol(symbol) {
                if let Some(ref cmd_tx) = self.kr_cmd_tx {
                    cmd_tx.send(cmd).await.map_err(|e|
                        ExchangeError::NetworkError(format!("KR 구독 명령 전송 실패: {}", e)))?;
                    return Ok(());
                }
            } else if let Some(ref cmd_tx) = self.us_cmd_tx {
                cmd_tx.send(cmd).await.map_err(|e|
                    ExchangeError::NetworkError(format!("US 구독 명령 전송 실패: {}", e)))?;
                return Ok(());
            }
        } else {
            // start() 전: 직접 하위 스트림에 구독
            if Self::is_korean_symbol(symbol) {
                if let Some(ref mut kr) = self.kr_stream {
                    return kr.subscribe_ticker(symbol).await;
                }
            } else if let Some(ref mut us) = self.us_stream {
                return us.subscribe_ticker(symbol).await;
            }
        }
        Err(ExchangeError::NotSupported(format!(
            "No stream available for symbol: {}",
            symbol
        )))
    }

    async fn subscribe_kline(&mut self, symbol: &str, timeframe: Timeframe) -> ExchangeResult<()> {
        // KIS는 실시간 캔들스틱을 지원하지 않음
        if self.mock_mode {
            if let Some(ref mut mock) = self.mock_stream {
                return mock.subscribe_kline(symbol, timeframe).await;
            }
        }
        Err(ExchangeError::NotSupported(
            "KIS does not support real-time kline streaming".to_string(),
        ))
    }

    async fn subscribe_order_book(&mut self, symbol: &str) -> ExchangeResult<()> {
        if self.mock_mode {
            if self.started {
                if let Some(ref cmd_tx) = self.mock_cmd_tx {
                    cmd_tx
                        .send(StreamCommand::SubscribeOrderBook { symbol: symbol.to_string() })
                        .await
                        .map_err(|e| ExchangeError::NetworkError(format!("Mock 호가 구독 명령 전송 실패: {}", e)))?;
                    return Ok(());
                }
            } else if let Some(ref mut mock) = self.mock_stream {
                return mock.subscribe_order_book(symbol).await;
            }
            return Err(ExchangeError::NotSupported("Mock 스트림이 설정되지 않았습니다".to_string()));
        }

        let cmd = StreamCommand::SubscribeOrderBook { symbol: symbol.to_string() };

        if self.started {
            if Self::is_korean_symbol(symbol) {
                if let Some(ref cmd_tx) = self.kr_cmd_tx {
                    cmd_tx.send(cmd).await.map_err(|e|
                        ExchangeError::NetworkError(format!("KR 호가 구독 명령 전송 실패: {}", e)))?;
                    return Ok(());
                }
            } else if let Some(ref cmd_tx) = self.us_cmd_tx {
                cmd_tx.send(cmd).await.map_err(|e|
                    ExchangeError::NetworkError(format!("US 호가 구독 명령 전송 실패: {}", e)))?;
                return Ok(());
            }
        } else {
            if Self::is_korean_symbol(symbol) {
                if let Some(ref mut kr) = self.kr_stream {
                    return kr.subscribe_order_book(symbol).await;
                }
            } else if let Some(ref mut us) = self.us_stream {
                return us.subscribe_order_book(symbol).await;
            }
        }
        Err(ExchangeError::NotSupported(format!(
            "No stream available for symbol: {}",
            symbol
        )))
    }

    async fn subscribe_trades(&mut self, symbol: &str) -> ExchangeResult<()> {
        self.subscribe_ticker(symbol).await
    }

    async fn unsubscribe(&mut self, symbol: &str) -> ExchangeResult<()> {
        if self.mock_mode {
            if self.started {
                if let Some(ref cmd_tx) = self.mock_cmd_tx {
                    cmd_tx
                        .send(StreamCommand::Unsubscribe { symbol: symbol.to_string() })
                        .await
                        .map_err(|e| ExchangeError::NetworkError(format!("Mock 구독 해제 명령 전송 실패: {}", e)))?;
                    return Ok(());
                }
            } else if let Some(ref mut mock) = self.mock_stream {
                return mock.unsubscribe(symbol).await;
            }
            return Ok(());
        }

        let cmd = StreamCommand::Unsubscribe { symbol: symbol.to_string() };

        if self.started {
            if Self::is_korean_symbol(symbol) {
                if let Some(ref cmd_tx) = self.kr_cmd_tx {
                    cmd_tx.send(cmd).await.map_err(|e|
                        ExchangeError::NetworkError(format!("KR 구독 해제 명령 전송 실패: {}", e)))?;
                }
            } else if let Some(ref cmd_tx) = self.us_cmd_tx {
                cmd_tx.send(cmd).await.map_err(|e|
                    ExchangeError::NetworkError(format!("US 구독 해제 명령 전송 실패: {}", e)))?;
            }
        } else {
            if Self::is_korean_symbol(symbol) {
                if let Some(ref mut kr) = self.kr_stream {
                    return kr.unsubscribe(symbol).await;
                }
            } else if let Some(ref mut us) = self.us_stream {
                return us.unsubscribe(symbol).await;
            }
        }
        Ok(())
    }

    async fn next_event(&mut self) -> Option<MarketEvent> {
        // bridge 태스크에서 통합된 이벤트를 수신
        if let Some(ref mut rx) = self.event_rx {
            return rx.recv().await;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_korean_symbol_detection() {
        assert!(UnifiedMarketStream::is_korean_symbol("005930"));
        assert!(UnifiedMarketStream::is_korean_symbol("005930/KRW"));
        assert!(!UnifiedMarketStream::is_korean_symbol("AAPL"));
        assert!(!UnifiedMarketStream::is_korean_symbol("AAPL/USD"));
    }
}
