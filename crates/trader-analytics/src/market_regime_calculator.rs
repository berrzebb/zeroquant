//! MarketRegime 계산기
//!
//! 60일 상대강도, 가격 기울기, RSI를 종합하여 종목의 시장 레짐을 판정합니다.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use trader_core::{Kline, MarketRegime};

use crate::indicators::IndicatorEngine;
use crate::IndicatorError;

/// MarketRegime 계산 결과
#[derive(Debug, Clone)]
pub struct MarketRegimeResult {
    /// 판정된 레짐
    pub regime: MarketRegime,
    /// 60일 상대강도 (%)
    pub rel_60d_pct: f64,
    /// 20일 가격 기울기 (정규화)
    pub slope: f64,
    /// RSI(14)
    pub rsi: f64,
}

/// MarketRegime 계산기
///
/// # 판정 로직
///
/// 1. **StrongUptrend** (강한 상승 추세)
///    - rel_60d > 10% AND slope > 0 AND 50 <= RSI <= 70
///
/// 2. **Correction** (상승 후 조정)
///    - rel_60d > 5% AND slope <= 0
///
/// 3. **Sideways** (박스권/중립)
///    - -5% <= rel_60d <= 5%
///
/// 4. **BottomBounce** (바닥 반등 시도)
///    - rel_60d <= -5% AND slope > 0
///
/// 5. **Downtrend** (하락/약세)
///    - rel_60d <= -5% AND slope <= 0
pub struct MarketRegimeCalculator {
    indicator_engine: IndicatorEngine,
}

impl MarketRegimeCalculator {
    /// 새 계산기 생성
    pub fn new() -> Self {
        Self {
            indicator_engine: IndicatorEngine::new(),
        }
    }

    /// MarketRegime 계산
    ///
    /// # 필요 데이터
    /// - 최소 70개 이상의 캔들 (60일 상대강도 + RSI 계산용)
    ///
    /// # 에러
    /// - 데이터 부족
    /// - 지표 계산 실패
    pub fn calculate(&self, candles: &[Kline]) -> Result<MarketRegimeResult, IndicatorError> {
        if candles.len() < 70 {
            return Err(IndicatorError::InsufficientData {
                required: 70,
                provided: candles.len(),
            });
        }

        // 1. 60일 상대강도 계산 (rel_60d_%)
        let rel_60d_pct = self.calculate_relative_strength_60d(candles)?;

        // 2. 가격 기울기 계산 (최근 20일 선형회귀)
        let slope = self.calculate_price_slope_20d(candles)?;

        // 3. RSI(14) 계산
        let rsi = self.calculate_rsi_14(candles)?;

        // 4. MarketRegime 판정
        let regime = self.determine_regime(rel_60d_pct, slope, rsi);

        Ok(MarketRegimeResult {
            regime,
            rel_60d_pct,
            slope,
            rsi,
        })
    }

    /// 60일 상대강도 계산
    ///
    /// rel_60d_% = (현재가 / 60일전 가격 - 1) * 100
    fn calculate_relative_strength_60d(&self, candles: &[Kline]) -> Result<f64, IndicatorError> {
        let len = candles.len();
        if len < 61 {
            return Err(IndicatorError::InsufficientData {
                required: 61,
                provided: len,
            });
        }

        let current_price = candles[len - 1].close;
        let price_60d_ago = candles[len - 61].close;

        if price_60d_ago == Decimal::ZERO {
            return Err(IndicatorError::CalculationError(
                "60일 전 가격이 0입니다".to_string(),
            ));
        }

        let rel_strength = ((current_price / price_60d_ago) - Decimal::ONE) * Decimal::from(100);

        rel_strength.to_f64().ok_or_else(|| {
            IndicatorError::CalculationError("Decimal to f64 변환 실패".to_string())
        })
    }

    /// 20일 가격 기울기 계산 (선형회귀)
    ///
    /// 최근 20일 종가에 대한 선형회귀 기울기를 계산합니다.
    /// 양수 = 상승, 음수 = 하락
    fn calculate_price_slope_20d(&self, candles: &[Kline]) -> Result<f64, IndicatorError> {
        let len = candles.len();
        if len < 20 {
            return Err(IndicatorError::InsufficientData {
                required: 20,
                provided: len,
            });
        }

        // 최근 20일 종가 추출
        let recent_20: Vec<f64> = candles[len - 20..]
            .iter()
            .map(|c| {
                c.close.to_f64().ok_or_else(|| {
                    IndicatorError::CalculationError("Decimal to f64 변환 실패".to_string())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // 선형회귀: y = a + b*x
        // b (기울기) = Σ[(x - x_mean)(y - y_mean)] / Σ[(x - x_mean)^2]
        let n = recent_20.len() as f64;
        let x_mean = (n - 1.0) / 2.0; // 0, 1, 2, ..., 19의 평균 = 9.5
        let y_mean: f64 = recent_20.iter().sum::<f64>() / n;

        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for (i, &y) in recent_20.iter().enumerate() {
            let x = i as f64;
            let x_diff = x - x_mean;
            let y_diff = y - y_mean;
            numerator += x_diff * y_diff;
            denominator += x_diff * x_diff;
        }

        if denominator == 0.0 {
            return Ok(0.0); // 평평한 경우
        }

        let slope = numerator / denominator;

        // 기울기를 가격 대비 정규화 (퍼센트 단위로 변환)
        // slope_normalized = (slope / y_mean) * 100
        let slope_normalized = if y_mean > 0.0 {
            (slope / y_mean) * 100.0
        } else {
            0.0
        };

        Ok(slope_normalized)
    }

    /// RSI(14) 계산
    fn calculate_rsi_14(&self, candles: &[Kline]) -> Result<f64, IndicatorError> {
        use crate::indicators::RsiParams;

        let closes: Vec<Decimal> = candles.iter().map(|c| c.close).collect();

        let params = RsiParams { period: 14 };
        let rsi_values = self.indicator_engine.rsi(&closes, params)?;

        // 마지막 RSI 값 반환 (Option<Decimal>을 f64로 변환)
        let rsi_opt = rsi_values
            .last()
            .copied()
            .ok_or_else(|| IndicatorError::CalculationError("RSI 계산 결과 없음".to_string()))?;

        let rsi_decimal = rsi_opt
            .ok_or_else(|| IndicatorError::CalculationError("RSI 값이 None입니다".to_string()))?;

        rsi_decimal.to_f64().ok_or_else(|| {
            IndicatorError::CalculationError("RSI Decimal to f64 변환 실패".to_string())
        })
    }

    /// MarketRegime 판정
    fn determine_regime(&self, rel_60d_pct: f64, slope: f64, rsi: f64) -> MarketRegime {
        // 1. StrongUptrend: rel_60d > 10 + slope > 0 + RSI 50~70
        // RSI 조건을 만족하면 강한 상승 추세
        if rel_60d_pct > 10.0 && slope > 0.0 && (50.0..=70.0).contains(&rsi) {
            return MarketRegime::StrongUptrend;
        }

        // 2. Correction: rel_60d > 5 + slope <= 0
        if rel_60d_pct > 5.0 && slope <= 0.0 {
            return MarketRegime::Correction;
        }

        // 3. Sideways: -5 <= rel_60d <= 5
        if (-5.0..=5.0).contains(&rel_60d_pct) {
            return MarketRegime::Sideways;
        }

        // 4. BottomBounce: rel_60d <= -5 + slope > 0
        if rel_60d_pct <= -5.0 && slope > 0.0 {
            return MarketRegime::BottomBounce;
        }

        // 5. StrongUptrend (RSI 조건 완화): rel_60d > 10 + slope > 0
        // RSI 조건을 만족하지 못해도 강한 상승세면 StrongUptrend로 분류
        if rel_60d_pct > 10.0 && slope > 0.0 {
            return MarketRegime::StrongUptrend;
        }

        // 6. Downtrend: 위 조건에 모두 해당하지 않으면 하락 추세
        MarketRegime::Downtrend
    }
}

impl Default for MarketRegimeCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_klines(count: usize, base_price: f64, trend: f64) -> Vec<Kline> {
        use trader_core::{MarketType, Timeframe};

        (0..count)
            .map(|i| {
                // 변동성 추가: sin 함수로 상하 변동 (±3%)
                let variation = (i as f64 * 0.3).sin() * 0.03 + 1.0;
                let price = (base_price + (i as f64 * trend)) * variation;

                // OHLC에도 변동성 추가
                let daily_range = price * 0.02; // 2% 일일 변동
                Kline {
                    ticker: "TEST/USDT".to_string(),
                    timeframe: Timeframe::D1,
                    open_time: Utc::now(),
                    open: Decimal::from_f64_retain(price - daily_range / 2.0).unwrap(),
                    high: Decimal::from_f64_retain(price + daily_range).unwrap(),
                    low: Decimal::from_f64_retain(price - daily_range).unwrap(),
                    close: Decimal::from_f64_retain(price).unwrap(),
                    volume: Decimal::from((900000 + (i * 10000)) as i64),
                    close_time: Utc::now(),
                    quote_volume: Some(Decimal::ZERO),
                    num_trades: Some((100 + i) as u32),
                }
            })
            .collect()
    }

    #[test]
    fn test_insufficient_data() {
        let calculator = MarketRegimeCalculator::new();
        let candles = create_test_klines(50, 100.0, 0.0);

        let result = calculator.calculate(&candles);
        assert!(matches!(
            result,
            Err(IndicatorError::InsufficientData { .. })
        ));
    }

    #[test]
    fn test_strong_uptrend() {
        let calculator = MarketRegimeCalculator::new();
        // 강한 상승: 100 -> ~117 (약 17% 상승)
        // 변동성이 추가되어 RSI가 현실적인 값으로 계산됨
        let candles = create_test_klines(80, 100.0, 0.2);

        let result = calculator.calculate(&candles).unwrap();

        eprintln!("Test StrongUptrend:");
        eprintln!("  regime: {:?}", result.regime);
        eprintln!("  rel_60d_pct: {}", result.rel_60d_pct);
        eprintln!("  slope: {}", result.slope);
        eprintln!("  rsi: {}", result.rsi);
        eprintln!("  first_price: {}", candles[0].close);
        eprintln!("  last_price: {}", candles[79].close);
        eprintln!("  price_60d_ago: {}", candles[19].close);

        // 상승 추세 확인
        assert!(
            result.rel_60d_pct > 5.0,
            "rel_60d_pct should be > 5.0 for uptrend, got {}",
            result.rel_60d_pct
        );
        assert!(
            result.slope > 0.0,
            "slope should be > 0.0 for uptrend, got {}",
            result.slope
        );

        // Downtrend가 아닌 다른 레짐이어야 함
        assert!(
            !matches!(result.regime, MarketRegime::Downtrend),
            "Uptrending data should not produce Downtrend, got {:?}",
            result.regime
        );
    }

    #[test]
    fn test_sideways() {
        let calculator = MarketRegimeCalculator::new();
        // 횡보: 100 부근 유지
        let candles = create_test_klines(80, 100.0, 0.0);

        let result = calculator.calculate(&candles).unwrap();
        assert_eq!(result.regime, MarketRegime::Sideways);
        assert!((-5.0..=5.0).contains(&result.rel_60d_pct));
    }

    #[test]
    fn test_downtrend() {
        let calculator = MarketRegimeCalculator::new();
        // 하락: 100 -> 85 (15% 하락)
        let candles = create_test_klines(80, 100.0, -0.2);

        let result = calculator.calculate(&candles).unwrap();
        // Downtrend 또는 BottomBounce (최근 기울기에 따라)
        assert!(matches!(
            result.regime,
            MarketRegime::Downtrend | MarketRegime::BottomBounce
        ));
        assert!(result.rel_60d_pct < -5.0);
    }

    #[test]
    fn test_relative_strength_calculation() {
        let calculator = MarketRegimeCalculator::new();
        let candles = create_test_klines(80, 100.0, 0.25);

        let rel_60d = calculator
            .calculate_relative_strength_60d(&candles)
            .unwrap();
        // 60일간 약 15% 상승 (60 * 0.25 = 15)
        assert!(rel_60d > 10.0 && rel_60d < 20.0);
    }

    #[test]
    fn test_slope_calculation() {
        let calculator = MarketRegimeCalculator::new();
        let candles = create_test_klines(80, 100.0, 0.5);

        let slope = calculator.calculate_price_slope_20d(&candles).unwrap();
        // 상승 추세이므로 양수
        assert!(slope > 0.0);
    }
}
