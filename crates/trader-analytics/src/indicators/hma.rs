//! HMA (Hull Moving Average) 지표.
//!
//! HMA는 Alan Hull이 개발한 이동평균으로, 빠른 반응속도와 낮은 휩소(lag)를 특징으로 합니다.
//!
//! ## 계산 방식
//! 1. WMA(n/2) * 2 - WMA(n) 계산
//! 2. 결과에 대해 WMA(sqrt(n)) 적용
//!
//! ## 특징
//! - 기존 이동평균보다 빠른 반응
//! - 부드러운 곡선
//! - 추세 전환 조기 감지

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use super::{IndicatorError, IndicatorResult};

/// HMA 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HmaParams {
    /// 이동평균 기간 (기본: 9).
    pub period: usize,
}

impl Default for HmaParams {
    fn default() -> Self {
        Self { period: 9 }
    }
}

/// HMA 계산기.
#[derive(Debug, Default)]
pub struct HmaIndicator;

impl HmaIndicator {
    /// 새로운 HMA 계산기 생성.
    pub fn new() -> Self {
        Self
    }

    /// HMA (Hull Moving Average) 계산.
    ///
    /// HMA = WMA(2 * WMA(n/2) - WMA(n), sqrt(n))
    ///
    /// # 인자
    /// * `prices` - 가격 데이터
    /// * `params` - HMA 파라미터
    ///
    /// # 반환
    /// 각 시점의 HMA 값
    pub fn calculate(
        &self,
        prices: &[Decimal],
        params: HmaParams,
    ) -> IndicatorResult<Vec<Option<Decimal>>> {
        let period = params.period;

        if period < 2 {
            return Err(IndicatorError::InvalidParameter(
                "HMA 기간은 2 이상이어야 합니다".to_string(),
            ));
        }

        if prices.len() < period {
            return Err(IndicatorError::InsufficientData {
                required: period,
                provided: prices.len(),
            });
        }

        let half_period = period / 2;
        let sqrt_period = (period as f64).sqrt().floor() as usize;

        // 1. WMA(n/2) 계산
        let wma_half = self.wma(prices, half_period)?;

        // 2. WMA(n) 계산
        let wma_full = self.wma(prices, period)?;

        // 3. 2 * WMA(n/2) - WMA(n) 계산
        let mut raw_hma = Vec::with_capacity(prices.len());
        for i in 0..prices.len() {
            if let (Some(half), Some(full)) = (wma_half[i], wma_full[i]) {
                raw_hma.push(half * dec!(2) - full);
            } else {
                raw_hma.push(Decimal::ZERO); // placeholder
            }
        }

        // 4. WMA(sqrt(n)) 적용
        let mut result = Vec::with_capacity(prices.len());
        for i in 0..prices.len() {
            // period - 1 + sqrt_period - 1 보다 작으면 계산 불가
            if i < period - 1 + sqrt_period - 1 {
                result.push(None);
            } else {
                // WMA(sqrt(n)) 계산
                let start = i + 1 - sqrt_period;
                let slice = &raw_hma[start..=i];

                let wma_val = self.calculate_wma(slice, sqrt_period)?;
                result.push(Some(wma_val));
            }
        }

        Ok(result)
    }

    /// WMA (Weighted Moving Average) 계산.
    ///
    /// WMA는 최근 데이터에 더 큰 가중치를 부여합니다.
    ///
    /// # 인자
    /// * `prices` - 가격 데이터
    /// * `period` - 기간
    ///
    /// # 반환
    /// 각 시점의 WMA 값
    fn wma(&self, prices: &[Decimal], period: usize) -> IndicatorResult<Vec<Option<Decimal>>> {
        let mut result = Vec::with_capacity(prices.len());

        for i in 0..prices.len() {
            if i < period - 1 {
                result.push(None);
            } else {
                let start = i + 1 - period;
                let slice = &prices[start..=i];
                let wma_val = self.calculate_wma(slice, period)?;
                result.push(Some(wma_val));
            }
        }

        Ok(result)
    }

    /// WMA 값 계산 (단일).
    ///
    /// WMA = (n*P1 + (n-1)*P2 + ... + 1*Pn) / (n + (n-1) + ... + 1)
    ///
    /// # 인자
    /// * `slice` - 가격 데이터 슬라이스
    /// * `period` - 기간
    fn calculate_wma(&self, slice: &[Decimal], period: usize) -> IndicatorResult<Decimal> {
        if slice.len() != period {
            return Err(IndicatorError::CalculationError(
                "슬라이스 길이가 기간과 일치하지 않습니다".to_string(),
            ));
        }

        let mut weighted_sum = Decimal::ZERO;
        let mut weight_sum = Decimal::ZERO;

        for (i, price) in slice.iter().enumerate() {
            let weight = Decimal::from(i + 1);
            weighted_sum += price * weight;
            weight_sum += weight;
        }

        if weight_sum == Decimal::ZERO {
            return Err(IndicatorError::CalculationError(
                "가중치 합이 0입니다".to_string(),
            ));
        }

        Ok(weighted_sum / weight_sum)
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
    fn test_hma_calculation() {
        let indicator = HmaIndicator::new();
        let prices = sample_prices();

        let hma = indicator
            .calculate(&prices, HmaParams { period: 9 })
            .unwrap();

        // 처음 몇 개는 None
        assert!(hma[0].is_none());
        assert!(hma[7].is_none());

        // 충분한 데이터가 있으면 값이 계산됨
        assert!(hma[prices.len() - 1].is_some());
    }

    #[test]
    fn test_wma_calculation() {
        let indicator = HmaIndicator::new();
        let prices = sample_prices();

        let wma = indicator.wma(&prices, 5).unwrap();

        // 처음 4개는 None
        assert!(wma[0].is_none());
        assert!(wma[3].is_none());

        // 5번째부터 값이 있어야 함
        assert!(wma[4].is_some());
    }

    #[test]
    fn test_wma_weighted_properly() {
        let indicator = HmaIndicator::new();
        // 단순한 테스트 케이스: [1, 2, 3]
        let prices = vec![dec!(1.0), dec!(2.0), dec!(3.0)];

        let wma = indicator.wma(&prices, 3).unwrap();

        // WMA = (1*1 + 2*2 + 3*3) / (1 + 2 + 3) = 14 / 6 = 2.333...
        assert!(wma[2].is_some());
        let value = wma[2].unwrap();
        assert!((value - dec!(2.333333333333333333333333333)).abs() < dec!(0.000001));
    }

    #[test]
    fn test_insufficient_data_error() {
        let indicator = HmaIndicator::new();
        let prices = vec![dec!(100.0), dec!(101.0)];

        let result = indicator.calculate(&prices, HmaParams { period: 9 });
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_period_error() {
        let indicator = HmaIndicator::new();
        let prices = sample_prices();

        let result = indicator.calculate(&prices, HmaParams { period: 1 });
        assert!(result.is_err());
    }
}
