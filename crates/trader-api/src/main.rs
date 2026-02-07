//! 트레이딩 봇 API 서버.
//!
//! Axum 기반 REST API 서버를 시작합니다.
//! 헬스 체크, 전략 관리, 주문/포지션 조회 등의 엔드포인트를 제공합니다.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{http::StatusCode, middleware, routing::get, Router};
use metrics_exporter_prometheus::PrometheusHandle;
use trader_data::{Database, DatabaseConfig};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

use trader_api::metrics::setup_metrics_recorder;
use trader_api::middleware::{
    metrics_layer, rate_limit_middleware, RateLimitConfig, RateLimitState,
};
use trader_api::openapi::swagger_ui_router;
use trader_api::repository::StrategyRepository;
use trader_api::routes::create_api_router;
use trader_api::services::ApiBotHandler;
use trader_notification::{NotificationManager, TelegramConfig, TelegramSender};
use trader_api::state::AppState;
use trader_api::websocket::{
    create_subscription_manager, standalone_websocket_router, start_simulator,
    WsState,
};
use trader_core::crypto::CredentialEncryptor;
use trader_data::cache::CachedHistoricalDataProvider;
use trader_data::RedisCache;
use trader_execution::{ConversionConfig, OrderExecutor};
use trader_risk::{RiskConfig, RiskManager};
use trader_strategy::{EngineConfig, StrategyEngine};

/// 서버 설정 구조체.
struct ServerConfig {
    /// 바인딩할 호스트 주소
    host: String,
    /// 바인딩할 포트
    port: u16,
    /// 초기 잔고 (리스크 매니저용)
    initial_balance: rust_decimal::Decimal,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            initial_balance: rust_decimal_macros::dec!(10000),
        }
    }
}

impl ServerConfig {
    /// 환경 변수에서 설정 로드.
    fn from_env() -> Self {
        let host = std::env::var("API_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("API_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);
        let initial_balance = std::env::var("INITIAL_BALANCE")
            .ok()
            .and_then(|b| b.parse().ok())
            .unwrap_or(rust_decimal_macros::dec!(10000));

        Self {
            host,
            port,
            initial_balance,
        }
    }

    /// 소켓 주소 반환.
    ///
    /// # Errors
    /// `host:port` 형식이 유효하지 않으면 `AddrParseError`를 반환합니다.
    fn socket_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.host, self.port).parse()
    }
}

/// Mock 시뮬레이터 시작 (개발/테스트용).
///
/// 실제 거래소 WebSocket 스트림은 DB 기반 자격증명을 사용하여
/// 전략 시작 시 lazy 초기화됩니다 (`services::market_stream::get_or_create_market_stream`).
///
/// # 환경변수
///
/// - `USE_REAL_EXCHANGE`: "true"면 Mock 시뮬레이터를 시작하지 않음
/// - `ENABLE_MOCK_DATA`: "false"면 Mock 시뮬레이터를 시작하지 않음 (기본값: true)
fn start_mock_simulator_if_needed(
    subscriptions: trader_api::websocket::SharedSubscriptionManager,
) -> bool {
    let use_real_exchange = std::env::var("USE_REAL_EXCHANGE")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if use_real_exchange {
        // 실거래 모드: Mock 시뮬레이터 시작하지 않음
        // 실제 스트림은 전략 시작 시 get_or_create_market_stream()으로 lazy 생성
        info!("실거래 모드: WebSocket 스트림은 전략 시작 시 자동 생성됩니다");
        return false;
    }

    // Mock 시뮬레이터 사용
    let enable_simulator = std::env::var("ENABLE_MOCK_DATA")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(true);

    if enable_simulator {
        start_simulator(subscriptions);
        info!("Mock data simulator started");
        return true;
    }

    false
}

/// AppState 초기화.
async fn create_app_state(config: &ServerConfig) -> AppState {
    // 전략 엔진 생성
    let strategy_engine = StrategyEngine::new(EngineConfig::default());

    // 리스크 매니저 생성
    let risk_manager = RiskManager::new(RiskConfig::default(), config.initial_balance);

    // 주문 실행기 생성
    let executor = OrderExecutor::new_complete(
        RiskManager::new(RiskConfig::default(), config.initial_balance),
        "default_exchange",
        ConversionConfig::default(),
    );

    // AppState 빌드
    let mut state = AppState::new(strategy_engine, risk_manager, executor);

    // Redis 캐시 연결 설정 (REDIS_URL 환경변수에서)
    // trader-data의 RedisCache를 사용하여 API 응답 캐싱 및 OHLCV 캐싱 활성화
    // DB 연결 전에 Redis를 먼저 연결하여 data_provider에서 사용할 수 있도록 함
    let redis_cache: Option<Arc<RedisCache>> = if let Ok(redis_url) = std::env::var("REDIS_URL") {
        state = state.with_redis_url(&redis_url).await;
        state.cache.clone()
    } else {
        warn!("REDIS_URL not set, Redis caching will be disabled");
        None
    };

    // DB 연결 설정 (DATABASE_URL 환경변수에서)
    if let Ok(database_url) = std::env::var("DATABASE_URL") {
        let db_config = DatabaseConfig::for_daemon(database_url);
        match Database::connect(&db_config).await {
            Ok(db) => {
                let pool = db.pool().clone();
                // 연결 테스트
                if sqlx::query("SELECT 1").fetch_one(&pool).await.is_ok() {
                    info!("Connected to TimescaleDB successfully");

                    // Phase 0-1: CachedHistoricalDataProvider 및 분석 인프라 초기화
                    // Redis 캐시가 있으면 3계층 캐시 구조 활성화 (Redis → PostgreSQL → 외부 API)
                    let data_provider = if let Some(redis) = &redis_cache {
                        info!("OHLCV 3계층 캐시 활성화: Redis → PostgreSQL → 외부 API");
                        CachedHistoricalDataProvider::new(pool.clone()).with_redis(redis.clone())
                    } else {
                        CachedHistoricalDataProvider::new(pool.clone())
                    };
                    state = state
                        .with_db_pool(pool)
                        .with_data_provider(data_provider)
                        .with_analytics_infrastructure();
                    info!("Analytics infrastructure initialized (Phase 0-1)");
                } else {
                    error!("Failed to verify database connection");
                }
            }
            Err(e) => {
                error!("Failed to connect to database: {}", e);
            }
        }
    } else {
        warn!("DATABASE_URL not set, database features will be disabled");
    }

    // 암호화 관리자 설정 (ENCRYPTION_MASTER_KEY 환경변수에서)
    if let Ok(master_key) = std::env::var("ENCRYPTION_MASTER_KEY") {
        match CredentialEncryptor::new(&master_key) {
            Ok(encryptor) => {
                info!("Credential encryptor initialized");
                state = state.with_encryptor(encryptor);
            }
            Err(e) => {
                error!("Failed to initialize credential encryptor: {}", e);
            }
        }
    } else {
        warn!("ENCRYPTION_MASTER_KEY not set, credential encryption will be disabled");
    }

    // 알림 매니저 초기화 (텔레그램 설정)
    // 우선순위: 1) DB 암호화 저장소, 2) 환경변수
    let telegram_config_opt: Option<TelegramConfig> =
        if let (Some(pool), Some(encryptor)) = (&state.db_pool, &state.encryptor) {
            // DB에서 telegram_settings 조회
            let row: Option<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, bool)> = sqlx::query_as(
                r#"
                SELECT encrypted_bot_token, encryption_nonce_token,
                       encrypted_chat_id, encryption_nonce_chat, is_enabled
                FROM telegram_settings
                WHERE is_enabled = true
                LIMIT 1
                "#,
            )
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

            if let Some((enc_token, nonce_token, enc_chat, nonce_chat, _enabled)) = row {
                // 복호화 시도
                match (
                    encryptor.decrypt(&enc_token, &nonce_token),
                    encryptor.decrypt(&enc_chat, &nonce_chat),
                ) {
                    (Ok(bot_token), Ok(chat_id)) => {
                        info!("텔레그램 설정을 암호화 저장소에서 로드했습니다");
                        Some(TelegramConfig::new(bot_token, chat_id))
                    }
                    (Err(e), _) | (_, Err(e)) => {
                        warn!("텔레그램 설정 복호화 실패: {:?}, 환경변수로 폴백", e);
                        TelegramConfig::from_env()
                    }
                }
            } else {
                info!("DB에 활성화된 텔레그램 설정 없음, 환경변수 확인");
                TelegramConfig::from_env()
            }
        } else {
            // DB 또는 encryptor가 없으면 환경변수 사용
            TelegramConfig::from_env()
        };

    if let Some(telegram_config) = telegram_config_opt {
        let telegram_sender = TelegramSender::new(telegram_config);
        let mut notification_manager = NotificationManager::new();
        notification_manager.add_sender(telegram_sender);
        state = state.with_notification_manager(notification_manager);
        info!("NotificationManager 초기화 완료 (텔레그램 알림 활성화)");
    } else {
        info!("텔레그램 설정 없음, 알림 기능 비활성화");
    }

    // ExchangeProvider 및 MarketDataProvider 설정 (거래소 중립)
    // DB 기반 credential만 사용 (레거시 환경변수 방식 제거됨)
    if let Some(pool) = &state.db_pool {
        use trader_api::repository::{create_provider_bundle, get_active_credential_id};

        match get_active_credential_id(pool).await {
            Ok(credential_id) => {
                // encryptor가 있으면 전달, 없으면 None (Mock은 encryptor 불필요)
                let encryptor_ref = state.encryptor.as_ref().map(|e| e.as_ref());

                match create_provider_bundle(pool, encryptor_ref, credential_id).await {
                    Ok(bundle) => {
                        info!(
                            "Provider 설정 완료 (credential: {}, exchange: {}, market_data: {})",
                            credential_id,
                            bundle.exchange.exchange_name(),
                            bundle.market_data.provider_name()
                        );
                        state = state
                            .with_exchange_provider(bundle.exchange)
                            .with_market_data_provider(bundle.market_data);
                    }
                    Err(e) => {
                        warn!("Provider 생성 실패: {}", e);
                    }
                }
            }
            Err(e) => {
                info!("Active credential 없음: {}", e);
            }
        }
    }

    state
}

/// CORS 미들웨어 구성.
///
/// CORS_ORIGINS 환경변수가 설정되어 있으면 해당 origin만 허용합니다.
/// 설정되지 않으면 개발 모드로 간주하여 모든 origin을 허용합니다.
///
/// # 환경변수
///
/// - `CORS_ORIGINS`: 쉼표로 구분된 허용 origin 목록
///   예: `https://dashboard.example.com,https://admin.example.com`
fn cors_layer() -> CorsLayer {
    let allow_origin = match std::env::var("CORS_ORIGINS") {
        Ok(origins) if !origins.is_empty() => {
            // 프로덕션: 특정 origin만 허용
            let origins: Vec<_> = origins
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();

            if origins.is_empty() {
                warn!("CORS_ORIGINS is set but contains no valid origins, allowing any");
                AllowOrigin::any()
            } else {
                info!("CORS configured with {} allowed origins", origins.len());
                AllowOrigin::list(origins)
            }
        }
        _ => {
            // 개발: 모든 origin 허용
            warn!("CORS_ORIGINS not set, allowing any origin (development mode)");
            AllowOrigin::any()
        }
    };

    CorsLayer::new()
        .allow_origin(allow_origin)
        // 허용되는 HTTP 메서드
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        // 허용되는 헤더
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
        ])
        // 자격 증명 포함 허용 (CORS_ORIGINS 설정 시에만)
        .allow_credentials(std::env::var("CORS_ORIGINS").is_ok())
        // preflight 요청 캐시 시간
        .max_age(Duration::from_secs(3600))
}

/// /metrics 엔드포인트 핸들러.
async fn metrics_handler(
    axum::extract::State(handle): axum::extract::State<PrometheusHandle>,
) -> String {
    handle.render()
}

/// Rate Limit 비활성화 여부 확인.
fn is_rate_limit_disabled() -> bool {
    std::env::var("RATE_LIMIT_DISABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
}

/// Rate Limit 설정 로드.
fn rate_limit_config() -> RateLimitConfig {
    let requests_per_minute = std::env::var("RATE_LIMIT_RPM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1200); // 기본: 분당 1200회

    info!(
        requests_per_minute = requests_per_minute,
        "Rate limiting configured"
    );

    RateLimitConfig::new(requests_per_minute)
}

/// 전체 라우터 생성.
fn create_router(
    state: Arc<AppState>,
    metrics_handle: PrometheusHandle,
    ws_state: WsState,
) -> Router {
    // 메트릭 라우터 (별도 상태, Rate Limit 제외)
    let metrics_router = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(metrics_handle);

    // API 라우터 (Rate Limit 조건부 적용)
    let api_router = if is_rate_limit_disabled() {
        info!("Rate limiting DISABLED (RATE_LIMIT_DISABLED=true)");
        create_api_router().with_state(state)
    } else {
        let rate_limit_state = RateLimitState::new(rate_limit_config());
        create_api_router()
            .with_state(state)
            .layer(middleware::from_fn_with_state(
                rate_limit_state,
                rate_limit_middleware,
            ))
    };

    // WebSocket 라우터
    let ws_router = standalone_websocket_router(ws_state);

    // 전체 라우터 조합
    Router::new()
        .merge(metrics_router)
        .merge(api_router)
        .nest("/ws", ws_router)
        // OpenAPI 문서 및 Swagger UI
        .merge(swagger_ui_router())
        // 메트릭 미들웨어 (모든 요청에 적용)
        .layer(middleware::from_fn(metrics_layer))
        // 기타 미들웨어
        .layer(TraceLayer::new_for_http())
        // 전역 타임아웃 (30초) - 408 상태 코드 반환
        .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(30)))
        .layer(cors_layer())
}

/// OpenAPI 스펙 내보내기 처리.
///
/// `--export-openapi` 플래그 또는 `EXPORT_OPENAPI` 환경변수가 설정된 경우
/// OpenAPI JSON 스펙을 stdout으로 출력하고 종료합니다.
fn handle_export_openapi() -> Result<(), Box<dyn std::error::Error>> {
    use trader_api::openapi::ApiDoc;
    use utoipa::OpenApi as _;

    // 명령줄 인자에서 --export-openapi 플래그 확인
    let export_flag = std::env::args().any(|arg| arg == "--export-openapi");

    // 환경변수 EXPORT_OPENAPI 확인
    let export_env = std::env::var("EXPORT_OPENAPI")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);

    if export_flag || export_env {
        // OpenAPI 스펙 생성
        let spec = ApiDoc::openapi();

        // JSON으로 직렬화
        let json = serde_json::to_string_pretty(&spec)?;

        // stdout으로 출력
        println!("{}", json);

        // 프로세스 종료
        std::process::exit(0);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // .env 파일 로드 (있는 경우)
    let _ = dotenvy::dotenv();

    // OpenAPI 내보내기 처리 (서버 시작 전)
    handle_export_openapi()?;

    // tracing 초기화
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "trader_api=info,tower_http=debug".into()),
        )
        .init();

    info!("Starting Trader API server...");

    // Prometheus 메트릭 레코더 설정
    let metrics_handle = setup_metrics_recorder();
    info!("Prometheus metrics recorder initialized");

    // 설정 로드
    let config = ServerConfig::from_env();
    let addr = config.socket_addr().map_err(|e| {
        error!(
            host = %config.host,
            port = config.port,
            error = %e,
            "소켓 주소 설정이 유효하지 않습니다. API_HOST, API_PORT 환경변수를 확인하세요."
        );
        e
    })?;

    // JWT 시크릿 로드
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
        warn!("JWT_SECRET not set, using default (INSECURE for development only)");
        "dev-secret-key-change-in-production".to_string()
    });

    // WebSocket 구독 관리자 생성
    let subscriptions = create_subscription_manager(1024);
    info!("WebSocket subscription manager initialized");

    // Mock 시뮬레이터 시작 (개발 모드일 때만)
    // 실거래 모드에서는 전략 시작 시 DB 자격증명 기반으로 lazy 초기화됨
    start_mock_simulator_if_needed(subscriptions.clone());

    // AppState 생성 (DB, Redis, 암호화 초기화 포함)
    // subscriptions를 AppState에도 전달하여 REST API에서 WebSocket 브로드캐스트 가능
    let subscriptions_for_ws = subscriptions.clone();
    let state = Arc::new(
        create_app_state(&config)
            .await
            .with_subscriptions(subscriptions),
    );

    // WebSocket 상태 생성 (AppState의 market_streams를 공유)
    let ws_state = WsState::new(subscriptions_for_ws, jwt_secret)
        .with_market_streams(state.market_streams.clone());

    info!(version = %state.version, "Application state initialized");
    info!(
        has_db = state.db_pool.is_some(),
        has_cache = state.has_cache(),
        has_encryptor = state.encryptor.is_some(),
        has_websocket = state.has_subscriptions(),
        has_analytics = state.has_analytics_provider(),
        has_context = state.has_strategy_context(),
        has_exchange_provider = state.has_exchange_provider(),
        "Service connections status"
    );

    // 전역 종료 토큰 생성 (graceful shutdown용, 백그라운드 태스크에서 사용)
    let shutdown_token = CancellationToken::new();

    // ContextSyncService 시작 (ExchangeProvider + AnalyticsProvider가 모두 설정된 경우)
    if let Some(_sync_handle) = state.start_context_sync(shutdown_token.clone()) {
        info!("ContextSyncService 시작됨 (거래소: 5초, 분석: 1분 주기)");
    } else {
        warn!("ContextSyncService 시작 실패: ExchangeProvider 또는 AnalyticsProvider 미설정");
    }

    // SignalProcessingService 시작 (Mock 거래소 체결 처리)
    if let Some(_signal_handle) = state.start_signal_processing(shutdown_token.clone()).await {
        info!("SignalProcessingService 시작됨 (Mock 거래소 체결 처리)");
    } else {
        warn!("SignalProcessingService 시작 실패: DB 미설정 또는 signal_rx 이미 사용됨");
    }

    // ConflictBroadcastService 시작 (Signal 충돌 WebSocket 알림)
    if let Some(_conflict_handle) = state.start_conflict_broadcast(shutdown_token.clone()).await {
        info!("ConflictBroadcastService 시작됨 (Signal 충돌 WebSocket 알림)");
    } else {
        warn!("ConflictBroadcastService 시작 실패: WebSocket 미설정 또는 conflict_rx 이미 사용됨");
    }

    // 데이터베이스에서 저장된 전략 로드
    if let Some(ref pool) = state.db_pool {
        let engine = state.strategy_engine.read().await;
        match StrategyRepository::load_strategies_into_engine(pool, &engine, state.strategy_context.clone()).await {
            Ok(count) => {
                if count > 0 {
                    info!(count, "Loaded strategies from database");
                } else {
                    info!("No strategies found in database to load");
                }
            }
            Err(e) => {
                warn!("Failed to load strategies from database: {:?}", e);
            }
        }
    }

    // 텔레그램 봇 시작 (백그라운드 태스크)
    if let Some(ref pool) = state.db_pool {
        let pool_clone = pool.clone();
        tokio::spawn(async move {
            ApiBotHandler::start(pool_clone).await;
        });
    }

    // 라우터 생성
    let app = create_router(state, metrics_handle, ws_state);

    // 서버 시작
    info!(%addr, "API server listening");
    info!("Swagger UI available at http://{}/swagger-ui", addr);
    info!("OpenAPI spec at http://{}/api-docs/openapi.json", addr);
    info!("Metrics available at http://{}/metrics", addr);
    info!("WebSocket available at ws://{}/ws", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    let shutdown_token_for_signal = shutdown_token.clone();

    // Graceful shutdown 처리 (타임아웃 포함)
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_token_for_signal))
        .await?;

    // 종료 시그널 받은 후 정리 작업
    info!("Server shutdown initiated, cleaning up...");

    // 종료 토큰 취소 (백그라운드 태스크에 종료 시그널 전파)
    shutdown_token.cancel();

    // 정리 작업에 최대 10초 대기
    let cleanup_timeout = tokio::time::timeout(Duration::from_secs(10), async {
        // 진행 중인 요청 완료 대기
        tokio::time::sleep(Duration::from_millis(500)).await;
        info!("Cleanup completed");
    })
    .await;

    if cleanup_timeout.is_err() {
        warn!("Cleanup timeout, forcing shutdown");
    }

    info!("Server stopped gracefully");

    Ok(())
}

/// Graceful shutdown 시그널 대기.
///
/// Ctrl+C 또는 SIGTERM 시그널을 수신하면 종료 토큰을 취소합니다.
///
/// # Arguments
/// * `shutdown_token` - 백그라운드 태스크에 종료를 전파할 CancellationToken
async fn shutdown_signal(shutdown_token: CancellationToken) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            warn!("Received Ctrl+C, initiating graceful shutdown...");
        }
        _ = terminate => {
            warn!("Received SIGTERM, initiating graceful shutdown...");
        }
    }

    // 모든 백그라운드 태스크에 종료 시그널 전파
    shutdown_token.cancel();
    info!("Shutdown signal propagated to background tasks");
}
