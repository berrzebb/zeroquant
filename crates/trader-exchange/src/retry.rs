//! 거래소 API 재시도 유틸리티.
//!
//! 네트워크 오류, Rate Limit 등 일시적인 오류에 대해 자동 재시도를 수행합니다.
//! 거래소 중립적으로 설계되어 모든 거래소 커넥터에서 사용 가능합니다.
//!
//! # 예시
//!
//! ```rust,ignore
//! use trader_exchange::retry::{RetryConfig, with_retry};
//!
//! let config = RetryConfig::default();
//! let result = with_retry(&config, || async {
//!     client.get_balance("USDT").await
//! }).await;
//! ```

use std::future::Future;
use std::time::Duration;
use tracing::{debug, warn};

use crate::ExchangeError;

/// 재시도 설정.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 최대 재시도 횟수 (초기 시도 제외).
    pub max_retries: u32,
    /// 기본 대기 시간 (에러에 지정된 대기 시간이 없을 때 사용).
    pub base_delay: Duration,
    /// 최대 대기 시간.
    pub max_delay: Duration,
    /// 지수 백오프 사용 여부.
    pub use_exponential_backoff: bool,
    /// 백오프 배수 (지수 백오프 시 사용).
    pub backoff_multiplier: f64,
    /// 재시도 시 지터(무작위 지연) 추가 여부.
    pub add_jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(60),
            use_exponential_backoff: true,
            backoff_multiplier: 2.0,
            add_jitter: true,
        }
    }
}

impl RetryConfig {
    /// 빠른 재시도 설정 (짧은 지연, 적은 재시도).
    pub fn fast() -> Self {
        Self {
            max_retries: 2,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            use_exponential_backoff: true,
            backoff_multiplier: 2.0,
            add_jitter: true,
        }
    }

    /// 적극적인 재시도 설정 (많은 재시도, 긴 대기).
    pub fn aggressive() -> Self {
        Self {
            max_retries: 5,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(120),
            use_exponential_backoff: true,
            backoff_multiplier: 2.0,
            add_jitter: true,
        }
    }

    /// 재시도 없음 (단일 시도).
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            ..Default::default()
        }
    }

    /// 대기 시간 계산.
    fn calculate_delay(&self, attempt: u32, error: &ExchangeError) -> Duration {
        // 에러에 지정된 대기 시간이 있으면 우선 사용
        let base = error
            .retry_delay_ms()
            .map(Duration::from_millis)
            .unwrap_or(self.base_delay);

        // 지수 백오프 적용
        let delay = if self.use_exponential_backoff && attempt > 0 {
            let multiplier = self.backoff_multiplier.powi(attempt as i32);
            Duration::from_secs_f64(base.as_secs_f64() * multiplier)
        } else {
            base
        };

        // 최대 대기 시간 제한
        let delay = delay.min(self.max_delay);

        // 지터 추가 (±25%)
        if self.add_jitter {
            let jitter_range = delay.as_millis() as f64 * 0.25;
            let jitter = (rand_simple() * 2.0 - 1.0) * jitter_range;
            Duration::from_millis((delay.as_millis() as f64 + jitter).max(0.0) as u64)
        } else {
            delay
        }
    }
}

/// 간단한 난수 생성 (0.0 ~ 1.0).
/// 외부 의존성 없이 시스템 시간 기반으로 생성.
fn rand_simple() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos as f64) / (u32::MAX as f64)
}

/// 재시도 컨텍스트 (콜백에서 현재 시도 정보 접근용).
#[derive(Debug, Clone)]
pub struct RetryContext {
    /// 현재 시도 횟수 (0부터 시작).
    pub attempt: u32,
    /// 최대 재시도 횟수.
    pub max_retries: u32,
    /// 이전 에러 (첫 시도 시 None).
    pub last_error: Option<String>,
}

/// 재시도 결과 통계.
#[derive(Debug, Clone)]
pub struct RetryStats {
    /// 총 시도 횟수.
    pub total_attempts: u32,
    /// 총 대기 시간.
    pub total_delay: Duration,
    /// 성공 여부.
    pub success: bool,
}

/// 재시도가 포함된 비동기 작업 실행.
///
/// # Arguments
/// * `config` - 재시도 설정
/// * `operation` - 실행할 비동기 작업
///
/// # Returns
/// * `Ok(T)` - 작업 성공 결과
/// * `Err(ExchangeError)` - 모든 재시도 실패 후 마지막 에러
///
/// # 예시
///
/// ```rust,ignore
/// let result = with_retry(&RetryConfig::default(), || async {
///     exchange.place_order(&order_request).await
/// }).await;
/// ```
pub async fn with_retry<T, F, Fut>(config: &RetryConfig, operation: F) -> Result<T, ExchangeError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, ExchangeError>>,
{
    let mut attempt = 0;
    let mut total_delay = Duration::ZERO;

    loop {
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    debug!(
                        attempts = attempt + 1,
                        total_delay_ms = total_delay.as_millis(),
                        "재시도 후 성공"
                    );
                }
                return Ok(result);
            }
            Err(e) => {
                // 치명적 에러는 재시도하지 않음
                if e.is_fatal() {
                    warn!(
                        error = %e,
                        "치명적 에러 발생, 재시도 없이 실패 반환"
                    );
                    return Err(e);
                }

                // 재시도 가능한 에러가 아니면 즉시 실패
                if !e.is_retryable() {
                    debug!(
                        error = %e,
                        "재시도 불가능한 에러, 즉시 실패 반환"
                    );
                    return Err(e);
                }

                // 최대 재시도 횟수 초과
                if attempt >= config.max_retries {
                    warn!(
                        error = %e,
                        attempts = attempt + 1,
                        max_retries = config.max_retries,
                        "최대 재시도 횟수 초과"
                    );
                    return Err(e);
                }

                // 대기 시간 계산 및 대기
                let delay = config.calculate_delay(attempt, &e);
                total_delay += delay;

                warn!(
                    error = %e,
                    attempt = attempt + 1,
                    max_retries = config.max_retries,
                    delay_ms = delay.as_millis(),
                    "재시도 대기 중"
                );

                tokio::time::sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

/// 재시도가 포함된 비동기 작업 실행 (컨텍스트 포함).
///
/// 콜백에서 현재 시도 정보에 접근할 수 있습니다.
///
/// # 예시
///
/// ```rust,ignore
/// let result = with_retry_context(&RetryConfig::default(), |ctx| async move {
///     if ctx.attempt > 0 {
///         println!("재시도 {}회차", ctx.attempt);
///     }
///     exchange.place_order(&order_request).await
/// }).await;
/// ```
pub async fn with_retry_context<T, F, Fut>(
    config: &RetryConfig,
    operation: F,
) -> Result<(T, RetryStats), ExchangeError>
where
    F: Fn(RetryContext) -> Fut,
    Fut: Future<Output = Result<T, ExchangeError>>,
{
    let mut attempt = 0;
    let mut total_delay = Duration::ZERO;
    let mut last_error: Option<String> = None;

    loop {
        let ctx = RetryContext {
            attempt,
            max_retries: config.max_retries,
            last_error: last_error.clone(),
        };

        match operation(ctx).await {
            Ok(result) => {
                let stats = RetryStats {
                    total_attempts: attempt + 1,
                    total_delay,
                    success: true,
                };
                return Ok((result, stats));
            }
            Err(e) => {
                last_error = Some(e.to_string());

                // 치명적 에러 또는 재시도 불가능한 에러
                if e.is_fatal() || !e.is_retryable() {
                    return Err(e);
                }

                // 최대 재시도 횟수 초과
                if attempt >= config.max_retries {
                    return Err(e);
                }

                // 대기
                let delay = config.calculate_delay(attempt, &e);
                total_delay += delay;
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

/// 특정 조건에서만 재시도하는 래퍼.
///
/// 사용자 정의 조건으로 재시도 여부를 결정할 수 있습니다.
pub async fn with_retry_if<T, F, Fut, P>(
    config: &RetryConfig,
    operation: F,
    should_retry: P,
) -> Result<T, ExchangeError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, ExchangeError>>,
    P: Fn(&ExchangeError) -> bool,
{
    let mut attempt = 0;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                // 사용자 정의 조건 확인
                if !should_retry(&e) || attempt >= config.max_retries {
                    return Err(e);
                }

                let delay = config.calculate_delay(attempt, &e);
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_immediate_success() {
        let config = RetryConfig::default();
        let result = with_retry(&config, || async { Ok::<_, ExchangeError>(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_on_network_error() {
        let config = RetryConfig {
            max_retries: 3,
            base_delay: Duration::from_millis(10),
            ..Default::default()
        };

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = with_retry(&config, || {
            let counter = counter_clone.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(ExchangeError::NetworkError("연결 실패".to_string()))
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3); // 3번 시도
    }

    #[tokio::test]
    async fn test_no_retry_on_fatal_error() {
        let config = RetryConfig::default();
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = with_retry(&config, || {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(ExchangeError::InsufficientBalance("잔고 부족".to_string()))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 1); // 1번만 시도
    }

    #[tokio::test]
    async fn test_max_retries_exceeded() {
        let config = RetryConfig {
            max_retries: 2,
            base_delay: Duration::from_millis(10),
            use_exponential_backoff: false,
            add_jitter: false,
            ..Default::default()
        };

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = with_retry(&config, || {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(ExchangeError::NetworkError("항상 실패".to_string()))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 3); // 초기 1회 + 재시도 2회 = 3번
    }

    #[test]
    fn test_config_presets() {
        let fast = RetryConfig::fast();
        assert_eq!(fast.max_retries, 2);
        assert_eq!(fast.base_delay, Duration::from_millis(100));

        let aggressive = RetryConfig::aggressive();
        assert_eq!(aggressive.max_retries, 5);

        let no_retry = RetryConfig::no_retry();
        assert_eq!(no_retry.max_retries, 0);
    }
}
