//! SDUI (Server-Driven UI) 스키마 정의.
//!
//! 이 모듈은 전략 설정 UI를 자동 생성하기 위한 스키마 타입을 제공합니다.
//! Fragment 기반 재사용과 매크로 기반 자동 생성을 지원합니다.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(feature = "ts-rs-support")]
use ts_rs::TS;

/// Fragment 카테고리.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs-support", derive(TS))]
#[cfg_attr(feature = "ts-rs-support", ts(export, export_to = "sdui/"))]
#[serde(rename_all = "snake_case")]
pub enum FragmentCategory {
    /// 기술적 지표 (RSI, MACD, Bollinger Bands 등)
    Indicator,
    /// 필터 조건 (RouteState, MarketRegime, Volume 등)
    Filter,
    /// 리스크 관리 (손절, 익절, 트레일링 스탑)
    RiskManagement,
    /// 포지션 크기 결정 (고정 비율, Kelly, ATR 기반)
    PositionSizing,
    /// 타이밍 설정 (리밸런싱 주기, 거래 시간)
    Timing,
    /// 자산 선택 (단일 심볼, 유니버스)
    Asset,
}

/// 필드 타입.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs-support", derive(TS))]
#[cfg_attr(feature = "ts-rs-support", ts(export, export_to = "sdui/"))]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    /// 정수형
    Integer,
    /// 실수형
    Number,
    /// 불리언
    Boolean,
    /// 문자열
    String,
    /// 단일 선택 (드롭다운)
    Select,
    /// 다중 선택 (체크박스)
    #[serde(rename = "multi_select")]
    MultiSelect,
    /// 심볼 입력 (자동완성)
    Symbol,
    /// 심볼 배열
    Symbols,
    /// 다중 타임프레임 (Primary + Secondary)
    #[serde(rename = "multi_timeframe")]
    MultiTimeframe,
}

/// 필드 스키마.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs-support", derive(TS))]
#[cfg_attr(feature = "ts-rs-support", ts(export, export_to = "sdui/"))]
pub struct FieldSchema {
    /// 필드 이름 (snake_case)
    pub name: String,

    /// 필드 타입 (프론트엔드와 동일한 키 이름 사용)
    pub field_type: FieldType,

    /// 표시 라벨 (한글)
    pub label: String,

    /// 설명 (옵션)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs-support", ts(optional))]
    pub description: Option<String>,

    /// 기본값 (JSON 값)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs-support", ts(optional, type = "unknown"))]
    pub default: Option<serde_json::Value>,

    /// 최소값 (number/integer 타입)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs-support", ts(optional))]
    pub min: Option<f64>,

    /// 최대값 (number/integer 타입)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs-support", ts(optional))]
    pub max: Option<f64>,

    /// 선택 옵션 (select/multi_select 타입)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub options: Vec<String>,

    /// 조건부 표시 (예: "enabled == true")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs-support", ts(optional))]
    pub condition: Option<String>,

    /// 필수 여부
    #[serde(default)]
    pub required: bool,

    /// 숨김 여부 (UI에서 표시하지 않음)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,

    /// 표시 순서 (낮을수록 먼저 표시)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs-support", ts(optional))]
    pub order: Option<i32>,

    /// 섹션 ID (같은 섹션의 필드들을 UI에서 그룹화)
    ///
    /// 예: "indicator" → "기술적 지표" 섹션으로 그룹화
    /// 예: "filter" → "필터 조건" 섹션으로 그룹화
    /// 예: "asset" → "자산 선택" 섹션으로 그룹화
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs-support", ts(optional))]
    pub section: Option<String>,
}

impl Default for FieldSchema {
    fn default() -> Self {
        Self {
            name: String::new(),
            field_type: FieldType::String,
            label: String::new(),
            description: None,
            default: None,
            min: None,
            max: None,
            options: Vec::new(),
            condition: None,
            required: false,
            hidden: false,
            order: None,
            section: None,
        }
    }
}

/// 재사용 가능한 UI 스키마 조각 (Fragment).
///
/// Fragment는 여러 전략에서 공통으로 사용되는 설정 그룹입니다.
/// 예: RSI 설정, 트레일링 스탑 설정 등
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs-support", derive(TS))]
#[cfg_attr(feature = "ts-rs-support", ts(export, export_to = "sdui/"))]
pub struct SchemaFragment {
    /// Fragment ID (예: "indicator.rsi", "risk.trailing_stop")
    pub id: String,

    /// Fragment 이름 (한글)
    pub name: String,

    /// 설명
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs-support", ts(optional))]
    pub description: Option<String>,

    /// 카테고리
    pub category: FragmentCategory,

    /// 필드 목록
    pub fields: Vec<FieldSchema>,

    /// 다른 Fragment에 대한 의존성
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dependencies: Vec<String>,
}

impl SchemaFragment {
    /// 새로운 Fragment를 생성합니다.
    pub fn new(id: impl Into<String>, name: impl Into<String>, category: FragmentCategory) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            category,
            fields: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    /// 설명을 설정합니다.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 필드를 추가합니다.
    pub fn with_field(mut self, field: FieldSchema) -> Self {
        self.fields.push(field);
        self
    }

    /// 필드 목록을 설정합니다.
    pub fn with_fields(mut self, fields: Vec<FieldSchema>) -> Self {
        self.fields = fields;
        self
    }

    /// 의존성을 추가합니다.
    pub fn with_dependency(mut self, dep: impl Into<String>) -> Self {
        self.dependencies.push(dep.into());
        self
    }
}

/// Fragment 참조.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs-support", derive(TS))]
#[cfg_attr(feature = "ts-rs-support", ts(export, export_to = "sdui/"))]
pub struct FragmentRef {
    /// Fragment ID
    pub id: String,

    /// 필수 여부
    #[serde(default)]
    pub required: bool,
}

impl FragmentRef {
    /// 필수 Fragment 참조를 생성합니다.
    pub fn required(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            required: true,
        }
    }

    /// 선택적 Fragment 참조를 생성합니다.
    pub fn optional(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            required: false,
        }
    }
}

/// 전략 UI 스키마.
///
/// 전략의 완전한 UI 구성을 나타냅니다.
/// Fragment 참조 + 커스텀 필드로 구성됩니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs-support", derive(TS))]
#[cfg_attr(feature = "ts-rs-support", ts(export, export_to = "sdui/"))]
pub struct StrategyUISchema {
    /// 전략 ID
    pub id: String,

    /// 전략 이름 (한글)
    pub name: String,

    /// 설명
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs-support", ts(optional))]
    pub description: Option<String>,

    /// 전략 카테고리
    pub category: String,

    /// 사용하는 Fragment 목록
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub fragments: Vec<FragmentRef>,

    /// 전략 고유 커스텀 필드
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub custom_fields: Vec<FieldSchema>,

    /// 기본 설정값 (옵션)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs-support", ts(optional, type = "Record<string, unknown> | null"))]
    pub defaults: Option<HashMap<String, serde_json::Value>>,
}

impl StrategyUISchema {
    /// 새로운 전략 UI 스키마를 생성합니다.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        category: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            category: category.into(),
            fragments: Vec::new(),
            custom_fields: Vec::new(),
            defaults: None,
        }
    }

    /// 설명을 설정합니다.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Fragment를 추가합니다.
    pub fn with_fragment(mut self, fragment_ref: FragmentRef) -> Self {
        self.fragments.push(fragment_ref);
        self
    }

    /// 커스텀 필드를 추가합니다.
    pub fn with_custom_field(mut self, field: FieldSchema) -> Self {
        self.custom_fields.push(field);
        self
    }

    /// 기본 설정값을 설정합니다.
    pub fn with_defaults(mut self, defaults: HashMap<String, serde_json::Value>) -> Self {
        self.defaults = Some(defaults);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_fragment_creation() {
        let fragment =
            SchemaFragment::new("indicator.rsi", "RSI 설정", FragmentCategory::Indicator)
                .with_description("RSI 지표 설정")
                .with_field(FieldSchema {
                    name: "period".to_string(),
                    field_type: FieldType::Integer,
                    label: "RSI 기간".to_string(),
                    default: Some(json!(14)),
                    min: Some(2.0),
                    max: Some(100.0),
                    ..Default::default()
                });

        assert_eq!(fragment.id, "indicator.rsi");
        assert_eq!(fragment.fields.len(), 1);
        assert_eq!(fragment.fields[0].name, "period");
    }

    #[test]
    fn test_strategy_ui_schema() {
        let schema = StrategyUISchema::new("rsi_mean_reversion", "RSI 평균회귀", "single_asset")
            .with_description("RSI 과매수/과매도 구간에서 평균회귀 매매")
            .with_fragment(FragmentRef::required("indicator.rsi"))
            .with_custom_field(FieldSchema {
                name: "cooldown_candles".to_string(),
                field_type: FieldType::Integer,
                label: "쿨다운 캔들 수".to_string(),
                default: Some(json!(5)),
                min: Some(0.0),
                max: Some(100.0),
                ..Default::default()
            });

        assert_eq!(schema.id, "rsi_mean_reversion");
        assert_eq!(schema.fragments.len(), 1);
        assert_eq!(schema.custom_fields.len(), 1);
    }

    #[test]
    fn test_fragment_ref() {
        let required = FragmentRef::required("indicator.rsi");
        assert!(required.required);

        let optional = FragmentRef::optional("filter.route_state");
        assert!(!optional.required);
    }

    #[test]
    fn test_serialization() {
        let fragment = SchemaFragment::new("test", "테스트", FragmentCategory::Indicator);
        let json = serde_json::to_string(&fragment).unwrap();
        let deserialized: SchemaFragment = serde_json::from_str(&json).unwrap();

        assert_eq!(fragment.id, deserialized.id);
        assert_eq!(fragment.category, deserialized.category);
    }
}
