//! 시장 상태 endpoint.
//!
//! 시장 상태, 시장 온도, 매크로 환경을 통합 조회합니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/market/overview` - 시장 상태 통합 조회 (status + breadth + macro)
//! - `GET /api/v1/market/klines` - 캔들스틱 데이터 조회 (실시간 거래소 데이터)
//! - `GET /api/v1/market/ticker` - 현재가 조회

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{Datelike, NaiveTime, Timelike, Utc, Weekday};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, warn};
use utoipa::{IntoParams, ToSchema};

use trader_core::Timeframe;
// API 서버는 ohlcv 테이블에서만 읽음 (외부 API 호출 없음)

use crate::repository::KlinesRepository;
use crate::routes::strategies::ApiError;
use crate::state::AppState;

// ==================== 응답 타입 ====================

/// 시장 상태 응답.
///
/// Frontend의 MarketStatus 타입과 매칭됩니다.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarketStatusResponse {
    /// 시장 코드 (KR/US)
    pub market: String,
    /// 시장 개장 여부
    pub is_open: bool,
    /// 다음 개장 시간 (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_open: Option<String>,
    /// 다음 폐장 시간 (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_close: Option<String>,
    /// 현재 세션 (Regular/PreMarket/AfterHours)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// 시장 통합 조회 응답.
///
/// 시장 상태(KR/US), 시장 온도(Breadth), 매크로 환경을 하나로 통합합니다.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarketOverviewResponse {
    /// 한국 시장 상태
    pub kr: MarketStatusResponse,
    /// 미국 시장 상태
    pub us: MarketStatusResponse,
    /// 시장 온도 (Collector 미실행 시 null)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breadth: Option<MarketBreadthResponse>,
    /// 매크로 환경 (Collector 미실행 시 null)
    #[serde(rename = "macro")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub macro_env: Option<MacroEnvironmentResponse>,
}

/// 시장 세션 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, ToSchema)]
pub enum MarketSession {
    Regular,
    PreMarket,
    AfterHours,
    Closed,
}

impl MarketSession {
    pub fn as_str(&self) -> Option<&'static str> {
        match self {
            MarketSession::Regular => Some("Regular"),
            MarketSession::PreMarket => Some("PreMarket"),
            MarketSession::AfterHours => Some("AfterHours"),
            MarketSession::Closed => None,
        }
    }
}

// ==================== Handler ====================

/// 시장 통합 조회.
///
/// GET /api/v1/market/overview
///
/// KR/US 시장 상태, Market Breadth, 매크로 환경을 하나의 응답으로 반환합니다.
/// Breadth/Macro는 Redis 캐시에서 조회하며, 데이터가 없으면 null 반환합니다.
#[utoipa::path(
    get,
    path = "/api/v1/market/overview",
    responses(
        (status = 200, description = "시장 통합 정보", body = MarketOverviewResponse)
    ),
    tag = "market"
)]
pub async fn get_market_overview(
    State(state): State<Arc<AppState>>,
) -> Json<MarketOverviewResponse> {
    let kr = get_kr_market_status();
    let us = get_us_market_status();

    // Breadth: Redis 캐시에서 조회, 실패 시 None
    let breadth = if let Some(cache) = &state.cache {
        match cache
            .get::<trader_core::MarketBreadth>(MARKET_BREADTH_CACHE_KEY)
            .await
        {
            Ok(Some(b)) => Some(MarketBreadthResponse {
                all: b.all_pct().to_string(),
                kospi: b.kospi_pct().to_string(),
                kosdaq: b.kosdaq_pct().to_string(),
                temperature: b.temperature.to_string(),
                temperature_icon: b.temperature.icon().to_string(),
                recommendation: b.temperature.recommendation().to_string(),
                calculated_at: b.calculated_at.to_rfc3339(),
            }),
            _ => None,
        }
    } else {
        None
    };

    // Macro: Redis 캐시에서 조회, 실패 시 None
    let macro_env = if let Some(cache) = &state.cache {
        match cache
            .get::<trader_data::cache::MacroData>(MACRO_ENV_CACHE_KEY)
            .await
        {
            Ok(Some(data)) => {
                let env = trader_core::MacroEnvironment::evaluate(
                    data.usd_krw,
                    data.usd_change_pct,
                    data.nasdaq_change_pct,
                    3,
                );
                Some(MacroEnvironmentResponse {
                    kospi: data.kospi_close.to_string(),
                    kospi_change_pct: data.kospi_change_pct,
                    kosdaq: data.kosdaq_close.to_string(),
                    kosdaq_change_pct: data.kosdaq_change_pct,
                    usd_krw: data.usd_krw.to_string(),
                    usd_change_pct: data.usd_change_pct,
                    vix: data.vix_close.to_string(),
                    vix_change_pct: data.vix_change_pct,
                    nasdaq: data.nasdaq_close.to_string(),
                    nasdaq_change_pct: data.nasdaq_change_pct,
                    risk_level: env.risk_level.to_string(),
                    risk_icon: env.risk_level.icon().to_string(),
                    adjusted_ebs: env.adjusted_ebs,
                    recommendation_limit: env.recommendation_limit,
                    summary: env.summary(),
                })
            }
            _ => None,
        }
    } else {
        None
    };

    debug!(
        kr_open = kr.is_open,
        us_open = us.is_open,
        breadth = breadth.is_some(),
        macro_env = macro_env.is_some(),
        "시장 통합 조회 완료"
    );

    Json(MarketOverviewResponse {
        kr,
        us,
        breadth,
        macro_env,
    })
}

/// 한국 시장 상태 계산.
///
/// 정규장: 09:00-15:30 KST (UTC+9)
fn get_kr_market_status() -> MarketStatusResponse {
    let now = Utc::now();
    // KST = UTC + 9시간
    let kst_hour = (now.hour() + 9) % 24;
    let kst_minute = now.minute();

    let is_weekday = !matches!(now.weekday(), Weekday::Sat | Weekday::Sun);

    // 정규장 시간: 09:00-15:30 KST
    let market_open = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
    let market_close = NaiveTime::from_hms_opt(15, 30, 0).unwrap();
    let current_time = NaiveTime::from_hms_opt(kst_hour, kst_minute, 0).unwrap();

    let is_open = is_weekday && current_time >= market_open && current_time < market_close;

    let session = if is_open {
        Some("Regular".to_string())
    } else {
        None
    };

    debug!(
        "KR market status: is_open={}, kst_hour={}, kst_minute={}",
        is_open, kst_hour, kst_minute
    );

    MarketStatusResponse {
        market: "KR".to_string(),
        is_open,
        next_open: None,  // TODO: 다음 개장 시간 계산
        next_close: None, // TODO: 다음 폐장 시간 계산
        session,
    }
}

/// 미국 시장 상태 계산.
///
/// - 프리마켓: 04:00-09:30 EST
/// - 정규장: 09:30-16:00 EST
/// - 애프터아워: 16:00-20:00 EST
fn get_us_market_status() -> MarketStatusResponse {
    let now = Utc::now();
    // EST = UTC - 5시간 (DST 미적용시)
    // EDT = UTC - 4시간 (DST 적용시, 3월 둘째 일요일 ~ 11월 첫째 일요일)
    // 간단히 -5로 계산 (정확한 DST 계산은 추후 개선)
    let est_hour = if now.hour() >= 5 {
        now.hour() - 5
    } else {
        24 + now.hour() - 5
    };
    let est_minute = now.minute();

    let is_weekday = !matches!(now.weekday(), Weekday::Sat | Weekday::Sun);

    let current_time = NaiveTime::from_hms_opt(est_hour, est_minute, 0).unwrap();

    // 시간대 정의
    let premarket_open = NaiveTime::from_hms_opt(4, 0, 0).unwrap();
    let regular_open = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
    let regular_close = NaiveTime::from_hms_opt(16, 0, 0).unwrap();
    let afterhours_close = NaiveTime::from_hms_opt(20, 0, 0).unwrap();

    let (is_open, session) = if !is_weekday {
        (false, MarketSession::Closed)
    } else if current_time >= premarket_open && current_time < regular_open {
        (true, MarketSession::PreMarket)
    } else if current_time >= regular_open && current_time < regular_close {
        (true, MarketSession::Regular)
    } else if current_time >= regular_close && current_time < afterhours_close {
        (true, MarketSession::AfterHours)
    } else {
        (false, MarketSession::Closed)
    };

    debug!(
        "US market status: is_open={}, session={:?}, est_hour={}",
        is_open, session, est_hour
    );

    MarketStatusResponse {
        market: "US".to_string(),
        is_open,
        next_open: None,
        next_close: None,
        session: session.as_str().map(|s| s.to_string()),
    }
}

// ==================== 캔들스틱 데이터 ====================

/// 캔들스틱 데이터 쿼리.
#[derive(Debug, Deserialize, IntoParams)]
pub struct KlinesQuery {
    /// 심볼 (예: BTC/USDT, 005930)
    pub symbol: String,
    /// 타임프레임 (1m, 5m, 15m, 1h, 4h, 1d)
    #[serde(default = "default_timeframe")]
    pub timeframe: String,
    /// 데이터 개수 (기본: 100)
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_timeframe() -> String {
    "1d".to_string()
}

fn default_limit() -> usize {
    100
}

/// 캔들스틱 데이터 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KlinesResponse {
    pub symbol: String,
    pub timeframe: String,
    pub data: Vec<CandleData>,
}

/// 단일 캔들 데이터.
#[derive(Debug, Serialize, ToSchema)]
pub struct CandleData {
    /// 타임스탬프 (ISO 8601 날짜)
    pub time: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// 다중 타임프레임 캔들 데이터 쿼리.
#[derive(Debug, Deserialize, IntoParams)]
pub struct MultiKlinesQuery {
    /// 심볼 (예: BTC/USDT, 005930)
    pub symbol: String,
    /// 쉼표로 구분된 타임프레임 목록 (예: "5m,1h,1d")
    pub timeframes: String,
    /// 타임프레임별 데이터 개수 (기본: 100)
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// 다중 타임프레임 캔들 데이터 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MultiKlinesResponse {
    pub symbol: String,
    /// 타임프레임별 캔들 데이터
    pub data: std::collections::HashMap<String, Vec<CandleData>>,
}

/// 캔들스틱 데이터 조회.
///
/// GET /api/v1/market/klines
///
/// **Yahoo Finance API**를 사용하여 과거 캔들 데이터를 조회합니다.
/// - 백테스트와 라이브에서 동일한 데이터셋 사용
/// - DB 캐시를 통한 효율적인 데이터 접근
/// - 분봉/시간봉: 최근 60일 제한
/// - 일봉 이상: 수년간 데이터 가능
/// - 한국 주식: ".KS" 접미사 자동 추가 (코스피)
///
/// # 캐싱 전략
/// - 요청 기반 자동 캐싱 및 증분 업데이트
/// - 동일 심볼+타임프레임 동시 요청 시 중복 API 호출 방지
/// - 시장 마감 후에는 불필요한 업데이트 생략
///
/// # 지원 간격
/// - 분봉: 1m, 5m, 15m, 30m
/// - 시간봉: 1h
/// - 일봉 이상: 1d, 1wk, 1mo
pub async fn get_klines(
    State(state): State<Arc<AppState>>,
    Query(query): Query<KlinesQuery>,
) -> Result<Json<KlinesResponse>, (StatusCode, Json<ApiError>)> {
    // 타임프레임 문자열을 Timeframe enum으로 변환
    let timeframe = parse_timeframe(&query.timeframe);

    debug!(
        symbol = %query.symbol,
        timeframe = %query.timeframe,
        limit = query.limit,
        "캔들 데이터 조회 시작"
    );

    // 공유 data_provider 사용 (Redis 3계층 캐시 포함)
    let cached_provider = state.data_provider.as_ref().ok_or_else(|| {
        error!("DB 연결 없음, 캔들 데이터 조회 불가");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "DB_NOT_CONNECTED",
                "데이터베이스 연결이 필요합니다".to_string(),
            )),
        )
    })?;

    debug!(
        symbol = %query.symbol,
        "읽기 전용 모드 - Redis/ohlcv 테이블에서 조회"
    );

    // 읽기 전용 모드: 외부 API 호출 없이 Redis/ohlcv 테이블에서만 조회
    let klines = cached_provider
        .get_klines_readonly(&query.symbol, timeframe, query.limit)
        .await
        .map_err(|e| {
            error!(
                symbol = %query.symbol,
                timeframe = %query.timeframe,
                error = %e,
                "캐시 데이터 조회 실패"
            );
            (
                StatusCode::BAD_GATEWAY,
                Json(ApiError::new(
                    "DATA_FETCH_ERROR",
                    format!("차트 데이터 조회 실패: {}", e),
                )),
            )
        })?;

    debug!(
        symbol = %query.symbol,
        timeframe = %query.timeframe,
        count = klines.len(),
        "캔들 데이터 조회 성공"
    );

    let candles = klines
        .into_iter()
        .map(|k| CandleData {
            time: k.open_time.format("%Y-%m-%d").to_string(),
            open: k.open.to_string().parse().unwrap_or(0.0),
            high: k.high.to_string().parse().unwrap_or(0.0),
            low: k.low.to_string().parse().unwrap_or(0.0),
            close: k.close.to_string().parse().unwrap_or(0.0),
            volume: k.volume.to_string().parse().unwrap_or(0.0),
        })
        .collect();

    Ok(Json(KlinesResponse {
        symbol: query.symbol,
        timeframe: query.timeframe,
        data: candles,
    }))
}

/// 다중 타임프레임 캔들스틱 데이터 조회.
///
/// GET /api/v1/market/klines/multi?symbol=BTC/USDT&timeframes=5m,1h,1d&limit=100
///
/// 여러 타임프레임의 캔들 데이터를 한 번에 조회합니다.
/// 다중 타임프레임 전략에서 필요한 모든 데이터를 단일 API 호출로 가져올 수 있습니다.
///
/// # 지원 타임프레임
/// - 분봉: 1m, 5m, 15m, 30m
/// - 시간봉: 1h, 4h
/// - 일봉 이상: 1d, 1wk, 1mo
pub async fn get_multi_klines(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MultiKlinesQuery>,
) -> Result<Json<MultiKlinesResponse>, (StatusCode, Json<ApiError>)> {
    // 타임프레임 목록 파싱
    let timeframes: Vec<String> = query
        .timeframes
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if timeframes.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "INVALID_TIMEFRAMES",
                "적어도 하나의 타임프레임을 지정해야 합니다",
            )),
        ));
    }

    if timeframes.len() > 5 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "TOO_MANY_TIMEFRAMES",
                "최대 5개의 타임프레임만 지원됩니다",
            )),
        ));
    }

    debug!(
        symbol = %query.symbol,
        timeframes = ?timeframes,
        limit = query.limit,
        "다중 타임프레임 캔들 데이터 조회 시작"
    );

    let mut data = std::collections::HashMap::new();

    // DB 연결이 있으면 단일 쿼리로 최적화된 조회 사용
    if let Some(pool) = &state.db_pool {
        // 단일 쿼리로 다중 타임프레임 조회 (UNION ALL 최적화)
        match KlinesRepository::get_latest_multi_timeframe(
            pool,
            &query.symbol,
            &timeframes,
            query.limit as i64,
        )
        .await
        {
            Ok(klines_by_tf) => {
                for (tf_str, klines) in klines_by_tf {
                    let candles: Vec<CandleData> = klines
                        .into_iter()
                        .map(|k| CandleData {
                            time: k.open_time.format("%Y-%m-%d %H:%M:%S").to_string(),
                            open: k.open.to_string().parse().unwrap_or(0.0),
                            high: k.high.to_string().parse().unwrap_or(0.0),
                            low: k.low.to_string().parse().unwrap_or(0.0),
                            close: k.close.to_string().parse().unwrap_or(0.0),
                            volume: k.volume.to_string().parse().unwrap_or(0.0),
                        })
                        .collect();
                    data.insert(tf_str, candles);
                }
                // 요청된 타임프레임 중 데이터가 없는 것은 빈 배열로 채움
                for tf_str in &timeframes {
                    data.entry(tf_str.clone()).or_insert_with(Vec::new);
                }
            }
            Err(e) => {
                warn!(
                    symbol = %query.symbol,
                    timeframes = ?timeframes,
                    error = %e,
                    "다중 타임프레임 데이터 조회 실패, 순차 조회로 폴백"
                );
                // 폴백: 순차 조회 (공유 data_provider 사용)
                let cached_provider = state.data_provider.as_ref().ok_or_else(|| {
                    (
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(ApiError::new("DB_NOT_CONNECTED", "데이터베이스 연결이 필요합니다")),
                    )
                })?;
                for tf_str in &timeframes {
                    let timeframe = parse_timeframe(tf_str);
                    match cached_provider
                        .get_klines_readonly(&query.symbol, timeframe, query.limit)
                        .await
                    {
                        Ok(klines) => {
                            let candles: Vec<CandleData> = klines
                                .into_iter()
                                .map(|k| CandleData {
                                    time: k.open_time.format("%Y-%m-%d %H:%M:%S").to_string(),
                                    open: k.open.to_string().parse().unwrap_or(0.0),
                                    high: k.high.to_string().parse().unwrap_or(0.0),
                                    low: k.low.to_string().parse().unwrap_or(0.0),
                                    close: k.close.to_string().parse().unwrap_or(0.0),
                                    volume: k.volume.to_string().parse().unwrap_or(0.0),
                                })
                                .collect();
                            data.insert(tf_str.clone(), candles);
                        }
                        Err(_) => {
                            data.insert(tf_str.clone(), vec![]);
                        }
                    }
                }
            }
        }
    } else {
        // DB 연결 필수 - ohlcv 테이블에서만 읽음
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "DB_NOT_CONNECTED",
                "데이터베이스 연결이 필요합니다".to_string(),
            )),
        ));
    }

    debug!(
        symbol = %query.symbol,
        timeframes = ?timeframes,
        "다중 타임프레임 캔들 데이터 조회 성공"
    );

    Ok(Json(MultiKlinesResponse {
        symbol: query.symbol,
        data,
    }))
}

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

// ==================== 현재가 (Ticker) ====================

/// 현재가 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TickerResponse {
    pub symbol: String,
    pub price: String,
    pub change_24h: String,
    pub change_24h_percent: String,
    pub high_24h: String,
    pub low_24h: String,
    pub volume_24h: String,
    pub timestamp: i64,
}

/// 현재가 쿼리.
#[derive(Debug, Deserialize, IntoParams)]
pub struct TickerQuery {
    pub symbol: String,
}

/// 현재가 조회.
///
/// GET /api/v1/market/ticker
///
/// AppState에 설정된 MarketDataProvider를 사용하여 현재가를 조회합니다.
/// MarketDataProvider는 현재 활성화된 거래소(KIS, Mock 등)에 연결되어 있습니다.
pub async fn get_ticker(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TickerQuery>,
) -> Result<Json<TickerResponse>, (StatusCode, Json<ApiError>)> {
    debug!(symbol = %query.symbol, "현재가 조회 요청");

    // MarketDataProvider 확인
    let provider = state.market_data_provider.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "MARKET_DATA_NOT_CONFIGURED",
                "시세 데이터 제공자가 설정되지 않았습니다. 거래소 계정을 등록하세요.",
            )),
        )
    })?;

    // MarketDataProvider를 통해 시세 조회
    match provider.get_quote(&query.symbol).await {
        Ok(quote) => {
            debug!(
                symbol = %query.symbol,
                price = %quote.current_price,
                provider = provider.provider_name(),
                "현재가 조회 성공"
            );

            Ok(Json(TickerResponse {
                symbol: query.symbol,
                price: quote.current_price.to_string(),
                change_24h: quote.price_change.to_string(),
                change_24h_percent: quote.change_percent.to_string(),
                high_24h: quote.high.to_string(),
                low_24h: quote.low.to_string(),
                volume_24h: quote.volume.to_string(),
                timestamp: quote.timestamp.timestamp(),
            }))
        }
        Err(e) => {
            error!(
                symbol = %query.symbol,
                error = %e,
                provider = provider.provider_name(),
                "현재가 조회 실패"
            );
            Err((
                StatusCode::BAD_GATEWAY,
                Json(ApiError::new(
                    "EXCHANGE_ERROR",
                    format!("현재가 조회 실패: {}", e),
                )),
            ))
        }
    }
}

// ==================== Market Breadth ====================

/// Market Breadth 응답.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarketBreadthResponse {
    /// 전체 시장 Above_MA20 비율 (백분율).
    pub all: String,
    /// KOSPI Above_MA20 비율 (백분율).
    pub kospi: String,
    /// KOSDAQ Above_MA20 비율 (백분율).
    pub kosdaq: String,
    /// 시장 온도 (OVERHEAT/NEUTRAL/COLD).
    pub temperature: String,
    /// 시장 온도 아이콘.
    pub temperature_icon: String,
    /// 매매 권장사항.
    pub recommendation: String,
    /// 계산 시각 (ISO 8601).
    pub calculated_at: String,
}

// ==================== 캐시 키 ====================

/// Market Breadth 캐시 키.
const MARKET_BREADTH_CACHE_KEY: &str = "macro:market_breadth";

/// 매크로 환경 캐시 키.
const MACRO_ENV_CACHE_KEY: &str = "macro:data";

/// 매크로 환경 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MacroEnvironmentResponse {
    /// KOSPI 지수
    pub kospi: String,
    /// KOSPI 전일 대비 변동률 (%)
    pub kospi_change_pct: f64,
    /// KOSDAQ 지수
    pub kosdaq: String,
    /// KOSDAQ 전일 대비 변동률 (%)
    pub kosdaq_change_pct: f64,
    /// USD/KRW 환율
    pub usd_krw: String,
    /// USD/KRW 전일 대비 변동률 (%)
    pub usd_change_pct: f64,
    /// VIX 변동성 지수
    pub vix: String,
    /// VIX 전일 대비 변동률 (%)
    pub vix_change_pct: f64,
    /// 나스닥 지수
    pub nasdaq: String,
    /// 나스닥 전일 대비 변동률 (%)
    pub nasdaq_change_pct: f64,
    /// 위험도 수준 (SAFE/CAUTION/WARNING/CRITICAL)
    pub risk_level: String,
    /// 위험도 아이콘
    pub risk_icon: String,
    /// 조정된 EBS 기준
    pub adjusted_ebs: u8,
    /// 추천 종목 수 제한
    pub recommendation_limit: usize,
    /// 요약 메시지
    pub summary: String,
}


// ==================== 라우터 ====================

/// 시장 상태 라우터 생성.
pub fn market_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/overview", get(get_market_overview))
        .route("/klines", get(get_klines))
        .route("/klines/multi", get(get_multi_klines))
        .route("/ticker", get(get_ticker))
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
    async fn test_get_market_overview() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/market/overview", get(get_market_overview))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/market/overview")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let overview: MarketOverviewResponse = serde_json::from_slice(&body).unwrap();

        // KR/US 상태는 항상 반환됨
        assert_eq!(overview.kr.market, "KR");
        assert_eq!(overview.us.market, "US");
        // Redis 없는 테스트 환경이므로 breadth/macro는 None
        assert!(overview.breadth.is_none());
        assert!(overview.macro_env.is_none());
    }
}
