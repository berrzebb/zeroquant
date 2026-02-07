//! 관심종목(Watchlist) 및 전략 관심 종목 우선 처리를 위한 헬퍼 모듈.
//!
//! 두 가지 소스의 관심 종목을 통합하여 우선 처리합니다:
//! - **수동 관심종목**: `watchlist_item` 테이블 (사용자가 UI에서 등록)
//! - **전략 관심종목**: `strategy_watched_tickers` 테이블 (전략이 자동 등록)
//!
//! OHLCV, Indicator, GlobalScore 동기화에서 이 종목들을 먼저 처리합니다.

use sqlx::PgPool;
use std::collections::HashSet;

use crate::error::CollectorError;
use crate::Result;

/// Watchlist에 등록된 모든 심볼(ticker) 목록 조회.
///
/// `watchlist_item` 테이블에서 DISTINCT ticker를 가져옵니다.
/// `idx_watchlist_item_symbol` 인덱스를 활용하여 빠르게 조회합니다.
pub async fn fetch_watchlist_tickers(pool: &PgPool) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT wi.symbol
        FROM watchlist_item wi
        INNER JOIN symbol_info si ON wi.symbol = si.ticker
        WHERE si.is_active = true
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(CollectorError::Database)?;

    let tickers: Vec<String> = rows.into_iter().map(|(s,)| s).collect();
    if !tickers.is_empty() {
        tracing::info!(count = tickers.len(), "관심종목 심볼 로드 완료");
    }

    Ok(tickers)
}

/// 전략에서 등록한 관심 종목 조회.
///
/// `strategy_watched_tickers` 테이블에서 DISTINCT ticker를 가져옵니다.
/// 전략이 시작될 때 config의 고정 티커와 스크리닝 결과의 동적 티커가 등록됩니다.
pub async fn fetch_strategy_watched_tickers(pool: &PgPool) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT swt.ticker
        FROM strategy_watched_tickers swt
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(CollectorError::Database)?;

    let tickers: Vec<String> = rows.into_iter().map(|(s,)| s).collect();
    if !tickers.is_empty() {
        tracing::info!(count = tickers.len(), "전략 관심종목 로드 완료");
    }

    Ok(tickers)
}

/// 수동 관심종목 + 전략 관심종목을 통합 조회.
///
/// 두 테이블의 UNION으로 중복을 제거합니다.
/// Collector에서 우선순위 처리에 사용합니다.
pub async fn fetch_all_priority_tickers(pool: &PgPool) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT ticker FROM (
            SELECT wi.symbol AS ticker
            FROM watchlist_item wi
            INNER JOIN symbol_info si ON wi.symbol = si.ticker
            WHERE si.is_active = true

            UNION

            SELECT swt.ticker
            FROM strategy_watched_tickers swt
        ) AS combined
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(CollectorError::Database)?;

    let tickers: Vec<String> = rows.into_iter().map(|(s,)| s).collect();
    if !tickers.is_empty() {
        tracing::info!(
            count = tickers.len(),
            "통합 우선순위 종목 로드 완료 (관심종목 + 전략)"
        );
    }

    Ok(tickers)
}

/// Watchlist 티커 목록을 HashSet으로 변환 (O(1) 멤버십 체크용).
pub fn to_hashset(tickers: &[String]) -> HashSet<String> {
    tickers.iter().cloned().collect()
}
