//! 백테스트 결과 저장/조회 API.
//!
//! 백테스트 결과를 PostgreSQL에 영구 저장하고 조회하는 기능을 제공합니다.
//!
//! # Repository 패턴
//!
//! 데이터베이스 접근 로직은 `crate::repository::backtest_results`에 있습니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/backtest/results` - 저장된 결과 목록 조회
//! - `POST /api/v1/backtest/results` - 결과 저장
//! - `GET /api/v1/backtest/results/{id}` - 단일 결과 조회
//! - `DELETE /api/v1/backtest/results/{id}` - 결과 삭제

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, warn};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::repository::{
    BacktestResultDto, BacktestResultInput, BacktestResultsRepository, ListResultsFilter,
};
use crate::state::AppState;

// ==================== 요청/응답 타입 (API용) ====================

/// 결과 저장 요청.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveBacktestResultRequest {
    /// 전략 ID (등록된 전략의 고유 ID)
    pub strategy_id: String,
    /// 전략 타입 (sma_crossover, bollinger 등)
    pub strategy_type: String,
    /// 심볼 (다중 자산은 콤마 구분)
    pub symbol: String,
    /// 시작 날짜 (YYYY-MM-DD)
    pub start_date: String,
    /// 종료 날짜 (YYYY-MM-DD)
    pub end_date: String,
    /// 초기 자본
    pub initial_capital: Decimal,
    /// 슬리피지율
    #[serde(default)]
    pub slippage_rate: Option<Decimal>,
    /// 성과 지표
    pub metrics: serde_json::Value,
    /// 설정 요약
    pub config_summary: serde_json::Value,
    /// 자산 곡선
    pub equity_curve: serde_json::Value,
    /// 거래 내역
    pub trades: serde_json::Value,
    /// 성공 여부
    pub success: bool,
    /// 백테스트에 사용된 타임프레임 설정 (다중 TF 백테스트 시)
    #[serde(default)]
    pub timeframes_used: Option<serde_json::Value>,
}

/// 저장된 결과 응답 (Repository DTO 재사용).
pub type BacktestResultResponse = BacktestResultDto;

/// 결과 목록 조회 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams)]
pub struct ListResultsQuery {
    /// 전략 ID 필터
    #[serde(default)]
    pub strategy_id: Option<String>,
    /// 전략 타입 필터
    #[serde(default)]
    pub strategy_type: Option<String>,
    /// 결과 수 제한
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// 오프셋
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

/// 결과 목록 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct ListResultsResponse {
    pub results: Vec<BacktestResultResponse>,
    pub total: i64,
}

/// 저장 성공 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct SaveResultResponse {
    pub id: String,
    pub message: String,
}

// ==================== 핸들러 ====================

/// 저장된 백테스트 결과 목록 조회.
///
/// `GET /api/v1/backtest/results`
#[utoipa::path(
    get,
    path = "/api/v1/backtest/results",
    tag = "backtest",
    params(ListResultsQuery),
    responses(
        (status = 200, description = "결과 목록", body = ListResultsResponse),
        (status = 503, description = "DB 미연결")
    )
)]
pub async fn list_backtest_results(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListResultsQuery>,
) -> impl IntoResponse {
    debug!("백테스트 결과 목록 조회: {:?}", query);

    let pool = match &state.db_pool {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "데이터베이스가 연결되지 않았습니다"
                })),
            )
                .into_response();
        }
    };

    // Repository를 통해 조회
    let filter = ListResultsFilter::new().with_pagination(query.limit, query.offset);

    let filter = match &query.strategy_id {
        Some(sid) => filter.with_strategy_id(sid),
        None => filter,
    };

    let filter = match &query.strategy_type {
        Some(stype) => filter.with_strategy_type(stype),
        None => filter,
    };

    match BacktestResultsRepository::list(pool, filter).await {
        Ok(response) => Json(ListResultsResponse {
            results: response.results,
            total: response.total,
        })
        .into_response(),
        Err(e) => {
            warn!("결과 목록 조회 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "결과 목록 조회 실패",
                    "details": e.to_string()
                })),
            )
                .into_response()
        }
    }
}

/// 백테스트 결과 저장.
///
/// `POST /api/v1/backtest/results`
#[utoipa::path(
    post,
    path = "/api/v1/backtest/results",
    tag = "backtest",
    request_body = SaveBacktestResultRequest,
    responses(
        (status = 201, description = "저장 성공", body = SaveResultResponse),
        (status = 400, description = "잘못된 요청"),
        (status = 503, description = "DB 미연결")
    )
)]
pub async fn save_backtest_result(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SaveBacktestResultRequest>,
) -> impl IntoResponse {
    debug!("백테스트 결과 저장: strategy_id={}", request.strategy_id);

    let pool = match &state.db_pool {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "데이터베이스가 연결되지 않았습니다"
                })),
            )
                .into_response();
        }
    };

    // 날짜 파싱
    let start_date = match NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "시작 날짜 형식이 올바르지 않습니다",
                    "details": e.to_string()
                })),
            )
                .into_response();
        }
    };

    let end_date = match NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "종료 날짜 형식이 올바르지 않습니다",
                    "details": e.to_string()
                })),
            )
                .into_response();
        }
    };

    // Repository Input 생성
    let input = BacktestResultInput {
        strategy_id: request.strategy_id,
        strategy_type: request.strategy_type,
        symbol: request.symbol,
        start_date,
        end_date,
        initial_capital: request.initial_capital,
        slippage_rate: request.slippage_rate,
        metrics: request.metrics,
        config_summary: request.config_summary,
        equity_curve: request.equity_curve,
        trades: request.trades,
        success: request.success,
        timeframes_used: request.timeframes_used,
    };

    match BacktestResultsRepository::save(pool, input).await {
        Ok(id) => {
            debug!("백테스트 결과 저장 완료: id={}", id);
            (
                StatusCode::CREATED,
                Json(SaveResultResponse {
                    id: id.to_string(),
                    message: "백테스트 결과가 저장되었습니다".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            warn!("결과 저장 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "결과 저장 실패",
                    "details": e.to_string()
                })),
            )
                .into_response()
        }
    }
}

/// 백테스트 결과 조회 (단일).
///
/// `GET /api/v1/backtest/results/{id}`
#[utoipa::path(
    get,
    path = "/api/v1/backtest/results/{id}",
    tag = "backtest",
    params(("id" = String, Path, description = "백테스트 결과 ID")),
    responses(
        (status = 200, description = "결과 상세", body = BacktestResultDto),
        (status = 404, description = "결과 없음"),
        (status = 503, description = "DB 미연결")
    )
)]
pub async fn get_backtest_result(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!("백테스트 결과 조회: id={}", id);

    let pool = match &state.db_pool {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "데이터베이스가 연결되지 않았습니다"
                })),
            )
                .into_response();
        }
    };

    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "유효하지 않은 ID 형식입니다"
                })),
            )
                .into_response();
        }
    };

    match BacktestResultsRepository::get_by_id(pool, uuid).await {
        Ok(Some(record)) => Json(BacktestResultDto::from(record)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "결과를 찾을 수 없습니다"
            })),
        )
            .into_response(),
        Err(e) => {
            warn!("결과 조회 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "결과 조회 실패",
                    "details": e.to_string()
                })),
            )
                .into_response()
        }
    }
}

/// 백테스트 결과 삭제 (soft delete).
///
/// `DELETE /api/v1/backtest/results/{id}`
#[utoipa::path(
    delete,
    path = "/api/v1/backtest/results/{id}",
    tag = "backtest",
    params(("id" = String, Path, description = "백테스트 결과 ID")),
    responses(
        (status = 200, description = "삭제 성공"),
        (status = 404, description = "결과 없음"),
        (status = 503, description = "DB 미연결")
    )
)]
pub async fn delete_backtest_result(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!("백테스트 결과 삭제: id={}", id);

    let pool = match &state.db_pool {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "데이터베이스가 연결되지 않았습니다"
                })),
            )
                .into_response();
        }
    };

    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "유효하지 않은 ID 형식입니다"
                })),
            )
                .into_response();
        }
    };

    match BacktestResultsRepository::delete(pool, uuid).await {
        Ok(true) => {
            debug!("백테스트 결과 삭제 완료: id={}", id);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "message": "백테스트 결과가 삭제되었습니다",
                    "id": id
                })),
            )
                .into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "결과를 찾을 수 없습니다"
            })),
        )
            .into_response(),
        Err(e) => {
            warn!("결과 삭제 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "결과 삭제 실패",
                    "details": e.to_string()
                })),
            )
                .into_response()
        }
    }
}

// ==================== 라우터 ====================

/// 백테스트 결과 라우터 생성.
///
/// Axum 0.8 기준: 라우트 파라미터는 `{id}` 문법 사용.
pub fn backtest_results_router() -> Router<Arc<AppState>> {
    Router::new()
        // 결과 목록 조회 + 저장 (같은 경로에 GET/POST)
        .route("/", get(list_backtest_results).post(save_backtest_result))
        // 단일 결과 조회 + 삭제 (같은 경로에 GET/DELETE)
        .route("/{id}", get(get_backtest_result).delete(delete_backtest_result))
}
