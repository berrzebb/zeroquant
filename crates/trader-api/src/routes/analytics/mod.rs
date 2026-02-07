//! 포트폴리오 analytics endpoint.
//!
//! 포트폴리오 분석 데이터를 제공하는 REST API입니다.
//!
//! # 엔드포인트
//!
//! ## 포트폴리오 분석
//! - `GET /api/v1/analytics/performance` - 성과 요약
//! - `GET /api/v1/analytics/equity-curve` - 자산 곡선 데이터
//! - `GET /api/v1/analytics/charts/cagr` - CAGR 추이 차트
//! - `GET /api/v1/analytics/charts/mdd` - MDD 추이 차트
//! - `GET /api/v1/analytics/monthly-returns` - 월별 수익률
//!
//! ## 기술적 지표
//! - `GET /api/v1/analytics/indicators` - 사용 가능한 지표 목록
//! - `GET /api/v1/analytics/indicators/sma` - 단순 이동평균
//! - `GET /api/v1/analytics/indicators/ema` - 지수 이동평균
//! - `GET /api/v1/analytics/indicators/rsi` - RSI
//! - `GET /api/v1/analytics/indicators/macd` - MACD
//! - `GET /api/v1/analytics/indicators/bollinger` - 볼린저 밴드
//! - `GET /api/v1/analytics/indicators/stochastic` - 스토캐스틱
//! - `GET /api/v1/analytics/indicators/atr` - ATR
//! - `POST /api/v1/analytics/indicators/calculate` - 다중 지표 계산

pub mod charts;
pub mod indicators;
pub mod manager;
pub mod performance;
pub mod sync;
pub mod types;

// Re-export types
pub use types::{
    ChartQuery, ChartResponse, EquityCurveResponse, MonthlyReturnsResponse, PerformanceResponse,
    PeriodQuery,
};

// Re-export manager
pub use manager::AnalyticsManager;

use axum::{routing::get, Router};
use std::sync::Arc;

use crate::state::AppState;

use charts::{
    get_cagr_chart, get_drawdown_chart, get_equity_curve, get_mdd_chart, get_monthly_returns,
};
use indicators::{
    calculate_indicators, get_atr_indicator, get_available_indicators, get_bollinger_indicator,
    get_correlation, get_ema_indicator, get_keltner_indicator, get_macd_indicator,
    get_obv_indicator, get_rsi_indicator, get_sma_indicator, get_stochastic_indicator,
    get_supertrend_indicator, get_volume_profile, get_vwap_indicator,
};
use performance::get_performance;
use sync::{clear_equity_cache, sync_equity_curve};

/// 포트폴리오 분석 라우터 생성.
pub fn analytics_router() -> Router<Arc<AppState>> {
    Router::new()
        // 포트폴리오 분석 엔드포인트
        .route("/performance", get(get_performance))
        .route("/equity-curve", get(get_equity_curve))
        .route("/charts/cagr", get(get_cagr_chart))
        .route("/charts/mdd", get(get_mdd_chart))
        .route("/charts/drawdown", get(get_drawdown_chart))
        .route("/monthly-returns", get(get_monthly_returns))
        // 자산 곡선 동기화
        .route("/sync-equity", axum::routing::post(sync_equity_curve))
        // 자산 곡선 캐시 삭제
        .route("/equity-cache", axum::routing::delete(clear_equity_cache))
        // 기술적 지표 엔드포인트
        .route("/indicators", get(get_available_indicators))
        .route("/indicators/sma", get(get_sma_indicator))
        .route("/indicators/ema", get(get_ema_indicator))
        .route("/indicators/rsi", get(get_rsi_indicator))
        .route("/indicators/macd", get(get_macd_indicator))
        .route("/indicators/bollinger", get(get_bollinger_indicator))
        .route("/indicators/stochastic", get(get_stochastic_indicator))
        .route("/indicators/atr", get(get_atr_indicator))
        .route(
            "/indicators/calculate",
            axum::routing::post(calculate_indicators),
        )
        .route("/indicators/volume-profile", get(get_volume_profile))
        .route("/indicators/vwap", get(get_vwap_indicator))
        .route("/indicators/keltner", get(get_keltner_indicator))
        .route("/indicators/obv", get(get_obv_indicator))
        .route("/indicators/supertrend", get(get_supertrend_indicator))
        .route("/correlation", get(get_correlation))
}
