//! 포트폴리오 저장소.
//!
//! 전략별 포지션 관리를 위한 데이터베이스 작업을 처리합니다.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// 포지션 레코드.
///
/// positions 테이블의 데이터베이스 표현입니다.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Position {
    pub id: Uuid,
    pub exchange: String,
    pub symbol_id: Uuid,
    pub side: String,
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

/// 포지션 업데이트 입력.
#[derive(Debug, Clone, Default)]
pub struct PositionUpdate {
    /// 현재 가격 업데이트
    pub current_price: Option<Decimal>,
    /// 수량 업데이트
    pub quantity: Option<Decimal>,
    /// 미실현 손익 업데이트
    pub unrealized_pnl: Option<Decimal>,
    /// 실현 손익 업데이트
    pub realized_pnl: Option<Decimal>,
    /// 메타데이터 업데이트
    pub metadata: Option<Value>,
}

/// 포트폴리오 저장소.
pub struct PortfolioRepository;

impl PortfolioRepository {
    /// 전략의 현재 열린 포지션 목록 조회.
    ///
    /// closed_at이 NULL인 포지션만 반환합니다.
    pub async fn get_current_positions(
        pool: &PgPool,
        strategy_id: &str,
    ) -> Result<Vec<Position>, sqlx::Error> {
        let records = sqlx::query_as::<_, Position>(
            r#"
            SELECT
                id, exchange, symbol_id, side, quantity,
                entry_price, current_price, unrealized_pnl, realized_pnl,
                strategy_id, opened_at, updated_at, closed_at, metadata
            FROM positions
            WHERE strategy_id = $1 AND closed_at IS NULL
            ORDER BY opened_at DESC
            "#,
        )
        .bind(strategy_id)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 포지션 정보 업데이트.
    ///
    /// 트랜잭션을 사용하여 원자성을 보장합니다.
    pub async fn update_position(
        pool: &PgPool,
        position_id: Uuid,
        updates: PositionUpdate,
    ) -> Result<Position, sqlx::Error> {
        let mut tx = pool.begin().await?;

        // 동적 쿼리 생성 - 변경된 필드만 업데이트
        let record = sqlx::query_as::<_, Position>(
            r#"
            UPDATE positions
            SET
                current_price = COALESCE($2, current_price),
                quantity = COALESCE($3, quantity),
                unrealized_pnl = COALESCE($4, unrealized_pnl),
                realized_pnl = COALESCE($5, realized_pnl),
                metadata = COALESCE($6, metadata),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(position_id)
        .bind(updates.current_price)
        .bind(updates.quantity)
        .bind(updates.unrealized_pnl)
        .bind(updates.realized_pnl)
        .bind(updates.metadata)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(record)
    }

    /// 전략의 모든 포지션 조회 (닫힌 포지션 포함).
    pub async fn get_all_positions(
        pool: &PgPool,
        strategy_id: &str,
    ) -> Result<Vec<Position>, sqlx::Error> {
        let records = sqlx::query_as::<_, Position>(
            r#"
            SELECT
                id, exchange, symbol_id, side, quantity,
                entry_price, current_price, unrealized_pnl, realized_pnl,
                strategy_id, opened_at, updated_at, closed_at, metadata
            FROM positions
            WHERE strategy_id = $1
            ORDER BY opened_at DESC
            "#,
        )
        .bind(strategy_id)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 거래소별 열린 포지션 조회.
    pub async fn get_positions_by_exchange(
        pool: &PgPool,
        exchange: &str,
    ) -> Result<Vec<Position>, sqlx::Error> {
        let records = sqlx::query_as::<_, Position>(
            r#"
            SELECT
                id, exchange, symbol_id, side, quantity,
                entry_price, current_price, unrealized_pnl, realized_pnl,
                strategy_id, opened_at, updated_at, closed_at, metadata
            FROM positions
            WHERE exchange = $1 AND closed_at IS NULL
            ORDER BY opened_at DESC
            "#,
        )
        .bind(exchange)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 전략의 총 미실현 손익 계산.
    pub async fn get_total_unrealized_pnl(
        pool: &PgPool,
        strategy_id: &str,
    ) -> Result<Decimal, sqlx::Error> {
        let result: (Option<Decimal>,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(unrealized_pnl), 0)
            FROM positions
            WHERE strategy_id = $1 AND closed_at IS NULL
            "#,
        )
        .bind(strategy_id)
        .fetch_one(pool)
        .await?;

        Ok(result.0.unwrap_or_default())
    }

    /// 전략의 총 실현 손익 계산.
    pub async fn get_total_realized_pnl(
        pool: &PgPool,
        strategy_id: &str,
    ) -> Result<Decimal, sqlx::Error> {
        let result: (Option<Decimal>,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(realized_pnl), 0)
            FROM positions
            WHERE strategy_id = $1
            "#,
        )
        .bind(strategy_id)
        .fetch_one(pool)
        .await?;

        Ok(result.0.unwrap_or_default())
    }
}
