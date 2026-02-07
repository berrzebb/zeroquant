//! KRX Open API 클라이언트.
//!
//! 한국거래소(KRX) Open API를 통해 주식 데이터를 수집합니다.
//! Yahoo Finance 의존성을 대체하여 국내 주식 데이터를 직접 수집합니다.
//!
//! # 지원 데이터
//!
//! - 종목 기본 정보 (시가총액, 발행주식수)
//! - 가격 지표 (PER, PBR, 배당수익률)
//! - OHLCV 일별 시세
//! - ETF 정보
//!
//! # API 키 관리
//!
//! KRX API 키는 `exchange_credentials` 시스템을 통해 암호화되어 관리됩니다:
//! - `exchange_id = 'krx'`
//! - `encrypted_credentials` = `{"api_key": "YOUR_AUTH_KEY"}`
//!
//! UI에서 credential 등록 시 자동으로 암호화됩니다.
//!
//! # 사용 예제
//!
//! ```rust,ignore
//! use trader_data::provider::krx_api::KrxApiClient;
//! use trader_core::CredentialEncryptor;
//! use sqlx::PgPool;
//!
//! // Credential 시스템에서 API 키를 읽어 클라이언트 생성 (권장)
//! let client = KrxApiClient::from_credential(&pool, &encryptor).await?;
//!
//! // 직접 API 키 지정 (테스트용)
//! let client = KrxApiClient::new("YOUR_AUTH_KEY");
//!
//! let stocks = client.fetch_kospi_stocks("20240101").await?;
//! ```

use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use trader_core::CredentialEncryptor;

/// KRX Open API 클라이언트.
#[derive(Clone)]
pub struct KrxApiClient {
    client: reqwest::Client,
    auth_key: String,
    base_url: String,
}

/// KRX 종목 기본 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrxStockInfo {
    /// 단축코드 (6자리)
    pub ticker: String,
    /// 종목명 (한글)
    pub name: String,
    /// 영문명
    pub name_en: Option<String>,
    /// 시장 (STK: KOSPI, KSQ: KOSDAQ)
    pub market: String,
    /// 업종
    pub sector: Option<String>,
    /// 시가총액 (억원)
    pub market_cap: Option<Decimal>,
    /// 발행주식수
    pub shares_outstanding: Option<i64>,
    /// 액면가
    pub par_value: Option<Decimal>,
    /// 상장일
    pub listing_date: Option<NaiveDate>,
}

/// KRX 가격 지표.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KrxValuation {
    /// 티커
    pub ticker: String,
    /// PER (주가수익비율)
    pub per: Option<Decimal>,
    /// PBR (주가순자산비율)
    pub pbr: Option<Decimal>,
    /// 배당수익률 (%)
    pub dividend_yield: Option<Decimal>,
    /// EPS (주당순이익)
    pub eps: Option<Decimal>,
    /// BPS (주당순자산)
    pub bps: Option<Decimal>,
}

/// KRX OHLCV 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrxOhlcv {
    /// 일자
    pub date: NaiveDate,
    /// 시가
    pub open: Decimal,
    /// 고가
    pub high: Decimal,
    /// 저가
    pub low: Decimal,
    /// 종가
    pub close: Decimal,
    /// 거래량
    pub volume: i64,
    /// 거래대금
    pub trading_value: Option<Decimal>,
}

/// KRX ETF 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrxEtfInfo {
    /// 단축코드
    pub ticker: String,
    /// 종목명
    pub name: String,
    /// 기초지수명
    pub underlying_index: Option<String>,
    /// 운용사
    pub issuer: Option<String>,
    /// 순자산가치 (NAV)
    pub nav: Option<Decimal>,
    /// 괴리율 (%)
    pub tracking_error: Option<Decimal>,
    /// 종가
    pub close: Option<Decimal>,
    /// 시가총액
    pub market_cap: Option<Decimal>,
    /// 거래량
    pub volume: Option<i64>,
}

/// KRX 지수 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrxIndexInfo {
    /// 기준일자
    pub date: NaiveDate,
    /// 계열구분 (KRX, KOSPI, KOSDAQ 등)
    pub index_class: String,
    /// 지수명
    pub index_name: String,
    /// 종가
    pub close: Decimal,
    /// 전일대비
    pub change: Option<Decimal>,
    /// 등락률 (%)
    pub change_rate: Option<Decimal>,
    /// 시가
    pub open: Option<Decimal>,
    /// 고가
    pub high: Option<Decimal>,
    /// 저가
    pub low: Option<Decimal>,
    /// 거래량
    pub volume: Option<i64>,
    /// 거래대금
    pub trading_value: Option<Decimal>,
    /// 상장시가총액
    pub market_cap: Option<Decimal>,
}

/// KRX 일별 매매정보 (전종목).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrxDailyTrade {
    /// 기준일자
    pub date: NaiveDate,
    /// 종목코드 (표준코드)
    pub code: String,
    /// 종목명
    pub name: String,
    /// 시장구분 (유가증권시장, 코스닥 등)
    pub market: String,
    /// 소속부 (섹터)
    pub sector: Option<String>,
    /// 종가
    pub close: Decimal,
    /// 전일대비
    pub change: Option<Decimal>,
    /// 등락률 (%)
    pub change_rate: Option<Decimal>,
    /// 시가
    pub open: Option<Decimal>,
    /// 고가
    pub high: Option<Decimal>,
    /// 저가
    pub low: Option<Decimal>,
    /// 거래량
    pub volume: i64,
    /// 거래대금
    pub trading_value: Option<Decimal>,
    /// 시가총액
    pub market_cap: Option<Decimal>,
    /// 상장주식수
    pub shares_outstanding: Option<i64>,
}

/// KRX ETN 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrxEtnInfo {
    /// 단축코드
    pub ticker: String,
    /// 종목명
    pub name: String,
    /// 기초지수명
    pub underlying_index: Option<String>,
    /// 지표가치 (IV)
    pub indicative_value: Option<Decimal>,
    /// 종가
    pub close: Option<Decimal>,
    /// 시가총액
    pub market_cap: Option<Decimal>,
    /// 거래량
    pub volume: Option<i64>,
}

/// API 응답 래퍼.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ApiResponse<T> {
    #[serde(rename = "OutBlock_1")]
    out_block: Option<Vec<T>>,
    #[serde(rename = "CURRENT_DATETIME", default)]
    current_datetime: Option<String>,
}

/// KRX credential 구조 (복호화 후).
#[derive(Debug, Deserialize)]
struct KrxCredentials {
    api_key: String,
}

/// DB에서 조회한 credential row.
#[derive(sqlx::FromRow)]
struct CredentialRow {
    encrypted_credentials: Vec<u8>,
    encryption_nonce: Vec<u8>,
}

/// API 카테고리별 URL 경로.
#[derive(Debug, Clone, Copy)]
pub enum ApiCategory {
    /// 지수 (idx)
    Index,
    /// 주식 (stk)
    Stock,
    /// 증권상품 - ETF, ETN, ELW (etp)
    Etp,
    /// 채권 (bnd)
    Bond,
    /// 파생상품 (drv)
    Derivative,
    /// 일반상품 - 금, 석유, 배출권 (gen)
    General,
    /// ESG (esg)
    Esg,
}

impl ApiCategory {
    /// 카테고리별 URL 경로 반환.
    fn path(&self) -> &'static str {
        match self {
            ApiCategory::Index => "idx",
            ApiCategory::Stock => "sto",
            ApiCategory::Etp => "etp",
            ApiCategory::Bond => "bnd",
            ApiCategory::Derivative => "drv",
            ApiCategory::General => "gen",
            ApiCategory::Esg => "esg",
        }
    }
}

impl KrxApiClient {
    /// 새로운 KRX API 클라이언트 생성.
    ///
    /// # Arguments
    /// * `auth_key` - KRX Open API 인증키
    ///
    /// # Note
    /// 직접 API 키를 하드코딩하지 마세요.
    /// `from_credential()` 또는 `from_env()`를 사용하세요.
    pub fn new(auth_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("HTTP 클라이언트 생성 실패"),
            auth_key: auth_key.into(),
            // KRX OPEN API Base URL (테스트 확인됨)
            base_url: "https://data-dbg.krx.co.kr".to_string(),
        }
    }

    /// Credential 시스템에서 API 키를 읽어 클라이언트 생성 (권장).
    ///
    /// `exchange_credentials` 테이블에서 `exchange_id = 'krx'`인
    /// 암호화된 credential을 복호화하여 API 키를 읽습니다.
    ///
    /// # Arguments
    /// * `pool` - PostgreSQL 연결 풀
    /// * `encryptor` - Credential 암호화/복호화 관리자
    ///
    /// # Returns
    /// - `Ok(Some(client))`: credential이 등록되어 있고 클라이언트 생성 성공
    /// - `Ok(None)`: credential이 등록되지 않음
    /// - `Err(...)`: DB 조회 또는 복호화 실패
    ///
    /// # Example
    /// ```rust,ignore
    /// let client = KrxApiClient::from_credential(&pool, &encryptor).await?
    ///     .ok_or("KRX API credential이 등록되지 않았습니다")?;
    /// ```
    pub async fn from_credential(
        pool: &PgPool,
        encryptor: &CredentialEncryptor,
    ) -> Result<Option<Self>, String> {
        // 1. DB에서 KRX credential 조회
        let result: Option<CredentialRow> = sqlx::query_as(
            r#"
            SELECT encrypted_credentials, encryption_nonce
            FROM exchange_credentials
            WHERE exchange_id = 'krx' AND is_active = true
            LIMIT 1
            "#,
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("KRX credential 조회 실패: {}", e))?;

        if let Some(row) = result {
            // 2. 복호화
            let credentials: KrxCredentials = encryptor
                .decrypt_json(&row.encrypted_credentials, &row.encryption_nonce)
                .map_err(|e| format!("KRX credential 복호화 실패: {}", e))?;

            tracing::info!("KRX API 키 로드: credential 시스템에서 로드됨");
            return Ok(Some(Self::new(credentials.api_key)));
        }

        // 3. 환경변수 폴백
        if let Ok(api_key) = std::env::var("KRX_API_KEY") {
            tracing::info!("KRX API 키 로드: 환경변수에서 로드됨 (폴백)");
            return Ok(Some(Self::new(api_key)));
        }

        tracing::warn!(
            "KRX API credential이 등록되지 않았습니다. \
            Settings에서 KRX API 키를 등록하세요."
        );
        Ok(None)
    }

    /// 환경변수에서 인증키를 로드하여 클라이언트 생성 (폴백용).
    ///
    /// 환경변수 `KRX_API_KEY`에서 인증키를 읽습니다.
    /// 가능하면 `from_credential()`를 사용하세요.
    pub fn from_env() -> Option<Self> {
        std::env::var("KRX_API_KEY").ok().map(Self::new)
    }

    /// API 요청 실행 (카테고리 지정).
    ///
    /// AUTH_KEY는 HTTP 헤더로 전달합니다 (KRX OPEN API 명세 준수).
    async fn request_with_category<T: for<'de> Deserialize<'de>>(
        &self,
        category: ApiCategory,
        api_id: &str,
        params: &HashMap<&str, &str>,
    ) -> Result<Vec<T>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/svc/apis/{}/{}",
            self.base_url,
            category.path(),
            api_id
        );

        tracing::debug!(
            api_id = api_id,
            category = ?category,
            url = %url,
            "KRX API 요청"
        );

        let response = self
            .client
            .get(&url)
            .query(params)
            // AUTH_KEY를 HTTP 헤더로 전달 (명세 준수)
            .header("AUTH_KEY", &self.auth_key)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("KRX API 오류 [{}]: {} - {}", api_id, status, body).into());
        }

        let data: ApiResponse<T> = response.json().await?;

        Ok(data.out_block.unwrap_or_default())
    }

    /// 기존 호환성을 위한 API 요청 (주식 카테고리 기본값).
    async fn request<T: for<'de> Deserialize<'de>>(
        &self,
        api_id: &str,
        params: &HashMap<&str, &str>,
    ) -> Result<Vec<T>, Box<dyn std::error::Error + Send + Sync>> {
        self.request_with_category(ApiCategory::Stock, api_id, params)
            .await
    }

    /// KOSPI 종목 기본 정보 조회.
    ///
    /// # Arguments
    /// * `base_date` - 기준일 (YYYYMMDD 형식)
    pub async fn fetch_kospi_stocks(
        &self,
        base_date: &str,
    ) -> Result<Vec<KrxStockInfo>, Box<dyn std::error::Error + Send + Sync>> {
        #[derive(Deserialize)]
        struct RawStock {
            #[serde(rename = "ISU_SRT_CD")]
            ticker: String,
            #[serde(rename = "ISU_ABBRV")]
            name: String,
            #[serde(rename = "ISU_ENG_NM", default)]
            name_en: Option<String>,
            #[serde(rename = "SECT_TP_NM", default)]
            sector: Option<String>,
            #[serde(rename = "MKTCAP", default)]
            market_cap: Option<String>,
            #[serde(rename = "LIST_SHRS", default)]
            shares: Option<String>,
            #[serde(rename = "PARVAL", default)]
            par_value: Option<String>,
            #[serde(rename = "LIST_DD", default)]
            listing_date: Option<String>,
        }

        let params: HashMap<&str, &str> = [("basDd", base_date)].into_iter().collect();
        let raw_stocks: Vec<RawStock> = self.request("stk_isu_base_info", &params).await?;

        let stocks: Vec<KrxStockInfo> = raw_stocks
            .into_iter()
            .map(|s| KrxStockInfo {
                ticker: s.ticker,
                name: s.name,
                name_en: s.name_en,
                market: "STK".to_string(),
                sector: s.sector,
                market_cap: parse_decimal_opt(&s.market_cap),
                shares_outstanding: s
                    .shares
                    .as_ref()
                    .and_then(|v| v.replace(",", "").parse().ok()),
                par_value: parse_decimal_opt(&s.par_value),
                listing_date: s
                    .listing_date
                    .as_ref()
                    .and_then(|d| NaiveDate::parse_from_str(d, "%Y/%m/%d").ok()),
            })
            .collect();

        tracing::info!(count = stocks.len(), "KOSPI 종목 조회 완료");
        Ok(stocks)
    }

    /// KOSDAQ 종목 기본 정보 조회.
    ///
    /// # Arguments
    /// * `base_date` - 기준일 (YYYYMMDD 형식)
    pub async fn fetch_kosdaq_stocks(
        &self,
        base_date: &str,
    ) -> Result<Vec<KrxStockInfo>, Box<dyn std::error::Error + Send + Sync>> {
        #[derive(Deserialize)]
        struct RawStock {
            #[serde(rename = "ISU_SRT_CD")]
            ticker: String,
            #[serde(rename = "ISU_ABBRV")]
            name: String,
            #[serde(rename = "ISU_ENG_NM", default)]
            name_en: Option<String>,
            #[serde(rename = "SECT_TP_NM", default)]
            sector: Option<String>,
            #[serde(rename = "MKTCAP", default)]
            market_cap: Option<String>,
            #[serde(rename = "LIST_SHRS", default)]
            shares: Option<String>,
            #[serde(rename = "PARVAL", default)]
            par_value: Option<String>,
            #[serde(rename = "LIST_DD", default)]
            listing_date: Option<String>,
        }

        let params: HashMap<&str, &str> = [("basDd", base_date)].into_iter().collect();
        let raw_stocks: Vec<RawStock> = self.request("ksq_isu_base_info", &params).await?;

        let stocks: Vec<KrxStockInfo> = raw_stocks
            .into_iter()
            .map(|s| KrxStockInfo {
                ticker: s.ticker,
                name: s.name,
                name_en: s.name_en,
                market: "KSQ".to_string(),
                sector: s.sector,
                market_cap: parse_decimal_opt(&s.market_cap),
                shares_outstanding: s
                    .shares
                    .as_ref()
                    .and_then(|v| v.replace(",", "").parse().ok()),
                par_value: parse_decimal_opt(&s.par_value),
                listing_date: s
                    .listing_date
                    .as_ref()
                    .and_then(|d| NaiveDate::parse_from_str(d, "%Y/%m/%d").ok()),
            })
            .collect();

        tracing::info!(count = stocks.len(), "KOSDAQ 종목 조회 완료");
        Ok(stocks)
    }

    /// ETF 전종목 정보 조회.
    ///
    /// # Arguments
    /// * `base_date` - 기준일 (YYYYMMDD 형식)
    ///
    /// API: etf_bydd_trd (증권상품 카테고리)
    pub async fn fetch_etfs(
        &self,
        base_date: &str,
    ) -> Result<Vec<KrxEtfInfo>, Box<dyn std::error::Error + Send + Sync>> {
        #[derive(Deserialize)]
        struct RawEtf {
            #[serde(rename = "ISU_CD", default)]
            ticker: Option<String>,
            #[serde(rename = "ISU_NM")]
            name: String,
            #[serde(rename = "IDX_IND_NM", default)]
            underlying_index: Option<String>,
            #[serde(rename = "NAV", default)]
            nav: Option<String>,
            #[serde(rename = "TDD_CLSPRC", default)]
            close: Option<String>,
            #[serde(rename = "MKTCAP", default)]
            market_cap: Option<String>,
            #[serde(rename = "ACC_TRDVOL", default)]
            volume: Option<String>,
            #[serde(rename = "FLUC_RT", default)]
            tracking_error: Option<String>,
        }

        let params: HashMap<&str, &str> = [("basDd", base_date)].into_iter().collect();
        // 증권상품(ETP) 카테고리 사용
        let raw_etfs: Vec<RawEtf> = self
            .request_with_category(ApiCategory::Etp, "etf_bydd_trd", &params)
            .await?;

        let etfs: Vec<KrxEtfInfo> = raw_etfs
            .into_iter()
            .filter_map(|e| {
                Some(KrxEtfInfo {
                    ticker: e.ticker?,
                    name: e.name,
                    underlying_index: e.underlying_index,
                    issuer: None, // ETF 일별매매정보에서는 운용사 정보 미제공
                    nav: parse_decimal_opt(&e.nav),
                    tracking_error: parse_decimal_opt(&e.tracking_error),
                    close: parse_decimal_opt(&e.close),
                    market_cap: parse_decimal_opt(&e.market_cap),
                    volume: e
                        .volume
                        .as_ref()
                        .and_then(|v| v.replace(",", "").parse().ok()),
                })
            })
            .collect();

        tracing::info!(count = etfs.len(), "ETF 종목 조회 완료");
        Ok(etfs)
    }

    /// 종목별 PER/PBR 조회.
    ///
    /// # Arguments
    /// * `base_date` - 기준일 (YYYYMMDD 형식)
    /// * `market` - 시장 (STK: KOSPI, KSQ: KOSDAQ)
    pub async fn fetch_valuation(
        &self,
        base_date: &str,
        market: &str,
    ) -> Result<Vec<KrxValuation>, Box<dyn std::error::Error + Send + Sync>> {
        #[derive(Deserialize)]
        struct RawValuation {
            #[serde(rename = "ISU_SRT_CD")]
            ticker: String,
            #[serde(rename = "PER", default)]
            per: Option<String>,
            #[serde(rename = "PBR", default)]
            pbr: Option<String>,
            #[serde(rename = "DVD_YLD", default)]
            dividend_yield: Option<String>,
            #[serde(rename = "EPS", default)]
            eps: Option<String>,
            #[serde(rename = "BPS", default)]
            bps: Option<String>,
        }

        // 시장에 따라 다른 API 사용
        let api_id = match market {
            "STK" => "stk_isu_per_pbr",
            "KSQ" => "ksq_isu_per_pbr",
            _ => return Err(format!("지원하지 않는 시장: {}", market).into()),
        };

        let params: HashMap<&str, &str> = [("basDd", base_date)].into_iter().collect();
        let raw_valuations: Vec<RawValuation> = self.request(api_id, &params).await?;

        let valuations: Vec<KrxValuation> = raw_valuations
            .into_iter()
            .map(|v| KrxValuation {
                ticker: v.ticker,
                per: parse_decimal_opt(&v.per),
                pbr: parse_decimal_opt(&v.pbr),
                dividend_yield: parse_decimal_opt(&v.dividend_yield),
                eps: parse_decimal_opt(&v.eps),
                bps: parse_decimal_opt(&v.bps),
            })
            .collect();

        tracing::info!(
            market = market,
            count = valuations.len(),
            "가치지표 조회 완료"
        );
        Ok(valuations)
    }

    /// 일별 시세 조회 (개별 종목).
    ///
    /// # Arguments
    /// * `ticker` - 종목코드
    /// * `start_date` - 시작일 (YYYYMMDD)
    /// * `end_date` - 종료일 (YYYYMMDD)
    pub async fn fetch_daily_ohlcv(
        &self,
        ticker: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<KrxOhlcv>, Box<dyn std::error::Error + Send + Sync>> {
        #[derive(Deserialize)]
        struct RawOhlcv {
            #[serde(rename = "TRD_DD")]
            date: String,
            #[serde(rename = "TDD_OPNPRC", default)]
            open: Option<String>,
            #[serde(rename = "TDD_HGPRC", default)]
            high: Option<String>,
            #[serde(rename = "TDD_LWPRC", default)]
            low: Option<String>,
            #[serde(rename = "TDD_CLSPRC", default)]
            close: Option<String>,
            #[serde(rename = "ACC_TRDVOL", default)]
            volume: Option<String>,
            #[serde(rename = "ACC_TRDVAL", default)]
            trading_value: Option<String>,
        }

        let params: HashMap<&str, &str> = [
            ("isuCd", ticker),
            ("strtDd", start_date),
            ("endDd", end_date),
        ]
        .into_iter()
        .collect();

        let raw_ohlcvs: Vec<RawOhlcv> = self.request("stk_isu_ohlcv", &params).await?;

        let ohlcvs: Vec<KrxOhlcv> = raw_ohlcvs
            .into_iter()
            .filter_map(|o| {
                let date = NaiveDate::parse_from_str(&o.date, "%Y/%m/%d").ok()?;
                Some(KrxOhlcv {
                    date,
                    open: parse_decimal_opt(&o.open).unwrap_or_default(),
                    high: parse_decimal_opt(&o.high).unwrap_or_default(),
                    low: parse_decimal_opt(&o.low).unwrap_or_default(),
                    close: parse_decimal_opt(&o.close).unwrap_or_default(),
                    volume: o
                        .volume
                        .as_ref()
                        .and_then(|v| v.replace(",", "").parse().ok())
                        .unwrap_or(0),
                    trading_value: parse_decimal_opt(&o.trading_value),
                })
            })
            .collect();

        tracing::debug!(ticker = ticker, count = ohlcvs.len(), "일별 시세 조회 완료");
        Ok(ohlcvs)
    }

    /// 전종목 일별 시세 조회.
    ///
    /// # Arguments
    /// * `base_date` - 기준일 (YYYYMMDD)
    /// * `market` - 시장 (STK: KOSPI, KSQ: KOSDAQ)
    pub async fn fetch_market_ohlcv(
        &self,
        base_date: &str,
        market: &str,
    ) -> Result<Vec<(String, KrxOhlcv)>, Box<dyn std::error::Error + Send + Sync>> {
        #[derive(Deserialize)]
        struct RawMarketOhlcv {
            #[serde(rename = "ISU_SRT_CD")]
            ticker: String,
            #[serde(rename = "TRD_DD")]
            date: String,
            #[serde(rename = "TDD_OPNPRC", default)]
            open: Option<String>,
            #[serde(rename = "TDD_HGPRC", default)]
            high: Option<String>,
            #[serde(rename = "TDD_LWPRC", default)]
            low: Option<String>,
            #[serde(rename = "TDD_CLSPRC", default)]
            close: Option<String>,
            #[serde(rename = "ACC_TRDVOL", default)]
            volume: Option<String>,
            #[serde(rename = "ACC_TRDVAL", default)]
            trading_value: Option<String>,
        }

        // 시장에 따라 다른 API 사용
        let api_id = match market {
            "STK" => "stk_bydd_trd",
            "KSQ" => "ksq_bydd_trd",
            _ => return Err(format!("지원하지 않는 시장: {}", market).into()),
        };

        let params: HashMap<&str, &str> = [("basDd", base_date)].into_iter().collect();
        let raw_ohlcvs: Vec<RawMarketOhlcv> = self.request(api_id, &params).await?;

        let results = raw_ohlcvs
            .into_iter()
            .filter_map(|o| {
                let date = NaiveDate::parse_from_str(&o.date, "%Y/%m/%d").ok()?;
                Some((
                    o.ticker,
                    KrxOhlcv {
                        date,
                        open: parse_decimal_opt(&o.open).unwrap_or_default(),
                        high: parse_decimal_opt(&o.high).unwrap_or_default(),
                        low: parse_decimal_opt(&o.low).unwrap_or_default(),
                        close: parse_decimal_opt(&o.close).unwrap_or_default(),
                        volume: o
                            .volume
                            .as_ref()
                            .and_then(|v| v.replace(",", "").parse().ok())
                            .unwrap_or(0),
                        trading_value: parse_decimal_opt(&o.trading_value),
                    },
                ))
            })
            .collect();

        tracing::info!(
            market = market,
            base_date = base_date,
            "전종목 시세 조회 완료"
        );
        Ok(results)
    }

    /// 모든 종목 기본 정보 조회 (KOSPI + KOSDAQ + ETF).
    pub async fn fetch_all_stocks(
        &self,
    ) -> Result<Vec<KrxStockInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let today = Utc::now().format("%Y%m%d").to_string();

        // 병렬로 KOSPI, KOSDAQ 조회
        let (kospi_result, kosdaq_result) = tokio::join!(
            self.fetch_kospi_stocks(&today),
            self.fetch_kosdaq_stocks(&today),
        );

        let mut all = kospi_result?;
        all.extend(kosdaq_result?);

        tracing::info!(total = all.len(), "전종목 기본 정보 조회 완료");
        Ok(all)
    }

    // ========================================================================
    // 새로운 KRX OPEN API 메서드들
    // ========================================================================

    /// KRX 시리즈 지수 조회.
    ///
    /// API: krx_dd_trd (지수 카테고리)
    pub async fn fetch_krx_index(
        &self,
        base_date: &str,
    ) -> Result<Vec<KrxIndexInfo>, Box<dyn std::error::Error + Send + Sync>> {
        self.fetch_index_internal("krx_dd_trd", base_date).await
    }

    /// KOSPI 시리즈 지수 조회.
    ///
    /// API: kospi_dd_trd (지수 카테고리)
    pub async fn fetch_kospi_index(
        &self,
        base_date: &str,
    ) -> Result<Vec<KrxIndexInfo>, Box<dyn std::error::Error + Send + Sync>> {
        self.fetch_index_internal("kospi_dd_trd", base_date).await
    }

    /// KOSDAQ 시리즈 지수 조회.
    ///
    /// API: kosdaq_dd_trd (지수 카테고리)
    pub async fn fetch_kosdaq_index(
        &self,
        base_date: &str,
    ) -> Result<Vec<KrxIndexInfo>, Box<dyn std::error::Error + Send + Sync>> {
        self.fetch_index_internal("kosdaq_dd_trd", base_date).await
    }

    /// 지수 조회 내부 구현.
    async fn fetch_index_internal(
        &self,
        api_id: &str,
        base_date: &str,
    ) -> Result<Vec<KrxIndexInfo>, Box<dyn std::error::Error + Send + Sync>> {
        #[derive(Deserialize)]
        struct RawIndex {
            #[serde(rename = "BAS_DD")]
            date: String,
            #[serde(rename = "IDX_CLSS", default)]
            index_class: Option<String>,
            #[serde(rename = "IDX_NM")]
            index_name: String,
            #[serde(rename = "CLSPRC_IDX")]
            close: String,
            #[serde(rename = "CMPPREVDD_IDX", default)]
            change: Option<String>,
            #[serde(rename = "FLUC_RT", default)]
            change_rate: Option<String>,
            #[serde(rename = "OPNPRC_IDX", default)]
            open: Option<String>,
            #[serde(rename = "HGPRC_IDX", default)]
            high: Option<String>,
            #[serde(rename = "LWPRC_IDX", default)]
            low: Option<String>,
            #[serde(rename = "ACC_TRDVOL", default)]
            volume: Option<String>,
            #[serde(rename = "ACC_TRDVAL", default)]
            trading_value: Option<String>,
            #[serde(rename = "MKTCAP", default)]
            market_cap: Option<String>,
        }

        let params: HashMap<&str, &str> = [("basDd", base_date)].into_iter().collect();
        let raw_indices: Vec<RawIndex> = self
            .request_with_category(ApiCategory::Index, api_id, &params)
            .await?;

        let indices: Vec<KrxIndexInfo> = raw_indices
            .into_iter()
            .filter_map(|i| {
                let date = parse_date_yyyymmdd(&i.date)?;
                let close = i.close.replace(",", "").parse().ok()?;
                Some(KrxIndexInfo {
                    date,
                    index_class: i.index_class.unwrap_or_default(),
                    index_name: i.index_name,
                    close,
                    change: parse_decimal_opt(&i.change),
                    change_rate: parse_decimal_opt(&i.change_rate),
                    open: parse_decimal_opt(&i.open),
                    high: parse_decimal_opt(&i.high),
                    low: parse_decimal_opt(&i.low),
                    volume: i
                        .volume
                        .as_ref()
                        .and_then(|v| v.replace(",", "").parse().ok()),
                    trading_value: parse_decimal_opt(&i.trading_value),
                    market_cap: parse_decimal_opt(&i.market_cap),
                })
            })
            .collect();

        tracing::info!(api_id = api_id, count = indices.len(), "지수 조회 완료");
        Ok(indices)
    }

    /// 유가증권 전종목 일별 매매정보 조회 (섹터 포함).
    ///
    /// API: stk_bydd_trd (주식 카테고리)
    pub async fn fetch_kospi_daily_trades(
        &self,
        base_date: &str,
    ) -> Result<Vec<KrxDailyTrade>, Box<dyn std::error::Error + Send + Sync>> {
        self.fetch_daily_trades_internal("stk_bydd_trd", "유가증권시장", base_date)
            .await
    }

    /// 코스닥 전종목 일별 매매정보 조회 (섹터 포함).
    ///
    /// API: ksq_bydd_trd (주식 카테고리)
    pub async fn fetch_kosdaq_daily_trades(
        &self,
        base_date: &str,
    ) -> Result<Vec<KrxDailyTrade>, Box<dyn std::error::Error + Send + Sync>> {
        self.fetch_daily_trades_internal("ksq_bydd_trd", "코스닥시장", base_date)
            .await
    }

    /// 코넥스 전종목 일별 매매정보 조회.
    ///
    /// API: knx_bydd_trd (주식 카테고리)
    pub async fn fetch_konex_daily_trades(
        &self,
        base_date: &str,
    ) -> Result<Vec<KrxDailyTrade>, Box<dyn std::error::Error + Send + Sync>> {
        self.fetch_daily_trades_internal("knx_bydd_trd", "코넥스시장", base_date)
            .await
    }

    /// 일별 매매정보 조회 내부 구현.
    async fn fetch_daily_trades_internal(
        &self,
        api_id: &str,
        default_market: &str,
        base_date: &str,
    ) -> Result<Vec<KrxDailyTrade>, Box<dyn std::error::Error + Send + Sync>> {
        #[derive(Deserialize)]
        struct RawDailyTrade {
            #[serde(rename = "BAS_DD")]
            date: String,
            #[serde(rename = "ISU_CD")]
            code: String,
            #[serde(rename = "ISU_NM")]
            name: String,
            #[serde(rename = "MKT_NM", default)]
            market: Option<String>,
            #[serde(rename = "SECT_TP_NM", default)]
            sector: Option<String>,
            #[serde(rename = "TDD_CLSPRC")]
            close: String,
            #[serde(rename = "CMPPREVDD_PRC", default)]
            change: Option<String>,
            #[serde(rename = "FLUC_RT", default)]
            change_rate: Option<String>,
            #[serde(rename = "TDD_OPNPRC", default)]
            open: Option<String>,
            #[serde(rename = "TDD_HGPRC", default)]
            high: Option<String>,
            #[serde(rename = "TDD_LWPRC", default)]
            low: Option<String>,
            #[serde(rename = "ACC_TRDVOL")]
            volume: String,
            #[serde(rename = "ACC_TRDVAL", default)]
            trading_value: Option<String>,
            #[serde(rename = "MKTCAP", default)]
            market_cap: Option<String>,
            #[serde(rename = "LIST_SHRS", default)]
            shares: Option<String>,
        }

        let params: HashMap<&str, &str> = [("basDd", base_date)].into_iter().collect();
        let raw_trades: Vec<RawDailyTrade> = self.request(api_id, &params).await?;

        let trades: Vec<KrxDailyTrade> = raw_trades
            .into_iter()
            .filter_map(|t| {
                let date = parse_date_yyyymmdd(&t.date)?;
                let close = t.close.replace(",", "").parse().ok()?;
                let volume = t.volume.replace(",", "").parse().ok()?;
                Some(KrxDailyTrade {
                    date,
                    code: t.code,
                    name: t.name,
                    market: t.market.unwrap_or_else(|| default_market.to_string()),
                    sector: t.sector,
                    close,
                    change: parse_decimal_opt(&t.change),
                    change_rate: parse_decimal_opt(&t.change_rate),
                    open: parse_decimal_opt(&t.open),
                    high: parse_decimal_opt(&t.high),
                    low: parse_decimal_opt(&t.low),
                    volume,
                    trading_value: parse_decimal_opt(&t.trading_value),
                    market_cap: parse_decimal_opt(&t.market_cap),
                    shares_outstanding: t
                        .shares
                        .as_ref()
                        .and_then(|v| v.replace(",", "").parse().ok()),
                })
            })
            .collect();

        tracing::info!(
            api_id = api_id,
            base_date = base_date,
            count = trades.len(),
            "일별 매매정보 조회 완료"
        );
        Ok(trades)
    }

    /// ETN 전종목 정보 조회.
    ///
    /// API: etn_bydd_trd (증권상품 카테고리)
    pub async fn fetch_etns(
        &self,
        base_date: &str,
    ) -> Result<Vec<KrxEtnInfo>, Box<dyn std::error::Error + Send + Sync>> {
        #[derive(Deserialize)]
        struct RawEtn {
            #[serde(rename = "ISU_CD", default)]
            ticker: Option<String>,
            #[serde(rename = "ISU_NM")]
            name: String,
            #[serde(rename = "IDX_IND_NM", default)]
            underlying_index: Option<String>,
            #[serde(rename = "PER1SECU_INDIC_VAL", default)]
            indicative_value: Option<String>,
            #[serde(rename = "TDD_CLSPRC", default)]
            close: Option<String>,
            #[serde(rename = "MKTCAP", default)]
            market_cap: Option<String>,
            #[serde(rename = "ACC_TRDVOL", default)]
            volume: Option<String>,
        }

        let params: HashMap<&str, &str> = [("basDd", base_date)].into_iter().collect();
        let raw_etns: Vec<RawEtn> = self
            .request_with_category(ApiCategory::Etp, "etn_bydd_trd", &params)
            .await?;

        let etns: Vec<KrxEtnInfo> = raw_etns
            .into_iter()
            .filter_map(|e| {
                Some(KrxEtnInfo {
                    ticker: e.ticker?,
                    name: e.name,
                    underlying_index: e.underlying_index,
                    indicative_value: parse_decimal_opt(&e.indicative_value),
                    close: parse_decimal_opt(&e.close),
                    market_cap: parse_decimal_opt(&e.market_cap),
                    volume: e
                        .volume
                        .as_ref()
                        .and_then(|v| v.replace(",", "").parse().ok()),
                })
            })
            .collect();

        tracing::info!(count = etns.len(), "ETN 종목 조회 완료");
        Ok(etns)
    }

    /// 모든 지수 조회 (KRX + KOSPI + KOSDAQ 시리즈).
    pub async fn fetch_all_indices(
        &self,
        base_date: &str,
    ) -> Result<Vec<KrxIndexInfo>, Box<dyn std::error::Error + Send + Sync>> {
        // 병렬로 모든 지수 시리즈 조회
        let (krx_result, kospi_result, kosdaq_result) = tokio::join!(
            self.fetch_krx_index(base_date),
            self.fetch_kospi_index(base_date),
            self.fetch_kosdaq_index(base_date),
        );

        let mut all = krx_result?;
        all.extend(kospi_result?);
        all.extend(kosdaq_result?);

        tracing::info!(total = all.len(), "전체 지수 조회 완료");
        Ok(all)
    }

    /// 모든 시장 일별 매매정보 조회 (KOSPI + KOSDAQ).
    pub async fn fetch_all_daily_trades(
        &self,
        base_date: &str,
    ) -> Result<Vec<KrxDailyTrade>, Box<dyn std::error::Error + Send + Sync>> {
        // 병렬로 KOSPI, KOSDAQ 조회
        let (kospi_result, kosdaq_result) = tokio::join!(
            self.fetch_kospi_daily_trades(base_date),
            self.fetch_kosdaq_daily_trades(base_date),
        );

        let mut all = kospi_result?;
        all.extend(kosdaq_result?);

        tracing::info!(
            total = all.len(),
            base_date = base_date,
            "전체 일별 매매정보 조회 완료"
        );
        Ok(all)
    }

    /// 섹터별 종목 그룹핑.
    ///
    /// 일별 매매정보에서 섹터 정보를 추출하여 그룹핑합니다.
    pub async fn fetch_stocks_by_sector(
        &self,
        base_date: &str,
    ) -> Result<HashMap<String, Vec<KrxDailyTrade>>, Box<dyn std::error::Error + Send + Sync>> {
        let trades = self.fetch_all_daily_trades(base_date).await?;

        let mut by_sector: HashMap<String, Vec<KrxDailyTrade>> = HashMap::new();
        for trade in trades {
            let sector = trade.sector.clone().unwrap_or_else(|| "기타".to_string());
            by_sector.entry(sector).or_default().push(trade);
        }

        tracing::info!(sector_count = by_sector.len(), "섹터별 종목 그룹핑 완료");
        Ok(by_sector)
    }
}

/// 문자열을 Decimal로 파싱 (쉼표 제거).
fn parse_decimal_opt(s: &Option<String>) -> Option<Decimal> {
    s.as_ref().and_then(|v| {
        let cleaned = v.replace(",", "").replace("%", "");
        cleaned.parse().ok()
    })
}

/// YYYYMMDD 형식의 날짜 문자열을 NaiveDate로 파싱.
fn parse_date_yyyymmdd(s: &str) -> Option<NaiveDate> {
    // YYYY/MM/DD 형식도 지원
    if s.contains('/') {
        NaiveDate::parse_from_str(s, "%Y/%m/%d").ok()
    } else {
        // YYYYMMDD 형식
        NaiveDate::parse_from_str(s, "%Y%m%d").ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_decimal() {
        assert_eq!(
            parse_decimal_opt(&Some("1,234.56".to_string())),
            Some(Decimal::new(123456, 2))
        );
        assert_eq!(
            parse_decimal_opt(&Some("12.34%".to_string())),
            Some(Decimal::new(1234, 2))
        );
        assert_eq!(parse_decimal_opt(&None), None);
    }
}
