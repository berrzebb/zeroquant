//! Score History 저장소.
//!
//! 종목별 Global Score, RouteState, 순위의 일별 히스토리를 관리합니다.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::debug;
use ts_rs::TS;
use utoipa::ToSchema;

use trader_data::RedisCache;

/// 스코어 히스토리 캐시 TTL (7일).
/// 히스토리 데이터는 불변이므로 긴 TTL 적용.
const SCORE_HISTORY_CACHE_TTL_SECS: u64 = 604800;

/// Score History 레코드.
///
/// 참고: 테이블이 TimescaleDB Hypertable로 변환되어 PRIMARY KEY가 (score_date, symbol)로 변경됨
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScoreHistoryRecord {
    pub score_date: NaiveDate,
    pub symbol: String,
    pub global_score: Option<Decimal>,
    pub route_state: Option<String>,
    pub rank: Option<i32>,
    pub component_scores: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Score History 입력.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreHistoryInput {
    pub symbol: String,
    pub score_date: NaiveDate,
    pub global_score: Option<Decimal>,
    pub route_state: Option<String>,
    pub rank: Option<i32>,
    pub component_scores: Option<serde_json::Value>,
}

/// Score History 요약 (API 응답용).
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export, export_to = "repository/")]
pub struct ScoreHistorySummary {
    pub symbol: String,
    pub score_date: NaiveDate,
    pub global_score: Option<f64>,
    pub route_state: Option<String>,
    pub rank: Option<i32>,
    pub score_change: Option<f64>,
    pub rank_change: Option<i32>,
}

/// Score History 저장소.
pub struct ScoreHistoryRepository;

impl ScoreHistoryRepository {
    /// 단일 점수 히스토리 저장 (upsert).
    ///
    /// 참고: Hypertable로 변환되어 PRIMARY KEY가 (score_date, symbol)로 변경됨
    pub async fn upsert(pool: &PgPool, input: &ScoreHistoryInput) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO score_history (score_date, symbol, global_score, route_state, rank, component_scores)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (score_date, symbol) DO UPDATE SET
                global_score = EXCLUDED.global_score,
                route_state = EXCLUDED.route_state,
                rank = EXCLUDED.rank,
                component_scores = EXCLUDED.component_scores
            "#
        )
        .bind(input.score_date)
        .bind(&input.symbol)
        .bind(input.global_score)
        .bind(&input.route_state)
        .bind(input.rank)
        .bind(&input.component_scores)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// 다수 점수 히스토리 일괄 저장 (UNNEST 배치).
    pub async fn upsert_batch(
        pool: &PgPool,
        inputs: &[ScoreHistoryInput],
    ) -> Result<usize, sqlx::Error> {
        if inputs.is_empty() {
            return Ok(0);
        }

        // 500건씩 배치 처리
        const BATCH_SIZE: usize = 500;
        let mut total_count = 0u64;

        for chunk in inputs.chunks(BATCH_SIZE) {
            let dates: Vec<NaiveDate> = chunk.iter().map(|i| i.score_date).collect();
            let symbols: Vec<&str> = chunk.iter().map(|i| i.symbol.as_str()).collect();
            let scores: Vec<Option<Decimal>> = chunk.iter().map(|i| i.global_score).collect();
            let states: Vec<Option<&str>> =
                chunk.iter().map(|i| i.route_state.as_deref()).collect();
            let ranks: Vec<Option<i32>> = chunk.iter().map(|i| i.rank).collect();
            let components: Vec<Option<serde_json::Value>> =
                chunk.iter().map(|i| i.component_scores.clone()).collect();

            let result = sqlx::query(
                r#"
                INSERT INTO score_history (score_date, symbol, global_score, route_state, rank, component_scores)
                SELECT * FROM UNNEST(
                    $1::date[],
                    $2::text[],
                    $3::decimal[],
                    $4::text[],
                    $5::integer[],
                    $6::jsonb[]
                )
                ON CONFLICT (score_date, symbol) DO UPDATE SET
                    global_score = EXCLUDED.global_score,
                    route_state = EXCLUDED.route_state,
                    rank = EXCLUDED.rank,
                    component_scores = EXCLUDED.component_scores
                "#,
            )
            .bind(&dates)
            .bind(&symbols)
            .bind(&scores)
            .bind(&states)
            .bind(&ranks)
            .bind(&components)
            .execute(pool)
            .await?;

            total_count += result.rows_affected();
        }

        debug!(count = total_count, "Score history batch upsert completed");
        Ok(total_count as usize)
    }

    /// 종목별 점수 히스토리 조회.
    pub async fn get_by_symbol(
        pool: &PgPool,
        symbol: &str,
        days: i32,
    ) -> Result<Vec<ScoreHistoryRecord>, sqlx::Error> {
        let records: Vec<ScoreHistoryRecord> = sqlx::query_as(
            r#"
            SELECT score_date, symbol, global_score, route_state, rank, component_scores, created_at
            FROM score_history
            WHERE symbol = $1
            ORDER BY score_date DESC
            LIMIT $2
            "#,
        )
        .bind(symbol)
        .bind(days)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 특정 날짜의 전체 순위 조회.
    pub async fn get_by_date(
        pool: &PgPool,
        date: NaiveDate,
        limit: i32,
    ) -> Result<Vec<ScoreHistoryRecord>, sqlx::Error> {
        let records: Vec<ScoreHistoryRecord> = sqlx::query_as(
            r#"
            SELECT score_date, symbol, global_score, route_state, rank, component_scores, created_at
            FROM score_history
            WHERE score_date = $1
            ORDER BY global_score DESC NULLS LAST
            LIMIT $2
            "#,
        )
        .bind(date)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// 종목별 점수 변화 요약 조회 (전일 대비).
    pub async fn get_with_change(
        pool: &PgPool,
        symbol: &str,
        days: i32,
    ) -> Result<Vec<ScoreHistorySummary>, sqlx::Error> {
        use rust_decimal::prelude::ToPrimitive;

        let records = Self::get_by_symbol(pool, symbol, days).await?;

        let mut summaries: Vec<ScoreHistorySummary> = Vec::with_capacity(records.len());

        for (i, record) in records.iter().enumerate() {
            let prev = records.get(i + 1);

            let score_change = match (record.global_score, prev.and_then(|p| p.global_score)) {
                (Some(curr), Some(prev_score)) => Some((curr - prev_score).to_f64().unwrap_or(0.0)),
                _ => None,
            };

            let rank_change = match (record.rank, prev.and_then(|p| p.rank)) {
                (Some(curr), Some(prev_rank)) => Some(prev_rank - curr), // 순위 상승은 양수
                _ => None,
            };

            summaries.push(ScoreHistorySummary {
                symbol: record.symbol.clone(),
                score_date: record.score_date,
                global_score: record.global_score.and_then(|d| d.to_f64()),
                route_state: record.route_state.clone(),
                rank: record.rank,
                score_change,
                rank_change,
            });
        }

        Ok(summaries)
    }

    /// 가장 최근 저장 날짜 조회.
    pub async fn get_latest_date(pool: &PgPool) -> Result<Option<NaiveDate>, sqlx::Error> {
        let result: Option<(NaiveDate,)> = sqlx::query_as(
            r#"
            SELECT MAX(score_date) as date
            FROM score_history
            "#,
        )
        .fetch_optional(pool)
        .await?;

        Ok(result.map(|r| r.0))
    }

    /// 특정 기간 데이터 삭제 (오래된 데이터 정리용).
    pub async fn delete_before(pool: &PgPool, before_date: NaiveDate) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM score_history
            WHERE score_date < $1
            "#,
        )
        .bind(before_date)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ==================== 캐시 래퍼 함수 ====================

    /// 종목별 점수 히스토리 조회 (Redis 캐시 적용).
    ///
    /// TTL: 7일 (히스토리 데이터는 불변)
    pub async fn get_by_symbol_cached(
        pool: &PgPool,
        cache: Option<&RedisCache>,
        symbol: &str,
        days: i32,
    ) -> Result<Vec<ScoreHistoryRecord>, sqlx::Error> {
        // 캐시 없으면 DB 직접 조회
        let Some(cache) = cache else {
            return Self::get_by_symbol(pool, symbol, days).await;
        };

        let cache_key = format!("score_history:{}:{}", symbol.to_uppercase(), days);

        // 캐시에서 조회 시도
        if let Ok(Some(cached)) = cache.get::<Vec<ScoreHistoryRecord>>(&cache_key).await {
            debug!(symbol = symbol, days = days, "스코어 히스토리 캐시 히트");
            return Ok(cached);
        }

        // DB 조회
        let results = Self::get_by_symbol(pool, symbol, days).await?;

        // 결과 캐시에 저장
        if !results.is_empty() {
            if let Err(e) = cache
                .set_with_ttl(&cache_key, &results, SCORE_HISTORY_CACHE_TTL_SECS)
                .await
            {
                debug!(error = %e, "스코어 히스토리 캐시 저장 실패");
            }
        }

        Ok(results)
    }
}
