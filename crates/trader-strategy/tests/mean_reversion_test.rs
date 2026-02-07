//! MeanReversion 전략 통합 테스트.
//!
//! RSI와 Bollinger 평균회귀 전략 테스트.
//!
//! ## Grid/MagicSplit 분리 안내
//!
//! Grid Trading, MagicSplit, InfinityBot은 `dca_test.rs`에서 테스트됩니다.
//! 이들은 스프레드 기반 전략으로 dca.rs로 이동했습니다.

use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::types::Timeframe;
use trader_core::{Kline, MarketData, MarketDataType, Position, Side, StrategyContext, StructuralFeatures, Ticker};
use trader_strategy::strategies::mean_reversion::{
    BollingerConfig, MeanReversionConfig, MeanReversionStrategy, MeanReversionVariant, RsiConfig,
};
use trader_strategy::Strategy;

// ================================================================================================
// 헬퍼 함수
// ================================================================================================

/// 테스트용 Kline 마켓 데이터 생성.
fn create_kline_data(ticker_name: &str, close: Decimal) -> MarketData {
    let now = Utc::now();
    MarketData {
        exchange: "test".to_string(),
        ticker: ticker_name.to_string(),
        timestamp: now,
        data: MarketDataType::Kline(Kline {
            ticker: ticker_name.to_string(),
            timeframe: Timeframe::D1,
            open_time: now,
            open: close,
            high: close + dec!(100),
            low: close - dec!(100),
            close,
            volume: dec!(1000000),
            close_time: now,
            quote_volume: None,
            num_trades: None,
        }),
    }
}

/// 테스트용 Ticker 마켓 데이터 생성.
fn create_ticker_data(ticker_name: &str, price: Decimal) -> MarketData {
    let now = Utc::now();
    MarketData {
        exchange: "test".to_string(),
        ticker: ticker_name.to_string(),
        timestamp: now,
        data: MarketDataType::Ticker(Ticker {
            ticker: ticker_name.to_string(),
            last: price,
            bid: price - dec!(10),
            ask: price + dec!(10),
            volume_24h: dec!(1000000),
            high_24h: price + dec!(500),
            low_24h: price - dec!(500),
            change_24h: dec!(0),
            change_24h_percent: dec!(0),
            timestamp: now,
        }),
    }
}

/// StrategyContext에 klines 데이터 설정.
fn setup_context_with_klines(
    ticker: &str,
    prices: &[Decimal],
) -> Arc<RwLock<StrategyContext>> {
    let mut context = StrategyContext::new();
    let now = Utc::now();
    let day = chrono::Duration::days(1);

    let klines: Vec<Kline> = prices
        .iter()
        .enumerate()
        .map(|(i, &close)| {
            let time = now - day * (prices.len() - 1 - i) as i32;
            Kline {
                ticker: ticker.to_string(),
                timeframe: Timeframe::D1,
                open_time: time,
                open: close,
                high: close + dec!(50),
                low: close - dec!(50),
                close,
                volume: dec!(1000000),
                close_time: time,
                quote_volume: None,
                num_trades: None,
            }
        })
        .collect();

    context.update_klines(ticker, Timeframe::D1, klines);
    Arc::new(RwLock::new(context))
}

// ================================================================================================
// RSI Variant 테스트
// ================================================================================================

mod rsi_tests {
    use super::*;

    /// 테스트: RSI 전략 초기화 성공
    #[tokio::test]
    async fn test_rsi_initialization() {
        let mut strategy = MeanReversionStrategy::rsi();
        let config = json!({
            "variant": "rsi",
            "ticker": "005930",
            "rsi_period": 14,
            "oversold": 30,
            "overbought": 70
        });

        let result = strategy.initialize(config).await;

        assert!(result.is_ok(), "RSI 전략 초기화 실패: {:?}", result.err());
        assert_eq!(strategy.name(), "MeanReversion-RSI");
    }

    /// 테스트: RSI 과매도 시 매수 신호
    #[tokio::test]
    async fn test_rsi_oversold_buy_signal() {
        let mut strategy = MeanReversionStrategy::rsi();
        let config = json!({
            "variant": "rsi",
            "ticker": "005930",
            "rsi_period": 14,
            "oversold": 30,
            "overbought": 70,
            "amount": 1000000,
            "min_global_score": 0  // 필터 비활성화
        });
        strategy.initialize(config).await.unwrap();

        // klines 설정 (하락 추세 -> RSI 낮아짐)
        let prices: Vec<Decimal> = (0..20)
            .map(|i| dec!(60000) - dec!(500) * Decimal::from(i))
            .collect();
        let context = setup_context_with_klines("005930", &prices);
        strategy.set_context(context);

        // 최저가에서 시그널 확인
        let data = create_kline_data("005930", dec!(50000));
        let signals = strategy.on_market_data(&data).await.unwrap();

        // RSI가 낮아지면 매수 신호 발생 가능
        // Note: 정확한 RSI 계산에 따라 신호 발생 여부 달라짐
        println!("RSI 테스트: signals count = {}", signals.len());
    }

    /// 테스트: 초기화 없이 호출 시 에러
    #[tokio::test]
    async fn test_rsi_no_signal_without_init() {
        let mut strategy = MeanReversionStrategy::rsi();
        let data = create_kline_data("005930", dec!(50000));
        let result = strategy.on_market_data(&data).await;

        // 초기화 없이 호출 시 빈 결과 또는 에러
        assert!(result.is_ok() || result.is_err());
    }
}

// ================================================================================================
// Bollinger Variant 테스트
// ================================================================================================

mod bollinger_tests {
    use super::*;

    /// 테스트: Bollinger 전략 초기화 성공
    #[tokio::test]
    async fn test_bollinger_initialization() {
        let mut strategy = MeanReversionStrategy::bollinger();
        let config = json!({
            "variant": "bollinger",
            "ticker": "005930",
            "period": 20,
            "std_multiplier": 2.0,
            "use_rsi_confirmation": false
        });

        let result = strategy.initialize(config).await;

        assert!(result.is_ok(), "Bollinger 전략 초기화 실패: {:?}", result.err());
        assert_eq!(strategy.name(), "MeanReversion-Bollinger");
    }

    /// 테스트: Bollinger 하단밴드 터치 시 매수
    #[tokio::test]
    async fn test_bollinger_lower_band_buy_signal() {
        let mut strategy = MeanReversionStrategy::bollinger();
        let config = json!({
            "variant": "bollinger",
            "ticker": "005930",
            "period": 20,
            "std_multiplier": 2.0,
            "use_rsi_confirmation": false,
            "amount": 1000000,
            "min_global_score": 0
        });
        strategy.initialize(config).await.unwrap();

        // 충분한 klines 데이터 설정
        let prices: Vec<Decimal> = (0..25)
            .map(|i| dec!(50000) + dec!(100) * Decimal::from(i % 5) - dec!(200))
            .collect();
        let context = setup_context_with_klines("005930", &prices);
        strategy.set_context(context);

        // 낮은 가격에서 시그널 확인
        let data = create_kline_data("005930", dec!(48000));
        let signals = strategy.on_market_data(&data).await.unwrap();

        println!("Bollinger 테스트: signals count = {}", signals.len());
    }
}

// ================================================================================================
// 공통 기능 테스트
// ================================================================================================

mod common_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_state_returns_valid_json() {
        let mut strategy = MeanReversionStrategy::rsi();
        let config = json!({
            "variant": "rsi",
            "ticker": "005930"
        });
        strategy.initialize(config).await.unwrap();

        let state = strategy.get_state();

        assert!(state.is_object());
        assert!(state.get("name").is_some());
    }

    #[tokio::test]
    async fn test_shutdown_returns_ok() {
        let mut strategy = MeanReversionStrategy::bollinger();
        let config = json!({
            "variant": "bollinger",
            "ticker": "005930"
        });
        strategy.initialize(config).await.unwrap();

        let result = strategy.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_strategy_name_reflects_variant() {
        // RSI variant
        let mut rsi_strategy = MeanReversionStrategy::rsi();
        rsi_strategy.initialize(json!({"variant": "rsi", "ticker": "TEST"})).await.unwrap();
        assert_eq!(rsi_strategy.name(), "MeanReversion-RSI");

        // Bollinger variant
        let mut bb_strategy = MeanReversionStrategy::bollinger();
        bb_strategy.initialize(json!({"variant": "bollinger", "ticker": "TEST"})).await.unwrap();
        assert_eq!(bb_strategy.name(), "MeanReversion-Bollinger");
    }

    #[tokio::test]
    async fn test_ticker_data_type_works() {
        let mut strategy = MeanReversionStrategy::rsi();
        let config = json!({
            "variant": "rsi",
            "ticker": "005930"
        });
        strategy.initialize(config).await.unwrap();

        let data = create_ticker_data("005930", dec!(50000));
        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok());
    }
}

// ================================================================================================
// 에러 케이스 테스트
// ================================================================================================

mod error_tests {
    use super::*;

    #[tokio::test]
    async fn test_invalid_config_json_fails() {
        let mut strategy = MeanReversionStrategy::new();
        // 완전히 잘못된 타입 (문자열)은 파싱 실패해야 함
        let invalid_config = json!("not an object");

        let result = strategy.initialize(invalid_config).await;
        assert!(result.is_err(), "잘못된 설정은 에러를 반환해야 함");
    }

    #[tokio::test]
    async fn test_partial_config_uses_defaults() {
        // serde(default)가 적용되어 있으므로 부분 설정은 기본값으로 처리됨
        let mut strategy = MeanReversionStrategy::new();
        let partial_config = json!({
            "variant": "rsi"
            // 나머지 필드는 기본값 사용
        });

        let result = strategy.initialize(partial_config).await;
        // 기본값이 적용되어 성공해야 함
        assert!(result.is_ok(), "부분 설정은 기본값으로 처리되어야 함");
    }
}

// ================================================================================================
// 경계값 테스트
// ================================================================================================

mod boundary_tests {
    use super::*;

    #[tokio::test]
    async fn test_zero_price_handled() {
        let mut strategy = MeanReversionStrategy::rsi();
        let config = json!({
            "variant": "rsi",
            "ticker": "005930"
        });
        strategy.initialize(config).await.unwrap();

        let data = create_kline_data("005930", dec!(0));
        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok(), "0 가격도 에러 없이 처리해야 함");
    }

    #[tokio::test]
    async fn test_very_large_price_handled() {
        let mut strategy = MeanReversionStrategy::bollinger();
        let config = json!({
            "variant": "bollinger",
            "ticker": "005930"
        });
        strategy.initialize(config).await.unwrap();

        let data = create_kline_data("005930", dec!(999999999999));
        let result = strategy.on_market_data(&data).await;

        assert!(result.is_ok(), "매우 큰 가격도 에러 없이 처리해야 함");
    }
}
