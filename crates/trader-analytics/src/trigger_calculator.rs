//! 진입 트리거 계산기.
//!
//! 캔들 데이터를 분석하여 진입 신호를 감지하고 TriggerResult를 생성합니다.
//! Phase 1-B.2 구현.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use thiserror::Error;
use trader_core::{Kline, TriggerResult, TriggerType};

use crate::indicators::{IndicatorEngine, IndicatorError};

#[cfg(feature = "ml")]
use crate::ml::pattern::{CandlestickPatternType, PatternRecognizer};

/// 트리거 계산 오류.
#[derive(Debug, Error)]
pub enum TriggerError {
    /// 데이터 부족
    #[error("데이터가 부족합니다: 필요 {required}개, 제공 {provided}개")]
    InsufficientData { required: usize, provided: usize },

    /// 지표 계산 오류
    #[error("지표 계산 실패: {0}")]
    IndicatorError(#[from] IndicatorError),

    /// 계산 오류
    #[error("계산 오류: {0}")]
    CalculationError(String),
}

/// 트리거 계산 결과 타입.
pub type TriggerCalculatorResult<T> = Result<T, TriggerError>;

/// 트리거 계산기.
///
/// 여러 기술적 조건을 분석하여 진입 트리거를 감지합니다.
pub struct TriggerCalculator {
    indicator_engine: IndicatorEngine,
    #[cfg(feature = "ml")]
    pattern_recognizer: PatternRecognizer,
}

impl TriggerCalculator {
    /// 새로운 트리거 계산기 생성.
    pub fn new() -> Self {
        Self {
            indicator_engine: IndicatorEngine::new(),
            #[cfg(feature = "ml")]
            pattern_recognizer: PatternRecognizer::with_defaults(),
        }
    }

    /// 캔들 데이터로부터 트리거 결과 계산.
    ///
    /// # 인자
    /// * `klines` - 캔들 데이터 (최소 50개 권장)
    ///
    /// # 반환
    /// TriggerResult - 감지된 트리거와 점수
    ///
    /// # 오류
    /// - InsufficientData: 캔들 데이터 부족
    /// - IndicatorError: 지표 계산 실패
    pub fn calculate(&self, klines: &[Kline]) -> TriggerCalculatorResult<TriggerResult> {
        const MIN_CANDLES: usize = 30;

        if klines.len() < MIN_CANDLES {
            return Err(TriggerError::InsufficientData {
                required: MIN_CANDLES,
                provided: klines.len(),
            });
        }

        let mut triggers = Vec::new();

        // 1. 캔들 패턴 감지
        if let Some(pattern_trigger) = self.detect_candle_patterns(klines) {
            triggers.push(pattern_trigger);
        }

        // 2. 거래량 폭증 감지
        if self.detect_volume_spike(klines)? {
            triggers.push(TriggerType::VolumeSpike);
        }

        // 3. 박스권 돌파 감지
        if self.detect_box_breakout(klines)? {
            triggers.push(TriggerType::BoxBreakout);
        }

        // 4. 모멘텀 상승 감지
        if self.detect_momentum_up(klines)? {
            triggers.push(TriggerType::MomentumUp);
        }

        // 5. TTM Squeeze 해제 감지 (향후 구현)
        // if self.detect_squeeze_break(klines)? {
        //     triggers.push(TriggerType::SqueezeBreak);
        // }

        Ok(TriggerResult::new(triggers))
    }

    /// 캔들 패턴 감지.
    ///
    /// Hammer와 Engulfing 패턴을 감지합니다.
    /// ml feature가 활성화되지 않으면 None을 반환합니다.
    fn detect_candle_patterns(&self, _klines: &[Kline]) -> Option<TriggerType> {
        #[cfg(feature = "ml")]
        {
            let patterns = self.pattern_recognizer.detect_candlestick_patterns(_klines);

            if patterns.is_empty() {
                return None;
            }

            // 최근 3개 캔들 내의 패턴만 유효
            let recent_patterns: Vec<_> = patterns
                .iter()
                .filter(|p| p.end_index + 3 >= _klines.len())
                .collect();

            if recent_patterns.is_empty() {
                return None;
            }

            // Engulfing 패턴 우선 (더 강한 신호)
            for pattern in &recent_patterns {
                if pattern.bullish
                    && matches!(
                        pattern.pattern_type,
                        CandlestickPatternType::BullishEngulfing
                    )
                    && pattern.confidence >= 0.7
                {
                    return Some(TriggerType::Engulfing);
                }
            }

            // Hammer 패턴
            for pattern in &recent_patterns {
                if pattern.bullish
                    && matches!(pattern.pattern_type, CandlestickPatternType::Hammer)
                    && pattern.confidence >= 0.6
                {
                    return Some(TriggerType::HammerCandle);
                }
            }

            None
        }
        #[cfg(not(feature = "ml"))]
        {
            None
        }
    }

    /// 거래량 폭증 감지.
    ///
    /// 최근 거래량이 평균 거래량 대비 2배 이상인 경우 true 반환.
    fn detect_volume_spike(&self, klines: &[Kline]) -> TriggerCalculatorResult<bool> {
        if klines.len() < 20 {
            return Ok(false);
        }

        // 최근 20일 평균 거래량 계산
        let avg_volume = klines[klines.len() - 20..]
            .iter()
            .map(|k| k.volume)
            .sum::<Decimal>()
            / Decimal::from(20);

        if avg_volume.is_zero() {
            return Ok(false);
        }

        // 현재 거래량
        let current_volume = klines.last().unwrap().volume;

        // 2배 이상이면 폭증
        Ok(current_volume >= avg_volume * dec!(2.0))
    }

    /// 박스권 돌파 감지.
    ///
    /// 최근 20일간 횡보하다가 저항선을 돌파한 경우 true 반환.
    fn detect_box_breakout(&self, klines: &[Kline]) -> TriggerCalculatorResult<bool> {
        const LOOKBACK: usize = 20;

        if klines.len() < LOOKBACK + 5 {
            return Ok(false);
        }

        let recent = &klines[klines.len() - LOOKBACK..];
        let current = klines.last().unwrap();

        // 최고가와 최저가 찾기
        let high_prices: Vec<Decimal> = recent.iter().map(|k| k.high).collect();
        let low_prices: Vec<Decimal> = recent.iter().map(|k| k.low).collect();

        let max_high = high_prices.iter().max().copied().unwrap_or(Decimal::ZERO);
        let min_low = low_prices.iter().min().copied().unwrap_or(Decimal::ZERO);

        if max_high.is_zero() || min_low.is_zero() {
            return Ok(false);
        }

        // 박스권 범위 계산 (고가-저가 차이)
        let range = max_high - min_low;
        let range_ratio = range / max_high;

        // 박스권 조건: 범위가 평균 가격의 15% 이내
        if range_ratio > dec!(0.15) {
            return Ok(false);
        }

        // 저항선 돌파 조건: 현재 종가가 최고가를 2% 이상 상회
        let breakout_threshold = max_high * dec!(1.02);
        Ok(current.close >= breakout_threshold && current.is_bullish())
    }

    /// 모멘텀 상승 감지.
    ///
    /// RSI가 50 이상이고 상승 중이며, 가격이 MA20 위에 있는 경우 true 반환.
    fn detect_momentum_up(&self, klines: &[Kline]) -> TriggerCalculatorResult<bool> {
        if klines.len() < 20 {
            return Ok(false);
        }

        let closes: Vec<Decimal> = klines.iter().map(|k| k.close).collect();

        // RSI 계산
        let rsi_values = self
            .indicator_engine
            .rsi(&closes, crate::indicators::RsiParams { period: 14 })?;

        // RSI가 50 이상이고 상승 중인지 확인
        let recent_rsi: Vec<Decimal> = rsi_values.iter().rev().take(3).filter_map(|&x| x).collect();

        if recent_rsi.len() < 3 {
            return Ok(false);
        }

        let current_rsi = recent_rsi[0];
        let prev_rsi = recent_rsi[1];

        let rsi_rising = current_rsi > prev_rsi && current_rsi >= dec!(50);

        // MA20 계산
        let sma20_values = self
            .indicator_engine
            .sma(&closes, crate::indicators::SmaParams { period: 20 })?;

        let current_price = klines.last().unwrap().close;
        let ma20 = sma20_values
            .last()
            .and_then(|&x| x)
            .unwrap_or(Decimal::ZERO);

        if ma20.is_zero() {
            return Ok(false);
        }

        // 가격이 MA20 위에 있는지 확인
        let above_ma20 = current_price > ma20;

        Ok(rsi_rising && above_ma20)
    }

    /// TTM Squeeze 해제 감지 (향후 구현 예정).
    ///
    /// Bollinger Bands가 Keltner Channel 밖으로 확장되는 시점을 감지합니다.
    /// Phase 1-B.3에서 구현 예정.
    #[allow(dead_code)]
    fn detect_squeeze_break(&self, _klines: &[Kline]) -> TriggerCalculatorResult<bool> {
        // TODO: Phase 1-B.3에서 구현
        Ok(false)
    }
}

impl Default for TriggerCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use rust_decimal_macros::dec;
    use trader_core::{Symbol, Timeframe};

    fn create_test_kline(
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
        volume: Decimal,
        index: i64,
    ) -> Kline {
        let time =
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap() + chrono::Duration::days(index);
        Kline {
            ticker: "BTC/USDT".to_string(),
            timeframe: Timeframe::D1,
            open_time: time,
            open,
            high,
            low,
            close,
            volume,
            close_time: time + chrono::Duration::days(1),
            quote_volume: None,
            num_trades: None,
        }
    }

    fn create_uptrend_klines(count: usize) -> Vec<Kline> {
        (0..count)
            .map(|i| {
                let base = dec!(100) + Decimal::from(i as i64);
                create_test_kline(
                    base,
                    base + dec!(2),
                    base - dec!(1),
                    base + dec!(1),
                    dec!(1000),
                    i as i64,
                )
            })
            .collect()
    }

    #[test]
    fn test_insufficient_data() {
        let calculator = TriggerCalculator::new();
        let klines = create_uptrend_klines(10);

        let result = calculator.calculate(&klines);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TriggerError::InsufficientData { .. }
        ));
    }

    #[test]
    fn test_volume_spike_detection() {
        let calculator = TriggerCalculator::new();

        // 평균 거래량 1000, 마지막 2500 (2.5배)
        let mut klines = (0..30)
            .map(|i| create_test_kline(dec!(100), dec!(105), dec!(95), dec!(100), dec!(1000), i))
            .collect::<Vec<_>>();

        // 마지막 캔들 거래량 폭증
        klines.push(create_test_kline(
            dec!(100),
            dec!(105),
            dec!(95),
            dec!(103),
            dec!(2500),
            30,
        ));

        let result = calculator.detect_volume_spike(&klines).unwrap();
        assert!(result);
    }

    #[test]
    fn test_box_breakout_detection() {
        let calculator = TriggerCalculator::new();

        // 20일간 100~105 박스권
        let mut klines = (0..20)
            .map(|i| create_test_kline(dec!(100), dec!(105), dec!(100), dec!(102), dec!(1000), i))
            .collect::<Vec<_>>();

        // 추가 데이터 (필요한 최소 개수 맞추기)
        for i in 20..25 {
            klines.push(create_test_kline(
                dec!(100),
                dec!(105),
                dec!(100),
                dec!(102),
                dec!(1000),
                i,
            ));
        }

        // 돌파 캔들 (105 * 1.02 = 107.1)
        klines.push(create_test_kline(
            dec!(105),
            dec!(108),
            dec!(104),
            dec!(108),
            dec!(2000),
            25,
        ));

        let result = calculator.detect_box_breakout(&klines).unwrap();
        assert!(result);
    }

    #[test]
    fn test_momentum_up_detection() {
        let calculator = TriggerCalculator::new();

        // RSI 상승 + MA20 위 데이터 생성
        let mut klines = Vec::new();

        // 하락 후 반등 패턴 (RSI가 30 → 60으로 상승)
        for i in 0..10 {
            let price = dec!(100) - Decimal::from(i as i64);
            klines.push(create_test_kline(
                price,
                price + dec!(1),
                price - dec!(1),
                price,
                dec!(1000),
                i,
            ));
        }

        // 반등 시작
        for i in 10..30 {
            let price = dec!(90) + Decimal::from(i as i64 - 10) * dec!(2);
            klines.push(create_test_kline(
                price,
                price + dec!(2),
                price - dec!(1),
                price + dec!(1.5),
                dec!(1000),
                i,
            ));
        }

        let result = calculator.detect_momentum_up(&klines).unwrap();
        assert!(result);
    }

    #[test]
    fn test_calculate_with_multiple_triggers() {
        let calculator = TriggerCalculator::new();

        // 거래량 폭증 + 모멘텀 상승 조건 생성
        let mut klines = Vec::new();

        for i in 0..30 {
            let price = dec!(100) + Decimal::from(i as i64);
            klines.push(create_test_kline(
                price,
                price + dec!(2),
                price - dec!(1),
                price + dec!(1),
                dec!(1000),
                i,
            ));
        }

        // 마지막 캔들: 거래량 폭증
        klines.push(create_test_kline(
            dec!(130),
            dec!(135),
            dec!(129),
            dec!(133),
            dec!(3000),
            30,
        ));

        let result = calculator.calculate(&klines).unwrap();

        // 최소 1개 이상의 트리거 발생 예상
        assert!(result.score > 0.0);
        assert!(!result.triggers.is_empty());
    }

    #[test]
    fn test_no_triggers() {
        let calculator = TriggerCalculator::new();

        // 평범한 횡보 패턴 (트리거 없음)
        let klines = (0..50)
            .map(|i| create_test_kline(dec!(100), dec!(102), dec!(98), dec!(100), dec!(1000), i))
            .collect::<Vec<_>>();

        let result = calculator.calculate(&klines).unwrap();

        // 트리거가 없거나 매우 적을 것으로 예상
        assert!(result.score < 30.0);
    }

    #[test]
    fn test_calculator_default() {
        let calc1 = TriggerCalculator::new();
        let calc2 = TriggerCalculator::default();

        let klines = create_uptrend_klines(50);

        let result1 = calc1.calculate(&klines);
        let result2 = calc2.calculate(&klines);

        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }
}
