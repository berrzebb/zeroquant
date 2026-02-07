//! 시뮬레이션 거래소를 위한 데이터 피드.
//!
//! 백테스팅을 위한 과거 데이터 로딩 및 재생을 관리합니다.

#![allow(dead_code)] // 테스트/디버깅용 유틸리티 함수

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use trader_core::{Kline, Ticker, Timeframe};

use crate::ExchangeError;

/// 데이터 피드 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFeedConfig {
    /// 재생용 기본 타임프레임
    pub default_timeframe: Timeframe,
    /// 재생 속도 배율 (1.0 = 실시간, 0 = 즉시)
    pub playback_speed: f64,
    /// 끝에 도달했을 때 데이터를 반복할지 여부
    pub loop_data: bool,
}

impl Default for DataFeedConfig {
    fn default() -> Self {
        Self {
            default_timeframe: Timeframe::M1,
            playback_speed: 0.0, // 백테스팅을 위한 즉시 재생
            loop_data: false,
        }
    }
}

/// 타임스탬프를 키로 하는 과거 데이터 항목.
#[derive(Debug, Clone)]
struct DataEntry {
    kline: Kline,
    ticker: Ticker,
}

/// 과거 데이터를 위한 데이터 피드 관리자.
pub struct DataFeed {
    /// 설정
    config: DataFeedConfig,
    /// 심볼 및 타임프레임별 과거 데이터 (ticker String 사용)
    /// 타임스탬프 순서 접근을 위한 BTreeMap
    data: HashMap<(String, Timeframe), BTreeMap<DateTime<Utc>, DataEntry>>,
    /// 심볼별 현재 재생 위치 (ticker String 사용)
    playback_position: HashMap<String, DateTime<Utc>>,
    /// 데이터 시작 시간
    start_time: Option<DateTime<Utc>>,
    /// 데이터 종료 시간
    end_time: Option<DateTime<Utc>>,
    /// 현재 시뮬레이션 시간
    current_time: Option<DateTime<Utc>>,
}

impl DataFeed {
    /// 새로운 데이터 피드를 생성합니다.
    pub fn new(config: DataFeedConfig) -> Self {
        Self {
            config,
            data: HashMap::new(),
            playback_position: HashMap::new(),
            start_time: None,
            end_time: None,
            current_time: None,
        }
    }

    /// 심볼의 Kline 데이터를 로드합니다.
    pub fn load_klines(&mut self, symbol: String, timeframe: Timeframe, klines: Vec<Kline>) {
        let mut data_map = BTreeMap::new();

        for kline in klines {
            let ticker = Self::kline_to_ticker(&kline);
            let entry = DataEntry {
                kline: kline.clone(),
                ticker,
            };
            data_map.insert(kline.open_time, entry);

            // 시간 범위 업데이트
            if self.start_time.is_none() || kline.open_time < self.start_time.unwrap() {
                self.start_time = Some(kline.open_time);
            }
            if self.end_time.is_none() || kline.close_time > self.end_time.unwrap() {
                self.end_time = Some(kline.close_time);
            }
        }

        self.data.insert((symbol, timeframe), data_map);
    }

    /// CSV 파일에서 Kline 데이터를 로드합니다.
    /// 예상 형식: timestamp,open,high,low,close,volume
    pub fn load_from_csv(
        &mut self,
        symbol: String,
        timeframe: Timeframe,
        path: impl AsRef<Path>,
    ) -> Result<usize, ExchangeError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| ExchangeError::ParseError(format!("Failed to read CSV file: {}", e)))?;

        let mut klines = Vec::new();
        let tf_duration =
            Duration::from_std(timeframe.duration()).unwrap_or_else(|_| Duration::minutes(1));

        for (line_no, line) in content.lines().enumerate() {
            // 헤더 건너뛰기
            let line_str: &str = line;
            if line_no == 0 && line_str.contains("timestamp") {
                continue;
            }

            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() < 6 {
                continue;
            }

            let open_time = parts[0]
                .parse::<i64>()
                .map(|ts| DateTime::from_timestamp_millis(ts).unwrap_or_default())
                .map_err(|e| {
                    ExchangeError::ParseError(format!(
                        "Invalid timestamp at line {}: {}",
                        line_no, e
                    ))
                })?;

            let close_time = open_time + tf_duration;

            let kline = Kline {
                ticker: symbol.to_string(),
                timeframe,
                open_time,
                close_time,
                open: parts[1].parse().unwrap_or_default(),
                high: parts[2].parse().unwrap_or_default(),
                low: parts[3].parse().unwrap_or_default(),
                close: parts[4].parse().unwrap_or_default(),
                volume: parts[5].parse().unwrap_or_default(),
                quote_volume: if parts.len() > 6 {
                    Some(parts[6].parse().unwrap_or_default())
                } else {
                    None
                },
                num_trades: if parts.len() > 7 {
                    Some(parts[7].parse().unwrap_or(0))
                } else {
                    None
                },
            };

            klines.push(kline);
        }

        let count = klines.len();
        self.load_klines(symbol, timeframe, klines);

        Ok(count)
    }

    /// 현재 가격 정보를 위해 Kline을 티커로 변환합니다.
    fn kline_to_ticker(kline: &Kline) -> Ticker {
        let price_change = kline.close - kline.open;
        let price_change_pct = if kline.open != dec!(0) {
            (price_change / kline.open) * dec!(100)
        } else {
            dec!(0)
        };

        Ticker {
            ticker: kline.ticker.clone(),
            last: kline.close,
            bid: kline.close * dec!(0.9999), // 시뮬레이션된 매수호가
            ask: kline.close * dec!(1.0001), // 시뮬레이션된 매도호가
            high_24h: kline.high,
            low_24h: kline.low,
            volume_24h: kline.volume,
            change_24h: price_change,
            change_24h_percent: price_change_pct,
            timestamp: kline.close_time,
        }
    }

    /// 재생을 처음으로 리셋합니다.
    pub fn reset(&mut self) {
        self.playback_position.clear();
        self.current_time = self.start_time;
    }

    /// 재생 위치를 특정 시간으로 설정합니다.
    pub fn seek(&mut self, time: DateTime<Utc>) {
        self.current_time = Some(time);
        self.playback_position.clear();
    }

    /// 현재 시뮬레이션 시간을 가져옵니다.
    pub fn current_time(&self) -> Option<DateTime<Utc>> {
        self.current_time
    }

    /// 데이터 시간 범위를 가져옵니다.
    pub fn time_range(&self) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => Some((start, end)),
            _ => None,
        }
    }

    /// 심볼의 다음 Kline을 가져옵니다.
    /// 재생 위치를 진행시킵니다.
    pub fn next_kline(&mut self, ticker: &str, timeframe: Timeframe) -> Option<Kline> {
        let key = (ticker.to_string(), timeframe);
        let data = self.data.get(&key)?;

        let current_pos = self.playback_position.get(ticker).copied();

        let next_entry = if let Some(pos) = current_pos {
            // 현재 위치 이후의 첫 번째 항목 찾기
            data.range((std::ops::Bound::Excluded(pos), std::ops::Bound::Unbounded))
                .next()
        } else {
            // 처음부터 시작
            data.iter().next()
        };

        if let Some((timestamp, entry)) = next_entry {
            self.playback_position
                .insert(ticker.to_string(), *timestamp);
            self.current_time = Some(entry.kline.close_time);
            Some(entry.kline.clone())
        } else if self.config.loop_data {
            // 처음으로 되돌아가기
            self.playback_position.remove(ticker);
            self.next_kline(ticker, timeframe)
        } else {
            None
        }
    }

    /// 심볼의 현재 티커를 가져옵니다.
    pub fn get_ticker(&self, ticker: &str) -> Option<Ticker> {
        let key = (ticker.to_string(), self.config.default_timeframe);
        let data = self.data.get(&key)?;

        let pos = self.playback_position.get(ticker)?;
        data.get(pos).map(|e| e.ticker.clone())
    }

    /// 진행 없이 심볼의 현재 Kline을 가져옵니다.
    pub fn get_current_kline(&self, ticker: &str, timeframe: Timeframe) -> Option<Kline> {
        let key = (ticker.to_string(), timeframe);
        let data = self.data.get(&key)?;

        let pos = self.playback_position.get(ticker)?;
        data.get(pos).map(|e| e.kline.clone())
    }

    /// 현재 시간까지의 과거 Kline을 가져옵니다.
    pub fn get_historical_klines(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Vec<Kline> {
        // ticker String으로 직접 키 생성
        let key = (symbol.to_string(), timeframe);
        let data = match self.data.get(&key) {
            Some(d) => d,
            None => return vec![],
        };

        let current_time = self.current_time.unwrap_or_else(Utc::now);

        data.range(..=current_time)
            .rev()
            .take(limit)
            .map(|(_, entry)| entry.kline.clone())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// 심볼의 현재 가격을 가져옵니다.
    pub fn get_current_price(&self, ticker: &str) -> Option<Decimal> {
        self.get_ticker(ticker).map(|t| t.last)
    }

    /// 모든 심볼의 데이터가 소진되었는지 확인합니다.
    pub fn is_exhausted(&self) -> bool {
        if self.data.is_empty() {
            return true;
        }

        for ((symbol, _timeframe), data) in &self.data {
            if let Some(pos) = self.playback_position.get(symbol) {
                // 현재 위치 이후에 더 많은 데이터가 있는지 확인
                if data
                    .range((std::ops::Bound::Excluded(*pos), std::ops::Bound::Unbounded))
                    .next()
                    .is_some()
                {
                    return false;
                }
            } else {
                // 아직 재생을 시작하지 않음
                if !data.is_empty() {
                    return false;
                }
            }
        }

        true
    }

    /// 로드된 모든 심볼 ticker를 가져옵니다.
    pub fn symbols(&self) -> Vec<String> {
        self.data
            .keys()
            .map(|(ticker, _)| ticker.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// 로드된 데이터 수를 가져옵니다.
    pub fn data_count(&self, ticker: &str, timeframe: Timeframe) -> usize {
        let key = (ticker.to_string(), timeframe);
        self.data.get(&key).map(|d| d.len()).unwrap_or(0)
    }
}

/// 테스트용 샘플 Kline을 생성합니다.
pub fn generate_sample_klines(
    symbol: String,
    timeframe: Timeframe,
    count: usize,
    start_price: Decimal,
    volatility: Decimal,
) -> Vec<Kline> {
    use rand::Rng;

    let mut klines = Vec::with_capacity(count);
    let mut rng = rand::thread_rng();
    let mut current_price = start_price;

    let tf_duration =
        Duration::from_std(timeframe.duration()).unwrap_or_else(|_| Duration::minutes(1));
    let mut current_time = Utc::now() - tf_duration * count as i32;

    let volatility_f64 = volatility.to_string().parse::<f64>().unwrap_or(0.02);

    for _ in 0..count {
        let change_pct = (rng.gen::<f64>() - 0.5) * 2.0 * volatility_f64;
        let change = current_price * Decimal::from_f64_retain(change_pct).unwrap_or_default();

        let open = current_price;
        let close = current_price + change;

        let high_extra = current_price.abs()
            * Decimal::from_f64_retain(rng.gen::<f64>() * 0.01).unwrap_or_default();
        let low_extra = current_price.abs()
            * Decimal::from_f64_retain(rng.gen::<f64>() * 0.01).unwrap_or_default();

        let high = open.max(close) + high_extra;
        let low = open.min(close) - low_extra;

        let volume = Decimal::from_f64_retain(rng.gen_range(10.0..1000.0)).unwrap_or(dec!(100));

        klines.push(Kline {
            ticker: symbol.to_string(),
            timeframe,
            open_time: current_time,
            close_time: current_time + tf_duration,
            open,
            high,
            low,
            close,
            volume,
            quote_volume: Some(volume * close),
            num_trades: Some(rng.gen_range(10..500)),
        });

        current_price = close;
        current_time += tf_duration;
    }

    klines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_symbol() -> String {
        "BTC/USDT".to_string()
    }

    #[test]
    fn test_load_klines() {
        let mut feed = DataFeed::new(DataFeedConfig::default());
        let symbol = create_test_symbol();
        let klines =
            generate_sample_klines(symbol.clone(), Timeframe::M1, 100, dec!(50000), dec!(0.02));

        feed.load_klines(symbol.clone(), Timeframe::M1, klines);

        assert_eq!(feed.data_count(&symbol, Timeframe::M1), 100);
        assert!(feed.time_range().is_some());
    }

    #[test]
    fn test_playback() {
        let mut feed = DataFeed::new(DataFeedConfig::default());
        let symbol = create_test_symbol();
        let klines =
            generate_sample_klines(symbol.clone(), Timeframe::M1, 10, dec!(50000), dec!(0.02));

        feed.load_klines(symbol.clone(), Timeframe::M1, klines);

        // 첫 번째 Kline 가져오기
        let kline1 = feed.next_kline(&symbol, Timeframe::M1);
        assert!(kline1.is_some());

        // 두 번째 Kline 가져오기
        let kline2 = feed.next_kline(&symbol, Timeframe::M1);
        assert!(kline2.is_some());
        assert_ne!(kline1.unwrap().open_time, kline2.unwrap().open_time);

        // 전체 재생
        let mut count = 2;
        while feed.next_kline(&symbol, Timeframe::M1).is_some() {
            count += 1;
        }
        assert_eq!(count, 10);
    }

    #[test]
    fn test_historical_klines() {
        let mut feed = DataFeed::new(DataFeedConfig::default());
        let symbol = create_test_symbol();
        let klines =
            generate_sample_klines(symbol.clone(), Timeframe::M1, 100, dec!(50000), dec!(0.02));

        feed.load_klines(symbol.clone(), Timeframe::M1, klines);

        // 중간까지 진행
        for _ in 0..50 {
            feed.next_kline(&symbol, Timeframe::M1);
        }

        // 마지막 20개 Kline 가져오기
        let historical = feed.get_historical_klines(&symbol, Timeframe::M1, 20);
        assert_eq!(historical.len(), 20);
    }

    #[test]
    fn test_loop_data() {
        let config = DataFeedConfig {
            loop_data: true,
            ..Default::default()
        };
        let mut feed = DataFeed::new(config);
        let symbol = create_test_symbol();
        let klines =
            generate_sample_klines(symbol.clone(), Timeframe::M1, 5, dec!(50000), dec!(0.02));

        feed.load_klines(symbol.clone(), Timeframe::M1, klines);

        // 전체 재생 + 3개 더 (반복되어야 함)
        for _ in 0..8 {
            let kline = feed.next_kline(&symbol, Timeframe::M1);
            assert!(kline.is_some());
        }
    }
}
