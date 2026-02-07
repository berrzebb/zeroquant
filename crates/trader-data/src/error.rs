//! 데이터 모듈 오류 타입.

use thiserror::Error;

/// 데이터 관련 오류.
#[derive(Debug, Error)]
pub enum DataError {
    /// 데이터베이스 연결 오류
    #[error("Database connection error: {0}")]
    ConnectionError(String),

    /// 쿼리 실행 오류
    #[error("Query error: {0}")]
    QueryError(String),

    /// 레코드를 찾을 수 없음
    #[error("Record not found: {0}")]
    NotFound(String),

    /// 중복 레코드
    #[error("Duplicate record: {0}")]
    DuplicateError(String),

    /// 직렬화/역직렬화 오류
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// 캐시 오류
    #[error("Cache error: {0}")]
    CacheError(String),

    /// 캐시 미스
    #[error("Cache miss: {0}")]
    CacheMiss(String),

    /// 잘못된 데이터 형식
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// 설정 오류
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// 마이그레이션 오류
    #[error("Migration error: {0}")]
    MigrationError(String),

    /// 연결 풀 소진
    #[error("Connection pool exhausted")]
    PoolExhausted,

    /// 타임아웃 오류
    #[error("Operation timeout: {0}")]
    Timeout(String),

    /// 데이터 삽입 오류
    #[error("Insert error: {0}")]
    InsertError(String),

    /// 데이터 삭제 오류
    #[error("Delete error: {0}")]
    DeleteError(String),

    /// 데이터 가져오기 오류 (외부 소스)
    #[error("Fetch error: {0}")]
    FetchError(String),

    /// 파싱 오류
    #[error("Parse error: {0}")]
    ParseError(String),

    /// 상장폐지된 심볼 (모든 데이터 소스 실패)
    #[error("Symbol delisted: {0}")]
    SymbolDelisted(String),

    /// 지원되지 않는 거래소
    #[error("Unsupported exchange: {0}")]
    UnsupportedExchange(String),
}

impl From<sqlx::Error> for DataError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => DataError::NotFound("Row not found".to_string()),
            sqlx::Error::PoolTimedOut => DataError::PoolExhausted,
            sqlx::Error::Database(db_err) => {
                let code = db_err.code().unwrap_or_default();
                if code == "23505" {
                    // PostgreSQL 고유 제약 조건 위반
                    DataError::DuplicateError(db_err.message().to_string())
                } else {
                    DataError::QueryError(db_err.message().to_string())
                }
            }
            _ => DataError::QueryError(err.to_string()),
        }
    }
}

impl From<redis::RedisError> for DataError {
    fn from(err: redis::RedisError) -> Self {
        DataError::CacheError(err.to_string())
    }
}

impl From<serde_json::Error> for DataError {
    fn from(err: serde_json::Error) -> Self {
        DataError::SerializationError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, DataError>;
