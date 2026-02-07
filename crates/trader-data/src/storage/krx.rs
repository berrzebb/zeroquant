//! KRX(한국거래소) 데이터 소스.
//!
//! KRX 정보데이터시스템에서 국내 주식 OHLCV 데이터를 조회합니다.
//!
//! # 사용 예제
//!
//! ```rust,ignore
//! use trader_data::storage::krx::KrxDataSource;
//!
//! let krx = KrxDataSource::new();
//! let klines = krx.get_ohlcv("005930", "20260101", "20260129").await?;
//! ```

use crate::error::{DataError, Result};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;
use tracing::{debug, info};
use trader_core::{Kline, Timeframe};

/// KRX API 기본 URL.
const KRX_API_URL: &str = "https://data.krx.co.kr/comm/bldAttendant/getJsonData.cmd";

/// KRX 개별종목 시세 조회 bld.
const BLD_STOCK_OHLCV: &str = "dbms/MDC/STAT/standard/MDCSTAT01701";

/// KRX 전종목 시세 조회 bld (일별).
#[allow(dead_code)] // 향후 전종목 시세 조회 기능에서 사용 예정
const BLD_MARKET_OHLCV: &str = "dbms/MDC/STAT/standard/MDCSTAT01501";

/// KRX 정보데이터시스템 API 응답 구조.
///
/// 참고: KRX 정보데이터시스템은 "output" 키를 사용하고,
/// KRX Open API는 "OutBlock_1" 키를 사용합니다.
#[derive(Debug, Deserialize)]
struct KrxApiResponse {
    /// 출력 데이터 배열 (정보데이터시스템 응답 키).
    #[serde(default)]
    output: Vec<KrxOhlcvRecord>,
}

/// KRX OHLCV 레코드.
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // API 응답 전체 필드 매핑 (일부만 사용)
struct KrxOhlcvRecord {
    /// 거래일자 (YYYY/MM/DD 또는 YYYYMMDD)
    #[serde(rename = "TRD_DD")]
    trd_dd: Option<String>,

    /// 종목코드
    #[serde(rename = "ISU_SRT_CD", default)]
    isu_srt_cd: String,

    /// 시가
    #[serde(rename = "TDD_OPNPRC", default)]
    open: String,

    /// 고가
    #[serde(rename = "TDD_HGPRC", default)]
    high: String,

    /// 저가
    #[serde(rename = "TDD_LWPRC", default)]
    low: String,

    /// 종가
    #[serde(rename = "TDD_CLSPRC", default)]
    close: String,

    /// 거래량
    #[serde(rename = "ACC_TRDVOL", default)]
    volume: String,

    /// 거래대금
    #[serde(rename = "ACC_TRDVAL", default)]
    value: String,
}

/// KRX 데이터 소스.
pub struct KrxDataSource {
    client: reqwest::Client,
}

impl KrxDataSource {
    /// 새로운 KRX 데이터 소스 생성.
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .expect("HTTP 클라이언트 생성 실패");

        Self { client }
    }

    /// 개별 종목 OHLCV 데이터 조회.
    ///
    /// # 인자
    /// - `stock_code`: 종목코드 (6자리, 예: "005930")
    /// - `start_date`: 시작일 (YYYYMMDD)
    /// - `end_date`: 종료일 (YYYYMMDD)
    pub async fn get_ohlcv(
        &self,
        stock_code: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<Kline>> {
        debug!(
            stock_code = stock_code,
            start = start_date,
            end = end_date,
            "KRX OHLCV 조회"
        );

        // ISIN 코드로 변환 (KR + 종목코드 + 체크디지트)
        // 간단히 종목코드만 사용 (KRX API가 단축코드도 지원)
        let isin_cd = format!("KR7{}003", stock_code);

        // KRX API 파라미터 (날짜는 YYYYMMDD 형식)
        let params = [
            ("bld", BLD_STOCK_OHLCV),
            ("isuCd", &isin_cd),
            ("strtDd", start_date),
            ("endDd", end_date),
            ("adjStkPrc", "2"), // 수정주가 사용
        ];

        let response = self
            .client
            .post(KRX_API_URL)
            .header(
                "Referer",
                "https://data.krx.co.kr/contents/MDC/MDI/outerLoader/index.cmd",
            )
            .form(&params)
            .send()
            .await
            .map_err(|e| DataError::FetchError(format!("KRX API 호출 실패: {}", e)))?;

        if !response.status().is_success() {
            return Err(DataError::FetchError(format!(
                "KRX API 오류: {}",
                response.status()
            )));
        }

        let text = response
            .text()
            .await
            .map_err(|e| DataError::FetchError(format!("응답 읽기 실패: {}", e)))?;

        debug!(response_len = text.len(), "KRX API 응답 수신");

        // JSON 파싱
        let api_response: KrxApiResponse = serde_json::from_str(&text).map_err(|e| {
            DataError::ParseError(format!(
                "JSON 파싱 실패: {} - {}",
                e,
                &text[..text.len().min(200)]
            ))
        })?;

        // Kline으로 변환
        let klines = self.convert_to_klines(stock_code, &api_response.output)?;

        info!(
            stock_code = stock_code,
            count = klines.len(),
            "KRX OHLCV 조회 완료"
        );

        Ok(klines)
    }

    /// KRX 레코드를 Kline으로 변환.
    fn convert_to_klines(
        &self,
        stock_code: &str,
        records: &[KrxOhlcvRecord],
    ) -> Result<Vec<Kline>> {
        let mut klines = Vec::with_capacity(records.len());

        for record in records {
            // 날짜 파싱 (YYYY/MM/DD 또는 YYYYMMDD)
            let date_str = record.trd_dd.as_deref().unwrap_or("");
            let date = parse_krx_date(date_str)?;

            // 숫자 파싱 (쉼표 제거)
            let open = parse_krx_number(&record.open)?;
            let high = parse_krx_number(&record.high)?;
            let low = parse_krx_number(&record.low)?;
            let close = parse_krx_number(&record.close)?;
            let volume = parse_krx_number(&record.volume)?;

            // 유효성 검사 (0이면 스킵)
            if close.is_zero() {
                continue;
            }

            klines.push(Kline {
                ticker: stock_code.to_string(),
                timeframe: Timeframe::D1,
                open_time: date,
                open,
                high,
                low,
                close,
                volume,
                close_time: date + chrono::Duration::days(1),
                quote_volume: None,
                num_trades: None,
            });
        }

        // 날짜순 정렬 (오래된 것부터)
        klines.sort_by(|a, b| a.open_time.cmp(&b.open_time));

        Ok(klines)
    }
}

impl Default for KrxDataSource {
    fn default() -> Self {
        Self::new()
    }
}

/// KRX 날짜 문자열 파싱.
fn parse_krx_date(s: &str) -> Result<DateTime<Utc>> {
    // YYYY/MM/DD 형식
    if s.contains('/') {
        let date = NaiveDate::parse_from_str(s, "%Y/%m/%d")
            .map_err(|e| DataError::ParseError(format!("날짜 파싱 실패: {} - {}", s, e)))?;
        return Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()));
    }

    // YYYYMMDD 형식
    let date = NaiveDate::parse_from_str(s, "%Y%m%d")
        .map_err(|e| DataError::ParseError(format!("날짜 파싱 실패: {} - {}", s, e)))?;
    Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()))
}

/// KRX 숫자 문자열 파싱 (쉼표 제거).
fn parse_krx_number(s: &str) -> Result<Decimal> {
    if s.is_empty() || s == "-" {
        return Ok(Decimal::ZERO);
    }

    // 쉼표 제거
    let cleaned = s.replace(',', "");

    Decimal::from_str(&cleaned)
        .map_err(|e| DataError::ParseError(format!("숫자 파싱 실패: {} - {}", s, e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_krx_date() {
        let date = parse_krx_date("2026/01/15").unwrap();
        assert_eq!(date.format("%Y-%m-%d").to_string(), "2026-01-15");

        let date2 = parse_krx_date("20260115").unwrap();
        assert_eq!(date2.format("%Y-%m-%d").to_string(), "2026-01-15");
    }

    #[test]
    fn test_parse_krx_number() {
        assert_eq!(
            parse_krx_number("1,234,567").unwrap(),
            Decimal::from(1234567)
        );
        assert_eq!(parse_krx_number("100").unwrap(), Decimal::from(100));
        assert_eq!(parse_krx_number("").unwrap(), Decimal::ZERO);
        assert_eq!(parse_krx_number("-").unwrap(), Decimal::ZERO);
    }
}
