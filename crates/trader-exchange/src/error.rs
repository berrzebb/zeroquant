//! 거래소 에러 타입.

use thiserror::Error;

/// 거래소 관련 에러.
#[derive(Debug, Error)]
pub enum ExchangeError {
    /// 네트워크/연결 에러
    #[error("Network error: {0}")]
    NetworkError(String),

    /// 거래소 연결 끊김
    #[error("Disconnected: {0}")]
    Disconnected(String),

    /// 인증/권한 에러
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// 요청 한도 초과
    #[error("Rate limit exceeded")]
    RateLimited,

    /// API 에러 코드
    #[error("API error {code}: {message}")]
    ApiError { code: i32, message: String },

    /// 파싱/역직렬화 에러
    #[error("Parse error: {0}")]
    ParseError(String),

    /// 유효하지 않은 수량
    #[error("Invalid quantity: {0}")]
    InvalidQuantity(String),

    /// 타임스탬프 동기화 에러
    #[error("Timestamp error: {0}")]
    TimestampError(String),

    /// 잔고 부족
    #[error("Insufficient balance: {0}")]
    InsufficientBalance(String),

    /// 주문을 찾을 수 없음
    #[error("Order not found: {0}")]
    OrderNotFound(String),

    /// 자산을 찾을 수 없음
    #[error("Asset not found: {0}")]
    AssetNotFound(String),

    /// 심볼을 찾을 수 없음
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    /// 주문 거부됨
    #[error("Order rejected: {0}")]
    OrderRejected(String),

    /// WebSocket 에러
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    /// 타임아웃
    #[error("Request timeout: {0}")]
    Timeout(String),

    /// 알 수 없는 에러
    #[error("Unknown error: {0}")]
    Unknown(String),

    /// 지원되지 않는 작업
    #[error("Not supported: {0}")]
    NotSupported(String),
}

impl ExchangeError {
    /// 재시도 가능한 에러인지 확인.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ExchangeError::NetworkError(_)
                | ExchangeError::Disconnected(_)
                | ExchangeError::RateLimited
                | ExchangeError::Timeout(_)
                | ExchangeError::WebSocket(_)
                | ExchangeError::TimestampError(_)
        )
    }

    /// 권장 재시도 대기 시간(밀리초) 반환.
    pub fn retry_delay_ms(&self) -> Option<u64> {
        match self {
            ExchangeError::RateLimited => Some(60000), // 1분
            ExchangeError::NetworkError(_) => Some(1000),
            ExchangeError::Disconnected(_) => Some(5000),
            ExchangeError::Timeout(_) => Some(500),
            ExchangeError::WebSocket(_) => Some(2000),
            ExchangeError::TimestampError(_) => Some(100),
            _ => None,
        }
    }

    /// 인증 에러인지 확인.
    pub fn is_auth_error(&self) -> bool {
        matches!(self, ExchangeError::Unauthorized(_))
    }

    /// 재시도하면 안 되는 치명적 에러인지 확인.
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            ExchangeError::Unauthorized(_)
                | ExchangeError::InsufficientBalance(_)
                | ExchangeError::InvalidQuantity(_)
                | ExchangeError::OrderRejected(_)
        )
    }
}

impl From<reqwest::Error> for ExchangeError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            ExchangeError::Timeout(err.to_string())
        } else if err.is_connect() {
            ExchangeError::NetworkError(err.to_string())
        } else {
            ExchangeError::Unknown(err.to_string())
        }
    }
}

impl From<serde_json::Error> for ExchangeError {
    fn from(err: serde_json::Error) -> Self {
        ExchangeError::ParseError(err.to_string())
    }
}
