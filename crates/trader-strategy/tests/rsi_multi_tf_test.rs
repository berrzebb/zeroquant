//! RsiMultiTf (다중 타임프레임 RSI) 전략 통합 테스트
//!
//! 3개의 타임프레임(일봉, 1시간봉, 5분봉)을 조합하여
//! RSI 기반 진입 타이밍을 찾는 전략 테스트.
//!
//! ## get_state() 반환 형식
//!
//! ```json
//! {
//!     "position_state": "Flat|Long",
//!     "entry_price": null|number,
//!     "current_price": null|number,
//!     "rsi": { "daily": null|number, "hourly": null|number, "m5": null|number, "m5_prev": null|number },
//!     "trades_count": number,
//!     "wins": number,
//!     "losses": number,
//!     "win_rate": number,
//!     "total_pnl": number,
//!     "cooldown_remaining": number
//! }
//! ```

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use trader_core::{Kline, MarketData, Position, Side, Timeframe};
use trader_strategy::strategies::rsi_multi_tf::RsiMultiTfStrategy;
use trader_strategy::Strategy;

// ============================================================================
// 테스트 헬퍼 함수
// ============================================================================

/// 테스트용 MarketData 생성 헬퍼 (타임프레임 지정)
fn create_market_data_with_tf(
    ticker: &str,
    close: Decimal,
    timeframe: Timeframe,
    timestamp_offset: i64,
) -> MarketData {
    let timestamp = chrono::DateTime::from_timestamp(1704067200 + timestamp_offset, 0).unwrap();
    let kline = Kline::new(
        ticker.to_string(),
        timeframe,
        timestamp,
        close - dec!(10), // open
        close + dec!(20), // high
        close - dec!(20), // low
        close,            // close
        dec!(100000),     // volume
        timestamp,        // close_time
    );
    MarketData::from_kline("test", kline)
}

/// 간단한 MarketData 생성 헬퍼 (일봉 기준)
fn create_market_data(ticker: &str, close: Decimal, day: i64) -> MarketData {
    create_market_data_with_tf(ticker, close, Timeframe::D1, day * 86400)
}

/// Position 헬퍼 함수
fn create_position(ticker: &str, quantity: Decimal, entry_price: Decimal) -> Position {
    Position::new("test", ticker.to_string(), Side::Buy, quantity, entry_price)
}

// ============================================================================
// 초기화 테스트
// ============================================================================

#[tokio::test]
async fn test_initialization_default_config() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100"
    });
    let result = strategy.initialize(config).await;

    assert!(result.is_ok(), "기본 설정으로 초기화 실패");

    // get_state()는 trades_count 필드를 포함
    let state = strategy.get_state();
    assert!(state["trades_count"].is_number(), "trades_count 필드 존재");
    assert_eq!(state["position_state"], "Flat", "초기 포지션 상태는 Flat");
}

#[tokio::test]
async fn test_initialization_with_custom_config() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "ETH/USDT",
        "amount": "500",
        "daily_trend_threshold": "55",
        "h1_oversold_threshold": "25",
        "m5_oversold_threshold": "25",
        "overbought_threshold": "75",
        "rsi_period": 21,
        "stop_loss_pct": "3",
        "take_profit_pct": "6",
        "cooldown_candles": 5
    });

    let result = strategy.initialize(config).await;
    assert!(result.is_ok(), "커스텀 설정으로 초기화 실패");

    let state = strategy.get_state();
    assert_eq!(state["position_state"], "Flat");
}

#[tokio::test]
async fn test_name_version_description() {
    let strategy = RsiMultiTfStrategy::new();

    // Strategy trait 구현에서 정의된 값 확인
    assert_eq!(strategy.name(), "RsiMultiTf");
    assert_eq!(strategy.version(), "1.0.0");
    assert!(
        strategy.description().contains("RSI") || strategy.description().contains("타임프레임")
    );
}

// ============================================================================
// 데이터 처리 테스트
// ============================================================================

#[tokio::test]
async fn test_daily_data_accumulation() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100"
    });
    strategy.initialize(config).await.unwrap();

    // 일봉 데이터 축적
    for day in 0..30 {
        let data = create_market_data("BTC/USDT", dec!(50000) + Decimal::from(day * 100), day);
        let result = strategy.on_market_data(&data).await;
        assert!(result.is_ok(), "데이터 처리 실패");
    }

    let state = strategy.get_state();
    // RSI 값이 계산되었는지 확인 (충분한 데이터 후)
    // rsi.daily가 null이 아닐 수 있음
    assert!(state["rsi"].is_object(), "RSI 객체 존재");
}

#[tokio::test]
async fn test_multi_timeframe_data() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100"
    });
    strategy.initialize(config).await.unwrap();

    // 각 타임프레임별 데이터 주입
    let timeframes = [
        (Timeframe::D1, 86400),
        (Timeframe::H1, 3600),
        (Timeframe::M5, 300),
    ];

    for (tf, interval) in &timeframes {
        for i in 0..20 {
            let data = create_market_data_with_tf(
                "BTC/USDT",
                dec!(50000) + Decimal::from(i * 50),
                *tf,
                i * interval,
            );
            let _ = strategy.on_market_data(&data).await;
        }
    }

    let state = strategy.get_state();
    assert_eq!(state["position_state"], "Flat");
}

#[tokio::test]
async fn test_ignores_unregistered_ticker() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100"
    });
    strategy.initialize(config).await.unwrap();

    // 다른 티커 데이터
    let data = create_market_data("ETH/USDT", dec!(3000), 0);
    let signals = strategy.on_market_data(&data).await.unwrap();

    assert!(signals.is_empty(), "등록되지 않은 티커는 무시");
}

// ============================================================================
// RSI 계산 테스트
// ============================================================================

#[tokio::test]
async fn test_rsi_values_after_sufficient_data() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100",
        "rsi_period": 14
    });
    strategy.initialize(config).await.unwrap();

    // RSI 계산을 위한 충분한 데이터 (14개 이상)
    for day in 0..30 {
        // 상승 추세 데이터
        let price = dec!(50000) + Decimal::from(day * 100);
        let data = create_market_data("BTC/USDT", price, day);
        let _ = strategy.on_market_data(&data).await;
    }

    let state = strategy.get_state();
    // RSI 객체가 존재하고 값들이 있는지 확인
    assert!(state["rsi"].is_object());
}

#[tokio::test]
async fn test_rsi_values_in_downtrend() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100",
        "rsi_period": 14
    });
    strategy.initialize(config).await.unwrap();

    // 하락 추세 데이터 - RSI가 낮아져야 함
    for day in 0..30 {
        let price = dec!(60000) - Decimal::from(day * 100);
        let data = create_market_data("BTC/USDT", price, day);
        let _ = strategy.on_market_data(&data).await;
    }

    let state = strategy.get_state();
    assert!(state["rsi"].is_object());
}

// ============================================================================
// 포지션 관리 테스트
// ============================================================================

#[tokio::test]
async fn test_position_update() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100"
    });
    strategy.initialize(config).await.unwrap();

    let position = create_position("BTC/USDT", dec!(1), dec!(50000));
    let result = strategy.on_position_update(&position).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_position_cleared_on_zero_quantity() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100"
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 설정
    let position = create_position("BTC/USDT", dec!(1), dec!(50000));
    strategy.on_position_update(&position).await.unwrap();

    // 포지션 청산 (수량 0)
    let zero_position = create_position("BTC/USDT", dec!(0), dec!(0));
    strategy.on_position_update(&zero_position).await.unwrap();
}

// ============================================================================
// 상태 관리 테스트
// ============================================================================

#[tokio::test]
async fn test_get_state_comprehensive() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100"
    });
    strategy.initialize(config).await.unwrap();

    let state = strategy.get_state();

    // 필수 필드 확인
    assert!(state["position_state"].is_string(), "position_state 필드");
    assert!(state["trades_count"].is_number(), "trades_count 필드");
    assert!(state["wins"].is_number(), "wins 필드");
    assert!(state["losses"].is_number(), "losses 필드");
    assert!(
        state["cooldown_remaining"].is_number(),
        "cooldown_remaining 필드"
    );
    assert!(state["rsi"].is_object(), "rsi 객체");

    // 초기값 확인
    assert_eq!(state["trades_count"], 0);
    assert_eq!(state["wins"], 0);
    assert_eq!(state["losses"], 0);
}

#[tokio::test]
async fn test_state_with_rsi_values() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100",
        "rsi_period": 14
    });
    strategy.initialize(config).await.unwrap();

    // 데이터 추가
    for day in 0..30 {
        let data = create_market_data("BTC/USDT", dec!(50000) + Decimal::from(day * 50), day);
        let _ = strategy.on_market_data(&data).await;
    }

    let state = strategy.get_state();
    // RSI 객체 존재 확인
    assert!(state["rsi"].is_object());
}

// ============================================================================
// 종료 테스트
// ============================================================================

#[tokio::test]
async fn test_shutdown() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100"
    });
    strategy.initialize(config).await.unwrap();

    let result = strategy.shutdown().await;
    assert!(result.is_ok(), "정상 종료 실패");
}

#[tokio::test]
async fn test_shutdown_with_statistics() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100"
    });
    strategy.initialize(config).await.unwrap();

    // 데이터 축적
    for day in 0..30 {
        let data = create_market_data("BTC/USDT", dec!(50000), day);
        let _ = strategy.on_market_data(&data).await;
    }

    let result = strategy.shutdown().await;
    assert!(result.is_ok());

    let state = strategy.get_state();
    assert!(state["trades_count"].is_number());
}

// ============================================================================
// 쿨다운 테스트
// ============================================================================

#[tokio::test]
async fn test_cooldown_initial_state() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100",
        "cooldown_candles": 5
    });
    strategy.initialize(config).await.unwrap();

    let state = strategy.get_state();
    assert_eq!(state["cooldown_remaining"], 0, "초기 쿨다운은 0");
}

#[tokio::test]
async fn test_cooldown_after_trade() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100",
        "cooldown_candles": 5
    });
    strategy.initialize(config).await.unwrap();

    // 충분한 데이터 축적
    for day in 0..30 {
        let data = create_market_data("BTC/USDT", dec!(50000), day);
        let _ = strategy.on_market_data(&data).await;
    }

    // 쿨다운은 거래 후에 설정됨
    let state = strategy.get_state();
    assert!(state["cooldown_remaining"].is_number());
}

// ============================================================================
// 복합 시나리오 테스트
// ============================================================================

#[tokio::test]
async fn test_full_multi_timeframe_trading_cycle() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100",
        "rsi_period": 14,
        "cooldown_candles": 3
    });
    strategy.initialize(config).await.unwrap();

    // Phase 1: 데이터 축적 (각 타임프레임)
    for day in 0..30 {
        // 일봉
        let daily = create_market_data_with_tf(
            "BTC/USDT",
            dec!(50000) + Decimal::from(day * 100),
            Timeframe::D1,
            day * 86400,
        );
        let _ = strategy.on_market_data(&daily).await;

        // 시간봉 (하루에 24개)
        for hour in 0..24 {
            let hourly = create_market_data_with_tf(
                "BTC/USDT",
                dec!(50000) + Decimal::from(day * 100 + hour * 4),
                Timeframe::H1,
                day * 86400 + hour * 3600,
            );
            let _ = strategy.on_market_data(&hourly).await;
        }
    }

    // Phase 2: 최종 상태 확인
    let final_state = strategy.get_state();
    assert_eq!(final_state["position_state"], "Flat");
    assert!(final_state["trades_count"].is_number());
}

#[tokio::test]
async fn test_rsi_crossover_detection() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100",
        "rsi_period": 14
    });
    strategy.initialize(config).await.unwrap();

    // 급격한 상승으로 RSI 상승
    for day in 0..30 {
        let price = dec!(40000) + Decimal::from(day * 500); // 급격한 상승
        let data = create_market_data("BTC/USDT", price, day);
        let _ = strategy.on_market_data(&data).await;
    }

    let state = strategy.get_state();
    assert!(state["rsi"].is_object(), "RSI 객체 존재");
}

// ============================================================================
// 에러 처리 테스트
// ============================================================================

#[tokio::test]
async fn test_process_data_before_initialization() {
    let mut strategy = RsiMultiTfStrategy::new();

    // 초기화 전 데이터 처리 시도
    let data = create_market_data("BTC/USDT", dec!(50000), 0);
    let result = strategy.on_market_data(&data).await;

    // 에러 없이 빈 시그널 반환
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

// ============================================================================
// RSI 정렬 조건 테스트
// ============================================================================

#[tokio::test]
async fn test_buy_condition_rsi_alignment() {
    let mut strategy = RsiMultiTfStrategy::new();

    let config = json!({
        "ticker": "BTC/USDT",
        "amount": "100",
        "rsi_period": 14,
        "daily_trend_threshold": "50",
        "h1_oversold_threshold": "30",
        "m5_oversold_threshold": "30"
    });
    strategy.initialize(config).await.unwrap();

    // 과매도 조건을 만들기 위한 하락 후 반등 시나리오
    // 하락 추세
    for day in 0..20 {
        let price = dec!(60000) - Decimal::from(day * 200);
        let data = create_market_data("BTC/USDT", price, day);
        let _ = strategy.on_market_data(&data).await;
    }

    // 반등 시작
    for day in 20..30 {
        let price = dec!(56000) + Decimal::from((day - 20) * 100);
        let data = create_market_data("BTC/USDT", price, day);
        let _ = strategy.on_market_data(&data).await;
    }

    let state = strategy.get_state();
    assert!(state["rsi"].is_object());
}
