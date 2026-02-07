//! SignalMarker 리포지토리
//!
//! 백테스트 및 실거래에서 발생한 기술적 신호를 저장하고 조회합니다.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::collections::HashMap;
use trader_core::{Side, SignalIndicators, SignalMarker, SignalType, Symbol};
use uuid::Uuid;

use crate::error::{ApiErrorResponse, ApiResult};
use axum::http::StatusCode;
use axum::Json;

/// SignalMarker 리포지토리
pub struct SignalMarkerRepository {
    pool: PgPool,
}

impl SignalMarkerRepository {
    /// 새 리포지토리 생성
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// SignalMarker 저장
    ///
    /// # 에러
    /// - 데이터베이스 오류
    /// - 심볼 정보 없음
    pub async fn save(&self, marker: &SignalMarker) -> ApiResult<Uuid> {
        // symbol_info_id 조회 (ticker와 market으로 조회)
        let symbol_id: Uuid = sqlx::query_scalar(
            r#"
            SELECT id FROM symbol_info
            WHERE ticker = $1
            LIMIT 1
            "#,
        )
        .bind(&marker.ticker)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DB_ERROR", e.to_string())),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse::new(
                    "NOT_FOUND",
                    format!("Symbol not found: {}", marker.ticker),
                )),
            )
        })?;

        // indicators를 JSONB로 변환
        let indicators_json = serde_json::to_value(&marker.indicators).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new(
                    "SERIALIZATION_ERROR",
                    format!("Failed to serialize indicators: {}", e),
                )),
            )
        })?;

        // metadata를 JSONB로 변환
        let metadata_json = serde_json::to_value(&marker.metadata).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new(
                    "SERIALIZATION_ERROR",
                    format!("Failed to serialize metadata: {}", e),
                )),
            )
        })?;

        // side를 Option<String>으로 변환
        let side_str = marker.side.as_ref().map(|s| s.to_string());

        // INSERT
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO signal_marker (
                id, symbol_id, timestamp, signal_type, side, price, strength,
                indicators, reason, strategy_id, strategy_name, executed, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING id
            "#,
        )
        .bind(marker.id)
        .bind(symbol_id)
        .bind(marker.timestamp)
        .bind(marker.signal_type.to_string())
        .bind(side_str)
        .bind(marker.price)
        .bind(marker.strength)
        .bind(&indicators_json)
        .bind(&marker.reason)
        .bind(&marker.strategy_id)
        .bind(&marker.strategy_name)
        .bind(marker.executed)
        .bind(&metadata_json)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DB_ERROR", e.to_string())),
            )
        })?;

        Ok(id)
    }

    /// 심볼별 SignalMarker 조회
    ///
    /// # 인자
    /// - `symbol`: 심볼 (예: "005930")
    /// - `exchange`: 거래소 (예: "KRX")
    /// - `start_time`: 시작 시각 (선택)
    /// - `end_time`: 종료 시각 (선택)
    /// - `limit`: 최대 개수 (기본 100)
    pub async fn find_by_symbol(
        &self,
        symbol: &str,
        exchange: &str,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        limit: Option<i64>,
    ) -> ApiResult<Vec<SignalMarker>> {
        let limit = limit.unwrap_or(100).min(1000); // 최대 1000개로 제한

        let markers = sqlx::query_as::<_, SignalMarkerRow>(
            r#"
            SELECT
                sm.id, sm.timestamp, sm.signal_type, sm.side, sm.price, sm.strength,
                sm.indicators, sm.reason, sm.strategy_id, sm.strategy_name,
                sm.executed, sm.metadata,
                si.ticker, si.exchange, si.market
            FROM signal_marker sm
            JOIN symbol_info si ON sm.symbol_id = si.id
            WHERE si.ticker = $1 AND si.exchange = $2
                AND ($3::timestamptz IS NULL OR sm.timestamp >= $3)
                AND ($4::timestamptz IS NULL OR sm.timestamp <= $4)
            ORDER BY sm.timestamp DESC
            LIMIT $5
            "#,
        )
        .bind(symbol)
        .bind(exchange)
        .bind(start_time)
        .bind(end_time)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DB_ERROR", e.to_string())),
            )
        })?;

        markers
            .into_iter()
            .map(|row| row.to_signal_marker())
            .collect::<Result<Vec<_>, _>>()
    }

    /// 전략별 SignalMarker 조회
    ///
    /// # 인자
    /// - `strategy_id`: 전략 ID
    /// - `start_time`: 시작 시각 (선택)
    /// - `end_time`: 종료 시각 (선택)
    /// - `limit`: 최대 개수 (기본 100)
    pub async fn find_by_strategy(
        &self,
        strategy_id: &str,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        limit: Option<i64>,
    ) -> ApiResult<Vec<SignalMarker>> {
        let limit = limit.unwrap_or(100).min(1000);

        let markers = sqlx::query_as::<_, SignalMarkerRow>(
            r#"
            SELECT
                sm.id, sm.timestamp, sm.signal_type, sm.side, sm.price, sm.strength,
                sm.indicators, sm.reason, sm.strategy_id, sm.strategy_name,
                sm.executed, sm.metadata,
                si.ticker, si.exchange, si.market
            FROM signal_marker sm
            JOIN symbol_info si ON sm.symbol_id = si.id
            WHERE sm.strategy_id = $1
                AND ($2::timestamptz IS NULL OR sm.timestamp >= $2)
                AND ($3::timestamptz IS NULL OR sm.timestamp <= $3)
            ORDER BY sm.timestamp DESC
            LIMIT $4
            "#,
        )
        .bind(strategy_id)
        .bind(start_time)
        .bind(end_time)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DB_ERROR", e.to_string())),
            )
        })?;

        markers
            .into_iter()
            .map(|row| row.to_signal_marker())
            .collect::<Result<Vec<_>, _>>()
    }

    /// 지표 기반 SignalMarker 검색
    ///
    /// # 인자
    /// - `indicator_filter`: JSONB 쿼리 (예: `{"rsi": {"$gte": 70}}`)
    /// - `signal_type`: 신호 유형 필터 (선택)
    /// - `limit`: 최대 개수 (기본 100)
    ///
    /// # 예시
    /// ```ignore
    /// // RSI >= 70인 진입 신호 찾기
    /// let filter = json!({"rsi": {"$gte": 70.0}});
    /// let markers = repo.search_by_indicator(filter, Some("Entry"), None).await?;
    /// ```
    pub async fn search_by_indicator(
        &self,
        indicator_filter: JsonValue,
        signal_type: Option<&str>,
        limit: Option<i64>,
    ) -> ApiResult<Vec<SignalMarker>> {
        let limit = limit.unwrap_or(100).min(1000);

        // JSONB 쿼리 변환 (간단한 비교 연산만 지원)
        // TODO: 더 복잡한 쿼리는 추후 확장
        let where_clause = build_jsonb_where_clause(&indicator_filter);

        let query = format!(
            r#"
            SELECT
                sm.id, sm.timestamp, sm.signal_type, sm.side, sm.price, sm.strength,
                sm.indicators, sm.reason, sm.strategy_id, sm.strategy_name,
                sm.executed, sm.metadata,
                si.ticker, si.exchange, si.market
            FROM signal_marker sm
            JOIN symbol_info si ON sm.symbol_id = si.id
            WHERE {}
                AND ($1::varchar IS NULL OR sm.signal_type = $1)
            ORDER BY sm.timestamp DESC
            LIMIT $2
            "#,
            where_clause
        );

        let markers = sqlx::query_as::<_, SignalMarkerRow>(&query)
            .bind(signal_type)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiErrorResponse::new("DB_ERROR", e.to_string())),
                )
            })?;

        markers
            .into_iter()
            .map(|row| row.to_signal_marker())
            .collect::<Result<Vec<_>, _>>()
    }

    /// SignalMarker 삭제 (전략 ID 기준)
    pub async fn delete_by_strategy(&self, strategy_id: &str) -> ApiResult<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM signal_marker
            WHERE strategy_id = $1
            "#,
        )
        .bind(strategy_id)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DB_ERROR", e.to_string())),
            )
        })?;

        Ok(result.rows_affected())
    }
}

// ==================== Helper Structs ====================

/// SignalMarker 데이터베이스 행
#[derive(sqlx::FromRow)]
struct SignalMarkerRow {
    id: Uuid,
    timestamp: DateTime<Utc>,
    signal_type: String,
    side: Option<String>,
    price: Decimal,
    strength: f64,
    indicators: JsonValue,
    reason: String,
    strategy_id: String,
    strategy_name: String,
    executed: bool,
    metadata: JsonValue,
    ticker: String,
    #[allow(dead_code)]
    exchange: String,
    market: String,
}

impl SignalMarkerRow {
    /// SignalMarker로 변환
    #[allow(clippy::wrong_self_convention)]
    #[allow(clippy::result_large_err)]
    fn to_signal_marker(self) -> ApiResult<SignalMarker> {
        // SignalType 파싱
        let signal_type = match self.signal_type.as_str() {
            "ENTRY" | "Entry" => SignalType::Entry,
            "EXIT" | "Exit" => SignalType::Exit,
            "ALERT" | "Alert" => SignalType::Alert,
            "ADD_TO_POSITION" | "AddToPosition" => SignalType::AddToPosition,
            "REDUCE_POSITION" | "ReducePosition" => SignalType::ReducePosition,
            "SCALE" | "Scale" => SignalType::Scale,
            _ => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiErrorResponse::new(
                        "PARSE_ERROR",
                        format!("Unknown signal type: {}", self.signal_type),
                    )),
                ))
            }
        };

        // Side 파싱
        let side = self.side.as_ref().map(|s| match s.as_str() {
            "Buy" => Side::Buy,
            "Sell" => Side::Sell,
            _ => Side::Buy, // 기본값
        });

        // Symbol 구성 (market에서 MarketType 추론)
        use trader_core::MarketType;
        let market_type = match self.market.as_str() {
            "CRYPTO" | "Crypto" => MarketType::Crypto,
            "KR" => MarketType::Stock,
            "US" => MarketType::Stock,
            "STOCK" | "Stock" => MarketType::Stock,
            "FOREX" | "Forex" => MarketType::Forex,
            _ => MarketType::Stock, // 기본값
        };

        // quote는 market에서 추론 (KR은 KRW, Crypto는 USDT 등)
        let quote = match self.market.as_str() {
            "KR" => "KRW",
            "CRYPTO" | "Crypto" => "USDT",
            _ => "USD",
        };

        let symbol = Symbol::new(&self.ticker, quote, market_type);

        // SignalIndicators 역직렬화
        let indicators: SignalIndicators =
            serde_json::from_value(self.indicators).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiErrorResponse::new(
                        "PARSE_ERROR",
                        format!("Failed to deserialize indicators: {}", e),
                    )),
                )
            })?;

        // Metadata 역직렬화
        let metadata: HashMap<String, JsonValue> =
            serde_json::from_value(self.metadata).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiErrorResponse::new(
                        "PARSE_ERROR",
                        format!("Failed to deserialize metadata: {}", e),
                    )),
                )
            })?;

        Ok(SignalMarker {
            id: self.id,
            ticker: symbol.to_string(),
            timestamp: self.timestamp,
            signal_type,
            side,
            price: self.price,
            strength: self.strength,
            indicators,
            reason: self.reason,
            strategy_id: self.strategy_id,
            strategy_name: self.strategy_name,
            executed: self.executed,
            metadata,
        })
    }
}

// ==================== Helper Functions ====================

/// JSONB 필터를 WHERE 절로 변환
///
/// # 지원하는 연산자
/// - `$gte`: >=
/// - `$lte`: <=
/// - `$gt`: >
/// - `$lt`: <
/// - `$eq`: =
///
/// # 예시
/// ```json
/// {"rsi": {"$gte": 70.0}} → (indicators->>'rsi')::float >= 70.0
/// {"macd": {"$gt": 0}} → (indicators->>'macd')::numeric > 0
/// ```
fn build_jsonb_where_clause(filter: &JsonValue) -> String {
    if !filter.is_object() {
        return "1=1".to_string(); // 필터 없음
    }

    let mut conditions = Vec::new();

    for (key, value) in filter.as_object().unwrap() {
        if let Some(obj) = value.as_object() {
            for (op, val) in obj {
                let sql_op = match op.as_str() {
                    "$gte" => ">=",
                    "$lte" => "<=",
                    "$gt" => ">",
                    "$lt" => "<",
                    "$eq" => "=",
                    _ => continue,
                };

                // 타입별 JSONB 캐스팅
                let cast_type = if val.is_f64() || val.is_i64() {
                    "::float"
                } else {
                    ""
                };

                let condition = format!("(indicators->>'{}'){} {} {}", key, cast_type, sql_op, val);
                conditions.push(condition);
            }
        }
    }

    if conditions.is_empty() {
        "1=1".to_string()
    } else {
        conditions.join(" AND ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_jsonb_where_clause() {
        let filter = json!({"rsi": {"$gte": 70.0}});
        let result = build_jsonb_where_clause(&filter);
        assert!(result.contains("indicators->>'rsi'"));
        assert!(result.contains("::float"));
        assert!(result.contains(">="));
        assert!(result.contains("70"));
    }

    #[test]
    fn test_build_jsonb_where_clause_multiple() {
        let filter = json!({
            "rsi": {"$gte": 30.0, "$lte": 70.0},
            "macd": {"$gt": 0}
        });
        let result = build_jsonb_where_clause(&filter);
        assert!(result.contains("rsi"));
        assert!(result.contains("macd"));
        assert!(result.contains(">="));
        assert!(result.contains("<="));
        assert!(result.contains(">"));
    }
}
