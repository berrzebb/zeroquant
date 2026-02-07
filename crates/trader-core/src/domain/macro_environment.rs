//! MacroEnvironment - 매크로 환경 필터 시스템.
//!
//! USD/KRW 환율과 나스닥 지수 모니터링으로 시장 위험도를 평가하고,
//! 진입 기준(EBS)과 추천 종목 수를 동적으로 조정합니다.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 매크로 위험도 수준.
///
/// # 상태 설명
///
/// - **Critical**: 위기 상황 (환율 1400+ or 나스닥 -2%)
///   - EBS 기준 +1 강화
///   - 추천 종목 3개로 제한
///
/// - **High**: 고위험 (환율 전일 대비 +0.5% 급등)
///   - EBS 기준 +1 강화
///   - 추천 종목 5개로 제한
///
/// - **Normal**: 정상 시장
///   - 기본 EBS 기준 적용
///   - 추천 종목 제한 없음
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum MacroRisk {
    /// 위기 상황
    ///
    /// **조건**:
    /// - USD/KRW >= 1400 원
    /// - **OR** 나스닥 전일 대비 -2% 이상 하락
    ///
    /// **조치**:
    /// - EBS 기준 +1 (예: 3 → 4)
    /// - 추천 종목 최대 3개
    Critical,

    /// 고위험
    ///
    /// **조건**:
    /// - USD/KRW 전일 대비 +0.5% 이상 급등
    ///
    /// **조치**:
    /// - EBS 기준 +1
    /// - 추천 종목 최대 5개
    High,

    /// 정상
    ///
    /// **조건**:
    /// - Critical/High 조건 미충족
    ///
    /// **조치**:
    /// - 기본 EBS 기준 적용
    /// - 추천 종목 제한 없음
    #[default]
    Normal,
}

impl MacroRisk {
    /// EBS 기준 조정값 반환 (0, 1).
    ///
    /// Critical/High 시 +1, Normal 시 0.
    pub fn ebs_adjustment(self) -> u8 {
        match self {
            Self::Critical | Self::High => 1,
            Self::Normal => 0,
        }
    }

    /// 추천 종목 수 제한 반환.
    ///
    /// Critical: 3개, High: 5개, Normal: 무제한(usize::MAX).
    pub fn recommendation_limit(self) -> usize {
        match self {
            Self::Critical => 3,
            Self::High => 5,
            Self::Normal => usize::MAX,
        }
    }

    /// 위험 우선순위 (높을수록 위험).
    pub fn priority(self) -> u8 {
        match self {
            Self::Critical => 3,
            Self::High => 2,
            Self::Normal => 1,
        }
    }

    /// 진입 주의 필요 여부.
    pub fn needs_caution(self) -> bool {
        !matches!(self, Self::Normal)
    }

    /// 컬러 코드 (UI용).
    pub fn color_code(self) -> &'static str {
        match self {
            Self::Critical => "#ef4444", // 빨간색
            Self::High => "#f59e0b",     // 주황색
            Self::Normal => "#10b981",   // 녹색
        }
    }

    /// 아이콘 (UI용).
    pub fn icon(self) -> &'static str {
        match self {
            Self::Critical => "🚨",
            Self::High => "⚠️",
            Self::Normal => "✅",
        }
    }

    /// 설명 문자열.
    pub fn description(self) -> &'static str {
        match self {
            Self::Critical => "위기 상황 (환율 1400+ or 나스닥 -2%)",
            Self::High => "고위험 (환율 +0.5% 급등)",
            Self::Normal => "정상 시장",
        }
    }
}

impl fmt::Display for MacroRisk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Critical => "CRITICAL",
            Self::High => "HIGH",
            Self::Normal => "NORMAL",
        };
        write!(f, "{}", s)
    }
}


/// 매크로 환경 상태.
///
/// # 필드 설명
///
/// - `risk_level`: 현재 위험도 수준
/// - `usd_krw`: 현재 USD/KRW 환율
/// - `usd_change_pct`: 전일 대비 환율 변동률 (%)
/// - `nasdaq_change_pct`: 전일 대비 나스닥 변동률 (%)
/// - `adjusted_ebs`: 조정된 EBS 기준 (base_ebs + adjustment)
/// - `recommendation_limit`: 추천 종목 수 제한
///
/// # 사용 예시
///
/// ```rust,ignore
/// use trader_core::{MacroEnvironment, MacroRisk};
/// use rust_decimal::Decimal;
///
/// let env = MacroEnvironment::evaluate(
///     Decimal::from(1420),  // USD/KRW
///     0.8,                  // +0.8% 환율 상승
///     -2.5,                 // -2.5% 나스닥 하락
///     3,                    // 기본 EBS
/// );
///
/// assert_eq!(env.risk_level, MacroRisk::Critical);
/// assert_eq!(env.adjusted_ebs, 4);  // 3 + 1
/// assert_eq!(env.recommendation_limit, 3);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroEnvironment {
    /// 위험도 수준
    pub risk_level: MacroRisk,

    /// 현재 USD/KRW 환율
    pub usd_krw: Decimal,

    /// 전일 대비 환율 변동률 (%)
    pub usd_change_pct: f64,

    /// 전일 대비 나스닥 변동률 (%)
    pub nasdaq_change_pct: f64,

    /// 조정된 EBS 기준 (base_ebs + risk adjustment)
    pub adjusted_ebs: u8,

    /// 추천 종목 수 제한
    pub recommendation_limit: usize,
}

impl MacroEnvironment {
    /// 매크로 환경 평가 및 생성.
    ///
    /// # 파라미터
    ///
    /// - `usd_krw`: 현재 USD/KRW 환율
    /// - `usd_change_pct`: 전일 대비 환율 변동률 (%)
    /// - `nasdaq_change_pct`: 전일 대비 나스닥 변동률 (%)
    /// - `base_ebs`: 기본 EBS 기준값 (일반적으로 3)
    ///
    /// # 평가 로직
    ///
    /// 1. **Critical 판정**:
    ///    - USD/KRW >= 1400 원
    ///    - **OR** 나스닥 <= -2.0%
    ///
    /// 2. **High 판정** (Critical 아닐 때):
    ///    - USD/KRW 전일 대비 >= +0.5%
    ///
    /// 3. **Normal**: 위 조건 모두 미충족
    ///
    /// # 예시
    ///
    /// ```rust
    /// use trader_core::{MacroEnvironment, MacroRisk};
    /// use rust_decimal::Decimal;
    ///
    /// // 위기 상황: 환율 1420원
    /// let critical = MacroEnvironment::evaluate(
    ///     Decimal::from(1420),
    ///     0.3,
    ///     -1.0,
    ///     3,
    /// );
    /// assert_eq!(critical.risk_level, MacroRisk::Critical);
    /// assert_eq!(critical.adjusted_ebs, 4);
    ///
    /// // 고위험: 환율 +0.6% 급등
    /// let high = MacroEnvironment::evaluate(
    ///     Decimal::from(1350),
    ///     0.6,
    ///     -0.5,
    ///     3,
    /// );
    /// assert_eq!(high.risk_level, MacroRisk::High);
    /// assert_eq!(high.adjusted_ebs, 4);
    ///
    /// // 정상 시장
    /// let normal = MacroEnvironment::evaluate(
    ///     Decimal::from(1300),
    ///     0.2,
    ///     0.5,
    ///     3,
    /// );
    /// assert_eq!(normal.risk_level, MacroRisk::Normal);
    /// assert_eq!(normal.adjusted_ebs, 3);
    /// ```
    pub fn evaluate(
        usd_krw: Decimal,
        usd_change_pct: f64,
        nasdaq_change_pct: f64,
        base_ebs: u8,
    ) -> Self {
        // 위험도 평가
        let risk_level = Self::assess_risk(usd_krw, usd_change_pct, nasdaq_change_pct);

        // EBS 기준 조정
        let adjusted_ebs = base_ebs + risk_level.ebs_adjustment();

        // 추천 종목 수 제한
        let recommendation_limit = risk_level.recommendation_limit();

        Self {
            risk_level,
            usd_krw,
            usd_change_pct,
            nasdaq_change_pct,
            adjusted_ebs,
            recommendation_limit,
        }
    }

    /// 위험도 평가 로직.
    fn assess_risk(usd_krw: Decimal, usd_change_pct: f64, nasdaq_change_pct: f64) -> MacroRisk {
        // Critical 조건 체크
        if usd_krw >= Decimal::from(1400) || nasdaq_change_pct <= -2.0 {
            return MacroRisk::Critical;
        }

        // High 조건 체크 (환율 급등)
        if usd_change_pct >= 0.5 {
            return MacroRisk::High;
        }

        // Normal
        MacroRisk::Normal
    }

    /// 진입 주의 필요 여부.
    pub fn needs_caution(&self) -> bool {
        self.risk_level.needs_caution()
    }

    /// 요약 문자열 생성 (로그/알림용).
    ///
    /// # 예시
    ///
    /// ```text
    /// 매크로 환경: CRITICAL 🚨
    /// USD/KRW: 1420.00 (+0.8%)
    /// NASDAQ: -2.5%
    /// EBS 기준: 4 (기본 3 + 조정 1)
    /// 추천 제한: 3개
    /// ```
    pub fn summary(&self) -> String {
        format!(
            "매크로 환경: {} {}\nUSD/KRW: {} ({:+.2}%)\nNASDAQ: {:+.2}%\nEBS 기준: {} (기본 {} + 조정 {})\n추천 제한: {}개",
            self.risk_level,
            self.risk_level.icon(),
            self.usd_krw,
            self.usd_change_pct,
            self.nasdaq_change_pct,
            self.adjusted_ebs,
            self.adjusted_ebs - self.risk_level.ebs_adjustment(),
            self.risk_level.ebs_adjustment(),
            if self.recommendation_limit == usize::MAX {
                "없음".to_string()
            } else {
                self.recommendation_limit.to_string()
            }
        )
    }
}

impl Default for MacroEnvironment {
    /// 기본값: 정상 시장 상태 (USD/KRW 1300, 변동 없음, EBS 3).
    fn default() -> Self {
        Self {
            risk_level: MacroRisk::Normal,
            usd_krw: Decimal::from(1300),
            usd_change_pct: 0.0,
            nasdaq_change_pct: 0.0,
            adjusted_ebs: 3,
            recommendation_limit: usize::MAX,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macro_risk_critical_by_usd_krw() {
        let env = MacroEnvironment::evaluate(Decimal::from(1420), 0.3, -1.0, 3);
        assert_eq!(env.risk_level, MacroRisk::Critical);
        assert_eq!(env.adjusted_ebs, 4);
        assert_eq!(env.recommendation_limit, 3);
    }

    #[test]
    fn test_macro_risk_critical_by_nasdaq() {
        let env = MacroEnvironment::evaluate(Decimal::from(1300), 0.1, -2.5, 3);
        assert_eq!(env.risk_level, MacroRisk::Critical);
        assert_eq!(env.adjusted_ebs, 4);
        assert_eq!(env.recommendation_limit, 3);
    }

    #[test]
    fn test_macro_risk_high() {
        let env = MacroEnvironment::evaluate(Decimal::from(1350), 0.6, -0.5, 3);
        assert_eq!(env.risk_level, MacroRisk::High);
        assert_eq!(env.adjusted_ebs, 4);
        assert_eq!(env.recommendation_limit, 5);
    }

    #[test]
    fn test_macro_risk_normal() {
        let env = MacroEnvironment::evaluate(Decimal::from(1300), 0.2, 0.5, 3);
        assert_eq!(env.risk_level, MacroRisk::Normal);
        assert_eq!(env.adjusted_ebs, 3);
        assert_eq!(env.recommendation_limit, usize::MAX);
    }

    #[test]
    fn test_macro_risk_boundary_usd_1400() {
        let critical = MacroEnvironment::evaluate(Decimal::from(1400), 0.0, 0.0, 3);
        assert_eq!(critical.risk_level, MacroRisk::Critical);

        let normal = MacroEnvironment::evaluate(Decimal::from(1399), 0.0, 0.0, 3);
        assert_eq!(normal.risk_level, MacroRisk::Normal);
    }

    #[test]
    fn test_macro_risk_boundary_nasdaq_minus_2() {
        let critical = MacroEnvironment::evaluate(Decimal::from(1300), 0.0, -2.0, 3);
        assert_eq!(critical.risk_level, MacroRisk::Critical);

        let normal = MacroEnvironment::evaluate(Decimal::from(1300), 0.0, -1.9, 3);
        assert_eq!(normal.risk_level, MacroRisk::Normal);
    }

    #[test]
    fn test_macro_risk_boundary_usd_change_0_5() {
        let high = MacroEnvironment::evaluate(Decimal::from(1300), 0.5, 0.0, 3);
        assert_eq!(high.risk_level, MacroRisk::High);

        let normal = MacroEnvironment::evaluate(Decimal::from(1300), 0.4, 0.0, 3);
        assert_eq!(normal.risk_level, MacroRisk::Normal);
    }

    #[test]
    fn test_ebs_adjustment() {
        assert_eq!(MacroRisk::Critical.ebs_adjustment(), 1);
        assert_eq!(MacroRisk::High.ebs_adjustment(), 1);
        assert_eq!(MacroRisk::Normal.ebs_adjustment(), 0);
    }

    #[test]
    fn test_recommendation_limit() {
        assert_eq!(MacroRisk::Critical.recommendation_limit(), 3);
        assert_eq!(MacroRisk::High.recommendation_limit(), 5);
        assert_eq!(MacroRisk::Normal.recommendation_limit(), usize::MAX);
    }

    #[test]
    fn test_priority() {
        assert!(MacroRisk::Critical.priority() > MacroRisk::High.priority());
        assert!(MacroRisk::High.priority() > MacroRisk::Normal.priority());
    }

    #[test]
    fn test_needs_caution() {
        assert!(MacroRisk::Critical.needs_caution());
        assert!(MacroRisk::High.needs_caution());
        assert!(!MacroRisk::Normal.needs_caution());
    }

    #[test]
    fn test_display() {
        assert_eq!(MacroRisk::Critical.to_string(), "CRITICAL");
        assert_eq!(MacroRisk::High.to_string(), "HIGH");
        assert_eq!(MacroRisk::Normal.to_string(), "NORMAL");
    }

    #[test]
    fn test_default() {
        assert_eq!(MacroRisk::default(), MacroRisk::Normal);

        let env = MacroEnvironment::default();
        assert_eq!(env.risk_level, MacroRisk::Normal);
        assert_eq!(env.adjusted_ebs, 3);
    }

    #[test]
    fn test_summary_format() {
        let env = MacroEnvironment::evaluate(Decimal::from(1420), 0.8, -2.5, 3);
        let summary = env.summary();
        assert!(summary.contains("CRITICAL"));
        assert!(summary.contains("1420"));
        assert!(summary.contains("+0.80"));
        assert!(summary.contains("-2.50"));
        assert!(summary.contains("EBS 기준: 4"));
        assert!(summary.contains("3개"));
    }
}
