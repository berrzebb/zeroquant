//! 활성 계정 관리 핸들러.
//!
//! 대시보드에 표시될 자산 정보의 기준 계정을 관리합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/credentials/active` - 활성 계정 조회
//! - `PUT /api/v1/credentials/active` - 활성 계정 설정

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::types::{ActiveAccountResponse, SetActiveAccountRequest};
use crate::routes::strategies::ApiError;
use crate::state::AppState;
use crate::websocket::{ActiveAccountChangedData, ServerMessage};

/// 활성 계정 조회.
///
/// 현재 대시보드에 표시될 자산 정보의 기준 계정을 조회합니다.
///
/// `GET /api/v1/credentials/active-account`
#[utoipa::path(
    get,
    path = "/api/v1/credentials/active-account",
    tag = "credentials",
    responses(
        (status = 200, description = "활성 계정 조회 성공", body = ActiveAccountResponse),
        (status = 500, description = "서버 내부 오류", body = ApiError)
    )
)]
pub async fn get_active_account(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    // DB 연결 확인
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    // app_settings 테이블에서 active_credential_id 조회
    let setting: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT setting_value FROM app_settings WHERE setting_key = 'active_credential_id' LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        // 테이블이 없으면 None 반환
        warn!("활성 계정 조회 실패 (테이블 없음?): {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
        )
    })?;

    match setting {
        Some((credential_id_str,)) => {
            // UUID 파싱
            let credential_id = Uuid::parse_str(&credential_id_str).ok();

            if let Some(cred_id) = credential_id {
                // 자격증명 정보 조회
                let row: Option<(String, String, bool)> = sqlx::query_as(
                    r#"
                    SELECT exchange_id, exchange_name, is_testnet
                    FROM exchange_credentials
                    WHERE id = $1
                    "#,
                )
                .bind(cred_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| {
                    error!("자격증명 조회 실패: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
                    )
                })?;

                if let Some((exchange_id, display_name, is_testnet)) = row {
                    return Ok(Json(ActiveAccountResponse {
                        credential_id: Some(cred_id),
                        exchange_id: Some(exchange_id),
                        display_name: Some(display_name),
                        is_testnet,
                    }));
                }
            }

            // 자격증명이 없으면 설정 초기화
            Ok(Json(ActiveAccountResponse {
                credential_id: None,
                exchange_id: None,
                display_name: None,
                is_testnet: false,
            }))
        }
        None => Ok(Json(ActiveAccountResponse {
            credential_id: None,
            exchange_id: None,
            display_name: None,
            is_testnet: false,
        })),
    }
}

/// 활성 계정 설정.
///
/// 대시보드에 표시될 자산 정보의 기준 계정을 설정합니다.
///
/// `POST /api/v1/credentials/active-account`
#[utoipa::path(
    post,
    path = "/api/v1/credentials/active-account",
    tag = "credentials",
    request_body = SetActiveAccountRequest,
    responses(
        (status = 200, description = "활성 계정 설정 성공", body = inline(serde_json::Value)),
        (status = 404, description = "자격증명을 찾을 수 없음", body = ApiError),
        (status = 500, description = "서버 내부 오류", body = ApiError)
    )
)]
pub async fn set_active_account(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SetActiveAccountRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    debug!("활성 계정 설정: {:?}", request.credential_id);

    // DB 연결 확인
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스 연결이 설정되지 않았습니다.",
            )),
        )
    })?;

    // credential_id가 있으면 해당 자격증명이 존재하는지 확인
    if let Some(cred_id) = request.credential_id {
        let row: Option<(Uuid, String)> =
            sqlx::query_as("SELECT id, exchange_id FROM exchange_credentials WHERE id = $1")
                .bind(cred_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| {
                    error!("자격증명 조회 실패: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiError::new("DB_ERROR", format!("조회 실패: {}", e))),
                    )
                })?;

        match row {
            None => {
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(ApiError::new("NOT_FOUND", "자격증명을 찾을 수 없습니다.")),
                ));
            }
            Some((_, exchange_id)) => {
                // 데이터 제공자는 활성 계정으로 설정 불가
                // KRX Open API는 시세 데이터 제공자이므로 거래소가 아님
                if exchange_id == "krx" {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(ApiError::new(
                            "INVALID_EXCHANGE",
                            "KRX Open API는 데이터 제공자입니다. 거래소 계정만 활성화할 수 있습니다.",
                        )),
                    ));
                }
            }
        }
    }

    // app_settings에 저장 (UPSERT)
    let credential_id_str = request
        .credential_id
        .map(|id| id.to_string())
        .unwrap_or_default();

    sqlx::query(
        r#"
        INSERT INTO app_settings (setting_key, setting_value, updated_at)
        VALUES ('active_credential_id', $1, NOW())
        ON CONFLICT (setting_key)
        DO UPDATE SET setting_value = EXCLUDED.setting_value, updated_at = NOW()
        "#,
    )
    .bind(&credential_id_str)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("활성 계정 저장 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("저장 실패: {}", e))),
        )
    })?;

    // 거래소 정보 조회 (WebSocket 브로드캐스트용)
    let (exchange_id, exchange_name) = if let Some(cred_id) = request.credential_id {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT exchange_id, exchange_name FROM exchange_credentials WHERE id = $1",
        )
        .bind(cred_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        row.map(|(eid, ename)| (Some(eid), Some(ename)))
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    // WebSocket으로 활성 계정 변경 브로드캐스트
    if let Some(subscriptions) = &state.subscriptions {
        let ws_message = ServerMessage::ActiveAccountChanged(ActiveAccountChangedData {
            credential_id: request.credential_id.map(|id| id.to_string()),
            exchange_id: exchange_id.clone(),
            exchange_name: exchange_name.clone(),
            timestamp: Utc::now().timestamp_millis(),
        });

        match subscriptions.broadcast(ws_message) {
            Ok(count) => {
                info!(
                    "활성 계정 변경 브로드캐스트 완료: {} 클라이언트에 전송",
                    count
                );
            }
            Err(e) => {
                warn!("활성 계정 변경 브로드캐스트 실패: {:?}", e);
            }
        }
    }

    let message = if request.credential_id.is_some() {
        "활성 계정이 설정되었습니다."
    } else {
        "활성 계정이 해제되었습니다."
    };

    debug!("{}", message);

    Ok(Json(serde_json::json!({
        "success": true,
        "message": message
    })))
}
