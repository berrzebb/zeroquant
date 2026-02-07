//! 체결 내역 캐시 저장소.
//!
//! 거래소 중립적인 체결 내역 캐싱을 지원합니다.
//! 모든 거래소(KIS, Binance 등)에서 동일한 인터페이스를 사용합니다.

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Row};
use trader_core::{Side, TradeInfo};
use uuid::Uuid;

/// 거래소 중립적인 체결 내역 레코드.
///
/// 모든 거래소의 체결 데이터를 이 형식으로 정규화합니다.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CachedExecution {
    pub id: Uuid,
    pub credential_id: Uuid,

    /// 거래소 식별자 (kis, binance, coinbase 등)
    pub exchange: String,

    /// 체결 일시
    pub executed_at: DateTime<Utc>,

    /// 종목/심볼 코드 (거래소별 형식)
    pub symbol: String,

    /// 정규화된 심볼 (예: BTC/USDT, 005930.KS)
    pub normalized_symbol: Option<String>,

    /// 매매 방향 (buy/sell)
    pub side: Side,

    /// 체결 수량
    pub quantity: Decimal,

    /// 체결 단가
    pub price: Decimal,

    /// 체결 금액 (quantity * price)
    pub amount: Decimal,

    /// 수수료
    pub fee: Option<Decimal>,

    /// 수수료 통화
    pub fee_currency: Option<String>,

    /// 주문 ID (거래소 발급)
    pub order_id: String,

    /// 체결 ID (거래소 발급)
    pub trade_id: Option<String>,

    /// 주문 유형 (market, limit 등)
    pub order_type: Option<String>,

    /// 거래소별 원본 데이터 (디버깅/확장용)
    pub raw_data: Option<serde_json::Value>,

    pub created_at: Option<DateTime<Utc>>,
}

impl TradeInfo for CachedExecution {
    fn symbol(&self) -> &str {
        // normalized_symbol이 있으면 사용, 없으면 원본 symbol 사용
        self.normalized_symbol.as_deref().unwrap_or(&self.symbol)
    }

    fn pnl(&self) -> Option<Decimal> {
        // CachedExecution은 단일 체결 내역이므로 pnl이 없음.
        // 실현 손익은 여러 체결을 조합하여 계산해야 함.
        None
    }

    fn fees(&self) -> Decimal {
        self.fee.unwrap_or(Decimal::ZERO)
    }

    fn entry_time(&self) -> DateTime<Utc> {
        self.executed_at
    }

    fn exit_time(&self) -> Option<DateTime<Utc>> {
        // CachedExecution은 단일 체결이므로 청산 시각이 없음.
        None
    }
}

/// 새 체결 내역 삽입용 구조체.
#[derive(Debug, Clone)]
pub struct NewExecution {
    pub credential_id: Uuid,
    pub exchange: String,
    pub executed_at: DateTime<Utc>,
    pub symbol: String,
    pub normalized_symbol: Option<String>,
    pub side: Side,
    pub quantity: Decimal,
    pub price: Decimal,
    pub amount: Decimal,
    pub fee: Option<Decimal>,
    pub fee_currency: Option<String>,
    pub order_id: String,
    pub trade_id: Option<String>,
    pub order_type: Option<String>,
    pub raw_data: Option<serde_json::Value>,
}

/// 캐시 메타데이터.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CacheMeta {
    pub id: Uuid,
    pub credential_id: Uuid,
    pub exchange: String,
    pub earliest_date: Option<NaiveDate>,
    pub latest_date: Option<NaiveDate>,
    pub total_records: Option<i32>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_sync_status: Option<String>,
    pub last_sync_message: Option<String>,
}

/// 체결 내역 제공자 trait.
///
/// 각 거래소 커넥터가 이 trait을 구현하여
/// 체결 내역을 정규화된 형식으로 제공합니다.
#[async_trait]
pub trait ExecutionProvider: Send + Sync {
    /// 거래소 이름 반환.
    fn exchange_name(&self) -> &str;

    /// 특정 기간의 체결 내역 조회.
    ///
    /// 거래소별 API를 호출하고 결과를 `NewExecution` 형식으로 정규화.
    async fn fetch_executions(
        &self,
        credential_id: Uuid,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<NewExecution>, Box<dyn std::error::Error + Send + Sync>>;

    /// 조회 가능한 최대 기간 (일 단위).
    ///
    /// 예: KIS ISA = 365일, KIS 일반 = 90일, Binance = 90일
    fn max_query_days(&self) -> u32;
}

/// 체결 내역 캐시 저장소.
pub struct ExecutionCacheRepository;

impl ExecutionCacheRepository {
    /// 마지막 캐시된 일자 조회.
    ///
    /// 캐시가 없으면 None 반환 (전체 조회 필요).
    pub async fn get_latest_cached_date(
        pool: &PgPool,
        credential_id: Uuid,
        exchange: &str,
    ) -> Result<Option<NaiveDate>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT latest_date FROM execution_cache_meta WHERE credential_id = $1 AND exchange = $2"
        )
        .bind(credential_id)
        .bind(exchange)
        .fetch_optional(pool)
        .await?;

        Ok(row.and_then(|r| r.get("latest_date")))
    }

    /// 가장 오래된 캐시 일자 조회.
    pub async fn get_earliest_cached_date(
        pool: &PgPool,
        credential_id: Uuid,
        exchange: &str,
    ) -> Result<Option<NaiveDate>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT earliest_date FROM execution_cache_meta WHERE credential_id = $1 AND exchange = $2"
        )
        .bind(credential_id)
        .bind(exchange)
        .fetch_optional(pool)
        .await?;

        Ok(row.and_then(|r| r.get("earliest_date")))
    }

    /// 캐시 메타데이터 조회.
    pub async fn get_cache_meta(
        pool: &PgPool,
        credential_id: Uuid,
        exchange: &str,
    ) -> Result<Option<CacheMeta>, sqlx::Error> {
        sqlx::query_as::<_, CacheMeta>(
            r#"
            SELECT
                id, credential_id, exchange,
                earliest_date, latest_date,
                total_records, last_sync_at,
                last_sync_status, last_sync_message
            FROM execution_cache_meta
            WHERE credential_id = $1 AND exchange = $2
            "#,
        )
        .bind(credential_id)
        .bind(exchange)
        .fetch_optional(pool)
        .await
    }

    /// 체결 내역 일괄 저장 (upsert).
    ///
    /// 중복 키가 있으면 업데이트.
    pub async fn upsert_executions(
        pool: &PgPool,
        executions: &[NewExecution],
    ) -> Result<usize, sqlx::Error> {
        if executions.is_empty() {
            return Ok(0);
        }

        let mut inserted = 0;

        for exec in executions {
            let trade_id = exec.trade_id.clone().unwrap_or_default();

            let result = sqlx::query(
                r#"
                INSERT INTO execution_cache (
                    credential_id, exchange, executed_at,
                    symbol, normalized_symbol, side,
                    quantity, price, amount,
                    fee, fee_currency,
                    order_id, trade_id, order_type,
                    raw_data
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9,
                    $10, $11, $12, $13, $14, $15
                )
                ON CONFLICT (credential_id, exchange, order_id, COALESCE(trade_id, ''))
                DO UPDATE SET
                    quantity = EXCLUDED.quantity,
                    price = EXCLUDED.price,
                    amount = EXCLUDED.amount,
                    fee = EXCLUDED.fee,
                    updated_at = NOW()
                "#,
            )
            .bind(exec.credential_id)
            .bind(&exec.exchange)
            .bind(exec.executed_at)
            .bind(&exec.symbol)
            .bind(&exec.normalized_symbol)
            .bind(exec.side)
            .bind(exec.quantity)
            .bind(exec.price)
            .bind(exec.amount)
            .bind(exec.fee)
            .bind(&exec.fee_currency)
            .bind(&exec.order_id)
            .bind(&trade_id)
            .bind(&exec.order_type)
            .bind(&exec.raw_data)
            .execute(pool)
            .await?;

            if result.rows_affected() > 0 {
                inserted += 1;
            }
        }

        Ok(inserted)
    }

    /// 캐시 메타데이터 업데이트.
    pub async fn update_cache_meta(
        pool: &PgPool,
        credential_id: Uuid,
        exchange: &str,
        earliest_date: Option<NaiveDate>,
        latest_date: Option<NaiveDate>,
        status: &str,
        message: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        // 총 레코드 수 조회
        let row = sqlx::query(
            "SELECT COUNT(*) as cnt FROM execution_cache WHERE credential_id = $1 AND exchange = $2"
        )
        .bind(credential_id)
        .bind(exchange)
        .fetch_one(pool)
        .await?;

        let total_records: i64 = row.get("cnt");

        sqlx::query(
            r#"
            INSERT INTO execution_cache_meta (
                credential_id, exchange,
                earliest_date, latest_date,
                total_records, last_sync_at, last_sync_status, last_sync_message
            ) VALUES ($1, $2, $3, $4, $5, NOW(), $6, $7)
            ON CONFLICT (credential_id, exchange)
            DO UPDATE SET
                earliest_date = COALESCE(LEAST(execution_cache_meta.earliest_date, $3), $3),
                latest_date = COALESCE(GREATEST(execution_cache_meta.latest_date, $4), $4),
                total_records = $5,
                last_sync_at = NOW(),
                last_sync_status = $6,
                last_sync_message = $7,
                updated_at = NOW()
            "#,
        )
        .bind(credential_id)
        .bind(exchange)
        .bind(earliest_date)
        .bind(latest_date)
        .bind(total_records as i32)
        .bind(status)
        .bind(message)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// 특정 기간의 캐시된 체결 내역 조회.
    pub async fn get_executions_in_range(
        pool: &PgPool,
        credential_id: Uuid,
        exchange: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<CachedExecution>, sqlx::Error> {
        sqlx::query_as::<_, CachedExecution>(
            r#"
            SELECT
                id, credential_id, exchange, executed_at,
                symbol, normalized_symbol, side,
                quantity, price, amount,
                fee, fee_currency,
                order_id, trade_id, order_type,
                raw_data, created_at
            FROM execution_cache
            WHERE credential_id = $1
              AND exchange = $2
              AND executed_at >= $3
              AND executed_at <= $4
            ORDER BY executed_at DESC
            "#,
        )
        .bind(credential_id)
        .bind(exchange)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(pool)
        .await
    }

    /// 전체 캐시된 체결 내역 조회.
    pub async fn get_all_executions(
        pool: &PgPool,
        credential_id: Uuid,
        exchange: &str,
    ) -> Result<Vec<CachedExecution>, sqlx::Error> {
        sqlx::query_as::<_, CachedExecution>(
            r#"
            SELECT
                id, credential_id, exchange, executed_at,
                symbol, normalized_symbol, side,
                quantity, price, amount,
                fee, fee_currency,
                order_id, trade_id, order_type,
                raw_data, created_at
            FROM execution_cache
            WHERE credential_id = $1 AND exchange = $2
            ORDER BY executed_at DESC
            "#,
        )
        .bind(credential_id)
        .bind(exchange)
        .fetch_all(pool)
        .await
    }

    /// 캐시 삭제 (계좌+거래소별).
    pub async fn clear_cache(
        pool: &PgPool,
        credential_id: Uuid,
        exchange: &str,
    ) -> Result<u64, sqlx::Error> {
        // 먼저 메타데이터 삭제
        sqlx::query("DELETE FROM execution_cache_meta WHERE credential_id = $1 AND exchange = $2")
            .bind(credential_id)
            .bind(exchange)
            .execute(pool)
            .await?;

        // 캐시 데이터 삭제
        let result =
            sqlx::query("DELETE FROM execution_cache WHERE credential_id = $1 AND exchange = $2")
                .bind(credential_id)
                .bind(exchange)
                .execute(pool)
                .await?;

        Ok(result.rows_affected())
    }

    /// 모든 계좌의 캐시 삭제.
    pub async fn clear_all_cache(pool: &PgPool, credential_id: Uuid) -> Result<u64, sqlx::Error> {
        // 먼저 메타데이터 삭제
        sqlx::query("DELETE FROM execution_cache_meta WHERE credential_id = $1")
            .bind(credential_id)
            .execute(pool)
            .await?;

        // 캐시 데이터 삭제
        let result = sqlx::query("DELETE FROM execution_cache WHERE credential_id = $1")
            .bind(credential_id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected())
    }

    // ================================================================================================
    // Position Metrics
    // ================================================================================================

    /// 해당 credential에 체결 기록이 있는지 확인.
    pub async fn has_executions(
        pool: &PgPool,
        credential_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM execution_cache WHERE credential_id = $1",
        )
        .bind(credential_id)
        .fetch_one(pool)
        .await?;

        Ok(count > 0)
    }

    /// 순 포지션이 양수인 가장 최근 활성 credential ID 조회.
    ///
    /// 체결 기록이 있고 순 포지션(매수-매도)이 양수인 계좌 중 가장 최근 체결이 있는 것을 반환.
    pub async fn get_active_credential_with_positions(
        pool: &PgPool,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT credential_id
            FROM execution_cache
            GROUP BY credential_id
            HAVING SUM(CASE WHEN side = 'buy' THEN quantity ELSE -quantity END) > 0
            ORDER BY MAX(executed_at) DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(pool)
        .await
    }

    /// 순 보유 포지션의 총 투자금(평균단가 기준) 계산.
    ///
    /// 종목별 순 포지션과 평균 매수단가를 계산한 후 합산합니다.
    /// 반환: (순 수량 합계, 총 투자금)
    pub async fn get_position_cost_basis(
        pool: &PgPool,
        credential_id: Uuid,
    ) -> Result<Option<(rust_decimal::Decimal, rust_decimal::Decimal)>, sqlx::Error> {
        // CTE를 사용하여 종목별 순 포지션과 평균 매수단가를 계산한 후 합산
        // ROUND()로 소수점 6자리까지 제한 (rust_decimal 호환성)
        sqlx::query_as::<_, (rust_decimal::Decimal, rust_decimal::Decimal)>(
            r#"
            WITH net_positions AS (
                SELECT
                    symbol,
                    SUM(CASE WHEN side = 'buy' THEN quantity ELSE -quantity END) as net_qty,
                    SUM(CASE WHEN side = 'buy' THEN quantity * price ELSE 0 END) as total_buy_cost,
                    SUM(CASE WHEN side = 'buy' THEN quantity ELSE 0 END) as total_buy_qty
                FROM execution_cache
                WHERE credential_id = $1
                GROUP BY symbol
                HAVING SUM(CASE WHEN side = 'buy' THEN quantity ELSE -quantity END) > 0
            )
            SELECT
                COALESCE(ROUND(SUM(net_qty), 6), 0) as total_net_qty,
                COALESCE(ROUND(SUM(
                    CASE WHEN total_buy_qty > 0
                    THEN (total_buy_cost / total_buy_qty) * net_qty
                    ELSE 0 END
                ), 2), 0) as total_cost_basis
            FROM net_positions
            "#,
        )
        .bind(credential_id)
        .fetch_optional(pool)
        .await
    }
}
