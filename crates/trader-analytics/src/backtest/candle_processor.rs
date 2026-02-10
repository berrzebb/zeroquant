//! 캔들 처리 공통 모듈
//!
//! BacktestEngine과 SimulationEngine 간 공통 캔들 처리 로직을 제공합니다.
//!
//! # 설계 원칙
//!
//! 두 엔진의 유일한 차이는 캔들이 한꺼번에(backtest) vs 스트리밍(simulation)으로 제공되는 것입니다.
//! CandleProcessor는 단일 캔들 시점의 처리 로직을 공통화하여:
//! - StrategyContext 업데이트 (지표, 스크리닝)
//! - 시그널 생성 (멀티 심볼/멀티 타임프레임)
//! - 포지션 동기화
//!
//! 를 한 곳에서 관리합니다. 이를 통해 StrategyContext 관련 수정 시 한 곳만 변경하면 됩니다.
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! let mut processor = CandleProcessor::new();
//!
//! // BacktestEngine에서:
//! for (idx, kline) in klines.iter().enumerate() {
//!     processor.update_context(idx, kline, &klines[..=idx], &context, ticker, screening_calc).await?;
//!     let signals = processor.generate_signals(&mut strategy, kline, &context, ticker, exchange).await?;
//!     // engine-specific: process_signal, record_trade, equity tracking
//!     processor.sync_positions(&mut strategy, executor.positions(), kline, exchange, ticker).await?;
//! }
//!
//! // SimulationEngine에서:
//! let ctx = ProcessCandleContext {
//!     idx, kline, historical_klines: historical, context: &context,
//!     ticker, exchange_name: exchange, screening_calculator: screening_calc,
//! };
//! let signals = processor.process_candle(ctx, &mut strategy).await?;
//! // engine-specific: process signals, update equity
//! processor.sync_positions(&mut strategy, executor.positions(), kline, exchange, ticker).await?;
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;
use tracing::debug;
use trader_core::{
    unrealized_pnl, Kline, MarketData, Position, RouteState, ScreeningCalculator,
    Side, Signal, SignalType, StrategyContext, Timeframe,
};
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::{
    IndicatorEngine,
    StructuralFeaturesCalculator,
    RouteStateCalculator,
    GlobalScorer,
    GlobalScorerParams,
    TimeframeAligner,
};
use trader_core::MarketType;
use trader_execution::ProcessorPosition;

use super::BacktestError;

/// 최소 지표 계산에 필요한 캔들 수 (StructuralFeaturesCalculator는 40개 필요)
pub const MIN_CANDLES_FOR_INDICATORS: usize = 40;

/// 캔들 처리 공통 프로세서
///
/// BacktestEngine과 SimulationEngine에서 공유하는 캔들 처리 로직을 제공합니다.
/// Strategy, SimulatedExecutor, StrategyContext를 소유하지 않고 참조만 받아 처리합니다.
pub struct CandleProcessor {
    /// 지표 엔진
    indicator_engine: IndicatorEngine,
    /// 현재 심볼별 가격
    current_prices: HashMap<String, Decimal>,
    /// 현재 처리 중인 시간
    current_time: DateTime<Utc>,
}

/// 시그널 생성 결과
///
/// Entry/Exit로 파티셔닝된 시그널을 포함합니다.
pub struct PartitionedSignals {
    /// 진입 시그널 (Entry, AddToPosition, Scale)
    pub entry_signals: Vec<Signal>,
    /// 청산 시그널 (Exit, ReducePosition)
    pub exit_signals: Vec<Signal>,
}

impl PartitionedSignals {
    /// 전체 시그널 수
    pub fn total_count(&self) -> usize {
        self.entry_signals.len() + self.exit_signals.len()
    }

    /// 모든 시그널을 순서대로 반환 (Entry 먼저, Exit 나중)
    pub fn into_ordered(self) -> Vec<Signal> {
        let mut result = self.entry_signals;
        result.extend(self.exit_signals);
        result
    }
}

/// process_candle 메서드의 컨텍스트 파라미터
///
/// 너무 많은 인자를 피하기 위해 관련 파라미터를 그룹화합니다.
pub struct ProcessCandleContext<'a> {
    /// 현재 캔들 인덱스
    pub idx: usize,
    /// 현재 캔들
    pub kline: &'a Kline,
    /// 과거 캔들 (idx까지 포함)
    pub historical_klines: &'a [Kline],
    /// 전략 컨텍스트
    pub context: &'a Arc<RwLock<StrategyContext>>,
    /// 심볼 티커
    pub ticker: &'a str,
    /// 거래소 이름
    pub exchange_name: &'a str,
    /// 스크리닝 계산기 (옵션)
    pub screening_calculator: Option<&'a dyn ScreeningCalculator>,
}

impl CandleProcessor {
    /// 새 CandleProcessor 생성
    pub fn new() -> Self {
        Self {
            indicator_engine: IndicatorEngine::new(),
            current_prices: HashMap::new(),
            current_time: Utc::now(),
        }
    }

    /// 현재 심볼별 가격 맵 참조
    pub fn current_prices(&self) -> &HashMap<String, Decimal> {
        &self.current_prices
    }

    /// 현재 처리 중인 시간
    pub fn current_time(&self) -> DateTime<Utc> {
        self.current_time
    }

    /// StrategyContext 업데이트
    ///
    /// 캔들 시점마다 호출되어 다음을 수행합니다:
    /// 1. 멀티 심볼 klines 업데이트 (현재 시점까지 필터링)
    /// 2. StructuralFeatures 계산
    /// 3. RouteState 설정 (백테스트에서는 Armed 고정)
    /// 4. GlobalScore 계산 (백테스트에서는 80점 고정)
    /// 5. 스크리닝 파이프라인 (동적 유니버스 전략용)
    ///
    /// # 인자
    ///
    /// * `idx` - 현재 캔들 인덱스
    /// * `kline` - 현재 캔들
    /// * `historical_klines` - 현재 시점까지의 과거 캔들 (klines[..=idx])
    /// * `context` - StrategyContext
    /// * `ticker` - 메인 티커
    /// * `screening_calculator` - 스크리닝 계산기 (동적 유니버스 전략용)
    pub async fn update_context(
        &mut self,
        idx: usize,
        kline: &Kline,
        historical_klines: &[Kline],
        context: &Arc<RwLock<StrategyContext>>,
        ticker: &str,
        screening_calculator: Option<&dyn ScreeningCalculator>,
    ) -> Result<(), BacktestError> {
        // 현재 시간/가격 업데이트
        self.current_time = kline.close_time;
        self.current_prices.insert(kline.ticker.to_string(), kline.close);

        // === 멀티 심볼 klines 업데이트 ===
        self.update_multi_symbol_klines(context, ticker).await;

        // === 주 심볼 지표 계산 (StructuralFeatures, RouteState, GlobalScore) ===
        if idx >= MIN_CANDLES_FOR_INDICATORS {
            self.update_primary_indicators(idx, historical_klines, context, ticker).await;
        }

        // === 스크리닝 파이프라인 ===
        if let Some(screening_calc) = screening_calculator {
            self.update_screening(idx, kline, context, screening_calc).await;
        }

        Ok(())
    }

    /// 시그널 생성 (Entry/Exit 파티셔닝 포함)
    ///
    /// 주 심볼과 다른 심볼들에 대해 MarketData를 전달하고,
    /// 멀티 타임프레임 전략이면 on_multi_timeframe_data를 호출합니다.
    /// 결과는 Entry/Exit로 파티셔닝되어 반환됩니다.
    ///
    /// # 인자
    ///
    /// * `strategy` - 전략 인스턴스
    /// * `kline` - 현재 캔들
    /// * `context` - StrategyContext
    /// * `ticker` - 메인 티커
    /// * `exchange_name` - 거래소 이름
    pub async fn generate_signals<S>(
        &self,
        strategy: &mut S,
        kline: &Kline,
        context: &Arc<RwLock<StrategyContext>>,
        ticker: &str,
        exchange_name: &str,
    ) -> Result<PartitionedSignals, BacktestError>
    where
        S: trader_strategy::Strategy + ?Sized,
    {
        let mut all_signals = Vec::new();

        // 1. 주 심볼의 시장 데이터 전달
        let market_data = MarketData::from_kline(exchange_name, kline.clone());

        // 멀티 타임프레임 전략 지원 체크
        let is_multi_tf = strategy.multi_timeframe_config().is_some();

        let signals = if is_multi_tf {
            // 멀티 타임프레임 전략: Secondary 데이터 수집 및 정렬
            let secondary_data: HashMap<Timeframe, Vec<Kline>> = {
                let ctx_read = context.read().await;
                let mut result = HashMap::new();

                // 주 심볼의 다른 타임프레임 데이터 수집
                if let Some(tf_map) = ctx_read.klines_by_timeframe.get(ticker) {
                    for (&tf, tf_klines) in tf_map.iter() {
                        result.insert(tf, tf_klines.clone());
                    }
                }
                result
            };

            // TimeframeAligner로 현재 시점 기준 유효한 데이터만 필터링
            let aligned_secondary = TimeframeAligner::align_multi_timeframe(
                &secondary_data,
                kline.close_time,
            );

            strategy
                .on_multi_timeframe_data(&market_data, &aligned_secondary)
                .await
                .map_err(|e| BacktestError::StrategyError(e.to_string()))?
        } else {
            // 단일 타임프레임 전략: 기존 로직
            strategy
                .on_market_data(&market_data)
                .await
                .map_err(|e| BacktestError::StrategyError(e.to_string()))?
        };
        all_signals.extend(signals);

        // 2. 다른 심볼들의 시장 데이터도 전달
        let other_klines: Vec<Kline> = {
            let ctx_read = context.read().await;
            ctx_read.klines_by_timeframe
                .iter()
                .filter(|(symbol, _)| *symbol != ticker)
                .filter_map(|(_, tf_map)| tf_map.get(&Timeframe::D1))
                .filter_map(|symbol_klines| {
                    symbol_klines
                        .iter()
                        .find(|k| k.close_time == self.current_time)
                        .cloned()
                })
                .collect()
        };

        for other_kline in other_klines {
            let symbol_market_data = MarketData::from_kline(exchange_name, other_kline);
            let symbol_signals = strategy
                .on_market_data(&symbol_market_data)
                .await
                .map_err(|e| BacktestError::StrategyError(e.to_string()))?;
            all_signals.extend(symbol_signals);
        }

        // Entry/Exit 파티셔닝
        let (entry_signals, exit_signals): (Vec<_>, Vec<_>) = all_signals.into_iter().partition(|s| {
            matches!(s.signal_type, SignalType::Entry | SignalType::AddToPosition | SignalType::Scale)
        });

        Ok(PartitionedSignals {
            entry_signals,
            exit_signals,
        })
    }

    /// 전략에 포지션 동기화
    ///
    /// 시그널 처리 후 전략에 현재 포지션 상태를 알립니다.
    /// 이를 통해 전략의 has_position() 체크가 정상 작동합니다.
    ///
    /// # 인자
    ///
    /// * `strategy` - 전략 인스턴스
    /// * `positions` - SimulatedExecutor의 현재 포지션 맵
    /// * `kline` - 현재 캔들 (현재가 참조)
    /// * `exchange_name` - 거래소 이름
    /// * `ticker` - 메인 티커 (빈 포지션 알림용)
    pub async fn sync_positions<S>(
        &self,
        strategy: &mut S,
        positions: &HashMap<String, ProcessorPosition>,
        kline: &Kline,
        exchange_name: &str,
        ticker: &str,
    ) -> Result<(), BacktestError>
    where
        S: trader_strategy::Strategy + ?Sized,
    {
        for (_, proc_pos) in positions.iter() {
            let position = Position {
                id: Uuid::new_v4(),
                exchange: exchange_name.to_string(),
                ticker: proc_pos.symbol.clone(),
                side: proc_pos.side,
                quantity: proc_pos.quantity,
                entry_price: proc_pos.entry_price,
                current_price: kline.close,
                unrealized_pnl: unrealized_pnl(
                    proc_pos.entry_price,
                    kline.close,
                    proc_pos.quantity,
                    proc_pos.side,
                ),
                realized_pnl: Decimal::ZERO,
                strategy_id: None,
                opened_at: proc_pos.entry_time,
                updated_at: Utc::now(),
                closed_at: None,
                metadata: serde_json::Value::Null,
            };
            strategy.on_position_update(&position).await
                .map_err(|e| BacktestError::StrategyError(e.to_string()))?;
        }

        // 포지션이 청산된 경우 빈 포지션 상태 알림 (주 티커만)
        if positions.is_empty() {
            let empty_position = Position {
                id: Uuid::new_v4(),
                exchange: exchange_name.to_string(),
                ticker: ticker.to_string(),
                side: Side::Buy,
                quantity: Decimal::ZERO,
                entry_price: Decimal::ZERO,
                current_price: kline.close,
                unrealized_pnl: Decimal::ZERO,
                realized_pnl: Decimal::ZERO,
                strategy_id: None,
                opened_at: Utc::now(),
                updated_at: Utc::now(),
                closed_at: None,
                metadata: serde_json::Value::Null,
            };
            strategy.on_position_update(&empty_position).await
                .map_err(|e| BacktestError::StrategyError(e.to_string()))?;
        }

        Ok(())
    }

    /// 편의 메서드: 캔들 하나를 처리하고 파티셔닝된 시그널을 반환
    ///
    /// update_context → generate_signals를 순차 호출합니다.
    /// 호출자는 반환된 시그널을 자체적으로 처리한 후 sync_positions를 호출해야 합니다.
    ///
    /// SimulationEngine처럼 한 번에 호출이 필요한 경우 사용합니다.
    pub async fn process_candle<S>(
        &mut self,
        ctx: ProcessCandleContext<'_>,
        strategy: &mut S,
    ) -> Result<PartitionedSignals, BacktestError>
    where
        S: trader_strategy::Strategy + ?Sized,
    {
        // 1. StrategyContext 업데이트
        self.update_context(
            ctx.idx,
            ctx.kline,
            ctx.historical_klines,
            ctx.context,
            ctx.ticker,
            ctx.screening_calculator,
        ).await?;

        // 2. 시그널 생성
        let signals = self.generate_signals(
            strategy,
            ctx.kline,
            ctx.context,
            ctx.ticker,
            ctx.exchange_name,
        ).await?;

        Ok(signals)
    }

    // =========================================================================
    // 내부 헬퍼 메서드
    // =========================================================================

    /// 멀티 심볼 klines 업데이트 (현재 시점까지 필터링)
    async fn update_multi_symbol_klines(
        &mut self,
        context: &Arc<RwLock<StrategyContext>>,
        ticker: &str,
    ) {
        // StrategyContext에서 등록된 심볼 목록 가져오기 (주 심볼 제외)
        let symbols: Vec<String> = {
            let ctx_read = context.read().await;
            ctx_read.klines_by_timeframe.keys()
                .filter(|s| *s != ticker)
                .cloned()
                .collect()
        };

        for symbol in symbols {
            // 해당 심볼의 현재 시점까지 klines 필터링
            let symbol_klines: Vec<Kline> = {
                let ctx_read = context.read().await;
                ctx_read.klines_by_timeframe
                    .get(&symbol)
                    .and_then(|tf_map| tf_map.get(&Timeframe::D1))
                    .map(|klines| {
                        klines.iter()
                            .filter(|k| k.close_time <= self.current_time)
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default()
            };

            // 현재 가격 업데이트
            if let Some(current_kline) = symbol_klines.last() {
                self.current_prices.insert(symbol.clone(), current_kline.close);
            }

            // 충분한 데이터가 있을 때만 지표 계산 및 klines 업데이트
            if symbol_klines.len() >= MIN_CANDLES_FOR_INDICATORS {
                let mut ctx_write = context.write().await;

                // 현재 시점까지의 klines로 업데이트
                ctx_write.update_klines(&symbol, Timeframe::D1, symbol_klines.clone());

                // RouteState - 백테스트에서는 Armed로 설정
                ctx_write.route_states.insert(symbol.clone(), RouteState::Armed);

                // GlobalScore 계산 (실제 캔들 데이터 기반)
                let scorer = GlobalScorer::new();
                let params = GlobalScorerParams {
                    symbol: Some(symbol.clone()),
                    market_type: Some(MarketType::Stock),
                    ..Default::default()
                };
                if let Ok(mut score) = scorer.calculate(&symbol_klines, params) {
                    // 백테스트에서는 필터 우회를 위해 80점으로 설정
                    score.overall_score = rust_decimal_macros::dec!(80);
                    ctx_write.global_scores.insert(symbol.clone(), score);
                }
            }
        }
    }

    /// 주 심볼의 지표 업데이트 (StructuralFeatures, RouteState, GlobalScore)
    async fn update_primary_indicators(
        &self,
        idx: usize,
        historical_klines: &[Kline],
        context: &Arc<RwLock<StrategyContext>>,
        ticker: &str,
    ) {
        // 1. StructuralFeatures 계산
        let features_result = StructuralFeaturesCalculator::from_candles(
            ticker,
            historical_klines,
            &self.indicator_engine,
        );
        let features_opt = features_result.ok();

        // 디버그: 첫 계산 시 로그 출력
        if idx == MIN_CANDLES_FOR_INDICATORS {
            match StructuralFeaturesCalculator::from_candles(ticker, historical_klines, &self.indicator_engine) {
                Ok(f) => debug!(
                    ticker = %ticker,
                    bb_lower = %f.bb_lower,
                    bb_middle = %f.bb_middle,
                    bb_upper = %f.bb_upper,
                    bb_width = %f.bb_width,
                    "StructuralFeatures 계산 성공"
                ),
                Err(e) => debug!(ticker = %ticker, error = %e, "StructuralFeatures 계산 실패"),
            }
        }

        // 2. RouteState 계산
        let _route_state_opt = {
            let calculator = RouteStateCalculator::new();
            calculator.calculate(historical_klines).ok()
        };

        // 3. GlobalScore 계산
        let global_score_opt = {
            let scorer = GlobalScorer::new();
            let params = GlobalScorerParams {
                symbol: Some(ticker.to_string()),
                market_type: Some(MarketType::Stock),
                ..Default::default()
            };
            scorer.calculate(historical_klines, params).ok()
        };

        // StrategyContext에 업데이트
        {
            let mut ctx_write = context.write().await;

            // StructuralFeatures 업데이트
            if let Some(features) = features_opt {
                ctx_write.structural_features.insert(ticker.to_string(), features);
            }

            // RouteState - 백테스트에서는 Armed로 강제 설정
            // 전략 로직 자체를 검증하기 위해 RouteState 필터 우회
            ctx_write.route_states.insert(ticker.to_string(), RouteState::Armed);

            // GlobalScore - 백테스트에서는 높은 점수로 강제 설정
            // 전략 로직 자체를 검증하기 위해 GlobalScore 필터 우회
            if let Some(mut score) = global_score_opt {
                score.overall_score = rust_decimal_macros::dec!(80);
                ctx_write.global_scores.insert(ticker.to_string(), score);
            }

            // klines 업데이트 (현재 시점까지만)
            ctx_write.update_klines(ticker, Timeframe::D1, historical_klines.to_vec());
        }
    }

    /// 스크리닝 파이프라인 업데이트 (동적 유니버스 전략 지원)
    async fn update_screening(
        &self,
        idx: usize,
        kline: &Kline,
        context: &Arc<RwLock<StrategyContext>>,
        screening_calc: &dyn ScreeningCalculator,
    ) {
        // 스크리닝 업데이트 조건 체크
        let last_screening_update = {
            let ctx_read = context.read().await;
            if ctx_read.screening_results.contains_key(&screening_calc.config().preset_name) {
                Some(ctx_read.last_analytics_sync)
            } else {
                None
            }
        };

        let should_update = screening_calc.should_update(
            idx,
            kline.close_time,
            last_screening_update,
        );

        if should_update {
            // 모든 심볼의 현재 시점까지 klines 수집
            let all_klines: HashMap<String, Vec<Kline>> = {
                let ctx_read = context.read().await;
                ctx_read
                    .klines_by_timeframe
                    .iter()
                    .filter_map(|(symbol, tf_map)| {
                        tf_map.get(&Timeframe::D1).map(|klines| {
                            let filtered: Vec<_> = klines
                                .iter()
                                .filter(|k| k.close_time <= self.current_time)
                                .cloned()
                                .collect();
                            (symbol.clone(), filtered)
                        })
                    })
                    .collect()
            };

            debug!(
                symbols_count = all_klines.len(),
                preset = %screening_calc.config().preset_name,
                idx = idx,
                "스크리닝 계산 시작: {} 심볼",
                all_klines.len()
            );

            // 스크리닝 결과 계산
            let screening_results = screening_calc.calculate_from_klines(
                &all_klines,
                self.current_time,
            );

            let results_count = screening_results.len();

            // StrategyContext 업데이트
            {
                let mut ctx_write = context.write().await;
                ctx_write.update_screening(
                    screening_calc.config().preset_name.clone(),
                    screening_results,
                );
            }

            debug!(
                preset = %screening_calc.config().preset_name,
                time = %self.current_time,
                results_count = results_count,
                "스크리닝 결과 업데이트 완료: {} 개 결과",
                results_count
            );
        }
    }
}

impl Default for CandleProcessor {
    fn default() -> Self {
        Self::new()
    }
}
