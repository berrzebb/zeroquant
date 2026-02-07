//! 데이터 수집 모듈.

pub mod checkpoint;
pub mod fundamental_sync;
pub mod global_score_sync;
pub mod indicator_sync;
pub mod macro_data_sync;
pub mod market_breadth_sync;
pub mod ohlcv_collect;
pub mod scheduler;
pub mod screening_refresh;
pub mod signal_performance_sync;
pub mod symbol_sync;
pub mod utils;
pub mod watchlist_helper;

pub use checkpoint::{
    clear_checkpoint, list_checkpoints, mark_interrupted, CheckpointInfo, CheckpointStatus,
};
pub use fundamental_sync::{
    fetch_and_save_naver_fundamental, sync_krx_fundamentals, sync_naver_fundamentals,
    sync_naver_fundamentals_with_options, sync_yahoo_fundamentals, FundamentalSyncStats,
    NaverSyncOptions, YahooSyncOptions,
};
pub use global_score_sync::{
    sync_global_scores, sync_global_scores_with_options, GlobalScoreSyncOptions,
};
pub use indicator_sync::{sync_indicators, sync_indicators_with_options, IndicatorSyncOptions};
pub use ohlcv_collect::collect_ohlcv;
pub use scheduler::{MarketHours, MarketStatus, Scheduler};
pub use screening_refresh::{get_screening_view_stats, refresh_screening_view, refresh_sector_rs_view, ScreeningViewStats};
pub use macro_data_sync::{sync_macro_data, sync_macro_data_arc, MacroSyncResult};
pub use market_breadth_sync::{sync_market_breadth, MarketBreadthSyncResult};
pub use signal_performance_sync::{sync_signal_performance, SignalPerformanceSyncOptions};
pub use symbol_sync::sync_symbols;
