//! Repository pattern for database operations.
//!
//! 데이터베이스 접근 로직을 라우트 핸들러에서 분리하여 관리합니다.
//! 모든 Repository는 static methods 패턴을 사용합니다.

pub mod alerts;
pub mod backtest_results;
pub mod cost_basis;
pub mod credentials;
pub mod equity_history;
pub mod execution_cache;
pub mod global_score;
pub mod journal;
pub mod kis_token;
pub mod klines;
pub mod orders;
pub mod portfolio;
pub mod positions;
pub mod reality_check;
pub mod score_history;
pub mod screening;
pub mod signal_alert_rule;
pub mod signal_marker;
pub mod signal_performance;
pub mod strategies;
pub mod strategy_watched_tickers;
pub mod symbol_fundamental;
pub mod symbol_info;
pub mod watchlist;

pub use alerts::{
    AlertChannel, AlertFilter, AlertHistory, AlertHistoryResponse, AlertStats, AlertStatus,
    AlertType, AlertsRepository, CreateAlertRequest,
};
pub use backtest_results::{
    BacktestResultDto, BacktestResultInput, BacktestResultRecord, BacktestResultsRepository,
    ListResultsFilter, ListResultsResponse as BacktestListResponse,
};
pub use credentials::{
    create_exchange_providers_from_credential, create_kis_client_from_credential,
    create_kis_provider_for_sync, create_mock_provider_concrete, create_provider_bundle,
    create_provider_for_credential, create_provider_for_mock_credential,
    get_active_credential_id, get_credential_info, CredentialInfo, ExchangeProviderArc,
    ProviderBundle,
};
pub use equity_history::{
    EquityHistoryRepository, EquityPoint, ExecutionForSync, MonthlyReturn, PortfolioSnapshot,
    SyncResult,
};
pub use execution_cache::{
    CacheMeta, CachedExecution, ExecutionCacheRepository, ExecutionProvider, NewExecution,
};
pub use klines::{CacheMetadata, KlineRecord, KlinesRepository, NewKline};
pub use orders::{Order, OrderInput, OrderRepository, OrderStatus};
pub use portfolio::{PortfolioRepository, Position, PositionUpdate};
pub use positions::{
    HoldingPosition, PositionInput, PositionRecord, PositionRepository,
    SyncResult as PositionSyncResult,
};
pub use reality_check::{
    CalculationResult, DailyStats, PriceSnapshot, RankStats, RealityCheckRecord,
    RealityCheckRepository, SnapshotInput, SourceStats,
};
pub use screening::{
    CreatePresetRequest, MomentumScreenResult, ScreeningFilter, ScreeningPreset,
    ScreeningPresetRecord, ScreeningRepository, ScreeningResult, SectorRsResult,
};
pub use strategies::StrategyRepository;
pub use strategy_watched_tickers::StrategyWatchedTickersRepository;
pub use symbol_fundamental::{
    IndicatorUpdate, NewSymbolFundamental, SymbolFundamental, SymbolFundamentalRepository,
    SymbolWithFundamental,
};
pub use symbol_info::{
    CascadeDeleteResult, DeactivatedStats, ExternalFetchError, FailedSymbolInfo,
    FetchFailureResult, NewSymbolInfo, SymbolInfo, SymbolInfoRepository, SymbolSearchResult,
    MAX_FETCH_FAILURES,
};

pub use global_score::{
    GlobalScoreRecord, GlobalScoreRepository, RankedSymbol, RankingFilter, SevenFactorData,
    SevenFactorResponse,
};

pub use journal::{
    // 고급 거래 통계
    AdvancedTradingStats,
    CumulativePnL,
    CurrentPosition,
    DailySummary,
    ExecutionFilter,
    JournalRepository,
    MonthlyPnL,
    PnLSummary,
    PositionSnapshotInput,
    PositionSnapshotRecord,
    // 손익 재계산
    RecalculateResult,
    StrategyPerformance,
    SymbolPnL,
    SyncResult as JournalSyncResult,
    TradeExecutionInput,
    TradeExecutionRecord,
    // 인사이트 타입
    TradingInsights,
    // 기간별 손익 타입
    WeeklyPnL,
    YearlyPnL,
};

pub use cost_basis::{
    build_tracker_from_executions, CostBasisSummary, CostBasisTracker, FifoSaleResult, Lot,
    LotUsage, TradeExecution,
};

pub use signal_alert_rule::{
    CreateAlertRuleRequest, SignalAlertRule, SignalAlertRuleRepository, UpdateAlertRuleRequest,
};
pub use signal_marker::SignalMarkerRepository;
pub use signal_performance::{
    SignalPerformanceRepository, SignalPerformanceResponse, SignalReturnPoint,
    SignalStrengthStats, SignalStrategyStats, SignalSymbolStats, SignalTypeStats,
};

pub use watchlist::{
    NewWatchlist, NewWatchlistItem, UpdateWatchlistItem, WatchlistItemRecord, WatchlistRecord,
    WatchlistRepository, WatchlistWithCount,
};

pub use kis_token::KisTokenRepository;

pub use score_history::{
    ScoreHistoryInput, ScoreHistoryRecord, ScoreHistoryRepository, ScoreHistorySummary,
};
