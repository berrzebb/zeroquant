//! 매매일지 저장소.
//!
//! 체결 내역(trade_executions)과 포지션 스냅샷(position_snapshots)을 관리합니다.
//!
//! # 비용 기준 계산
//!
//! - **가중평균 매입가**: 물타기(추가 매수) 시 자동 계산
//! - **FIFO 실현손익**: 선입선출 방식으로 매도 시 실현 손익 계산
//!
//! [`calculate_cost_basis`](JournalRepository::calculate_cost_basis) 메서드로 특정 종목의
//! 비용 기준을 계산할 수 있습니다.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use trader_core::{unrealized_pnl, Side};
use uuid::Uuid;

use super::cost_basis::{self, CostBasisSummary, CostBasisTracker};

// =====================================================
// Trade Execution 타입
// =====================================================

/// 체결 내역 레코드.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TradeExecutionRecord {
    pub id: Uuid,
    pub credential_id: Uuid,
    pub exchange: String,
    pub symbol: String,
    pub symbol_name: Option<String>,
    pub side: Side,
    pub order_type: String,
    pub quantity: Decimal,
    pub price: Decimal,
    pub notional_value: Decimal,
    pub fee: Option<Decimal>,
    pub fee_currency: Option<String>,
    pub position_effect: Option<String>,
    pub realized_pnl: Option<Decimal>,
    pub order_id: Option<Uuid>,
    pub exchange_order_id: Option<String>,
    pub exchange_trade_id: Option<String>,
    pub strategy_id: Option<String>,
    pub strategy_name: Option<String>,
    pub executed_at: DateTime<Utc>,
    pub memo: Option<String>,
    pub tags: Option<Value>,
    pub metadata: Option<Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 체결 내역 생성 입력.
#[derive(Debug, Clone)]
pub struct TradeExecutionInput {
    pub credential_id: Uuid,
    pub exchange: String,
    pub symbol: String,
    pub symbol_name: Option<String>,
    pub side: Side,
    pub order_type: String,
    pub quantity: Decimal,
    pub price: Decimal,
    pub fee: Option<Decimal>,
    pub fee_currency: Option<String>,
    pub position_effect: Option<String>,
    pub realized_pnl: Option<Decimal>,
    pub order_id: Option<Uuid>,
    pub exchange_order_id: Option<String>,
    pub exchange_trade_id: Option<String>,
    pub strategy_id: Option<String>,
    pub strategy_name: Option<String>,
    pub executed_at: DateTime<Utc>,
    pub memo: Option<String>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<Value>,
}

// =====================================================
// Position Snapshot 타입
// =====================================================

/// 포지션 스냅샷 레코드.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PositionSnapshotRecord {
    pub id: Uuid,
    pub credential_id: Uuid,
    pub snapshot_time: DateTime<Utc>,
    pub exchange: String,
    pub symbol: String,
    pub symbol_name: Option<String>,
    pub side: Side,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub current_price: Option<Decimal>,
    pub cost_basis: Decimal,
    pub market_value: Option<Decimal>,
    pub unrealized_pnl: Option<Decimal>,
    pub unrealized_pnl_pct: Option<Decimal>,
    pub realized_pnl: Option<Decimal>,
    pub weight_pct: Option<Decimal>,
    pub first_trade_at: Option<DateTime<Utc>>,
    pub last_trade_at: Option<DateTime<Utc>>,
    pub trade_count: Option<i32>,
    pub strategy_id: Option<String>,
    pub metadata: Option<Value>,
    pub created_at: Option<DateTime<Utc>>,
}

/// 포지션 스냅샷 생성 입력.
#[derive(Debug, Clone)]
pub struct PositionSnapshotInput {
    pub credential_id: Uuid,
    pub exchange: String,
    pub symbol: String,
    pub symbol_name: Option<String>,
    pub side: Side,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub current_price: Option<Decimal>,
    pub realized_pnl: Option<Decimal>,
    pub weight_pct: Option<Decimal>,
    pub first_trade_at: Option<DateTime<Utc>>,
    pub last_trade_at: Option<DateTime<Utc>>,
    pub trade_count: Option<i32>,
    pub strategy_id: Option<String>,
    pub metadata: Option<Value>,
}

// =====================================================
// 집계 타입
// =====================================================

/// 일별 거래 요약.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DailySummary {
    pub credential_id: Uuid,
    pub trade_date: NaiveDate,
    pub total_trades: i64,
    pub buy_count: Option<i64>,
    pub sell_count: Option<i64>,
    pub total_volume: Option<Decimal>,
    pub total_fees: Option<Decimal>,
    pub realized_pnl: Option<Decimal>,
    pub symbol_count: Option<i64>,
}

/// 종목별 손익 요약.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SymbolPnL {
    pub credential_id: Uuid,
    pub symbol: String,
    pub symbol_name: Option<String>,
    pub total_trades: i64,
    pub total_buy_qty: Option<Decimal>,
    pub total_sell_qty: Option<Decimal>,
    pub total_buy_value: Option<Decimal>,
    pub total_sell_value: Option<Decimal>,
    pub total_fees: Option<Decimal>,
    pub realized_pnl: Option<Decimal>,
    pub first_trade_at: Option<DateTime<Utc>>,
    pub last_trade_at: Option<DateTime<Utc>>,
}

/// 현재 포지션 (최신 스냅샷).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CurrentPosition {
    pub id: Uuid,
    pub credential_id: Uuid,
    pub snapshot_time: DateTime<Utc>,
    pub exchange: String,
    pub symbol: String,
    pub symbol_name: Option<String>,
    pub side: Side,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub current_price: Option<Decimal>,
    pub cost_basis: Decimal,
    pub market_value: Option<Decimal>,
    pub unrealized_pnl: Option<Decimal>,
    pub unrealized_pnl_pct: Option<Decimal>,
    pub realized_pnl: Option<Decimal>,
    pub weight_pct: Option<Decimal>,
    pub first_trade_at: Option<DateTime<Utc>>,
    pub last_trade_at: Option<DateTime<Utc>>,
    pub trade_count: Option<i32>,
    pub strategy_id: Option<String>,
}

// =====================================================
// 체결 내역 필터
// =====================================================

/// 체결 내역 조회 필터.
#[derive(Debug, Clone, Default)]
pub struct ExecutionFilter {
    pub symbol: Option<String>,
    pub side: Option<String>,
    pub strategy_id: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// =====================================================
// JournalRepository
// =====================================================

/// 매매일지 저장소.
pub struct JournalRepository;

impl JournalRepository {
    // =====================================================
    // Trade Executions CRUD
    // =====================================================

    /// 체결 내역 추가.
    pub async fn create_execution(
        pool: &PgPool,
        input: TradeExecutionInput,
    ) -> Result<TradeExecutionRecord, sqlx::Error> {
        let notional_value = input.quantity * input.price;
        let tags_json: Option<Value> = input
            .tags
            .map(|t| serde_json::to_value(t).unwrap_or_default());

        let record = sqlx::query_as::<_, TradeExecutionRecord>(
            r#"
            INSERT INTO trade_executions (
                credential_id, exchange, symbol, symbol_name,
                side, order_type, quantity, price, notional_value,
                fee, fee_currency, position_effect, realized_pnl,
                order_id, exchange_order_id, exchange_trade_id,
                strategy_id, strategy_name, executed_at,
                memo, tags, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22)
            RETURNING *
            "#,
        )
        .bind(input.credential_id)
        .bind(&input.exchange)
        .bind(&input.symbol)
        .bind(&input.symbol_name)
        .bind(input.side)
        .bind(&input.order_type)
        .bind(input.quantity)
        .bind(input.price)
        .bind(notional_value)
        .bind(input.fee)
        .bind(&input.fee_currency)
        .bind(&input.position_effect)
        .bind(input.realized_pnl)
        .bind(input.order_id)
        .bind(&input.exchange_order_id)
        .bind(&input.exchange_trade_id)
        .bind(&input.strategy_id)
        .bind(&input.strategy_name)
        .bind(input.executed_at)
        .bind(&input.memo)
        .bind(tags_json)
        .bind(&input.metadata)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    /// 체결 내역 조회 (필터 적용).
    ///
    /// v_journal_executions 뷰를 사용하여 execution_cache + trade_executions를 조회합니다.
    pub async fn list_executions(
        pool: &PgPool,
        credential_id: Uuid,
        filter: ExecutionFilter,
    ) -> Result<Vec<TradeExecutionRecord>, sqlx::Error> {
        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);

        // 통합 뷰 사용으로 쿼리 단순화
        let records = sqlx::query_as::<_, TradeExecutionRecord>(
            r#"
            SELECT *
            FROM v_journal_executions
            WHERE credential_id = $1
                AND ($2::text IS NULL OR symbol = $2)
                AND ($3::text IS NULL OR side = $3)
                AND ($4::text IS NULL OR strategy_id = $4)
                AND ($5::timestamptz IS NULL OR executed_at >= $5)
                AND ($6::timestamptz IS NULL OR executed_at <= $6)
            ORDER BY executed_at DESC
            LIMIT $7 OFFSET $8
            "#,
        )
        .bind(credential_id)
        .bind(&filter.symbol)
        .bind(&filter.side)
        .bind(&filter.strategy_id)
        .bind(filter.start_date)
        .bind(filter.end_date)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 체결 내역 개수 조회.
    ///
    /// v_journal_executions 뷰를 사용합니다.
    pub async fn count_executions(
        pool: &PgPool,
        credential_id: Uuid,
        filter: ExecutionFilter,
    ) -> Result<i64, sqlx::Error> {
        let result: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM v_journal_executions
            WHERE credential_id = $1
                AND ($2::text IS NULL OR symbol = $2)
                AND ($3::text IS NULL OR side = $3)
                AND ($4::text IS NULL OR strategy_id = $4)
                AND ($5::timestamptz IS NULL OR executed_at >= $5)
                AND ($6::timestamptz IS NULL OR executed_at <= $6)
            "#,
        )
        .bind(credential_id)
        .bind(&filter.symbol)
        .bind(&filter.side)
        .bind(&filter.strategy_id)
        .bind(filter.start_date)
        .bind(filter.end_date)
        .fetch_one(pool)
        .await?;

        Ok(result.0)
    }

    /// 체결 내역 ID로 조회.
    ///
    /// v_journal_executions 뷰에서 execution_cache.id로 조회합니다.
    pub async fn get_execution(
        pool: &PgPool,
        execution_id: Uuid,
    ) -> Result<Option<TradeExecutionRecord>, sqlx::Error> {
        let record = sqlx::query_as::<_, TradeExecutionRecord>(
            "SELECT * FROM v_journal_executions WHERE id = $1",
        )
        .bind(execution_id)
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    /// 체결 내역 메모/태그 업데이트.
    ///
    /// execution_cache.id를 기준으로 trade_executions에 메모/태그를 저장합니다.
    /// trade_executions에 해당 레코드가 없으면 생성합니다.
    pub async fn update_execution_memo(
        pool: &PgPool,
        execution_id: Uuid,
        memo: Option<String>,
        tags: Option<Vec<String>>,
    ) -> Result<TradeExecutionRecord, sqlx::Error> {
        let tags_json: Option<Value> = tags.map(|t| serde_json::to_value(t).unwrap_or_default());

        // 1. execution_cache에서 원본 데이터 조회
        let cache_record = sqlx::query_as::<_, (Uuid, String, String, Option<String>)>(
            r#"
            SELECT credential_id, exchange, trade_id, symbol
            FROM execution_cache
            WHERE id = $1
            "#,
        )
        .bind(execution_id)
        .fetch_one(pool)
        .await?;

        let (_credential_id, _exchange, _trade_id, _symbol) = cache_record;

        // 2. trade_executions에 upsert (메모/태그만 저장)
        sqlx::query(
            r#"
            INSERT INTO trade_executions (
                credential_id, exchange, exchange_trade_id, symbol,
                side, order_type, quantity, price, notional_value,
                executed_at, memo, tags
            )
            SELECT
                ec.credential_id, ec.exchange, ec.trade_id, ec.symbol,
                ec.side, COALESCE(ec.order_type, 'market'), ec.quantity, ec.price, ec.amount,
                ec.executed_at, $2, $3
            FROM execution_cache ec
            WHERE ec.id = $1
            ON CONFLICT (credential_id, exchange, exchange_trade_id)
            DO UPDATE SET
                memo = COALESCE($2, trade_executions.memo),
                tags = COALESCE($3, trade_executions.tags),
                updated_at = NOW()
            "#,
        )
        .bind(execution_id)
        .bind(&memo)
        .bind(&tags_json)
        .execute(pool)
        .await?;

        // 3. 뷰에서 업데이트된 레코드 반환
        let record = sqlx::query_as::<_, TradeExecutionRecord>(
            "SELECT * FROM v_journal_executions WHERE id = $1",
        )
        .bind(execution_id)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    // =====================================================
    // Position Snapshots CRUD
    // =====================================================

    /// 포지션 스냅샷 저장.
    ///
    /// 동일 credential, symbol, 시간에 이미 존재하면 업데이트합니다.
    pub async fn save_position_snapshot(
        pool: &PgPool,
        input: PositionSnapshotInput,
    ) -> Result<PositionSnapshotRecord, sqlx::Error> {
        let cost_basis = input.entry_price * input.quantity;
        let market_value = input.current_price.map(|p| p * input.quantity);
        let unrealized_pnl_value = input.current_price.map(|current_price| {
            unrealized_pnl(input.entry_price, current_price, input.quantity, input.side)
        });
        let unrealized_pnl_pct = if cost_basis > Decimal::ZERO {
            unrealized_pnl_value.map(|pnl| (pnl / cost_basis) * Decimal::from(100))
        } else {
            None
        };

        let record = sqlx::query_as::<_, PositionSnapshotRecord>(
            r#"
            INSERT INTO position_snapshots (
                credential_id, exchange, symbol, symbol_name,
                side, quantity, entry_price, current_price,
                cost_basis, market_value, unrealized_pnl, unrealized_pnl_pct,
                realized_pnl, weight_pct, first_trade_at, last_trade_at,
                trade_count, strategy_id, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
            ON CONFLICT (credential_id, symbol, snapshot_time)
            DO UPDATE SET
                current_price = EXCLUDED.current_price,
                market_value = EXCLUDED.market_value,
                unrealized_pnl = EXCLUDED.unrealized_pnl,
                unrealized_pnl_pct = EXCLUDED.unrealized_pnl_pct,
                weight_pct = EXCLUDED.weight_pct
            RETURNING *
            "#,
        )
        .bind(input.credential_id)
        .bind(&input.exchange)
        .bind(&input.symbol)
        .bind(&input.symbol_name)
        .bind(input.side)
        .bind(input.quantity)
        .bind(input.entry_price)
        .bind(input.current_price)
        .bind(cost_basis)
        .bind(market_value)
        .bind(unrealized_pnl_value)
        .bind(unrealized_pnl_pct)
        .bind(input.realized_pnl)
        .bind(input.weight_pct)
        .bind(input.first_trade_at)
        .bind(input.last_trade_at)
        .bind(input.trade_count)
        .bind(&input.strategy_id)
        .bind(&input.metadata)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    /// 현재 포지션 조회 (최신 스냅샷).
    pub async fn get_current_positions(
        pool: &PgPool,
        credential_id: Uuid,
    ) -> Result<Vec<CurrentPosition>, sqlx::Error> {
        let records = sqlx::query_as::<_, CurrentPosition>(
            r#"
            SELECT DISTINCT ON (credential_id, symbol)
                id, credential_id, snapshot_time, exchange, symbol, symbol_name,
                side, quantity, entry_price, current_price, cost_basis, market_value,
                unrealized_pnl, unrealized_pnl_pct, realized_pnl, weight_pct,
                first_trade_at, last_trade_at, trade_count, strategy_id
            FROM position_snapshots
            WHERE credential_id = $1 AND quantity > 0
            ORDER BY credential_id, symbol, snapshot_time DESC
            "#,
        )
        .bind(credential_id)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 특정 종목 포지션 히스토리 조회.
    pub async fn get_position_history(
        pool: &PgPool,
        credential_id: Uuid,
        symbol: &str,
        limit: i64,
    ) -> Result<Vec<PositionSnapshotRecord>, sqlx::Error> {
        let records = sqlx::query_as::<_, PositionSnapshotRecord>(
            r#"
            SELECT *
            FROM position_snapshots
            WHERE credential_id = $1 AND symbol = $2
            ORDER BY snapshot_time DESC
            LIMIT $3
            "#,
        )
        .bind(credential_id)
        .bind(symbol)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    // =====================================================
    // 집계 조회
    // =====================================================

    /// 일별 거래 요약 조회.
    ///
    /// v_daily_pnl 뷰를 사용합니다.
    pub async fn get_daily_summary(
        pool: &PgPool,
        credential_id: Uuid,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<DailySummary>, sqlx::Error> {
        let records = sqlx::query_as::<_, DailySummary>(
            r#"
            SELECT
                credential_id,
                trade_date,
                total_trades,
                buy_count,
                sell_count,
                total_volume,
                total_fees,
                realized_pnl,
                symbol_count
            FROM v_daily_pnl
            WHERE credential_id = $1
                AND ($2::date IS NULL OR trade_date >= $2)
                AND ($3::date IS NULL OR trade_date <= $3)
            ORDER BY trade_date DESC
            "#,
        )
        .bind(credential_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 종목별 손익 요약 조회.
    ///
    /// v_symbol_pnl 뷰를 사용합니다.
    pub async fn get_symbol_pnl(
        pool: &PgPool,
        credential_id: Uuid,
    ) -> Result<Vec<SymbolPnL>, sqlx::Error> {
        let records = sqlx::query_as::<_, SymbolPnL>(
            r#"
            SELECT
                credential_id,
                symbol,
                symbol_name,
                total_trades,
                total_buy_qty,
                total_sell_qty,
                total_buy_value,
                total_sell_value,
                total_fees,
                realized_pnl,
                first_trade_at,
                last_trade_at
            FROM v_symbol_pnl
            WHERE credential_id = $1
            ORDER BY last_trade_at DESC
            "#,
        )
        .bind(credential_id)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 전체 PnL 요약 조회.
    ///
    /// v_total_pnl 뷰를 사용합니다.
    pub async fn get_total_pnl(
        pool: &PgPool,
        credential_id: Uuid,
    ) -> Result<PnLSummary, sqlx::Error> {
        // 뷰에서 데이터가 없을 수 있으므로 기본값 처리
        let result = sqlx::query_as::<_, PnLSummary>(
            r#"
            SELECT
                COALESCE(total_realized_pnl, 0) as total_realized_pnl,
                COALESCE(total_fees, 0) as total_fees,
                COALESCE(total_trades, 0) as total_trades,
                buy_trades,
                sell_trades,
                winning_trades,
                losing_trades,
                COALESCE(total_volume, 0) as total_volume,
                first_trade_at,
                last_trade_at
            FROM v_total_pnl
            WHERE credential_id = $1
            "#,
        )
        .bind(credential_id)
        .fetch_optional(pool)
        .await?;

        // 데이터가 없으면 빈 요약 반환
        Ok(result.unwrap_or(PnLSummary {
            total_realized_pnl: Decimal::ZERO,
            total_fees: Decimal::ZERO,
            total_trades: 0,
            buy_trades: Some(0),
            sell_trades: Some(0),
            winning_trades: Some(0),
            losing_trades: Some(0),
            total_volume: Decimal::ZERO,
            first_trade_at: None,
            last_trade_at: None,
        }))
    }

    // =====================================================
    // 동기화 (거래소 데이터 → 매매일지)
    // =====================================================

    /// 거래소 체결 내역 동기화.
    ///
    /// 거래소에서 가져온 체결 내역을 매매일지에 저장합니다.
    /// exchange_trade_id로 중복 체크하여 이미 있는 건 스킵합니다.
    pub async fn sync_executions(
        pool: &PgPool,
        credential_id: Uuid,
        executions: Vec<TradeExecutionInput>,
    ) -> Result<SyncResult, sqlx::Error> {
        if executions.is_empty() {
            return Ok(SyncResult {
                inserted: 0,
                skipped: 0,
            });
        }

        // 1. 기존 exchange_trade_id를 배치로 조회 (N+1 EXISTS 제거)
        let trade_ids: Vec<Option<&str>> = executions
            .iter()
            .map(|e| e.exchange_trade_id.as_deref())
            .collect();
        let existing_ids: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT exchange_trade_id
            FROM trade_executions
            WHERE credential_id = $1
              AND exchange_trade_id = ANY($2)
            "#,
        )
        .bind(credential_id)
        .bind(&trade_ids)
        .fetch_all(pool)
        .await?;

        let existing_set: std::collections::HashSet<&str> =
            existing_ids.iter().map(|(id,)| id.as_str()).collect();

        // 2. 새 항목만 필터링
        let new_executions: Vec<&TradeExecutionInput> = executions
            .iter()
            .filter(|e| {
                e.exchange_trade_id
                    .as_deref()
                    .map_or(true, |id| !existing_set.contains(id))
            })
            .collect();

        let skipped = (executions.len() - new_executions.len()) as i32;

        if new_executions.is_empty() {
            return Ok(SyncResult {
                inserted: 0,
                skipped,
            });
        }

        // 3. 트랜잭션 내 배치 INSERT (새 항목만)
        let mut tx = pool.begin().await?;
        let mut inserted = 0i32;

        for input in &new_executions {
            let notional_value = input.quantity * input.price;
            let tags_json: Option<Value> = input
                .tags
                .as_ref()
                .and_then(|t| serde_json::to_value(t).ok());

            sqlx::query(
                r#"
                INSERT INTO trade_executions (
                    credential_id, exchange, symbol, symbol_name,
                    side, order_type, quantity, price, notional_value,
                    fee, fee_currency, position_effect, realized_pnl,
                    order_id, exchange_order_id, exchange_trade_id,
                    strategy_id, strategy_name, executed_at,
                    memo, tags, metadata
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22)
                "#,
            )
            .bind(credential_id)
            .bind(&input.exchange)
            .bind(&input.symbol)
            .bind(&input.symbol_name)
            .bind(input.side)
            .bind(&input.order_type)
            .bind(input.quantity)
            .bind(input.price)
            .bind(notional_value)
            .bind(input.fee)
            .bind(&input.fee_currency)
            .bind(&input.position_effect)
            .bind(input.realized_pnl)
            .bind(input.order_id)
            .bind(&input.exchange_order_id)
            .bind(&input.exchange_trade_id)
            .bind(&input.strategy_id)
            .bind(&input.strategy_name)
            .bind(input.executed_at)
            .bind(&input.memo)
            .bind(tags_json)
            .bind(&input.metadata)
            .execute(&mut *tx)
            .await?;

            inserted += 1;
        }

        tx.commit().await?;

        Ok(SyncResult { inserted, skipped })
    }
}

/// PnL 요약.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PnLSummary {
    pub total_realized_pnl: Decimal,
    pub total_fees: Decimal,
    pub total_trades: i64,
    pub buy_trades: Option<i64>,
    pub sell_trades: Option<i64>,
    pub winning_trades: Option<i64>,
    pub losing_trades: Option<i64>,
    pub total_volume: Decimal,
    pub first_trade_at: Option<DateTime<Utc>>,
    pub last_trade_at: Option<DateTime<Utc>>,
}

/// 동기화 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub inserted: i32,
    pub skipped: i32,
}

// =====================================================
// 기간별 손익 타입
// =====================================================

/// 주별 손익 요약.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WeeklyPnL {
    pub credential_id: Uuid,
    pub week_start: NaiveDate,
    pub total_trades: i64,
    pub buy_count: Option<i64>,
    pub sell_count: Option<i64>,
    pub total_volume: Option<Decimal>,
    pub total_fees: Option<Decimal>,
    pub realized_pnl: Option<Decimal>,
    pub symbol_count: Option<i64>,
    pub trading_days: Option<i64>,
}

/// 월별 손익 요약.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MonthlyPnL {
    pub credential_id: Uuid,
    pub year: i32,
    pub month: i32,
    pub total_trades: i64,
    pub buy_count: Option<i64>,
    pub sell_count: Option<i64>,
    pub total_volume: Option<Decimal>,
    pub total_fees: Option<Decimal>,
    pub realized_pnl: Option<Decimal>,
    pub symbol_count: Option<i64>,
    pub trading_days: Option<i64>,
}

/// 연도별 손익 요약.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct YearlyPnL {
    pub credential_id: Uuid,
    pub year: i32,
    pub total_trades: i64,
    pub buy_count: Option<i64>,
    pub sell_count: Option<i64>,
    pub total_volume: Option<Decimal>,
    pub total_fees: Option<Decimal>,
    pub realized_pnl: Option<Decimal>,
    pub symbol_count: Option<i64>,
    pub trading_days: Option<i64>,
    pub trading_months: Option<i64>,
}

/// 누적 손익.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CumulativePnL {
    pub credential_id: Uuid,
    pub trade_date: NaiveDate,
    pub total_trades: i64,
    pub realized_pnl: Option<Decimal>,
    pub total_fees: Option<Decimal>,
    pub cumulative_pnl: Option<Decimal>,
    pub cumulative_fees: Option<Decimal>,
    pub cumulative_trades: Option<i64>,
}

/// 투자 인사이트.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TradingInsights {
    pub credential_id: Uuid,
    pub total_trades: i64,
    pub buy_trades: Option<i64>,
    pub sell_trades: Option<i64>,
    pub unique_symbols: Option<i64>,
    pub total_realized_pnl: Option<Decimal>,
    pub total_fees: Option<Decimal>,
    pub winning_trades: Option<i64>,
    pub losing_trades: Option<i64>,
    pub win_rate_pct: Option<Decimal>,
    pub avg_win: Option<Decimal>,
    pub avg_loss: Option<Decimal>,
    pub profit_factor: Option<Decimal>,
    pub trading_period_days: Option<i32>,
    pub active_trading_days: Option<i64>,
    pub largest_win: Option<Decimal>,
    pub largest_loss: Option<Decimal>,
    pub first_trade_at: Option<DateTime<Utc>>,
    pub last_trade_at: Option<DateTime<Utc>>,
}

/// 전략별 성과.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StrategyPerformance {
    pub credential_id: Uuid,
    pub strategy_id: String,
    pub strategy_name: String,
    pub total_trades: i64,
    pub buy_trades: Option<i64>,
    pub sell_trades: Option<i64>,
    pub unique_symbols: Option<i64>,
    pub total_volume: Option<Decimal>,
    pub total_fees: Option<Decimal>,
    pub realized_pnl: Option<Decimal>,
    pub winning_trades: Option<i64>,
    pub losing_trades: Option<i64>,
    pub win_rate_pct: Option<Decimal>,
    pub avg_win: Option<Decimal>,
    pub avg_loss: Option<Decimal>,
    pub profit_factor: Option<Decimal>,
    pub largest_win: Option<Decimal>,
    pub largest_loss: Option<Decimal>,
    pub active_trading_days: Option<i64>,
    pub first_trade_at: Option<DateTime<Utc>>,
    pub last_trade_at: Option<DateTime<Utc>>,
}

// =====================================================
// 확장 쿼리 메서드
// =====================================================

impl JournalRepository {
    /// 주별 손익 조회.
    pub async fn get_weekly_pnl(
        pool: &PgPool,
        credential_id: Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<WeeklyPnL>, sqlx::Error> {
        let limit = limit.unwrap_or(52); // 기본 1년치
        sqlx::query_as::<_, WeeklyPnL>(
            r#"
            SELECT *
            FROM v_weekly_pnl
            WHERE credential_id = $1
            ORDER BY week_start DESC
            LIMIT $2
            "#,
        )
        .bind(credential_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// 월별 손익 조회.
    pub async fn get_monthly_pnl(
        pool: &PgPool,
        credential_id: Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<MonthlyPnL>, sqlx::Error> {
        let limit = limit.unwrap_or(24); // 기본 2년치
        sqlx::query_as::<_, MonthlyPnL>(
            r#"
            SELECT *
            FROM v_monthly_pnl
            WHERE credential_id = $1
            ORDER BY year DESC, month DESC
            LIMIT $2
            "#,
        )
        .bind(credential_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// 연도별 손익 조회.
    pub async fn get_yearly_pnl(
        pool: &PgPool,
        credential_id: Uuid,
    ) -> Result<Vec<YearlyPnL>, sqlx::Error> {
        sqlx::query_as::<_, YearlyPnL>(
            r#"
            SELECT *
            FROM v_yearly_pnl
            WHERE credential_id = $1
            ORDER BY year DESC
            "#,
        )
        .bind(credential_id)
        .fetch_all(pool)
        .await
    }

    /// 누적 손익 조회 (일별).
    pub async fn get_cumulative_pnl(
        pool: &PgPool,
        credential_id: Uuid,
    ) -> Result<Vec<CumulativePnL>, sqlx::Error> {
        sqlx::query_as::<_, CumulativePnL>(
            r#"
            SELECT *
            FROM v_cumulative_pnl
            WHERE credential_id = $1
            ORDER BY trade_date ASC
            "#,
        )
        .bind(credential_id)
        .fetch_all(pool)
        .await
    }

    /// 투자 인사이트 조회.
    pub async fn get_trading_insights(
        pool: &PgPool,
        credential_id: Uuid,
    ) -> Result<Option<TradingInsights>, sqlx::Error> {
        sqlx::query_as::<_, TradingInsights>(
            r#"
            SELECT *
            FROM v_trading_insights
            WHERE credential_id = $1
            "#,
        )
        .bind(credential_id)
        .fetch_optional(pool)
        .await
    }

    /// 전략별 성과 조회.
    pub async fn get_strategy_performance(
        pool: &PgPool,
        credential_id: Uuid,
    ) -> Result<Vec<StrategyPerformance>, sqlx::Error> {
        sqlx::query_as::<_, StrategyPerformance>(
            r#"
            SELECT *
            FROM v_strategy_performance
            WHERE credential_id = $1
            ORDER BY realized_pnl DESC NULLS LAST
            "#,
        )
        .bind(credential_id)
        .fetch_all(pool)
        .await
    }

    // =====================================================
    // 비용 기준 계산 (물타기 평균가, FIFO 실현손익)
    // =====================================================

    /// 특정 종목의 비용 기준 계산.
    ///
    /// 체결 내역을 분석하여 가중평균 매입가와 FIFO 기반 실현손익을 계산합니다.
    ///
    /// # Arguments
    /// * `pool` - 데이터베이스 커넥션 풀
    /// * `credential_id` - 계정 ID
    /// * `symbol` - 종목 심볼
    /// * `current_price` - 현재가 (미실현 손익 계산용, 선택)
    ///
    /// # Returns
    /// 비용 기준 요약 정보 (가중평균 매입가, 실현/미실현 손익 등)
    pub async fn calculate_cost_basis(
        pool: &PgPool,
        credential_id: Uuid,
        symbol: &str,
        current_price: Option<Decimal>,
    ) -> Result<CostBasisSummary, sqlx::Error> {
        // 해당 종목의 모든 체결 내역 조회
        let executions = sqlx::query_as::<_, ExecutionRow>(
            r#"
            SELECT id, symbol, side, quantity, price,
                   COALESCE(fee, 0) as fee, executed_at
            FROM v_journal_executions
            WHERE credential_id = $1 AND symbol = $2
            ORDER BY executed_at ASC
            "#,
        )
        .bind(credential_id)
        .bind(symbol)
        .fetch_all(pool)
        .await?;

        // cost_basis 모듈의 TradeExecution으로 변환
        let trade_executions: Vec<cost_basis::TradeExecution> = executions
            .into_iter()
            .map(|e| cost_basis::TradeExecution {
                id: e.id,
                symbol: e.symbol,
                side: e.side,
                quantity: e.quantity,
                price: e.price,
                fee: e.fee,
                executed_at: e.executed_at,
            })
            .collect();

        // 비용 기준 추적기 생성
        let tracker = cost_basis::build_tracker_from_executions(symbol, trade_executions);

        Ok(tracker.summary(current_price))
    }

    /// 모든 종목의 비용 기준 계산.
    ///
    /// 모든 체결 내역을 한 번에 조회하여 Rust에서 심볼별로 그룹핑합니다.
    pub async fn calculate_all_cost_basis(
        pool: &PgPool,
        credential_id: Uuid,
        current_prices: &std::collections::HashMap<String, Decimal>,
    ) -> Result<Vec<CostBasisSummary>, sqlx::Error> {
        // 모든 종목의 체결 내역을 단일 쿼리로 조회 (N+1 제거)
        let all_executions = sqlx::query_as::<_, ExecutionRow>(
            r#"
            SELECT id, symbol, side, quantity, price,
                   COALESCE(fee, 0) as fee, executed_at
            FROM v_journal_executions
            WHERE credential_id = $1
            ORDER BY symbol, executed_at ASC
            "#,
        )
        .bind(credential_id)
        .fetch_all(pool)
        .await?;

        // 심볼별로 그룹핑
        let mut grouped: std::collections::HashMap<String, Vec<cost_basis::TradeExecution>> =
            std::collections::HashMap::new();
        for e in all_executions {
            grouped
                .entry(e.symbol.clone())
                .or_default()
                .push(cost_basis::TradeExecution {
                    id: e.id,
                    symbol: e.symbol,
                    side: e.side,
                    quantity: e.quantity,
                    price: e.price,
                    fee: e.fee,
                    executed_at: e.executed_at,
                });
        }

        // 각 심볼별 비용 기준 계산 (DB 호출 없음)
        let mut summaries = Vec::new();
        for (symbol, executions) in grouped {
            let current_price = current_prices.get(&symbol).copied();
            let tracker = cost_basis::build_tracker_from_executions(&symbol, executions);
            let summary = tracker.summary(current_price);

            if summary.total_quantity > Decimal::ZERO {
                summaries.push(summary);
            }
        }

        Ok(summaries)
    }

    /// 비용 기준 추적기 반환 (상세 분석용).
    ///
    /// 로트별 상세 정보가 필요한 경우 사용합니다.
    pub async fn get_cost_basis_tracker(
        pool: &PgPool,
        credential_id: Uuid,
        symbol: &str,
    ) -> Result<CostBasisTracker, sqlx::Error> {
        let executions = sqlx::query_as::<_, ExecutionRow>(
            r#"
            SELECT id, symbol, side, quantity, price,
                   COALESCE(fee, 0) as fee, executed_at
            FROM v_journal_executions
            WHERE credential_id = $1 AND symbol = $2
            ORDER BY executed_at ASC
            "#,
        )
        .bind(credential_id)
        .bind(symbol)
        .fetch_all(pool)
        .await?;

        let trade_executions: Vec<cost_basis::TradeExecution> = executions
            .into_iter()
            .map(|e| cost_basis::TradeExecution {
                id: e.id,
                symbol: e.symbol,
                side: e.side,
                quantity: e.quantity,
                price: e.price,
                fee: e.fee,
                executed_at: e.executed_at,
            })
            .collect();

        Ok(cost_basis::build_tracker_from_executions(
            symbol,
            trade_executions,
        ))
    }

    // =====================================================
    // 고급 거래 통계 (연속 승/패, Max Drawdown)
    // =====================================================

    /// 고급 거래 통계 조회.
    ///
    /// 연속 승/패 최대 횟수와 최대 낙폭(Max Drawdown)을 계산합니다.
    pub async fn get_advanced_stats(
        pool: &PgPool,
        credential_id: Uuid,
    ) -> Result<AdvancedTradingStats, sqlx::Error> {
        // realized_pnl이 있는 거래만 시간순 조회
        let trades: Vec<(Decimal, DateTime<Utc>)> = sqlx::query_as(
            r#"
            SELECT te.realized_pnl, ec.executed_at
            FROM trade_executions te
            JOIN execution_cache ec
                ON ec.credential_id = te.credential_id
                AND ec.exchange = te.exchange
                AND ec.trade_id = te.exchange_trade_id
            WHERE te.credential_id = $1
                AND te.realized_pnl IS NOT NULL
            ORDER BY ec.executed_at ASC
            "#,
        )
        .bind(credential_id)
        .fetch_all(pool)
        .await?;

        if trades.is_empty() {
            return Ok(AdvancedTradingStats::default());
        }

        // 연속 승/패 계산
        let mut max_consecutive_wins = 0i64;
        let mut max_consecutive_losses = 0i64;
        let mut current_wins = 0i64;
        let mut current_losses = 0i64;

        for (pnl, _) in &trades {
            if *pnl > Decimal::ZERO {
                current_wins += 1;
                current_losses = 0;
                max_consecutive_wins = max_consecutive_wins.max(current_wins);
            } else if *pnl < Decimal::ZERO {
                current_losses += 1;
                current_wins = 0;
                max_consecutive_losses = max_consecutive_losses.max(current_losses);
            }
            // pnl == 0은 무시 (손익 없음)
        }

        // Max Drawdown 계산 (누적 손익 기준)
        let mut cumulative_pnl = Decimal::ZERO;
        let mut peak = Decimal::ZERO;
        let mut max_drawdown = Decimal::ZERO;
        let mut max_drawdown_pct = Decimal::ZERO;

        for (pnl, _) in &trades {
            cumulative_pnl += pnl;

            if cumulative_pnl > peak {
                peak = cumulative_pnl;
            }

            let drawdown = peak - cumulative_pnl;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
                // 최고점 기준 낙폭 %
                if peak > Decimal::ZERO {
                    max_drawdown_pct = (drawdown / peak) * Decimal::from(100);
                }
            }
        }

        Ok(AdvancedTradingStats {
            max_consecutive_wins,
            max_consecutive_losses,
            max_drawdown,
            max_drawdown_pct,
            total_analyzed_trades: trades.len() as i64,
        })
    }
}

/// 고급 거래 통계.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdvancedTradingStats {
    /// 최대 연속 승리 횟수
    pub max_consecutive_wins: i64,
    /// 최대 연속 패배 횟수
    pub max_consecutive_losses: i64,
    /// 최대 낙폭 (절대값)
    pub max_drawdown: Decimal,
    /// 최대 낙폭 (%)
    pub max_drawdown_pct: Decimal,
    /// 분석된 거래 수
    pub total_analyzed_trades: i64,
}

/// 내부 체결 내역 행 (비용 기준 계산용).
#[derive(Debug, FromRow)]
struct ExecutionRow {
    id: Uuid,
    symbol: String,
    side: Side,
    quantity: Decimal,
    price: Decimal,
    fee: Decimal,
    executed_at: DateTime<Utc>,
}

// =====================================================
// 손익 자동 계산 (FIFO 기반)
// =====================================================

/// 손익 재계산 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecalculateResult {
    /// 처리된 종목 수
    pub symbols_processed: i32,
    /// 업데이트된 체결 수
    pub executions_updated: i32,
    /// 총 실현 손익
    pub total_realized_pnl: Decimal,
    /// 오류 목록
    pub errors: Vec<String>,
}

/// 개별 체결의 계산된 손익 정보.
#[derive(Debug, Clone)]
struct CalculatedPnL {
    execution_id: Uuid,
    realized_pnl: Option<Decimal>,
    position_effect: String,
}

impl JournalRepository {
    /// 모든 체결 내역의 손익 재계산.
    ///
    /// CostBasisTracker를 사용하여 FIFO 기반으로 각 체결의
    /// realized_pnl과 position_effect를 계산합니다.
    ///
    /// # 계산 로직
    ///
    /// 1. 각 종목별로 체결 내역을 시간순 정렬
    /// 2. 매수: 로트 추가, position_effect = "open" 또는 "add"
    /// 3. 매도: FIFO 기반 실현손익 계산, position_effect = "close" 또는 "reduce"
    /// 4. 결과를 trade_executions 테이블에 저장
    pub async fn recalculate_all_pnl(
        pool: &PgPool,
        credential_id: Uuid,
    ) -> Result<RecalculateResult, sqlx::Error> {
        let mut result = RecalculateResult {
            symbols_processed: 0,
            executions_updated: 0,
            total_realized_pnl: Decimal::ZERO,
            errors: Vec::new(),
        };

        // 1. 거래한 모든 종목 조회
        let symbols: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT DISTINCT symbol
            FROM execution_cache
            WHERE credential_id = $1
            "#,
        )
        .bind(credential_id)
        .fetch_all(pool)
        .await?;

        // 2. 각 종목별로 손익 계산
        for (symbol,) in symbols {
            match Self::recalculate_symbol_pnl(pool, credential_id, &symbol).await {
                Ok((updated, realized_pnl)) => {
                    result.symbols_processed += 1;
                    result.executions_updated += updated;
                    result.total_realized_pnl += realized_pnl;
                }
                Err(e) => {
                    result.errors.push(format!("{}: {}", symbol, e));
                }
            }
        }

        Ok(result)
    }

    /// 특정 종목의 손익 재계산.
    ///
    /// # Returns
    /// (업데이트된 체결 수, 총 실현손익)
    async fn recalculate_symbol_pnl(
        pool: &PgPool,
        credential_id: Uuid,
        symbol: &str,
    ) -> Result<(i32, Decimal), sqlx::Error> {
        // 1. 해당 종목의 모든 체결 내역 조회 (시간순)
        let executions: Vec<ExecutionRowWithExchange> = sqlx::query_as(
            r#"
            SELECT id, exchange, symbol, side::text as side_str, quantity, price,
                   COALESCE(fee, 0) as fee, executed_at, trade_id
            FROM execution_cache
            WHERE credential_id = $1 AND symbol = $2
            ORDER BY executed_at ASC
            "#,
        )
        .bind(credential_id)
        .bind(symbol)
        .fetch_all(pool)
        .await?;

        if executions.is_empty() {
            return Ok((0, Decimal::ZERO));
        }

        // 2. CostBasisTracker로 손익 계산
        let mut tracker = cost_basis::CostBasisTracker::new(symbol);
        let mut calculated_pnls: Vec<CalculatedPnL> = Vec::new();
        let mut is_first_buy = true;

        for exec in &executions {
            let side = Side::from_str_flexible(&exec.side_str).unwrap_or(Side::Buy);

            match side {
                Side::Buy => {
                    // 매수: 로트 추가
                    let lot =
                        cost_basis::Lot::new(exec.quantity, exec.price, exec.fee, exec.executed_at)
                            .with_execution_id(exec.id);
                    tracker.add_lot(lot);

                    // position_effect 결정
                    let position_effect = if is_first_buy {
                        is_first_buy = false;
                        "open"
                    } else {
                        "add"
                    };

                    calculated_pnls.push(CalculatedPnL {
                        execution_id: exec.id,
                        realized_pnl: None, // 매수는 실현손익 없음
                        position_effect: position_effect.to_string(),
                    });
                }
                Side::Sell => {
                    // 매도: FIFO 기반 실현손익 계산
                    let sale_result =
                        tracker.sell(exec.quantity, exec.price, exec.fee, exec.executed_at);

                    let (realized_pnl, position_effect) = match sale_result {
                        Ok(result) => {
                            // 남은 수량에 따라 position_effect 결정
                            let effect = if tracker.total_quantity() == Decimal::ZERO {
                                is_first_buy = true; // 청산 후 다음 매수는 다시 open
                                "close"
                            } else {
                                "reduce"
                            };
                            (Some(result.realized_pnl), effect)
                        }
                        Err(_) => {
                            // 보유 수량 부족 (공매도 또는 데이터 오류)
                            // 실현손익 계산 불가
                            (None, "unknown")
                        }
                    };

                    calculated_pnls.push(CalculatedPnL {
                        execution_id: exec.id,
                        realized_pnl,
                        position_effect: position_effect.to_string(),
                    });
                }
            }
        }

        // 3. 계산 결과를 trade_executions 테이블에 저장
        let mut updated = 0;
        let mut total_realized = Decimal::ZERO;

        for pnl_info in &calculated_pnls {
            if let Some(pnl) = pnl_info.realized_pnl {
                total_realized += pnl;
            }

            // 해당 execution_cache의 정보로 trade_executions에 upsert
            let exec = executions.iter().find(|e| e.id == pnl_info.execution_id);
            if let Some(exec) = exec {
                let affected = sqlx::query(
                    r#"
                    INSERT INTO trade_executions (
                        credential_id, exchange, symbol, side, order_type,
                        quantity, price, notional_value, fee,
                        position_effect, realized_pnl,
                        exchange_trade_id, executed_at
                    )
                    SELECT
                        $1, $2, $3, side, COALESCE(order_type, 'market'),
                        quantity, price, amount, COALESCE(fee, 0),
                        $4, $5, trade_id, executed_at
                    FROM execution_cache
                    WHERE id = $6
                    ON CONFLICT (credential_id, exchange, exchange_trade_id)
                    DO UPDATE SET
                        position_effect = EXCLUDED.position_effect,
                        realized_pnl = EXCLUDED.realized_pnl,
                        updated_at = NOW()
                    "#,
                )
                .bind(credential_id)
                .bind(&exec.exchange)
                .bind(symbol)
                .bind(&pnl_info.position_effect)
                .bind(pnl_info.realized_pnl)
                .bind(exec.id)
                .execute(pool)
                .await?;

                if affected.rows_affected() > 0 {
                    updated += 1;
                }
            }
        }

        Ok((updated, total_realized))
    }
}

/// 확장된 체결 내역 행 (exchange, trade_id 포함).
#[allow(dead_code)]
#[derive(Debug, FromRow)]
struct ExecutionRowWithExchange {
    id: Uuid,
    exchange: String,
    symbol: String,
    side_str: String,
    quantity: Decimal,
    price: Decimal,
    fee: Decimal,
    executed_at: DateTime<Utc>,
    trade_id: Option<String>,
}
