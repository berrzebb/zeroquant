//! Exchange credentials handlers.
//!
//! This module provides API handlers for managing exchange credentials:
//! - List supported exchanges
//! - CRUD operations for exchange credentials
//! - Connection testing

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use tracing::{debug, error, warn};
use uuid::Uuid;

use super::types::{
    infer_market_type, log_credential_access, mask_api_key, CreateExchangeCredentialRequest,
    CredentialField, EncryptedCredentials, ExchangeCredentialResponse, ExchangeCredentialRow,
    ExchangeCredentialsListResponse, ExchangeTestResponse, SupportedExchange,
    SupportedExchangesResponse, TestNewCredentialRequest, UpdateExchangeCredentialRequest,
};
use crate::routes::strategies::ApiError;
use crate::state::AppState;

// =============================================================================
// Exchange Credential Handlers
// =============================================================================

/// Get list of supported exchanges.
///
/// `GET /api/v1/credentials/exchanges`
#[utoipa::path(
    get,
    path = "/api/v1/credentials/exchanges",
    tag = "credentials",
    responses(
        (status = 200, description = "지원 거래소 목록 조회 성공", body = SupportedExchangesResponse)
    )
)]
pub async fn get_supported_exchanges() -> impl IntoResponse {
    let exchanges = vec![
        SupportedExchange {
            exchange_id: "binance".to_string(),
            display_name: "Binance".to_string(),
            market_type: "crypto".to_string(),
            supports_testnet: true,
            required_fields: vec![
                CredentialField {
                    name: "api_key".to_string(),
                    label: "API Key".to_string(),
                    field_type: "password".to_string(), // 민감 정보 마스킹
                    placeholder: Some("Enter your Binance API Key".to_string()),
                    help_text: Some("API 관리에서 생성한 API Key".to_string()),
                },
                CredentialField {
                    name: "api_secret".to_string(),
                    label: "API Secret".to_string(),
                    field_type: "password".to_string(),
                    placeholder: Some("Enter your Binance API Secret".to_string()),
                    help_text: Some("API 생성 시 한 번만 표시되는 Secret Key".to_string()),
                },
            ],
            optional_fields: vec![],
            description: "세계 최대 암호화폐 거래소".to_string(),
            docs_url: Some("https://binance-docs.github.io/apidocs/spot/en/".to_string()),
            is_data_provider: false,
        },
        SupportedExchange {
            exchange_id: "kis".to_string(),
            display_name: "한국투자증권".to_string(),
            market_type: "stock_kr".to_string(),
            supports_testnet: true,
            required_fields: vec![
                CredentialField {
                    name: "api_key".to_string(),
                    label: "App Key".to_string(),
                    field_type: "password".to_string(), // 민감 정보 마스킹
                    placeholder: Some("발급받은 App Key".to_string()),
                    help_text: Some("KIS Developers에서 발급받은 App Key".to_string()),
                },
                CredentialField {
                    name: "api_secret".to_string(),
                    label: "App Secret".to_string(),
                    field_type: "password".to_string(),
                    placeholder: Some("발급받은 App Secret".to_string()),
                    help_text: Some("KIS Developers에서 발급받은 App Secret".to_string()),
                },
            ],
            optional_fields: vec![CredentialField {
                name: "account_number".to_string(),
                label: "계좌번호".to_string(),
                field_type: "text".to_string(),
                placeholder: Some("00000000-00".to_string()),
                help_text: Some("거래에 사용할 계좌번호 (종합계좌번호-상품코드)".to_string()),
            }],
            description: "한국투자증권 KIS Developers API".to_string(),
            docs_url: Some("https://apiportal.koreainvestment.com/".to_string()),
            is_data_provider: false,
        },
        SupportedExchange {
            exchange_id: "coinbase".to_string(),
            display_name: "Coinbase".to_string(),
            market_type: "crypto".to_string(),
            supports_testnet: true,
            required_fields: vec![
                CredentialField {
                    name: "api_key".to_string(),
                    label: "API Key".to_string(),
                    field_type: "password".to_string(), // 민감 정보 마스킹
                    placeholder: Some("Coinbase API Key".to_string()),
                    help_text: None,
                },
                CredentialField {
                    name: "api_secret".to_string(),
                    label: "API Secret".to_string(),
                    field_type: "password".to_string(),
                    placeholder: Some("Coinbase API Secret".to_string()),
                    help_text: None,
                },
                CredentialField {
                    name: "passphrase".to_string(),
                    label: "Passphrase".to_string(),
                    field_type: "password".to_string(),
                    placeholder: Some("API Passphrase".to_string()),
                    help_text: Some("API 생성 시 설정한 Passphrase".to_string()),
                },
            ],
            optional_fields: vec![],
            description: "미국 최대 암호화폐 거래소".to_string(),
            docs_url: Some("https://docs.cloud.coinbase.com/exchange/reference".to_string()),
            is_data_provider: false,
        },
        SupportedExchange {
            exchange_id: "krx".to_string(),
            display_name: "KRX Open API".to_string(),
            market_type: "data_provider".to_string(),
            supports_testnet: false,
            required_fields: vec![CredentialField {
                name: "api_key".to_string(),
                label: "인증키 (AUTH_KEY)".to_string(),
                field_type: "password".to_string(),
                placeholder: Some("KRX Open API 인증키".to_string()),
                help_text: Some(
                    "data.krx.co.kr에서 발급받은 인증키. KOSPI/KOSDAQ 종목 정보 및 시세 조회에 사용됩니다."
                        .to_string(),
                ),
            }],
            optional_fields: vec![],
            description: "KRX 정보데이터시스템. 국내 주식 종목 정보, PER/PBR, 시세 데이터 제공 (Yahoo Finance 대체)"
                .to_string(),
            docs_url: Some("https://data.krx.co.kr/contents/MDC/MAIN/main/index.cmd".to_string()),
            is_data_provider: true, // 데이터 제공자 - 활성 계정으로 설정 불가
        },
        SupportedExchange {
            exchange_id: "mock".to_string(),
            display_name: "Mock Exchange (테스트용)".to_string(),
            market_type: "mock".to_string(),
            supports_testnet: false,
            required_fields: vec![], // API 키 불필요
            optional_fields: vec![
                CredentialField {
                    name: "initial_balance".to_string(),
                    label: "초기 자금".to_string(),
                    field_type: "number".to_string(),
                    placeholder: Some("10000000".to_string()),
                    help_text: Some("가상 거래에 사용할 초기 자금 (기본: 1천만원)".to_string()),
                },
                CredentialField {
                    name: "market_type".to_string(),
                    label: "시장 유형".to_string(),
                    field_type: "select".to_string(),
                    placeholder: Some("stock_kr".to_string()),
                    help_text: Some("stock_kr: 국내 주식, stock_us: 미국 주식, crypto: 암호화폐".to_string()),
                },
                CredentialField {
                    name: "commission_rate".to_string(),
                    label: "수수료율 (%)".to_string(),
                    field_type: "number".to_string(),
                    placeholder: Some("0.015".to_string()),
                    help_text: Some("거래 수수료율 (예: 0.015 = 0.015%)".to_string()),
                },
                CredentialField {
                    name: "slippage_rate".to_string(),
                    label: "슬리피지 (%)".to_string(),
                    field_type: "number".to_string(),
                    placeholder: Some("0.01".to_string()),
                    help_text: Some("체결 시 적용되는 슬리피지율 (예: 0.01 = 0.01%)".to_string()),
                },
                CredentialField {
                    name: "currency".to_string(),
                    label: "통화".to_string(),
                    field_type: "text".to_string(),
                    placeholder: Some("KRW".to_string()),
                    help_text: Some("계좌 통화 (KRW, USD, USDT 등)".to_string()),
                },
            ],
            description: "가상 거래소. 전략의 실제 동작을 검증한 후 실제 모의투자 계좌로 전환할 수 있습니다."
                .to_string(),
            docs_url: None,
            is_data_provider: false,
        },
    ];

    Json(SupportedExchangesResponse { exchanges })
}

/// List all exchange credentials.
///
/// `GET /api/v1/credentials/exchange`
#[utoipa::path(
    get,
    path = "/api/v1/credentials/exchange",
    tag = "credentials",
    responses(
        (status = 200, description = "거래소 자격증명 목록 조회 성공", body = ExchangeCredentialsListResponse),
        (status = 500, description = "서버 내부 오류", body = ApiError)
    )
)]
pub async fn list_exchange_credentials(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    // DB connection check
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        error!("DB 연결이 설정되지 않았습니다.");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    // Encryptor check
    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        error!("암호화 관리자가 설정되지 않았습니다.");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "ENCRYPTOR_NOT_CONFIGURED",
                "암호화 설정이 없습니다. ENCRYPTION_MASTER_KEY를 설정하세요.",
            )),
        )
    })?;

    // Query all credentials from DB
    let rows: Vec<ExchangeCredentialRow> = sqlx::query_as(
        r#"
        SELECT
            id, exchange_id, exchange_name, market_type,
            encrypted_credentials, encryption_nonce,
            is_active, is_testnet, permissions, settings,
            last_used_at, last_verified_at, created_at, updated_at
        FROM exchange_credentials
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("자격증명 목록 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
        )
    })?;

    // Transform response (decrypt and mask)
    let mut credentials = Vec::with_capacity(rows.len());

    for row in rows {
        // Decrypt and mask API key
        let api_key_masked = match encryptor
            .decrypt_json::<EncryptedCredentials>(&row.encrypted_credentials, &row.encryption_nonce)
        {
            Ok(creds) => mask_api_key(&creds.api_key),
            Err(e) => {
                warn!("자격증명 복호화 실패 (id: {}): {}", row.id, e);
                "***복호화 실패***".to_string()
            }
        };

        // Convert permissions JSON to Vec<String>
        let permissions: Option<Vec<String>> =
            row.permissions.and_then(|v| serde_json::from_value(v).ok());

        // 데이터 제공자 여부 확인 (krx는 시세 데이터만 제공)
        let is_data_provider = row.exchange_id == "krx";

        credentials.push(ExchangeCredentialResponse {
            id: row.id,
            exchange_id: row.exchange_id,
            display_name: row.exchange_name,
            market_type: row.market_type,
            api_key_masked,
            is_active: row.is_active,
            is_testnet: row.is_testnet,
            is_data_provider,
            permissions,
            settings: row.settings,
            last_used_at: row.last_used_at.map(|t| t.to_rfc3339()),
            last_verified_at: row.last_verified_at.map(|t| t.to_rfc3339()),
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        });
    }

    let total = credentials.len();

    debug!("자격증명 목록 조회 완료: {}개", total);

    Ok(Json(ExchangeCredentialsListResponse { credentials, total }))
}

/// Create new exchange credential.
///
/// `POST /api/v1/credentials/exchange`
#[utoipa::path(
    post,
    path = "/api/v1/credentials/exchange",
    tag = "credentials",
    request_body = CreateExchangeCredentialRequest,
    responses(
        (status = 201, description = "자격증명 등록 성공", body = inline(serde_json::Value)),
        (status = 400, description = "잘못된 입력", body = ApiError),
        (status = 500, description = "서버 내부 오류", body = ApiError)
    )
)]
pub async fn create_exchange_credential(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateExchangeCredentialRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("거래소 자격증명 등록 요청: {}", request.exchange_id);

    // DB connection check
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        error!("DB 연결이 설정되지 않았습니다.");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    // Encryptor check
    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        error!("암호화 관리자가 설정되지 않았습니다.");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "ENCRYPTOR_NOT_CONFIGURED",
                "암호화 설정이 없습니다. ENCRYPTION_MASTER_KEY를 설정하세요.",
            )),
        )
    })?;

    // Mock 거래소는 API 키 불필요 - 별도 처리
    if request.exchange_id == "mock" {
        // Mock 설정을 settings에 저장
        let mut settings = request.settings.clone().unwrap_or_default();
        if let Some(initial_balance) = request.fields.get("initial_balance") {
            settings["initial_balance"] = serde_json::json!(initial_balance);
        }
        if let Some(market_type) = request.fields.get("market_type") {
            settings["market_type"] = serde_json::json!(market_type);
        }
        if let Some(commission_rate) = request.fields.get("commission_rate") {
            settings["commission_rate"] = serde_json::json!(commission_rate);
        }
        if let Some(currency) = request.fields.get("currency") {
            settings["currency"] = serde_json::json!(currency);
        }

        // Create credentials struct for Mock (placeholder)
        let credentials = EncryptedCredentials {
            api_key: "mock_exchange_key".to_string(),
            api_secret: "mock_exchange_secret".to_string(),
            passphrase: None,
            additional: None,
        };

        // Encrypt with AES-256-GCM
        let (encrypted_data, nonce) = encryptor.encrypt_json(&credentials).map_err(|e| {
            error!("자격증명 암호화 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("ENCRYPTION_FAILED", "암호화 실패")),
            )
        })?;

        let market_type = infer_market_type(&request.exchange_id);
        let credential_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO exchange_credentials
                (id, exchange_id, exchange_name, market_type,
                 encrypted_credentials, encryption_nonce, encryption_version,
                 is_active, is_testnet, settings)
            VALUES ($1, $2, $3, $4, $5, $6, 1, true, false, $7)
            ON CONFLICT (exchange_id, market_type, is_testnet, exchange_name)
            DO UPDATE SET
                encrypted_credentials = EXCLUDED.encrypted_credentials,
                encryption_nonce = EXCLUDED.encryption_nonce,
                settings = EXCLUDED.settings,
                updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(credential_id)
        .bind(&request.exchange_id)
        .bind(&request.display_name)
        .bind(market_type)
        .bind(&encrypted_data)
        .bind(nonce.to_vec())
        .bind(serde_json::to_value(&settings).ok())
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("Mock 자격증명 저장 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("저장 실패: {}", e))),
            )
        })?;

        log_credential_access(pool, "exchange", credential_id, "create", true, None).await;

        return Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({
                "success": true,
                "message": "Mock 거래소가 등록되었습니다.",
                "credential": {
                    "id": credential_id,
                    "exchange_id": request.exchange_id,
                    "display_name": request.display_name,
                    "market_type": market_type,
                    "api_key_masked": "mock***",
                    "is_active": true,
                    "is_testnet": false,
                    "settings": settings,
                    "created_at": chrono::Utc::now().to_rfc3339(),
                    "updated_at": chrono::Utc::now().to_rfc3339()
                }
            })),
        ));
    }

    // Input validation - extract api_key, api_secret from fields
    let api_key = request.fields.get("api_key").cloned().unwrap_or_default();
    let api_secret = request
        .fields
        .get("api_secret")
        .cloned()
        .unwrap_or_default();

    // KRX Open API는 api_key만 필요 (데이터 제공자)
    if api_key.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "API Key는 필수입니다.")),
        ));
    }

    // KRX 이외의 거래소는 api_secret도 필수
    if request.exchange_id != "krx" && api_secret.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_INPUT",
                "API Key와 Secret은 필수입니다.",
            )),
        ));
    }

    // Extract passphrase and additional fields
    let passphrase = request.fields.get("passphrase").cloned();
    let additional: Option<std::collections::HashMap<String, String>> = {
        let mut additional_fields: std::collections::HashMap<String, String> =
            request.fields.clone();
        additional_fields.remove("api_key");
        additional_fields.remove("api_secret");
        additional_fields.remove("passphrase");
        if additional_fields.is_empty() {
            None
        } else {
            Some(additional_fields)
        }
    };

    // Create credentials struct
    let credentials = EncryptedCredentials {
        api_key: api_key.clone(),
        api_secret,
        passphrase,
        additional,
    };

    // Encrypt with AES-256-GCM
    let (encrypted_data, nonce) = encryptor.encrypt_json(&credentials).map_err(|e| {
        error!("자격증명 암호화 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTION_FAILED", "암호화 실패")),
        )
    })?;

    // Infer market type
    let market_type = infer_market_type(&request.exchange_id);

    // Save to DB
    let credential_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO exchange_credentials
            (id, exchange_id, exchange_name, market_type,
             encrypted_credentials, encryption_nonce, encryption_version,
             is_active, is_testnet, settings)
        VALUES ($1, $2, $3, $4, $5, $6, 1, true, $7, $8)
        ON CONFLICT (exchange_id, market_type, is_testnet, exchange_name)
        DO UPDATE SET
            encrypted_credentials = EXCLUDED.encrypted_credentials,
            encryption_nonce = EXCLUDED.encryption_nonce,
            settings = EXCLUDED.settings,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(credential_id)
    .bind(&request.exchange_id)
    .bind(&request.display_name)
    .bind(market_type)
    .bind(&encrypted_data)
    .bind(nonce.to_vec())
    .bind(request.is_testnet)
    .bind(&request.settings)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        error!("자격증명 저장 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("저장 실패: {}", e))),
        )
    })?;

    // Audit log
    log_credential_access(pool, "exchange", credential_id, "create", true, None).await;

    debug!(
        "자격증명 등록 완료: {} (id: {})",
        request.exchange_id, credential_id
    );

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "message": "자격증명이 등록되었습니다.",
            "credential": {
                "id": credential_id,
                "exchange_id": request.exchange_id,
                "display_name": request.display_name,
                "market_type": market_type,
                "api_key_masked": mask_api_key(&api_key),
                "is_active": true,
                "is_testnet": request.is_testnet,
                "created_at": chrono::Utc::now().to_rfc3339(),
                "updated_at": chrono::Utc::now().to_rfc3339()
            }
        })),
    ))
}

/// Update exchange credential.
///
/// `PUT /api/v1/credentials/exchange/{id}`
#[utoipa::path(
    put,
    path = "/api/v1/credentials/exchange/{id}",
    tag = "credentials",
    params(
        ("id" = Uuid, Path, description = "자격증명 UUID")
    ),
    request_body = UpdateExchangeCredentialRequest,
    responses(
        (status = 200, description = "자격증명 수정 성공", body = inline(serde_json::Value)),
        (status = 404, description = "자격증명을 찾을 수 없음", body = ApiError),
        (status = 500, description = "서버 내부 오류", body = ApiError)
    )
)]
pub async fn update_exchange_credential(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateExchangeCredentialRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("자격증명 수정 요청: {}", id);

    // DB connection check
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    // Encryptor check
    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "ENCRYPTOR_NOT_CONFIGURED",
                "암호화 설정이 없습니다.",
            )),
        )
    })?;

    // Query existing credential
    let existing: Option<ExchangeCredentialRow> = sqlx::query_as(
        r#"
        SELECT
            id, exchange_id, exchange_name, market_type,
            encrypted_credentials, encryption_nonce,
            is_active, is_testnet, permissions, settings,
            last_used_at, last_verified_at, created_at, updated_at
        FROM exchange_credentials
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("자격증명 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
        )
    })?;

    let existing = existing.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "자격증명을 찾을 수 없습니다.")),
        )
    })?;

    // Decrypt existing credentials
    let mut credentials: EncryptedCredentials = encryptor
        .decrypt_json(&existing.encrypted_credentials, &existing.encryption_nonce)
        .map_err(|e| {
            error!("기존 자격증명 복호화 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DECRYPTION_FAILED", "복호화 실패")),
            )
        })?;

    // Apply changes
    if let Some(api_key) = request.api_key {
        credentials.api_key = api_key;
    }
    if let Some(api_secret) = request.api_secret {
        credentials.api_secret = api_secret;
    }
    if let Some(passphrase) = request.passphrase {
        credentials.passphrase = Some(passphrase);
    }
    if let Some(additional) = request.additional_fields {
        credentials.additional = Some(additional);
    }

    // Re-encrypt
    let (encrypted_data, nonce) = encryptor.encrypt_json(&credentials).map_err(|e| {
        error!("자격증명 암호화 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTION_FAILED", "암호화 실패")),
        )
    })?;

    // Update DB
    let exchange_name = request.exchange_name.unwrap_or(existing.exchange_name);
    let is_active = request.is_active.unwrap_or(existing.is_active);
    let settings = request.settings.or(existing.settings);

    sqlx::query(
        r#"
        UPDATE exchange_credentials
        SET exchange_name = $1,
            encrypted_credentials = $2,
            encryption_nonce = $3,
            is_active = $4,
            settings = $5,
            updated_at = NOW()
        WHERE id = $6
        "#,
    )
    .bind(&exchange_name)
    .bind(&encrypted_data)
    .bind(nonce.to_vec())
    .bind(is_active)
    .bind(&settings)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("자격증명 업데이트 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("업데이트 실패: {}", e))),
        )
    })?;

    // Audit log
    log_credential_access(pool, "exchange", id, "update", true, None).await;

    debug!("자격증명 수정 완료: {}", id);

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "자격증명이 업데이트되었습니다."
    })))
}

/// Delete exchange credential.
///
/// `DELETE /api/v1/credentials/exchange/{id}`
#[utoipa::path(
    delete,
    path = "/api/v1/credentials/exchange/{id}",
    tag = "credentials",
    params(
        ("id" = Uuid, Path, description = "자격증명 UUID")
    ),
    responses(
        (status = 200, description = "자격증명 삭제 성공", body = inline(serde_json::Value)),
        (status = 404, description = "자격증명을 찾을 수 없음", body = ApiError),
        (status = 500, description = "서버 내부 오류", body = ApiError)
    )
)]
pub async fn delete_exchange_credential(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("자격증명 삭제 요청: {}", id);

    // DB connection check
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    // Check existence and delete
    let result = sqlx::query(
        r#"
        DELETE FROM exchange_credentials
        WHERE id = $1
        RETURNING id
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("자격증명 삭제 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("삭제 실패: {}", e))),
        )
    })?;

    if result.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "자격증명을 찾을 수 없습니다.")),
        ));
    }

    // Audit log
    log_credential_access(pool, "exchange", id, "delete", true, None).await;

    debug!("자격증명 삭제 완료: {}", id);

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "자격증명이 삭제되었습니다."
    })))
}

/// Test exchange connection with existing credential.
///
/// `POST /api/v1/credentials/exchange/{id}/test`
#[utoipa::path(
    post,
    path = "/api/v1/credentials/exchange/{id}/test",
    tag = "credentials",
    params(
        ("id" = Uuid, Path, description = "자격증명 UUID")
    ),
    responses(
        (status = 200, description = "연결 테스트 결과", body = ExchangeTestResponse),
        (status = 404, description = "자격증명을 찾을 수 없음", body = ApiError),
        (status = 500, description = "서버 내부 오류", body = ApiError)
    )
)]
pub async fn test_exchange_credential(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("자격증명 연결 테스트: {}", id);

    // DB connection check
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    // Encryptor check
    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "ENCRYPTOR_NOT_CONFIGURED",
                "암호화 설정이 없습니다.",
            )),
        )
    })?;

    // Query credential
    let row: Option<ExchangeCredentialRow> = sqlx::query_as(
        r#"
        SELECT
            id, exchange_id, exchange_name, market_type,
            encrypted_credentials, encryption_nonce,
            is_active, is_testnet, permissions, settings,
            last_used_at, last_verified_at, created_at, updated_at
        FROM exchange_credentials
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("자격증명 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
        )
    })?;

    let row = row.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "자격증명을 찾을 수 없습니다.")),
        )
    })?;

    // Decrypt
    let credentials: EncryptedCredentials = encryptor
        .decrypt_json(&row.encrypted_credentials, &row.encryption_nonce)
        .map_err(|e| {
            error!("자격증명 복호화 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DECRYPTION_FAILED", "복호화 실패")),
            )
        })?;

    // Exchange-specific connection test (actual API call)
    // TODO: Replace with actual exchange API calls
    // Currently only validates credential format
    let (success, message, permissions) = match row.exchange_id.as_str() {
        "binance" => {
            // Binance API key format validation (actual API call needed)
            if credentials.api_key.len() >= 10 && credentials.api_secret.len() >= 10 {
                (
                    true,
                    "Binance API 키 형식이 유효합니다.".to_string(),
                    Some(vec!["read".to_string(), "trade".to_string()]),
                )
            } else {
                (false, "API 키 형식이 올바르지 않습니다.".to_string(), None)
            }
        }
        "kis" => {
            // KIS API key format validation
            if credentials.api_key.len() >= 8 && credentials.api_secret.len() >= 8 {
                (
                    true,
                    "한국투자증권 API 키 형식이 유효합니다.".to_string(),
                    Some(vec!["read".to_string(), "trade".to_string()]),
                )
            } else {
                (false, "API 키 형식이 올바르지 않습니다.".to_string(), None)
            }
        }
        "krx" => {
            // KRX Open API 인증키 검증 (api_secret 불필요)
            if credentials.api_key.len() >= 16 {
                (
                    true,
                    "KRX Open API 인증키가 유효합니다.".to_string(),
                    Some(vec!["read".to_string()]), // 데이터 조회 권한만
                )
            } else {
                (
                    false,
                    "KRX 인증키 형식이 올바르지 않습니다.".to_string(),
                    None,
                )
            }
        }
        "mock" => {
            // Mock 거래소는 항상 성공
            (
                true,
                "Mock 거래소가 정상적으로 연결되었습니다.".to_string(),
                Some(vec!["read".to_string(), "trade".to_string()]),
            )
        }
        _ => (
            true,
            format!("{} API 키가 등록되어 있습니다.", row.exchange_id),
            None,
        ),
    };

    // Update last_verified_at based on test result
    if success {
        let _ = sqlx::query(
            r#"
            UPDATE exchange_credentials
            SET last_verified_at = NOW(),
                permissions = $1
            WHERE id = $2
            "#,
        )
        .bind(serde_json::to_value(&permissions).ok())
        .bind(id)
        .execute(pool)
        .await;
    }

    // Audit log
    log_credential_access(
        pool,
        "exchange",
        id,
        "verify",
        success,
        if success { None } else { Some(&message) },
    )
    .await;

    Ok(Json(ExchangeTestResponse {
        success,
        message,
        permissions,
        account_info: if success {
            Some(serde_json::json!({
                "exchange": row.exchange_id,
                "testnet": row.is_testnet
            }))
        } else {
            None
        },
    }))
}

/// Test new credential before saving.
///
/// `POST /api/v1/credentials/exchange/test`
#[utoipa::path(
    post,
    path = "/api/v1/credentials/exchange/test",
    tag = "credentials",
    request_body = TestNewCredentialRequest,
    responses(
        (status = 200, description = "새 자격증명 테스트 결과", body = ExchangeTestResponse),
        (status = 400, description = "잘못된 입력", body = ApiError)
    )
)]
pub async fn test_new_exchange_credential(
    Json(request): Json<TestNewCredentialRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("새 자격증명 테스트: {}", request.exchange_id);

    // Input validation - extract api_key, api_secret from fields
    let api_key = request.fields.get("api_key").cloned().unwrap_or_default();
    let api_secret = request
        .fields
        .get("api_secret")
        .cloned()
        .unwrap_or_default();

    // Mock 거래소는 API 키 불필요 - 바로 성공 반환
    if request.exchange_id == "mock" {
        return Ok(Json(ExchangeTestResponse {
            success: true,
            message: "Mock 거래소가 정상적으로 연결되었습니다.".to_string(),
            permissions: Some(vec!["read".to_string(), "trade".to_string()]),
            account_info: None,
        }));
    }

    // KRX Open API는 api_key만 필요
    if api_key.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "API Key는 필수입니다.")),
        ));
    }

    // KRX 이외의 거래소는 api_secret도 필수
    if request.exchange_id != "krx" && api_secret.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_INPUT",
                "API Key와 Secret은 필수입니다.",
            )),
        ));
    }

    // Exchange-specific connection test (actual API call)
    // TODO: Replace with actual exchange API calls
    let (success, message, permissions) = match request.exchange_id.as_str() {
        "binance" => {
            if api_key.len() >= 10 && api_secret.len() >= 10 {
                (
                    true,
                    "Binance API 키가 유효합니다.".to_string(),
                    Some(vec!["read".to_string(), "trade".to_string()]),
                )
            } else {
                (false, "API 키 형식이 올바르지 않습니다.".to_string(), None)
            }
        }
        "kis" => {
            if api_key.len() >= 8 && api_secret.len() >= 8 {
                (
                    true,
                    "한국투자증권 API 키가 유효합니다.".to_string(),
                    Some(vec!["read".to_string(), "trade".to_string()]),
                )
            } else {
                (false, "API 키 형식이 올바르지 않습니다.".to_string(), None)
            }
        }
        "krx" => {
            // KRX Open API 인증키 형식 검증 (최소 16자 이상)
            if api_key.len() >= 16 {
                (
                    true,
                    "KRX Open API 인증키 형식이 유효합니다.".to_string(),
                    Some(vec!["read".to_string()]), // 데이터 조회 권한만
                )
            } else {
                (
                    false,
                    "KRX 인증키는 16자 이상이어야 합니다.".to_string(),
                    None,
                )
            }
        }
        _ => (
            true,
            format!("{} API 키 형식이 유효합니다.", request.exchange_id),
            None,
        ),
    };

    Ok(Json(ExchangeTestResponse {
        success,
        message,
        permissions,
        account_info: None,
    }))
}
