//! 포트폴리오 분석 모듈
//!
//! 투자 포트폴리오의 성과를 시각화하고 분석하기 위한 도구를 제공합니다.
//!
//! # 모듈 구성
//!
//! - [`equity_curve`]: 자산 곡선 데이터 생성 및 관리
//! - [`charts`]: 차트 데이터 구조 (CAGR, MDD, 월별 수익률 등)
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! use trader_analytics::portfolio::{EquityCurve, TimeFrame, EquityCurveBuilder};
//! use rust_decimal_macros::dec;
//!
//! // 자산 곡선 빌더 생성
//! let mut builder = EquityCurveBuilder::new(dec!(10_000_000));
//!
//! // 거래 결과 추가
//! builder.add_trade_result(timestamp1, dec!(10_100_000));
//! builder.add_trade_result(timestamp2, dec!(10_250_000));
//!
//! // 자산 곡선 데이터 생성
//! let curve = builder.build();
//!
//! // 일별 집계 데이터 조회
//! let daily = curve.aggregate(TimeFrame::Daily);
//!
//! // 현재 Drawdown 확인
//! println!("현재 낙폭: {}%", curve.current_drawdown());
//! ```

pub mod charts;
pub mod equity_curve;

pub use charts::*;
pub use equity_curve::*;
