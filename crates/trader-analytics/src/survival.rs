//! 생존일 추적기 (Survival Days Tracker).
//!
//! 종목이 연속으로 상위권에 머문 일수를 추적합니다.
//! "살아있는 종목"과 "죽은 종목"을 구분하여 추세 지속성을 평가합니다.
//!
//! # 개념
//!
//! - **생존일**: 연속으로 상위 랭킹에 진입한 일수
//! - **탈락 조건**: 하루라도 랭킹에서 빠지면 카운트 리셋
//! - **활용**: 생존일이 긴 종목 = 지속적인 강세 = 신뢰도 높은 매수 후보
//!
//! # 예시
//!
//! ```rust,ignore
//! use trader_analytics::survival::{SurvivalTracker, DailyRanking};
//!
//! let history = vec![
//!     DailyRanking { date: "2024-01-10", tickers: vec!["A", "B", "C"] },
//!     DailyRanking { date: "2024-01-11", tickers: vec!["A", "B", "D"] },
//!     DailyRanking { date: "2024-01-12", tickers: vec!["A", "B", "D", "E"] },
//! ];
//!
//! let tracker = SurvivalTracker::new(15);
//! let days = tracker.calculate("A", &history);
//! assert_eq!(days, 3); // A는 3일 연속 상위권
//!
//! let days = tracker.calculate("C", &history);
//! assert_eq!(days, 0); // C는 첫날 이후 탈락 → 리셋
//! ```

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 일별 랭킹 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyRanking {
    /// 날짜
    pub date: NaiveDate,
    /// 상위권에 진입한 종목 목록
    pub tickers: Vec<String>,
}

/// 종목별 생존일 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurvivalResult {
    /// 종목 티커
    pub ticker: String,
    /// 연속 생존일 (현재 기준)
    pub survival_days: u32,
    /// 최초 진입일
    pub first_entry_date: Option<NaiveDate>,
    /// 마지막 확인일
    pub last_seen_date: Option<NaiveDate>,
    /// 과거 15일 내 총 출현 횟수
    pub appearance_count: u32,
}

/// 생존일 추적기.
///
/// 종목이 연속으로 상위권에 머문 일수를 계산합니다.
#[derive(Debug, Clone)]
pub struct SurvivalTracker {
    /// 추적 기간 (기본: 15일)
    lookback_days: usize,
}

impl Default for SurvivalTracker {
    fn default() -> Self {
        Self::new(15)
    }
}

impl SurvivalTracker {
    /// 새로운 추적기 생성.
    ///
    /// # 인자
    ///
    /// * `lookback_days` - 추적할 과거 일수 (기본: 15일)
    pub fn new(lookback_days: usize) -> Self {
        Self { lookback_days }
    }

    /// 특정 종목의 연속 생존일 계산.
    ///
    /// 가장 최근 날짜부터 역순으로 탐색하여
    /// 연속으로 상위권에 있었던 일수를 계산합니다.
    ///
    /// # 인자
    ///
    /// * `ticker` - 조회할 종목 티커
    /// * `history` - 일별 랭킹 히스토리 (날짜 오름차순 정렬)
    ///
    /// # 반환
    ///
    /// 연속 생존일 (0 = 오늘 상위권에 없음)
    pub fn calculate(&self, ticker: &str, history: &[DailyRanking]) -> u32 {
        if history.is_empty() {
            return 0;
        }

        let mut consecutive_days = 0u32;
        let mut found_break = false;

        // 최신 데이터부터 역순으로 탐색
        for ranking in history.iter().rev().take(self.lookback_days) {
            if ranking.tickers.contains(&ticker.to_string()) {
                if !found_break {
                    consecutive_days += 1;
                }
            } else {
                found_break = true;
            }
        }

        consecutive_days
    }

    /// 특정 종목의 상세 생존 정보 계산.
    ///
    /// 연속 생존일뿐 아니라 최초 진입일, 총 출현 횟수 등을 포함합니다.
    pub fn calculate_detailed(&self, ticker: &str, history: &[DailyRanking]) -> SurvivalResult {
        if history.is_empty() {
            return SurvivalResult {
                ticker: ticker.to_string(),
                survival_days: 0,
                first_entry_date: None,
                last_seen_date: None,
                appearance_count: 0,
            };
        }

        let survival_days = self.calculate(ticker, history);

        // 출현 횟수 및 날짜 계산
        let mut appearance_count = 0u32;
        let mut first_entry_date: Option<NaiveDate> = None;
        let mut last_seen_date: Option<NaiveDate> = None;

        for ranking in history.iter().take(self.lookback_days) {
            if ranking.tickers.contains(&ticker.to_string()) {
                appearance_count += 1;
                if first_entry_date.is_none() {
                    first_entry_date = Some(ranking.date);
                }
                last_seen_date = Some(ranking.date);
            }
        }

        SurvivalResult {
            ticker: ticker.to_string(),
            survival_days,
            first_entry_date,
            last_seen_date,
            appearance_count,
        }
    }

    /// 여러 종목의 생존일을 일괄 계산.
    ///
    /// # 반환
    ///
    /// ticker → SurvivalResult 매핑
    pub fn calculate_batch(
        &self,
        tickers: &[String],
        history: &[DailyRanking],
    ) -> HashMap<String, SurvivalResult> {
        tickers
            .iter()
            .map(|t| (t.clone(), self.calculate_detailed(t, history)))
            .collect()
    }

    /// 히스토리에서 모든 종목의 생존일 계산.
    ///
    /// 마지막 날짜 기준 상위권에 있는 모든 종목의 생존일을 계산합니다.
    pub fn calculate_all(&self, history: &[DailyRanking]) -> HashMap<String, SurvivalResult> {
        if history.is_empty() {
            return HashMap::new();
        }

        // 마지막 날짜의 종목들 수집
        let latest = history.last().unwrap();
        self.calculate_batch(&latest.tickers, history)
    }

    /// 생존일 기준 랭킹 생성.
    ///
    /// 생존일이 긴 순서대로 정렬된 종목 목록을 반환합니다.
    pub fn rank_by_survival(&self, history: &[DailyRanking]) -> Vec<SurvivalResult> {
        let mut results: Vec<SurvivalResult> = self.calculate_all(history).into_values().collect();
        results.sort_by(|a, b| b.survival_days.cmp(&a.survival_days));
        results
    }
}

/// 일별 랭킹 데이터 빌더.
///
/// 스크리닝 결과를 DailyRanking으로 변환하는 헬퍼.
pub struct DailyRankingBuilder;

impl DailyRankingBuilder {
    /// 스크리닝 결과에서 DailyRanking 생성.
    ///
    /// # 인자
    ///
    /// * `date` - 랭킹 날짜
    /// * `screening_results` - 스크리닝 통과 종목 목록
    /// * `top_n` - 상위 N개만 포함 (None이면 전체)
    pub fn from_screening(
        date: NaiveDate,
        screening_results: &[trader_core::domain::ScreeningResult],
        top_n: Option<usize>,
    ) -> DailyRanking {
        let mut results: Vec<_> = screening_results.iter().filter(|r| r.passed).collect();

        // 점수 기준 내림차순 정렬
        results.sort_by(|a, b| b.overall_score.partial_cmp(&a.overall_score).unwrap());

        let tickers: Vec<String> = if let Some(n) = top_n {
            results
                .into_iter()
                .take(n)
                .map(|r| r.ticker.clone())
                .collect()
        } else {
            results.into_iter().map(|r| r.ticker.clone()).collect()
        };

        DailyRanking { date, tickers }
    }

    /// GlobalScore 결과에서 DailyRanking 생성.
    ///
    /// # 인자
    ///
    /// * `date` - 랭킹 날짜
    /// * `scores` - GlobalScore 결과 목록
    /// * `min_score` - 최소 점수 (이 이상만 포함)
    /// * `top_n` - 상위 N개만 포함 (None이면 전체)
    pub fn from_global_scores(
        date: NaiveDate,
        scores: &[trader_core::domain::GlobalScoreResult],
        min_score: rust_decimal::Decimal,
        top_n: Option<usize>,
    ) -> DailyRanking {
        let mut results: Vec<_> = scores
            .iter()
            .filter(|s| s.overall_score >= min_score)
            .collect();

        // 점수 기준 내림차순 정렬
        results.sort_by(|a, b| b.overall_score.partial_cmp(&a.overall_score).unwrap());

        let tickers: Vec<String> = if let Some(n) = top_n {
            results
                .into_iter()
                .take(n)
                .filter_map(|s| s.ticker.clone())
                .collect()
        } else {
            results
                .into_iter()
                .filter_map(|s| s.ticker.clone())
                .collect()
        };

        DailyRanking { date, tickers }
    }
}

/// 생존일을 ScreeningResult에 병합하는 헬퍼 함수.
///
/// 현재 ScreeningResult에는 survival_days 필드가 없으므로
/// 별도로 조회해서 사용해야 합니다.
/// 향후 ScreeningResult에 필드 추가 시 이 함수를 활용합니다.
pub fn get_survival_days_map(
    tracker: &SurvivalTracker,
    history: &[DailyRanking],
    tickers: &[String],
) -> HashMap<String, u32> {
    tickers
        .iter()
        .map(|t| (t.clone(), tracker.calculate(t, history)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn create_test_history() -> Vec<DailyRanking> {
        vec![
            DailyRanking {
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                tickers: vec!["A".to_string(), "B".to_string(), "C".to_string()],
            },
            DailyRanking {
                date: NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
                tickers: vec!["A".to_string(), "B".to_string(), "D".to_string()],
            },
            DailyRanking {
                date: NaiveDate::from_ymd_opt(2024, 1, 3).unwrap(),
                tickers: vec!["A".to_string(), "B".to_string(), "D".to_string()],
            },
            DailyRanking {
                date: NaiveDate::from_ymd_opt(2024, 1, 4).unwrap(),
                tickers: vec![
                    "A".to_string(),
                    "B".to_string(),
                    "D".to_string(),
                    "E".to_string(),
                ],
            },
            DailyRanking {
                date: NaiveDate::from_ymd_opt(2024, 1, 5).unwrap(),
                tickers: vec!["A".to_string(), "B".to_string(), "E".to_string()],
            },
        ]
    }

    #[test]
    fn test_continuous_survival() {
        let tracker = SurvivalTracker::new(15);
        let history = create_test_history();

        // A와 B는 5일 연속 상위권
        assert_eq!(tracker.calculate("A", &history), 5);
        assert_eq!(tracker.calculate("B", &history), 5);
    }

    #[test]
    fn test_survival_with_gap() {
        let tracker = SurvivalTracker::new(15);
        let history = create_test_history();

        // C는 첫날만 상위권, 이후 탈락 → 생존일 0
        assert_eq!(tracker.calculate("C", &history), 0);
    }

    #[test]
    fn test_late_entry() {
        let tracker = SurvivalTracker::new(15);
        let history = create_test_history();

        // D는 2~4일차에 상위권, 마지막 날 탈락 → 생존일 0
        assert_eq!(tracker.calculate("D", &history), 0);

        // E는 4~5일차에 상위권 → 생존일 2
        assert_eq!(tracker.calculate("E", &history), 2);
    }

    #[test]
    fn test_detailed_result() {
        let tracker = SurvivalTracker::new(15);
        let history = create_test_history();

        let result = tracker.calculate_detailed("A", &history);
        assert_eq!(result.survival_days, 5);
        assert_eq!(result.appearance_count, 5);
        assert_eq!(
            result.first_entry_date,
            Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
        );
    }

    #[test]
    fn test_empty_history() {
        let tracker = SurvivalTracker::new(15);
        let history: Vec<DailyRanking> = vec![];

        assert_eq!(tracker.calculate("A", &history), 0);
    }

    #[test]
    fn test_rank_by_survival() {
        let tracker = SurvivalTracker::new(15);
        let history = create_test_history();

        let ranked = tracker.rank_by_survival(&history);

        // A, B가 최상위 (5일), E가 다음 (2일)
        assert!(ranked.len() >= 2);
        assert_eq!(ranked[0].survival_days, 5);
        assert!(ranked[0].ticker == "A" || ranked[0].ticker == "B");
    }

    #[test]
    fn test_not_in_history() {
        let tracker = SurvivalTracker::new(15);
        let history = create_test_history();

        // 히스토리에 없는 종목
        assert_eq!(tracker.calculate("Z", &history), 0);
    }
}
