//! RouteState - 종목의 매매 단계 분류 시스템.
//!
//! 종목의 현재 상태를 5단계로 분류하여 매매 타이밍을 판단합니다.
//!
//! **참고**: 현재 다른 에이전트에서 구현 중입니다.

use serde::{Deserialize, Serialize};
use std::fmt;

/// 종목의 매매 단계를 나타내는 5단계 상태.
///
/// # 상태 설명
///
/// - **Attack**: 진입 적기 - TTM Squeeze 해제 + 모멘텀 상승
/// - **Armed**: 대기 준비 - Squeeze 중 + 강한 기본 구조
/// - **Wait**: 관찰 중 - 정배열 유지, 저가 상승 추세
/// - **Overheat**: 과열 - 급등 후 조정 필요
/// - **Neutral**: 중립 - 명확한 신호 없음
#[allow(dead_code)] // TODO: 다른 에이전트에서 구현 중
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa-support", derive(utoipa::ToSchema))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum RouteState {
    /// 진입 적기 (Attack)
    ///
    /// **조건**:
    /// - TTM Squeeze 해제
    /// - 모멘텀 상승
    /// - RSI 45~65 (건강한 범위)
    /// - Range_Pos >= 0.8 (박스권 상단)
    ///
    /// **의미**: 돌파 직전, 매수 타이밍
    Attack,

    /// 대기 준비 (Armed)
    ///
    /// **조건**:
    /// - TTM Squeeze 중 (압축 상태)
    /// - MA20 위 또는 Vol_Quality >= 2.0 (강한 매집)
    ///
    /// **의미**: 에너지 축적 중, 관찰 필요
    Armed,

    /// 관찰 중 (Wait)
    ///
    /// **조건**:
    /// - 정배열 유지
    /// - MA 지지
    /// - Low_Trend > 0 (저가 상승)
    ///
    /// **의미**: 건강한 조정, 진입 기회 대기
    Wait,

    /// 과열 (Overheat)
    ///
    /// **조건**:
    /// - 5일 수익률 > 20%
    /// - 또는 RSI >= 75
    ///
    /// **의미**: 급등 후 조정 가능성, 청산 검토
    Overheat,

    /// 중립 (Neutral)
    ///
    /// **조건**: 위 4가지 상태 조건 미충족
    ///
    /// **의미**: 명확한 신호 없음, 관망
    #[default]
    Neutral,
}

impl RouteState {
    /// 상태의 우선순위를 반환합니다.
    ///
    /// 여러 조건이 동시에 충족될 경우 우선순위가 높은 상태를 선택합니다.
    ///
    /// # 우선순위
    ///
    /// 1. Overheat (가장 높음 - 위험 신호)
    /// 2. Attack (진입 기회)
    /// 3. Armed (준비 상태)
    /// 4. Wait (관찰)
    /// 5. Neutral (가장 낮음 - 기본값)
    pub fn priority(self) -> u8 {
        match self {
            RouteState::Overheat => 1,
            RouteState::Attack => 2,
            RouteState::Armed => 3,
            RouteState::Wait => 4,
            RouteState::Neutral => 5,
        }
    }

    /// 상태가 진입 가능한지 여부를 반환합니다.
    pub fn is_entry_ready(self) -> bool {
        matches!(self, RouteState::Attack)
    }

    /// 상태가 청산 검토가 필요한지 여부를 반환합니다.
    pub fn needs_exit_review(self) -> bool {
        matches!(self, RouteState::Overheat)
    }

    /// 상태의 색상 코드를 반환합니다 (UI 표시용).
    pub fn color_code(self) -> &'static str {
        match self {
            RouteState::Attack => "#22c55e",   // 초록 (진입)
            RouteState::Armed => "#eab308",    // 노랑 (대기)
            RouteState::Wait => "#3b82f6",     // 파랑 (관찰)
            RouteState::Overheat => "#ef4444", // 빨강 (과열)
            RouteState::Neutral => "#6b7280",  // 회색 (중립)
        }
    }

    /// 상태의 아이콘 이모지를 반환합니다.
    pub fn icon(self) -> &'static str {
        match self {
            RouteState::Attack => "🚀",
            RouteState::Armed => "⚡",
            RouteState::Wait => "👀",
            RouteState::Overheat => "🔥",
            RouteState::Neutral => "😐",
        }
    }
}

impl fmt::Display for RouteState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            RouteState::Attack => "ATTACK",
            RouteState::Armed => "ARMED",
            RouteState::Wait => "WAIT",
            RouteState::Overheat => "OVERHEAT",
            RouteState::Neutral => "NEUTRAL",
        };
        write!(f, "{}", s)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_order() {
        assert!(RouteState::Overheat.priority() < RouteState::Attack.priority());
        assert!(RouteState::Attack.priority() < RouteState::Armed.priority());
        assert!(RouteState::Armed.priority() < RouteState::Wait.priority());
        assert!(RouteState::Wait.priority() < RouteState::Neutral.priority());
    }

    #[test]
    fn test_entry_ready() {
        assert!(RouteState::Attack.is_entry_ready());
        assert!(!RouteState::Armed.is_entry_ready());
        assert!(!RouteState::Neutral.is_entry_ready());
    }

    #[test]
    fn test_needs_exit_review() {
        assert!(RouteState::Overheat.needs_exit_review());
        assert!(!RouteState::Attack.needs_exit_review());
        assert!(!RouteState::Neutral.needs_exit_review());
    }

    #[test]
    fn test_display() {
        assert_eq!(RouteState::Attack.to_string(), "ATTACK");
        assert_eq!(RouteState::Overheat.to_string(), "OVERHEAT");
    }

    #[test]
    fn test_default() {
        assert_eq!(RouteState::default(), RouteState::Neutral);
    }
}
