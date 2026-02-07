//! 데이터 로딩 함수들
//!
//! DB에서 캔들 데이터를 로드하거나 샘플 데이터를 생성하는 함수를 제공합니다.

use chrono::{NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use tracing::{debug, warn};

use trader_core::{Kline, MarketType, Symbol, Timeframe};
use trader_data::cache::CachedHistoricalDataProvider;

/// CachedHistoricalDataProvider를 통해 Kline 데이터 로드
///
/// ohlcv 테이블에서 통합 관리되는 데이터를 조회합니다.
/// 캐시에 데이터가 없으면 자동으로 Yahoo Finance에서 다운로드하여 캐싱합니다.
pub async fn load_klines_from_db(
    data_provider: &CachedHistoricalDataProvider,
    symbol_str: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<Vec<Kline>, String> {
    let provider = data_provider;

    // 날짜 범위를 거래일 기준 limit으로 변환 (주말 제외 대략 계산)
    let total_days = (end_date - start_date).num_days() as usize;
    let trading_days = (total_days as f64 * 5.0 / 7.0).ceil() as usize;
    let limit = trading_days.max(100); // 최소 100개

    debug!(
        symbol = symbol_str,
        start = %start_date,
        end = %end_date,
        limit = limit,
        "CachedHistoricalDataProvider로 캔들 데이터 로드"
    );

    // CachedHistoricalDataProvider가 캐시 조회 + 자동 다운로드 + 캐싱 처리
    let klines = provider
        .get_klines(symbol_str, Timeframe::D1, limit)
        .await
        .map_err(|e| format!("캔들 데이터 조회 실패: {}", e))?;

    // 날짜 범위 필터링
    let start_dt = Utc.from_utc_datetime(&start_date.and_hms_opt(0, 0, 0).unwrap());
    let end_dt = Utc.from_utc_datetime(&end_date.and_hms_opt(23, 59, 59).unwrap());

    let filtered: Vec<Kline> = klines
        .into_iter()
        .filter(|k| k.open_time >= start_dt && k.open_time <= end_dt)
        .collect();

    debug!(
        symbol = symbol_str,
        count = filtered.len(),
        "캔들 데이터 로드 완료"
    );

    Ok(filtered)
}

/// 샘플 Kline 데이터 생성 (DB 데이터가 없을 경우 사용)
pub fn generate_sample_klines(
    symbol_str: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<Kline> {
    use rust_decimal::prelude::FromPrimitive;

    let (base, quote) = parse_symbol(symbol_str);

    // Symbol 생성자를 통해 country 필드 자동 추론
    let symbol = Symbol::new(base, quote, MarketType::Stock);

    let days = (end_date - start_date).num_days() as usize;
    let base_price = 50000.0_f64; // 기본 가격

    (0..=days)
        .map(|i| {
            let date = start_date + chrono::Duration::days(i as i64);
            let open_time = Utc.from_utc_datetime(&date.and_hms_opt(9, 0, 0).unwrap());
            let close_time = Utc.from_utc_datetime(&date.and_hms_opt(15, 30, 0).unwrap());

            // 랜덤한 가격 변동 시뮬레이션
            let noise = ((i as f64 * 0.7).sin() + (i as f64 * 1.3).cos()) * 0.02;
            let trend = i as f64 * 0.001;
            let price_mult = 1.0 + noise + trend;

            let open = base_price * price_mult;
            let high = open * 1.02;
            let low = open * 0.98;
            let close = open * (1.0 + noise * 0.5);
            let volume = 1000000.0 * (1.0 + noise.abs());

            Kline {
                ticker: symbol.to_string(),
                timeframe: Timeframe::D1,
                open_time,
                close_time,
                open: Decimal::from_f64(open).unwrap_or(Decimal::from(50000)),
                high: Decimal::from_f64(high).unwrap_or(Decimal::from(51000)),
                low: Decimal::from_f64(low).unwrap_or(Decimal::from(49000)),
                close: Decimal::from_f64(close).unwrap_or(Decimal::from(50500)),
                volume: Decimal::from_f64(volume).unwrap_or(Decimal::from(1000000)),
                quote_volume: None,
                num_trades: None,
            }
        })
        .collect()
}

/// 특정 타임프레임의 Kline 데이터 로드
///
/// ohlcv 테이블에서 지정된 타임프레임의 데이터를 조회합니다.
#[allow(dead_code)]
pub async fn load_klines_with_timeframe(
    data_provider: &CachedHistoricalDataProvider,
    symbol_str: &str,
    timeframe: Timeframe,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<Vec<Kline>, String> {
    let provider = data_provider;

    // 날짜 범위를 캔들 개수로 변환
    let total_days = (end_date - start_date).num_days() as usize;
    let limit = match timeframe {
        Timeframe::M1 => total_days * 24 * 60,
        Timeframe::M3 => total_days * 24 * 20,
        Timeframe::M5 => total_days * 24 * 12,
        Timeframe::M15 => total_days * 24 * 4,
        Timeframe::M30 => total_days * 24 * 2,
        Timeframe::H1 => total_days * 24,
        Timeframe::H2 => total_days * 12,
        Timeframe::H4 => total_days * 6,
        Timeframe::H6 => total_days * 4,
        Timeframe::H8 => total_days * 3,
        Timeframe::H12 => total_days * 2,
        Timeframe::D1 => (total_days as f64 * 5.0 / 7.0).ceil() as usize,
        Timeframe::D3 => total_days / 3 + 1,
        Timeframe::W1 => total_days / 7 + 1,
        Timeframe::MN1 => total_days / 30 + 1,
    };
    let limit = limit.max(60); // 최소 60개

    debug!(
        symbol = symbol_str,
        ?timeframe,
        start = %start_date,
        end = %end_date,
        limit = limit,
        "타임프레임별 캔들 데이터 로드"
    );

    let klines = provider
        .get_klines(symbol_str, timeframe, limit)
        .await
        .map_err(|e| format!("캔들 데이터 조회 실패 ({}): {}", timeframe, e))?;

    // 날짜 범위 필터링
    let start_dt = Utc.from_utc_datetime(&start_date.and_hms_opt(0, 0, 0).unwrap());
    let end_dt = Utc.from_utc_datetime(&end_date.and_hms_opt(23, 59, 59).unwrap());

    let filtered: Vec<Kline> = klines
        .into_iter()
        .filter(|k| k.open_time >= start_dt && k.open_time <= end_dt)
        .collect();

    debug!(
        symbol = symbol_str,
        ?timeframe,
        count = filtered.len(),
        "타임프레임별 캔들 데이터 로드 완료"
    );

    Ok(filtered)
}

/// 다중 타임프레임 데이터 로드
///
/// 각 타임프레임별로 지정된 개수의 캔들 데이터를 HashMap으로 반환합니다.
#[allow(dead_code)]
pub async fn load_secondary_timeframe_klines(
    data_provider: &CachedHistoricalDataProvider,
    symbol_str: &str,
    secondary_timeframes: &[(Timeframe, usize)],
    end_date: NaiveDate,
) -> HashMap<Timeframe, Vec<Kline>> {
    let provider = data_provider;
    let mut result = HashMap::new();

    for (timeframe, count) in secondary_timeframes {
        debug!(
            symbol = symbol_str,
            ?timeframe,
            count = count,
            "Secondary 타임프레임 데이터 로드"
        );

        match provider.get_klines(symbol_str, *timeframe, *count).await {
            Ok(klines) => {
                // 날짜 필터링: end_date 이전 데이터만
                let end_dt = Utc.from_utc_datetime(&end_date.and_hms_opt(23, 59, 59).unwrap());
                let filtered: Vec<Kline> = klines
                    .into_iter()
                    .filter(|k| k.close_time <= end_dt)
                    .collect();

                debug!(
                    symbol = symbol_str,
                    ?timeframe,
                    count = filtered.len(),
                    "Secondary 타임프레임 데이터 로드 완료"
                );
                result.insert(*timeframe, filtered);
            }
            Err(e) => {
                warn!(
                    symbol = symbol_str,
                    ?timeframe,
                    error = %e,
                    "Secondary 타임프레임 데이터 로드 실패"
                );
            }
        }
    }

    result
}

/// 다중 심볼의 Kline 데이터를 CachedHistoricalDataProvider로 로드
///
/// 각 심볼에 대해 캐시 조회 + 자동 다운로드 + 캐싱이 처리됩니다.
pub async fn load_multi_klines_from_db(
    data_provider: &CachedHistoricalDataProvider,
    symbols: &[String],
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<HashMap<String, Vec<Kline>>, String> {
    let mut result = HashMap::new();

    for symbol_str in symbols {
        match load_klines_from_db(data_provider, symbol_str, start_date, end_date).await {
            Ok(klines) if !klines.is_empty() => {
                debug!("심볼 {} 캔들 {} 개 로드 완료", symbol_str, klines.len());
                result.insert(symbol_str.clone(), klines);
            }
            Ok(_) => {
                warn!("심볼 {} 데이터 없음", symbol_str);
            }
            Err(e) => {
                warn!("심볼 {} 로드 실패: {}", symbol_str, e);
            }
        }
    }

    Ok(result)
}

/// 다중 심볼 Kline 데이터를 시간순으로 병합
pub fn merge_multi_klines(multi_klines: &HashMap<String, Vec<Kline>>) -> Vec<Kline> {
    let mut all_klines: Vec<Kline> = multi_klines
        .values()
        .flat_map(|klines| klines.iter().cloned())
        .collect();

    // 시간순 정렬
    all_klines.sort_by(|a, b| a.open_time.cmp(&b.open_time));

    all_klines
}

/// 심볼 문자열을 base/quote로 파싱
pub fn parse_symbol(symbol_str: &str) -> (String, String) {
    if symbol_str.contains('/') {
        let parts: Vec<&str> = symbol_str.split('/').collect();
        (
            parts[0].to_string(),
            parts
                .get(1)
                .map(|s| s.to_string())
                .unwrap_or("KRW".to_string()),
        )
    } else if symbol_str.chars().all(|c| c.is_ascii_digit()) {
        (symbol_str.to_string(), "KRW".to_string())
    } else {
        (symbol_str.to_string(), "USD".to_string())
    }
}

/// 전략별로 필요한 모든 심볼을 확장
///
/// 사용자가 입력한 심볼 외에 전략이 필요로 하는 추가 심볼을 자동으로 포함합니다.
pub fn expand_strategy_symbols(strategy_id: &str, user_symbols: &[String]) -> Vec<String> {
    let mut symbols: HashSet<String> = user_symbols.iter().cloned().collect();

    // 전략별 필수 심볼 추가
    let required_symbols: &[&str] = match strategy_id {
        "compound_momentum" => &["TQQQ", "SCHD", "TMF", "PFIX"],
        "haa" => &["SPY", "TLT", "VEA", "VWO", "TIP", "BIL", "IEF"],
        "xaa" => &["SPY", "QQQ", "TLT", "IEF", "VEA", "VWO", "PDBC", "VNQ"],
        "stock_rotation" => &[], // 사용자 지정 심볼만 사용
        // 올웨더: SPY, TLT, IEF, GLD, PDBC, IYK
        "all_weather" => &["SPY", "TLT", "IEF", "GLD", "PDBC", "IYK"],
        // 모멘텀 파워 US: UPRO, TLT, BIL, TIP
        "momentum_power" => &["UPRO", "TLT", "BIL", "TIP"],
        // BAA: 카나리아(SPY, VEA, VWO, BND) + 공격(QQQ, IWM) + 방어(TIP, DBC, BIL, IEF, TLT)
        "baa" => &[
            "SPY", "VEA", "VWO", "BND", "QQQ", "IWM", "TIP", "DBC", "BIL", "IEF", "TLT",
        ],
        // 섹터 모멘텀 US
        "sector_momentum" => &[
            "XLK", "XLF", "XLV", "XLY", "XLP", "XLE", "XLI", "XLB", "XLU", "XLRE", "XLC",
        ],
        // 듀얼 모멘텀: 한국 주식 + 미국 채권
        "dual_momentum" => &["069500", "229200", "TLT", "IEF", "BIL"],
        // 연금 자동화
        "pension_bot" => &[
            "448290", "379780", "294400", "305080", "148070", "319640", "130730",
        ],
        // 시총 TOP
        "market_cap_top" => &[
            "AAPL", "MSFT", "GOOGL", "AMZN", "NVDA", "META", "TSLA", "BRK-B", "UNH", "JPM",
        ],
        // 시장 양방향: 레버리지 + 인버스
        "market_both_side" => &["122630", "252670"],
        // 모멘텀 급등: 레버리지 ETF
        "momentum_surge" => &["122630", "233740", "252670", "251340"],
        _ => &[],
    };

    for sym in required_symbols {
        symbols.insert(sym.to_string());
    }

    // 정렬된 벡터로 변환
    let mut result: Vec<String> = symbols.into_iter().collect();
    result.sort();
    result
}
