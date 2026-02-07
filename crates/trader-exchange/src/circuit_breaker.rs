//! Circuit Breaker pattern implementation.
//!
//! 외부 서비스 장애 시 연쇄 실패를 방지하고 시스템 복원력을 향상시킵니다.
//!
//! # 상태 전이
//!
//! ```text
//! Closed ──[실패 임계치 도달]──> Open
//!    ↑                            │
//!    │                   [타임아웃 경과]
//!    │                            ↓
//!    └──[성공]── HalfOpen ──[실패]──> Open
//! ```
//!
//! # 에러 유형별 임계치
//!
//! 각 에러 카테고리별로 다른 임계치를 설정할 수 있습니다:
//!
//! - **Network**: 네트워크/연결 오류 (기본 5회)
//! - **RateLimit**: API 요청 한도 초과 (기본 10회 - 더 관대)
//! - **Timeout**: 요청 타임아웃 (기본 5회)
//! - **Service**: 기타 서비스 오류 (기본 5회)

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::ExchangeError;

/// 에러 카테고리.
///
/// 에러를 카테고리별로 분류하여 차등 임계치를 적용합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// 네트워크/연결 오류 (NetworkError, Disconnected, WebSocket)
    Network,
    /// API 요청 한도 초과 (RateLimited)
    RateLimit,
    /// 요청 타임아웃 (Timeout)
    Timeout,
    /// 기타 서비스 오류 (TimestampError 등)
    Service,
}

impl ErrorCategory {
    /// ExchangeError에서 카테고리 추출.
    ///
    /// 재시도 불가능한 에러는 None 반환.
    pub fn from_error(error: &ExchangeError) -> Option<Self> {
        match error {
            ExchangeError::NetworkError(_)
            | ExchangeError::Disconnected(_)
            | ExchangeError::WebSocket(_) => Some(ErrorCategory::Network),
            ExchangeError::RateLimited => Some(ErrorCategory::RateLimit),
            ExchangeError::Timeout(_) => Some(ErrorCategory::Timeout),
            ExchangeError::TimestampError(_) => Some(ErrorCategory::Service),
            // 재시도 불가능한 에러
            _ => None,
        }
    }

    /// 모든 카테고리 반환.
    pub fn all() -> [ErrorCategory; 4] {
        [
            ErrorCategory::Network,
            ErrorCategory::RateLimit,
            ErrorCategory::Timeout,
            ErrorCategory::Service,
        ]
    }
}

impl std::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCategory::Network => write!(f, "network"),
            ErrorCategory::RateLimit => write!(f, "rate_limit"),
            ErrorCategory::Timeout => write!(f, "timeout"),
            ErrorCategory::Service => write!(f, "service"),
        }
    }
}

/// 에러 카테고리별 임계치 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryThresholds {
    /// 네트워크 오류 임계치
    #[serde(default = "default_network_threshold")]
    pub network: u32,
    /// Rate limit 임계치 (더 관대함)
    #[serde(default = "default_rate_limit_threshold")]
    pub rate_limit: u32,
    /// 타임아웃 임계치
    #[serde(default = "default_timeout_threshold")]
    pub timeout: u32,
    /// 서비스 오류 임계치
    #[serde(default = "default_service_threshold")]
    pub service: u32,
}

fn default_network_threshold() -> u32 {
    5
}
fn default_rate_limit_threshold() -> u32 {
    10
} // Rate limit은 더 관대하게
fn default_timeout_threshold() -> u32 {
    5
}
fn default_service_threshold() -> u32 {
    5
}

impl Default for CategoryThresholds {
    fn default() -> Self {
        Self {
            network: default_network_threshold(),
            rate_limit: default_rate_limit_threshold(),
            timeout: default_timeout_threshold(),
            service: default_service_threshold(),
        }
    }
}

impl CategoryThresholds {
    /// 카테고리별 임계치 조회.
    pub fn get(&self, category: ErrorCategory) -> u32 {
        match category {
            ErrorCategory::Network => self.network,
            ErrorCategory::RateLimit => self.rate_limit,
            ErrorCategory::Timeout => self.timeout,
            ErrorCategory::Service => self.service,
        }
    }

    /// 보수적인 설정 (낮은 임계치).
    pub fn conservative() -> Self {
        Self {
            network: 3,
            rate_limit: 5,
            timeout: 3,
            service: 3,
        }
    }

    /// 공격적인 설정 (높은 임계치).
    pub fn aggressive() -> Self {
        Self {
            network: 10,
            rate_limit: 20,
            timeout: 10,
            service: 10,
        }
    }
}

/// Circuit Breaker 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// 정상 상태 - 모든 요청 허용
    Closed,
    /// 장애 상태 - 모든 요청 즉시 거부
    Open,
    /// 복구 테스트 상태 - 단일 요청만 허용
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "closed"),
            CircuitState::Open => write!(f, "open"),
            CircuitState::HalfOpen => write!(f, "half_open"),
        }
    }
}

/// Circuit Breaker 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// 기본 실패 임계치 (카테고리별 설정이 없을 때 사용)
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    /// Open 상태 유지 시간 (밀리초, 이후 HalfOpen으로 전이)
    #[serde(default = "default_reset_timeout_ms")]
    pub reset_timeout_ms: u64,
    /// 연속 성공 횟수 (HalfOpen에서 Closed로 전이하기 위한 조건)
    #[serde(default = "default_success_threshold")]
    pub success_threshold: u32,
    /// 에러 카테고리별 임계치 (설정 시 category_thresholds 우선 적용)
    #[serde(default)]
    pub category_thresholds: Option<CategoryThresholds>,
    /// 캐시된 Duration (직렬화 제외)
    #[serde(skip)]
    reset_timeout: Option<Duration>,
}

fn default_failure_threshold() -> u32 {
    5
}
fn default_reset_timeout_ms() -> u64 {
    30_000
} // 30초
fn default_success_threshold() -> u32 {
    1
}

impl CircuitBreakerConfig {
    /// reset_timeout Duration 반환 (캐싱).
    pub fn reset_timeout(&self) -> Duration {
        self.reset_timeout
            .unwrap_or_else(|| Duration::from_millis(self.reset_timeout_ms))
    }
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: default_failure_threshold(),
            reset_timeout_ms: default_reset_timeout_ms(),
            success_threshold: default_success_threshold(),
            category_thresholds: None,
            reset_timeout: None,
        }
    }
}

impl CircuitBreakerConfig {
    /// 새 설정 생성.
    pub fn new(failure_threshold: u32, reset_timeout_secs: u64, success_threshold: u32) -> Self {
        Self {
            failure_threshold,
            reset_timeout_ms: reset_timeout_secs * 1000,
            success_threshold,
            category_thresholds: None,
            reset_timeout: Some(Duration::from_secs(reset_timeout_secs)),
        }
    }

    /// 카테고리별 임계치 설정 추가.
    pub fn with_category_thresholds(mut self, thresholds: CategoryThresholds) -> Self {
        self.category_thresholds = Some(thresholds);
        self
    }

    /// 특정 카테고리의 임계치 조회.
    pub fn threshold_for(&self, category: ErrorCategory) -> u32 {
        self.category_thresholds
            .as_ref()
            .map(|t| t.get(category))
            .unwrap_or(self.failure_threshold)
    }

    /// 보수적인 설정 (낮은 임계치, 긴 타임아웃).
    pub fn conservative() -> Self {
        Self {
            failure_threshold: 3,
            reset_timeout_ms: 60_000, // 60초
            success_threshold: 2,
            category_thresholds: Some(CategoryThresholds::conservative()),
            reset_timeout: Some(Duration::from_secs(60)),
        }
    }

    /// 공격적인 설정 (높은 임계치, 짧은 타임아웃).
    pub fn aggressive() -> Self {
        Self {
            failure_threshold: 10,
            reset_timeout_ms: 10_000, // 10초
            success_threshold: 1,
            category_thresholds: Some(CategoryThresholds::aggressive()),
            reset_timeout: Some(Duration::from_secs(10)),
        }
    }
}

/// Circuit Breaker 내부 상태.
struct CircuitBreakerState {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    last_state_change: Instant,
    /// 카테고리별 실패 카운트
    category_failures: HashMap<ErrorCategory, u32>,
    /// Circuit을 Open으로 전이시킨 카테고리
    tripped_by: Option<ErrorCategory>,
}

impl CircuitBreakerState {
    fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            last_state_change: Instant::now(),
            category_failures: HashMap::new(),
            tripped_by: None,
        }
    }

    /// 특정 카테고리의 실패 카운트 조회.
    fn category_failure_count(&self, category: ErrorCategory) -> u32 {
        *self.category_failures.get(&category).unwrap_or(&0)
    }

    /// 특정 카테고리의 실패 카운트 증가.
    fn increment_category_failure(&mut self, category: ErrorCategory) {
        *self.category_failures.entry(category).or_insert(0) += 1;
    }

    /// 카테고리별 실패 카운트 리셋.
    fn reset_category_failures(&mut self) {
        self.category_failures.clear();
        self.tripped_by = None;
    }
}

/// Circuit Breaker.
///
/// 외부 서비스 호출 시 연쇄 실패를 방지합니다.
///
/// # Example
///
/// ```ignore
/// let cb = CircuitBreaker::new("binance", CircuitBreakerConfig::default());
///
/// // 요청 허용 여부 확인
/// if cb.is_allowed() {
///     match make_request().await {
///         Ok(result) => {
///             cb.record_success();
///             // 결과 처리
///         }
///         Err(e) => {
///             if e.is_retryable() {
///                 cb.record_failure();
///             }
///         }
///     }
/// } else {
///     // Circuit이 열려있음 - 빠른 실패
///     return Err(ExchangeError::CircuitOpen);
/// }
/// ```
pub struct CircuitBreaker {
    /// 서비스 이름 (로깅 및 메트릭용)
    name: String,
    /// 설정
    config: CircuitBreakerConfig,
    /// 내부 상태 (RwLock으로 보호)
    state: RwLock<CircuitBreakerState>,
    /// 총 실패 횟수 (메트릭용)
    total_failures: AtomicU64,
    /// 총 성공 횟수 (메트릭용)
    total_successes: AtomicU64,
    /// Circuit Open 횟수 (메트릭용)
    open_count: AtomicU64,
}

impl CircuitBreaker {
    /// 새 Circuit Breaker 생성.
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            name: name.into(),
            config,
            state: RwLock::new(CircuitBreakerState::new()),
            total_failures: AtomicU64::new(0),
            total_successes: AtomicU64::new(0),
            open_count: AtomicU64::new(0),
        }
    }

    /// 기본 설정으로 생성.
    pub fn with_defaults(name: impl Into<String>) -> Self {
        Self::new(name, CircuitBreakerConfig::default())
    }

    /// 서비스 이름 반환.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 현재 상태 반환.
    pub fn state(&self) -> CircuitState {
        let mut state = self.state.write().unwrap();
        self.maybe_transition_from_open(&mut state);
        state.state
    }

    /// 요청이 허용되는지 확인.
    ///
    /// HalfOpen 상태에서는 복구 테스트를 위해 단일 요청만 허용됩니다.
    pub fn is_allowed(&self) -> bool {
        let mut state = self.state.write().unwrap();

        // Open 상태에서 타임아웃이 경과했으면 HalfOpen으로 전이
        self.maybe_transition_from_open(&mut state);

        match state.state {
            CircuitState::Closed => true,
            CircuitState::Open => false,
            CircuitState::HalfOpen => {
                // HalfOpen에서는 이미 테스트 요청이 진행 중이면 거부
                // 단순 구현을 위해 HalfOpen에서는 항상 허용
                // (실제로는 semaphore로 제한할 수 있음)
                true
            }
        }
    }

    /// 성공 기록.
    ///
    /// HalfOpen 상태에서 성공하면 Closed로 전이합니다.
    pub fn record_success(&self) {
        self.total_successes.fetch_add(1, Ordering::Relaxed);

        let mut state = self.state.write().unwrap();

        match state.state {
            CircuitState::HalfOpen => {
                state.success_count += 1;
                if state.success_count >= self.config.success_threshold {
                    // HalfOpen → Closed
                    self.transition_to(&mut state, CircuitState::Closed);
                    tracing::info!(
                        circuit_breaker = %self.name,
                        "Circuit breaker recovered: HalfOpen -> Closed"
                    );
                }
            }
            CircuitState::Closed => {
                // Closed 상태에서 성공하면 실패 카운터 리셋
                state.failure_count = 0;
                state.reset_category_failures();
            }
            CircuitState::Open => {
                // Open 상태에서는 요청이 거부되므로 이 케이스는 발생하지 않아야 함
            }
        }
    }

    /// 실패 기록 (기본 카테고리 사용).
    ///
    /// 실패 횟수가 임계치를 초과하면 Open 상태로 전이합니다.
    /// 카테고리 지정 없이 호출 시 기본 failure_threshold 사용.
    pub fn record_failure(&self) {
        self.record_failure_internal(None);
    }

    /// 특정 카테고리의 실패 기록.
    ///
    /// 카테고리별 임계치가 설정된 경우 해당 임계치 사용.
    pub fn record_failure_with_category(&self, category: ErrorCategory) {
        self.record_failure_internal(Some(category));
    }

    /// 내부 실패 기록 로직.
    fn record_failure_internal(&self, category: Option<ErrorCategory>) {
        self.total_failures.fetch_add(1, Ordering::Relaxed);

        let mut state = self.state.write().unwrap();
        state.last_failure_time = Some(Instant::now());

        match state.state {
            CircuitState::Closed => {
                state.failure_count += 1;

                // 카테고리별 실패 카운트 업데이트 및 임계치 체크
                let threshold_exceeded = if let Some(cat) = category {
                    state.increment_category_failure(cat);
                    let cat_count = state.category_failure_count(cat);
                    let cat_threshold = self.config.threshold_for(cat);

                    if cat_count >= cat_threshold {
                        state.tripped_by = Some(cat);
                        tracing::warn!(
                            circuit_breaker = %self.name,
                            category = %cat,
                            failure_count = cat_count,
                            threshold = cat_threshold,
                            "Category threshold exceeded"
                        );
                        true
                    } else {
                        false
                    }
                } else {
                    // 카테고리 없으면 전체 카운트로 판단
                    state.failure_count >= self.config.failure_threshold
                };

                if threshold_exceeded {
                    // Closed → Open
                    self.transition_to(&mut state, CircuitState::Open);
                    self.open_count.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(
                        circuit_breaker = %self.name,
                        failure_count = state.failure_count,
                        tripped_by = ?state.tripped_by,
                        "Circuit breaker tripped: Closed -> Open"
                    );
                }
            }
            CircuitState::HalfOpen => {
                // HalfOpen → Open (복구 테스트 실패)
                if let Some(cat) = category {
                    state.tripped_by = Some(cat);
                }
                self.transition_to(&mut state, CircuitState::Open);
                self.open_count.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    circuit_breaker = %self.name,
                    category = ?category,
                    "Circuit breaker recovery failed: HalfOpen -> Open"
                );
            }
            CircuitState::Open => {
                // 이미 Open 상태
            }
        }
    }

    /// ExchangeError 기반 결과 기록.
    ///
    /// 에러 카테고리별 임계치를 자동으로 적용합니다.
    pub fn record_result<T>(&self, result: &Result<T, ExchangeError>) {
        match result {
            Ok(_) => self.record_success(),
            Err(e) => {
                // 에러 카테고리 추출 (재시도 가능한 에러만)
                if let Some(category) = ErrorCategory::from_error(e) {
                    self.record_failure_with_category(category);
                }
                // 재시도 불가능한 에러는 Circuit Breaker에 영향 주지 않음
            }
        }
    }

    /// 수동으로 Circuit 리셋.
    pub fn reset(&self) {
        let mut state = self.state.write().unwrap();
        self.transition_to(&mut state, CircuitState::Closed);
        state.failure_count = 0;
        state.success_count = 0;
        state.reset_category_failures();
        tracing::info!(
            circuit_breaker = %self.name,
            "Circuit breaker manually reset"
        );
    }

    /// 메트릭 반환.
    pub fn metrics(&self) -> CircuitBreakerMetrics {
        let state = self.state.read().unwrap();
        CircuitBreakerMetrics {
            name: self.name.clone(),
            state: state.state,
            failure_count: state.failure_count,
            total_failures: self.total_failures.load(Ordering::Relaxed),
            total_successes: self.total_successes.load(Ordering::Relaxed),
            open_count: self.open_count.load(Ordering::Relaxed),
            time_in_current_state: state.last_state_change.elapsed(),
            category_failures: state.category_failures.clone(),
            tripped_by: state.tripped_by,
        }
    }

    /// Open 상태에서 타임아웃이 경과했으면 HalfOpen으로 전이.
    fn maybe_transition_from_open(&self, state: &mut CircuitBreakerState) {
        if state.state == CircuitState::Open
            && state.last_state_change.elapsed() >= self.config.reset_timeout() {
                self.transition_to(state, CircuitState::HalfOpen);
                tracing::info!(
                    circuit_breaker = %self.name,
                    "Circuit breaker timeout: Open -> HalfOpen"
                );
            }
    }

    /// 상태 전이.
    fn transition_to(&self, state: &mut CircuitBreakerState, new_state: CircuitState) {
        state.state = new_state;
        state.last_state_change = Instant::now();

        if new_state == CircuitState::Closed {
            state.failure_count = 0;
            state.success_count = 0;
            state.reset_category_failures();
        } else if new_state == CircuitState::HalfOpen {
            state.success_count = 0;
        }
    }

    /// Circuit을 Open으로 전이시킨 에러 카테고리 조회.
    pub fn tripped_by(&self) -> Option<ErrorCategory> {
        let state = self.state.read().unwrap();
        state.tripped_by
    }

    /// 카테고리별 현재 실패 카운트 조회.
    pub fn category_failures(&self) -> HashMap<ErrorCategory, u32> {
        let state = self.state.read().unwrap();
        state.category_failures.clone()
    }
}

/// Circuit Breaker 메트릭.
#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    /// 서비스 이름
    pub name: String,
    /// 현재 상태
    pub state: CircuitState,
    /// 현재 연속 실패 횟수
    pub failure_count: u32,
    /// 총 실패 횟수
    pub total_failures: u64,
    /// 총 성공 횟수
    pub total_successes: u64,
    /// Circuit Open 횟수
    pub open_count: u64,
    /// 현재 상태 유지 시간
    pub time_in_current_state: Duration,
    /// 카테고리별 현재 실패 카운트
    pub category_failures: HashMap<ErrorCategory, u32>,
    /// Circuit을 Open으로 전이시킨 카테고리
    pub tripped_by: Option<ErrorCategory>,
}

/// Circuit이 열려있을 때 반환되는 에러.
#[derive(Debug, Clone)]
pub struct CircuitOpenError {
    /// Circuit Breaker 이름
    pub name: String,
    /// 남은 대기 시간 (예상)
    pub retry_after: Option<Duration>,
}

impl std::fmt::Display for CircuitOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Circuit breaker '{}' is open", self.name)?;
        if let Some(retry_after) = self.retry_after {
            write!(f, " (retry after {:?})", retry_after)?;
        }
        Ok(())
    }
}

impl std::error::Error for CircuitOpenError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_circuit_breaker_initial_state() {
        let cb = CircuitBreaker::with_defaults("test");
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_allowed());
    }

    #[test]
    fn test_circuit_breaker_opens_after_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            reset_timeout_ms: 30_000,
            success_threshold: 1,
            category_thresholds: None,
            reset_timeout: None,
        };
        let cb = CircuitBreaker::new("test", config);

        // 3번 실패
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.is_allowed());
    }

    #[test]
    fn test_circuit_breaker_success_resets_failure_count() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            reset_timeout_ms: 30_000,
            success_threshold: 1,
            category_thresholds: None,
            reset_timeout: None,
        };
        let cb = CircuitBreaker::new("test", config);

        // 2번 실패 후 성공
        cb.record_failure();
        cb.record_failure();
        cb.record_success();

        // 다시 3번 실패해야 Open
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_half_open_after_timeout() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            reset_timeout_ms: 50,
            success_threshold: 1,
            category_thresholds: None,
            reset_timeout: None,
        };
        let cb = CircuitBreaker::new("test", config);

        // Open 상태로 전이
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // 타임아웃 대기
        thread::sleep(Duration::from_millis(60));

        // HalfOpen으로 전이
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        assert!(cb.is_allowed());
    }

    #[test]
    fn test_circuit_breaker_recovers_on_success() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            reset_timeout_ms: 50,
            success_threshold: 1,
            category_thresholds: None,
            reset_timeout: None,
        };
        let cb = CircuitBreaker::new("test", config);

        // Open → HalfOpen → Closed
        cb.record_failure();
        thread::sleep(Duration::from_millis(60));
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_half_open_failure_reopens() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            reset_timeout_ms: 50,
            success_threshold: 1,
            category_thresholds: None,
            reset_timeout: None,
        };
        let cb = CircuitBreaker::new("test", config);

        // Open → HalfOpen → Open
        cb.record_failure();
        thread::sleep(Duration::from_millis(60));
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_record_result() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            reset_timeout_ms: 30_000,
            success_threshold: 1,
            category_thresholds: None,
            reset_timeout: None,
        };
        let cb = CircuitBreaker::new("test", config);

        // 성공
        let ok_result: Result<i32, ExchangeError> = Ok(42);
        cb.record_result(&ok_result);
        assert_eq!(cb.state(), CircuitState::Closed);

        // 재시도 가능한 에러 (실패로 카운트)
        let retryable_err: Result<i32, ExchangeError> =
            Err(ExchangeError::NetworkError("connection failed".to_string()));
        cb.record_result(&retryable_err);
        cb.record_result(&retryable_err);
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_non_retryable_error_ignored() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            reset_timeout_ms: 30_000,
            success_threshold: 1,
            category_thresholds: None,
            reset_timeout: None,
        };
        let cb = CircuitBreaker::new("test", config);

        // 재시도 불가능한 에러 (실패로 카운트 안 됨)
        let non_retryable: Result<i32, ExchangeError> =
            Err(ExchangeError::InsufficientBalance("not enough".to_string()));

        cb.record_result(&non_retryable);
        cb.record_result(&non_retryable);
        cb.record_result(&non_retryable);

        // 여전히 Closed
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_metrics() {
        let cb = CircuitBreaker::with_defaults("test");

        cb.record_success();
        cb.record_success();
        cb.record_failure();

        let metrics = cb.metrics();
        assert_eq!(metrics.name, "test");
        assert_eq!(metrics.state, CircuitState::Closed);
        assert_eq!(metrics.total_successes, 2);
        assert_eq!(metrics.total_failures, 1);
        assert_eq!(metrics.failure_count, 1);
    }

    #[test]
    fn test_circuit_breaker_manual_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            reset_timeout_ms: 300_000,
            success_threshold: 1,
            category_thresholds: None,
            reset_timeout: None,
        };
        let cb = CircuitBreaker::new("test", config);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_allowed());
    }

    #[test]
    fn test_category_thresholds_network() {
        // 네트워크 카테고리에 낮은 임계치 설정
        let thresholds = CategoryThresholds {
            network: 2,
            rate_limit: 10,
            timeout: 5,
            service: 5,
        };
        let config = CircuitBreakerConfig::default().with_category_thresholds(thresholds);
        let cb = CircuitBreaker::new("test", config);

        // 네트워크 에러 2번 → Open
        cb.record_failure_with_category(ErrorCategory::Network);
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure_with_category(ErrorCategory::Network);
        assert_eq!(cb.state(), CircuitState::Open);
        assert_eq!(cb.tripped_by(), Some(ErrorCategory::Network));
    }

    #[test]
    fn test_category_thresholds_rate_limit_more_tolerant() {
        // Rate limit은 더 높은 임계치
        let thresholds = CategoryThresholds {
            network: 2,
            rate_limit: 5, // 더 관대
            timeout: 2,
            service: 2,
        };
        let config = CircuitBreakerConfig::default().with_category_thresholds(thresholds);
        let cb = CircuitBreaker::new("test", config);

        // Rate limit 4번 → 아직 Closed
        for _ in 0..4 {
            cb.record_failure_with_category(ErrorCategory::RateLimit);
        }
        assert_eq!(cb.state(), CircuitState::Closed);

        // 5번째 → Open
        cb.record_failure_with_category(ErrorCategory::RateLimit);
        assert_eq!(cb.state(), CircuitState::Open);
        assert_eq!(cb.tripped_by(), Some(ErrorCategory::RateLimit));
    }

    #[test]
    fn test_different_categories_independent() {
        // 각 카테고리가 독립적으로 카운트되는지 확인
        let thresholds = CategoryThresholds {
            network: 3,
            rate_limit: 3,
            timeout: 3,
            service: 3,
        };
        let config = CircuitBreakerConfig::default().with_category_thresholds(thresholds);
        let cb = CircuitBreaker::new("test", config);

        // 각 카테고리 2번씩 실패 (총 8번, 개별로는 임계치 미달)
        cb.record_failure_with_category(ErrorCategory::Network);
        cb.record_failure_with_category(ErrorCategory::Network);
        cb.record_failure_with_category(ErrorCategory::RateLimit);
        cb.record_failure_with_category(ErrorCategory::RateLimit);
        cb.record_failure_with_category(ErrorCategory::Timeout);
        cb.record_failure_with_category(ErrorCategory::Timeout);
        cb.record_failure_with_category(ErrorCategory::Service);
        cb.record_failure_with_category(ErrorCategory::Service);

        // 아직 Closed (각 카테고리가 2개씩)
        assert_eq!(cb.state(), CircuitState::Closed);

        // Network 1번 더 → Open
        cb.record_failure_with_category(ErrorCategory::Network);
        assert_eq!(cb.state(), CircuitState::Open);
        assert_eq!(cb.tripped_by(), Some(ErrorCategory::Network));
    }

    #[test]
    fn test_record_result_with_categories() {
        let thresholds = CategoryThresholds {
            network: 2,
            rate_limit: 3,
            timeout: 2,
            service: 2,
        };
        let config = CircuitBreakerConfig::default().with_category_thresholds(thresholds);
        let cb = CircuitBreaker::new("test", config);

        // RateLimited 에러 (rate_limit 카테고리)
        let rate_limit_err: Result<i32, ExchangeError> = Err(ExchangeError::RateLimited);
        cb.record_result(&rate_limit_err);
        cb.record_result(&rate_limit_err);
        assert_eq!(cb.state(), CircuitState::Closed); // 임계치 3, 아직 2

        // Timeout 에러 (timeout 카테고리)
        let timeout_err: Result<i32, ExchangeError> =
            Err(ExchangeError::Timeout("timeout".to_string()));
        cb.record_result(&timeout_err);
        cb.record_result(&timeout_err);
        assert_eq!(cb.state(), CircuitState::Open); // timeout 임계치 2 도달
        assert_eq!(cb.tripped_by(), Some(ErrorCategory::Timeout));
    }

    #[test]
    fn test_category_failures_reset_on_success() {
        let thresholds = CategoryThresholds {
            network: 3,
            rate_limit: 3,
            timeout: 3,
            service: 3,
        };
        let config = CircuitBreakerConfig::default().with_category_thresholds(thresholds);
        let cb = CircuitBreaker::new("test", config);

        // 네트워크 에러 2번
        cb.record_failure_with_category(ErrorCategory::Network);
        cb.record_failure_with_category(ErrorCategory::Network);

        // 성공하면 리셋
        cb.record_success();

        // 다시 3번 실패해야 Open
        cb.record_failure_with_category(ErrorCategory::Network);
        cb.record_failure_with_category(ErrorCategory::Network);
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure_with_category(ErrorCategory::Network);
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_metrics_includes_category_info() {
        let thresholds = CategoryThresholds {
            network: 5,
            rate_limit: 10,
            timeout: 5,
            service: 5,
        };
        let config = CircuitBreakerConfig::default().with_category_thresholds(thresholds);
        let cb = CircuitBreaker::new("test", config);

        cb.record_failure_with_category(ErrorCategory::Network);
        cb.record_failure_with_category(ErrorCategory::Network);
        cb.record_failure_with_category(ErrorCategory::RateLimit);

        let metrics = cb.metrics();
        assert_eq!(
            metrics.category_failures.get(&ErrorCategory::Network),
            Some(&2)
        );
        assert_eq!(
            metrics.category_failures.get(&ErrorCategory::RateLimit),
            Some(&1)
        );
        assert_eq!(metrics.tripped_by, None); // 아직 Open 아님

        // Open 시키기
        for _ in 0..3 {
            cb.record_failure_with_category(ErrorCategory::Network);
        }

        let metrics = cb.metrics();
        assert_eq!(metrics.tripped_by, Some(ErrorCategory::Network));
    }

    #[test]
    fn test_error_category_from_error() {
        // Network 카테고리
        assert_eq!(
            ErrorCategory::from_error(&ExchangeError::NetworkError("test".to_string())),
            Some(ErrorCategory::Network)
        );
        assert_eq!(
            ErrorCategory::from_error(&ExchangeError::Disconnected("test".to_string())),
            Some(ErrorCategory::Network)
        );
        assert_eq!(
            ErrorCategory::from_error(&ExchangeError::WebSocket("test".to_string())),
            Some(ErrorCategory::Network)
        );

        // RateLimit 카테고리
        assert_eq!(
            ErrorCategory::from_error(&ExchangeError::RateLimited),
            Some(ErrorCategory::RateLimit)
        );

        // Timeout 카테고리
        assert_eq!(
            ErrorCategory::from_error(&ExchangeError::Timeout("test".to_string())),
            Some(ErrorCategory::Timeout)
        );

        // Service 카테고리
        assert_eq!(
            ErrorCategory::from_error(&ExchangeError::TimestampError("test".to_string())),
            Some(ErrorCategory::Service)
        );

        // 재시도 불가능 에러 → None
        assert_eq!(
            ErrorCategory::from_error(&ExchangeError::InsufficientBalance("test".to_string())),
            None
        );
        assert_eq!(
            ErrorCategory::from_error(&ExchangeError::InvalidQuantity("test".to_string())),
            None
        );
    }

    #[test]
    fn test_conservative_and_aggressive_presets() {
        // Conservative 설정
        let conservative = CircuitBreakerConfig::conservative();
        assert!(conservative.category_thresholds.is_some());
        let ct = conservative.category_thresholds.unwrap();
        assert_eq!(ct.network, 3);
        assert_eq!(ct.rate_limit, 5);

        // Aggressive 설정
        let aggressive = CircuitBreakerConfig::aggressive();
        assert!(aggressive.category_thresholds.is_some());
        let ct = aggressive.category_thresholds.unwrap();
        assert_eq!(ct.network, 10);
        assert_eq!(ct.rate_limit, 20);
    }
}
