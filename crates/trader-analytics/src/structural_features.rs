//! 구조적 피처 계산기.
//!
//! "살아있는 횡보"와 "죽은 횡보"를 구분하여 돌파 가능성을 예측합니다.
//!
//! # 설계 원칙
//!
//! - IndicatorEngine 기반 정밀 계산 (SMA, RSI, BollingerBands 활용)
//! - 모든 수치를 Decimal로 유지하여 금융 데이터 정밀도 보장
//! - 커스텀 피처 (low_trend, vol_quality, range_pos)는 Decimal 기반 자체 구현
//!
//! # 사용 예시
//!
//! ```ignore
//! use trader_analytics::{IndicatorEngine, StructuralFeaturesCalculator};
//!
//! let engine = IndicatorEngine::new();
//! let features = StructuralFeaturesCalculator::from_candles("005930", &candles, &engine)?;
//!
//! if features.is_alive_consolidation() {
//!     println!("살아있는 횡보 감지! 돌파 가능성: {}%", features.breakout_score());
//! }
//! ```

use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use trader_core::{domain::StructuralFeatures, Kline};

use crate::indicators::{
    BollingerBandsParams, IndicatorEngine, IndicatorError, IndicatorResult, RsiParams, SmaParams,
};

/// 최소 필요 캔들 개수.
pub const MIN_STRUCTURAL_CANDLES: usize = 40;

/// 구조적 피처 계산기.
///
/// IndicatorEngine을 활용하여 정밀한 구조적 피처를 계산합니다.
pub struct StructuralFeaturesCalculator;

impl StructuralFeaturesCalculator {
    /// OHLCV 데이터로부터 구조적 피처 계산.
    ///
    /// IndicatorEngine의 SMA, RSI, BollingerBands를 활용하여
    /// 정확한 구조적 피처를 계산합니다.
    ///
    /// # 인자
    ///
    /// * `ticker` - 종목 티커 (예: "005930", "AAPL")
    /// * `candles` - OHLCV 데이터 (최소 40개)
    /// * `engine` - 지표 계산 엔진
    ///
    /// # 반환
    ///
    /// StructuralFeatures (Decimal 기반)
    ///
    /// # 에러
    ///
    /// - 캔들 개수가 40개 미만인 경우
    /// - 지표 계산 실패
    pub fn from_candles(
        ticker: &str,
        candles: &[Kline],
        engine: &IndicatorEngine,
    ) -> IndicatorResult<StructuralFeatures> {
        if candles.len() < MIN_STRUCTURAL_CANDLES {
            return Err(IndicatorError::InsufficientData {
                required: MIN_STRUCTURAL_CANDLES,
                provided: candles.len(),
            });
        }

        let closes: Vec<Decimal> = candles.iter().map(|k| k.close).collect();

        // 1. MA20 이격도 (IndicatorEngine SMA 사용)
        let ma20 = engine.sma(&closes, SmaParams { period: 20 })?;
        let current_price = closes.last().copied().unwrap_or(Decimal::ZERO);
        let current_ma20 = ma20
            .last()
            .and_then(|v| *v)
            .ok_or_else(|| IndicatorError::CalculationError("MA20 계산 실패".to_string()))?;

        let dist_ma20 = if current_ma20 > Decimal::ZERO {
            ((current_price - current_ma20) / current_ma20 * dec!(100))
                .max(dec!(-20))
                .min(dec!(20))
        } else {
            Decimal::ZERO
        };

        // 2. 볼린저 밴드 (IndicatorEngine BB 사용)
        let bb = engine.bollinger_bands(&closes, BollingerBandsParams::default())?;
        let last_bb = bb
            .last()
            .ok_or_else(|| IndicatorError::CalculationError("볼린저 밴드 계산 실패".to_string()))?;

        let (bb_upper, bb_middle, bb_lower, bb_width) =
            match (last_bb.upper, last_bb.lower, last_bb.middle) {
                (Some(upper), Some(lower), Some(middle)) if middle > Decimal::ZERO => {
                    let width = ((upper - lower) / middle * dec!(100))
                        .max(Decimal::ZERO)
                        .min(dec!(50));
                    (upper, middle, lower, width)
                }
                _ => (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO, Decimal::ZERO),
            };

        // 3. RSI (IndicatorEngine RSI 사용)
        let rsi_values = engine.rsi(&closes, RsiParams { period: 14 })?;
        let rsi = rsi_values
            .last()
            .and_then(|v| *v)
            .ok_or_else(|| IndicatorError::CalculationError("RSI 계산 실패".to_string()))?;

        // 4. 커스텀 피처 (Decimal 기반 자체 계산)
        let low_trend = Self::calculate_low_trend(candles);
        let vol_quality = Self::calculate_vol_quality(candles);
        let range_pos = Self::calculate_range_pos(candles);

        Ok(StructuralFeatures {
            ticker: ticker.to_string(),
            low_trend,
            vol_quality,
            range_pos,
            dist_ma20,
            bb_width,
            bb_upper,
            bb_middle,
            bb_lower,
            rsi,
            timestamp: Utc::now(),
        })
    }

    /// Higher Low 강도 계산.
    ///
    /// 최근 20일간의 저점이 상승하는지 측정합니다.
    ///
    /// # 반환
    ///
    /// -1.0 ~ 1.0 (양수=저점 상승, 음수=저점 하락)
    fn calculate_low_trend(candles: &[Kline]) -> Decimal {
        let len = candles.len().min(20);
        if len < 10 {
            return Decimal::ZERO;
        }

        let recent = &candles[candles.len() - len..];

        // 최근 10개와 이전 10개의 평균 저가 비교
        let first_half: Decimal = recent[..len / 2].iter().map(|k| k.low).sum();
        let first_count = Decimal::from(len / 2);

        let second_half: Decimal = recent[len / 2..].iter().map(|k| k.low).sum();
        let second_count = Decimal::from(len - len / 2);

        if first_count.is_zero() || second_count.is_zero() || first_half.is_zero() {
            return Decimal::ZERO;
        }

        let avg_first = first_half / first_count;
        let avg_second = second_half / second_count;

        // 변화율을 -1.0 ~ 1.0 범위로 정규화
        let change_pct = (avg_second - avg_first) / avg_first * dec!(100);
        let clamped = change_pct.max(dec!(-10)).min(dec!(10));
        clamped / dec!(10)
    }

    /// 매집/이탈 판별.
    ///
    /// 거래량 패턴으로 기관 매집 또는 이탈을 감지합니다.
    ///
    /// # 반환
    ///
    /// -2 ~ 4 (2.0 이상=매집, -2.0 이하=이탈)
    fn calculate_vol_quality(candles: &[Kline]) -> Decimal {
        let len = candles.len().min(20);
        if len < 10 {
            return Decimal::ZERO;
        }

        let recent = &candles[candles.len() - len..];

        // 상승일 거래량 vs 하락일 거래량 비교
        let mut up_vol = Decimal::ZERO;
        let mut down_vol = Decimal::ZERO;

        for k in recent.iter() {
            if k.close > k.open {
                up_vol += k.volume;
            } else {
                down_vol += k.volume;
            }
        }

        // 거래량 비율을 -2 ~ 4 범위로 정규화
        if down_vol.is_zero() {
            return dec!(5);
        }
        let ratio = up_vol / down_vol;
        (ratio - Decimal::ONE).max(dec!(-2)).min(dec!(4))
    }

    /// 박스권 위치 계산.
    ///
    /// 현재 가격이 최근 범위의 어디에 위치하는지 측정합니다.
    ///
    /// # 반환
    ///
    /// 0.0 ~ 1.0 (0=하단, 1=상단)
    fn calculate_range_pos(candles: &[Kline]) -> Decimal {
        let len = candles.len().min(60);
        if len < 20 {
            return dec!(0.5);
        }

        let recent = &candles[candles.len() - len..];
        let current_price = match candles.last() {
            Some(k) => k.close,
            None => return dec!(0.5),
        };

        let high_60d = recent
            .iter()
            .map(|k| k.high)
            .max()
            .unwrap_or(Decimal::ZERO);
        let low_60d = recent
            .iter()
            .map(|k| k.low)
            .min()
            .unwrap_or(Decimal::ZERO);

        if high_60d == low_60d {
            return dec!(0.5);
        }

        ((current_price - low_60d) / (high_60d - low_60d))
            .max(Decimal::ZERO)
            .min(Decimal::ONE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_candles(count: usize) -> Vec<Kline> {
        let ticker = "TEST/USD".to_string();
        (0..count)
            .map(|i| {
                let now = chrono::Utc::now();
                Kline {
                    ticker: ticker.clone(),
                    timeframe: trader_core::types::Timeframe::D1,
                    open_time: now,
                    close_time: now,
                    open: dec!(100) + Decimal::from(i as i64),
                    high: dec!(105) + Decimal::from(i as i64),
                    low: dec!(95) + Decimal::from(i as i64),
                    close: dec!(102) + Decimal::from(i as i64),
                    volume: dec!(1000),
                    quote_volume: Some(dec!(0)),
                    num_trades: Some(0),
                }
            })
            .collect()
    }

    #[test]
    fn test_from_candles() {
        let engine = IndicatorEngine::new();
        let ticker = "TEST";
        let candles = create_test_candles(60);

        let result = StructuralFeaturesCalculator::from_candles(ticker, &candles, &engine);
        assert!(result.is_ok());

        let features = result.unwrap();
        assert_eq!(features.ticker, ticker);
        assert!(features.low_trend >= dec!(-1) && features.low_trend <= dec!(1));
        assert!(features.range_pos >= Decimal::ZERO && features.range_pos <= Decimal::ONE);
        assert!(features.rsi >= Decimal::ZERO && features.rsi <= dec!(100));
        assert!(features.bb_upper >= Decimal::ZERO);
        assert!(features.bb_middle >= Decimal::ZERO);
    }

    #[test]
    fn test_insufficient_data() {
        let engine = IndicatorEngine::new();
        let ticker = "TEST";
        let candles = create_test_candles(30); // 40개 미만

        let result = StructuralFeaturesCalculator::from_candles(ticker, &candles, &engine);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            IndicatorError::InsufficientData {
                required: 40,
                provided: 30,
            }
        ));
    }

    #[test]
    fn test_breakout_score_range() {
        let engine = IndicatorEngine::new();
        let candles = create_test_candles(60);

        let features =
            StructuralFeaturesCalculator::from_candles("TEST", &candles, &engine).unwrap();
        let score = features.breakout_score();

        assert!(score >= Decimal::ZERO && score <= dec!(100));
    }

    #[test]
    fn test_is_alive_consolidation() {
        // "살아있는 횡보" 조건 충족
        let features = StructuralFeatures {
            low_trend: dec!(0.3),
            vol_quality: dec!(0.2),
            bb_width: dec!(2.5),
            ..Default::default()
        };
        assert!(features.is_alive_consolidation());

        // 조건 미충족
        let features_dead = StructuralFeatures {
            low_trend: dec!(0.1), // < 0.2
            vol_quality: dec!(0.2),
            bb_width: dec!(2.5),
            ..Default::default()
        };
        assert!(!features_dead.is_alive_consolidation());
    }
}
