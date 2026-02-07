//! 트레이딩 전략을 위한 모멘텀 계산 유틸리티.
//!
//! HAA, XAA, BAA와 같은 자산 배분 전략에서 사용되는 모멘텀 스코어링을 제공합니다.
//!
//! # 모멘텀 유형
//!
//! - **단순 모멘텀**: 단일 기간 동안의 가격 변화
//! - **다기간 평균**: 여러 룩백 기간에 걸친 모멘텀의 평균
//! - **가중 모멘텀**: 기간별 사용자 정의 가중치를 적용한 모멘텀
//!
//! # 예제
//!
//! ```rust
//! use trader_strategy::strategies::common::MomentumCalculator;
//! use rust_decimal_macros::dec;
//!
//! // 표준 기간(1, 3, 6, 12개월)으로 계산기 생성
//! let calc = MomentumCalculator::standard();
//!
//! // 샘플 일별 가격 (가장 최근 가격이 먼저)
//! let prices = vec![
//!     dec!(100.0), // 오늘
//!     dec!(98.0),  // 1일 전
//!     // ... 더 많은 가격
//! ];
//!
//! // 모멘텀 점수 계산
//! let result = calc.calculate(&prices);
//! println!("모멘텀 점수: {:?}", result.score);
//! ```

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 월별 거래일 수 (근사치).
pub const TRADING_DAYS_PER_MONTH: usize = 21;

/// 연간 거래일 수.
pub const TRADING_DAYS_PER_YEAR: usize = 252;

/// 모멘텀 계산 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomentumConfig {
    /// 거래일 기준 룩백 기간.
    /// 기본값: [21, 63, 126, 252] (1, 3, 6, 12개월)
    pub lookback_periods: Vec<usize>,
    /// 평균 계산 시 동일 가중치 사용 여부.
    pub equal_weights: bool,
    /// 필요한 최소 데이터 포인트 수 (기본값: 최대 룩백 + 1).
    pub min_data_points: Option<usize>,
}

impl Default for MomentumConfig {
    fn default() -> Self {
        Self {
            lookback_periods: vec![
                TRADING_DAYS_PER_MONTH,      // 1 month (21 days)
                TRADING_DAYS_PER_MONTH * 3,  // 3 months (63 days)
                TRADING_DAYS_PER_MONTH * 6,  // 6 months (126 days)
                TRADING_DAYS_PER_MONTH * 12, // 12 months (252 days)
            ],
            equal_weights: true,
            min_data_points: None,
        }
    }
}

/// 가중 모멘텀 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedMomentumConfig {
    /// 기간과 가중치: [(기간_일수, 가중치), ...]
    pub period_weights: Vec<(usize, Decimal)>,
    /// 가중치 합계를 1로 정규화할지 여부.
    pub normalize_weights: bool,
}

impl Default for WeightedMomentumConfig {
    fn default() -> Self {
        // Default weights used in some infinity bot strategies:
        // 10-month * 0.3 + 100-day * 0.2 + 10-day * 0.3
        Self {
            period_weights: vec![
                (TRADING_DAYS_PER_MONTH * 10, dec!(0.3)), // 10 months
                (100, dec!(0.2)),                         // 100 days
                (10, dec!(0.3)),                          // 10 days
            ],
            normalize_weights: false,
        }
    }
}

/// 단일 기간에 대한 개별 모멘텀 점수.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomentumScore {
    /// 일 단위 룩백 기간.
    pub period_days: usize,
    /// 모멘텀 값 (가격 수익률).
    pub momentum: Decimal,
    /// 시작 가격 (과거).
    pub start_price: Decimal,
    /// 종료 가격 (현재).
    pub end_price: Decimal,
}

/// 모멘텀 계산 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomentumResult {
    /// 최종 모멘텀 점수 (평균 또는 가중치 적용).
    pub score: Decimal,
    /// 기간별 개별 점수.
    pub period_scores: Vec<MomentumScore>,
    /// 사용된 유효 기간 수.
    pub valid_periods: usize,
    /// 결과 유효 여부 (충분한 데이터 존재).
    pub is_valid: bool,
    /// 유효하지 않은 경우 에러 메시지.
    pub error: Option<String>,
}

impl MomentumResult {
    /// 에러 메시지와 함께 유효하지 않은 결과 생성.
    pub fn invalid(error: impl Into<String>) -> Self {
        Self {
            score: Decimal::ZERO,
            period_scores: vec![],
            valid_periods: 0,
            is_valid: false,
            error: Some(error.into()),
        }
    }

    /// 모멘텀이 양수인지 확인.
    pub fn is_positive(&self) -> bool {
        self.is_valid && self.score > Decimal::ZERO
    }

    /// 모멘텀이 음수인지 확인.
    pub fn is_negative(&self) -> bool {
        self.is_valid && self.score < Decimal::ZERO
    }

    /// 모멘텀을 백분율로 반환.
    pub fn as_percentage(&self) -> Decimal {
        self.score * dec!(100)
    }
}

/// 트레이딩 전략을 위한 모멘텀 계산기.
///
/// HAA (Hierarchical Asset Allocation), XAA (Extended Asset Allocation),
/// BAA (Balanced Asset Allocation)와 같은 자산 배분 전략에서 사용되는
/// 모멘텀 점수를 계산합니다.
#[derive(Debug, Clone)]
pub struct MomentumCalculator {
    config: MomentumConfig,
}

impl MomentumCalculator {
    /// 사용자 정의 설정으로 새 모멘텀 계산기 생성.
    pub fn new(config: MomentumConfig) -> Self {
        Self { config }
    }

    /// 표준 12개월 모멘텀 기간으로 계산기 생성.
    ///
    /// 사용 기간: 1, 3, 6, 12개월 (21, 63, 126, 252 거래일).
    pub fn standard() -> Self {
        Self::new(MomentumConfig::default())
    }

    /// 사용자 정의 룩백 기간(월 단위)으로 계산기 생성.
    pub fn with_months(months: &[usize]) -> Self {
        let lookback_periods: Vec<usize> =
            months.iter().map(|m| m * TRADING_DAYS_PER_MONTH).collect();

        Self::new(MomentumConfig {
            lookback_periods,
            equal_weights: true,
            min_data_points: None,
        })
    }

    /// 사용자 정의 룩백 기간(일 단위)으로 계산기 생성.
    pub fn with_days(days: &[usize]) -> Self {
        Self::new(MomentumConfig {
            lookback_periods: days.to_vec(),
            equal_weights: true,
            min_data_points: None,
        })
    }

    /// 가격 시리즈에서 모멘텀 점수 계산.
    ///
    /// # 인수
    /// * `prices` - 가장 최근 가격이 먼저 오는 가격 시리즈 (인덱스 0 = 오늘).
    ///
    /// # 반환값
    /// 평균 모멘텀 점수와 기간별 개별 점수를 포함하는 `MomentumResult`.
    pub fn calculate(&self, prices: &[Decimal]) -> MomentumResult {
        let max_period = self
            .config
            .lookback_periods
            .iter()
            .max()
            .copied()
            .unwrap_or(0);
        let min_required = self.config.min_data_points.unwrap_or(max_period + 1);

        if prices.len() < min_required {
            return MomentumResult::invalid(format!(
                "Insufficient data: need {} prices, got {}",
                min_required,
                prices.len()
            ));
        }

        if prices.is_empty() {
            return MomentumResult::invalid("Empty price series");
        }

        let current_price = prices[0];
        if current_price.is_zero() {
            return MomentumResult::invalid("Current price is zero");
        }

        let mut period_scores = Vec::new();
        let mut total_momentum = Decimal::ZERO;
        let mut valid_count = 0usize;

        for &period in &self.config.lookback_periods {
            if period < prices.len() {
                let past_price = prices[period];
                if !past_price.is_zero() {
                    // Momentum = (current / past) - 1 = price return
                    let momentum = (current_price / past_price) - Decimal::ONE;

                    period_scores.push(MomentumScore {
                        period_days: period,
                        momentum,
                        start_price: past_price,
                        end_price: current_price,
                    });

                    total_momentum += momentum;
                    valid_count += 1;
                }
            }
        }

        if valid_count == 0 {
            return MomentumResult::invalid("No valid periods could be calculated");
        }

        // Average momentum across all valid periods
        let score = total_momentum / Decimal::from(valid_count);

        MomentumResult {
            score,
            period_scores,
            valid_periods: valid_count,
            is_valid: true,
            error: None,
        }
    }

    /// 여러 자산의 모멘텀을 계산하고 순위를 매김.
    ///
    /// # 인수
    /// * `asset_prices` - 자산 심볼과 가격 시리즈의 HashMap.
    ///
    /// # 반환값
    /// 점수 내림차순으로 정렬된 (심볼, 모멘텀_결과) 벡터.
    pub fn rank_assets(
        &self,
        asset_prices: &HashMap<String, Vec<Decimal>>,
    ) -> Vec<(String, MomentumResult)> {
        let mut results: Vec<(String, MomentumResult)> = asset_prices
            .iter()
            .map(|(ticker, prices)| (ticker.clone(), self.calculate(prices)))
            .filter(|(_, result)| result.is_valid)
            .collect();

        // Sort by score descending (highest momentum first)
        results.sort_by(|a, b| b.1.score.cmp(&a.1.score));

        results
    }

    /// 설정 반환.
    pub fn config(&self) -> &MomentumConfig {
        &self.config
    }
}

/// 가중 모멘텀 계산기.
///
/// 서로 다른 시간 기간에 서로 다른 가중치를 적용하는 전략에서 사용됩니다.
#[derive(Debug, Clone)]
pub struct WeightedMomentumCalculator {
    config: WeightedMomentumConfig,
}

impl WeightedMomentumCalculator {
    /// 새 가중 모멘텀 계산기 생성.
    pub fn new(config: WeightedMomentumConfig) -> Self {
        Self { config }
    }

    /// 인피니티 봇 스타일 가중치로 생성.
    ///
    /// 사용: 10개월 * 0.3 + 100일 * 0.2 + 10일 * 0.3
    pub fn infinity_bot_style() -> Self {
        Self::new(WeightedMomentumConfig::default())
    }

    /// 사용자 정의 기간-가중치 쌍으로 생성.
    ///
    /// # 인수
    /// * `period_weights` - (일수, 가중치) 튜플의 슬라이스.
    pub fn with_weights(period_weights: &[(usize, Decimal)]) -> Self {
        Self::new(WeightedMomentumConfig {
            period_weights: period_weights.to_vec(),
            normalize_weights: false,
        })
    }

    /// 가중 모멘텀 계산.
    ///
    /// # 인수
    /// * `prices` - 가장 최근 가격이 먼저 오는 가격 시리즈.
    pub fn calculate(&self, prices: &[Decimal]) -> MomentumResult {
        if prices.is_empty() {
            return MomentumResult::invalid("Empty price series");
        }

        let current_price = prices[0];
        if current_price.is_zero() {
            return MomentumResult::invalid("Current price is zero");
        }

        let mut period_scores = Vec::new();
        let mut weighted_sum = Decimal::ZERO;
        let mut total_weight = Decimal::ZERO;
        let mut valid_count = 0usize;

        for &(period, weight) in &self.config.period_weights {
            if period < prices.len() {
                let past_price = prices[period];
                if !past_price.is_zero() {
                    let momentum = (current_price / past_price) - Decimal::ONE;

                    period_scores.push(MomentumScore {
                        period_days: period,
                        momentum,
                        start_price: past_price,
                        end_price: current_price,
                    });

                    weighted_sum += momentum * weight;
                    total_weight += weight;
                    valid_count += 1;
                }
            }
        }

        if valid_count == 0 {
            return MomentumResult::invalid("No valid periods could be calculated");
        }

        // Normalize if configured and total weight != 0
        let score = if self.config.normalize_weights && !total_weight.is_zero() {
            weighted_sum / total_weight
        } else {
            weighted_sum
        };

        MomentumResult {
            score,
            period_scores,
            valid_periods: valid_count,
            is_valid: true,
            error: None,
        }
    }

    /// 설정 반환.
    pub fn config(&self) -> &WeightedMomentumConfig {
        &self.config
    }
}

/// 단순 모멘텀 계산 (단일 기간).
///
/// # 인수
/// * `prices` - 가장 최근 가격이 먼저 오는 가격 시리즈.
/// * `period` - 거래일 기준 룩백 기간.
///
/// # 반환값
/// 소수점 형태의 모멘텀 (예: 0.05 = 5% 수익률).
pub fn simple_momentum(prices: &[Decimal], period: usize) -> Option<Decimal> {
    if prices.len() <= period {
        return None;
    }

    let current = prices[0];
    let past = prices[period];

    if past.is_zero() {
        return None;
    }

    Some((current / past) - Decimal::ONE)
}

/// 자산들 사이에서 모멘텀 백분위 순위 계산.
///
/// # 인수
/// * `scores` - 서로 다른 자산들의 모멘텀 점수 슬라이스.
/// * `target_score` - 백분위를 찾을 점수.
///
/// # 반환값
/// 백분위 순위 (0.0 ~ 1.0).
pub fn momentum_percentile(scores: &[Decimal], target_score: Decimal) -> Decimal {
    if scores.is_empty() {
        return Decimal::ZERO;
    }

    let count_below = scores.iter().filter(|&&s| s < target_score).count();
    Decimal::from(count_below) / Decimal::from(scores.len())
}

/// 모멘텀이 "위험 회피" 모드인지 확인.
///
/// 카나리아 자산 전략에서 사용됩니다 (예: HAA는 TIP을 카나리아로 사용).
///
/// # 인수
/// * `canary_momentum` - 카나리아 자산의 모멘텀.
///
/// # 반환값
/// 모멘텀이 음수(위험 회피)이면 `true`, 그렇지 않으면 `false`.
pub fn is_risk_off(canary_momentum: Decimal) -> bool {
    canary_momentum < Decimal::ZERO
}

/// 듀얼 모멘텀 시그널 계산.
///
/// 절대 모멘텀(무위험 수익률 대비)과 상대 모멘텀(벤치마크 대비)을 결합합니다.
///
/// # 인수
/// * `asset_momentum` - 자산의 절대 모멘텀.
/// * `benchmark_momentum` - 벤치마크의 모멘텀.
/// * `risk_free_rate` - 무위험 수익률 (연환산, 소수점).
///
/// # 반환값
/// 절대 모멘텀과 상대 모멘텀이 모두 양수이면 `true`.
pub fn dual_momentum_signal(
    asset_momentum: Decimal,
    benchmark_momentum: Decimal,
    risk_free_rate: Decimal,
) -> bool {
    // Absolute momentum: asset return > risk-free rate
    let absolute = asset_momentum > risk_free_rate;
    // Relative momentum: asset return > benchmark return
    let relative = asset_momentum > benchmark_momentum;

    absolute && relative
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_prices() -> Vec<Decimal> {
        // 300 days of prices, starting from today (index 0) going back
        // Simulating an uptrend: prices decrease as we go back
        (0..300)
            .map(|i| dec!(100) - Decimal::from(i) * dec!(0.1))
            .collect()
    }

    fn downtrend_prices() -> Vec<Decimal> {
        // Simulating a downtrend: prices increase as we go back (current is lower)
        (0..300)
            .map(|i| dec!(70) + Decimal::from(i) * dec!(0.1))
            .collect()
    }

    #[test]
    fn test_simple_momentum_positive() {
        let prices = sample_prices();
        // Current: 100, 21 days ago: 97.9
        // Momentum = 100/97.9 - 1 ≈ 0.0214
        let mom = simple_momentum(&prices, 21).unwrap();
        assert!(mom > Decimal::ZERO);
    }

    #[test]
    fn test_simple_momentum_negative() {
        let prices = downtrend_prices();
        // Current: 70, 21 days ago: 72.1
        // Momentum = 70/72.1 - 1 ≈ -0.029
        let mom = simple_momentum(&prices, 21).unwrap();
        assert!(mom < Decimal::ZERO);
    }

    #[test]
    fn test_momentum_calculator_standard() {
        let calc = MomentumCalculator::standard();
        let prices = sample_prices();
        let result = calc.calculate(&prices);

        assert!(result.is_valid);
        assert!(result.score > Decimal::ZERO);
        assert_eq!(result.valid_periods, 4); // 1, 3, 6, 12 months
    }

    #[test]
    fn test_momentum_calculator_insufficient_data() {
        let calc = MomentumCalculator::standard();
        let prices: Vec<Decimal> = (0..50).map(|i| Decimal::from(100 - i)).collect();
        let result = calc.calculate(&prices);

        // Only 50 days, but needs 252+1 for 12-month
        assert!(!result.is_valid || result.valid_periods < 4);
    }

    #[test]
    fn test_weighted_momentum() {
        let calc = WeightedMomentumCalculator::with_weights(&[
            (21, dec!(0.5)), // 1 month, 50% weight
            (63, dec!(0.5)), // 3 months, 50% weight
        ]);

        let prices = sample_prices();
        let result = calc.calculate(&prices);

        assert!(result.is_valid);
        assert_eq!(result.valid_periods, 2);
    }

    #[test]
    fn test_momentum_percentile() {
        let scores = vec![dec!(0.01), dec!(0.05), dec!(0.10), dec!(0.15), dec!(0.20)];

        // 0.10 is the 3rd value, so 2 values below it
        let percentile = momentum_percentile(&scores, dec!(0.10));
        assert_eq!(percentile, dec!(0.4)); // 2/5 = 0.4
    }

    #[test]
    fn test_rank_assets() {
        let calc = MomentumCalculator::with_months(&[1, 3]);

        let mut asset_prices = HashMap::new();

        // Asset A: strong uptrend
        asset_prices.insert(
            "A".to_string(),
            (0..100)
                .map(|i| dec!(100) - Decimal::from(i) * dec!(0.2))
                .collect(),
        );

        // Asset B: weak uptrend
        asset_prices.insert(
            "B".to_string(),
            (0..100)
                .map(|i| dec!(100) - Decimal::from(i) * dec!(0.05))
                .collect(),
        );

        // Asset C: downtrend
        asset_prices.insert(
            "C".to_string(),
            (0..100)
                .map(|i| dec!(80) + Decimal::from(i) * dec!(0.1))
                .collect(),
        );

        let ranked = calc.rank_assets(&asset_prices);

        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].0, "A"); // Highest momentum
        assert_eq!(ranked[1].0, "B");
        assert_eq!(ranked[2].0, "C"); // Lowest (negative) momentum
    }

    #[test]
    fn test_dual_momentum_signal() {
        // Asset outperforms both risk-free and benchmark
        assert!(dual_momentum_signal(dec!(0.10), dec!(0.05), dec!(0.02)));

        // Asset underperforms benchmark
        assert!(!dual_momentum_signal(dec!(0.03), dec!(0.05), dec!(0.02)));

        // Asset underperforms risk-free rate
        assert!(!dual_momentum_signal(dec!(0.01), dec!(0.00), dec!(0.02)));
    }

    #[test]
    fn test_is_risk_off() {
        assert!(is_risk_off(dec!(-0.05)));
        assert!(!is_risk_off(dec!(0.05)));
        assert!(!is_risk_off(Decimal::ZERO));
    }

    #[test]
    fn test_momentum_result_methods() {
        let result = MomentumResult {
            score: dec!(0.05),
            period_scores: vec![],
            valid_periods: 4,
            is_valid: true,
            error: None,
        };

        assert!(result.is_positive());
        assert!(!result.is_negative());
        assert_eq!(result.as_percentage(), dec!(5));
    }
}
