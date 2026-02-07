//! 백테스트 실행 함수들
//!
//! 전략별 백테스트를 실행하고 결과를 변환하는 함수를 제공합니다.
//!
//! # SDUI 기반 동적 파라미터
//!
//! 전략 파라미터는 SDUI(Server-Driven UI)를 통해 동적으로 제공됩니다.
//! StrategyRegistry를 사용하여 전략 인스턴스를 생성하고,
//! SDUI에서 전달받은 params를 그대로 초기화에 사용합니다.
//!
//! # CPU-intensive 작업 처리
//!
//! 백테스트는 대량의 캔들 데이터를 처리하는 CPU-intensive 작업입니다.
//! Tokio async runtime의 worker thread를 블로킹하지 않도록
//! `tokio::task::spawn_blocking`을 사용하여 별도의 blocking thread pool에서 실행합니다.

use chrono::{NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use super::loader::parse_symbol;
use super::types::{
    BacktestConfigSummary, BacktestMetricsResponse, BacktestMultiRunResponse, BacktestRunResponse,
    EquityCurvePoint, TradeHistoryItem, TradeResultItem,
};

use trader_analytics::backtest::{BacktestConfig, BacktestEngine, BacktestReport};
use trader_core::{Kline, MarketType, StrategyContext, Symbol, Timeframe};
use trader_strategy::StrategyRegistry;

/// 전략별 백테스트 실행
///
/// CPU-intensive 백테스트 계산을 `spawn_blocking`으로 별도 thread pool에서 실행하여
/// Tokio async runtime의 worker thread를 블로킹하지 않습니다.
pub async fn run_strategy_backtest(
    strategy_id: &str,
    config: BacktestConfig,
    klines: &[Kline],
    params: &Option<serde_json::Value>,
) -> Result<BacktestReport, String> {
    // 데이터를 owned 타입으로 변환하여 spawn_blocking으로 이동
    let strategy_id = strategy_id.to_string();
    let klines = klines.to_vec();
    let params = params.clone();

    // CPU-intensive 작업을 blocking thread pool에서 실행
    let report = tokio::task::spawn_blocking(move || {
        // blocking 컨텍스트에서 async 코드를 실행하기 위해 새 runtime 생성
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Runtime 생성 실패: {}", e))?;

        rt.block_on(run_strategy_backtest_inner(
            &strategy_id,
            config,
            &klines,
            &params,
        ))
    })
    .await
    .map_err(|e| format!("백테스트 태스크 실행 실패: {}", e))??;

    Ok(report)
}

/// SDUI params에 ticker 주입
///
/// SDUI에서 ticker가 제공되지 않은 경우, klines에서 추출한 ticker를 주입합니다.
fn inject_ticker(params: Option<serde_json::Value>, ticker: &str) -> serde_json::Value {
    let mut config = params.unwrap_or(serde_json::json!({}));

    if let Some(obj) = config.as_object_mut() {
        // ticker가 없으면 주입
        if !obj.contains_key("ticker") {
            obj.insert("ticker".to_string(), serde_json::json!(ticker));
        }
        // amount가 없으면 기본값 주입
        if !obj.contains_key("amount") {
            obj.insert("amount".to_string(), serde_json::json!("100000"));
        }
    }

    config
}

/// 내부 백테스트 실행 함수 (sync 컨텍스트에서 호출됨)
///
/// StrategyRegistry를 사용하여 전략 인스턴스를 동적으로 생성합니다.
/// SDUI에서 제공된 params가 그대로 전략 초기화에 사용됩니다.
///
/// **중요**: run_with_context를 사용하여 StrategyContext가 제대로 업데이트되도록 합니다.
/// 이렇게 해야 전략이 RouteState, GlobalScore, StructuralFeatures 등의 지표를 활용할 수 있습니다.
async fn run_strategy_backtest_inner(
    strategy_id: &str,
    config: BacktestConfig,
    klines: &[Kline],
    params: &Option<serde_json::Value>,
) -> Result<BacktestReport, String> {
    let mut engine = BacktestEngine::new(config);

    // 심볼 추출 (klines에서)
    let symbol_str = if let Some(first_kline) = klines.first() {
        first_kline.ticker.to_string()
    } else {
        "BTC/USDT".to_string()
    };

    // StrategyRegistry에서 전략 인스턴스 생성
    let mut strategy = StrategyRegistry::create_instance(strategy_id)
        .map_err(|e| format!("전략 생성 실패: {}", e))?;

    // SDUI params에 ticker 주입 후 초기화
    let strategy_config = inject_ticker(params.clone(), &symbol_str);

    debug!(
        strategy_id = strategy_id,
        ticker = %symbol_str,
        config = ?strategy_config,
        "전략 초기화 (StrategyRegistry 기반)"
    );

    strategy
        .initialize(strategy_config)
        .await
        .map_err(|e| format!("전략 초기화 실패: {}", e))?;

    // StrategyContext 생성 및 초기 klines 설정
    let mut context = StrategyContext::default();
    context.update_klines(&symbol_str, Timeframe::D1, klines.to_vec());
    let context = Arc::new(RwLock::new(context));

    // **핵심**: 전략에 context 전달 (RouteState, GlobalScore 등 접근 가능하게)
    strategy.set_context(Arc::clone(&context));

    // run_with_context 사용: RouteState, GlobalScore, StructuralFeatures 등 지표 계산
    engine
        .run_with_context(&mut *strategy, klines, context, &symbol_str, None)
        .await
        .map_err(|e| e.to_string())
}

/// 다중 자산 전략 백테스트 실행
///
/// CPU-intensive 백테스트 계산을 `spawn_blocking`으로 별도 thread pool에서 실행하여
/// Tokio async runtime의 worker thread를 블로킹하지 않습니다.
pub async fn run_multi_strategy_backtest(
    strategy_id: &str,
    config: BacktestConfig,
    merged_klines: &[Kline],
    multi_klines: &HashMap<String, Vec<Kline>>,
    params: &Option<serde_json::Value>,
) -> Result<BacktestReport, String> {
    // 데이터를 owned 타입으로 변환하여 spawn_blocking으로 이동
    let strategy_id = strategy_id.to_string();
    let merged_klines = merged_klines.to_vec();
    let multi_klines = multi_klines.clone();
    let params = params.clone();

    // CPU-intensive 작업을 blocking thread pool에서 실행
    let report = tokio::task::spawn_blocking(move || {
        // blocking 컨텍스트에서 async 코드를 실행하기 위해 새 runtime 생성
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Runtime 생성 실패: {}", e))?;

        rt.block_on(run_multi_strategy_backtest_inner(
            &strategy_id,
            config,
            &merged_klines,
            &multi_klines,
            &params,
        ))
    })
    .await
    .map_err(|e| format!("다중 자산 백테스트 태스크 실행 실패: {}", e))??;

    Ok(report)
}

/// 다중 자산 전략용 파라미터 주입
///
/// SDUI params에 symbols와 initial_capital을 주입합니다.
fn inject_multi_asset_params(
    params: Option<serde_json::Value>,
    symbols: &[String],
    initial_capital: Decimal,
) -> serde_json::Value {
    let mut config = params.unwrap_or(serde_json::json!({}));

    if let Some(obj) = config.as_object_mut() {
        // 심볼 목록 주입
        if !obj.contains_key("symbols") {
            obj.insert(
                "symbols".to_string(),
                serde_json::Value::Array(
                    symbols
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect(),
                ),
            );
        }
        // 초기 자본금 주입
        if !obj.contains_key("initial_capital") {
            obj.insert(
                "initial_capital".to_string(),
                serde_json::Value::String(initial_capital.to_string()),
            );
        }
        // amount 기본값
        if !obj.contains_key("amount") {
            obj.insert("amount".to_string(), serde_json::json!("100000"));
        }
    }

    config
}

/// 내부 다중 자산 백테스트 실행 함수 (sync 컨텍스트에서 호출됨)
///
/// StrategyRegistry를 사용하여 전략 인스턴스를 동적으로 생성합니다.
/// SDUI에서 제공된 params가 그대로 전략 초기화에 사용됩니다.
///
/// **중요**: run_with_context를 사용하여 StrategyContext가 제대로 업데이트되도록 합니다.
/// 다중 자산 전략에서는 모든 심볼의 klines를 context에 로드합니다.
async fn run_multi_strategy_backtest_inner(
    strategy_id: &str,
    config: BacktestConfig,
    merged_klines: &[Kline],
    multi_klines: &HashMap<String, Vec<Kline>>,
    params: &Option<serde_json::Value>,
) -> Result<BacktestReport, String> {
    let initial_capital = config.initial_capital;
    let mut engine = BacktestEngine::new(config);

    // 심볼 목록 추출
    let symbols: Vec<String> = multi_klines.keys().cloned().collect();

    // StrategyRegistry에서 전략 인스턴스 생성
    let mut strategy = StrategyRegistry::create_instance(strategy_id)
        .map_err(|e| format!("전략 생성 실패: {}", e))?;

    // 다중 자산 전략용 params 주입
    let strategy_config = inject_multi_asset_params(params.clone(), &symbols, initial_capital);

    debug!(
        strategy_id = strategy_id,
        symbols = ?symbols,
        config = ?strategy_config,
        "다중 자산 전략 초기화 (StrategyRegistry 기반)"
    );

    strategy
        .initialize(strategy_config)
        .await
        .map_err(|e| format!("전략 초기화 실패: {}", e))?;

    // StrategyContext 생성 및 모든 심볼의 klines 설정
    let mut context = StrategyContext::default();
    for (symbol, klines) in multi_klines {
        context.update_klines(symbol, Timeframe::D1, klines.clone());
    }
    let context = Arc::new(RwLock::new(context));

    // **핵심**: 전략에 context 전달 (RouteState, GlobalScore 등 접근 가능하게)
    strategy.set_context(Arc::clone(&context));

    // 첫 번째 심볼을 기준 ticker로 사용
    let primary_ticker = symbols.first().map(|s| s.as_str()).unwrap_or("BTC/USDT");

    // run_with_context 사용: RouteState, GlobalScore, StructuralFeatures 등 지표 계산
    // multi_klines는 이미 StrategyContext에 등록됨 (위 코드 참조)
    engine
        .run_with_context(&mut *strategy, merged_klines, context, primary_ticker, None)
        .await
        .map_err(|e| e.to_string())
}

/// BacktestReport를 API 응답으로 변환
pub fn convert_report_to_response(
    report: &BacktestReport,
    strategy_id: &str,
    symbol: &str,
    start_date: &str,
    end_date: &str,
) -> BacktestRunResponse {
    let result_id = uuid::Uuid::new_v4().to_string();

    // 자산 곡선 변환
    let equity_curve: Vec<EquityCurvePoint> = report
        .equity_curve
        .iter()
        .map(|ep| EquityCurvePoint {
            timestamp: ep.timestamp.timestamp(),
            equity: ep.equity,
            drawdown_pct: ep.drawdown_pct,
        })
        .collect();

    // 거래 내역 변환
    let trades: Vec<TradeHistoryItem> = report
        .trades
        .iter()
        .map(|rt| TradeHistoryItem {
            symbol: rt.symbol.to_string(),
            entry_time: rt.entry_time,
            exit_time: rt.exit_time,
            entry_price: rt.entry_price,
            exit_price: rt.exit_price,
            quantity: rt.quantity,
            side: rt.side,
            pnl: rt.pnl,
            return_pct: rt.return_pct,
            entry_reason: rt.entry_reason.clone(),
            exit_reason: rt.exit_reason.clone(),
        })
        .collect();

    // 성과 지표 변환
    let metrics = BacktestMetricsResponse {
        total_return_pct: report.metrics.total_return_pct,
        annualized_return_pct: report.metrics.annualized_return_pct,
        net_profit: report.metrics.net_profit,
        total_trades: report.metrics.total_trades,
        win_rate_pct: report.metrics.win_rate_pct,
        profit_factor: report.metrics.profit_factor,
        sharpe_ratio: report.metrics.sharpe_ratio,
        sortino_ratio: report.metrics.sortino_ratio,
        max_drawdown_pct: report.metrics.max_drawdown_pct,
        calmar_ratio: report.metrics.calmar_ratio,
        avg_win: report.metrics.avg_win,
        avg_loss: report.metrics.avg_loss,
        largest_win: report.metrics.largest_win,
        largest_loss: report.metrics.largest_loss,
    };

    // 모든 거래 기록 변환 (매수/매도 개별 기록)
    // String 타입으로 변환하여 ts-rs 타입 생성 지원
    let all_trades: Vec<TradeResultItem> = report
        .all_trades
        .iter()
        .map(|tr| TradeResultItem {
            symbol: tr.symbol.clone(),
            side: format!("{:?}", tr.side).to_lowercase(), // Buy -> "buy", Sell -> "sell"
            signal_type: format!("{:?}", tr.signal_type).to_lowercase(), // Entry -> "entry"
            quantity: tr.quantity.to_string(),
            price: tr.price.to_string(),
            commission: tr.commission.to_string(),
            slippage: tr.slippage.to_string(),
            timestamp: tr.timestamp.to_rfc3339(),
            realized_pnl: tr.realized_pnl.map(|d| d.to_string()),
            is_partial: tr.is_partial,
            metadata: tr.metadata.clone(),
        })
        .collect();

    let config_summary = BacktestConfigSummary {
        initial_capital: report.config.initial_capital,
        commission_rate: report.config.commission_rate,
        slippage_rate: report.config.slippage_rate,
        total_commission: report.total_commission,
        total_slippage: report.total_slippage,
        data_points: report.data_points,
    };

    BacktestRunResponse {
        id: result_id,
        success: true,
        strategy_id: strategy_id.to_string(),
        symbol: symbol.to_string(),
        start_date: start_date.to_string(),
        end_date: end_date.to_string(),
        metrics,
        equity_curve,
        trades,
        config_summary,
        all_trades,
    }
}

/// 다중 자산 BacktestReport를 API 응답으로 변환
pub fn convert_multi_report_to_response(
    report: &BacktestReport,
    strategy_id: &str,
    symbols: &[String],
    start_date: &str,
    end_date: &str,
    data_points_by_symbol: HashMap<String, usize>,
) -> BacktestMultiRunResponse {
    let result_id = uuid::Uuid::new_v4().to_string();

    // 다중 자산 전략에서 같은 timestamp에 여러 equity 값이 기록될 수 있음
    // 같은 timestamp의 마지막 값만 유지하여 데이터 왜곡 방지
    let mut equity_map: BTreeMap<i64, EquityCurvePoint> = BTreeMap::new();
    for ep in &report.equity_curve {
        let ts = ep.timestamp.timestamp();
        // 같은 timestamp면 마지막 값(전체 포트폴리오 equity)으로 덮어씀
        equity_map.insert(
            ts,
            EquityCurvePoint {
                timestamp: ts,
                equity: ep.equity,
                drawdown_pct: ep.drawdown_pct,
            },
        );
    }
    let equity_curve: Vec<EquityCurvePoint> = equity_map.into_values().collect();

    let trades: Vec<TradeHistoryItem> = report
        .trades
        .iter()
        .map(|rt| TradeHistoryItem {
            symbol: rt.symbol.to_string(),
            entry_time: rt.entry_time,
            exit_time: rt.exit_time,
            entry_price: rt.entry_price,
            exit_price: rt.exit_price,
            quantity: rt.quantity,
            side: rt.side,
            pnl: rt.pnl,
            return_pct: rt.return_pct,
            entry_reason: rt.entry_reason.clone(),
            exit_reason: rt.exit_reason.clone(),
        })
        .collect();

    let metrics = BacktestMetricsResponse {
        total_return_pct: report.metrics.total_return_pct,
        annualized_return_pct: report.metrics.annualized_return_pct,
        net_profit: report.metrics.net_profit,
        total_trades: report.metrics.total_trades,
        win_rate_pct: report.metrics.win_rate_pct,
        profit_factor: report.metrics.profit_factor,
        sharpe_ratio: report.metrics.sharpe_ratio,
        sortino_ratio: report.metrics.sortino_ratio,
        max_drawdown_pct: report.metrics.max_drawdown_pct,
        calmar_ratio: report.metrics.calmar_ratio,
        avg_win: report.metrics.avg_win,
        avg_loss: report.metrics.avg_loss,
        largest_win: report.metrics.largest_win,
        largest_loss: report.metrics.largest_loss,
    };

    let config_summary = BacktestConfigSummary {
        initial_capital: report.config.initial_capital,
        commission_rate: report.config.commission_rate,
        slippage_rate: report.config.slippage_rate,
        total_commission: report.total_commission,
        total_slippage: report.total_slippage,
        data_points: report.data_points,
    };

    BacktestMultiRunResponse {
        id: result_id,
        success: true,
        strategy_id: strategy_id.to_string(),
        symbols: symbols.to_vec(),
        start_date: start_date.to_string(),
        end_date: end_date.to_string(),
        metrics,
        equity_curve,
        trades,
        config_summary,
        data_points_by_symbol,
    }
}

/// 다중 심볼 샘플 Kline 데이터 생성
pub fn generate_multi_sample_klines(
    symbols: &[String],
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> HashMap<String, Vec<Kline>> {
    use rust_decimal::prelude::FromPrimitive;

    let mut result = HashMap::new();
    let days = (end_date - start_date).num_days() as usize;

    // 심볼별 기본 가격 설정 (다양성을 위해)
    let base_prices: HashMap<&str, f64> = [
        ("TQQQ", 45.0),
        ("SCHD", 75.0),
        ("PFIX", 25.0),
        ("TMF", 8.0),
        ("SPY", 450.0),
        ("QQQ", 380.0),
        ("TLT", 95.0),
        ("IEF", 100.0),
        ("VEA", 45.0),
        ("VWO", 40.0),
        ("TIP", 110.0),
        ("BIL", 91.5),
        ("PDBC", 15.0),
        ("VNQ", 85.0),
        ("EFA", 72.0),
        ("EEM", 38.0),
        ("LQD", 108.0),
        ("BND", 72.0),
    ]
    .iter()
    .cloned()
    .collect();

    for symbol_str in symbols {
        let (base, quote) = parse_symbol(symbol_str);
        // Symbol 생성자를 통해 country 필드 자동 추론
        let symbol = Symbol::new(&base, &quote, MarketType::Stock);

        let base_price = *base_prices.get(base.as_str()).unwrap_or(&50.0);

        let klines: Vec<Kline> = (0..=days)
            .map(|i| {
                let date = start_date + chrono::Duration::days(i as i64);
                let open_time = Utc.from_utc_datetime(&date.and_hms_opt(9, 0, 0).unwrap());
                let close_time = Utc.from_utc_datetime(&date.and_hms_opt(15, 30, 0).unwrap());

                // 심볼별로 다른 변동성 패턴
                let volatility = match base.as_str() {
                    "TQQQ" | "TMF" => 0.04, // 레버리지 ETF: 높은 변동성
                    "BIL" => 0.001,         // 단기 채권: 매우 낮은 변동성
                    "TLT" | "IEF" => 0.015, // 채권 ETF: 중간 변동성
                    _ => 0.02,              // 일반 ETF
                };

                let noise = ((i as f64 * 0.7).sin() + (i as f64 * 1.3).cos()) * volatility;
                let trend = match base.as_str() {
                    "TQQQ" | "QQQ" | "SPY" => i as f64 * 0.0005, // 상승 추세
                    "TLT" | "TMF" => i as f64 * -0.0003,         // 하락 추세 (금리 상승)
                    _ => i as f64 * 0.0001,
                };
                let price_mult = 1.0 + noise + trend;

                let open = base_price * price_mult;
                let high = open * (1.0 + volatility * 0.5);
                let low = open * (1.0 - volatility * 0.5);
                let close = open * (1.0 + noise * 0.3);
                let volume = 1000000.0 * (1.0 + noise.abs());

                Kline {
                    ticker: symbol.to_string(),
                    timeframe: Timeframe::D1,
                    open_time,
                    close_time,
                    open: Decimal::from_f64(open).unwrap_or(Decimal::from(50)),
                    high: Decimal::from_f64(high).unwrap_or(Decimal::from(51)),
                    low: Decimal::from_f64(low).unwrap_or(Decimal::from(49)),
                    close: Decimal::from_f64(close).unwrap_or(Decimal::from(50)),
                    volume: Decimal::from_f64(volume).unwrap_or(Decimal::from(1000000)),
                    quote_volume: None,
                    num_trades: None,
                }
            })
            .collect();

        result.insert(symbol_str.clone(), klines);
    }

    result
}
