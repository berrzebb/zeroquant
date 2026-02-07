//! 전략 레지스트리 시스템
//!
//! 컴파일 타임에 모든 전략을 자동으로 수집하고 관리합니다.
//! `inventory` crate를 사용하여 전략 메타데이터를 자동 등록합니다.

use serde::{Deserialize, Serialize};
use trader_core::{MarketType, StrategyUISchema};

/// 전략 카테고리
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StrategyCategory {
    /// 실시간 전략 (1m, 그리드, 무한매수 등)
    Realtime,
    /// 분봉 전략 (15m, RSI, 볼린저 등)
    Intraday,
    /// 일봉 전략 (변동성 돌파, 섹터 등)
    Daily,
    /// 월봉 전략 (자산배분, HAA, BAA 등)
    Monthly,
}

/// 전략 메타데이터 (컴파일 타임 상수)
///
/// 각 전략은 `register_strategy!` 매크로를 통해 자동으로 등록됩니다.
#[derive(Clone)]
pub struct StrategyMeta {
    /// 전략 ID (영문, snake_case)
    pub id: &'static str,

    /// 별칭 (여러 이름으로 접근 가능)
    pub aliases: &'static [&'static str],

    /// 전략 이름 (한글)
    pub name: &'static str,

    /// 전략 설명
    pub description: &'static str,

    /// 기본 타임프레임 (Primary)
    pub default_timeframe: &'static str,

    /// 보조 타임프레임 (Secondary, 다중 TF 전략용)
    ///
    /// 빈 배열 = 단일 타임프레임 전략
    /// 예: ["1h", "1d"] = Primary 외에 1시간봉과 일봉도 사용
    pub secondary_timeframes: &'static [&'static str],

    /// 권장 심볼 (빈 배열 = 단일 종목, 사용자 지정)
    pub default_tickers: &'static [&'static str],

    /// 전략 카테고리
    pub category: StrategyCategory,

    /// 지원 시장 (복수 가능)
    pub supported_markets: &'static [MarketType],

    /// 팩토리 함수 (Box<dyn Strategy> 생성)
    pub factory: fn() -> Box<dyn crate::Strategy>,

    /// UI 스키마 팩토리 함수 (SDUI 지원)
    ///
    /// Config 타입에 `#[derive(StrategyConfig)]` 매크로가 적용된 경우,
    /// `Config::ui_schema()`를 호출하여 SDUI 스키마를 반환합니다.
    /// None인 경우 기본 스키마가 사용됩니다.
    pub ui_schema_factory: Option<fn() -> StrategyUISchema>,
}

impl std::fmt::Debug for StrategyMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrategyMeta")
            .field("id", &self.id)
            .field("aliases", &self.aliases)
            .field("name", &self.name)
            .field("description", &self.description)
            .field("default_timeframe", &self.default_timeframe)
            .field("secondary_timeframes", &self.secondary_timeframes)
            .field("default_tickers", &self.default_tickers)
            .field("category", &self.category)
            .field("supported_markets", &self.supported_markets)
            .field("factory", &"<fn>")
            .field("ui_schema_factory", &self.ui_schema_factory.map(|_| "<fn>"))
            .finish()
    }
}

impl StrategyMeta {
    /// 전략 ID 또는 별칭으로 매칭
    pub fn matches(&self, query: &str) -> bool {
        self.id == query || self.aliases.contains(&query)
    }
}

// 전역 레지스트리에 등록 (inventory 사용)
inventory::collect!(StrategyMeta);

/// 전략 레지스트리 조회 API
pub struct StrategyRegistry;

impl StrategyRegistry {
    /// 모든 등록된 전략 메타데이터
    pub fn all() -> impl Iterator<Item = &'static StrategyMeta> + Clone {
        inventory::iter::<StrategyMeta>.into_iter()
    }

    /// ID/별칭으로 전략 검색
    pub fn find(query: &str) -> Option<&'static StrategyMeta> {
        Self::all().find(|meta| meta.matches(query))
    }

    /// 카테고리별 필터링
    pub fn by_category(category: StrategyCategory) -> impl Iterator<Item = &'static StrategyMeta> {
        Self::all().filter(move |meta| meta.category == category)
    }

    /// 전략 인스턴스 생성
    pub fn create_instance(query: &str) -> Result<Box<dyn crate::Strategy>, String> {
        Self::find(query)
            .map(|meta| (meta.factory)())
            .ok_or_else(|| format!("Unknown strategy: {}", query))
    }

    /// 전략 목록 (프론트엔드용 JSON)
    pub fn to_json() -> serde_json::Value {
        use serde_json::json;
        let strategies: Vec<_> = Self::all()
            .map(|meta| {
                json!({
                    "id": meta.id,
                    "aliases": meta.aliases,
                    "name": meta.name,
                    "description": meta.description,
                    "defaultTimeframe": meta.default_timeframe,
                    "secondaryTimeframes": meta.secondary_timeframes,
                    "isMultiTimeframe": !meta.secondary_timeframes.is_empty(),
                    "defaultStrings": meta.default_tickers,
                    "category": meta.category,
                    "supportedMarkets": meta.supported_markets.iter()
                        .map(|m| format!("{:?}", m)).collect::<Vec<_>>(),
                })
            })
            .collect();
        json!({ "strategies": strategies })
    }

    /// 전략 ID 목록 (디버깅용)
    pub fn list_ids() -> Vec<&'static str> {
        Self::all().map(|meta| meta.id).collect()
    }

    /// 등록된 전략 수
    pub fn count() -> usize {
        Self::all().count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_api() {
        // 레지스트리가 초기화되어 있어야 함
        // (아직 전략 등록 전이므로 0개일 수 있음)
        let count = StrategyRegistry::count();
        println!("Registered strategies: {}", count);

        // JSON 생성 테스트
        let json = StrategyRegistry::to_json();
        assert!(json.is_object());
        assert!(json.get("strategies").is_some());
    }

    #[test]
    fn test_category_serialization() {
        use serde_json;

        let category = StrategyCategory::Intraday;
        let json = serde_json::to_string(&category).unwrap();
        assert_eq!(json, "\"Intraday\"");

        let deserialized: StrategyCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, category);
    }
}
