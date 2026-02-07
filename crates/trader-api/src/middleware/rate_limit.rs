//! Rate limiting middleware.
//!
//! Token Bucket 알고리즘 기반 rate limiting을 제공합니다.

#![allow(dead_code)] // Rate limiting 레이어는 향후 프로덕션 배포 시 활성화 예정

use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use metrics::counter;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Rate Limiter 설정.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// 분당 최대 요청 수
    pub requests_per_minute: u32,
    /// 버스트 허용량 (순간적으로 허용되는 추가 요청)
    pub burst_size: u32,
    /// 버킷 정리 간격
    pub cleanup_interval: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 1200, // 분당 1200회 (초당 20회)
            burst_size: 50,            // 50회 버스트 허용
            cleanup_interval: Duration::from_secs(60),
        }
    }
}

impl RateLimitConfig {
    /// 새 설정 생성.
    pub fn new(requests_per_minute: u32) -> Self {
        Self {
            requests_per_minute,
            burst_size: requests_per_minute / 10, // 10% 버스트
            ..Default::default()
        }
    }

    /// 엄격한 설정 (버스트 없음).
    pub fn strict(requests_per_minute: u32) -> Self {
        Self {
            requests_per_minute,
            burst_size: 0,
            ..Default::default()
        }
    }
}

/// Token Bucket 구조체.
#[derive(Debug)]
struct TokenBucket {
    /// 현재 토큰 수
    tokens: f64,
    /// 마지막 리필 시간
    last_refill: Instant,
    /// 최대 토큰 수 (버킷 용량)
    max_tokens: f64,
    /// 초당 리필되는 토큰 수
    refill_rate: f64,
}

impl TokenBucket {
    fn new(config: &RateLimitConfig) -> Self {
        let max_tokens = config.requests_per_minute as f64 / 60.0 + config.burst_size as f64;
        let refill_rate = config.requests_per_minute as f64 / 60.0; // 초당 토큰

        Self {
            tokens: max_tokens,
            last_refill: Instant::now(),
            max_tokens,
            refill_rate,
        }
    }

    /// 토큰 소비 시도.
    ///
    /// 성공하면 `true`, Rate limit 초과 시 `false` 반환.
    fn try_acquire(&mut self) -> bool {
        self.refill();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// 토큰 리필.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;
    }

    /// 다음 토큰까지 대기 시간 (초).
    fn time_until_next_token(&self) -> f64 {
        if self.tokens >= 1.0 {
            0.0
        } else {
            (1.0 - self.tokens) / self.refill_rate
        }
    }
}

/// Rate Limiter.
///
/// IP 주소별로 Rate Limiting을 적용합니다.
#[derive(Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    buckets: Arc<RwLock<HashMap<IpAddr, TokenBucket>>>,
}

impl RateLimiter {
    /// 새 Rate Limiter 생성.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 기본 설정으로 생성.
    pub fn with_defaults() -> Self {
        Self::new(RateLimitConfig::default())
    }

    /// 요청 허용 여부 확인.
    pub async fn check(&self, ip: IpAddr) -> RateLimitResult {
        let mut buckets = self.buckets.write().await;

        let bucket = buckets
            .entry(ip)
            .or_insert_with(|| TokenBucket::new(&self.config));

        if bucket.try_acquire() {
            RateLimitResult::Allowed
        } else {
            let retry_after = bucket.time_until_next_token().ceil() as u64;
            RateLimitResult::Limited { retry_after }
        }
    }

    /// 오래된 버킷 정리.
    pub async fn cleanup(&self) {
        let mut buckets = self.buckets.write().await;
        let threshold = Instant::now() - self.config.cleanup_interval;

        buckets.retain(|_, bucket| bucket.last_refill > threshold);
    }

    /// 현재 추적 중인 IP 수 반환.
    pub async fn tracked_ips(&self) -> usize {
        self.buckets.read().await.len()
    }
}

/// Rate Limit 확인 결과.
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    /// 요청 허용됨
    Allowed,
    /// Rate limit 초과
    Limited {
        /// 재시도까지 대기 시간 (초)
        retry_after: u64,
    },
}

/// Rate Limit 미들웨어 상태.
#[derive(Clone)]
pub struct RateLimitState {
    limiter: RateLimiter,
}

impl RateLimitState {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            limiter: RateLimiter::new(config),
        }
    }

    pub fn with_defaults() -> Self {
        Self {
            limiter: RateLimiter::with_defaults(),
        }
    }
}

/// Rate Limiting 미들웨어 함수.
///
/// 클라이언트 IP별로 Rate Limiting을 적용합니다.
pub async fn rate_limit_middleware(
    axum::extract::State(state): axum::extract::State<RateLimitState>,
    request: Request,
    next: Next,
) -> Response {
    // 클라이언트 IP 추출
    let ip = extract_client_ip(&request);

    // Rate limit 확인
    match state.limiter.check(ip).await {
        RateLimitResult::Allowed => {
            // 요청 허용 - 다음 핸들러로 진행
            counter!("rate_limit_requests_total", "status" => "allowed").increment(1);
            next.run(request).await
        }
        RateLimitResult::Limited { retry_after } => {
            // Rate limit 초과 - 429 응답
            counter!("rate_limit_requests_total", "status" => "limited").increment(1);

            tracing::warn!(
                client_ip = %ip,
                retry_after = retry_after,
                "Rate limit exceeded"
            );

            let mut response = (
                StatusCode::TOO_MANY_REQUESTS,
                serde_json::json!({
                    "error": "Too Many Requests",
                    "message": "Rate limit exceeded. Please try again later.",
                    "retry_after": retry_after
                })
                .to_string(),
            )
                .into_response();

            // Retry-After 헤더 추가
            response.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                HeaderValue::from_str(&retry_after.to_string()).unwrap(),
            );

            response
        }
    }
}

/// 요청에서 클라이언트 IP 추출.
///
/// X-Forwarded-For, X-Real-IP 헤더를 우선 확인합니다 (프록시/로드밸런서 뒤에 있을 경우).
fn extract_client_ip(request: &Request) -> IpAddr {
    // X-Forwarded-For 헤더 확인
    if let Some(forwarded_for) = request.headers().get("x-forwarded-for") {
        if let Ok(value) = forwarded_for.to_str() {
            // 첫 번째 IP 사용 (클라이언트 원본 IP)
            if let Some(ip_str) = value.split(',').next() {
                if let Ok(ip) = ip_str.trim().parse() {
                    return ip;
                }
            }
        }
    }

    // X-Real-IP 헤더 확인
    if let Some(real_ip) = request.headers().get("x-real-ip") {
        if let Ok(value) = real_ip.to_str() {
            if let Ok(ip) = value.trim().parse() {
                return ip;
            }
        }
    }

    // 연결 정보에서 IP 추출 (fallback)
    // Axum에서는 ConnectInfo extractor를 사용해야 하지만,
    // 미들웨어에서는 직접 접근이 어려우므로 기본값 사용
    "127.0.0.1".parse().unwrap()
}

/// Rate Limit 레이어 생성 헬퍼.
#[allow(clippy::type_complexity)]
pub fn create_rate_limit_layer(
    requests_per_minute: u32,
) -> (
    RateLimitState,
    fn(
        axum::extract::State<RateLimitState>,
        Request,
        Next,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>,
) {
    let state = RateLimitState::new(RateLimitConfig::new(requests_per_minute));
    (state, |s, r, n| Box::pin(rate_limit_middleware(s, r, n)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_requests() {
        let config = RateLimitConfig {
            requests_per_minute: 60,
            burst_size: 10,
            cleanup_interval: Duration::from_secs(60),
        };
        let limiter = RateLimiter::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        // 첫 요청은 허용되어야 함
        assert!(matches!(limiter.check(ip).await, RateLimitResult::Allowed));
    }

    #[tokio::test]
    async fn test_rate_limiter_limits_burst() {
        let config = RateLimitConfig {
            requests_per_minute: 60,
            burst_size: 5,
            cleanup_interval: Duration::from_secs(60),
        };
        let limiter = RateLimiter::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        // 버스트 + 기본 토큰 수만큼 허용
        let max_allowed = 60 / 60 + 5; // 1 (초당) + 5 (버스트) = 6

        for i in 0..max_allowed {
            let result = limiter.check(ip).await;
            assert!(
                matches!(result, RateLimitResult::Allowed),
                "Request {} should be allowed",
                i
            );
        }

        // 다음 요청은 제한되어야 함
        assert!(matches!(
            limiter.check(ip).await,
            RateLimitResult::Limited { .. }
        ));
    }

    #[tokio::test]
    async fn test_rate_limiter_different_ips() {
        let config = RateLimitConfig {
            requests_per_minute: 60,
            burst_size: 0,
            cleanup_interval: Duration::from_secs(60),
        };
        let limiter = RateLimiter::new(config);
        let ip1: IpAddr = "192.168.1.1".parse().unwrap();
        let ip2: IpAddr = "192.168.1.2".parse().unwrap();

        // IP1 토큰 소진
        assert!(matches!(limiter.check(ip1).await, RateLimitResult::Allowed));
        assert!(matches!(
            limiter.check(ip1).await,
            RateLimitResult::Limited { .. }
        ));

        // IP2는 별도 버킷이므로 허용
        assert!(matches!(limiter.check(ip2).await, RateLimitResult::Allowed));
    }

    #[tokio::test]
    async fn test_rate_limiter_cleanup() {
        let config = RateLimitConfig {
            requests_per_minute: 60,
            burst_size: 0,
            cleanup_interval: Duration::from_millis(10),
        };
        let limiter = RateLimiter::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        // 요청으로 버킷 생성
        let _ = limiter.check(ip).await;
        assert_eq!(limiter.tracked_ips().await, 1);

        // 정리 간격 대기
        tokio::time::sleep(Duration::from_millis(20)).await;

        // 정리 실행
        limiter.cleanup().await;
        assert_eq!(limiter.tracked_ips().await, 0);
    }

    #[tokio::test]
    async fn test_token_bucket_refill() {
        let config = RateLimitConfig {
            requests_per_minute: 6000, // 초당 100회
            burst_size: 0,
            cleanup_interval: Duration::from_secs(60),
        };
        let limiter = RateLimiter::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        // 토큰 소진
        for _ in 0..100 {
            let _ = limiter.check(ip).await;
        }

        // 제한됨
        assert!(matches!(
            limiter.check(ip).await,
            RateLimitResult::Limited { .. }
        ));

        // 잠시 대기 후 토큰 리필
        tokio::time::sleep(Duration::from_millis(20)).await;

        // 일부 토큰이 리필되어 허용
        assert!(matches!(limiter.check(ip).await, RateLimitResult::Allowed));
    }

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.requests_per_minute, 1200);
        assert_eq!(config.burst_size, 50);
    }

    #[test]
    fn test_rate_limit_config_new() {
        let config = RateLimitConfig::new(600);
        assert_eq!(config.requests_per_minute, 600);
        assert_eq!(config.burst_size, 60); // 10%
    }

    #[test]
    fn test_rate_limit_config_strict() {
        let config = RateLimitConfig::strict(100);
        assert_eq!(config.requests_per_minute, 100);
        assert_eq!(config.burst_size, 0);
    }
}
