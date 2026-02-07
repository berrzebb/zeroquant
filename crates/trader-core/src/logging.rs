//! tracing을 사용한 로깅 인프라.
//!
//! 이 모듈은 다양한 출력 형식을 지원하는 구조화된 로깅을 제공합니다:
//! - **pretty**: 개발용 사람이 읽기 쉬운 형식
//! - **json**: 운영환경/로그 집계용 JSON 형식
//! - **compact**: 로그 크기를 줄이기 위한 간결한 형식

use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

/// 로그 출력 형식.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFormat {
    /// 색상이 포함된 사람이 읽기 쉬운 형식 (개발용)
    #[default]
    Pretty,
    /// 로그 집계용 JSON 형식 (운영용)
    Json,
    /// 간결한 한 줄 형식
    Compact,
}

impl std::str::FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pretty" => Ok(Self::Pretty),
            "json" => Ok(Self::Json),
            "compact" => Ok(Self::Compact),
            _ => Err(format!("Unknown log format: {}", s)),
        }
    }
}

/// 로깅 설정.
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// 로그 레벨 필터 (예: "info", "debug", "trader_core=debug")
    pub level: String,
    /// 출력 형식
    pub format: LogFormat,
    /// span 이벤트 포함 여부 (진입/종료)
    pub with_span_events: bool,
    /// 파일명과 줄 번호 포함 여부
    pub with_file: bool,
    /// 스레드 ID 포함 여부
    pub with_thread_ids: bool,
    /// 대상(모듈 경로) 포함 여부
    pub with_target: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: LogFormat::Pretty,
            with_span_events: false,
            with_file: true,
            with_thread_ids: false,
            with_target: true,
        }
    }
}

impl LogConfig {
    /// 새 로그 설정을 생성합니다.
    pub fn new(level: impl Into<String>) -> Self {
        Self {
            level: level.into(),
            ..Default::default()
        }
    }

    /// 로그 형식을 설정합니다.
    pub fn with_format(mut self, format: LogFormat) -> Self {
        self.format = format;
        self
    }

    /// span 이벤트를 활성화합니다.
    pub fn with_span_events(mut self, enabled: bool) -> Self {
        self.with_span_events = enabled;
        self
    }

    /// 환경 변수에서 설정을 생성합니다.
    pub fn from_env() -> Self {
        let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        let format = std::env::var("LOG_FORMAT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(LogFormat::Pretty);

        Self {
            level,
            format,
            ..Default::default()
        }
    }
}

/// 주어진 설정으로 로깅 시스템을 초기화합니다.
///
/// # 예제
///
/// ```no_run
/// use trader_core::logging::{init_logging, LogConfig, LogFormat};
///
/// // 간단한 초기화
/// init_logging(LogConfig::default()).unwrap();
///
/// // 사용자 정의 설정으로 초기화
/// let config = LogConfig::new("debug")
///     .with_format(LogFormat::Json);
/// init_logging(config).unwrap();
/// ```
pub fn init_logging(config: LogConfig) -> Result<(), Box<dyn std::error::Error>> {
    let env_filter =
        EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new(&config.level))?;

    let span_events = if config.with_span_events {
        FmtSpan::NEW | FmtSpan::CLOSE
    } else {
        FmtSpan::NONE
    };

    match config.format {
        LogFormat::Pretty => {
            let fmt_layer = fmt::layer()
                .pretty()
                .with_file(config.with_file)
                .with_line_number(config.with_file)
                .with_thread_ids(config.with_thread_ids)
                .with_target(config.with_target)
                .with_span_events(span_events);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer)
                .try_init()?;
        }
        LogFormat::Json => {
            let fmt_layer = fmt::layer()
                .json()
                .with_file(config.with_file)
                .with_line_number(config.with_file)
                .with_thread_ids(config.with_thread_ids)
                .with_target(config.with_target)
                .with_span_events(span_events);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer)
                .try_init()?;
        }
        LogFormat::Compact => {
            let fmt_layer = fmt::layer()
                .compact()
                .with_file(config.with_file)
                .with_line_number(config.with_file)
                .with_thread_ids(config.with_thread_ids)
                .with_target(config.with_target)
                .with_span_events(span_events);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer)
                .try_init()?;
        }
    }

    tracing::info!(
        format = ?config.format,
        level = %config.level,
        "Logging initialized"
    );

    Ok(())
}

/// 환경 변수에서 로깅을 초기화합니다.
///
/// 레벨에는 `RUST_LOG`를, 형식에는 `LOG_FORMAT`을 사용합니다.
pub fn init_logging_from_env() -> Result<(), Box<dyn std::error::Error>> {
    init_logging(LogConfig::from_env())
}

/// 공통 트레이딩 컨텍스트 필드가 포함된 span을 생성하는 매크로.
#[macro_export]
macro_rules! trading_span {
    ($name:expr, $symbol:expr) => {
        tracing::info_span!($name, symbol = %$symbol)
    };
    ($name:expr, $symbol:expr, $strategy:expr) => {
        tracing::info_span!($name, symbol = %$symbol, strategy = %$strategy)
    };
    ($name:expr, $symbol:expr, $strategy:expr, $order_id:expr) => {
        tracing::info_span!(
            $name,
            symbol = %$symbol,
            strategy = %$strategy,
            order_id = %$order_id
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_format_from_str() {
        assert_eq!("pretty".parse::<LogFormat>().unwrap(), LogFormat::Pretty);
        assert_eq!("json".parse::<LogFormat>().unwrap(), LogFormat::Json);
        assert_eq!("compact".parse::<LogFormat>().unwrap(), LogFormat::Compact);
        assert_eq!("PRETTY".parse::<LogFormat>().unwrap(), LogFormat::Pretty);
        assert!("invalid".parse::<LogFormat>().is_err());
    }

    #[test]
    fn test_log_config_builder() {
        let config = LogConfig::new("debug")
            .with_format(LogFormat::Json)
            .with_span_events(true);

        assert_eq!(config.level, "debug");
        assert_eq!(config.format, LogFormat::Json);
        assert!(config.with_span_events);
    }
}
