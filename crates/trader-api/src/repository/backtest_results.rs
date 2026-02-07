//! 백테스트 결과 Repository.
//!
//! 백테스트 결과를 PostgreSQL에 영구 저장하고 조회하는 기능을 제공합니다.
//! Soft delete 패턴을 사용하여 데이터 무결성을 보장합니다.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use tracing::{debug, info, warn};
use uuid::Uuid;

// ==================== DB 레코드 ====================

/// 백테스트 결과 DB 레코드.
#[derive(Debug, Clone, FromRow)]
pub struct BacktestResultRecord {
    pub id: Uuid,
    pub strategy_id: String,
    pub strategy_type: String,
    pub symbol: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub initial_capital: Decimal,
    pub slippage_rate: Option<Decimal>,
    pub metrics: serde_json::Value,
    pub config_summary: serde_json::Value,
    pub equity_curve: serde_json::Value,
    pub trades: serde_json::Value,
    pub success: bool,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    /// 백테스트에 사용된 타임프레임 설정 (다중 TF 백테스트 시)
    pub timeframes_used: Option<serde_json::Value>,
}

// ==================== 요청/응답 타입 ====================

/// 결과 저장용 입력 데이터.
#[derive(Debug, Clone)]
pub struct BacktestResultInput {
    /// 전략 ID (등록된 전략의 고유 ID)
    pub strategy_id: String,
    /// 전략 타입 (sma_crossover, bollinger 등)
    pub strategy_type: String,
    /// 심볼 (다중 자산은 콤마 구분)
    pub symbol: String,
    /// 시작 날짜
    pub start_date: NaiveDate,
    /// 종료 날짜
    pub end_date: NaiveDate,
    /// 초기 자본
    pub initial_capital: Decimal,
    /// 슬리피지율
    pub slippage_rate: Option<Decimal>,
    /// 성과 지표
    pub metrics: serde_json::Value,
    /// 설정 요약
    pub config_summary: serde_json::Value,
    /// 자산 곡선
    pub equity_curve: serde_json::Value,
    /// 거래 내역
    pub trades: serde_json::Value,
    /// 성공 여부
    pub success: bool,
    /// 백테스트에 사용된 타임프레임 설정 (다중 TF 백테스트 시)
    pub timeframes_used: Option<serde_json::Value>,
}

/// 저장된 결과 응답용 DTO.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct BacktestResultDto {
    pub id: String,
    pub strategy_id: String,
    pub strategy_type: String,
    pub symbol: String,
    pub start_date: String,
    pub end_date: String,
    pub initial_capital: String,
    pub slippage_rate: Option<String>,
    pub metrics: serde_json::Value,
    pub config_summary: serde_json::Value,
    pub equity_curve: serde_json::Value,
    pub trades: serde_json::Value,
    pub success: bool,
    pub created_at: String,
    /// 백테스트에 사용된 타임프레임 설정
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeframes_used: Option<serde_json::Value>,
}

impl From<BacktestResultRecord> for BacktestResultDto {
    fn from(record: BacktestResultRecord) -> Self {
        Self {
            id: record.id.to_string(),
            strategy_id: record.strategy_id,
            strategy_type: record.strategy_type,
            symbol: record.symbol,
            start_date: record.start_date.to_string(),
            end_date: record.end_date.to_string(),
            initial_capital: record.initial_capital.to_string(),
            slippage_rate: record.slippage_rate.map(|r| r.to_string()),
            metrics: record.metrics,
            config_summary: record.config_summary,
            equity_curve: record.equity_curve,
            trades: record.trades,
            success: record.success,
            created_at: record.created_at.to_rfc3339(),
            timeframes_used: record.timeframes_used,
        }
    }
}

/// 결과 목록 조회 필터.
#[derive(Debug, Clone, Default)]
pub struct ListResultsFilter {
    /// 전략 ID 필터
    pub strategy_id: Option<String>,
    /// 전략 타입 필터
    pub strategy_type: Option<String>,
    /// 결과 수 제한
    pub limit: i64,
    /// 오프셋
    pub offset: i64,
}

impl ListResultsFilter {
    /// 기본 limit 50으로 생성.
    pub fn new() -> Self {
        Self {
            strategy_id: None,
            strategy_type: None,
            limit: 50,
            offset: 0,
        }
    }

    /// 전략 ID 필터 설정.
    pub fn with_strategy_id(mut self, strategy_id: impl Into<String>) -> Self {
        self.strategy_id = Some(strategy_id.into());
        self
    }

    /// 전략 타입 필터 설정.
    pub fn with_strategy_type(mut self, strategy_type: impl Into<String>) -> Self {
        self.strategy_type = Some(strategy_type.into());
        self
    }

    /// 페이지네이션 설정.
    pub fn with_pagination(mut self, limit: i64, offset: i64) -> Self {
        self.limit = limit;
        self.offset = offset;
        self
    }
}

/// 결과 목록 응답.
#[derive(Debug, Clone, Serialize)]
pub struct ListResultsResponse {
    pub results: Vec<BacktestResultDto>,
    pub total: i64,
}

// ==================== Repository ====================

/// 백테스트 결과 Repository.
///
/// `backtest_results` 테이블에 대한 CRUD 작업을 제공합니다.
/// Soft delete 패턴을 사용하여 삭제된 데이터도 보존합니다.
pub struct BacktestResultsRepository;

impl BacktestResultsRepository {
    /// 백테스트 결과 저장.
    pub async fn save(pool: &PgPool, input: BacktestResultInput) -> Result<Uuid, sqlx::Error> {
        debug!("백테스트 결과 저장: strategy_id={}", input.strategy_id);

        let row: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO backtest_results (
                strategy_id, strategy_type, symbol, start_date, end_date,
                initial_capital, slippage_rate, metrics, config_summary,
                equity_curve, trades, success, timeframes_used
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING id
            "#,
        )
        .bind(&input.strategy_id)
        .bind(&input.strategy_type)
        .bind(&input.symbol)
        .bind(input.start_date)
        .bind(input.end_date)
        .bind(input.initial_capital)
        .bind(input.slippage_rate)
        .bind(&input.metrics)
        .bind(&input.config_summary)
        .bind(&input.equity_curve)
        .bind(&input.trades)
        .bind(input.success)
        .bind(&input.timeframes_used)
        .fetch_one(pool)
        .await?;

        info!("백테스트 결과 저장 완료: id={}", row.0);
        Ok(row.0)
    }

    /// 백테스트 결과 조회 (단일).
    pub async fn get_by_id(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<Option<BacktestResultRecord>, sqlx::Error> {
        debug!("백테스트 결과 조회: id={}", id);

        let result = sqlx::query_as::<_, BacktestResultRecord>(
            r#"
            SELECT id, strategy_id, strategy_type, symbol, start_date, end_date,
                   initial_capital, slippage_rate, metrics, config_summary,
                   equity_curve, trades, success, error_message, created_at, deleted_at, timeframes_used
            FROM backtest_results
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(result)
    }

    /// 백테스트 결과 목록 조회.
    pub async fn list(
        pool: &PgPool,
        filter: ListResultsFilter,
    ) -> Result<ListResultsResponse, sqlx::Error> {
        debug!("백테스트 결과 목록 조회: {:?}", filter);

        // 전체 개수 조회
        let count_result: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) as count
            FROM backtest_results
            WHERE deleted_at IS NULL
              AND ($1::text IS NULL OR strategy_id = $1)
              AND ($2::text IS NULL OR strategy_type = $2)
            "#,
        )
        .bind(&filter.strategy_id)
        .bind(&filter.strategy_type)
        .fetch_one(pool)
        .await?;

        let total = count_result.0;

        // 결과 목록 조회
        let records: Vec<BacktestResultRecord> = sqlx::query_as(
            r#"
            SELECT id, strategy_id, strategy_type, symbol, start_date, end_date,
                   initial_capital, slippage_rate, metrics, config_summary,
                   equity_curve, trades, success, error_message, created_at, deleted_at, timeframes_used
            FROM backtest_results
            WHERE deleted_at IS NULL
              AND ($1::text IS NULL OR strategy_id = $1)
              AND ($2::text IS NULL OR strategy_type = $2)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(&filter.strategy_id)
        .bind(&filter.strategy_type)
        .bind(filter.limit)
        .bind(filter.offset)
        .fetch_all(pool)
        .await?;

        let results: Vec<BacktestResultDto> = records.into_iter().map(Into::into).collect();

        Ok(ListResultsResponse { results, total })
    }

    /// 백테스트 결과 삭제 (soft delete).
    ///
    /// 실제로 데이터를 삭제하지 않고 `deleted_at` 타임스탬프를 설정합니다.
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        debug!("백테스트 결과 삭제: id={}", id);

        let result = sqlx::query(
            r#"
            UPDATE backtest_results
            SET deleted_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .execute(pool)
        .await?;

        if result.rows_affected() > 0 {
            info!("백테스트 결과 삭제 완료: id={}", id);
            Ok(true)
        } else {
            warn!("백테스트 결과를 찾을 수 없음: id={}", id);
            Ok(false)
        }
    }

    /// 특정 전략의 모든 결과 조회.
    pub async fn get_by_strategy_id(
        pool: &PgPool,
        strategy_id: &str,
    ) -> Result<Vec<BacktestResultRecord>, sqlx::Error> {
        debug!("전략별 백테스트 결과 조회: strategy_id={}", strategy_id);

        let records = sqlx::query_as::<_, BacktestResultRecord>(
            r#"
            SELECT id, strategy_id, strategy_type, symbol, start_date, end_date,
                   initial_capital, slippage_rate, metrics, config_summary,
                   equity_curve, trades, success, error_message, created_at, deleted_at, timeframes_used
            FROM backtest_results
            WHERE strategy_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            "#,
        )
        .bind(strategy_id)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 최근 N개 결과 조회.
    pub async fn get_recent(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<BacktestResultRecord>, sqlx::Error> {
        debug!("최근 백테스트 결과 조회: limit={}", limit);

        let records = sqlx::query_as::<_, BacktestResultRecord>(
            r#"
            SELECT id, strategy_id, strategy_type, symbol, start_date, end_date,
                   initial_capital, slippage_rate, metrics, config_summary,
                   equity_curve, trades, success, error_message, created_at, deleted_at, timeframes_used
            FROM backtest_results
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 결과 개수 조회.
    pub async fn count(pool: &PgPool, strategy_id: Option<&str>) -> Result<i64, sqlx::Error> {
        let count: Option<i64> = if let Some(sid) = strategy_id {
            sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM backtest_results
                WHERE strategy_id = $1 AND deleted_at IS NULL
                "#,
            )
            .bind(sid)
            .fetch_one(pool)
            .await?
        } else {
            sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM backtest_results
                WHERE deleted_at IS NULL
                "#,
            )
            .fetch_one(pool)
            .await?
        };

        Ok(count.unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_list_results_filter_builder() {
        let filter = ListResultsFilter::new()
            .with_strategy_id("strat-001")
            .with_strategy_type("sma_crossover")
            .with_pagination(20, 40);

        assert_eq!(filter.strategy_id, Some("strat-001".to_string()));
        assert_eq!(filter.strategy_type, Some("sma_crossover".to_string()));
        assert_eq!(filter.limit, 20);
        assert_eq!(filter.offset, 40);
    }

    #[test]
    fn test_backtest_result_dto_from_record() {
        let record = BacktestResultRecord {
            id: Uuid::new_v4(),
            strategy_id: "test-strategy".to_string(),
            strategy_type: "sma_crossover".to_string(),
            symbol: "AAPL".to_string(),
            start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            initial_capital: dec!(10000),
            slippage_rate: Some(dec!(0.001)),
            metrics: serde_json::json!({"sharpe_ratio": 1.5}),
            config_summary: serde_json::json!({"fast_period": 10}),
            equity_curve: serde_json::json!([]),
            trades: serde_json::json!([]),
            success: true,
            error_message: None,
            created_at: Utc::now(),
            deleted_at: None,
            timeframes_used: None,
        };

        let dto: BacktestResultDto = record.into();

        assert_eq!(dto.strategy_id, "test-strategy");
        assert_eq!(dto.strategy_type, "sma_crossover");
        assert!(dto.success);
    }
}
