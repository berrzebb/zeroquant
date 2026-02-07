//! 전략 통합 테스트 도구.
//!
//! UI와 동일한 환경에서 전략을 테스트하고 상세 진단 정보를 출력합니다.
//!
//! # 주요 기능
//!
//! 1. **UI 동일 흐름**: JSON config → StrategyContext 주입 → 전략 초기화 → 백테스트
//! 2. **상세 진단**: 신호 발생 여부, 거래 내역, 조건 평가 결과
//! 3. **거래 분석**: 진입/청산 시점, 가격, PnL 상세
//! 4. **문제 원인 분석**: 신호 미발생 시 원인 추적
//! 5. **다중 심볼 지원**: 로테이션/자산배분 전략 테스트
//!
//! # 사용 예시
//!
//! ```bash
//! # RSI 전략 테스트 (단일 심볼)
//! trader strategy-test --strategy rsi --symbol 005930 --market KR
//!
//! # 다중 심볼 테스트 (로테이션 전략)
//! trader strategy-test --strategy sector_momentum --symbols "SPY,QQQ,IWM,EFA" --market US
//!
//! # JSON config로 테스트
//! trader strategy-test --strategy grid --config '{"ticker":"005930","grid_count":10}'
//!
//! # 상세 디버그 모드
//! trader strategy-test --strategy rsi --symbol 005930 --debug
//! ```

use anyhow::{anyhow, Result};
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal_macros::dec;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use trader_analytics::backtest::{
    BacktestConfig, BacktestEngine, BacktestReport, BacktestScreeningProvider,
    ScreeningCalculatorConfig,
};
use trader_analytics::AnalyticsProviderImpl;
use trader_core::{AnalyticsProvider, Kline, MarketType, StrategyContext, Timeframe};
use trader_data::cache::CachedHistoricalDataProvider;
use trader_data::storage::ohlcv::OhlcvCache;
use trader_data::{Database, DatabaseConfig};
use trader_strategy::StrategyRegistry;

use crate::commands::download::Market;

/// 전략 테스트 CLI 설정
#[derive(Debug, Clone)]
pub struct StrategyTestConfig {
    /// 전략 ID (예: rsi, grid, bollinger)
    pub strategy_id: String,
    /// 종목 코드 목록 (다중 심볼 지원)
    pub symbols: Vec<String>,
    /// 시장 (KR/US)
    pub market: Market,
    /// JSON 설정 (옵션)
    pub json_config: Option<String>,
    /// 시작일
    pub start_date: Option<NaiveDate>,
    /// 종료일
    pub end_date: Option<NaiveDate>,
    /// 초기 자본금
    pub initial_capital: Decimal,
    /// 디버그 모드 (상세 로그)
    pub debug: bool,
    /// 데이터베이스 URL
    pub db_url: Option<String>,
}

impl Default for StrategyTestConfig {
    fn default() -> Self {
        Self {
            strategy_id: String::new(),
            symbols: Vec::new(),
            market: Market::KR,
            json_config: None,
            start_date: None,
            end_date: None,
            initial_capital: Decimal::from(10_000_000),
            debug: false,
            db_url: None,
        }
    }
}

/// 테스트 결과 상세
#[derive(Debug, Clone)]
#[allow(dead_code)]  // 회귀 테스트 기능에서 사용 예정
pub struct TestResult {
    pub success: bool,
    pub strategy_id: String,
    pub symbols: Vec<String>,
    pub data_points: usize,
    pub signals_generated: usize,
    pub trades_executed: usize,
    pub total_return_pct: Decimal,
    pub win_rate_pct: Decimal,
    pub report: Option<BacktestReport>,
    pub diagnostics: Vec<String>,
}

// ================================================================================================
// 스크리닝 기반 전략 지원
// ================================================================================================

/// 스크리닝 기반 전략 ID 목록
const SCREENING_BASED_STRATEGIES: &[&str] = &[
    "small_cap_quant_v2",
    "pension_bot_v2",
    "dynamic_universe",
    // 필요 시 추가
];

/// 스크리닝 기반 전략이 사용하는 기본 preset_name
/// (ScreeningBasedConfig의 default_preset_name()과 일치해야 함)
const SCREENING_PRESET_NAME: &str = "screening_based";

/// 전략 ID에 따라 BacktestScreeningProvider 생성
///
/// 스크리닝 기반 전략인 경우 적절한 설정으로 Provider를 생성합니다.
/// 전략들은 기본값으로 "screening_based" preset을 사용합니다.
/// 일반 전략인 경우 None을 반환합니다.
fn create_screening_provider_for_strategy(strategy_id: &str) -> Option<BacktestScreeningProvider> {
    if !SCREENING_BASED_STRATEGIES.contains(&strategy_id) {
        return None;
    }

    // 전략별 스크리닝 설정
    // 모든 스크리닝 기반 전략은 기본값으로 "screening_based" preset을 사용
    // (serde default = "default_preset_name" → "screening_based")
    //
    // 주의: 백테스트에서는 Fundamental 데이터(목표가, 손절가, 추천가)가 없어서
    // GlobalScore 계산 시 기술적 지표(33%)만 반영됩니다.
    // 따라서 min_score를 낮게 설정해야 스크리닝 결과가 나옵니다.
    let config = match strategy_id {
        "small_cap_quant_v2" => ScreeningCalculatorConfig::monthly(
            SCREENING_PRESET_NAME,
            dec!(0), // 백테스트: Fundamental 없이 기술적 지표만 계산되므로 min_score 0
        ),
        "pension_bot_v2" => ScreeningCalculatorConfig::weekly(
            SCREENING_PRESET_NAME,
            dec!(0), // 백테스트: Fundamental 없이 기술적 지표만 계산되므로 min_score 0
        ),
        "dynamic_universe" => ScreeningCalculatorConfig::monthly(
            SCREENING_PRESET_NAME,
            dec!(0), // 백테스트: Fundamental 없이 기술적 지표만 계산되므로 min_score 0
        ),
        _ => ScreeningCalculatorConfig::default(),
    };

    Some(BacktestScreeningProvider::with_config(config))
}

/// 전략별 BacktestConfig 생성
///
/// 각 전략의 특성에 따라 allow_short, max_positions 등을 설정합니다.
fn create_backtest_config_for_strategy(
    strategy_id: &str,
    initial_capital: Decimal,
    num_symbols: usize,
) -> BacktestConfig {
    let commission_rate = Decimal::from_f64(0.00015).unwrap_or(Decimal::ZERO);
    let slippage_rate = Decimal::from_f64(0.0005).unwrap_or(Decimal::ZERO);

    // 전략별 숏 허용 여부
    let allow_short = matches!(
        strategy_id,
        "volatility_breakout" | "candle_pattern" | "sma_crossover" | "momentum_surge" |
        "market_bothside" | "us_3x_leverage"
    );

    // 전략별 최대 포지션 수 설정
    let max_positions = match strategy_id {
        // 섹터 기반 전략은 섹터 수에 맞게 설정
        "sector_momentum" => 11,  // US 11개 섹터
        "sector_momentum_kr" => 9,  // KR 9개 섹터
        "sector_vb" => num_symbols.max(10),  // 유니버스 크기에 맞게
        // 로테이션/배분 전략은 유니버스 크기에 맞게
        "stock_rotation" | "stock_rotation_kr" | "market_cap_top" => num_symbols.max(20),
        // 자산 배분 전략
        "haa" | "xaa" | "baa" | "all_weather" | "dual_momentum" => num_symbols.max(10),
        "compound_momentum" | "momentum_power" => num_symbols.max(6),
        // 단일 티커 전략
        _ => 10,
    };

    BacktestConfig::new(initial_capital)
        .with_commission_rate(commission_rate)
        .with_slippage_rate(slippage_rate)
        .with_allow_short(allow_short)
        .with_max_positions(max_positions)
}

/// 전략 테스트 실행
pub async fn run_strategy_test(config: StrategyTestConfig) -> Result<TestResult> {
    let symbols_display = if config.symbols.len() > 3 {
        format!("{}, ... ({} 종목)", config.symbols[..3].join(", "), config.symbols.len())
    } else {
        config.symbols.join(", ")
    };

    println!("\n🧪 전략 통합 테스트 시작");
    println!("═══════════════════════════════════════════════════════════════");
    println!("  전략 ID: {}", config.strategy_id);
    println!("  종목: {} ({})", symbols_display, match config.market {
        Market::KR => "한국",
        Market::US => "미국",
    });
    println!("  초기 자본: {}원", config.initial_capital);
    println!("═══════════════════════════════════════════════════════════════\n");

    let mut diagnostics = Vec::new();

    // 1. 전략 존재 여부 확인
    println!("📋 [1/6] 전략 검증...");
    let available_strategies = StrategyRegistry::list_ids();
    if !available_strategies.contains(&config.strategy_id.as_str()) {
        diagnostics.push(format!("❌ 전략 '{}' 를 찾을 수 없습니다.", config.strategy_id));
        diagnostics.push(format!("사용 가능한 전략: {:?}", available_strategies));
        return Ok(TestResult {
            success: false,
            strategy_id: config.strategy_id,
            symbols: config.symbols,
            data_points: 0,
            signals_generated: 0,
            trades_executed: 0,
            total_return_pct: Decimal::ZERO,
            win_rate_pct: Decimal::ZERO,
            report: None,
            diagnostics,
        });
    }
    println!("  ✅ 전략 '{}' 확인됨", config.strategy_id);

    // 2. 데이터베이스 연결
    println!("\n📋 [2/6] 데이터베이스 연결...");
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

    // 3. 캔들 데이터 로드 (모든 심볼) - ohlcv 테이블 사용
    println!("\n📋 [3/6] 캔들 데이터 로드...");
    let ohlcv_cache = OhlcvCache::new(pool.clone());

    let now = Utc::now();
    let requested_start = config.start_date
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
        .unwrap_or_else(|| now - chrono::Duration::days(365));
    let requested_end = config.end_date
        .map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc())
        .unwrap_or(now);

    // 전략 설정 미리 준비하여 필요한 모든 심볼 추출
    // (자산 배분 전략의 경우 내부 기본 자산 + JSON config의 추가 심볼)
    let mut config = config;  // make mutable
    let preliminary_config = prepare_strategy_config(&config)?;
    let all_required_symbols = extract_required_symbols(
        &preliminary_config,
        &config.symbols[0],
        &config.strategy_id,
    );

    // config.symbols 업데이트 (추출된 모든 심볼 포함, 원래 주 심볼 순서 유지)
    if all_required_symbols.len() > config.symbols.len() {
        // 원래 주 심볼을 첫 번째로 유지
        let primary = config.symbols[0].clone();
        let mut updated_symbols = vec![primary.clone()];
        for symbol in &all_required_symbols {
            if symbol != &primary && !updated_symbols.contains(symbol) {
                updated_symbols.push(symbol.clone());
            }
        }
        println!("  ℹ️ 전략 설정에서 추가 심볼 추출: {} → {} 개 (주 심볼: {})",
            config.symbols.len(), updated_symbols.len(), primary);
        config.symbols = updated_symbols;
    }

    // 모든 심볼의 klines 로드 및 공통 시간 범위 계산
    let mut all_klines: std::collections::HashMap<String, Vec<Kline>> = std::collections::HashMap::new();
    let mut common_start: Option<chrono::DateTime<Utc>> = None;
    let mut common_end: Option<chrono::DateTime<Utc>> = None;

    println!("  📥 {} 심볼 로드 중...", config.symbols.len());
    for symbol in &config.symbols {
        match ohlcv_cache.get_cached_klines_range(symbol, Timeframe::D1, requested_start, requested_end).await {
            Ok(symbol_klines) if !symbol_klines.is_empty() => {
                let sym_start = symbol_klines.first().unwrap().open_time;
                let sym_end = symbol_klines.last().unwrap().close_time;
                println!("    ✅ {} 캔들 로드: {} 개 ({} ~ {})",
                    symbol, symbol_klines.len(),
                    sym_start.format("%Y-%m-%d"),
                    sym_end.format("%Y-%m-%d"));

                // 공통 시간 범위 계산 (교집합)
                common_start = Some(common_start.map_or(sym_start, |cs| cs.max(sym_start)));
                common_end = Some(common_end.map_or(sym_end, |ce| ce.min(sym_end)));

                all_klines.insert(symbol.clone(), symbol_klines);
            }
            Ok(_) => {
                println!("    ⚠️ {} 캔들 없음 (건너뜀)", symbol);
            }
            Err(e) => {
                println!("    ⚠️ {} 로드 실패: {} (건너뜀)", symbol, e);
            }
        }
    }

    // 공통 시간 범위로 klines 필터링
    let (start, end) = match (common_start, common_end) {
        (Some(cs), Some(ce)) if cs < ce => {
            println!("  📅 공통 시간 범위: {} ~ {}",
                cs.format("%Y-%m-%d"), ce.format("%Y-%m-%d"));
            (cs, ce)
        }
        _ => {
            println!("  ⚠️ 공통 시간 범위를 찾을 수 없음, 요청 범위 사용");
            (requested_start, requested_end)
        }
    };

    // 공통 시간 범위로 필터링
    for (_, klines) in all_klines.iter_mut() {
        klines.retain(|k| k.open_time >= start && k.close_time <= end);
    }

    // 첫 번째 심볼의 klines를 메인으로 사용 (백테스트 엔진용)
    let primary_symbol = &config.symbols[0];
    let klines = all_klines.get(primary_symbol).cloned().unwrap_or_default();

    if klines.is_empty() {
        diagnostics.push("❌ 캔들 데이터가 없습니다.".to_string());
        diagnostics.push(format!("  종목: {}", primary_symbol));
        diagnostics.push(format!("  기간: {} ~ {}", start, end));
        diagnostics.push("  해결: `trader download` 또는 `trader import-db`로 데이터 다운로드".to_string());
        return Ok(TestResult {
            success: false,
            strategy_id: config.strategy_id,
            symbols: config.symbols,
            data_points: 0,
            signals_generated: 0,
            trades_executed: 0,
            total_return_pct: Decimal::ZERO,
            win_rate_pct: Decimal::ZERO,
            report: None,
            diagnostics,
        });
    }

    println!("  ✅ {} 캔들 로드 완료 ({})", klines.len(), primary_symbol);
    println!("    보정된 기간: {} ~ {}",
        klines.first().map(|k| k.open_time.format("%Y-%m-%d").to_string()).unwrap_or_default(),
        klines.last().map(|k| k.open_time.format("%Y-%m-%d").to_string()).unwrap_or_default()
    );
    println!("  📊 로드된 심볼: {} 개", all_klines.len());

    // 4. StrategyContext 생성 및 분석 데이터 로드
    println!("\n📋 [4/6] StrategyContext 생성 및 분석 데이터 로드...");
    let context = create_strategy_context(pool.clone(), &config).await?;
    {
        let ctx_read = context.read().await;
        println!("  ✅ StrategyContext 생성 완료");
        println!("    - global_scores: {} 개", ctx_read.global_scores.len());
        println!("    - route_states: {} 개", ctx_read.route_states.len());
        println!("    - screening_results: {} 개", ctx_read.screening_results.len());
    }

    // 백테스트용: route_states를 Armed로 설정 (진입 가능 상태)
    // 실제 RouteState는 백테스트 엔진에서 각 캔들 시점마다 계산하여 업데이트됨
    {
        let mut ctx_write = context.write().await;
        for symbol in &config.symbols {
            ctx_write.route_states.insert(symbol.clone(), trader_core::RouteState::Armed);
        }
        debug!("백테스트용 RouteState 초기화: Armed");

        // 모든 심볼의 klines를 StrategyContext에 저장
        // (MomentumPower 등 멀티 자산 전략이 context.get_klines()로 접근 가능)
        for (symbol, symbol_klines) in &all_klines {
            ctx_write.update_klines(symbol, Timeframe::D1, symbol_klines.clone());
        }
        println!("  ✅ {} 심볼의 klines를 StrategyContext에 저장", all_klines.len());
    }

    // 5. 전략 초기화 및 백테스트
    println!("\n📋 [5/6] 전략 초기화 및 백테스트 실행...");

    // JSON config 준비
    let strategy_config = prepare_strategy_config(&config)?;
    println!("  설정: {}", serde_json::to_string_pretty(&strategy_config)?);

    // 전략 생성
    let mut strategy = StrategyRegistry::create_instance(&config.strategy_id)
        .map_err(|e| anyhow!("전략 생성 실패: {}", e))?;

    // StrategyContext 주입 (중요: initialize 전에 호출!)
    strategy.set_context(Arc::clone(&context));
    println!("  ✅ StrategyContext 주입 완료");

    // 전략 초기화
    strategy
        .initialize(strategy_config.clone())
        .await
        .map_err(|e| {
            diagnostics.push(format!("❌ 전략 초기화 실패: {}", e));
            anyhow!("전략 초기화 실패: {}", e)
        })?;
    println!("  ✅ 전략 초기화 성공");

    // 멀티 타임프레임 전략: 추가 타임프레임 데이터 로드
    if let Some(mtf_config) = strategy.multi_timeframe_config() {
        let required_timeframes: Vec<Timeframe> = mtf_config.timeframes.keys().cloned().collect();
        println!("  📊 멀티 타임프레임 전략 감지: {:?}", required_timeframes);
        
        for tf in &required_timeframes {
            // D1은 이미 로드됨
            if *tf == Timeframe::D1 {
                continue;
            }
            
            // 주 심볼의 추가 타임프레임 데이터 로드
            let primary = &config.symbols[0];
            if let Ok(tf_klines) = ohlcv_cache.get_cached_klines_range(
                primary, 
                *tf, 
                requested_start, 
                requested_end
            ).await {
                if !tf_klines.is_empty() {
                    let mut ctx_write = context.write().await;
                    ctx_write.update_klines(primary, *tf, tf_klines.clone());
                    println!("  ✅ {} 타임프레임 데이터 로드: {} 캔들", tf, tf_klines.len());
                } else {
                    println!("  ⚠️ {} 타임프레임 데이터 없음", tf);
                }
            } else {
                println!("  ⚠️ {} 타임프레임 데이터 로드 실패", tf);
            }
        }
    }

    // 백테스트 실행 (run_with_context 사용)
    // 각 캔들 시점마다 StructuralFeatures를 계산하여 StrategyContext에 업데이트
    let backtest_config = create_backtest_config_for_strategy(
        &config.strategy_id,
        config.initial_capital,
        config.symbols.len(),
    );

    let mut engine = BacktestEngine::new(backtest_config);
    let ticker = config.symbols[0].clone();

    // 스크리닝 기반 전략용 Provider 생성 (해당하는 경우만)
    let screening_provider = create_screening_provider_for_strategy(&config.strategy_id);
    if screening_provider.is_some() {
        println!("  📊 스크리닝 기반 전략 감지: 동적 유니버스 스크리닝 활성화");
    }

    let report = engine
        .run_with_context(&mut *strategy, &klines, context.clone(), &ticker, screening_provider.as_ref().map(|p| p as &dyn trader_core::ScreeningCalculator))
        .await
        .map_err(|e| {
            diagnostics.push(format!("❌ 백테스트 실행 실패: {}", e));
            anyhow!("백테스트 실행 실패: {}", e)
        })?;

    // 6. 결과 분석
    println!("\n📋 [6/6] 결과 분석...");

    let signals_generated = report.trades.len();
    let trades_executed = report.metrics.total_trades;

    // 결과 출력
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("📊 테스트 결과");
    println!("═══════════════════════════════════════════════════════════════");

    if trades_executed == 0 {
        println!("\n⚠️  거래가 발생하지 않았습니다!");
        diagnostics.push("⚠️ 거래 미발생".to_string());
        analyze_no_trades(&klines, &strategy_config, &mut diagnostics);
    } else {
        println!("\n✅ 거래 발생: {} 건", trades_executed);
    }

    println!("\n📈 성과 지표:");
    println!("  총 수익률: {:.2}%", report.metrics.total_return_pct);
    println!("  연환산 수익률: {:.2}%", report.metrics.annualized_return_pct);
    println!("  순이익: {:+.0}원", report.metrics.net_profit);
    println!("  총 거래 수: {}", report.metrics.total_trades);
    println!("  승률: {:.1}%", report.metrics.win_rate_pct);
    println!("  수익 팩터: {:.2}", report.metrics.profit_factor);
    println!("  샤프 비율: {:.2}", report.metrics.sharpe_ratio);
    println!("  최대 낙폭: {:.2}%", report.metrics.max_drawdown_pct);

    // 거래 내역 출력 (디버그 모드 또는 거래 수가 적을 때)
    if config.debug || report.trades.len() <= 20 {
        println!("\n📝 거래 내역:");
        println!("  ─────────────────────────────────────────────────────────────");
        for (i, trade) in report.trades.iter().enumerate() {
            println!("  [{}] {} {} @ {:.0} → {:.0} | PnL: {:+.0} ({:+.2}%)",
                i + 1,
                trade.side,
                trade.symbol,
                trade.entry_price,
                trade.exit_price,
                trade.pnl,
                trade.return_pct
            );
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

    Ok(TestResult {
        success: trades_executed > 0,
        strategy_id: config.strategy_id,
        symbols: config.symbols,
        data_points: klines.len(),
        signals_generated,
        trades_executed: trades_executed as usize,
        total_return_pct: report.metrics.total_return_pct,
        win_rate_pct: report.metrics.win_rate_pct,
        report: Some(report),
        diagnostics,
    })
}

/// 전략 설정 준비
fn prepare_strategy_config(config: &StrategyTestConfig) -> Result<serde_json::Value> {
    let mut json_config = if let Some(ref json_str) = config.json_config {
        serde_json::from_str(json_str)?
    } else {
        serde_json::json!({})
    };

    // ticker/tickers 주입
    if let Some(obj) = json_config.as_object_mut() {
        // 다중 심볼인 경우 tickers 배열로, 단일 심볼인 경우 ticker로
        if config.symbols.len() > 1 {
            if !obj.contains_key("tickers") {
                obj.insert("tickers".to_string(), serde_json::json!(config.symbols));
            }
        } else if !obj.contains_key("ticker") {
            obj.insert("ticker".to_string(), serde_json::json!(&config.symbols[0]));
        }

        if !obj.contains_key("amount") {
            obj.insert("amount".to_string(), serde_json::json!(config.initial_capital.to_string()));
        }

        // 자산배분 전략용 기본 설정 주입
        inject_asset_allocation_defaults(obj, &config.strategy_id, &config.market);
    }

    Ok(json_config)
}

/// 전략 설정에서 필요한 모든 심볼 추출
///
/// HAA, XAA 등 자산 배분 전략의 경우 내부 기본 자산 목록과
/// JSON config의 커스텀 설정 모두에서 심볼을 추출
fn extract_required_symbols(
    json_config: &serde_json::Value,
    primary_symbol: &str,
    strategy_id: &str,
) -> Vec<String> {
    let mut symbols = std::collections::HashSet::new();

    // 주 심볼 추가
    symbols.insert(primary_symbol.to_string());

    // 자산 배분 전략의 내부 기본 자산 추가
    // (이 티커들은 전략 내부에서 하드코딩되어 있음)
    match strategy_id {
        "haa" => {
            // HAA 기본 자산 (AssetAllocationConfig::haa_default)
            for ticker in ["VWO", "BND", "SPY", "VEA", "AGG", "SHY", "IEF", "LQD", "BIL"] {
                symbols.insert(ticker.to_string());
            }
        }
        "xaa" => {
            // XAA 기본 자산
            for ticker in ["VWO", "BND", "SPY", "VEA", "LQD", "HYG", "EMB", "SHY", "IEF", "TLT", "BIL"] {
                symbols.insert(ticker.to_string());
            }
        }
        "baa" => {
            // BAA 기본 자산
            for ticker in ["SPY", "VEA", "VWO", "AGG", "SHY", "IEF", "TLT", "LQD", "BIL"] {
                symbols.insert(ticker.to_string());
            }
        }
        "all_weather" => {
            // All Weather 기본 자산
            for ticker in ["SPY", "TLT", "IEF", "GLD", "DBC", "BIL"] {
                symbols.insert(ticker.to_string());
            }
        }
        "dual_momentum" => {
            // Dual Momentum 기본 자산
            for ticker in ["SPY", "EFA", "BIL", "AGG"] {
                symbols.insert(ticker.to_string());
            }
        }
        "compound_momentum" | "momentum_power" => {
            // Momentum Power 기본 자산 (US: TIP, UPRO, TLT, BIL / KR: TIP, 122630, 148070, 114800)
            for ticker in ["TIP", "UPRO", "TLT", "BIL", "122630", "148070", "114800"] {
                symbols.insert(ticker.to_string());
            }
        }
        "momentum_surge" => {
            // Momentum Surge 기본 ETF
            for ticker in ["122630", "229200", "114800", "132030", "251340", "233740", "305720"] {
                symbols.insert(ticker.to_string());
            }
        }
        "sector_momentum" => {
            // US 섹터 ETF 기본값
            for ticker in ["XLK", "XLF", "XLV", "XLY", "XLP", "XLE", "XLI", "XLB", "XLU", "XLRE", "XLC"] {
                symbols.insert(ticker.to_string());
            }
        }
        "sector_momentum_kr" => {
            // KR 섹터 ETF 기본값
            for ticker in ["091160", "091170", "091180", "266360", "266390", "266410", "266430"] {
                symbols.insert(ticker.to_string());
            }
        }
        "pension_bot" => {
            // 연금 자동화 기본 포트폴리오 (한국 ETF 20개)
            for ticker in [
                "448290", "379780", "294400", "200250", "283580", "195970", "161510",
                "445910", "305080", "148070", "385560", "304660", "114470", "329750",
                "319640", "276000", "261220", "139310", "137610", "130730"
            ] {
                symbols.insert(ticker.to_string());
            }
        }
        "us_3x_leverage" => {
            // 미국 3배 레버리지 기본 ETF
            for ticker in ["TQQQ", "SOXL", "SQQQ", "SOXS"] {
                symbols.insert(ticker.to_string());
            }
        }
        "market_bothside" => {
            // 양방향 전략 기본 ETF (레버리지 + 인버스)
            for ticker in ["122630", "252670"] {
                symbols.insert(ticker.to_string());
            }
        }
        "sector_vb" => {
            // 섹터 변동성 돌파 기본 섹터 ETF
            for ticker in ["091160", "091230", "305720", "305540", "091170", "091220", "102970", "117460", "091180"] {
                symbols.insert(ticker.to_string());
            }
        }
        _ => {}
    }

    // JSON config에서 추가 심볼 추출
    if let Some(obj) = json_config.as_object() {
        // ticker 필드
        if let Some(ticker) = obj.get("ticker").and_then(|v| v.as_str()) {
            symbols.insert(ticker.to_string());
        }

        // tickers 배열
        if let Some(tickers) = obj.get("tickers").and_then(|v| v.as_array()) {
            for ticker in tickers.iter().filter_map(|v| v.as_str()) {
                symbols.insert(ticker.to_string());
            }
        }

        // canary_tickers 배열 (HAA 계열)
        if let Some(tickers) = obj.get("canary_tickers").and_then(|v| v.as_array()) {
            for ticker in tickers.iter().filter_map(|v| v.as_str()) {
                symbols.insert(ticker.to_string());
            }
        }

        // offensive_tickers 배열 (HAA, XAA)
        if let Some(tickers) = obj.get("offensive_tickers").and_then(|v| v.as_array()) {
            for ticker in tickers.iter().filter_map(|v| v.as_str()) {
                symbols.insert(ticker.to_string());
            }
        }

        // defensive_tickers 배열 (HAA, XAA)
        if let Some(tickers) = obj.get("defensive_tickers").and_then(|v| v.as_array()) {
            for ticker in tickers.iter().filter_map(|v| v.as_str()) {
                symbols.insert(ticker.to_string());
            }
        }

        // cash_ticker (HAA, XAA, BAA)
        if let Some(ticker) = obj.get("cash_ticker").and_then(|v| v.as_str()) {
            symbols.insert(ticker.to_string());
        }

        // universe 배열 (로테이션 전략)
        if let Some(tickers) = obj.get("universe").and_then(|v| v.as_array()) {
            for ticker in tickers.iter().filter_map(|v| v.as_str()) {
                symbols.insert(ticker.to_string());
            }
        }

        // asset_universe 배열 (Momentum Power 등)
        if let Some(tickers) = obj.get("asset_universe").and_then(|v| v.as_array()) {
            for ticker in tickers.iter().filter_map(|v| v.as_str()) {
                symbols.insert(ticker.to_string());
            }
        }

        // attack_asset, safe_asset (Momentum Power)
        if let Some(ticker) = obj.get("attack_asset").and_then(|v| v.as_str()) {
            symbols.insert(ticker.to_string());
        }
        if let Some(ticker) = obj.get("safe_asset").and_then(|v| v.as_str()) {
            symbols.insert(ticker.to_string());
        }

        // etf_tickers (Momentum Surge)
        if let Some(tickers) = obj.get("etf_tickers").and_then(|v| v.as_array()) {
            for ticker in tickers.iter().filter_map(|v| v.as_str()) {
                symbols.insert(ticker.to_string());
            }
        }
    }

    symbols.into_iter().collect()
}

/// 자산배분 전략용 기본 설정 주입
fn inject_asset_allocation_defaults(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    strategy_id: &str,
    market: &Market,
) {
    // 자산배분 전략 목록
    let asset_allocation_strategies = [
        "haa", "xaa", "baa", "all_weather", "dual_momentum",
        "sector_momentum", "stock_rotation", "sector_momentum_kr", "stock_rotation_kr",
        "market_cap_top", "compound_momentum", "momentum_power"
    ];

    if !asset_allocation_strategies.contains(&strategy_id) {
        return;
    }

    // cash_ticker 기본값
    if !obj.contains_key("cash_ticker") {
        let default_cash = match market {
            Market::US => "BIL",  // 미국 단기 국채 ETF
            Market::KR => "SHY",  // 한국은 적당한 현금 대용이 없어 미국 단기채 사용
        };
        obj.insert("cash_ticker".to_string(), serde_json::json!(default_cash));
    }

    // offensive_top_n 기본값 (HAA, XAA, BAA)
    if !obj.contains_key("offensive_top_n") {
        let top_n = match strategy_id {
            "baa" | "all_weather" | "dual_momentum" => 1,
            _ => 4,  // HAA, XAA 기본값
        };
        obj.insert("offensive_top_n".to_string(), serde_json::json!(top_n));
    }

    // defensive_top_n 기본값
    if !obj.contains_key("defensive_top_n") {
        obj.insert("defensive_top_n".to_string(), serde_json::json!(3));
    }

    // invest_rate 기본값
    if !obj.contains_key("invest_rate") {
        obj.insert("invest_rate".to_string(), serde_json::json!("1.0"));
    }

    // rebalance_threshold 기본값
    if !obj.contains_key("rebalance_threshold") {
        obj.insert("rebalance_threshold".to_string(), serde_json::json!("5.0"));
    }

    // min_global_score 기본값
    if !obj.contains_key("min_global_score") {
        let score = match strategy_id {
            "all_weather" => 0,  // All Weather는 스코어 필터 없음
            _ => 55,
        };
        obj.insert("min_global_score".to_string(), serde_json::json!(score));
    }

    // canary_threshold 기본값
    if !obj.contains_key("canary_threshold") {
        let threshold = match strategy_id {
            "baa" => "0.75",            // BAA는 75%
            "dual_momentum" => "1.0",   // DualMomentum은 100%
            "all_weather" => "0.0",     // AllWeather는 카나리아 없음
            _ => "0.5",                 // HAA, XAA 기본값 50%
        };
        obj.insert("canary_threshold".to_string(), serde_json::json!(threshold));
    }

    // bond_momentum_months 기본값 (XAA용)
    if strategy_id == "xaa" && !obj.contains_key("bond_momentum_months") {
        obj.insert("bond_momentum_months".to_string(), serde_json::json!(6));
    }

    // canary_tickers 기본값 (HAA 계열)
    if strategy_id == "haa" && !obj.contains_key("canary_tickers") {
        obj.insert("canary_tickers".to_string(), serde_json::json!(["SPY", "EFA"]));
    }

    // 로테이션 전략용 기본 설정
    if (strategy_id.contains("rotation") || strategy_id.contains("momentum"))
        && !obj.contains_key("lookback_period") {
        obj.insert("lookback_period".to_string(), serde_json::json!(12));
    }

    // 리밸런싱 주기
    if !obj.contains_key("rebalance_period") {
        obj.insert("rebalance_period".to_string(), serde_json::json!("monthly"));
    }
}

/// StrategyContext 생성 및 분석 데이터 로드
///
/// 실제 trader-api와 동일한 방식으로 AnalyticsProvider를 사용하여
/// GlobalScore, RouteState, StructuralFeatures 등을 로드합니다.
async fn create_strategy_context(
    pool: sqlx::PgPool,
    config: &StrategyTestConfig,
) -> Result<Arc<RwLock<StrategyContext>>> {
    // 데이터 제공자 생성
    let data_provider = Arc::new(CachedHistoricalDataProvider::new(pool.clone()));
    let analytics_provider = AnalyticsProviderImpl::new(data_provider);

    // 기본 StrategyContext 생성
    let mut ctx = StrategyContext::default();

    // 시장 유형 결정
    let market_type = match config.market {
        Market::KR => MarketType::Stock,
        Market::US => MarketType::Stock,
    };

    // Global Score 로드
    match analytics_provider.fetch_global_scores(market_type).await {
        Ok(scores) => {
            ctx.update_global_scores(scores);
            debug!("GlobalScore 로드 완료: {} 개", ctx.global_scores.len());
        }
        Err(e) => {
            warn!("GlobalScore 로드 실패 (계속 진행): {}", e);
        }
    }

    // RouteState 로드 (모든 테스트 대상 종목)
    let tickers: Vec<&str> = config.symbols.iter().map(|s| s.as_str()).collect();
    match analytics_provider.fetch_route_states(&tickers).await {
        Ok(states) => {
            ctx.update_route_states(states);
            debug!("RouteState 로드 완료: {} 개", ctx.route_states.len());
        }
        Err(e) => {
            warn!("RouteState 로드 실패 (계속 진행): {}", e);
        }
    }

    // MarketRegime 로드
    match analytics_provider.fetch_market_regimes(&tickers).await {
        Ok(regimes) => {
            ctx.update_market_regime(regimes);
            debug!("MarketRegime 로드 완료");
        }
        Err(e) => {
            warn!("MarketRegime 로드 실패 (계속 진행): {}", e);
        }
    }

    // StructuralFeatures 로드
    match analytics_provider.fetch_features(&tickers).await {
        Ok(features) => {
            ctx.update_features(features);
            debug!("StructuralFeatures 로드 완료");
        }
        Err(e) => {
            warn!("StructuralFeatures 로드 실패 (계속 진행): {}", e);
        }
    }

    // MacroEnvironment 로드 (글로벌)
    match analytics_provider.fetch_macro_environment().await {
        Ok(macro_env) => {
            ctx.update_macro_environment(macro_env);
            debug!("MacroEnvironment 로드 완료");
        }
        Err(e) => {
            warn!("MacroEnvironment 로드 실패 (계속 진행): {}", e);
        }
    }

    // MarketBreadth 로드 (글로벌)
    match analytics_provider.fetch_market_breadth().await {
        Ok(breadth) => {
            ctx.update_market_breadth(breadth);
            debug!("MarketBreadth 로드 완료");
        }
        Err(e) => {
            warn!("MarketBreadth 로드 실패 (계속 진행): {}", e);
        }
    }

    Ok(Arc::new(RwLock::new(ctx)))
}

/// 거래 미발생 원인 분석
fn analyze_no_trades(klines: &[Kline], config: &serde_json::Value, diagnostics: &mut Vec<String>) {
    diagnostics.push("\n🔍 거래 미발생 원인 분석:".to_string());

    // 1. 데이터 부족 확인
    if klines.len() < 50 {
        diagnostics.push(format!("  - 데이터 부족: {}개 캔들 (최소 50개 권장)", klines.len()));
    }

    // 2. 설정 값 확인
    if let Some(obj) = config.as_object() {
        // RSI 설정
        if let Some(overbought) = obj.get("overbought").and_then(|v| v.as_f64()) {
            if overbought < 60.0 {
                diagnostics.push(format!("  - RSI 과매수 임계값이 낮음: {}", overbought));
            }
        }
        if let Some(oversold) = obj.get("oversold").and_then(|v| v.as_f64()) {
            if oversold > 40.0 {
                diagnostics.push(format!("  - RSI 과매도 임계값이 높음: {}", oversold));
            }
        }

        // min_score 확인
        if let Some(min_score) = obj.get("min_score").and_then(|v| v.as_f64()) {
            if min_score > 80.0 {
                diagnostics.push(format!("  - GlobalScore 필터가 너무 엄격: min_score={}", min_score));
            }
        }

        // enable_route_filter 확인
        if obj.get("enable_route_filter").and_then(|v| v.as_bool()).unwrap_or(false) {
            diagnostics.push("  - RouteState 필터 활성화됨 (백테스트에서는 RouteState가 없을 수 있음)".to_string());
        }
    }

    // 3. 가격 움직임 분석
    if klines.len() > 1 {
        let first_close = klines.first().map(|k| k.close).unwrap_or(Decimal::ONE);
        let last_close = klines.last().map(|k| k.close).unwrap_or(Decimal::ONE);

        if first_close > Decimal::ZERO {
            let change_pct = ((last_close - first_close) / first_close * Decimal::from(100))
                .to_string().parse::<f64>().unwrap_or(0.0);

            if change_pct.abs() < 5.0 {
                diagnostics.push(format!("  - 기간 내 가격 변동이 적음: {:.1}%", change_pct));
            }
        }
    }

    diagnostics.push("\n💡 권장 조치:".to_string());
    diagnostics.push("  1. 전략 파라미터 완화 (RSI 임계값 조정 등)".to_string());
    diagnostics.push("  2. GlobalScore/RouteState 필터 비활성화".to_string());
    diagnostics.push("  3. 더 긴 기간 또는 더 변동성 있는 종목으로 테스트".to_string());
}

/// 사용 가능한 전략 목록 출력
pub fn print_available_strategies() {
    println!("\n📋 사용 가능한 전략 목록:");
    println!("═══════════════════════════════════════════════════════════════");

    let strategies = StrategyRegistry::all();
    for meta in strategies {
        println!("  {:15} | {:20} | {}", meta.id, meta.name, meta.description);
    }

    println!("═══════════════════════════════════════════════════════════════");
}

// ============================================================================
// 회귀 테스트 시스템
// ============================================================================

use serde::{Deserialize, Serialize};
use std::path::Path;

/// 회귀 테스트 Fixture 파일 구조
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureFile {
    pub description: String,
    pub strategies: Vec<StrategyFixture>,
}

/// 개별 전략 Fixture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyFixture {
    /// 전략 ID (레지스트리 등록명)
    pub strategy_id: String,
    /// 전략 표시 이름
    pub name: String,
    /// 테스트 종목 목록
    pub symbols: Vec<String>,
    /// 시장 타입 (KR/US)
    pub market: String,
    /// 전략 설정
    pub config: serde_json::Value,
    /// 기대 결과
    pub expected: ExpectedResult,
}

/// 기대 결과 (회귀 테스트 baseline)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedResult {
    /// 초기화 성공 여부 ("success" | "failure")
    pub initialization: String,

    // === 정확한 baseline 값 (회귀 테스트의 핵심) ===
    /// 정확한 거래 수 (baseline)
    #[serde(default)]
    pub trades_executed: Option<usize>,
    /// 정확한 총 수익률 % (baseline)
    #[serde(default)]
    pub total_return_pct: Option<f64>,
    /// 정확한 최대 낙폭 % (baseline)
    #[serde(default)]
    pub max_drawdown_pct: Option<f64>,
    /// 정확한 승률 % (baseline)
    #[serde(default)]
    pub win_rate_pct: Option<f64>,

    // === 하위 호환용 (범위 검증) ===
    /// 최소 거래 수 (범위 검증용)
    #[serde(default)]
    pub min_trades: Option<usize>,
    /// 최소 수익률 (범위 검증용)
    #[serde(default)]
    pub min_return_pct: Option<f64>,

    /// 수치 비교 시 허용 오차 % (기본: 1.0 = 1%)
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,
}

fn default_tolerance() -> f64 {
    1.0 // 1% 허용 오차
}

/// 회귀 테스트 결과
#[derive(Debug, Clone)]
#[allow(dead_code)]  // 회귀 테스트 CLI 명령에서 사용 예정
pub struct RegressionTestResult {
    pub fixture_path: String,
    pub total_tests: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<SingleTestResult>,
}

/// 개별 테스트 결과
#[derive(Debug, Clone)]
pub struct SingleTestResult {
    pub strategy_id: String,
    pub strategy_name: String,
    pub passed: bool,
    pub error_message: Option<String>,
    pub test_result: Option<TestResult>,
}

/// Fixture 파일 로드
pub fn load_fixture(path: &Path) -> Result<FixtureFile> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow!("Fixture 파일 읽기 실패 ({}): {}", path.display(), e))?;

    let fixture: FixtureFile = serde_json::from_str(&content)
        .map_err(|e| anyhow!("Fixture JSON 파싱 실패 ({}): {}", path.display(), e))?;

    Ok(fixture)
}

/// 모든 Fixture 파일 발견
pub fn discover_fixtures(fixtures_dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut fixtures = Vec::new();

    if !fixtures_dir.exists() {
        return Err(anyhow!("Fixture 디렉토리가 존재하지 않습니다: {}", fixtures_dir.display()));
    }

    for entry in std::fs::read_dir(fixtures_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            fixtures.push(path);
        }
    }

    fixtures.sort();
    Ok(fixtures)
}

/// 회귀 테스트 설정
#[derive(Debug, Clone)]
pub struct RegressionTestOptions {
    /// 차트 출력 디렉토리 (None이면 차트 생성 안함)
    pub chart_output_dir: Option<std::path::PathBuf>,
    /// 데이터베이스 URL
    pub db_url: Option<String>,
}

impl Default for RegressionTestOptions {
    fn default() -> Self {
        Self {
            chart_output_dir: None,
            db_url: None,
        }
    }
}

/// 회귀 테스트 실행
#[allow(dead_code)]  // 회귀 테스트 CLI 명령 추가 시 활성화
pub async fn run_regression_tests(fixtures_dir: &Path, db_url: Option<String>) -> Result<Vec<RegressionTestResult>> {
    run_regression_tests_with_options(
        fixtures_dir,
        RegressionTestOptions {
            chart_output_dir: None,
            db_url,
        },
    )
    .await
}

/// 회귀 테스트 실행 (차트 생성 옵션 포함)
pub async fn run_regression_tests_with_options(
    fixtures_dir: &Path,
    options: RegressionTestOptions,
) -> Result<Vec<RegressionTestResult>> {
    let fixture_paths = discover_fixtures(fixtures_dir)?;

    if fixture_paths.is_empty() {
        return Err(anyhow!("Fixture 파일이 없습니다: {}", fixtures_dir.display()));
    }

    println!("\n🧪 회귀 테스트 시작");
    println!("═══════════════════════════════════════════════════════════════");
    println!("  Fixture 디렉토리: {}", fixtures_dir.display());
    println!("  발견된 Fixture 파일: {} 개", fixture_paths.len());
    if options.chart_output_dir.is_some() {
        println!("  📊 차트 생성: 활성화");
    }
    println!("═══════════════════════════════════════════════════════════════\n");

    let mut all_results = Vec::new();

    for fixture_path in fixture_paths {
        let result = run_fixture_tests(&fixture_path, options.db_url.clone()).await?;
        all_results.push(result);
    }

    // 최종 요약
    print_regression_summary(&all_results);

    // 차트 생성 (옵션이 설정된 경우)
    if let Some(ref chart_dir) = options.chart_output_dir {
        generate_charts_from_results(&all_results, chart_dir)?;
    }

    Ok(all_results)
}

/// 테스트 결과에서 차트 생성
pub fn generate_charts_from_results(results: &[RegressionTestResult], output_dir: &Path) -> Result<()> {
    use crate::commands::chart_gen::RegressionChartGenerator;

    println!("\n📊 차트 이미지 생성 중...");
    println!("───────────────────────────────────────────────────────────────");

    // 테스트 결과에서 BacktestReport 추출
    let mut chart_data: Vec<(String, String, trader_analytics::backtest::BacktestReport)> = Vec::new();

    for result in results {
        for test in &result.results {
            if let Some(ref test_result) = test.test_result {
                if let Some(ref report) = test_result.report {
                    chart_data.push((
                        test.strategy_id.clone(),
                        test.strategy_name.clone(),
                        report.clone(),
                    ));
                }
            }
        }
    }

    if chart_data.is_empty() {
        println!("  ⚠️  차트 생성할 데이터가 없습니다");
        return Ok(());
    }

    // 차트 디렉토리 생성
    std::fs::create_dir_all(output_dir)?;

    let generator = RegressionChartGenerator::new();
    let mut generated_count = 0;

    for (strategy_id, _name, report) in &chart_data {
        if report.equity_curve.is_empty() {
            println!("  ⚠️  {} - 자산 곡선 데이터 없음 (차트 생략)", strategy_id);
            continue;
        }

        // 최소 2개의 데이터 포인트 필요
        if report.equity_curve.len() < 2 {
            println!(
                "  ⚠️  {} - 데이터 포인트 부족 ({} < 2) (차트 생략)",
                strategy_id,
                report.equity_curve.len()
            );
            continue;
        }

        let filename = format!("{}_chart.png", strategy_id);
        let output_path = output_dir.join(&filename);

        // panic catch로 차트 생성 (plotters 오버플로우 방지)
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generator.generate_combined_chart(report, strategy_id, &output_path)
        }));

        match result {
            Ok(Ok(())) => {
                println!("  ✅ {} - {}", strategy_id, filename);
                generated_count += 1;
            }
            Ok(Err(e)) => {
                println!("  ❌ {} - 차트 생성 실패: {}", strategy_id, e);
            }
            Err(_) => {
                println!("  ❌ {} - 차트 생성 중 내부 오류 (데이터 범위 문제)", strategy_id);
            }
        }
    }

    println!("───────────────────────────────────────────────────────────────");
    println!("  📁 출력 디렉토리: {}", output_dir.display());
    println!("  📊 생성된 차트: {} 개", generated_count);

    Ok(())
}

/// 단일 Fixture 파일의 테스트 실행
pub async fn run_fixture_tests(fixture_path: &Path, db_url: Option<String>) -> Result<RegressionTestResult> {
    let fixture = load_fixture(fixture_path)?;

    println!("\n📁 Fixture: {} ({})", fixture_path.file_name().unwrap().to_string_lossy(), fixture.description);
    println!("───────────────────────────────────────────────────────────────");

    let mut results = Vec::new();
    let mut passed = 0;
    let mut failed = 0;

    for strategy_fixture in &fixture.strategies {
        let result = run_single_fixture_test(strategy_fixture, db_url.clone()).await;

        match &result {
            Ok(test_result) => {
                let (test_passed, validation_errors) = validate_test_result_detailed(test_result, &strategy_fixture.expected);

                // 실제 결과 출력
                let return_pct: f64 = test_result.total_return_pct.try_into().unwrap_or(0.0);
                let win_rate: f64 = test_result.win_rate_pct.try_into().unwrap_or(0.0);

                if test_passed {
                    passed += 1;
                    println!("  ✅ {} ({}) | 거래: {} | 수익률: {:.2}% | 승률: {:.1}%",
                        strategy_fixture.name,
                        strategy_fixture.strategy_id,
                        test_result.trades_executed,
                        return_pct,
                        win_rate
                    );
                } else {
                    failed += 1;
                    println!("  ❌ {} ({}) | 거래: {} | 수익률: {:.2}% | 승률: {:.1}%",
                        strategy_fixture.name,
                        strategy_fixture.strategy_id,
                        test_result.trades_executed,
                        return_pct,
                        win_rate
                    );
                    // 검증 실패 사유 출력
                    for err in &validation_errors {
                        println!("     └─ {}", err);
                    }
                }

                results.push(SingleTestResult {
                    strategy_id: strategy_fixture.strategy_id.clone(),
                    strategy_name: strategy_fixture.name.clone(),
                    passed: test_passed,
                    error_message: if test_passed { None } else { Some(validation_errors.join("; ")) },
                    test_result: Some(test_result.clone()),
                });
            }
            Err(e) => {
                failed += 1;
                let expected_failure = strategy_fixture.expected.initialization == "failure";

                if expected_failure {
                    passed += 1;
                    failed -= 1;
                    println!("  ✅ {} ({}) - 예상된 실패", strategy_fixture.name, strategy_fixture.strategy_id);
                    results.push(SingleTestResult {
                        strategy_id: strategy_fixture.strategy_id.clone(),
                        strategy_name: strategy_fixture.name.clone(),
                        passed: true,
                        error_message: None,
                        test_result: None,
                    });
                } else {
                    println!("  ❌ {} ({}) - {}", strategy_fixture.name, strategy_fixture.strategy_id, e);
                    results.push(SingleTestResult {
                        strategy_id: strategy_fixture.strategy_id.clone(),
                        strategy_name: strategy_fixture.name.clone(),
                        passed: false,
                        error_message: Some(e.to_string()),
                        test_result: None,
                    });
                }
            }
        }
    }

    Ok(RegressionTestResult {
        fixture_path: fixture_path.display().to_string(),
        total_tests: fixture.strategies.len(),
        passed,
        failed,
        results,
    })
}

/// 개별 Fixture 테스트 실행
async fn run_single_fixture_test(fixture: &StrategyFixture, db_url: Option<String>) -> Result<TestResult> {
    let market = match fixture.market.to_uppercase().as_str() {
        "KR" => Market::KR,
        "US" => Market::US,
        _ => return Err(anyhow!("알 수 없는 시장: {}", fixture.market)),
    };

    let config = StrategyTestConfig {
        strategy_id: fixture.strategy_id.clone(),
        symbols: fixture.symbols.clone(),
        market,
        json_config: Some(serde_json::to_string(&fixture.config)?),
        start_date: None,
        end_date: None,
        initial_capital: Decimal::from(10_000_000),
        debug: false,
        db_url,
    };

    // 조용한 모드로 테스트 실행 (로깅 최소화)
    run_strategy_test_quiet(config).await
}

/// 조용한 모드 테스트 실행 (회귀 테스트용)
async fn run_strategy_test_quiet(config: StrategyTestConfig) -> Result<TestResult> {
    // 전략 존재 여부 확인
    let available_strategies = StrategyRegistry::list_ids();
    if !available_strategies.contains(&config.strategy_id.as_str()) {
        return Err(anyhow!("전략 '{}' 를 찾을 수 없습니다", config.strategy_id));
    }

    // 데이터베이스 연결
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

    // 캔들 데이터 로드 - ohlcv 테이블 사용
    let ohlcv_cache = OhlcvCache::new(pool.clone());

    let now = Utc::now();
    let requested_start = now - chrono::Duration::days(365);
    let requested_end = now;

    // 전략 설정 미리 준비하여 필요한 모든 심볼 추출
    // (자산 배분 전략의 경우 내부 기본 자산 + JSON config의 추가 심볼)
    let mut config = config;  // make mutable
    let preliminary_config = prepare_strategy_config(&config)?;
    let all_required_symbols = extract_required_symbols(
        &preliminary_config,
        &config.symbols[0],
        &config.strategy_id,
    );

    // config.symbols 업데이트 (추출된 모든 심볼 포함)
    if all_required_symbols.len() > config.symbols.len() {
        let primary = config.symbols[0].clone();
        let mut updated_symbols = vec![primary.clone()];
        for symbol in &all_required_symbols {
            if symbol != &primary && !updated_symbols.contains(symbol) {
                updated_symbols.push(symbol.clone());
            }
        }
        config.symbols = updated_symbols;
    }

    // 모든 심볼의 klines 로드 및 공통 시간 범위 계산
    let mut all_klines: std::collections::HashMap<String, Vec<Kline>> = std::collections::HashMap::new();
    let mut common_start: Option<chrono::DateTime<Utc>> = None;
    let mut common_end: Option<chrono::DateTime<Utc>> = None;

    for symbol in &config.symbols {
        match ohlcv_cache.get_cached_klines_range(symbol, Timeframe::D1, requested_start, requested_end).await {
            Ok(symbol_klines) if !symbol_klines.is_empty() => {
                let sym_start = symbol_klines.first().unwrap().open_time;
                let sym_end = symbol_klines.last().unwrap().close_time;

                // 공통 시간 범위 계산 (교집합)
                common_start = Some(common_start.map_or(sym_start, |cs| cs.max(sym_start)));
                common_end = Some(common_end.map_or(sym_end, |ce| ce.min(sym_end)));

                all_klines.insert(symbol.clone(), symbol_klines);
            }
            _ => {
                // 데이터 없는 심볼은 건너뜀
            }
        }
    }

    // 공통 시간 범위로 klines 필터링
    let (start, end) = match (common_start, common_end) {
        (Some(cs), Some(ce)) if cs < ce => (cs, ce),
        _ => (requested_start, requested_end),
    };

    for (_, klines) in all_klines.iter_mut() {
        klines.retain(|k| k.open_time >= start && k.close_time <= end);
    }

    // 첫 번째 심볼의 klines를 메인으로 사용 (백테스트 엔진용)
    let primary_symbol = &config.symbols[0];
    let klines = all_klines.get(primary_symbol).cloned().unwrap_or_default();

    if klines.is_empty() {
        return Err(anyhow!("캔들 데이터가 없습니다: {}", primary_symbol));
    }

    // StrategyContext 생성
    let context = create_strategy_context(pool.clone(), &config).await?;

    // 모든 심볼의 klines를 StrategyContext에 저장
    // (멀티 자산 전략이 context.get_klines()로 접근 가능)
    {
        let mut ctx_write = context.write().await;
        for (symbol, symbol_klines) in &all_klines {
            ctx_write.update_klines(symbol, Timeframe::D1, symbol_klines.clone());
        }
    }

    // 전략 설정 준비
    let strategy_config = prepare_strategy_config(&config)?;

    // 전략 생성 및 초기화
    let mut strategy = StrategyRegistry::create_instance(&config.strategy_id)
        .map_err(|e| anyhow!("전략 생성 실패: {}", e))?;

    strategy.set_context(Arc::clone(&context));

    strategy
        .initialize(strategy_config.clone())
        .await
        .map_err(|e| anyhow!("전략 초기화 실패: {}", e))?;

    // 멀티 타임프레임 전략: 추가 타임프레임 데이터 로드
    if let Some(mtf_config) = strategy.multi_timeframe_config() {
        let required_timeframes: Vec<Timeframe> = mtf_config.timeframes.keys().cloned().collect();
        
        for tf in &required_timeframes {
            // D1은 이미 로드됨
            if *tf == Timeframe::D1 {
                continue;
            }
            
            // 주 심볼의 추가 타임프레임 데이터 로드
            let primary = &config.symbols[0];
            if let Ok(tf_klines) = ohlcv_cache.get_cached_klines_range(
                primary, 
                *tf, 
                requested_start, 
                requested_end
            ).await {
                if !tf_klines.is_empty() {
                    let mut ctx_write = context.write().await;
                    ctx_write.update_klines(primary, *tf, tf_klines);
                }
            }
        }
    }

    // 백테스트 실행
    let backtest_config = create_backtest_config_for_strategy(
        &config.strategy_id,
        config.initial_capital,
        config.symbols.len(),
    );

    let mut engine = BacktestEngine::new(backtest_config);
    let ticker = primary_symbol.clone();

    // 스크리닝 기반 전략용 Provider 생성
    let screening_provider = create_screening_provider_for_strategy(&config.strategy_id);

    let report = engine
        .run_with_context(&mut *strategy, &klines, context.clone(), &ticker, screening_provider.as_ref().map(|p| p as &dyn trader_core::ScreeningCalculator))
        .await
        .map_err(|e| anyhow!("백테스트 실행 실패: {}", e))?;

    let signals_generated = report.trades.len();
    let trades_executed = report.metrics.total_trades;

    Ok(TestResult {
        success: true,  // 초기화 성공
        strategy_id: config.strategy_id,
        symbols: config.symbols,
        data_points: klines.len(),
        signals_generated,
        trades_executed: trades_executed as usize,
        total_return_pct: report.metrics.total_return_pct,
        win_rate_pct: report.metrics.win_rate_pct,
        report: Some(report),
        diagnostics: Vec::new(),
    })
}

/// 테스트 결과 상세 검증 (baseline 비교)
///
/// 결과값을 완전히 검증하고, 차이가 있는 항목을 반환합니다.
/// P/F가 목적이 아니라, 결과값 자체의 검증이 목적입니다.
fn validate_test_result_detailed(result: &TestResult, expected: &ExpectedResult) -> (bool, Vec<String>) {
    let mut errors = Vec::new();
    let tolerance = expected.tolerance;

    // 초기화 실패 케이스
    if expected.initialization == "failure" && result.success {
        errors.push("초기화가 성공했으나 실패 예상".to_string());
    }
    if expected.initialization == "success" && !result.success {
        errors.push("초기화가 실패했으나 성공 예상".to_string());
    }

    // ⚠️ 핵심 검증: 0 거래는 일반적으로 실패
    // baseline이 명시적으로 0을 지정하거나, min_trades가 0인 경우는 허용
    if result.trades_executed == 0 {
        let expected_zero = expected.trades_executed == Some(0);
        let min_trades_zero = expected.min_trades == Some(0);
        if !expected_zero && !min_trades_zero {
            errors.push("거래 0건 - 전략이 신호를 생성하지 않음".to_string());
        }
    }

    // baseline 비교: 거래 수 (명시적으로 설정된 경우)
    if let Some(expected_trades) = expected.trades_executed {
        if result.trades_executed != expected_trades {
            errors.push(format!(
                "거래 수 불일치: 예상 {} → 실제 {}",
                expected_trades, result.trades_executed
            ));
        }
    }

    let actual_return: f64 = result.total_return_pct.try_into().unwrap_or(0.0);
    let actual_win_rate: f64 = result.win_rate_pct.try_into().unwrap_or(0.0);

    // baseline 비교: 수익률
    if let Some(expected_return) = expected.total_return_pct {
        let diff = (actual_return - expected_return).abs();
        if diff > tolerance {
            errors.push(format!(
                "수익률 불일치: 예상 {:.2}% → 실제 {:.2}% (차이: {:.2}%)",
                expected_return, actual_return, diff
            ));
        }
    }

    // baseline 비교: 최대 낙폭
    if let Some(expected_dd) = expected.max_drawdown_pct {
        if let Some(ref report) = result.report {
            let actual_dd: f64 = report.metrics.max_drawdown_pct.try_into().unwrap_or(0.0);
            let diff = (actual_dd - expected_dd).abs();
            if diff > tolerance {
                errors.push(format!(
                    "최대낙폭 불일치: 예상 {:.2}% → 실제 {:.2}% (차이: {:.2}%)",
                    expected_dd, actual_dd, diff
                ));
            }
        }
    }

    // baseline 비교: 승률
    if let Some(expected_win_rate) = expected.win_rate_pct {
        let diff = (actual_win_rate - expected_win_rate).abs();
        if diff > tolerance {
            errors.push(format!(
                "승률 불일치: 예상 {:.1}% → 실제 {:.1}% (차이: {:.1}%)",
                expected_win_rate, actual_win_rate, diff
            ));
        }
    }

    // 하위 호환: 최소 거래 수
    if let Some(min_trades) = expected.min_trades {
        if result.trades_executed < min_trades {
            errors.push(format!(
                "최소 거래 수 미달: 최소 {} → 실제 {}",
                min_trades, result.trades_executed
            ));
        }
    }

    // 하위 호환: 최소 수익률
    if let Some(min_return) = expected.min_return_pct {
        if actual_return < min_return {
            errors.push(format!(
                "최소 수익률 미달: 최소 {:.2}% → 실제 {:.2}%",
                min_return, actual_return
            ));
        }
    }

    (errors.is_empty(), errors)
}

/// 테스트 결과 검증 (간단 버전 - 하위 호환)
#[allow(dead_code)]
fn validate_test_result(result: &TestResult, expected: &ExpectedResult) -> bool {
    // 초기화 성공/실패 확인
    if expected.initialization == "failure" && result.success {
        return false;
    }
    if expected.initialization == "success" && !result.success {
        return false;
    }

    // 최소 거래 수 확인
    if let Some(min_trades) = expected.min_trades {
        if result.trades_executed < min_trades {
            return false;
        }
    }

    // 최소 수익률 확인
    if let Some(min_return) = expected.min_return_pct {
        let return_pct: f64 = result.total_return_pct.try_into().unwrap_or(0.0);
        if return_pct < min_return {
            return false;
        }
    }

    true
}

/// 회귀 테스트 요약 출력
fn print_regression_summary(results: &[RegressionTestResult]) {
    let total_tests: usize = results.iter().map(|r| r.total_tests).sum();
    let total_passed: usize = results.iter().map(|r| r.passed).sum();
    let total_failed: usize = results.iter().map(|r| r.failed).sum();

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("📊 회귀 테스트 최종 결과");
    println!("═══════════════════════════════════════════════════════════════");
    println!("  총 테스트: {} 개", total_tests);
    println!("  ✅ 통과: {} 개", total_passed);
    println!("  ❌ 실패: {} 개", total_failed);
    println!("  통과율: {:.1}%", (total_passed as f64 / total_tests as f64) * 100.0);

    if total_failed > 0 {
        println!("\n⚠️  실패한 테스트:");
        for result in results {
            for test in &result.results {
                if !test.passed {
                    println!("  - {} ({}): {}",
                        test.strategy_name,
                        test.strategy_id,
                        test.error_message.as_deref().unwrap_or("알 수 없는 오류")
                    );
                }
            }
        }
    }

    println!("═══════════════════════════════════════════════════════════════\n");
}

/// 초기화 전용 회귀 테스트 (빠른 검증)
///
/// 전략 초기화만 테스트하여 빠르게 회귀 여부를 확인합니다.
/// 백테스트는 실행하지 않으므로 데이터베이스 연결이 필요 없습니다.
pub async fn run_init_only_regression_tests(fixtures_dir: &Path) -> Result<Vec<RegressionTestResult>> {
    let fixture_paths = discover_fixtures(fixtures_dir)?;

    if fixture_paths.is_empty() {
        return Err(anyhow!("Fixture 파일이 없습니다: {}", fixtures_dir.display()));
    }

    println!("\n🧪 초기화 전용 회귀 테스트 (빠른 검증)");
    println!("═══════════════════════════════════════════════════════════════");
    println!("  Fixture 디렉토리: {}", fixtures_dir.display());
    println!("  발견된 Fixture 파일: {} 개", fixture_paths.len());
    println!("═══════════════════════════════════════════════════════════════\n");

    let mut all_results = Vec::new();

    for fixture_path in fixture_paths {
        let fixture = load_fixture(&fixture_path)?;

        println!("\n📁 Fixture: {} ({})", fixture_path.file_name().unwrap().to_string_lossy(), fixture.description);
        println!("───────────────────────────────────────────────────────────────");

        let mut results = Vec::new();
        let mut passed = 0;
        let mut failed = 0;

        for strategy_fixture in &fixture.strategies {
            let test_passed = test_strategy_init_only(&strategy_fixture);

            if test_passed {
                passed += 1;
                println!("  ✅ {} ({})", strategy_fixture.name, strategy_fixture.strategy_id);
            } else {
                failed += 1;
                println!("  ❌ {} ({})", strategy_fixture.name, strategy_fixture.strategy_id);
            }

            results.push(SingleTestResult {
                strategy_id: strategy_fixture.strategy_id.clone(),
                strategy_name: strategy_fixture.name.clone(),
                passed: test_passed,
                error_message: if test_passed { None } else { Some("초기화 실패".to_string()) },
                test_result: None,
            });
        }

        all_results.push(RegressionTestResult {
            fixture_path: fixture_path.display().to_string(),
            total_tests: fixture.strategies.len(),
            passed,
            failed,
            results,
        });
    }

    print_regression_summary(&all_results);

    Ok(all_results)
}

/// 전략 초기화만 테스트 (DB 연결 없이)
fn test_strategy_init_only(fixture: &StrategyFixture) -> bool {
    // 전략 존재 여부 확인
    let available_strategies = StrategyRegistry::list_ids();
    if !available_strategies.contains(&fixture.strategy_id.as_str()) {
        // 존재하지 않는 전략인데 expected.initialization이 failure면 통과
        return fixture.expected.initialization == "failure";
    }

    // 전략 생성
    let strategy = match StrategyRegistry::create_instance(&fixture.strategy_id) {
        Ok(s) => s,
        Err(_) => return fixture.expected.initialization == "failure",
    };

    // 전략 이름/버전 확인
    let _ = strategy.name();
    let _ = strategy.version();

    // 기대 결과와 비교
    fixture.expected.initialization == "success"
}

#[cfg(test)]
mod tests {
    use super::*;
    use trader_core::Symbol;

    /// 테스트용 심볼 객체 생성
    fn create_symbol(ticker: &str, market: &Market) -> Symbol {
        match market {
            Market::KR => Symbol::kr_stock(ticker.to_uppercase(), "KRW"),
            Market::US => Symbol::us_stock(ticker.to_uppercase(), "USD"),
        }
    }

    #[test]
    fn test_default_config() {
        let config = StrategyTestConfig::default();
        assert_eq!(config.initial_capital, Decimal::from(10_000_000));
        assert!(matches!(config.market, Market::KR));
        assert!(config.symbols.is_empty());
    }

    #[test]
    fn test_create_symbol_kr() {
        let symbol = create_symbol("005930", &Market::KR);
        assert_eq!(symbol.base, "005930");
        assert_eq!(symbol.quote, "KRW");
    }

    #[test]
    fn test_create_symbol_us() {
        let symbol = create_symbol("SPY", &Market::US);
        assert_eq!(symbol.base, "SPY");
        assert_eq!(symbol.quote, "USD");
    }

    #[test]
    fn test_fixture_parsing() {
        let json = r#"{
            "description": "테스트",
            "strategies": [{
                "strategy_id": "rsi",
                "name": "RSI",
                "symbols": ["005930"],
                "market": "KR",
                "config": {"ticker": "005930"},
                "expected": {"initialization": "success"}
            }]
        }"#;

        let fixture: FixtureFile = serde_json::from_str(json).unwrap();
        assert_eq!(fixture.strategies.len(), 1);
        assert_eq!(fixture.strategies[0].strategy_id, "rsi");
    }
}
