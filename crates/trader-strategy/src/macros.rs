//! 전략 등록 매크로
//!
//! `register_strategy!` 매크로를 사용하여 전략을 레지스트리에 자동 등록합니다.

/// 전략 등록 매크로
///
/// 전략 메타데이터를 선언적으로 정의하고 inventory에 자동 등록합니다.
///
/// # 필수 필드
/// - `id`: 전략 고유 ID (snake_case, 예: "rsi_mean_reversion")
/// - `name`: 한글 이름 (예: "RSI 평균회귀")
/// - `description`: 전략 설명
/// - `timeframe`: 기본 타임프레임 ("1m", "15m", "1h", "1d" 등)
/// - `category`: 전략 카테고리 (Realtime, Intraday, Daily, Monthly)
/// - `type` 또는 `factory`: 전략 타입 또는 커스텀 팩토리 함수
///
/// # 선택 필드
/// - `aliases`: 별칭 배열 (기본값: 빈 배열)
/// - `tickers`: 기본 심볼 배열 (기본값: 빈 배열 = 사용자 지정)
/// - `markets`: 지원 시장 배열 (기본값: [Crypto, Kr, Us])
/// - `secondary_timeframes`: 보조 타임프레임 (다중 TF 전략용, 기본값: 빈 배열)
///
/// # 예시 (기본 type 사용)
/// ```ignore
/// register_strategy! {
///     id: "rsi_mean_reversion",
///     aliases: ["rsi"],
///     name: "RSI 평균회귀",
///     description: "RSI 과매수/과매도 구간에서 평균회귀 매매",
///     timeframe: "15m",
///     tickers: [],
///     category: Intraday,
///     markets: [Crypto, Stock],
///     type: RsiStrategy
/// }
/// ```
///
/// # 예시 (커스텀 factory 사용 - 파생 전략)
/// ```ignore
/// register_strategy! {
///     id: "haa",
///     aliases: ["hierarchical_asset_allocation"],
///     name: "HAA 자산배분",
///     description: "계층적 자산 배분 전략",
///     timeframe: "1M",
///     tickers: ["SPY", "VEA", "VWO", "AGG", "SHY", "IEF", "LQD", "BIL"],
///     category: Monthly,
///     markets: [Stock],
///     factory: AssetAllocationStrategy::haa
/// }
/// ```
///
/// # 동작 방식
/// - 컴파일 타임에 StrategyMeta를 생성하여 inventory에 등록
/// - StrategyRegistry::find()로 조회 가능
/// - 별칭을 통한 다중 접근 지원 (하위 호환성)
#[macro_export]
macro_rules! register_strategy {
    // 패턴 1: 커스텀 factory 사용 (파생 전략용)
    (
        id: $id:expr,
        aliases: [$($alias:expr),* $(,)?],
        name: $name:expr,
        description: $desc:expr,
        timeframe: $tf:expr,
        tickers: [$($ticker:expr),* $(,)?],
        category: $cat:ident,
        markets: [$($market:ident),* $(,)?],
        factory: $factory:expr
    ) => {
        inventory::submit! {
            $crate::registry::StrategyMeta {
                id: $id,
                aliases: &[$($alias),*],
                name: $name,
                description: $desc,
                default_timeframe: $tf,
                secondary_timeframes: &[],
                default_tickers: &[$($ticker),*],
                category: $crate::registry::StrategyCategory::$cat,
                supported_markets: &[$(trader_core::MarketType::$market),*],
                factory: || Box::new($factory()),
                ui_schema_factory: None,
            }
        }
    };

    // 패턴 1-1: 커스텀 factory + config 타입 (SDUI 지원)
    (
        id: $id:expr,
        aliases: [$($alias:expr),* $(,)?],
        name: $name:expr,
        description: $desc:expr,
        timeframe: $tf:expr,
        tickers: [$($ticker:expr),* $(,)?],
        category: $cat:ident,
        markets: [$($market:ident),* $(,)?],
        factory: $factory:expr,
        config: $config_ty:ty
    ) => {
        inventory::submit! {
            $crate::registry::StrategyMeta {
                id: $id,
                aliases: &[$($alias),*],
                name: $name,
                description: $desc,
                default_timeframe: $tf,
                secondary_timeframes: &[],
                default_tickers: &[$($ticker),*],
                category: $crate::registry::StrategyCategory::$cat,
                supported_markets: &[$(trader_core::MarketType::$market),*],
                factory: || Box::new($factory()),
                ui_schema_factory: Some(|| <$config_ty>::ui_schema()),
            }
        }
    };

    // 패턴 2: 커스텀 factory + 다중 타임프레임
    (
        id: $id:expr,
        aliases: [$($alias:expr),* $(,)?],
        name: $name:expr,
        description: $desc:expr,
        timeframe: $tf:expr,
        secondary_timeframes: [$($sec_tf:expr),* $(,)?],
        tickers: [$($ticker:expr),* $(,)?],
        category: $cat:ident,
        markets: [$($market:ident),* $(,)?],
        factory: $factory:expr
    ) => {
        inventory::submit! {
            $crate::registry::StrategyMeta {
                id: $id,
                aliases: &[$($alias),*],
                name: $name,
                description: $desc,
                default_timeframe: $tf,
                secondary_timeframes: &[$($sec_tf),*],
                default_tickers: &[$($ticker),*],
                category: $crate::registry::StrategyCategory::$cat,
                supported_markets: &[$(trader_core::MarketType::$market),*],
                factory: || Box::new($factory()),
                ui_schema_factory: None,
            }
        }
    };

    // 패턴 2-1: 커스텀 factory + 다중 타임프레임 + config (SDUI 지원)
    (
        id: $id:expr,
        aliases: [$($alias:expr),* $(,)?],
        name: $name:expr,
        description: $desc:expr,
        timeframe: $tf:expr,
        secondary_timeframes: [$($sec_tf:expr),* $(,)?],
        tickers: [$($ticker:expr),* $(,)?],
        category: $cat:ident,
        markets: [$($market:ident),* $(,)?],
        factory: $factory:expr,
        config: $config_ty:ty
    ) => {
        inventory::submit! {
            $crate::registry::StrategyMeta {
                id: $id,
                aliases: &[$($alias),*],
                name: $name,
                description: $desc,
                default_timeframe: $tf,
                secondary_timeframes: &[$($sec_tf),*],
                default_tickers: &[$($ticker),*],
                category: $crate::registry::StrategyCategory::$cat,
                supported_markets: &[$(trader_core::MarketType::$market),*],
                factory: || Box::new($factory()),
                ui_schema_factory: Some(|| <$config_ty>::ui_schema()),
            }
        }
    };

    // 패턴 3: 다중 타임프레임 전략 (기본 type::new() 사용)
    (
        id: $id:expr,
        aliases: [$($alias:expr),* $(,)?],
        name: $name:expr,
        description: $desc:expr,
        timeframe: $tf:expr,
        secondary_timeframes: [$($sec_tf:expr),* $(,)?],
        tickers: [$($ticker:expr),* $(,)?],
        category: $cat:ident,
        markets: [$($market:ident),* $(,)?],
        type: $ty:ty
    ) => {
        inventory::submit! {
            $crate::registry::StrategyMeta {
                id: $id,
                aliases: &[$($alias),*],
                name: $name,
                description: $desc,
                default_timeframe: $tf,
                secondary_timeframes: &[$($sec_tf),*],
                default_tickers: &[$($ticker),*],
                category: $crate::registry::StrategyCategory::$cat,
                supported_markets: &[$(trader_core::MarketType::$market),*],
                factory: || Box::new(<$ty>::new()),
                ui_schema_factory: None,
            }
        }
    };

    // 패턴 3-1: 다중 타임프레임 전략 + config (SDUI 지원)
    (
        id: $id:expr,
        aliases: [$($alias:expr),* $(,)?],
        name: $name:expr,
        description: $desc:expr,
        timeframe: $tf:expr,
        secondary_timeframes: [$($sec_tf:expr),* $(,)?],
        tickers: [$($ticker:expr),* $(,)?],
        category: $cat:ident,
        markets: [$($market:ident),* $(,)?],
        type: $ty:ty,
        config: $config_ty:ty
    ) => {
        inventory::submit! {
            $crate::registry::StrategyMeta {
                id: $id,
                aliases: &[$($alias),*],
                name: $name,
                description: $desc,
                default_timeframe: $tf,
                secondary_timeframes: &[$($sec_tf),*],
                default_tickers: &[$($ticker),*],
                category: $crate::registry::StrategyCategory::$cat,
                supported_markets: &[$(trader_core::MarketType::$market),*],
                factory: || Box::new(<$ty>::new()),
                ui_schema_factory: Some(|| <$config_ty>::ui_schema()),
            }
        }
    };

    // 패턴 4: 단일 타임프레임 전략 (기본 type::new() 사용, 기존 호환)
    (
        id: $id:expr,
        aliases: [$($alias:expr),* $(,)?],
        name: $name:expr,
        description: $desc:expr,
        timeframe: $tf:expr,
        tickers: [$($ticker:expr),* $(,)?],
        category: $cat:ident,
        markets: [$($market:ident),* $(,)?],
        type: $ty:ty
    ) => {
        inventory::submit! {
            $crate::registry::StrategyMeta {
                id: $id,
                aliases: &[$($alias),*],
                name: $name,
                description: $desc,
                default_timeframe: $tf,
                secondary_timeframes: &[],
                default_tickers: &[$($ticker),*],
                category: $crate::registry::StrategyCategory::$cat,
                supported_markets: &[$(trader_core::MarketType::$market),*],
                factory: || Box::new(<$ty>::new()),
                ui_schema_factory: None,
            }
        }
    };

    // 패턴 4-1: 단일 타임프레임 전략 + config (SDUI 지원)
    (
        id: $id:expr,
        aliases: [$($alias:expr),* $(,)?],
        name: $name:expr,
        description: $desc:expr,
        timeframe: $tf:expr,
        tickers: [$($ticker:expr),* $(,)?],
        category: $cat:ident,
        markets: [$($market:ident),* $(,)?],
        type: $ty:ty,
        config: $config_ty:ty
    ) => {
        inventory::submit! {
            $crate::registry::StrategyMeta {
                id: $id,
                aliases: &[$($alias),*],
                name: $name,
                description: $desc,
                default_timeframe: $tf,
                secondary_timeframes: &[],
                default_tickers: &[$($ticker),*],
                category: $crate::registry::StrategyCategory::$cat,
                supported_markets: &[$(trader_core::MarketType::$market),*],
                factory: || Box::new(<$ty>::new()),
                ui_schema_factory: Some(|| <$config_ty>::ui_schema()),
            }
        }
    };
}

#[cfg(test)]
mod tests {
    // 매크로 확장 테스트는 실제 전략에서 수행
}
