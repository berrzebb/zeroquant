//! Standalone data collector CLI.

use clap::{Parser, Subcommand};
use sqlx::PgPool;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use trader_collector::{modules, CollectorConfig, CollectorError};
use trader_data::{Database, DatabaseConfig};

/// 데이터베이스 URL에서 민감정보(비밀번호) 마스킹.
/// 예: postgres://user:password@host:5432/db → postgres://user:****@host:5432/db
fn mask_database_url(url: &str) -> String {
    // URL 형식: scheme://user:password@host:port/database
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            // scheme://user: 까지 + **** + @host:port/database
            let prefix = &url[..colon_pos + 1];
            let suffix = &url[at_pos..];
            return format!("{}****{}", prefix, suffix);
        }
    }
    // 파싱 실패 시 전체 마스킹
    "****".to_string()
}

/// 그룹 A: 외부 API 워크플로우 (Rate Limited)
/// - 심볼 동기화, Fundamental(Naver/KRX/Yahoo), OHLCV
async fn run_external_api_workflow(pool: &PgPool, config: &CollectorConfig) {
    tracing::info!("[Group A] 외부 API 워크플로우 시작");

    // 1. 심볼 동기화
    match modules::sync_symbols(pool, config).await {
        Ok(stats) => stats.log_summary("[A] 심볼 동기화"),
        Err(e) => tracing::error!("[A] 심볼 동기화 실패: {}", e),
    }

    // 2. Fundamental 동기화 (PER, PBR, 섹터 등)
    // 우선순위: KRX API > 네이버 금융
    if config.providers.krx_api_enabled {
        match modules::sync_krx_fundamentals(pool, &config.fundamental_collect).await {
            Ok(stats) => tracing::info!(
                processed = stats.processed,
                valuation = stats.valuation_updated,
                sector = stats.sector_updated,
                "[A] KRX Fundamental 동기화 완료"
            ),
            Err(e) => tracing::error!("[A] KRX Fundamental 동기화 실패: {}", e),
        }
    } else if config.providers.naver_enabled {
        let naver_options = modules::NaverSyncOptions {
            request_delay_ms: config.providers.naver_request_delay_ms,
            batch_size: None,
            resume: false,
            stale_hours: Some(config.fundamental_collect.stale_days as u32 * 24),
            force: false,  // 기존 값 보존
            concurrent_limit: None,
        };
        match modules::sync_naver_fundamentals_with_options(pool, naver_options).await {
            Ok(stats) => tracing::info!(
                processed = stats.processed,
                valuation = stats.valuation_updated,
                sector = stats.sector_updated,
                "[A] 네이버 Fundamental 동기화 완료"
            ),
            Err(e) => tracing::error!("[A] 네이버 Fundamental 동기화 실패: {}", e),
        }
    }

    // 2-2. Yahoo Finance Fundamental 동기화 (US/글로벌 시장)
    if config.providers.yahoo_enabled {
        let yahoo_options = modules::YahooSyncOptions {
            request_delay_ms: config.fundamental_collect.request_delay_ms,
            batch_size: Some(100),
            market_filter: None,
            resume: false,
            stale_hours: Some(config.fundamental_collect.stale_days as u32 * 24),
            force: false,  // 기존 값 보존
        };
        match modules::sync_yahoo_fundamentals(pool, yahoo_options).await {
            Ok(stats) => tracing::info!(
                processed = stats.processed,
                valuation = stats.valuation_updated,
                market_cap = stats.market_cap_updated,
                "[A] Yahoo Fundamental 동기화 완료"
            ),
            Err(e) => tracing::error!("[A] Yahoo Fundamental 동기화 실패: {}", e),
        }
    }

    // 3. OHLCV 수집 - 데몬 모드에서는 24시간 증분 수집
    match modules::collect_ohlcv(pool, config, None, Some(24)).await {
        Ok(stats) => stats.log_summary("[A] OHLCV 수집"),
        Err(e) => tracing::error!("[A] OHLCV 수집 실패: {}", e),
    }

    tracing::info!("[Group A] 외부 API 워크플로우 완료");
}

/// 매크로 데이터 + Market Breadth 동기화 (Redis 캐시 사용)
/// - USD/KRW 환율, NASDAQ 지수
/// - Market Breadth (20일선 상회 비율)
/// 시장 시간과 무관하게 항상 최신 데이터 유지
async fn run_macro_data_sync(pool: &PgPool, redis_cache: &Option<std::sync::Arc<trader_data::cache::RedisCache>>) {
    if let Some(cache) = redis_cache {
        // 1. 매크로 데이터 동기화
        match modules::sync_macro_data(cache).await {
            Ok(result) => {
                if result.success {
                    tracing::debug!(
                        "[Macro] 동기화 완료: USD/KRW={}, NASDAQ={:+.2}%",
                        result.usd_krw.unwrap_or_default(),
                        result.nasdaq_change_pct.unwrap_or(0.0)
                    );
                } else {
                    tracing::warn!("[Macro] 동기화 실패: {}", result.error.unwrap_or_default());
                }
            }
            Err(e) => tracing::error!("[Macro] 동기화 오류: {}", e),
        }

        // 2. Market Breadth 동기화
        match modules::sync_market_breadth(pool, cache).await {
            Ok(result) => {
                if result.success {
                    tracing::debug!(
                        "[Breadth] 동기화 완료: all={}, kospi={}, kosdaq={}",
                        result.all_pct.unwrap_or_default(),
                        result.kospi_pct.unwrap_or_default(),
                        result.kosdaq_pct.unwrap_or_default()
                    );
                } else {
                    tracing::warn!("[Breadth] 동기화 실패: {}", result.error.unwrap_or_default());
                }
            }
            Err(e) => tracing::error!("[Breadth] 동기화 오류: {}", e),
        }
    } else {
        tracing::debug!("[Macro] Redis 캐시 없음, 건너뜀");
    }
}

/// 그룹 B: 내부 계산 파이프라인 (No Rate Limit, 5분 주기)
/// Indicator → GlobalScore → Screening View → Sector RS → Signal Performance
/// DB 데이터만 사용하므로 연속 실행 가능. 의존성 순서 보장.
async fn run_ranking_workflow(pool: &PgPool, config: &CollectorConfig) {
    tracing::debug!("[Group B] 계산 파이프라인 시작");

    // 1. 분석 지표 동기화 (RouteState, MarketRegime, TTM Squeeze)
    //    GlobalScore의 입력 데이터이므로 가장 먼저 실행
    let ind_options = modules::IndicatorSyncOptions {
        batch_size: Some(0), // 제한 없음: DB 계산 워크플로우
        ..Default::default()
    };
    match modules::sync_indicators_with_options(pool, config, None, ind_options).await {
        Ok(stats) => {
            if stats.success > 0 {
                stats.log_summary("[B] 지표 동기화");
            }
        }
        Err(e) => tracing::error!("[B] 지표 동기화 실패: {}", e),
    }

    // 2. GlobalScore 동기화 (Indicator 완료 후 즉시 실행)
    let gs_options = modules::GlobalScoreSyncOptions {
        batch_size: Some(0), // 제한 없음: DB 계산 워크플로우
        ..Default::default()
    };
    match modules::sync_global_scores_with_options(pool, config, None, gs_options).await {
        Ok(stats) => {
            if stats.success > 0 {
                stats.log_summary("[B] GlobalScore 동기화");
            }
        }
        Err(e) => tracing::error!("[B] GlobalScore 동기화 실패: {}", e),
    }

    // 3. 스크리닝 Materialized View 갱신 (GlobalScore 반영)
    match modules::refresh_screening_view(pool).await {
        Ok(stats) => {
            if stats.success > 0 {
                stats.log_summary("[B] 스크리닝 뷰 갱신");
            }
        }
        Err(e) => tracing::error!("[B] 스크리닝 뷰 갱신 실패: {}", e),
    }

    // 4. 섹터 RS Materialized View 갱신
    match modules::refresh_sector_rs_view(pool).await {
        Ok(stats) => {
            if stats.success > 0 {
                stats.log_summary("[B] 섹터 RS 뷰 갱신");
            }
        }
        Err(e) => tracing::error!("[B] 섹터 RS 뷰 갱신 실패: {}", e),
    }

    // 5. 신호 성과 동기화 (독립 계산)
    let signal_options = modules::SignalPerformanceSyncOptions::from(&config.signal_performance);
    match modules::sync_signal_performance(pool, signal_options).await {
        Ok(stats) => {
            if stats.success > 0 {
                stats.log_summary("[B] 신호 성과 동기화");
            }
        }
        Err(e) => tracing::error!("[B] 신호 성과 동기화 실패: {}", e),
    }

    tracing::debug!("[Group B] 계산 파이프라인 완료");
}

#[derive(Parser)]
#[command(name = "trader-collector")]
#[command(about = "ZeroQuant Standalone Data Collector", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// 로그 레벨 (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    /// 심볼 정보 동기화 (KRX, Binance, Yahoo)
    SyncSymbols,

    /// OHLCV 데이터 수집 (일봉)
    CollectOhlcv {
        /// 특정 심볼만 수집 (쉼표로 구분, 예: "005930,000660")
        #[arg(long)]
        symbols: Option<String>,

        /// 증분 수집: 이 시간(hours) 이전에 업데이트된 심볼만 수집
        /// 예: --stale-hours 24 (24시간 이상 지난 심볼만)
        #[arg(long)]
        stale_hours: Option<u32>,

        /// 이전 중단점부터 재개
        #[arg(long)]
        resume: bool,
    },

    /// 체크포인트 상태 조회/관리
    Checkpoint {
        #[command(subcommand)]
        action: CheckpointAction,
    },

    /// 분석 지표 동기화 (RouteState, MarketRegime, TTM Squeeze)
    SyncIndicators {
        /// 특정 심볼만 처리 (쉼표로 구분, 예: "005930,000660")
        #[arg(long)]
        symbols: Option<String>,

        /// 이전 중단점부터 재개
        #[arg(long)]
        resume: bool,

        /// N시간 이내 업데이트된 심볼 스킵
        #[arg(long)]
        stale_hours: Option<u32>,
    },

    /// GlobalScore 동기화 (랭킹용 종합 점수)
    SyncGlobalScores {
        /// 특정 심볼만 처리 (쉼표로 구분, 예: "005930,000660")
        #[arg(long)]
        symbols: Option<String>,

        /// 이전 중단점부터 재개
        #[arg(long)]
        resume: bool,

        /// N시간 이내 업데이트된 심볼 스킵
        #[arg(long)]
        stale_hours: Option<u32>,
    },

    /// KRX Fundamental 데이터 동기화 (PER, PBR, 배당수익률, 섹터 등)
    SyncKrxFundamentals,

    /// 네이버 금융 Fundamental 데이터 동기화 (KR 시장)
    /// KRX API 없이 네이버 크롤링으로 PER, PBR, ROE, 섹터, 시장타입 등 수집
    SyncNaverFundamentals {
        /// 배치당 처리할 심볼 수 (기본: 전체)
        #[arg(long)]
        batch_size: Option<i64>,

        /// 특정 심볼 하나만 처리 (테스트용)
        #[arg(long)]
        ticker: Option<String>,

        /// 이전 중단점부터 재개
        #[arg(long)]
        resume: bool,

        /// N시간 이내 업데이트된 심볼 스킵
        #[arg(long)]
        stale_hours: Option<u32>,
    },

    /// Yahoo Finance Fundamental 데이터 동기화 (US/글로벌 시장)
    /// PER, PBR, ROE, ROA, 영업이익률 등 수집
    SyncYahooFundamentals {
        /// 배치당 처리할 심볼 수 (기본: 전체)
        #[arg(long)]
        batch_size: Option<i64>,

        /// 특정 시장만 처리 (예: "US")
        #[arg(long)]
        market: Option<String>,

        /// 이전 중단점부터 재개
        #[arg(long)]
        resume: bool,

        /// N시간 이내 업데이트된 심볼 스킵
        #[arg(long)]
        stale_hours: Option<u32>,
    },

    /// 스크리닝 Materialized View 갱신
    /// symbol_info + fundamental + global_score 통합 뷰 갱신
    RefreshScreening,

    /// 신호 성과 동기화
    /// signal_marker의 신호에 대해 N일 후 수익률 계산
    SyncSignalPerformance {
        /// 최소 경과 일수 (기본: 1일)
        #[arg(long, default_value = "1")]
        min_days: u32,

        /// 최대 추적 일수 (기본: 20일)
        #[arg(long, default_value = "20")]
        max_days: u32,

        /// 이전 중단점부터 재개
        #[arg(long)]
        resume: bool,
    },

    /// 스케줄러 상태 확인
    SchedulerStatus {
        /// 시장 코드 (KR, US, JP)
        #[arg(long, default_value = "KR")]
        market: String,
    },

    /// 전체 워크플로우 실행 (심볼 → Fundamental → OHLCV → 지표 → GlobalScore → 스크리닝)
    RunAll {
        /// 특정 심볼만 처리 (테스트용, 예: "005930")
        #[arg(long)]
        ticker: Option<String>,
    },

    /// 데몬 모드: 주기적으로 전체 워크플로우 실행
    Daemon,
}

/// 체크포인트 관리 액션
#[derive(Subcommand)]
enum CheckpointAction {
    /// 모든 체크포인트 상태 조회
    List,

    /// 특정 워크플로우의 체크포인트 삭제
    Clear {
        /// 워크플로우 이름 (naver_fundamental, indicator_sync, global_score_sync)
        workflow: String,
    },

    /// 실행 중인 워크플로우를 interrupted 상태로 마킹
    Interrupt {
        /// 워크플로우 이름
        workflow: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // 로깅 초기화 (trader_collector, trader_data 모두 포함)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!(
                    "trader_collector={},trader_data={},trader_analytics={}",
                    cli.log_level, cli.log_level, cli.log_level
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("ZeroQuant Data Collector 시작");

    // 설정 로드
    let config = CollectorConfig::from_env()?;
    // 민감정보 마스킹 (비밀번호, 사용자명 숨김)
    let masked_url = mask_database_url(&config.database_url);
    tracing::debug!(database_url = %masked_url, "설정 로드 완료");

    // DB 연결 (중앙화된 풀 설정 사용)
    let db_config = DatabaseConfig::for_daemon(config.database_url.clone());
    let db = Database::connect(&db_config).await
        .map_err(|e| CollectorError::Config(format!("데이터베이스 연결 실패: {}", e)))?;
    let pool = db.pool().clone();

    // 명령 실행
    match cli.command {
        Commands::SyncSymbols => {
            let stats = modules::sync_symbols(&pool, &config).await?;
            stats.log_summary("심볼 동기화");
        }
        Commands::CollectOhlcv {
            symbols,
            stale_hours,
            resume,
        } => {
            if resume {
                tracing::info!("OHLCV resume 모드는 현재 stale_hours 옵션으로 대체 가능합니다");
            }
            let stats = modules::collect_ohlcv(&pool, &config, symbols, stale_hours).await?;
            stats.log_summary("OHLCV 수집");
        }
        Commands::Checkpoint { action } => match action {
            CheckpointAction::List => {
                let checkpoints = modules::list_checkpoints(&pool).await?;
                if checkpoints.is_empty() {
                    println!("저장된 체크포인트가 없습니다.");
                } else {
                    println!("\n📋 체크포인트 상태:");
                    println!("{:-<80}", "");
                    for cp in checkpoints {
                        println!(
                            "  {:<25} | 상태: {:<12} | 처리: {:>5}개 | 마지막: {}",
                            cp.workflow_name,
                            cp.status,
                            cp.total_processed,
                            cp.last_ticker.unwrap_or_else(|| "-".to_string())
                        );
                    }
                    println!("{:-<80}", "");
                }
            }
            CheckpointAction::Clear { workflow } => {
                modules::clear_checkpoint(&pool, &workflow).await?;
                println!("✅ {} 체크포인트 삭제 완료", workflow);
            }
            CheckpointAction::Interrupt { workflow } => {
                modules::mark_interrupted(&pool, &workflow).await?;
                println!("✅ {} 워크플로우를 interrupted 상태로 마킹", workflow);
            }
        },
        Commands::SyncIndicators {
            symbols,
            resume,
            stale_hours,
        } => {
            let options = modules::IndicatorSyncOptions {
                resume,
                stale_hours,
                batch_size: None, // CLI: config 기본값 사용
            };
            let stats =
                modules::sync_indicators_with_options(&pool, &config, symbols, options).await?;
            stats.log_summary("지표 동기화");
        }
        Commands::SyncGlobalScores {
            symbols,
            resume,
            stale_hours,
        } => {
            let options = modules::GlobalScoreSyncOptions {
                resume,
                stale_hours,
                batch_size: None, // CLI: config 기본값 사용
            };
            let stats =
                modules::sync_global_scores_with_options(&pool, &config, symbols, options).await?;
            stats.log_summary("GlobalScore 동기화");
        }
        Commands::SyncKrxFundamentals => {
            if !config.providers.krx_api_enabled {
                tracing::warn!("KRX API가 비활성화되어 있습니다. PROVIDER_KRX_API_ENABLED=true로 활성화하세요.");
                return Ok(());
            }
            let stats = modules::sync_krx_fundamentals(&pool, &config.fundamental_collect).await?;
            tracing::info!(
                processed = stats.processed,
                valuation = stats.valuation_updated,
                market_cap = stats.market_cap_updated,
                sector = stats.sector_updated,
                "KRX Fundamental 동기화 완료"
            );
        }
        Commands::SyncNaverFundamentals {
            batch_size,
            ticker,
            resume,
            stale_hours,
        } => {
            if !config.providers.naver_enabled {
                tracing::warn!("네이버 금융이 비활성화되어 있습니다. NAVER_FUNDAMENTAL_ENABLED=true로 활성화하세요.");
                return Ok(());
            }

            // 단일 종목 테스트 모드
            if let Some(t) = ticker {
                tracing::info!("단일 종목 테스트: {}", t);
                match modules::fetch_and_save_naver_fundamental(&pool, &t).await {
                    Ok(data) => {
                        println!("\n✅ 네이버 데이터 수집 완료: {}", t);
                        println!("  종목명: {:?}", data.name);
                        println!("  시장: {}", data.market_type);
                        println!("  섹터: {:?}", data.sector);
                        println!("  시가총액: {:?}", data.market_cap);
                        println!("  PER: {:?}", data.per);
                        println!("  PBR: {:?}", data.pbr);
                        println!("  ROE: {:?}", data.roe);
                        println!("  52주 고가: {:?}", data.week_52_high);
                        println!("  52주 저가: {:?}", data.week_52_low);
                    }
                    Err(e) => {
                        tracing::error!("네이버 데이터 수집 실패: {}", e);
                        return Err(e.into());
                    }
                }
            } else {
                // 배치 모드 (옵션 포함)
                let options = modules::NaverSyncOptions {
                    request_delay_ms: config.providers.naver_request_delay_ms,
                    batch_size,
                    resume,
                    stale_hours,
                    force: false,  // CLI에서는 기존 값 보존이 기본
                    concurrent_limit: None,
                };
                let stats = modules::sync_naver_fundamentals_with_options(&pool, options).await?;
                tracing::info!(
                    processed = stats.processed,
                    valuation = stats.valuation_updated,
                    market_cap = stats.market_cap_updated,
                    sector = stats.sector_updated,
                    week_52 = stats.week_52_updated,
                    market_type = stats.market_type_updated,
                    failed = stats.failed,
                    "네이버 Fundamental 동기화 완료"
                );
            }
        }
        Commands::SyncYahooFundamentals {
            batch_size,
            market,
            resume,
            stale_hours,
        } => {
            if !config.providers.yahoo_enabled {
                tracing::warn!("Yahoo Finance가 비활성화되어 있습니다. PROVIDER_YAHOO_ENABLED=true로 활성화하세요.");
                return Ok(());
            }

            let options = modules::YahooSyncOptions {
                request_delay_ms: config.fundamental_collect.request_delay_ms,
                batch_size,
                market_filter: market,
                resume,
                stale_hours,
                force: false,  // CLI에서는 기존 값 보존이 기본
            };
            let stats = modules::sync_yahoo_fundamentals(&pool, options).await?;
            tracing::info!(
                processed = stats.processed,
                valuation = stats.valuation_updated,
                market_cap = stats.market_cap_updated,
                failed = stats.failed,
                "Yahoo Fundamental 동기화 완료"
            );
        }
        Commands::RefreshScreening => {
            let stats = modules::refresh_screening_view(&pool).await?;
            stats.log_summary("스크리닝 뷰 갱신");

            // 통계 출력
            if let Ok(view_stats) = modules::get_screening_view_stats(&pool).await {
                println!("\n📊 스크리닝 뷰 통계:");
                println!("  총 레코드: {}", view_stats.total_rows);
                println!("  Global Score 있음: {}", view_stats.with_score);
                println!("  Fundamental 있음: {}", view_stats.with_fundamental);
                println!("  시장별:");
                for (market, count) in &view_stats.by_market {
                    println!("    {}: {}", market, count);
                }
            }
        }
        Commands::SyncSignalPerformance {
            min_days,
            max_days,
            resume,
        } => {
            let options = modules::SignalPerformanceSyncOptions {
                min_days_after: min_days,
                max_days,
                batch_size: config.signal_performance.batch_size,
                resume,
            };
            let stats = modules::sync_signal_performance(&pool, options).await?;
            stats.log_summary("신호 성과 동기화");
        }
        Commands::SchedulerStatus { market } => {
            let mut scheduler = modules::Scheduler::new(&config.scheduling);
            scheduler.load_kr_holidays_2025();
            scheduler.load_kr_holidays_2026();

            let now = chrono::Utc::now();
            let status = scheduler.get_market_status(&market, now);

            println!("\n📅 스케줄러 상태:");
            println!("{}", scheduler.status_summary(now));
            println!("\n시장 {} 상태: {:?}", market, status);

            if let Some(seconds) = scheduler.seconds_until_next_run(&market, now) {
                let hours = seconds / 3600;
                let mins = (seconds % 3600) / 60;
                println!("다음 실행까지: {}시간 {}분", hours, mins);
            }
        }
        Commands::RunAll { ticker } => {
            let is_single = ticker.is_some();
            let symbols_filter = ticker.clone();

            if is_single {
                tracing::info!(
                    "=== 단일 종목 워크플로우 시작: {} ===",
                    ticker.as_ref().unwrap()
                );
            } else {
                tracing::info!("=== 전체 워크플로우 시작 ===");
            }

            // 1. 심볼 동기화 (단일 종목 모드에서는 건너뜀)
            if !is_single {
                tracing::info!("Step 1/6: 심볼 동기화");
                let sync_stats = modules::sync_symbols(&pool, &config).await?;
                sync_stats.log_summary("심볼 동기화");
            } else {
                tracing::info!("Step 1/6: 심볼 동기화 (건너뜀 - 단일 종목 모드)");
            }

            // 2. Fundamental 동기화 (PER, PBR, 섹터 등)
            tracing::info!("Step 2/6: Fundamental 동기화");
            if let Some(ref t) = ticker {
                // 단일 종목: 네이버 금융으로 직접 수집
                if config.providers.naver_enabled {
                    match modules::fetch_and_save_naver_fundamental(&pool, t).await {
                        Ok(data) => {
                            println!("\n✅ 네이버 Fundamental 수집 완료: {}", t);
                            println!("  종목명: {:?}", data.name);
                            println!("  시장: {}", data.market_type);
                            println!("  섹터: {:?}", data.sector);
                            println!(
                                "  PER: {:?}, PBR: {:?}, ROE: {:?}",
                                data.per, data.pbr, data.roe
                            );
                        }
                        Err(e) => tracing::error!("네이버 Fundamental 수집 실패: {}", e),
                    }
                }
            } else if config.providers.krx_api_enabled {
                let krx_stats =
                    modules::sync_krx_fundamentals(&pool, &config.fundamental_collect).await?;
                tracing::info!(
                    processed = krx_stats.processed,
                    valuation = krx_stats.valuation_updated,
                    sector = krx_stats.sector_updated,
                    "KRX Fundamental 동기화 완료"
                );
            } else if config.providers.naver_enabled {
                // 24시간 이상 지난 데이터만 업데이트 (성장률 등 신규 필드 포함)
                let naver_options = modules::NaverSyncOptions {
                    request_delay_ms: config.providers.naver_request_delay_ms,
                    batch_size: None,
                    resume: false,
                    stale_hours: Some(24),
                    force: false,  // 기존 값 보존
                    concurrent_limit: None,
                };
                let naver_stats = modules::sync_naver_fundamentals_with_options(
                    &pool,
                    naver_options,
                )
                .await?;
                tracing::info!(
                    processed = naver_stats.processed,
                    valuation = naver_stats.valuation_updated,
                    sector = naver_stats.sector_updated,
                    "네이버 Fundamental 동기화 완료"
                );
            } else {
                tracing::info!("Fundamental 동기화 건너뜀 (KRX API, 네이버 모두 비활성화)");
            }

            // 3. OHLCV 수집 (지표도 함께 계산)
            tracing::info!("Step 3/6: OHLCV 수집");
            let ohlcv_stats =
                modules::collect_ohlcv(&pool, &config, symbols_filter.clone(), None).await?;
            ohlcv_stats.log_summary("OHLCV 수집");

            // 4. 분석 지표 동기화 (누락된 지표 보완)
            tracing::info!("Step 4/6: 분석 지표 동기화");
            let indicator_stats =
                modules::sync_indicators(&pool, &config, symbols_filter.clone()).await?;
            indicator_stats.log_summary("지표 동기화");

            // 5. GlobalScore 동기화 (랭킹용)
            tracing::info!("Step 5/6: GlobalScore 동기화");
            let global_score_stats =
                modules::sync_global_scores(&pool, &config, symbols_filter.clone()).await?;
            global_score_stats.log_summary("GlobalScore 동기화");

            // 6. 스크리닝 Materialized View 갱신
            tracing::info!("Step 6/6: 스크리닝 뷰 갱신");
            let screening_stats = modules::refresh_screening_view(&pool).await?;
            screening_stats.log_summary("스크리닝 뷰 갱신");

            if is_single {
                tracing::info!(
                    "=== 단일 종목 워크플로우 완료: {} ===",
                    ticker.as_ref().unwrap()
                );
            } else {
                tracing::info!("=== 전체 워크플로우 완료 ===");
            }
        }
        Commands::Daemon => {
            // Group B: 랭킹 워크플로우 주기 (분) - 기본값 15분
            let ranking_interval_minutes: u64 = std::env::var("RANKING_INTERVAL_MINUTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5); // 기본값 5분: DB 계산만 수행하므로 짧은 주기 가능

            // Redis 캐시 초기화 (매크로 데이터용)
            let redis_cache: Option<std::sync::Arc<trader_data::cache::RedisCache>> =
                if let Ok(redis_url) = std::env::var("REDIS_URL") {
                    let redis_config = trader_data::cache::RedisConfig {
                        url: redis_url,
                        default_ttl_secs: 300, // 5분
                        pool_size: 4,
                    };
                    match trader_data::cache::RedisCache::connect(&redis_config).await {
                        Ok(cache) => {
                            tracing::info!("Redis 캐시 연결 성공 (매크로 데이터용)");
                            Some(std::sync::Arc::new(cache))
                        }
                        Err(e) => {
                            tracing::warn!("Redis 캐시 연결 실패: {}", e);
                            None
                        }
                    }
                } else {
                    tracing::warn!("REDIS_URL 환경변수 없음, 매크로 데이터 캐싱 비활성화");
                    None
                };

            tracing::info!(
                "=== 데몬 모드 시작 ===\n  \
                 [Group A] 데이터 수집: {}분 (Symbol, Fundamental, OHLCV)\n  \
                 [Group B] 계산 파이프라인: {}분 (Indicator → GlobalScore → Screening → SignalPerf)\n  \
                 [Group C] 매크로+Breadth: 5분 (USD/KRW, KOSPI, NASDAQ, MarketBreadth)",
                config.daemon.interval_minutes,
                ranking_interval_minutes,
            );

            // 3개 그룹을 독립적으로 병렬 실행
            let pool_a = pool.clone();
            let pool_b = pool.clone();
            let config_a = config.clone();
            let config_b = config.clone();
            let pool_c = pool.clone();
            let redis_cache_c = redis_cache.clone();

            // 종료 시그널 공유
            let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
            let mut shutdown_rx_a = shutdown_tx.subscribe();
            let mut shutdown_rx_b = shutdown_tx.subscribe();
            let mut shutdown_rx_c = shutdown_tx.subscribe();
            // 워크플로우 실행 중에도 종료 신호 감지를 위해 sender를 각 그룹에 전달
            let shutdown_tx_a = shutdown_tx.clone();
            let shutdown_tx_b = shutdown_tx.clone();
            let shutdown_tx_c = shutdown_tx.clone();

            // Group A: 외부 API 워크플로우 (긴 주기)
            let group_a_handle = tokio::spawn(async move {
                // 첫 실행 — 종료 신호 감지 가능
                {
                    let mut first_shutdown = shutdown_tx_a.subscribe();
                    tokio::select! {
                        _ = run_external_api_workflow(&pool_a, &config_a) => {
                            tracing::info!(
                                "[Group A] 첫 실행 완료, 다음 실행: {}분 후",
                                config_a.daemon.interval_minutes
                            );
                        }
                        _ = first_shutdown.recv() => {
                            tracing::info!("[Group A] 첫 실행 중 종료 신호 수신");
                            return;
                        }
                    }
                }

                let mut interval = tokio::time::interval(config_a.daemon.interval());
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                interval.tick().await; // 첫 tick 즉시 반환 (소비)

                loop {
                    tokio::select! {
                        _ = shutdown_rx_a.recv() => {
                            tracing::info!("[Group A] 종료 신호 수신");
                            break;
                        }
                        _ = interval.tick() => {
                            // 워크플로우 실행 중에도 종료 신호 감지
                            let mut inner_shutdown = shutdown_tx_a.subscribe();
                            tokio::select! {
                                _ = run_external_api_workflow(&pool_a, &config_a) => {
                                    tracing::info!(
                                        "[Group A] 다음 실행: {}분 후",
                                        config_a.daemon.interval_minutes
                                    );
                                }
                                _ = inner_shutdown.recv() => {
                                    tracing::info!("[Group A] 워크플로우 실행 중 종료 신호 수신");
                                    break;
                                }
                            }
                        }
                    }
                }
            });

            // Group B: 랭킹 워크플로우 (Score, Screening, SignalPerf)
            let group_b_handle = tokio::spawn(async move {
                // 첫 실행 — 종료 신호 감지 가능
                {
                    let mut first_shutdown = shutdown_tx_b.subscribe();
                    tokio::select! {
                        _ = run_ranking_workflow(&pool_b, &config_b) => {
                            tracing::info!("[Group B] 첫 실행 완료, 다음 실행: {}분 후", ranking_interval_minutes);
                        }
                        _ = first_shutdown.recv() => {
                            tracing::info!("[Group B] 첫 실행 중 종료 신호 수신");
                            return;
                        }
                    }
                }

                let mut interval = tokio::time::interval(
                    std::time::Duration::from_secs(ranking_interval_minutes * 60)
                );
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                interval.tick().await; // 첫 tick 즉시 반환 (소비)

                loop {
                    tokio::select! {
                        _ = shutdown_rx_b.recv() => {
                            tracing::info!("[Group B] 종료 신호 수신");
                            break;
                        }
                        _ = interval.tick() => {
                            let mut inner_shutdown = shutdown_tx_b.subscribe();
                            tokio::select! {
                                _ = run_ranking_workflow(&pool_b, &config_b) => {
                                    tracing::debug!("[Group B] 다음 실행: {}분 후", ranking_interval_minutes);
                                }
                                _ = inner_shutdown.recv() => {
                                    tracing::info!("[Group B] 워크플로우 실행 중 종료 신호 수신");
                                    break;
                                }
                            }
                        }
                    }
                }
            });

            // Group C: 매크로 데이터 워크플로우 (경량 외부 API, 5분 주기)
            let group_c_handle = tokio::spawn(async move {
                // 첫 실행 — 종료 신호 감지 가능
                {
                    let mut first_shutdown = shutdown_tx_c.subscribe();
                    tokio::select! {
                        _ = run_macro_data_sync(&pool_c, &redis_cache_c) => {
                            tracing::info!("[Group C] 매크로+Breadth 첫 실행 완료, 다음 실행: 5분 후");
                        }
                        _ = first_shutdown.recv() => {
                            tracing::info!("[Group C] 첫 실행 중 종료 신호 수신");
                            return;
                        }
                    }
                }

                let mut interval = tokio::time::interval(
                    std::time::Duration::from_secs(5 * 60) // 5분 고정 주기
                );
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                interval.tick().await;

                loop {
                    tokio::select! {
                        _ = shutdown_rx_c.recv() => {
                            tracing::info!("[Group C] 종료 신호 수신");
                            break;
                        }
                        _ = interval.tick() => {
                            run_macro_data_sync(&pool_c, &redis_cache_c).await;
                            tracing::debug!("[Group C] 다음 실행: 5분 후");
                        }
                    }
                }
            });

            // Ctrl+C 대기 후 종료 시그널 전송
            tokio::signal::ctrl_c().await.ok();
            tracing::info!("종료 신호 수신, 데몬 종료 중...");
            let _ = shutdown_tx.send(());

            // 3개 그룹 종료 대기
            let _ = tokio::join!(group_a_handle, group_b_handle, group_c_handle);
        }
    }

    pool.close().await;
    tracing::info!("ZeroQuant Data Collector 종료");

    Ok(())
}
