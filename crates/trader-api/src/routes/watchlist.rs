//! Watchlist API 라우트
//!
//! 관심종목 관리 API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/watchlist` - 모든 관심종목 그룹 조회
//! - `POST /api/v1/watchlist` - 새 관심종목 그룹 생성
//! - `GET /api/v1/watchlist/:id` - 그룹 상세 조회 (아이템 포함)
//! - `DELETE /api/v1/watchlist/:id` - 그룹 삭제
//! - `POST /api/v1/watchlist/:id/items` - 아이템 추가
//! - `DELETE /api/v1/watchlist/:id/items/:symbol` - 아이템 삭제
//! - `PUT /api/v1/watchlist/items/:item_id` - 아이템 수정

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::repository::{
    NewWatchlist, NewWatchlistItem, UpdateWatchlistItem, WatchlistItemRecord, WatchlistRecord,
    WatchlistRepository, WatchlistWithCount,
};
use crate::state::AppState;

// ================================================================================================
// Request/Response Types
// ================================================================================================

/// 관심종목 그룹 목록 응답
#[derive(Debug, Serialize, ToSchema)]
pub struct WatchlistListResponse {
    /// 그룹 목록
    pub watchlists: Vec<WatchlistWithCount>,
    /// 총 개수
    pub total: usize,
}

/// 관심종목 그룹 상세 응답 (아이템 포함)
#[derive(Debug, Serialize, ToSchema)]
pub struct WatchlistDetailResponse {
    /// 그룹 정보
    #[serde(flatten)]
    pub watchlist: WatchlistRecord,
    /// 아이템 목록
    pub items: Vec<WatchlistItemRecord>,
    /// 아이템 수
    pub item_count: usize,
}

/// 아이템 추가 요청
#[derive(Debug, Deserialize, ToSchema)]
pub struct AddItemsRequest {
    /// 추가할 아이템 목록
    pub items: Vec<NewWatchlistItem>,
}

/// 아이템 추가 응답
#[derive(Debug, Serialize, ToSchema)]
pub struct AddItemsResponse {
    /// 추가된 아이템 목록
    pub added: Vec<WatchlistItemRecord>,
    /// 추가된 개수
    pub count: usize,
}

/// 아이템 삭제 쿼리
#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteItemQuery {
    /// 시장 (기본값: KR)
    #[serde(default = "default_market")]
    pub market: String,
}

fn default_market() -> String {
    "KR".to_string()
}

/// 성공 응답
#[derive(Debug, Serialize, ToSchema)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
}

// ================================================================================================
// Handlers
// ================================================================================================

/// GET /api/v1/watchlist - 모든 관심종목 그룹 조회
#[utoipa::path(
    get,
    path = "/api/v1/watchlist",
    responses(
        (status = 200, description = "관심종목 그룹 목록", body = WatchlistListResponse),
        (status = 500, description = "서버 오류")
    ),
    tag = "watchlist"
)]
pub async fn list_watchlists(
    State(state): State<Arc<AppState>>,
) -> Result<Json<WatchlistListResponse>, (StatusCode, String)> {
    debug!("관심종목 그룹 목록 조회");

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    let watchlists = WatchlistRepository::get_all_watchlists(pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("관심종목 조회 실패: {}", e),
            )
        })?;

    let total = watchlists.len();

    Ok(Json(WatchlistListResponse { watchlists, total }))
}

/// POST /api/v1/watchlist - 새 관심종목 그룹 생성
#[utoipa::path(
    post,
    path = "/api/v1/watchlist",
    request_body = NewWatchlist,
    responses(
        (status = 200, description = "생성된 관심종목 그룹", body = WatchlistRecord),
        (status = 409, description = "중복된 그룹 이름"),
        (status = 500, description = "서버 오류")
    ),
    tag = "watchlist"
)]
pub async fn create_watchlist(
    State(state): State<Arc<AppState>>,
    Json(input): Json<NewWatchlist>,
) -> Result<Json<WatchlistRecord>, (StatusCode, String)> {
    info!("관심종목 그룹 생성: {}", input.name);

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    let watchlist = WatchlistRepository::create_watchlist(pool, input)
        .await
        .map_err(|e| {
            if e.to_string().contains("unique_watchlist_name") {
                (
                    StatusCode::CONFLICT,
                    "이미 존재하는 그룹 이름입니다".to_string(),
                )
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("그룹 생성 실패: {}", e),
                )
            }
        })?;

    Ok(Json(watchlist))
}

/// GET /api/v1/watchlist/:id - 그룹 상세 조회
#[utoipa::path(
    get,
    path = "/api/v1/watchlist/{id}",
    params(
        ("id" = Uuid, Path, description = "관심종목 그룹 ID")
    ),
    responses(
        (status = 200, description = "관심종목 그룹 상세", body = WatchlistDetailResponse),
        (status = 404, description = "그룹 없음"),
        (status = 500, description = "서버 오류")
    ),
    tag = "watchlist"
)]
pub async fn get_watchlist_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<WatchlistDetailResponse>, (StatusCode, String)> {
    debug!("관심종목 그룹 상세 조회: {}", id);

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    // 그룹 정보 조회
    let watchlist = WatchlistRepository::get_watchlist_by_id(pool, id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("조회 실패: {}", e),
            )
        })?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "그룹을 찾을 수 없습니다".to_string()))?;

    // 아이템 조회
    let items = WatchlistRepository::get_items_by_watchlist(pool, id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("아이템 조회 실패: {}", e),
            )
        })?;

    let item_count = items.len();

    Ok(Json(WatchlistDetailResponse {
        watchlist,
        items,
        item_count,
    }))
}

/// DELETE /api/v1/watchlist/:id - 그룹 삭제
#[utoipa::path(
    delete,
    path = "/api/v1/watchlist/{id}",
    params(
        ("id" = Uuid, Path, description = "관심종목 그룹 ID")
    ),
    responses(
        (status = 200, description = "삭제 성공", body = SuccessResponse),
        (status = 404, description = "그룹 없음"),
        (status = 500, description = "서버 오류")
    ),
    tag = "watchlist"
)]
pub async fn delete_watchlist(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<SuccessResponse>, (StatusCode, String)> {
    info!("관심종목 그룹 삭제: {}", id);

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    let deleted = WatchlistRepository::delete_watchlist(pool, id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("삭제 실패: {}", e),
            )
        })?;

    if !deleted {
        return Err((StatusCode::NOT_FOUND, "그룹을 찾을 수 없습니다".to_string()));
    }

    Ok(Json(SuccessResponse {
        success: true,
        message: "그룹이 삭제되었습니다".to_string(),
    }))
}

/// POST /api/v1/watchlist/:id/items - 아이템 추가
#[utoipa::path(
    post,
    path = "/api/v1/watchlist/{id}/items",
    params(
        ("id" = Uuid, Path, description = "관심종목 그룹 ID")
    ),
    request_body = AddItemsRequest,
    responses(
        (status = 200, description = "추가된 아이템 목록", body = AddItemsResponse),
        (status = 404, description = "그룹 없음"),
        (status = 500, description = "서버 오류")
    ),
    tag = "watchlist"
)]
pub async fn add_items(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(request): Json<AddItemsRequest>,
) -> Result<Json<AddItemsResponse>, (StatusCode, String)> {
    info!("관심종목 아이템 추가: {} ({}개)", id, request.items.len());

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    // 그룹 존재 여부 확인
    let watchlist = WatchlistRepository::get_watchlist_by_id(pool, id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("조회 실패: {}", e),
            )
        })?;

    if watchlist.is_none() {
        return Err((StatusCode::NOT_FOUND, "그룹을 찾을 수 없습니다".to_string()));
    }

    let added = WatchlistRepository::add_items_batch(pool, id, request.items)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("아이템 추가 실패: {}", e),
            )
        })?;

    let count = added.len();

    Ok(Json(AddItemsResponse { added, count }))
}

/// DELETE /api/v1/watchlist/:id/items/:symbol - 아이템 삭제
#[utoipa::path(
    delete,
    path = "/api/v1/watchlist/{id}/items/{item_id}",
    params(
        ("id" = Uuid, Path, description = "관심종목 그룹 ID"),
        ("item_id" = String, Path, description = "종목 심볼")
    ),
    responses(
        (status = 200, description = "삭제 성공", body = SuccessResponse),
        (status = 404, description = "아이템 없음"),
        (status = 500, description = "서버 오류")
    ),
    tag = "watchlist"
)]
pub async fn remove_item(
    State(state): State<Arc<AppState>>,
    Path((id, symbol)): Path<(Uuid, String)>,
    Query(query): Query<DeleteItemQuery>,
) -> Result<Json<SuccessResponse>, (StatusCode, String)> {
    info!(
        "관심종목 아이템 삭제: {} - {} ({})",
        id, symbol, query.market
    );

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    let removed = WatchlistRepository::remove_item(pool, id, &symbol, &query.market)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("삭제 실패: {}", e),
            )
        })?;

    if !removed {
        return Err((
            StatusCode::NOT_FOUND,
            "아이템을 찾을 수 없습니다".to_string(),
        ));
    }

    Ok(Json(SuccessResponse {
        success: true,
        message: format!("{} 종목이 삭제되었습니다", symbol),
    }))
}

/// PUT /api/v1/watchlist/items/:item_id - 아이템 수정
#[utoipa::path(
    put,
    path = "/api/v1/watchlist/{id}/items/{item_id}",
    params(
        ("id" = Uuid, Path, description = "관심종목 그룹 ID"),
        ("item_id" = Uuid, Path, description = "아이템 ID")
    ),
    request_body = UpdateWatchlistItem,
    responses(
        (status = 200, description = "수정된 아이템", body = WatchlistItemRecord),
        (status = 404, description = "아이템 없음"),
        (status = 500, description = "서버 오류")
    ),
    tag = "watchlist"
)]
pub async fn update_item(
    State(state): State<Arc<AppState>>,
    Path(item_id): Path<Uuid>,
    Json(input): Json<UpdateWatchlistItem>,
) -> Result<Json<WatchlistItemRecord>, (StatusCode, String)> {
    debug!("관심종목 아이템 수정: {}", item_id);

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    let updated = WatchlistRepository::update_item(pool, item_id, input)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("수정 실패: {}", e),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "아이템을 찾을 수 없습니다".to_string(),
            )
        })?;

    Ok(Json(updated))
}

/// GET /api/v1/watchlist/symbol/:symbol - 특정 종목이 포함된 그룹 조회
#[utoipa::path(
    get,
    path = "/api/v1/watchlist/search",
    params(
        ("symbol" = String, Path, description = "종목 심볼"),
        ("market" = String, Query, description = "시장 (기본값: KR)")
    ),
    responses(
        (status = 200, description = "종목이 포함된 그룹 목록", body = Vec<WatchlistRecord>),
        (status = 500, description = "서버 오류")
    ),
    tag = "watchlist"
)]
pub async fn find_symbol_in_watchlists(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(query): Query<DeleteItemQuery>,
) -> Result<Json<Vec<WatchlistRecord>>, (StatusCode, String)> {
    debug!("종목 포함 그룹 조회: {} ({})", symbol, query.market);

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    let watchlists =
        WatchlistRepository::find_watchlists_containing_symbol(pool, &symbol, &query.market)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("조회 실패: {}", e),
                )
            })?;

    Ok(Json(watchlists))
}

// ================================================================================================
// Router
// ================================================================================================

/// Watchlist 라우터 생성
pub fn watchlist_router() -> Router<Arc<AppState>> {
    Router::new()
        // 그룹 관리
        .route("/", get(list_watchlists).post(create_watchlist))
        .route("/{id}", get(get_watchlist_detail).delete(delete_watchlist))
        // 아이템 관리
        .route("/{id}/items", post(add_items))
        .route("/{id}/items/{symbol}", delete(remove_item))
        .route("/items/{item_id}", put(update_item))
        // 종목 검색
        .route("/symbol/{symbol}", get(find_symbol_in_watchlists))
}
