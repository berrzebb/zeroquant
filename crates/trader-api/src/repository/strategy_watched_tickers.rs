//! 전략 관심 종목 관리 Repository.
//!
//! 전략별 관심 종목을 DB에 저장하여 Collector가 우선순위로 데이터를 수집합니다.
//! - 고정 티커: 전략 config에서 추출 (source = 'config')
//! - 동적 티커: 스크리닝/유니버스에서 생성 (source = 'dynamic')

use sqlx::PgPool;

/// 전략 관심 종목 Repository.
pub struct StrategyWatchedTickersRepository;

impl StrategyWatchedTickersRepository {
    /// 전략의 관심 종목 일괄 등록 (UPSERT).
    ///
    /// 기존 데이터와 중복되면 updated_at만 갱신됩니다.
    pub async fn upsert_tickers(
        pool: &PgPool,
        strategy_id: &str,
        tickers: &[String],
        source: &str,
    ) -> Result<usize, sqlx::Error> {
        let mut count = 0;
        for ticker in tickers {
            sqlx::query(
                r#"
                INSERT INTO strategy_watched_tickers (strategy_id, ticker, source)
                VALUES ($1, $2, $3)
                ON CONFLICT (strategy_id, ticker) DO UPDATE
                SET source = $3, updated_at = NOW()
                "#,
            )
            .bind(strategy_id)
            .bind(ticker)
            .bind(source)
            .execute(pool)
            .await?;
            count += 1;
        }
        Ok(count)
    }

    /// 전략의 관심 종목 전체 삭제.
    ///
    /// 전략 정지 또는 삭제 시 호출됩니다.
    pub async fn delete_by_strategy(
        pool: &PgPool,
        strategy_id: &str,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM strategy_watched_tickers WHERE strategy_id = $1",
        )
        .bind(strategy_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// 전략의 동적 티커만 삭제 (고정 티커는 유지).
    ///
    /// 스크리닝 결과가 변경될 때 기존 동적 티커를 정리합니다.
    pub async fn delete_dynamic_by_strategy(
        pool: &PgPool,
        strategy_id: &str,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM strategy_watched_tickers WHERE strategy_id = $1 AND source = 'dynamic'",
        )
        .bind(strategy_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// 특정 전략의 관심 종목 목록 조회.
    pub async fn get_by_strategy(
        pool: &PgPool,
        strategy_id: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT ticker FROM strategy_watched_tickers WHERE strategy_id = $1 ORDER BY ticker",
        )
        .bind(strategy_id)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|(t,)| t).collect())
    }

    /// 모든 전략의 관심 종목 통합 조회 (중복 제거).
    pub async fn get_all_tickers(pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT ticker FROM strategy_watched_tickers ORDER BY ticker",
        )
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|(t,)| t).collect())
    }
}
