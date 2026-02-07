//! ML 모델을 위한 feature engineering.
//!
//! ML 모델 입력으로 사용하기 위해 Kline 데이터에서
//! 기술 지표와 파생 feature를 추출합니다.

use crate::ml::{FeatureVector, MlError, MlResult};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use trader_core::Kline;

/// feature 추출을 위한 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// 계산할 SMA 기간
    pub sma_periods: Vec<usize>,
    /// 계산할 EMA 기간
    pub ema_periods: Vec<usize>,
    /// RSI 기간
    pub rsi_period: usize,
    /// MACD 파라미터 (fast, slow, signal)
    pub macd_params: (usize, usize, usize),
    /// Bollinger Bands 기간
    pub bb_period: usize,
    /// Bollinger Bands 표준편차 승수
    pub bb_std_dev: f64,
    /// ATR 기간
    pub atr_period: usize,
    /// 포함할 수익률 기간 수
    pub return_periods: Vec<usize>,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            sma_periods: vec![5, 10, 20, 50],
            ema_periods: vec![12, 26],
            rsi_period: 14,
            macd_params: (12, 26, 9),
            bb_period: 20,
            bb_std_dev: 2.0,
            atr_period: 14,
            return_periods: vec![1, 5, 10],
        }
    }
}

impl FeatureConfig {
    /// feature 추출에 필요한 최소 kline 수 반환.
    pub fn min_klines_required(&self) -> usize {
        let max_sma = self.sma_periods.iter().max().copied().unwrap_or(0);
        let max_ema = self.ema_periods.iter().max().copied().unwrap_or(0);
        let max_return = self.return_periods.iter().max().copied().unwrap_or(0);

        *[
            max_sma,
            max_ema,
            self.rsi_period + 1,
            self.macd_params.1 + self.macd_params.2,
            self.bb_period,
            self.atr_period,
            max_return + 1,
        ]
        .iter()
        .max()
        .unwrap_or(&50)
    }

    /// 예상되는 feature vector 크기 반환.
    pub fn feature_count(&self) -> usize {
        // SMA 대비 가격 비율
        let sma_features = self.sma_periods.len();
        // EMA 대비 가격 비율
        let ema_features = self.ema_periods.len();
        // RSI (1)
        let rsi_features = 1;
        // MACD (histogram, signal line 비율) = 2
        let macd_features = 2;
        // Bollinger Bands (%B, bandwidth) = 2
        let bb_features = 2;
        // ATR 비율 (1)
        let atr_features = 1;
        // 수익률
        let return_features = self.return_periods.len();
        // 로그 수익률
        let log_return_features = self.return_periods.len();
        // 캔들 feature (몸통 비율, 윗꼬리, 아랫꼬리, 거래량 변화) = 4
        let candle_features = 4;

        sma_features
            + ema_features
            + rsi_features
            + macd_features
            + bb_features
            + atr_features
            + return_features
            + log_return_features
            + candle_features
    }
}

/// Kline 데이터를 ML feature vector로 변환하는 feature 추출기.
pub struct FeatureExtractor {
    config: FeatureConfig,
}

impl FeatureExtractor {
    /// 주어진 설정으로 새 feature 추출기 생성.
    pub fn new(config: FeatureConfig) -> Self {
        Self { config }
    }

    /// 기본 설정으로 feature 추출기 생성.
    pub fn with_defaults() -> Self {
        Self::new(FeatureConfig::default())
    }

    /// 설정 반환.
    pub fn config(&self) -> &FeatureConfig {
        &self.config
    }

    /// Kline 슬라이스에서 feature 추출.
    ///
    /// ML 모델 입력에 적합한 FeatureVector 반환.
    /// kline은 오래된 것부터 최신 순으로 정렬되어야 함.
    pub fn extract(&self, klines: &[Kline]) -> MlResult<FeatureVector> {
        let min_required = self.config.min_klines_required();
        if klines.len() < min_required {
            return Err(MlError::InsufficientData {
                required: min_required,
                actual: klines.len(),
            });
        }

        // 계산을 위해 f64 배열로 변환
        let closes: Vec<f64> = klines
            .iter()
            .map(|k| k.close.to_f64().unwrap_or(0.0))
            .collect();
        let highs: Vec<f64> = klines
            .iter()
            .map(|k| k.high.to_f64().unwrap_or(0.0))
            .collect();
        let lows: Vec<f64> = klines
            .iter()
            .map(|k| k.low.to_f64().unwrap_or(0.0))
            .collect();
        let opens: Vec<f64> = klines
            .iter()
            .map(|k| k.open.to_f64().unwrap_or(0.0))
            .collect();
        let volumes: Vec<f64> = klines
            .iter()
            .map(|k| k.volume.to_f64().unwrap_or(0.0))
            .collect();

        let current_close = *closes.last().unwrap();
        let current_high = *highs.last().unwrap();
        let current_low = *lows.last().unwrap();
        let current_open = *opens.last().unwrap();

        let mut features = Vec::with_capacity(self.config.feature_count());
        let mut names = Vec::with_capacity(self.config.feature_count());

        // 1. SMA 비율 (price / SMA - 1)
        for period in &self.config.sma_periods {
            let sma = self.calculate_sma(&closes, *period);
            let ratio = if sma > 0.0 {
                (current_close / sma) - 1.0
            } else {
                0.0
            };
            features.push(ratio as f32);
            names.push(format!("sma_{}_ratio", period));
        }

        // 2. EMA 비율
        for period in &self.config.ema_periods {
            let ema = self.calculate_ema(&closes, *period);
            let ratio = if ema > 0.0 {
                (current_close / ema) - 1.0
            } else {
                0.0
            };
            features.push(ratio as f32);
            names.push(format!("ema_{}_ratio", period));
        }

        // 3. RSI (0-1로 정규화)
        let rsi = self.calculate_rsi(&closes, self.config.rsi_period);
        features.push((rsi / 100.0) as f32);
        names.push("rsi".to_string());

        // 4. MACD feature
        let (macd_hist, macd_signal_ratio) = self.calculate_macd(
            &closes,
            self.config.macd_params.0,
            self.config.macd_params.1,
            self.config.macd_params.2,
        );
        features.push(macd_hist as f32);
        names.push("macd_histogram".to_string());
        features.push(macd_signal_ratio as f32);
        names.push("macd_signal_ratio".to_string());

        // 5. Bollinger Bands feature
        let (bb_percent_b, bb_bandwidth) =
            self.calculate_bollinger(&closes, self.config.bb_period, self.config.bb_std_dev);
        features.push(bb_percent_b as f32);
        names.push("bb_percent_b".to_string());
        features.push(bb_bandwidth as f32);
        names.push("bb_bandwidth".to_string());

        // 6. ATR 비율
        let atr = self.calculate_atr(&highs, &lows, &closes, self.config.atr_period);
        let atr_ratio = if current_close > 0.0 {
            atr / current_close
        } else {
            0.0
        };
        features.push(atr_ratio as f32);
        names.push("atr_ratio".to_string());

        // 7. 수익률
        for period in &self.config.return_periods {
            let ret = self.calculate_return(&closes, *period);
            features.push(ret as f32);
            names.push(format!("return_{}", period));
        }

        // 8. 로그 수익률
        for period in &self.config.return_periods {
            let log_ret = self.calculate_log_return(&closes, *period);
            features.push(log_ret as f32);
            names.push(format!("log_return_{}", period));
        }

        // 9. 캔들 feature
        let range = current_high - current_low;
        let body = (current_close - current_open).abs();

        // 몸통 비율 (body / range)
        let body_ratio = if range > 0.0 { body / range } else { 0.0 };
        features.push(body_ratio as f32);
        names.push("body_ratio".to_string());

        // 윗꼬리 비율
        let upper_shadow = if current_close > current_open {
            current_high - current_close
        } else {
            current_high - current_open
        };
        let upper_shadow_ratio = if range > 0.0 {
            upper_shadow / range
        } else {
            0.0
        };
        features.push(upper_shadow_ratio as f32);
        names.push("upper_shadow_ratio".to_string());

        // 아랫꼬리 비율
        let lower_shadow = if current_close > current_open {
            current_open - current_low
        } else {
            current_close - current_low
        };
        let lower_shadow_ratio = if range > 0.0 {
            lower_shadow / range
        } else {
            0.0
        };
        features.push(lower_shadow_ratio as f32);
        names.push("lower_shadow_ratio".to_string());

        // 거래량 변화 비율
        let prev_volume = if volumes.len() >= 2 {
            volumes[volumes.len() - 2]
        } else {
            volumes[0]
        };
        let current_volume = *volumes.last().unwrap();
        let volume_change = if prev_volume > 0.0 {
            (current_volume / prev_volume) - 1.0
        } else {
            0.0
        };
        features.push(volume_change.clamp(-2.0, 2.0) as f32);
        names.push("volume_change".to_string());

        Ok(FeatureVector::with_names(features, names))
    }

    /// feature를 추출하고 표준화.
    pub fn extract_standardized(&self, klines: &[Kline]) -> MlResult<FeatureVector> {
        let mut features = self.extract(klines)?;
        features.standardize();
        Ok(features)
    }

    // === 비공개 계산 메서드 ===

    fn calculate_sma(&self, data: &[f64], period: usize) -> f64 {
        if data.len() < period || period == 0 {
            return 0.0;
        }
        let start = data.len() - period;
        data[start..].iter().sum::<f64>() / period as f64
    }

    fn calculate_ema(&self, data: &[f64], period: usize) -> f64 {
        if data.is_empty() || period == 0 {
            return 0.0;
        }

        let multiplier = 2.0 / (period as f64 + 1.0);
        let mut ema = data[0];

        for value in data.iter().skip(1) {
            ema = (value - ema) * multiplier + ema;
        }

        ema
    }

    fn calculate_rsi(&self, closes: &[f64], period: usize) -> f64 {
        if closes.len() < period + 1 {
            return 50.0; // Neutral
        }

        let mut gains = Vec::new();
        let mut losses = Vec::new();

        for i in 1..closes.len() {
            let change = closes[i] - closes[i - 1];
            if change > 0.0 {
                gains.push(change);
                losses.push(0.0);
            } else {
                gains.push(0.0);
                losses.push(change.abs());
            }
        }

        // 마지막 `period` 개의 값 사용
        let start = if gains.len() > period {
            gains.len() - period
        } else {
            0
        };

        let avg_gain: f64 = gains[start..].iter().sum::<f64>() / period as f64;
        let avg_loss: f64 = losses[start..].iter().sum::<f64>() / period as f64;

        if avg_loss == 0.0 {
            return 100.0;
        }

        let rs = avg_gain / avg_loss;
        100.0 - (100.0 / (1.0 + rs))
    }

    fn calculate_macd(
        &self,
        closes: &[f64],
        fast_period: usize,
        slow_period: usize,
        signal_period: usize,
    ) -> (f64, f64) {
        let fast_ema = self.calculate_ema(closes, fast_period);
        let slow_ema = self.calculate_ema(closes, slow_period);
        let macd_line = fast_ema - slow_ema;

        // signal을 위해 MACD 라인 히스토리 계산
        let mut macd_history = Vec::new();
        for i in slow_period..=closes.len() {
            let fast = self.calculate_ema(&closes[..i], fast_period);
            let slow = self.calculate_ema(&closes[..i], slow_period);
            macd_history.push(fast - slow);
        }

        let signal_line = if macd_history.len() >= signal_period {
            self.calculate_ema(&macd_history, signal_period)
        } else {
            macd_line
        };

        let histogram = macd_line - signal_line;

        // 가격 대비 histogram 정규화
        let current_price = *closes.last().unwrap_or(&1.0);
        let norm_histogram = if current_price > 0.0 {
            histogram / current_price * 100.0
        } else {
            0.0
        };

        // Signal 비율
        let signal_ratio = if signal_line.abs() > 0.0001 {
            (macd_line / signal_line) - 1.0
        } else {
            0.0
        };

        (norm_histogram, signal_ratio.clamp(-1.0, 1.0))
    }

    fn calculate_bollinger(&self, closes: &[f64], period: usize, std_dev_mult: f64) -> (f64, f64) {
        if closes.len() < period {
            return (0.5, 0.0); // Neutral %B, zero bandwidth
        }

        let start = closes.len() - period;
        let window = &closes[start..];

        let sma: f64 = window.iter().sum::<f64>() / period as f64;
        let variance: f64 = window.iter().map(|x| (x - sma).powi(2)).sum::<f64>() / period as f64;
        let std_dev = variance.sqrt();

        let upper_band = sma + std_dev_mult * std_dev;
        let lower_band = sma - std_dev_mult * std_dev;
        let bandwidth = upper_band - lower_band;

        let current_price = *closes.last().unwrap();

        // %B: 밴드 대비 가격 위치 (0 = 하단, 1 = 상단)
        let percent_b = if bandwidth > 0.0 {
            (current_price - lower_band) / bandwidth
        } else {
            0.5
        };

        // 중간 밴드 대비 bandwidth 퍼센트
        let bandwidth_pct = if sma > 0.0 { bandwidth / sma } else { 0.0 };

        (percent_b.clamp(0.0, 1.0), bandwidth_pct)
    }

    fn calculate_atr(&self, highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> f64 {
        if highs.len() < period + 1 || highs.len() != lows.len() || highs.len() != closes.len() {
            return 0.0;
        }

        let mut true_ranges = Vec::new();

        for i in 1..highs.len() {
            let high_low = highs[i] - lows[i];
            let high_close = (highs[i] - closes[i - 1]).abs();
            let low_close = (lows[i] - closes[i - 1]).abs();

            let tr = high_low.max(high_close).max(low_close);
            true_ranges.push(tr);
        }

        // 마지막 `period` 개의 true range 평균
        if true_ranges.len() < period {
            return true_ranges.iter().sum::<f64>() / true_ranges.len().max(1) as f64;
        }

        let start = true_ranges.len() - period;
        true_ranges[start..].iter().sum::<f64>() / period as f64
    }

    fn calculate_return(&self, closes: &[f64], period: usize) -> f64 {
        if closes.len() <= period {
            return 0.0;
        }

        let current = closes[closes.len() - 1];
        let past = closes[closes.len() - 1 - period];

        if past > 0.0 {
            (current / past) - 1.0
        } else {
            0.0
        }
    }

    fn calculate_log_return(&self, closes: &[f64], period: usize) -> f64 {
        if closes.len() <= period {
            return 0.0;
        }

        let current = closes[closes.len() - 1];
        let past = closes[closes.len() - 1 - period];

        if past > 0.0 && current > 0.0 {
            (current / past).ln()
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use trader_core::Timeframe;

    fn create_test_klines(count: usize) -> Vec<Kline> {
        let ticker = "BTC/USDT".to_string();
        let base_price = 50000.0;

        (0..count)
            .map(|i| {
                // 약간의 가격 변동 생성
                let variation = (i as f64 * 0.1).sin() * 1000.0;
                let open = base_price + variation;
                let close = open + (i as f64 % 3.0 - 1.0) * 100.0;
                let high = open.max(close) + 50.0;
                let low = open.min(close) - 50.0;

                Kline::new(
                    ticker.clone(),
                    Timeframe::H1,
                    Utc::now(),
                    rust_decimal::Decimal::from_f64_retain(open).unwrap_or(dec!(50000)),
                    rust_decimal::Decimal::from_f64_retain(high).unwrap_or(dec!(50050)),
                    rust_decimal::Decimal::from_f64_retain(low).unwrap_or(dec!(49950)),
                    rust_decimal::Decimal::from_f64_retain(close).unwrap_or(dec!(50000)),
                    dec!(100) + rust_decimal::Decimal::from(i as u32),
                    Utc::now(),
                )
            })
            .collect()
    }

    #[test]
    fn test_feature_config_default() {
        let config = FeatureConfig::default();
        assert_eq!(config.sma_periods, vec![5, 10, 20, 50]);
        assert_eq!(config.rsi_period, 14);
        assert!(config.min_klines_required() >= 50);
    }

    #[test]
    fn test_feature_extraction() {
        let config = FeatureConfig::default();
        let extractor = FeatureExtractor::new(config.clone());
        let klines = create_test_klines(100);

        let features = extractor.extract(&klines).unwrap();

        assert_eq!(features.len(), config.feature_count());
        assert!(!features.is_empty());

        // feature 이름 존재 확인
        assert!(features.names().is_some());
        let names = features.names().unwrap();
        assert!(names.contains(&"rsi".to_string()));
        assert!(names.contains(&"macd_histogram".to_string()));
    }

    #[test]
    fn test_insufficient_data() {
        let extractor = FeatureExtractor::with_defaults();
        let klines = create_test_klines(10); // 너무 적음

        let result = extractor.extract(&klines);
        assert!(result.is_err());

        match result {
            Err(MlError::InsufficientData { required, actual }) => {
                assert!(required > actual);
                assert_eq!(actual, 10);
            }
            _ => panic!("Expected InsufficientData error"),
        }
    }

    #[test]
    fn test_sma_calculation() {
        let extractor = FeatureExtractor::with_defaults();
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        let sma = extractor.calculate_sma(&data, 5);
        assert!((sma - 3.0).abs() < 0.001);

        let sma_3 = extractor.calculate_sma(&data, 3);
        assert!((sma_3 - 4.0).abs() < 0.001); // (3+4+5)/3 = 4
    }

    #[test]
    fn test_rsi_calculation() {
        let extractor = FeatureExtractor::with_defaults();

        // 모두 상승
        let up_data: Vec<f64> = (0..20).map(|i| 100.0 + i as f64).collect();
        let rsi_up = extractor.calculate_rsi(&up_data, 14);
        assert!(rsi_up > 90.0);

        // 모두 하락
        let down_data: Vec<f64> = (0..20).map(|i| 100.0 - i as f64).collect();
        let rsi_down = extractor.calculate_rsi(&down_data, 14);
        assert!(rsi_down < 10.0);
    }

    #[test]
    fn test_feature_standardization() {
        let extractor = FeatureExtractor::with_defaults();
        let klines = create_test_klines(100);

        let features = extractor.extract_standardized(&klines).unwrap();

        // 표준화 후 평균은 0에 가까워야 함
        let values = features.as_slice();
        let mean: f32 = values.iter().sum::<f32>() / values.len() as f32;
        assert!(mean.abs() < 0.1, "Mean should be close to 0, got {}", mean);
    }

    #[test]
    fn test_bollinger_bands() {
        let extractor = FeatureExtractor::with_defaults();
        let data: Vec<f64> = (0..30)
            .map(|i| 100.0 + (i as f64 * 0.5).sin() * 5.0)
            .collect();

        let (percent_b, bandwidth) = extractor.calculate_bollinger(&data, 20, 2.0);

        assert!(percent_b >= 0.0 && percent_b <= 1.0);
        assert!(bandwidth >= 0.0);
    }

    #[test]
    fn test_atr_calculation() {
        let extractor = FeatureExtractor::with_defaults();

        let highs: Vec<f64> = (0..20).map(|i| 105.0 + i as f64).collect();
        let lows: Vec<f64> = (0..20).map(|i| 95.0 + i as f64).collect();
        let closes: Vec<f64> = (0..20).map(|i| 100.0 + i as f64).collect();

        let atr = extractor.calculate_atr(&highs, &lows, &closes, 14);
        assert!(atr > 0.0);
    }
}
