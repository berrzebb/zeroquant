//! 주문 관리 endpoint.
//!
//! 주문 목록 조회, 생성, 취소를 위한 REST API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/orders` - 활성 주문 목록 조회
//! - `GET /api/v1/orders/:id` - 특정 주문 상세 조회
//! - `DELETE /api/v1/orders/:id` - 주문 취소

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::metrics::record_order;
use crate::routes::strategies::ApiError;
use crate::state::AppState;
use crate::websocket::{OrderUpdateData, ServerMessage};
use trader_core::{Order, OrderStatusType, OrderType, Side};

// ==================== 응답 타입 ====================

/// 주문 목록 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OrdersListResponse {
    /// 주문 목록
    pub orders: Vec<OrderResponse>,
    /// 전체 주문 수
    pub total: usize,
}

/// 주문 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OrderResponse {
    /// 주문 ID
    pub id: String,
    /// 거래소 주문 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange_order_id: Option<String>,
    /// 심볼
    pub symbol: String,
    /// 표시 이름 (예: "005930(삼성전자)")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 주문 방향
    pub side: Side,
    /// 주문 유형
    pub order_type: OrderType,
    /// 주문 수량
    pub quantity: Decimal,
    /// 체결 수량
    pub filled_quantity: Decimal,
    /// 주문 가격 (시장가 주문은 None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Decimal>,
    /// 평균 체결 가격
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_fill_price: Option<Decimal>,
    /// 주문 상태
    pub status: OrderStatusType,
    /// 전략 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy_id: Option<String>,
    /// 생성 시간
    pub created_at: String,
    /// 업데이트 시간
    pub updated_at: String,
}

impl From<&Order> for OrderResponse {
    fn from(order: &Order) -> Self {
        Self {
            id: order.id.to_string(),
            exchange_order_id: order.exchange_order_id.clone(),
            symbol: order.ticker.to_string(),
            display_name: None, // 핸들러에서 설정
            side: order.side,
            order_type: order.order_type,
            quantity: order.quantity,
            filled_quantity: order.filled_quantity,
            price: order.price,
            average_fill_price: order.average_fill_price,
            status: order.status,
            strategy_id: order.strategy_id.clone(),
            created_at: order.created_at.to_rfc3339(),
            updated_at: order.updated_at.to_rfc3339(),
        }
    }
}

/// 주문 취소 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CancelOrderResponse {
    /// 성공 여부
    pub success: bool,
    /// 취소된 주문 ID
    pub order_id: String,
    /// 메시지
    pub message: String,
}

/// 주문 취소 요청.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CancelOrderRequest {
    /// 취소 사유 (선택)
    #[serde(default)]
    pub reason: Option<String>,
}

/// 주문 생성 요청.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrderRequest {
    /// 심볼
    pub symbol: String,
    /// 주문 방향 (Buy/Sell)
    pub side: Side,
    /// 주문 유형 (Market/Limit)
    #[serde(rename = "type")]
    pub order_type: OrderType,
    /// 주문 수량
    pub quantity: Decimal,
    /// 주문 가격 (지정가 주문시 필수)
    #[serde(default)]
    pub price: Option<Decimal>,
}

/// 주문 생성 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrderResponse {
    /// 성공 여부
    pub success: bool,
    /// 생성된 주문 ID
    pub order_id: String,
    /// 메시지
    pub message: String,
    /// 주문 상세
    pub order: OrderResponse,
}

/// 주문 통계 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct OrderStatsResponse {
    /// 전체 주문 수
    pub total: usize,
    /// 상태별 주문 수
    pub by_status: OrderStatsByStatus,
    /// 방향별 주문 수
    pub by_side: OrderStatsBySide,
}

/// 상태별 주문 통계.
#[derive(Debug, Serialize, ToSchema)]
pub struct OrderStatsByStatus {
    /// 대기 중
    pub pending: usize,
    /// 열림
    pub open: usize,
    /// 부분 체결
    pub partially_filled: usize,
}

/// 방향별 주문 통계.
#[derive(Debug, Serialize, ToSchema)]
pub struct OrderStatsBySide {
    /// 매수
    pub buy: usize,
    /// 매도
    pub sell: usize,
}

// ==================== Handler ====================

/// 주문 생성.
#[utoipa::path(
    post,
    path = "/api/v1/orders",
    tag = "orders",
    request_body = CreateOrderRequest,
    responses(
        (status = 200, description = "주문 생성 성공", body = CreateOrderResponse),
        (status = 400, description = "잘못된 요청", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn create_order(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateOrderRequest>,
) -> Result<Json<CreateOrderResponse>, (StatusCode, Json<ApiError>)> {
    use trader_core::{MarketType, OrderRequest, Symbol, TimeInForce};

    // 심볼 파싱 (기본적으로 Crypto 시장으로 가정)
    // 심볼 형식: "BTC/USDT" 또는 "AAPL/USD"
    let symbol = Symbol::from_string(&request.symbol, MarketType::Crypto).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_SYMBOL",
                format!(
                    "Invalid symbol format: {}. Expected format: BASE/QUOTE (e.g., BTC/USDT)",
                    request.symbol
                ),
            )),
        )
    })?;

    // 지정가 주문시 가격 필수 체크
    if request.order_type == OrderType::Limit && request.price.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "PRICE_REQUIRED",
                "지정가 주문시 가격이 필요합니다",
            )),
        ));
    }

    // OrderRequest 생성
    let order_request = OrderRequest {
        ticker: symbol.to_string(),
        side: request.side,
        order_type: request.order_type,
        quantity: request.quantity,
        price: request.price,
        stop_price: None,
        time_in_force: TimeInForce::GTC,
        client_order_id: None,
        strategy_id: None,
    };

    // Order 생성 (Order::from_request 사용)
    let order = Order::from_request(order_request, "api_manual");
    let order_id = order.id;

    // OrderManager에 주문 추가 - 최소 락 홀드
    // executor 락을 빠르게 해제하기 위해 Arc 클론 후 즉시 드롭
    let order_manager = {
        let executor = state.executor.read().await;
        std::sync::Arc::clone(executor.order_manager())
    }; // executor 락 해제됨

    {
        let mut order_manager_guard = order_manager.write().await;
        if let Err(e) = order_manager_guard.add_order(order.clone()) {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("ORDER_ADD_FAILED", e.to_string())),
            ));
        }
    }

    // 메트릭 기록
    let side_str = match request.side {
        Side::Buy => "buy",
        Side::Sell => "sell",
    };
    record_order(&request.symbol, side_str, "manual");

    // WebSocket 브로드캐스트: 주문 생성 알림
    state.broadcast(ServerMessage::OrderUpdate(OrderUpdateData {
        order_id: order_id.to_string(),
        symbol: request.symbol.clone(),
        status: "pending".to_string(),
        side: side_str.to_string(),
        order_type: format!("{:?}", request.order_type).to_lowercase(),
        quantity: request.quantity,
        filled_quantity: Decimal::ZERO,
        price: request.price,
        average_price: None,
        timestamp: Utc::now().timestamp_millis(),
    }));

    // display_name 조회
    let mut order_response = OrderResponse::from(&order);
    order_response.display_name = Some(state.get_display_name(&request.symbol, false).await);

    Ok(Json(CreateOrderResponse {
        success: true,
        order_id: order_id.to_string(),
        message: "주문이 성공적으로 생성되었습니다".to_string(),
        order: order_response,
    }))
}

/// 활성 주문 목록 조회.
#[utoipa::path(
    get,
    path = "/api/v1/orders",
    tag = "orders",
    responses(
        (status = 200, description = "주문 목록 조회 성공", body = OrdersListResponse)
    )
)]
pub async fn list_orders(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // 최소 락 홀드: 주문 목록만 빠르게 복사
    let orders = {
        let executor = state.executor.read().await;
        executor.get_active_orders().await
    }; // 락 해제됨

    // 락 없이 후속 작업 수행
    let symbols: Vec<String> = orders.iter().map(|o| o.ticker.clone()).collect();
    let display_names = state.get_display_names(&symbols, false).await;

    let order_responses: Vec<OrderResponse> = orders
        .iter()
        .map(|o| {
            let mut resp = OrderResponse::from(o);
            if let Some(name) = display_names.get(&o.ticker) {
                resp.display_name = Some(name.clone());
            }
            resp
        })
        .collect();

    let total = order_responses.len();

    Json(OrdersListResponse {
        orders: order_responses,
        total,
    })
}

/// 특정 주문 상세 조회.
#[utoipa::path(
    get,
    path = "/api/v1/orders/{id}",
    tag = "orders",
    params(
        ("id" = String, Path, description = "주문 ID (UUID)")
    ),
    responses(
        (status = 200, description = "주문 조회 성공", body = OrderResponse),
        (status = 400, description = "잘못된 주문 ID", body = ApiError),
        (status = 404, description = "주문 없음", body = ApiError)
    )
)]
pub async fn get_order(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<OrderResponse>, (StatusCode, Json<ApiError>)> {
    // UUID 파싱
    let order_id = Uuid::parse_str(&id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_ORDER_ID",
                format!("Invalid order ID format: {}", id),
            )),
        )
    })?;

    // 최소 락 홀드: 주문 조회 후 즉시 락 해제
    let order = {
        let executor = state.executor.read().await;
        executor.get_order(order_id).await
    }; // 락 해제됨

    // 락 없이 응답 생성
    match order {
        Some(order) => {
            let mut resp = OrderResponse::from(&order);
            resp.display_name = Some(
                state
                    .get_display_name(&order.ticker.to_string(), false)
                    .await,
            );
            Ok(Json(resp))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new(
                "ORDER_NOT_FOUND",
                format!("Order not found: {}", id),
            )),
        )),
    }
}

/// 주문 취소.
#[utoipa::path(
    delete,
    path = "/api/v1/orders/{id}",
    tag = "orders",
    params(
        ("id" = String, Path, description = "주문 ID (UUID)")
    ),
    request_body(content = Option<CancelOrderRequest>, description = "취소 사유 (선택)"),
    responses(
        (status = 200, description = "주문 취소 성공", body = CancelOrderResponse),
        (status = 400, description = "잘못된 주문 ID", body = ApiError),
        (status = 404, description = "주문 없음", body = ApiError),
        (status = 500, description = "취소 실패", body = ApiError)
    )
)]
pub async fn cancel_order(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Option<Json<CancelOrderRequest>>,
) -> Result<Json<CancelOrderResponse>, (StatusCode, Json<ApiError>)> {
    // UUID 파싱
    let order_id = Uuid::parse_str(&id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_ORDER_ID",
                format!("Invalid order ID format: {}", id),
            )),
        )
    })?;

    let reason = body.and_then(|b| b.reason.clone());

    // 최소 락 홀드: 주문 정보 조회 후 즉시 해제
    let order_info = {
        let executor = state.executor.read().await;
        match executor.get_order(order_id).await {
            Some(order) => order,
            None => {
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(ApiError::new(
                        "ORDER_NOT_FOUND",
                        format!("Order not found: {}", id),
                    )),
                ));
            }
        }
    }; // 락 해제됨

    // 주문 취소 (별도 락 획득)
    let cancel_result = {
        let executor = state.executor.read().await;
        executor.cancel_order(order_id, reason).await
    }; // 락 해제됨

    match cancel_result {
        Ok(()) => {
            // 주문 취소 메트릭 기록
            let side_str = match order_info.side {
                Side::Buy => "buy",
                Side::Sell => "sell",
            };
            record_order(&order_info.ticker, "cancelled", "default");

            // WebSocket 브로드캐스트: 주문 취소 알림
            state.broadcast(ServerMessage::OrderUpdate(OrderUpdateData {
                order_id: order_id.to_string(),
                symbol: order_info.ticker.clone(),
                status: "cancelled".to_string(),
                side: side_str.to_string(),
                order_type: format!("{:?}", order_info.order_type).to_lowercase(),
                quantity: order_info.quantity,
                filled_quantity: order_info.filled_quantity,
                price: order_info.price,
                average_price: order_info.average_fill_price,
                timestamp: Utc::now().timestamp_millis(),
            }));

            Ok(Json(CancelOrderResponse {
                success: true,
                order_id: id,
                message: "주문이 성공적으로 취소되었습니다".to_string(),
            }))
        }
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("CANCEL_FAILED", err.to_string())),
        )),
    }
}

/// 주문 통계 조회.
#[utoipa::path(
    get,
    path = "/api/v1/orders/stats",
    tag = "orders",
    responses(
        (status = 200, description = "주문 통계 조회 성공", body = OrderStatsResponse)
    )
)]
pub async fn get_order_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // 최소 락 홀드: 주문 목록만 빠르게 복사
    let orders = {
        let executor = state.executor.read().await;
        executor.get_active_orders().await
    }; // 락 해제됨

    // 락 없이 통계 계산
    let total = orders.len();
    let pending = orders
        .iter()
        .filter(|o| o.status == OrderStatusType::Pending)
        .count();
    let open = orders
        .iter()
        .filter(|o| o.status == OrderStatusType::Open)
        .count();
    let partially_filled = orders
        .iter()
        .filter(|o| o.status == OrderStatusType::PartiallyFilled)
        .count();

    let buy_orders = orders.iter().filter(|o| o.side == Side::Buy).count();
    let sell_orders = orders.iter().filter(|o| o.side == Side::Sell).count();

    Json(OrderStatsResponse {
        total,
        by_status: OrderStatsByStatus {
            pending,
            open,
            partially_filled,
        },
        by_side: OrderStatsBySide {
            buy: buy_orders,
            sell: sell_orders,
        },
    })
}

// ==================== 라우터 ====================

/// 주문 관리 라우터 생성.
pub fn orders_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_orders).post(create_order))
        .route("/stats", get(get_order_stats))
        .route("/{id}", get(get_order).delete(cancel_order))
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::delete,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_list_orders_empty() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/orders", get(list_orders))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/orders")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list: OrdersListResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(list.total, 0);
        assert!(list.orders.is_empty());
    }

    #[tokio::test]
    async fn test_get_order_not_found() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/orders/{id}", get(get_order))
            .with_state(state);

        let order_id = Uuid::new_v4();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/orders/{}", order_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_order_invalid_id() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/orders/{id}", get(get_order))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/orders/invalid-uuid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error: ApiError = serde_json::from_slice(&body).unwrap();

        assert_eq!(error.code, "INVALID_ORDER_ID");
    }

    #[tokio::test]
    async fn test_cancel_order_not_found() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/orders/{id}", delete(cancel_order))
            .with_state(state);

        let order_id = Uuid::new_v4();
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/orders/{}", order_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_order_stats() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/orders/stats", get(get_order_stats))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/orders/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let stats: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(stats["total"], 0);
    }
}
