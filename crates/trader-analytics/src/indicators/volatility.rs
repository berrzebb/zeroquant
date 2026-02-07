//! 변동성 지표 (Volatility Indicators).
//!
//! 가격 변동성을 측정하는 지표들을 제공합니다.
//! - Bollinger Bands (볼린저 밴드)
//! - ATR (Average True Range, 평균 실제 범위)

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use super::{IndicatorError, IndicatorResult};

/// 볼린저 밴드 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BollingerBandsParams {
    /// 이동평균 기간 (기본: 20).
    pub period: usize,
    /// 표준편차 배수 (기본: 2.0).
    pub std_dev_multiplier: Decimal,
}

impl Default for BollingerBandsParams {
    fn default() -> Self {
        Self {
            period: 20,
            std_dev_multiplier: dec!(2.0),
        }
    }
}

/// 볼린저 밴드 결과.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BollingerBandsResult {
    /// 상단 밴드 (MA + k × σ).
    pub upper: Option<Decimal>,
    /// 중간 밴드 (이동평균).
    pub middle: Option<Decimal>,
    /// 하단 밴드 (MA - k × σ).
    pub lower: Option<Decimal>,
    /// %B 지표 ((현재가 - 하단) / (상단 - 하단)).
    pub percent_b: Option<Decimal>,
    /// 밴드 폭 ((상단 - 하단) / 중간).
    pub bandwidth: Option<Decimal>,
}

/// ATR 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AtrParams {
    /// ATR 기간 (기본: 14).
    pub period: usize,
}

/// Keltner Channel 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct KeltnerChannelParams {
    /// 이동평균 기간 (기본: 20).
    pub period: usize,
    /// ATR 배수 (기본: 1.5).
    pub atr_multiplier: Decimal,
}

impl Default for KeltnerChannelParams {
    fn default() -> Self {
        Self {
            period: 20,
            atr_multiplier: dec!(1.5),
        }
    }
}

/// Keltner Channel 결과.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct KeltnerChannelResult {
    /// 상단 채널 (MA + k × ATR).
    pub upper: Option<Decimal>,
    /// 중간 채널 (이동평균).
    pub middle: Option<Decimal>,
    /// 하단 채널 (MA - k × ATR).
    pub lower: Option<Decimal>,
    /// 채널 폭 ((상단 - 하단) / 중간).
    pub width: Option<Decimal>,
}

/// TTM Squeeze 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TtmSqueezeParams {
    /// Bollinger Bands 기간 (기본: 20).
    pub bb_period: usize,
    /// Bollinger Bands 표준편차 배수 (기본: 2.0).
    pub bb_std_dev: Decimal,
    /// Keltner Channel 기간 (기본: 20).
    pub kc_period: usize,
    /// Keltner Channel ATR 배수 (기본: 1.5).
    pub kc_atr_multiplier: Decimal,
    /// ATR 계산 기간 (기본: 14).
    pub atr_period: usize,
}

impl Default for TtmSqueezeParams {
    fn default() -> Self {
        Self {
            bb_period: 20,
            bb_std_dev: dec!(2.0),
            kc_period: 20,
            kc_atr_multiplier: dec!(1.5),
            atr_period: 14,
        }
    }
}

/// TTM Squeeze 결과.
///
/// John Carter의 TTM Squeeze 지표.
/// Bollinger Bands가 Keltner Channel 내부로 들어가면 에너지 응축 상태(squeeze).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TtmSqueezeResult {
    /// Squeeze 상태 (BB가 KC 내부에 있으면 true).
    pub is_squeeze: bool,
    /// 연속 Squeeze 카운트 (squeeze가 지속된 기간).
    pub squeeze_count: u32,
    /// 모멘텀 (종가 - KC 중간선).
    pub momentum: Option<Decimal>,
    /// Squeeze 해제 여부 (이전 squeeze에서 방금 벗어남).
    pub released: bool,
}

impl Default for AtrParams {
    fn default() -> Self {
        Self { period: 14 }
    }
}

/// 변동성 지표 계산기.
#[derive(Debug, Default)]
pub struct VolatilityIndicators;

impl VolatilityIndicators {
    /// 새로운 변동성 지표 계산기 생성.
    pub fn new() -> Self {
        Self
    }

    /// 볼린저 밴드 계산.
    ///
    /// 상단 밴드 = MA + (k × σ)
    /// 중간 밴드 = MA (이동평균)
    /// 하단 밴드 = MA - (k × σ)
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `params` - 볼린저 밴드 파라미터
    ///
    /// # 반환
    /// 상단, 중간, 하단 밴드 값들
    pub fn bollinger_bands(
        &self,
        prices: &[Decimal],
        params: BollingerBandsParams,
    ) -> IndicatorResult<Vec<BollingerBandsResult>> {
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
                result.push(BollingerBandsResult {
                    upper: None,
                    middle: None,
                    lower: None,
                    percent_b: None,
                    bandwidth: None,
                });
            } else {
                let window = &prices[i + 1 - period..=i];

                // 이동평균 (중간 밴드)
                let sum: Decimal = window.iter().sum();
                let ma = sum / period_decimal;

                // 표준편차 계산
                let variance: Decimal = window
                    .iter()
                    .map(|&p| {
                        let diff = p - ma;
                        diff * diff
                    })
                    .sum::<Decimal>()
                    / period_decimal;

                let std_dev = self.sqrt_decimal(variance);

                // 밴드 계산
                let deviation = params.std_dev_multiplier * std_dev;
                let upper = ma + deviation;
                let lower = ma - deviation;

                // %B 계산
                let percent_b = if upper != lower {
                    Some((prices[i] - lower) / (upper - lower))
                } else {
                    Some(dec!(0.5)) // 밴드가 수렴하면 중립값
                };

                // 밴드 폭 계산
                let bandwidth = if ma != Decimal::ZERO {
                    Some((upper - lower) / ma)
                } else {
                    None
                };

                result.push(BollingerBandsResult {
                    upper: Some(upper),
                    middle: Some(ma),
                    lower: Some(lower),
                    percent_b,
                    bandwidth,
                });
            }
        }

        Ok(result)
    }

    /// ATR (Average True Range) 계산.
    ///
    /// True Range = max(고가 - 저가, |고가 - 전일종가|, |저가 - 전일종가|)
    /// ATR = True Range의 이동평균
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - ATR 파라미터
    ///
    /// # 반환
    /// ATR 값들
    pub fn atr(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: AtrParams,
    ) -> IndicatorResult<Vec<Option<Decimal>>> {
        let len = high.len().min(low.len()).min(close.len());
        let period = params.period;

        if len < period + 1 {
            return Err(IndicatorError::InsufficientData {
                required: period + 1,
                provided: len,
            });
        }

        // True Range 계산
        let mut true_ranges = Vec::with_capacity(len);
        true_ranges.push(high[0] - low[0]); // 첫 번째는 당일 범위

        for i in 1..len {
            let hl = high[i] - low[i];
            let hc = (high[i] - close[i - 1]).abs();
            let lc = (low[i] - close[i - 1]).abs();
            true_ranges.push(hl.max(hc).max(lc));
        }

        // ATR 계산 (EMA 방식)
        let mut result = Vec::with_capacity(len);
        let alpha = Decimal::ONE / Decimal::from(period);
        let one_minus_alpha = Decimal::ONE - alpha;

        for i in 0..len {
            if i < period - 1 {
                result.push(None);
            } else if i == period - 1 {
                // 초기 ATR은 단순 평균
                let sum: Decimal = true_ranges[..=i].iter().sum();
                let initial_atr = sum / Decimal::from(period);
                result.push(Some(initial_atr));
            } else {
                // EMA 방식으로 ATR 업데이트
                if let Some(prev_atr) = result[i - 1] {
                    let atr = (true_ranges[i] * alpha) + (prev_atr * one_minus_alpha);
                    result.push(Some(atr));
                } else {
                    result.push(None);
                }
            }
        }

        Ok(result)
    }

    /// ATR 퍼센트 계산 (ATR / 현재가 × 100).
    ///
    /// 가격 대비 변동성을 측정하는 데 유용합니다.
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - ATR 파라미터
    ///
    /// # 반환
    /// ATR 퍼센트 값들
    pub fn atr_percent(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: AtrParams,
    ) -> IndicatorResult<Vec<Option<Decimal>>> {
        let atr_values = self.atr(high, low, close, params)?;
        let len = close.len().min(atr_values.len());

        let mut result = Vec::with_capacity(len);

        for i in 0..len {
            match atr_values[i] {
                Some(atr) if close[i] != Decimal::ZERO => {
                    result.push(Some((atr / close[i]) * dec!(100)));
                }
                _ => result.push(None),
            }
        }

        Ok(result)
    }

    /// Keltner Channel 계산.
    ///
    /// 상단 채널 = MA + (k × ATR)
    /// 중간 채널 = MA (이동평균)
    /// 하단 채널 = MA - (k × ATR)
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - Keltner Channel 파라미터
    ///
    /// # 반환
    /// Keltner Channel 값들
    pub fn keltner_channel(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: KeltnerChannelParams,
    ) -> IndicatorResult<Vec<KeltnerChannelResult>> {
        let period = params.period;
        let len = close.len();

        if len < period {
            return Err(IndicatorError::InsufficientData {
                required: period,
                provided: len,
            });
        }

        // ATR 계산
        let atr_values = self.atr(
            high,
            low,
            close,
            AtrParams {
                period: params.period,
            },
        )?;

        let mut result = Vec::with_capacity(len);
        let period_decimal = Decimal::from(period);

        for i in 0..len {
            if i < period - 1 {
                result.push(KeltnerChannelResult {
                    upper: None,
                    middle: None,
                    lower: None,
                    width: None,
                });
            } else {
                // 이동평균 계산 (중간선)
                let window = &close[i + 1 - period..=i];
                let sum: Decimal = window.iter().sum();
                let ma = sum / period_decimal;

                // ATR 기반 채널 계산
                if let Some(atr) = atr_values[i] {
                    let deviation = params.atr_multiplier * atr;
                    let upper = ma + deviation;
                    let lower = ma - deviation;

                    // 채널 폭 계산
                    let width = if ma != Decimal::ZERO {
                        Some((upper - lower) / ma)
                    } else {
                        None
                    };

                    result.push(KeltnerChannelResult {
                        upper: Some(upper),
                        middle: Some(ma),
                        lower: Some(lower),
                        width,
                    });
                } else {
                    result.push(KeltnerChannelResult {
                        upper: None,
                        middle: None,
                        lower: None,
                        width: None,
                    });
                }
            }
        }

        Ok(result)
    }

    /// TTM Squeeze 계산.
    ///
    /// John Carter의 TTM Squeeze 지표.
    /// Bollinger Bands가 Keltner Channel 내부로 들어가면 에너지 응축(squeeze) 상태.
    /// Squeeze가 해제되면 강한 방향성 움직임 기대.
    ///
    /// # Squeeze 조건
    /// - BB Upper < KC Upper AND BB Lower > KC Lower
    /// - 즉, BB 폭 < KC 폭
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - TTM Squeeze 파라미터
    ///
    /// # 반환
    /// TTM Squeeze 상태 및 모멘텀 정보
    pub fn ttm_squeeze(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: TtmSqueezeParams,
    ) -> IndicatorResult<Vec<TtmSqueezeResult>> {
        let len = close.len();

        // Bollinger Bands 계산
        let bb_results = self.bollinger_bands(
            close,
            BollingerBandsParams {
                period: params.bb_period,
                std_dev_multiplier: params.bb_std_dev,
            },
        )?;

        // Keltner Channel 계산
        let kc_results = self.keltner_channel(
            high,
            low,
            close,
            KeltnerChannelParams {
                period: params.kc_period,
                atr_multiplier: params.kc_atr_multiplier,
            },
        )?;

        let mut result = Vec::with_capacity(len);
        let mut squeeze_count = 0u32;
        let mut prev_squeeze = false;

        for i in 0..len {
            let bb = &bb_results[i];
            let kc = &kc_results[i];

            // BB와 KC가 모두 계산되어 있는지 확인
            let is_squeeze =
                if let (Some(bb_upper), Some(bb_lower), Some(kc_upper), Some(kc_lower)) =
                    (bb.upper, bb.lower, kc.upper, kc.lower)
                {
                    // Squeeze 조건: BB가 KC 내부에 있음
                    bb_upper < kc_upper && bb_lower > kc_lower
                } else {
                    false
                };

            // Squeeze 카운트 업데이트
            if is_squeeze {
                squeeze_count += 1;
            } else {
                squeeze_count = 0;
            }

            // Squeeze 해제 감지 (이전에 squeeze였다가 지금 해제됨)
            let released = prev_squeeze && !is_squeeze;

            // 모멘텀 계산 (종가 - KC 중간선)
            let momentum = kc.middle.map(|kc_middle| close[i] - kc_middle);

            result.push(TtmSqueezeResult {
                is_squeeze,
                squeeze_count,
                momentum,
                released,
            });

            prev_squeeze = is_squeeze;
        }

        Ok(result)
    }

    /// Decimal 제곱근 계산 (Newton-Raphson 방법).
    ///
    /// Decimal 타입은 기본 제곱근 함수가 없으므로 직접 구현합니다.
    fn sqrt_decimal(&self, value: Decimal) -> Decimal {
        if value <= Decimal::ZERO {
            return Decimal::ZERO;
        }

        // Newton-Raphson 방법으로 제곱근 근사
        let mut x = value;
        let two = dec!(2);

        // 10회 반복이면 충분한 정밀도
        for _ in 0..10 {
            x = (x + value / x) / two;
        }

        x
    }

    /// 변동성 필터 체크.
    ///
    /// ATR이 임계값을 초과하는지 확인합니다.
    /// 높은 변동성 시 거래를 중단하는 리스크 관리에 사용됩니다.
    ///
    /// # 인자
    /// * `atr_percent` - ATR 퍼센트 값
    /// * `threshold` - 변동성 임계값 (%)
    ///
    /// # 반환
    /// 변동성이 임계값 초과 여부
    pub fn is_high_volatility(&self, atr_percent: Decimal, threshold: Decimal) -> bool {
        atr_percent > threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_ohlc() -> (Vec<Decimal>, Vec<Decimal>, Vec<Decimal>) {
        let high = vec![
            dec!(102),
            dec!(104),
            dec!(103),
            dec!(105),
            dec!(107),
            dec!(106),
            dec!(108),
            dec!(110),
            dec!(109),
            dec!(111),
            dec!(113),
            dec!(112),
            dec!(114),
            dec!(116),
            dec!(115),
            dec!(117),
            dec!(119),
            dec!(118),
            dec!(120),
            dec!(122),
        ];
        let low = vec![
            dec!(98),
            dec!(100),
            dec!(99),
            dec!(101),
            dec!(103),
            dec!(102),
            dec!(104),
            dec!(106),
            dec!(105),
            dec!(107),
            dec!(109),
            dec!(108),
            dec!(110),
            dec!(112),
            dec!(111),
            dec!(113),
            dec!(115),
            dec!(114),
            dec!(116),
            dec!(118),
        ];
        let close = vec![
            dec!(100),
            dec!(102),
            dec!(101),
            dec!(103),
            dec!(105),
            dec!(104),
            dec!(106),
            dec!(108),
            dec!(107),
            dec!(109),
            dec!(111),
            dec!(110),
            dec!(112),
            dec!(114),
            dec!(113),
            dec!(115),
            dec!(117),
            dec!(116),
            dec!(118),
            dec!(120),
        ];

        (high, low, close)
    }

    #[test]
    fn test_bollinger_bands() {
        let volatility = VolatilityIndicators::new();
        let (_, _, close) = sample_ohlc();

        let bb = volatility
            .bollinger_bands(
                &close,
                BollingerBandsParams {
                    period: 10,
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(bb.len(), close.len());

        // 처음 9개는 None
        assert!(bb[8].middle.is_none());

        // 10번째부터 값이 있어야 함
        assert!(bb[9].middle.is_some());
        assert!(bb[9].upper.is_some());
        assert!(bb[9].lower.is_some());

        // 상단 > 중간 > 하단
        if let (Some(u), Some(m), Some(l)) = (bb[15].upper, bb[15].middle, bb[15].lower) {
            assert!(u > m);
            assert!(m > l);
        }
    }

    #[test]
    fn test_bollinger_percent_b() {
        let volatility = VolatilityIndicators::new();
        let (_, _, close) = sample_ohlc();

        let bb = volatility
            .bollinger_bands(
                &close,
                BollingerBandsParams {
                    period: 10,
                    ..Default::default()
                },
            )
            .unwrap();

        // %B가 0-1 범위 근처인지 확인
        for b in bb.iter().skip(10) {
            if let Some(percent_b) = b.percent_b {
                // 약간의 오버슈팅 허용 (밴드 밖으로 나갈 수 있음)
                assert!(percent_b >= dec!(-0.5) && percent_b <= dec!(1.5));
            }
        }
    }

    #[test]
    fn test_atr_calculation() {
        let volatility = VolatilityIndicators::new();
        let (high, low, close) = sample_ohlc();

        let atr = volatility
            .atr(&high, &low, &close, AtrParams { period: 14 })
            .unwrap();

        assert_eq!(atr.len(), close.len());

        // 처음 13개는 None
        assert!(atr[12].is_none());

        // 14번째부터 값이 있어야 함
        assert!(atr[13].is_some());

        // ATR은 양수
        for value in atr.iter().flatten() {
            assert!(*value > Decimal::ZERO);
        }
    }

    #[test]
    fn test_atr_percent() {
        let volatility = VolatilityIndicators::new();
        let (high, low, close) = sample_ohlc();

        let atr_pct = volatility
            .atr_percent(&high, &low, &close, AtrParams { period: 14 })
            .unwrap();

        // ATR%는 양수
        for value in atr_pct.iter().flatten() {
            assert!(*value > Decimal::ZERO);
        }
    }

    #[test]
    fn test_sqrt_decimal() {
        let volatility = VolatilityIndicators::new();

        // 간단한 제곱근 테스트
        let sqrt_4 = volatility.sqrt_decimal(dec!(4));
        assert!((sqrt_4 - dec!(2)).abs() < dec!(0.0001));

        let sqrt_9 = volatility.sqrt_decimal(dec!(9));
        assert!((sqrt_9 - dec!(3)).abs() < dec!(0.0001));

        let sqrt_2 = volatility.sqrt_decimal(dec!(2));
        assert!((sqrt_2 - dec!(1.4142)).abs() < dec!(0.001));
    }

    #[test]
    fn test_high_volatility_filter() {
        let volatility = VolatilityIndicators::new();

        assert!(volatility.is_high_volatility(dec!(5.0), dec!(3.0)));
        assert!(!volatility.is_high_volatility(dec!(2.0), dec!(3.0)));
    }
}
