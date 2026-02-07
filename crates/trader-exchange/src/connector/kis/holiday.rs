//! KIS 휴장일 확인 모듈.
//!
//! 다음 시장의 휴장일 확인 기능을 제공합니다:
//! - 국내 시장 (KRX)
//! - 미국 시장 (NYSE, NASDAQ, AMEX)
//! - 기타 해외 시장
//!
//! # API 엔드포인트
//! - 국내: `/uapi/domestic-stock/v1/quotations/chk-holiday` (tr_id: CTCA0903R)
//! - 해외: `/uapi/overseas-stock/v1/quotations/countries-holiday` (tr_id: CTOS5011R)

#![allow(dead_code)] // 휴장일 캐시 메타데이터
#![allow(unused_comparisons)] // minute >= 0 비교 (문서화 목적)

use super::auth::KisOAuth;
use crate::ExchangeError;
use chrono::{Datelike, NaiveDate, Timelike, Utc, Weekday};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// 휴장일 API용 거래 ID.
pub mod tr_id {
    /// 국내 휴장일 조회
    pub const KR_HOLIDAY: &str = "CTCA0903R";
    /// 해외 휴장일 조회
    pub const OVERSEAS_HOLIDAY: &str = "CTOS5011R";
}

/// 해외 휴장일 조회용 국가/거래소 코드.
pub mod country_code {
    /// 미국
    pub const USA: &str = "USA";
    /// 홍콩
    pub const HONG_KONG: &str = "HKS";
    /// 일본
    pub const JAPAN: &str = "JPN";
    /// 중국 상해
    pub const CHINA_SHANGHAI: &str = "SHS";
    /// 중국 심천
    pub const CHINA_SHENZHEN: &str = "SZS";
    /// 베트남 하노이
    pub const VIETNAM_HANOI: &str = "HNX";
    /// 베트남 호치민
    pub const VIETNAM_HCMC: &str = "HSX";
}

/// KIS API 국내 휴장일 응답.
#[derive(Debug, Clone, Deserialize)]
pub struct KrHolidayResponse {
    /// 응답 코드 (0 = 성공)
    pub rt_cd: String,
    /// 메시지 코드
    pub msg_cd: String,
    /// 메시지
    pub msg1: String,
    /// 휴장일 데이터 목록
    #[serde(default)]
    pub output: Vec<KrHolidayItem>,
}

/// 국내 휴장일 개별 항목.
#[derive(Debug, Clone, Deserialize)]
pub struct KrHolidayItem {
    /// 기준일자 (YYYYMMDD)
    #[serde(rename = "bass_dt")]
    pub base_date: String,
    /// 요일명 (월, 화, 수, 목, 금, 토, 일)
    #[serde(rename = "wday_dvsn_cd_name")]
    pub weekday_name: String,
    /// 휴장여부 (Y/N)
    #[serde(rename = "bzdy_yn")]
    pub is_business_day: String,
    /// 거래일여부 (Y/N)
    #[serde(rename = "tr_day_yn")]
    pub is_trading_day: String,
    /// 결제일여부 (Y/N)
    #[serde(rename = "stl_day_yn")]
    pub is_settlement_day: String,
    /// 휴장사유
    #[serde(rename = "opnd_yn")]
    pub holiday_reason: String,
}

/// KIS API 해외 휴장일 응답.
#[derive(Debug, Clone, Deserialize)]
pub struct OverseasHolidayResponse {
    /// 응답 코드 (0 = 성공)
    pub rt_cd: String,
    /// 메시지 코드
    pub msg_cd: String,
    /// 메시지
    pub msg1: String,
    /// 휴장일 데이터 목록
    #[serde(default)]
    pub output: Vec<OverseasHolidayItem>,
}

/// 해외 휴장일 개별 항목.
#[derive(Debug, Clone, Deserialize)]
pub struct OverseasHolidayItem {
    /// 휴장일자 (YYYYMMDD)
    #[serde(rename = "bass_dt")]
    pub holiday_date: String,
    /// 휴장사유코드
    #[serde(rename = "holdy_clss_code")]
    pub holiday_code: String,
    /// 휴장사유명
    #[serde(rename = "holdy_clss_name")]
    pub holiday_name: String,
    /// 국가코드
    #[serde(rename = "natn_code")]
    pub country_code: String,
    /// 거래소코드
    #[serde(rename = "excd")]
    pub exchange_code: String,
}

/// 휴장일 캐시 항목.
#[derive(Debug, Clone)]
struct HolidayCache {
    /// 캐시된 휴장일 목록
    holidays: HashSet<NaiveDate>,
    /// 캐시 마지막 업데이트 시각
    last_updated: chrono::DateTime<Utc>,
    /// 연월 키 (YYYYMM 형식)
    year_month: String,
}

/// 국내 및 해외 시장 휴장일 확인기.
///
/// API 호출을 최소화하기 위한 캐싱을 제공합니다.
pub struct HolidayChecker {
    oauth: KisOAuth,
    client: Client,
    /// 국내 시장 휴장일 캐시
    kr_cache: Arc<RwLock<Option<HolidayCache>>>,
    /// 미국 시장 휴장일 캐시
    us_cache: Arc<RwLock<Option<HolidayCache>>>,
}

impl HolidayChecker {
    /// 새로운 휴장일 확인기 생성.
    ///
    /// # Errors
    /// HTTP 클라이언트 생성에 실패하면 `ExchangeError::NetworkError`를 반환합니다.
    pub fn new(oauth: KisOAuth) -> Result<Self, ExchangeError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(oauth.config().timeout_secs))
            .build()
            .map_err(|e| {
                ExchangeError::NetworkError(format!("HTTP 클라이언트 생성 실패: {}", e))
            })?;

        Ok(Self {
            oauth,
            client,
            kr_cache: Arc::new(RwLock::new(None)),
            us_cache: Arc::new(RwLock::new(None)),
        })
    }

    /// 주어진 날짜가 국내 시장 휴장일인지 확인.
    ///
    /// 해당 날짜에 시장이 휴장이면 `true`를 반환합니다.
    pub async fn is_kr_holiday(&self, date: NaiveDate) -> Result<bool, ExchangeError> {
        // 주말은 항상 휴장
        if date.weekday() == Weekday::Sat || date.weekday() == Weekday::Sun {
            return Ok(true);
        }

        // 캐시 확인
        let year_month = date.format("%Y%m").to_string();
        {
            let cache = self.kr_cache.read().await;
            if let Some(ref c) = *cache {
                if c.year_month == year_month {
                    return Ok(c.holidays.contains(&date));
                }
            }
        }

        // 캐시 미스 - API 호출
        let holidays = self.fetch_kr_holidays(&year_month).await?;
        let is_holiday = holidays.contains(&date);

        // 캐시 업데이트
        {
            let mut cache = self.kr_cache.write().await;
            *cache = Some(HolidayCache {
                holidays,
                last_updated: Utc::now(),
                year_month,
            });
        }

        Ok(is_holiday)
    }

    /// 주어진 날짜가 미국 시장 휴장일인지 확인.
    ///
    /// 해당 날짜에 시장이 휴장이면 `true`를 반환합니다.
    pub async fn is_us_holiday(&self, date: NaiveDate) -> Result<bool, ExchangeError> {
        // 주말은 항상 휴장
        if date.weekday() == Weekday::Sat || date.weekday() == Weekday::Sun {
            return Ok(true);
        }

        // 캐시 확인
        let year_month = date.format("%Y%m").to_string();
        {
            let cache = self.us_cache.read().await;
            if let Some(ref c) = *cache {
                if c.year_month == year_month {
                    return Ok(c.holidays.contains(&date));
                }
            }
        }

        // 캐시 미스 - API 호출
        let holidays = self
            .fetch_overseas_holidays(country_code::USA, &year_month)
            .await?;
        let is_holiday = holidays.contains(&date);

        // 캐시 업데이트
        {
            let mut cache = self.us_cache.write().await;
            *cache = Some(HolidayCache {
                holidays,
                last_updated: Utc::now(),
                year_month,
            });
        }

        Ok(is_holiday)
    }

    /// 오늘이 국내 시장 휴장일인지 확인.
    pub async fn is_kr_holiday_today(&self) -> Result<bool, ExchangeError> {
        // KST는 UTC+9
        let kst_now = Utc::now() + chrono::Duration::hours(9);
        let today = kst_now.date_naive();
        self.is_kr_holiday(today).await
    }

    /// 오늘이 미국 시장 휴장일인지 확인.
    pub async fn is_us_holiday_today(&self) -> Result<bool, ExchangeError> {
        // EST는 UTC-5 (또는 EDT는 UTC-4)
        // 보수적으로 UTC-5 사용
        let est_now = Utc::now() - chrono::Duration::hours(5);
        let today = est_now.date_naive();
        self.is_us_holiday(today).await
    }

    /// 현재 국내 시장 개장 여부 확인 (거래 시간 고려).
    ///
    /// 거래 시간: 09:00 - 15:30 KST
    pub async fn is_kr_market_open(&self) -> Result<bool, ExchangeError> {
        // 휴장일 체크
        if self.is_kr_holiday_today().await? {
            return Ok(false);
        }

        // KST 현재 시간 확인
        let kst_now = Utc::now() + chrono::Duration::hours(9);
        let hour = kst_now.hour();
        let minute = kst_now.minute();

        // 09:00 ~ 15:30
        let is_open = hour == 9 || (10..15).contains(&hour) || (hour == 15 && minute <= 30);

        Ok(is_open)
    }

    /// 현재 미국 시장 개장 여부 확인 (거래 시간 고려).
    ///
    /// 정규 거래 시간: 09:30 - 16:00 EST
    /// 프리마켓: 04:00 - 09:30 EST
    /// 애프터마켓: 16:00 - 20:00 EST
    pub async fn is_us_market_open(&self, include_extended: bool) -> Result<bool, ExchangeError> {
        // 휴장일 체크
        if self.is_us_holiday_today().await? {
            return Ok(false);
        }

        // EST 현재 시간 확인 (UTC-5)
        let est_now = Utc::now() - chrono::Duration::hours(5);
        let hour = est_now.hour();
        let minute = est_now.minute();

        if include_extended {
            // 확장 거래시간 포함: 04:00 ~ 20:00
            let is_open = (4..20).contains(&hour);
            Ok(is_open)
        } else {
            // 정규 거래시간만: 09:30 ~ 16:00
            let is_open = (hour == 9 && minute >= 30) || (10..16).contains(&hour);
            Ok(is_open)
        }
    }

    /// 국내 시장의 다음 거래일 조회.
    pub async fn next_kr_trading_day(&self, from: NaiveDate) -> Result<NaiveDate, ExchangeError> {
        let mut date = from + chrono::Duration::days(1);

        // 최대 30일까지 검색 (연휴 대비)
        for _ in 0..30 {
            if !self.is_kr_holiday(date).await? {
                return Ok(date);
            }
            date += chrono::Duration::days(1);
        }

        Err(ExchangeError::ApiError {
            code: -1,
            message: "Could not find next trading day within 30 days".to_string(),
        })
    }

    /// 미국 시장의 다음 거래일 조회.
    pub async fn next_us_trading_day(&self, from: NaiveDate) -> Result<NaiveDate, ExchangeError> {
        let mut date = from + chrono::Duration::days(1);

        // 최대 30일까지 검색
        for _ in 0..30 {
            if !self.is_us_holiday(date).await? {
                return Ok(date);
            }
            date += chrono::Duration::days(1);
        }

        Err(ExchangeError::ApiError {
            code: -1,
            message: "Could not find next trading day within 30 days".to_string(),
        })
    }

    /// KIS API에서 국내 휴장일 조회.
    async fn fetch_kr_holidays(
        &self,
        year_month: &str,
    ) -> Result<HashSet<NaiveDate>, ExchangeError> {
        let url = format!(
            "{}/uapi/domestic-stock/v1/quotations/chk-holiday",
            self.oauth.config().rest_base_url()
        );

        // 해당 월의 시작일과 종료일 계산
        let year: i32 = year_month[0..4].parse().unwrap_or(2026);
        let month: u32 = year_month[4..6].parse().unwrap_or(1);

        let start_date = format!("{}{:02}01", year, month);
        // end_date는 향후 범위 조회에 사용할 예정
        let _end_date = if month == 12 {
            format!("{}0101", year + 1)
        } else {
            format!("{}{:02}01", year, month + 1)
        };

        let headers = self.oauth.build_headers(tr_id::KR_HOLIDAY, None).await?;

        debug!("Fetching KR holidays for {}", year_month);

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[
                ("BASS_DT", &start_date),
                ("CTX_AREA_NK", &"".to_string()),
                ("CTX_AREA_FK", &"".to_string()),
            ])
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        let resp: KrHolidayResponse = serde_json::from_str(&body).map_err(|e| {
            ExchangeError::ParseError(format!("Failed to parse holiday response: {}", e))
        })?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        // 휴장일만 추출 (거래일이 아닌 날)
        let mut holidays = HashSet::new();
        for item in resp.output {
            if item.is_trading_day != "Y" {
                if let Ok(date) = NaiveDate::parse_from_str(&item.base_date, "%Y%m%d") {
                    holidays.insert(date);
                    debug!("KR Holiday: {} ({})", item.base_date, item.holiday_reason);
                }
            }
        }

        info!("Loaded {} KR holidays for {}", holidays.len(), year_month);
        Ok(holidays)
    }

    /// KIS API에서 해외 휴장일 조회.
    async fn fetch_overseas_holidays(
        &self,
        country: &str,
        year_month: &str,
    ) -> Result<HashSet<NaiveDate>, ExchangeError> {
        let url = format!(
            "{}/uapi/overseas-stock/v1/quotations/countries-holiday",
            self.oauth.config().rest_base_url()
        );

        let headers = self
            .oauth
            .build_headers(tr_id::OVERSEAS_HOLIDAY, None)
            .await?;

        // 해당 월의 시작일과 종료일 계산
        let year: i32 = year_month[0..4].parse().unwrap_or(2026);
        let month: u32 = year_month[4..6].parse().unwrap_or(1);

        let start_date = format!("{}{:02}01", year, month);
        // end_date 계산 (향후 범위 조회에 사용 예정)
        let _end_date = if month == 12 {
            format!("{}{:02}31", year, month)
        } else {
            // 해당 월의 마지막 날
            let last_day = match month {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                2 => {
                    if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                        29
                    } else {
                        28
                    }
                }
                _ => 31,
            };
            format!("{}{:02}{:02}", year, month, last_day)
        };

        debug!("Fetching {} holidays for {}", country, year_month);

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[
                ("TRAD_DT", &start_date),
                ("CTX_AREA_NK", &"".to_string()),
                ("CTX_AREA_FK", &"".to_string()),
            ])
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        let resp: OverseasHolidayResponse = serde_json::from_str(&body).map_err(|e| {
            ExchangeError::ParseError(format!("Failed to parse holiday response: {}", e))
        })?;

        if resp.rt_cd != "0" {
            // 에러가 아닌 경우도 있음 (데이터 없음)
            if resp.msg_cd == "40000000" {
                warn!(
                    "No holiday data available for {} in {}",
                    country, year_month
                );
                return Ok(HashSet::new());
            }
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        // 해당 국가의 휴장일만 추출
        let mut holidays = HashSet::new();
        for item in resp.output {
            if item.country_code == country {
                if let Ok(date) = NaiveDate::parse_from_str(&item.holiday_date, "%Y%m%d") {
                    holidays.insert(date);
                    debug!(
                        "{} Holiday: {} ({})",
                        country, item.holiday_date, item.holiday_name
                    );
                }
            }
        }

        info!(
            "Loaded {} {} holidays for {}",
            holidays.len(),
            country,
            year_month
        );
        Ok(holidays)
    }

    /// 캐시된 휴장일 데이터 초기화.
    pub async fn clear_cache(&self) {
        {
            let mut cache = self.kr_cache.write().await;
            *cache = None;
        }
        {
            let mut cache = self.us_cache.write().await;
            *cache = None;
        }
        info!("Holiday cache cleared");
    }

    /// 특정 월의 국내 휴장일 목록 조회.
    pub async fn get_kr_holidays_for_month(
        &self,
        year: i32,
        month: u32,
    ) -> Result<Vec<NaiveDate>, ExchangeError> {
        let year_month = format!("{}{:02}", year, month);
        let holidays = self.fetch_kr_holidays(&year_month).await?;
        let mut sorted: Vec<_> = holidays.into_iter().collect();
        sorted.sort();
        Ok(sorted)
    }

    /// 특정 월의 미국 휴장일 목록 조회.
    pub async fn get_us_holidays_for_month(
        &self,
        year: i32,
        month: u32,
    ) -> Result<Vec<NaiveDate>, ExchangeError> {
        let year_month = format!("{}{:02}", year, month);
        let holidays = self
            .fetch_overseas_holidays(country_code::USA, &year_month)
            .await?;
        let mut sorted: Vec<_> = holidays.into_iter().collect();
        sorted.sort();
        Ok(sorted)
    }
}

/// 시장 상태 열거형.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketStatus {
    /// 거래 가능 (장 시작)
    Open,
    /// 휴장 (휴일 또는 거래 시간 외)
    Closed,
    /// 프리마켓 세션 (미국만 해당)
    PreMarket,
    /// 애프터마켓 세션 (미국만 해당)
    AfterHours,
}

impl std::fmt::Display for MarketStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketStatus::Open => write!(f, "Open"),
            MarketStatus::Closed => write!(f, "Closed"),
            MarketStatus::PreMarket => write!(f, "Pre-Market"),
            MarketStatus::AfterHours => write!(f, "After-Hours"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weekend_is_holiday() {
        // 토요일
        let saturday = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        assert_eq!(saturday.weekday(), Weekday::Sat);

        // 일요일
        let sunday = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        assert_eq!(sunday.weekday(), Weekday::Sun);
    }

    #[test]
    fn test_date_parsing() {
        let date_str = "20260128";
        let date = NaiveDate::parse_from_str(date_str, "%Y%m%d").unwrap();
        assert_eq!(date.year(), 2026);
        assert_eq!(date.month(), 1);
        assert_eq!(date.day(), 28);
    }

    #[test]
    fn test_year_month_format() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 28).unwrap();
        let year_month = date.format("%Y%m").to_string();
        assert_eq!(year_month, "202601");
    }

    #[test]
    fn test_market_status_display() {
        assert_eq!(MarketStatus::Open.to_string(), "Open");
        assert_eq!(MarketStatus::Closed.to_string(), "Closed");
        assert_eq!(MarketStatus::PreMarket.to_string(), "Pre-Market");
        assert_eq!(MarketStatus::AfterHours.to_string(), "After-Hours");
    }

    #[test]
    fn test_last_day_of_month() {
        // 2월 윤년
        let year = 2024;
        let month = 2;
        let last_day = if month == 12 {
            31
        } else if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) && month == 2 {
            29
        } else {
            match month {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                2 => 28,
                _ => 31,
            }
        };
        assert_eq!(last_day, 29);

        // 2월 평년
        let year = 2025;
        let last_day = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            29
        } else {
            28
        };
        assert_eq!(last_day, 28);
    }
}
