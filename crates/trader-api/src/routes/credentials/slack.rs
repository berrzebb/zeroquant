//! Slack settings handlers.
//!
//! This module provides handlers for managing Slack Webhook notification settings
//! with AES-256-GCM encryption for sensitive data (webhook_url).

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

use super::types::{
    log_credential_access, mask_api_key, NotificationSettingsConfig, SaveSlackSettingsRequest,
    SlackSettingsRow,
};
use crate::routes::strategies::ApiError;
use crate::state::AppState;

// =============================================================================
// Slack Settings Handlers
// =============================================================================

/// Slack 설정 조회.
///
/// `GET /api/v1/credentials/slack`
#[utoipa::path(
    get,
    path = "/api/v1/credentials/slack",
    tag = "credentials",
    responses(
        (status = 200, description = "Slack 설정 조회 성공"),
        (status = 404, description = "설정 없음"),
        (status = 500, description = "서버 오류")
    )
)]
pub async fn get_slack_settings(
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

    let row: Option<SlackSettingsRow> = sqlx::query_as(
        r#"
        SELECT
            id, encrypted_webhook_url, encryption_nonce_webhook,
            display_name, workspace_name, channel_name,
            is_enabled, notification_settings,
            last_message_at, last_verified_at, created_at, updated_at
        FROM slack_settings
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("Slack 설정 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
        )
    })?;

    match row {
        Some(settings) => {
            let webhook_url_masked = match encryptor.decrypt(
                &settings.encrypted_webhook_url,
                &settings.encryption_nonce_webhook,
            ) {
                Ok(url) => mask_api_key(&url),
                Err(_) => "***복호화 실패***".to_string(),
            };

            let notification_settings: Option<NotificationSettingsConfig> = settings
                .notification_settings
                .and_then(|v| serde_json::from_value(v).ok());

            Ok(Json(serde_json::json!({
                "configured": true,
                "id": settings.id,
                "webhook_url_masked": webhook_url_masked,
                "display_name": settings.display_name,
                "workspace_name": settings.workspace_name,
                "channel_name": settings.channel_name,
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
            "message": "Slack 설정이 없습니다. 설정해주세요."
        }))),
    }
}

/// Slack 설정 저장.
///
/// `POST /api/v1/credentials/slack`
#[utoipa::path(
    post,
    path = "/api/v1/credentials/slack",
    tag = "credentials",
    request_body = SaveSlackSettingsRequest,
    responses(
        (status = 201, description = "Slack 설정 저장 성공"),
        (status = 400, description = "잘못된 Webhook URL"),
        (status = 500, description = "서버 오류")
    )
)]
pub async fn save_slack_settings(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SaveSlackSettingsRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("Slack 설정 저장 요청");

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
    if request.webhook_url.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "Webhook URL은 필수입니다.")),
        ));
    }

    if !request.webhook_url.starts_with("https://hooks.slack.com/services/") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "유효한 Slack Webhook URL이 아닙니다.")),
        ));
    }

    // Webhook URL 암호화
    let (encrypted_webhook_url, nonce_webhook) = encryptor.encrypt(&request.webhook_url).map_err(|e| {
        error!("Webhook URL 암호화 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTION_FAILED", "암호화 실패")),
        )
    })?;

    let notification_settings = request.notification_settings.as_ref()
        .map(|s| serde_json::to_value(s).ok())
        .flatten();

    let settings_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO slack_settings
            (id, encrypted_webhook_url, encryption_nonce_webhook,
             display_name, workspace_name, channel_name,
             is_enabled, notification_settings)
        VALUES ($1, $2, $3, $4, $5, $6, true, $7)
        ON CONFLICT ((1))
        DO UPDATE SET
            encrypted_webhook_url = EXCLUDED.encrypted_webhook_url,
            encryption_nonce_webhook = EXCLUDED.encryption_nonce_webhook,
            display_name = EXCLUDED.display_name,
            workspace_name = EXCLUDED.workspace_name,
            channel_name = EXCLUDED.channel_name,
            notification_settings = EXCLUDED.notification_settings,
            updated_at = NOW()
        "#,
    )
    .bind(settings_id)
    .bind(&encrypted_webhook_url)
    .bind(nonce_webhook.to_vec())
    .bind(&request.display_name)
    .bind(&request.workspace_name)
    .bind(&request.channel_name)
    .bind(&notification_settings)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("Slack 설정 저장 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("저장 실패: {}", e))),
        )
    })?;

    log_credential_access(pool, "slack", settings_id, "create", true, None).await;

    debug!("Slack 설정 저장 완료");

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "message": "Slack 설정이 저장되었습니다.",
            "webhook_url_masked": mask_api_key(&request.webhook_url)
        })),
    ))
}

/// Slack 설정 삭제.
///
/// `DELETE /api/v1/credentials/slack`
#[utoipa::path(
    delete,
    path = "/api/v1/credentials/slack",
    tag = "credentials",
    responses(
        (status = 200, description = "Slack 설정 삭제 성공"),
        (status = 404, description = "설정 없음"),
        (status = 500, description = "서버 오류")
    )
)]
pub async fn delete_slack_settings(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("Slack 설정 삭제 요청");

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM slack_settings LIMIT 1")
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Slack 설정 조회 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
            )
        })?;

    let result = sqlx::query("DELETE FROM slack_settings")
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Slack 설정 삭제 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("삭제 실패: {}", e))),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "삭제할 Slack 설정이 없습니다.")),
        ));
    }

    if let Some((id,)) = row {
        log_credential_access(pool, "slack", id, "delete", true, None).await;
    }

    debug!("Slack 설정 삭제 완료");

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Slack 설정이 삭제되었습니다."
    })))
}

/// Slack 새 설정 테스트 (저장 전).
///
/// `POST /api/v1/credentials/slack/test/new`
///
/// 저장하지 않고 입력된 Webhook URL로 테스트 메시지를 전송합니다.
#[utoipa::path(
    post,
    path = "/api/v1/credentials/slack/test/new",
    tag = "credentials",
    request_body = SaveSlackSettingsRequest,
    responses(
        (status = 200, description = "테스트 메시지 전송 성공"),
        (status = 400, description = "잘못된 Webhook URL"),
        (status = 500, description = "전송 실패")
    )
)]
pub async fn test_new_slack_settings(
    Json(request): Json<SaveSlackSettingsRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("Slack 새 설정 테스트 (저장 전)");

    // 입력 검증
    if request.webhook_url.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "Webhook URL은 필수입니다.")),
        ));
    }

    if !request.webhook_url.starts_with("https://hooks.slack.com/services/") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_INPUT",
                "유효한 Slack Webhook URL이 아닙니다.",
            )),
        ));
    }

    // 실제 Slack 테스트 전송
    let config = trader_notification::SlackConfig::new(request.webhook_url.clone());
    let config = if let Some(name) = &request.display_name {
        config.with_display_name(name.clone())
    } else {
        config
    };

    let sender = trader_notification::SlackSender::new(config);

    match sender.send_test().await {
        Ok(()) => {
            debug!("Slack 새 설정 테스트 성공");
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "테스트 메시지가 Slack에 전송되었습니다."
            })))
        }
        Err(e) => {
            let error_msg = format!("Slack 전송 실패: {}", e);
            error!("{}", error_msg);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("SEND_FAILED", error_msg)),
            ))
        }
    }
}

/// Slack 설정 테스트.
///
/// `POST /api/v1/credentials/slack/test`
#[utoipa::path(
    post,
    path = "/api/v1/credentials/slack/test",
    tag = "credentials",
    responses(
        (status = 200, description = "테스트 메시지 전송 성공"),
        (status = 404, description = "설정 없음"),
        (status = 500, description = "전송 실패")
    )
)]
pub async fn test_slack_settings(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("Slack 설정 테스트");

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

    let row: Option<SlackSettingsRow> = sqlx::query_as(
        r#"
        SELECT
            id, encrypted_webhook_url, encryption_nonce_webhook,
            display_name, workspace_name, channel_name,
            is_enabled, notification_settings,
            last_message_at, last_verified_at, created_at, updated_at
        FROM slack_settings
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("Slack 설정 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
        )
    })?;

    let settings = row.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "Slack 설정이 없습니다.")),
        )
    })?;

    // 복호화
    let webhook_url = encryptor
        .decrypt(
            &settings.encrypted_webhook_url,
            &settings.encryption_nonce_webhook,
        )
        .map_err(|e| {
            error!("Webhook URL 복호화 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DECRYPTION_FAILED", "복호화 실패")),
            )
        })?;

    // 실제 Slack 테스트 전송
    let config = trader_notification::SlackConfig::new(webhook_url);
    let config = if let Some(name) = &settings.display_name {
        config.with_display_name(name.clone())
    } else {
        config
    };

    let sender = trader_notification::SlackSender::new(config);

    match sender.send_test().await {
        Ok(()) => {
            let _ = sqlx::query("UPDATE slack_settings SET last_verified_at = NOW() WHERE id = $1")
                .bind(settings.id)
                .execute(pool)
                .await;

            log_credential_access(pool, "slack", settings.id, "verify", true, None).await;

            Ok(Json(serde_json::json!({
                "success": true,
                "message": "테스트 메시지가 Slack에 전송되었습니다."
            })))
        }
        Err(e) => {
            let error_msg = format!("Slack 전송 실패: {}", e);
            log_credential_access(pool, "slack", settings.id, "verify", false, Some(&error_msg)).await;

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("SEND_FAILED", error_msg)),
            ))
        }
    }
}
