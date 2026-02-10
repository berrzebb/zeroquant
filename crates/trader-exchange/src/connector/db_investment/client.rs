use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use reqwest::{Client, Method};
use rust_decimal::Decimal;
use std::str::FromStr;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use trader_core::domain::{
    ExchangeProvider, MarketDataProvider, StrategyAccountInfo, PendingOrder,
    StrategyPositionInfo, OrderStatusType, Side, OrderResponse,
};
use trader_core::ProviderError;
use trader_core::QuoteData;

// ============================================================================
// 설정
// ============================================================================

#[derive(Clone)]
pub struct DbInvestmentConfig {
    pub app_key: String,
    pub app_secret: String,
    pub base_url: String,
    pub is_virtual: bool,
}

impl std::fmt::Debug for DbInvestmentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbInvestmentConfig")
            .field("app_key", &"***")
            .field("app_secret", &"***")
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl DbInvestmentConfig {
    pub fn new(app_key: String, app_secret: String, base_url: Option<String>) -> Self {
        Self {
            app_key,
            app_secret,
            base_url: base_url.unwrap_or_else(|| "https://openapi.dbsec.co.kr:8443".to_string()),
            is_virtual: false,
        }
    }


}

// ============================================================================
// 토큰 관리
// ============================================================================

struct TokenManager {
    access_token: Option<String>,
    expires_at: Instant,
}

impl TokenManager {
    fn new() -> Self {
        Self {
            access_token: None,
            expires_at: Instant::now(),
        }
    }
}

// ============================================================================
// DB Investment 클라이언트
// ============================================================================

pub struct DbInvestmentClient {
    client: Client,
    config: DbInvestmentConfig,
    token_manager: Mutex<TokenManager>,
}

impl DbInvestmentClient {
    pub fn new(config: DbInvestmentConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            token_manager: Mutex::new(TokenManager::new()),
        }
    }

    async fn get_token(&self) -> Result<String, ProviderError> {
        let mut tm = self.token_manager.lock().await;

        // 메모리 캐시 확인
        if let Some(token) = &tm.access_token {
            if Instant::now() < tm.expires_at {
                return Ok(token.clone());
            }
        }

        // 만료 시 새 토큰 발급
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
            tm.expires_at = Instant::now() + Duration::from_secs(expires_in.saturating_sub(60));
            tm.access_token = Some(token.to_string());
            
            Ok(token.to_string())
        } else {
            Err(ProviderError::Authentication(
                "No access_token in response".to_string(),
            ))
        }
    }

    #[allow(dead_code)]
    fn is_us_symbol(&self, symbol: &str) -> bool {
        symbol.chars().any(|c| c.is_alphabetic())
    }

    async fn request<T: for<'de> Deserialize<'de>>(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<T, ProviderError> {
        let token = self.get_token().await?;
        let url = format!("{}/{}", self.config.base_url, path);

        let mut builder = self.client.request(method, &url);
        builder = builder.header("Authorization", format!("Bearer {}", token));
        builder = builder.header("Content-Type", "application/json; charset=utf-8");
        builder = builder.header("cont_yn", "N"); 
        builder = builder.header("cont_key", "");

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
                "DB Investment API Error: {}",
                error_text
            )));
        }

        let text = response.text().await.map_err(|e| ProviderError::Network(e.to_string()))?;
        serde_json::from_str::<T>(&text).map_err(|e| {
            ProviderError::Parse(format!("Failed to parse response: {}. Body: {}", e, text))
        })
    }

    async fn fetch_kr_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        let body = json!({ "In": { "QryTpCode0": "0" } });
        let res: KrStockListResponse = self.request(Method::POST, "api/v1/trading/kr-stock/inquiry/balance", Some(body)).await?;
        
        let mut positions = Vec::new();
        for stock in res.out1 {
            let qty = Decimal::from_str(&stock.bal_qty).unwrap_or_default();
            if qty.is_zero() { continue; }
            
            let mut symbol = stock.isu_no;
            if symbol.len() == 7 && symbol.starts_with('A') {
                symbol = symbol[1..].to_string();
            }

            positions.push(StrategyPositionInfo::new(
                symbol,
                Side::Buy,
                qty,
                Decimal::from_str(&stock.exec_prc).unwrap_or_default(),
            ));
        }
        Ok(positions)
    }

    async fn fetch_us_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        let body = json!({
            "In": {
                "TrxTpCode": "2",
                "CmsnTpCode": "2",
                "WonFcurrTpCode": "2",
                "DpntBalTpCode": "0"
            }
        });
        let res: UsStockListResponse = self.request(Method::POST, "api/v1/trading/overseas-stock/inquiry/balance-margin", Some(body)).await?;

        let mut positions = Vec::new();
        for stock in res.out2 {
            let qty = Decimal::from_str(&stock.qty).unwrap_or_default();
            if qty.is_zero() { continue; }

            positions.push(StrategyPositionInfo::new(
                stock.sym_code,
                Side::Buy,
                qty,
                Decimal::from_str(&stock.avg_pchs_prc).unwrap_or_default(),
            ));
        }
        Ok(positions)
    }

    /// 주문 실행
    ///
    /// # Arguments
    ///
    /// * `symbol` - 종목코드 (예: "005930")
    /// * `side` - 매수/매도 방향
    /// * `quantity` - 주문 수량
    /// * `price` - 주문 가격
    /// * `order_class` - 호가 유형 ("00": 지정가, "01": 시장가)
    pub async fn place_order(
        &self,
        symbol: &str,
        side: Side,
        quantity: u32,
        price: Decimal,
        order_class: &str,
    ) -> Result<OrderResponse, ProviderError> {
        // 시장 구분 코드 결정 (간단히 1:코스피, 2:코스닥 추정)
        let market_div_code = if symbol.starts_with('0') || symbol.starts_with('1') {
            "1" // 코스피
        } else {
            "2" // 코스닥
        };

        // 매수/매도 구분 코드 변환
        let dvsn_code = match side {
            Side::Buy => "1",
            Side::Sell => "2",
        };

        let body = json!({
            "In": {
                "InputCondMrktDivCode": market_div_code,
                "InputIscd": symbol,
                "InputQty": quantity.to_string(),
                "InputPrc": price.to_string(),
                "InputDvsnCode": dvsn_code,
                "InputOrdPtnCode": order_class,
            }
        });

        // TR 헤더 추가
        let token = self.get_token().await?;
        let url = format!("{}/api/v1/trading/kr-stock/order", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json; charset=utf-8")
            .header("tr_cd", "CSPAT00600")
            .header("tr_cont", "N")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::Api(format!(
                "DB Investment 주문 실패: {}",
                error_text
            )));
        }

        let text = response.text().await.map_err(|e| ProviderError::Network(e.to_string()))?;
        let res: DbOrderResponseWrapper = serde_json::from_str(&text).map_err(|e| {
            ProviderError::Parse(format!("주문 응답 파싱 실패: {}. Body: {}", e, text))
        })?;

        tracing::info!(
            order_id = %res.out.ord_no,
            symbol = %symbol,
            side = ?side,
            quantity = %quantity,
            price = %price,
            "DB증권 주문 성공"
        );

        Ok(OrderResponse {
            order_no: res.out.ord_no,
            order_time: Utc::now().format("%H%M%S").to_string(),
        })
    }

    /// 주문 취소
    ///
    /// # Arguments
    ///
    /// * `order_id` - 취소할 주문번호
    pub async fn cancel_order(&self, order_id: &str) -> Result<(), ProviderError> {
        let body = json!({
            "In": {
                "InputOrdNo": order_id,
            }
        });

        let token = self.get_token().await?;
        let url = format!("{}/api/v1/trading/kr-stock/order-cancel", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json; charset=utf-8")
            .header("tr_cd", "CSPAT00602")
            .header("tr_cont", "N")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::Api(format!(
                "DB Investment 주문 취소 실패: {}",
                error_text
            )));
        }

        let text = response.text().await.map_err(|e| ProviderError::Network(e.to_string()))?;
        let _res: DbCancelOrderResponseWrapper = serde_json::from_str(&text).map_err(|e| {
            ProviderError::Parse(format!("주문 취소 응답 파싱 실패: {}. Body: {}", e, text))
        })?;

        tracing::info!(order_id = %order_id, "DB증권 주문 취소 성공");

        Ok(())
    }

    /// 체결 내역 조회
    ///
    /// # Arguments
    ///
    /// * `limit` - 조회할 최대 개수
    pub async fn fetch_execution_history(
        &self,
        limit: usize,
    ) -> Result<Vec<ExecutionRecord>, ProviderError> {
        let body = json!({
            "In": {
                "ExecYn": "1", // 1=체결된 주문만
                "BnsTpCode": "0", // 0=전체 (매수/매도)
                "IsuTpCode": "0",
                "QryTp": "0"
            }
        });

        let res: DbExecutionHistoryResponse = self
            .request(Method::POST, "api/v1/trading/kr-stock/inquiry/transaction-history", Some(body))
            .await?;

        let mut executions = Vec::new();
        for record in res.out1.into_iter().take(limit) {
            let mut ticker = record.isu_no;
            // A 접두사 제거 (예: A005930 → 005930)
            if ticker.len() == 7 && ticker.starts_with('A') {
                ticker = ticker[1..].to_string();
            }

            let side = if record.bns_tp_code == "1" { Side::Sell } else { Side::Buy };

            executions.push(ExecutionRecord {
                order_no: record.ord_no,
                exec_no: record.exec_no.unwrap_or_default(),
                ticker,
                side,
                exec_qty: Decimal::from_str(&record.exec_qty).unwrap_or_default(),
                exec_prc: Decimal::from_str(&record.exec_prc).unwrap_or_default(),
                exec_time: record.exec_time.unwrap_or_default(),
                fee: Decimal::from_str(&record.fee.unwrap_or_default()).unwrap_or_default(),
            });
        }

        Ok(executions)
    }

    /// 주문 정정
    ///
    /// # Arguments
    ///
    /// * `order_id` - 정정할 주문번호
    /// * `quantity` - 정정할 수량
    /// * `price` - 정정할 가격
    pub async fn modify_order(
        &self,
        order_id: &str,
        quantity: u32,
        price: Decimal,
    ) -> Result<OrderResponse, ProviderError> {
        let body = json!({
            "In": {
                "InputOrdNo": order_id,
                "InputOrdQty": quantity.to_string(),
                "InputOrdPrc": price.to_string(),
            }
        });

        let token = self.get_token().await?;
        let url = format!("{}/api/v1/trading/kr-stock/order-modify", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json; charset=utf-8")
            .header("tr_cd", "CSPAT00601")
            .header("tr_cont", "N")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::Api(format!(
                "DB Investment 주문 정정 실패: {}",
                error_text
            )));
        }

        let text = response.text().await.map_err(|e| ProviderError::Network(e.to_string()))?;
        let res: DbModifyOrderResponseWrapper = serde_json::from_str(&text).map_err(|e| {
            ProviderError::Parse(format!("주문 정정 응답 파싱 실패: {}. Body: {}", e, text))
        })?;

        tracing::info!(
            order_id = %res.out.ord_no,
            quantity = %quantity,
            price = %price,
            "DB증권 주문 정정 성공"
        );

        Ok(OrderResponse {
            order_no: res.out.ord_no,
            order_time: Utc::now().format("%H%M%S").to_string(),
        })
    }
}

// ============================================================================
// Data Structures for Responses
// ============================================================================

/// 체결 내역 레코드
#[derive(Debug, Clone)]
pub struct ExecutionRecord {
    /// 주문번호
    pub order_no: String,
    /// 체결번호
    pub exec_no: String,
    /// 종목코드
    pub ticker: String,
    /// 매매구분
    pub side: Side,
    /// 체결수량
    pub exec_qty: Decimal,
    /// 체결가격
    pub exec_prc: Decimal,
    /// 체결시각
    pub exec_time: String,
    /// 수수료
    pub fee: Decimal,
}

#[derive(Debug, Deserialize)]
struct DbExecutionHistoryResponse {
    #[serde(rename = "Out1")]
    out1: Vec<DbExecutionHistoryBlock>,
}

#[derive(Debug, Deserialize)]
struct DbExecutionHistoryBlock {
    #[serde(rename = "OrdNo")]
    ord_no: String,
    #[serde(rename = "ExecNo", default)]
    exec_no: Option<String>,
    #[serde(rename = "IsuNo")]
    isu_no: String,
    #[serde(rename = "BnsTpCode")]
    bns_tp_code: String,
    #[serde(rename = "ExecQty")]
    exec_qty: String,
    #[serde(rename = "ExecPrc")]
    exec_prc: String,
    #[serde(rename = "ExecTime", default)]
    exec_time: Option<String>,
    #[serde(rename = "Fee", default)]
    fee: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KrBalanceResponse {
    #[serde(rename = "Out")]
    out: KrBalanceOut,
}

#[derive(Debug, Deserialize)]
struct KrBalanceOut {
    #[serde(rename = "DpsastAmt")]
    dpsast_amt: Value,
    #[serde(rename = "Dps2")]
    dps2: Value,
}

#[derive(Debug, Deserialize)]
struct UsBalanceResponse {
     #[serde(rename = "Out1")]
    out1: Vec<UsBalanceOut1>,
}

#[derive(Debug, Deserialize)]
struct UsBalanceOut1 {
    #[serde(rename = "CrcyCode")]
    crcy_code: String,
    #[serde(rename = "AstkAssetEvalAmt")]
    astk_asset_eval_amt: Value,
    #[serde(rename = "AstkOrdAbleAmt")]
    astk_ord_able_amt: Value,
    #[serde(rename = "Xchrat")]
    xchrat: Value,
}

#[derive(Debug, Deserialize)]
struct KrStockListResponse {
    #[serde(rename = "Out1")]
    out1: Vec<KrStockBalance>,
}

#[derive(Debug, Deserialize)]
struct KrStockBalance {
    #[serde(rename = "IsuNo")]
    isu_no: String,
    #[serde(rename = "BalQty0")]
    bal_qty: String,
    #[serde(rename = "ExecPrc")]
    exec_prc: String,
}

#[derive(Debug, Deserialize)]
struct UsStockListResponse {
    #[serde(rename = "Out2")]
    out2: Vec<UsStockBalance>,
}

#[derive(Debug, Deserialize)]
struct UsStockBalance {
    #[serde(rename = "SymCode")]
    sym_code: String,
    #[serde(rename = "AstkExecBaseQty")]
    qty: String,
    #[serde(rename = "AvgPchsPrc")]
    avg_pchs_prc: String,
}

#[derive(Deserialize, Debug)]
struct DbPendingOrderResponse {
    #[serde(rename = "Out1")]
    out1: Vec<DbPendingOrderBlock>,
}

#[derive(Deserialize, Debug)]
struct DbPendingOrderBlock {
    #[serde(rename = "OrdNo")]
    ord_no: String,
    #[serde(rename = "IsuNo")]
    isu_no: String,
    #[serde(rename = "BnsTpCode")]
    bns_tp_code: String,
    #[serde(rename = "OrdQty")]
    ord_qty: String,
    #[serde(rename = "OrdPrc")]
    ord_prc: String,
    #[serde(rename = "CheQty")]
    che_qty: String,
}

#[derive(Deserialize, Debug)]
struct DbQuoteResponse {
    #[serde(rename = "Out")]
    out: DbQuoteOut,
}

#[derive(Deserialize, Debug)]
struct DbQuoteOut {
    #[serde(rename = "Prpr")]
    prpr: String,
    #[serde(rename = "PrdyVrss")]
    prdy_vrss: String,
    #[serde(rename = "PrdyVrssRat")]
    prdy_vrss_rat: String,
    // 현재가 확장 필드
    #[serde(rename = "OpenPrice", default)]
    open_price: String,
    #[serde(rename = "HighPrice", default)]
    high_price: String,
    #[serde(rename = "LowPrice", default)]
    low_price: String,
    #[serde(rename = "YdayClpr", default)]
    yday_clpr: String,
    #[serde(rename = "AccTrdvol", default)]
    acc_trdvol: String,
    #[serde(rename = "AccTrdval", default)]
    acc_trdval: String,
}

// 주문 응답 구조체
#[derive(Debug, Deserialize)]
struct DbOrderResponseWrapper {
    #[serde(rename = "Out")]
    out: DbOrderOut,
}

#[derive(Debug, Deserialize)]
struct DbOrderOut {
    #[serde(rename = "OrdNo")]
    ord_no: String,
    #[serde(rename = "OrdSttCd")]
    _ord_stt_cd: String,
}

// 주문 정정 응답 구조체
#[derive(Debug, Deserialize)]
struct DbModifyOrderResponseWrapper {
    #[serde(rename = "Out")]
    out: DbModifyOrderOut,
}

#[derive(Debug, Deserialize)]
struct DbModifyOrderOut {
    #[serde(rename = "OrdNo")]
    ord_no: String,
}

// 주문 취소 응답 구조체
#[derive(Debug, Deserialize)]
struct DbCancelOrderResponseWrapper {
    #[serde(rename = "Out")]
    _out: DbCancelOrderOut,
}

#[derive(Debug, Deserialize)]
struct DbCancelOrderOut {
    #[serde(rename = "OrdNo")]
    _ord_no: String,
}

// ============================================================================
// ExchangeProvider 구현
// ============================================================================

#[async_trait]
impl ExchangeProvider for DbInvestmentClient {
    fn exchange_name(&self) -> &str {
        "db_investment"
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

        // KR
        let kr_body = json!({ "In": { "QryTpCode0": "0" } });
        if let Ok(res) = self.request::<KrBalanceResponse>(Method::POST, "api/v1/trading/kr-stock/inquiry/balance", Some(kr_body)).await {
            available_balance += parse_value(&res.out.dps2);
            total_balance += parse_value(&res.out.dpsast_amt);
        }

        // US
        let us_body = json!({
            "In": {
                "TrxTpCode": "1",
                "CmsnTpCode": "2",
                "WonFcurrTpCode": "2",
                "DpntBalTpCode": "0"
            }
        });
        if let Ok(res) = self.request::<UsBalanceResponse>(Method::POST, "api/v1/trading/overseas-stock/inquiry/balance-margin", Some(us_body)).await {
            for balance in res.out1 {
                 if balance.crcy_code == "USD" {
                     let exrate = parse_value(&balance.xchrat);
                     let usd_total = parse_value(&balance.astk_asset_eval_amt);
                     let usd_avail = parse_value(&balance.astk_ord_able_amt);

                     total_balance += usd_total * exrate;
                     available_balance += usd_avail * exrate;
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
        let mut all_positions = Vec::new();
        
        // KR
        match self.fetch_kr_positions().await {
            Ok(pos) => all_positions.extend(pos),
            Err(e) => eprintln!("Failed to fetch KR positions: {:?}", e),
        }

        // US
        match self.fetch_us_positions().await {
            Ok(pos) => all_positions.extend(pos),
            Err(e) => eprintln!("Failed to fetch US positions: {:?}", e),
        }

        Ok(all_positions)
    }

    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError> {
        let body = json!({
            "In": {
                "ExecYn": "2", // 2=OPEN (Unexecuted)
                "BnsTpCode": "0", // 0=ALL
                "IsuTpCode": "0",
                "QryTp": "0"
            }
        });
        let res: DbPendingOrderResponse = self.request(Method::POST, "api/v1/trading/kr-stock/inquiry/transaction-history", Some(body)).await?;
        
        let mut pending = Vec::new();
            for order in res.out1 {
            let mut ticker = order.isu_no;
            if ticker.len() == 7 && ticker.starts_with('A') {
                ticker = ticker[1..].to_string();
            }

            let side = if order.bns_tp_code == "1" { Side::Sell } else { Side::Buy };

            pending.push(PendingOrder {
                order_id: order.ord_no,
                ticker,
                side,
                price: Decimal::from_str(&order.ord_prc).unwrap_or_default(),
                quantity: Decimal::from_str(&order.ord_qty).unwrap_or_default(),
                filled_quantity: Decimal::from_str(&order.che_qty).unwrap_or_default(),
                status: OrderStatusType::Open,
                created_at: Utc::now(),
            });
        }
        Ok(pending)
    }
}

#[async_trait]
impl MarketDataProvider for DbInvestmentClient {
    async fn get_quote(&self, symbol: &str) -> Result<QuoteData, ProviderError> {
        let body = json!({
            "In": {
                "InputCondMrktDivCode": "J",
                "InputIscd1": symbol
            }
        });
        let res: DbQuoteResponse = self.request(Method::POST, "api/v1/quote/kr-stock/inquiry/price", Some(body)).await?;
        
        Ok(QuoteData {
            symbol: symbol.to_string(),
            current_price: Decimal::from_str(&res.out.prpr).unwrap_or_default(),
            price_change: Decimal::from_str(&res.out.prdy_vrss).unwrap_or_default(),
            change_percent: Decimal::from_str(&res.out.prdy_vrss_rat).unwrap_or_default(),
            high: Decimal::from_str(&res.out.high_price).unwrap_or_default(),
            low: Decimal::from_str(&res.out.low_price).unwrap_or_default(),
            open: Decimal::from_str(&res.out.open_price).unwrap_or_default(),
            prev_close: Decimal::from_str(&res.out.yday_clpr).unwrap_or_default(),
            volume: Decimal::from_str(&res.out.acc_trdvol).unwrap_or_default(),
            trading_value: Decimal::from_str(&res.out.acc_trdval).unwrap_or_default(),
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
        "db_investment"
    }
}
