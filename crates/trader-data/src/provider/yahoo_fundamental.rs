//! Yahoo Finance 펀더멘털 데이터 수집기.
//!
//! yahoo_finance_api crate를 사용하여 글로벌 주식의 펀더멘털 데이터를 수집합니다.
//! Crumb 인증이 자동으로 처리됩니다.
//!
//! ## 수집 항목
//!
//! - **밸류에이션**: PER(trailing/forward), PBR, PSR, EPS, BPS
//! - **시장 정보**: 시가총액, 52주 고저, 평균 거래량
//! - **수익성**: ROE, ROA, 영업이익률, 순이익률, 매출총이익률
//! - **성장성**: 매출성장률, 이익성장률
//! - **배당**: 배당수익률
//! - **기타**: 섹터, 산업, 베타, 부채비율
//!
//! ## 사용 예시
//!
//! ```rust,ignore
//! let fetcher = YahooFundamentalFetcher::new()?;
//! let data = fetcher.fetch_fundamental("AAPL").await?;
//! println!("Apple PER: {:?}", data.per);
//! ```

use rust_decimal::Decimal;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::debug;
use yahoo_finance_api as yahoo;

/// Yahoo Finance 펀더멘털 에러
#[derive(Debug, Error)]
pub enum YahooFundamentalError {
    #[error("Yahoo API 에러: {0}")]
    YahooError(String),

    #[error("데이터 없음: {ticker}")]
    NoData { ticker: String },

    #[error("Rate limit 초과")]
    RateLimited,

    #[error("API 오류: {0}")]
    ApiError(String),
}

impl From<yahoo::YahooError> for YahooFundamentalError {
    fn from(e: yahoo::YahooError) -> Self {
        let msg = format!("{:?}", e);
        if msg.contains("429") || msg.contains("Too Many") {
            YahooFundamentalError::RateLimited
        } else {
            YahooFundamentalError::YahooError(msg)
        }
    }
}

/// Yahoo Finance 펀더멘털 데이터
#[derive(Debug, Clone, Default)]
pub struct YahooFundamentalData {
    /// 종목 코드 (Yahoo 형식, 예: AAPL, 005930.KS)
    pub ticker: String,
    /// 종목명
    pub name: Option<String>,
    /// 시가총액
    pub market_cap: Option<Decimal>,
    /// Trailing PER
    pub per: Option<Decimal>,
    /// Forward PER
    pub forward_per: Option<Decimal>,
    /// PBR (Price to Book)
    pub pbr: Option<Decimal>,
    /// PSR (Price to Sales)
    pub psr: Option<Decimal>,
    /// EPS (Trailing)
    pub eps: Option<Decimal>,
    /// Forward EPS
    pub forward_eps: Option<Decimal>,
    /// BPS (Book Value per Share)
    pub bps: Option<Decimal>,
    /// 배당수익률 (%)
    pub dividend_yield: Option<Decimal>,
    /// 52주 최고가
    pub week_52_high: Option<Decimal>,
    /// 52주 최저가
    pub week_52_low: Option<Decimal>,
    /// 10일 평균 거래량
    pub avg_volume_10d: Option<i64>,
    /// 3개월 평균 거래량
    pub avg_volume_3m: Option<i64>,
    /// ROE (자기자본이익률, %)
    pub roe: Option<Decimal>,
    /// ROA (총자산이익률, %)
    pub roa: Option<Decimal>,
    /// 매출총이익률 (%)
    pub gross_margin: Option<Decimal>,
    /// 영업이익률 (%)
    pub operating_margin: Option<Decimal>,
    /// 순이익률 (%)
    pub net_profit_margin: Option<Decimal>,
    /// 부채비율 (%)
    pub debt_to_equity: Option<Decimal>,
    /// 유동비율 (%)
    pub current_ratio: Option<Decimal>,
    /// 당좌비율 (%)
    pub quick_ratio: Option<Decimal>,
    /// 매출액
    pub revenue: Option<Decimal>,
    /// 순이익
    pub net_income: Option<Decimal>,
    /// 매출 성장률 YoY (%)
    pub revenue_growth_yoy: Option<Decimal>,
    /// 이익 성장률 YoY (%)
    pub earnings_growth_yoy: Option<Decimal>,
    /// 섹터
    pub sector: Option<String>,
    /// 산업
    pub industry: Option<String>,
    /// 베타 (시장 민감도)
    pub beta: Option<Decimal>,
    /// 통화
    pub currency: String,
    /// 발행주식수
    pub shares_outstanding: Option<i64>,
    /// 유통주식수
    pub float_shares: Option<i64>,
}

/// Yahoo Finance 펀더멘털 크롤러
///
/// yahoo_finance_api crate를 사용하여 Crumb 인증을 자동으로 처리합니다.
pub struct YahooFundamentalFetcher {
    provider: Mutex<yahoo::YahooConnector>,
    /// 요청 간 딜레이 (기본: 500ms)
    request_delay: Duration,
}

impl YahooFundamentalFetcher {
    /// 기본 설정으로 생성
    pub fn new() -> Result<Self, YahooFundamentalError> {
        Self::with_delay(Duration::from_millis(500))
    }

    /// 커스텀 딜레이로 생성
    pub fn with_delay(request_delay: Duration) -> Result<Self, YahooFundamentalError> {
        let provider = yahoo::YahooConnector::new()
            .map_err(|e| YahooFundamentalError::YahooError(format!("{:?}", e)))?;

        Ok(Self {
            provider: Mutex::new(provider),
            request_delay,
        })
    }

    /// 요청 딜레이 반환
    pub fn request_delay(&self) -> Duration {
        self.request_delay
    }

    /// 펀더멘털 데이터 수집
    ///
    /// # Arguments
    /// * `ticker` - Yahoo Finance 형식 종목 코드 (예: "AAPL", "005930.KS")
    pub async fn fetch_fundamental(
        &self,
        ticker: &str,
    ) -> Result<YahooFundamentalData, YahooFundamentalError> {
        let mut data = YahooFundamentalData {
            ticker: ticker.to_string(),
            currency: self.guess_currency(ticker),
            ..Default::default()
        };

        debug!(ticker = ticker, "Yahoo Finance get_ticker_info 요청");

        // quoteSummary API 호출 (yahoo_finance_api crate의 get_ticker_info 사용)
        let quote_summary = {
            let mut provider = self.provider.lock().await;
            provider.get_ticker_info(ticker).await
        }.map_err(|e| {
            let msg = format!("{:?}", e);
            if msg.contains("429") || msg.contains("Too Many") {
                YahooFundamentalError::RateLimited
            } else if msg.contains("404") || msg.contains("Not Found") {
                YahooFundamentalError::NoData { ticker: ticker.to_string() }
            } else {
                YahooFundamentalError::YahooError(msg)
            }
        })?;

        // YQuoteSummary → quote_summary → result[0] → YSummaryData 구조 탐색
        let summary_data = quote_summary
            .quote_summary
            .and_then(|qs| qs.result)
            .and_then(|results| results.into_iter().next())
            .ok_or_else(|| YahooFundamentalError::NoData { ticker: ticker.to_string() })?;

        // QuoteType 정보 (종목명)
        if let Some(ref qt) = summary_data.quote_type {
            data.name = qt.long_name.clone().or_else(|| qt.short_name.clone());
        }

        // SummaryDetail (PER, 배당, 52주 고저, 거래량, 베타, 시가총액)
        if let Some(ref sd) = summary_data.summary_detail {
            data.per = sd.trailing_pe.and_then(|v| Decimal::from_f64_retain(v));
            data.forward_per = sd.forward_pe.and_then(|v| Decimal::from_f64_retain(v));
            data.dividend_yield = sd.dividend_yield.and_then(|v| Decimal::from_f64_retain(v * 100.0));
            data.week_52_high = sd.fifty_two_week_high.and_then(|v| Decimal::from_f64_retain(v));
            data.week_52_low = sd.fifty_two_week_low.and_then(|v| Decimal::from_f64_retain(v));
            data.avg_volume_10d = sd.average_volume_10days.map(|v| v as i64);
            data.avg_volume_3m = sd.average_volume.map(|v| v as i64);
            data.beta = sd.beta.and_then(|v| Decimal::from_f64_retain(v));
            // 시가총액 (u64 → Decimal)
            data.market_cap = sd.market_cap.map(|v| Decimal::from(v));
            // 통화
            if let Some(ref c) = sd.currency {
                data.currency = c.clone();
            }
        }

        // FinancialData (매출, 수익성 지표)
        if let Some(ref fd) = summary_data.financial_data {
            // total_revenue는 i64 타입
            data.revenue = fd.total_revenue.map(|v| Decimal::from(v));
            data.roe = fd.return_on_equity.and_then(|v| Decimal::from_f64_retain(v * 100.0));
            data.roa = fd.return_on_assets.and_then(|v| Decimal::from_f64_retain(v * 100.0));
            data.gross_margin = fd.gross_margins.and_then(|v| Decimal::from_f64_retain(v * 100.0));
            data.operating_margin = fd.operating_margins.and_then(|v| Decimal::from_f64_retain(v * 100.0));
            data.net_profit_margin = fd.profit_margins.and_then(|v| Decimal::from_f64_retain(v * 100.0));
            data.debt_to_equity = fd.debt_to_equity.and_then(|v| Decimal::from_f64_retain(v));
            data.current_ratio = fd.current_ratio.and_then(|v| Decimal::from_f64_retain(v));
            data.quick_ratio = fd.quick_ratio.and_then(|v| Decimal::from_f64_retain(v));
            data.revenue_growth_yoy = fd.revenue_growth.and_then(|v| Decimal::from_f64_retain(v * 100.0));
            data.earnings_growth_yoy = fd.earnings_growth.and_then(|v| Decimal::from_f64_retain(v * 100.0));
        }

        // DefaultKeyStatistics (EPS, BPS, PBR, PSR, 주식수, 순이익)
        if let Some(ref ks) = summary_data.default_key_statistics {
            data.eps = ks.trailing_eps.and_then(|v| Decimal::from_f64_retain(v));
            data.forward_eps = ks.forward_eps.and_then(|v| Decimal::from_f64_retain(v));
            data.bps = ks.book_value.and_then(|v| Decimal::from_f64_retain(v));
            data.pbr = ks.price_to_book.and_then(|v| Decimal::from_f64_retain(v));
            data.shares_outstanding = ks.shares_outstanding.map(|v| v as i64);
            data.float_shares = ks.float_shares.map(|v| v as i64);
            // net_income_to_common은 i64 타입
            data.net_income = ks.net_income_to_common.map(|v| Decimal::from(v));

            // beta가 아직 없으면 여기서도 시도
            if data.beta.is_none() {
                data.beta = ks.beta.and_then(|v| Decimal::from_f64_retain(v));
            }
        }

        // AssetProfile (섹터, 산업)
        if let Some(ref ap) = summary_data.asset_profile {
            data.sector = ap.sector.clone();
            data.industry = ap.industry.clone();
        }

        debug!(
            ticker = ticker,
            per = ?data.per,
            roe = ?data.roe,
            roa = ?data.roa,
            "Yahoo Finance 데이터 파싱 완료"
        );

        Ok(data)
    }

    /// 통화 추측 (티커 접미사 기반)
    fn guess_currency(&self, ticker: &str) -> String {
        if ticker.ends_with(".KS") || ticker.ends_with(".KQ") {
            "KRW".to_string()
        } else if ticker.ends_with(".T") {
            "JPY".to_string()
        } else if ticker.ends_with(".HK") {
            "HKD".to_string()
        } else if ticker.ends_with(".L") {
            "GBP".to_string()
        } else if ticker.ends_with(".DE") || ticker.ends_with(".PA") {
            "EUR".to_string()
        } else {
            "USD".to_string()
        }
    }
}
