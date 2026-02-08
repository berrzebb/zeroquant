use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde_json::{json, Value};
use serde::Deserialize;
use tokio::sync::Mutex;
use rust_decimal::Decimal;
use std::str::FromStr;

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
pub struct LsSecConfig {
    pub app_key: String,
    pub app_secret: String,
    pub base_url: String, 
}

impl std::fmt::Debug for LsSecConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LsSecConfig")
            .field("app_key", &"***")
            .field("app_secret", &"***")
            .field("base_url", &self.base_url)
            .finish()
    }
}

// ============================================================================
// 토큰 관리
// ============================================================================

struct TokenManager {
    access_token: Option<String>,
    expires_at: Instant,
}

// ============================================================================
// LS Securities 클라이언트
// ============================================================================

pub struct LsSecClient {
    client: Client,
    config: LsSecConfig,
    token_manager: Arc<Mutex<TokenManager>>,
}

#[derive(Deserialize, Debug)]
struct BalanceResponse {
    #[serde(rename = "CSPAQ12200OutBlock2")]
    out_block: BalanceOutBlock,
}

#[derive(Deserialize, Debug)]
struct BalanceOutBlock {
    #[serde(rename = "BalEvalAmt")]
    _bal_eval_amt: Value,
    #[serde(rename = "MnyOrdAbleAmt")]
    mny_ord_able_amt: Value,
    #[serde(rename = "DpsastTotamt")]
    dpsast_totamt: Value,
}

#[derive(Deserialize, Debug)]
struct KrPositionsResponse {
    #[serde(rename = "CSPAQ12300OutBlock2")]
    out_block2: Vec<KrPosition>,
}

#[derive(Deserialize, Debug)]
struct KrPosition {
    #[serde(rename = "IsuNo")]
    isu_no: String,
    #[serde(rename = "BalQty")]
    bal_qty: String,
    #[serde(rename = "PchsAvgPrc")]
    pchs_avg_prc: String,
}

#[derive(Deserialize, Debug)]
struct UsPositionsResponse {
    #[serde(rename = "COSOQ00201OutBlock2")]
    out_block2: Vec<UsPosition>,
}

#[derive(Deserialize, Debug)]
struct UsPosition {
    #[serde(rename = "SymCode")]
    sym_code: String,
    #[serde(rename = "ExecQty")]
    exec_qty: String,
    #[serde(rename = "AvgPchsPrcUsd")]
    avg_pchs_prc_usd: String,
}

#[derive(Deserialize, Debug)]
struct PendingOrderResponse {
    #[serde(rename = "t0425OutBlock1")]
    out_block1: Vec<PendingOrderBlock>,
}

#[derive(Deserialize, Debug)]
struct PendingOrderBlock {
    #[serde(rename = "ordno")]
    ordno: Value,
    #[serde(rename = "expcode")]
    expcode: String,
    #[serde(rename = "medosu")]
    medosu: String,
    #[serde(rename = "qty")]
    qty: Value,
    #[serde(rename = "price")]
    price: Value,
    #[serde(rename = "ordrem")]
    ordrem: Value,
    #[serde(rename = "status")]
    _status: String,
}

#[derive(Deserialize, Debug)]
struct QuoteResponse {
    #[serde(rename = "t1101OutBlock")]
    out_block: QuoteOutBlock,
}

#[derive(Deserialize, Debug)]
struct QuoteOutBlock {
    #[serde(rename = "shcode")]
    _shcode: String,
    #[serde(rename = "price")]
    price: Value,
    #[serde(rename = "change")]
    change: Value,
    #[serde(rename = "diff")]
    diff: Value,
}

impl LsSecClient {
    pub fn new(app_key: String, app_secret: String) -> Self {
        let base_url = std::env::var("LS_BASE_URL")
            .unwrap_or_else(|_| "https://openapi.ls-sec.co.kr:8080".to_string());
            
        Self {
            client: Client::new(),
            config: LsSecConfig {
                app_key,
                app_secret,
                base_url,
            },
            token_manager: Arc::new(Mutex::new(TokenManager {
                access_token: None,
                expires_at: Instant::now(),
            })),
        }
    }

    async fn get_token(&self) -> Result<String, ProviderError> {
        let mut tm = self.token_manager.lock().await;

        if let Some(token) = &tm.access_token {
            if Instant::now() < tm.expires_at {
                return Ok(token.clone());
            }
        }

        let token_file_path = "ls_token.json";
        if let Ok(file) = std::fs::File::open(token_file_path) {
             if let Ok(saved_token) = serde_json::from_reader::<_, Value>(file) {
                 if let Some(token) = saved_token["access_token"].as_str() {
                     if let Ok(metadata) = std::fs::metadata(token_file_path) {
                         if let Ok(modified) = metadata.modified() {
                             if let Ok(duration) = SystemTime::now().duration_since(modified) {
                                 if duration.as_secs() < 6 * 3600 {
                                     tm.access_token = Some(token.to_string());
                                     tm.expires_at = Instant::now() + Duration::from_secs(3600); 
                                     return Ok(token.to_string());
                                 }
                             }
                         }
                     }
                 }
            }
        }

        let url = format!("{}/oauth2/token", self.config.base_url);
        let params = [
            ("grant_type", "client_credentials"),
            ("appkey", &self.config.app_key),
            ("appsecretkey", &self.config.app_secret),
            ("scope", "oob"),
        ];

        let response = self
            .client
            .post(&url)
            .form(&params)
            .send()
            .await
            .map_err(|e| ProviderError::Authentication(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::Authentication(format!(
                "Failed to get token: {} - {}",
                status, text
            )));
        }

        let body_text = response.text().await.map_err(|e| ProviderError::Parse(e.to_string()))?;
        let body: Value = serde_json::from_str(&body_text).map_err(|e| ProviderError::Parse(e.to_string()))?;
        
        if let Some(token) = body["access_token"].as_str() {
            let expires_in = body["expires_in"].as_u64().unwrap_or(3600);
            tm.expires_at = Instant::now() + Duration::from_secs(expires_in.saturating_sub(10));
            tm.access_token = Some(token.to_string());
            
            let save_data = json!({
                "access_token": token,
                "expires_in": expires_in,
                "timestamp": Utc::now().to_rfc3339()
            });
            if let Ok(file) = std::fs::File::create(token_file_path) {
                let _ = serde_json::to_writer(file, &save_data);
            }
            
            Ok(token.to_string())
        } else {
            Err(ProviderError::Authentication(
                "No access_token in LS response".to_string(),
            ))
        }
    }

    async fn request<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        tr_cd: &str,
        body: Option<Value>,
    ) -> Result<T, ProviderError> {
        let token = self.get_token().await?;
        let url = format!("{}/{}", self.config.base_url, path);

        let mut builder = self.client.post(&url);
        builder = builder.header("Authorization", format!("Bearer {}", token));
        builder = builder.header("Content-Type", "application/json; charset=UTF-8");
        builder = builder.header("tr_cd", tr_cd);
        builder = builder.header("tr_cont", "N");
        builder = builder.header("tr_cont_key", "");

        if let Some(b) = body {
            builder = builder.json(&b);
        }

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
                "LS Securities API Error: {}",
                error_text
            )));
        }

        let text = response.text().await.map_err(|e| ProviderError::Network(e.to_string()))?;
        serde_json::from_str::<T>(&text).map_err(|e| {
            ProviderError::Parse(format!("Failed to parse LS response: {}. Body: {}", e, text))
        })
    }

    #[allow(dead_code)]
    fn is_us_symbol(&self, symbol: &str) -> bool {
        symbol.chars().any(|c| c.is_alphabetic())
    }

    async fn fetch_kr_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        // CSPAQ12300: KR Stock Balance/Positions
        let body = json!({
            "CSPAQ12300InBlock1": {
                "BalCreTp": "0"
            }
        });
        let res: KrPositionsResponse = self.request("stock/accno", "CSPAQ12300", Some(body)).await?;
        
        let mut positions = Vec::new();
        for stock in res.out_block2 {
            let qty = Decimal::from_str(&stock.bal_qty).unwrap_or_default();
            if qty.is_zero() { continue; }

            positions.push(StrategyPositionInfo::new(
                stock.isu_no,
                Side::Buy,
                qty,
                Decimal::from_str(&stock.pchs_avg_prc).unwrap_or_default(),
            ));
        }
        Ok(positions)
    }

    async fn fetch_us_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        // COSOQ00201: US Stock Balance/Positions
        let body = json!({
            "COSOQ00201InBlock1": {
                "D2_KRW_Base_Tp": "0"
            }
        });
        let res: UsPositionsResponse = self.request("overseas-stock/accno", "COSOQ00201", Some(body)).await?;
        
        let mut positions = Vec::new();
        for stock in res.out_block2 {
            let qty = Decimal::from_str(&stock.exec_qty).unwrap_or_default();
            if qty.is_zero() { continue; }

            positions.push(StrategyPositionInfo::new(
                stock.sym_code,
                Side::Buy,
                qty,
                Decimal::from_str(&stock.avg_pchs_prc_usd).unwrap_or_default(),
            ));
        }
        Ok(positions)
    }
}

// ============================================================================
// ExchangeProvider 구현
// ============================================================================

#[async_trait]
impl ExchangeProvider for LsSecClient {
    fn exchange_name(&self) -> &str {
        "ls_securities"
    }

    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
        let mut total_balance = Decimal::ZERO;
        let mut available_balance = Decimal::ZERO;

        fn parse_value(v: &Value) -> Decimal {
            if let Some(s) = v.as_str() {
                Decimal::from_str(s).unwrap_or_default()
            } else if let Some(n) = v.as_f64() {
                Decimal::from_f64_retain(n).unwrap_or_default()
            } else if let Some(n) = v.as_i64() {
                Decimal::from(n)
            } else {
                Decimal::ZERO
            }
        }

        // KR Balance (CSPAQ12200)
        let kr_body = json!({ "CSPAQ12200InBlock1": { "BalCreTp": "0" } });
        if let Ok(res) = self.request::<BalanceResponse>("stock/accno", "CSPAQ12200", Some(kr_body)).await {
            total_balance += parse_value(&res.out_block.dpsast_totamt);
            available_balance += parse_value(&res.out_block.mny_ord_able_amt);
        }

        // US Balance (COSOQ02701)
        let _us_body = json!({ "COSOQ02701InBlock1": { "D2_KRW_Base_Tp": "0" } });
        // (Assuming US balance response structure is similar or check it)
        // For simplicity, we just implement KR for now if US structure is too different without exact specs.
        // But let's try to add a placeholder or implementation if we have it in Python.

        Ok(StrategyAccountInfo {
            total_balance,
            available_balance,
            margin_used: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            currency: "KRW".to_string(),
        })
    }

    async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        let mut all_positions = Vec::new();
        
        match self.fetch_kr_positions().await {
            Ok(pos) => all_positions.extend(pos),
            Err(e) => eprintln!("Failed to fetch LS KR positions: {:?}", e),
        }

        match self.fetch_us_positions().await {
            Ok(pos) => all_positions.extend(pos),
            Err(e) => eprintln!("Failed to fetch LS US positions: {:?}", e),
        }

        Ok(all_positions)
    }

    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError> {
        // t0425: KR Stock Unexecuted Orders
        let body = json!({
            "t0425InBlock": {
                "expcode": "",
                "chegb": "2", // 2=미체결 (Unexecuted)
                "medosu": "0", // 0=전체
                "sortgb": "1",
                "cts_ordno": ""
            }
        });

        let res: PendingOrderResponse = self.request("stock/accno", "t0425", Some(body)).await?;
        
        let mut pending = Vec::new();
            for order in res.out_block1 {
            let mut ticker = order.expcode;
            if ticker.len() == 7 && ticker.starts_with('A') {
                ticker = ticker[1..].to_string();
            }

            let side = if order.medosu.contains("매도") || order.medosu == "1" {
                Side::Sell
            } else {
                Side::Buy
            };

            fn parse_num(v: &Value) -> Decimal {
                if let Some(s) = v.as_str() { Decimal::from_str(s).unwrap_or_default() }
                else if let Some(n) = v.as_f64() { Decimal::from_f64_retain(n).unwrap_or_default() }
                else if let Some(n) = v.as_i64() { Decimal::from(n) }
                else { Decimal::ZERO }
            }

            let order_id = if let Some(s) = order.ordno.as_str() { s.to_string() } else { order.ordno.to_string() };

            pending.push(PendingOrder {
                order_id,
                ticker,
                side,
                price: parse_num(&order.price),
                quantity: parse_num(&order.qty),
                filled_quantity: parse_num(&order.qty) - parse_num(&order.ordrem),
                status: OrderStatusType::Open,
                created_at: Utc::now(),
            });
        }

        // TODO: US Pending Orders (COSAT00301 or similar)
        
        Ok(pending)
    }
}

#[async_trait]
impl MarketDataProvider for LsSecClient {
    async fn get_quote(&self, symbol: &str) -> Result<QuoteData, ProviderError> {
        let body = json!({
            "t1101InBlock": {
                "shcode": symbol
            }
        });
        let res: QuoteResponse = self.request("stock/market-data", "t1101", Some(body)).await?;
        
        fn parse_num(v: &Value) -> Decimal {
            if let Some(s) = v.as_str() { Decimal::from_str(s).unwrap_or_default() }
            else if let Some(n) = v.as_f64() { Decimal::from_f64_retain(n).unwrap_or_default() }
            else if let Some(n) = v.as_i64() { Decimal::from(n) }
            else { Decimal::ZERO }
        }

        Ok(QuoteData {
            symbol: symbol.to_string(),
            current_price: parse_num(&res.out_block.price),
            price_change: parse_num(&res.out_block.change),
            change_percent: parse_num(&res.out_block.diff),
            high: Decimal::ZERO,
            low: Decimal::ZERO,
            open: Decimal::ZERO,
            prev_close: Decimal::ZERO,
            volume: Decimal::ZERO,
            trading_value: Decimal::ZERO,
            timestamp: Utc::now(),
        })
    }

    async fn get_quotes(&self, symbols: &[String]) -> Vec<QuoteData> {
        let mut quotes = Vec::new();
        for symbol in symbols {
            if let Ok(quote) = self.get_quote(symbol).await {
                quotes.push(quote);
            }
        }
        quotes
    }
    
    fn provider_name(&self) -> &str {
        "ls_securities"
    }
}
