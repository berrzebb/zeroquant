//! 자산 곡선 동기화 핸들러.
//!
//! 거래소 API에서 체결 내역을 가져와 자산 곡선 데이터를 재구성합니다.

use axum::{extract::State, response::IntoResponse, Json};
use chrono::{DateTime, NaiveDate, Utc};
use std::sync::Arc;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::repository::{
    create_kis_provider_for_sync, create_provider_for_mock_credential, get_credential_info,
    EquityHistoryRepository, ExecutionCacheRepository, ExecutionForSync, NewExecution,
};
use crate::state::AppState;
use trader_core::{ExecutionHistoryRequest, Side};

use super::types::{SyncEquityCurveRequest, SyncEquityCurveResponse};

/// 거래소 체결 내역으로 자산 곡선 동기화.
///
/// KIS API에서 체결 내역을 가져와 자산 곡선 데이터를 재구성합니다.
#[utoipa::path(
    post,
    path = "/api/v1/analytics/sync-equity",
    tag = "analytics",
    request_body = SyncEquityCurveRequest,
    responses(
        (status = 200, description = "동기화 성공", body = SyncEquityCurveResponse),
        (status = 400, description = "잘못된 요청"),
        (status = 500, description = "서버 오류")
    )
)]
pub async fn sync_equity_curve(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SyncEquityCurveRequest>,
) -> impl IntoResponse {
    // 1. credential_id 파싱
    let credential_id = match Uuid::parse_str(&request.credential_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date,
                    end_date: request.end_date,
                    message: "Invalid credential_id format".to_string(),
                }),
            );
        }
    };

    // 1.5 DB 연결 확인 (exchange_id 조회에 필요)
    let pool = match state.db_pool.as_ref() {
        Some(p) => p,
        None => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: "DB pool이 없습니다".to_string(),
                }),
            );
        }
    };

    // 2. exchange_id 조회
    let exchange_id: String = match sqlx::query_scalar(
        "SELECT exchange_id FROM exchange_credentials WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(id)) => id,
        Ok(None) => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: "Credential을 찾을 수 없습니다".to_string(),
                }),
            );
        }
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: format!("exchange_id 조회 실패: {}", e),
                }),
            );
        }
    };

    debug!("Syncing equity curve for exchange: {}", exchange_id);

    // 3. Mock 거래소 처리 (별도 분기)
    if exchange_id == "mock" {
        return sync_equity_curve_mock(pool, credential_id, &request).await;
    }

    // 4. KIS 거래소 처리: encryptor 확인
    let encryptor = match state.encryptor.as_ref() {
        Some(e) => e,
        None => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: "Encryptor가 없습니다".to_string(),
                }),
            );
        }
    };

    // 5. KIS Provider 생성 (통합 Provider 사용)
    let kis_provider =
        match create_kis_provider_for_sync(pool, encryptor, credential_id).await {
            Ok(provider) => provider,
            Err(e) => {
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(SyncEquityCurveResponse {
                        success: false,
                        synced_count: 0,
                        execution_count: 0,
                        start_date: request.start_date.clone(),
                        end_date: request.end_date.clone(),
                        message: format!("KIS Provider 생성 실패: {}", e),
                    }),
                );
            }
        };

    // 4. Credential 정보 조회 (Repository 사용)
    let cred_info = match get_credential_info(pool, credential_id).await {
        Ok(Some(info)) => info,
        Ok(None) => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: "Credential을 찾을 수 없습니다".to_string(),
                }),
            );
        }
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: format!("Credential 조회 실패: {}", e),
                }),
            );
        }
    };

    // 5. 캐시 확인 및 조회 범위 결정
    let exchange_name = exchange_id.as_str();
    let is_isa_account = cred_info.is_isa_account;

    // 요청된 날짜 파싱
    let requested_start = NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d")
        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
    let requested_end = NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Utc::now().date_naive());

    // DB에서 마지막 캐시 일자 확인
    let (actual_start, cached_executions) = if let Some(pool) = &state.db_pool {
        match ExecutionCacheRepository::get_latest_cached_date(pool, credential_id, exchange_name)
            .await
        {
            Ok(Some(latest_date)) => {
                // 캐시가 있으면 그 다음날부터 조회
                let new_start = latest_date + chrono::Duration::days(1);
                debug!(
                    "Cache found: latest_date={}, querying from {}",
                    latest_date, new_start
                );

                // 기존 캐시 데이터 조회
                let cached = ExecutionCacheRepository::get_all_executions(
                    pool,
                    credential_id,
                    exchange_name,
                )
                .await
                .unwrap_or_default();
                (new_start, cached)
            }
            Ok(None) => {
                debug!(
                    "No cache found, querying from requested start: {}",
                    requested_start
                );
                (requested_start, Vec::new())
            }
            Err(e) => {
                warn!("Failed to check cache: {}, querying full range", e);
                (requested_start, Vec::new())
            }
        }
    } else {
        (requested_start, Vec::new())
    };

    // 캐시된 데이터를 ExecutionForSync로 변환
    let mut all_executions: Vec<ExecutionForSync> = cached_executions
        .iter()
        .map(|c| ExecutionForSync {
            execution_time: c.executed_at,
            amount: c.amount,
            is_buy: c.side == Side::Buy,
            symbol: c.symbol.clone(),
        })
        .collect();

    debug!("Starting with {} cached executions", all_executions.len());

    // 이미 최신 데이터가 있으면 API 호출 스킵
    if actual_start > requested_end {
        debug!("Cache is up to date, skipping API call");
    } else {
        // 날짜 형식 변환 (ISO 8601 -> YYYYMMDD)
        let start_date_yyyymmdd = actual_start.format("%Y%m%d").to_string();
        let end_date_yyyymmdd = requested_end.format("%Y%m%d").to_string();
        debug!(
            "Date range for API: {} ~ {}",
            start_date_yyyymmdd, end_date_yyyymmdd
        );

        // KisProvider를 통한 체결 내역 조회 (ISA/일반 계좌 처리, 날짜 분할, 페이지네이션 모두 내부 처리)
        let executions = match kis_provider
            .fetch_execution_history_for_sync(&start_date_yyyymmdd, &end_date_yyyymmdd, cred_info.is_testnet)
            .await
        {
            Ok(execs) => execs,
            Err(e) => {
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(SyncEquityCurveResponse {
                        success: false,
                        synced_count: 0,
                        execution_count: all_executions.len(),
                        start_date: request.start_date,
                        end_date: request.end_date,
                        message: format!("Failed to fetch execution history: {:?}", e),
                    }),
                );
            }
        };

        debug!(
            "Fetched {} executions from KisProvider ({} account)",
            executions.len(),
            if is_isa_account { "ISA" } else { "general" }
        );

        // 체결 내역 변환 (OrderExecution -> ExecutionForSync, NewExecution)
        let mut new_executions_for_cache: Vec<NewExecution> = Vec::new();

        for exec in executions {
            // 체결 시간 파싱 (order_date: YYYYMMDD, order_time: HHMMSS)
            let exec_date = format!("{}{}", exec.order_date, exec.order_time);
            let execution_time =
                chrono::NaiveDateTime::parse_from_str(&exec_date, "%Y%m%d%H%M%S")
                    .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
                    .unwrap_or_else(|_| Utc::now());

            let amount = exec.filled_amount; // 총 체결 금액
            let is_buy = exec.side == "buy";
            let side = if is_buy { Side::Buy } else { Side::Sell };

            // 동기화용 데이터 추가
            all_executions.push(ExecutionForSync {
                execution_time,
                amount,
                is_buy,
                symbol: exec.symbol.clone(),
            });

            // 캐시용 데이터 추가
            new_executions_for_cache.push(NewExecution {
                credential_id,
                exchange: exchange_name.to_string(),
                executed_at: execution_time,
                symbol: exec.symbol.clone(),
                normalized_symbol: Some(format!("{}.KS", exec.symbol)),
                side,
                quantity: exec.filled_qty,
                price: exec.avg_price, // 체결평균가
                amount,
                fee: None,
                fee_currency: Some("KRW".to_string()),
                order_id: exec.order_no.clone(),
                trade_id: None,
                order_type: None,
                raw_data: None,
            });
        }

        // 새로 조회한 체결 내역을 캐시에 저장
        if !new_executions_for_cache.is_empty() {
            if let Some(pool) = &state.db_pool {
                debug!(
                    "Saving {} new executions to cache",
                    new_executions_for_cache.len()
                );

                match ExecutionCacheRepository::upsert_executions(pool, &new_executions_for_cache)
                    .await
                {
                    Ok(count) => {
                        debug!("Successfully cached {} executions", count);

                        // 캐시 메타데이터 업데이트
                        let earliest = new_executions_for_cache
                            .iter()
                            .map(|e| e.executed_at.date_naive())
                            .min();
                        let latest = new_executions_for_cache
                            .iter()
                            .map(|e| e.executed_at.date_naive())
                            .max();

                        if let (Some(earliest_date), Some(latest_date)) = (earliest, latest) {
                            if let Err(e) = ExecutionCacheRepository::update_cache_meta(
                                pool,
                                credential_id,
                                exchange_name,
                                Some(earliest_date),
                                Some(latest_date),
                                "success",
                                None,
                            )
                            .await
                            {
                                warn!("Failed to update cache meta: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to cache executions: {}", e);
                        // 캐시 실패해도 동기화는 계속 진행
                    }
                }
            }
        }
    } // end of else block (API 호출 필요한 경우)

    let execution_count = all_executions.len();

    // 4. 현재 잔고 조회 (KisProvider 사용)
    let (current_equity, current_cash) = match kis_provider.get_balance_for_sync().await {
        Ok(account) => {
            // StrategyAccountInfo에서 총 잔고와 가용 잔고 추출
            (account.total_balance, account.available_balance)
        }
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count,
                    start_date: request.start_date,
                    end_date: request.end_date,
                    message: format!("Failed to fetch balance: {}", e),
                }),
            );
        }
    };

    tracing::debug!(
        "Current balance - equity: {}, cash: {}",
        current_equity,
        current_cash
    );

    // 5. DB에 자산 곡선 저장
    if let Some(pool) = &state.db_pool {
        // 종가 기반 계산 vs 현금 흐름 기반 계산
        if request.use_market_prices {
            // 현재 실제 현금 잔고를 기준으로 과거 자산 역산
            // (initial_capital 지정 시 해당 값을 현재 현금으로 사용 - 테스트용)
            let cash_for_sync = request.initial_capital.unwrap_or(current_cash);

            tracing::debug!(
                "Using market prices for equity calculation (current_cash: {})",
                cash_for_sync
            );

            match EquityHistoryRepository::sync_with_market_prices(
                pool,
                credential_id,
                cash_for_sync, // 현재 실제 현금 잔고
                "KRW",
                "KR",
                Some("real"),
            )
            .await
            {
                Ok(synced_count) => {
                    return (
                        axum::http::StatusCode::OK,
                        Json(SyncEquityCurveResponse {
                            success: true,
                            synced_count,
                            execution_count,
                            start_date: request.start_date,
                            end_date: request.end_date,
                            message: format!(
                                "Successfully synced {} equity points with market prices from {} executions",
                                synced_count, execution_count
                            ),
                        }),
                    );
                }
                Err(e) => {
                    return (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        Json(SyncEquityCurveResponse {
                            success: false,
                            synced_count: 0,
                            execution_count,
                            start_date: request.start_date,
                            end_date: request.end_date,
                            message: format!(
                                "Failed to save equity curve with market prices: {}",
                                e
                            ),
                        }),
                    );
                }
            }
        } else {
            // 기존 현금 흐름 기반 계산
            match EquityHistoryRepository::sync_from_executions(
                pool,
                credential_id,
                all_executions,
                current_equity,
                "KRW",
                "KR",
                Some("real"),
            )
            .await
            {
                Ok(synced_count) => {
                    return (
                        axum::http::StatusCode::OK,
                        Json(SyncEquityCurveResponse {
                            success: true,
                            synced_count,
                            execution_count,
                            start_date: request.start_date,
                            end_date: request.end_date,
                            message: format!(
                                "Successfully synced {} equity points from {} executions",
                                synced_count, execution_count
                            ),
                        }),
                    );
                }
                Err(e) => {
                    return (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        Json(SyncEquityCurveResponse {
                            success: false,
                            synced_count: 0,
                            execution_count,
                            start_date: request.start_date,
                            end_date: request.end_date,
                            message: format!("Failed to save equity curve: {}", e),
                        }),
                    );
                }
            }
        }
    }

    (
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        Json(SyncEquityCurveResponse {
            success: false,
            synced_count: 0,
            execution_count,
            start_date: request.start_date,
            end_date: request.end_date,
            message: "Database not available".to_string(),
        }),
    )
}

/// 자산 곡선 캐시 삭제 요청.
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct ClearEquityCacheRequest {
    /// 자격증명 ID
    pub credential_id: String,
}

/// 자산 곡선 캐시 삭제 응답.
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ClearEquityCacheResponse {
    pub success: bool,
    pub deleted_count: u64,
    pub message: String,
}

/// 자산 곡선 캐시 삭제.
///
/// 특정 credential의 자산 곡선 데이터를 삭제합니다.
#[utoipa::path(
    delete,
    path = "/api/v1/analytics/equity-cache",
    tag = "analytics",
    request_body = ClearEquityCacheRequest,
    responses(
        (status = 200, description = "캐시 삭제 성공", body = ClearEquityCacheResponse),
        (status = 400, description = "잘못된 요청"),
        (status = 500, description = "서버 오류")
    )
)]
pub async fn clear_equity_cache(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ClearEquityCacheRequest>,
) -> impl IntoResponse {
    // 1. credential_id 파싱
    let credential_id = match Uuid::parse_str(&request.credential_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(ClearEquityCacheResponse {
                    success: false,
                    deleted_count: 0,
                    message: "Invalid credential_id format".to_string(),
                }),
            );
        }
    };

    // 2. DB 연결 확인
    let pool = match state.db_pool.as_ref() {
        Some(p) => p,
        None => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(ClearEquityCacheResponse {
                    success: false,
                    deleted_count: 0,
                    message: "DB pool이 없습니다".to_string(),
                }),
            );
        }
    };

    // 3. 캐시 삭제
    match EquityHistoryRepository::clear_cache(pool, credential_id).await {
        Ok(deleted_count) => {
            debug!(
                "자산 곡선 캐시 삭제 완료: credential={}, 삭제={}건",
                credential_id, deleted_count
            );
            (
                axum::http::StatusCode::OK,
                Json(ClearEquityCacheResponse {
                    success: true,
                    deleted_count,
                    message: format!("{}건의 자산 곡선 데이터가 삭제되었습니다.", deleted_count),
                }),
            )
        }
        Err(e) => {
            warn!("자산 곡선 캐시 삭제 실패: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(ClearEquityCacheResponse {
                    success: false,
                    deleted_count: 0,
                    message: format!("캐시 삭제 실패: {}", e),
                }),
            )
        }
    }
}

/// Mock 거래소용 자산 곡선 동기화.
///
/// ExchangeProvider 인터페이스를 사용하여 체결 내역과 잔고를 조회합니다.
async fn sync_equity_curve_mock(
    pool: &sqlx::PgPool,
    credential_id: Uuid,
    request: &SyncEquityCurveRequest,
) -> (axum::http::StatusCode, Json<SyncEquityCurveResponse>) {
    debug!("Mock 거래소 자산 곡선 동기화 시작");

    // 1. Mock Provider 생성
    let provider = match create_provider_for_mock_credential(pool, credential_id).await {
        Ok(p) => p,
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: format!("Mock Provider 생성 실패: {}", e),
                }),
            );
        }
    };

    // 2. 체결 내역 조회 (ExchangeProvider 인터페이스)
    let history_request = ExecutionHistoryRequest {
        start_date: request.start_date.clone(),
        end_date: request.end_date.clone(),
        cursor: None,
        side: None,
    };

    let executions = match provider.fetch_execution_history(&history_request).await {
        Ok(response) => response.trades,
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: 0,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: format!("체결 내역 조회 실패: {:?}", e),
                }),
            );
        }
    };

    debug!("Mock 체결 내역 조회 완료: {} 건", executions.len());

    // 3. 잔고 조회 (ExchangeProvider 인터페이스)
    let account = match provider.fetch_account().await {
        Ok(acc) => acc,
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count: executions.len(),
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: format!("계정 정보 조회 실패: {:?}", e),
                }),
            );
        }
    };

    let current_equity = account.total_balance;
    debug!("Mock 현재 자산: {}", current_equity);

    // 4. 체결 내역을 ExecutionForSync 형식으로 변환
    let all_executions: Vec<ExecutionForSync> = executions
        .iter()
        .map(|trade| ExecutionForSync {
            execution_time: trade.executed_at,
            amount: trade.price * trade.quantity,
            is_buy: trade.side == Side::Buy,
            symbol: trade.ticker.clone(),
        })
        .collect();

    let execution_count = all_executions.len();

    // 5. 자산 곡선 저장
    if request.use_market_prices {
        // 시장가 기반 계산
        let cash_for_sync = request.initial_capital.unwrap_or(account.available_balance);

        match EquityHistoryRepository::sync_with_market_prices(
            pool,
            credential_id,
            cash_for_sync,
            "KRW",
            "KR",
            Some("mock"),
        )
        .await
        {
            Ok(synced_count) => (
                axum::http::StatusCode::OK,
                Json(SyncEquityCurveResponse {
                    success: true,
                    synced_count,
                    execution_count,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: format!(
                        "Mock: {} equity points synced with market prices from {} executions",
                        synced_count, execution_count
                    ),
                }),
            ),
            Err(e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: format!("자산 곡선 저장 실패: {}", e),
                }),
            ),
        }
    } else {
        // 현금 흐름 기반 계산
        match EquityHistoryRepository::sync_from_executions(
            pool,
            credential_id,
            all_executions,
            current_equity,
            "KRW",
            "KR",
            Some("mock"),
        )
        .await
        {
            Ok(synced_count) => (
                axum::http::StatusCode::OK,
                Json(SyncEquityCurveResponse {
                    success: true,
                    synced_count,
                    execution_count,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: format!(
                        "Mock: {} equity points synced from {} executions",
                        synced_count, execution_count
                    ),
                }),
            ),
            Err(e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncEquityCurveResponse {
                    success: false,
                    synced_count: 0,
                    execution_count,
                    start_date: request.start_date.clone(),
                    end_date: request.end_date.clone(),
                    message: format!("자산 곡선 저장 실패: {}", e),
                }),
            ),
        }
    }
}
