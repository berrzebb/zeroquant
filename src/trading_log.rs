//! Trading log for recording and analyzing trades

use crate::types::{Trade, OrderSide};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use anyhow::Result;

/// Trading log to track all trades
#[derive(Debug, Clone, Default)]
pub struct TradingLog {
    trades: Vec<Trade>,
}

impl TradingLog {
    pub fn new() -> Self {
        Self {
            trades: Vec::new(),
        }
    }

    /// Record a trade
    pub fn record_trade(&mut self, trade: Trade) {
        self.trades.push(trade);
    }

    /// Get all trades
    pub fn get_all_trades(&self) -> Vec<Trade> {
        self.trades.clone()
    }

    /// Get trades for a specific symbol
    pub fn get_trades_by_symbol(&self, symbol: &str) -> Vec<Trade> {
        self.trades
            .iter()
            .filter(|t| t.symbol == symbol)
            .cloned()
            .collect()
    }

    /// Get buy trades
    pub fn get_buy_trades(&self) -> Vec<Trade> {
        self.trades
            .iter()
            .filter(|t| t.side == OrderSide::Buy)
            .cloned()
            .collect()
    }

    /// Get sell trades
    pub fn get_sell_trades(&self) -> Vec<Trade> {
        self.trades
            .iter()
            .filter(|t| t.side == OrderSide::Sell)
            .cloned()
            .collect()
    }

    /// Calculate total profit/loss
    pub fn calculate_pnl(&self) -> f64 {
        let mut pnl = 0.0;
        for trade in &self.trades {
            match trade.side {
                OrderSide::Buy => pnl -= trade.price * trade.quantity + trade.commission,
                OrderSide::Sell => pnl += trade.price * trade.quantity - trade.commission,
            }
        }
        pnl
    }

    /// Get trade statistics
    pub fn get_statistics(&self) -> TradeStatistics {
        let total_trades = self.trades.len();
        let buy_trades = self.get_buy_trades().len();
        let sell_trades = self.get_sell_trades().len();
        let total_pnl = self.calculate_pnl();
        let total_commission: f64 = self.trades.iter().map(|t| t.commission).sum();

        TradeStatistics {
            total_trades,
            buy_trades,
            sell_trades,
            total_pnl,
            total_commission,
        }
    }

    /// Export trades to JSON file
    pub fn export_to_json(&self, filename: &str) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.trades)?;
        let mut file = File::create(filename)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    /// Clear all trades
    pub fn clear(&mut self) {
        self.trades.clear();
    }

    /// Print summary
    pub fn print_summary(&self) {
        let stats = self.get_statistics();
        println!("=== Trading Log Summary ===");
        println!("Total Trades: {}", stats.total_trades);
        println!("Buy Trades: {}", stats.buy_trades);
        println!("Sell Trades: {}", stats.sell_trades);
        println!("Total P&L: ${:.2}", stats.total_pnl);
        println!("Total Commission: ${:.2}", stats.total_commission);
    }
}

/// Statistics about trades
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeStatistics {
    pub total_trades: usize,
    pub buy_trades: usize,
    pub sell_trades: usize,
    pub total_pnl: f64,
    pub total_commission: f64,
}
