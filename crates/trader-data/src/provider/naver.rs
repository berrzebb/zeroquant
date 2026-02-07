//! 네이버 금융 크롤러.
//!
//! 국내(KR) 주식의 펀더멘털 데이터를 네이버 금융에서 수집합니다.
//! Yahoo Finance보다 더 정확한 한국 주식 데이터를 제공합니다.
//!
//! ## 데이터 소스
//! - `/item/main.naver`: 시가총액, 52주 고저, 거래량, 업종
//! - `/item/coinfo.naver`: PER, PBR, ROE, EPS, BPS, 배당수익률
//!
//! ## 사용 예시
//! ```rust,ignore
//! let fetcher = NaverFinanceFetcher::new();
//! let data = fetcher.fetch_fundamental("005930").await?;
//! println!("삼성전자 PER: {:?}", data.per);
//! ```

use reqwest::Client;
use rust_decimal::Decimal;
use scraper::{Html, Selector};
use std::time::Duration;
use thiserror::Error;

/// 네이버 금융 크롤러 에러
#[derive(Debug, Error)]
pub enum NaverError {
    #[error("HTTP 요청 실패: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("HTML 파싱 실패: {0}")]
    ParseError(String),

    #[error("데이터 없음: {ticker}")]
    NoData { ticker: String },

    #[error("Rate limit 초과")]
    RateLimited,
}

/// 시장 구분 (KOSPI/KOSDAQ/ETF/KONEX 등)
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub enum KrMarketType {
    /// 유가증권시장 (코스피)
    Kospi,
    /// 코스닥
    Kosdaq,
    /// ETF (상장지수펀드)
    Etf,
    /// ETN (상장지수증권)
    Etn,
    /// 코넥스
    Konex,
    /// 기타/알 수 없음
    #[default]
    Unknown,
}


impl std::fmt::Display for KrMarketType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Kospi => write!(f, "KOSPI"),
            Self::Kosdaq => write!(f, "KOSDAQ"),
            Self::Etf => write!(f, "ETF"),
            Self::Etn => write!(f, "ETN"),
            Self::Konex => write!(f, "KONEX"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

impl KrMarketType {
    /// 문자열에서 시장 타입 파싱
    pub fn parse(s: &str) -> Self {
        let s = s.to_uppercase();
        if s.contains("KOSPI") || s.contains("코스피") || s.contains("유가증권") {
            Self::Kospi
        } else if s.contains("KOSDAQ") || s.contains("코스닥") {
            Self::Kosdaq
        } else if s.contains("ETF") {
            Self::Etf
        } else if s.contains("ETN") {
            Self::Etn
        } else if s.contains("KONEX") || s.contains("코넥스") {
            Self::Konex
        } else {
            Self::Unknown
        }
    }

    /// 거래소 코드 반환 (Yahoo Finance 접미사용)
    pub fn yahoo_suffix(&self) -> &'static str {
        match self {
            Self::Kospi | Self::Etf | Self::Etn => ".KS",
            Self::Kosdaq => ".KQ",
            Self::Konex => ".KQ",   // KONEX도 KQ 사용
            Self::Unknown => ".KS", // 기본값
        }
    }
}

/// 네이버 금융 펀더멘털 데이터
///
/// Yahoo Finance와 동일한 수준의 데이터를 제공합니다.
#[derive(Debug, Clone, Default)]
pub struct NaverFundamentalData {
    /// 종목 코드
    pub ticker: String,
    /// 종목명
    pub name: Option<String>,
    /// 시장 구분 (KOSPI/KOSDAQ/ETF 등)
    pub market_type: KrMarketType,
    /// 시가총액 (원)
    pub market_cap: Option<Decimal>,
    /// PER (주가수익비율)
    pub per: Option<Decimal>,
    /// Forward PER (예상 PER) - 네이버 미제공, 항상 None
    pub forward_per: Option<Decimal>,
    /// PBR (주가순자산비율)
    pub pbr: Option<Decimal>,
    /// PSR (주가매출비율)
    pub psr: Option<Decimal>,
    /// ROE (자기자본이익률, %)
    pub roe: Option<Decimal>,
    /// EPS (주당순이익, 원)
    pub eps: Option<Decimal>,
    /// Forward EPS (예상 EPS) - 네이버 미제공, 항상 None
    pub forward_eps: Option<Decimal>,
    /// BPS (주당순자산, 원)
    pub bps: Option<Decimal>,
    /// 배당수익률 (%)
    pub dividend_yield: Option<Decimal>,
    /// 52주 최고가
    pub week_52_high: Option<Decimal>,
    /// 52주 최저가
    pub week_52_low: Option<Decimal>,
    /// 10일 평균 거래량
    pub avg_volume_10d: Option<i64>,
    /// 3개월 평균 거래량 - 네이버 미제공, 항상 None
    pub avg_volume_3m: Option<i64>,
    /// 업종(섹터)
    pub sector: Option<String>,
    /// 동일업종 PER
    pub sector_per: Option<Decimal>,
    /// 외국인 소진율 (%)
    pub foreign_ratio: Option<Decimal>,
    /// 현재가
    pub current_price: Option<Decimal>,
    /// 거래량
    pub volume: Option<i64>,
    /// 매출액 (억원)
    pub revenue: Option<Decimal>,
    /// 영업이익 (억원)
    pub operating_income: Option<Decimal>,
    /// 순이익 (억원)
    pub net_income: Option<Decimal>,
    /// 매출액 성장률 (YoY, %)
    pub revenue_growth_yoy: Option<Decimal>,
    /// 영업이익 성장률 (YoY, %)
    pub operating_income_growth_yoy: Option<Decimal>,
    /// 순이익 성장률 (YoY, %)
    pub net_income_growth_yoy: Option<Decimal>,
    /// ROA (총자산이익률, %)
    pub roa: Option<Decimal>,
    /// 매출총이익률 (%) - 네이버 미제공, 항상 None
    pub gross_margin: Option<Decimal>,
    /// 영업이익률 (%)
    pub operating_margin: Option<Decimal>,
    /// 순이익률 (%) - 계산: 순이익/매출액*100
    pub net_profit_margin: Option<Decimal>,
    /// 부채비율 (%)
    pub debt_ratio: Option<Decimal>,
    /// 유동비율 (%)
    pub current_ratio: Option<Decimal>,
    /// 당좌비율 (%)
    pub quick_ratio: Option<Decimal>,
    /// 발행주식수
    pub shares_outstanding: Option<i64>,
    /// 유통주식수 - 네이버 미제공, 항상 None
    pub float_shares: Option<i64>,
    /// 베타 - 네이버 미제공, 항상 None
    pub beta: Option<Decimal>,
    /// 통화 (항상 KRW)
    pub currency: String,
}

/// 네이버 금융 크롤러
///
/// HTML 파싱을 통해 네이버 금융에서 주식 데이터를 수집합니다.
pub struct NaverFinanceFetcher {
    client: Client,
    /// 요청 간 딜레이 (기본: 300ms)
    request_delay: Duration,
}

impl NaverFinanceFetcher {
    /// 기본 설정으로 생성
    pub fn new() -> Self {
        Self::with_delay(Duration::from_millis(300))
    }

    /// 커스텀 딜레이로 생성
    pub fn with_delay(request_delay: Duration) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .expect("HTTP 클라이언트 생성 실패");

        Self {
            client,
            request_delay,
        }
    }

    /// 요청 딜레이 반환
    pub fn request_delay(&self) -> Duration {
        self.request_delay
    }

    /// 펀더멘털 데이터 수집 (main 페이지 우선, coinfo 보조)
    ///
    /// main 페이지에서 대부분의 데이터를 ID 기반 셀렉터로 추출합니다.
    /// BPS, ROE 등 main에 없는 데이터는 coinfo 페이지에서 보완합니다.
    ///
    /// # Arguments
    /// * `ticker` - 종목 코드 (예: "005930")
    pub async fn fetch_fundamental(
        &self,
        ticker: &str,
    ) -> Result<NaverFundamentalData, NaverError> {
        let mut data = NaverFundamentalData {
            ticker: ticker.to_string(),
            currency: "KRW".to_string(),
            ..Default::default()
        };

        // 1. main 페이지에서 기본 정보 + 투자지표 수집
        // PER, PBR, EPS, 배당수익률은 main 페이지에서 ID 기반으로 추출
        self.fetch_main_page(ticker, &mut data).await?;

        // 2. BPS, ROE 등 누락된 데이터가 있으면 coinfo 페이지에서 보완
        if data.bps.is_none() || data.roe.is_none() {
            tokio::time::sleep(self.request_delay).await;
            let _ = self.fetch_coinfo_page(ticker, &mut data).await;
        }

        // TODO: 컨센서스 데이터 (Forward PER/EPS) 수집
        // 네이버 금융 컨센서스 페이지 크롤링 필요
        // if data.forward_per.is_none() || data.forward_eps.is_none() {
        //     tokio::time::sleep(self.request_delay).await;
        //     let _ = self.fetch_consensus_page(ticker, &mut data).await;
        // }

        // TODO: 재무제표에서 매출총이익률 추출
        // 네이버 금융 재무제표 페이지 크롤링 필요
        // if data.gross_margin.is_none() {
        //     tokio::time::sleep(self.request_delay).await;
        //     let _ = self.fetch_financial_statements(ticker, &mut data).await;
        // }

        // 5. 추가 계산: 누락된 지표 보완
        self.calculate_derived_metrics(&mut data);

        Ok(data)
    }

    /// 파생 지표 계산 (직접 추출 불가능한 지표들)
    fn calculate_derived_metrics(&self, data: &mut NaverFundamentalData) {
        // 순이익률 계산: 순이익 / 매출액 * 100
        if data.net_profit_margin.is_none() {
            if let (Some(net_income), Some(revenue)) = (data.net_income, data.revenue) {
                if revenue > Decimal::ZERO {
                    data.net_profit_margin = Some((net_income / revenue) * Decimal::from(100));
                }
            }
        }

        // 발행주식수 계산: 시가총액 / 현재가
        if data.shares_outstanding.is_none() {
            if let (Some(market_cap), Some(current_price)) = (data.market_cap, data.current_price) {
                if current_price > Decimal::ZERO {
                    let shares = market_cap / current_price;
                    data.shares_outstanding = Some(shares.to_string().parse::<i64>().unwrap_or(0));
                }
            }
        }
    }

    /// main 페이지 크롤링 (시가총액, 52주 고저, 업종, 투자지표 등)
    ///
    /// 네이버 금융 main 페이지에서 대부분의 데이터를 추출합니다.
    /// 투자지표(PER, PBR, EPS, 배당수익률)도 main 페이지에 ID 기반으로 제공됩니다.
    async fn fetch_main_page(
        &self,
        ticker: &str,
        data: &mut NaverFundamentalData,
    ) -> Result<(), NaverError> {
        let url = format!("https://finance.naver.com/item/main.naver?code={}", ticker);

        let response = self.client.get(&url).send().await?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(NaverError::RateLimited);
        }

        let html = response.text().await?;
        let document = Html::parse_document(&html);

        // 종목명 추출
        if let Some(name) = self.extract_stock_name(&document) {
            data.name = Some(name);
        }

        // 현재가 추출
        data.current_price = self.extract_current_price(&document);

        // 시가총액 추출
        data.market_cap = self.extract_market_cap(&document);

        // 52주 고저 추출
        let (high, low) = self.extract_52week_high_low(&document);
        data.week_52_high = high;
        data.week_52_low = low;

        // 업종(섹터) 추출
        data.sector = self.extract_sector(&document);

        // 시장 타입 추출 (KOSPI/KOSDAQ/ETF)
        data.market_type = self.extract_market_type(&document);

        // 외국인 소진율 추출
        data.foreign_ratio = self.extract_foreign_ratio(&document);

        // 거래량 추출
        data.volume = self.extract_volume(&document);

        // 투자지표 추출 (ID 기반 셀렉터 사용)
        // main 페이지에 <em id="_per">, <em id="_pbr"> 등으로 제공됨
        data.per = self.extract_value_by_id(&document, "_per");
        data.pbr = self.extract_value_by_id(&document, "_pbr");
        data.eps = self.extract_value_by_id(&document, "_eps");
        data.dividend_yield = self.extract_value_by_id(&document, "_dvr");

        // ROE 추출 (main 페이지의 동종업종비교 테이블에서)
        data.roe = self.extract_roe(&document);

        // ROA, 영업이익률, 부채비율, 유동비율, 당좌비율 추출 (동종업종비교 테이블)
        self.extract_financial_ratios(&document, data);

        // 성장률 및 절대값 추출 (재무정보 요약 테이블에서)
        // revenue, operating_income, net_income도 여기서 추출됨
        self.extract_growth_rates(&document, data);

        // PSR 계산 (시가총액 / 매출액)
        // 네이버에서 직접 제공하지 않으므로 계산으로 산출
        if data.psr.is_none() {
            if let (Some(market_cap), Some(revenue)) = (data.market_cap, data.revenue) {
                // 매출액이 0보다 크고 시가총액이 있을 때만 계산
                // 매출액은 억원 단위, 시가총액은 원 단위이므로 변환 필요
                // PSR = 시가총액 / (매출액 * 1억)
                if revenue > Decimal::ZERO {
                    let revenue_won = revenue * Decimal::from(100_000_000); // 억원 → 원
                    data.psr = Some(market_cap / revenue_won);
                }
            }
        }

        Ok(())
    }

    /// ROE(자기자본이익률) 추출
    ///
    /// HTML 구조: `<th class="th_cop_anal13">ROE(지배주주)</th><td>17.07</td>`
    fn extract_roe(&self, document: &Html) -> Option<Decimal> {
        // "ROE" 텍스트가 포함된 행에서 td 값 추출
        if let Ok(tr_selector) = Selector::parse("tr") {
            if let Ok(td_selector) = Selector::parse("td") {
                for tr in document.select(&tr_selector) {
                    let text = tr.text().collect::<String>();

                    // "ROE"가 포함되고 "동일업종"이 아닌 행
                    if text.contains("ROE") && !text.contains("동일업종") {
                        for td in tr.select(&td_selector) {
                            let td_text = td.text().collect::<String>().trim().to_string();
                            // 숫자로 시작하는 값 찾기
                            if let Some(val) = parse_decimal_value(&td_text) {
                                return Some(val);
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// ROA, 영업이익률, 부채비율, 유동비율, 당좌비율 추출 (동종업종비교 테이블)
    ///
    /// 네이버 금융 main 페이지의 동종업종비교 테이블에서 재무비율 추출
    fn extract_financial_ratios(&self, document: &Html, data: &mut NaverFundamentalData) {
        if let Ok(tr_selector) = Selector::parse("tr") {
            if let Ok(td_selector) = Selector::parse("td") {
                for tr in document.select(&tr_selector) {
                    let text = tr.text().collect::<String>();

                    // ROA (총자산이익률) 추출
                    if text.contains("ROA") && !text.contains("동일업종") && data.roa.is_none() {
                        for td in tr.select(&td_selector) {
                            let td_text = td.text().collect::<String>().trim().to_string();
                            if let Some(val) = parse_decimal_value(&td_text) {
                                data.roa = Some(val);
                                break;
                            }
                        }
                    }

                    // 영업이익률 추출
                    if (text.contains("영업이익률") || text.contains("영업이익율"))
                        && !text.contains("동일업종")
                        && data.operating_margin.is_none()
                    {
                        for td in tr.select(&td_selector) {
                            let td_text = td.text().collect::<String>().trim().to_string();
                            if let Some(val) = parse_decimal_value(&td_text) {
                                data.operating_margin = Some(val);
                                break;
                            }
                        }
                    }

                    // 부채비율 추출
                    if text.contains("부채비율") && !text.contains("동일업종") && data.debt_ratio.is_none() {
                        for td in tr.select(&td_selector) {
                            let td_text = td.text().collect::<String>().trim().to_string();
                            if let Some(val) = parse_decimal_value(&td_text) {
                                data.debt_ratio = Some(val);
                                break;
                            }
                        }
                    }

                    // 유동비율 추출
                    if text.contains("유동비율") && !text.contains("동일업종") && data.current_ratio.is_none() {
                        for td in tr.select(&td_selector) {
                            let td_text = td.text().collect::<String>().trim().to_string();
                            if let Some(val) = parse_decimal_value(&td_text) {
                                data.current_ratio = Some(val);
                                break;
                            }
                        }
                    }

                    // 당좌비율 추출
                    if text.contains("당좌비율") && !text.contains("동일업종") && data.quick_ratio.is_none() {
                        for td in tr.select(&td_selector) {
                            let td_text = td.text().collect::<String>().trim().to_string();
                            if let Some(val) = parse_decimal_value(&td_text) {
                                data.quick_ratio = Some(val);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    /// 성장률 및 절대값 추출 (재무정보 요약 테이블에서)
    ///
    /// 네이버 금융 main 페이지의 재무정보 테이블에서 최근 2개년 데이터를 비교하여
    /// 매출액, 영업이익, 순이익의 YoY 성장률을 계산하고 최신 절대값도 저장합니다.
    ///
    /// HTML 구조 (네이버 금융 main 페이지):
    /// ```html
    /// <div class="section cop_analysis">
    ///   <table class="tb_type1 tb_num">
    ///     <tr><th>매출액</th><td>100,000</td><td>120,000</td>...</tr>
    ///     <tr><th>영업이익</th><td>10,000</td><td>15,000</td>...</tr>
    ///     <tr><th>당기순이익</th><td>8,000</td><td>12,000</td>...</tr>
    ///   </table>
    /// </div>
    /// ```
    fn extract_growth_rates(&self, document: &Html, data: &mut NaverFundamentalData) {
        // 네이버 금융의 재무정보 테이블 셀렉터들 (다양한 페이지 구조 대응)
        let table_selectors = [
            "div.cop_analysis table.tb_type1 tr",           // 기업분석 섹션
            "div.section table.tb_type1 tr",                // 일반 섹션
            "table.tb_type1.tb_num tr",                     // 숫자 테이블
            "table.per_table tr",                           // PER 테이블
            "div#tab_con1 table tr",                        // 탭 콘텐츠
        ];

        let td_selector = match Selector::parse("td") {
            Ok(s) => s,
            Err(_) => return,
        };
        let th_selector = match Selector::parse("th") {
            Ok(s) => s,
            Err(_) => return,
        };

        // 각 테이블 셀렉터를 시도하여 재무 데이터 추출
        for selector_str in &table_selectors {
            let tr_selector = match Selector::parse(selector_str) {
                Ok(s) => s,
                Err(_) => continue,
            };

            for tr in document.select(&tr_selector) {
                // 행의 헤더 텍스트 추출
                let header_text = tr
                    .select(&th_selector)
                    .next()
                    .map(|th| th.text().collect::<String>())
                    .unwrap_or_default();

                // 관심 있는 재무 항목인지 확인
                let is_revenue = header_text.contains("매출액") && !header_text.contains("원가");
                let is_operating = header_text.contains("영업이익") && !header_text.contains("률");
                let is_net_income = header_text.contains("당기순이익")
                    || (header_text.contains("순이익") && !header_text.contains("률"));

                if !is_revenue && !is_operating && !is_net_income {
                    continue;
                }

                // 해당 행의 td 값들 추출 (단위: 억원 또는 원)
                let values: Vec<Option<Decimal>> = tr
                    .select(&td_selector)
                    .filter_map(|td| {
                        let text = td.text().collect::<String>();
                        // 빈 셀이나 N/A 등 제외
                        if text.trim().is_empty() || text.contains("N/A") {
                            return None;
                        }
                        Some(parse_financial_value(&text))
                    })
                    .collect();

                // 최소 2개 연도 데이터가 있어야 성장률 계산 가능
                // 유효한 값만 필터링
                let valid_values: Vec<Decimal> = values.into_iter().flatten().collect();

                if valid_values.len() >= 2 {
                    // 네이버 금융은 보통 왼쪽이 과거, 오른쪽이 최신
                    // 마지막 2개 값으로 YoY 계산 (최신 2개년)
                    let len = valid_values.len();
                    let prev = valid_values[len - 2];
                    let recent = valid_values[len - 1];

                    // 최신 절대값 저장 (억원 단위)
                    if is_revenue && data.revenue.is_none() {
                        data.revenue = Some(recent);
                    } else if is_operating && data.operating_income.is_none() {
                        data.operating_income = Some(recent);
                    } else if is_net_income && data.net_income.is_none() {
                        data.net_income = Some(recent);
                    }

                    // 전년도 값이 0이 아닌 경우에만 성장률 계산
                    if prev != Decimal::ZERO {
                        let growth = ((recent - prev) / prev.abs()) * Decimal::from(100);

                        // 성장률이 너무 극단적인 경우 (-1000% ~ +10000%) 제한
                        let capped_growth = growth
                            .max(Decimal::from(-1000))
                            .min(Decimal::from(10000));

                        if is_revenue && data.revenue_growth_yoy.is_none() {
                            data.revenue_growth_yoy = Some(capped_growth);
                        } else if is_operating && data.operating_income_growth_yoy.is_none() {
                            data.operating_income_growth_yoy = Some(capped_growth);
                        } else if is_net_income && data.net_income_growth_yoy.is_none() {
                            data.net_income_growth_yoy = Some(capped_growth);
                        }
                    }
                } else if !valid_values.is_empty() {
                    // 1개 연도 데이터만 있는 경우 최신값만 저장
                    let recent = valid_values[valid_values.len() - 1];
                    if is_revenue && data.revenue.is_none() {
                        data.revenue = Some(recent);
                    } else if is_operating && data.operating_income.is_none() {
                        data.operating_income = Some(recent);
                    } else if is_net_income && data.net_income.is_none() {
                        data.net_income = Some(recent);
                    }
                }
            }

            // 모든 데이터가 추출되었으면 조기 종료
            if data.revenue_growth_yoy.is_some()
                && data.operating_income_growth_yoy.is_some()
                && data.net_income_growth_yoy.is_some()
                && data.revenue.is_some()
                && data.operating_income.is_some()
                && data.net_income.is_some()
            {
                break;
            }
        }
    }

    /// ID 기반 값 추출 (네이버 금융 페이지의 em 태그)
    ///
    /// `<em id="_per">35.11</em>배` 형태에서 숫자 값 추출
    fn extract_value_by_id(&self, document: &Html, id: &str) -> Option<Decimal> {
        let selector_str = format!("em#{}", id);
        let selector = Selector::parse(&selector_str).ok()?;

        document.select(&selector).next().and_then(|el| {
            let text = el.text().collect::<String>();
            parse_decimal_value(&text)
        })
    }

    /// coinfo 페이지 크롤링 (PER, PBR, ROE, EPS, BPS, 배당수익률)
    async fn fetch_coinfo_page(
        &self,
        ticker: &str,
        data: &mut NaverFundamentalData,
    ) -> Result<(), NaverError> {
        let url = format!(
            "https://finance.naver.com/item/coinfo.naver?code={}",
            ticker
        );

        let response = self.client.get(&url).send().await?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(NaverError::RateLimited);
        }

        let html = response.text().await?;
        let document = Html::parse_document(&html);

        // 투자 지표 테이블에서 데이터 추출
        self.extract_investment_indicators(&document, data);

        Ok(())
    }

    /// 종목명 추출
    fn extract_stock_name(&self, document: &Html) -> Option<String> {
        // <div class="wrap_company"> 내의 종목명
        let selector = Selector::parse("div.wrap_company h2 a").ok()?;
        document
            .select(&selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
    }

    /// 현재가 추출
    fn extract_current_price(&self, document: &Html) -> Option<Decimal> {
        // 현재가: <p class="no_today"> 내의 <span class="blind">
        let selector = Selector::parse("p.no_today span.blind").ok()?;
        document.select(&selector).next().and_then(|el| {
            let text = el.text().collect::<String>();
            parse_korean_number(&text)
        })
    }

    /// 시가총액 추출
    fn extract_market_cap(&self, document: &Html) -> Option<Decimal> {
        // 시가총액: <table class="no_info"> 내에서 "시가총액" 찾기
        let selector = Selector::parse("table.no_info td").ok()?;

        for (i, td) in document.select(&selector).enumerate() {
            let text = td.text().collect::<String>();
            if text.contains("시가총액") {
                // 다음 td에 값이 있음
                if let Some(next_td) = document.select(&selector).nth(i + 1) {
                    let value_text = next_td.text().collect::<String>();
                    // "억원" 단위를 원 단위로 변환
                    return parse_market_cap(&value_text);
                }
            }
        }

        // 대안: em#_market_sum 셀렉터
        let alt_selector = Selector::parse("em#_market_sum").ok()?;
        document.select(&alt_selector).next().and_then(|el| {
            let text = el.text().collect::<String>();
            parse_market_cap(&text)
        })
    }

    /// 52주 최고/최저 추출
    ///
    /// HTML 구조: `<th>52주최고|최저</th><td><em>169,400</em><span>|</span><em>52,500</em></td>`
    fn extract_52week_high_low(&self, document: &Html) -> (Option<Decimal>, Option<Decimal>) {
        let mut high = None;
        let mut low = None;

        // 방법 1: "52주최고" 텍스트가 포함된 행에서 em 태그 추출
        if let Ok(tr_selector) = Selector::parse("tr") {
            if let Ok(em_selector) = Selector::parse("em") {
                for tr in document.select(&tr_selector) {
                    let text = tr.text().collect::<String>();

                    if text.contains("52주최고") || text.contains("52주 최고") {
                        let ems: Vec<_> = tr.select(&em_selector).collect();
                        // 첫 번째 em = 52주 최고, 두 번째 em = 52주 최저
                        if ems.len() >= 2 {
                            high = parse_korean_number(&ems[0].text().collect::<String>());
                            low = parse_korean_number(&ems[1].text().collect::<String>());
                            return (high, low);
                        } else if ems.len() == 1 {
                            // em이 하나만 있으면 최고가로 간주
                            high = parse_korean_number(&ems[0].text().collect::<String>());
                        }
                    }
                }
            }
        }

        (high, low)
    }

    /// 업종(섹터) 추출
    ///
    /// HTML 구조: `<a href="/sise/sise_group_detail.naver?type=upjong&no=278">반도체와반도체장비</a>`
    fn extract_sector(&self, document: &Html) -> Option<String> {
        // 모든 a 태그에서 upjong 링크 찾기 (가장 넓은 범위)
        let selector = Selector::parse("a").ok()?;

        for a in document.select(&selector) {
            let href = a.value().attr("href").unwrap_or("");
            // upjong 링크이면서 sise_group_detail을 포함하는 경우 (동종업종비교 섹션)
            if href.contains("sise_group_detail") && href.contains("upjong") {
                let text = a.text().collect::<String>().trim().to_string();
                // 빈 문자열이 아니고, "더보기", "동일업종" 등의 텍스트가 아닌 경우만
                if !text.is_empty()
                    && !text.contains("더보기")
                    && !text.contains("동일업종")
                    && !text.contains("PER")
                    && !text.contains("등락률")
                {
                    return Some(text);
                }
            }
        }

        None
    }

    /// 시장 타입 추출 (KOSPI/KOSDAQ/ETF)
    fn extract_market_type(&self, document: &Html) -> KrMarketType {
        // 1. 탭 메뉴에서 시장 정보 추출
        // 네이버 금융 페이지 상단의 시장 정보 링크
        if let Ok(selector) = Selector::parse("div.wrap_company img, div.description img") {
            for img in document.select(&selector) {
                if let Some(alt) = img.value().attr("alt") {
                    let market_type = KrMarketType::parse(alt);
                    if market_type != KrMarketType::Unknown {
                        return market_type;
                    }
                }
                if let Some(src) = img.value().attr("src") {
                    // 이미지 경로에서 시장 타입 추출
                    if src.contains("kospi") {
                        return KrMarketType::Kospi;
                    } else if src.contains("kosdaq") {
                        return KrMarketType::Kosdaq;
                    } else if src.contains("etf") {
                        return KrMarketType::Etf;
                    }
                }
            }
        }

        // 2. 페이지 타이틀이나 텍스트에서 추출
        if let Ok(selector) = Selector::parse("div.wrap_company, div.description, span.market") {
            for el in document.select(&selector) {
                let text = el.text().collect::<String>();
                if text.contains("ETF") || text.contains("상장지수") {
                    return KrMarketType::Etf;
                } else if text.contains("ETN") {
                    return KrMarketType::Etn;
                } else if text.contains("KOSDAQ") || text.contains("코스닥") {
                    return KrMarketType::Kosdaq;
                } else if text.contains("KOSPI") || text.contains("코스피") {
                    return KrMarketType::Kospi;
                } else if text.contains("KONEX") || text.contains("코넥스") {
                    return KrMarketType::Konex;
                }
            }
        }

        // 3. URL 기반 추론 (ETF 전용 페이지)
        // ETF는 /item/main.naver?code=XXX 이후 다른 구조를 가질 수 있음
        if let Ok(selector) = Selector::parse("a[href*='etf']") {
            if document.select(&selector).next().is_some() {
                // ETF 관련 링크가 많으면 ETF일 가능성
            }
        }

        // 4. 종목코드 패턴 기반 추론
        // 일반적으로 6자리 숫자 코드
        // - ETF: 주로 1~3, 069, 091, 102, 114, 122, 130, 143, 148, 152, 161, 168, 200대 시작
        // - 보통주: 다양한 패턴

        KrMarketType::Unknown
    }

    /// 외국인 소진율 추출
    ///
    /// HTML 구조: `<th class="th_cop_comp6">외국인비율(%)</th><td>51.72</td>`
    fn extract_foreign_ratio(&self, document: &Html) -> Option<Decimal> {
        // 방법 1: "외국인비율" 텍스트가 포함된 행에서 값 추출
        if let Ok(tr_selector) = Selector::parse("tr") {
            if let Ok(td_selector) = Selector::parse("td") {
                for tr in document.select(&tr_selector) {
                    let text = tr.text().collect::<String>();

                    if text.contains("외국인비율") {
                        // 해당 행의 td에서 숫자 추출
                        for td in tr.select(&td_selector) {
                            let td_text = td.text().collect::<String>().trim().to_string();
                            if let Some(val) = parse_decimal_value(&td_text) {
                                return Some(val);
                            }
                        }
                    }
                }
            }
        }

        // 방법 2: th_cop_comp6 클래스 다음 td
        if let Ok(th_selector) = Selector::parse("th.th_cop_comp6") {
            for th in document.select(&th_selector) {
                let text = th.text().collect::<String>();
                if text.contains("외국인") {
                    // 다음 형제 td 찾기
                    if let Some(parent) = th.parent() {
                        if let Ok(td_selector) = Selector::parse("td") {
                            if let Some(tr) = parent.parent() {
                                let tr_element = scraper::ElementRef::wrap(tr)?;
                                if let Some(td) = tr_element.select(&td_selector).next() {
                                    let td_text = td.text().collect::<String>();
                                    return parse_decimal_value(&td_text);
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// 거래량 추출
    fn extract_volume(&self, document: &Html) -> Option<i64> {
        // 거래량: <table class="no_info"> 내에서 "거래량" 찾기
        let selector = Selector::parse("table.no_info td").ok()?;

        let mut found_volume = false;
        for td in document.select(&selector) {
            let text = td.text().collect::<String>();

            if text.contains("거래량") {
                found_volume = true;
                continue;
            }

            if found_volume {
                let cleaned: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
                if !cleaned.is_empty() {
                    return cleaned.parse().ok();
                }
                found_volume = false;
            }
        }

        None
    }

    /// 투자 지표 추출 (coinfo 페이지) - 누락된 값만 채움
    ///
    /// main 페이지에서 이미 추출한 값은 덮어쓰지 않고,
    /// BPS, ROE 등 main에 없는 데이터만 보완합니다.
    fn extract_investment_indicators(&self, document: &Html, data: &mut NaverFundamentalData) {
        // coinfo 페이지의 투자 지표 테이블 파싱
        // 이미 값이 있으면 덮어쓰지 않음 (main 페이지에서 ID 기반으로 추출한 값 우선)

        // PER - main에서 이미 추출됨 (백업용)
        if data.per.is_none() {
            data.per = self.extract_indicator_value(document, "PER");
        }

        // PBR - main에서 이미 추출됨 (백업용)
        if data.pbr.is_none() {
            data.pbr = self.extract_indicator_value(document, "PBR");
        }

        // ROE - main에 없음, coinfo에서만 추출 가능
        if data.roe.is_none() {
            data.roe = self.extract_indicator_value(document, "ROE");
        }

        // EPS - main에서 이미 추출됨 (백업용)
        if data.eps.is_none() {
            data.eps = self.extract_indicator_value(document, "EPS");
        }

        // BPS - main에 없음, coinfo에서 추출
        if data.bps.is_none() {
            data.bps = self.extract_bps_value(document);
        }

        // 배당수익률 - main에서 이미 추출됨 (백업용)
        if data.dividend_yield.is_none() {
            data.dividend_yield = self.extract_indicator_value(document, "배당수익률");
        }

        // 동일업종 PER
        if data.sector_per.is_none() {
            data.sector_per = self.extract_indicator_value(document, "동일업종 PER");
        }
    }

    /// BPS 값 추출 (coinfo 페이지 전용)
    ///
    /// coinfo 페이지에서 BPS는 "PBR|BPS" 행의 두 번째 <em> 태그에 있음
    /// 예: `<em>60,632</em>원`
    fn extract_bps_value(&self, document: &Html) -> Option<Decimal> {
        // PBR|BPS 행을 찾아서 BPS 값 추출
        let tr_selector = Selector::parse("table.per_table tr").ok()?;

        for tr in document.select(&tr_selector) {
            let text = tr.text().collect::<String>();

            // "BPS" 가 포함된 행 찾기
            if text.contains("BPS") {
                // 해당 행의 em 태그들에서 값 추출
                let em_selector = Selector::parse("em").ok()?;
                let ems: Vec<_> = tr.select(&em_selector).collect();

                // 두 번째 em이 보통 BPS 값 (첫 번째는 PBR)
                if ems.len() >= 2 {
                    let bps_text = ems[1].text().collect::<String>();
                    return parse_decimal_value(&bps_text);
                }
            }
        }

        None
    }

    /// 지표 값 추출 헬퍼
    fn extract_indicator_value(&self, document: &Html, indicator_name: &str) -> Option<Decimal> {
        // 테이블 행에서 지표명을 찾고 해당 값 추출
        let td_selector = match Selector::parse("td, th") {
            Ok(s) => s,
            Err(_) => return None,
        };

        let tds: Vec<_> = document.select(&td_selector).collect();

        for (i, td) in tds.iter().enumerate() {
            let text = td.text().collect::<String>();

            if text.contains(indicator_name) {
                // 같은 행 또는 다음 셀에서 값 찾기
                // PER, PBR 등은 보통 다음 셀에 숫자 값이 있음
                for td_elem in tds.iter().skip(i + 1).take(4) {
                    let value_text = td_elem.text().collect::<String>();
                    // 숫자로 시작하거나 음수(-)로 시작하는 경우
                    let trimmed = value_text.trim();
                    if trimmed
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_digit() || c == '-')
                    {
                        if let Some(val) = parse_decimal_value(trimmed) {
                            return Some(val);
                        }
                    }
                }
            }
        }

        // 대안: span 또는 em 태그에서 직접 찾기
        let alt_selectors = [
            format!("td:contains('{}')", indicator_name),
            format!("th:contains('{}')", indicator_name),
        ];

        for _sel_str in &alt_selectors {
            // scraper는 :contains를 지원하지 않으므로 위 방식 사용
        }

        None
    }

    /// 배치 수집 (여러 종목)
    ///
    /// # Arguments
    /// * `tickers` - 종목 코드 목록
    /// * `progress_callback` - 진행 상황 콜백 (현재 인덱스, 전체 수)
    pub async fn fetch_batch<F>(
        &self,
        tickers: &[String],
        mut progress_callback: F,
    ) -> Vec<Result<NaverFundamentalData, NaverError>>
    where
        F: FnMut(usize, usize),
    {
        let mut results = Vec::with_capacity(tickers.len());
        let total = tickers.len();

        for (i, ticker) in tickers.iter().enumerate() {
            progress_callback(i + 1, total);

            let result = self.fetch_fundamental(ticker).await;
            results.push(result);

            // 마지막 항목이 아니면 딜레이 적용
            if i + 1 < total {
                tokio::time::sleep(self.request_delay).await;
            }
        }

        results
    }
}

impl Default for NaverFinanceFetcher {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== 파싱 유틸리티 함수 ====================

/// 한글 숫자 문자열 파싱 (쉼표 제거)
///
/// "1,234,567" -> 1234567
fn parse_korean_number(text: &str) -> Option<Decimal> {
    let cleaned: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();

    if cleaned.is_empty() {
        return None;
    }

    cleaned.parse().ok()
}

/// 시가총액 파싱 (억원/조원 -> 원)
///
/// "1,234억원" -> 123400000000
/// "1.5조원" -> 1500000000000
fn parse_market_cap(text: &str) -> Option<Decimal> {
    let text = text.trim();

    // 숫자 부분 추출
    let num_str: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .collect();

    let num_str = num_str.replace(',', "");

    if num_str.is_empty() {
        return None;
    }

    let value: Decimal = num_str.parse().ok()?;

    // 단위 변환
    if text.contains('조') {
        // 조 = 1,000,000,000,000 (10^12)
        Some(value * Decimal::from(1_000_000_000_000i64))
    } else if text.contains('억') {
        // 억 = 100,000,000 (10^8)
        Some(value * Decimal::from(100_000_000i64))
    } else {
        // 단위 없으면 그대로 (원 단위로 가정)
        Some(value)
    }
}

/// 퍼센트 문자열 파싱
///
/// "12.34%" -> 12.34
#[allow(dead_code)]
fn parse_percentage(text: &str) -> Option<Decimal> {
    let cleaned: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();

    if cleaned.is_empty() {
        return None;
    }

    cleaned.parse().ok()
}

/// 일반 Decimal 값 파싱
///
/// 쉼표 제거 및 숫자 추출
fn parse_decimal_value(text: &str) -> Option<Decimal> {
    let cleaned: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();

    if cleaned.is_empty() || cleaned == "-" {
        return None;
    }

    cleaned.parse().ok()
}

/// 재무제표 값 파싱 (억원/조원/원 단위 처리)
///
/// 네이버 금융의 재무정보 테이블에서 사용되는 다양한 형식 지원:
/// - "1,234" -> 1234
/// - "1,234억" -> 123400000000
/// - "1.5조" -> 1500000000000
/// - "-100" -> -100
fn parse_financial_value(text: &str) -> Option<Decimal> {
    let text = text.trim();

    // 빈 문자열 또는 N/A 체크
    if text.is_empty() || text == "-" || text.contains("N/A") {
        return None;
    }

    // 숫자 부분 추출 (쉼표, 소수점, 음수 부호 포함)
    let num_str: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',' || *c == '-')
        .collect();

    let num_str = num_str.replace(',', "");

    if num_str.is_empty() || num_str == "-" {
        return None;
    }

    let value: Decimal = num_str.parse().ok()?;

    // 단위 변환 (네이버 금융은 보통 억원 단위)
    // 하지만 성장률 계산에는 단위가 같기만 하면 되므로 그대로 반환
    // 조/억 단위 표시가 있으면 변환, 없으면 그대로
    if text.contains('조') {
        Some(value * Decimal::from(10000)) // 억원 단위로 통일
    } else {
        Some(value) // 이미 억원 단위로 가정
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_korean_number() {
        assert_eq!(
            parse_korean_number("1,234,567"),
            Some(Decimal::from(1234567))
        );
        assert_eq!(parse_korean_number("56,000"), Some(Decimal::from(56000)));
        assert_eq!(parse_korean_number("-100"), Some(Decimal::from(-100)));
        assert_eq!(parse_korean_number(""), None);
    }

    #[test]
    fn test_parse_market_cap() {
        // 억원 단위
        assert_eq!(
            parse_market_cap("1,234억원"),
            Some(Decimal::from(123_400_000_000i64))
        );
        assert_eq!(parse_market_cap("5억"), Some(Decimal::from(500_000_000i64)));

        // 조원 단위
        assert_eq!(
            parse_market_cap("1.5조"),
            Some(Decimal::new(15, 1) * Decimal::from(1_000_000_000_000i64))
        );

        // 단위 없음
        assert_eq!(parse_market_cap("1000000"), Some(Decimal::from(1000000)));
    }

    #[test]
    fn test_parse_percentage() {
        assert_eq!(parse_percentage("12.34%"), Some(Decimal::new(1234, 2)));
        assert_eq!(parse_percentage("-5.5%"), Some(Decimal::new(-55, 1)));
    }

    #[tokio::test]
    #[ignore] // 실제 네트워크 테스트는 ignore
    async fn test_fetch_samsung() {
        let fetcher = NaverFinanceFetcher::new();
        let result = fetcher.fetch_fundamental("005930").await;

        match result {
            Ok(data) => {
                println!("삼성전자 데이터:");
                println!("  종목명: {:?}", data.name);
                println!("  시가총액: {:?}", data.market_cap);
                println!("  PER: {:?}", data.per);
                println!("  PBR: {:?}", data.pbr);
                println!("  EPS: {:?}", data.eps);
                println!("  BPS: {:?}", data.bps);
                println!("  배당수익률: {:?}", data.dividend_yield);
                println!("  ROE: {:?}", data.roe);
                println!("  업종: {:?}", data.sector);
                println!("  시장타입: {:?}", data.market_type);

                // 주요 값이 있는지 검증
                assert!(data.per.is_some(), "PER이 추출되어야 함");
                assert!(data.pbr.is_some(), "PBR이 추출되어야 함");
                assert!(data.eps.is_some(), "EPS가 추출되어야 함");
            }
            Err(e) => {
                eprintln!("오류: {}", e);
            }
        }
    }
}
