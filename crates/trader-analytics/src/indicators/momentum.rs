//! 모멘텀 지표 (Momentum Indicators).
//!
//! 가격 모멘텀과 과매수/과매도 상태를 측정하는 지표들을 제공합니다.
//! - RSI (Relative Strength Index)
//! - Stochastic Oscillator
//! - 다기간 모멘텀 점수

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use super::{IndicatorError, IndicatorResult};

/// RSI 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RsiParams {
    /// RSI 기간 (기본: 14).
    pub period: usize,
}

impl Default for RsiParams {
    fn default() -> Self {
        Self { period: 14 }
    }
}

/// 스토캐스틱 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StochasticParams {
    /// %K 기간 (기본: 14).
    pub k_period: usize,
    /// %D 기간 (smoothing, 기본: 3).
    pub d_period: usize,
}

impl Default for StochasticParams {
    fn default() -> Self {
        Self {
            k_period: 14,
            d_period: 3,
        }
    }
}

/// 스토캐스틱 결과.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StochasticResult {
    /// %K (Fast Stochastic).
    pub k: Option<Decimal>,
    /// %D (Slow Stochastic, %K의 이동평균).
    pub d: Option<Decimal>,
}

/// 모멘텀 지표 계산기.
#[derive(Debug, Default)]
pub struct MomentumCalculator;

impl MomentumCalculator {
    /// 새로운 모멘텀 계산기 생성.
    pub fn new() -> Self {
        Self
    }

    /// RSI (Relative Strength Index) 계산.
    ///
    /// RSI = 100 - (100 / (1 + RS))
    /// RS = 평균 상승폭 / 평균 하락폭
    ///
    /// EWM (지수 가중 이동평균) 방식을 사용하여 표준 RSI 계산.
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `params` - RSI 파라미터
    ///
    /// # 반환
    /// 0-100 사이의 RSI 값들
    pub fn rsi(
        &self,
        prices: &[Decimal],
        params: RsiParams,
    ) -> IndicatorResult<Vec<Option<Decimal>>> {
        let period = params.period;

        if prices.len() < period + 1 {
            return Err(IndicatorError::InsufficientData {
                required: period + 1,
                provided: prices.len(),
            });
        }

        if period == 0 {
            return Err(IndicatorError::InvalidParameter(
                "기간은 0보다 커야 합니다".to_string(),
            ));
        }

        // 가격 변화 계산
        let mut deltas = Vec::with_capacity(prices.len());
        deltas.push(Decimal::ZERO); // 첫 번째는 변화 없음
        for i in 1..prices.len() {
            deltas.push(prices[i] - prices[i - 1]);
        }

        // 상승/하락 분리
        let gains: Vec<Decimal> = deltas
            .iter()
            .map(|&d| if d > Decimal::ZERO { d } else { Decimal::ZERO })
            .collect();
        let losses: Vec<Decimal> = deltas
            .iter()
            .map(|&d| {
                if d < Decimal::ZERO {
                    d.abs()
                } else {
                    Decimal::ZERO
                }
            })
            .collect();

        // EWM (Exponential Weighted Mean) 계산
        // com = (period - 1) 이면 alpha = 1 / period
        let alpha = Decimal::ONE / Decimal::from(period);
        let one_minus_alpha = Decimal::ONE - alpha;

        let avg_gains = self.ewm(&gains, alpha, one_minus_alpha, period);
        let avg_losses = self.ewm(&losses, alpha, one_minus_alpha, period);

        // RSI 계산
        let mut result = Vec::with_capacity(prices.len());
        for i in 0..prices.len() {
            match (avg_gains[i], avg_losses[i]) {
                (Some(gain), Some(loss)) => {
                    if loss == Decimal::ZERO {
                        result.push(Some(dec!(100)));
                    } else {
                        let rs = gain / loss;
                        let rsi = dec!(100) - (dec!(100) / (Decimal::ONE + rs));
                        result.push(Some(rsi));
                    }
                }
                _ => result.push(None),
            }
        }

        Ok(result)
    }

    /// EWM (Exponential Weighted Mean) 계산.
    fn ewm(
        &self,
        values: &[Decimal],
        alpha: Decimal,
        one_minus_alpha: Decimal,
        min_periods: usize,
    ) -> Vec<Option<Decimal>> {
        let mut result = Vec::with_capacity(values.len());

        if values.is_empty() {
            return result;
        }

        let mut ewm_value = values[0];

        for i in 0..values.len() {
            if i < min_periods - 1 {
                result.push(None);
                if i > 0 {
                    ewm_value = (values[i] * alpha) + (ewm_value * one_minus_alpha);
                }
            } else if i == min_periods - 1 {
                // 초기 EWM은 단순 평균으로 시작
                let sum: Decimal = values[..=i].iter().sum();
                ewm_value = sum / Decimal::from(i + 1);
                result.push(Some(ewm_value));
            } else {
                ewm_value = (values[i] * alpha) + (ewm_value * one_minus_alpha);
                result.push(Some(ewm_value));
            }
        }

        result
    }

    /// 스토캐스틱 오실레이터 계산.
    ///
    /// %K = (현재가 - 최저가) / (최고가 - 최저가) × 100
    /// %D = %K의 이동평균
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - 스토캐스틱 파라미터
    ///
    /// # 반환
    /// %K, %D 값들
    pub fn stochastic(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: StochasticParams,
    ) -> IndicatorResult<Vec<StochasticResult>> {
        let len = high.len().min(low.len()).min(close.len());

        if len < params.k_period {
            return Err(IndicatorError::InsufficientData {
                required: params.k_period,
                provided: len,
            });
        }

        let mut result = Vec::with_capacity(len);
        let mut k_values: Vec<Option<Decimal>> = Vec::with_capacity(len);

        // %K 계산
        for i in 0..len {
            if i < params.k_period - 1 {
                k_values.push(None);
            } else {
                let start = i + 1 - params.k_period;
                let highest = high[start..=i]
                    .iter()
                    .max()
                    .copied()
                    .unwrap_or(Decimal::ZERO);
                let lowest = low[start..=i]
                    .iter()
                    .min()
                    .copied()
                    .unwrap_or(Decimal::ZERO);

                let range = highest - lowest;
                if range == Decimal::ZERO {
                    k_values.push(Some(dec!(50))); // 범위가 0이면 중립값
                } else {
                    let k = ((close[i] - lowest) / range) * dec!(100);
                    k_values.push(Some(k));
                }
            }
        }

        // %D 계산 (%K의 이동평균)
        for i in 0..len {
            if i < params.k_period + params.d_period - 2 {
                result.push(StochasticResult {
                    k: k_values[i],
                    d: None,
                });
            } else {
                let start = i + 1 - params.d_period;
                let sum: Decimal = k_values[start..=i].iter().filter_map(|v| *v).sum();
                let count = k_values[start..=i].iter().filter(|v| v.is_some()).count();

                let d = if count > 0 {
                    Some(sum / Decimal::from(count))
                } else {
                    None
                };

                result.push(StochasticResult { k: k_values[i], d });
            }
        }

        Ok(result)
    }

    /// 다기간 모멘텀 점수 계산.
    ///
    /// 모멘텀 = Σ((현재가 - N일전 가격) / N일전 가격) / 기간 수
    ///
    /// 예: lookback_periods = [20, 60, 120, 240] (1, 3, 6, 12개월)
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `lookback_periods` - 참조 기간들 (일 단위)
    ///
    /// # 반환
    /// 현재 가격 기준 모멘텀 점수
    pub fn momentum_score(
        &self,
        prices: &[Decimal],
        lookback_periods: &[usize],
    ) -> IndicatorResult<Decimal> {
        if prices.is_empty() {
            return Err(IndicatorError::InsufficientData {
                required: 1,
                provided: 0,
            });
        }

        let max_period = lookback_periods.iter().max().copied().unwrap_or(0);
        if prices.len() < max_period + 1 {
            return Err(IndicatorError::InsufficientData {
                required: max_period + 1,
                provided: prices.len(),
            });
        }

        if lookback_periods.is_empty() {
            return Err(IndicatorError::InvalidParameter(
                "lookback_periods가 비어있습니다".to_string(),
            ));
        }

        let current_price = prices[prices.len() - 1];
        let mut momentum_sum = Decimal::ZERO;
        let mut valid_count = 0;

        for &period in lookback_periods {
            if period < prices.len() {
                let past_price = prices[prices.len() - 1 - period];
                if past_price != Decimal::ZERO {
                    let momentum = (current_price - past_price) / past_price;
                    momentum_sum += momentum;
                    valid_count += 1;
                }
            }
        }

        if valid_count == 0 {
            return Err(IndicatorError::CalculationError(
                "유효한 모멘텀 값을 계산할 수 없습니다".to_string(),
            ));
        }

        Ok(momentum_sum / Decimal::from(valid_count))
    }

    /// 모든 시점에서의 모멘텀 점수 계산.
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `lookback_periods` - 참조 기간들 (일 단위)
    ///
    /// # 반환
    /// 각 시점의 모멘텀 점수
    pub fn momentum_scores(
        &self,
        prices: &[Decimal],
        lookback_periods: &[usize],
    ) -> IndicatorResult<Vec<Option<Decimal>>> {
        let max_period = lookback_periods.iter().max().copied().unwrap_or(0);

        if prices.len() < max_period + 1 {
            return Err(IndicatorError::InsufficientData {
                required: max_period + 1,
                provided: prices.len(),
            });
        }

        let mut result = Vec::with_capacity(prices.len());

        for i in 0..prices.len() {
            if i < max_period {
                result.push(None);
            } else {
                let current_price = prices[i];
                let mut momentum_sum = Decimal::ZERO;
                let mut valid_count = 0;

                for &period in lookback_periods {
                    if period <= i {
                        let past_price = prices[i - period];
                        if past_price != Decimal::ZERO {
                            let momentum = (current_price - past_price) / past_price;
                            momentum_sum += momentum;
                            valid_count += 1;
                        }
                    }
                }

                if valid_count > 0 {
                    result.push(Some(momentum_sum / Decimal::from(valid_count)));
                } else {
                    result.push(None);
                }
            }
        }

        Ok(result)
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
            dec!(111.0),
            dec!(110.0),
            dec!(112.0),
            dec!(114.0),
            dec!(113.0),
            dec!(115.0),
        ]
    }

    #[test]
    fn test_rsi_calculation() {
        let momentum = MomentumCalculator::new();
        let prices = sample_prices();

        let rsi = momentum.rsi(&prices, RsiParams { period: 14 }).unwrap();

        // RSI 값이 0-100 범위
        for value in rsi.iter().flatten() {
            assert!(*value >= Decimal::ZERO);
            assert!(*value <= dec!(100));
        }
    }

    #[test]
    fn test_rsi_bullish_market() {
        let momentum = MomentumCalculator::new();

        // 계속 상승하는 시장
        let prices: Vec<Decimal> = (0..20).map(|i| Decimal::from(100 + i)).collect();

        let rsi = momentum.rsi(&prices, RsiParams { period: 14 }).unwrap();

        // 상승 시장에서 RSI는 높아야 함 (50 이상)
        if let Some(last_rsi) = rsi.last().unwrap() {
            assert!(*last_rsi > dec!(50));
        }
    }

    #[test]
    fn test_stochastic_calculation() {
        let momentum = MomentumCalculator::new();

        let high: Vec<Decimal> = (0..20).map(|i| Decimal::from(105 + i)).collect();
        let low: Vec<Decimal> = (0..20).map(|i| Decimal::from(95 + i)).collect();
        let close: Vec<Decimal> = (0..20).map(|i| Decimal::from(100 + i)).collect();

        let stoch = momentum
            .stochastic(&high, &low, &close, StochasticParams::default())
            .unwrap();

        // 결과 길이 확인
        assert_eq!(stoch.len(), 20);

        // %K, %D 값이 0-100 범위
        for s in stoch.iter() {
            if let Some(k) = s.k {
                assert!(k >= Decimal::ZERO && k <= dec!(100));
            }
            if let Some(d) = s.d {
                assert!(d >= Decimal::ZERO && d <= dec!(100));
            }
        }
    }

    #[test]
    fn test_momentum_score() {
        let momentum = MomentumCalculator::new();
        let prices = sample_prices();

        // 1, 3, 5일 모멘텀 평균
        let score = momentum.momentum_score(&prices, &[1, 3, 5]).unwrap();

        // 상승 추세이므로 모멘텀은 양수
        assert!(score > Decimal::ZERO);
    }

    #[test]
    fn test_momentum_scores_over_time() {
        let momentum = MomentumCalculator::new();
        let prices = sample_prices();

        let scores = momentum.momentum_scores(&prices, &[1, 3, 5]).unwrap();

        assert_eq!(scores.len(), prices.len());

        // 처음 5개는 None
        assert!(scores[0].is_none());
        assert!(scores[4].is_none());

        // 6번째부터 값이 있어야 함
        assert!(scores[5].is_some());
    }
}
