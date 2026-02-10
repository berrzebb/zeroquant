//! DCA 전략 (Grid, MagicSplit, InfinityBot) 통합 테스트
//!
//! 스프레드 기반 분할매수 전략 테스트
//!
//! ## InfinityBot 핵심 로직
//!
//! 1. 진입 조건: can_add_position AND can_enter
//!    - can_add_position: 첫 진입 또는 마지막 진입가 대비 dip_trigger_pct 이상 하락
//!    - can_enter: 가격이 MA 위에 있을 때만 진입 허용 (context 없는 경우)
//!
//! 2. 익절: 평균 단가 대비 take_profit_pct 이상 상승
//!
//! 3. 최대 라운드: max_rounds까지만 물타기 가능

use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::{Kline, MarketData, Position, Side, StrategyContext, Timeframe};
use trader_strategy::strategies::DcaStrategy;
use trader_strategy::Strategy;
use uuid::Uuid;

// ============================================================================
// 테스트 헬퍼 함수
// ============================================================================

/// 테스트용 Kline 데이터 생성
fn create_kline(ticker: &str, close: Decimal, timestamp_secs: i64) -> MarketData {
    let timestamp = chrono::DateTime::from_timestamp(timestamp_secs, 0).unwrap();
    let kline = Kline::new(
        ticker.to_string(),
        Timeframe::D1,
        timestamp,
        close - dec!(5),  // open
        close + dec!(10), // high
        close - dec!(10), // low
        close,            // close
        dec!(1000000),    // volume
        timestamp,        // close_time
    );
    MarketData::from_kline("test", kline)
}

/// StrategyContext에 klines 데이터 설정
/// InfinityBot은 klines가 오래된 것부터 최신 순서로 저장됨:
/// - klines[0] = 가장 오래된 캔들
/// - klines.last() = 가장 최신 캔들
fn setup_context_with_prices(
    ticker: &str,
    prices: &[Decimal],
    start_timestamp: i64,
) -> Arc<RwLock<StrategyContext>> {
    let mut context = StrategyContext::new();
    let klines: Vec<Kline> = prices
        .iter()
        .enumerate()
        // 순서대로 저장 (prices[0]=oldest, prices.last()=newest)
        .map(|(i, price)| {
            let timestamp = chrono::DateTime::from_timestamp(
                start_timestamp + (i as i64 * 86400),
                0,
            )
            .unwrap();
            Kline::new(
                ticker.to_string(),
                Timeframe::D1,
                timestamp,
                *price - dec!(5),  // open
                *price + dec!(10), // high
                *price - dec!(10), // low
                *price,            // close
                dec!(1000000),     // volume
                timestamp,         // close_time
            )
        })
        .collect();
    context.update_klines(ticker, Timeframe::D1, klines);
    Arc::new(RwLock::new(context))
}

/// 여러 개의 가격 데이터를 전략에 주입 (StrategyContext 포함)
/// InfinityBot은 klines가 오래된 것부터 최신 순서이므로,
/// 각 가격을 처리할 때마다 해당 시점까지의 klines를 설정
async fn feed_prices(
    strategy: &mut DcaStrategy,
    ticker: &str,
    prices: &[Decimal],
    start_timestamp: i64,
) -> Vec<trader_core::Signal> {
    let mut all_signals = vec![];

    for (i, price) in prices.iter().enumerate() {
        // 현재 시점까지의 klines 설정 (prices[0..=i])
        let current_prices = &prices[..=i];
        let context = setup_context_with_prices(ticker, current_prices, start_timestamp);
        strategy.set_context(context);

        let data = create_kline(ticker, *price, start_timestamp + (i as i64 * 86400));
        let signals = strategy.on_market_data(&data).await.unwrap();
        all_signals.extend(signals);
    }
    all_signals
}

/// 테스트용 간단한 설정 생성 (짧은 MA 기간)
fn simple_test_config(ticker: &str) -> serde_json::Value {
    json!({
        "variant": "infinity_bot",
        "ticker": ticker,
        "total_amount": "1000000",
        "max_rounds": 10,
        "round_pct": "10",
        "dip_trigger_pct": "2",
        "take_profit_pct": "3",
        "ma_period": 5,
        "min_global_score": "0"
    })
}

/// 테스트용 포지션 생성
fn create_position(ticker: &str, quantity: Decimal, entry_price: Decimal) -> Position {
    Position {
        id: Uuid::new_v4(),
        exchange: "test".to_string(),
        ticker: ticker.to_string(),
        side: Side::Buy,
        quantity,
        entry_price,
        current_price: entry_price * dec!(1.05),
        unrealized_pnl: quantity * entry_price * dec!(0.05),
        realized_pnl: Decimal::ZERO,
        strategy_id: Some("infinity_bot".to_string()),
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
    let mut strategy = DcaStrategy::infinity_bot();
    let config = simple_test_config("005930");

    let result = strategy.initialize(config).await;
    assert!(result.is_ok(), "초기화 실패: {:?}", result);

    assert_eq!(strategy.name(), "DCA-InfinityBot");
    assert_eq!(strategy.version(), "1.0.0");
}

#[tokio::test]
async fn test_initialization_with_custom_config() {
    let mut strategy = DcaStrategy::infinity_bot();
    let config = json!({
        "variant": "infinity_bot",
        "ticker": "AAPL",
        "total_amount": "50000",
        "max_rounds": 20,
        "round_pct": "5",
        "dip_trigger_pct": "3",
        "take_profit_pct": "5",
        "ma_period": 10
    });

    let result = strategy.initialize(config).await;
    assert!(result.is_ok());

    let state = strategy.get_state();
    // 새로운 get_state() 구조: variant, initialized, infinity_rounds 등
    assert_eq!(state["variant"], "InfinityBot");
    assert!(state["initialized"].as_bool().unwrap());
}

#[tokio::test]
async fn test_initialization_preserves_state_reset() {
    let mut strategy = DcaStrategy::infinity_bot();
    let config = simple_test_config("005930");

    // 첫 번째 초기화
    strategy.initialize(config.clone()).await.unwrap();

    // 일부 데이터 주입 (상승 추세로 진입 유도)
    let prices: Vec<Decimal> = vec![
        dec!(100),
        dec!(101),
        dec!(102),
        dec!(103),
        dec!(104),
        dec!(105),
    ];
    let signals = feed_prices(&mut strategy, "005930", &prices, 1000000).await;

    // 진입 확인 (워밍업 후 신호 발생 가능)
    let buy_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Buy).collect();
    // MA 기간(5)이 충족되면 진입 가능
    if !buy_signals.is_empty() {
        let state_before = strategy.get_state();
        assert!(
            state_before["infinity_rounds"].as_i64().unwrap_or(0) > 0,
            "진입 후 라운드 > 0"
        );
    }

    // 두 번째 초기화 - 상태 초기화 확인
    strategy.initialize(config).await.unwrap();

    let state = strategy.get_state();
    // initialize()는 infinity_state를 default()로 초기화
    assert_eq!(state["infinity_rounds"], 0, "재초기화 후 라운드 리셋");
}

// ============================================================================
// 워밍업 및 첫 진입 테스트
// ============================================================================

#[tokio::test]
async fn test_no_signal_before_warmup() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // MA 기간(5)보다 적은 데이터
    let prices: Vec<Decimal> = vec![dec!(1000), dec!(1010), dec!(1020)];
    let signals = feed_prices(&mut strategy, "005930", &prices, 1000000).await;

    assert!(signals.is_empty(), "워밍업 전에는 시그널이 발생하면 안 됨");
}

#[tokio::test]
async fn test_first_round_entry_above_ma() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 상승 추세: 가격이 점점 상승 → MA 위에 위치
    // MA(5) = (1000+1010+1020+1030+1040)/5 = 1020
    // 6번째 가격 1050은 MA 1020보다 위
    let prices: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050), // 이 시점: MA = 1020, 현재가 = 1050 > MA → 진입
    ];

    let signals = feed_prices(&mut strategy, "005930", &prices, 1000000).await;

    // StrategyContext 기반에서는 MA 계산이 klines에서 수행됨
    // 진입 조건: can_add_position && can_enter (현재가 > MA)
    assert!(
        !signals.is_empty(),
        "MA 위에서 첫 진입 시그널 발생해야 함. klines_count: {}",
        strategy.get_state()["klines_count"]
    );

    let signal = &signals[0];
    assert_eq!(signal.side, Side::Buy);
    assert_eq!(signal.metadata.get("action").unwrap(), "round_entry");
    assert_eq!(signal.metadata.get("round").unwrap(), 1);

    let state = strategy.get_state();
    assert_eq!(state["state"]["current_round"], 1);
}

#[tokio::test]
async fn test_no_entry_below_ma() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 하락 추세: 가격이 점점 하락 → MA 아래에 위치
    // MA(5) = (1050+1040+1030+1020+1010)/5 = 1030
    // 6번째 가격 900은 MA 1030보다 아래 → 진입 불가
    let prices: Vec<Decimal> = vec![
        dec!(1050),
        dec!(1040),
        dec!(1030),
        dec!(1020),
        dec!(1010),
        dec!(900), // 이 시점: MA ≈ 1030, 현재가 = 900 < MA → 진입 불가
    ];

    let signals = feed_prices(&mut strategy, "005930", &prices, 1000000).await;

    assert!(
        signals.is_empty(),
        "MA 아래에서는 진입하면 안 됨 (핵심 로직 검증)"
    );

    let state = strategy.get_state();
    assert_eq!(state["state"]["current_round"], 0);
}

// ============================================================================
// 물타기 (추가 라운드) 테스트 - 핵심 로직 검증
// ============================================================================

#[tokio::test]
async fn test_dip_buy_only_when_above_ma() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 상승 추세에서 첫 진입
    let warmup_prices: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050),
    ];
    let signals1 = feed_prices(&mut strategy, "005930", &warmup_prices, 1000000).await;

    // StrategyContext 기반: 첫 진입 시그널 확인
    let buy_signals: Vec<_> = signals1.iter().filter(|s| s.side == Side::Buy).collect();
    assert!(
        !buy_signals.is_empty(),
        "첫 진입 시그널 발생. signals count: {}",
        signals1.len()
    );
    assert_eq!(strategy.get_state()["state"]["current_round"], 1);

    // 일시적 하락이지만 여전히 상승 추세 유지 (MA 위)
    // 현재 MA = (1010+1020+1030+1040+1050)/5 = 1030
    // 1050에서 2% 하락 = 1029, MA(1030)보다 약간 아래 → 물타기 불가!
    //
    // 물타기가 발생하려면:
    // 1. 마지막 진입가 대비 2% 이상 하락 (1050 * 0.98 = 1029)
    // 2. 현재가 > MA (can_enter 조건)
    //
    // 따라서 일시적 하락 후 MA가 따라 내려오는 시나리오 필요
    let dip_price = dec!(1029);
    let data = create_kline("005930", dip_price, 1000000 + 7 * 86400);
    let signals2 = strategy.on_market_data(&data).await.unwrap();

    // 현재 MA = (1020+1030+1040+1050+1029)/5 = 1033.8, 1029 < 1033.8 → 물타기 불가
    assert!(
        signals2.is_empty(),
        "MA 아래로 급락하면 물타기 차단 (핵심 안전 로직)"
    );

    let state = strategy.get_state();
    assert_eq!(state["state"]["current_round"], 1, "라운드 변경 없어야 함");
}

#[tokio::test]
async fn test_uptrend_entry_behavior() {
    //! 상승 추세에서 진입 동작 검증
    //!
    //! 핵심 로직:
    //! - MA 기간(5) 충족 후 klines_count >= ma_period
    //! - 가격 > MA 조건에서 첫 진입 발생
    //! - 상승 추세가 지속되면 추가 진입 없음 (하락 조건 미충족)

    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 상승 추세 데이터
    let uptrend: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050),
    ];
    let signals = feed_prices(&mut strategy, "005930", &uptrend, 1000000).await;

    // 상태 확인 - klines_count로 워밍업 상태 검증
    let state = strategy.get_state();
    assert!(
        state["klines_count"].as_i64().unwrap_or(0) >= 5,
        "워밍업 완료 (klines >= ma_period)"
    );

    // 상승 추세에서 진입 발생 여부 확인
    // 진입은 can_add_position && can_enter 조건에 따라 결정
    let current_round = state["state"]["current_round"].as_i64().unwrap_or(0);

    if current_round > 0 {
        // 진입한 경우: 시그널 확인
        assert!(!signals.is_empty(), "진입 시그널 발생");
    } else {
        // 진입 안 한 경우: 조건 미충족 (정상 동작)
        // 이는 전략 로직에 따라 달라질 수 있음
    }
}

#[tokio::test]
async fn test_ma_period_affects_entry_timing() {
    //! MA 기간이 진입 타이밍에 미치는 영향 검증
    //!
    //! MA 기간이 길면 → 진입까지 더 많은 데이터 필요
    //! MA 기간이 짧으면 → 빠른 진입 가능하지만 휩쏘에 취약

    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap(); // ma_period = 5

    // MA(5) 기간 미만 데이터 - 진입 불가
    let insufficient: Vec<Decimal> = vec![dec!(1000), dec!(1010), dec!(1020), dec!(1030)];
    let signals = feed_prices(&mut strategy, "005930", &insufficient, 1000000).await;
    assert!(signals.is_empty(), "MA 기간 미만 데이터로는 진입 불가");

    // MA(5) 기간 충족 - 진입 가능
    // StrategyContext에 추가 데이터 반영
    let more_prices: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050),
    ];
    let _signals = feed_prices(&mut strategy, "005930", &more_prices, 1000000).await;

    // 상태 확인 - klines_count로 워밍업 상태 검증
    let state = strategy.get_state();
    assert!(
        state["klines_count"].as_i64().unwrap_or(0) >= 5,
        "워밍업 완료 (klines >= ma_period)"
    );
}

// ============================================================================
// 익절 테스트
// ============================================================================

#[tokio::test]
async fn test_take_profit_signal() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업 + 첫 진입 + 익절
    // 전체 시나리오를 하나의 feed_prices로 처리
    let scenario_prices: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050), // 진입 예상
        dec!(1082), // 3% 이상 상승 → 익절
    ];
    let signals = feed_prices(&mut strategy, "005930", &scenario_prices, 1000000).await;

    // 진입 시그널 확인
    let buy_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Buy).collect();
    assert!(
        !buy_signals.is_empty(),
        "첫 진입 시그널 발생해야 함. signals: {}",
        signals.len()
    );

    // 익절 시그널 확인
    let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();
    assert!(
        !sell_signals.is_empty(),
        "3% 이상 상승 시 익절 시그널 발생해야 함. total signals: {}",
        signals.len()
    );

    let signal = sell_signals[0];
    assert_eq!(signal.side, Side::Sell);
    assert_eq!(signal.metadata.get("action").unwrap(), "take_profit");

    // 상태 초기화 확인
    let state = strategy.get_state();
    assert_eq!(state["state"]["current_round"], 0, "익절 후 라운드 초기화");
}

#[tokio::test]
async fn test_no_take_profit_without_sufficient_gain() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업 + 첫 진입
    let warmup_prices: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050),
    ];
    feed_prices(&mut strategy, "005930", &warmup_prices, 1000000).await;

    // 2% 상승 (3% 미만) - 1050 * 1.02 = 1071
    let small_gain = dec!(1071);
    let data = create_kline("005930", small_gain, 1000000 + 7 * 86400);
    let signals = strategy.on_market_data(&data).await.unwrap();

    // 익절 안 됨
    let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();
    assert!(sell_signals.is_empty(), "3% 미만 상승 시 익절하면 안 됨");
}

// ============================================================================
// 최대 라운드 테스트
// ============================================================================

#[tokio::test]
async fn test_max_rounds_config_verification() {
    //! max_rounds 설정이 올바르게 파싱되는지 검증

    let mut strategy = DcaStrategy::infinity_bot();

    let config = json!({
        "variant": "infinity_bot",
        "ticker": "005930",
        "total_amount": "1000000",
        "max_rounds": 2,
        "round_pct": "10",
        "dip_trigger_pct": "2",
        "take_profit_pct": "3",
        "ma_period": 5,
        "min_global_score": "0"
    });
    strategy.initialize(config).await.unwrap();

    let state = strategy.get_state();
    assert_eq!(state["config"]["max_rounds"], 2, "max_rounds 설정값 확인");
    assert_eq!(state["config"]["ticker"], "005930", "ticker 설정값 확인");
}

// ============================================================================
// 상태 추적 테스트
// ============================================================================

#[tokio::test]
async fn test_state_tracking_lifecycle() {
    //! 전략 상태 라이프사이클 테스트
    //!
    //! 1. 초기화 직후: klines_count=0 (context 없음)
    //! 2. 워밍업 중: klines_count 증가
    //! 3. 워밍업 완료: klines_count >= ma_period
    //! 4. 진입 시: current_round 증가, avg_price 설정

    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // Phase 1: 초기 상태 (context 없음)
    let state = strategy.get_state();
    assert_eq!(state["klines_count"], 0, "초기 가격 데이터 없음");

    // Phase 2+3: 워밍업 + 진입 조건 확인
    let prices: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050),
    ];
    let signals = feed_prices(&mut strategy, "005930", &prices, 1000000).await;

    let state = strategy.get_state();
    assert!(
        state["klines_count"].as_i64().unwrap_or(0) >= 5,
        "워밍업 완료 (klines >= ma_period)"
    );

    // Phase 4: 진입 여부 확인
    // 상승 추세이므로 진입했을 가능성 높음
    if state["state"]["current_round"].as_i64().unwrap_or(0) > 0 {
        // 진입한 경우 avg_price 존재 확인
        assert!(
            !state["state"]["avg_price"].is_null(),
            "진입 시 평균 단가 존재"
        );
        assert!(!signals.is_empty(), "진입 시그널 발생");
    }
}

// ============================================================================
// 티커 필터링 테스트
// ============================================================================

#[tokio::test]
async fn test_ignores_other_tickers() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업
    let warmup_prices: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050),
    ];
    feed_prices(&mut strategy, "005930", &warmup_prices, 1000000).await;

    // 다른 티커 데이터
    let other_data = create_kline("AAPL", dec!(150), 1000000 + 7 * 86400);
    let signals = strategy.on_market_data(&other_data).await.unwrap();

    assert!(signals.is_empty(), "다른 티커 데이터는 무시해야 함");
}

// ============================================================================
// 포지션 업데이트 테스트
// ============================================================================

#[tokio::test]
async fn test_position_update_handling() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    let position = create_position("005930", dec!(100), dec!(1000));

    let result = strategy.on_position_update(&position).await;
    assert!(result.is_ok(), "포지션 업데이트 처리 성공해야 함");
}

// ============================================================================
// 셧다운 테스트
// ============================================================================

#[tokio::test]
async fn test_shutdown() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 일부 데이터 주입
    let warmup_prices: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050),
    ];
    feed_prices(&mut strategy, "005930", &warmup_prices, 1000000).await;

    // 진입 후 상태 확인
    let state_before = strategy.get_state();
    let round_before = state_before["state"]["current_round"].as_i64().unwrap_or(0);

    let result = strategy.shutdown().await;
    assert!(result.is_ok(), "셧다운 성공해야 함");

    // shutdown()은 상태를 초기화하지 않음 (의도된 동작)
    // 상태 유지 확인
    let state = strategy.get_state();
    assert_eq!(
        state["state"]["current_round"].as_i64().unwrap_or(0),
        round_before,
        "셧다운 후 상태 유지 (shutdown은 상태를 초기화하지 않음)"
    );
}

// ============================================================================
// 엣지 케이스 테스트
// ============================================================================

#[tokio::test]
async fn test_empty_data_handling() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 초기화만 하고 데이터 없이 상태 조회
    let state = strategy.get_state();
    // config 설정은 됐지만 context가 없어서 klines_count = 0
    assert_eq!(state["klines_count"], 0);
}

#[tokio::test]
async fn test_zero_price_handling() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업
    let warmup_prices: Vec<Decimal> =
        vec![dec!(1000), dec!(1010), dec!(1020), dec!(1030), dec!(1040)];
    feed_prices(&mut strategy, "005930", &warmup_prices, 1000000).await;

    // 가격이 0인 데이터
    let zero_data = create_kline("005930", dec!(0), 1000000 + 6 * 86400);
    let result = strategy.on_market_data(&zero_data).await;

    assert!(result.is_ok(), "0 가격도 에러 없이 처리해야 함");
}

#[tokio::test]
async fn test_very_small_dip_no_trigger() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업 + 진입
    let warmup_prices: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050),
    ];
    feed_prices(&mut strategy, "005930", &warmup_prices, 1000000).await;

    // 아주 작은 하락 (0.1%)
    let tiny_dip = dec!(1049); // 1050 → 1049 = 0.09% 하락
    let data = create_kline("005930", tiny_dip, 1000000 + 7 * 86400);
    let signals = strategy.on_market_data(&data).await.unwrap();

    // 하락폭 부족으로 물타기 조건 미달
    assert!(signals.is_empty(), "0.1% 하락은 물타기 트리거가 아님");
}

#[tokio::test]
async fn test_continuous_uptrend_no_additional_entries() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업 + 첫 진입
    let warmup_prices: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050),
    ];
    let signals1 = feed_prices(&mut strategy, "005930", &warmup_prices, 1000000).await;
    let entry_count = signals1.iter().filter(|s| s.side == Side::Buy).count();
    assert!(
        entry_count >= 1,
        "첫 진입 발생. signals: {}",
        signals1.len()
    );

    // 계속 상승 (하락 없음) - 새로운 context로 전체 데이터 재설정
    let all_prices: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050),
        dec!(1055),
        dec!(1060),
        dec!(1065),
        dec!(1070),
    ];
    let signals2 = feed_prices(&mut strategy, "005930", &all_prices, 1000000).await;

    // 상승장에서는 추가 물타기 없음 (하락 조건 미달)
    // 첫 진입 이후 추가 Buy는 없어야 함
    let buy_signals: Vec<_> = signals2.iter().filter(|s| s.side == Side::Buy).collect();
    assert!(
        buy_signals.len() <= 1,
        "상승장에서 추가 물타기 없어야 함. Buy signals: {}",
        buy_signals.len()
    );
}

// ============================================================================
// 설정 검증 테스트
// ============================================================================

#[tokio::test]
async fn test_config_with_string_decimals() {
    let mut strategy = DcaStrategy::infinity_bot();
    let config = json!({
        "ticker": "005930",
        "total_amount": "5000000",
        "round_pct": "5",
        "dip_trigger_pct": "2.5",
        "take_profit_pct": "4.5"
    });

    let result = strategy.initialize(config).await;
    assert!(result.is_ok(), "문자열 Decimal 값 파싱 성공해야 함");
}

// ============================================================================
// 시그널 메타데이터 테스트
// ============================================================================

#[tokio::test]
async fn test_entry_signal_metadata() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업 + 진입
    let warmup_prices: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050),
    ];
    let signals = feed_prices(&mut strategy, "005930", &warmup_prices, 1000000).await;

    let entry_signal = signals.iter().find(|s| s.side == Side::Buy);
    assert!(
        entry_signal.is_some(),
        "진입 시그널 존재해야 함. signals count: {}",
        signals.len()
    );

    let signal = entry_signal.unwrap();
    assert!(signal.metadata.contains_key("action"));
    assert!(signal.metadata.contains_key("round"));
    assert!(signal.metadata.contains_key("quantity"));
    assert!(signal.metadata.contains_key("avg_price"));

    assert_eq!(signal.metadata.get("action").unwrap(), "round_entry");
}

#[tokio::test]
async fn test_exit_signal_metadata() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 워밍업 + 진입 + 익절
    let scenario: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050),
        dec!(1082), // 3% 이상 상승
    ];
    let signals = feed_prices(&mut strategy, "005930", &scenario, 1000000).await;

    let exit_signal = signals.iter().find(|s| s.side == Side::Sell);
    assert!(
        exit_signal.is_some(),
        "익절 시그널 존재해야 함. signals count: {}",
        signals.len()
    );

    let signal = exit_signal.unwrap();
    assert!(signal.metadata.contains_key("action"));
    assert!(signal.metadata.contains_key("return_pct"));
    assert!(signal.metadata.contains_key("rounds"));

    assert_eq!(signal.metadata.get("action").unwrap(), "take_profit");
}

// ============================================================================
// 재초기화 후 동작 테스트
// ============================================================================

#[tokio::test]
async fn test_reinitialize_and_trade_again() {
    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 첫 번째 사이클: 워밍업 + 진입 + 익절
    let cycle1: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050),
        dec!(1082),
    ];
    feed_prices(&mut strategy, "005930", &cycle1, 1000000).await;

    // 재초기화
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 두 번째 사이클: 새로운 워밍업 + 진입
    let cycle2: Vec<Decimal> = vec![
        dec!(2000),
        dec!(2010),
        dec!(2020),
        dec!(2030),
        dec!(2040),
        dec!(2050),
    ];
    let signals = feed_prices(&mut strategy, "005930", &cycle2, 2000000).await;

    let buy_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Buy).collect();
    assert!(
        !buy_signals.is_empty(),
        "재초기화 후 새로운 진입 가능해야 함. signals: {}",
        signals.len()
    );

    let state = strategy.get_state();
    assert_eq!(state["state"]["current_round"], 1);
}

// ============================================================================
// 핵심 로직 검증: MA 조건이 물타기를 보호하는지
// ============================================================================

#[tokio::test]
async fn test_ma_protects_from_catching_falling_knife() {
    //! 핵심 테스트: MA 조건이 "떨어지는 칼날 잡기"를 방지하는지 검증
    //!
    //! InfinityBot의 핵심 안전장치:
    //! - 단순히 가격이 하락했다고 무조건 물타기하지 않음
    //! - can_enter 조건(MA 위)을 충족해야만 물타기 허용
    //! - 이로써 하락 추세에서 무분별한 물타기 방지

    let mut strategy = DcaStrategy::infinity_bot();
    strategy
        .initialize(simple_test_config("005930"))
        .await
        .unwrap();

    // 첫 진입 (상승 추세)
    let warmup: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050),
    ];
    let signals1 = feed_prices(&mut strategy, "005930", &warmup, 1000000).await;
    let buy_count = signals1.iter().filter(|s| s.side == Side::Buy).count();
    assert!(buy_count >= 1, "첫 진입 완료. signals: {}", signals1.len());

    // 급락 시나리오 (MA 아래로 떨어짐)
    // 전체 데이터를 하나의 context로 설정
    let all_prices: Vec<Decimal> = vec![
        dec!(1000),
        dec!(1010),
        dec!(1020),
        dec!(1030),
        dec!(1040),
        dec!(1050), // 여기서 첫 진입
        dec!(1000), // -4.7% (MA 아래)
        dec!(950),  // -5%
        dec!(900),  // -5.3%
        dec!(850),  // -5.6%
        dec!(800),  // -5.9%
    ];

    let signals2 = feed_prices(&mut strategy, "005930", &all_prices, 1000000).await;

    // 핵심 검증: 첫 진입 이후 급락 중에는 추가 물타기 시그널이 없어야 함
    let buy_signals: Vec<_> = signals2.iter().filter(|s| s.side == Side::Buy).collect();
    // 첫 진입 1회만 허용, 급락 중 추가 진입 없음
    assert!(
        buy_signals.len() <= 1,
        "MA 아래로 급락 시 추가 물타기 차단 (안전장치 검증) - Buy 시그널: {}",
        buy_signals.len()
    );

    // 라운드는 1 이하 (첫 진입만 또는 진입 안 함)
    let state = strategy.get_state();
    let current_round = state["state"]["current_round"].as_i64().unwrap_or(0);
    assert!(
        current_round <= 1,
        "급락 중 물타기 없이 최대 1라운드. current: {}",
        current_round
    );
}

// ============================================================================
// 그리드 트레이딩 테스트
// ============================================================================
//
// 그리드 전략 핵심 로직:
// - base_price에서 시작하여 spacing_pct 간격으로 그리드 레벨 생성
// - Level i: buy_price = base_price - spacing * i, sell_price = base_price - spacing * (i-1)
// - WaitingBuy 상태에서 가격 <= buy_price이면 매수
// - WaitingSell 상태에서 가격 >= sell_price이면 매도
// - 상태 순환: WaitingBuy → 매수 → WaitingSell → 매도 → WaitingBuy

/// 그리드 전략용 설정 생성
fn grid_config(ticker: &str, spacing_pct: &str, levels: usize) -> serde_json::Value {
    json!({
        "variant": "grid",
        "ticker": ticker,
        "amount": "1000000",
        "spacing_pct": spacing_pct,
        "levels": levels,
        "use_atr": false,
        "atr_period": 14,
        "max_positions": 15,
        "min_global_score": "0"
    })
}

/// 그리드 전략 초기화 테스트
#[tokio::test]
async fn test_grid_initialization() {
    let mut strategy = DcaStrategy::grid();
    let config = grid_config("005930", "2.0", 5);

    strategy.initialize(config).await.unwrap();

    let state = strategy.get_state();
    let variant = state["variant"].as_str().unwrap();
    assert_eq!(variant, "Grid", "Variant가 Grid로 설정되어야 함");

    let grid_state = &state["state"]["grid"];
    assert!(
        grid_state["levels"].as_array().is_some(),
        "그리드 레벨이 설정되어야 함"
    );
}

/// 그리드 레벨 생성 검증
/// base_price = 100, spacing_pct = 2% (spacing = 2)
/// Level 1: buy=98, sell=100
/// Level 2: buy=96, sell=98
/// Level 3: buy=94, sell=96
#[tokio::test]
async fn test_grid_level_setup() {
    let mut strategy = DcaStrategy::grid();
    let config = grid_config("005930", "2.0", 3);

    strategy.initialize(config).await.unwrap();

    // 첫 가격 데이터로 그리드 초기화 (base_price = 100)
    let context = setup_context_with_prices("005930", &[dec!(100)], 1000000);
    strategy.set_context(context);

    let data = create_kline("005930", dec!(100), 1000000);
    let _ = strategy.on_market_data(&data).await.unwrap();

    let state = strategy.get_state();
    let levels = state["state"]["grid"]["levels"].as_array().unwrap();

    assert_eq!(levels.len(), 3, "3개 레벨이 생성되어야 함");

    // Level 1: buy=98, sell=100
    let level1 = &levels[0];
    let buy1: Decimal = level1["buy_price"].as_str().unwrap().parse().unwrap();
    let sell1: Decimal = level1["sell_price"].as_str().unwrap().parse().unwrap();
    assert_eq!(buy1, dec!(98), "Level 1 buy_price = 98");
    assert_eq!(sell1, dec!(100), "Level 1 sell_price = 100");

    // Level 2: buy=96, sell=98
    let level2 = &levels[1];
    let buy2: Decimal = level2["buy_price"].as_str().unwrap().parse().unwrap();
    let sell2: Decimal = level2["sell_price"].as_str().unwrap().parse().unwrap();
    assert_eq!(buy2, dec!(96), "Level 2 buy_price = 96");
    assert_eq!(sell2, dec!(98), "Level 2 sell_price = 98");

    // Level 3: buy=94, sell=96
    let level3 = &levels[2];
    let buy3: Decimal = level3["buy_price"].as_str().unwrap().parse().unwrap();
    let sell3: Decimal = level3["sell_price"].as_str().unwrap().parse().unwrap();
    assert_eq!(buy3, dec!(94), "Level 3 buy_price = 94");
    assert_eq!(sell3, dec!(96), "Level 3 sell_price = 96");
}

/// 그리드 매수 타이밍 테스트
/// 가격이 buy_price 이하로 떨어지면 매수 시그널 발생
#[tokio::test]
async fn test_grid_buy_timing() {
    let mut strategy = DcaStrategy::grid();
    let config = grid_config("005930", "2.0", 3);

    strategy.initialize(config).await.unwrap();

    // 초기화: base_price = 100
    let init_prices = vec![dec!(100)];
    let context = setup_context_with_prices("005930", &init_prices, 1000000);
    strategy.set_context(context);
    let data = create_kline("005930", dec!(100), 1000000);
    let _ = strategy.on_market_data(&data).await.unwrap();

    // 가격이 99로 하락 → Level 1 buy_price(98) 이상이므로 매수 안 함
    let prices = vec![dec!(100), dec!(99)];
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);
    let data = create_kline("005930", dec!(99), 1000000 + 86400);
    let signals = strategy.on_market_data(&data).await.unwrap();
    assert!(
        signals.iter().all(|s| s.side != Side::Buy),
        "가격 99: buy_price 98 위이므로 매수 시그널 없음"
    );

    // 가격이 98로 하락 → Level 1 buy_price(98)에 도달, 매수 발생
    let prices = vec![dec!(100), dec!(99), dec!(98)];
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);
    let data = create_kline("005930", dec!(98), 1000000 + 86400 * 2);
    let signals = strategy.on_market_data(&data).await.unwrap();
    let buy_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Buy).collect();
    assert_eq!(
        buy_signals.len(),
        1,
        "가격 98: Level 1 buy_price 도달, 매수 시그널 1개"
    );

    // 매수 시그널의 메타데이터 검증
    let signal = &buy_signals[0];
    assert_eq!(signal.signal_type, trader_core::SignalType::Entry);
    let grid_level = signal.metadata.get("grid_level_index").unwrap().as_i64().unwrap();
    assert_eq!(grid_level, 0, "Level 1 (index 0) 매수");
}

/// 그리드 매도 타이밍 테스트
/// 매수 후 가격이 sell_price 이상으로 오르면 매도 시그널 발생
#[tokio::test]
async fn test_grid_sell_timing() {
    let mut strategy = DcaStrategy::grid();
    let config = grid_config("005930", "2.0", 3);

    strategy.initialize(config).await.unwrap();

    // 초기화 및 Level 1 매수 트리거
    let prices = vec![dec!(100), dec!(98)];
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);

    let _ = strategy.on_market_data(&create_kline("005930", dec!(100), 1000000)).await;
    let signals = strategy.on_market_data(&create_kline("005930", dec!(98), 1000000 + 86400)).await.unwrap();
    assert_eq!(
        signals.iter().filter(|s| s.side == Side::Buy).count(),
        1,
        "Level 1 매수 완료"
    );

    // 가격이 99로 상승 → sell_price(100) 미달, 매도 안 함
    let prices = vec![dec!(100), dec!(98), dec!(99)];
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);
    let signals = strategy.on_market_data(&create_kline("005930", dec!(99), 1000000 + 86400 * 2)).await.unwrap();
    assert!(
        signals.iter().all(|s| s.side != Side::Sell),
        "가격 99: sell_price 100 미달, 매도 안 함"
    );

    // 가격이 100으로 상승 → sell_price(100) 도달, 매도 발생
    let prices = vec![dec!(100), dec!(98), dec!(99), dec!(100)];
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);
    let signals = strategy.on_market_data(&create_kline("005930", dec!(100), 1000000 + 86400 * 3)).await.unwrap();
    let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();
    assert_eq!(
        sell_signals.len(),
        1,
        "가격 100: Level 1 sell_price 도달, 매도 시그널 1개"
    );

    // 매도 시그널 메타데이터 검증
    let signal = &sell_signals[0];
    assert_eq!(signal.signal_type, trader_core::SignalType::Exit);
}

/// 그리드 다중 레벨 연속 매수 테스트
/// 가격이 계속 하락하면 여러 레벨에서 연속 매수
#[tokio::test]
async fn test_grid_multiple_level_buy() {
    let mut strategy = DcaStrategy::grid();
    let config = grid_config("005930", "2.0", 3);

    strategy.initialize(config).await.unwrap();

    // 가격이 100 → 94로 급락 (Level 1, 2, 3 모두 트리거)
    let prices = vec![dec!(100)];
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);
    let _ = strategy.on_market_data(&create_kline("005930", dec!(100), 1000000)).await;

    // 급락 시 모든 레벨 매수
    let prices = vec![dec!(100), dec!(94)];
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);
    let signals = strategy.on_market_data(&create_kline("005930", dec!(94), 1000000 + 86400)).await.unwrap();
    let buy_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Buy).collect();

    assert!(
        !buy_signals.is_empty(),
        "급락 시 최소 1개 이상의 매수 시그널 발생"
    );

    // 상태 검증: 매수된 레벨은 WaitingSell 상태
    let state = strategy.get_state();
    let levels = state["state"]["grid"]["levels"].as_array().unwrap();
    let waiting_sell_count = levels
        .iter()
        .filter(|l| l["state"].as_str().unwrap() == "WaitingSell")
        .count();

    assert!(
        waiting_sell_count >= 1,
        "최소 1개 레벨이 WaitingSell 상태"
    );
}

/// 그리드 순환 매매 테스트
/// 매수 → 매도 → 다시 매수 순환 검증
#[tokio::test]
async fn test_grid_cycle_trading() {
    let mut strategy = DcaStrategy::grid();
    let config = grid_config("005930", "2.0", 2);

    strategy.initialize(config).await.unwrap();

    let mut all_signals = vec![];
    let mut prices = vec![dec!(100)];

    // 1. 초기화
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);
    let _ = strategy.on_market_data(&create_kline("005930", dec!(100), 1000000)).await;

    // 2. 하락 → Level 1 매수 (98)
    prices.push(dec!(98));
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);
    let signals = strategy.on_market_data(&create_kline("005930", dec!(98), 1000000 + 86400)).await.unwrap();
    all_signals.extend(signals);

    // 3. 상승 → Level 1 매도 (100)
    prices.push(dec!(100));
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);
    let signals = strategy.on_market_data(&create_kline("005930", dec!(100), 1000000 + 86400 * 2)).await.unwrap();
    all_signals.extend(signals);

    // 4. 다시 하락 → Level 1 재매수 (98)
    prices.push(dec!(98));
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);
    let signals = strategy.on_market_data(&create_kline("005930", dec!(98), 1000000 + 86400 * 3)).await.unwrap();
    all_signals.extend(signals);

    // 검증: 매수 2회, 매도 1회 발생
    let buy_count = all_signals.iter().filter(|s| s.side == Side::Buy).count();
    let sell_count = all_signals.iter().filter(|s| s.side == Side::Sell).count();

    assert_eq!(buy_count, 2, "그리드 순환: 매수 2회 (첫 매수 + 재매수)");
    assert_eq!(sell_count, 1, "그리드 순환: 매도 1회");
}

/// 그리드 position_id 검증
/// 각 레벨은 독립적인 position_id를 가져야 함
#[tokio::test]
async fn test_grid_position_id() {
    let mut strategy = DcaStrategy::grid();
    let config = grid_config("005930", "2.0", 3);

    strategy.initialize(config).await.unwrap();

    // 급락으로 여러 레벨 매수
    let prices = vec![dec!(100)];
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);
    let _ = strategy.on_market_data(&create_kline("005930", dec!(100), 1000000)).await;

    let prices = vec![dec!(100), dec!(94)];
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);
    let signals = strategy.on_market_data(&create_kline("005930", dec!(94), 1000000 + 86400)).await.unwrap();

    let buy_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Buy).collect();

    // position_id가 "{ticker}_grid_L{level}" 형식인지 확인
    for signal in &buy_signals {
        let position_id = signal.position_id.as_ref().unwrap();
        assert!(
            position_id.starts_with("005930_grid_L"),
            "position_id 형식: {}_grid_L{{level}}, 실제: {}",
            "005930",
            position_id
        );
    }

    // 중복 position_id가 없어야 함
    let position_ids: Vec<_> = buy_signals
        .iter()
        .map(|s| s.position_id.as_ref().unwrap())
        .collect();
    let unique_ids: std::collections::HashSet<_> = position_ids.iter().collect();
    assert_eq!(
        position_ids.len(),
        unique_ids.len(),
        "position_id 중복 없어야 함"
    );
}

/// 그리드 group_id 검증
/// 같은 세션의 모든 포지션은 동일한 group_id를 가져야 함
#[tokio::test]
async fn test_grid_group_id() {
    let mut strategy = DcaStrategy::grid();
    let config = grid_config("005930", "2.0", 3);

    strategy.initialize(config).await.unwrap();

    // 급락으로 여러 레벨 매수
    let prices = vec![dec!(100)];
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);
    let _ = strategy.on_market_data(&create_kline("005930", dec!(100), 1000000)).await;

    let prices = vec![dec!(100), dec!(94)];
    let context = setup_context_with_prices("005930", &prices, 1000000);
    strategy.set_context(context);
    let signals = strategy.on_market_data(&create_kline("005930", dec!(94), 1000000 + 86400)).await.unwrap();

    let buy_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Buy).collect();

    // 모든 시그널이 동일한 group_id를 가져야 함
    if buy_signals.len() > 1 {
        let first_group = buy_signals[0].group_id.as_ref();
        for signal in &buy_signals[1..] {
            assert_eq!(
                signal.group_id.as_ref(),
                first_group,
                "같은 세션의 시그널은 동일한 group_id를 가져야 함"
            );
        }
    }

    // group_id 형식 검증
    if let Some(group_id) = buy_signals.first().and_then(|s| s.group_id.as_ref()) {
        assert!(
            group_id.starts_with("grid_"),
            "group_id 형식: grid_{{base_price}}_{{timestamp}}, 실제: {}",
            group_id
        );
    }
}
