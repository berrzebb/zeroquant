//! 트레이딩 시스템의 에러 타입.
//!
//! 이 모듈은 트레이딩 시스템 전반에서 사용되는 에러 타입을 정의합니다.

use thiserror::Error;

/// 핵심 트레이딩 에러.
#[derive(Debug, Error)]
pub enum TraderError {
    /// 설정 에러
    #[error("설정 에러: {0}")]
    Config(String),

    /// 거래소 연결 에러
    #[error("거래소 에러: {0}")]
    Exchange(String),

    /// 주문 에러
    #[error("주문 에러: {0}")]
    Order(String),

    /// 포지션 에러
    #[error("포지션 에러: {0}")]
    Position(String),

    /// 리스크 관리 에러
    #[error("리스크 에러: {0}")]
    Risk(String),

    /// 전략 에러
    #[error("전략 에러: {0}")]
    Strategy(String),

    /// 데이터 에러
    #[error("데이터 에러: {0}")]
    Data(String),

    /// 인증 에러
    #[error("인증 에러: {0}")]
    Auth(String),

    /// 요청 한도 초과
    #[error("요청 한도 초과: {0}")]
    RateLimit(String),

    /// 네트워크 에러
    #[error("네트워크 에러: {0}")]
    Network(String),

    /// 직렬화 에러
    #[error("직렬화 에러: {0}")]
    Serialization(String),

    /// 데이터베이스 에러
    #[error("데이터베이스 에러: {0}")]
    Database(String),

    /// 잔고 부족
    #[error("잔고 부족: {0}")]
    InsufficientFunds(String),

    /// 잘못된 입력
    #[error("잘못된 입력: {0}")]
    InvalidInput(String),

    /// 찾을 수 없음
    #[error("찾을 수 없음: {0}")]
    NotFound(String),

    /// 내부 에러
    #[error("내부 에러: {0}")]
    Internal(String),
}

/// 트레이딩 작업을 위한 Result 타입.
pub type TraderResult<T> = Result<T, TraderError>;

impl TraderError {
    /// 재시도 가능한 에러인지 확인합니다.
    pub fn is_retryable(&self) -> bool {
        matches!(self, TraderError::Network(_) | TraderError::RateLimit(_))
    }

    /// 치명적인 에러인지 확인합니다.
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            TraderError::Auth(_) | TraderError::InsufficientFunds(_)
        )
    }
}

impl From<serde_json::Error> for TraderError {
    fn from(err: serde_json::Error) -> Self {
        TraderError::Serialization(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_retryable() {
        let network_err = TraderError::Network("timeout".to_string());
        assert!(network_err.is_retryable());

        let auth_err = TraderError::Auth("invalid key".to_string());
        assert!(!auth_err.is_retryable());
    }

    #[test]
    fn test_error_critical() {
        let auth_err = TraderError::Auth("invalid key".to_string());
        assert!(auth_err.is_critical());

        let order_err = TraderError::Order("invalid quantity".to_string());
        assert!(!order_err.is_critical());
    }
}
