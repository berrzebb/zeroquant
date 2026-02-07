//! 포트폴리오 차트 데이터 구조
//!
//! 웹 대시보드 및 텔레그램에서 사용할 차트 데이터를 생성합니다.
//!
//! # 제공 차트
//!
//! - 자산 곡선 (Equity Curve)
//! - CAGR 추이 차트
//! - MDD 추이 차트
//! - 월별 수익률 히트맵
//! - 롤링 샤프 비율

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use super::equity_curve::{EquityCurve, TimeFrame};

/// 차트 데이터 포인트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartPoint {
    /// X축 값 (타임스탬프, 밀리초)
    pub x: i64,

    /// Y축 값
    pub y: Decimal,

    /// 레이블 (선택적)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl ChartPoint {
    /// 새로운 차트 포인트를 생성합니다.
    pub fn new(timestamp: DateTime<Utc>, value: Decimal) -> Self {
        Self {
            x: timestamp.timestamp_millis(),
            y: value,
            label: None,
        }
    }

    /// 레이블이 있는 차트 포인트를 생성합니다.
    pub fn with_label(timestamp: DateTime<Utc>, value: Decimal, label: impl Into<String>) -> Self {
        Self {
            x: timestamp.timestamp_millis(),
            y: value,
            label: Some(label.into()),
        }
    }
}

/// 월별 수익률 셀 (히트맵용)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyReturnCell {
    /// 연도
    pub year: i32,

    /// 월 (1-12)
    pub month: u32,

    /// 수익률 (%)
    pub return_pct: Decimal,

    /// 색상 강도 (-1.0 ~ 1.0, 정규화됨)
    pub intensity: f64,
}

/// 포트폴리오 차트 데이터 모음
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioCharts {
    /// 자산 곡선 차트 데이터
    pub equity_curve: Vec<ChartPoint>,

    /// 수익률 곡선 차트 데이터
    pub returns_curve: Vec<ChartPoint>,

    /// Drawdown 차트 데이터
    pub drawdown_curve: Vec<ChartPoint>,

    /// 롤링 CAGR 차트 데이터 (1년 윈도우)
    pub rolling_cagr: Vec<ChartPoint>,

    /// 롤링 MDD 차트 데이터 (1년 윈도우)
    pub rolling_mdd: Vec<ChartPoint>,

    /// 롤링 샤프 비율 차트 데이터 (1년 윈도우)
    pub rolling_sharpe: Vec<ChartPoint>,

    /// 월별 수익률 히트맵 데이터
    pub monthly_returns: Vec<MonthlyReturnCell>,

    /// 연간 수익률 막대 차트 데이터
    pub yearly_returns: Vec<ChartPoint>,
}

impl PortfolioCharts {
    /// 자산 곡선에서 모든 차트 데이터를 생성합니다.
    ///
    /// # 매개변수
    ///
    /// * `curve` - 자산 곡선 데이터
    ///
    /// # 성능
    ///
    /// 1년 데이터(365 포인트) 기준 < 100ms 처리
    pub fn from_equity_curve(curve: &EquityCurve) -> Self {
        Self::from_equity_curve_with_params(curve, 365, 0.05)
    }

    /// 파라미터를 지정하여 차트 데이터를 생성합니다.
    ///
    /// # 매개변수
    ///
    /// * `curve` - 자산 곡선 데이터
    /// * `window_days` - 롤링 지표 계산 윈도우 (일)
    /// * `risk_free_rate` - 연간 무위험 이자율 (샤프 비율 계산용)
    pub fn from_equity_curve_with_params(
        curve: &EquityCurve,
        window_days: i64,
        risk_free_rate: f64,
    ) -> Self {
        Self {
            equity_curve: Self::build_equity_chart(curve),
            returns_curve: Self::build_returns_chart(curve),
            drawdown_curve: Self::build_drawdown_chart(curve),
            rolling_cagr: Self::build_rolling_cagr_chart(curve, window_days),
            rolling_mdd: Self::build_rolling_mdd_chart(curve, window_days),
            rolling_sharpe: Self::build_rolling_sharpe_chart(curve, window_days, risk_free_rate),
            monthly_returns: Self::build_monthly_heatmap(curve),
            yearly_returns: Self::build_yearly_returns_chart(curve),
        }
    }

    /// 자산 곡선 차트 데이터를 생성합니다.
    fn build_equity_chart(curve: &EquityCurve) -> Vec<ChartPoint> {
        curve
            .equity_series()
            .into_iter()
            .map(|(ts, equity)| ChartPoint::new(ts, equity))
            .collect()
    }

    /// 수익률 곡선 차트 데이터를 생성합니다.
    fn build_returns_chart(curve: &EquityCurve) -> Vec<ChartPoint> {
        curve
            .returns_series()
            .into_iter()
            .map(|(ts, ret)| ChartPoint::new(ts, ret))
            .collect()
    }

    /// Drawdown 차트 데이터를 생성합니다.
    fn build_drawdown_chart(curve: &EquityCurve) -> Vec<ChartPoint> {
        curve
            .drawdown_series()
            .into_iter()
            .map(|(ts, dd)| {
                // Drawdown을 음수로 표시 (차트에서 아래로 향하도록)
                ChartPoint::new(ts, -dd)
            })
            .collect()
    }

    /// 롤링 CAGR 차트 데이터를 생성합니다.
    fn build_rolling_cagr_chart(curve: &EquityCurve, window_days: i64) -> Vec<ChartPoint> {
        curve
            .rolling_cagr(window_days)
            .into_iter()
            .map(|(ts, cagr)| ChartPoint::new(ts, cagr))
            .collect()
    }

    /// 롤링 MDD 차트 데이터를 생성합니다.
    ///
    /// MDD는 음수로 표시됩니다 (차트에서 아래로 향하도록).
    fn build_rolling_mdd_chart(curve: &EquityCurve, window_days: i64) -> Vec<ChartPoint> {
        curve
            .rolling_mdd(window_days)
            .into_iter()
            .map(|(ts, mdd)| ChartPoint::new(ts, -mdd)) // 음수로 표시
            .collect()
    }

    /// 롤링 샤프 비율 차트 데이터를 생성합니다.
    fn build_rolling_sharpe_chart(
        curve: &EquityCurve,
        window_days: i64,
        risk_free_rate: f64,
    ) -> Vec<ChartPoint> {
        curve
            .rolling_sharpe(window_days, risk_free_rate)
            .into_iter()
            .map(|(ts, sharpe)| ChartPoint::new(ts, sharpe))
            .collect()
    }

    /// 월별 수익률 히트맵 데이터를 생성합니다.
    fn build_monthly_heatmap(curve: &EquityCurve) -> Vec<MonthlyReturnCell> {
        let monthly = curve.aggregate(TimeFrame::Monthly);
        let points = monthly.points();

        if points.is_empty() {
            return Vec::new();
        }

        // 월별 수익률 계산
        let mut cells: Vec<MonthlyReturnCell> = Vec::new();
        let mut prev_equity = curve.initial_capital();

        for point in points {
            let return_pct = if prev_equity > Decimal::ZERO {
                (point.equity - prev_equity) / prev_equity * dec!(100)
            } else {
                Decimal::ZERO
            };

            let date = point.timestamp.date_naive();
            cells.push(MonthlyReturnCell {
                year: date.year(),
                month: date.month(),
                return_pct,
                intensity: 0.0, // 나중에 정규화
            });

            prev_equity = point.equity;
        }

        // 정규화 (최대/최소 기준)
        if !cells.is_empty() {
            let max_abs = cells
                .iter()
                .map(|c| c.return_pct.abs())
                .max()
                .unwrap_or(Decimal::ONE);

            if max_abs > Decimal::ZERO {
                for cell in &mut cells {
                    cell.intensity = (cell.return_pct / max_abs)
                        .to_f64()
                        .unwrap_or(0.0)
                        .clamp(-1.0, 1.0);
                }
            }
        }

        cells
    }

    /// 연간 수익률 막대 차트 데이터를 생성합니다.
    fn build_yearly_returns_chart(curve: &EquityCurve) -> Vec<ChartPoint> {
        let yearly = curve.aggregate(TimeFrame::Yearly);
        let points = yearly.points();

        if points.is_empty() {
            return Vec::new();
        }

        let mut prev_equity = curve.initial_capital();
        let mut results = Vec::new();

        for point in points {
            let return_pct = if prev_equity > Decimal::ZERO {
                (point.equity - prev_equity) / prev_equity * dec!(100)
            } else {
                Decimal::ZERO
            };

            results.push(ChartPoint::with_label(
                point.timestamp,
                return_pct,
                point.timestamp.format("%Y").to_string(),
            ));

            prev_equity = point.equity;
        }

        results
    }
}

/// 성과 요약 데이터 (대시보드/텔레그램용)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    /// 현재 자산 가치
    pub current_equity: Decimal,

    /// 초기 자본
    pub initial_capital: Decimal,

    /// 총 수익/손실 금액
    pub total_pnl: Decimal,

    /// 총 수익률 (%)
    pub total_return_pct: Decimal,

    /// CAGR (%)
    pub cagr_pct: Decimal,

    /// 최대 낙폭 (%)
    pub max_drawdown_pct: Decimal,

    /// 현재 낙폭 (%)
    pub current_drawdown_pct: Decimal,

    /// 고점 자산 가치
    pub peak_equity: Decimal,

    /// 데이터 기간 (일)
    pub period_days: i64,

    /// 마지막 업데이트 시각
    pub last_updated: DateTime<Utc>,
}

impl PerformanceSummary {
    /// 자산 곡선에서 성과 요약을 생성합니다.
    pub fn from_equity_curve(curve: &EquityCurve) -> Self {
        let current = curve.current_equity();
        let initial = curve.initial_capital();

        let period_days = if curve.is_empty() {
            0
        } else {
            let first = curve.points().first().unwrap().timestamp;
            let last = curve.points().last().unwrap().timestamp;
            (last - first).num_days()
        };

        Self {
            current_equity: current,
            initial_capital: initial,
            total_pnl: current - initial,
            total_return_pct: curve.total_return(),
            cagr_pct: curve.cagr(),
            max_drawdown_pct: curve.max_drawdown(),
            current_drawdown_pct: curve.current_drawdown(),
            peak_equity: curve.peak_equity(),
            period_days,
            last_updated: Utc::now(),
        }
    }

    /// 텔레그램용 포맷된 메시지를 생성합니다.
    pub fn to_telegram_message(&self) -> String {
        let pnl_emoji = if self.total_pnl >= Decimal::ZERO {
            "📈"
        } else {
            "📉"
        };

        let dd_emoji = if self.current_drawdown_pct > dec!(10) {
            "⚠️"
        } else {
            "✅"
        };

        format!(
            "💼 *포트폴리오 현황*\n\
            \n\
            *자산 가치*\n\
            현재: ₩{:.0}\n\
            고점: ₩{:.0}\n\
            \n\
            {} *수익/손실*\n\
            금액: ₩{:.0}\n\
            수익률: {:.2}%\n\
            CAGR: {:.2}%\n\
            \n\
            {} *리스크*\n\
            최대 낙폭: {:.2}%\n\
            현재 낙폭: {:.2}%\n\
            \n\
            📅 기간: {}일",
            self.current_equity,
            self.peak_equity,
            pnl_emoji,
            self.total_pnl,
            self.total_return_pct,
            self.cagr_pct,
            dd_emoji,
            self.max_drawdown_pct,
            self.current_drawdown_pct,
            self.period_days
        )
    }
}

/// 기간별 성과 비교
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodPerformance {
    /// 기간 이름 (예: "1W", "1M", "3M", "1Y", "YTD", "All")
    pub period: String,

    /// 수익률 (%)
    pub return_pct: Decimal,

    /// 기간 시작 자산
    pub start_equity: Decimal,

    /// 기간 종료 자산
    pub end_equity: Decimal,
}

impl PeriodPerformance {
    /// 여러 기간의 성과를 계산합니다.
    pub fn calculate_periods(curve: &EquityCurve) -> Vec<Self> {
        use chrono::Duration;

        if curve.is_empty() {
            return Vec::new();
        }

        let now = curve.points().last().unwrap().timestamp;
        let mut results = Vec::new();

        let periods = [
            ("1W", Duration::days(7)),
            ("1M", Duration::days(30)),
            ("3M", Duration::days(90)),
            ("6M", Duration::days(180)),
            ("1Y", Duration::days(365)),
        ];

        for (name, duration) in periods {
            let start_time = now - duration;
            let filtered = curve.filter_range(start_time, now);

            if !filtered.is_empty() {
                let start = filtered.initial_capital();
                let end = filtered.current_equity();
                let return_pct = if start > Decimal::ZERO {
                    (end - start) / start * dec!(100)
                } else {
                    Decimal::ZERO
                };

                results.push(Self {
                    period: name.to_string(),
                    return_pct,
                    start_equity: start,
                    end_equity: end,
                });
            }
        }

        // YTD (Year to Date)
        let ytd_start = DateTime::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(now.year(), 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            Utc,
        );
        let ytd_filtered = curve.filter_range(ytd_start, now);
        if !ytd_filtered.is_empty() {
            let start = ytd_filtered.initial_capital();
            let end = ytd_filtered.current_equity();
            let return_pct = if start > Decimal::ZERO {
                (end - start) / start * dec!(100)
            } else {
                Decimal::ZERO
            };

            results.push(Self {
                period: "YTD".to_string(),
                return_pct,
                start_equity: start,
                end_equity: end,
            });
        }

        // All Time
        results.push(Self {
            period: "All".to_string(),
            return_pct: curve.total_return(),
            start_equity: curve.initial_capital(),
            end_equity: curve.current_equity(),
        });

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portfolio::EquityCurveBuilder;
    use chrono::Duration;

    fn create_test_curve() -> EquityCurve {
        let mut builder = EquityCurveBuilder::new(dec!(10_000_000));
        let base_time = Utc::now() - Duration::days(365);

        // 1년간의 데이터 생성
        for i in 0..365 {
            // 변동성 있는 상승 곡선 시뮬레이션
            let growth = dec!(1.0) + Decimal::from(i) * dec!(0.0005);
            let noise = if i % 7 == 0 {
                dec!(-0.02)
            } else if i % 3 == 0 {
                dec!(0.01)
            } else {
                Decimal::ZERO
            };
            let equity = dec!(10_000_000) * growth * (dec!(1.0) + noise);

            builder.add_trade_result(base_time + Duration::days(i), equity);
        }

        builder.build()
    }

    #[test]
    fn test_portfolio_charts_generation() {
        let curve = create_test_curve();
        let charts = PortfolioCharts::from_equity_curve(&curve);

        assert!(!charts.equity_curve.is_empty());
        assert!(!charts.returns_curve.is_empty());
        assert!(!charts.drawdown_curve.is_empty());
        assert!(!charts.monthly_returns.is_empty());
        // 롤링 MDD와 롤링 샤프는 충분한 데이터가 있어야 생성됨
        assert!(!charts.rolling_mdd.is_empty() || curve.len() < 365);
    }

    #[test]
    fn test_rolling_mdd_chart() {
        let curve = create_test_curve();
        let charts = PortfolioCharts::from_equity_curve(&curve);

        // 롤링 MDD는 음수로 표시됨
        for point in &charts.rolling_mdd {
            assert!(point.y <= Decimal::ZERO);
        }
    }

    #[test]
    fn test_rolling_sharpe_chart() {
        let curve = create_test_curve();
        let charts = PortfolioCharts::from_equity_curve(&curve);

        // 샤프 비율이 계산되어야 함
        // (데이터가 충분하면)
        if !charts.rolling_sharpe.is_empty() {
            // 샤프 비율은 일반적으로 -5 ~ 5 범위
            for point in &charts.rolling_sharpe {
                assert!(point.y > dec!(-10) && point.y < dec!(10));
            }
        }
    }

    #[test]
    fn test_charts_with_custom_params() {
        let curve = create_test_curve();
        // 90일 윈도우, 3% 무위험 이자율로 테스트
        let charts = PortfolioCharts::from_equity_curve_with_params(&curve, 90, 0.03);

        assert!(!charts.equity_curve.is_empty());
        // 짧은 윈도우에서는 더 많은 데이터 포인트가 생성됨
        assert!(charts.rolling_cagr.len() >= charts.equity_curve.len() / 2 || curve.len() < 90);
    }

    #[test]
    fn test_chart_point() {
        let now = Utc::now();
        let point = ChartPoint::new(now, dec!(100));

        assert_eq!(point.x, now.timestamp_millis());
        assert_eq!(point.y, dec!(100));
        assert!(point.label.is_none());
    }

    #[test]
    fn test_chart_point_with_label() {
        let now = Utc::now();
        let point = ChartPoint::with_label(now, dec!(100), "Test");

        assert_eq!(point.label, Some("Test".to_string()));
    }

    #[test]
    fn test_monthly_heatmap() {
        let curve = create_test_curve();
        let charts = PortfolioCharts::from_equity_curve(&curve);

        // 12개월 데이터가 있어야 함
        assert!(charts.monthly_returns.len() >= 10);

        // 강도 값이 -1.0 ~ 1.0 범위
        for cell in &charts.monthly_returns {
            assert!(cell.intensity >= -1.0 && cell.intensity <= 1.0);
        }
    }

    #[test]
    fn test_performance_summary() {
        let curve = create_test_curve();
        let summary = PerformanceSummary::from_equity_curve(&curve);

        assert!(summary.current_equity > Decimal::ZERO);
        assert!(summary.period_days > 0);
    }

    #[test]
    fn test_telegram_message() {
        let curve = create_test_curve();
        let summary = PerformanceSummary::from_equity_curve(&curve);
        let message = summary.to_telegram_message();

        assert!(message.contains("포트폴리오 현황"));
        assert!(message.contains("CAGR"));
    }

    #[test]
    fn test_period_performance() {
        let curve = create_test_curve();
        let periods = PeriodPerformance::calculate_periods(&curve);

        // 여러 기간이 계산되어야 함
        assert!(periods.len() >= 4);

        // "All" 기간이 포함되어야 함
        assert!(periods.iter().any(|p| p.period == "All"));
    }

    #[test]
    fn test_rolling_cagr_chart() {
        let curve = create_test_curve();
        let charts = PortfolioCharts::from_equity_curve(&curve);

        // 365일 윈도우로 롤링 CAGR이 계산됨
        // 데이터가 1년이므로 마지막 포인트에서만 유효
        assert!(!charts.rolling_cagr.is_empty() || curve.len() < 365);
    }

    #[test]
    fn test_drawdown_chart_negative() {
        let curve = create_test_curve();
        let charts = PortfolioCharts::from_equity_curve(&curve);

        // Drawdown은 음수로 표시됨 (차트에서 아래로)
        for point in &charts.drawdown_curve {
            assert!(point.y <= Decimal::ZERO);
        }
    }
}
