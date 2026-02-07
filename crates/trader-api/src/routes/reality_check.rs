//! Reality Check API 라우트
//!
//! 전일 추천 종목의 익일 실제 성과 검증 API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/reality-check/stats` - 통계 조회 (일별/소스별/랭크별)
//! - `GET /api/v1/reality-check/results` - 검증 결과 조회 (기간 필터)
//! - `GET /api/v1/reality-check/snapshots` - 스냅샷 조회
//! - `POST /api/v1/reality-check/snapshot` - 스냅샷 저장 (내부용)
//! - `POST /api/v1/reality-check/calculate` - Reality Check 계산 (내부용)

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use utoipa::{IntoParams, ToSchema};

use crate::repository::{
    CalculationResult, DailyStats, PriceSnapshot, RankStats, RealityCheckRecord,
    RealityCheckRepository, SnapshotInput, SourceStats,
};
use crate::state::AppState;

// ==================== Request/Response 타입 ====================

/// 통계 조회 요청
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct StatsQuery {
    /// 일별 통계 조회 개수 (기본값: 30)
    #[serde(default = "default_limit")]
    pub limit: i32,
}

fn default_limit() -> i32 {
    30
}

/// 통합 통계 응답
#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = RealityCheckStatsResponse)]
pub struct StatsResponse {
    /// 일별 통계
    pub daily: Vec<DailyStats>,
    /// 소스별 통계
    pub source: Vec<SourceStats>,
    /// 랭크별 통계
    pub rank: Vec<RankStats>,
}

/// 검증 결과 조회 요청
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct ResultsQuery {
    /// 시작 날짜 (YYYY-MM-DD)
    pub start_date: Option<String>,
    /// 종료 날짜 (YYYY-MM-DD)
    pub end_date: Option<String>,
    /// 추천 소스 필터
    pub recommend_source: Option<String>,
}

/// 검증 결과 응답
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ResultsResponse {
    pub total: usize,
    pub results: Vec<RealityCheckRecord>,
}

/// 스냅샷 조회 요청
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct SnapshotsQuery {
    /// 조회 날짜 (YYYY-MM-DD, 기본값: 오늘)
    pub snapshot_date: Option<String>,
}

/// 스냅샷 응답
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SnapshotsResponse {
    pub snapshot_date: String,
    pub total: usize,
    pub snapshots: Vec<PriceSnapshot>,
}

/// 스냅샷 저장 요청
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SaveSnapshotRequest {
    /// 스냅샷 날짜 (선택, 기본값: 오늘)
    pub snapshot_date: Option<String>,
    /// 스냅샷 데이터 배열
    pub snapshots: Vec<SnapshotInput>,
}

/// 스냅샷 저장 응답
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SaveSnapshotResponse {
    pub success: bool,
    pub snapshot_date: String,
    pub saved_count: usize,
}

/// Reality Check 계산 요청
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CalculateRequest {
    /// 추천 날짜 (YYYY-MM-DD, 기본값: 어제)
    pub recommend_date: Option<String>,
    /// 검증 날짜 (YYYY-MM-DD, 기본값: 오늘)
    pub check_date: Option<String>,
}

/// Reality Check 계산 응답
#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = RealityCheckCalculateResponse)]
pub struct CalculateResponse {
    pub success: bool,
    pub recommend_date: String,
    pub check_date: String,
    pub processed_count: usize,
    pub results: Vec<CalculationResult>,
}

// ==================== 라우터 ====================

/// Reality Check API 라우터 생성
pub fn reality_check_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/stats", get(get_stats))
        .route("/results", get(get_results))
        .route("/snapshots", get(get_snapshots))
        .route("/snapshot", post(save_snapshot))
        .route("/calculate", post(calculate_reality_check))
}

// ==================== 핸들러 ====================

/// 통계 조회 (일별/소스별/랭크별)
///
/// GET /api/v1/reality-check/stats?limit=30
#[utoipa::path(
    get,
    path = "/api/v1/reality-check/stats",
    params(StatsQuery),
    responses(
        (status = 200, description = "통계 조회 성공", body = StatsResponse),
        (status = 500, description = "서버 오류")
    ),
    tag = "reality-check"
)]
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatsQuery>,
) -> impl IntoResponse {
    debug!("GET /stats (limit: {})", query.limit);

    // 세 가지 통계를 병렬로 조회
    let daily_result = RealityCheckRepository::get_daily_stats(
        state.db_pool.as_ref().expect("DB pool not initialized"),
        query.limit,
    );
    let source_result = RealityCheckRepository::get_source_stats(
        state.db_pool.as_ref().expect("DB pool not initialized"),
    );
    let rank_result = RealityCheckRepository::get_rank_stats(
        state.db_pool.as_ref().expect("DB pool not initialized"),
    );

    let (daily, source, rank) = tokio::join!(daily_result, source_result, rank_result);

    match (daily, source, rank) {
        (Ok(daily_stats), Ok(source_stats), Ok(rank_stats)) => {
            info!(
                "Stats fetched: {} daily, {} sources, {} ranks",
                daily_stats.len(),
                source_stats.len(),
                rank_stats.len()
            );

            (
                StatusCode::OK,
                Json(StatsResponse {
                    daily: daily_stats,
                    source: source_stats,
                    rank: rank_stats,
                }),
            )
        }
        _ => {
            error!("Failed to fetch stats");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(StatsResponse {
                    daily: vec![],
                    source: vec![],
                    rank: vec![],
                }),
            )
        }
    }
}

/// 검증 결과 조회
///
/// GET /api/v1/reality-check/results?start_date=2025-01-01&end_date=2025-01-31&recommend_source=screening_momentum
#[utoipa::path(
    get,
    path = "/api/v1/reality-check/results",
    params(ResultsQuery),
    responses(
        (status = 200, description = "결과 조회 성공", body = ResultsResponse),
        (status = 400, description = "잘못된 요청"),
        (status = 500, description = "서버 오류")
    ),
    tag = "reality-check"
)]
pub async fn get_results(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ResultsQuery>,
) -> impl IntoResponse {
    debug!("GET /results: {:?}", query);

    // 날짜 파싱 (기본값: 최근 30일)
    let end_date = query
        .end_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| Utc::now().naive_utc().date());

    let start_date = query
        .start_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| end_date - chrono::Duration::days(30));

    if start_date > end_date {
        warn!("Invalid date range: {} > {}", start_date, end_date);
        return (
            StatusCode::BAD_REQUEST,
            Json(ResultsResponse {
                total: 0,
                results: vec![],
            }),
        );
    }

    let recommend_source = query.recommend_source.as_deref();

    match RealityCheckRepository::get_reality_checks(
        state.db_pool.as_ref().expect("DB pool not initialized"),
        start_date,
        end_date,
        recommend_source,
    )
    .await
    {
        Ok(results) => {
            info!(
                "Fetched {} reality check results from {} to {}",
                results.len(),
                start_date,
                end_date
            );

            (
                StatusCode::OK,
                Json(ResultsResponse {
                    total: results.len(),
                    results,
                }),
            )
        }
        Err(e) => {
            error!("Failed to fetch results: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResultsResponse {
                    total: 0,
                    results: vec![],
                }),
            )
        }
    }
}

/// 스냅샷 조회
///
/// GET /api/v1/reality-check/snapshots?snapshot_date=2025-02-01
#[utoipa::path(
    get,
    path = "/api/v1/reality-check/snapshots",
    params(SnapshotsQuery),
    responses(
        (status = 200, description = "스냅샷 조회 성공", body = SnapshotsResponse),
        (status = 500, description = "서버 오류")
    ),
    tag = "reality-check"
)]
pub async fn get_snapshots(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SnapshotsQuery>,
) -> impl IntoResponse {
    debug!("GET /snapshots: {:?}", query);

    let snapshot_date = query
        .snapshot_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| Utc::now().naive_utc().date());

    match RealityCheckRepository::get_snapshots(
        state.db_pool.as_ref().expect("DB pool not initialized"),
        snapshot_date,
    )
    .await
    {
        Ok(snapshots) => {
            info!(
                "Fetched {} snapshots for {}",
                snapshots.len(),
                snapshot_date
            );

            (
                StatusCode::OK,
                Json(SnapshotsResponse {
                    snapshot_date: snapshot_date.to_string(),
                    total: snapshots.len(),
                    snapshots,
                }),
            )
        }
        Err(e) => {
            error!("Failed to fetch snapshots: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SnapshotsResponse {
                    snapshot_date: snapshot_date.to_string(),
                    total: 0,
                    snapshots: vec![],
                }),
            )
        }
    }
}

/// 스냅샷 저장 (내부용)
///
/// POST /api/v1/reality-check/snapshot
///
/// **참고**: 이 엔드포인트는 일일 배치 작업 또는 스크리닝 결과 저장 시 사용됩니다.
#[utoipa::path(
    post,
    path = "/api/v1/reality-check/snapshot",
    request_body = SaveSnapshotRequest,
    responses(
        (status = 200, description = "스냅샷 저장 성공", body = SaveSnapshotResponse),
        (status = 400, description = "잘못된 요청"),
        (status = 500, description = "서버 오류")
    ),
    tag = "reality-check"
)]
pub async fn save_snapshot(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SaveSnapshotRequest>,
) -> impl IntoResponse {
    debug!("POST /snapshot: {} snapshots", request.snapshots.len());

    if request.snapshots.is_empty() {
        warn!("Empty snapshots array");
        return (
            StatusCode::BAD_REQUEST,
            Json(SaveSnapshotResponse {
                success: false,
                snapshot_date: "".to_string(),
                saved_count: 0,
            }),
        );
    }

    let snapshot_date = request
        .snapshot_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| Utc::now().naive_utc().date());

    match RealityCheckRepository::save_snapshots_batch(
        state.db_pool.as_ref().expect("DB pool not initialized"),
        snapshot_date,
        &request.snapshots,
    )
    .await
    {
        Ok(saved_count) => {
            info!("Saved {} snapshots for {}", saved_count, snapshot_date);

            (
                StatusCode::OK,
                Json(SaveSnapshotResponse {
                    success: true,
                    snapshot_date: snapshot_date.to_string(),
                    saved_count,
                }),
            )
        }
        Err(e) => {
            error!("Failed to save snapshots: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SaveSnapshotResponse {
                    success: false,
                    snapshot_date: snapshot_date.to_string(),
                    saved_count: 0,
                }),
            )
        }
    }
}

/// Reality Check 계산 (내부용)
///
/// POST /api/v1/reality-check/calculate
///
/// **참고**: 이 엔드포인트는 익일 장 마감 후 자동으로 실행되어
/// 전일 추천 종목의 실제 성과를 계산합니다.
#[utoipa::path(
    post,
    path = "/api/v1/reality-check/calculate",
    request_body = CalculateRequest,
    responses(
        (status = 200, description = "계산 성공", body = CalculateResponse),
        (status = 400, description = "잘못된 요청"),
        (status = 500, description = "서버 오류")
    ),
    tag = "reality-check"
)]
pub async fn calculate_reality_check(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CalculateRequest>,
) -> impl IntoResponse {
    debug!("POST /calculate: {:?}", request);

    let today = Utc::now().naive_utc().date();
    let yesterday = today - chrono::Duration::days(1);

    let check_date = request
        .check_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(today);

    let recommend_date = request
        .recommend_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(yesterday);

    if recommend_date >= check_date {
        warn!(
            "Invalid date pair: recommend_date {} >= check_date {}",
            recommend_date, check_date
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(CalculateResponse {
                success: false,
                recommend_date: recommend_date.to_string(),
                check_date: check_date.to_string(),
                processed_count: 0,
                results: vec![],
            }),
        );
    }

    match RealityCheckRepository::calculate_reality_check(
        state.db_pool.as_ref().expect("DB pool not initialized"),
        recommend_date,
        check_date,
    )
    .await
    {
        Ok(results) => {
            info!(
                "Reality check calculated: {} results for {} -> {}",
                results.len(),
                recommend_date,
                check_date
            );

            (
                StatusCode::OK,
                Json(CalculateResponse {
                    success: true,
                    recommend_date: recommend_date.to_string(),
                    check_date: check_date.to_string(),
                    processed_count: results.len(),
                    results,
                }),
            )
        }
        Err(e) => {
            error!("Failed to calculate reality check: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CalculateResponse {
                    success: false,
                    recommend_date: recommend_date.to_string(),
                    check_date: check_date.to_string(),
                    processed_count: 0,
                    results: vec![],
                }),
            )
        }
    }
}
