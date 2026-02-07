//! RouteState 계산 로직.
//!
//! StructuralFeatures와 기술적 지표를 기반으로 종목의 매매 단계(RouteState)를 판정합니다.
//!
//! # 동적 라우트 태깅 (Dynamic Route Tagging)
//!
//! 고정 임계값 대신 현재 시장 데이터의 **퍼센타일 분포**를 기반으로 동적 임계값을 계산합니다.
//! 이를 통해 시장 상황에 적응하는 유연한 판정이 가능합니다.
//!
//! ## 동적 임계값 기준
//!
//! - `r5_q75`: 5일 수익률 상위 25% (75번째 퍼센타일)
//! - `slope_q60`: MACD slope 상위 40% (60번째 퍼센타일)
//! - `vol_quality_q60`: 거래량 품질 상위 40% (60번째 퍼센타일)
//! - `range_pos_q75`: 박스권 위치 상위 25% (75번째 퍼센타일)
//!
//! # 예시
//!
//! ```rust,ignore
//! use trader_analytics::route_state_calculator::{RouteStateCalculator, DynamicThresholds};
//!
//! let calculator = RouteStateCalculator::new();
//!
//! // 전체 종목 데이터로 동적 임계값 계산
//! let all_data: Vec<SymbolData> = /* ... */;
//! let thresholds = DynamicThresholds::compute(&all_data);
//!
//! // 동적 임계값 기반 라우트 판정
//! let state = calculator.calculate_dynamic(&candles, &thresholds)?;
//! ```

use crate::indicators::{IndicatorEngine, IndicatorError, IndicatorResult};
use crate::StructuralFeaturesCalculator;
use rust_decimal::prelude::ToPrimitive;
use trader_core::domain::StructuralFeatures;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use trader_core::{Kline, RouteState};

/// RouteState 계산기.
///
/// StructuralFeatures와 추가 지표를 조합하여 종목의 현재 매매 단계를 판정합니다.
pub struct RouteStateCalculator {
    engine: IndicatorEngine,
}

impl RouteStateCalculator {
    /// 새로운 계산기 인스턴스 생성.
    pub fn new() -> Self {
        Self {
            engine: IndicatorEngine::new(),
        }
    }

    /// 캔들 데이터로부터 RouteState 계산.
    ///
    /// # 인자
    ///
    /// * `candles` - 최소 40개의 캔들 데이터
    ///
    /// # 반환
    ///
    /// 계산된 RouteState
    ///
    /// # 우선순위
    ///
    /// 1. Overheat (위험 신호 최우선)
    /// 2. Attack (진입 기회)
    /// 3. Armed (대기 준비)
    /// 4. Wait (관찰)
    /// 5. Neutral (기본값)
    pub fn calculate(&self, candles: &[Kline]) -> IndicatorResult<RouteState> {
        if candles.len() < 40 {
            return Err(IndicatorError::InsufficientData {
                required: 40,
                provided: candles.len(),
            });
        }

        // StructuralFeatures 계산
        let ticker = candles.first().map(|k| k.ticker.as_str()).unwrap_or("");
        let features = StructuralFeaturesCalculator::from_candles(ticker, candles, &self.engine)?;

        // 5일 수익률 계산
        let return_5d = self.calculate_return_5d(candles)?;

        // 우선순위에 따라 상태 판정
        // 1. Overheat (가장 높은 우선순위)
        if self.is_overheat(&features, return_5d) {
            return Ok(RouteState::Overheat);
        }

        // 2. Attack
        // Note: TTM Squeeze는 Phase 1.2.x에서 구현 예정
        // 현재는 StructuralFeatures만으로 판정
        if self.is_attack(&features) {
            return Ok(RouteState::Attack);
        }

        // 3. Armed
        if self.is_armed(&features) {
            return Ok(RouteState::Armed);
        }

        // 4. Wait
        if self.is_wait(&features) {
            return Ok(RouteState::Wait);
        }

        // 5. Neutral (기본값)
        Ok(RouteState::Neutral)
    }

    /// Overheat 판정.
    ///
    /// **조건**:
    /// - 5일 수익률 > 20% 또는
    /// - RSI >= 75
    fn is_overheat(&self, features: &StructuralFeatures, return_5d: f64) -> bool {
        return_5d > 20.0 || features.rsi >= Decimal::new(75, 0)
    }

    /// Attack 판정.
    ///
    /// **조건**:
    /// - RSI 45~65 (건강한 범위)
    /// - Range_Pos >= 0.8 (박스권 상단)
    /// - Low_Trend > 0 (저가 상승)
    /// - Vol_Quality > 0 (매집 신호)
    ///
    /// Note: TTM Squeeze 조건은 Phase 1.2.x에서 추가 예정
    fn is_attack(&self, features: &StructuralFeatures) -> bool {
        features.rsi >= Decimal::new(45, 0)
            && features.rsi <= Decimal::new(65, 0)
            && features.range_pos >= Decimal::new(8, 1)
            && features.low_trend > Decimal::ZERO
            && features.vol_quality > Decimal::ZERO
    }

    /// Armed 판정.
    ///
    /// **조건**:
    /// - Vol_Quality >= 0.2 (강한 매집) 또는
    /// - Dist_MA20 > -2.0 (MA20 근처 또는 위)
    ///
    /// Note: TTM Squeeze 조건은 Phase 1.2.x에서 추가 예정
    fn is_armed(&self, features: &StructuralFeatures) -> bool {
        features.vol_quality >= Decimal::new(2, 1) || features.dist_ma20 > Decimal::new(-2, 0)
    }

    /// Wait 판정.
    ///
    /// **조건**:
    /// - Low_Trend > 0 (저가 상승 = 정배열 유지)
    /// - Dist_MA20 > -5.0 (MA 지지)
    fn is_wait(&self, features: &StructuralFeatures) -> bool {
        features.low_trend > Decimal::ZERO && features.dist_ma20 > Decimal::new(-5, 0)
    }

    /// 5일 수익률 계산 (%).
    ///
    /// (최근 종가 / 5일 전 종가 - 1) * 100
    fn calculate_return_5d(&self, candles: &[Kline]) -> IndicatorResult<f64> {
        let len = candles.len();
        if len < 6 {
            return Err(IndicatorError::InsufficientData {
                required: 6,
                provided: len,
            });
        }

        let current = candles[len - 1].close;
        let past = candles[len - 6].close;

        if past == Decimal::ZERO {
            return Ok(0.0);
        }

        let return_pct = ((current / past - Decimal::ONE) * Decimal::from(100))
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.0);

        Ok(return_pct)
    }
}

impl Default for RouteStateCalculator {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// 동적 라우트 태깅 (Dynamic Route Tagging)
// =============================================================================

/// 종목별 분석 데이터 (동적 임계값 계산용).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolData {
    /// 종목 티커
    pub ticker: String,
    /// 5일 수익률 (%)
    pub return_5d: f64,
    /// MACD slope
    pub macd_slope: f64,
    /// 거래량 품질 (vol_quality)
    pub vol_quality: f64,
    /// 박스권 위치 (range_pos)
    pub range_pos: f64,
    /// RSI
    pub rsi: f64,
    /// MA20 이격도
    pub dist_ma20: f64,
    /// 저점 추세
    pub low_trend: f64,
    /// TTM Squeeze 상태
    pub ttm_squeeze: bool,
}

/// 동적 임계값.
///
/// 전체 종목 데이터의 퍼센타일 분포를 기반으로 계산된 동적 임계값입니다.
/// 시장 상황에 따라 자동으로 조정됩니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicThresholds {
    /// 5일 수익률 75번째 퍼센타일 (상위 25%)
    pub r5_q75: f64,
    /// 5일 수익률 25번째 퍼센타일 (하위 25%)
    pub r5_q25: f64,
    /// MACD slope 60번째 퍼센타일 (상위 40%)
    pub slope_q60: f64,
    /// 거래량 품질 60번째 퍼센타일 (상위 40%)
    pub vol_quality_q60: f64,
    /// 박스권 위치 75번째 퍼센타일 (상위 25%)
    pub range_pos_q75: f64,
    /// RSI 과열 임계값 (고정)
    pub rsi_overheat: f64,
    /// RSI 건강 범위 상한 (고정)
    pub rsi_healthy_high: f64,
    /// RSI 건강 범위 하한 (고정)
    pub rsi_healthy_low: f64,
}

impl Default for DynamicThresholds {
    /// 기본값 (고정 임계값과 유사).
    fn default() -> Self {
        Self {
            r5_q75: 5.0,          // 5일 수익률 5% 이상이면 상위권
            r5_q25: -3.0,         // 5일 수익률 -3% 이하면 하위권
            slope_q60: 0.0,       // MACD slope 양수면 상위권
            vol_quality_q60: 0.2, // 거래량 품질 0.2 이상
            range_pos_q75: 0.75,  // 박스권 위치 75% 이상
            rsi_overheat: 75.0,   // RSI 75 이상이면 과열
            rsi_healthy_high: 65.0,
            rsi_healthy_low: 45.0,
        }
    }
}

impl DynamicThresholds {
    /// 종목 데이터 목록에서 동적 임계값 계산.
    ///
    /// 각 지표의 퍼센타일을 계산하여 동적 임계값을 생성합니다.
    ///
    /// # 인자
    ///
    /// * `data` - 전체 종목의 분석 데이터
    ///
    /// # 반환
    ///
    /// 계산된 동적 임계값
    pub fn compute(data: &[SymbolData]) -> Self {
        if data.is_empty() {
            return Self::default();
        }

        // 각 지표별 값 수집
        let mut returns_5d: Vec<f64> = data.iter().map(|d| d.return_5d).collect();
        let mut slopes: Vec<f64> = data.iter().map(|d| d.macd_slope).collect();
        let mut vol_qualities: Vec<f64> = data.iter().map(|d| d.vol_quality).collect();
        let mut range_positions: Vec<f64> = data.iter().map(|d| d.range_pos).collect();

        // 정렬
        returns_5d.sort_by(|a, b| a.partial_cmp(b).unwrap());
        slopes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        vol_qualities.sort_by(|a, b| a.partial_cmp(b).unwrap());
        range_positions.sort_by(|a, b| a.partial_cmp(b).unwrap());

        Self {
            r5_q75: percentile(&returns_5d, 75),
            r5_q25: percentile(&returns_5d, 25),
            slope_q60: percentile(&slopes, 60),
            vol_quality_q60: percentile(&vol_qualities, 60),
            range_pos_q75: percentile(&range_positions, 75),
            // RSI 기준은 고정
            rsi_overheat: 75.0,
            rsi_healthy_high: 65.0,
            rsi_healthy_low: 45.0,
        }
    }

    /// 임계값이 유효한지 확인.
    pub fn is_valid(&self) -> bool {
        self.r5_q75 > self.r5_q25
            && self.rsi_healthy_high > self.rsi_healthy_low
            && self.range_pos_q75 > 0.0
            && self.range_pos_q75 <= 1.0
    }
}

/// 퍼센타일 계산.
///
/// # 인자
///
/// * `sorted_data` - 정렬된 데이터
/// * `p` - 퍼센타일 (0-100)
fn percentile(sorted_data: &[f64], p: usize) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }

    let p = p.min(100);
    let idx = (sorted_data.len() * p / 100).min(sorted_data.len() - 1);
    sorted_data[idx]
}

impl RouteStateCalculator {
    /// 동적 임계값 기반 RouteState 계산.
    ///
    /// 전체 시장 데이터의 퍼센타일 분포를 기반으로 판정합니다.
    /// 시장 상황에 적응하는 유연한 판정이 가능합니다.
    ///
    /// # 인자
    ///
    /// * `candles` - 캔들 데이터
    /// * `thresholds` - 동적 임계값
    ///
    /// # 반환
    ///
    /// 계산된 RouteState
    pub fn calculate_dynamic(
        &self,
        candles: &[Kline],
        thresholds: &DynamicThresholds,
    ) -> IndicatorResult<RouteState> {
        if candles.len() < 40 {
            return Err(IndicatorError::InsufficientData {
                required: 40,
                provided: candles.len(),
            });
        }

        // StructuralFeatures 계산
        let ticker = candles.first().map(|k| k.ticker.as_str()).unwrap_or("");
        let features = StructuralFeaturesCalculator::from_candles(ticker, candles, &self.engine)?;

        // 5일 수익률 계산
        let return_5d = self.calculate_return_5d(candles)?;

        // 우선순위에 따라 상태 판정 (동적 임계값 사용)
        // 1. Overheat (가장 높은 우선순위)
        if self.is_overheat_dynamic(&features, return_5d, thresholds) {
            return Ok(RouteState::Overheat);
        }

        // 2. Attack
        if self.is_attack_dynamic(&features, return_5d, thresholds) {
            return Ok(RouteState::Attack);
        }

        // 3. Armed
        if self.is_armed_dynamic(&features, thresholds) {
            return Ok(RouteState::Armed);
        }

        // 4. Wait
        if self.is_wait_dynamic(&features, thresholds) {
            return Ok(RouteState::Wait);
        }

        // 5. Neutral (기본값)
        Ok(RouteState::Neutral)
    }

    /// 동적 Overheat 판정.
    ///
    /// **조건**:
    /// - 5일 수익률 > 20% (절대값, 변경 없음) 또는
    /// - RSI >= rsi_overheat 또는
    /// - 5일 수익률 > r5_q75 * 3 (상위 25%의 3배)
    fn is_overheat_dynamic(
        &self,
        features: &StructuralFeatures,
        return_5d: f64,
        thresholds: &DynamicThresholds,
    ) -> bool {
        let rsi = features.rsi.to_f64().unwrap_or(0.0);
        return_5d > 20.0
            || rsi >= thresholds.rsi_overheat
            || (thresholds.r5_q75 > 0.0 && return_5d > thresholds.r5_q75 * 3.0)
    }

    /// 동적 Attack 판정.
    ///
    /// **조건**:
    /// - RSI 건강 범위 내 (rsi_healthy_low ~ rsi_healthy_high)
    /// - Range_Pos >= range_pos_q75 (상위 25% 박스권 위치)
    /// - Low_Trend > 0 (저가 상승)
    /// - Vol_Quality >= vol_quality_q60 (상위 40% 거래량 품질)
    /// - 5일 수익률 >= r5_q75 (상위 25% 수익률)
    fn is_attack_dynamic(
        &self,
        features: &StructuralFeatures,
        return_5d: f64,
        thresholds: &DynamicThresholds,
    ) -> bool {
        let rsi = features.rsi.to_f64().unwrap_or(0.0);
        let range_pos = features.range_pos.to_f64().unwrap_or(0.0);
        let vol_quality = features.vol_quality.to_f64().unwrap_or(0.0);

        rsi >= thresholds.rsi_healthy_low
            && rsi <= thresholds.rsi_healthy_high
            && range_pos >= thresholds.range_pos_q75
            && features.low_trend > Decimal::ZERO
            && vol_quality >= thresholds.vol_quality_q60
            && return_5d >= thresholds.r5_q75
    }

    /// 동적 Armed 판정.
    ///
    /// **조건**:
    /// - Vol_Quality >= vol_quality_q60 (매집 신호) 또는
    /// - Dist_MA20 > -2.0 (MA20 근처 또는 위)
    fn is_armed_dynamic(
        &self,
        features: &StructuralFeatures,
        thresholds: &DynamicThresholds,
    ) -> bool {
        let vol_quality = features.vol_quality.to_f64().unwrap_or(0.0);
        vol_quality >= thresholds.vol_quality_q60 || features.dist_ma20 > Decimal::new(-2, 0)
    }

    /// 동적 Wait 판정.
    ///
    /// **조건**:
    /// - Low_Trend > 0 (저가 상승 = 정배열 유지)
    /// - Dist_MA20 > -5.0 (MA 지지)
    fn is_wait_dynamic(
        &self,
        features: &StructuralFeatures,
        _thresholds: &DynamicThresholds,
    ) -> bool {
        // Wait 조건은 고정 임계값 유지 (시장 적응 불필요)
        features.low_trend > Decimal::ZERO && features.dist_ma20 > Decimal::new(-5, 0)
    }

    /// SymbolData 생성 헬퍼.
    ///
    /// 캔들 데이터에서 동적 임계값 계산에 필요한 SymbolData를 생성합니다.
    pub fn create_symbol_data(
        &self,
        ticker: &str,
        candles: &[Kline],
    ) -> IndicatorResult<SymbolData> {
        if candles.len() < 40 {
            return Err(IndicatorError::InsufficientData {
                required: 40,
                provided: candles.len(),
            });
        }

        let features = StructuralFeaturesCalculator::from_candles(ticker, candles, &self.engine)?;
        let return_5d = self.calculate_return_5d(candles)?;

        // MACD slope 계산 (간단 버전: RSI 변화율로 대체)
        let rsi_f64 = features.rsi.to_f64().unwrap_or(50.0);
        let macd_slope = if candles.len() >= 5 {
            // 5일 전 RSI는 없으므로 단순히 현재 RSI에서 50을 뺀 값으로 대체
            (rsi_f64 - 50.0) / 25.0 // -1.0 ~ 1.0 범위로 정규화
        } else {
            0.0
        };

        Ok(SymbolData {
            ticker: ticker.to_string(),
            return_5d,
            macd_slope,
            vol_quality: features.vol_quality.to_f64().unwrap_or(0.0),
            range_pos: features.range_pos.to_f64().unwrap_or(0.0),
            rsi: rsi_f64,
            dist_ma20: features.dist_ma20.to_f64().unwrap_or(0.0),
            low_trend: features.low_trend.to_f64().unwrap_or(0.0),
            ttm_squeeze: false, // TTM Squeeze는 별도 계산 필요
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use trader_core::Timeframe;
    use trader_core::domain::StructuralFeatures;

    fn create_test_candles(count: usize, trend: &str) -> Vec<Kline> {
        let ticker = "TEST/USD".to_string();
        let mut candles = Vec::with_capacity(count);

        for i in 0..count {
            let base_price = dec!(100.0) + Decimal::from(i as i64);
            let (open, close) = match trend {
                "up" => (base_price, base_price + dec!(1.0)),
                "down" => (base_price, base_price - dec!(1.0)),
                _ => (base_price, base_price),
            };

            let now = Utc::now();
            candles.push(Kline {
                ticker: ticker.clone(),
                timeframe: Timeframe::D1,
                open_time: now,
                open,
                high: open.max(close) + dec!(0.5),
                low: open.min(close) - dec!(0.5),
                close,
                volume: dec!(1000000.0),
                close_time: now + chrono::Duration::days(1),
                quote_volume: None,
                num_trades: None,
            });
        }

        candles
    }

    #[test]
    fn test_insufficient_data() {
        let calculator = RouteStateCalculator::new();
        let candles = create_test_candles(30, "up");

        let result = calculator.calculate(&candles);
        assert!(result.is_err());
    }

    #[test]
    fn test_return_5d_calculation() {
        let calculator = RouteStateCalculator::new();
        let mut candles = create_test_candles(50, "neutral");

        // 5일 전: 100, 현재: 120 → 20% 상승
        candles[44].close = dec!(100.0);
        candles[49].close = dec!(120.0);

        let return_5d = calculator.calculate_return_5d(&candles).unwrap();
        assert!((return_5d - 20.0).abs() < 0.1);
    }

    #[test]
    fn test_overheat_by_return() {
        let calculator = RouteStateCalculator::new();
        let mut candles = create_test_candles(50, "up");

        // 5일 수익률 > 20%
        candles[44].close = dec!(100.0);
        candles[49].close = dec!(125.0);

        let state = calculator.calculate(&candles).unwrap();
        assert_eq!(state, RouteState::Overheat);
    }

    #[test]
    fn test_default() {
        let calc1 = RouteStateCalculator::new();
        let calc2 = RouteStateCalculator::default();

        let candles = create_test_candles(50, "up");
        let state1 = calc1.calculate(&candles).unwrap();
        let state2 = calc2.calculate(&candles).unwrap();

        assert_eq!(state1, state2);
    }

    // === Dynamic Route Tagging 테스트 ===

    fn create_test_symbol_data(count: usize) -> Vec<SymbolData> {
        (0..count)
            .map(|i| {
                let factor = i as f64 / count as f64;
                SymbolData {
                    ticker: format!("TEST{}", i),
                    return_5d: -10.0 + factor * 30.0, // -10% ~ 20%
                    macd_slope: -0.5 + factor * 1.5,  // -0.5 ~ 1.0
                    vol_quality: factor * 100.0,      // 0 ~ 100
                    range_pos: factor,                // 0.0 ~ 1.0
                    rsi: 30.0 + factor * 40.0,        // 30 ~ 70
                    dist_ma20: -5.0 + factor * 15.0,  // -5% ~ 10%
                    low_trend: -0.3 + factor * 0.6,   // -0.3 ~ 0.3
                    ttm_squeeze: i % 5 == 0,          // 20% TTM squeeze
                }
            })
            .collect()
    }

    #[test]
    fn test_percentile_basic() {
        // 1~10 데이터 (정렬됨)
        let data: Vec<f64> = (1..=10).map(|x| x as f64).collect();

        // percentile 함수는 보간 없이 인덱스 방식 사용
        // idx = (len * p / 100) = (10 * 50 / 100) = 5 → data[5] = 6
        let p50 = percentile(&data, 50);
        assert!((p50 - 6.0).abs() < 0.1, "P50 should be 6, got {}", p50);

        // 100번째 퍼센타일 = 최대값 (idx = min(10, 9) = 9)
        let p100 = percentile(&data, 100);
        assert!((p100 - 10.0).abs() < 0.1, "P100 should be 10, got {}", p100);

        // 0번째 퍼센타일 = 최소값 (idx = 0)
        let p0 = percentile(&data, 0);
        assert!((p0 - 1.0).abs() < 0.1, "P0 should be 1, got {}", p0);

        // 75번째 퍼센타일 (idx = 7 → data[7] = 8)
        let p75 = percentile(&data, 75);
        assert!((p75 - 8.0).abs() < 0.1, "P75 should be 8, got {}", p75);
    }

    #[test]
    fn test_percentile_edge_cases() {
        // 빈 배열 → 0.0 반환
        let empty: Vec<f64> = vec![];
        assert!(
            (percentile(&empty, 50) - 0.0).abs() < 0.1,
            "Empty should return 0.0"
        );

        // 단일 요소
        let single = vec![42.0];
        let p = percentile(&single, 50);
        assert!(
            (p - 42.0).abs() < 0.1,
            "단일 요소 P50 should be 42, got {}",
            p
        );
    }

    #[test]
    fn test_dynamic_thresholds_compute() {
        let data = create_test_symbol_data(100);
        let thresholds = DynamicThresholds::compute(&data);

        // 75번째 퍼센타일 검증 (r5_q75)
        // return_5d 범위: -10 ~ 20, 75% 지점 ≈ 12.5
        assert!(
            thresholds.r5_q75 > 5.0,
            "r5_q75 should be > 5, got {}",
            thresholds.r5_q75
        );
        assert!(
            thresholds.r5_q75 < 20.0,
            "r5_q75 should be < 20, got {}",
            thresholds.r5_q75
        );

        // 25번째 퍼센타일 검증 (r5_q25)
        assert!(
            thresholds.r5_q25 < 0.0,
            "r5_q25 should be < 0, got {}",
            thresholds.r5_q25
        );

        // slope_q60 검증
        // macd_slope 범위: -0.5 ~ 1.0, 60% 지점
        assert!(
            thresholds.slope_q60 > 0.0,
            "slope_q60 should be > 0, got {}",
            thresholds.slope_q60
        );

        // 모든 임계값이 유효한 값인지 확인
        assert!(!thresholds.r5_q75.is_nan(), "r5_q75 should not be NaN");
        assert!(
            !thresholds.slope_q60.is_nan(),
            "slope_q60 should not be NaN"
        );
        assert!(
            !thresholds.vol_quality_q60.is_nan(),
            "vol_quality_q60 should not be NaN"
        );

        // RSI 임계값은 고정
        assert!(
            (thresholds.rsi_overheat - 75.0).abs() < 0.1,
            "rsi_overheat should be 75"
        );
    }

    #[test]
    fn test_dynamic_thresholds_with_empty_data() {
        let empty: Vec<SymbolData> = vec![];
        let thresholds = DynamicThresholds::compute(&empty);

        // 빈 데이터에서는 기본값 (default) 반환
        let default = DynamicThresholds::default();
        assert!(
            (thresholds.r5_q75 - default.r5_q75).abs() < 0.1,
            "Should return default values"
        );
        assert!(
            (thresholds.r5_q25 - default.r5_q25).abs() < 0.1,
            "Should return default values"
        );
    }

    #[test]
    fn test_symbol_data_creation() {
        let data = create_test_symbol_data(10);
        assert_eq!(data.len(), 10);

        // 첫 번째와 마지막 데이터 검증
        assert!(
            data[0].return_5d < data[9].return_5d,
            "return_5d should increase"
        );
        assert!(
            data[0].vol_quality < data[9].vol_quality,
            "vol_quality should increase"
        );
    }

    #[test]
    fn test_is_attack_dynamic_via_features() {
        let calculator = RouteStateCalculator::new();

        // 강한 돌파 StructuralFeatures 생성
        // Note: StructuralFeatures에는 macd_slope가 없음 - low_trend로 모멘텀 대체
        let strong_features = StructuralFeatures {
            vol_quality: dec!(80.0),
            range_pos: dec!(0.9),
            rsi: dec!(65.0),
            dist_ma20: dec!(3.0),
            low_trend: dec!(0.8),
            bb_width: dec!(5.0),
            bb_upper: dec!(110.0),
            bb_middle: dec!(100.0),
            bb_lower: dec!(90.0),
            ..Default::default()
        };

        // 낮은 임계값으로 Attack 판정 확인
        let low_thresholds = DynamicThresholds {
            r5_q75: 10.0, // 낮은 임계값
            r5_q25: -5.0,
            slope_q60: 0.5,
            vol_quality_q60: 60.0,
            range_pos_q75: 0.7,
            rsi_overheat: 75.0,
            rsi_healthy_high: 65.0,
            rsi_healthy_low: 45.0,
        };

        let return_5d = 15.0; // 높은 5일 수익률

        assert!(
            calculator.is_attack_dynamic(&strong_features, return_5d, &low_thresholds),
            "Strong features should be Attack with low thresholds"
        );

        // 높은 임계값으로 Attack 판정 안 됨
        let high_thresholds = DynamicThresholds {
            r5_q75: 20.0, // 높은 임계값
            r5_q25: -2.0,
            slope_q60: 1.0,
            vol_quality_q60: 90.0,
            range_pos_q75: 0.95,
            rsi_overheat: 75.0,
            rsi_healthy_high: 65.0,
            rsi_healthy_low: 45.0,
        };

        assert!(
            !calculator.is_attack_dynamic(&strong_features, return_5d, &high_thresholds),
            "Strong features should NOT be Attack with high thresholds"
        );
    }

    #[test]
    fn test_is_overheat_dynamic_via_features() {
        let calculator = RouteStateCalculator::new();

        // 과열 StructuralFeatures (RSI > 75)
        let overheat_features = StructuralFeatures {
            vol_quality: dec!(50.0),
            range_pos: dec!(0.5),
            rsi: dec!(80.0),
            dist_ma20: dec!(2.0),
            low_trend: dec!(0.1),
            bb_width: dec!(5.0),
            bb_upper: dec!(110.0),
            bb_middle: dec!(100.0),
            bb_lower: dec!(90.0),
            ..Default::default()
        };

        let thresholds = DynamicThresholds {
            r5_q75: 10.0,
            r5_q25: -5.0,
            slope_q60: 0.5,
            vol_quality_q60: 60.0,
            range_pos_q75: 0.7,
            rsi_overheat: 75.0,
            rsi_healthy_high: 65.0,
            rsi_healthy_low: 45.0,
        };

        let return_5d = 5.0;

        assert!(
            calculator.is_overheat_dynamic(&overheat_features, return_5d, &thresholds),
            "RSI > 75 should be Overheat"
        );

        // 수익률 과열 테스트 (r5 > r5_q75 * 2)
        let normal_features = StructuralFeatures {
            vol_quality: dec!(50.0),
            range_pos: dec!(0.5),
            rsi: dec!(60.0),
            dist_ma20: dec!(2.0),
            low_trend: dec!(0.1),
            bb_width: dec!(5.0),
            bb_upper: dec!(110.0),
            bb_middle: dec!(100.0),
            bb_lower: dec!(90.0),
            ..Default::default()
        };

        let high_return_5d = 25.0; // 10.0 * 2 = 20.0보다 큼

        assert!(
            calculator.is_overheat_dynamic(&normal_features, high_return_5d, &thresholds),
            "return_5d > r5_q75 * 2 should be Overheat"
        );
    }

    #[test]
    fn test_route_state_priority_via_features() {
        // RouteState 우선순위: Overheat > Attack > Armed > Wait > Neutral
        // 동적 임계값 기반으로 올바른 우선순위 적용 확인

        let calculator = RouteStateCalculator::new();
        let thresholds = DynamicThresholds {
            r5_q75: 10.0,
            r5_q25: -5.0,
            slope_q60: 0.5,
            vol_quality_q60: 60.0,
            range_pos_q75: 0.7,
            rsi_overheat: 75.0,
            rsi_healthy_high: 65.0,
            rsi_healthy_low: 45.0,
        };

        // Overheat 조건 (RSI > 75)과 Attack 조건 동시 충족 → Overheat 우선
        let both_conditions_features = StructuralFeatures {
            vol_quality: dec!(80.0),
            range_pos: dec!(0.9),
            rsi: dec!(80.0),
            dist_ma20: dec!(3.0),
            low_trend: dec!(0.8),
            bb_width: dec!(5.0),
            bb_upper: dec!(110.0),
            bb_middle: dec!(100.0),
            bb_lower: dec!(90.0),
            ..Default::default()
        };

        let return_5d = 15.0;

        // Overheat가 우선
        assert!(
            calculator.is_overheat_dynamic(&both_conditions_features, return_5d, &thresholds),
            "Overheat should take priority"
        );
    }

    #[test]
    fn test_default_thresholds() {
        let default = DynamicThresholds::default();

        // 기본값 검증
        assert!((default.r5_q75 - 5.0).abs() < 0.1);
        assert!((default.r5_q25 - (-3.0)).abs() < 0.1);
        assert!((default.rsi_overheat - 75.0).abs() < 0.1);
        assert!((default.rsi_healthy_high - 65.0).abs() < 0.1);
        assert!((default.rsi_healthy_low - 45.0).abs() < 0.1);
    }
}
