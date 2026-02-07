//! 동적 슬리피지 모델.
//!
//! 다양한 시장 조건에 따라 슬리피지를 계산하는 모델을 제공합니다.
//!
//! # 지원 모델
//!
//! - **Fixed**: 고정 비율 슬리피지 (기본 0.05%)
//! - **Linear**: 기본 슬리피지 + 거래량 기반 시장 충격
//! - **VolatilityBased**: 변동성에 비례하는 슬리피지
//! - **Tiered**: 거래 금액 구간별 차등 슬리피지
//!
//! # 거래소 중립 설계
//!
//! 모든 모델은 거래소에 독립적으로 동작합니다.
//! Kline 데이터만으로 슬리피지를 계산할 수 있습니다.

use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use trader_core::{Kline, Side};

/// 슬리피지 모델.
///
/// 시장 상황에 따라 적절한 슬리피지를 계산합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SlippageModel {
    /// 고정 비율 슬리피지.
    ///
    /// 가장 단순한 모델로, 항상 동일한 비율을 적용합니다.
    Fixed {
        /// 슬리피지 비율 (예: 0.0005 = 0.05%)
        #[serde(default = "default_fixed_rate")]
        rate: Decimal,
    },

    /// 선형 시장 충격 모델.
    ///
    /// 기본 슬리피지에 거래 금액 대비 일일 거래량의 영향을 추가합니다.
    /// slippage = base + (order_value / daily_volume) * impact
    Linear {
        /// 기본 슬리피지 비율
        #[serde(default = "default_linear_base")]
        base: Decimal,
        /// 시장 충격 계수 (주문 크기 대비)
        #[serde(default = "default_linear_impact")]
        impact: Decimal,
    },

    /// 변동성 기반 슬리피지.
    ///
    /// ATR(Average True Range) 또는 캔들 범위를 기반으로 슬리피지를 계산합니다.
    /// slippage = (high - low) / close * multiplier
    VolatilityBased {
        /// 변동성 승수 (기본: 0.5 = 변동성의 50%)
        #[serde(default = "default_volatility_multiplier")]
        multiplier: f64,
        /// 최소 슬리피지 비율
        #[serde(default = "default_min_slippage")]
        min_rate: Decimal,
        /// 최대 슬리피지 비율
        #[serde(default = "default_max_slippage")]
        max_rate: Decimal,
    },

    /// 구간별 차등 슬리피지.
    ///
    /// 거래 금액 구간에 따라 다른 슬리피지 비율을 적용합니다.
    /// 대형 주문일수록 높은 슬리피지를 적용합니다.
    Tiered {
        /// 구간별 슬리피지 설정 [(금액 임계값, 비율)]
        /// 예: [(100000, 0.0003), (1000000, 0.0005), (MAX, 0.001)]
        tiers: Vec<SlippageTier>,
    },
}

/// 구간별 슬리피지 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageTier {
    /// 금액 임계값 (이 금액 이하에 적용)
    pub threshold: Decimal,
    /// 해당 구간의 슬리피지 비율
    pub rate: Decimal,
}

// 기본값 함수들
fn default_fixed_rate() -> Decimal {
    dec!(0.0005)
} // 0.05%
fn default_linear_base() -> Decimal {
    dec!(0.0003)
} // 0.03%
fn default_linear_impact() -> Decimal {
    dec!(0.1)
} // 10% 충격 계수
fn default_volatility_multiplier() -> f64 {
    0.5
}
fn default_min_slippage() -> Decimal {
    dec!(0.0001)
} // 0.01%
fn default_max_slippage() -> Decimal {
    dec!(0.01)
} // 1%

impl Default for SlippageModel {
    fn default() -> Self {
        Self::Fixed {
            rate: default_fixed_rate(),
        }
    }
}

impl SlippageModel {
    /// 고정 슬리피지 모델 생성.
    pub fn fixed(rate: Decimal) -> Self {
        Self::Fixed { rate }
    }

    /// 선형 시장 충격 모델 생성.
    pub fn linear(base: Decimal, impact: Decimal) -> Self {
        Self::Linear { base, impact }
    }

    /// 변동성 기반 모델 생성.
    pub fn volatility_based(multiplier: f64) -> Self {
        Self::VolatilityBased {
            multiplier,
            min_rate: default_min_slippage(),
            max_rate: default_max_slippage(),
        }
    }

    /// 구간별 모델 생성.
    pub fn tiered(tiers: Vec<(Decimal, Decimal)>) -> Self {
        Self::Tiered {
            tiers: tiers
                .into_iter()
                .map(|(threshold, rate)| SlippageTier { threshold, rate })
                .collect(),
        }
    }

    /// 슬리피지 계산.
    ///
    /// # Arguments
    /// * `price` - 기준 가격
    /// * `side` - 주문 방향 (Buy/Sell)
    /// * `order_value` - 주문 금액
    /// * `kline` - 현재 캔들 데이터 (변동성 계산용)
    ///
    /// # Returns
    /// 슬리피지가 적용된 실행 가격
    pub fn calculate_execution_price(
        &self,
        price: Decimal,
        side: Side,
        order_value: Decimal,
        kline: Option<&Kline>,
    ) -> SlippageResult {
        let slippage_rate = self.calculate_rate(price, order_value, kline);
        let slippage_amount = price * slippage_rate;

        let execution_price = match side {
            Side::Buy => price + slippage_amount,  // 매수는 높은 가격
            Side::Sell => price - slippage_amount, // 매도는 낮은 가격
        };

        SlippageResult {
            base_price: price,
            execution_price,
            slippage_rate,
            slippage_amount,
        }
    }

    /// 슬리피지 비율만 계산.
    pub fn calculate_rate(
        &self,
        _price: Decimal,
        order_value: Decimal,
        kline: Option<&Kline>,
    ) -> Decimal {
        match self {
            SlippageModel::Fixed { rate } => *rate,

            SlippageModel::Linear { base, impact } => {
                // 일일 거래량 기반 시장 충격
                let daily_volume_value = kline.map(|k| k.volume * k.close).unwrap_or(Decimal::MAX);

                if daily_volume_value > Decimal::ZERO {
                    let volume_impact = (order_value / daily_volume_value) * *impact;
                    *base + volume_impact
                } else {
                    *base
                }
            }

            SlippageModel::VolatilityBased {
                multiplier,
                min_rate,
                max_rate,
            } => {
                let volatility_rate = kline
                    .map(|k| {
                        if k.close > Decimal::ZERO {
                            (k.high - k.low) / k.close
                                * Decimal::from_f64(*multiplier).unwrap_or(dec!(0.5))
                        } else {
                            *min_rate
                        }
                    })
                    .unwrap_or(*min_rate);

                // 최소/최대 범위로 클램핑
                volatility_rate.max(*min_rate).min(*max_rate)
            }

            SlippageModel::Tiered { tiers } => {
                // 주문 금액에 해당하는 구간 찾기
                for tier in tiers {
                    if order_value <= tier.threshold {
                        return tier.rate;
                    }
                }
                // 모든 구간 초과 시 마지막 구간 사용
                tiers.last().map(|t| t.rate).unwrap_or(default_fixed_rate())
            }
        }
    }

    /// 모델 이름 반환.
    pub fn name(&self) -> &'static str {
        match self {
            SlippageModel::Fixed { .. } => "Fixed",
            SlippageModel::Linear { .. } => "Linear",
            SlippageModel::VolatilityBased { .. } => "VolatilityBased",
            SlippageModel::Tiered { .. } => "Tiered",
        }
    }
}

/// 슬리피지 계산 결과.
#[derive(Debug, Clone)]
pub struct SlippageResult {
    /// 기준 가격
    pub base_price: Decimal,
    /// 슬리피지 적용 후 실행 가격
    pub execution_price: Decimal,
    /// 적용된 슬리피지 비율
    pub slippage_rate: Decimal,
    /// 슬리피지 금액 (단위 가격 기준)
    pub slippage_amount: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_fixed_slippage() {
        let model = SlippageModel::fixed(dec!(0.001)); // 0.1%
        let result = model.calculate_execution_price(dec!(100), Side::Buy, dec!(10000), None);

        assert_eq!(result.base_price, dec!(100));
        assert_eq!(result.slippage_rate, dec!(0.001));
        assert_eq!(result.slippage_amount, dec!(0.1)); // 100 * 0.001
        assert_eq!(result.execution_price, dec!(100.1)); // 100 + 0.1
    }

    #[test]
    fn test_fixed_slippage_sell() {
        let model = SlippageModel::fixed(dec!(0.001));
        let result = model.calculate_execution_price(dec!(100), Side::Sell, dec!(10000), None);

        assert_eq!(result.execution_price, dec!(99.9)); // 100 - 0.1
    }

    #[test]
    fn test_tiered_slippage() {
        let model = SlippageModel::tiered(vec![
            (dec!(10000), dec!(0.0003)),  // < 10000: 0.03%
            (dec!(100000), dec!(0.0005)), // < 100000: 0.05%
            (dec!(1000000), dec!(0.001)), // < 1000000: 0.1%
        ]);

        // 소액 주문
        let small = model.calculate_rate(dec!(100), dec!(5000), None);
        assert_eq!(small, dec!(0.0003));

        // 중간 주문
        let medium = model.calculate_rate(dec!(100), dec!(50000), None);
        assert_eq!(medium, dec!(0.0005));

        // 대형 주문
        let large = model.calculate_rate(dec!(100), dec!(500000), None);
        assert_eq!(large, dec!(0.001));
    }

    #[test]
    fn test_default_model() {
        let model = SlippageModel::default();
        assert!(matches!(model, SlippageModel::Fixed { .. }));

        if let SlippageModel::Fixed { rate } = model {
            assert_eq!(rate, dec!(0.0005)); // 0.05%
        }
    }
}
