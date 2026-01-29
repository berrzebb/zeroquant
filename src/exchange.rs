//! Exchange module for supporting multiple exchanges

use crate::types::{Order, Trade, MarketData};
use anyhow::Result;
use async_trait::async_trait;

/// Supported exchange types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeType {
    Binance,
    Coinbase,
    Kraken,
    Simulation,
}

/// Trait that all exchanges must implement
#[async_trait]
pub trait Exchange: Send + Sync {
    /// Get the exchange type
    fn exchange_type(&self) -> ExchangeType;

    /// Place an order on the exchange
    async fn place_order(&mut self, order: Order) -> Result<Trade>;

    /// Get current market data
    async fn get_market_data(&self, symbol: &str) -> Result<MarketData>;

    /// Get account balance
    async fn get_balance(&self, asset: &str) -> Result<f64>;
}

/// Simulated exchange for backtesting and simulation
pub struct SimulatedExchange {
    exchange_type: ExchangeType,
    balances: std::collections::HashMap<String, f64>,
}

impl SimulatedExchange {
    pub fn new() -> Self {
        let mut balances = std::collections::HashMap::new();
        balances.insert("USD".to_string(), 10000.0); // Start with 10k USD
        
        Self {
            exchange_type: ExchangeType::Simulation,
            balances,
        }
    }

    pub fn set_balance(&mut self, asset: &str, amount: f64) {
        self.balances.insert(asset.to_string(), amount);
    }
}

impl Default for SimulatedExchange {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Exchange for SimulatedExchange {
    fn exchange_type(&self) -> ExchangeType {
        self.exchange_type
    }

    async fn place_order(&mut self, order: Order) -> Result<Trade> {
        // Simple simulation: execute at current price
        let price = order.price.unwrap_or(100.0); // Default price for market orders
        let commission = order.quantity * price * 0.001; // 0.1% commission

        let trade = Trade {
            id: format!("trade_{}", uuid::Uuid::new_v4()),
            order_id: order.id.clone(),
            symbol: order.symbol.clone(),
            side: order.side,
            quantity: order.quantity,
            price,
            timestamp: order.timestamp,
            commission,
        };

        Ok(trade)
    }

    async fn get_market_data(&self, _symbol: &str) -> Result<MarketData> {
        // Return mock data
        Ok(MarketData {
            symbol: _symbol.to_string(),
            timestamp: chrono::Utc::now(),
            open: 100.0,
            high: 105.0,
            low: 95.0,
            close: 102.0,
            volume: 1000.0,
        })
    }

    async fn get_balance(&self, asset: &str) -> Result<f64> {
        Ok(*self.balances.get(asset).unwrap_or(&0.0))
    }
}
