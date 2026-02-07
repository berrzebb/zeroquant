//! Credential 관리 Repository (거래소 중립)
//!
//! Single Source of Truth for credential 복호화 및 거래소 클라이언트 생성.
//!
//! # 설계 원칙
//! - 모든 credential 관련 로직은 이 모듈을 통해서만 처리
//! - OAuth 토큰은 DB에 캐싱하여 rate limit 대응 (1분당 1회 제한)
//! - **거래소 중립**: 특정 거래소에 의존하지 않음

use super::kis_token::KisTokenRepository;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};
use trader_core::CredentialEncryptor;
use trader_core::{ExchangeProvider, MarketDataProvider};
use trader_exchange::connector::kis::KisAccountType;
use trader_exchange::connector::{KisConfig, KisOAuth};
use trader_exchange::connector::kis::KisClient;
use trader_exchange::provider::{
    KisProvider, MockConfig, MockExchangeProvider,
};

/// Exchange 및 MarketData Provider 번들.
///
/// 하나의 credential에 대해 두 가지 provider를 모두 제공합니다.
pub struct ProviderBundle {
    /// 계좌/포지션/주문 조회용
    pub exchange: Arc<dyn ExchangeProvider>,
    /// 시세 데이터 조회용
    pub market_data: Arc<dyn MarketDataProvider>,
}
use uuid::Uuid;

/// 암호화된 credential 구조
///
/// # 보안 설계
/// DB에는 이 구조체 전체가 암호화되어 저장됩니다.
/// APP_KEY, APP_SECRET, ACCOUNT_NUMBER 모두 암호화됩니다.
///
/// # Backward Compatibility
/// account_number는 Optional로, additional 맵에서 fallback 읽기 지원
#[derive(Debug, serde::Deserialize)]
struct EncryptedCredentials {
    api_key: String,
    api_secret: String,
    /// 계좌번호 (최상위 필드, 없으면 additional에서 읽음)
    #[serde(default)]
    account_number: Option<String>,
    #[serde(default)]
    additional: Option<HashMap<String, String>>,
}

impl EncryptedCredentials {
    /// 계좌번호 가져오기 (최상위 필드 우선, 없으면 additional에서)
    fn get_account_number(&self) -> Result<String, String> {
        // 1. 최상위 필드 확인
        if let Some(ref acc) = self.account_number {
            if !acc.is_empty() {
                return Ok(acc.clone());
            }
        }

        // 2. additional 맵에서 확인
        if let Some(ref additional) = self.additional {
            if let Some(acc) = additional.get("account_number") {
                if !acc.is_empty() {
                    return Ok(acc.clone());
                }
            }
        }

        Err("account_number가 없습니다. DB credential을 확인하세요.".to_string())
    }
}

/// DB에서 조회한 credential row
#[derive(sqlx::FromRow)]
struct CredentialRow {
    encrypted_credentials: Vec<u8>,
    encryption_nonce: Vec<u8>,
    is_testnet: bool,
    settings: Option<serde_json::Value>,
    exchange_name: String,
}

/// ExchangeProvider의 Arc 타입 별칭 (거래소 중립).
///
/// 각 credential에 대해 하나의 ExchangeProvider 인스턴스를 공유합니다.
pub type ExchangeProviderArc = Arc<dyn trader_core::ExchangeProvider>;

/// ISA 계좌 여부 판단
fn is_isa_account(settings: &Option<serde_json::Value>, exchange_name: &str) -> bool {
    // settings에 account_type 필드 확인
    if let Some(settings) = settings {
        if let Some(account_type) = settings.get("account_type").and_then(|v| v.as_str()) {
            if account_type == "isa" {
                return true;
            }
        }
    }

    // exchange_name에 "ISA" 포함 여부 확인
    exchange_name.to_uppercase().contains("ISA")
}

/// Active credential ID 조회
///
/// app_settings 테이블에서 active_credential_id를 조회합니다.
pub async fn get_active_credential_id(pool: &PgPool) -> Result<Uuid, String> {
    let row: (String,) = sqlx::query_as(
        "SELECT setting_value FROM app_settings WHERE setting_key = 'active_credential_id' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Active credential 조회 실패: {}", e))?
    .ok_or_else(|| "Active credential이 설정되지 않았습니다.".to_string())?;

    Uuid::parse_str(&row.0).map_err(|e| format!("Invalid credential UUID: {}", e))
}

/// Credential 정보 (외부 노출용)
///
/// rate limit, ISA 계좌 판단 등에 사용
#[derive(Debug, Clone)]
pub struct CredentialInfo {
    pub is_testnet: bool,
    pub is_isa_account: bool,
    pub exchange_name: String,
}

/// Credential 정보 조회
///
/// is_testnet, is_isa_account 등 credential 메타 정보를 반환합니다.
/// 암호화된 API 키/시크릿은 포함하지 않습니다.
pub async fn get_credential_info(
    pool: &PgPool,
    credential_id: Uuid,
) -> Result<Option<CredentialInfo>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct InfoRow {
        is_testnet: bool,
        settings: Option<serde_json::Value>,
        exchange_name: String,
    }

    let row: Option<InfoRow> = sqlx::query_as(
        "SELECT is_testnet, settings, exchange_name FROM exchange_credentials WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| CredentialInfo {
        is_testnet: r.is_testnet,
        is_isa_account: is_isa_account(&r.settings, &r.exchange_name),
        exchange_name: r.exchange_name,
    }))
}

/// 거래소 Provider 생성 (거래소 중립)
///
/// # Single Source of Truth for Exchange Integration
///
/// 이 함수는 credential로부터 ExchangeProvider를 생성하는 **유일한 원천**입니다.
/// 거래소 특정 타입을 직접 사용하지 않고, KisClient 통합 인터페이스를 사용하세요.
///
/// # Arguments
///
/// * `pool` - DB 연결 풀
/// * `encryptor` - Credential 암호화/복호화 관리자
/// * `credential_id` - Credential UUID
/// * `cached_oauth` - 캐시된 OAuth (선택적)
///
/// # Returns
///
/// 거래소 Provider 쌍 (KR, US)
pub async fn create_exchange_providers_from_credential(
    pool: &PgPool,
    encryptor: &CredentialEncryptor,
    credential_id: Uuid,
    cached_oauth: Option<Arc<KisOAuth>>,
) -> Result<ExchangeProviderArc, String> {
    // 1. Credential 조회
    let row: CredentialRow = sqlx::query_as(
        r#"
        SELECT encrypted_credentials, encryption_nonce, is_testnet, settings, exchange_name
        FROM exchange_credentials
        WHERE id = $1 AND exchange_id = 'kis' AND is_active = true
        "#,
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Credential 조회 실패: {}", e))?
    .ok_or_else(|| "해당 credential을 찾을 수 없습니다.".to_string())?;

    info!(
        "거래소 계좌 로드: id={}, name={}, is_testnet={}, is_isa={}",
        credential_id,
        row.exchange_name,
        row.is_testnet,
        is_isa_account(&row.settings, &row.exchange_name)
    );

    // 2. Credential 복호화
    let credentials: EncryptedCredentials = encryptor
        .decrypt_json(&row.encrypted_credentials, &row.encryption_nonce)
        .map_err(|e| format!("Credential 복호화 실패: {}", e))?;

    // 3. 계좌번호 추출 (최상위 필드 또는 additional에서 fallback)
    let account_number = credentials.get_account_number()?;

    // 4. 계좌 유형 결정
    let account_type = if row.is_testnet {
        KisAccountType::Paper
    } else if is_isa_account(&row.settings, &row.exchange_name) {
        KisAccountType::RealIsa
    } else {
        KisAccountType::RealGeneral
    };

    info!(
        "거래소 클라이언트 생성: credential_id={}, account_type={:?}, account={}***",
        credential_id,
        account_type,
        if account_number.len() > 4 {
            &account_number[..4]
        } else {
            &account_number
        }
    );

    // 6. KisConfig 생성
    let config = KisConfig::new(
        credentials.api_key.clone(),
        credentials.api_secret.clone(),
        account_number.clone(),
        account_type,
    );

    // 7. OAuth 생성 (캐시된 것이 있으면 재사용)
    let oauth = if let Some(cached) = cached_oauth {
        info!("OAuth 캐시 재사용: credential_id={}", credential_id);
        cached
    } else {
        let new_oauth =
            Arc::new(KisOAuth::new(config.clone()).map_err(|e| format!("OAuth 생성 실패: {}", e))?);

        // DB에서 유효한 토큰 조회 (rate limit 대응)
        let environment = if row.is_testnet { "paper" } else { "real" };
        if let Some(cached_token) =
            KisTokenRepository::load_valid_token(pool, credential_id, environment).await
        {
            // DB에 유효한 토큰이 있으면 OAuth에 설정
            debug!("DB 캐시된 토큰 사용: credential_id={}", credential_id);
            new_oauth.set_cached_token(cached_token).await;
        } else {
            // DB에 유효한 토큰이 없으면 새로 발급
            info!(
                "DB에 유효한 토큰 없음, 새로 발급: credential_id={}",
                credential_id
            );
            match new_oauth.refresh_and_get_token().await {
                Ok(token) => {
                    // 발급받은 토큰을 DB에 저장
                    if let Err(e) =
                        KisTokenRepository::save_token(pool, credential_id, environment, &token).await
                    {
                        warn!("토큰 DB 저장 실패 (계속 진행): {}", e);
                    }
                }
                Err(e) => {
                    // Fallback: 완화된 조건으로 DB 재조회
                    warn!("토큰 발급 실패, fallback 조회 시도: {}", e);
                    if let Some(fb_token) =
                        KisTokenRepository::load_any_valid_token(pool, credential_id, environment).await
                    {
                        info!("Fallback 토큰 사용: credential_id={}", credential_id);
                        new_oauth.set_cached_token(fb_token).await;
                    } else {
                        return Err(format!("OAuth 토큰 획득 실패: {}", e));
                    }
                }
            }
        }

        new_oauth
    };

    // 8. 통합 KisClient 생성 (내부적으로 KR/US 클라이언트 자동 생성)
    let client = Arc::new(
        KisClient::new(oauth)
            .map_err(|e| format!("KIS 클라이언트 생성 실패: {}", e))?,
    );

    // 9. ExchangeProvider로 래핑 (KisExchangeProvider가 KR+US 모두 처리)
    let provider: Arc<dyn ExchangeProvider> =
        Arc::new(KisProvider::new(client));

    Ok(provider)
}

/// KIS 통합 클라이언트 생성 헬퍼 (체결 내역 조회 등)
///
/// # 용도
///
/// ExchangeProvider trait에 없는 거래소 특화 API (체결 내역 조회 등)를 위한 헬퍼입니다.
/// 가능한 한 `create_exchange_providers_from_credential()`를 사용하고,
/// 정말 필요한 경우에만 이 함수를 사용하세요.
pub async fn create_kis_client_from_credential(
    pool: &PgPool,
    encryptor: &CredentialEncryptor,
    credential_id: Uuid,
) -> Result<Arc<KisClient>, String> {
    // create_exchange_providers_from_credential()와 동일한 로직
    let row: CredentialRow = sqlx::query_as(
        r#"
        SELECT encrypted_credentials, encryption_nonce, is_testnet, settings, exchange_name
        FROM exchange_credentials
        WHERE id = $1 AND exchange_id = 'kis' AND is_active = true
        "#,
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Credential 조회 실패: {}", e))?
    .ok_or_else(|| "해당 credential을 찾을 수 없습니다.".to_string())?;

    let credentials: EncryptedCredentials = encryptor
        .decrypt_json(&row.encrypted_credentials, &row.encryption_nonce)
        .map_err(|e| format!("Credential 복호화 실패: {}", e))?;

    let account_number = credentials.get_account_number()?;

    let account_type = if row.is_testnet {
        KisAccountType::Paper
    } else if is_isa_account(&row.settings, &row.exchange_name) {
        KisAccountType::RealIsa
    } else {
        KisAccountType::RealGeneral
    };

    let config = KisConfig::new(
        credentials.api_key,
        credentials.api_secret,
        account_number,
        account_type,
    );

    let oauth =
        Arc::new(KisOAuth::new(config.clone()).map_err(|e| format!("OAuth 생성 실패: {}", e))?);

    // DB에서 유효한 토큰 조회 (rate limit 대응)
    let environment = if row.is_testnet { "paper" } else { "real" };
    if let Some(cached_token) =
        KisTokenRepository::load_valid_token(pool, credential_id, environment).await
    {
        // DB에 유효한 토큰이 있으면 OAuth에 설정
        debug!("DB 캐시된 토큰 사용: credential_id={}", credential_id);
        oauth.set_cached_token(cached_token).await;
    } else {
        // DB에 유효한 토큰이 없으면 새로 발급
        info!(
            "DB에 유효한 토큰 없음, 새로 발급: credential_id={}",
            credential_id
        );
        match oauth.refresh_and_get_token().await {
            Ok(token) => {
                // 발급받은 토큰을 DB에 저장
                if let Err(e) =
                    KisTokenRepository::save_token(pool, credential_id, environment, &token).await
                {
                    warn!("토큰 DB 저장 실패 (계속 진행): {}", e);
                }
            }
            Err(e) => {
                // Fallback: 완화된 조건으로 DB 재조회
                warn!("토큰 발급 실패, fallback 조회 시도: {}", e);
                if let Some(fb_token) =
                    KisTokenRepository::load_any_valid_token(pool, credential_id, environment).await
                {
                    info!("Fallback 토큰 사용: credential_id={}", credential_id);
                    oauth.set_cached_token(fb_token).await;
                } else {
                    return Err(format!("OAuth 토큰 획득 실패: {}", e));
                }
            }
        }
    }

    // 통합 KisClient 생성
    Ok(Arc::new(
        KisClient::new(oauth)
            .map_err(|e| format!("KIS 클라이언트 생성 실패: {}", e))?,
    ))
}

/// 거래소 중립적 Provider 생성
///
/// exchange_id에 따라 적절한 ExchangeProvider를 생성합니다.
/// - mock: MockExchangeProvider (API 키 불필요, DB에서 상태 관리)
/// - kis: KIS Provider (KR 마켓용)
///
/// # Arguments
///
/// * `pool` - DB 연결 풀
/// * `encryptor` - Credential 암호화/복호화 관리자 (kis 등 실제 거래소용)
/// * `credential_id` - Credential UUID
///
/// # Returns
///
/// 거래소 중립적 ExchangeProvider
pub async fn create_provider_for_credential(
    pool: &PgPool,
    encryptor: &CredentialEncryptor,
    credential_id: Uuid,
) -> Result<Arc<dyn ExchangeProvider>, String> {
    // 1. exchange_id 조회
    let exchange_id: String = sqlx::query_scalar(
        "SELECT exchange_id FROM exchange_credentials WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("exchange_id 조회 실패: {}", e))?
    .ok_or_else(|| "해당 credential을 찾을 수 없습니다.".to_string())?;

    info!("Provider 생성: exchange_id={}, credential_id={}", exchange_id, credential_id);

    match exchange_id.as_str() {
        "mock" => create_mock_provider(pool, credential_id).await,
        "kis" => {
            // KIS는 기존 로직 재사용 (KR Provider 반환)
            let provider = create_exchange_providers_from_credential(
                pool, encryptor, credential_id, None,
            ).await?;
            Ok(provider)
        }
        _ => Err(format!("지원하지 않는 거래소입니다: {}", exchange_id)),
    }
}

/// Mock 거래소용 Provider 생성 (encryptor 불필요)
///
/// encryptor가 없어도 Mock Provider를 생성할 수 있습니다.
/// ENCRYPTION_MASTER_KEY 환경변수가 없는 개발 환경에서 사용합니다.
pub async fn create_provider_for_mock_credential(
    pool: &PgPool,
    credential_id: Uuid,
) -> Result<Arc<dyn ExchangeProvider>, String> {
    // exchange_id 확인
    let exchange_id: String = sqlx::query_scalar(
        "SELECT exchange_id FROM exchange_credentials WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("exchange_id 조회 실패: {}", e))?
    .ok_or_else(|| "해당 credential을 찾을 수 없습니다.".to_string())?;

    if exchange_id != "mock" {
        return Err(format!(
            "이 함수는 Mock 거래소만 지원합니다. (현재: {})",
            exchange_id
        ));
    }

    create_mock_provider(pool, credential_id).await
}

/// Mock Provider 생성
///
/// DB에서 Mock 설정을 읽어 MockExchangeProvider를 생성합니다.
/// Arc<dyn ExchangeProvider>를 반환하며, process_signal이 필요하면 create_mock_provider_concrete 사용
pub async fn create_mock_provider(
    pool: &PgPool,
    credential_id: Uuid,
) -> Result<Arc<dyn ExchangeProvider>, String> {
    // settings에서 Mock 설정 읽기
    let settings: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT settings FROM exchange_credentials WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Mock 설정 조회 실패: {}", e))?
    .flatten();

    // 설정에서 값 추출 (기본값 적용)
    let initial_balance = settings
        .as_ref()
        .and_then(|s| s.get("initial_balance"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Decimal>().ok())
        .unwrap_or_else(|| Decimal::new(10_000_000, 0)); // 기본 1천만원

    let commission_rate = settings
        .as_ref()
        .and_then(|s| s.get("commission_rate"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Decimal>().ok())
        .unwrap_or_else(|| Decimal::new(15, 5)); // 기본 0.015%

    let slippage_rate = settings
        .as_ref()
        .and_then(|s| s.get("slippage_rate"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Decimal>().ok())
        .unwrap_or_else(|| Decimal::new(1, 4)); // 기본 0.01%

    let market_type = settings
        .as_ref()
        .and_then(|s| s.get("market_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("stock_kr")
        .to_string();

    let currency = settings
        .as_ref()
        .and_then(|s| s.get("currency"))
        .and_then(|v| v.as_str())
        .unwrap_or("KRW")
        .to_string();

    let config = MockConfig {
        initial_balance,
        commission_rate,
        slippage_rate,
        market_type,
        currency,
    };

    info!(
        "Mock Provider 생성: credential_id={}, balance={}, commission={}%, market={}",
        credential_id, initial_balance, commission_rate, config.market_type
    );

    let provider = MockExchangeProvider::new(credential_id, config, pool.clone())
        .await
        .map_err(|e| format!("Mock Provider 생성 실패: {}", e))?;

    Ok(Arc::new(provider))
}

/// Mock Provider 생성 (구체적 타입 반환)
///
/// SignalProcessingService에서 process_signal()을 호출하기 위해 구체적 타입을 반환합니다.
pub async fn create_mock_provider_concrete(
    pool: &PgPool,
    credential_id: Uuid,
) -> Result<MockExchangeProvider, String> {
    // settings에서 Mock 설정 읽기
    let settings: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT settings FROM exchange_credentials WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Mock 설정 조회 실패: {}", e))?
    .flatten();

    // 설정에서 값 추출 (기본값 적용)
    let initial_balance = settings
        .as_ref()
        .and_then(|s| s.get("initial_balance"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Decimal>().ok())
        .unwrap_or_else(|| Decimal::new(10_000_000, 0)); // 기본 1천만원

    let commission_rate = settings
        .as_ref()
        .and_then(|s| s.get("commission_rate"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Decimal>().ok())
        .unwrap_or_else(|| Decimal::new(15, 5)); // 기본 0.015%

    let slippage_rate = settings
        .as_ref()
        .and_then(|s| s.get("slippage_rate"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Decimal>().ok())
        .unwrap_or_else(|| Decimal::new(1, 4)); // 기본 0.01%

    let market_type = settings
        .as_ref()
        .and_then(|s| s.get("market_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("stock_kr")
        .to_string();

    let currency = settings
        .as_ref()
        .and_then(|s| s.get("currency"))
        .and_then(|v| v.as_str())
        .unwrap_or("KRW")
        .to_string();

    let config = MockConfig {
        initial_balance,
        commission_rate,
        slippage_rate,
        market_type,
        currency,
    };

    info!(
        "Mock Provider (concrete) 생성: credential_id={}, balance={}, commission={}%",
        credential_id, initial_balance, commission_rate
    );

    MockExchangeProvider::new(credential_id, config, pool.clone())
        .await
        .map_err(|e| format!("Mock Provider 생성 실패: {}", e))
}

/// credential_id로 ExchangeProvider와 MarketDataProvider 번들 생성.
///
/// Mock과 KIS 거래소 모두 지원합니다.
/// - Mock: 동일한 MockExchangeProvider를 두 trait에 사용
/// - KIS: KisExchangeProvider (ExchangeProvider + MarketDataProvider) 생성
pub async fn create_provider_bundle(
    pool: &PgPool,
    encryptor: Option<&CredentialEncryptor>,
    credential_id: Uuid,
) -> Result<ProviderBundle, String> {
    // exchange_id 조회
    let exchange_id: String = sqlx::query_scalar(
        "SELECT exchange_id FROM exchange_credentials WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("exchange_id 조회 실패: {}", e))?
    .ok_or_else(|| "해당 credential을 찾을 수 없습니다.".to_string())?;

    match exchange_id.as_str() {
        "mock" => {
            // Mock: 동일 인스턴스를 두 trait에 사용
            let provider = create_mock_provider_concrete(pool, credential_id).await?;
            let arc_provider = Arc::new(provider);
            Ok(ProviderBundle {
                exchange: arc_provider.clone(),
                market_data: arc_provider,
            })
        }
        "kis" => {
            // KIS: 통합 KisProvider 사용 (ExchangeProvider + MarketDataProvider)
            let encryptor = encryptor.ok_or("KIS는 encryptor가 필요합니다.")?;

            // 통합 KisClient 생성 (공유 OAuth 사용)
            let client = create_kis_client(pool, encryptor, credential_id).await?;

            // KisProvider는 두 trait 모두 구현
            let provider = Arc::new(KisProvider::new(client));

            Ok(ProviderBundle {
                exchange: provider.clone(),
                market_data: provider,
            })
        }
        _ => Err(format!("지원하지 않는 거래소: {}", exchange_id)),
    }
}

/// KIS 통합 클라이언트 생성 (공유 OAuth 사용)
async fn create_kis_client(
    pool: &PgPool,
    encryptor: &CredentialEncryptor,
    credential_id: Uuid,
) -> Result<Arc<KisClient>, String> {
    // credential 정보 조회
    let row: Option<(Vec<u8>, Vec<u8>, bool, Option<serde_json::Value>)> = sqlx::query_as(
        r#"
        SELECT encrypted_credentials, encryption_nonce, is_testnet, settings
        FROM exchange_credentials
        WHERE id = $1 AND exchange_id = 'kis'
        "#,
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("credential 조회 실패: {}", e))?;

    let (encrypted, nonce, is_testnet, settings) =
        row.ok_or_else(|| "KIS credential을 찾을 수 없습니다.".to_string())?;

    // 복호화
    let credentials: EncryptedCredentials = encryptor
        .decrypt_json(&encrypted, &nonce)
        .map_err(|e| format!("복호화 실패: {}", e))?;

    let account_number = credentials.get_account_number()?;

    // 계좌 유형 결정
    let account_type = if is_testnet {
        KisAccountType::Paper
    } else {
        // settings에서 ISA 여부 확인
        let is_isa = settings
            .as_ref()
            .and_then(|s| s.get("account_type"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase().contains("isa"))
            .unwrap_or(false);

        if is_isa {
            KisAccountType::RealIsa
        } else {
            KisAccountType::RealGeneral
        }
    };

    debug!(
        "KIS 클라이언트 생성: credential={}, account_type={:?}",
        credential_id, account_type
    );

    // KisConfig 생성
    let config = KisConfig::new(
        credentials.api_key.clone(),
        credentials.api_secret.clone(),
        account_number.clone(),
        account_type,
    );

    // OAuth 생성 (토큰 공유)
    let oauth = KisOAuth::new(config.clone())
        .map_err(|e| format!("OAuth 생성 실패: {}", e))?;
    let oauth_arc = Arc::new(oauth);

    // DB에서 유효한 토큰 조회 (rate limit 대응)
    let environment = if is_testnet { "paper" } else { "real" };
    if let Some(cached_token) =
        KisTokenRepository::load_valid_token(pool, credential_id, environment).await
    {
        debug!("DB 캐시된 토큰 사용 (create_kis_client): credential_id={}", credential_id);
        oauth_arc.set_cached_token(cached_token).await;
    } else {
        info!(
            "DB에 유효한 토큰 없음, 새로 발급 (create_kis_client): credential_id={}",
            credential_id
        );
        match oauth_arc.refresh_and_get_token().await {
            Ok(token) => {
                if let Err(e) =
                    KisTokenRepository::save_token(pool, credential_id, environment, &token).await
                {
                    warn!("토큰 DB 저장 실패 (계속 진행): {}", e);
                }
            }
            Err(e) => {
                // Fallback: 완화된 조건으로 DB 재조회 (다른 경로에서 방금 발급했을 수 있음)
                warn!("토큰 발급 실패, fallback 조회 시도: {}", e);
                if let Some(fb_token) =
                    KisTokenRepository::load_any_valid_token(pool, credential_id, environment).await
                {
                    info!("Fallback 토큰 사용: credential_id={}", credential_id);
                    oauth_arc.set_cached_token(fb_token).await;
                } else {
                    return Err(format!("OAuth 토큰 획득 실패: {}", e));
                }
            }
        }
    }

    // 통합 KisClient 생성 (내부적으로 KR/US 클라이언트 자동 생성)
    let client = KisClient::new(oauth_arc)
        .map_err(|e| format!("KIS 클라이언트 생성 실패: {}", e))?;

    Ok(Arc::new(client))
}

/// KIS Provider 생성 (동기화용).
///
/// sync.rs에서 체결 내역 동기화에 사용합니다.
/// 구체적인 KisProvider 타입을 반환하여 sync 전용 메서드에 접근할 수 있습니다.
pub async fn create_kis_provider_for_sync(
    pool: &PgPool,
    encryptor: &CredentialEncryptor,
    credential_id: Uuid,
) -> Result<KisProvider, String> {
    let client = create_kis_client(pool, encryptor, credential_id).await?;
    Ok(KisProvider::new(client))
}
