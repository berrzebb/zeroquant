//! 거래소별 WebSocket 스트림 싱글턴 관리 서비스.
//!
//! credential_id별로 하나의 WebSocket 스트림을 유지하여,
//! 동일 계좌의 여러 전략이 하나의 스트림을 공유합니다.
//!
//! # 주요 컴포넌트
//!
//! - [`MarketStreamHandle`]: 스트림 제어 핸들 (구독/해제)
//! - [`get_or_create_market_stream`]: 싱글턴 스트림 생성/조회
//!
//! # 사용 예제
//!
//! ```rust,ignore
//! let handle = get_or_create_market_stream(&state, credential_id).await?;
//! handle.subscribe("005930").await?;
//! handle.subscribe("AAPL").await?;
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

use trader_core::crypto::CredentialEncryptor;
use trader_exchange::connector::kis::{KisConfig, KisOAuth};
use trader_exchange::stream::{KisKrMarketStream, KisUsMarketStream, UnifiedMarketStream};
use trader_exchange::traits::MarketStream;

use crate::websocket::aggregator::MarketDataAggregator;
use crate::websocket::SharedSubscriptionManager;

/// 거래소별 WebSocket 스트림 핸들.
///
/// - 동적 구독 제어 (심볼 추가/제거)
/// - 참조 카운트 기반 심볼 관리 (여러 전략이 동일 심볼 구독 시)
/// - credential_id별 싱글턴 보장
pub struct MarketStreamHandle {
    /// UnifiedMarketStream에 대한 제어 핸들
    stream: Arc<RwLock<UnifiedMarketStream>>,
    /// 심볼별 참조 카운트 (여러 전략이 동일 심볼을 구독할 수 있음)
    subscribed_symbols: Arc<RwLock<HashMap<String, usize>>>,
    /// 연결된 credential ID
    credential_id: Uuid,
}

impl MarketStreamHandle {
    /// 심볼 구독 추가.
    ///
    /// 이미 구독 중인 심볼이면 참조 카운트만 증가합니다.
    /// 새로운 심볼이면 실제 구독을 수행합니다.
    pub async fn subscribe(&self, symbol: &str) -> Result<(), String> {
        let mut symbols = self.subscribed_symbols.write().await;
        let count = symbols.entry(symbol.to_string()).or_insert(0);

        if *count == 0 {
            // 새로운 심볼: 실제 구독 수행
            let mut stream = self.stream.write().await;
            stream
                .subscribe_ticker(symbol)
                .await
                .map_err(|e| format!("구독 실패 ({}): {}", symbol, e))?;
            info!(symbol = %symbol, credential_id = %self.credential_id, "심볼 구독 추가");
        }

        *count += 1;
        Ok(())
    }

    /// 심볼 구독 해제.
    ///
    /// 참조 카운트가 0이 되면 실제 구독을 해제합니다.
    /// 다른 전략이 동일 심볼을 사용 중이면 구독을 유지합니다.
    pub async fn unsubscribe(&self, symbol: &str) -> Result<(), String> {
        let mut symbols = self.subscribed_symbols.write().await;

        if let Some(count) = symbols.get_mut(symbol) {
            *count = count.saturating_sub(1);

            if *count == 0 {
                symbols.remove(symbol);

                // 참조 카운트 0: 실제 구독 해제
                let mut stream = self.stream.write().await;
                stream
                    .unsubscribe(symbol)
                    .await
                    .map_err(|e| format!("구독 해제 실패 ({}): {}", symbol, e))?;
                info!(symbol = %symbol, credential_id = %self.credential_id, "심볼 구독 해제");
            }
        }

        Ok(())
    }

    /// 현재 구독 중인 심볼 목록 반환.
    pub async fn subscribed_symbols(&self) -> Vec<String> {
        let symbols = self.subscribed_symbols.read().await;
        symbols.keys().cloned().collect()
    }

    /// 현재 구독 중인 심볼 수 반환.
    pub async fn subscribed_count(&self) -> usize {
        let symbols = self.subscribed_symbols.read().await;
        symbols.len()
    }

    /// 연결된 credential ID 반환.
    pub fn credential_id(&self) -> Uuid {
        self.credential_id
    }
}

/// credential_id에 해당하는 MarketStream 핸들 가져오기 (없으면 생성).
///
/// # 싱글턴 보장
///
/// 동일한 credential_id에 대해 하나의 스트림만 유지합니다.
/// 이미 생성된 스트림이 있으면 캐시에서 반환합니다.
///
/// # Arguments
///
/// * `market_streams` - 스트림 핸들 캐시
/// * `pool` - DB 연결 풀
/// * `encryptor` - 자격증명 복호화기
/// * `kis_oauth_cache` - OAuth 토큰 캐시
/// * `credential_id` - 거래소 자격증명 ID
/// * `subscriptions` - WebSocket 구독 관리자 (이벤트 브로드캐스트용)
pub async fn get_or_create_market_stream(
    market_streams: &Arc<RwLock<HashMap<Uuid, Arc<MarketStreamHandle>>>>,
    pool: &sqlx::PgPool,
    encryptor: &CredentialEncryptor,
    kis_oauth_cache: &Arc<RwLock<HashMap<String, Arc<KisOAuth>>>>,
    credential_id: Uuid,
    subscriptions: Option<&SharedSubscriptionManager>,
) -> Result<Arc<MarketStreamHandle>, String> {
    // 1. 캐시 확인
    {
        let streams = market_streams.read().await;
        if let Some(handle) = streams.get(&credential_id) {
            return Ok(handle.clone());
        }
    }

    // 2. DB에서 자격증명 로드 → KisConfig 생성
    let kis_config = load_kis_config_from_credential(pool, encryptor, credential_id).await?;

    // 3. OAuth 생성 (KR/US 각각 별도 OAuth 인스턴스)
    let oauth_kr = create_oauth_instance(kis_oauth_cache, &kis_config, "kr").await?;
    let oauth_us = create_oauth_instance(kis_oauth_cache, &kis_config, "us").await?;

    // 4. UnifiedMarketStream 생성
    let kr_stream = KisKrMarketStream::new(oauth_kr);
    let us_stream = KisUsMarketStream::new(oauth_us);

    let mut stream = UnifiedMarketStream::new()
        .with_kr_stream(kr_stream)
        .with_us_stream(us_stream);

    // 5. 스트림 시작
    stream
        .start()
        .await
        .map_err(|e| format!("MarketStream 시작 실패: {}", e))?;

    // 6. Aggregator 연결 (stream → WebSocket 브로드캐스트)
    // stream의 이벤트를 SubscriptionManager로 전달하는 bridge 태스크를 spawn합니다.
    // 동적 구독/해제를 위해 stream은 Arc<RwLock>으로 래핑되므로,
    // bridge 태스크에서 주기적으로 next_event()를 호출합니다.
    let stream = Arc::new(RwLock::new(stream));

    if let Some(subs) = subscriptions {
        let subs = subs.clone();
        let stream_for_aggregator = stream.clone();
        let cred_id = credential_id;
        tokio::spawn(async move {
            info!(credential_id = %cred_id, "MarketStream aggregator bridge 시작");
            let aggregator = MarketDataAggregator::new(subs);
            // Arc<RwLock<UnifiedMarketStream>>에서 이벤트를 꺼내 aggregator로 전달
            loop {
                let event = {
                    let mut stream = stream_for_aggregator.write().await;
                    stream.next_event().await
                };
                match event {
                    Some(event) => {
                        aggregator.handle_event(event);
                    }
                    None => {
                        warn!(credential_id = %cred_id, "MarketStream 이벤트 스트림 종료");
                        break;
                    }
                }
            }
        });
    }

    // 7. 핸들 생성
    let handle = Arc::new(MarketStreamHandle {
        stream,
        subscribed_symbols: Arc::new(RwLock::new(HashMap::new())),
        credential_id,
    });

    // 8. 캐시에 저장
    market_streams
        .write()
        .await
        .insert(credential_id, handle.clone());

    info!(credential_id = %credential_id, "MarketStream 생성 및 시작 완료");

    Ok(handle)
}

/// DB에서 자격증명을 로드하여 KisConfig를 생성합니다.
async fn load_kis_config_from_credential(
    pool: &sqlx::PgPool,
    encryptor: &CredentialEncryptor,
    credential_id: Uuid,
) -> Result<KisConfig, String> {
    use trader_exchange::connector::kis::KisAccountType;

    // DB에서 암호화된 자격증명 조회
    let row: Option<CredentialDbRow> = sqlx::query_as(
        "SELECT encrypted_credentials, encryption_nonce, is_testnet, settings, exchange_name \
         FROM exchange_credentials \
         WHERE id = $1 AND exchange_id = 'kis' AND is_active = true",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB 조회 실패: {}", e))?;

    let row = row.ok_or_else(|| {
        format!(
            "활성 KIS 자격증명을 찾을 수 없습니다: {}",
            credential_id
        )
    })?;

    // 자격증명 복호화
    let credentials: DecryptedCredentials = encryptor
        .decrypt_json(&row.encrypted_credentials, &row.encryption_nonce)
        .map_err(|e| format!("자격증명 복호화 실패: {}", e))?;

    let account_number = credentials.get_account_number()?;

    // 계좌 유형 결정
    let account_type = if row.is_testnet {
        KisAccountType::Paper
    } else {
        KisAccountType::RealGeneral
    };

    let config = KisConfig::new(
        credentials.api_key,
        credentials.api_secret,
        account_number,
        account_type,
    );

    Ok(config)
}

/// KIS OAuth 인스턴스를 생성합니다.
///
/// KisOAuth는 Clone을 구현하지 않으므로 매번 새로 생성합니다.
/// 단, 내부적으로 토큰 캐싱이 있어 실제 토큰 발급은 최초 1회만 수행됩니다.
async fn create_oauth_instance(
    _cache: &Arc<RwLock<HashMap<String, Arc<KisOAuth>>>>,
    config: &KisConfig,
    _suffix: &str,
) -> Result<KisOAuth, String> {
    KisOAuth::new(config.clone()).map_err(|e| format!("OAuth 생성 실패: {}", e))
}

// ============================================================================
// 내부 DB 타입 (repository 직접 조회)
// ============================================================================

/// DB에서 자격증명을 조회하기 위한 Row 타입.
#[derive(sqlx::FromRow)]
struct CredentialDbRow {
    encrypted_credentials: Vec<u8>,
    encryption_nonce: Vec<u8>,
    is_testnet: bool,
    #[allow(dead_code)]
    settings: Option<serde_json::Value>,
    #[allow(dead_code)]
    exchange_name: String,
}

/// 복호화된 자격증명 구조체.
#[derive(serde::Deserialize)]
struct DecryptedCredentials {
    api_key: String,
    api_secret: String,
    #[serde(default)]
    account_number: Option<String>,
    #[serde(default)]
    additional: Option<HashMap<String, String>>,
}

impl DecryptedCredentials {
    /// 계좌번호를 가져옵니다 (top-level → additional 순으로 시도).
    fn get_account_number(&self) -> Result<String, String> {
        if let Some(ref num) = self.account_number {
            if !num.is_empty() {
                return Ok(num.clone());
            }
        }
        if let Some(ref additional) = self.additional {
            if let Some(num) = additional.get("account_number") {
                if !num.is_empty() {
                    return Ok(num.clone());
                }
            }
        }
        Err("계좌번호가 설정되지 않았습니다".to_string())
    }
}
