//! 모니터링 모듈.
//!
//! AI 디버깅 및 운영 모니터링을 위한 기능을 제공합니다.
//!
//! # 주요 컴포넌트
//!
//! - [`error_tracker`]: 구조화된 에러 로그 수집 및 조회
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! use trader_api::monitoring::{global_tracker, ErrorRecordBuilder, ErrorSeverity, ErrorCategory};
//!
//! // 에러 기록
//! let record = ErrorRecordBuilder::new("데이터베이스 쿼리 실패")
//!     .severity(ErrorSeverity::Error)
//!     .category(ErrorCategory::Database)
//!     .entity("AAPL")
//!     .with_context("query", "SELECT * FROM ...")
//!     .raw_error(&e)
//!     .build();
//!
//! global_tracker().record(record);
//!
//! // 최근 에러 조회
//! let recent_errors = global_tracker().get_recent(10);
//! ```

pub mod error_tracker;

// Re-exports
pub use error_tracker::{
    global_tracker, init_global_tracker, ErrorCategory, ErrorRecord, ErrorRecordBuilder,
    ErrorSeverity, ErrorStats, ErrorTracker, ErrorTrackerConfig, SourceLocation,
};
