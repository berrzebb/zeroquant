//! Prometheus 메트릭 설정 및 유틸리티.
//!
//! HTTP 요청 메트릭, 비즈니스 메트릭을 수집하고 `/metrics` 엔드포인트로 노출합니다.

use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

/// Prometheus 메트릭 레코더를 설정하고 핸들을 반환합니다.
///
/// # 반환값
///
/// `/metrics` 엔드포인트에서 메트릭을 렌더링하기 위한 `PrometheusHandle`
///
/// # 패닉
///
/// 레코더가 이미 설치되어 있으면 패닉합니다.
pub fn setup_metrics_recorder() -> PrometheusHandle {
    PrometheusBuilder::new()
        // HTTP 요청 지속 시간 히스토그램 버킷 설정
        .set_buckets_for_metric(
            Matcher::Full("http_request_duration_seconds".to_string()),
            &[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
        )
        .expect("히스토그램 버킷 설정 실패")
        .install_recorder()
        .expect("Prometheus 레코더 설치 실패")
}

// ============================================================================
// HTTP 메트릭 헬퍼 함수
// ============================================================================

/// HTTP 요청 카운터 증가.
pub fn record_http_request(method: &str, path: &str) {
    counter!("http_requests_total", "method" => method.to_string(), "path" => path.to_string())
        .increment(1);
}

/// HTTP 응답 카운터 증가.
pub fn record_http_response(method: &str, path: &str, status: u16) {
    counter!(
        "http_responses_total",
        "method" => method.to_string(),
        "path" => path.to_string(),
        "status" => status.to_string()
    )
    .increment(1);
}

/// HTTP 요청 지속 시간 기록.
pub fn record_http_duration(method: &str, path: &str, duration_secs: f64) {
    histogram!(
        "http_request_duration_seconds",
        "method" => method.to_string(),
        "path" => path.to_string()
    )
    .record(duration_secs);
}

// ============================================================================
// 비즈니스 메트릭 헬퍼 함수
// ============================================================================

/// 거래 주문 카운터 증가.
pub fn record_order(side: &str, status: &str, exchange: &str) {
    counter!(
        "trading_orders_total",
        "side" => side.to_string(),
        "status" => status.to_string(),
        "exchange" => exchange.to_string()
    )
    .increment(1);
}

/// 열린 포지션 수 설정.
pub fn set_open_positions(exchange: &str, symbol: &str, count: f64) {
    gauge!(
        "trading_positions_open",
        "exchange" => exchange.to_string(),
        "symbol" => symbol.to_string()
    )
    .set(count);
}

/// 실현 손익 기록.
pub fn record_realized_pnl(strategy: &str, pnl: f64) {
    if pnl >= 0.0 {
        counter!("trading_pnl_realized_profit_total", "strategy" => strategy.to_string())
            .increment(pnl as u64);
    } else {
        counter!("trading_pnl_realized_loss_total", "strategy" => strategy.to_string())
            .increment((-pnl) as u64);
    }
}

/// WebSocket 연결 수 설정.
pub fn set_websocket_connections(count: f64) {
    gauge!("websocket_connections_active").set(count);
}

/// WebSocket 연결 수 증가.
pub fn increment_websocket_connections() {
    gauge!("websocket_connections_active").increment(1.0);
}

/// WebSocket 연결 수 감소.
pub fn decrement_websocket_connections() {
    gauge!("websocket_connections_active").decrement(1.0);
}

// ============================================================================
// 경로 정규화 유틸리티
// ============================================================================

/// 경로에서 동적 파라미터를 정규화합니다.
///
/// 예: `/orders/123e4567-e89b-12d3-a456-426614174000` → `/orders/:id`
pub fn normalize_path(path: &str) -> String {
    let segments: Vec<&str> = path.split('/').collect();
    let normalized: Vec<String> = segments
        .iter()
        .map(|segment| {
            // UUID 패턴 또는 숫자만 있는 경우 :id로 대체
            let is_uuid = segment.len() == 36 && segment.chars().filter(|c| *c == '-').count() == 4;
            let is_numeric = !segment.is_empty() && segment.chars().all(|c| c.is_ascii_digit());

            if is_uuid || is_numeric {
                ":id".to_string()
            } else {
                (*segment).to_string()
            }
        })
        .collect();
    normalized.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_uuid() {
        let path = "/api/v1/orders/123e4567-e89b-12d3-a456-426614174000";
        assert_eq!(normalize_path(path), "/api/v1/orders/:id");
    }

    #[test]
    fn test_normalize_path_numeric() {
        let path = "/api/v1/orders/12345";
        assert_eq!(normalize_path(path), "/api/v1/orders/:id");
    }

    #[test]
    fn test_normalize_path_no_params() {
        let path = "/api/v1/strategies";
        assert_eq!(normalize_path(path), "/api/v1/strategies");
    }

    #[test]
    fn test_normalize_path_mixed() {
        let path = "/api/v1/strategies/grid_btc/orders/123";
        assert_eq!(
            normalize_path(path),
            "/api/v1/strategies/grid_btc/orders/:id"
        );
    }
}
