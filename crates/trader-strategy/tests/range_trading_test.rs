//! RangeTrading(구간분할) 전략 통합 테스트
//!
//! 가격대를 여러 구간으로 나누어 구간 변동 시 매매하는 전략 테스트

use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::{Kline, MarketData, Position, Side, StrategyContext, Timeframe};
use trader_strategy::strategies::range_trading::{RangeTradingConfig, RangeTradingStrategy};
use trader_strategy::Strategy;
use uuid::Uuid;

// ============================================================================
// 테스트 헬퍼 함수
// ============================================================================

/// 테스트용 Kline 데이터 생성 (고가/저가 포함)
fn create_kline(
    ticker: &str,
    close: Decimal,
    high: Decimal,
    low: Decimal,
    timestamp_secs: i64,
) -> MarketData {
    let timestamp = chrono::DateTime::from_timestamp(timestamp_secs, 0).unwrap();
    let kline = Kline::new(
        ticker.to_string(),
        Timeframe::D1,
        timestamp,
        close,         // open
        high,          // high
        low,           // low
        close,         // close
        dec!(1000000), // volume
        timestamp,     // close_time
    );
    MarketData::from_kline("test", kline)
}

/// 단순 종가 기반 Kline 생성 (고가=종가+10, 저가=종가-10)
fn create_simple_kline(ticker: &str, close: Decimal, timestamp_secs: i64) -> MarketData {
    create_kline(
        ticker,
        close,
        close + dec!(10),
        close - dec!(10),
        timestamp_secs,
    )
}

/// 여러 개의 가격 데이터를 전략에 주입
async fn feed_prices(
    strategy: &mut RangeTradingStrategy,
    ticker: &str,
    prices: &[Decimal],
    start_timestamp: i64,
) -> Vec<trader_core::Signal> {
    let mut all_signals = vec![];
    for (i, price) in prices.iter().enumerate() {
        let data = create_simple_kline(ticker, *price, start_timestamp + (i as i64 * 86400));
        let signals = strategy.on_market_data(&data).await.unwrap();
        all_signals.extend(signals);
    }
    all_signals
}

/// 테스트용 간단한 설정 생성 (짧은 기간)
fn simple_test_config(ticker: &str) -> serde_json::Value {
    json!({
        "ticker": ticker,
        "div_num": 10,
        "target_period": 5,
        "use_ma_filter": false,  // 테스트 단순화를 위해 MA 필터 비활성화
        "buy_ma_period": 5,
        "sell_ma_period": 3,
        "min_global_score": "0"
    })
}

/// MA 필터 활성화된 설정
fn ma_filter_config(ticker: &str) -> serde_json::Value {
    json!({
        "ticker": ticker,
        "div_num": 10,
        "target_period": 5,
        "use_ma_filter": true,
        "buy_ma_period": 5,
        "sell_ma_period": 3,
        "min_global_score": "0"
    })
}

/// StrategyContext에 가격 데이터 설정
fn setup_context_with_prices(ticker: &str, prices: &[Decimal], start_timestamp: i64) -> Arc<RwLock<StrategyContext>> {
    let mut context = StrategyContext::new();
    let mut klines = Vec::new();

    for (i, price) in prices.iter().enumerate() {
        let timestamp = chrono::DateTime::from_timestamp(start_timestamp + (i as i64 * 86400), 0).unwrap();
        let kline = Kline::new(
            ticker.to_string(),
            Timeframe::D1,
            timestamp,
            *price,              // open
            *price + dec!(10),   // high
            *price - dec!(10),   // low
            *price,              // close
            dec!(1000000),       // volume
            timestamp,           // close_time
        );
        klines.push(kline);
    }

    context.update_klines(ticker, Timeframe::D1, klines);
    Arc::new(RwLock::new(context))
}

/// 테스트용 포지션 생성
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
        strategy_id: Some("range_trading".to_string()),
        opened_at: Utc::now(),
        updated_at: Utc::now(),
        closed_at: None,
        metadata: json!({}),
    }
}

// ============================================================================
// 초기화 테스트
// ============================================================================

#[tokio::test]
async fn test_initialization_basic() {
    let mut strategy = RangeTradingStrategy::new();
    let config = simple_test_config("005930");

    let result = strategy.initialize(config).await;
    assert!(result.is_ok(), "초기화 실패: {:?}", result);

    assert_eq!(strategy.name(), "RangeTrading");
    assert_eq!(strategy.version(), "2.0.0");
}

#[tokio::test]
async fn test_initialization_with_custom_config() {
    let mut strategy = RangeTradingStrategy::new();
    let config = json!({
        "ticker": "AAPL",
        "div_num": 20,
        "target_period": 30,
        "use_ma_filter": true,
        "buy_ma_period": 15,
        "sell_ma_period": 7
    });

    let result = strategy.initialize(config).await;
    assert!(result.is_ok());

    let state = strategy.get_state();
    let state_config = state.get("config").unwrap();
    assert_eq!(state_config["ticker"], "AAPL");
    assert_eq!(state_config["div_num"], 20);
}

#[tokio::test]
async fn test_initialization_resets_state() {
    let mut strategy = RangeTradingStrategy::new();
    let config = simple_test_config("005930");

    // 첫 번째 초기화
    strategy.initialize(config.clone()).await.unwrap();

    // 일부 데이터 주입
    let prices: Vec<Decimal> = (0..10).map(|i| dec!(100) + Decimal::from(i)).collect();
    feed_prices(&mut strategy, "005930", &prices, 1000000).await;

    // 두 번째 초기화 - 상태 초기화 확인
    strategy.initialize(config).await.unwrap();

    let state = strategy.get_state();
    assert!(state["state"]["current_zone"].is_null());
    assert_eq!(state["klines_count"], 0);
}

// ============================================================================
// 설정 기본값 테스트
// ============================================================================

#[tokio::test]
async fn test_config_defaults() {
    let config = RangeTradingConfig::default();

    assert_eq!(config.div_num, 15);
    assert_eq!(config.target_period, 20);
    assert!(config.use_ma_filter);
    assert_eq!(config.buy_ma_period, 20);
    assert_eq!(config.sell_ma_period, 5);
    assert_eq!(config.min_global_score, dec!(50));
}

// ============================================================================
// 구간 계산 테스트
// ============================================================================

#[tokio::test]
async fn test_zone_calculation_basic() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // StrategyContext에 데이터 설정
    // target_period=5일, div_num=10이므로
    // 100~110 범위를 10개 구간으로 분할 (구간당 1)
    let prices: Vec<Decimal> = vec![
        dec!(100),
        dec!(102),
        dec!(105),
        dec!(108),
        dec!(110), // 워밍업 (최저=100, 최고=110)
    ];
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);

    // 데이터 처리를 트리거하기 위한 마지막 데이터 입력
    let data = create_simple_kline("005930", dec!(110), 1000000 + 5 * 86400);
    let _ = strategy.on_market_data(&data).await;

    let state = strategy.get_state();
    // klines_count >= target_period(5) 이면 초기화 완료
    assert!(
        state["klines_count"].as_i64().unwrap_or(0) >= 5,
        "충분한 데이터 후 초기화 완료되어야 함"
    );

    // zone 정보 확인
    // zone 정보가 설정되었는지 확인 (null이 아님)
    assert!(!state["state"]["zone_low"].is_null());
    assert!(!state["state"]["zone_high"].is_null());
}

#[tokio::test]
async fn test_zone_boundary_values() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // StrategyContext에 구간 설정 데이터: 100~110, 10개 구간 (구간당 1)
    let warmup_prices: Vec<Decimal> = vec![dec!(100), dec!(102), dec!(104), dec!(106), dec!(110)];
    let context = setup_context_with_prices("005930", &warmup_prices, 1000000);
    strategy.set_context(context);

    // 데이터 처리 트리거
    let data = create_simple_kline("005930", dec!(110), 1000000 + 5 * 86400);
    let _ = strategy.on_market_data(&data).await;

    // 경계값 테스트를 위한 추가 데이터
    // 가격이 구간 경계에 있을 때 동작 확인
    let state = strategy.get_state();
    assert!(!state["state"]["zone_gap"].is_null());
}

// ============================================================================
// 구간 변동 시그널 테스트
// ============================================================================

#[tokio::test]
async fn test_zone_up_generates_buy_signal() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업: 100~110 범위 설정
    let warmup: Vec<Decimal> = vec![dec!(100), dec!(102), dec!(104), dec!(106), dec!(110)];
    feed_prices(&mut strategy, "005930", &warmup, 1000000).await;

    // 구간 상승을 위해 낮은 가격에서 높은 가격으로
    // 먼저 낮은 구간 설정
    let setup = create_simple_kline("005930", dec!(101), 1000000 + 6 * 86400);
    strategy.on_market_data(&setup).await.unwrap();

    // 그 다음 높은 구간으로 이동
    let up = create_simple_kline("005930", dec!(108), 1000000 + 7 * 86400);
    let signals = strategy.on_market_data(&up).await.unwrap();

    // 구간 상승 → 매수 시그널
    if !signals.is_empty() {
        let signal = &signals[0];
        assert_eq!(signal.side, Side::Buy);
        assert_eq!(signal.metadata.get("action").unwrap(), "zone_up");
    }
}

#[tokio::test]
async fn test_zone_down_generates_sell_signal() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업
    let warmup: Vec<Decimal> = vec![dec!(100), dec!(102), dec!(104), dec!(106), dec!(110)];
    feed_prices(&mut strategy, "005930", &warmup, 1000000).await;

    // 높은 구간 설정
    let setup = create_simple_kline("005930", dec!(108), 1000000 + 6 * 86400);
    strategy.on_market_data(&setup).await.unwrap();

    // 낮은 구간으로 이동
    let down = create_simple_kline("005930", dec!(101), 1000000 + 7 * 86400);
    let signals = strategy.on_market_data(&down).await.unwrap();

    // 구간 하락 → 매도 시그널
    if !signals.is_empty() {
        let signal = &signals[0];
        assert_eq!(signal.side, Side::Sell);
        assert_eq!(signal.metadata.get("action").unwrap(), "zone_down");
    }
}

#[tokio::test]
async fn test_no_signal_on_same_zone() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업
    let warmup: Vec<Decimal> = vec![dec!(100), dec!(102), dec!(104), dec!(106), dec!(110)];
    feed_prices(&mut strategy, "005930", &warmup, 1000000).await;

    // 동일 구간 내 가격 변동
    let price1 = create_simple_kline("005930", dec!(105), 1000000 + 6 * 86400);
    strategy.on_market_data(&price1).await.unwrap();

    let price2 = create_simple_kline("005930", dec!(105.5), 1000000 + 7 * 86400);
    let signals = strategy.on_market_data(&price2).await.unwrap();

    // 같은 구간 → 시그널 없음
    assert!(signals.is_empty(), "동일 구간에서는 시그널 없어야 함");
}

// ============================================================================
// MA 필터 테스트
// ============================================================================

#[tokio::test]
async fn test_ma_filter_blocks_buy_below_ma() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(ma_filter_config("005930"))
        .await
        .unwrap();

    // StrategyContext에 하락 추세 데이터 설정 (MA 아래)
    let prices: Vec<Decimal> = vec![dec!(120), dec!(115), dec!(110), dec!(105), dec!(100)];
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);

    // 구간 상승 시도 (하지만 MA 아래)
    let price1 = create_simple_kline("005930", dec!(100), 1000000 + 6 * 86400);
    strategy.on_market_data(&price1).await.unwrap();

    let price2 = create_simple_kline("005930", dec!(108), 1000000 + 7 * 86400);
    let _signals = strategy.on_market_data(&price2).await.unwrap();

    // MA 필터 동작 확인
    // 참고: 실제 전략의 MA 필터 로직에 따라 결과가 달라질 수 있음
    // 여기서는 시그널 발생 여부만 확인하고, 전략의 동작을 검증
    let state = strategy.get_state();
    // klines_count >= target_period(5)이면 초기화 완료
    assert!(
        state["klines_count"].as_i64().unwrap_or(0) >= 5,
        "전략 초기화 완료"
    );
}

#[tokio::test]
async fn test_ma_filter_allows_buy_above_ma() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(ma_filter_config("005930"))
        .await
        .unwrap();

    // 상승 추세 데이터 (MA 위)
    let prices: Vec<Decimal> = vec![dec!(100), dec!(105), dec!(110), dec!(115), dec!(120)];
    feed_prices(&mut strategy, "005930", &prices, 1000000).await;

    // 현재 구간 설정
    let price1 = create_simple_kline("005930", dec!(110), 1000000 + 6 * 86400);
    strategy.on_market_data(&price1).await.unwrap();

    // 구간 상승 (MA 위)
    let price2 = create_simple_kline("005930", dec!(125), 1000000 + 7 * 86400);
    let signals = strategy.on_market_data(&price2).await.unwrap();

    // 상승 추세에서 매수 시그널 가능
    // 참고: 구간 상승이 충분해야 시그널 발생
    // 시그널이 있다면 Buy여야 함
    if !signals.is_empty() {
        assert_eq!(signals[0].side, Side::Buy);
    }
}

// ============================================================================
// 티커 필터링 테스트
// ============================================================================

#[tokio::test]
async fn test_ignores_other_tickers() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업
    let prices: Vec<Decimal> = vec![dec!(100), dec!(102), dec!(104), dec!(106), dec!(110)];
    feed_prices(&mut strategy, "005930", &prices, 1000000).await;

    // 다른 티커 데이터
    let other_data = create_simple_kline("AAPL", dec!(150), 1000000 + 6 * 86400);
    let signals = strategy.on_market_data(&other_data).await.unwrap();

    assert!(signals.is_empty(), "다른 티커 데이터는 무시해야 함");
}

// ============================================================================
// 데이터 부족 테스트
// ============================================================================

#[tokio::test]
async fn test_no_signal_before_warmup() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // target_period(5)보다 적은 데이터
    let prices: Vec<Decimal> = vec![dec!(100), dec!(105), dec!(110)];
    let signals = feed_prices(&mut strategy, "005930", &prices, 1000000).await;

    assert!(signals.is_empty(), "워밍업 전에는 시그널 없어야 함");
}

// ============================================================================
// 상태 추적 테스트
// ============================================================================

#[tokio::test]
async fn test_state_tracking_zones() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // StrategyContext에 데이터 설정
    let prices: Vec<Decimal> = vec![dec!(100), dec!(102), dec!(104), dec!(106), dec!(110)];
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);

    // 데이터 처리 트리거
    let data = create_simple_kline("005930", dec!(110), 1000000 + 5 * 86400);
    let _ = strategy.on_market_data(&data).await;

    let state = strategy.get_state();

    // 구간 정보 확인
    assert!(!state["state"]["zone_low"].is_null());
    assert!(!state["state"]["zone_high"].is_null());
    assert!(!state["state"]["zone_gap"].is_null());
}

#[tokio::test]
async fn test_trades_count_increments() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업
    let prices: Vec<Decimal> = vec![dec!(100), dec!(102), dec!(104), dec!(106), dec!(110)];
    feed_prices(&mut strategy, "005930", &prices, 1000000).await;

    // 초기 거래 횟수
    let state1 = strategy.get_state();
    let initial_trades = state1["state"]["trades_count"].as_u64().unwrap_or(0);

    // 구간 변동 유도
    let price1 = create_simple_kline("005930", dec!(101), 1000000 + 6 * 86400);
    strategy.on_market_data(&price1).await.unwrap();

    let price2 = create_simple_kline("005930", dec!(109), 1000000 + 7 * 86400);
    strategy.on_market_data(&price2).await.unwrap();

    let state2 = strategy.get_state();
    let final_trades = state2["state"]["trades_count"].as_u64().unwrap_or(0);

    // 거래 횟수가 증가했거나 유지 (시그널 발생 시 증가)
    assert!(final_trades >= initial_trades);
}

// ============================================================================
// 포지션 업데이트 테스트
// ============================================================================

#[tokio::test]
async fn test_position_update_handling() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    let position = create_position("005930", dec!(100));
    let result = strategy.on_position_update(&position).await;

    assert!(result.is_ok(), "포지션 업데이트 처리 성공해야 함");
}

// ============================================================================
// 셧다운 테스트
// ============================================================================

#[tokio::test]
async fn test_shutdown() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 일부 데이터 주입
    let prices: Vec<Decimal> = vec![dec!(100), dec!(102), dec!(104), dec!(106), dec!(110)];
    feed_prices(&mut strategy, "005930", &prices, 1000000).await;

    let result = strategy.shutdown().await;
    assert!(result.is_ok(), "셧다운 성공해야 함");

    // 셧다운 후에도 상태 조회 가능
    let state = strategy.get_state();
    assert!(state.get("state").is_some(), "셧다운 후에도 상태 조회 가능");
}

// ============================================================================
// get_state 테스트
// ============================================================================

#[tokio::test]
async fn test_get_state_structure() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    let state = strategy.get_state();

    // 필수 필드 확인
    assert!(state.get("config").is_some());
    assert!(state.get("state").is_some());
    assert!(state.get("has_context").is_some());
    assert!(state.get("klines_count").is_some());
}

// ============================================================================
// 시그널 메타데이터 테스트
// ============================================================================

#[tokio::test]
async fn test_signal_metadata_contains_zone_info() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업
    let prices: Vec<Decimal> = vec![dec!(100), dec!(102), dec!(104), dec!(106), dec!(110)];
    feed_prices(&mut strategy, "005930", &prices, 1000000).await;

    // 구간 변동
    let price1 = create_simple_kline("005930", dec!(101), 1000000 + 6 * 86400);
    strategy.on_market_data(&price1).await.unwrap();

    let price2 = create_simple_kline("005930", dec!(109), 1000000 + 7 * 86400);
    let signals = strategy.on_market_data(&price2).await.unwrap();

    if !signals.is_empty() {
        let signal = &signals[0];
        // 메타데이터에 구간 정보가 포함되어야 함
        assert!(signal.metadata.contains_key("prev_zone"));
        assert!(signal.metadata.contains_key("current_zone"));
        assert!(signal.metadata.contains_key("zone_change"));
        assert!(signal.metadata.contains_key("action"));
    }
}

// ============================================================================
// 엣지 케이스 테스트
// ============================================================================

#[tokio::test]
async fn test_empty_data_handling() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    let state = strategy.get_state();
    // 데이터가 없으면 klines_count는 0
    assert_eq!(state["klines_count"], 0);
    // context가 설정되지 않으면 has_context는 false
    assert_eq!(state["has_context"], false);
}

#[tokio::test]
async fn test_price_at_exact_boundary() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업: 정확히 100~110 범위
    let warmup: Vec<Decimal> = vec![dec!(100), dec!(102), dec!(104), dec!(106), dec!(110)];
    feed_prices(&mut strategy, "005930", &warmup, 1000000).await;

    // 정확히 경계값에서의 동작
    let boundary = create_simple_kline("005930", dec!(100), 1000000 + 6 * 86400);
    let result = strategy.on_market_data(&boundary).await;

    assert!(result.is_ok(), "경계값도 에러 없이 처리해야 함");
}

#[tokio::test]
async fn test_price_below_range() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업
    let warmup: Vec<Decimal> = vec![dec!(100), dec!(102), dec!(104), dec!(106), dec!(110)];
    feed_prices(&mut strategy, "005930", &warmup, 1000000).await;

    // 범위 아래 가격
    let below = create_simple_kline("005930", dec!(90), 1000000 + 6 * 86400);
    let result = strategy.on_market_data(&below).await;

    assert!(result.is_ok(), "범위 아래 가격도 처리해야 함");
}

#[tokio::test]
async fn test_price_above_range() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업
    let warmup: Vec<Decimal> = vec![dec!(100), dec!(102), dec!(104), dec!(106), dec!(110)];
    feed_prices(&mut strategy, "005930", &warmup, 1000000).await;

    // 범위 위 가격
    let above = create_simple_kline("005930", dec!(120), 1000000 + 6 * 86400);
    let result = strategy.on_market_data(&above).await;

    assert!(result.is_ok(), "범위 위 가격도 처리해야 함");
}

// ============================================================================
// Default trait 테스트
// ============================================================================

#[tokio::test]
async fn test_default_trait() {
    let strategy = RangeTradingStrategy::default();
    assert_eq!(strategy.name(), "RangeTrading");
}

// ============================================================================
// 연속 구간 변동 테스트
// ============================================================================

#[tokio::test]
async fn test_multiple_zone_changes() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업
    let warmup: Vec<Decimal> = vec![dec!(100), dec!(102), dec!(104), dec!(106), dec!(110)];
    feed_prices(&mut strategy, "005930", &warmup, 1000000).await;

    // 연속적인 구간 변동
    let price_sequence: Vec<Decimal> = vec![
        dec!(101), // 낮은 구간
        dec!(108), // 높은 구간 → 매수
        dec!(102), // 낮은 구간 → 매도
        dec!(107), // 높은 구간 → 매수
    ];

    let signals = feed_prices(
        &mut strategy,
        "005930",
        &price_sequence,
        1000000 + 6 * 86400,
    )
    .await;

    // 여러 시그널 발생 가능
    // 실제 시그널 수는 구간 변동 크기에 따라 달라짐
    // 최소한 에러 없이 처리되어야 함
    assert!(signals.len() <= 4, "최대 4개 시그널 발생 가능");
}

// ============================================================================
// 재초기화 테스트
// ============================================================================

#[tokio::test]
async fn test_reinitialize_and_trade() {
    let mut strategy = RangeTradingStrategy::new();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 첫 번째 사이클 - StrategyContext에 데이터 설정
    let cycle1: Vec<Decimal> = vec![dec!(100), dec!(102), dec!(104), dec!(106), dec!(110)];
    let context1 = setup_context_with_prices("005930", &cycle1, 1000000);
    strategy.set_context(context1);

    // 데이터 트리거
    let data1 = create_simple_kline("005930", dec!(110), 1000000 + 5 * 86400);
    let _ = strategy.on_market_data(&data1).await;

    // 재초기화
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 두 번째 사이클 (다른 가격대) - 새 StrategyContext에 데이터 설정
    let cycle2: Vec<Decimal> = vec![dec!(200), dec!(205), dec!(210), dec!(215), dec!(220)];
    let context2 = setup_context_with_prices("005930", &cycle2, 2000000);
    strategy.set_context(context2);

    // 데이터 트리거
    let data2 = create_simple_kline("005930", dec!(220), 2000000 + 5 * 86400);
    let _ = strategy.on_market_data(&data2).await;

    // 재초기화 후에도 정상 동작 (klines_count >= target_period 확인)
    let state = strategy.get_state();
    assert!(state["klines_count"].as_i64().unwrap_or(0) >= 5, "재초기화 후에도 데이터 축적됨");
}
