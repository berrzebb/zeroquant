//! 스크리닝 Materialized View 갱신 모듈.
//!
//! `mv_symbol_screening` Materialized View를 갱신하여
//! 스크리닝 쿼리 성능을 최적화합니다.

use sqlx::PgPool;
use std::time::Instant;
use tracing::{debug, info};

use crate::{CollectionStats, CollectorError, Result};

/// 스크리닝 Materialized View 갱신.
///
/// `mv_symbol_screening`은 symbol_info, symbol_fundamental, symbol_global_score를
/// 조인한 통합 뷰로, 스크리닝 쿼리 성능을 크게 향상시킵니다.
///
/// # 주의사항
/// - CONCURRENTLY 옵션으로 갱신하여 읽기 차단 없음
/// - 갱신 중에도 기존 데이터로 조회 가능
/// - 전체 갱신에 수 초 ~ 수십 초 소요 (데이터 양에 따라 다름)
pub async fn refresh_screening_view(pool: &PgPool) -> Result<CollectionStats> {
    let start = Instant::now();
    info!("스크리닝 Materialized View 갱신 시작");

    // CONCURRENTLY 옵션: 읽기 차단 없이 갱신
    // 단, UNIQUE INDEX가 있어야 사용 가능 (idx_mv_screening_symbol_id)
    let result = sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY mv_symbol_screening")
        .execute(pool)
        .await;

    let elapsed = start.elapsed();

    match result {
        Ok(_) => {
            // 갱신된 레코드 수 조회
            let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mv_symbol_screening")
                .fetch_one(pool)
                .await
                .unwrap_or((0,));

            info!(
                rows = count.0,
                elapsed_ms = elapsed.as_millis(),
                "스크리닝 Materialized View 갱신 완료"
            );

            Ok(CollectionStats {
                total: count.0 as usize,
                success: count.0 as usize,
                errors: 0,
                skipped: 0,
                empty: 0,
                total_klines: 0,
                elapsed,
            })
        }
        Err(e) => {
            // Materialized View가 없는 경우 (마이그레이션 미적용)
            if e.to_string().contains("does not exist") {
                debug!("mv_symbol_screening이 존재하지 않습니다. 마이그레이션을 확인하세요.");
                Ok(CollectionStats {
                    total: 0,
                    success: 0,
                    errors: 0,
                    skipped: 1,
                    empty: 0,
                    total_klines: 0,
                    elapsed,
                })
            } else {
                Err(CollectorError::Database(e))
            }
        }
    }
}

/// 섹터 Relative Strength Materialized View 갱신.
///
/// `mv_sector_rs`는 섹터별 RS(상대강도)를 사전 계산한 뷰로,
/// API의 섹터 랭킹 조회 성능을 크게 향상시킵니다.
///
/// # 주의사항
/// - CONCURRENTLY 옵션으로 갱신하여 읽기 차단 없음
/// - OHLCV 수집 완료 후 호출 권장 (최신 가격 데이터 반영)
pub async fn refresh_sector_rs_view(pool: &PgPool) -> Result<CollectionStats> {
    let start = Instant::now();
    debug!("섹터 RS Materialized View 갱신 시작");

    let result = sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY mv_sector_rs")
        .execute(pool)
        .await;

    let elapsed = start.elapsed();

    match result {
        Ok(_) => {
            let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mv_sector_rs")
                .fetch_one(pool)
                .await
                .unwrap_or((0,));

            info!(
                rows = count.0,
                elapsed_ms = elapsed.as_millis(),
                "섹터 RS Materialized View 갱신 완료"
            );

            Ok(CollectionStats {
                total: count.0 as usize,
                success: count.0 as usize,
                errors: 0,
                skipped: 0,
                empty: 0,
                total_klines: 0,
                elapsed,
            })
        }
        Err(e) => {
            if e.to_string().contains("does not exist") {
                debug!("mv_sector_rs가 존재하지 않습니다. 마이그레이션을 확인하세요.");
                Ok(CollectionStats {
                    total: 0,
                    success: 0,
                    errors: 0,
                    skipped: 1,
                    empty: 0,
                    total_klines: 0,
                    elapsed,
                })
            } else {
                Err(CollectorError::Database(e))
            }
        }
    }
}

/// 스크리닝 뷰 통계.
#[derive(Debug)]
pub struct ScreeningViewStats {
    pub total_rows: i64,
    pub with_score: i64,
    pub with_fundamental: i64,
    pub by_market: Vec<(String, i64)>,
}

/// 스크리닝 뷰 상태 조회.
pub async fn get_screening_view_stats(pool: &PgPool) -> Result<ScreeningViewStats> {
    // 전체 행 수
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mv_symbol_screening")
        .fetch_one(pool)
        .await?;

    // Global Score가 있는 행 수
    let with_score: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM mv_symbol_screening WHERE global_score IS NOT NULL")
            .fetch_one(pool)
            .await?;

    // Fundamental 데이터가 있는 행 수 (PER 기준)
    let with_fundamental: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM mv_symbol_screening WHERE per IS NOT NULL")
            .fetch_one(pool)
            .await?;

    // 시장별 행 수
    let by_market: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT market, COUNT(*) as cnt
        FROM mv_symbol_screening
        GROUP BY market
        ORDER BY cnt DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(ScreeningViewStats {
        total_rows: total.0,
        with_score: with_score.0,
        with_fundamental: with_fundamental.0,
        by_market,
    })
}
