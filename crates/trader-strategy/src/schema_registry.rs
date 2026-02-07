//! SDUI Fragment 레지스트리 및 빌트인 Fragment 정의.
//!
//! 이 모듈은 전략 UI 스키마 Fragment를 관리하고,
//! 26개 전략에서 재사용 가능한 빌트인 Fragment를 제공합니다.

use serde_json::json;
use std::collections::HashMap;
use trader_core::{FieldSchema, FieldType, FragmentCategory, SchemaFragment};

/// Fragment 레지스트리.
///
/// 전략에서 사용할 수 있는 모든 Fragment를 관리합니다.
pub struct FragmentRegistry {
    fragments: HashMap<String, SchemaFragment>,
}

impl FragmentRegistry {
    /// 빈 레지스트리를 생성합니다.
    pub fn new() -> Self {
        Self {
            fragments: HashMap::new(),
        }
    }

    /// 빌트인 Fragment가 등록된 레지스트리를 생성합니다.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register_builtins();
        registry
    }

    /// Fragment를 등록합니다.
    pub fn register(&mut self, fragment: SchemaFragment) {
        self.fragments.insert(fragment.id.clone(), fragment);
    }

    /// Fragment를 조회합니다.
    pub fn get(&self, id: &str) -> Option<&SchemaFragment> {
        self.fragments.get(id)
    }

    /// 카테고리별 Fragment 목록을 반환합니다.
    pub fn list_by_category(&self, category: FragmentCategory) -> Vec<&SchemaFragment> {
        self.fragments
            .values()
            .filter(|f| f.category == category)
            .collect()
    }

    /// 모든 Fragment를 반환합니다.
    pub fn list_all(&self) -> Vec<&SchemaFragment> {
        self.fragments.values().collect()
    }

    /// 의존성을 포함한 Fragment 목록을 반환합니다.
    pub fn resolve_with_dependencies(&self, ids: &[&str]) -> Vec<&SchemaFragment> {
        let mut resolved = Vec::new();
        let mut visited = std::collections::HashSet::new();

        for id in ids {
            self.resolve_recursive(id, &mut resolved, &mut visited);
        }

        resolved
    }

    fn resolve_recursive<'a>(
        &'a self,
        id: &str,
        resolved: &mut Vec<&'a SchemaFragment>,
        visited: &mut std::collections::HashSet<String>,
    ) {
        if visited.contains(id) {
            return;
        }

        if let Some(fragment) = self.fragments.get(id) {
            visited.insert(id.to_string());

            // 의존성 먼저 해결
            for dep in &fragment.dependencies {
                self.resolve_recursive(dep, resolved, visited);
            }

            resolved.push(fragment);
        }
    }

    /// 빌트인 Fragment를 등록합니다.
    fn register_builtins(&mut self) {
        // Indicator 카테고리
        self.register(create_rsi_fragment());
        self.register(create_macd_fragment());
        self.register(create_bollinger_fragment());
        self.register(create_ma_fragment());
        self.register(create_atr_fragment());
        self.register(create_entry_signal_fragment());

        // Filter 카테고리
        self.register(create_route_state_filter());
        self.register(create_market_regime_filter());
        self.register(create_volume_filter());

        // RiskManagement 카테고리
        self.register(create_stop_loss_fragment());
        self.register(create_take_profit_fragment());
        self.register(create_trailing_stop_fragment());
        self.register(create_exit_config_fragment());

        // PositionSizing 카테고리
        self.register(create_fixed_ratio_fragment());
        self.register(create_kelly_fragment());
        self.register(create_atr_sizing_fragment());

        // Timing 카테고리
        self.register(create_rebalance_fragment());

        // Asset 카테고리
        self.register(create_single_ticker_fragment());
        self.register(create_universe_fragment());
    }
}

impl Default for FragmentRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

// ============================================================================
// Indicator Fragments
// ============================================================================

fn create_rsi_fragment() -> SchemaFragment {
    SchemaFragment::new("indicator.rsi", "RSI 설정", FragmentCategory::Indicator)
        .with_description("RSI (Relative Strength Index) 지표 설정")
        .with_fields(vec![
            FieldSchema {
                name: "period".to_string(),
                field_type: FieldType::Integer,
                label: "RSI 기간".to_string(),
                description: Some("RSI 계산에 사용할 기간".to_string()),
                default: Some(json!(14)),
                min: Some(2.0),
                max: Some(100.0),
                required: true,
                ..Default::default()
            },
            FieldSchema {
                name: "overbought".to_string(),
                field_type: FieldType::Number,
                label: "과매수 임계값".to_string(),
                description: Some("이 값 이상일 때 과매수로 판단".to_string()),
                default: Some(json!(70.0)),
                min: Some(50.0),
                max: Some(100.0),
                required: true,
                ..Default::default()
            },
            FieldSchema {
                name: "oversold".to_string(),
                field_type: FieldType::Number,
                label: "과매도 임계값".to_string(),
                description: Some("이 값 이하일 때 과매도로 판단".to_string()),
                default: Some(json!(30.0)),
                min: Some(0.0),
                max: Some(50.0),
                required: true,
                ..Default::default()
            },
        ])
}

fn create_macd_fragment() -> SchemaFragment {
    SchemaFragment::new("indicator.macd", "MACD 설정", FragmentCategory::Indicator)
        .with_description("MACD (Moving Average Convergence Divergence) 지표 설정")
        .with_fields(vec![
            FieldSchema {
                name: "fast_period".to_string(),
                field_type: FieldType::Integer,
                label: "단기 EMA 기간".to_string(),
                default: Some(json!(12)),
                min: Some(2.0),
                max: Some(100.0),
                required: true,
                ..Default::default()
            },
            FieldSchema {
                name: "slow_period".to_string(),
                field_type: FieldType::Integer,
                label: "장기 EMA 기간".to_string(),
                default: Some(json!(26)),
                min: Some(2.0),
                max: Some(100.0),
                required: true,
                ..Default::default()
            },
            FieldSchema {
                name: "signal_period".to_string(),
                field_type: FieldType::Integer,
                label: "시그널 EMA 기간".to_string(),
                default: Some(json!(9)),
                min: Some(2.0),
                max: Some(100.0),
                required: true,
                ..Default::default()
            },
        ])
}

fn create_bollinger_fragment() -> SchemaFragment {
    SchemaFragment::new(
        "indicator.bollinger",
        "볼린저 밴드 설정",
        FragmentCategory::Indicator,
    )
    .with_description("Bollinger Bands 지표 설정")
    .with_fields(vec![
        FieldSchema {
            name: "period".to_string(),
            field_type: FieldType::Integer,
            label: "기간".to_string(),
            default: Some(json!(20)),
            min: Some(2.0),
            max: Some(100.0),
            required: true,
            ..Default::default()
        },
        FieldSchema {
            name: "std_dev".to_string(),
            field_type: FieldType::Number,
            label: "표준편차 배수".to_string(),
            default: Some(json!(2.0)),
            min: Some(0.5),
            max: Some(5.0),
            required: true,
            ..Default::default()
        },
    ])
}

fn create_ma_fragment() -> SchemaFragment {
    SchemaFragment::new("indicator.ma", "이동평균 설정", FragmentCategory::Indicator)
        .with_description("SMA/EMA 이동평균 지표 설정")
        .with_fields(vec![
            FieldSchema {
                name: "ma_type".to_string(),
                field_type: FieldType::Select,
                label: "이동평균 타입".to_string(),
                default: Some(json!("sma")),
                options: vec!["sma".to_string(), "ema".to_string()],
                required: true,
                ..Default::default()
            },
            FieldSchema {
                name: "period".to_string(),
                field_type: FieldType::Integer,
                label: "기간".to_string(),
                default: Some(json!(20)),
                min: Some(2.0),
                max: Some(200.0),
                required: true,
                ..Default::default()
            },
        ])
}

fn create_atr_fragment() -> SchemaFragment {
    SchemaFragment::new("indicator.atr", "ATR 설정", FragmentCategory::Indicator)
        .with_description("ATR (Average True Range) 지표 설정")
        .with_fields(vec![FieldSchema {
            name: "period".to_string(),
            field_type: FieldType::Integer,
            label: "ATR 기간".to_string(),
            default: Some(json!(14)),
            min: Some(2.0),
            max: Some(100.0),
            required: true,
            ..Default::default()
        }])
}

fn create_entry_signal_fragment() -> SchemaFragment {
    SchemaFragment::new(
        "indicator.entry_signal",
        "진입 신호 설정",
        FragmentCategory::Indicator,
    )
    .with_description("그리드/RSI/볼린저 진입 신호 설정")
    .with_fields(vec![
        FieldSchema {
            name: "spacing_pct".to_string(),
            field_type: FieldType::Number,
            label: "그리드 간격".to_string(),
            description: Some("가격 대비 그리드 간격 비율".to_string()),
            default: Some(json!(1.0)),
            min: Some(0.1),
            max: Some(10.0),
            required: true,
            ..Default::default()
        },
        FieldSchema {
            name: "levels".to_string(),
            field_type: FieldType::Integer,
            label: "그리드 레벨 수".to_string(),
            description: Some("상하 그리드 레벨 개수".to_string()),
            default: Some(json!(5)),
            min: Some(1.0),
            max: Some(20.0),
            required: true,
            ..Default::default()
        },
        FieldSchema {
            name: "rsi_period".to_string(),
            field_type: FieldType::Integer,
            label: "RSI 기간".to_string(),
            description: Some("RSI 계산 기간 (RSI 변형용)".to_string()),
            default: Some(json!(14)),
            min: Some(2.0),
            max: Some(100.0),
            required: false,
            ..Default::default()
        },
        FieldSchema {
            name: "oversold".to_string(),
            field_type: FieldType::Number,
            label: "과매도 임계값".to_string(),
            description: Some("RSI 과매도 기준 (RSI 변형용)".to_string()),
            default: Some(json!(30.0)),
            min: Some(0.0),
            max: Some(50.0),
            required: false,
            ..Default::default()
        },
        FieldSchema {
            name: "overbought".to_string(),
            field_type: FieldType::Number,
            label: "과매수 임계값".to_string(),
            description: Some("RSI 과매수 기준 (RSI 변형용)".to_string()),
            default: Some(json!(70.0)),
            min: Some(50.0),
            max: Some(100.0),
            required: false,
            ..Default::default()
        },
    ])
}

// ============================================================================
// Filter Fragments
// ============================================================================

fn create_route_state_filter() -> SchemaFragment {
    SchemaFragment::new(
        "filter.route_state",
        "RouteState 필터",
        FragmentCategory::Filter,
    )
    .with_description("RouteState 기반 종목 필터링")
    .with_fields(vec![
        FieldSchema {
            name: "enabled".to_string(),
            field_type: FieldType::Boolean,
            label: "RouteState 필터 활성화".to_string(),
            default: Some(json!(false)),
            required: true,
            ..Default::default()
        },
        FieldSchema {
            name: "allowed_states".to_string(),
            field_type: FieldType::MultiSelect,
            label: "허용 상태".to_string(),
            default: Some(json!(["Attack", "Armed"])),
            options: vec![
                "Attack".to_string(),
                "Armed".to_string(),
                "Wait".to_string(),
                "Overheat".to_string(),
                "Neutral".to_string(),
            ],
            condition: Some("enabled == true".to_string()),
            ..Default::default()
        },
    ])
}

fn create_market_regime_filter() -> SchemaFragment {
    SchemaFragment::new(
        "filter.market_regime",
        "시장 국면 필터",
        FragmentCategory::Filter,
    )
    .with_description("시장 국면에 따른 전략 활성화/비활성화")
    .with_fields(vec![
        FieldSchema {
            name: "enabled".to_string(),
            field_type: FieldType::Boolean,
            label: "시장 국면 필터 활성화".to_string(),
            default: Some(json!(false)),
            required: true,
            ..Default::default()
        },
        FieldSchema {
            name: "allowed_regimes".to_string(),
            field_type: FieldType::MultiSelect,
            label: "허용 국면".to_string(),
            default: Some(json!(["bull", "neutral"])),
            options: vec![
                "bull".to_string(),
                "neutral".to_string(),
                "bear".to_string(),
            ],
            condition: Some("enabled == true".to_string()),
            ..Default::default()
        },
    ])
}

fn create_volume_filter() -> SchemaFragment {
    SchemaFragment::new("filter.volume", "거래량 필터", FragmentCategory::Filter)
        .with_description("거래량 기반 신호 필터링")
        .with_fields(vec![
            FieldSchema {
                name: "enabled".to_string(),
                field_type: FieldType::Boolean,
                label: "거래량 필터 활성화".to_string(),
                default: Some(json!(false)),
                required: true,
                ..Default::default()
            },
            FieldSchema {
                name: "min_volume_ratio".to_string(),
                field_type: FieldType::Number,
                label: "최소 거래량 비율".to_string(),
                description: Some("평균 거래량 대비 최소 비율".to_string()),
                default: Some(json!(1.5)),
                min: Some(0.1),
                max: Some(10.0),
                condition: Some("enabled == true".to_string()),
                ..Default::default()
            },
        ])
}

// ============================================================================
// RiskManagement Fragments
// ============================================================================

fn create_stop_loss_fragment() -> SchemaFragment {
    SchemaFragment::new(
        "risk.stop_loss",
        "손절 설정",
        FragmentCategory::RiskManagement,
    )
    .with_description("손절가 설정")
    .with_fields(vec![
        FieldSchema {
            name: "enabled".to_string(),
            field_type: FieldType::Boolean,
            label: "손절 활성화".to_string(),
            default: Some(json!(true)),
            required: true,
            ..Default::default()
        },
        FieldSchema {
            name: "stop_loss_pct".to_string(),
            field_type: FieldType::Number,
            label: "손절 비율 (%)".to_string(),
            description: Some("진입가 대비 손절 비율".to_string()),
            default: Some(json!(3.0)),
            min: Some(0.1),
            max: Some(20.0),
            condition: Some("enabled == true".to_string()),
            required: true,
            ..Default::default()
        },
    ])
}

fn create_take_profit_fragment() -> SchemaFragment {
    SchemaFragment::new(
        "risk.take_profit",
        "익절 설정",
        FragmentCategory::RiskManagement,
    )
    .with_description("익절가 설정")
    .with_fields(vec![
        FieldSchema {
            name: "enabled".to_string(),
            field_type: FieldType::Boolean,
            label: "익절 활성화".to_string(),
            default: Some(json!(true)),
            required: true,
            ..Default::default()
        },
        FieldSchema {
            name: "take_profit_pct".to_string(),
            field_type: FieldType::Number,
            label: "익절 비율 (%)".to_string(),
            description: Some("진입가 대비 익절 비율".to_string()),
            default: Some(json!(5.0)),
            min: Some(0.1),
            max: Some(50.0),
            condition: Some("enabled == true".to_string()),
            required: true,
            ..Default::default()
        },
    ])
}

fn create_trailing_stop_fragment() -> SchemaFragment {
    SchemaFragment::new(
        "risk.trailing_stop",
        "트레일링 스탑",
        FragmentCategory::RiskManagement,
    )
    .with_description("트레일링 스탑 설정")
    .with_fields(vec![
        FieldSchema {
            name: "enabled".to_string(),
            field_type: FieldType::Boolean,
            label: "트레일링 스탑 활성화".to_string(),
            default: Some(json!(false)),
            required: true,
            ..Default::default()
        },
        FieldSchema {
            name: "trigger_pct".to_string(),
            field_type: FieldType::Number,
            label: "활성화 수익률 (%)".to_string(),
            description: Some("이 수익률 달성 시 트레일링 스탑 시작".to_string()),
            default: Some(json!(2.0)),
            min: Some(0.1),
            max: Some(20.0),
            condition: Some("enabled == true".to_string()),
            ..Default::default()
        },
        FieldSchema {
            name: "trail_pct".to_string(),
            field_type: FieldType::Number,
            label: "추적 비율 (%)".to_string(),
            description: Some("고점 대비 하락 허용 비율".to_string()),
            default: Some(json!(1.0)),
            min: Some(0.1),
            max: Some(10.0),
            condition: Some("enabled == true".to_string()),
            ..Default::default()
        },
    ])
}

fn create_exit_config_fragment() -> SchemaFragment {
    SchemaFragment::new(
        "risk.exit_config",
        "리스크 관리",
        FragmentCategory::RiskManagement,
    )
    .with_description("손절/익절/트레일링 스탑 설정")
    .with_fields(vec![
        // === 손절 설정 ===
        FieldSchema {
            name: "stop_loss_enabled".to_string(),
            field_type: FieldType::Boolean,
            label: "손절 활성화".to_string(),
            description: Some("손절가 설정 활성화".to_string()),
            default: Some(json!(true)),
            required: true,
            ..Default::default()
        },
        FieldSchema {
            name: "stop_loss_pct".to_string(),
            field_type: FieldType::Number,
            label: "손절 비율".to_string(),
            description: Some("진입가 대비 손절 비율 (%)".to_string()),
            default: Some(json!(2.0)),
            min: Some(0.1),
            max: Some(20.0),
            condition: Some("stop_loss_enabled == true".to_string()),
            required: true,
            ..Default::default()
        },
        // === 익절 설정 ===
        FieldSchema {
            name: "take_profit_enabled".to_string(),
            field_type: FieldType::Boolean,
            label: "익절 활성화".to_string(),
            description: Some("익절가 설정 활성화".to_string()),
            default: Some(json!(true)),
            required: true,
            ..Default::default()
        },
        FieldSchema {
            name: "take_profit_pct".to_string(),
            field_type: FieldType::Number,
            label: "익절 비율".to_string(),
            description: Some("진입가 대비 익절 비율 (%)".to_string()),
            default: Some(json!(4.0)),
            min: Some(0.1),
            max: Some(50.0),
            condition: Some("take_profit_enabled == true".to_string()),
            required: true,
            ..Default::default()
        },
        // === 트레일링 스탑 설정 ===
        FieldSchema {
            name: "trailing_stop_enabled".to_string(),
            field_type: FieldType::Boolean,
            label: "트레일링 스탑 활성화".to_string(),
            description: Some("고점 추적 손절 활성화".to_string()),
            default: Some(json!(false)),
            required: true,
            ..Default::default()
        },
        FieldSchema {
            name: "trailing_trigger_pct".to_string(),
            field_type: FieldType::Number,
            label: "트레일링 시작 수익률".to_string(),
            description: Some("이 수익률 달성 시 트레일링 스탑 시작 (%)".to_string()),
            default: Some(json!(2.0)),
            min: Some(0.1),
            max: Some(20.0),
            condition: Some("trailing_stop_enabled == true".to_string()),
            ..Default::default()
        },
        FieldSchema {
            name: "trailing_stop_pct".to_string(),
            field_type: FieldType::Number,
            label: "트레일링 비율".to_string(),
            description: Some("고점 대비 하락 허용 비율 (%)".to_string()),
            default: Some(json!(1.0)),
            min: Some(0.1),
            max: Some(10.0),
            condition: Some("trailing_stop_enabled == true".to_string()),
            ..Default::default()
        },
        // === 기타 청산 조건 ===
        FieldSchema {
            name: "exit_on_neutral".to_string(),
            field_type: FieldType::Boolean,
            label: "중립점 청산".to_string(),
            description: Some("RSI 50, 중간밴드 도달 시 청산".to_string()),
            default: Some(json!(false)),
            required: false,
            ..Default::default()
        },
        FieldSchema {
            name: "cooldown_candles".to_string(),
            field_type: FieldType::Integer,
            label: "쿨다운 캔들 수".to_string(),
            description: Some("청산 후 재진입까지 대기 캔들 수".to_string()),
            default: Some(json!(5)),
            min: Some(0.0),
            max: Some(100.0),
            required: false,
            ..Default::default()
        },
    ])
}

// ============================================================================
// PositionSizing Fragments
// ============================================================================

fn create_fixed_ratio_fragment() -> SchemaFragment {
    SchemaFragment::new(
        "sizing.fixed_ratio",
        "고정 비율",
        FragmentCategory::PositionSizing,
    )
    .with_description("자본의 고정 비율로 포지션 크기 결정")
    .with_fields(vec![FieldSchema {
        name: "position_ratio".to_string(),
        field_type: FieldType::Number,
        label: "포지션 비율 (%)".to_string(),
        description: Some("총 자본 대비 단일 포지션 비율".to_string()),
        default: Some(json!(10.0)),
        min: Some(1.0),
        max: Some(100.0),
        required: true,
        ..Default::default()
    }])
}

fn create_kelly_fragment() -> SchemaFragment {
    SchemaFragment::new(
        "sizing.kelly",
        "켈리 기준",
        FragmentCategory::PositionSizing,
    )
    .with_description("Kelly Criterion 기반 포지션 크기 결정")
    .with_fields(vec![
        FieldSchema {
            name: "win_rate".to_string(),
            field_type: FieldType::Number,
            label: "승률".to_string(),
            description: Some("과거 승률 (0~1)".to_string()),
            default: Some(json!(0.6)),
            min: Some(0.0),
            max: Some(1.0),
            required: true,
            ..Default::default()
        },
        FieldSchema {
            name: "win_loss_ratio".to_string(),
            field_type: FieldType::Number,
            label: "손익비".to_string(),
            description: Some("평균 수익 / 평균 손실".to_string()),
            default: Some(json!(2.0)),
            min: Some(0.1),
            max: Some(10.0),
            required: true,
            ..Default::default()
        },
        FieldSchema {
            name: "kelly_fraction".to_string(),
            field_type: FieldType::Number,
            label: "켈리 비율".to_string(),
            description: Some("Full Kelly의 비율 (보수적 조정)".to_string()),
            default: Some(json!(0.25)),
            min: Some(0.1),
            max: Some(1.0),
            required: true,
            ..Default::default()
        },
    ])
}

fn create_atr_sizing_fragment() -> SchemaFragment {
    SchemaFragment::new("sizing.atr", "ATR 기반", FragmentCategory::PositionSizing)
        .with_description("ATR 기반 포지션 크기 결정")
        .with_dependency("indicator.atr")
        .with_fields(vec![FieldSchema {
            name: "risk_per_trade".to_string(),
            field_type: FieldType::Number,
            label: "거래당 리스크 (%)".to_string(),
            description: Some("총 자본 대비 거래당 리스크".to_string()),
            default: Some(json!(1.0)),
            min: Some(0.1),
            max: Some(5.0),
            required: true,
            ..Default::default()
        }])
}

// ============================================================================
// Timing Fragments
// ============================================================================

fn create_rebalance_fragment() -> SchemaFragment {
    SchemaFragment::new(
        "timing.rebalance",
        "리밸런싱 주기",
        FragmentCategory::Timing,
    )
    .with_description("포트폴리오 리밸런싱 주기 설정")
    .with_fields(vec![
        FieldSchema {
            name: "rebalance_interval".to_string(),
            field_type: FieldType::Select,
            label: "리밸런싱 주기".to_string(),
            default: Some(json!("monthly")),
            options: vec![
                "daily".to_string(),
                "weekly".to_string(),
                "monthly".to_string(),
                "quarterly".to_string(),
            ],
            required: true,
            ..Default::default()
        },
        FieldSchema {
            name: "threshold_pct".to_string(),
            field_type: FieldType::Number,
            label: "리밸런싱 임계값 (%)".to_string(),
            description: Some(
                "목표 비중과 현재 비중의 차이가 이 값 이상일 때만 리밸런싱".to_string(),
            ),
            default: Some(json!(5.0)),
            min: Some(0.0),
            max: Some(50.0),
            ..Default::default()
        },
    ])
}

// ============================================================================
// Asset Fragments
// ============================================================================

fn create_single_ticker_fragment() -> SchemaFragment {
    SchemaFragment::new("asset.single", "단일 심볼", FragmentCategory::Asset)
        .with_description("단일 종목 거래")
        .with_fields(vec![FieldSchema {
            name: "ticker".to_string(),
            field_type: FieldType::String,
            label: "종목 심볼".to_string(),
            required: true,
            ..Default::default()
        }])
}

fn create_universe_fragment() -> SchemaFragment {
    SchemaFragment::new("asset.universe", "종목 유니버스", FragmentCategory::Asset)
        .with_description("다중 종목 포트폴리오")
        .with_fields(vec![
            FieldSchema {
                name: "tickers".to_string(),
                field_type: FieldType::String,
                label: "종목 목록".to_string(),
                required: true,
                ..Default::default()
            },
            FieldSchema {
                name: "max_positions".to_string(),
                field_type: FieldType::Integer,
                label: "최대 보유 종목 수".to_string(),
                default: Some(json!(10)),
                min: Some(1.0),
                max: Some(100.0),
                ..Default::default()
            },
        ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = FragmentRegistry::with_builtins();
        assert!(registry.get("indicator.rsi").is_some());
        assert!(registry.get("risk.stop_loss").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_list_by_category() {
        let registry = FragmentRegistry::with_builtins();
        let indicators = registry.list_by_category(FragmentCategory::Indicator);
        assert!(indicators.len() >= 5);

        let risk_fragments = registry.list_by_category(FragmentCategory::RiskManagement);
        assert!(risk_fragments.len() >= 3);
    }

    #[test]
    fn test_dependency_resolution() {
        let registry = FragmentRegistry::with_builtins();
        let resolved = registry.resolve_with_dependencies(&["sizing.atr"]);

        // ATR sizing은 indicator.atr에 의존
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].id, "indicator.atr"); // 의존성이 먼저
        assert_eq!(resolved[1].id, "sizing.atr");
    }

    #[test]
    fn test_fragment_fields() {
        let registry = FragmentRegistry::with_builtins();
        let rsi = registry.get("indicator.rsi").unwrap();

        assert_eq!(rsi.fields.len(), 3);
        assert_eq!(rsi.fields[0].name, "period");
        assert_eq!(rsi.fields[1].name, "overbought");
        assert_eq!(rsi.fields[2].name, "oversold");
    }
}
