//! 캐시 기반 과거 데이터 제공자.
//!
//! Yahoo Finance와 DB 캐시를 통합하여 효율적인 데이터 접근을 제공합니다.
//!
//! # 주요 기능
//!
//! - **동시성 제어**: 같은 심볼+타임프레임 중복 요청 방지
//! - **시장 시간 체크**: 마감 후 불필요한 API 호출 방지
//! - **갭 감지**: 누락된 캔들 자동 감지
//! - **증분 업데이트**: 새 데이터만 가져와 캐시
//!
//! # 동작 흐름
//!
//! ```text
//! 요청 (symbol, timeframe, limit)
//!         │
//!         ▼
//! ┌───────────────────┐
//! │ 1. 동시성 Lock 획득 │ ← 같은 심볼+TF는 하나만 처리
//! └─────────┬─────────┘
//!           │
//! ┌─────────▼─────────┐
//! │ 2. 시장 시간 체크   │ ← 마감 후 1시간 이내인가?
//! └─────────┬─────────┘
//!           │
//!     ┌─────┴─────┐
//!     │ 캐시 충분? │
//!     └─────┬─────┘
//!       YES │ NO
//!           │   │
//!           │   ▼
//!           │ ┌─────────────────────┐
//!           │ │ 3. Yahoo Finance    │
//!           │ │    증분 업데이트     │
//!           │ └──────────┬──────────┘
//!           │            │
//!           │   ┌────────▼────────┐
//!           │   │ 4. 갭 감지/경고  │
//!           │   └────────┬────────┘
//!           │            │
//!           ▼            ▼
//!     ┌─────────────────────┐
//!     │ 5. 캐시에서 반환     │
//!     └─────────────────────┘
//! ```

use crate::error::{DataError, Result};
use crate::provider::SymbolResolver;
use crate::storage::ohlcv::{timeframe_to_string, OhlcvCache};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use std::sync::Arc;
use time::OffsetDateTime;
use tracing::{debug, info, instrument, warn};
use trader_core::{Kline, Timeframe};

// =============================================================================
// 상장폐지 감지 상수 및 함수
// =============================================================================

/// 상장폐지 오류 패턴.
/// Yahoo Finance 및 기타 데이터 소스에서 반환하는 상장폐지 관련 오류 메시지 패턴.
pub const DELISTED_ERROR_PATTERNS: &[&str] = &[
    "symbol may be delisted",
    "No data found",
    "Not Found",
    "delisted",
    "invalid symbol",
    "No timezone found",
    "status code: 404",
];

/// 오류 메시지가 상장폐지 관련인지 확인.
pub fn is_delisted_error(error_message: &str) -> bool {
    let lower = error_message.to_lowercase();
    DELISTED_ERROR_PATTERNS
        .iter()
        .any(|p| lower.contains(&p.to_lowercase()))
}

/// 거래소 API Rate Limit 설정.
pub struct ExchangeRateLimits {
    /// 요청 간 최소 대기 시간 (밀리초)
    pub min_delay_ms: u64,
    /// 분당 최대 요청 수
    pub max_requests_per_minute: u32,
}

impl Default for ExchangeRateLimits {
    fn default() -> Self {
        Self {
            min_delay_ms: 500,           // 500ms 기본 딜레이
            max_requests_per_minute: 10, // 분당 10회
        }
    }
}

/// 캐시 기반 과거 데이터 제공자.
///
/// 요청 기반 자동 캐싱과 증분 업데이트를 제공합니다.
/// 모든 심볼은 canonical 형식으로 처리되며, SymbolResolver를 통해
/// 각 데이터 소스에 맞는 형식으로 변환됩니다.
///
/// # 3계층 캐시 구조
///
/// ```text
/// 요청 → Redis (ms) → PostgreSQL (10ms) → 외부 API (100ms+)
/// ```
///
/// - Redis: 핫 데이터 캐싱 (타임프레임별 TTL)
/// - PostgreSQL: 영구 저장 (ohlcv 테이블)
/// - 외부 API: Yahoo Finance, KRX
pub struct CachedHistoricalDataProvider {
    cache: OhlcvCache,
    /// 심볼 변환 서비스
    symbol_resolver: SymbolResolver,
    /// 캐시 유효 기간 (이 시간 이내면 신선하다고 간주)
    cache_freshness: Duration,
    /// Redis 캐시 (선택적, 성능 최적화)
    redis_cache: Option<Arc<crate::storage::redis::RedisCache>>,
}

impl CachedHistoricalDataProvider {
    /// 새로운 캐시 기반 제공자 생성.
    pub fn new(pool: PgPool) -> Self {
        Self {
            cache: OhlcvCache::new(pool.clone()),
            symbol_resolver: SymbolResolver::new(pool),
            cache_freshness: Duration::minutes(5),
            redis_cache: None,
        }
    }

    /// 캐시 유효 기간 설정.
    pub fn with_freshness(mut self, duration: Duration) -> Self {
        self.cache_freshness = duration;
        self
    }


    /// Redis 캐시 설정 (3계층 캐시 활성화).
    ///
    /// Redis가 설정되면 OHLCV 조회 시 다음 순서로 확인:
    /// 1. Redis 캐시 (가장 빠름, ms 단위)
    /// 2. PostgreSQL ohlcv 테이블
    /// 3. 외부 API (Yahoo Finance, KRX)
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// let redis = RedisCache::connect(&redis_config).await?;
    /// let provider = CachedHistoricalDataProvider::new(pool)
    ///     .with_redis(Arc::new(redis));
    /// ```
    pub fn with_redis(mut self, redis: Arc<crate::storage::redis::RedisCache>) -> Self {
        info!("Redis 캐시 활성화 - 3계층 캐시 구조 사용");
        self.redis_cache = Some(redis);
        self
    }

    /// 캔들 데이터 조회 (읽기 전용).
    ///
    /// 3계층 캐시 구조로 데이터를 조회합니다:
    /// 1. Redis 캐시 (ms 단위 응답)
    /// 2. PostgreSQL ohlcv 테이블 (10ms 단위)
    ///
    /// **중요**: 데이터가 없거나 부족해도 외부 API를 호출하지 않습니다.
    /// 데이터 수집/갱신은 Collector에서만 수행합니다.
    ///
    /// # 인자
    /// - `symbol`: canonical 심볼 (예: "005930", "AAPL", "BTC/USDT")
    /// - `timeframe`: 타임프레임
    /// - `limit`: 최대 캔들 수
    ///
    /// # 반환
    /// 캐시에 있는 캔들 데이터 (없으면 빈 Vec)
    #[instrument(skip(self))]
    pub async fn get_klines(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>> {
        // 읽기 전용 메서드로 위임 (외부 API 호출 없음)
        self.get_klines_readonly(symbol, timeframe, limit).await
    }

    /// 캔들 데이터 조회 (읽기 전용 - 외부 API 호출 없음).
    ///
    /// 3계층 캐시 구조로 데이터를 조회합니다:
    /// 1. Redis 캐시 (ms 단위 응답, 가장 빠름)
    /// 2. PostgreSQL ohlcv 테이블 (10ms 단위)
    ///
    /// Redis 캐시 미스 시 PostgreSQL에서 조회 후 Redis에 캐싱합니다.
    /// 데이터가 없거나 부족해도 외부 API를 호출하지 않습니다.
    /// API 서버에서 사용하며, 데이터 수집은 Collector에서만 수행합니다.
    ///
    /// # 인자
    /// - `symbol`: canonical 심볼 (예: "005930", "AAPL")
    /// - `timeframe`: 타임프레임
    /// - `limit`: 최대 캔들 수
    ///
    /// # 반환
    /// 캐시에 있는 캔들 데이터 (없으면 빈 Vec)
    #[instrument(skip(self))]
    pub async fn get_klines_readonly(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>> {
        // SymbolResolver를 통해 ticker 조회
        let (ticker, _yahoo_symbol, _market) = self.resolve_symbol(symbol).await?;

        // 1단계: Redis 캐시 확인 (가장 빠름)
        if let Some(redis) = &self.redis_cache {
            // "local"을 exchange로 사용 (단일 시장 환경)
            match redis.get_klines("local", &ticker, &timeframe).await {
                // Redis 캐시에 충분한 데이터가 있을 때만 히트로 처리
                // (캐시된 캔들 수 < limit이면 PostgreSQL로 fallback)
                Ok(Some(cached_klines)) if cached_klines.len() >= limit => {
                    // Redis 히트 - 가장 최근 limit개 반환
                    // 캐시는 시간순(oldest→newest) 정렬이므로 뒤에서 limit개를 가져와야 함
                    let skip_count = cached_klines.len().saturating_sub(limit);
                    let result: Vec<Kline> = cached_klines
                        .into_iter()
                        .skip(skip_count)
                        .map(|kline| Kline {
                            ticker: symbol.to_string(),
                            ..kline
                        })
                        .collect();

                    debug!(
                        canonical = %symbol,
                        ticker = %ticker,
                        returned = result.len(),
                        source = "redis",
                        "캔들 데이터 반환 (Redis 캐시 히트)"
                    );
                    return Ok(result);
                }
                Ok(Some(cached)) => {
                    debug!(
                        ticker = %ticker,
                        cached = cached.len(),
                        requested = limit,
                        "Redis 캐시 부족, PostgreSQL fallback"
                    );
                }
                Ok(None) => {
                    debug!(ticker = %ticker, "Redis 캐시 미스, PostgreSQL fallback");
                }
                Err(e) => {
                    warn!(ticker = %ticker, error = %e, "Redis 조회 실패, PostgreSQL fallback");
                }
            }
        }

        // 2단계: PostgreSQL ohlcv 테이블에서 조회
        let records = self
            .cache
            .get_cached_klines(&ticker, timeframe, limit)
            .await?;

        // canonical 심볼로 Kline 변환
        let klines: Vec<Kline> = records
            .into_iter()
            .map(|kline| Kline {
                ticker: symbol.to_string(),
                ..kline
            })
            .collect();

        // 3단계: PostgreSQL 결과를 Redis에 캐싱 (백그라운드)
        if let Some(redis) = &self.redis_cache {
            if !klines.is_empty() {
                // ticker 형식으로 Redis에 저장 (API 조회 시 ticker 기준)
                let redis_klines: Vec<Kline> = klines
                    .iter()
                    .map(|k| Kline {
                        ticker: ticker.clone(),
                        ..k.clone()
                    })
                    .collect();

                let redis = redis.clone();
                let ticker_clone = ticker.clone();
                tokio::spawn(async move {
                    if let Err(e) = redis
                        .set_klines("local", &ticker_clone, &timeframe, &redis_klines)
                        .await
                    {
                        warn!(ticker = %ticker_clone, error = %e, "Redis 캐시 저장 실패");
                    } else {
                        debug!(ticker = %ticker_clone, count = redis_klines.len(), "Redis 캐시 저장 완료");
                    }
                });
            }
        }

        debug!(
            canonical = %symbol,
            ticker = %ticker,
            returned = klines.len(),
            source = "postgresql",
            "캔들 데이터 반환 (PostgreSQL)"
        );

        Ok(klines)
    }

    /// 심볼 정보 조회.
    ///
    /// DB의 symbol_info 테이블에서 조회:
    /// - ticker: 저장/조회 키 (모든 곳에서 사용)
    /// - yahoo_symbol: Yahoo Finance API 호출 시에만 사용
    ///
    /// 반환: (ticker, yahoo_symbol, market)
    async fn resolve_symbol(&self, canonical: &str) -> Result<(String, Option<String>, String)> {
        // DB에서 심볼 정보 조회 (필수)
        let info = self
            .symbol_resolver
            .get_symbol_info(canonical)
            .await
            .map_err(|e| DataError::QueryError(format!("DB 조회 실패: {}", e)))?
            .ok_or_else(|| {
                DataError::NotFound(format!("심볼을 찾을 수 없습니다: {}", canonical))
            })?;

        Ok((
            info.ticker.clone(),
            info.yahoo_symbol.clone(),
            info.market.clone(),
        ))
    }

    /// 날짜 범위로 캔들 데이터 조회 (읽기 전용).
    ///
    /// PostgreSQL ohlcv 테이블에서만 데이터를 조회합니다.
    ///
    /// **중요**: 데이터가 없거나 부족해도 외부 API를 호출하지 않습니다.
    /// 데이터 수집/갱신은 Collector에서만 수행합니다.
    ///
    /// # 인자
    /// - `symbol`: canonical 심볼 (예: "005930", "AAPL", "BTC/USDT")
    /// - `timeframe`: 타임프레임
    /// - `start_date`: 시작 날짜
    /// - `end_date`: 종료 날짜
    ///
    /// # 반환
    /// 캐시에 있는 캔들 데이터 (없으면 빈 Vec)
    #[instrument(skip(self))]
    pub async fn get_klines_range(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<Kline>> {
        // SymbolResolver를 통해 ticker 조회
        let (ticker, _yahoo_symbol, _market) = self.resolve_symbol(symbol).await?;

        debug!(
            canonical = %symbol,
            ticker = %ticker,
            timeframe = %timeframe_to_string(timeframe),
            start = %start_date,
            end = %end_date,
            "날짜 범위 데이터 조회 요청 (읽기 전용)"
        );

        // PostgreSQL 캐시에서 조회 (외부 API 호출 없음)
        let start_dt = Utc.from_utc_datetime(&start_date.and_hms_opt(0, 0, 0).unwrap());
        let end_dt = Utc.from_utc_datetime(&end_date.and_hms_opt(23, 59, 59).unwrap());

        let cached_klines = self
            .cache
            .get_cached_klines_range(&ticker, timeframe, start_dt, end_dt)
            .await?;

        // canonical 심볼로 변환하여 반환
        let klines: Vec<Kline> = cached_klines
            .into_iter()
            .map(|k| Kline {
                ticker: symbol.to_string(),
                ..k
            })
            .collect();

        debug!(
            canonical = %symbol,
            ticker = %ticker,
            returned = klines.len(),
            source = "postgresql",
            "날짜 범위 데이터 반환 (읽기 전용)"
        );

        Ok(klines)
    }

    /// 캐시 통계 조회.
    pub async fn get_cache_stats(&self) -> Result<Vec<CacheStats>> {
        use crate::storage::ohlcv::OhlcvMetadataRecord;
        let records: Vec<OhlcvMetadataRecord> = self.cache.get_all_cache_stats().await?;
        Ok(records
            .into_iter()
            .map(|r| CacheStats {
                symbol: r.symbol,
                timeframe: r.timeframe,
                first_time: r.first_cached_time,
                last_time: r.last_cached_time,
                candle_count: r.total_candles.unwrap_or(0) as i64,
                last_updated: r.last_updated_at,
            })
            .collect())
    }

    /// 특정 심볼 캐시 삭제.
    ///
    /// # 인자
    /// - `symbol`: canonical 심볼 (예: "005930", "AAPL")
    pub async fn clear_cache(&self, symbol: &str) -> Result<u64> {
        let (ticker, _, _) = self.resolve_symbol(symbol).await?;
        self.cache.clear_symbol_cache(&ticker).await
    }

    /// 캐시 Warmup (주요 심볼 미리 캐시).
    pub async fn warmup(&self, symbols: &[(&str, Timeframe, usize)]) -> Result<usize> {
        let mut total = 0;
        for (symbol, timeframe, limit) in symbols {
            match self.get_klines(symbol, *timeframe, *limit).await {
                Ok(klines) => {
                    total += klines.len();
                    info!(symbol = symbol, count = klines.len(), "Warmup 완료");
                }
                Err(e) => {
                    warn!(symbol = symbol, error = %e, "Warmup 실패");
                }
            }
        }
        Ok(total)
    }

    /// 다중 타임프레임 캐시 Warmup (병렬 처리).
    ///
    /// 단일 심볼에 대해 여러 타임프레임의 데이터를 병렬로 미리 캐시합니다.
    ///
    /// # 인자
    ///
    /// * `symbol` - canonical 심볼 (예: "005930", "BTCUSDT")
    /// * `config` - 다중 타임프레임 설정
    ///
    /// # 반환
    ///
    /// 타임프레임별 로드된 캔들 수
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// use trader_core::{domain::MultiTimeframeConfig, Timeframe};
    ///
    /// let config = MultiTimeframeConfig::new()
    ///     .with_timeframe(Timeframe::M5, 60)
    ///     .with_timeframe(Timeframe::H1, 24)
    ///     .with_timeframe(Timeframe::D1, 14);
    ///
    /// let counts = provider.warmup_multi_timeframe("BTCUSDT", &config).await?;
    /// for (tf, count) in &counts {
    ///     println!("{:?}: {} candles", tf, count);
    /// }
    /// ```
    pub async fn warmup_multi_timeframe(
        &self,
        symbol: &str,
        config: &trader_core::domain::MultiTimeframeConfig,
    ) -> Result<std::collections::HashMap<Timeframe, usize>> {
        use futures::future::join_all;

        let timeframes: Vec<_> = config.timeframes.iter().collect();

        // 각 타임프레임별 병렬 로드
        let futures: Vec<_> = timeframes
            .iter()
            .map(|(&tf, &limit)| {
                let symbol = symbol.to_string();
                async move {
                    let result = self.get_klines(&symbol, tf, limit).await;
                    (tf, result)
                }
            })
            .collect();

        let results = join_all(futures).await;

        let mut counts = std::collections::HashMap::new();
        for (tf, result) in results {
            match result {
                Ok(klines) => {
                    let count = klines.len();
                    counts.insert(tf, count);
                    info!(
                        symbol = symbol,
                        timeframe = ?tf,
                        count = count,
                        "다중 TF Warmup 완료"
                    );
                }
                Err(e) => {
                    counts.insert(tf, 0);
                    warn!(
                        symbol = symbol,
                        timeframe = ?tf,
                        error = %e,
                        "다중 TF Warmup 실패"
                    );
                }
            }
        }

        Ok(counts)
    }

    /// 여러 타임프레임의 캔들 데이터를 병렬로 조회.
    ///
    /// # 인자
    ///
    /// * `symbol` - canonical 심볼
    /// * `config` - 다중 타임프레임 설정
    ///
    /// # 반환
    ///
    /// 타임프레임별 캔들 데이터
    pub async fn get_multi_timeframe_klines(
        &self,
        symbol: &str,
        config: &trader_core::domain::MultiTimeframeConfig,
    ) -> Result<std::collections::HashMap<Timeframe, Vec<Kline>>> {
        use futures::future::join_all;

        let timeframes: Vec<_> = config.timeframes.iter().collect();

        let futures: Vec<_> = timeframes
            .iter()
            .map(|(&tf, &limit)| {
                let symbol = symbol.to_string();
                async move {
                    let result = self.get_klines(&symbol, tf, limit).await;
                    (tf, result)
                }
            })
            .collect();

        let results = join_all(futures).await;

        let mut map = std::collections::HashMap::new();
        for (tf, result) in results {
            match result {
                Ok(klines) => {
                    map.insert(tf, klines);
                }
                Err(e) => {
                    warn!(
                        symbol = symbol,
                        timeframe = ?tf,
                        error = %e,
                        "다중 TF 조회 실패, 빈 데이터 반환"
                    );
                    map.insert(tf, Vec::new());
                }
            }
        }

        Ok(map)
    }
}

/// 캐시 통계.
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub symbol: String,
    pub timeframe: String,
    pub first_time: Option<DateTime<Utc>>,
    pub last_time: Option<DateTime<Utc>>,
    pub candle_count: i64,
    pub last_updated: Option<DateTime<Utc>>,
}

// =============================================================================
// 헬퍼 함수
// =============================================================================

/// Timeframe의 Duration 계산.
fn timeframe_to_duration(timeframe: Timeframe) -> Duration {
    match timeframe {
        Timeframe::M1 => Duration::minutes(1),
        Timeframe::M3 => Duration::minutes(3),
        Timeframe::M5 => Duration::minutes(5),
        Timeframe::M15 => Duration::minutes(15),
        Timeframe::M30 => Duration::minutes(30),
        Timeframe::H1 => Duration::hours(1),
        Timeframe::H2 => Duration::hours(2),
        Timeframe::H4 => Duration::hours(4),
        Timeframe::H6 => Duration::hours(6),
        Timeframe::H8 => Duration::hours(8),
        Timeframe::H12 => Duration::hours(12),
        Timeframe::D1 => Duration::days(1),
        Timeframe::D3 => Duration::days(3),
        Timeframe::W1 => Duration::weeks(1),
        Timeframe::MN1 => Duration::days(30),
    }
}



/// 심볼에서 통화 코드 추정.
pub(crate) fn guess_currency(symbol: &str) -> &'static str {
    if symbol.ends_with(".KS") || symbol.ends_with(".KQ") {
        "KRW"
    } else if symbol.ends_with(".T") {
        "JPY"
    } else if symbol.ends_with(".L") {
        "GBP"
    } else {
        "USD"
    }
}

// =============================================================================
// Yahoo Finance Provider 래퍼
// =============================================================================

/// Yahoo Finance Provider 래퍼.
///
/// `SymbolResolver`를 통해 ticker에서 yahoo_symbol을 조회합니다.
pub struct YahooProviderWrapper {
    connector: yahoo_finance_api::YahooConnector,
    symbol_resolver: SymbolResolver,
}

impl YahooProviderWrapper {
    pub fn new(symbol_resolver: SymbolResolver) -> Result<Self> {
        let connector = yahoo_finance_api::YahooConnector::new()
            .map_err(|e| DataError::ConnectionError(format!("Yahoo Finance 연결 실패: {}", e)))?;
        Ok(Self {
            connector,
            symbol_resolver,
        })
    }

    /// ticker를 Yahoo Finance API 호출용 심볼로 변환.
    ///
    /// `SymbolResolver`를 통해 DB에서 정확한 yahoo_symbol을 조회합니다.
    /// DB에 없으면 fallback으로 6자리 숫자는 `.KS` 추가.
    async fn resolve_yahoo_symbol(&self, ticker: &str) -> String {
        // DB에서 yahoo_symbol 조회 시도
        if let Ok(Some(info)) = self.symbol_resolver.get_symbol_info(ticker).await {
            if let Some(yahoo_symbol) = info.yahoo_symbol {
                return yahoo_symbol;
            }
        }
        // Fallback: 6자리 숫자 한국 주식인 경우 .KS 추가
        if ticker.len() == 6 && ticker.chars().all(|c| c.is_ascii_digit()) {
            format!("{}.KS", ticker)
        } else {
            ticker.to_string()
        }
    }

    pub async fn get_klines_internal(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>> {
        let interval = match timeframe {
            Timeframe::M1 => "1m",
            Timeframe::M3 | Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1
            | Timeframe::H2
            | Timeframe::H4
            | Timeframe::H6
            | Timeframe::H8
            | Timeframe::H12 => "1h",
            Timeframe::D1 | Timeframe::D3 => "1d",
            Timeframe::W1 => "1wk",
            Timeframe::MN1 => "1mo",
        };

        let range = calculate_range_string(timeframe, limit);

        // SymbolResolver를 통해 yahoo_symbol 조회
        let yahoo_symbol = self.resolve_yahoo_symbol(symbol).await;

        debug!(
            ticker = symbol,
            yahoo_symbol = %yahoo_symbol,
            interval = interval,
            range = range,
            "Yahoo Finance API 호출"
        );

        let response = self
            .connector
            .get_quote_range(&yahoo_symbol, interval, range)
            .await
            .map_err(|e| {
                DataError::FetchError(format!("Yahoo Finance API 오류 ({}): {}", yahoo_symbol, e))
            })?;

        let quotes = response
            .quotes()
            .map_err(|e| DataError::ParseError(format!("Quote 파싱 오류: {}", e)))?;

        if quotes.is_empty() {
            return Ok(Vec::new());
        }

        let _currency = guess_currency(symbol);
        let symbol_obj = symbol.to_string();

        let klines: Vec<Kline> = quotes
            .iter()
            .map(|q| {
                let open_time = Utc
                    .timestamp_opt(q.timestamp, 0)
                    .single()
                    .unwrap_or_else(Utc::now);
                let close_time = open_time + timeframe_to_duration(timeframe);

                Kline {
                    ticker: symbol_obj.clone(),
                    timeframe,
                    open_time,
                    open: Decimal::from_f64_retain(q.open).unwrap_or_default(),
                    high: Decimal::from_f64_retain(q.high).unwrap_or_default(),
                    low: Decimal::from_f64_retain(q.low).unwrap_or_default(),
                    close: Decimal::from_f64_retain(q.close).unwrap_or_default(),
                    volume: Decimal::from(q.volume),
                    close_time,
                    quote_volume: None,
                    num_trades: None,
                }
            })
            .collect();

        let mut sorted = klines;
        sorted.sort_by_key(|k| k.open_time);

        if sorted.len() > limit {
            let skip = sorted.len() - limit;
            sorted = sorted.into_iter().skip(skip).collect();
        }

        Ok(sorted)
    }

    /// 날짜 범위로 캔들 데이터 조회.
    ///
    /// # Arguments
    /// * `ticker` - 순수 ticker (예: "005930", "AAPL")
    ///
    /// 내부에서 Yahoo Finance API 호출용 심볼로 변환 (한국 주식: .KS 추가)
    pub async fn get_klines_range(
        &self,
        ticker: &str,
        timeframe: Timeframe,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<Kline>> {
        let interval = match timeframe {
            Timeframe::M1 => "1m",
            Timeframe::M3 | Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1
            | Timeframe::H2
            | Timeframe::H4
            | Timeframe::H6
            | Timeframe::H8
            | Timeframe::H12 => "1h",
            Timeframe::D1 | Timeframe::D3 => "1d",
            Timeframe::W1 => "1wk",
            Timeframe::MN1 => "1mo",
        };

        // chrono::NaiveDate → time::OffsetDateTime 변환
        let start = naive_date_to_offset_datetime(start_date);
        let end = naive_date_to_offset_datetime(end_date);

        // SymbolResolver를 통해 yahoo_symbol 조회
        let yahoo_symbol = self.resolve_yahoo_symbol(ticker).await;

        debug!(
            ticker = ticker,
            yahoo_symbol = %yahoo_symbol,
            interval = interval,
            start = %start_date,
            end = %end_date,
            "Yahoo Finance API 날짜 범위 호출"
        );

        let response = self
            .connector
            .get_quote_history_interval(&yahoo_symbol, start, end, interval)
            .await
            .map_err(|e| {
                DataError::FetchError(format!("Yahoo Finance API 오류 ({}): {}", yahoo_symbol, e))
            })?;

        let quotes = response
            .quotes()
            .map_err(|e| DataError::ParseError(format!("Quote 파싱 오류: {}", e)))?;

        if quotes.is_empty() {
            return Ok(Vec::new());
        }

        // 저장용 ticker 사용
        let klines: Vec<Kline> = quotes
            .iter()
            .map(|q| {
                let open_time = Utc
                    .timestamp_opt(q.timestamp, 0)
                    .single()
                    .unwrap_or_else(Utc::now);
                let close_time = open_time + timeframe_to_duration(timeframe);

                Kline {
                    ticker: ticker.to_string(),
                    timeframe,
                    open_time,
                    open: Decimal::from_f64_retain(q.open).unwrap_or_default(),
                    high: Decimal::from_f64_retain(q.high).unwrap_or_default(),
                    low: Decimal::from_f64_retain(q.low).unwrap_or_default(),
                    close: Decimal::from_f64_retain(q.close).unwrap_or_default(),
                    volume: Decimal::from(q.volume),
                    close_time,
                    quote_volume: None,
                    num_trades: None,
                }
            })
            .collect();

        let mut sorted = klines;
        sorted.sort_by_key(|k| k.open_time);

        Ok(sorted)
    }
}

/// NaiveDate를 OffsetDateTime으로 변환.
fn naive_date_to_offset_datetime(date: NaiveDate) -> OffsetDateTime {
    let (year, month, day) = (date.year(), date.month() as u8, date.day() as u8);
    time::Date::from_calendar_date(year, time::Month::try_from(month).unwrap(), day)
        .unwrap()
        .midnight()
        .assume_utc()
}

fn calculate_range_string(timeframe: Timeframe, limit: usize) -> &'static str {
    match timeframe {
        Timeframe::M1 | Timeframe::M3 | Timeframe::M5 | Timeframe::M15 | Timeframe::M30 => {
            if limit <= 100 {
                "5d"
            } else if limit <= 500 {
                "1mo"
            } else {
                "3mo"
            }
        }
        Timeframe::H1
        | Timeframe::H2
        | Timeframe::H4
        | Timeframe::H6
        | Timeframe::H8
        | Timeframe::H12 => {
            if limit <= 50 {
                "5d"
            } else if limit <= 200 {
                "1mo"
            } else {
                "3mo"
            }
        }
        Timeframe::D1 => {
            if limit <= 5 {
                "5d"
            } else if limit <= 20 {
                "1mo"
            } else if limit <= 60 {
                "3mo"
            } else if limit <= 120 {
                "6mo"
            } else if limit <= 250 {
                "1y"
            } else if limit <= 500 {
                "2y"
            } else if limit <= 1250 {
                "5y"
            } else {
                "10y"
            }
        }
        Timeframe::D3 => {
            if limit <= 10 {
                "1mo"
            } else if limit <= 30 {
                "3mo"
            } else if limit <= 60 {
                "6mo"
            } else {
                "1y"
            }
        }
        Timeframe::W1 => {
            if limit <= 4 {
                "1mo"
            } else if limit <= 12 {
                "3mo"
            } else if limit <= 26 {
                "6mo"
            } else if limit <= 52 {
                "1y"
            } else if limit <= 104 {
                "2y"
            } else {
                "5y"
            }
        }
        Timeframe::MN1 => {
            if limit <= 3 {
                "3mo"
            } else if limit <= 6 {
                "6mo"
            } else if limit <= 12 {
                "1y"
            } else if limit <= 24 {
                "2y"
            } else if limit <= 60 {
                "5y"
            } else {
                "10y"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // 현재 테스트할 standalone 함수 없음
    // 리팩토링 이후 필요 시 추가
}
