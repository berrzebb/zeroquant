//! Serde helper functions for custom serialization and deserialization.
//!
//! This module provides utility functions for handling common serialization patterns
//! such as symbol normalization, decimal parsing, and flexible boolean parsing.

use rust_decimal::Decimal;
use serde::{de, Deserialize, Deserializer, Serializer};
use std::str::FromStr;

/// Deserializes a symbol string with normalization.
///
/// Performs the following transformations:
/// - Trims leading and trailing whitespace
/// - Converts to uppercase
///
/// # Examples
///
/// ```ignore
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Order {
///     #[serde(deserialize_with = "deserialize_symbol")]
///     symbol: String,
/// }
/// ```
pub fn deserialize_symbol<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(s.trim().to_uppercase())
}

/// Deserializes a `Decimal` from a string representation.
///
/// # Errors
///
/// Returns an error if the string cannot be parsed as a valid `Decimal`.
///
/// # Examples
///
/// ```ignore
/// use serde::Deserialize;
/// use rust_decimal::Decimal;
///
/// #[derive(Deserialize)]
/// struct Price {
///     #[serde(deserialize_with = "deserialize_decimal_from_string")]
///     value: Decimal,
/// }
/// ```
pub fn deserialize_decimal_from_string<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Decimal::from_str(s.trim()).map_err(de::Error::custom)
}

/// Deserializes an `Option<Decimal>` from an optional string representation.
///
/// Handles the following cases:
/// - `null` or missing field -> `None`
/// - Empty string -> `None`
/// - Valid decimal string -> `Some(Decimal)`
///
/// # Errors
///
/// Returns an error if the string is present but cannot be parsed as a valid `Decimal`.
///
/// # Examples
///
/// ```ignore
/// use serde::Deserialize;
/// use rust_decimal::Decimal;
///
/// #[derive(Deserialize)]
/// struct Order {
///     #[serde(default, deserialize_with = "deserialize_decimal_opt_from_string")]
///     stop_price: Option<Decimal>,
/// }
/// ```
pub fn deserialize_decimal_opt_from_string<'de, D>(
    deserializer: D,
) -> Result<Option<Decimal>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;

    match opt {
        None => Ok(None),
        Some(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Decimal::from_str(trimmed)
                    .map(Some)
                    .map_err(de::Error::custom)
            }
        }
    }
}

/// Serializes a `Decimal` as a string.
///
/// This is useful for APIs that expect decimal values as strings to preserve precision.
///
/// # Examples
///
/// ```ignore
/// use serde::Serialize;
/// use rust_decimal::Decimal;
///
/// #[derive(Serialize)]
/// struct Price {
///     #[serde(serialize_with = "serialize_decimal_as_string")]
///     value: Decimal,
/// }
/// ```
pub fn serialize_decimal_as_string<S>(value: &Decimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

/// Deserializes a boolean from various string representations.
///
/// Supports the following truthy values (case-insensitive):
/// - `"true"`, `"1"`, `"yes"`
///
/// Supports the following falsy values (case-insensitive):
/// - `"false"`, `"0"`, `"no"`
///
/// # Errors
///
/// Returns an error if the string is not a recognized boolean representation.
///
/// # Examples
///
/// ```ignore
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Settings {
///     #[serde(deserialize_with = "deserialize_bool_from_string")]
///     enabled: bool,
/// }
/// ```
pub fn deserialize_bool_from_string<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let trimmed = s.trim().to_lowercase();

    match trimmed.as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(de::Error::custom(format!(
            "invalid boolean value: '{}'. Expected one of: true, false, 1, 0, yes, no",
            s
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestSymbol {
        #[serde(deserialize_with = "deserialize_symbol")]
        symbol: String,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestDecimal {
        #[serde(deserialize_with = "deserialize_decimal_from_string")]
        value: Decimal,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestDecimalOpt {
        #[serde(default, deserialize_with = "deserialize_decimal_opt_from_string")]
        value: Option<Decimal>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestBool {
        #[serde(deserialize_with = "deserialize_bool_from_string")]
        flag: bool,
    }

    #[test]
    fn test_deserialize_symbol_trims_whitespace() {
        let json = json!({"symbol": "  btcusdt  "});
        let result: TestSymbol = serde_json::from_value(json).unwrap();
        assert_eq!(result.symbol, "BTCUSDT");
    }

    #[test]
    fn test_deserialize_symbol_uppercase() {
        let json = json!({"symbol": "ethusdt"});
        let result: TestSymbol = serde_json::from_value(json).unwrap();
        assert_eq!(result.symbol, "ETHUSDT");
    }

    #[test]
    fn test_deserialize_decimal_from_string() {
        let json = json!({"value": "123.456"});
        let result: TestDecimal = serde_json::from_value(json).unwrap();
        assert_eq!(result.value, Decimal::from_str("123.456").unwrap());
    }

    #[test]
    fn test_deserialize_decimal_from_string_with_whitespace() {
        let json = json!({"value": "  789.012  "});
        let result: TestDecimal = serde_json::from_value(json).unwrap();
        assert_eq!(result.value, Decimal::from_str("789.012").unwrap());
    }

    #[test]
    fn test_deserialize_decimal_from_string_invalid() {
        let json = json!({"value": "not_a_number"});
        let result: Result<TestDecimal, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_decimal_opt_some() {
        let json = json!({"value": "456.789"});
        let result: TestDecimalOpt = serde_json::from_value(json).unwrap();
        assert_eq!(result.value, Some(Decimal::from_str("456.789").unwrap()));
    }

    #[test]
    fn test_deserialize_decimal_opt_none_null() {
        let json = json!({"value": null});
        let result: TestDecimalOpt = serde_json::from_value(json).unwrap();
        assert_eq!(result.value, None);
    }

    #[test]
    fn test_deserialize_decimal_opt_none_empty() {
        let json = json!({"value": ""});
        let result: TestDecimalOpt = serde_json::from_value(json).unwrap();
        assert_eq!(result.value, None);
    }

    #[test]
    fn test_deserialize_decimal_opt_none_whitespace() {
        let json = json!({"value": "   "});
        let result: TestDecimalOpt = serde_json::from_value(json).unwrap();
        assert_eq!(result.value, None);
    }

    #[test]
    fn test_deserialize_bool_true_variants() {
        for value in ["true", "TRUE", "True", "1", "yes", "YES", "Yes"] {
            let json = json!({"flag": value});
            let result: TestBool = serde_json::from_value(json).unwrap();
            assert!(result.flag, "Failed for value: {}", value);
        }
    }

    #[test]
    fn test_deserialize_bool_false_variants() {
        for value in ["false", "FALSE", "False", "0", "no", "NO", "No"] {
            let json = json!({"flag": value});
            let result: TestBool = serde_json::from_value(json).unwrap();
            assert!(!result.flag, "Failed for value: {}", value);
        }
    }

    #[test]
    fn test_deserialize_bool_invalid() {
        let json = json!({"flag": "maybe"});
        let result: Result<TestBool, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_serialize_decimal_as_string() {
        use serde::Serialize;

        #[derive(Serialize)]
        struct TestSerialize {
            #[serde(serialize_with = "serialize_decimal_as_string")]
            value: Decimal,
        }

        let test = TestSerialize {
            value: Decimal::from_str("123.456").unwrap(),
        };
        let json = serde_json::to_value(&test).unwrap();
        assert_eq!(json["value"], "123.456");
    }
}
