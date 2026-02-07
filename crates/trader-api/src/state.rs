//! 모든 핸들러에서 공유되는 애플리케이션 상태.
//!
//! AppState는 모든 API 핸들러에서 공유되는 상태를 관리합니다.
//! Arc로 래핑되어 여러 요청 간에 안전하게 공유됩니다.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use trader_analytics::ml::MlService;
use trader_analytics::AnalyticsProviderImpl;
use trader_core::crypto::CredentialEncryptor;
use trader_core::{AnalyticsProvider, ExchangeProvider, MarketDataProvider, StrategyContext};
use trader_data::cache::CachedHistoricalDataProvider;
use trader_data::{RedisCache, RedisConfig, SymbolResolver};
use trader_exchange::connector::kis::KisOAuth;
use trader_execution::OrderExecutor;
use trader_notification::NotificationManager;
use trader_risk::RiskManager;
use trader_strategy::{SignalConflictEvent, StrategyEngine};
use uuid::Uuid;

use crate::repository::ExchangeProviderArc;
use crate::services::context_sync::start_context_sync_service;
use crate::services::MarketStreamHandle;
use crate::websocket::{ServerMessage, SharedSubscriptionManager};
use trader_exchange::provider::MockExchangeProvider;

/// 애플리케이션 공유 상태.
///
/// 이 구조체는 모든 API 핸들러에서 접근할 수 있는 공유 리소스를 포함합니다.
/// Axum의 State extractor를 통해 핸들러에 주입됩니다.
#[derive(Clone)]
pub struct AppState {
    /// 전략 실행 엔진 - 전략 등록, 시작/중지, 신호 처리
    pub strategy_engine: Arc<RwLock<StrategyEngine>>,

    /// 리스크 매니저 - 주문 검증, 일일 손실 한도, 변동성 필터
    pub risk_manager: Arc<RwLock<RiskManager>>,

    /// 주문 실행기 - 신호→주문 변환, 포지션 추적
    pub executor: Arc<RwLock<OrderExecutor>>,

    /// 데이터베이스 연결 풀 (TimescaleDB/PostgreSQL)
    pub db_pool: Option<sqlx::PgPool>,

    /// Redis 캐시 (trader-data의 RedisCache 활용)
    /// 전략 목록, 심볼 정보, 백테스트 결과 등 캐싱에 사용
    pub cache: Option<Arc<RedisCache>>,

    /// credential_id별 거래소 Provider 캐시 (거래소 중립).
    ///
    /// 매 요청마다 새 Provider를 생성하면 토큰 발급 제한(1분 1회)에 걸리므로,
    /// 캐시된 Provider를 재사용합니다.
    pub exchange_providers_cache: Arc<RwLock<HashMap<Uuid, ExchangeProviderArc>>>,

    /// app_key 기반 KisOAuth 캐시 (토큰 공유).
    ///
    /// 동일한 app_key를 사용하는 모든 클라이언트(국내/해외, 실계좌/모의투자)가
    /// OAuth를 공유하여 토큰 발급 rate limit (1분 1회)을 우회합니다.
    ///
    /// 키: "app_key:environment" (예: "PSxxxxxx:real" 또는 "PSxxxxxx:paper")
    pub kis_oauth_cache: Arc<RwLock<HashMap<String, Arc<KisOAuth>>>>,

    /// 자격증명 암호화 관리자 (AES-256-GCM)
    pub encryptor: Option<Arc<CredentialEncryptor>>,

    /// WebSocket 구독 관리자 - 실시간 이벤트 브로드캐스트
    pub subscriptions: Option<SharedSubscriptionManager>,

    /// 심볼 변환 서비스 - 심볼 정규화 및 display name 제공
    pub symbol_resolver: Option<Arc<SymbolResolver>>,

    /// ML 서비스 - 패턴 인식, 피처 추출, 가격 예측
    pub ml_service: Arc<RwLock<MlService>>,

    // ===== Phase 0-1 분석 인프라 =====
    /// 캐싱된 히스토리컬 데이터 제공자 (캔들 데이터 조회)
    pub data_provider: Option<Arc<CachedHistoricalDataProvider>>,

    /// 분석 결과 제공자 (AnalyticsProviderImpl)
    ///
    /// StrategyContext를 업데이트하기 위한 분석 데이터를 제공합니다.
    pub analytics_provider: Option<Arc<dyn AnalyticsProvider>>,

    /// 전략 컨텍스트 (공유)
    ///
    /// ContextSyncService가 주기적으로 업데이트하며,
    /// 모든 전략이 이 컨텍스트를 통해 분석 결과에 접근합니다.
    pub strategy_context: Option<Arc<RwLock<StrategyContext>>>,

    /// 거래소 정보 제공자 (ExchangeProvider)
    ///
    /// 계좌 정보, 포지션, 미체결 주문을 조회합니다.
    pub exchange_provider: Option<Arc<dyn ExchangeProvider>>,

    /// 시세 데이터 제공자 (MarketDataProvider)
    ///
    /// 현재 선택된 거래소를 통해 실시간 시세를 조회합니다.
    pub market_data_provider: Option<Arc<dyn MarketDataProvider>>,

    /// 서버 시작 시간 (업타임 계산용)
    pub started_at: chrono::DateTime<chrono::Utc>,

    /// API 버전
    pub version: String,

    /// 알림 매니저 (텔레그램 등)
    ///
    /// 시그널 발생, 주문 체결 등의 이벤트를 사용자에게 알림으로 전송합니다.
    pub notification_manager: Option<Arc<NotificationManager>>,

    /// Mock 거래소 Provider 캐시 (Paper Trading용).
    ///
    /// **credential_id별로** MockExchangeProvider를 캐싱합니다.
    /// 동일한 계정을 사용하는 여러 전략이 하나의 스트림 원천을 공유합니다.
    /// - 키: credential_id (Uuid)
    /// - 값: 해당 Mock 계정의 Provider (스트리밍 + 상태 관리)
    pub mock_providers: Arc<RwLock<HashMap<Uuid, Arc<MockExchangeProvider>>>>,

    /// credential_id별 MarketStream 핸들 (싱글턴).
    ///
    /// 동일 계좌의 여러 전략이 하나의 WebSocket 스트림을 공유합니다.
    /// `get_or_create_market_stream()`으로 생성/조회합니다.
    pub market_streams: Arc<RwLock<HashMap<Uuid, Arc<MarketStreamHandle>>>>,
}

impl AppState {
    /// 새로운 AppState 생성.
    ///
    /// # 인자
    /// * `strategy_engine` - 전략 실행 엔진
    /// * `risk_manager` - 리스크 매니저
    /// * `executor` - 주문 실행기
    pub fn new(
        strategy_engine: StrategyEngine,
        risk_manager: RiskManager,
        executor: OrderExecutor,
    ) -> Self {
        // 환경변수에서 암호화 마스터 키 로드 시도
        let encryptor = std::env::var("ENCRYPTION_MASTER_KEY")
            .ok()
            .and_then(|key| CredentialEncryptor::new(&key).ok())
            .map(Arc::new);

        // ML 서비스 초기화 (기본 설정으로 시작, 필요시 ONNX 모델 로드)
        let ml_service = MlService::with_defaults().expect("Failed to create MlService");

        Self {
            strategy_engine: Arc::new(RwLock::new(strategy_engine)),
            risk_manager: Arc::new(RwLock::new(risk_manager)),
            executor: Arc::new(RwLock::new(executor)),
            db_pool: None,
            cache: None,
            exchange_providers_cache: Arc::new(RwLock::new(HashMap::new())),
            kis_oauth_cache: Arc::new(RwLock::new(HashMap::new())),
            encryptor,
            subscriptions: None,
            symbol_resolver: None,
            ml_service: Arc::new(RwLock::new(ml_service)),
            data_provider: None,
            analytics_provider: None,
            strategy_context: None,
            exchange_provider: None,
            market_data_provider: None,
            started_at: chrono::Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            notification_manager: None,
            mock_providers: Arc::new(RwLock::new(HashMap::new())),
            market_streams: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 자격증명 암호화 관리자 설정.
    pub fn with_encryptor(mut self, encryptor: CredentialEncryptor) -> Self {
        self.encryptor = Some(Arc::new(encryptor));
        self
    }

    /// 암호화 관리자 설정 여부 확인.
    pub fn has_encryptor(&self) -> bool {
        self.encryptor.is_some()
    }

    /// 데이터베이스 연결 설정.
    ///
    /// DB 연결이 설정되면 SymbolResolver도 자동으로 생성됩니다.
    pub fn with_db_pool(mut self, pool: sqlx::PgPool) -> Self {
        // SymbolResolver 생성 (DB 연결 필요)
        self.symbol_resolver = Some(Arc::new(SymbolResolver::new(pool.clone())));
        self.db_pool = Some(pool);
        self
    }

    /// CachedHistoricalDataProvider 설정.
    ///
    /// 캔들 데이터를 캐싱하여 제공합니다.
    /// 이 provider는 AnalyticsProviderImpl의 의존성입니다.
    pub fn with_data_provider(mut self, provider: CachedHistoricalDataProvider) -> Self {
        self.data_provider = Some(Arc::new(provider));
        self
    }

    /// 분석 인프라 초기화 (Phase 0-1 연결).
    ///
    /// CachedHistoricalDataProvider가 먼저 설정되어 있어야 합니다.
    /// 이 메서드는 AnalyticsProviderImpl과 StrategyContext를 생성하고 연결합니다.
    ///
    /// # Panics
    ///
    /// data_provider가 설정되지 않은 경우 패닉합니다.
    pub fn with_analytics_infrastructure(mut self) -> Self {
        let data_provider = self
            .data_provider
            .clone()
            .expect("data_provider must be set before calling with_analytics_infrastructure");

        // AnalyticsProviderImpl 생성
        let analytics_provider = AnalyticsProviderImpl::new(data_provider);
        self.analytics_provider = Some(Arc::new(analytics_provider));

        // 공유 StrategyContext 생성
        self.strategy_context = Some(Arc::new(RwLock::new(StrategyContext::default())));

        tracing::info!("분석 인프라 초기화 완료 (AnalyticsProvider + StrategyContext)");
        self
    }

    /// AnalyticsProvider 설정 여부 확인.
    pub fn has_analytics_provider(&self) -> bool {
        self.analytics_provider.is_some()
    }

    /// StrategyContext 설정 여부 확인.
    pub fn has_strategy_context(&self) -> bool {
        self.strategy_context.is_some()
    }

    /// StrategyContext 참조 반환 (읽기용).
    pub async fn get_strategy_context(&self) -> Option<StrategyContext> {
        if let Some(ctx) = &self.strategy_context {
            Some(ctx.read().await.clone())
        } else {
            None
        }
    }

    /// ExchangeProvider 설정.
    ///
    /// 거래소별 Provider를 설정합니다 (예: KisExchangeProvider, BinanceProvider).
    pub fn with_exchange_provider(mut self, provider: Arc<dyn ExchangeProvider>) -> Self {
        self.exchange_provider = Some(provider);
        self
    }

    /// ExchangeProvider 설정 여부 확인.
    pub fn has_exchange_provider(&self) -> bool {
        self.exchange_provider.is_some()
    }

    /// MarketDataProvider 설정.
    ///
    /// 시세 데이터 제공자를 설정합니다 (예: KisExchangeProvider, MockExchangeProvider).
    pub fn with_market_data_provider(mut self, provider: Arc<dyn MarketDataProvider>) -> Self {
        self.market_data_provider = Some(provider);
        self
    }

    /// MarketDataProvider 설정 여부 확인.
    pub fn has_market_data_provider(&self) -> bool {
        self.market_data_provider.is_some()
    }

    /// ContextSyncService 시작.
    ///
    /// ExchangeProvider와 AnalyticsProvider가 모두 설정되어 있어야 합니다.
    /// StrategyContext를 주기적으로 동기화하는 백그라운드 태스크를 시작합니다.
    ///
    /// # Arguments
    ///
    /// * `shutdown` - Graceful shutdown을 위한 CancellationToken
    ///
    /// # Returns
    ///
    /// 백그라운드 태스크의 JoinHandle. None이면 필요한 provider가 설정되지 않은 것입니다.
    pub fn start_context_sync(
        &self,
        shutdown: CancellationToken,
    ) -> Option<tokio::task::JoinHandle<()>> {
        let exchange_provider = self.exchange_provider.clone()?;
        let analytics_provider = self.analytics_provider.clone()?;
        let strategy_context = self.strategy_context.clone()?;

        Some(start_context_sync_service(
            exchange_provider,
            analytics_provider,
            strategy_context,
            shutdown,
        ))
    }

    /// Signal 처리 서비스 시작.
    ///
    /// StrategyEngine에서 생성된 Signal을 수신하여 Mock/실제 거래소로 라우팅합니다.
    ///
    /// # Arguments
    ///
    /// * `shutdown` - Graceful shutdown을 위한 CancellationToken
    ///
    /// # Returns
    ///
    /// 백그라운드 태스크의 JoinHandle. None이면 DB가 설정되지 않은 것입니다.
    pub async fn start_signal_processing(
        &self,
        shutdown: CancellationToken,
    ) -> Option<tokio::task::JoinHandle<()>> {
        let db_pool = self.db_pool.clone()?;

        // StrategyEngine에서 signal_rx 가져오기
        let signal_rx = {
            let mut engine = self.strategy_engine.write().await;
            engine.take_signal_receiver()
        }?;

        tracing::info!("SignalProcessingService 시작");

        Some(crate::services::start_signal_processing_service(
            signal_rx,
            db_pool,
            shutdown,
        ))
    }

    /// Signal 충돌 브로드캐스트 서비스 시작.
    ///
    /// StrategyEngine에서 발생한 SignalConflictEvent를 WebSocket으로 브로드캐스트합니다.
    /// 클라이언트는 충돌 알림을 실시간으로 수신할 수 있습니다.
    ///
    /// # Arguments
    ///
    /// * `shutdown` - Graceful shutdown을 위한 CancellationToken
    ///
    /// # Returns
    ///
    /// 백그라운드 태스크의 JoinHandle. None이면 subscriptions 또는 conflict_rx가 없습니다.
    pub async fn start_conflict_broadcast(
        &self,
        shutdown: CancellationToken,
    ) -> Option<tokio::task::JoinHandle<()>> {
        let subscriptions = self.subscriptions.clone()?;

        // StrategyEngine에서 conflict_rx 가져오기
        let conflict_rx = {
            let mut engine = self.strategy_engine.write().await;
            engine.take_conflict_receiver()
        }?;

        tracing::info!("ConflictBroadcastService 시작");

        Some(tokio::spawn(async move {
            Self::run_conflict_broadcast(conflict_rx, subscriptions, shutdown).await;
        }))
    }

    /// 충돌 브로드캐스트 태스크 실행.
    async fn run_conflict_broadcast(
        mut conflict_rx: tokio::sync::mpsc::Receiver<SignalConflictEvent>,
        subscriptions: SharedSubscriptionManager,
        shutdown: CancellationToken,
    ) {
        loop {
            tokio::select! {
                Some(event) = conflict_rx.recv() => {
                    // SignalConflictEvent → SignalConflictData 변환
                    let data = crate::websocket::SignalConflictData {
                        strategy_id: event.strategy_id,
                        ticker: event.ticker,
                        signal_type: format!("{:?}", event.signal_type),
                        conflict_type: event.conflict_type,
                        message: event.message,
                        timestamp: event.timestamp.timestamp_millis(),
                    };

                    // WebSocket 브로드캐스트
                    let message = ServerMessage::SignalConflict(data);
                    if let Err(e) = subscriptions.broadcast(message) {
                        tracing::debug!(error = %e, "충돌 알림 브로드캐스트 실패");
                    } else {
                        tracing::debug!("Signal 충돌 알림 브로드캐스트");
                    }
                }

                _ = shutdown.cancelled() => {
                    tracing::info!("ConflictBroadcastService 종료");
                    break;
                }
            }
        }
    }

    /// Redis 캐시 설정.
    ///
    /// trader-data의 RedisCache를 사용하여 API 응답 캐싱을 활성화합니다.
    /// 전략 목록(5분 TTL), 심볼 정보(1시간 TTL) 등 자주 조회되는 데이터를 캐싱합니다.
    pub fn with_cache(mut self, cache: RedisCache) -> Self {
        self.cache = Some(Arc::new(cache));
        self
    }

    /// Redis URL에서 캐시 연결 생성 (편의 메서드).
    pub async fn with_redis_url(mut self, redis_url: &str) -> Self {
        let config = RedisConfig {
            url: redis_url.to_string(),
            default_ttl_secs: 300, // 기본 5분
            pool_size: 10,
        };
        match RedisCache::connect(&config).await {
            Ok(cache) => {
                tracing::info!("Redis 캐시 연결 성공");
                self.cache = Some(Arc::new(cache));
            }
            Err(e) => {
                tracing::warn!("Redis 캐시 연결 실패: {}. 캐시 없이 계속합니다.", e);
            }
        }
        self
    }

    /// WebSocket 구독 관리자 설정.
    ///
    /// REST API에서 실시간 이벤트를 브로드캐스트할 수 있게 합니다.
    pub fn with_subscriptions(mut self, subscriptions: SharedSubscriptionManager) -> Self {
        self.subscriptions = Some(subscriptions);
        self
    }

    /// WebSocket 메시지 브로드캐스트.
    ///
    /// 연결된 모든 클라이언트에게 메시지를 전송합니다.
    /// subscriptions가 설정되지 않은 경우 무시됩니다.
    pub fn broadcast(&self, message: ServerMessage) {
        if let Some(ref subs) = self.subscriptions {
            let _ = subs.broadcast(message);
        }
    }

    /// WebSocket 구독 관리자 설정 여부 확인.
    pub fn has_subscriptions(&self) -> bool {
        self.subscriptions.is_some()
    }

    /// 서버 업타임(초) 반환.
    pub fn uptime_secs(&self) -> i64 {
        chrono::Utc::now()
            .signed_duration_since(self.started_at)
            .num_seconds()
    }

    /// 데이터베이스 연결 상태 확인.
    pub async fn is_db_healthy(&self) -> bool {
        if let Some(pool) = &self.db_pool {
            sqlx::query("SELECT 1").fetch_one(pool).await.is_ok()
        } else {
            false
        }
    }

    /// Redis 캐시 연결 상태 확인.
    pub async fn is_redis_healthy(&self) -> bool {
        if let Some(cache) = &self.cache {
            cache.health_check().await.unwrap_or(false)
        } else {
            false
        }
    }

    /// 캐시 설정 여부 확인.
    pub fn has_cache(&self) -> bool {
        self.cache.is_some()
    }

    /// 알림 매니저 설정.
    ///
    /// 텔레그램 등으로 알림을 전송하는 매니저를 설정합니다.
    pub fn with_notification_manager(mut self, manager: NotificationManager) -> Self {
        self.notification_manager = Some(Arc::new(manager));
        self
    }

    /// 알림 매니저 설정 여부 확인.
    pub fn has_notification_manager(&self) -> bool {
        self.notification_manager.is_some()
    }

    /// SymbolResolver 설정 여부 확인.
    pub fn has_symbol_resolver(&self) -> bool {
        self.symbol_resolver.is_some()
    }

    /// SymbolResolver 캐시 클리어.
    ///
    /// 심볼 정보가 DB에서 업데이트된 후 호출하여 메모리 캐시를 무효화합니다.
    /// 다음 조회 시 최신 DB 데이터를 다시 로드합니다.
    pub async fn clear_symbol_cache(&self) {
        if let Some(ref resolver) = self.symbol_resolver {
            resolver.clear_cache().await;
            tracing::info!("SymbolResolver 캐시 클리어 완료");
        }
    }

    /// SymbolResolver 캐시 크기 조회.
    pub async fn symbol_cache_size(&self) -> usize {
        if let Some(ref resolver) = self.symbol_resolver {
            resolver.cache_size().await
        } else {
            0
        }
    }

    /// 여러 심볼의 display name을 배치로 조회.
    ///
    /// SymbolResolver가 설정되지 않은 경우 빈 HashMap 반환.
    ///
    /// # Arguments
    /// * `symbols` - 조회할 심볼 목록
    /// * `use_english` - 영문명 사용 여부
    ///
    /// # Returns
    /// HashMap<심볼, display_name> (예: {"005930" => "005930(삼성전자)"})
    pub async fn get_display_names(
        &self,
        symbols: &[String],
        use_english: bool,
    ) -> HashMap<String, String> {
        if let Some(ref resolver) = self.symbol_resolver {
            resolver
                .get_display_names_batch(symbols, use_english)
                .await
                .unwrap_or_default()
        } else {
            HashMap::new()
        }
    }

    /// 단일 심볼의 display name 조회.
    ///
    /// SymbolResolver가 설정되지 않은 경우 원본 심볼 반환.
    pub async fn get_display_name(&self, symbol: &str, use_english: bool) -> String {
        if let Some(ref resolver) = self.symbol_resolver {
            resolver
                .to_display_string(symbol, use_english)
                .await
                .unwrap_or_else(|_| symbol.to_string())
        } else {
            symbol.to_string()
        }
    }

    /// ONNX 모델을 로드하여 ML 예측 활성화.
    ///
    /// # Arguments
    /// * `model_path` - ONNX 모델 파일 경로
    /// * `model_name` - 모델 식별 이름
    pub async fn load_ml_model(
        &self,
        model_path: impl AsRef<std::path::Path>,
        model_name: &str,
    ) -> Result<(), trader_analytics::ml::MlError> {
        let ml_service = self.ml_service.read().await;
        ml_service.load_onnx_model(model_path, model_name).await
    }

    /// 현재 로드된 ML 모델 이름 반환.
    pub async fn current_ml_model(&self) -> String {
        let ml_service = self.ml_service.read().await;
        ml_service.current_model_name().await
    }

    /// ML 예측 기능 활성화 상태 확인.
    pub async fn is_ml_prediction_enabled(&self) -> bool {
        let ml_service = self.ml_service.read().await;
        ml_service.is_prediction_enabled()
    }

    // =========================================================================
    // 캐시 유틸리티 메서드
    // =========================================================================

    /// 캐시에서 값을 조회합니다.
    ///
    /// 캐시가 설정되지 않은 경우 None을 반환합니다.
    pub async fn cache_get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        if let Some(cache) = &self.cache {
            cache.get(key).await.ok().flatten()
        } else {
            None
        }
    }

    /// 캐시에 값을 저장합니다.
    ///
    /// 캐시가 설정되지 않은 경우 아무 동작도 하지 않습니다.
    pub async fn cache_set<T: serde::Serialize>(&self, key: &str, value: &T, ttl_secs: u64) {
        if let Some(cache) = &self.cache {
            let _ = cache.set_with_ttl(key, value, ttl_secs).await;
        }
    }

    /// 캐시에서 값을 조회하거나, 없으면 fetch 함수를 호출하여 저장 후 반환합니다.
    ///
    /// # Arguments
    /// * `key` - 캐시 키
    /// * `ttl_secs` - TTL (초)
    /// * `fetch` - 캐시 미스 시 호출할 async 함수
    ///
    /// # Example
    /// ```ignore
    /// let strategies = state.cache_get_or_fetch(
    ///     "strategies:list",
    ///     300, // 5분
    ///     || async { fetch_strategies_from_db().await }
    /// ).await;
    /// ```
    pub async fn cache_get_or_fetch<T, F, Fut>(&self, key: &str, ttl_secs: u64, fetch: F) -> T
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        // 캐시 조회 시도
        if let Some(cached) = self.cache_get::<T>(key).await {
            return cached;
        }

        // 캐시 미스: 데이터 fetch
        let value = fetch().await;

        // 캐시에 저장
        self.cache_set(key, &value, ttl_secs).await;

        value
    }

    /// 캐시 키를 무효화(삭제)합니다.
    pub async fn cache_invalidate(&self, key: &str) {
        if let Some(cache) = &self.cache {
            let _ = cache.delete(key).await;
        }
    }

    /// 패턴에 일치하는 모든 캐시 키를 무효화합니다.
    ///
    /// # Example
    /// ```ignore
    /// // 모든 전략 관련 캐시 삭제
    /// state.cache_invalidate_pattern("strategies:*").await;
    /// ```
    pub async fn cache_invalidate_pattern(&self, pattern: &str) {
        if let Some(cache) = &self.cache {
            let _ = cache.delete_pattern(pattern).await;
        }
    }
}

/// 테스트용 AppState 생성 헬퍼.
///
/// 실제 DB 연결 없이 테스트할 수 있는 최소한의 상태를 생성합니다.
/// 분석 인프라(AnalyticsProvider, StrategyContext)는 포함되지 않습니다.
#[cfg(any(test, feature = "test-utils"))]
pub fn create_test_state() -> AppState {
    use rust_decimal_macros::dec;
    use trader_execution::ConversionConfig;
    use trader_risk::RiskConfig;
    use trader_strategy::EngineConfig;

    let strategy_engine = StrategyEngine::new(EngineConfig::default());
    let risk_manager = RiskManager::new(RiskConfig::default(), dec!(10000));
    let executor = OrderExecutor::new_complete(
        RiskManager::new(RiskConfig::default(), dec!(10000)),
        "test_exchange",
        ConversionConfig::default(),
    );
    let ml_service = MlService::with_defaults().expect("Failed to create MlService for test");

    let mut state = AppState::new(strategy_engine, risk_manager, executor);
    state.ml_service = Arc::new(RwLock::new(ml_service));
    // 테스트용 StrategyContext 추가 (분석 인프라 없이도 컨텍스트 접근 가능)
    state.strategy_context = Some(Arc::new(RwLock::new(StrategyContext::default())));
    state.market_streams = Arc::new(RwLock::new(HashMap::new()));
    state
}
