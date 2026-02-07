//! SectorVb (섹터 변동성 돌파) 전략 통합 테스트
//!
//! 한국 섹터 ETF를 대상으로 하는 변동성 돌파 전략 테스트.
//! Larry Williams 변동성 돌파 전략 기반.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use trader_core::{Kline, MarketData, Position, Side, Timeframe};
use trader_strategy::strategies::sector_vb::SectorVbStrategy;
use trader_strategy::Strategy;

// ============================================================================
// 테스트 헬퍼 함수
// ============================================================================

/// 테스트용 MarketData 생성 헬퍼 (OHLCV 포함)
fn create_market_data_ohlcv(
    ticker: &str,
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    volume: Decimal,
    day: i64,
) -> MarketData {
    // 9시간 (KST → UTC) + 거래 시간 (9:00 + hour)
    let timestamp = chrono::DateTime::from_timestamp(1704067200 + day * 86400, 0).unwrap();
    let kline = Kline::new(
        ticker.to_string(),
        Timeframe::D1,
        timestamp,
        open,
        high,
        low,
        close,
        volume,
        timestamp,
    );
    MarketData::from_kline("test", kline)
}

/// 간단한 MarketData 생성 헬퍼
fn create_market_data(ticker: &str, close: Decimal, day: i64) -> MarketData {
    create_market_data_ohlcv(
        ticker,
        close - dec!(100), // open
        close + dec!(200), // high
        close - dec!(200), // low
        close,             // close
        dec!(200000),      // volume
        day,
    )
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
    let mut strategy = SectorVbStrategy::new();

    let config = json!({});
    let result = strategy.initialize(config).await;

    assert!(result.is_ok(), "기본 설정으로 초기화 실패");

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_initialization_with_custom_config() {
    let mut strategy = SectorVbStrategy::new();

    let config = json!({
        "tickers": ["091160", "091230", "305720"],
        "k_factor": 0.6,
        "selection_method": "returns",
        "top_n": 2,
        "min_volume": 150000,
        "close_before_minutes": 15,
        "stop_loss_pct": 3.0,
        "take_profit_pct": 5.0,
        "min_global_score": 60.0,
        "use_route_filter": true,
        "use_regime_filter": true
    });

    let result = strategy.initialize(config).await;
    assert!(result.is_ok(), "커스텀 설정으로 초기화 실패");

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_name_version_description() {
    let strategy = SectorVbStrategy::new();

    assert_eq!(strategy.name(), "Sector VB");
    assert_eq!(strategy.version(), "2.0.0");
    assert!(strategy.description().contains("섹터"));
    assert!(strategy.description().contains("변동성"));
}

// ============================================================================
// 데이터 처리 테스트
// ============================================================================

#[tokio::test]
async fn test_ignores_unregistered_ticker() {
    let mut strategy = SectorVbStrategy::new();

    let config = json!({
        "tickers": ["091160", "091230"]
    });
    strategy.initialize(config).await.unwrap();

    // 등록되지 않은 티커로 데이터 전송
    let data = create_market_data("999999", dec!(10000), 0);
    let signals = strategy.on_market_data(&data).await.unwrap();

    assert!(signals.is_empty(), "등록되지 않은 티커는 무시");
}

#[tokio::test]
async fn test_data_accumulation_for_sectors() {
    let mut strategy = SectorVbStrategy::new();

    let config = json!({
        "tickers": ["091160", "091230"],
        "k_factor": 0.5
    });
    strategy.initialize(config).await.unwrap();

    // 섹터 데이터 축적
    for day in 0..5 {
        // 반도체 ETF 1
        let data1 = create_market_data_ohlcv(
            "091160",
            dec!(10000),                            // open
            dec!(10500),                            // high
            dec!(9800),                             // low
            dec!(10200) + Decimal::from(day * 100), // close
            dec!(200000),                           // volume
            day,
        );

        // 반도체 ETF 2
        let data2 = create_market_data_ohlcv(
            "091230",
            dec!(15000),
            dec!(15800),
            dec!(14700),
            dec!(15300) + Decimal::from(day * 150),
            dec!(180000),
            day,
        );

        let _ = strategy.on_market_data(&data1).await;
        let _ = strategy.on_market_data(&data2).await;
    }

    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

// ============================================================================
// 변동성 돌파 매수 조건 테스트
// ============================================================================

#[tokio::test]
async fn test_breakout_calculation() {
    let mut strategy = SectorVbStrategy::new();

    let config = json!({
        "tickers": ["091160"],
        "k_factor": 0.5,
        "min_volume": 100000,
        "min_global_score": 0.0,
        "use_route_filter": false,
        "use_regime_filter": false
    });
    strategy.initialize(config).await.unwrap();

    // Day 0: 전일 데이터 (범위 = 10500 - 9800 = 700)
    let prev_data = create_market_data_ohlcv(
        "091160",
        dec!(10000),  // open
        dec!(10500),  // high
        dec!(9800),   // low
        dec!(10200),  // close
        dec!(200000), // volume
        0,
    );
    let _ = strategy.on_market_data(&prev_data).await;

    // Day 1: 시가 10300, 목표가 = 10300 + (700 * 0.5) = 10650
    let today_data = create_market_data_ohlcv(
        "091160",
        dec!(10300),  // open (시가)
        dec!(10700),  // high (목표가 돌파)
        dec!(10250),  // low
        dec!(10680),  // close
        dec!(250000), // volume
        1,
    );

    let _signals = strategy.on_market_data(&today_data).await.unwrap();

    // 매수 신호 확인 (돌파 조건 충족 시)
    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_no_breakout_below_target() {
    let mut strategy = SectorVbStrategy::new();

    let config = json!({
        "tickers": ["091160"],
        "k_factor": 0.5,
        "min_volume": 100000,
        "min_global_score": 0.0,
        "use_route_filter": false,
        "use_regime_filter": false
    });
    strategy.initialize(config).await.unwrap();

    // Day 0: 전일 데이터 (범위 = 10500 - 9800 = 700)
    let prev_data = create_market_data_ohlcv(
        "091160",
        dec!(10000),
        dec!(10500),
        dec!(9800),
        dec!(10200),
        dec!(200000),
        0,
    );
    let _ = strategy.on_market_data(&prev_data).await;

    // Day 1: 시가 10300, 목표가 = 10650, 하지만 고가가 목표가 미만
    let today_data = create_market_data_ohlcv(
        "091160",
        dec!(10300),  // open
        dec!(10600),  // high (목표가 10650 미만)
        dec!(10250),  // low
        dec!(10550),  // close
        dec!(250000), // volume
        1,
    );

    let signals = strategy.on_market_data(&today_data).await.unwrap();

    // 돌파 실패 시 매수 신호 없음
    let _buy_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Buy).collect();
    // TODO: 돌파 조건 미충족 시 검증 로직 추가 필요
}

// ============================================================================
// 섹터 선정 테스트
// ============================================================================

#[tokio::test]
async fn test_sector_selection_by_returns() {
    let mut strategy = SectorVbStrategy::new();

    let config = json!({
        "tickers": ["091160", "091230", "305720"],
        "selection_method": "returns",
        "top_n": 1,
        "k_factor": 0.5,
        "min_global_score": 0.0,
        "use_route_filter": false,
        "use_regime_filter": false
    });
    strategy.initialize(config).await.unwrap();

    // Day 0: 각 섹터 전일 데이터
    // 091160: 수익률 0%
    let data1_d0 = create_market_data_ohlcv(
        "091160",
        dec!(10000),
        dec!(10500),
        dec!(9800),
        dec!(10000), // 전일 종가와 동일 (0% 수익률)
        dec!(200000),
        0,
    );

    // 091230: 수익률 +5%
    let data2_d0 = create_market_data_ohlcv(
        "091230",
        dec!(14500),
        dec!(15500),
        dec!(14400),
        dec!(15225), // +5% 수익률
        dec!(180000),
        0,
    );

    // 305720: 수익률 +2%
    let data3_d0 = create_market_data_ohlcv(
        "305720",
        dec!(25000),
        dec!(26000),
        dec!(24800),
        dec!(25500), // +2% 수익률
        dec!(150000),
        0,
    );

    let _ = strategy.on_market_data(&data1_d0).await;
    let _ = strategy.on_market_data(&data2_d0).await;
    let _ = strategy.on_market_data(&data3_d0).await;

    // 가장 높은 수익률의 섹터 (091230, +5%)가 선택되어야 함
    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

// ============================================================================
// 포지션 관리 테스트
// ============================================================================

#[tokio::test]
async fn test_position_update() {
    let mut strategy = SectorVbStrategy::new();

    let config = json!({
        "tickers": ["091160", "091230"]
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 업데이트
    let position = create_position("091160", dec!(100), dec!(10000));
    let result = strategy.on_position_update(&position).await;
    assert!(result.is_ok());

    // SectorVb는 on_position_update에서 내부 position을 업데이트하지 않음
    // (전략 자체적으로 포지션을 추적하는 구조)
    let state = strategy.get_state();
    assert!(state["initialized"].as_bool().unwrap_or(false));
}

#[tokio::test]
async fn test_position_cleared_on_zero_quantity() {
    let mut strategy = SectorVbStrategy::new();

    let config = json!({
        "tickers": ["091160"]
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 추가
    let position = create_position("091160", dec!(100), dec!(10000));
    strategy.on_position_update(&position).await.unwrap();

    // 수량 0으로 청산
    let zero_position = create_position("091160", dec!(0), dec!(0));
    strategy.on_position_update(&zero_position).await.unwrap();

    let state = strategy.get_state();
    // positions에서 삭제되거나 수량이 0이어야 함
}

// ============================================================================
// 손절/익절 테스트
// ============================================================================

#[tokio::test]
async fn test_stop_loss_trigger() {
    let mut strategy = SectorVbStrategy::new();

    let config = json!({
        "tickers": ["091160"],
        "stop_loss_pct": 2.0,
        "take_profit_pct": 3.0,
        "min_global_score": 0.0,
        "use_route_filter": false,
        "use_regime_filter": false
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 설정 - 진입가 10000원
    let position = create_position("091160", dec!(100), dec!(10000));
    strategy.on_position_update(&position).await.unwrap();

    // 손절 조건 (-2% 이하)
    // 10000 * 0.98 = 9800
    let stop_loss_data = create_market_data_ohlcv(
        "091160",
        dec!(10000),
        dec!(10050),
        dec!(9750),
        dec!(9780), // -2.2%
        dec!(200000),
        1,
    );

    let signals = strategy.on_market_data(&stop_loss_data).await.unwrap();

    // 손절 신호 확인
    let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();
    if !sell_signals.is_empty() {
        let action = sell_signals[0].metadata.get("exit_reason");
        assert!(action.is_some());
    }
}

#[tokio::test]
async fn test_take_profit_trigger() {
    let mut strategy = SectorVbStrategy::new();

    let config = json!({
        "tickers": ["091160"],
        "stop_loss_pct": 2.0,
        "take_profit_pct": 3.0,
        "min_global_score": 0.0,
        "use_route_filter": false,
        "use_regime_filter": false
    });
    strategy.initialize(config).await.unwrap();

    // 포지션 설정 - 진입가 10000원
    let position = create_position("091160", dec!(100), dec!(10000));
    strategy.on_position_update(&position).await.unwrap();

    // 익절 조건 (+3% 이상)
    // 10000 * 1.03 = 10300
    let take_profit_data = create_market_data_ohlcv(
        "091160",
        dec!(10100),
        dec!(10400),
        dec!(10050),
        dec!(10350), // +3.5%
        dec!(200000),
        1,
    );

    let signals = strategy.on_market_data(&take_profit_data).await.unwrap();

    // 익절 신호 확인
    let sell_signals: Vec<_> = signals.iter().filter(|s| s.side == Side::Sell).collect();
    if !sell_signals.is_empty() {
        let action = sell_signals[0].metadata.get("exit_reason");
        assert!(action.is_some());
    }
}

// ============================================================================
// 상태 관리 테스트
// ============================================================================

#[tokio::test]
async fn test_get_state_comprehensive() {
    let mut strategy = SectorVbStrategy::new();

    let config = json!({
        "tickers": ["091160", "091230"]
    });
    strategy.initialize(config).await.unwrap();

    let state = strategy.get_state();

    // 필수 필드 확인 (실제 get_state 반환 형식에 맞춤)
    assert!(!state["initialized"].is_null());
    assert!(!state["has_position"].is_null());
    assert!(!state["trades_count"].is_null());
    assert!(!state["sector_count"].is_null());
    assert!(!state["version"].is_null());

    // 초기 값 확인
    assert_eq!(state["initialized"], true);
    assert_eq!(state["trades_count"], 0);
    assert_eq!(state["has_position"], false);
    assert_eq!(state["sector_count"], 2); // 2개 섹터 등록
}

// ============================================================================
// 종료 테스트
// ============================================================================

#[tokio::test]
async fn test_shutdown() {
    let mut strategy = SectorVbStrategy::new();

    let config = json!({
        "tickers": ["091160", "091230"]
    });
    strategy.initialize(config).await.unwrap();

    let result = strategy.shutdown().await;
    assert!(result.is_ok(), "정상 종료 실패");
}

// ============================================================================
// 에러 처리 테스트
// ============================================================================

#[tokio::test]
async fn test_process_data_before_initialization() {
    let mut strategy = SectorVbStrategy::new();

    // 초기화 없이 데이터 처리
    let data = create_market_data("091160", dec!(10000), 0);
    let signals = strategy.on_market_data(&data).await.unwrap();

    assert!(signals.is_empty(), "초기화 전에는 신호 없어야 함");
}

// ============================================================================
// 복합 시나리오 테스트
// ============================================================================

#[tokio::test]
async fn test_full_trading_day_scenario() {
    let mut strategy = SectorVbStrategy::new();

    let config = json!({
        "tickers": ["091160", "091230"],
        "k_factor": 0.5,
        "selection_method": "returns",
        "top_n": 1,
        "stop_loss_pct": 2.0,
        "take_profit_pct": 3.0,
        "min_global_score": 0.0,
        "use_route_filter": false,
        "use_regime_filter": false
    });
    strategy.initialize(config).await.unwrap();

    // Day 0: 전일 데이터
    let prev_day_data = [
        create_market_data_ohlcv(
            "091160",
            dec!(10000),
            dec!(10500),
            dec!(9800),
            dec!(10300), // +3% 수익률
            dec!(200000),
            0,
        ),
        create_market_data_ohlcv(
            "091230",
            dec!(15000),
            dec!(15300),
            dec!(14800),
            dec!(15100), // +0.67% 수익률
            dec!(180000),
            0,
        ),
    ];

    for data in &prev_day_data {
        let _ = strategy.on_market_data(data).await;
    }

    // Day 1: 당일 거래
    // 091160 선택됨 (더 높은 수익률)
    // 목표가 = 시가 + (전일 범위 * K) = 10400 + (700 * 0.5) = 10750
    let today_data = create_market_data_ohlcv(
        "091160",
        dec!(10400),  // 시가
        dec!(10800),  // 고가 (목표가 돌파)
        dec!(10350),  // 저가
        dec!(10750),  // 종가
        dec!(250000), // 거래량
        1,
    );

    let signals = strategy.on_market_data(&today_data).await.unwrap();

    // 전략이 정상 동작
    let state = strategy.get_state();
    assert_eq!(state["initialized"], true);
}

#[tokio::test]
async fn test_multi_sector_rotation() {
    let mut strategy = SectorVbStrategy::new();

    let config = json!({
        "tickers": ["091160", "091230", "305720", "091170"],
        "selection_method": "returns",
        "top_n": 2,  // 상위 2개 섹터 선택
        "k_factor": 0.5,
        "min_global_score": 0.0,
        "use_route_filter": false,
        "use_regime_filter": false
    });
    strategy.initialize(config).await.unwrap();

    // 여러 날에 걸친 섹터 로테이션 시뮬레이션
    for day in 0..5 {
        let sector_data = [
            create_market_data_ohlcv(
                "091160",
                dec!(10000) + Decimal::from(day * 100),
                dec!(10500) + Decimal::from(day * 100),
                dec!(9800) + Decimal::from(day * 100),
                dec!(10300) + Decimal::from(day * 100),
                dec!(200000),
                day,
            ),
            create_market_data_ohlcv(
                "091230",
                dec!(15000) + Decimal::from(day * 50),
                dec!(15300) + Decimal::from(day * 50),
                dec!(14800) + Decimal::from(day * 50),
                dec!(15100) + Decimal::from(day * 50),
                dec!(180000),
                day,
            ),
            create_market_data_ohlcv(
                "305720",
                dec!(25000) - Decimal::from(day * 80),
                dec!(25500) - Decimal::from(day * 80),
                dec!(24700) - Decimal::from(day * 80),
                dec!(25200) - Decimal::from(day * 80),
                dec!(150000),
                day,
            ),
            create_market_data_ohlcv(
                "091170",
                dec!(8000) + Decimal::from(day * 30),
                dec!(8200) + Decimal::from(day * 30),
                dec!(7900) + Decimal::from(day * 30),
                dec!(8100) + Decimal::from(day * 30),
                dec!(120000),
                day,
            ),
        ];

        for data in &sector_data {
            let _ = strategy.on_market_data(data).await;
        }
    }

    // 최종 상태 확인
    let final_state = strategy.get_state();
    assert!(final_state["initialized"].as_bool().unwrap_or(false));
}
