//! 진입 트리거 시스템.
//!
//! 여러 기술적 조건을 종합하여 진입 신호 강도와 트리거 라벨을 생성합니다.
//! Phase 1-B.2 구현.

use serde::{Deserialize, Serialize};
use std::fmt;

/// 트리거 유형.
///
/// 각 트리거는 특정 기술적 조건이 충족되었을 때 발생하며,
/// 고유한 점수를 가집니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    /// TTM Squeeze 해제 (+30점)
    ///
    /// Bollinger Bands가 Keltner Channel을 벗어나며
    /// 응축된 에너지가 폭발하는 신호입니다.
    SqueezeBreak,

    /// 박스권 돌파 (+25점)
    ///
    /// 일정 기간 횡보하던 가격이 저항선을 상향 돌파하는 신호입니다.
    BoxBreakout,

    /// 거래량 폭증 (+20점)
    ///
    /// 평균 거래량 대비 2배 이상 급증하는 신호입니다.
    VolumeSpike,

    /// 모멘텀 상승 (+15점)
    ///
    /// 단기 모멘텀이 상승 전환하는 신호입니다.
    MomentumUp,

    /// 망치형 캔들 (+10점)
    ///
    /// 하락 추세에서 긴 아래꼬리를 가진 반전 캔들입니다.
    HammerCandle,

    /// 장악형 캔들 (+10점)
    ///
    /// 이전 음봉을 완전히 감싸는 강한 양봉입니다.
    Engulfing,
}

impl TriggerType {
    /// 트리거 점수 반환.
    pub fn score(&self) -> f64 {
        match self {
            TriggerType::SqueezeBreak => 30.0,
            TriggerType::BoxBreakout => 25.0,
            TriggerType::VolumeSpike => 20.0,
            TriggerType::MomentumUp => 15.0,
            TriggerType::HammerCandle => 10.0,
            TriggerType::Engulfing => 10.0,
        }
    }

    /// 트리거 한글 이름 반환.
    pub fn name(&self) -> &str {
        match self {
            TriggerType::SqueezeBreak => "스퀴즈 해제",
            TriggerType::BoxBreakout => "박스권 돌파",
            TriggerType::VolumeSpike => "거래량 폭증",
            TriggerType::MomentumUp => "모멘텀 상승",
            TriggerType::HammerCandle => "망치형",
            TriggerType::Engulfing => "장악형",
        }
    }

    /// 트리거 이모지 반환.
    pub fn emoji(&self) -> &str {
        match self {
            TriggerType::SqueezeBreak => "🚀",
            TriggerType::BoxBreakout => "📦",
            TriggerType::VolumeSpike => "📊",
            TriggerType::MomentumUp => "⚡",
            TriggerType::HammerCandle => "🔨",
            TriggerType::Engulfing => "🎯",
        }
    }

    /// 이모지 + 이름 형식의 라벨 반환.
    pub fn label(&self) -> String {
        format!("{}{}", self.emoji(), self.name())
    }
}

impl fmt::Display for TriggerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// 트리거 결과.
///
/// 여러 트리거를 종합한 진입 신호 강도와 라벨을 제공합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerResult {
    /// 종합 점수 (0~100)
    ///
    /// 모든 활성화된 트리거의 점수 합계입니다.
    /// 100점을 초과할 수 있으며, 높을수록 강한 신호입니다.
    pub score: f64,

    /// 활성화된 트리거 목록
    ///
    /// 현재 충족된 트리거들의 리스트입니다.
    pub triggers: Vec<TriggerType>,

    /// 트리거 라벨
    ///
    /// 쉼표로 구분된 트리거 라벨 문자열입니다.
    /// 예: "🚀스퀴즈 해제, 📦박스권 돌파, 📊거래량 폭증"
    pub label: String,
}

impl TriggerResult {
    /// 새로운 트리거 결과 생성.
    ///
    /// # 인자
    /// * `triggers` - 활성화된 트리거 목록
    ///
    /// # 반환
    /// 점수와 라벨이 자동 계산된 TriggerResult
    pub fn new(triggers: Vec<TriggerType>) -> Self {
        let score = triggers.iter().map(|t| t.score()).sum();
        let label = if triggers.is_empty() {
            "진입 신호 없음".to_string()
        } else {
            triggers
                .iter()
                .map(|t| t.label())
                .collect::<Vec<_>>()
                .join(", ")
        };

        Self {
            score,
            triggers,
            label,
        }
    }

    /// 트리거가 없는 빈 결과 생성.
    pub fn empty() -> Self {
        Self {
            score: 0.0,
            triggers: Vec::new(),
            label: "진입 신호 없음".to_string(),
        }
    }

    /// 특정 트리거가 활성화되었는지 확인.
    pub fn has_trigger(&self, trigger_type: TriggerType) -> bool {
        self.triggers.contains(&trigger_type)
    }

    /// 트리거 개수 반환.
    pub fn count(&self) -> usize {
        self.triggers.len()
    }

    /// 강한 신호인지 판단 (점수 50 이상).
    pub fn is_strong(&self) -> bool {
        self.score >= 50.0
    }

    /// 중간 신호인지 판단 (점수 30~50).
    pub fn is_moderate(&self) -> bool {
        self.score >= 30.0 && self.score < 50.0
    }

    /// 약한 신호인지 판단 (점수 30 미만).
    pub fn is_weak(&self) -> bool {
        self.score < 30.0
    }

    /// 신호 강도 문자열 반환.
    pub fn strength_label(&self) -> &str {
        if self.is_strong() {
            "강함"
        } else if self.is_moderate() {
            "중간"
        } else if self.score > 0.0 {
            "약함"
        } else {
            "없음"
        }
    }
}

impl Default for TriggerResult {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_type_score() {
        assert_eq!(TriggerType::SqueezeBreak.score(), 30.0);
        assert_eq!(TriggerType::BoxBreakout.score(), 25.0);
        assert_eq!(TriggerType::VolumeSpike.score(), 20.0);
        assert_eq!(TriggerType::MomentumUp.score(), 15.0);
        assert_eq!(TriggerType::HammerCandle.score(), 10.0);
        assert_eq!(TriggerType::Engulfing.score(), 10.0);
    }

    #[test]
    fn test_trigger_type_label() {
        assert_eq!(TriggerType::SqueezeBreak.label(), "🚀스퀴즈 해제");
        assert_eq!(TriggerType::BoxBreakout.label(), "📦박스권 돌파");
        assert_eq!(TriggerType::VolumeSpike.label(), "📊거래량 폭증");
    }

    #[test]
    fn test_trigger_result_empty() {
        let result = TriggerResult::empty();
        assert_eq!(result.score, 0.0);
        assert_eq!(result.triggers.len(), 0);
        assert_eq!(result.label, "진입 신호 없음");
        assert!(!result.is_strong());
        assert!(!result.is_moderate());
        assert!(result.is_weak());
    }

    #[test]
    fn test_trigger_result_single() {
        let result = TriggerResult::new(vec![TriggerType::VolumeSpike]);
        assert_eq!(result.score, 20.0);
        assert_eq!(result.triggers.len(), 1);
        assert_eq!(result.label, "📊거래량 폭증");
        assert!(result.has_trigger(TriggerType::VolumeSpike));
        assert!(!result.has_trigger(TriggerType::BoxBreakout));
    }

    #[test]
    fn test_trigger_result_multiple() {
        let triggers = vec![
            TriggerType::SqueezeBreak,
            TriggerType::BoxBreakout,
            TriggerType::VolumeSpike,
        ];
        let result = TriggerResult::new(triggers);

        assert_eq!(result.score, 75.0); // 30 + 25 + 20
        assert_eq!(result.triggers.len(), 3);
        assert!(result.label.contains("🚀스퀴즈 해제"));
        assert!(result.label.contains("📦박스권 돌파"));
        assert!(result.label.contains("📊거래량 폭증"));
        assert!(result.is_strong());
    }

    #[test]
    fn test_trigger_strength_classification() {
        let strong = TriggerResult::new(vec![TriggerType::SqueezeBreak, TriggerType::BoxBreakout]);
        assert!(strong.is_strong());
        assert_eq!(strong.strength_label(), "강함");

        let moderate = TriggerResult::new(vec![TriggerType::BoxBreakout, TriggerType::MomentumUp]);
        assert!(moderate.is_moderate());
        assert_eq!(moderate.strength_label(), "중간");

        let weak = TriggerResult::new(vec![TriggerType::HammerCandle]);
        assert!(weak.is_weak());
        assert_eq!(weak.strength_label(), "약함");
    }

    #[test]
    fn test_trigger_result_default() {
        let result = TriggerResult::default();
        assert_eq!(result.score, 0.0);
        assert_eq!(result.triggers.len(), 0);
    }

    #[test]
    fn test_trigger_type_display() {
        assert_eq!(format!("{}", TriggerType::SqueezeBreak), "스퀴즈 해제");
        assert_eq!(format!("{}", TriggerType::BoxBreakout), "박스권 돌파");
    }
}
