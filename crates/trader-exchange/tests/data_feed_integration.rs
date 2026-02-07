//! Integration tests for DataFeed with downloaded CSV data.

use rust_decimal_macros::dec;
use std::path::Path;
use trader_core::Timeframe;
use trader_exchange::simulated::{DataFeed, DataFeedConfig};

/// Test loading downloaded CSV data into DataFeed.
#[test]
fn test_load_downloaded_csv() {
    // Check if the test file exists (downloaded from Binance)
    let csv_path = Path::new("../../data/test_btcusdt_1h.csv");

    if !csv_path.exists() {
        eprintln!("Skipping test: CSV file not found at {:?}", csv_path);
        eprintln!("Run: cargo run -p trader-cli -- download -s BTCUSDT -i 1h -f 2024-01-01 -t 2024-01-07 -o data/test_btcusdt_1h.csv");
        return;
    }

    let config = DataFeedConfig::default();
    let mut feed = DataFeed::new(config);
    let ticker = "BTC/USDT";

    // Load CSV
    let count = feed
        .load_from_csv(ticker.to_string(), Timeframe::H1, csv_path)
        .expect("Failed to load CSV");

    // Verify data was loaded
    assert!(count > 0, "Should load at least some candles");
    println!("Loaded {} candles from CSV", count);

    // Verify data count
    assert_eq!(feed.data_count(ticker, Timeframe::H1), count);

    // Verify time range
    let (start, end) = feed.time_range().expect("Should have time range");
    println!("Data range: {} to {}", start, end);
    assert!(start < end, "Start should be before end");

    // Test playback
    let kline = feed
        .next_kline(ticker, Timeframe::H1)
        .expect("Should get first kline");

    println!(
        "First kline: open={}, high={}, low={}, close={}, volume={}",
        kline.open, kline.high, kline.low, kline.close, kline.volume
    );

    // Verify BTC price is reasonable for 2024
    assert!(kline.close > dec!(30000), "BTC price should be > 30k");
    assert!(kline.close < dec!(100000), "BTC price should be < 100k");

    // Test getting historical klines
    for _ in 0..10 {
        feed.next_kline(ticker, Timeframe::H1);
    }

    let historical = feed.get_historical_klines(ticker, Timeframe::H1, 5);
    assert_eq!(historical.len(), 5, "Should get 5 historical klines");

    // Historical should be in chronological order
    for window in historical.windows(2) {
        assert!(
            window[0].open_time < window[1].open_time,
            "Historical klines should be in chronological order"
        );
    }
}

/// Test playback exhaustion.
#[test]
fn test_playback_exhaustion() {
    let csv_path = Path::new("../../data/test_btcusdt_1h.csv");

    if !csv_path.exists() {
        return;
    }

    let config = DataFeedConfig {
        loop_data: false,
        ..Default::default()
    };
    let mut feed = DataFeed::new(config);
    let ticker = "BTC/USDT";

    let count = feed
        .load_from_csv(ticker.to_string(), Timeframe::H1, csv_path)
        .expect("Failed to load CSV");

    // Play through all data
    let mut played = 0;
    while feed.next_kline(ticker, Timeframe::H1).is_some() {
        played += 1;
    }

    assert_eq!(played, count, "Should play exactly {} candles", count);
    assert!(feed.is_exhausted(), "Feed should be exhausted");
}

/// Test large dataset (1-minute data).
#[test]
fn test_load_large_dataset() {
    let csv_path = Path::new("../../data/btcusdt_1m_jan2024.csv");

    if !csv_path.exists() {
        eprintln!("Skipping large dataset test: file not found");
        return;
    }

    let config = DataFeedConfig::default();
    let mut feed = DataFeed::new(config);
    let ticker = "BTC/USDT";

    let start = std::time::Instant::now();
    let count = feed
        .load_from_csv(ticker.to_string(), Timeframe::M1, csv_path)
        .expect("Failed to load CSV");
    let elapsed = start.elapsed();

    println!("Loaded {} candles in {:?}", count, elapsed);

    // Should load quickly (< 1 second for ~45k candles)
    assert!(elapsed.as_secs() < 5, "Loading should be fast");

    // Verify expected count (~44k candles for January)
    assert!(count > 40000, "Should have at least 40k candles");
    assert!(count < 50000, "Should have at most 50k candles");

    // Test playback performance
    let start = std::time::Instant::now();
    let mut total = 0;
    while feed.next_kline(ticker, Timeframe::M1).is_some() {
        total += 1;
    }
    let elapsed = start.elapsed();

    println!("Played {} candles in {:?}", total, elapsed);
    assert!(elapsed.as_secs() < 5, "Playback should be fast");
}
