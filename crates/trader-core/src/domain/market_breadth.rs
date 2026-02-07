//! Market Breadth - 시장 온도 측정 시스템.
//!
//! 20일선 상회 종목 비율로 시장 전체 건강 상태를 측정합니다.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 시장 온도 (Market Temperature).
///
/// 20일선 상회 종목 비율로 시장 과열/냉각 상태를 판단합니다.
///
/// # 기준
///
/// - **Overheat**: >= 65% 🔥 (과열)
/// - **Neutral**: 35~65% 🌤 (중립)
/// - **Cold**: <= 35% 🧊 (냉각)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum MarketTemperature {
    /// 과열 (>= 65%)
    ///
    /// 시장이 과열되어 조정 가능성이 있습니다.
    /// 신규 진입보다는 보유 종목 관리에 집중해야 합니다.
    Overheat,

    /// 중립 (35~65%)
    ///
    /// 정상적인 시장 상태입니다.
    /// 선별적 매수가 가능합니다.
    #[default]
    Neutral,

    /// 냉각 (<= 35%)
    ///
    /// 시장이 약세이거나 조정 중입니다.
    /// 바닥 매수 기회를 찾거나 현금 비중을 높여야 합니다.
    Cold,
}

impl MarketTemperature {
    /// 비율로부터 시장 온도 판단.
    ///
    /// # Arguments
    ///
    /// * `ratio` - 20일선 상회 종목 비율 (0.0 ~ 1.0)
    ///
    /// # Returns
    ///
    /// 해당하는 MarketTemperature
    ///
    /// # Examples
    ///
    /// ```
    /// use trader_core::domain::MarketTemperature;
    /// use rust_decimal_macros::dec;
    ///
    /// let temp = MarketTemperature::from_ratio(dec!(0.70));
    /// assert_eq!(temp, MarketTemperature::Overheat);
    ///
    /// let temp = MarketTemperature::from_ratio(dec!(0.50));
    /// assert_eq!(temp, MarketTemperature::Neutral);
    ///
    /// let temp = MarketTemperature::from_ratio(dec!(0.30));
    /// assert_eq!(temp, MarketTemperature::Cold);
    /// ```
    pub fn from_ratio(ratio: Decimal) -> Self {
        let pct = ratio * Decimal::from(100);
        if pct >= Decimal::from(65) {
            Self::Overheat
        } else if pct >= Decimal::from(35) {
            Self::Neutral
        } else {
            Self::Cold
        }
    }

    /// 아이콘 (UI용).
    pub fn icon(self) -> &'static str {
        match self {
            Self::Overheat => "🔥",
            Self::Neutral => "🌤",
            Self::Cold => "🧊",
        }
    }

    /// 설명 문자열.
    pub fn description(self) -> &'static str {
        match self {
            Self::Overheat => "과열",
            Self::Neutral => "중립",
            Self::Cold => "냉각",
        }
    }

    /// 컬러 코드 (UI용).
    pub fn color_code(self) -> &'static str {
        match self {
            Self::Overheat => "#ef4444", // 빨간색
            Self::Neutral => "#3b82f6",  // 파란색
            Self::Cold => "#6b7280",     // 회색
        }
    }

    /// 매매 권장사항.
    pub fn recommendation(self) -> &'static str {
        match self {
            Self::Overheat => "시장 과열. 신규 진입 신중, 보유 종목 관리 집중",
            Self::Neutral => "정상 시장. 선별적 매수 가능",
            Self::Cold => "시장 냉각. 바닥 매수 기회 탐색 또는 현금 비중 확대",
        }
    }
}

impl fmt::Display for MarketTemperature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Overheat => "OVERHEAT",
            Self::Neutral => "NEUTRAL",
            Self::Cold => "COLD",
        };
        write!(f, "{}", s)
    }
}


/// Market Breadth - 시장 폭 지표.
///
/// 20일 이동평균선 상회 종목 비율로 시장 건강도를 측정합니다.
///
/// # 필드
///
/// - `all`: 전체 시장 (KOSPI + KOSDAQ)
/// - `kospi`: KOSPI 시장
/// - `kosdaq`: KOSDAQ 시장
/// - `temperature`: 시장 온도 (전체 기준)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketBreadth {
    /// 전체 시장 Above_MA20 비율 (0.0 ~ 1.0).
    pub all: Decimal,

    /// KOSPI Above_MA20 비율 (0.0 ~ 1.0).
    pub kospi: Decimal,

    /// KOSDAQ Above_MA20 비율 (0.0 ~ 1.0).
    pub kosdaq: Decimal,

    /// 시장 온도 (전체 기준).
    pub temperature: MarketTemperature,

    /// 계산 시각.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub calculated_at: chrono::DateTime<chrono::Utc>,
}

impl MarketBreadth {
    /// 새로운 MarketBreadth 생성.
    ///
    /// # Arguments
    ///
    /// * `all` - 전체 시장 비율
    /// * `kospi` - KOSPI 비율
    /// * `kosdaq` - KOSDAQ 비율
    ///
    /// # Returns
    ///
    /// MarketBreadth 인스턴스
    pub fn new(all: Decimal, kospi: Decimal, kosdaq: Decimal) -> Self {
        let temperature = MarketTemperature::from_ratio(all);
        Self {
            all,
            kospi,
            kosdaq,
            temperature,
            calculated_at: chrono::Utc::now(),
        }
    }

    /// 전체 시장 비율 (백분율).
    pub fn all_pct(&self) -> Decimal {
        self.all * Decimal::from(100)
    }

    /// KOSPI 비율 (백분율).
    pub fn kospi_pct(&self) -> Decimal {
        self.kospi * Decimal::from(100)
    }

    /// KOSDAQ 비율 (백분율).
    pub fn kosdaq_pct(&self) -> Decimal {
        self.kosdaq * Decimal::from(100)
    }

    /// 시장이 건강한지 여부 (>= 50%).
    pub fn is_healthy(&self) -> bool {
        self.all >= Decimal::from_f32_retain(0.5).unwrap()
    }

    /// 시장이 약세인지 여부 (<= 35%).
    pub fn is_weak(&self) -> bool {
        self.all <= Decimal::from_f32_retain(0.35).unwrap()
    }

    /// 시장이 과열인지 여부 (>= 65%).
    pub fn is_overheated(&self) -> bool {
        self.all >= Decimal::from_f32_retain(0.65).unwrap()
    }
}

impl Default for MarketBreadth {
    /// 기본값 생성 (50% 중립 상태).
    fn default() -> Self {
        Self {
            all: Decimal::from_f32_retain(0.5).unwrap(),
            kospi: Decimal::from_f32_retain(0.5).unwrap(),
            kosdaq: Decimal::from_f32_retain(0.5).unwrap(),
            temperature: MarketTemperature::default(),
            calculated_at: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_market_temperature_from_ratio() {
        assert_eq!(
            MarketTemperature::from_ratio(dec!(0.70)),
            MarketTemperature::Overheat
        );
        assert_eq!(
            MarketTemperature::from_ratio(dec!(0.65)),
            MarketTemperature::Overheat
        );
        assert_eq!(
            MarketTemperature::from_ratio(dec!(0.50)),
            MarketTemperature::Neutral
        );
        assert_eq!(
            MarketTemperature::from_ratio(dec!(0.35)),
            MarketTemperature::Neutral
        );
        assert_eq!(
            MarketTemperature::from_ratio(dec!(0.30)),
            MarketTemperature::Cold
        );
    }

    #[test]
    fn test_market_temperature_display() {
        assert_eq!(MarketTemperature::Overheat.to_string(), "OVERHEAT");
        assert_eq!(MarketTemperature::Neutral.to_string(), "NEUTRAL");
        assert_eq!(MarketTemperature::Cold.to_string(), "COLD");
    }

    #[test]
    fn test_market_temperature_default() {
        assert_eq!(MarketTemperature::default(), MarketTemperature::Neutral);
    }

    #[test]
    fn test_market_breadth_new() {
        let breadth = MarketBreadth::new(dec!(0.55), dec!(0.52), dec!(0.58));
        assert_eq!(breadth.all, dec!(0.55));
        assert_eq!(breadth.kospi, dec!(0.52));
        assert_eq!(breadth.kosdaq, dec!(0.58));
        assert_eq!(breadth.temperature, MarketTemperature::Neutral);
    }

    #[test]
    fn test_market_breadth_pct() {
        let breadth = MarketBreadth::new(dec!(0.55), dec!(0.52), dec!(0.58));
        assert_eq!(breadth.all_pct(), dec!(55));
        assert_eq!(breadth.kospi_pct(), dec!(52));
        assert_eq!(breadth.kosdaq_pct(), dec!(58));
    }

    #[test]
    fn test_market_breadth_is_healthy() {
        let breadth = MarketBreadth::new(dec!(0.55), dec!(0.52), dec!(0.58));
        assert!(breadth.is_healthy());

        let breadth = MarketBreadth::new(dec!(0.30), dec!(0.28), dec!(0.32));
        assert!(!breadth.is_healthy());
    }

    #[test]
    fn test_market_breadth_is_weak() {
        let breadth = MarketBreadth::new(dec!(0.30), dec!(0.28), dec!(0.32));
        assert!(breadth.is_weak());

        let breadth = MarketBreadth::new(dec!(0.55), dec!(0.52), dec!(0.58));
        assert!(!breadth.is_weak());
    }

    #[test]
    fn test_market_breadth_is_overheated() {
        let breadth = MarketBreadth::new(dec!(0.70), dec!(0.68), dec!(0.72));
        assert!(breadth.is_overheated());

        let breadth = MarketBreadth::new(dec!(0.55), dec!(0.52), dec!(0.58));
        assert!(!breadth.is_overheated());
    }
}
