//! Ranking API 라우트
//!
//! GlobalScore 기반 종목 랭킹 및 7Factor 분석 API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `POST /api/v1/ranking/global` - 모든 심볼 GlobalScore 계산
//! - `GET /api/v1/ranking/top` - 상위 랭킹 조회
//! - `GET /api/v1/ranking/7factor/{ticker}` - 7Factor 데이터 조회
//! - `POST /api/v1/ranking/7factor/batch` - 7Factor 일괄 조회

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};
use ts_rs::TS;
use utoipa::{IntoParams, ToSchema};

use crate::repository::{
    GlobalScoreRepository, RankedSymbol, RankingFilter, ScoreHistoryRepository,
    ScoreHistorySummary, SevenFactorResponse,
};
use crate::state::AppState;

// ================================================================================================
// Request/Response Types
// ================================================================================================

/// GlobalScore 계산 응답
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "ranking/")]
pub struct CalculateResponse {
    /// 처리된 종목 수
    pub processed: i32,
    /// 계산 시작 시간
    pub started_at: String,
    /// 계산 완료 시간
    pub completed_at: String,
}

/// 랭킹 조회 쿼리
#[derive(Debug, Deserialize, ToSchema, IntoParams, TS)]
#[ts(export, export_to = "ranking/")]
pub struct RankingQuery {
    /// 시장 필터 (KR, US 등)
    #[serde(default)]
    pub market: Option<String>,

    /// 등급 필터 (BUY, WATCH 등)
    #[serde(default)]
    pub grade: Option<String>,

    /// 최소 점수
    #[serde(default)]
    pub min_score: Option<String>,

    /// 반환 개수 (기본 50, 최대 500)
    /// NOTE: i32 사용 (ts-rs에서 number로 변환, bigint 대신)
    #[serde(default)]
    pub limit: Option<i32>,

    /// 오프셋 (무한 스크롤용)
    /// NOTE: i32 사용 (ts-rs에서 number로 변환, bigint 대신)
    #[serde(default)]
    pub offset: Option<i32>,

    /// RouteState 필터 (ATTACK, ARMED, WATCH, REST)
    #[serde(default)]
    pub route_state: Option<String>,
}

/// 랭킹 조회 응답
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "ranking/")]
pub struct RankingResponse {
    /// 종목 목록
    pub symbols: Vec<RankedSymbol>,
    /// 총 개수
    pub total: usize,
    /// 필터 정보
    pub filter: FilterInfo,
}

/// 필터 정보
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "ranking/")]
pub struct FilterInfo {
    pub market: Option<String>,
    pub grade: Option<String>,
    pub min_score: Option<String>,
    /// NOTE: i32 사용 (ts-rs에서 number로 변환, bigint 대신)
    pub limit: i32,
    /// NOTE: i32 사용 (ts-rs에서 number로 변환, bigint 대신)
    pub offset: i32,
    pub route_state: Option<String>,
}

/// 7Factor 조회 쿼리
#[derive(Debug, Deserialize, ToSchema, IntoParams, TS)]
#[ts(export, export_to = "ranking/")]
pub struct SevenFactorQuery {
    /// 시장 (기본: KR)
    #[serde(default = "default_market")]
    pub market: String,
}

fn default_market() -> String {
    "KR".to_string()
}

/// 7Factor 일괄 조회 요청
#[derive(Debug, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "ranking/")]
pub struct SevenFactorBatchRequest {
    /// 티커 목록
    pub tickers: Vec<String>,
    /// 시장 (기본: KR)
    #[serde(default = "default_market")]
    pub market: String,
}

/// 7Factor 일괄 조회 응답
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "ranking/")]
pub struct SevenFactorBatchResponse {
    /// 7Factor 데이터 목록
    pub factors: Vec<SevenFactorResponse>,
    /// 총 개수
    pub total: usize,
}

// ================================================================================================
// Handlers
// ================================================================================================

/// POST /api/v1/ranking/global - 모든 심볼 GlobalScore 계산
#[utoipa::path(
    post,
    path = "/api/v1/ranking/global",
    tag = "ranking",
    responses(
        (status = 200, description = "계산 완료", body = CalculateResponse),
        (status = 500, description = "서버 에러")
    )
)]
pub async fn calculate_global(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CalculateResponse>, (StatusCode, String)> {
    let started_at = chrono::Utc::now();

    info!("GlobalScore 계산 요청 수신");

    // DB 연결 확인
    let db_pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    // 공유 data_provider 사용 (Redis 3계층 캐시 포함)
    let data_provider = state.data_provider.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Data provider not available".to_string(),
        )
    })?;

    let processed = GlobalScoreRepository::calculate_all(db_pool, data_provider)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("GlobalScore 계산 실패: {}", e),
            )
        })?;

    let completed_at = chrono::Utc::now();

    info!("GlobalScore 계산 완료: {} 종목", processed);

    Ok(Json(CalculateResponse {
        processed,
        started_at: started_at.to_rfc3339(),
        completed_at: completed_at.to_rfc3339(),
    }))
}

/// GET /api/v1/ranking/top - 상위 랭킹 조회
#[utoipa::path(
    get,
    path = "/api/v1/ranking/top",
    tag = "ranking",
    params(RankingQuery),
    responses(
        (status = 200, description = "랭킹 목록", body = RankingResponse),
        (status = 500, description = "서버 에러")
    )
)]
pub async fn get_top_ranked(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RankingQuery>,
) -> Result<Json<RankingResponse>, (StatusCode, String)> {
    debug!("랭킹 조회 요청: {:?}", query);

    // DB 연결 확인
    let db_pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    // 공유 data_provider 사용 (Redis 3계층 캐시 포함)
    let data_provider = state.data_provider.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Data provider not available".to_string(),
        )
    })?;

    // min_score 파싱
    let min_score = query
        .min_score
        .as_ref()
        .and_then(|s| s.parse::<Decimal>().ok());

    let filter = RankingFilter {
        market: query.market.clone(),
        grade: query.grade.clone(),
        min_score,
        limit: query.limit.map(|l| l as i64),
        offset: query.offset.map(|o| o as i64),
        route_state: query.route_state.clone(),
    };

    let symbols = GlobalScoreRepository::get_top_ranked(db_pool, data_provider, filter)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("랭킹 조회 실패: {}", e),
            )
        })?;

    let total = symbols.len();

    Ok(Json(RankingResponse {
        symbols,
        total,
        filter: FilterInfo {
            market: query.market,
            grade: query.grade,
            min_score: query.min_score,
            limit: query.limit.unwrap_or(50),
            offset: query.offset.unwrap_or(0),
            route_state: query.route_state,
        },
    }))
}

/// GET /api/v1/ranking/7factor/{ticker} - 특정 종목의 7Factor 데이터 조회
#[utoipa::path(
    get,
    path = "/api/v1/ranking/7factor/{ticker}",
    tag = "ranking",
    params(
        ("ticker" = String, Path, description = "종목 티커"),
        SevenFactorQuery
    ),
    responses(
        (status = 200, description = "7Factor 데이터", body = SevenFactorResponse),
        (status = 404, description = "종목 없음"),
        (status = 500, description = "서버 에러")
    )
)]
pub async fn get_seven_factor(
    State(state): State<Arc<AppState>>,
    Path(ticker): Path<String>,
    Query(query): Query<SevenFactorQuery>,
) -> Result<Json<SevenFactorResponse>, (StatusCode, String)> {
    debug!("7Factor 조회: {} ({})", ticker, query.market);

    let db_pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    // 공유 data_provider 사용 (Redis 3계층 캐시 포함)
    let data_provider = state.data_provider.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Data provider not available".to_string(),
        )
    })?;

    let result = GlobalScoreRepository::get_seven_factor(db_pool, data_provider, &ticker, &query.market)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("7Factor 조회 실패: {}", e),
            )
        })?;

    result
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("종목을 찾을 수 없습니다: {}", ticker),
            )
        })
        .map(Json)
}

/// POST /api/v1/ranking/7factor/batch - 7Factor 일괄 조회
#[utoipa::path(
    post,
    path = "/api/v1/ranking/7factor/batch",
    tag = "ranking",
    request_body = SevenFactorBatchRequest,
    responses(
        (status = 200, description = "7Factor 일괄 데이터", body = SevenFactorBatchResponse),
        (status = 500, description = "서버 에러")
    )
)]
pub async fn get_seven_factor_batch(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SevenFactorBatchRequest>,
) -> Result<Json<SevenFactorBatchResponse>, (StatusCode, String)> {
    debug!(
        "7Factor 일괄 조회: {:?} ({})",
        request.tickers, request.market
    );

    let db_pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    // 공유 data_provider 사용 (Redis 3계층 캐시 포함)
    let data_provider = state.data_provider.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Data provider not available".to_string(),
        )
    })?;

    let factors = GlobalScoreRepository::get_seven_factor_batch(
        db_pool,
        data_provider,
        &request.tickers,
        &request.market,
    )
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("7Factor 일괄 조회 실패: {}", e),
                )
            })?;

    let total = factors.len();

    Ok(Json(SevenFactorBatchResponse { factors, total }))
}

// ================================================================================================
// Score History Types
// ================================================================================================

/// Score History 조회 쿼리
#[derive(Debug, Deserialize, ToSchema, IntoParams, TS)]
#[ts(export, export_to = "ranking/")]
pub struct ScoreHistoryQuery {
    /// 조회 일수 (기본 90, 최대 365)
    #[serde(default = "default_history_days")]
    pub days: i32,
}

fn default_history_days() -> i32 {
    90
}

/// Score History 응답
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "ranking/")]
pub struct ScoreHistoryResponse {
    /// 종목 코드
    pub symbol: String,
    /// 히스토리 데이터
    pub history: Vec<ScoreHistorySummary>,
    /// 총 레코드 수
    pub total: usize,
}

// ================================================================================================
// Score History Handlers
// ================================================================================================

/// 종목별 Score History 조회
///
/// # 경로
///
/// `GET /api/v1/ranking/history/{ticker}?days=90`
#[utoipa::path(
    get,
    path = "/api/v1/ranking/history/{ticker}",
    params(
        ("ticker" = String, Path, description = "종목 코드"),
        ScoreHistoryQuery
    ),
    responses(
        (status = 200, description = "Score History 조회 성공", body = ScoreHistoryResponse)
    ),
    tag = "ranking"
)]
pub async fn get_score_history(
    State(state): State<Arc<AppState>>,
    Path(ticker): Path<String>,
    Query(query): Query<ScoreHistoryQuery>,
) -> Result<Json<ScoreHistoryResponse>, (StatusCode, String)> {
    debug!("Score History 조회: {} ({}일)", ticker, query.days);

    let db_pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    let days = query.days.clamp(1, 365);

    let history = ScoreHistoryRepository::get_with_change(db_pool, &ticker, days)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Score History 조회 실패: {}", e),
            )
        })?;

    let total = history.len();

    Ok(Json(ScoreHistoryResponse {
        symbol: ticker,
        history,
        total,
    }))
}

// ================================================================================================
// Router
// ================================================================================================

/// Ranking 라우터 생성
pub fn ranking_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/global", post(calculate_global))
        .route("/top", get(get_top_ranked))
        .route("/7factor/{ticker}", get(get_seven_factor))
        .route("/7factor/batch", post(get_seven_factor_batch))
        .route("/history/{ticker}", get(get_score_history))
}
