//! Dataset 관리 endpoint.
//!
//! 과거 데이터 캐시 조회, 다운로드 요청, 삭제 등을 관리합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/dataset` - 캐시된 데이터셋 목록 조회
//! - `POST /api/v1/dataset/fetch` - 새 데이터셋 다운로드 요청
//! - `GET /api/v1/dataset/search` - 심볼 검색
//! - `POST /api/v1/dataset/symbols/batch` - 여러 티커의 심볼 정보 일괄 조회
//! - `GET /api/v1/dataset/:symbol` - 특정 심볼의 캔들 데이터 조회
//! - `DELETE /api/v1/dataset/:symbol` - 특정 심볼 캐시 삭제
//! - `DELETE /api/v1/dataset/symbols/:ticker` - 심볼 삭제 + 연쇄 정리
//! - `POST /api/v1/dataset/symbols/cleanup-orphans` - 고아 데이터 일괄 정리

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error};
use utoipa::{IntoParams, ToSchema};

use trader_core::Timeframe;

use crate::repository::{CascadeDeleteResult, SymbolInfoRepository, SymbolSearchResult};
use crate::routes::strategies::ApiError;
use crate::state::AppState;

// ==================== 응답 타입 ====================

/// 캐시된 데이터셋 요약.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DatasetSummary {
    /// 심볼
    pub symbol: String,
    /// 표시 이름 (예: "005930(삼성전자)")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 타임프레임
    pub timeframe: String,
    /// 첫 번째 캔들 시간
    pub first_time: Option<DateTime<Utc>>,
    /// 마지막 캔들 시간
    pub last_time: Option<DateTime<Utc>>,
    /// 총 캔들 수
    pub candle_count: i64,
    /// 마지막 업데이트 시간
    pub last_updated: Option<DateTime<Utc>>,
}

/// 데이터셋 목록 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DatasetListResponse {
    pub datasets: Vec<DatasetSummary>,
    pub total_count: usize,
}

/// 캔들 데이터 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CandleDataResponse {
    pub symbol: String,
    pub timeframe: String,
    pub candles: Vec<CandleItem>,
    pub total_count: usize,
}

/// 단일 캔들.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CandleItem {
    pub time: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
}

/// 데이터셋 다운로드 요청.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FetchDatasetRequest {
    /// 심볼 (예: 005930, AAPL)
    pub symbol: String,
    /// 타임프레임 (1d, 1h, 5m 등)
    #[serde(default = "default_timeframe")]
    pub timeframe: String,
    /// 다운로드할 캔들 수 (start_date/end_date가 없을 때 사용)
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// 시작 날짜 (ISO 8601, 예: 2024-01-01)
    #[serde(default)]
    pub start_date: Option<String>,
    /// 종료 날짜 (ISO 8601, 예: 2024-12-31)
    #[serde(default)]
    pub end_date: Option<String>,
}

fn default_timeframe() -> String {
    "1d".to_string()
}

fn default_limit() -> usize {
    500
}

/// 데이터셋 다운로드 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FetchDatasetResponse {
    pub symbol: String,
    pub timeframe: String,
    pub fetched_count: usize,
    pub message: String,
}

/// 정렬 컬럼.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SortColumn {
    #[default]
    Time,
    Open,
    High,
    Low,
    Close,
    Volume,
}

impl SortColumn {
    /// DB 컬럼명으로 변환.
    fn to_db_column(self) -> &'static str {
        match self {
            Self::Time => "open_time",
            Self::Open => "open",
            Self::High => "high",
            Self::Low => "low",
            Self::Close => "close",
            Self::Volume => "volume",
        }
    }
}

/// 정렬 방향.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SortOrder {
    Asc,
    #[default]
    Desc,
}

impl SortOrder {
    fn to_sql(self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }
}

/// 캔들 조회 쿼리.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct CandleQuery {
    /// 타임프레임 (기본: 1d)
    #[serde(default = "default_timeframe")]
    pub timeframe: String,
    /// 조회할 캔들 수 (기본: 100)
    #[serde(default = "default_candle_limit")]
    pub limit: usize,
    /// 페이지 (기본: 0)
    #[serde(default)]
    pub page: usize,
    /// 정렬 컬럼 (기본: time)
    #[serde(default)]
    pub sort_by: SortColumn,
    /// 정렬 방향 (기본: desc)
    #[serde(default)]
    pub sort_order: SortOrder,
}

fn default_candle_limit() -> usize {
    100
}

/// 삭제 쿼리.
#[derive(Debug, Deserialize, IntoParams)]
pub struct DeleteQuery {
    /// 특정 타임프레임만 삭제 (선택사항)
    pub timeframe: Option<String>,
}

// ==================== Handler ====================

/// 캐시된 데이터셋 목록 조회.
#[utoipa::path(
    get,
    path = "/api/v1/dataset",
    tag = "dataset",
    responses(
        (status = 200, description = "데이터셋 목록 조회 성공", body = DatasetListResponse),
        (status = 500, description = "서버 오류", body = ApiError),
        (status = 503, description = "DB 연결 없음", body = ApiError)
    )
)]
pub async fn list_datasets(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DatasetListResponse>, (StatusCode, Json<ApiError>)> {
    // 공유 data_provider 사용 (Redis 3계층 캐시 포함)
    let provider = state.data_provider.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "DB_NOT_AVAILABLE",
                "데이터베이스 연결이 없습니다",
            )),
        )
    })?;

    match provider.get_cache_stats().await {
        Ok(stats) => {
            // 심볼 목록 추출 (Yahoo 형식에서 canonical로 변환)
            let symbols: Vec<String> = stats
                .iter()
                .map(|s| {
                    // .KS, .KQ 등 Yahoo 접미사 제거
                    if s.symbol.ends_with(".KS") || s.symbol.ends_with(".KQ") {
                        s.symbol[..s.symbol.len() - 3].to_string()
                    } else {
                        s.symbol.clone()
                    }
                })
                .collect();

            // display_name 배치 조회
            let display_names = state.get_display_names(&symbols, false).await;

            let datasets: Vec<DatasetSummary> = stats
                .into_iter()
                .map(|s| {
                    // Yahoo 형식에서 canonical 심볼로 변환
                    let canonical = if s.symbol.ends_with(".KS") || s.symbol.ends_with(".KQ") {
                        s.symbol[..s.symbol.len() - 3].to_string()
                    } else {
                        s.symbol.clone()
                    };
                    let display_name = display_names.get(&canonical).cloned();
                    DatasetSummary {
                        symbol: canonical,
                        display_name,
                        timeframe: s.timeframe,
                        first_time: s.first_time,
                        last_time: s.last_time,
                        candle_count: s.candle_count,
                        last_updated: s.last_updated,
                    }
                })
                .collect();

            let total_count = datasets.len();

            debug!(count = total_count, "데이터셋 목록 조회 성공");

            Ok(Json(DatasetListResponse {
                datasets,
                total_count,
            }))
        }
        Err(e) => {
            error!(error = %e, "데이터셋 목록 조회 실패");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "CACHE_STATS_ERROR",
                    format!("캐시 통계 조회 실패: {}", e),
                )),
            ))
        }
    }
}

/// 새 데이터셋 다운로드 요청.
#[utoipa::path(
    post,
    path = "/api/v1/dataset/fetch",
    tag = "dataset",
    request_body = FetchDatasetRequest,
    responses(
        (status = 200, description = "데이터셋 다운로드 성공", body = FetchDatasetResponse),
        (status = 400, description = "잘못된 요청", body = ApiError),
        (status = 502, description = "외부 API 오류", body = ApiError),
        (status = 503, description = "DB 연결 없음", body = ApiError)
    )
)]
pub async fn fetch_dataset(
    State(state): State<Arc<AppState>>,
    Json(req): Json<FetchDatasetRequest>,
) -> Result<Json<FetchDatasetResponse>, (StatusCode, Json<ApiError>)> {
    // 공유 data_provider 사용 (Redis 3계층 캐시 포함)
    let provider = state.data_provider.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "DB_NOT_AVAILABLE",
                "데이터베이스 연결이 없습니다",
            )),
        )
    })?;

    let timeframe = parse_timeframe(&req.timeframe);

    debug!(
        symbol = %req.symbol,
        timeframe = %req.timeframe,
        limit = req.limit,
        start_date = ?req.start_date,
        end_date = ?req.end_date,
        "데이터셋 다운로드 요청"
    );

    // 날짜 범위가 지정된 경우 날짜 범위 API 사용
    let result = if req.start_date.is_some() && req.end_date.is_some() {
        let (start_date, end_date) = parse_date_range(&req)?;
        provider
            .get_klines_range(&req.symbol, timeframe, start_date, end_date)
            .await
    } else if req.start_date.is_some() {
        // 시작 날짜만 있으면: 시작일 ~ 오늘
        let start_date = parse_start_date(&req)?;
        let end_date = Utc::now().date_naive();
        provider
            .get_klines_range(&req.symbol, timeframe, start_date, end_date)
            .await
    } else {
        // 날짜 범위 없으면 기존 방식 (최근 N개)
        provider.get_klines(&req.symbol, timeframe, req.limit).await
    };

    match result {
        Ok(klines) => {
            let fetched_count = klines.len();
            let message = if req.start_date.is_some() || req.end_date.is_some() {
                format!(
                    "{}개 캔들 데이터가 캐시되었습니다 (기간: {} ~ {})",
                    fetched_count,
                    req.start_date.as_deref().unwrap_or("처음"),
                    req.end_date.as_deref().unwrap_or("오늘")
                )
            } else {
                format!("{}개 캔들 데이터가 캐시되었습니다", fetched_count)
            };

            debug!(
                symbol = %req.symbol,
                count = fetched_count,
                "데이터셋 다운로드 완료"
            );

            Ok(Json(FetchDatasetResponse {
                symbol: req.symbol,
                timeframe: req.timeframe,
                fetched_count,
                message,
            }))
        }
        Err(e) => {
            error!(
                symbol = %req.symbol,
                error = %e,
                "데이터셋 다운로드 실패"
            );
            Err((
                StatusCode::BAD_GATEWAY,
                Json(ApiError::new(
                    "FETCH_ERROR",
                    format!("데이터 다운로드 실패: {}", e),
                )),
            ))
        }
    }
}

/// 날짜 범위 파싱.
fn parse_date_range(
    req: &FetchDatasetRequest,
) -> Result<(NaiveDate, NaiveDate), (StatusCode, Json<ApiError>)> {
    let start = req.start_date.as_ref().unwrap();
    let end = req.end_date.as_ref().unwrap();

    let start_date = NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_START_DATE",
                format!("시작 날짜 형식 오류 (YYYY-MM-DD): {}", e),
            )),
        )
    })?;

    let end_date = NaiveDate::parse_from_str(end, "%Y-%m-%d").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_END_DATE",
                format!("종료 날짜 형식 오류 (YYYY-MM-DD): {}", e),
            )),
        )
    })?;

    if start_date > end_date {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_DATE_RANGE",
                "시작 날짜가 종료 날짜보다 늦습니다",
            )),
        ));
    }

    Ok((start_date, end_date))
}

/// 시작 날짜 파싱.
fn parse_start_date(req: &FetchDatasetRequest) -> Result<NaiveDate, (StatusCode, Json<ApiError>)> {
    let start = req.start_date.as_ref().unwrap();

    NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_START_DATE",
                format!("시작 날짜 형식 오류 (YYYY-MM-DD): {}", e),
            )),
        )
    })
}

#[allow(dead_code)]
/// 날짜 범위에서 유효한 limit 계산 (사용하지 않음).
fn calculate_effective_limit(
    req: &FetchDatasetRequest,
) -> Result<usize, (StatusCode, Json<ApiError>)> {
    match (&req.start_date, &req.end_date) {
        (Some(start), Some(end)) => {
            let start_date = NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "INVALID_START_DATE",
                        format!("시작 날짜 형식 오류 (YYYY-MM-DD): {}", e),
                    )),
                )
            })?;
            let end_date = NaiveDate::parse_from_str(end, "%Y-%m-%d").map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "INVALID_END_DATE",
                        format!("종료 날짜 형식 오류 (YYYY-MM-DD): {}", e),
                    )),
                )
            })?;

            if start_date > end_date {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "INVALID_DATE_RANGE",
                        "시작 날짜가 종료 날짜보다 늦습니다",
                    )),
                ));
            }

            let days = (end_date - start_date).num_days() as usize;
            // 최소 1일, 여유분 포함
            Ok(days.max(1) + 30)
        }
        (Some(start), None) => {
            let start_date = NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "INVALID_START_DATE",
                        format!("시작 날짜 형식 오류 (YYYY-MM-DD): {}", e),
                    )),
                )
            })?;
            let today = Utc::now().date_naive();
            let days = (today - start_date).num_days() as usize;
            Ok(days.max(1) + 30)
        }
        (None, Some(end)) => {
            // 종료 날짜만 지정된 경우, 기본 limit 사용하되 최대 범위 제한
            let end_date = NaiveDate::parse_from_str(end, "%Y-%m-%d").map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "INVALID_END_DATE",
                        format!("종료 날짜 형식 오류 (YYYY-MM-DD): {}", e),
                    )),
                )
            })?;
            let _ = end_date; // 유효성 검사만 수행
            Ok(req.limit)
        }
        (None, None) => Ok(req.limit),
    }
}

/// 특정 심볼의 캔들 데이터 조회 (정렬 지원).
#[utoipa::path(
    get,
    path = "/api/v1/dataset/{symbol}",
    tag = "dataset",
    params(
        ("symbol" = String, Path, description = "심볼 (예: 005930, AAPL)"),
        CandleQuery
    ),
    responses(
        (status = 200, description = "캔들 데이터 조회 성공", body = CandleDataResponse),
        (status = 404, description = "심볼 없음", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError),
        (status = 502, description = "외부 API 오류", body = ApiError),
        (status = 503, description = "DB 연결 없음", body = ApiError)
    )
)]
pub async fn get_candles(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(query): Query<CandleQuery>,
) -> Result<Json<CandleDataResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "DB_NOT_AVAILABLE",
                "데이터베이스 연결이 없습니다",
            )),
        )
    })?;

    // DB에서 yahoo_symbol 조회 (Single Source of Truth)
    let symbol_info = SymbolInfoRepository::get_by_ticker(pool, &symbol, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("심볼 조회 실패: {}", e))),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiError::new(
                    "SYMBOL_NOT_FOUND",
                    format!("심볼을 찾을 수 없습니다: {}", symbol),
                )),
            )
        })?;

    // OHLCV 테이블은 ticker로 조회 (yahoo_symbol은 API 호출 시에만 사용)
    let ticker = &symbol_info.ticker;
    let tf_str = timeframe_to_db_string(&query.timeframe);

    // 정렬 조건이 기본값(time desc)이 아닌 경우 DB 직접 쿼리
    // 기본값인 경우 캐시 제공자 사용 (캐시 업데이트 로직 활용)
    if query.sort_by == SortColumn::Time && query.sort_order == SortOrder::Desc {
        // 기본 정렬: 공유 data_provider 사용 (Redis 3계층 캐시 포함)
        let timeframe = parse_timeframe(&query.timeframe);
        let provider = state.data_provider.as_ref().ok_or_else(|| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiError::new("DB_NOT_AVAILABLE", "데이터베이스 연결이 없습니다")),
            )
        })?;
        let total_limit = (query.page + 1) * query.limit;

        // 전체 개수를 DB에서 조회 (캐시 제공자는 limit만큼만 반환하므로)
        let total_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ohlcv WHERE symbol = $1 AND timeframe = $2")
                .bind(ticker)
                .bind(&tf_str)
                .fetch_one(pool)
                .await
                .unwrap_or(0);

        match provider.get_klines(&symbol, timeframe, total_limit).await {
            Ok(klines) => {
                let start = query.page * query.limit;

                // 최신순 정렬 (desc)
                let mut sorted_klines = klines;
                sorted_klines.sort_by(|a, b| b.open_time.cmp(&a.open_time));

                let candles: Vec<CandleItem> = sorted_klines
                    .into_iter()
                    .skip(start)
                    .take(query.limit)
                    .map(|k| CandleItem {
                        time: k.open_time.format("%Y-%m-%d %H:%M:%S").to_string(),
                        open: k.open.to_string(),
                        high: k.high.to_string(),
                        low: k.low.to_string(),
                        close: k.close.to_string(),
                        volume: k.volume.to_string(),
                    })
                    .collect();

                debug!(
                    symbol = %symbol,
                    timeframe = %query.timeframe,
                    sort_by = "time",
                    sort_order = "desc",
                    page = query.page,
                    returned = candles.len(),
                    total = total_count,
                    "캔들 데이터 조회 성공 (캐시)"
                );

                return Ok(Json(CandleDataResponse {
                    symbol,
                    timeframe: query.timeframe,
                    candles,
                    total_count: total_count as usize,
                }));
            }
            Err(e) => {
                error!(symbol = %symbol, error = %e, "캔들 데이터 조회 실패");
                return Err((
                    StatusCode::BAD_GATEWAY,
                    Json(ApiError::new(
                        "FETCH_ERROR",
                        format!("캔들 데이터 조회 실패: {}", e),
                    )),
                ));
            }
        }
    }

    // 커스텀 정렬: DB 직접 쿼리
    // Window Function으로 데이터와 총 개수를 단일 쿼리로 조회 (2 → 1 쿼리)
    let sort_column = query.sort_by.to_db_column();
    let sort_order = query.sort_order.to_sql();
    let offset = query.page * query.limit;

    // 동적 ORDER BY로 데이터 조회 (COUNT(*) OVER() 포함)
    // 보안: sort_column은 enum에서 변환되므로 SQL 인젝션 위험 없음
    let data_query = format!(
        r#"
        SELECT open_time, open, high, low, close, volume,
               COUNT(*) OVER() as total_count
        FROM ohlcv
        WHERE symbol = $1 AND timeframe = $2
        ORDER BY {} {}
        LIMIT $3 OFFSET $4
        "#,
        sort_column, sort_order
    );

    #[derive(sqlx::FromRow)]
    struct OhlcvRow {
        open_time: DateTime<Utc>,
        open: rust_decimal::Decimal,
        high: rust_decimal::Decimal,
        low: rust_decimal::Decimal,
        close: rust_decimal::Decimal,
        volume: rust_decimal::Decimal,
        total_count: i64,
    }

    let rows: Vec<OhlcvRow> = sqlx::query_as(&data_query)
        .bind(ticker)
        .bind(&tf_str)
        .bind(query.limit as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!(error = %e, "캔들 데이터 조회 실패");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("데이터 조회 실패: {}", e),
                )),
            )
        })?;

    // 첫 번째 행에서 총 개수 추출 (Window Function 결과)
    // 결과가 없으면 total_count = 0
    let total_count = rows.first().map(|r| r.total_count).unwrap_or(0);

    let candles: Vec<CandleItem> = rows
        .into_iter()
        .map(|r| CandleItem {
            time: r.open_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            open: r.open.to_string(),
            high: r.high.to_string(),
            low: r.low.to_string(),
            close: r.close.to_string(),
            volume: r.volume.to_string(),
        })
        .collect();

    debug!(
        symbol = %symbol,
        timeframe = %query.timeframe,
        sort_by = %sort_column,
        sort_order = %sort_order,
        page = query.page,
        returned = candles.len(),
        total = total_count,
        "캔들 데이터 조회 성공 (DB 직접)"
    );

    Ok(Json(CandleDataResponse {
        symbol,
        timeframe: query.timeframe,
        candles,
        total_count: total_count as usize,
    }))
}

/// 특정 심볼 캐시 삭제.
#[utoipa::path(
    delete,
    path = "/api/v1/dataset/{symbol}",
    tag = "dataset",
    params(
        ("symbol" = String, Path, description = "심볼 (예: 005930, AAPL)"),
        DeleteQuery
    ),
    responses(
        (status = 200, description = "캐시 삭제 성공"),
        (status = 404, description = "심볼 없음", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError),
        (status = 503, description = "DB 연결 없음", body = ApiError)
    )
)]
pub async fn delete_dataset(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(query): Query<DeleteQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "DB_NOT_AVAILABLE",
                "데이터베이스 연결이 없습니다",
            )),
        )
    })?;

    // DB에서 yahoo_symbol 조회 (Single Source of Truth)
    let symbol_info = SymbolInfoRepository::get_by_ticker(pool, &symbol, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("심볼 조회 실패: {}", e))),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiError::new(
                    "SYMBOL_NOT_FOUND",
                    format!("심볼을 찾을 수 없습니다: {}", symbol),
                )),
            )
        })?;

    // OHLCV 테이블은 ticker로 조회 (yahoo_symbol은 API 호출 시에만 사용)
    let ticker = &symbol_info.ticker;

    debug!(
        symbol = %ticker,
        timeframe = ?query.timeframe,
        "데이터셋 삭제 요청"
    );

    let deleted = if let Some(ref tf) = query.timeframe {
        // 특정 타임프레임만 삭제
        sqlx::query("DELETE FROM ohlcv WHERE symbol = $1 AND timeframe = $2")
            .bind(ticker)
            .bind(tf)
            .execute(pool)
            .await
            .map_err(|e| {
                error!(error = %e, "캐시 삭제 실패");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new("DELETE_ERROR", format!("삭제 실패: {}", e))),
                )
            })?
            .rows_affected()
    } else {
        // 모든 타임프레임 삭제
        sqlx::query("DELETE FROM ohlcv WHERE symbol = $1")
            .bind(ticker)
            .execute(pool)
            .await
            .map_err(|e| {
                error!(error = %e, "캐시 삭제 실패");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new("DELETE_ERROR", format!("삭제 실패: {}", e))),
                )
            })?
            .rows_affected()
    };

    // 메타데이터도 삭제
    if let Some(ref tf) = query.timeframe {
        let _ = sqlx::query("DELETE FROM ohlcv_metadata WHERE symbol = $1 AND timeframe = $2")
            .bind(ticker)
            .bind(tf)
            .execute(pool)
            .await;
    } else {
        let _ = sqlx::query("DELETE FROM ohlcv_metadata WHERE symbol = $1")
            .bind(ticker)
            .execute(pool)
            .await;
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "symbol": symbol,
        "deleted_count": deleted,
        "message": format!("{}개 캔들 데이터가 삭제되었습니다", deleted)
    })))
}

// ==================== 헬퍼 함수 ====================

/// 타임프레임 문자열을 Timeframe enum으로 변환.
fn parse_timeframe(tf: &str) -> Timeframe {
    match tf.to_lowercase().as_str() {
        "1m" => Timeframe::M1,
        "3m" => Timeframe::M3,
        "5m" => Timeframe::M5,
        "15m" => Timeframe::M15,
        "30m" => Timeframe::M30,
        "1h" => Timeframe::H1,
        "2h" => Timeframe::H2,
        "4h" => Timeframe::H4,
        "6h" => Timeframe::H6,
        "8h" => Timeframe::H8,
        "12h" => Timeframe::H12,
        "1d" | "d" => Timeframe::D1,
        "3d" => Timeframe::D3,
        "1w" | "w" => Timeframe::W1,
        "1M" | "M" | "1mn" | "mn" => Timeframe::MN1,
        _ => Timeframe::D1, // 기본값: 일봉
    }
}

/// 타임프레임 문자열을 DB 저장 형식으로 변환.
fn timeframe_to_db_string(tf: &str) -> String {
    match tf.to_lowercase().as_str() {
        "1m" => "1m",
        "3m" => "3m",
        "5m" => "5m",
        "15m" => "15m",
        "30m" => "30m",
        "1h" => "1h",
        "2h" => "2h",
        "4h" => "4h",
        "6h" => "6h",
        "8h" => "8h",
        "12h" => "12h",
        "1d" | "d" => "1d",
        "3d" => "3d",
        "1w" | "w" => "1wk",
        "1M" | "M" | "1mn" | "mn" => "1mo",
        _ => "1d",
    }
    .to_string()
}

// ==================== 심볼 검색 ====================

/// 심볼 검색 요청 파라미터.
#[derive(Debug, Deserialize, IntoParams)]
pub struct SymbolSearchQuery {
    /// 검색어 (티커 또는 회사명)
    pub q: String,
    /// 최대 결과 수 (기본: 10)
    #[serde(default = "default_search_limit")]
    pub limit: i64,
}

fn default_search_limit() -> i64 {
    10
}

/// 심볼 검색 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SymbolSearchResponse {
    pub results: Vec<SymbolSearchResult>,
    pub total: usize,
}

/// 심볼 검색 API.
///
/// 티커 코드와 회사명으로 검색합니다.
/// DB에 없는 종목은 KRX/Yahoo Finance에서 자동으로 조회하여 저장합니다.
#[utoipa::path(
    get,
    path = "/api/v1/dataset/search",
    tag = "dataset",
    params(SymbolSearchQuery),
    responses(
        (status = 200, description = "심볼 검색 성공", body = SymbolSearchResponse),
        (status = 400, description = "잘못된 검색어", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError),
        (status = 503, description = "DB 연결 없음", body = ApiError)
    )
)]
pub async fn search_symbols(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SymbolSearchQuery>,
) -> Result<Json<SymbolSearchResponse>, (StatusCode, Json<ApiError>)> {
    if params.q.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                code: "INVALID_QUERY".to_string(),
                message: "검색어가 필요합니다".to_string(),
            }),
        ));
    }

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError {
                code: "DB_UNAVAILABLE".to_string(),
                message: "데이터베이스를 사용할 수 없습니다".to_string(),
            }),
        )
    })?;

    // DB에서 먼저 검색
    let mut results = SymbolInfoRepository::search(pool, &params.q, params.limit)
        .await
        .map_err(|e| {
            error!("심볼 검색 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    code: "SEARCH_ERROR".to_string(),
                    message: format!("검색 실패: {}", e),
                }),
            )
        })?;

    // DB에서 결과가 없고, 검색어가 티커 형식인 경우 외부 API에서 조회
    if results.is_empty() {
        let query_trimmed = params.q.trim();
        let looks_like_ticker = query_trimmed.len() <= 10
            && query_trimmed
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '-');

        if looks_like_ticker {
            debug!(
                ticker = query_trimmed,
                "DB에 없는 티커, 외부 API에서 조회 시도"
            );

            match SymbolInfoRepository::get_or_fetch(pool, query_trimmed).await {
                Ok(Some(result)) => {
                    results.push(result);
                }
                Ok(None) => {
                    // 외부 API에서도 찾지 못함
                }
                Err(e) => {
                    // 외부 조회 실패 로그만 남기고 빈 결과 반환
                    error!("외부 API 조회 실패: {}", e);
                }
            }
        }
    }

    let total = results.len();

    Ok(Json(SymbolSearchResponse { results, total }))
}

// ==================== 심볼 배치 조회 ====================

/// 심볼 배치 조회 요청.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SymbolBatchRequest {
    /// 조회할 티커 목록 (최대 100개)
    pub tickers: Vec<String>,
}

/// 심볼 배치 조회 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SymbolBatchResponse {
    /// 심볼 정보 목록
    pub symbols: Vec<SymbolSearchResult>,
    /// 조회된 심볼 수
    pub total: usize,
}

/// 여러 티커의 심볼 정보 일괄 조회.
#[utoipa::path(
    post,
    path = "/api/v1/dataset/symbols/batch",
    tag = "dataset",
    request_body = SymbolBatchRequest,
    responses(
        (status = 200, description = "심볼 배치 조회 성공", body = SymbolBatchResponse),
        (status = 400, description = "잘못된 요청 (티커 없거나 100개 초과)", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError),
        (status = 503, description = "DB 연결 없음", body = ApiError)
    )
)]
pub async fn get_symbols_batch(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SymbolBatchRequest>,
) -> Result<Json<SymbolBatchResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "DB_NOT_CONFIGURED",
                "데이터베이스가 설정되지 않았습니다",
            )),
        )
    })?;

    // 최대 100개 제한
    if request.tickers.len() > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "TOO_MANY_TICKERS",
                "한 번에 최대 100개의 티커만 조회할 수 있습니다",
            )),
        ));
    }

    // 빈 요청 처리
    if request.tickers.is_empty() {
        return Ok(Json(SymbolBatchResponse {
            symbols: vec![],
            total: 0,
        }));
    }

    let results = SymbolInfoRepository::get_by_tickers(pool, &request.tickers)
        .await
        .map_err(|e| {
            error!("심볼 배치 조회 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "QUERY_ERROR",
                    format!("심볼 조회 실패: {}", e),
                )),
            )
        })?;

    let total = results.len();
    Ok(Json(SymbolBatchResponse {
        symbols: results,
        total,
    }))
}

// ==================== CSV 동기화 기능 제거됨 ====================
// KRX, EOD CSV 동기화는 trader-collector로 이동되었습니다.
// API 엔드포인트로 제공되던 기능은 CLI 도구를 통해 수행할 수 있습니다.
//   cargo run --bin trader-collector -- sync-symbols

// ==================== 라우터 ====================

/// Dataset 라우터 생성.
pub fn dataset_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_datasets))
        .route("/fetch", post(fetch_dataset))
        .route("/search", get(search_symbols))
        // 심볼 배치 조회
        .route("/symbols/batch", post(get_symbols_batch))
        // 심볼 상태 관리 (실패/비활성화)
        .route("/symbols/failed", get(get_failed_symbols))
        .route("/symbols/stats", get(get_symbol_stats))
        .route("/symbols/reactivate", post(reactivate_symbols))
        // 심볼 무결성 관리
        .route("/symbols/{ticker}", delete(delete_symbol))
        .route("/symbols/cleanup-orphans", post(cleanup_orphans))
        // 심볼별 조회/삭제
        .route("/{symbol}", get(get_candles))
        .route("/{symbol}", delete(delete_dataset))
}

// ==================== 심볼 상태 관리 ====================

/// 실패 심볼 목록 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FailedSymbolsResponse {
    /// 실패한 심볼 목록.
    pub symbols: Vec<FailedSymbolDto>,
    /// 총 개수.
    pub total_count: usize,
}

/// 실패 심볼 정보 DTO.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FailedSymbolDto {
    pub id: String,
    pub ticker: String,
    pub name: String,
    pub market: String,
    pub yahoo_symbol: Option<String>,
    pub is_active: bool,
    pub fail_count: i32,
    pub last_error: Option<String>,
    pub last_attempt: Option<String>,
    /// 실패 레벨: CRITICAL (3+), WARNING (2), MINOR (1).
    pub failure_level: String,
}

/// 심볼 통계 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SymbolStatsResponse {
    /// 비활성화된 심볼 수.
    pub deactivated: i64,
    /// 임계 상태 (3회+ 실패, 아직 활성).
    pub critical: i64,
    /// 경고 상태 (1-2회 실패, 아직 활성).
    pub warning: i64,
}

/// 심볼 재활성화 요청.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReactivateSymbolsRequest {
    /// 재활성화할 심볼 ID 목록.
    pub symbol_ids: Vec<String>,
}

/// 심볼 재활성화 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReactivateSymbolsResponse {
    pub success: bool,
    /// 재활성화된 심볼 수.
    pub reactivated_count: u64,
    pub message: String,
}

/// 실패 심볼 목록 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct FailedSymbolsQuery {
    /// 최소 실패 횟수 (기본: 1).
    #[serde(default = "default_min_failures")]
    pub min_failures: i32,
}

fn default_min_failures() -> i32 {
    1
}

/// 실패 심볼 목록 조회.
#[utoipa::path(
    get,
    path = "/api/v1/dataset/symbols/failed",
    tag = "dataset",
    params(FailedSymbolsQuery),
    responses(
        (status = 200, description = "실패 심볼 목록 조회 성공", body = FailedSymbolsResponse),
        (status = 500, description = "서버 오류", body = ApiError),
        (status = 503, description = "DB 연결 없음", body = ApiError)
    )
)]
async fn get_failed_symbols(
    State(state): State<Arc<AppState>>,
    Query(params): Query<FailedSymbolsQuery>,
) -> Result<Json<FailedSymbolsResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "DB_NOT_AVAILABLE",
                "데이터베이스 연결이 없습니다",
            )),
        )
    })?;

    match SymbolInfoRepository::get_failed_symbols(pool, params.min_failures).await {
        Ok(symbols) => {
            let total_count = symbols.len();
            let symbols_dto: Vec<FailedSymbolDto> = symbols
                .into_iter()
                .map(|s| {
                    let fail_count = s.fetch_fail_count.unwrap_or(0);
                    let failure_level = match fail_count {
                        c if c >= 3 => "CRITICAL",
                        2 => "WARNING",
                        1 => "MINOR",
                        _ => "OK",
                    };

                    FailedSymbolDto {
                        id: s.id.to_string(),
                        ticker: s.ticker,
                        name: s.name,
                        market: s.market,
                        yahoo_symbol: s.yahoo_symbol,
                        is_active: s.is_active.unwrap_or(true),
                        fail_count,
                        last_error: s.last_fetch_error,
                        last_attempt: s.last_fetch_attempt.map(|t| t.to_rfc3339()),
                        failure_level: failure_level.to_string(),
                    }
                })
                .collect();

            Ok(Json(FailedSymbolsResponse {
                symbols: symbols_dto,
                total_count,
            }))
        }
        Err(e) => {
            error!(error = %e, "실패 심볼 목록 조회 실패");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", e.to_string())),
            ))
        }
    }
}

/// 심볼 통계 조회.
#[utoipa::path(
    get,
    path = "/api/v1/dataset/symbols/stats",
    tag = "dataset",
    responses(
        (status = 200, description = "심볼 통계 조회 성공", body = SymbolStatsResponse),
        (status = 500, description = "서버 오류", body = ApiError),
        (status = 503, description = "DB 연결 없음", body = ApiError)
    )
)]
async fn get_symbol_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SymbolStatsResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "DB_NOT_AVAILABLE",
                "데이터베이스 연결이 없습니다",
            )),
        )
    })?;

    match SymbolInfoRepository::get_deactivated_stats(pool).await {
        Ok(stats) => Ok(Json(SymbolStatsResponse {
            deactivated: stats.deactivated,
            critical: stats.critical,
            warning: stats.warning,
        })),
        Err(e) => {
            error!(error = %e, "심볼 통계 조회 실패");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", e.to_string())),
            ))
        }
    }
}

/// 심볼 재활성화.
#[utoipa::path(
    post,
    path = "/api/v1/dataset/symbols/reactivate",
    tag = "dataset",
    request_body = ReactivateSymbolsRequest,
    responses(
        (status = 200, description = "심볼 재활성화 성공", body = ReactivateSymbolsResponse),
        (status = 400, description = "잘못된 요청", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError),
        (status = 503, description = "DB 연결 없음", body = ApiError)
    )
)]
async fn reactivate_symbols(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ReactivateSymbolsRequest>,
) -> Result<Json<ReactivateSymbolsResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "DB_NOT_AVAILABLE",
                "데이터베이스 연결이 없습니다",
            )),
        )
    })?;

    // UUID 파싱
    let symbol_ids: Result<Vec<uuid::Uuid>, _> = request
        .symbol_ids
        .iter()
        .map(|id| uuid::Uuid::parse_str(id))
        .collect();

    let symbol_ids = match symbol_ids {
        Ok(ids) => ids,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "INVALID_UUID",
                    format!("잘못된 UUID 형식: {}", e),
                )),
            ));
        }
    };

    if symbol_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("EMPTY_REQUEST", "심볼 ID가 필요합니다")),
        ));
    }

    match SymbolInfoRepository::reactivate_symbols(pool, &symbol_ids).await {
        Ok(count) => {
            debug!(count = count, "심볼 재활성화 완료");

            // SymbolResolver 캐시 클리어
            state.clear_symbol_cache().await;

            Ok(Json(ReactivateSymbolsResponse {
                success: true,
                reactivated_count: count,
                message: format!("{}개 심볼 재활성화 완료", count),
            }))
        }
        Err(e) => {
            error!(error = %e, "심볼 재활성화 실패");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", e.to_string())),
            ))
        }
    }
}

// ==================== 심볼 무결성 관리 ====================

/// 심볼 삭제 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams)]
pub struct DeleteSymbolQuery {
    /// 시장 코드 (KR, US, CRYPTO 등)
    pub market: String,
}

/// 심볼 삭제 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSymbolResponse {
    pub success: bool,
    pub ticker: String,
    pub market: String,
    pub deleted_tables: Vec<CascadeDeleteResult>,
    pub message: String,
}

/// 고아 데이터 정리 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CleanupOrphansResponse {
    pub success: bool,
    pub cleaned_tables: Vec<CascadeDeleteResult>,
    pub total_deleted: i64,
    pub message: String,
}

/// 심볼 삭제 + 연쇄 정리.
///
/// symbol_info 레코드와 관련된 모든 데이터(ohlcv, 포지션, 체결 등)를 삭제합니다.
/// DELETE /api/v1/dataset/symbols/:ticker?market=KR
#[utoipa::path(
    delete,
    path = "/api/v1/dataset/symbols/{ticker}",
    params(
        ("ticker" = String, Path, description = "삭제할 심볼 티커"),
        DeleteSymbolQuery,
    ),
    responses(
        (status = 200, description = "심볼 삭제 완료", body = DeleteSymbolResponse),
        (status = 404, description = "심볼을 찾을 수 없음"),
        (status = 500, description = "서버 오류"),
    ),
    tag = "dataset"
)]
pub async fn delete_symbol(
    State(state): State<Arc<AppState>>,
    Path(ticker): Path<String>,
    Query(query): Query<DeleteSymbolQuery>,
) -> Result<Json<DeleteSymbolResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("DB_NOT_CONFIGURED", "DB 미설정")),
        )
    })?;

    match SymbolInfoRepository::delete_symbol(pool, &ticker, &query.market).await {
        Ok(results) => {
            // SymbolResolver 캐시 클리어
            state.clear_symbol_cache().await;

            let total: i64 = results.iter().map(|r| r.deleted_count).sum();
            Ok(Json(DeleteSymbolResponse {
                success: true,
                ticker: ticker.clone(),
                market: query.market.clone(),
                deleted_tables: results,
                message: format!("심볼 '{}' ({}) 삭제 완료 (총 {}건)", ticker, query.market, total),
            }))
        }
        Err(e) => {
            let err_msg = e.to_string();
            if err_msg.contains("symbol_info not found") {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(ApiError::new("SYMBOL_NOT_FOUND", format!(
                        "심볼을 찾을 수 없습니다: ticker={}, market={}", ticker, query.market
                    ))),
                ))
            } else {
                error!(error = %e, ticker = %ticker, "심볼 삭제 실패");
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new("DB_ERROR", e.to_string())),
                ))
            }
        }
    }
}

/// 고아 데이터 일괄 정리.
///
/// symbol_info에 존재하지 않는 심볼의 데이터를 모든 관련 테이블에서 삭제합니다.
/// POST /api/v1/dataset/symbols/cleanup-orphans
#[utoipa::path(
    post,
    path = "/api/v1/dataset/symbols/cleanup-orphans",
    responses(
        (status = 200, description = "고아 데이터 정리 완료", body = CleanupOrphansResponse),
        (status = 500, description = "서버 오류"),
    ),
    tag = "dataset"
)]
pub async fn cleanup_orphans(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CleanupOrphansResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("DB_NOT_CONFIGURED", "DB 미설정")),
        )
    })?;

    match SymbolInfoRepository::cleanup_orphans(pool).await {
        Ok(results) => {
            let total: i64 = results.iter().map(|r| r.deleted_count).sum();
            Ok(Json(CleanupOrphansResponse {
                success: true,
                cleaned_tables: results,
                total_deleted: total,
                message: format!("고아 데이터 정리 완료 (총 {}건 삭제)", total),
            }))
        }
        Err(e) => {
            error!(error = %e, "고아 데이터 정리 실패");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", e.to_string())),
            ))
        }
    }
}
