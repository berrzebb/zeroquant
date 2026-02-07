//! 종목 목록 조회 기능.

use anyhow::{Context, Result};
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use trader_data::{Database, DatabaseConfig};
use tracing::info;

/// 종목 조회 설정.
#[derive(Debug)]
pub struct ListSymbolsConfig {
    /// 시장 필터
    pub market: String,
    /// 활성화된 종목만
    pub active_only: bool,
    /// 출력 형식
    pub format: OutputFormat,
    /// 출력 파일 경로
    pub output: Option<String>,
    /// 검색 키워드
    pub search: Option<String>,
    /// 최대 결과 수
    pub limit: usize,
    /// 데이터베이스 URL
    pub db_url: Option<String>,
}

/// 출력 형식.
#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Table,
    Csv,
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "table" => Ok(Self::Table),
            "csv" => Ok(Self::Csv),
            "json" => Ok(Self::Json),
            _ => Err(anyhow::anyhow!(
                "Invalid format: {}. Use: table, csv, json",
                s
            )),
        }
    }
}

/// 종목 정보.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct SymbolInfo {
    pub ticker: String,
    pub name: String,
    pub market: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sector: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yahoo_symbol: Option<String>,
    pub is_active: bool,
}

/// 종목 목록 조회.
pub async fn list_symbols(config: ListSymbolsConfig) -> Result<usize> {
    // DB URL 가져오기
    let db_url = config
        .db_url
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "DATABASE_URL not found. Set DATABASE_URL environment variable or use --db-url flag"
            )
        })?;

    let db_config = DatabaseConfig::for_cli(db_url);
    let db = Database::connect(&db_config).await
        .context("데이터베이스 연결 실패")?;
    let pool = db.pool().clone();

    // 쿼리 생성
    let mut query = String::from("SELECT ticker, name, market, exchange, sector, yahoo_symbol, is_active FROM symbol_info WHERE 1=1");

    // 시장 필터
    if config.market.to_uppercase() != "ALL" {
        query.push_str(&format!(" AND market = '{}'", config.market.to_uppercase()));
    }

    // 활성화 필터
    if config.active_only {
        query.push_str(" AND is_active = true");
    }

    // 검색 키워드
    if let Some(ref search) = config.search {
        query.push_str(&format!(
            " AND (ticker ILIKE '%{}%' OR name ILIKE '%{}%')",
            search, search
        ));
    }

    // 정렬
    query.push_str(" ORDER BY market, ticker");

    // 제한
    if config.limit > 0 {
        query.push_str(&format!(" LIMIT {}", config.limit));
    }

    info!("Querying symbols...");
    let symbols: Vec<SymbolInfo> = sqlx::query_as(&query)
        .fetch_all(&pool)
        .await
        .context("Failed to query symbols")?;

    pool.close().await;

    info!("Found {} symbols", symbols.len());

    // 출력
    output_symbols(&symbols, config.format, config.output.as_deref())?;

    Ok(symbols.len())
}

/// 종목 목록 출력.
fn output_symbols(
    symbols: &[SymbolInfo],
    format: OutputFormat,
    output_path: Option<&str>,
) -> Result<()> {
    let content = match format {
        OutputFormat::Table => format_table(symbols),
        OutputFormat::Csv => format_csv(symbols),
        OutputFormat::Json => format_json(symbols)?,
    };

    // 파일 또는 stdout에 출력
    if let Some(path) = output_path {
        let mut file = File::create(path)
            .with_context(|| format!("Failed to create output file: {}", path))?;
        file.write_all(content.as_bytes())
            .context("Failed to write to file")?;
        info!("Output written to: {}", path);
    } else {
        println!("{}", content);
    }

    Ok(())
}

/// 테이블 형식 출력.
fn format_table(symbols: &[SymbolInfo]) -> String {
    let mut output = String::new();

    // 헤더
    output.push_str(&format!(
        "{:<12} {:<50} {:<8} {:<12} {:<30} {:<15} {:<8}\n",
        "TICKER", "NAME", "MARKET", "EXCHANGE", "SECTOR", "YAHOO_SYMBOL", "ACTIVE"
    ));
    output.push_str(&"-".repeat(145));
    output.push('\n');

    // 데이터
    for symbol in symbols {
        output.push_str(&format!(
            "{:<12} {:<50} {:<8} {:<12} {:<30} {:<15} {:<8}\n",
            symbol.ticker,
            truncate(&symbol.name, 50),
            symbol.market,
            symbol.exchange.as_deref().unwrap_or("-"),
            truncate(symbol.sector.as_deref().unwrap_or("-"), 30),
            symbol.yahoo_symbol.as_deref().unwrap_or("-"),
            if symbol.is_active { "✓" } else { "✗" }
        ));
    }

    // 요약
    output.push('\n');
    output.push_str(&format!("Total: {} symbols", symbols.len()));

    // 시장별 통계
    let mut market_stats: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for symbol in symbols {
        *market_stats.entry(symbol.market.clone()).or_insert(0) += 1;
    }

    if market_stats.len() > 1 {
        output.push_str("\n\nBy Market:\n");
        let mut markets: Vec<_> = market_stats.iter().collect();
        markets.sort_by_key(|(market, _)| *market);
        for (market, count) in markets {
            output.push_str(&format!("  {}: {}\n", market, count));
        }
    }

    output
}

/// CSV 형식 출력.
fn format_csv(symbols: &[SymbolInfo]) -> String {
    let mut output = String::new();

    // 헤더
    output.push_str("ticker,name,market,exchange,sector,yahoo_symbol,is_active\n");

    // 데이터
    for symbol in symbols {
        output.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            symbol.ticker,
            escape_csv(&symbol.name),
            symbol.market,
            symbol.exchange.as_deref().unwrap_or(""),
            escape_csv(symbol.sector.as_deref().unwrap_or("")),
            symbol.yahoo_symbol.as_deref().unwrap_or(""),
            symbol.is_active
        ));
    }

    output
}

/// JSON 형식 출력.
fn format_json(symbols: &[SymbolInfo]) -> Result<String> {
    serde_json::to_string_pretty(symbols).context("Failed to serialize to JSON")
}

/// 문자열 자르기 (UTF-8 안전).
fn truncate(s: &str, max_len: usize) -> String {
    // 문자 수로 계산 (바이트가 아님)
    let char_count = s.chars().count();

    if char_count <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    }
}

/// CSV 이스케이프 (콤마나 따옴표 포함 시 따옴표로 감싸기).
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
