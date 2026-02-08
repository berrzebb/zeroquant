use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use chrono::{Utc, DateTime};
use jsonwebtoken::{encode, EncodingKey, Header};
use reqwest::{Client, Method};
use rust_decimal::Decimal;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use trader_core::domain::{
    ExchangeProvider, MarketDataProvider, StrategyAccountInfo, PendingOrder, 
    StrategyPositionInfo, OrderStatusType, Side,
};
use trader_core::ProviderError;
use trader_core::QuoteData;

// ============================================================================
// 설정
// ============================================================================

#[derive(Clone)]
pub struct BithumbConfig {
    pub access_key: String,
    pub secret_key: String,
}

impl std::fmt::Debug for BithumbConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BithumbConfig")
            .field("access_key", &"***")
            .field("secret_key", &"***")
            .finish()
    }
}

impl BithumbConfig {
    pub fn new(access_key: String, secret_key: String) -> Self {
        Self {
            access_key,
            secret_key,
        }
    }

    pub fn from_env() -> Option<Self> {
        let access_key = std::env::var("BITHUMB_ACCESS_KEY").ok()?;
        let secret_key = std::env::var("BITHUMB_SECRET_KEY").ok()?;
        Some(Self::new(access_key, secret_key))
    }
}

// ============================================================================
// API 응답 타입
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct BithumbPayload {
    pub access_key: String,
    pub nonce: String,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_hash_alg: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BithumbBalance {
    pub currency: String,
    pub balance: String,
    pub locked: String,
    pub avg_buy_price: String,
    pub unit_currency: String,
}

#[derive(Debug, Deserialize)]
pub struct BithumbTicker {
    pub market: String,
    pub trade_date: String,
    pub trade_time: String,
    pub trade_date_kst: String,
    pub trade_time_kst: String,
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
pub struct BithumbOrder {
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

// ============================================================================
// Bithumb 클라이언트
// ============================================================================

pub struct BithumbClient {
    client: Client,
    config: BithumbConfig,
    base_url: String,
}

impl BithumbClient {
    pub fn new(config: BithumbConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            base_url: "https://api.bithumb.com/v1".to_string(),
        }
    }

    fn generate_token(&self, query_hash: Option<String>) -> Result<String, ProviderError> {
        let nonce = Uuid::new_v4().to_string();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let claims = BithumbPayload {
            access_key: self.config.access_key.clone(),
            nonce,
            timestamp,
            query_hash,
            query_hash_alg: Some("SHA512".to_string()),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.secret_key.as_bytes()),
        ).map_err(|e: jsonwebtoken::errors::Error| ProviderError::Authentication(e.to_string()))?;

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
        if let Some(q) = query {
            let query_string = serde_urlencoded::to_string(q).unwrap_or_default();
            if !query_string.is_empty() {
                use sha2::{Digest, Sha512};
                let mut hasher = Sha512::new();
                hasher.update(query_string.as_bytes());
                query_hash = Some(hex::encode(hasher.finalize()));
                builder = builder.query(q);
            }
        } else if let Some(b) = body {
             let query_string = serde_urlencoded::to_string(b).unwrap_or_default();
             use sha2::{Digest, Sha512};
             let mut hasher = Sha512::new();
             hasher.update(query_string.as_bytes());
             query_hash = Some(hex::encode(hasher.finalize()));
             
             builder = builder.json(b);
        }

        let token = self.generate_token(query_hash)?;
        builder = builder.header("Authorization", token);
        builder = builder.header("Content-Type", "application/json");

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
                "Bithumb API Error: {}",
                error_text
            )));
        }

        let text = response.text().await.map_err(|e| ProviderError::Network(e.to_string()))?;
        serde_json::from_str::<T>(&text).map_err(|e| {
            ProviderError::Parse(format!("Failed to parse Bithumb response: {}. Body: {}", e, text))
        })
    }
}

// ============================================================================
// ExchangeProvider 구현
// ============================================================================

#[async_trait]
impl ExchangeProvider for BithumbClient {
    fn exchange_name(&self) -> &str {
        "bithumb"
    }

    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
        let balances: Vec<BithumbBalance> = self
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
         let balances: Vec<BithumbBalance> = self
            .request(Method::GET, "/accounts", None, None)
            .await?;

        let mut positions = Vec::new();
        for b in balances {
            if b.currency == "KRW" { continue; } 
            
            let quantity = Decimal::from_str(&b.balance).unwrap_or_default() + Decimal::from_str(&b.locked).unwrap_or_default();
            let avg_price = Decimal::ZERO; 
            
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
        
        let orders: Vec<BithumbOrder> = self.request(Method::GET, "/orders", Some(&query), None).await?;
        
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
impl MarketDataProvider for BithumbClient {
    async fn get_quote(&self, symbol: &str) -> Result<QuoteData, ProviderError> {
        let query = serde_json::json!({
            "markets": symbol
        });

        let tickers: Vec<BithumbTicker> = self.request(Method::GET, "/ticker", Some(&query), None).await?;
        
        if let Some(t) = tickers.into_iter().next() {
            Ok(QuoteData {
                symbol: t.market,
                current_price: Decimal::from_f64_retain(t.trade_price).unwrap_or_default(),
                price_change: Decimal::from_f64_retain(t.signed_change_price).unwrap_or_default(),
                change_percent: Decimal::from_f64_retain(t.signed_change_rate * 100.0).unwrap_or_default(),
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

        match self.request::<Vec<BithumbTicker>>(Method::GET, "/ticker", Some(&query), None).await {
            Ok(tickers) => {
                tickers.into_iter().map(|t| {
                    QuoteData {
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
                    }
                }).collect()
            }
            Err(_) => Vec::new(),
        }
    }
    
    fn provider_name(&self) -> &str {
        "bithumb"
    }
}
