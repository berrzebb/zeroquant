//! 백테스트용 스크리닝 제공자
//!
//! 백테스트 환경에서 캔들 데이터만으로 스크리닝 결과를 생성합니다.
//! 실거래의 AnalyticsProvider 역할을 대신합니다.

use std::collections::HashMap;

use chrono::{DateTime, Datelike, Utc};
use rust_decimal_macros::dec;
use trader_core::domain::{Kline, RouteState, ScreeningResult};
// trader-core에서 정의된 trait과 타입 사용
use trader_core::{ScreeningCalculator, ScreeningCalculatorConfig, ScreeningUpdateFrequency};

use crate::global_scorer::{GlobalScorer, GlobalScorerParams};
use crate::route_state_calculator::RouteStateCalculator;

/// 백테스트용 스크리닝 결과를 계산하는 최소 캔들 수
pub const MIN_CANDLES_FOR_SCREENING: usize = 50;

// ================================================================================================
// 하위 호환성을 위한 타입 별칭
// ================================================================================================

/// 백테스트용 스크리닝 설정 (하위 호환성용 별칭)
///
/// 새 코드에서는 `ScreeningCalculatorConfig`를 직접 사용하세요.
pub type BacktestScreeningConfig = ScreeningCalculatorConfig;

// ================================================================================================
// BacktestScreeningProvider
// ================================================================================================

/// 백테스트용 스크리닝 제공자
///
/// 캔들 데이터만으로 GlobalScore와 RouteState를 계산하여
/// ScreeningResult를 생성합니다.
///
/// # ScreeningCalculator trait 구현
///
/// 이 struct는 `trader-core`에 정의된 `ScreeningCalculator` trait을 구현합니다.
/// BacktestEngine에서 의존성 주입을 통해 사용됩니다.
///
/// ```ignore
/// let provider = BacktestScreeningProvider::with_config(config);
/// let results = provider.calculate_from_klines(&klines, timestamp);
/// ```
pub struct BacktestScreeningProvider {
    global_scorer: GlobalScorer,
    route_calculator: RouteStateCalculator,
    config: ScreeningCalculatorConfig,
}

impl Default for BacktestScreeningProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl BacktestScreeningProvider {
    /// 기본 설정으로 스크리닝 제공자 생성
    pub fn new() -> Self {
        Self {
            global_scorer: GlobalScorer::new(),
            route_calculator: RouteStateCalculator::new(),
            config: ScreeningCalculatorConfig::default(),
        }
    }

    /// 지정된 설정으로 스크리닝 제공자 생성
    pub fn with_config(config: ScreeningCalculatorConfig) -> Self {
        Self {
            global_scorer: GlobalScorer::new(),
            route_calculator: RouteStateCalculator::new(),
            config,
        }
    }

    /// 캔들 데이터 기반 스크리닝 결과 생성 (기존 API 하위 호환용)
    ///
    /// 새 코드에서는 `ScreeningCalculator::calculate_from_klines()`를 사용하세요.
    pub fn calculate_from_klines_with_config(
        &self,
        all_klines: &HashMap<String, Vec<Kline>>,
        current_time: DateTime<Utc>,
        config: &ScreeningCalculatorConfig,
    ) -> Vec<ScreeningResult> {
        self.calculate_screening_internal(all_klines, current_time, config)
    }

    /// 내부 스크리닝 계산 로직
    fn calculate_screening_internal(
        &self,
        all_klines: &HashMap<String, Vec<Kline>>,
        current_time: DateTime<Utc>,
        config: &ScreeningCalculatorConfig,
    ) -> Vec<ScreeningResult> {
        let mut results = Vec::new();

        for (ticker, klines) in all_klines {
            // 현재 시점까지의 데이터만 필터링 (Look-Ahead Bias 방지)
            let historical: Vec<_> = klines
                .iter()
                .filter(|k| k.close_time <= current_time)
                .cloned()
                .collect();

            // 최소 캔들 수 체크
            if historical.len() < MIN_CANDLES_FOR_SCREENING {
                continue;
            }

            // 스크리닝 결과 계산
            if let Some(result) = self.calculate_single(ticker, &historical, current_time, config) {
                results.push(result);
            }
        }

        // overall_score 기준 내림차순 정렬
        results.sort_by(|a, b| b.overall_score.cmp(&a.overall_score));

        results
    }

    /// 단일 종목 스크리닝 결과 계산
    fn calculate_single(
        &self,
        ticker: &str,
        klines: &[Kline],
        current_time: DateTime<Utc>,
        config: &ScreeningCalculatorConfig,
    ) -> Option<ScreeningResult> {
        // 1. GlobalScore 계산
        let params = GlobalScorerParams {
            symbol: Some(ticker.to_string()),
            ..Default::default()
        };

        let score_result = match self.global_scorer.calculate(klines, params) {
            Ok(result) => result,
            Err(_) => return None,
        };

        let overall_score = score_result.overall_score;

        // 2. RouteState 계산
        let route_state = self
            .route_calculator
            .calculate(klines)
            .unwrap_or(RouteState::Neutral);

        // 3. criteria_results 구성 (기술적 지표 기반)
        let mut criteria_results = HashMap::new();
        criteria_results.insert("global_score".to_string(), overall_score >= config.min_score);
        criteria_results.insert("route_state_favorable".to_string(),
            matches!(route_state, RouteState::Attack | RouteState::Armed));

        // 컴포넌트 점수를 criteria로 추가
        for (key, value) in &score_result.component_scores {
            criteria_results.insert(format!("score_{}", key), *value >= dec!(50));
        }

        // 4. ScreeningResult 생성
        Some(ScreeningResult {
            ticker: ticker.to_string(),
            preset_name: config.preset_name.clone(),
            passed: overall_score >= config.min_score,
            overall_score,
            route_state,
            criteria_results,
            timestamp: current_time,
            sector_rs: None,        // 백테스트에서는 섹터 RS 미지원
            sector_rank: None,
            trigger_score: None,    // 향후 확장 가능
            trigger_label: None,
        })
    }

    /// 스크리닝 업데이트 필요 여부 판단 (static 메서드)
    ///
    /// 하위 호환성을 위해 유지됩니다.
    /// 새 코드에서는 `ScreeningCalculator::should_update()`를 사용하세요.
    pub fn should_update_static(
        idx: usize,
        current_time: DateTime<Utc>,
        frequency: ScreeningUpdateFrequency,
        last_update: Option<DateTime<Utc>>,
    ) -> bool {
        // 첫 실행이면 무조건 업데이트
        if last_update.is_none() {
            return idx >= MIN_CANDLES_FOR_SCREENING;
        }

        match frequency {
            ScreeningUpdateFrequency::EveryCandle => true,
            ScreeningUpdateFrequency::Daily => true, // 일봉 기준 매 캔들
            ScreeningUpdateFrequency::Weekly => {
                // 월요일인지 체크
                current_time.weekday() == chrono::Weekday::Mon
            }
            ScreeningUpdateFrequency::Monthly => {
                // 매월 1일인지 체크
                current_time.day() == 1
            }
            ScreeningUpdateFrequency::Custom(n) => {
                idx % n == 0
            }
        }
    }

    /// 상위 N개 종목만 필터링
    pub fn top_n(results: Vec<ScreeningResult>, n: usize) -> Vec<ScreeningResult> {
        results.into_iter().take(n).collect()
    }

    /// passed=true인 종목만 필터링
    pub fn passed_only(results: Vec<ScreeningResult>) -> Vec<ScreeningResult> {
        results.into_iter().filter(|r| r.passed).collect()
    }

    /// 특정 RouteState인 종목만 필터링
    pub fn by_route_state(results: Vec<ScreeningResult>, state: RouteState) -> Vec<ScreeningResult> {
        results.into_iter().filter(|r| r.route_state == state).collect()
    }
}

// ================================================================================================
// ScreeningCalculator trait 구현
// ================================================================================================

impl ScreeningCalculator for BacktestScreeningProvider {
    fn calculate_from_klines(
        &self,
        all_klines: &HashMap<String, Vec<Kline>>,
        current_time: DateTime<Utc>,
    ) -> Vec<ScreeningResult> {
        self.calculate_screening_internal(all_klines, current_time, &self.config)
    }

    fn config(&self) -> &ScreeningCalculatorConfig {
        &self.config
    }

    fn should_update(
        &self,
        idx: usize,
        current_time: DateTime<Utc>,
        last_update: Option<DateTime<Utc>>,
    ) -> bool {
        Self::should_update_static(idx, current_time, self.config.update_frequency, last_update)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal::Decimal;
    use trader_core::Timeframe;

    fn create_test_klines(count: usize, base_price: f64) -> Vec<Kline> {
        (0..count)
            .map(|i| {
                let price = Decimal::from_f64_retain(base_price + (i as f64 * 0.1)).unwrap();
                Kline {
                    ticker: "TEST".to_string(),
                    timeframe: Timeframe::D1,
                    open_time: Utc.with_ymd_and_hms(2024, 1, 1 + (i as u32 / 24), 0, 0, 0).unwrap(),
                    close_time: Utc.with_ymd_and_hms(2024, 1, 1 + (i as u32 / 24), 23, 59, 59).unwrap(),
                    open: price,
                    high: price + dec!(1),
                    low: price - dec!(1),
                    close: price,
                    volume: Decimal::from(1000),
                    quote_volume: Some(price * Decimal::from(1000)),
                    num_trades: Some(100),
                }
            })
            .collect()
    }

    #[test]
    fn test_screening_provider_creation() {
        let provider = BacktestScreeningProvider::new();
        assert!(provider.global_scorer.calculate(&[], GlobalScorerParams::default()).is_err());
    }

    #[test]
    fn test_screening_config_default() {
        let config = ScreeningCalculatorConfig::default();
        assert_eq!(config.preset_name, "backtest");
        assert_eq!(config.update_frequency, ScreeningUpdateFrequency::Monthly);
        assert_eq!(config.min_score, Decimal::from(60));
    }

    #[test]
    fn test_should_update_first_run() {
        // 첫 실행, 충분한 캔들
        assert!(BacktestScreeningProvider::should_update_static(
            60,
            Utc::now(),
            ScreeningUpdateFrequency::Monthly,
            None
        ));

        // 첫 실행, 캔들 부족
        assert!(!BacktestScreeningProvider::should_update_static(
            10,
            Utc::now(),
            ScreeningUpdateFrequency::Monthly,
            None
        ));
    }

    #[test]
    fn test_should_update_monthly() {
        let first_of_month = Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap();
        let mid_month = Utc.with_ymd_and_hms(2024, 2, 15, 0, 0, 0).unwrap();
        let last_update = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

        // 월초 - 업데이트 필요
        assert!(BacktestScreeningProvider::should_update_static(
            100,
            first_of_month,
            ScreeningUpdateFrequency::Monthly,
            Some(last_update)
        ));

        // 월중 - 업데이트 불필요
        assert!(!BacktestScreeningProvider::should_update_static(
            100,
            mid_month,
            ScreeningUpdateFrequency::Monthly,
            Some(last_update)
        ));
    }

    #[test]
    fn test_calculate_from_klines_insufficient_data() {
        let config = ScreeningCalculatorConfig::default();
        let provider = BacktestScreeningProvider::with_config(config);

        // 캔들 부족한 데이터
        let mut all_klines = HashMap::new();
        all_klines.insert("TEST".to_string(), create_test_klines(10, 100.0));

        // ScreeningCalculator trait 메서드 사용
        let results = provider.calculate_from_klines(
            &all_klines,
            Utc::now(),
        );

        // 캔들 부족으로 결과 없음
        assert!(results.is_empty());
    }

    #[test]
    fn test_top_n_filter() {
        let results = vec![
            ScreeningResult {
                ticker: "A".to_string(),
                preset_name: "test".to_string(),
                passed: true,
                overall_score: dec!(90),
                route_state: RouteState::Attack,
                criteria_results: HashMap::new(),
                timestamp: Utc::now(),
                sector_rs: None,
                sector_rank: None,
                trigger_score: None,
                trigger_label: None,
            },
            ScreeningResult {
                ticker: "B".to_string(),
                preset_name: "test".to_string(),
                passed: true,
                overall_score: dec!(80),
                route_state: RouteState::Armed,
                criteria_results: HashMap::new(),
                timestamp: Utc::now(),
                sector_rs: None,
                sector_rank: None,
                trigger_score: None,
                trigger_label: None,
            },
            ScreeningResult {
                ticker: "C".to_string(),
                preset_name: "test".to_string(),
                passed: false,
                overall_score: dec!(50),
                route_state: RouteState::Wait,
                criteria_results: HashMap::new(),
                timestamp: Utc::now(),
                sector_rs: None,
                sector_rank: None,
                trigger_score: None,
                trigger_label: None,
            },
        ];

        let top_2 = BacktestScreeningProvider::top_n(results.clone(), 2);
        assert_eq!(top_2.len(), 2);
        assert_eq!(top_2[0].ticker, "A");
        assert_eq!(top_2[1].ticker, "B");

        let passed = BacktestScreeningProvider::passed_only(results.clone());
        assert_eq!(passed.len(), 2);

        let attack = BacktestScreeningProvider::by_route_state(results, RouteState::Attack);
        assert_eq!(attack.len(), 1);
        assert_eq!(attack[0].ticker, "A");
    }

    #[test]
    fn test_screening_calculator_trait_impl() {
        // trait 구현 검증
        let config = ScreeningCalculatorConfig::monthly("test_preset", Decimal::from(70));
        let provider = BacktestScreeningProvider::with_config(config);

        // trait 메서드 호출
        assert_eq!(provider.config().preset_name, "test_preset");
        assert_eq!(provider.config().min_score, Decimal::from(70));
        assert_eq!(provider.config().update_frequency, ScreeningUpdateFrequency::Monthly);
    }
}
