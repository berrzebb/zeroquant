//! 시장 운영 시간 기반 스케줄러.
//!
//! 각 시장의 운영 시간을 고려하여 워크플로우 실행 시점을 결정합니다.

use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, Timelike, Utc, Weekday};
use chrono_tz::Tz;
use std::collections::HashSet;
use tracing::{debug, info};

use crate::config::SchedulingConfig;

/// 시장 운영 시간 정보
#[derive(Debug, Clone)]
pub struct MarketHours {
    /// 시장 코드 (KR, US 등)
    pub market: String,
    /// 시장 타임존
    pub timezone: Tz,
    /// 장 시작 시간 (현지 시간)
    pub open_time: NaiveTime,
    /// 장 마감 시간 (현지 시간)
    pub close_time: NaiveTime,
}

impl MarketHours {
    /// KRX 시장 (한국)
    pub fn krx() -> Self {
        Self {
            market: "KR".to_string(),
            timezone: chrono_tz::Asia::Seoul,
            open_time: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            close_time: NaiveTime::from_hms_opt(15, 30, 0).unwrap(),
        }
    }

    /// US 시장 (뉴욕)
    pub fn us() -> Self {
        Self {
            market: "US".to_string(),
            timezone: chrono_tz::America::New_York,
            open_time: NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
            close_time: NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
        }
    }

    /// JP 시장 (일본)
    pub fn jp() -> Self {
        Self {
            market: "JP".to_string(),
            timezone: chrono_tz::Asia::Tokyo,
            open_time: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            close_time: NaiveTime::from_hms_opt(15, 30, 0).unwrap(),
        }
    }
}

/// 시장 상태
#[derive(Debug, Clone, PartialEq)]
pub enum MarketStatus {
    /// 장중 (거래 시간)
    Open,
    /// 장 마감 (데이터 수집 가능)
    Closed,
    /// 휴장 (주말 또는 공휴일)
    Holiday,
}

/// 시장 기반 스케줄러
pub struct Scheduler {
    /// 시장별 운영 시간
    markets: Vec<MarketHours>,
    /// 공휴일 목록 (시장코드:날짜)
    holidays: HashSet<String>,
    /// 설정
    config: SchedulingConfig,
    /// 마지막 일일 워크플로우 실행 날짜 (시장코드별)
    last_daily_run: std::collections::HashMap<String, NaiveDate>,
}

impl Scheduler {
    /// 새 스케줄러 생성
    pub fn new(config: &SchedulingConfig) -> Self {
        let markets = vec![
            MarketHours::krx(),
            MarketHours::us(),
            MarketHours::jp(),
        ];

        Self {
            markets,
            holidays: HashSet::new(),
            config: config.clone(),
            last_daily_run: std::collections::HashMap::new(),
        }
    }

    /// 공휴일 추가 (형식: "KR:2024-01-01")
    pub fn add_holiday(&mut self, market: &str, date: NaiveDate) {
        let key = format!("{}:{}", market, date);
        self.holidays.insert(key);
    }

    /// 2025년 한국 공휴일 로드
    pub fn load_kr_holidays_2025(&mut self) {
        let holidays = [
            "2025-01-01", // 신정
            "2025-01-28", // 설날 연휴
            "2025-01-29", // 설날
            "2025-01-30", // 설날 연휴
            "2025-03-01", // 삼일절
            "2025-05-05", // 어린이날
            "2025-05-06", // 부처님오신날
            "2025-06-06", // 현충일
            "2025-08-15", // 광복절
            "2025-10-03", // 개천절
            "2025-10-05", // 추석 연휴
            "2025-10-06", // 추석
            "2025-10-07", // 추석 연휴
            "2025-10-09", // 한글날
            "2025-12-25", // 크리스마스
        ];

        for date_str in holidays {
            if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                self.add_holiday("KR", date);
            }
        }
    }

    /// 2026년 한국 공휴일 로드
    pub fn load_kr_holidays_2026(&mut self) {
        let holidays = [
            "2026-01-01", // 신정
            "2026-02-16", // 설날 연휴
            "2026-02-17", // 설날
            "2026-02-18", // 설날 연휴
            "2026-03-01", // 삼일절 (일요일)
            "2026-03-02", // 대체공휴일
            "2026-05-05", // 어린이날
            "2026-05-24", // 부처님오신날
            "2026-06-06", // 현충일
            "2026-08-15", // 광복절
            "2026-09-24", // 추석 연휴
            "2026-09-25", // 추석
            "2026-09-26", // 추석 연휴
            "2026-10-03", // 개천절
            "2026-10-09", // 한글날
            "2026-12-25", // 크리스마스
        ];

        for date_str in holidays {
            if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                self.add_holiday("KR", date);
            }
        }
    }

    /// 특정 시장의 운영 시간 조회
    pub fn get_market_hours(&self, market: &str) -> Option<&MarketHours> {
        self.markets.iter().find(|m| m.market == market)
    }

    /// 주말 여부 확인
    pub fn is_weekend(date: NaiveDate) -> bool {
        matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
    }

    /// 공휴일 여부 확인
    pub fn is_holiday(&self, market: &str, date: NaiveDate) -> bool {
        let key = format!("{}:{}", market, date);
        self.holidays.contains(&key)
    }

    /// 시장 상태 조회
    pub fn get_market_status(&self, market: &str, now: DateTime<Utc>) -> MarketStatus {
        let market_hours = match self.get_market_hours(market) {
            Some(h) => h,
            None => return MarketStatus::Closed, // 알 수 없는 시장은 항상 마감 상태
        };

        // UTC를 현지 시간으로 변환
        let local_time = now.with_timezone(&market_hours.timezone);
        let local_date = local_time.date_naive();
        let local_naive_time = local_time.time();

        // 주말 체크
        if self.config.skip_weekends && Self::is_weekend(local_date) {
            return MarketStatus::Holiday;
        }

        // 공휴일 체크
        if self.config.skip_holidays && self.is_holiday(market, local_date) {
            return MarketStatus::Holiday;
        }

        // 장 운영 시간 체크
        if local_naive_time >= market_hours.open_time && local_naive_time < market_hours.close_time {
            MarketStatus::Open
        } else {
            MarketStatus::Closed
        }
    }

    /// 일일 워크플로우 실행 여부 판단
    ///
    /// 조건:
    /// 1. 장이 마감된 상태
    /// 2. 마감 후 설정된 시간이 경과
    /// 3. 오늘 아직 실행하지 않음
    pub fn should_run_daily_workflow(&mut self, market: &str, now: DateTime<Utc>) -> bool {
        let market_hours = match self.get_market_hours(market) {
            Some(h) => h,
            None => return false,
        };

        // 현지 시간으로 변환
        let local_time = now.with_timezone(&market_hours.timezone);
        let local_date = local_time.date_naive();
        let local_naive_time = local_time.time();

        // 주말/공휴일이면 실행 안함
        if self.config.skip_weekends && Self::is_weekend(local_date) {
            return false;
        }
        if self.config.skip_holidays && self.is_holiday(market, local_date) {
            return false;
        }

        // 장 마감 후 대기 시간 계산
        let delay_minutes = if market == "KR" {
            self.config.krx_delay_after_close_minutes
        } else {
            60 // 기타 시장 기본값
        };

        let earliest_run_time = market_hours.close_time + chrono::Duration::minutes(delay_minutes as i64);

        // 마감 후 대기 시간이 지났는지 확인
        if local_naive_time < earliest_run_time {
            return false;
        }

        // 오늘 이미 실행했는지 확인
        if let Some(last_run) = self.last_daily_run.get(market) {
            if *last_run == local_date {
                debug!(market = %market, "오늘 이미 일일 워크플로우 실행함");
                return false;
            }
        }

        // 실행 가능 - 실행 날짜 기록
        self.last_daily_run.insert(market.to_string(), local_date);
        info!(
            market = %market,
            local_time = %local_time.format("%Y-%m-%d %H:%M:%S"),
            "일일 워크플로우 실행 조건 충족"
        );

        true
    }

    /// 다음 실행 시간까지 대기해야 하는 시간 (초)
    pub fn seconds_until_next_run(&self, market: &str, now: DateTime<Utc>) -> Option<i64> {
        let market_hours = self.get_market_hours(market)?;

        let local_time = now.with_timezone(&market_hours.timezone);
        let local_naive_time = local_time.time();

        let delay_minutes = if market == "KR" {
            self.config.krx_delay_after_close_minutes
        } else {
            60
        };

        let target_time = market_hours.close_time + chrono::Duration::minutes(delay_minutes as i64);

        if local_naive_time < target_time {
            // 오늘 실행 예정
            let diff = target_time.signed_duration_since(local_naive_time);
            Some(diff.num_seconds())
        } else {
            // 내일 실행
            // 대략적 계산 (24시간 - 현재 시간 + 마감 후 시간)
            let seconds_to_midnight = NaiveTime::from_hms_opt(23, 59, 59)
                .unwrap()
                .signed_duration_since(local_naive_time)
                .num_seconds();
            let seconds_from_midnight = target_time.num_seconds_from_midnight() as i64;
            Some(seconds_to_midnight + seconds_from_midnight)
        }
    }

    /// 스케줄러 상태 요약
    pub fn status_summary(&self, now: DateTime<Utc>) -> String {
        let mut lines = vec!["=== 스케줄러 상태 ===".to_string()];

        for market_hours in &self.markets {
            let status = self.get_market_status(&market_hours.market, now);
            let local_time = now.with_timezone(&market_hours.timezone);

            lines.push(format!(
                "{}: {:?} (현지시간: {})",
                market_hours.market,
                status,
                local_time.format("%H:%M:%S")
            ));
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weekend_check() {
        let saturday = NaiveDate::from_ymd_opt(2025, 1, 4).unwrap(); // 토요일
        let monday = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(); // 월요일

        assert!(Scheduler::is_weekend(saturday));
        assert!(!Scheduler::is_weekend(monday));
    }

    #[test]
    fn test_holiday_check() {
        let config = SchedulingConfig {
            enabled: true,
            krx_delay_after_close_minutes: 60,
            skip_weekends: true,
            skip_holidays: true,
        };
        let mut scheduler = Scheduler::new(&config);
        scheduler.load_kr_holidays_2025();

        let new_year = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let regular_day = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();

        assert!(scheduler.is_holiday("KR", new_year));
        assert!(!scheduler.is_holiday("KR", regular_day));
    }

    #[test]
    fn test_market_hours() {
        let krx = MarketHours::krx();
        assert_eq!(krx.market, "KR");
        assert_eq!(krx.open_time.hour(), 9);
        assert_eq!(krx.close_time.hour(), 15);
        assert_eq!(krx.close_time.minute(), 30);
    }
}
