//! UI 스키마 로더
//!
//! JSON 파일에서 전략별 SDUI 스키마를 동적으로 로드합니다.
//! 이를 통해 Rust 재빌드 없이 UI 스키마를 수정할 수 있습니다.

#![allow(dead_code)] // SDUI 스키마 유틸리티는 프론트엔드 통합 시 사용 예정

use super::types::{
    UiCondition, UiConditionOperator, UiField, UiFieldGroup, UiFieldType, UiLayout, UiSchema,
    UiSelectOption, UiValidation,
};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use tracing::{debug, error, warn};

// ==================== JSON 스키마 타입 ====================

#[derive(Debug, Deserialize)]
pub struct JsonSchemaFile {
    pub version: String,
    pub common: JsonCommonSchema,
    pub strategies: HashMap<String, JsonStrategySchema>,
}

#[derive(Debug, Deserialize)]
pub struct JsonCommonSchema {
    pub risk_groups: Vec<JsonFieldGroup>,
    pub risk_fields: Vec<JsonField>,
}

#[derive(Debug, Deserialize)]
pub struct JsonStrategySchema {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub layout: Option<JsonLayout>,
    #[serde(default)]
    pub risk_defaults: JsonRiskDefaults,
    #[serde(default)]
    pub groups: Vec<JsonFieldGroup>,
    #[serde(default)]
    pub fields: Vec<JsonField>,
}

#[derive(Debug, Deserialize, Default)]
pub struct JsonRiskDefaults {
    #[serde(default = "default_stop_loss")]
    pub stop_loss_pct: f64,
    #[serde(default = "default_take_profit")]
    pub take_profit_pct: f64,
    #[serde(default)]
    pub use_trailing_stop: bool,
    #[serde(default = "default_trailing_stop")]
    pub trailing_stop_pct: f64,
    #[serde(default = "default_max_position")]
    pub max_position_pct: f64,
    #[serde(default = "default_daily_loss")]
    pub daily_loss_limit_pct: f64,
}

fn default_stop_loss() -> f64 {
    3.0
}
fn default_take_profit() -> f64 {
    5.0
}
fn default_trailing_stop() -> f64 {
    2.0
}
fn default_max_position() -> f64 {
    10.0
}
fn default_daily_loss() -> f64 {
    3.0
}

#[derive(Debug, Deserialize)]
pub struct JsonLayout {
    pub columns: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JsonFieldGroup {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub collapsed: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JsonField {
    pub key: String,
    pub label: String,
    pub field_type: String,
    #[serde(default)]
    pub default_value: Option<serde_json::Value>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub help_text: Option<String>,
    #[serde(default)]
    pub validation: Option<JsonValidation>,
    #[serde(default)]
    pub options: Option<Vec<JsonSelectOption>>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub show_when: Option<JsonCondition>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct JsonValidation {
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub step: Option<f64>,
    #[serde(default)]
    pub min_length: Option<usize>,
    #[serde(default)]
    pub max_length: Option<usize>,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub min_items: Option<usize>,
    #[serde(default)]
    pub max_items: Option<usize>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JsonSelectOption {
    pub label: String,
    pub value: serde_json::Value,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JsonCondition {
    pub field: String,
    pub operator: String,
    pub value: serde_json::Value,
}

// ==================== 스키마 캐시 ====================

static SCHEMA_CACHE: Lazy<Option<JsonSchemaFile>> = Lazy::new(|| {
    let paths = [
        "config/sdui/strategy_schemas.json",
        "../config/sdui/strategy_schemas.json",
        "../../config/sdui/strategy_schemas.json",
    ];

    for path in &paths {
        if let Ok(content) = fs::read_to_string(path) {
            match serde_json::from_str::<JsonSchemaFile>(&content) {
                Ok(schema) => {
                    debug!(
                        "SDUI 스키마 로드 완료: {} 전략, 버전 {}",
                        schema.strategies.len(),
                        schema.version
                    );
                    return Some(schema);
                }
                Err(e) => {
                    error!("SDUI 스키마 파싱 오류 ({}): {}", path, e);
                }
            }
        }
    }

    warn!("SDUI 스키마 파일을 찾을 수 없습니다. 기본값 사용.");
    None
});

// ==================== 변환 함수 ====================

fn convert_field_type(type_str: &str) -> UiFieldType {
    match type_str {
        "Number" => UiFieldType::Number,
        "Boolean" => UiFieldType::Boolean,
        "Select" => UiFieldType::Select,
        "Range" => UiFieldType::Range,
        "SymbolPicker" => UiFieldType::SymbolPicker,
        "SymbolCategoryGroup" => UiFieldType::SymbolCategoryGroup,
        _ => UiFieldType::Text,
    }
}

fn convert_condition_operator(op_str: &str) -> UiConditionOperator {
    match op_str.to_lowercase().as_str() {
        "equals" | "eq" | "==" => UiConditionOperator::Equals,
        "not_equals" | "ne" | "!=" => UiConditionOperator::NotEquals,
        "greater_than" | "gt" | ">" => UiConditionOperator::GreaterThan,
        "less_than" | "lt" | "<" => UiConditionOperator::LessThan,
        "contains" => UiConditionOperator::Contains,
        _ => UiConditionOperator::Equals,
    }
}

fn convert_validation(json: &Option<JsonValidation>) -> UiValidation {
    match json {
        Some(v) => UiValidation {
            required: v.required,
            min: v.min,
            max: v.max,
            step: v.step,
            min_length: v.min_length,
            max_length: v.max_length,
            pattern: v.pattern.clone(),
            min_items: v.min_items,
            max_items: v.max_items,
        },
        None => UiValidation::default(),
    }
}

fn convert_condition(json: &Option<JsonCondition>) -> Option<UiCondition> {
    json.as_ref().map(|c| UiCondition {
        field: c.field.clone(),
        operator: convert_condition_operator(&c.operator),
        value: c.value.clone(),
    })
}

fn convert_options(json: &Option<Vec<JsonSelectOption>>) -> Option<Vec<UiSelectOption>> {
    json.as_ref().map(|opts| {
        opts.iter()
            .map(|o| UiSelectOption {
                label: o.label.clone(),
                value: o.value.clone(),
                description: o.description.clone(),
            })
            .collect()
    })
}

fn convert_field(json: &JsonField) -> UiField {
    UiField {
        key: json.key.clone(),
        label: json.label.clone(),
        field_type: convert_field_type(&json.field_type),
        default_value: json.default_value.clone(),
        placeholder: json.placeholder.clone(),
        help_text: json.help_text.clone(),
        validation: convert_validation(&json.validation),
        options: convert_options(&json.options),
        symbol_categories: None,
        group: json.group.clone(),
        order: json.order,
        show_when: convert_condition(&json.show_when),
        unit: json.unit.clone(),
    }
}

fn convert_group(json: &JsonFieldGroup) -> UiFieldGroup {
    UiFieldGroup {
        id: json.id.clone(),
        label: json.label.clone(),
        description: json.description.clone(),
        order: json.order,
        collapsed: json.collapsed,
    }
}

// ==================== 리스크 관리 필드 적용 ====================

fn apply_risk_defaults(fields: &mut [UiField], defaults: &JsonRiskDefaults) {
    for field in fields.iter_mut() {
        match field.key.as_str() {
            "stop_loss_pct" => {
                field.default_value = Some(serde_json::json!(defaults.stop_loss_pct));
            }
            "take_profit_pct" => {
                field.default_value = Some(serde_json::json!(defaults.take_profit_pct));
            }
            "use_trailing_stop" => {
                field.default_value = Some(serde_json::json!(defaults.use_trailing_stop));
            }
            "trailing_stop_pct" => {
                field.default_value = Some(serde_json::json!(defaults.trailing_stop_pct));
            }
            "max_position_pct" => {
                field.default_value = Some(serde_json::json!(defaults.max_position_pct));
            }
            "daily_loss_limit_pct" => {
                field.default_value = Some(serde_json::json!(defaults.daily_loss_limit_pct));
            }
            _ => {}
        }
    }
}

// ==================== 공개 API ====================

/// JSON에서 전략 UI 스키마 조회
pub fn get_ui_schema_for_strategy(strategy_id: &str) -> Option<UiSchema> {
    let schema_file = SCHEMA_CACHE.as_ref()?;

    let strategy = schema_file.strategies.get(strategy_id)?;

    // 전략 고유 필드 변환
    let mut fields: Vec<UiField> = strategy.fields.iter().map(convert_field).collect();

    // 공통 리스크 관리 필드 추가
    let mut risk_fields: Vec<UiField> = schema_file
        .common
        .risk_fields
        .iter()
        .map(convert_field)
        .collect();

    // 전략별 리스크 기본값 적용
    apply_risk_defaults(&mut risk_fields, &strategy.risk_defaults);

    fields.extend(risk_fields);

    // 전략 고유 그룹 변환
    let mut groups: Vec<UiFieldGroup> = strategy.groups.iter().map(convert_group).collect();

    // 공통 리스크 관리 그룹 추가
    let risk_groups: Vec<UiFieldGroup> = schema_file
        .common
        .risk_groups
        .iter()
        .map(convert_group)
        .collect();

    groups.extend(risk_groups);

    // 레이아웃
    let layout = strategy.layout.as_ref().map(|l| UiLayout {
        columns: l.columns as usize,
    });

    Some(UiSchema {
        fields,
        groups,
        layout,
    })
}

/// 사용 가능한 모든 전략 ID 목록 조회
pub fn get_available_strategy_ids() -> Vec<String> {
    match SCHEMA_CACHE.as_ref() {
        Some(schema) => schema.strategies.keys().cloned().collect(),
        None => Vec::new(),
    }
}

/// 전략 메타데이터 조회 (이름, 설명)
pub fn get_strategy_metadata(strategy_id: &str) -> Option<(String, String)> {
    let schema_file = SCHEMA_CACHE.as_ref()?;
    let strategy = schema_file.strategies.get(strategy_id)?;
    Some((strategy.name.clone(), strategy.description.clone()))
}

/// 전략별 기본 리스크 설정 조회
pub fn get_strategy_risk_defaults(strategy_id: &str) -> Option<JsonRiskDefaults> {
    let schema_file = SCHEMA_CACHE.as_ref()?;
    let strategy = schema_file.strategies.get(strategy_id)?;
    Some(JsonRiskDefaults {
        stop_loss_pct: strategy.risk_defaults.stop_loss_pct,
        take_profit_pct: strategy.risk_defaults.take_profit_pct,
        use_trailing_stop: strategy.risk_defaults.use_trailing_stop,
        trailing_stop_pct: strategy.risk_defaults.trailing_stop_pct,
        max_position_pct: strategy.risk_defaults.max_position_pct,
        daily_loss_limit_pct: strategy.risk_defaults.daily_loss_limit_pct,
    })
}

/// 스키마 파일 다시 로드 (런타임 핫 리로드용)
pub fn reload_schemas() -> Result<(), String> {
    // Lazy<T>는 한 번만 초기화되므로, 완전한 핫 리로드를 위해서는
    // RwLock을 사용하거나 서버 재시작이 필요합니다.
    // 현재는 정보 로깅만 수행합니다.
    warn!("스키마 핫 리로드는 서버 재시작이 필요합니다.");
    Ok(())
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_loading() {
        // JSON 파일이 존재하면 로드 테스트
        if let Some(schema) = SCHEMA_CACHE.as_ref() {
            assert!(!schema.strategies.is_empty());
            assert!(!schema.common.risk_fields.is_empty());
            assert!(!schema.common.risk_groups.is_empty());
        }
    }

    #[test]
    fn test_get_ui_schema() {
        if SCHEMA_CACHE.is_some() {
            let schema = get_ui_schema_for_strategy("rsi_mean_reversion");
            assert!(schema.is_some());

            let schema = schema.unwrap();
            assert!(!schema.fields.is_empty());
            assert!(!schema.groups.is_empty());

            // 리스크 관리 필드가 포함되어 있는지 확인
            let has_stop_loss = schema.fields.iter().any(|f| f.key == "stop_loss_pct");
            assert!(has_stop_loss, "리스크 관리 필드가 포함되어야 함");
        }
    }

    #[test]
    fn test_field_type_conversion() {
        assert!(matches!(convert_field_type("Number"), UiFieldType::Number));
        assert!(matches!(
            convert_field_type("Boolean"),
            UiFieldType::Boolean
        ));
        assert!(matches!(convert_field_type("Select"), UiFieldType::Select));
        assert!(matches!(
            convert_field_type("SymbolPicker"),
            UiFieldType::SymbolPicker
        ));
        assert!(matches!(convert_field_type("Unknown"), UiFieldType::Text));
    }
}
