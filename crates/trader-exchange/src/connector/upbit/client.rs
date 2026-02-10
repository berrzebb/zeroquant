use async_trait::async_trait;
use chrono::{Utc, DateTime};
use jsonwebtoken::{encode, EncodingKey, Header};
use reqwest::{Client, Method};
use rust_decimal::Decimal;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use trader_core::domain::{
    ExchangeProvider, MarketDataProvider, StrategyAccountInfo,
    StrategyPositionInfo, PendingOrder, OrderStatusType, Side,
};
use trader_core::ProviderError;
use trader_core::QuoteData;

// ============================================================================
// 설정
// ============================================================================

#[derive(Clone)]
pub struct UpbitConfig {
    pub access_key: String,
    pub secret_key: String,
}

impl std::fmt::Debug for UpbitConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpbitConfig")
            .field("access_key", &"***")
            .field("secret_key", &"***")
            .finish()
    }
}

impl UpbitConfig {
    pub fn new(access_key: String, secret_key: String) -> Self {
        Self {
            access_key,
            secret_key,
        }
    }


}

// ============================================================================
// API 응답 타입
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct UpbitPayload {
    pub access_key: String,
    pub nonce: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_hash_alg: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpbitBalance {
    pub currency: String,
    pub balance: String,
    pub locked: String,
    pub avg_buy_price: String,
    pub avg_buy_price_modified: bool,
    pub unit_currency: String,
}

#[derive(Debug, Deserialize)]
pub struct UpbitTicker {
    pub market: String,
    pub trade_date: String,
    pub trade_time: String,
    pub trade_date_kst: String,
    pub trade_time_kst: String,
    pub trade_timestamp: i64,
    pub opening_price: f64,
    pub high_price: f64,
    pub low_price: f64,
    pub trade_price: f64,
    pub prev_closing_price: f64,
    pub change: String,
    pub change_price: f64,
    pub change_rate: f64,
    pub signed_change_price: f64,
    pub signed_change_rate: f64,
    pub trade_volume: f64,
    pub acc_trade_price: f64,
    pub acc_trade_price_24h: f64,
    pub acc_trade_volume: f64,
    pub acc_trade_volume_24h: f64,
    pub highest_52_week_price: f64,
    pub highest_52_week_date: String,
    pub lowest_52_week_price: f64,
    pub lowest_52_week_date: String,
    pub timestamp: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpbitOrder {
    pub uuid: String,
    pub side: String,
    pub ord_type: String,
    pub price: Option<String>,
    pub state: String,
    pub market: String,
    pub created_at: String,
    pub volume: Option<String>,
    pub remaining_volume: Option<String>,
    pub reserved_fee: Option<String>,
    pub remaining_fee: Option<String>,
    pub paid_fee: Option<String>,
    pub locked: Option<String>,
    pub executed_volume: Option<String>,
    pub trades_count: Option<u64>,
}

/// Upbit 체결 완료 주문 상세 (개별 조회 시 trades 배열 포함).
#[derive(Debug, Deserialize)]
pub struct UpbitOrderDetail {
    pub uuid: String,
    pub side: String,
    pub ord_type: String,
    pub price: Option<String>,
    pub state: String,
    pub market: String,
    pub created_at: String,
    pub volume: Option<String>,
    pub remaining_volume: Option<String>,
    pub executed_volume: Option<String>,
    pub paid_fee: Option<String>,
    pub trades_count: Option<u64>,
    pub trades: Option<Vec<UpbitTrade>>,
}

/// Upbit 개별 체결 내역.
#[derive(Debug, Deserialize, Clone)]
pub struct UpbitTrade {
    pub uuid: String,
    pub price: String,
    pub volume: String,
    pub funds: String,
    pub created_at: String,
    pub side: String,
}

// ============================================================================
// Upbit 클라이언트
// ============================================================================

pub struct UpbitClient {
    client: Client,
    config: UpbitConfig,
    base_url: String,
}

impl UpbitClient {
    pub fn new(config: UpbitConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            base_url: "https://api.upbit.com/v1".to_string(),
        }
    }

    fn generate_token(&self, query_hash: Option<String>) -> Result<String, ProviderError> {
        let nonce = Uuid::new_v4().to_string();
        let payload = UpbitPayload {
            access_key: self.config.access_key.clone(),
            nonce,
            query_hash,
            query_hash_alg: Some("SHA512".to_string()),
        };

        let token = encode(
            &Header::default(),
            &payload,
            &EncodingKey::from_secret(self.config.secret_key.as_bytes()),
        )
        .map_err(|e| ProviderError::Authentication(e.to_string()))?;

        Ok(format!("Bearer {}", token))
    }

    async fn request<T: for<'de> Deserialize<'de>>(
        &self,
        method: Method,
        endpoint: &str,
        query: Option<&serde_json::Value>,
        body: Option<&serde_json::Value>,
    ) -> Result<T, ProviderError> {
        let url = format!("{}{}", self.base_url, endpoint);
        let mut builder = self.client.request(method.clone(), &url);

        let mut query_hash = None;

        // GET: 쿼리 파라미터 해싱
        if let Some(q) = query {
            let query_string = serde_urlencoded::to_string(q).unwrap_or_default();
            if !query_string.is_empty() {
                use sha2::{Digest, Sha512};
                let mut hasher = Sha512::new();
                hasher.update(query_string.as_bytes());
                query_hash = Some(hex::encode(hasher.finalize()));
                builder = builder.query(q);
            }
        }

        // POST/DELETE: body를 query string으로 변환 후 해싱 (Upbit JWT 인증 규격)
        if let Some(b) = body {
            let body_query_string = serde_urlencoded::to_string(b).unwrap_or_default();
            if !body_query_string.is_empty() {
                use sha2::{Digest, Sha512};
                let mut hasher = Sha512::new();
                hasher.update(body_query_string.as_bytes());
                query_hash = Some(hex::encode(hasher.finalize()));
            }
            builder = builder.json(b);
        }

        let token = self.generate_token(query_hash)?;
        builder = builder.header("Authorization", token);

        let response = builder
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::Api(format!(
                "Upbit API Error: {}",
                error_text
            )));
        }

        response
            .json::<T>()
            .await
            .map_err(|e| ProviderError::Parse(e.to_string()))
    }
}

// ============================================================================
// 주문 실행 메서드
// ============================================================================

impl UpbitClient {
    /// 주문 생성 (POST /v1/orders)
    pub async fn place_order(
        &self,
        market: &str,
        side: &str,
        ord_type: &str,
        volume: Option<&str>,
        price: Option<&str>,
    ) -> Result<UpbitOrder, ProviderError> {
        let mut body = serde_json::json!({
            "market": market,
            "side": side,
            "ord_type": ord_type,
        });

        // 지정가/시장가 매도: volume 필수
        if let Some(v) = volume {
            body["volume"] = serde_json::Value::String(v.to_string());
        }
        // 지정가/시장가 매수(price): price 필수
        if let Some(p) = price {
            body["price"] = serde_json::Value::String(p.to_string());
        }

        self.request(Method::POST, "/orders", None, Some(&body)).await
    }

    /// 주문 취소 (DELETE /v1/order)
    pub async fn cancel_order(&self, uuid: &str) -> Result<UpbitOrder, ProviderError> {
        let query = serde_json::json!({
            "uuid": uuid,
        });
        self.request(Method::DELETE, "/order", Some(&query), None).await
    }

    /// 주문 조회 (GET /v1/order)
    pub async fn get_order(&self, uuid: &str) -> Result<UpbitOrder, ProviderError> {
        let query = serde_json::json!({
            "uuid": uuid,
        });
        self.request(Method::GET, "/order", Some(&query), None).await
    }

    /// 체결 내역 조회 (완료된 주문 목록).
    ///
    /// `/v1/orders/closed?state=done` 엔드포인트를 호출하여 체결 완료된 주문 목록을 조회합니다.
    ///
    /// # Arguments
    /// * `start_date` - 조회 시작 날짜 (YYYYMMDD)
    /// * `end_date` - 조회 종료 날짜 (YYYYMMDD)
    /// * `limit` - 조회 개수 (최대 1000, 기본 100)
    ///
    /// # Returns
    /// 완료된 주문 목록 (각 주문은 하나의 Trade로 변환됨)
    pub async fn fetch_execution_history(
        &self,
        start_date: &str,
        end_date: &str,
        limit: usize,
    ) -> Result<Vec<UpbitOrderDetail>, ProviderError> {
        // YYYYMMDD → ISO 8601 변환
        let start_time = format!("{}T00:00:00Z",
            &format!("{}-{}-{}", &start_date[0..4], &start_date[4..6], &start_date[6..8]));
        let end_time = format!("{}T23:59:59Z",
            &format!("{}-{}-{}", &end_date[0..4], &end_date[4..6], &end_date[6..8]));

        let query = serde_json::json!({
            "state": "done",
            "start_time": start_time,
            "end_time": end_time,
            "limit": limit.min(1000),
            "order_by": "desc",
        });

        // 완료된 주문 목록 조회
        let orders: Vec<UpbitOrder> = self
            .request(Method::GET, "/orders/closed", Some(&query), None)
            .await?;

        // 각 주문을 UpbitOrderDetail로 변환 (간단 버전: trades 배열 없이)
        let details: Vec<UpbitOrderDetail> = orders.into_iter().map(|o| {
            UpbitOrderDetail {
                uuid: o.uuid,
                side: o.side,
                ord_type: o.ord_type,
                price: o.price,
                state: o.state,
                market: o.market,
                created_at: o.created_at,
                volume: o.volume,
                remaining_volume: o.remaining_volume,
                executed_volume: o.executed_volume,
                paid_fee: o.paid_fee,
                trades_count: o.trades_count,
                trades: None, // 주문 수준 조회에서는 trades 없음
            }
        }).collect();

        Ok(details)
    }
}

// ============================================================================
// ExchangeProvider 구현
// ============================================================================

#[async_trait]
impl ExchangeProvider for UpbitClient {
    fn exchange_name(&self) -> &str {
        "upbit"
    }

    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
        let balances: Vec<UpbitBalance> = self
            .request(Method::GET, "/accounts", None, None)
            .await?;

        let mut total_balance = Decimal::ZERO;
        let mut available_balance = Decimal::ZERO;

        for b in &balances {
            if b.currency == "KRW" {
                if let Ok(val) = Decimal::from_str(&b.balance) {
                    available_balance += val;
                    total_balance += val;
                }
                if let Ok(locked) = Decimal::from_str(&b.locked) {
                    total_balance += locked;
                }
            }
        }

        Ok(StrategyAccountInfo {
            total_balance,
            available_balance,
            margin_used: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            currency: "KRW".to_string(),
        })
    }

    async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        let balances: Vec<UpbitBalance> = self
            .request(Method::GET, "/accounts", None, None)
            .await?;

        let mut positions = Vec::new();
        for b in balances {
            if b.currency == "KRW" { continue; }
            
            let quantity = Decimal::from_str(&b.balance).unwrap_or_default() + Decimal::from_str(&b.locked).unwrap_or_default();
            let avg_price = Decimal::from_str(&b.avg_buy_price).unwrap_or_default();
            
            if quantity > Decimal::ZERO {
                let ticker = format!("KRW-{}", b.currency);
                positions.push(StrategyPositionInfo::new(
                    ticker,
                    Side::Buy,
                    quantity,
                    avg_price
                ));
            }
        }
        Ok(positions)
    }

    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError> {
        let query = serde_json::json!({
            "state": "wait",
        });
        
        let orders: Vec<UpbitOrder> = self.request(Method::GET, "/orders", Some(&query), None).await?;
        
        let mut open_orders = Vec::new();
        for order in orders {
             let side = if order.side == "bid" { Side::Buy } else { Side::Sell };
             let price = order.price.and_then(|p| Decimal::from_str(&p).ok()).unwrap_or_default();
             let quantity = order.remaining_volume.and_then(|v| Decimal::from_str(&v).ok()).unwrap_or_default(); 

             open_orders.push(PendingOrder {
                 order_id: order.uuid,
                 ticker: order.market,
                 side,
                 price,
                 quantity,
                 filled_quantity: Decimal::ZERO,
                 status: OrderStatusType::Pending,
                 created_at: DateTime::parse_from_rfc3339(&format!("{}+09:00", order.created_at)).unwrap_or_default().with_timezone(&Utc),
             });
        }
        
        Ok(open_orders)
    }
}

#[async_trait]
impl MarketDataProvider for UpbitClient {
    async fn get_quote(&self, symbol: &str) -> Result<QuoteData, ProviderError> {
        let query = serde_json::json!({
            "markets": symbol
        });

        let tickers: Vec<UpbitTicker> = self.request(Method::GET, "/ticker", Some(&query), None).await?;
        
        if let Some(t) = tickers.into_iter().next() {
            Ok(QuoteData {
                symbol: t.market,
                current_price: Decimal::from_f64_retain(t.trade_price).unwrap_or_default(),
                price_change: Decimal::from_f64_retain(t.change_price).unwrap_or_default(),
                change_percent: Decimal::from_f64_retain(t.change_rate).unwrap_or_default(),
                high: Decimal::from_f64_retain(t.high_price).unwrap_or_default(),
                low: Decimal::from_f64_retain(t.low_price).unwrap_or_default(),
                open: Decimal::from_f64_retain(t.opening_price).unwrap_or_default(),
                prev_close: Decimal::from_f64_retain(t.prev_closing_price).unwrap_or_default(),
                volume: Decimal::from_f64_retain(t.acc_trade_volume_24h).unwrap_or_default(),
                trading_value: Decimal::from_f64_retain(t.acc_trade_price_24h).unwrap_or_default(),
                timestamp: Utc::now(),
            })
        } else {
             Err(ProviderError::Api("Quote not found".to_string()))
        }
    }

    async fn get_quotes(&self, symbols: &[String]) -> Vec<QuoteData> {
        if symbols.is_empty() {
            return Vec::new();
        }
        
        let markets = symbols.join(",");
        let query = serde_json::json!({
            "markets": markets
        });

        match self.request::<Vec<UpbitTicker>>(Method::GET, "/ticker", Some(&query), None).await {
            Ok(tickers) => {
                tickers.into_iter().map(|t| {
                    QuoteData {
                         symbol: t.market,
                         current_price: Decimal::from_f64_retain(t.trade_price).unwrap_or_default(),
                         price_change: Decimal::from_f64_retain(t.change_price).unwrap_or_default(),
                         change_percent: Decimal::from_f64_retain(t.change_rate * 100.0).unwrap_or_default(),
                         high: Decimal::from_f64_retain(t.high_price).unwrap_or_default(),
                         low: Decimal::from_f64_retain(t.low_price).unwrap_or_default(),
                         open: Decimal::from_f64_retain(t.opening_price).unwrap_or_default(),
                         prev_close: Decimal::from_f64_retain(t.prev_closing_price).unwrap_or_default(),
                         volume: Decimal::from_f64_retain(t.acc_trade_volume_24h).unwrap_or_default(),
                         trading_value: Decimal::from_f64_retain(t.acc_trade_price_24h).unwrap_or_default(),
                         timestamp: Utc::now(),
                    }
                }).collect()
            }
            Err(_) => Vec::new(),
        }
    }
    
    fn provider_name(&self) -> &str {
        "upbit"
    }
}
