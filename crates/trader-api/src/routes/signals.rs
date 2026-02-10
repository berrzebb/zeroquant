//! SignalMarker API 라우트
//!
//! 백테스트 및 실거래에서 발생한 기술 신호를 조회하고 검색합니다.
//! 시그널 생성 시 텔레그램 알림 전송 기능을 포함합니다.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tracing::{debug, info, warn};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::error::ApiErrorResponse;
use crate::repository::{
    BacktestResultsRepository, SignalMarkerRepository, SignalPerformanceRepository,
    SignalPerformanceResponse, SignalReturnPoint, SignalSymbolStats,
};
use crate::AppState;
use trader_core::{Side, SignalIndicators, SignalMarker, SignalType};

// ==================== Request/Response 타입 ====================

/// 지표 기반 검색 요청
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SignalSearchRequest {
    /// 지표 필터 (JSONB 쿼리)
    ///
    /// # 예시
    /// ```json
    /// {
    ///   "rsi": {"$gte": 70.0},
    ///   "macd": {"$gt": 0}
    /// }
    /// ```
    pub indicator_filter: JsonValue,

    /// 신호 유형 필터 (선택)
    #[serde(default)]
    pub signal_type: Option<String>,

    /// 최대 결과 개수 (기본 100, 최대 1000)
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    100
}

/// 심볼별 신호 조회 요청
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct SymbolSignalsQuery {
    /// 심볼 (예: "005930")
    pub symbol: String,

    /// 거래소 (예: "KRX")
    pub exchange: String,

    /// 시작 시각 (ISO 8601)
    #[serde(default)]
    pub start_time: Option<DateTime<Utc>>,

    /// 종료 시각 (ISO 8601)
    #[serde(default)]
    pub end_time: Option<DateTime<Utc>>,

    /// 최대 결과 개수
    #[serde(default = "default_limit")]
    pub limit: i64,
}

/// 전략별 신호 조회 요청
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct StrategySignalsQuery {
    /// 전략 ID
    pub strategy_id: String,

    /// 시작 시각 (ISO 8601)
    #[serde(default)]
    pub start_time: Option<DateTime<Utc>>,

    /// 종료 시각 (ISO 8601)
    #[serde(default)]
    pub end_time: Option<DateTime<Utc>>,

    /// 최대 결과 개수
    #[serde(default = "default_limit")]
    pub limit: i64,
}

/// 시그널 생성 요청
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateSignalRequest {
    /// 심볼 (예: "005930", "BTC/USDT")
    pub symbol: String,

    /// 신호 유형 ("Entry", "Exit", "Alert", "Scale")
    pub signal_type: String,

    /// 방향 ("Buy", "Sell")
    #[serde(default)]
    pub side: Option<String>,

    /// 신호 발생 가격
    pub price: String,

    /// 신호 강도 (0.0 ~ 1.0)
    #[serde(default = "default_strength")]
    pub strength: f64,

    /// 신호 이유 (사람이 읽을 수 있는 설명)
    pub reason: String,

    /// 전략 ID
    pub strategy_id: String,

    /// 전략 이름
    pub strategy_name: String,

    /// 지표 정보 (선택)
    #[serde(default)]
    pub indicators: Option<JsonValue>,

    /// 알림 전송 여부 (기본: true)
    #[serde(default = "default_notify")]
    pub notify: bool,
}

fn default_strength() -> f64 {
    0.7
}

fn default_notify() -> bool {
    true
}

/// 시그널 생성 응답
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreateSignalResponse {
    /// 생성된 시그널 ID
    pub id: String,

    /// 심볼
    pub symbol: String,

    /// 신호 유형
    pub signal_type: String,

    /// 알림 전송 여부
    pub notified: bool,

    /// 알림 전송 결과 메시지
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_result: Option<String>,
}

/// 신호 마커 응답 DTO
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SignalMarkerDto {
    /// 신호 ID
    pub id: String,

    /// 심볼
    pub symbol: String,

    /// 타임스탬프
    pub timestamp: DateTime<Utc>,

    /// 신호 유형
    pub signal_type: String,

    /// 방향 (Buy/Sell)
    pub side: Option<String>,

    /// 가격
    pub price: String,

    /// 신호 강도 (0.0 ~ 1.0)
    pub strength: f64,

    /// 지표 정보
    pub indicators: SignalIndicators,

    /// 신호 이유
    pub reason: String,

    /// 전략 ID
    pub strategy_id: String,

    /// 전략 이름
    pub strategy_name: String,

    /// 실행 여부
    pub executed: bool,
}

impl From<SignalMarker> for SignalMarkerDto {
    fn from(marker: SignalMarker) -> Self {
        Self {
            id: marker.id.to_string(),
            symbol: marker.ticker.to_string(),
            timestamp: marker.timestamp,
            signal_type: marker.signal_type.to_string(),
            side: marker.side.map(|s| s.to_string()),
            price: marker.price.to_string(),
            strength: marker.strength,
            indicators: marker.indicators,
            reason: marker.reason,
            strategy_id: marker.strategy_id,
            strategy_name: marker.strategy_name,
            executed: marker.executed,
        }
    }
}

/// 신호 검색 응답
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SignalSearchResponse {
    /// 총 결과 수
    pub total: usize,

    /// 신호 목록
    pub signals: Vec<SignalMarkerDto>,
}

// ==================== API 핸들러 ====================

/// 지표 기반 신호 검색
///
/// JSONB 쿼리를 사용하여 특정 지표 조건을 만족하는 신호를 검색합니다.
///
/// # 지원 연산자
/// - `$gte`: >=
/// - `$lte`: <=
/// - `$gt`: >
/// - `$lt`: <
/// - `$eq`: =
#[utoipa::path(
    post,
    path = "/api/v1/signals/search",
    request_body = SignalSearchRequest,
    responses(
        (status = 200, description = "검색 성공", body = SignalSearchResponse),
        (status = 400, description = "잘못된 요청", body = ApiErrorResponse),
        (status = 500, description = "서버 오류", body = ApiErrorResponse)
    ),
    tag = "signals"
)]
pub async fn search_signals(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SignalSearchRequest>,
) -> Result<Json<SignalSearchResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new(
                    "DATABASE_ERROR",
                    "Database not available",
                )),
            ))
        }
    };

    let repo = SignalMarkerRepository::new(db_pool.clone());

    let markers = repo
        .search_by_indicator(
            req.indicator_filter,
            req.signal_type.as_deref(),
            Some(req.limit),
        )
        .await?;

    let total = markers.len();
    let signals = markers.into_iter().map(SignalMarkerDto::from).collect();

    Ok(Json(SignalSearchResponse { total, signals }))
}

/// 심볼별 신호 조회
#[utoipa::path(
    get,
    path = "/api/v1/signals/by-symbol",
    params(SymbolSignalsQuery),
    responses(
        (status = 200, description = "조회 성공", body = SignalSearchResponse),
        (status = 400, description = "잘못된 요청", body = ApiErrorResponse),
        (status = 500, description = "서버 오류", body = ApiErrorResponse)
    ),
    tag = "signals"
)]
pub async fn get_signals_by_symbol(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SymbolSignalsQuery>,
) -> Result<Json<SignalSearchResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new(
                    "DATABASE_ERROR",
                    "Database not available",
                )),
            ))
        }
    };

    let repo = SignalMarkerRepository::new(db_pool.clone());

    let markers = repo
        .find_by_symbol(
            &query.symbol,
            &query.exchange,
            query.start_time,
            query.end_time,
            Some(query.limit),
        )
        .await?;

    let total = markers.len();
    let signals = markers.into_iter().map(SignalMarkerDto::from).collect();

    Ok(Json(SignalSearchResponse { total, signals }))
}

/// 전략별 신호 조회
#[utoipa::path(
    get,
    path = "/api/v1/signals/by-strategy",
    params(StrategySignalsQuery),
    responses(
        (status = 200, description = "조회 성공", body = SignalSearchResponse),
        (status = 400, description = "잘못된 요청", body = ApiErrorResponse),
        (status = 500, description = "서버 오류", body = ApiErrorResponse)
    ),
    tag = "signals"
)]
pub async fn get_signals_by_strategy(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StrategySignalsQuery>,
) -> Result<Json<SignalSearchResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new(
                    "DATABASE_ERROR",
                    "Database not available",
                )),
            ))
        }
    };

    let repo = SignalMarkerRepository::new(db_pool.clone());

    let markers = repo
        .find_by_strategy(
            &query.strategy_id,
            query.start_time,
            query.end_time,
            Some(query.limit),
        )
        .await?;

    let total = markers.len();
    let signals = markers.into_iter().map(SignalMarkerDto::from).collect();

    Ok(Json(SignalSearchResponse { total, signals }))
}

/// 백테스트 신호(거래) 응답
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BacktestSignalsResponse {
    /// 백테스트 ID
    pub backtest_id: Uuid,

    /// 전략 ID
    pub strategy_id: String,

    /// 전략 유형
    pub strategy_type: String,

    /// 심볼
    pub symbol: String,

    /// 총 거래 수
    pub total_trades: usize,

    /// 거래 목록 (JSON 형태)
    pub trades: JsonValue,
}

/// 백테스트 신호(거래) 조회
///
/// 특정 백테스트의 신호 및 거래 내역을 조회합니다.
#[utoipa::path(
    get,
    path = "/api/v1/signals/markers/backtest/{id}",
    params(
        ("id" = Uuid, Path, description = "백테스트 결과 ID")
    ),
    responses(
        (status = 200, description = "조회 성공", body = BacktestSignalsResponse),
        (status = 404, description = "백테스트를 찾을 수 없음", body = ApiErrorResponse),
        (status = 500, description = "서버 오류", body = ApiErrorResponse)
    ),
    tag = "signals"
)]
pub async fn get_backtest_signals(
    State(state): State<Arc<AppState>>,
    Path(backtest_id): Path<Uuid>,
) -> Result<Json<BacktestSignalsResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new(
                    "DATABASE_ERROR",
                    "Database not available",
                )),
            ))
        }
    };

    // 백테스트 결과 조회
    let result = BacktestResultsRepository::get_by_id(db_pool, backtest_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DATABASE_ERROR", e.to_string())),
            )
        })?;

    let result = match result {
        Some(r) => r,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse::new(
                    "NOT_FOUND",
                    "백테스트 결과를 찾을 수 없습니다",
                )),
            ))
        }
    };

    // trades 배열의 길이 계산
    let total_trades = result.trades.as_array().map(|arr| arr.len()).unwrap_or(0);

    Ok(Json(BacktestSignalsResponse {
        backtest_id: result.id,
        strategy_id: result.strategy_id,
        strategy_type: result.strategy_type,
        symbol: result.symbol,
        total_trades,
        trades: result.trades,
    }))
}

/// 시그널 생성 및 알림 전송
///
/// POST /api/v1/signals
///
/// 새 시그널을 생성하고 필터 조건을 만족하면 텔레그램 알림을 전송합니다.
/// 알림 조건: 강도 >= 0.7, Entry/Exit/Alert 유형
#[utoipa::path(
    post,
    path = "/api/v1/signals",
    request_body = CreateSignalRequest,
    responses(
        (status = 201, description = "시그널 생성 성공", body = CreateSignalResponse),
        (status = 400, description = "잘못된 요청", body = ApiErrorResponse),
        (status = 500, description = "서버 오류", body = ApiErrorResponse)
    ),
    tag = "signals"
)]
pub async fn create_signal(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSignalRequest>,
) -> Result<(StatusCode, Json<CreateSignalResponse>), (StatusCode, Json<ApiErrorResponse>)> {
    debug!(
        symbol = %req.symbol,
        signal_type = %req.signal_type,
        strength = req.strength,
        "시그널 생성 요청"
    );

    // 가격 파싱
    let price: Decimal = req.price.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse::new("INVALID_PRICE", "유효하지 않은 가격 형식")),
        )
    })?;

    // 신호 유형 파싱
    let signal_type = match req.signal_type.to_lowercase().as_str() {
        "entry" => SignalType::Entry,
        "exit" => SignalType::Exit,
        "alert" => SignalType::Alert,
        "scale" => SignalType::Scale,
        "addtoposition" => SignalType::AddToPosition,
        "reduceposition" => SignalType::ReducePosition,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse::new(
                    "INVALID_SIGNAL_TYPE",
                    "유효하지 않은 신호 유형. Entry, Exit, Alert, Scale 중 하나 사용",
                )),
            ))
        }
    };

    // 방향 파싱
    let side = req.side.as_ref().and_then(|s| match s.to_lowercase().as_str() {
        "buy" => Some(Side::Buy),
        "sell" => Some(Side::Sell),
        _ => None,
    });

    // SignalMarker 생성
    let marker = SignalMarker {
        id: Uuid::new_v4(),
        ticker: req.symbol.clone(),
        timestamp: Utc::now(),
        signal_type,
        side,
        price,
        strength: req.strength,
        indicators: SignalIndicators::default(),
        reason: req.reason.clone(),
        strategy_id: req.strategy_id.clone(),
        strategy_name: req.strategy_name.clone(),
        executed: false,
        metadata: std::collections::HashMap::new(),
    };

    let signal_id = marker.id.to_string();

    // DB 저장 (선택)
    if let Some(pool) = &state.db_pool {
        let repo = SignalMarkerRepository::new(pool.clone());
        if let Err(e) = repo.save(&marker).await {
            warn!("시그널 DB 저장 실패: {:?} (알림은 계속 전송)", e);
        } else {
            debug!(signal_id = %signal_id, "시그널 DB 저장 완료");
        }
    }

    // 알림 전송
    let mut notified = false;
    #[allow(unused_assignments)]
    let mut notification_result: Option<String> = None;

    if req.notify && req.strength >= 0.7 {
        if let Some(notification_manager) = &state.notification_manager {
            // 방향 문자열
            let side_str = side.map(|s| match s {
                Side::Buy => "Buy",
                Side::Sell => "Sell",
            });

            // 지표 정보 JSON 변환
            let indicators_json = req.indicators.clone().unwrap_or(serde_json::json!({}));

            // 알림 전송
            match notification_manager
                .notify_signal_alert(
                    &req.signal_type,
                    &req.symbol,
                    side_str,
                    price,
                    req.strength,
                    &req.reason,
                    &req.strategy_name,
                    indicators_json,
                )
                .await
            {
                Ok(_) => {
                    notified = true;
                    notification_result = Some("텔레그램 알림 전송 완료".to_string());
                    info!(
                        symbol = %req.symbol,
                        signal_type = %req.signal_type,
                        strength = req.strength,
                        "시그널 알림 전송 완료"
                    );
                }
                Err(e) => {
                    notification_result = Some(format!("알림 전송 실패: {}", e));
                    warn!(error = %e, "시그널 알림 전송 실패");
                }
            }
        } else {
            notification_result = Some("알림 매니저가 설정되지 않음".to_string());
        }
    } else if !req.notify {
        notification_result = Some("알림 비활성화 (notify=false)".to_string());
    } else {
        notification_result = Some(format!(
            "알림 필터 미충족 (강도 {:.1}% < 70%)",
            req.strength * 100.0
        ));
    }

    Ok((
        StatusCode::CREATED,
        Json(CreateSignalResponse {
            id: signal_id,
            symbol: req.symbol,
            signal_type: req.signal_type,
            notified,
            notification_result,
        }),
    ))
}

// ==================== 신호 성과 API ====================

/// 신호 성과 통계 조회
///
/// 신호 타입별, 강도별, 심볼별, 전략별 성과 통계를 조회합니다.
#[utoipa::path(
    get,
    path = "/api/v1/signals/performance",
    responses(
        (status = 200, description = "성과 통계 조회 성공", body = SignalPerformanceResponse),
        (status = 500, description = "서버 오류", body = ApiErrorResponse)
    ),
    tag = "signals"
)]
pub async fn get_signal_performance(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SignalPerformanceResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new(
                    "DATABASE_ERROR",
                    "Database not available",
                )),
            ))
        }
    };

    let response = SignalPerformanceRepository::get_performance_summary(db_pool)
        .await
        .map_err(|e| {
            warn!(error = %e, "신호 성과 통계 조회 실패");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DATABASE_ERROR", e.to_string())),
            )
        })?;

    Ok(Json(response))
}

/// 신호-수익률 산점도 데이터 조회 쿼리
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct ScatterQuery {
    /// 종목 코드 (선택, 미지정 시 전체)
    pub ticker: Option<String>,
    /// 최대 결과 개수 (기본 100)
    #[serde(default = "default_limit")]
    pub limit: i64,
}

/// 신호-수익률 산점도 데이터 조회
///
/// 신호 강도와 수익률의 상관관계를 분석하기 위한 산점도 데이터를 조회합니다.
#[utoipa::path(
    get,
    path = "/api/v1/signals/performance/scatter",
    params(ScatterQuery),
    responses(
        (status = 200, description = "산점도 데이터 조회 성공", body = Vec<SignalReturnPoint>),
        (status = 500, description = "서버 오류", body = ApiErrorResponse)
    ),
    tag = "signals"
)]
pub async fn get_signal_scatter(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ScatterQuery>,
) -> Result<Json<Vec<SignalReturnPoint>>, (StatusCode, Json<ApiErrorResponse>)> {
    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new(
                    "DATABASE_ERROR",
                    "Database not available",
                )),
            ))
        }
    };

    let points = SignalPerformanceRepository::get_return_scatter(
        db_pool,
        query.ticker.as_deref(),
        query.limit.min(500),
    )
    .await
    .map_err(|e| {
        warn!(error = %e, "신호 산점도 데이터 조회 실패");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse::new("DATABASE_ERROR", e.to_string())),
        )
    })?;

    Ok(Json(points))
}

/// 특정 심볼의 신호 성과 조회
#[utoipa::path(
    get,
    path = "/api/v1/signals/performance/{ticker}",
    params(
        ("ticker" = String, Path, description = "종목 코드")
    ),
    responses(
        (status = 200, description = "심볼 성과 조회 성공", body = SignalSymbolStats),
        (status = 404, description = "데이터 없음", body = ApiErrorResponse),
        (status = 500, description = "서버 오류", body = ApiErrorResponse)
    ),
    tag = "signals"
)]
pub async fn get_symbol_signal_performance(
    State(state): State<Arc<AppState>>,
    Path(ticker): Path<String>,
) -> Result<Json<SignalSymbolStats>, (StatusCode, Json<ApiErrorResponse>)> {
    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new(
                    "DATABASE_ERROR",
                    "Database not available",
                )),
            ))
        }
    };

    let stats = SignalPerformanceRepository::get_symbol_performance(db_pool, &ticker)
        .await
        .map_err(|e| {
            warn!(error = %e, ticker = %ticker, "심볼 신호 성과 조회 실패");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DATABASE_ERROR", e.to_string())),
            )
        })?;

    match stats {
        Some(s) => Ok(Json(s)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiErrorResponse::new(
                "NOT_FOUND",
                format!("{}의 신호 성과 데이터가 없습니다", ticker),
            )),
        )),
    }
}

// ==================== 라우터 ====================

/// SignalMarker API 라우터
pub fn signals_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(create_signal))
        .route("/search", post(search_signals))
        .route("/by-symbol", get(get_signals_by_symbol))
        .route("/by-strategy", get(get_signals_by_strategy))
        .route("/markers/backtest/{id}", get(get_backtest_signals))
        // 신호 성과 API
        .route("/performance", get(get_signal_performance))
        .route("/performance/scatter", get(get_signal_scatter))
        .route("/performance/{ticker}", get(get_symbol_signal_performance))
}
