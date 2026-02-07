//! 트레이딩 전략 엔진 및 플러그인 시스템.
//!
//! 이 크레이트가 제공하는 기능:
//! - 트레이딩 전략 구현을 위한 Strategy trait
//! - 동적 전략 로딩을 위한 플러그인 로더
//! - 전략 실행 엔진
//! - 내장 전략 (그리드 트레이딩, RSI 평균 회귀)
//!
//! # 예제
//!
//! ```rust,ignore
//! use trader_strategy::{StrategyEngine, EngineConfig, GridStrategy};
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() {
//!     let engine = StrategyEngine::new(EngineConfig::default());
//!
//!     // 그리드 트레이딩 전략 등록
//!     let strategy = Box::new(GridStrategy::new());
//!     let config = json!({
//!         "ticker": "BTC/USDT",
//!         "grid_levels": 10,
//!         "grid_spacing_pct": 1.0,
//!         "amount_per_level": "100"
//!     });
//!
//!     engine.register_strategy("btc_grid", strategy, config).await.unwrap();
//!     engine.start_strategy("btc_grid").await.unwrap();
//! }
//! ```

pub mod engine;
pub mod macros;
pub mod plugin;
pub mod registry;
pub mod schema_composer;
pub mod schema_registry;
pub mod strategies;
pub mod traits;

// 주요 타입 재내보내기
pub use engine::{
    extract_tickers_from_config, EngineConfig, EngineError, EngineStats, SignalConflictEvent,
    StrategyEngine, StrategyStats, StrategyStatus,
};
pub use plugin::{BuiltinStrategyFactory, LoaderConfig, PluginError, PluginLoader, PluginMetadata};
pub use registry::{StrategyCategory, StrategyMeta, StrategyRegistry};
pub use schema_composer::SchemaComposer;
pub use schema_registry::FragmentRegistry;
pub use strategies::{MeanReversionConfig, MeanReversionStrategy, MeanReversionVariant};
pub use traits::{Strategy, StrategyMetadata};

// 프로시저 매크로 재내보내기
pub use trader_strategy_macro::StrategyConfig;
