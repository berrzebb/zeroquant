//! Formatting utilities for timestamps, decimals, and currency values.
//!
//! This module provides consistent formatting functions used throughout the API
//! for serializing values to human-readable and machine-parseable strings.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// Formats a UTC datetime to RFC3339 format.
///
/// # Arguments
///
/// * `dt` - The datetime to format
///
/// # Returns
///
/// A string in RFC3339 format (e.g., "2024-01-15T10:30:00+00:00")
///
/// # Examples
///
/// ```
/// use chrono::{DateTime, Utc};
/// use trader_api::utils::format_timestamp;
///
/// let dt = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
///     .unwrap()
///     .with_timezone(&Utc);
/// assert_eq!(format_timestamp(&dt), "2024-01-15T10:30:00+00:00");
/// ```
#[inline]
pub fn format_timestamp(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

/// Formats an optional UTC datetime to RFC3339 format.
///
/// # Arguments
///
/// * `dt` - The optional datetime to format
///
/// # Returns
///
/// `Some(String)` in RFC3339 format if the input is `Some`, otherwise `None`
///
/// # Examples
///
/// ```
/// use chrono::{DateTime, Utc};
/// use trader_api::utils::format_timestamp_opt;
///
/// let dt = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
///     .unwrap()
///     .with_timezone(&Utc);
/// assert_eq!(format_timestamp_opt(Some(&dt)), Some("2024-01-15T10:30:00+00:00".to_string()));
/// assert_eq!(format_timestamp_opt(None), None);
/// ```
#[inline]
pub fn format_timestamp_opt(dt: Option<&DateTime<Utc>>) -> Option<String> {
    dt.map(|d| d.to_rfc3339())
}

/// Formats a decimal value with the specified precision.
///
/// # Arguments
///
/// * `value` - The decimal value to format
/// * `precision` - Number of decimal places to display
///
/// # Returns
///
/// A string representation of the decimal with the specified precision
///
/// # Examples
///
/// ```
/// use rust_decimal_macros::dec;
/// use trader_api::utils::format_decimal;
///
/// assert_eq!(format_decimal(&dec!(123.456789), 2), "123.45");
/// assert_eq!(format_decimal(&dec!(123.4), 4), "123.4000");
/// assert_eq!(format_decimal(&dec!(100), 2), "100.00");
/// ```
#[inline]
pub fn format_decimal(value: &Decimal, precision: u32) -> String {
    format!("{:.prec$}", value, prec = precision as usize)
}

/// Formats a decimal value as a percentage.
///
/// Converts a decimal fraction to a percentage string (e.g., 0.1523 -> "15.23").
///
/// # Arguments
///
/// * `value` - The decimal value to format (as a fraction, e.g., 0.1523 for 15.23%)
/// * `precision` - Number of decimal places to display in the percentage
///
/// # Returns
///
/// A string representation of the percentage value without the '%' symbol
///
/// # Examples
///
/// ```
/// use rust_decimal_macros::dec;
/// use trader_api::utils::format_percentage;
///
/// assert_eq!(format_percentage(&dec!(0.1523), 2), "15.23");
/// assert_eq!(format_percentage(&dec!(0.5), 1), "50.0");
/// assert_eq!(format_percentage(&dec!(-0.0325), 2), "-3.25");
/// ```
#[inline]
pub fn format_percentage(value: &Decimal, precision: u32) -> String {
    let percentage = value * Decimal::from(100);
    format!("{:.prec$}", percentage, prec = precision as usize)
}

/// Formats a decimal value as currency with thousands separators.
///
/// # Arguments
///
/// * `value` - The decimal value to format
/// * `currency` - The currency code (e.g., "USD", "KRW")
///
/// # Returns
///
/// A formatted string with thousands separators and currency code (e.g., "1,234.56 USD")
///
/// # Examples
///
/// ```
/// use rust_decimal_macros::dec;
/// use trader_api::utils::format_currency;
///
/// assert_eq!(format_currency(&dec!(1234.56), "USD"), "1,234.56 USD");
/// assert_eq!(format_currency(&dec!(1000000), "KRW"), "1,000,000.00 KRW");
/// assert_eq!(format_currency(&dec!(-500.5), "EUR"), "-500.50 EUR");
/// ```
#[inline]
pub fn format_currency(value: &Decimal, currency: &str) -> String {
    let is_negative = value.is_sign_negative();
    let abs_value = value.abs();

    // Round to 2 decimal places for currency
    let rounded = abs_value.round_dp(2);
    let formatted = format!("{:.2}", rounded);

    // Split into integer and decimal parts
    let parts: Vec<&str> = formatted.split('.').collect();
    let integer_part = parts[0];
    let decimal_part = parts.get(1).unwrap_or(&"00");

    // Add thousands separators to integer part
    let integer_with_separators: String = integer_part
        .chars()
        .rev()
        .enumerate()
        .flat_map(|(i, c)| {
            if i > 0 && i % 3 == 0 {
                vec![',', c]
            } else {
                vec![c]
            }
        })
        .collect::<Vec<char>>()
        .into_iter()
        .rev()
        .collect();

    if is_negative {
        format!("-{}.{} {}", integer_with_separators, decimal_part, currency)
    } else {
        format!("{}.{} {}", integer_with_separators, decimal_part, currency)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_format_timestamp() {
        let dt = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let result = format_timestamp(&dt);
        assert!(result.starts_with("2024-01-15T10:30:00"));
    }

    #[test]
    fn test_format_timestamp_opt() {
        let dt = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        assert!(format_timestamp_opt(Some(&dt)).is_some());
        assert!(format_timestamp_opt(None).is_none());
    }

    #[test]
    fn test_format_decimal() {
        // rust_decimal의 Display는 기본적으로 버림을 사용 (은행가 반올림 아님)
        assert_eq!(format_decimal(&dec!(123.456789), 2), "123.45");
        assert_eq!(format_decimal(&dec!(123.4), 4), "123.4000");
        assert_eq!(format_decimal(&dec!(100), 0), "100");
        assert_eq!(format_decimal(&dec!(-50.5), 2), "-50.50");
    }

    #[test]
    fn test_format_percentage() {
        assert_eq!(format_percentage(&dec!(0.1523), 2), "15.23");
        assert_eq!(format_percentage(&dec!(0.5), 1), "50.0");
        assert_eq!(format_percentage(&dec!(1.0), 0), "100");
        assert_eq!(format_percentage(&dec!(-0.0325), 2), "-3.25");
    }

    #[test]
    fn test_format_currency() {
        assert_eq!(format_currency(&dec!(1234.56), "USD"), "1,234.56 USD");
        assert_eq!(format_currency(&dec!(1000000), "KRW"), "1,000,000.00 KRW");
        assert_eq!(format_currency(&dec!(0.5), "EUR"), "0.50 EUR");
        assert_eq!(format_currency(&dec!(-500.5), "EUR"), "-500.50 EUR");
        assert_eq!(
            format_currency(&dec!(123456789.12), "USD"),
            "123,456,789.12 USD"
        );
    }

    #[test]
    fn test_format_currency_edge_cases() {
        assert_eq!(format_currency(&dec!(0), "USD"), "0.00 USD");
        assert_eq!(format_currency(&dec!(1), "USD"), "1.00 USD");
        assert_eq!(format_currency(&dec!(12), "USD"), "12.00 USD");
        assert_eq!(format_currency(&dec!(123), "USD"), "123.00 USD");
        assert_eq!(format_currency(&dec!(1234), "USD"), "1,234.00 USD");
    }
}
