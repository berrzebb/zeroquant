//! AnalyticsProvider 구현체.
//!
//! trader-core의 AnalyticsProvider trait을 구현하여
//! 전략에서 분석 결과를 조회할 수 있도록 합니다.
//!
//! # 설계 원칙
//!
//! - 모든 공개 API는 ticker 문자열을 받습니다 (Symbol 객체가 아님)
//! - 내부적으로 CachedHistoricalDataProvider를 통해 캔들 데이터를 조회합니다
//! - 각 calculator를 호출하여 분석 결과를 생성합니다

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, warn};

use trader_core::domain::{
    AnalyticsError, AnalyticsProvider, GlobalScoreResult, MacroEnvironment, MarketBreadth,
    MarketRegime, RouteState, ScreeningPreset, ScreeningResult, StructuralFeatures,
};
use trader_core::types::MarketType;
use trader_core::Timeframe;
use trader_data::cache::CachedHistoricalDataProvider;

use crate::{
    GlobalScorer, IndicatorEngine, MarketRegimeCalculator, RouteStateCalculator,
    StructuralFeaturesCalculator,
};

/// AnalyticsProvider 구현체.
///
/// 캔들 데이터를 기반으로 다양한 분석 결과를 계산합니다.
///
/// # 다중 타임프레임 지원 (1.4.2)
///
/// 현재는 `default_timeframe`을 사용하여 단일 타임프레임만 지원합니다.
/// 1.4.2 구현 시 `fetch_*_multi_tf()` 메서드를 추가하여 다중 타임프레임을 지원할 예정입니다.
#[allow(dead_code)]
pub struct AnalyticsProviderImpl {
    /// 캔들 데이터 제공자
    data_provider: Arc<CachedHistoricalDataProvider>,
    /// 기본 타임프레임 (1.4.2에서 다중 타임프레임 지원 예정)
    default_timeframe: Timeframe,
    /// RouteState 계산기
    route_state_calc: RouteStateCalculator,
    /// MarketRegime 계산기
    market_regime_calc: MarketRegimeCalculator,
    /// GlobalScore 계산기
    global_scorer: GlobalScorer,
}

impl AnalyticsProviderImpl {
    /// 새 AnalyticsProvider 생성.
    ///
    /// # Arguments
    ///
    /// * `data_provider` - 캔들 데이터 제공자
    pub fn new(data_provider: Arc<CachedHistoricalDataProvider>) -> Self {
        Self {
            data_provider,
            default_timeframe: Timeframe::D1,
            route_state_calc: RouteStateCalculator::default(),
            market_regime_calc: MarketRegimeCalculator::default(),
            global_scorer: GlobalScorer::default(),
        }
    }

    /// 기본 타임프레임 설정.
    ///
    /// # Arguments
    ///
    /// * `timeframe` - 사용할 타임프레임
    pub fn with_timeframe(mut self, timeframe: Timeframe) -> Self {
        self.default_timeframe = timeframe;
        self
    }

    /// 특정 종목의 캔들 데이터 조회.
    ///
    /// `default_timeframe`을 사용하여 캔들 데이터를 조회합니다.
    async fn get_candles(
        &self,
        ticker: &str,
        limit: usize,
    ) -> Result<Vec<trader_core::Kline>, AnalyticsError> {
        self.data_provider
            .get_klines(ticker, self.default_timeframe, limit)
            .await
            .map_err(|e| {
                AnalyticsError::DataFetch(format!("Failed to fetch candles for {}: {}", ticker, e))
            })
    }

    /// 다중 타임프레임 캔들 데이터 로드 (Phase 1.4.2).
    ///
    /// 지정된 타임프레임들의 캔들 데이터를 병렬로 로드합니다.
    ///
    /// # 인자
    ///
    /// * `ticker` - 종목 심볼
    /// * `config` - 다중 타임프레임 설정
    ///
    /// # 반환
    ///
    /// (타임프레임, 캔들 데이터) 튜플의 벡터
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// use trader_core::{Timeframe, domain::MultiTimeframeConfig};
    ///
    /// let config = MultiTimeframeConfig::new()
    ///     .with_timeframe(Timeframe::D1, 60)
    ///     .with_timeframe(Timeframe::H4, 120);
    ///
    /// let data = provider.fetch_multi_timeframe_klines("005930", &config).await?;
    /// ```
    pub async fn fetch_multi_timeframe_klines(
        &self,
        ticker: &str,
        config: &trader_core::domain::MultiTimeframeConfig,
    ) -> Result<Vec<(Timeframe, Vec<trader_core::Kline>)>, AnalyticsError> {
        use futures::future::join_all;

        let futures: Vec<_> = config
            .timeframes
            .iter()
            .map(|(&timeframe, &limit)| async move {
                let result = self
                    .data_provider
                    .get_klines(ticker, timeframe, limit)
                    .await
                    .map_err(|e| {
                        AnalyticsError::DataFetch(format!(
                            "Failed to fetch {:?} candles for {}: {}",
                            timeframe, ticker, e
                        ))
                    });
                (timeframe, result)
            })
            .collect();

        let results = join_all(futures).await;

        // 성공한 결과만 수집 (실패한 타임프레임은 경고 로그)
        let mut klines_data = Vec::new();
        for (timeframe, result) in results {
            match result {
                Ok(klines) => {
                    debug!(
                        ticker = ticker,
                        timeframe = ?timeframe,
                        candle_count = klines.len(),
                        "Loaded multi-timeframe klines"
                    );
                    klines_data.push((timeframe, klines));
                }
                Err(e) => {
                    warn!(
                        ticker = ticker,
                        timeframe = ?timeframe,
                        error = %e,
                        "Failed to load klines for timeframe"
                    );
                }
            }
        }

        Ok(klines_data)
    }

    /// 여러 종목의 다중 타임프레임 캔들 데이터를 한 번에 로드.
    ///
    /// # 인자
    ///
    /// * `tickers` - 종목 심볼 목록
    /// * `config` - 다중 타임프레임 설정
    ///
    /// # 반환
    ///
    /// ticker → (timeframe → klines) 매핑
    pub async fn fetch_multi_timeframe_klines_batch(
        &self,
        tickers: &[&str],
        config: &trader_core::domain::MultiTimeframeConfig,
    ) -> Result<
        std::collections::HashMap<String, Vec<(Timeframe, Vec<trader_core::Kline>)>>,
        AnalyticsError,
    > {
        use futures::future::join_all;

        let futures: Vec<_> = tickers
            .iter()
            .map(|&ticker| async move {
                let result = self.fetch_multi_timeframe_klines(ticker, config).await;
                (ticker.to_string(), result)
            })
            .collect();

        let results = join_all(futures).await;

        let mut batch_results = std::collections::HashMap::new();
        for (ticker, result) in results {
            match result {
                Ok(data) => {
                    batch_results.insert(ticker, data);
                }
                Err(e) => {
                    warn!(ticker = ticker, error = %e, "Failed to load multi-timeframe klines");
                }
            }
        }

        Ok(batch_results)
    }
}

#[async_trait]
impl AnalyticsProvider for AnalyticsProviderImpl {
    async fn fetch_global_scores(
        &self,
        market_type: MarketType,
    ) -> Result<Vec<GlobalScoreResult>, AnalyticsError> {
        // TODO: Phase 1.2에서 구현
        // 현재는 빈 결과 반환
        debug!(market_type = ?market_type, "fetch_global_scores called (not yet implemented)");
        Ok(Vec::new())
    }

    async fn fetch_route_states(
        &self,
        tickers: &[&str],
    ) -> Result<HashMap<String, RouteState>, AnalyticsError> {
        let mut results = HashMap::new();

        for ticker in tickers {
            match self.get_candles(ticker, 60).await {
                Ok(candles) => match self.route_state_calc.calculate(&candles) {
                    Ok(state) => {
                        results.insert(ticker.to_string(), state);
                    }
                    Err(e) => {
                        warn!(ticker = ticker, error = %e, "RouteState calculation failed");
                    }
                },
                Err(e) => {
                    warn!(ticker = ticker, error = %e, "Failed to fetch candles for RouteState");
                }
            }
        }

        Ok(results)
    }

    async fn fetch_screening(
        &self,
        preset: ScreeningPreset,
    ) -> Result<Vec<ScreeningResult>, AnalyticsError> {
        // TODO: Phase 1.3에서 구현
        // 현재는 빈 결과 반환
        debug!(preset_name = %preset.name, "fetch_screening called (not yet implemented)");
        Ok(Vec::new())
    }

    async fn fetch_features(
        &self,
        tickers: &[&str],
    ) -> Result<HashMap<String, StructuralFeatures>, AnalyticsError> {
        let mut results = HashMap::new();

        for ticker in tickers {
            match self.get_candles(ticker, 60).await {
                Ok(candles) => {
                    // IndicatorEngine 기반 StructuralFeatures 계산
                    let indicator_engine = IndicatorEngine::new();
                    match StructuralFeaturesCalculator::from_candles(ticker, &candles, &indicator_engine) {
                        Ok(features) => {
                            results.insert(ticker.to_string(), features);
                        }
                        Err(e) => {
                            warn!(ticker = ticker, error = %e, "StructuralFeatures calculation failed");
                        }
                    }
                }
                Err(e) => {
                    warn!(ticker = ticker, error = %e, "Failed to fetch candles for StructuralFeatures");
                }
            }
        }

        Ok(results)
    }

    async fn fetch_market_regimes(
        &self,
        tickers: &[&str],
    ) -> Result<HashMap<String, MarketRegime>, AnalyticsError> {
        let mut results = HashMap::new();

        for ticker in tickers {
            match self.get_candles(ticker, 80).await {
                Ok(candles) => match self.market_regime_calc.calculate(&candles) {
                    Ok(regime_result) => {
                        results.insert(ticker.to_string(), regime_result.regime);
                    }
                    Err(e) => {
                        warn!(ticker = ticker, error = %e, "MarketRegime calculation failed");
                    }
                },
                Err(e) => {
                    warn!(ticker = ticker, error = %e, "Failed to fetch candles for MarketRegime");
                }
            }
        }

        Ok(results)
    }

    async fn fetch_macro_environment(&self) -> Result<MacroEnvironment, AnalyticsError> {
        // TODO: Phase 1.4에서 구현
        // 현재는 기본값 반환
        debug!("fetch_macro_environment called (not yet implemented)");
        Ok(MacroEnvironment::default())
    }

    async fn fetch_market_breadth(&self) -> Result<MarketBreadth, AnalyticsError> {
        // TODO: Phase 1.4에서 구현
        // 현재는 기본값 반환
        debug!("fetch_market_breadth called (not yet implemented)");
        Ok(MarketBreadth::default())
    }
}

#[cfg(test)]
mod tests {
    // 테스트는 실제 DB 연결이 필요하므로 integration test로 이동
}
