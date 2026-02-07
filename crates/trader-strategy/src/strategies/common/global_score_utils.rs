//! GlobalScore 유틸리티 함수.
//!
//! StrategyContext에서 GlobalScore를 활용하여 종목을 선택하고 가중치를 계산합니다.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

use trader_core::domain::{GlobalScoreResult, StrategyContext};

/// GlobalScore 기반 종목 필터링 옵션.
#[derive(Debug, Clone)]
pub struct ScoreFilterOptions {
    /// 최소 점수 (0~100)
    pub min_score: Option<Decimal>,
    /// 필수 등급 (BUY, WATCH, HOLD, AVOID)
    pub required_grade: Option<String>,
    /// 최소 신뢰도 (HIGH, MEDIUM, LOW)
    pub min_confidence: Option<String>,
    /// 최대 반환 개수
    pub limit: Option<usize>,
}

impl Default for ScoreFilterOptions {
    fn default() -> Self {
        Self {
            min_score: Some(dec!(70)), // 기본 70점 이상
            required_grade: None,
            min_confidence: None,
            limit: Some(10), // 기본 상위 10개
        }
    }
}

/// StrategyContext에서 GlobalScore 기반 상위 종목 선택.
///
/// # 인자
///
/// * `context` - 전략 실행 컨텍스트
/// * `options` - 필터링 옵션
///
/// # 반환
///
/// (ticker, GlobalScoreResult) 쌍의 벡터 (overall_score DESC 정렬)
///
/// # 예시
///
/// ```rust,ignore
/// let options = ScoreFilterOptions {
///     min_score: Some(75.0),
///     required_grade: Some("BUY".to_string()),
///     limit: Some(5),
///     ..Default::default()
/// };
///
/// let top_tickers = select_top_tickers(&context, options);
/// for (ticker, score) in top_tickers {
///     println!("{}: {}", ticker, score.overall_score);
/// }
/// ```
pub fn select_top_tickers(
    context: &StrategyContext,
    options: ScoreFilterOptions,
) -> Vec<(String, GlobalScoreResult)> {
    let mut results: Vec<_> = context
        .global_scores
        .iter()
        .filter(|(_, score)| {
            // 최소 점수 필터
            if let Some(min) = options.min_score {
                if score.overall_score < min {
                    return false;
                }
            }

            // 등급 필터
            if let Some(ref required_grade) = options.required_grade {
                if &score.recommendation != required_grade {
                    return false;
                }
            }

            // 신뢰도 필터
            if let Some(ref min_conf) = options.min_confidence {
                let conf_level = if score.confidence >= dec!(0.8) {
                    "HIGH"
                } else if score.confidence >= dec!(0.6) {
                    "MEDIUM"
                } else {
                    "LOW"
                };

                // HIGH > MEDIUM > LOW 순서
                let required_level = match min_conf.as_str() {
                    "HIGH" => 2,
                    "MEDIUM" => 1,
                    _ => 0,
                };

                let current_level = match conf_level {
                    "HIGH" => 2,
                    "MEDIUM" => 1,
                    _ => 0,
                };

                if current_level < required_level {
                    return false;
                }
            }

            true
        })
        .map(|(sym, score)| (sym.clone(), score.clone()))
        .collect();

    // overall_score 내림차순 정렬
    results.sort_by(|a, b| {
        b.1.overall_score
            .partial_cmp(&a.1.overall_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // limit 적용
    if let Some(limit) = options.limit {
        results.truncate(limit);
    }

    results
}

/// 특정 종목의 GlobalScore 조회.
///
/// # 인자
///
/// * `context` - 전략 실행 컨텍스트
/// * `ticker` - 조회할 종목 티커
///
/// # 반환
///
/// GlobalScoreResult (없으면 None)
pub fn get_score<'a>(context: &'a StrategyContext, ticker: &str) -> Option<&'a GlobalScoreResult> {
    context.global_scores.get(ticker)
}

/// GlobalScore를 포지션 사이징 가중치로 변환.
///
/// # 점수별 가중치
///
/// - 90점 이상: 1.5x (최대)
/// - 80~90점: 1.2x
/// - 70~80점: 1.0x (기본)
/// - 60~70점: 0.8x
/// - 60점 미만: 0.5x (최소)
///
/// # 인자
///
/// * `score` - GlobalScore (0~100)
///
/// # 반환
///
/// 가중치 (0.5 ~ 1.5)
pub fn calculate_score_weight(score: Decimal) -> Decimal {
    if score >= dec!(90) {
        dec!(1.5)
    } else if score >= dec!(80) {
        dec!(1.2)
    } else if score >= dec!(70) {
        dec!(1.0)
    } else if score >= dec!(60) {
        dec!(0.8)
    } else {
        dec!(0.5)
    }
}

/// GlobalScore를 리스크 조정 계수로 변환.
///
/// 높은 점수일수록 리스크를 더 감수 (더 큰 포지션)
///
/// # 점수별 계수
///
/// - 90점 이상: 1.0 (리스크 그대로)
/// - 80~90점: 0.8
/// - 70~80점: 0.6
/// - 60~70점: 0.4
/// - 60점 미만: 0.2 (최소 리스크)
///
/// # 인자
///
/// * `score` - GlobalScore (0~100)
///
/// # 반환
///
/// 리스크 조정 계수 (0.2 ~ 1.0)
pub fn calculate_risk_adjustment(score: Decimal) -> Decimal {
    if score >= dec!(90) {
        dec!(1.0)
    } else if score >= dec!(80) {
        dec!(0.8)
    } else if score >= dec!(70) {
        dec!(0.6)
    } else if score >= dec!(60) {
        dec!(0.4)
    } else {
        dec!(0.2)
    }
}


/// GlobalScore를 Signal strength (f64)로 변환.
///
/// Signal.strength 필드에 사용되며, Executor의 min_strength 필터와
/// 포지션 사이징에 영향을 줍니다.
///
/// # 점수별 강도
///
/// - 90점 이상: 0.8 (강한 신호)
/// - 80~90점: 0.7
/// - 70~80점: 0.6 (기본)
/// - 60~70점: 0.5
/// - 60점 미만: 0.4 (약한 신호)
///
/// # 인자
///
/// * `score` - GlobalScore (0~100)
///
/// # 반환
///
/// Signal strength (0.4 ~ 0.8)
pub fn calculate_signal_strength(score: Decimal) -> f64 {
    if score >= dec!(90) {
        0.8
    } else if score >= dec!(80) {
        0.7
    } else if score >= dec!(70) {
        0.6
    } else if score >= dec!(60) {
        0.5
    } else {
        0.4
    }
}

/// GlobalScore와 전략 기본 강도를 결합하여 최종 Signal strength 계산.
///
/// 전략이 산출한 기본 강도와 GlobalScore를 결합하여
/// 최종 신호 강도를 계산합니다.
///
/// # 공식
///
/// `final_strength = base_strength * score_multiplier`
/// - score_multiplier: 0.75 ~ 1.25 (GlobalScore 기반)
///
/// # 인자
///
/// * `base_strength` - 전략이 계산한 기본 강도 (0.0 ~ 1.0)
/// * `global_score` - GlobalScore (0 ~ 100), None이면 기본 강도 반환
///
/// # 반환
///
/// 조정된 Signal strength (0.0 ~ 1.0 범위로 클램핑)
pub fn adjust_strength_by_score(base_strength: f64, global_score: Option<Decimal>) -> f64 {
    let Some(score) = global_score else {
        return base_strength;
    };

    // Score를 0.75 ~ 1.25 배율로 변환
    let multiplier = if score >= dec!(90) {
        1.25
    } else if score >= dec!(80) {
        1.15
    } else if score >= dec!(70) {
        1.0
    } else if score >= dec!(60) {
        0.85
    } else {
        0.75
    };

    // 최종 강도 계산 및 클램핑
    (base_strength * multiplier).clamp(0.0, 1.0)
}

/// 종목 그룹을 GlobalScore로 가중 평균.
///
/// 포트폴리오 리밸런싱 시 사용.
///
/// # 인자
///
/// * `scores` - 종목별 GlobalScore (ticker → result)
///
/// # 반환
///
/// 가중 평균 점수
pub fn calculate_weighted_average(scores: &HashMap<String, GlobalScoreResult>) -> Decimal {
    if scores.is_empty() {
        return Decimal::ZERO;
    }

    let total_score: Decimal = scores.values().map(|s| s.overall_score).sum();
    total_score / Decimal::from(scores.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use trader_core::types::MarketType;

    fn create_test_score(
        ticker: &str,
        score: Decimal,
        grade: &str,
        confidence: Decimal,
    ) -> (String, GlobalScoreResult) {
        let result = GlobalScoreResult {
            ticker: Some(ticker.to_string()),
            market_type: Some(MarketType::Stock),
            overall_score: score,
            component_scores: Default::default(),
            recommendation: grade.to_string(),
            confidence,
            timestamp: Utc::now(),
        };
        (ticker.to_string(), result)
    }

    #[test]
    fn test_calculate_score_weight() {
        assert_eq!(calculate_score_weight(dec!(95)), dec!(1.5));
        assert_eq!(calculate_score_weight(dec!(85)), dec!(1.2));
        assert_eq!(calculate_score_weight(dec!(75)), dec!(1.0));
        assert_eq!(calculate_score_weight(dec!(65)), dec!(0.8));
        assert_eq!(calculate_score_weight(dec!(50)), dec!(0.5));
    }

    #[test]
    fn test_calculate_risk_adjustment() {
        assert_eq!(calculate_risk_adjustment(dec!(95)), dec!(1.0));
        assert_eq!(calculate_risk_adjustment(dec!(85)), dec!(0.8));
        assert_eq!(calculate_risk_adjustment(dec!(75)), dec!(0.6));
        assert_eq!(calculate_risk_adjustment(dec!(65)), dec!(0.4));
        assert_eq!(calculate_risk_adjustment(dec!(50)), dec!(0.2));
    }

    #[test]
    fn test_select_top_tickers_with_min_score() {
        use std::collections::HashMap;

        let mut scores = HashMap::new();
        let (t1, s1) = create_test_score("A", dec!(90), "BUY", dec!(0.9));
        let (t2, s2) = create_test_score("B", dec!(75), "BUY", dec!(0.8));
        let (t3, s3) = create_test_score("C", dec!(60), "WATCH", dec!(0.7));
        scores.insert(t1, s1);
        scores.insert(t2, s2);
        scores.insert(t3, s3);

        let context = StrategyContext {
            global_scores: scores,
            ..Default::default()
        };

        let options = ScoreFilterOptions {
            min_score: Some(dec!(70)),
            required_grade: None,
            min_confidence: None,
            limit: None,
        };

        let results = select_top_tickers(&context, options);

        // 70점 이상만 선택됨
        assert_eq!(results.len(), 2);
        // 점수 순 정렬 확인
        assert_eq!(results[0].1.overall_score, dec!(90));
        assert_eq!(results[1].1.overall_score, dec!(75));
    }

    #[test]
    fn test_select_top_tickers_with_grade_filter() {
        use std::collections::HashMap;

        let mut scores = HashMap::new();
        let (t1, s1) = create_test_score("A", dec!(90), "BUY", dec!(0.9));
        let (t2, s2) = create_test_score("B", dec!(75), "BUY", dec!(0.8));
        let (t3, s3) = create_test_score("C", dec!(85), "WATCH", dec!(0.7));
        scores.insert(t1, s1);
        scores.insert(t2, s2);
        scores.insert(t3, s3);

        let context = StrategyContext {
            global_scores: scores,
            ..Default::default()
        };

        let options = ScoreFilterOptions {
            min_score: None,
            required_grade: Some("BUY".to_string()),
            min_confidence: None,
            limit: None,
        };

        let results = select_top_tickers(&context, options);

        // BUY 등급만 선택됨
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].1.recommendation, "BUY");
        assert_eq!(results[1].1.recommendation, "BUY");
    }
}
