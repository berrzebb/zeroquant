//! 섹터 상대강도 (Sector RS) 계산기.
//!
//! 섹터별 상대강도를 계산하여 주도 섹터를 식별합니다.
//! 시장 대비 초과수익으로 진짜 주도 섹터를 발굴합니다.
//!
//! # 계산 공식
//!
//! - **상대강도 (RS)**: 섹터 수익률 / 시장 수익률
//! - **종합 점수**: RS × 0.6 + 단순수익 × 0.4
//!
//! # 예시
//!
//! ```rust,ignore
//! use trader_analytics::sector_rs::{SectorRsCalculator, SectorRsInput};
//! use trader_core::Kline;
//!
//! let inputs = vec![
//!     SectorRsInput {
//!         ticker: "005930".to_string(),
//!         sector: "반도체".to_string(),
//!         klines: vec![/* kline data */],
//!     },
//!     // ... more inputs
//! ];
//!
//! let calculator = SectorRsCalculator::new();
//! let results = calculator.calculate(&inputs, 20);
//!
//! for sector in &results {
//!     println!("{}: RS={:.2}, Score={:.2}, Rank={}",
//!         sector.sector, sector.relative_strength, sector.composite_score, sector.rank);
//! }
//! ```

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use trader_core::Kline;

/// 섹터 RS 계산 입력 데이터.
#[derive(Debug, Clone)]
pub struct SectorRsInput {
    /// 종목 티커
    pub ticker: String,
    /// 섹터명
    pub sector: String,
    /// 캔들 데이터 (최신이 마지막)
    pub klines: Vec<Kline>,
}

/// 섹터 상대강도 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorRsResult {
    /// 섹터명
    pub sector: String,
    /// 섹터 내 종목 수
    pub symbol_count: usize,
    /// 섹터 평균 수익률 (%, lookback 기간)
    pub avg_return_pct: Decimal,
    /// 섹터 5일 평균 수익률 (%)
    pub avg_return_5d_pct: Decimal,
    /// 시장 평균 수익률 (%)
    pub market_return: Decimal,
    /// 상대강도 (RS = 섹터수익률 / 시장수익률)
    pub relative_strength: Decimal,
    /// 종합 점수 (RS × 0.6 + 단순수익 × 0.4)
    pub composite_score: Decimal,
    /// 순위 (1이 가장 높음)
    pub rank: u32,
    /// 섹터 총 시가총액 (옵션)
    pub total_market_cap: Option<Decimal>,
}

/// 종목별 섹터 RS 정보.
///
/// 각 종목이 속한 섹터의 RS 정보를 제공합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerSectorRs {
    /// 종목 티커
    pub ticker: String,
    /// 섹터명
    pub sector: String,
    /// 섹터 상대강도
    pub sector_rs: Decimal,
    /// 섹터 순위
    pub sector_rank: u32,
}

/// 섹터 RS 계산기.
///
/// 메모리 기반으로 섹터 상대강도를 계산합니다.
/// SQL 의존 없이 전략에서 직접 사용 가능합니다.
#[derive(Debug, Default)]
pub struct SectorRsCalculator {
    /// RS 가중치 (기본: 0.6)
    rs_weight: Decimal,
    /// 단순수익 가중치 (기본: 0.4)
    return_weight: Decimal,
}

impl SectorRsCalculator {
    /// 새로운 계산기 생성 (기본 가중치 사용).
    pub fn new() -> Self {
        Self {
            rs_weight: dec!(0.6),
            return_weight: dec!(0.4),
        }
    }

    /// 커스텀 가중치로 계산기 생성.
    ///
    /// # 인자
    ///
    /// * `rs_weight` - RS 가중치 (0.0 ~ 1.0)
    /// * `return_weight` - 단순수익 가중치 (0.0 ~ 1.0)
    pub fn with_weights(rs_weight: Decimal, return_weight: Decimal) -> Self {
        Self {
            rs_weight,
            return_weight,
        }
    }

    /// 섹터 상대강도 계산.
    ///
    /// # 인자
    ///
    /// * `inputs` - 종목별 섹터 및 캔들 데이터
    /// * `lookback_days` - 수익률 계산 기간 (기본: 20일)
    ///
    /// # 반환
    ///
    /// 섹터별 RS 결과 (순위순 정렬)
    pub fn calculate(&self, inputs: &[SectorRsInput], lookback_days: usize) -> Vec<SectorRsResult> {
        if inputs.is_empty() {
            return Vec::new();
        }

        // 1. 종목별 수익률 계산
        let ticker_returns: Vec<(String, String, Decimal, Decimal)> = inputs
            .iter()
            .filter_map(|input| {
                let return_pct = self.calculate_return(&input.klines, lookback_days)?;
                let return_5d = self
                    .calculate_return(&input.klines, 5)
                    .unwrap_or(Decimal::ZERO);
                Some((
                    input.ticker.clone(),
                    input.sector.clone(),
                    return_pct,
                    return_5d,
                ))
            })
            .collect();

        if ticker_returns.is_empty() {
            return Vec::new();
        }

        // 2. 시장 전체 평균 수익률 계산
        let market_return: Decimal = ticker_returns
            .iter()
            .map(|(_, _, ret, _)| *ret)
            .sum::<Decimal>()
            / Decimal::from(ticker_returns.len());

        // 3. 섹터별 그룹화 및 평균 계산
        let mut sector_data: HashMap<String, (Vec<Decimal>, Vec<Decimal>)> = HashMap::new();
        for (_, sector, ret, ret_5d) in &ticker_returns {
            sector_data
                .entry(sector.clone())
                .or_insert_with(|| (Vec::new(), Vec::new()))
                .0
                .push(*ret);
            sector_data.get_mut(sector).unwrap().1.push(*ret_5d);
        }

        // 4. 섹터별 RS 계산
        let mut results: Vec<SectorRsResult> = sector_data
            .into_iter()
            .map(|(sector, (returns, returns_5d))| {
                let symbol_count = returns.len();
                let avg_return_pct: Decimal =
                    returns.iter().sum::<Decimal>() / Decimal::from(symbol_count);
                let avg_return_5d_pct: Decimal =
                    returns_5d.iter().sum::<Decimal>() / Decimal::from(symbol_count);

                // 상대강도 계산 (시장 대비)
                let relative_strength = if market_return.abs() > dec!(0.0001) {
                    avg_return_pct / market_return
                } else {
                    // 시장 수익률이 0에 가까우면 섹터 수익률 기준
                    if avg_return_pct > Decimal::ZERO {
                        dec!(1.5)
                    } else if avg_return_pct < Decimal::ZERO {
                        dec!(0.5)
                    } else {
                        dec!(1.0)
                    }
                };

                // 종합 점수 계산
                // RS 정규화: 1.0 기준으로 스케일링 (1.2 → 120점, 0.8 → 80점)
                let normalized_rs = relative_strength * dec!(100);
                // 단순 수익률: 이미 % 단위이므로 10배 스케일링 (5% → 50점)
                let normalized_return = avg_return_pct * dec!(10);

                let composite_score =
                    normalized_rs * self.rs_weight + normalized_return * self.return_weight;

                SectorRsResult {
                    sector,
                    symbol_count,
                    avg_return_pct,
                    avg_return_5d_pct,
                    market_return,
                    relative_strength,
                    composite_score,
                    rank: 0, // 나중에 설정
                    total_market_cap: None,
                }
            })
            .collect();

        // 5. 순위 매기기 (composite_score 내림차순)
        results.sort_by(|a, b| b.composite_score.partial_cmp(&a.composite_score).unwrap());
        for (idx, result) in results.iter_mut().enumerate() {
            result.rank = (idx + 1) as u32;
        }

        results
    }

    /// 종목별 섹터 RS 매핑 생성.
    ///
    /// 각 종목이 속한 섹터의 RS 정보를 매핑합니다.
    /// 전략에서 특정 종목의 섹터 강도를 조회할 때 사용합니다.
    ///
    /// # 반환
    ///
    /// ticker → TickerSectorRs 매핑
    pub fn map_to_tickers(
        &self,
        inputs: &[SectorRsInput],
        sector_results: &[SectorRsResult],
    ) -> HashMap<String, TickerSectorRs> {
        let sector_map: HashMap<&str, (&Decimal, u32)> = sector_results
            .iter()
            .map(|r| (r.sector.as_str(), (&r.composite_score, r.rank)))
            .collect();

        inputs
            .iter()
            .filter_map(|input| {
                let (score, rank) = sector_map.get(input.sector.as_str())?;
                Some((
                    input.ticker.clone(),
                    TickerSectorRs {
                        ticker: input.ticker.clone(),
                        sector: input.sector.clone(),
                        sector_rs: **score,
                        sector_rank: *rank,
                    },
                ))
            })
            .collect()
    }

    /// 수익률 계산.
    ///
    /// 캔들 데이터에서 지정된 기간의 수익률을 계산합니다.
    fn calculate_return(&self, klines: &[Kline], days: usize) -> Option<Decimal> {
        if klines.len() < days + 1 {
            return None;
        }

        let end_idx = klines.len() - 1;
        let start_idx = end_idx.saturating_sub(days);

        let start_price = klines[start_idx].close;
        let end_price = klines[end_idx].close;

        if start_price.is_zero() {
            return None;
        }

        // 수익률 (%)
        let return_pct = ((end_price - start_price) / start_price) * dec!(100);
        Some(return_pct)
    }
}

/// 섹터 RS를 ScreeningResult에 병합하는 헬퍼 함수.
///
/// 스크리닝 결과에 섹터 RS 정보를 추가합니다.
pub fn enrich_screening_with_sector_rs(
    ticker_sector_rs: &HashMap<String, TickerSectorRs>,
    screening_results: &mut [trader_core::domain::ScreeningResult],
) {
    for result in screening_results {
        if let Some(sector_info) = ticker_sector_rs.get(&result.ticker) {
            result.sector_rs = Some(sector_info.sector_rs);
            result.sector_rank = Some(sector_info.sector_rank as i32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use trader_core::Timeframe;

    fn create_test_klines(prices: &[Decimal]) -> Vec<Kline> {
        prices
            .iter()
            .map(|&close| Kline {
                ticker: "TEST".to_string(),
                timeframe: Timeframe::D1,
                open_time: Utc::now(),
                open: close,
                high: close + dec!(1),
                low: close - dec!(1),
                close,
                volume: dec!(1000000),
                close_time: Utc::now(),
                quote_volume: Some(dec!(0)),
                num_trades: Some(100),
            })
            .collect()
    }

    #[test]
    fn test_sector_rs_calculation() {
        let calculator = SectorRsCalculator::new();

        // 상승 섹터 (반도체)
        let semiconductor_prices: Vec<Decimal> = (0..25)
            .map(|i| dec!(100) + Decimal::from(i) * dec!(0.5))
            .collect();

        // 하락 섹터 (건설)
        let construction_prices: Vec<Decimal> = (0..25)
            .map(|i| dec!(100) - Decimal::from(i) * dec!(0.3))
            .collect();

        // 횡보 섹터 (금융)
        let finance_prices: Vec<Decimal> = (0..25).map(|_| dec!(100)).collect();

        let inputs = vec![
            SectorRsInput {
                ticker: "005930".to_string(),
                sector: "반도체".to_string(),
                klines: create_test_klines(&semiconductor_prices),
            },
            SectorRsInput {
                ticker: "000660".to_string(),
                sector: "반도체".to_string(),
                klines: create_test_klines(&semiconductor_prices),
            },
            SectorRsInput {
                ticker: "000720".to_string(),
                sector: "건설".to_string(),
                klines: create_test_klines(&construction_prices),
            },
            SectorRsInput {
                ticker: "105560".to_string(),
                sector: "금융".to_string(),
                klines: create_test_klines(&finance_prices),
            },
        ];

        let results = calculator.calculate(&inputs, 20);

        assert_eq!(results.len(), 3);

        // 반도체가 1위
        assert_eq!(results[0].sector, "반도체");
        assert_eq!(results[0].rank, 1);
        assert!(results[0].avg_return_pct > Decimal::ZERO);

        // 건설이 최하위
        let construction = results.iter().find(|r| r.sector == "건설").unwrap();
        assert!(construction.avg_return_pct < Decimal::ZERO);
    }

    #[test]
    fn test_map_to_tickers() {
        let calculator = SectorRsCalculator::new();

        let prices: Vec<Decimal> = (0..25).map(|i| dec!(100) + Decimal::from(i)).collect();

        let inputs = vec![
            SectorRsInput {
                ticker: "005930".to_string(),
                sector: "반도체".to_string(),
                klines: create_test_klines(&prices),
            },
            SectorRsInput {
                ticker: "000660".to_string(),
                sector: "반도체".to_string(),
                klines: create_test_klines(&prices),
            },
        ];

        let sector_results = calculator.calculate(&inputs, 20);
        let ticker_map = calculator.map_to_tickers(&inputs, &sector_results);

        assert_eq!(ticker_map.len(), 2);
        assert!(ticker_map.contains_key("005930"));
        assert!(ticker_map.contains_key("000660"));

        let samsung = ticker_map.get("005930").unwrap();
        assert_eq!(samsung.sector, "반도체");
        assert_eq!(samsung.sector_rank, 1);
    }

    #[test]
    fn test_empty_input() {
        let calculator = SectorRsCalculator::new();
        let results = calculator.calculate(&[], 20);
        assert!(results.is_empty());
    }

    #[test]
    fn test_insufficient_klines() {
        let calculator = SectorRsCalculator::new();

        // 10개 캔들만 (20일 계산 불가)
        let prices: Vec<Decimal> = (0..10).map(|i| dec!(100) + Decimal::from(i)).collect();

        let inputs = vec![SectorRsInput {
            ticker: "005930".to_string(),
            sector: "반도체".to_string(),
            klines: create_test_klines(&prices),
        }];

        let results = calculator.calculate(&inputs, 20);
        assert!(results.is_empty());
    }
}
