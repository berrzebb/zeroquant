//! 환경변수 기반 설정 모듈.

use crate::Result;
use std::time::Duration;

/// Collector 전체 설정
#[derive(Debug, Clone)]
pub struct CollectorConfig {
    /// 데이터베이스 URL
    pub database_url: String,
    /// 데이터 프로바이더 설정
    pub providers: DataProviderConfig,
    /// 심볼 동기화 설정
    pub symbol_sync: SymbolSyncConfig,
    /// OHLCV 수집 설정
    pub ohlcv_collect: OhlcvCollectConfig,
    /// Fundamental 수집 설정
    pub fundamental_collect: FundamentalCollectConfig,
    /// 데몬 모드 설정
    pub daemon: DaemonConfig,
    /// 스케줄링 설정
    pub scheduling: SchedulingConfig,
    /// 신호 성과 설정
    pub signal_performance: SignalPerformanceConfig,
    /// 관심종목 우선 처리 여부
    pub prioritize_watchlist: bool,
}

/// 데이터 프로바이더 설정
///
/// 각 프로바이더의 활성화 여부를 제어합니다.
/// KRX API는 사용 권한 신청 후 활성화하세요.
#[derive(Debug, Clone)]
pub struct DataProviderConfig {
    /// KRX API 활성화 (OHLCV, Fundamental)
    /// 기본값: false (승인 전까지 비활성화)
    pub krx_api_enabled: bool,
    /// Yahoo Finance 활성화 (OHLCV)
    /// 기본값: true
    pub yahoo_enabled: bool,
    /// 네이버 금융 크롤러 활성화 (KR 시장 Fundamental)
    /// 기본값: true (KR 시장 데이터 수집용)
    pub naver_enabled: bool,
    /// 네이버 요청 간 딜레이 (밀리초)
    /// 기본값: 300ms
    pub naver_request_delay_ms: u64,
}

/// 심볼 동기화 설정
#[derive(Debug, Clone)]
pub struct SymbolSyncConfig {
    /// 최소 심볼 수 (이 수 이하일 때만 동기화 실행)
    pub min_symbol_count: i64,
    /// KRX 동기화 활성화
    pub enable_krx: bool,
    /// Binance 동기화 활성화
    pub enable_binance: bool,
    /// Yahoo 동기화 활성화
    pub enable_yahoo: bool,
    /// Yahoo 최대 수집 종목 수
    pub yahoo_max_symbols: usize,
}

/// OHLCV 수집 설정
#[derive(Debug, Clone)]
pub struct OhlcvCollectConfig {
    /// 배치당 심볼 수
    pub batch_size: i64,
    /// 갱신 기준 일수 (마지막 수집 후 N일 경과 시 재수집)
    pub stale_days: i64,
    /// API 요청 간 딜레이 (밀리초)
    pub request_delay_ms: u64,
    /// 수집 시작 날짜 (YYYYMMDD)
    pub start_date: Option<String>,
    /// 수집 종료 날짜 (YYYYMMDD)
    pub end_date: Option<String>,
    /// 대상 시장 목록 (빈 경우 전체, 예: ["US", "KR"])
    pub target_markets: Vec<String>,
    /// 최대 보존 기간 (년), 이 기간 이전 데이터는 수집하지 않음
    pub max_retention_years: u32,
    /// 수집할 타임프레임 목록 (예: ["1d", "1w"])
    pub timeframes: Vec<String>,
    /// 동시 수집 심볼 수 (기본 5)
    pub concurrent_limit: Option<usize>,
    /// 비우선순위 종목의 최대 허용 갭 (일).
    /// 이 값을 초과하는 갭이 있는 종목은 우선순위 목록(watchlist/전략)에
    /// 포함되지 않은 경우 수집을 건너뜁니다.
    /// 0이면 제한 없음 (기존 동작). 기본값: 90일.
    pub max_gap_days_non_priority: i64,
}

/// Fundamental 및 지표 수집 설정
#[derive(Debug, Clone)]
pub struct FundamentalCollectConfig {
    /// 배치당 심볼 수
    pub batch_size: i64,
    /// 갱신 기준 일수 (기본: 1일 - 지표는 매일 갱신 필요)
    pub stale_days: i64,
    /// API 요청 간 딜레이 (밀리초)
    pub request_delay_ms: u64,
    /// OHLCV 데이터 함께 수집 여부
    pub include_ohlcv: bool,
}

/// 데몬 모드 설정
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// 워크플로우 실행 주기 (분 단위)
    pub interval_minutes: u64,
}

/// 스케줄링 설정 (시장 운영 시간 기반)
#[derive(Debug, Clone)]
pub struct SchedulingConfig {
    /// 스케줄링 활성화 여부
    pub enabled: bool,
    /// KRX 장 마감 후 대기 시간 (분)
    /// 기본: 60분 (15:30 마감 + 60분 = 16:30부터 수집)
    pub krx_delay_after_close_minutes: u32,
    /// 주말 건너뛰기
    pub skip_weekends: bool,
    /// 공휴일 건너뛰기
    pub skip_holidays: bool,
}

/// 신호 성과 계산 설정
#[derive(Debug, Clone)]
pub struct SignalPerformanceConfig {
    /// 배치당 처리할 신호 수
    pub batch_size: usize,
    /// 최소 경과 일수 (신호 발생 후 N일 경과해야 계산)
    pub min_days_after: u32,
    /// 최대 추적 일수 (N일까지 성과 계산)
    pub max_days: u32,
}

impl CollectorConfig {
    /// 환경변수에서 설정 로드
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let database_url = std::env::var("DATABASE_URL").map_err(|_| {
            crate::error::CollectorError::Config(
                "DATABASE_URL 환경변수가 설정되지 않았습니다".to_string(),
            )
        })?;

        Ok(Self {
            database_url,
            providers: DataProviderConfig {
                // KRX API: 기본 비활성화 (승인 후 true로 변경)
                krx_api_enabled: env_var_bool("PROVIDER_KRX_API_ENABLED", false),
                // Yahoo Finance: 기본 활성화
                yahoo_enabled: env_var_bool("PROVIDER_YAHOO_ENABLED", true),
                // 네이버 금융: KR 시장 fundamental 수집용
                naver_enabled: env_var_bool("NAVER_FUNDAMENTAL_ENABLED", true),
                naver_request_delay_ms: env_var_parse("NAVER_REQUEST_DELAY_MS", 300),
            },
            symbol_sync: SymbolSyncConfig {
                min_symbol_count: env_var_parse("SYMBOL_SYNC_MIN_COUNT", 100),
                enable_krx: env_var_bool("SYMBOL_SYNC_KRX", true),
                enable_binance: env_var_bool("SYMBOL_SYNC_BINANCE", false),
                enable_yahoo: env_var_bool("SYMBOL_SYNC_YAHOO", true),
                yahoo_max_symbols: env_var_parse("SYMBOL_SYNC_YAHOO_MAX", 500),
            },
            ohlcv_collect: OhlcvCollectConfig {
                batch_size: env_var_parse("OHLCV_BATCH_SIZE", 50),
                stale_days: env_var_parse("OHLCV_STALE_DAYS", 1),
                request_delay_ms: env_var_parse("OHLCV_REQUEST_DELAY_MS", 500),
                start_date: std::env::var("OHLCV_START_DATE").ok(),
                end_date: std::env::var("OHLCV_END_DATE").ok(),
                target_markets: env_var_list("OHLCV_TARGET_MARKETS"),
                max_retention_years: env_var_parse("OHLCV_MAX_RETENTION_YEARS", 3),
                timeframes: env_var_list_or_default("OHLCV_TIMEFRAMES", vec!["1d".to_string()]),
                concurrent_limit: std::env::var("OHLCV_CONCURRENT_LIMIT").ok().and_then(|v| v.parse().ok()),
                max_gap_days_non_priority: env_var_parse("OHLCV_MAX_GAP_DAYS_NON_PRIORITY", 90),
            },
            fundamental_collect: FundamentalCollectConfig {
                batch_size: env_var_parse("FUNDAMENTAL_BATCH_SIZE", 100),
                // FUNDAMENTAL_STALE_DAYS 우선, INDICATOR_STALE_DAYS 폴백 (하위 호환)
                stale_days: std::env::var("FUNDAMENTAL_STALE_DAYS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| env_var_parse("INDICATOR_STALE_DAYS", 7)),
                request_delay_ms: env_var_parse("FUNDAMENTAL_REQUEST_DELAY_MS", 50),
                include_ohlcv: env_var_bool("FUNDAMENTAL_INCLUDE_OHLCV", true),
            },
            daemon: DaemonConfig {
                interval_minutes: env_var_parse("DAEMON_INTERVAL_MINUTES", 60),
            },
            scheduling: SchedulingConfig {
                enabled: env_var_bool("SCHEDULING_ENABLED", false),
                krx_delay_after_close_minutes: env_var_parse("SCHEDULING_KRX_DELAY_MINUTES", 60),
                skip_weekends: env_var_bool("SCHEDULING_SKIP_WEEKENDS", true),
                skip_holidays: env_var_bool("SCHEDULING_SKIP_HOLIDAYS", true),
            },
            signal_performance: SignalPerformanceConfig {
                batch_size: env_var_parse("SIGNAL_PERFORMANCE_BATCH_SIZE", 100),
                min_days_after: env_var_parse("SIGNAL_PERFORMANCE_MIN_DAYS", 1),
                max_days: env_var_parse("SIGNAL_PERFORMANCE_MAX_DAYS", 20),
            },
            prioritize_watchlist: env_var_bool("PRIORITIZE_WATCHLIST", true),
        })
    }
}

impl OhlcvCollectConfig {
    /// API 요청 간 딜레이를 Duration으로 반환
    pub fn request_delay(&self) -> Duration {
        Duration::from_millis(self.request_delay_ms)
    }
}

impl FundamentalCollectConfig {
    /// API 요청 간 딜레이를 Duration으로 반환
    pub fn request_delay(&self) -> Duration {
        Duration::from_millis(self.request_delay_ms)
    }
}

impl DaemonConfig {
    /// 워크플로우 실행 주기를 Duration으로 반환
    pub fn interval(&self) -> Duration {
        Duration::from_secs(self.interval_minutes * 60)
    }
}

/// 환경변수에서 값을 파싱 (실패 시 기본값 사용)
fn env_var_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// 환경변수에서 bool 값 파싱
fn env_var_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .map(|v| v == "true" || v == "1")
        .unwrap_or(default)
}

/// 환경변수에서 쉼표로 구분된 리스트 파싱
fn env_var_list(key: &str) -> Vec<String> {
    std::env::var(key)
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_uppercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// 환경변수에서 리스트 파싱 (기본값 지원)
fn env_var_list_or_default(key: &str, default: Vec<String>) -> Vec<String> {
    std::env::var(key)
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or(default)
}
