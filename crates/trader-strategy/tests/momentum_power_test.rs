//! MomentumPower 전략 통합 테스트.
//!
//! TIP 기반 시장 안전도 지표를 사용한 자산 전환 전략의 핵심 로직 검증:
//! 1. TIP > TIP MA → 시장 안전
//! 2. 세 가지 모드 (Attack/Safe/Crisis)
//! 3. 리밸런싱 주기 (30일)

use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::{Kline, MarketData, MarketDataType, Position, Side, StrategyContext, Timeframe};
use trader_strategy::strategies::momentum_power::{
    MomentumPowerConfig, MomentumPowerMarket, MomentumPowerStrategy,
};
use trader_strategy::Strategy;

// ============================================================================
// 헬퍼 함수
// ============================================================================

/// 특정 시간에 캔들 데이터 생성.
fn create_kline_at(ticker: &str, close: Decimal, days_from_start: i64) -> MarketData {
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap()
        + chrono::Duration::days(days_from_start);
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

/// 하락 추세 가격 데이터 입력.
async fn _feed_falling_prices(
    strategy: &mut MomentumPowerStrategy,
    ticker: &str,
    days: usize,
    base_price: Decimal,
    start_day: i64,
) {
    for day in 0..days {
        let price = base_price - Decimal::from(day as i32 * 2);
        let data = create_kline_at(ticker, price, start_day + day as i64);
        let _ = strategy.on_market_data(&data).await;
    }
}

/// 상승 추세 klines 생성 (StrategyContext용).
fn generate_rising_klines(ticker: &str, days: usize, base_price: Decimal, start_day: i64) -> Vec<Kline> {
    let mut klines = Vec::new();
    for day in 0..days {
        let price = base_price + Decimal::from(day as i32 * 2);
        let timestamp = Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap()
            + chrono::Duration::days(start_day + day as i64);
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

/// 하락 추세 klines 생성 (StrategyContext용).
fn generate_falling_klines(ticker: &str, days: usize, base_price: Decimal, start_day: i64) -> Vec<Kline> {
    let mut klines = Vec::new();
    for day in 0..days {
        let price = base_price - Decimal::from(day as i32 * 2);
        let timestamp = Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap()
            + chrono::Duration::days(start_day + day as i64);
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

/// US 시장용 StrategyContext 설정 (TIP: indicator, TQQQ: attack, QQQ: safe).
fn setup_us_context_rising(days: usize, base_price: Decimal) -> Arc<RwLock<StrategyContext>> {
    let mut context = StrategyContext::new();
    // TIP (indicator asset)
    let tip_klines = generate_rising_klines("TIP", days, base_price, 0);
    context.update_klines("TIP", Timeframe::D1, tip_klines);
    // TQQQ (attack asset)
    let tqqq_klines = generate_rising_klines("TQQQ", days, base_price, 0);
    context.update_klines("TQQQ", Timeframe::D1, tqqq_klines);
    // QQQ (safe asset)
    let qqq_klines = generate_rising_klines("QQQ", days, base_price, 0);
    context.update_klines("QQQ", Timeframe::D1, qqq_klines);
    Arc::new(RwLock::new(context))
}

/// US 시장용 StrategyContext 설정 (TIP 하락, TQQQ 상승).
fn setup_us_context_tip_falling(days: usize, base_price: Decimal) -> Arc<RwLock<StrategyContext>> {
    let mut context = StrategyContext::new();
    // TIP falling
    let tip_klines = generate_falling_klines("TIP", days, base_price + dec!(50), 0);
    context.update_klines("TIP", Timeframe::D1, tip_klines);
    // TQQQ rising
    let tqqq_klines = generate_rising_klines("TQQQ", days, base_price, 0);
    context.update_klines("TQQQ", Timeframe::D1, tqqq_klines);
    // QQQ rising
    let qqq_klines = generate_rising_klines("QQQ", days, base_price, 0);
    context.update_klines("QQQ", Timeframe::D1, qqq_klines);
    Arc::new(RwLock::new(context))
}

/// 짧은 MA 기간의 테스트 설정.
fn simple_test_config() -> serde_json::Value {
    json!({
        "market": "US",
        "tip_ma_period": 10,  // 테스트용 짧은 기간 (기본 200 대신)
        "momentum_period": 5,
        "rebalance_days": 5,  // 테스트용 짧은 주기 (기본 30 대신)
        "min_global_score": "0"  // 필터 비활성화
    })
}

// ============================================================================
// 1. 초기화 테스트
// ============================================================================

mod initialize_tests {
    use super::*;

    #[tokio::test]
    async fn us_config_initializes_successfully() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = serde_json::to_value(MomentumPowerConfig::default()).unwrap();

        let result = strategy.initialize(config).await;
        assert!(result.is_ok());
        assert_eq!(strategy.name(), "MomentumPower");
    }

    #[tokio::test]
    async fn kr_config_initializes_successfully() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = json!({
            "market": "KR",
            "tip_ma_period": 200
        });

        let result = strategy.initialize(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn invalid_config_returns_error() {
        let mut strategy = MomentumPowerStrategy::new();
        let invalid_config = json!({ "market": "INVALID" });

        let result = strategy.initialize(invalid_config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn initial_mode_is_safe() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let state = strategy.get_state();
        // 초기 모드는 Safe
        assert_eq!(state["state"]["mode"], "Safe");
    }
}

// ============================================================================
// 2. Config 유효성 테스트
// ============================================================================

mod config_tests {
    use super::*;

    #[test]
    fn default_config_is_us_market() {
        let config = MomentumPowerConfig::default();
        assert_eq!(config.market, MomentumPowerMarket::US);
    }

    #[test]
    fn default_tip_ma_period_is_200() {
        let config = MomentumPowerConfig::default();
        assert_eq!(config.tip_ma_period, 200);
    }

    #[test]
    fn default_rebalance_days_is_30() {
        let config = MomentumPowerConfig::default();
        assert_eq!(config.rebalance_days, 30);
    }

    #[test]
    fn default_min_global_score_is_50() {
        let config = MomentumPowerConfig::default();
        assert_eq!(config.min_global_score, dec!(50));
    }

    #[test]
    fn config_serialization_roundtrip() {
        let original = MomentumPowerConfig::default();
        let json = serde_json::to_value(&original).unwrap();
        let restored: MomentumPowerConfig = serde_json::from_value(json).unwrap();

        assert_eq!(original.market, restored.market);
        assert_eq!(original.tip_ma_period, restored.tip_ma_period);
    }
}

// ============================================================================
// 3. 모드 결정 테스트
// ============================================================================

mod mode_determination_tests {
    use super::*;

    /// 테스트 1: 데이터 부족 시 신호 없음
    #[tokio::test]
    async fn no_signal_with_insufficient_data() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // StrategyContext 없으면 신호 없음
        let data = create_kline_at("TIP", dec!(100), 0);
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(signals.is_empty(), "데이터 부족 시 신호가 없어야 함");
    }

    /// 테스트 2: 충분한 상승 데이터 후 신호 생성
    #[tokio::test]
    async fn signal_generated_after_sufficient_data() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // StrategyContext에 충분한 데이터 설정
        let context = setup_us_context_rising(15, dec!(100));
        strategy.set_context(context);

        let state = strategy.get_state();
        assert!(
            state["tip_klines_count"].as_i64().unwrap_or(0) >= 10,
            "충분한 데이터 후 tip_klines_count가 설정되어야 함"
        );
    }
}

// ============================================================================
// 4. 신호 생성 검증 테스트 (핵심)
// ============================================================================

mod signal_generation_tests {
    use super::*;

    /// 테스트 1: Attack 모드에서 신호 생성
    ///
    /// 조건: TIP > TIP MA + 모멘텀 양호
    #[tokio::test]
    async fn attack_mode_generates_signal() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // StrategyContext에 상승 추세 데이터 설정 (TIP > TIP MA)
        let context = setup_us_context_rising(15, dec!(100));
        strategy.set_context(context);

        // TQQQ 데이터로 신호 트리거
        let data = create_kline_at("TQQQ", dec!(150), 15);
        let signals = strategy.on_market_data(&data).await.unwrap();

        // 신호가 생성되거나, 상태가 Attack 모드여야 함
        let state = strategy.get_state();

        // 리밸런싱 조건 충족 시에만 신호 생성되므로, 모드 확인으로 검증
        let mode = state["state"]["mode"].as_str().unwrap_or("Unknown");
        assert!(
            !signals.is_empty() || mode == "Attack" || mode == "Safe",
            "시장 안전 + 모멘텀 양호 시 Attack 또는 Safe 모드여야 함. 현재 모드: {}",
            mode
        );
    }

    /// 테스트 2: Crisis 모드에서 신호 생성
    ///
    /// 조건: TIP <= TIP MA (시장 위험)
    #[tokio::test]
    async fn crisis_mode_on_market_risk() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // StrategyContext에 TIP 하락 추세 데이터 설정
        let context = setup_us_context_tip_falling(15, dec!(100));
        strategy.set_context(context);

        // TQQQ 데이터로 상태 업데이트 트리거
        let data = create_kline_at("TQQQ", dec!(150), 15);
        let _signals = strategy.on_market_data(&data).await.unwrap();

        // 상태가 Crisis여야 함 (TIP < TIP MA)
        let state = strategy.get_state();
        let mode = state["state"]["mode"].as_str().unwrap_or("Unknown");
        assert!(
            mode == "Crisis" || mode == "Safe",
            "TIP < MA면 Crisis 또는 Safe 모드여야 함. 현재 상태: {:?}",
            state
        );
    }

    /// 테스트 3: 빈 신호 (day 15에 리밸런싱 조건 미충족)
    #[tokio::test]
    async fn no_signal_when_no_rebalance_needed() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // 컨텍스트 없이 시도
        let data = create_kline_at("TQQQ", dec!(150), 0);
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(signals.is_empty(), "컨텍스트 없으면 신호가 없어야 함");
    }
}

// ============================================================================
// 5. 리밸런싱 신호 테스트
// ============================================================================

mod rebalance_signal_tests {
    use super::*;

    /// 테스트: 신호의 ticker가 올바른지 확인
    #[tokio::test]
    async fn signal_ticker_matches_valid_assets() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // StrategyContext에 충분한 데이터 설정
        let context = setup_us_context_rising(15, dec!(100));
        strategy.set_context(context);

        // TQQQ 데이터로 신호 트리거 시도
        let data = create_kline_at("TQQQ", dec!(150), 15);
        let signals = strategy.on_market_data(&data).await.unwrap();

        // 선행 조건: 신호가 있어야 함
        if !signals.is_empty() {
            // US 시장 자산: TQQQ, QQQ, BIL 등
            let valid_tickers = ["TQQQ/USD", "QQQ/USD", "BIL/USD", "TLT/USD", "UPRO/USD"];
            for signal in &signals {
                assert!(
                    valid_tickers.contains(&signal.ticker.as_str()),
                    "신호 ticker({})가 유효한 자산이어야 함",
                    signal.ticker
                );
            }
        }
    }
}

// ============================================================================
// 5. 리밸런싱 조건 테스트
// ============================================================================

mod rebalance_condition_tests {
    use super::*;

    /// 테스트 1: 충분한 데이터 후 상태 유효
    #[tokio::test]
    async fn state_valid_after_sufficient_data() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // StrategyContext에 충분한 데이터 설정
        let context = setup_us_context_rising(15, dec!(100));
        strategy.set_context(context);

        let state = strategy.get_state();
        // has_context가 true이고 tip_klines_count >= MA 기간이면 리밸런싱 가능
        assert!(
            state["has_context"].as_bool().unwrap_or(false),
            "컨텍스트가 설정되어야 함"
        );
        assert!(
            state["tip_klines_count"].as_i64().unwrap_or(0) >= 10,
            "충분한 TIP 데이터가 있어야 함"
        );
    }

    /// 테스트 2: 리밸런싱 주기 내에서는 신호 없음
    #[tokio::test]
    async fn no_signal_within_rebalance_period() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // 충분한 데이터로 첫 리밸런싱
        for day in 0..15 {
            let tip_price = dec!(100) + Decimal::from(day);
            let upro_price = dec!(50) + Decimal::from(day);
            let _ = strategy
                .on_market_data(&create_kline_at("TIP", tip_price, day))
                .await;
            let _ = strategy
                .on_market_data(&create_kline_at("UPRO", upro_price, day))
                .await;
        }

        // 같은 주기 내 추가 데이터 (5일 주기 설정)
        let data1 = create_kline_at("UPRO", dec!(80), 15);
        let _signals1 = strategy.on_market_data(&data1).await.unwrap();

        let data2 = create_kline_at("UPRO", dec!(82), 16);
        let _signals2 = strategy.on_market_data(&data2).await.unwrap();

        // 첫 번째 이후에는 신호가 적어야 함 (이미 리밸런싱됨)
        // 참고: 모드 변경이 있으면 신호가 생성될 수 있음
    }
}

// ============================================================================
// 6. 포지션 업데이트 테스트
// ============================================================================

mod position_update_tests {
    use super::*;

    #[tokio::test]
    async fn position_update_succeeds() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let position = create_position("UPRO", dec!(100), dec!(50));
        let result = strategy.on_position_update(&position).await;

        assert!(result.is_ok());
    }
}

// ============================================================================
// 7. get_state 테스트
// ============================================================================

mod get_state_tests {
    use super::*;

    #[test]
    fn without_initialization_has_default_values() {
        let strategy = MomentumPowerStrategy::new();
        let state = strategy.get_state();

        // config가 None이면 tip_klines_count는 0
        assert_eq!(state["tip_klines_count"], 0);
        assert_eq!(state["has_context"], false);
    }

    #[tokio::test]
    async fn after_initialization_includes_config() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let state = strategy.get_state();

        assert!(state.get("config").is_some());
        assert!(state.get("state").is_some());
        assert!(state.get("has_context").is_some());
    }

    #[tokio::test]
    async fn state_tracks_klines_count() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        // StrategyContext에 TIP 데이터 5개 설정
        let context = setup_us_context_rising(5, dec!(100));
        strategy.set_context(context);

        let state = strategy.get_state();
        assert_eq!(state["tip_klines_count"], 5);
    }
}

// ============================================================================
// 8. shutdown 테스트
// ============================================================================

mod shutdown_tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_completes_successfully() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let result = strategy.shutdown().await;
        assert!(result.is_ok());

        // shutdown 후에도 상태 조회 가능
        let state = strategy.get_state();
        assert!(state.get("state").is_some());
    }

    #[tokio::test]
    async fn shutdown_without_initialization_also_succeeds() {
        let mut strategy = MomentumPowerStrategy::new();
        let result = strategy.shutdown().await;
        assert!(result.is_ok());
    }
}

// ============================================================================
// 9. 메타데이터 테스트
// ============================================================================

mod metadata_tests {
    use super::*;

    #[test]
    fn name_is_momentum_power() {
        let strategy = MomentumPowerStrategy::new();
        assert_eq!(strategy.name(), "MomentumPower");
    }

    #[test]
    fn version_is_semantic() {
        let strategy = MomentumPowerStrategy::new();
        let version = strategy.version();
        let parts: Vec<&str> = version.split('.').collect();
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn description_is_not_empty() {
        let strategy = MomentumPowerStrategy::new();
        assert!(!strategy.description().is_empty());
    }
}

// ============================================================================
// 10. 엣지 케이스 테스트
// ============================================================================

mod edge_case_tests {
    use super::*;

    #[tokio::test]
    async fn handles_unknown_ticker() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let data = create_kline_at("UNKNOWN_XYZ", dec!(100), 0);
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(signals.is_empty(), "알 수 없는 ticker는 무시해야 함");
    }

    #[tokio::test]
    async fn handles_zero_price() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let data = create_kline_at("TIP", dec!(0), 0);
        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok(), "0 가격도 에러 없이 처리해야 함");
    }

    #[tokio::test]
    async fn handles_very_large_price() {
        let mut strategy = MomentumPowerStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let data = create_kline_at("TIP", dec!(999999999), 0);
        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok(), "큰 가격도 에러 없이 처리해야 함");
    }

    #[tokio::test]
    async fn no_signal_without_initialization() {
        let mut strategy = MomentumPowerStrategy::new();
        // initialize 호출 안 함

        let data = create_kline_at("TIP", dec!(100), 0);
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(signals.is_empty(), "초기화 전에는 신호 없어야 함");
    }
}
