//! 매크로 경제 지표 동기화 모듈.
//!
//! Yahoo Finance API를 통해 주요 시장 지표를 주기적으로 조회하여
//! Redis 캐시에 저장합니다.
//!
//! # 지원 지표
//! - KOSPI (^KS11)
//! - KOSDAQ (^KQ11)
//! - USD/KRW (KRW=X)
//! - VIX (^VIX)
//! - NASDAQ (^IXIC)

use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, warn};

use trader_data::cache::{MacroDataProvider, MacroDataProviderTrait, RedisCache};

use crate::Result;

/// 매크로 데이터 캐시 키 (API와 동일)
const MACRO_DATA_CACHE_KEY: &str = "macro:data";

/// 매크로 데이터 캐시 TTL (10분)
/// 동기화 주기(5분)보다 충분히 길게 설정하여 캐시 갱신 전 만료 방지
const MACRO_DATA_CACHE_TTL_SECS: u64 = 600;

/// 매크로 데이터 동기화 결과.
#[derive(Debug)]
pub struct MacroSyncResult {
    /// 성공 여부
    pub success: bool,
    /// KOSPI 지수
    pub kospi: Option<String>,
    /// KOSPI 변동률
    pub kospi_change_pct: Option<f64>,
    /// KOSDAQ 지수
    pub kosdaq: Option<String>,
    /// KOSDAQ 변동률
    pub kosdaq_change_pct: Option<f64>,
    /// USD/KRW 환율
    pub usd_krw: Option<String>,
    /// USD 변동률
    pub usd_change_pct: Option<f64>,
    /// VIX 지수
    pub vix: Option<String>,
    /// VIX 변동률
    pub vix_change_pct: Option<f64>,
    /// NASDAQ 지수
    pub nasdaq: Option<String>,
    /// NASDAQ 변동률
    pub nasdaq_change_pct: Option<f64>,
    /// 소요 시간 (ms)
    pub elapsed_ms: u64,
    /// 에러 메시지 (실패 시)
    pub error: Option<String>,
}

/// 매크로 경제 지표 동기화.
///
/// Yahoo Finance API를 통해 USD/KRW와 NASDAQ 데이터를 조회하여
/// Redis 캐시에 저장합니다.
///
/// # 인자
/// * `cache` - Redis 캐시 인스턴스
///
/// # 반환
/// * 동기화 결과
pub async fn sync_macro_data(cache: &RedisCache) -> Result<MacroSyncResult> {
    let start = Instant::now();

    // MacroDataProvider 생성
    let provider = match MacroDataProvider::new() {
        Ok(p) => p,
        Err(e) => {
            error!("MacroDataProvider 생성 실패: {}", e);
            return Ok(MacroSyncResult {
                success: false,
                kospi: None,
                kospi_change_pct: None,
                kosdaq: None,
                kosdaq_change_pct: None,
                usd_krw: None,
                usd_change_pct: None,
                vix: None,
                vix_change_pct: None,
                nasdaq: None,
                nasdaq_change_pct: None,
                elapsed_ms: start.elapsed().as_millis() as u64,
                error: Some(format!("Provider 생성 실패: {}", e)),
            });
        }
    };

    // 매크로 데이터 조회
    let data = match provider.fetch_macro_data().await {
        Ok(d) => d,
        Err(e) => {
            warn!("매크로 데이터 조회 실패: {}", e);
            return Ok(MacroSyncResult {
                success: false,
                kospi: None,
                kospi_change_pct: None,
                kosdaq: None,
                kosdaq_change_pct: None,
                usd_krw: None,
                usd_change_pct: None,
                vix: None,
                vix_change_pct: None,
                nasdaq: None,
                nasdaq_change_pct: None,
                elapsed_ms: start.elapsed().as_millis() as u64,
                error: Some(format!("API 조회 실패: {}", e)),
            });
        }
    };

    // Redis 캐시에 저장
    if let Err(e) = cache
        .set_with_ttl(MACRO_DATA_CACHE_KEY, &data, MACRO_DATA_CACHE_TTL_SECS)
        .await
    {
        error!("매크로 데이터 캐시 저장 실패: {}", e);
        return Ok(MacroSyncResult {
            success: false,
            kospi: Some(data.kospi_close.to_string()),
            kospi_change_pct: Some(data.kospi_change_pct),
            kosdaq: Some(data.kosdaq_close.to_string()),
            kosdaq_change_pct: Some(data.kosdaq_change_pct),
            usd_krw: Some(data.usd_krw.to_string()),
            usd_change_pct: Some(data.usd_change_pct),
            vix: Some(data.vix_close.to_string()),
            vix_change_pct: Some(data.vix_change_pct),
            nasdaq: Some(data.nasdaq_close.to_string()),
            nasdaq_change_pct: Some(data.nasdaq_change_pct),
            elapsed_ms: start.elapsed().as_millis() as u64,
            error: Some(format!("캐시 저장 실패: {}", e)),
        });
    }

    let elapsed_ms = start.elapsed().as_millis() as u64;

    info!(
        kospi = %data.kospi_close,
        kospi_change = %format!("{:+.2}%", data.kospi_change_pct),
        kosdaq = %data.kosdaq_close,
        kosdaq_change = %format!("{:+.2}%", data.kosdaq_change_pct),
        usd_krw = %data.usd_krw,
        usd_change = %format!("{:+.2}%", data.usd_change_pct),
        vix = %data.vix_close,
        vix_change = %format!("{:+.2}%", data.vix_change_pct),
        nasdaq_change = %format!("{:+.2}%", data.nasdaq_change_pct),
        elapsed_ms = elapsed_ms,
        "매크로 데이터 동기화 완료"
    );

    Ok(MacroSyncResult {
        success: true,
        kospi: Some(data.kospi_close.to_string()),
        kospi_change_pct: Some(data.kospi_change_pct),
        kosdaq: Some(data.kosdaq_close.to_string()),
        kosdaq_change_pct: Some(data.kosdaq_change_pct),
        usd_krw: Some(data.usd_krw.to_string()),
        usd_change_pct: Some(data.usd_change_pct),
        vix: Some(data.vix_close.to_string()),
        vix_change_pct: Some(data.vix_change_pct),
        nasdaq: Some(data.nasdaq_close.to_string()),
        nasdaq_change_pct: Some(data.nasdaq_change_pct),
        elapsed_ms,
        error: None,
    })
}

/// 매크로 데이터 동기화 (Arc<RedisCache> 버전).
///
/// Scheduler에서 사용하기 위한 래퍼 함수.
pub async fn sync_macro_data_arc(cache: Arc<RedisCache>) -> Result<MacroSyncResult> {
    sync_macro_data(&cache).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macro_sync_result() {
        let result = MacroSyncResult {
            success: true,
            kospi: Some("2650.00".to_string()),
            kospi_change_pct: Some(0.45),
            kosdaq: Some("850.00".to_string()),
            kosdaq_change_pct: Some(-0.12),
            usd_krw: Some("1350.00".to_string()),
            usd_change_pct: Some(0.5),
            vix: Some("15.32".to_string()),
            vix_change_pct: Some(-2.31),
            nasdaq: Some("16500.00".to_string()),
            nasdaq_change_pct: Some(-1.2),
            elapsed_ms: 100,
            error: None,
        };

        assert!(result.success);
        assert_eq!(result.usd_krw, Some("1350.00".to_string()));
        assert_eq!(result.kospi, Some("2650.00".to_string()));
    }
}
