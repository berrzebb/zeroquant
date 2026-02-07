//! API 라우트.
//!
//! 모든 REST API 엔드포인트를 정의하고 라우터를 구성합니다.
//!
//! # 라우트 구조
//!
//! - `/health` - 헬스 체크 (liveness)
//! - `/health/ready` - 상세 헬스 체크 (readiness)
//! - `/api/v1/strategies` - 전략 관리
//! - `/api/v1/orders` - 주문 관리
//! - `/api/v1/positions` - 포지션 관리
//! - `/api/v1/notifications` - 알림 설정
//! - `/api/v1/backtest` - 백테스트 실행
//! - `/api/v1/analytics` - 포트폴리오 분석
//! - `/api/v1/patterns` - 패턴 인식 (캔들스틱/차트)
//! - `/api/v1/portfolio` - 포트폴리오 요약/잔고/보유종목
//! - `/api/v1/market` - 시장 상태
//! - `/api/v1/credentials` - 자격증명 관리 (API 키, 텔레그램 설정)
//! - `/api/v1/ml` - ML 훈련 관리
//! - `/api/v1/journal` - 매매일지 (체결 내역, 포지션 현황, 손익 분석)
//! - `/api/v1/screening` - 종목 스크리닝 (Fundamental + 기술적 필터)
//! - `/api/v1/reality-check` - 추천 검증 (전일 추천 vs 익일 실제 성과)
//! - `/api/v1/monitoring` - 모니터링 (에러 추적, 통계)
//! - `/api/v1/ranking` - GlobalScore 기반 종목 랭킹
//! - `/api/v1/watchlist` - 관심종목 관리
//! - `/api/v1/alerts` - 알림 히스토리

pub mod alert_history;
pub mod analytics;
pub mod backtest;
pub mod backtest_results;
pub mod credentials;
pub mod dataset;
pub mod equity_history;
pub mod health;
pub mod journal;
pub mod market;
pub mod ml;
pub mod monitoring;
#[cfg(feature = "notifications")]
pub mod notifications;
pub mod orders;
pub mod paper_trading;
pub mod patterns;
pub mod portfolio;
pub mod positions;
pub mod ranking;
pub mod reality_check;
pub mod schema;
pub mod screening;
pub mod signal_alerts;
pub mod signals;
pub mod simulation;
pub mod strategies;
pub mod watchlist;

pub use alert_history::{alert_history_router, FrontendAlertHistoryResponse};
pub use analytics::{
    analytics_router, ChartResponse, EquityCurveResponse, MonthlyReturnsResponse,
    PerformanceResponse,
};
pub use backtest::{
    backtest_router, BacktestMultiRunRequest, BacktestMultiRunResponse, BacktestRunRequest,
    BacktestRunResponse, BacktestStrategiesResponse,
};
pub use backtest_results::{
    backtest_results_router, BacktestResultResponse, ListResultsResponse, SaveBacktestResultRequest,
};
pub use credentials::{
    credentials_router, EncryptedCredentials, ExchangeCredentialResponse,
    SupportedExchangesResponse, TelegramSettingsResponse,
};
pub use dataset::{dataset_router, DatasetListResponse, DatasetSummary, FetchDatasetRequest};
pub use health::{health_router, ComponentHealth, ComponentStatus, HealthResponse};
pub use journal::{
    journal_router, ExecutionsListResponse, JournalPositionsResponse, PnLSummaryResponse,
    SyncResponse,
};
pub use market::{
    market_router, MacroEnvironmentResponse, MarketBreadthResponse, MarketOverviewResponse,
    MarketStatusResponse,
};
pub use ml::{ml_router, ModelType, TrainedModel, TrainingJob, TrainingStatus};
pub use monitoring::{monitoring_router, ErrorRecordDto, ErrorsResponse, StatsResponse};
#[cfg(feature = "notifications")]
pub use notifications::{notifications_router, TelegramTestRequest, TelegramTestResponse};
pub use orders::{orders_router, CancelOrderResponse, OrderResponse, OrdersListResponse};
pub use patterns::{
    patterns_router, CandlestickPatternsResponse, ChartPatternsResponse, PatternTypesResponse,
};
pub use portfolio::{
    invalidate_order_history_cache, invalidate_portfolio_cache, portfolio_router, BalanceResponse,
    HoldingsResponse, PortfolioSummaryResponse,
};
pub use positions::{
    positions_router, PositionResponse, PositionSummaryResponse, PositionsListResponse,
};
pub use ranking::{
    ranking_router, CalculateResponse as RankingCalculateResponse, RankingQuery, RankingResponse,
    SevenFactorBatchRequest, SevenFactorBatchResponse, SevenFactorQuery,
};
pub use reality_check::{
    reality_check_router, CalculateRequest, CalculateResponse, ResultsQuery, ResultsResponse,
    SaveSnapshotRequest, SaveSnapshotResponse, SnapshotsQuery, SnapshotsResponse, StatsQuery,
};
pub use schema::schema_router;
pub use screening::{
    screening_router, sectors_router, MomentumResponse, ScreeningRequest, ScreeningResponse,
    SectorRankingResponse, SectorRsDto,
};
pub use signals::{
    signals_router, SignalMarkerDto, SignalSearchRequest, SignalSearchResponse,
    StrategySignalsQuery, SymbolSignalsQuery,
};
pub use simulation::{simulation_router, SimulationStartRequest, SimulationStatusResponse};
pub use strategies::{strategies_router, ApiError, StrategiesListResponse, StrategyDetailResponse};
pub use watchlist::{
    watchlist_router, AddItemsRequest, AddItemsResponse, WatchlistDetailResponse,
    WatchlistListResponse,
};

use axum::Router;
use std::sync::Arc;

use crate::state::AppState;

/// 전체 API 라우터 생성.
///
/// 모든 서브 라우터를 조합하여 하나의 라우터로 반환합니다.
///
/// # Feature Flags
/// - `notifications`: 알림 라우터 활성화 (`/api/v1/notifications`)
pub fn create_api_router() -> Router<Arc<AppState>> {
    let router = Router::new()
        // 헬스 체크 엔드포인트
        .nest("/health", health_router())
        // API v1 엔드포인트
        .nest("/api/v1/strategies", strategies_router())
        .nest("/api/v1/orders", orders_router())
        .nest("/api/v1/positions", positions_router())
        .nest("/api/v1/backtest", backtest_router())
        .nest("/api/v1/backtest/results", backtest_results_router())
        .nest("/api/v1/simulation", simulation_router())
        .nest("/api/v1/analytics", analytics_router())
        .nest("/api/v1/patterns", patterns_router())
        .nest("/api/v1/portfolio", portfolio_router())
        .nest("/api/v1/market", market_router())
        .nest("/api/v1/credentials", credentials_router())
        .nest("/api/v1/ml", ml_router())
        .nest("/api/v1/dataset", dataset_router())
        .nest("/api/v1/journal", journal_router())
        .nest("/api/v1/schema", schema_router())
        .nest("/api/v1/screening", screening_router())
        .nest("/api/v1/sectors", sectors_router())
        .nest("/api/v1/signals", signals_router())
        .nest("/api/v1/signal-alerts", signal_alerts::signal_alerts_router())
        .nest("/api/v1/reality-check", reality_check_router())
        .nest("/api/v1/monitoring", monitoring_router())
        .nest("/api/v1/ranking", ranking_router())
        .nest("/api/v1/watchlist", watchlist_router())
        .nest("/api/v1/alerts", alert_history_router())
        .nest("/api/v1/paper-trading", paper_trading::router());

    // Feature: notifications - 텔레그램/이메일 알림
    #[cfg(feature = "notifications")]
    let router = router.nest("/api/v1/notifications", notifications_router());

    router
}
