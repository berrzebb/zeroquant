//! Paper Trading API 엔드포인트.
//!
//! Mock 거래소를 사용한 실시간 Paper Trading 기능을 제공합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/paper-trading/accounts` - Mock 계정 목록 조회
//! - `GET /api/v1/paper-trading/accounts/:id` - Mock 계정 상세 조회 (실시간 P&L)
//! - `GET /api/v1/paper-trading/accounts/:id/positions` - 포지션 목록 조회
//! - `GET /api/v1/paper-trading/accounts/:id/executions` - 체결 내역 조회
//! - `POST /api/v1/paper-trading/accounts/:id/reset` - 계정 초기화

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::Serialize;
use std::sync::Arc;
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::state::AppState;
use trader_exchange::provider::{MockConfig, MockExchangeProvider};

// ==================== 응답 타입 ====================

/// Paper Trading 계정 정보.
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "paper_trading/")]
pub struct PaperTradingAccount {
    /// 계정 ID (credential_id)
    pub id: String,
    /// 계정 이름
    pub name: String,
    /// 거래소 ID (mock)
    #[serde(rename = "exchangeId")]
    pub exchange_id: String,
    /// 시장 유형 (stock_kr, stock_us, crypto)
    #[serde(rename = "marketType")]
    pub market_type: String,
    /// 통화
    pub currency: String,
    /// 초기 자금
    #[serde(rename = "initialBalance")]
    pub initial_balance: String,
    /// 현재 잔고
    #[serde(rename = "currentBalance")]
    pub current_balance: String,
    /// 포지션 평가액
    #[serde(rename = "positionValue")]
    pub position_value: String,
    /// 총 자산 (잔고 + 포지션)
    #[serde(rename = "totalEquity")]
    pub total_equity: String,
    /// 미실현 손익
    #[serde(rename = "unrealizedPnl")]
    pub unrealized_pnl: String,
    /// 실현 손익
    #[serde(rename = "realizedPnl")]
    pub realized_pnl: String,
    /// 수익률 (%)
    #[serde(rename = "returnPct")]
    pub return_pct: String,
    /// 연결된 전략 수
    #[serde(rename = "strategyCount")]
    pub strategy_count: i32,
    /// 활성 상태
    #[serde(rename = "isActive")]
    pub is_active: bool,
}

/// Paper Trading 계정 목록 응답.
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "paper_trading/")]
pub struct PaperTradingAccountsResponse {
    /// 계정 목록
    pub accounts: Vec<PaperTradingAccount>,
    /// 총 계정 수
    pub total: usize,
}

/// Paper Trading 포지션 정보.
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "paper_trading/")]
pub struct PaperTradingPosition {
    /// 심볼
    pub symbol: String,
    /// 포지션 방향 (Long/Short)
    pub side: String,
    /// 수량
    pub quantity: String,
    /// 진입가
    #[serde(rename = "entryPrice")]
    pub entry_price: String,
    /// 현재가
    #[serde(rename = "currentPrice")]
    pub current_price: String,
    /// 평가금액
    #[serde(rename = "marketValue")]
    pub market_value: String,
    /// 미실현 손익
    #[serde(rename = "unrealizedPnl")]
    pub unrealized_pnl: String,
    /// 수익률 (%)
    #[serde(rename = "returnPct")]
    pub return_pct: String,
    /// 진입 시간
    #[serde(rename = "entryTime")]
    pub entry_time: String,
}

/// Paper Trading 포지션 목록 응답.
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "paper_trading/")]
pub struct PaperTradingPositionsResponse {
    /// 포지션 목록
    pub positions: Vec<PaperTradingPosition>,
    /// 총 포지션 수
    pub total: usize,
    /// 총 평가액
    #[serde(rename = "totalValue")]
    pub total_value: String,
    /// 총 미실현 손익
    #[serde(rename = "totalUnrealizedPnl")]
    pub total_unrealized_pnl: String,
}

/// Paper Trading 체결 내역.
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "paper_trading/")]
pub struct PaperTradingExecution {
    /// 체결 ID
    pub id: String,
    /// 심볼
    pub symbol: String,
    /// 방향 (Buy/Sell)
    pub side: String,
    /// 수량
    pub quantity: String,
    /// 체결가
    pub price: String,
    /// 수수료
    pub commission: String,
    /// 실현 손익 (청산 시)
    #[serde(rename = "realizedPnl")]
    pub realized_pnl: Option<String>,
    /// 체결 시간
    #[serde(rename = "executedAt")]
    pub executed_at: String,
}

/// Paper Trading 체결 내역 응답.
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "paper_trading/")]
pub struct PaperTradingExecutionsResponse {
    /// 체결 내역
    pub executions: Vec<PaperTradingExecution>,
    /// 총 체결 수
    pub total: usize,
}

/// 계정 초기화 응답.
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "paper_trading/")]
pub struct ResetAccountResponse {
    pub success: bool,
    pub message: String,
}

// ==================== Mock Provider 헬퍼 ====================

/// Mock Provider를 가져오거나 생성합니다.
///
/// credential_id별로 캐시하여 동일 계정을 사용하는 여러 전략이
/// 같은 스트림 원천을 공유합니다.
async fn get_or_create_mock_provider(
    state: &AppState,
    credential_id: Uuid,
    initial_balance: Decimal,
    market_type: &str,
) -> Result<Arc<MockExchangeProvider>, (StatusCode, Json<serde_json::Value>)> {
    // 1. 캐시에서 먼저 조회
    {
        let providers = state.mock_providers.read().await;
        if let Some(provider) = providers.get(&credential_id) {
            return Ok(Arc::clone(provider));
        }
    }

    // 2. 없으면 새로 생성
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "DB 연결 없음"})),
        )
    })?;

    let config = match market_type {
        "stock_us" => MockConfig::stock_us().with_balance(initial_balance),
        "crypto" => MockConfig::crypto().with_balance(initial_balance),
        _ => MockConfig::default().with_balance(initial_balance),
    };

    let provider = MockExchangeProvider::new(credential_id, config, pool.clone())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Mock Provider 생성 실패: {:?}", e)})),
            )
        })?;

    let provider = Arc::new(provider);

    // 3. 캐시에 저장
    {
        let mut providers = state.mock_providers.write().await;
        providers.insert(credential_id, Arc::clone(&provider));
    }

    tracing::info!("Mock Provider 생성 완료: {}", credential_id);
    Ok(provider)
}

/// 전략에서 사용하는 심볼 목록 조회.
#[allow(dead_code)]
async fn get_strategy_symbols(
    pool: &sqlx::PgPool,
    strategy_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    // 전략 설정에서 tickers 필드 조회
    let row = sqlx::query!(
        r#"SELECT config FROM strategies WHERE id = $1"#,
        strategy_id
    )
    .fetch_optional(pool)
    .await?;

    if let Some(row) = row {
        if let Some(config) = row.config {
            // JSON에서 tickers/ticker/symbols 필드 추출
            if let Some(tickers) = config.get("tickers").and_then(|v| v.as_array()) {
                return Ok(tickers
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect());
            }
            if let Some(ticker) = config.get("ticker").and_then(|v| v.as_str()) {
                return Ok(vec![ticker.to_string()]);
            }
            if let Some(symbols) = config.get("symbols").and_then(|v| v.as_array()) {
                return Ok(symbols
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect());
            }
        }
    }

    Ok(Vec::new())
}

/// 전략별 구독 심볼 관리.
///
/// 동일한 credential_id를 사용하는 여러 전략의 심볼을 수집하여
/// 스트리밍에 필요한 전체 심볼 목록을 반환합니다.
async fn collect_streaming_symbols(
    pool: &sqlx::PgPool,
    credential_id: Uuid,
) -> Result<Vec<String>, sqlx::Error> {
    // 해당 credential_id를 사용하는 모든 활성 세션의 전략 조회
    let rows = sqlx::query!(
        r#"
        SELECT s.config
        FROM paper_trading_sessions pts
        JOIN strategies s ON s.id = pts.strategy_id
        WHERE pts.credential_id = $1 AND pts.status = 'running'
        "#,
        credential_id
    )
    .fetch_all(pool)
    .await?;

    let mut all_symbols = std::collections::HashSet::new();
    for row in rows {
        if let Some(config) = row.config {
            if let Some(tickers) = config.get("tickers").and_then(|v| v.as_array()) {
                for ticker in tickers.iter().filter_map(|v| v.as_str()) {
                    all_symbols.insert(ticker.to_string());
                }
            }
            if let Some(ticker) = config.get("ticker").and_then(|v| v.as_str()) {
                all_symbols.insert(ticker.to_string());
            }
        }
    }

    Ok(all_symbols.into_iter().collect())
}

// ==================== 라우터 ====================

/// Paper Trading 라우터 생성.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // 계정 기반 API (기존, 계정 관리용)
        .route("/accounts", get(list_accounts))
        .route("/accounts/{:id}", get(get_account))
        .route("/accounts/{:id}/positions", get(get_positions))
        .route("/accounts/{:id}/executions", get(get_executions))
        .route("/accounts/{:id}/reset", post(reset_account))
        // 전략 기반 API (신규, Paper Trading 실행용)
        .route("/strategies", get(list_paper_trading_sessions))
        .route("/strategies/{strategy_id}/status", get(get_paper_trading_status))
        .route("/strategies/{strategy_id}/start", post(start_paper_trading))
        .route("/strategies/{strategy_id}/stop", post(stop_paper_trading))
        .route("/strategies/{strategy_id}/reset", post(reset_paper_trading))
        .route("/strategies/{strategy_id}/positions", get(get_strategy_positions))
        .route("/strategies/{strategy_id}/trades", get(get_strategy_trades))
}

// ==================== 핸들러 ====================

/// Mock 계정 목록 조회.
///
/// GET /api/v1/paper-trading/accounts
#[utoipa::path(
    get,
    path = "/api/v1/paper-trading/accounts",
    tag = "paper-trading",
    responses(
        (status = 200, description = "Mock 계정 목록", body = PaperTradingAccountsResponse)
    )
)]
pub async fn list_accounts(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "DB 연결 없음"})),
        )
    })?;

    // Mock 거래소 계정만 조회
    let rows = sqlx::query!(
        r#"
        SELECT
            ec.id,
            ec.exchange_name,
            ec.exchange_id,
            ec.settings,
            ec.is_active,
            mes.current_balance as "current_balance?: rust_decimal::Decimal",
            COALESCE(
                (SELECT COUNT(*) FROM strategies s WHERE s.credential_id = ec.id),
                0
            ) as strategy_count
        FROM exchange_credentials ec
        LEFT JOIN mock_exchange_state mes ON mes.credential_id = ec.id
        WHERE ec.exchange_id = 'mock'
        ORDER BY ec.created_at DESC
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB 조회 실패: {}", e)})),
        )
    })?;

    let mut accounts = Vec::new();

    for row in rows {
        // settings에서 설정값 추출
        let settings = row.settings.unwrap_or(serde_json::json!({}));
        let initial_balance = settings
            .get("initial_balance")
            .and_then(|v| v.as_str())
            .unwrap_or("10000000");
        let market_type = settings
            .get("market_type")
            .and_then(|v| v.as_str())
            .unwrap_or("stock_kr");
        let currency = settings
            .get("currency")
            .and_then(|v| v.as_str())
            .unwrap_or("KRW");

        // 현재 잔고 (DB에서 또는 초기값)
        let initial_bal_dec: Decimal = initial_balance.parse().unwrap_or(Decimal::ZERO);
        let current_balance = row.current_balance.unwrap_or(initial_bal_dec);

        // 포지션 평가액 조회
        let position_value: Decimal = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(SUM(quantity * entry_price), 0) as "value!"
            FROM mock_positions
            WHERE credential_id = $1
            "#,
            row.id
        )
        .fetch_one(pool)
        .await
        .unwrap_or(Decimal::ZERO);

        // 실현 손익 조회
        let realized_pnl: Decimal = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(SUM(realized_pnl), 0) as "value!"
            FROM mock_executions
            WHERE credential_id = $1 AND realized_pnl IS NOT NULL
            "#,
            row.id
        )
        .fetch_one(pool)
        .await
        .unwrap_or(Decimal::ZERO);

        let total_equity = current_balance + position_value;
        let initial_bal: Decimal = initial_balance.parse().unwrap_or(Decimal::ZERO);
        let return_pct = if initial_bal > Decimal::ZERO {
            ((total_equity - initial_bal) / initial_bal * Decimal::from(100))
                .round_dp(2)
        } else {
            Decimal::ZERO
        };

        accounts.push(PaperTradingAccount {
            id: row.id.to_string(),
            name: row.exchange_name,
            exchange_id: row.exchange_id,
            market_type: market_type.to_string(),
            currency: currency.to_string(),
            initial_balance: initial_balance.to_string(),
            current_balance: current_balance.to_string(),
            position_value: position_value.to_string(),
            total_equity: total_equity.to_string(),
            unrealized_pnl: "0".to_string(), // 실시간 계산 필요
            realized_pnl: realized_pnl.to_string(),
            return_pct: return_pct.to_string(),
            strategy_count: row.strategy_count.unwrap_or(0) as i32,
            is_active: row.is_active,
        });
    }

    Ok(Json(PaperTradingAccountsResponse {
        total: accounts.len(),
        accounts,
    }))
}

/// Mock 계정 상세 조회.
///
/// GET /api/v1/paper-trading/accounts/:id
#[utoipa::path(
    get,
    path = "/api/v1/paper-trading/accounts/{id}",
    tag = "paper-trading",
    params(
        ("id" = String, Path, description = "계정 ID")
    ),
    responses(
        (status = 200, description = "계정 상세 정보", body = PaperTradingAccount)
    )
)]
pub async fn get_account(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "DB 연결 없음"})),
        )
    })?;

    let credential_id = Uuid::parse_str(&id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "잘못된 ID 형식"})),
        )
    })?;

    // 계정 정보 조회
    let row = sqlx::query!(
        r#"
        SELECT
            ec.id,
            ec.exchange_name,
            ec.exchange_id,
            ec.settings,
            ec.is_active,
            mes.current_balance as "current_balance?: rust_decimal::Decimal",
            COALESCE(
                (SELECT COUNT(*) FROM strategies s WHERE s.credential_id = ec.id),
                0
            ) as strategy_count
        FROM exchange_credentials ec
        LEFT JOIN mock_exchange_state mes ON mes.credential_id = ec.id
        WHERE ec.id = $1 AND ec.exchange_id = 'mock'
        "#,
        credential_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB 조회 실패: {}", e)})),
        )
    })?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "계정을 찾을 수 없습니다"})),
        )
    })?;

    let settings = row.settings.unwrap_or(serde_json::json!({}));
    let initial_balance = settings
        .get("initial_balance")
        .and_then(|v| v.as_str())
        .unwrap_or("10000000");
    let market_type = settings
        .get("market_type")
        .and_then(|v| v.as_str())
        .unwrap_or("stock_kr");
    let currency = settings
        .get("currency")
        .and_then(|v| v.as_str())
        .unwrap_or("KRW");

    let initial_bal_dec: Decimal = initial_balance.parse().unwrap_or(Decimal::ZERO);
    let current_balance = row.current_balance.unwrap_or(initial_bal_dec);

    // 포지션 평가액 및 미실현 손익 계산 (실시간 시세 적용 가능)
    let position_value: Decimal = sqlx::query_scalar!(
        r#"
        SELECT COALESCE(SUM(quantity * entry_price), 0) as "value!"
        FROM mock_positions
        WHERE credential_id = $1
        "#,
        credential_id
    )
    .fetch_one(pool)
    .await
    .unwrap_or(Decimal::ZERO);

    let realized_pnl: Decimal = sqlx::query_scalar!(
        r#"
        SELECT COALESCE(SUM(realized_pnl), 0) as "value!"
        FROM mock_executions
        WHERE credential_id = $1 AND realized_pnl IS NOT NULL
        "#,
        credential_id
    )
    .fetch_one(pool)
    .await
    .unwrap_or(Decimal::ZERO);

    let total_equity = current_balance + position_value;
    let initial_bal: Decimal = initial_balance.parse().unwrap_or(Decimal::ZERO);
    let return_pct = if initial_bal > Decimal::ZERO {
        ((total_equity - initial_bal) / initial_bal * Decimal::from(100)).round_dp(2)
    } else {
        Decimal::ZERO
    };

    Ok(Json(PaperTradingAccount {
        id: row.id.to_string(),
        name: row.exchange_name,
        exchange_id: row.exchange_id,
        market_type: market_type.to_string(),
        currency: currency.to_string(),
        initial_balance: initial_balance.to_string(),
        current_balance: current_balance.to_string(),
        position_value: position_value.to_string(),
        total_equity: total_equity.to_string(),
        unrealized_pnl: "0".to_string(),
        realized_pnl: realized_pnl.to_string(),
        return_pct: return_pct.to_string(),
        strategy_count: row.strategy_count.unwrap_or(0) as i32,
        is_active: row.is_active,
    }))
}

/// 포지션 목록 조회.
///
/// GET /api/v1/paper-trading/accounts/:id/positions
#[utoipa::path(
    get,
    path = "/api/v1/paper-trading/accounts/{id}/positions",
    tag = "paper-trading",
    params(
        ("id" = String, Path, description = "계정 ID")
    ),
    responses(
        (status = 200, description = "포지션 목록", body = PaperTradingPositionsResponse)
    )
)]
pub async fn get_positions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "DB 연결 없음"})),
        )
    })?;

    let credential_id = Uuid::parse_str(&id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "잘못된 ID 형식"})),
        )
    })?;

    let rows = sqlx::query!(
        r#"
        SELECT symbol, side, quantity, entry_price, entry_time
        FROM mock_positions
        WHERE credential_id = $1
        ORDER BY entry_time DESC
        "#,
        credential_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB 조회 실패: {}", e)})),
        )
    })?;

    let mut positions = Vec::new();
    let mut total_value = Decimal::ZERO;
    let mut total_unrealized_pnl = Decimal::ZERO;

    for row in rows {
        // 현재가는 진입가로 대체 (실시간 시세 조회는 비용이 크므로)
        let current_price = row.entry_price;
        let market_value = row.quantity * current_price;
        let unrealized_pnl = Decimal::ZERO; // 실시간 계산 필요 시 Yahoo Finance 호출

        total_value += market_value;
        total_unrealized_pnl += unrealized_pnl;

        let return_pct = if row.entry_price > Decimal::ZERO {
            ((current_price - row.entry_price) / row.entry_price * Decimal::from(100))
                .round_dp(2)
        } else {
            Decimal::ZERO
        };

        positions.push(PaperTradingPosition {
            symbol: row.symbol,
            side: if row.side == "Buy" { "Long".to_string() } else { "Short".to_string() },
            quantity: row.quantity.to_string(),
            entry_price: row.entry_price.to_string(),
            current_price: current_price.to_string(),
            market_value: market_value.to_string(),
            unrealized_pnl: unrealized_pnl.to_string(),
            return_pct: return_pct.to_string(),
            entry_time: row.entry_time.to_rfc3339(),
        });
    }

    Ok(Json(PaperTradingPositionsResponse {
        total: positions.len(),
        positions,
        total_value: total_value.to_string(),
        total_unrealized_pnl: total_unrealized_pnl.to_string(),
    }))
}

/// 체결 내역 조회.
///
/// GET /api/v1/paper-trading/accounts/:id/executions
#[utoipa::path(
    get,
    path = "/api/v1/paper-trading/accounts/{id}/executions",
    tag = "paper-trading",
    params(
        ("id" = String, Path, description = "계정 ID")
    ),
    responses(
        (status = 200, description = "체결 내역", body = PaperTradingExecutionsResponse)
    )
)]
pub async fn get_executions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "DB 연결 없음"})),
        )
    })?;

    let credential_id = Uuid::parse_str(&id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "잘못된 ID 형식"})),
        )
    })?;

    let rows = sqlx::query!(
        r#"
        SELECT id, symbol, side, quantity, price, commission, realized_pnl, executed_at
        FROM mock_executions
        WHERE credential_id = $1
        ORDER BY executed_at DESC
        LIMIT 100
        "#,
        credential_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB 조회 실패: {}", e)})),
        )
    })?;

    let executions: Vec<PaperTradingExecution> = rows
        .into_iter()
        .map(|row| PaperTradingExecution {
            id: row.id.to_string(),
            symbol: row.symbol,
            side: row.side,
            quantity: row.quantity.to_string(),
            price: row.price.to_string(),
            commission: row.commission.to_string(),
            realized_pnl: row.realized_pnl.map(|v| v.to_string()),
            executed_at: row.executed_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(PaperTradingExecutionsResponse {
        total: executions.len(),
        executions,
    }))
}

/// 계정 초기화 (리셋).
///
/// POST /api/v1/paper-trading/accounts/:id/reset
#[utoipa::path(
    post,
    path = "/api/v1/paper-trading/accounts/{id}/reset",
    tag = "paper-trading",
    params(
        ("id" = String, Path, description = "계정 ID")
    ),
    responses(
        (status = 200, description = "초기화 성공", body = ResetAccountResponse)
    )
)]
pub async fn reset_account(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "DB 연결 없음"})),
        )
    })?;

    let credential_id = Uuid::parse_str(&id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "잘못된 ID 형식"})),
        )
    })?;

    // DB 직접 삭제 방식으로 계정 리셋
    // 상태 테이블 삭제
    sqlx::query!(
        r#"DELETE FROM mock_exchange_state WHERE credential_id = $1"#,
        credential_id
    )
    .execute(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB 에러: {}", e)})),
        )
    })?;

    // 포지션 삭제
    sqlx::query!(
        r#"DELETE FROM mock_positions WHERE credential_id = $1"#,
        credential_id
    )
    .execute(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB 에러: {}", e)})),
        )
    })?;

    // 체결 내역 삭제
    sqlx::query!(
        r#"DELETE FROM mock_executions WHERE credential_id = $1"#,
        credential_id
    )
    .execute(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB 에러: {}", e)})),
        )
    })?;

    Ok(Json(ResetAccountResponse {
        success: true,
        message: "계정이 초기화되었습니다.".to_string(),
    }))
}

// ==================== 전략 기반 Paper Trading API ====================

/// Paper Trading 시작 요청.
#[derive(Debug, serde::Deserialize, ToSchema, TS)]
#[ts(export, export_to = "paper_trading/")]
pub struct PaperTradingStartRequest {
    /// 사용할 Mock 계정 ID
    #[serde(rename = "credentialId")]
    pub credential_id: String,
    /// 초기 잔고 (옵션, 없으면 계정 기본값)
    #[serde(rename = "initialBalance")]
    #[ts(optional, type = "number")]
    pub initial_balance: Option<f64>,
}

/// Paper Trading 세션 상태 응답.
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "paper_trading/")]
pub struct PaperTradingSessionResponse {
    /// 전략 ID
    #[serde(rename = "strategyId")]
    pub strategy_id: String,
    /// 계정 ID
    #[serde(rename = "credentialId")]
    pub credential_id: String,
    /// 상태 (running, stopped, paused)
    pub status: String,
    /// 초기 잔고
    #[serde(rename = "initialBalance")]
    pub initial_balance: String,
    /// 현재 잔고
    #[serde(rename = "currentBalance")]
    pub current_balance: String,
    /// 포지션 수
    #[serde(rename = "positionCount")]
    pub position_count: i32,
    /// 거래 수
    #[serde(rename = "tradeCount")]
    pub trade_count: i32,
    /// 실현 손익
    #[serde(rename = "realizedPnl")]
    pub realized_pnl: String,
    /// 미실현 손익
    #[serde(rename = "unrealizedPnl")]
    pub unrealized_pnl: String,
    /// 수익률 (%)
    #[serde(rename = "returnPct")]
    pub return_pct: String,
    /// 시작 시간
    #[serde(rename = "startedAt")]
    pub started_at: Option<String>,
}

/// Paper Trading 시작/중지 응답.
#[derive(Debug, Serialize, ToSchema, TS)]
#[ts(export, export_to = "paper_trading/")]
pub struct PaperTradingActionResponse {
    pub success: bool,
    #[serde(rename = "strategyId")]
    pub strategy_id: String,
    pub action: String,
    pub message: String,
}

/// 전략 기반 Paper Trading 세션 목록 조회.
///
/// GET /api/v1/paper-trading/strategies
#[utoipa::path(
    get,
    path = "/api/v1/paper-trading/strategies",
    tag = "paper-trading",
    responses(
        (status = 200, description = "Paper Trading 세션 목록")
    )
)]
pub async fn list_paper_trading_sessions(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "DB 연결 없음"})),
        )
    })?;

    let rows = sqlx::query!(
        r#"
        SELECT
            pts.strategy_id,
            pts.credential_id,
            pts.status,
            pts.initial_balance,
            pts.current_balance,
            pts.started_at,
            COALESCE((SELECT COUNT(*) FROM mock_positions mp WHERE mp.strategy_id = pts.strategy_id), 0) as position_count,
            COALESCE((SELECT COUNT(*) FROM mock_executions me WHERE me.strategy_id = pts.strategy_id), 0) as trade_count,
            COALESCE((SELECT SUM(realized_pnl) FROM mock_executions me WHERE me.strategy_id = pts.strategy_id AND realized_pnl IS NOT NULL), 0) as realized_pnl
        FROM paper_trading_sessions pts
        ORDER BY pts.updated_at DESC
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB 조회 실패: {}", e)})),
        )
    })?;

    let sessions: Vec<PaperTradingSessionResponse> = rows
        .into_iter()
        .map(|row| {
            let initial_bal = row.initial_balance;
            let current_bal = row.current_balance;
            let realized_pnl = row.realized_pnl.unwrap_or(Decimal::ZERO);
            let return_pct = if initial_bal > Decimal::ZERO {
                ((current_bal - initial_bal) / initial_bal * Decimal::from(100)).round_dp(2)
            } else {
                Decimal::ZERO
            };

            PaperTradingSessionResponse {
                strategy_id: row.strategy_id,
                credential_id: row.credential_id.to_string(),
                status: row.status,
                initial_balance: initial_bal.to_string(),
                current_balance: current_bal.to_string(),
                position_count: row.position_count.unwrap_or(0) as i32,
                trade_count: row.trade_count.unwrap_or(0) as i32,
                realized_pnl: realized_pnl.to_string(),
                unrealized_pnl: "0".to_string(), // TODO: 실시간 계산
                return_pct: return_pct.to_string(),
                started_at: row.started_at.map(|t| t.to_rfc3339()),
            }
        })
        .collect();

    Ok(Json(serde_json::json!({
        "sessions": sessions,
        "total": sessions.len()
    })))
}

/// 전략별 Paper Trading 상태 조회.
///
/// GET /api/v1/paper-trading/strategies/:strategy_id/status
#[utoipa::path(
    get,
    path = "/api/v1/paper-trading/strategies/{strategy_id}/status",
    tag = "paper-trading",
    params(
        ("strategy_id" = String, Path, description = "전략 ID")
    ),
    responses(
        (status = 200, description = "Paper Trading 세션 상태", body = PaperTradingSessionResponse)
    )
)]
pub async fn get_paper_trading_status(
    State(state): State<Arc<AppState>>,
    Path(strategy_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "DB 연결 없음"})),
        )
    })?;

    let row = sqlx::query!(
        r#"
        SELECT
            pts.strategy_id,
            pts.credential_id,
            pts.status,
            pts.initial_balance,
            pts.current_balance,
            pts.started_at,
            COALESCE((SELECT COUNT(*) FROM mock_positions mp WHERE mp.strategy_id = pts.strategy_id), 0) as position_count,
            COALESCE((SELECT COUNT(*) FROM mock_executions me WHERE me.strategy_id = pts.strategy_id), 0) as trade_count,
            COALESCE((SELECT SUM(realized_pnl) FROM mock_executions me WHERE me.strategy_id = pts.strategy_id AND realized_pnl IS NOT NULL), 0) as realized_pnl
        FROM paper_trading_sessions pts
        WHERE pts.strategy_id = $1
        "#,
        strategy_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB 조회 실패: {}", e)})),
        )
    })?;

    match row {
        Some(row) => {
            let initial_bal = row.initial_balance;
            let current_bal = row.current_balance;
            let realized_pnl = row.realized_pnl.unwrap_or(Decimal::ZERO);
            let return_pct = if initial_bal > Decimal::ZERO {
                ((current_bal - initial_bal) / initial_bal * Decimal::from(100)).round_dp(2)
            } else {
                Decimal::ZERO
            };

            Ok(Json(PaperTradingSessionResponse {
                strategy_id: row.strategy_id,
                credential_id: row.credential_id.to_string(),
                status: row.status,
                initial_balance: initial_bal.to_string(),
                current_balance: current_bal.to_string(),
                position_count: row.position_count.unwrap_or(0) as i32,
                trade_count: row.trade_count.unwrap_or(0) as i32,
                realized_pnl: realized_pnl.to_string(),
                unrealized_pnl: "0".to_string(),
                return_pct: return_pct.to_string(),
                started_at: row.started_at.map(|t| t.to_rfc3339()),
            }))
        }
        None => {
            // 세션이 없으면 stopped 상태 반환
            Ok(Json(PaperTradingSessionResponse {
                strategy_id: strategy_id.clone(),
                credential_id: String::new(),
                status: "stopped".to_string(),
                initial_balance: "0".to_string(),
                current_balance: "0".to_string(),
                position_count: 0,
                trade_count: 0,
                realized_pnl: "0".to_string(),
                unrealized_pnl: "0".to_string(),
                return_pct: "0".to_string(),
                started_at: None,
            }))
        }
    }
}

/// Paper Trading 시작.
///
/// POST /api/v1/paper-trading/strategies/:strategy_id/start
#[utoipa::path(
    post,
    path = "/api/v1/paper-trading/strategies/{strategy_id}/start",
    tag = "paper-trading",
    params(
        ("strategy_id" = String, Path, description = "전략 ID")
    ),
    request_body = PaperTradingStartRequest,
    responses(
        (status = 200, description = "Paper Trading 시작 성공", body = PaperTradingActionResponse)
    )
)]
pub async fn start_paper_trading(
    State(state): State<Arc<AppState>>,
    Path(strategy_id): Path<String>,
    Json(request): Json<PaperTradingStartRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "DB 연결 없음"})),
        )
    })?;

    let credential_id = Uuid::parse_str(&request.credential_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "잘못된 credential_id 형식"})),
        )
    })?;

    // Mock 계정 확인
    let credential = sqlx::query!(
        r#"SELECT settings FROM exchange_credentials WHERE id = $1 AND exchange_id = 'mock'"#,
        credential_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB 조회 실패: {}", e)})),
        )
    })?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Mock 계정을 찾을 수 없습니다"})),
        )
    })?;

    // 초기 잔고 결정
    let settings = credential.settings.unwrap_or(serde_json::json!({}));
    let default_balance: Decimal = settings
        .get("initial_balance")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(Decimal::from(10_000_000));
    let initial_balance = request
        .initial_balance
        .map(|v| Decimal::try_from(v).unwrap_or(default_balance))
        .unwrap_or(default_balance);

    // 세션 생성/업데이트 (UPSERT)
    sqlx::query!(
        r#"
        INSERT INTO paper_trading_sessions (strategy_id, credential_id, status, initial_balance, current_balance, started_at, updated_at)
        VALUES ($1, $2, 'running', $3, $3, NOW(), NOW())
        ON CONFLICT (strategy_id) DO UPDATE SET
            credential_id = $2,
            status = 'running',
            initial_balance = $3,
            current_balance = $3,
            started_at = NOW(),
            updated_at = NOW()
        "#,
        strategy_id,
        credential_id,
        initial_balance
    )
    .execute(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("세션 생성 실패: {}", e)})),
        )
    })?;

    // 전략 시작 (Strategy Engine에 위임)
    let engine = state.strategy_engine.read().await;
    if let Err(e) = engine.start_strategy(&strategy_id).await {
        tracing::warn!("전략 시작 실패 (이미 실행 중일 수 있음): {}", e);
    }
    drop(engine);

    // 시장 유형 결정
    let market_type = settings
        .get("market_type")
        .and_then(|v| v.as_str())
        .unwrap_or("stock_kr");

    // Mock 거래소 Provider 생성/조회
    let mock_provider = get_or_create_mock_provider(
        &state,
        credential_id,
        initial_balance,
        market_type,
    ).await?;

    // 해당 계정을 사용하는 모든 활성 전략의 심볼 수집
    let symbols = collect_streaming_symbols(pool, credential_id)
        .await
        .unwrap_or_default();

    // 스트리밍 시작 (아직 시작되지 않은 경우)
    if !symbols.is_empty() && !mock_provider.is_streaming() {
        // 5초 간격으로 가격 스트리밍 시작
        if let Err(e) = mock_provider.start_streaming(symbols.clone(), 5).await {
            tracing::warn!("Mock 스트리밍 시작 실패: {:?}", e);
        } else {
            tracing::info!("Mock 스트리밍 시작 (계정: {}): {:?}", credential_id, symbols);
        }
    }

    Ok(Json(PaperTradingActionResponse {
        success: true,
        strategy_id: strategy_id.clone(),
        action: "start".to_string(),
        message: format!("Paper Trading 시작: {}", strategy_id),
    }))
}

/// Paper Trading 중지.
///
/// POST /api/v1/paper-trading/strategies/:strategy_id/stop
#[utoipa::path(
    post,
    path = "/api/v1/paper-trading/strategies/{strategy_id}/stop",
    tag = "paper-trading",
    params(
        ("strategy_id" = String, Path, description = "전략 ID")
    ),
    responses(
        (status = 200, description = "Paper Trading 중지 성공", body = PaperTradingActionResponse)
    )
)]
pub async fn stop_paper_trading(
    State(state): State<Arc<AppState>>,
    Path(strategy_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "DB 연결 없음"})),
        )
    })?;

    // 세션 상태 업데이트
    sqlx::query!(
        r#"
        UPDATE paper_trading_sessions
        SET status = 'stopped', stopped_at = NOW(), updated_at = NOW()
        WHERE strategy_id = $1
        "#,
        strategy_id
    )
    .execute(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("세션 업데이트 실패: {}", e)})),
        )
    })?;

    // 전략 중지 (Strategy Engine에 위임)
    let engine = state.strategy_engine.read().await;
    if let Err(e) = engine.stop_strategy(&strategy_id).await {
        tracing::warn!("전략 중지 실패: {}", e);
    }
    drop(engine);

    // 해당 계정에서 다른 실행 중인 전략이 있는지 확인
    let session_info = sqlx::query!(
        r#"SELECT credential_id FROM paper_trading_sessions WHERE strategy_id = $1"#,
        strategy_id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if let Some(session) = session_info {
        let credential_id = session.credential_id;

        // 같은 계정의 다른 실행 중인 전략 수 확인
        let running_count: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM paper_trading_sessions
            WHERE credential_id = $1 AND status = 'running'
            "#,
            credential_id
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        // 실행 중인 전략이 없으면 스트리밍 중지
        if running_count == 0 {
            let providers = state.mock_providers.read().await;
            if let Some(provider) = providers.get(&credential_id) {
                provider.stop_streaming().await;
                tracing::info!("Mock 스트리밍 중지 (계정: {})", credential_id);
            }
        }
    }

    Ok(Json(PaperTradingActionResponse {
        success: true,
        strategy_id: strategy_id.clone(),
        action: "stop".to_string(),
        message: format!("Paper Trading 중지: {}", strategy_id),
    }))
}

/// Paper Trading 리셋 (전략별).
///
/// POST /api/v1/paper-trading/strategies/:strategy_id/reset
#[utoipa::path(
    post,
    path = "/api/v1/paper-trading/strategies/{strategy_id}/reset",
    tag = "paper-trading",
    params(
        ("strategy_id" = String, Path, description = "전략 ID")
    ),
    responses(
        (status = 200, description = "Paper Trading 리셋 성공", body = PaperTradingActionResponse)
    )
)]
pub async fn reset_paper_trading(
    State(state): State<Arc<AppState>>,
    Path(strategy_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "DB 연결 없음"})),
        )
    })?;

    // 세션 정보 조회
    let session = sqlx::query!(
        r#"SELECT initial_balance FROM paper_trading_sessions WHERE strategy_id = $1"#,
        strategy_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB 조회 실패: {}", e)})),
        )
    })?;

    if session.is_some() {
        // 포지션 삭제
        sqlx::query!(
            r#"DELETE FROM mock_positions WHERE strategy_id = $1"#,
            strategy_id
        )
        .execute(pool)
        .await
        .ok();

        // 체결 내역 삭제
        sqlx::query!(
            r#"DELETE FROM mock_executions WHERE strategy_id = $1"#,
            strategy_id
        )
        .execute(pool)
        .await
        .ok();

        // 세션 잔고 초기화
        sqlx::query!(
            r#"
            UPDATE paper_trading_sessions
            SET current_balance = initial_balance, status = 'stopped', stopped_at = NOW(), updated_at = NOW()
            WHERE strategy_id = $1
            "#,
            strategy_id
        )
        .execute(pool)
        .await
        .ok();
    }

    Ok(Json(PaperTradingActionResponse {
        success: true,
        strategy_id: strategy_id.clone(),
        action: "reset".to_string(),
        message: format!("Paper Trading 리셋 완료: {}", strategy_id),
    }))
}

/// 전략별 포지션 조회.
///
/// GET /api/v1/paper-trading/strategies/:strategy_id/positions
#[utoipa::path(
    get,
    path = "/api/v1/paper-trading/strategies/{strategy_id}/positions",
    tag = "paper-trading",
    params(
        ("strategy_id" = String, Path, description = "전략 ID")
    ),
    responses(
        (status = 200, description = "포지션 목록", body = PaperTradingPositionsResponse)
    )
)]
pub async fn get_strategy_positions(
    State(state): State<Arc<AppState>>,
    Path(strategy_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "DB 연결 없음"})),
        )
    })?;

    let rows = sqlx::query!(
        r#"
        SELECT symbol, side, quantity, entry_price, entry_time
        FROM mock_positions
        WHERE strategy_id = $1
        ORDER BY entry_time DESC
        "#,
        strategy_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB 조회 실패: {}", e)})),
        )
    })?;

    let mut positions = Vec::new();
    let mut total_value = Decimal::ZERO;

    for row in rows {
        let current_price = row.entry_price; // TODO: 실시간 시세 조회
        let market_value = row.quantity * current_price;
        total_value += market_value;

        let return_pct = if row.entry_price > Decimal::ZERO {
            ((current_price - row.entry_price) / row.entry_price * Decimal::from(100)).round_dp(2)
        } else {
            Decimal::ZERO
        };

        positions.push(PaperTradingPosition {
            symbol: row.symbol,
            side: if row.side == "Buy" { "Long".to_string() } else { "Short".to_string() },
            quantity: row.quantity.to_string(),
            entry_price: row.entry_price.to_string(),
            current_price: current_price.to_string(),
            market_value: market_value.to_string(),
            unrealized_pnl: "0".to_string(),
            return_pct: return_pct.to_string(),
            entry_time: row.entry_time.to_rfc3339(),
        });
    }

    Ok(Json(PaperTradingPositionsResponse {
        total: positions.len(),
        positions,
        total_value: total_value.to_string(),
        total_unrealized_pnl: "0".to_string(),
    }))
}

/// 전략별 체결 내역 조회.
///
/// GET /api/v1/paper-trading/strategies/:strategy_id/trades
#[utoipa::path(
    get,
    path = "/api/v1/paper-trading/strategies/{strategy_id}/trades",
    tag = "paper-trading",
    params(
        ("strategy_id" = String, Path, description = "전략 ID")
    ),
    responses(
        (status = 200, description = "체결 내역", body = PaperTradingExecutionsResponse)
    )
)]
pub async fn get_strategy_trades(
    State(state): State<Arc<AppState>>,
    Path(strategy_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "DB 연결 없음"})),
        )
    })?;

    let rows = sqlx::query!(
        r#"
        SELECT id, symbol, side, quantity, price, commission, realized_pnl, executed_at
        FROM mock_executions
        WHERE strategy_id = $1
        ORDER BY executed_at DESC
        LIMIT 100
        "#,
        strategy_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB 조회 실패: {}", e)})),
        )
    })?;

    let executions: Vec<PaperTradingExecution> = rows
        .into_iter()
        .map(|row| PaperTradingExecution {
            id: row.id.to_string(),
            symbol: row.symbol,
            side: row.side,
            quantity: row.quantity.to_string(),
            price: row.price.to_string(),
            commission: row.commission.to_string(),
            realized_pnl: row.realized_pnl.map(|v| v.to_string()),
            executed_at: row.executed_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(PaperTradingExecutionsResponse {
        total: executions.len(),
        executions,
    }))
}
