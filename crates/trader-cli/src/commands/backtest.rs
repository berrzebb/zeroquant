//! 백테스트 명령어.
//!
//! TimescaleDB에 저장된 과거 데이터로 전략을 백테스트합니다.
//!
//! # 사용 예시
//!
//! ```bash
//! # 삼성전자 데이터로 RSI 전략 백테스트
//! trader backtest -c config/backtest/rsi.toml -s 005930 -m KR
//!
//! # SPY 데이터로 Compound Momentum 전략 백테스트
//! trader backtest -c config/backtest/compound_momentum.toml -s SPY -m US
//!
//! # 특정 기간만 백테스트
//! trader backtest -c config/backtest/haa.toml -s SPY -m US -f 2024-01-01 -t 2024-12-31
//!
//! # 사용 가능한 전략 목록
//! trader backtest --list-strategies
//! ```

use anyhow::{anyhow, Result};
use chrono::{NaiveDate, Utc};
use rust_decimal::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use tracing::{debug, info, warn};

use crate::commands::chart_gen::RegressionChartGenerator;

use trader_analytics::backtest::{BacktestConfig, BacktestEngine, BacktestReport};
use trader_core::{Kline, StrategyContext, Timeframe};
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_data::{Database, DatabaseConfig, OhlcvCache};
use trader_strategy::strategies::{
    AssetAllocationStrategy, CompoundMomentumStrategy, DayTradingStrategy, DcaStrategy,
    MeanReversionStrategy, RotationStrategy,
};
use trader_strategy::Strategy;

use crate::commands::download::Market;

/// 백테스트 CLI 설정
#[derive(Debug, Clone)]
pub struct BacktestCliConfig {
    /// 전략 설정 파일 경로
    pub config_path: String,
    /// 시장 (KR/US)
    pub market: Market,
    /// 종목 코드
    pub symbol: String,
    /// 시작일 (옵션)
    pub start_date: Option<NaiveDate>,
    /// 종료일 (옵션)
    pub end_date: Option<NaiveDate>,
    /// 초기 자본금
    pub initial_capital: Decimal,
    /// 수수료율
    pub commission_rate: Decimal,
    /// 슬리피지율
    pub slippage_rate: Decimal,
    /// 데이터베이스 URL
    pub db_url: Option<String>,
    /// 결과 저장 경로 (옵션)
    pub output_path: Option<String>,
    /// 차트 생성 여부
    pub generate_chart: bool,
    /// Signal 분석 리포트 상세 출력
    pub verbose_signals: bool,
}

impl Default for BacktestCliConfig {
    fn default() -> Self {
        Self {
            config_path: String::new(),
            market: Market::KR,
            symbol: String::new(),
            start_date: None,
            end_date: None,
            initial_capital: Decimal::from(10_000_000), // 1천만원
            commission_rate: Decimal::from_str("0.00015").unwrap(), // 0.015% (한국 증권사 평균)
            slippage_rate: Decimal::from_str("0.0005").unwrap(), // 0.05%
            db_url: None,
            output_path: None,
            generate_chart: true,      // 기본: 차트 생성
            verbose_signals: true,     // 기본: 상세 신호 분석 출력
        }
    }
}

/// 전략 설정 파일 형식
#[derive(Debug, Deserialize)]
pub struct StrategyConfigFile {
    /// 전략 이름
    pub name: String,
    /// 전략 타입
    pub strategy_type: String,
    /// 전략 매개변수
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// 지원하는 전략 타입
#[derive(Debug, Clone, Copy)]
pub enum StrategyType {
    Grid,
    Rsi,
    Bollinger,
    Volatility,
    MagicSplit,
    InfinityBot,
    CompoundMomentum,
    Haa,
    Xaa,
    StockRotation,
}

impl StrategyType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "grid" | "gridtrading" => Some(Self::Grid),
            "rsi" | "rsimeanreversion" => Some(Self::Rsi),
            "bollinger" | "bollingerbands" => Some(Self::Bollinger),
            "volatility" | "volatilitybreakout" => Some(Self::Volatility),
            "magic_split" | "magicsplit" => Some(Self::MagicSplit),
            "infinity_bot" | "infinitybot" => Some(Self::InfinityBot),
            "compound_momentum" | "compoundmomentum" => Some(Self::CompoundMomentum),
            "haa" => Some(Self::Haa),
            "xaa" => Some(Self::Xaa),
            "stock_rotation" | "stockrotation" => Some(Self::StockRotation),
            _ => None,
        }
    }
}

/// 백테스트 실행
pub async fn run_backtest(config: BacktestCliConfig) -> Result<BacktestReport> {
    info!(
        "Running backtest for {} {} with config: {}",
        match config.market {
            Market::KR => "KR",
            Market::US => "US",
        },
        config.symbol,
        config.config_path
    );

    // 1. 전략 설정 파일 로드
    let strategy_config = load_strategy_config(&config.config_path)?;
    info!("Loaded strategy config: {}", strategy_config.name);

    // 2. 데이터베이스 연결
    let db_url = config.db_url.clone().unwrap_or_else(|| {
        std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://trader:trader@localhost:5432/trader".to_string())
    });

    let db_config = DatabaseConfig {
        url: db_url,
        ..Default::default()
    };

    info!("Connecting to database...");
    let db = Database::connect(&db_config).await?;

    // 3. OhlcvCache 생성 및 심볼 이름 준비
    let ohlcv_cache = OhlcvCache::new(db.pool().clone());

    // DB에 저장된 심볼 형식에 맞춰 시도 (Yahoo 형식 → 원본 순서)
    // DB에는 "005930" 또는 "005930.KS" 형식으로 저장될 수 있음
    let symbol_candidates = match config.market {
        Market::KR => vec![
            config.symbol.clone(),                      // 먼저 원본 (005930)
            format!("{}.KS", config.symbol),           // 코스피 형식
            format!("{}.KQ", config.symbol),           // 코스닥 형식
        ],
        Market::US => vec![config.symbol.clone()],
    };

    let mut klines = Vec::new();
    let mut used_symbol = String::new();

    for symbol in &symbol_candidates {
        info!("Trying symbol: {}", symbol);
        let loaded = load_klines_from_db(&ohlcv_cache, symbol, config.start_date, config.end_date).await?;
        if !loaded.is_empty() {
            klines = loaded;
            used_symbol = symbol.clone();
            break;
        }
    }

    if klines.is_empty() {
        return Err(anyhow!(
            "No historical data found for {} (tried: {:?}). Run import-db first.",
            config.symbol,
            symbol_candidates
        ));
    }

    info!("Loaded {} klines for {} (symbol: {})", klines.len(), config.symbol, used_symbol);

    info!("Loaded {} klines for backtest", klines.len());

    // 5. 전략 타입 파싱
    let strategy_type =
        StrategyType::parse(&strategy_config.strategy_type).ok_or_else(|| {
            anyhow!(
                "Unknown strategy type: {}. Use --list-strategies to see available strategies.",
                strategy_config.strategy_type
            )
        })?;

    // 6. 멀티 자산 전략: 추가 심볼 데이터 로드
    let mut multi_asset_klines: HashMap<String, Vec<Kline>> = HashMap::new();
    if is_multi_asset_strategy(&strategy_type) {
        let universe = extract_universe(&strategy_type, &strategy_config.parameters);
        info!("멀티 자산 전략 감지 - 유니버스: {:?}", universe);

        for symbol in &universe {
            // 이미 로드된 주 심볼은 건너뜀
            if symbol == &used_symbol || symbol == &config.symbol {
                multi_asset_klines.insert(symbol.clone(), klines.clone());
                continue;
            }

            let loaded = load_klines_from_db(&ohlcv_cache, symbol, config.start_date, config.end_date).await?;
            if !loaded.is_empty() {
                info!("  {} 심볼: {} 캔들 로드", symbol, loaded.len());
                multi_asset_klines.insert(symbol.clone(), loaded);
            } else {
                warn!("  {} 심볼: 데이터 없음", symbol);
            }
        }

        info!("멀티 자산 데이터 로드 완료: {}/{} 심볼", multi_asset_klines.len(), universe.len());
    }

    // 7. 백테스트 엔진 설정
    // 전략별 max_positions 추출
    let max_positions = extract_max_positions(&strategy_type, &strategy_config.parameters);

    // exit_config에서 리스크 관리 파라미터 추출
    let exit_config = strategy_config.parameters.get("exit_config");

    let stop_loss_enabled = exit_config
        .and_then(|c| c.get("stop_loss_enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let stop_loss_pct = exit_config
        .and_then(|c| c.get("stop_loss_pct"))
        .and_then(|v| v.as_f64())
        .map(|v| Decimal::from_f64_retain(v / 100.0).unwrap_or(Decimal::new(5, 2)))
        .unwrap_or(Decimal::new(5, 2)); // 기본 5%

    let take_profit_enabled = exit_config
        .and_then(|c| c.get("take_profit_enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let take_profit_pct = exit_config
        .and_then(|c| c.get("take_profit_pct"))
        .and_then(|v| v.as_f64())
        .map(|v| Decimal::from_f64_retain(v / 100.0).unwrap_or(Decimal::new(10, 2)))
        .unwrap_or(Decimal::new(10, 2)); // 기본 10%

    // max_position_size_pct 추출 (기본 20%)
    let max_position_size_pct = strategy_config.parameters
        .get("max_position_size_pct")
        .and_then(|v| v.as_f64())
        .map(|v| Decimal::from_f64_retain(v / 100.0).unwrap_or(Decimal::new(2, 1)))
        .unwrap_or(Decimal::new(2, 1)); // 20%

    let backtest_config = BacktestConfig::new(config.initial_capital)
        .with_commission_rate(config.commission_rate)
        .with_slippage_rate(config.slippage_rate)
        .with_max_positions(max_positions)
        .with_max_position_size_pct(max_position_size_pct)
        .with_allow_short(false) // 주식은 기본적으로 숏 비허용
        .with_stop_loss(stop_loss_enabled, stop_loss_pct)
        .with_take_profit(take_profit_enabled, take_profit_pct);

    // 8. 전략별 백테스트 실행
    let report = if is_multi_asset_strategy(&strategy_type) {
        run_multi_asset_backtest(
            strategy_type,
            backtest_config,
            &klines,
            &multi_asset_klines,
            &strategy_config.parameters,
            config.initial_capital,
        )
        .await?
    } else {
        run_strategy_backtest(
            strategy_type,
            backtest_config,
            &klines,
            &strategy_config.parameters,
        )
        .await?
    };

    // 8. 결과 출력
    println!("\n{}", report.summary());

    // 9. Signal 분석 리포트 (Claude 검증용 상세 텍스트)
    if config.verbose_signals {
        println!("\n{}", generate_signal_analysis(&report));
    }

    // 10. 차트 생성 (사용자 확인용 이미지)
    if config.generate_chart {
        // regression_charts 디렉토리 생성
        let charts_dir = Path::new("regression_charts");
        if !charts_dir.exists() {
            std::fs::create_dir_all(charts_dir).ok();
        }

        let chart_filename = config.output_path
            .as_ref()
            .map(|p| {
                let filename = Path::new(p).file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("backtest");
                filename.replace(".json", "_chart.png").replace(".txt", "_chart.png")
            })
            .unwrap_or_else(|| format!("backtest_{}_{}_chart.png", config.symbol, chrono::Utc::now().format("%Y%m%d_%H%M%S")));

        let chart_path = charts_dir.join(&chart_filename);

        let generator = RegressionChartGenerator::new();
        match generator.generate_combined_chart(&report, &strategy_config.name, &chart_path) {
            Ok(()) => {
                println!("\n📊 차트 저장: {}", chart_path.display());
            }
            Err(e) => {
                println!("\n⚠️ 차트 생성 실패: {}", e);
            }
        }
    }

    // 11. 결과 저장 (옵션)
    if let Some(output_path) = &config.output_path {
        save_report(&report, output_path)?;
        info!("Report saved to: {}", output_path);
    }

    Ok(report)
}

/// 전략별 max_positions 추출
/// 각 전략의 파라미터에서 적절한 값을 가져옴
fn extract_max_positions(strategy_type: &StrategyType, params: &serde_json::Value) -> usize {
    match strategy_type {
        // Grid 전략: levels 또는 max_positions
        StrategyType::Grid => {
            params.get("levels")
                .or(params.get("max_positions"))
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(15) // Grid 기본값
        }
        // InfinityBot: max_rounds
        StrategyType::InfinityBot => {
            params.get("max_rounds")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(50) // InfinityBot 기본값
        }
        // MagicSplit: levels 배열 길이 또는 max_positions
        StrategyType::MagicSplit => {
            params.get("levels")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len())
                .or_else(|| {
                    params.get("max_positions")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as usize)
                })
                .unwrap_or(10) // MagicSplit 기본값
        }
        // 자산배분 전략들: 유니버스 크기 기반
        StrategyType::CompoundMomentum | StrategyType::Haa | StrategyType::Xaa => {
            15 // 자산배분 전략은 여러 자산 동시 보유
        }
        // 로테이션 전략: 동시 보유 개수
        StrategyType::StockRotation => {
            params.get("top_n")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(10)
        }
        // 기타: 기본값
        _ => 10,
    }
}

/// 멀티 자산 전략인지 확인
fn is_multi_asset_strategy(strategy_type: &StrategyType) -> bool {
    matches!(
        strategy_type,
        StrategyType::CompoundMomentum | StrategyType::Haa | StrategyType::Xaa | StrategyType::StockRotation
    )
}

/// 전략 config에서 유니버스(심볼 목록) 추출
fn extract_universe(strategy_type: &StrategyType, params: &serde_json::Value) -> Vec<String> {
    match strategy_type {
        // CompoundMomentum: 4개 고정 자산
        StrategyType::CompoundMomentum => {
            let aggressive = params.get("aggressive_asset")
                .and_then(|v| v.as_str())
                .unwrap_or("TQQQ");
            let dividend = params.get("dividend_asset")
                .and_then(|v| v.as_str())
                .unwrap_or("SCHD");
            let rate_hedge = params.get("rate_hedge_asset")
                .and_then(|v| v.as_str())
                .unwrap_or("PFIX");
            let bond_leverage = params.get("bond_leverage_asset")
                .and_then(|v| v.as_str())
                .unwrap_or("TMF");
            vec![
                aggressive.to_string(),
                dividend.to_string(),
                rate_hedge.to_string(),
                bond_leverage.to_string(),
            ]
        }
        // HAA/XAA: 고정 유니버스 (공격/방어/카나리아 자산)
        StrategyType::Haa | StrategyType::Xaa => {
            // 기본 HAA/XAA 유니버스
            vec![
                // 공격 자산
                "QQQ".to_string(), "SPY".to_string(), "EFA".to_string(), "EEM".to_string(),
                // 방어 자산
                "TLT".to_string(), "IEF".to_string(), "LQD".to_string(),
                // 카나리아 자산
                "VWO".to_string(), "BND".to_string(),
                // 현금
                params.get("cash_ticker")
                    .and_then(|v| v.as_str())
                    .unwrap_or("BIL")
                    .to_string(),
            ]
        }
        // StockRotation: universe 필드 또는 기본 유니버스
        StrategyType::StockRotation => {
            if let Some(universe) = params.get("universe").and_then(|v| v.as_array()) {
                universe.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            } else {
                // 기본 미국 대형주 유니버스
                vec![
                    "AAPL".to_string(), "MSFT".to_string(), "GOOGL".to_string(),
                    "AMZN".to_string(), "META".to_string(), "NVDA".to_string(),
                    "TSLA".to_string(), "BRK-B".to_string(), "JPM".to_string(),
                    "V".to_string(),
                ]
            }
        }
        _ => vec![],
    }
}

/// DCA 계열 전략에 variant 필드 주입
fn inject_variant(params: &serde_json::Value, variant: &str) -> serde_json::Value {
    let mut params = params.clone();
    if let Some(obj) = params.as_object_mut() {
        // variant가 없으면 주입
        if !obj.contains_key("variant") {
            obj.insert("variant".to_string(), serde_json::json!(variant));
        }
    }
    params
}

/// 전략별 백테스트 실행 (제네릭 문제 해결을 위한 매크로 대신 개별 함수)
async fn run_strategy_backtest(
    strategy_type: StrategyType,
    backtest_config: BacktestConfig,
    klines: &[Kline],
    params: &serde_json::Value,
) -> Result<BacktestReport> {
    // StrategyContext 기반 전략은 run() 사용
    let ticker = params.get("ticker")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| klines.first().map(|k| k.ticker.as_str()).unwrap_or("UNKNOWN"));

    let context = Arc::new(RwLock::new(StrategyContext::default()));

    // DCA 계열 전략에 variant 필드 주입 (설정 파일에 없으면 strategy_type 기반으로 설정)
    let params = match strategy_type {
        StrategyType::Grid => inject_variant(params, "grid"),
        StrategyType::MagicSplit => inject_variant(params, "magic_split"),
        StrategyType::InfinityBot => inject_variant(params, "infinity_bot"),
        _ => params.clone(),
    };

    match strategy_type {
        StrategyType::Grid => {
            let mut strategy = DcaStrategy::grid();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            strategy.set_context(context.clone());
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines, context, ticker, None)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
        StrategyType::Rsi => {
            let mut strategy = MeanReversionStrategy::rsi();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            // RSI 전략은 StrategyContext 필요 (StructuralFeatures에서 RSI 가져옴)
            strategy.set_context(context.clone());
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines, context, ticker, None)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
        StrategyType::Bollinger => {
            let mut strategy = MeanReversionStrategy::bollinger();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            // Bollinger 전략도 StrategyContext 필요
            strategy.set_context(context.clone());
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines, context, ticker, None)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
        StrategyType::Volatility => {
            let mut strategy = DayTradingStrategy::breakout();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            // 변동성 돌파 전략도 StrategyContext 필요
            strategy.set_context(context.clone());
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines, context, ticker, None)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
        StrategyType::MagicSplit => {
            let mut strategy = DcaStrategy::magic_split();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            strategy.set_context(context.clone());
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines, context, ticker, None)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
        StrategyType::InfinityBot => {
            let mut strategy = DcaStrategy::infinity_bot();
            strategy
                .initialize(params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize strategy: {}", e))?;
            strategy.set_context(context.clone());
            let mut engine = BacktestEngine::new(backtest_config);
            engine
                .run(&mut strategy, klines, context, ticker, None)
                .await
                .map_err(|e| anyhow!("Backtest failed: {}", e))
        }
        // CompoundMomentum, HAA, XAA, StockRotation은 is_multi_asset_strategy()로
        // run_multi_asset_backtest()에서 처리됨 (여기 도달 불가)
        _ => unreachable!("멀티 자산 전략은 run_multi_asset_backtest()에서 처리됩니다"),
    }
}

/// 멀티 자산 전략 백테스트 실행
///
/// CompoundMomentum, HAA, XAA, StockRotation 등 여러 심볼 데이터가 필요한 전략용.
/// StrategyContext에 모든 심볼의 klines를 등록한 후 run()를 호출합니다.
async fn run_multi_asset_backtest(
    strategy_type: StrategyType,
    backtest_config: BacktestConfig,
    main_klines: &[Kline],
    multi_asset_klines: &HashMap<String, Vec<Kline>>,
    params: &serde_json::Value,
    initial_capital: Decimal,
) -> Result<BacktestReport> {
    use trader_core::StrategyAccountInfo;

    // 0. params에 initial_capital 주입 (전략이 cash_balance를 설정하기 위해)
    let mut enriched_params = params.clone();
    if let Some(obj) = enriched_params.as_object_mut() {
        if !obj.contains_key("initial_capital") {
            obj.insert("initial_capital".to_string(), serde_json::json!(initial_capital.to_string()));
            debug!("params에 initial_capital 주입: {}", initial_capital);
        }
    }

    // 1. StrategyContext 생성
    let context = Arc::new(RwLock::new(StrategyContext::default()));

    // 2. 초기 자본 설정
    {
        let mut ctx = context.write().await;
        ctx.update_account(StrategyAccountInfo {
            total_balance: initial_capital,
            available_balance: initial_capital,
            ..Default::default()
        });
    }

    // 3. 모든 심볼의 klines를 StrategyContext에 등록
    {
        let mut ctx = context.write().await;
        for (symbol, klines) in multi_asset_klines {
            ctx.update_klines(symbol, Timeframe::D1, klines.clone());
            debug!("StrategyContext에 {} 심볼 등록: {} 캔들", symbol, klines.len());
        }
    }

    // 4. 주 티커 결정
    let main_ticker = multi_asset_klines
        .keys()
        .next()
        .map(|s| s.as_str())
        .unwrap_or("SPY");

    // 5. 전략별 백테스트 실행
    let mut engine = BacktestEngine::new(backtest_config);

    match strategy_type {
        StrategyType::CompoundMomentum => {
            let mut strategy = CompoundMomentumStrategy::new();
            strategy
                .initialize(enriched_params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize CompoundMomentum strategy: {}", e))?;
            strategy.set_context(context.clone());

            // 주 티커: aggressive_asset (기본 TQQQ)
            let ticker = enriched_params
                .get("aggressive_asset")
                .and_then(|v| v.as_str())
                .unwrap_or("TQQQ");

            engine
                .run(&mut strategy, main_klines, context, ticker, None)
                .await
                .map_err(|e| anyhow!("CompoundMomentum backtest failed: {}", e))
        }
        StrategyType::Haa => {
            let mut strategy = AssetAllocationStrategy::haa();
            strategy
                .initialize(enriched_params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize HAA strategy: {}", e))?;
            strategy.set_context(context.clone());

            // HAA는 SPY를 주 티커로 사용
            let ticker = "SPY";
            engine
                .run(&mut strategy, main_klines, context, ticker, None)
                .await
                .map_err(|e| anyhow!("HAA backtest failed: {}", e))
        }
        StrategyType::Xaa => {
            let mut strategy = AssetAllocationStrategy::xaa();
            strategy
                .initialize(enriched_params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize XAA strategy: {}", e))?;
            strategy.set_context(context.clone());

            // XAA도 SPY를 주 티커로 사용
            let ticker = "SPY";
            engine
                .run(&mut strategy, main_klines, context, ticker, None)
                .await
                .map_err(|e| anyhow!("XAA backtest failed: {}", e))
        }
        StrategyType::StockRotation => {
            let mut strategy = RotationStrategy::stock_rotation();
            strategy
                .initialize(enriched_params.clone())
                .await
                .map_err(|e| anyhow!("Failed to initialize StockRotation strategy: {}", e))?;
            strategy.set_context(context.clone());

            // 종목 로테이션: 첫 번째 심볼을 주 티커로
            engine
                .run(&mut strategy, main_klines, context, main_ticker, None)
                .await
                .map_err(|e| anyhow!("StockRotation backtest failed: {}", e))
        }
        // 단일 자산 전략은 여기로 오지 않음 (is_multi_asset_strategy 필터링됨)
        _ => Err(anyhow!(
            "Strategy type {:?} is not a multi-asset strategy",
            strategy_type
        )),
    }
}

/// 전략 설정 파일 로드
fn load_strategy_config(path: &str) -> Result<StrategyConfigFile> {
    let path = Path::new(path);

    if !path.exists() {
        return Err(anyhow!(
            "Strategy config file not found: {}",
            path.display()
        ));
    }

    let content = std::fs::read_to_string(path)?;

    if path.extension().is_some_and(|ext| ext == "toml") {
        Ok(toml::from_str(&content)?)
    } else if path.extension().is_some_and(|ext| ext == "json") {
        Ok(serde_json::from_str(&content)?)
    } else {
        Err(anyhow!(
            "Unsupported config format. Use .toml or .json: {}",
            path.display()
        ))
    }
}

/// 데이터베이스에서 캔들 데이터 로드 (ohlcv 테이블 사용)
async fn load_klines_from_db(
    ohlcv_cache: &OhlcvCache,
    symbol: &str,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> Result<Vec<Kline>> {
    // 시작/종료 날짜가 없으면 기본값 사용 (최근 1년)
    let now = Utc::now();
    let start = start_date
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
        .unwrap_or_else(|| now - chrono::Duration::days(365));
    let end = end_date
        .map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc())
        .unwrap_or(now);

    // OhlcvCache의 get_cached_klines_range 메서드 사용
    let klines = ohlcv_cache
        .get_cached_klines_range(symbol, Timeframe::D1, start, end)
        .await
        .map_err(|e| anyhow!("Failed to load klines: {}", e))?;

    debug!("Loaded {} klines from ohlcv table", klines.len());

    Ok(klines)
}

/// 백테스트 리포트를 파일로 저장
fn save_report(report: &BacktestReport, path: &str) -> Result<()> {
    let path = Path::new(path);

    // 디렉토리 생성
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = if path.extension().is_some_and(|ext| ext == "json") {
        serde_json::to_string_pretty(report)?
    } else {
        // 기본: 텍스트 요약
        report.summary()
    };

    std::fs::write(path, content)?;
    Ok(())
}

// ==================== Signal 분석 리포트 (Claude 검증용) ====================

/// 백테스트 결과에서 Signal 패턴을 분석하여 구조화된 텍스트 리포트 생성.
///
/// UTF-8 안전한 문자열 자르기 (한글 호환)
fn truncate_str(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// 이 함수는 Claude가 전략 논리를 검증할 수 있도록 설계되었습니다:
/// - Entry/Exit 쌍 매칭 검증
/// - 레벨별 분포 분석 (그리드/분할매수)
/// - 이상 신호 패턴 검출
/// - 시간대별 신호 분포
fn generate_signal_analysis(report: &BacktestReport) -> String {
    use trader_core::SignalType;

    let mut output = String::new();
    output.push_str("\n╔════════════════════════════════════════════════════════════════╗\n");
    output.push_str("║           🔍 SIGNAL ANALYSIS REPORT (Claude 검증용)            ║\n");
    output.push_str("╚════════════════════════════════════════════════════════════════╝\n\n");

    let markers = &report.signal_markers;

    // 1. 기본 통계
    let total_signals = markers.len();
    let entry_count = markers.iter().filter(|m| m.signal_type == SignalType::Entry).count();
    let exit_count = markers.iter().filter(|m| m.signal_type == SignalType::Exit).count();
    let add_count = markers.iter().filter(|m| m.signal_type == SignalType::AddToPosition).count();
    let reduce_count = markers.iter().filter(|m| m.signal_type == SignalType::ReducePosition).count();
    let executed_count = markers.iter().filter(|m| m.executed).count();

    output.push_str("┌─────────────────────────────────────────────────────────────────┐\n");
    output.push_str("│ 1. 기본 통계                                                     │\n");
    output.push_str("├─────────────────────────────────────────────────────────────────┤\n");
    output.push_str(&format!("│ 총 신호 수: {:>6}                                              │\n", total_signals));
    output.push_str(&format!("│ Entry:      {:>6}                                              │\n", entry_count));
    output.push_str(&format!("│ Exit:       {:>6}                                              │\n", exit_count));
    output.push_str(&format!("│ Add:        {:>6}                                              │\n", add_count));
    output.push_str(&format!("│ Reduce:     {:>6}                                              │\n", reduce_count));
    output.push_str(&format!("│ 실행됨:     {:>6} ({:.1}%)                                     │\n",
        executed_count,
        if total_signals > 0 { executed_count as f64 / total_signals as f64 * 100.0 } else { 0.0 }
    ));
    output.push_str("└─────────────────────────────────────────────────────────────────┘\n\n");

    // 2. Entry/Exit 균형 분석
    output.push_str("┌─────────────────────────────────────────────────────────────────┐\n");
    output.push_str("│ 2. Entry/Exit 균형 분석                                         │\n");
    output.push_str("├─────────────────────────────────────────────────────────────────┤\n");

    let entry_exit_diff = (entry_count as i32) - (exit_count as i32);
    let balance_status = if entry_exit_diff == 0 {
        "✅ 균형 (정상)"
    } else if entry_exit_diff.abs() <= 2 {
        "⚠️ 경미한 불균형 (마지막 포지션 미청산 가능)"
    } else {
        "❌ 심각한 불균형 (전략 로직 검토 필요)"
    };

    output.push_str(&format!("│ Entry - Exit = {:+}                                              │\n", entry_exit_diff));
    output.push_str(&format!("│ 상태: {}                                        │\n", balance_status));
    output.push_str("└─────────────────────────────────────────────────────────────────┘\n\n");

    // 3. 레벨별 분석 (Grid/Split 전략용)
    let mut level_stats: HashMap<String, (usize, usize)> = HashMap::new();
    for marker in markers {
        if let Some(level_str) = marker.reason.split("L").last() {
            if let Ok(level) = level_str.parse::<usize>() {
                let entry = level_stats.entry(format!("Level_{}", level)).or_insert((0, 0));
                match marker.signal_type {
                    SignalType::Entry | SignalType::AddToPosition => entry.0 += 1,
                    SignalType::Exit | SignalType::ReducePosition => entry.1 += 1,
                    _ => {}
                }
            }
        }
    }

    if !level_stats.is_empty() {
        output.push_str("┌─────────────────────────────────────────────────────────────────┐\n");
        output.push_str("│ 3. 레벨별 분석 (Grid/Split 전략)                                │\n");
        output.push_str("├─────────────────────────────────────────────────────────────────┤\n");
        output.push_str("│ Level        │ Entry  │ Exit   │ Balance │ Status               │\n");
        output.push_str("├──────────────┼────────┼────────┼─────────┼──────────────────────┤\n");

        let mut levels: Vec<_> = level_stats.iter().collect();
        levels.sort_by_key(|(k, _)| (*k).clone());

        for (level, (entries, exits)) in levels {
            let diff = (*entries as i32) - (*exits as i32);
            let status = if diff == 0 { "✅" } else if diff.abs() == 1 { "⚠️" } else { "❌" };
            output.push_str(&format!("│ {:12} │ {:>6} │ {:>6} │ {:>+7} │ {}                    │\n",
                level, entries, exits, diff, status
            ));
        }
        output.push_str("└─────────────────────────────────────────────────────────────────┘\n\n");
    }

    // 4. 시간순 신호 시퀀스 (처음 10개 + 마지막 10개)
    output.push_str("┌─────────────────────────────────────────────────────────────────┐\n");
    output.push_str("│ 4. 시간순 신호 시퀀스 (처음/마지막 10개)                        │\n");
    output.push_str("├─────────────────────────────────────────────────────────────────┤\n");

    let show_count = 10.min(markers.len());
    if !markers.is_empty() {
        output.push_str("│ [처음 신호들]                                                   │\n");
        for marker in markers.iter().take(show_count) {
            let type_str = match marker.signal_type {
                SignalType::Entry => "ENTRY",
                SignalType::Exit => "EXIT ",
                SignalType::AddToPosition => "ADD  ",
                SignalType::ReducePosition => "REDUC",
                SignalType::Alert => "ALERT",
                SignalType::Scale => "SCALE",
            };
            let side_str = marker.side.map(|s| format!("{:?}", s)).unwrap_or_else(|| "-".to_string());
            let exec_str = if marker.executed { "✓" } else { "○" };
            output.push_str(&format!("│ {} {} {:>5} @ {:>12} {} {}                   │\n",
                marker.timestamp.format("%Y-%m-%d"),
                type_str,
                side_str,
                marker.price.round_dp(2),
                exec_str,
                truncate_str(&marker.reason, 15)
            ));
        }

        if markers.len() > show_count * 2 {
            output.push_str("│ ...                                                             │\n");
            output.push_str("│ [마지막 신호들]                                                 │\n");
            for marker in markers.iter().skip(markers.len() - show_count) {
                let type_str = match marker.signal_type {
                    SignalType::Entry => "ENTRY",
                    SignalType::Exit => "EXIT ",
                    SignalType::AddToPosition => "ADD  ",
                    SignalType::ReducePosition => "REDUC",
                    SignalType::Alert => "ALERT",
                    SignalType::Scale => "SCALE",
                };
                let side_str = marker.side.map(|s| format!("{:?}", s)).unwrap_or_else(|| "-".to_string());
                let exec_str = if marker.executed { "✓" } else { "○" };
                output.push_str(&format!("│ {} {} {:>5} @ {:>12} {} {}                   │\n",
                    marker.timestamp.format("%Y-%m-%d"),
                    type_str,
                    side_str,
                    marker.price.round_dp(2),
                    exec_str,
                    truncate_str(&marker.reason, 15)
                ));
            }
        }
    } else {
        output.push_str("│ 신호 없음                                                       │\n");
    }
    output.push_str("└─────────────────────────────────────────────────────────────────┘\n\n");

    // 5. 거래 결과 요약
    output.push_str("┌─────────────────────────────────────────────────────────────────┐\n");
    output.push_str("│ 5. 거래 결과 요약                                               │\n");
    output.push_str("├─────────────────────────────────────────────────────────────────┤\n");
    output.push_str(&format!("│ 완료된 라운드트립: {:>6}                                       │\n", report.trades.len()));
    output.push_str(&format!("│ 총 거래 수:        {:>6}                                       │\n", report.all_trades.len()));

    if !report.trades.is_empty() {
        let winning = report.trades.iter().filter(|t| t.pnl > Decimal::ZERO).count();
        let losing = report.trades.iter().filter(|t| t.pnl < Decimal::ZERO).count();
        let win_rate = winning as f64 / report.trades.len() as f64 * 100.0;

        output.push_str(&format!("│ 승리:              {:>6} ({:.1}%)                              │\n", winning, win_rate));
        output.push_str(&format!("│ 패배:              {:>6}                                       │\n", losing));
    }
    output.push_str("└─────────────────────────────────────────────────────────────────┘\n");

    // 6. 검증 체크리스트
    output.push_str("\n┌─────────────────────────────────────────────────────────────────┐\n");
    output.push_str("│ 6. 검증 체크리스트                                              │\n");
    output.push_str("├─────────────────────────────────────────────────────────────────┤\n");

    let checks = [
        (entry_exit_diff.abs() <= 1, "Entry/Exit 균형"),
        (total_signals > 0, "신호 생성됨"),
        (executed_count > 0, "실행된 신호 존재"),
        (!report.trades.is_empty(), "거래 완료됨"),
        (report.metrics.max_drawdown_pct < Decimal::from(50), "MDD < 50%"),
    ];

    for (passed, desc) in checks {
        let status = if passed { "✅" } else { "❌" };
        output.push_str(&format!("│ {} {}                                                      │\n", status, desc));
    }
    output.push_str("└─────────────────────────────────────────────────────────────────┘\n");

    output
}

/// 사용 가능한 전략 목록 출력
pub fn print_available_strategies() {
    println!("\n📋 사용 가능한 전략 목록:");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("  전략 타입           | 설명");
    println!("  ─────────────────────────────────────────────────────────────");
    println!("  grid               | 그리드 트레이딩 (횡보장 적합)");
    println!("  rsi                | RSI 평균회귀 (과매수/과매도)");
    println!("  bollinger          | 볼린저 밴드 (동적 변동성)");
    println!("  volatility         | 변동성 돌파 (Larry Williams)");
    println!("  magic_split        | 매직 스플릿 (분할 매수)");
    println!("  compound_momentum  | 복합 모멘텀 (TQQQ/SCHD 자산배분)");
    println!("  haa                | HAA 계층적 자산배분 (카나리아)");
    println!("  xaa                | XAA 확장 자산배분");
    println!("  stock_rotation     | 종목 갈아타기 시스템");
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("예시 설정 파일 (config/backtest/rsi.toml):");
    println!("  name = \"RSI Strategy Backtest\"");
    println!("  strategy_type = \"rsi\"");
    println!("  ");
    println!("  [parameters]");
    println!("  period = 14");
    println!("  overbought = 70");
    println!("  oversold = 30");
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use trader_core::Symbol;

    /// 테스트용 심볼 객체 생성
    fn create_symbol(config: &BacktestCliConfig) -> Symbol {
        match config.market {
            Market::KR => Symbol::kr_stock(config.symbol.to_uppercase(), "KRW"),
            Market::US => Symbol::us_stock(config.symbol.to_uppercase(), "USD"),
        }
    }

    #[test]
    fn test_default_config() {
        let config = BacktestCliConfig::default();
        assert_eq!(config.initial_capital, Decimal::from(10_000_000i64));
    }

    #[test]
    fn test_create_symbol_kr() {
        let config = BacktestCliConfig {
            market: Market::KR,
            symbol: "005930".to_string(),
            ..Default::default()
        };

        let symbol = create_symbol(&config);
        assert_eq!(symbol.base, "005930");
        assert_eq!(symbol.quote, "KRW");
    }

    #[test]
    fn test_create_symbol_us() {
        let config = BacktestCliConfig {
            market: Market::US,
            symbol: "spy".to_string(),
            ..Default::default()
        };

        let symbol = create_symbol(&config);
        assert_eq!(symbol.base, "SPY");
        assert_eq!(symbol.quote, "USD");
    }

    #[test]
    fn test_strategy_type_parsing() {
        assert!(matches!(
            StrategyType::parse("grid"),
            Some(StrategyType::Grid)
        ));
        assert!(matches!(
            StrategyType::parse("RSI"),
            Some(StrategyType::Rsi)
        ));
        assert!(matches!(
            StrategyType::parse("compound_momentum"),
            Some(StrategyType::CompoundMomentum)
        ));
        assert!(StrategyType::parse("unknown").is_none());
    }
}
