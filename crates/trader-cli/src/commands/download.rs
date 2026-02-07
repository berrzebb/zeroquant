//! 주식 시장 과거 OHLCV 데이터 다운로드 명령어.
//!
//! 데이터 소스 우선순위:
//! - US: Yahoo Finance → KIS API (fallback)
//! - KR: Yahoo Finance → KIS API (fallback)
//!
//! KIS API는 사용량 제한이 있으므로 외부 데이터 소스를 우선적으로 사용합니다.

use anyhow::{Context, Result};
use chrono::{NaiveDate, TimeZone, Utc};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::str::FromStr;
use tracing::{debug, info, warn};

/// 지원되는 시장 유형
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Market {
    /// 한국 주식 시장 (KOSPI/KOSDAQ)
    KR,
    /// 미국 주식 시장 (NYSE/NASDAQ/AMEX)
    US,
}

impl Market {
    /// 문자열에서 시장 파싱
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "KR" | "KOREA" | "KRX" | "KOSPI" | "KOSDAQ" => Some(Self::KR),
            "US" | "USA" | "NYSE" | "NASDAQ" | "AMEX" => Some(Self::US),
            _ => None,
        }
    }

    /// Yahoo Finance 심볼 접미사 반환
    #[allow(dead_code)]
    pub fn yahoo_suffix(&self) -> &'static str {
        match self {
            Self::KR => ".KS", // 코스피 (코스닥은 .KQ)
            Self::US => "",    // 미국은 접미사 없음
        }
    }
}

/// 지원되는 타임프레임 간격
#[derive(Debug, Clone, Copy)]
pub enum Interval {
    D1, // 일봉
    W1, // 주봉
    M1, // 월봉
}

impl Interval {
    /// 문자열에서 간격 파싱
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "1d" | "d1" | "d" | "daily" => Some(Self::D1),
            "1w" | "w1" | "w" | "weekly" => Some(Self::W1),
            "1m" | "m1" | "m" | "monthly" => Some(Self::M1),
            _ => None,
        }
    }

    /// Yahoo Finance 간격 문자열 반환
    pub fn to_yahoo_str(self) -> &'static str {
        match self {
            Self::D1 => "1d",
            Self::W1 => "1wk",
            Self::M1 => "1mo",
        }
    }

    /// KIS API 기간 유형 반환
    #[allow(dead_code)]
    pub fn to_kis_period(self) -> &'static str {
        match self {
            Self::D1 => "D",
            Self::W1 => "W",
            Self::M1 => "M",
        }
    }
}

/// 다운로드 설정
pub struct DownloadConfig {
    pub market: Market,
    pub symbol: String,
    pub interval: Interval,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub output_path: String,
    /// 코스닥 종목 여부 (한국 시장 전용)
    pub is_kosdaq: bool,
}

/// OHLCV 데이터 포인트
#[derive(Debug, Clone)]
pub struct OhlcvData {
    pub date: String,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
}

/// Yahoo Finance API v8 응답 구조
#[derive(Debug, Deserialize)]
struct YahooChartResponse {
    chart: YahooChart,
}

#[derive(Debug, Deserialize)]
struct YahooChart {
    result: Option<Vec<YahooResult>>,
    error: Option<YahooError>,
}

#[derive(Debug, Deserialize)]
struct YahooError {
    code: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct YahooResult {
    timestamp: Option<Vec<i64>>,
    indicators: YahooIndicators,
}

#[derive(Debug, Deserialize)]
struct YahooIndicators {
    quote: Vec<YahooQuote>,
    #[serde(rename = "adjclose")]
    adj_close: Option<Vec<YahooAdjClose>>,
}

#[derive(Debug, Deserialize)]
struct YahooQuote {
    open: Option<Vec<Option<f64>>>,
    high: Option<Vec<Option<f64>>>,
    low: Option<Vec<Option<f64>>>,
    close: Option<Vec<Option<f64>>>,
    volume: Option<Vec<Option<i64>>>,
}

#[derive(Debug, Deserialize)]
struct YahooAdjClose {
    #[serde(rename = "adjclose")]
    adj_close: Option<Vec<Option<f64>>>,
}

/// 과거 데이터 다운로드 (데이터 소스 자동 선택)
pub async fn download_data(config: DownloadConfig) -> Result<usize> {
    info!(
        "Downloading {} {} data for {} from {} to {}",
        config.market_name(),
        config.interval_name(),
        config.symbol,
        config.start_date,
        config.end_date
    );

    // 1차: Yahoo Finance 시도
    match download_from_yahoo(&config).await {
        Ok(data) if !data.is_empty() => {
            info!(
                "Successfully fetched {} candles from Yahoo Finance",
                data.len()
            );
            return save_to_csv(&config, &data);
        }
        Ok(_) => {
            warn!("Yahoo Finance returned empty data, trying fallback...");
        }
        Err(e) => {
            warn!("Yahoo Finance failed: {}, trying fallback...", e);
        }
    }

    // 2차: KIS API fallback (현재는 미구현 - 추후 연동)
    warn!("KIS API fallback not yet implemented for historical data download");
    anyhow::bail!(
        "Failed to download data for {} from all sources. \
        Please check if the symbol is correct and the date range is valid.",
        config.symbol
    )
}

impl DownloadConfig {
    fn market_name(&self) -> &'static str {
        match self.market {
            Market::KR => "Korean",
            Market::US => "US",
        }
    }

    fn interval_name(&self) -> &'static str {
        match self.interval {
            Interval::D1 => "daily",
            Interval::W1 => "weekly",
            Interval::M1 => "monthly",
        }
    }

    /// Yahoo Finance 심볼 생성
    fn yahoo_symbol(&self) -> String {
        match self.market {
            Market::KR => {
                let suffix = if self.is_kosdaq { ".KQ" } else { ".KS" };
                format!("{}{}", self.symbol, suffix)
            }
            Market::US => self.symbol.to_uppercase(),
        }
    }
}

/// Yahoo Finance에서 데이터 다운로드
#[allow(clippy::needless_range_loop)]
async fn download_from_yahoo(config: &DownloadConfig) -> Result<Vec<OhlcvData>> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    let yahoo_symbol = config.yahoo_symbol();

    // 날짜를 UNIX 타임스탬프로 변환
    let start_ts = Utc
        .from_utc_datetime(&config.start_date.and_hms_opt(0, 0, 0).unwrap())
        .timestamp();
    let end_ts = Utc
        .from_utc_datetime(&config.end_date.and_hms_opt(23, 59, 59).unwrap())
        .timestamp();

    // Yahoo Finance API v8 URL
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?period1={}&period2={}&interval={}&events=history",
        yahoo_symbol,
        start_ts,
        end_ts,
        config.interval.to_yahoo_str()
    );

    debug!("Fetching from Yahoo Finance: {}", url);

    // 진행률 표시줄
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(format!("Fetching {} from Yahoo Finance...", yahoo_symbol));

    let response = client
        .get(&url)
        .send()
        .await
        .with_context(|| "Failed to send request to Yahoo Finance")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Yahoo Finance API error: {} - {}", status, body);
    }

    let body = response.text().await?;
    debug!("Yahoo Finance response length: {} bytes", body.len());

    let chart_response: YahooChartResponse =
        serde_json::from_str(&body).with_context(|| "Failed to parse Yahoo Finance response")?;

    // 에러 체크
    if let Some(error) = chart_response.chart.error {
        anyhow::bail!(
            "Yahoo Finance error: {} - {}",
            error.code,
            error.description
        );
    }

    // 결과 파싱
    let result = chart_response
        .chart
        .result
        .and_then(|r| r.into_iter().next())
        .ok_or_else(|| anyhow::anyhow!("No data returned from Yahoo Finance"))?;

    let timestamps = result.timestamp.unwrap_or_default();
    let quote = result
        .indicators
        .quote
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No quote data in response"))?;

    let opens = quote.open.unwrap_or_default();
    let highs = quote.high.unwrap_or_default();
    let lows = quote.low.unwrap_or_default();
    let closes = quote.close.unwrap_or_default();
    let volumes = quote.volume.unwrap_or_default();

    // 조정 종가 사용 (있는 경우)
    let adj_closes = result
        .indicators
        .adj_close
        .and_then(|ac| ac.into_iter().next())
        .and_then(|ac| ac.adj_close);

    let mut data = Vec::new();
    let len = timestamps.len();

    for i in 0..len {
        // 모든 필드가 유효한 경우만 추가
        let open = opens.get(i).and_then(|v| *v);
        let high = highs.get(i).and_then(|v| *v);
        let low = lows.get(i).and_then(|v| *v);
        let close = adj_closes
            .as_ref()
            .and_then(|ac| ac.get(i).and_then(|v| *v))
            .or_else(|| closes.get(i).and_then(|v| *v));
        let volume = volumes.get(i).and_then(|v| *v);

        if let (Some(o), Some(h), Some(l), Some(c), Some(v)) = (open, high, low, close, volume) {
            let ts = timestamps[i];
            let date = chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| ts.to_string());

            data.push(OhlcvData {
                date,
                open: Decimal::from_str(&format!("{:.4}", o)).unwrap_or_default(),
                high: Decimal::from_str(&format!("{:.4}", h)).unwrap_or_default(),
                low: Decimal::from_str(&format!("{:.4}", l)).unwrap_or_default(),
                close: Decimal::from_str(&format!("{:.4}", c)).unwrap_or_default(),
                volume: Decimal::from(v),
            });
        }
    }

    // 날짜순 정렬 (오래된 것부터)
    data.sort_by(|a, b| a.date.cmp(&b.date));

    pb.finish_with_message(format!(
        "Downloaded {} candles from Yahoo Finance",
        data.len()
    ));

    Ok(data)
}

/// CSV 파일로 저장
fn save_to_csv(config: &DownloadConfig, data: &[OhlcvData]) -> Result<usize> {
    let output_path = Path::new(&config.output_path);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(output_path)
        .with_context(|| format!("Failed to create output file: {}", config.output_path))?;
    let mut writer = BufWriter::new(file);

    // CSV 헤더 작성
    writeln!(writer, "date,open,high,low,close,volume")?;

    // 데이터 작성
    for candle in data {
        writeln!(
            writer,
            "{},{},{},{},{},{}",
            candle.date, candle.open, candle.high, candle.low, candle.close, candle.volume
        )?;
    }

    writer.flush()?;

    info!("Saved {} candles to {}", data.len(), config.output_path);

    Ok(data.len())
}

/// 날짜 문자열 파싱 (YYYY-MM-DD)
pub fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .with_context(|| format!("Invalid date format: {}. Expected YYYY-MM-DD", s))
}

/// 인기 한국 ETF 목록
pub fn popular_kr_etfs() -> Vec<(&'static str, &'static str)> {
    vec![
        ("069500", "KODEX 200"),
        ("122630", "KODEX 레버리지"),
        ("252670", "KODEX 200선물인버스2X"),
        ("305540", "TIGER 2차전지테마"),
        ("379800", "KODEX 미국S&P500TR"),
        ("371460", "TIGER 차이나전기차SOLACTIVE"),
        ("364690", "KODEX Fn반도체"),
        ("139260", "TIGER 200IT"),
        ("091230", "TIGER 반도체"),
        ("102110", "TIGER 200"),
    ]
}

/// 인기 미국 ETF 목록
pub fn popular_us_etfs() -> Vec<(&'static str, &'static str)> {
    vec![
        ("SPY", "S&P 500 ETF"),
        ("QQQ", "Nasdaq 100 ETF"),
        ("TQQQ", "3x Nasdaq ETF"),
        ("SQQQ", "3x Inverse Nasdaq"),
        ("TLT", "20년 국채 ETF"),
        ("IEF", "7-10년 국채 ETF"),
        ("VEA", "선진국 ETF"),
        ("VWO", "신흥국 ETF"),
        ("GLD", "Gold ETF"),
        ("SCHD", "배당 ETF"),
    ]
}

/// 사용 가능한 종목 목록 출력
pub fn print_available_symbols(market: Market) {
    match market {
        Market::KR => {
            println!("\n인기 한국 ETF:");
            println!("{:-<50}", "");
            for (code, name) in popular_kr_etfs() {
                println!("  {} - {}", code, name);
            }
            println!("\n* 코스닥 종목은 --kosdaq 플래그를 사용하세요.");
        }
        Market::US => {
            println!("\n인기 미국 ETF:");
            println!("{:-<50}", "");
            for (symbol, name) in popular_us_etfs() {
                println!("  {} - {}", symbol, name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_market_parsing() {
        assert_eq!(Market::parse("KR"), Some(Market::KR));
        assert_eq!(Market::parse("korea"), Some(Market::KR));
        assert_eq!(Market::parse("US"), Some(Market::US));
        assert_eq!(Market::parse("nasdaq"), Some(Market::US));
        assert_eq!(Market::parse("invalid"), None);
    }

    #[test]
    fn test_interval_parsing() {
        assert!(matches!(Interval::parse("1d"), Some(Interval::D1)));
        assert!(matches!(Interval::parse("daily"), Some(Interval::D1)));
        assert!(matches!(Interval::parse("1w"), Some(Interval::W1)));
        assert!(matches!(Interval::parse("1m"), Some(Interval::M1)));
        assert!(Interval::parse("invalid").is_none());
    }

    #[test]
    fn test_yahoo_symbol_generation() {
        let kr_config = DownloadConfig {
            market: Market::KR,
            symbol: "005930".to_string(),
            interval: Interval::D1,
            start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            output_path: "test.csv".to_string(),
            is_kosdaq: false,
        };
        assert_eq!(kr_config.yahoo_symbol(), "005930.KS");

        let kosdaq_config = DownloadConfig {
            market: Market::KR,
            symbol: "035720".to_string(),
            interval: Interval::D1,
            start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            output_path: "test.csv".to_string(),
            is_kosdaq: true,
        };
        assert_eq!(kosdaq_config.yahoo_symbol(), "035720.KQ");

        let us_config = DownloadConfig {
            market: Market::US,
            symbol: "spy".to_string(),
            interval: Interval::D1,
            start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            output_path: "test.csv".to_string(),
            is_kosdaq: false,
        };
        assert_eq!(us_config.yahoo_symbol(), "SPY");
    }

    #[test]
    fn test_date_parsing() {
        let date = parse_date("2024-01-15").unwrap();
        assert_eq!(date.year(), 2024);
        assert_eq!(date.month(), 1);
        assert_eq!(date.day(), 15);
    }
}
