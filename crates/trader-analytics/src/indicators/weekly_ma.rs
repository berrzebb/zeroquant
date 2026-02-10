//! 주봉 기반 이동평균 (Weekly MA).
//!
//! 일봉 데이터를 주봉으로 리샘플링하고 이동평균을 계산합니다.
//! 중기 추세 분석에 유용합니다.
//!
//! # 리샘플링 규칙
//!
//! - **Open**: 주 첫 거래일의 시가
//! - **High**: 주 중 최고가
//! - **Low**: 주 중 최저가
//! - **Close**: 주 마지막 거래일의 종가
//! - **Volume**: 주 전체 거래량 합계
//!
//! # 예시
//!
//! ```rust,ignore
//! use trader_analytics::indicators::weekly_ma::{resample_to_weekly, calculate_weekly_ma};
//! use trader_core::Kline;
//!
//! let daily_klines = vec![/* 100일 일봉 데이터 */];
//!
//! // 주봉 변환
//! let weekly_klines = resample_to_weekly(&daily_klines);
//!
//! // 주봉 MA20 계산
//! let weekly_ma20 = calculate_weekly_ma(&daily_klines, 20);
//!
//! // 현재 주봉 MA20 조회
//! if let Some(current_ma) = weekly_ma20.last() {
//!     println!("주봉 MA20: {}", current_ma.value);
//! }
//! ```

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use trader_core::{Kline, Timeframe};

/// 주봉 MA 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyMaResult {
    /// 주 시작 날짜 (해당 주의 월요일)
    pub week_start: NaiveDate,
    /// MA 값
    pub value: Decimal,
    /// 주봉 종가 (MA 비교용)
    pub weekly_close: Decimal,
    /// MA 대비 이격도 (%)
    pub distance_pct: Decimal,
}

/// 일봉 → 주봉 리샘플링.
///
/// 일봉 데이터를 주봉으로 변환합니다.
/// 주 경계는 월요일 기준이며, 불완전한 주(마지막 주)도 포함합니다.
///
/// # 인자
///
/// * `daily_klines` - 일봉 데이터 (날짜 오름차순 정렬)
///
/// # 반환
///
/// 주봉 데이터 목록
pub fn resample_to_weekly(daily_klines: &[Kline]) -> Vec<Kline> {
    if daily_klines.is_empty() {
        return Vec::new();
    }

    let mut weekly_klines: Vec<Kline> = Vec::new();
    let mut current_week_klines: Vec<&Kline> = Vec::new();
    let mut current_week_number: Option<(i32, u32)> = None; // (year, week)

    for kline in daily_klines {
        let date = kline.open_time.date_naive();
        let iso_week = date.iso_week();
        let week_key = (iso_week.year(), iso_week.week());

        if current_week_number.is_none() {
            current_week_number = Some(week_key);
        }

        if current_week_number != Some(week_key) {
            // 이전 주 캔들 생성
            if !current_week_klines.is_empty() {
                if let Some(weekly) = create_weekly_candle(&current_week_klines) {
                    weekly_klines.push(weekly);
                }
            }
            current_week_klines.clear();
            current_week_number = Some(week_key);
        }

        current_week_klines.push(kline);
    }

    // 마지막 주 처리
    if !current_week_klines.is_empty() {
        if let Some(weekly) = create_weekly_candle(&current_week_klines) {
            weekly_klines.push(weekly);
        }
    }

    weekly_klines
}

/// 주별 캔들 생성.
fn create_weekly_candle(daily_klines: &[&Kline]) -> Option<Kline> {
    if daily_klines.is_empty() {
        return None;
    }

    let first = daily_klines.first()?;
    let last = daily_klines.last()?;

    let open = first.open;
    let close = last.close;
    let high = daily_klines.iter().map(|k| k.high).max()?;
    let low = daily_klines.iter().map(|k| k.low).min()?;
    let volume: Decimal = daily_klines.iter().map(|k| k.volume).sum();

    Some(Kline {
        ticker: first.ticker.clone(),
        timeframe: Timeframe::W1,
        open_time: first.open_time,
        open,
        high,
        low,
        close,
        volume,
        close_time: last.close_time,
        quote_volume: None,
        num_trades: None,
    })
}

/// 주봉 MA 계산.
///
/// 일봉 데이터를 주봉으로 변환 후 MA를 계산합니다.
///
/// # 인자
///
/// * `daily_klines` - 일봉 데이터
/// * `period` - MA 기간 (기본: 20)
///
/// # 반환
///
/// 주봉 MA 결과 목록
pub fn calculate_weekly_ma(daily_klines: &[Kline], period: usize) -> Vec<WeeklyMaResult> {
    let weekly_klines = resample_to_weekly(daily_klines);

    if weekly_klines.len() < period {
        return Vec::new();
    }

    let mut results: Vec<WeeklyMaResult> = Vec::new();

    for i in (period - 1)..weekly_klines.len() {
        let slice = &weekly_klines[(i + 1 - period)..=i];
        let sum: Decimal = slice.iter().map(|k| k.close).sum();
        let ma_value = sum / Decimal::from(period);

        let current = &weekly_klines[i];
        let weekly_close = current.close;

        // 이격도 계산
        let distance_pct = if ma_value > Decimal::ZERO {
            ((weekly_close - ma_value) / ma_value) * dec!(100)
        } else {
            Decimal::ZERO
        };

        let week_start = get_week_start(current.open_time);

        results.push(WeeklyMaResult {
            week_start,
            value: ma_value,
            weekly_close,
            distance_pct,
        });
    }

    results
}

/// 주 시작일 (월요일) 계산.
fn get_week_start(dt: DateTime<Utc>) -> NaiveDate {
    let date = dt.date_naive();
    let weekday = date.weekday();
    let days_from_monday = weekday.num_days_from_monday();
    date - chrono::Duration::days(days_from_monday as i64)
}

/// 일봉에 주봉 MA 매핑.
///
/// 각 일봉에 해당 주의 MA 값을 매핑합니다.
/// 전략에서 일봉 기준 판단 시 주봉 MA를 함께 참조할 때 사용합니다.
///
/// # 인자
///
/// * `daily_klines` - 일봉 데이터
/// * `period` - MA 기간 (기본: 20)
///
/// # 반환
///
/// 날짜 → 주봉 MA 매핑
pub fn map_weekly_ma_to_daily(
    daily_klines: &[Kline],
    period: usize,
) -> std::collections::HashMap<NaiveDate, Decimal> {
    use std::collections::HashMap;

    let weekly_ma = calculate_weekly_ma(daily_klines, period);

    // 주 시작일 → MA 값 매핑
    let week_ma_map: HashMap<NaiveDate, Decimal> =
        weekly_ma.iter().map(|r| (r.week_start, r.value)).collect();

    // 일봉 날짜 → 해당 주의 MA 값 매핑
    let mut daily_map: HashMap<NaiveDate, Decimal> = HashMap::new();

    for kline in daily_klines {
        let date = kline.open_time.date_naive();
        let week_start = get_week_start(kline.open_time);

        // 해당 주 또는 이전 주의 MA 찾기
        if let Some(&ma) = week_ma_map.get(&week_start) {
            daily_map.insert(date, ma);
        } else {
            // 이전 주 MA 찾기
            let prev_week = week_start - chrono::Duration::days(7);
            if let Some(&ma) = week_ma_map.get(&prev_week) {
                daily_map.insert(date, ma);
            }
        }
    }

    daily_map
}

/// 현재 가격의 주봉 MA20 대비 위치.
///
/// # 반환값
///
/// - 양수: 주봉 MA20 위에 있음 (상승 추세)
/// - 음수: 주봉 MA20 아래에 있음 (하락 추세)
pub fn get_current_weekly_ma_distance(daily_klines: &[Kline], period: usize) -> Option<Decimal> {
    let weekly_ma = calculate_weekly_ma(daily_klines, period);
    weekly_ma.last().map(|r| r.distance_pct)
}

/// 주봉 MA 골든크로스/데드크로스 감지.
///
/// 짧은 기간 MA가 긴 기간 MA를 상향/하향 돌파했는지 확인합니다.
///
/// # 반환
///
/// - Some(true): 골든크로스 (상향 돌파)
/// - Some(false): 데드크로스 (하향 돌파)
/// - None: 신호 없음
pub fn detect_weekly_ma_cross(
    daily_klines: &[Kline],
    short_period: usize,
    long_period: usize,
) -> Option<bool> {
    let short_ma = calculate_weekly_ma(daily_klines, short_period);
    let long_ma = calculate_weekly_ma(daily_klines, long_period);

    if short_ma.len() < 2 || long_ma.len() < 2 {
        return None;
    }

    let short_len = short_ma.len();
    let long_len = long_ma.len();

    // 가장 최근 2개 비교
    let curr_short = short_ma[short_len - 1].value;
    let prev_short = short_ma[short_len - 2].value;
    let curr_long = long_ma[long_len - 1].value;
    let prev_long = long_ma[long_len - 2].value;

    // 골든크로스: 이전에 아래였다가 현재 위로
    if prev_short < prev_long && curr_short > curr_long {
        return Some(true);
    }

    // 데드크로스: 이전에 위였다가 현재 아래로
    if prev_short > prev_long && curr_short < curr_long {
        return Some(false);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Weekday};
    use rust_decimal_macros::dec;

    fn create_test_daily_klines() -> Vec<Kline> {
        // 2024년 1월 - 4주 + α (약 22일)
        let base_price = dec!(100);
        let mut klines = Vec::new();

        for day in 1..=22 {
            let date = Utc.with_ymd_and_hms(2024, 1, day, 9, 0, 0).unwrap();
            let weekday = date.weekday();

            // 주말 제외
            if weekday == Weekday::Sat || weekday == Weekday::Sun {
                continue;
            }

            let price = base_price + Decimal::from(day);

            klines.push(Kline {
                ticker: "TEST".to_string(),
                timeframe: Timeframe::D1,
                open_time: date,
                open: price - dec!(0.5),
                high: price + dec!(1),
                low: price - dec!(1),
                close: price,
                volume: dec!(1000000),
                close_time: date + chrono::Duration::hours(9),
                quote_volume: None,
                num_trades: None,
            });
        }

        klines
    }

    fn create_extended_test_klines() -> Vec<Kline> {
        // 140일 (28주) 데이터 - MA20 계산 가능
        let base_price = dec!(100);
        let mut klines = Vec::new();
        let mut current_date = Utc.with_ymd_and_hms(2023, 6, 1, 9, 0, 0).unwrap();

        for day in 0..200 {
            // 주말 스킵
            let weekday = current_date.weekday();
            if weekday == Weekday::Sat || weekday == Weekday::Sun {
                current_date += chrono::Duration::days(1);
                continue;
            }

            // 가격 트렌드: 서서히 상승
            let trend = Decimal::from(day) * dec!(0.1);
            let price = base_price + trend;

            klines.push(Kline {
                ticker: "TEST".to_string(),
                timeframe: Timeframe::D1,
                open_time: current_date,
                open: price - dec!(0.5),
                high: price + dec!(1),
                low: price - dec!(1),
                close: price,
                volume: dec!(1000000),
                close_time: current_date + chrono::Duration::hours(9),
                quote_volume: None,
                num_trades: None,
            });

            current_date += chrono::Duration::days(1);
        }

        klines
    }

    #[test]
    fn test_resample_to_weekly() {
        let daily = create_test_daily_klines();
        let weekly = resample_to_weekly(&daily);

        // 3주 완료 + 1주 진행중 = 최대 4주
        assert!(weekly.len() >= 3);
        assert!(weekly.len() <= 4);

        // 주봉 타임프레임 확인
        for kline in &weekly {
            assert_eq!(kline.timeframe, Timeframe::W1);
        }
    }

    #[test]
    fn test_weekly_ohlcv_aggregation() {
        let daily = create_test_daily_klines();
        let weekly = resample_to_weekly(&daily);

        if let Some(first_week) = weekly.first() {
            // High는 주 중 최고가
            // Low는 주 중 최저가
            assert!(first_week.high >= first_week.open);
            assert!(first_week.high >= first_week.close);
            assert!(first_week.low <= first_week.open);
            assert!(first_week.low <= first_week.close);

            // Volume은 합계이므로 일봉보다 커야 함
            assert!(first_week.volume > dec!(1000000));
        }
    }

    #[test]
    fn test_calculate_weekly_ma20() {
        let daily = create_extended_test_klines();
        let weekly_ma = calculate_weekly_ma(&daily, 20);

        // MA20 계산 가능해야 함 (최소 20주 필요)
        assert!(!weekly_ma.is_empty());

        // MA 값이 양수여야 함
        for result in &weekly_ma {
            assert!(result.value > Decimal::ZERO);
        }
    }

    #[test]
    fn test_map_weekly_ma_to_daily() {
        let daily = create_extended_test_klines();
        let daily_map = map_weekly_ma_to_daily(&daily, 20);

        // 일봉 대부분에 MA 매핑 있어야 함
        assert!(!daily_map.is_empty());
    }

    #[test]
    fn test_distance_pct() {
        let daily = create_extended_test_klines();
        let distance = get_current_weekly_ma_distance(&daily, 20);

        // 상승 추세이므로 양수 이격도 예상
        assert!(distance.is_some());
    }

    #[test]
    fn test_empty_input() {
        let empty: Vec<Kline> = Vec::new();
        let weekly = resample_to_weekly(&empty);
        assert!(weekly.is_empty());

        let ma = calculate_weekly_ma(&empty, 20);
        assert!(ma.is_empty());
    }

    #[test]
    fn test_insufficient_data() {
        let daily = create_test_daily_klines(); // 약 15일
        let ma = calculate_weekly_ma(&daily, 20);

        // 20주 데이터 없으므로 MA20 계산 불가
        assert!(ma.is_empty());
    }
}
