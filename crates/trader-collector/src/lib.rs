//! Standalone data collector for ZeroQuant trading system.
//!
//! 이 crate는 API 서버와 독립적으로 데이터를 수집하는 바이너리를 제공합니다:
//! - 심볼 정보 동기화 (KRX, Binance, Yahoo Finance)
//! - OHLCV 데이터 수집 (일봉)
//! - Fundamental 데이터 수집 (재무 지표)

pub mod config;
pub mod error;
pub mod modules;
pub mod stats;

pub use config::CollectorConfig;
pub use error::{CollectorError, Result};
pub use stats::CollectionStats;
