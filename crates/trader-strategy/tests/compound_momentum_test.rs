//! CompoundMomentum (Simple Power) 전략 통합 테스트.
//!
//! TQQQ/SCHD/PFIX/TMF 기반 모멘텀 자산배분 전략의 핵심 로직 검증:
//! 1. MA130 기반 모멘텀 필터 (전일종가 vs MA, MA 추세)
//! 2. 비중 조정 로직 (cut_count에 따른 rate_multiplier)
//! 3. PFIX/TMF 대체 로직
//! 4. 월간 리밸런싱

use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::{Kline, MarketData, MarketDataType, Position, Side, StrategyContext, Timeframe};
use trader_strategy::strategies::compound_momentum::{
    CompoundMomentumConfig, CompoundMomentumStrategy, MarketType,
};
use trader_strategy::Strategy;

// ============================================================================
// 헬퍼 함수
// ============================================================================

/// 특정 연월에 캔들 데이터 생성.
fn create_kline_at_month(
    ticker: &str,
    close: Decimal,
    year: i32,
    month: u32,
    day: u32,
) -> MarketData {
    let timestamp = Utc.with_ymd_and_hms(year, month, day, 12, 0, 0).unwrap();
    MarketData {
        exchange: "test".to_string(),
        ticker: ticker.to_string(),
        timestamp,
        data: MarketDataType::Kline(Kline {
            ticker: ticker.to_string(),
            timeframe: Timeframe::D1,
            open_time: timestamp,
            close_time: timestamp,
            open: close - dec!(1),
            high: close + dec!(1),
            low: close - dec!(2),
            close,
            volume: dec!(10000),
            quote_volume: Some(close * dec!(10000)),
            num_trades: Some(100),
        }),
    }
}

/// 테스트용 Position 생성.
fn create_position(ticker: &str, quantity: Decimal, entry_price: Decimal) -> Position {
    Position::new("test", ticker.to_string(), Side::Buy, quantity, entry_price)
}

/// 특정 연월에 상승 추세 klines 생성 (StrategyContext용).
fn generate_rising_klines_at_month(
    ticker: &str,
    days: usize,
    base_price: Decimal,
    year: i32,
    month: u32,
) -> Vec<Kline> {
    let mut klines = Vec::new();
    for day in 1..=days {
        let day_num = (day as u32).min(28);
        let price = base_price + Decimal::from(day as i32 * 2);
        let timestamp = Utc.with_ymd_and_hms(year, month, day_num, 12, 0, 0).unwrap();
        let kline = Kline::new(
            ticker.to_string(),
            Timeframe::D1,
            timestamp,
            price - dec!(1),
            price + dec!(1),
            price - dec!(2),
            price,
            dec!(10000),
            timestamp,
        );
        klines.push(kline);
    }
    klines
}

/// StrategyContext에 상승 추세 데이터 설정.
fn setup_context_with_rising_prices(
    tickers: &[&str],
    days: usize,
    base_price: Decimal,
    year: i32,
    month: u32,
) -> Arc<RwLock<StrategyContext>> {
    let mut context = StrategyContext::new();
    for ticker in tickers {
        let klines = generate_rising_klines_at_month(ticker, days, base_price, year, month);
        context.update_klines(ticker, Timeframe::D1, klines);
    }
    Arc::new(RwLock::new(context))
}

/// 짧은 MA 기간의 테스트 설정 (기본 130일 대신 10일 사용).
fn simple_test_config() -> serde_json::Value {
    json!({
        "market": "US",
        "aggressive_asset": "TQQQ",
        "aggressive_weight": "0.5",
        "dividend_asset": "SCHD",
        "dividend_weight": "0.2",
        "rate_hedge_asset": "PFIX",
        "rate_hedge_weight": "0.15",
        "bond_leverage_asset": "TMF",
        "bond_leverage_weight": "0.15",
        "ma_period": 10,  // 테스트용 짧은 기간
        "rebalance_interval_months": 1,
        "invest_rate": "1.0",
        "rebalance_threshold": "0.03",
        "min_global_score": "0",  // GlobalScore 필터 비활성화
        "initial_capital": "100000"
    })
}

// ============================================================================
// 1. 초기화 테스트
// ============================================================================

mod initialize_tests {
    use super::*;

    #[tokio::test]
    async fn us_config_initializes_successfully() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = serde_json::to_value(CompoundMomentumConfig::us_default()).unwrap();

        let result = strategy.initialize(config).await;
        assert!(result.is_ok());
        assert_eq!(strategy.name(), "Simple Power");
    }

    #[tokio::test]
    async fn kr_config_initializes_successfully() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = serde_json::to_value(CompoundMomentumConfig::kr_default()).unwrap();

        let result = strategy.initialize(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn invalid_config_returns_error() {
        let mut strategy = CompoundMomentumStrategy::new();
        // 완전히 잘못된 타입 (문자열)은 파싱 실패해야 함
        let invalid_config = json!("not an object");

        let result = strategy.initialize(invalid_config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn initial_capital_reflected_in_state() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();

        strategy.initialize(config).await.unwrap();

        let state = strategy.get_state();
        assert_eq!(state["cash_balance"], "100000");
    }
}

// ============================================================================
// 2. Config 유효성 테스트
// ============================================================================

mod config_tests {
    use super::*;

    #[test]
    fn us_default_has_correct_assets() {
        let config = CompoundMomentumConfig::us_default();
        assert_eq!(config.market, MarketType::US);
        assert_eq!(config.aggressive_asset, "TQQQ");
        assert_eq!(config.dividend_asset, "SCHD");
        assert_eq!(config.rate_hedge_asset, "PFIX");
        assert_eq!(config.bond_leverage_asset, "TMF");
    }

    #[test]
    fn kr_default_has_correct_assets() {
        let config = CompoundMomentumConfig::kr_default();
        assert_eq!(config.market, MarketType::KR);
        assert_eq!(config.aggressive_asset, "409820");
        assert_eq!(config.dividend_asset, "441640");
    }

    #[test]
    fn all_assets_returns_four_tickers() {
        let config = CompoundMomentumConfig::us_default();
        let assets = config.all_assets();

        assert_eq!(assets.len(), 4);
        assert!(assets.contains(&"TQQQ".to_string()));
        assert!(assets.contains(&"SCHD".to_string()));
        assert!(assets.contains(&"PFIX".to_string()));
        assert!(assets.contains(&"TMF".to_string()));
    }

    #[test]
    fn base_weights_sum_to_one() {
        let config = CompoundMomentumConfig::us_default();
        let weights = config.base_weights();
        let sum: Decimal = weights.values().sum();

        assert_eq!(sum, dec!(1.0), "비중 합계가 100%여야 함");
    }

    #[test]
    fn config_serialization_roundtrip() {
        let original = CompoundMomentumConfig::us_default();
        let json = serde_json::to_value(&original).unwrap();
        let restored: CompoundMomentumConfig = serde_json::from_value(json).unwrap();

        assert_eq!(original.aggressive_asset, restored.aggressive_asset);
        assert_eq!(original.ma_period, restored.ma_period);
    }
}

// ============================================================================
// 3. 리밸런싱 조건 테스트
// ============================================================================

mod rebalance_condition_tests {
    use super::*;

    /// 테스트 1: 첫 번째 리밸런싱은 항상 실행
    #[tokio::test]
    async fn first_rebalance_always_triggers() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let tickers = vec!["TQQQ", "SCHD", "PFIX", "TMF"];

        // StrategyContext에 충분한 데이터 설정 (MA 10일 + 여유분)
        let context = setup_context_with_rising_prices(&tickers, 15, dec!(100), 2025, 12);
        strategy.set_context(context);

        // 현재 시점 데이터로 리밸런싱 트리거
        let data = create_kline_at_month("TQQQ", dec!(150), 2025, 12, 28);
        let _signals = strategy.on_market_data(&data).await.unwrap();

        let state = strategy.get_state();
        assert!(
            state.get("last_rebalance_ym").is_some() && !state["last_rebalance_ym"].is_null(),
            "첫 번째 리밸런싱 후 last_rebalance_ym이 설정되어야 함"
        );
    }

    /// 테스트 2: 같은 달에는 리밸런싱 안 함
    #[tokio::test]
    async fn same_month_no_rebalance() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let tickers = vec!["TQQQ", "SCHD", "PFIX", "TMF"];

        // StrategyContext에 충분한 데이터 설정
        let context = setup_context_with_rising_prices(&tickers, 15, dec!(100), 2025, 12);
        strategy.set_context(context);

        // 첫 번째 리밸런싱 (12월 20일)
        let data1 = create_kline_at_month("TQQQ", dec!(150), 2025, 12, 20);
        let _signals1 = strategy.on_market_data(&data1).await.unwrap();

        // 같은 달에 두 번째 데이터 (12월 25일)
        let data2 = create_kline_at_month("TQQQ", dec!(155), 2025, 12, 25);
        let signals2 = strategy.on_market_data(&data2).await.unwrap();

        // 검증: 두 번째는 신호가 없어야 함 (같은 달)
        assert!(
            signals2.is_empty(),
            "같은 달에는 리밸런싱 신호가 생성되지 않아야 함. 생성된 신호: {}개",
            signals2.len()
        );
    }

    /// 테스트 3: 다른 달에는 리밸런싱 실행
    #[tokio::test]
    async fn different_month_triggers_rebalance() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let tickers = vec!["TQQQ", "SCHD", "PFIX", "TMF"];

        // StrategyContext에 12월 데이터 설정
        let context = setup_context_with_rising_prices(&tickers, 15, dec!(100), 2025, 12);
        strategy.set_context(context);

        // 12월 리밸런싱
        let data = create_kline_at_month("TQQQ", dec!(150), 2025, 12, 28);
        let _ = strategy.on_market_data(&data).await;

        let state_dec = strategy.get_state();
        let dec_ym = state_dec["last_rebalance_ym"].as_str().unwrap_or("");

        // 1월 데이터로 컨텍스트 업데이트
        let context_jan = setup_context_with_rising_prices(&tickers, 15, dec!(155), 2026, 1);
        strategy.set_context(context_jan);

        // 1월 리밸런싱
        let data_jan = create_kline_at_month("TQQQ", dec!(180), 2026, 1, 15);
        let _ = strategy.on_market_data(&data_jan).await;

        let state_jan = strategy.get_state();
        let jan_ym = state_jan["last_rebalance_ym"].as_str().unwrap_or("");

        // 검증: 12월과 1월 리밸런싱 시점이 달라야 함
        assert_ne!(
            dec_ym, jan_ym,
            "다른 달에는 last_rebalance_ym이 업데이트되어야 함. 12월: {}, 1월: {}",
            dec_ym, jan_ym
        );
    }
}

// ============================================================================
// 4. 신호 생성 검증 테스트 (핵심)
// ============================================================================

mod signal_generation_tests {
    use super::*;

    /// 테스트 1: 신호가 실제로 생성되는지 확인
    ///
    /// 핵심 로직:
    /// 1. StrategyContext에 충분한 klines 데이터 설정
    /// 2. 리밸런싱 트리거 → 신호 생성
    #[tokio::test]
    async fn signals_are_actually_generated() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let tickers = vec!["TQQQ", "SCHD", "PFIX", "TMF"];

        // StrategyContext에 충분한 데이터 설정 (MA 10일 + 여유분)
        let context = setup_context_with_rising_prices(&tickers, 15, dec!(100), 2025, 12);
        strategy.set_context(context);

        // 리밸런싱 트리거
        let data = create_kline_at_month("TQQQ", dec!(180), 2025, 12, 28);
        let signals = strategy.on_market_data(&data).await.unwrap();

        // 핵심 검증: 신호가 실제로 생성되어야 함
        assert!(
            !signals.is_empty(),
            "리밸런싱 시 신호가 생성되어야 함. \
            원인 분석: \
            1) 모든 ticker에 MA 기간+3일 이상 데이터 필요 \
            2) initial_capital이 설정되어야 cash_balance > 0"
        );

        // 신호 구조 검증
        for signal in &signals {
            assert!(
                signal.side == Side::Buy || signal.side == Side::Sell,
                "신호는 Buy 또는 Sell이어야 함"
            );
        }
    }

    /// 테스트 2: 생성된 신호의 ticker가 설정에 있는지 확인
    #[tokio::test]
    async fn signal_tickers_are_from_config() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let tickers = vec!["TQQQ", "SCHD", "PFIX", "TMF"];

        // StrategyContext에 데이터 설정
        let context = setup_context_with_rising_prices(&tickers, 15, dec!(100), 2025, 12);
        strategy.set_context(context);

        // 리밸런싱 트리거
        let data = create_kline_at_month("TQQQ", dec!(180), 2025, 12, 28);
        let signals = strategy.on_market_data(&data).await.unwrap();

        // 선행 조건: 신호가 먼저 생성되어야 함
        assert!(
            !signals.is_empty(),
            "테스트 전제 조건 실패: 신호가 생성되어야 함"
        );

        // 신호의 ticker가 설정에 있는지 확인 (ticker/USD 형식)
        for signal in &signals {
            let base_ticker = signal.ticker.split('/').next().unwrap_or("");
            assert!(
                tickers.contains(&base_ticker) || base_ticker == "USD",
                "신호 ticker({})가 설정에 없음. 설정 tickers: {:?}",
                signal.ticker,
                tickers
            );
        }
    }

    /// 테스트 3: 초기 매수 신호가 생성되는지 확인
    #[tokio::test]
    async fn initial_buy_signals_generated() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let tickers = vec!["TQQQ", "SCHD", "PFIX", "TMF"];

        // StrategyContext에 데이터 설정 (상승 추세)
        let context = setup_context_with_rising_prices(&tickers, 15, dec!(100), 2025, 12);
        strategy.set_context(context);

        // 리밸런싱 트리거
        let data = create_kline_at_month("TQQQ", dec!(180), 2025, 12, 28);
        let signals = strategy.on_market_data(&data).await.unwrap();

        // 포지션이 없는 상태에서 목표 비중이 있으면 매수 신호 생성
        let buy_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Buy).collect();
        assert!(
            !buy_signals.is_empty(),
            "초기 리밸런싱에서 매수 신호가 생성되어야 함"
        );
    }
}

// ============================================================================
// 5. 포지션 업데이트 테스트
// ============================================================================

mod position_update_tests {
    use super::*;

    #[tokio::test]
    async fn position_update_reflected_in_state() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let position = create_position("TQQQ", dec!(100), dec!(50));
        strategy.on_position_update(&position).await.unwrap();

        let state = strategy.get_state();
        let positions = state["positions"].as_object().unwrap();

        assert!(positions.contains_key("TQQQ"));
        assert_eq!(positions["TQQQ"], "100");
    }

    #[tokio::test]
    async fn multiple_positions_tracked_correctly() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let pos1 = create_position("TQQQ", dec!(50), dec!(100));
        let pos2 = create_position("SCHD", dec!(30), dec!(80));

        strategy.on_position_update(&pos1).await.unwrap();
        strategy.on_position_update(&pos2).await.unwrap();

        let state = strategy.get_state();
        let positions = state["positions"].as_object().unwrap();

        assert_eq!(positions["TQQQ"], "50");
        assert_eq!(positions["SCHD"], "30");
    }
}

// ============================================================================
// 6. get_state 테스트
// ============================================================================

mod get_state_tests {
    use super::*;

    #[test]
    fn without_initialization_has_default_values() {
        let strategy = CompoundMomentumStrategy::new();
        let state = strategy.get_state();

        assert_eq!(state["name"], "Simple Power");
        assert_eq!(state["version"], "2.0.0");
    }

    #[tokio::test]
    async fn after_initialization_includes_config_info() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let state = strategy.get_state();

        assert!(state.get("name").is_some());
        assert!(state.get("version").is_some());
        assert!(state.get("positions").is_some());
        assert!(state.get("cash_balance").is_some());
        assert!(state.get("momentum_states").is_some());
    }
}

// ============================================================================
// 7. shutdown 테스트
// ============================================================================

mod shutdown_tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_completes_successfully() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let result = strategy.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn shutdown_without_initialization_also_succeeds() {
        let mut strategy = CompoundMomentumStrategy::new();
        let result = strategy.shutdown().await;
        assert!(result.is_ok());
    }
}

// ============================================================================
// 8. 메타데이터 테스트
// ============================================================================

mod metadata_tests {
    use super::*;

    #[test]
    fn name_is_compound_momentum() {
        let strategy = CompoundMomentumStrategy::new();
        assert_eq!(strategy.name(), "CompoundMomentum");
    }

    #[test]
    fn version_is_semantic() {
        let strategy = CompoundMomentumStrategy::new();
        let version = strategy.version();
        let parts: Vec<&str> = version.split('.').collect();
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn description_is_not_empty() {
        let strategy = CompoundMomentumStrategy::new();
        assert!(!strategy.description().is_empty());
    }
}

// ============================================================================
// 9. 엣지 케이스 테스트
// ============================================================================

mod edge_case_tests {
    use super::*;

    #[tokio::test]
    async fn handles_insufficient_data_gracefully() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // 단일 데이터만 입력 (MA 계산 불가)
        let data = create_kline_at_month("TQQQ", dec!(100), 2025, 12, 1);
        let result = strategy.on_market_data(&data).await;

        // 데이터 부족 시 에러 없이 처리
        assert!(result.is_ok(), "데이터 부족 시 에러 없이 처리해야 함");
        // 초기 매수 신호가 발생할 수 있으므로 신호 존재 여부만 확인
    }

    #[tokio::test]
    async fn handles_unknown_ticker() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let data = create_kline_at_month("UNKNOWN_XYZ", dec!(100), 2025, 12, 1);
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(signals.is_empty(), "알 수 없는 ticker는 무시해야 함");
    }

    #[tokio::test]
    async fn handles_zero_price() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let data = create_kline_at_month("TQQQ", dec!(0), 2025, 12, 1);
        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok(), "0 가격도 에러 없이 처리해야 함");
    }

    #[tokio::test]
    async fn handles_very_large_price() {
        let mut strategy = CompoundMomentumStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let data = create_kline_at_month("TQQQ", dec!(999999999), 2025, 12, 1);
        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok(), "큰 가격도 에러 없이 처리해야 함");
    }
}
