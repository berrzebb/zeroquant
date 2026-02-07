//! 포지션 저장소.
//!
//! 포지션 열기, 닫기, 조회를 위한 데이터베이스 작업을 처리합니다.
//! 거래소 API에서 가져온 보유 현황을 DB에 동기화하는 기능도 제공합니다.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use trader_core::{unrealized_pnl, Side};
use uuid::Uuid;

/// 포지션 레코드.
///
/// positions 테이블의 데이터베이스 표현입니다.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PositionRecord {
    pub id: Uuid,
    pub credential_id: Option<Uuid>,
    pub exchange: String,
    pub symbol_id: Uuid,
    pub symbol: Option<String>,
    pub symbol_name: Option<String>,
    pub side: Side,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub current_price: Option<Decimal>,
    pub unrealized_pnl: Option<Decimal>,
    pub realized_pnl: Option<Decimal>,
    pub strategy_id: Option<String>,
    pub opened_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub metadata: Option<Value>,
}

/// 새 포지션 생성 입력.
#[derive(Debug, Clone)]
pub struct PositionInput {
    pub credential_id: Option<Uuid>,
    pub exchange: String,
    pub symbol_id: Uuid,
    pub symbol: Option<String>,
    pub symbol_name: Option<String>,
    pub side: Side,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub strategy_id: Option<String>,
    pub metadata: Option<Value>,
}

/// 거래소에서 가져온 보유 현황 (동기화용).
#[derive(Debug, Clone)]
pub struct HoldingPosition {
    /// 거래소 자격증명 ID
    pub credential_id: Uuid,
    /// 거래소 구분 (kis, binance 등)
    pub exchange: String,
    /// 심볼 (종목 코드)
    pub symbol: String,
    /// 종목명
    pub symbol_name: String,
    /// 보유 수량
    pub quantity: Decimal,
    /// 매입 평균가
    pub avg_price: Decimal,
    /// 현재가
    pub current_price: Decimal,
    /// 평가손익
    pub profit_loss: Decimal,
    /// 수익률 (%)
    pub profit_loss_rate: Decimal,
    /// 시장 (KR/US)
    pub market: String,
}

/// 동기화 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub synced: i32,
    pub closed: i32,
}

/// 포지션 저장소.
pub struct PositionRepository;

impl PositionRepository {
    /// 새 포지션 열기.
    ///
    /// 트랜잭션을 사용하여 원자성을 보장합니다.
    pub async fn open_position(
        pool: &PgPool,
        input: PositionInput,
    ) -> Result<PositionRecord, sqlx::Error> {
        let mut tx = pool.begin().await?;

        let record = sqlx::query_as::<_, PositionRecord>(
            r#"
            INSERT INTO positions (
                exchange, symbol_id, side, quantity, entry_price,
                current_price, strategy_id, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(&input.exchange)
        .bind(input.symbol_id)
        .bind(input.side)
        .bind(input.quantity)
        .bind(input.entry_price)
        .bind(&input.strategy_id)
        .bind(&input.metadata)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(record)
    }

    /// 포지션 닫기.
    ///
    /// 트랜잭션을 사용하여 포지션 종료와 손익 계산을 원자적으로 처리합니다.
    /// 실현 손익은 (종가 - 진입가) * 수량으로 계산됩니다 (롱 포지션 기준).
    pub async fn close_position(
        pool: &PgPool,
        position_id: Uuid,
        close_price: Decimal,
    ) -> Result<PositionRecord, sqlx::Error> {
        let mut tx = pool.begin().await?;

        // 먼저 현재 포지션 정보 조회
        let current: PositionRecord = sqlx::query_as("SELECT * FROM positions WHERE id = $1")
            .bind(position_id)
            .fetch_one(&mut *tx)
            .await?;

        // 실현 손익 계산
        // 롱 포지션: (종가 - 진입가) * 수량
        // 숏 포지션: (진입가 - 종가) * 수량
        let realized_pnl = match current.side {
            Side::Buy => (close_price - current.entry_price) * current.quantity,
            Side::Sell => (current.entry_price - close_price) * current.quantity,
        };

        // 기존 실현 손익과 합산
        let total_realized_pnl = current.realized_pnl.unwrap_or_default() + realized_pnl;

        let record = sqlx::query_as::<_, PositionRecord>(
            r#"
            UPDATE positions
            SET
                current_price = $2,
                realized_pnl = $3,
                unrealized_pnl = 0,
                closed_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(position_id)
        .bind(close_price)
        .bind(total_realized_pnl)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(record)
    }

    /// 포지션 ID로 조회.
    pub async fn get_position(
        pool: &PgPool,
        position_id: Uuid,
    ) -> Result<Option<PositionRecord>, sqlx::Error> {
        let record = sqlx::query_as::<_, PositionRecord>(
            r#"
            SELECT
                id, exchange, symbol_id, side, quantity,
                entry_price, current_price, unrealized_pnl, realized_pnl,
                strategy_id, opened_at, updated_at, closed_at, metadata
            FROM positions
            WHERE id = $1
            "#,
        )
        .bind(position_id)
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    /// 심볼과 전략으로 열린 포지션 조회.
    ///
    /// 동일 심볼에 대해 이미 열린 포지션이 있는지 확인할 때 사용합니다.
    pub async fn get_open_position_by_symbol(
        pool: &PgPool,
        strategy_id: &str,
        symbol_id: Uuid,
    ) -> Result<Option<PositionRecord>, sqlx::Error> {
        let record = sqlx::query_as::<_, PositionRecord>(
            r#"
            SELECT
                id, exchange, symbol_id, side, quantity,
                entry_price, current_price, unrealized_pnl, realized_pnl,
                strategy_id, opened_at, updated_at, closed_at, metadata
            FROM positions
            WHERE strategy_id = $1 AND symbol_id = $2 AND closed_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(strategy_id)
        .bind(symbol_id)
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    /// 포지션 수량 추가 (피라미딩).
    ///
    /// 기존 포지션에 수량을 추가하고 평균 진입가를 재계산합니다.
    pub async fn add_to_position(
        pool: &PgPool,
        position_id: Uuid,
        additional_quantity: Decimal,
        additional_price: Decimal,
    ) -> Result<PositionRecord, sqlx::Error> {
        let mut tx = pool.begin().await?;

        // 현재 포지션 조회
        let current: PositionRecord = sqlx::query_as("SELECT * FROM positions WHERE id = $1")
            .bind(position_id)
            .fetch_one(&mut *tx)
            .await?;

        // 새로운 평균 진입가 계산
        // (기존수량 * 기존가격 + 추가수량 * 추가가격) / (기존수량 + 추가수량)
        let new_quantity = current.quantity + additional_quantity;
        let new_entry_price = (current.quantity * current.entry_price
            + additional_quantity * additional_price)
            / new_quantity;

        let record = sqlx::query_as::<_, PositionRecord>(
            r#"
            UPDATE positions
            SET
                quantity = $2,
                entry_price = $3,
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(position_id)
        .bind(new_quantity)
        .bind(new_entry_price)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(record)
    }

    /// 포지션 수량 감소 (부분 청산).
    ///
    /// 트랜잭션을 사용하여 수량 감소와 부분 손익 실현을 원자적으로 처리합니다.
    pub async fn reduce_position(
        pool: &PgPool,
        position_id: Uuid,
        reduce_quantity: Decimal,
        close_price: Decimal,
    ) -> Result<PositionRecord, sqlx::Error> {
        let mut tx = pool.begin().await?;

        // 현재 포지션 조회
        let current: PositionRecord = sqlx::query_as("SELECT * FROM positions WHERE id = $1")
            .bind(position_id)
            .fetch_one(&mut *tx)
            .await?;

        // 부분 실현 손익 계산
        let partial_pnl = match current.side {
            Side::Buy => (close_price - current.entry_price) * reduce_quantity,
            Side::Sell => (current.entry_price - close_price) * reduce_quantity,
        };

        let new_quantity = current.quantity - reduce_quantity;
        let total_realized_pnl = current.realized_pnl.unwrap_or_default() + partial_pnl;

        // 수량이 0이 되면 포지션 종료
        let record = if new_quantity <= Decimal::ZERO {
            sqlx::query_as::<_, PositionRecord>(
                r#"
                UPDATE positions
                SET
                    quantity = 0,
                    current_price = $2,
                    realized_pnl = $3,
                    unrealized_pnl = 0,
                    closed_at = NOW(),
                    updated_at = NOW()
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(position_id)
            .bind(close_price)
            .bind(total_realized_pnl)
            .fetch_one(&mut *tx)
            .await?
        } else {
            sqlx::query_as::<_, PositionRecord>(
                r#"
                UPDATE positions
                SET
                    quantity = $2,
                    current_price = $3,
                    realized_pnl = $4,
                    updated_at = NOW()
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(position_id)
            .bind(new_quantity)
            .bind(close_price)
            .bind(total_realized_pnl)
            .fetch_one(&mut *tx)
            .await?
        };

        tx.commit().await?;

        Ok(record)
    }

    /// 포지션의 현재가 및 미실현 손익 업데이트.
    ///
    /// 조회-계산-업데이트를 트랜잭션으로 묶어 원자성을 보장합니다.
    pub async fn update_market_price(
        pool: &PgPool,
        position_id: Uuid,
        current_price: Decimal,
    ) -> Result<PositionRecord, sqlx::Error> {
        // 트랜잭션 시작 (조회-계산-업데이트 원자성 보장)
        let mut tx = pool.begin().await?;

        // 먼저 현재 포지션 조회 (트랜잭션 내에서)
        let current: PositionRecord =
            sqlx::query_as("SELECT * FROM positions WHERE id = $1 FOR UPDATE")
                .bind(position_id)
                .fetch_one(&mut *tx)
                .await?;

        // 미실현 손익 계산
        let unrealized_pnl_value = unrealized_pnl(
            current.entry_price,
            current_price,
            current.quantity,
            current.side,
        );

        let record = sqlx::query_as::<_, PositionRecord>(
            r#"
            UPDATE positions
            SET
                current_price = $2,
                unrealized_pnl = $3,
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(position_id)
        .bind(current_price)
        .bind(unrealized_pnl_value)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(record)
    }

    /// 포지션 존재 여부 확인.
    pub async fn exists(pool: &PgPool, position_id: Uuid) -> Result<bool, sqlx::Error> {
        let result: (bool,) =
            sqlx::query_as("SELECT EXISTS(SELECT 1 FROM positions WHERE id = $1)")
                .bind(position_id)
                .fetch_one(pool)
                .await?;

        Ok(result.0)
    }

    /// 열린 포지션 여부 확인.
    pub async fn is_open(pool: &PgPool, position_id: Uuid) -> Result<bool, sqlx::Error> {
        let result: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM positions WHERE id = $1 AND closed_at IS NULL)",
        )
        .bind(position_id)
        .fetch_one(pool)
        .await?;

        Ok(result.0)
    }

    // =====================================================
    // 거래소 데이터 동기화
    // =====================================================

    /// 거래소 보유 현황을 positions 테이블에 동기화.
    ///
    /// 거래소 API에서 가져온 보유 현황을 DB에 저장합니다.
    /// - 새로운 종목: INSERT
    /// - 기존 종목: 현재가, 평가손익 UPDATE
    /// - 거래소에 없는 종목: closed_at 설정 (청산됨)
    pub async fn sync_holdings(
        pool: &PgPool,
        credential_id: Uuid,
        exchange: &str,
        holdings: Vec<HoldingPosition>,
    ) -> Result<SyncResult, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let mut synced = 0;
        let mut closed = 0;

        // 현재 열린 포지션의 심볼 목록 조회
        let current_symbols: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT symbol FROM positions
            WHERE credential_id = $1 AND exchange = $2 AND closed_at IS NULL
            "#,
        )
        .bind(credential_id)
        .bind(exchange)
        .fetch_all(&mut *tx)
        .await?;

        let current_symbol_set: std::collections::HashSet<String> =
            current_symbols.into_iter().map(|(s,)| s).collect();

        // 거래소에서 가져온 심볼 목록
        let incoming_symbols: std::collections::HashSet<String> =
            holdings.iter().map(|h| h.symbol.clone()).collect();

        // 1. 각 보유 종목 처리
        for holding in &holdings {
            // 미실현 손익 계산 (side는 보유 = buy로 가정)
            let unrealized_pnl = holding.profit_loss;

            // 기존 포지션 확인
            let existing: Option<(Uuid,)> = sqlx::query_as(
                r#"
                SELECT id FROM positions
                WHERE credential_id = $1 AND exchange = $2 AND symbol = $3 AND closed_at IS NULL
                LIMIT 1
                "#,
            )
            .bind(credential_id)
            .bind(exchange)
            .bind(&holding.symbol)
            .fetch_optional(&mut *tx)
            .await?;

            if let Some((position_id,)) = existing {
                // 기존 포지션 업데이트
                sqlx::query(
                    r#"
                    UPDATE positions
                    SET
                        quantity = $2,
                        entry_price = $3,
                        current_price = $4,
                        unrealized_pnl = $5,
                        symbol_name = $6,
                        updated_at = NOW()
                    WHERE id = $1
                    "#,
                )
                .bind(position_id)
                .bind(holding.quantity)
                .bind(holding.avg_price)
                .bind(holding.current_price)
                .bind(unrealized_pnl)
                .bind(&holding.symbol_name)
                .execute(&mut *tx)
                .await?;
            } else {
                // 새 포지션 생성 (symbol_id는 NULL 허용 또는 기본값 사용)
                // 주의: symbol_id FK가 있으므로 symbols 테이블에서 조회하거나 생성해야 함
                // 여기서는 간단히 symbol 컬럼만 사용
                sqlx::query(
                    r#"
                    INSERT INTO positions (
                        credential_id, exchange, symbol, symbol_name,
                        side, quantity, entry_price, current_price,
                        unrealized_pnl, symbol_id
                    )
                    VALUES (
                        $1, $2, $3, $4,
                        'buy', $5, $6, $7,
                        $8,
                        COALESCE(
                            (SELECT id FROM symbols WHERE base = $3 OR quote = $3 LIMIT 1),
                            (SELECT id FROM symbols LIMIT 1)
                        )
                    )
                    "#,
                )
                .bind(credential_id)
                .bind(exchange)
                .bind(&holding.symbol)
                .bind(&holding.symbol_name)
                .bind(holding.quantity)
                .bind(holding.avg_price)
                .bind(holding.current_price)
                .bind(unrealized_pnl)
                .execute(&mut *tx)
                .await?;
            }
            synced += 1;
        }

        // 2. 거래소에 없는 포지션 청산 처리
        for symbol in current_symbol_set.difference(&incoming_symbols) {
            sqlx::query(
                r#"
                UPDATE positions
                SET closed_at = NOW(), updated_at = NOW(), quantity = 0
                WHERE credential_id = $1 AND exchange = $2 AND symbol = $3 AND closed_at IS NULL
                "#,
            )
            .bind(credential_id)
            .bind(exchange)
            .bind(symbol)
            .execute(&mut *tx)
            .await?;
            closed += 1;
        }

        tx.commit().await?;

        Ok(SyncResult { synced, closed })
    }

    /// credential_id로 열린 포지션 조회.
    pub async fn get_open_positions_by_credential(
        pool: &PgPool,
        credential_id: Uuid,
    ) -> Result<Vec<PositionRecord>, sqlx::Error> {
        let records = sqlx::query_as::<_, PositionRecord>(
            r#"
            SELECT
                id, credential_id, exchange, symbol_id, symbol, symbol_name,
                side::text as side, quantity, entry_price, current_price,
                unrealized_pnl, realized_pnl, strategy_id,
                opened_at, updated_at, closed_at, metadata
            FROM positions
            WHERE credential_id = $1 AND closed_at IS NULL AND quantity > 0
            ORDER BY updated_at DESC
            "#,
        )
        .bind(credential_id)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 모든 열린 포지션 조회 (credential_id 무관).
    pub async fn get_all_open_positions(pool: &PgPool) -> Result<Vec<PositionRecord>, sqlx::Error> {
        let records = sqlx::query_as::<_, PositionRecord>(
            r#"
            SELECT
                id, credential_id, exchange, symbol_id, symbol, symbol_name,
                side::text as side, quantity, entry_price, current_price,
                unrealized_pnl, realized_pnl, strategy_id,
                opened_at, updated_at, closed_at, metadata
            FROM positions
            WHERE closed_at IS NULL AND quantity > 0
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }
}
