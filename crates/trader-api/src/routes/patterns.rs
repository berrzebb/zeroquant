//! ML 패턴 인식 API 엔드포인트.
//!
//! 캔들스틱 및 차트 패턴 감지를 위한 REST API입니다.
//!
//! # 엔드포인트
//!
//! - `GET /api/v1/patterns/candlestick` - 캔들스틱 패턴 감지
//! - `GET /api/v1/patterns/chart` - 차트 패턴 감지
//! - `GET /api/v1/patterns/detect` - 모든 패턴 감지 (캔들스틱 + 차트)
//! - `GET /api/v1/patterns/types` - 지원되는 패턴 타입 목록

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use trader_analytics::ml::{CandlestickPatternInfo, ChartPatternInfo, PatternDetectionResult};
use trader_core::{Kline, Timeframe};
use utoipa::{IntoParams, ToSchema};

use crate::state::AppState;

/// 타임프레임 문자열 파싱 헬퍼
fn parse_timeframe(s: &str) -> Option<Timeframe> {
    s.parse::<Timeframe>().ok()
}

/// data_provider에서 Kline 데이터 조회
async fn fetch_klines(
    state: &AppState,
    symbol: &str,
    timeframe_str: &str,
    limit: usize,
) -> Vec<Kline> {
    let Some(timeframe) = parse_timeframe(timeframe_str) else {
        tracing::warn!("Invalid timeframe: {}", timeframe_str);
        return Vec::new();
    };

    let Some(ref data_provider) = state.data_provider else {
        tracing::warn!("data_provider not configured");
        return Vec::new();
    };

    match data_provider.get_klines(symbol, timeframe, limit).await {
        Ok(klines) => {
            tracing::debug!(
                "Fetched {} klines for {} ({})",
                klines.len(),
                symbol,
                timeframe_str
            );
            klines
        }
        Err(e) => {
            tracing::warn!("Failed to fetch klines for {}: {}", symbol, e);
            Vec::new()
        }
    }
}

// ==================== 쿼리 파라미터 ====================

/// 패턴 감지 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct PatternQuery {
    /// 심볼 (예: "BTC/USDT", "005930")
    pub symbol: String,
    /// 타임프레임 (1m, 5m, 15m, 1h, 4h, 1d)
    #[serde(default = "default_timeframe")]
    pub timeframe: String,
    /// 분석할 캔들 수 (기본: 100)
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// 최소 신뢰도 (0.0 - 1.0, 기본: 0.6)
    #[serde(default = "default_confidence")]
    pub min_confidence: f64,
}

fn default_timeframe() -> String {
    "1h".to_string()
}

fn default_limit() -> usize {
    100
}

fn default_confidence() -> f64 {
    0.6
}

// ==================== 응답 타입 ====================

/// 캔들스틱 패턴 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct CandlestickPatternsResponse {
    /// 심볼
    pub symbol: String,
    /// 타임프레임
    pub timeframe: String,
    /// 분석된 캔들 수
    pub candles_analyzed: usize,
    /// 감지된 패턴 수
    pub patterns_found: usize,
    /// 감지된 패턴 목록
    pub patterns: Vec<CandlestickPatternInfo>,
}

/// 차트 패턴 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct ChartPatternsResponse {
    /// 심볼
    pub symbol: String,
    /// 타임프레임
    pub timeframe: String,
    /// 분석된 캔들 수
    pub candles_analyzed: usize,
    /// 감지된 패턴 수
    pub patterns_found: usize,
    /// 감지된 패턴 목록
    pub patterns: Vec<ChartPatternInfo>,
}

/// 패턴 타입 정보.
#[derive(Debug, Serialize, ToSchema)]
pub struct PatternTypeInfo {
    /// 패턴 ID (snake_case)
    pub id: String,
    /// 패턴 이름 (한글)
    pub name: String,
    /// 패턴 카테고리
    pub category: String,
    /// 신호 방향
    pub signal: String,
    /// 패턴 설명
    pub description: String,
}

/// 지원 패턴 타입 목록 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct PatternTypesResponse {
    /// 캔들스틱 패턴 타입
    pub candlestick_patterns: Vec<PatternTypeInfo>,
    /// 차트 패턴 타입
    pub chart_patterns: Vec<PatternTypeInfo>,
}

// ==================== 핸들러 ====================

/// 캔들스틱 패턴 감지.
///
/// GET /api/v1/patterns/candlestick?symbol=BTC/USDT&timeframe=1h&limit=100
#[utoipa::path(
    get,
    path = "/api/v1/patterns/candlestick",
    tag = "patterns",
    params(PatternQuery),
    responses(
        (status = 200, description = "캔들스틱 패턴 감지 성공", body = CandlestickPatternsResponse)
    )
)]
async fn get_candlestick_patterns(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PatternQuery>,
) -> Json<CandlestickPatternsResponse> {
    // AppState의 ML 서비스 사용
    let ml_service = state.ml_service.read().await;

    // Kline 데이터 조회
    let klines = fetch_klines(&state, &query.symbol, &query.timeframe, query.limit).await;

    // 패턴 감지
    let detection = ml_service.detect_patterns(&query.symbol, &query.timeframe, &klines);

    // 최소 신뢰도 필터링
    let filtered_patterns: Vec<CandlestickPatternInfo> = detection
        .candlestick_patterns
        .into_iter()
        .filter(|p| p.confidence >= query.min_confidence as f32)
        .collect();

    Json(CandlestickPatternsResponse {
        symbol: query.symbol,
        timeframe: query.timeframe,
        candles_analyzed: klines.len(),
        patterns_found: filtered_patterns.len(),
        patterns: filtered_patterns,
    })
}

/// 차트 패턴 감지.
///
/// GET /api/v1/patterns/chart?symbol=BTC/USDT&timeframe=1h&limit=100
#[utoipa::path(
    get,
    path = "/api/v1/patterns/chart",
    tag = "patterns",
    params(PatternQuery),
    responses(
        (status = 200, description = "차트 패턴 감지 성공", body = ChartPatternsResponse)
    )
)]
async fn get_chart_patterns(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PatternQuery>,
) -> Json<ChartPatternsResponse> {
    // AppState의 ML 서비스 사용
    let ml_service = state.ml_service.read().await;

    // Kline 데이터 조회
    let klines = fetch_klines(&state, &query.symbol, &query.timeframe, query.limit).await;

    // 패턴 감지
    let detection = ml_service.detect_patterns(&query.symbol, &query.timeframe, &klines);

    // 최소 신뢰도 필터링
    let filtered_patterns: Vec<ChartPatternInfo> = detection
        .chart_patterns
        .into_iter()
        .filter(|p| p.confidence >= query.min_confidence as f32)
        .collect();

    Json(ChartPatternsResponse {
        symbol: query.symbol,
        timeframe: query.timeframe,
        candles_analyzed: klines.len(),
        patterns_found: filtered_patterns.len(),
        patterns: filtered_patterns,
    })
}

/// 모든 패턴 감지 (캔들스틱 + 차트).
///
/// GET /api/v1/patterns/detect?symbol=BTC/USDT&timeframe=1h&limit=100
#[utoipa::path(
    get,
    path = "/api/v1/patterns/detect",
    tag = "patterns",
    params(PatternQuery),
    responses(
        (status = 200, description = "모든 패턴 감지 성공", body = PatternDetectionResult)
    )
)]
async fn detect_all_patterns(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PatternQuery>,
) -> Json<PatternDetectionResult> {
    // AppState의 ML 서비스 사용
    let ml_service = state.ml_service.read().await;

    // Kline 데이터 조회
    let klines = fetch_klines(&state, &query.symbol, &query.timeframe, query.limit).await;

    // 패턴 감지
    let mut detection = ml_service.detect_patterns(&query.symbol, &query.timeframe, &klines);

    // 최소 신뢰도 필터링
    detection
        .candlestick_patterns
        .retain(|p| p.confidence >= query.min_confidence as f32);

    detection
        .chart_patterns
        .retain(|p| p.confidence >= query.min_confidence as f32);

    Json(detection)
}

/// 지원되는 패턴 타입 목록.
///
/// GET /api/v1/patterns/types
#[utoipa::path(
    get,
    path = "/api/v1/patterns/types",
    tag = "patterns",
    responses(
        (status = 200, description = "패턴 타입 목록 조회 성공", body = PatternTypesResponse)
    )
)]
async fn get_pattern_types() -> Json<PatternTypesResponse> {
    let candlestick_patterns = vec![
        // 단일 캔들 (Bullish)
        PatternTypeInfo {
            id: "doji".to_string(),
            name: "도지".to_string(),
            category: "단일 캔들".to_string(),
            signal: "neutral".to_string(),
            description: "시가와 종가가 거의 같은 중립 패턴".to_string(),
        },
        PatternTypeInfo {
            id: "dragonfly_doji".to_string(),
            name: "잠자리 도지".to_string(),
            category: "단일 캔들".to_string(),
            signal: "bullish".to_string(),
            description: "긴 아래꼬리를 가진 도지, 상승 반전 신호".to_string(),
        },
        PatternTypeInfo {
            id: "gravestone_doji".to_string(),
            name: "비석 도지".to_string(),
            category: "단일 캔들".to_string(),
            signal: "bearish".to_string(),
            description: "긴 위꼬리를 가진 도지, 하락 반전 신호".to_string(),
        },
        PatternTypeInfo {
            id: "hammer".to_string(),
            name: "해머".to_string(),
            category: "단일 캔들".to_string(),
            signal: "bullish".to_string(),
            description: "하락 추세에서 긴 아래꼬리, 상승 반전 신호".to_string(),
        },
        PatternTypeInfo {
            id: "inverted_hammer".to_string(),
            name: "역해머".to_string(),
            category: "단일 캔들".to_string(),
            signal: "bullish".to_string(),
            description: "하락 추세에서 긴 위꼬리, 상승 반전 가능".to_string(),
        },
        PatternTypeInfo {
            id: "bullish_marubozu".to_string(),
            name: "양봉 장대".to_string(),
            category: "단일 캔들".to_string(),
            signal: "bullish".to_string(),
            description: "꼬리 없는 강한 양봉, 매수세 강함".to_string(),
        },
        PatternTypeInfo {
            id: "bearish_marubozu".to_string(),
            name: "음봉 장대".to_string(),
            category: "단일 캔들".to_string(),
            signal: "bearish".to_string(),
            description: "꼬리 없는 강한 음봉, 매도세 강함".to_string(),
        },
        PatternTypeInfo {
            id: "shooting_star".to_string(),
            name: "유성형".to_string(),
            category: "단일 캔들".to_string(),
            signal: "bearish".to_string(),
            description: "상승 추세에서 긴 위꼬리, 하락 반전 신호".to_string(),
        },
        PatternTypeInfo {
            id: "hanging_man".to_string(),
            name: "교수형".to_string(),
            category: "단일 캔들".to_string(),
            signal: "bearish".to_string(),
            description: "상승 추세에서 긴 아래꼬리, 하락 반전 경고".to_string(),
        },
        // 이중 캔들 (Bullish)
        PatternTypeInfo {
            id: "bullish_engulfing".to_string(),
            name: "상승 장악형".to_string(),
            category: "이중 캔들".to_string(),
            signal: "bullish".to_string(),
            description: "음봉을 완전히 감싸는 양봉, 강한 상승 신호".to_string(),
        },
        PatternTypeInfo {
            id: "bearish_engulfing".to_string(),
            name: "하락 장악형".to_string(),
            category: "이중 캔들".to_string(),
            signal: "bearish".to_string(),
            description: "양봉을 완전히 감싸는 음봉, 강한 하락 신호".to_string(),
        },
        PatternTypeInfo {
            id: "bullish_harami".to_string(),
            name: "상승 잉태형".to_string(),
            category: "이중 캔들".to_string(),
            signal: "bullish".to_string(),
            description: "큰 음봉 안에 작은 양봉, 상승 반전 가능".to_string(),
        },
        PatternTypeInfo {
            id: "bearish_harami".to_string(),
            name: "하락 잉태형".to_string(),
            category: "이중 캔들".to_string(),
            signal: "bearish".to_string(),
            description: "큰 양봉 안에 작은 음봉, 하락 반전 가능".to_string(),
        },
        PatternTypeInfo {
            id: "piercing_line".to_string(),
            name: "관통형".to_string(),
            category: "이중 캔들".to_string(),
            signal: "bullish".to_string(),
            description: "음봉 후 중간 이상 상승하는 양봉, 상승 반전".to_string(),
        },
        PatternTypeInfo {
            id: "dark_cloud_cover".to_string(),
            name: "먹구름형".to_string(),
            category: "이중 캔들".to_string(),
            signal: "bearish".to_string(),
            description: "양봉 후 중간 이하로 하락하는 음봉, 하락 반전".to_string(),
        },
        // 삼중 캔들
        PatternTypeInfo {
            id: "morning_star".to_string(),
            name: "샛별형".to_string(),
            category: "삼중 캔들".to_string(),
            signal: "bullish".to_string(),
            description: "음봉-작은봉-양봉, 강한 상승 반전".to_string(),
        },
        PatternTypeInfo {
            id: "evening_star".to_string(),
            name: "석별형".to_string(),
            category: "삼중 캔들".to_string(),
            signal: "bearish".to_string(),
            description: "양봉-작은봉-음봉, 강한 하락 반전".to_string(),
        },
        PatternTypeInfo {
            id: "three_white_soldiers".to_string(),
            name: "적삼병".to_string(),
            category: "삼중 캔들".to_string(),
            signal: "bullish".to_string(),
            description: "연속 3개 양봉, 강한 상승 추세".to_string(),
        },
        PatternTypeInfo {
            id: "three_black_crows".to_string(),
            name: "흑삼병".to_string(),
            category: "삼중 캔들".to_string(),
            signal: "bearish".to_string(),
            description: "연속 3개 음봉, 강한 하락 추세".to_string(),
        },
    ];

    let chart_patterns = vec![
        // 반전 패턴
        PatternTypeInfo {
            id: "head_and_shoulders".to_string(),
            name: "머리어깨형".to_string(),
            category: "반전 패턴".to_string(),
            signal: "bearish".to_string(),
            description: "세 개의 고점으로 구성, 강한 하락 반전 신호".to_string(),
        },
        PatternTypeInfo {
            id: "inverse_head_and_shoulders".to_string(),
            name: "역머리어깨형".to_string(),
            category: "반전 패턴".to_string(),
            signal: "bullish".to_string(),
            description: "세 개의 저점으로 구성, 강한 상승 반전 신호".to_string(),
        },
        PatternTypeInfo {
            id: "double_top".to_string(),
            name: "이중 천장".to_string(),
            category: "반전 패턴".to_string(),
            signal: "bearish".to_string(),
            description: "두 개의 비슷한 고점, 하락 반전 신호".to_string(),
        },
        PatternTypeInfo {
            id: "double_bottom".to_string(),
            name: "이중 바닥".to_string(),
            category: "반전 패턴".to_string(),
            signal: "bullish".to_string(),
            description: "두 개의 비슷한 저점, 상승 반전 신호".to_string(),
        },
        // 지속 패턴
        PatternTypeInfo {
            id: "ascending_triangle".to_string(),
            name: "상승 삼각형".to_string(),
            category: "지속 패턴".to_string(),
            signal: "bullish".to_string(),
            description: "수평 저항선과 상승하는 지지선".to_string(),
        },
        PatternTypeInfo {
            id: "descending_triangle".to_string(),
            name: "하락 삼각형".to_string(),
            category: "지속 패턴".to_string(),
            signal: "bearish".to_string(),
            description: "수평 지지선과 하락하는 저항선".to_string(),
        },
        PatternTypeInfo {
            id: "symmetrical_triangle".to_string(),
            name: "대칭 삼각형".to_string(),
            category: "지속 패턴".to_string(),
            signal: "neutral".to_string(),
            description: "수렴하는 지지/저항선, 돌파 방향 주목".to_string(),
        },
        PatternTypeInfo {
            id: "bullish_flag".to_string(),
            name: "상승 깃발형".to_string(),
            category: "지속 패턴".to_string(),
            signal: "bullish".to_string(),
            description: "급등 후 하락 조정, 상승 지속 신호".to_string(),
        },
        PatternTypeInfo {
            id: "bearish_flag".to_string(),
            name: "하락 깃발형".to_string(),
            category: "지속 패턴".to_string(),
            signal: "bearish".to_string(),
            description: "급락 후 상승 조정, 하락 지속 신호".to_string(),
        },
        // 채널
        PatternTypeInfo {
            id: "ascending_channel".to_string(),
            name: "상승 채널".to_string(),
            category: "채널".to_string(),
            signal: "bullish".to_string(),
            description: "평행한 상승 추세선 사이의 가격 움직임".to_string(),
        },
        PatternTypeInfo {
            id: "descending_channel".to_string(),
            name: "하락 채널".to_string(),
            category: "채널".to_string(),
            signal: "bearish".to_string(),
            description: "평행한 하락 추세선 사이의 가격 움직임".to_string(),
        },
        PatternTypeInfo {
            id: "horizontal_channel".to_string(),
            name: "수평 채널 (박스권)".to_string(),
            category: "채널".to_string(),
            signal: "neutral".to_string(),
            description: "수평 지지/저항선 사이의 횡보 움직임".to_string(),
        },
        // 기타
        PatternTypeInfo {
            id: "cup_and_handle".to_string(),
            name: "컵앤핸들".to_string(),
            category: "기타".to_string(),
            signal: "bullish".to_string(),
            description: "U자형 컵과 작은 조정, 강한 상승 신호".to_string(),
        },
    ];

    Json(PatternTypesResponse {
        candlestick_patterns,
        chart_patterns,
    })
}

// ==================== 라우터 ====================

/// 패턴 인식 라우터 생성.
pub fn patterns_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/candlestick", get(get_candlestick_patterns))
        .route("/chart", get(get_chart_patterns))
        .route("/detect", get(detect_all_patterns))
        .route("/types", get(get_pattern_types))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_query_defaults() {
        // serde를 사용한 기본값 테스트
        let query: PatternQuery = serde_json::from_str(r#"{"symbol": "BTC/USDT"}"#).unwrap();
        assert_eq!(query.symbol, "BTC/USDT");
        assert_eq!(query.timeframe, "1h");
        assert_eq!(query.limit, 100);
        assert!((query.min_confidence - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_pattern_types_response() {
        // 패턴 타입 정보 구조체 테스트
        let info = PatternTypeInfo {
            id: "hammer".to_string(),
            name: "해머".to_string(),
            category: "단일 캔들".to_string(),
            signal: "bullish".to_string(),
            description: "Test description".to_string(),
        };
        assert_eq!(info.signal, "bullish");
    }
}
