//! 심볼 동기화 모듈.

use crate::{CollectionStats, CollectorConfig, Result};
use sqlx::PgPool;
use std::time::Instant;
use trader_data::provider::symbol_info::{KrxSymbolProvider, SymbolInfoProvider};

/// 심볼 정보 동기화
pub async fn sync_symbols(pool: &PgPool, config: &CollectorConfig) -> Result<CollectionStats> {
    let start = Instant::now();
    let mut stats = CollectionStats::new();

    tracing::info!("심볼 동기화 시작");

    // 1. 현재 심볼 수 확인
    let current_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM symbol_info")
        .fetch_one(pool)
        .await?;

    tracing::info!(
        current_count,
        min = config.symbol_sync.min_symbol_count,
        "심볼 수 확인"
    );

    if current_count >= config.symbol_sync.min_symbol_count {
        tracing::info!("심볼 수 충분, 동기화 건너뛰기");
        stats.skipped = 1;
        stats.elapsed = start.elapsed();
        return Ok(stats);
    }

    // 2. KRX 동기화
    if config.symbol_sync.enable_krx {
        tracing::info!("KRX 심볼 동기화 시작");
        match sync_krx_symbols(pool).await {
            Ok(count) => {
                stats.success += 1;
                stats.total += count;
                tracing::info!(count, "KRX 심볼 동기화 완료");
            }
            Err(e) => {
                stats.errors += 1;
                tracing::error!(error = %e, "KRX 동기화 실패");
            }
        }
    }

    // TODO: Binance, Yahoo 동기화 구현

    stats.elapsed = start.elapsed();
    Ok(stats)
}

/// KRX 심볼 동기화 (배치 UPSERT)
async fn sync_krx_symbols(pool: &PgPool) -> Result<usize> {
    let provider = KrxSymbolProvider::new();

    // KRX에서 종목 목록 조회
    let symbols = provider
        .fetch_all()
        .await
        .map_err(|e| crate::error::CollectorError::DataSource(e.to_string()))?;

    tracing::info!(count = symbols.len(), "KRX 종목 조회 완료");

    if symbols.is_empty() {
        return Ok(0);
    }

    // 배치 UPSERT (500개씩)
    const BATCH_SIZE: usize = 500;
    let mut total_affected = 0u64;

    for chunk in symbols.chunks(BATCH_SIZE) {
        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO symbol_info (id, ticker, name, name_en, market, exchange, sector, yahoo_symbol, is_active, created_at, updated_at) "
        );

        query_builder.push_values(chunk, |mut b, sym| {
            let ticker = sym.ticker.trim_end_matches(".KS");
            b.push("gen_random_uuid()")
                .push_bind(ticker.to_string())
                .push_bind(&sym.name)
                .push_bind(sym.name_en.as_deref())
                .push_bind(&sym.market)
                .push_bind(sym.exchange.as_deref())
                .push_bind(sym.sector.as_deref())
                .push_bind(sym.yahoo_symbol.as_deref())
                .push("true")
                .push("NOW()")
                .push("NOW()");
        });

        query_builder.push(
            " ON CONFLICT (ticker, market) DO UPDATE SET \
             name = EXCLUDED.name, \
             name_en = EXCLUDED.name_en, \
             exchange = EXCLUDED.exchange, \
             sector = EXCLUDED.sector, \
             yahoo_symbol = EXCLUDED.yahoo_symbol, \
             is_active = true, \
             updated_at = NOW()"
        );

        match query_builder.build().execute(pool).await {
            Ok(result) => {
                total_affected += result.rows_affected();
            }
            Err(e) => {
                tracing::error!(error = %e, "심볼 배치 저장 실패");
            }
        }
    }

    tracing::info!(
        total = symbols.len(),
        affected = total_affected,
        batches = symbols.len().div_ceil(BATCH_SIZE),
        "KRX 심볼 배치 UPSERT 완료"
    );

    Ok(total_affected as usize)
}
