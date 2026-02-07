//! Utility modules for the trader-api crate.
//!
//! This module contains helper functions and utilities used across the API:
//!
//! - [`format`]: Formatting functions for timestamps, decimals, and currency values
//! - [`serde_helpers`]: Custom serde serialization/deserialization helpers

pub mod format;
pub mod serde_helpers;

pub use format::{
    format_currency, format_decimal, format_percentage, format_timestamp, format_timestamp_opt,
};
pub use serde_helpers::{
    deserialize_bool_from_string, deserialize_decimal_from_string,
    deserialize_decimal_opt_from_string, deserialize_symbol, serialize_decimal_as_string,
};
