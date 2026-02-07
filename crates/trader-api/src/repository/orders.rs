//! 주문 저장소.
//!
//! 주문 생성, 조회, 상태 업데이트를 위한 데이터베이스 작업을 처리합니다.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// 주문 레코드.
///
/// orders 테이블의 데이터베이스 표현입니다.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Order {
    pub id: Uuid,
    pub exchange: String,
    pub exchange_order_id: Option<String>,
    pub symbol_id: Uuid,
    pub side: String,
    pub order_type: String,
    pub status: String,
    pub time_in_force: Option<String>,
    pub quantity: Decimal,
    pub filled_quantity: Option<Decimal>,
    pub price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub average_fill_price: Option<Decimal>,
    pub strategy_id: Option<String>,
    pub client_order_id: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub filled_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub metadata: Option<Value>,
}

/// 새 주문 생성 입력.
#[derive(Debug, Clone)]
pub struct OrderInput {
    pub exchange: String,
    pub symbol_id: Uuid,
    pub side: String,
    pub order_type: String,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub time_in_force: Option<String>,
    pub strategy_id: Option<String>,
    pub client_order_id: Option<String>,
    pub metadata: Option<Value>,
}

/// 주문 상태 열거형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

impl OrderStatus {
    /// 문자열로 변환.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Open => "open",
            Self::PartiallyFilled => "partially_filled",
            Self::Filled => "filled",
            Self::Cancelled => "cancelled",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
        }
    }
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 주문 저장소.
pub struct OrderRepository;

impl OrderRepository {
    /// 새 주문 생성.
    ///
    /// 트랜잭션을 사용하여 원자성을 보장합니다.
    pub async fn create_order(pool: &PgPool, input: OrderInput) -> Result<Order, sqlx::Error> {
        let mut tx = pool.begin().await?;

        let time_in_force = input.time_in_force.unwrap_or_else(|| "gtc".to_string());

        let record = sqlx::query_as::<_, Order>(
            r#"
            INSERT INTO orders (
                exchange, symbol_id, side, order_type, status,
                time_in_force, quantity, price, stop_price,
                strategy_id, client_order_id, metadata
            )
            VALUES ($1, $2, $3, $4, 'pending', $5, $6, $7, $8, $9, $10, $11)
            RETURNING *
            "#,
        )
        .bind(&input.exchange)
        .bind(input.symbol_id)
        .bind(&input.side)
        .bind(&input.order_type)
        .bind(&time_in_force)
        .bind(input.quantity)
        .bind(input.price)
        .bind(input.stop_price)
        .bind(&input.strategy_id)
        .bind(&input.client_order_id)
        .bind(&input.metadata)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(record)
    }

    /// 전략의 주문 목록 조회.
    ///
    /// limit 파라미터로 반환할 최대 개수를 지정합니다.
    pub async fn list_orders(
        pool: &PgPool,
        strategy_id: &str,
        limit: i64,
    ) -> Result<Vec<Order>, sqlx::Error> {
        let records = sqlx::query_as::<_, Order>(
            r#"
            SELECT
                id, exchange, exchange_order_id, symbol_id,
                side, order_type, status, time_in_force,
                quantity, filled_quantity, price, stop_price,
                average_fill_price, strategy_id, client_order_id,
                created_at, updated_at, filled_at, cancelled_at, metadata
            FROM orders
            WHERE strategy_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(strategy_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 주문 상태 업데이트.
    ///
    /// 트랜잭션을 사용하여 원자성을 보장합니다.
    /// 상태에 따라 filled_at 또는 cancelled_at 타임스탬프도 업데이트합니다.
    pub async fn update_order_status(
        pool: &PgPool,
        order_id: Uuid,
        status: OrderStatus,
    ) -> Result<Order, sqlx::Error> {
        let mut tx = pool.begin().await?;

        // 상태에 따라 타임스탬프 필드 결정
        let record = match status {
            OrderStatus::Filled => {
                sqlx::query_as::<_, Order>(
                    r#"
                    UPDATE orders
                    SET status = $2, filled_at = NOW(), updated_at = NOW()
                    WHERE id = $1
                    RETURNING *
                    "#,
                )
                .bind(order_id)
                .bind(status.as_str())
                .fetch_one(&mut *tx)
                .await?
            }
            OrderStatus::Cancelled | OrderStatus::Rejected | OrderStatus::Expired => {
                sqlx::query_as::<_, Order>(
                    r#"
                    UPDATE orders
                    SET status = $2, cancelled_at = NOW(), updated_at = NOW()
                    WHERE id = $1
                    RETURNING *
                    "#,
                )
                .bind(order_id)
                .bind(status.as_str())
                .fetch_one(&mut *tx)
                .await?
            }
            _ => {
                sqlx::query_as::<_, Order>(
                    r#"
                    UPDATE orders
                    SET status = $2, updated_at = NOW()
                    WHERE id = $1
                    RETURNING *
                    "#,
                )
                .bind(order_id)
                .bind(status.as_str())
                .fetch_one(&mut *tx)
                .await?
            }
        };

        tx.commit().await?;

        Ok(record)
    }

    /// 주문 ID로 조회.
    pub async fn get_by_id(pool: &PgPool, order_id: Uuid) -> Result<Option<Order>, sqlx::Error> {
        let record = sqlx::query_as::<_, Order>(
            r#"
            SELECT
                id, exchange, exchange_order_id, symbol_id,
                side, order_type, status, time_in_force,
                quantity, filled_quantity, price, stop_price,
                average_fill_price, strategy_id, client_order_id,
                created_at, updated_at, filled_at, cancelled_at, metadata
            FROM orders
            WHERE id = $1
            "#,
        )
        .bind(order_id)
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    /// 활성 주문 조회 (pending, open, partially_filled).
    pub async fn get_active_orders(
        pool: &PgPool,
        strategy_id: &str,
    ) -> Result<Vec<Order>, sqlx::Error> {
        let records = sqlx::query_as::<_, Order>(
            r#"
            SELECT
                id, exchange, exchange_order_id, symbol_id,
                side, order_type, status, time_in_force,
                quantity, filled_quantity, price, stop_price,
                average_fill_price, strategy_id, client_order_id,
                created_at, updated_at, filled_at, cancelled_at, metadata
            FROM orders
            WHERE strategy_id = $1
              AND status IN ('pending', 'open', 'partially_filled')
            ORDER BY created_at DESC
            "#,
        )
        .bind(strategy_id)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 거래소 주문 ID로 조회.
    pub async fn get_by_exchange_order_id(
        pool: &PgPool,
        exchange: &str,
        exchange_order_id: &str,
    ) -> Result<Option<Order>, sqlx::Error> {
        let record = sqlx::query_as::<_, Order>(
            r#"
            SELECT
                id, exchange, exchange_order_id, symbol_id,
                side, order_type, status, time_in_force,
                quantity, filled_quantity, price, stop_price,
                average_fill_price, strategy_id, client_order_id,
                created_at, updated_at, filled_at, cancelled_at, metadata
            FROM orders
            WHERE exchange = $1 AND exchange_order_id = $2
            "#,
        )
        .bind(exchange)
        .bind(exchange_order_id)
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    /// 거래소 주문 ID 설정.
    ///
    /// 주문이 거래소에 제출된 후 거래소가 발급한 ID를 저장합니다.
    /// 상태 변경과 ID 설정을 트랜잭션으로 묶어 원자성을 보장합니다.
    pub async fn set_exchange_order_id(
        pool: &PgPool,
        order_id: Uuid,
        exchange_order_id: &str,
    ) -> Result<Order, sqlx::Error> {
        // 트랜잭션으로 상태 변경과 ID 설정의 원자성 보장
        let mut tx = pool.begin().await?;

        let record = sqlx::query_as::<_, Order>(
            r#"
            UPDATE orders
            SET exchange_order_id = $2, status = 'open', updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(order_id)
        .bind(exchange_order_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(record)
    }

    /// 체결 수량 업데이트.
    pub async fn update_filled_quantity(
        pool: &PgPool,
        order_id: Uuid,
        filled_quantity: Decimal,
        average_fill_price: Decimal,
    ) -> Result<Order, sqlx::Error> {
        let mut tx = pool.begin().await?;

        // 먼저 현재 주문 정보 조회
        let current: Order = sqlx::query_as("SELECT * FROM orders WHERE id = $1")
            .bind(order_id)
            .fetch_one(&mut *tx)
            .await?;

        // 전량 체결 여부 확인
        let new_status = if filled_quantity >= current.quantity {
            "filled"
        } else if filled_quantity > Decimal::ZERO {
            "partially_filled"
        } else {
            "open"
        };

        let record = if new_status == "filled" {
            sqlx::query_as::<_, Order>(
                r#"
                UPDATE orders
                SET filled_quantity = $2, average_fill_price = $3,
                    status = $4, filled_at = NOW(), updated_at = NOW()
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(order_id)
            .bind(filled_quantity)
            .bind(average_fill_price)
            .bind(new_status)
            .fetch_one(&mut *tx)
            .await?
        } else {
            sqlx::query_as::<_, Order>(
                r#"
                UPDATE orders
                SET filled_quantity = $2, average_fill_price = $3,
                    status = $4, updated_at = NOW()
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(order_id)
            .bind(filled_quantity)
            .bind(average_fill_price)
            .bind(new_status)
            .fetch_one(&mut *tx)
            .await?
        };

        tx.commit().await?;

        Ok(record)
    }
}
