//! Binance 거래소 커넥터.
//!
//! Binance Spot용 REST API 및 WebSocket 연결 구현.
//! 메인넷과 테스트넷 모두 지원.

#![allow(dead_code)] // API 응답 필드 전체 매핑 (일부만 사용)

use crate::traits::{AccountInfo, Balance, ExchangeResult};
use crate::ExchangeError;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use sha2::Sha256;
use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};
use trader_core::{
    Kline, MarketType, OrderBook, OrderBookLevel, OrderRequest, OrderStatus, OrderType, Position,
    RoundMethod, Side, Symbol, TickSizeProvider, Ticker, Timeframe, TradeTick,
};

type HmacSha256 = Hmac<Sha256>;

// ============================================================================
// 설정
// ============================================================================

/// Binance 클라이언트 설정.
///
/// # 보안
/// - `Debug` 구현은 민감 정보(`api_key`, `api_secret`)를 마스킹합니다.
#[derive(Clone)]
pub struct BinanceConfig {
    /// API 키
    pub api_key: String,
    /// API 시크릿
    pub api_secret: String,
    /// 테스트넷 사용
    pub testnet: bool,
    /// 요청 타임아웃 (초)
    pub timeout_secs: u64,
    /// 수신 윈도우 (밀리초)
    pub recv_window: u64,
}

impl fmt::Debug for BinanceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let masked_key = if self.api_key.len() > 8 {
            format!(
                "{}...{}",
                &self.api_key[..4],
                &self.api_key[self.api_key.len() - 4..]
            )
        } else {
            "***REDACTED***".to_string()
        };

        f.debug_struct("BinanceConfig")
            .field("api_key", &masked_key)
            .field("api_secret", &"***REDACTED***")
            .field("testnet", &self.testnet)
            .field("timeout_secs", &self.timeout_secs)
            .field("recv_window", &self.recv_window)
            .finish()
    }
}

impl BinanceConfig {
    /// 새 설정 생성.
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self {
            api_key,
            api_secret,
            testnet: false,
            timeout_secs: 30,
            recv_window: 5000,
        }
    }

    /// 테스트넷 사용.
    pub fn with_testnet(mut self, testnet: bool) -> Self {
        self.testnet = testnet;
        self
    }

    /// 환경 변수에서 생성.
    pub fn from_env() -> Option<Self> {
        let testnet = std::env::var("BINANCE_TESTNET")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        let (api_key, api_secret) = if testnet {
            (
                std::env::var("BINANCE_TESTNET_API_KEY").ok()?,
                std::env::var("BINANCE_TESTNET_API_SECRET").ok()?,
            )
        } else {
            (
                std::env::var("BINANCE_API_KEY").ok()?,
                std::env::var("BINANCE_API_SECRET").ok()?,
            )
        };

        Some(Self {
            api_key,
            api_secret,
            testnet,
            timeout_secs: 30,
            recv_window: 5000,
        })
    }

    /// REST API 기본 URL 반환.
    pub fn rest_base_url(&self) -> &str {
        if self.testnet {
            "https://testnet.binance.vision"
        } else {
            "https://api.binance.com"
        }
    }

    /// WebSocket 기본 URL 반환.
    pub fn ws_base_url(&self) -> &str {
        if self.testnet {
            "wss://testnet.binance.vision/ws"
        } else {
            "wss://stream.binance.com:9443/ws"
        }
    }
}

// ============================================================================
// API 응답 타입
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct BinanceServerTime {
    server_time: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceAccountBalance {
    asset: String,
    free: String,
    locked: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceAccountInfo {
    balances: Vec<BinanceAccountBalance>,
    can_trade: bool,
    can_withdraw: bool,
    can_deposit: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct BinanceTicker {
    symbol: String,
    price_change: String,
    price_change_percent: String,
    last_price: String,
    bid_price: String,
    ask_price: String,
    open_price: String,
    high_price: String,
    low_price: String,
    volume: String,
    quote_volume: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct BinanceOrderBook {
    last_update_id: i64,
    bids: Vec<[String; 2]>,
    asks: Vec<[String; 2]>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceTrade {
    id: i64,
    price: String,
    qty: String,
    time: i64,
    is_buyer_maker: bool,
}

/// Binance 내 체결 내역 응답 타입 (`/api/v3/myTrades`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BinanceMyTrade {
    pub symbol: String,
    pub id: i64,
    pub order_id: i64,
    pub price: String,
    pub qty: String,
    pub quote_qty: String,
    pub commission: String,
    pub commission_asset: String,
    pub time: i64,
    pub is_buyer: bool,
    pub is_maker: bool,
}

#[derive(Debug, Deserialize)]
struct BinanceKline(
    i64,    // 0: Open time
    String, // 1: Open
    String, // 2: High
    String, // 3: Low
    String, // 4: Close
    String, // 5: Volume
    i64,    // 6: Close time
    String, // 7: Quote asset volume
    i64,    // 8: Number of trades
    String, // 9: Taker buy base asset volume
    String, // 10: Taker buy quote asset volume
    String, // 11: Ignore
);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceOrderResponse {
    symbol: String,
    order_id: i64,
    client_order_id: String,
    transact_time: Option<i64>,
    price: String,
    orig_qty: String,
    executed_qty: String,
    status: String,
    #[serde(rename = "type")]
    order_type: String,
    side: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceError {
    code: i32,
    msg: String,
}

// ============================================================================
// Binance 클라이언트
// ============================================================================

/// Binance 거래소 클라이언트.
pub struct BinanceClient {
    config: BinanceConfig,
    client: Client,
    connected: bool,
    /// 호가 단위 제공자 (옵션, 설정 시 주문 가격 자동 라운딩)
    tick_size_provider: Option<Arc<dyn TickSizeProvider>>,
}

impl BinanceClient {
    /// 새 Binance 클라이언트 생성.
    ///
    /// # Errors
    /// HTTP 클라이언트 생성에 실패하면 `ExchangeError::NetworkError`를 반환합니다.
    pub fn new(config: BinanceConfig) -> Result<Self, ExchangeError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| {
                ExchangeError::NetworkError(format!("HTTP 클라이언트 생성 실패: {}", e))
            })?;

        Ok(Self {
            config,
            client,
            connected: false,
            tick_size_provider: None,
        })
    }

    /// 호가 단위 제공자를 설정합니다.
    ///
    /// 설정 시 주문 가격이 자동으로 호가 단위로 라운딩됩니다.
    /// - 매수 주문: Floor (내림) - 보수적으로 더 낮은 가격
    /// - 매도 주문: Ceil (올림) - 보수적으로 더 높은 가격
    ///
    /// # Arguments
    /// * `provider` - 호가 단위 제공자 (예: `BinanceTickSize`)
    pub fn with_tick_size_provider(mut self, provider: Arc<dyn TickSizeProvider>) -> Self {
        self.tick_size_provider = Some(provider);
        self
    }

    /// 환경 변수에서 생성.
    ///
    /// 환경 변수가 설정되지 않았거나 클라이언트 생성에 실패하면 `None`을 반환합니다.
    pub fn from_env() -> Option<Self> {
        BinanceConfig::from_env().and_then(|config| Self::new(config).ok())
    }

    /// 현재 타임스탬프(밀리초) 반환.
    fn timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64
    }

    /// HMAC-SHA256으로 쿼리 문자열 서명.
    fn sign(&self, query: &str) -> String {
        let mut mac =
            HmacSha256::new_from_slice(self.config.api_secret.as_bytes()).expect("Invalid key");
        mac.update(query.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// 파라미터에서 쿼리 문자열 생성.
    fn build_query(params: &[(&str, String)]) -> String {
        params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// 공개 API 요청 (인증 불필요).
    async fn public_get<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
    ) -> ExchangeResult<T> {
        let url = format!("{}{}", self.config.rest_base_url(), endpoint);
        let query = Self::build_query(params);

        let full_url = if query.is_empty() {
            url
        } else {
            format!("{}?{}", url, query)
        };

        debug!("GET {}", full_url);

        let response = self
            .client
            .get(&full_url)
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        self.handle_response(response).await
    }

    /// 서명된 API 요청 (인증 필요).
    async fn signed_get<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
    ) -> ExchangeResult<T> {
        let url = format!("{}{}", self.config.rest_base_url(), endpoint);

        let mut all_params = params.to_vec();
        all_params.push(("timestamp", Self::timestamp_ms().to_string()));
        all_params.push(("recvWindow", self.config.recv_window.to_string()));

        let query = Self::build_query(&all_params);
        let signature = self.sign(&query);
        let full_url = format!("{}?{}&signature={}", url, query, signature);

        debug!("GET (signed) {}", endpoint);

        let response = self
            .client
            .get(&full_url)
            .header("X-MBX-APIKEY", &self.config.api_key)
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        self.handle_response(response).await
    }

    /// 서명된 POST 요청.
    async fn signed_post<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
    ) -> ExchangeResult<T> {
        let url = format!("{}{}", self.config.rest_base_url(), endpoint);

        let mut all_params = params.to_vec();
        all_params.push(("timestamp", Self::timestamp_ms().to_string()));
        all_params.push(("recvWindow", self.config.recv_window.to_string()));

        let query = Self::build_query(&all_params);
        let signature = self.sign(&query);
        let body = format!("{}&signature={}", query, signature);

        debug!("POST (signed) {}", endpoint);

        let response = self
            .client
            .post(&url)
            .header("X-MBX-APIKEY", &self.config.api_key)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        self.handle_response(response).await
    }

    /// 서명된 DELETE 요청.
    async fn signed_delete<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
    ) -> ExchangeResult<T> {
        let url = format!("{}{}", self.config.rest_base_url(), endpoint);

        let mut all_params = params.to_vec();
        all_params.push(("timestamp", Self::timestamp_ms().to_string()));
        all_params.push(("recvWindow", self.config.recv_window.to_string()));

        let query = Self::build_query(&all_params);
        let signature = self.sign(&query);
        let full_url = format!("{}?{}&signature={}", url, query, signature);

        debug!("DELETE (signed) {}", endpoint);

        let response = self
            .client
            .delete(&full_url)
            .header("X-MBX-APIKEY", &self.config.api_key)
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        self.handle_response(response).await
    }

    /// API 응답 처리.
    async fn handle_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
    ) -> ExchangeResult<T> {
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if status.is_success() {
            serde_json::from_str(&body).map_err(|e| {
                error!("Failed to parse response: {} - Body: {}", e, body);
                ExchangeError::ParseError(e.to_string())
            })
        } else {
            // 에러 응답 파싱 시도
            if let Ok(error) = serde_json::from_str::<BinanceError>(&body) {
                Err(self.map_error_code(error.code, &error.msg))
            } else {
                Err(ExchangeError::ApiError {
                    code: status.as_u16() as i32,
                    message: body,
                })
            }
        }
    }

    /// Binance 에러 코드를 ExchangeError로 매핑.
    fn map_error_code(&self, code: i32, msg: &str) -> ExchangeError {
        match code {
            -1000 => ExchangeError::Unknown(msg.to_string()),
            -1001 => ExchangeError::Disconnected(msg.to_string()),
            -1002 => ExchangeError::Unauthorized(msg.to_string()),
            -1003 => ExchangeError::RateLimited,
            -1013 => ExchangeError::InvalidQuantity(msg.to_string()),
            -1021 => ExchangeError::TimestampError(msg.to_string()),
            -2010 => ExchangeError::InsufficientBalance(msg.to_string()),
            -2011 => ExchangeError::OrderNotFound(msg.to_string()),
            -2013 => ExchangeError::OrderNotFound(msg.to_string()),
            _ => ExchangeError::ApiError {
                code,
                message: msg.to_string(),
            },
        }
    }

    /// Binance 심볼 형식을 내부 Symbol로 변환.
    fn to_symbol(binance_symbol: &str) -> Symbol {
        // 일반적인 호가 자산
        let quotes = ["USDT", "BUSD", "BTC", "ETH", "BNB", "USDC"];

        for quote in quotes {
            if let Some(base) = binance_symbol.strip_suffix(quote) {
                return Symbol::new(base, quote, MarketType::Crypto);
            }
        }

        // 폴백: USDT로 가정
        Symbol::new(binance_symbol, "USDT", MarketType::Crypto)
    }

    /// 내부 Symbol을 Binance 심볼 형식으로 변환.
    fn from_symbol(ticker: &str) -> String {
        // "BTC/USDT" -> "BTCUSDT"
        ticker.replace("/", "")
    }

    /// 문자열에서 Decimal 파싱.
    fn parse_decimal(s: &str) -> Decimal {
        s.parse().unwrap_or(Decimal::ZERO)
    }

    /// Binance 주문 상태를 내부 OrderStatus로 변환.
    fn parse_order_status(resp: &BinanceOrderResponse) -> OrderStatus {
        let status = match resp.status.as_str() {
            "NEW" => trader_core::OrderStatusType::Open,
            "PARTIALLY_FILLED" => trader_core::OrderStatusType::PartiallyFilled,
            "FILLED" => trader_core::OrderStatusType::Filled,
            "CANCELED" => trader_core::OrderStatusType::Cancelled,
            "REJECTED" => trader_core::OrderStatusType::Rejected,
            "EXPIRED" => trader_core::OrderStatusType::Expired,
            _ => trader_core::OrderStatusType::Open,
        };

        let side = match resp.side.as_str() {
            "BUY" => Some(trader_core::Side::Buy),
            "SELL" => Some(trader_core::Side::Sell),
            _ => None,
        };

        OrderStatus {
            order_id: resp.order_id.to_string(),
            client_order_id: Some(resp.client_order_id.clone()),
            ticker: Some(resp.symbol.clone()),
            side,
            quantity: Some(Self::parse_decimal(&resp.orig_qty)),
            price: Some(Self::parse_decimal(&resp.price)),
            status,
            filled_quantity: Self::parse_decimal(&resp.executed_qty),
            average_price: if Self::parse_decimal(&resp.executed_qty) > Decimal::ZERO {
                Some(Self::parse_decimal(&resp.price))
            } else {
                None
            },
            updated_at: Utc::now(),
        }
    }
}

/// Exchange 메서드 (이전 Exchange trait에서 전환).
impl BinanceClient {
    pub fn name(&self) -> &str {
        if self.config.testnet {
            "binance-testnet"
        } else {
            "binance"
        }
    }

    pub async fn is_connected(&self) -> bool {
        self.connected
    }

    pub async fn connect(&mut self) -> ExchangeResult<()> {
        info!(
            "Connecting to Binance {}...",
            if self.config.testnet {
                "testnet"
            } else {
                "mainnet"
            }
        );

        // 서버 시간 조회로 연결 테스트
        let _: BinanceServerTime = self.public_get("/api/v3/time", &[]).await?;

        self.connected = true;
        info!("Connected to Binance successfully");
        Ok(())
    }

    pub async fn disconnect(&mut self) -> ExchangeResult<()> {
        self.connected = false;
        info!("Disconnected from Binance");
        Ok(())
    }

    pub async fn get_account(&self) -> ExchangeResult<AccountInfo> {
        let resp: BinanceAccountInfo = self.signed_get("/api/v3/account", &[]).await?;

        let balances = resp
            .balances
            .into_iter()
            .filter(|b| {
                let free: Decimal = b.free.parse().unwrap_or(Decimal::ZERO);
                let locked: Decimal = b.locked.parse().unwrap_or(Decimal::ZERO);
                free > Decimal::ZERO || locked > Decimal::ZERO
            })
            .map(|b| Balance {
                asset: b.asset,
                free: b.free.parse().unwrap_or(Decimal::ZERO),
                locked: b.locked.parse().unwrap_or(Decimal::ZERO),
            })
            .collect();

        Ok(AccountInfo {
            balances,
            can_trade: resp.can_trade,
            can_withdraw: resp.can_withdraw,
            can_deposit: resp.can_deposit,
        })
    }

    pub async fn get_balance(&self, asset: &str) -> ExchangeResult<Balance> {
        let account = self.get_account().await?;

        account
            .balances
            .into_iter()
            .find(|b| b.asset.eq_ignore_ascii_case(asset))
            .ok_or_else(|| ExchangeError::AssetNotFound(asset.to_string()))
    }

    pub async fn get_ticker(&self, symbol: &str) -> ExchangeResult<Ticker> {
        let binance_symbol = Self::from_symbol(symbol);
        let resp: BinanceTicker = self
            .public_get("/api/v3/ticker/24hr", &[("symbol", binance_symbol)])
            .await?;

        Ok(Ticker {
            ticker: symbol.to_string(),
            bid: Self::parse_decimal(&resp.bid_price),
            ask: Self::parse_decimal(&resp.ask_price),
            last: Self::parse_decimal(&resp.last_price),
            volume_24h: Self::parse_decimal(&resp.volume),
            high_24h: Self::parse_decimal(&resp.high_price),
            low_24h: Self::parse_decimal(&resp.low_price),
            change_24h: Self::parse_decimal(&resp.price_change),
            change_24h_percent: Self::parse_decimal(&resp.price_change_percent),
            timestamp: Utc::now(),
        })
    }

    pub async fn get_order_book(&self, symbol: &str, limit: Option<u32>) -> ExchangeResult<OrderBook> {
        let binance_symbol = Self::from_symbol(symbol);
        let limit_str = limit.unwrap_or(100).to_string();

        let resp: BinanceOrderBook = self
            .public_get(
                "/api/v3/depth",
                &[("symbol", binance_symbol), ("limit", limit_str)],
            )
            .await?;

        let bids = resp
            .bids
            .into_iter()
            .map(|[price, qty]| OrderBookLevel {
                price: Self::parse_decimal(&price),
                quantity: Self::parse_decimal(&qty),
            })
            .collect();

        let asks = resp
            .asks
            .into_iter()
            .map(|[price, qty]| OrderBookLevel {
                price: Self::parse_decimal(&price),
                quantity: Self::parse_decimal(&qty),
            })
            .collect();

        Ok(OrderBook {
            ticker: symbol.to_string(),
            bids,
            asks,
            timestamp: Utc::now(),
        })
    }

    pub async fn get_recent_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> ExchangeResult<Vec<TradeTick>> {
        let binance_symbol = Self::from_symbol(symbol);
        let limit_str = limit.unwrap_or(500).to_string();

        let resp: Vec<BinanceTrade> = self
            .public_get(
                "/api/v3/trades",
                &[("symbol", binance_symbol), ("limit", limit_str)],
            )
            .await?;

        Ok(resp
            .into_iter()
            .map(|t| TradeTick {
                ticker: symbol.to_string(),
                id: t.id.to_string(),
                price: Self::parse_decimal(&t.price),
                quantity: Self::parse_decimal(&t.qty),
                side: if t.is_buyer_maker {
                    Side::Sell
                } else {
                    Side::Buy
                },
                timestamp: DateTime::from_timestamp_millis(t.time).unwrap_or_else(Utc::now),
            })
            .collect())
    }

    pub async fn get_klines(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: Option<u32>,
    ) -> ExchangeResult<Vec<Kline>> {
        let binance_symbol = Self::from_symbol(symbol);
        let interval = timeframe.to_binance_interval();
        let limit_str = limit.unwrap_or(500).to_string();

        let resp: Vec<BinanceKline> = self
            .public_get(
                "/api/v3/klines",
                &[
                    ("symbol", binance_symbol),
                    ("interval", interval.to_string()),
                    ("limit", limit_str),
                ],
            )
            .await?;

        Ok(resp
            .into_iter()
            .map(|k| Kline {
                ticker: symbol.to_string(),
                timeframe,
                open_time: DateTime::from_timestamp_millis(k.0).unwrap_or_else(Utc::now),
                open: Self::parse_decimal(&k.1),
                high: Self::parse_decimal(&k.2),
                low: Self::parse_decimal(&k.3),
                close: Self::parse_decimal(&k.4),
                volume: Self::parse_decimal(&k.5),
                close_time: DateTime::from_timestamp_millis(k.6).unwrap_or_else(Utc::now),
                quote_volume: Some(Self::parse_decimal(&k.7)),
                num_trades: Some(k.8 as u32),
            })
            .collect())
    }

    pub async fn place_order(&self, request: &OrderRequest) -> ExchangeResult<String> {
        // TODO: SymbolResolver로 ticker → Symbol 변환 후 from_symbol 사용
        let binance_symbol = request.ticker.clone();

        let side = match request.side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        };

        let order_type = match request.order_type {
            OrderType::Market => "MARKET",
            OrderType::Limit => "LIMIT",
            OrderType::StopLoss => "STOP_LOSS",
            OrderType::StopLossLimit => "STOP_LOSS_LIMIT",
            OrderType::TakeProfit => "TAKE_PROFIT",
            OrderType::TakeProfitLimit => "TAKE_PROFIT_LIMIT",
            OrderType::TrailingStop => "TRAILING_STOP_MARKET",
        };

        // 호가 단위 라운딩 헬퍼 (클로저)
        let round_price = |price: Decimal, is_buy: bool| -> Decimal {
            if let Some(ref provider) = self.tick_size_provider {
                // 매수: Floor (내림) - 보수적으로 더 낮은 가격
                // 매도: Ceil (올림) - 보수적으로 더 높은 가격
                let method = if is_buy {
                    RoundMethod::Floor
                } else {
                    RoundMethod::Ceil
                };
                let adjusted = provider.round_to_tick(price, method);
                if adjusted != price {
                    warn!(
                        "주문 가격 호가 단위 조정: {} -> {} (종목: {}, 방향: {})",
                        price,
                        adjusted,
                        request.ticker,
                        if is_buy { "매수" } else { "매도" }
                    );
                }
                adjusted
            } else {
                price
            }
        };

        let is_buy = matches!(request.side, Side::Buy);

        let mut params = vec![
            ("symbol", binance_symbol),
            ("side", side.to_string()),
            ("type", order_type.to_string()),
            ("quantity", request.quantity.to_string()),
        ];

        // 지정가 주문에 가격 추가 (라운딩 적용)
        if let Some(price) = request.price {
            let rounded_price = round_price(price, is_buy);
            params.push(("price", rounded_price.to_string()));
            params.push(("timeInForce", "GTC".to_string()));
        }

        // 스톱 가격이 있으면 추가 (라운딩 적용)
        if let Some(stop_price) = request.stop_price {
            let rounded_stop = round_price(stop_price, is_buy);
            params.push(("stopPrice", rounded_stop.to_string()));
        }

        // 클라이언트 주문 ID가 있으면 추가
        if let Some(ref client_id) = request.client_order_id {
            params.push(("newClientOrderId", client_id.clone()));
        }

        info!(
            "Placing {} {} order for {} {} @ {:?}",
            side, order_type, request.quantity, request.ticker, request.price
        );

        let resp: BinanceOrderResponse = self.signed_post("/api/v3/order", &params).await?;

        info!("Order placed successfully: {}", resp.order_id);
        Ok(resp.order_id.to_string())
    }

    pub async fn cancel_order(&self, symbol: &str, order_id: &str) -> ExchangeResult<()> {
        let binance_symbol = Self::from_symbol(symbol);

        let params = vec![
            ("symbol", binance_symbol),
            ("orderId", order_id.to_string()),
        ];

        let _: BinanceOrderResponse = self.signed_delete("/api/v3/order", &params).await?;

        info!("Order {} cancelled", order_id);
        Ok(())
    }

    pub async fn get_order(&self, symbol: &str, order_id: &str) -> ExchangeResult<OrderStatus> {
        let binance_symbol = Self::from_symbol(symbol);

        let params = vec![
            ("symbol", binance_symbol),
            ("orderId", order_id.to_string()),
        ];

        let resp: BinanceOrderResponse = self.signed_get("/api/v3/order", &params).await?;

        Ok(Self::parse_order_status(&resp))
    }

    pub async fn get_open_orders(&self, symbol: Option<&str>) -> ExchangeResult<Vec<OrderStatus>> {
        let params: Vec<(&str, String)> = if let Some(s) = symbol {
            vec![("symbol", Self::from_symbol(s))]
        } else {
            vec![]
        };

        let resp: Vec<BinanceOrderResponse> =
            self.signed_get("/api/v3/openOrders", &params).await?;

        Ok(resp.iter().map(Self::parse_order_status).collect())
    }

    pub async fn get_positions(&self) -> ExchangeResult<Vec<Position>> {
        // 현물은 포지션이 없음, 빈 값 반환
        Ok(vec![])
    }

    /// 특정 심볼의 체결 내역 조회 (`/api/v3/myTrades`).
    ///
    /// # Arguments
    ///
    /// * `symbol` - 조회할 심볼 (예: "BTC/USDT")
    /// * `start_time` - 시작 시간 (Unix ms, 옵션)
    /// * `end_time` - 종료 시간 (Unix ms, 옵션)
    /// * `limit` - 최대 조회 건수 (기본 500, 최대 1000)
    pub(crate) async fn get_my_trades(
        &self,
        symbol: &str,
        start_time: Option<u64>,
        end_time: Option<u64>,
        from_id: Option<u64>,
        limit: Option<u32>,
    ) -> ExchangeResult<Vec<BinanceMyTrade>> {
        let binance_symbol = Self::from_symbol(symbol);
        let mut params: Vec<(&str, String)> = vec![("symbol", binance_symbol)];

        if let Some(st) = start_time {
            params.push(("startTime", st.to_string()));
        }
        if let Some(et) = end_time {
            params.push(("endTime", et.to_string()));
        }
        if let Some(fid) = from_id {
            params.push(("fromId", fid.to_string()));
        }
        params.push(("limit", limit.unwrap_or(500).to_string()));

        self.signed_get("/api/v3/myTrades", &params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_conversion() {
        let ticker = "BTC/USDT";
        assert_eq!(BinanceClient::from_symbol(ticker), "BTCUSDT");

        let parsed = BinanceClient::to_symbol("ETHUSDT");
        assert_eq!(parsed.base, "ETH");
        assert_eq!(parsed.quote, "USDT");
    }

    #[test]
    fn test_sign() {
        let config = BinanceConfig::new(
            "vmPUZE6mv9SD5VNHk4HlWFsOr6aKE2zvsw0MuIgwCIPy6utIco14y7Ju91duEh8A".to_string(),
            "NhqPtmdSJYdKjVHjA7PZj4Mge3R5YNiP1e3UZjInClVN65XAbvqqM6A7H5fATj0j".to_string(),
        );
        let client = BinanceClient::new(config).expect("테스트용 클라이언트 생성 실패");

        // Test signature
        let query = "symbol=LTCBTC&side=BUY&type=LIMIT&timeInForce=GTC&quantity=1&price=0.1&recvWindow=5000&timestamp=1499827319559";
        let signature = client.sign(query);

        assert_eq!(
            signature,
            "c8db56825ae71d6d79447849e617115f4a920fa2acdcab2b053c4b2838bd6b71"
        );
    }
}
