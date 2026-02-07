//! Reality Check 추천 검증 Repository
//!
//! 전일 추천 종목의 익일 실제 성과를 자동으로 검증합니다.
//!
//! # 워크플로우
//! 1. 매일 장 마감 후: `save_snapshot()` - 추천 종목 가격 스냅샷 저장
//! 2. 익일 장 마감 후: `calculate_reality_check()` - 실제 성과 계산
//! 3. 통계 조회: `get_daily_stats()`, `get_source_stats()` 등

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tracing::{debug, error, info};
use utoipa::ToSchema;

/// 가격 스냅샷 레코드
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct PriceSnapshot {
    pub snapshot_date: NaiveDate,
    pub symbol: String,
    pub close_price: Decimal,
    pub volume: Option<i64>,
    pub recommend_source: Option<String>,
    pub recommend_rank: Option<i32>,
    pub recommend_score: Option<Decimal>,
    pub expected_return: Option<Decimal>,
    pub expected_holding_days: Option<i32>,
    pub market: Option<String>,
    pub sector: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// 스냅샷 저장 입력
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SnapshotInput {
    pub symbol: String,
    pub close_price: Decimal,
    pub volume: Option<i64>,
    pub recommend_source: String,
    pub recommend_rank: Option<i32>,
    pub recommend_score: Option<Decimal>,
    pub expected_return: Option<Decimal>,
    pub expected_holding_days: Option<i32>,
    pub market: Option<String>,
    pub sector: Option<String>,
}

/// Reality Check 결과 레코드
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct RealityCheckRecord {
    pub check_date: NaiveDate,
    pub recommend_date: NaiveDate,
    pub symbol: String,
    pub recommend_source: Option<String>,
    pub recommend_rank: Option<i32>,
    pub recommend_score: Option<Decimal>,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub actual_return: Decimal,
    pub is_profitable: bool,
    pub entry_volume: Option<i64>,
    pub exit_volume: Option<i64>,
    pub volume_change: Option<Decimal>,
    pub expected_return: Option<Decimal>,
    pub return_error: Option<Decimal>,
    pub max_profit: Option<Decimal>,
    pub max_drawdown: Option<Decimal>,
    pub volatility: Option<Decimal>,
    pub market: Option<String>,
    pub sector: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// 일별 통계
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct DailyStats {
    pub check_date: Option<NaiveDate>,
    pub total_count: Option<i64>,
    pub win_count: Option<i64>,
    pub win_rate: Option<Decimal>,
    pub avg_return: Option<Decimal>,
    pub avg_win_return: Option<Decimal>,
    pub avg_loss_return: Option<Decimal>,
    pub max_return: Option<Decimal>,
    pub min_return: Option<Decimal>,
    pub return_stddev: Option<Decimal>,
}

/// 소스별 통계
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct SourceStats {
    pub recommend_source: Option<String>,
    pub total_count: Option<i64>,
    pub win_count: Option<i64>,
    pub win_rate: Option<Decimal>,
    pub avg_return: Option<Decimal>,
    pub avg_win_return: Option<Decimal>,
    pub avg_loss_return: Option<Decimal>,
}

/// 랭크별 통계
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct RankStats {
    pub recommend_rank: Option<i32>,
    pub total_count: Option<i64>,
    pub win_rate: Option<Decimal>,
    pub avg_return: Option<Decimal>,
}

/// Reality Check 계산 결과
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct CalculationResult {
    pub symbol: Option<String>,
    pub actual_return: Option<Decimal>,
    pub is_profitable: Option<bool>,
    pub processed_count: Option<i32>,
}

/// Reality Check Repository
pub struct RealityCheckRepository;

impl RealityCheckRepository {
    // ==================== 스냅샷 관리 ====================

    /// 가격 스냅샷 저장 (단일)
    ///
    /// 매일 장 마감 후 추천 종목의 가격을 스냅샷으로 저장합니다.
    pub async fn save_snapshot(
        pool: &PgPool,
        snapshot_date: NaiveDate,
        input: &SnapshotInput,
    ) -> Result<(), sqlx::Error> {
        debug!(
            "Saving price snapshot for {} on {}",
            input.symbol, snapshot_date
        );

        sqlx::query!(
            r#"
            INSERT INTO price_snapshot (
                snapshot_date, symbol, close_price, volume,
                recommend_source, recommend_rank, recommend_score,
                expected_return, expected_holding_days,
                market, sector
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (snapshot_date, symbol) DO UPDATE SET
                close_price = EXCLUDED.close_price,
                volume = EXCLUDED.volume,
                recommend_source = EXCLUDED.recommend_source,
                recommend_rank = EXCLUDED.recommend_rank,
                recommend_score = EXCLUDED.recommend_score,
                expected_return = EXCLUDED.expected_return,
                expected_holding_days = EXCLUDED.expected_holding_days,
                market = EXCLUDED.market,
                sector = EXCLUDED.sector
            "#,
            snapshot_date,
            input.symbol,
            input.close_price,
            input.volume,
            input.recommend_source,
            input.recommend_rank,
            input.recommend_score,
            input.expected_return,
            input.expected_holding_days,
            input.market,
            input.sector,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// 가격 스냅샷 일괄 저장
    ///
    /// 여러 종목의 스냅샷을 한 번에 저장합니다.
    pub async fn save_snapshots_batch(
        pool: &PgPool,
        snapshot_date: NaiveDate,
        snapshots: &[SnapshotInput],
    ) -> Result<usize, sqlx::Error> {
        debug!(
            "Saving {} price snapshots for {}",
            snapshots.len(),
            snapshot_date
        );

        let mut tx = pool.begin().await?;
        let mut saved_count = 0;

        for snapshot in snapshots {
            let result = sqlx::query!(
                r#"
                INSERT INTO price_snapshot (
                    snapshot_date, symbol, close_price, volume,
                    recommend_source, recommend_rank, recommend_score,
                    expected_return, expected_holding_days,
                    market, sector
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT (snapshot_date, symbol) DO UPDATE SET
                    close_price = EXCLUDED.close_price,
                    volume = EXCLUDED.volume,
                    recommend_source = EXCLUDED.recommend_source,
                    recommend_rank = EXCLUDED.recommend_rank,
                    recommend_score = EXCLUDED.recommend_score,
                    expected_return = EXCLUDED.expected_return,
                    expected_holding_days = EXCLUDED.expected_holding_days,
                    market = EXCLUDED.market,
                    sector = EXCLUDED.sector
                "#,
                snapshot_date,
                snapshot.symbol,
                snapshot.close_price,
                snapshot.volume,
                snapshot.recommend_source,
                snapshot.recommend_rank,
                snapshot.recommend_score,
                snapshot.expected_return,
                snapshot.expected_holding_days,
                snapshot.market,
                snapshot.sector,
            )
            .execute(&mut *tx)
            .await;

            match result {
                Ok(_) => saved_count += 1,
                Err(e) => {
                    error!("Failed to save snapshot for {}: {}", snapshot.symbol, e);
                }
            }
        }

        tx.commit().await?;

        info!(
            "Successfully saved {} snapshots for {}",
            saved_count, snapshot_date
        );

        Ok(saved_count)
    }

    /// 특정 날짜의 스냅샷 조회
    pub async fn get_snapshots(
        pool: &PgPool,
        snapshot_date: NaiveDate,
    ) -> Result<Vec<PriceSnapshot>, sqlx::Error> {
        debug!("Fetching snapshots for {}", snapshot_date);

        let snapshots = sqlx::query_as!(
            PriceSnapshot,
            r#"
            SELECT
                snapshot_date,
                symbol,
                close_price,
                volume,
                recommend_source,
                recommend_rank,
                recommend_score,
                expected_return,
                expected_holding_days,
                market,
                sector,
                created_at
            FROM price_snapshot
            WHERE snapshot_date = $1
            ORDER BY recommend_rank NULLS LAST, recommend_score DESC NULLS LAST
            "#,
            snapshot_date,
        )
        .fetch_all(pool)
        .await?;

        Ok(snapshots)
    }

    // ==================== Reality Check 계산 ====================

    /// Reality Check 계산 실행
    ///
    /// 전일(recommend_date) 추천 종목의 금일(check_date) 성과를 계산합니다.
    /// DB 함수를 호출하여 자동으로 계산 및 저장됩니다.
    pub async fn calculate_reality_check(
        pool: &PgPool,
        recommend_date: NaiveDate,
        check_date: NaiveDate,
    ) -> Result<Vec<CalculationResult>, sqlx::Error> {
        info!(
            "Calculating reality check for recommendations on {} checked on {}",
            recommend_date, check_date
        );

        let results = sqlx::query_as!(
            CalculationResult,
            r#"
            SELECT
                symbol,
                actual_return,
                is_profitable,
                processed_count
            FROM calculate_reality_check($1, $2)
            "#,
            recommend_date,
            check_date,
        )
        .fetch_all(pool)
        .await?;

        info!(
            "Reality check calculation completed: {} records",
            results.len()
        );

        Ok(results)
    }

    /// Reality Check 결과 조회 (기간)
    pub async fn get_reality_checks(
        pool: &PgPool,
        start_date: NaiveDate,
        end_date: NaiveDate,
        recommend_source: Option<&str>,
    ) -> Result<Vec<RealityCheckRecord>, sqlx::Error> {
        debug!(
            "Fetching reality check results from {} to {}",
            start_date, end_date
        );

        let results = if let Some(source) = recommend_source {
            sqlx::query_as!(
                RealityCheckRecord,
                r#"
                SELECT
                    check_date,
                    recommend_date,
                    symbol,
                    recommend_source,
                    recommend_rank,
                    recommend_score,
                    entry_price,
                    exit_price,
                    actual_return,
                    is_profitable,
                    entry_volume,
                    exit_volume,
                    volume_change,
                    expected_return,
                    return_error,
                    max_profit,
                    max_drawdown,
                    volatility,
                    market,
                    sector,
                    created_at
                FROM reality_check
                WHERE check_date >= $1
                    AND check_date <= $2
                    AND recommend_source = $3
                ORDER BY check_date DESC, actual_return DESC
                "#,
                start_date,
                end_date,
                source,
            )
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as!(
                RealityCheckRecord,
                r#"
                SELECT
                    check_date,
                    recommend_date,
                    symbol,
                    recommend_source,
                    recommend_rank,
                    recommend_score,
                    entry_price,
                    exit_price,
                    actual_return,
                    is_profitable,
                    entry_volume,
                    exit_volume,
                    volume_change,
                    expected_return,
                    return_error,
                    max_profit,
                    max_drawdown,
                    volatility,
                    market,
                    sector,
                    created_at
                FROM reality_check
                WHERE check_date >= $1 AND check_date <= $2
                ORDER BY check_date DESC, actual_return DESC
                "#,
                start_date,
                end_date,
            )
            .fetch_all(pool)
            .await?
        };

        Ok(results)
    }

    // ==================== 통계 조회 ====================

    /// 일별 통계 조회
    pub async fn get_daily_stats(
        pool: &PgPool,
        limit: i32,
    ) -> Result<Vec<DailyStats>, sqlx::Error> {
        debug!("Fetching daily stats (limit: {})", limit);

        let stats = sqlx::query_as!(
            DailyStats,
            r#"
            SELECT
                check_date,
                total_count,
                win_count,
                win_rate,
                avg_return,
                avg_win_return,
                avg_loss_return,
                max_return,
                min_return,
                return_stddev
            FROM v_reality_check_daily_stats
            ORDER BY check_date DESC
            LIMIT $1
            "#,
            limit as i64,
        )
        .fetch_all(pool)
        .await?;

        Ok(stats)
    }

    /// 소스별 통계 조회
    pub async fn get_source_stats(pool: &PgPool) -> Result<Vec<SourceStats>, sqlx::Error> {
        debug!("Fetching source stats");

        let stats = sqlx::query_as!(
            SourceStats,
            r#"
            SELECT
                recommend_source,
                total_count,
                win_count,
                win_rate,
                avg_return,
                avg_win_return,
                avg_loss_return
            FROM v_reality_check_source_stats
            ORDER BY avg_return DESC
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(stats)
    }

    /// 랭크별 통계 조회
    pub async fn get_rank_stats(pool: &PgPool) -> Result<Vec<RankStats>, sqlx::Error> {
        debug!("Fetching rank stats");

        let stats = sqlx::query_as!(
            RankStats,
            r#"
            SELECT
                recommend_rank,
                total_count,
                win_rate,
                avg_return
            FROM v_reality_check_rank_stats
            ORDER BY recommend_rank
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(stats)
    }

    /// 특정 소스의 최근 성과 조회
    pub async fn get_recent_performance(
        pool: &PgPool,
        recommend_source: &str,
        days: i32,
    ) -> Result<Vec<RealityCheckRecord>, sqlx::Error> {
        debug!(
            "Fetching recent {} performance for last {} days",
            recommend_source, days
        );

        let results = sqlx::query_as!(
            RealityCheckRecord,
            r#"
            SELECT
                check_date,
                recommend_date,
                symbol,
                recommend_source,
                recommend_rank,
                recommend_score,
                entry_price,
                exit_price,
                actual_return,
                is_profitable,
                entry_volume,
                exit_volume,
                volume_change,
                expected_return,
                return_error,
                max_profit,
                max_drawdown,
                volatility,
                market,
                sector,
                created_at
            FROM reality_check
            WHERE recommend_source = $1
                AND check_date >= CURRENT_DATE - ($2 || ' days')::interval
            ORDER BY check_date DESC, actual_return DESC
            "#,
            recommend_source,
            days.to_string(),
        )
        .fetch_all(pool)
        .await?;

        Ok(results)
    }

    /// 전체 통계 요약
    pub async fn get_summary_stats(pool: &PgPool, days: i32) -> Result<DailyStats, sqlx::Error> {
        debug!("Fetching summary stats for last {} days", days);

        let stats = sqlx::query_as!(
            DailyStats,
            r#"
            SELECT
                CURRENT_DATE as "check_date!",
                COUNT(*) as "total_count!",
                COUNT(*) FILTER (WHERE is_profitable) as "win_count!",
                ROUND(COUNT(*) FILTER (WHERE is_profitable)::NUMERIC / COUNT(*) * 100, 2) as "win_rate!",
                ROUND(AVG(actual_return), 4) as "avg_return!",
                ROUND(AVG(actual_return) FILTER (WHERE is_profitable), 4) as avg_win_return,
                ROUND(AVG(actual_return) FILTER (WHERE NOT is_profitable), 4) as avg_loss_return,
                ROUND(MAX(actual_return), 4) as "max_return!",
                ROUND(MIN(actual_return), 4) as "min_return!",
                ROUND(STDDEV(actual_return), 4) as return_stddev
            FROM reality_check
            WHERE check_date >= CURRENT_DATE - ($1 || ' days')::interval
            "#,
            days.to_string(),
        )
        .fetch_one(pool)
        .await?;

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;

    #[tokio::test]
    #[ignore] // DB 연결 필요
    async fn test_save_snapshot() {
        let pool = PgPool::connect("postgres://trader:trader_secret@localhost/trader")
            .await
            .unwrap();

        let snapshot = SnapshotInput {
            symbol: "005930".to_string(),
            close_price: dec!(70000),
            volume: Some(10000000),
            recommend_source: "screening_momentum".to_string(),
            recommend_rank: Some(1),
            recommend_score: Some(dec!(95.5)),
            expected_return: Some(dec!(5.0)),
            expected_holding_days: Some(3),
            market: Some("KR".to_string()),
            sector: Some("IT".to_string()),
        };

        let today = Utc::now().naive_utc().date();
        let result = RealityCheckRepository::save_snapshot(&pool, today, &snapshot).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore] // DB 연결 필요
    async fn test_calculate_reality_check() {
        let pool = PgPool::connect("postgres://trader:trader_secret@localhost/trader")
            .await
            .unwrap();

        let yesterday = Utc::now().naive_utc().date() - chrono::Duration::days(1);
        let today = Utc::now().naive_utc().date();

        let results =
            RealityCheckRepository::calculate_reality_check(&pool, yesterday, today).await;

        assert!(results.is_ok());
    }
}
