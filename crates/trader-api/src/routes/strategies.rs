//! 전략 관리 endpoint.
//!
//! 전략 목록 조회, 시작/중지, 설정 변경을 위한 REST API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/strategies` - 전략 목록 조회
//! - `POST /api/v1/strategies` - 전략 생성
//! - `GET /api/v1/strategies/{id}` - 특정 전략 상세 조회
//! - `DELETE /api/v1/strategies/{id}` - 전략 삭제
//! - `POST /api/v1/strategies/{id}/start` - 전략 시작
//! - `POST /api/v1/strategies/{id}/stop` - 전략 중지
//! - `PUT /api/v1/strategies/{id}/config` - 전략 설정 변경

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use chrono::Utc;
use futures::FutureExt;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::repository::{strategies::CreateStrategyInput, StrategyRepository};
use crate::state::AppState;
use crate::websocket::{ServerMessage, StrategyUpdateData};
use trader_strategy::{EngineError, EngineStats, Strategy, StrategyStatus};

// ==================== 응답 타입 ====================

/// 전략 목록 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "strategies/")]
pub struct StrategiesListResponse {
    /// 전략 목록
    pub strategies: Vec<StrategyListItem>,
    /// 전체 전략 수
    pub total: usize,
    /// 실행 중인 전략 수
    pub running: usize,
}

/// 전략 목록 항목.
#[derive(Debug, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "strategies/")]
pub struct StrategyListItem {
    /// 전략 ID
    pub id: String,
    /// 전략 타입 (예: "rsi", "grid_trading", "sma" 등)
    #[serde(rename = "strategyType")]
    pub strategy_type: String,
    /// 전략 이름
    pub name: String,
    /// 전략 상태 ("Running", "Stopped", "Error")
    pub status: String,
    /// 시장 ("KR", "US", "CRYPTO")
    pub market: String,
    /// 거래 심볼 목록
    pub symbols: Vec<String>,
    /// 타임프레임 (1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w, 1M)
    pub timeframe: String,
    /// 손익
    pub pnl: f64,
    /// 승률
    #[serde(rename = "winRate")]
    pub win_rate: f64,
    /// 거래 횟수
    /// NOTE: u32 사용 (ts-rs에서 number로 변환, bigint 대신)
    #[serde(rename = "tradesCount")]
    pub trades_count: u32,
    /// 리스크 프로필 (conservative, default, aggressive, custom)
    #[serde(rename = "riskProfile")]
    pub risk_profile: Option<String>,
    /// 할당 자본
    #[serde(rename = "allocatedCapital")]
    pub allocated_capital: Option<f64>,
    /// 다중 타임프레임 여부
    #[serde(rename = "isMultiTimeframe")]
    pub is_multi_timeframe: bool,
    /// 다중 타임프레임 설정 (NULL이면 단일 TF 전략)
    #[serde(
        rename = "multiTimeframeConfig",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional, type = "Record<string, unknown> | null")]
    pub multi_timeframe_config: Option<Value>,
    /// 연결된 거래소 계정 ID
    #[serde(rename = "credentialId", skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub credential_id: Option<String>,
    /// 연결된 거래소 계정 이름 (표시용)
    #[serde(rename = "credentialName", skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub credential_name: Option<String>,
}

/// 전략 상세 응답.
// Note: ToSchema not derived due to StrategyStatus dependency from trader-strategy crate
// Note: TS not derived - StrategyStatus는 외부 crate 타입이라 ts-rs 미지원
#[derive(Debug, Serialize)]
pub struct StrategyDetailResponse {
    /// 전략 ID
    pub id: String,
    /// 전략 타입 (예: "grid_trading", "rsi")
    pub strategy_type: String,
    /// 전략 상태 정보
    #[serde(flatten)]
    pub status: StrategyStatus,
    /// 전략 설정 (편집용)
    pub config: Value,
    /// 연결된 거래소 계정 ID
    #[serde(rename = "credentialId", skip_serializing_if = "Option::is_none")]
    pub credential_id: Option<String>,
    /// 대상 시장 (KR/US/CRYPTO)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market: Option<String>,
}

/// 전략 시작/중지 응답.
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "strategies/")]
pub struct StrategyActionResponse {
    /// 성공 여부
    pub success: bool,
    /// 전략 ID
    pub strategy_id: String,
    /// 수행된 액션
    pub action: String,
    /// 메시지
    pub message: String,
}

/// 전략 설정 변경 요청.
#[derive(Debug, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "strategies/")]
pub struct UpdateConfigRequest {
    /// 새로운 설정 (JSON)
    #[ts(type = "Record<string, unknown>")]
    #[schema(value_type = Object)]
    pub config: Value,
}

/// 리스크 설정 변경 요청.
#[derive(Debug, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "strategies/")]
pub struct UpdateRiskSettingsRequest {
    /// 리스크 설정 (RiskConfig 형식)
    #[serde(default)]
    #[ts(type = "Record<string, unknown> | null")]
    #[schema(value_type = Option<Object>)]
    pub risk_config: Option<Value>,
    /// 할당 자본 (NULL이면 전체 계좌 잔고 사용)
    #[serde(default)]
    pub allocated_capital: Option<f64>,
    /// 리스크 프로필 (conservative, default, aggressive, custom)
    #[serde(default)]
    pub risk_profile: Option<String>,
}

/// 전략 심볼 변경 요청.
#[derive(Debug, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "strategies/")]
pub struct UpdateSymbolsRequest {
    /// 새로운 심볼 목록
    pub symbols: Vec<String>,
}

/// 전략 거래소 계정 업데이트 요청.
#[derive(Debug, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "strategies/")]
pub struct UpdateCredentialRequest {
    /// 새로운 거래소 계정 ID (NULL이면 연결 해제)
    #[serde(rename = "credentialId")]
    #[ts(optional)]
    pub credential_id: Option<String>,
    /// 대상 시장 (KR/US/CRYPTO)
    #[ts(optional)]
    pub market: Option<String>,
}

/// 전략 복사 요청.
#[derive(Debug, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "strategies/")]
pub struct CloneStrategyRequest {
    /// 새 전략 이름
    pub new_name: String,
    /// 파라미터 오버라이드 (옵션)
    #[serde(default)]
    #[ts(type = "Record<string, unknown> | null")]
    #[schema(value_type = Option<Object>)]
    pub override_params: Option<Value>,
    /// 리스크 설정 오버라이드 (옵션)
    #[serde(default)]
    #[ts(type = "Record<string, unknown> | null")]
    #[schema(value_type = Option<Object>)]
    pub override_risk_config: Option<Value>,
    /// 할당 자본 오버라이드 (옵션)
    #[serde(default)]
    pub override_allocated_capital: Option<f64>,
}

/// 전략 복사 응답.
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "strategies/")]
pub struct CloneStrategyResponse {
    /// 성공 여부
    pub success: bool,
    /// 원본 전략 ID
    pub source_id: String,
    /// 생성된 전략 ID
    pub new_id: String,
    /// 새 전략 이름
    pub new_name: String,
    /// 메시지
    pub message: String,
}

/// 전략 생성 요청.
#[derive(Debug, Deserialize, Validate, ToSchema, TS)]
#[ts(export, export_to = "strategies/")]
pub struct CreateStrategyRequest {
    /// 전략 타입 (예: "grid_trading", "rsi", "bollinger" 등)
    #[validate(length(min = 1, max = 50, message = "전략 타입은 1-50자여야 합니다"))]
    pub strategy_type: String,
    /// 전략 이름 (사용자 지정, 옵션)
    #[validate(length(max = 100, message = "전략 이름은 100자 이하여야 합니다"))]
    pub name: Option<String>,
    /// 전략 파라미터
    #[ts(type = "Record<string, unknown>")]
    #[schema(value_type = Object)]
    pub parameters: Value,
    /// 리스크 설정 (옵션, RiskConfig 형식)
    #[serde(default)]
    #[ts(type = "Record<string, unknown> | null")]
    #[schema(value_type = Option<Object>)]
    pub risk_config: Option<Value>,
    /// 할당 자본 (옵션, NULL이면 전체 계좌 잔고 사용)
    #[serde(default)]
    #[validate(range(min = 0.0, message = "할당 자본은 0 이상이어야 합니다"))]
    pub allocated_capital: Option<f64>,
    /// 리스크 프로필 (conservative, default, aggressive, custom)
    #[serde(default)]
    #[validate(custom(function = "validate_risk_profile"))]
    pub risk_profile: Option<String>,
    /// 다중 타임프레임 설정 (옵션)
    /// 형식: {"primary": "5m", "secondary": [{"timeframe": "1h", "candle_count": 24}]}
    #[serde(default, rename = "multiTimeframeConfig")]
    #[ts(type = "Record<string, unknown> | null")]
    #[schema(value_type = Option<Object>)]
    pub multi_timeframe_config: Option<Value>,
    /// 연결할 거래소 계정 ID (옵션, NULL이면 기본 활성 계정 사용)
    #[serde(default, rename = "credentialId")]
    #[ts(optional)]
    pub credential_id: Option<String>,
    /// 대상 시장 (옵션, KR/US/CRYPTO)
    /// 지정하지 않으면 symbols에서 자동 추론
    #[serde(default)]
    #[ts(optional)]
    pub market: Option<String>,
}

/// 전략 생성 응답.
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "strategies/")]
pub struct CreateStrategyResponse {
    /// 성공 여부
    pub success: bool,
    /// 생성된 전략 ID
    pub strategy_id: String,
    /// 전략 이름
    pub name: String,
    /// 메시지
    pub message: String,
}

/// 엔진 통계 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "strategies/")]
pub struct EngineStatsResponse {
    /// 전체 전략 수
    pub total_strategies: usize,
    /// 실행 중인 전략 수
    pub running_strategies: usize,
    /// 총 생성된 신호 수
    pub total_signals_generated: u64,
    /// 총 체결된 주문 수
    pub total_orders_filled: u64,
    /// 총 처리된 시장 데이터 수
    pub total_market_data_processed: u64,
}

impl From<EngineStats> for EngineStatsResponse {
    fn from(stats: EngineStats) -> Self {
        Self {
            total_strategies: stats.total_strategies,
            running_strategies: stats.running_strategies,
            total_signals_generated: stats.total_signals_generated,
            total_orders_filled: stats.total_orders_filled,
            total_market_data_processed: stats.total_market_data_processed,
        }
    }
}

// ==================== 유효성 검사 함수 ====================

/// 리스크 프로필 유효성 검사.
fn validate_risk_profile(profile: &str) -> Result<(), validator::ValidationError> {
    const VALID_PROFILES: &[&str] = &["conservative", "default", "aggressive", "custom"];
    if VALID_PROFILES.contains(&profile) {
        Ok(())
    } else {
        let mut error = validator::ValidationError::new("invalid_risk_profile");
        error.message = Some(std::borrow::Cow::from(
            "리스크 프로필은 conservative, default, aggressive, custom 중 하나여야 합니다",
        ));
        Err(error)
    }
}

// ==================== API 에러 ====================

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiError {
    /// 에러 코드
    pub code: String,
    /// 에러 메시지
    pub message: String,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// 유효성 검사 오류로부터 ApiError 생성.
    pub fn validation_error(errors: &validator::ValidationErrors) -> Self {
        let messages: Vec<String> = errors
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                errors.iter().map(move |e| {
                    e.message
                        .as_ref()
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| format!("{}: 유효하지 않은 값", field))
                })
            })
            .collect();

        Self {
            code: "VALIDATION_ERROR".to_string(),
            message: messages.join("; "),
        }
    }
}

// ==================== 전략 팩토리 ====================

/// 전략 타입에 따라 전략 인스턴스를 생성.
///
/// StrategyRegistry를 통해 등록된 전략의 인스턴스를 생성합니다.
fn create_strategy_instance(strategy_type: &str) -> Result<Box<dyn Strategy>, String> {
    trader_strategy::StrategyRegistry::create_instance(strategy_type)
}

/// 전략 타입에서 기본 이름 가져오기.
///
/// StrategyRegistry를 통해 등록된 전략의 이름을 조회합니다.
fn get_strategy_default_name(strategy_type: &str) -> &'static str {
    trader_strategy::StrategyRegistry::find(strategy_type)
        .map(|meta| meta.name)
        .unwrap_or("Unknown Strategy")
}

/// 전략 타입에서 기본 타임프레임 가져오기.
///
/// StrategyRegistry를 통해 등록된 전략의 기본 타임프레임을 조회합니다.
fn get_strategy_default_timeframe(strategy_type: &str) -> &'static str {
    trader_strategy::StrategyRegistry::find(strategy_type)
        .map(|meta| meta.default_timeframe)
        .unwrap_or("1d")
}

/// 전략 타입에서 권장 심볼 가져오기.
///
/// StrategyRegistry를 통해 등록된 전략의 권장 심볼 목록을 조회합니다.
fn get_strategy_default_symbols(strategy_type: &str) -> Vec<String> {
    trader_strategy::StrategyRegistry::find(strategy_type)
        .map(|meta| meta.default_tickers.iter().map(|s| s.to_string()).collect())
        .unwrap_or_default()
}

// ==================== 에러 처리 ====================

/// EngineError를 HTTP 응답으로 변환.
fn engine_error_to_response(err: EngineError) -> (StatusCode, Json<ApiError>) {
    let (status, code) = match &err {
        EngineError::StrategyNotFound(_) => (StatusCode::NOT_FOUND, "STRATEGY_NOT_FOUND"),
        EngineError::StrategyAlreadyExists(_) => (StatusCode::CONFLICT, "STRATEGY_EXISTS"),
        EngineError::InitializationFailed(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INIT_FAILED"),
        EngineError::NotRunning(_) => (StatusCode::BAD_REQUEST, "NOT_RUNNING"),
        EngineError::AlreadyRunning(_) => (StatusCode::BAD_REQUEST, "ALREADY_RUNNING"),
        EngineError::ChannelError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "CHANNEL_ERROR"),
        EngineError::InternalError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
    };

    (status, Json(ApiError::new(code, err.to_string())))
}

// ==================== handler ====================

/// 전략 생성.
///
/// POST /api/v1/strategies
#[utoipa::path(
    post,
    path = "/api/v1/strategies",
    tag = "strategies",
    request_body = CreateStrategyRequest,
    responses(
        (status = 200, description = "전략 생성 성공", body = CreateStrategyResponse),
        (status = 400, description = "잘못된 요청", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn create_strategy(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateStrategyRequest>,
) -> Result<Json<CreateStrategyResponse>, (StatusCode, Json<ApiError>)> {
    // 입력 유효성 검사
    if let Err(errors) = request.validate() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::validation_error(&errors)),
        ));
    }

    // 전략 인스턴스 생성
    let strategy = create_strategy_instance(&request.strategy_type).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("INVALID_STRATEGY_TYPE", e)),
        )
    })?;

    // 전략 ID 생성 (UUID)
    let strategy_id = format!(
        "{}_{}",
        request.strategy_type,
        &Uuid::new_v4().to_string()[..8]
    );

    // 전략 이름 (커스텀 이름이 있으면 사용, 없으면 기본 이름)
    let custom_name = request.name.clone();
    let display_name = custom_name
        .clone()
        .unwrap_or_else(|| get_strategy_default_name(&request.strategy_type).to_string());

    // 파라미터에서 symbols 추출 (symbols → config 전체에서 추출 → 권장 심볼)
    let symbols: Vec<String> = request
        .parameters
        .get("symbols")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty())
        // config 전체에서 ticker/tickers/universe 추출
        .or_else(|| {
            let extracted = trader_strategy::extract_tickers_from_config(&request.parameters);
            if extracted.is_empty() {
                None
            } else {
                Some(extracted)
            }
        })
        .unwrap_or_else(|| get_strategy_default_symbols(&request.strategy_type));

    // 파라미터에서 timeframe 추출 (없으면 기본 타임프레임 사용)
    let timeframe = request
        .parameters
        .get("timeframe")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| get_strategy_default_timeframe(&request.strategy_type).to_string());

    // 마켓 타입: request에서 지정한 값 사용, 없으면 심볼 기반 추론
    let market = request.market.clone().unwrap_or_else(|| {
        // 심볼 기반 추론 (폴백)
        if symbols
            .first()
            .map(|s| s.chars().all(|c| c.is_numeric()))
            .unwrap_or(false)
        {
            "KR".to_string()
        } else if symbols.first().map(|s| s.contains('/')).unwrap_or(false) {
            "CRYPTO".to_string()
        } else {
            "US".to_string()
        }
    });

    // 할당 자본을 Decimal로 변환
    let allocated_capital = request
        .allocated_capital
        .map(|v| Decimal::try_from(v).unwrap_or(Decimal::ZERO));

    // 데이터베이스에 저장 (DB가 연결된 경우)
    if let Some(ref pool) = state.db_pool {
        // credential_id 파싱 (String -> Uuid)
        let credential_id = request
            .credential_id
            .as_ref()
            .and_then(|s| Uuid::parse_str(s).ok());

        let input = CreateStrategyInput {
            id: strategy_id.clone(),
            name: display_name.clone(),
            description: None,
            strategy_type: request.strategy_type.clone(),
            symbols: symbols.clone(),
            market: market.clone(),
            timeframe: timeframe.clone(),
            config: request.parameters.clone(),
            risk_config: request.risk_config.clone(),
            allocated_capital,
            risk_profile: request.risk_profile.clone(),
            multi_timeframe_config: request.multi_timeframe_config.clone(),
            credential_id,
        };

        StrategyRepository::create(pool, input).await.map_err(|e| {
            tracing::error!("Failed to save strategy to database: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to save strategy: {}", e),
                )),
            )
        })?;
    }

    // 엔진에 전략 등록 (공유 StrategyContext 전달)
    let engine = state.strategy_engine.read().await;
    engine
        .register_strategy(
            &strategy_id,
            strategy,
            request.parameters.clone(),
            custom_name,
            state.strategy_context.clone(),
        )
        .await
        .map_err(engine_error_to_response)?;

    // WebSocket 브로드캐스트: 전략 생성 알림
    state.broadcast(ServerMessage::StrategyUpdate(StrategyUpdateData {
        strategy_id: strategy_id.clone(),
        name: display_name.clone(),
        running: false,
        event: "created".to_string(),
        data: Some(serde_json::json!({
            "strategy_type": request.strategy_type
        })),
        timestamp: Utc::now().timestamp_millis(),
    }));

    Ok(Json(CreateStrategyResponse {
        success: true,
        strategy_id: strategy_id.clone(),
        name: display_name,
        message: format!("Strategy '{}' created successfully", strategy_id),
    }))
}

/// 전략 삭제.
///
/// DELETE /api/v1/strategies/{id}
#[utoipa::path(
    delete,
    path = "/api/v1/strategies/{id}",
    tag = "strategies",
    params(("id" = String, Path, description = "전략 ID")),
    responses(
        (status = 200, description = "전략 삭제 성공", body = StrategyActionResponse),
        (status = 404, description = "전략을 찾을 수 없음", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn delete_strategy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<StrategyActionResponse>, (StatusCode, Json<ApiError>)> {
    let engine = state.strategy_engine.read().await;

    // 삭제 전 전략 정보 가져오기 (브로드캐스트용)
    let strategy_name = engine
        .get_strategy_status(&id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| id.clone());

    engine
        .unregister_strategy(&id)
        .await
        .map_err(engine_error_to_response)?;

    // 데이터베이스에서 삭제 (DB가 연결된 경우)
    if let Some(ref pool) = state.db_pool {
        if let Err(e) = StrategyRepository::delete(pool, &id).await {
            tracing::warn!("Failed to delete strategy from database: {:?}", e);
            // DB 삭제 실패는 경고만 남기고 계속 진행 (엔진에서는 이미 삭제됨)
        }

        // 전략 관심 종목도 정리 (Collector 우선순위에서 제외)
        if let Err(e) =
            crate::repository::StrategyWatchedTickersRepository::delete_by_strategy(pool, &id)
                .await
        {
            tracing::warn!(strategy_id = %id, error = %e, "전략 관심 종목 DB 정리 실패");
        }
    }

    // WebSocket 브로드캐스트: 전략 삭제 알림
    state.broadcast(ServerMessage::StrategyUpdate(StrategyUpdateData {
        strategy_id: id.clone(),
        name: strategy_name,
        running: false,
        event: "deleted".to_string(),
        data: None,
        timestamp: Utc::now().timestamp_millis(),
    }));

    Ok(Json(StrategyActionResponse {
        success: true,
        strategy_id: id.clone(),
        action: "delete".to_string(),
        message: format!("Strategy '{}' deleted successfully", id),
    }))
}

/// 전략 목록 조회 쿼리 파라미터.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ListStrategiesQuery {
    /// 특정 계정의 전략만 조회 (옵션)
    #[serde(rename = "credentialId")]
    pub credential_id: Option<String>,
}

/// 전략 목록 조회.
///
/// GET /api/v1/strategies
#[utoipa::path(
    get,
    path = "/api/v1/strategies",
    tag = "strategies",
    params(
        ("credentialId" = Option<String>, Query, description = "특정 계정의 전략만 조회")
    ),
    responses(
        (status = 200, description = "전략 목록 조회 성공", body = StrategiesListResponse)
    )
)]
pub async fn list_strategies(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<ListStrategiesQuery>,
) -> impl IntoResponse {
    // DB에서 credential_id 필터링된 전략 조회 (가능한 경우)
    let db_strategies = if let Some(ref pool) = state.db_pool {
        let credential_uuid = query
            .credential_id
            .as_ref()
            .and_then(|s| Uuid::parse_str(s).ok());

        match StrategyRepository::get_by_credential(pool, credential_uuid).await {
            Ok(records) => records,
            Err(e) => {
                tracing::warn!("DB에서 전략 조회 실패: {:?}", e);
                vec![]
            }
        }
    } else {
        vec![]
    };

    // DB 전략을 HashMap으로 변환 (빠른 조회용)
    let db_strategy_map: std::collections::HashMap<String, _> = db_strategies
        .into_iter()
        .map(|r| (r.id.clone(), r))
        .collect();

    let engine = state.strategy_engine.read().await;
    let all_statuses = engine.get_all_statuses().await;

    let mut strategies: Vec<StrategyListItem> = Vec::new();

    for (id, status) in all_statuses {
        // credential_id 필터가 있는 경우, DB에 있는 전략만 포함
        if query.credential_id.is_some() && !db_strategy_map.contains_key(&id) {
            continue;
        }

        // DB에서 전략 정보 조회
        let db_record = db_strategy_map.get(&id);

        // 전략 타입 조회 (DB 우선, 없으면 엔진에서)
        let strategy_type = db_record
            .and_then(|r| r.strategy_type.clone())
            .unwrap_or_else(|| {
                engine
                    .get_strategy_type(&id)
                    .now_or_never()
                    .and_then(|r| r.ok())
                    .unwrap_or_else(|| "unknown".to_string())
            });

        // 전략 상태 문자열 변환
        let status_str = if status.running {
            "Running".to_string()
        } else {
            "Stopped".to_string()
        };

        // 시장 정보 (DB 우선, fallback은 심볼 기반 추론)
        let market = db_record
            .and_then(|r| r.market.clone())
            .unwrap_or_else(|| {
                if id.contains("binance") || id.contains("crypto") {
                    "CRYPTO".to_string()
                } else {
                    "UNKNOWN".to_string()
                }
            });

        // 심볼 목록 (DB symbols 컬럼 → config에서 추출 → 레지스트리 기본값)
        let symbols = db_record
            .and_then(|r| {
                r.symbols.as_ref().and_then(|v| {
                    v.as_array().map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect::<Vec<_>>()
                    })
                })
            })
            // 빈 배열이면 None으로 변환하여 다음 폴백으로 진행
            .filter(|v| !v.is_empty())
            // config에서 ticker/tickers/universe 추출 시도
            .or_else(|| {
                let tickers: Vec<String> = db_record
                    .map(|r| trader_strategy::extract_tickers_from_config(&r.config))
                    .unwrap_or_default();
                if tickers.is_empty() {
                    None
                } else {
                    Some(tickers)
                }
            })
            // 레지스트리 기본값
            .unwrap_or_else(|| {
                let syms = get_strategy_default_symbols(&strategy_type);
                if syms.is_empty() {
                    vec!["005930".to_string()]
                } else {
                    syms
                }
            });

        // 타임프레임 (DB 우선)
        let timeframe = db_record
            .and_then(|r| r.timeframe.clone())
            .unwrap_or_else(|| get_strategy_default_timeframe(&strategy_type).to_string());

        // credential_id (DB에서)
        let credential_id = db_record.and_then(|r| r.credential_id.map(|u| u.to_string()));

        // allocated_capital (DB에서)
        let allocated_capital = db_record
            .and_then(|r| r.allocated_capital)
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0));

        // risk_profile (DB에서)
        let risk_profile = db_record.and_then(|r| r.risk_profile.clone());

        // multi_timeframe (DB에서)
        let is_multi_timeframe = db_record
            .map(|r| r.multi_timeframe_config.is_some())
            .unwrap_or(false);
        let multi_timeframe_config = db_record.and_then(|r| r.multi_timeframe_config.clone());

        strategies.push(StrategyListItem {
            id,
            strategy_type,
            name: status.name,
            status: status_str,
            market,
            symbols,
            timeframe,
            pnl: 0.0,
            win_rate: 0.0,
            trades_count: status.stats.signals_generated as u32,
            risk_profile,
            allocated_capital,
            is_multi_timeframe,
            multi_timeframe_config,
            credential_id,
            credential_name: None, // TODO: JOIN으로 계정명 조회
        });
    }

    // ID로 정렬
    strategies.sort_by(|a, b| a.id.cmp(&b.id));

    let running_count = strategies.iter().filter(|s| s.status == "Running").count();
    let total = strategies.len();

    Json(StrategiesListResponse {
        strategies,
        total,
        running: running_count,
    })
}

/// 특정 전략 상세 조회.
///
/// GET /api/v1/strategies/{id}
#[utoipa::path(
    get,
    path = "/api/v1/strategies/{id}",
    tag = "strategies",
    params(("id" = String, Path, description = "전략 ID")),
    responses(
        (status = 200, description = "전략 상세 조회 성공", body = Object, description = "StrategyDetailResponse (StrategyStatus 포함)"),
        (status = 404, description = "전략을 찾을 수 없음", body = ApiError)
    )
)]
pub async fn get_strategy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<StrategyDetailResponse>, (StatusCode, Json<ApiError>)> {
    let engine = state.strategy_engine.read().await;

    // 상태 조회
    let status = engine
        .get_strategy_status(&id)
        .await
        .map_err(engine_error_to_response)?;

    // 설정 조회
    let config = engine
        .get_strategy_config(&id)
        .await
        .map_err(engine_error_to_response)?;

    // 전략 타입 조회
    let strategy_type = engine
        .get_strategy_type(&id)
        .await
        .map_err(engine_error_to_response)?;

    // DB에서 credential_id와 market 조회
    let (credential_id, market) = if let Some(ref pool) = state.db_pool {
        match StrategyRepository::get_by_id(pool, &id).await {
            Ok(Some(record)) => (
                record.credential_id.map(|u| u.to_string()),
                record.market,
            ),
            _ => (None, None),
        }
    } else {
        (None, None)
    };

    Ok(Json(StrategyDetailResponse {
        id,
        strategy_type,
        status,
        config,
        credential_id,
        market,
    }))
}

/// 전략 시작.
///
/// POST /api/v1/strategies/{id}/start
///
/// 다중 타임프레임 전략의 경우, 시작 전에 필요한 캔들 데이터를 자동으로 로드합니다.
#[utoipa::path(
    post,
    path = "/api/v1/strategies/{id}/start",
    tag = "strategies",
    params(("id" = String, Path, description = "전략 ID")),
    responses(
        (status = 200, description = "전략 시작 성공", body = StrategyActionResponse),
        (status = 400, description = "이미 실행 중", body = ApiError),
        (status = 404, description = "전략을 찾을 수 없음", body = ApiError)
    )
)]
pub async fn start_strategy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<StrategyActionResponse>, (StatusCode, Json<ApiError>)> {
    let engine = state.strategy_engine.read().await;

    // 전략 이름 가져오기 (브로드캐스트용)
    let strategy_name = engine
        .get_strategy_status(&id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| id.clone());

    // 다중 타임프레임 전략인 경우 데이터 자동 로드
    if let Ok(Some(mtf_config)) = engine.get_strategy_multi_tf_config(&id).await {
        if let Err(e) = load_multi_timeframe_data(&state, &engine, &id, &mtf_config).await {
            tracing::warn!(
                strategy_id = %id,
                error = %e,
                "다중 타임프레임 데이터 로드 실패 (전략은 계속 시작됨)"
            );
        }
    }

    match engine.start_strategy(&id).await {
        Ok(()) => {
            // 실시간 스트림 구독 (credential_id가 연결된 경우)
            subscribe_strategy_symbols_to_stream(&state, &engine, &id).await;

            // 전략 관심 종목을 DB에 저장 (Collector 우선순위 연동)
            persist_strategy_watched_tickers(&state, &engine, &id).await;

            // WebSocket 브로드캐스트: 전략 시작 알림
            state.broadcast(ServerMessage::StrategyUpdate(StrategyUpdateData {
                strategy_id: id.clone(),
                name: strategy_name,
                running: true,
                event: "started".to_string(),
                data: None,
                timestamp: Utc::now().timestamp_millis(),
            }));

            Ok(Json(StrategyActionResponse {
                success: true,
                strategy_id: id.clone(),
                action: "start".to_string(),
                message: format!("Strategy '{}' started successfully", id),
            }))
        }
        Err(err) => Err(engine_error_to_response(err)),
    }
}

/// 전략의 심볼을 실시간 MarketStream에 구독.
///
/// 전략에 연결된 credential_id가 있으면, 해당 거래소의 WebSocket 스트림을
/// lazy 초기화하고 전략의 관심 심볼을 구독합니다.
async fn subscribe_strategy_symbols_to_stream(
    state: &AppState,
    engine: &trader_strategy::StrategyEngine,
    strategy_id: &str,
) {
    use crate::services::get_or_create_market_stream;

    // 1. 전략 설정에서 credential_id와 symbols 가져오기
    let config = match engine.get_strategy_config(strategy_id).await {
        Ok(config) => config,
        Err(_) => return,
    };

    let credential_id_str = config
        .get("credential_id")
        .and_then(|v| v.as_str())
        .or_else(|| config.get("credentialId").and_then(|v| v.as_str()));

    let credential_id = match credential_id_str {
        Some(id) => match uuid::Uuid::parse_str(id) {
            Ok(uuid) => uuid,
            Err(_) => return,
        },
        None => return, // credential_id가 없으면 스트림 구독 불필요
    };

    let symbols: Vec<String> = config
        .get("symbols")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if symbols.is_empty() {
        return;
    }

    // 2. DB/암호화 관련 의존성 확인
    let (pool, encryptor) = match (&state.db_pool, &state.encryptor) {
        (Some(pool), Some(encryptor)) => (pool, encryptor),
        _ => {
            tracing::debug!(
                strategy_id = %strategy_id,
                "DB 또는 암호화 관리자 미설정, 실시간 스트림 구독 스킵"
            );
            return;
        }
    };

    // 3. MarketStream 핸들 가져오기 (없으면 생성)
    let handle = match get_or_create_market_stream(
        &state.market_streams,
        pool,
        encryptor.as_ref(),
        &state.kis_oauth_cache,
        credential_id,
        state.subscriptions.as_ref(),
    )
    .await
    {
        Ok(handle) => handle,
        Err(e) => {
            tracing::warn!(
                strategy_id = %strategy_id,
                credential_id = %credential_id,
                error = %e,
                "MarketStream 생성 실패, 실시간 데이터 없이 전략 실행"
            );
            return;
        }
    };

    // 4. 전략 심볼 구독
    for symbol in &symbols {
        if let Err(e) = handle.subscribe(symbol).await {
            tracing::warn!(
                strategy_id = %strategy_id,
                symbol = %symbol,
                error = %e,
                "심볼 실시간 구독 실패"
            );
        }
    }

    tracing::info!(
        strategy_id = %strategy_id,
        credential_id = %credential_id,
        symbol_count = symbols.len(),
        "전략 심볼 실시간 스트림 구독 완료"
    );
}

/// 전략 관심 종목을 DB에 저장 (Collector 우선순위 연동).
///
/// 전략 config에서 ticker/tickers/universe/symbols 필드를 추출하여
/// `strategy_watched_tickers` 테이블에 UPSERT합니다.
/// Collector가 이 종목들의 데이터를 우선적으로 수집합니다.
async fn persist_strategy_watched_tickers(
    state: &AppState,
    engine: &trader_strategy::StrategyEngine,
    strategy_id: &str,
) {
    use crate::repository::StrategyWatchedTickersRepository;

    let pool = match &state.db_pool {
        Some(pool) => pool,
        None => return,
    };

    let config = match engine.get_strategy_config(strategy_id).await {
        Ok(config) => config,
        Err(_) => return,
    };

    // config에서 티커 추출 (engine.rs의 extract_tickers_from_config과 동일 로직)
    let mut tickers = Vec::new();

    if let Some(ticker) = config.get("ticker").and_then(|v| v.as_str()) {
        if !ticker.is_empty() {
            tickers.push(ticker.to_string());
        }
    }

    for field in &["tickers", "universe", "symbols"] {
        if let Some(arr) = config.get(*field).and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(t) = item.as_str() {
                    if !t.is_empty() && !tickers.contains(&t.to_string()) {
                        tickers.push(t.to_string());
                    }
                }
            }
        }
    }

    if tickers.is_empty() {
        return;
    }

    match StrategyWatchedTickersRepository::upsert_tickers(
        pool,
        strategy_id,
        &tickers,
        "config",
    )
    .await
    {
        Ok(count) => {
            tracing::info!(
                strategy_id = %strategy_id,
                ticker_count = count,
                "전략 관심 종목 DB 저장 완료"
            );
        }
        Err(e) => {
            tracing::warn!(
                strategy_id = %strategy_id,
                error = %e,
                "전략 관심 종목 DB 저장 실패"
            );
        }
    }
}

/// 다중 타임프레임 데이터 자동 로드 (전략 시작 전).
///
/// 전략의 설정에서 심볼 목록을 가져와 각 심볼에 대해
/// 다중 타임프레임 캔들 데이터를 로드하고 컨텍스트에 업데이트합니다.
async fn load_multi_timeframe_data(
    state: &AppState,
    engine: &trader_strategy::StrategyEngine,
    strategy_id: &str,
    mtf_config: &trader_core::domain::MultiTimeframeConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use trader_core::Timeframe;

    // 데이터 프로바이더 확인
    let data_provider = state
        .data_provider
        .as_ref()
        .ok_or("데이터 프로바이더 없음")?;

    // 전략 설정에서 심볼 목록 가져오기
    let config = engine.get_strategy_config(strategy_id).await?;
    let symbols: Vec<String> = config
        .get("symbols")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if symbols.is_empty() {
        tracing::debug!(strategy_id = %strategy_id, "심볼 목록 없음, 데이터 로드 스킵");
        return Ok(());
    }

    tracing::info!(
        strategy_id = %strategy_id,
        symbols = ?symbols,
        timeframes = ?mtf_config.timeframes.keys().collect::<Vec<_>>(),
        "다중 타임프레임 데이터 로드 시작"
    );

    // 각 심볼에 대해 데이터 로드
    for symbol in &symbols {
        match data_provider
            .get_multi_timeframe_klines(symbol, mtf_config)
            .await
        {
            Ok(klines_by_tf) => {
                // Kline 데이터를 (Timeframe, Vec<Kline>) 형태로 변환
                let data: Vec<(Timeframe, Vec<trader_core::Kline>)> =
                    klines_by_tf.into_iter().collect();

                // 전략 컨텍스트에 업데이트
                if let Err(e) = engine
                    .update_strategy_klines(strategy_id, symbol, data)
                    .await
                {
                    tracing::warn!(
                        strategy_id = %strategy_id,
                        symbol = %symbol,
                        error = %e,
                        "컨텍스트 업데이트 실패"
                    );
                } else {
                    tracing::info!(
                        strategy_id = %strategy_id,
                        symbol = %symbol,
                        "다중 타임프레임 데이터 로드 완료"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    strategy_id = %strategy_id,
                    symbol = %symbol,
                    error = %e,
                    "데이터 로드 실패"
                );
            }
        }
    }

    Ok(())
}

/// 전략 중지.
///
/// POST /api/v1/strategies/{id}/stop
#[utoipa::path(
    post,
    path = "/api/v1/strategies/{id}/stop",
    tag = "strategies",
    params(("id" = String, Path, description = "전략 ID")),
    responses(
        (status = 200, description = "전략 중지 성공", body = StrategyActionResponse),
        (status = 400, description = "실행 중이 아님", body = ApiError),
        (status = 404, description = "전략을 찾을 수 없음", body = ApiError)
    )
)]
pub async fn stop_strategy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<StrategyActionResponse>, (StatusCode, Json<ApiError>)> {
    let engine = state.strategy_engine.read().await;

    // 전략 이름 가져오기 (브로드캐스트용)
    let strategy_name = engine
        .get_strategy_status(&id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| id.clone());

    match engine.stop_strategy(&id).await {
        Ok(()) => {
            // WebSocket 브로드캐스트: 전략 중지 알림
            state.broadcast(ServerMessage::StrategyUpdate(StrategyUpdateData {
                strategy_id: id.clone(),
                name: strategy_name,
                running: false,
                event: "stopped".to_string(),
                data: None,
                timestamp: Utc::now().timestamp_millis(),
            }));

            Ok(Json(StrategyActionResponse {
                success: true,
                strategy_id: id.clone(),
                action: "stop".to_string(),
                message: format!("Strategy '{}' stopped successfully", id),
            }))
        }
        Err(err) => Err(engine_error_to_response(err)),
    }
}

/// 전략 설정 변경.
///
/// PUT /api/v1/strategies/{id}/config
#[utoipa::path(
    put,
    path = "/api/v1/strategies/{id}/config",
    tag = "strategies",
    params(("id" = String, Path, description = "전략 ID")),
    request_body = UpdateConfigRequest,
    responses(
        (status = 200, description = "설정 변경 성공", body = StrategyActionResponse),
        (status = 404, description = "전략을 찾을 수 없음", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<UpdateConfigRequest>,
) -> Result<Json<StrategyActionResponse>, (StatusCode, Json<ApiError>)> {
    let engine = state.strategy_engine.read().await;

    // 전략 상태 가져오기 (브로드캐스트용)
    let (strategy_name, is_running) = engine
        .get_strategy_status(&id)
        .await
        .map(|s| (s.name, s.running))
        .unwrap_or_else(|_| (id.clone(), false));

    match engine
        .update_strategy_config(&id, request.config.clone())
        .await
    {
        Ok(()) => {
            // DB에도 설정 저장 (DB가 연결된 경우)
            if let Some(pool) = state.db_pool.as_ref() {
                if let Err(e) =
                    StrategyRepository::update_config(pool, &id, request.config.clone()).await
                {
                    tracing::warn!(strategy_id = %id, error = %e, "Failed to persist strategy config to DB");
                    // DB 저장 실패해도 메모리 업데이트는 성공했으므로 계속 진행
                }
            }

            // WebSocket 브로드캐스트: 설정 변경 알림
            state.broadcast(ServerMessage::StrategyUpdate(StrategyUpdateData {
                strategy_id: id.clone(),
                name: strategy_name,
                running: is_running,
                event: "config_updated".to_string(),
                data: Some(request.config),
                timestamp: Utc::now().timestamp_millis(),
            }));

            Ok(Json(StrategyActionResponse {
                success: true,
                strategy_id: id.clone(),
                action: "update_config".to_string(),
                message: format!("Strategy '{}' configuration updated successfully", id),
            }))
        }
        Err(err) => Err(engine_error_to_response(err)),
    }
}

/// 전략 리스크 설정 변경.
///
/// PUT /api/v1/strategies/{id}/risk
#[utoipa::path(
    put,
    path = "/api/v1/strategies/{id}/risk",
    tag = "strategies",
    params(("id" = String, Path, description = "전략 ID")),
    request_body = UpdateRiskSettingsRequest,
    responses(
        (status = 200, description = "리스크 설정 변경 성공", body = StrategyActionResponse),
        (status = 404, description = "전략을 찾을 수 없음", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn update_risk_settings(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<UpdateRiskSettingsRequest>,
) -> Result<Json<StrategyActionResponse>, (StatusCode, Json<ApiError>)> {
    // DB가 연결된 경우에만 동작
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONNECTED", "Database not connected")),
        )
    })?;

    // 할당 자본을 Decimal로 변환
    let allocated_capital = request
        .allocated_capital
        .map(|v| Decimal::try_from(v).unwrap_or(Decimal::ZERO));

    // DB에 리스크 설정 업데이트
    StrategyRepository::update_risk_settings(
        pool,
        &id,
        request.risk_config.clone(),
        allocated_capital,
        request.risk_profile.as_deref(),
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to update risk settings: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_ERROR",
                format!("Failed to update risk settings: {}", e),
            )),
        )
    })?;

    // 전략 이름 가져오기 (브로드캐스트용)
    let engine = state.strategy_engine.read().await;
    let (strategy_name, is_running) = engine
        .get_strategy_status(&id)
        .await
        .map(|s| (s.name, s.running))
        .unwrap_or_else(|_| (id.clone(), false));

    // WebSocket 브로드캐스트: 리스크 설정 변경 알림
    state.broadcast(ServerMessage::StrategyUpdate(StrategyUpdateData {
        strategy_id: id.clone(),
        name: strategy_name,
        running: is_running,
        event: "risk_updated".to_string(),
        data: Some(serde_json::json!({
            "risk_profile": request.risk_profile,
            "allocated_capital": request.allocated_capital,
        })),
        timestamp: Utc::now().timestamp_millis(),
    }));

    Ok(Json(StrategyActionResponse {
        success: true,
        strategy_id: id.clone(),
        action: "update_risk".to_string(),
        message: format!("Strategy '{}' risk settings updated successfully", id),
    }))
}

/// 전략 심볼 변경.
///
/// PUT /api/v1/strategies/{id}/symbols
#[utoipa::path(
    put,
    path = "/api/v1/strategies/{id}/symbols",
    tag = "strategies",
    params(("id" = String, Path, description = "전략 ID")),
    request_body = UpdateSymbolsRequest,
    responses(
        (status = 200, description = "심볼 변경 성공", body = StrategyActionResponse),
        (status = 404, description = "전략을 찾을 수 없음", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn update_symbols(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<UpdateSymbolsRequest>,
) -> Result<Json<StrategyActionResponse>, (StatusCode, Json<ApiError>)> {
    // DB가 연결된 경우에만 동작
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONNECTED", "Database not connected")),
        )
    })?;

    // DB에 심볼 업데이트
    StrategyRepository::update_symbols(pool, &id, request.symbols.clone())
        .await
        .map_err(|e| {
            tracing::error!("Failed to update strategy symbols: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to update symbols: {}", e),
                )),
            )
        })?;

    // 전략 이름 가져오기 (브로드캐스트용)
    let engine = state.strategy_engine.read().await;
    let (strategy_name, is_running) = engine
        .get_strategy_status(&id)
        .await
        .map(|s| (s.name, s.running))
        .unwrap_or_else(|_| (id.clone(), false));

    // WebSocket 브로드캐스트: 심볼 변경 알림
    state.broadcast(ServerMessage::StrategyUpdate(StrategyUpdateData {
        strategy_id: id.clone(),
        name: strategy_name,
        running: is_running,
        event: "symbols_updated".to_string(),
        data: Some(serde_json::json!({
            "symbols": request.symbols,
        })),
        timestamp: Utc::now().timestamp_millis(),
    }));

    Ok(Json(StrategyActionResponse {
        success: true,
        strategy_id: id.clone(),
        action: "update_symbols".to_string(),
        message: format!("Strategy '{}' symbols updated successfully", id),
    }))
}

/// 전략 거래소 계정 업데이트.
///
/// PUT /api/v1/strategies/{id}/credential
#[utoipa::path(
    put,
    path = "/api/v1/strategies/{id}/credential",
    tag = "strategies",
    params(("id" = String, Path, description = "전략 ID")),
    request_body = UpdateCredentialRequest,
    responses(
        (status = 200, description = "거래소 계정 업데이트 성공", body = StrategyActionResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn update_credential(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<UpdateCredentialRequest>,
) -> Result<Json<StrategyActionResponse>, (StatusCode, Json<ApiError>)> {
    // DB가 연결된 경우에만 동작
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONNECTED", "Database not connected")),
        )
    })?;

    // credential_id 업데이트
    if let Some(ref cred_id_str) = request.credential_id {
        let credential_uuid = Uuid::parse_str(cred_id_str).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("INVALID_UUID", "Invalid credential_id format")),
            )
        })?;

        StrategyRepository::update_credential(pool, &id, Some(credential_uuid))
            .await
            .map_err(|e| {
                tracing::error!("Failed to update strategy credential: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new(
                        "DB_ERROR",
                        format!("Failed to update credential: {}", e),
                    )),
                )
            })?;
    } else {
        // credential_id가 None이면 연결 해제
        StrategyRepository::update_credential(pool, &id, None)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update strategy credential: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new(
                        "DB_ERROR",
                        format!("Failed to update credential: {}", e),
                    )),
                )
            })?;
    }

    // market 업데이트 (제공된 경우)
    if let Some(ref market) = request.market {
        StrategyRepository::update_market(pool, &id, market)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update strategy market: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new(
                        "DB_ERROR",
                        format!("Failed to update market: {}", e),
                    )),
                )
            })?;
    }

    // 전략 이름 가져오기 (브로드캐스트용)
    let engine = state.strategy_engine.read().await;
    let (strategy_name, is_running) = engine
        .get_strategy_status(&id)
        .await
        .map(|s| (s.name, s.running))
        .unwrap_or_else(|_| (id.clone(), false));

    // WebSocket 브로드캐스트: 거래소 계정 변경 알림
    state.broadcast(ServerMessage::StrategyUpdate(StrategyUpdateData {
        strategy_id: id.clone(),
        name: strategy_name,
        running: is_running,
        event: "credential_updated".to_string(),
        data: Some(serde_json::json!({
            "credential_id": request.credential_id,
            "market": request.market,
        })),
        timestamp: Utc::now().timestamp_millis(),
    }));

    Ok(Json(StrategyActionResponse {
        success: true,
        strategy_id: id.clone(),
        action: "update_credential".to_string(),
        message: format!("Strategy '{}' credential updated successfully", id),
    }))
}

/// 전략 복사 (파생 전략 생성).
///
/// POST /api/v1/strategies/{id}/clone
#[utoipa::path(
    post,
    path = "/api/v1/strategies/{id}/clone",
    tag = "strategies",
    params(("id" = String, Path, description = "원본 전략 ID")),
    request_body = CloneStrategyRequest,
    responses(
        (status = 200, description = "전략 복사 성공", body = CloneStrategyResponse),
        (status = 404, description = "원본 전략을 찾을 수 없음", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn clone_strategy(
    State(state): State<Arc<AppState>>,
    Path(source_id): Path<String>,
    Json(request): Json<CloneStrategyRequest>,
) -> Result<Json<CloneStrategyResponse>, (StatusCode, Json<ApiError>)> {
    // DB가 연결된 경우에만 동작
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_NOT_CONNECTED", "Database not connected")),
        )
    })?;

    // 원본 전략 조회
    let source = StrategyRepository::get_by_id(pool, &source_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to get source strategy: {}", e),
                )),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiError::new(
                    "STRATEGY_NOT_FOUND",
                    format!("Strategy '{}' not found", source_id),
                )),
            )
        })?;

    // 전략 타입 확인
    let strategy_type = source
        .strategy_type
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    // 새 전략 ID 생성
    let new_id = format!("{}_{}", strategy_type, &Uuid::new_v4().to_string()[..8]);

    // 파라미터 병합 (원본 + 오버라이드)
    let merged_config = if let Some(override_params) = request.override_params {
        let mut base = source.config.clone();
        if let (Some(base_obj), Some(override_obj)) =
            (base.as_object_mut(), override_params.as_object())
        {
            for (key, value) in override_obj {
                base_obj.insert(key.clone(), value.clone());
            }
        }
        base
    } else {
        source.config.clone()
    };

    // 리스크 설정 병합
    let merged_risk = request
        .override_risk_config
        .unwrap_or(source.risk_limits.clone());

    // 할당 자본 설정
    let allocated_capital = request
        .override_allocated_capital
        .map(|v| Decimal::try_from(v).unwrap_or(Decimal::ZERO))
        .or(source.allocated_capital);

    // 심볼 목록 추출
    let symbols: Vec<String> = source
        .symbols
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // 새 전략 생성 (원본 전략의 credential_id 복사)
    let input = CreateStrategyInput {
        id: new_id.clone(),
        name: request.new_name.clone(),
        description: source.description.clone(),
        strategy_type: strategy_type.clone(),
        symbols,
        market: source.market.clone().unwrap_or_else(|| "KR".to_string()),
        timeframe: source.timeframe.clone().unwrap_or_else(|| "1d".to_string()),
        config: merged_config.clone(),
        risk_config: Some(merged_risk),
        allocated_capital,
        risk_profile: source.risk_profile.clone(),
        multi_timeframe_config: source.multi_timeframe_config.clone(),
        credential_id: source.credential_id,
    };

    StrategyRepository::create(pool, input).await.map_err(|e| {
        tracing::error!("Failed to create cloned strategy: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_ERROR",
                format!("Failed to create cloned strategy: {}", e),
            )),
        )
    })?;

    // 전략 인스턴스 생성 및 엔진에 등록 (공유 StrategyContext 전달)
    if let Ok(strategy) = create_strategy_instance(&strategy_type) {
        let engine = state.strategy_engine.read().await;
        let _ = engine
            .register_strategy(
                &new_id,
                strategy,
                merged_config,
                Some(request.new_name.clone()),
                state.strategy_context.clone(),
            )
            .await;
    }

    // WebSocket 브로드캐스트: 전략 복사 알림
    state.broadcast(ServerMessage::StrategyUpdate(StrategyUpdateData {
        strategy_id: new_id.clone(),
        name: request.new_name.clone(),
        running: false,
        event: "cloned".to_string(),
        data: Some(serde_json::json!({
            "source_id": source_id,
            "strategy_type": strategy_type,
        })),
        timestamp: Utc::now().timestamp_millis(),
    }));

    Ok(Json(CloneStrategyResponse {
        success: true,
        source_id: source_id.clone(),
        new_id: new_id.clone(),
        new_name: request.new_name,
        message: format!(
            "Strategy '{}' cloned to '{}' successfully",
            source_id, new_id
        ),
    }))
}

/// 엔진 통계 조회.
///
/// GET /api/v1/strategies/stats
#[utoipa::path(
    get,
    path = "/api/v1/strategies/stats",
    tag = "strategies",
    responses(
        (status = 200, description = "엔진 통계 조회 성공", body = EngineStatsResponse)
    )
)]
pub async fn get_engine_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let engine = state.strategy_engine.read().await;
    let stats = engine.get_engine_stats().await;

    Json(EngineStatsResponse::from(stats))
}

// ==================== 다중 타임프레임 ====================

/// 타임프레임 설정 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct TimeframeConfigResponse {
    /// 전략 ID
    pub strategy_id: String,
    /// 기본 타임프레임
    pub primary_timeframe: String,
    /// 다중 타임프레임 여부
    pub is_multi_timeframe: bool,
    /// 다중 타임프레임 설정 (NULL이면 단일 TF)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub multi_timeframe_config: Option<Value>,
    /// Secondary 타임프레임 목록 (편의용)
    pub secondary_timeframes: Vec<String>,
}

/// 타임프레임 설정 업데이트 요청.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTimeframeConfigRequest {
    /// 다중 타임프레임 설정
    /// 형식: {"primary": "5m", "secondary": [{"timeframe": "1h", "candle_count": 24}]}
    #[serde(rename = "multiTimeframeConfig")]
    #[schema(value_type = Option<Object>)]
    pub multi_timeframe_config: Option<Value>,
}

/// 전략 타임프레임 설정 조회.
#[utoipa::path(
    get,
    path = "/api/v1/strategies/{id}/timeframes",
    tag = "strategies",
    params(
        ("id" = String, Path, description = "전략 ID")
    ),
    responses(
        (status = 200, description = "타임프레임 설정 조회 성공", body = TimeframeConfigResponse),
        (status = 404, description = "전략 없음", body = ApiError),
        (status = 503, description = "DB 미사용 가능", body = ApiError)
    )
)]
pub async fn get_strategy_timeframes(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<TimeframeConfigResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("DB_UNAVAILABLE", "Database not available")),
        )
    })?;

    let record = StrategyRepository::get_by_id(pool, &id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get strategy: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("Database error: {}", e))),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiError::new(
                    "NOT_FOUND",
                    format!("Strategy not found: {}", id),
                )),
            )
        })?;

    // Secondary 타임프레임 목록 추출
    let secondary_timeframes: Vec<String> = record
        .multi_timeframe_config
        .as_ref()
        .and_then(|config| config.get("secondary"))
        .and_then(|sec| sec.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.get("timeframe").and_then(|tf| tf.as_str()))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let is_multi_tf = record.multi_timeframe_config.is_some() && !secondary_timeframes.is_empty();

    Ok(Json(TimeframeConfigResponse {
        strategy_id: id,
        primary_timeframe: record.timeframe.unwrap_or_else(|| "1d".to_string()),
        is_multi_timeframe: is_multi_tf,
        multi_timeframe_config: record.multi_timeframe_config,
        secondary_timeframes,
    }))
}

/// 전략 타임프레임 설정 업데이트.
#[utoipa::path(
    put,
    path = "/api/v1/strategies/{id}/timeframes",
    tag = "strategies",
    params(
        ("id" = String, Path, description = "전략 ID")
    ),
    request_body = UpdateTimeframeConfigRequest,
    responses(
        (status = 200, description = "타임프레임 설정 업데이트 성공", body = TimeframeConfigResponse),
        (status = 400, description = "잘못된 요청", body = ApiError),
        (status = 404, description = "전략 없음", body = ApiError),
        (status = 503, description = "DB 미사용 가능", body = ApiError)
    )
)]
pub async fn update_strategy_timeframes(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<UpdateTimeframeConfigRequest>,
) -> Result<Json<TimeframeConfigResponse>, (StatusCode, Json<ApiError>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("DB_UNAVAILABLE", "Database not available")),
        )
    })?;

    // 전략 존재 확인
    let exists = StrategyRepository::exists(pool, &id).await.map_err(|e| {
        tracing::error!("Failed to check strategy existence: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("Database error: {}", e))),
        )
    })?;

    if !exists {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new(
                "NOT_FOUND",
                format!("Strategy not found: {}", id),
            )),
        ));
    }

    // 설정 유효성 검증
    if let Some(ref config) = request.multi_timeframe_config {
        // primary 필드 확인
        if config.get("primary").is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "VALIDATION_ERROR",
                    "multi_timeframe_config.primary is required",
                )),
            ));
        }

        // secondary 필드 확인 (배열이어야 함)
        if let Some(secondary) = config.get("secondary") {
            if !secondary.is_array() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "VALIDATION_ERROR",
                        "multi_timeframe_config.secondary must be an array",
                    )),
                ));
            }
        }
    }

    // DB 업데이트
    let record = sqlx::query_as::<_, crate::repository::strategies::StrategyRecord>(
        r#"
        UPDATE strategies
        SET multi_timeframe_config = $2, updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(&id)
    .bind(&request.multi_timeframe_config)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update timeframe config: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_ERROR",
                format!("Failed to update: {}", e),
            )),
        )
    })?;

    // Secondary 타임프레임 목록 추출
    let secondary_timeframes: Vec<String> = record
        .multi_timeframe_config
        .as_ref()
        .and_then(|config| config.get("secondary"))
        .and_then(|sec| sec.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.get("timeframe").and_then(|tf| tf.as_str()))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let is_multi_tf = record.multi_timeframe_config.is_some() && !secondary_timeframes.is_empty();

    tracing::info!(
        "Updated timeframe config for strategy {}: is_multi_tf={}",
        id,
        is_multi_tf
    );

    Ok(Json(TimeframeConfigResponse {
        strategy_id: id,
        primary_timeframe: record.timeframe.unwrap_or_else(|| "1d".to_string()),
        is_multi_timeframe: is_multi_tf,
        multi_timeframe_config: record.multi_timeframe_config,
        secondary_timeframes,
    }))
}

// ==================== router ====================

/// 전략 관리 라우터 생성.
pub fn strategies_router() -> Router<Arc<AppState>> {
    use super::schema::{get_strategy_schema, list_strategy_meta};

    Router::new()
        // 목록, 생성, 통계
        .route("/", get(list_strategies).post(create_strategy))
        .route("/stats", get(get_engine_stats))
        // 전략 메타데이터 (SDUI 스키마용)
        .route("/meta", get(list_strategy_meta))
        // 개별 전략 조작
        .route("/{id}", get(get_strategy).delete(delete_strategy))
        .route("/{id}/start", post(start_strategy))
        .route("/{id}/stop", post(stop_strategy))
        .route("/{id}/config", put(update_config))
        .route("/{id}/risk", put(update_risk_settings))
        .route("/{id}/symbols", put(update_symbols))
        .route("/{id}/credential", put(update_credential))
        .route("/{id}/clone", post(clone_strategy))
        // 전략 스키마 (SDUI)
        .route("/{id}/schema", get(get_strategy_schema))
        // 다중 타임프레임 설정
        .route("/{id}/timeframes", get(get_strategy_timeframes).put(update_strategy_timeframes))
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
    async fn test_list_strategies_empty() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/strategies", get(list_strategies))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/strategies")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list: StrategiesListResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(list.total, 0);
        assert_eq!(list.running, 0);
        assert!(list.strategies.is_empty());
    }

    #[tokio::test]
    async fn test_get_strategy_not_found() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/strategies/{id}", get(get_strategy))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/strategies/nonexistent")
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

        assert_eq!(error.code, "STRATEGY_NOT_FOUND");
    }

    #[tokio::test]
    async fn test_start_strategy_not_found() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/strategies/{id}/start", post(start_strategy))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/strategies/nonexistent/start")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_engine_stats() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/strategies/stats", get(get_engine_stats))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/strategies/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let stats: EngineStatsResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(stats.total_strategies, 0);
        assert_eq!(stats.running_strategies, 0);
    }

    #[test]
    fn test_api_error_creation() {
        let error = ApiError::new("TEST_ERROR", "Test message");
        assert_eq!(error.code, "TEST_ERROR");
        assert_eq!(error.message, "Test message");
    }

    #[test]
    fn test_engine_stats_conversion() {
        let stats = EngineStats {
            total_strategies: 5,
            running_strategies: 2,
            total_signals_generated: 100,
            total_orders_filled: 50,
            total_market_data_processed: 1000,
        };

        let response: EngineStatsResponse = stats.into();
        assert_eq!(response.total_strategies, 5);
        assert_eq!(response.running_strategies, 2);
        assert_eq!(response.total_signals_generated, 100);
    }
}
