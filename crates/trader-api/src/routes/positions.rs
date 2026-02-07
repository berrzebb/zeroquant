//! 포지션 관리 endpoint.
//!
//! 포지션 목록 조회 및 개별 포지션 상세 정보를 위한 REST API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/positions` - 열린 포지션 목록 조회
//! - `GET /api/v1/positions/summary` - 포지션 요약 통계
//! - `GET /api/v1/positions/{symbol}` - 특정 심볼 포지션 조회

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::routes::strategies::ApiError;
use crate::state::AppState;
use trader_core::{Position, Side};

// ==================== 응답 타입 ====================

/// 포지션 목록 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PositionsListResponse {
    /// 포지션 목록
    pub positions: Vec<PositionResponse>,
    /// 전체 포지션 수
    pub total: usize,
    /// 요약 정보
    pub summary: PositionSummaryResponse,
}

/// 포지션 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PositionResponse {
    /// 포지션 ID
    pub id: String,
    /// 거래소 이름
    pub exchange: String,
    /// 심볼
    pub symbol: String,
    /// 표시 이름 (예: "005930(삼성전자)")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 포지션 방향 (Long/Short)
    pub side: Side,
    /// 현재 수량
    pub quantity: Decimal,
    /// 평균 진입 가격
    pub entry_price: Decimal,
    /// 현재 시장 가격
    pub current_price: Decimal,
    /// 미실현 손익
    pub unrealized_pnl: Decimal,
    /// 실현 손익
    pub realized_pnl: Decimal,
    /// 포지션 가치 (현재가 × 수량)
    pub notional_value: Decimal,
    /// 수익률 (%)
    pub return_pct: Decimal,
    /// 전략 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy_id: Option<String>,
    /// 포지션 오픈 시간
    pub opened_at: String,
    /// 마지막 업데이트 시간
    pub updated_at: String,
}

impl From<&Position> for PositionResponse {
    fn from(position: &Position) -> Self {
        Self {
            id: position.id.to_string(),
            exchange: position.exchange.clone(),
            symbol: position.ticker.to_string(),
            display_name: None, // 핸들러에서 설정
            side: position.side,
            quantity: position.quantity,
            entry_price: position.entry_price,
            current_price: position.current_price,
            unrealized_pnl: position.unrealized_pnl,
            realized_pnl: position.realized_pnl,
            notional_value: position.notional_value(),
            return_pct: position.return_pct(),
            strategy_id: position.strategy_id.clone(),
            opened_at: position.opened_at.to_rfc3339(),
            updated_at: position.updated_at.to_rfc3339(),
        }
    }
}

impl PositionResponse {
    /// display_name 설정
    pub fn with_display_name(mut self, name: String) -> Self {
        self.display_name = Some(name);
        self
    }
}

/// 포지션 요약 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PositionSummaryResponse {
    /// 전체 오픈 포지션 수
    pub total_positions: usize,
    /// 총 미실현 손익
    pub total_unrealized_pnl: Decimal,
    /// 총 실현 손익
    pub total_realized_pnl: Decimal,
    /// 총 포지션 가치
    pub total_notional_value: Decimal,
    /// 롱 포지션 수
    pub long_count: usize,
    /// 숏 포지션 수
    pub short_count: usize,
}

impl PositionSummaryResponse {
    /// 포지션 목록에서 요약 생성.
    pub fn from_positions(positions: &[Position]) -> Self {
        let open_positions: Vec<_> = positions.iter().filter(|p| p.is_open()).collect();

        Self {
            total_positions: open_positions.len(),
            total_unrealized_pnl: open_positions.iter().map(|p| p.unrealized_pnl).sum(),
            total_realized_pnl: positions.iter().map(|p| p.realized_pnl).sum(),
            total_notional_value: open_positions.iter().map(|p| p.notional_value()).sum(),
            long_count: open_positions
                .iter()
                .filter(|p| p.side == Side::Buy)
                .count(),
            short_count: open_positions
                .iter()
                .filter(|p| p.side == Side::Sell)
                .count(),
        }
    }
}

// ==================== handler ====================

/// 열린 포지션 목록 조회.
#[utoipa::path(
    get,
    path = "/api/v1/positions",
    tag = "positions",
    responses(
        (status = 200, description = "포지션 목록 조회 성공", body = PositionsListResponse)
    )
)]
pub async fn list_positions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let executor = state.executor.read().await;
    let positions = executor.get_open_positions().await;

    // 심볼 목록 추출
    let symbols: Vec<String> = positions.iter().map(|p| p.ticker.to_string()).collect();

    // display name 배치 조회
    let display_names = state.get_display_names(&symbols, false).await;

    // 응답 생성 및 display_name 설정
    let position_responses: Vec<PositionResponse> = positions
        .iter()
        .map(|p| {
            let mut resp = PositionResponse::from(p);
            if let Some(name) = display_names.get(&p.ticker.to_string()) {
                resp.display_name = Some(name.clone());
            }
            resp
        })
        .collect();

    let summary = PositionSummaryResponse::from_positions(&positions);
    let total = position_responses.len();

    Json(PositionsListResponse {
        positions: position_responses,
        total,
        summary,
    })
}

/// 포지션 요약 통계 조회.
#[utoipa::path(
    get,
    path = "/api/v1/positions/summary",
    tag = "positions",
    responses(
        (status = 200, description = "포지션 요약 조회 성공", body = PositionSummaryResponse)
    )
)]
pub async fn get_positions_summary(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let executor = state.executor.read().await;
    let positions = executor.get_open_positions().await;

    Json(PositionSummaryResponse::from_positions(&positions))
}

/// 특정 심볼 포지션 조회.
#[utoipa::path(
    get,
    path = "/api/v1/positions/{symbol}",
    tag = "positions",
    params(
        ("symbol" = String, Path, description = "심볼 (예: BTC/USDT)")
    ),
    responses(
        (status = 200, description = "포지션 조회 성공", body = PositionResponse),
        (status = 404, description = "포지션 없음", body = ApiError)
    )
)]
pub async fn get_position(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
) -> Result<Json<PositionResponse>, (StatusCode, Json<ApiError>)> {
    let executor = state.executor.read().await;

    match executor.get_position(&symbol).await {
        Some(position) => {
            let mut resp = PositionResponse::from(&position);
            // display_name 조회
            resp.display_name = Some(state.get_display_name(&symbol, false).await);
            Ok(Json(resp))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new(
                "POSITION_NOT_FOUND",
                format!("No open position for symbol: {}", symbol),
            )),
        )),
    }
}

// ==================== router ====================

/// 포지션 관리 라우터 생성.
pub fn positions_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_positions))
        .route("/summary", get(get_positions_summary))
        .route("/{symbol}", get(get_position))
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_list_positions_empty() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/positions", get(list_positions))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/positions")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list: PositionsListResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(list.total, 0);
        assert!(list.positions.is_empty());
        assert_eq!(list.summary.total_positions, 0);
    }

    #[tokio::test]
    async fn test_get_position_not_found() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/positions/{symbol}", get(get_position))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/positions/BTC-USDT")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error: ApiError = serde_json::from_slice(&body).unwrap();

        assert_eq!(error.code, "POSITION_NOT_FOUND");
    }

    #[tokio::test]
    async fn test_get_positions_summary() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/positions/summary", get(get_positions_summary))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/positions/summary")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let summary: PositionSummaryResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(summary.total_positions, 0);
        assert_eq!(summary.long_count, 0);
        assert_eq!(summary.short_count, 0);
    }

    #[test]
    fn test_position_summary_calculation() {
        use rust_decimal_macros::dec;

        let mut positions = vec![
            Position::new(
                "binance",
                "BTC/USDT".to_string(),
                Side::Buy,
                dec!(1.0),
                dec!(50000),
            ),
            Position::new(
                "binance",
                "ETH/USDT".to_string(),
                Side::Buy,
                dec!(10.0),
                dec!(3000),
            ),
            Position::new(
                "binance",
                "SOL/USDT".to_string(),
                Side::Sell,
                dec!(100.0),
                dec!(100),
            ),
        ];

        // 가격 업데이트하여 PnL 계산
        positions[0].update_price(dec!(55000)); // +5000
        positions[1].update_price(dec!(3200)); // +2000
        positions[2].update_price(dec!(90)); // +1000 (숏이므로 하락이 이익)

        let summary = PositionSummaryResponse::from_positions(&positions);

        assert_eq!(summary.total_positions, 3);
        assert_eq!(summary.long_count, 2);
        assert_eq!(summary.short_count, 1);
        assert_eq!(summary.total_unrealized_pnl, dec!(8000)); // 5000 + 2000 + 1000
    }
}
