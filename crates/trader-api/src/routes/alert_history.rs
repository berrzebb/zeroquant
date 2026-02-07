//! 알림 히스토리 API 라우트
//!
//! 알림 히스토리 조회 및 관리 기능을 제공합니다.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, warn};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::repository::{AlertFilter, AlertHistory, AlertsRepository};
use crate::routes::strategies::ApiError;
use crate::state::AppState;

// ==================== 타입 정의 ====================

/// 알림 히스토리 조회 쿼리
#[derive(Debug, Deserialize, IntoParams)]
pub struct AlertHistoryQuery {
    /// 조회 개수 제한 (기본 20)
    #[serde(default = "default_limit")]
    pub limit: i32,
    /// 시작 오프셋 (기본 0)
    #[serde(default)]
    pub offset: i32,
    /// 상태 필터 (PENDING/SENT/ACKNOWLEDGED/FAILED)
    pub status: Option<String>,
    /// 알림 타입 필터
    pub alert_type: Option<String>,
    /// 채널 필터
    pub channel: Option<String>,
}

fn default_limit() -> i32 {
    20
}

/// 프론트엔드용 알림 히스토리 응답
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAlertHistoryResponse {
    /// 알림 목록
    pub alerts: Vec<FrontendAlertHistoryItem>,
    /// 총 개수
    pub total: i64,
    /// 읽지 않은 알림 수
    pub unread_count: i64,
}

/// 프론트엔드용 알림 아이템
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAlertHistoryItem {
    pub id: String,
    pub rule_id: Option<String>,
    pub signal_marker_id: Option<String>,
    pub alert_type: String,
    pub channel: String,
    pub symbol: Option<String>,
    pub strategy_id: Option<String>,
    pub message: String,
    pub status: String,
    pub sent_at: String,
    pub read_at: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

impl From<AlertHistory> for FrontendAlertHistoryItem {
    fn from(alert: AlertHistory) -> Self {
        // metadata에서 symbol과 strategy_id 추출
        let symbol = alert.metadata.get("symbol").and_then(|v| v.as_str().map(String::from));
        let strategy_id = alert.metadata.get("strategy_id").and_then(|v| v.as_str().map(String::from));

        Self {
            id: alert.id.to_string(),
            rule_id: alert.rule_id.map(|id| id.to_string()),
            signal_marker_id: alert.signal_marker_id.map(|id| id.to_string()),
            alert_type: alert.alert_type,
            channel: alert.channel,
            symbol,
            strategy_id,
            message: alert.message,
            status: alert.status,
            sent_at: alert.sent_at.map(|t| t.to_rfc3339()).unwrap_or_else(|| alert.created_at.to_rfc3339()),
            read_at: alert.acknowledged_at.map(|t| t.to_rfc3339()),
            metadata: Some(alert.metadata),
        }
    }
}

// ==================== 핸들러 ====================

/// 알림 히스토리 목록 조회
///
/// 최근 알림 히스토리를 조회합니다.
#[utoipa::path(
    get,
    path = "/api/v1/alerts/history",
    params(AlertHistoryQuery),
    responses(
        (status = 200, description = "알림 목록", body = FrontendAlertHistoryResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    ),
    tag = "alerts"
)]
pub async fn list_alert_history(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AlertHistoryQuery>,
) -> Result<Json<FrontendAlertHistoryResponse>, (StatusCode, Json<ApiError>)> {
    let db_pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONFIGURED", "데이터베이스가 구성되지 않았습니다")),
        )
    })?;

    // 필터 생성
    let filter = AlertFilter {
        alert_type: query.alert_type,
        channel: query.channel,
        status: query.status,
        limit: query.limit,
        offset: query.offset,
        ..Default::default()
    };

    // 알림 목록 조회
    let result = AlertsRepository::list(db_pool, &filter)
        .await
        .map_err(|e| {
            warn!(error = %e, "알림 히스토리 조회 실패");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("알림 조회 실패: {}", e))),
            )
        })?;

    // 읽지 않은 알림 수 조회
    let unread_count = AlertsRepository::count_unread(db_pool)
        .await
        .unwrap_or(0);

    let alerts: Vec<FrontendAlertHistoryItem> = result.alerts.into_iter().map(Into::into).collect();

    debug!(count = alerts.len(), total = result.total, unread = unread_count, "알림 히스토리 조회");

    Ok(Json(FrontendAlertHistoryResponse {
        alerts,
        total: result.total,
        unread_count,
    }))
}

/// 알림 읽음 처리
///
/// 특정 알림을 읽음 처리합니다.
#[utoipa::path(
    patch,
    path = "/api/v1/alerts/history/{id}/read",
    params(
        ("id" = Uuid, Path, description = "알림 ID")
    ),
    responses(
        (status = 204, description = "읽음 처리 완료"),
        (status = 404, description = "알림 없음", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError)
    ),
    tag = "alerts"
)]
pub async fn mark_alert_as_read(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let db_pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONFIGURED", "데이터베이스가 구성되지 않았습니다")),
        )
    })?;

    // 알림 존재 확인
    let alert = AlertsRepository::get_by_id(db_pool, id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("알림 조회 실패: {}", e))),
            )
        })?;

    if alert.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "알림을 찾을 수 없습니다")),
        ));
    }

    // 읽음 처리 (acknowledge)
    AlertsRepository::acknowledge(db_pool, id, Some("user"))
        .await
        .map_err(|e| {
            warn!(error = %e, alert_id = %id, "알림 읽음 처리 실패");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("알림 읽음 처리 실패: {}", e))),
            )
        })?;

    debug!(alert_id = %id, "알림 읽음 처리 완료");
    Ok(StatusCode::NO_CONTENT)
}

/// 모든 알림 읽음 처리
///
/// 모든 알림을 읽음 처리합니다.
#[utoipa::path(
    patch,
    path = "/api/v1/alerts/history/read-all",
    responses(
        (status = 204, description = "전체 읽음 처리 완료"),
        (status = 500, description = "서버 오류", body = ApiError)
    ),
    tag = "alerts"
)]
pub async fn mark_all_alerts_as_read(
    State(state): State<Arc<AppState>>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let db_pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONFIGURED", "데이터베이스가 구성되지 않았습니다")),
        )
    })?;

    AlertsRepository::acknowledge_all(db_pool)
        .await
        .map_err(|e| {
            warn!(error = %e, "전체 알림 읽음 처리 실패");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("전체 알림 읽음 처리 실패: {}", e))),
            )
        })?;

    debug!("전체 알림 읽음 처리 완료");
    Ok(StatusCode::NO_CONTENT)
}

// ==================== 라우터 ====================

/// 알림 히스토리 API 라우터
pub fn alert_history_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/history", get(list_alert_history))
        .route("/history/{id}/read", patch(mark_alert_as_read))
        .route("/history/read-all", patch(mark_all_alerts_as_read))
}
