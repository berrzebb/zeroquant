//! 7Factor 정규화 점수 계산기.
//!
//! 7개 팩터를 0-100 스케일로 정규화하여 레이더 차트 시각화를 지원합니다.
//!
//! # 7 Factors
//!
//! 1. **NORM_MOMENTUM** - 가격 모멘텀 (RSI, 최근 수익률)
//! 2. **NORM_VALUE** - 가치 평가 (PER, PBR, PSR)
//! 3. **NORM_QUALITY** - 수익성/품질 (ROE, ROA, 마진)
//! 4. **NORM_VOLATILITY** - 변동성 (낮을수록 높은 점수)
//! 5. **NORM_LIQUIDITY** - 유동성 (거래량, 거래대금)
//! 6. **NORM_GROWTH** - 성장성 (매출/이익 성장률)
//! 7. **NORM_SENTIMENT** - 시장 심리 (52주 위치, 최근 추세)

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ================================================================================================
// Types
// ================================================================================================

/// 7Factor 입력 데이터.
///
/// 기술적 분석 데이터와 펀더멘털 데이터를 모두 포함합니다.
#[derive(Debug, Clone, Default)]
pub struct SevenFactorInput {
    // 기술적 지표 (from GlobalScorer / IndicatorEngine)
    /// RSI (0-100)
    pub rsi: Option<Decimal>,
    /// 최근 5일 수익률 (%)
    pub return_5d: Option<Decimal>,
    /// 최근 20일 수익률 (%)
    pub return_20d: Option<Decimal>,
    /// ATR / 현재가 비율 (%)
    pub atr_pct: Option<Decimal>,
    /// 거래량 백분위 (0-100)
    pub volume_percentile: Option<Decimal>,
    /// 평균 거래대금 (원)
    pub avg_volume_amount: Option<Decimal>,

    // 펀더멘털 데이터 (from SymbolFundamental)
    /// PER (주가수익비율)
    pub per: Option<Decimal>,
    /// PBR (주가순자산비율)
    pub pbr: Option<Decimal>,
    /// PSR (주가매출비율)
    pub psr: Option<Decimal>,
    /// ROE (자기자본이익률, %)
    pub roe: Option<Decimal>,
    /// ROA (총자산이익률, %)
    pub roa: Option<Decimal>,
    /// 영업이익률 (%)
    pub operating_margin: Option<Decimal>,
    /// 순이익률 (%)
    pub net_profit_margin: Option<Decimal>,
    /// 매출 성장률 YoY (%)
    pub revenue_growth_yoy: Option<Decimal>,
    /// 순이익 성장률 YoY (%)
    pub earnings_growth_yoy: Option<Decimal>,

    // 가격 위치
    /// 52주 최고가
    pub week_52_high: Option<Decimal>,
    /// 52주 최저가
    pub week_52_low: Option<Decimal>,
    /// 현재가
    pub current_price: Option<Decimal>,
}

/// 7Factor 정규화 점수 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SevenFactorScores {
    /// 모멘텀 (0-100)
    pub norm_momentum: Decimal,
    /// 가치 (0-100)
    pub norm_value: Decimal,
    /// 품질 (0-100)
    pub norm_quality: Decimal,
    /// 변동성 (0-100, 낮은 변동성 = 높은 점수)
    pub norm_volatility: Decimal,
    /// 유동성 (0-100)
    pub norm_liquidity: Decimal,
    /// 성장성 (0-100)
    pub norm_growth: Decimal,
    /// 시장 심리 (0-100)
    pub norm_sentiment: Decimal,
}

impl SevenFactorScores {
    /// HashMap으로 변환 (component_scores에 추가용).
    pub fn to_hashmap(&self) -> HashMap<String, Decimal> {
        let mut map = HashMap::new();
        map.insert("NORM_MOMENTUM".to_string(), self.norm_momentum);
        map.insert("NORM_VALUE".to_string(), self.norm_value);
        map.insert("NORM_QUALITY".to_string(), self.norm_quality);
        map.insert("NORM_VOLATILITY".to_string(), self.norm_volatility);
        map.insert("NORM_LIQUIDITY".to_string(), self.norm_liquidity);
        map.insert("NORM_GROWTH".to_string(), self.norm_growth);
        map.insert("NORM_SENTIMENT".to_string(), self.norm_sentiment);
        map
    }

    /// 종합 점수 계산 (가중 평균).
    ///
    /// 기본 가중치: 모멘텀 0.2, 가치 0.15, 품질 0.2, 변동성 0.1, 유동성 0.1, 성장 0.15, 심리 0.1
    pub fn composite_score(&self) -> Decimal {
        let weights = [
            (self.norm_momentum, dec!(0.20)),
            (self.norm_value, dec!(0.15)),
            (self.norm_quality, dec!(0.20)),
            (self.norm_volatility, dec!(0.10)),
            (self.norm_liquidity, dec!(0.10)),
            (self.norm_growth, dec!(0.15)),
            (self.norm_sentiment, dec!(0.10)),
        ];

        weights.iter().map(|(score, weight)| *score * *weight).sum()
    }
}

impl Default for SevenFactorScores {
    fn default() -> Self {
        Self {
            norm_momentum: dec!(50),
            norm_value: dec!(50),
            norm_quality: dec!(50),
            norm_volatility: dec!(50),
            norm_liquidity: dec!(50),
            norm_growth: dec!(50),
            norm_sentiment: dec!(50),
        }
    }
}

// ================================================================================================
// Calculator
// ================================================================================================

/// 7Factor 계산기.
pub struct SevenFactorCalculator;

impl SevenFactorCalculator {
    /// 7Factor 점수 계산.
    ///
    /// # 인자
    ///
    /// * `input` - 기술적/펀더멘털 데이터
    ///
    /// # 반환
    ///
    /// 7개 정규화된 팩터 점수 (각각 0-100)
    pub fn calculate(input: &SevenFactorInput) -> SevenFactorScores {
        SevenFactorScores {
            norm_momentum: Self::calculate_momentum(input),
            norm_value: Self::calculate_value(input),
            norm_quality: Self::calculate_quality(input),
            norm_volatility: Self::calculate_volatility(input),
            norm_liquidity: Self::calculate_liquidity(input),
            norm_growth: Self::calculate_growth(input),
            norm_sentiment: Self::calculate_sentiment(input),
        }
    }

    /// 모멘텀 팩터 계산.
    ///
    /// RSI와 최근 수익률을 조합하여 모멘텀 점수 산출.
    /// - RSI 40-60: 중립 → 50점
    /// - RSI > 70: 과열 → 30-50점 (상승 모멘텀 있지만 과열)
    /// - RSI < 30: 과매도 → 70-90점 (반등 기대)
    /// - 최근 수익률 양호하면 가산
    fn calculate_momentum(input: &SevenFactorInput) -> Decimal {
        let mut score = dec!(50);
        let mut factors = 0;

        // RSI 기반 점수 (0-100 스케일, 중립=50)
        if let Some(rsi) = input.rsi {
            // RSI를 모멘텀 점수로 변환
            // RSI 50 → 50점, RSI 30 → 70점 (반등 기대), RSI 70 → 60점
            let rsi_score = if rsi < dec!(30) {
                // 과매도 - 반등 모멘텀 기대
                dec!(70) + (dec!(30) - rsi) * dec!(0.67)
            } else if rsi > dec!(70) {
                // 과매수 - 모멘텀은 있지만 조정 가능성
                dec!(60) - (rsi - dec!(70)) * dec!(0.33)
            } else {
                // 중립 구간 - RSI 그대로 사용
                rsi
            };
            score += rsi_score;
            factors += 1;
        }

        // 5일 수익률 반영
        if let Some(ret_5d) = input.return_5d {
            // -10% ~ +10% 범위를 0-100으로 매핑
            let ret_score = Self::normalize_range(ret_5d, dec!(-10), dec!(10), dec!(0), dec!(100));
            score += ret_score;
            factors += 1;
        }

        // 20일 수익률 반영
        if let Some(ret_20d) = input.return_20d {
            // -20% ~ +20% 범위를 0-100으로 매핑
            let ret_score = Self::normalize_range(ret_20d, dec!(-20), dec!(20), dec!(0), dec!(100));
            score += ret_score;
            factors += 1;
        }

        if factors > 0 {
            (score / Decimal::from(factors + 1))
                .min(dec!(100))
                .max(dec!(0))
        } else {
            dec!(50)
        }
    }

    /// 가치 팩터 계산.
    ///
    /// PER, PBR, PSR이 낮을수록 높은 점수.
    fn calculate_value(input: &SevenFactorInput) -> Decimal {
        let mut score = dec!(0);
        let mut factors = 0;

        // PER 점수 (낮을수록 좋음)
        // PER 5 → 100점, PER 15 → 50점, PER 30+ → 0점
        if let Some(per) = input.per {
            if per > dec!(0) {
                let per_score = Self::normalize_range(per, dec!(5), dec!(30), dec!(100), dec!(0));
                score += per_score;
                factors += 1;
            }
        }

        // PBR 점수 (낮을수록 좋음)
        // PBR 0.5 → 100점, PBR 1.5 → 50점, PBR 3.0+ → 0점
        if let Some(pbr) = input.pbr {
            if pbr > dec!(0) {
                let pbr_score =
                    Self::normalize_range(pbr, dec!(0.5), dec!(3.0), dec!(100), dec!(0));
                score += pbr_score;
                factors += 1;
            }
        }

        // PSR 점수 (낮을수록 좋음)
        // PSR 0.5 → 100점, PSR 3.0 → 50점, PSR 10+ → 0점
        if let Some(psr) = input.psr {
            if psr > dec!(0) {
                let psr_score =
                    Self::normalize_range(psr, dec!(0.5), dec!(10.0), dec!(100), dec!(0));
                score += psr_score;
                factors += 1;
            }
        }

        if factors > 0 {
            (score / Decimal::from(factors)).min(dec!(100)).max(dec!(0))
        } else {
            dec!(50)
        }
    }

    /// 품질 팩터 계산.
    ///
    /// ROE, ROA, 마진이 높을수록 높은 점수.
    fn calculate_quality(input: &SevenFactorInput) -> Decimal {
        let mut score = dec!(0);
        let mut factors = 0;

        // ROE 점수 (높을수록 좋음)
        // ROE 5% → 25점, ROE 15% → 75점, ROE 25%+ → 100점
        if let Some(roe) = input.roe {
            let roe_score = Self::normalize_range(roe, dec!(0), dec!(25), dec!(0), dec!(100));
            score += roe_score;
            factors += 1;
        }

        // ROA 점수 (높을수록 좋음)
        // ROA 2% → 25점, ROA 8% → 75점, ROA 15%+ → 100점
        if let Some(roa) = input.roa {
            let roa_score = Self::normalize_range(roa, dec!(0), dec!(15), dec!(0), dec!(100));
            score += roa_score;
            factors += 1;
        }

        // 영업이익률 점수
        // 영업이익률 5% → 25점, 15% → 75점, 25%+ → 100점
        if let Some(margin) = input.operating_margin {
            let margin_score = Self::normalize_range(margin, dec!(0), dec!(25), dec!(0), dec!(100));
            score += margin_score;
            factors += 1;
        }

        // 순이익률 점수
        if let Some(npm) = input.net_profit_margin {
            let npm_score = Self::normalize_range(npm, dec!(0), dec!(20), dec!(0), dec!(100));
            score += npm_score;
            factors += 1;
        }

        if factors > 0 {
            (score / Decimal::from(factors)).min(dec!(100)).max(dec!(0))
        } else {
            dec!(50)
        }
    }

    /// 변동성 팩터 계산.
    ///
    /// 낮은 변동성 = 높은 점수 (안정적).
    fn calculate_volatility(input: &SevenFactorInput) -> Decimal {
        // ATR% 기반 점수 (낮을수록 좋음)
        // ATR% 1% → 100점, ATR% 3% → 50점, ATR% 5%+ → 0점
        if let Some(atr_pct) = input.atr_pct {
            let vol_score = Self::normalize_range(atr_pct, dec!(1), dec!(5), dec!(100), dec!(0));
            return vol_score.min(dec!(100)).max(dec!(0));
        }

        dec!(50)
    }

    /// 유동성 팩터 계산.
    ///
    /// 거래량 백분위와 거래대금 기반.
    fn calculate_liquidity(input: &SevenFactorInput) -> Decimal {
        let mut score = dec!(0);
        let mut factors = 0;

        // 거래량 백분위 (그대로 점수로 사용)
        if let Some(vol_pct) = input.volume_percentile {
            score += vol_pct;
            factors += 1;
        }

        // 평균 거래대금 기반 점수
        // 1억 → 30점, 10억 → 70점, 50억+ → 100점
        if let Some(amount) = input.avg_volume_amount {
            let amount_billion = amount / dec!(100_000_000); // 억 단위로 변환
            let amount_score =
                Self::normalize_range(amount_billion, dec!(1), dec!(50), dec!(30), dec!(100));
            score += amount_score;
            factors += 1;
        }

        if factors > 0 {
            (score / Decimal::from(factors)).min(dec!(100)).max(dec!(0))
        } else {
            dec!(50)
        }
    }

    /// 성장 팩터 계산.
    ///
    /// 매출/이익 성장률 기반.
    fn calculate_growth(input: &SevenFactorInput) -> Decimal {
        let mut score = dec!(0);
        let mut factors = 0;

        // 매출 성장률 (YoY)
        // 0% → 30점, 15% → 70점, 30%+ → 100점
        if let Some(rev_growth) = input.revenue_growth_yoy {
            let rev_score =
                Self::normalize_range(rev_growth, dec!(-10), dec!(30), dec!(0), dec!(100));
            score += rev_score;
            factors += 1;
        }

        // 이익 성장률 (YoY)
        // 0% → 30점, 20% → 70점, 50%+ → 100점
        if let Some(earn_growth) = input.earnings_growth_yoy {
            let earn_score =
                Self::normalize_range(earn_growth, dec!(-20), dec!(50), dec!(0), dec!(100));
            score += earn_score;
            factors += 1;
        }

        if factors > 0 {
            (score / Decimal::from(factors)).min(dec!(100)).max(dec!(0))
        } else {
            dec!(50)
        }
    }

    /// 시장 심리 팩터 계산.
    ///
    /// 52주 가격 위치와 최근 추세 기반.
    fn calculate_sentiment(input: &SevenFactorInput) -> Decimal {
        // 52주 고점/저점 대비 현재 위치
        if let (Some(high), Some(low), Some(current)) =
            (input.week_52_high, input.week_52_low, input.current_price)
        {
            if high > low && high > dec!(0) {
                // 0 = 52주 저점, 100 = 52주 고점
                let range = high - low;
                let position = (current - low) / range * dec!(100);

                // 극단적 위치 조정
                // 20% 아래 (저점 근처): 반등 기대 → 점수 상승
                // 80% 위 (고점 근처): 조정 가능성 → 점수 하락
                let sentiment = if position < dec!(20) {
                    dec!(70) + (dec!(20) - position) * dec!(1.5)
                } else if position > dec!(80) {
                    dec!(60) - (position - dec!(80)) * dec!(1.5)
                } else {
                    position
                };

                return sentiment.min(dec!(100)).max(dec!(0));
            }
        }

        dec!(50)
    }

    /// 범위 정규화 헬퍼.
    ///
    /// `value`를 `[in_min, in_max]` 범위에서 `[out_min, out_max]` 범위로 선형 변환.
    fn normalize_range(
        value: Decimal,
        in_min: Decimal,
        in_max: Decimal,
        out_min: Decimal,
        out_max: Decimal,
    ) -> Decimal {
        if in_max == in_min {
            return (out_min + out_max) / dec!(2);
        }

        let clamped = value.max(in_min).min(in_max);
        let ratio = (clamped - in_min) / (in_max - in_min);
        out_min + ratio * (out_max - out_min)
    }
}

// ================================================================================================
// Tests
// ================================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_scores() {
        let scores = SevenFactorScores::default();
        assert_eq!(scores.norm_momentum, dec!(50));
        assert_eq!(scores.norm_value, dec!(50));
        assert_eq!(scores.composite_score(), dec!(50));
    }

    #[test]
    fn test_momentum_calculation() {
        // RSI 과매도 상황
        let input = SevenFactorInput {
            rsi: Some(dec!(25)),
            return_5d: Some(dec!(5)),
            ..Default::default()
        };
        let scores = SevenFactorCalculator::calculate(&input);
        assert!(
            scores.norm_momentum > dec!(50),
            "과매도 RSI는 반등 기대로 50점 이상이어야 함"
        );
    }

    #[test]
    fn test_value_calculation() {
        // 저평가 종목
        let input = SevenFactorInput {
            per: Some(dec!(8)),
            pbr: Some(dec!(0.8)),
            ..Default::default()
        };
        let scores = SevenFactorCalculator::calculate(&input);
        assert!(scores.norm_value > dec!(60), "저PER/저PBR은 높은 가치 점수");
    }

    #[test]
    fn test_quality_calculation() {
        // 고품질 종목
        let input = SevenFactorInput {
            roe: Some(dec!(20)),
            roa: Some(dec!(10)),
            operating_margin: Some(dec!(15)),
            ..Default::default()
        };
        let scores = SevenFactorCalculator::calculate(&input);
        assert!(
            scores.norm_quality > dec!(60),
            "높은 ROE/ROA는 높은 품질 점수"
        );
    }

    #[test]
    fn test_volatility_calculation() {
        // 저변동성 종목
        let input = SevenFactorInput {
            atr_pct: Some(dec!(1.5)),
            ..Default::default()
        };
        let scores = SevenFactorCalculator::calculate(&input);
        assert!(
            scores.norm_volatility > dec!(70),
            "낮은 ATR%는 높은 안정성 점수"
        );
    }

    #[test]
    fn test_growth_calculation() {
        // 고성장 종목
        let input = SevenFactorInput {
            revenue_growth_yoy: Some(dec!(25)),
            earnings_growth_yoy: Some(dec!(40)),
            ..Default::default()
        };
        let scores = SevenFactorCalculator::calculate(&input);
        assert!(
            scores.norm_growth > dec!(70),
            "높은 성장률은 높은 성장 점수"
        );
    }

    #[test]
    fn test_sentiment_at_52week_low() {
        // 52주 저점 근처
        let input = SevenFactorInput {
            week_52_high: Some(dec!(100)),
            week_52_low: Some(dec!(50)),
            current_price: Some(dec!(55)),
            ..Default::default()
        };
        let scores = SevenFactorCalculator::calculate(&input);
        assert!(
            scores.norm_sentiment > dec!(70),
            "52주 저점 근처는 반등 기대로 높은 심리 점수"
        );
    }

    #[test]
    fn test_to_hashmap() {
        let scores = SevenFactorScores {
            norm_momentum: dec!(60),
            norm_value: dec!(70),
            norm_quality: dec!(80),
            norm_volatility: dec!(50),
            norm_liquidity: dec!(40),
            norm_growth: dec!(55),
            norm_sentiment: dec!(65),
        };

        let map = scores.to_hashmap();
        assert_eq!(map.get("NORM_MOMENTUM"), Some(&dec!(60)));
        assert_eq!(map.get("NORM_VALUE"), Some(&dec!(70)));
        assert_eq!(map.len(), 7);
    }

    #[test]
    fn test_composite_score() {
        let scores = SevenFactorScores {
            norm_momentum: dec!(100),
            norm_value: dec!(100),
            norm_quality: dec!(100),
            norm_volatility: dec!(100),
            norm_liquidity: dec!(100),
            norm_growth: dec!(100),
            norm_sentiment: dec!(100),
        };
        assert_eq!(scores.composite_score(), dec!(100));

        let scores_zero = SevenFactorScores {
            norm_momentum: dec!(0),
            norm_value: dec!(0),
            norm_quality: dec!(0),
            norm_volatility: dec!(0),
            norm_liquidity: dec!(0),
            norm_growth: dec!(0),
            norm_sentiment: dec!(0),
        };
        assert_eq!(scores_zero.composite_score(), dec!(0));
    }
}
