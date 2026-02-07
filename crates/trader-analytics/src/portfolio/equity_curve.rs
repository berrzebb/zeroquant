//! 자산 곡선(Equity Curve) 데이터 모듈
//!
//! 포트폴리오의 자산 가치 변화를 시계열로 추적하고 분석합니다.
//!
//! # 주요 기능
//!
//! - 시간별 자산 가치 추적
//! - 실시간 Drawdown 계산
//! - 일별/주별/월별 데이터 집계
//! - 수익률 시계열 생성
//!
//! # 성능 목표
//!
//! - 1년 데이터(약 365개 포인트) 처리 시간: < 100ms

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 시간 프레임 (데이터 집계 단위)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeFrame {
    /// 일별 집계
    Daily,
    /// 주별 집계 (월요일 시작)
    Weekly,
    /// 월별 집계
    Monthly,
    /// 분기별 집계
    Quarterly,
    /// 연간 집계
    Yearly,
}

impl TimeFrame {
    /// 시간 프레임의 표시 이름
    pub fn display_name(&self) -> &'static str {
        match self {
            TimeFrame::Daily => "일별",
            TimeFrame::Weekly => "주별",
            TimeFrame::Monthly => "월별",
            TimeFrame::Quarterly => "분기별",
            TimeFrame::Yearly => "연간",
        }
    }
}

/// 단일 자산 곡선 데이터 포인트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    /// 타임스탬프 (UTC)
    pub timestamp: DateTime<Utc>,

    /// 자산 가치
    pub equity: Decimal,

    /// 고점 대비 낙폭 (%)
    /// 0 이상의 값 (0 = 고점, 양수 = 하락 중)
    pub drawdown_pct: Decimal,

    /// 초기 자본 대비 수익률 (%)
    pub return_pct: Decimal,

    /// 전일/전 기간 대비 수익률 (%)
    pub period_return_pct: Decimal,
}

/// 자산 곡선 데이터
///
/// 시간에 따른 포트폴리오 가치 변화를 추적합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityCurve {
    /// 초기 자본
    initial_capital: Decimal,

    /// 시계열 데이터 포인트 (시간순 정렬)
    points: Vec<EquityPoint>,

    /// 현재 고점 자산 가치
    peak_equity: Decimal,

    /// 최대 낙폭 (%)
    max_drawdown_pct: Decimal,

    /// 최대 낙폭 발생 시점
    max_drawdown_timestamp: Option<DateTime<Utc>>,
}

impl EquityCurve {
    /// 새로운 자산 곡선을 생성합니다.
    ///
    /// # 매개변수
    ///
    /// * `initial_capital` - 초기 자본금
    pub fn new(initial_capital: Decimal) -> Self {
        Self {
            initial_capital,
            points: Vec::new(),
            peak_equity: initial_capital,
            max_drawdown_pct: Decimal::ZERO,
            max_drawdown_timestamp: None,
        }
    }

    /// 초기 자본을 반환합니다.
    pub fn initial_capital(&self) -> Decimal {
        self.initial_capital
    }

    /// 모든 데이터 포인트를 반환합니다.
    pub fn points(&self) -> &[EquityPoint] {
        &self.points
    }

    /// 현재 자산 가치를 반환합니다.
    pub fn current_equity(&self) -> Decimal {
        self.points
            .last()
            .map(|p| p.equity)
            .unwrap_or(self.initial_capital)
    }

    /// 현재 고점 자산 가치를 반환합니다.
    pub fn peak_equity(&self) -> Decimal {
        self.peak_equity
    }

    /// 현재 Drawdown을 반환합니다 (%).
    pub fn current_drawdown(&self) -> Decimal {
        self.points
            .last()
            .map(|p| p.drawdown_pct)
            .unwrap_or(Decimal::ZERO)
    }

    /// 최대 Drawdown을 반환합니다 (%).
    pub fn max_drawdown(&self) -> Decimal {
        self.max_drawdown_pct
    }

    /// 최대 Drawdown 발생 시점을 반환합니다.
    pub fn max_drawdown_timestamp(&self) -> Option<DateTime<Utc>> {
        self.max_drawdown_timestamp
    }

    /// 총 수익률을 반환합니다 (%).
    pub fn total_return(&self) -> Decimal {
        if self.initial_capital.is_zero() {
            return Decimal::ZERO;
        }

        let current = self.current_equity();
        (current - self.initial_capital) / self.initial_capital * dec!(100)
    }

    /// 데이터 포인트 수를 반환합니다.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// 데이터가 비어있는지 확인합니다.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// 새로운 자산 가치를 추가합니다.
    ///
    /// # 매개변수
    ///
    /// * `timestamp` - 타임스탬프
    /// * `equity` - 자산 가치
    pub fn add_point(&mut self, timestamp: DateTime<Utc>, equity: Decimal) {
        // 이전 자산 가치
        let prev_equity = self.current_equity();

        // 고점 갱신
        if equity > self.peak_equity {
            self.peak_equity = equity;
        }

        // Drawdown 계산
        let drawdown_pct = if self.peak_equity > Decimal::ZERO {
            (self.peak_equity - equity) / self.peak_equity * dec!(100)
        } else {
            Decimal::ZERO
        };

        // 최대 Drawdown 갱신
        if drawdown_pct > self.max_drawdown_pct {
            self.max_drawdown_pct = drawdown_pct;
            self.max_drawdown_timestamp = Some(timestamp);
        }

        // 총 수익률
        let return_pct = if self.initial_capital > Decimal::ZERO {
            (equity - self.initial_capital) / self.initial_capital * dec!(100)
        } else {
            Decimal::ZERO
        };

        // 기간 수익률
        let period_return_pct = if prev_equity > Decimal::ZERO {
            (equity - prev_equity) / prev_equity * dec!(100)
        } else {
            Decimal::ZERO
        };

        self.points.push(EquityPoint {
            timestamp,
            equity,
            drawdown_pct,
            return_pct,
            period_return_pct,
        });
    }

    /// 시간 프레임별로 데이터를 집계합니다.
    ///
    /// # 매개변수
    ///
    /// * `timeframe` - 집계 단위 (일/주/월/분기/연)
    ///
    /// # 반환값
    ///
    /// 집계된 자산 곡선 데이터
    pub fn aggregate(&self, timeframe: TimeFrame) -> EquityCurve {
        if self.points.is_empty() {
            return EquityCurve::new(self.initial_capital);
        }

        // 기간별로 그룹화
        let mut grouped: BTreeMap<String, Vec<&EquityPoint>> = BTreeMap::new();

        for point in &self.points {
            let key = Self::period_key(&point.timestamp, timeframe);
            grouped.entry(key).or_default().push(point);
        }

        // 각 기간의 마지막 값으로 새 곡선 생성
        let mut aggregated = EquityCurve::new(self.initial_capital);

        for (_, points) in grouped {
            if let Some(last_point) = points.last() {
                aggregated.add_point(last_point.timestamp, last_point.equity);
            }
        }

        aggregated
    }

    /// 타임스탬프를 기간 키로 변환합니다.
    fn period_key(timestamp: &DateTime<Utc>, timeframe: TimeFrame) -> String {
        let date = timestamp.date_naive();

        match timeframe {
            TimeFrame::Daily => date.format("%Y-%m-%d").to_string(),
            TimeFrame::Weekly => {
                // ISO 주차 사용
                let week = date.iso_week();
                format!("{}-W{:02}", week.year(), week.week())
            }
            TimeFrame::Monthly => date.format("%Y-%m").to_string(),
            TimeFrame::Quarterly => {
                let quarter = (date.month() - 1) / 3 + 1;
                format!("{}-Q{}", date.year(), quarter)
            }
            TimeFrame::Yearly => date.format("%Y").to_string(),
        }
    }

    /// 특정 기간의 데이터만 필터링합니다.
    ///
    /// # 매개변수
    ///
    /// * `start` - 시작 시각 (포함)
    /// * `end` - 종료 시각 (포함)
    pub fn filter_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> EquityCurve {
        let mut filtered = EquityCurve::new(self.initial_capital);

        // 시작 전 마지막 자산 가치 찾기 (초기값으로 사용)
        let initial_equity = self
            .points
            .iter().rfind(|p| p.timestamp < start)
            .map(|p| p.equity)
            .unwrap_or(self.initial_capital);

        filtered.peak_equity = initial_equity;

        for point in &self.points {
            if point.timestamp >= start && point.timestamp <= end {
                filtered.add_point(point.timestamp, point.equity);
            }
        }

        filtered
    }

    /// 일별 수익률 시계열을 반환합니다.
    ///
    /// # 반환값
    ///
    /// (날짜, 수익률%) 튜플의 벡터
    pub fn daily_returns(&self) -> Vec<(NaiveDate, Decimal)> {
        let daily = self.aggregate(TimeFrame::Daily);

        daily
            .points
            .iter()
            .map(|p| (p.timestamp.date_naive(), p.period_return_pct))
            .collect()
    }

    /// 월별 수익률 시계열을 반환합니다.
    ///
    /// # 반환값
    ///
    /// (연월 문자열, 수익률%) 튜플의 벡터
    pub fn monthly_returns(&self) -> Vec<(String, Decimal)> {
        let monthly = self.aggregate(TimeFrame::Monthly);

        monthly
            .points
            .iter()
            .map(|p| {
                let key = p.timestamp.format("%Y-%m").to_string();
                (key, p.period_return_pct)
            })
            .collect()
    }

    /// 연복리 수익률(CAGR)을 계산합니다.
    ///
    /// # 계산 공식
    ///
    /// CAGR = ((최종가치 / 초기가치) ^ (1/년수)) - 1
    ///
    /// # 반환값
    ///
    /// CAGR 백분율
    pub fn cagr(&self) -> Decimal {
        if self.points.is_empty() || self.initial_capital.is_zero() {
            return Decimal::ZERO;
        }

        let first = self.points.first().unwrap();
        let last = self.points.last().unwrap();

        let days = (last.timestamp - first.timestamp).num_days();
        if days <= 0 {
            return Decimal::ZERO;
        }

        let years = Decimal::from(days) / Decimal::from(365);
        if years.is_zero() {
            return Decimal::ZERO;
        }

        let current = self.current_equity();
        let ratio = current / self.initial_capital;

        // (ratio ^ (1/years) - 1) × 100
        // Decimal doesn't have pow for fractional exponents, use f64
        let ratio_f64 = ratio.to_f64().unwrap_or(1.0);
        let years_f64 = years.to_f64().unwrap_or(1.0);

        let cagr = ratio_f64.powf(1.0 / years_f64) - 1.0;

        Decimal::from_f64(cagr * 100.0).unwrap_or(Decimal::ZERO)
    }

    /// 롤링 CAGR을 계산합니다.
    ///
    /// # 매개변수
    ///
    /// * `window_days` - 계산 윈도우 (일)
    ///
    /// # 반환값
    ///
    /// (타임스탬프, CAGR%) 튜플의 벡터
    pub fn rolling_cagr(&self, window_days: i64) -> Vec<(DateTime<Utc>, Decimal)> {
        if self.points.len() < 2 {
            return Vec::new();
        }

        let mut results = Vec::new();
        let window = Duration::days(window_days);

        for (i, point) in self.points.iter().enumerate() {
            // window_days 전 시점 찾기
            let start_time = point.timestamp - window;

            // 시작 시점의 자산 가치 찾기
            let start_equity = self
                .points
                .iter()
                .take(i).rfind(|p| p.timestamp <= start_time)
                .map(|p| p.equity)
                .unwrap_or(self.initial_capital);

            if start_equity.is_zero() {
                continue;
            }

            let ratio = point.equity / start_equity;
            let years = Decimal::from(window_days) / Decimal::from(365);

            if years > Decimal::ZERO {
                let ratio_f64 = ratio.to_f64().unwrap_or(1.0);
                let years_f64 = years.to_f64().unwrap_or(1.0);
                let cagr = (ratio_f64.powf(1.0 / years_f64) - 1.0) * 100.0;

                results.push((
                    point.timestamp,
                    Decimal::from_f64(cagr).unwrap_or(Decimal::ZERO),
                ));
            }
        }

        results
    }

    /// 롤링 최대 낙폭(MDD)을 계산합니다.
    ///
    /// 각 시점에서 과거 window_days 기간 내의 최대 낙폭을 계산합니다.
    ///
    /// # 매개변수
    ///
    /// * `window_days` - 계산 윈도우 (일)
    ///
    /// # 반환값
    ///
    /// (타임스탬프, MDD%) 튜플의 벡터
    pub fn rolling_mdd(&self, window_days: i64) -> Vec<(DateTime<Utc>, Decimal)> {
        if self.points.len() < 2 {
            return Vec::new();
        }

        let mut results = Vec::new();
        let window = Duration::days(window_days);

        for (i, point) in self.points.iter().enumerate() {
            let start_time = point.timestamp - window;

            // 윈도우 내 데이터 추출
            let window_points: Vec<_> = self
                .points
                .iter()
                .take(i + 1)
                .filter(|p| p.timestamp >= start_time)
                .collect();

            if window_points.len() < 2 {
                results.push((point.timestamp, Decimal::ZERO));
                continue;
            }

            // 윈도우 내 최대 낙폭 계산
            let mut peak = window_points[0].equity;
            let mut max_dd = Decimal::ZERO;

            for wp in &window_points {
                if wp.equity > peak {
                    peak = wp.equity;
                }
                if peak > Decimal::ZERO {
                    let dd = (peak - wp.equity) / peak * dec!(100);
                    if dd > max_dd {
                        max_dd = dd;
                    }
                }
            }

            results.push((point.timestamp, max_dd));
        }

        results
    }

    /// 롤링 샤프 비율을 계산합니다.
    ///
    /// 각 시점에서 과거 window_days 기간의 일별 수익률을 기반으로
    /// 샤프 비율을 계산합니다.
    ///
    /// # 매개변수
    ///
    /// * `window_days` - 계산 윈도우 (일)
    /// * `risk_free_rate` - 연간 무위험 이자율 (예: 0.05 = 5%)
    ///
    /// # 반환값
    ///
    /// (타임스탬프, 샤프비율) 튜플의 벡터
    ///
    /// # 계산 공식
    ///
    /// Sharpe = (평균 일별 수익률 - 일별 무위험 이자율) / 일별 수익률 표준편차 × √252
    pub fn rolling_sharpe(
        &self,
        window_days: i64,
        risk_free_rate: f64,
    ) -> Vec<(DateTime<Utc>, Decimal)> {
        if self.points.len() < 2 {
            return Vec::new();
        }

        let mut results = Vec::new();
        let window = Duration::days(window_days);
        let daily_rf = risk_free_rate / 252.0; // 연간 → 일간

        for (i, point) in self.points.iter().enumerate() {
            let start_time = point.timestamp - window;

            // 윈도우 내 데이터 추출
            let window_points: Vec<_> = self
                .points
                .iter()
                .take(i + 1)
                .filter(|p| p.timestamp >= start_time)
                .collect();

            // 최소 2개 포인트 필요 (수익률 계산용)
            if window_points.len() < 3 {
                continue;
            }

            // 일별 수익률 계산
            let mut daily_returns: Vec<f64> = Vec::new();
            for j in 1..window_points.len() {
                let prev = window_points[j - 1].equity;
                let curr = window_points[j].equity;
                if prev > Decimal::ZERO {
                    let ret = ((curr - prev) / prev).to_f64().unwrap_or(0.0);
                    daily_returns.push(ret);
                }
            }

            if daily_returns.len() < 2 {
                continue;
            }

            // 평균 수익률
            let n = daily_returns.len() as f64;
            let mean_return: f64 = daily_returns.iter().sum::<f64>() / n;

            // 표준편차
            let variance: f64 = daily_returns
                .iter()
                .map(|r| (r - mean_return).powi(2))
                .sum::<f64>()
                / (n - 1.0);
            let std_dev = variance.sqrt();

            // 샤프 비율 계산
            // 최소 표준편차 임계값 (0.5% 일일 변동성 = 연 ~8%)
            // 너무 낮은 변동성에서는 샤프 비율이 의미 없음
            const MIN_STD_DEV: f64 = 0.005;

            if std_dev > MIN_STD_DEV {
                let excess_return = mean_return - daily_rf;
                let sharpe = (excess_return / std_dev) * 252_f64.sqrt();

                // 샤프 비율 상한/하한 (-9.99 ~ 9.99)
                // 실제 금융에서 ±3 범위가 일반적, 10 이상은 비현실적
                let bounded_sharpe = sharpe.clamp(-9.99, 9.99);

                results.push((
                    point.timestamp,
                    Decimal::from_f64(bounded_sharpe).unwrap_or(Decimal::ZERO),
                ));
            }
        }

        results
    }

    /// Drawdown 시계열을 반환합니다.
    ///
    /// # 반환값
    ///
    /// (타임스탬프, Drawdown%) 튜플의 벡터
    pub fn drawdown_series(&self) -> Vec<(DateTime<Utc>, Decimal)> {
        self.points
            .iter()
            .map(|p| (p.timestamp, p.drawdown_pct))
            .collect()
    }

    /// 수익 곡선 (초기 대비 수익률)을 반환합니다.
    ///
    /// # 반환값
    ///
    /// (타임스탬프, 수익률%) 튜플의 벡터
    pub fn returns_series(&self) -> Vec<(DateTime<Utc>, Decimal)> {
        self.points
            .iter()
            .map(|p| (p.timestamp, p.return_pct))
            .collect()
    }

    /// 자산 가치 시계열을 반환합니다.
    ///
    /// # 반환값
    ///
    /// (타임스탬프, 자산가치) 튜플의 벡터
    pub fn equity_series(&self) -> Vec<(DateTime<Utc>, Decimal)> {
        self.points
            .iter()
            .map(|p| (p.timestamp, p.equity))
            .collect()
    }
}

/// 자산 곡선 빌더
///
/// 거래 결과를 순차적으로 추가하여 자산 곡선을 구축합니다.
#[derive(Debug, Clone)]
pub struct EquityCurveBuilder {
    curve: EquityCurve,
}

impl EquityCurveBuilder {
    /// 새로운 빌더를 생성합니다.
    ///
    /// # 매개변수
    ///
    /// * `initial_capital` - 초기 자본금
    pub fn new(initial_capital: Decimal) -> Self {
        Self {
            curve: EquityCurve::new(initial_capital),
        }
    }

    /// 거래 결과를 추가합니다.
    ///
    /// # 매개변수
    ///
    /// * `timestamp` - 거래 시각
    /// * `new_equity` - 거래 후 자산 가치
    pub fn add_trade_result(&mut self, timestamp: DateTime<Utc>, new_equity: Decimal) -> &mut Self {
        self.curve.add_point(timestamp, new_equity);
        self
    }

    /// PnL로 자산 가치를 업데이트합니다.
    ///
    /// # 매개변수
    ///
    /// * `timestamp` - 거래 시각
    /// * `pnl` - 손익 (양수 = 수익, 음수 = 손실)
    pub fn add_pnl(&mut self, timestamp: DateTime<Utc>, pnl: Decimal) -> &mut Self {
        let new_equity = self.curve.current_equity() + pnl;
        self.curve.add_point(timestamp, new_equity);
        self
    }

    /// 자산 곡선을 빌드합니다.
    pub fn build(self) -> EquityCurve {
        self.curve
    }

    /// 현재 자산 곡선에 대한 참조를 반환합니다.
    pub fn current(&self) -> &EquityCurve {
        &self.curve
    }
}

/// Drawdown 기간 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawdownPeriod {
    /// 낙폭 시작 시점 (고점)
    pub start: DateTime<Utc>,

    /// 낙폭 종료 시점 (회복 또는 현재)
    pub end: Option<DateTime<Utc>>,

    /// 최저점 시점
    pub trough: DateTime<Utc>,

    /// 최대 낙폭 (%)
    pub max_drawdown_pct: Decimal,

    /// 고점 자산 가치
    pub peak_equity: Decimal,

    /// 최저점 자산 가치
    pub trough_equity: Decimal,

    /// 낙폭 기간 (일)
    pub duration_days: i64,

    /// 회복까지 기간 (일), 미회복시 None
    pub recovery_days: Option<i64>,
}

impl EquityCurve {
    /// 모든 Drawdown 기간을 분석합니다.
    ///
    /// # 반환값
    ///
    /// Drawdown 기간 목록 (크기순 정렬)
    pub fn analyze_drawdowns(&self) -> Vec<DrawdownPeriod> {
        if self.points.is_empty() {
            return Vec::new();
        }

        let mut periods = Vec::new();
        let mut peak = self.initial_capital;
        let mut peak_time = self.points.first().map(|p| p.timestamp).unwrap();
        let mut in_drawdown = false;
        let mut current_trough = peak;
        let mut current_trough_time = peak_time;

        for point in &self.points {
            if point.equity >= peak {
                // 새 고점 또는 회복
                if in_drawdown {
                    // Drawdown 종료
                    let dd_pct = (peak - current_trough) / peak * dec!(100);
                    let duration = (point.timestamp - peak_time).num_days();
                    let recovery = (point.timestamp - current_trough_time).num_days();

                    periods.push(DrawdownPeriod {
                        start: peak_time,
                        end: Some(point.timestamp),
                        trough: current_trough_time,
                        max_drawdown_pct: dd_pct,
                        peak_equity: peak,
                        trough_equity: current_trough,
                        duration_days: duration,
                        recovery_days: Some(recovery),
                    });

                    in_drawdown = false;
                }

                peak = point.equity;
                peak_time = point.timestamp;
                current_trough = peak;
                current_trough_time = peak_time;
            } else {
                // Drawdown 중
                if !in_drawdown {
                    in_drawdown = true;
                }

                if point.equity < current_trough {
                    current_trough = point.equity;
                    current_trough_time = point.timestamp;
                }
            }
        }

        // 현재 진행 중인 Drawdown
        if in_drawdown {
            let last_time = self.points.last().map(|p| p.timestamp).unwrap();
            let dd_pct = (peak - current_trough) / peak * dec!(100);
            let duration = (last_time - peak_time).num_days();

            periods.push(DrawdownPeriod {
                start: peak_time,
                end: None,
                trough: current_trough_time,
                max_drawdown_pct: dd_pct,
                peak_equity: peak,
                trough_equity: current_trough,
                duration_days: duration,
                recovery_days: None,
            });
        }

        // 최대 낙폭 기준 내림차순 정렬
        periods.sort_by(|a, b| b.max_drawdown_pct.cmp(&a.max_drawdown_pct));

        periods
    }

    /// 상위 N개 Drawdown 기간을 반환합니다.
    pub fn top_drawdowns(&self, n: usize) -> Vec<DrawdownPeriod> {
        self.analyze_drawdowns().into_iter().take(n).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_curve() -> EquityCurve {
        let mut builder = EquityCurveBuilder::new(dec!(10_000_000));
        let base_time = Utc::now() - Duration::days(30);

        // 상승 → 하락 → 상승 패턴
        builder.add_trade_result(base_time, dec!(10_000_000));
        builder.add_trade_result(base_time + Duration::days(5), dec!(10_500_000));
        builder.add_trade_result(base_time + Duration::days(10), dec!(11_000_000)); // 고점
        builder.add_trade_result(base_time + Duration::days(15), dec!(10_200_000)); // 하락
        builder.add_trade_result(base_time + Duration::days(20), dec!(9_800_000)); // 최저점
        builder.add_trade_result(base_time + Duration::days(25), dec!(10_500_000));
        builder.add_trade_result(base_time + Duration::days(30), dec!(11_500_000)); // 새 고점

        builder.build()
    }

    #[test]
    fn test_equity_curve_basic() {
        let curve = create_test_curve();

        assert_eq!(curve.initial_capital(), dec!(10_000_000));
        assert_eq!(curve.current_equity(), dec!(11_500_000));
        assert_eq!(curve.peak_equity(), dec!(11_500_000));
        assert_eq!(curve.len(), 7);
    }

    #[test]
    fn test_total_return() {
        let curve = create_test_curve();
        let return_pct = curve.total_return();

        // (11,500,000 - 10,000,000) / 10,000,000 * 100 = 15%
        assert_eq!(return_pct, dec!(15));
    }

    #[test]
    fn test_max_drawdown() {
        let curve = create_test_curve();

        // 고점 11,000,000에서 9,800,000까지 하락
        // (11,000,000 - 9,800,000) / 11,000,000 * 100 ≈ 10.9%
        let max_dd = curve.max_drawdown();
        assert!(max_dd > dec!(10) && max_dd < dec!(11));
    }

    #[test]
    fn test_drawdown_series() {
        let curve = create_test_curve();
        let series = curve.drawdown_series();

        assert_eq!(series.len(), 7);

        // 첫 번째 포인트는 Drawdown 0
        assert_eq!(series[0].1, Decimal::ZERO);

        // 새 고점에서는 Drawdown 0
        assert_eq!(series[6].1, Decimal::ZERO);
    }

    #[test]
    fn test_builder_add_pnl() {
        let mut builder = EquityCurveBuilder::new(dec!(10_000_000));
        let time = Utc::now();

        builder.add_pnl(time, dec!(100_000)); // +10만원
        builder.add_pnl(time + Duration::days(1), dec!(-50_000)); // -5만원

        let curve = builder.build();

        // 10,000,000 + 100,000 - 50,000 = 10,050,000
        assert_eq!(curve.current_equity(), dec!(10_050_000));
    }

    #[test]
    fn test_aggregate_monthly() {
        let mut builder = EquityCurveBuilder::new(dec!(10_000_000));
        let base_time = Utc::now() - Duration::days(60);

        // 2달간의 데이터
        for i in 0..60 {
            let equity = dec!(10_000_000) + Decimal::from(i * 10_000);
            builder.add_trade_result(base_time + Duration::days(i), equity);
        }

        let curve = builder.build();
        let monthly = curve.aggregate(TimeFrame::Monthly);

        // 약 2개월이므로 2-3개 포인트
        assert!(monthly.len() <= 3);
    }

    #[test]
    fn test_cagr() {
        let mut builder = EquityCurveBuilder::new(dec!(10_000_000));
        let start = Utc::now() - Duration::days(365);

        builder.add_trade_result(start, dec!(10_000_000));
        builder.add_trade_result(start + Duration::days(365), dec!(12_000_000)); // +20%

        let curve = builder.build();
        let cagr = curve.cagr();

        // 1년간 20% 성장 → CAGR ≈ 20%
        assert!(cagr > dec!(19) && cagr < dec!(21));
    }

    #[test]
    fn test_analyze_drawdowns() {
        let curve = create_test_curve();
        let drawdowns = curve.analyze_drawdowns();

        // 최소 1개의 Drawdown 기간이 있어야 함
        assert!(!drawdowns.is_empty());

        // 가장 큰 Drawdown이 첫 번째
        let largest = &drawdowns[0];
        assert!(largest.max_drawdown_pct > Decimal::ZERO);
    }

    #[test]
    fn test_filter_range() {
        let curve = create_test_curve();
        let base_time = Utc::now() - Duration::days(30);

        let filtered = curve.filter_range(
            base_time + Duration::days(10),
            base_time + Duration::days(25),
        );

        // 10일~25일 사이의 데이터만
        assert!(filtered.len() < curve.len());
    }

    #[test]
    fn test_monthly_returns() {
        let mut builder = EquityCurveBuilder::new(dec!(10_000_000));
        let base_time = Utc::now() - Duration::days(60);

        builder.add_trade_result(base_time, dec!(10_000_000));
        builder.add_trade_result(base_time + Duration::days(30), dec!(10_500_000));
        builder.add_trade_result(base_time + Duration::days(60), dec!(11_000_000));

        let curve = builder.build();
        let monthly = curve.monthly_returns();

        // 월별 수익률이 계산되어야 함
        assert!(!monthly.is_empty());
    }

    #[test]
    fn test_empty_curve() {
        let curve = EquityCurve::new(dec!(10_000_000));

        assert!(curve.is_empty());
        assert_eq!(curve.current_equity(), dec!(10_000_000));
        assert_eq!(curve.total_return(), Decimal::ZERO);
        assert_eq!(curve.cagr(), Decimal::ZERO);
    }
}
