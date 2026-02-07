//! 종목 스크리닝 API 라우트
//!
//! Fundamental + OHLCV 데이터 기반 종목 필터링 API를 제공합니다.
//!
//! # 엔드포인트
//!
//! - `POST /api/v1/screening` - 커스텀 스크리닝 실행
//! - `GET /api/v1/screening/presets` - 사용 가능한 프리셋 목록
//! - `GET /api/v1/screening/presets/{preset}` - 프리셋 스크리닝 실행
//! - `GET /api/v1/screening/momentum` - 모멘텀 기반 스크리닝

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info, warn};
use ts_rs::TS;
use utoipa::ToSchema;

use trader_core::MacroEnvironment;
use trader_data::cache::MacroData;
use trader_data::RedisCache;

use crate::repository::{
    MomentumScreenResult, ScreeningFilter, ScreeningPreset, ScreeningRepository, ScreeningResult,
};
use crate::state::AppState;

// ==================== Request/Response 타입 ====================

/// 커스텀 스크리닝 요청
#[derive(Debug, Clone, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "screening/")]
pub struct ScreeningRequest {
    /// 시장 필터 (KR, US, CRYPTO)
    #[serde(default)]
    pub market: Option<String>,
    /// 거래소 필터
    #[serde(default)]
    pub exchange: Option<String>,
    /// 섹터 필터
    #[serde(default)]
    pub sector: Option<String>,

    // 시가총액 필터
    #[serde(default)]
    pub min_market_cap: Option<String>,
    #[serde(default)]
    pub max_market_cap: Option<String>,

    // 밸류에이션 필터
    #[serde(default)]
    pub min_per: Option<String>,
    #[serde(default)]
    pub max_per: Option<String>,
    #[serde(default)]
    pub min_pbr: Option<String>,
    #[serde(default)]
    pub max_pbr: Option<String>,

    // 수익성 필터
    #[serde(default)]
    pub min_roe: Option<String>,
    #[serde(default)]
    pub max_roe: Option<String>,
    #[serde(default)]
    pub min_roa: Option<String>,
    #[serde(default)]
    pub max_roa: Option<String>,

    // 배당 필터
    #[serde(default)]
    pub min_dividend_yield: Option<String>,
    #[serde(default)]
    pub max_dividend_yield: Option<String>,

    // 안정성 필터
    #[serde(default)]
    pub max_debt_ratio: Option<String>,

    // 성장성 필터
    #[serde(default)]
    pub min_revenue_growth: Option<String>,
    #[serde(default)]
    pub min_earnings_growth: Option<String>,

    // 52주 고저가 필터
    #[serde(default)]
    pub max_distance_from_52w_high: Option<String>,
    #[serde(default)]
    pub min_distance_from_52w_low: Option<String>,

    // 거래량 필터
    #[serde(default)]
    pub min_volume_ratio: Option<String>,

    // 구조적 피처 필터
    #[serde(default)]
    pub min_low_trend: Option<String>,
    #[serde(default)]
    pub min_vol_quality: Option<String>,
    #[serde(default)]
    pub min_breakout_score: Option<String>,
    #[serde(default)]
    pub only_alive_consolidation: Option<bool>,

    // RouteState 필터
    #[serde(default)]
    pub filter_route_state: Option<String>,

    // TTM Squeeze 필터
    #[serde(default)]
    pub filter_ttm_squeeze: Option<bool>,
    #[serde(default)]
    pub min_ttm_squeeze_cnt: Option<String>,

    // 정렬 및 페이지네이션
    #[serde(default)]
    pub sort_by: Option<String>,
    #[serde(default)]
    pub sort_order: Option<String>,
    #[serde(default)]
    pub limit: Option<i32>,
    #[serde(default)]
    pub offset: Option<i32>,
}

/// 스크리닝 결과 응답
#[derive(Debug, Clone, Serialize, ToSchema, TS)]
#[ts(export, export_to = "screening/")]
pub struct ScreeningResponse {
    /// 총 결과 수
    pub total: usize,
    /// 결과 목록
    pub results: Vec<ScreeningResultDto>,
    /// 적용된 필터 요약
    pub filter_summary: String,
    /// 매크로 위험도 (옵셔널)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub macro_risk: Option<String>,
}

/// 스크리닝 결과 DTO
#[derive(Debug, Clone, Serialize, ToSchema, TS)]
#[ts(export, export_to = "screening/")]
pub struct ScreeningResultDto {
    pub ticker: String,
    pub name: String,
    pub market: String,
    pub exchange: Option<String>,
    pub sector: Option<String>,

    // Fundamental
    pub market_cap: Option<String>,
    pub per: Option<String>,
    pub pbr: Option<String>,
    pub roe: Option<String>,
    pub roa: Option<String>,
    pub eps: Option<String>,
    pub dividend_yield: Option<String>,
    pub operating_margin: Option<String>,
    pub debt_ratio: Option<String>,
    pub revenue_growth_yoy: Option<String>,
    pub earnings_growth_yoy: Option<String>,

    // 가격 정보
    pub current_price: Option<String>,
    pub week_52_high: Option<String>,
    pub week_52_low: Option<String>,
    pub distance_from_52w_high: Option<String>,
    pub distance_from_52w_low: Option<String>,

    // 구조적 피처
    pub low_trend: Option<f64>,
    pub vol_quality: Option<f64>,
    pub range_pos: Option<f64>,
    pub dist_ma20: Option<f64>,
    pub bb_width: Option<f64>,
    pub rsi_14: Option<f64>,
    pub breakout_score: Option<f64>,

    // MACD 지표
    /// MACD 라인 값
    pub macd: Option<f64>,
    /// 시그널 라인 값
    pub macd_signal: Option<f64>,
    /// 히스토그램 (MACD - Signal)
    pub macd_histogram: Option<f64>,
    /// 크로스 상태 ("golden" = 골든크로스, "dead" = 데드크로스, null = 없음)
    pub macd_cross: Option<String>,

    // RouteState
    pub route_state: Option<String>,

    // MarketRegime
    pub regime: Option<String>,

    // Sector RS (섹터 상대강도)
    pub sector_rs: Option<String>,
    pub sector_rank: Option<i32>,

    // TTM Squeeze (에너지 응축 지표)
    pub ttm_squeeze: Option<bool>,
    pub ttm_squeeze_cnt: Option<i32>,

    // TRIGGER (진입 트리거)
    pub trigger_score: Option<f64>,
    pub trigger_label: Option<String>,

    // GlobalScore (종합 점수)
    pub overall_score: Option<String>,
    pub grade: Option<String>,
    pub confidence: Option<String>,
}

/// 프리셋 목록 응답
#[derive(Debug, Clone, Serialize, ToSchema, TS)]
#[ts(export, export_to = "screening/")]
pub struct PresetsListResponse {
    #[ts(type = "Array<Record<string, unknown>>")]
    pub presets: Vec<ScreeningPreset>,
}

/// DB 프리셋 목록 응답
#[derive(Debug, Clone, Serialize, ToSchema, TS)]
#[ts(export, export_to = "screening/")]
pub struct PresetsListResponseV2 {
    #[ts(type = "Array<Record<string, unknown>>")]
    pub presets: Vec<crate::repository::ScreeningPresetRecord>,
    pub total: usize,
}

/// 프리셋 저장 응답
#[derive(Debug, Clone, Serialize, ToSchema, TS)]
#[ts(export, export_to = "screening/")]
pub struct SavePresetResponse {
    pub success: bool,
    #[ts(type = "Record<string, unknown>")]
    pub preset: crate::repository::ScreeningPresetRecord,
    pub message: String,
}

/// 프리셋 삭제 응답
#[derive(Debug, Clone, Serialize, ToSchema, TS)]
#[ts(export, export_to = "screening/")]
pub struct DeletePresetResponse {
    pub success: bool,
    pub message: String,
}

/// 프리셋 스크리닝 쿼리
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, export_to = "screening/")]
pub struct PresetQuery {
    /// 시장 필터 (KR, US)
    #[serde(default)]
    pub market: Option<String>,
    /// 결과 제한
    #[serde(default)]
    pub limit: Option<i32>,
    /// 오프셋 (페이징)
    #[serde(default)]
    pub offset: Option<i32>,
}

/// 모멘텀 스크리닝 쿼리
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, export_to = "screening/")]
pub struct MomentumQuery {
    /// 시장 필터
    #[serde(default)]
    pub market: Option<String>,
    /// 조회 기간 (일)
    #[serde(default = "default_momentum_days")]
    pub days: i32,
    /// 최소 변동률 (%)
    #[serde(default = "default_min_change")]
    pub min_change_pct: String,
    /// 최소 거래량 배율
    #[serde(default)]
    pub min_volume_ratio: Option<String>,
    /// 결과 제한
    #[serde(default = "default_momentum_limit")]
    pub limit: i32,
    /// 오프셋 (페이징)
    #[serde(default)]
    pub offset: Option<i32>,
}

fn default_momentum_days() -> i32 {
    5
}

fn default_min_change() -> String {
    "5".to_string()
}

fn default_momentum_limit() -> i32 {
    50
}

/// 모멘텀 스크리닝 결과 응답
#[derive(Debug, Clone, Serialize, ToSchema, TS)]
#[ts(export, export_to = "screening/")]
pub struct MomentumResponse {
    pub total: usize,
    pub days: i32,
    pub min_change_pct: String,
    pub results: Vec<MomentumResultDto>,
}

/// 모멘텀 결과 DTO
#[derive(Debug, Clone, Serialize, ToSchema, TS)]
#[ts(export, export_to = "screening/")]
pub struct MomentumResultDto {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub exchange: Option<String>,
    pub start_price: String,
    pub end_price: String,
    pub change_pct: String,
    pub avg_volume: String,
    pub current_volume: String,
    pub volume_ratio: String,
}

/// 에러 응답
#[derive(Debug, Clone, Serialize, ToSchema, TS)]
#[ts(export, export_to = "common/")]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
}

// ==================== 헬퍼 함수 ====================

fn parse_decimal(s: &Option<String>) -> Option<Decimal> {
    s.as_ref().and_then(|v| v.parse::<Decimal>().ok())
}

fn parse_f64(s: &Option<String>) -> Option<f64> {
    s.as_ref().and_then(|v| v.parse::<f64>().ok())
}

fn decimal_to_string(d: Option<Decimal>) -> Option<String> {
    d.map(|v| v.to_string())
}

fn to_screening_filter(req: &ScreeningRequest) -> ScreeningFilter {
    ScreeningFilter {
        market: req.market.clone(),
        exchange: req.exchange.clone(),
        sector: req.sector.clone(),
        min_market_cap: parse_decimal(&req.min_market_cap),
        max_market_cap: parse_decimal(&req.max_market_cap),
        min_per: parse_decimal(&req.min_per),
        max_per: parse_decimal(&req.max_per),
        min_pbr: parse_decimal(&req.min_pbr),
        max_pbr: parse_decimal(&req.max_pbr),
        min_roe: parse_decimal(&req.min_roe),
        max_roe: parse_decimal(&req.max_roe),
        min_roa: parse_decimal(&req.min_roa),
        max_roa: parse_decimal(&req.max_roa),
        min_dividend_yield: parse_decimal(&req.min_dividend_yield),
        max_dividend_yield: parse_decimal(&req.max_dividend_yield),
        max_debt_ratio: parse_decimal(&req.max_debt_ratio),
        min_revenue_growth: parse_decimal(&req.min_revenue_growth),
        min_earnings_growth: parse_decimal(&req.min_earnings_growth),
        max_distance_from_52w_high: parse_decimal(&req.max_distance_from_52w_high),
        min_distance_from_52w_low: parse_decimal(&req.min_distance_from_52w_low),
        min_volume_ratio: parse_decimal(&req.min_volume_ratio),
        min_low_trend: parse_f64(&req.min_low_trend),
        min_vol_quality: parse_f64(&req.min_vol_quality),
        min_breakout_score: parse_f64(&req.min_breakout_score),
        only_alive_consolidation: req.only_alive_consolidation,
        filter_route_state: req.filter_route_state.clone(),
        filter_ttm_squeeze: req.filter_ttm_squeeze,
        min_ttm_squeeze_cnt: req
            .min_ttm_squeeze_cnt
            .as_ref()
            .and_then(|v| v.parse::<i32>().ok()),
        sort_by: req.sort_by.clone(),
        sort_order: req.sort_order.clone(),
        limit: req.limit,
        offset: req.offset,
        ..Default::default()
    }
}

fn to_result_dto(r: ScreeningResult) -> ScreeningResultDto {
    ScreeningResultDto {
        ticker: r.ticker,
        name: r.name,
        market: r.market,
        exchange: r.exchange,
        sector: r.sector,
        market_cap: decimal_to_string(r.market_cap),
        per: decimal_to_string(r.per),
        pbr: decimal_to_string(r.pbr),
        roe: decimal_to_string(r.roe),
        roa: decimal_to_string(r.roa),
        eps: decimal_to_string(r.eps),
        dividend_yield: decimal_to_string(r.dividend_yield),
        operating_margin: decimal_to_string(r.operating_margin),
        debt_ratio: decimal_to_string(r.debt_ratio),
        revenue_growth_yoy: decimal_to_string(r.revenue_growth_yoy),
        earnings_growth_yoy: decimal_to_string(r.earnings_growth_yoy),
        current_price: decimal_to_string(r.current_price),
        week_52_high: decimal_to_string(r.week_52_high),
        week_52_low: decimal_to_string(r.week_52_low),
        distance_from_52w_high: decimal_to_string(r.distance_from_52w_high),
        distance_from_52w_low: decimal_to_string(r.distance_from_52w_low),
        low_trend: r.low_trend,
        vol_quality: r.vol_quality,
        range_pos: r.range_pos,
        dist_ma20: r.dist_ma20,
        bb_width: r.bb_width,
        rsi_14: r.rsi_14,
        breakout_score: r.breakout_score,
        macd: r.macd,
        macd_signal: r.macd_signal,
        macd_histogram: r.macd_histogram,
        macd_cross: r.macd_cross,
        route_state: r.route_state,
        regime: r.regime,
        sector_rs: decimal_to_string(r.sector_rs),
        sector_rank: r.sector_rank,
        ttm_squeeze: r.ttm_squeeze,
        ttm_squeeze_cnt: r.ttm_squeeze_cnt,
        trigger_score: r.trigger_score,
        trigger_label: r.trigger_label,
        overall_score: decimal_to_string(r.overall_score),
        grade: r.grade,
        confidence: r.confidence,
    }
}

fn to_momentum_dto(r: MomentumScreenResult) -> MomentumResultDto {
    MomentumResultDto {
        symbol: r.symbol,
        name: r.name,
        market: r.market,
        exchange: r.exchange,
        start_price: r.start_price.to_string(),
        end_price: r.end_price.to_string(),
        change_pct: r.change_pct.to_string(),
        avg_volume: r.avg_volume.to_string(),
        current_volume: r.current_volume.to_string(),
        volume_ratio: r.volume_ratio.to_string(),
    }
}

/// 섹터 순위 쿼리
#[derive(Debug, Clone, Deserialize)]
pub struct SectorRankingQuery {
    /// 시장 필터 (KR, US)
    #[serde(default)]
    pub market: Option<String>,
    /// 계산 기간 (일, 기본: 20)
    #[serde(default)]
    pub days: Option<i32>,
}

/// 섹터 순위 응답
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SectorRankingResponse {
    /// 총 섹터 수
    pub total: usize,
    /// 계산 기간 (일)
    pub days: i32,
    /// 적용된 시장 필터
    pub market: Option<String>,
    /// 섹터 목록
    pub results: Vec<SectorRsDto>,
}

/// 섹터 RS DTO
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SectorRsDto {
    /// 섹터명
    pub sector: String,
    /// 섹터 내 종목 수
    pub symbol_count: i64,
    /// 섹터 평균 수익률 (%)
    pub avg_return_pct: String,
    /// 시장 평균 수익률 (%)
    pub market_return: String,
    /// 상대강도 (RS)
    pub relative_strength: String,
    /// 종합 점수
    pub composite_score: String,
    /// 순위
    pub rank: i32,
    /// 5일 평균 수익률 (%) - SectorMomentumBar 용
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_return_5d_pct: Option<String>,
    /// 섹터 총 시가총액 - SectorTreemap 용
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_market_cap: Option<String>,
}

fn to_sector_rs_dto(r: crate::repository::SectorRsResult) -> SectorRsDto {
    SectorRsDto {
        sector: r.sector,
        symbol_count: r.symbol_count,
        avg_return_pct: r.avg_return_pct.to_string(),
        market_return: r.market_return.to_string(),
        relative_strength: r.relative_strength.to_string(),
        composite_score: r.composite_score.to_string(),
        rank: r.rank,
        avg_return_5d_pct: r.avg_return_5d_pct.map(|v| v.to_string()),
        total_market_cap: r.total_market_cap.map(|v| v.to_string()),
    }
}

fn error_response(code: &str, message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            code: code.to_string(),
            message: message.to_string(),
        }),
    )
}

// ==================== 핸들러 ====================

/// 커스텀 스크리닝 실행
///
/// POST /api/v1/screening
#[utoipa::path(
    post,
    path = "/api/v1/screening",
    request_body = ScreeningRequest,
    responses(
        (status = 200, description = "스크리닝 성공", body = ScreeningResponse),
        (status = 500, description = "서버 오류", body = ErrorResponse)
    ),
    tag = "screening"
)]
pub async fn run_screening(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ScreeningRequest>,
) -> impl IntoResponse {
    debug!("커스텀 스크리닝 요청: {:?}", request);

    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return error_response("DATABASE_ERROR", "Database not available").into_response();
        }
    };

    // 공유 data_provider 사용 (Redis 3계층 캐시 포함)
    let data_provider = match &state.data_provider {
        Some(dp) => dp,
        None => {
            return error_response("DATA_PROVIDER_ERROR", "Data provider not available").into_response();
        }
    };

    let filter = to_screening_filter(&request);

    let redis_cache = state.cache.as_deref();
    let results = match ScreeningRepository::screen(db_pool, data_provider, &filter, redis_cache).await {
        Ok(r) => r,
        Err(e) => {
            warn!("스크리닝 실패: {}", e);
            return error_response("SCREENING_ERROR", &format!("스크리닝 실패: {}", e))
                .into_response();
        }
    };

    // 섹터 RS 정보 추가
    let results = match ScreeningRepository::enrich_with_sector_rs(
        db_pool,
        results,
        request.market.as_deref(),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("섹터 RS 추가 실패 (무시): {}", e);
            vec![] // 실패하면 빈 결과 반환
        }
    };

    // 매크로 환경 평가 (Redis 캐시에서 읽기, 실패 시 기본값 사용)
    let macro_risk_str = match fetch_and_evaluate_macro_env(state.cache.as_deref(), 3).await {
        Ok(env) => {
            info!("매크로 환경: {}", env.summary());
            Some(format!(
                "{} {} (EBS: {}, 추천: {}개)",
                env.risk_level,
                env.risk_level.icon(),
                env.adjusted_ebs,
                if env.recommendation_limit == usize::MAX {
                    "무제한".to_string()
                } else {
                    env.recommendation_limit.to_string()
                }
            ))
        }
        Err(e) => {
            warn!("매크로 환경 평가 실패 (무시): {}", e);
            None
        }
    };

    let filter_summary = build_filter_summary(&request);
    let total = results.len();
    let dto_results: Vec<ScreeningResultDto> = results.into_iter().map(to_result_dto).collect();

    Json(ScreeningResponse {
        total,
        results: dto_results,
        filter_summary,
        macro_risk: macro_risk_str,
    })
    .into_response()
}

/// 프리셋 목록 조회
///
/// GET /api/v1/screening/presets
#[utoipa::path(
    get,
    path = "/api/v1/screening/presets",
    responses(
        (status = 200, description = "프리셋 목록", body = PresetsListResponse)
    ),
    tag = "screening"
)]
pub async fn list_presets() -> impl IntoResponse {
    let presets = ScreeningRepository::available_presets();
    Json(PresetsListResponse { presets })
}

/// 프리셋 목록 조회 (DB)
///
/// GET /api/v1/screening/presets/all
pub async fn list_presets_v2(
    State(state): State<Arc<AppState>>,
) -> Result<Json<PresetsListResponseV2>, (StatusCode, String)> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    let presets = ScreeningRepository::get_all_presets(pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("조회 실패: {}", e),
            )
        })?;

    let total = presets.len();

    Ok(Json(PresetsListResponseV2 { presets, total }))
}

/// 프리셋 저장
///
/// POST /api/v1/screening/presets
pub async fn save_preset(
    State(state): State<Arc<AppState>>,
    Json(request): Json<crate::repository::CreatePresetRequest>,
) -> Result<Json<SavePresetResponse>, (StatusCode, String)> {
    info!("프리셋 저장 요청: {}", request.name);

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    let preset = ScreeningRepository::save_preset(pool, request)
        .await
        .map_err(|e| {
            if e.to_string().contains("unique_preset_name") {
                (
                    StatusCode::CONFLICT,
                    "이미 존재하는 프리셋 이름입니다".to_string(),
                )
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("저장 실패: {}", e),
                )
            }
        })?;

    Ok(Json(SavePresetResponse {
        success: true,
        message: format!("프리셋 '{}'이(가) 저장되었습니다", preset.name),
        preset,
    }))
}

/// 프리셋 삭제
///
/// DELETE /api/v1/screening/presets/{id}
pub async fn delete_preset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<DeletePresetResponse>, (StatusCode, String)> {
    info!("프리셋 삭제 요청: {}", id);

    let pool = state.db_pool.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database not available".to_string(),
        )
    })?;

    let deleted = ScreeningRepository::delete_preset(pool, id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("삭제 실패: {}", e),
            )
        })?;

    if !deleted {
        return Err((
            StatusCode::BAD_REQUEST,
            "기본 프리셋은 삭제할 수 없습니다".to_string(),
        ));
    }

    Ok(Json(DeletePresetResponse {
        success: true,
        message: "프리셋이 삭제되었습니다".to_string(),
    }))
}

/// 프리셋 스크리닝 실행
///
/// GET /api/v1/screening/presets/{preset}
#[utoipa::path(
    get,
    path = "/api/v1/screening/presets/{preset}",
    params(
        ("preset" = String, Path, description = "프리셋 ID (value, dividend, growth, snowball, large_cap, near_52w_low)")
    ),
    responses(
        (status = 200, description = "프리셋 스크리닝 성공", body = ScreeningResponse),
        (status = 500, description = "서버 오류", body = ErrorResponse)
    ),
    tag = "screening"
)]
pub async fn run_preset_screening(
    State(state): State<Arc<AppState>>,
    Path(preset): Path<String>,
    Query(query): Query<PresetQuery>,
) -> impl IntoResponse {
    debug!(
        "프리셋 스크리닝 요청: preset={}, market={:?}",
        preset, query.market
    );

    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return error_response("DATABASE_ERROR", "Database not available").into_response();
        }
    };

    // 공유 data_provider 사용 (Redis 3계층 캐시 포함)
    let data_provider = match &state.data_provider {
        Some(dp) => dp,
        None => {
            return error_response("DATA_PROVIDER_ERROR", "Data provider not available").into_response();
        }
    };

    let redis_cache = state.cache.as_deref();
    let results =
        match ScreeningRepository::screen_preset(db_pool, data_provider, &preset, query.market.as_deref(), redis_cache).await {
            Ok(r) => r,
            Err(e) => {
                warn!("프리셋 스크리닝 실패: {}", e);
                return error_response("SCREENING_ERROR", &format!("프리셋 스크리닝 실패: {}", e))
                    .into_response();
            }
        };

    // 섹터 RS 정보 추가
    let results =
        match ScreeningRepository::enrich_with_sector_rs(db_pool, results, query.market.as_deref())
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("섹터 RS 추가 실패 (무시): {}", e);
                vec![] // 실패하면 빈 결과 반환
            }
        };

    let filter_summary = format!(
        "프리셋: {}, 시장: {}",
        preset,
        query.market.as_deref().unwrap_or("전체")
    );
    let total = results.len();

    // 전체 결과 반환 (프론트엔드 무한 스크롤 대응)
    let results: Vec<_> = results.into_iter().collect();

    let dto_results: Vec<ScreeningResultDto> = results.into_iter().map(to_result_dto).collect();

    // 매크로 환경 평가 (Redis 캐시에서 읽기, 실패 시 None)
    let macro_risk_str = match fetch_and_evaluate_macro_env(state.cache.as_deref(), 3).await {
        Ok(env) => {
            info!("프리셋 스크리닝 매크로 환경: {}", env.summary());
            Some(format!(
                "{} {} (EBS: {}, 추천: {}개)",
                env.risk_level,
                env.risk_level.icon(),
                env.adjusted_ebs,
                if env.recommendation_limit == usize::MAX {
                    "무제한".to_string()
                } else {
                    env.recommendation_limit.to_string()
                }
            ))
        }
        Err(e) => {
            warn!("프리셋 스크리닝 매크로 환경 평가 실패 (무시): {}", e);
            None
        }
    };

    Json(ScreeningResponse {
        total,
        results: dto_results,
        filter_summary,
        macro_risk: macro_risk_str,
    })
    .into_response()
}

/// 모멘텀 스크리닝 실행
///
/// GET /api/v1/screening/momentum
#[utoipa::path(
    get,
    path = "/api/v1/screening/momentum",
    params(
        ("market" = Option<String>, Query, description = "시장 필터 (KR, US)"),
        ("days" = Option<i32>, Query, description = "조회 기간 (일)"),
        ("min_change_pct" = Option<String>, Query, description = "최소 변동률 (%)"),
        ("min_volume_ratio" = Option<String>, Query, description = "최소 거래량 배율"),
        ("limit" = Option<i32>, Query, description = "결과 제한")
    ),
    responses(
        (status = 200, description = "모멘텀 스크리닝 성공", body = MomentumResponse),
        (status = 400, description = "잘못된 요청", body = ErrorResponse),
        (status = 500, description = "서버 오류", body = ErrorResponse)
    ),
    tag = "screening"
)]
pub async fn run_momentum_screening(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MomentumQuery>,
) -> impl IntoResponse {
    debug!(
        "모멘텀 스크리닝 요청: days={}, min_change={}, market={:?}",
        query.days, query.min_change_pct, query.market
    );

    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return error_response("DATABASE_ERROR", "Database not available").into_response();
        }
    };

    let min_change: Decimal = match query.min_change_pct.parse() {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    code: "BAD_REQUEST".to_string(),
                    message: "Invalid min_change_pct value".to_string(),
                }),
            )
                .into_response();
        }
    };

    let min_volume_ratio = query
        .min_volume_ratio
        .as_ref()
        .and_then(|v| v.parse::<Decimal>().ok());

    let results = match ScreeningRepository::screen_momentum(
        db_pool,
        query.market.as_deref(),
        query.days,
        min_change,
        min_volume_ratio,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("모멘텀 스크리닝 실패: {}", e);
            return error_response("SCREENING_ERROR", &format!("모멘텀 스크리닝 실패: {}", e))
                .into_response();
        }
    };

    let total = results.len();
    let dto_results: Vec<MomentumResultDto> = results.into_iter().map(to_momentum_dto).collect();

    Json(MomentumResponse {
        total,
        days: query.days,
        min_change_pct: query.min_change_pct,
        results: dto_results,
    })
    .into_response()
}

/// 섹터 순위 조회
///
/// GET /api/v1/sectors/ranking
#[utoipa::path(
    get,
    path = "/api/v1/sectors/ranking",
    params(
        ("market" = Option<String>, Query, description = "시장 필터 (KR, US)"),
        ("days" = Option<i32>, Query, description = "계산 기간 (일, 기본: 20)")
    ),
    responses(
        (status = 200, description = "섹터 순위 조회 성공", body = SectorRankingResponse),
        (status = 500, description = "서버 오류", body = ErrorResponse)
    ),
    tag = "sectors"
)]
pub async fn get_sector_ranking(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SectorRankingQuery>,
) -> impl IntoResponse {
    debug!(
        "섹터 순위 조회 요청: market={:?}, days={:?}",
        query.market, query.days
    );

    let db_pool = match &state.db_pool {
        Some(pool) => pool,
        None => {
            return error_response("DATABASE_ERROR", "Database not available").into_response();
        }
    };

    let days = query.days.unwrap_or(20);

    let results = match ScreeningRepository::calculate_sector_rs(
        db_pool,
        query.market.as_deref(),
        days,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("섹터 순위 조회 실패: {}", e);
            return error_response("SECTOR_RS_ERROR", &format!("섹터 순위 조회 실패: {}", e))
                .into_response();
        }
    };

    // 자산군 카테고리 및 시장 부 분류 제외 (산업 섹터만 표시)
    // - 자산 분류: "주식", "채권", "혼합자산", "Cryptocurrency"
    // - KOSPI/KOSDAQ 시장 부: "중견기업부", "우량기업부", "벤처기업부" 등
    const EXCLUDED_CATEGORIES: &[&str] = &[
        // 자산 분류
        "주식",
        "채권",
        "혼합자산",
        "Cryptocurrency",
        "기타",
        // KOSPI/KOSDAQ 시장 부 분류 (산업 섹터 아님)
        "중견기업부",
        "우량기업부",
        "벤처기업부",
        "기술성장기업부",
        "투자주의환기종목(소속부없음)",
        "SPAC(소속부없음)",
        "외국기업(소속부없음)",
    ];

    let filtered_results: Vec<_> = results
        .into_iter()
        .filter(|r| !EXCLUDED_CATEGORIES.contains(&r.sector.as_str()))
        .collect();

    let total = filtered_results.len();
    let dto_results: Vec<SectorRsDto> = filtered_results.into_iter().map(to_sector_rs_dto).collect();

    Json(SectorRankingResponse {
        total,
        days,
        market: query.market,
        results: dto_results,
    })
    .into_response()
}

/// 매크로 경제 지표 조회 및 환경 평가 (Redis 캐시 전용).
///
/// Collector가 5분마다 Redis에 캐시한 매크로 데이터를 읽습니다.
/// 외부 API를 호출하지 않으므로 빠르고 안정적입니다.
///
/// # 파라미터
///
/// - `cache`: Redis 캐시 (없으면 에러 반환)
/// - `base_ebs`: 기본 EBS 기준값 (일반적으로 3)
///
/// # 반환
///
/// - `Ok(MacroEnvironment)`: 평가된 매크로 환경
/// - `Err(String)`: 에러 메시지
async fn fetch_and_evaluate_macro_env(
    cache: Option<&RedisCache>,
    base_ebs: u8,
) -> Result<MacroEnvironment, String> {
    // Redis 캐시에서 매크로 데이터 읽기
    let cache = cache.ok_or_else(|| "Redis 캐시가 설정되지 않았습니다".to_string())?;

    let data: MacroData = cache
        .get("macro:data")
        .await
        .map_err(|e| format!("매크로 데이터 캐시 조회 실패: {}", e))?
        .ok_or_else(|| {
            "매크로 데이터가 캐시에 없습니다. Collector를 실행하세요.".to_string()
        })?;

    // 매크로 환경 평가
    let env = MacroEnvironment::evaluate(
        data.usd_krw,
        data.usd_change_pct,
        data.nasdaq_change_pct,
        base_ebs,
    );

    Ok(env)
}

/// 필터 요약 문자열 생성
fn build_filter_summary(req: &ScreeningRequest) -> String {
    let mut parts = Vec::new();

    if let Some(ref m) = req.market {
        parts.push(format!("시장={}", m));
    }
    if let Some(ref e) = req.exchange {
        parts.push(format!("거래소={}", e));
    }
    if let Some(max_per) = &req.max_per {
        parts.push(format!("PER≤{}", max_per));
    }
    if let Some(max_pbr) = &req.max_pbr {
        parts.push(format!("PBR≤{}", max_pbr));
    }
    if let Some(min_roe) = &req.min_roe {
        parts.push(format!("ROE≥{}", min_roe));
    }
    if let Some(min_dividend_yield) = &req.min_dividend_yield {
        parts.push(format!("배당수익률≥{}", min_dividend_yield));
    }

    if parts.is_empty() {
        "필터 없음".to_string()
    } else {
        parts.join(", ")
    }
}

// ==================== 라우터 ====================

/// 스크리닝 라우터 생성
pub fn screening_router() -> Router<Arc<AppState>> {
    use axum::routing::delete;

    Router::new()
        .route("/", post(run_screening))
        .route("/presets", get(list_presets).post(save_preset))
        .route("/presets/all", get(list_presets_v2))
        .route("/presets/{preset}", get(run_preset_screening))
        .route("/presets/id/{id}", delete(delete_preset))
        .route("/momentum", get(run_momentum_screening))
}

/// 섹터 분석 라우터 생성
pub fn sectors_router() -> Router<Arc<AppState>> {
    Router::new().route("/ranking", get(get_sector_ranking))
}
