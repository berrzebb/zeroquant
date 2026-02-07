//! Signal 처리 서비스.
//!
//! 전략에서 생성된 Signal을 수신하여 해당 거래소로 라우팅합니다.
//!
//! # 아키텍처
//!
//! ```text
//! StrategyEngine          SignalProcessingService          MockExchangeProvider
//!       │                         │                               │
//!       │ ─── signal_rx ──────────>│                               │
//!       │                         │ ── lookup credential_id ───>  DB
//!       │                         │ <── exchange_id: "mock" ────  DB
//!       │                         │ ── process_signal() ───────> │
//!       │                         │ <── TradeResult ──────────── │
//!       │                         │                               │
//! ```
//!
//! # 거래소 라우팅
//!
//! - Mock 거래소: MockExchangeProvider.process_signal() 호출
//! - 실제 거래소: (향후) KIS/Binance Provider의 주문 API 호출

use chrono::Utc;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::repository::create_mock_provider_concrete;
use trader_core::Signal;
use trader_exchange::provider::MockExchangeProvider;

/// 거래소별 Provider 캐시.
///
/// credential_id를 키로 사용하여 Provider 인스턴스를 캐싱합니다.
type ProviderCache = HashMap<Uuid, Arc<RwLock<MockExchangeProvider>>>;

/// Signal 처리 서비스.
///
/// 전략에서 생성된 Signal을 수신하여 해당 거래소로 라우팅합니다.
pub struct SignalProcessingService {
    /// Signal 수신 채널
    signal_rx: mpsc::Receiver<Signal>,
    /// DB 연결 풀
    db_pool: PgPool,
    /// Provider 캐시 (credential_id → MockExchangeProvider)
    provider_cache: Arc<RwLock<ProviderCache>>,
}

impl SignalProcessingService {
    /// 새 서비스 생성.
    pub fn new(signal_rx: mpsc::Receiver<Signal>, db_pool: PgPool) -> Self {
        Self {
            signal_rx,
            db_pool,
            provider_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 서비스 시작.
    pub async fn run(mut self, shutdown: CancellationToken) {
        info!("SignalProcessingService 시작");

        loop {
            tokio::select! {
                Some(signal) = self.signal_rx.recv() => {
                    if let Err(e) = self.process_signal(&signal).await {
                        error!(
                            strategy_id = %signal.strategy_id,
                            ticker = %signal.ticker,
                            error = %e,
                            "Signal 처리 실패"
                        );
                    }
                }

                _ = shutdown.cancelled() => {
                    info!("SignalProcessingService 종료");
                    break;
                }
            }
        }
    }

    /// Signal 처리.
    ///
    /// 1. strategy_id에서 credential_id 조회
    /// 2. credential_id에서 exchange_id 확인
    /// 3. Mock 거래소면 MockExchangeProvider로 체결 처리
    async fn process_signal(&self, signal: &Signal) -> Result<(), String> {
        debug!(
            strategy_id = %signal.strategy_id,
            ticker = %signal.ticker,
            signal_type = %signal.signal_type,
            side = ?signal.side,
            "Signal 수신"
        );

        // 1. strategy_id에서 credential_id 조회
        let credential_id = self.get_credential_id(&signal.strategy_id).await?;

        let credential_id = match credential_id {
            Some(id) => id,
            None => {
                debug!(
                    strategy_id = %signal.strategy_id,
                    "전략에 연결된 계정 없음 - Signal 무시"
                );
                return Ok(());
            }
        };

        // 2. exchange_id 확인
        let exchange_id = self.get_exchange_id(credential_id).await?;

        // 3. 거래소별 처리
        match exchange_id.as_str() {
            "mock" => self.process_mock_signal(credential_id, signal).await,
            _ => {
                warn!(
                    exchange_id = %exchange_id,
                    "지원하지 않는 거래소 - Signal 무시"
                );
                Ok(())
            }
        }
    }

    /// Mock 거래소 Signal 처리.
    async fn process_mock_signal(
        &self,
        credential_id: Uuid,
        signal: &Signal,
    ) -> Result<(), String> {
        // Provider 가져오기 (캐시에 없으면 생성)
        let provider = self.get_or_create_provider(credential_id).await?;

        // 현재가 조회
        let provider_read = provider.read().await;
        let current_price = provider_read
            .get_current_price(&signal.ticker)
            .await
            .map_err(|e| format!("현재가 조회 실패: {:?}", e))?;
        drop(provider_read);

        // Signal 처리 (체결)
        let provider_guard = provider.read().await;
        let result = provider_guard
            .process_signal(signal, current_price, Utc::now())
            .await;

        match result {
            Ok(Some(trade)) => {
                info!(
                    symbol = %trade.symbol,
                    side = ?trade.side,
                    quantity = %trade.quantity,
                    price = %trade.price,
                    realized_pnl = ?trade.realized_pnl,
                    "Mock 체결 완료"
                );
            }
            Ok(None) => {
                debug!("Signal 처리됨 (체결 없음)");
            }
            Err(e) => {
                return Err(format!("체결 실패: {:?}", e));
            }
        }

        Ok(())
    }

    /// 전략 ID에서 credential_id 조회.
    ///
    /// strategy_id는 `{strategy_type}_{uuid}` 형식입니다.
    /// DB의 strategies 테이블에서 credential_id를 조회합니다.
    async fn get_credential_id(&self, strategy_id: &str) -> Result<Option<Uuid>, String> {
        // strategy_id에서 UUID 추출 시도
        let parts: Vec<&str> = strategy_id.rsplitn(2, '_').collect();
        if parts.is_empty() {
            return Err(format!("잘못된 strategy_id 형식: {}", strategy_id));
        }

        let uuid_str = parts[0];
        let strategy_uuid =
            Uuid::parse_str(uuid_str).map_err(|e| format!("UUID 파싱 실패: {}", e))?;

        // DB에서 credential_id 조회
        let row: Option<(Option<Uuid>,)> = sqlx::query_as(
            "SELECT credential_id FROM strategies WHERE id = $1",
        )
        .bind(strategy_uuid)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| format!("DB 조회 실패: {}", e))?;

        Ok(row.and_then(|r| r.0))
    }

    /// credential_id에서 exchange_id 조회.
    async fn get_exchange_id(&self, credential_id: Uuid) -> Result<String, String> {
        let row: (String,) = sqlx::query_as(
            "SELECT exchange_id FROM exchange_credentials WHERE id = $1",
        )
        .bind(credential_id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| format!("exchange_id 조회 실패: {}", e))?;

        Ok(row.0)
    }

    /// MockExchangeProvider 가져오기 또는 생성.
    async fn get_or_create_provider(
        &self,
        credential_id: Uuid,
    ) -> Result<Arc<RwLock<MockExchangeProvider>>, String> {
        // 1. 캐시에서 확인
        {
            let cache = self.provider_cache.read().await;
            if let Some(provider) = cache.get(&credential_id) {
                return Ok(Arc::clone(provider));
            }
        }

        // 2. 새로 생성
        let provider = create_mock_provider_concrete(&self.db_pool, credential_id)
            .await
            .map_err(|e| format!("MockExchangeProvider 생성 실패: {}", e))?;

        let provider = Arc::new(RwLock::new(provider));

        // 3. 캐시에 저장
        {
            let mut cache = self.provider_cache.write().await;
            cache.insert(credential_id, Arc::clone(&provider));
        }

        Ok(provider)
    }
}

/// SignalProcessingService 시작 헬퍼 함수.
pub fn start_signal_processing_service(
    signal_rx: mpsc::Receiver<Signal>,
    db_pool: PgPool,
    shutdown: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    let service = SignalProcessingService::new(signal_rx, db_pool);

    tokio::spawn(async move {
        service.run(shutdown).await;
    })
}
