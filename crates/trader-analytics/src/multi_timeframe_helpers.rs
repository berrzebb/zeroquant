//! 다중 타임프레임 분석 헬퍼 함수.
//!
//! 여러 타임프레임의 데이터를 종합하여 트렌드를 분석하고
//! 신호를 결합하는 유틸리티 함수를 제공합니다.
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! use trader_analytics::multi_timeframe_helpers::{analyze_trend, combine_signals, TrendDirection};
//! use trader_core::Kline;
//! use std::collections::HashMap;
//!
//! // 여러 타임프레임의 추세 분석
//! let trends = analyze_trend(&klines_map);
//!
//! // 신호 결합 (가중 평균)
//! let combined = combine_signals(&signal_scores, &weights);
//! ```

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use trader_core::{Kline, Timeframe};

/// 추세 방향.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendDirection {
    /// 강한 상승 추세
    StrongUptrend,
    /// 상승 추세
    Uptrend,
    /// 횡보
    Sideways,
    /// 하락 추세
    Downtrend,
    /// 강한 하락 추세
    StrongDowntrend,
}

impl TrendDirection {
    /// 추세를 숫자로 변환 (-2 ~ +2).
    pub fn score(&self) -> i32 {
        match self {
            TrendDirection::StrongUptrend => 2,
            TrendDirection::Uptrend => 1,
            TrendDirection::Sideways => 0,
            TrendDirection::Downtrend => -1,
            TrendDirection::StrongDowntrend => -2,
        }
    }

    /// 추세가 상승인지 확인.
    pub fn is_bullish(&self) -> bool {
        matches!(
            self,
            TrendDirection::StrongUptrend | TrendDirection::Uptrend
        )
    }

    /// 추세가 하락인지 확인.
    pub fn is_bearish(&self) -> bool {
        matches!(
            self,
            TrendDirection::StrongDowntrend | TrendDirection::Downtrend
        )
    }
}

/// 타임프레임별 추세 분석 결과.
#[derive(Debug, Clone)]
pub struct TrendAnalysis {
    /// 타임프레임별 추세
    pub trends: HashMap<Timeframe, TrendDirection>,
    /// 전체 합산 점수 (-10 ~ +10, 5개 TF 기준)
    pub total_score: i32,
    /// 모든 타임프레임이 같은 방향인지
    pub is_aligned: bool,
    /// 지배적인 추세 방향
    pub dominant_trend: TrendDirection,
}

/// 여러 타임프레임의 추세를 분석합니다.
///
/// SMA 기반으로 각 타임프레임의 추세를 판단합니다.
///
/// # 인자
///
/// * `klines_map` - 타임프레임별 캔들 데이터
/// * `sma_period` - SMA 기간 (기본값 20)
///
/// # 반환
///
/// 타임프레임별 추세 분석 결과
pub fn analyze_trend(
    klines_map: &HashMap<Timeframe, Vec<Kline>>,
    sma_period: usize,
) -> TrendAnalysis {
    let mut trends = HashMap::new();
    let mut total_score = 0;

    for (&timeframe, klines) in klines_map {
        if klines.len() < sma_period + 5 {
            continue;
        }

        let trend = analyze_single_timeframe_trend(klines, sma_period);
        total_score += trend.score();
        trends.insert(timeframe, trend);
    }

    // 지배적 추세 계산
    let dominant_trend = match total_score {
        s if s >= 4 => TrendDirection::StrongUptrend,
        s if s >= 2 => TrendDirection::Uptrend,
        s if s <= -4 => TrendDirection::StrongDowntrend,
        s if s <= -2 => TrendDirection::Downtrend,
        _ => TrendDirection::Sideways,
    };

    // 정렬 여부 확인 (모든 TF가 같은 방향)
    let is_aligned =
        trends.values().all(|t| t.is_bullish()) || trends.values().all(|t| t.is_bearish());

    TrendAnalysis {
        trends,
        total_score,
        is_aligned,
        dominant_trend,
    }
}

/// 단일 타임프레임의 추세를 분석합니다.
fn analyze_single_timeframe_trend(klines: &[Kline], sma_period: usize) -> TrendDirection {
    if klines.len() < sma_period + 5 {
        return TrendDirection::Sideways;
    }

    // 현재가
    let current_close = klines.last().map(|k| k.close).unwrap_or(Decimal::ZERO);

    // SMA 계산
    let sma: Decimal = klines
        .iter()
        .rev()
        .take(sma_period)
        .map(|k| k.close)
        .sum::<Decimal>()
        / Decimal::from(sma_period);

    // 5개 전 SMA (추세 기울기 확인용)
    let sma_5_ago: Decimal = klines
        .iter()
        .rev()
        .skip(5)
        .take(sma_period)
        .map(|k| k.close)
        .sum::<Decimal>()
        / Decimal::from(sma_period);

    // 현재가와 SMA 비교
    let price_vs_sma = (current_close - sma) / sma * dec!(100);

    // SMA 기울기 (5봉 전과 비교)
    let sma_slope = if sma_5_ago > Decimal::ZERO {
        (sma - sma_5_ago) / sma_5_ago * dec!(100)
    } else {
        Decimal::ZERO
    };

    // 추세 판단
    match (price_vs_sma, sma_slope) {
        (p, s) if p > dec!(3) && s > dec!(1) => TrendDirection::StrongUptrend,
        (p, s) if p > dec!(1) && s > dec!(0) => TrendDirection::Uptrend,
        (p, s) if p < dec!(-3) && s < dec!(-1) => TrendDirection::StrongDowntrend,
        (p, s) if p < dec!(-1) && s < dec!(0) => TrendDirection::Downtrend,
        _ => TrendDirection::Sideways,
    }
}

/// 신호 결합 결과.
#[derive(Debug, Clone)]
pub struct CombinedSignal {
    /// 결합된 신호 점수 (-1.0 ~ +1.0)
    pub score: Decimal,
    /// 신호 강도 (0.0 ~ 1.0)
    pub strength: Decimal,
    /// 신호 방향 (양수 = 매수, 음수 = 매도)
    pub direction: SignalDirection,
    /// 신뢰도 (0.0 ~ 1.0, 신호 간 일관성 기반)
    pub confidence: Decimal,
}

/// 신호 방향.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalDirection {
    /// 강한 매수
    StrongBuy,
    /// 매수
    Buy,
    /// 중립
    Neutral,
    /// 매도
    Sell,
    /// 강한 매도
    StrongSell,
}

impl SignalDirection {
    /// 점수로부터 방향 결정.
    pub fn from_score(score: Decimal) -> Self {
        if score >= dec!(0.6) {
            SignalDirection::StrongBuy
        } else if score >= dec!(0.2) {
            SignalDirection::Buy
        } else if score <= dec!(-0.6) {
            SignalDirection::StrongSell
        } else if score <= dec!(-0.2) {
            SignalDirection::Sell
        } else {
            SignalDirection::Neutral
        }
    }

    /// 매수 신호인지 확인.
    pub fn is_buy(&self) -> bool {
        matches!(self, SignalDirection::StrongBuy | SignalDirection::Buy)
    }

    /// 매도 신호인지 확인.
    pub fn is_sell(&self) -> bool {
        matches!(self, SignalDirection::StrongSell | SignalDirection::Sell)
    }
}

/// 여러 타임프레임의 신호를 결합합니다.
///
/// 가중 평균으로 신호를 결합하며, 신호 간 일관성에 따라 신뢰도를 계산합니다.
///
/// # 인자
///
/// * `signals` - 타임프레임별 신호 점수 (-1.0 ~ +1.0)
/// * `weights` - 타임프레임별 가중치 (합이 1.0이 아니어도 됨, 자동 정규화)
///
/// # 반환
///
/// 결합된 신호 정보
///
/// # 예시
///
/// ```rust,ignore
/// use trader_analytics::multi_timeframe_helpers::combine_signals;
/// use trader_core::Timeframe;
/// use rust_decimal_macros::dec;
///
/// let signals = HashMap::from([
///     (Timeframe::D1, dec!(0.8)),   // 일봉: 강한 매수
///     (Timeframe::H1, dec!(0.5)),   // 1시간봉: 매수
///     (Timeframe::M5, dec!(-0.2)),  // 5분봉: 약한 매도
/// ]);
///
/// let weights = HashMap::from([
///     (Timeframe::D1, dec!(0.5)),   // 일봉 가중치 높음
///     (Timeframe::H1, dec!(0.3)),
///     (Timeframe::M5, dec!(0.2)),
/// ]);
///
/// let combined = combine_signals(&signals, &weights);
/// println!("Combined score: {}", combined.score);
/// ```
pub fn combine_signals(
    signals: &HashMap<Timeframe, Decimal>,
    weights: &HashMap<Timeframe, Decimal>,
) -> CombinedSignal {
    if signals.is_empty() {
        return CombinedSignal {
            score: Decimal::ZERO,
            strength: Decimal::ZERO,
            direction: SignalDirection::Neutral,
            confidence: Decimal::ZERO,
        };
    }

    // 가중치 정규화
    let total_weight: Decimal = signals
        .keys()
        .map(|tf| weights.get(tf).copied().unwrap_or(dec!(1)))
        .sum();

    if total_weight == Decimal::ZERO {
        return CombinedSignal {
            score: Decimal::ZERO,
            strength: Decimal::ZERO,
            direction: SignalDirection::Neutral,
            confidence: Decimal::ZERO,
        };
    }

    // 가중 평균 계산
    let weighted_sum: Decimal = signals
        .iter()
        .map(|(tf, &score)| {
            let weight = weights.get(tf).copied().unwrap_or(dec!(1));
            score * weight
        })
        .sum();

    let score = weighted_sum / total_weight;

    // 강도 계산 (절대값)
    let strength = score.abs().min(dec!(1));

    // 신뢰도 계산 (신호 간 일관성)
    // 모든 신호가 같은 방향이면 신뢰도 높음
    let positive_count = signals.values().filter(|&&s| s > Decimal::ZERO).count();
    let negative_count = signals.values().filter(|&&s| s < Decimal::ZERO).count();
    let total_count = signals.len();

    let consistency = if total_count == 0 {
        Decimal::ZERO
    } else {
        let max_count = positive_count.max(negative_count);
        Decimal::from(max_count) / Decimal::from(total_count)
    };

    // 신뢰도 = 일관성 * 강도
    let confidence = (consistency * strength).min(dec!(1));

    let direction = SignalDirection::from_score(score);

    CombinedSignal {
        score,
        strength,
        direction,
        confidence,
    }
}

/// 타임프레임 우선순위에 따른 기본 가중치를 생성합니다.
///
/// 큰 타임프레임에 더 높은 가중치를 부여합니다.
///
/// # 인자
///
/// * `timeframes` - 타임프레임 목록
///
/// # 반환
///
/// 타임프레임별 가중치 (합 = 1.0)
pub fn default_weights(timeframes: &[Timeframe]) -> HashMap<Timeframe, Decimal> {
    if timeframes.is_empty() {
        return HashMap::new();
    }

    // 타임프레임 크기순 정렬 (큰 것 먼저)
    let mut sorted: Vec<_> = timeframes.to_vec();
    sorted.sort_by_key(|tf| std::cmp::Reverse(tf.duration().as_secs()));

    // 가중치 할당 (큰 TF에 높은 가중치)
    let total = sorted.len();
    let mut weights = HashMap::new();

    for (i, tf) in sorted.iter().enumerate() {
        // 선형 감소: 첫 번째 = 가장 큰 가중치
        let weight = Decimal::from(total - i);
        weights.insert(*tf, weight);
    }

    // 정규화
    let sum: Decimal = weights.values().sum();
    if sum > Decimal::ZERO {
        for weight in weights.values_mut() {
            *weight /= sum;
        }
    }

    weights
}

/// 타임프레임 간 다이버전스(괴리)를 감지합니다.
///
/// 큰 타임프레임과 작은 타임프레임의 추세가 다를 때 감지합니다.
///
/// # 인자
///
/// * `analysis` - 추세 분석 결과
/// * `higher_tf` - 상위 타임프레임
/// * `lower_tf` - 하위 타임프레임
///
/// # 반환
///
/// 다이버전스 유형 (Some = 다이버전스 존재, None = 추세 일치)
pub fn detect_divergence(
    analysis: &TrendAnalysis,
    higher_tf: Timeframe,
    lower_tf: Timeframe,
) -> Option<DivergenceType> {
    let higher_trend = analysis.trends.get(&higher_tf)?;
    let lower_trend = analysis.trends.get(&lower_tf)?;

    match (higher_trend.is_bullish(), lower_trend.is_bullish()) {
        (true, false) => Some(DivergenceType::BullishRetracement),
        (false, true) => Some(DivergenceType::BearishRetracement),
        _ => None,
    }
}

/// 다이버전스 유형.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DivergenceType {
    /// 상승 추세 중 조정 (큰 TF 상승, 작은 TF 하락)
    /// → 잠재적 매수 기회
    BullishRetracement,
    /// 하락 추세 중 반등 (큰 TF 하락, 작은 TF 상승)
    /// → 잠재적 매도 기회
    BearishRetracement,
}

// =============================================================================
// 테스트
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_kline(close: Decimal) -> Kline {
        Kline {
            ticker: "TEST".to_string(),
            timeframe: Timeframe::D1,
            open_time: Utc::now(),
            open: close,
            high: close + dec!(1),
            low: close - dec!(1),
            close,
            volume: dec!(1000),
            close_time: Utc::now(),
            quote_volume: None,
            num_trades: None,
        }
    }

    #[test]
    fn test_trend_direction_score() {
        assert_eq!(TrendDirection::StrongUptrend.score(), 2);
        assert_eq!(TrendDirection::Uptrend.score(), 1);
        assert_eq!(TrendDirection::Sideways.score(), 0);
        assert_eq!(TrendDirection::Downtrend.score(), -1);
        assert_eq!(TrendDirection::StrongDowntrend.score(), -2);
    }

    #[test]
    fn test_combine_signals_basic() {
        let signals = HashMap::from([
            (Timeframe::D1, dec!(0.8)),
            (Timeframe::H1, dec!(0.5)),
            (Timeframe::M5, dec!(0.2)),
        ]);

        let weights = HashMap::from([
            (Timeframe::D1, dec!(0.5)),
            (Timeframe::H1, dec!(0.3)),
            (Timeframe::M5, dec!(0.2)),
        ]);

        let combined = combine_signals(&signals, &weights);

        assert!(combined.score > Decimal::ZERO);
        assert!(combined.direction.is_buy());
        assert!(combined.confidence > Decimal::ZERO);
    }

    #[test]
    fn test_combine_signals_conflicting() {
        let signals = HashMap::from([(Timeframe::D1, dec!(0.8)), (Timeframe::H1, dec!(-0.8))]);

        let weights = HashMap::from([(Timeframe::D1, dec!(0.5)), (Timeframe::H1, dec!(0.5))]);

        let combined = combine_signals(&signals, &weights);

        // 상반된 신호 → 낮은 신뢰도
        assert!(combined.confidence < dec!(0.5));
    }

    #[test]
    fn test_default_weights() {
        let timeframes = vec![Timeframe::M5, Timeframe::H1, Timeframe::D1];
        let weights = default_weights(&timeframes);

        // 큰 TF에 더 높은 가중치
        assert!(weights.get(&Timeframe::D1) > weights.get(&Timeframe::H1));
        assert!(weights.get(&Timeframe::H1) > weights.get(&Timeframe::M5));

        // 합계 = 1
        let sum: Decimal = weights.values().sum();
        assert!((sum - dec!(1)).abs() < dec!(0.001));
    }

    #[test]
    fn test_signal_direction_from_score() {
        assert_eq!(
            SignalDirection::from_score(dec!(0.8)),
            SignalDirection::StrongBuy
        );
        assert_eq!(SignalDirection::from_score(dec!(0.3)), SignalDirection::Buy);
        assert_eq!(
            SignalDirection::from_score(dec!(0.0)),
            SignalDirection::Neutral
        );
        assert_eq!(
            SignalDirection::from_score(dec!(-0.3)),
            SignalDirection::Sell
        );
        assert_eq!(
            SignalDirection::from_score(dec!(-0.8)),
            SignalDirection::StrongSell
        );
    }
}
