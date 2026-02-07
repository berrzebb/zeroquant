//! SMS settings handlers (Twilio).
//!
//! This module provides handlers for managing SMS notification settings
//! with AES-256-GCM encryption for sensitive data (account_sid and auth_token).

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

use super::types::{
    log_credential_access, mask_api_key, NotificationSettingsConfig, SaveSmsSettingsRequest,
    SmsSettingsRow,
};
use crate::routes::strategies::ApiError;
use crate::state::AppState;

// =============================================================================
// SMS Settings Handlers
// =============================================================================

/// SMS 설정 조회.
///
/// `GET /api/v1/credentials/sms`
#[utoipa::path(
    get,
    path = "/api/v1/credentials/sms",
    tag = "credentials",
    responses(
        (status = 200, description = "SMS 설정 조회 성공"),
        (status = 404, description = "설정 없음"),
        (status = 500, description = "서버 오류")
    )
)]
pub async fn get_sms_settings(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "ENCRYPTOR_NOT_CONFIGURED",
                "암호화 설정이 없습니다.",
            )),
        )
    })?;

    let row: Option<SmsSettingsRow> = sqlx::query_as(
        r#"
        SELECT
            id, provider,
            encrypted_account_sid, encryption_nonce_sid,
            encrypted_auth_token, encryption_nonce_token,
            from_number, to_numbers,
            is_enabled, notification_settings,
            last_message_at, last_verified_at, created_at, updated_at
        FROM sms_settings
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("SMS 설정 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
        )
    })?;

    match row {
        Some(settings) => {
            let account_sid_masked = match encryptor.decrypt(
                &settings.encrypted_account_sid,
                &settings.encryption_nonce_sid,
            ) {
                Ok(sid) => mask_api_key(&sid),
                Err(_) => "***복호화 실패***".to_string(),
            };

            let notification_settings: Option<NotificationSettingsConfig> = settings
                .notification_settings
                .and_then(|v| serde_json::from_value(v).ok());

            let to_numbers: Vec<String> = serde_json::from_value(settings.to_numbers)
                .unwrap_or_default();

            Ok(Json(serde_json::json!({
                "configured": true,
                "id": settings.id,
                "provider": settings.provider,
                "account_sid_masked": account_sid_masked,
                "from_number": settings.from_number,
                "to_numbers": to_numbers,
                "is_enabled": settings.is_enabled,
                "notification_settings": notification_settings,
                "last_message_at": settings.last_message_at.map(|t| t.to_rfc3339()),
                "last_verified_at": settings.last_verified_at.map(|t| t.to_rfc3339()),
                "created_at": settings.created_at.to_rfc3339(),
                "updated_at": settings.updated_at.to_rfc3339()
            })))
        }
        None => Ok(Json(serde_json::json!({
            "configured": false,
            "message": "SMS 설정이 없습니다. 설정해주세요."
        }))),
    }
}

/// SMS 설정 저장.
///
/// `POST /api/v1/credentials/sms`
#[utoipa::path(
    post,
    path = "/api/v1/credentials/sms",
    tag = "credentials",
    request_body = SaveSmsSettingsRequest,
    responses(
        (status = 201, description = "SMS 설정 저장 성공"),
        (status = 400, description = "잘못된 입력"),
        (status = 500, description = "서버 오류")
    )
)]
pub async fn save_sms_settings(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SaveSmsSettingsRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("SMS 설정 저장 요청");

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "ENCRYPTOR_NOT_CONFIGURED",
                "암호화 설정이 없습니다.",
            )),
        )
    })?;

    // 입력 검증
    if request.account_sid.is_empty() || request.auth_token.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "Account SID와 Auth Token은 필수입니다.")),
        ));
    }

    if request.from_number.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "발신 전화번호는 필수입니다.")),
        ));
    }

    if request.to_numbers.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "수신 전화번호는 최소 1개 필요합니다.")),
        ));
    }

    // E.164 형식 검증 (간단한 검증)
    if !request.from_number.starts_with('+') {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "전화번호는 E.164 형식이어야 합니다 (예: +15551234567).")),
        ));
    }

    // Account SID 암호화
    let (encrypted_account_sid, nonce_sid) = encryptor.encrypt(&request.account_sid).map_err(|e| {
        error!("Account SID 암호화 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTION_FAILED", "암호화 실패")),
        )
    })?;

    // Auth Token 암호화
    let (encrypted_auth_token, nonce_token) = encryptor.encrypt(&request.auth_token).map_err(|e| {
        error!("Auth Token 암호화 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTION_FAILED", "암호화 실패")),
        )
    })?;

    let notification_settings = request.notification_settings.as_ref()
        .map(|s| serde_json::to_value(s).ok())
        .flatten();

    let to_numbers_json = serde_json::to_value(&request.to_numbers)
        .unwrap_or(serde_json::json!([]));

    let settings_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO sms_settings
            (id, provider,
             encrypted_account_sid, encryption_nonce_sid,
             encrypted_auth_token, encryption_nonce_token,
             from_number, to_numbers,
             is_enabled, notification_settings)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, true, $9)
        ON CONFLICT ((1))
        DO UPDATE SET
            provider = EXCLUDED.provider,
            encrypted_account_sid = EXCLUDED.encrypted_account_sid,
            encryption_nonce_sid = EXCLUDED.encryption_nonce_sid,
            encrypted_auth_token = EXCLUDED.encrypted_auth_token,
            encryption_nonce_token = EXCLUDED.encryption_nonce_token,
            from_number = EXCLUDED.from_number,
            to_numbers = EXCLUDED.to_numbers,
            notification_settings = EXCLUDED.notification_settings,
            updated_at = NOW()
        "#,
    )
    .bind(settings_id)
    .bind(&request.provider)
    .bind(&encrypted_account_sid)
    .bind(nonce_sid.to_vec())
    .bind(&encrypted_auth_token)
    .bind(nonce_token.to_vec())
    .bind(&request.from_number)
    .bind(&to_numbers_json)
    .bind(&notification_settings)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("SMS 설정 저장 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("저장 실패: {}", e))),
        )
    })?;

    log_credential_access(pool, "sms", settings_id, "create", true, None).await;

    debug!("SMS 설정 저장 완료");

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "message": "SMS 설정이 저장되었습니다.",
            "provider": request.provider,
            "account_sid_masked": mask_api_key(&request.account_sid)
        })),
    ))
}

/// SMS 설정 삭제.
///
/// `DELETE /api/v1/credentials/sms`
#[utoipa::path(
    delete,
    path = "/api/v1/credentials/sms",
    tag = "credentials",
    responses(
        (status = 200, description = "SMS 설정 삭제 성공"),
        (status = 404, description = "설정 없음"),
        (status = 500, description = "서버 오류")
    )
)]
pub async fn delete_sms_settings(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("SMS 설정 삭제 요청");

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM sms_settings LIMIT 1")
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("SMS 설정 조회 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
            )
        })?;

    let result = sqlx::query("DELETE FROM sms_settings")
        .execute(pool)
        .await
        .map_err(|e| {
            error!("SMS 설정 삭제 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("삭제 실패: {}", e))),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "삭제할 SMS 설정이 없습니다.")),
        ));
    }

    if let Some((id,)) = row {
        log_credential_access(pool, "sms", id, "delete", true, None).await;
    }

    debug!("SMS 설정 삭제 완료");

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "SMS 설정이 삭제되었습니다."
    })))
}

/// SMS 새 설정 테스트 (저장 전).
///
/// `POST /api/v1/credentials/sms/test/new`
///
/// 저장하지 않고 입력된 Twilio 설정으로 테스트 SMS를 전송합니다.
#[utoipa::path(
    post,
    path = "/api/v1/credentials/sms/test/new",
    tag = "credentials",
    request_body = SaveSmsSettingsRequest,
    responses(
        (status = 200, description = "테스트 SMS 전송 성공"),
        (status = 400, description = "잘못된 입력"),
        (status = 500, description = "전송 실패")
    )
)]
pub async fn test_new_sms_settings(
    Json(request): Json<SaveSmsSettingsRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("SMS 새 설정 테스트 (저장 전)");

    // 입력 검증
    if request.account_sid.is_empty() || request.auth_token.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_INPUT",
                "Account SID와 Auth Token은 필수입니다.",
            )),
        ));
    }

    if request.from_number.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "발신 전화번호는 필수입니다.")),
        ));
    }

    if request.to_numbers.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_INPUT",
                "수신 전화번호는 최소 1개 필요합니다.",
            )),
        ));
    }

    // E.164 형식 검증
    if !request.from_number.starts_with('+') {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_INPUT",
                "전화번호는 E.164 형식이어야 합니다 (예: +15551234567).",
            )),
        ));
    }

    // 실제 SMS 테스트 전송
    let config = trader_notification::SmsConfig::new_twilio(
        request.account_sid.clone(),
        request.auth_token.clone(),
        request.from_number.clone(),
        request.to_numbers.clone(),
    );

    let sender = trader_notification::SmsSender::new(config);

    match sender.send_test().await {
        Ok(()) => {
            debug!("SMS 새 설정 테스트 성공");
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "테스트 SMS가 전송되었습니다."
            })))
        }
        Err(e) => {
            let error_msg = format!("SMS 전송 실패: {}", e);
            error!("{}", error_msg);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("SEND_FAILED", error_msg)),
            ))
        }
    }
}

/// SMS 설정 테스트.
///
/// `POST /api/v1/credentials/sms/test`
#[utoipa::path(
    post,
    path = "/api/v1/credentials/sms/test",
    tag = "credentials",
    responses(
        (status = 200, description = "테스트 메시지 전송 성공"),
        (status = 404, description = "설정 없음"),
        (status = 500, description = "전송 실패")
    )
)]
pub async fn test_sms_settings(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("SMS 설정 테스트");

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "ENCRYPTOR_NOT_CONFIGURED",
                "암호화 설정이 없습니다.",
            )),
        )
    })?;

    let row: Option<SmsSettingsRow> = sqlx::query_as(
        r#"
        SELECT
            id, provider,
            encrypted_account_sid, encryption_nonce_sid,
            encrypted_auth_token, encryption_nonce_token,
            from_number, to_numbers,
            is_enabled, notification_settings,
            last_message_at, last_verified_at, created_at, updated_at
        FROM sms_settings
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("SMS 설정 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
        )
    })?;

    let settings = row.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "SMS 설정이 없습니다.")),
        )
    })?;

    // 복호화
    let account_sid = encryptor
        .decrypt(
            &settings.encrypted_account_sid,
            &settings.encryption_nonce_sid,
        )
        .map_err(|e| {
            error!("Account SID 복호화 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DECRYPTION_FAILED", "복호화 실패")),
            )
        })?;

    let auth_token = encryptor
        .decrypt(
            &settings.encrypted_auth_token,
            &settings.encryption_nonce_token,
        )
        .map_err(|e| {
            error!("Auth Token 복호화 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DECRYPTION_FAILED", "복호화 실패")),
            )
        })?;

    let to_numbers: Vec<String> = serde_json::from_value(settings.to_numbers.clone())
        .unwrap_or_default();

    // 실제 SMS 테스트 전송
    let config = trader_notification::SmsConfig::new_twilio(
        account_sid,
        auth_token,
        settings.from_number.clone(),
        to_numbers,
    );

    let sender = trader_notification::SmsSender::new(config);

    match sender.send_test().await {
        Ok(()) => {
            let _ = sqlx::query("UPDATE sms_settings SET last_verified_at = NOW() WHERE id = $1")
                .bind(settings.id)
                .execute(pool)
                .await;

            log_credential_access(pool, "sms", settings.id, "verify", true, None).await;

            Ok(Json(serde_json::json!({
                "success": true,
                "message": "테스트 SMS가 전송되었습니다."
            })))
        }
        Err(e) => {
            let error_msg = format!("SMS 전송 실패: {}", e);
            log_credential_access(pool, "sms", settings.id, "verify", false, Some(&error_msg)).await;

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("SEND_FAILED", error_msg)),
            ))
        }
    }
}
