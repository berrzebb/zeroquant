//! 심볼 정보 Provider.
//!
//! 국내(KRX), 해외(Yahoo Finance), 코인(Binance 등)의
//! 심볼 정보(티커, 회사명)를 제공합니다.

#![allow(clippy::type_complexity)]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 심볼 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMetadata {
    /// 티커 코드 (예: AAPL, 005930, BTCUSDT)
    pub ticker: String,
    /// 회사/자산명 (예: Apple Inc., 삼성전자, Bitcoin)
    pub name: String,
    /// 영문명 (선택)
    pub name_en: Option<String>,
    /// 시장 (KR, US, CRYPTO)
    pub market: String,
    /// 거래소 (NYSE, NASDAQ, KRX, KOSDAQ, BINANCE)
    pub exchange: Option<String>,
    /// 섹터/업종
    pub sector: Option<String>,
    /// Yahoo Finance 호환 심볼
    pub yahoo_symbol: Option<String>,
}

/// 심볼 정보 Provider trait.
#[async_trait]
pub trait SymbolInfoProvider: Send + Sync {
    /// Provider 이름.
    fn name(&self) -> &str;

    /// 지원하는 시장.
    fn supported_markets(&self) -> Vec<&str>;

    /// 모든 심볼 정보 조회.
    async fn fetch_all(
        &self,
    ) -> Result<Vec<SymbolMetadata>, Box<dyn std::error::Error + Send + Sync>>;

    /// 심볼 검색.
    async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SymbolMetadata>, Box<dyn std::error::Error + Send + Sync>>;
}

// ==================== KRX Provider ====================

/// KRX 심볼 정보 Provider.
///
/// KRX Open API를 통해 한국 주식 심볼 정보를 제공합니다.
pub struct KrxSymbolProvider;

impl KrxSymbolProvider {
    pub fn new() -> Self {
        Self
    }

    /// KRX Open API 인증키 (환경변수에서 로드)
    pub fn get_api_key() -> Option<String> {
        std::env::var("KRX_API_KEY").ok()
    }

    /// ETF 전종목 조회 (data.krx.co.kr).
    async fn fetch_etf(
        &self,
        client: &reqwest::Client,
    ) -> Result<Vec<SymbolMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        #[derive(Deserialize)]
        struct KrxEtfResponse {
            #[serde(rename = "OutBlock_1")]
            out_block: Option<Vec<KrxEtf>>,
        }

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct KrxEtf {
            #[serde(rename = "ISU_SRT_CD")]
            ticker: String,
            #[serde(rename = "ISU_NM")]
            name: String,
            #[serde(rename = "IDX_IND_NM", default)]
            index_name: Option<String>,
        }

        let params = [
            ("bld", "dbms/MDC/STAT/standard/MDCSTAT04601"),
            ("share", "1"),
            ("csvxls_isNo", "false"),
        ];

        let response = client
            .post("http://data.krx.co.kr/comm/bldAttendant/getJsonData.cmd")
            .form(&params)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await?;

        let data: KrxEtfResponse = response.json().await?;

        let symbols: Vec<SymbolMetadata> = data
            .out_block
            .unwrap_or_default()
            .into_iter()
            .map(|s| SymbolMetadata {
                ticker: s.ticker.clone(),
                name: s.name,
                name_en: None,
                market: "KR".to_string(),
                exchange: Some("KRX".to_string()),
                sector: Some("ETF".to_string()),
                yahoo_symbol: Some(format!("{}.KS", s.ticker)),
            })
            .collect();

        tracing::info!("KRX ETF 종목 수집: {}개", symbols.len());
        Ok(symbols)
    }

    /// 시장별 종목 조회 (data.krx.co.kr).
    async fn fetch_market(
        &self,
        client: &reqwest::Client,
        market_code: &str,
    ) -> Result<Vec<SymbolMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        #[derive(Deserialize)]
        struct KrxResponse {
            #[serde(rename = "OutBlock_1")]
            out_block: Option<Vec<KrxStock>>,
        }

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct KrxStock {
            #[serde(rename = "ISU_SRT_CD")]
            ticker: String,
            #[serde(rename = "ISU_ABBRV")]
            name: String,
            #[serde(rename = "ISU_ENG_NM", default)]
            name_en: Option<String>,
            #[serde(rename = "MKT_NM", default)]
            market_name: Option<String>,
            #[serde(rename = "SECT_TP_NM", default)]
            sector: Option<String>,
        }

        let params = [
            ("bld", "dbms/MDC/STAT/standard/MDCSTAT01501"),
            ("mktId", market_code),
            ("share", "1"),
            ("csvxls_is498No", "false"),
        ];

        let response = client
            .post("http://data.krx.co.kr/comm/bldAttendant/getJsonData.cmd")
            .form(&params)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await?;

        let data: KrxResponse = response.json().await?;

        let exchange = if market_code == "STK" {
            "KRX"
        } else {
            "KOSDAQ"
        };
        let suffix = if market_code == "STK" { ".KS" } else { ".KQ" };

        let symbols: Vec<SymbolMetadata> = data
            .out_block
            .unwrap_or_default()
            .into_iter()
            .map(|s| SymbolMetadata {
                ticker: s.ticker.clone(),
                name: s.name,
                name_en: s.name_en,
                market: "KR".to_string(),
                exchange: Some(exchange.to_string()),
                sector: s.sector,
                yahoo_symbol: Some(format!("{}{}", s.ticker, suffix)),
            })
            .collect();

        Ok(symbols)
    }
}

impl Default for KrxSymbolProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SymbolInfoProvider for KrxSymbolProvider {
    fn name(&self) -> &str {
        "KRX"
    }

    fn supported_markets(&self) -> Vec<&str> {
        vec!["KR"]
    }

    async fn fetch_all(
        &self,
    ) -> Result<Vec<SymbolMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::new();

        // KOSPI 종목
        let kospi = self.fetch_market(&client, "STK").await?;
        let kospi_count = kospi.len();

        // KOSDAQ 종목
        let kosdaq = self.fetch_market(&client, "KSQ").await?;
        let kosdaq_count = kosdaq.len();

        // ETF 종목
        let etf = self.fetch_etf(&client).await.unwrap_or_else(|e| {
            tracing::warn!("ETF 종목 수집 실패: {}", e);
            Vec::new()
        });
        let etf_count = etf.len();

        let mut all = kospi;
        all.extend(kosdaq);
        all.extend(etf);

        tracing::info!(
            "KRX 종목 수집 완료: KOSPI {}개, KOSDAQ {}개, ETF {}개",
            kospi_count,
            kosdaq_count,
            etf_count
        );

        Ok(all)
    }

    async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SymbolMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        let all = self.fetch_all().await?;
        let query_upper = query.to_uppercase();

        let results: Vec<SymbolMetadata> = all
            .into_iter()
            .filter(|s| {
                s.ticker.to_uppercase().contains(&query_upper)
                    || s.name.to_uppercase().contains(&query_upper)
                    || s.name_en
                        .as_ref()
                        .map(|n| n.to_uppercase().contains(&query_upper))
                        .unwrap_or(false)
            })
            .take(limit)
            .collect();

        Ok(results)
    }
}

// ==================== Binance Provider ====================

/// Binance 심볼 정보 Provider.
///
/// Binance API를 통해 암호화폐 심볼 정보를 제공합니다.
pub struct BinanceSymbolProvider;

impl BinanceSymbolProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BinanceSymbolProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SymbolInfoProvider for BinanceSymbolProvider {
    fn name(&self) -> &str {
        "Binance"
    }

    fn supported_markets(&self) -> Vec<&str> {
        vec!["CRYPTO"]
    }

    async fn fetch_all(
        &self,
    ) -> Result<Vec<SymbolMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::new();

        #[derive(Deserialize)]
        struct ExchangeInfo {
            symbols: Vec<BinanceSymbol>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)] // API 응답 전체 필드 매핑 (일부만 사용)
        struct BinanceSymbol {
            symbol: String,
            base_asset: String,
            quote_asset: String,
            status: String,
        }

        let response = client
            .get("https://api.binance.com/api/v3/exchangeInfo")
            .send()
            .await?;

        let data: ExchangeInfo = response.json().await?;

        // 티커는 정규화된 형식(BTC/USDT)으로 저장
        // Yahoo Finance는 암호화폐를 지원하지 않으므로 yahoo_symbol은 None
        let symbols: Vec<SymbolMetadata> = data
            .symbols
            .into_iter()
            .filter(|s| s.status == "TRADING")
            .map(|s| {
                let normalized_ticker = format!("{}/{}", s.base_asset, s.quote_asset);
                SymbolMetadata {
                    ticker: normalized_ticker.clone(), // 정규화된 형식
                    name: normalized_ticker,
                    name_en: Some(s.base_asset.clone()),
                    market: "CRYPTO".to_string(),
                    exchange: Some("BINANCE".to_string()),
                    sector: None,
                    yahoo_symbol: None, // Yahoo Finance는 암호화폐 미지원
                }
            })
            .collect();

        Ok(symbols)
    }

    async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SymbolMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        let all = self.fetch_all().await?;
        let query_upper = query.to_uppercase();

        let results: Vec<SymbolMetadata> = all
            .into_iter()
            .filter(|s| {
                s.ticker.to_uppercase().contains(&query_upper)
                    || s.name.to_uppercase().contains(&query_upper)
            })
            .take(limit)
            .collect();

        Ok(results)
    }
}

// ==================== Yahoo Finance Provider ====================

/// Yahoo Finance 심볼 정보 Provider.
///
/// Yahoo Finance Screener API를 통해 미국 주식 심볼 정보를 제공합니다.
/// S&P 500, NASDAQ 100 등 주요 지수 구성 종목을 수집합니다.
pub struct YahooSymbolProvider {
    /// 수집할 최대 종목 수 (기본: 1000)
    max_symbols: usize,
}

impl YahooSymbolProvider {
    pub fn new() -> Self {
        Self { max_symbols: 1000 }
    }

    /// 최대 종목 수 설정.
    pub fn with_max_symbols(max_symbols: usize) -> Self {
        Self { max_symbols }
    }

    /// Yahoo Finance Screener API로 종목 목록 조회.
    async fn fetch_screener(
        &self,
        client: &reqwest::Client,
        screener_id: &str,
    ) -> Result<Vec<SymbolMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        // Yahoo Finance Screener API 엔드포인트
        // 미리 정의된 screener: most_actives, day_gainers, day_losers, undervalued_large_caps 등
        let url = format!(
            "https://query1.finance.yahoo.com/v1/finance/screener/predefined/saved?scrIds={}&count=250",
            screener_id
        );

        let response = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await?;

        #[derive(Deserialize)]
        struct ScreenerResponse {
            finance: Option<FinanceResult>,
        }

        #[derive(Deserialize)]
        struct FinanceResult {
            result: Option<Vec<ScreenerResult>>,
        }

        #[derive(Deserialize)]
        struct ScreenerResult {
            quotes: Option<Vec<Quote>>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct Quote {
            symbol: String,
            #[serde(default)]
            short_name: Option<String>,
            #[serde(default)]
            long_name: Option<String>,
            #[serde(default)]
            exchange: Option<String>,
            #[serde(default)]
            market: Option<String>,
            #[serde(default)]
            sector: Option<String>,
        }

        let data: ScreenerResponse = response.json().await?;

        let symbols: Vec<SymbolMetadata> = data
            .finance
            .and_then(|f| f.result)
            .and_then(|r| r.into_iter().next())
            .and_then(|r| r.quotes)
            .unwrap_or_default()
            .into_iter()
            .filter(|q| {
                !q.symbol.contains('.') || q.symbol.ends_with(".L") || q.symbol.ends_with(".T")
            })
            .map(|q| {
                let name = q
                    .long_name
                    .or(q.short_name)
                    .unwrap_or_else(|| q.symbol.clone());
                let exchange_name = q.exchange.clone();
                // 정규화된 티커: Yahoo 접미사 제거 (AAPL.L → AAPL for UK)
                // US 주식은 접미사가 없으므로 그대로 사용
                let normalized_ticker = q.symbol.clone();
                SymbolMetadata {
                    ticker: normalized_ticker, // 정규화된 형식
                    name: name.clone(),
                    name_en: Some(name),
                    market: "US".to_string(),
                    exchange: exchange_name,
                    sector: q.sector,
                    yahoo_symbol: Some(q.symbol), // 조회용 Yahoo 심볼
                }
            })
            .collect();

        Ok(symbols)
    }

    /// 주요 지수 구성 종목 조회 (Yahoo Finance).
    #[allow(dead_code)]
    async fn fetch_index_components(
        &self,
        client: &reqwest::Client,
        index_symbol: &str,
    ) -> Result<Vec<SymbolMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        // Yahoo Finance Quote API를 사용하여 지수 정보 조회
        // 실제로는 별도 데이터 소스가 필요할 수 있음
        let url = format!(
            "https://query1.finance.yahoo.com/v7/finance/quote?symbols={}",
            index_symbol
        );

        let response = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await?;

        // 지수 자체 정보만 반환 (구성 종목은 screener로 수집)
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct QuoteResponse {
            quote_response: Option<QuoteResult>,
        }

        #[derive(Deserialize)]
        struct QuoteResult {
            result: Option<Vec<QuoteData>>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct QuoteData {
            symbol: String,
            #[serde(default)]
            short_name: Option<String>,
            #[serde(default)]
            long_name: Option<String>,
        }

        let _data: QuoteResponse = response.json().await?;

        // 지수 자체는 건너뛰고, 구성 종목은 screener API 사용
        Ok(vec![])
    }
}

impl Default for YahooSymbolProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SymbolInfoProvider for YahooSymbolProvider {
    fn name(&self) -> &str {
        "Yahoo Finance"
    }

    fn supported_markets(&self) -> Vec<&str> {
        vec!["US", "UK", "JP"]
    }

    async fn fetch_all(
        &self,
    ) -> Result<Vec<SymbolMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::new();
        let mut all_symbols: Vec<SymbolMetadata> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // 여러 Screener 카테고리에서 종목 수집
        let screener_ids = [
            "most_actives",              // 거래량 상위
            "day_gainers",               // 상승 종목
            "day_losers",                // 하락 종목
            "undervalued_large_caps",    // 저평가 대형주
            "growth_technology_stocks",  // 기술 성장주
            "undervalued_growth_stocks", // 저평가 성장주
            "small_cap_gainers",         // 소형주 상승
        ];

        for screener_id in &screener_ids {
            match self.fetch_screener(&client, screener_id).await {
                Ok(symbols) => {
                    for symbol in symbols {
                        if !seen.contains(&symbol.ticker) && all_symbols.len() < self.max_symbols {
                            seen.insert(symbol.ticker.clone());
                            all_symbols.push(symbol);
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!(
                        screener = screener_id,
                        error = %e,
                        "Screener 조회 실패, 계속 진행"
                    );
                }
            }

            // Rate limiting: screener 요청 간 딜레이
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            if all_symbols.len() >= self.max_symbols {
                break;
            }
        }

        tracing::info!(count = all_symbols.len(), "Yahoo Finance 심볼 수집 완료");

        Ok(all_symbols)
    }

    async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SymbolMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        // Yahoo Finance 검색 API 사용
        let client = reqwest::Client::new();
        let url = format!(
            "https://query1.finance.yahoo.com/v1/finance/search?q={}&quotesCount={}&newsCount=0",
            query, limit
        );

        let response = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await?;

        #[derive(Deserialize)]
        struct SearchResponse {
            quotes: Option<Vec<SearchQuote>>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SearchQuote {
            symbol: String,
            #[serde(default)]
            short_name: Option<String>,
            #[serde(default)]
            long_name: Option<String>,
            #[serde(default)]
            exchange: Option<String>,
            #[serde(default)]
            quote_type: Option<String>,
            #[serde(default)]
            sector: Option<String>,
        }

        let data: SearchResponse = response.json().await?;

        let symbols: Vec<SymbolMetadata> = data
            .quotes
            .unwrap_or_default()
            .into_iter()
            .filter(|q| {
                // EQUITY만 필터링 (ETF, INDEX 등 제외)
                q.quote_type.as_ref().map(|t| t == "EQUITY").unwrap_or(true)
            })
            .take(limit)
            .map(|q| {
                let name = q
                    .long_name
                    .or(q.short_name)
                    .unwrap_or_else(|| q.symbol.clone());
                let market = if q.symbol.ends_with(".KS") || q.symbol.ends_with(".KQ") {
                    "KR"
                } else if q.symbol.ends_with(".T") {
                    "JP"
                } else if q.symbol.ends_with(".L") {
                    "UK"
                } else {
                    "US"
                };
                SymbolMetadata {
                    ticker: q.symbol.clone(),
                    name: name.clone(),
                    name_en: Some(name),
                    market: market.to_string(),
                    exchange: q.exchange,
                    sector: q.sector,
                    yahoo_symbol: Some(q.symbol),
                }
            })
            .collect();

        Ok(symbols)
    }
}

// ==================== Composite Provider ====================

/// 복합 심볼 정보 Provider.
///
/// 여러 Provider를 조합하여 국내/해외/코인 심볼 정보를 통합 제공합니다.
pub struct CompositeSymbolProvider {
    providers: Vec<Box<dyn SymbolInfoProvider>>,
}

impl CompositeSymbolProvider {
    pub fn new() -> Self {
        Self {
            providers: vec![
                Box::new(KrxSymbolProvider::new()),
                Box::new(BinanceSymbolProvider::new()),
                Box::new(YahooSymbolProvider::new()),
            ],
        }
    }

    /// Provider 추가.
    pub fn add_provider(&mut self, provider: Box<dyn SymbolInfoProvider>) {
        self.providers.push(provider);
    }

    /// 모든 심볼 정보 조회.
    pub async fn fetch_all(
        &self,
    ) -> Result<Vec<SymbolMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all = Vec::new();

        for provider in &self.providers {
            match provider.fetch_all().await {
                Ok(symbols) => {
                    tracing::info!(
                        provider = provider.name(),
                        count = symbols.len(),
                        "심볼 정보 로드 완료"
                    );
                    all.extend(symbols);
                }
                Err(e) => {
                    tracing::warn!(
                        provider = provider.name(),
                        error = %e,
                        "심볼 정보 로드 실패"
                    );
                }
            }
        }

        Ok(all)
    }

    /// 심볼 검색.
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SymbolMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        let mut results = Vec::new();

        for provider in &self.providers {
            if let Ok(symbols) = provider.search(query, limit).await {
                results.extend(symbols);
            }
        }

        // 정렬: 정확히 매칭되는 것 우선
        let query_upper = query.to_uppercase();
        results.sort_by(|a, b| {
            let a_exact = a.ticker.to_uppercase() == query_upper;
            let b_exact = b.ticker.to_uppercase() == query_upper;
            b_exact.cmp(&a_exact)
        });

        results.truncate(limit);
        Ok(results)
    }
}

impl Default for CompositeSymbolProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== Symbol Resolver ====================

/// 심볼 변환 서비스.
///
/// 중립 심볼(canonical)과 데이터 소스별 심볼 간의 변환을 담당합니다.
/// DB(symbol_info 테이블)를 통해 매핑 정보를 조회합니다.
///
/// **캐싱**: 한 번 조회된 심볼 정보는 메모리에 캐시되어 빠르게 반환됩니다.
pub struct SymbolResolver {
    pool: sqlx::PgPool,
    /// 심볼 메타데이터 캐시 (ticker uppercase -> SymbolMetadata)
    cache: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, SymbolMetadata>>>,
}

impl SymbolResolver {
    /// 새로운 SymbolResolver 생성.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            pool,
            cache: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// 캐시 크기 조회.
    pub async fn cache_size(&self) -> usize {
        self.cache.read().await.len()
    }

    /// 캐시 초기화.
    pub async fn clear_cache(&self) {
        self.cache.write().await.clear();
    }

    /// 중립 심볼(ticker)을 데이터 소스 심볼로 변환.
    ///
    /// # Arguments
    /// * `canonical` - 중립 심볼 (예: "005930", "AAPL", "BTC/USDT")
    /// * `source` - 데이터 소스 (예: "yahoo", "binance", "kis")
    ///
    /// # Returns
    /// 데이터 소스별 심볼 (예: "005930.KS", "BTCUSDT")
    pub async fn to_source_symbol(
        &self,
        canonical: &str,
        source: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        match source.to_lowercase().as_str() {
            "yahoo" => {
                let result: Option<(Option<String>,)> = sqlx::query_as(
                    "SELECT yahoo_symbol FROM symbol_info WHERE UPPER(ticker) = UPPER($1) AND is_active = true LIMIT 1",
                )
                .bind(canonical)
                .fetch_optional(&self.pool)
                .await?;

                Ok(result.and_then(|(yahoo,)| yahoo))
            }
            "binance" => {
                // Binance: "BTC/USDT" → "BTCUSDT"
                Ok(Some(canonical.replace("/", "")))
            }
            "kis" => {
                // KIS: 티커 그대로 사용
                Ok(Some(canonical.to_string()))
            }
            _ => Ok(Some(canonical.to_string())),
        }
    }

    /// 데이터 소스 심볼을 중립 심볼(ticker)로 변환.
    ///
    /// # Arguments
    /// * `source_symbol` - 데이터 소스 심볼 (예: "005930.KS", "BTCUSDT")
    /// * `source` - 데이터 소스 (예: "yahoo", "binance", "kis")
    ///
    /// # Returns
    /// 중립 심볼 (예: "005930", "BTC/USDT")
    pub async fn to_canonical(
        &self,
        source_symbol: &str,
        source: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        match source.to_lowercase().as_str() {
            "yahoo" => {
                let result: Option<(String,)> = sqlx::query_as(
                    "SELECT ticker FROM symbol_info WHERE yahoo_symbol = $1 AND is_active = true LIMIT 1",
                )
                .bind(source_symbol)
                .fetch_optional(&self.pool)
                .await?;

                Ok(result.map(|(ticker,)| ticker))
            }
            "binance" => {
                // BTCUSDT → BTC/USDT (간단한 휴리스틱)
                // 실제로는 symbol_info 테이블 조회 필요
                if let Some(base) = source_symbol.strip_suffix("USDT") {
                    Ok(Some(format!("{}/USDT", base)))
                } else if let Some(base) = source_symbol.strip_suffix("BTC") {
                    Ok(Some(format!("{}/BTC", base)))
                } else {
                    Ok(Some(source_symbol.to_string()))
                }
            }
            "kis" => {
                // KIS: 티커 그대로
                Ok(Some(source_symbol.to_string()))
            }
            _ => Ok(Some(source_symbol.to_string())),
        }
    }

    /// Yahoo 심볼에서 시장 정보 추출.
    ///
    /// 접미사로 시장 판별:
    /// - .KS: KOSPI
    /// - .KQ: KOSDAQ
    /// - .T: Tokyo
    /// - 없음: US
    pub fn get_market_from_yahoo_symbol(yahoo_symbol: &str) -> &'static str {
        if yahoo_symbol.ends_with(".KS") || yahoo_symbol.ends_with(".KQ") {
            "KR"
        } else if yahoo_symbol.ends_with(".T") {
            "JP"
        } else if yahoo_symbol.ends_with(".L") {
            "UK"
        } else if yahoo_symbol.contains("-") {
            // BTC-USD 형식
            "CRYPTO"
        } else {
            "US"
        }
    }

    /// 심볼 메타데이터 조회.
    /// 심볼 메타데이터 조회 (캐싱 지원).
    ///
    /// 심볼 형식을 자동으로 인식하여 정규화합니다.
    /// - "005930.KS", "005930.KQ" → "005930"
    /// - "BTCUSDT" → "BTC/USDT"
    /// - "BTC-USD" → "BTC/USD"
    /// - 그 외는 그대로 사용
    ///
    /// # Arguments
    /// * `symbol` - 심볼 (어떤 형식이든 가능)
    ///
    /// # Examples
    /// ```
    /// resolver.get_symbol_info("005930").await?;     // Korean stock
    /// resolver.get_symbol_info("005930.KS").await?;  // Yahoo format
    /// resolver.get_symbol_info("AAPL").await?;       // US stock
    /// resolver.get_symbol_info("BTCUSDT").await?;    // Binance format
    /// ```
    pub async fn get_symbol_info(
        &self,
        symbol: &str,
    ) -> Result<Option<SymbolMetadata>, sqlx::Error> {
        // 심볼 형식 자동 인식 및 정규화
        let canonical = Self::normalize_symbol(symbol);
        let cache_key = canonical.to_uppercase();

        // 1. 캐시 확인
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                return Ok(Some(cached.clone()));
            }
        }

        // 2. DB 조회 - ticker 또는 yahoo_symbol로 검색
        let result: Option<(
            String,
            String,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(
            r#"
            SELECT ticker, name, name_en, market, exchange, sector, yahoo_symbol
            FROM symbol_info
            WHERE (UPPER(ticker) = UPPER($1) OR UPPER(ticker) = UPPER($2) OR yahoo_symbol = $2) AND is_active = true
            LIMIT 1
            "#,
        )
        .bind(&canonical)
        .bind(symbol) // 원본 심볼로도 ticker 및 yahoo_symbol 검색
        .fetch_optional(&self.pool)
        .await?;

        let metadata = result.map(
            |(ticker, name, name_en, market, exchange, sector, yahoo_symbol)| SymbolMetadata {
                ticker,
                name,
                name_en,
                market,
                exchange,
                sector,
                yahoo_symbol,
            },
        );

        // 3. 캐시 저장
        if let Some(ref meta) = metadata {
            let mut cache = self.cache.write().await;
            cache.insert(meta.ticker.to_uppercase(), meta.clone());
        }

        Ok(metadata)
    }

    /// 심볼 형식을 자동 인식하여 정규화 (canonical 형식으로 변환).
    ///
    /// # 변환 규칙
    /// - "005930.KS", "005930.KQ" → "005930" (한국 주식)
    /// - "BTCUSDT" → "BTC/USDT" (암호화폐)
    /// - "ETHBTC" → "ETH/BTC" (암호화폐)
    /// - "BTC-USD" → "BTC/USD" (암호화폐)
    /// - 그 외는 그대로 사용
    pub fn normalize_symbol(symbol: &str) -> String {
        let s = symbol.trim();

        // 1. Yahoo 한국 주식 접미사 제거 (.KS, .KQ)
        if let Some(pos) = s.rfind('.') {
            let suffix = &s[pos..];
            if suffix == ".KS" || suffix == ".KQ" {
                return s[..pos].to_string();
            }
        }

        // 2. Yahoo 암호화폐 형식 변환 (BTC-USD → BTC/USD)
        if s.contains('-') && s.len() <= 12 {
            let parts: Vec<&str> = s.split('-').collect();
            if parts.len() == 2 && parts[1].len() <= 4 {
                return format!("{}/{}", parts[0], parts[1]);
            }
        }

        // 3. Binance 형식 변환 (BTCUSDT → BTC/USDT)
        if !s.contains('/') && !s.contains('.') && !s.contains('-') {
            // 일반적인 quote 통화 접미사 확인
            for quote in &["USDT", "USDC", "BUSD", "USD", "BTC", "ETH", "BNB", "KRW"] {
                if s.ends_with(quote) && s.len() > quote.len() {
                    let base = &s[..s.len() - quote.len()];
                    // 모두 알파벳인 경우에만 변환 (숫자가 포함되면 주식 코드일 수 있음)
                    if base.chars().all(|c| c.is_ascii_alphabetic()) {
                        return format!("{}/{}", base, quote);
                    }
                }
            }
        }

        // 4. 이미 canonical 형식이거나 변환 불필요
        s.to_string()
    }

    /// 심볼 정보를 조회하거나, 없으면 자동 생성하여 DB에 저장.
    ///
    /// 이 메서드를 통해 심볼 데이터베이스가 지속적으로 업데이트됩니다.
    ///
    /// # Arguments
    /// * `symbol` - 심볼 (canonical 또는 데이터 소스별 심볼)
    /// * `data_source` - 데이터 소스 (None이면 canonical, "yahoo", "binance", "kis" 등)
    pub async fn get_or_create_symbol_info(
        &self,
        symbol: &str,
    ) -> Result<SymbolMetadata, sqlx::Error> {
        // 1. 기존 조회 시도
        if let Some(info) = self.get_symbol_info(symbol).await? {
            return Ok(info);
        }

        // 2. 심볼 정규화
        let canonical = Self::normalize_symbol(symbol);

        // 3. 메타데이터 자동 생성
        let metadata = Self::build_symbol_metadata(&canonical);

        // 4. DB에 저장
        self.save_symbol_info(&metadata).await?;

        // 5. 캐시에 저장
        {
            let mut cache = self.cache.write().await;
            cache.insert(canonical.to_uppercase(), metadata.clone());
        }

        Ok(metadata)
    }

    /// 심볼 정보 자동 생성.
    ///
    /// 티커 형식에 따라 시장과 Yahoo 심볼을 추론합니다:
    /// - 6자리 숫자: 한국 주식 (KRX) → yahoo_symbol: {ticker}.KS
    /// - "/"가 포함: 암호화폐 (CRYPTO)
    /// - 그 외: 미국 주식 (US)
    pub fn build_symbol_metadata(canonical: &str) -> SymbolMetadata {
        let ticker = canonical.trim().to_uppercase();

        // 한국 주식 (6자리 숫자)
        if ticker.len() == 6 && ticker.chars().all(|c| c.is_ascii_digit()) {
            return SymbolMetadata {
                ticker: ticker.clone(),
                name: ticker.clone(), // 이름은 나중에 외부 API로 업데이트 가능
                name_en: None,
                market: "KR".to_string(),
                exchange: Some("KRX".to_string()),
                sector: None,
                yahoo_symbol: Some(format!("{}.KS", ticker)),
            };
        }

        // 암호화폐 (BTC/USDT 형식)
        if canonical.contains('/') {
            let parts: Vec<&str> = canonical.split('/').collect();
            let base = parts.first().unwrap_or(&"").to_uppercase();
            return SymbolMetadata {
                ticker: ticker.clone(),
                name: base.clone(),
                name_en: None,
                market: "CRYPTO".to_string(),
                exchange: Some("BINANCE".to_string()),
                sector: None,
                yahoo_symbol: Some(format!("{}-USD", base)),
            };
        }

        // 미국 주식 (기본)
        SymbolMetadata {
            ticker: ticker.clone(),
            name: ticker.clone(),
            name_en: None,
            market: "US".to_string(),
            exchange: None,
            sector: None,
            yahoo_symbol: Some(ticker),
        }
    }

    /// 심볼 정보를 DB에 저장.
    pub async fn save_symbol_info(&self, metadata: &SymbolMetadata) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO symbol_info (ticker, name, name_en, market, exchange, sector, yahoo_symbol, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7, true)
            ON CONFLICT (ticker, market) DO UPDATE SET
                name = EXCLUDED.name,
                name_en = COALESCE(symbol_info.name_en, EXCLUDED.name_en),
                exchange = COALESCE(symbol_info.exchange, EXCLUDED.exchange),
                sector = COALESCE(symbol_info.sector, EXCLUDED.sector),
                yahoo_symbol = COALESCE(symbol_info.yahoo_symbol, EXCLUDED.yahoo_symbol),
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&metadata.ticker)
        .bind(&metadata.name)
        .bind(&metadata.name_en)
        .bind(&metadata.market)
        .bind(&metadata.exchange)
        .bind(&metadata.sector)
        .bind(&metadata.yahoo_symbol)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 여러 심볼의 display name을 배치로 조회.
    ///
    /// 캐시를 먼저 확인하고, 캐시에 없는 심볼만 DB에서 조회합니다.
    /// DB에도 없는 심볼은 자동으로 생성하여 저장합니다.
    ///
    /// # Arguments
    /// * `symbols` - 중립 심볼 목록
    /// * `use_english` - 영문명 사용 여부
    ///
    /// # Returns
    /// HashMap<심볼, display_name>
    pub async fn get_display_names_batch(
        &self,
        symbols: &[String],
        use_english: bool,
    ) -> Result<std::collections::HashMap<String, String>, sqlx::Error> {
        use std::collections::HashMap;

        if symbols.is_empty() {
            return Ok(HashMap::new());
        }

        let mut result = HashMap::new();
        let mut missing_entries: Vec<(String, String)> = Vec::new(); // (원본 심볼, 정규화된 심볼)

        // 1. 심볼 정규화 및 캐시 확인
        {
            let cache = self.cache.read().await;
            for symbol in symbols {
                let normalized = Self::normalize_symbol(symbol);
                let cache_key = normalized.to_uppercase();

                if let Some(meta) = cache.get(&cache_key) {
                    result.insert(symbol.clone(), meta.to_display_string(use_english));
                } else {
                    missing_entries.push((symbol.clone(), normalized));
                }
            }
        }

        // 2. 캐시에 없는 심볼은 DB에서 배치 조회
        if !missing_entries.is_empty() {
            let normalized_upper: Vec<String> = missing_entries
                .iter()
                .map(|(_, n)| n.to_uppercase())
                .collect();

            let records: Vec<(
                String,
                String,
                Option<String>,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
            )> = sqlx::query_as(
                r#"
                SELECT ticker, name, name_en, market, exchange, sector, yahoo_symbol
                FROM symbol_info
                WHERE UPPER(ticker) = ANY($1) AND is_active = true
                "#,
            )
            .bind(&normalized_upper)
            .fetch_all(&self.pool)
            .await?;

            // 조회된 결과 캐시 및 결과에 추가
            let mut cache = self.cache.write().await;
            let mut found_normalized = std::collections::HashSet::new();

            for (ticker, name, name_en, market, exchange, sector, yahoo_symbol) in records {
                let meta = SymbolMetadata {
                    ticker: ticker.clone(),
                    name,
                    name_en,
                    market,
                    exchange,
                    sector,
                    yahoo_symbol,
                };

                let display_name = meta.to_display_string(use_english);
                let key = ticker.to_uppercase();
                found_normalized.insert(key.clone());
                cache.insert(key, meta);

                // 원래 심볼과 매칭
                for (orig_symbol, normalized) in &missing_entries {
                    if normalized.to_uppercase() == ticker.to_uppercase() {
                        result.insert(orig_symbol.clone(), display_name.clone());
                    }
                }
            }

            // 3. DB에도 없는 심볼은 자동 생성
            drop(cache); // write lock 해제

            for (orig_symbol, normalized) in &missing_entries {
                if !found_normalized.contains(&normalized.to_uppercase()) {
                    // 자동 생성
                    let meta = Self::build_symbol_metadata(normalized);
                    let _ = self.save_symbol_info(&meta).await; // 저장 실패해도 계속 진행

                    let display_name = meta.to_display_string(use_english);
                    result.insert(orig_symbol.clone(), display_name);

                    // 캐시에 저장
                    let mut cache = self.cache.write().await;
                    cache.insert(normalized.to_uppercase(), meta);
                }
            }
        }

        Ok(result)
    }

    /// 표시 문자열 생성: "티커(종목명)".
    ///
    /// # Arguments
    /// * `canonical` - 중립 심볼 (예: "005930")
    /// * `use_english` - 영문명 사용 여부
    ///
    /// # Returns
    /// "005930(삼성전자)" 또는 "005930(Samsung Electronics)"
    /// 표시 문자열 생성: "티커(종목명)".
    ///
    /// 심볼 정보가 없으면 자동으로 생성하여 DB에 저장합니다.
    ///
    /// # Arguments
    /// * `symbol` - 심볼 (canonical 또는 데이터 소스별 심볼)
    /// * `data_source` - 데이터 소스 (None이면 canonical)
    /// * `use_english` - 영문명 사용 여부
    pub async fn to_display_string(
        &self,
        symbol: &str,
        use_english: bool,
    ) -> Result<String, sqlx::Error> {
        let info = self.get_or_create_symbol_info(symbol).await?;
        Ok(info.to_display_string(use_english))
    }

    /// 통합 심볼 검색.
    ///
    /// 티커, 종목명, 영문명, 모든 Alias에서 검색합니다.
    ///
    /// # Arguments
    /// * `query` - 검색어
    /// * `limit` - 최대 결과 수
    ///
    /// # Returns
    /// 매칭된 심볼 메타데이터 목록
    pub async fn search(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<SymbolMetadata>, sqlx::Error> {
        let query_upper = query.to_uppercase();
        let query_pattern = format!("%{}%", query_upper);

        let results: Vec<(
            String,
            String,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(
            r#"
            SELECT ticker, name, name_en, market, exchange, sector, yahoo_symbol
            FROM symbol_info
            WHERE is_active = true
              AND (
                  UPPER(ticker) LIKE $1
                  OR UPPER(name) LIKE $1
                  OR UPPER(COALESCE(name_en, '')) LIKE $1
                  OR UPPER(COALESCE(yahoo_symbol, '')) LIKE $1
              )
            ORDER BY
                CASE WHEN UPPER(ticker) = $2 THEN 0
                     WHEN UPPER(ticker) LIKE $3 THEN 1
                     WHEN UPPER(yahoo_symbol) = $2 THEN 2
                     ELSE 3
                END,
                ticker
            LIMIT $4
            "#,
        )
        .bind(&query_pattern)
        .bind(&query_upper)
        .bind(format!("{}%", query_upper))
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(results
            .into_iter()
            .map(
                |(ticker, name, name_en, market, exchange, sector, yahoo_symbol)| SymbolMetadata {
                    ticker,
                    name,
                    name_en,
                    market,
                    exchange,
                    sector,
                    yahoo_symbol,
                },
            )
            .collect())
    }
}

impl SymbolMetadata {
    /// 표시 문자열 생성: "티커(종목명)".
    pub fn to_display_string(&self, use_english: bool) -> String {
        let name = if use_english {
            self.name_en.as_ref().unwrap_or(&self.name)
        } else {
            &self.name
        };
        format!("{}({})", self.ticker, name)
    }

    /// 데이터 소스별 심볼 반환.
    pub fn get_source_symbol(&self, source: &str) -> Option<&str> {
        match source.to_lowercase().as_str() {
            "yahoo" => self.yahoo_symbol.as_deref(),
            "kis" | "krx" => Some(&self.ticker),
            _ => Some(&self.ticker),
        }
    }
}
