//! SDUI 스키마 조합기.
//!
//! StrategyUISchema와 FragmentRegistry를 조합하여
//! 프론트엔드에서 렌더링할 수 있는 완전한 SDUI JSON을 생성합니다.

use serde_json::{json, Value};
use std::sync::Arc;
use trader_core::{FieldSchema, SchemaFragment, StrategyUISchema};

use crate::FragmentRegistry;

/// 스키마 조합기.
///
/// StrategyUISchema + FragmentRegistry → 완성된 SDUI JSON
pub struct SchemaComposer {
    registry: Arc<FragmentRegistry>,
}

impl SchemaComposer {
    /// 새로운 SchemaComposer를 생성합니다.
    pub fn new(registry: Arc<FragmentRegistry>) -> Self {
        Self { registry }
    }

    /// 기본 FragmentRegistry를 사용하는 SchemaComposer를 생성합니다.
    pub fn with_default_registry() -> Self {
        Self::new(Arc::new(FragmentRegistry::with_builtins()))
    }

    /// StrategyUISchema를 완성된 SDUI JSON으로 변환합니다.
    ///
    /// # Arguments
    /// * `strategy_schema` - 전략 UI 스키마
    ///
    /// # Returns
    /// 프론트엔드에서 렌더링할 수 있는 완전한 SDUI JSON
    /// (ts-rs 자동 생성 타입과 일치하는 구조)
    pub fn compose(&self, strategy_schema: &StrategyUISchema) -> Value {
        // Fragment 참조 목록
        let fragments: Vec<Value> = strategy_schema
            .fragments
            .iter()
            .map(|frag_ref| {
                json!({
                    "id": frag_ref.id,
                    "required": frag_ref.required
                })
            })
            .collect();

        // 커스텀 필드 목록
        let custom_fields: Vec<Value> = strategy_schema
            .custom_fields
            .iter()
            .map(|f| self.field_to_json(f))
            .collect();

        // 기본값 맵 (custom_fields에서 추출)
        let mut defaults = serde_json::Map::new();
        for field in &strategy_schema.custom_fields {
            if let Some(ref default_val) = field.default {
                defaults.insert(field.name.clone(), default_val.clone());
            }
        }

        // ts-rs 자동 생성 타입(StrategyUISchema)과 일치하는 구조로 반환
        json!({
            "id": strategy_schema.id,
            "name": strategy_schema.name,
            "description": strategy_schema.description,
            "category": strategy_schema.category,
            "fragments": fragments,
            "custom_fields": custom_fields,
            "defaults": defaults
        })
    }

    /// Fragment를 섹션으로 변환합니다.
    ///
    /// FragmentRegistry에서 Fragment를 조회하여 섹션 JSON으로 변환합니다.
    /// 향후 compose 메서드에서 Fragment 확장 시 사용됩니다.
    pub fn fragment_to_section(&self, fragment: &SchemaFragment, required: bool) -> Value {
        json!({
            "id": fragment.id,
            "name": fragment.name,
            "description": fragment.description,
            "category": format!("{:?}", fragment.category),
            "required": required,
            "collapsible": !required,
            "fields": fragment.fields.iter().map(|f| self.field_to_json(f)).collect::<Vec<_>>()
        })
    }

    /// 커스텀 필드를 섹션으로 변환합니다.
    ///
    /// 전략별 고유 필드들을 커스텀 섹션으로 그룹화합니다.
    pub fn custom_fields_section(&self, fields: &[FieldSchema]) -> Value {
        json!({
            "id": "custom",
            "name": "커스텀 설정",
            "description": "전략별 고유 설정",
            "required": false,
            "collapsible": true,
            "fields": fields.iter().map(|f| self.field_to_json(f)).collect::<Vec<_>>()
        })
    }

    /// FieldSchema를 JSON으로 변환합니다.
    ///
    /// serde를 통해 직렬화하여 모든 serde 속성(rename_all, rename 등)이
    /// 올바르게 적용되도록 합니다.
    fn field_to_json(&self, field: &FieldSchema) -> Value {
        // serde 직렬화를 통해 모든 속성이 올바르게 적용됨
        // (예: MultiSelect → "multi_select", MultiTimeframe → "multi_timeframe")
        serde_json::to_value(field).unwrap_or_else(|_| {
            // fallback: 직접 JSON 생성
            json!({
                "name": field.name,
                "field_type": "string",
                "label": field.label,
                "required": field.required,
            })
        })
    }

    /// Fragment 카탈로그를 JSON으로 반환합니다.
    ///
    /// 사용 가능한 모든 Fragment 목록을 프론트엔드에 제공합니다.
    pub fn get_fragment_catalog(&self) -> Value {
        let fragments: Vec<Value> = self
            .registry
            .list_all()
            .iter()
            .map(|frag| {
                json!({
                    "id": frag.id,
                    "name": frag.name,
                    "description": frag.description,
                    "category": format!("{:?}", frag.category),
                    "dependencies": frag.dependencies,
                    "fields": frag.fields.iter().map(|f| self.field_to_json(f)).collect::<Vec<_>>()
                })
            })
            .collect();

        json!({
            "fragments": fragments
        })
    }

    /// 카테고리별 Fragment 목록을 JSON으로 반환합니다.
    pub fn get_fragments_by_category(&self, category: trader_core::FragmentCategory) -> Value {
        let fragments: Vec<Value> = self
            .registry
            .list_by_category(category)
            .iter()
            .map(|frag| {
                json!({
                    "id": frag.id,
                    "name": frag.name,
                    "description": frag.description,
                    "category": format!("{:?}", frag.category),
                    "fields": frag.fields.iter().map(|f| self.field_to_json(f)).collect::<Vec<_>>()
                })
            })
            .collect();

        json!({
            "category": format!("{:?}", category),
            "fragments": fragments
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trader_core::{FieldType, FragmentRef, StrategyUISchema};

    #[test]
    fn test_compose_basic_schema() {
        let composer = SchemaComposer::with_default_registry();

        let schema = StrategyUISchema::new("test_strategy", "테스트 전략", "single_asset")
            .with_description("테스트용 전략")
            .with_fragment(FragmentRef::required("indicator.rsi"));

        let json = composer.compose(&schema);

        // compose()는 id 필드 반환 (strategy_id 아님)
        assert_eq!(json["id"], "test_strategy");
        assert_eq!(json["name"], "테스트 전략");
        assert!(json["fragments"].is_array());

        let fragments = json["fragments"].as_array().unwrap();
        assert_eq!(fragments.len(), 1); // RSI fragment만
        assert_eq!(fragments[0]["id"], "indicator.rsi");
        assert_eq!(fragments[0]["required"], true);
    }

    #[test]
    fn test_compose_with_custom_fields() {
        let composer = SchemaComposer::with_default_registry();

        let schema = StrategyUISchema::new("test_strategy", "테스트 전략", "single_asset")
            .with_fragment(FragmentRef::required("indicator.rsi"))
            .with_custom_field(FieldSchema {
                name: "cooldown_candles".to_string(),
                field_type: FieldType::Integer,
                label: "쿨다운 캔들 수".to_string(),
                default: Some(json!(5)),
                min: Some(0.0),
                max: Some(100.0),
                required: true,
                ..Default::default()
            });

        let json = composer.compose(&schema);

        // fragments와 custom_fields가 별도 배열로 반환됨
        let fragments = json["fragments"].as_array().unwrap();
        assert_eq!(fragments.len(), 1); // RSI fragment만

        let custom_fields = json["custom_fields"].as_array().unwrap();
        assert_eq!(custom_fields.len(), 1);
        assert_eq!(custom_fields[0]["name"], "cooldown_candles");
        assert_eq!(custom_fields[0]["label"], "쿨다운 캔들 수");

        // defaults에 기본값이 포함됨
        assert_eq!(json["defaults"]["cooldown_candles"], 5);
    }

    #[test]
    fn test_fragment_catalog() {
        let composer = SchemaComposer::with_default_registry();
        let catalog = composer.get_fragment_catalog();

        assert!(catalog["fragments"].is_array());
        let fragments = catalog["fragments"].as_array().unwrap();
        assert!(fragments.len() > 0);

        // RSI fragment가 있는지 확인
        let rsi_fragment = fragments.iter().find(|f| f["id"] == "indicator.rsi");
        assert!(rsi_fragment.is_some());
    }

    #[test]
    fn test_fragments_by_category() {
        let composer = SchemaComposer::with_default_registry();
        let indicators =
            composer.get_fragments_by_category(trader_core::FragmentCategory::Indicator);

        assert!(indicators["fragments"].is_array());
        let fragments = indicators["fragments"].as_array().unwrap();
        assert!(fragments.len() >= 5); // RSI, MACD, BB, MA, ATR

        // 모든 fragment가 Indicator 카테고리인지 확인
        for frag in fragments {
            assert_eq!(frag["category"], "Indicator");
        }
    }

    #[test]
    fn test_optional_fragment() {
        let composer = SchemaComposer::with_default_registry();

        let schema = StrategyUISchema::new("test_strategy", "테스트 전략", "single_asset")
            .with_fragment(FragmentRef::required("indicator.rsi"))
            .with_fragment(FragmentRef::optional("filter.route_state"));

        let json = composer.compose(&schema);

        let fragments = json["fragments"].as_array().unwrap();
        assert_eq!(fragments.len(), 2);

        // 첫 번째는 필수
        assert_eq!(fragments[0]["id"], "indicator.rsi");
        assert_eq!(fragments[0]["required"], true);

        // 두 번째는 선택적
        assert_eq!(fragments[1]["id"], "filter.route_state");
        assert_eq!(fragments[1]["required"], false);
    }
}
