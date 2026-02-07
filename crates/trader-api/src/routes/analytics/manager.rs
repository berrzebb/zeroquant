//! 분석 데이터 매니저.
//!
//! 포트폴리오 자산 곡선을 관리하고 분석 데이터를 제공합니다.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use trader_analytics::portfolio::{
    ChartPoint, EquityCurve, EquityCurveBuilder, PerformanceSummary, PeriodPerformance,
    PortfolioCharts,
};

/// 분석 데이터 매니저.
///
/// 포트폴리오 자산 곡선을 관리하고 분석 데이터를 제공합니다.
pub struct AnalyticsManager {
    /// 자산 곡선 빌더
    builder: EquityCurveBuilder,

    /// 빌드된 자산 곡선 캐시
    pub(crate) curve_cache: Option<EquityCurve>,

    /// 캐시 유효 시간
    cache_valid_until: Option<DateTime<Utc>>,
}

impl AnalyticsManager {
    /// 새 매니저 생성.
    pub fn new(initial_capital: Decimal) -> Self {
        Self {
            builder: EquityCurveBuilder::new(initial_capital),
            curve_cache: None,
            cache_valid_until: None,
        }
    }

    /// 거래 결과 추가.
    pub fn add_trade_result(&mut self, timestamp: DateTime<Utc>, equity: Decimal) {
        self.builder.add_trade_result(timestamp, equity);
        self.invalidate_cache();
    }

    /// 캐시 무효화.
    fn invalidate_cache(&mut self) {
        self.curve_cache = None;
        self.cache_valid_until = None;
    }

    /// 자산 곡선 가져오기 (캐시 사용).
    pub fn get_curve(&mut self) -> &EquityCurve {
        let now = Utc::now();

        // 캐시가 유효하면 반환
        #[allow(clippy::unnecessary_unwrap)]
        if let Some(valid_until) = self.cache_valid_until {
            if now < valid_until && self.curve_cache.is_some() {
                return self.curve_cache.as_ref().unwrap();
            }
        }

        // 캐시 재생성 (builder를 clone하여 소유권 문제 해결)
        self.curve_cache = Some(self.builder.clone().build());
        self.cache_valid_until = Some(now + Duration::minutes(5));

        self.curve_cache.as_ref().unwrap()
    }

    /// 성과 요약 가져오기.
    pub fn get_performance_summary(&mut self) -> PerformanceSummary {
        let curve = self.get_curve();
        PerformanceSummary::from_equity_curve(curve)
    }

    /// 기간별 성과 가져오기.
    pub fn get_period_performance(&mut self) -> Vec<PeriodPerformance> {
        let curve = self.get_curve();
        PeriodPerformance::calculate_periods(curve)
    }

    /// 차트 데이터 가져오기.
    pub fn get_charts(&mut self, window_days: i64) -> PortfolioCharts {
        let curve = self.get_curve();
        PortfolioCharts::from_equity_curve_with_params(curve, window_days, 0.05)
    }

    /// 자산 곡선 데이터 가져오기.
    pub fn get_equity_curve_data(&mut self) -> Vec<ChartPoint> {
        let curve = self.get_curve();
        curve
            .equity_series()
            .into_iter()
            .map(|(ts, equity)| ChartPoint::new(ts, equity))
            .collect()
    }

    /// 샘플 데이터 로드 (테스트용).
    pub fn load_sample_data(&mut self) {
        let base_time = Utc::now() - Duration::days(365);
        let mut equity = dec!(10_000_000);

        for i in 0..365 {
            // 변동성 있는 상승 곡선 시뮬레이션
            let daily_return = if i % 7 == 0 {
                dec!(-0.02) // 주간 조정
            } else if i % 3 == 0 {
                dec!(0.015) // 소폭 상승
            } else {
                dec!(0.003) // 일반 상승
            };

            equity *= dec!(1.0) + daily_return;
            self.add_trade_result(base_time + Duration::days(i), equity);
        }
    }
}

impl Default for AnalyticsManager {
    fn default() -> Self {
        Self::new(dec!(10_000_000))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analytics_manager_creation() {
        let manager = AnalyticsManager::new(dec!(10_000_000));
        assert!(manager.curve_cache.is_none());
    }

    #[test]
    fn test_analytics_manager_add_trade() {
        let mut manager = AnalyticsManager::new(dec!(10_000_000));
        manager.add_trade_result(Utc::now(), dec!(10_100_000));

        let curve = manager.get_curve();
        assert!(!curve.is_empty());
    }

    #[test]
    fn test_analytics_manager_sample_data() {
        let mut manager = AnalyticsManager::default();
        manager.load_sample_data();

        let summary = manager.get_performance_summary();
        assert!(summary.current_equity > Decimal::ZERO);
        assert!(summary.period_days > 0);
    }

    #[test]
    fn test_analytics_manager_charts() {
        let mut manager = AnalyticsManager::default();
        manager.load_sample_data();

        let charts = manager.get_charts(365);
        assert!(!charts.equity_curve.is_empty());
        assert!(!charts.drawdown_curve.is_empty());
    }
}
