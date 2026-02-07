//! 온라인 소스에서 종목 정보 자동 수집 및 DB 동기화.

use anyhow::{Context, Result};
use sqlx::PgPool;
use std::fs;
use std::path::Path;
use trader_data::{Database, DatabaseConfig};
use tracing::{error, info, warn};

/// 종목 수집 설정.
#[derive(Debug)]
pub struct FetchSymbolsConfig {
    /// 시장 유형
    pub market: String,
    /// CSV 파일로도 저장
    pub save_csv: bool,
    /// CSV 출력 디렉토리
    pub csv_dir: String,
    /// 데이터베이스 URL
    pub db_url: Option<String>,
    /// 드라이런 모드
    pub dry_run: bool,
}

/// 시장별 종목 수집 결과.
#[derive(Debug, Default)]
pub struct FetchResult {
    pub kr_count: usize,
    pub us_count: usize,
    pub crypto_count: usize,
    pub total: usize,
}

/// 온라인 소스에서 종목 정보 수집 및 동기화.
pub async fn fetch_symbols(config: FetchSymbolsConfig) -> Result<FetchResult> {
    println!("\n🔍 종목 정보 자동 수집 시작...");
    println!("대상 시장: {}", config.market);
    if config.dry_run {
        println!("⚠️  드라이런 모드: DB에 저장하지 않습니다");
    }
    println!();

    let mut result = FetchResult::default();

    // DB 연결 (드라이런이 아닐 때만)
    let pool = if !config.dry_run {
        let db_url = config
            .db_url
            .clone()
            .or_else(|| std::env::var("DATABASE_URL").ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "DATABASE_URL not found. Set DATABASE_URL environment variable or use --db-url flag"
                )
            })?;

        let db_config = DatabaseConfig::for_cli(db_url);
        let db = Database::connect(&db_config).await
            .context("데이터베이스 연결 실패")?;
        Some(db.pool().clone())
    } else {
        None
    };

    // CSV 디렉토리 생성
    if config.save_csv {
        fs::create_dir_all(&config.csv_dir)
            .with_context(|| format!("Failed to create CSV directory: {}", config.csv_dir))?;
    }

    let market_upper = config.market.to_uppercase();

    // 한국 시장
    if market_upper == "KR" || market_upper == "ALL" {
        match fetch_kr_symbols(pool.as_ref(), &config).await {
            Ok(count) => {
                result.kr_count = count;
                result.total += count;
                info!("✅ 한국 시장: {}개 종목 수집", count);
            }
            Err(e) => {
                error!("✗ 한국 시장 수집 실패: {}", e);
                warn!("계속 진행합니다...");
            }
        }
    }

    // 미국 시장
    if market_upper == "US" || market_upper == "ALL" {
        match fetch_us_symbols(pool.as_ref(), &config).await {
            Ok(count) => {
                result.us_count = count;
                result.total += count;
                info!("✅ 미국 시장: {}개 종목 수집", count);
            }
            Err(e) => {
                error!("✗ 미국 시장 수집 실패: {}", e);
                warn!("계속 진행합니다...");
            }
        }
    }

    // 암호화폐 시장
    if market_upper == "CRYPTO" || market_upper == "ALL" {
        match fetch_crypto_symbols(pool.as_ref(), &config).await {
            Ok(count) => {
                result.crypto_count = count;
                result.total += count;
                info!("✅ 암호화폐 시장: {}개 종목 수집", count);
            }
            Err(e) => {
                error!("✗ 암호화폐 시장 수집 실패: {}", e);
                warn!("계속 진행합니다...");
            }
        }
    }

    // DB 연결 종료
    if let Some(pool) = pool {
        pool.close().await;
    }

    println!("\n{}", "=".repeat(60));
    println!("✅ 종목 수집 완료!");
    println!("   한국: {}개", result.kr_count);
    println!("   미국: {}개", result.us_count);
    println!("   암호화폐: {}개", result.crypto_count);
    println!("   총: {}개", result.total);
    println!("{}\n", "=".repeat(60));

    Ok(result)
}

/// 한국 시장 종목 수집.
async fn fetch_kr_symbols(pool: Option<&PgPool>, config: &FetchSymbolsConfig) -> Result<usize> {
    println!("📊 한국 시장 수집 중 (KRX)...");

    use trader_data::provider::{KrxSymbolProvider, SymbolInfoProvider};

    let provider = KrxSymbolProvider::new();
    let symbols = provider
        .fetch_all()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch KRX symbols: {}", e))?;

    println!("   KRX에서 {}개 종목 조회 완료", symbols.len());

    // CSV 저장 (선택적)
    if config.save_csv {
        save_to_csv(&symbols, Path::new(&config.csv_dir).join("krx_symbols.csv"))?;
        println!("   CSV 저장: {}/krx_symbols.csv", config.csv_dir);
    }

    // DB 저장
    if let Some(pool) = pool {
        let new_symbols: Vec<trader_api::repository::NewSymbolInfo> = symbols
            .into_iter()
            .map(|s| trader_api::repository::NewSymbolInfo {
                ticker: s.ticker,
                name: s.name,
                name_en: s.name_en,
                market: s.market,
                exchange: s.exchange,
                sector: s.sector,
                yahoo_symbol: s.yahoo_symbol,
            })
            .collect();

        let upserted =
            trader_api::repository::SymbolInfoRepository::upsert_batch(pool, &new_symbols)
                .await
                .context("Failed to upsert KRX symbols")?;

        println!("   DB 저장: {}개 종목 업데이트", upserted);
        return Ok(upserted);
    }

    Ok(symbols.len())
}

/// 미국 시장 종목 수집.
async fn fetch_us_symbols(pool: Option<&PgPool>, config: &FetchSymbolsConfig) -> Result<usize> {
    println!("📊 미국 시장 수집 중 (Yahoo Finance)...");

    use trader_data::provider::{SymbolInfoProvider, YahooSymbolProvider};

    // 상위 500개만 수집 (전체 수집은 시간이 오래 걸림)
    let provider = YahooSymbolProvider::with_max_symbols(500);
    let symbols = provider
        .fetch_all()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch Yahoo symbols: {}", e))?;

    println!("   Yahoo Finance에서 {}개 종목 조회 완료", symbols.len());

    // CSV 저장 (선택적)
    if config.save_csv {
        save_to_csv(&symbols, Path::new(&config.csv_dir).join("us_symbols.csv"))?;
        println!("   CSV 저장: {}/us_symbols.csv", config.csv_dir);
    }

    // DB 저장
    if let Some(pool) = pool {
        let new_symbols: Vec<trader_api::repository::NewSymbolInfo> = symbols
            .into_iter()
            .map(|s| trader_api::repository::NewSymbolInfo {
                ticker: s.ticker,
                name: s.name,
                name_en: s.name_en,
                market: s.market,
                exchange: s.exchange,
                sector: s.sector,
                yahoo_symbol: s.yahoo_symbol,
            })
            .collect();

        let upserted =
            trader_api::repository::SymbolInfoRepository::upsert_batch(pool, &new_symbols)
                .await
                .context("Failed to upsert US symbols")?;

        println!("   DB 저장: {}개 종목 업데이트", upserted);
        return Ok(upserted);
    }

    Ok(symbols.len())
}

/// 암호화폐 시장 종목 수집.
async fn fetch_crypto_symbols(pool: Option<&PgPool>, config: &FetchSymbolsConfig) -> Result<usize> {
    println!("📊 암호화폐 시장 수집 중 (Binance)...");

    // Binance API를 통해 USDT 페어 조회
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.binance.com/api/v3/exchangeInfo")
        .send()
        .await
        .context("Failed to fetch Binance exchange info")?;

    #[derive(serde::Deserialize)]
    struct ExchangeInfo {
        symbols: Vec<BinanceSymbol>,
    }

    #[derive(serde::Deserialize)]
    struct BinanceSymbol {
        #[allow(dead_code)]
        symbol: String,
        #[serde(rename = "baseAsset")]
        base_asset: String,
        #[serde(rename = "quoteAsset")]
        quote_asset: String,
        status: String,
    }

    let exchange_info: ExchangeInfo = response
        .json()
        .await
        .context("Failed to parse Binance response")?;

    // USDT 페어만 필터링
    let usdt_pairs: Vec<_> = exchange_info
        .symbols
        .into_iter()
        .filter(|s| s.quote_asset == "USDT" && s.status == "TRADING")
        .collect();

    println!("   Binance에서 {}개 USDT 페어 조회 완료", usdt_pairs.len());

    // CSV 저장 (선택적)
    if config.save_csv {
        let csv_path = Path::new(&config.csv_dir).join("crypto_symbols.csv");
        let mut wtr = csv::Writer::from_path(&csv_path).context("Failed to create CSV writer")?;

        wtr.write_record(["ticker", "name", "market", "exchange"])
            .context("Failed to write CSV header")?;

        for pair in &usdt_pairs {
            wtr.write_record(&[
                format!("{}/USDT", pair.base_asset),
                format!("{}/USDT", pair.base_asset),
                "CRYPTO".to_string(),
                "BINANCE".to_string(),
            ])
            .context("Failed to write CSV record")?;
        }

        wtr.flush().context("Failed to flush CSV writer")?;
        println!("   CSV 저장: {}/crypto_symbols.csv", config.csv_dir);
    }

    // DB 저장
    if let Some(pool) = pool {
        let new_symbols: Vec<trader_api::repository::NewSymbolInfo> = usdt_pairs
            .into_iter()
            .map(|s| trader_api::repository::NewSymbolInfo {
                ticker: format!("{}/USDT", s.base_asset),
                name: format!("{}/USDT", s.base_asset),
                name_en: Some(s.base_asset.clone()),
                market: "CRYPTO".to_string(),
                exchange: Some("BINANCE".to_string()),
                sector: Some("Cryptocurrency".to_string()),
                yahoo_symbol: None, // Yahoo Finance는 암호화폐 미지원
            })
            .collect();

        let _count = new_symbols.len();
        let upserted =
            trader_api::repository::SymbolInfoRepository::upsert_batch(pool, &new_symbols)
                .await
                .context("Failed to upsert crypto symbols")?;

        println!("   DB 저장: {}개 종목 업데이트", upserted);
        return Ok(upserted);
    }

    Ok(usdt_pairs.len())
}

/// 종목 정보를 CSV 파일로 저장.
fn save_to_csv(
    symbols: &[trader_data::provider::SymbolMetadata],
    path: impl AsRef<Path>,
) -> Result<()> {
    let mut wtr = csv::Writer::from_path(path.as_ref()).context("Failed to create CSV writer")?;

    // 헤더 작성
    wtr.write_record([
        "ticker",
        "name",
        "name_en",
        "market",
        "exchange",
        "sector",
        "yahoo_symbol",
    ])
    .context("Failed to write CSV header")?;

    // 데이터 작성
    for symbol in symbols {
        wtr.write_record([
            &symbol.ticker,
            &symbol.name,
            symbol.name_en.as_deref().unwrap_or(""),
            &symbol.market,
            symbol.exchange.as_deref().unwrap_or(""),
            symbol.sector.as_deref().unwrap_or(""),
            symbol.yahoo_symbol.as_deref().unwrap_or(""),
        ])
        .context("Failed to write CSV record")?;
    }

    wtr.flush().context("Failed to flush CSV writer")?;

    Ok(())
}
