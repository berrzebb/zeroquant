//! 거래량 기반 지표 (Volume-Based Indicators).
//!
//! ## OBV (On-Balance Volume)
//!
//! OBV는 거래량을 이용하여 스마트 머니(기관투자자)의 자금 흐름을 추적하는 지표입니다.
//!
//! ### 계산 방식
//! - 종가 상승: OBV += 거래량
//! - 종가 하락: OBV -= 거래량
//! - 종가 동일: OBV 변화 없음
//!
//! ### 활용
//! - 가격 상승 + OBV 상승: 강한 상승 추세
//! - 가격 상승 + OBV 하락: 약한 상승 (다이버전스)
//! - OBV 돌파: 추세 전환 신호
//!
//! ## VWAP (Volume Weighted Average Price)
//!
//! VWAP는 거래량 가중 평균 가격으로, 기관 투자자들이 많이 사용하는 지표입니다.
//!
//! ### 계산 방식
//! - Typical Price (TP) = (High + Low + Close) / 3
//! - VWAP = Σ(TP × Volume) / Σ(Volume)
//!
//! ### 활용
//! - 가격 > VWAP: 강세 (매수 우위)
//! - 가격 < VWAP: 약세 (매도 우위)
//! - VWAP 밴드: 지지/저항선으로 활용

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::{IndicatorError, IndicatorResult};

/// OBV 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[derive(Default)]
pub struct ObvParams {
    /// 초기값 (기본: 0).
    pub initial_value: i64,
}


/// OBV 결과.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ObvResult {
    /// OBV 값.
    pub obv: i64,
    /// OBV 변화량 (전일 대비).
    pub change: i64,
}

/// OBV 계산기.
#[derive(Debug, Default)]
pub struct ObvIndicator;

impl ObvIndicator {
    /// 새로운 OBV 계산기 생성.
    pub fn new() -> Self {
        Self
    }

    /// OBV (On-Balance Volume) 계산.
    ///
    /// # 인자
    /// * `close` - 종가 데이터
    /// * `volume` - 거래량 데이터
    /// * `params` - OBV 파라미터
    ///
    /// # 반환
    /// 각 시점의 OBV 값과 변화량
    pub fn calculate(
        &self,
        close: &[Decimal],
        volume: &[Decimal],
        params: ObvParams,
    ) -> IndicatorResult<Vec<ObvResult>> {
        if close.len() != volume.len() {
            return Err(IndicatorError::InvalidParameter(
                "종가와 거래량 데이터의 길이가 일치하지 않습니다".to_string(),
            ));
        }

        if close.is_empty() {
            return Err(IndicatorError::InsufficientData {
                required: 2,
                provided: close.len(),
            });
        }

        let mut result = Vec::with_capacity(close.len());
        let mut current_obv = params.initial_value;

        for i in 0..close.len() {
            let change = if i == 0 {
                // 첫 번째 데이터는 변화 없음
                0
            } else {
                let price_change = close[i] - close[i - 1];

                if price_change > Decimal::ZERO {
                    // 가격 상승: 거래량 추가
                    self.volume_to_i64(volume[i])?
                } else if price_change < Decimal::ZERO {
                    // 가격 하락: 거래량 차감
                    -self.volume_to_i64(volume[i])?
                } else {
                    // 가격 동일: 변화 없음
                    0
                }
            };

            current_obv = current_obv.saturating_add(change);

            result.push(ObvResult {
                obv: current_obv,
                change,
            });
        }

        Ok(result)
    }

    /// OBV 다이버전스 감지.
    ///
    /// 가격과 OBV의 방향이 반대인 경우를 감지합니다.
    ///
    /// # 인자
    /// * `close` - 종가 데이터
    /// * `obv_results` - OBV 계산 결과
    /// * `lookback` - 비교 기간 (기본: 5)
    ///
    /// # 반환
    /// 각 시점에서 다이버전스 발생 여부
    /// - true: 약세 다이버전스 (가격 상승, OBV 하락)
    /// - false: 정상 또는 강세 다이버전스
    pub fn detect_divergence(
        &self,
        close: &[Decimal],
        obv_results: &[ObvResult],
        lookback: usize,
    ) -> IndicatorResult<Vec<bool>> {
        if close.len() != obv_results.len() {
            return Err(IndicatorError::InvalidParameter(
                "종가와 OBV 데이터의 길이가 일치하지 않습니다".to_string(),
            ));
        }

        let mut divergences = Vec::with_capacity(close.len());

        for i in 0..close.len() {
            if i < lookback {
                divergences.push(false);
                continue;
            }

            let price_change = close[i] - close[i - lookback];
            let obv_change = obv_results[i].obv - obv_results[i - lookback].obv;

            // 약세 다이버전스: 가격은 상승했지만 OBV는 하락
            let is_bearish_divergence = price_change > Decimal::ZERO && obv_change < 0;

            divergences.push(is_bearish_divergence);
        }

        Ok(divergences)
    }

    /// Decimal 거래량을 i64로 변환.
    ///
    /// # 인자
    /// * `volume` - 거래량 (Decimal)
    ///
    /// # 반환
    /// i64 거래량
    fn volume_to_i64(&self, volume: Decimal) -> IndicatorResult<i64> {
        // Decimal을 정수로 변환 (소수점 이하 버림)
        volume.to_i64().ok_or_else(|| {
            IndicatorError::CalculationError(format!(
                "거래량을 i64로 변환할 수 없습니다: {}",
                volume
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_data() -> (Vec<Decimal>, Vec<Decimal>) {
        let close = vec![
            dec!(100.0),
            dec!(102.0), // 상승
            dec!(101.0), // 하락
            dec!(103.0), // 상승
            dec!(103.0), // 동일
            dec!(102.0), // 하락
            dec!(104.0), // 상승
        ];

        let volume = vec![
            dec!(1000.0),
            dec!(1500.0),
            dec!(1200.0),
            dec!(1800.0),
            dec!(1000.0),
            dec!(1300.0),
            dec!(2000.0),
        ];

        (close, volume)
    }

    #[test]
    fn test_obv_calculation() {
        let indicator = ObvIndicator::new();
        let (close, volume) = sample_data();

        let obv = indicator
            .calculate(&close, &volume, ObvParams::default())
            .unwrap();

        assert_eq!(obv.len(), close.len());

        // 첫 번째는 변화 없음
        assert_eq!(obv[0].change, 0);

        // 두 번째는 가격 상승 -> 거래량 추가
        assert_eq!(obv[1].change, 1500);
        assert_eq!(obv[1].obv, 1500);

        // 세 번째는 가격 하락 -> 거래량 차감
        assert_eq!(obv[2].change, -1200);
        assert_eq!(obv[2].obv, 300);

        // 네 번째는 가격 상승 -> 거래량 추가
        assert_eq!(obv[3].change, 1800);
        assert_eq!(obv[3].obv, 2100);

        // 다섯 번째는 가격 동일 -> 변화 없음
        assert_eq!(obv[4].change, 0);
        assert_eq!(obv[4].obv, 2100);
    }

    #[test]
    fn test_obv_with_custom_initial() {
        let indicator = ObvIndicator::new();
        let (close, volume) = sample_data();

        let obv = indicator
            .calculate(
                &close,
                &volume,
                ObvParams {
                    initial_value: 10000,
                },
            )
            .unwrap();

        // 초기값이 반영되어야 함
        assert_eq!(obv[0].obv, 10000);
        assert_eq!(obv[1].obv, 11500);
    }

    #[test]
    fn test_divergence_detection() {
        let indicator = ObvIndicator::new();
        let close = vec![
            dec!(100.0),
            dec!(102.0),
            dec!(104.0),
            dec!(106.0),
            dec!(108.0), // 가격은 계속 상승
        ];
        let volume = vec![
            dec!(2000.0),
            dec!(1500.0), // 거래량은 감소
            dec!(1000.0),
            dec!(800.0),
            dec!(500.0),
        ];

        let obv_results = indicator
            .calculate(&close, &volume, ObvParams::default())
            .unwrap();

        let divergences = indicator
            .detect_divergence(&close, &obv_results, 2)
            .unwrap();

        // 가격 상승 + OBV 하락 = 약세 다이버전스
        assert!(divergences[divergences.len() - 1]);
    }

    #[test]
    fn test_mismatched_length_error() {
        let indicator = ObvIndicator::new();
        let close = vec![dec!(100.0), dec!(101.0)];
        let volume = vec![dec!(1000.0)];

        let result = indicator.calculate(&close, &volume, ObvParams::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_data_error() {
        let indicator = ObvIndicator::new();
        let close: Vec<Decimal> = vec![];
        let volume: Vec<Decimal> = vec![];

        let result = indicator.calculate(&close, &volume, ObvParams::default());
        assert!(result.is_err());
    }
}

// ============================================================================
// VWAP (Volume Weighted Average Price)
// ============================================================================

/// VWAP 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VwapParams {
    /// 밴드 표준편차 배수 (기본: 2.0).
    /// VWAP 상단/하단 밴드 계산에 사용.
    pub band_multiplier: Decimal,
    /// 세션 리셋 여부 (기본: false).
    /// true면 매일 VWAP 리셋, false면 누적.
    pub reset_daily: bool,
}

impl Default for VwapParams {
    fn default() -> Self {
        Self {
            band_multiplier: Decimal::TWO,
            reset_daily: false,
        }
    }
}

/// VWAP 결과.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VwapResult {
    /// VWAP 값.
    pub vwap: Decimal,
    /// 상단 밴드 (VWAP + std * multiplier).
    pub upper_band: Option<Decimal>,
    /// 하단 밴드 (VWAP - std * multiplier).
    pub lower_band: Option<Decimal>,
    /// 누적 거래량.
    pub cumulative_volume: Decimal,
    /// 현재 가격과 VWAP의 괴리율 (%).
    pub deviation_pct: Option<Decimal>,
}

/// VWAP 계산기.
#[derive(Debug, Default)]
pub struct VwapIndicator;

impl VwapIndicator {
    /// 새로운 VWAP 계산기 생성.
    pub fn new() -> Self {
        Self
    }

    /// VWAP (Volume Weighted Average Price) 계산.
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `volume` - 거래량 데이터
    /// * `params` - VWAP 파라미터
    ///
    /// # 반환
    /// 각 시점의 VWAP 값과 밴드
    ///
    /// # 공식
    /// - Typical Price (TP) = (High + Low + Close) / 3
    /// - VWAP = Σ(TP × Volume) / Σ(Volume)
    pub fn calculate(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        volume: &[Decimal],
        params: VwapParams,
    ) -> IndicatorResult<Vec<VwapResult>> {
        let len = high.len();

        // 데이터 길이 검증
        if len != low.len() || len != close.len() || len != volume.len() {
            return Err(IndicatorError::InvalidParameter(
                "고가, 저가, 종가, 거래량 데이터의 길이가 일치하지 않습니다".to_string(),
            ));
        }

        if len == 0 {
            return Err(IndicatorError::InsufficientData {
                required: 1,
                provided: 0,
            });
        }

        let mut results = Vec::with_capacity(len);
        let mut cumulative_tp_vol = Decimal::ZERO; // Σ(TP × Volume)
        let mut cumulative_vol = Decimal::ZERO; // Σ(Volume)
        let mut tp_squared_vol_sum = Decimal::ZERO; // 표준편차 계산용

        let three = Decimal::from(3);

        for i in 0..len {
            // Typical Price 계산
            let tp = (high[i] + low[i] + close[i]) / three;

            // 누적값 갱신
            cumulative_tp_vol += tp * volume[i];
            cumulative_vol += volume[i];
            tp_squared_vol_sum += tp * tp * volume[i];

            // VWAP 계산 (거래량이 0이면 이전 값 유지 또는 TP 사용)
            let vwap = if cumulative_vol > Decimal::ZERO {
                cumulative_tp_vol / cumulative_vol
            } else {
                tp
            };

            // 표준편차 및 밴드 계산
            let (upper_band, lower_band) = if cumulative_vol > Decimal::ZERO && i > 0 {
                // 거래량 가중 표준편차: sqrt(Σ(TP² × Vol) / Σ(Vol) - VWAP²)
                let variance = (tp_squared_vol_sum / cumulative_vol) - (vwap * vwap);
                if variance > Decimal::ZERO {
                    // 근사 제곱근 계산 (Newton-Raphson)
                    let std_dev = self.sqrt_approx(variance);
                    let band_width = std_dev * params.band_multiplier;
                    (Some(vwap + band_width), Some(vwap - band_width))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            // 괴리율 계산
            let deviation_pct = if vwap > Decimal::ZERO {
                Some(((close[i] - vwap) / vwap) * Decimal::ONE_HUNDRED)
            } else {
                None
            };

            results.push(VwapResult {
                vwap,
                upper_band,
                lower_band,
                cumulative_volume: cumulative_vol,
                deviation_pct,
            });
        }

        Ok(results)
    }

    /// VWAP 상/하향 돌파 감지.
    ///
    /// # 인자
    /// * `close` - 종가 데이터
    /// * `vwap_results` - VWAP 계산 결과
    ///
    /// # 반환
    /// 각 시점에서 돌파 방향 (1: 상향 돌파, -1: 하향 돌파, 0: 없음)
    pub fn detect_crossover(
        &self,
        close: &[Decimal],
        vwap_results: &[VwapResult],
    ) -> IndicatorResult<Vec<i8>> {
        if close.len() != vwap_results.len() {
            return Err(IndicatorError::InvalidParameter(
                "종가와 VWAP 데이터의 길이가 일치하지 않습니다".to_string(),
            ));
        }

        let mut crossovers = Vec::with_capacity(close.len());

        for i in 0..close.len() {
            if i == 0 {
                crossovers.push(0);
                continue;
            }

            let prev_close = close[i - 1];
            let curr_close = close[i];
            let prev_vwap = vwap_results[i - 1].vwap;
            let curr_vwap = vwap_results[i].vwap;

            // 상향 돌파: 이전에 VWAP 아래, 현재 VWAP 위
            if prev_close < prev_vwap && curr_close > curr_vwap {
                crossovers.push(1);
            }
            // 하향 돌파: 이전에 VWAP 위, 현재 VWAP 아래
            else if prev_close > prev_vwap && curr_close < curr_vwap {
                crossovers.push(-1);
            } else {
                crossovers.push(0);
            }
        }

        Ok(crossovers)
    }

    /// Newton-Raphson 방식 제곱근 근사.
    fn sqrt_approx(&self, value: Decimal) -> Decimal {
        if value <= Decimal::ZERO {
            return Decimal::ZERO;
        }

        // 초기 추정값
        let mut x = value / Decimal::TWO;
        let tolerance = Decimal::new(1, 10); // 0.0000000001

        // 최대 20회 반복
        for _ in 0..20 {
            let next_x = (x + value / x) / Decimal::TWO;
            if (next_x - x).abs() < tolerance {
                return next_x;
            }
            x = next_x;
        }

        x
    }
}

#[cfg(test)]
mod vwap_tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_ohlcv() -> (Vec<Decimal>, Vec<Decimal>, Vec<Decimal>, Vec<Decimal>) {
        let high = vec![
            dec!(102.0),
            dec!(104.0),
            dec!(103.0),
            dec!(106.0),
            dec!(105.0),
        ];
        let low = vec![
            dec!(98.0),
            dec!(100.0),
            dec!(99.0),
            dec!(102.0),
            dec!(101.0),
        ];
        let close = vec![
            dec!(100.0),
            dec!(103.0),
            dec!(101.0),
            dec!(105.0),
            dec!(103.0),
        ];
        let volume = vec![
            dec!(1000.0),
            dec!(1500.0),
            dec!(1200.0),
            dec!(2000.0),
            dec!(1800.0),
        ];
        (high, low, close, volume)
    }

    #[test]
    fn test_vwap_calculation() {
        let indicator = VwapIndicator::new();
        let (high, low, close, volume) = sample_ohlcv();

        let results = indicator
            .calculate(&high, &low, &close, &volume, VwapParams::default())
            .unwrap();

        assert_eq!(results.len(), 5);

        // 첫 번째 VWAP = TP1 (거래량 하나만 있으므로)
        // TP1 = (102 + 98 + 100) / 3 = 100
        assert_eq!(results[0].vwap, dec!(100));
        assert_eq!(results[0].cumulative_volume, dec!(1000));

        // 두 번째 이후부터 누적 계산
        // 누적 거래량 확인
        assert_eq!(results[1].cumulative_volume, dec!(2500)); // 1000 + 1500
        assert_eq!(results[2].cumulative_volume, dec!(3700)); // 2500 + 1200
    }

    #[test]
    fn test_vwap_deviation() {
        let indicator = VwapIndicator::new();
        let (high, low, close, volume) = sample_ohlcv();

        let results = indicator
            .calculate(&high, &low, &close, &volume, VwapParams::default())
            .unwrap();

        // 괴리율이 계산되어야 함
        for result in &results {
            assert!(result.deviation_pct.is_some());
        }
    }

    #[test]
    fn test_vwap_crossover() {
        let indicator = VwapIndicator::new();

        // VWAP 상향 돌파 시나리오
        let close = vec![dec!(99.0), dec!(100.0), dec!(102.0)];
        let vwap_results = vec![
            VwapResult {
                vwap: dec!(100.0),
                upper_band: None,
                lower_band: None,
                cumulative_volume: dec!(1000),
                deviation_pct: None,
            },
            VwapResult {
                vwap: dec!(100.5),
                upper_band: None,
                lower_band: None,
                cumulative_volume: dec!(2000),
                deviation_pct: None,
            },
            VwapResult {
                vwap: dec!(101.0),
                upper_band: None,
                lower_band: None,
                cumulative_volume: dec!(3000),
                deviation_pct: None,
            },
        ];

        let crossovers = indicator.detect_crossover(&close, &vwap_results).unwrap();

        assert_eq!(crossovers[0], 0); // 첫 번째는 항상 0
        assert_eq!(crossovers[1], 0); // 아직 돌파 안함
        assert_eq!(crossovers[2], 1); // 상향 돌파
    }

    #[test]
    fn test_vwap_mismatched_length() {
        let indicator = VwapIndicator::new();
        let high = vec![dec!(100.0)];
        let low = vec![dec!(98.0), dec!(99.0)];
        let close = vec![dec!(99.0)];
        let volume = vec![dec!(1000.0)];

        let result = indicator.calculate(&high, &low, &close, &volume, VwapParams::default());
        assert!(result.is_err());
    }
}
