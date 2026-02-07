//! Fundamental 데이터 수집기.
//!
//! Yahoo Finance API를 통해 펀더멘털 데이터를 수집합니다.
//! 주요 지표: 시가총액, PER, PBR, ROE, 배당수익률 등.

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::{DataError, Result};

/// f64를 Decimal로 변환 후 소수점 4자리로 반올림.
///
/// PostgreSQL DECIMAL(20, 4) 컬럼에 저장 가능하도록 정밀도를 제한합니다.
/// 부동소수점 변환 시 발생하는 무한 소수점 문제를 방지합니다.
fn round_decimal_from_f64(value: f64) -> Option<Decimal> {
    Decimal::from_f64(value).map(|d| d.round_dp(4))
}

/// f64를 Decimal로 변환 후 소수점 2자리로 반올림.
///
/// 퍼센트(%) 값 등 소수점 2자리가 적절한 경우 사용합니다.
fn round_decimal_from_f64_dp2(value: f64) -> Option<Decimal> {
    Decimal::from_f64(value).map(|d| d.round_dp(2))
}
use trader_core::{Kline, MarketType, Timeframe};

/// Yahoo Finance에서 가져온 Fundamental 데이터.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FundamentalData {
    pub symbol: String,

    // 시장 데이터
    pub market_cap: Option<Decimal>,
    pub shares_outstanding: Option<i64>,
    pub float_shares: Option<i64>,

    // 가격 관련
    pub week_52_high: Option<Decimal>,
    pub week_52_low: Option<Decimal>,
    pub avg_volume_10d: Option<i64>,
    pub avg_volume_3m: Option<i64>,

    // 밸류에이션
    pub per: Option<Decimal>,         // trailing PE
    pub forward_per: Option<Decimal>, // forward PE
    pub pbr: Option<Decimal>,         // price to book
    pub psr: Option<Decimal>,         // price to sales
    pub ev_ebitda: Option<Decimal>,

    // 주당 지표
    pub eps: Option<Decimal>, // trailing EPS
    pub bps: Option<Decimal>, // book value per share
    pub dps: Option<Decimal>, // dividend per share

    // 배당
    pub dividend_yield: Option<Decimal>,
    pub dividend_payout_ratio: Option<Decimal>,
    pub ex_dividend_date: Option<NaiveDate>,

    // 수익성 지표
    pub roe: Option<Decimal>,
    pub roa: Option<Decimal>,
    pub operating_margin: Option<Decimal>,
    pub net_profit_margin: Option<Decimal>,
    pub gross_margin: Option<Decimal>,

    // 안정성 지표
    pub debt_ratio: Option<Decimal>,
    pub current_ratio: Option<Decimal>,
    pub quick_ratio: Option<Decimal>,

    // 성장성 지표
    pub revenue_growth_yoy: Option<Decimal>,
    pub earnings_growth_yoy: Option<Decimal>,

    // 메타데이터
    pub currency: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

/// Fundamental 데이터 수집기.
///
/// `yahoo_finance_api` 크레이트를 사용하여 Yahoo Finance API에서
/// 펀더멘털 데이터를 수집합니다. 인증(crumb 토큰)은 자동 처리됩니다.
pub struct FundamentalFetcher {
    /// Yahoo Finance API 커넥터 (get_ticker_info 메서드용 mutable 필요)
    connector: yahoo_finance_api::YahooConnector,
}

impl FundamentalFetcher {
    /// 새 FundamentalFetcher 생성.
    pub fn new() -> Result<Self> {
        let connector = yahoo_finance_api::YahooConnector::new()
            .map_err(|e| DataError::ConnectionError(format!("Yahoo Finance 연결 실패: {}", e)))?;
        Ok(Self { connector })
    }

    /// Yahoo Finance에서 Fundamental 데이터 수집.
    ///
    /// Yahoo Finance는 quote summary API를 통해 다양한 지표를 제공합니다.
    /// 한국 주식: 005930.KS 형식으로 조회.
    pub async fn fetch(&self, yahoo_symbol: &str) -> Result<FundamentalData> {
        debug!(symbol = yahoo_symbol, "Fundamental 데이터 수집 시작");

        // 1. 기본 시세 데이터 조회 (search_ticker로 요약 정보 가져오기)
        let search_result = self
            .connector
            .search_ticker(yahoo_symbol)
            .await
            .map_err(|e| {
                DataError::FetchError(format!("Yahoo Finance 검색 실패 ({}): {}", yahoo_symbol, e))
            })?;

        // 2. 최근 1개월 가격 데이터로 52주 고저가 및 평균 거래량 계산
        let price_data = self.fetch_price_statistics(yahoo_symbol).await;

        let mut data = FundamentalData {
            symbol: yahoo_symbol.to_string(),
            fetched_at: Utc::now(),
            ..Default::default()
        };

        // search_result에서 기본 정보 추출
        if let Some(quote) = search_result.quotes.first() {
            // 티커 검색 결과에서는 제한된 정보만 제공됨
            debug!(
                symbol = yahoo_symbol,
                name = ?quote.short_name,
                "검색 결과 확인"
            );
        }

        // 3. 가격 통계 적용
        if let Ok((high_52w, low_52w, avg_vol_10d, avg_vol_3m)) = price_data {
            data.week_52_high = high_52w;
            data.week_52_low = low_52w;
            data.avg_volume_10d = avg_vol_10d;
            data.avg_volume_3m = avg_vol_3m;
        }

        // 4. 최근 시세 정보로 현재가 기반 지표 계산
        if let Ok(current_quote) = self.fetch_latest_quote(yahoo_symbol).await {
            data.market_cap = current_quote.market_cap;
            data.per = current_quote.per;
            data.eps = current_quote.eps;
            data.dividend_yield = current_quote.dividend_yield;

            // PBR 계산: 시가총액 / (주당순자산 * 발행주식수)
            // Yahoo Finance에서 직접 제공하지 않는 경우 OHLCV 기반으로 추정
        }

        info!(
            symbol = yahoo_symbol,
            market_cap = ?data.market_cap,
            per = ?data.per,
            "Fundamental 데이터 수집 완료"
        );

        Ok(data)
    }

    /// 가격 통계 수집 (52주 고저가, 평균 거래량).
    async fn fetch_price_statistics(
        &self,
        symbol: &str,
    ) -> Result<(Option<Decimal>, Option<Decimal>, Option<i64>, Option<i64>)> {
        // 1년치 일봉 데이터 조회
        let response = self
            .connector
            .get_quote_range(symbol, "1d", "1y")
            .await
            .map_err(|e| {
                DataError::FetchError(format!("가격 통계 조회 실패 ({}): {}", symbol, e))
            })?;

        let quotes = response
            .quotes()
            .map_err(|e| DataError::ParseError(format!("Quote 파싱 오류: {}", e)))?;

        if quotes.is_empty() {
            return Ok((None, None, None, None));
        }

        // 52주 고저가 계산 (소수점 4자리로 반올림)
        let high_52w = quotes
            .iter()
            .map(|q| q.high)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .and_then(round_decimal_from_f64);

        let low_52w = quotes
            .iter()
            .map(|q| q.low)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .and_then(round_decimal_from_f64);

        // 평균 거래량 계산
        let recent_10 = quotes.iter().rev().take(10);
        let avg_vol_10d: Option<i64> = {
            let volumes: Vec<u64> = recent_10.clone().map(|q| q.volume).collect();
            if volumes.is_empty() {
                None
            } else {
                Some((volumes.iter().sum::<u64>() / volumes.len() as u64) as i64)
            }
        };

        let recent_63 = quotes.iter().rev().take(63); // 약 3개월
        let avg_vol_3m: Option<i64> = {
            let volumes: Vec<u64> = recent_63.map(|q| q.volume).collect();
            if volumes.is_empty() {
                None
            } else {
                Some((volumes.iter().sum::<u64>() / volumes.len() as u64) as i64)
            }
        };

        Ok((high_52w, low_52w, avg_vol_10d, avg_vol_3m))
    }

    /// 최신 시세 정보 조회.
    async fn fetch_latest_quote(&self, symbol: &str) -> Result<LatestQuote> {
        // 최근 5일 데이터로 현재가 정보 조회
        let response = self
            .connector
            .get_quote_range(symbol, "1d", "5d")
            .await
            .map_err(|e| {
                DataError::FetchError(format!("최신 시세 조회 실패 ({}): {}", symbol, e))
            })?;

        let quotes = response
            .quotes()
            .map_err(|e| DataError::ParseError(format!("Quote 파싱 오류: {}", e)))?;

        if quotes.is_empty() {
            return Err(DataError::FetchError(format!(
                "시세 데이터 없음: {}",
                symbol
            )));
        }

        let latest = quotes.last().unwrap();

        // Yahoo Finance quote에서 직접 가져올 수 있는 정보는 제한적
        // 메타데이터에서 추가 정보 추출 시도
        let metadata = response.metadata();

        let mut quote = LatestQuote::default();

        // 메타데이터에서 가용한 정보 추출
        if let Ok(meta) = metadata {
            // 시가총액 (장 마감 시세 * 발행주식수)
            // yahoo_finance_api 4.1에서는 메타데이터에 제한적 정보만 제공
            // 대략적인 계산 시도
            quote.current_price = round_decimal_from_f64(latest.close);

            // 통화 정보
            quote.currency = meta.currency.clone();
        } else {
            warn!(symbol = symbol, "메타데이터 조회 실패, 기본값 사용");
            quote.current_price = round_decimal_from_f64(latest.close);
        }

        Ok(quote)
    }

    /// Fundamental 데이터 수집 (재시도 포함).
    ///
    /// 네트워크 오류나 일시적 API 실패 시 지정된 횟수만큼 재시도합니다.
    async fn fetch_fundamental_with_retry(
        &mut self,
        symbol: &str,
        max_retries: u32,
    ) -> Option<LatestQuote> {
        for attempt in 1..=max_retries {
            match self.fetch_fundamental_from_ticker_info(symbol).await {
                Ok(quote) => return Some(quote),
                Err(e) => {
                    if attempt < max_retries {
                        debug!(
                            symbol = symbol,
                            attempt = attempt,
                            max_retries = max_retries,
                            error = %e,
                            "Fundamental 수집 재시도 예정"
                        );
                        // 재시도 전 짧은 대기 (exponential backoff: 500ms, 1000ms, ...)
                        tokio::time::sleep(std::time::Duration::from_millis(500 * attempt as u64))
                            .await;
                    } else {
                        warn!(
                            symbol = symbol,
                            attempts = max_retries,
                            error = %e,
                            "Fundamental 수집 최종 실패 (OHLCV만 저장됨)"
                        );
                    }
                }
            }
        }
        None
    }

    /// Yahoo Finance `get_ticker_info` API를 사용하여 Fundamental 데이터 수집.
    ///
    /// 시가총액, PER, PBR, ROE, 배당수익률 등 핵심 지표를 수집합니다.
    /// `yahoo_finance_api` 크레이트가 crumb 토큰 인증을 자동 처리합니다.
    async fn fetch_fundamental_from_ticker_info(&mut self, symbol: &str) -> Result<LatestQuote> {
        let summary = self.connector.get_ticker_info(symbol).await.map_err(|e| {
            DataError::FetchError(format!("Yahoo ticker info 조회 실패 ({}): {}", symbol, e))
        })?;

        // quote_summary에서 데이터 추출
        let quote_summary = summary.quote_summary.ok_or_else(|| {
            DataError::FetchError(format!("Yahoo ticker info 결과 없음: {}", symbol))
        })?;

        let result_data = quote_summary
            .result
            .and_then(|r| r.into_iter().next())
            .ok_or_else(|| {
                DataError::FetchError(format!("Yahoo ticker info 결과 비어있음: {}", symbol))
            })?;

        // SummaryDetail에서 시가총액, PER, 배당수익률 추출
        let summary_detail = result_data.summary_detail.as_ref();
        let market_cap = summary_detail
            .and_then(|sd| sd.market_cap)
            .and_then(Decimal::from_u64);
        let trailing_pe = summary_detail
            .and_then(|sd| sd.trailing_pe)
            .and_then(round_decimal_from_f64);
        let forward_pe_from_sd = summary_detail
            .and_then(|sd| sd.forward_pe)
            .and_then(round_decimal_from_f64);
        let dividend_yield = summary_detail
            .and_then(|sd| sd.trailing_annual_dividend_yield)
            .and_then(|v| round_decimal_from_f64_dp2(v * 100.0)); // % 변환
        let payout_ratio = summary_detail
            .and_then(|sd| sd.payout_ratio)
            .and_then(|v| round_decimal_from_f64_dp2(v * 100.0)); // % 변환
        let ex_dividend_date = summary_detail
            .and_then(|sd| sd.ex_dividend_date)
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.date_naive()));

        // DefaultKeyStatistics에서 PBR, EPS, BPS, 발행주식수 추출
        let key_stats = result_data.default_key_statistics.as_ref();
        let price_to_book = key_stats
            .and_then(|ks| ks.price_to_book)
            .and_then(round_decimal_from_f64);
        let trailing_eps = key_stats
            .and_then(|ks| ks.trailing_eps)
            .and_then(round_decimal_from_f64);
        let book_value = key_stats
            .and_then(|ks| ks.book_value)
            .and_then(round_decimal_from_f64);
        let forward_pe = key_stats
            .and_then(|ks| ks.forward_pe)
            .and_then(round_decimal_from_f64)
            .or(forward_pe_from_sd); // SummaryDetail fallback
        let shares_outstanding = key_stats
            .and_then(|ks| ks.shares_outstanding)
            .map(|v| v as i64);
        let float_shares = key_stats.and_then(|ks| ks.float_shares).map(|v| v as i64);

        // FinancialData에서 ROE, ROA, 마진율 추출 (퍼센트는 소수점 2자리로 반올림)
        let financial_data = result_data.financial_data.as_ref();
        let roe = financial_data
            .and_then(|fd| fd.return_on_equity)
            .and_then(|v| round_decimal_from_f64_dp2(v * 100.0)); // % 변환
        let roa = financial_data
            .and_then(|fd| fd.return_on_assets)
            .and_then(|v| round_decimal_from_f64_dp2(v * 100.0)); // % 변환
        let operating_margin = financial_data
            .and_then(|fd| fd.operating_margins)
            .and_then(|v| round_decimal_from_f64_dp2(v * 100.0)); // % 변환
        let profit_margin = financial_data
            .and_then(|fd| fd.profit_margins)
            .and_then(|v| round_decimal_from_f64_dp2(v * 100.0)); // % 변환
        let gross_margin = financial_data
            .and_then(|fd| fd.gross_margins)
            .and_then(|v| round_decimal_from_f64_dp2(v * 100.0)); // % 변환
        let current_ratio = financial_data
            .and_then(|fd| fd.current_ratio)
            .and_then(round_decimal_from_f64);
        let quick_ratio = financial_data
            .and_then(|fd| fd.quick_ratio)
            .and_then(round_decimal_from_f64);
        let debt_to_equity = financial_data
            .and_then(|fd| fd.debt_to_equity)
            .and_then(round_decimal_from_f64);
        let total_revenue = financial_data
            .and_then(|fd| fd.total_revenue)
            .and_then(Decimal::from_i64);
        let revenue_growth = financial_data
            .and_then(|fd| fd.revenue_growth)
            .and_then(|v| round_decimal_from_f64_dp2(v * 100.0)); // % 변환
        let earnings_growth = financial_data
            .and_then(|fd| fd.earnings_growth)
            .and_then(|v| round_decimal_from_f64_dp2(v * 100.0)); // % 변환
        let currency = financial_data.and_then(|fd| fd.financial_currency.clone());

        // QuoteType에서 종목명 추출
        let name = result_data
            .quote_type
            .as_ref()
            .and_then(|qt| qt.long_name.clone().or(qt.short_name.clone()));

        let quote = LatestQuote {
            name,
            current_price: None, // OHLCV에서 가져옴
            market_cap,
            per: trailing_pe,
            forward_per: forward_pe,
            pbr: price_to_book,
            eps: trailing_eps,
            bps: book_value,
            dividend_yield,
            dividend_payout_ratio: payout_ratio,
            ex_dividend_date,
            roe,
            roa,
            operating_margin,
            profit_margin,
            gross_margin,
            current_ratio,
            quick_ratio,
            debt_ratio: debt_to_equity, // debt_to_equity를 부채비율로 사용
            revenue: total_revenue,
            revenue_growth_yoy: revenue_growth,
            earnings_growth_yoy: earnings_growth,
            shares_outstanding,
            float_shares,
            currency,
        };

        debug!(
            symbol = symbol,
            name = ?quote.name,
            market_cap = ?quote.market_cap,
            per = ?quote.per,
            pbr = ?quote.pbr,
            roe = ?quote.roe,
            "Yahoo ticker info Fundamental 데이터 수집 완료"
        );

        Ok(quote)
    }
}

/// 최신 시세 및 Fundamental 정보.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // API 응답 전체 필드 매핑 (일부만 사용)
struct LatestQuote {
    /// 종목명 (한글/영문)
    name: Option<String>,
    current_price: Option<Decimal>,
    market_cap: Option<Decimal>,
    per: Option<Decimal>,
    forward_per: Option<Decimal>,
    pbr: Option<Decimal>,
    eps: Option<Decimal>,
    bps: Option<Decimal>,
    dividend_yield: Option<Decimal>,
    dividend_payout_ratio: Option<Decimal>,
    ex_dividend_date: Option<NaiveDate>,
    roe: Option<Decimal>,
    roa: Option<Decimal>,
    operating_margin: Option<Decimal>,
    profit_margin: Option<Decimal>,
    gross_margin: Option<Decimal>,
    current_ratio: Option<Decimal>,
    quick_ratio: Option<Decimal>,
    debt_ratio: Option<Decimal>,
    revenue: Option<Decimal>,
    revenue_growth_yoy: Option<Decimal>,
    earnings_growth_yoy: Option<Decimal>,
    shares_outstanding: Option<i64>,
    float_shares: Option<i64>,
    currency: Option<String>,
}

/// Fundamental + OHLCV 통합 수집 결과.
#[derive(Debug, Clone)]
pub struct FetchResult {
    /// Fundamental 데이터
    pub fundamental: FundamentalData,
    /// OHLCV 캔들 데이터 (일봉 기준)
    pub klines: Vec<Kline>,
    /// 종목명 (Yahoo Finance에서 수집)
    pub name: Option<String>,
}

impl FundamentalFetcher {
    /// Yahoo Finance에서 Fundamental + OHLCV 데이터 통합 수집.
    ///
    /// 한 번의 API 호출로 펀더멘털 지표와 OHLCV 캔들 데이터를 모두 가져옵니다.
    /// API 호출 효율성을 극대화하여 Rate Limiting을 방지합니다.
    ///
    /// # Arguments
    /// * `yahoo_symbol` - Yahoo Finance 심볼 (예: "005930.KS", "AAPL")
    /// * `canonical_ticker` - 정규화된 티커 (예: "005930", "AAPL")
    /// * `market` - 시장 코드 (예: "KR", "US")
    ///
    /// # Returns
    /// `FetchResult` - Fundamental 데이터와 OHLCV 캔들 데이터
    pub async fn fetch_with_ohlcv(
        &mut self,
        yahoo_symbol: &str,
        canonical_ticker: &str,
        market: &str,
    ) -> Result<FetchResult> {
        debug!(symbol = yahoo_symbol, "Fundamental + OHLCV 통합 수집 시작");

        // 1. 1년치 일봉 데이터 조회 (펀더멘털 + OHLCV 공용)
        let response = self
            .connector
            .get_quote_range(yahoo_symbol, "1d", "1y")
            .await
            .map_err(|e| {
                DataError::FetchError(format!("Yahoo Finance 조회 실패 ({}): {}", yahoo_symbol, e))
            })?;

        let quotes = response
            .quotes()
            .map_err(|e| DataError::ParseError(format!("Quote 파싱 오류: {}", e)))?;

        // 빈 데이터 처리
        if quotes.is_empty() {
            return Ok(FetchResult {
                fundamental: FundamentalData {
                    symbol: yahoo_symbol.to_string(),
                    fetched_at: Utc::now(),
                    ..Default::default()
                },
                klines: Vec::new(),
                name: None,
            });
        }

        // 2. OHLCV 캔들 데이터 변환
        let (_quote_currency, _market_type) = match market.to_uppercase().as_str() {
            "KR" => ("KRW", MarketType::Stock),
            "US" => ("USD", MarketType::Stock),
            "CRYPTO" => ("USDT", MarketType::Crypto),
            _ => ("USD", MarketType::Stock),
        };

        // Symbol 생성자를 통해 country 필드 자동 추론
        let klines: Vec<Kline> = quotes
            .iter()
            .filter_map(|q| {
                // Unix timestamp → DateTime 변환
                let open_time = Utc.timestamp_opt(q.timestamp, 0).single()?;
                let close_time = open_time + chrono::Duration::days(1);

                Some(Kline {
                    ticker: canonical_ticker.to_string(),
                    timeframe: Timeframe::D1,
                    open_time,
                    open: round_decimal_from_f64(q.open)?,
                    high: round_decimal_from_f64(q.high)?,
                    low: round_decimal_from_f64(q.low)?,
                    close: round_decimal_from_f64(q.close)?,
                    volume: round_decimal_from_f64(q.volume as f64).unwrap_or_default(),
                    close_time,
                    quote_volume: None,
                    num_trades: None,
                })
            })
            .collect();

        // 3. Fundamental 지표 계산 (소수점 4자리로 반올림)
        let high_52w = quotes
            .iter()
            .map(|q| q.high)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .and_then(round_decimal_from_f64);

        let low_52w = quotes
            .iter()
            .map(|q| q.low)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .and_then(round_decimal_from_f64);

        // 평균 거래량 계산
        let recent_10: Vec<u64> = quotes.iter().rev().take(10).map(|q| q.volume).collect();
        let avg_vol_10d = if recent_10.is_empty() {
            None
        } else {
            Some((recent_10.iter().sum::<u64>() / recent_10.len() as u64) as i64)
        };

        let recent_63: Vec<u64> = quotes.iter().rev().take(63).map(|q| q.volume).collect();
        let avg_vol_3m = if recent_63.is_empty() {
            None
        } else {
            Some((recent_63.iter().sum::<u64>() / recent_63.len() as u64) as i64)
        };

        // 메타데이터에서 통화 정보 추출
        let default_currency = response.metadata().ok().and_then(|m| m.currency.clone());

        // 4. Yahoo ticker info API에서 Fundamental 데이터 추가 수집
        // get_ticker_info는 crumb 토큰 인증을 자동 처리함
        // 최대 3회 재시도 (네트워크 오류, 일시적 API 실패 대응)
        let quote_data = self.fetch_fundamental_with_retry(yahoo_symbol, 3).await;

        let (fundamental, name) = if let Some(qd) = quote_data {
            let fetched_name = qd.name.clone();
            let fund = FundamentalData {
                symbol: yahoo_symbol.to_string(),
                // Quote API 데이터 - 시장 정보
                market_cap: qd.market_cap,
                shares_outstanding: qd.shares_outstanding,
                float_shares: qd.float_shares,
                // 밸류에이션 지표
                per: qd.per,
                forward_per: qd.forward_per,
                pbr: qd.pbr,
                // 주당 지표
                eps: qd.eps,
                bps: qd.bps,
                // 배당 관련
                dividend_yield: qd.dividend_yield,
                dividend_payout_ratio: qd.dividend_payout_ratio,
                ex_dividend_date: qd.ex_dividend_date,
                // 수익성 지표
                roe: qd.roe,
                roa: qd.roa,
                operating_margin: qd.operating_margin,
                net_profit_margin: qd.profit_margin,
                gross_margin: qd.gross_margin,
                // 안정성 지표
                debt_ratio: qd.debt_ratio,
                current_ratio: qd.current_ratio,
                quick_ratio: qd.quick_ratio,
                // 성장성 지표
                revenue_growth_yoy: qd.revenue_growth_yoy,
                earnings_growth_yoy: qd.earnings_growth_yoy,
                // OHLCV 기반 데이터
                week_52_high: high_52w,
                week_52_low: low_52w,
                avg_volume_10d: avg_vol_10d,
                avg_volume_3m: avg_vol_3m,
                currency: qd.currency.or(default_currency),
                fetched_at: Utc::now(),
                ..Default::default()
            };
            (fund, fetched_name)
        } else {
            // Quote API 실패 시 OHLCV 기반 데이터만 사용
            let fund = FundamentalData {
                symbol: yahoo_symbol.to_string(),
                week_52_high: high_52w,
                week_52_low: low_52w,
                avg_volume_10d: avg_vol_10d,
                avg_volume_3m: avg_vol_3m,
                currency: default_currency,
                fetched_at: Utc::now(),
                ..Default::default()
            };
            (fund, None)
        };

        info!(
            symbol = yahoo_symbol,
            klines_count = klines.len(),
            name = ?name,
            market_cap = ?fundamental.market_cap,
            per = ?fundamental.per,
            pbr = ?fundamental.pbr,
            week_52_high = ?high_52w,
            week_52_low = ?low_52w,
            "Fundamental + OHLCV 통합 수집 완료"
        );

        Ok(FetchResult {
            fundamental,
            klines,
            name,
        })
    }
}

#[cfg(test)]
mod tests {
    // 테스트 작성 예정
}
