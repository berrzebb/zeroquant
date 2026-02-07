//! 매매일지 API 엔드포인트.
//!
//! 체결 내역, 포지션 현황, 손익 분석을 위한 REST API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/journal/positions` - 보유 현황 조회
//! - `GET /api/v1/journal/executions` - 체결 내역 조회
//! - `GET /api/v1/journal/pnl` - 손익 요약 조회
//! - `GET /api/v1/journal/pnl/daily` - 일별 손익 조회
//! - `GET /api/v1/journal/pnl/symbol` - 종목별 손익 조회
//! - `POST /api/v1/journal/sync` - 거래소 체결 내역 동기화
//! - `PATCH /api/v1/journal/executions/{id}` - 체결 내역 메모/태그 수정
//! - `GET /api/v1/journal/cost-basis/{symbol}` - FIFO 원가 계산

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, patch, post},
    Json, Router,
};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use trader_core::Side;
use ts_rs::TS;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// ==================== 날짜 파싱 헬퍼 ====================

/// 날짜/시간 파싱 에러.
#[derive(Debug)]
pub struct DateParseError {
    pub field: String,
    pub value: String,
    pub expected_formats: Vec<&'static str>,
}

impl DateParseError {
    fn new(field: &str, value: &str, expected_formats: Vec<&'static str>) -> Self {
        Self {
            field: field.to_string(),
            value: value.to_string(),
            expected_formats,
        }
    }

    fn to_api_error(&self) -> ApiError {
        ApiError::new(
            "INVALID_DATE_FORMAT",
            format!(
                "'{}' 필드의 날짜 형식이 올바르지 않습니다: '{}'. 허용 형식: {}",
                self.field,
                self.value,
                self.expected_formats.join(", ")
            ),
        )
    }
}

/// DateTime<Utc>로 유연하게 파싱합니다.
///
/// 지원 형식:
/// - RFC 3339: `2024-01-15T09:30:00Z`, `2024-01-15T09:30:00+09:00`
/// - ISO 8601 날짜만: `2024-01-15` (00:00:00 UTC로 변환)
/// - 슬래시 구분: `2024/01/15`
fn parse_datetime_flexible(s: &str, field_name: &str) -> Result<DateTime<Utc>, DateParseError> {
    const FORMATS: &[&str] = &[
        "RFC3339 (2024-01-15T09:30:00Z)",
        "YYYY-MM-DD (2024-01-15)",
        "YYYY/MM/DD (2024/01/15)",
    ];

    // RFC3339 먼저 시도
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    // YYYY-MM-DD 형식
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()));
    }

    // YYYY/MM/DD 형식
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y/%m/%d") {
        return Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()));
    }

    Err(DateParseError::new(field_name, s, FORMATS.to_vec()))
}

/// NaiveDate로 유연하게 파싱합니다.
///
/// 지원 형식:
/// - ISO 8601: `2024-01-15`
/// - 슬래시 구분: `2024/01/15`
/// - 한국식: `20240115` (YYYYMMDD)
fn parse_date_flexible(s: &str, field_name: &str) -> Result<NaiveDate, DateParseError> {
    const FORMATS: &[&str] = &[
        "YYYY-MM-DD (2024-01-15)",
        "YYYY/MM/DD (2024/01/15)",
        "YYYYMMDD (20240115)",
    ];

    // YYYY-MM-DD 형식
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(date);
    }

    // YYYY/MM/DD 형식
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y/%m/%d") {
        return Ok(date);
    }

    // YYYYMMDD 형식 (KIS API에서 사용)
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y%m%d") {
        return Ok(date);
    }

    Err(DateParseError::new(field_name, s, FORMATS.to_vec()))
}

use crate::repository::{
    build_tracker_from_executions, create_exchange_providers_from_credential,
    create_provider_for_mock_credential, CostBasisSummary, CumulativePnL,
    CurrentPosition as RepoCurrentPosition, DailySummary, EquityHistoryRepository,
    ExecutionCacheRepository, ExecutionFilter, JournalRepository, MonthlyPnL, NewExecution,
    PnLSummary, PositionRepository, StrategyPerformance, SymbolPnL, TradeExecution,
    TradeExecutionRecord, TradingInsights, WeeklyPnL, YearlyPnL,
};
use crate::routes::strategies::ApiError;
use crate::state::AppState;
use tracing::{debug, error, warn};

// ==================== 요청 타입 ====================

/// 체결 내역 조회 쿼리 파라미터.
#[derive(Debug, Deserialize, TS, ToSchema, IntoParams)]
#[ts(export, export_to = "journal/")]
pub struct ListExecutionsQuery {
    /// 종목 필터
    pub symbol: Option<String>,
    /// 매수/매도 필터 (buy, sell)
    pub side: Option<String>,
    /// 전략 ID 필터
    pub strategy_id: Option<String>,
    /// 시작 날짜 (ISO 8601)
    pub start_date: Option<String>,
    /// 종료 날짜 (ISO 8601)
    pub end_date: Option<String>,
    /// 페이지 크기 (기본 50)
    /// NOTE: i32 사용 (ts-rs에서 number로 변환, bigint 대신)
    pub limit: Option<i32>,
    /// 오프셋 (기본 0)
    /// NOTE: i32 사용 (ts-rs에서 number로 변환, bigint 대신)
    pub offset: Option<i32>,
}

/// 일별 손익 조회 쿼리 파라미터.
#[derive(Debug, Deserialize, TS, ToSchema, IntoParams)]
#[ts(export, export_to = "journal/")]
pub struct DailyPnLQuery {
    /// 시작 날짜 (YYYY-MM-DD)
    pub start_date: Option<String>,
    /// 종료 날짜 (YYYY-MM-DD)
    pub end_date: Option<String>,
}

/// 체결 내역 수정 요청.
#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export, export_to = "journal/")]
pub struct UpdateExecutionRequest {
    /// 메모
    pub memo: Option<String>,
    /// 태그 목록
    pub tags: Option<Vec<String>>,
}

/// 동기화 요청.
#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export, export_to = "journal/")]
pub struct SyncRequest {
    /// 동기화할 거래소 (선택적, 기본값은 활성 계정의 거래소)
    pub exchange: Option<String>,
    /// 시작 날짜 (선택적)
    pub start_date: Option<String>,
    /// 강제 전체 동기화 (캐시 초기화 후 전체 내역 조회)
    #[serde(default)]
    pub force_full_sync: bool,
}

// ==================== 응답 타입 ====================

/// 포지션 목록 응답.
#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export, export_to = "journal/")]
pub struct JournalPositionsResponse {
    /// 포지션 목록
    pub positions: Vec<JournalPositionResponse>,
    /// 전체 포지션 수
    pub total: usize,
    /// 요약
    pub summary: PositionsSummary,
}

/// 포지션 응답.
#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export, export_to = "journal/")]
pub struct JournalPositionResponse {
    pub id: String,
    pub exchange: String,
    pub symbol: String,
    pub symbol_name: Option<String>,
    pub side: String,
    pub quantity: String,
    pub entry_price: String,
    pub current_price: Option<String>,
    pub cost_basis: String,
    pub market_value: Option<String>,
    pub unrealized_pnl: Option<String>,
    pub unrealized_pnl_pct: Option<String>,
    pub realized_pnl: Option<String>,
    pub weight_pct: Option<String>,
    pub first_trade_at: Option<String>,
    pub last_trade_at: Option<String>,
    pub trade_count: Option<i32>,
    pub strategy_id: Option<String>,
    pub snapshot_time: String,
}

impl From<RepoCurrentPosition> for JournalPositionResponse {
    fn from(p: RepoCurrentPosition) -> Self {
        Self {
            id: p.id.to_string(),
            exchange: p.exchange,
            symbol: p.symbol,
            symbol_name: p.symbol_name,
            side: p.side.as_str().to_string(),
            quantity: p.quantity.to_string(),
            entry_price: p.entry_price.to_string(),
            current_price: p.current_price.map(|v| v.to_string()),
            cost_basis: p.cost_basis.to_string(),
            market_value: p.market_value.map(|v| v.to_string()),
            unrealized_pnl: p.unrealized_pnl.map(|v| v.to_string()),
            unrealized_pnl_pct: p.unrealized_pnl_pct.map(|v| format!("{:.2}", v)),
            realized_pnl: p.realized_pnl.map(|v| v.to_string()),
            weight_pct: p.weight_pct.map(|v| format!("{:.2}", v)),
            first_trade_at: p.first_trade_at.map(|t| t.to_rfc3339()),
            last_trade_at: p.last_trade_at.map(|t| t.to_rfc3339()),
            trade_count: p.trade_count,
            strategy_id: p.strategy_id,
            snapshot_time: p.snapshot_time.to_rfc3339(),
        }
    }
}

/// 포지션 요약.
#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export, export_to = "journal/")]
pub struct PositionsSummary {
    pub total_positions: usize,
    pub total_cost_basis: String,
    pub total_market_value: String,
    pub total_unrealized_pnl: String,
    pub total_unrealized_pnl_pct: String,
}

/// 체결 내역 목록 응답.
#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export, export_to = "journal/")]
pub struct ExecutionsListResponse {
    pub executions: Vec<ExecutionResponse>,
    /// NOTE: i32 사용 (ts-rs에서 number로 변환, bigint 대신)
    pub total: i32,
    /// NOTE: i32 사용 (ts-rs에서 number로 변환, bigint 대신)
    pub limit: i32,
    /// NOTE: i32 사용 (ts-rs에서 number로 변환, bigint 대신)
    pub offset: i32,
}

/// 체결 내역 응답.
#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export, export_to = "journal/")]
pub struct ExecutionResponse {
    pub id: String,
    pub exchange: String,
    pub symbol: String,
    pub symbol_name: Option<String>,
    pub side: String,
    pub order_type: String,
    pub quantity: String,
    pub price: String,
    pub notional_value: String,
    pub fee: Option<String>,
    pub fee_currency: Option<String>,
    pub position_effect: Option<String>,
    pub realized_pnl: Option<String>,
    pub strategy_id: Option<String>,
    pub strategy_name: Option<String>,
    pub executed_at: String,
    pub memo: Option<String>,
    pub tags: Option<Vec<String>>,
}

impl From<TradeExecutionRecord> for ExecutionResponse {
    fn from(r: TradeExecutionRecord) -> Self {
        let tags: Option<Vec<String>> = r.tags.and_then(|v| serde_json::from_value(v).ok());

        Self {
            id: r.id.to_string(),
            exchange: r.exchange,
            symbol: r.symbol,
            symbol_name: r.symbol_name,
            side: r.side.as_str().to_string(),
            order_type: r.order_type,
            quantity: r.quantity.to_string(),
            price: r.price.to_string(),
            notional_value: r.notional_value.to_string(),
            fee: r.fee.map(|v| v.to_string()),
            fee_currency: r.fee_currency,
            position_effect: r.position_effect,
            realized_pnl: r.realized_pnl.map(|v| v.to_string()),
            strategy_id: r.strategy_id,
            strategy_name: r.strategy_name,
            executed_at: r.executed_at.to_rfc3339(),
            memo: r.memo,
            tags,
        }
    }
}

/// PnL 요약 응답.
#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export, export_to = "journal/")]
pub struct PnLSummaryResponse {
    pub total_realized_pnl: String,
    pub total_fees: String,
    pub net_pnl: String,
    pub total_trades: i64,
    pub buy_trades: i64,
    pub sell_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub win_rate: String,
    pub total_volume: String,
    pub first_trade_at: Option<String>,
    pub last_trade_at: Option<String>,
}

impl From<PnLSummary> for PnLSummaryResponse {
    fn from(s: PnLSummary) -> Self {
        let net_pnl = s.total_realized_pnl - s.total_fees;
        let winning = s.winning_trades.unwrap_or(0);
        let losing = s.losing_trades.unwrap_or(0);
        let total_closed = winning + losing;
        let win_rate = if total_closed > 0 {
            (winning as f64 / total_closed as f64) * 100.0
        } else {
            0.0
        };

        Self {
            total_realized_pnl: s.total_realized_pnl.to_string(),
            total_fees: s.total_fees.to_string(),
            net_pnl: net_pnl.to_string(),
            total_trades: s.total_trades,
            buy_trades: s.buy_trades.unwrap_or(0),
            sell_trades: s.sell_trades.unwrap_or(0),
            winning_trades: winning,
            losing_trades: losing,
            win_rate: format!("{:.2}", win_rate),
            total_volume: s.total_volume.to_string(),
            first_trade_at: s.first_trade_at.map(|t| t.to_rfc3339()),
            last_trade_at: s.last_trade_at.map(|t| t.to_rfc3339()),
        }
    }
}

/// 일별 손익 응답.
#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export, export_to = "journal/")]
pub struct DailyPnLResponse {
    pub daily: Vec<DailyPnLItem>,
    pub total_days: usize,
}

/// 일별 손익 항목.
#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export, export_to = "journal/")]
pub struct DailyPnLItem {
    pub date: String,
    pub total_trades: i64,
    pub buy_count: i64,
    pub sell_count: i64,
    pub total_volume: String,
    pub total_fees: String,
    pub realized_pnl: String,
    pub symbol_count: i64,
}

impl From<DailySummary> for DailyPnLItem {
    fn from(s: DailySummary) -> Self {
        Self {
            date: s.trade_date.to_string(),
            total_trades: s.total_trades,
            buy_count: s.buy_count.unwrap_or(0),
            sell_count: s.sell_count.unwrap_or(0),
            total_volume: s.total_volume.unwrap_or(Decimal::ZERO).to_string(),
            total_fees: s.total_fees.unwrap_or(Decimal::ZERO).to_string(),
            realized_pnl: s.realized_pnl.unwrap_or(Decimal::ZERO).to_string(),
            symbol_count: s.symbol_count.unwrap_or(0),
        }
    }
}

/// 종목별 손익 응답.
#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export, export_to = "journal/")]
pub struct SymbolPnLResponse {
    pub symbols: Vec<SymbolPnLItem>,
    pub total: usize,
}

/// 종목별 손익 항목.
#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export, export_to = "journal/")]
pub struct SymbolPnLItem {
    pub symbol: String,
    pub symbol_name: Option<String>,
    pub total_trades: i64,
    pub total_buy_qty: String,
    pub total_sell_qty: String,
    pub total_buy_value: String,
    pub total_sell_value: String,
    pub total_fees: String,
    pub realized_pnl: String,
    pub first_trade_at: Option<String>,
    pub last_trade_at: Option<String>,
}

impl From<SymbolPnL> for SymbolPnLItem {
    fn from(s: SymbolPnL) -> Self {
        Self {
            symbol: s.symbol,
            symbol_name: s.symbol_name,
            total_trades: s.total_trades,
            total_buy_qty: s.total_buy_qty.unwrap_or(Decimal::ZERO).to_string(),
            total_sell_qty: s.total_sell_qty.unwrap_or(Decimal::ZERO).to_string(),
            total_buy_value: s.total_buy_value.unwrap_or(Decimal::ZERO).to_string(),
            total_sell_value: s.total_sell_value.unwrap_or(Decimal::ZERO).to_string(),
            total_fees: s.total_fees.unwrap_or(Decimal::ZERO).to_string(),
            realized_pnl: s.realized_pnl.unwrap_or(Decimal::ZERO).to_string(),
            first_trade_at: s.first_trade_at.map(|t| t.to_rfc3339()),
            last_trade_at: s.last_trade_at.map(|t| t.to_rfc3339()),
        }
    }
}

/// 동기화 응답.
#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export, export_to = "journal/")]
pub struct SyncResponse {
    pub success: bool,
    pub inserted: i32,
    pub skipped: i32,
    pub message: String,
}

// ==================== 핸들러 ====================

/// 보유 현황(포지션) 조회.
///
/// positions 테이블에서 현재 열린 포지션을 조회합니다.
/// Dashboard의 holdings API 호출 시 자동으로 동기화됩니다.
#[utoipa::path(
    get,
    path = "/api/v1/journal/positions",
    tag = "journal",
    responses(
        (status = 200, description = "포지션 조회 성공", body = JournalPositionsResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn get_journal_positions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<JournalPositionsResponse>, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    // PositionRepository에서 열린 포지션 조회
    let positions = PositionRepository::get_open_positions_by_credential(pool, credential_id)
        .await
        .map_err(|e| {
            warn!("Failed to get positions: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to get positions: {}", e),
                )),
            )
        })?;

    // PositionRecord를 JournalPositionResponse로 변환
    let now = Utc::now();
    let position_responses: Vec<JournalPositionResponse> = positions
        .iter()
        .map(|p| {
            let cost_basis = p.entry_price * p.quantity;
            let market_value = p.current_price.map(|cp| cp * p.quantity);
            let unrealized_pnl = p.unrealized_pnl;
            let unrealized_pnl_pct = if cost_basis > Decimal::ZERO {
                unrealized_pnl.map(|pnl| (pnl / cost_basis) * Decimal::from(100))
            } else {
                None
            };

            JournalPositionResponse {
                id: p.id.to_string(),
                exchange: p.exchange.clone(),
                symbol: p.symbol.clone().unwrap_or_default(),
                symbol_name: p.symbol_name.clone(),
                side: p.side.as_str().to_string(),
                quantity: p.quantity.to_string(),
                entry_price: p.entry_price.to_string(),
                current_price: p.current_price.map(|v| v.to_string()),
                cost_basis: cost_basis.to_string(),
                market_value: market_value.map(|v| v.to_string()),
                unrealized_pnl: unrealized_pnl.map(|v| v.to_string()),
                unrealized_pnl_pct: unrealized_pnl_pct.map(|v| format!("{:.2}", v)),
                realized_pnl: p.realized_pnl.map(|v| v.to_string()),
                weight_pct: None, // 비중 계산은 별도로 필요
                first_trade_at: p.opened_at.map(|t| t.to_rfc3339()),
                last_trade_at: p.updated_at.map(|t| t.to_rfc3339()),
                trade_count: None,
                strategy_id: p.strategy_id.clone(),
                snapshot_time: now.to_rfc3339(),
            }
        })
        .collect();

    // 요약 계산
    let total_cost_basis: Decimal = positions.iter().map(|p| p.entry_price * p.quantity).sum();
    let total_market_value: Decimal = positions
        .iter()
        .filter_map(|p| p.current_price.map(|cp| cp * p.quantity))
        .sum();
    let total_unrealized_pnl: Decimal = positions.iter().filter_map(|p| p.unrealized_pnl).sum();
    let total_unrealized_pnl_pct = if total_cost_basis > Decimal::ZERO {
        (total_unrealized_pnl / total_cost_basis) * Decimal::from(100)
    } else {
        Decimal::ZERO
    };

    let total = positions.len();

    Ok(Json(JournalPositionsResponse {
        positions: position_responses,
        total,
        summary: PositionsSummary {
            total_positions: total,
            total_cost_basis: total_cost_basis.to_string(),
            total_market_value: total_market_value.to_string(),
            total_unrealized_pnl: total_unrealized_pnl.to_string(),
            total_unrealized_pnl_pct: format!("{:.2}", total_unrealized_pnl_pct),
        },
    }))
}

/// 체결 내역 조회.
#[utoipa::path(
    get,
    path = "/api/v1/journal/executions",
    tag = "journal",
    params(ListExecutionsQuery),
    responses(
        (status = 200, description = "체결 내역 조회 성공", body = ExecutionsListResponse),
        (status = 400, description = "잘못된 요청", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn list_executions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListExecutionsQuery>,
) -> Result<Json<ExecutionsListResponse>, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    // 날짜 파싱 - 유연한 형식 지원 + 에러 반환
    let start_date: Option<DateTime<Utc>> = match query.start_date {
        Some(ref s) => Some(
            parse_datetime_flexible(s, "start_date")
                .map_err(|e| (StatusCode::BAD_REQUEST, Json(e.to_api_error())))?,
        ),
        None => None,
    };

    let end_date: Option<DateTime<Utc>> = match query.end_date {
        Some(ref s) => Some(
            parse_datetime_flexible(s, "end_date")
                .map_err(|e| (StatusCode::BAD_REQUEST, Json(e.to_api_error())))?,
        ),
        None => None,
    };

    let filter = ExecutionFilter {
        symbol: query.symbol,
        side: query.side,
        strategy_id: query.strategy_id,
        start_date,
        end_date,
        limit: Some(limit as i64),
        offset: Some(offset as i64),
    };

    let executions = JournalRepository::list_executions(pool, credential_id, filter.clone())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to list executions: {}", e),
                )),
            )
        })?;

    let total = JournalRepository::count_executions(pool, credential_id, filter)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to count executions: {}", e),
                )),
            )
        })?;

    let execution_responses: Vec<ExecutionResponse> =
        executions.into_iter().map(Into::into).collect();

    Ok(Json(ExecutionsListResponse {
        executions: execution_responses,
        total: total as i32,
        limit,
        offset,
    }))
}

/// PnL 요약 조회.
#[utoipa::path(
    get,
    path = "/api/v1/journal/pnl",
    tag = "journal",
    responses(
        (status = 200, description = "손익 요약 조회 성공", body = PnLSummaryResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn get_pnl_summary(
    State(state): State<Arc<AppState>>,
) -> Result<Json<PnLSummaryResponse>, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    let summary = JournalRepository::get_total_pnl(pool, credential_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to get PnL summary: {}", e),
                )),
            )
        })?;

    Ok(Json(summary.into()))
}

/// 일별 손익 조회.
#[utoipa::path(
    get,
    path = "/api/v1/journal/pnl/daily",
    tag = "journal",
    params(DailyPnLQuery),
    responses(
        (status = 200, description = "일별 손익 조회 성공", body = DailyPnLResponse),
        (status = 400, description = "잘못된 요청", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn get_daily_pnl(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DailyPnLQuery>,
) -> Result<Json<DailyPnLResponse>, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    // 날짜 파싱 - 유연한 형식 지원 + 에러 반환
    let start_date: Option<NaiveDate> = match query.start_date {
        Some(ref s) => Some(
            parse_date_flexible(s, "start_date")
                .map_err(|e| (StatusCode::BAD_REQUEST, Json(e.to_api_error())))?,
        ),
        None => None,
    };

    let end_date: Option<NaiveDate> = match query.end_date {
        Some(ref s) => Some(
            parse_date_flexible(s, "end_date")
                .map_err(|e| (StatusCode::BAD_REQUEST, Json(e.to_api_error())))?,
        ),
        None => None,
    };

    let daily = JournalRepository::get_daily_summary(pool, credential_id, start_date, end_date)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to get daily PnL: {}", e),
                )),
            )
        })?;

    let total_days = daily.len();
    let daily_items: Vec<DailyPnLItem> = daily.into_iter().map(Into::into).collect();

    Ok(Json(DailyPnLResponse {
        daily: daily_items,
        total_days,
    }))
}

/// 종목별 손익 조회.
#[utoipa::path(
    get,
    path = "/api/v1/journal/pnl/symbol",
    tag = "journal",
    responses(
        (status = 200, description = "종목별 손익 조회 성공", body = SymbolPnLResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn get_symbol_pnl(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SymbolPnLResponse>, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    let symbols = JournalRepository::get_symbol_pnl(pool, credential_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to get symbol PnL: {}", e),
                )),
            )
        })?;

    let total = symbols.len();
    let symbol_items: Vec<SymbolPnLItem> = symbols.into_iter().map(Into::into).collect();

    Ok(Json(SymbolPnLResponse {
        symbols: symbol_items,
        total,
    }))
}

// ==================== 기간별 손익 API ====================

/// 주별 손익 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct WeeklyPnLResponse {
    pub weekly: Vec<WeeklyPnLItem>,
    pub total_weeks: usize,
}

/// 주별 손익 항목.
#[derive(Debug, Serialize, ToSchema)]
pub struct WeeklyPnLItem {
    pub week_start: String,
    pub total_trades: i64,
    pub buy_count: i64,
    pub sell_count: i64,
    pub total_volume: String,
    pub total_fees: String,
    pub realized_pnl: String,
    pub symbol_count: i64,
    pub trading_days: i64,
}

impl From<WeeklyPnL> for WeeklyPnLItem {
    fn from(w: WeeklyPnL) -> Self {
        Self {
            week_start: w.week_start.to_string(),
            total_trades: w.total_trades,
            buy_count: w.buy_count.unwrap_or(0),
            sell_count: w.sell_count.unwrap_or(0),
            total_volume: w.total_volume.unwrap_or(Decimal::ZERO).to_string(),
            total_fees: w.total_fees.unwrap_or(Decimal::ZERO).to_string(),
            realized_pnl: w.realized_pnl.unwrap_or(Decimal::ZERO).to_string(),
            symbol_count: w.symbol_count.unwrap_or(0),
            trading_days: w.trading_days.unwrap_or(0),
        }
    }
}

/// 월별 손익 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct MonthlyPnLResponse {
    pub monthly: Vec<MonthlyPnLItem>,
    pub total_months: usize,
}

/// 월별 손익 항목.
#[derive(Debug, Serialize, ToSchema)]
pub struct MonthlyPnLItem {
    pub year: i32,
    pub month: i32,
    pub total_trades: i64,
    pub buy_count: i64,
    pub sell_count: i64,
    pub total_volume: String,
    pub total_fees: String,
    pub realized_pnl: String,
    pub symbol_count: i64,
    pub trading_days: i64,
}

impl From<MonthlyPnL> for MonthlyPnLItem {
    fn from(m: MonthlyPnL) -> Self {
        Self {
            year: m.year,
            month: m.month,
            total_trades: m.total_trades,
            buy_count: m.buy_count.unwrap_or(0),
            sell_count: m.sell_count.unwrap_or(0),
            total_volume: m.total_volume.unwrap_or(Decimal::ZERO).to_string(),
            total_fees: m.total_fees.unwrap_or(Decimal::ZERO).to_string(),
            realized_pnl: m.realized_pnl.unwrap_or(Decimal::ZERO).to_string(),
            symbol_count: m.symbol_count.unwrap_or(0),
            trading_days: m.trading_days.unwrap_or(0),
        }
    }
}

/// 연도별 손익 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct YearlyPnLResponse {
    pub yearly: Vec<YearlyPnLItem>,
    pub total_years: usize,
}

/// 연도별 손익 항목.
#[derive(Debug, Serialize, ToSchema)]
pub struct YearlyPnLItem {
    pub year: i32,
    pub total_trades: i64,
    pub buy_count: i64,
    pub sell_count: i64,
    pub total_volume: String,
    pub total_fees: String,
    pub realized_pnl: String,
    pub symbol_count: i64,
    pub trading_days: i64,
    pub trading_months: i64,
}

impl From<YearlyPnL> for YearlyPnLItem {
    fn from(y: YearlyPnL) -> Self {
        Self {
            year: y.year,
            total_trades: y.total_trades,
            buy_count: y.buy_count.unwrap_or(0),
            sell_count: y.sell_count.unwrap_or(0),
            total_volume: y.total_volume.unwrap_or(Decimal::ZERO).to_string(),
            total_fees: y.total_fees.unwrap_or(Decimal::ZERO).to_string(),
            realized_pnl: y.realized_pnl.unwrap_or(Decimal::ZERO).to_string(),
            symbol_count: y.symbol_count.unwrap_or(0),
            trading_days: y.trading_days.unwrap_or(0),
            trading_months: y.trading_months.unwrap_or(0),
        }
    }
}

/// 누적 손익 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct CumulativePnLResponse {
    pub curve: Vec<CumulativePnLPoint>,
    pub total_points: usize,
}

/// 누적 손익 포인트.
#[derive(Debug, Serialize, ToSchema)]
pub struct CumulativePnLPoint {
    pub date: String,
    pub cumulative_pnl: String,
    pub cumulative_fees: String,
    pub cumulative_trades: i64,
    pub daily_pnl: String,
}

impl From<CumulativePnL> for CumulativePnLPoint {
    fn from(c: CumulativePnL) -> Self {
        Self {
            date: c.trade_date.to_string(),
            cumulative_pnl: c.cumulative_pnl.unwrap_or(Decimal::ZERO).to_string(),
            cumulative_fees: c.cumulative_fees.unwrap_or(Decimal::ZERO).to_string(),
            cumulative_trades: c.cumulative_trades.unwrap_or(0),
            daily_pnl: c.realized_pnl.unwrap_or(Decimal::ZERO).to_string(),
        }
    }
}

/// 투자 인사이트 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct TradingInsightsResponse {
    /// 총 거래 통계
    pub total_trades: i64,
    pub buy_trades: i64,
    pub sell_trades: i64,
    pub unique_symbols: i64,

    /// 손익 통계
    pub total_realized_pnl: String,
    pub total_fees: String,
    pub winning_trades: i64,
    pub losing_trades: i64,

    /// 성과 지표
    pub win_rate_pct: String,
    pub profit_factor: Option<String>,
    pub avg_win: String,
    pub avg_loss: String,

    /// 최대 손익
    pub largest_win: String,
    pub largest_loss: String,

    /// 거래 활동
    pub trading_period_days: i32,
    pub active_trading_days: i64,
    pub first_trade_at: Option<String>,
    pub last_trade_at: Option<String>,

    /// 고급 통계 (연속 승/패, Max Drawdown)
    pub max_consecutive_wins: Option<i64>,
    pub max_consecutive_losses: Option<i64>,
    pub max_drawdown: Option<String>,
    pub max_drawdown_pct: Option<String>,
}

impl From<TradingInsights> for TradingInsightsResponse {
    fn from(t: TradingInsights) -> Self {
        Self {
            total_trades: t.total_trades,
            buy_trades: t.buy_trades.unwrap_or(0),
            sell_trades: t.sell_trades.unwrap_or(0),
            unique_symbols: t.unique_symbols.unwrap_or(0),
            total_realized_pnl: t.total_realized_pnl.unwrap_or(Decimal::ZERO).to_string(),
            total_fees: t.total_fees.unwrap_or(Decimal::ZERO).to_string(),
            winning_trades: t.winning_trades.unwrap_or(0),
            losing_trades: t.losing_trades.unwrap_or(0),
            win_rate_pct: format!("{:.2}", t.win_rate_pct.unwrap_or(Decimal::ZERO)),
            profit_factor: t.profit_factor.map(|v| format!("{:.2}", v)),
            avg_win: t.avg_win.unwrap_or(Decimal::ZERO).to_string(),
            avg_loss: t.avg_loss.unwrap_or(Decimal::ZERO).to_string(),
            largest_win: t.largest_win.unwrap_or(Decimal::ZERO).to_string(),
            largest_loss: t.largest_loss.unwrap_or(Decimal::ZERO).to_string(),
            trading_period_days: t.trading_period_days.unwrap_or(0),
            active_trading_days: t.active_trading_days.unwrap_or(0),
            first_trade_at: t.first_trade_at.map(|dt| dt.to_rfc3339()),
            last_trade_at: t.last_trade_at.map(|dt| dt.to_rfc3339()),
            // 고급 통계는 별도 조회 필요 (From에서는 None)
            max_consecutive_wins: None,
            max_consecutive_losses: None,
            max_drawdown: None,
            max_drawdown_pct: None,
        }
    }
}

impl TradingInsightsResponse {
    /// 고급 통계를 추가한 새 인스턴스 반환.
    pub fn with_advanced_stats(mut self, stats: &crate::repository::AdvancedTradingStats) -> Self {
        self.max_consecutive_wins = Some(stats.max_consecutive_wins);
        self.max_consecutive_losses = Some(stats.max_consecutive_losses);
        self.max_drawdown = Some(stats.max_drawdown.to_string());
        self.max_drawdown_pct = Some(format!("{:.2}", stats.max_drawdown_pct));
        self
    }
}

/// 전략별 성과 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct StrategyPerformanceResponse {
    pub strategies: Vec<StrategyPerformanceItem>,
    pub total: usize,
}

/// 전략별 성과 항목.
#[derive(Debug, Serialize, ToSchema)]
pub struct StrategyPerformanceItem {
    pub strategy_id: String,
    pub strategy_name: String,
    pub total_trades: i64,
    pub buy_trades: i64,
    pub sell_trades: i64,
    pub unique_symbols: i64,
    pub total_volume: String,
    pub total_fees: String,
    pub realized_pnl: String,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub win_rate_pct: String,
    pub profit_factor: Option<String>,
    pub avg_win: String,
    pub avg_loss: String,
    pub largest_win: String,
    pub largest_loss: String,
    pub active_trading_days: i64,
    pub first_trade_at: Option<String>,
    pub last_trade_at: Option<String>,
}

impl From<StrategyPerformance> for StrategyPerformanceItem {
    fn from(s: StrategyPerformance) -> Self {
        Self {
            strategy_id: s.strategy_id,
            strategy_name: s.strategy_name,
            total_trades: s.total_trades,
            buy_trades: s.buy_trades.unwrap_or(0),
            sell_trades: s.sell_trades.unwrap_or(0),
            unique_symbols: s.unique_symbols.unwrap_or(0),
            total_volume: s.total_volume.unwrap_or(Decimal::ZERO).to_string(),
            total_fees: s.total_fees.unwrap_or(Decimal::ZERO).to_string(),
            realized_pnl: s.realized_pnl.unwrap_or(Decimal::ZERO).to_string(),
            winning_trades: s.winning_trades.unwrap_or(0),
            losing_trades: s.losing_trades.unwrap_or(0),
            win_rate_pct: format!("{:.2}", s.win_rate_pct.unwrap_or(Decimal::ZERO)),
            profit_factor: s.profit_factor.map(|v| format!("{:.2}", v)),
            avg_win: s.avg_win.unwrap_or(Decimal::ZERO).to_string(),
            avg_loss: s.avg_loss.unwrap_or(Decimal::ZERO).to_string(),
            largest_win: s.largest_win.unwrap_or(Decimal::ZERO).to_string(),
            largest_loss: s.largest_loss.unwrap_or(Decimal::ZERO).to_string(),
            active_trading_days: s.active_trading_days.unwrap_or(0),
            first_trade_at: s.first_trade_at.map(|dt| dt.to_rfc3339()),
            last_trade_at: s.last_trade_at.map(|dt| dt.to_rfc3339()),
        }
    }
}

/// 주별 손익 조회.
#[utoipa::path(
    get,
    path = "/api/v1/journal/pnl/weekly",
    tag = "journal",
    responses(
        (status = 200, description = "주별 손익 조회 성공", body = WeeklyPnLResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn get_weekly_pnl(
    State(state): State<Arc<AppState>>,
) -> Result<Json<WeeklyPnLResponse>, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    let weekly = JournalRepository::get_weekly_pnl(pool, credential_id, Some(52))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to get weekly PnL: {}", e),
                )),
            )
        })?;

    let total_weeks = weekly.len();
    let items: Vec<WeeklyPnLItem> = weekly.into_iter().map(Into::into).collect();

    Ok(Json(WeeklyPnLResponse {
        weekly: items,
        total_weeks,
    }))
}

/// 월별 손익 조회.
#[utoipa::path(
    get,
    path = "/api/v1/journal/pnl/monthly",
    tag = "journal",
    responses(
        (status = 200, description = "월별 손익 조회 성공", body = MonthlyPnLResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn get_monthly_pnl(
    State(state): State<Arc<AppState>>,
) -> Result<Json<MonthlyPnLResponse>, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    let monthly = JournalRepository::get_monthly_pnl(pool, credential_id, Some(24))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to get monthly PnL: {}", e),
                )),
            )
        })?;

    let total_months = monthly.len();
    let items: Vec<MonthlyPnLItem> = monthly.into_iter().map(Into::into).collect();

    Ok(Json(MonthlyPnLResponse {
        monthly: items,
        total_months,
    }))
}

/// 연도별 손익 조회.
#[utoipa::path(
    get,
    path = "/api/v1/journal/pnl/yearly",
    tag = "journal",
    responses(
        (status = 200, description = "연도별 손익 조회 성공", body = YearlyPnLResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn get_yearly_pnl(
    State(state): State<Arc<AppState>>,
) -> Result<Json<YearlyPnLResponse>, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    let yearly = JournalRepository::get_yearly_pnl(pool, credential_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to get yearly PnL: {}", e),
                )),
            )
        })?;

    let total_years = yearly.len();
    let items: Vec<YearlyPnLItem> = yearly.into_iter().map(Into::into).collect();

    Ok(Json(YearlyPnLResponse {
        yearly: items,
        total_years,
    }))
}

/// 누적 손익 곡선 조회.
#[utoipa::path(
    get,
    path = "/api/v1/journal/pnl/cumulative",
    tag = "journal",
    responses(
        (status = 200, description = "누적 손익 조회 성공", body = CumulativePnLResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn get_cumulative_pnl(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CumulativePnLResponse>, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    let cumulative = JournalRepository::get_cumulative_pnl(pool, credential_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to get cumulative PnL: {}", e),
                )),
            )
        })?;

    let total_points = cumulative.len();
    let curve: Vec<CumulativePnLPoint> = cumulative.into_iter().map(Into::into).collect();

    Ok(Json(CumulativePnLResponse {
        curve,
        total_points,
    }))
}

/// 투자 인사이트 조회.
///
/// 기본 인사이트와 함께 고급 통계(연속 승/패, Max Drawdown)를 반환합니다.
#[utoipa::path(
    get,
    path = "/api/v1/journal/insights",
    tag = "journal",
    responses(
        (status = 200, description = "투자 인사이트 조회 성공", body = TradingInsightsResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn get_trading_insights(
    State(state): State<Arc<AppState>>,
) -> Result<Json<TradingInsightsResponse>, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    let insights = JournalRepository::get_trading_insights(pool, credential_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to get trading insights: {}", e),
                )),
            )
        })?;

    match insights {
        Some(i) => {
            // 기본 인사이트를 응답으로 변환
            let mut response: TradingInsightsResponse = i.into();

            // 고급 통계 조회 (별도 계산)
            match JournalRepository::get_advanced_stats(pool, credential_id).await {
                Ok(stats) => {
                    response = response.with_advanced_stats(&stats);
                }
                Err(e) => {
                    tracing::warn!("Failed to get advanced stats: {}", e);
                    // 고급 통계 실패해도 기본 응답은 반환
                }
            }

            Ok(Json(response))
        }
        None => Ok(Json(TradingInsightsResponse {
            total_trades: 0,
            buy_trades: 0,
            sell_trades: 0,
            unique_symbols: 0,
            total_realized_pnl: "0".to_string(),
            total_fees: "0".to_string(),
            winning_trades: 0,
            losing_trades: 0,
            win_rate_pct: "0.00".to_string(),
            profit_factor: None,
            avg_win: "0".to_string(),
            avg_loss: "0".to_string(),
            largest_win: "0".to_string(),
            largest_loss: "0".to_string(),
            trading_period_days: 0,
            active_trading_days: 0,
            first_trade_at: None,
            last_trade_at: None,
            max_consecutive_wins: None,
            max_consecutive_losses: None,
            max_drawdown: None,
            max_drawdown_pct: None,
        })),
    }
}

/// 전략별 성과 조회.
#[utoipa::path(
    get,
    path = "/api/v1/journal/strategies",
    tag = "journal",
    responses(
        (status = 200, description = "전략별 성과 조회 성공", body = StrategyPerformanceResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn get_strategy_performance(
    State(state): State<Arc<AppState>>,
) -> Result<Json<StrategyPerformanceResponse>, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    let strategies = JournalRepository::get_strategy_performance(pool, credential_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to get strategy performance: {}", e),
                )),
            )
        })?;

    let total = strategies.len();
    let items: Vec<StrategyPerformanceItem> = strategies.into_iter().map(Into::into).collect();

    Ok(Json(StrategyPerformanceResponse {
        strategies: items,
        total,
    }))
}

/// 체결 내역 메모/태그 수정.
#[utoipa::path(
    patch,
    path = "/api/v1/journal/executions/{id}",
    tag = "journal",
    params(
        ("id" = Uuid, Path, description = "체결 ID")
    ),
    request_body = UpdateExecutionRequest,
    responses(
        (status = 200, description = "체결 내역 수정 성공", body = ExecutionResponse),
        (status = 403, description = "접근 금지", body = ApiError),
        (status = 404, description = "체결 내역 없음", body = ApiError),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn update_execution(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateExecutionRequest>,
) -> Result<Json<ExecutionResponse>, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    let existing = JournalRepository::get_execution(pool, id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("Failed to get execution: {}", e),
                )),
            )
        })?;

    match existing {
        Some(exec) if exec.credential_id == credential_id => {
            let updated = JournalRepository::update_execution_memo(pool, id, req.memo, req.tags)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiError::new(
                            "DB_ERROR",
                            format!("Failed to update execution: {}", e),
                        )),
                    )
                })?;

            Ok(Json(updated.into()))
        }
        Some(_) => Err((
            StatusCode::FORBIDDEN,
            Json(ApiError::new(
                "FORBIDDEN",
                "Execution belongs to another account",
            )),
        )),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new(
                "NOT_FOUND",
                format!("Execution not found: {}", id),
            )),
        )),
    }
}

/// 거래소 체결 내역 동기화.
///
/// KIS API에서 체결 내역을 가져와 execution_cache 테이블에 저장합니다.
/// trade_executions는 메모/태그 등 추가 정보만 저장합니다.
#[utoipa::path(
    post,
    path = "/api/v1/journal/sync",
    tag = "journal",
    request_body = SyncRequest,
    responses(
        (status = 200, description = "동기화 성공", body = SyncResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn sync_executions(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SyncRequest>,
) -> Result<Json<SyncResponse>, (StatusCode, Json<ApiError>)> {
    let _pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    debug!(
        "체결 내역 동기화 시작: credential_id={}, force_full_sync={}",
        credential_id, req.force_full_sync
    );

    // DB 연결 확인
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", "DB pool이 없습니다")),
        )
    })?;

    // exchange_id 조회
    let exchange_id: String = sqlx::query_scalar(
        "SELECT exchange_id FROM exchange_credentials WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("exchange_id 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("exchange_id 조회 실패: {}", e))),
        )
    })?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("NOT_FOUND", "해당 credential을 찾을 수 없습니다.")),
        )
    })?;

    // 거래소별 Provider 생성
    let provider: std::sync::Arc<dyn trader_core::ExchangeProvider> = if exchange_id == "mock" {
        // Mock 거래소는 encryptor 불필요
        create_provider_for_mock_credential(pool, credential_id)
            .await
            .map_err(|e| {
                error!("Mock Provider 생성 실패: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new("CLIENT_ERROR", &e)),
                )
            })?
    } else {
        // 실제 거래소는 encryptor 필요
        let encryptor = state.encryptor.as_ref().ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("ENCRYPTOR_ERROR", "Encryptor가 없습니다")),
            )
        })?;
        create_exchange_providers_from_credential(pool, encryptor, credential_id, None)
            .await
            .map_err(|e| {
                error!("거래소 Provider 생성 실패: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new("CLIENT_ERROR", &e)),
                )
            })?
    };

    // 강제 전체 동기화 시 캐시 초기화
    if req.force_full_sync {
        debug!("강제 전체 동기화: 캐시 초기화 수행");
        match ExecutionCacheRepository::clear_cache(pool, credential_id, &exchange_id).await {
            Ok(deleted) => {
                debug!("캐시 초기화 완료: {} 건 삭제", deleted);
            }
            Err(e) => {
                warn!("캐시 초기화 실패 (계속 진행): {}", e);
            }
        }
    }

    // 날짜 설정 (기본: 30일 전 ~ 오늘)
    let today = chrono::Utc::now() + chrono::Duration::hours(9); // KST
    let default_start = (today - chrono::Duration::days(30))
        .format("%Y%m%d")
        .to_string();
    let default_end = today.format("%Y%m%d").to_string();

    // 사용자가 입력한 날짜 파싱 (다양한 형식 지원) -> YYYYMMDD로 변환
    let start_date = match req.start_date {
        Some(ref s) => {
            let parsed = parse_date_flexible(s, "start_date")
                .map_err(|e| (StatusCode::BAD_REQUEST, Json(e.to_api_error())))?;
            parsed.format("%Y%m%d").to_string()
        }
        None => default_start,
    };
    let end_date = default_end;

    // 체결 내역 조회 (ExchangeProvider 사용)
    let request = trader_core::ExecutionHistoryRequest::new(&start_date, &end_date).with_side("00");

    let history_response = provider
        .fetch_execution_history(&request)
        .await
        .map_err(|e| {
            error!("체결 내역 조회 실패: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "HISTORY_FETCH_ERROR",
                    format!("체결 내역 조회 실패: {:?}", e),
                )),
            )
        })?;

    // Trade를 NewExecution으로 변환 (execution_cache용)
    let executions: Vec<NewExecution> = history_response
        .trades
        .iter()
        .filter(|t| t.quantity > Decimal::ZERO) // 체결된 것만
        .map(|t| {
            let side_str = format!("{:?}", t.side).to_lowercase();
            let side = Side::from_str_flexible(&side_str).unwrap_or(Side::Buy);
            let amount = t.quantity * t.price;
            let trade_id = format!("{}_{}", t.exchange_trade_id, t.executed_at.timestamp());
            let asset_name = t
                .metadata
                .get("stock_name")
                .and_then(|v| v.as_str())
                .unwrap_or(&t.ticker)
                .to_string();

            NewExecution {
                credential_id,
                exchange: exchange_id.clone(),
                executed_at: t.executed_at,
                symbol: t.ticker.clone(),
                normalized_symbol: Some(asset_name),
                side,
                quantity: t.quantity,
                price: t.price,
                amount,
                fee: Some(t.fee),
                fee_currency: Some(t.fee_currency.clone()),
                order_id: t.exchange_trade_id.clone(),
                trade_id: Some(trade_id),
                order_type: Some("LIMIT".to_string()), // KIS API는 주문 유형을 제공하지 않음
                raw_data: None,
            }
        })
        .collect();

    let count = executions.len();

    // execution_cache에 저장 (upsert)
    let inserted = ExecutionCacheRepository::upsert_executions(pool, &executions)
        .await
        .map_err(|e| {
            error!("체결 내역 저장 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "DB_ERROR",
                    format!("체결 내역 저장 실패: {}", e),
                )),
            )
        })?;

    let skipped = count - inserted;

    // 캐시 메타데이터 업데이트
    if !executions.is_empty() {
        let earliest = executions.iter().map(|e| e.executed_at.date_naive()).min();
        let latest = executions.iter().map(|e| e.executed_at.date_naive()).max();

        let _ = ExecutionCacheRepository::update_cache_meta(
            pool,
            credential_id,
            &exchange_id,
            earliest,
            latest,
            "success",
            Some(&format!("동기화 완료: {} 건", inserted)),
        )
        .await;
    }

    // 자산 곡선 동기화 (체결 내역 기반)
    // ISA 계좌 등 잔고 조회 API가 제한된 경우 체결 내역과 종가로 자산 곡선 복원
    let equity_sync_count = match EquityHistoryRepository::sync_with_market_prices(
        pool,
        credential_id,
        Decimal::ZERO, // 현금 잔고는 종가 기반 계산에서 제외
        "KRW",
        "KR",
        None, // account_type은 credential에서 자동 감지
    )
    .await
    {
        Ok(count) => {
            debug!("자산 곡선 동기화 완료: {} 포인트", count);
            count
        }
        Err(e) => {
            warn!("자산 곡선 동기화 실패 (계속 진행): {}", e);
            0
        }
    };

    debug!(
        "체결 내역 동기화 완료: {} 건 조회, {} 건 저장, {} 건 스킵, 자산 곡선 {} 포인트",
        count, inserted, skipped, equity_sync_count
    );

    Ok(Json(SyncResponse {
        success: true,
        inserted: inserted as i32,
        skipped: skipped as i32,
        message: format!(
            "동기화 완료: {} 건 저장, {} 건 스킵, 자산 곡선 {} 포인트 (기간: {} ~ {})",
            inserted, skipped, equity_sync_count, start_date, end_date
        ),
    }))
}

// ==================== 헬퍼 함수 ====================

/// DB 연결 풀 조회.
fn get_db_pool(state: &Arc<AppState>) -> Result<&sqlx::PgPool, (StatusCode, Json<ApiError>)> {
    state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "DB_NOT_CONNECTED",
                "Database connection is not available",
            )),
        )
    })
}

/// 활성 계정의 credential_id 조회.
async fn get_active_credential_id(
    state: &Arc<AppState>,
) -> Result<Uuid, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(state)?;

    // app_settings 테이블에서 active_credential_id 조회
    let result: Option<(String,)> = sqlx::query_as(
        "SELECT setting_value FROM app_settings WHERE setting_key = 'active_credential_id' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "DB_ERROR",
                format!("Failed to get active account: {}", e),
            )),
        )
    })?;

    match result {
        Some((credential_id_str,)) if !credential_id_str.is_empty() => {
            Uuid::parse_str(&credential_id_str).map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new(
                        "INVALID_UUID",
                        "Invalid credential ID format",
                    )),
                )
            })
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "NO_ACTIVE_ACCOUNT",
                "No active account selected. Please select an account in Settings.",
            )),
        )),
    }
}

// ==================== FIFO 원가 계산 ====================

/// FIFO 원가 계산 쿼리.
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct CostBasisQuery {
    /// 시장 (기본: KR)
    #[serde(default = "default_market_kr")]
    pub market: String,
    /// 현재가 (미실현 손익 계산용, 선택)
    #[serde(default)]
    pub current_price: Option<Decimal>,
}

fn default_market_kr() -> String {
    "KR".to_string()
}

/// FIFO 원가 계산 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct CostBasisResponse {
    /// 심볼
    pub symbol: String,
    /// 총 보유 수량
    pub total_quantity: Decimal,
    /// 가중평균 매입가 (수수료 포함)
    pub average_cost: Decimal,
    /// 가중평균 매입가 (수수료 제외)
    pub average_price: Decimal,
    /// 총 비용 기준
    pub total_cost_basis: Decimal,
    /// 시장 가치 (현재가 기준, 선택)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_value: Option<Decimal>,
    /// 미실현 손익
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unrealized_pnl: Option<Decimal>,
    /// 미실현 손익률 (%)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unrealized_pnl_pct: Option<Decimal>,
    /// 누적 실현 손익
    pub total_realized_pnl: Decimal,
    /// 총 매도 건수
    pub total_sales: u32,
    /// 총 수수료
    pub total_fees: Decimal,
    /// 로트 개수
    pub lot_count: usize,
}

impl From<CostBasisSummary> for CostBasisResponse {
    fn from(s: CostBasisSummary) -> Self {
        Self {
            symbol: s.symbol,
            total_quantity: s.total_quantity,
            average_cost: s.average_cost,
            average_price: s.average_price,
            total_cost_basis: s.total_cost_basis,
            market_value: s.market_value,
            unrealized_pnl: s.unrealized_pnl,
            unrealized_pnl_pct: s.unrealized_pnl_pct,
            total_realized_pnl: s.total_realized_pnl,
            total_sales: s.total_sales,
            total_fees: s.total_fees,
            lot_count: s.lot_count,
        }
    }
}

/// FIFO 원가 계산.
///
/// 특정 종목의 체결 내역을 기반으로 FIFO 원가와 실현/미실현 손익을 계산합니다.
#[utoipa::path(
    get,
    path = "/api/v1/journal/cost-basis/{symbol}",
    tag = "journal",
    params(
        ("symbol" = String, Path, description = "종목 심볼"),
        CostBasisQuery
    ),
    responses(
        (status = 200, description = "원가 계산 성공", body = CostBasisResponse),
        (status = 500, description = "서버 오류")
    )
)]
async fn get_cost_basis(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
    Query(query): Query<CostBasisQuery>,
) -> Result<Json<CostBasisResponse>, (StatusCode, String)> {
    tracing::debug!("FIFO 원가 계산 요청: {} ({})", symbol, query.market);

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    // 활성 credential 조회
    let credential_id = crate::repository::get_active_credential_id(pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("credential 조회 실패: {}", e),
            )
        })?;

    // 종목 체결 내역 조회 (시간순 정렬은 build_tracker_from_executions에서 수행)
    let filter = ExecutionFilter {
        symbol: Some(symbol.clone()),
        strategy_id: None,
        start_date: None,
        end_date: None,
        side: None,
        limit: Some(10000), // 충분한 개수
        offset: None,
    };

    let executions = JournalRepository::list_executions(pool, credential_id, filter)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("체결 내역 조회 실패: {}", e),
            )
        })?;

    if executions.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("종목 '{}' 체결 내역이 없습니다", symbol),
        ));
    }

    // TradeExecutionRecord → TradeExecution 변환
    let trade_executions: Vec<TradeExecution> = executions
        .into_iter()
        .map(|e| TradeExecution {
            id: e.id,
            symbol: e.symbol,
            side: e.side,
            quantity: e.quantity,
            price: e.price,
            fee: e.fee.unwrap_or(Decimal::ZERO),
            executed_at: e.executed_at,
        })
        .collect();

    // FIFO 추적기 빌드
    let tracker = build_tracker_from_executions(&symbol, trade_executions);

    // 요약 생성 (현재가가 있으면 미실현 손익도 계산)
    let summary = tracker.summary(query.current_price);

    Ok(Json(summary.into()))
}

// ==================== 손익 재계산 ====================

/// 손익 재계산 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct RecalculateResponse {
    pub symbols_processed: i32,
    pub executions_updated: i32,
    pub total_realized_pnl: Decimal,
    pub errors: Vec<String>,
}

/// 모든 체결 내역의 손익 재계산.
///
/// CostBasisTracker를 사용하여 FIFO 기반으로 각 체결의
/// realized_pnl과 position_effect를 자동으로 계산합니다.
///
/// # 계산 항목
///
/// - **realized_pnl**: 매도 시 FIFO 기반 실현 손익
/// - **position_effect**: open(신규), add(추가매수), reduce(부분매도), close(전량매도)
#[utoipa::path(
    post,
    path = "/api/v1/journal/recalculate",
    responses(
        (status = 200, description = "재계산 완료", body = RecalculateResponse),
        (status = 500, description = "서버 오류")
    ),
    tag = "journal"
)]
async fn recalculate_pnl(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RecalculateResponse>, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    debug!(credential_id = %credential_id, "손익 재계산 시작");

    let result = JournalRepository::recalculate_all_pnl(pool, credential_id)
        .await
        .map_err(|e| {
            error!(error = %e, "손익 재계산 실패");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "RECALCULATE_ERROR",
                    format!("재계산 실패: {}", e),
                )),
            )
        })?;

    debug!(
        symbols = result.symbols_processed,
        executions = result.executions_updated,
        total_pnl = %result.total_realized_pnl,
        errors = result.errors.len(),
        "손익 재계산 완료"
    );

    Ok(Json(RecalculateResponse {
        symbols_processed: result.symbols_processed,
        executions_updated: result.executions_updated,
        total_realized_pnl: result.total_realized_pnl,
        errors: result.errors,
    }))
}

// ==================== 캐시 관리 ====================

/// 캐시 삭제 응답.
#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export, export_to = "journal/")]
pub struct ClearCacheResponse {
    pub success: bool,
    pub deleted_count: u64,
    pub message: String,
}

/// 체결 내역 캐시 삭제.
///
/// ISA 계좌 등 전체 내역 재동기화가 필요할 때 사용합니다.
#[utoipa::path(
    delete,
    path = "/api/v1/journal/cache",
    tag = "journal",
    responses(
        (status = 200, description = "캐시 삭제 성공", body = ClearCacheResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn clear_execution_cache(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ClearCacheResponse>, (StatusCode, Json<ApiError>)> {
    let pool = get_db_pool(&state)?;
    let credential_id = get_active_credential_id(&state).await?;

    debug!("캐시 삭제 요청: credential_id={}", credential_id);

    // credential에서 exchange_id 동적 조회
    let exchange_id: String = sqlx::query_scalar(
        "SELECT exchange_id FROM exchange_credentials WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("exchange_id 조회 실패: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("DB_ERROR", format!("exchange_id 조회 실패: {}", e))),
        )
    })?
    .unwrap_or_else(|| "unknown".to_string());

    let deleted = ExecutionCacheRepository::clear_cache(pool, credential_id, &exchange_id)
        .await
        .map_err(|e| {
            error!("캐시 삭제 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("DB_ERROR", format!("캐시 삭제 실패: {}", e))),
            )
        })?;

    debug!("캐시 삭제 완료: {} 건", deleted);

    Ok(Json(ClearCacheResponse {
        success: true,
        deleted_count: deleted,
        message: format!("캐시 {} 건 삭제 완료", deleted),
    }))
}

// ==================== 라우터 ====================

/// 매매일지 라우터 생성.
pub fn journal_router() -> Router<Arc<AppState>> {
    Router::new()
        // 기본 API
        .route("/positions", get(get_journal_positions))
        .route("/executions", get(list_executions))
        .route("/executions/{id}", patch(update_execution))
        .route("/sync", post(sync_executions))
        .route("/cache", delete(clear_execution_cache))
        .route("/recalculate", post(recalculate_pnl))
        // 손익 API
        .route("/pnl", get(get_pnl_summary))
        .route("/pnl/daily", get(get_daily_pnl))
        .route("/pnl/weekly", get(get_weekly_pnl))
        .route("/pnl/monthly", get(get_monthly_pnl))
        .route("/pnl/yearly", get(get_yearly_pnl))
        .route("/pnl/symbol", get(get_symbol_pnl))
        .route("/pnl/cumulative", get(get_cumulative_pnl))
        // 인사이트 API
        .route("/insights", get(get_trading_insights))
        .route("/strategies", get(get_strategy_performance))
        // 원가 계산 API
        .route("/cost-basis/{symbol}", get(get_cost_basis))
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pnl_summary_response_conversion() {
        let summary = PnLSummary {
            total_realized_pnl: Decimal::from(10000),
            total_fees: Decimal::from(100),
            total_trades: 50,
            buy_trades: Some(25),
            sell_trades: Some(25),
            winning_trades: Some(30),
            losing_trades: Some(15),
            total_volume: Decimal::from(1000000),
            first_trade_at: None,
            last_trade_at: None,
        };

        let response: PnLSummaryResponse = summary.into();

        assert_eq!(response.total_realized_pnl, "10000");
        assert_eq!(response.total_fees, "100");
        assert_eq!(response.net_pnl, "9900");
        assert_eq!(response.winning_trades, 30);
        assert_eq!(response.losing_trades, 15);
        // 30 / 45 = 66.67%
        assert!(response.win_rate.starts_with("66.6"));
    }

    #[test]
    fn test_daily_pnl_item_conversion() {
        let summary = DailySummary {
            credential_id: Uuid::new_v4(),
            trade_date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            total_trades: 10,
            buy_count: Some(6),
            sell_count: Some(4),
            total_volume: Some(Decimal::from(500000)),
            total_fees: Some(Decimal::from(50)),
            realized_pnl: Some(Decimal::from(1000)),
            symbol_count: Some(3),
        };

        let item: DailyPnLItem = summary.into();

        assert_eq!(item.date, "2024-01-15");
        assert_eq!(item.total_trades, 10);
        assert_eq!(item.buy_count, 6);
        assert_eq!(item.sell_count, 4);
    }

    // ==================== 날짜 파싱 테스트 ====================

    #[test]
    fn test_parse_datetime_flexible_rfc3339() {
        // RFC3339 형식 (Z)
        let result = parse_datetime_flexible("2024-01-15T09:30:00Z", "test");
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-01-15");

        // RFC3339 형식 (타임존 오프셋)
        let result = parse_datetime_flexible("2024-01-15T18:30:00+09:00", "test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_datetime_flexible_date_only() {
        // YYYY-MM-DD 형식
        let result = parse_datetime_flexible("2024-01-15", "test");
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-01-15");

        // YYYY/MM/DD 형식
        let result = parse_datetime_flexible("2024/01/15", "test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_datetime_flexible_invalid() {
        // 잘못된 형식
        let result = parse_datetime_flexible("invalid-date", "start_date");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.field, "start_date");
        assert_eq!(err.value, "invalid-date");
        assert!(!err.expected_formats.is_empty());
    }

    #[test]
    fn test_parse_date_flexible_iso() {
        // YYYY-MM-DD 형식
        let result = parse_date_flexible("2024-01-15", "test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_string(), "2024-01-15");

        // YYYY/MM/DD 형식
        let result = parse_date_flexible("2024/01/15", "test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_date_flexible_yyyymmdd() {
        // YYYYMMDD 형식 (KIS API용)
        let result = parse_date_flexible("20240115", "test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_string(), "2024-01-15");
    }

    #[test]
    fn test_parse_date_flexible_invalid() {
        // 잘못된 형식
        let result = parse_date_flexible("15-01-2024", "end_date");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.field, "end_date");
        assert!(err.to_api_error().message.contains("허용 형식"));
    }

    #[test]
    fn test_date_parse_error_message() {
        let err = DateParseError::new("start_date", "bad-date", vec!["YYYY-MM-DD", "YYYYMMDD"]);
        let api_err = err.to_api_error();

        assert_eq!(api_err.code, "INVALID_DATE_FORMAT");
        assert!(api_err.message.contains("start_date"));
        assert!(api_err.message.contains("bad-date"));
        assert!(api_err.message.contains("YYYY-MM-DD"));
    }
}
