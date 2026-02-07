//! 다중 타임프레임 성능 테스트.
//!
//! 목표: 3개 타임프레임 조회 < 50ms
//!
//! # 실행 방법
//!
//! ```bash
//! # DB 연결이 필요한 테스트
//! DATABASE_URL="postgresql://trader:trader_secret@localhost/trader" \
//!     cargo test -p trader-analytics --test multi_timeframe_perf -- --nocapture
//! ```

use std::collections::HashMap;
use std::time::Instant;

use rust_decimal::Decimal;
use trader_core::{Kline, Timeframe};

/// 다중 타임프레임 조회 성능 테스트 (모의 데이터)
#[test]
fn test_multi_timeframe_in_memory_performance() {
    // 모의 데이터 생성: 3개 타임프레임, 각 1000개 캔들
    let timeframes = vec![Timeframe::M5, Timeframe::H1, Timeframe::D1];
    let candle_count = 1000;

    let mut data: HashMap<Timeframe, Vec<Kline>> = HashMap::new();

    let start = Instant::now();

    // 데이터 생성 (실제 DB 조회 시뮬레이션)
    for tf in &timeframes {
        let klines = create_mock_klines(*tf, candle_count);
        data.insert(*tf, klines);
    }

    let elapsed = start.elapsed();
    println!(
        "모의 데이터 생성 ({} TF x {} candles): {:?}",
        timeframes.len(),
        candle_count,
        elapsed
    );

    // 데이터 처리 성능 테스트
    let start = Instant::now();

    let mut total_candles = 0;
    for (_tf, klines) in &data {
        total_candles += klines.len();
        // 간단한 처리: 마지막 종가 확인
        let _last_close = klines.last().map(|k| k.close);
    }

    let elapsed = start.elapsed();
    println!("데이터 처리 ({} candles): {:?}", total_candles, elapsed);

    // 목표: 50ms 이내
    assert!(
        elapsed.as_millis() < 50,
        "데이터 처리가 50ms를 초과: {:?}",
        elapsed
    );
}

/// HashMap 그룹화 성능 테스트
#[test]
fn test_grouping_performance() {
    use chrono::{TimeZone, Utc};
    use rust_decimal_macros::dec;

    // 대량의 플랫 데이터를 타임프레임별로 그룹화
    let total_records = 3000; // 각 TF당 1000개
    let timeframes = ["5m", "1h", "1d"];

    let mut flat_data: Vec<(String, Kline)> = Vec::with_capacity(total_records);

    for (idx, tf) in timeframes.iter().cycle().enumerate().take(total_records) {
        let kline = Kline {
            ticker: "BTC/USDT".to_string(),
            timeframe: Timeframe::M5, // 임시
            open_time: Utc
                .with_ymd_and_hms(2024, 1, 1, 0, idx as u32 % 24, 0)
                .unwrap(),
            open: dec!(50000),
            high: dec!(50100),
            low: dec!(49900),
            close: dec!(50050),
            volume: dec!(100),
            close_time: Utc
                .with_ymd_and_hms(2024, 1, 1, 0, idx as u32 % 24, 5)
                .unwrap(),
            quote_volume: None,
            num_trades: None,
        };
        flat_data.push((tf.to_string(), kline));
    }

    let start = Instant::now();

    // 그룹화 (UNION ALL 쿼리 결과 처리 시뮬레이션)
    let mut grouped: HashMap<String, Vec<Kline>> = HashMap::new();
    for (tf, kline) in flat_data {
        grouped.entry(tf).or_default().push(kline);
    }

    let elapsed = start.elapsed();
    println!(
        "그룹화 ({} records -> {} groups): {:?}",
        total_records,
        grouped.len(),
        elapsed
    );

    // 그룹화는 매우 빨라야 함 (< 10ms)
    assert!(
        elapsed.as_millis() < 10,
        "그룹화가 10ms를 초과: {:?}",
        elapsed
    );
}

/// 캐시 히트 시나리오 성능 테스트
#[test]
fn test_cache_hit_scenario() {
    let timeframes = vec![Timeframe::M5, Timeframe::H1, Timeframe::D1];
    let iterations = 100;

    // 캐시된 데이터 (메모리에 이미 존재)
    let mut cached_data: HashMap<Timeframe, Vec<Kline>> = HashMap::new();
    for tf in &timeframes {
        cached_data.insert(*tf, create_mock_klines(*tf, 1000));
    }

    let start = Instant::now();

    for _ in 0..iterations {
        for tf in &timeframes {
            let _data = cached_data.get(tf);
        }
    }

    let elapsed = start.elapsed();
    let avg_per_lookup = elapsed.as_micros() / (iterations * timeframes.len() as u32) as u128;

    println!(
        "캐시 히트 ({} iterations x {} TF): {:?}, 평균: {}µs/조회",
        iterations,
        timeframes.len(),
        elapsed,
        avg_per_lookup
    );

    // 캐시 히트는 마이크로초 단위여야 함
    assert!(
        avg_per_lookup < 10,
        "캐시 조회가 너무 느림: {}µs",
        avg_per_lookup
    );
}

/// 모의 Kline 데이터 생성
fn create_mock_klines(timeframe: Timeframe, count: usize) -> Vec<Kline> {
    use chrono::{Duration, TimeZone, Utc};
    use rust_decimal_macros::dec;

    let interval_secs = match timeframe {
        Timeframe::M1 => 60,
        Timeframe::M3 => 180,
        Timeframe::M5 => 300,
        Timeframe::M15 => 900,
        Timeframe::M30 => 1800,
        Timeframe::H1 => 3600,
        Timeframe::H2 => 7200,
        Timeframe::H4 => 14400,
        Timeframe::H6 => 21600,
        Timeframe::H8 => 28800,
        Timeframe::H12 => 43200,
        Timeframe::D1 => 86400,
        Timeframe::D3 => 259200,
        Timeframe::W1 => 604800,
        Timeframe::MN1 => 2592000,
    };

    let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

    (0..count)
        .map(|i| {
            let open_time = base_time + Duration::seconds(i as i64 * interval_secs);
            let close_time = open_time + Duration::seconds(interval_secs);

            Kline {
                ticker: "BTC/USDT".to_string(),
                timeframe,
                open_time,
                open: dec!(50000) + Decimal::from(i as i32),
                high: dec!(50100) + Decimal::from(i as i32),
                low: dec!(49900) + Decimal::from(i as i32),
                close: dec!(50050) + Decimal::from(i as i32),
                volume: dec!(100),
                close_time,
                quote_volume: None,
                num_trades: None,
            }
        })
        .collect()
}

// =============================================================================
// DB 연결 필요 테스트 (환경변수 DATABASE_URL 필요)
// =============================================================================

#[cfg(feature = "db_test")]
mod db_tests {
    use super::*;
    use sqlx::PgPool;
    use std::env;

    async fn get_pool() -> Option<PgPool> {
        let url = env::var("DATABASE_URL").ok()?;
        PgPool::connect(&url).await.ok()
    }

    #[tokio::test]
    async fn test_multi_timeframe_db_performance() {
        let Some(pool) = get_pool().await else {
            println!("DATABASE_URL 환경변수가 설정되지 않아 테스트 스킵");
            return;
        };

        // TODO: 실제 DB 조회 테스트 구현
        // KlinesRepository::get_latest_multi_timeframe 호출 후 성능 측정
        println!("DB 성능 테스트: 구현 예정");
    }
}
