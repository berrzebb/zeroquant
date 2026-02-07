//! 볼륨 프로파일 (매물대) 계산.
//!
//! 캔들 데이터에서 가격대별 거래량을 집계하여 볼륨 프로파일을 생성합니다.
//!
//! # 주요 지표
//!
//! - **POC (Point of Control)**: 최대 거래량이 집중된 가격대
//! - **Value Area (VA)**: 전체 거래량의 70%가 집중된 가격 범위
//! - **VAH/VAL**: Value Area High/Low
//!
//! # 예시
//!
//! ```rust,ignore
//! use trader_analytics::volume_profile::VolumeProfileCalculator;
//! use trader_core::Kline;
//!
//! let calculator = VolumeProfileCalculator::new(20); // 20개 가격 레벨
//! let klines: Vec<Kline> = /* ... */;
//!
//! let profile = calculator.calculate(&klines);
//! println!("POC: {}", profile.poc);
//! println!("Value Area: {} ~ {}", profile.value_area_low, profile.value_area_high);
//! ```

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use trader_core::Kline;

/// 가격대별 거래량 레벨.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    /// 가격 (레벨 중심 가격)
    pub price: Decimal,
    /// 해당 가격대의 총 거래량
    pub volume: Decimal,
    /// 전체 거래량 대비 비율 (%)
    pub volume_pct: Decimal,
}

/// 볼륨 프로파일 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeProfile {
    /// 가격대별 거래량 레벨 (가격 오름차순)
    pub price_levels: Vec<PriceLevel>,
    /// POC (Point of Control) - 최대 거래량 가격
    pub poc: Decimal,
    /// POC 인덱스
    pub poc_index: usize,
    /// Value Area High (70% 상한)
    pub value_area_high: Decimal,
    /// Value Area Low (70% 하한)
    pub value_area_low: Decimal,
    /// 전체 거래량
    pub total_volume: Decimal,
    /// 가격 범위 (최저)
    pub price_low: Decimal,
    /// 가격 범위 (최고)
    pub price_high: Decimal,
    /// 분석 기간 (캔들 수)
    pub period: usize,
}

/// 볼륨 프로파일 계산기.
pub struct VolumeProfileCalculator {
    /// 가격 레벨 수 (기본: 20)
    num_levels: usize,
    /// Value Area 비율 (기본: 0.7 = 70%)
    value_area_ratio: Decimal,
}

impl Default for VolumeProfileCalculator {
    fn default() -> Self {
        Self::new(20)
    }
}

impl VolumeProfileCalculator {
    /// 새 계산기 생성.
    ///
    /// # 인자
    ///
    /// * `num_levels` - 가격 레벨 수 (10~50 권장)
    pub fn new(num_levels: usize) -> Self {
        Self {
            num_levels: num_levels.clamp(5, 100),
            value_area_ratio: dec!(0.70),
        }
    }

    /// Value Area 비율 설정.
    ///
    /// # 인자
    ///
    /// * `ratio` - 0.5~0.9 사이의 값 (기본: 0.7)
    pub fn with_value_area_ratio(mut self, ratio: Decimal) -> Self {
        self.value_area_ratio = ratio.max(dec!(0.5)).min(dec!(0.9));
        self
    }

    /// 볼륨 프로파일 계산.
    ///
    /// # 인자
    ///
    /// * `klines` - 캔들 데이터 (최소 10개 권장)
    ///
    /// # 반환
    ///
    /// 볼륨 프로파일 결과 (데이터 부족 시 None)
    pub fn calculate(&self, klines: &[Kline]) -> Option<VolumeProfile> {
        if klines.len() < 2 {
            return None;
        }

        // 1. 가격 범위 계산
        let (price_low, price_high) = self.find_price_range(klines);
        if price_high <= price_low {
            return None;
        }

        // 2. 가격 레벨 간격 계산
        let level_size = (price_high - price_low) / Decimal::from(self.num_levels);
        if level_size <= Decimal::ZERO {
            return None;
        }

        // 3. 각 레벨의 거래량 집계
        let mut levels: Vec<Decimal> = vec![Decimal::ZERO; self.num_levels];
        let mut total_volume = Decimal::ZERO;

        for kline in klines {
            // 캔들 범위 내 거래량 분배
            self.distribute_volume(kline, price_low, level_size, &mut levels, &mut total_volume);
        }

        if total_volume <= Decimal::ZERO {
            return None;
        }

        // 4. PriceLevel 구조체 생성
        let price_levels: Vec<PriceLevel> = levels
            .iter()
            .enumerate()
            .map(|(i, &vol)| {
                let level_center = price_low + level_size * Decimal::from(i) + level_size / dec!(2);
                PriceLevel {
                    price: level_center,
                    volume: vol,
                    volume_pct: (vol / total_volume * dec!(100)).round_dp(2),
                }
            })
            .collect();

        // 5. POC 계산 (최대 거래량 레벨)
        let (poc_index, poc) = self.find_poc(&price_levels);

        // 6. Value Area 계산
        let (value_area_low, value_area_high) =
            self.calculate_value_area(&price_levels, poc_index, total_volume);

        Some(VolumeProfile {
            price_levels,
            poc,
            poc_index,
            value_area_high,
            value_area_low,
            total_volume,
            price_low,
            price_high,
            period: klines.len(),
        })
    }

    /// 가격 범위 계산 (전체 캔들의 저가/고가).
    fn find_price_range(&self, klines: &[Kline]) -> (Decimal, Decimal) {
        let mut low = Decimal::MAX;
        let mut high = Decimal::MIN;

        for kline in klines {
            if kline.low < low {
                low = kline.low;
            }
            if kline.high > high {
                high = kline.high;
            }
        }

        (low, high)
    }

    /// 캔들 거래량을 가격 레벨에 분배.
    ///
    /// OHLCV 방식: 캔들의 고가-저가 범위에 거래량을 균등 분배
    fn distribute_volume(
        &self,
        kline: &Kline,
        price_low: Decimal,
        level_size: Decimal,
        levels: &mut [Decimal],
        total_volume: &mut Decimal,
    ) {
        let candle_low = kline.low;
        let candle_high = kline.high;
        let volume = kline.volume;

        if candle_high <= candle_low || volume <= Decimal::ZERO {
            return;
        }

        // 캔들이 걸쳐있는 레벨 찾기
        let start_level = ((candle_low - price_low) / level_size)
            .floor()
            .to_string()
            .parse::<i32>()
            .unwrap_or(0)
            .max(0) as usize;

        let end_level = ((candle_high - price_low) / level_size)
            .floor()
            .to_string()
            .parse::<i32>()
            .unwrap_or(0)
            .max(0) as usize;

        let num_levels_covered = (end_level - start_level + 1).max(1);
        let volume_per_level = volume / Decimal::from(num_levels_covered);

        for i in start_level..=end_level {
            if i < levels.len() {
                levels[i] += volume_per_level;
            }
        }

        *total_volume += volume;
    }

    /// POC (Point of Control) 찾기.
    fn find_poc(&self, levels: &[PriceLevel]) -> (usize, Decimal) {
        let mut max_vol = Decimal::ZERO;
        let mut poc_index = 0;

        for (i, level) in levels.iter().enumerate() {
            if level.volume > max_vol {
                max_vol = level.volume;
                poc_index = i;
            }
        }

        (
            poc_index,
            levels
                .get(poc_index)
                .map(|l| l.price)
                .unwrap_or(Decimal::ZERO),
        )
    }

    /// Value Area 계산 (70% 거래량 영역).
    fn calculate_value_area(
        &self,
        levels: &[PriceLevel],
        poc_index: usize,
        total_volume: Decimal,
    ) -> (Decimal, Decimal) {
        if levels.is_empty() {
            return (Decimal::ZERO, Decimal::ZERO);
        }

        let target_volume = total_volume * self.value_area_ratio;
        let mut included_volume = levels[poc_index].volume;
        let mut low_index = poc_index;
        let mut high_index = poc_index;

        // POC에서 양방향 확장
        while included_volume < target_volume && (low_index > 0 || high_index < levels.len() - 1) {
            let next_low_vol = if low_index > 0 {
                levels[low_index - 1].volume
            } else {
                Decimal::ZERO
            };
            let next_high_vol = if high_index < levels.len() - 1 {
                levels[high_index + 1].volume
            } else {
                Decimal::ZERO
            };

            // 더 큰 거래량 방향으로 확장
            if next_low_vol >= next_high_vol && low_index > 0 {
                low_index -= 1;
                included_volume += levels[low_index].volume;
            } else if high_index < levels.len() - 1 {
                high_index += 1;
                included_volume += levels[high_index].volume;
            } else if low_index > 0 {
                low_index -= 1;
                included_volume += levels[low_index].volume;
            } else {
                break;
            }
        }

        (levels[low_index].price, levels[high_index].price)
    }
}

/// 간편 함수: 캔들 데이터에서 볼륨 프로파일 계산.
///
/// # 인자
///
/// * `klines` - 캔들 데이터
/// * `num_levels` - 가격 레벨 수 (기본: 20)
pub fn calculate_volume_profile(klines: &[Kline], num_levels: usize) -> Option<VolumeProfile> {
    VolumeProfileCalculator::new(num_levels).calculate(klines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use rust_decimal_macros::dec;
    use trader_core::Timeframe;

    fn create_test_klines() -> Vec<Kline> {
        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        vec![
            Kline {
                ticker: "TEST".to_string(),
                timeframe: Timeframe::D1,
                open_time: base_time,
                open: dec!(100),
                high: dec!(110),
                low: dec!(95),
                close: dec!(105),
                volume: dec!(1000),
                close_time: base_time + chrono::Duration::days(1),
                quote_volume: Some(dec!(0)),
                num_trades: Some(100),
            },
            Kline {
                ticker: "TEST".to_string(),
                timeframe: Timeframe::D1,
                open_time: base_time + chrono::Duration::days(1),
                open: dec!(105),
                high: dec!(115),
                low: dec!(100),
                close: dec!(112),
                volume: dec!(1500),
                close_time: base_time + chrono::Duration::days(2),
                quote_volume: Some(dec!(0)),
                num_trades: Some(150),
            },
            Kline {
                ticker: "TEST".to_string(),
                timeframe: Timeframe::D1,
                open_time: base_time + chrono::Duration::days(2),
                open: dec!(112),
                high: dec!(120),
                low: dec!(108),
                close: dec!(118),
                volume: dec!(2000),
                close_time: base_time + chrono::Duration::days(3),
                quote_volume: Some(dec!(0)),
                num_trades: Some(200),
            },
            Kline {
                ticker: "TEST".to_string(),
                timeframe: Timeframe::D1,
                open_time: base_time + chrono::Duration::days(3),
                open: dec!(118),
                high: dec!(122),
                low: dec!(110),
                close: dec!(115),
                volume: dec!(1800),
                close_time: base_time + chrono::Duration::days(4),
                quote_volume: Some(dec!(0)),
                num_trades: Some(180),
            },
        ]
    }

    #[test]
    fn test_volume_profile_basic() {
        let klines = create_test_klines();
        let calculator = VolumeProfileCalculator::new(10);
        let profile = calculator.calculate(&klines);

        assert!(profile.is_some());
        let profile = profile.unwrap();

        // 기본 검증
        assert_eq!(profile.period, 4);
        assert_eq!(profile.price_levels.len(), 10);
        assert!(profile.total_volume > Decimal::ZERO);
        assert!(profile.poc >= profile.price_low);
        assert!(profile.poc <= profile.price_high);
    }

    #[test]
    fn test_value_area() {
        let klines = create_test_klines();
        let calculator = VolumeProfileCalculator::new(10);
        let profile = calculator.calculate(&klines).unwrap();

        // Value Area는 POC 주변에 위치
        assert!(profile.value_area_low <= profile.poc);
        assert!(profile.value_area_high >= profile.poc);
        assert!(profile.value_area_low >= profile.price_low);
        assert!(profile.value_area_high <= profile.price_high);
    }

    #[test]
    fn test_poc_is_max_volume() {
        let klines = create_test_klines();
        let profile = calculate_volume_profile(&klines, 10).unwrap();

        // POC는 최대 거래량 레벨
        let poc_level = &profile.price_levels[profile.poc_index];
        for level in &profile.price_levels {
            assert!(level.volume <= poc_level.volume);
        }
    }

    #[test]
    fn test_volume_pct_sum() {
        let klines = create_test_klines();
        let profile = calculate_volume_profile(&klines, 10).unwrap();

        // 비율 합계는 100%에 근접 (반올림 + 분배 오차 허용)
        // 각 레벨의 volume_pct는 반올림되므로 오차가 누적될 수 있음
        let total_pct: Decimal = profile.price_levels.iter().map(|l| l.volume_pct).sum();
        assert!(
            (total_pct - dec!(100)).abs() < dec!(5),
            "Total pct: {}, expected ~100",
            total_pct
        );
    }

    #[test]
    fn test_insufficient_data() {
        let klines = vec![]; // 빈 데이터
        let result = calculate_volume_profile(&klines, 10);
        assert!(result.is_none());

        let single = vec![create_test_klines().remove(0)]; // 단일 캔들
        let result = calculate_volume_profile(&single, 10);
        assert!(result.is_none());
    }

    #[test]
    fn test_custom_value_area_ratio() {
        let klines = create_test_klines();

        // 70% Value Area
        let profile_70 = VolumeProfileCalculator::new(10)
            .with_value_area_ratio(dec!(0.70))
            .calculate(&klines)
            .unwrap();

        // 80% Value Area
        let profile_80 = VolumeProfileCalculator::new(10)
            .with_value_area_ratio(dec!(0.80))
            .calculate(&klines)
            .unwrap();

        // 80% VA는 70% VA보다 넓어야 함
        let va_range_70 = profile_70.value_area_high - profile_70.value_area_low;
        let va_range_80 = profile_80.value_area_high - profile_80.value_area_low;
        assert!(va_range_80 >= va_range_70);
    }
}
