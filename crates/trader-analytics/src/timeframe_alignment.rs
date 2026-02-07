//! 다중 타임프레임 정렬 유틸리티.
//!
//! 백테스트에서 미래 데이터 누출(Look-Ahead Bias)을 방지하기 위한
//! 타임프레임 정렬 로직을 제공합니다.
//!
//! # 핵심 개념
//!
//! 다중 타임프레임 전략에서 Secondary 타임프레임(예: 1시간봉, 일봉)의 데이터는
//! Primary 타임프레임(예: 5분봉)의 현재 시점에서 "완료된" 캔들만 사용해야 합니다.
//!
//! ## 예시
//!
//! - Primary: 5분봉, 현재 시점 10:07
//! - Secondary: 1시간봉
//! - **유효한 1시간봉**: 09:00~10:00 (10:00에 완료된 캔들)
//! - **무효한 1시간봉**: 10:00~11:00 (아직 진행 중)
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! use trader_analytics::timeframe_alignment::TimeframeAligner;
//! use chrono::Utc;
//!
//! let h1_klines = vec![/* ... */];
//! let current_time = Utc::now();
//!
//! // 현재 시점 기준으로 완료된 캔들만 필터링
//! let valid_klines = TimeframeAligner::get_aligned_klines(&h1_klines, current_time);
//!
//! // 가장 최근 완료된 캔들 조회
//! let latest = TimeframeAligner::find_latest_completed(&h1_klines, current_time);
//! ```

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use trader_core::{Kline, Timeframe};

/// 타임프레임 정렬 유틸리티.
///
/// 백테스트 및 실시간 분석에서 미래 데이터 누출을 방지합니다.
pub struct TimeframeAligner;

impl TimeframeAligner {
    /// Secondary 캔들이 특정 시점에서 유효한지 확인.
    ///
    /// 캔들의 종료 시간(`close_time`)이 기준 시점 이전이면 유효합니다.
    ///
    /// # 인자
    ///
    /// * `kline` - 확인할 캔들
    /// * `reference_time` - 기준 시점 (보통 Primary 캔들의 종료 시간)
    ///
    /// # 반환
    ///
    /// 캔들이 유효하면 `true`, 아직 완료되지 않았으면 `false`
    #[inline]
    pub fn is_valid_at(kline: &Kline, reference_time: DateTime<Utc>) -> bool {
        kline.close_time <= reference_time
    }

    /// 기준 시점까지 완료된 캔들만 필터링.
    ///
    /// # 인자
    ///
    /// * `klines` - Secondary 타임프레임 캔들 목록
    /// * `reference_time` - 기준 시점
    ///
    /// # 반환
    ///
    /// 완료된 캔들에 대한 참조 벡터 (시간순 정렬 유지)
    pub fn get_aligned_klines(
        klines: &[Kline],
        reference_time: DateTime<Utc>,
    ) -> Vec<&Kline> {
        klines
            .iter()
            .filter(|k| Self::is_valid_at(k, reference_time))
            .collect()
    }

    /// 기준 시점에서 가장 최근 완료된 캔들 찾기.
    ///
    /// # 인자
    ///
    /// * `klines` - Secondary 타임프레임 캔들 목록 (시간순 정렬 가정)
    /// * `reference_time` - 기준 시점
    ///
    /// # 반환
    ///
    /// 가장 최근 완료된 캔들에 대한 참조, 없으면 `None`
    pub fn find_latest_completed(
        klines: &[Kline],
        reference_time: DateTime<Utc>,
    ) -> Option<&Kline> {
        // 역순으로 탐색하여 첫 번째 유효한 캔들 반환
        klines
            .iter()
            .rev()
            .find(|k| Self::is_valid_at(k, reference_time))
    }

    /// 여러 타임프레임의 데이터를 기준 시점에 맞게 정렬.
    ///
    /// # 인자
    ///
    /// * `secondary_data` - 타임프레임별 캔들 데이터
    /// * `reference_time` - 기준 시점
    ///
    /// # 반환
    ///
    /// 타임프레임별 정렬된 캔들 데이터 (복사본)
    pub fn align_multi_timeframe(
        secondary_data: &HashMap<Timeframe, Vec<Kline>>,
        reference_time: DateTime<Utc>,
    ) -> HashMap<Timeframe, Vec<Kline>> {
        secondary_data
            .iter()
            .map(|(&tf, klines)| {
                let aligned: Vec<Kline> = klines
                    .iter()
                    .filter(|k| Self::is_valid_at(k, reference_time))
                    .cloned()
                    .collect();
                (tf, aligned)
            })
            .collect()
    }

    /// N개의 가장 최근 완료된 캔들 반환.
    ///
    /// # 인자
    ///
    /// * `klines` - 캔들 목록
    /// * `reference_time` - 기준 시점
    /// * `count` - 반환할 캔들 수
    ///
    /// # 반환
    ///
    /// 가장 최근 완료된 N개 캔들 (시간순)
    pub fn get_latest_n_completed(
        klines: &[Kline],
        reference_time: DateTime<Utc>,
        count: usize,
    ) -> Vec<Kline> {
        let aligned: Vec<&Kline> = Self::get_aligned_klines(klines, reference_time);
        let start = aligned.len().saturating_sub(count);
        aligned[start..].iter().map(|&k| k.clone()).collect()
    }

    /// 특정 타임프레임의 캔들이 "완료"되는 시점 계산.
    ///
    /// 주어진 시간이 포함된 캔들의 종료 시간을 반환합니다.
    ///
    /// # 인자
    ///
    /// * `time` - 확인할 시점
    /// * `timeframe` - 타임프레임
    ///
    /// # 반환
    ///
    /// 해당 캔들이 완료되는 시점
    pub fn get_candle_close_time(time: DateTime<Utc>, timeframe: Timeframe) -> DateTime<Utc> {
        

        let duration = timeframe.duration();
        let duration_secs = duration.as_secs() as i64;

        if duration_secs == 0 {
            return time;
        }

        // 현재 시간을 타임프레임 단위로 내림
        let timestamp = time.timestamp();
        let aligned_timestamp = (timestamp / duration_secs) * duration_secs;

        // 캔들 종료 시간 = 정렬된 시작 시간 + duration
        DateTime::from_timestamp(aligned_timestamp + duration_secs, 0).unwrap_or(time)
    }

    /// 두 시점 사이의 캔들 수 계산.
    ///
    /// # 인자
    ///
    /// * `start` - 시작 시점
    /// * `end` - 종료 시점
    /// * `timeframe` - 타임프레임
    ///
    /// # 반환
    ///
    /// 캔들 수 (정수)
    pub fn count_candles_between(
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        timeframe: Timeframe,
    ) -> usize {
        let duration = timeframe.duration();
        let duration_secs = duration.as_secs() as i64;

        if duration_secs == 0 {
            return 0;
        }

        let diff = end.signed_duration_since(start).num_seconds();
        (diff / duration_secs).max(0) as usize
    }
}

// =============================================================================
// 테스트
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal_macros::dec;

    fn make_kline(open_time: DateTime<Utc>, close_time: DateTime<Utc>) -> Kline {
        Kline {
            ticker: "TEST".to_string(),
            timeframe: Timeframe::H1,
            open_time,
            open: dec!(100),
            high: dec!(110),
            low: dec!(90),
            close: dec!(105),
            volume: dec!(1000),
            close_time,
            quote_volume: None,
            num_trades: None,
        }
    }

    #[test]
    fn test_is_valid_at() {
        let kline = make_kline(
            Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap(),
        );

        // 캔들 종료 시간 이후 → 유효
        let after = Utc.with_ymd_and_hms(2024, 1, 1, 10, 5, 0).unwrap();
        assert!(TimeframeAligner::is_valid_at(&kline, after));

        // 캔들 종료 시간과 동일 → 유효
        let equal = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
        assert!(TimeframeAligner::is_valid_at(&kline, equal));

        // 캔들 종료 시간 이전 → 무효
        let before = Utc.with_ymd_and_hms(2024, 1, 1, 9, 30, 0).unwrap();
        assert!(!TimeframeAligner::is_valid_at(&kline, before));
    }

    #[test]
    fn test_get_aligned_klines() {
        let klines = vec![
            make_kline(
                Utc.with_ymd_and_hms(2024, 1, 1, 8, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap(),
            ),
            make_kline(
                Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap(),
            ),
            make_kline(
                Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2024, 1, 1, 11, 0, 0).unwrap(),
            ),
        ];

        // 10:30 기준: 9시, 10시 캔들만 유효
        let reference = Utc.with_ymd_and_hms(2024, 1, 1, 10, 30, 0).unwrap();
        let aligned = TimeframeAligner::get_aligned_klines(&klines, reference);

        assert_eq!(aligned.len(), 2);
        assert_eq!(
            aligned[0].close_time,
            Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap()
        );
        assert_eq!(
            aligned[1].close_time,
            Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap()
        );
    }

    #[test]
    fn test_find_latest_completed() {
        let klines = vec![
            make_kline(
                Utc.with_ymd_and_hms(2024, 1, 1, 8, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap(),
            ),
            make_kline(
                Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap(),
            ),
            make_kline(
                Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2024, 1, 1, 11, 0, 0).unwrap(),
            ),
        ];

        let reference = Utc.with_ymd_and_hms(2024, 1, 1, 10, 30, 0).unwrap();
        let latest = TimeframeAligner::find_latest_completed(&klines, reference);

        assert!(latest.is_some());
        assert_eq!(
            latest.unwrap().close_time,
            Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap()
        );
    }

    #[test]
    fn test_count_candles_between() {
        let start = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

        // 3시간 → 1시간봉 3개
        assert_eq!(
            TimeframeAligner::count_candles_between(start, end, Timeframe::H1),
            3
        );

        // 3시간 → 15분봉 12개
        assert_eq!(
            TimeframeAligner::count_candles_between(start, end, Timeframe::M15),
            12
        );
    }
}
