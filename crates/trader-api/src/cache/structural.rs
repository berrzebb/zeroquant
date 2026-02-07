//! 구조적 피처 캐싱.
//!
//! StructuralFeatures 계산 결과를 Redis에 캐싱하여 성능을 개선합니다.

use trader_core::domain::StructuralFeatures;
use trader_data::cache::RedisCache;
use trader_data::error::Result;

/// 구조적 피처 캐싱 래퍼.
#[derive(Clone)]
pub struct StructuralFeaturesCache {
    redis: RedisCache,
}

impl StructuralFeaturesCache {
    /// 새로운 캐시 인스턴스 생성.
    pub fn new(redis: RedisCache) -> Self {
        Self { redis }
    }

    /// 캐시에서 피처 조회.
    ///
    /// # 인자
    ///
    /// * `symbol` - 심볼 (예: "005930", "AAPL")
    /// * `timeframe` - 타임프레임 (예: "1d", "1h")
    ///
    /// # 반환
    ///
    /// 캐시된 피처 또는 None
    pub async fn get(&self, symbol: &str, timeframe: &str) -> Result<Option<StructuralFeatures>> {
        let key = Self::cache_key(symbol, timeframe);
        self.redis.get(&key).await
    }

    /// 피처를 캐시에 저장.
    ///
    /// # 인자
    ///
    /// * `symbol` - 심볼
    /// * `timeframe` - 타임프레임
    /// * `features` - 저장할 피처
    ///
    /// # TTL
    ///
    /// 타임프레임별 동적 TTL:
    /// - 1d: 6시간 (하루 4회 갱신)
    /// - 1h: 1시간
    /// - 5m: 5분
    pub async fn set(
        &self,
        symbol: &str,
        timeframe: &str,
        features: &StructuralFeatures,
    ) -> Result<()> {
        let key = Self::cache_key(symbol, timeframe);
        let ttl = Self::ttl_for_timeframe(timeframe);

        self.redis.set_with_ttl(&key, features, ttl).await
    }

    /// 특정 심볼의 모든 캐시 무효화.
    ///
    /// 패턴: `features:{symbol}:*`
    pub async fn invalidate_symbol(&self, symbol: &str) -> Result<()> {
        let pattern = format!("features:{}:*", symbol);
        let keys: Vec<String> = vec![pattern];

        for key in keys {
            let _ = self.redis.delete(&key).await;
        }

        Ok(())
    }

    /// 캐시 키 생성.
    ///
    /// 형식: `features:{symbol}:{timeframe}`
    fn cache_key(symbol: &str, timeframe: &str) -> String {
        format!("features:{}:{}", symbol, timeframe)
    }

    /// 타임프레임별 TTL 결정 (초 단위).
    fn ttl_for_timeframe(timeframe: &str) -> u64 {
        match timeframe {
            "1m" => 60,     // 1분
            "5m" => 300,    // 5분
            "15m" => 900,   // 15분
            "1h" => 3600,   // 1시간
            "4h" => 14400,  // 4시간
            "1d" => 21600,  // 6시간 (하루 4회 갱신)
            "1w" => 86400,  // 1일
            "1M" => 604800, // 1주
            _ => 3600,      // 기본값: 1시간
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_format() {
        let key = StructuralFeaturesCache::cache_key("005930", "1d");
        assert_eq!(key, "features:005930:1d");
    }

    #[test]
    fn test_ttl_for_timeframe() {
        assert_eq!(StructuralFeaturesCache::ttl_for_timeframe("1d"), 21600); // 6시간
        assert_eq!(StructuralFeaturesCache::ttl_for_timeframe("1h"), 3600); // 1시간
        assert_eq!(StructuralFeaturesCache::ttl_for_timeframe("5m"), 300); // 5분
        assert_eq!(StructuralFeaturesCache::ttl_for_timeframe("unknown"), 3600);
        // 기본값
    }
}
