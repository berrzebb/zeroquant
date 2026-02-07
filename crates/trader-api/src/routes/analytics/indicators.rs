//! 기술적 지표 핸들러.
//!
//! SMA, EMA, RSI, MACD, 볼린저 밴드, 스토캐스틱, ATR 등의 지표 API를 제공합니다.

use axum::{extract::Query, response::IntoResponse, Json};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use trader_analytics::{
    AtrParams, BollingerBandsParams, EmaParams, IndicatorEngine, KeltnerChannelParams, MacdParams,
    ObvParams, RsiParams, SmaParams, StochasticParams, SuperTrendParams, VwapParams,
};

use super::types::{
    AtrQuery, AvailableIndicatorsResponse, BollingerQuery, CalculateIndicatorsRequest,
    CalculateIndicatorsResponse, EmaQuery, IndicatorDataResponse, IndicatorInfo, IndicatorPoint,
    IndicatorSeries, KeltnerParamsResponse, KeltnerPointResponse, KeltnerQuery, KeltnerResponse,
    MacdQuery, ObvPointResponse, ObvQuery, ObvResponse, RsiQuery, SmaQuery, StochasticQuery,
    SuperTrendParamsResponse, SuperTrendPointResponse, SuperTrendQuery, SuperTrendResponse,
    VwapParamsResponse, VwapPointResponse, VwapQuery, VwapResponse,
};

/// 사용 가능한 지표 목록 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/indicators",
    tag = "analytics",
    responses(
        (status = 200, description = "지표 목록 조회 성공", body = AvailableIndicatorsResponse)
    )
)]
pub async fn get_available_indicators() -> impl IntoResponse {
    let indicators = vec![
        IndicatorInfo {
            id: "sma".to_string(),
            name: "단순 이동평균 (SMA)".to_string(),
            description: "지정된 기간 동안의 종가 평균을 계산합니다.".to_string(),
            category: "추세".to_string(),
            default_params: serde_json::json!({ "period": 20 }),
            overlay: true,
        },
        IndicatorInfo {
            id: "ema".to_string(),
            name: "지수 이동평균 (EMA)".to_string(),
            description: "최근 가격에 더 큰 가중치를 부여하는 이동평균입니다.".to_string(),
            category: "추세".to_string(),
            default_params: serde_json::json!({ "period": 12 }),
            overlay: true,
        },
        IndicatorInfo {
            id: "rsi".to_string(),
            name: "상대강도지수 (RSI)".to_string(),
            description: "과매수/과매도 상태를 측정합니다. 70 이상: 과매수, 30 이하: 과매도."
                .to_string(),
            category: "모멘텀".to_string(),
            default_params: serde_json::json!({ "period": 14 }),
            overlay: false,
        },
        IndicatorInfo {
            id: "macd".to_string(),
            name: "MACD".to_string(),
            description: "두 EMA의 차이로 추세의 강도와 방향을 분석합니다.".to_string(),
            category: "추세".to_string(),
            default_params: serde_json::json!({
                "fast_period": 12,
                "slow_period": 26,
                "signal_period": 9
            }),
            overlay: false,
        },
        IndicatorInfo {
            id: "bollinger".to_string(),
            name: "볼린저 밴드".to_string(),
            description: "이동평균을 중심으로 표준편차 밴드를 그려 변동성을 시각화합니다."
                .to_string(),
            category: "변동성".to_string(),
            default_params: serde_json::json!({ "period": 20, "std_dev": 2.0 }),
            overlay: true,
        },
        IndicatorInfo {
            id: "stochastic".to_string(),
            name: "스토캐스틱".to_string(),
            description: "현재 가격이 일정 기간 가격 범위 내에서 어디에 위치하는지 측정합니다."
                .to_string(),
            category: "모멘텀".to_string(),
            default_params: serde_json::json!({ "k_period": 14, "d_period": 3 }),
            overlay: false,
        },
        IndicatorInfo {
            id: "atr".to_string(),
            name: "평균 실제 범위 (ATR)".to_string(),
            description: "가격 변동성을 측정합니다. 값이 클수록 변동성이 높습니다.".to_string(),
            category: "변동성".to_string(),
            default_params: serde_json::json!({ "period": 14 }),
            overlay: false,
        },
        IndicatorInfo {
            id: "vwap".to_string(),
            name: "거래량 가중 평균가 (VWAP)".to_string(),
            description: "거래량을 가중치로 사용한 평균 가격입니다. 기관 매매 기준가로 활용됩니다."
                .to_string(),
            category: "거래량".to_string(),
            default_params: serde_json::json!({ "band_multiplier": 2.0, "reset_daily": false }),
            overlay: true,
        },
        IndicatorInfo {
            id: "keltner".to_string(),
            name: "켈트너 채널".to_string(),
            description: "EMA를 중심으로 ATR 기반 밴드를 그립니다. TTM Squeeze와 함께 사용됩니다."
                .to_string(),
            category: "변동성".to_string(),
            default_params: serde_json::json!({ "ema_period": 20, "atr_multiplier": 2.0 }),
            overlay: true,
        },
        IndicatorInfo {
            id: "obv".to_string(),
            name: "OBV (On-Balance Volume)".to_string(),
            description:
                "거래량 기반 스마트 머니 추적 지표입니다. 가격과 OBV의 다이버전스를 활용합니다."
                    .to_string(),
            category: "거래량".to_string(),
            default_params: serde_json::json!({}),
            overlay: false,
        },
        IndicatorInfo {
            id: "supertrend".to_string(),
            name: "SuperTrend".to_string(),
            description: "ATR 기반 추세 추종 지표입니다. 명확한 매수/매도 시그널을 제공합니다."
                .to_string(),
            category: "추세".to_string(),
            default_params: serde_json::json!({ "atr_period": 10, "multiplier": 3.0 }),
            overlay: true,
        },
    ];

    Json(AvailableIndicatorsResponse { indicators })
}

/// 샘플 OHLCV 데이터 생성 (테스트용).
#[allow(clippy::type_complexity)]
fn generate_sample_ohlcv(
    days: i64,
) -> (
    Vec<i64>,
    Vec<Decimal>,
    Vec<Decimal>,
    Vec<Decimal>,
    Vec<Decimal>,
    Vec<Decimal>,
) {
    let base_time = Utc::now() - Duration::days(days);
    let mut timestamps = Vec::with_capacity(days as usize);
    let mut opens = Vec::with_capacity(days as usize);
    let mut highs = Vec::with_capacity(days as usize);
    let mut lows = Vec::with_capacity(days as usize);
    let mut closes = Vec::with_capacity(days as usize);
    let mut volumes = Vec::with_capacity(days as usize);

    let mut price = dec!(50000); // 시작 가격

    for i in 0..days {
        let ts = (base_time + Duration::days(i)).timestamp_millis();
        timestamps.push(ts);

        // 변동성 있는 가격 생성
        let change_pct = if i % 5 == 0 {
            dec!(-0.02)
        } else if i % 3 == 0 {
            dec!(0.015)
        } else {
            dec!(0.005)
        };

        let open = price;
        let close = price * (dec!(1.0) + change_pct);
        let high = if close > open {
            close * dec!(1.005)
        } else {
            open * dec!(1.005)
        };
        let low = if close < open {
            close * dec!(0.995)
        } else {
            open * dec!(0.995)
        };

        // 거래량 생성 (가격 변동 크기에 따라 변동)
        let base_volume = dec!(1000000);
        let volume_multiplier = if change_pct.abs() > dec!(0.01) {
            dec!(1.5)
        } else {
            dec!(1.0)
        };
        let volume = base_volume * volume_multiplier;

        opens.push(open);
        highs.push(high);
        lows.push(low);
        closes.push(close);
        volumes.push(volume);

        price = close;
    }

    (timestamps, opens, highs, lows, closes, volumes)
}

/// 기간 문자열을 일수로 변환.
fn parse_period_to_days(period: &str) -> i64 {
    match period.to_lowercase().as_str() {
        "1d" => 1,
        "1w" => 7,
        "1m" => 30,
        "3m" => 90,
        "6m" => 180,
        "1y" | "12m" => 365,
        "all" => 1000,
        _ => 90, // 기본값: 3개월
    }
}

/// SMA 지표 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/indicators/sma",
    tag = "analytics",
    params(SmaQuery),
    responses(
        (status = 200, description = "SMA 데이터 조회 성공", body = IndicatorDataResponse)
    )
)]
pub async fn get_sma_indicator(Query(query): Query<SmaQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes, _) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = SmaParams {
        period: query.sma_period,
    };

    match engine.sma(&closes, params) {
        Ok(sma_values) => {
            let data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(sma_values.iter())
                .map(|(&ts, value)| IndicatorPoint {
                    x: ts,
                    y: value.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "sma".to_string(),
                name: format!("SMA({})", query.sma_period),
                symbol: query.symbol,
                params: serde_json::json!({ "period": query.sma_period }),
                series: vec![IndicatorSeries {
                    name: "sma".to_string(),
                    data,
                    color: Some("#2196F3".to_string()),
                    series_type: "line".to_string(),
                }],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "sma".to_string(),
            name: format!("SMA({}) - 오류", query.sma_period),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// EMA 지표 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/indicators/ema",
    tag = "analytics",
    params(EmaQuery),
    responses(
        (status = 200, description = "EMA 데이터 조회 성공", body = IndicatorDataResponse)
    )
)]
pub async fn get_ema_indicator(Query(query): Query<EmaQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes, _) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = EmaParams {
        period: query.ema_period,
    };

    match engine.ema(&closes, params) {
        Ok(ema_values) => {
            let data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(ema_values.iter())
                .map(|(&ts, value)| IndicatorPoint {
                    x: ts,
                    y: value.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "ema".to_string(),
                name: format!("EMA({})", query.ema_period),
                symbol: query.symbol,
                params: serde_json::json!({ "period": query.ema_period }),
                series: vec![IndicatorSeries {
                    name: "ema".to_string(),
                    data,
                    color: Some("#FF9800".to_string()),
                    series_type: "line".to_string(),
                }],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "ema".to_string(),
            name: format!("EMA({}) - 오류", query.ema_period),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// RSI 지표 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/indicators/rsi",
    tag = "analytics",
    params(RsiQuery),
    responses(
        (status = 200, description = "RSI 데이터 조회 성공", body = IndicatorDataResponse)
    )
)]
pub async fn get_rsi_indicator(Query(query): Query<RsiQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes, _) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = RsiParams {
        period: query.rsi_period,
    };

    match engine.rsi(&closes, params) {
        Ok(rsi_values) => {
            let data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(rsi_values.iter())
                .map(|(&ts, value)| IndicatorPoint {
                    x: ts,
                    y: value.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "rsi".to_string(),
                name: format!("RSI({})", query.rsi_period),
                symbol: query.symbol,
                params: serde_json::json!({ "period": query.rsi_period }),
                series: vec![IndicatorSeries {
                    name: "rsi".to_string(),
                    data,
                    color: Some("#9C27B0".to_string()),
                    series_type: "line".to_string(),
                }],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "rsi".to_string(),
            name: format!("RSI({}) - 오류", query.rsi_period),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// MACD 지표 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/indicators/macd",
    tag = "analytics",
    params(MacdQuery),
    responses(
        (status = 200, description = "MACD 데이터 조회 성공", body = IndicatorDataResponse)
    )
)]
pub async fn get_macd_indicator(Query(query): Query<MacdQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes, _) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = MacdParams {
        fast_period: query.fast_period,
        slow_period: query.slow_period,
        signal_period: query.signal_period,
    };

    match engine.macd(&closes, params) {
        Ok(macd_results) => {
            let macd_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(macd_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.macd.map(|v| v.to_string()),
                })
                .collect();

            let signal_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(macd_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.signal.map(|v| v.to_string()),
                })
                .collect();

            let histogram_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(macd_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.histogram.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "macd".to_string(),
                name: format!(
                    "MACD({},{},{})",
                    query.fast_period, query.slow_period, query.signal_period
                ),
                symbol: query.symbol,
                params: serde_json::json!({
                    "fast_period": query.fast_period,
                    "slow_period": query.slow_period,
                    "signal_period": query.signal_period
                }),
                series: vec![
                    IndicatorSeries {
                        name: "macd".to_string(),
                        data: macd_data,
                        color: Some("#2196F3".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "signal".to_string(),
                        data: signal_data,
                        color: Some("#FF5722".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "histogram".to_string(),
                        data: histogram_data,
                        color: Some("#4CAF50".to_string()),
                        series_type: "bar".to_string(),
                    },
                ],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "macd".to_string(),
            name: "MACD - 오류".to_string(),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// 볼린저 밴드 지표 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/indicators/bollinger",
    tag = "analytics",
    params(BollingerQuery),
    responses(
        (status = 200, description = "볼린저 밴드 데이터 조회 성공", body = IndicatorDataResponse)
    )
)]
pub async fn get_bollinger_indicator(Query(query): Query<BollingerQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes, _) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = BollingerBandsParams {
        period: query.bb_period,
        std_dev_multiplier: Decimal::from_f64_retain(query.std_dev).unwrap_or(dec!(2.0)),
    };

    match engine.bollinger_bands(&closes, params) {
        Ok(bb_results) => {
            let upper_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(bb_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.upper.map(|v| v.to_string()),
                })
                .collect();

            let middle_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(bb_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.middle.map(|v| v.to_string()),
                })
                .collect();

            let lower_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(bb_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.lower.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "bollinger".to_string(),
                name: format!("BB({}, {})", query.bb_period, query.std_dev),
                symbol: query.symbol,
                params: serde_json::json!({
                    "period": query.bb_period,
                    "std_dev": query.std_dev
                }),
                series: vec![
                    IndicatorSeries {
                        name: "upper".to_string(),
                        data: upper_data,
                        color: Some("#E91E63".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "middle".to_string(),
                        data: middle_data,
                        color: Some("#9C27B0".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "lower".to_string(),
                        data: lower_data,
                        color: Some("#2196F3".to_string()),
                        series_type: "line".to_string(),
                    },
                ],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "bollinger".to_string(),
            name: "Bollinger Bands - 오류".to_string(),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// 스토캐스틱 지표 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/indicators/stochastic",
    tag = "analytics",
    params(StochasticQuery),
    responses(
        (status = 200, description = "스토캐스틱 데이터 조회 성공", body = IndicatorDataResponse)
    )
)]
pub async fn get_stochastic_indicator(Query(query): Query<StochasticQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, highs, lows, closes, _) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = StochasticParams {
        k_period: query.k_period,
        d_period: query.d_period,
    };

    match engine.stochastic(&highs, &lows, &closes, params) {
        Ok(stoch_results) => {
            let k_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(stoch_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.k.map(|v| v.to_string()),
                })
                .collect();

            let d_data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(stoch_results.iter())
                .map(|(&ts, result)| IndicatorPoint {
                    x: ts,
                    y: result.d.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "stochastic".to_string(),
                name: format!("Stochastic({}, {})", query.k_period, query.d_period),
                symbol: query.symbol,
                params: serde_json::json!({
                    "k_period": query.k_period,
                    "d_period": query.d_period
                }),
                series: vec![
                    IndicatorSeries {
                        name: "%K".to_string(),
                        data: k_data,
                        color: Some("#2196F3".to_string()),
                        series_type: "line".to_string(),
                    },
                    IndicatorSeries {
                        name: "%D".to_string(),
                        data: d_data,
                        color: Some("#FF9800".to_string()),
                        series_type: "line".to_string(),
                    },
                ],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "stochastic".to_string(),
            name: "Stochastic - 오류".to_string(),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// ATR 지표 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/indicators/atr",
    tag = "analytics",
    params(AtrQuery),
    responses(
        (status = 200, description = "ATR 데이터 조회 성공", body = IndicatorDataResponse)
    )
)]
pub async fn get_atr_indicator(Query(query): Query<AtrQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, highs, lows, closes, _) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = AtrParams {
        period: query.atr_period,
    };

    match engine.atr(&highs, &lows, &closes, params) {
        Ok(atr_values) => {
            let data: Vec<IndicatorPoint> = timestamps
                .iter()
                .zip(atr_values.iter())
                .map(|(&ts, value)| IndicatorPoint {
                    x: ts,
                    y: value.map(|v| v.to_string()),
                })
                .collect();

            Json(IndicatorDataResponse {
                indicator: "atr".to_string(),
                name: format!("ATR({})", query.atr_period),
                symbol: query.symbol,
                params: serde_json::json!({ "period": query.atr_period }),
                series: vec![IndicatorSeries {
                    name: "atr".to_string(),
                    data,
                    color: Some("#795548".to_string()),
                    series_type: "line".to_string(),
                }],
            })
        }
        Err(e) => Json(IndicatorDataResponse {
            indicator: "atr".to_string(),
            name: format!("ATR({}) - 오류", query.atr_period),
            symbol: query.symbol,
            params: serde_json::json!({ "error": e.to_string() }),
            series: vec![],
        }),
    }
}

/// 다중 지표 계산.
#[utoipa::path(
    post,
    path = "/api/v1/analytics/indicators/calculate",
    tag = "analytics",
    request_body = CalculateIndicatorsRequest,
    responses(
        (status = 200, description = "지표 계산 성공", body = CalculateIndicatorsResponse)
    )
)]
pub async fn calculate_indicators(
    Json(request): Json<CalculateIndicatorsRequest>,
) -> impl IntoResponse {
    let days = parse_period_to_days(&request.period);
    let (timestamps, _, highs, lows, closes, _) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let mut results = Vec::new();

    for config in &request.indicators {
        let indicator_result = match config.indicator_type.as_str() {
            "sma" => {
                let period = config
                    .params
                    .get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize;

                if let Ok(values) = engine.sma(&closes, SmaParams { period }) {
                    let data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(values.iter())
                        .map(|(&ts, v)| IndicatorPoint {
                            x: ts,
                            y: v.map(|d| d.to_string()),
                        })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "sma".to_string(),
                        name: config
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("SMA({})", period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period }),
                        series: vec![IndicatorSeries {
                            name: "sma".to_string(),
                            data,
                            color: config.color.clone().or_else(|| Some("#2196F3".to_string())),
                            series_type: "line".to_string(),
                        }],
                    })
                } else {
                    None
                }
            }
            "ema" => {
                let period = config
                    .params
                    .get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(12) as usize;

                if let Ok(values) = engine.ema(&closes, EmaParams { period }) {
                    let data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(values.iter())
                        .map(|(&ts, v)| IndicatorPoint {
                            x: ts,
                            y: v.map(|d| d.to_string()),
                        })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "ema".to_string(),
                        name: config
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("EMA({})", period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period }),
                        series: vec![IndicatorSeries {
                            name: "ema".to_string(),
                            data,
                            color: config.color.clone().or_else(|| Some("#FF9800".to_string())),
                            series_type: "line".to_string(),
                        }],
                    })
                } else {
                    None
                }
            }
            "rsi" => {
                let period = config
                    .params
                    .get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(14) as usize;

                if let Ok(values) = engine.rsi(&closes, RsiParams { period }) {
                    let data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(values.iter())
                        .map(|(&ts, v)| IndicatorPoint {
                            x: ts,
                            y: v.map(|d| d.to_string()),
                        })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "rsi".to_string(),
                        name: config
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("RSI({})", period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period }),
                        series: vec![IndicatorSeries {
                            name: "rsi".to_string(),
                            data,
                            color: config.color.clone().or_else(|| Some("#9C27B0".to_string())),
                            series_type: "line".to_string(),
                        }],
                    })
                } else {
                    None
                }
            }
            "macd" => {
                let fast = config
                    .params
                    .get("fast_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(12) as usize;
                let slow = config
                    .params
                    .get("slow_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(26) as usize;
                let signal = config
                    .params
                    .get("signal_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(9) as usize;

                if let Ok(macd_results) = engine.macd(
                    &closes,
                    MacdParams {
                        fast_period: fast,
                        slow_period: slow,
                        signal_period: signal,
                    },
                ) {
                    let macd_data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(macd_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.macd.map(|d| d.to_string()),
                        })
                        .collect();
                    let signal_data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(macd_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.signal.map(|d| d.to_string()),
                        })
                        .collect();
                    let hist_data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(macd_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.histogram.map(|d| d.to_string()),
                        })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "macd".to_string(),
                        name: config
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("MACD({},{},{})", fast, slow, signal)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "fast_period": fast, "slow_period": slow, "signal_period": signal }),
                        series: vec![
                            IndicatorSeries {
                                name: "macd".to_string(),
                                data: macd_data,
                                color: Some("#2196F3".to_string()),
                                series_type: "line".to_string(),
                            },
                            IndicatorSeries {
                                name: "signal".to_string(),
                                data: signal_data,
                                color: Some("#FF5722".to_string()),
                                series_type: "line".to_string(),
                            },
                            IndicatorSeries {
                                name: "histogram".to_string(),
                                data: hist_data,
                                color: Some("#4CAF50".to_string()),
                                series_type: "bar".to_string(),
                            },
                        ],
                    })
                } else {
                    None
                }
            }
            "bollinger" => {
                let period = config
                    .params
                    .get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize;
                let std_dev = config
                    .params
                    .get("std_dev")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(2.0);

                if let Ok(bb_results) = engine.bollinger_bands(
                    &closes,
                    BollingerBandsParams {
                        period,
                        std_dev_multiplier: Decimal::from_f64_retain(std_dev).unwrap_or(dec!(2.0)),
                    },
                ) {
                    let upper: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(bb_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.upper.map(|d| d.to_string()),
                        })
                        .collect();
                    let middle: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(bb_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.middle.map(|d| d.to_string()),
                        })
                        .collect();
                    let lower: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(bb_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.lower.map(|d| d.to_string()),
                        })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "bollinger".to_string(),
                        name: config
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("BB({}, {})", period, std_dev)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period, "std_dev": std_dev }),
                        series: vec![
                            IndicatorSeries {
                                name: "upper".to_string(),
                                data: upper,
                                color: Some("#E91E63".to_string()),
                                series_type: "line".to_string(),
                            },
                            IndicatorSeries {
                                name: "middle".to_string(),
                                data: middle,
                                color: Some("#9C27B0".to_string()),
                                series_type: "line".to_string(),
                            },
                            IndicatorSeries {
                                name: "lower".to_string(),
                                data: lower,
                                color: Some("#2196F3".to_string()),
                                series_type: "line".to_string(),
                            },
                        ],
                    })
                } else {
                    None
                }
            }
            "stochastic" => {
                let k_period = config
                    .params
                    .get("k_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(14) as usize;
                let d_period = config
                    .params
                    .get("d_period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(3) as usize;

                if let Ok(stoch_results) = engine.stochastic(
                    &highs,
                    &lows,
                    &closes,
                    StochasticParams { k_period, d_period },
                ) {
                    let k_data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(stoch_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.k.map(|d| d.to_string()),
                        })
                        .collect();
                    let d_data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(stoch_results.iter())
                        .map(|(&ts, r)| IndicatorPoint {
                            x: ts,
                            y: r.d.map(|d| d.to_string()),
                        })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "stochastic".to_string(),
                        name: config
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("Stochastic({}, {})", k_period, d_period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "k_period": k_period, "d_period": d_period }),
                        series: vec![
                            IndicatorSeries {
                                name: "%K".to_string(),
                                data: k_data,
                                color: Some("#2196F3".to_string()),
                                series_type: "line".to_string(),
                            },
                            IndicatorSeries {
                                name: "%D".to_string(),
                                data: d_data,
                                color: Some("#FF9800".to_string()),
                                series_type: "line".to_string(),
                            },
                        ],
                    })
                } else {
                    None
                }
            }
            "atr" => {
                let period = config
                    .params
                    .get("period")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(14) as usize;

                if let Ok(values) = engine.atr(&highs, &lows, &closes, AtrParams { period }) {
                    let data: Vec<IndicatorPoint> = timestamps
                        .iter()
                        .zip(values.iter())
                        .map(|(&ts, v)| IndicatorPoint {
                            x: ts,
                            y: v.map(|d| d.to_string()),
                        })
                        .collect();

                    Some(IndicatorDataResponse {
                        indicator: "atr".to_string(),
                        name: config
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("ATR({})", period)),
                        symbol: request.symbol.clone(),
                        params: serde_json::json!({ "period": period }),
                        series: vec![IndicatorSeries {
                            name: "atr".to_string(),
                            data,
                            color: config.color.clone().or_else(|| Some("#795548".to_string())),
                            series_type: "line".to_string(),
                        }],
                    })
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(result) = indicator_result {
            results.push(result);
        }
    }

    Json(CalculateIndicatorsResponse {
        symbol: request.symbol,
        period: request.period,
        results,
    })
}

/// Volume Profile 계산.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/indicators/volume-profile",
    tag = "analytics",
    params(super::types::VolumeProfileQuery),
    responses(
        (status = 200, description = "Volume Profile 계산 성공", body = super::types::VolumeProfileResponse)
    )
)]
pub async fn get_volume_profile(
    Query(query): Query<super::types::VolumeProfileQuery>,
) -> impl IntoResponse {
    use super::types::{PriceLevelResponse, VolumeProfileResponse};
    use rust_decimal::prelude::ToPrimitive;
    use trader_analytics::VolumeProfileCalculator;
    use trader_core::Kline;

    // OHLCV 데이터 생성 (실제로는 DB에서 조회)
    let days = query.period as i64;
    let (timestamps, opens, highs, lows, closes, volumes) = generate_sample_ohlcv(days);

    // Kline으로 변환
    let klines: Vec<Kline> = timestamps
        .iter()
        .zip(opens.iter())
        .zip(highs.iter())
        .zip(lows.iter())
        .zip(closes.iter())
        .zip(volumes.iter())
        .map(|(((((ts, open), high), low), close), vol)| {
            use chrono::TimeZone;
            Kline {
                ticker: query.symbol.clone(),
                timeframe: trader_core::Timeframe::D1,
                open_time: Utc.timestamp_millis_opt(*ts).unwrap(),
                open: *open,
                high: *high,
                low: *low,
                close: *close,
                volume: *vol,
                close_time: Utc.timestamp_millis_opt(*ts + 86400000).unwrap(),
                quote_volume: None,
                num_trades: None,
            }
        })
        .collect();

    // Volume Profile 계산
    let mut calculator = VolumeProfileCalculator::new(query.num_levels);
    if let Some(ratio) = query.value_area_ratio {
        calculator =
            calculator.with_value_area_ratio(Decimal::from_f64_retain(ratio).unwrap_or(dec!(0.7)));
    }

    match calculator.calculate(&klines) {
        Some(profile) => {
            let price_levels: Vec<PriceLevelResponse> = profile
                .price_levels
                .iter()
                .map(|level| PriceLevelResponse {
                    price: level.price.to_f64().unwrap_or(0.0),
                    volume: level.volume.to_f64().unwrap_or(0.0),
                    volume_pct: level.volume_pct.to_f64().unwrap_or(0.0),
                })
                .collect();

            Json(VolumeProfileResponse {
                symbol: query.symbol,
                period: profile.period,
                price_levels,
                poc: profile.poc.to_f64().unwrap_or(0.0),
                poc_index: profile.poc_index,
                value_area_high: profile.value_area_high.to_f64().unwrap_or(0.0),
                value_area_low: profile.value_area_low.to_f64().unwrap_or(0.0),
                total_volume: profile.total_volume.to_f64().unwrap_or(0.0),
                price_low: profile.price_low.to_f64().unwrap_or(0.0),
                price_high: profile.price_high.to_f64().unwrap_or(0.0),
            })
        }
        None => {
            // 에러 응답 (간단히 빈 결과 반환)
            Json(VolumeProfileResponse {
                symbol: query.symbol,
                period: 0,
                price_levels: vec![],
                poc: 0.0,
                poc_index: 0,
                value_area_high: 0.0,
                value_area_low: 0.0,
                total_volume: 0.0,
                price_low: 0.0,
                price_high: 0.0,
            })
        }
    }
}

/// 상관행렬 계산 핸들러.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/correlation",
    tag = "analytics",
    params(super::types::CorrelationQuery),
    responses(
        (status = 200, description = "상관행렬 계산 성공", body = super::types::CorrelationResponse)
    )
)]
pub async fn get_correlation(
    Query(query): Query<super::types::CorrelationQuery>,
) -> impl IntoResponse {
    use super::types::CorrelationResponse;
    use std::collections::HashMap;
    use trader_analytics::correlation::calculate_correlation_matrix;

    // 종목 코드 파싱 (쉼표 구분)
    let symbols: Vec<String> = query
        .symbols
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if symbols.len() < 2 {
        return Json(CorrelationResponse {
            symbols: vec![],
            matrix: vec![],
            period: 0,
        });
    }

    // 각 종목별 샘플 가격 데이터 생성 (실제로는 DB에서 조회)
    let days = query.period as i64;
    let mut prices: HashMap<String, Vec<f64>> = HashMap::new();

    // 시드 기반으로 종목별 다른 가격 움직임 생성
    for (idx, symbol) in symbols.iter().enumerate() {
        let mut price_series = Vec::with_capacity(days as usize);
        let mut price = 50000.0 + (idx as f64 * 10000.0); // 종목별 시작 가격 다르게

        for i in 0..days {
            // 종목별 다른 패턴의 가격 변동
            let change = match idx % 3 {
                0 => {
                    // 상승 추세
                    if i % 5 == 0 {
                        -0.015
                    } else {
                        0.01
                    }
                }
                1 => {
                    // 하락 추세
                    if i % 5 == 0 {
                        0.01
                    } else {
                        -0.008
                    }
                }
                _ => {
                    // 횡보
                    if i % 2 == 0 {
                        0.005
                    } else {
                        -0.005
                    }
                }
            };

            price_series.push(price);
            price *= 1.0 + change;
        }

        prices.insert(symbol.clone(), price_series);
    }

    // 상관행렬 계산
    match calculate_correlation_matrix(&prices, Some(symbols.clone())) {
        Some(matrix) => Json(CorrelationResponse {
            symbols: matrix.symbols,
            matrix: matrix.matrix,
            period: matrix.period,
        }),
        None => Json(CorrelationResponse {
            symbols: vec![],
            matrix: vec![],
            period: 0,
        }),
    }
}

/// VWAP 지표 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/indicators/vwap",
    tag = "analytics",
    params(VwapQuery),
    responses(
        (status = 200, description = "VWAP 데이터 조회 성공", body = VwapResponse)
    )
)]
pub async fn get_vwap_indicator(Query(query): Query<VwapQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, highs, lows, closes, volumes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = VwapParams {
        band_multiplier: Decimal::from_f64_retain(query.band_multiplier).unwrap_or(dec!(2.0)),
        reset_daily: query.reset_daily,
    };

    match engine.vwap(&highs, &lows, &closes, &volumes, params) {
        Ok(vwap_results) => {
            let data: Vec<VwapPointResponse> = timestamps
                .iter()
                .zip(vwap_results.iter())
                .map(|(&ts, result)| VwapPointResponse {
                    x: ts,
                    vwap: result.vwap.to_string(),
                    upper_band: result.upper_band.map(|v| v.to_string()),
                    lower_band: result.lower_band.map(|v| v.to_string()),
                    deviation_pct: result.deviation_pct.map(|v| v.to_string()),
                })
                .collect();

            // 최신 값 추출
            let current_vwap = vwap_results
                .last()
                .map(|r| r.vwap.to_string())
                .unwrap_or_else(|| "0".to_string());
            let current_deviation = vwap_results
                .last()
                .and_then(|r| r.deviation_pct.map(|v| v.to_string()));

            Json(VwapResponse {
                symbol: query.symbol,
                period: query.period,
                params: VwapParamsResponse {
                    band_multiplier: query.band_multiplier,
                    reset_daily: query.reset_daily,
                },
                data,
                count: vwap_results.len(),
                current_vwap,
                current_deviation,
            })
        }
        Err(e) => {
            // 에러 시 빈 응답 반환
            Json(VwapResponse {
                symbol: query.symbol.clone(),
                period: query.period.clone(),
                params: VwapParamsResponse {
                    band_multiplier: query.band_multiplier,
                    reset_daily: query.reset_daily,
                },
                data: vec![],
                count: 0,
                current_vwap: "0".to_string(),
                current_deviation: Some(format!("Error: {}", e)),
            })
        }
    }
}

/// Keltner Channel 지표 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/indicators/keltner",
    tag = "analytics",
    params(KeltnerQuery),
    responses(
        (status = 200, description = "Keltner Channel 데이터 조회 성공", body = KeltnerResponse)
    )
)]
pub async fn get_keltner_indicator(Query(query): Query<KeltnerQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, highs, lows, closes, _) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    // 참고: KeltnerChannelParams는 EMA와 ATR 모두 동일한 period를 사용
    let params = KeltnerChannelParams {
        period: query.ema_period,
        atr_multiplier: Decimal::from_f64_retain(query.atr_multiplier).unwrap_or(dec!(2.0)),
    };

    match engine.keltner_channel(&highs, &lows, &closes, params) {
        Ok(keltner_results) => {
            let data: Vec<KeltnerPointResponse> = timestamps
                .iter()
                .zip(keltner_results.iter())
                .map(|(&ts, result)| {
                    // 채널 폭 계산 (%)
                    let width_pct = match (result.upper, result.lower, result.middle) {
                        (Some(upper), Some(lower), Some(middle)) if middle > Decimal::ZERO => {
                            let width = (upper - lower) / middle * dec!(100);
                            Some(width.to_string())
                        }
                        _ => None,
                    };

                    KeltnerPointResponse {
                        x: ts,
                        middle: result.middle.map(|v| v.to_string()).unwrap_or_default(),
                        upper: result.upper.map(|v| v.to_string()).unwrap_or_default(),
                        lower: result.lower.map(|v| v.to_string()).unwrap_or_default(),
                        width_pct,
                    }
                })
                .collect();

            // 최신 값 추출
            let (current_middle, current_upper, current_lower) = keltner_results
                .last()
                .map(|r| {
                    (
                        r.middle.map(|v| v.to_string()).unwrap_or_default(),
                        r.upper.map(|v| v.to_string()).unwrap_or_default(),
                        r.lower.map(|v| v.to_string()).unwrap_or_default(),
                    )
                })
                .unwrap_or_else(|| ("0".to_string(), "0".to_string(), "0".to_string()));

            Json(KeltnerResponse {
                symbol: query.symbol,
                period: query.period,
                params: KeltnerParamsResponse {
                    ema_period: query.ema_period,
                    atr_multiplier: query.atr_multiplier,
                },
                data,
                count: keltner_results.len(),
                current_middle,
                current_upper,
                current_lower,
            })
        }
        Err(e) => {
            // 에러 시 빈 응답 반환
            Json(KeltnerResponse {
                symbol: query.symbol.clone(),
                period: query.period.clone(),
                params: KeltnerParamsResponse {
                    ema_period: query.ema_period,
                    atr_multiplier: query.atr_multiplier,
                },
                data: vec![],
                count: 0,
                current_middle: format!("Error: {}", e),
                current_upper: "0".to_string(),
                current_lower: "0".to_string(),
            })
        }
    }
}

/// OBV (On-Balance Volume) 지표 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/indicators/obv",
    tag = "analytics",
    params(ObvQuery),
    responses(
        (status = 200, description = "OBV 데이터 조회 성공", body = ObvResponse)
    )
)]
pub async fn get_obv_indicator(Query(query): Query<ObvQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, _, _, closes, volumes) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = ObvParams::default();

    match engine.obv(&closes, &volumes, params) {
        Ok(obv_results) => {
            let data: Vec<ObvPointResponse> = timestamps
                .iter()
                .zip(obv_results.iter())
                .map(|(&ts, result)| ObvPointResponse {
                    x: ts,
                    obv: result.obv,
                    change: result.change,
                })
                .collect();

            // 최신 값 추출
            let (current_obv, current_change) = obv_results
                .last()
                .map(|r| (r.obv, r.change))
                .unwrap_or((0, 0));

            Json(ObvResponse {
                symbol: query.symbol,
                period: query.period,
                data,
                count: obv_results.len(),
                current_obv,
                current_change,
            })
        }
        Err(_) => {
            // 에러 시 빈 응답 반환
            Json(ObvResponse {
                symbol: query.symbol.clone(),
                period: query.period.clone(),
                data: vec![],
                count: 0,
                current_obv: 0,
                current_change: 0,
            })
        }
    }
}

/// SuperTrend 지표 데이터 조회.
#[utoipa::path(
    get,
    path = "/api/v1/analytics/indicators/supertrend",
    tag = "analytics",
    params(SuperTrendQuery),
    responses(
        (status = 200, description = "SuperTrend 데이터 조회 성공", body = SuperTrendResponse)
    )
)]
pub async fn get_supertrend_indicator(Query(query): Query<SuperTrendQuery>) -> impl IntoResponse {
    let days = parse_period_to_days(&query.period);
    let (timestamps, _, highs, lows, closes, _) = generate_sample_ohlcv(days);

    let engine = IndicatorEngine::new();
    let params = SuperTrendParams {
        atr_period: query.atr_period,
        multiplier: Decimal::from_f64_retain(query.multiplier).unwrap_or(dec!(3.0)),
    };

    match engine.supertrend(&highs, &lows, &closes, params) {
        Ok(st_results) => {
            let data: Vec<SuperTrendPointResponse> = timestamps
                .iter()
                .zip(st_results.iter())
                .map(|(&ts, result)| SuperTrendPointResponse {
                    x: ts,
                    value: result.value.map(|v| v.to_string()),
                    is_uptrend: result.is_uptrend,
                    buy_signal: result.buy_signal,
                    sell_signal: result.sell_signal,
                })
                .collect();

            // 최신 값 추출
            let (current_value, current_trend) = st_results
                .last()
                .map(|r| {
                    (
                        r.value.map(|v| v.to_string()),
                        if r.is_uptrend { "UP" } else { "DOWN" }.to_string(),
                    )
                })
                .unwrap_or((None, "UNKNOWN".to_string()));

            // 시그널 카운트
            let total_buy_signals = st_results.iter().filter(|r| r.buy_signal).count();
            let total_sell_signals = st_results.iter().filter(|r| r.sell_signal).count();

            Json(SuperTrendResponse {
                symbol: query.symbol,
                period: query.period,
                params: SuperTrendParamsResponse {
                    atr_period: query.atr_period,
                    multiplier: query.multiplier,
                },
                data,
                count: st_results.len(),
                current_value,
                current_trend,
                total_buy_signals,
                total_sell_signals,
            })
        }
        Err(e) => {
            // 에러 시 빈 응답 반환
            Json(SuperTrendResponse {
                symbol: query.symbol.clone(),
                period: query.period.clone(),
                params: SuperTrendParamsResponse {
                    atr_period: query.atr_period,
                    multiplier: query.multiplier,
                },
                data: vec![],
                count: 0,
                current_value: Some(format!("Error: {}", e)),
                current_trend: "ERROR".to_string(),
                total_buy_signals: 0,
                total_sell_signals: 0,
            })
        }
    }
}
