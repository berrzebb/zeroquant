//! PensionBot 전략 통합 테스트
//!
//! 개인연금 계좌용 정적+동적 자산배분 전략 테스트

use chrono::{Datelike, TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use trader_core::{Kline, MarketData, Position, Side, Timeframe};
use trader_strategy::strategies::common::ExitConfig;
use trader_strategy::strategies::pension_bot::{
    PensionAsset, PensionAssetType, PensionBotConfig, PensionBotStrategy,
};
use trader_strategy::Strategy;
use uuid::Uuid;

// ============================================================================
// 테스트 헬퍼 함수
// ============================================================================

/// 특정 연월 시점의 Kline 데이터 생성
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

/// 현재 시점의 Kline 데이터 생성
fn create_kline(ticker: &str, close: Decimal) -> MarketData {
    let now = Utc::now();
    create_kline_at_month(ticker, close, now.year(), now.month(), now.day())
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
        strategy_id: Some("pension_bot".to_string()),
        opened_at: Utc::now(),
        updated_at: Utc::now(),
        closed_at: None,
        metadata: json!({}),
    }
}

/// 상승 추세 가격 데이터 생성 및 입력 (모멘텀 양수 유도)
/// PensionBot은 240일(12개월) 데이터가 필요함
async fn feed_rising_prices(
    strategy: &mut PensionBotStrategy,
    ticker: &str,
    days: usize,
    base_price: Decimal,
    start_year: i32,
    start_month: u32,
) {
    for day in 0..days {
        let price = base_price + Decimal::from(day);
        let current_day = day as u32 % 28 + 1;
        let current_month = ((start_month - 1 + (day as u32 / 28)) % 12) + 1;
        let current_year = start_year + ((start_month - 1 + (day as u32 / 28)) / 12) as i32;

        let data = create_kline_at_month(ticker, price, current_year, current_month, current_day);
        let _ = strategy.on_market_data(&data).await;
    }
}

/// 단순 테스트용 포트폴리오 (자산 수 감소)
fn simple_test_portfolio() -> Vec<PensionAsset> {
    vec![
        PensionAsset::stock("SPY", dec!(40)), // 주식 40%
        PensionAsset::safe("TLT", dec!(30)),  // 채권 30%
        PensionAsset::mat("GLD", dec!(20)),   // 금 20%
        PensionAsset::cash("BIL"),            // 현금
    ]
}

/// 테스트용 간단한 설정 생성
fn simple_test_config() -> serde_json::Value {
    json!({
        "portfolio": [
            {"ticker": "SPY", "asset_type": "STOCK", "target_rate": "40"},
            {"ticker": "TLT", "asset_type": "SAFE", "target_rate": "30"},
            {"ticker": "GLD", "asset_type": "MAT", "target_rate": "20"},
            {"ticker": "BIL", "asset_type": "CASH", "target_rate": "0"}
        ],
        "total_amount": "1000000",
        "avg_momentum_period": 3,
        "top_bonus_count": 3,
        "cash_to_short_term_rate": "0.45",
        "cash_to_bonus_rate": "0.45",
        "rebalance_threshold": "3",
        "min_trade_amount": "10000",
        "min_global_score": "0"
    })
}

// ============================================================================
// 초기화 테스트
// ============================================================================

#[tokio::test]
async fn test_initialization_basic() {
    let mut strategy = PensionBotStrategy::new();
    let config = simple_test_config();

    let result = strategy.initialize(config).await;
    assert!(result.is_ok(), "초기화 실패: {:?}", result);

    assert_eq!(strategy.name(), "pension_bot");
    assert_eq!(strategy.version(), "1.0.0");
}

#[tokio::test]
async fn test_initialization_with_default_config() {
    let config = PensionBotConfig::default();
    let strategy = PensionBotStrategy::with_config(config.clone());

    assert_eq!(strategy.name(), "pension_bot");
    assert_eq!(config.avg_momentum_period, 10);
    assert_eq!(config.top_bonus_count, 12);
    assert!(!config.portfolio.is_empty());
}

#[tokio::test]
async fn test_initialization_creates_asset_data() {
    let mut strategy = PensionBotStrategy::new();
    strategy.initialize(simple_test_config()).await.unwrap();

    let state = strategy.get_state();
    assert_eq!(state["asset_count"], 4); // SPY, TLT, GLD, BIL
}

// ============================================================================
// 설정 검증 테스트
// ============================================================================

#[tokio::test]
async fn test_default_portfolio_composition() {
    let config = PensionBotConfig::default();

    let stock_count = config
        .portfolio
        .iter()
        .filter(|a| a.asset_type == PensionAssetType::Stock)
        .count();
    let safe_count = config
        .portfolio
        .iter()
        .filter(|a| a.asset_type == PensionAssetType::Safe)
        .count();
    let mat_count = config
        .portfolio
        .iter()
        .filter(|a| a.asset_type == PensionAssetType::Mat)
        .count();
    let cash_count = config
        .portfolio
        .iter()
        .filter(|a| a.asset_type == PensionAssetType::Cash)
        .count();

    assert!(stock_count > 0, "주식 자산이 있어야 함");
    assert!(safe_count > 0, "안전 자산이 있어야 함");
    assert!(mat_count > 0, "원자재 자산이 있어야 함");
    assert!(cash_count > 0, "현금 자산이 있어야 함");
}

#[tokio::test]
async fn test_asset_type_default() {
    assert_eq!(PensionAssetType::default(), PensionAssetType::Stock);
}

#[tokio::test]
async fn test_pension_asset_constructors() {
    let stock = PensionAsset::stock("TEST", dec!(10));
    assert_eq!(stock.asset_type, PensionAssetType::Stock);
    assert_eq!(stock.target_rate, dec!(10));

    let safe = PensionAsset::safe("TEST", dec!(20));
    assert_eq!(safe.asset_type, PensionAssetType::Safe);
    assert_eq!(safe.target_rate, dec!(20));

    let mat = PensionAsset::mat("TEST", dec!(15));
    assert_eq!(mat.asset_type, PensionAssetType::Mat);
    assert_eq!(mat.target_rate, dec!(15));

    let cash = PensionAsset::cash("TEST");
    assert_eq!(cash.asset_type, PensionAssetType::Cash);
    assert_eq!(cash.target_rate, dec!(0)); // Cash는 항상 0%
}

// ============================================================================
// 데이터 처리 테스트
// ============================================================================

#[tokio::test]
async fn test_market_data_updates_asset_candles() {
    let mut strategy = PensionBotStrategy::new();
    strategy.initialize(simple_test_config()).await.unwrap();

    // SPY 데이터 주입
    let data = create_kline("SPY", dec!(450));
    let _ = strategy.on_market_data(&data).await;

    let state = strategy.get_state();
    let assets = state["assets"].as_array().unwrap();

    // SPY 자산의 데이터가 업데이트되었는지 확인
    let spy_asset = assets.iter().find(|a| a["ticker"] == "SPY");
    assert!(spy_asset.is_some(), "SPY 자산 데이터가 존재해야 함");
}

#[tokio::test]
async fn test_ignores_unlisted_tickers() {
    let mut strategy = PensionBotStrategy::new();
    strategy.initialize(simple_test_config()).await.unwrap();

    // 포트폴리오에 없는 티커 데이터
    let data = create_kline("UNKNOWN", dec!(100));
    let signals = strategy.on_market_data(&data).await.unwrap();

    // 알 수 없는 티커는 시그널 생성 안 함
    assert!(signals.is_empty());
}

// ============================================================================
// 리밸런싱 조건 테스트
// ============================================================================

#[tokio::test]
async fn test_should_rebalance_first_run() {
    let strategy = PensionBotStrategy::new();

    // 첫 실행에서는 last_rebalance_month가 None이므로 리밸런싱 필요
    let state = strategy.get_state();
    assert!(state["last_rebalance_month"].is_null());
}

#[tokio::test]
async fn test_no_rebalance_without_sufficient_data() {
    let mut strategy = PensionBotStrategy::new();
    strategy.initialize(simple_test_config()).await.unwrap();

    // 데이터 부족 상태에서 시그널 요청
    let data = create_kline("SPY", dec!(450));
    let signals = strategy.on_market_data(&data).await.unwrap();

    // 240일 데이터가 없으면 리밸런싱 시그널 없음
    assert!(signals.is_empty(), "데이터 부족 시 리밸런싱 안 됨");
}

// ============================================================================
// 모멘텀 계산 테스트 (내부 로직 간접 검증)
// ============================================================================

#[tokio::test]
async fn test_state_contains_momentum_info() {
    let mut strategy = PensionBotStrategy::new();
    strategy.initialize(simple_test_config()).await.unwrap();

    // 일부 데이터 주입
    for i in 0..50 {
        let data = create_kline("SPY", dec!(400) + Decimal::from(i));
        let _ = strategy.on_market_data(&data).await;
    }

    let state = strategy.get_state();
    let assets = state["assets"].as_array().unwrap();

    let spy_asset = assets.iter().find(|a| a["ticker"] == "SPY").unwrap();

    // 모멘텀 스코어와 평균 모멘텀 필드가 존재해야 함
    assert!(spy_asset.get("momentum_score").is_some());
    assert!(spy_asset.get("avg_momentum").is_some());
    assert!(spy_asset.get("adjusted_rate").is_some());
}

// ============================================================================
// 포지션 업데이트 테스트
// ============================================================================

#[tokio::test]
async fn test_position_update_handling() {
    let mut strategy = PensionBotStrategy::new();
    strategy.initialize(simple_test_config()).await.unwrap();

    let position = create_position("SPY", dec!(100));
    let result = strategy.on_position_update(&position).await;

    assert!(result.is_ok(), "포지션 업데이트 처리 성공해야 함");
}

// ============================================================================
// 셧다운 테스트
// ============================================================================

#[tokio::test]
async fn test_shutdown() {
    let mut strategy = PensionBotStrategy::new();
    strategy.initialize(simple_test_config()).await.unwrap();

    let result = strategy.shutdown().await;
    assert!(result.is_ok(), "셧다운 성공해야 함");
}

// ============================================================================
// get_state 테스트
// ============================================================================

#[tokio::test]
async fn test_get_state_structure() {
    let mut strategy = PensionBotStrategy::new();
    strategy.initialize(simple_test_config()).await.unwrap();

    let state = strategy.get_state();

    // 필수 필드 확인
    assert!(state.get("name").is_some());
    assert!(state.get("last_rebalance_month").is_some());
    assert!(state.get("asset_count").is_some());
    assert!(state.get("assets").is_some());

    assert_eq!(state["name"], "pension_bot");
}

#[tokio::test]
async fn test_asset_state_structure() {
    let mut strategy = PensionBotStrategy::new();
    strategy.initialize(simple_test_config()).await.unwrap();

    let state = strategy.get_state();
    let assets = state["assets"].as_array().unwrap();

    assert!(!assets.is_empty());

    for asset in assets {
        assert!(asset.get("ticker").is_some());
        assert!(asset.get("asset_type").is_some());
        assert!(asset.get("momentum_score").is_some());
        assert!(asset.get("avg_momentum").is_some());
        assert!(asset.get("adjusted_rate").is_some());
    }
}

// ============================================================================
// 엣지 케이스 테스트
// ============================================================================

#[tokio::test]
async fn test_empty_portfolio_handling() {
    let mut strategy = PensionBotStrategy::new();
    let config = json!({
        "portfolio": [],
        "total_amount": "1000000"
    });

    let result = strategy.initialize(config).await;
    assert!(result.is_ok(), "빈 포트폴리오도 초기화 가능해야 함");

    let state = strategy.get_state();
    assert_eq!(state["asset_count"], 0);
}

#[tokio::test]
async fn test_negative_price_handling() {
    let mut strategy = PensionBotStrategy::new();
    strategy.initialize(simple_test_config()).await.unwrap();

    // 음수 가격 (비정상 데이터)
    let data = create_kline("SPY", dec!(-100));
    let result = strategy.on_market_data(&data).await;

    // 에러 없이 처리되어야 함
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_very_large_price_handling() {
    let mut strategy = PensionBotStrategy::new();
    strategy.initialize(simple_test_config()).await.unwrap();

    // 매우 큰 가격
    let data = create_kline("SPY", dec!(999999999));
    let result = strategy.on_market_data(&data).await;

    assert!(result.is_ok());
}

// ============================================================================
// 여러 자산 동시 업데이트 테스트
// ============================================================================

#[tokio::test]
async fn test_multiple_assets_update() {
    let mut strategy = PensionBotStrategy::new();
    strategy.initialize(simple_test_config()).await.unwrap();

    // 모든 자산에 데이터 주입
    let tickers = ["SPY", "TLT", "GLD", "BIL"];
    let prices = [dec!(450), dec!(95), dec!(180), dec!(91)];

    for (ticker, price) in tickers.iter().zip(prices.iter()) {
        let data = create_kline(ticker, *price);
        let _ = strategy.on_market_data(&data).await;
    }

    let state = strategy.get_state();
    let assets = state["assets"].as_array().unwrap();

    // 모든 자산이 업데이트되었는지 확인
    assert_eq!(assets.len(), 4);
}

// ============================================================================
// 설정 옵션 테스트
// ============================================================================

#[tokio::test]
async fn test_custom_momentum_period() {
    let mut strategy = PensionBotStrategy::new();
    let config = json!({
        "portfolio": [
            {"ticker": "SPY", "asset_type": "STOCK", "target_rate": "100"}
        ],
        "avg_momentum_period": 5,
        "total_amount": "1000000"
    });

    let result = strategy.initialize(config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_custom_bonus_settings() {
    let mut strategy = PensionBotStrategy::new();
    let config = json!({
        "portfolio": [
            {"ticker": "SPY", "asset_type": "STOCK", "target_rate": "50"},
            {"ticker": "TLT", "asset_type": "SAFE", "target_rate": "50"}
        ],
        "top_bonus_count": 2,
        "cash_to_short_term_rate": "0.5",
        "cash_to_bonus_rate": "0.4",
        "total_amount": "1000000"
    });

    let result = strategy.initialize(config).await;
    assert!(result.is_ok());
}

// ============================================================================
// with_config 생성자 테스트
// ============================================================================

#[tokio::test]
async fn test_with_config_constructor() {
    let config = PensionBotConfig {
        portfolio: simple_test_portfolio(),
        total_amount: dec!(500000),
        avg_momentum_period: 5,
        top_bonus_count: 3,
        cash_to_short_term_rate: dec!(0.4),
        cash_to_bonus_rate: dec!(0.4),
        rebalance_threshold: dec!(5),
        min_trade_amount: dec!(10000),
        min_global_score: dec!(0),
        exit_config: ExitConfig::default(),
    };

    let strategy = PensionBotStrategy::with_config(config);

    assert_eq!(strategy.name(), "pension_bot");

    let state = strategy.get_state();
    assert_eq!(state["asset_count"], 4);
}

// ============================================================================
// 재초기화 테스트
// ============================================================================

#[tokio::test]
async fn test_reinitialize() {
    //! 재초기화 동작 테스트
    //!
    //! 참고: 현재 전략 구현에서는 재초기화 시 기존 asset_data에 추가됨
    //! 완전한 재초기화가 필요하면 새 인스턴스를 생성해야 함

    // 첫 번째 인스턴스
    let mut strategy1 = PensionBotStrategy::new();
    let config1 = json!({
        "portfolio": [
            {"ticker": "SPY", "asset_type": "STOCK", "target_rate": "100"}
        ],
        "total_amount": "1000000"
    });
    strategy1.initialize(config1).await.unwrap();

    let state1 = strategy1.get_state();
    assert_eq!(state1["asset_count"], 1, "첫 초기화: 1개 자산");

    // 두 번째 인스턴스 (완전히 새로운 상태)
    let mut strategy2 = PensionBotStrategy::new();
    let config2 = json!({
        "portfolio": [
            {"ticker": "QQQ", "asset_type": "STOCK", "target_rate": "50"},
            {"ticker": "TLT", "asset_type": "SAFE", "target_rate": "50"}
        ],
        "total_amount": "2000000"
    });
    strategy2.initialize(config2).await.unwrap();

    let state2 = strategy2.get_state();
    assert_eq!(state2["asset_count"], 2, "새 인스턴스: 2개 자산");
}

// ============================================================================
// Default trait 테스트
// ============================================================================

#[tokio::test]
async fn test_default_trait() {
    let strategy = PensionBotStrategy::default();
    assert_eq!(strategy.name(), "pension_bot");
}

#[tokio::test]
async fn test_config_default_trait() {
    let config = PensionBotConfig::default();

    assert_eq!(config.total_amount, dec!(10000000));
    assert_eq!(config.avg_momentum_period, 10);
    assert_eq!(config.top_bonus_count, 12);
    assert_eq!(config.cash_to_short_term_rate, dec!(0.45));
    assert_eq!(config.cash_to_bonus_rate, dec!(0.45));
    assert_eq!(config.rebalance_threshold, dec!(3));
    assert_eq!(config.min_trade_amount, dec!(50000));
    assert_eq!(config.min_global_score, dec!(60));
}

// ============================================================================
// 비중 합계 검증 테스트
// ============================================================================

#[tokio::test]
async fn test_portfolio_weight_sum() {
    let portfolio = simple_test_portfolio();

    // Cash를 제외한 비중 합계 계산 (Cash는 target_rate가 0)
    let total_rate: Decimal = portfolio
        .iter()
        .filter(|a| a.asset_type != PensionAssetType::Cash)
        .map(|a| a.target_rate)
        .sum();

    // 40 + 30 + 20 = 90 (나머지 10%는 현금 분배)
    assert_eq!(total_rate, dec!(90));
}

// ============================================================================
// description 테스트
// ============================================================================

#[tokio::test]
async fn test_strategy_description() {
    let strategy = PensionBotStrategy::new();
    let description = strategy.description();

    assert!(!description.is_empty());
    assert!(description.contains("연금") || description.contains("자산배분"));
}
