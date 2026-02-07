//! MarketRegime - 시장 레짐 분류 시스템.
//!
//! 종목의 추세 단계를 5단계로 분류하여 매매 타이밍을 판단합니다.

use serde::{Deserialize, Serialize};
use std::fmt;

/// 종목의 추세 단계를 나타내는 5단계 레짐.
///
/// # 상태 설명
///
/// - **StrongUptrend**: 강한 상승 추세 (rel_60d > 10 + slope > 0 + RSI 50~70)
/// - **Correction**: 상승 후 조정 (rel_60d > 5 + slope <= 0)
/// - **Sideways**: 박스권 / 중립 (-5 <= rel_60d <= 5)
/// - **BottomBounce**: 바닥 반등 시도 (rel_60d <= -5 + slope > 0)
/// - **Downtrend**: 하락 / 약세
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum MarketRegime {
    /// 강한 상승 추세
    ///
    /// **조건**:
    /// - 60일 상대강도 > 10%
    /// - 가격 기울기 > 0 (상승 중)
    /// - RSI 50~70 (건강한 모멘텀)
    StrongUptrend,

    /// 상승 후 조정
    ///
    /// **조건**:
    /// - 60일 상대강도 > 5%
    /// - 가격 기울기 <= 0 (조정 중)
    Correction,

    /// 박스권 / 중립
    ///
    /// **조건**:
    /// - -5% <= 60일 상대강도 <= 5%
    #[default]
    Sideways,

    /// 바닥 반등 시도
    ///
    /// **조건**:
    /// - 60일 상대강도 <= -5%
    /// - 가격 기울기 > 0 (반등 중)
    BottomBounce,

    /// 하락 / 약세
    ///
    /// **조건**:
    /// - 60일 상대강도 <= -5%
    /// - 가격 기울기 <= 0 (하락 중)
    Downtrend,
}

impl MarketRegime {
    /// 레짐 우선순위 (높을수록 유리)
    pub fn priority(self) -> u8 {
        match self {
            Self::StrongUptrend => 5,
            Self::BottomBounce => 4,
            Self::Sideways => 3,
            Self::Correction => 2,
            Self::Downtrend => 1,
        }
    }

    /// 진입 적합 여부
    pub fn is_entry_friendly(self) -> bool {
        matches!(self, Self::StrongUptrend | Self::BottomBounce)
    }

    /// 주의 필요 여부
    pub fn needs_caution(self) -> bool {
        matches!(self, Self::Correction | Self::Downtrend)
    }

    /// 컬러 코드 (UI용)
    pub fn color_code(self) -> &'static str {
        match self {
            Self::StrongUptrend => "#10b981", // 녹색
            Self::BottomBounce => "#3b82f6",  // 파란색
            Self::Sideways => "#6b7280",      // 회색
            Self::Correction => "#f59e0b",    // 주황색
            Self::Downtrend => "#ef4444",     // 빨간색
        }
    }

    /// 아이콘 (UI용)
    pub fn icon(self) -> &'static str {
        match self {
            Self::StrongUptrend => "📈",
            Self::BottomBounce => "🔄",
            Self::Sideways => "↔️",
            Self::Correction => "📉",
            Self::Downtrend => "⬇️",
        }
    }

    /// 설명 문자열
    pub fn description(self) -> &'static str {
        match self {
            Self::StrongUptrend => "강한 상승 추세",
            Self::Correction => "상승 후 조정",
            Self::Sideways => "박스권/중립",
            Self::BottomBounce => "바닥 반등 시도",
            Self::Downtrend => "하락/약세",
        }
    }
}

impl fmt::Display for MarketRegime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::StrongUptrend => "STRONG_UPTREND",
            Self::Correction => "CORRECTION",
            Self::Sideways => "SIDEWAYS",
            Self::BottomBounce => "BOTTOM_BOUNCE",
            Self::Downtrend => "DOWNTREND",
        };
        write!(f, "{}", s)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_order() {
        assert!(MarketRegime::StrongUptrend.priority() > MarketRegime::Correction.priority());
        assert!(MarketRegime::BottomBounce.priority() > MarketRegime::Sideways.priority());
        assert!(MarketRegime::Downtrend.priority() < MarketRegime::Sideways.priority());
    }

    #[test]
    fn test_entry_friendly() {
        assert!(MarketRegime::StrongUptrend.is_entry_friendly());
        assert!(MarketRegime::BottomBounce.is_entry_friendly());
        assert!(!MarketRegime::Correction.is_entry_friendly());
        assert!(!MarketRegime::Downtrend.is_entry_friendly());
    }

    #[test]
    fn test_needs_caution() {
        assert!(MarketRegime::Correction.needs_caution());
        assert!(MarketRegime::Downtrend.needs_caution());
        assert!(!MarketRegime::StrongUptrend.needs_caution());
    }

    #[test]
    fn test_display() {
        assert_eq!(MarketRegime::StrongUptrend.to_string(), "STRONG_UPTREND");
        assert_eq!(MarketRegime::Sideways.to_string(), "SIDEWAYS");
    }

    #[test]
    fn test_default() {
        assert_eq!(MarketRegime::default(), MarketRegime::Sideways);
    }
}
