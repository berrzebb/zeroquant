//! HTTP 요청 metrics middleware.
//!
//! 모든 HTTP 요청에 대해 메트릭을 수집합니다.

use axum::{extract::Request, middleware::Next, response::Response};
use std::time::Instant;

use crate::metrics::{
    normalize_path, record_http_duration, record_http_request, record_http_response,
};

/// HTTP 메트릭을 수집하는 미들웨어 레이어.
///
/// 각 요청에 대해 다음 메트릭을 기록합니다:
/// - `http_requests_total`: 총 요청 수 (method, path 라벨)
/// - `http_responses_total`: 총 응답 수 (method, path, status 라벨)
/// - `http_request_duration_seconds`: 요청 처리 시간 히스토그램
pub async fn metrics_layer(request: Request, next: Next) -> Response {
    let start = Instant::now();

    // 요청 정보 추출
    let method = request.method().to_string();
    let path = normalize_path(request.uri().path());

    // 요청 카운터 증가
    record_http_request(&method, &path);

    // 다음 핸들러 호출
    let response = next.run(request).await;

    // 응답 정보 기록
    let status = response.status().as_u16();
    let duration = start.elapsed().as_secs_f64();

    // 메트릭 기록
    record_http_response(&method, &path, status);
    record_http_duration(&method, &path, duration);

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    async fn test_handler() -> &'static str {
        "OK"
    }

    #[tokio::test]
    async fn test_metrics_middleware() {
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn(metrics_layer));

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_metrics_middleware_with_params() {
        let app = Router::new()
            .route("/orders/{id}", get(test_handler))
            .layer(middleware::from_fn(metrics_layer));

        let request = Request::builder()
            .method(Method::GET)
            .uri("/orders/123e4567-e89b-12d3-a456-426614174000")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
