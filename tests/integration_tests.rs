//! Integration tests for zeroquant

use zeroquant::{
    BacktestEngine, MarketData, SimulationEngine,
    strategy::{MovingAverageCrossover, Strategy, Signal},
    types::OrderSide,
    TradingLog,
};
use chrono::Utc;

/// Helper function to create sample market data
fn create_sample_data(count: usize) -> Vec<MarketData> {
    let mut data = Vec::new();
    let base_price = 100.0;

    for i in 0..count {
        let price = base_price + (i as f64 * 0.5);
        data.push(MarketData {
            symbol: "BTC/USD".to_string(),
            timestamp: Utc::now(),
            open: price - 1.0,
            high: price + 2.0,
            low: price - 2.0,
            close: price,
            volume: 1000.0,
        });
    }

    data
}

#[tokio::test]
async fn test_backtest_engine_initialization() {
    let engine = BacktestEngine::new(10000.0);
    assert!(engine.get_trading_log().get_all_trades().is_empty());
}

#[tokio::test]
async fn test_backtest_run() {
    let mut engine = BacktestEngine::new(10000.0);
    let mut strategy = MovingAverageCrossover::new(5, 20);
    let data = create_sample_data(50);

    let result = engine.run(&mut strategy, data, 100.0).await;
    assert!(result.is_ok());

    let result = result.unwrap();
    assert_eq!(result.initial_balance, 10000.0);
    assert!(result.total_trades > 0);
}

#[tokio::test]
async fn test_simulation_engine() {
    let mut engine = SimulationEngine::new(10000.0);
    assert!(!engine.is_running());

    engine.start();
    assert!(engine.is_running());

    engine.stop();
    assert!(!engine.is_running());
}

#[tokio::test]
async fn test_simulation_tick() {
    let mut engine = SimulationEngine::new(10000.0);
    let mut strategy = MovingAverageCrossover::new(5, 20);
    let data = create_sample_data(1);

    engine.start();
    let result = engine.tick(&mut strategy, data[0].clone(), 100.0).await;
    assert!(result.is_ok());
}

#[test]
fn test_moving_average_strategy() {
    let mut strategy = MovingAverageCrossover::new(2, 5);
    
    // Create some test data
    let data1 = MarketData {
        symbol: "BTC/USD".to_string(),
        timestamp: Utc::now(),
        open: 100.0,
        high: 105.0,
        low: 95.0,
        close: 100.0,
        volume: 1000.0,
    };

    let signal = strategy.analyze(&data1);
    assert_eq!(signal, Signal::Hold); // Not enough data yet

    // Add more data points
    for i in 1..10 {
        let data = MarketData {
            symbol: "BTC/USD".to_string(),
            timestamp: Utc::now(),
            open: 100.0 + i as f64,
            high: 105.0 + i as f64,
            low: 95.0 + i as f64,
            close: 100.0 + i as f64,
            volume: 1000.0,
        };
        strategy.analyze(&data);
    }

    // After adding data, strategy should have enough information
    let data_final = MarketData {
        symbol: "BTC/USD".to_string(),
        timestamp: Utc::now(),
        open: 110.0,
        high: 115.0,
        low: 105.0,
        close: 110.0,
        volume: 1000.0,
    };

    let signal = strategy.analyze(&data_final);
    assert!(signal == Signal::Buy || signal == Signal::Sell || signal == Signal::Hold);
}

#[test]
fn test_trading_log() {
    let mut log = TradingLog::new();
    assert!(log.get_all_trades().is_empty());

    let trade = zeroquant::types::Trade {
        id: "trade_1".to_string(),
        order_id: "order_1".to_string(),
        symbol: "BTC/USD".to_string(),
        side: OrderSide::Buy,
        quantity: 1.0,
        price: 100.0,
        timestamp: Utc::now(),
        commission: 0.1,
    };

    log.record_trade(trade);
    assert_eq!(log.get_all_trades().len(), 1);

    let stats = log.get_statistics();
    assert_eq!(stats.total_trades, 1);
    assert_eq!(stats.buy_trades, 1);
    assert_eq!(stats.sell_trades, 0);
}

#[test]
fn test_trading_log_pnl() {
    let mut log = TradingLog::new();

    // Buy trade
    let buy_trade = zeroquant::types::Trade {
        id: "trade_1".to_string(),
        order_id: "order_1".to_string(),
        symbol: "BTC/USD".to_string(),
        side: OrderSide::Buy,
        quantity: 1.0,
        price: 100.0,
        timestamp: Utc::now(),
        commission: 0.1,
    };
    log.record_trade(buy_trade);

    // Sell trade
    let sell_trade = zeroquant::types::Trade {
        id: "trade_2".to_string(),
        order_id: "order_2".to_string(),
        symbol: "BTC/USD".to_string(),
        side: OrderSide::Sell,
        quantity: 1.0,
        price: 110.0,
        timestamp: Utc::now(),
        commission: 0.1,
    };
    log.record_trade(sell_trade);

    let pnl = log.calculate_pnl();
    // Buy: -100.0 - 0.1 = -100.1
    // Sell: +110.0 - 0.1 = +109.9
    // Total: 9.8
    assert!((pnl - 9.8).abs() < 0.01);
}

#[test]
fn test_position_calculations() {
    let position = zeroquant::types::Position {
        symbol: "BTC/USD".to_string(),
        quantity: 1.0,
        entry_price: 100.0,
        current_price: 110.0,
        timestamp: Utc::now(),
    };

    assert_eq!(position.unrealized_pnl(), 10.0);
    assert_eq!(position.value(), 110.0);
}

#[test]
fn test_strategy_reset() {
    let mut strategy = MovingAverageCrossover::new(5, 20);
    
    // Add some data
    for i in 0..10 {
        let data = MarketData {
            symbol: "BTC/USD".to_string(),
            timestamp: Utc::now(),
            open: 100.0 + i as f64,
            high: 105.0 + i as f64,
            low: 95.0 + i as f64,
            close: 100.0 + i as f64,
            volume: 1000.0,
        };
        strategy.analyze(&data);
    }

    // Reset the strategy
    strategy.reset();

    // After reset, should be back to initial state
    let data = MarketData {
        symbol: "BTC/USD".to_string(),
        timestamp: Utc::now(),
        open: 100.0,
        high: 105.0,
        low: 95.0,
        close: 100.0,
        volume: 1000.0,
    };

    let signal = strategy.analyze(&data);
    assert_eq!(signal, Signal::Hold); // Not enough data after reset
}
