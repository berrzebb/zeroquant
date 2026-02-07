//! 추세 지표 (Trend Indicators).
//!
//! 이동평균 기반의 추세 지표들을 제공합니다.
//! - SMA (Simple Moving Average)
//! - EMA (Exponential Moving Average)
//! - MACD (Moving Average Convergence Divergence)

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use super::{IndicatorError, IndicatorResult};

/// SMA 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SmaParams {
    /// 이동평균 기간.
    pub period: usize,
}

impl Default for SmaParams {
    fn default() -> Self {
        Self { period: 20 }
    }
}

/// EMA 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EmaParams {
    /// 이동평균 기간.
    pub period: usize,
}

impl Default for EmaParams {
    fn default() -> Self {
        Self { period: 12 }
    }
}

/// MACD 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MacdParams {
    /// 단기 EMA 기간 (기본: 12).
    pub fast_period: usize,
    /// 장기 EMA 기간 (기본: 26).
    pub slow_period: usize,
    /// 시그널 라인 기간 (기본: 9).
    pub signal_period: usize,
}

impl Default for MacdParams {
    fn default() -> Self {
        Self {
            fast_period: 12,
            slow_period: 26,
            signal_period: 9,
        }
    }
}

/// MACD 결과.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MacdResult {
    /// MACD 라인 (단기 EMA - 장기 EMA).
    pub macd: Option<Decimal>,
    /// 시그널 라인 (MACD의 EMA).
    pub signal: Option<Decimal>,
    /// 히스토그램 (MACD - 시그널).
    pub histogram: Option<Decimal>,
}

/// 추세 지표 계산기.
#[derive(Debug, Default)]
pub struct TrendIndicators;

impl TrendIndicators {
    /// 새로운 추세 지표 계산기 생성.
    pub fn new() -> Self {
        Self
    }

    /// 단순 이동평균 (SMA) 계산.
    ///
    /// SMA = (P1 + P2 + ... + Pn) / n
    ///
    /// # 인자
    /// * `prices` - 가격 데이터
    /// * `params` - SMA 파라미터
    ///
    /// # 반환
    /// 각 시점의 SMA 값 (처음 period-1개는 None)
    pub fn sma(
        &self,
        prices: &[Decimal],
        params: SmaParams,
    ) -> IndicatorResult<Vec<Option<Decimal>>> {
        let period = params.period;

        if prices.len() < period {
            return Err(IndicatorError::InsufficientData {
                required: period,
                provided: prices.len(),
            });
        }

        if period == 0 {
            return Err(IndicatorError::InvalidParameter(
                "기간은 0보다 커야 합니다".to_string(),
            ));
        }

        let mut result = Vec::with_capacity(prices.len());
        let period_decimal = Decimal::from(period);

        for i in 0..prices.len() {
            if i < period - 1 {
                result.push(None);
            } else {
                let sum: Decimal = prices[i + 1 - period..=i].iter().sum();
                result.push(Some(sum / period_decimal));
            }
        }

        Ok(result)
    }

    /// 지수 이동평균 (EMA) 계산.
    ///
    /// EMA = (현재가 × k) + (이전 EMA × (1 - k))
    /// k = 2 / (period + 1)
    ///
    /// # 인자
    /// * `prices` - 가격 데이터
    /// * `params` - EMA 파라미터
    ///
    /// # 반환
    /// 각 시점의 EMA 값
    pub fn ema(
        &self,
        prices: &[Decimal],
        params: EmaParams,
    ) -> IndicatorResult<Vec<Option<Decimal>>> {
        let period = params.period;

        if prices.len() < period {
            return Err(IndicatorError::InsufficientData {
                required: period,
                provided: prices.len(),
            });
        }

        if period == 0 {
            return Err(IndicatorError::InvalidParameter(
                "기간은 0보다 커야 합니다".to_string(),
            ));
        }

        let mut result = Vec::with_capacity(prices.len());
        let multiplier = dec!(2) / Decimal::from(period + 1);

        // 처음 period-1개는 None
        for _ in 0..period - 1 {
            result.push(None);
        }

        // 첫 EMA는 SMA로 시작
        let initial_sma: Decimal = prices[..period].iter().sum::<Decimal>() / Decimal::from(period);
        result.push(Some(initial_sma));

        // 이후 EMA 계산
        let mut prev_ema = initial_sma;
        for price in prices.iter().skip(period) {
            let ema = (*price * multiplier) + (prev_ema * (Decimal::ONE - multiplier));
            result.push(Some(ema));
            prev_ema = ema;
        }

        Ok(result)
    }

    /// MACD 계산.
    ///
    /// MACD 라인 = 단기 EMA - 장기 EMA
    /// 시그널 라인 = MACD 라인의 EMA
    /// 히스토그램 = MACD 라인 - 시그널 라인
    ///
    /// # 인자
    /// * `prices` - 가격 데이터
    /// * `params` - MACD 파라미터
    ///
    /// # 반환
    /// 각 시점의 MACD, 시그널, 히스토그램 값
    pub fn macd(&self, prices: &[Decimal], params: MacdParams) -> IndicatorResult<Vec<MacdResult>> {
        let min_required = params.slow_period + params.signal_period;

        if prices.len() < min_required {
            return Err(IndicatorError::InsufficientData {
                required: min_required,
                provided: prices.len(),
            });
        }

        // 단기, 장기 EMA 계산
        let fast_ema = self.ema(
            prices,
            EmaParams {
                period: params.fast_period,
            },
        )?;
        let slow_ema = self.ema(
            prices,
            EmaParams {
                period: params.slow_period,
            },
        )?;

        // MACD 라인 계산
        let mut macd_line: Vec<Option<Decimal>> = Vec::with_capacity(prices.len());
        for i in 0..prices.len() {
            match (fast_ema[i], slow_ema[i]) {
                (Some(fast), Some(slow)) => macd_line.push(Some(fast - slow)),
                _ => macd_line.push(None),
            }
        }

        // 시그널 라인 계산 (MACD 라인의 EMA)
        let macd_values: Vec<Decimal> = macd_line.iter().flatten().copied().collect();
        let signal_ema = if macd_values.len() >= params.signal_period {
            self.ema(
                &macd_values,
                EmaParams {
                    period: params.signal_period,
                },
            )?
        } else {
            vec![None; macd_values.len()]
        };

        // 결과 조합
        let mut result = Vec::with_capacity(prices.len());
        let mut signal_idx = 0;

        for macd_val in macd_line.iter() {
            if macd_val.is_some() {
                let signal = signal_ema.get(signal_idx).copied().flatten();
                let histogram = match (*macd_val, signal) {
                    (Some(m), Some(s)) => Some(m - s),
                    _ => None,
                };

                result.push(MacdResult {
                    macd: *macd_val,
                    signal,
                    histogram,
                });
                signal_idx += 1;
            } else {
                result.push(MacdResult {
                    macd: None,
                    signal: None,
                    histogram: None,
                });
            }
        }

        Ok(result)
    }

    /// 골든 크로스 감지.
    ///
    /// 단기 이동평균이 장기 이동평균을 상향 돌파하는 시점.
    /// 이전: 단기 < 장기, 현재: 단기 > 장기
    ///
    /// # 인자
    /// * `short_ma` - 단기 이동평균 값들
    /// * `long_ma` - 장기 이동평균 값들
    ///
    /// # 반환
    /// 각 시점에서 골든 크로스 발생 여부
    #[allow(clippy::needless_range_loop)]
    pub fn detect_golden_cross(
        &self,
        short_ma: &[Option<Decimal>],
        long_ma: &[Option<Decimal>],
    ) -> Vec<bool> {
        let mut result = vec![false; short_ma.len()];

        for i in 1..short_ma.len().min(long_ma.len()) {
            if let (Some(prev_short), Some(prev_long), Some(curr_short), Some(curr_long)) = (
                short_ma.get(i - 1).and_then(|v| *v),
                long_ma.get(i - 1).and_then(|v| *v),
                short_ma.get(i).and_then(|v| *v),
                long_ma.get(i).and_then(|v| *v),
            ) {
                // 이전: 단기 < 장기, 현재: 단기 > 장기
                result[i] = prev_short < prev_long && curr_short > curr_long;
            }
        }

        result
    }

    /// 데드 크로스 감지.
    ///
    /// 단기 이동평균이 장기 이동평균을 하향 돌파하는 시점.
    /// 이전: 단기 > 장기, 현재: 단기 < 장기
    ///
    /// # 인자
    /// * `short_ma` - 단기 이동평균 값들
    /// * `long_ma` - 장기 이동평균 값들
    ///
    /// # 반환
    /// 각 시점에서 데드 크로스 발생 여부
    #[allow(clippy::needless_range_loop)]
    pub fn detect_dead_cross(
        &self,
        short_ma: &[Option<Decimal>],
        long_ma: &[Option<Decimal>],
    ) -> Vec<bool> {
        let mut result = vec![false; short_ma.len()];

        for i in 1..short_ma.len().min(long_ma.len()) {
            if let (Some(prev_short), Some(prev_long), Some(curr_short), Some(curr_long)) = (
                short_ma.get(i - 1).and_then(|v| *v),
                long_ma.get(i - 1).and_then(|v| *v),
                short_ma.get(i).and_then(|v| *v),
                long_ma.get(i).and_then(|v| *v),
            ) {
                // 이전: 단기 > 장기, 현재: 단기 < 장기
                result[i] = prev_short > prev_long && curr_short < curr_long;
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_prices() -> Vec<Decimal> {
        vec![
            dec!(100.0),
            dec!(102.0),
            dec!(101.0),
            dec!(103.0),
            dec!(105.0),
            dec!(104.0),
            dec!(106.0),
            dec!(108.0),
            dec!(107.0),
            dec!(109.0),
        ]
    }

    #[test]
    fn test_sma_basic() {
        let trend = TrendIndicators::new();
        let prices = sample_prices();

        let sma = trend.sma(&prices, SmaParams { period: 3 }).unwrap();

        // 처음 2개는 None
        assert!(sma[0].is_none());
        assert!(sma[1].is_none());

        // 3번째 값: (100 + 102 + 101) / 3 = 101
        assert_eq!(sma[2], Some(dec!(101)));
    }

    #[test]
    fn test_ema_basic() {
        let trend = TrendIndicators::new();
        let prices = sample_prices();

        let ema = trend.ema(&prices, EmaParams { period: 3 }).unwrap();

        // 처음 2개는 None
        assert!(ema[0].is_none());
        assert!(ema[1].is_none());

        // 3번째 값: SMA와 같음
        assert!(ema[2].is_some());
    }

    #[test]
    fn test_macd_basic() {
        let trend = TrendIndicators::new();
        // MACD는 더 많은 데이터가 필요
        let prices: Vec<Decimal> = (0..50).map(|i| Decimal::from(100 + i)).collect();

        let macd = trend.macd(&prices, MacdParams::default()).unwrap();

        assert_eq!(macd.len(), prices.len());

        // 처음 몇 개는 None
        assert!(macd[0].macd.is_none());

        // 나중 값은 Some
        assert!(macd[40].macd.is_some());
    }

    #[test]
    fn test_golden_cross_detection() {
        let trend = TrendIndicators::new();

        let short_ma = vec![
            Some(dec!(95)),
            Some(dec!(98)),
            Some(dec!(101)), // 골든 크로스!
            Some(dec!(103)),
        ];
        let long_ma = vec![
            Some(dec!(100)),
            Some(dec!(100)),
            Some(dec!(100)),
            Some(dec!(100)),
        ];

        let crosses = trend.detect_golden_cross(&short_ma, &long_ma);

        assert!(!crosses[0]);
        assert!(!crosses[1]);
        assert!(crosses[2]); // 골든 크로스
        assert!(!crosses[3]);
    }

    #[test]
    fn test_dead_cross_detection() {
        let trend = TrendIndicators::new();

        let short_ma = vec![
            Some(dec!(105)),
            Some(dec!(102)),
            Some(dec!(99)), // 데드 크로스!
            Some(dec!(97)),
        ];
        let long_ma = vec![
            Some(dec!(100)),
            Some(dec!(100)),
            Some(dec!(100)),
            Some(dec!(100)),
        ];

        let crosses = trend.detect_dead_cross(&short_ma, &long_ma);

        assert!(!crosses[0]);
        assert!(!crosses[1]);
        assert!(crosses[2]); // 데드 크로스
        assert!(!crosses[3]);
    }
}
