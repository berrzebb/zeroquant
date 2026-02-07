//! 헬스 체크 endpoint.
//!
//! 서버 상태 확인을 위한 헬스 체크 엔드포인트를 제공합니다.
//! 로드밸런서나 오케스트레이션 시스템(Kubernetes 등)에서 사용됩니다.

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::state::AppState;

/// 헬스 체크 응답 구조체.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    /// 전체 서비스 상태 ("healthy" | "degraded" | "unhealthy")
    pub status: String,

    /// API 버전
    pub version: String,

    /// 서버 업타임(초)
    pub uptime_secs: i64,

    /// 현재 시간 (ISO 8601)
    pub timestamp: String,

    /// 개별 컴포넌트 상태
    pub components: ComponentHealth,
}

/// 개별 컴포넌트 상태.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ComponentHealth {
    /// 데이터베이스 연결 상태
    pub database: ComponentStatus,

    /// Redis 연결 상태
    pub redis: ComponentStatus,

    /// 전략 엔진 상태
    pub strategy_engine: ComponentStatus,
}

/// 컴포넌트 상태.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ComponentStatus {
    /// 상태 ("up" | "down" | "not_configured")
    pub status: String,

    /// 추가 정보 (선택적)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl ComponentStatus {
    /// 정상 상태.
    pub fn up() -> Self {
        Self {
            status: "up".to_string(),
            message: None,
        }
    }

    /// 비정상 상태.
    pub fn down(message: impl Into<String>) -> Self {
        Self {
            status: "down".to_string(),
            message: Some(message.into()),
        }
    }

    /// 미설정 상태.
    pub fn not_configured() -> Self {
        Self {
            status: "not_configured".to_string(),
            message: None,
        }
    }

    /// 정보 포함 정상 상태.
    pub fn up_with_info(message: impl Into<String>) -> Self {
        Self {
            status: "up".to_string(),
            message: Some(message.into()),
        }
    }
}

/// 간단한 헬스 체크 (liveness probe용).
///
/// 서버가 응답 가능한 상태인지만 확인합니다.
/// GET /health
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "서버가 정상 동작 중", body = String)
    )
)]
pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// 상세 헬스 체크 (readiness probe용).
///
/// 모든 의존성(DB, Redis 등)의 상태를 확인합니다.
/// GET /health/ready
#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "health",
    responses(
        (status = 200, description = "모든 컴포넌트 정상", body = HealthResponse),
        (status = 503, description = "일부 컴포넌트 장애", body = HealthResponse)
    )
)]
pub async fn health_ready(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut overall_status = "healthy";
    let mut status_code = StatusCode::OK;

    // 데이터베이스 상태 확인
    let database_status = if state.db_pool.is_some() {
        if state.is_db_healthy().await {
            ComponentStatus::up()
        } else {
            overall_status = "degraded";
            status_code = StatusCode::SERVICE_UNAVAILABLE;
            ComponentStatus::down("연결 실패")
        }
    } else {
        ComponentStatus::not_configured()
    };

    // Redis 캐시 상태 확인
    let redis_status = if state.has_cache() {
        if state.is_redis_healthy().await {
            ComponentStatus::up()
        } else {
            // Redis 실패는 degraded로 처리 (크리티컬하지 않음)
            if overall_status == "healthy" {
                overall_status = "degraded";
            }
            ComponentStatus::down("연결 실패")
        }
    } else {
        ComponentStatus::not_configured()
    };

    // 전략 엔진 상태 확인 - 최소 락 홀드
    let stats = {
        let engine = state.strategy_engine.read().await;
        engine.get_engine_stats().await
    }; // 락 해제됨

    // 락 없이 상태 생성
    let engine_status = ComponentStatus::up_with_info(format!(
        "{} strategies registered, {} running",
        stats.total_strategies, stats.running_strategies
    ));

    let response = HealthResponse {
        status: overall_status.to_string(),
        version: state.version.clone(),
        uptime_secs: state.uptime_secs(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        components: ComponentHealth {
            database: database_status,
            redis: redis_status,
            strategy_engine: engine_status,
        },
    };

    (status_code, Json(response))
}

/// 헬스 체크 라우터 생성.
pub fn health_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(health_check))
        .route("/ready", get(health_ready))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check_returns_ok() {
        let app = Router::new().route("/health", get(health_check));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_ready_returns_json() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/health/ready", get(health_ready))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let health: HealthResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(health.status, "healthy");
        assert!(!health.version.is_empty());
    }

    #[test]
    fn test_component_status_variants() {
        let up = ComponentStatus::up();
        assert_eq!(up.status, "up");
        assert!(up.message.is_none());

        let down = ComponentStatus::down("error");
        assert_eq!(down.status, "down");
        assert_eq!(down.message, Some("error".to_string()));

        let not_configured = ComponentStatus::not_configured();
        assert_eq!(not_configured.status, "not_configured");
    }
}
