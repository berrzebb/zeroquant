//! 신호 알림 규칙 API 라우트.
//!
//! 알림 규칙 CRUD 엔드포인트를 제공합니다.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::error::{ApiErrorResponse, ApiResult};
use crate::repository::{
    CreateAlertRuleRequest, SignalAlertRule, SignalAlertRuleRepository, UpdateAlertRuleRequest,
};
use crate::AppState;

// ==================== Request/Response 타입 ====================

/// 알림 규칙 목록 조회 쿼리.
#[derive(Debug, Deserialize, IntoParams)]
pub struct ListAlertRulesQuery {
    /// 활성화된 규칙만 조회 (기본 false)
    #[serde(default)]
    pub enabled_only: bool,
}

/// 알림 규칙 목록 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct ListAlertRulesResponse {
    /// 총 규칙 수
    pub total: usize,
    /// 규칙 목록
    pub rules: Vec<SignalAlertRule>,
}

// ==================== API 핸들러 ====================

/// 알림 규칙 생성.
#[utoipa::path(
    post,
    path = "/api/v1/signal-alerts",
    tag = "signal-alerts",
    request_body = CreateAlertRuleRequest,
    responses(
        (status = 200, description = "알림 규칙 생성 성공", body = SignalAlertRule),
        (status = 500, description = "서버 에러", body = ApiErrorResponse)
    )
)]
pub async fn create_alert_rule(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateAlertRuleRequest>,
) -> ApiResult<Json<SignalAlertRule>> {
    let db_pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse::new(
                "DATABASE_ERROR",
                "Database not available",
            )),
        )
    })?;

    let repo = SignalAlertRuleRepository::new(db_pool.clone());
    let rule = repo.create(req).await?;

    Ok(Json(rule))
}

/// 알림 규칙 목록 조회.
#[utoipa::path(
    get,
    path = "/api/v1/signal-alerts",
    tag = "signal-alerts",
    params(ListAlertRulesQuery),
    responses(
        (status = 200, description = "알림 규칙 목록", body = ListAlertRulesResponse),
        (status = 500, description = "서버 에러", body = ApiErrorResponse)
    )
)]
pub async fn list_alert_rules(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListAlertRulesQuery>,
) -> ApiResult<Json<ListAlertRulesResponse>> {
    let db_pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse::new(
                "DATABASE_ERROR",
                "Database not available",
            )),
        )
    })?;

    let repo = SignalAlertRuleRepository::new(db_pool.clone());
    let rules = repo.list(query.enabled_only).await?;

    Ok(Json(ListAlertRulesResponse {
        total: rules.len(),
        rules,
    }))
}

/// ID로 알림 규칙 조회.
#[utoipa::path(
    get,
    path = "/api/v1/signal-alerts/{id}",
    tag = "signal-alerts",
    params(("id" = Uuid, Path, description = "알림 규칙 ID")),
    responses(
        (status = 200, description = "알림 규칙 상세", body = SignalAlertRule),
        (status = 404, description = "규칙 없음", body = ApiErrorResponse),
        (status = 500, description = "서버 에러", body = ApiErrorResponse)
    )
)]
pub async fn get_alert_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<SignalAlertRule>> {
    let db_pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse::new(
                "DATABASE_ERROR",
                "Database not available",
            )),
        )
    })?;

    let repo = SignalAlertRuleRepository::new(db_pool.clone());
    let rule = repo.find_by_id(id).await?;

    Ok(Json(rule))
}

/// 알림 규칙 수정.
#[utoipa::path(
    put,
    path = "/api/v1/signal-alerts/{id}",
    tag = "signal-alerts",
    params(("id" = Uuid, Path, description = "알림 규칙 ID")),
    request_body = UpdateAlertRuleRequest,
    responses(
        (status = 200, description = "알림 규칙 수정 성공", body = SignalAlertRule),
        (status = 404, description = "규칙 없음", body = ApiErrorResponse),
        (status = 500, description = "서버 에러", body = ApiErrorResponse)
    )
)]
pub async fn update_alert_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateAlertRuleRequest>,
) -> ApiResult<Json<SignalAlertRule>> {
    let db_pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse::new(
                "DATABASE_ERROR",
                "Database not available",
            )),
        )
    })?;

    let repo = SignalAlertRuleRepository::new(db_pool.clone());
    let rule = repo.update(id, req).await?;

    Ok(Json(rule))
}

/// 알림 규칙 삭제.
#[utoipa::path(
    delete,
    path = "/api/v1/signal-alerts/{id}",
    tag = "signal-alerts",
    params(("id" = Uuid, Path, description = "알림 규칙 ID")),
    responses(
        (status = 204, description = "삭제 성공"),
        (status = 404, description = "규칙 없음", body = ApiErrorResponse),
        (status = 500, description = "서버 에러", body = ApiErrorResponse)
    )
)]
pub async fn delete_alert_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let db_pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse::new(
                "DATABASE_ERROR",
                "Database not available",
            )),
        )
    })?;

    let repo = SignalAlertRuleRepository::new(db_pool.clone());
    repo.delete(id).await?;

    Ok(StatusCode::NO_CONTENT)
}

// ==================== 라우터 ====================

/// 신호 알림 규칙 API 라우터.
pub fn signal_alerts_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(create_alert_rule))
        .route("/", get(list_alert_rules))
        .route("/{:id}", get(get_alert_rule))
        .route("/{:id}", put(update_alert_rule))
        .route("/{:id}", delete(delete_alert_rule))
}
