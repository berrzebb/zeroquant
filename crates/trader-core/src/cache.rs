//! 공용 캐시 인프라.
//!
//! 거래소 Provider 등에서 공통으로 사용할 수 있는 TTL 기반 캐시를 제공합니다.
//!
//! # 구조
//!
//! ```text
//! TtlCache<T>          // 범용 TTL 캐시 (어디서든 사용 가능)
//! ExchangeCache        // 거래소 데이터 전용 캐시 묶음
//! ├── account          // 계좌 정보
//! ├── positions        // 보유 포지션
//! └── pending_orders   // 미체결 주문
//! ```
//!
//! # 사용 패턴
//!
//! ```text
//! // ExchangeProvider 구현에서 캐시 사용
//! async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
//!     if let Some(cached) = self.cache.get_account().await {
//!         return Ok(cached);
//!     }
//!     let data = self.api_call().await?;
//!     self.cache.set_account(data.clone()).await;
//!     Ok(data)
//! }
//!
//! // OrderExecutionProvider에서 주문 후 캐시 무효화
//! async fn place_order(&self, ...) -> Result<...> {
//!     let result = self.api_call().await?;
//!     self.cache.invalidate_all().await;
//!     Ok(result)
//! }
//! ```

use std::fmt;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::domain::{PendingOrder, StrategyAccountInfo, StrategyPositionInfo};

// ==================== TtlCache<T> ====================

/// TTL 기반 범용 캐시.
///
/// 지정된 TTL(Time-To-Live) 후 자동 만료되는 스레드 안전 캐시입니다.
/// `get()`은 TTL이 지나면 `None`을 반환하고, `set()`으로 새 값을 저장합니다.
///
/// # 스레드 안전성
///
/// 내부적으로 `RwLock`을 사용하여 다중 읽기 / 단일 쓰기를 보장합니다.
pub struct TtlCache<T> {
    data: RwLock<Option<TtlEntry<T>>>,
    ttl: Duration,
}

/// 캐시 내부 저장 항목.
struct TtlEntry<T> {
    data: T,
    created_at: Instant,
}

impl<T: Clone + Send + Sync> TtlCache<T> {
    /// 지정된 TTL로 빈 캐시 생성.
    pub fn new(ttl: Duration) -> Self {
        Self {
            data: RwLock::new(None),
            ttl,
        }
    }

    /// 캐시된 값 조회.
    ///
    /// TTL이 만료되었으면 `None`을 반환합니다.
    pub async fn get(&self) -> Option<T> {
        let guard = self.data.read().await;
        guard.as_ref().and_then(|entry| {
            if entry.created_at.elapsed() < self.ttl {
                Some(entry.data.clone())
            } else {
                None
            }
        })
    }

    /// 값을 캐시에 저장.
    ///
    /// 기존 값이 있으면 덮어씁니다.
    pub async fn set(&self, data: T) {
        let mut guard = self.data.write().await;
        *guard = Some(TtlEntry {
            data,
            created_at: Instant::now(),
        });
    }

    /// 캐시 무효화.
    ///
    /// 다음 `get()` 호출 시 `None`이 반환됩니다.
    pub async fn invalidate(&self) {
        let mut guard = self.data.write().await;
        *guard = None;
    }

    /// 캐시에 유효한 값이 있는지 확인.
    pub async fn is_valid(&self) -> bool {
        let guard = self.data.read().await;
        guard
            .as_ref()
            .map(|entry| entry.created_at.elapsed() < self.ttl)
            .unwrap_or(false)
    }

    /// 남은 TTL 시간 (초).
    ///
    /// 캐시가 비어있거나 만료되었으면 `None`.
    pub async fn remaining_ttl_secs(&self) -> Option<f64> {
        let guard = self.data.read().await;
        guard.as_ref().and_then(|entry| {
            let elapsed = entry.created_at.elapsed();
            if elapsed < self.ttl {
                Some((self.ttl - elapsed).as_secs_f64())
            } else {
                None
            }
        })
    }
}

impl<T> fmt::Debug for TtlCache<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TtlCache")
            .field("ttl", &self.ttl)
            .finish()
    }
}

// ==================== ExchangeCache ====================

/// 거래소 캐시 설정.
#[derive(Debug, Clone)]
pub struct ExchangeCacheConfig {
    /// 계좌 정보 TTL
    pub account_ttl: Duration,
    /// 포지션 TTL
    pub positions_ttl: Duration,
    /// 미체결 주문 TTL
    pub pending_orders_ttl: Duration,
}

impl Default for ExchangeCacheConfig {
    fn default() -> Self {
        Self {
            account_ttl: Duration::from_secs(30),
            positions_ttl: Duration::from_secs(30),
            pending_orders_ttl: Duration::from_secs(30),
        }
    }
}

/// 거래소 데이터 공용 캐시.
///
/// `ExchangeProvider`와 `OrderExecutionProvider`를 동시에 구현하는
/// Provider에서 공유하여 사용합니다.
///
/// # 캐시 무효화 전략
///
/// - 주문 제출/취소/정정 시 `invalidate_all()` 호출
/// - TTL 만료 시 자동 갱신 (외부 수동 주문 감지용 안전장치)
///
/// # 사용 예시
///
/// ```text
/// struct MyExchangeProvider {
///     client: MyClient,
///     cache: Arc<ExchangeCache>,
/// }
///
/// // ExchangeProvider 구현
/// async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
///     if let Some(cached) = self.cache.get_positions().await {
///         return Ok(cached);
///     }
///     let positions = self.client.get_positions().await?;
///     self.cache.set_positions(positions.clone()).await;
///     Ok(positions)
/// }
///
/// // OrderExecutionProvider 구현
/// async fn place_order(&self, ...) -> Result<OrderResponse, ProviderError> {
///     let result = self.client.place_order(...).await?;
///     self.cache.invalidate_all().await;  // 주문 후 캐시 무효화
///     Ok(result)
/// }
/// ```
pub struct ExchangeCache {
    /// 계좌 정보 캐시
    account: TtlCache<StrategyAccountInfo>,
    /// 포지션 캐시
    positions: TtlCache<Vec<StrategyPositionInfo>>,
    /// 미체결 주문 캐시
    pending_orders: TtlCache<Vec<PendingOrder>>,
}

impl ExchangeCache {
    /// 설정 기반 캐시 생성.
    pub fn new(config: ExchangeCacheConfig) -> Self {
        Self {
            account: TtlCache::new(config.account_ttl),
            positions: TtlCache::new(config.positions_ttl),
            pending_orders: TtlCache::new(config.pending_orders_ttl),
        }
    }

    /// 기본 설정으로 캐시 생성 (모든 TTL 30초).
    pub fn with_defaults() -> Self {
        Self::new(ExchangeCacheConfig::default())
    }

    // ===== 계좌 정보 =====

    /// 캐시된 계좌 정보 조회.
    pub async fn get_account(&self) -> Option<StrategyAccountInfo> {
        self.account.get().await
    }

    /// 계좌 정보 캐시 저장.
    pub async fn set_account(&self, data: StrategyAccountInfo) {
        self.account.set(data).await;
    }

    // ===== 포지션 =====

    /// 캐시된 포지션 조회.
    pub async fn get_positions(&self) -> Option<Vec<StrategyPositionInfo>> {
        self.positions.get().await
    }

    /// 포지션 캐시 저장.
    pub async fn set_positions(&self, data: Vec<StrategyPositionInfo>) {
        self.positions.set(data).await;
    }

    // ===== 미체결 주문 =====

    /// 캐시된 미체결 주문 조회.
    pub async fn get_pending_orders(&self) -> Option<Vec<PendingOrder>> {
        self.pending_orders.get().await
    }

    /// 미체결 주문 캐시 저장.
    pub async fn set_pending_orders(&self, data: Vec<PendingOrder>) {
        self.pending_orders.set(data).await;
    }

    // ===== 무효화 =====

    /// 모든 캐시 무효화.
    ///
    /// 주문 제출/취소/정정 후 호출하여
    /// 다음 동기화 사이클에서 최신 데이터를 조회합니다.
    pub async fn invalidate_all(&self) {
        self.account.invalidate().await;
        self.positions.invalidate().await;
        self.pending_orders.invalidate().await;
        tracing::debug!("ExchangeCache 전체 무효화 완료");
    }

    /// 계좌 정보만 무효화.
    pub async fn invalidate_account(&self) {
        self.account.invalidate().await;
    }

    /// 포지션만 무효화.
    pub async fn invalidate_positions(&self) {
        self.positions.invalidate().await;
    }

    /// 미체결 주문만 무효화.
    pub async fn invalidate_pending_orders(&self) {
        self.pending_orders.invalidate().await;
    }
}

impl fmt::Debug for ExchangeCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExchangeCache")
            .field("account", &self.account)
            .field("positions", &self.positions)
            .field("pending_orders", &self.pending_orders)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[tokio::test]
    async fn ttl_cache_basic_operations() {
        let cache: TtlCache<String> = TtlCache::new(Duration::from_secs(10));

        // 초기 상태: 비어있음
        assert!(cache.get().await.is_none());
        assert!(!cache.is_valid().await);

        // 값 저장
        cache.set("hello".to_string()).await;
        assert_eq!(cache.get().await, Some("hello".to_string()));
        assert!(cache.is_valid().await);

        // 남은 TTL 확인
        let remaining = cache.remaining_ttl_secs().await;
        assert!(remaining.is_some());
        assert!(remaining.unwrap() > 9.0);
    }

    #[tokio::test]
    async fn ttl_cache_invalidation() {
        let cache: TtlCache<i32> = TtlCache::new(Duration::from_secs(10));

        cache.set(42).await;
        assert_eq!(cache.get().await, Some(42));

        // 무효화
        cache.invalidate().await;
        assert!(cache.get().await.is_none());
        assert!(!cache.is_valid().await);
    }

    #[tokio::test]
    async fn ttl_cache_expiration() {
        let cache: TtlCache<i32> = TtlCache::new(Duration::from_millis(50));

        cache.set(42).await;
        assert_eq!(cache.get().await, Some(42));

        // TTL 만료 대기
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(cache.get().await.is_none());
    }

    #[tokio::test]
    async fn ttl_cache_overwrite() {
        let cache: TtlCache<String> = TtlCache::new(Duration::from_secs(10));

        cache.set("first".to_string()).await;
        cache.set("second".to_string()).await;
        assert_eq!(cache.get().await, Some("second".to_string()));
    }

    #[tokio::test]
    async fn exchange_cache_basic() {
        let cache = ExchangeCache::with_defaults();

        // 초기 상태: 모두 비어있음
        assert!(cache.get_account().await.is_none());
        assert!(cache.get_positions().await.is_none());
        assert!(cache.get_pending_orders().await.is_none());

        // 계좌 정보 저장
        let account = StrategyAccountInfo {
            total_balance: Decimal::from(1_000_000),
            available_balance: Decimal::from(500_000),
            margin_used: Decimal::ZERO,
            unrealized_pnl: Decimal::from(50_000),
            currency: "KRW".to_string(),
        };
        cache.set_account(account.clone()).await;
        let cached = cache.get_account().await.unwrap();
        assert_eq!(cached.total_balance, Decimal::from(1_000_000));
    }

    #[tokio::test]
    async fn exchange_cache_invalidate_all() {
        let cache = ExchangeCache::with_defaults();

        // 모든 캐시에 데이터 저장
        cache
            .set_account(StrategyAccountInfo {
                total_balance: Decimal::from(100),
                available_balance: Decimal::from(50),
                margin_used: Decimal::ZERO,
                unrealized_pnl: Decimal::ZERO,
                currency: "KRW".to_string(),
            })
            .await;
        cache.set_positions(vec![]).await;
        cache.set_pending_orders(vec![]).await;

        // 전체 무효화
        cache.invalidate_all().await;

        assert!(cache.get_account().await.is_none());
        assert!(cache.get_positions().await.is_none());
        assert!(cache.get_pending_orders().await.is_none());
    }

    #[tokio::test]
    async fn exchange_cache_selective_invalidation() {
        let cache = ExchangeCache::with_defaults();

        cache
            .set_account(StrategyAccountInfo {
                total_balance: Decimal::from(100),
                available_balance: Decimal::from(50),
                margin_used: Decimal::ZERO,
                unrealized_pnl: Decimal::ZERO,
                currency: "KRW".to_string(),
            })
            .await;
        cache.set_positions(vec![]).await;
        cache.set_pending_orders(vec![]).await;

        // 미체결 주문만 무효화
        cache.invalidate_pending_orders().await;

        assert!(cache.get_account().await.is_some()); // 유지
        assert!(cache.get_positions().await.is_some()); // 유지
        assert!(cache.get_pending_orders().await.is_none()); // 무효화됨
    }
}
