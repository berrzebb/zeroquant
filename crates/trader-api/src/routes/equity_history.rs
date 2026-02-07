//! 포트폴리오 자산 히스토리 관리 모듈.
//!
//! 자산 곡선(Equity Curve) 데이터의 저장 및 조회를 담당합니다.
//!
//! # Repository 패턴
//!
//! 실제 데이터베이스 접근 로직은 `crate::repository::equity_history`에 있습니다.

// Repository에서 타입 re-export
pub use crate::repository::equity_history::{
    EquityHistoryRepository, EquityPoint, ExecutionForSync, MonthlyReturn, PortfolioSnapshot,
    SyncResult,
};
