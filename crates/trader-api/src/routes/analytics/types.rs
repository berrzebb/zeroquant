//! 분석 API 타입 정의.
//!
//! 이 모듈은 analytics 관련 요청/응답 타입을 정의합니다.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use trader_analytics::portfolio::{ChartPoint, MonthlyReturnCell, PerformanceSummary};
use utoipa::{IntoParams, ToSchema};

// ==================== 쿼리 파라미터 ====================

/// 기간 필터 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct PeriodQuery {
    /// 기간 (1w, 1m, 3m, 6m, 1y, ytd, all)
    #[serde(default = "default_period")]
    pub period: String,

    /// 시작 날짜 (ISO 8601, 선택적)
    pub start_date: Option<String>,

    /// 종료 날짜 (ISO 8601, 선택적)
    pub end_date: Option<String>,

    /// 자격증명 ID (선택적, 특정 계좌만 조회)
    pub credential_id: Option<String>,
}

pub(crate) fn default_period() -> String {
    "3m".to_string()
}

/// 차트 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct ChartQuery {
    /// 기간 (1w, 1m, 3m, 6m, 1y, ytd, all)
    #[serde(default = "default_period")]
    pub period: String,

    /// 롤링 윈도우 일수 (기본: 365)
    #[serde(default = "default_window")]
    pub window_days: i64,
}

pub(crate) fn default_window() -> i64 {
    365
}

// ==================== 응답 타입 ====================

/// 성과 요약 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PerformanceResponse {
    /// 현재 자산 가치
    pub current_equity: String,

    /// 초기 자본 (기간 시작점)
    pub initial_capital: String,

    /// 총 수익/손실 금액 (기간 시작점 대비)
    pub total_pnl: String,

    /// 총 수익률 (%) - 기간 시작점 대비
    pub total_return_pct: String,

    /// CAGR (%)
    pub cagr_pct: String,

    /// 최대 낙폭 (%)
    pub max_drawdown_pct: String,

    /// 현재 낙폭 (%)
    pub current_drawdown_pct: String,

    /// 고점 자산 가치
    pub peak_equity: String,

    /// 데이터 기간 (일)
    pub period_days: i64,

    /// 기간별 수익률
    pub period_returns: Vec<PeriodReturnResponse>,

    /// 마지막 업데이트 시각
    pub last_updated: String,

    // === 포지션 기반 지표 (실제 투자 원금 대비) ===
    /// 총 투자 원금 (매입 총액)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost_basis: Option<String>,

    /// 포지션 손익 금액 (현재 평가액 - 매입 총액)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_pnl: Option<String>,

    /// 포지션 손익률 (%) - 실제 투자 원금 대비
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_pnl_pct: Option<String>,
}

/// 기간별 수익률 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PeriodReturnResponse {
    /// 기간 이름 (1W, 1M, 3M, etc.)
    pub period: String,

    /// 수익률 (%)
    pub return_pct: String,
}

impl From<&PerformanceSummary> for PerformanceResponse {
    fn from(summary: &PerformanceSummary) -> Self {
        Self {
            current_equity: summary.current_equity.to_string(),
            initial_capital: summary.initial_capital.to_string(),
            total_pnl: summary.total_pnl.to_string(),
            total_return_pct: summary.total_return_pct.to_string(),
            cagr_pct: summary.cagr_pct.to_string(),
            max_drawdown_pct: summary.max_drawdown_pct.to_string(),
            current_drawdown_pct: summary.current_drawdown_pct.to_string(),
            peak_equity: summary.peak_equity.to_string(),
            period_days: summary.period_days,
            period_returns: Vec::new(),
            last_updated: summary.last_updated.to_rfc3339(),
            // 포지션 기반 지표 (샘플 데이터에서는 None)
            total_cost_basis: None,
            position_pnl: None,
            position_pnl_pct: None,
        }
    }
}

/// 자산 곡선 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EquityCurveResponse {
    /// 차트 데이터 포인트
    pub data: Vec<ChartPointResponse>,

    /// 데이터 포인트 수
    pub count: usize,

    /// 기간
    pub period: String,

    /// 시작 시간
    pub start_time: String,

    /// 종료 시간
    pub end_time: String,
}

/// 차트 데이터 포인트 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ChartPointResponse {
    /// 타임스탬프 (밀리초)
    pub x: i64,

    /// 값
    pub y: String,

    /// 레이블 (선택적)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl From<&ChartPoint> for ChartPointResponse {
    fn from(point: &ChartPoint) -> Self {
        Self {
            x: point.x,
            y: point.y.to_string(),
            label: point.label.clone(),
        }
    }
}

/// 차트 데이터 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ChartResponse {
    /// 차트 이름
    pub name: String,

    /// 차트 데이터 포인트
    pub data: Vec<ChartPointResponse>,

    /// 데이터 포인트 수
    pub count: usize,

    /// 기간
    pub period: String,
}

/// 월별 수익률 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MonthlyReturnsResponse {
    /// 월별 데이터
    pub data: Vec<MonthlyReturnCellResponse>,

    /// 총 월 수
    pub count: usize,

    /// 연도 범위
    pub year_range: (i32, i32),
}

/// 월별 수익률 셀 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MonthlyReturnCellResponse {
    /// 연도
    pub year: i32,

    /// 월 (1-12)
    pub month: u32,

    /// 수익률 (%)
    pub return_pct: String,

    /// 색상 강도 (-1.0 ~ 1.0)
    pub intensity: f64,
}

impl From<&MonthlyReturnCell> for MonthlyReturnCellResponse {
    fn from(cell: &MonthlyReturnCell) -> Self {
        Self {
            year: cell.year,
            month: cell.month,
            return_pct: cell.return_pct.to_string(),
            intensity: cell.intensity,
        }
    }
}

// ==================== 기술적 지표 쿼리 타입 ====================

/// 지표 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct IndicatorQuery {
    /// 심볼 (예: 005930, AAPL)
    pub symbol: String,

    /// 기간 (1d, 1w, 1m, 3m, 6m, 1y)
    #[serde(default = "default_indicator_period")]
    pub period: String,

    /// 지표별 파라미터 (JSON 형식)
    pub params: Option<String>,
}

pub(crate) fn default_indicator_period() -> String {
    "3m".to_string()
}

/// SMA 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct SmaQuery {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// SMA 기간 (기본: 20)
    #[serde(default = "default_sma_period")]
    pub sma_period: usize,
}

pub(crate) fn default_sma_period() -> usize {
    20
}

/// EMA 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct EmaQuery {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// EMA 기간 (기본: 12)
    #[serde(default = "default_ema_period")]
    pub ema_period: usize,
}

pub(crate) fn default_ema_period() -> usize {
    12
}

/// RSI 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct RsiQuery {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// RSI 기간 (기본: 14)
    #[serde(default = "default_rsi_period")]
    pub rsi_period: usize,
}

pub(crate) fn default_rsi_period() -> usize {
    14
}

/// MACD 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct MacdQuery {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// 단기 EMA 기간 (기본: 12)
    #[serde(default = "default_macd_fast")]
    pub fast_period: usize,
    /// 장기 EMA 기간 (기본: 26)
    #[serde(default = "default_macd_slow")]
    pub slow_period: usize,
    /// 시그널 라인 기간 (기본: 9)
    #[serde(default = "default_macd_signal")]
    pub signal_period: usize,
}

pub(crate) fn default_macd_fast() -> usize {
    12
}

pub(crate) fn default_macd_slow() -> usize {
    26
}

pub(crate) fn default_macd_signal() -> usize {
    9
}

/// 볼린저 밴드 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct BollingerQuery {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// 이동평균 기간 (기본: 20)
    #[serde(default = "default_bollinger_period")]
    pub bb_period: usize,
    /// 표준편차 배수 (기본: 2.0)
    #[serde(default = "default_bollinger_std")]
    pub std_dev: f64,
}

pub(crate) fn default_bollinger_period() -> usize {
    20
}

pub(crate) fn default_bollinger_std() -> f64 {
    2.0
}

/// 스토캐스틱 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct StochasticQuery {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// %K 기간 (기본: 14)
    #[serde(default = "default_stochastic_k")]
    pub k_period: usize,
    /// %D 기간 (기본: 3)
    #[serde(default = "default_stochastic_d")]
    pub d_period: usize,
}

pub(crate) fn default_stochastic_k() -> usize {
    14
}

pub(crate) fn default_stochastic_d() -> usize {
    3
}

/// ATR 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct AtrQuery {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// ATR 기간 (기본: 14)
    #[serde(default = "default_atr_period")]
    pub atr_period: usize,
}

pub(crate) fn default_atr_period() -> usize {
    14
}

// ==================== 기술적 지표 응답 타입 ====================

/// 다중 지표 계산 요청.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CalculateIndicatorsRequest {
    /// 심볼
    pub symbol: String,
    /// 기간
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// 계산할 지표 목록
    pub indicators: Vec<IndicatorConfig>,
}

/// 지표 설정.
#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct IndicatorConfig {
    /// 지표 타입
    #[serde(rename = "type")]
    pub indicator_type: String,
    /// 지표 파라미터
    pub params: serde_json::Value,
    /// 차트에 표시할 색상 (선택적)
    pub color: Option<String>,
    /// 차트에 표시할 이름 (선택적)
    pub name: Option<String>,
}

/// 사용 가능한 지표 목록 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct AvailableIndicatorsResponse {
    /// 지표 목록
    pub indicators: Vec<IndicatorInfo>,
}

/// 지표 정보.
#[derive(Debug, Serialize, ToSchema)]
pub struct IndicatorInfo {
    /// 지표 ID
    pub id: String,
    /// 지표 이름
    pub name: String,
    /// 지표 설명
    pub description: String,
    /// 지표 카테고리
    pub category: String,
    /// 기본 파라미터
    pub default_params: serde_json::Value,
    /// 오버레이 여부 (가격 차트 위에 표시)
    pub overlay: bool,
}

/// 단일 지표 데이터 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct IndicatorDataResponse {
    /// 지표 ID
    pub indicator: String,
    /// 지표 이름
    pub name: String,
    /// 심볼
    pub symbol: String,
    /// 사용된 파라미터
    pub params: serde_json::Value,
    /// 데이터 시리즈
    pub series: Vec<IndicatorSeries>,
}

/// 지표 시리즈 데이터.
#[derive(Debug, Serialize, ToSchema)]
pub struct IndicatorSeries {
    /// 시리즈 이름 (예: "macd", "signal", "histogram")
    pub name: String,
    /// 데이터 포인트
    pub data: Vec<IndicatorPoint>,
    /// 색상 (선택적)
    pub color: Option<String>,
    /// 시리즈 타입 (line, bar, area)
    pub series_type: String,
}

/// 지표 데이터 포인트.
#[derive(Debug, Serialize, ToSchema)]
pub struct IndicatorPoint {
    /// 타임스탬프 (밀리초)
    pub x: i64,
    /// 값
    pub y: Option<String>,
}

/// 다중 지표 계산 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct CalculateIndicatorsResponse {
    /// 심볼
    pub symbol: String,
    /// 기간
    pub period: String,
    /// 지표별 결과
    pub results: Vec<IndicatorDataResponse>,
}

// ==================== 동기화 타입 ====================

/// 동기화 요청.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SyncEquityCurveRequest {
    /// 자격증명 ID
    pub credential_id: String,
    /// 조회 시작일 (YYYYMMDD)
    pub start_date: String,
    /// 조회 종료일 (YYYYMMDD)
    pub end_date: String,
    /// 종가 기반 계산 사용 여부 (true: 정확한 계산, false: 현금 흐름 기반)
    #[serde(default)]
    pub use_market_prices: bool,
    /// 초기 자본금 (종가 기반 계산 시 필수)
    pub initial_capital: Option<Decimal>,
}

/// 동기화 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct SyncEquityCurveResponse {
    /// 성공 여부
    pub success: bool,
    /// 동기화된 스냅샷 개수
    pub synced_count: usize,
    /// 처리된 체결 내역 개수
    pub execution_count: usize,
    /// 시작 날짜
    pub start_date: String,
    /// 종료 날짜
    pub end_date: String,
    /// 메시지
    pub message: String,
}

// ==================== Volume Profile 타입 ====================

/// Volume Profile 요청 쿼리.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct VolumeProfileQuery {
    /// 종목 코드 (필수)
    pub symbol: String,
    /// 분석 기간 (일, 기본: 60)
    #[serde(default = "default_vp_period")]
    pub period: i32,
    /// 가격 레벨 수 (기본: 20)
    #[serde(default = "default_vp_levels")]
    pub num_levels: usize,
    /// Value Area 비율 (기본: 0.7)
    pub value_area_ratio: Option<f64>,
}

fn default_vp_period() -> i32 {
    60
}

fn default_vp_levels() -> usize {
    20
}

/// 가격대별 거래량 레벨.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PriceLevelResponse {
    /// 가격 (레벨 중심 가격)
    pub price: f64,
    /// 해당 가격대의 총 거래량
    pub volume: f64,
    /// 전체 거래량 대비 비율 (%)
    pub volume_pct: f64,
}

/// Volume Profile 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VolumeProfileResponse {
    /// 종목 코드
    pub symbol: String,
    /// 분석 기간 (캔들 수)
    pub period: usize,
    /// 가격대별 거래량 레벨 (가격 오름차순)
    pub price_levels: Vec<PriceLevelResponse>,
    /// POC (Point of Control) - 최대 거래량 가격
    pub poc: f64,
    /// POC 인덱스
    pub poc_index: usize,
    /// Value Area High (70% 상한)
    pub value_area_high: f64,
    /// Value Area Low (70% 하한)
    pub value_area_low: f64,
    /// 전체 거래량
    pub total_volume: f64,
    /// 가격 범위 (최저)
    pub price_low: f64,
    /// 가격 범위 (최고)
    pub price_high: f64,
}

// ==================== Correlation (상관관계) 타입 ====================

/// 상관행렬 요청 쿼리.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct CorrelationQuery {
    /// 분석할 종목 코드들 (쉼표 구분)
    pub symbols: String,
    /// 분석 기간 (일, 기본: 60)
    #[serde(default = "default_corr_period")]
    pub period: i32,
}

fn default_corr_period() -> i32 {
    60
}

/// 상관행렬 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CorrelationResponse {
    /// 종목 목록 (행/열 순서)
    pub symbols: Vec<String>,
    /// 상관계수 행렬 (N×N, -1.0 ~ 1.0)
    pub matrix: Vec<Vec<f64>>,
    /// 분석 기간 (일수)
    pub period: usize,
}

// ==================== VWAP (거래량 가중 평균가격) 타입 ====================

/// VWAP 요청 쿼리.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct VwapQuery {
    /// 종목 코드 (필수)
    pub symbol: String,
    /// 분석 기간 (1d, 1w, 1m, 3m, 6m, 1y)
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// 밴드 배수 (기본: 2.0)
    #[serde(default = "default_vwap_band")]
    pub band_multiplier: f64,
    /// 일간 리셋 여부 (기본: false - 연속 계산)
    #[serde(default)]
    pub reset_daily: bool,
}

fn default_vwap_band() -> f64 {
    2.0
}

/// VWAP 데이터 포인트.
#[derive(Debug, Serialize, ToSchema)]
pub struct VwapPointResponse {
    /// 타임스탬프 (밀리초)
    pub x: i64,
    /// VWAP 값
    pub vwap: String,
    /// 상단 밴드 (선택적)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upper_band: Option<String>,
    /// 하단 밴드 (선택적)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lower_band: Option<String>,
    /// 현재가와의 괴리율 (%)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deviation_pct: Option<String>,
}

/// VWAP 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct VwapResponse {
    /// 종목 코드
    pub symbol: String,
    /// 분석 기간
    pub period: String,
    /// 사용된 파라미터
    pub params: VwapParamsResponse,
    /// VWAP 데이터 시리즈
    pub data: Vec<VwapPointResponse>,
    /// 데이터 포인트 수
    pub count: usize,
    /// 최신 VWAP 값
    pub current_vwap: String,
    /// 최신 괴리율 (%)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_deviation: Option<String>,
}

/// VWAP 파라미터 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct VwapParamsResponse {
    /// 밴드 배수
    pub band_multiplier: f64,
    /// 일간 리셋 여부
    pub reset_daily: bool,
}

// ==================== Keltner Channel 타입 ====================

/// Keltner Channel 요청 쿼리.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct KeltnerQuery {
    /// 종목 코드 (필수)
    pub symbol: String,
    /// 분석 기간 (1d, 1w, 1m, 3m, 6m, 1y)
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// EMA/ATR 기간 (기본: 20) - EMA와 ATR 모두 동일한 기간 사용
    #[serde(default = "default_keltner_period")]
    pub ema_period: usize,
    /// ATR 배수 (기본: 2.0)
    #[serde(default = "default_keltner_mult")]
    pub atr_multiplier: f64,
}

fn default_keltner_period() -> usize {
    20
}

fn default_keltner_mult() -> f64 {
    2.0
}

/// Keltner Channel 데이터 포인트.
#[derive(Debug, Serialize, ToSchema)]
pub struct KeltnerPointResponse {
    /// 타임스탬프 (밀리초)
    pub x: i64,
    /// 중간선 (EMA)
    pub middle: String,
    /// 상단선
    pub upper: String,
    /// 하단선
    pub lower: String,
    /// 채널 폭 (%)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_pct: Option<String>,
}

/// Keltner Channel 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct KeltnerResponse {
    /// 종목 코드
    pub symbol: String,
    /// 분석 기간
    pub period: String,
    /// 사용된 파라미터
    pub params: KeltnerParamsResponse,
    /// Keltner Channel 데이터 시리즈
    pub data: Vec<KeltnerPointResponse>,
    /// 데이터 포인트 수
    pub count: usize,
    /// 최신 중간선 값
    pub current_middle: String,
    /// 최신 상단선 값
    pub current_upper: String,
    /// 최신 하단선 값
    pub current_lower: String,
}

/// Keltner Channel 파라미터 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct KeltnerParamsResponse {
    /// EMA/ATR 기간 (EMA와 ATR 모두 동일한 기간 사용)
    pub ema_period: usize,
    /// ATR 배수
    pub atr_multiplier: f64,
}

// ==================== OBV (On-Balance Volume) 타입 ====================

/// OBV 요청 쿼리.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct ObvQuery {
    /// 종목 코드 (필수)
    pub symbol: String,
    /// 분석 기간 (1d, 1w, 1m, 3m, 6m, 1y)
    #[serde(default = "default_indicator_period")]
    pub period: String,
}

/// OBV 데이터 포인트.
#[derive(Debug, Serialize, ToSchema)]
pub struct ObvPointResponse {
    /// 타임스탬프 (밀리초)
    pub x: i64,
    /// OBV 값
    pub obv: i64,
    /// OBV 변화량
    pub change: i64,
}

/// OBV 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct ObvResponse {
    /// 종목 코드
    pub symbol: String,
    /// 분석 기간
    pub period: String,
    /// OBV 데이터 시리즈
    pub data: Vec<ObvPointResponse>,
    /// 데이터 포인트 수
    pub count: usize,
    /// 최신 OBV 값
    pub current_obv: i64,
    /// 최신 OBV 변화량
    pub current_change: i64,
}

// ==================== SuperTrend 타입 ====================

/// SuperTrend 요청 쿼리.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct SuperTrendQuery {
    /// 종목 코드 (필수)
    pub symbol: String,
    /// 분석 기간 (1d, 1w, 1m, 3m, 6m, 1y)
    #[serde(default = "default_indicator_period")]
    pub period: String,
    /// ATR 기간 (기본: 10)
    #[serde(default = "default_supertrend_atr_period")]
    pub atr_period: usize,
    /// ATR 배수 (기본: 3.0)
    #[serde(default = "default_supertrend_multiplier")]
    pub multiplier: f64,
}

fn default_supertrend_atr_period() -> usize {
    10
}

fn default_supertrend_multiplier() -> f64 {
    3.0
}

/// SuperTrend 데이터 포인트.
#[derive(Debug, Serialize, ToSchema)]
pub struct SuperTrendPointResponse {
    /// 타임스탬프 (밀리초)
    pub x: i64,
    /// SuperTrend 값
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// 추세 방향 (true: 상승, false: 하락)
    pub is_uptrend: bool,
    /// 매수 시그널
    pub buy_signal: bool,
    /// 매도 시그널
    pub sell_signal: bool,
}

/// SuperTrend 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct SuperTrendResponse {
    /// 종목 코드
    pub symbol: String,
    /// 분석 기간
    pub period: String,
    /// 사용된 파라미터
    pub params: SuperTrendParamsResponse,
    /// SuperTrend 데이터 시리즈
    pub data: Vec<SuperTrendPointResponse>,
    /// 데이터 포인트 수
    pub count: usize,
    /// 현재 SuperTrend 값
    pub current_value: Option<String>,
    /// 현재 추세 방향
    pub current_trend: String,
    /// 총 매수 시그널 수
    pub total_buy_signals: usize,
    /// 총 매도 시그널 수
    pub total_sell_signals: usize,
}

/// SuperTrend 파라미터 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct SuperTrendParamsResponse {
    /// ATR 기간
    pub atr_period: usize,
    /// ATR 배수
    pub multiplier: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use trader_analytics::portfolio::ChartPoint;

    #[test]
    fn test_period_return_response() {
        let resp = PeriodReturnResponse {
            period: "1M".to_string(),
            return_pct: "5.25".to_string(),
        };
        assert_eq!(resp.period, "1M");
    }

    #[test]
    fn test_chart_point_response() {
        let point = ChartPoint::new(Utc::now(), dec!(100));
        let resp = ChartPointResponse::from(&point);
        assert_eq!(resp.y, "100");
    }
}
