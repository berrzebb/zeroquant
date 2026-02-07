//! 포트폴리오 관리 endpoint.
//!
//! 포트폴리오 요약, 잔고 조회, 수익률 정보를 위한 REST API를 제공합니다.
//! KIS API를 통해 실제 계좌 데이터를 조회합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/portfolio/summary` - 포트폴리오 요약
//! - `GET /api/v1/portfolio/balance` - 상세 잔고 조회
//! - `GET /api/v1/portfolio/holdings` - 보유 종목 목록
//!
//! # 쿼리 파라미터
//!
//! - `credential_id` (선택): 특정 거래소 자격증명 ID로 조회

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, warn};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::repository::{
    create_provider_for_mock_credential, EquityHistoryRepository, ExchangeProviderArc,
    HoldingPosition, PortfolioSnapshot, PositionRepository,
};
use crate::routes::strategies::ApiError;
use crate::state::AppState;
use chrono::Utc;
use trader_core::{
    ExecutionHistoryRequest, ExecutionHistoryResponse, ExecutionRecord, StrategyAccountInfo,
    StrategyPositionInfo,
};

// ==================== 캐시 키 및 TTL ====================

/// 포트폴리오 API 캐시 설정.
mod cache_keys {
    /// 계좌 정보 캐시 TTL (30초) - 계좌 잔고는 자주 변동
    pub const ACCOUNT_TTL_SECS: u64 = 30;
    /// 포지션 정보 캐시 TTL (30초)
    pub const POSITIONS_TTL_SECS: u64 = 30;
    /// 보유 종목 캐시 TTL (60초)
    pub const HOLDINGS_TTL_SECS: u64 = 60;
    /// 일반 체결 내역 캐시 TTL (5분)
    pub const ORDERS_TTL_SECS: u64 = 300;
    /// ISA 전체 체결 내역 캐시 TTL (10분) - 가장 느린 조회
    pub const ISA_ORDERS_TTL_SECS: u64 = 600;

    /// 계좌 정보 캐시 키.
    pub fn account_key(credential_id: &str) -> String {
        format!("portfolio:account:{}", credential_id)
    }

    /// 포지션 정보 캐시 키.
    pub fn positions_key(credential_id: &str) -> String {
        format!("portfolio:positions:{}", credential_id)
    }

    /// 보유 종목 캐시 키.
    pub fn holdings_key(credential_id: &str) -> String {
        format!("portfolio:holdings:{}", credential_id)
    }

    /// 체결 내역 캐시 키 (날짜 범위 포함).
    pub fn orders_key(credential_id: &str, start: &str, end: &str) -> String {
        format!("portfolio:orders:{}:{}:{}", credential_id, start, end)
    }

    /// ISA 전체 체결 내역 캐시 키.
    pub fn isa_orders_key(credential_id: &str) -> String {
        format!("portfolio:orders:isa:{}", credential_id)
    }
}

// ==================== 캐시 무효화 ====================

/// 포트폴리오 캐시 무효화.
///
/// 주문 체결/취소 시 호출하여 캐시된 포트폴리오 데이터를 무효화합니다.
/// 다음 조회 시 최신 데이터를 거래소 API에서 가져옵니다.
///
/// # 인자
///
/// * `state` - AppState (Redis 캐시 포함)
/// * `credential_id` - 무효화할 자격증명 ID
///
/// # 예시
///
/// ```ignore
/// // 주문 체결 후 캐시 무효화
/// invalidate_portfolio_cache(&state, credential_id).await;
/// ```
pub async fn invalidate_portfolio_cache(state: &AppState, credential_id: uuid::Uuid) {
    let id_str = credential_id.to_string();

    if let Some(cache) = &state.cache {
        // 계좌 정보 캐시 삭제
        if let Err(e) = cache.delete(&cache_keys::account_key(&id_str)).await {
            warn!("계좌 캐시 삭제 실패: {}", e);
        }

        // 포지션 캐시 삭제
        if let Err(e) = cache.delete(&cache_keys::positions_key(&id_str)).await {
            warn!("포지션 캐시 삭제 실패: {}", e);
        }

        // 보유 종목 캐시 삭제
        if let Err(e) = cache.delete(&cache_keys::holdings_key(&id_str)).await {
            warn!("보유종목 캐시 삭제 실패: {}", e);
        }

        debug!("포트폴리오 캐시 무효화 완료: credential_id={}", credential_id);
    }
}

/// 체결 내역 캐시 전체 무효화 (패턴 기반).
///
/// 특정 자격증명의 모든 체결 내역 캐시를 삭제합니다.
pub async fn invalidate_order_history_cache(state: &AppState, credential_id: uuid::Uuid) {
    let id_str = credential_id.to_string();

    if let Some(cache) = &state.cache {
        // ISA 체결 내역 캐시 삭제
        if let Err(e) = cache.delete(&cache_keys::isa_orders_key(&id_str)).await {
            warn!("ISA 체결 내역 캐시 삭제 실패: {}", e);
        }

        // 패턴 기반으로 일반 체결 내역 캐시 삭제
        let pattern = format!("portfolio:orders:{}:*", id_str);
        match cache.delete_pattern(&pattern).await {
            Ok(count) => {
                if count > 0 {
                    debug!(
                        "체결 내역 캐시 무효화 완료: credential_id={}, 삭제 {} 건",
                        credential_id, count
                    );
                }
            }
            Err(e) => {
                warn!("체결 내역 캐시 패턴 삭제 실패: {}", e);
            }
        }
    }
}

// ==================== 응답 타입 ====================

/// 포트폴리오 요약 응답.
///
/// Frontend의 PortfolioSummary 타입과 매칭됩니다.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioSummaryResponse {
    /// 총 자산 가치 (현금 + 평가액)
    pub total_value: Decimal,
    /// 총 손익
    pub total_pnl: Decimal,
    /// 총 수익률 (%)
    pub total_pnl_percent: Decimal,
    /// 당일 손익
    pub daily_pnl: Decimal,
    /// 당일 수익률 (%)
    pub daily_pnl_percent: Decimal,
    /// 현금 잔고
    pub cash_balance: Decimal,
    /// 사용 중인 마진/증거금
    pub margin_used: Decimal,
}

/// 상세 잔고 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BalanceResponse {
    /// 계좌 정보 (거래소 중립적)
    pub account: AccountInfo,
    /// 총 자산 가치
    pub total_value: Decimal,
}

/// 거래소 중립적 계좌 정보.
///
/// KR, US, CRYPTO 등 모든 시장에서 동일한 구조를 사용합니다.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    /// 예수금 (현금)
    pub cash_balance: Decimal,
    /// 총 평가금액
    pub total_eval_amount: Decimal,
    /// 총 평가손익
    pub total_profit_loss: Decimal,
    /// 보유 종목 수
    pub holdings_count: usize,
    /// 통화 (KRW, USD 등)
    pub currency: String,
    /// 시장 (KR, US, CRYPTO 등)
    pub market: String,
}

/// 보유 종목 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HoldingsResponse {
    /// 전체 보유 종목 (시장 구분은 각 HoldingInfo.market 필드 사용)
    pub holdings: Vec<HoldingInfo>,
    /// 총 보유 종목 수
    pub total_count: usize,
}

/// 개별 보유 종목 정보.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HoldingInfo {
    /// 종목 코드/심볼
    pub symbol: String,
    /// 표시 이름 (예: "005930(삼성전자)")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// 종목명 (KIS API에서 받아온 원본)
    pub name: String,
    /// 보유 수량
    pub quantity: Decimal,
    /// 매입 평균가
    pub avg_price: Decimal,
    /// 현재가
    pub current_price: Decimal,
    /// 평가금액
    pub eval_amount: Decimal,
    /// 평가손익
    pub profit_loss: Decimal,
    /// 수익률 (%)
    pub profit_loss_rate: Decimal,
    /// 시장 (KR/US)
    pub market: String,
}

// ==================== 쿼리 파라미터 ====================

/// 포트폴리오 API 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams)]
pub struct PortfolioQuery {
    /// 특정 자격증명 ID로 조회 (선택)
    pub credential_id: Option<Uuid>,
}

/// 체결 내역 조회 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams)]
pub struct OrderHistoryQuery {
    /// 자격증명 ID (필수)
    pub credential_id: Uuid,
    /// 조회 시작일 (YYYYMMDD, 기본: 30일 전)
    pub start_date: Option<String>,
    /// 조회 종료일 (YYYYMMDD, 기본: 오늘)
    pub end_date: Option<String>,
    /// 매수/매도 구분 ("00"=전체, "01"=매도, "02"=매수, 기본: 전체)
    pub side: Option<String>,
    /// 페이지 커서 (연속 조회용)
    pub cursor: Option<String>,
}

// ==================== 헬퍼 함수 ====================

/// 특정 credential_id로 거래소 Provider 조회 (캐시 우선) 또는 생성 (거래소 중립).
///
/// # Single Source of Truth
///
/// 이 함수는 `create_exchange_providers_from_credential()`를 통해서만 Provider를 생성합니다.
/// 토큰 재사용을 위해 AppState의 캐시를 먼저 확인합니다.
///
/// # Returns
///
/// 거래소 Provider 쌍 (KR, US)
pub async fn get_or_create_exchange_providers(
    state: &AppState,
    credential_id: Uuid,
) -> Result<ExchangeProviderArc, String> {
    // 1. Provider 캐시 확인
    {
        let cache = state.exchange_providers_cache.read().await;
        if let Some(pair) = cache.get(&credential_id) {
            debug!("거래소 Provider 캐시 히트: credential_id={}", credential_id);
            return Ok(Arc::clone(pair));
        }
    }

    // 2. 캐시 미스 - 쓰기 락으로 전환하여 생성
    debug!(
        "거래소 Provider 캐시 미스, 새로 생성: credential_id={}",
        credential_id
    );

    // Double-Check Locking: 쓰기 락 획득 후 다시 확인
    let mut cache = state.exchange_providers_cache.write().await;

    // 다른 스레드가 이미 생성했을 수 있으므로 다시 확인
    if let Some(pair) = cache.get(&credential_id) {
        debug!(
            "거래소 Provider 캐시 히트 (재확인): credential_id={}",
            credential_id
        );
        return Ok(Arc::clone(pair));
    }

    // DB 연결 확인
    let pool = state
        .db_pool
        .as_ref()
        .ok_or("데이터베이스 연결이 설정되지 않았습니다.")?;

    // exchange_id 조회하여 Mock인지 확인
    let exchange_id: String = sqlx::query_scalar(
        "SELECT exchange_id FROM exchange_credentials WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("exchange_id 조회 실패: {}", e))?
    .ok_or_else(|| "해당 credential을 찾을 수 없습니다.".to_string())?;

    // 3. 거래소별 Provider 생성
    let provider = if exchange_id == "mock" {
        // Mock 거래소는 encryptor 불필요
        let mock_provider = create_provider_for_mock_credential(pool, credential_id).await?;
        mock_provider
    } else {
        // 실제 거래소는 encryptor 필요
        let encryptor = state
            .encryptor
            .as_ref()
            .ok_or("암호화 설정이 없습니다. ENCRYPTION_MASTER_KEY를 설정하세요.")?;

        crate::repository::create_exchange_providers_from_credential(
            pool,
            encryptor,
            credential_id,
            None, // OAuth 캐시는 repository에서 관리
        )
        .await?
    };

    // 4. Provider 캐시에 저장
    cache.insert(credential_id, Arc::clone(&provider));
    debug!(
        "거래소 Provider 캐시 저장: credential_id={}, 캐시 크기={}",
        credential_id,
        cache.len()
    );

    Ok(provider)
}

// ==================== Handler ====================

/// 포트폴리오 요약 조회.
///
/// KIS API에서 실제 계좌 정보를 조회하여 반환합니다.
/// credential_id가 제공되면 해당 계정의 데이터를 조회합니다.
#[utoipa::path(
    get,
    path = "/api/v1/portfolio/summary",
    tag = "portfolio",
    params(PortfolioQuery),
    responses(
        (status = 200, description = "포트폴리오 요약 조회 성공", body = PortfolioSummaryResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn get_portfolio_summary(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PortfolioQuery>,
) -> Result<Json<PortfolioSummaryResponse>, (StatusCode, Json<ApiError>)> {
    let mut total_value = Decimal::ZERO;
    let mut total_pnl = Decimal::ZERO;
    let mut cash_balance = Decimal::ZERO;
    let mut cache_hit = false;
    let mut account_currency = "KRW".to_string(); // 계좌 통화 (동적 추출)

    // credential_id가 제공된 경우 동적으로 클라이언트 생성
    if let Some(credential_id) = params.credential_id {
        let credential_id_str = credential_id.to_string();
        let cache_key = cache_keys::account_key(&credential_id_str);

        // Redis 캐시 확인
        if let Some(cache) = &state.cache {
            match cache.get::<StrategyAccountInfo>(&cache_key).await {
                Ok(Some(cached_info)) => {
                    debug!(
                        "포트폴리오 캐시 히트: credential_id={}, total={}",
                        credential_id, cached_info.total_balance
                    );
                    cash_balance = cached_info.available_balance;
                    total_value = cached_info.total_balance;
                    total_pnl = cached_info.unrealized_pnl;
                    account_currency = cached_info.currency.clone();
                    cache_hit = true;
                }
                Ok(None) => {
                    debug!("포트폴리오 캐시 미스: {}", cache_key);
                }
                Err(e) => {
                    warn!("캐시 조회 실패 (무시하고 계속): {}", e);
                }
            }
        } else {
            debug!("Redis 캐시 비활성화됨 (REDIS_URL 환경변수 필요)");
        }

        // 캐시 미스인 경우 거래소 API 호출
        if !cache_hit {
            let cache_status = if state.cache.is_some() { "캐시 미스" } else { "캐시 비활성화" };
            debug!("포트폴리오 조회 ({}): credential_id={}", cache_status, credential_id);

            match get_or_create_exchange_providers(&state, credential_id).await {
                Ok(providers) => {
                    // 거래소 계좌 잔고 조회 (ExchangeProvider 사용)
                    match providers.fetch_account().await {
                        Ok(account_info) => {
                            debug!(
                                "Account info fetched for credential {}: total={}, available={}, currency={}",
                                credential_id,
                                account_info.total_balance,
                                account_info.available_balance,
                                account_info.currency
                            );

                            cash_balance = account_info.available_balance;
                            total_value = account_info.total_balance;
                            total_pnl = account_info.unrealized_pnl;
                            account_currency = account_info.currency.clone();

                            // Redis 캐시에 저장
                            if let Some(cache) = &state.cache {
                                if let Err(e) = cache
                                    .set_with_ttl(
                                        &cache_key,
                                        &account_info,
                                        cache_keys::ACCOUNT_TTL_SECS,
                                    )
                                    .await
                                {
                                    warn!("캐시 저장 실패 (무시하고 계속): {}", e);
                                } else {
                                    debug!(
                                        "포트폴리오 캐시 저장: {}, TTL={}초",
                                        cache_key,
                                        cache_keys::ACCOUNT_TTL_SECS
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            error!(
                                "Account fetch failed for credential {}: {:?}",
                                credential_id, e
                            );
                            return Err((
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(ApiError::new(
                                    "BALANCE_FETCH_ERROR",
                                    format!("계좌 조회 실패: {:?}", e),
                                )),
                            ));
                        }
                    }
                }
                Err(e) => {
                    error!("거래소 Provider 생성 실패: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiError::new("CLIENT_ERROR", &e)),
                    ));
                }
            }
        }
    } else {
        // credential_id가 없으면 에러 반환 (레거시 환경변수 방식 제거됨)
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "CREDENTIAL_REQUIRED",
                "credential_id가 필요합니다. 설정 > 거래소 계정에서 계정을 등록하세요.",
            )),
        ));
    }

    // 수익률 계산
    let total_pnl_percent = if total_value > Decimal::ZERO && total_value != total_pnl {
        (total_pnl / (total_value - total_pnl)) * Decimal::from(100)
    } else {
        Decimal::ZERO
    };

    // 포트폴리오 스냅샷 저장 (캐시 미스일 때만, 자산 곡선 데이터 축적)
    // 캐시 히트 시에는 이미 최근에 저장했으므로 중복 저장 방지
    if !cache_hit {
        if let (Some(db_pool), Some(credential_id)) = (&state.db_pool, params.credential_id) {
            let securities_value = total_value - cash_balance;

            // 검증: 총 자산이 0보다 크고, 합계가 맞는지 확인
            // (총 자산 0인 스냅샷은 MDD 계산을 왜곡하므로 저장하지 않음)
            let is_valid = total_value > Decimal::ZERO
                && cash_balance >= Decimal::ZERO
                && securities_value >= Decimal::ZERO
                && (cash_balance + securities_value - total_value).abs() < Decimal::ONE; // 허용 오차 1원

            if !is_valid {
                warn!(
                    "포트폴리오 스냅샷 검증 실패: total={}, cash={}, securities={}, credential_id={}",
                    total_value, cash_balance, securities_value, credential_id
                );
            } else {
                let snapshot = PortfolioSnapshot {
                    credential_id,
                    snapshot_time: Utc::now(),
                    total_equity: total_value,
                    cash_balance,
                    securities_value,
                    total_pnl,
                    daily_pnl: Decimal::ZERO, // TODO: 전일 대비 계산
                    currency: account_currency.clone(),
                    market: detect_market_from_currency(&account_currency),
                    account_type: None, // 계좌 타입은 credential에서 가져올 수 있음
                };

                // 비동기로 저장 (실패해도 API 응답에 영향 없음)
                let pool = db_pool.clone();
                tokio::spawn(async move {
                    match EquityHistoryRepository::save_snapshot(&pool, &snapshot).await {
                        Ok(_) => debug!(
                            "포트폴리오 스냅샷 저장 성공: credential_id={}",
                            credential_id
                        ),
                        Err(e) => warn!("포트폴리오 스냅샷 저장 실패: {}", e),
                    }
                });
            }
        }
    }

    Ok(Json(PortfolioSummaryResponse {
        total_value,
        total_pnl,
        total_pnl_percent,
        daily_pnl: Decimal::ZERO, // TODO: 당일 손익 계산 필요
        daily_pnl_percent: Decimal::ZERO,
        cash_balance,
        margin_used: Decimal::ZERO, // 현금 계좌는 마진 없음
    }))
}

/// 상세 잔고 조회.
#[utoipa::path(
    get,
    path = "/api/v1/portfolio/balance",
    tag = "portfolio",
    params(PortfolioQuery),
    responses(
        (status = 200, description = "잔고 조회 성공", body = BalanceResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn get_balance(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PortfolioQuery>,
) -> Result<Json<BalanceResponse>, (StatusCode, Json<ApiError>)> {
    // credential_id 필수 (레거시 환경변수 방식 제거됨)
    let credential_id = match params.credential_id {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "CREDENTIAL_REQUIRED",
                    "credential_id가 필요합니다. 설정 > 거래소 계정에서 계정을 등록하세요.",
                )),
            ));
        }
    };

    // Provider에서 계좌 정보와 포지션 조회 (캐시 포함)
    let credential_id_str = credential_id.to_string();
    let account_cache_key = cache_keys::account_key(&credential_id_str);
    let positions_cache_key = cache_keys::positions_key(&credential_id_str);

    // 캐시에서 계좌 정보와 포지션 정보 조회 시도
    let mut cached_account: Option<StrategyAccountInfo> = None;
    let mut cached_positions: Option<Vec<StrategyPositionInfo>> = None;

    if let Some(cache) = &state.cache {
        // 계좌 정보 캐시 조회
        if let Ok(Some(info)) = cache.get::<StrategyAccountInfo>(&account_cache_key).await {
            debug!("잔고 캐시 히트 (계좌): {}", account_cache_key);
            cached_account = Some(info);
        }

        // 포지션 정보 캐시 조회
        if let Ok(Some(positions)) = cache
            .get::<Vec<StrategyPositionInfo>>(&positions_cache_key)
            .await
        {
            debug!("잔고 캐시 히트 (포지션): {}", positions_cache_key);
            cached_positions = Some(positions);
        }
    }

    // 캐시에서 둘 다 조회된 경우 바로 반환
    if let (Some(account_info), Some(positions)) = (&cached_account, &cached_positions) {
        debug!(
            "잔고 캐시 히트: credential_id={}, 포지션 {} 건",
            credential_id,
            positions.len()
        );
        return Ok(Json(BalanceResponse {
            account: AccountInfo {
                cash_balance: account_info.available_balance,
                total_eval_amount: account_info.total_balance,
                total_profit_loss: account_info.unrealized_pnl,
                holdings_count: positions.len(),
                currency: account_info.currency.clone(),
                market: detect_market_from_currency(&account_info.currency),
            },
            total_value: account_info.total_balance,
        }));
    }

    // 캐시 미스: Provider에서 직접 조회
    let providers = match get_or_create_exchange_providers(&state, credential_id).await {
        Ok(p) => p,
        Err(e) => {
            error!("거래소 Provider 생성 실패: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("CLIENT_ERROR", &e)),
            ));
        }
    };

    // 계좌 정보 조회 (캐시 미스된 경우만)
    let account_info = match cached_account {
        Some(cached) => cached,
        None => match providers.fetch_account().await {
            Ok(info) => {
                // 캐시에 저장
                if let Some(cache) = &state.cache {
                    let _ = cache
                        .set_with_ttl(&account_cache_key, &info, cache_keys::ACCOUNT_TTL_SECS)
                        .await;
                }
                info
            }
            Err(e) => {
                warn!("계좌 정보 조회 실패: {:?}", e);
                return Ok(Json(BalanceResponse {
                    account: AccountInfo {
                        cash_balance: Decimal::ZERO,
                        total_eval_amount: Decimal::ZERO,
                        total_profit_loss: Decimal::ZERO,
                        holdings_count: 0,
                        currency: "UNKNOWN".to_string(),
                        market: "UNKNOWN".to_string(),
                    },
                    total_value: Decimal::ZERO,
                }));
            }
        },
    };

    // 포지션 조회 (캐시 미스된 경우만)
    let positions = match cached_positions {
        Some(cached) => cached,
        None => match providers.fetch_positions().await {
            Ok(pos) => {
                // 캐시에 저장
                if let Some(cache) = &state.cache {
                    let _ = cache
                        .set_with_ttl(&positions_cache_key, &pos, cache_keys::POSITIONS_TTL_SECS)
                        .await;
                }
                pos
            }
            Err(_) => Vec::new(),
        },
    };

    Ok(Json(BalanceResponse {
        account: AccountInfo {
            cash_balance: account_info.available_balance,
            total_eval_amount: account_info.total_balance,
            total_profit_loss: account_info.unrealized_pnl,
            holdings_count: positions.len(),
            currency: account_info.currency.clone(),
            market: detect_market_from_currency(&account_info.currency),
        },
        total_value: account_info.total_balance,
    }))
}

/// 보유 종목 목록 조회.
#[utoipa::path(
    get,
    path = "/api/v1/portfolio/holdings",
    tag = "portfolio",
    params(PortfolioQuery),
    responses(
        (status = 200, description = "보유종목 조회 성공", body = HoldingsResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn get_holdings(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PortfolioQuery>,
) -> Result<Json<HoldingsResponse>, (StatusCode, Json<ApiError>)> {
    let mut holdings: Vec<HoldingInfo> = Vec::new();
    let mut cache_hit = false;

    // credential_id가 제공된 경우 동적으로 클라이언트 생성
    if let Some(credential_id) = params.credential_id {
        let credential_id_str = credential_id.to_string();
        let cache_key = cache_keys::holdings_key(&credential_id_str);

        // Redis 캐시 확인
        if let Some(cache) = &state.cache {
            match cache
                .get::<Vec<StrategyPositionInfo>>(&cache_key)
                .await
            {
                Ok(Some(cached_positions)) => {
                    debug!(
                        "보유종목 캐시 히트: credential_id={}, {} 건",
                        credential_id,
                        cached_positions.len()
                    );
                    holdings = convert_positions_to_holdings(&cached_positions);
                    cache_hit = true;
                }
                Ok(None) => {
                    debug!("보유종목 캐시 미스: {}", cache_key);
                }
                Err(e) => {
                    warn!("캐시 조회 실패 (무시하고 계속): {}", e);
                }
            }
        }

        // 캐시 미스인 경우 거래소 API 호출
        if !cache_hit {
            debug!("보유종목 조회: credential_id={}", credential_id);

            match get_or_create_exchange_providers(&state, credential_id).await {
                Ok(providers) => {
                    // 한국 주식 보유 종목 (ExchangeProvider 사용)
                    match providers.fetch_positions().await {
                        Ok(positions) => {
                            // Redis 캐시에 저장
                            if let Some(cache) = &state.cache {
                                if let Err(e) = cache
                                    .set_with_ttl(
                                        &cache_key,
                                        &positions,
                                        cache_keys::HOLDINGS_TTL_SECS,
                                    )
                                    .await
                                {
                                    warn!("캐시 저장 실패 (무시하고 계속): {}", e);
                                } else {
                                    debug!(
                                        "보유종목 캐시 저장: {}, TTL={}초",
                                        cache_key,
                                        cache_keys::HOLDINGS_TTL_SECS
                                    );
                                }
                            }

                            holdings = convert_positions_to_holdings(&positions);
                        }
                        Err(e) => {
                            warn!("보유종목 조회 실패 (credential {}): {:?}", credential_id, e);
                        }
                    }
                }
                Err(e) => {
                    error!("거래소 Provider 생성 실패: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiError::new("CLIENT_ERROR", &e)),
                    ));
                }
            }
        }
    } else {
        // credential_id가 없으면 에러 반환 (레거시 환경변수 방식 제거됨)
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "CREDENTIAL_REQUIRED",
                "credential_id가 필요합니다. 설정 > 거래소 계정에서 계정을 등록하세요.",
            )),
        ));
    }

    let total_count = holdings.len();

    // 거래소 데이터를 positions 테이블에 동기화
    if let (Some(db_pool), Some(credential_id)) = (&state.db_pool, params.credential_id) {
        // credential에서 exchange_id 조회
        let exchange_id = sqlx::query_scalar::<_, String>(
            "SELECT exchange_id FROM exchange_credentials WHERE id = $1",
        )
        .bind(credential_id)
        .fetch_optional(db_pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "unknown".to_string());

        // 동기화할 holdings 데이터 준비
        let sync_holdings: Vec<HoldingPosition> = holdings
            .iter()
            .map(|h| HoldingPosition {
                credential_id,
                exchange: exchange_id.clone(),
                symbol: h.symbol.clone(),
                symbol_name: h.name.clone(),
                quantity: h.quantity,
                avg_price: h.avg_price,
                current_price: h.current_price,
                profit_loss: h.profit_loss,
                profit_loss_rate: h.profit_loss_rate,
                market: h.market.clone(),
            })
            .collect();

        // 비동기로 동기화 (API 응답 지연 방지)
        let pool = db_pool.clone();
        tokio::spawn(async move {
            match PositionRepository::sync_holdings(&pool, credential_id, &exchange_id, sync_holdings)
                .await
            {
                Ok(result) => {
                    debug!(
                        "포지션 동기화 완료: credential_id={}, synced={}, closed={}",
                        credential_id, result.synced, result.closed
                    );
                }
                Err(e) => {
                    warn!(
                        "포지션 동기화 실패: credential_id={}, error={}",
                        credential_id, e
                    );
                }
            }
        });
    }

    Ok(Json(HoldingsResponse {
        holdings,
        total_count,
    }))
}

/// 체결 내역 조회 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrderHistoryResponse {
    /// 체결 내역 목록
    pub records: Vec<ExecutionRecordDto>,
    /// 추가 데이터 존재 여부
    pub has_more: bool,
    /// 다음 페이지 커서
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// 총 레코드 수 (현재 페이지)
    pub count: usize,
}

/// 체결 내역 DTO (프론트엔드용 직렬화).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionRecordDto {
    /// 거래소
    pub exchange: String,
    /// 주문 ID
    pub order_id: String,
    /// 심볼
    pub symbol: String,
    /// 종목명
    pub asset_name: String,
    /// 매수/매도
    pub side: String,
    /// 주문 유형
    pub order_type: String,
    /// 주문 수량
    pub order_qty: Decimal,
    /// 주문 가격
    pub order_price: Decimal,
    /// 체결 수량
    pub filled_qty: Decimal,
    /// 체결 평균가
    pub filled_price: Decimal,
    /// 체결 금액
    pub filled_amount: Decimal,
    /// 상태
    pub status: String,
    /// 취소 여부
    pub is_cancelled: bool,
    /// 주문 일시 (ISO 8601)
    pub ordered_at: String,
}

impl From<&ExecutionRecord> for ExecutionRecordDto {
    fn from(record: &ExecutionRecord) -> Self {
        Self {
            exchange: record.exchange.clone(),
            order_id: record.order_id.clone(),
            symbol: record.symbol.to_string(),
            asset_name: record.asset_name.clone(),
            side: format!("{:?}", record.side),
            order_type: record.order_type.clone(),
            order_qty: record.order_qty,
            order_price: record.order_price,
            filled_qty: record.filled_qty,
            filled_price: record.filled_price,
            filled_amount: record.filled_amount,
            status: format!("{:?}", record.status),
            is_cancelled: record.is_cancelled,
            ordered_at: record.ordered_at.to_rfc3339(),
        }
    }
}

/// 체결 내역 조회.
///
/// 거래소 중립적인 ExecutionHistory를 반환합니다.
#[utoipa::path(
    get,
    path = "/api/v1/portfolio/orders",
    tag = "portfolio",
    params(OrderHistoryQuery),
    responses(
        (status = 200, description = "체결 내역 조회 성공", body = OrderHistoryResponse),
        (status = 500, description = "서버 오류", body = ApiError)
    )
)]
pub async fn get_order_history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<OrderHistoryQuery>,
) -> Result<Json<OrderHistoryResponse>, (StatusCode, Json<ApiError>)> {
    // 기본 날짜 설정 (30일 전 ~ 오늘)
    let today = chrono::Utc::now() + chrono::Duration::hours(9); // KST
    let default_start = (today - chrono::Duration::days(30))
        .format("%Y%m%d")
        .to_string();
    let default_end = today.format("%Y%m%d").to_string();

    let start_date = params.start_date.unwrap_or(default_start);
    let end_date = params.end_date.unwrap_or(default_end);
    let side = params.side.unwrap_or_else(|| "00".to_string());
    let credential_id_str = params.credential_id.to_string();

    // 커서가 있으면 캐시를 사용하지 않음 (페이지네이션 중)
    let use_cache = params.cursor.is_none();

    // 캐시 키 생성 (ISA 계좌는 전체 기간 조회이므로 별도 키 사용)
    // 10년치 조회면 ISA 계좌로 간주
    let is_isa = {
        let start_year: i32 = start_date[0..4].parse().unwrap_or(0);
        let end_year: i32 = end_date[0..4].parse().unwrap_or(0);
        end_year - start_year >= 5 // 5년 이상 조회면 ISA로 간주
    };
    let cache_key = if is_isa {
        cache_keys::isa_orders_key(&credential_id_str)
    } else {
        cache_keys::orders_key(&credential_id_str, &start_date, &end_date)
    };
    let cache_ttl = if is_isa {
        cache_keys::ISA_ORDERS_TTL_SECS
    } else {
        cache_keys::ORDERS_TTL_SECS
    };

    // Redis 캐시 확인
    if use_cache {
        if let Some(cache) = &state.cache {
            match cache.get::<ExecutionHistoryResponse>(&cache_key).await {
                Ok(Some(cached_response)) => {
                    let count = cached_response.trades.len();
                    debug!(
                        "체결 내역 캐시 히트: credential_id={}, {} 건",
                        params.credential_id, count
                    );

                    // 캐시된 Trade를 DTO로 변환
                    let records = convert_trades_to_dto(&cached_response.trades);
                    return Ok(Json(OrderHistoryResponse {
                        records,
                        has_more: cached_response.next_cursor.is_some(),
                        next_cursor: cached_response.next_cursor,
                        count,
                    }));
                }
                Ok(None) => {
                    debug!("체결 내역 캐시 미스: {}", cache_key);
                }
                Err(e) => {
                    warn!("캐시 조회 실패 (무시하고 계속): {}", e);
                }
            }
        } else {
            debug!("Redis 캐시 비활성화됨 (REDIS_URL 환경변수 필요)");
        }
    }

    let cache_status = if state.cache.is_some() { "캐시 미스" } else { "캐시 비활성화" };
    debug!(
        "체결 내역 조회 ({}): credential_id={}, 기간={}~{}",
        cache_status, params.credential_id, start_date, end_date
    );

    // 커서 파싱 (format: "ctx_fk100|ctx_nk100")
    let (ctx_fk100, ctx_nk100) = if let Some(cursor) = &params.cursor {
        let parts: Vec<&str> = cursor.split('|').collect();
        if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (String::new(), String::new())
        }
    } else {
        (String::new(), String::new())
    };

    // ExchangeProvider 획득
    let providers = get_or_create_exchange_providers(&state, params.credential_id)
        .await
        .map_err(|e| {
            error!("거래소 Provider 생성 실패: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("CLIENT_ERROR", &e)),
            )
        })?;

    // 체결 내역 조회 (ExchangeProvider 사용)
    let mut request = ExecutionHistoryRequest::new(&start_date, &end_date).with_side(&side);
    if !ctx_fk100.is_empty() && !ctx_nk100.is_empty() {
        request = request.with_cursor(format!("{}|{}", ctx_fk100, ctx_nk100));
    }

    let history_response = providers
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

    // Redis 캐시에 저장 (첫 페이지만, 페이지네이션 중에는 저장 안 함)
    if use_cache {
        if let Some(cache) = &state.cache {
            if let Err(e) = cache
                .set_with_ttl(&cache_key, &history_response, cache_ttl)
                .await
            {
                warn!("캐시 저장 실패 (무시하고 계속): {}", e);
            } else {
                debug!(
                    "체결 내역 캐시 저장: {}, TTL={}초",
                    cache_key, cache_ttl
                );
            }
        }
    }

    // Trade를 ExecutionRecordDto로 변환
    let records = convert_trades_to_dto(&history_response.trades);
    let count = records.len();
    let has_more = history_response.next_cursor.is_some();

    debug!("체결 내역 조회 완료: {} 건, has_more={}", count, has_more);

    Ok(Json(OrderHistoryResponse {
        records,
        has_more,
        next_cursor: history_response.next_cursor,
        count,
    }))
}

/// Trade 목록을 ExecutionRecordDto로 변환하는 헬퍼 함수.
fn convert_trades_to_dto(trades: &[trader_core::Trade]) -> Vec<ExecutionRecordDto> {
    trades
        .iter()
        .map(|trade| ExecutionRecordDto {
            exchange: trade.exchange.clone(),
            order_id: trade.exchange_trade_id.clone(),
            symbol: trade.ticker.clone(),
            asset_name: trade
                .metadata
                .get("stock_name")
                .and_then(|v| v.as_str())
                .unwrap_or(&trade.ticker)
                .to_string(),
            side: match trade.side {
                trader_core::Side::Buy => "BUY".to_string(),
                trader_core::Side::Sell => "SELL".to_string(),
            },
            order_type: "LIMIT".to_string(),
            order_qty: trade.quantity,
            order_price: trade.price,
            filled_qty: trade.quantity,
            filled_price: trade.price,
            filled_amount: trade.quantity * trade.price,
            status: "FILLED".to_string(),
            is_cancelled: false,
            ordered_at: trade.executed_at.to_rfc3339(),
        })
        .collect()
}

/// 통화 코드로부터 시장 구분을 추론하는 헬퍼 함수.
fn detect_market_from_currency(currency: &str) -> String {
    match currency {
        "KRW" => "KR".to_string(),
        "USD" => "US".to_string(),
        "USDT" | "BTC" | "ETH" => "CRYPTO".to_string(),
        _ => "UNKNOWN".to_string(),
    }
}

/// ticker 패턴으로부터 시장 구분을 추론하는 헬퍼 함수.
///
/// - 숫자로만 구성: KR (한국 주식 코드, 예: "005930")
/// - '/' 포함: CRYPTO (예: "BTC/USDT")
/// - 그 외: US (알파벳 심볼, 예: "AAPL")
fn detect_market_from_ticker(ticker: &str) -> String {
    if ticker.chars().all(|c| c.is_ascii_digit()) {
        "KR".to_string()
    } else if ticker.contains('/') {
        "CRYPTO".to_string()
    } else {
        "US".to_string()
    }
}

/// StrategyPositionInfo를 HoldingInfo로 변환하는 헬퍼 함수.
fn convert_positions_to_holdings(positions: &[StrategyPositionInfo]) -> Vec<HoldingInfo> {
    positions
        .iter()
        .map(|position| {
            let symbol_str = position.ticker.clone();
            let eval_amount = position.quantity * position.current_price;
            let profit_loss_rate = if position.avg_entry_price > Decimal::ZERO {
                ((position.current_price - position.avg_entry_price) / position.avg_entry_price)
                    * Decimal::from(100)
            } else {
                Decimal::ZERO
            };

            HoldingInfo {
                symbol: symbol_str.clone(),
                display_name: Some(symbol_str.clone()),
                name: symbol_str.clone(),
                quantity: position.quantity,
                avg_price: position.avg_entry_price,
                current_price: position.current_price,
                eval_amount,
                profit_loss: position.unrealized_pnl,
                profit_loss_rate,
                market: detect_market_from_ticker(&symbol_str),
            }
        })
        .collect()
}

// ==================== 라우터 ====================

/// 포트폴리오 관리 라우터 생성.
pub fn portfolio_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/summary", get(get_portfolio_summary))
        .route("/balance", get(get_balance))
        .route("/holdings", get(get_holdings))
        .route("/orders", get(get_order_history))
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
    async fn test_get_portfolio_summary_mock() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/portfolio/summary", get(get_portfolio_summary))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/portfolio/summary")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let summary: PortfolioSummaryResponse = serde_json::from_slice(&body).unwrap();

        // Mock 데이터 확인
        assert!(summary.total_value > Decimal::ZERO);
    }

    #[tokio::test]
    async fn test_get_holdings_empty() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/portfolio/holdings", get(get_holdings))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/portfolio/holdings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let holdings: HoldingsResponse = serde_json::from_slice(&body).unwrap();

        // KIS 클라이언트 미설정 시 빈 목록
        assert_eq!(holdings.total_count, 0);
    }
}
