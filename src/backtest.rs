//! Backtesting engine for testing strategies on historical data

use crate::{
    exchange::{Exchange, SimulatedExchange},
    strategy::{create_order_from_signal, Strategy},
    trading_log::TradingLog,
    types::{MarketData, Trade},
};
use anyhow::Result;

/// Backtesting engine
pub struct BacktestEngine {
    exchange: SimulatedExchange,
    trading_log: TradingLog,
    initial_balance: f64,
}

impl BacktestEngine {
    pub fn new(initial_balance: f64) -> Self {
        let mut exchange = SimulatedExchange::new();
        exchange.set_balance("USD", initial_balance);

        Self {
            exchange,
            trading_log: TradingLog::new(),
            initial_balance,
        }
    }

    /// Run backtest with historical data and a strategy
    pub async fn run(
        &mut self,
        strategy: &mut dyn Strategy,
        historical_data: Vec<MarketData>,
        position_size: f64,
    ) -> Result<BacktestResult> {
        strategy.reset();
        self.trading_log.clear();

        for market_data in historical_data {
            let signal = strategy.analyze(&market_data);

            if let Some(order) = create_order_from_signal(
                signal,
                &market_data.symbol,
                position_size,
                Some(market_data.close),
            ) {
                match self.exchange.place_order(order).await {
                    Ok(trade) => {
                        self.trading_log.record_trade(trade);
                    }
                    Err(e) => {
                        eprintln!("Failed to execute order: {}", e);
                    }
                }
            }
        }

        let final_balance = self.exchange.get_balance("USD").await?;
        let trades = self.trading_log.get_all_trades();
        
        Ok(BacktestResult {
            initial_balance: self.initial_balance,
            final_balance,
            total_trades: trades.len(),
            profit_loss: final_balance - self.initial_balance,
            trades,
        })
    }

    /// Get the trading log
    pub fn get_trading_log(&self) -> &TradingLog {
        &self.trading_log
    }
}

/// Result of a backtest
#[derive(Debug, Clone)]
pub struct BacktestResult {
    pub initial_balance: f64,
    pub final_balance: f64,
    pub total_trades: usize,
    pub profit_loss: f64,
    pub trades: Vec<Trade>,
}

impl BacktestResult {
    /// Calculate return percentage
    pub fn return_percentage(&self) -> f64 {
        (self.profit_loss / self.initial_balance) * 100.0
    }

    /// Print summary
    pub fn print_summary(&self) {
        println!("=== Backtest Results ===");
        println!("Initial Balance: ${:.2}", self.initial_balance);
        println!("Final Balance: ${:.2}", self.final_balance);
        println!("Profit/Loss: ${:.2}", self.profit_loss);
        println!("Return: {:.2}%", self.return_percentage());
        println!("Total Trades: {}", self.total_trades);
    }
}
