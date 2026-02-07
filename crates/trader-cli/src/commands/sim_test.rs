//! 시뮬레이션 엔진 테스트 도구.
//!
//! BacktestEngine과 별개로 SimulationEngine을 직접 테스트합니다.
//! 주로 그리드 전략 등 분할 매수/매도가 필요한 전략 검증에 사용합니다.
//!
//! # 사용 예시
//!
//! ```bash
//! # 그리드 전략 테스트
//! trader sim-test --strategy grid --symbol 005930 --market KR
//!
//! # 상세 디버그 모드
//! trader sim-test --strategy grid --symbol 005930 --debug
//! ```

use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal_macros::dec;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use trader_core::{Kline, MarketType, StrategyContext, Timeframe, SignalType};
use trader_data::cache::CachedHistoricalDataProvider;
use trader_data::storage::ohlcv::OhlcvCache;
use trader_data::{Database, DatabaseConfig};
use trader_strategy::StrategyRegistry;

use crate::commands::download::Market;

/// 시뮬레이션 테스트 설정
#[derive(Debug, Clone)]
pub struct SimTestConfig {
    /// 전략 ID
    pub strategy_id: String,
    /// 종목 코드
    pub symbol: String,
    /// 시장 (KR/US)
    pub market: Market,
    /// JSON 설정 (옵션)
    pub json_config: Option<String>,
    /// 시작일
    pub start_date: Option<NaiveDate>,
    /// 종료일
    pub end_date: Option<NaiveDate>,
    /// 초기 자본금
    pub initial_balance: Decimal,
    /// 디버그 모드
    pub debug: bool,
    /// 데이터베이스 URL
    pub db_url: Option<String>,
}

impl Default for SimTestConfig {
    fn default() -> Self {
        Self {
            strategy_id: String::new(),
            symbol: String::new(),
            market: Market::KR,
            json_config: None,
            start_date: None,
            end_date: None,
            initial_balance: Decimal::from(10_000_000),
            debug: false,
            db_url: None,
        }
    }
}

/// 시뮬레이션 테스트 결과
#[derive(Debug, Clone)]
pub struct SimTestResult {
    pub success: bool,
    pub strategy_id: String,
    pub symbol: String,
    pub data_points: usize,
    pub signals_generated: usize,
    pub trades_executed: usize,
    pub final_equity: Decimal,
    pub total_return_pct: Decimal,
    pub signal_details: Vec<SignalDetail>,
    pub trade_details: Vec<TradeDetail>,
    pub diagnostics: Vec<String>,
}

/// 신호 상세 정보
#[derive(Debug, Clone)]
pub struct SignalDetail {
    pub timestamp: DateTime<Utc>,
    pub signal_type: String,
    pub side: String,
    pub price: Decimal,
    pub grid_level: Option<String>,
    pub metadata: String,
}

/// 거래 상세 정보
#[derive(Debug, Clone)]
pub struct TradeDetail {
    pub timestamp: DateTime<Utc>,
    pub side: String,
    pub quantity: Decimal,
    pub price: Decimal,
    pub pnl: Option<Decimal>,
}

/// 시뮬레이션 테스트 실행
pub async fn run_sim_test(config: SimTestConfig) -> Result<SimTestResult> {
    println!("\n🔬 시뮬레이션 엔진 테스트 시작");
    println!("═══════════════════════════════════════════════════════════════");
    println!("  전략 ID: {}", config.strategy_id);
    println!("  종목: {} ({})", config.symbol, match config.market {
        Market::KR => "한국",
        Market::US => "미국",
    });
    println!("  초기 자본: {}원", config.initial_balance);
    println!("═══════════════════════════════════════════════════════════════\n");

    let mut diagnostics = Vec::new();

    // 1. 전략 검증
    println!("📋 [1/5] 전략 검증...");
    let available_strategies = StrategyRegistry::list_ids();
    if !available_strategies.contains(&config.strategy_id.as_str()) {
        diagnostics.push(format!("❌ 전략 '{}' 를 찾을 수 없습니다.", config.strategy_id));
        return Ok(SimTestResult {
            success: false,
            strategy_id: config.strategy_id,
            symbol: config.symbol,
            data_points: 0,
            signals_generated: 0,
            trades_executed: 0,
            final_equity: Decimal::ZERO,
            total_return_pct: Decimal::ZERO,
            signal_details: vec![],
            trade_details: vec![],
            diagnostics,
        });
    }
    println!("  ✅ 전략 '{}' 확인됨", config.strategy_id);

    // 2. 데이터베이스 연결
    println!("\n📋 [2/5] 데이터베이스 연결...");
    let db_url = config.db_url.clone().unwrap_or_else(|| {
        std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://trader:trader_secret@localhost:5432/trader".to_string())
    });

    let db_config = DatabaseConfig {
        url: db_url,
        ..Default::default()
    };

    let db = Database::connect(&db_config).await?;
    let pool = db.pool();
    println!("  ✅ 데이터베이스 연결 성공");

    // 3. 캔들 데이터 로드
    println!("\n📋 [3/5] 캔들 데이터 로드...");
    let ohlcv_cache = OhlcvCache::new(pool.clone());

    let now = Utc::now();
    let start = config.start_date
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
        .unwrap_or_else(|| now - chrono::Duration::days(365));
    let end = config.end_date
        .map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc())
        .unwrap_or(now);

    let klines = ohlcv_cache
        .get_cached_klines_range(&config.symbol, Timeframe::D1, start, end)
        .await
        .map_err(|e| anyhow!("캔들 데이터 로드 실패: {}", e))?;

    if klines.is_empty() {
        diagnostics.push("❌ 캔들 데이터가 없습니다.".to_string());
        return Ok(SimTestResult {
            success: false,
            strategy_id: config.strategy_id,
            symbol: config.symbol,
            data_points: 0,
            signals_generated: 0,
            trades_executed: 0,
            final_equity: Decimal::ZERO,
            total_return_pct: Decimal::ZERO,
            signal_details: vec![],
            trade_details: vec![],
            diagnostics,
        });
    }

    println!("  ✅ {} 캔들 로드 완료", klines.len());
    println!("    기간: {} ~ {}",
        klines.first().map(|k| k.open_time.format("%Y-%m-%d").to_string()).unwrap_or_default(),
        klines.last().map(|k| k.open_time.format("%Y-%m-%d").to_string()).unwrap_or_default()
    );

    // 4. 전략 초기화
    println!("\n📋 [4/5] 전략 초기화...");

    // 전략 설정 준비
    let mut json_config = if let Some(ref json_str) = config.json_config {
        serde_json::from_str(json_str)?
    } else {
        serde_json::json!({})
    };

    if let Some(obj) = json_config.as_object_mut() {
        if !obj.contains_key("ticker") {
            obj.insert("ticker".to_string(), serde_json::json!(&config.symbol));
        }
        if !obj.contains_key("amount") {
            obj.insert("amount".to_string(), serde_json::json!(config.initial_balance.to_string()));
        }
    }

    println!("  설정: {}", serde_json::to_string_pretty(&json_config)?);

    // StrategyContext 생성 (백테스트와 동일하게 Armed 상태로 설정)
    let context = Arc::new(RwLock::new(StrategyContext::default()));
    {
        let mut ctx_write = context.write().await;
        ctx_write.route_states.insert(config.symbol.clone(), trader_core::RouteState::Armed);
        ctx_write.update_klines(&config.symbol, Timeframe::D1, klines.clone());
    }

    // 전략 생성 및 초기화
    let mut strategy = StrategyRegistry::create_instance(&config.strategy_id)
        .map_err(|e| anyhow!("전략 생성 실패: {}", e))?;

    strategy.set_context(Arc::clone(&context));

    strategy
        .initialize(json_config.clone())
        .await
        .map_err(|e| anyhow!("전략 초기화 실패: {}", e))?;

    println!("  ✅ 전략 초기화 성공");

    // 5. 시뮬레이션 실행 (수동 캔들 순회)
    println!("\n📋 [5/5] 시뮬레이션 실행 (캔들 순회)...");

    let mut signal_details = Vec::new();
    let mut trade_details = Vec::new();
    let mut balance = config.initial_balance;
    let mut position_qty = Decimal::ZERO;
    let mut position_entry_price = Decimal::ZERO;
    let commission_rate = dec!(0.001); // 0.1%

    for (idx, kline) in klines.iter().enumerate() {
        // 전략에 캔들 전달
        let market_data = trader_core::MarketData::from_kline("simulation", kline.clone());

        match strategy.on_market_data(&market_data).await {
            Ok(signals) => {
                for signal in &signals {
                    let grid_level = signal.metadata.get("grid_level")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    signal_details.push(SignalDetail {
                        timestamp: kline.close_time,
                        signal_type: format!("{:?}", signal.signal_type),
                        side: format!("{:?}", signal.side),
                        price: signal.suggested_price.unwrap_or(kline.close),
                        grid_level,
                        metadata: serde_json::to_string(&signal.metadata).unwrap_or_default(),
                    });

                    // 간단한 거래 시뮬레이션
                    let price = signal.suggested_price.unwrap_or(kline.close);

                    match signal.signal_type {
                        SignalType::Entry | SignalType::AddToPosition => {
                            if signal.side == trader_core::Side::Buy {
                                // 매수
                                let trade_amount = balance * dec!(0.1); // 10%씩 분할 매수
                                let qty = trade_amount / price;
                                let commission = trade_amount * commission_rate;

                                if trade_amount + commission <= balance {
                                    balance -= trade_amount + commission;

                                    // 평균 단가 업데이트
                                    if position_qty > Decimal::ZERO {
                                        position_entry_price = (position_entry_price * position_qty + price * qty)
                                            / (position_qty + qty);
                                    } else {
                                        position_entry_price = price;
                                    }
                                    position_qty += qty;

                                    trade_details.push(TradeDetail {
                                        timestamp: kline.close_time,
                                        side: "Buy".to_string(),
                                        quantity: qty,
                                        price,
                                        pnl: None,
                                    });

                                    if config.debug {
                                        println!("    [{}] 매수: {} @ {} (잔고: {:.0})",
                                            kline.open_time.format("%Y-%m-%d"),
                                            qty,
                                            price,
                                            balance
                                        );
                                    }
                                }
                            }
                        }
                        SignalType::Exit | SignalType::ReducePosition => {
                            if signal.side == trader_core::Side::Sell && position_qty > Decimal::ZERO {
                                // 매도 (Grid는 레벨별로 분할 청산)
                                let sell_qty = position_qty / Decimal::from(5); // 20%씩 분할 청산
                                let sell_value = sell_qty * price;
                                let commission = sell_value * commission_rate;
                                let pnl = (price - position_entry_price) * sell_qty - commission;

                                balance += sell_value - commission;
                                position_qty -= sell_qty;

                                trade_details.push(TradeDetail {
                                    timestamp: kline.close_time,
                                    side: "Sell".to_string(),
                                    quantity: sell_qty,
                                    price,
                                    pnl: Some(pnl),
                                });

                                if config.debug {
                                    println!("    [{}] 매도: {} @ {} (PnL: {:+.0}, 잔고: {:.0})",
                                        kline.open_time.format("%Y-%m-%d"),
                                        sell_qty,
                                        price,
                                        pnl,
                                        balance
                                    );
                                }
                            } else if position_qty == Decimal::ZERO {
                                diagnostics.push(format!(
                                    "⚠️ [{}] 포지션 없이 매도 신호 발생!",
                                    kline.open_time.format("%Y-%m-%d")
                                ));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                warn!("전략 실행 오류 (캔들 {}): {}", idx, e);
            }
        }
    }

    // 최종 자산 계산
    let last_price = klines.last().map(|k| k.close).unwrap_or(Decimal::ZERO);
    let final_equity = balance + position_qty * last_price;
    let total_return_pct = if config.initial_balance > Decimal::ZERO {
        (final_equity - config.initial_balance) / config.initial_balance * dec!(100)
    } else {
        Decimal::ZERO
    };

    // 결과 출력
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("📊 시뮬레이션 결과");
    println!("═══════════════════════════════════════════════════════════════");
    println!("  총 신호 수: {}", signal_details.len());
    println!("  총 거래 수: {}", trade_details.len());
    println!("  최종 자산: {:.0}원", final_equity);
    println!("  수익률: {:.2}%", total_return_pct);

    // 신호 분포 분석
    let buy_signals = signal_details.iter().filter(|s| s.side == "Buy").count();
    let sell_signals = signal_details.iter().filter(|s| s.side == "Sell").count();
    println!("\n  📈 신호 분포:");
    println!("    - 매수 신호: {} 개", buy_signals);
    println!("    - 매도 신호: {} 개", sell_signals);

    // 첫/마지막 신호 시간
    if let Some(first) = signal_details.first() {
        println!("\n  📍 첫 신호: {} ({} {})",
            first.timestamp.format("%Y-%m-%d"),
            first.side,
            first.signal_type
        );
    }
    if let Some(last) = signal_details.last() {
        println!("  📍 마지막 신호: {} ({} {})",
            last.timestamp.format("%Y-%m-%d"),
            last.side,
            last.signal_type
        );
    }

    // Grid 레벨 분석 (Grid 전략인 경우)
    if config.strategy_id.contains("grid") {
        println!("\n  🔢 그리드 레벨별 신호:");
        let mut level_counts: std::collections::HashMap<String, (usize, usize)> = std::collections::HashMap::new();
        for s in &signal_details {
            if let Some(ref level) = s.grid_level {
                let entry = level_counts.entry(level.clone()).or_insert((0, 0));
                if s.side == "Buy" {
                    entry.0 += 1;
                } else {
                    entry.1 += 1;
                }
            }
        }
        for (level, (buys, sells)) in &level_counts {
            println!("    - 레벨 {}: 매수 {}, 매도 {}", level, buys, sells);
        }
    }

    // 디버그 모드에서 상세 신호 출력
    if config.debug {
        println!("\n  📝 상세 신호 목록:");
        println!("  ───────────────────────────────────────────────────────────────");
        for (i, s) in signal_details.iter().enumerate().take(50) {
            println!("  [{}] {} | {} {} @ {} | level: {:?}",
                i + 1,
                s.timestamp.format("%Y-%m-%d"),
                s.side,
                s.signal_type,
                s.price,
                s.grid_level
            );
        }
        if signal_details.len() > 50 {
            println!("  ... 외 {} 개 신호", signal_details.len() - 50);
        }
    }

    // 진단 정보 출력
    if !diagnostics.is_empty() {
        println!("\n🔍 진단 정보:");
        for diag in &diagnostics {
            println!("  {}", diag);
        }
    }

    println!("\n═══════════════════════════════════════════════════════════════\n");

    Ok(SimTestResult {
        success: trade_details.len() > 0,
        strategy_id: config.strategy_id,
        symbol: config.symbol,
        data_points: klines.len(),
        signals_generated: signal_details.len(),
        trades_executed: trade_details.len(),
        final_equity,
        total_return_pct,
        signal_details,
        trade_details,
        diagnostics,
    })
}

/// Grid 전략 전용 상세 분석
pub async fn analyze_grid_strategy(config: SimTestConfig) -> Result<()> {
    println!("\n🔬 Grid 전략 상세 분석");
    println!("═══════════════════════════════════════════════════════════════\n");

    let result = run_sim_test(SimTestConfig {
        debug: true,
        ..config
    }).await?;

    // Grid 전략 핵심 검증
    println!("\n📋 Grid 전략 검증 체크리스트:");
    println!("───────────────────────────────────────────────────────────────");

    // 1. 첫 신호가 매수인지 확인
    let first_is_buy = result.signal_details.first()
        .map(|s| s.side == "Buy")
        .unwrap_or(false);
    if first_is_buy {
        println!("  ✅ 첫 신호가 매수 (정상)");
    } else {
        println!("  ❌ 첫 신호가 매도 (비정상 - 포지션 없이 매도)");
    }

    // 2. 매수 → 매도 순서 확인
    let mut buy_count = 0;
    let mut sell_before_buy = 0;
    for s in &result.signal_details {
        if s.side == "Buy" {
            buy_count += 1;
        } else if s.side == "Sell" {
            if buy_count == 0 {
                sell_before_buy += 1;
            }
        }
    }
    if sell_before_buy == 0 {
        println!("  ✅ 모든 매도가 매수 이후에 발생 (정상)");
    } else {
        println!("  ❌ {} 개의 매도가 매수 전에 발생 (비정상)", sell_before_buy);
    }

    // 3. 레벨별 매수/매도 균형 확인
    let buy_signals = result.signal_details.iter().filter(|s| s.side == "Buy").count();
    let sell_signals = result.signal_details.iter().filter(|s| s.side == "Sell").count();
    let ratio = if buy_signals > 0 {
        sell_signals as f64 / buy_signals as f64
    } else {
        0.0
    };

    if ratio >= 0.5 && ratio <= 2.0 {
        println!("  ✅ 매수/매도 비율 정상 ({:.2})", ratio);
    } else {
        println!("  ⚠️ 매수/매도 비율 불균형 ({:.2}) - 검토 필요", ratio);
    }

    // 4. 그리드 레벨 활용도
    let unique_levels: std::collections::HashSet<_> = result.signal_details
        .iter()
        .filter_map(|s| s.grid_level.as_ref())
        .collect();
    println!("  📊 활성화된 그리드 레벨: {} 개", unique_levels.len());

    println!("\n═══════════════════════════════════════════════════════════════\n");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sim_test_config_default() {
        let config = SimTestConfig::default();
        assert_eq!(config.initial_balance, Decimal::from(10_000_000));
        assert!(matches!(config.market, Market::KR));
    }
}
