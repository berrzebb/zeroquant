//! 에러 타입 정의.

use std::fmt;

/// Collector 에러 타입
#[derive(Debug)]
pub enum CollectorError {
    /// 데이터베이스 에러
    Database(sqlx::Error),
    /// 설정 에러
    Config(String),
    /// 데이터 소스 에러 (KRX, Yahoo 등)
    DataSource(String),
    /// 신호 성과 계산 에러
    SignalPerformance {
        signal_id: String,
        reason: String,
    },
    /// 스케줄링 에러
    Scheduling(String),
    /// 시장 운영 시간 에러
    MarketHours(String),
    /// 일반 에러
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for CollectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(e) => write!(f, "Database error: {}", e),
            Self::Config(msg) => write!(f, "Configuration error: {}", msg),
            Self::DataSource(msg) => write!(f, "Data source error: {}", msg),
            Self::SignalPerformance { signal_id, reason } => {
                write!(f, "Signal performance error [{}]: {}", signal_id, reason)
            }
            Self::Scheduling(msg) => write!(f, "Scheduling error: {}", msg),
            Self::MarketHours(msg) => write!(f, "Market hours error: {}", msg),
            Self::Other(e) => write!(f, "Error: {}", e),
        }
    }
}

impl std::error::Error for CollectorError {}

impl From<sqlx::Error> for CollectorError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err)
    }
}

impl From<std::env::VarError> for CollectorError {
    fn from(err: std::env::VarError) -> Self {
        Self::Config(err.to_string())
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for CollectorError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::Other(err)
    }
}

/// Result 타입 별칭
pub type Result<T> = std::result::Result<T, CollectorError>;
