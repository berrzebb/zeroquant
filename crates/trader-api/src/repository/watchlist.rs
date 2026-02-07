//! Watchlist Repository
//!
//! 관심종목 관련 데이터베이스 연산을 담당합니다.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;
use uuid::Uuid;

// ================================================================================================
// Types
// ================================================================================================

/// 관심종목 그룹 레코드
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct WatchlistRecord {
    pub id: Uuid,
    pub name: String,
    #[sqlx(default)]
    pub description: Option<String>,
    pub sort_order: i32,
    #[sqlx(default)]
    pub color: Option<String>,
    #[sqlx(default)]
    pub icon: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 관심종목 아이템 레코드
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct WatchlistItemRecord {
    pub id: Uuid,
    pub watchlist_id: Uuid,
    pub symbol: String,
    pub market: String,
    #[sqlx(default)]
    pub memo: Option<String>,
    #[sqlx(default)]
    pub target_price: Option<Decimal>,
    #[sqlx(default)]
    pub stop_price: Option<Decimal>,
    #[sqlx(default)]
    pub alert_enabled: Option<bool>,
    pub sort_order: i32,
    #[sqlx(default)]
    pub added_price: Option<Decimal>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 새 관심종목 그룹 입력
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewWatchlist {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

/// 새 관심종목 아이템 입력
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewWatchlistItem {
    pub symbol: String,
    #[serde(default = "default_market")]
    pub market: String,
    #[serde(default)]
    pub memo: Option<String>,
    #[serde(default)]
    pub target_price: Option<Decimal>,
    #[serde(default)]
    pub stop_price: Option<Decimal>,
    #[serde(default)]
    pub added_price: Option<Decimal>,
}

fn default_market() -> String {
    "KR".to_string()
}

/// 아이템 업데이트 입력
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateWatchlistItem {
    #[serde(default)]
    pub memo: Option<String>,
    #[serde(default)]
    pub target_price: Option<Decimal>,
    #[serde(default)]
    pub stop_price: Option<Decimal>,
    #[serde(default)]
    pub alert_enabled: Option<bool>,
}

/// 관심종목 그룹 + 아이템 수
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct WatchlistWithCount {
    pub id: Uuid,
    pub name: String,
    #[sqlx(default)]
    pub description: Option<String>,
    pub sort_order: i32,
    #[sqlx(default)]
    pub color: Option<String>,
    #[sqlx(default)]
    pub icon: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub item_count: i64,
}

// ================================================================================================
// Repository
// ================================================================================================

/// Watchlist Repository
pub struct WatchlistRepository;

impl WatchlistRepository {
    // ============================================================================================
    // Watchlist (Group) Operations
    // ============================================================================================

    /// 모든 관심종목 그룹 조회 (아이템 수 포함)
    pub async fn get_all_watchlists(pool: &PgPool) -> Result<Vec<WatchlistWithCount>, sqlx::Error> {
        let records = sqlx::query_as::<_, WatchlistWithCount>(
            r#"
            SELECT
                w.id, w.name, w.description, w.sort_order, w.color, w.icon,
                w.created_at, w.updated_at,
                COALESCE(COUNT(wi.id), 0) as item_count
            FROM watchlist w
            LEFT JOIN watchlist_item wi ON w.id = wi.watchlist_id
            GROUP BY w.id
            ORDER BY w.sort_order, w.name
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 관심종목 그룹 상세 조회
    pub async fn get_watchlist_by_id(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<Option<WatchlistRecord>, sqlx::Error> {
        let record = sqlx::query_as::<_, WatchlistRecord>("SELECT * FROM watchlist WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(record)
    }

    /// 관심종목 그룹 생성
    pub async fn create_watchlist(
        pool: &PgPool,
        input: NewWatchlist,
    ) -> Result<WatchlistRecord, sqlx::Error> {
        // 현재 최대 sort_order 조회
        let max_order: Option<i32> = sqlx::query_scalar("SELECT MAX(sort_order) FROM watchlist")
            .fetch_one(pool)
            .await?;

        let next_order = max_order.unwrap_or(-1) + 1;

        let record = sqlx::query_as::<_, WatchlistRecord>(
            r#"
            INSERT INTO watchlist (name, description, color, icon, sort_order)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.color)
        .bind(&input.icon)
        .bind(next_order)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    /// 관심종목 그룹 삭제 (CASCADE로 아이템도 삭제됨)
    pub async fn delete_watchlist(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM watchlist WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// 관심종목 그룹 이름 변경
    pub async fn update_watchlist_name(
        pool: &PgPool,
        id: Uuid,
        name: &str,
    ) -> Result<Option<WatchlistRecord>, sqlx::Error> {
        let record = sqlx::query_as::<_, WatchlistRecord>(
            r#"
            UPDATE watchlist
            SET name = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(name)
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    // ============================================================================================
    // Watchlist Item Operations
    // ============================================================================================

    /// 그룹 내 모든 아이템 조회
    pub async fn get_items_by_watchlist(
        pool: &PgPool,
        watchlist_id: Uuid,
    ) -> Result<Vec<WatchlistItemRecord>, sqlx::Error> {
        let records = sqlx::query_as::<_, WatchlistItemRecord>(
            r#"
            SELECT * FROM watchlist_item
            WHERE watchlist_id = $1
            ORDER BY sort_order, created_at
            "#,
        )
        .bind(watchlist_id)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 아이템 추가
    pub async fn add_item(
        pool: &PgPool,
        watchlist_id: Uuid,
        input: NewWatchlistItem,
    ) -> Result<WatchlistItemRecord, sqlx::Error> {
        // 현재 최대 sort_order 조회
        let max_order: Option<i32> = sqlx::query_scalar(
            "SELECT MAX(sort_order) FROM watchlist_item WHERE watchlist_id = $1",
        )
        .bind(watchlist_id)
        .fetch_one(pool)
        .await?;

        let next_order = max_order.unwrap_or(-1) + 1;

        let record = sqlx::query_as::<_, WatchlistItemRecord>(
            r#"
            INSERT INTO watchlist_item
                (watchlist_id, symbol, market, memo, target_price, stop_price, added_price, sort_order)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (watchlist_id, symbol, market) DO UPDATE
            SET
                memo = COALESCE(EXCLUDED.memo, watchlist_item.memo),
                target_price = COALESCE(EXCLUDED.target_price, watchlist_item.target_price),
                stop_price = COALESCE(EXCLUDED.stop_price, watchlist_item.stop_price),
                updated_at = NOW()
            RETURNING *
            "#,
        )
        .bind(watchlist_id)
        .bind(&input.symbol)
        .bind(&input.market)
        .bind(&input.memo)
        .bind(input.target_price)
        .bind(input.stop_price)
        .bind(input.added_price)
        .bind(next_order)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    /// 여러 아이템 일괄 추가
    pub async fn add_items_batch(
        pool: &PgPool,
        watchlist_id: Uuid,
        items: Vec<NewWatchlistItem>,
    ) -> Result<Vec<WatchlistItemRecord>, sqlx::Error> {
        let mut results = Vec::with_capacity(items.len());

        for item in items {
            let record = Self::add_item(pool, watchlist_id, item).await?;
            results.push(record);
        }

        Ok(results)
    }

    /// 아이템 삭제
    pub async fn remove_item(
        pool: &PgPool,
        watchlist_id: Uuid,
        symbol: &str,
        market: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM watchlist_item WHERE watchlist_id = $1 AND symbol = $2 AND market = $3",
        )
        .bind(watchlist_id)
        .bind(symbol)
        .bind(market)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// 아이템 ID로 삭제
    pub async fn remove_item_by_id(pool: &PgPool, item_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM watchlist_item WHERE id = $1")
            .bind(item_id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// 아이템 업데이트 (메모, 목표가, 손절가, 알림)
    pub async fn update_item(
        pool: &PgPool,
        item_id: Uuid,
        input: UpdateWatchlistItem,
    ) -> Result<Option<WatchlistItemRecord>, sqlx::Error> {
        let record = sqlx::query_as::<_, WatchlistItemRecord>(
            r#"
            UPDATE watchlist_item
            SET
                memo = COALESCE($2, memo),
                target_price = COALESCE($3, target_price),
                stop_price = COALESCE($4, stop_price),
                alert_enabled = COALESCE($5, alert_enabled),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(item_id)
        .bind(&input.memo)
        .bind(input.target_price)
        .bind(input.stop_price)
        .bind(input.alert_enabled)
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    /// 특정 종목이 포함된 모든 그룹 조회
    pub async fn find_watchlists_containing_symbol(
        pool: &PgPool,
        symbol: &str,
        market: &str,
    ) -> Result<Vec<WatchlistRecord>, sqlx::Error> {
        let records = sqlx::query_as::<_, WatchlistRecord>(
            r#"
            SELECT w.* FROM watchlist w
            INNER JOIN watchlist_item wi ON w.id = wi.watchlist_id
            WHERE wi.symbol = $1 AND wi.market = $2
            ORDER BY w.sort_order
            "#,
        )
        .bind(symbol)
        .bind(market)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 알림이 활성화된 아이템 조회
    pub async fn get_items_with_alert_enabled(
        pool: &PgPool,
    ) -> Result<Vec<WatchlistItemRecord>, sqlx::Error> {
        let records = sqlx::query_as::<_, WatchlistItemRecord>(
            "SELECT * FROM watchlist_item WHERE alert_enabled = true ORDER BY created_at",
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }
}
