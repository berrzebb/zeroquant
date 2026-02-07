//! 회귀 테스트용 차트 이미지 생성 모듈.
//!
//! 백테스트 결과를 시각화하여 PNG 이미지로 저장합니다.
//!
//! # 생성되는 차트 (3패널 레이아웃)
//!
//! 1. **캔들스틱 차트 + Volume**: 실제 가격 움직임과 거래량, 신호 마커 표시
//! 2. **자산 곡선 (Equity Curve)**: 시간에 따른 포트폴리오 가치 변화
//! 3. **낙폭 차트 (Drawdown Chart)**: 고점 대비 하락률
//!
//! # 기술적 참고
//!
//! plotters의 RangedDateTime은 내부적으로 나노초 계산 시 overflow가 발생할 수 있어,
//! 타임스탬프를 f64로 변환하여 안전하게 처리합니다.

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use plotters::prelude::*;
use rust_decimal::Decimal;
use std::path::Path;
use trader_analytics::backtest::BacktestReport;
use trader_analytics::performance::EquityPoint;
use trader_core::{Kline, Side, SignalMarker, SignalType};

/// 차트 생성 설정
#[derive(Debug, Clone)]
pub struct ChartConfig {
    /// 차트 너비 (픽셀)
    pub width: u32,
    /// 차트 높이 (픽셀)
    pub height: u32,
    /// 배경색
    pub background_color: RGBColor,
    /// 자산 곡선 색상
    pub equity_color: RGBColor,
    /// 낙폭 색상
    pub drawdown_color: RGBColor,
    /// 상승 캔들 색상
    pub candle_up_color: RGBColor,
    /// 하락 캔들 색상
    pub candle_down_color: RGBColor,
    /// Volume 색상
    #[allow(dead_code)]
    pub volume_color: RGBColor,
    /// 그리드 표시 여부
    #[allow(dead_code)]
    pub show_grid: bool,
}

impl Default for ChartConfig {
    fn default() -> Self {
        Self {
            width: 1600,
            height: 1000,
            background_color: WHITE,
            equity_color: RGBColor(0, 100, 180),
            drawdown_color: RGBColor(200, 50, 50),
            candle_up_color: RGBColor(0, 150, 0),
            candle_down_color: RGBColor(200, 0, 0),
            volume_color: RGBColor(100, 100, 200),
            show_grid: true,
        }
    }
}

/// 회귀 테스트 차트 생성기
pub struct RegressionChartGenerator {
    config: ChartConfig,
}

impl RegressionChartGenerator {
    /// 기본 설정으로 생성
    pub fn new() -> Self {
        Self {
            config: ChartConfig::default(),
        }
    }

    /// 사용자 지정 설정으로 생성
    #[allow(dead_code)]
    pub fn with_config(config: ChartConfig) -> Self {
        Self { config }
    }

    /// 백테스트 결과에서 3패널 복합 차트 생성
    ///
    /// 헤더(메트릭스) + 캔들스틱(Volume, 시그널) + 자산곡선 + 낙폭을 함께 표시합니다.
    pub fn generate_combined_chart(
        &self,
        report: &BacktestReport,
        strategy_name: &str,
        output_path: &Path,
    ) -> Result<()> {
        // 최소 2개 이상의 데이터 포인트가 필요 (plotters 오버플로우 방지)
        if report.equity_curve.len() < 2 {
            return Err(anyhow::anyhow!(
                "자산 곡선 데이터가 부족합니다 ({} 포인트)",
                report.equity_curve.len()
            ));
        }

        let root = BitMapBackend::new(output_path, (self.config.width, self.config.height))
            .into_drawing_area();
        root.fill(&self.config.background_color)?;

        // 헤더 영역 (10%)과 차트 영역 (90%) 분리
        let (header_area, chart_area) = root.split_vertically(self.config.height / 10);

        // 헤더 그리기 (메트릭스 정보)
        self.draw_header(&header_area, report, strategy_name)?;

        // 캔들 데이터가 충분하면 3패널, 부족하면 2패널 (최소 2개 필요)
        if report.klines.len() >= 2 {
            // 3패널: 캔들(45%) + Equity(30%) + Drawdown(25%)
            let chart_height = (self.config.height / 10) * 9;  // 900
            let upper_height = (chart_height * 45) / 100;      // 405 (캔들)
            let middle_height = (chart_height * 30) / 100;     // 270 (Equity)
            // lower = 225 (Drawdown)
            let (upper, lower_combined) = chart_area.split_vertically(upper_height);
            let (middle, lower) = lower_combined.split_vertically(middle_height);

            let (time_range, equity_range, drawdown_range) =
                self.calculate_ranges(&report.equity_curve);
            let (candle_time_range, price_range, volume_range) =
                self.calculate_candle_ranges(&report.klines);

            // 상단: 캔들스틱 + Volume + 신호 마커
            self.draw_candlestick_chart(
                &upper,
                &report.klines,
                &report.signal_markers,
                &candle_time_range,
                &price_range,
                &volume_range,
            )?;

            // 중간: 자산 곡선
            self.draw_equity_curve(
                &middle,
                &report.equity_curve,
                &[],
                "Equity Curve",
                &time_range,
                &equity_range,
            )?;

            // 하단: 낙폭 차트
            self.draw_drawdown_chart(&lower, &report.equity_curve, &time_range, &drawdown_range)?;
        } else {
            // 2패널 (캔들 없음): Equity(70%) + Drawdown(30%)
            let chart_height = (self.config.height / 10) * 9;
            let (upper, lower) = chart_area.split_vertically((chart_height / 10) * 7);

            let (time_range, equity_range, drawdown_range) =
                self.calculate_ranges(&report.equity_curve);

            // 상단: 자산 곡선 (신호 마커 포함)
            self.draw_equity_curve(
                &upper,
                &report.equity_curve,
                &report.signal_markers,
                "Equity Curve",
                &time_range,
                &equity_range,
            )?;

            // 하단: 낙폭 차트
            self.draw_drawdown_chart(&lower, &report.equity_curve, &time_range, &drawdown_range)?;
        }

        root.present()?;
        Ok(())
    }

    /// 헤더 영역에 메트릭스 정보 그리기
    fn draw_header<DB: DrawingBackend>(
        &self,
        area: &DrawingArea<DB, plotters::coord::Shift>,
        report: &BacktestReport,
        strategy_name: &str,
    ) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
        let metrics = &report.metrics;

        // 배경색 (연한 회색)
        area.fill(&RGBColor(245, 245, 245))?;

        // 전략명 + 심볼
        let symbol_info = if !report.symbol.is_empty() {
            format!("{} | {}", strategy_name, report.symbol)
        } else {
            strategy_name.to_string()
        };

        // 좌측: 전략명 + 심볼
        let title_style = ("sans-serif", 28).into_font().color(&BLACK);
        area.draw_text(&symbol_info, &title_style, (20, 20))?;

        // 메트릭스 포맷팅
        let initial_capital = decimal_to_f64(report.config.initial_capital);
        let net_profit = decimal_to_f64(metrics.net_profit);
        let final_capital = initial_capital + net_profit;
        let total_return = decimal_to_f64(metrics.total_return_pct);
        let cagr = decimal_to_f64(metrics.annualized_return_pct);
        let mdd = decimal_to_f64(metrics.max_drawdown_pct);
        let sharpe = decimal_to_f64(metrics.sharpe_ratio);
        let win_rate = decimal_to_f64(metrics.win_rate_pct);
        let total_trades = metrics.total_trades;

        // 우측: 메트릭스 (2줄)
        let metric_style = ("sans-serif", 16).into_font().color(&RGBColor(50, 50, 50));
        let value_style = ("sans-serif", 16).into_font().color(&BLACK);

        // 첫 번째 줄
        let line1 = format!(
            "초기 자본: {}  |  최종 자본: {}  |  총 수익률: {:.2}%  |  CAGR: {:.2}%",
            format_number(initial_capital),
            format_number(final_capital),
            total_return,
            cagr
        );
        area.draw_text(&line1, &metric_style, (400, 25))?;

        // 두 번째 줄
        let mdd_color = if mdd > 20.0 {
            RGBColor(200, 0, 0)
        } else {
            RGBColor(50, 50, 50)
        };
        let _mdd_style = ("sans-serif", 16).into_font().color(&mdd_color);

        let line2_prefix = format!(
            "MDD: {:.2}%  |  샤프 비율: {:.2}  |  승률: {:.1}%  |  총 거래: {}건",
            mdd, sharpe, win_rate, total_trades
        );
        area.draw_text(&line2_prefix, &value_style, (400, 55))?;

        // 투자금액 (있는 경우)
        if initial_capital > 0.0 {
            let invest_text = format!("투자금액: {}원", format_number(initial_capital));
            area.draw_text(&invest_text, &value_style, (20, 55))?;
        }

        Ok(())
    }

    /// 캔들스틱 차트 + Volume + 신호 마커 그리기 (f64 타임스탬프 사용)
    fn draw_candlestick_chart<DB: DrawingBackend>(
        &self,
        area: &DrawingArea<DB, plotters::coord::Shift>,
        klines: &[Kline],
        signal_markers: &[SignalMarker],
        time_range: &std::ops::Range<f64>,
        price_range: &std::ops::Range<f64>,
        volume_range: &std::ops::Range<f64>,
    ) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
        // 캔들 영역 (80%)과 볼륨 영역 (20%) 분리
        let area_height = area.dim_in_pixel().1;
        let (candle_area, volume_area) = area.split_vertically((area_height / 10) * 8);

        // 캔들스틱 차트 (f64 좌표계 사용)
        let mut candle_chart = ChartBuilder::on(&candle_area)
            .caption("Price Chart", ("sans-serif", 18).into_font())
            .margin(10)
            .x_label_area_size(0)
            .y_label_area_size(80)
            .build_cartesian_2d(time_range.clone(), price_range.clone())?;

        candle_chart
            .configure_mesh()
            .x_labels(0)
            .y_labels(8)
            .y_label_formatter(&|v| format!("{:.0}", v))
            .draw()?;

        // 캔들 너비 계산 (초 단위)
        let candle_width_sec = if klines.len() >= 2 {
            let time_diff = klines[1].open_time.timestamp() - klines[0].open_time.timestamp();
            (time_diff as f64) * 0.8
        } else {
            14400.0 // 4시간
        };

        // 캔들 그리기
        for kline in klines {
            let open = decimal_to_f64(kline.open);
            let high = decimal_to_f64(kline.high);
            let low = decimal_to_f64(kline.low);
            let close = decimal_to_f64(kline.close);
            let open_ts = kline.open_time.timestamp() as f64;
            let close_ts = kline.close_time.timestamp() as f64;

            let is_up = close >= open;
            let color = if is_up {
                &self.config.candle_up_color
            } else {
                &self.config.candle_down_color
            };

            // 심지 (위아래 라인)
            let wick_time = (open_ts + close_ts) / 2.0;
            candle_chart.draw_series(LineSeries::new(
                vec![(wick_time, low), (wick_time, high)],
                color,
            ))?;

            // 몸통 (사각형)
            let body_top = open.max(close);
            let body_bottom = open.min(close);
            let left_time = open_ts + candle_width_sec * 0.125;
            let right_time = close_ts - candle_width_sec * 0.125;

            candle_chart.draw_series(std::iter::once(Rectangle::new(
                [(left_time, body_bottom), (right_time, body_top)],
                color.filled(),
            )))?;
        }

        // 신호 마커 그리기 (캔들 차트 위에)
        self.add_signal_markers_to_candle(&mut candle_chart, signal_markers, klines)?;

        // 볼륨 차트 (f64 좌표계 사용)
        let mut volume_chart = ChartBuilder::on(&volume_area)
            .margin(10)
            .x_label_area_size(40)
            .y_label_area_size(80)
            .build_cartesian_2d(time_range.clone(), volume_range.clone())?;

        volume_chart
            .configure_mesh()
            .x_labels(10)
            .y_labels(3)
            .y_label_formatter(&|v| format_volume(*v))
            .x_label_formatter(&|ts| timestamp_to_date_str(*ts))
            .draw()?;

        // 볼륨 바 그리기
        for kline in klines {
            let volume = decimal_to_f64(kline.volume);
            let is_up = decimal_to_f64(kline.close) >= decimal_to_f64(kline.open);
            let color = if is_up {
                self.config.candle_up_color.mix(0.5)
            } else {
                self.config.candle_down_color.mix(0.5)
            };

            let open_ts = kline.open_time.timestamp() as f64;
            let close_ts = kline.close_time.timestamp() as f64;
            let left_time = open_ts + candle_width_sec * 0.125;
            let right_time = close_ts - candle_width_sec * 0.125;

            volume_chart.draw_series(std::iter::once(Rectangle::new(
                [(left_time, 0.0), (right_time, volume)],
                color.filled(),
            )))?;
        }

        Ok(())
    }

    /// 캔들 차트에 신호 마커 추가 (f64 타임스탬프 좌표계)
    fn add_signal_markers_to_candle<DB: DrawingBackend>(
        &self,
        chart: &mut ChartContext<DB, Cartesian2d<plotters::coord::types::RangedCoordf64, plotters::coord::types::RangedCoordf64>>,
        signal_markers: &[SignalMarker],
        klines: &[Kline],
    ) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
        if signal_markers.is_empty() || klines.is_empty() {
            return Ok(());
        }

        // 시간별 캔들 가격 맵 생성
        let find_price_at_time = |timestamp: DateTime<Utc>| -> Option<(f64, f64)> {
            klines
                .iter()
                .find(|k| k.open_time <= timestamp && timestamp <= k.close_time)
                .map(|k| (decimal_to_f64(k.high), decimal_to_f64(k.low)))
        };

        for marker in signal_markers {
            let Some((high, low)) = find_price_at_time(marker.timestamp) else {
                continue;
            };

            let ts = marker.timestamp.timestamp() as f64;
            let alpha = if marker.executed { 1.0 } else { 0.5 };

            match marker.signal_type {
                SignalType::Entry => {
                    let (y_pos, color, offset) = match marker.side {
                        Some(Side::Buy) => (low, RGBColor(0, 180, 0).mix(alpha), 8),
                        Some(Side::Sell) => (high, RGBColor(180, 0, 0).mix(alpha), -8),
                        None => (low, RGBColor(100, 100, 100).mix(alpha), 0),
                    };

                    chart.draw_series(PointSeries::of_element(
                        vec![(ts, y_pos)],
                        10,
                        &color,
                        &move |coord, size, style| {
                            let tri_size = if offset > 0 {
                                size as i32
                            } else {
                                -(size as i32)
                            };
                            EmptyElement::at(coord)
                                + TriangleMarker::new((0, offset), tri_size, style.filled())
                        },
                    ))?;
                }
                SignalType::Exit => {
                    let (y_pos, color) = match marker.side {
                        Some(Side::Sell) => (high, RGBColor(0, 180, 180).mix(alpha)),
                        Some(Side::Buy) => (low, RGBColor(255, 140, 0).mix(alpha)),
                        None => ((high + low) / 2.0, RGBColor(128, 128, 128).mix(alpha)),
                    };

                    chart.draw_series(PointSeries::of_element(
                        vec![(ts, y_pos)],
                        8,
                        &color,
                        &|coord, size, style| {
                            EmptyElement::at(coord) + Cross::new((0, 0), size, style.stroke_width(2))
                        },
                    ))?;
                }
                SignalType::AddToPosition => {
                    let color = RGBColor(0, 200, 100).mix(alpha);
                    chart.draw_series(PointSeries::of_element(
                        vec![(ts, low)],
                        5,
                        &color,
                        &|coord, size, style| {
                            EmptyElement::at(coord) + Circle::new((0, 10), size, style.filled())
                        },
                    ))?;
                }
                SignalType::ReducePosition => {
                    let color = RGBColor(255, 165, 0).mix(alpha);
                    chart.draw_series(PointSeries::of_element(
                        vec![(ts, high)],
                        5,
                        &color,
                        &|coord, size, style| {
                            EmptyElement::at(coord) + Circle::new((0, -10), size, style.filled())
                        },
                    ))?;
                }
                SignalType::Alert => {
                    let color = RGBColor(100, 100, 255).mix(0.5);
                    chart.draw_series(PointSeries::of_element(
                        vec![(ts, (high + low) / 2.0)],
                        4,
                        &color,
                        &|coord, size, style| {
                            EmptyElement::at(coord)
                                + Circle::new((0, 0), size, style.stroke_width(1))
                        },
                    ))?;
                }
                SignalType::Scale => {
                    let color = RGBColor(150, 0, 150).mix(alpha);
                    chart.draw_series(PointSeries::of_element(
                        vec![(ts, (high + low) / 2.0)],
                        5,
                        &color,
                        &|coord, size, style| {
                            EmptyElement::at(coord)
                                + Circle::new((0, 0), size, style.stroke_width(2))
                        },
                    ))?;
                }
            }
        }

        Ok(())
    }

    /// 자산 곡선만 생성
    #[allow(dead_code)]
    pub fn generate_equity_chart(
        &self,
        report: &BacktestReport,
        strategy_name: &str,
        output_path: &Path,
    ) -> Result<()> {
        if report.equity_curve.is_empty() {
            return Err(anyhow::anyhow!("자산 곡선 데이터가 비어있습니다"));
        }

        let root = BitMapBackend::new(output_path, (self.config.width, self.config.height))
            .into_drawing_area();
        root.fill(&self.config.background_color)?;

        let (time_range, equity_range, _) = self.calculate_ranges(&report.equity_curve);

        self.draw_equity_curve(
            &root,
            &report.equity_curve,
            &report.signal_markers,
            strategy_name,
            &time_range,
            &equity_range,
        )?;

        root.present()?;
        Ok(())
    }

    /// 캔들 데이터 범위 계산 (f64 타임스탬프 사용 - overflow 방지)
    fn calculate_candle_ranges(
        &self,
        klines: &[Kline],
    ) -> (
        std::ops::Range<f64>,
        std::ops::Range<f64>,
        std::ops::Range<f64>,
    ) {
        let now = Utc::now().timestamp() as f64;
        let start_time = klines
            .first()
            .map(|k| k.open_time.timestamp() as f64)
            .unwrap_or(now);
        let mut end_time = klines
            .last()
            .map(|k| k.close_time.timestamp() as f64)
            .unwrap_or(now);

        // 시간 범위가 같으면 최소 1일 차이 추가
        if end_time <= start_time {
            end_time = start_time + 86400.0; // 1일 = 86400초
        }

        let prices: Vec<f64> = klines
            .iter()
            .flat_map(|k| vec![decimal_to_f64(k.high), decimal_to_f64(k.low)])
            .collect();

        let volumes: Vec<f64> = klines.iter().map(|k| decimal_to_f64(k.volume)).collect();

        let min_price = prices
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min)
            .max(0.0);
        let max_price = prices
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max)
            .max(min_price + 1.0);
        let max_volume = volumes.iter().cloned().fold(0.0, f64::max).max(1.0);

        let price_margin = ((max_price - min_price) * 0.1).max(1.0);

        (
            start_time..end_time,
            (min_price - price_margin)..(max_price + price_margin),
            0.0..(max_volume * 1.2),
        )
    }

    /// 데이터 범위 계산 (f64 타임스탬프 사용 - overflow 방지)
    fn calculate_ranges(
        &self,
        equity_curve: &[EquityPoint],
    ) -> (
        std::ops::Range<f64>,
        std::ops::Range<f64>,
        std::ops::Range<f64>,
    ) {
        let now = Utc::now().timestamp() as f64;
        let start_time = equity_curve
            .first()
            .map(|p| p.timestamp.timestamp() as f64)
            .unwrap_or(now);
        let mut end_time = equity_curve
            .last()
            .map(|p| p.timestamp.timestamp() as f64)
            .unwrap_or(now);

        // 시간 범위가 같으면 최소 1일 차이 추가
        if end_time <= start_time {
            end_time = start_time + 86400.0;
        }

        let equities: Vec<f64> = equity_curve
            .iter()
            .map(|p| decimal_to_f64(p.equity))
            .collect();

        let drawdowns: Vec<f64> = equity_curve
            .iter()
            .map(|p| decimal_to_f64(p.drawdown_pct))
            .collect();

        let min_equity = equities
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min)
            .max(0.0);
        let max_equity = equities
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max)
            .max(min_equity + 1.0);

        let min_dd = drawdowns.iter().cloned().fold(0.0, f64::min);
        let max_dd = drawdowns.iter().cloned().fold(0.0, f64::max).max(0.1);

        let equity_margin = ((max_equity - min_equity) * 0.1).max(1.0);
        let dd_margin = ((max_dd - min_dd).abs() * 0.1).max(0.1);

        // drawdown_range: draw_drawdown_chart에서 음수로 변환하므로 음수 범위 반환
        // max_dd가 가장 큰 하락(양수), 음수로 변환하면 가장 낮은 값
        let drawdown_range = (-(max_dd + dd_margin))..dd_margin;

        (
            start_time..end_time,
            (min_equity - equity_margin)..(max_equity + equity_margin),
            drawdown_range,
        )
    }

    /// 자산 곡선 차트 그리기 (f64 타임스탬프 좌표계)
    fn draw_equity_curve<DB: DrawingBackend>(
        &self,
        area: &DrawingArea<DB, plotters::coord::Shift>,
        equity_curve: &[EquityPoint],
        signal_markers: &[SignalMarker],
        caption: &str,
        time_range: &std::ops::Range<f64>,
        equity_range: &std::ops::Range<f64>,
    ) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
        let mut chart = ChartBuilder::on(area)
            .caption(caption, ("sans-serif", 18).into_font())
            .margin(10)
            .x_label_area_size(40)
            .y_label_area_size(80)
            .build_cartesian_2d(time_range.clone(), equity_range.clone())?;

        chart
            .configure_mesh()
            .x_labels(10)
            .y_labels(8)
            .y_label_formatter(&|v| format_currency(*v))
            .x_label_formatter(&|ts| timestamp_to_month_str(*ts))
            .draw()?;

        // 자산 곡선 라인 (f64 타임스탬프)
        let data: Vec<(f64, f64)> = equity_curve
            .iter()
            .map(|p| (p.timestamp.timestamp() as f64, decimal_to_f64(p.equity)))
            .collect();

        chart.draw_series(LineSeries::new(data.clone(), &self.config.equity_color))?;

        // 영역 채우기 (반투명)
        let fill_color = self.config.equity_color.mix(0.2);
        chart.draw_series(AreaSeries::new(
            data.iter().cloned(),
            equity_range.start,
            fill_color,
        ))?;

        // 주요 지점 마커 (시작/종료/MDD)
        self.add_equity_markers(&mut chart, equity_curve)?;

        // 신호 마커 (캔들 차트가 없을 때만)
        if !signal_markers.is_empty() {
            self.add_signal_markers(&mut chart, signal_markers, equity_curve)?;
        }

        Ok(())
    }

    /// 낙폭 차트 그리기 (f64 타임스탬프 좌표계)
    fn draw_drawdown_chart<DB: DrawingBackend>(
        &self,
        area: &DrawingArea<DB, plotters::coord::Shift>,
        equity_curve: &[EquityPoint],
        time_range: &std::ops::Range<f64>,
        drawdown_range: &std::ops::Range<f64>,
    ) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
        let mut chart = ChartBuilder::on(area)
            .caption("Drawdown %", ("sans-serif", 16).into_font())
            .margin(10)
            .x_label_area_size(40)
            .y_label_area_size(80)
            .build_cartesian_2d(time_range.clone(), drawdown_range.clone())?;

        chart
            .configure_mesh()
            .x_labels(10)
            .y_labels(5)
            .y_label_formatter(&|v| format!("{:.1}%", v))
            .x_label_formatter(&|ts| timestamp_to_month_str(*ts))
            .draw()?;

        // 0% 기준선
        chart.draw_series(LineSeries::new(
            vec![(time_range.start, 0.0), (time_range.end, 0.0)],
            &BLACK.mix(0.3),
        ))?;

        // 낙폭 영역 (f64 타임스탬프)
        let data: Vec<(f64, f64)> = equity_curve
            .iter()
            .map(|p| {
                (
                    p.timestamp.timestamp() as f64,
                    -decimal_to_f64(p.drawdown_pct),
                )
            })
            .collect();

        let fill_color = self.config.drawdown_color.mix(0.4);
        chart.draw_series(AreaSeries::new(data.iter().cloned(), 0.0, fill_color))?;

        chart.draw_series(LineSeries::new(data, &self.config.drawdown_color))?;

        Ok(())
    }

    /// 자산 곡선에 주요 지점 마커 추가 (f64 타임스탬프 좌표계)
    fn add_equity_markers<DB: DrawingBackend>(
        &self,
        chart: &mut ChartContext<
            DB,
            Cartesian2d<plotters::coord::types::RangedCoordf64, plotters::coord::types::RangedCoordf64>,
        >,
        equity_curve: &[EquityPoint],
    ) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
        if equity_curve.is_empty() {
            return Ok(());
        }

        // 시작점
        let start = &equity_curve[0];
        let start_ts = start.timestamp.timestamp() as f64;
        chart.draw_series(PointSeries::of_element(
            vec![(start_ts, decimal_to_f64(start.equity))],
            5,
            &GREEN,
            &|coord, size, style| {
                EmptyElement::at(coord) + Circle::new((0, 0), size, style.filled())
            },
        ))?;

        // 종료점
        let end = &equity_curve[equity_curve.len() - 1];
        let end_ts = end.timestamp.timestamp() as f64;
        let end_color = if end.equity >= start.equity {
            &GREEN
        } else {
            &RED
        };
        chart.draw_series(PointSeries::of_element(
            vec![(end_ts, decimal_to_f64(end.equity))],
            5,
            end_color,
            &|coord, size, style| {
                EmptyElement::at(coord) + Circle::new((0, 0), size, style.filled())
            },
        ))?;

        // 최대 낙폭 지점
        if let Some(max_dd_point) = equity_curve.iter().max_by(|a, b| {
            a.drawdown_pct
                .partial_cmp(&b.drawdown_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            let mdd_ts = max_dd_point.timestamp.timestamp() as f64;
            chart.draw_series(PointSeries::of_element(
                vec![(mdd_ts, decimal_to_f64(max_dd_point.equity))],
                7,
                &RED,
                &|coord, size, style| {
                    EmptyElement::at(coord)
                        + Circle::new((0, 0), size, style.stroke_width(2))
                        + Text::new("MDD", (10, -10), ("sans-serif", 12).into_font())
                },
            ))?;
        }

        Ok(())
    }

    /// 신호 마커를 Equity 차트에 추가 (f64 타임스탬프 좌표계)
    fn add_signal_markers<DB: DrawingBackend>(
        &self,
        chart: &mut ChartContext<
            DB,
            Cartesian2d<plotters::coord::types::RangedCoordf64, plotters::coord::types::RangedCoordf64>,
        >,
        signal_markers: &[SignalMarker],
        equity_curve: &[EquityPoint],
    ) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
        if signal_markers.is_empty() || equity_curve.is_empty() {
            return Ok(());
        }

        let find_equity_at_time = |timestamp: DateTime<Utc>| -> f64 {
            equity_curve
                .iter()
                .min_by_key(|ep| (ep.timestamp.timestamp() - timestamp.timestamp()).abs())
                .map(|ep| decimal_to_f64(ep.equity))
                .unwrap_or(0.0)
        };

        for marker in signal_markers {
            let ts = marker.timestamp.timestamp() as f64;
            let y_value = find_equity_at_time(marker.timestamp);
            let alpha = if marker.executed { 1.0 } else { 0.5 };

            match marker.signal_type {
                SignalType::Entry => {
                    let (color, offset) = match marker.side {
                        Some(Side::Buy) => (RGBColor(0, 180, 0).mix(alpha), -15),
                        Some(Side::Sell) => (RGBColor(180, 0, 0).mix(alpha), 15),
                        None => (RGBColor(100, 100, 100).mix(alpha), 0),
                    };
                    chart.draw_series(PointSeries::of_element(
                        vec![(ts, y_value)],
                        8,
                        &color,
                        &move |coord, size, style| {
                            let triangle = if offset < 0 {
                                TriangleMarker::new((0, offset), size, style.filled())
                            } else {
                                TriangleMarker::new((0, offset), -(size as i32), style.filled())
                            };
                            EmptyElement::at(coord) + triangle
                        },
                    ))?;
                }
                SignalType::Exit => {
                    let color = match marker.side {
                        Some(Side::Sell) => RGBColor(0, 180, 180).mix(alpha),
                        Some(Side::Buy) => RGBColor(255, 140, 0).mix(alpha),
                        None => RGBColor(128, 128, 128).mix(alpha),
                    };
                    chart.draw_series(PointSeries::of_element(
                        vec![(ts, y_value)],
                        6,
                        &color,
                        &|coord, size, style| {
                            EmptyElement::at(coord) + Cross::new((0, 0), size, style.stroke_width(2))
                        },
                    ))?;
                }
                SignalType::AddToPosition => {
                    let color = RGBColor(0, 200, 100).mix(alpha);
                    chart.draw_series(PointSeries::of_element(
                        vec![(ts, y_value)],
                        4,
                        &color,
                        &|coord, size, style| {
                            EmptyElement::at(coord) + Circle::new((0, -10), size, style.filled())
                        },
                    ))?;
                }
                SignalType::ReducePosition => {
                    let color = RGBColor(255, 165, 0).mix(alpha);
                    chart.draw_series(PointSeries::of_element(
                        vec![(ts, y_value)],
                        4,
                        &color,
                        &|coord, size, style| {
                            EmptyElement::at(coord) + Circle::new((0, 10), size, style.filled())
                        },
                    ))?;
                }
                SignalType::Alert => {
                    let color = RGBColor(100, 100, 255).mix(0.5);
                    chart.draw_series(PointSeries::of_element(
                        vec![(ts, y_value)],
                        3,
                        &color,
                        &|coord, size, style| {
                            EmptyElement::at(coord)
                                + Circle::new((0, -8), size, style.stroke_width(1))
                        },
                    ))?;
                }
                SignalType::Scale => {
                    let color = RGBColor(150, 0, 150).mix(alpha);
                    chart.draw_series(PointSeries::of_element(
                        vec![(ts, y_value)],
                        4,
                        &color,
                        &|coord, size, style| {
                            EmptyElement::at(coord)
                                + Circle::new((0, 0), size, style.stroke_width(2))
                        },
                    ))?;
                }
            }
        }

        Ok(())
    }
}

impl Default for RegressionChartGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Decimal을 f64로 변환
fn decimal_to_f64(d: Decimal) -> f64 {
    d.to_string().parse().unwrap_or(0.0)
}

/// f64 타임스탬프를 날짜 문자열로 변환
fn timestamp_to_date_str(ts: f64) -> String {
    Utc.timestamp_opt(ts as i64, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

/// f64 타임스탬프를 연월 문자열로 변환
fn timestamp_to_month_str(ts: f64) -> String {
    Utc.timestamp_opt(ts as i64, 0)
        .single()
        .map(|dt| dt.format("%Y-%m").to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

/// 숫자를 천 단위 구분자로 포맷
fn format_number(v: f64) -> String {
    let int_part = v as i64;
    let formatted = int_part
        .to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect::<Vec<_>>()
        .join(",");
    formatted
}

/// 통화 형식으로 포맷
fn format_currency(v: f64) -> String {
    if v >= 1_000_000_000.0 {
        format!("{:.1}B", v / 1_000_000_000.0)
    } else if v >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if v >= 1_000.0 {
        format!("{:.0}K", v / 1_000.0)
    } else {
        format!("{:.0}", v)
    }
}

/// 볼륨 형식으로 포맷
fn format_volume(v: f64) -> String {
    if v >= 1_000_000_000.0 {
        format!("{:.1}B", v / 1_000_000_000.0)
    } else if v >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if v >= 1_000.0 {
        format!("{:.0}K", v / 1_000.0)
    } else {
        format!("{:.0}", v)
    }
}

/// 회귀 테스트 결과 차트 일괄 생성
#[allow(dead_code)]
pub fn generate_regression_charts(
    results: &[(String, String, BacktestReport)],
    output_dir: &Path,
) -> Result<Vec<String>> {
    std::fs::create_dir_all(output_dir)?;

    let generator = RegressionChartGenerator::new();
    let mut generated_files = Vec::new();

    for (strategy_id, name, report) in results {
        if report.equity_curve.is_empty() {
            println!("  {} - 자산 곡선 데이터 없음 (차트 생략)", strategy_id);
            continue;
        }

        let filename = format!("{}_chart.png", strategy_id);
        let output_path = output_dir.join(&filename);

        match generator.generate_combined_chart(report, name, &output_path) {
            Ok(()) => {
                generated_files.push(output_path.display().to_string());
                println!("  {} - 차트 생성 완료: {}", strategy_id, filename);
            }
            Err(e) => {
                println!("  {} - 차트 생성 실패: {}", strategy_id, e);
            }
        }
    }

    Ok(generated_files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal::prelude::FromPrimitive;

    fn create_test_equity_curve() -> Vec<EquityPoint> {
        let base = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        (0..100)
            .map(|i| {
                let equity = Decimal::from(10_000_000) + Decimal::from(i * 10000);
                let drawdown = if i > 50 {
                    Decimal::from_f64((i - 50) as f64 / 10.0).unwrap_or(Decimal::ZERO)
                } else {
                    Decimal::ZERO
                };
                EquityPoint {
                    timestamp: base + chrono::Duration::days(i),
                    equity,
                    drawdown_pct: drawdown,
                }
            })
            .collect()
    }

    #[test]
    fn test_chart_generation_config() {
        let config = ChartConfig::default();
        assert_eq!(config.width, 1600);
        assert_eq!(config.height, 1000);
    }

    #[test]
    fn test_calculate_ranges() {
        let generator = RegressionChartGenerator::new();
        let equity_curve = create_test_equity_curve();

        let (time_range, equity_range, _dd_range) = generator.calculate_ranges(&equity_curve);

        assert!(time_range.start < time_range.end);
        assert!(equity_range.start < equity_range.end);
    }

    #[test]
    fn test_format_currency() {
        assert_eq!(format_currency(1_500_000_000.0), "1.5B");
        assert_eq!(format_currency(2_500_000.0), "2.5M");
        assert_eq!(format_currency(50_000.0), "50K");
        assert_eq!(format_currency(500.0), "500");
    }
}
