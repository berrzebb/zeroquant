//! 모니터링 API 엔드포인트.
//!
//! AI 디버깅을 위한 에러 로그 조회 및 시스템 상태 확인 API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/monitoring/errors` - 최근 에러 목록 조회
//! - `GET /api/v1/monitoring/errors/critical` - Critical 에러만 조회
//! - `GET /api/v1/monitoring/errors/:id` - 특정 에러 상세 조회
//! - `GET /api/v1/monitoring/stats` - 에러 통계 조회
//! - `POST /api/v1/monitoring/stats/reset` - 통계 초기화
//! - `DELETE /api/v1/monitoring/errors` - 에러 히스토리 삭제

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::monitoring::{global_tracker, ErrorCategory, ErrorRecord, ErrorSeverity, ErrorStats};
use crate::state::AppState;

/// 에러 목록 조회 쿼리 파라미터.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ErrorsQuery {
    /// 조회할 최대 에러 수 (기본값: 50, 최대: 200)
    #[serde(default = "default_limit")]
    pub limit: usize,

    /// 심각도 필터 (warning, error, critical)
    pub severity: Option<String>,

    /// 카테고리 필터
    pub category: Option<String>,
}

fn default_limit() -> usize {
    50
}

/// 에러 목록 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorsResponse {
    /// 에러 목록
    pub errors: Vec<ErrorRecordDto>,
    /// 조회된 에러 수
    pub count: usize,
    /// 총 에러 수 (히스토리 내)
    pub total_in_history: usize,
}

/// 에러 레코드 DTO (API 응답용).
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorRecordDto {
    /// 에러 ID
    pub id: u64,
    /// 발생 시간 (ISO 8601)
    pub timestamp: String,
    /// 심각도 (warning, error, critical)
    pub severity: String,
    /// 카테고리
    pub category: String,
    /// 에러 메시지
    pub message: String,
    /// 발생 위치 (파일:라인)
    pub location: String,
    /// 함수명 (있는 경우)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// 관련 엔티티 (티커, 주문ID 등)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
    /// 상세 컨텍스트
    pub context: std::collections::HashMap<String, String>,
    /// 원본 에러
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_error: Option<String>,
    /// 스택 트레이스 (있는 경우)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backtrace: Option<String>,
}

impl From<ErrorRecord> for ErrorRecordDto {
    fn from(record: ErrorRecord) -> Self {
        Self {
            id: record.id,
            timestamp: record.timestamp.to_rfc3339(),
            severity: record.severity.to_string(),
            category: record.category.to_string(),
            message: record.message,
            location: format!(
                "{}:{}",
                record.source_location.file, record.source_location.line
            ),
            function: record.source_location.function,
            entity: record.entity,
            context: record.context,
            raw_error: record.raw_error,
            backtrace: record.backtrace,
        }
    }
}

/// 에러 통계 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StatsResponse {
    /// 심각도별 에러 수
    pub by_severity: std::collections::HashMap<String, u64>,
    /// 카테고리별 에러 수
    pub by_category: std::collections::HashMap<String, u64>,
    /// 총 에러 수
    pub total_count: u64,
    /// 마지막 에러 시간
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_at: Option<String>,
    /// 통계 시작 시간
    pub stats_since: String,
}

impl From<ErrorStats> for StatsResponse {
    fn from(stats: ErrorStats) -> Self {
        Self {
            by_severity: stats.by_severity,
            by_category: stats.by_category,
            total_count: stats.total_count,
            last_error_at: stats.last_error_at.map(|t| t.to_rfc3339()),
            stats_since: stats.stats_since.to_rfc3339(),
        }
    }
}

/// 최근 에러 목록 조회.
///
/// GET /api/v1/monitoring/errors
#[utoipa::path(
    get,
    path = "/api/v1/monitoring/errors",
    tag = "monitoring",
    params(
        ("limit" = Option<usize>, Query, description = "조회할 최대 에러 수 (기본값: 50)"),
        ("severity" = Option<String>, Query, description = "심각도 필터 (warning, error, critical)"),
        ("category" = Option<String>, Query, description = "카테고리 필터")
    ),
    responses(
        (status = 200, description = "에러 목록 조회 성공", body = ErrorsResponse)
    )
)]
pub async fn list_errors(Query(query): Query<ErrorsQuery>) -> impl IntoResponse {
    let tracker = global_tracker();
    let limit = query.limit.min(200);

    let errors: Vec<ErrorRecord> = match (&query.severity, &query.category) {
        (Some(sev), _) => {
            // 심각도 필터
            let severity = match sev.to_lowercase().as_str() {
                "warning" => ErrorSeverity::Warning,
                "error" => ErrorSeverity::Error,
                "critical" => ErrorSeverity::Critical,
                _ => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": "Invalid severity. Use: warning, error, critical"
                        })),
                    )
                        .into_response()
                }
            };
            tracker.get_by_severity(severity, limit)
        }
        (_, Some(cat)) => {
            // 카테고리 필터
            let category = match cat.to_lowercase().as_str() {
                "database" => ErrorCategory::Database,
                "external_api" => ErrorCategory::ExternalApi,
                "data_conversion" => ErrorCategory::DataConversion,
                "authentication" => ErrorCategory::Authentication,
                "network" => ErrorCategory::Network,
                "business_logic" => ErrorCategory::BusinessLogic,
                "system" => ErrorCategory::System,
                "other" => ErrorCategory::Other,
                _ => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": "Invalid category"
                        })),
                    )
                        .into_response()
                }
            };
            tracker.get_by_category(&category, limit)
        }
        _ => tracker.get_recent(limit),
    };

    let stats = tracker.get_stats();
    let response = ErrorsResponse {
        count: errors.len(),
        total_in_history: stats.total_count as usize,
        errors: errors.into_iter().map(ErrorRecordDto::from).collect(),
    };

    Json(response).into_response()
}

/// Critical 에러만 조회.
///
/// GET /api/v1/monitoring/errors/critical
#[utoipa::path(
    get,
    path = "/api/v1/monitoring/errors/critical",
    tag = "monitoring",
    params(
        ("limit" = Option<usize>, Query, description = "조회할 최대 에러 수 (기본값: 50)")
    ),
    responses(
        (status = 200, description = "Critical 에러 목록", body = ErrorsResponse)
    )
)]
pub async fn list_critical_errors(Query(query): Query<ErrorsQuery>) -> Json<ErrorsResponse> {
    let tracker = global_tracker();
    let limit = query.limit.min(200);

    let errors = tracker.get_by_severity(ErrorSeverity::Critical, limit);
    let stats = tracker.get_stats();

    Json(ErrorsResponse {
        count: errors.len(),
        total_in_history: stats.total_count as usize,
        errors: errors.into_iter().map(ErrorRecordDto::from).collect(),
    })
}

/// 특정 에러 상세 조회.
///
/// GET /api/v1/monitoring/errors/:id
#[utoipa::path(
    get,
    path = "/api/v1/monitoring/errors/{id}",
    tag = "monitoring",
    params(
        ("id" = u64, Path, description = "에러 ID")
    ),
    responses(
        (status = 200, description = "에러 상세 정보", body = ErrorRecordDto),
        (status = 404, description = "에러를 찾을 수 없음")
    )
)]
pub async fn get_error_by_id(Path(id): Path<u64>) -> impl IntoResponse {
    let tracker = global_tracker();

    // 최근 에러에서 ID로 검색
    let errors = tracker.get_recent(1000);

    if let Some(record) = errors.into_iter().find(|e| e.id == id) {
        Json(ErrorRecordDto::from(record)).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Error not found" })),
        )
            .into_response()
    }
}

/// 에러 통계 조회.
///
/// GET /api/v1/monitoring/stats
#[utoipa::path(
    get,
    path = "/api/v1/monitoring/stats",
    tag = "monitoring",
    responses(
        (status = 200, description = "에러 통계", body = StatsResponse)
    )
)]
pub async fn get_stats() -> Json<StatsResponse> {
    let stats = global_tracker().get_stats();
    Json(StatsResponse::from(stats))
}

/// 에러 통계 초기화.
///
/// POST /api/v1/monitoring/stats/reset
#[utoipa::path(
    post,
    path = "/api/v1/monitoring/stats/reset",
    tag = "monitoring",
    responses(
        (status = 200, description = "통계 초기화 완료")
    )
)]
pub async fn reset_stats() -> impl IntoResponse {
    global_tracker().reset_stats();
    Json(serde_json::json!({
        "message": "Stats reset successfully",
        "reset_at": chrono::Utc::now().to_rfc3339()
    }))
}

/// 에러 히스토리 삭제.
///
/// DELETE /api/v1/monitoring/errors
#[utoipa::path(
    delete,
    path = "/api/v1/monitoring/errors",
    tag = "monitoring",
    responses(
        (status = 200, description = "히스토리 삭제 완료")
    )
)]
pub async fn clear_errors() -> impl IntoResponse {
    global_tracker().clear_history();
    Json(serde_json::json!({
        "message": "Error history cleared",
        "cleared_at": chrono::Utc::now().to_rfc3339()
    }))
}

/// 시스템 요약 정보 (디버깅용).
///
/// GET /api/v1/monitoring/summary
#[utoipa::path(
    get,
    path = "/api/v1/monitoring/summary",
    tag = "monitoring",
    responses(
        (status = 200, description = "시스템 모니터링 요약")
    )
)]
pub async fn get_summary(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let tracker = global_tracker();
    let stats = tracker.get_stats();
    let critical_count = stats.by_severity.get("critical").copied().unwrap_or(0);
    let error_count = stats.by_severity.get("error").copied().unwrap_or(0);
    let warning_count = stats.by_severity.get("warning").copied().unwrap_or(0);

    // 최근 5개 에러 요약
    let recent_errors: Vec<_> = tracker
        .get_recent(5)
        .into_iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "severity": e.severity.to_string(),
                "category": e.category.to_string(),
                "message": e.message,
                "timestamp": e.timestamp.to_rfc3339(),
                "entity": e.entity,
            })
        })
        .collect();

    Json(serde_json::json!({
        "uptime_secs": state.uptime_secs(),
        "version": state.version,
        "error_summary": {
            "total": stats.total_count,
            "critical": critical_count,
            "error": error_count,
            "warning": warning_count,
            "stats_since": stats.stats_since.to_rfc3339(),
            "last_error_at": stats.last_error_at.map(|t| t.to_rfc3339()),
        },
        "recent_errors": recent_errors,
        "top_categories": stats.by_category,
    }))
}

/// 모니터링 라우터 생성.
pub fn monitoring_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/errors", get(list_errors).delete(clear_errors))
        .route("/errors/critical", get(list_critical_errors))
        .route("/errors/{id}", get(get_error_by_id))
        .route("/stats", get(get_stats))
        .route("/stats/reset", post(reset_stats))
        .route("/summary", get(get_summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    use crate::monitoring::{
        init_global_tracker, ErrorCategory, ErrorRecordBuilder, ErrorSeverity, ErrorTrackerConfig,
    };

    fn setup_tracker() {
        // 테스트용 트래커 초기화 (이미 초기화된 경우 무시됨)
        init_global_tracker(ErrorTrackerConfig::default());
    }

    #[tokio::test]
    async fn test_list_errors() {
        setup_tracker();

        // 테스트 에러 기록
        let record = ErrorRecordBuilder::new("테스트 에러")
            .severity(ErrorSeverity::Error)
            .category(ErrorCategory::Database)
            .entity("TEST")
            .build();
        global_tracker().record(record);

        let app = Router::new().route("/errors", get(list_errors));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/errors")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_stats() {
        setup_tracker();

        let app = Router::new().route("/stats", get(get_stats));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let stats: StatsResponse = serde_json::from_slice(&body).unwrap();

        assert!(!stats.stats_since.is_empty());
    }
}
