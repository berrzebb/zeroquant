//! OHLCV 캔들 데이터 캐시.
//!
//! 전략, 백테스팅, 시뮬레이션, 트레이딩에서 공통으로 사용하는
//! 캔들 데이터를 캐시하고 증분 업데이트합니다.
//!
//! # 동작 방식
//!
//! 1. 데이터 요청 시 캐시 확인
//! 2. 캐시에 없거나 오래된 경우 데이터 소스에서 가져옴
//! 3. 새 데이터를 DB에 저장 (증분 업데이트)
//! 4. 캐시된 데이터 반환
//!
//! # 사용 예제
//!
//! ```rust,ignore
//! use trader_data::OhlcvCache;
//!
//! let cache = OhlcvCache::new(pool).await?;
//! let klines = cache.get_klines("AAPL", Timeframe::D1, 100).await?;
//! ```

use crate::error::{DataError, Result};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use tracing::{debug, info, instrument};
use trader_core::{Kline, Timeframe};

/// OHLCV 캔들 데이터베이스 레코드.
#[derive(Debug, Clone, FromRow)]
pub struct OhlcvRecord {
    pub symbol: String,
    pub timeframe: String,
    pub open_time: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub close_time: Option<DateTime<Utc>>,
    pub fetched_at: Option<DateTime<Utc>>,
}

impl OhlcvRecord {
    /// Kline 도메인 객체로 변환.
    ///
    /// DB에 저장된 심볼(데이터 소스 형식)을 그대로 사용합니다.
    /// 중립 심볼(canonical)로 변환이 필요한 경우 `to_kline_with_canonical`을 사용하세요.
    pub fn to_kline(&self) -> Kline {
        let timeframe = self.timeframe.parse().unwrap_or(Timeframe::D1);
        let close_time = self
            .close_time
            .unwrap_or_else(|| self.open_time + timeframe_to_duration(timeframe));

        // ticker 문자열만 저장
        Kline {
            ticker: self.symbol.clone(),
            timeframe,
            open_time: self.open_time,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.volume,
            close_time,
            quote_volume: None,
            num_trades: None,
        }
    }

    /// 중립 심볼(canonical)을 지정하여 Kline으로 변환.
    ///
    /// SymbolResolver를 통해 미리 조회한 canonical 심볼을 전달합니다.
    ///
    /// # Arguments
    /// * `canonical` - 중립 심볼 (예: "005930", "AAPL", "BTC/USDT")
    /// * `quote` - 호가 통화 (예: "KRW", "USD", "USDT")
    /// * `market_type` - 시장 유형
    pub fn to_kline_with_canonical(
        &self,
        canonical: &str,
        _quote: &str,
        _market_type: trader_core::MarketType,
    ) -> Kline {
        let timeframe = self.timeframe.parse().unwrap_or(Timeframe::D1);
        let close_time = self
            .close_time
            .unwrap_or_else(|| self.open_time + timeframe_to_duration(timeframe));

        // ticker 문자열 사용 (canonical)
        Kline {
            ticker: canonical.to_string(),
            timeframe,
            open_time: self.open_time,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.volume,
            close_time,
            quote_volume: None,
            num_trades: None,
        }
    }
}

/// 캐시 메타데이터 레코드.
#[derive(Debug, Clone, FromRow)]
pub struct OhlcvMetadataRecord {
    pub symbol: String,
    pub timeframe: String,
    pub first_cached_time: Option<DateTime<Utc>>,
    pub last_cached_time: Option<DateTime<Utc>>,
    pub last_updated_at: Option<DateTime<Utc>>,
    pub total_candles: Option<i32>,
}

/// OHLCV 캔들 캐시 서비스.
///
/// 요청 기반 자동 캐싱과 증분 업데이트를 제공합니다.
#[derive(Clone)]
pub struct OhlcvCache {
    pool: PgPool,
}

impl OhlcvCache {
    /// 새로운 캐시 서비스 생성.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 캐시에서 캔들 데이터 조회.
    ///
    /// 최신 `limit`개의 캔들을 반환합니다.
    #[instrument(skip(self))]
    pub async fn get_cached_klines(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>> {
        let tf_str = timeframe_to_string(timeframe);

        let records: Vec<OhlcvRecord> = sqlx::query_as(
            r#"
            SELECT symbol, timeframe, open_time, open, high, low, close, volume, close_time, fetched_at
            FROM ohlcv
            WHERE symbol = $1 AND timeframe = $2
            ORDER BY open_time DESC
            LIMIT $3
            "#,
        )
        .bind(symbol)
        .bind(&tf_str)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DataError::QueryError(e.to_string()))?;

        // 시간순 정렬 (오래된 것부터)
        let mut klines: Vec<Kline> = records.into_iter().map(|r| r.to_kline()).collect();
        klines.reverse();

        debug!(
            symbol = symbol,
            timeframe = %tf_str,
            count = klines.len(),
            "캐시에서 캔들 조회"
        );

        Ok(klines)
    }

    /// 특정 시간 범위의 캔들 조회.
    #[instrument(skip(self))]
    pub async fn get_cached_klines_range(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Kline>> {
        let tf_str = timeframe_to_string(timeframe);

        let records: Vec<OhlcvRecord> = sqlx::query_as(
            r#"
            SELECT symbol, timeframe, open_time, open, high, low, close, volume, close_time, fetched_at
            FROM ohlcv
            WHERE symbol = $1 AND timeframe = $2 AND open_time >= $3 AND open_time < $4
            ORDER BY open_time ASC
            "#,
        )
        .bind(symbol)
        .bind(&tf_str)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DataError::QueryError(e.to_string()))?;

        let klines: Vec<Kline> = records.into_iter().map(|r| r.to_kline()).collect();

        Ok(klines)
    }

    /// 캔들 데이터를 캐시에 저장.
    ///
    /// ON CONFLICT로 중복 데이터 자동 처리.
    #[instrument(skip(self, klines), fields(count = klines.len()))]
    pub async fn save_klines(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        klines: &[Kline],
    ) -> Result<usize> {
        if klines.is_empty() {
            return Ok(0);
        }

        let tf_str = timeframe_to_string(timeframe);
        let mut inserted = 0;

        // UNNEST 패턴으로 일괄 삽입 (N+1 쿼리 문제 해결)
        for chunk in klines.chunks(500) {
            // 각 컬럼에 대한 배열 생성
            let symbols: Vec<&str> = chunk.iter().map(|_| symbol).collect();
            let timeframes: Vec<&str> = chunk.iter().map(|_| tf_str.as_str()).collect();
            let open_times: Vec<DateTime<Utc>> = chunk.iter().map(|k| k.open_time).collect();
            let opens: Vec<Decimal> = chunk.iter().map(|k| k.open).collect();
            let highs: Vec<Decimal> = chunk.iter().map(|k| k.high).collect();
            let lows: Vec<Decimal> = chunk.iter().map(|k| k.low).collect();
            let closes: Vec<Decimal> = chunk.iter().map(|k| k.close).collect();
            let volumes: Vec<Decimal> = chunk.iter().map(|k| k.volume).collect();
            let close_times: Vec<DateTime<Utc>> = chunk.iter().map(|k| k.close_time).collect();

            let result = sqlx::query(
                r#"
                INSERT INTO ohlcv
                    (symbol, timeframe, open_time, open, high, low, close, volume, close_time, fetched_at)
                SELECT * FROM UNNEST(
                    $1::text[], $2::text[], $3::timestamptz[],
                    $4::numeric[], $5::numeric[], $6::numeric[], $7::numeric[], $8::numeric[],
                    $9::timestamptz[]
                ), NOW()
                ON CONFLICT (symbol, timeframe, open_time) DO UPDATE SET
                    high = GREATEST(ohlcv.high, EXCLUDED.high),
                    low = LEAST(ohlcv.low, EXCLUDED.low),
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
            .execute(&self.pool)
            .await
            .map_err(|e| DataError::InsertError(e.to_string()))?;

            inserted += result.rows_affected() as usize;
        }

        info!(
            symbol = symbol,
            timeframe = %tf_str,
            inserted = inserted,
            "캔들 데이터 캐시에 저장"
        );

        Ok(inserted)
    }

    /// 캐시 메타데이터 조회.
    ///
    /// 증분 업데이트를 위해 마지막 캐시 시간 확인.
    pub async fn get_cache_metadata(
        &self,
        symbol: &str,
        timeframe: Timeframe,
    ) -> Result<Option<OhlcvMetadataRecord>> {
        let tf_str = timeframe_to_string(timeframe);

        sqlx::query_as(
            r#"
            SELECT symbol, timeframe, first_cached_time, last_cached_time, last_updated_at, total_candles
            FROM ohlcv_metadata
            WHERE symbol = $1 AND timeframe = $2
            "#,
        )
        .bind(symbol)
        .bind(&tf_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DataError::QueryError(e.to_string()))
    }

    /// 캐시에서 가장 최근 캔들의 시간 조회.
    ///
    /// 증분 업데이트 시 시작점 결정에 사용.
    pub async fn get_last_cached_time(
        &self,
        symbol: &str,
        timeframe: Timeframe,
    ) -> Result<Option<DateTime<Utc>>> {
        let tf_str = timeframe_to_string(timeframe);

        let result: Option<(DateTime<Utc>,)> = sqlx::query_as(
            r#"
            SELECT open_time FROM ohlcv
            WHERE symbol = $1 AND timeframe = $2
            ORDER BY open_time DESC
            LIMIT 1
            "#,
        )
        .bind(symbol)
        .bind(&tf_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DataError::QueryError(e.to_string()))?;

        Ok(result.map(|(t,)| t))
    }

    /// 캐시된 데이터 수 조회.
    pub async fn get_cached_count(&self, symbol: &str, timeframe: Timeframe) -> Result<i64> {
        let tf_str = timeframe_to_string(timeframe);

        let result: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM ohlcv
            WHERE symbol = $1 AND timeframe = $2
            "#,
        )
        .bind(symbol)
        .bind(&tf_str)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DataError::QueryError(e.to_string()))?;

        Ok(result.0)
    }

    /// 오래된 캐시 삭제 (데이터 보존 정책).
    ///
    /// - 분봉: 90일 이전 데이터 삭제
    /// - 일봉 이상: 5년 이전 데이터 삭제
    pub async fn cleanup_old_cache(&self, symbol: &str, timeframe: Timeframe) -> Result<u64> {
        let tf_str = timeframe_to_string(timeframe);

        let retention_days = if is_intraday(timeframe) {
            90 // 분봉/시간봉: 90일
        } else {
            365 * 5 // 일봉 이상: 5년
        };

        let cutoff = Utc::now() - Duration::days(retention_days);

        let result = sqlx::query(
            r#"
            DELETE FROM ohlcv
            WHERE symbol = $1 AND timeframe = $2 AND open_time < $3
            "#,
        )
        .bind(symbol)
        .bind(&tf_str)
        .bind(cutoff)
        .execute(&self.pool)
        .await
        .map_err(|e| DataError::DeleteError(e.to_string()))?;

        let deleted = result.rows_affected();
        if deleted > 0 {
            info!(
                symbol = symbol,
                timeframe = %tf_str,
                deleted = deleted,
                "오래된 캐시 삭제"
            );
        }

        Ok(deleted)
    }

    /// 전체 캐시 통계 조회.
    pub async fn get_all_cache_stats(&self) -> Result<Vec<OhlcvMetadataRecord>> {
        sqlx::query_as(
            r#"
            SELECT symbol, timeframe, first_cached_time, last_cached_time, last_updated_at, total_candles
            FROM ohlcv_metadata
            ORDER BY last_updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DataError::QueryError(e.to_string()))
    }

    /// 특정 심볼의 모든 타임프레임 캐시 삭제.
    pub async fn clear_symbol_cache(&self, symbol: &str) -> Result<u64> {
        let result = sqlx::query("DELETE FROM ohlcv WHERE symbol = $1")
            .bind(symbol)
            .execute(&self.pool)
            .await
            .map_err(|e| DataError::DeleteError(e.to_string()))?;

        // 메타데이터도 삭제
        sqlx::query("DELETE FROM ohlcv_metadata WHERE symbol = $1")
            .bind(symbol)
            .execute(&self.pool)
            .await
            .ok();

        info!(
            symbol = symbol,
            deleted = result.rows_affected(),
            "심볼 캐시 삭제"
        );
        Ok(result.rows_affected())
    }
}

// =============================================================================
// 헬퍼 함수
// =============================================================================

/// Timeframe을 DB 저장용 문자열로 변환.
pub fn timeframe_to_string(timeframe: Timeframe) -> String {
    match timeframe {
        Timeframe::M1 => "1m".to_string(),
        Timeframe::M3 => "5m".to_string(), // 3분봉은 5분봉으로 대체
        Timeframe::M5 => "5m".to_string(),
        Timeframe::M15 => "15m".to_string(),
        Timeframe::M30 => "30m".to_string(),
        Timeframe::H1 => "1h".to_string(),
        Timeframe::H2 => "1h".to_string(),
        Timeframe::H4 => "1h".to_string(),
        Timeframe::H6 => "1h".to_string(),
        Timeframe::H8 => "1h".to_string(),
        Timeframe::H12 => "1h".to_string(),
        Timeframe::D1 => "1d".to_string(),
        Timeframe::D3 => "1d".to_string(),
        Timeframe::W1 => "1wk".to_string(),
        Timeframe::MN1 => "1mo".to_string(),
    }
}

/// Timeframe의 Duration 계산.
fn timeframe_to_duration(timeframe: Timeframe) -> Duration {
    match timeframe {
        Timeframe::M1 => Duration::minutes(1),
        Timeframe::M3 => Duration::minutes(3),
        Timeframe::M5 => Duration::minutes(5),
        Timeframe::M15 => Duration::minutes(15),
        Timeframe::M30 => Duration::minutes(30),
        Timeframe::H1 => Duration::hours(1),
        Timeframe::H2 => Duration::hours(2),
        Timeframe::H4 => Duration::hours(4),
        Timeframe::H6 => Duration::hours(6),
        Timeframe::H8 => Duration::hours(8),
        Timeframe::H12 => Duration::hours(12),
        Timeframe::D1 => Duration::days(1),
        Timeframe::D3 => Duration::days(3),
        Timeframe::W1 => Duration::weeks(1),
        Timeframe::MN1 => Duration::days(30),
    }
}

/// 분봉/시간봉인지 확인.
fn is_intraday(timeframe: Timeframe) -> bool {
    matches!(
        timeframe,
        Timeframe::M1
            | Timeframe::M3
            | Timeframe::M5
            | Timeframe::M15
            | Timeframe::M30
            | Timeframe::H1
            | Timeframe::H2
            | Timeframe::H4
            | Timeframe::H6
            | Timeframe::H8
            | Timeframe::H12
    )
}

// =============================================================================
// 테스트
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::historical::guess_currency;

    #[test]
    fn test_timeframe_to_string() {
        assert_eq!(timeframe_to_string(Timeframe::M1), "1m");
        assert_eq!(timeframe_to_string(Timeframe::H1), "1h");
        assert_eq!(timeframe_to_string(Timeframe::D1), "1d");
        assert_eq!(timeframe_to_string(Timeframe::W1), "1wk");
        assert_eq!(timeframe_to_string(Timeframe::MN1), "1mo");
    }

    #[test]
    fn test_is_intraday() {
        assert!(is_intraday(Timeframe::M1));
        assert!(is_intraday(Timeframe::H1));
        assert!(!is_intraday(Timeframe::D1));
        assert!(!is_intraday(Timeframe::W1));
    }

    #[test]
    fn test_guess_currency() {
        assert_eq!(guess_currency("005930.KS"), "KRW");
        assert_eq!(guess_currency("AAPL"), "USD");
        assert_eq!(guess_currency("7203.T"), "JPY");
    }
}
