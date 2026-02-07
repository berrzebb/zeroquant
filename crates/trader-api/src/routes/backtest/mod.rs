//! 백테스트 API 엔드포인트
//!
//! 과거 데이터로 트레이딩 전략을 시뮬레이션하고 성과를 분석하는 API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/backtest/strategies` - 백테스트 가능한 전략 목록
//! - `POST /api/v1/backtest/run` - 백테스트 실행
//! - `GET /api/v1/backtest/results/{id}` - 백테스트 결과 조회

mod engine;
mod loader;
mod types;
mod ui_schema;

// Re-export public types
pub use types::{
    BacktestApiError,
    BacktestConfigSummary,
    BacktestMetricsResponse,
    BacktestMultiRunRequest,
    BacktestMultiRunResponse,
    BacktestRunRequest,
    BacktestRunResponse,
    BacktestStrategiesResponse,
    BacktestableStrategy,
    BatchBacktestItem,
    // 배치 백테스트
    BatchBacktestRequest,
    BatchBacktestResponse,
    BatchBacktestResultItem,
    EquityCurvePoint,
    ExecutionSchedule,
    // 다중 타임프레임
    MultiTimeframeRequest,
    SecondaryTimeframeConfig,
    SymbolCategory,
    TradeHistoryItem,
    UiCondition,
    UiConditionOperator,
    UiField,
    UiFieldGroup,
    UiFieldType,
    UiLayout,
    UiSchema,
    UiSelectOption,
    UiValidation,
};

// Re-export UI schema functions
pub use ui_schema::get_ui_schema_for_strategy;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::sync::Arc;
use tracing::{debug, warn};

use crate::state::AppState;
use trader_analytics::backtest::BacktestConfig;
use trader_strategy::StrategyRegistry;

use engine::{
    convert_multi_report_to_response, convert_report_to_response, generate_multi_sample_klines,
    run_multi_strategy_backtest, run_strategy_backtest,
};
use loader::{
    expand_strategy_symbols, generate_sample_klines, load_klines_from_db,
    load_multi_klines_from_db, merge_multi_klines,
};
// ui_schema 함수들은 get_ui_schema_for_strategy로 대체됨

// ==================== 핸들러 ====================

/// 백테스트 가능한 전략 목록 조회.
///
/// 현재 등록된 모든 전략 중 백테스트가 가능한 전략 목록을 반환합니다.
#[utoipa::path(
    get,
    path = "/api/v1/backtest/strategies",
    tag = "backtest",
    responses(
        (status = 200, description = "전략 목록 조회 성공", body = BacktestStrategiesResponse)
    )
)]
pub async fn list_backtest_strategies(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // 최소 락 홀드: 데이터만 빠르게 복사하고 즉시 락 해제
    let all_statuses = {
        let engine = state.strategy_engine.read().await;
        engine.get_all_statuses().await
    }; // 락 해제됨

    // 락 없이 계산 수행
    let strategies: Vec<BacktestableStrategy> = all_statuses
        .into_iter()
        .map(|(id, status)| {
            let ui_schema = get_ui_schema_for_strategy(&id);
            BacktestableStrategy {
                id: id.clone(),
                name: status.name,
                description: format!("버전 {}", status.version),
                supported_symbols: vec!["BTC/USDT".to_string(), "ETH/USDT".to_string()],
                default_params: serde_json::json!({
                    "period": 14,
                    "threshold": 30.0
                }),
                ui_schema,
                category: Some("사용자정의".to_string()),
                tags: vec!["플러그인".to_string()],
                execution_schedule: None,
                schedule_detail: None,
                how_it_works: None,
            }
        })
        .collect();

    // 기본 내장 전략 추가 (전략 엔진에 등록되지 않은 경우)
    let mut all_strategies = strategies;

    // 구현된 모든 전략 목록 (SDUI 스키마 포함)
    let builtin_strategies = get_builtin_strategies();

    // 아직 추가되지 않은 전략만 추가
    for strategy in builtin_strategies {
        if !all_strategies.iter().any(|s| s.id == strategy.id) {
            all_strategies.push(strategy);
        }
    }

    let total = all_strategies.len();

    Json(BacktestStrategiesResponse {
        strategies: all_strategies,
        total,
    })
}

/// 백테스트 실행.
///
/// 주어진 설정으로 백테스트를 실행하고 결과를 반환합니다.
#[utoipa::path(
    post,
    path = "/api/v1/backtest/run",
    tag = "backtest",
    request_body = BacktestRunRequest,
    responses(
        (status = 200, description = "백테스트 실행 성공", body = BacktestRunResponse),
        (status = 400, description = "잘못된 요청", body = BacktestApiError),
        (status = 404, description = "전략 없음", body = BacktestApiError),
        (status = 500, description = "서버 오류", body = BacktestApiError)
    )
)]
pub async fn run_backtest(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BacktestRunRequest>,
) -> Result<Json<BacktestRunResponse>, (StatusCode, Json<BacktestApiError>)> {
    debug!(
        "백테스트 실행 요청: strategy={}, symbol={}",
        request.strategy_id, request.symbol
    );

    // 날짜 파싱 검증
    let start_date = NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d").map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_DATE",
                format!("잘못된 시작 날짜 형식: {}", request.start_date),
            )),
        )
    })?;

    let end_date = NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d").map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_DATE",
                format!("잘못된 종료 날짜 형식: {}", request.end_date),
            )),
        )
    })?;

    // 날짜 유효성 검사
    if end_date <= start_date {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_DATE_RANGE",
                "종료 날짜는 시작 날짜보다 이후여야 합니다",
            )),
        ));
    }

    // 초기 자본금 검증
    if request.initial_capital <= Decimal::ZERO {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_CAPITAL",
                "초기 자본금은 0보다 커야 합니다",
            )),
        ));
    }

    // 전략 레지스트리에서 동적으로 전략 확인
    if StrategyRegistry::find(&request.strategy_id).is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(BacktestApiError::new(
                "STRATEGY_NOT_FOUND",
                format!("전략을 찾을 수 없습니다: {}", request.strategy_id),
            )),
        ));
    }

    // 수수료/슬리피지 기본값 설정
    let commission_rate = request.commission_rate.unwrap_or(Decimal::new(1, 3)); // 0.1%
    let slippage_rate = request.slippage_rate.unwrap_or(Decimal::new(5, 4)); // 0.05%

    // 전략별로 필요한 심볼을 동적으로 확장 (하드코딩 없이 expand_strategy_symbols에 위임)
    let user_symbols = vec![request.symbol.clone()];
    let expanded_symbols = expand_strategy_symbols(&request.strategy_id, &user_symbols);

    // 확장된 심볼이 1개 초과이면 다중 심볼 전략으로 처리
    let is_multi_symbol_strategy = expanded_symbols.len() > 1;

    if is_multi_symbol_strategy {
        debug!(
            "다중 심볼 전략 {} 심볼 확장: {:?} -> {:?}",
            request.strategy_id, user_symbols, expanded_symbols
        );

        // 다중 심볼 데이터 로드 (공유 data_provider 사용 - Redis 3계층 캐시)
        let multi_klines = if let Some(data_provider) = &state.data_provider {
            match load_multi_klines_from_db(data_provider, &expanded_symbols, start_date, end_date).await {
                Ok(data) if !data.is_empty() => {
                    debug!("DB에서 {} 심볼의 데이터 로드 완료", data.len());
                    for (sym, klines) in &data {
                        debug!("  - {} 심볼: {} 개 캔들", sym, klines.len());
                    }
                    data
                }
                Ok(_) => {
                    warn!(
                        symbols = ?expanded_symbols,
                        "DB에 데이터가 없어 샘플 데이터로 백테스트 실행"
                    );
                    generate_multi_sample_klines(&expanded_symbols, start_date, end_date)
                }
                Err(e) => {
                    warn!(
                        symbols = ?expanded_symbols,
                        error = %e,
                        "DB 로드 실패, 샘플 데이터 사용"
                    );
                    generate_multi_sample_klines(&expanded_symbols, start_date, end_date)
                }
            }
        } else {
            warn!(
                symbols = ?expanded_symbols,
                "data_provider 없음! DATABASE_URL이 설정되었는지 확인하세요. 샘플 데이터로 백테스트 실행"
            );
            generate_multi_sample_klines(&expanded_symbols, start_date, end_date)
        };

        // 모든 심볼의 캔들 데이터를 시간순으로 병합
        let merged_klines = merge_multi_klines(&multi_klines);

        if merged_klines.is_empty() {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BacktestApiError::new(
                    "NO_DATA",
                    "백테스트를 위한 데이터가 없습니다",
                )),
            ));
        }

        // 백테스트 설정
        let config = BacktestConfig::new(request.initial_capital)
            .with_commission_rate(commission_rate)
            .with_slippage_rate(slippage_rate);

        // 모든 전략은 동일한 run_strategy_backtest 함수로 처리 (하드코딩 방지)
        // 병합된 캔들 데이터를 전달하여 전략이 필요한 심볼 데이터를 자체적으로 처리
        let report = run_strategy_backtest(
            &request.strategy_id,
            config,
            &merged_klines,
            &request.parameters,
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BacktestApiError::new("BACKTEST_ERROR", e.to_string())),
            )
        })?;

        // BacktestReport를 API 응답으로 변환 (다중 심볼 표시)
        let symbols_str = expanded_symbols.join(",");
        let response = convert_report_to_response(
            &report,
            &request.strategy_id,
            &symbols_str,
            &request.start_date,
            &request.end_date,
        );

        debug!(
            "다중 심볼 백테스트 완료: total_return={:.2}%",
            report.metrics.total_return_pct
        );

        return Ok(Json(response));
    }

    // 단일 심볼 전략 (공유 data_provider 사용 - Redis 3계층 캐시)
    let klines = if let Some(data_provider) = &state.data_provider {
        match load_klines_from_db(data_provider, &request.symbol, start_date, end_date).await {
            Ok(data) if !data.is_empty() => {
                debug!(
                    symbol = %request.symbol,
                    count = data.len(),
                    start = %start_date,
                    end = %end_date,
                    "DB에서 캔들 데이터 로드 완료"
                );
                data
            }
            Ok(_) => {
                warn!(
                    symbol = %request.symbol,
                    start = %start_date,
                    end = %end_date,
                    "DB에 데이터가 없어 샘플 데이터로 백테스트 실행"
                );
                generate_sample_klines(&request.symbol, start_date, end_date)
            }
            Err(e) => {
                warn!(
                    symbol = %request.symbol,
                    error = %e,
                    start = %start_date,
                    end = %end_date,
                    "DB 로드 실패, 샘플 데이터 사용"
                );
                generate_sample_klines(&request.symbol, start_date, end_date)
            }
        }
    } else {
        // WARN으로 변경: data_provider가 없으면 심각한 설정 문제
        warn!(
            symbol = %request.symbol,
            "data_provider 없음! DATABASE_URL이 설정되었는지 확인하세요. 샘플 데이터로 백테스트 실행"
        );
        generate_sample_klines(&request.symbol, start_date, end_date)
    };

    // 백테스트 설정
    let config = BacktestConfig::new(request.initial_capital)
        .with_commission_rate(commission_rate)
        .with_slippage_rate(slippage_rate);

    // 전략별 백테스트 실행
    let report = run_strategy_backtest(&request.strategy_id, config, &klines, &request.parameters)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BacktestApiError::new("BACKTEST_ERROR", e.to_string())),
            )
        })?;

    // BacktestReport를 API 응답으로 변환
    let response = convert_report_to_response(
        &report,
        &request.strategy_id,
        &request.symbol,
        &request.start_date,
        &request.end_date,
    );

    debug!(
        "백테스트 완료: total_return={:.2}%",
        report.metrics.total_return_pct
    );

    Ok(Json(response))
}

/// 백테스트 결과 조회.
///
/// 저장된 백테스트 결과를 조회합니다.
#[utoipa::path(
    get,
    path = "/api/v1/backtest/results/{id}",
    tag = "backtest",
    params(
        ("id" = String, Path, description = "백테스트 결과 ID")
    ),
    responses(
        (status = 200, description = "결과 조회 성공"),
        (status = 404, description = "결과 없음", body = BacktestApiError)
    )
)]
pub async fn get_backtest_result(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<BacktestApiError>)> {
    // 현재는 저장 기능이 없으므로 NOT_FOUND 반환
    // 추후 데이터베이스 연동 시 구현
    Err((
        StatusCode::NOT_FOUND,
        Json(BacktestApiError::new(
            "RESULT_NOT_FOUND",
            format!("백테스트 결과를 찾을 수 없습니다: {}", id),
        )),
    ))
}

/// 다중 자산 백테스트 실행.
///
/// 여러 심볼을 사용하는 자산배분 전략의 백테스트를 실행합니다.
#[utoipa::path(
    post,
    path = "/api/v1/backtest/run-multi",
    tag = "backtest",
    request_body = BacktestMultiRunRequest,
    responses(
        (status = 200, description = "다중 자산 백테스트 성공", body = BacktestMultiRunResponse),
        (status = 400, description = "잘못된 요청", body = BacktestApiError),
        (status = 500, description = "서버 오류", body = BacktestApiError)
    )
)]
pub async fn run_multi_backtest(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BacktestMultiRunRequest>,
) -> Result<Json<BacktestMultiRunResponse>, (StatusCode, Json<BacktestApiError>)> {
    debug!(
        "다중 자산 백테스트 실행 요청: strategy={}, symbols={:?}",
        request.strategy_id, request.symbols
    );

    // 날짜 파싱 검증
    let start_date = NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d").map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_DATE",
                format!("잘못된 시작 날짜 형식: {}", request.start_date),
            )),
        )
    })?;

    let end_date = NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d").map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_DATE",
                format!("잘못된 종료 날짜 형식: {}", request.end_date),
            )),
        )
    })?;

    // 날짜 유효성 검사
    if end_date <= start_date {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_DATE_RANGE",
                "종료 날짜는 시작 날짜보다 이후여야 합니다",
            )),
        ));
    }

    // 심볼 목록 검증
    if request.symbols.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_SYMBOLS",
                "최소 하나의 심볼이 필요합니다",
            )),
        ));
    }

    // 초기 자본금 검증
    if request.initial_capital <= Decimal::ZERO {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_CAPITAL",
                "초기 자본금은 0보다 커야 합니다",
            )),
        ));
    }

    // 다중 자산 전략만 허용
    let valid_multi_strategies = [
        "compound_momentum",
        "haa",
        "xaa",
        "stock_rotation",
        // 추가 다중 자산 전략들
        "all_weather",
        "momentum_power",
        "baa",
        "sector_momentum",
        "dual_momentum",
        "pension_bot",
        "market_cap_top",
    ];
    if !valid_multi_strategies.contains(&request.strategy_id.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_STRATEGY",
                format!(
                    "다중 자산 백테스트는 다음 전략만 지원합니다: {:?}",
                    valid_multi_strategies
                ),
            )),
        ));
    }

    // 수수료/슬리피지 기본값 설정
    let commission_rate = request.commission_rate.unwrap_or(Decimal::new(1, 3));
    let slippage_rate = request.slippage_rate.unwrap_or(Decimal::new(5, 4));

    // 전략별로 필요한 모든 심볼 확장
    let expanded_symbols = expand_strategy_symbols(&request.strategy_id, &request.symbols);
    debug!(
        "전략 {} 심볼 확장: {:?} -> {:?}",
        request.strategy_id, request.symbols, expanded_symbols
    );

    // 다중 심볼 데이터 로드 (공유 data_provider 사용 - Redis 3계층 캐시)
    let multi_klines = if let Some(data_provider) = &state.data_provider {
        match load_multi_klines_from_db(data_provider, &expanded_symbols, start_date, end_date).await {
            Ok(data) if !data.is_empty() => {
                debug!(
                    symbol_count = data.len(),
                    "DB에서 심볼 데이터 로드 완료"
                );
                data
            }
            Ok(_) => {
                warn!(
                    symbols = ?expanded_symbols,
                    "DB에 데이터가 없어 샘플 데이터로 백테스트 실행"
                );
                generate_multi_sample_klines(&expanded_symbols, start_date, end_date)
            }
            Err(e) => {
                warn!(
                    symbols = ?expanded_symbols,
                    error = %e,
                    "DB 로드 실패, 샘플 데이터 사용"
                );
                generate_multi_sample_klines(&expanded_symbols, start_date, end_date)
            }
        }
    } else {
        warn!(
            symbols = ?expanded_symbols,
            "data_provider 없음! DATABASE_URL이 설정되었는지 확인하세요. 샘플 데이터로 백테스트 실행"
        );
        generate_multi_sample_klines(&expanded_symbols, start_date, end_date)
    };

    // 심볼별 데이터 포인트 수 계산
    let data_points_by_symbol: std::collections::HashMap<String, usize> = multi_klines
        .iter()
        .map(|(symbol, klines)| (symbol.clone(), klines.len()))
        .collect();

    // 모든 심볼의 캔들 데이터를 시간순으로 병합
    let merged_klines = merge_multi_klines(&multi_klines);

    if merged_klines.is_empty() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(BacktestApiError::new(
                "NO_DATA",
                "백테스트를 위한 데이터가 없습니다",
            )),
        ));
    }

    // 백테스트 설정
    let config = BacktestConfig::new(request.initial_capital)
        .with_commission_rate(commission_rate)
        .with_slippage_rate(slippage_rate);

    // 전략별 백테스트 실행 (다중 심볼 지원)
    let report = run_multi_strategy_backtest(
        &request.strategy_id,
        config,
        &merged_klines,
        &multi_klines,
        &request.parameters,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(BacktestApiError::new("BACKTEST_ERROR", e.to_string())),
        )
    })?;

    // BacktestReport를 API 응답으로 변환
    let response = convert_multi_report_to_response(
        &report,
        &request.strategy_id,
        &request.symbols,
        &request.start_date,
        &request.end_date,
        data_points_by_symbol,
    );

    debug!(
        "다중 자산 백테스트 완료: total_return={:.2}%",
        report.metrics.total_return_pct
    );

    Ok(Json(response))
}

// ==================== 라우터 ====================

/// 백테스트 라우터 생성
pub fn backtest_router() -> Router<Arc<AppState>> {
    Router::new()
        // 백테스트 가능한 전략 목록
        .route("/strategies", get(list_backtest_strategies))
        // 백테스트 실행 (단일 심볼)
        .route("/run", post(run_backtest))
        // 다중 자산 백테스트 실행
        .route("/run-multi", post(run_multi_backtest))
        // 배치 백테스트 (병렬 실행)
        .route("/run-batch", post(run_batch_backtest))
    // 백테스트 결과 조회는 backtest_results_router에서 처리
}

/// 배치 백테스트 실행 (병렬).
///
/// 여러 전략을 병렬로 백테스트합니다. 최대 10개 전략을 동시에 실행할 수 있습니다.
#[utoipa::path(
    post,
    path = "/api/v1/backtest/run-batch",
    tag = "backtest",
    request_body = BatchBacktestRequest,
    responses(
        (status = 200, description = "배치 백테스트 성공", body = BatchBacktestResponse),
        (status = 400, description = "잘못된 요청", body = BacktestApiError),
        (status = 500, description = "서버 오류", body = BacktestApiError)
    )
)]
pub async fn run_batch_backtest(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BatchBacktestRequest>,
) -> Result<Json<BatchBacktestResponse>, (StatusCode, Json<BacktestApiError>)> {
    use futures::stream::{self, StreamExt};
    use std::time::Instant;
    use validator::Validate;

    // 입력 유효성 검사
    if let Err(errors) = request.validate() {
        let message = errors
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
            .collect::<Vec<_>>()
            .join("; ");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new("VALIDATION_ERROR", message)),
        ));
    }

    let start_time = Instant::now();
    let request_id = uuid::Uuid::new_v4().to_string();
    let parallelism = request.parallelism.unwrap_or(4).min(10);

    debug!(
        request_id = %request_id,
        strategies = request.strategies.len(),
        parallelism = parallelism,
        "Starting batch backtest"
    );

    // 날짜 파싱
    let start_date = NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_DATE",
                format!("시작 날짜 파싱 실패: {}", e),
            )),
        )
    })?;
    let end_date = NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(BacktestApiError::new(
                "INVALID_DATE",
                format!("종료 날짜 파싱 실패: {}", e),
            )),
        )
    })?;

    // 수수료/슬리피지 기본값
    let commission_rate = request.commission_rate.unwrap_or(Decimal::new(1, 3));
    let slippage_rate = request.slippage_rate.unwrap_or(Decimal::new(5, 4));

    // 각 전략에 대한 백테스트 Future 생성
    let backtest_futures: Vec<_> = request
        .strategies
        .into_iter()
        .map(|item| {
            let state = Arc::clone(&state);
            let initial_capital = request.initial_capital;
            let strategy_id = item.strategy_id.clone();

            async move {
                let task_start = Instant::now();

                // 심볼에 따라 단일/다중 자산 백테스트 선택
                let result = if item.symbols.len() == 1 {
                    // 단일 심볼 백테스트
                    let symbol = item.symbols[0].clone();
                    run_single_strategy_internal(
                        &state,
                        &strategy_id,
                        &symbol,
                        start_date,
                        end_date,
                        initial_capital,
                        commission_rate,
                        slippage_rate,
                        &item.parameters,
                    )
                    .await
                } else {
                    // 다중 심볼 백테스트
                    run_multi_strategy_internal(
                        &state,
                        &strategy_id,
                        &item.symbols,
                        start_date,
                        end_date,
                        initial_capital,
                        commission_rate,
                        slippage_rate,
                        &item.parameters,
                    )
                    .await
                };

                let execution_time_ms = task_start.elapsed().as_millis() as u64;

                match result {
                    Ok(metrics) => BatchBacktestResultItem {
                        strategy_id,
                        success: true,
                        error: None,
                        metrics: Some(metrics),
                        execution_time_ms,
                    },
                    Err(e) => BatchBacktestResultItem {
                        strategy_id,
                        success: false,
                        error: Some(e),
                        metrics: None,
                        execution_time_ms,
                    },
                }
            }
        })
        .collect();

    // 병렬 실행 (parallelism 제한 적용: buffer_unordered 사용)
    let total_strategies = backtest_futures.len();
    let results: Vec<BatchBacktestResultItem> = stream::iter(backtest_futures)
        .buffer_unordered(parallelism)
        .collect()
        .await;

    let total_execution_time_ms = start_time.elapsed().as_millis() as u64;
    let successful = results.iter().filter(|r| r.success).count();
    let failed = results.iter().filter(|r| !r.success).count();

    debug!(
        request_id = %request_id,
        successful = successful,
        failed = failed,
        total_time_ms = total_execution_time_ms,
        "Batch backtest completed"
    );

    Ok(Json(BatchBacktestResponse {
        request_id,
        total_strategies,
        successful,
        failed,
        total_execution_time_ms,
        results,
    }))
}

/// 단일 전략 내부 실행 (배치용).
#[allow(clippy::too_many_arguments)]
async fn run_single_strategy_internal(
    state: &Arc<AppState>,
    strategy_id: &str,
    symbol: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    initial_capital: Decimal,
    commission_rate: Decimal,
    slippage_rate: Decimal,
    params: &Option<serde_json::Value>,
) -> Result<BacktestMetricsResponse, String> {
    use trader_analytics::backtest::BacktestConfig;

    // 데이터 로드 (공유 data_provider 사용 - Redis 3계층 캐시)
    let klines = if let Some(data_provider) = &state.data_provider {
        match load_klines_from_db(data_provider, symbol, start_date, end_date).await {
            Ok(data) if !data.is_empty() => data,
            _ => generate_sample_klines(symbol, start_date, end_date),
        }
    } else {
        generate_sample_klines(symbol, start_date, end_date)
    };

    if klines.is_empty() {
        return Err("데이터 없음".to_string());
    }

    // 백테스트 설정
    let config = BacktestConfig::new(initial_capital)
        .with_commission_rate(commission_rate)
        .with_slippage_rate(slippage_rate);

    // 백테스트 실행
    let report = run_strategy_backtest(strategy_id, config, &klines, params)
        .await
        .map_err(|e| e.to_string())?;

    // 메트릭만 반환
    Ok(convert_report_to_metrics(&report))
}

/// 다중 자산 전략 내부 실행 (배치용).
#[allow(clippy::too_many_arguments)]
async fn run_multi_strategy_internal(
    state: &Arc<AppState>,
    strategy_id: &str,
    symbols: &[String],
    start_date: NaiveDate,
    end_date: NaiveDate,
    initial_capital: Decimal,
    commission_rate: Decimal,
    slippage_rate: Decimal,
    params: &Option<serde_json::Value>,
) -> Result<BacktestMetricsResponse, String> {
    use trader_analytics::backtest::BacktestConfig;

    // 심볼 확장
    let expanded_symbols = expand_strategy_symbols(strategy_id, symbols);

    // 다중 심볼 데이터 로드 (공유 data_provider 사용 - Redis 3계층 캐시)
    let multi_klines = if let Some(data_provider) = &state.data_provider {
        match load_multi_klines_from_db(data_provider, &expanded_symbols, start_date, end_date).await {
            Ok(data) if !data.is_empty() => data,
            Ok(_) => {
                warn!(strategy = %strategy_id, "배치: DB에 데이터 없음, 샘플 데이터 사용");
                generate_multi_sample_klines(&expanded_symbols, start_date, end_date)
            }
            Err(e) => {
                warn!(strategy = %strategy_id, error = %e, "배치: DB 로드 실패, 샘플 데이터 사용");
                generate_multi_sample_klines(&expanded_symbols, start_date, end_date)
            }
        }
    } else {
        warn!(strategy = %strategy_id, "배치: data_provider 없음, 샘플 데이터 사용");
        generate_multi_sample_klines(&expanded_symbols, start_date, end_date)
    };

    // 병합
    let merged_klines = merge_multi_klines(&multi_klines);

    if merged_klines.is_empty() {
        return Err("데이터 없음".to_string());
    }

    // 백테스트 설정
    let config = BacktestConfig::new(initial_capital)
        .with_commission_rate(commission_rate)
        .with_slippage_rate(slippage_rate);

    // 백테스트 실행
    let report =
        run_multi_strategy_backtest(strategy_id, config, &merged_klines, &multi_klines, params)
            .await
            .map_err(|e| e.to_string())?;

    // 메트릭만 반환
    Ok(convert_report_to_metrics(&report))
}

/// BacktestReport에서 메트릭만 추출.
fn convert_report_to_metrics(
    report: &trader_analytics::backtest::BacktestReport,
) -> BacktestMetricsResponse {
    let metrics = &report.metrics;
    BacktestMetricsResponse {
        total_return_pct: metrics.total_return_pct,
        annualized_return_pct: metrics.annualized_return_pct,
        net_profit: metrics.net_profit,
        total_trades: metrics.total_trades,
        win_rate_pct: metrics.win_rate_pct,
        profit_factor: metrics.profit_factor,
        sharpe_ratio: metrics.sharpe_ratio,
        sortino_ratio: metrics.sortino_ratio,
        max_drawdown_pct: metrics.max_drawdown_pct,
        calmar_ratio: metrics.calmar_ratio,
        avg_win: metrics.avg_win,
        avg_loss: metrics.avg_loss,
        largest_win: metrics.largest_win,
        largest_loss: metrics.largest_loss,
    }
}

// ==================== 내장 전략 목록 ====================

/// StrategyUISchema를 UiSchema로 변환
fn convert_core_schema_to_ui_schema(
    schema: &trader_core::StrategyUISchema,
) -> UiSchema {
    let fields: Vec<UiField> = schema
        .custom_fields
        .iter()
        .enumerate()
        .map(|(idx, field)| {
            // FieldType -> UiFieldType 변환
            let field_type = match field.field_type {
                trader_core::FieldType::Integer => UiFieldType::Number,
                trader_core::FieldType::Number => UiFieldType::Number,
                trader_core::FieldType::Boolean => UiFieldType::Boolean,
                trader_core::FieldType::String => UiFieldType::Text,
                trader_core::FieldType::Select => UiFieldType::Select,
                trader_core::FieldType::MultiSelect => UiFieldType::Select,
                trader_core::FieldType::Symbol => UiFieldType::SymbolPicker,
                trader_core::FieldType::Symbols => UiFieldType::SymbolPicker,
                trader_core::FieldType::MultiTimeframe => UiFieldType::Timeframe,
            };

            // options 변환 (Select 타입용)
            let options = if !field.options.is_empty() {
                Some(
                    field
                        .options
                        .iter()
                        .map(|opt| UiSelectOption {
                            label: opt.clone(),
                            value: serde_json::Value::String(opt.clone()),
                            description: None,
                        })
                        .collect(),
                )
            } else {
                None
            };

            // UiValidation 구성
            let validation = UiValidation {
                required: field.required,
                min: field.min,
                max: field.max,
                step: None,
                min_length: None,
                max_length: None,
                pattern: None,
                min_items: None,
                max_items: None,
            };

            UiField {
                key: field.name.clone(),
                label: field.label.clone(),
                field_type,
                default_value: field.default.clone(),
                placeholder: None,
                help_text: field.description.clone(),
                validation,
                options,
                symbol_categories: None,
                group: None,
                order: idx as i32,
                show_when: None,
                unit: None,
            }
        })
        .collect();

    UiSchema {
        fields,
        groups: Vec::new(),
        layout: None,
    }
}

/// ExecutionSchedule을 StrategyCategory에서 추론
fn infer_execution_schedule(
    category: trader_strategy::StrategyCategory,
) -> ExecutionSchedule {
    match category {
        trader_strategy::StrategyCategory::Realtime => ExecutionSchedule::Realtime,
        trader_strategy::StrategyCategory::Intraday => ExecutionSchedule::OnCandleClose,
        trader_strategy::StrategyCategory::Daily => ExecutionSchedule::Daily,
        trader_strategy::StrategyCategory::Monthly => ExecutionSchedule::Monthly,
    }
}

/// 구현된 모든 내장 전략 목록을 반환 (StrategyRegistry 기반)
fn get_builtin_strategies() -> Vec<BacktestableStrategy> {
    StrategyRegistry::all()
        .map(|meta| {
            // UI 스키마: 팩토리가 있으면 변환, 없으면 기존 방식
            let ui_schema = if let Some(schema_factory) = meta.ui_schema_factory {
                Some(convert_core_schema_to_ui_schema(&schema_factory()))
            } else {
                // 레거시: 기존 get_ui_schema_for_strategy 함수 사용
                get_ui_schema_for_strategy(meta.id)
            };

            // 카테고리 문자열
            let category_str = match meta.category {
                trader_strategy::StrategyCategory::Realtime => "실시간",
                trader_strategy::StrategyCategory::Intraday => "일중",
                trader_strategy::StrategyCategory::Daily => "일간",
                trader_strategy::StrategyCategory::Monthly => "월간",
            };

            BacktestableStrategy {
                id: meta.id.to_string(),
                name: meta.name.to_string(),
                description: meta.description.to_string(),
                supported_symbols: meta
                    .default_tickers
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                default_params: serde_json::json!({}),
                ui_schema,
                category: Some(category_str.to_string()),
                tags: vec![category_str.to_string()],
                execution_schedule: Some(infer_execution_schedule(meta.category)),
                schedule_detail: None,
                how_it_works: Some(meta.description.to_string()),
            }
        })
        .collect()
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
    async fn test_list_backtest_strategies() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/strategies", get(list_backtest_strategies))
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
        let list: BacktestStrategiesResponse = serde_json::from_slice(&body).unwrap();

        // 최소 3개의 기본 전략이 있어야 함
        assert!(list.total >= 3);
        assert!(list.strategies.iter().any(|s| s.id == "sma_crossover"));
        assert!(list.strategies.iter().any(|s| s.id == "rsi_mean_reversion"));
        assert!(list.strategies.iter().any(|s| s.id == "grid_trading"));
    }

    #[tokio::test]
    async fn test_run_backtest_success() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/run", post(run_backtest))
            .with_state(state);

        let request_body = serde_json::json!({
            "strategy_id": "sma_crossover",
            "symbol": "BTC/USDT",
            "start_date": "2024-01-01",
            "end_date": "2024-06-30",
            "initial_capital": 10000000
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let result: BacktestRunResponse = serde_json::from_slice(&body).unwrap();

        assert!(result.success);
        assert_eq!(result.strategy_id, "sma_crossover");
        assert_eq!(result.symbol, "BTC/USDT");
        assert!(!result.equity_curve.is_empty());
        // trades는 샘플 데이터에서 거래 신호가 발생하지 않을 수 있음
        // 실제 DB 데이터에서는 trades가 생성됨
    }

    #[tokio::test]
    async fn test_run_backtest_invalid_date() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/run", post(run_backtest))
            .with_state(state);

        let request_body = serde_json::json!({
            "strategy_id": "sma_crossover",
            "symbol": "BTC/USDT",
            "start_date": "invalid-date",
            "end_date": "2024-06-30",
            "initial_capital": 10000000
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error: BacktestApiError = serde_json::from_slice(&body).unwrap();

        assert_eq!(error.code, "INVALID_DATE");
    }

    #[tokio::test]
    async fn test_run_backtest_invalid_date_range() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/run", post(run_backtest))
            .with_state(state);

        // 종료일이 시작일보다 이전
        let request_body = serde_json::json!({
            "strategy_id": "sma_crossover",
            "symbol": "BTC/USDT",
            "start_date": "2024-06-30",
            "end_date": "2024-01-01",
            "initial_capital": 10000000
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error: BacktestApiError = serde_json::from_slice(&body).unwrap();

        assert_eq!(error.code, "INVALID_DATE_RANGE");
    }

    #[tokio::test]
    async fn test_run_backtest_strategy_not_found() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/run", post(run_backtest))
            .with_state(state);

        let request_body = serde_json::json!({
            "strategy_id": "nonexistent_strategy",
            "symbol": "BTC/USDT",
            "start_date": "2024-01-01",
            "end_date": "2024-06-30",
            "initial_capital": 10000000
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let error: BacktestApiError = serde_json::from_slice(&body).unwrap();

        assert_eq!(error.code, "STRATEGY_NOT_FOUND");
    }

    #[tokio::test]
    async fn test_get_backtest_result_not_found() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/results/{id}", get(get_backtest_result))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/results/nonexistent-id")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_backtest_api_error_creation() {
        let error = BacktestApiError::new("TEST_ERROR", "테스트 메시지");
        assert_eq!(error.code, "TEST_ERROR");
        assert_eq!(error.message, "테스트 메시지");
    }
}
