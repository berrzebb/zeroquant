# ZeroQuant

A Rust-based quantitative trading bot that supports multiple exchanges with backtesting, simulation, trading logs, and custom strategies.

## Features

- **Multiple Exchange Support**: Extensible exchange interface for connecting to various exchanges (Binance, Coinbase, Kraken, etc.)
- **Backtesting**: Test your trading strategies on historical data
- **Simulation**: Real-time simulation mode for strategy validation
- **Trading Logs**: Comprehensive logging and analysis of all trades
- **Strategy Framework**: Flexible strategy interface with built-in examples

## Quick Start

### Build the Project

```bash
cargo build --release
```

### Run Examples

```bash
cargo run
```

## Architecture

### Core Components

1. **Exchange Module** (`src/exchange.rs`)
   - Trait-based interface for exchange implementations
   - Simulated exchange for backtesting and simulation
   - Support for multiple exchange types

2. **Strategy Module** (`src/strategy.rs`)
   - Strategy trait for implementing custom strategies
   - Built-in Moving Average Crossover strategy
   - Signal generation (Buy/Sell/Hold)

3. **Backtest Engine** (`src/backtest.rs`)
   - Run strategies on historical data
   - Performance metrics and analysis
   - Detailed results reporting

4. **Simulation Engine** (`src/simulation.rs`)
   - Real-time strategy testing
   - Tick-by-tick market data processing
   - Live position tracking

5. **Trading Log** (`src/trading_log.rs`)
   - Record all trades
   - Calculate P&L and statistics
   - Export to JSON

## Usage Examples

### Backtesting

```rust
use zeroquant::{BacktestEngine, strategy::MovingAverageCrossover};

#[tokio::main]
async fn main() {
    let mut engine = BacktestEngine::new(10000.0);
    let mut strategy = MovingAverageCrossover::new(5, 20);
    
    let result = engine.run(&mut strategy, historical_data, 100.0).await?;
    result.print_summary();
}
```

### Creating a Custom Strategy

```rust
use zeroquant::{Strategy, Signal, MarketData};

struct MyStrategy;

impl Strategy for MyStrategy {
    fn analyze(&mut self, data: &MarketData) -> Signal {
        // Your strategy logic here
        if data.close > 100.0 {
            Signal::Buy
        } else {
            Signal::Sell
        }
    }

    fn name(&self) -> &str {
        "MyStrategy"
    }

    fn reset(&mut self) {
        // Reset strategy state
    }
}
```

## Development

### Testing

```bash
cargo test
```

### Documentation

```bash
cargo doc --open
```

## License

MIT License - See [LICENSE](LICENSE) file for details