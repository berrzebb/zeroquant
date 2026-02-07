//! 전략 컨텍스트 동기화 서비스.
//!
//! ExchangeProvider와 AnalyticsProvider를 주기적으로 호출하여
//! StrategyContext를 실시간으로 업데이트합니다.
//!
//! # 설계 원칙
//!
//! - 모든 종목 식별은 ticker 문자열을 사용합니다 (Symbol 객체가 아님)
//! - 내부적으로 AnalyticsProvider는 ticker를 받아 CachedHistoricalDataProvider를 통해 데이터 조회
//! - SymbolResolver가 단일 원천(single source of truth)으로 Symbol 정보 관리

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use trader_core::{
    AnalyticsProvider, ExchangeProvider, MarketType, ScreeningPreset, StrategyContext,
};

/// 전략 컨텍스트 동기화 서비스.
///
/// 두 가지 독립적인 동기화 주기를 사용합니다:
/// - 거래소 정보: 5초마다 (계좌, 포지션, 주문)
/// - 분석 결과: 1분마다 (Global Score, RouteState, 스크리닝, 피처)
pub struct ContextSyncService {
    exchange_provider: Arc<dyn ExchangeProvider>,
    analytics_provider: Arc<dyn AnalyticsProvider>,
    context: Arc<RwLock<StrategyContext>>,
    exchange_sync_interval: Duration,
    analytics_sync_interval: Duration,
}

impl ContextSyncService {
    /// 새 서비스 인스턴스 생성.
    ///
    /// # Arguments
    ///
    /// * `exchange_provider` - 거래소 정보 제공자
    /// * `analytics_provider` - 분석 결과 제공자
    /// * `context` - 공유 컨텍스트
    /// * `exchange_sync_interval` - 거래소 동기화 주기
    /// * `analytics_sync_interval` - 분석 결과 동기화 주기
    pub fn new(
        exchange_provider: Arc<dyn ExchangeProvider>,
        analytics_provider: Arc<dyn AnalyticsProvider>,
        context: Arc<RwLock<StrategyContext>>,
        exchange_sync_interval: Duration,
        analytics_sync_interval: Duration,
    ) -> Self {
        Self {
            exchange_provider,
            analytics_provider,
            context,
            exchange_sync_interval,
            analytics_sync_interval,
        }
    }

    /// 서비스 시작 (메인 루프).
    ///
    /// 두 개의 독립적인 타이머로 거래소 정보와 분석 결과를 주기적으로 동기화합니다.
    /// CancellationToken을 통해 graceful shutdown을 지원합니다.
    pub async fn run(self, shutdown: CancellationToken) {
        let mut exchange_ticker = tokio::time::interval(self.exchange_sync_interval);
        let mut analytics_ticker = tokio::time::interval(self.analytics_sync_interval);

        loop {
            tokio::select! {
                _ = exchange_ticker.tick() => {
                    if let Err(e) = self.sync_exchange().await {
                        tracing::warn!("거래소 동기화 일부 실패: {}", e);
                    }
                }

                _ = analytics_ticker.tick() => {
                    if let Err(e) = self.sync_analytics().await {
                        tracing::error!("분석 결과 동기화 실패: {}", e);
                    }
                }

                _ = shutdown.cancelled() => {
                    tracing::info!("ContextSyncService 종료");
                    break;
                }
            }
        }
    }

    /// 거래소 정보 동기화.
    ///
    /// 계좌 정보, 포지션, 미체결 주문을 조회하여 컨텍스트를 업데이트합니다.
    /// 각 API 호출은 독립적으로 처리되어, 하나의 실패가 다른 데이터 동기화를 중단하지 않습니다.
    ///
    /// # Rate Limit 대응
    ///
    /// KIS API는 초당 호출 수 제한이 있습니다:
    /// - 실계좌: 200ms (초당 5건)
    /// - 모의계좌: 510ms (초당 2건)
    ///
    /// 보수적으로 520ms 딜레이를 적용하여 모의계좌에서도 안전하게 동작합니다.
    async fn sync_exchange(&self) -> Result<(), String> {
        // KIS API Rate Limit 대응 딜레이 (모의계좌 기준: 510ms + 안전 마진)
        const API_DELAY: Duration = Duration::from_millis(520);

        let mut errors: Vec<String> = Vec::new();

        // 1. 계좌 정보 조회 (실패 시에도 계속 진행)
        match self.exchange_provider.fetch_account().await {
            Ok(account) => {
                let mut ctx = self.context.write().await;
                ctx.update_account(account);
            }
            Err(e) => {
                errors.push(format!("계좌 조회: {}", e));
            }
        }

        tokio::time::sleep(API_DELAY).await;

        // 2. 포지션 조회 (실패 시에도 계속 진행)
        match self.exchange_provider.fetch_positions().await {
            Ok(positions) => {
                let mut ctx = self.context.write().await;
                ctx.update_positions(positions);
            }
            Err(e) => {
                errors.push(format!("포지션 조회: {}", e));
            }
        }

        tokio::time::sleep(API_DELAY).await;

        // 3. 미체결 주문 조회 (실패 시에도 계속 진행)
        match self.exchange_provider.fetch_pending_orders().await {
            Ok(orders) => {
                let mut ctx = self.context.write().await;
                ctx.update_pending_orders(orders);
            }
            Err(e) => {
                errors.push(format!("미체결 주문: {}", e));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join(", "))
        }
    }

    /// 분석 결과 동기화.
    ///
    /// 모든 분석 결과를 조회하여 컨텍스트를 업데이트합니다:
    /// - Global Score (시장별)
    /// - RouteState (종목별)
    /// - 스크리닝 결과 (프리셋별)
    /// - 구조적 피처 (종목별)
    /// - MarketRegime (종목별)
    /// - MacroEnvironment (글로벌)
    /// - MarketBreadth (글로벌)
    async fn sync_analytics(&self) -> Result<(), String> {
        // 1. Global Score 조회 (시장별 - 예: KR Stock)
        let scores = self
            .analytics_provider
            .fetch_global_scores(MarketType::Stock)
            .await
            .map_err(|e| format!("Global Score 조회 실패: {}", e))?;

        // 2. 분석 대상 종목 추출 (보유 포지션 + 전략 관심 종목)
        let ctx_read = self.context.read().await;
        let tickers: Vec<String> = ctx_read.analytics_target_tickers();
        drop(ctx_read);

        // ticker 참조 슬라이스 생성 (API는 &[&str]을 받음)
        let ticker_refs: Vec<&str> = tickers.iter().map(|s| s.as_str()).collect();

        // 3. RouteState 조회 (현재 보유 종목)
        let states = self
            .analytics_provider
            .fetch_route_states(&ticker_refs)
            .await
            .map_err(|e| format!("RouteState 조회 실패: {}", e))?;

        // 4. 스크리닝 결과 조회 (프리셋 예: "default")
        let preset = ScreeningPreset::default_preset();
        let preset_name = preset.name.clone();
        let screening = self
            .analytics_provider
            .fetch_screening(preset)
            .await
            .map_err(|e| format!("스크리닝 조회 실패: {}", e))?;

        // 5. 구조적 피처 조회
        let features = self
            .analytics_provider
            .fetch_features(&ticker_refs)
            .await
            .map_err(|e| format!("Features 조회 실패: {}", e))?;

        // 6. MarketRegime 조회
        let regimes = self
            .analytics_provider
            .fetch_market_regimes(&ticker_refs)
            .await
            .map_err(|e| format!("MarketRegime 조회 실패: {}", e))?;

        // 7. MacroEnvironment 조회 (글로벌)
        let macro_env = self
            .analytics_provider
            .fetch_macro_environment()
            .await
            .map_err(|e| format!("MacroEnvironment 조회 실패: {}", e))?;

        // 8. MarketBreadth 조회 (글로벌)
        let breadth = self
            .analytics_provider
            .fetch_market_breadth()
            .await
            .map_err(|e| format!("MarketBreadth 조회 실패: {}", e))?;

        // 9. 컨텍스트 업데이트
        let mut ctx = self.context.write().await;
        ctx.update_global_scores(scores);
        ctx.update_route_states(states);
        ctx.update_screening(preset_name, screening);
        ctx.update_features(features);
        ctx.update_market_regime(regimes);
        ctx.update_macro_environment(macro_env);
        ctx.update_market_breadth(breadth);

        tracing::debug!(ticker_count = tickers.len(), "분석 결과 동기화 완료");

        Ok(())
    }
}

/// ContextSyncService를 백그라운드 task로 시작.
///
/// 기본 동기화 주기:
/// - 거래소: 5초
/// - 분석: 1분
///
/// # Arguments
///
/// * `exchange_provider` - 거래소 정보 제공자
/// * `analytics_provider` - 분석 결과 제공자
/// * `context` - 공유 컨텍스트
/// * `shutdown` - Graceful shutdown을 위한 CancellationToken
///
/// # Returns
///
/// 백그라운드 task의 JoinHandle
pub fn start_context_sync_service(
    exchange_provider: Arc<dyn ExchangeProvider>,
    analytics_provider: Arc<dyn AnalyticsProvider>,
    context: Arc<RwLock<StrategyContext>>,
    shutdown: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    let service = ContextSyncService::new(
        exchange_provider,
        analytics_provider,
        context,
        Duration::from_secs(5),  // 거래소: 5초
        Duration::from_secs(60), // 분석: 1분
    );

    tokio::spawn(async move {
        service.run(shutdown).await;
    })
}
