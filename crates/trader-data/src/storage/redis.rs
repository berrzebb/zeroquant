//! Redis cache 구현.
//!
//! 자주 접근하는 시장 데이터에 대한 cache 레이어를 제공하여
//! 데이터베이스 부하를 줄이고 응답 시간을 개선합니다.

use crate::error::{DataError, Result};
use redis::{aio::MultiplexedConnection, AsyncCommands, Client};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tracing::{info, instrument, warn};
use trader_core::{Kline, OrderBook, Ticker, Timeframe};

/// Redis 설정.
#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    /// Redis URL (redis://user:password@host:port/db)
    pub url: String,
    /// cache 항목의 기본 TTL (초 단위)
    #[serde(default = "default_ttl")]
    pub default_ttl_secs: u64,
    /// 연결 풀 크기
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,
}

fn default_ttl() -> u64 {
    300 // 5 minutes
}
fn default_pool_size() -> usize {
    10
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379/0".to_string(),
            default_ttl_secs: default_ttl(),
            pool_size: default_pool_size(),
        }
    }
}

/// Redis 연결 래퍼.
///
/// `MultiplexedConnection`은 내부적으로 thread-safe하고 Clone 가능하므로
/// 별도의 `RwLock` 없이 동시 접근이 가능합니다.
#[derive(Clone)]
pub struct RedisCache {
    client: Client,
    /// Multiplexed connection은 Clone 시 같은 연결을 공유합니다.
    /// 내부적으로 tokio mpsc channel을 사용하여 비동기 요청을 처리합니다.
    connection: MultiplexedConnection,
    config: RedisConfig,
}

impl RedisCache {
    /// 새로운 Redis cache 연결을 생성합니다.
    ///
    /// `MultiplexedConnection`은 내부적으로 연결을 공유하며,
    /// Clone 시에도 같은 연결을 사용하므로 락 없이 동시 접근이 가능합니다.
    pub async fn connect(config: &RedisConfig) -> Result<Self> {
        info!("Connecting to Redis...");

        let client =
            Client::open(config.url.as_str()).map_err(|e| DataError::CacheError(e.to_string()))?;

        let connection = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| DataError::CacheError(e.to_string()))?;

        info!("Redis connection established (non-blocking multiplexed)");

        Ok(Self {
            client,
            connection,
            config: config.clone(),
        })
    }

    /// Redis 상태를 확인합니다.
    pub async fn health_check(&self) -> Result<bool> {
        let mut conn = self.connection.clone();
        let result: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| DataError::CacheError(e.to_string()))?;

        Ok(result == "PONG")
    }

    // =========================================================================
    // 일반 Cache 작업
    // =========================================================================

    /// cache에서 값을 가져옵니다.
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let mut conn = self.connection.clone();
        let value: Option<String> = conn
            .get(key)
            .await
            .map_err(|e| DataError::CacheError(e.to_string()))?;

        match value {
            Some(json) => {
                let parsed = serde_json::from_str(&json)
                    .map_err(|e| DataError::SerializationError(e.to_string()))?;
                Ok(Some(parsed))
            }
            None => Ok(None),
        }
    }

    /// 기본 TTL로 cache에 값을 설정합니다.
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.set_with_ttl(key, value, self.config.default_ttl_secs)
            .await
    }

    /// 사용자 정의 TTL로 cache에 값을 설정합니다.
    pub async fn set_with_ttl<T: Serialize + ?Sized>(
        &self,
        key: &str,
        value: &T,
        ttl_secs: u64,
    ) -> Result<()> {
        let json = serde_json::to_string(value)
            .map_err(|e| DataError::SerializationError(e.to_string()))?;

        let mut conn = self.connection.clone();
        let _: () = conn
            .set_ex(key, json, ttl_secs)
            .await
            .map_err(|e| DataError::CacheError(e.to_string()))?;

        Ok(())
    }

    /// cache에서 키를 삭제합니다.
    pub async fn delete(&self, key: &str) -> Result<bool> {
        let mut conn = self.connection.clone();
        let deleted: i64 = conn
            .del(key)
            .await
            .map_err(|e| DataError::CacheError(e.to_string()))?;

        Ok(deleted > 0)
    }

    /// 키가 존재하는지 확인합니다.
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.connection.clone();
        let exists: bool = conn
            .exists(key)
            .await
            .map_err(|e| DataError::CacheError(e.to_string()))?;

        Ok(exists)
    }

    /// 기존 키에 TTL을 설정합니다.
    pub async fn expire(&self, key: &str, ttl_secs: u64) -> Result<bool> {
        let mut conn = self.connection.clone();
        let result: bool = conn
            .expire(key, ttl_secs as i64)
            .await
            .map_err(|e| DataError::CacheError(e.to_string()))?;

        Ok(result)
    }

    /// 패턴과 일치하는 키들을 삭제합니다.
    pub async fn delete_pattern(&self, pattern: &str) -> Result<usize> {
        let mut conn = self.connection.clone();
        let keys: Vec<String> = conn
            .keys(pattern)
            .await
            .map_err(|e| DataError::CacheError(e.to_string()))?;

        if keys.is_empty() {
            return Ok(0);
        }

        let deleted: i64 = conn
            .del(&keys)
            .await
            .map_err(|e| DataError::CacheError(e.to_string()))?;

        Ok(deleted as usize)
    }

    // =========================================================================
    // Ticker Cache
    // =========================================================================

    /// ticker용 cache 키.
    fn ticker_key(exchange: &str, symbol: &str) -> String {
        format!("ticker:{}:{}", exchange, symbol)
    }

    /// ticker를 cache에 저장합니다.
    #[instrument(skip(self, ticker))]
    pub async fn set_ticker(&self, exchange: &str, ticker: &Ticker) -> Result<()> {
        let key = Self::ticker_key(exchange, &ticker.ticker.to_string());
        // Ticker 데이터는 시간에 매우 민감하므로 짧은 TTL 사용
        self.set_with_ttl(&key, ticker, 10).await
    }

    /// cache된 ticker를 가져옵니다.
    pub async fn get_ticker(&self, exchange: &str, symbol: &str) -> Result<Option<Ticker>> {
        let key = Self::ticker_key(exchange, symbol);
        self.get(&key).await
    }

    // =========================================================================
    // Order Book Cache
    // =========================================================================

    /// order book용 cache 키.
    fn orderbook_key(exchange: &str, symbol: &str) -> String {
        format!("orderbook:{}:{}", exchange, symbol)
    }

    /// order book을 cache에 저장합니다.
    #[instrument(skip(self, orderbook))]
    pub async fn set_orderbook(&self, exchange: &str, orderbook: &OrderBook) -> Result<()> {
        let key = Self::orderbook_key(exchange, &orderbook.ticker);
        // Order book 데이터는 시간에 매우 민감하므로 짧은 TTL 사용
        self.set_with_ttl(&key, orderbook, 5).await
    }

    /// cache된 order book을 가져옵니다.
    pub async fn get_orderbook(&self, exchange: &str, symbol: &str) -> Result<Option<OrderBook>> {
        let key = Self::orderbook_key(exchange, symbol);
        self.get(&key).await
    }

    // =========================================================================
    // Kline Cache
    // =========================================================================

    /// kline용 cache 키.
    fn klines_key(exchange: &str, symbol: &str, timeframe: &Timeframe) -> String {
        format!("klines:{}:{}:{}", exchange, symbol, timeframe)
    }

    /// kline을 cache에 저장합니다.
    #[instrument(skip(self, klines), fields(count = klines.len()))]
    pub async fn set_klines(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: &Timeframe,
        klines: &[Kline],
    ) -> Result<()> {
        let key = Self::klines_key(exchange, symbol, timeframe);
        // Kline은 과거 데이터이므로 더 오래 cache할 수 있음
        let ttl = match timeframe {
            Timeframe::M1 => 60,       // 1분봉은 1분
            Timeframe::M3 => 180,      // 3분봉은 3분
            Timeframe::M5 => 300,      // 5분봉은 5분
            Timeframe::M15 => 900,     // 15분봉은 15분
            Timeframe::M30 => 1800,    // 30분봉은 30분
            Timeframe::H1 => 3600,     // 1시간봉은 1시간
            Timeframe::H2 => 7200,     // 2시간봉은 2시간
            Timeframe::H4 => 14400,    // 4시간봉은 4시간
            Timeframe::H6 => 21600,    // 6시간봉은 6시간
            Timeframe::H8 => 28800,    // 8시간봉은 8시간
            Timeframe::H12 => 43200,   // 12시간봉은 12시간
            Timeframe::D1 => 86400,    // 일봉은 1일
            Timeframe::D3 => 259200,   // 3일봉은 3일
            Timeframe::W1 => 604800,   // 주봉은 1주
            Timeframe::MN1 => 2592000, // 월봉은 30일
        };
        self.set_with_ttl(&key, klines, ttl).await
    }

    /// cache된 kline을 가져옵니다.
    pub async fn get_klines(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: &Timeframe,
    ) -> Result<Option<Vec<Kline>>> {
        let key = Self::klines_key(exchange, symbol, timeframe);
        self.get(&key).await
    }

    /// cache된 kline에 새 kline을 추가합니다.
    pub async fn append_kline(
        &self,
        exchange: &str,
        symbol: &str,
        timeframe: &Timeframe,
        kline: &Kline,
        max_count: usize,
    ) -> Result<()> {
        let key = Self::klines_key(exchange, symbol, timeframe);

        let mut klines: Vec<Kline> = self.get(&key).await?.unwrap_or_default();

        // 마지막 캔들을 업데이트할지 새로 추가할지 확인
        if let Some(last) = klines.last() {
            if last.open_time == kline.open_time {
                // 기존 캔들 업데이트
                klines.pop();
            }
        }

        klines.push(kline.clone());

        // 가장 최근 캔들만 유지
        if klines.len() > max_count {
            klines = klines.split_off(klines.len() - max_count);
        }

        self.set_klines(exchange, symbol, timeframe, &klines).await
    }

    /// 여러 타임프레임의 kline을 병렬로 조회합니다.
    ///
    /// # 인자
    ///
    /// * `exchange` - 거래소 ID
    /// * `symbol` - 심볼
    /// * `timeframes` - 조회할 타임프레임 목록
    ///
    /// # 반환
    ///
    /// 타임프레임별 캔들 데이터 HashMap. 캐시 미스인 경우 해당 TF는 빈 Vec.
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// let klines = cache.get_multi_klines(
    ///     "binance",
    ///     "BTCUSDT",
    ///     &[Timeframe::M5, Timeframe::H1, Timeframe::D1],
    /// ).await?;
    ///
    /// for (tf, data) in &klines {
    ///     println!("{:?}: {} candles", tf, data.len());
    /// }
    /// ```
    #[instrument(skip(self), fields(timeframe_count = timeframes.len()))]
    pub async fn get_multi_klines(
        &self,
        exchange: &str,
        symbol: &str,
        timeframes: &[Timeframe],
    ) -> Result<std::collections::HashMap<Timeframe, Vec<Kline>>> {
        use futures::future::join_all;

        // 각 타임프레임별 조회를 병렬로 실행
        let futures: Vec<_> = timeframes
            .iter()
            .map(|tf| {
                let exchange = exchange.to_string();
                let symbol = symbol.to_string();
                let tf = *tf;
                async move {
                    let result = self.get_klines(&exchange, &symbol, &tf).await;
                    (tf, result)
                }
            })
            .collect();

        let results = join_all(futures).await;

        // 결과를 HashMap으로 변환
        let mut map = std::collections::HashMap::new();
        for (tf, result) in results {
            match result {
                Ok(Some(klines)) => {
                    map.insert(tf, klines);
                }
                Ok(None) => {
                    // 캐시 미스: 빈 Vec 삽입
                    map.insert(tf, Vec::new());
                }
                Err(e) => {
                    // 에러 발생 시 경고 로그 후 빈 Vec
                    warn!("Failed to get klines for {:?}: {}", tf, e);
                    map.insert(tf, Vec::new());
                }
            }
        }

        Ok(map)
    }

    /// 여러 타임프레임의 kline을 병렬로 저장합니다.
    ///
    /// # 인자
    ///
    /// * `exchange` - 거래소 ID
    /// * `symbol` - 심볼
    /// * `data` - 타임프레임별 캔들 데이터
    #[instrument(skip(self, data), fields(timeframe_count = data.len()))]
    pub async fn set_multi_klines(
        &self,
        exchange: &str,
        symbol: &str,
        data: &[(Timeframe, Vec<Kline>)],
    ) -> Result<()> {
        use futures::future::join_all;

        let futures: Vec<_> = data
            .iter()
            .map(|(tf, klines)| {
                let exchange = exchange.to_string();
                let symbol = symbol.to_string();
                let tf = *tf;
                let klines = klines.clone();
                async move { self.set_klines(&exchange, &symbol, &tf, &klines).await }
            })
            .collect();

        let results = join_all(futures).await;

        // 하나라도 실패하면 에러 반환
        for result in results {
            result?;
        }

        Ok(())
    }

    // =========================================================================
    // Symbol 정보 Cache
    // =========================================================================

    /// symbol 정보용 cache 키.
    fn symbol_info_key(exchange: &str, symbol: &str) -> String {
        format!("symbol:{}:{}", exchange, symbol)
    }

    /// symbol 정보를 cache에 저장합니다.
    pub async fn set_symbol_info<T: Serialize>(
        &self,
        exchange: &str,
        symbol: &str,
        info: &T,
    ) -> Result<()> {
        let key = Self::symbol_info_key(exchange, symbol);
        // Symbol 정보는 자주 변경되지 않으므로 더 긴 TTL 사용
        self.set_with_ttl(&key, info, 3600).await
    }

    /// cache된 symbol 정보를 가져옵니다.
    pub async fn get_symbol_info<T: DeserializeOwned>(
        &self,
        exchange: &str,
        symbol: &str,
    ) -> Result<Option<T>> {
        let key = Self::symbol_info_key(exchange, symbol);
        self.get(&key).await
    }

    // =========================================================================
    // Rate Limit 추적
    // =========================================================================

    /// Rate limit 키.
    fn rate_limit_key(exchange: &str, endpoint: &str) -> String {
        format!("ratelimit:{}:{}", exchange, endpoint)
    }

    /// Rate limit 카운터를 증가시킵니다.
    pub async fn increment_rate_limit(
        &self,
        exchange: &str,
        endpoint: &str,
        window_secs: u64,
    ) -> Result<i64> {
        let key = Self::rate_limit_key(exchange, endpoint);
        let mut conn = self.connection.clone();

        // 원자적으로 증가시키고 만료 시간 설정
        let count: i64 = conn
            .incr(&key, 1)
            .await
            .map_err(|e| DataError::CacheError(e.to_string()))?;

        if count == 1 {
            // 이 윈도우에서 첫 번째 요청, 만료 시간 설정
            let _: () = conn
                .expire(&key, window_secs as i64)
                .await
                .map_err(|e| DataError::CacheError(e.to_string()))?;
        }

        Ok(count)
    }

    /// 현재 rate limit 카운트를 가져옵니다.
    pub async fn get_rate_limit_count(&self, exchange: &str, endpoint: &str) -> Result<i64> {
        let key = Self::rate_limit_key(exchange, endpoint);
        let mut conn = self.connection.clone();

        let count: Option<i64> = conn
            .get(&key)
            .await
            .map_err(|e| DataError::CacheError(e.to_string()))?;

        Ok(count.unwrap_or(0))
    }

    // =========================================================================
    // 실시간 데이터용 Pub/Sub
    // =========================================================================

    /// 채널에 메시지를 발행합니다.
    pub async fn publish<T: Serialize>(&self, channel: &str, message: &T) -> Result<()> {
        let json = serde_json::to_string(message)
            .map_err(|e| DataError::SerializationError(e.to_string()))?;

        let mut conn = self.connection.clone();
        let _: () = conn
            .publish(channel, json)
            .await
            .map_err(|e| DataError::CacheError(e.to_string()))?;

        Ok(())
    }

    /// 구독용 pubsub 연결을 가져옵니다.
    pub async fn get_pubsub(&self) -> Result<redis::aio::PubSub> {
        let pubsub = self
            .client
            .get_async_pubsub()
            .await
            .map_err(|e| DataError::CacheError(e.to_string()))?;

        Ok(pubsub)
    }

    // =========================================================================
    // 분산 잠금
    // =========================================================================

    /// 분산 잠금을 획득합니다.
    pub async fn acquire_lock(&self, lock_name: &str, ttl_secs: u64) -> Result<bool> {
        let key = format!("lock:{}", lock_name);
        let mut conn = self.connection.clone();

        // 원자적 잠금 획득을 위해 SET NX EX 사용
        let result: Option<String> = redis::cmd("SET")
            .arg(&key)
            .arg("locked")
            .arg("NX")
            .arg("EX")
            .arg(ttl_secs)
            .query_async(&mut conn)
            .await
            .map_err(|e| DataError::CacheError(e.to_string()))?;

        Ok(result.is_some())
    }

    /// 분산 잠금을 해제합니다.
    pub async fn release_lock(&self, lock_name: &str) -> Result<bool> {
        let key = format!("lock:{}", lock_name);
        self.delete(&key).await
    }

    // =========================================================================
    // 세션 관리
    // =========================================================================

    /// 세션을 저장합니다.
    pub async fn set_session(
        &self,
        session_id: &str,
        data: &impl Serialize,
        ttl_secs: u64,
    ) -> Result<()> {
        let key = format!("session:{}", session_id);
        self.set_with_ttl(&key, data, ttl_secs).await
    }

    /// 세션을 가져옵니다.
    pub async fn get_session<T: DeserializeOwned>(&self, session_id: &str) -> Result<Option<T>> {
        let key = format!("session:{}", session_id);
        self.get(&key).await
    }

    /// 세션을 삭제합니다.
    pub async fn delete_session(&self, session_id: &str) -> Result<bool> {
        let key = format!("session:{}", session_id);
        self.delete(&key).await
    }

    /// 세션 TTL을 연장합니다.
    pub async fn extend_session(&self, session_id: &str, ttl_secs: u64) -> Result<bool> {
        let key = format!("session:{}", session_id);
        self.expire(&key, ttl_secs).await
    }
}

// =============================================================================
// Cache 메트릭
// =============================================================================

/// Cache 통계.
#[derive(Debug, Default, Clone, Serialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

/// 메트릭 추적이 포함된 cache.
pub struct MetricsCache {
    cache: RedisCache,
    hits: std::sync::atomic::AtomicU64,
    misses: std::sync::atomic::AtomicU64,
}

impl MetricsCache {
    pub fn new(cache: RedisCache) -> Self {
        Self {
            cache,
            hits: std::sync::atomic::AtomicU64::new(0),
            misses: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// 메트릭 추적과 함께 값을 가져옵니다.
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let result = self.cache.get(key).await?;
        if result.is_some() {
            self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        } else {
            self.misses
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(result)
    }

    /// 값을 설정합니다.
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.cache.set(key, value).await
    }

    /// Cache 통계를 가져옵니다.
    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.misses.load(std::sync::atomic::Ordering::Relaxed);
        let total = hits + misses;
        let hit_rate = if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        };

        CacheStats {
            hits,
            misses,
            hit_rate,
        }
    }

    /// 통계를 초기화합니다.
    pub fn reset_stats(&self) {
        self.hits.store(0, std::sync::atomic::Ordering::Relaxed);
        self.misses.store(0, std::sync::atomic::Ordering::Relaxed);
    }

    /// 내부 cache를 가져옵니다.
    pub fn inner(&self) -> &RedisCache {
        &self.cache
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RedisConfig::default();
        assert_eq!(config.default_ttl_secs, 300);
        assert_eq!(config.pool_size, 10);
    }

    #[test]
    fn test_cache_keys() {
        assert_eq!(
            RedisCache::ticker_key("binance", "BTCUSDT"),
            "ticker:binance:BTCUSDT"
        );
        assert_eq!(
            RedisCache::orderbook_key("binance", "ETHUSDT"),
            "orderbook:binance:ETHUSDT"
        );
    }
}
