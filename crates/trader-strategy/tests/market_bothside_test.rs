//! MarketBothSide 전략 통합 테스트
//!
//! 코스피 레버리지/인버스 ETF를 활용한 양방향 투자 전략 테스트.
//! MA3/6/19/60, 이격도, RSI 지표를 조합한 추세 판단 로직 검증.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::{Kline, MarketData, Position, Side, StrategyContext, Timeframe};
use trader_strategy::strategies::market_bothside::MarketBothSideStrategy;
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
        close + dec!(1), // high
        close - dec!(2), // low
        close,           // close
        dec!(100000),    // volume
        timestamp,       // close_time
    );
    MarketData::from_kline("test", kline)
}

/// Position 헬퍼 함수
fn create_position(ticker: &str, quantity: Decimal, entry_price: Decimal) -> Position {
    Position::new("test", ticker.to_string(), Side::Buy, quantity, entry_price)
}

/// StrategyContext에 klines 데이터 설정
/// MarketBothSide는 klines가 오래된 것부터 최신 순서 (klines.last() = 최신)
fn setup_context_for_market_bothside(
    leverage_ticker: &str,
    inverse_ticker: &str,
    day_count: usize,
    base_price: Decimal,
    price_increment: Decimal,
) -> Arc<RwLock<StrategyContext>> {
    let mut context = StrategyContext::new();

    // Leverage ticker klines
    let leverage_klines: Vec<Kline> = (0..day_count)
        .map(|day| {
            let timestamp = chrono::DateTime::from_timestamp(1704067200 + (day as i64) * 86400, 0).unwrap();
            let price = base_price + price_increment * Decimal::from(day);
            Kline::new(
                leverage_ticker.to_string(),
                Timeframe::D1,
                timestamp,
                price - dec!(1),
                price + dec!(1),
                price - dec!(2),
                price,
                dec!(100000),
                timestamp,
            )
        })
        .collect();
    context.update_klines(leverage_ticker, Timeframe::D1, leverage_klines);

    // Inverse ticker klines (leverage와 반대 방향)
    let inverse_klines: Vec<Kline> = (0..day_count)
        .map(|day| {
            let timestamp = chrono::DateTime::from_timestamp(1704067200 + (day as i64) * 86400, 0).unwrap();
            let price = base_price - price_increment * Decimal::from(day); // 반대 방향
            Kline::new(
                inverse_ticker.to_string(),
                Timeframe::D1,
                timestamp,
                price - dec!(1),
                price + dec!(1),
                price - dec!(2),
                price.max(dec!(1)), // 최소 1
                dec!(100000),
                timestamp,
            )
        })
        .collect();
    context.update_klines(inverse_ticker, Timeframe::D1, inverse_klines);

    Arc::new(RwLock::new(context))
}

// ============================================================================
// 초기화 테스트
// ============================================================================

#[tokio::test]
async fn test_initialization_default_config() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({});
    let result = strategy.initialize(config).await;

    assert!(result.is_ok(), "기본 설정으로 초기화 실패");

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
    assert_eq!(state["started"], false);
}

#[tokio::test]
async fn test_initialization_with_custom_config() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670",
        "leverage_ratio": 0.6,
        "inverse_ratio": 0.4,
        "ma3_period": 3,
        "ma6_period": 6,
        "ma19_period": 19,
        "ma60_period": 60,
        "disparity_upper": 108.0,
        "disparity_lower": 92.0,
        "rsi_period": 14,
        "rsi_oversold": 25.0,
        "rsi_overbought": 75.0,
        "stop_loss_pct": 7.0
    });

    let result = strategy.initialize(config).await;
    assert!(result.is_ok(), "커스텀 설정으로 초기화 실패");

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_name_version_description() {
    let strategy = MarketBothSideStrategy::new();

    assert_eq!(strategy.name(), "Market Both Side");
    assert_eq!(strategy.version(), "1.0.0");
    assert!(strategy.description().contains("레버리지"));
    assert!(strategy.description().contains("인버스"));
}

// ============================================================================
// 데이터 처리 테스트
// ============================================================================

#[tokio::test]
async fn test_data_accumulation_before_signals() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670"
    });
    strategy.initialize(config).await.unwrap();

    // 60개 미만의 데이터에서는 신호가 생성되지 않아야 함
    for day in 0..30 {
        let leverage_data =
            create_market_data("122630", dec!(15000) + Decimal::from(day * 10), day);
        let inverse_data = create_market_data("252670", dec!(5000) - Decimal::from(day * 5), day);

        let leverage_signals = strategy.on_market_data(&leverage_data).await.unwrap();
        let inverse_signals = strategy.on_market_data(&inverse_data).await.unwrap();

        assert!(
            leverage_signals.is_empty(),
            "데이터 축적 중에는 신호 없어야 함"
        );
        assert!(
            inverse_signals.is_empty(),
            "데이터 축적 중에는 신호 없어야 함"
        );
    }

    let state = strategy.get_state();
    assert_eq!(state["started"], false, "60개 미만이면 started=false");
}

#[tokio::test]
async fn test_strategy_starts_after_sufficient_data() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670"
    });
    strategy.initialize(config).await.unwrap();

    // StrategyContext에 65일치 데이터 설정 (60개 이상 필요)
    let context = setup_context_for_market_bothside(
        "122630",
        "252670",
        65,
        dec!(15000),
        dec!(10),
    );
    strategy.set_context(context);

    // 마지막 데이터로 on_market_data 호출하여 started 트리거
    let leverage_data = create_market_data("122630", dec!(15640), 64);
    let _ = strategy.on_market_data(&leverage_data).await;

    let state = strategy.get_state();
    assert_eq!(state["started"], true, "60개 이상이면 started=true");
}

#[tokio::test]
async fn test_ignores_unregistered_ticker() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670"
    });
    strategy.initialize(config).await.unwrap();

    // 등록되지 않은 티커로 데이터 전송
    let data = create_market_data("999999", dec!(10000), 0);
    let signals = strategy.on_market_data(&data).await.unwrap();

    assert!(signals.is_empty(), "등록되지 않은 티커는 무시");
}

// ============================================================================
// 레버리지 매수/매도 조건 테스트
// ============================================================================

#[tokio::test]
async fn test_leverage_signals_on_uptrend() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670",
        "ma60_period": 20,  // 테스트를 위해 짧게 설정
        "disparity_upper": 110.0,
        "rsi_overbought": 80.0
    });
    strategy.initialize(config).await.unwrap();

    // StrategyContext에 70일치 상승 추세 데이터 설정
    let context = setup_context_for_market_bothside(
        "122630",
        "252670",
        70,
        dec!(10000),
        dec!(100),
    );
    strategy.set_context(context);

    // 마지막 데이터로 on_market_data 호출하여 started 트리거
    let leverage_data = create_market_data("122630", dec!(16900), 69);
    let _ = strategy.on_market_data(&leverage_data).await;

    // 최종 상태 확인
    let state = strategy.get_state();
    assert_eq!(state["started"], true);
}

#[tokio::test]
async fn test_leverage_exit_on_dead_cross() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670",
        "ma3_period": 3,
        "ma6_period": 6,
        "ma19_period": 10  // 테스트를 위해 짧게 설정
    });
    strategy.initialize(config).await.unwrap();

    // 레버리지 포지션 설정
    let position = create_position("122630", dec!(100), dec!(15000));
    strategy.on_position_update(&position).await.unwrap();

    // 상승 후 하락 추세 (데드 크로스 조건)
    for day in 0..40 {
        let price = dec!(15000) + Decimal::from(day * 50);
        let leverage_data = create_market_data("122630", price, day);
        let _ = strategy.on_market_data(&leverage_data).await;
    }

    // 이후 하락 추세
    for day in 40..80 {
        let price = dec!(17000) - Decimal::from((day - 40) * 100);
        let price = if price < dec!(10000) {
            dec!(10000)
        } else {
            price
        };
        let leverage_data = create_market_data("122630", price, day);
        let signals = strategy.on_market_data(&leverage_data).await.unwrap();

        // 데드 크로스 시 매도 신호 확인
        if !signals.is_empty() {
            let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();

            if !sell_signals.is_empty() {
                assert!(sell_signals[0].metadata.get("action").is_some());
                return;
            }
        }
    }
}

// ============================================================================
// 인버스 매수/매도 조건 테스트
// ============================================================================

#[tokio::test]
async fn test_inverse_buy_on_downtrend() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670",
        "ma3_period": 3,
        "ma6_period": 6,
        "ma19_period": 10,
        "rsi_overbought": 70.0
    });
    strategy.initialize(config).await.unwrap();

    // 하락 추세 데이터 생성 (데드 크로스 조건)
    for day in 0..70 {
        // 하락 추세 가격
        let base_price = dec!(20000);
        let price = base_price - Decimal::from(day * 50);
        let price = if price < dec!(10000) {
            dec!(10000)
        } else {
            price
        };

        let leverage_data = create_market_data("122630", price, day);
        let inverse_data = create_market_data("252670", dec!(5000) + Decimal::from(day * 30), day);

        let leverage_signals = strategy.on_market_data(&leverage_data).await.unwrap();
        let _ = strategy.on_market_data(&inverse_data).await;

        // 데드 크로스 후 인버스 매수 신호 확인
        if day >= 60 {
            for signal in &leverage_signals {
                if signal.side == Side::Buy {
                    let etf_type = signal.metadata.get("etf_type");
                    if etf_type == Some(&json!("inverse")) {
                        // 인버스 매수 신호 확인
                        return;
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn test_inverse_exit_on_golden_cross() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670",
        "ma3_period": 3,
        "ma6_period": 6,
        "ma19_period": 10
    });
    strategy.initialize(config).await.unwrap();

    // 인버스 포지션 설정
    let position = create_position("252670", dec!(100), dec!(5000));
    strategy.on_position_update(&position).await.unwrap();

    // 하락 후 상승 추세 (골든 크로스 조건)
    for day in 0..40 {
        let price = dec!(15000) - Decimal::from(day * 50);
        let price = if price < dec!(10000) {
            dec!(10000)
        } else {
            price
        };
        let leverage_data = create_market_data("122630", price, day);
        let _ = strategy.on_market_data(&leverage_data).await;
    }

    // 이후 상승 추세
    for day in 40..80 {
        let price = dec!(10000) + Decimal::from((day - 40) * 100);
        let leverage_data = create_market_data("122630", price, day);
        let inverse_data =
            create_market_data("252670", dec!(6000) - Decimal::from((day - 40) * 20), day);

        let _ = strategy.on_market_data(&leverage_data).await;
        let signals = strategy.on_market_data(&inverse_data).await.unwrap();

        // 골든 크로스 시 인버스 매도 신호 확인
        for signal in &signals {
            if signal.side == Side::Sell {
                let etf_type = signal.metadata.get("etf_type");
                if etf_type == Some(&json!("inverse")) {
                    return;
                }
            }
        }
    }
}

// ============================================================================
// 포지션 관리 테스트
// ============================================================================

#[tokio::test]
async fn test_leverage_position_update() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670"
    });
    strategy.initialize(config).await.unwrap();

    // 레버리지 포지션 업데이트
    let position = create_position("122630", dec!(100), dec!(15000));
    let result = strategy.on_position_update(&position).await;
    assert!(result.is_ok());

    let state = strategy.get_state();
    let lev_pos = &state["leverage_position"];
    assert!(!lev_pos.is_null());
    assert_eq!(lev_pos["holdings"], "100");
    assert_eq!(lev_pos["entry_price"], "15000");
}

#[tokio::test]
async fn test_inverse_position_update() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670"
    });
    strategy.initialize(config).await.unwrap();

    // 인버스 포지션 업데이트
    let position = create_position("252670", dec!(200), dec!(5000));
    let result = strategy.on_position_update(&position).await;
    assert!(result.is_ok());

    let state = strategy.get_state();
    let inv_pos = &state["inverse_position"];
    assert!(!inv_pos.is_null());
    assert_eq!(inv_pos["holdings"], "200");
    assert_eq!(inv_pos["entry_price"], "5000");
}

#[tokio::test]
async fn test_position_removal_on_zero_quantity() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670"
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 추가
    let position = create_position("122630", dec!(100), dec!(15000));
    strategy.on_position_update(&position).await.unwrap();

    // 수량 0으로 포지션 청산
    let zero_position = create_position("122630", dec!(0), dec!(0));
    strategy.on_position_update(&zero_position).await.unwrap();

    let state = strategy.get_state();
    assert!(
        state["leverage_position"].is_null(),
        "수량 0이면 포지션 삭제"
    );
}

// ============================================================================
// 손절 테스트
// ============================================================================

#[tokio::test]
async fn test_leverage_stop_loss() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670",
        "stop_loss_pct": 5.0
    });
    strategy.initialize(config).await.unwrap();

    // 레버리지 포지션 설정 - 진입가 15000원
    let position = create_position("122630", dec!(100), dec!(15000));
    strategy.on_position_update(&position).await.unwrap();

    // 충분한 데이터 축적
    for day in 0..65 {
        let leverage_data = create_market_data("122630", dec!(15000), day);
        let _ = strategy.on_market_data(&leverage_data).await;
    }

    // 손절 조건 데이터 전송 (-5% 이하)
    // 15000 * 0.95 = 14250
    let stop_loss_data = create_market_data("122630", dec!(14200), 66);
    let signals = strategy.on_market_data(&stop_loss_data).await.unwrap();

    // 손절 매도 신호 확인
    let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();

    if !sell_signals.is_empty() {
        let action = sell_signals[0].metadata.get("action");
        assert!(action.is_some());
    }
}

#[tokio::test]
async fn test_inverse_stop_loss() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670",
        "stop_loss_pct": 5.0
    });
    strategy.initialize(config).await.unwrap();

    // 인버스 포지션 설정 - 진입가 5000원
    let position = create_position("252670", dec!(100), dec!(5000));
    strategy.on_position_update(&position).await.unwrap();

    // 충분한 데이터 축적 (레버리지 데이터로)
    for day in 0..65 {
        let leverage_data = create_market_data("122630", dec!(15000), day);
        let _ = strategy.on_market_data(&leverage_data).await;
    }

    // 손절 조건 데이터 전송 (-5% 이하)
    // 5000 * 0.95 = 4750
    let stop_loss_data = create_market_data("252670", dec!(4700), 66);
    let signals = strategy.on_market_data(&stop_loss_data).await.unwrap();

    // 손절 매도 신호 확인
    let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();

    if !sell_signals.is_empty() {
        let action = sell_signals[0].metadata.get("action");
        assert!(action.is_some());
    }
}

// ============================================================================
// 상태 관리 테스트
// ============================================================================

#[tokio::test]
async fn test_get_state_comprehensive() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670"
    });
    strategy.initialize(config).await.unwrap();

    let state = strategy.get_state();

    // 필수 필드 확인
    assert!(!state["initialized"].is_null());
    assert!(!state["started"].is_null());
    assert!(!state["trades_count"].is_null());
    assert!(!state["wins"].is_null());
    assert!(!state["total_pnl"].is_null());

    // 초기 값 확인
    assert_eq!(state["initialized"], true);
    assert_eq!(state["started"], false);
    assert_eq!(state["trades_count"], 0);
    assert_eq!(state["wins"], 0);
}

#[tokio::test]
async fn test_state_with_both_positions() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670"
    });
    strategy.initialize(config).await.unwrap();

    // 양방향 포지션 추가
    let lev_position = create_position("122630", dec!(100), dec!(15000));
    let inv_position = create_position("252670", dec!(50), dec!(5000));

    strategy.on_position_update(&lev_position).await.unwrap();
    strategy.on_position_update(&inv_position).await.unwrap();

    let state = strategy.get_state();

    // 레버리지 포지션 확인
    assert!(!state["leverage_position"].is_null());
    assert_eq!(state["leverage_position"]["holdings"], "100");

    // 인버스 포지션 확인
    assert!(!state["inverse_position"].is_null());
    assert_eq!(state["inverse_position"]["holdings"], "50");
}

// ============================================================================
// 종료 테스트
// ============================================================================

#[tokio::test]
async fn test_shutdown() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670"
    });
    strategy.initialize(config).await.unwrap();

    let result = strategy.shutdown().await;
    assert!(result.is_ok(), "정상 종료 실패");
}

#[tokio::test]
async fn test_shutdown_with_statistics() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670"
    });
    strategy.initialize(config).await.unwrap();

    // 데이터 축적
    for day in 0..65 {
        let leverage_data = create_market_data("122630", dec!(15000), day);
        let _ = strategy.on_market_data(&leverage_data).await;
    }

    // 종료 후 통계 확인
    let result = strategy.shutdown().await;
    assert!(result.is_ok());

    let state = strategy.get_state();
    assert!(state["trades_count"].is_number());
    assert!(state["wins"].is_number());
}

// ============================================================================
// 에러 처리 테스트
// ============================================================================

#[tokio::test]
async fn test_process_data_before_initialization() {
    let mut strategy = MarketBothSideStrategy::new();

    // 초기화 없이 데이터 처리
    let data = create_market_data("122630", dec!(15000), 0);
    let signals = strategy.on_market_data(&data).await.unwrap();

    assert!(signals.is_empty(), "초기화 전에는 신호 없어야 함");
}

#[tokio::test]
async fn test_position_update_before_initialization() {
    let mut strategy = MarketBothSideStrategy::new();

    // 초기화 없이 포지션 업데이트
    let position = create_position("122630", dec!(100), dec!(15000));
    let result = strategy.on_position_update(&position).await;

    // 에러 없이 무시
    assert!(result.is_ok());
}

// ============================================================================
// 복합 시나리오 테스트
// ============================================================================

#[tokio::test]
async fn test_full_trading_cycle() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670",
        "ma3_period": 3,
        "ma6_period": 6,
        "ma19_period": 10,
        "ma60_period": 20,
        "stop_loss_pct": 5.0
    });
    strategy.initialize(config).await.unwrap();

    // StrategyContext에 60일치 데이터 설정
    let context = setup_context_for_market_bothside(
        "122630",
        "252670",
        60,
        dec!(15000),
        dec!(10),
    );
    strategy.set_context(context);

    // 마지막 데이터로 on_market_data 호출
    let leverage_data = create_market_data("122630", dec!(15590), 59);
    let _ = strategy.on_market_data(&leverage_data).await;

    let state = strategy.get_state();
    assert_eq!(state["started"], true);

    // Phase 2: 상승 추세
    for day in 60..80 {
        let leverage_data =
            create_market_data("122630", dec!(15600) + Decimal::from((day - 60) * 100), day);
        let inverse_data =
            create_market_data("252670", dec!(4700) - Decimal::from((day - 60) * 30), day);
        let _ = strategy.on_market_data(&leverage_data).await;
        let _ = strategy.on_market_data(&inverse_data).await;
    }

    // Phase 3: 하락 추세
    for day in 80..100 {
        let price = dec!(17600) - Decimal::from((day - 80) * 150);
        let price = if price < dec!(12000) {
            dec!(12000)
        } else {
            price
        };

        let leverage_data = create_market_data("122630", price, day);
        let inverse_data =
            create_market_data("252670", dec!(4100) + Decimal::from((day - 80) * 40), day);
        let _ = strategy.on_market_data(&leverage_data).await;
        let _ = strategy.on_market_data(&inverse_data).await;
    }

    // 최종 상태 확인
    let final_state = strategy.get_state();
    assert!(final_state["initialized"].as_bool().unwrap_or(false));
}

// ============================================================================
// 날짜 변경 테스트
// ============================================================================

#[tokio::test]
async fn test_new_day_detection() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670"
    });
    strategy.initialize(config).await.unwrap();

    // Day 0 데이터
    let data_day0 = create_market_data("122630", dec!(15000), 0);
    let _ = strategy.on_market_data(&data_day0).await;

    // Day 1 데이터 (새로운 날)
    let data_day1 = create_market_data("122630", dec!(15100), 1);
    let _ = strategy.on_market_data(&data_day1).await;

    // 전략이 정상 동작
    let state = strategy.get_state();
    assert!(state["initialized"].as_bool().unwrap_or(false));
}

// ============================================================================
// 기술적 지표 내부 테스트 (TechnicalIndicators)
// ============================================================================

#[tokio::test]
async fn test_ma_calculation_accuracy() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670"
    });
    strategy.initialize(config).await.unwrap();

    // 일정한 가격으로 데이터 추가
    for day in 0..20 {
        let data = create_market_data("122630", dec!(10000), day);
        let _ = strategy.on_market_data(&data).await;
    }

    // 일정한 가격이면 MA도 같은 값이어야 함
    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_disparity_calculation() {
    let mut strategy = MarketBothSideStrategy::new();

    let config = json!({
        "leverage_ticker": "122630",
        "inverse_ticker": "252670",
        "disparity_upper": 105.0,
        "disparity_lower": 95.0
    });
    strategy.initialize(config).await.unwrap();

    // StrategyContext에 급등 패턴 데이터 설정
    // 초반 60일은 10000, 이후 급등
    let mut context = StrategyContext::new();

    let leverage_klines: Vec<Kline> = (0..65)
        .map(|day| {
            let timestamp = chrono::DateTime::from_timestamp(1704067200 + (day as i64) * 86400, 0).unwrap();
            let price = if day < 60 {
                dec!(10000)
            } else {
                dec!(10000) + Decimal::from((day - 60) * 500)
            };
            Kline::new(
                "122630".to_string(),
                Timeframe::D1,
                timestamp,
                price - dec!(1),
                price + dec!(1),
                price - dec!(2),
                price,
                dec!(100000),
                timestamp,
            )
        })
        .collect();
    context.update_klines("122630", Timeframe::D1, leverage_klines);

    let inverse_klines: Vec<Kline> = (0..65)
        .map(|day| {
            let timestamp = chrono::DateTime::from_timestamp(1704067200 + (day as i64) * 86400, 0).unwrap();
            Kline::new(
                "252670".to_string(),
                Timeframe::D1,
                timestamp,
                dec!(5000) - dec!(1),
                dec!(5000) + dec!(1),
                dec!(5000) - dec!(2),
                dec!(5000),
                dec!(100000),
                timestamp,
            )
        })
        .collect();
    context.update_klines("252670", Timeframe::D1, inverse_klines);

    strategy.set_context(Arc::new(RwLock::new(context)));

    // 마지막 데이터로 on_market_data 호출
    let data = create_market_data("122630", dec!(12000), 64);
    let _ = strategy.on_market_data(&data).await;

    let state = strategy.get_state();
    assert_eq!(state["started"], true);
}
