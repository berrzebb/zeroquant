//! 설정 관리.
//!
//! 이 모듈은 애플리케이션 설정을 정의하고 관리합니다.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// 애플리케이션 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    /// 서버 설정
    pub server: ServerConfig,
    /// 데이터베이스 설정
    pub database: DatabaseConfig,
    /// Redis 설정
    pub redis: RedisConfig,
    /// 로깅 설정
    pub logging: LoggingConfig,
    /// 리스크 관리 설정
    pub risk: RiskConfig,
    /// 거래소 설정
    pub exchanges: HashMap<String, ExchangeConfig>,
    /// 데이터 관리 설정
    pub data: DataConfig,
    /// 전략 엔진 설정
    pub strategy: StrategyConfig,
    /// 알림 설정
    pub notifications: NotificationConfig,
}

/// 서버 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// 바인딩할 호스트
    pub host: String,
    /// 리스닝할 포트
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
        }
    }
}

/// 데이터베이스 설정 (TOML 직렬화용).
/// 풀 생성 시에는 `trader_data::DatabaseConfig`를 사용하세요.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    /// 최대 연결 수
    pub max_connections: u32,
    /// 최소 연결 수
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
    /// 연결 타임아웃 (초)
    pub connection_timeout_secs: u64,
    /// 유휴 타임아웃 (초)
    pub idle_timeout_secs: u64,
    /// 연결 최대 수명 (초) — 좀비 연결 방지
    #[serde(default = "default_max_lifetime_secs")]
    pub max_lifetime_secs: u64,
}

fn default_min_connections() -> u32 {
    2
}
fn default_max_lifetime_secs() -> u64 {
    1800
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: default_min_connections(),
            connection_timeout_secs: 30,
            idle_timeout_secs: 300,
            max_lifetime_secs: default_max_lifetime_secs(),
        }
    }
}

/// Redis 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RedisConfig {
    /// 최대 연결 수
    pub max_connections: u32,
    /// 연결 타임아웃 (초)
    pub connection_timeout_secs: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            max_connections: 5,
            connection_timeout_secs: 5,
        }
    }
}

/// 로깅 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// 로그 레벨
    pub level: String,
    /// 로그 형식 (pretty, json, compact)
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
        }
    }
}

/// 리스크 관리 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RiskConfig {
    /// 거래당 최대 포지션 크기 (계좌 대비 %)
    pub max_position_pct: Decimal,
    /// 최대 총 노출 (계좌 대비 %)
    pub max_total_exposure_pct: Decimal,
    /// 최대 일일 손실 (절대값)
    pub max_daily_loss: Decimal,
    /// 최대 일일 손실 (계좌 대비 %)
    pub max_daily_loss_pct: Decimal,
    /// 기본 손절 비율
    pub default_stop_loss_pct: Decimal,
    /// 기본 익절 비율
    pub default_take_profit_pct: Decimal,
    /// 최대 오픈 포지션 수
    pub max_open_positions: usize,
    /// 거래 일시중지를 위한 변동성 임계값
    pub volatility_threshold: Decimal,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_pct: Decimal::new(5, 0),
            max_total_exposure_pct: Decimal::new(70, 0),
            max_daily_loss: Decimal::new(100000, 0),
            max_daily_loss_pct: Decimal::new(3, 0),
            default_stop_loss_pct: Decimal::new(2, 0),
            default_take_profit_pct: Decimal::new(5, 0),
            max_open_positions: 10,
            volatility_threshold: Decimal::new(10, 0),
        }
    }
}

/// 거래소 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExchangeConfig {
    /// 이 거래소 활성화 여부
    pub enabled: bool,
    /// 거래소 이름
    pub name: String,
    /// 테스트넷/모의투자 사용
    #[serde(default)]
    pub testnet: bool,
    #[serde(default)]
    pub paper_trading: bool,
    /// REST API 기본 URL
    pub rest_base_url: String,
    /// REST API 테스트넷/모의투자 URL
    #[serde(default)]
    pub rest_testnet_url: Option<String>,
    #[serde(default)]
    pub rest_paper_url: Option<String>,
    /// WebSocket 기본 URL
    pub ws_base_url: String,
    /// WebSocket 테스트넷/모의투자 URL
    #[serde(default)]
    pub ws_testnet_url: Option<String>,
    #[serde(default)]
    pub ws_paper_url: Option<String>,
    /// 분당 요청 한도
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_minute: u32,
    /// 초당 주문 한도
    #[serde(default = "default_order_rate_limit")]
    pub order_rate_limit_per_second: u32,
    /// WebSocket 재연결 간격 (초)
    #[serde(default = "default_ws_reconnect")]
    pub ws_reconnect_interval_secs: u64,
    /// 최대 WebSocket 재연결 시도 횟수
    #[serde(default = "default_ws_max_reconnect")]
    pub ws_max_reconnect_attempts: u32,
}

fn default_rate_limit() -> u32 {
    1200
}
fn default_order_rate_limit() -> u32 {
    10
}
fn default_ws_reconnect() -> u64 {
    5
}
fn default_ws_max_reconnect() -> u32 {
    10
}

/// 데이터 관리 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataConfig {
    /// 수집할 기본 타임프레임
    pub default_timeframes: Vec<String>,
    /// 요청당 최대 캔들 수
    pub max_klines_per_request: usize,
    /// 시세 데이터 캐시 TTL (초)
    pub ticker_cache_ttl_secs: u64,
    /// 호가창 데이터 캐시 TTL (초)
    pub orderbook_cache_ttl_secs: u64,
    /// 캔들 데이터 캐시 TTL (초)
    pub kline_cache_ttl_secs: u64,
}

impl Default for DataConfig {
    fn default() -> Self {
        Self {
            default_timeframes: vec![
                "1m".to_string(),
                "5m".to_string(),
                "15m".to_string(),
                "1h".to_string(),
                "4h".to_string(),
                "1d".to_string(),
            ],
            max_klines_per_request: 1000,
            ticker_cache_ttl_secs: 1,
            orderbook_cache_ttl_secs: 1,
            kline_cache_ttl_secs: 60,
        }
    }
}

/// 전략 엔진 설정.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StrategyConfig {
    /// 플러그인 디렉토리 경로
    pub plugin_dir: String,
    /// 핫 리로드 활성화
    pub hot_reload: bool,
    /// 신호 처리 간격 (밀리초)
    pub signal_process_interval_ms: u64,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            plugin_dir: "./plugins".to_string(),
            hot_reload: true,
            signal_process_interval_ms: 100,
        }
    }
}

/// 알림 설정.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NotificationConfig {
    /// 알림 활성화 여부
    pub enabled: bool,
    /// 텔레그램 설정
    #[serde(default)]
    pub telegram: TelegramConfig,
    /// 디스코드 설정
    #[serde(default)]
    pub discord: DiscordConfig,
}

/// 텔레그램 알림 설정.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TelegramConfig {
    /// 활성화 여부
    pub enabled: bool,
    /// 봇 토큰
    #[serde(default)]
    pub bot_token: String,
    /// 채팅 ID
    #[serde(default)]
    pub chat_id: String,
}

/// 디스코드 알림 설정.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DiscordConfig {
    /// 활성화 여부
    pub enabled: bool,
    /// 웹훅 URL
    #[serde(default)]
    pub webhook_url: String,
}

impl AppConfig {
    /// 파일과 환경 변수에서 설정을 로드합니다.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, config::ConfigError> {
        let builder = config::Config::builder()
            // 기본값으로 시작
            .set_default("server.host", "127.0.0.1")?
            .set_default("server.port", 3000)?
            // 파일에서 로드
            .add_source(config::File::from(path.as_ref()))
            // 환경 변수로 오버라이드
            .add_source(
                config::Environment::with_prefix("TRADER")
                    .separator("__")
                    .try_parsing(true),
            );

        let config = builder.build()?;
        config.try_deserialize()
    }

    /// 기본 경로에서 설정을 로드합니다.
    pub fn load_default() -> Result<Self, config::ConfigError> {
        Self::load("config/default.toml")
    }
}
