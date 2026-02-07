//! KRX API 및 네이버 금융을 활용한 Fundamental 데이터 수집 모듈.
//!
//! ## 데이터 소스
//!
//! ### KRX OPEN API (인증 필요)
//! - 가치 지표: PER, PBR, 배당수익률, EPS, BPS
//! - 시가총액, 상장주식수
//! - 섹터 정보 업데이트
//!
//! ### 네이버 금융 크롤러 (인증 불필요)
//! - 가치 지표: PER, PBR, ROE, EPS, BPS, 배당수익률
//! - 시가총액, 52주 고저
//! - 섹터, 시장 구분 (KOSPI/KOSDAQ/ETF)
//! - 외국인 소진율

use chrono::Utc;
use sqlx::{PgPool, QueryBuilder, Postgres};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};
use uuid::Uuid;

use trader_core::CredentialEncryptor;
use trader_data::provider::krx_api::{KrxApiClient, KrxDailyTrade};
use trader_data::provider::naver::{NaverFinanceFetcher, NaverFundamentalData};

use super::checkpoint::{self, CheckpointStatus};
use crate::{config::FundamentalCollectConfig, error::CollectorError, Result};

/// Fundamental 동기화 통계.
#[derive(Debug, Default)]
pub struct FundamentalSyncStats {
    /// 처리된 종목 수
    pub processed: usize,
    /// PER/PBR 업데이트된 종목 수
    pub valuation_updated: usize,
    /// 시가총액 업데이트된 종목 수
    pub market_cap_updated: usize,
    /// 섹터 업데이트된 종목 수
    pub sector_updated: usize,
    /// 52주 고저 업데이트된 종목 수
    pub week_52_updated: usize,
    /// 시장 타입(KOSPI/KOSDAQ/ETF) 업데이트된 종목 수
    pub market_type_updated: usize,
    /// 실패 수
    pub failed: usize,
    /// 데이터 소스
    pub data_source: String,
}

/// KRX fundamental 데이터 동기화.
///
/// KOSPI/KOSDAQ 종목의 가치 지표, 시가총액, 섹터 정보를 KRX API에서 수집하여
/// symbol_fundamental 및 symbol_info 테이블에 저장합니다.
pub async fn sync_krx_fundamentals(
    pool: &PgPool,
    config: &FundamentalCollectConfig,
) -> Result<FundamentalSyncStats> {
    info!("KRX Fundamental 데이터 동기화 시작");

    // KRX API 클라이언트 생성
    let master_key = match std::env::var("ENCRYPTION_MASTER_KEY") {
        Ok(key) => key,
        Err(_) => {
            warn!("ENCRYPTION_MASTER_KEY 환경변수가 설정되지 않았습니다. 동기화를 건너뜁니다.");
            return Ok(FundamentalSyncStats::default());
        }
    };

    let encryptor = CredentialEncryptor::new(&master_key)
        .map_err(|e| CollectorError::DataSource(format!("암호화키 로드 실패: {}", e)))?;

    let client = match KrxApiClient::from_credential(pool, &encryptor).await {
        Ok(Some(client)) => client,
        Ok(None) => {
            warn!("KRX API credential이 등록되지 않았습니다. 동기화를 건너뜁니다.");
            return Ok(FundamentalSyncStats::default());
        }
        Err(e) => {
            return Err(CollectorError::DataSource(format!(
                "KRX API 클라이언트 생성 실패: {}",
                e
            )))
        }
    };

    // T-1 날짜 사용 (KRX API는 전일 데이터만 제공)
    let yesterday = (Utc::now() - chrono::Duration::days(1)).format("%Y%m%d").to_string();
    let mut stats = FundamentalSyncStats::default();

    // NOTE: 가치 지표 API (stk_isu_per_pbr, ksq_isu_per_pbr)는 KRX에서 제공하지 않음
    // PER/PBR 데이터는 Naver 크롤링으로 수집 (sync_naver_fundamentals 사용)
    // stats.valuation_updated는 0으로 유지

    // 일별 매매정보에서 시가총액, 섹터 정보 수집
    info!(base_date = %yesterday, "시가총액 및 섹터 정보 수집 중 (T-1 데이터)...");
    let (market_cap_stats, sector_stats) = sync_market_data(pool, &client, &yesterday, config).await?;
    stats.market_cap_updated = market_cap_stats;
    stats.sector_updated = sector_stats;

    stats.processed = stats.valuation_updated + stats.market_cap_updated;

    info!(
        processed = stats.processed,
        valuation = stats.valuation_updated,
        market_cap = stats.market_cap_updated,
        sector = stats.sector_updated,
        failed = stats.failed,
        "KRX Fundamental 데이터 동기화 완료"
    );

    Ok(stats)
}





/// 시가총액 및 섹터 정보 동기화.
async fn sync_market_data(
    pool: &PgPool,
    client: &KrxApiClient,
    base_date: &str,
    _config: &FundamentalCollectConfig,
) -> Result<(usize, usize)> {
    // KOSPI 일별 매매정보 조회
    let kospi_trades = match client.fetch_kospi_daily_trades(base_date).await {
        Ok(t) => t,
        Err(e) => {
            warn!(error = %e, "KOSPI 일별 매매정보 조회 실패");
            Vec::new()
        }
    };

    // API 호출 간 딜레이
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // KOSDAQ 일별 매매정보 조회
    let kosdaq_trades = match client.fetch_kosdaq_daily_trades(base_date).await {
        Ok(t) => t,
        Err(e) => {
            warn!(error = %e, "KOSDAQ 일별 매매정보 조회 실패");
            Vec::new()
        }
    };

    let all_trades: Vec<KrxDailyTrade> = kospi_trades
        .into_iter()
        .chain(kosdaq_trades.into_iter())
        .collect();

    info!(count = all_trades.len(), "일별 매매정보 조회 완료");

    // DB에 저장
    let mut market_cap_updated = 0;
    // sector_updated: KRX 소속부 정보는 사용하지 않음 (네이버 금융에서 업종 정보 수집)
    let sector_updated = 0;

    for trade in &all_trades {
        // 시가총액 업데이트
        if let Err(e) = upsert_market_cap(pool, trade).await {
            debug!(ticker = %trade.code, error = %e, "시가총액 저장 실패");
        } else {
            market_cap_updated += 1;
        }

        // NOTE: KRX 일별 매매정보의 sector는 "소속부"(중견기업부, 우량기업부 등)로
        // 실제 산업 업종(반도체, 자동차 등)이 아님.
        // 네이버 금융 크롤러에서 올바른 산업 업종을 가져오므로 KRX 소속부 정보는 저장하지 않음.
        // 기존 코드:
        // if let Some(sector) = &trade.sector {
        //     if !sector.is_empty() {
        //         if let Err(e) = update_sector(pool, &trade.code, sector).await {
        //             debug!(ticker = %trade.code, error = %e, "섹터 업데이트 실패");
        //         } else {
        //             sector_updated += 1;
        //         }
        //     }
        // }
        let _ = &trade.sector; // 컴파일 경고 방지
    }

    Ok((market_cap_updated, sector_updated))
}

/// 시가총액 및 상장주식수를 symbol_fundamental 테이블에 저장.
async fn upsert_market_cap(pool: &PgPool, trade: &KrxDailyTrade) -> Result<()> {
    // 종목코드에서 티커 추출 (KR7005930003 → 005930)
    let ticker = extract_ticker(&trade.code);

    // symbol_info에서 ID 조회
    let symbol_info: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id
        FROM symbol_info
        WHERE ticker = $1 AND market = 'KR' AND is_active = true
        LIMIT 1
        "#,
    )
    .bind(&ticker)
    .fetch_optional(pool)
    .await?;

    let symbol_info_id = match symbol_info {
        Some((id,)) => id,
        None => return Ok(()), // 심볼이 없으면 건너뜀
    };

    // symbol_fundamental에 시가총액 Upsert
    sqlx::query(
        r#"
        INSERT INTO symbol_fundamental (
            symbol_info_id, market_cap, shares_outstanding,
            data_source, currency, fetched_at, updated_at
        )
        VALUES ($1, $2, $3, 'KRX', 'KRW', NOW(), NOW())
        ON CONFLICT (symbol_info_id)
        DO UPDATE SET
            market_cap = COALESCE(EXCLUDED.market_cap, symbol_fundamental.market_cap),
            shares_outstanding = COALESCE(EXCLUDED.shares_outstanding, symbol_fundamental.shares_outstanding),
            fetched_at = NOW(),
            updated_at = NOW()
        "#,
    )
    .bind(symbol_info_id)
    .bind(trade.market_cap)
    .bind(trade.shares_outstanding)
    .execute(pool)
    .await
    ?;

    Ok(())
}

/// KRX 종목코드에서 티커 추출.
///
/// KR7005930003 → 005930
/// 005930 → 005930 (이미 티커인 경우 그대로 반환)
fn extract_ticker(code: &str) -> String {
    // KR7XXXXXX003 형식에서 6자리 티커 추출
    if code.len() == 12 && code.starts_with("KR") {
        code[3..9].to_string()
    } else if code.len() == 6 {
        code.to_string()
    } else {
        // 기타 형식은 그대로 반환
        code.to_string()
    }
}

/// 섹터별 통계 조회.
pub async fn get_sector_statistics(pool: &PgPool) -> Result<HashMap<String, usize>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT
            COALESCE(sector, '미분류') as sector,
            COUNT(*) as count
        FROM symbol_info
        WHERE market = 'KR' AND is_active = true
        GROUP BY sector
        ORDER BY count DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let stats: HashMap<String, usize> = rows
        .into_iter()
        .map(|(sector, count)| (sector, count as usize))
        .collect();

    Ok(stats)
}

// ==================== 네이버 금융 크롤러 ====================

/// 네이버 금융을 통한 KR 시장 fundamental 데이터 동기화.
///
/// KRX API 인증 없이 네이버 금융 크롤링을 통해 데이터를 수집합니다.
/// 수집 항목:
/// - PER, PBR, ROE, EPS, BPS, 배당수익률
/// - 시가총액, 52주 고저
/// - 섹터, 시장 타입 (KOSPI/KOSDAQ/ETF)
/// - 외국인 소진율
///
/// # Arguments
/// * `pool` - DB 연결 풀
/// * `request_delay_ms` - 요청 간 딜레이 (밀리초)
/// * `batch_size` - 배치당 처리할 심볼 수 (None이면 전체)
///
/// 네이버 Fundamental 동기화 옵션
#[derive(Debug, Default)]
pub struct NaverSyncOptions {
    /// 요청 간 딜레이 (ms)
    pub request_delay_ms: u64,
    /// 배치 크기 (None이면 전체)
    pub batch_size: Option<i64>,
    /// 중단점부터 재개
    pub resume: bool,
    /// N시간 이내 업데이트된 심볼 스킵
    pub stale_hours: Option<u32>,
    /// 기존 값 강제 덮어쓰기 (기본: false - 기존 값 보존)
    pub force: bool,
    /// 동시 크롤링 수 (기본 3)
    pub concurrent_limit: Option<usize>,
}

pub async fn sync_naver_fundamentals(
    pool: &PgPool,
    request_delay_ms: u64,
    batch_size: Option<i64>,
) -> Result<FundamentalSyncStats> {
    // 기본 옵션으로 호출
    let options = NaverSyncOptions {
        request_delay_ms,
        batch_size,
        resume: false,
        stale_hours: None,
        force: false,  // 기본: 기존 값 보존
        concurrent_limit: None,
    };
    sync_naver_fundamentals_with_options(pool, options).await
}

/// 네이버 금융을 통한 KR 시장 fundamental 데이터 동기화 (옵션 포함).
///
/// - `resume`: true면 이전 중단점부터 재개
/// - `stale_hours`: 지정 시 해당 시간 이내 업데이트된 심볼 스킵
pub async fn sync_naver_fundamentals_with_options(
    pool: &PgPool,
    options: NaverSyncOptions,
) -> Result<FundamentalSyncStats> {
    info!("네이버 금융 Fundamental 데이터 동기화 시작");

    let mut stats = FundamentalSyncStats {
        data_source: "NAVER".to_string(),
        ..Default::default()
    };

    // 체크포인트 로드 (resume 모드)
    let resume_ticker = if options.resume {
        match checkpoint::load_checkpoint(pool, "naver_fundamental").await? {
            Some(t) => {
                info!(last_ticker = %t, "중단점부터 재개");
                Some(t)
            }
            None => {
                info!("이전 중단점 없음, 처음부터 시작");
                None
            }
        }
    } else {
        None
    };

    // KR 시장 활성 심볼 조회 (stale_hours 조건 포함)
    // QueryBuilder 사용으로 SQL 주입 방지
    let limit = options.batch_size.unwrap_or(i64::MAX);

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
        SELECT si.id, si.ticker
        FROM symbol_info si
        LEFT JOIN symbol_fundamental sf ON si.id = sf.symbol_info_id
        WHERE si.market = 'KR' AND si.is_active = true
          AND si.symbol_type IN ('STOCK', 'ETF')
        "#,
    );

    // stale_hours 조건 (파라미터 바인딩)
    if let Some(hours) = options.stale_hours {
        qb.push(" AND (sf.updated_at IS NULL OR sf.updated_at < NOW() - INTERVAL '");
        qb.push(hours.to_string());
        qb.push(" hours')");
    }

    // resume_ticker 조건 (파라미터 바인딩으로 SQL 주입 방지)
    if let Some(ref t) = resume_ticker {
        qb.push(" AND si.ticker > ");
        qb.push_bind(t.clone());
    }

    qb.push(" ORDER BY si.ticker LIMIT ");
    qb.push_bind(limit);

    let symbols: Vec<(Uuid, String)> = qb.build_query_as().fetch_all(pool).await?;

    let total = symbols.len();

    if options.stale_hours.is_some() {
        info!(count = total, stale_hours = ?options.stale_hours, "업데이트 필요한 심볼 조회 완료");
    } else {
        info!(count = total, "KR 심볼 조회 완료");
    }

    if symbols.is_empty() {
        // 완료 상태로 저장
        checkpoint::save_checkpoint(
            pool,
            "naver_fundamental",
            "",
            0,
            CheckpointStatus::Completed,
        )
        .await?;
        return Ok(stats);
    }

    // 시작 상태 저장
    checkpoint::save_checkpoint(pool, "naver_fundamental", "", 0, CheckpointStatus::Running)
        .await?;

    // 네이버 금융 크롤러 초기화
    let fetcher = NaverFinanceFetcher::with_delay(Duration::from_millis(options.request_delay_ms));
    let concurrent_limit = options.concurrent_limit.unwrap_or(3);
    let force = options.force;
    let request_delay_ms = options.request_delay_ms;

    // Semaphore 기반 동시성 제한
    let semaphore = Arc::new(Semaphore::new(concurrent_limit));

    info!(concurrent_limit = concurrent_limit, total = total, "네이버 동시 크롤링 시작");

    for (idx, (symbol_info_id, ticker)) in symbols.iter().enumerate() {
        stats.processed += 1;

        // Semaphore로 동시 실행 수 제한
        let _permit = semaphore.clone().acquire_owned().await.expect("세마포어 획득 실패");

        if (idx + 1) % 100 == 0 || idx + 1 == total {
            info!(
                progress = format!("{}/{}", idx + 1, total),
                "네이버 Fundamental 수집 진행 중"
            );
            // 체크포인트 저장 (100개마다)
            checkpoint::save_checkpoint(
                pool,
                "naver_fundamental",
                ticker,
                stats.processed as i32,
                CheckpointStatus::Running,
            )
            .await?;
        }

        // 네이버 금융에서 데이터 수집
        match fetcher.fetch_fundamental(ticker).await {
            Ok(data) => {
                // DB에 저장 (force 옵션에 따라 기존 값 보존 또는 덮어쓰기)
                if let Err(e) = upsert_naver_fundamental(pool, *symbol_info_id, &data, force).await {
                    debug!(ticker = ticker, error = %e, "네이버 데이터 저장 실패");
                    stats.failed += 1;
                } else {
                    // 업데이트된 항목 카운트
                    if data.per.is_some() || data.pbr.is_some() {
                        stats.valuation_updated += 1;
                    }
                    if data.market_cap.is_some() {
                        stats.market_cap_updated += 1;
                    }
                    if data.sector.is_some() {
                        stats.sector_updated += 1;
                    }
                    if data.week_52_high.is_some() || data.week_52_low.is_some() {
                        stats.week_52_updated += 1;
                    }
                }

                // 시장 타입 업데이트 (KOSPI/KOSDAQ/ETF)
                if let Err(e) =
                    update_market_type(pool, *symbol_info_id, &data.market_type.to_string()).await
                {
                    debug!(ticker = ticker, error = %e, "시장 타입 업데이트 실패");
                } else {
                    stats.market_type_updated += 1;
                }
            }
            Err(e) => {
                // Rate limit 에러는 경고, 나머지는 debug
                if matches!(e, trader_data::provider::naver::NaverError::RateLimited) {
                    warn!(ticker = ticker, "네이버 Rate limit 초과 - 잠시 대기");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                } else {
                    debug!(ticker = ticker, error = %e, "네이버 데이터 수집 실패");
                }
                stats.failed += 1;
            }
        }

        // 요청 간 딜레이 (마지막 항목이 아닐 때만)
        if idx + 1 < total {
            tokio::time::sleep(Duration::from_millis(request_delay_ms)).await;
        }
    }

    // 완료 상태 저장
    checkpoint::save_checkpoint(
        pool,
        "naver_fundamental",
        "",
        stats.processed as i32,
        CheckpointStatus::Completed,
    )
    .await?;

    info!(
        processed = stats.processed,
        valuation = stats.valuation_updated,
        market_cap = stats.market_cap_updated,
        sector = stats.sector_updated,
        week_52 = stats.week_52_updated,
        market_type = stats.market_type_updated,
        failed = stats.failed,
        "네이버 금융 Fundamental 데이터 동기화 완료"
    );

    Ok(stats)
}

/// 네이버 금융 데이터를 symbol_fundamental 테이블에 저장 (Upsert).
///
/// # Arguments
/// * `pool` - DB 연결 풀
/// * `symbol_info_id` - 심볼 ID
/// * `data` - 네이버 금융 데이터
/// * `force` - true: 기존 값 덮어쓰기, false: 기존 값 보존 (NULL만 채움)
async fn upsert_naver_fundamental(
    pool: &PgPool,
    symbol_info_id: Uuid,
    data: &NaverFundamentalData,
    force: bool,
) -> Result<()> {
    // OHLCV 테이블에서 10일 평균 거래량 계산
    let avg_volume_10d: Option<i64> = if data.avg_volume_10d.is_some() {
        data.avg_volume_10d
    } else {
        // 티커로 OHLCV에서 계산
        calculate_avg_volume(pool, symbol_info_id, 10).await.ok()
    };

    // force=true: 새 값 우선 (기존 동작)
    // force=false: 기존 값 우선 (기존 값이 있으면 보존)
    let query = if force {
        // 새 값 우선: COALESCE(EXCLUDED.xxx, symbol_fundamental.xxx)
        r#"
        INSERT INTO symbol_fundamental (
            symbol_info_id, market_cap, shares_outstanding, float_shares,
            week_52_high, week_52_low, avg_volume_10d, avg_volume_3m,
            per, forward_per, pbr, psr, eps, bps,
            dividend_yield, roe, roa, gross_margin, operating_margin, net_profit_margin,
            debt_ratio, current_ratio, quick_ratio,
            revenue, net_income, revenue_growth_yoy, earnings_growth_yoy,
            foreign_ratio, data_source, currency, fetched_at, updated_at
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, 'NAVER', 'KRW', NOW(), NOW()
        )
        ON CONFLICT (symbol_info_id)
        DO UPDATE SET
            market_cap = COALESCE(EXCLUDED.market_cap, symbol_fundamental.market_cap),
            shares_outstanding = COALESCE(EXCLUDED.shares_outstanding, symbol_fundamental.shares_outstanding),
            float_shares = COALESCE(EXCLUDED.float_shares, symbol_fundamental.float_shares),
            week_52_high = COALESCE(EXCLUDED.week_52_high, symbol_fundamental.week_52_high),
            week_52_low = COALESCE(EXCLUDED.week_52_low, symbol_fundamental.week_52_low),
            avg_volume_10d = COALESCE(EXCLUDED.avg_volume_10d, symbol_fundamental.avg_volume_10d),
            avg_volume_3m = COALESCE(EXCLUDED.avg_volume_3m, symbol_fundamental.avg_volume_3m),
            per = COALESCE(EXCLUDED.per, symbol_fundamental.per),
            forward_per = COALESCE(EXCLUDED.forward_per, symbol_fundamental.forward_per),
            pbr = COALESCE(EXCLUDED.pbr, symbol_fundamental.pbr),
            psr = COALESCE(EXCLUDED.psr, symbol_fundamental.psr),
            eps = COALESCE(EXCLUDED.eps, symbol_fundamental.eps),
            bps = COALESCE(EXCLUDED.bps, symbol_fundamental.bps),
            dividend_yield = COALESCE(EXCLUDED.dividend_yield, symbol_fundamental.dividend_yield),
            roe = COALESCE(EXCLUDED.roe, symbol_fundamental.roe),
            roa = COALESCE(EXCLUDED.roa, symbol_fundamental.roa),
            gross_margin = COALESCE(EXCLUDED.gross_margin, symbol_fundamental.gross_margin),
            operating_margin = COALESCE(EXCLUDED.operating_margin, symbol_fundamental.operating_margin),
            net_profit_margin = COALESCE(EXCLUDED.net_profit_margin, symbol_fundamental.net_profit_margin),
            debt_ratio = COALESCE(EXCLUDED.debt_ratio, symbol_fundamental.debt_ratio),
            current_ratio = COALESCE(EXCLUDED.current_ratio, symbol_fundamental.current_ratio),
            quick_ratio = COALESCE(EXCLUDED.quick_ratio, symbol_fundamental.quick_ratio),
            revenue = COALESCE(EXCLUDED.revenue, symbol_fundamental.revenue),
            net_income = COALESCE(EXCLUDED.net_income, symbol_fundamental.net_income),
            revenue_growth_yoy = COALESCE(EXCLUDED.revenue_growth_yoy, symbol_fundamental.revenue_growth_yoy),
            earnings_growth_yoy = COALESCE(EXCLUDED.earnings_growth_yoy, symbol_fundamental.earnings_growth_yoy),
            foreign_ratio = COALESCE(EXCLUDED.foreign_ratio, symbol_fundamental.foreign_ratio),
            data_source = 'NAVER',
            fetched_at = NOW(),
            updated_at = NOW()
        "#
    } else {
        // 기존 값 우선: COALESCE(symbol_fundamental.xxx, EXCLUDED.xxx)
        // 기존 값이 있으면 보존, NULL인 경우에만 새 값으로 채움
        r#"
        INSERT INTO symbol_fundamental (
            symbol_info_id, market_cap, shares_outstanding, float_shares,
            week_52_high, week_52_low, avg_volume_10d, avg_volume_3m,
            per, forward_per, pbr, psr, eps, bps,
            dividend_yield, roe, roa, gross_margin, operating_margin, net_profit_margin,
            debt_ratio, current_ratio, quick_ratio,
            revenue, net_income, revenue_growth_yoy, earnings_growth_yoy,
            foreign_ratio, data_source, currency, fetched_at, updated_at
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, 'NAVER', 'KRW', NOW(), NOW()
        )
        ON CONFLICT (symbol_info_id)
        DO UPDATE SET
            market_cap = COALESCE(symbol_fundamental.market_cap, EXCLUDED.market_cap),
            shares_outstanding = COALESCE(symbol_fundamental.shares_outstanding, EXCLUDED.shares_outstanding),
            float_shares = COALESCE(symbol_fundamental.float_shares, EXCLUDED.float_shares),
            week_52_high = COALESCE(symbol_fundamental.week_52_high, EXCLUDED.week_52_high),
            week_52_low = COALESCE(symbol_fundamental.week_52_low, EXCLUDED.week_52_low),
            avg_volume_10d = COALESCE(symbol_fundamental.avg_volume_10d, EXCLUDED.avg_volume_10d),
            avg_volume_3m = COALESCE(symbol_fundamental.avg_volume_3m, EXCLUDED.avg_volume_3m),
            per = COALESCE(symbol_fundamental.per, EXCLUDED.per),
            forward_per = COALESCE(symbol_fundamental.forward_per, EXCLUDED.forward_per),
            pbr = COALESCE(symbol_fundamental.pbr, EXCLUDED.pbr),
            psr = COALESCE(symbol_fundamental.psr, EXCLUDED.psr),
            eps = COALESCE(symbol_fundamental.eps, EXCLUDED.eps),
            bps = COALESCE(symbol_fundamental.bps, EXCLUDED.bps),
            dividend_yield = COALESCE(symbol_fundamental.dividend_yield, EXCLUDED.dividend_yield),
            roe = COALESCE(symbol_fundamental.roe, EXCLUDED.roe),
            roa = COALESCE(symbol_fundamental.roa, EXCLUDED.roa),
            gross_margin = COALESCE(symbol_fundamental.gross_margin, EXCLUDED.gross_margin),
            operating_margin = COALESCE(symbol_fundamental.operating_margin, EXCLUDED.operating_margin),
            net_profit_margin = COALESCE(symbol_fundamental.net_profit_margin, EXCLUDED.net_profit_margin),
            debt_ratio = COALESCE(symbol_fundamental.debt_ratio, EXCLUDED.debt_ratio),
            current_ratio = COALESCE(symbol_fundamental.current_ratio, EXCLUDED.current_ratio),
            quick_ratio = COALESCE(symbol_fundamental.quick_ratio, EXCLUDED.quick_ratio),
            revenue = COALESCE(symbol_fundamental.revenue, EXCLUDED.revenue),
            net_income = COALESCE(symbol_fundamental.net_income, EXCLUDED.net_income),
            revenue_growth_yoy = COALESCE(symbol_fundamental.revenue_growth_yoy, EXCLUDED.revenue_growth_yoy),
            earnings_growth_yoy = COALESCE(symbol_fundamental.earnings_growth_yoy, EXCLUDED.earnings_growth_yoy),
            foreign_ratio = COALESCE(symbol_fundamental.foreign_ratio, EXCLUDED.foreign_ratio),
            data_source = 'NAVER',
            fetched_at = NOW(),
            updated_at = NOW()
        "#
    };

    sqlx::query(query)
    .bind(symbol_info_id)
    .bind(data.market_cap)
    .bind(data.shares_outstanding)
    .bind(data.float_shares)          // 네이버 미제공, None
    .bind(data.week_52_high)
    .bind(data.week_52_low)
    .bind(avg_volume_10d)
    .bind(data.avg_volume_3m)         // 네이버 미제공, None (추후 OHLCV에서 계산)
    .bind(data.per)
    .bind(data.forward_per)           // 네이버 미제공, None
    .bind(data.pbr)
    .bind(data.psr)
    .bind(data.eps)
    .bind(data.bps)
    .bind(data.dividend_yield)
    .bind(data.roe)
    .bind(data.roa)
    .bind(data.gross_margin)          // 네이버 미제공, None
    .bind(data.operating_margin)
    .bind(data.net_profit_margin)     // 계산됨: 순이익/매출액
    .bind(data.debt_ratio)
    .bind(data.current_ratio)
    .bind(data.quick_ratio)
    .bind(data.revenue)
    .bind(data.net_income)
    .bind(data.revenue_growth_yoy)
    .bind(data.net_income_growth_yoy) // earnings_growth_yoy로 매핑
    .bind(data.foreign_ratio)
    .execute(pool)
    .await?;

    // 섹터 정보도 symbol_info에 업데이트 (기존 섹터가 없거나 force일 때만)
    if let Some(sector) = &data.sector {
        if !sector.is_empty() {
            let sector_query = if force {
                // force=true: 무조건 덮어쓰기
                r#"
                UPDATE symbol_info
                SET sector = $2, updated_at = NOW()
                WHERE id = $1
                "#
            } else {
                // force=false: 기존 섹터가 NULL인 경우에만
                r#"
                UPDATE symbol_info
                SET sector = $2, updated_at = NOW()
                WHERE id = $1 AND (sector IS NULL OR sector = '')
                "#
            };
            sqlx::query(sector_query)
                .bind(symbol_info_id)
                .bind(sector)
                .execute(pool)
                .await?;
        }
    }

    Ok(())
}

/// OHLCV 테이블에서 평균 거래량 계산
async fn calculate_avg_volume(pool: &PgPool, symbol_info_id: Uuid, days: i32) -> Result<i64> {
    let result: Option<(Option<rust_decimal::Decimal>,)> = sqlx::query_as(
        r#"
        SELECT AVG(volume)::DECIMAL as avg_vol
        FROM ohlcv o
        WHERE o.symbol_info_id = $1
          AND o.timeframe = '1d'
          AND o.open_time >= NOW() - INTERVAL '1 day' * $2
        "#,
    )
    .bind(symbol_info_id)
    .bind(days)
    .fetch_optional(pool)
    .await?;

    match result {
        Some((Some(avg),)) => Ok(avg.to_string().parse::<i64>().unwrap_or(0)),
        _ => Err(CollectorError::DataSource("OHLCV 데이터 없음".to_string())),
    }
}





/// 시장 타입(KOSPI/KOSDAQ/ETF)을 symbol_info의 exchange 필드에 업데이트.
async fn update_market_type(pool: &PgPool, symbol_info_id: Uuid, market_type: &str) -> Result<()> {
    // UNKNOWN이 아닌 경우에만 업데이트
    if market_type != "UNKNOWN" {
        sqlx::query(
            r#"
            UPDATE symbol_info
            SET exchange = $2, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(symbol_info_id)
        .bind(market_type)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// 특정 심볼의 네이버 fundamental 데이터 조회 및 저장 (단건).
///
/// 테스트나 개별 심볼 업데이트에 유용합니다.
pub async fn fetch_and_save_naver_fundamental(
    pool: &PgPool,
    ticker: &str,
) -> Result<NaverFundamentalData> {
    // symbol_info에서 ID 조회
    let symbol_info: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id
        FROM symbol_info
        WHERE ticker = $1 AND market = 'KR' AND is_active = true
        LIMIT 1
        "#,
    )
    .bind(ticker)
    .fetch_optional(pool)
    .await?;

    let symbol_info_id = match symbol_info {
        Some((id,)) => id,
        None => {
            return Err(CollectorError::DataSource(format!(
                "심볼을 찾을 수 없습니다: {}",
                ticker
            )))
        }
    };

    // 네이버 금융에서 데이터 수집
    let fetcher = NaverFinanceFetcher::new();
    let data = fetcher
        .fetch_fundamental(ticker)
        .await
        .map_err(|e| CollectorError::DataSource(format!("네이버 데이터 수집 실패: {}", e)))?;

    // DB에 저장 (단건 조회는 강제 덮어쓰기)
    upsert_naver_fundamental(pool, symbol_info_id, &data, true).await?;

    // 시장 타입 업데이트
    update_market_type(pool, symbol_info_id, &data.market_type.to_string()).await?;

    info!(
        ticker = ticker,
        name = ?data.name,
        market_type = %data.market_type,
        sector = ?data.sector,
        per = ?data.per,
        "네이버 Fundamental 데이터 저장 완료"
    );

    Ok(data)
}

// ==================== Yahoo Finance 펀더멘털 크롤러 ====================

use trader_data::provider::yahoo_fundamental::{YahooFundamentalData, YahooFundamentalFetcher};

/// Yahoo Finance Fundamental 동기화 옵션
#[derive(Debug, Default)]
pub struct YahooSyncOptions {
    /// 요청 간 딜레이 (ms)
    pub request_delay_ms: u64,
    /// 배치 크기 (None이면 전체)
    pub batch_size: Option<i64>,
    /// 중단점부터 재개
    pub resume: bool,
    /// N시간 이내 업데이트된 심볼 스킵
    pub stale_hours: Option<u32>,
    /// 특정 시장만 (US, JP, HK 등, None이면 전체)
    pub market_filter: Option<String>,
    /// 기존 값 강제 덮어쓰기 (기본: false - 기존 값 보존)
    pub force: bool,
}

/// Yahoo Finance를 통한 글로벌 시장 fundamental 데이터 동기화.
///
/// 미국, 일본, 홍콩 등 글로벌 주식의 펀더멘털 데이터를 수집합니다.
/// Naver Finance와 유사한 수준의 데이터를 제공합니다.
///
/// # 수집 항목
/// - 밸류에이션: PER, PBR, PSR, EPS, BPS, 배당수익률
/// - 시장 정보: 시가총액, 52주 고저, 평균 거래량
/// - 수익성: ROE, ROA, 영업이익률, 순이익률
/// - 성장성: 매출성장률, 이익성장률
/// - 안정성: 부채비율, 유동비율, 당좌비율
pub async fn sync_yahoo_fundamentals(
    pool: &PgPool,
    options: YahooSyncOptions,
) -> Result<FundamentalSyncStats> {
    info!("Yahoo Finance Fundamental 데이터 동기화 시작");

    let mut stats = FundamentalSyncStats {
        data_source: "YAHOO".to_string(),
        ..Default::default()
    };

    // 체크포인트 로드 (resume 모드)
    let resume_ticker = if options.resume {
        match checkpoint::load_checkpoint(pool, "yahoo_fundamental").await? {
            Some(t) => {
                info!(last_ticker = %t, "중단점부터 재개");
                Some(t)
            }
            None => {
                info!("이전 중단점 없음, 처음부터 시작");
                None
            }
        }
    } else {
        None
    };

    let limit = options.batch_size.unwrap_or(i64::MAX);

    // QueryBuilder로 SQL 인젝션 방지 (파라미터 바인딩 사용)
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
        SELECT si.id, si.yahoo_symbol
        FROM symbol_info si
        LEFT JOIN symbol_fundamental sf ON si.id = sf.symbol_info_id
        WHERE si.is_active = true
          AND si.yahoo_symbol IS NOT NULL
          AND si.yahoo_symbol != ''
        "#,
    );

    // 시장 필터 조건
    if let Some(ref m) = options.market_filter {
        qb.push(" AND si.market = ");
        qb.push_bind(m.clone());
    } else {
        // KR 시장은 Naver가 처리하므로 제외
        qb.push(" AND si.market != 'KR'");
    }

    // stale 조건
    if let Some(hours) = options.stale_hours {
        qb.push(" AND (sf.updated_at IS NULL OR sf.updated_at < NOW() - INTERVAL '");
        qb.push(hours.to_string());
        qb.push(" hours')");
    }

    // resume_ticker 조건 (파라미터 바인딩)
    if let Some(ref t) = resume_ticker {
        qb.push(" AND si.yahoo_symbol > ");
        qb.push_bind(t.clone());
    }

    qb.push(" ORDER BY si.yahoo_symbol LIMIT ");
    qb.push_bind(limit);

    let symbols: Vec<(Uuid, String)> = qb.build_query_as().fetch_all(pool).await?;

    let total = symbols.len();

    if options.stale_hours.is_some() {
        info!(count = total, stale_hours = ?options.stale_hours, "업데이트 필요한 심볼 조회 완료");
    } else {
        info!(count = total, "글로벌 심볼 조회 완료");
    }

    if symbols.is_empty() {
        checkpoint::save_checkpoint(
            pool,
            "yahoo_fundamental",
            "",
            0,
            CheckpointStatus::Completed,
        )
        .await?;
        return Ok(stats);
    }

    // 시작 상태 저장
    checkpoint::save_checkpoint(pool, "yahoo_fundamental", "", 0, CheckpointStatus::Running)
        .await?;

    // Yahoo Finance 크롤러 초기화
    let fetcher = YahooFundamentalFetcher::with_delay(Duration::from_millis(
        if options.request_delay_ms > 0 {
            options.request_delay_ms
        } else {
            500
        },
    ))
    .map_err(|e| CollectorError::DataSource(format!("Yahoo 크롤러 초기화 실패: {}", e)))?;

    for (idx, (symbol_info_id, yahoo_symbol)) in symbols.iter().enumerate() {
        stats.processed += 1;

        if (idx + 1) % 50 == 0 || idx + 1 == total {
            info!(
                progress = format!("{}/{}", idx + 1, total),
                "Yahoo Fundamental 수집 진행 중"
            );
            checkpoint::save_checkpoint(
                pool,
                "yahoo_fundamental",
                yahoo_symbol,
                stats.processed as i32,
                CheckpointStatus::Running,
            )
            .await?;
        }

        // Yahoo Finance에서 데이터 수집
        match fetcher.fetch_fundamental(yahoo_symbol).await {
            Ok(data) => {
                if let Err(e) = upsert_yahoo_fundamental(pool, *symbol_info_id, &data, options.force).await {
                    debug!(yahoo_symbol = yahoo_symbol, error = %e, "Yahoo 데이터 저장 실패");
                    stats.failed += 1;
                } else {
                    if data.per.is_some() || data.pbr.is_some() {
                        stats.valuation_updated += 1;
                    }
                    if data.market_cap.is_some() {
                        stats.market_cap_updated += 1;
                    }
                    if data.sector.is_some() {
                        stats.sector_updated += 1;
                    }
                    if data.week_52_high.is_some() || data.week_52_low.is_some() {
                        stats.week_52_updated += 1;
                    }
                }
            }
            Err(e) => {
                if matches!(
                    e,
                    trader_data::provider::yahoo_fundamental::YahooFundamentalError::RateLimited
                ) {
                    warn!(yahoo_symbol = yahoo_symbol, "Yahoo Rate limit 초과 - 5초 대기");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                } else {
                    debug!(yahoo_symbol = yahoo_symbol, error = %e, "Yahoo 데이터 수집 실패");
                }
                stats.failed += 1;
            }
        }

        // 요청 간 딜레이
        if idx + 1 < total {
            tokio::time::sleep(Duration::from_millis(
                if options.request_delay_ms > 0 {
                    options.request_delay_ms
                } else {
                    500
                },
            ))
            .await;
        }
    }

    // 완료 상태 저장
    checkpoint::save_checkpoint(
        pool,
        "yahoo_fundamental",
        "",
        stats.processed as i32,
        CheckpointStatus::Completed,
    )
    .await?;

    info!(
        processed = stats.processed,
        valuation = stats.valuation_updated,
        market_cap = stats.market_cap_updated,
        sector = stats.sector_updated,
        week_52 = stats.week_52_updated,
        failed = stats.failed,
        "Yahoo Finance Fundamental 데이터 동기화 완료"
    );

    Ok(stats)
}

/// Yahoo Finance 데이터를 symbol_fundamental 테이블에 저장 (Upsert).
///
/// # Arguments
/// * `force` - true: 새 값으로 덮어쓰기, false: 기존 값이 있으면 보존
async fn upsert_yahoo_fundamental(
    pool: &PgPool,
    symbol_info_id: Uuid,
    data: &YahooFundamentalData,
    force: bool,
) -> Result<()> {
    // force에 따라 COALESCE 순서 결정
    // force=true: COALESCE(EXCLUDED.xxx, symbol_fundamental.xxx) - 새 값 우선
    // force=false: COALESCE(symbol_fundamental.xxx, EXCLUDED.xxx) - 기존 값 우선
    let query = if force {
        r#"
        INSERT INTO symbol_fundamental (
            symbol_info_id, market_cap, shares_outstanding, float_shares,
            week_52_high, week_52_low, avg_volume_10d, avg_volume_3m,
            per, forward_per, pbr, psr, eps, bps,
            dividend_yield, roe, roa, gross_margin, operating_margin, net_profit_margin,
            debt_ratio, current_ratio, quick_ratio,
            revenue, net_income, revenue_growth_yoy, earnings_growth_yoy,
            data_source, currency, fetched_at, updated_at
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, 'YAHOO', $28, NOW(), NOW()
        )
        ON CONFLICT (symbol_info_id)
        DO UPDATE SET
            market_cap = COALESCE(EXCLUDED.market_cap, symbol_fundamental.market_cap),
            shares_outstanding = COALESCE(EXCLUDED.shares_outstanding, symbol_fundamental.shares_outstanding),
            float_shares = COALESCE(EXCLUDED.float_shares, symbol_fundamental.float_shares),
            week_52_high = COALESCE(EXCLUDED.week_52_high, symbol_fundamental.week_52_high),
            week_52_low = COALESCE(EXCLUDED.week_52_low, symbol_fundamental.week_52_low),
            avg_volume_10d = COALESCE(EXCLUDED.avg_volume_10d, symbol_fundamental.avg_volume_10d),
            avg_volume_3m = COALESCE(EXCLUDED.avg_volume_3m, symbol_fundamental.avg_volume_3m),
            per = COALESCE(EXCLUDED.per, symbol_fundamental.per),
            forward_per = COALESCE(EXCLUDED.forward_per, symbol_fundamental.forward_per),
            pbr = COALESCE(EXCLUDED.pbr, symbol_fundamental.pbr),
            psr = COALESCE(EXCLUDED.psr, symbol_fundamental.psr),
            eps = COALESCE(EXCLUDED.eps, symbol_fundamental.eps),
            bps = COALESCE(EXCLUDED.bps, symbol_fundamental.bps),
            dividend_yield = COALESCE(EXCLUDED.dividend_yield, symbol_fundamental.dividend_yield),
            roe = COALESCE(EXCLUDED.roe, symbol_fundamental.roe),
            roa = COALESCE(EXCLUDED.roa, symbol_fundamental.roa),
            gross_margin = COALESCE(EXCLUDED.gross_margin, symbol_fundamental.gross_margin),
            operating_margin = COALESCE(EXCLUDED.operating_margin, symbol_fundamental.operating_margin),
            net_profit_margin = COALESCE(EXCLUDED.net_profit_margin, symbol_fundamental.net_profit_margin),
            debt_ratio = COALESCE(EXCLUDED.debt_ratio, symbol_fundamental.debt_ratio),
            current_ratio = COALESCE(EXCLUDED.current_ratio, symbol_fundamental.current_ratio),
            quick_ratio = COALESCE(EXCLUDED.quick_ratio, symbol_fundamental.quick_ratio),
            revenue = COALESCE(EXCLUDED.revenue, symbol_fundamental.revenue),
            net_income = COALESCE(EXCLUDED.net_income, symbol_fundamental.net_income),
            revenue_growth_yoy = COALESCE(EXCLUDED.revenue_growth_yoy, symbol_fundamental.revenue_growth_yoy),
            earnings_growth_yoy = COALESCE(EXCLUDED.earnings_growth_yoy, symbol_fundamental.earnings_growth_yoy),
            data_source = 'YAHOO',
            fetched_at = NOW(),
            updated_at = NOW()
        "#
    } else {
        // 기존 값 보존 모드: 기존 값이 NULL인 경우에만 새 값 적용
        r#"
        INSERT INTO symbol_fundamental (
            symbol_info_id, market_cap, shares_outstanding, float_shares,
            week_52_high, week_52_low, avg_volume_10d, avg_volume_3m,
            per, forward_per, pbr, psr, eps, bps,
            dividend_yield, roe, roa, gross_margin, operating_margin, net_profit_margin,
            debt_ratio, current_ratio, quick_ratio,
            revenue, net_income, revenue_growth_yoy, earnings_growth_yoy,
            data_source, currency, fetched_at, updated_at
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, 'YAHOO', $28, NOW(), NOW()
        )
        ON CONFLICT (symbol_info_id)
        DO UPDATE SET
            market_cap = COALESCE(symbol_fundamental.market_cap, EXCLUDED.market_cap),
            shares_outstanding = COALESCE(symbol_fundamental.shares_outstanding, EXCLUDED.shares_outstanding),
            float_shares = COALESCE(symbol_fundamental.float_shares, EXCLUDED.float_shares),
            week_52_high = COALESCE(symbol_fundamental.week_52_high, EXCLUDED.week_52_high),
            week_52_low = COALESCE(symbol_fundamental.week_52_low, EXCLUDED.week_52_low),
            avg_volume_10d = COALESCE(symbol_fundamental.avg_volume_10d, EXCLUDED.avg_volume_10d),
            avg_volume_3m = COALESCE(symbol_fundamental.avg_volume_3m, EXCLUDED.avg_volume_3m),
            per = COALESCE(symbol_fundamental.per, EXCLUDED.per),
            forward_per = COALESCE(symbol_fundamental.forward_per, EXCLUDED.forward_per),
            pbr = COALESCE(symbol_fundamental.pbr, EXCLUDED.pbr),
            psr = COALESCE(symbol_fundamental.psr, EXCLUDED.psr),
            eps = COALESCE(symbol_fundamental.eps, EXCLUDED.eps),
            bps = COALESCE(symbol_fundamental.bps, EXCLUDED.bps),
            dividend_yield = COALESCE(symbol_fundamental.dividend_yield, EXCLUDED.dividend_yield),
            roe = COALESCE(symbol_fundamental.roe, EXCLUDED.roe),
            roa = COALESCE(symbol_fundamental.roa, EXCLUDED.roa),
            gross_margin = COALESCE(symbol_fundamental.gross_margin, EXCLUDED.gross_margin),
            operating_margin = COALESCE(symbol_fundamental.operating_margin, EXCLUDED.operating_margin),
            net_profit_margin = COALESCE(symbol_fundamental.net_profit_margin, EXCLUDED.net_profit_margin),
            debt_ratio = COALESCE(symbol_fundamental.debt_ratio, EXCLUDED.debt_ratio),
            current_ratio = COALESCE(symbol_fundamental.current_ratio, EXCLUDED.current_ratio),
            quick_ratio = COALESCE(symbol_fundamental.quick_ratio, EXCLUDED.quick_ratio),
            revenue = COALESCE(symbol_fundamental.revenue, EXCLUDED.revenue),
            net_income = COALESCE(symbol_fundamental.net_income, EXCLUDED.net_income),
            revenue_growth_yoy = COALESCE(symbol_fundamental.revenue_growth_yoy, EXCLUDED.revenue_growth_yoy),
            earnings_growth_yoy = COALESCE(symbol_fundamental.earnings_growth_yoy, EXCLUDED.earnings_growth_yoy),
            data_source = 'YAHOO',
            fetched_at = NOW(),
            updated_at = NOW()
        "#
    };

    sqlx::query(query)
        .bind(symbol_info_id)
        .bind(data.market_cap)
        .bind(data.shares_outstanding)
        .bind(data.float_shares)
        .bind(data.week_52_high)
        .bind(data.week_52_low)
        .bind(data.avg_volume_10d)
        .bind(data.avg_volume_3m)
        .bind(data.per)
        .bind(data.forward_per)
        .bind(data.pbr)
        .bind(data.psr)
        .bind(data.eps)
        .bind(data.bps)
        .bind(data.dividend_yield)
        .bind(data.roe)
        .bind(data.roa)
        .bind(data.gross_margin)
        .bind(data.operating_margin)
        .bind(data.net_profit_margin)
        .bind(data.debt_to_equity) // debt_ratio로 매핑
        .bind(data.current_ratio)
        .bind(data.quick_ratio)
        .bind(data.revenue)
        .bind(data.net_income)
        .bind(data.revenue_growth_yoy)
        .bind(data.earnings_growth_yoy)
        .bind(&data.currency)
        .execute(pool)
        .await?;

    // 섹터/산업 정보 업데이트 (force 모드일 때만 덮어쓰기)
    if data.sector.is_some() || data.industry.is_some() {
        let sector_query = if force {
            r#"
            UPDATE symbol_info
            SET sector = COALESCE($2, sector),
                updated_at = NOW()
            WHERE id = $1
            "#
        } else {
            // 기존 값 보존: sector가 NULL인 경우에만 업데이트
            r#"
            UPDATE symbol_info
            SET sector = COALESCE(sector, $2),
                updated_at = NOW()
            WHERE id = $1
            "#
        };

        sqlx::query(sector_query)
            .bind(symbol_info_id)
            .bind(&data.sector)
            .execute(pool)
            .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ticker() {
        assert_eq!(extract_ticker("KR7005930003"), "005930");
        assert_eq!(extract_ticker("005930"), "005930");
        assert_eq!(extract_ticker("KR7000660001"), "000660");
    }
}
