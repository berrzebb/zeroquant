//! SuperTrend 지표.
//!
//! SuperTrend는 ATR 기반 추세 추종 지표로, 매수/매도 시그널을 명확하게 제공합니다.
//!
//! ## 계산 방식
//! 1. 기본 밴드 = (고가 + 저가) / 2 ± (배수 × ATR)
//! 2. 상단/하단 밴드 계산
//! 3. 추세 방향 결정
//!
//! ## 시그널
//! - SuperTrend < 가격: 상승 추세 (매수)
//! - SuperTrend > 가격: 하락 추세 (매도)
//! - 추세 전환: 명확한 진입/청산 신호

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use super::{IndicatorError, IndicatorResult};

/// SuperTrend 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SuperTrendParams {
    /// ATR 기간 (기본: 10).
    pub atr_period: usize,
    /// ATR 배수 (기본: 3.0).
    pub multiplier: Decimal,
}

impl Default for SuperTrendParams {
    fn default() -> Self {
        Self {
            atr_period: 10,
            multiplier: dec!(3.0),
        }
    }
}

/// SuperTrend 결과.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SuperTrendResult {
    /// SuperTrend 값.
    pub value: Option<Decimal>,
    /// 추세 방향 (true: 상승, false: 하락).
    pub is_uptrend: bool,
    /// 매수 시그널 (추세 전환: 하락 -> 상승).
    pub buy_signal: bool,
    /// 매도 시그널 (추세 전환: 상승 -> 하락).
    pub sell_signal: bool,
}

/// SuperTrend 계산기.
#[derive(Debug, Default)]
pub struct SuperTrendIndicator;

impl SuperTrendIndicator {
    /// 새로운 SuperTrend 계산기 생성.
    pub fn new() -> Self {
        Self
    }

    /// SuperTrend 지표 계산.
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - SuperTrend 파라미터
    ///
    /// # 반환
    /// 각 시점의 SuperTrend 값과 시그널
    pub fn calculate(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: SuperTrendParams,
    ) -> IndicatorResult<Vec<SuperTrendResult>> {
        if high.len() != low.len() || high.len() != close.len() {
            return Err(IndicatorError::InvalidParameter(
                "고가, 저가, 종가 데이터의 길이가 일치하지 않습니다".to_string(),
            ));
        }

        if high.len() < params.atr_period {
            return Err(IndicatorError::InsufficientData {
                required: params.atr_period,
                provided: high.len(),
            });
        }

        if params.multiplier <= Decimal::ZERO {
            return Err(IndicatorError::InvalidParameter(
                "배수는 0보다 커야 합니다".to_string(),
            ));
        }

        // ATR 계산
        let atr = self.calculate_atr(high, low, close, params.atr_period)?;

        let mut result = Vec::with_capacity(high.len());
        let mut prev_upper_band = Decimal::ZERO;
        let mut prev_lower_band = Decimal::ZERO;
        let mut prev_supertrend = Decimal::ZERO;
        let mut prev_is_uptrend = true;

        for i in 0..high.len() {
            if atr[i].is_none() {
                result.push(SuperTrendResult {
                    value: None,
                    is_uptrend: true,
                    buy_signal: false,
                    sell_signal: false,
                });
                continue;
            }

            let atr_val = atr[i].unwrap();
            let hl_avg = (high[i] + low[i]) / dec!(2);

            // 기본 밴드 계산
            let basic_upper = hl_avg + params.multiplier * atr_val;
            let basic_lower = hl_avg - params.multiplier * atr_val;

            // 최종 밴드 계산 (이전 종가 기준 조정)
            let final_upper =
                if i == 0 || basic_upper < prev_upper_band || close[i - 1] > prev_upper_band {
                    basic_upper
                } else {
                    prev_upper_band
                };

            let final_lower =
                if i == 0 || basic_lower > prev_lower_band || close[i - 1] < prev_lower_band {
                    basic_lower
                } else {
                    prev_lower_band
                };

            // 추세 방향 결정
            let is_uptrend = if i == 0 {
                close[i] > hl_avg
            } else if prev_supertrend == prev_upper_band {
                close[i] <= final_upper
            } else {
                close[i] >= final_lower
            };

            // SuperTrend 값
            let supertrend = if is_uptrend { final_lower } else { final_upper };

            // 시그널 감지
            let buy_signal = i > 0 && is_uptrend && !prev_is_uptrend;
            let sell_signal = i > 0 && !is_uptrend && prev_is_uptrend;

            result.push(SuperTrendResult {
                value: Some(supertrend),
                is_uptrend,
                buy_signal,
                sell_signal,
            });

            // 다음 반복을 위한 상태 저장
            prev_upper_band = final_upper;
            prev_lower_band = final_lower;
            prev_supertrend = supertrend;
            prev_is_uptrend = is_uptrend;
        }

        Ok(result)
    }

    /// ATR (Average True Range) 계산.
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `period` - 기간
    ///
    /// # 반환
    /// 각 시점의 ATR 값
    fn calculate_atr(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        period: usize,
    ) -> IndicatorResult<Vec<Option<Decimal>>> {
        let mut tr_values = Vec::with_capacity(high.len());

        // True Range 계산
        for i in 0..high.len() {
            let tr = if i == 0 {
                high[i] - low[i]
            } else {
                let hl = high[i] - low[i];
                let hc = (high[i] - close[i - 1]).abs();
                let lc = (low[i] - close[i - 1]).abs();
                hl.max(hc).max(lc)
            };
            tr_values.push(tr);
        }

        // ATR 계산 (EMA 방식)
        let mut atr_values = Vec::with_capacity(high.len());
        let multiplier = dec!(2) / Decimal::from(period + 1);

        for i in 0..tr_values.len() {
            if i < period - 1 {
                atr_values.push(None);
            } else if i == period - 1 {
                // 첫 ATR은 단순 평균
                let sum: Decimal = tr_values[0..period].iter().sum();
                atr_values.push(Some(sum / Decimal::from(period)));
            } else {
                // 이후는 EMA 방식
                let prev_atr = atr_values[i - 1].unwrap();
                let current_tr = tr_values[i];
                let new_atr = (current_tr - prev_atr) * multiplier + prev_atr;
                atr_values.push(Some(new_atr));
            }
        }

        Ok(atr_values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_data() -> (Vec<Decimal>, Vec<Decimal>, Vec<Decimal>) {
        let high = vec![
            dec!(102.0),
            dec!(104.0),
            dec!(103.0),
            dec!(106.0),
            dec!(108.0),
            dec!(107.0),
            dec!(110.0),
            dec!(112.0),
            dec!(111.0),
            dec!(114.0),
            dec!(116.0),
            dec!(115.0),
        ];

        let low = vec![
            dec!(98.0),
            dec!(100.0),
            dec!(99.0),
            dec!(102.0),
            dec!(104.0),
            dec!(103.0),
            dec!(106.0),
            dec!(108.0),
            dec!(107.0),
            dec!(110.0),
            dec!(112.0),
            dec!(111.0),
        ];

        let close = vec![
            dec!(100.0),
            dec!(102.0),
            dec!(101.0),
            dec!(104.0),
            dec!(106.0),
            dec!(105.0),
            dec!(108.0),
            dec!(110.0),
            dec!(109.0),
            dec!(112.0),
            dec!(114.0),
            dec!(113.0),
        ];

        (high, low, close)
    }

    #[test]
    fn test_supertrend_calculation() {
        let indicator = SuperTrendIndicator::new();
        let (high, low, close) = sample_data();

        let result = indicator
            .calculate(&high, &low, &close, SuperTrendParams::default())
            .unwrap();

        assert_eq!(result.len(), high.len());

        // 처음 몇 개는 ATR 계산 대기로 None
        assert!(result[0].value.is_none());

        // 충분한 데이터가 있으면 값이 계산됨
        assert!(result[result.len() - 1].value.is_some());
    }

    #[test]
    fn test_supertrend_trend_detection() {
        let indicator = SuperTrendIndicator::new();
        // 명확한 상승 추세
        let high = vec![
            dec!(101.0),
            dec!(102.0),
            dec!(103.0),
            dec!(104.0),
            dec!(105.0),
            dec!(106.0),
            dec!(107.0),
            dec!(108.0),
            dec!(109.0),
            dec!(110.0),
            dec!(111.0),
            dec!(112.0),
        ];

        let low = vec![
            dec!(99.0),
            dec!(100.0),
            dec!(101.0),
            dec!(102.0),
            dec!(103.0),
            dec!(104.0),
            dec!(105.0),
            dec!(106.0),
            dec!(107.0),
            dec!(108.0),
            dec!(109.0),
            dec!(110.0),
        ];

        let close = vec![
            dec!(100.0),
            dec!(101.0),
            dec!(102.0),
            dec!(103.0),
            dec!(104.0),
            dec!(105.0),
            dec!(106.0),
            dec!(107.0),
            dec!(108.0),
            dec!(109.0),
            dec!(110.0),
            dec!(111.0),
        ];

        let result = indicator
            .calculate(&high, &low, &close, SuperTrendParams::default())
            .unwrap();

        // 대부분 상승 추세여야 함
        let uptrend_count = result.iter().filter(|r| r.is_uptrend).count();
        assert!(uptrend_count > result.len() / 2);
    }

    #[test]
    fn test_supertrend_signals() {
        let indicator = SuperTrendIndicator::new();
        let (high, low, close) = sample_data();

        let result = indicator
            .calculate(&high, &low, &close, SuperTrendParams::default())
            .unwrap();

        // 시그널은 추세 전환 시에만 발생
        let _buy_signals: Vec<_> = result.iter().filter(|r| r.buy_signal).collect();
        let _sell_signals: Vec<_> = result.iter().filter(|r| r.sell_signal).collect();

        // 매수와 매도 시그널은 동시에 발생하지 않음
        for r in &result {
            assert!(!(r.buy_signal && r.sell_signal));
        }
    }

    #[test]
    fn test_atr_calculation() {
        let indicator = SuperTrendIndicator::new();
        let (high, low, close) = sample_data();

        let atr = indicator.calculate_atr(&high, &low, &close, 10).unwrap();

        assert_eq!(atr.len(), high.len());

        // 처음 9개는 None
        for val in &atr[0..9] {
            assert!(val.is_none());
        }

        // 10번째부터 값이 있어야 함
        assert!(atr[9].is_some());

        // ATR은 양수여야 함
        for value in atr.iter().flatten() {
            assert!(*value >= Decimal::ZERO);
        }
    }

    #[test]
    fn test_mismatched_length_error() {
        let indicator = SuperTrendIndicator::new();
        let high = vec![dec!(100.0), dec!(101.0)];
        let low = vec![dec!(99.0)];
        let close = vec![dec!(100.0), dec!(101.0)];

        let result = indicator.calculate(&high, &low, &close, SuperTrendParams::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_insufficient_data_error() {
        let indicator = SuperTrendIndicator::new();
        let high = vec![dec!(100.0)];
        let low = vec![dec!(99.0)];
        let close = vec![dec!(100.0)];

        let result = indicator.calculate(&high, &low, &close, SuperTrendParams::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_multiplier_error() {
        let indicator = SuperTrendIndicator::new();
        let (high, low, close) = sample_data();

        let result = indicator.calculate(
            &high,
            &low,
            &close,
            SuperTrendParams {
                atr_period: 10,
                multiplier: dec!(-1.0),
            },
        );
        assert!(result.is_err());
    }
}
