//! Email settings handlers.
//!
//! This module provides handlers for managing Email notification settings
//! with AES-256-GCM encryption for sensitive data (username and password).

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

use super::types::{
    log_credential_access, mask_api_key, EmailSettingsRow, NotificationSettingsConfig,
    SaveEmailSettingsRequest,
};
use crate::routes::strategies::ApiError;
use crate::state::AppState;

// =============================================================================
// Email Settings Handlers
// =============================================================================

/// Email 설정 조회.
///
/// `GET /api/v1/credentials/email`
#[utoipa::path(
    get,
    path = "/api/v1/credentials/email",
    tag = "credentials",
    responses(
        (status = 200, description = "Email 설정 조회 성공"),
        (status = 404, description = "설정 없음"),
        (status = 500, description = "서버 오류")
    )
)]
pub async fn get_email_settings(
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

    let row: Option<EmailSettingsRow> = sqlx::query_as(
        r#"
        SELECT
            id, smtp_host, smtp_port, use_tls,
            encrypted_username, encryption_nonce_username,
            encrypted_password, encryption_nonce_password,
            from_email, from_name, to_emails,
            is_enabled, notification_settings,
            last_message_at, last_verified_at, created_at, updated_at
        FROM email_settings
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("이메일 설정 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
        )
    })?;

    match row {
        Some(settings) => {
            // 사용자명 복호화 및 마스킹
            let username_masked = match encryptor.decrypt(
                &settings.encrypted_username,
                &settings.encryption_nonce_username,
            ) {
                Ok(username) => mask_api_key(&username),
                Err(_) => "***복호화 실패***".to_string(),
            };

            let notification_settings: Option<NotificationSettingsConfig> = settings
                .notification_settings
                .and_then(|v| serde_json::from_value(v).ok());

            let to_emails: Vec<String> = serde_json::from_value(settings.to_emails)
                .unwrap_or_default();

            Ok(Json(serde_json::json!({
                "configured": true,
                "id": settings.id,
                "smtp_host": settings.smtp_host,
                "smtp_port": settings.smtp_port,
                "use_tls": settings.use_tls,
                "username_masked": username_masked,
                "from_email": settings.from_email,
                "from_name": settings.from_name,
                "to_emails": to_emails,
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
            "message": "이메일 설정이 없습니다. 설정해주세요."
        }))),
    }
}

/// Email 설정 저장.
///
/// `POST /api/v1/credentials/email`
#[utoipa::path(
    post,
    path = "/api/v1/credentials/email",
    tag = "credentials",
    request_body = SaveEmailSettingsRequest,
    responses(
        (status = 201, description = "Email 설정 저장 성공"),
        (status = 400, description = "잘못된 입력"),
        (status = 500, description = "서버 오류")
    )
)]
pub async fn save_email_settings(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SaveEmailSettingsRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("이메일 설정 저장 요청");

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
    if request.smtp_host.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "SMTP 호스트는 필수입니다.")),
        ));
    }

    if request.username.is_empty() || request.password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "사용자명과 비밀번호는 필수입니다.")),
        ));
    }

    if request.to_emails.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "수신자 이메일은 최소 1개 필요합니다.")),
        ));
    }

    // 사용자명 암호화
    let (encrypted_username, nonce_username) = encryptor.encrypt(&request.username).map_err(|e| {
        error!("사용자명 암호화 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTION_FAILED", "암호화 실패")),
        )
    })?;

    // 비밀번호 암호화
    let (encrypted_password, nonce_password) = encryptor.encrypt(&request.password).map_err(|e| {
        error!("비밀번호 암호화 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("ENCRYPTION_FAILED", "암호화 실패")),
        )
    })?;

    let notification_settings = request.notification_settings.as_ref()
        .and_then(|s| serde_json::to_value(s).ok());

    let to_emails_json = serde_json::to_value(&request.to_emails)
        .unwrap_or(serde_json::json!([]));

    let settings_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO email_settings
            (id, smtp_host, smtp_port, use_tls,
             encrypted_username, encryption_nonce_username,
             encrypted_password, encryption_nonce_password,
             from_email, from_name, to_emails,
             is_enabled, notification_settings)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, true, $12)
        ON CONFLICT ((1))
        DO UPDATE SET
            smtp_host = EXCLUDED.smtp_host,
            smtp_port = EXCLUDED.smtp_port,
            use_tls = EXCLUDED.use_tls,
            encrypted_username = EXCLUDED.encrypted_username,
            encryption_nonce_username = EXCLUDED.encryption_nonce_username,
            encrypted_password = EXCLUDED.encrypted_password,
            encryption_nonce_password = EXCLUDED.encryption_nonce_password,
            from_email = EXCLUDED.from_email,
            from_name = EXCLUDED.from_name,
            to_emails = EXCLUDED.to_emails,
            notification_settings = EXCLUDED.notification_settings,
            updated_at = NOW()
        "#,
    )
    .bind(settings_id)
    .bind(&request.smtp_host)
    .bind(request.smtp_port as i32)
    .bind(request.use_tls)
    .bind(&encrypted_username)
    .bind(nonce_username.to_vec())
    .bind(&encrypted_password)
    .bind(nonce_password.to_vec())
    .bind(&request.from_email)
    .bind(&request.from_name)
    .bind(&to_emails_json)
    .bind(&notification_settings)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("이메일 설정 저장 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("저장 실패: {}", e))),
        )
    })?;

    log_credential_access(pool, "email", settings_id, "create", true, None).await;

    debug!("이메일 설정 저장 완료");

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "message": "이메일 설정이 저장되었습니다.",
            "smtp_host": request.smtp_host,
            "username_masked": mask_api_key(&request.username)
        })),
    ))
}

/// Email 설정 삭제.
///
/// `DELETE /api/v1/credentials/email`
#[utoipa::path(
    delete,
    path = "/api/v1/credentials/email",
    tag = "credentials",
    responses(
        (status = 200, description = "Email 설정 삭제 성공"),
        (status = 404, description = "설정 없음"),
        (status = 500, description = "서버 오류")
    )
)]
pub async fn delete_email_settings(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("이메일 설정 삭제 요청");

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM email_settings LIMIT 1")
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("이메일 설정 조회 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
            )
        })?;

    let result = sqlx::query("DELETE FROM email_settings")
        .execute(pool)
        .await
        .map_err(|e| {
            error!("이메일 설정 삭제 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("삭제 실패: {}", e))),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "삭제할 이메일 설정이 없습니다.")),
        ));
    }

    if let Some((id,)) = row {
        log_credential_access(pool, "email", id, "delete", true, None).await;
    }

    debug!("이메일 설정 삭제 완료");

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "이메일 설정이 삭제되었습니다."
    })))
}

/// Email 새 설정 테스트 (저장 전).
///
/// `POST /api/v1/credentials/email/test/new`
///
/// 저장하지 않고 입력된 SMTP 설정으로 테스트 이메일을 전송합니다.
#[utoipa::path(
    post,
    path = "/api/v1/credentials/email/test/new",
    tag = "credentials",
    request_body = SaveEmailSettingsRequest,
    responses(
        (status = 200, description = "테스트 이메일 전송 성공"),
        (status = 400, description = "잘못된 입력"),
        (status = 500, description = "전송 실패")
    )
)]
pub async fn test_new_email_settings(
    Json(request): Json<SaveEmailSettingsRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("Email 새 설정 테스트 (저장 전)");

    // 입력 검증
    if request.smtp_host.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_INPUT", "SMTP 호스트는 필수입니다.")),
        ));
    }

    if request.username.is_empty() || request.password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_INPUT",
                "사용자명과 비밀번호는 필수입니다.",
            )),
        ));
    }

    if request.to_emails.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_INPUT",
                "수신자 이메일은 최소 1개 필요합니다.",
            )),
        ));
    }

    // 실제 이메일 테스트 전송
    let config = trader_notification::EmailConfig::new(
        request.smtp_host.clone(),
        request.smtp_port,
        request.username.clone(),
        request.password.clone(),
        request.from_email.clone(),
        request.to_emails.clone(),
    );
    let config = if let Some(name) = &request.from_name {
        config.with_from_name(name.clone())
    } else {
        config
    };

    let sender = trader_notification::EmailSender::new(config);

    match sender.send_test().await {
        Ok(()) => {
            debug!("Email 새 설정 테스트 성공");
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "테스트 이메일이 전송되었습니다."
            })))
        }
        Err(e) => {
            let error_msg = format!("이메일 전송 실패: {}", e);
            error!("{}", error_msg);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("SEND_FAILED", error_msg)),
            ))
        }
    }
}

/// Email 설정 테스트.
///
/// `POST /api/v1/credentials/email/test`
#[utoipa::path(
    post,
    path = "/api/v1/credentials/email/test",
    tag = "credentials",
    responses(
        (status = 200, description = "테스트 메시지 전송 성공"),
        (status = 404, description = "설정 없음"),
        (status = 500, description = "전송 실패")
    )
)]
pub async fn test_email_settings(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("이메일 설정 테스트");

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

    let row: Option<EmailSettingsRow> = sqlx::query_as(
        r#"
        SELECT
            id, smtp_host, smtp_port, use_tls,
            encrypted_username, encryption_nonce_username,
            encrypted_password, encryption_nonce_password,
            from_email, from_name, to_emails,
            is_enabled, notification_settings,
            last_message_at, last_verified_at, created_at, updated_at
        FROM email_settings
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("이메일 설정 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
        )
    })?;

    let settings = row.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "이메일 설정이 없습니다.")),
        )
    })?;

    // 복호화
    let username = encryptor
        .decrypt(
            &settings.encrypted_username,
            &settings.encryption_nonce_username,
        )
        .map_err(|e| {
            error!("사용자명 복호화 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DECRYPTION_FAILED", "복호화 실패")),
            )
        })?;

    let password = encryptor
        .decrypt(
            &settings.encrypted_password,
            &settings.encryption_nonce_password,
        )
        .map_err(|e| {
            error!("비밀번호 복호화 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DECRYPTION_FAILED", "복호화 실패")),
            )
        })?;

    let to_emails: Vec<String> = serde_json::from_value(settings.to_emails.clone())
        .unwrap_or_default();

    // 실제 이메일 테스트 전송
    let config = trader_notification::EmailConfig::new(
        settings.smtp_host.clone(),
        settings.smtp_port as u16,
        username,
        password,
        settings.from_email.clone(),
        to_emails,
    );
    let config = if let Some(name) = &settings.from_name {
        config.with_from_name(name.clone())
    } else {
        config
    };

    let sender = trader_notification::EmailSender::new(config);

    match sender.send_test().await {
        Ok(()) => {
            // 검증 시간 업데이트
            let _ = sqlx::query("UPDATE email_settings SET last_verified_at = NOW() WHERE id = $1")
                .bind(settings.id)
                .execute(pool)
                .await;

            log_credential_access(pool, "email", settings.id, "verify", true, None).await;

            Ok(Json(serde_json::json!({
                "success": true,
                "message": "테스트 이메일이 전송되었습니다."
            })))
        }
        Err(e) => {
            let error_msg = format!("이메일 전송 실패: {}", e);
            log_credential_access(pool, "email", settings.id, "verify", false, Some(&error_msg)).await;

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("SEND_FAILED", error_msg)),
            ))
        }
    }
}
