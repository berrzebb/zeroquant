use trader_exchange::connector::upbit::{UpbitClient, UpbitConfig};
use trader_exchange::connector::bithumb::{BithumbClient, BithumbConfig};
use trader_exchange::connector::db_investment::{DbInvestmentClient, DbInvestmentConfig};
use trader_core::domain::ExchangeProvider;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Upbit Verification
    println!("--- Verifying Upbit Connector ---");
    let upbit_config = UpbitConfig::new(
        std::env::var("UPBIT_ACCESS_KEY").unwrap_or_else(|_| "YOUR_UPBIT_ACCESS_KEY".to_string()),
        std::env::var("UPBIT_SECRET_KEY").unwrap_or_else(|_| "YOUR_UPBIT_SECRET_KEY".to_string()),
    );
    let upbit_client = UpbitClient::new(upbit_config);
    match upbit_client.fetch_account().await {
        Ok(info) => println!("Upbit Account Info: {:?}", info),
        Err(e) => eprintln!("Upbit Error: {:?}", e),
    }

    // 2. Bithumb Verification
    println!("\n--- Verifying Bithumb Connector ---");
    let bithumb_config = BithumbConfig::new(
        std::env::var("BITHUMB_ACCESS_KEY").unwrap_or_else(|_| "YOUR_BITHUMB_ACCESS_KEY".to_string()),
        std::env::var("BITHUMB_SECRET_KEY").unwrap_or_else(|_| "YOUR_BITHUMB_SECRET_KEY".to_string()),
    );
    let bithumb_client = BithumbClient::new(bithumb_config);
    match bithumb_client.fetch_account().await {
        Ok(info) => println!("Bithumb Account Info: {:?}", info),
        Err(e) => eprintln!("Bithumb Error: {:?}", e),
    }

    // 3. DB Investment Verification
    println!("\n--- Verifying DB Investment Connector ---");
    let db_config = DbInvestmentConfig::new(
        std::env::var("DB_APP_KEY").unwrap_or_else(|_| "YOUR_DB_APP_KEY".to_string()),
        std::env::var("DB_SECRET_KEY").unwrap_or_else(|_| "YOUR_DB_SECRET_KEY".to_string()),
        None, // Use default URL
    );
    let db_client = DbInvestmentClient::new(db_config);
    match db_client.fetch_account().await {
        Ok(info) => println!("DB Investment Account Info: {:?}", info),
        Err(e) => eprintln!("DB Investment Error: {:?}", e),
    }

    Ok(())
}
