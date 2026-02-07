//! OpenAPI 문서화 설정.
//!
//! utoipa를 사용하여 REST API의 OpenAPI 3.0 스펙을 생성합니다.
//! Swagger UI는 `/swagger-ui` 경로에서 사용 가능합니다.
//!
//! # 자동 생성 구조
//!
//! 각 라우트 모듈은 자체 스키마를 정의하고, 중앙 `ApiDoc`에서 자동으로 집계합니다.
//! 새로운 엔드포인트를 추가할 때:
//!
//! 1. 응답/요청 타입에 `#[derive(ToSchema)]` 추가
//! 2. 핸들러에 `#[utoipa::path(...)]` 어노테이션 추가
//! 3. 이 파일의 `components(schemas(...))` 및 `paths(...)` 섹션에 추가
//!
//! # 외부 타입 처리
//!
//! 외부 크레이트의 타입은 두 가지 방법으로 처리:
//! - 해당 크레이트에 `ToSchema` 구현 추가
//! - 또는 `#[schema(value_type = Object)]` 사용하여 JSON 객체로 처리

use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

// trader-core 도메인 타입 (ToSchema 지원)
use trader_core::types::{MarketType, Symbol};
use trader_core::{OrderStatusType, OrderType, Side, SignalIndicators, TimeInForce};

// trader-analytics ML 타입
use trader_analytics::ml::{CandlestickPatternInfo, ChartPatternInfo, PatternDetectionResult};

// ==================== 각 모듈에서 스키마 Import ====================

use crate::error::ApiErrorResponse;
use crate::repository::{RankedSymbol, SevenFactorData, SevenFactorResponse};
use crate::repository::signal_performance::{
    SignalPerformanceResponse, SignalReturnPoint, SignalSymbolStats,
};
use crate::routes::{
    // Alert History 모듈
    alert_history::FrontendAlertHistoryResponse,
    // Analytics 모듈
    analytics::types::{
        AvailableIndicatorsResponse, ChartQuery, ChartResponse, CorrelationResponse,
        EquityCurveResponse, IndicatorDataResponse, IndicatorQuery, KeltnerResponse,
        MonthlyReturnsResponse, ObvResponse, PerformanceResponse, PeriodQuery, SuperTrendResponse,
        VolumeProfileQuery, VolumeProfileResponse, VwapResponse,
    },
    // Credentials 모듈
    credentials::{
        DiscordSettingsResponse, EmailSettingsResponse, NotificationSettingsConfig,
        SaveDiscordSettingsRequest, SaveEmailSettingsRequest, SaveSlackSettingsRequest,
        SaveSmsSettingsRequest, SlackSettingsResponse, SmsSettingsResponse,
    },
    // Dataset 추가 타입
    dataset::{FailedSymbolsResponse, ReactivateSymbolsRequest, ReactivateSymbolsResponse, SymbolStatsResponse},
    // Market 모듈
    market::{
        MacroEnvironmentResponse, MarketBreadthResponse, MarketOverviewResponse,
        MarketStatusResponse,
    },
    // Patterns 모듈
    patterns::{
        CandlestickPatternsResponse, ChartPatternsResponse, PatternTypeInfo, PatternTypesResponse,
    },
    // Ranking 모듈
    ranking::{
        CalculateResponse, FilterInfo, RankingQuery, RankingResponse, SevenFactorBatchRequest,
        SevenFactorBatchResponse, SevenFactorQuery,
    },
    // Reality Check 모듈
    reality_check::{
        CalculateRequest as RcCalculateRequest,
        CalculateResponse as RcCalculateResponse,
        ResultsResponse as RcResultsResponse,
        SaveSnapshotRequest as RcSaveSnapshotRequest,
        SaveSnapshotResponse as RcSaveSnapshotResponse,
        SnapshotsResponse as RcSnapshotsResponse,
        StatsResponse as RcStatsResponse,
    },
    // Signals 모듈
    signals::{
        CreateSignalRequest, CreateSignalResponse,
        SignalMarkerDto, SignalSearchRequest, SignalSearchResponse, StrategySignalsQuery,
        SymbolSignalsQuery,
    },
    // Strategies 모듈
    strategies::{ApiError, StrategyListItem},
    // Health 모듈
    ComponentHealth,
    ComponentStatus,
    // Monitoring 모듈
    ErrorRecordDto,
    ErrorsResponse,
    HealthResponse,
    // Screening 모듈
    MomentumResponse,
    ScreeningRequest,
    ScreeningResponse,
    StatsResponse,
    StrategiesListResponse,
};

// ==================== OpenAPI 문서 정의 ====================

/// Trader API 문서.
///
/// 모든 엔드포인트와 스키마를 포함하는 OpenAPI 3.0 스펙입니다.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "ZeroQuant Trading API",
        version = "0.6.0",
        description = r#"
# ZeroQuant 트레이딩 봇 REST API

전략 관리, 백테스트, 포트폴리오 분석을 위한 REST API입니다.

## 주요 기능

- **전략 관리**: 트레이딩 전략 생성, 조회, 시작/중지
- **백테스트**: 과거 데이터 기반 전략 성과 분석
- **포트폴리오**: 실시간 포트폴리오 상태 조회
- **시장 데이터**: 실시간 시세 및 차트 데이터
- **모니터링**: 에러 추적 및 시스템 상태 모니터링
- **스크리닝**: 종목 스크리닝 및 필터링
- **ML**: 머신러닝 모델 훈련 및 배포

## 인증

대부분의 엔드포인트는 JWT Bearer 토큰 인증이 필요합니다.
`Authorization: Bearer <token>` 헤더를 포함하세요.

## 심볼 동기화

- **KRX**: `POST /api/v1/dataset/sync/krx` - 한국 거래소 심볼
- **EODData**: `POST /api/v1/dataset/sync/eod` - 해외 거래소 심볼
"#,
        license(name = "MIT", url = "https://opensource.org/licenses/MIT"),
        contact(
            name = "ZeroQuant Team",
            url = "https://github.com/user/zeroquant"
        )
    ),
    servers(
        (url = "http://localhost:3000", description = "로컬 개발 서버"),
    ),
    tags(
        (name = "health", description = "헬스 체크 - 서버 상태 확인"),
        (name = "strategies", description = "전략 관리 - 트레이딩 전략 CRUD"),
        (name = "orders", description = "주문 관리 - 주문 생성/조회/취소"),
        (name = "positions", description = "포지션 - 현재 보유 포지션 조회"),
        (name = "portfolio", description = "포트폴리오 - 계좌 잔고 및 요약"),
        (name = "backtest", description = "백테스트 - 전략 과거 성과 분석"),
        (name = "analytics", description = "분석 - 성과 지표 및 차트"),
        (name = "patterns", description = "패턴 - 캔들/차트 패턴 인식"),
        (name = "market", description = "시장 - 시장 상태 및 시세"),
        (name = "credentials", description = "자격증명 - API 키 및 알림 설정 관리 (Telegram, Email, Discord, Slack, SMS)"),
        (name = "notifications", description = "알림 - 텔레그램 등 알림 설정"),
        (name = "ml", description = "ML - 머신러닝 모델 훈련"),
        (name = "dataset", description = "데이터셋 - 심볼 동기화 및 데이터 관리"),
        (name = "journal", description = "매매일지 - 체결 내역 및 손익 분석"),
        (name = "screening", description = "스크리닝 - 종목 필터링"),
        (name = "simulation", description = "시뮬레이션 - 모의 거래"),
        (name = "monitoring", description = "모니터링 - 에러 추적 및 시스템 상태"),
        (name = "signals", description = "신호 마커 - 백테스트/실거래 신호 조회 및 검색"),
        (name = "ranking", description = "랭킹 - GlobalScore 기반 종목 랭킹 및 7Factor 분석"),
        (name = "reality_check", description = "실제 검증 - 백테스트와 실거래 비교"),
        (name = "signal-alerts", description = "신호 알림 - 신호 기반 알림 규칙 관리"),
        (name = "alerts", description = "알림 히스토리 - 발생한 알림 이력 조회"),
        (name = "schema", description = "스키마 - 전략 스키마 및 프래그먼트 조회"),
        (name = "watchlist", description = "관심종목 - 관심종목 리스트 관리")
    ),
    // ==================== 스키마 등록 ====================
    components(
        schemas(
            // ===== Health =====
            HealthResponse,
            ComponentHealth,
            ComponentStatus,

            // ===== Common Error Types =====
            ApiError,
            ApiErrorResponse,

            // ===== Strategies =====
            StrategiesListResponse,
            StrategyListItem,

            // ===== Monitoring =====
            ErrorsResponse,
            ErrorRecordDto,
            StatsResponse,

            // ===== Screening =====
            ScreeningRequest,
            ScreeningResponse,
            MomentumResponse,

            // ===== Signals =====
            SignalMarkerDto,
            SignalSearchRequest,
            SignalSearchResponse,
            SymbolSignalsQuery,
            StrategySignalsQuery,

            // ===== Core Domain Types =====
            Side,
            OrderType,
            OrderStatusType,
            TimeInForce,
            Symbol,
            MarketType,
            SignalIndicators,

            // ===== Ranking =====
            CalculateResponse,
            RankingQuery,
            RankingResponse,
            FilterInfo,
            RankedSymbol,
            SevenFactorQuery,
            SevenFactorBatchRequest,
            SevenFactorBatchResponse,
            SevenFactorResponse,
            SevenFactorData,

            // ===== Patterns =====
            CandlestickPatternsResponse,
            ChartPatternsResponse,
            PatternTypeInfo,
            PatternTypesResponse,
            PatternDetectionResult,
            CandlestickPatternInfo,
            ChartPatternInfo,

            // ===== Analytics =====
            PerformanceResponse,
            PeriodQuery,
            ChartQuery,
            ChartResponse,
            EquityCurveResponse,
            MonthlyReturnsResponse,
            IndicatorQuery,
            IndicatorDataResponse,
            AvailableIndicatorsResponse,
            VolumeProfileQuery,
            VolumeProfileResponse,
            CorrelationResponse,
            VwapResponse,
            KeltnerResponse,
            ObvResponse,
            SuperTrendResponse,

            // ===== Credentials (Notification Providers) =====
            NotificationSettingsConfig,
            SaveEmailSettingsRequest,
            EmailSettingsResponse,
            SaveDiscordSettingsRequest,
            DiscordSettingsResponse,
            SaveSlackSettingsRequest,
            SlackSettingsResponse,
            SaveSmsSettingsRequest,
            SmsSettingsResponse,

            // ===== Alert History =====
            FrontendAlertHistoryResponse,

            // ===== Reality Check =====
            RcStatsResponse,
            RcResultsResponse,
            RcSnapshotsResponse,
            RcSaveSnapshotRequest,
            RcSaveSnapshotResponse,
            RcCalculateRequest,
            RcCalculateResponse,

            // ===== Signals (추가) =====
            CreateSignalRequest,
            CreateSignalResponse,
            SignalPerformanceResponse,
            SignalReturnPoint,
            SignalSymbolStats,

            // ===== Dataset (추가) =====
            FailedSymbolsResponse,
            SymbolStatsResponse,
            ReactivateSymbolsRequest,
            ReactivateSymbolsResponse,

            // ===== Market =====
            MarketOverviewResponse,
            MarketStatusResponse,
            MarketBreadthResponse,
            MacroEnvironmentResponse,
        )
    ),
    // ==================== 경로 등록 ====================
    paths(
        // ===== Health =====
        crate::routes::health::health_check,
        crate::routes::health::health_ready,

        // ===== Strategies =====
        crate::routes::strategies::list_strategies,
        crate::routes::strategies::create_strategy,
        crate::routes::strategies::delete_strategy,
        crate::routes::strategies::get_strategy,
        crate::routes::strategies::start_strategy,
        crate::routes::strategies::stop_strategy,
        crate::routes::strategies::update_config,
        crate::routes::strategies::update_risk_settings,
        crate::routes::strategies::update_symbols,
        crate::routes::strategies::clone_strategy,
        crate::routes::strategies::get_engine_stats,
        crate::routes::strategies::get_strategy_timeframes,
        crate::routes::strategies::update_strategy_timeframes,

        // ===== Monitoring =====
        crate::routes::monitoring::list_errors,
        crate::routes::monitoring::list_critical_errors,
        crate::routes::monitoring::get_error_by_id,
        crate::routes::monitoring::get_stats,
        crate::routes::monitoring::reset_stats,
        crate::routes::monitoring::clear_errors,
        crate::routes::monitoring::get_summary,

        // ===== Screening =====
        crate::routes::screening::run_screening,
        crate::routes::screening::list_presets,
        crate::routes::screening::run_preset_screening,
        crate::routes::screening::run_momentum_screening,
        crate::routes::screening::get_sector_ranking,

        // ===== Signals =====
        crate::routes::signals::search_signals,
        crate::routes::signals::get_signals_by_symbol,
        crate::routes::signals::get_signals_by_strategy,
        crate::routes::signals::get_backtest_signals,
        crate::routes::signals::create_signal,
        crate::routes::signals::get_signal_performance,
        crate::routes::signals::get_signal_scatter,
        crate::routes::signals::get_symbol_signal_performance,

        // ===== Ranking =====
        crate::routes::ranking::calculate_global,
        crate::routes::ranking::get_top_ranked,
        crate::routes::ranking::get_seven_factor,
        crate::routes::ranking::get_seven_factor_batch,
        crate::routes::ranking::get_score_history,

        // ===== Patterns =====
        crate::routes::patterns::get_candlestick_patterns,
        crate::routes::patterns::get_chart_patterns,
        crate::routes::patterns::detect_all_patterns,
        crate::routes::patterns::get_pattern_types,

        // ===== Analytics =====
        // Charts
        crate::routes::analytics::charts::get_equity_curve,
        crate::routes::analytics::charts::get_cagr_chart,
        crate::routes::analytics::charts::get_mdd_chart,
        crate::routes::analytics::charts::get_drawdown_chart,
        crate::routes::analytics::charts::get_monthly_returns,
        // Indicators
        crate::routes::analytics::indicators::get_available_indicators,
        crate::routes::analytics::indicators::get_sma_indicator,
        crate::routes::analytics::indicators::get_ema_indicator,
        crate::routes::analytics::indicators::get_rsi_indicator,
        crate::routes::analytics::indicators::get_macd_indicator,
        crate::routes::analytics::indicators::get_bollinger_indicator,
        crate::routes::analytics::indicators::get_stochastic_indicator,
        crate::routes::analytics::indicators::get_atr_indicator,
        crate::routes::analytics::indicators::calculate_indicators,
        crate::routes::analytics::indicators::get_volume_profile,
        crate::routes::analytics::indicators::get_correlation,
        crate::routes::analytics::indicators::get_vwap_indicator,
        crate::routes::analytics::indicators::get_keltner_indicator,
        crate::routes::analytics::indicators::get_obv_indicator,
        crate::routes::analytics::indicators::get_supertrend_indicator,
        // Performance
        crate::routes::analytics::performance::get_performance,
        // Sync
        crate::routes::analytics::sync::sync_equity_curve,
        crate::routes::analytics::sync::clear_equity_cache,

        // ===== Backtest =====
        crate::routes::backtest::list_backtest_strategies,
        crate::routes::backtest::run_backtest,
        crate::routes::backtest::get_backtest_result,
        crate::routes::backtest::run_multi_backtest,
        crate::routes::backtest::run_batch_backtest,

        // ===== Orders =====
        crate::routes::orders::create_order,
        crate::routes::orders::list_orders,
        crate::routes::orders::get_order,
        crate::routes::orders::cancel_order,
        crate::routes::orders::get_order_stats,

        // ===== Positions =====
        crate::routes::positions::list_positions,
        crate::routes::positions::get_positions_summary,
        crate::routes::positions::get_position,

        // ===== Portfolio =====
        crate::routes::portfolio::get_portfolio_summary,
        crate::routes::portfolio::get_balance,
        crate::routes::portfolio::get_holdings,
        crate::routes::portfolio::get_order_history,

        // ===== Journal =====
        crate::routes::journal::get_journal_positions,
        crate::routes::journal::list_executions,
        crate::routes::journal::get_pnl_summary,
        crate::routes::journal::get_daily_pnl,
        crate::routes::journal::get_symbol_pnl,
        crate::routes::journal::get_weekly_pnl,
        crate::routes::journal::get_monthly_pnl,
        crate::routes::journal::get_yearly_pnl,
        crate::routes::journal::get_cumulative_pnl,
        crate::routes::journal::get_trading_insights,
        crate::routes::journal::get_strategy_performance,
        crate::routes::journal::update_execution,
        crate::routes::journal::sync_executions,
        crate::routes::journal::get_cost_basis,
        crate::routes::journal::clear_execution_cache,

        // ===== Dataset =====
        crate::routes::dataset::list_datasets,
        crate::routes::dataset::fetch_dataset,
        crate::routes::dataset::get_candles,
        crate::routes::dataset::delete_dataset,
        crate::routes::dataset::search_symbols,
        crate::routes::dataset::get_symbols_batch,
        crate::routes::dataset::get_failed_symbols,
        crate::routes::dataset::get_symbol_stats,
        crate::routes::dataset::reactivate_symbols,

        // ===== Credentials (Notification Providers) =====
        crate::routes::credentials::email::get_email_settings,
        crate::routes::credentials::email::save_email_settings,
        crate::routes::credentials::email::delete_email_settings,
        crate::routes::credentials::email::test_email_settings,
        crate::routes::credentials::discord::get_discord_settings,
        crate::routes::credentials::discord::save_discord_settings,
        crate::routes::credentials::discord::delete_discord_settings,
        crate::routes::credentials::discord::test_discord_settings,
        crate::routes::credentials::slack::get_slack_settings,
        crate::routes::credentials::slack::save_slack_settings,
        crate::routes::credentials::slack::delete_slack_settings,
        crate::routes::credentials::slack::test_slack_settings,
        crate::routes::credentials::sms::get_sms_settings,
        crate::routes::credentials::sms::save_sms_settings,
        crate::routes::credentials::sms::delete_sms_settings,
        crate::routes::credentials::sms::test_sms_settings,
        crate::routes::credentials::discord::test_new_discord_settings,
        crate::routes::credentials::email::test_new_email_settings,
        crate::routes::credentials::slack::test_new_slack_settings,
        crate::routes::credentials::sms::test_new_sms_settings,

        // ===== Alert History =====
        crate::routes::alert_history::list_alert_history,
        crate::routes::alert_history::mark_alert_as_read,
        crate::routes::alert_history::mark_all_alerts_as_read,

        // ===== Reality Check =====
        crate::routes::reality_check::get_stats,
        crate::routes::reality_check::get_results,
        crate::routes::reality_check::get_snapshots,
        crate::routes::reality_check::save_snapshot,
        crate::routes::reality_check::calculate_reality_check,

        // ===== Market =====
        crate::routes::market::get_market_overview,

        // ===== Signal Alerts =====
        crate::routes::signal_alerts::create_alert_rule,
        crate::routes::signal_alerts::list_alert_rules,
        crate::routes::signal_alerts::get_alert_rule,
        crate::routes::signal_alerts::update_alert_rule,
        crate::routes::signal_alerts::delete_alert_rule,

        // ===== Backtest Results =====
        crate::routes::backtest_results::list_backtest_results,
        crate::routes::backtest_results::save_backtest_result,
        crate::routes::backtest_results::get_backtest_result,
        crate::routes::backtest_results::delete_backtest_result,

        // ===== Schema =====
        crate::routes::schema::list_strategy_meta,
        crate::routes::schema::get_strategy_schema,
        crate::routes::schema::list_fragments,
        crate::routes::schema::list_fragments_by_category,
        crate::routes::schema::get_fragment_detail,

        // ===== Watchlist =====
        crate::routes::watchlist::list_watchlists,
        crate::routes::watchlist::create_watchlist,
        crate::routes::watchlist::get_watchlist_detail,
        crate::routes::watchlist::delete_watchlist,
        crate::routes::watchlist::add_items,
        crate::routes::watchlist::remove_item,
        crate::routes::watchlist::update_item,
        crate::routes::watchlist::find_symbol_in_watchlists,

        // ===== ML =====
        crate::routes::ml::start_training,
        crate::routes::ml::get_training_jobs,
        crate::routes::ml::get_training_job,
        crate::routes::ml::cancel_training_job,
        crate::routes::ml::get_models,
        crate::routes::ml::get_model,
        crate::routes::ml::delete_model,
        crate::routes::ml::deploy_model,
        crate::routes::ml::download_model,
        crate::routes::ml::register_external_model,
        crate::routes::ml::get_deployed_models,

        // ===== Simulation =====
        crate::routes::simulation::start_simulation,
        crate::routes::simulation::stop_simulation,
        crate::routes::simulation::pause_simulation,
        crate::routes::simulation::get_simulation_status,
        crate::routes::simulation::get_simulation_positions,
        crate::routes::simulation::get_simulation_trades,
        crate::routes::simulation::get_simulation_equity,
        crate::routes::simulation::get_simulation_signals,
        crate::routes::simulation::reset_simulation,

        // ===== Credentials (Exchange) =====
        crate::routes::credentials::exchange::get_supported_exchanges,
        crate::routes::credentials::exchange::list_exchange_credentials,
        crate::routes::credentials::exchange::create_exchange_credential,
        crate::routes::credentials::exchange::update_exchange_credential,
        crate::routes::credentials::exchange::delete_exchange_credential,
        crate::routes::credentials::exchange::test_exchange_credential,
        crate::routes::credentials::exchange::test_new_exchange_credential,

        // ===== Credentials (Telegram) =====
        crate::routes::credentials::telegram::get_telegram_settings,
        crate::routes::credentials::telegram::save_telegram_settings,
        crate::routes::credentials::telegram::delete_telegram_settings,
        crate::routes::credentials::telegram::test_telegram_settings,

        // ===== Credentials (Active Account) =====
        crate::routes::credentials::active_account::get_active_account,
        crate::routes::credentials::active_account::set_active_account,

        // ===== Notifications =====
        crate::routes::notifications::test_telegram,
        crate::routes::notifications::get_notification_settings,
        crate::routes::notifications::test_telegram_env,
        crate::routes::notifications::get_templates,
        crate::routes::notifications::test_template,
        crate::routes::notifications::test_all_templates,
    )
)]
pub struct ApiDoc;

// ==================== Swagger UI 라우터 ====================

/// Swagger UI 라우터 생성.
///
/// 다음 경로에 문서 UI를 마운트합니다:
/// - `/swagger-ui` - Swagger UI 대화형 문서
/// - `/api-docs/openapi.json` - OpenAPI JSON 스펙
pub fn swagger_ui_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDoc::openapi())
        .into()
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_spec_valid() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_string_pretty(&spec).unwrap();

        // 기본 정보 확인
        assert!(json.contains("ZeroQuant Trading API"));
        assert!(json.contains("0.6.0"));

        // 태그 확인
        assert!(json.contains("health"));
        assert!(json.contains("strategies"));
        assert!(json.contains("monitoring"));
        assert!(json.contains("screening"));

        // 경로 확인
        assert!(json.contains("/health"));
        assert!(json.contains("/health/ready"));
        assert!(json.contains("/api/v1/monitoring/errors"));
        assert!(json.contains("/api/v1/screening"));
    }

    #[test]
    fn test_swagger_ui_router_creates() {
        let _router: Router<()> = swagger_ui_router();
    }

    #[test]
    fn test_openapi_contains_schemas() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_string(&spec).unwrap();

        // 스키마 확인
        assert!(json.contains("HealthResponse"));
        assert!(json.contains("ErrorsResponse"));
        assert!(json.contains("ScreeningRequest"));
        assert!(json.contains("ApiError"));
    }
}
