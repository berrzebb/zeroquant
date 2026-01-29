//! ZeroQuant - Quantitative Trading Bot
//! 
//! Example usage of the zeroquant library

use zeroquant::{
    BacktestEngine, MarketData, SimulationEngine,
    strategy::MovingAverageCrossover,
};
use chrono::Utc;

#[tokio::main]
async fn main() {
    println!("=== ZeroQuant - Quantitative Trading Bot ===\n");

    // Create sample historical data
    let historical_data = create_sample_data();

    // Example 1: Backtesting
    println!("Running backtest...\n");
    run_backtest_example(historical_data.clone()).await;

    println!("\n{}\n", "=".repeat(50));

    // Example 2: Simulation
    println!("Running simulation...\n");
    run_simulation_example(historical_data).await;
}

async fn run_backtest_example(historical_data: Vec<MarketData>) {
    // Create a backtest engine with $10,000 initial balance
    let mut engine = BacktestEngine::new(10000.0);

    // Create a moving average crossover strategy (5 and 20 period)
    let mut strategy = MovingAverageCrossover::new(5, 20);

    // Run the backtest
    match engine.run(&mut strategy, historical_data, 100.0).await {
        Ok(result) => {
            result.print_summary();
            println!("\nTrading Log:");
            engine.get_trading_log().print_summary();
        }
        Err(e) => eprintln!("Backtest failed: {}", e),
    }
}

async fn run_simulation_example(historical_data: Vec<MarketData>) {
    // Create a simulation engine with $10,000 initial balance
    let mut engine = SimulationEngine::new(10000.0);

    // Create a strategy
    let mut strategy = MovingAverageCrossover::new(5, 20);

    // Start the simulation
    engine.start();

    println!("Processing market data ticks...");
    
    // Process each market data point
    for (i, data) in historical_data.iter().enumerate() {
        if let Err(e) = engine.tick(&mut strategy, data.clone(), 100.0).await {
            eprintln!("Error processing tick {}: {}", i, e);
        }
    }

    // Stop the simulation
    engine.stop();

    // Print results
    println!("\nSimulation completed!");
    match engine.get_balance("USD").await {
        Ok(balance) => println!("Final Balance: ${:.2}", balance),
        Err(e) => eprintln!("Error getting balance: {}", e),
    }

    println!("\nTrading Log:");
    engine.get_trading_log().print_summary();
}

/// Create sample market data for demonstration
fn create_sample_data() -> Vec<MarketData> {
    let mut data = Vec::new();
    let base_price = 100.0;

    // Generate 100 data points with trending price
    for i in 0..100 {
        let price = base_price + (i as f64 * 0.5) + (i as f64 % 10 as f64 - 5.0);
        data.push(MarketData {
            symbol: "BTC/USD".to_string(),
            timestamp: Utc::now(),
            open: price - 1.0,
            high: price + 2.0,
            low: price - 2.0,
            close: price,
            volume: 1000.0 + (i as f64 * 10.0),
        });
    }

    data
}
