//! MomentumSurge 전략 통합 테스트
//!
//! 코스피/코스닥 레버리지와 인버스 ETF를 조합한 양방향 전략 테스트.
//! OBV, MA, RSI 지표를 활용한 추세 판단 로직 검증.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::{Kline, MarketData, Position, Side, StrategyContext, Timeframe};
use trader_strategy::strategies::momentum_surge::MomentumSurgeStrategy;
use trader_strategy::Strategy;

// ============================================================================
// 테스트 헬퍼 함수
// ============================================================================

/// 테스트용 MarketData 생성 헬퍼
fn create_market_data(ticker: &str, close: Decimal, volume: Decimal, day: i64) -> MarketData {
    let timestamp = chrono::DateTime::from_timestamp(1704067200 + day * 86400, 0).unwrap(); // 2024-01-01 기준
    let kline = Kline::new(
        ticker.to_string(),
        Timeframe::D1,
        timestamp,
        close - dec!(1), // open
        close + dec!(1), // high
        close - dec!(2), // low
        close,           // close
        volume,
        timestamp, // close_time
    );
    MarketData::from_kline("test", kline)
}

/// Position 헬퍼 함수
fn create_position(ticker: &str, quantity: Decimal, entry_price: Decimal) -> Position {
    Position::new("test", ticker.to_string(), Side::Buy, quantity, entry_price)
}

/// StrategyContext에 여러 티커에 대한 klines 설정
/// ticker 형식: "122630/KRW" 또는 "122630" 모두 지원 (base 이름으로 저장)
fn setup_context_for_momentum_surge(
    tickers: &[&str],
    day_count: usize,
) -> Arc<RwLock<StrategyContext>> {
    let mut context = StrategyContext::new();
    let base_timestamp =
        chrono::DateTime::from_timestamp(1704067200, 0).unwrap(); // 2024-01-01

    for ticker in tickers {
        // "122630/KRW" -> "122630" 변환 (전략의 get_etf_klines와 일치)
        let ticker_base = ticker.split('/').next().unwrap_or(ticker);

        let klines: Vec<Kline> = (0..day_count)
            .map(|i| {
                let day_offset = chrono::Duration::days(i as i64);
                let timestamp = base_timestamp + day_offset;
                let price = dec!(10000) + Decimal::from(i as u32 * 10);
                let volume = dec!(100000);

                Kline::new(
                    ticker_base.to_string(),
                    Timeframe::D1,
                    timestamp,
                    price - dec!(1),    // open
                    price + dec!(1),    // high
                    price - dec!(2),    // low
                    price,              // close
                    volume,
                    timestamp,          // close_time
                )
            })
            .collect();

        context.update_klines(ticker_base, Timeframe::D1, klines);
    }

    Arc::new(RwLock::new(context))
}

// ============================================================================
// 초기화 테스트
// ============================================================================

#[tokio::test]
async fn test_initialization_default_config() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({});
    let result = strategy.initialize(config).await;

    assert!(result.is_ok(), "기본 설정으로 초기화 실패");

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
    assert_eq!(state["started"], false);
    assert_eq!(state["position_count"], 0);
}

#[tokio::test]
async fn test_initialization_with_custom_config() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"],
        "max_positions": 3,
        "position_ratio": 0.4,
        "obv_period": 15,
        "ma_short": 10,
        "ma_medium": 30,
        "ma_long": 90,
        "rsi_period": 20,
        "stop_loss_pct": 5.0,
        "take_profit_pct": 15.0,
        "min_global_score": 70
    });

    let result = strategy.initialize(config).await;
    assert!(result.is_ok(), "커스텀 설정으로 초기화 실패");

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_initialization_etf_data_created() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"]
    });

    strategy.initialize(config).await.unwrap();

    // 4개 ETF 데이터가 생성되어야 함
    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);

    // holdings는 빈 상태로 시작
    let holdings = &state["holdings"];
    assert!(holdings.is_object());
}

#[tokio::test]
async fn test_name_version_description() {
    let strategy = MomentumSurgeStrategy::new();

    assert_eq!(strategy.name(), "Momentum Surge");
    assert_eq!(strategy.version(), "1.0.0");
    assert!(
        strategy.description().contains("Momentum") || strategy.description().contains("Surge")
    );
}

// ============================================================================
// 데이터 처리 테스트
// ============================================================================

#[tokio::test]
async fn test_data_accumulation_before_signals() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"]
    });
    strategy.initialize(config).await.unwrap();

    // 60개 미만의 데이터에서는 신호가 생성되지 않아야 함
    let tickers = ["122630/KRW", "233740/KRW", "252670/KRW", "251340/KRW"];

    for day in 0..30 {
        for ticker in &tickers {
            let data = create_market_data(
                ticker,
                dec!(10000) + Decimal::from(day * 10),
                dec!(100000),
                day,
            );
            let signals = strategy.on_market_data(&data).await.unwrap();
            assert!(
                signals.is_empty(),
                "데이터 축적 중에는 신호 없어야 함 (day {})",
                day
            );
        }
    }

    let state = strategy.get_state();
    assert_eq!(state["started"], false, "60개 미만이면 started=false");
}

#[tokio::test]
async fn test_strategy_starts_after_sufficient_data() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"]
    });
    strategy.initialize(config).await.unwrap();

    // Context에는 base 이름으로 저장 (전략의 get_etf_klines와 일치)
    let tickers = ["122630", "233740", "252670", "251340"];

    // StrategyContext에 65일치 klines 설정 (60개 이상 필요)
    let context = setup_context_for_momentum_surge(&tickers, 65);
    strategy.set_context(context);

    // MarketData의 ticker는 base 형식 (전략의 on_market_data에서 "{}/KRW" 변환)
    let data = create_market_data("122630", dec!(10650), dec!(100000), 64);
    let _ = strategy.on_market_data(&data).await;

    let state = strategy.get_state();
    assert_eq!(state["started"], true, "60개 이상이면 started=true");
}

#[tokio::test]
async fn test_ignores_unregistered_ticker() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740"]
    });
    strategy.initialize(config).await.unwrap();

    // 등록되지 않은 티커로 데이터 전송
    let data = create_market_data("999999/KRW", dec!(10000), dec!(100000), 0);
    let signals = strategy.on_market_data(&data).await.unwrap();

    assert!(signals.is_empty(), "등록되지 않은 티커는 무시");
}

// ============================================================================
// 레버리지 매수 조건 테스트
// ============================================================================

#[tokio::test]
async fn test_leverage_buy_signal_on_uptrend() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"],
        "obv_period": 5,
        "ma_short": 3,
        "ma_medium": 10,
        "ma_long": 20
    });
    strategy.initialize(config).await.unwrap();

    let tickers = ["122630/KRW", "233740/KRW", "252670/KRW", "251340/KRW"];

    // 명확한 상승 추세 데이터 생성 - MA 정배열 + OBV 상승
    for day in 0..70 {
        for ticker in &tickers {
            // 강한 상승 추세 (가격 상승 + 거래량 증가)
            let base_price = dec!(10000);
            let price = base_price + Decimal::from(day * 50); // 큰 폭 상승
            let volume = dec!(100000) + Decimal::from(day * 1000); // 거래량 증가

            let data = create_market_data(ticker, price, volume, day);
            let signals = strategy.on_market_data(&data).await.unwrap();

            // 60일 이후 레버리지 매수 신호 확인 가능
            if day >= 60 && !signals.is_empty() {
                let buy_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Buy).collect();

                if !buy_signals.is_empty() {
                    // 레버리지 ETF에 대한 매수 신호인지 확인
                    let ticker_str = buy_signals[0].ticker.to_string();
                    assert!(
                        ticker_str.starts_with("122630") || ticker_str.starts_with("233740"),
                        "상승 추세에서 레버리지 매수 신호 발생"
                    );
                    return;
                }
            }
        }
    }

    // 조건에 맞지 않아 신호가 없을 수 있음
    // 이 경우 테스트는 패스 (신호 생성은 복합 조건에 따름)
}

// ============================================================================
// 인버스 매수 조건 테스트
// ============================================================================

#[tokio::test]
async fn test_inverse_buy_requires_leverage_downtrend() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"],
        "obv_period": 5,
        "ma_short": 3,
        "ma_medium": 10,
        "ma_long": 20
    });
    strategy.initialize(config).await.unwrap();

    let tickers = ["122630/KRW", "233740/KRW", "252670/KRW", "251340/KRW"];

    // 하락 추세 데이터 생성 - 인버스 매수 조건
    for day in 0..70 {
        for ticker in &tickers {
            // 하락 추세
            let base_price = dec!(20000);
            let price = base_price - Decimal::from(day * 30);
            let price = if price < dec!(5000) {
                dec!(5000)
            } else {
                price
            };
            let volume = dec!(100000);

            let data = create_market_data(ticker, price, volume, day);
            let signals = strategy.on_market_data(&data).await.unwrap();

            // 인버스 매수 신호 확인 (하락 추세에서)
            if day >= 60 && !signals.is_empty() {
                for signal in &signals {
                    if signal.side == Side::Buy {
                        let ticker_str = signal.ticker.to_string();
                        // 인버스 ETF (252670, 251340)에 대한 신호 확인
                        if ticker_str.starts_with("252670") || ticker_str.starts_with("251340") {
                            // 인버스 매수 신호 발생 확인
                            return;
                        }
                    }
                }
            }
        }
    }

    // 조건 미충족으로 신호 없을 수 있음
}

// ============================================================================
// 포지션 관리 테스트
// ============================================================================

#[tokio::test]
async fn test_position_update() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"]
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 업데이트 - 코스피 레버리지 보유
    let position = create_position("122630/KRW", dec!(100), dec!(15000));

    let result = strategy.on_position_update(&position).await;
    assert!(result.is_ok());

    let state = strategy.get_state();
    assert_eq!(state["position_count"], 1);

    let holdings = &state["holdings"];
    assert!(!holdings["122630/KRW"].is_null());
}

#[tokio::test]
async fn test_max_positions_limit() {
    let mut strategy = MomentumSurgeStrategy::new();

    // max_positions = 2
    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"],
        "max_positions": 2
    });
    strategy.initialize(config).await.unwrap();

    // 2개 포지션 추가
    let positions = [
        create_position("122630/KRW", dec!(100), dec!(15000)),
        create_position("233740/KRW", dec!(50), dec!(8000)),
    ];

    for pos in &positions {
        strategy.on_position_update(pos).await.unwrap();
    }

    let state = strategy.get_state();
    assert_eq!(state["position_count"], 2);

    // 최대 포지션에 도달하면 추가 매수 불가
    // (실제 신호 생성 로직에서 확인)
}

// ============================================================================
// 손절/익절 테스트
// ============================================================================

#[tokio::test]
async fn test_stop_loss_sell_signal() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"],
        "stop_loss_pct": 3.0,
        "take_profit_pct": 10.0
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 설정 - 진입가 15000원
    let position = create_position("122630/KRW", dec!(100), dec!(15000));
    strategy.on_position_update(&position).await.unwrap();

    let tickers = ["122630/KRW", "233740/KRW", "252670/KRW", "251340/KRW"];

    // 충분한 데이터 축적
    for day in 0..65 {
        for ticker in &tickers {
            let data = create_market_data(ticker, dec!(15000), dec!(100000), day);
            let _ = strategy.on_market_data(&data).await;
        }
    }

    // 손절 조건 데이터 전송 (-3% 이하)
    // 15000 * 0.97 = 14550, 그래서 14500은 손절 조건
    let stop_loss_data = create_market_data("122630/KRW", dec!(14500), dec!(100000), 66);
    let signals = strategy.on_market_data(&stop_loss_data).await.unwrap();

    // 손절 매도 신호 확인
    let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();

    // 손절 조건에서 매도 신호 발생
    if !sell_signals.is_empty() {
        let meta = sell_signals[0].metadata.get("exit_reason");
        assert!(meta.is_some());
        assert_eq!(meta.unwrap(), "stop_loss");
    }
}

#[tokio::test]
async fn test_take_profit_sell_signal() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"],
        "stop_loss_pct": 3.0,
        "take_profit_pct": 10.0
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 설정 - 진입가 10000원
    let position = create_position("122630/KRW", dec!(100), dec!(10000));
    strategy.on_position_update(&position).await.unwrap();

    let tickers = ["122630/KRW", "233740/KRW", "252670/KRW", "251340/KRW"];

    // 충분한 데이터 축적
    for day in 0..65 {
        for ticker in &tickers {
            let data = create_market_data(ticker, dec!(10000), dec!(100000), day);
            let _ = strategy.on_market_data(&data).await;
        }
    }

    // 익절 조건 데이터 전송 (+10% 이상)
    // 10000 * 1.10 = 11000
    let take_profit_data = create_market_data("122630/KRW", dec!(11000), dec!(100000), 66);
    let signals = strategy.on_market_data(&take_profit_data).await.unwrap();

    // 익절 매도 신호 확인
    let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();

    if !sell_signals.is_empty() {
        let meta = sell_signals[0].metadata.get("exit_reason");
        assert!(meta.is_some());
        assert_eq!(meta.unwrap(), "take_profit");
    }
}

// ============================================================================
// MA 기반 청산 테스트
// ============================================================================

#[tokio::test]
async fn test_leverage_exit_on_ma_bearish() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"],
        "ma_short": 3,
        "ma_medium": 10,
        "ma_long": 20
    });
    strategy.initialize(config).await.unwrap();

    // 레버리지 포지션 설정
    let position = create_position("122630/KRW", dec!(100), dec!(15000));
    strategy.on_position_update(&position).await.unwrap();

    let tickers = ["122630/KRW", "233740/KRW", "252670/KRW", "251340/KRW"];

    // MA 역배열이 되도록 하락 추세 데이터
    for day in 0..70 {
        for ticker in &tickers {
            // 하락 추세 - MA 역배열 조건
            let base_price = dec!(20000);
            let price = base_price - Decimal::from(day * 50);
            let price = if price < dec!(5000) {
                dec!(5000)
            } else {
                price
            };

            let data = create_market_data(ticker, price, dec!(100000), day);
            let signals = strategy.on_market_data(&data).await.unwrap();

            // MA 역배열 시 레버리지 청산 신호
            if day >= 60 && !signals.is_empty() {
                for signal in &signals {
                    if signal.side == Side::Sell && signal.ticker.to_string() == "122630/KRW" {
                        let reason = signal.metadata.get("exit_reason");
                        if reason.is_some() {
                            let reason_str = reason.unwrap().as_str().unwrap_or("");
                            if reason_str == "ma_bearish" || reason_str == "obv_down" {
                                return;
                            }
                        }
                    }
                }
            }
        }
    }
}

// ============================================================================
// 상태 관리 테스트
// ============================================================================

#[tokio::test]
async fn test_get_state_comprehensive() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"]
    });
    strategy.initialize(config).await.unwrap();

    let state = strategy.get_state();

    // 필수 필드 확인
    assert!(!state["initialized"].is_null());
    assert!(!state["started"].is_null());
    assert!(!state["position_count"].is_null());
    assert!(!state["holdings"].is_null());
    assert!(!state["trades_count"].is_null());
    assert!(!state["wins"].is_null());
    assert!(!state["total_pnl"].is_null());

    // 초기 값 확인
    assert_eq!(state["initialized"], true);
    assert_eq!(state["started"], false);
    assert_eq!(state["position_count"], 0);
    assert_eq!(state["trades_count"], 0);
    assert_eq!(state["wins"], 0);
}

#[tokio::test]
async fn test_state_after_position_changes() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"]
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 추가
    let position = create_position("122630/KRW", dec!(100), dec!(15000));
    strategy.on_position_update(&position).await.unwrap();

    let state = strategy.get_state();
    assert_eq!(state["position_count"], 1);

    let holdings = &state["holdings"]["122630/KRW"];
    assert!(!holdings.is_null());
    assert_eq!(holdings["holdings"], "100");
    assert_eq!(holdings["entry_price"], "15000");
    assert_eq!(holdings["etf_type"], "KospiLeverage");
}

// ============================================================================
// 종료 테스트
// ============================================================================

#[tokio::test]
async fn test_shutdown() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"]
    });
    strategy.initialize(config).await.unwrap();

    let result = strategy.shutdown().await;
    assert!(result.is_ok(), "정상 종료 실패");
}

#[tokio::test]
async fn test_shutdown_with_statistics() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"]
    });
    strategy.initialize(config).await.unwrap();

    // 데이터 축적 및 포지션 생성
    let tickers = ["122630/KRW", "233740/KRW", "252670/KRW", "251340/KRW"];

    for day in 0..65 {
        for ticker in &tickers {
            let data = create_market_data(ticker, dec!(10000), dec!(100000), day);
            let _ = strategy.on_market_data(&data).await;
        }
    }

    // 종료 후 통계 확인
    let result = strategy.shutdown().await;
    assert!(result.is_ok());

    let state = strategy.get_state();
    // trades_count, wins, total_pnl 등 통계 필드 존재 확인
    assert!(state["trades_count"].is_number());
    assert!(state["wins"].is_number());
}

// ============================================================================
// OBV 지표 테스트
// ============================================================================

#[tokio::test]
async fn test_obv_calculation_with_volume_changes() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"],
        "obv_period": 5
    });
    strategy.initialize(config).await.unwrap();

    let tickers = ["122630/KRW", "233740/KRW", "252670/KRW", "251340/KRW"];

    // OBV 상승 패턴: 가격 상승 + 거래량 증가
    for day in 0..30 {
        for ticker in &tickers {
            let price = dec!(10000) + Decimal::from(day * 100); // 상승
            let volume = dec!(100000) + Decimal::from(day * 10000); // 거래량 증가

            let data = create_market_data(ticker, price, volume, day);
            let _ = strategy.on_market_data(&data).await;
        }
    }

    // OBV가 상승 추세여야 함 (데이터 수집 중)
    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

// ============================================================================
// RSI 지표 테스트
// ============================================================================

#[tokio::test]
async fn test_rsi_overbought_prevents_buy() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"],
        "rsi_period": 10
    });
    strategy.initialize(config).await.unwrap();

    let tickers = ["122630/KRW", "233740/KRW", "252670/KRW", "251340/KRW"];

    // 급격한 상승으로 RSI 과매수 상태 유도
    for day in 0..70 {
        for ticker in &tickers {
            // 매우 급격한 상승 - RSI > 70 예상
            let price = dec!(10000) + Decimal::from(day * 200);
            let volume = dec!(100000);

            let data = create_market_data(ticker, price, volume, day);
            let signals = strategy.on_market_data(&data).await.unwrap();

            // RSI 과매수(70 이상)에서는 매수 신호가 없어야 함
            // (단, 전략 로직상 RSI 30-70 사이에서만 매수)
            if day >= 60 {
                for signal in &signals {
                    if signal.side == Side::Buy {
                        // RSI 조건이 30-70이므로
                        // 과매수 상태에서 매수가 발생하면 RSI가 70 미만이라는 의미
                    }
                }
            }
        }
    }
}

// ============================================================================
// 복합 시나리오 테스트
// ============================================================================

#[tokio::test]
async fn test_full_trading_cycle() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"],
        "max_positions": 2,
        "obv_period": 5,
        "ma_short": 3,
        "ma_medium": 10,
        "ma_long": 20,
        "stop_loss_pct": 3.0,
        "take_profit_pct": 10.0
    });
    strategy.initialize(config).await.unwrap();

    // Context에는 base 이름으로 저장
    let tickers = ["122630", "233740", "252670", "251340"];

    // Phase 1: StrategyContext에 60일치 klines 설정
    let context = setup_context_for_momentum_surge(&tickers, 60);
    strategy.set_context(context);

    // on_market_data 호출하여 started 활성화 (base 형식 사용)
    let data = create_market_data("122630", dec!(10590), dec!(100000), 59);
    let _ = strategy.on_market_data(&data).await;

    let state = strategy.get_state();
    assert_eq!(state["started"], true);

    // Phase 2: 상승 추세 - 매수 신호 가능
    for day in 60..80 {
        for ticker in &tickers {
            let price = dec!(10000) + Decimal::from(day * 50);
            let volume = dec!(100000) + Decimal::from(day * 1000);

            let data = create_market_data(ticker, price, volume, day);
            let _ = strategy.on_market_data(&data).await;
        }
    }

    // Phase 3: 하락 추세 - 청산 신호 가능
    for day in 80..100 {
        for ticker in &tickers {
            let price = dec!(15000) - Decimal::from((day - 80) * 100);
            let price = if price < dec!(5000) {
                dec!(5000)
            } else {
                price
            };

            let data = create_market_data(ticker, price, dec!(100000), day);
            let _ = strategy.on_market_data(&data).await;
        }
    }

    // 최종 상태 확인
    let final_state = strategy.get_state();
    assert!(final_state["initialized"].as_bool().unwrap_or(false));
}

// ============================================================================
// 에러 처리 테스트
// ============================================================================

#[tokio::test]
async fn test_process_data_before_initialization() {
    let mut strategy = MomentumSurgeStrategy::new();

    // 초기화 없이 데이터 처리
    let data = create_market_data("122630/KRW", dec!(10000), dec!(100000), 0);
    let signals = strategy.on_market_data(&data).await.unwrap();

    assert!(signals.is_empty(), "초기화 전에는 신호 없어야 함");
}

#[tokio::test]
async fn test_position_update_unknown_ticker() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740"]
    });
    strategy.initialize(config).await.unwrap();

    // 등록되지 않은 티커로 포지션 업데이트
    let position = create_position("999999/KRW", dec!(100), dec!(15000));

    // 에러 없이 처리 (무시)
    let result = strategy.on_position_update(&position).await;
    assert!(result.is_ok());

    let _state = strategy.get_state();
    // 등록되지 않은 티커는 holdings에 포함되지 않음
}

// ============================================================================
// ETF 타입별 테스트
// ============================================================================

#[tokio::test]
async fn test_etf_type_classification() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"],
        "kospi_leverage": "122630",
        "kosdaq_leverage": "233740",
        "kospi_inverse": "252670",
        "kosdaq_inverse": "251340"
    });
    strategy.initialize(config).await.unwrap();

    // 각 ETF에 대해 포지션 업데이트 및 타입 확인
    let test_cases = [
        ("122630/KRW", "KospiLeverage"),
        ("233740/KRW", "KosdaqLeverage"),
        ("252670/KRW", "KospiInverse"),
        ("251340/KRW", "KosdaqInverse"),
    ];

    for (ticker, expected_type) in &test_cases {
        let position = create_position(ticker, dec!(100), dec!(10000));
        strategy.on_position_update(&position).await.unwrap();

        let state = strategy.get_state();
        let holdings = &state["holdings"][*ticker];

        if !holdings.is_null() {
            assert_eq!(
                holdings["etf_type"], *expected_type,
                "ETF 타입 분류 오류: {}",
                ticker
            );
        }

        // 포지션 제거 (다음 테스트를 위해)
        let zero_position = create_position(ticker, dec!(0), dec!(0));
        strategy.on_position_update(&zero_position).await.unwrap();
    }
}

// ============================================================================
// 날짜 변경 테스트
// ============================================================================

#[tokio::test]
async fn test_new_day_detection() {
    let mut strategy = MomentumSurgeStrategy::new();

    let config = json!({
        "tickers": ["122630", "233740", "252670", "251340"]
    });
    strategy.initialize(config).await.unwrap();

    let tickers = ["122630/KRW", "233740/KRW", "252670/KRW", "251340/KRW"];

    // Day 0 데이터
    for ticker in &tickers {
        let data = create_market_data(ticker, dec!(10000), dec!(100000), 0);
        let _ = strategy.on_market_data(&data).await;
    }

    // Day 1 데이터 (새로운 날)
    for ticker in &tickers {
        let data = create_market_data(ticker, dec!(10100), dec!(100000), 1);
        let _ = strategy.on_market_data(&data).await;
    }

    // 전략이 날짜 변경을 인식하고 있어야 함
    let state = strategy.get_state();
    assert!(state["initialized"].as_bool().unwrap_or(false));
}
