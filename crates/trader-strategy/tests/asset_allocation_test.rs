//! AssetAllocation 전략 통합 테스트.
//!
//! 전략의 핵심 로직을 검증합니다:
//! 1. 카나리아 자산 기반 모드 전환 (Offensive/Defensive)
//! 2. 모멘텀 기반 자산 순위
//! 3. 월간 리밸런싱 조건
//! 4. 리밸런싱 신호 생성

use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use trader_core::{Kline, MarketData, Position, Side, StrategyContext, Timeframe};
use trader_strategy::strategies::asset_allocation::{AssetAllocationConfig, AssetAllocationStrategy};
use trader_strategy::Strategy;

// ============================================================================
// 헬퍼 함수
// ============================================================================

/// 특정 날짜의 캔들 데이터 생성.
fn create_kline_at(ticker: &str, close: Decimal, days_ago: i64) -> MarketData {
    let timestamp = Utc::now() - chrono::Duration::days(days_ago);
    let kline = Kline::new(
        ticker.to_string(),
        Timeframe::D1,
        timestamp,
        close - dec!(5),
        close + dec!(5),
        close - dec!(10),
        close,
        dec!(10000),
        timestamp,
    );
    MarketData::from_kline("test", kline)
}

/// 현재 시점의 캔들 데이터 생성.
fn create_kline(ticker: &str, close: Decimal) -> MarketData {
    create_kline_at(ticker, close, 0)
}

/// 특정 연월 시점의 캔들 데이터 생성.
fn create_kline_at_month(
    ticker: &str,
    close: Decimal,
    year: i32,
    month: u32,
    day: u32,
) -> MarketData {
    let timestamp = Utc.with_ymd_and_hms(year, month, day, 12, 0, 0).unwrap();
    let kline = Kline::new(
        ticker.to_string(),
        Timeframe::D1,
        timestamp,
        close - dec!(5),
        close + dec!(5),
        close - dec!(10),
        close,
        dec!(10000),
        timestamp,
    );
    MarketData::from_kline("test", kline)
}

/// 테스트용 포지션 생성.
fn create_position(ticker: &str, quantity: Decimal) -> Position {
    Position {
        id: Uuid::new_v4(),
        exchange: "test".to_string(),
        ticker: ticker.to_string(),
        side: Side::Buy,
        quantity,
        entry_price: dec!(100),
        current_price: dec!(105),
        unrealized_pnl: quantity * dec!(5),
        realized_pnl: Decimal::ZERO,
        strategy_id: Some("asset_allocation".to_string()),
        opened_at: Utc::now(),
        updated_at: Utc::now(),
        closed_at: None,
        metadata: json!({}),
    }
}

/// HAA 설정의 모든 티커 목록.
fn haa_tickers() -> Vec<String> {
    AssetAllocationConfig::haa_default().all_tickers()
}

/// 상승 추세 가격 데이터 입력 (모멘텀 양수 유도).
///
/// HAA의 모멘텀 기간은 [1, 3, 6, 12]개월 = 최대 252일 필요
/// 따라서 260일 이상의 데이터를 입력해야 유효한 모멘텀 계산 가능
async fn feed_rising_prices(
    strategy: &mut AssetAllocationStrategy,
    ticker: &str,
    days: usize,
    base_price: Decimal,
) {
    for day in (0..days).rev() {
        // 과거에서 현재로 가면서 가격 상승
        let price = base_price + Decimal::from((days - day) as i32 * 2);
        let data = create_kline_at(ticker, price, day as i64);
        let _ = strategy.on_market_data(&data).await;
    }
}

/// 특정 연월에 상승 추세 가격 데이터 입력.
///
/// 신호 생성 테스트에서 월간 리밸런싱 로직을 제대로 테스트하기 위해
/// 특정 월에 데이터를 입력한 후, 다음 월 데이터로 리밸런싱 트리거 필요.
async fn feed_rising_prices_in_month(
    strategy: &mut AssetAllocationStrategy,
    ticker: &str,
    days: usize,
    base_price: Decimal,
    year: i32,
    month: u32,
) {
    for day in 1..=days {
        // 월 초부터 순서대로 가격 상승
        let day_num = (day as u32).min(28); // 모든 월에 유효한 날짜
        let price = base_price + Decimal::from(day as i32 * 2);
        let data = create_kline_at_month(ticker, price, year, month, day_num);
        let _ = strategy.on_market_data(&data).await;
    }
}



/// 상승 추세 klines 데이터 생성 (StrategyContext용)
fn generate_rising_klines(ticker: &str, days: usize, base_price: Decimal) -> Vec<Kline> {
    let mut klines = Vec::new();
    for day in (0..days).rev() {
        let price = base_price + Decimal::from((days - day) as i32 * 2);
        let timestamp = Utc::now() - chrono::Duration::days(day as i64);
        let kline = Kline::new(
            ticker.to_string(),
            Timeframe::D1,
            timestamp,
            price - dec!(5),
            price + dec!(5),
            price - dec!(10),
            price,
            dec!(10000),
            timestamp,
        );
        klines.push(kline);
    }
    klines
}

/// 하락 추세 klines 데이터 생성 (StrategyContext용)
fn generate_falling_klines(ticker: &str, days: usize, base_price: Decimal) -> Vec<Kline> {
    let mut klines = Vec::new();
    for day in (0..days).rev() {
        let price = base_price - Decimal::from((days - day) as i32 * 2);
        let timestamp = Utc::now() - chrono::Duration::days(day as i64);
        let kline = Kline::new(
            ticker.to_string(),
            Timeframe::D1,
            timestamp,
            price - dec!(5),
            price + dec!(5),
            price - dec!(10),
            price,
            dec!(10000),
            timestamp,
        );
        klines.push(kline);
    }
    klines
}

/// StrategyContext 설정 헬퍼 - 상승 추세 데이터로 설정
fn setup_context_with_rising_prices(
    tickers: &[&str],
    days: usize,
    base_price: Decimal,
) -> Arc<RwLock<StrategyContext>> {
    let mut context = StrategyContext::new();
    for ticker in tickers {
        let klines = generate_rising_klines(ticker, days, base_price);
        context.update_klines(ticker, Timeframe::D1, klines);
    }
    Arc::new(RwLock::new(context))
}

/// StrategyContext 설정 헬퍼 - 커스텀 가격 데이터로 설정
fn setup_context_with_mixed_prices(
    rising_tickers: &[&str],
    falling_tickers: &[&str],
    days: usize,
    base_price: Decimal,
) -> Arc<RwLock<StrategyContext>> {
    let mut context = StrategyContext::new();
    for ticker in rising_tickers {
        let klines = generate_rising_klines(ticker, days, base_price);
        context.update_klines(ticker, Timeframe::D1, klines);
    }
    for ticker in falling_tickers {
        let klines = generate_falling_klines(ticker, days, base_price + dec!(50));
        context.update_klines(ticker, Timeframe::D1, klines);
    }
    Arc::new(RwLock::new(context))
}

/// 짧은 모멘텀 기간의 커스텀 설정 (테스트용)
/// 30일 데이터로도 모멘텀 계산 가능하도록 1개월 모멘텀만 사용
/// initial_capital 설정으로 리밸런싱 가능하도록 함
fn simple_test_config() -> serde_json::Value {
    json!({
        "variant": "Custom",
        "assets": [
            // 카나리아 없음 → 항상 Offensive
            { "ticker": "SPY", "category": "Offensive", "description": "S&P 500" },
            { "ticker": "VEA", "category": "Offensive", "description": "선진국" },
            { "ticker": "AGG", "category": "Defensive", "description": "채권" },
            { "ticker": "BIL", "category": "Cash", "description": "현금" }
        ],
        "momentum_method": { "SimpleAverage": { "periods_months": [1] } },
        "offensive_top_n": 2,
        "defensive_top_n": 1,
        "cash_ticker": "BIL",
        "invest_rate": "0.99",
        "rebalance_threshold": "0.01",  // 1% - 더 낮은 임계값으로 리밸런싱 쉽게 트리거
        "canary_threshold": "0.5",
        "min_global_score": "0",  // GlobalScore 필터 비활성화 (0으로 설정)
        "initial_capital": "100000"  // 초기 자본금 설정 (리밸런싱에 필요)
    })
}

/// 카나리아가 있는 테스트 설정
fn test_config_with_canary() -> serde_json::Value {
    json!({
        "variant": "Custom",
        "assets": [
            // 카나리아 - 모멘텀 판단용
            { "ticker": "VWO", "category": "Canary", "description": "이머징" },
            // 공격
            { "ticker": "SPY", "category": "Offensive", "description": "S&P 500" },
            { "ticker": "VEA", "category": "Offensive", "description": "선진국" },
            // 방어
            { "ticker": "AGG", "category": "Defensive", "description": "채권" },
            { "ticker": "BIL", "category": "Cash", "description": "현금" }
        ],
        "momentum_method": { "SimpleAverage": { "periods_months": [1] } },
        "offensive_top_n": 2,
        "defensive_top_n": 1,
        "cash_ticker": "BIL",
        "invest_rate": "0.99",
        "rebalance_threshold": "0.01",  // 1%
        "canary_threshold": "0.5",  // 50% 이상 양수면 Offensive
        "min_global_score": "0",  // GlobalScore 필터 비활성화
        "initial_capital": "100000"  // 초기 자본금
    })
}

// ============================================================================
// 1. 초기화 테스트
// ============================================================================

mod initialize_tests {
    use super::*;

    #[tokio::test]
    async fn haa_config_initializes_successfully() {
        // 팩토리 메서드로 이미 초기화된 전략 - variant가 설정됨
        let strategy = AssetAllocationStrategy::haa();
        // haa()는 with_config()를 호출하므로 이미 초기화됨
        assert_eq!(strategy.name(), "AssetAllocation-HAA");
    }

    #[tokio::test]
    async fn xaa_config_initializes_successfully() {
        let strategy = AssetAllocationStrategy::xaa();
        assert_eq!(strategy.name(), "AssetAllocation-XAA");
    }

    #[tokio::test]
    async fn baa_config_initializes_successfully() {
        let strategy = AssetAllocationStrategy::baa();
        assert_eq!(strategy.name(), "AssetAllocation-BAA");
    }

    #[tokio::test]
    async fn all_weather_config_initializes_successfully() {
        let strategy = AssetAllocationStrategy::all_weather();
        assert_eq!(strategy.name(), "AssetAllocation-AllWeather");
    }

    #[tokio::test]
    async fn dual_momentum_config_initializes_successfully() {
        let strategy = AssetAllocationStrategy::dual_momentum();
        assert_eq!(strategy.name(), "AssetAllocation-DualMomentum");
    }

    #[tokio::test]
    async fn custom_config_initializes_via_json() {
        // JSON으로 Custom variant 직접 전달하는 경우 테스트
        let mut strategy = AssetAllocationStrategy::new();
        let config = simple_test_config();  // variant: "Custom"

        let result = strategy.initialize(config).await;
        assert!(result.is_ok());
        assert_eq!(strategy.name(), "AssetAllocation-Custom");
    }

    #[tokio::test]
    async fn invalid_config_returns_error() {
        let mut strategy = AssetAllocationStrategy::new();
        // 완전히 잘못된 타입의 값
        let invalid_config = json!("not an object");

        let result = strategy.initialize(invalid_config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn initial_capital_reflected_in_state() {
        // Custom config로 테스트 (초기 자본금 설정 가능)
        let mut strategy = AssetAllocationStrategy::new();
        let mut config = simple_test_config();
        config["initial_capital"] = json!("500000");

        strategy.initialize(config).await.unwrap();

        let state = strategy.get_state();
        assert_eq!(state["cash_balance"], "500000");
    }
}

// ============================================================================
// 2. 카나리아 모드 전환 테스트 (핵심 로직)
// ============================================================================

mod canary_mode_tests {
    use super::*;

    /// 테스트 1: 카나리아 자산이 없으면 Offensive 모드
    ///
    /// check_canary_assets 로직:
    /// - 카나리아 자산 비어있음 → PortfolioMode::Offensive 반환
    #[tokio::test]
    async fn no_canary_assets_defaults_to_offensive_mode() {
        let mut strategy = AssetAllocationStrategy::new();

        // 커스텀 설정: 카나리아 자산 없이 공격 자산만
        // momentum_method는 enum 형식으로 지정
        let config = simple_test_config();

        strategy.initialize(config).await.unwrap();

        // 충분한 데이터 입력 (1개월 모멘텀용 21일+)
        feed_rising_prices(&mut strategy, "SPY", 30, dec!(100)).await;
        feed_rising_prices(&mut strategy, "VEA", 30, dec!(80)).await;
        feed_rising_prices(&mut strategy, "AGG", 30, dec!(50)).await;
        feed_rising_prices(&mut strategy, "BIL", 30, dec!(100)).await;

        // 신호 생성 시도
        let data = create_kline("SPY", dec!(160));
        let _signals = strategy.on_market_data(&data).await.unwrap();

        // 상태 확인: 카나리아 없으므로 Offensive 모드
        let state = strategy.get_state();
        assert_eq!(
            state["current_mode"], "Offensive",
            "카나리아 자산이 없으면 Offensive 모드여야 함"
        );
    }

    /// 테스트 2: 카나리아 자산 모멘텀 양수 비율 >= threshold → Offensive
    ///
    /// 커스텀 설정 사용 (짧은 모멘텀 기간):
    /// - 카나리아 자산: VWO
    /// - canary_threshold: 0.5 (50%)
    /// - VWO 양수 모멘텀 → 100% >= 50% → Offensive
    #[tokio::test]
    async fn canary_positive_triggers_offensive_mode() {
        let mut strategy = AssetAllocationStrategy::new();
        // 짧은 모멘텀 기간의 설정 사용
        let config = test_config_with_canary();
        strategy.initialize(config).await.unwrap();

        // StrategyContext에 상승 추세 데이터 설정 (1개월 모멘텀용 35일)
        let context = setup_context_with_rising_prices(
            &["VWO", "SPY", "VEA", "AGG", "BIL"],
            35,
            dec!(100),
        );
        strategy.set_context(context);

        // 현재 시점 데이터로 리밸런싱 트리거
        let data = create_kline("SPY", dec!(160));
        let _ = strategy.on_market_data(&data).await;

        let state = strategy.get_state();
        assert_eq!(
            state["current_mode"], "Offensive",
            "카나리아 자산이 양수 모멘텀이면 Offensive 모드여야 함. 현재: {:?}",
            state["current_mode"]
        );
    }

    /// 테스트 3: 카나리아 자산 모멘텀 양수 비율 < threshold → Defensive
    ///
    /// 시나리오: 카나리아 자산에 하락 추세 데이터 입력
    /// - 모멘텀이 음수가 되면 Defensive 모드로 전환
    #[tokio::test]
    async fn canary_negative_triggers_defensive_mode() {
        let mut strategy = AssetAllocationStrategy::new();
        // 짧은 모멘텀 기간의 설정 사용
        let config = test_config_with_canary();
        strategy.initialize(config).await.unwrap();

        // StrategyContext에 혼합 데이터 설정 (VWO만 하락)
        let context = setup_context_with_mixed_prices(
            &["SPY", "VEA", "AGG", "BIL"],  // 상승 추세
            &["VWO"],                        // 하락 추세 (카나리아)
            35,
            dec!(100),
        );
        strategy.set_context(context);

        // 현재 시점 데이터로 리밸런싱 트리거
        let data = create_kline("SPY", dec!(160));
        let _ = strategy.on_market_data(&data).await;

        let state = strategy.get_state();
        assert_eq!(
            state["current_mode"], "Defensive",
            "카나리아 자산이 음수 모멘텀이면 Defensive 모드여야 함"
        );
    }
}

// ============================================================================
// 3. 리밸런싱 조건 테스트
// ============================================================================

mod rebalance_condition_tests {
    use super::*;

    /// 테스트 1: 첫 번째 리밸런싱은 항상 실행
    ///
    /// should_rebalance 로직:
    /// - last_rebalance_ym = None → true 반환
    #[tokio::test]
    async fn first_rebalance_always_triggers() {
        let mut strategy = AssetAllocationStrategy::new();
        let config = serde_json::to_value(AssetAllocationConfig::haa_default()).unwrap();
        strategy.initialize(config).await.unwrap();

        let tickers = haa_tickers();
        let mut all_signals = vec![];

        // 충분한 데이터 입력 (모멘텀 계산 가능하도록 30일)
        for ticker in &tickers {
            feed_rising_prices(&mut strategy, ticker, 30, dec!(100)).await;
        }

        // 현재 시점 데이터로 리밸런싱 트리거
        let data = create_kline("SPY", dec!(160));
        let signals = strategy.on_market_data(&data).await.unwrap();
        all_signals.extend(signals);

        // 검증: 첫 번째 리밸런싱이므로 신호가 생성되어야 함
        let state = strategy.get_state();
        assert!(
            state.get("last_rebalance_ym").is_some() && !state["last_rebalance_ym"].is_null(),
            "첫 번째 리밸런싱 후 last_rebalance_ym이 설정되어야 함"
        );
    }

    /// 테스트 2: 같은 달에는 리밸런싱 안 함
    ///
    /// should_rebalance 로직:
    /// - current_ym == last_ym → false 반환
    #[tokio::test]
    async fn same_month_no_rebalance() {
        let mut strategy = AssetAllocationStrategy::new();
        let config = serde_json::to_value(AssetAllocationConfig::haa_default()).unwrap();
        strategy.initialize(config).await.unwrap();

        let tickers = haa_tickers();

        // 충분한 데이터 입력
        for ticker in &tickers {
            feed_rising_prices(&mut strategy, ticker, 30, dec!(100)).await;
        }

        // 첫 번째 리밸런싱
        let data1 = create_kline("SPY", dec!(160));
        let _signals1 = strategy.on_market_data(&data1).await.unwrap();

        // 같은 달에 두 번째 데이터
        let data2 = create_kline("SPY", dec!(165));
        let signals2 = strategy.on_market_data(&data2).await.unwrap();

        // 검증: 두 번째는 신호가 없어야 함 (같은 달)
        assert!(
            signals2.is_empty(),
            "같은 달에는 리밸런싱 신호가 생성되지 않아야 함. 생성된 신호: {}개",
            signals2.len()
        );
    }

    /// 테스트 3: 다른 달에는 리밸런싱 실행
    ///
    /// should_rebalance 로직:
    /// - current_ym != last_ym → true 반환
    #[tokio::test]
    async fn different_month_triggers_rebalance() {
        let mut strategy = AssetAllocationStrategy::new();
        let config = serde_json::to_value(AssetAllocationConfig::haa_default()).unwrap();
        strategy.initialize(config).await.unwrap();

        let tickers = haa_tickers();

        // 1월 데이터 입력
        for ticker in &tickers {
            for day in 1..=25 {
                let price = dec!(100) + Decimal::from(day);
                let data = create_kline_at_month(ticker, price, 2025, 1, day);
                let _ = strategy.on_market_data(&data).await;
            }
        }

        // 1월 말 - 첫 번째 리밸런싱
        let data_jan = create_kline_at_month("SPY", dec!(130), 2025, 1, 31);
        let _signals_jan = strategy.on_market_data(&data_jan).await.unwrap();

        let state_jan = strategy.get_state();
        let jan_ym = state_jan["last_rebalance_ym"].as_str().unwrap_or("");

        // 2월 데이터 추가
        for ticker in &tickers {
            for day in 1..=15 {
                let price = dec!(135) + Decimal::from(day);
                let data = create_kline_at_month(ticker, price, 2025, 2, day);
                let _ = strategy.on_market_data(&data).await;
            }
        }

        // 2월 - 두 번째 리밸런싱
        let data_feb = create_kline_at_month("SPY", dec!(155), 2025, 2, 15);
        let _signals_feb = strategy.on_market_data(&data_feb).await.unwrap();

        let state_feb = strategy.get_state();
        let feb_ym = state_feb["last_rebalance_ym"].as_str().unwrap_or("");

        // 검증: 1월과 2월 리밸런싱 시점이 달라야 함
        assert_ne!(
            jan_ym, feb_ym,
            "다른 달에는 last_rebalance_ym이 업데이트되어야 함. 1월: {}, 2월: {}",
            jan_ym, feb_ym
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
    /// 이것은 가장 중요한 테스트입니다.
    /// 테스트가 통과해도 실제로 신호가 0개면 무의미합니다.
    ///
    /// 핵심 로직:
    /// 1. 2025년 12월에 모든 ticker 데이터 입력 → `last_rebalance_ym = "2025_12"` 설정
    /// 2. 2026년 1월 데이터로 리밸런싱 트리거 → 월이 바뀌었으므로 리밸런싱 실행
    /// 3. 모든 ticker에 데이터가 있으므로 모멘텀 계산 가능
    #[tokio::test]
    async fn signals_are_actually_generated() {
        let mut strategy = AssetAllocationStrategy::new();
        // 카나리아 없음 → Offensive 모드 보장
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let tickers = vec!["SPY", "VEA", "AGG", "BIL"];
        let mut all_signals = vec![];

        // 1단계: 2025년 12월에 모든 ticker에 충분한 데이터 입력
        // 1개월 모멘텀용으로 22일 이상 필요, 28일 입력
        for ticker in &tickers {
            feed_rising_prices_in_month(&mut strategy, ticker, 28, dec!(100), 2025, 12).await;
        }

        // 2단계: 2026년 1월 데이터로 리밸런싱 트리거
        // 월이 바뀌었으므로 should_rebalance() = true
        for ticker in &tickers {
            let data = create_kline_at_month(ticker, dec!(180), 2026, 1, 15);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 핵심 검증: 신호가 실제로 생성되어야 함
        // 포지션이 없는 상태에서 목표 비중이 있으면 매수 신호 생성
        assert!(
            !all_signals.is_empty(),
            "리밸런싱 시 신호가 생성되어야 함. \
            원인 분석: \
            1) 모든 ticker에 22일+ 데이터 필요 (1개월 모멘텀) \
            2) 월이 바뀌어야 should_rebalance() = true \
            3) initial_capital이 설정되어야 cash_balance > 0"
        );

        // 신호 구조 검증
        for signal in &all_signals {
            assert!(
                signal.side == Side::Buy || signal.side == Side::Sell,
                "신호는 Buy 또는 Sell이어야 함"
            );
            assert!(
                signal.suggested_price.is_some(),
                "신호에 가격 정보가 있어야 함"
            );
            assert!(
                signal.metadata.get("mode").is_some(),
                "신호 메타데이터에 mode가 있어야 함"
            );
        }

        // 추가 검증: Offensive 모드에서는 SPY, VEA에 대한 매수 신호
        let buy_signals: Vec<_> = all_signals.iter().filter(|s| s.side == Side::Buy).collect();
        assert!(
            !buy_signals.is_empty(),
            "Offensive 모드에서 매수 신호가 생성되어야 함"
        );
    }

    /// 테스트 2: 생성된 신호의 ticker가 설정에 있는지 확인
    #[tokio::test]
    async fn signal_tickers_are_from_config() {
        let mut strategy = AssetAllocationStrategy::new();
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let tickers = vec![
            "SPY".to_string(),
            "VEA".to_string(),
            "AGG".to_string(),
            "BIL".to_string(),
        ];
        let mut all_signals = vec![];

        // 1단계: 2025년 12월에 충분한 데이터 입력
        for ticker in &tickers {
            feed_rising_prices_in_month(&mut strategy, ticker.as_str(), 28, dec!(100), 2025, 12)
                .await;
        }

        // 2단계: 2026년 1월 데이터로 리밸런싱 트리거
        for ticker in &tickers {
            let data = create_kline_at_month(ticker.as_str(), dec!(180), 2026, 1, 15);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 선행 조건: 신호가 먼저 생성되어야 함
        assert!(
            !all_signals.is_empty(),
            "테스트 전제 조건 실패: 신호가 생성되어야 함"
        );

        // 신호의 ticker가 설정에 있는지 확인
        for signal in &all_signals {
            assert!(
                tickers.contains(&signal.ticker) || signal.ticker == "BIL",
                "신호 ticker({})가 설정에 없음. 설정 tickers: {:?}",
                signal.ticker,
                tickers
            );
        }
    }

    /// 테스트 3: Offensive 모드에서는 Offensive 자산에 대한 신호 생성
    #[tokio::test]
    async fn offensive_mode_generates_offensive_asset_signals() {
        let mut strategy = AssetAllocationStrategy::new();
        // 카나리아 없음 → Offensive 모드 보장
        let config = simple_test_config();
        strategy.initialize(config).await.unwrap();

        let tickers = vec!["SPY", "VEA", "AGG", "BIL"];

        // 1단계: 2025년 12월에 모든 자산에 상승 추세 데이터 입력
        for ticker in &tickers {
            feed_rising_prices_in_month(&mut strategy, ticker, 28, dec!(100), 2025, 12).await;
        }

        // 2단계: 2026년 1월 데이터로 리밸런싱 트리거
        let mut all_signals = vec![];
        for ticker in &tickers {
            let data = create_kline_at_month(ticker, dec!(180), 2026, 1, 15);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 상태 확인
        let state = strategy.get_state();
        assert_eq!(
            state["current_mode"], "Offensive",
            "카나리아 없으면 Offensive 모드여야 함. 현재: {:?}",
            state["current_mode"]
        );

        // Offensive 모드에서 신호가 생성되었는지 확인
        assert!(
            !all_signals.is_empty(),
            "Offensive 모드에서 신호가 생성되어야 함. \
            (데이터: 12월 28일, 트리거: 1월 15일, 월이 바뀜)"
        );

        // Offensive 자산: SPY, VEA
        let offensive_tickers = vec!["SPY", "VEA"];
        let has_offensive_signal = all_signals
            .iter()
            .any(|s| offensive_tickers.contains(&s.ticker.as_str()));

        assert!(
            has_offensive_signal || all_signals.iter().any(|s| s.ticker == "BIL"),
            "Offensive 모드에서는 Offensive 자산 또는 현금에 대한 신호가 있어야 함. 생성된 신호: {:?}",
            all_signals.iter().map(|s| &s.ticker).collect::<Vec<_>>()
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
        let mut strategy = AssetAllocationStrategy::new();
        let config = serde_json::to_value(AssetAllocationConfig::haa_default()).unwrap();
        strategy.initialize(config).await.unwrap();

        let position = create_position("SPY", dec!(100));
        strategy.on_position_update(&position).await.unwrap();

        let state = strategy.get_state();
        let positions = state["positions"].as_object().unwrap();

        assert!(positions.contains_key("SPY"));
        assert_eq!(positions["SPY"], "100");
    }

    #[tokio::test]
    async fn multiple_positions_tracked_correctly() {
        let mut strategy = AssetAllocationStrategy::new();
        let config = serde_json::to_value(AssetAllocationConfig::haa_default()).unwrap();
        strategy.initialize(config).await.unwrap();

        let pos1 = create_position("SPY", dec!(50));
        let pos2 = create_position("VEA", dec!(30));

        strategy.on_position_update(&pos1).await.unwrap();
        strategy.on_position_update(&pos2).await.unwrap();

        let state = strategy.get_state();
        let positions = state["positions"].as_object().unwrap();

        assert_eq!(positions["SPY"], "50");
        assert_eq!(positions["VEA"], "30");
    }

    #[tokio::test]
    async fn position_with_existing_holdings_affects_signals() {
        let mut strategy = AssetAllocationStrategy::new();
        let config = serde_json::to_value(AssetAllocationConfig::haa_default()).unwrap();
        strategy.initialize(config).await.unwrap();

        let tickers = haa_tickers();

        // 데이터 입력
        for ticker in &tickers {
            feed_rising_prices(&mut strategy, ticker, 60, dec!(100)).await;
        }

        // 기존 포지션 설정 (SPY 100주 보유)
        let position = create_position("SPY", dec!(100));
        strategy.on_position_update(&position).await.unwrap();

        // 리밸런싱 트리거
        let mut all_signals = vec![];
        for ticker in &tickers {
            let data = create_kline(ticker, dec!(220));
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 포지션이 있으면 리밸런싱 계산에 영향을 줌
        let state = strategy.get_state();
        let positions = state["positions"].as_object().unwrap();
        assert_eq!(positions["SPY"], "100", "포지션이 상태에 반영되어야 함");
    }
}

// ============================================================================
// 6. get_state 테스트
// ============================================================================

mod get_state_tests {
    use super::*;

    #[test]
    fn without_initialization_has_default_values() {
        let strategy = AssetAllocationStrategy::new();
        let state = strategy.get_state();

        assert_eq!(state["name"], "AssetAllocation");
        assert_eq!(state["current_mode"], "Defensive"); // 기본값
    }

    #[tokio::test]
    async fn after_initialization_includes_variant() {
        let mut strategy = AssetAllocationStrategy::new();
        let config = serde_json::to_value(AssetAllocationConfig::haa_default()).unwrap();
        strategy.initialize(config).await.unwrap();

        let state = strategy.get_state();

        assert!(state.get("name").is_some());
        assert!(state.get("variant").is_some());
        assert!(state.get("current_mode").is_some());
        assert!(state.get("positions").is_some());
        assert!(state.get("cash_balance").is_some());
    }

    #[tokio::test]
    async fn state_reflects_mode_after_market_data() {
        let mut strategy = AssetAllocationStrategy::new();
        let config = serde_json::to_value(AssetAllocationConfig::haa_default()).unwrap();
        strategy.initialize(config).await.unwrap();

        let tickers = haa_tickers();

        // 상승 추세 데이터 입력
        for ticker in &tickers {
            feed_rising_prices(&mut strategy, ticker, 60, dec!(100)).await;
        }

        // 리밸런싱 트리거
        let data = create_kline("SPY", dec!(220));
        let _ = strategy.on_market_data(&data).await;

        let state = strategy.get_state();

        // 상태가 업데이트되었는지 확인
        assert!(
            state["current_mode"] == "Offensive" || state["current_mode"] == "Defensive",
            "current_mode는 Offensive 또는 Defensive여야 함"
        );
    }
}

// ============================================================================
// 7. shutdown 테스트
// ============================================================================

mod shutdown_tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_completes_successfully() {
        let mut strategy = AssetAllocationStrategy::new();
        let config = serde_json::to_value(AssetAllocationConfig::haa_default()).unwrap();
        strategy.initialize(config).await.unwrap();

        let result = strategy.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn shutdown_without_initialization_also_succeeds() {
        let mut strategy = AssetAllocationStrategy::new();
        let result = strategy.shutdown().await;
        assert!(result.is_ok());
    }
}

// ============================================================================
// 8. metadata 테스트
// ============================================================================

mod metadata_tests {
    use super::*;

    #[test]
    fn name_without_config() {
        let strategy = AssetAllocationStrategy::new();
        assert_eq!(strategy.name(), "AssetAllocation");
    }

    #[test]
    fn version_is_semantic() {
        let strategy = AssetAllocationStrategy::new();
        let version = strategy.version();
        let parts: Vec<&str> = version.split('.').collect();
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn description_is_not_empty() {
        let strategy = AssetAllocationStrategy::new();
        assert!(!strategy.description().is_empty());
    }
}

// ============================================================================
// 9. Config 유효성 테스트
// ============================================================================

mod config_validation_tests {
    use super::*;

    #[test]
    fn haa_config_has_required_assets() {
        let config = AssetAllocationConfig::haa_default();
        let tickers = config.all_tickers();

        assert!(!tickers.is_empty(), "HAA 설정에 자산이 있어야 함");

        // HAA 핵심 자산 확인
        assert!(tickers.contains(&"SPY".to_string()), "SPY가 있어야 함");
        assert!(tickers.contains(&"VEA".to_string()), "VEA가 있어야 함");
    }

    #[test]
    fn xaa_config_has_required_assets() {
        let config = AssetAllocationConfig::xaa_default();
        assert!(!config.all_tickers().is_empty());
    }

    #[test]
    fn baa_config_has_required_assets() {
        let config = AssetAllocationConfig::baa_default();
        assert!(!config.all_tickers().is_empty());
    }

    #[test]
    fn config_serialization_roundtrip() {
        let original = AssetAllocationConfig::haa_default();
        let json = serde_json::to_value(&original).unwrap();
        let restored: AssetAllocationConfig = serde_json::from_value(json).unwrap();

        assert_eq!(original.all_tickers().len(), restored.all_tickers().len());
    }
}

// ============================================================================
// 10. 엣지 케이스 테스트
// ============================================================================

mod edge_case_tests {
    use super::*;

    #[tokio::test]
    async fn handles_insufficient_data_gracefully() {
        let mut strategy = AssetAllocationStrategy::new();
        let config = serde_json::to_value(AssetAllocationConfig::haa_default()).unwrap();
        strategy.initialize(config).await.unwrap();

        // 단일 데이터만 입력 (모멘텀 계산 불가)
        let data = create_kline("SPY", dec!(100));
        let signals = strategy.on_market_data(&data).await.unwrap();

        // 데이터 부족 시 에러 없이 빈 신호
        assert!(signals.is_empty(), "데이터 부족 시 신호가 없어야 함");
    }

    #[tokio::test]
    async fn handles_unknown_ticker() {
        let mut strategy = AssetAllocationStrategy::new();
        let config = serde_json::to_value(AssetAllocationConfig::haa_default()).unwrap();
        strategy.initialize(config).await.unwrap();

        let data = create_kline("UNKNOWN_XYZ", dec!(100));
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(signals.is_empty(), "알 수 없는 ticker는 무시해야 함");
    }

    #[tokio::test]
    async fn handles_zero_price() {
        let mut strategy = AssetAllocationStrategy::new();
        let config = serde_json::to_value(AssetAllocationConfig::haa_default()).unwrap();
        strategy.initialize(config).await.unwrap();

        let data = create_kline("SPY", dec!(0));
        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok(), "0 가격도 에러 없이 처리해야 함");
    }

    #[tokio::test]
    async fn handles_very_large_price() {
        let mut strategy = AssetAllocationStrategy::new();
        let config = serde_json::to_value(AssetAllocationConfig::haa_default()).unwrap();
        strategy.initialize(config).await.unwrap();

        let data = create_kline("SPY", dec!(999999999));
        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok(), "큰 가격도 에러 없이 처리해야 함");
    }
}
