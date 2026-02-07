//! 캐싱 레이어.
//!
//! - Redis 캐시: 실시간 데이터 캐싱
//! - Historical 캐시: Yahoo Finance 캔들 데이터 캐싱
//! - Fundamental 캐시: Yahoo Finance 펀더멘털 데이터 수집
//! - Macro 캐시: 매크로 경제 지표 (USD/KRW, NASDAQ)

pub mod fundamental;
pub mod historical;
pub mod macro_data;

pub use crate::storage::redis::{CacheStats, MetricsCache, RedisCache, RedisConfig};
pub use fundamental::{FetchResult, FundamentalData, FundamentalFetcher};
pub use historical::{CacheStats as HistoricalCacheStats, CachedHistoricalDataProvider};
pub use macro_data::{
    CachedMacroDataProvider, MacroData, MacroDataError, MacroDataProvider, MacroDataProviderTrait,
};
