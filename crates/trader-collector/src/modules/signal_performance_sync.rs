//! 신호 성과 동기화 모듈.
//!
//! signal_marker 테이블의 신호에 대해 N일 후 수익률을 계산하여
//! signal_performance 테이블에 저장합니다.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sqlx::PgPool;
use std::time::Instant;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::config::SignalPerformanceConfig;
use crate::error::CollectorError;
use crate::stats::CollectionStats;
use crate::Result;

/// 신호 성과 조회 결과 타입 (N일 후 종가 + MFE/MAE)
type SignalPerformanceRow = (
    Option<Decimal>, Option<Decimal>, Option<Decimal>, Option<Decimal>, Option<Decimal>,
    Option<Decimal>, Option<Decimal>,
);

/// 신호 성과 동기화 옵션
#[derive(Debug, Clone)]
pub struct SignalPerformanceSyncOptions {
    /// 최소 경과 일수 (신호 발생 후 N일 경과해야 계산)
    pub min_days_after: u32,
    /// 최대 추적 일수
    pub max_days: u32,
    /// 배치 크기
    pub batch_size: usize,
    /// 중단점부터 재개
    pub resume: bool,
}

impl Default for SignalPerformanceSyncOptions {
    fn default() -> Self {
        Self {
            min_days_after: 1,
            max_days: 20,
            batch_size: 100,
            resume: false,
        }
    }
}

impl From<&SignalPerformanceConfig> for SignalPerformanceSyncOptions {
    fn from(config: &SignalPerformanceConfig) -> Self {
        Self {
            min_days_after: config.min_days_after,
            max_days: config.max_days,
            batch_size: config.batch_size,
            resume: false,
        }
    }
}

/// 미완료 신호 정보
#[derive(Debug)]
struct PendingSignal {
    id: Uuid,
    symbol_id: Uuid,
    ticker: String,
    timestamp: DateTime<Utc>,
    signal_type: String,
    side: Option<String>,
    price: Decimal,
    strength: f64,
    strategy_id: String,
}

/// 신호 성과 동기화 실행.
///
/// # 동작
/// 1. 미완료 신호 조회 (calculated_at IS NULL)
/// 2. 각 신호에 대해 N일 후 가격 조회
/// 3. 수익률 및 MFE/MAE 계산
/// 4. signal_performance 테이블에 UPSERT
pub async fn sync_signal_performance(
    pool: &PgPool,
    options: SignalPerformanceSyncOptions,
) -> Result<CollectionStats> {
    let start = Instant::now();
    let mut stats = CollectionStats::new();

    // 미완료 신호 조회
    let pending_signals = get_pending_signals(
        pool,
        options.min_days_after,
        options.batch_size as i64,
    ).await?;

    if pending_signals.is_empty() {
        info!("처리할 미완료 신호가 없습니다");
        stats.elapsed = start.elapsed();
        return Ok(stats);
    }

    info!("신호 성과 계산 시작: {} 신호", pending_signals.len());
    stats.total = pending_signals.len();

    for signal in pending_signals {
        match calculate_and_save_performance(pool, &signal, options.max_days).await {
            Ok(true) => {
                stats.success += 1;
            }
            Ok(false) => {
                // 가격 데이터 부족으로 스킵
                stats.skipped += 1;
            }
            Err(e) => {
                warn!(
                    signal_id = %signal.id,
                    ticker = %signal.ticker,
                    error = %e,
                    "신호 성과 계산 실패"
                );
                stats.errors += 1;
            }
        }
    }

    stats.elapsed = start.elapsed();
    info!(
        "신호 성과 계산 완료: {}/{} 성공, {} 스킵, {} 오류",
        stats.success, stats.total, stats.skipped, stats.errors
    );

    Ok(stats)
}

/// 미완료 신호 조회.
/// signal_performance 테이블에 calculated_at이 NULL인 신호만 조회.
async fn get_pending_signals(
    pool: &PgPool,
    min_days_after: u32,
    limit: i64,
) -> Result<Vec<PendingSignal>> {
    let cutoff_time = Utc::now() - Duration::days(min_days_after as i64);

    let rows = sqlx::query_as::<_, (Uuid, Uuid, String, DateTime<Utc>, String, Option<String>, Decimal, f64, String)>(
        r#"
        SELECT
            sm.id,
            sm.symbol_id,
            si.ticker,
            sm.timestamp,
            sm.signal_type,
            sm.side,
            sm.price,
            sm.strength,
            sm.strategy_id
        FROM signal_marker sm
        JOIN symbol_info si ON sm.symbol_id = si.id
        LEFT JOIN signal_performance sp ON sm.id = sp.signal_id
        WHERE sp.calculated_at IS NULL
          AND sm.timestamp < $1
          AND sm.signal_type IN ('Entry', 'Exit')
        ORDER BY sm.timestamp ASC
        LIMIT $2
        "#,
    )
    .bind(cutoff_time)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(CollectorError::Database)?;

    Ok(rows
        .into_iter()
        .map(|(id, symbol_id, ticker, timestamp, signal_type, side, price, strength, strategy_id)| {
            PendingSignal {
                id,
                symbol_id,
                ticker,
                timestamp,
                signal_type,
                side,
                price,
                strength,
                strategy_id,
            }
        })
        .collect())
}

/// 단일 신호에 대해 성과 계산 및 저장.
///
/// 단일 쿼리로 1/3/5/10/20일 후 가격 + MFE/MAE를 한 번에 조회하여
/// DB 왕복을 최소화합니다 (기존 7회 → 1회).
async fn calculate_and_save_performance(
    pool: &PgPool,
    signal: &PendingSignal,
    max_days: u32,
) -> Result<bool> {
    let signal_price = signal.price;
    let side = signal.side.as_deref().unwrap_or("Buy");

    let signal_date = signal.timestamp.date_naive();
    let target_1d = (signal.timestamp + Duration::days(1)).date_naive();
    let target_3d = (signal.timestamp + Duration::days(3)).date_naive();
    let target_5d = (signal.timestamp + Duration::days(5)).date_naive();
    let target_10d = (signal.timestamp + Duration::days(10)).date_naive();
    let target_20d = (signal.timestamp + Duration::days(20)).date_naive();
    let mfe_end = (signal.timestamp + Duration::days(max_days as i64)).date_naive();

    // 단일 쿼리로 N일 후 가격 + MFE/MAE 한번에 조회
    let row: Option<SignalPerformanceRow> = sqlx::query_as(
        r#"
        SELECT
            -- N일 후 종가 (각 목표일 이후 첫 거래일 기준)
            (SELECT close FROM ohlcv WHERE symbol = $1 AND timeframe = '1d' AND open_time::date >= $3 ORDER BY open_time LIMIT 1) as price_1d,
            (SELECT close FROM ohlcv WHERE symbol = $1 AND timeframe = '1d' AND open_time::date >= $4 ORDER BY open_time LIMIT 1) as price_3d,
            (SELECT close FROM ohlcv WHERE symbol = $1 AND timeframe = '1d' AND open_time::date >= $5 ORDER BY open_time LIMIT 1) as price_5d,
            (SELECT close FROM ohlcv WHERE symbol = $1 AND timeframe = '1d' AND open_time::date >= $6 ORDER BY open_time LIMIT 1) as price_10d,
            (SELECT close FROM ohlcv WHERE symbol = $1 AND timeframe = '1d' AND open_time::date >= $7 ORDER BY open_time LIMIT 1) as price_20d,
            -- MFE/MAE용 고가/저가 (신호일 다음날 ~ max_days일 이내)
            (SELECT MAX(high) FROM ohlcv WHERE symbol = $1 AND timeframe = '1d' AND open_time::date > $2 AND open_time::date <= $8) as max_high,
            (SELECT MIN(low) FROM ohlcv WHERE symbol = $1 AND timeframe = '1d' AND open_time::date > $2 AND open_time::date <= $8) as min_low
        "#,
    )
    .bind(&signal.ticker)   // $1
    .bind(signal_date)       // $2
    .bind(target_1d)         // $3
    .bind(target_3d)         // $4
    .bind(target_5d)         // $5
    .bind(target_10d)        // $6
    .bind(target_20d)        // $7
    .bind(mfe_end)           // $8
    .fetch_optional(pool)
    .await
    .map_err(CollectorError::Database)?;

    let (price_1d, price_3d, price_5d, price_10d, price_20d, max_high, min_low) =
        row.unwrap_or((None, None, None, None, None, None, None));

    // 최소 1일 후 가격이 없으면 스킵
    if price_1d.is_none() {
        debug!(
            ticker = %signal.ticker,
            signal_time = %signal.timestamp,
            "1일 후 가격 데이터 없음, 스킵"
        );
        return Ok(false);
    }

    // 수익률 계산 (매도 신호는 부호 반전)
    let calc_return = |price_nd: Option<Decimal>| -> Option<Decimal> {
        price_nd.map(|p| {
            if side == "Sell" {
                (signal_price - p) / signal_price * dec!(100)
            } else {
                (p - signal_price) / signal_price * dec!(100)
            }
        })
    };

    let return_1d = calc_return(price_1d);
    let return_3d = calc_return(price_3d);
    let return_5d = calc_return(price_5d);
    let return_10d = calc_return(price_10d);
    let return_20d = calc_return(price_20d);

    // MFE/MAE 계산
    let (max_return, max_drawdown) = match (max_high, min_low) {
        (Some(high), Some(low)) => {
            if side == "Sell" {
                let mfe = (signal_price - low) / signal_price * dec!(100);
                let mae = (high - signal_price) / signal_price * dec!(-100);
                (Some(mfe), Some(mae))
            } else {
                let mfe = (high - signal_price) / signal_price * dec!(100);
                let mae = (low - signal_price) / signal_price * dec!(100);
                (Some(mfe), Some(mae))
            }
        }
        _ => (None, None),
    };

    // 승리 여부 판정 (5일 수익률 기준)
    let is_winner = return_5d.map(|r| r > Decimal::ZERO);

    // DB UPSERT
    sqlx::query(
        r#"
        INSERT INTO signal_performance (
            signal_id, symbol_id, ticker, signal_price,
            price_1d, price_3d, price_5d, price_10d, price_20d,
            return_1d, return_3d, return_5d, return_10d, return_20d,
            max_return, max_drawdown,
            signal_type, side, strength, strategy_id,
            is_winner, calculated_at
        ) VALUES (
            $1, $2, $3, $4,
            $5, $6, $7, $8, $9,
            $10, $11, $12, $13, $14,
            $15, $16,
            $17, $18, $19, $20,
            $21, NOW()
        )
        ON CONFLICT (signal_id) DO UPDATE SET
            price_1d = EXCLUDED.price_1d,
            price_3d = EXCLUDED.price_3d,
            price_5d = EXCLUDED.price_5d,
            price_10d = EXCLUDED.price_10d,
            price_20d = EXCLUDED.price_20d,
            return_1d = EXCLUDED.return_1d,
            return_3d = EXCLUDED.return_3d,
            return_5d = EXCLUDED.return_5d,
            return_10d = EXCLUDED.return_10d,
            return_20d = EXCLUDED.return_20d,
            max_return = EXCLUDED.max_return,
            max_drawdown = EXCLUDED.max_drawdown,
            is_winner = EXCLUDED.is_winner,
            calculated_at = NOW()
        "#,
    )
    .bind(signal.id)
    .bind(signal.symbol_id)
    .bind(&signal.ticker)
    .bind(signal_price)
    .bind(price_1d)
    .bind(price_3d)
    .bind(price_5d)
    .bind(price_10d)
    .bind(price_20d)
    .bind(return_1d)
    .bind(return_3d)
    .bind(return_5d)
    .bind(return_10d)
    .bind(return_20d)
    .bind(max_return)
    .bind(max_drawdown)
    .bind(&signal.signal_type)
    .bind(&signal.side)
    .bind(Decimal::try_from(signal.strength).unwrap_or(Decimal::ZERO))
    .bind(&signal.strategy_id)
    .bind(is_winner)
    .execute(pool)
    .await
    .map_err(CollectorError::Database)?;

    debug!(
        ticker = %signal.ticker,
        return_5d = ?return_5d,
        is_winner = ?is_winner,
        "신호 성과 저장 완료"
    );

    Ok(true)
}





#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_return_calculation() {
        let signal_price = dec!(10000);
        let price_after = dec!(10500);

        // 매수: 상승이 수익
        let buy_return = (price_after - signal_price) / signal_price * dec!(100);
        assert_eq!(buy_return, dec!(5)); // +5%

        // 매도: 하락이 수익
        let sell_return = (signal_price - price_after) / signal_price * dec!(100);
        assert_eq!(sell_return, dec!(-5)); // -5% (매도 후 상승 = 손실)
    }
}
