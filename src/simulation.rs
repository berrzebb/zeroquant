//! Simulation engine for real-time strategy testing

use crate::{
    exchange::{Exchange, SimulatedExchange},
    strategy::{create_order_from_signal, Strategy},
    trading_log::TradingLog,
    types::MarketData,
};
use anyhow::Result;

/// Simulation engine for real-time testing
pub struct SimulationEngine {
    exchange: SimulatedExchange,
    trading_log: TradingLog,
    is_running: bool,
}

impl SimulationEngine {
    pub fn new(initial_balance: f64) -> Self {
        let mut exchange = SimulatedExchange::new();
        exchange.set_balance("USD", initial_balance);

        Self {
            exchange,
            trading_log: TradingLog::new(),
            is_running: false,
        }
    }

    /// Start simulation
    pub fn start(&mut self) {
        self.is_running = true;
        println!("Simulation started");
    }

    /// Stop simulation
    pub fn stop(&mut self) {
        self.is_running = false;
        println!("Simulation stopped");
    }

    /// Check if simulation is running
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Process a single tick of market data
    pub async fn tick(
        &mut self,
        strategy: &mut dyn Strategy,
        market_data: MarketData,
        position_size: f64,
    ) -> Result<()> {
        if !self.is_running {
            return Ok(());
        }

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
                    println!("Trade executed at price: {}", market_data.close);
                }
                Err(e) => {
                    eprintln!("Failed to execute order: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Get current balance
    pub async fn get_balance(&self, asset: &str) -> Result<f64> {
        self.exchange.get_balance(asset).await
    }

    /// Get the trading log
    pub fn get_trading_log(&self) -> &TradingLog {
        &self.trading_log
    }

    /// Reset simulation
    pub fn reset(&mut self, initial_balance: f64) {
        self.exchange.set_balance("USD", initial_balance);
        self.trading_log.clear();
        self.is_running = false;
    }
}
