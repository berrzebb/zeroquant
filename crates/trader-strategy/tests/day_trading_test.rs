//! DayTrading 전략 테스트
//!
//! 각 variant별 핵심 케이스를 모두 검증합니다:
//! - Breakout: 변동성 돌파, 손절/익절
//! - Crossover: 골든/데드 크로스
//! - VolumeSurge: 거래량 급증 + 연속 상승

use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::{
    Kline, MarketData, MarketDataType, Order, OrderStatusType, Position, Side, StrategyContext,
    Timeframe,
};
use trader_strategy::strategies::day_trading::{
    BreakoutConfig, CrossoverConfig, DayTradingConfig, DayTradingStrategy, DayTradingVariant,
    VolumeSurgeConfig,
};
use trader_strategy::Strategy;
use uuid::Uuid;

// ====== 헬퍼 함수 ======

fn create_kline(
    ticker: &str,
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    volume: Decimal,
    hour: u32,
    day: u32,
) -> MarketData {
    let timestamp = Utc.with_ymd_and_hms(2024, 1, day, hour, 0, 0).unwrap();
    MarketData {
        exchange: "test".to_string(),
        ticker: ticker.to_string(),
        timestamp,
        data: MarketDataType::Kline(Kline {
            ticker: ticker.to_string(),
            timeframe: Timeframe::H1,
            open_time: timestamp,
            close_time: timestamp,
            open,
            high,
            low,
            close,
            volume,
            quote_volume: Some(close * volume),
            num_trades: Some(100),
        }),
    }
}

fn create_simple_kline(ticker: &str, close: Decimal, hour: u32, day: u32) -> MarketData {
    create_kline(
        ticker,
        close,
        close + dec!(10),
        close - dec!(10),
        close,
        dec!(1000),
        hour,
        day,
    )
}

fn create_position(ticker: &str, side: Side, quantity: Decimal, entry_price: Decimal) -> Position {
    Position::new("test", ticker.to_string(), side, quantity, entry_price)
}

fn create_order(ticker: &str, side: Side, quantity: Decimal, price: Decimal) -> Order {
    Order {
        id: Uuid::new_v4(),
        exchange: "test".to_string(),
        exchange_order_id: None,
        ticker: ticker.to_string(),
        side,
        order_type: trader_core::OrderType::Limit,
        quantity,
        price: Some(price),
        stop_price: None,
        status: OrderStatusType::Filled,
        filled_quantity: quantity,
        average_fill_price: Some(price),
        time_in_force: trader_core::TimeInForce::GTC,
        strategy_id: None,
        client_order_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: serde_json::Value::Null,
    }
}

/// StrategyContext에 Breakout용 전일 캔들 데이터 설정
fn setup_context_for_breakout(
    ticker: &str,
    prev_high: Decimal,
    prev_low: Decimal,
    prev_close: Decimal,
) -> Arc<RwLock<StrategyContext>> {
    let mut context = StrategyContext::new();

    // 전일 캔들 데이터 (day 0)
    let prev_timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let prev_kline = Kline::new(
        ticker.to_string(),
        Timeframe::D1,
        prev_timestamp,
        prev_close - dec!(100), // open
        prev_high,
        prev_low,
        prev_close,
        dec!(10000),
        prev_timestamp,
    );

    // 당일 시가 캔들 (day 1)
    let today_timestamp = Utc.with_ymd_and_hms(2024, 1, 2, 9, 0, 0).unwrap();
    let today_kline = Kline::new(
        ticker.to_string(),
        Timeframe::D1,
        today_timestamp,
        prev_close + dec!(200), // open (= period_open)
        prev_close + dec!(300),
        prev_close + dec!(100),
        prev_close + dec!(200),
        dec!(10000),
        today_timestamp,
    );

    // klines[0] = today (가장 최근), klines[1] = prev (전일)
    context.update_klines(ticker, Timeframe::D1, vec![today_kline, prev_kline]);

    Arc::new(RwLock::new(context))
}

// ================================================================================================
// 초기화 테스트
// ================================================================================================

mod init_tests {
    use super::*;

    #[tokio::test]
    async fn test_breakout_initialization() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "Breakout",
            "ticker": "BTC/USDT"
        });

        let result = strategy.initialize(config).await;
        assert!(result.is_ok(), "Breakout 초기화 실패");

        let state = strategy.get_state();
        assert_eq!(state["initialized"], true);
        assert_eq!(state["variant"], "Breakout");
    }

    #[tokio::test]
    async fn test_crossover_initialization() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "Crossover",
            "ticker": "ETH/USDT",
            "short_period": 5,
            "long_period": 10
        });

        let result = strategy.initialize(config).await;
        assert!(result.is_ok(), "Crossover 초기화 실패");

        let state = strategy.get_state();
        assert_eq!(state["variant"], "Crossover");
    }

    #[tokio::test]
    async fn test_volume_surge_initialization() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "VolumeSurge",
            "ticker": "005930/KRW"
        });

        let result = strategy.initialize(config).await;
        assert!(result.is_ok(), "VolumeSurge 초기화 실패");

        let state = strategy.get_state();
        assert_eq!(state["variant"], "VolumeSurge");
    }
}

// ================================================================================================
// Breakout 전략 테스트 - 핵심 케이스 완전 검증
// ================================================================================================

mod breakout_tests {
    use super::*;

    /// 테스트 1: 레인지 없이 신호 없음 (첫 번째 캔들)
    #[tokio::test]
    async fn test_no_signal_without_range() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "Breakout",
            "ticker": "BTC/USDT",
            "breakout_config": { "k_factor": "0.5" }
        });
        strategy.initialize(config).await.unwrap();

        // 첫 번째 캔들 - 레인지 설정용
        let data = create_kline(
            "BTC/USDT",
            dec!(50000),
            dec!(50500),
            dec!(49500),
            dec!(50200),
            dec!(1000),
            0,
            1,
        );
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(
            signals.is_empty(),
            "첫 번째 캔들에서는 신호가 없어야 함 (레인지 설정용)"
        );
    }

    /// 테스트 2: 상방 돌파 시 매수 신호
    ///
    /// StrategyContext에서 D1 캔들 가져와서:
    /// - 전일 캔들: high=50500, low=49500 → range=1000
    /// - 당일 시가: 50200 → upper = 50200 + 1000*0.5 = 50700
    /// close >= 50700 → 롱 신호
    #[tokio::test]
    async fn test_upward_breakout_buy_signal() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "Breakout",
            "ticker": "BTC/USDT",
            "breakout_config": {
                "k_factor": "0.5",
                "min_range_pct": "0.1",
                "max_range_pct": "20.0"
            }
        });
        strategy.initialize(config).await.unwrap();

        // StrategyContext 설정: D1 캔들 (당일이 먼저, 전일이 뒤에)
        // get_range_from_context는 klines[0]=당일, klines[1]=전일로 가정
        let context = setup_context_for_breakout(
            "BTC/USDT",
            dec!(50500), // prev_high
            dec!(49500), // prev_low
            dec!(50000), // prev_close
        );
        strategy.set_context(context);

        // 돌파 시도: close=50800 >= upper=50700
        let data = create_kline(
            "BTC/USDT",
            dec!(50200),
            dec!(51000),
            dec!(50200),
            dec!(50800),
            dec!(1000),
            10,
            2, // 당일 (day=2)
        );
        let signals = strategy.on_market_data(&data).await.unwrap();

        // 검증: 매수 신호 발생
        assert!(!signals.is_empty(), "상방 돌파 시 매수 신호가 발생해야 함");
        assert_eq!(signals[0].side, Side::Buy, "상방 돌파는 매수 신호여야 함");

        // variant 메타데이터 확인
        let has_breakout = signals[0]
            .metadata
            .get("variant")
            .map(|v| v == "breakout")
            .unwrap_or(false);
        assert!(has_breakout, "신호에 breakout variant가 있어야 함");
    }

    /// 테스트 3: 하방 돌파 시 매도 신호 (trade_both_directions=true)
    ///
    /// StrategyContext에서 D1 캔들 가져와서:
    /// - 전일 캔들: high=50500, low=49500 → range=1000
    /// - 당일 시가: 50200 → lower = 50200 - 1000*0.5 = 49700
    /// close <= 49700 → 숏 신호
    #[tokio::test]
    async fn test_downward_breakout_sell_signal() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "Breakout",
            "ticker": "BTC/USDT",
            "breakout_config": {
                "k_factor": "0.5",
                "min_range_pct": "0.1",
                "max_range_pct": "20.0",
                "trade_both_directions": true
            }
        });
        strategy.initialize(config).await.unwrap();

        // StrategyContext 설정: D1 캔들
        // prev: high=50500, low=49500 → range=1000
        // today: open=50200 → lower = 50200 - 1000*0.5 = 49700
        let context = setup_context_for_breakout(
            "BTC/USDT",
            dec!(50500), // prev_high
            dec!(49500), // prev_low
            dec!(50000), // prev_close
        );
        strategy.set_context(context);

        // 하방 돌파: close=49600 <= lower=49700
        let data = create_kline(
            "BTC/USDT",
            dec!(50200),
            dec!(50200),
            dec!(49500),
            dec!(49600),
            dec!(1000),
            10,
            2, // 당일 (day=2)
        );
        let signals = strategy.on_market_data(&data).await.unwrap();

        // 검증: 매도(숏) 신호 발생
        assert!(!signals.is_empty(), "하방 돌파 시 매도 신호가 발생해야 함");
        assert_eq!(signals[0].side, Side::Sell, "하방 돌파는 매도 신호여야 함");
    }

    /// 테스트 4: 돌파 레벨 미달 시 신호 없음
    #[tokio::test]
    async fn test_no_signal_below_breakout_level() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "Breakout",
            "ticker": "BTC/USDT",
            "breakout_config": { "k_factor": "0.5" }
        });
        strategy.initialize(config).await.unwrap();

        // 레인지 설정
        let data1 = create_kline(
            "BTC/USDT",
            dec!(50000),
            dec!(50500),
            dec!(49500),
            dec!(50200),
            dec!(1000),
            0,
            1,
        );
        let _ = strategy.on_market_data(&data1).await.unwrap();

        // 새 기간 시작
        let data2 = create_kline(
            "BTC/USDT",
            dec!(50200),
            dec!(50200),
            dec!(50200),
            dec!(50200),
            dec!(1000),
            1,
            1,
        );
        let _ = strategy.on_market_data(&data2).await.unwrap();

        // 돌파 레벨 미달 (close=50500 < upper=50700)
        let data3 = create_kline(
            "BTC/USDT",
            dec!(50200),
            dec!(50600),
            dec!(50100),
            dec!(50500),
            dec!(1000),
            1,
            1,
        );
        let signals = strategy.on_market_data(&data3).await.unwrap();

        assert!(signals.is_empty(), "돌파 레벨 미달 시 신호가 없어야 함");
    }
}

// ================================================================================================
// Crossover 전략 테스트
// ================================================================================================

mod crossover_tests {
    use super::*;

    /// 테스트 1: 데이터 부족 시 신호 없음
    #[tokio::test]
    async fn test_no_signal_with_insufficient_data() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "Crossover",
            "ticker": "ETH/USDT",
            "short_period": 3,
            "long_period": 5
        });
        strategy.initialize(config).await.unwrap();

        // long_period(5) 미만의 데이터
        for i in 0..4 {
            let data =
                create_simple_kline("ETH/USDT", dec!(2000) + Decimal::from(i * 10), i as u32, 1);
            let signals = strategy.on_market_data(&data).await.unwrap();
            assert!(
                signals.is_empty(),
                "데이터 부족 시({}/5) 신호가 없어야 함",
                i + 1
            );
        }
    }

    /// 테스트 2: 골든 크로스 (단기 MA가 장기 MA를 상향 돌파)
    #[tokio::test]
    async fn test_golden_cross_buy_signal() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "Crossover",
            "ticker": "ETH/USDT",
            "short_period": 3,
            "long_period": 5
        });
        strategy.initialize(config).await.unwrap();

        let mut all_signals = vec![];

        // 하락 추세 (장기 MA > 단기 MA)
        let falling = [
            dec!(110),
            dec!(108),
            dec!(106),
            dec!(104),
            dec!(102),
            dec!(100),
        ];
        for (i, &price) in falling.iter().enumerate() {
            let data = create_simple_kline("ETH/USDT", price, i as u32, 1);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 급격한 상승 추세로 전환 (단기 MA가 장기 MA를 상향 돌파)
        let rising = [dec!(105), dec!(115), dec!(125), dec!(135), dec!(145)];
        for (i, &price) in rising.iter().enumerate() {
            let data = create_simple_kline("ETH/USDT", price, (6 + i) as u32, 1);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 검증: 골든 크로스에서 매수 신호 발생
        let buy_signals: Vec<_> = all_signals.iter().filter(|s| s.side == Side::Buy).collect();
        assert!(
            !buy_signals.is_empty(),
            "골든 크로스(단기 MA > 장기 MA) 시 매수 신호가 발생해야 함. 총 신호: {}",
            all_signals.len()
        );
    }

    /// 테스트 3: 데드 크로스 청산 (골든 크로스 포지션 보유 후 데드 크로스에서 청산)
    ///
    /// Crossover 전략은 롱 전용:
    /// - 골든 크로스: 매수 진입
    /// - 데드 크로스: 기존 포지션 청산 (숏 진입 아님)
    ///
    /// 시나리오:
    /// 1. 하락 추세 (장기 MA > 단기 MA)
    /// 2. 골든 크로스로 매수 포지션 진입
    /// 3. 상승 추세 유지
    /// 4. 데드 크로스로 청산 (매도)
    #[tokio::test]
    async fn test_death_cross_sell_signal() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "Crossover",
            "ticker": "ETH/USDT",
            "short_period": 3,
            "long_period": 5,
            "exit_config": {
                "exit_on_opposite_signal": true
            }
        });
        strategy.initialize(config).await.unwrap();

        let mut all_signals = vec![];
        let mut idx = 0u32;

        // Phase 1: 하락 추세 (장기 MA > 단기 MA 상태 생성)
        let falling = [
            dec!(120),
            dec!(115),
            dec!(110),
            dec!(105),
            dec!(100),
            dec!(95),
        ];
        for &price in falling.iter() {
            let data = create_simple_kline("ETH/USDT", price, idx, 1);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
            idx += 1;
        }

        // Phase 2: 급격한 상승 (골든 크로스 유발 → 매수 진입)
        // 단기(3) MA가 장기(5) MA를 상향 돌파
        let rising = [dec!(100), dec!(110), dec!(125), dec!(140), dec!(155)];
        for &price in rising.iter() {
            let data = create_simple_kline("ETH/USDT", price, idx, 1);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
            idx += 1;
        }

        // 중간 검증: 골든 크로스에서 매수 신호 발생 확인
        let buy_count = all_signals.iter().filter(|s| s.side == Side::Buy).count();
        assert!(
            buy_count > 0,
            "골든 크로스에서 매수 신호가 먼저 발생해야 함. 현재 신호 수: {}",
            all_signals.len()
        );

        // Phase 3: 급격한 하락 (데드 크로스 유발 → 포지션 청산)
        // 단기(3) MA가 장기(5) MA를 하향 돌파
        let falling2 = [dec!(145), dec!(125), dec!(100), dec!(75), dec!(50)];
        for &price in falling2.iter() {
            let data = create_simple_kline("ETH/USDT", price, idx, 1);
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
            idx += 1;
        }

        // 검증: 데드 크로스에서 매도(청산) 신호 발생
        let sell_count = all_signals.iter().filter(|s| s.side == Side::Sell).count();
        assert!(
            sell_count > 0,
            "데드 크로스에서 포지션 청산(매도) 신호가 발생해야 함. 총 신호: {}, 매수: {}, 매도: {}",
            all_signals.len(),
            buy_count,
            sell_count
        );
    }
}

// ================================================================================================
// VolumeSurge 전략 테스트
// ================================================================================================

mod volume_surge_tests {
    use super::*;

    /// 테스트 1: 데이터 부족 시 신호 없음
    #[tokio::test]
    async fn test_no_signal_with_insufficient_data() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "VolumeSurge",
            "ticker": "005930/KRW",
            "volume_multiplier": "2.0",
            "volume_period": 20,
            "consecutive_up_candles": 3
        });
        strategy.initialize(config).await.unwrap();

        // 데이터 부족
        for i in 0..5 {
            let data = create_kline(
                "005930/KRW",
                dec!(70000),
                dec!(70200),
                dec!(69800),
                dec!(70100),
                dec!(100000),
                i as u32,
                1,
            );
            let signals = strategy.on_market_data(&data).await.unwrap();
            assert!(
                signals.is_empty(),
                "volume_period(20) 미만의 데이터에서는 신호가 없어야 함"
            );
        }
    }

    /// 테스트 2: 거래량 급증 + 연속 상승봉 시 매수 신호
    #[tokio::test]
    async fn test_volume_surge_with_consecutive_up_buy_signal() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "VolumeSurge",
            "ticker": "005930/KRW",
            "volume_multiplier": "2.0",
            "volume_period": 5,
            "consecutive_up_candles": 3,
            "rsi_overbought": "90" // 높게 설정하여 필터 회피
        });
        strategy.initialize(config).await.unwrap();

        let mut all_signals = vec![];

        // 평균 거래량 설정 (100,000)
        for i in 0..6 {
            let data = create_kline(
                "005930/KRW",
                dec!(70000),
                dec!(70200),
                dec!(69800),
                dec!(70000),
                dec!(100000), // 평균 거래량
                i as u32,
                1,
            );
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 연속 상승봉 (3개) + 마지막에 거래량 급증
        let up_candles = [
            (dec!(70000), dec!(70100), dec!(100000)), // 상승 1
            (dec!(70100), dec!(70200), dec!(100000)), // 상승 2
            (dec!(70200), dec!(70300), dec!(100000)), // 상승 3
            (dec!(70300), dec!(70500), dec!(250000)), // 상승 4 + 거래량 급증 (2.5배)
        ];

        for (i, (open, close, vol)) in up_candles.iter().enumerate() {
            let data = create_kline(
                "005930/KRW",
                *open,
                *close + dec!(50),
                *open - dec!(50),
                *close,
                *vol,
                (6 + i) as u32,
                1,
            );
            let signals = strategy.on_market_data(&data).await.unwrap();
            all_signals.extend(signals);
        }

        // 검증: 거래량 급증 + 연속 상승봉에서 매수 신호 발생
        let buy_signals: Vec<_> = all_signals.iter().filter(|s| s.side == Side::Buy).collect();
        assert!(
            !buy_signals.is_empty(),
            "거래량 급증 + 연속 상승봉 시 매수 신호가 발생해야 함. 총 신호: {}",
            all_signals.len()
        );
    }
}

// ================================================================================================
// 공통 API 테스트
// ================================================================================================

mod common_tests {
    use super::*;

    #[tokio::test]
    async fn test_on_order_filled() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "Breakout",
            "ticker": "BTC/USDT"
        });
        strategy.initialize(config).await.unwrap();

        let order = create_order("BTC/USDT", Side::Buy, dec!(0.1), dec!(50000));
        let result = strategy.on_order_filled(&order).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_on_position_update() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "Crossover",
            "ticker": "ETH/USDT"
        });
        strategy.initialize(config).await.unwrap();

        let position = create_position("ETH/USDT", Side::Buy, dec!(1.0), dec!(2000));
        let result = strategy.on_position_update(&position).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_shutdown() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "VolumeSurge",
            "ticker": "005930/KRW"
        });
        strategy.initialize(config).await.unwrap();

        let result = strategy.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_state_has_required_fields() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "Breakout",
            "ticker": "BTC/USDT"
        });
        strategy.initialize(config).await.unwrap();

        let state = strategy.get_state();

        assert!(state.get("variant").is_some(), "variant 필드 필요");
        assert!(state.get("initialized").is_some(), "initialized 필드 필요");
        assert!(
            state.get("has_position").is_some(),
            "has_position 필드 필요"
        );
        assert!(
            state.get("trades_count").is_some(),
            "trades_count 필드 필요"
        );
    }

    #[tokio::test]
    async fn test_ignores_different_ticker() {
        let mut strategy = DayTradingStrategy::new();
        let config = json!({
            "variant": "Breakout",
            "ticker": "BTC/USDT"
        });
        strategy.initialize(config).await.unwrap();

        // 다른 티커
        let data = create_simple_kline("ETH/USDT", dec!(2000), 0, 1);
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(signals.is_empty(), "다른 티커 데이터는 무시해야 함");
    }

    #[tokio::test]
    async fn test_no_signal_without_initialization() {
        let mut strategy = DayTradingStrategy::new();

        let data = create_simple_kline("BTC/USDT", dec!(50000), 0, 1);
        let signals = strategy.on_market_data(&data).await.unwrap();

        assert!(signals.is_empty(), "초기화 전에는 신호가 없어야 함");
    }
}

// ================================================================================================
// 메타데이터 테스트
// ================================================================================================

mod metadata_tests {
    use super::*;

    #[test]
    fn test_strategy_name() {
        let strategy = DayTradingStrategy::new();
        assert_eq!(strategy.name(), "Day Trading");
    }

    #[test]
    fn test_strategy_version() {
        let strategy = DayTradingStrategy::new();
        assert_eq!(strategy.version(), "1.0.0");
    }

    #[test]
    fn test_strategy_description() {
        let strategy = DayTradingStrategy::new();
        assert!(!strategy.description().is_empty());
    }
}

// ================================================================================================
// Config 기본값 테스트
// ================================================================================================

mod config_tests {
    use super::*;

    #[test]
    fn test_breakout_config_default() {
        let config = BreakoutConfig::default();
        assert_eq!(config.k_factor, dec!(0.5));
        assert!(config.trade_both_directions);
    }

    #[test]
    fn test_crossover_config_default() {
        let config = CrossoverConfig::default();
        assert_eq!(config.short_period, 10);
        assert_eq!(config.long_period, 20);
    }

    #[test]
    fn test_volume_surge_config_default() {
        let config = VolumeSurgeConfig::default();
        assert_eq!(config.volume_multiplier, dec!(2.0));
        assert_eq!(config.consecutive_up_candles, 3);
    }

    #[test]
    fn test_day_trading_config_default() {
        let config = DayTradingConfig::default();
        assert_eq!(config.variant, DayTradingVariant::Breakout);
    }

    #[test]
    fn test_variant_serialization() {
        let breakout = DayTradingVariant::Breakout;
        let crossover = DayTradingVariant::Crossover;
        let volume_surge = DayTradingVariant::VolumeSurge;

        assert_eq!(serde_json::to_string(&breakout).unwrap(), "\"Breakout\"");
        assert_eq!(serde_json::to_string(&crossover).unwrap(), "\"Crossover\"");
        assert_eq!(
            serde_json::to_string(&volume_surge).unwrap(),
            "\"VolumeSurge\""
        );
    }
}
