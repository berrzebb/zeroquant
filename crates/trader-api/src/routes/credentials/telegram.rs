//! Telegram settings handlers.
//!
//! This module provides handlers for managing Telegram notification settings
//! with AES-256-GCM encryption for sensitive data (bot_token and chat_id).

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

use super::types::{
    log_credential_access, mask_api_key, SaveTelegramSettingsRequest, TelegramNotificationSettings,
    TelegramSettingsRow,
};
use crate::routes::strategies::ApiError;
use crate::state::AppState;

// =============================================================================
// Telegram Settings Handlers
// =============================================================================

/// Get telegram settings.
///
/// `GET /api/v1/credentials/telegram`
///
/// Retrieves the current Telegram settings with masked sensitive data.
/// Bot token and chat ID are decrypted and then masked for display.
#[utoipa::path(
    get,
    path = "/api/v1/credentials/telegram",
    tag = "credentials",
    responses(
        (status = 200, description = "텔레그램 설정 조회 성공", body = inline(serde_json::Value)),
        (status = 500, description = "서버 내부 오류", body = ApiError)
    )
)]
pub async fn get_telegram_settings(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    // Check DB connection
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    // Check encryptor
    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "ENCRYPTOR_NOT_CONFIGURED",
                "암호화 설정이 없습니다.",
            )),
        )
    })?;

    // Query telegram settings from DB
    let row: Option<TelegramSettingsRow> = sqlx::query_as(
        r#"
        SELECT
            id, encrypted_bot_token, encryption_nonce_token,
            encrypted_chat_id, encryption_nonce_chat, encryption_version,
            is_enabled, notification_settings, bot_username, chat_type,
            last_message_at, last_verified_at, created_at, updated_at
        FROM telegram_settings
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("텔레그램 설정 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
        )
    })?;

    match row {
        Some(settings) => {
            // Decrypt and mask bot token
            let bot_token_masked = match encryptor.decrypt(
                &settings.encrypted_bot_token,
                &settings.encryption_nonce_token,
            ) {
                Ok(token) => mask_api_key(&token),
                Err(_) => "***복호화 실패***".to_string(),
            };

            // Decrypt and mask chat ID
            let chat_id_masked = match encryptor
                .decrypt(&settings.encrypted_chat_id, &settings.encryption_nonce_chat)
            {
                Ok(chat_id) => mask_api_key(&chat_id),
                Err(_) => "***복호화 실패***".to_string(),
            };

            let notification_settings: TelegramNotificationSettings = settings
                .notification_settings
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();

            Ok(Json(serde_json::json!({
                "configured": true,
                "id": settings.id,
                "display_name": settings.bot_username.clone().unwrap_or_else(|| "Telegram".to_string()),
                "masked_token": bot_token_masked,
                "masked_chat_id": chat_id_masked,
                "is_enabled": settings.is_enabled,
                "notification_settings": notification_settings,
                "bot_username": settings.bot_username,
                "chat_type": settings.chat_type,
                "last_message_at": settings.last_message_at.map(|t| t.to_rfc3339()),
                "last_tested_at": settings.last_verified_at.map(|t| t.to_rfc3339()),
                "created_at": settings.created_at.to_rfc3339(),
                "updated_at": settings.updated_at.to_rfc3339()
            })))
        }
        None => Ok(Json(serde_json::json!({
            "configured": false,
            "message": "텔레그램 설정이 없습니다. 설정해주세요."
        }))),
    }
}

/// Save telegram settings.
///
/// `POST /api/v1/credentials/telegram`
///
/// Encrypts bot_token and chat_id using AES-256-GCM before storing in database.
/// Uses upsert pattern - inserts new settings or updates existing ones.
#[utoipa::path(
    post,
    path = "/api/v1/credentials/telegram",
    tag = "credentials",
    request_body = SaveTelegramSettingsRequest,
    responses(
        (status = 201, description = "텔레그램 설정 저장 성공", body = inline(serde_json::Value)),
        (status = 400, description = "잘못된 입력", body = ApiError),
        (status = 500, description = "서버 내부 오류", body = ApiError)
    )
)]
pub async fn save_telegram_settings(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SaveTelegramSettingsRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!("텔레그램 설정 저장 요청");

    // Check DB connection
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    // Check encryptor
    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "ENCRYPTOR_NOT_CONFIGURED",
                "암호화 설정이 없습니다.",
            )),
        )
    })?;

    // Input validation
    if request.bot_token.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "Bot Token은 필수입니다.")),
        ));
    }

    if request.chat_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "Chat ID는 필수입니다.")),
        ));
    }

    // Encrypt bot token
    let (encrypted_bot_token, nonce_token) =
        encryptor.encrypt(&request.bot_token).map_err(|e| {
            error!("Bot Token 암호화 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("ENCRYPTION_FAILED", "암호화 실패")),
            )
        })?;

    // Encrypt chat ID
    let (encrypted_chat_id, nonce_chat) = encryptor.encrypt(&request.chat_id).map_err(|e| {
        error!("Chat ID 암호화 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTION_FAILED", "암호화 실패")),
        )
    })?;

    let notification_settings = request.notification_settings.unwrap_or_default();
    let notification_settings_json = serde_json::to_value(&notification_settings).ok();

    // Upsert: insert if not exists, update if exists
    let settings_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO telegram_settings
            (id, encrypted_bot_token, encryption_nonce_token,
             encrypted_chat_id, encryption_nonce_chat, encryption_version,
             is_enabled, notification_settings)
        VALUES ($1, $2, $3, $4, $5, 1, true, $6)
        ON CONFLICT ((1))
        DO UPDATE SET
            encrypted_bot_token = EXCLUDED.encrypted_bot_token,
            encryption_nonce_token = EXCLUDED.encryption_nonce_token,
            encrypted_chat_id = EXCLUDED.encrypted_chat_id,
            encryption_nonce_chat = EXCLUDED.encryption_nonce_chat,
            notification_settings = EXCLUDED.notification_settings,
            updated_at = NOW()
        "#,
    )
    .bind(settings_id)
    .bind(&encrypted_bot_token)
    .bind(nonce_token.to_vec())
    .bind(&encrypted_chat_id)
    .bind(nonce_chat.to_vec())
    .bind(&notification_settings_json)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("텔레그램 설정 저장 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("저장 실패: {}", e))),
        )
    })?;

    // Audit log
    log_credential_access(pool, "telegram", settings_id, "create", true, None).await;

    info!("텔레그램 설정 저장 완료");

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "message": "텔레그램 설정이 저장되었습니다.",
            "bot_token_masked": mask_api_key(&request.bot_token),
            "chat_id_masked": mask_api_key(&request.chat_id)
        })),
    ))
}

/// Delete telegram settings.
///
/// `DELETE /api/v1/credentials/telegram`
///
/// Removes all telegram settings from the database.
#[utoipa::path(
    delete,
    path = "/api/v1/credentials/telegram",
    tag = "credentials",
    responses(
        (status = 200, description = "텔레그램 설정 삭제 성공", body = inline(serde_json::Value)),
        (status = 404, description = "텔레그램 설정 없음", body = ApiError),
        (status = 500, description = "서버 내부 오류", body = ApiError)
    )
)]
pub async fn delete_telegram_settings(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!("텔레그램 설정 삭제 요청");

    // Check DB connection
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    // Get ID before deletion for audit log
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM telegram_settings LIMIT 1")
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("텔레그램 설정 조회 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
            )
        })?;

    // Execute deletion
    let result = sqlx::query("DELETE FROM telegram_settings")
        .execute(pool)
        .await
        .map_err(|e| {
            error!("텔레그램 설정 삭제 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("삭제 실패: {}", e))),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new(
                "NOT_FOUND",
                "삭제할 텔레그램 설정이 없습니다.",
            )),
        ));
    }

    // Audit log
    if let Some((id,)) = row {
        log_credential_access(pool, "telegram", id, "delete", true, None).await;
    }

    info!("텔레그램 설정 삭제 완료");

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "텔레그램 설정이 삭제되었습니다."
    })))
}

/// Test telegram settings.
///
/// `POST /api/v1/credentials/telegram/test`
///
/// Validates the stored telegram settings by decrypting and checking format.
/// Updates last_verified_at timestamp on success.
#[utoipa::path(
    post,
    path = "/api/v1/credentials/telegram/test",
    tag = "credentials",
    responses(
        (status = 200, description = "텔레그램 설정 테스트 결과", body = inline(serde_json::Value)),
        (status = 404, description = "텔레그램 설정 없음", body = ApiError),
        (status = 500, description = "서버 내부 오류", body = ApiError)
    )
)]
pub async fn test_telegram_settings(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!("텔레그램 설정 테스트");

    // Check DB connection
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    // Check encryptor
    let encryptor = state.encryptor.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "ENCRYPTOR_NOT_CONFIGURED",
                "암호화 설정이 없습니다.",
            )),
        )
    })?;

    // Query settings
    let row: Option<TelegramSettingsRow> = sqlx::query_as(
        r#"
        SELECT
            id, encrypted_bot_token, encryption_nonce_token,
            encrypted_chat_id, encryption_nonce_chat, encryption_version,
            is_enabled, notification_settings, bot_username, chat_type,
            last_message_at, last_verified_at, created_at, updated_at
        FROM telegram_settings
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("텔레그램 설정 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
        )
    })?;

    let settings = row.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "텔레그램 설정이 없습니다.")),
        )
    })?;

    // Decrypt bot token
    let bot_token = encryptor
        .decrypt(
            &settings.encrypted_bot_token,
            &settings.encryption_nonce_token,
        )
        .map_err(|e| {
            error!("Bot Token 복호화 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DECRYPTION_FAILED", "복호화 실패")),
            )
        })?;

    // Decrypt chat ID
    let chat_id = encryptor
        .decrypt(&settings.encrypted_chat_id, &settings.encryption_nonce_chat)
        .map_err(|e| {
            error!("Chat ID 복호화 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DECRYPTION_FAILED", "복호화 실패")),
            )
        })?;

    // TODO: Send actual test message via Telegram API
    // Currently only performing format validation
    let success = bot_token.len() > 10 && !chat_id.is_empty();
    let message = if success {
        "텔레그램 설정이 유효합니다."
    } else {
        "텔레그램 설정이 올바르지 않습니다."
    };

    // Update last_verified_at on success
    if success {
        let _ = sqlx::query("UPDATE telegram_settings SET last_verified_at = NOW() WHERE id = $1")
            .bind(settings.id)
            .execute(pool)
            .await;
    }

    // Audit log
    log_credential_access(
        pool,
        "telegram",
        settings.id,
        "verify",
        success,
        if success { None } else { Some(message) },
    )
    .await;

    Ok(Json(serde_json::json!({
        "success": success,
        "message": message
    })))
}
