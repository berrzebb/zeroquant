//! 백테스트 API 타입 정의
//!
//! 요청/응답 타입 및 SDUI 스키마 타입을 정의합니다.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::prelude::FromStr;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use trader_core::{Side, Timeframe, TradeInfo};
use ts_rs::TS;
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

// ==================== 커스텀 검증 함수 ====================

/// 초기 자본금 검증 (100 ~ 10억)
fn validate_initial_capital(value: &Decimal) -> Result<(), ValidationError> {
    let min = Decimal::from(100);
    let max = Decimal::from(1_000_000_000);

    if *value < min {
        return Err(ValidationError::new("initial_capital_too_small")
            .with_message("초기 자본금은 최소 100 이상이어야 합니다".into()));
    }
    if *value > max {
        return Err(ValidationError::new("initial_capital_too_large")
            .with_message("초기 자본금은 10억을 초과할 수 없습니다".into()));
    }
    Ok(())
}

/// 수수료율 검증 (0 ~ 0.1 = 10%)
/// 참고: Option<Decimal> 필드에 사용 시 validator가 Some일 때만 호출하므로 &Decimal을 받음
fn validate_commission_rate(value: &Decimal) -> Result<(), ValidationError> {
    let max = Decimal::from_str("0.1").unwrap_or(Decimal::ONE);
    if *value < Decimal::ZERO {
        return Err(ValidationError::new("commission_rate_negative")
            .with_message("수수료율은 0 이상이어야 합니다".into()));
    }
    if *value > max {
        return Err(ValidationError::new("commission_rate_too_high")
            .with_message("수수료율은 10%를 초과할 수 없습니다".into()));
    }
    Ok(())
}

/// 슬리피지율 검증 (0 ~ 0.05 = 5%)
/// 참고: Option<Decimal> 필드에 사용 시 validator가 Some일 때만 호출하므로 &Decimal을 받음
fn validate_slippage_rate(value: &Decimal) -> Result<(), ValidationError> {
    let max = Decimal::from_str("0.05").unwrap_or(Decimal::ONE);
    if *value < Decimal::ZERO {
        return Err(ValidationError::new("slippage_rate_negative")
            .with_message("슬리피지율은 0 이상이어야 합니다".into()));
    }
    if *value > max {
        return Err(ValidationError::new("slippage_rate_too_high")
            .with_message("슬리피지율은 5%를 초과할 수 없습니다".into()));
    }
    Ok(())
}

/// 날짜 형식 검증 (YYYY-MM-DD)
fn validate_date_format(value: &str) -> Result<(), ValidationError> {
    if NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err() {
        return Err(ValidationError::new("invalid_date_format")
            .with_message("날짜 형식은 YYYY-MM-DD여야 합니다".into()));
    }
    Ok(())
}

// ==================== SDUI (Server Driven UI) 스키마 ====================

/// UI 필드 타입
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UiFieldType {
    /// 숫자 입력
    Number,
    /// 텍스트 입력
    Text,
    /// 드롭다운 선택
    Select,
    /// 체크박스
    Boolean,
    /// 심볼 선택 (멀티 선택 가능)
    SymbolPicker,
    /// 범위 슬라이더
    Range,
    /// 분할 레벨 배열 (Magic Split용)
    SplitLevels,
    /// 심볼 카테고리 그룹 (자산배분 전략용 - 카테고리별 심볼 선택)
    SymbolCategoryGroup,
    /// 날짜 선택
    Date,
    /// 시간대 선택
    Timeframe,
}

/// 유효성 검사 규칙
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct UiValidation {
    /// 필수 여부
    #[serde(default)]
    pub required: bool,
    /// 최소값 (Number, Range)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// 최대값 (Number, Range)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// 단계 (Number, Range)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// 최소 길이 (Text)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_length: Option<usize>,
    /// 최대 길이 (Text)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,
    /// 정규식 패턴 (Text)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// 최소 선택 수 (SymbolPicker)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_items: Option<usize>,
    /// 최대 선택 수 (SymbolPicker)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
}


/// 선택 옵션 (Select 타입용)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UiSelectOption {
    /// 표시 레이블
    pub label: String,
    /// 실제 값
    pub value: serde_json::Value,
    /// 설명 (툴팁)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 심볼 카테고리 정의 (자산배분 전략용)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SymbolCategory {
    /// 카테고리 키 (예: "canary_assets", "offensive_assets")
    pub key: String,
    /// 카테고리 표시 이름 (예: "카나리아 자산")
    pub label: String,
    /// 카테고리 설명
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 기본 심볼 목록
    #[serde(default)]
    pub default_symbols: Vec<String>,
    /// 추천 심볼 목록 (사용자에게 선택 가이드)
    #[serde(default)]
    pub suggested_symbols: Vec<String>,
    /// 최소 선택 수
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_items: Option<usize>,
    /// 최대 선택 수
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    /// 표시 순서
    #[serde(default)]
    pub order: i32,
}

/// UI 필드 정의 (SDUI 핵심)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UiField {
    /// 필드 키 (파라미터 이름)
    pub key: String,
    /// 표시 레이블
    pub label: String,
    /// 필드 타입
    pub field_type: UiFieldType,
    /// 기본값
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
    /// 플레이스홀더
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// 도움말/설명
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_text: Option<String>,
    /// 유효성 검사 규칙
    #[serde(default)]
    pub validation: UiValidation,
    /// 선택 옵션 (Select 타입용)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<UiSelectOption>>,
    /// 심볼 카테고리 목록 (SymbolCategoryGroup 타입용)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_categories: Option<Vec<SymbolCategory>>,
    /// 그룹 ID (필드 그룹핑용)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// 표시 순서
    #[serde(default)]
    pub order: i32,
    /// 조건부 표시 (다른 필드 값에 따라)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_when: Option<UiCondition>,
    /// 단위 (예: %, 원, USD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// 조건부 표시 규칙
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UiCondition {
    /// 참조 필드 키
    pub field: String,
    /// 연산자
    pub operator: UiConditionOperator,
    /// 비교 값
    pub value: serde_json::Value,
}

/// 조건 연산자
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UiConditionOperator {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    Contains,
}

/// 필드 그룹 정의
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UiFieldGroup {
    /// 그룹 ID
    pub id: String,
    /// 그룹 레이블
    pub label: String,
    /// 그룹 설명
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 표시 순서
    #[serde(default)]
    pub order: i32,
    /// 접힘 여부 (기본 접힘)
    #[serde(default)]
    pub collapsed: bool,
}

/// SDUI 스키마 (전략별 UI 정의)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UiSchema {
    /// 필드 정의 목록
    pub fields: Vec<UiField>,
    /// 필드 그룹 정의
    #[serde(default)]
    pub groups: Vec<UiFieldGroup>,
    /// 레이아웃 힌트 (예: 2열 레이아웃)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<UiLayout>,
}

/// 레이아웃 힌트
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UiLayout {
    /// 열 수 (기본 1)
    #[serde(default = "default_columns")]
    pub columns: usize,
}

fn default_columns() -> usize {
    1
}

/// 전략 실행 주기
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionSchedule {
    /// 실시간 (가격 변동 시마다)
    Realtime,
    /// 캔들 완성 시 (분봉/일봉)
    OnCandleClose,
    /// 일 1회
    Daily,
    /// 주 1회
    Weekly,
    /// 월 1회
    Monthly,
}

impl ExecutionSchedule {
    /// 한글 표시명
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Realtime => "실시간",
            Self::OnCandleClose => "캔들 완성 시",
            Self::Daily => "일 1회",
            Self::Weekly => "주 1회",
            Self::Monthly => "월 1회",
        }
    }

    /// 상세 설명
    pub fn description(&self) -> &'static str {
        match self {
            Self::Realtime => "가격 변동 시마다 전략을 실행합니다",
            Self::OnCandleClose => "분봉 또는 일봉 완성 시 전략을 실행합니다",
            Self::Daily => "매일 지정된 시간에 전략을 실행합니다",
            Self::Weekly => "매주 지정된 요일에 전략을 실행합니다",
            Self::Monthly => "매월 첫 거래일에 전략을 실행합니다",
        }
    }
}

// ==================== 다중 타임프레임 설정 ====================

/// Secondary 타임프레임 설정 (API 요청용)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SecondaryTimeframeConfig {
    /// 타임프레임
    pub timeframe: Timeframe,
    /// 캔들 개수 (선택, 기본값: 60)
    #[serde(default)]
    pub candle_count: Option<usize>,
}

/// 다중 타임프레임 설정 (API 요청용)
///
/// 프론트엔드에서 전송하는 형식과 일치합니다.
/// 백엔드의 `trader_core::MultiTimeframeConfig`로 변환되어 사용됩니다.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MultiTimeframeRequest {
    /// Primary 타임프레임 (전략 실행 기준)
    pub primary: Timeframe,
    /// Secondary 타임프레임 목록 (추세 확인용, 최대 3개)
    #[serde(default)]
    pub secondary: Vec<SecondaryTimeframeConfig>,
}

impl MultiTimeframeRequest {
    /// trader_core::MultiTimeframeConfig로 변환
    pub fn to_core_config(&self) -> trader_core::domain::MultiTimeframeConfig {
        let mut config = trader_core::domain::MultiTimeframeConfig::new()
            .with_primary(self.primary);

        for sec in &self.secondary {
            let count = sec.candle_count.unwrap_or(60);
            config = config.with_timeframe(sec.timeframe, count);
        }

        config
    }

    /// Secondary 타임프레임 목록만 반환
    pub fn secondary_timeframes(&self) -> Vec<Timeframe> {
        self.secondary.iter().map(|s| s.timeframe).collect()
    }
}

// ==================== 요청/응답 타입 ====================

/// 백테스트 가능한 전략 항목
// Note: 복잡한 타입(UiSchema, ExecutionSchedule)은 skip 처리
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export, export_to = "backtest/")]
pub struct BacktestableStrategy {
    /// 전략 ID
    pub id: String,
    /// 전략 이름
    pub name: String,
    /// 전략 설명
    pub description: String,
    /// 지원하는 심볼 목록
    pub supported_symbols: Vec<String>,
    /// 기본 설정 파라미터
    #[ts(skip)]
    pub default_params: serde_json::Value,
    /// SDUI 스키마 (UI 메타데이터)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(skip)]
    pub ui_schema: Option<UiSchema>,
    /// 전략 카테고리
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub category: Option<String>,
    /// 전략 태그
    #[serde(default)]
    pub tags: Vec<String>,
    /// 실행 주기
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(skip)]
    pub execution_schedule: Option<ExecutionSchedule>,
    /// 실행 주기 상세 설명 (예: "장 시작 5분 후", "매월 첫 거래일")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub schedule_detail: Option<String>,
    /// 작동 방식 상세 설명
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub how_it_works: Option<String>,
}

/// 백테스트 가능한 전략 목록 응답
#[derive(Debug, Serialize, Deserialize, TS, ToSchema)]
#[ts(export, export_to = "backtest/")]
pub struct BacktestStrategiesResponse {
    /// 전략 목록
    pub strategies: Vec<BacktestableStrategy>,
    /// 전체 전략 수
    pub total: usize,
}

/// 백테스트 실행 요청
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct BacktestRunRequest {
    /// 전략 ID
    #[validate(length(min = 1, max = 100, message = "전략 ID는 1-100자여야 합니다"))]
    pub strategy_id: String,
    /// 거래 심볼 (예: "BTC/USDT")
    #[validate(length(min = 1, max = 20, message = "심볼은 1-20자여야 합니다"))]
    pub symbol: String,
    /// 시작 날짜 (YYYY-MM-DD)
    #[validate(custom(function = "validate_date_format"))]
    pub start_date: String,
    /// 종료 날짜 (YYYY-MM-DD)
    #[validate(custom(function = "validate_date_format"))]
    pub end_date: String,
    /// 초기 자본금 (100 ~ 10억)
    #[validate(custom(function = "validate_initial_capital"))]
    pub initial_capital: Decimal,
    /// 수수료율 (선택, 기본: 0.001 = 0.1%, 최대: 10%)
    #[serde(default)]
    #[validate(custom(function = "validate_commission_rate"))]
    pub commission_rate: Option<Decimal>,
    /// 슬리피지율 (선택, 기본: 0.0005 = 0.05%, 최대: 5%)
    #[serde(default)]
    #[validate(custom(function = "validate_slippage_rate"))]
    pub slippage_rate: Option<Decimal>,
    /// 전략 파라미터 (선택)
    #[serde(default)]
    pub parameters: Option<serde_json::Value>,
    /// 다중 타임프레임 설정 (선택)
    /// 지정 시 secondary 타임프레임 데이터도 로드하여 전략에 전달
    #[serde(default)]
    pub multi_timeframe_config: Option<MultiTimeframeRequest>,
}

/// 다중 자산 백테스트 실행 요청
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct BacktestMultiRunRequest {
    /// 전략 ID
    #[validate(length(min = 1, max = 100, message = "전략 ID는 1-100자여야 합니다"))]
    pub strategy_id: String,
    /// 거래 심볼 목록 (예: ["TQQQ", "SCHD", "PFIX", "TMF"])
    #[validate(length(min = 1, max = 50, message = "심볼은 1-50개 사이여야 합니다"))]
    pub symbols: Vec<String>,
    /// 시작 날짜 (YYYY-MM-DD)
    #[validate(custom(function = "validate_date_format"))]
    pub start_date: String,
    /// 종료 날짜 (YYYY-MM-DD)
    #[validate(custom(function = "validate_date_format"))]
    pub end_date: String,
    /// 초기 자본금 (100 ~ 10억)
    #[validate(custom(function = "validate_initial_capital"))]
    pub initial_capital: Decimal,
    /// 수수료율 (선택, 기본: 0.001 = 0.1%, 최대: 10%)
    #[serde(default)]
    #[validate(custom(function = "validate_commission_rate"))]
    pub commission_rate: Option<Decimal>,
    /// 슬리피지율 (선택, 기본: 0.0005 = 0.05%, 최대: 5%)
    #[serde(default)]
    #[validate(custom(function = "validate_slippage_rate"))]
    pub slippage_rate: Option<Decimal>,
    /// 전략 파라미터 (선택)
    #[serde(default)]
    pub parameters: Option<serde_json::Value>,
}

/// 다중 자산 백테스트 실행 응답
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BacktestMultiRunResponse {
    /// 백테스트 결과 ID
    pub id: String,
    /// 성공 여부
    pub success: bool,
    /// 전략 ID
    pub strategy_id: String,
    /// 심볼 목록
    pub symbols: Vec<String>,
    /// 시작 날짜
    pub start_date: String,
    /// 종료 날짜
    pub end_date: String,
    /// 성과 지표
    pub metrics: BacktestMetricsResponse,
    /// 자산 곡선 (시간순)
    pub equity_curve: Vec<EquityCurvePoint>,
    /// 거래 내역
    pub trades: Vec<TradeHistoryItem>,
    /// 백테스트 설정 요약
    pub config_summary: BacktestConfigSummary,
    /// 심볼별 데이터 포인트 수
    pub data_points_by_symbol: HashMap<String, usize>,
}

/// 백테스트 성과 지표 응답
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export, export_to = "backtest/")]
pub struct BacktestMetricsResponse {
    /// 총 수익률 (%)
    #[ts(type = "number")]
    pub total_return_pct: Decimal,
    /// 연율화 수익률 (%)
    #[ts(type = "number")]
    pub annualized_return_pct: Decimal,
    /// 순수익
    #[ts(type = "number")]
    pub net_profit: Decimal,
    /// 총 거래 수
    pub total_trades: usize,
    /// 승률 (%)
    #[ts(type = "number")]
    pub win_rate_pct: Decimal,
    /// 프로핏 팩터
    #[ts(type = "number")]
    pub profit_factor: Decimal,
    /// 샤프 비율
    #[ts(type = "number")]
    pub sharpe_ratio: Decimal,
    /// 소르티노 비율
    #[ts(type = "number")]
    pub sortino_ratio: Decimal,
    /// 최대 낙폭 (%)
    #[ts(type = "number")]
    pub max_drawdown_pct: Decimal,
    /// 칼마 비율
    #[ts(type = "number")]
    pub calmar_ratio: Decimal,
    /// 평균 수익 거래
    #[ts(type = "number")]
    pub avg_win: Decimal,
    /// 평균 손실 거래
    #[ts(type = "number")]
    pub avg_loss: Decimal,
    /// 최대 수익 거래
    #[ts(type = "number")]
    pub largest_win: Decimal,
    /// 최대 손실 거래
    #[ts(type = "number")]
    pub largest_loss: Decimal,
}

/// 자산 곡선 데이터 포인트
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EquityCurvePoint {
    /// 타임스탬프 (Unix timestamp)
    pub timestamp: i64,
    /// 자산 가치
    pub equity: Decimal,
    /// 낙폭 (%)
    pub drawdown_pct: Decimal,
}

/// 거래 내역 항목
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TradeHistoryItem {
    /// 심볼
    pub symbol: String,
    /// 진입 시간
    pub entry_time: DateTime<Utc>,
    /// 청산 시간
    pub exit_time: DateTime<Utc>,
    /// 진입가
    pub entry_price: Decimal,
    /// 청산가
    pub exit_price: Decimal,
    /// 수량
    pub quantity: Decimal,
    /// 방향 (Buy/Sell)
    pub side: Side,
    /// 손익
    pub pnl: Decimal,
    /// 손익률 (%)
    pub return_pct: Decimal,
    /// 진입 이유 (전략의 논리적 목적)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_reason: Option<String>,
    /// 청산 이유 (전략의 논리적 목적)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_reason: Option<String>,
}

impl TradeInfo for TradeHistoryItem {
    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn pnl(&self) -> Option<Decimal> {
        Some(self.pnl)
    }

    fn fees(&self) -> Decimal {
        // TradeHistoryItem은 수수료 필드가 없음.
        // 백테스트에서는 pnl에 수수료가 이미 반영되어 있으므로 0 반환.
        Decimal::ZERO
    }

    fn entry_time(&self) -> DateTime<Utc> {
        self.entry_time
    }

    fn exit_time(&self) -> Option<DateTime<Utc>> {
        Some(self.exit_time)
    }
}

/// 개별 거래 결과 (매수 또는 매도 단일 거래)
/// BacktestRunResponse.all_trades에 사용
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "backtest/")]
pub struct TradeResultItem {
    /// 심볼
    pub symbol: String,
    /// 방향 (buy/sell)
    pub side: String,
    /// 신호 유형 (entry, exit, add_to_position 등)
    pub signal_type: String,
    /// 체결 수량
    pub quantity: String,
    /// 체결 가격
    pub price: String,
    /// 수수료
    pub commission: String,
    /// 슬리피지 금액
    pub slippage: String,
    /// 체결 시간 (ISO 8601)
    pub timestamp: String,
    /// 실현 손익 (청산 거래인 경우)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realized_pnl: Option<String>,
    /// 분할 매수/매도 여부
    pub is_partial: bool,
    /// 메타데이터
    pub metadata: std::collections::HashMap<String, String>,
}

/// 백테스트 실행 응답
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BacktestRunResponse {
    /// 백테스트 결과 ID
    pub id: String,
    /// 성공 여부
    pub success: bool,
    /// 전략 ID
    pub strategy_id: String,
    /// 심볼
    pub symbol: String,
    /// 시작 날짜
    pub start_date: String,
    /// 종료 날짜
    pub end_date: String,
    /// 성과 지표
    pub metrics: BacktestMetricsResponse,
    /// 자산 곡선 (시간순)
    pub equity_curve: Vec<EquityCurvePoint>,
    /// 거래 내역 (완료된 라운드트립)
    pub trades: Vec<TradeHistoryItem>,
    /// 백테스트 설정 요약
    pub config_summary: BacktestConfigSummary,
    /// 모든 거래 기록 (매수/매도 개별 거래 포함) - 매매일지용
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub all_trades: Vec<TradeResultItem>,
}

/// 백테스트 설정 요약
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BacktestConfigSummary {
    /// 초기 자본금
    pub initial_capital: Decimal,
    /// 수수료율
    pub commission_rate: Decimal,
    /// 슬리피지율
    pub slippage_rate: Decimal,
    /// 총 수수료
    pub total_commission: Decimal,
    /// 총 슬리피지 비용
    pub total_slippage: Decimal,
    /// 데이터 포인트 수
    pub data_points: usize,
}

/// API 에러 응답
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BacktestApiError {
    /// 에러 코드
    pub code: String,
    /// 에러 메시지
    pub message: String,
}

impl BacktestApiError {
    /// 새로운 에러 생성
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

// ==================== 배치 백테스트 (병렬 실행) ====================

/// 배치 백테스트 요청 항목.
///
/// 단일 전략의 백테스트 설정을 정의합니다.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct BatchBacktestItem {
    /// 전략 ID
    #[validate(length(min = 1, max = 100))]
    pub strategy_id: String,
    /// 심볼 (단일 자산) 또는 심볼 목록 (다중 자산)
    pub symbols: Vec<String>,
    /// 전략 파라미터 (선택)
    #[serde(default)]
    pub parameters: Option<serde_json::Value>,
}

/// 배치 백테스트 요청.
///
/// 여러 전략을 병렬로 실행합니다.
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct BatchBacktestRequest {
    /// 백테스트할 전략 목록 (최대 10개)
    #[validate(length(min = 1, max = 10, message = "전략은 1-10개 사이여야 합니다"))]
    #[validate(nested)]
    pub strategies: Vec<BatchBacktestItem>,
    /// 시작 날짜 (YYYY-MM-DD)
    #[validate(custom(function = "validate_date_format"))]
    pub start_date: String,
    /// 종료 날짜 (YYYY-MM-DD)
    #[validate(custom(function = "validate_date_format"))]
    pub end_date: String,
    /// 초기 자본금 (모든 전략에 동일하게 적용, 100 ~ 10억)
    #[validate(custom(function = "validate_initial_capital"))]
    pub initial_capital: Decimal,
    /// 수수료율 (선택, 기본: 0.001, 최대: 10%)
    #[serde(default)]
    #[validate(custom(function = "validate_commission_rate"))]
    pub commission_rate: Option<Decimal>,
    /// 슬리피지율 (선택, 기본: 0.0005, 최대: 5%)
    #[serde(default)]
    #[validate(custom(function = "validate_slippage_rate"))]
    pub slippage_rate: Option<Decimal>,
    /// 병렬 실행 수 (선택, 기본: 4, 최대: 10)
    #[serde(default)]
    #[validate(range(min = 1, max = 10))]
    pub parallelism: Option<usize>,
}

/// 배치 백테스트 결과 항목.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchBacktestResultItem {
    /// 전략 ID
    pub strategy_id: String,
    /// 성공 여부
    pub success: bool,
    /// 에러 메시지 (실패 시)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// 성과 지표 (성공 시)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<BacktestMetricsResponse>,
    /// 실행 시간 (밀리초)
    pub execution_time_ms: u64,
}

/// 배치 백테스트 응답.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchBacktestResponse {
    /// 요청 ID
    pub request_id: String,
    /// 총 전략 수
    pub total_strategies: usize,
    /// 성공 수
    pub successful: usize,
    /// 실패 수
    pub failed: usize,
    /// 총 실행 시간 (밀리초)
    pub total_execution_time_ms: u64,
    /// 각 전략별 결과
    pub results: Vec<BatchBacktestResultItem>,
}
