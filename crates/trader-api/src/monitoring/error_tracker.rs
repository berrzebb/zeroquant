//! 에러 추적 및 모니터링 모듈.
//!
//! AI 디버깅을 위한 구조화된 에러 로그 수집 및 조회 기능을 제공합니다.
//! - 에러 발생 시 상세 컨텍스트 저장
//! - 최근 에러 히스토리 보관 (메모리 기반)
//! - 에러 유형별 집계
//! - Critical 에러 발생 시 Telegram 알림

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use tracing::{error, warn};

/// 에러 심각도 수준.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorSeverity {
    /// 경미한 에러 (기능 일부 영향)
    Warning,
    /// 일반 에러 (기능 실패)
    Error,
    /// 심각한 에러 (시스템 영향)
    Critical,
}

impl std::fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// 에러 카테고리.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// 데이터베이스 관련
    Database,
    /// 외부 API 호출 (Yahoo Finance, KIS 등)
    ExternalApi,
    /// 데이터 변환/파싱
    DataConversion,
    /// 인증/권한
    Authentication,
    /// 네트워크
    Network,
    /// 비즈니스 로직
    BusinessLogic,
    /// 시스템/인프라
    System,
    /// 기타
    Other,
}

impl std::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Database => write!(f, "database"),
            Self::ExternalApi => write!(f, "external_api"),
            Self::DataConversion => write!(f, "data_conversion"),
            Self::Authentication => write!(f, "authentication"),
            Self::Network => write!(f, "network"),
            Self::BusinessLogic => write!(f, "business_logic"),
            Self::System => write!(f, "system"),
            Self::Other => write!(f, "other"),
        }
    }
}

/// 소스 코드 위치 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    /// 파일 경로
    pub file: String,
    /// 라인 번호
    pub line: u32,
    /// 컬럼 번호
    pub column: u32,
    /// 함수/모듈 이름 (수동 입력)
    pub function: Option<String>,
}

impl SourceLocation {
    /// std::panic::Location에서 생성.
    #[track_caller]
    pub fn capture() -> Self {
        let loc = std::panic::Location::caller();
        Self {
            file: loc.file().to_string(),
            line: loc.line(),
            column: loc.column(),
            function: None,
        }
    }

    /// 함수명 포함하여 생성.
    #[track_caller]
    pub fn capture_with_function(function: impl Into<String>) -> Self {
        let loc = std::panic::Location::caller();
        Self {
            file: loc.file().to_string(),
            line: loc.line(),
            column: loc.column(),
            function: Some(function.into()),
        }
    }

    /// 수동 생성.
    pub fn manual(file: impl Into<String>, line: u32, function: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            line,
            column: 0,
            function: Some(function.into()),
        }
    }
}

impl std::fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref func) = self.function {
            write!(f, "{}:{}:{} in {}", self.file, self.line, self.column, func)
        } else {
            write!(f, "{}:{}:{}", self.file, self.line, self.column)
        }
    }
}

/// 구조화된 에러 레코드.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    /// 에러 ID (순차 증가)
    pub id: u64,
    /// 발생 시간
    pub timestamp: DateTime<Utc>,
    /// 심각도
    pub severity: ErrorSeverity,
    /// 카테고리
    pub category: ErrorCategory,
    /// 에러 메시지
    pub message: String,
    /// 발생 위치 (소스 코드)
    pub source_location: SourceLocation,
    /// 발생 위치 (레거시 - 문자열)
    pub location: String,
    /// 관련 엔티티 (티커, 주문ID 등)
    pub entity: Option<String>,
    /// 상세 컨텍스트 (키-값 쌍)
    pub context: HashMap<String, String>,
    /// 원본 에러 문자열
    pub raw_error: Option<String>,
    /// 스택 트레이스 (선택적)
    pub backtrace: Option<String>,
}

/// 에러 집계 통계.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorStats {
    /// 심각도별 에러 수
    pub by_severity: HashMap<String, u64>,
    /// 카테고리별 에러 수
    pub by_category: HashMap<String, u64>,
    /// 총 에러 수
    pub total_count: u64,
    /// 마지막 에러 시간
    pub last_error_at: Option<DateTime<Utc>>,
    /// 통계 시작 시간
    pub stats_since: DateTime<Utc>,
}

/// 에러 추적기 설정.
#[derive(Debug, Clone)]
pub struct ErrorTrackerConfig {
    /// 보관할 최대 에러 수
    pub max_history_size: usize,
    /// Critical 에러 시 Telegram 알림 활성화
    pub telegram_alert_enabled: bool,
}

impl Default for ErrorTrackerConfig {
    fn default() -> Self {
        Self {
            max_history_size: 1000,
            telegram_alert_enabled: true,
        }
    }
}

/// 에러 추적기 (스레드 안전).
#[derive(Clone)]
pub struct ErrorTracker {
    inner: Arc<RwLock<ErrorTrackerInner>>,
    config: ErrorTrackerConfig,
}

struct ErrorTrackerInner {
    /// 에러 히스토리 (최근 N개)
    history: VecDeque<ErrorRecord>,
    /// 에러 ID 카운터
    next_id: u64,
    /// 집계 통계
    stats: ErrorStats,
}

impl ErrorTracker {
    /// 새 에러 추적기 생성.
    pub fn new(config: ErrorTrackerConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ErrorTrackerInner {
                history: VecDeque::with_capacity(config.max_history_size),
                next_id: 1,
                stats: ErrorStats {
                    stats_since: Utc::now(),
                    ..Default::default()
                },
            })),
            config,
        }
    }

    /// 기본 설정으로 생성.
    pub fn with_defaults() -> Self {
        Self::new(ErrorTrackerConfig::default())
    }

    /// 에러 기록.
    pub fn record(&self, record: ErrorRecord) -> u64 {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                error!("ErrorTracker RwLock poisoned (write), recovering");
                poisoned.into_inner()
            }
        };

        // ID 할당
        let id = inner.next_id;
        inner.next_id += 1;

        let mut record = record;
        record.id = id;
        record.timestamp = Utc::now();

        // 통계 업데이트
        *inner
            .stats
            .by_severity
            .entry(record.severity.to_string())
            .or_insert(0) += 1;
        *inner
            .stats
            .by_category
            .entry(record.category.to_string())
            .or_insert(0) += 1;
        inner.stats.total_count += 1;
        inner.stats.last_error_at = Some(record.timestamp);

        // 로그 출력
        match record.severity {
            ErrorSeverity::Critical => {
                error!(
                    id = id,
                    category = %record.category,
                    location = %record.location,
                    entity = ?record.entity,
                    message = %record.message,
                    context = ?record.context,
                    "[CRITICAL ERROR]"
                );
            }
            ErrorSeverity::Error => {
                error!(
                    id = id,
                    category = %record.category,
                    location = %record.location,
                    entity = ?record.entity,
                    message = %record.message,
                    "[ERROR]"
                );
            }
            ErrorSeverity::Warning => {
                warn!(
                    id = id,
                    category = %record.category,
                    location = %record.location,
                    entity = ?record.entity,
                    message = %record.message,
                    "[WARNING]"
                );
            }
        }

        // 히스토리에 추가 (최대 크기 초과 시 가장 오래된 것 제거)
        if inner.history.len() >= self.config.max_history_size {
            inner.history.pop_front();
        }
        inner.history.push_back(record);

        id
    }

    /// 최근 에러 조회.
    pub fn get_recent(&self, limit: usize) -> Vec<ErrorRecord> {
        let inner = match self.inner.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                error!("ErrorTracker RwLock poisoned (read), recovering");
                poisoned.into_inner()
            }
        };
        inner.history.iter().rev().take(limit).cloned().collect()
    }

    /// 심각도별 에러 조회.
    pub fn get_by_severity(&self, severity: ErrorSeverity, limit: usize) -> Vec<ErrorRecord> {
        let inner = match self.inner.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                error!("ErrorTracker RwLock poisoned (read), recovering");
                poisoned.into_inner()
            }
        };
        inner
            .history
            .iter()
            .rev()
            .filter(|r| r.severity == severity)
            .take(limit)
            .cloned()
            .collect()
    }

    /// 카테고리별 에러 조회.
    pub fn get_by_category(&self, category: &ErrorCategory, limit: usize) -> Vec<ErrorRecord> {
        let inner = match self.inner.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                error!("ErrorTracker RwLock poisoned (read), recovering");
                poisoned.into_inner()
            }
        };
        inner
            .history
            .iter()
            .rev()
            .filter(|r| &r.category == category)
            .take(limit)
            .cloned()
            .collect()
    }

    /// 에러 통계 조회.
    pub fn get_stats(&self) -> ErrorStats {
        let inner = match self.inner.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                error!("ErrorTracker RwLock poisoned (read), recovering");
                poisoned.into_inner()
            }
        };
        inner.stats.clone()
    }

    /// 통계 초기화.
    pub fn reset_stats(&self) {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                error!("ErrorTracker RwLock poisoned (write), recovering");
                poisoned.into_inner()
            }
        };
        inner.stats = ErrorStats {
            stats_since: Utc::now(),
            ..Default::default()
        };
    }

    /// 히스토리 전체 삭제.
    pub fn clear_history(&self) {
        let mut inner = match self.inner.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                error!("ErrorTracker RwLock poisoned (write), recovering");
                poisoned.into_inner()
            }
        };
        inner.history.clear();
    }
}

/// 빌더 패턴으로 에러 레코드 생성.
#[derive(Debug)]
pub struct ErrorRecordBuilder {
    severity: Option<ErrorSeverity>,
    category: Option<ErrorCategory>,
    message: String,
    source_location: SourceLocation,
    location: String,
    entity: Option<String>,
    context: HashMap<String, String>,
    raw_error: Option<String>,
    backtrace: Option<String>,
}

impl ErrorRecordBuilder {
    /// 새 빌더 생성 (호출 위치 자동 캡처).
    #[track_caller]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source_location: SourceLocation::capture(),
            severity: None,
            category: None,
            location: String::new(),
            entity: None,
            context: HashMap::new(),
            raw_error: None,
            backtrace: None,
        }
    }

    /// 심각도 설정.
    pub fn severity(mut self, severity: ErrorSeverity) -> Self {
        self.severity = Some(severity);
        self
    }

    /// 카테고리 설정.
    pub fn category(mut self, category: ErrorCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// 함수명 설정 (소스 위치에 추가).
    pub fn function(mut self, function: impl Into<String>) -> Self {
        self.source_location.function = Some(function.into());
        self
    }

    /// 발생 위치 설정 (레거시 문자열).
    pub fn location(mut self, location: impl Into<String>) -> Self {
        self.location = location.into();
        self
    }

    /// 관련 엔티티 설정 (티커, 주문ID 등).
    pub fn entity(mut self, entity: impl Into<String>) -> Self {
        self.entity = Some(entity.into());
        self
    }

    /// 컨텍스트 추가 (문자열).
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// 컨텍스트 추가 (Decimal).
    pub fn with_decimal(mut self, key: impl Into<String>, value: Option<Decimal>) -> Self {
        let val_str = value
            .map(|d| d.to_string())
            .unwrap_or_else(|| "None".to_string());
        self.context.insert(key.into(), val_str);
        self
    }

    /// 컨텍스트 추가 (i64).
    pub fn with_i64(mut self, key: impl Into<String>, value: Option<i64>) -> Self {
        let val_str = value
            .map(|v| v.to_string())
            .unwrap_or_else(|| "None".to_string());
        self.context.insert(key.into(), val_str);
        self
    }

    /// 원본 에러 설정.
    pub fn raw_error(mut self, error: impl std::fmt::Display) -> Self {
        self.raw_error = Some(error.to_string());
        self
    }

    /// 스택 트레이스 캡처.
    pub fn capture_backtrace(mut self) -> Self {
        self.backtrace = Some(std::backtrace::Backtrace::force_capture().to_string());
        self
    }

    /// 에러 레코드 빌드.
    pub fn build(self) -> ErrorRecord {
        ErrorRecord {
            id: 0, // 추적기에서 할당
            timestamp: Utc::now(),
            severity: self.severity.unwrap_or(ErrorSeverity::Error),
            category: self.category.unwrap_or(ErrorCategory::Other),
            message: self.message,
            source_location: self.source_location,
            location: self.location,
            entity: self.entity,
            context: self.context,
            raw_error: self.raw_error,
            backtrace: self.backtrace,
        }
    }
}

/// 전역 에러 추적기 (싱글톤).
static GLOBAL_TRACKER: once_cell::sync::OnceCell<ErrorTracker> = once_cell::sync::OnceCell::new();

/// 전역 에러 추적기 초기화.
pub fn init_global_tracker(config: ErrorTrackerConfig) {
    let _ = GLOBAL_TRACKER.set(ErrorTracker::new(config));
}

/// 전역 에러 추적기 가져오기.
pub fn global_tracker() -> &'static ErrorTracker {
    GLOBAL_TRACKER.get_or_init(ErrorTracker::with_defaults)
}

/// 에러 기록 매크로 (간편 사용).
#[macro_export]
macro_rules! track_error {
    ($message:expr) => {
        $crate::monitoring::error_tracker::global_tracker().record(
            $crate::monitoring::error_tracker::ErrorRecordBuilder::new($message).build()
        )
    };
    ($message:expr, $($key:ident = $value:expr),* $(,)?) => {
        {
            let mut builder = $crate::monitoring::error_tracker::ErrorRecordBuilder::new($message);
            $(
                builder = builder.with_context(stringify!($key), format!("{:?}", $value));
            )*
            $crate::monitoring::error_tracker::global_tracker().record(builder.build())
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_tracker_basic() {
        let tracker = ErrorTracker::with_defaults();

        let record = ErrorRecordBuilder::new("테스트 에러")
            .severity(ErrorSeverity::Error)
            .category(ErrorCategory::Database)
            .function("test_error_tracker_basic")
            .entity("TEST_TICKER")
            .with_context("field", "market_cap")
            .with_decimal("value", Some(Decimal::new(12345, 2)))
            .build();

        let id = tracker.record(record);
        assert_eq!(id, 1);

        let recent = tracker.get_recent(10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].entity, Some("TEST_TICKER".to_string()));
        assert!(recent[0].source_location.file.contains("error_tracker.rs"));

        let stats = tracker.get_stats();
        assert_eq!(stats.total_count, 1);
    }

    #[test]
    fn test_error_stats() {
        let tracker = ErrorTracker::with_defaults();

        // 여러 에러 기록
        for i in 0..5 {
            let record = ErrorRecordBuilder::new(format!("에러 {}", i))
                .severity(if i % 2 == 0 {
                    ErrorSeverity::Error
                } else {
                    ErrorSeverity::Warning
                })
                .category(ErrorCategory::Database)
                .build();
            tracker.record(record);
        }

        let stats = tracker.get_stats();
        assert_eq!(stats.total_count, 5);
        assert_eq!(*stats.by_category.get("database").unwrap_or(&0), 5);
    }

    #[test]
    fn test_source_location_capture() {
        let loc = SourceLocation::capture_with_function("test_function");
        assert!(loc.file.contains("error_tracker.rs"));
        assert!(loc.line > 0);
        assert_eq!(loc.function, Some("test_function".to_string()));
    }
}
