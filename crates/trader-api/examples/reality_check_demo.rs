//! Reality Check 시스템 데모
//!
//! 이 예제는 Reality Check 시스템의 기본 사용법을 보여줍니다.
//!
//! # 실행 방법
//!
//! ```bash
//! cargo run --example reality_check_demo
//! ```
//!
//! # 사전 요구사항
//!
//! 1. PostgreSQL (TimescaleDB) 실행 중
//! 2. 026_reality_check_system.sql 마이그레이션 완료
//! 3. mv_latest_prices 뷰에 데이터 존재

use chrono::{Duration, Utc};
use rust_decimal_macros::dec;
use sqlx::PgPool;
use trader_api::repository::{RealityCheckRepository, SnapshotInput};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 로깅 초기화
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // 데이터베이스 연결
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://trader:trader_secret@localhost:5432/trader".to_string());

    println!("🔌 Connecting to database...");
    let pool = PgPool::connect(&database_url).await?;
    println!("✅ Connected!");

    // 날짜 설정
    let today = Utc::now().naive_utc().date();
    let yesterday = today - Duration::days(1);

    println!("\n📅 Dates:");
    println!("  Today: {}", today);
    println!("  Yesterday: {}", yesterday);

    // ==================== 1. 스냅샷 저장 ====================
    println!("\n📸 Step 1: Saving price snapshots...");

    let snapshots = vec![
        SnapshotInput {
            symbol: "005930".to_string(), // 삼성전자
            close_price: dec!(70000),
            volume: Some(10000000),
            recommend_source: "demo_screening".to_string(),
            recommend_rank: Some(1),
            recommend_score: Some(dec!(95.5)),
            expected_return: Some(dec!(5.0)),
            expected_holding_days: Some(3),
            market: Some("KR".to_string()),
            sector: Some("IT".to_string()),
        },
        SnapshotInput {
            symbol: "000660".to_string(), // SK하이닉스
            close_price: dec!(130000),
            volume: Some(5000000),
            recommend_source: "demo_screening".to_string(),
            recommend_rank: Some(2),
            recommend_score: Some(dec!(92.3)),
            expected_return: Some(dec!(4.5)),
            expected_holding_days: Some(3),
            market: Some("KR".to_string()),
            sector: Some("반도체".to_string()),
        },
        SnapshotInput {
            symbol: "035420".to_string(), // NAVER
            close_price: dec!(210000),
            volume: Some(3000000),
            recommend_source: "demo_screening".to_string(),
            recommend_rank: Some(3),
            recommend_score: Some(dec!(88.7)),
            expected_return: Some(dec!(3.5)),
            expected_holding_days: Some(3),
            market: Some("KR".to_string()),
            sector: Some("IT".to_string()),
        },
    ];

    match RealityCheckRepository::save_snapshots_batch(&pool, yesterday, &snapshots).await {
        Ok(count) => println!("✅ Saved {} snapshots for {}", count, yesterday),
        Err(e) => println!("❌ Failed to save snapshots: {}", e),
    }

    // ==================== 2. Reality Check 계산 ====================
    println!("\n🧮 Step 2: Calculating reality check...");

    match RealityCheckRepository::calculate_reality_check(&pool, yesterday, today).await {
        Ok(results) => {
            println!("✅ Calculated {} reality checks", results.len());
            for result in &results {
                let is_profitable = result.is_profitable.unwrap_or(false);
                let emoji = if is_profitable { "📈" } else { "📉" };
                println!(
                    "  {} {}: {:.4}% {}",
                    emoji,
                    result.symbol.as_deref().unwrap_or("N/A"),
                    result.actual_return.unwrap_or_default(),
                    if is_profitable { "WIN" } else { "LOSS" }
                );
            }
        }
        Err(e) => println!("❌ Failed to calculate: {}", e),
    }

    // ==================== 3. 통계 조회 ====================
    println!("\n📊 Step 3: Fetching statistics...");

    // 일별 통계
    match RealityCheckRepository::get_daily_stats(&pool, 7).await {
        Ok(stats) => {
            println!("\n  📅 Daily Stats (Last 7 days):");
            for stat in stats {
                println!(
                    "    {}: {:.2}% win rate, {:.4}% avg return (Total: {})",
                    stat.check_date
                        .map(|d| d.to_string())
                        .unwrap_or_else(|| "N/A".to_string()),
                    stat.win_rate.unwrap_or_default(),
                    stat.avg_return.unwrap_or_default(),
                    stat.total_count.unwrap_or(0)
                );
            }
        }
        Err(e) => println!("  ❌ Failed to fetch daily stats: {}", e),
    }

    // 소스별 통계
    match RealityCheckRepository::get_source_stats(&pool).await {
        Ok(stats) => {
            println!("\n  🎯 Source Stats:");
            for stat in stats {
                println!(
                    "    {}: {:.2}% win rate, {:.4}% avg return (Total: {})",
                    stat.recommend_source.as_deref().unwrap_or("N/A"),
                    stat.win_rate.unwrap_or_default(),
                    stat.avg_return.unwrap_or_default(),
                    stat.total_count.unwrap_or(0)
                );
            }
        }
        Err(e) => println!("  ❌ Failed to fetch source stats: {}", e),
    }

    // 랭크별 통계
    match RealityCheckRepository::get_rank_stats(&pool).await {
        Ok(stats) => {
            println!("\n  🏆 Rank Stats (Top 10):");
            for stat in stats {
                println!(
                    "    Rank {}: {:.2}% win rate, {:.4}% avg return",
                    stat.recommend_rank.unwrap_or(0),
                    stat.win_rate.unwrap_or_default(),
                    stat.avg_return.unwrap_or_default()
                );
            }
        }
        Err(e) => println!("  ❌ Failed to fetch rank stats: {}", e),
    }

    // ==================== 4. 최근 성과 조회 ====================
    println!("\n📈 Step 4: Fetching recent performance...");

    match RealityCheckRepository::get_recent_performance(&pool, "demo_screening", 7).await {
        Ok(results) => {
            println!("  Found {} recent records:", results.len());
            for result in results.iter().take(5) {
                println!(
                    "    {} -> {}: {} ({}% return)",
                    result.recommend_date, result.check_date, result.symbol, result.actual_return
                );
            }
        }
        Err(e) => println!("  ❌ Failed: {}", e),
    }

    // ==================== 5. 전체 요약 통계 ====================
    println!("\n📋 Step 5: Overall summary...");

    match RealityCheckRepository::get_summary_stats(&pool, 30).await {
        Ok(summary) => {
            println!("  📊 Last 30 Days Summary:");
            println!("    Total Trades: {}", summary.total_count.unwrap_or(0));
            println!("    Win Trades: {}", summary.win_count.unwrap_or(0));
            println!("    Win Rate: {:.2}%", summary.win_rate.unwrap_or_default());
            println!(
                "    Avg Return: {:.4}%",
                summary.avg_return.unwrap_or_default()
            );
            if let Some(avg_win) = summary.avg_win_return {
                println!("    Avg Win: {:.4}%", avg_win);
            }
            if let Some(avg_loss) = summary.avg_loss_return {
                println!("    Avg Loss: {:.4}%", avg_loss);
            }
            println!(
                "    Max Return: {:.4}%",
                summary.max_return.unwrap_or_default()
            );
            println!(
                "    Min Return: {:.4}%",
                summary.min_return.unwrap_or_default()
            );
            if let Some(stddev) = summary.return_stddev {
                println!("    Std Dev: {:.4}%", stddev);
            }
        }
        Err(e) => println!("  ❌ Failed: {}", e),
    }

    println!("\n✅ Demo completed!");

    Ok(())
}
