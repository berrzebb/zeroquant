//! # ZeroQuant
//!
//! A Rust-based quantitative trading bot that supports:
//! - Multiple exchanges
//! - Backtesting
//! - Simulation
//! - Trading logs
//! - Custom strategies

pub mod exchange;
pub mod strategy;
pub mod backtest;
pub mod simulation;
pub mod trading_log;
pub mod types;

pub use exchange::{Exchange, ExchangeType};
pub use strategy::{Strategy, Signal};
pub use backtest::BacktestEngine;
pub use simulation::SimulationEngine;
pub use trading_log::TradingLog;
pub use types::{Order, Trade, Position, MarketData};
