//! OHLCV (Klines) 데이터 Repository
//!
//! Yahoo Finance에서 가져온 캔들 데이터의 CRUD 작업을 처리합니다.
//! TimescaleDB Hypertable을 사용하여 시계열 데이터를 효율적으로 저장합니다.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::debug;

/// OHLCV 캔들 데이터 레코드
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct KlineRecord {
    /// 심볼 (예: "AAPL", "005930.KS", "SPY")
    pub symbol: String,
    /// 타임프레임 (예: "1m", "5m", "15m", "1h", "1d")
    pub timeframe: String,
    /// 캔들 시작 시간 (UTC)
    pub open_time: DateTime<Utc>,
    /// 시가
    pub open: Decimal,
    /// 고가
    pub high: Decimal,
    /// 저가
    pub low: Decimal,
    /// 종가
    pub close: Decimal,
    /// 거래량
    pub volume: Decimal,
    /// 캔들 종료 시간
    pub close_time: Option<DateTime<Utc>>,
    /// 데이터 수집 시간
    pub fetched_at: Option<DateTime<Utc>>,
}

/// 새 캔들 데이터 입력
#[derive(Debug, Clone)]
pub struct NewKline {
    pub symbol: String,
    pub timeframe: String,
    pub open_time: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub close_time: Option<DateTime<Utc>>,
}

/// 캐시 메타데이터
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CacheMetadata {
    pub symbol: String,
    pub timeframe: String,
    pub first_cached_time: Option<DateTime<Utc>>,
    pub last_cached_time: Option<DateTime<Utc>>,
    pub last_updated_at: Option<DateTime<Utc>>,
    pub total_candles: Option<i32>,
}

/// Klines Repository
///
/// OHLCV 데이터의 저장, 조회, 배치 처리를 담당합니다.
pub struct KlinesRepository;

impl KlinesRepository {
    /// OHLCV 데이터 배치 저장 (UNNEST 최적화)
    ///
    /// 중복 키 발생 시 기존 데이터를 업데이트합니다 (ON CONFLICT DO UPDATE).
    ///
    /// # Arguments
    /// * `pool` - 데이터베이스 연결 풀
    /// * `klines` - 저장할 캔들 데이터 목록
    ///
    /// # Returns
    /// 저장된 레코드 수
    pub async fn save_batch(pool: &PgPool, klines: &[NewKline]) -> Result<usize, sqlx::Error> {
        if klines.is_empty() {
            return Ok(0);
        }

        // UNNEST 배열 준비
        let symbols: Vec<&str> = klines.iter().map(|k| k.symbol.as_str()).collect();
        let timeframes: Vec<&str> = klines.iter().map(|k| k.timeframe.as_str()).collect();
        let open_times: Vec<DateTime<Utc>> = klines.iter().map(|k| k.open_time).collect();
        let opens: Vec<Decimal> = klines.iter().map(|k| k.open).collect();
        let highs: Vec<Decimal> = klines.iter().map(|k| k.high).collect();
        let lows: Vec<Decimal> = klines.iter().map(|k| k.low).collect();
        let closes: Vec<Decimal> = klines.iter().map(|k| k.close).collect();
        let volumes: Vec<Decimal> = klines.iter().map(|k| k.volume).collect();
        let close_times: Vec<Option<DateTime<Utc>>> = klines.iter().map(|k| k.close_time).collect();

        let result = sqlx::query(
            r#"
            INSERT INTO ohlcv (symbol, timeframe, open_time, open, high, low, close, volume, close_time)
            SELECT * FROM UNNEST(
                $1::text[],
                $2::text[],
                $3::timestamptz[],
                $4::decimal[],
                $5::decimal[],
                $6::decimal[],
                $7::decimal[],
                $8::decimal[],
                $9::timestamptz[]
            )
            ON CONFLICT (symbol, timeframe, open_time) DO UPDATE SET
                open = EXCLUDED.open,
                high = EXCLUDED.high,
                low = EXCLUDED.low,
                close = EXCLUDED.close,
                volume = EXCLUDED.volume,
                close_time = EXCLUDED.close_time,
                fetched_at = NOW()
            "#,
        )
        .bind(&symbols)
        .bind(&timeframes)
        .bind(&open_times)
        .bind(&opens)
        .bind(&highs)
        .bind(&lows)
        .bind(&closes)
        .bind(&volumes)
        .bind(&close_times)
        .execute(pool)
        .await?;

        debug!(
            "Saved {} klines (affected: {})",
            klines.len(),
            result.rows_affected()
        );

        Ok(result.rows_affected() as usize)
    }

    /// 특정 심볼/타임프레임의 기간별 데이터 조회
    ///
    /// # Arguments
    /// * `pool` - 데이터베이스 연결 풀
    /// * `symbol` - 심볼
    /// * `timeframe` - 타임프레임
    /// * `start` - 시작 시간 (포함)
    /// * `end` - 종료 시간 (포함)
    pub async fn get_range(
        pool: &PgPool,
        symbol: &str,
        timeframe: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<KlineRecord>, sqlx::Error> {
        let records = sqlx::query_as::<_, KlineRecord>(
            r#"
            SELECT symbol, timeframe, open_time, open, high, low, close, volume, close_time, fetched_at
            FROM ohlcv
            WHERE symbol = $1
              AND timeframe = $2
              AND open_time >= $3
              AND open_time <= $4
            ORDER BY open_time ASC
            "#,
        )
        .bind(symbol)
        .bind(timeframe)
        .bind(start)
        .bind(end)
        .fetch_all(pool)
        .await?;

        debug!(
            "Fetched {} klines for {} {} from {} to {}",
            records.len(),
            symbol,
            timeframe,
            start,
            end
        );

        Ok(records)
    }

    /// 특정 심볼/타임프레임의 최신 N개 데이터 조회
    ///
    /// # Arguments
    /// * `pool` - 데이터베이스 연결 풀
    /// * `symbol` - 심볼
    /// * `timeframe` - 타임프레임
    /// * `count` - 가져올 캔들 수
    pub async fn get_latest(
        pool: &PgPool,
        symbol: &str,
        timeframe: &str,
        count: i32,
    ) -> Result<Vec<KlineRecord>, sqlx::Error> {
        // 서브쿼리: DESC로 최신 N개 선택 → 외부에서 ASC 정렬 (Rust reverse() 불필요)
        let records = sqlx::query_as::<_, KlineRecord>(
            r#"
            SELECT symbol, timeframe, open_time, open, high, low, close, volume, close_time, fetched_at
            FROM (
                SELECT symbol, timeframe, open_time, open, high, low, close, volume, close_time, fetched_at
                FROM ohlcv
                WHERE symbol = $1
                  AND timeframe = $2
                ORDER BY open_time DESC
                LIMIT $3
            ) AS latest
            ORDER BY open_time ASC
            "#,
        )
        .bind(symbol)
        .bind(timeframe)
        .bind(count)
        .fetch_all(pool)
        .await?;

        debug!(
            "Fetched {} latest klines for {} {}",
            records.len(),
            symbol,
            timeframe
        );

        Ok(records)
    }

    /// 저장된 심볼 목록 조회
    ///
    /// # Arguments
    /// * `pool` - 데이터베이스 연결 풀
    pub async fn list_symbols(pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
        let symbols: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT DISTINCT symbol
            FROM ohlcv
            ORDER BY symbol ASC
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(symbols.into_iter().map(|(s,)| s).collect())
    }

    /// 심볼별 타임프레임 목록 조회
    ///
    /// # Arguments
    /// * `pool` - 데이터베이스 연결 풀
    /// * `symbol` - 심볼
    pub async fn list_timeframes(pool: &PgPool, symbol: &str) -> Result<Vec<String>, sqlx::Error> {
        let timeframes: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT DISTINCT timeframe
            FROM ohlcv
            WHERE symbol = $1
            ORDER BY timeframe ASC
            "#,
        )
        .bind(symbol)
        .fetch_all(pool)
        .await?;

        Ok(timeframes.into_iter().map(|(tf,)| tf).collect())
    }

    /// 캐시 메타데이터 조회
    ///
    /// # Arguments
    /// * `pool` - 데이터베이스 연결 풀
    /// * `symbol` - 심볼
    /// * `timeframe` - 타임프레임
    pub async fn get_metadata(
        pool: &PgPool,
        symbol: &str,
        timeframe: &str,
    ) -> Result<Option<CacheMetadata>, sqlx::Error> {
        let metadata = sqlx::query_as::<_, CacheMetadata>(
            r#"
            SELECT symbol, timeframe, first_cached_time, last_cached_time, last_updated_at, total_candles
            FROM ohlcv_metadata
            WHERE symbol = $1 AND timeframe = $2
            "#,
        )
        .bind(symbol)
        .bind(timeframe)
        .fetch_optional(pool)
        .await?;

        Ok(metadata)
    }

    /// 모든 캐시 메타데이터 조회
    ///
    /// # Arguments
    /// * `pool` - 데이터베이스 연결 풀
    pub async fn list_metadata(pool: &PgPool) -> Result<Vec<CacheMetadata>, sqlx::Error> {
        let metadata = sqlx::query_as::<_, CacheMetadata>(
            r#"
            SELECT symbol, timeframe, first_cached_time, last_cached_time, last_updated_at, total_candles
            FROM ohlcv_metadata
            ORDER BY last_updated_at DESC
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(metadata)
    }

    /// 특정 심볼/타임프레임의 캐시 데이터 삭제
    ///
    /// # Arguments
    /// * `pool` - 데이터베이스 연결 풀
    /// * `symbol` - 심볼
    /// * `timeframe` - 타임프레임
    pub async fn delete(pool: &PgPool, symbol: &str, timeframe: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM ohlcv
            WHERE symbol = $1 AND timeframe = $2
            "#,
        )
        .bind(symbol)
        .bind(timeframe)
        .execute(pool)
        .await?;

        // 메타데이터도 삭제
        let _ = sqlx::query(
            r#"
            DELETE FROM ohlcv_metadata
            WHERE symbol = $1 AND timeframe = $2
            "#,
        )
        .bind(symbol)
        .bind(timeframe)
        .execute(pool)
        .await;

        debug!(
            "Deleted {} klines for {} {}",
            result.rows_affected(),
            symbol,
            timeframe
        );

        Ok(result.rows_affected())
    }

    /// 마지막 캐시 시간 조회
    ///
    /// 증분 업데이트 시 시작점을 결정하기 위해 사용합니다.
    ///
    /// # Arguments
    /// * `pool` - 데이터베이스 연결 풀
    /// * `symbol` - 심볼
    /// * `timeframe` - 타임프레임
    pub async fn get_last_cached_time(
        pool: &PgPool,
        symbol: &str,
        timeframe: &str,
    ) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
        let result: Option<(DateTime<Utc>,)> = sqlx::query_as(
            r#"
            SELECT MAX(open_time)
            FROM ohlcv
            WHERE symbol = $1 AND timeframe = $2
            "#,
        )
        .bind(symbol)
        .bind(timeframe)
        .fetch_optional(pool)
        .await?;

        Ok(result.map(|(t,)| t))
    }

    /// 캔들 수 조회
    ///
    /// # Arguments
    /// * `pool` - 데이터베이스 연결 풀
    /// * `symbol` - 심볼
    /// * `timeframe` - 타임프레임
    pub async fn count(pool: &PgPool, symbol: &str, timeframe: &str) -> Result<i64, sqlx::Error> {
        let result: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM ohlcv
            WHERE symbol = $1 AND timeframe = $2
            "#,
        )
        .bind(symbol)
        .bind(timeframe)
        .fetch_one(pool)
        .await?;

        Ok(result.0)
    }

    /// 다중 심볼 기간별 데이터 배치 조회
    ///
    /// # Arguments
    /// * `pool` - 데이터베이스 연결 풀
    /// * `symbols` - 심볼 목록
    /// * `timeframe` - 타임프레임
    /// * `start` - 시작 시간 (포함)
    /// * `end` - 종료 시간 (포함)
    pub async fn get_range_batch(
        pool: &PgPool,
        symbols: &[String],
        timeframe: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<KlineRecord>, sqlx::Error> {
        if symbols.is_empty() {
            return Ok(vec![]);
        }

        let records = sqlx::query_as::<_, KlineRecord>(
            r#"
            SELECT symbol, timeframe, open_time, open, high, low, close, volume, close_time, fetched_at
            FROM ohlcv
            WHERE symbol = ANY($1::text[])
              AND timeframe = $2
              AND open_time >= $3
              AND open_time <= $4
            ORDER BY symbol, open_time ASC
            "#,
        )
        .bind(symbols)
        .bind(timeframe)
        .bind(start)
        .bind(end)
        .fetch_all(pool)
        .await?;

        debug!(
            "Fetched {} klines for {} symbols in {} from {} to {}",
            records.len(),
            symbols.len(),
            timeframe,
            start,
            end
        );

        Ok(records)
    }

    /// 다중 타임프레임 기간별 데이터 조회 (UNION ALL 최적화)
    ///
    /// 여러 타임프레임의 데이터를 단일 쿼리로 조회하여 성능을 최적화합니다.
    /// 기존 N+1 쿼리 문제를 해결합니다.
    ///
    /// # Arguments
    /// * `pool` - 데이터베이스 연결 풀
    /// * `symbol` - 심볼
    /// * `timeframes` - 타임프레임 목록
    /// * `start` - 시작 시간 (포함)
    /// * `end` - 종료 시간 (포함)
    ///
    /// # Returns
    /// 타임프레임별로 그룹화된 캔들 데이터
    ///
    /// # Example
    /// ```rust,ignore
    /// let result = KlinesRepository::get_range_multi_timeframe(
    ///     &pool,
    ///     "BTC/USDT",
    ///     &["5m".to_string(), "1h".to_string(), "1d".to_string()],
    ///     start_time,
    ///     end_time,
    /// ).await?;
    /// // result: HashMap<String, Vec<KlineRecord>>
    /// ```
    pub async fn get_range_multi_timeframe(
        pool: &PgPool,
        symbol: &str,
        timeframes: &[String],
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<std::collections::HashMap<String, Vec<KlineRecord>>, sqlx::Error> {
        use std::collections::HashMap;

        if timeframes.is_empty() {
            return Ok(HashMap::new());
        }

        // ANY 연산자로 다중 타임프레임 단일 쿼리 조회
        let records = sqlx::query_as::<_, KlineRecord>(
            r#"
            SELECT symbol, timeframe, open_time, open, high, low, close, volume, close_time, fetched_at
            FROM ohlcv
            WHERE symbol = $1
              AND timeframe = ANY($2::text[])
              AND open_time >= $3
              AND open_time <= $4
            ORDER BY timeframe, open_time ASC
            "#,
        )
        .bind(symbol)
        .bind(timeframes)
        .bind(start)
        .bind(end)
        .fetch_all(pool)
        .await?;

        // 타임프레임별로 그룹화
        let mut grouped: HashMap<String, Vec<KlineRecord>> = HashMap::new();
        for record in records {
            grouped
                .entry(record.timeframe.clone())
                .or_default()
                .push(record);
        }

        debug!(
            "Fetched {} klines for {} across {} timeframes from {} to {}",
            grouped.values().map(|v| v.len()).sum::<usize>(),
            symbol,
            timeframes.len(),
            start,
            end
        );

        Ok(grouped)
    }

    /// 다중 타임프레임 최신 데이터 조회 (limit 기반)
    ///
    /// 각 타임프레임별로 최신 N개의 캔들을 단일 쿼리로 조회합니다.
    ///
    /// # Arguments
    /// * `pool` - 데이터베이스 연결 풀
    /// * `symbol` - 심볼
    /// * `timeframes` - 타임프레임 목록
    /// * `limit` - 각 타임프레임별 최대 캔들 수
    pub async fn get_latest_multi_timeframe(
        pool: &PgPool,
        symbol: &str,
        timeframes: &[String],
        limit: i64,
    ) -> Result<std::collections::HashMap<String, Vec<KlineRecord>>, sqlx::Error> {
        use std::collections::HashMap;

        if timeframes.is_empty() {
            return Ok(HashMap::new());
        }

        // LATERAL JOIN을 사용한 각 타임프레임별 limit 적용
        // PostgreSQL/TimescaleDB에서 효율적으로 동작
        let records = sqlx::query_as::<_, KlineRecord>(
            r#"
            SELECT k.symbol, k.timeframe, k.open_time, k.open, k.high, k.low, k.close, k.volume, k.close_time, k.fetched_at
            FROM unnest($2::text[]) AS tf(timeframe)
            CROSS JOIN LATERAL (
                SELECT symbol, timeframe, open_time, open, high, low, close, volume, close_time, fetched_at
                FROM ohlcv
                WHERE symbol = $1
                  AND timeframe = tf.timeframe
                ORDER BY open_time DESC
                LIMIT $3
            ) AS k
            ORDER BY k.timeframe, k.open_time ASC
            "#,
        )
        .bind(symbol)
        .bind(timeframes)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        // 타임프레임별로 그룹화
        let mut grouped: HashMap<String, Vec<KlineRecord>> = HashMap::new();
        for record in records {
            grouped
                .entry(record.timeframe.clone())
                .or_default()
                .push(record);
        }

        debug!(
            "Fetched {} latest klines for {} across {} timeframes (limit={})",
            grouped.values().map(|v| v.len()).sum::<usize>(),
            symbol,
            timeframes.len(),
            limit
        );

        Ok(grouped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_kline_creation() {
        let kline = NewKline {
            symbol: "AAPL".to_string(),
            timeframe: "1d".to_string(),
            open_time: Utc::now(),
            open: Decimal::from(150),
            high: Decimal::from(155),
            low: Decimal::from(149),
            close: Decimal::from(154),
            volume: Decimal::from(1000000),
            close_time: None,
        };

        assert_eq!(kline.symbol, "AAPL");
        assert_eq!(kline.timeframe, "1d");
        assert!(kline.high >= kline.low);
    }
}
