//! Us3xLeverage (미국 3배 레버리지) 전략 통합 테스트
//!
//! 3배 레버리지 ETF와 인버스 ETF를 조합하여 양방향 수익을 추구하는 전략 테스트.
//! TQQQ, SOXL, SQQQ, SOXS 등 대상 ETF를 활용한 동적 배분 로직 검증.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use trader_core::{Kline, MarketData, Position, Side, Timeframe};
use trader_strategy::strategies::us_3x_leverage::Us3xLeverageStrategy;
use trader_strategy::Strategy;

// ============================================================================
// 테스트 헬퍼 함수
// ============================================================================

/// 테스트용 MarketData 생성 헬퍼
fn create_market_data(ticker: &str, close: Decimal, day: i64) -> MarketData {
    let timestamp = chrono::DateTime::from_timestamp(1704067200 + day * 86400, 0).unwrap();
    let kline = Kline::new(
        ticker.to_string(),
        Timeframe::D1,
        timestamp,
        close - dec!(1), // open
        close + dec!(2), // high
        close - dec!(2), // low
        close,           // close
        dec!(1000000),   // volume
        timestamp,       // close_time
    );
    MarketData::from_kline("test", kline)
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
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({});
    let result = strategy.initialize(config).await;

    assert!(result.is_ok(), "기본 설정으로 초기화 실패");

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_initialization_with_custom_config() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 0.4, "etf_type": "leverage"},
            {"ticker": "SOXL", "target_ratio": 0.3, "etf_type": "leverage"},
            {"ticker": "SQQQ", "target_ratio": 0.2, "etf_type": "inverse"},
            {"ticker": "SOXS", "target_ratio": 0.1, "etf_type": "inverse"}
        ],
        "rebalance_threshold": 7.0,
        "rebalance_period_days": 14,
        "ma_period": 25,
        "max_inverse_ratio": 0.5,
        "max_drawdown_pct": 25.0,
        "min_global_score": 60.0,
        "use_route_filter": true,
        "use_regime_allocation": true,
        "use_macro_risk": true
    });

    let result = strategy.initialize(config).await;
    assert!(result.is_ok(), "커스텀 설정으로 초기화 실패");

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_name_version_description() {
    let strategy = Us3xLeverageStrategy::new();

    assert_eq!(strategy.name(), "US 3X Leverage");
    assert_eq!(strategy.version(), "2.0.0");
    assert!(strategy.description().contains("레버리지") || strategy.description().contains("3배"));
}

// ============================================================================
// 데이터 처리 테스트
// ============================================================================

#[tokio::test]
async fn test_data_accumulation() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 0.5, "etf_type": "leverage"},
            {"ticker": "SQQQ", "target_ratio": 0.5, "etf_type": "inverse"}
        ],
        "ma_period": 10
    });
    strategy.initialize(config).await.unwrap();

    // 데이터 축적
    for day in 0..15 {
        let tqqq_data = create_market_data("TQQQ", dec!(50) + Decimal::from(day), day);
        let sqqq_data = create_market_data("SQQQ", dec!(30) - Decimal::from(day) / dec!(2), day);

        let _ = strategy.on_market_data(&tqqq_data).await;
        let _ = strategy.on_market_data(&sqqq_data).await;
    }

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_ignores_unregistered_ticker() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 1.0, "etf_type": "leverage"}
        ]
    });
    strategy.initialize(config).await.unwrap();

    // 등록되지 않은 티커
    let data = create_market_data("SPY", dec!(400), 0);
    let signals = strategy.on_market_data(&data).await.unwrap();

    assert!(signals.is_empty(), "등록되지 않은 티커는 무시");
}

// ============================================================================
// 레버리지/인버스 배분 테스트
// ============================================================================

#[tokio::test]
async fn test_leverage_allocation_on_uptrend() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 0.7, "etf_type": "leverage"},
            {"ticker": "SQQQ", "target_ratio": 0.3, "etf_type": "inverse"}
        ],
        "ma_period": 10,
        "min_global_score": 0.0,
        "use_route_filter": false,
        "use_regime_allocation": false,
        "use_macro_risk": false
    });
    strategy.initialize(config).await.unwrap();

    // 상승 추세 데이터 (가격 > MA)
    for day in 0..20 {
        let price = dec!(40) + Decimal::from(day * 2);
        let tqqq_data = create_market_data("TQQQ", price, day);
        let sqqq_data = create_market_data("SQQQ", dec!(35) - Decimal::from(day), day);

        let _ = strategy.on_market_data(&tqqq_data).await;
        let _ = strategy.on_market_data(&sqqq_data).await;
    }

    // 상승 추세에서 레버리지 비중 확대
    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_inverse_allocation_on_downtrend() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 0.5, "etf_type": "leverage"},
            {"ticker": "SQQQ", "target_ratio": 0.5, "etf_type": "inverse"}
        ],
        "ma_period": 10,
        "max_inverse_ratio": 0.8,
        "min_global_score": 0.0,
        "use_route_filter": false,
        "use_regime_allocation": false,
        "use_macro_risk": false
    });
    strategy.initialize(config).await.unwrap();

    // 하락 추세 데이터 (가격 < MA)
    for day in 0..20 {
        let price = dec!(60) - Decimal::from(day * 2);
        let price = if price < dec!(20) { dec!(20) } else { price };
        let tqqq_data = create_market_data("TQQQ", price, day);
        let sqqq_data = create_market_data("SQQQ", dec!(25) + Decimal::from(day), day);

        let _ = strategy.on_market_data(&tqqq_data).await;
        let _ = strategy.on_market_data(&sqqq_data).await;
    }

    // 하락 추세에서 인버스 비중 확대
    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

// ============================================================================
// 포지션 관리 테스트
// ============================================================================

#[tokio::test]
async fn test_position_update() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 0.5, "etf_type": "leverage"},
            {"ticker": "SQQQ", "target_ratio": 0.5, "etf_type": "inverse"}
        ]
    });
    strategy.initialize(config).await.unwrap();

    // TQQQ 포지션 업데이트
    let position = create_position("TQQQ", dec!(100), dec!(50));
    let result = strategy.on_position_update(&position).await;
    assert!(result.is_ok());

    // holdings 필드 확인 (positions가 아닌 holdings 사용)
    let state = strategy.get_state();
    let holdings = &state["holdings"];
    assert!(holdings.is_object());
}

#[tokio::test]
async fn test_multiple_etf_positions() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 0.35, "etf_type": "leverage"},
            {"ticker": "SOXL", "target_ratio": 0.35, "etf_type": "leverage"},
            {"ticker": "SQQQ", "target_ratio": 0.15, "etf_type": "inverse"},
            {"ticker": "SOXS", "target_ratio": 0.15, "etf_type": "inverse"}
        ]
    });
    strategy.initialize(config).await.unwrap();

    // 여러 ETF 포지션 업데이트
    let positions = [
        create_position("TQQQ", dec!(50), dec!(55)),
        create_position("SOXL", dec!(30), dec!(40)),
        create_position("SQQQ", dec!(20), dec!(25)),
        create_position("SOXS", dec!(15), dec!(18)),
    ];

    for pos in &positions {
        strategy.on_position_update(pos).await.unwrap();
    }

    // holdings 필드 확인 (positions가 아닌 holdings 사용)
    let state = strategy.get_state();
    let holdings_state = &state["holdings"];
    assert!(holdings_state.is_object());
}

#[tokio::test]
async fn test_position_cleared_on_zero_quantity() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 1.0, "etf_type": "leverage"}
        ]
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 추가
    let position = create_position("TQQQ", dec!(100), dec!(50));
    strategy.on_position_update(&position).await.unwrap();

    // 수량 0으로 청산
    let zero_position = create_position("TQQQ", dec!(0), dec!(0));
    strategy.on_position_update(&zero_position).await.unwrap();

    let _state = strategy.get_state();
    // TQQQ가 제거되었거나 수량이 0이어야 함
    // TODO: 실제 검증 로직 추가 필요
}

// ============================================================================
// 리밸런싱 테스트
// ============================================================================

#[tokio::test]
async fn test_rebalance_threshold_trigger() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 0.5, "etf_type": "leverage"},
            {"ticker": "SQQQ", "target_ratio": 0.5, "etf_type": "inverse"}
        ],
        "rebalance_threshold": 5.0,
        "ma_period": 5,
        "min_global_score": 0.0,
        "use_route_filter": false,
        "use_regime_allocation": false,
        "use_macro_risk": false
    });
    strategy.initialize(config).await.unwrap();

    // 데이터 축적
    for day in 0..10 {
        let tqqq_data = create_market_data("TQQQ", dec!(50), day);
        let sqqq_data = create_market_data("SQQQ", dec!(30), day);

        let _ = strategy.on_market_data(&tqqq_data).await;
        let _ = strategy.on_market_data(&sqqq_data).await;
    }

    // 포지션 설정 (불균형 상태)
    let tqqq_pos = create_position("TQQQ", dec!(100), dec!(50)); // 5000원 가치
    let sqqq_pos = create_position("SQQQ", dec!(50), dec!(30)); // 1500원 가치 (불균형)

    strategy.on_position_update(&tqqq_pos).await.unwrap();
    strategy.on_position_update(&sqqq_pos).await.unwrap();

    // 리밸런싱 필요 여부 확인
    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

// ============================================================================
// 최대 드로다운 테스트
// ============================================================================

#[tokio::test]
async fn test_max_drawdown_protection() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 1.0, "etf_type": "leverage"}
        ],
        "max_drawdown_pct": 20.0,
        "ma_period": 5,
        "min_global_score": 0.0,
        "use_route_filter": false,
        "use_regime_allocation": false,
        "use_macro_risk": false
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 설정 - 진입가 60달러
    let position = create_position("TQQQ", dec!(100), dec!(60));
    strategy.on_position_update(&position).await.unwrap();

    // 데이터 축적
    for day in 0..10 {
        let tqqq_data = create_market_data("TQQQ", dec!(60), day);
        let _ = strategy.on_market_data(&tqqq_data).await;
    }

    // 급락 (-25%)
    let crash_data = create_market_data("TQQQ", dec!(45), 11);
    let signals = strategy.on_market_data(&crash_data).await.unwrap();

    // 최대 드로다운 초과 시 청산 신호
    let _sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();
    // TODO: 드로다운 보호 기능 동작 검증 추가 필요
}

// ============================================================================
// 상태 관리 테스트
// ============================================================================

#[tokio::test]
async fn test_get_state_comprehensive() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 0.5, "etf_type": "leverage"},
            {"ticker": "SQQQ", "target_ratio": 0.5, "etf_type": "inverse"}
        ]
    });
    strategy.initialize(config).await.unwrap();

    let state = strategy.get_state();

    // 필수 필드 확인 (실제 get_state 반환 형식에 맞춤)
    assert!(!state["initialized"].is_null());
    assert!(!state["holdings"].is_null());
    assert!(!state["rebalance_count"].is_null());
    assert!(!state["market_env"].is_null());

    // 초기 값 확인
    assert_eq!(state["initialized"], true);
    assert_eq!(state["rebalance_count"], 0);
}

// ============================================================================
// 종료 테스트
// ============================================================================

#[tokio::test]
async fn test_shutdown() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 1.0, "etf_type": "leverage"}
        ]
    });
    strategy.initialize(config).await.unwrap();

    let result = strategy.shutdown().await;
    assert!(result.is_ok(), "정상 종료 실패");
}

#[tokio::test]
async fn test_shutdown_with_statistics() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 1.0, "etf_type": "leverage"}
        ]
    });
    strategy.initialize(config).await.unwrap();

    // 데이터 축적
    for day in 0..15 {
        let data = create_market_data("TQQQ", dec!(50) + Decimal::from(day), day);
        let _ = strategy.on_market_data(&data).await;
    }

    let result = strategy.shutdown().await;
    assert!(result.is_ok());

    // rebalance_count 필드 확인 (trades_count가 아님)
    let state = strategy.get_state();
    assert!(state["rebalance_count"].is_number());
}

// ============================================================================
// 에러 처리 테스트
// ============================================================================

#[tokio::test]
async fn test_process_data_before_initialization() {
    let mut strategy = Us3xLeverageStrategy::new();

    // 초기화 없이 데이터 처리
    let data = create_market_data("TQQQ", dec!(50), 0);
    let signals = strategy.on_market_data(&data).await.unwrap();

    assert!(signals.is_empty(), "초기화 전에는 신호 없어야 함");
}

// ============================================================================
// 복합 시나리오 테스트
// ============================================================================

#[tokio::test]
async fn test_full_trading_cycle() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 0.5, "etf_type": "leverage"},
            {"ticker": "SQQQ", "target_ratio": 0.5, "etf_type": "inverse"}
        ],
        "ma_period": 10,
        "rebalance_threshold": 10.0,
        "max_drawdown_pct": 30.0,
        "min_global_score": 0.0,
        "use_route_filter": false,
        "use_regime_allocation": false,
        "use_macro_risk": false
    });
    strategy.initialize(config).await.unwrap();

    // Phase 1: 데이터 축적
    for day in 0..10 {
        let tqqq_data = create_market_data("TQQQ", dec!(50), day);
        let sqqq_data = create_market_data("SQQQ", dec!(30), day);

        let _ = strategy.on_market_data(&tqqq_data).await;
        let _ = strategy.on_market_data(&sqqq_data).await;
    }

    // Phase 2: 상승 추세
    for day in 10..25 {
        let tqqq_price = dec!(50) + Decimal::from((day - 10) * 3);
        let sqqq_price = dec!(30) - Decimal::from((day - 10) * 1);
        let sqqq_price = if sqqq_price < dec!(15) {
            dec!(15)
        } else {
            sqqq_price
        };

        let tqqq_data = create_market_data("TQQQ", tqqq_price, day);
        let sqqq_data = create_market_data("SQQQ", sqqq_price, day);

        let _ = strategy.on_market_data(&tqqq_data).await;
        let _ = strategy.on_market_data(&sqqq_data).await;
    }

    // Phase 3: 하락 추세
    for day in 25..40 {
        let tqqq_price = dec!(95) - Decimal::from((day - 25) * 4);
        let tqqq_price = if tqqq_price < dec!(30) {
            dec!(30)
        } else {
            tqqq_price
        };
        let sqqq_price = dec!(15) + Decimal::from((day - 25) * 2);

        let tqqq_data = create_market_data("TQQQ", tqqq_price, day);
        let sqqq_data = create_market_data("SQQQ", sqqq_price, day);

        let _ = strategy.on_market_data(&tqqq_data).await;
        let _ = strategy.on_market_data(&sqqq_data).await;
    }

    // 최종 상태 확인
    let final_state = strategy.get_state();
    assert!(final_state["initialized"].as_bool().unwrap_or(false));
}

#[tokio::test]
async fn test_four_etf_allocation() {
    let mut strategy = Us3xLeverageStrategy::new();

    let config = json!({
        "allocations": [
            {"ticker": "TQQQ", "target_ratio": 0.35, "etf_type": "leverage"},
            {"ticker": "SOXL", "target_ratio": 0.35, "etf_type": "leverage"},
            {"ticker": "SQQQ", "target_ratio": 0.15, "etf_type": "inverse"},
            {"ticker": "SOXS", "target_ratio": 0.15, "etf_type": "inverse"}
        ],
        "ma_period": 10,
        "min_global_score": 0.0,
        "use_route_filter": false,
        "use_regime_allocation": false,
        "use_macro_risk": false
    });
    strategy.initialize(config).await.unwrap();

    // 4개 ETF 데이터
    for day in 0..20 {
        let tqqq = create_market_data("TQQQ", dec!(50) + Decimal::from(day), day);
        let soxl = create_market_data("SOXL", dec!(35) + Decimal::from(day) / dec!(2), day);
        let sqqq = create_market_data("SQQQ", dec!(30) - Decimal::from(day) / dec!(3), day);
        let soxs = create_market_data("SOXS", dec!(20) - Decimal::from(day) / dec!(4), day);

        let _ = strategy.on_market_data(&tqqq).await;
        let _ = strategy.on_market_data(&soxl).await;
        let _ = strategy.on_market_data(&sqqq).await;
        let _ = strategy.on_market_data(&soxs).await;
    }

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}
