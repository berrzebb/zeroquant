//! 성과 분석 모듈
//!
//! 트레이딩 전략의 성과를 측정하고 분석하기 위한 도구를 제공합니다.
//!
//! # 모듈 구성
//!
//! - [`metrics`]: 성과 지표 계산 (샤프비율, 최대낙폭, 승률 등)
//! - [`tracker`]: 실시간 성과 추적 및 이벤트 발생

pub mod metrics;
pub mod tracker;

pub use metrics::*;
pub use tracker::*;
