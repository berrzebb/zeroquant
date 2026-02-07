//! 캔들 패턴 감지 지표.
//!
//! 대표적인 캔들스틱 패턴을 감지하여 반전 시그널을 제공합니다.
//!
//! ## 지원 패턴
//! - **망치형 (Hammer)**: 하락 추세에서 반등 시그널
//! - **역망치형 (Inverted Hammer)**: 하락 추세에서 반등 가능성
//! - **장악형 (Engulfing)**: 강력한 추세 반전 시그널
//! - **도지 (Doji)**: 시장 우유부단, 추세 전환 가능성
//!
//! ## 활용
//! - 추세 반전 예측
//! - 진입/청산 타이밍 포착
//! - 다른 지표와 결합하여 신뢰도 향상

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use super::{IndicatorError, IndicatorResult};

/// 캔들 패턴 유형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandlePatternType {
    /// 망치형 (강세 반전).
    Hammer,
    /// 역망치형 (강세 반전 가능성).
    InvertedHammer,
    /// 교수형 (약세 반전).
    HangingMan,
    /// 유성형 (약세 반전).
    ShootingStar,
    /// 강세 장악형 (강력한 강세 반전).
    BullishEngulfing,
    /// 약세 장악형 (강력한 약세 반전).
    BearishEngulfing,
    /// 도지 (추세 전환 가능성).
    Doji,
    /// 패턴 없음.
    None,
}

/// 캔들 패턴 파라미터.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CandlePatternParams {
    /// 몸통 비율 임계값 (기본: 0.1, 도지 판단용).
    pub body_ratio_threshold: Decimal,
    /// 그림자 비율 임계값 (기본: 2.0, 망치형 판단용).
    pub shadow_ratio_threshold: Decimal,
    /// 추세 확인 기간 (기본: 5).
    pub trend_period: usize,
}

impl Default for CandlePatternParams {
    fn default() -> Self {
        Self {
            body_ratio_threshold: dec!(0.1),
            shadow_ratio_threshold: dec!(2.0),
            trend_period: 5,
        }
    }
}

/// 캔들 패턴 결과.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CandlePatternResult {
    /// 감지된 패턴.
    pub pattern: CandlePatternType,
    /// 신뢰도 (0.0 ~ 1.0).
    pub confidence: Decimal,
}

/// 캔들 데이터.
#[derive(Debug, Clone, Copy)]
struct Candle {
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
}

impl Candle {
    /// 캔들 몸통 크기.
    fn body(&self) -> Decimal {
        (self.close - self.open).abs()
    }

    /// 전체 캔들 크기.
    fn range(&self) -> Decimal {
        self.high - self.low
    }

    /// 상단 그림자 크기.
    fn upper_shadow(&self) -> Decimal {
        self.high - self.open.max(self.close)
    }

    /// 하단 그림자 크기.
    fn lower_shadow(&self) -> Decimal {
        self.open.min(self.close) - self.low
    }

    /// 상승 캔들 여부.
    fn is_bullish(&self) -> bool {
        self.close > self.open
    }
}

/// 캔들 패턴 감지기.
#[derive(Debug, Default)]
pub struct CandlePatternIndicator;

impl CandlePatternIndicator {
    /// 새로운 캔들 패턴 감지기 생성.
    pub fn new() -> Self {
        Self
    }

    /// 캔들 패턴 감지.
    ///
    /// # 인자
    /// * `open` - 시가 데이터
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - 캔들 패턴 파라미터
    ///
    /// # 반환
    /// 각 시점에서 감지된 패턴과 신뢰도
    pub fn detect(
        &self,
        open: &[Decimal],
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: CandlePatternParams,
    ) -> IndicatorResult<Vec<CandlePatternResult>> {
        if open.len() != high.len() || open.len() != low.len() || open.len() != close.len() {
            return Err(IndicatorError::InvalidParameter(
                "시가, 고가, 저가, 종가 데이터의 길이가 일치하지 않습니다".to_string(),
            ));
        }

        if open.is_empty() {
            return Err(IndicatorError::InsufficientData {
                required: 1,
                provided: 0,
            });
        }

        let mut result = Vec::with_capacity(open.len());

        for i in 0..open.len() {
            let candle = Candle {
                open: open[i],
                high: high[i],
                low: low[i],
                close: close[i],
            };

            // 추세 확인 (충분한 데이터가 있을 때만)
            let trend = if i >= params.trend_period {
                self.detect_trend(close, i, params.trend_period)
            } else {
                0 // 중립
            };

            // 패턴 감지
            let pattern_result =
                self.detect_pattern(&candle, trend, &params, i, open, high, low, close);

            result.push(pattern_result);
        }

        Ok(result)
    }

    /// 단일 캔들에서 패턴 감지.
    #[allow(clippy::too_many_arguments)]
    fn detect_pattern(
        &self,
        candle: &Candle,
        trend: i32,
        params: &CandlePatternParams,
        index: usize,
        open: &[Decimal],
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
    ) -> CandlePatternResult {
        // 도지 패턴 확인
        if let Some(confidence) = self.is_doji(candle, params) {
            return CandlePatternResult {
                pattern: CandlePatternType::Doji,
                confidence,
            };
        }

        // 장악형 패턴 확인 (이전 캔들 필요)
        if index > 0 {
            let prev_candle = Candle {
                open: open[index - 1],
                high: high[index - 1],
                low: low[index - 1],
                close: close[index - 1],
            };

            if let Some(confidence) = self.is_bullish_engulfing(candle, &prev_candle, trend) {
                return CandlePatternResult {
                    pattern: CandlePatternType::BullishEngulfing,
                    confidence,
                };
            }

            if let Some(confidence) = self.is_bearish_engulfing(candle, &prev_candle, trend) {
                return CandlePatternResult {
                    pattern: CandlePatternType::BearishEngulfing,
                    confidence,
                };
            }
        }

        // 망치형/유성형 패턴 확인
        if let Some(confidence) = self.is_hammer(candle, trend, params) {
            return CandlePatternResult {
                pattern: if trend < 0 {
                    CandlePatternType::Hammer
                } else {
                    CandlePatternType::HangingMan
                },
                confidence,
            };
        }

        if let Some(confidence) = self.is_inverted_hammer(candle, trend, params) {
            return CandlePatternResult {
                pattern: if trend < 0 {
                    CandlePatternType::InvertedHammer
                } else {
                    CandlePatternType::ShootingStar
                },
                confidence,
            };
        }

        CandlePatternResult {
            pattern: CandlePatternType::None,
            confidence: Decimal::ZERO,
        }
    }

    /// 도지 패턴 확인.
    fn is_doji(&self, candle: &Candle, params: &CandlePatternParams) -> Option<Decimal> {
        let body_ratio = if candle.range() > Decimal::ZERO {
            candle.body() / candle.range()
        } else {
            Decimal::ZERO
        };

        if body_ratio < params.body_ratio_threshold {
            // 몸통이 매우 작으면 도지
            let confidence = dec!(1.0) - body_ratio / params.body_ratio_threshold;
            Some(confidence.min(dec!(1.0)))
        } else {
            None
        }
    }

    /// 망치형 패턴 확인.
    fn is_hammer(
        &self,
        candle: &Candle,
        trend: i32,
        params: &CandlePatternParams,
    ) -> Option<Decimal> {
        let body = candle.body();
        let lower_shadow = candle.lower_shadow();
        let upper_shadow = candle.upper_shadow();

        // 조건: 하단 그림자 >= 몸통 * 2, 상단 그림자 작음
        if body > Decimal::ZERO
            && lower_shadow >= body * params.shadow_ratio_threshold
            && upper_shadow < body * dec!(0.5)
        {
            let mut confidence = dec!(0.7);

            // 하락 추세에서 더 높은 신뢰도
            if trend < 0 {
                confidence += dec!(0.2);
            }

            // 하단 그림자가 더 길수록 신뢰도 증가
            let shadow_ratio = lower_shadow / body;
            if shadow_ratio > dec!(3.0) {
                confidence += dec!(0.1);
            }

            Some(confidence.min(dec!(1.0)))
        } else {
            None
        }
    }

    /// 역망치형 패턴 확인.
    fn is_inverted_hammer(
        &self,
        candle: &Candle,
        trend: i32,
        params: &CandlePatternParams,
    ) -> Option<Decimal> {
        let body = candle.body();
        let lower_shadow = candle.lower_shadow();
        let upper_shadow = candle.upper_shadow();

        // 조건: 상단 그림자 >= 몸통 * 2, 하단 그림자 작음
        if body > Decimal::ZERO
            && upper_shadow >= body * params.shadow_ratio_threshold
            && lower_shadow < body * dec!(0.5)
        {
            let mut confidence = dec!(0.6);

            // 하락 추세에서 더 높은 신뢰도
            if trend < 0 {
                confidence += dec!(0.2);
            }

            // 상단 그림자가 더 길수록 신뢰도 증가
            let shadow_ratio = upper_shadow / body;
            if shadow_ratio > dec!(3.0) {
                confidence += dec!(0.1);
            }

            Some(confidence.min(dec!(1.0)))
        } else {
            None
        }
    }

    /// 강세 장악형 패턴 확인.
    fn is_bullish_engulfing(&self, candle: &Candle, prev: &Candle, trend: i32) -> Option<Decimal> {
        // 조건: 이전 캔들 하락, 현재 캔들 상승, 현재 몸통이 이전 몸통 완전 포함
        if !prev.is_bullish()
            && candle.is_bullish()
            && candle.open < prev.close
            && candle.close > prev.open
        {
            let mut confidence = dec!(0.8);

            // 하락 추세에서 더 높은 신뢰도
            if trend < 0 {
                confidence += dec!(0.15);
            }

            // 몸통 크기 비율로 신뢰도 조정 (Division by zero 방지)
            if prev.body() > Decimal::ZERO {
                let body_ratio = candle.body() / prev.body();
                if body_ratio > dec!(1.5) {
                    confidence += dec!(0.05);
                }
            }

            Some(confidence.min(dec!(1.0)))
        } else {
            None
        }
    }

    /// 약세 장악형 패턴 확인.
    fn is_bearish_engulfing(&self, candle: &Candle, prev: &Candle, trend: i32) -> Option<Decimal> {
        // 조건: 이전 캔들 상승, 현재 캔들 하락, 현재 몸통이 이전 몸통 완전 포함
        if prev.is_bullish()
            && !candle.is_bullish()
            && candle.open > prev.close
            && candle.close < prev.open
        {
            let mut confidence = dec!(0.8);

            // 상승 추세에서 더 높은 신뢰도
            if trend > 0 {
                confidence += dec!(0.15);
            }

            // 몸통 크기 비율로 신뢰도 조정 (Division by zero 방지)
            if prev.body() > Decimal::ZERO {
                let body_ratio = candle.body() / prev.body();
                if body_ratio > dec!(1.5) {
                    confidence += dec!(0.05);
                }
            }

            Some(confidence.min(dec!(1.0)))
        } else {
            None
        }
    }

    /// 추세 감지.
    ///
    /// # 반환
    /// - 양수: 상승 추세
    /// - 음수: 하락 추세
    /// - 0: 중립
    fn detect_trend(&self, close: &[Decimal], index: usize, period: usize) -> i32 {
        if index < period {
            return 0;
        }

        let current = close[index];
        let past = close[index - period];

        if current > past * dec!(1.02) {
            1 // 상승 추세 (2% 이상)
        } else if current < past * dec!(0.98) {
            -1 // 하락 추세 (2% 이상)
        } else {
            0 // 중립
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_doji_detection() {
        let indicator = CandlePatternIndicator::new();

        // 도지: 시가 = 종가
        let open = vec![dec!(100.0)];
        let high = vec![dec!(102.0)];
        let low = vec![dec!(98.0)];
        let close = vec![dec!(100.0)];

        let result = indicator
            .detect(&open, &high, &low, &close, CandlePatternParams::default())
            .unwrap();

        assert_eq!(result[0].pattern, CandlePatternType::Doji);
        assert!(result[0].confidence > dec!(0.5));
    }

    #[test]
    fn test_hammer_detection() {
        let indicator = CandlePatternIndicator::new();

        // 망치형: 하단 그림자 긴 상승 캔들
        let open = vec![
            dec!(100.0),
            dec!(99.0),
            dec!(98.0),
            dec!(97.0),
            dec!(96.0),
            dec!(95.0),
        ];
        let high = vec![
            dec!(101.0),
            dec!(100.0),
            dec!(99.0),
            dec!(98.0),
            dec!(97.0),
            dec!(101.0),
        ];
        let low = vec![
            dec!(99.0),
            dec!(98.0),
            dec!(97.0),
            dec!(96.0),
            dec!(95.0),
            dec!(90.0),
        ]; // 긴 하단 그림자
        let close = vec![
            dec!(100.0),
            dec!(99.0),
            dec!(98.0),
            dec!(97.0),
            dec!(96.0),
            dec!(100.0),
        ];

        let result = indicator
            .detect(&open, &high, &low, &close, CandlePatternParams::default())
            .unwrap();

        // 마지막 캔들이 망치형이어야 함
        assert!(
            result[5].pattern == CandlePatternType::Hammer
                || result[5].pattern == CandlePatternType::HangingMan
        );
    }

    #[test]
    fn test_bullish_engulfing_detection() {
        let indicator = CandlePatternIndicator::new();

        // 강세 장악형: 하락 캔들 후 큰 상승 캔들
        let open = vec![dec!(100.0), dec!(98.0)];
        let high = vec![dec!(100.0), dec!(102.0)];
        let low = vec![dec!(95.0), dec!(94.0)];
        let close = vec![dec!(96.0), dec!(101.0)]; // 이전 캔들 완전 포함

        let result = indicator
            .detect(&open, &high, &low, &close, CandlePatternParams::default())
            .unwrap();

        assert_eq!(result[1].pattern, CandlePatternType::BullishEngulfing);
        assert!(result[1].confidence > dec!(0.5));
    }

    #[test]
    fn test_bearish_engulfing_detection() {
        let indicator = CandlePatternIndicator::new();

        // 약세 장악형: 상승 캔들 후 큰 하락 캔들
        let open = vec![dec!(95.0), dec!(102.0)];
        let high = vec![dec!(100.0), dec!(105.0)];
        let low = vec![dec!(95.0), dec!(94.0)];
        let close = vec![dec!(100.0), dec!(96.0)]; // 이전 캔들 완전 포함

        let result = indicator
            .detect(&open, &high, &low, &close, CandlePatternParams::default())
            .unwrap();

        assert_eq!(result[1].pattern, CandlePatternType::BearishEngulfing);
        assert!(result[1].confidence > dec!(0.5));
    }

    #[test]
    fn test_no_pattern() {
        let indicator = CandlePatternIndicator::new();

        // 일반적인 캔들
        let open = vec![dec!(100.0)];
        let high = vec![dec!(101.0)];
        let low = vec![dec!(99.0)];
        let close = vec![dec!(100.5)];

        let result = indicator
            .detect(&open, &high, &low, &close, CandlePatternParams::default())
            .unwrap();

        assert_eq!(result[0].pattern, CandlePatternType::None);
    }

    #[test]
    fn test_mismatched_length_error() {
        let indicator = CandlePatternIndicator::new();

        let open = vec![dec!(100.0), dec!(101.0)];
        let high = vec![dec!(101.0)];
        let low = vec![dec!(99.0), dec!(100.0)];
        let close = vec![dec!(100.0), dec!(101.0)];

        let result = indicator.detect(&open, &high, &low, &close, CandlePatternParams::default());
        assert!(result.is_err());
    }
}
