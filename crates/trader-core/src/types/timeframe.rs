//! 캔들스틱 데이터를 위한 타임프레임 정의.
//!
//! 이 모듈은 다양한 시간 간격을 나타내는 타임프레임 타입을 정의합니다.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

/// 캔들스틱 타임프레임.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa-support", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum Timeframe {
    /// 1분봉
    M1,
    /// 3분봉
    M3,
    /// 5분봉
    M5,
    /// 15분봉
    M15,
    /// 30분봉
    M30,
    /// 1시간봉
    H1,
    /// 2시간봉
    H2,
    /// 4시간봉
    H4,
    /// 6시간봉
    H6,
    /// 8시간봉
    H8,
    /// 12시간봉
    H12,
    /// 일봉
    D1,
    /// 3일봉
    D3,
    /// 주봉
    W1,
    /// 월봉
    MN1,
}

impl Timeframe {
    /// 이 타임프레임의 기간을 반환합니다.
    pub fn duration(&self) -> Duration {
        match self {
            Timeframe::M1 => Duration::from_secs(60),
            Timeframe::M3 => Duration::from_secs(3 * 60),
            Timeframe::M5 => Duration::from_secs(5 * 60),
            Timeframe::M15 => Duration::from_secs(15 * 60),
            Timeframe::M30 => Duration::from_secs(30 * 60),
            Timeframe::H1 => Duration::from_secs(60 * 60),
            Timeframe::H2 => Duration::from_secs(2 * 60 * 60),
            Timeframe::H4 => Duration::from_secs(4 * 60 * 60),
            Timeframe::H6 => Duration::from_secs(6 * 60 * 60),
            Timeframe::H8 => Duration::from_secs(8 * 60 * 60),
            Timeframe::H12 => Duration::from_secs(12 * 60 * 60),
            Timeframe::D1 => Duration::from_secs(24 * 60 * 60),
            Timeframe::D3 => Duration::from_secs(3 * 24 * 60 * 60),
            Timeframe::W1 => Duration::from_secs(7 * 24 * 60 * 60),
            Timeframe::MN1 => Duration::from_secs(30 * 24 * 60 * 60), // 근사값
        }
    }

    /// 이 타임프레임의 초 단위 값을 반환합니다.
    pub fn as_secs(&self) -> u64 {
        self.duration().as_secs()
    }

    /// 이 타임프레임의 분 단위 값을 반환합니다.
    pub fn as_minutes(&self) -> u64 {
        self.as_secs() / 60
    }

    /// 바이낸스 간격 문자열로 변환합니다.
    pub fn to_binance_interval(&self) -> &'static str {
        match self {
            Timeframe::M1 => "1m",
            Timeframe::M3 => "3m",
            Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1 => "1h",
            Timeframe::H2 => "2h",
            Timeframe::H4 => "4h",
            Timeframe::H6 => "6h",
            Timeframe::H8 => "8h",
            Timeframe::H12 => "12h",
            Timeframe::D1 => "1d",
            Timeframe::D3 => "3d",
            Timeframe::W1 => "1w",
            Timeframe::MN1 => "1M",
        }
    }

    /// 바이낸스 간격 문자열에서 파싱합니다.
    pub fn from_binance_interval(s: &str) -> Option<Self> {
        match s {
            "1m" => Some(Timeframe::M1),
            "3m" => Some(Timeframe::M3),
            "5m" => Some(Timeframe::M5),
            "15m" => Some(Timeframe::M15),
            "30m" => Some(Timeframe::M30),
            "1h" => Some(Timeframe::H1),
            "2h" => Some(Timeframe::H2),
            "4h" => Some(Timeframe::H4),
            "6h" => Some(Timeframe::H6),
            "8h" => Some(Timeframe::H8),
            "12h" => Some(Timeframe::H12),
            "1d" => Some(Timeframe::D1),
            "3d" => Some(Timeframe::D3),
            "1w" => Some(Timeframe::W1),
            "1M" => Some(Timeframe::MN1),
            _ => None,
        }
    }
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_binance_interval())
    }
}

impl FromStr for Timeframe {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_binance_interval(s).ok_or_else(|| format!("Invalid timeframe: {}", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeframe_duration() {
        assert_eq!(Timeframe::M1.as_secs(), 60);
        assert_eq!(Timeframe::H1.as_secs(), 3600);
        assert_eq!(Timeframe::D1.as_secs(), 86400);
    }

    #[test]
    fn test_timeframe_binance() {
        assert_eq!(Timeframe::M15.to_binance_interval(), "15m");
        assert_eq!(Timeframe::from_binance_interval("4h"), Some(Timeframe::H4));
    }
}
