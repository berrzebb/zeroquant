//! Serde 역직렬화를 위한 공통 헬퍼 함수.
//!
//! SDUI와 전략 설정 간의 타입 불일치를 처리합니다.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// 문자열 또는 문자열 배열을 단일 문자열로 역직렬화.
///
/// SDUI의 `ticker_picker`는 배열을 생성하지만 전략은 단일 심볼을 기대합니다.
/// 이 함수는 두 형식 모두 처리할 수 있습니다.
///
/// # Examples
///
/// ```ignore
/// #[derive(Deserialize)]
/// struct Config {
///     #[serde(deserialize_with = "deserialize_ticker")]
///     ticker:  String,
/// }
///
/// // 다음 모두 작동:
/// // { "ticker": "005930" }
/// // { "ticker": ["005930"] }
/// ```
pub fn deserialize_ticker<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let value = Value::deserialize(deserializer)?;

    match value {
        Value::String(s) => Ok(s),
        Value::Array(arr) => {
            if let Some(Value::String(s)) = arr.first() {
                return Ok(s.clone());
            }
            Err(D::Error::custom(
                "ticker array is empty or contains non-string",
            ))
        }
        _ => Err(D::Error::custom(
            "ticker must be a string or array of strings",
        )),
    }
}

/// 문자열 배열을 Vec<String>으로 역직렬화.
///
/// SDUI의 `ticker_picker`는 배열을 생성합니다.
/// 이 함수는 배열 또는 단일 문자열 모두 처리할 수 있습니다.
///
/// # Examples
///
/// ```ignore
/// #[derive(Deserialize)]
/// struct Config {
///     #[serde(deserialize_with = "deserialize_tickers")]
///     tickers: Vec<String>,
/// }
///
/// // 다음 모두 작동:
/// // { "tickers": ["005930", "000660"] }
/// // { "tickers": "005930" }
/// ```
pub fn deserialize_tickers<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let value = Value::deserialize(deserializer)?;

    match value {
        Value::String(s) => Ok(vec![s]),
        Value::Array(arr) => {
            let mut result = Vec::new();
            for item in arr {
                if let Value::String(s) = item {
                    result.push(s);
                } else {
                    return Err(D::Error::custom("tickers array contains non-string"));
                }
            }
            Ok(result)
        }
        _ => Err(D::Error::custom(
            "tickers must be a string or array of strings",
        )),
    }
}

/// 옵션 필드용: 문자열 또는 문자열 배열을 Option<String>으로 역직렬화.
pub fn deserialize_ticker_opt<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let value = Option::<Value>::deserialize(deserializer)?;

    match value {
        None => Ok(None),
        Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(Value::Array(arr)) if arr.is_empty() => Ok(None),
        Some(Value::Array(arr)) => {
            if let Some(Value::String(s)) = arr.first() {
                return Ok(Some(s.clone()));
            }
            Err(D::Error::custom("ticker array contains non-string"))
        }
        Some(_) => Err(D::Error::custom(
            "ticker must be a string or array of strings",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct TestConfig {
        #[serde(deserialize_with = "deserialize_ticker")]
        ticker: String,
    }

    #[test]
    fn test_deserialize_string() {
        let json = r#"{"ticker": "005930"}"#;
        let config: TestConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.ticker, "005930");
    }

    #[test]
    fn test_deserialize_array() {
        let json = r#"{"ticker": ["005930"]}"#;
        let config: TestConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.ticker, "005930");
    }

    #[test]
    fn test_deserialize_array_multiple() {
        // 여러 심볼이 있어도 첫 번째만 사용
        let json = r#"{"ticker": ["005930", "000660"]}"#;
        let config: TestConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.ticker, "005930");
    }
}
