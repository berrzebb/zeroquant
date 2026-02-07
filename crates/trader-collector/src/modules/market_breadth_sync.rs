//! Market Breadth 동기화 모듈.
//!
//! DB에서 종목별 20일선 상회 비율을 계산하여 Redis 캐시에 저장합니다.
//! Collector Group C에서 5분 주기로 실행됩니다.

use sqlx::PgPool;
use std::time::Instant;
use tracing::{error, info, warn};

use trader_core::MarketBreadth;
use trader_data::cache::RedisCache;
use trader_data::MarketBreadthCalculator;

use crate::Result;

/// Market Breadth 캐시 키 (API와 동일)
const MARKET_BREADTH_CACHE_KEY: &str = "macro:market_breadth";

/// Market Breadth 캐시 TTL (10분)
/// 동기화 주기(5분)보다 충분히 길게 설정하여 계산 실패 시에도 이전 값 유지
const MARKET_BREADTH_CACHE_TTL_SECS: u64 = 600;

/// Market Breadth 동기화 결과.
#[derive(Debug)]
pub struct MarketBreadthSyncResult {
    /// 성공 여부
    pub success: bool,
    /// 전체 시장 비율 (% 문자열)
    pub all_pct: Option<String>,
    /// KOSPI 비율 (% 문자열)
    pub kospi_pct: Option<String>,
    /// KOSDAQ 비율 (% 문자열)
    pub kosdaq_pct: Option<String>,
    /// 소요 시간 (ms)
    pub elapsed_ms: u64,
    /// 에러 메시지 (실패 시)
    pub error: Option<String>,
}

/// Market Breadth 동기화.
///
/// MarketBreadthCalculator를 사용하여 DB에서 계산한 결과를
/// Redis 캐시에 저장합니다.
///
/// # 인자
/// * `pool` - PostgreSQL 연결 풀
/// * `cache` - Redis 캐시 인스턴스
///
/// # 반환
/// * 동기화 결과
pub async fn sync_market_breadth(pool: &PgPool, cache: &RedisCache) -> Result<MarketBreadthSyncResult> {
    let start = Instant::now();

    // MarketBreadthCalculator로 계산
    let calculator = MarketBreadthCalculator::new(pool.clone());
    let breadth: MarketBreadth = match calculator.calculate().await {
        Ok(b) => b,
        Err(e) => {
            warn!("Market Breadth 계산 실패: {}", e);
            return Ok(MarketBreadthSyncResult {
                success: false,
                all_pct: None,
                kospi_pct: None,
                kosdaq_pct: None,
                elapsed_ms: start.elapsed().as_millis() as u64,
                error: Some(format!("계산 실패: {}", e)),
            });
        }
    };

    // Redis 캐시에 MarketBreadth 도메인 모델 저장
    if let Err(e) = cache
        .set_with_ttl(MARKET_BREADTH_CACHE_KEY, &breadth, MARKET_BREADTH_CACHE_TTL_SECS)
        .await
    {
        error!("Market Breadth 캐시 저장 실패: {}", e);
        return Ok(MarketBreadthSyncResult {
            success: false,
            all_pct: Some(breadth.all_pct().to_string()),
            kospi_pct: Some(breadth.kospi_pct().to_string()),
            kosdaq_pct: Some(breadth.kosdaq_pct().to_string()),
            elapsed_ms: start.elapsed().as_millis() as u64,
            error: Some(format!("캐시 저장 실패: {}", e)),
        });
    }

    let elapsed_ms = start.elapsed().as_millis() as u64;

    info!(
        all = %breadth.all_pct(),
        kospi = %breadth.kospi_pct(),
        kosdaq = %breadth.kosdaq_pct(),
        temperature = %breadth.temperature,
        elapsed_ms = elapsed_ms,
        "Market Breadth 동기화 완료"
    );

    Ok(MarketBreadthSyncResult {
        success: true,
        all_pct: Some(breadth.all_pct().to_string()),
        kospi_pct: Some(breadth.kospi_pct().to_string()),
        kosdaq_pct: Some(breadth.kosdaq_pct().to_string()),
        elapsed_ms,
        error: None,
    })
}
