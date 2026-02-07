//! 포트폴리오 차트 핸들러.
//!
//! 자산 곡선, CAGR, MDD, Drawdown, 월별 수익률 차트 API를 제공합니다.

use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Datelike, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use std::sync::Arc;
use tracing::{debug, warn};

use crate::repository::EquityHistoryRepository;
use crate::state::AppState;

use super::manager::AnalyticsManager;
use super::performance::parse_period_duration;
use super::types::{
    ChartPointResponse, ChartQuery, ChartResponse, EquityCurveResponse, MonthlyReturnCellResponse,
    MonthlyReturnsResponse, PeriodQuery,
};

/// 자산 곡선 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/equity-curve",
    tag = "analytics",
    params(PeriodQuery),
    responses(
        (status = 200, description = "자산 곡선 조회 성공", body = EquityCurveResponse)
    )
)]
pub async fn get_equity_curve(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PeriodQuery>,
) -> impl IntoResponse {
    let duration = parse_period_duration(&query.period);
    let start_time = Utc::now() - duration;
    let end_time = Utc::now();

    // credential_id 파싱
    let credential_id = query
        .credential_id
        .as_ref()
        .and_then(|id| uuid::Uuid::parse_str(id).ok());

    // DB에서 실제 데이터 조회 시도
    if let Some(db_pool) = &state.db_pool {
        // credential_id가 있으면 특정 계좌만 조회, 없으면 전체 합산
        let data_result = if let Some(cred_id) = credential_id {
            debug!(credential_id = %cred_id, "특정 계좌 자산 곡선 조회");
            EquityHistoryRepository::get_equity_curve(db_pool, cred_id, start_time, end_time).await
        } else {
            debug!("전체 계좌 통합 자산 곡선 조회");
            EquityHistoryRepository::get_aggregated_equity_curve(db_pool, start_time, end_time)
                .await
        };

        match data_result {
            Ok(data) if !data.is_empty() => {
                debug!("DB에서 {} 개의 자산 곡선 포인트 로드됨", data.len());

                let filtered: Vec<ChartPointResponse> = data
                    .iter()
                    .map(|p| ChartPointResponse {
                        x: p.timestamp.timestamp_millis(),
                        y: p.equity.to_string(),
                        label: None,
                    })
                    .collect();

                let (start_str, end_str) = match (filtered.first(), filtered.last()) {
                    (Some(first), Some(last)) => {
                        let start = DateTime::from_timestamp_millis(first.x).unwrap_or(Utc::now());
                        let end = DateTime::from_timestamp_millis(last.x).unwrap_or(Utc::now());
                        (start.to_rfc3339(), end.to_rfc3339())
                    }
                    _ => (Utc::now().to_rfc3339(), Utc::now().to_rfc3339()),
                };

                return Json(EquityCurveResponse {
                    count: filtered.len(),
                    data: filtered,
                    period: query.period,
                    start_time: start_str,
                    end_time: end_str,
                });
            }
            Ok(_) => {
                // credential_id가 명시된 경우: 해당 계좌에 데이터 없음 → 빈 응답 반환 (샘플 데이터 사용 안 함)
                if credential_id.is_some() {
                    debug!("특정 계좌에 자산 곡선 데이터 없음, 빈 응답 반환");
                    return Json(EquityCurveResponse {
                        count: 0,
                        data: Vec::new(),
                        period: query.period,
                        start_time: Utc::now().to_rfc3339(),
                        end_time: Utc::now().to_rfc3339(),
                    });
                }
                debug!("DB에 자산 곡선 데이터 없음, 샘플 데이터 사용");
            }
            Err(e) => {
                warn!("자산 곡선 데이터 조회 실패: {}", e);
            }
        }
    }

    // Fallback: 샘플 데이터 (credential_id 없이 통합 조회에서도 데이터 없을 때만)
    let mut manager = AnalyticsManager::default();
    manager.load_sample_data();

    let data = manager.get_equity_curve_data();
    let cutoff = Utc::now() - duration;

    // 기간 필터링
    let filtered: Vec<ChartPointResponse> = data
        .iter()
        .filter(|p| {
            let ts = DateTime::from_timestamp_millis(p.x).unwrap_or(Utc::now());
            ts >= cutoff
        })
        .map(ChartPointResponse::from)
        .collect();

    let (start_str, end_str) = match (filtered.first(), filtered.last()) {
        (Some(first), Some(last)) => {
            let start = DateTime::from_timestamp_millis(first.x).unwrap_or(Utc::now());
            let end = DateTime::from_timestamp_millis(last.x).unwrap_or(Utc::now());
            (start.to_rfc3339(), end.to_rfc3339())
        }
        _ => (Utc::now().to_rfc3339(), Utc::now().to_rfc3339()),
    };

    Json(EquityCurveResponse {
        count: filtered.len(),
        data: filtered,
        period: query.period,
        start_time: start_str,
        end_time: end_str,
    })
}

/// CAGR 차트 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/charts/cagr",
    tag = "analytics",
    params(ChartQuery),
    responses(
        (status = 200, description = "CAGR 차트 조회 성공", body = ChartResponse)
    )
)]
pub async fn get_cagr_chart(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<ChartQuery>,
) -> impl IntoResponse {
    let mut manager = AnalyticsManager::default();
    manager.load_sample_data();

    let charts = manager.get_charts(query.window_days);
    let duration = parse_period_duration(&query.period);
    let cutoff = Utc::now() - duration;

    let filtered: Vec<ChartPointResponse> = charts
        .rolling_cagr
        .iter()
        .filter(|p| {
            let ts = DateTime::from_timestamp_millis(p.x).unwrap_or(Utc::now());
            ts >= cutoff
        })
        .map(ChartPointResponse::from)
        .collect();

    Json(ChartResponse {
        name: "Rolling CAGR".to_string(),
        count: filtered.len(),
        data: filtered,
        period: query.period,
    })
}

/// MDD 차트 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/charts/mdd",
    tag = "analytics",
    params(ChartQuery),
    responses(
        (status = 200, description = "MDD 차트 조회 성공", body = ChartResponse)
    )
)]
pub async fn get_mdd_chart(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<ChartQuery>,
) -> impl IntoResponse {
    let mut manager = AnalyticsManager::default();
    manager.load_sample_data();

    let charts = manager.get_charts(query.window_days);
    let duration = parse_period_duration(&query.period);
    let cutoff = Utc::now() - duration;

    let filtered: Vec<ChartPointResponse> = charts
        .rolling_mdd
        .iter()
        .filter(|p| {
            let ts = DateTime::from_timestamp_millis(p.x).unwrap_or(Utc::now());
            ts >= cutoff
        })
        .map(ChartPointResponse::from)
        .collect();

    Json(ChartResponse {
        name: "Rolling MDD".to_string(),
        count: filtered.len(),
        data: filtered,
        period: query.period,
    })
}

/// Drawdown 차트 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/charts/drawdown",
    tag = "analytics",
    params(ChartQuery),
    responses(
        (status = 200, description = "Drawdown 차트 조회 성공", body = ChartResponse)
    )
)]
pub async fn get_drawdown_chart(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<ChartQuery>,
) -> impl IntoResponse {
    let mut manager = AnalyticsManager::default();
    manager.load_sample_data();

    let charts = manager.get_charts(query.window_days);
    let duration = parse_period_duration(&query.period);
    let cutoff = Utc::now() - duration;

    let filtered: Vec<ChartPointResponse> = charts
        .drawdown_curve
        .iter()
        .filter(|p| {
            let ts = DateTime::from_timestamp_millis(p.x).unwrap_or(Utc::now());
            ts >= cutoff
        })
        .map(ChartPointResponse::from)
        .collect();

    Json(ChartResponse {
        name: "Drawdown".to_string(),
        count: filtered.len(),
        data: filtered,
        period: query.period,
    })
}

/// 월별 수익률 히트맵 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/monthly-returns",
    tag = "analytics",
    responses(
        (status = 200, description = "월별 수익률 조회 성공", body = MonthlyReturnsResponse)
    )
)]
pub async fn get_monthly_returns(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // DB에서 실제 데이터 조회 시도
    if let Some(db_pool) = &state.db_pool {
        match EquityHistoryRepository::get_monthly_returns(db_pool, None, 3).await {
            Ok(monthly_data) if !monthly_data.is_empty() => {
                debug!(
                    "DB에서 {} 개의 월별 수익률 데이터 로드됨",
                    monthly_data.len()
                );

                // 강도(intensity) 계산을 위한 최대/최소값 찾기
                let max_return = monthly_data
                    .iter()
                    .map(|m| m.return_pct.abs())
                    .max()
                    .unwrap_or(dec!(10));

                let data: Vec<MonthlyReturnCellResponse> = monthly_data
                    .iter()
                    .map(|m| {
                        let intensity = if max_return > rust_decimal::Decimal::ZERO {
                            (m.return_pct / max_return).to_f64().unwrap_or(0.0)
                        } else {
                            0.0
                        };

                        MonthlyReturnCellResponse {
                            year: m.year,
                            month: m.month,
                            return_pct: m.return_pct.to_string(),
                            intensity,
                        }
                    })
                    .collect();

                let (min_year, max_year) = if data.is_empty() {
                    (Utc::now().year(), Utc::now().year())
                } else {
                    let min = data.iter().map(|c| c.year).min().unwrap();
                    let max = data.iter().map(|c| c.year).max().unwrap();
                    (min, max)
                };

                return Json(MonthlyReturnsResponse {
                    count: data.len(),
                    data,
                    year_range: (min_year, max_year),
                });
            }
            Ok(_) => {
                debug!("DB에 월별 수익률 데이터 없음, 샘플 데이터 사용");
            }
            Err(e) => {
                warn!("월별 수익률 데이터 조회 실패: {}", e);
            }
        }
    }

    // Fallback: 샘플 데이터
    let mut manager = AnalyticsManager::default();
    manager.load_sample_data();

    let charts = manager.get_charts(365);

    let data: Vec<MonthlyReturnCellResponse> = charts
        .monthly_returns
        .iter()
        .map(MonthlyReturnCellResponse::from)
        .collect();

    let (min_year, max_year) = if data.is_empty() {
        (Utc::now().year(), Utc::now().year())
    } else {
        let min = data.iter().map(|c| c.year).min().unwrap();
        let max = data.iter().map(|c| c.year).max().unwrap();
        (min, max)
    };

    Json(MonthlyReturnsResponse {
        count: data.len(),
        data,
        year_range: (min_year, max_year),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::get, Router};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_get_equity_curve_endpoint() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/equity-curve", get(get_equity_curve))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/equity-curve?period=3m")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let curve: EquityCurveResponse = serde_json::from_slice(&body).unwrap();

        assert!(!curve.data.is_empty());
        assert_eq!(curve.period, "3m");
    }

    #[tokio::test]
    async fn test_get_monthly_returns_endpoint() {
        use crate::state::create_test_state;

        let state = Arc::new(create_test_state());
        let app = Router::new()
            .route("/monthly-returns", get(get_monthly_returns))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/monthly-returns")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let monthly: MonthlyReturnsResponse = serde_json::from_slice(&body).unwrap();

        assert!(!monthly.data.is_empty());
    }
}
