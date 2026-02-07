//! 데이터 관리 및 저장.
//!
//! 이 crate는 다음을 제공합니다:
//! - 실시간 및 과거 데이터를 위한 데이터 관리자
//! - TimescaleDB 저장소
//! - Redis 캐싱
//! - OHLCV 캔들 데이터 캐싱 (증분 업데이트 지원)
//! - 데이터 가져오기 유틸리티

pub mod cache;
pub mod error;
pub mod manager;
pub mod market_breadth;
pub mod provider;
pub mod storage;

pub use error::{DataError, Result};
pub use manager::*;

// 저장소 타입 재내보내기
pub use storage::redis::{CacheStats, MetricsCache, RedisCache, RedisConfig};
pub use storage::timescale::{
    Database, DatabaseConfig, OrderRecord, OrderRepository, PositionRecord, PositionRepository,
    SymbolRecord, SymbolRepository, TradeRecord, TradeRepository, TradeTickRecord,
    TradeTickRepository,
};

// OHLCV 캔들 캐시 재내보내기
pub use cache::historical::{CacheStats as HistoricalCacheStats, CachedHistoricalDataProvider};
pub use storage::ohlcv::{OhlcvCache, OhlcvMetadataRecord, OhlcvRecord};

// Fundamental 데이터 수집 재내보내기
pub use cache::fundamental::{FetchResult, FundamentalData, FundamentalFetcher};

// KRX 데이터 소스 재내보내기
pub use storage::krx::KrxDataSource;

// 심볼 정보 Provider 재내보내기
pub use provider::{
    BinanceSymbolProvider, CompositeSymbolProvider, KrxSymbolProvider, SymbolInfoProvider,
    SymbolMetadata, SymbolResolver, YahooSymbolProvider,
};

// Market Breadth 계산 재내보내기
pub use market_breadth::MarketBreadthCalculator;
