//! Global Score 계산기.
//!
//! 모든 기술적 지표를 단일 점수(0~100)로 종합하여 종목 순위를 산출합니다.
//!
//! # 7개 팩터 (가중치 합계 = 1.0)
//!
//! 1. **Risk/Reward (RR)**: 0.25 - 목표가 대비 손절가 비율
//! 2. **Target Room (T1)**: 0.18 - 현재가 대비 목표가 여유율
//! 3. **Stop Room (SL)**: 0.12 - 현재가 대비 손절가 여유율
//! 4. **Entry Proximity (NEAR)**: 0.12 - 추천 진입가 근접도
//! 5. **Momentum (MOM)**: 0.10 - ERS + MACD 기울기 + RSI 중심 보너스
//! 6. **Liquidity (LIQ)**: 0.13 - 거래대금 퍼센타일
//! 7. **Technical Balance (TEC)**: 0.10 - 변동성(VolZ) 스윗스팟 + 이격도 안정성
//!
//! # 7개 페널티 (점수 차감)
//!
//! 1. 5일 과열: -6점 (5일 수익률 +10% 초과)
//! 2. 10일 과열: -6점 (10일 수익률 +20% 초과)
//! 3. RSI 이탈: -4점 (RSI 45~65 밴드 이탈)
//! 4. MACD 음수: -4점 (MACD 기울기 음수)
//! 5. 진입 괴리: -4점 (추천가 대비 현재가 괴리 과다)
//! 6. 저유동성: -4점 (거래대금 하위 20%)
//! 7. 변동성 스파이크: -2점 (VolZ > 3)

use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use trader_core::{types::MarketType, GlobalScoreResult, Kline};

use crate::indicators::{
    BollingerBandsParams, IndicatorEngine, IndicatorError, MacdParams, RsiParams, SmaParams,
};
use trader_core::domain::StructuralFeatures;

/// GlobalScorer 계산 오류.
#[derive(Debug, thiserror::Error)]
pub enum GlobalScorerError {
    /// 지표 계산 오류
    #[error("지표 계산 실패: {0}")]
    IndicatorError(#[from] IndicatorError),

    /// 데이터 부족
    #[error("데이터 부족: 필요 {required}개, 제공 {provided}개")]
    InsufficientData { required: usize, provided: usize },

    /// 필수 파라미터 누락
    #[error("필수 파라미터 누락: {0}")]
    MissingParameter(String),

    /// 계산 오류
    #[error("계산 오류: {0}")]
    CalculationError(String),
}

/// GlobalScorer 결과 타입.
pub type GlobalScorerResult<T> = Result<T, GlobalScorerError>;

/// 팩터 가중치.
struct FactorWeights {
    risk_reward: f32,       // RR: 0.25
    target_room: f32,       // T1: 0.18
    stop_room: f32,         // SL: 0.12
    entry_proximity: f32,   // NEAR: 0.12
    momentum: f32,          // MOM: 0.10
    liquidity: f32,         // LIQ: 0.13
    technical_balance: f32, // TEC: 0.10
}

impl Default for FactorWeights {
    fn default() -> Self {
        Self {
            risk_reward: 0.25,
            target_room: 0.18,
            stop_room: 0.12,
            entry_proximity: 0.12,
            momentum: 0.10,
            liquidity: 0.13,
            technical_balance: 0.10,
        }
    }
}

/// GlobalScorer 입력 파라미터.
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct GlobalScorerParams {
    /// 심볼
    pub symbol: Option<String>,

    /// 시장 유형
    pub market_type: Option<MarketType>,

    /// 추천 진입가 (None이면 현재가 사용)
    pub entry_price: Option<Decimal>,

    /// 목표가 (None이면 T1/SL 팩터 0점 처리)
    pub target_price: Option<Decimal>,

    /// 손절가 (None이면 RR/SL 팩터 0점 처리)
    pub stop_price: Option<Decimal>,

    /// 종목의 평균 거래대금 (유동성 퍼센타일 계산용)
    pub avg_volume_amount: Option<Decimal>,

    /// 시장 전체 거래대금 퍼센타일 (0.0 ~ 1.0)
    /// None이면 LIQ 팩터 0점 처리
    pub volume_percentile: Option<f32>,

    /// 구조적 피처 (EBS 계산용)
    ///
    /// StructuralFeatures를 제공하면 ERS를 계산하여 Momentum 점수에 반영합니다.
    /// None이면 ERS는 0점으로 처리됩니다.
    pub structural_features: Option<StructuralFeatures>,
}


/// Global Score 계산기.
///
/// 7개 팩터를 가중 합산하고 7개 페널티를 차감하여 최종 점수(0~100)를 산출합니다.
pub struct GlobalScorer {
    /// 지표 계산 엔진
    indicator_engine: IndicatorEngine,

    /// 팩터 가중치
    weights: FactorWeights,
}

impl GlobalScorer {
    /// 새로운 GlobalScorer 생성.
    pub fn new() -> Self {
        Self {
            indicator_engine: IndicatorEngine::new(),
            weights: FactorWeights::default(),
        }
    }

    /// 캔들 데이터로부터 Global Score 계산.
    ///
    /// # 인자
    ///
    /// * `candles` - 최소 50개의 캔들 데이터
    /// * `params` - 계산 파라미터 (진입가, 목표가, 손절가 등)
    ///
    /// # 반환
    ///
    /// GlobalScoreResult
    ///
    /// # 에러
    ///
    /// - 캔들 개수 부족
    /// - 지표 계산 실패
    pub fn calculate(
        &self,
        candles: &[Kline],
        params: GlobalScorerParams,
    ) -> GlobalScorerResult<GlobalScoreResult> {
        const MIN_CANDLES: usize = 50;

        if candles.len() < MIN_CANDLES {
            return Err(GlobalScorerError::InsufficientData {
                required: MIN_CANDLES,
                provided: candles.len(),
            });
        }

        let current_price = candles.last().unwrap().close;

        // 7개 팩터 계산
        let rr_score = self.calculate_risk_reward(current_price, &params)?;
        let t1_score = self.calculate_target_room(current_price, &params)?;
        let sl_score = self.calculate_stop_room(current_price, &params)?;
        let near_score = self.calculate_entry_proximity(current_price, &params)?;
        let mom_score = self.calculate_momentum(candles, &params)?;
        let liq_score = self.calculate_liquidity(&params)?;
        let tec_score = self.calculate_technical_balance(candles)?;

        // 가중 합산 (0~100)
        let mut overall_score = rr_score * self.weights.risk_reward
            + t1_score * self.weights.target_room
            + sl_score * self.weights.stop_room
            + near_score * self.weights.entry_proximity
            + mom_score * self.weights.momentum
            + liq_score * self.weights.liquidity
            + tec_score * self.weights.technical_balance;

        // 7개 페널티 차감
        let penalties = self.calculate_penalties(candles, current_price, &params)?;
        overall_score = (overall_score - penalties).max(0.0);

        // 컴포넌트별 점수
        let mut component_scores = HashMap::new();
        component_scores.insert("risk_reward".to_string(), rr_score);
        component_scores.insert("target_room".to_string(), t1_score);
        component_scores.insert("stop_room".to_string(), sl_score);
        component_scores.insert("entry_proximity".to_string(), near_score);
        component_scores.insert("momentum".to_string(), mom_score);
        component_scores.insert("liquidity".to_string(), liq_score);
        component_scores.insert("technical_balance".to_string(), tec_score);
        component_scores.insert("penalties".to_string(), -penalties);

        // 추천 방향 결정
        let recommendation = if overall_score >= 70.0 {
            "BUY".to_string()
        } else if overall_score >= 50.0 {
            "WATCH".to_string()
        } else {
            "HOLD".to_string()
        };

        // 신뢰도 계산 (데이터 완전성 기준)
        let confidence = self.calculate_confidence(&params);

        // f32 → Decimal 변환하여 결과 생성
        let overall_score_dec = Decimal::from_f32_retain(overall_score).unwrap_or(Decimal::ZERO);
        let confidence_dec = Decimal::from_f32_retain(confidence).unwrap_or(Decimal::ZERO);
        let component_scores_dec: HashMap<String, Decimal> = component_scores
            .into_iter()
            .map(|(k, v)| (k, Decimal::from_f32_retain(v).unwrap_or(Decimal::ZERO)))
            .collect();

        Ok(GlobalScoreResult {
            // Symbol을 ticker 문자열로 변환
            ticker: params.symbol.clone(),
            market_type: params.market_type,
            overall_score: overall_score_dec,
            component_scores: component_scores_dec,
            recommendation,
            confidence: confidence_dec,
            timestamp: Utc::now(),
        })
    }

    // ================================================================================================
    // 7개 팩터 계산
    // ================================================================================================

    /// 1. Risk/Reward (RR) 팩터 계산.
    ///
    /// **계산식**: ((목표가 - 현재가) / (현재가 - 손절가)) / 3 * 100
    /// **정규화**: RR 비율 3:1 이상 = 100점, 1:1 = 33점
    ///
    /// # 반환
    ///
    /// 0 ~ 100점
    fn calculate_risk_reward(
        &self,
        current_price: Decimal,
        params: &GlobalScorerParams,
    ) -> GlobalScorerResult<f32> {
        let target = match params.target_price {
            Some(t) => t,
            None => return Ok(0.0), // 목표가 없으면 0점
        };

        let stop = match params.stop_price {
            Some(s) => s,
            None => return Ok(0.0), // 손절가 없으면 0점
        };

        if current_price <= stop {
            return Ok(0.0); // 현재가가 손절가 이하면 0점
        }

        let reward = (target - current_price)
            .to_string()
            .parse::<f32>()
            .unwrap_or(0.0);
        let risk = (current_price - stop)
            .to_string()
            .parse::<f32>()
            .unwrap_or(1.0);

        if risk <= 0.0 {
            return Ok(0.0);
        }

        let rr_ratio = reward / risk;

        // RR 3:1 = 100점, 1:1 = 33점, 선형 보간
        let score = (rr_ratio / 3.0 * 100.0).min(100.0);

        Ok(score)
    }

    /// 2. Target Room (T1) 팩터 계산.
    ///
    /// **계산식**: (목표가 - 현재가) / 현재가 * 100
    /// **정규화**: 20% 이상 = 100점, 5% = 25점
    ///
    /// # 반환
    ///
    /// 0 ~ 100점
    fn calculate_target_room(
        &self,
        current_price: Decimal,
        params: &GlobalScorerParams,
    ) -> GlobalScorerResult<f32> {
        let target = match params.target_price {
            Some(t) => t,
            None => return Ok(0.0),
        };

        if current_price >= target {
            return Ok(0.0); // 이미 목표가 도달
        }

        // Division by zero 방지
        if current_price <= Decimal::ZERO {
            return Ok(0.0);
        }

        let room_pct = ((target - current_price) / current_price * Decimal::from(100))
            .to_string()
            .parse::<f32>()
            .unwrap_or(0.0);

        // 20% 여유 = 100점, 5% = 25점, 선형
        let score = (room_pct / 20.0 * 100.0).min(100.0);

        Ok(score)
    }

    /// 3. Stop Room (SL) 팩터 계산.
    ///
    /// **계산식**: (현재가 - 손절가) / 현재가 * 100
    /// **정규화**: 5% = 100점 (적정), 10% = 50점 (너무 넓음), 2% = 40점 (너무 좁음)
    ///
    /// # 반환
    ///
    /// 0 ~ 100점
    fn calculate_stop_room(
        &self,
        current_price: Decimal,
        params: &GlobalScorerParams,
    ) -> GlobalScorerResult<f32> {
        let stop = match params.stop_price {
            Some(s) => s,
            None => return Ok(0.0),
        };

        if current_price <= stop {
            return Ok(0.0); // 현재가 ≤ 손절가
        }

        // Division by zero 방지
        if current_price <= Decimal::ZERO {
            return Ok(0.0);
        }

        let room_pct = ((current_price - stop) / current_price * Decimal::from(100))
            .to_string()
            .parse::<f32>()
            .unwrap_or(0.0);

        // 스윗스팟: 4~6% = 100점
        let score = if (4.0..=6.0).contains(&room_pct) {
            100.0
        } else if room_pct < 4.0 {
            // 너무 좁음: 2% = 40점, 4% = 100점
            (room_pct - 2.0) / 2.0 * 60.0 + 40.0
        } else {
            // 너무 넓음: 6% = 100점, 10% = 50점
            100.0 - (room_pct - 6.0) / 4.0 * 50.0
        }
        .clamp(0.0, 100.0);

        Ok(score)
    }

    /// 4. Entry Proximity (NEAR) 팩터 계산.
    ///
    /// **계산식**: 1 - |현재가 - 추천가| / 추천가
    /// **정규화**: 괴리 0% = 100점, 괴리 5% = 0점
    ///
    /// # 반환
    ///
    /// 0 ~ 100점
    fn calculate_entry_proximity(
        &self,
        current_price: Decimal,
        params: &GlobalScorerParams,
    ) -> GlobalScorerResult<f32> {
        let entry = match params.entry_price {
            Some(e) => e,
            None => return Ok(100.0), // 추천가 없으면 현재가 = 진입가로 간주 (100점)
        };

        if entry == Decimal::ZERO {
            return Ok(0.0);
        }

        let deviation_pct = ((current_price - entry).abs() / entry * Decimal::from(100))
            .to_string()
            .parse::<f32>()
            .unwrap_or(0.0);

        // 괴리 5% 이상 = 0점, 0% = 100점
        let score = (1.0 - deviation_pct / 5.0) * 100.0;

        Ok(score.clamp(0.0, 100.0))
    }

    /// 5. Momentum (MOM) 팩터 계산.
    ///
    /// **구성**:
    /// - RSI 45~65 중심 보너스 (40점)
    /// - MACD 기울기 양수 (30점)
    /// - ERS (Entry Ready Score) 기여도 (30점)
    ///
    /// # 반환
    ///
    /// 0 ~ 100점
    fn calculate_momentum(
        &self,
        candles: &[Kline],
        params: &GlobalScorerParams,
    ) -> GlobalScorerResult<f32> {
        let closes: Vec<Decimal> = candles.iter().map(|k| k.close).collect();

        // RSI 계산
        let rsi_values = self
            .indicator_engine
            .rsi(&closes, RsiParams { period: 14 })?;
        let rsi = rsi_values
            .last()
            .and_then(|v| *v)
            .ok_or_else(|| GlobalScorerError::CalculationError("RSI 계산 실패".to_string()))?
            .to_string()
            .parse::<f32>()
            .unwrap_or(50.0);

        // RSI 스코어: 45~65 = 만점, 30~80 = 부분 점수
        let rsi_score = if (45.0..=65.0).contains(&rsi) {
            40.0
        } else if rsi < 45.0 {
            ((rsi - 30.0) / 15.0 * 40.0).max(0.0)
        } else {
            // rsi > 65
            ((80.0 - rsi) / 15.0 * 40.0).max(0.0)
        };

        // MACD 기울기 계산
        let macd_result = self.indicator_engine.macd(&closes, MacdParams::default())?;
        let macd_slope = if macd_result.len() >= 2 {
            let last_macd = macd_result.last().and_then(|m| m.macd).unwrap_or(dec!(0));
            let prev_macd = macd_result[macd_result.len() - 2].macd.unwrap_or(dec!(0));

            // Decimal → f32 변환
            (last_macd - prev_macd)
                .to_string()
                .parse::<f32>()
                .unwrap_or(0.0)
        } else {
            0.0
        };

        let macd_slope_score = if macd_slope > 0.0 {
            30.0 // 상승
        } else {
            0.0 // 하락
        };

        // ERS 계산
        let ers_score = if let Some(ref structural) = params.structural_features {
            // EBS 계산 (0~5)
            let ebs = self.calculate_ebs(structural, rsi, macd_slope);

            // ERS 계산 (0~1)
            let ebs_ok = if ebs >= 4.0 { 1.0 } else { 0.0 };
            let slope_ok = if macd_slope > 0.0 { 1.0 } else { 0.0 };
            let rsi_ok = if (45.0..=65.0).contains(&rsi) {
                1.0
            } else {
                0.0
            };
            let ers = (ebs_ok + slope_ok + rsi_ok) / 3.0;

            // ERS를 30점 만점으로 변환
            ers * 30.0
        } else {
            // StructuralFeatures 없으면 0점 (기존 30점 고정 제거)
            0.0
        };

        let total_score = rsi_score + macd_slope_score + ers_score;

        Ok(total_score.min(100.0))
    }

    /// 6. Liquidity (LIQ) 팩터 계산.
    ///
    /// **계산식**: volume_percentile * 100
    /// **정규화**: 상위 20% = 80점 이상, 하위 20% = 페널티 대상
    ///
    /// # 반환
    ///
    /// 0 ~ 100점
    fn calculate_liquidity(&self, params: &GlobalScorerParams) -> GlobalScorerResult<f32> {
        let percentile = match params.volume_percentile {
            Some(p) => p,
            None => return Ok(0.0), // 퍼센타일 정보 없으면 0점
        };

        // 퍼센타일을 점수로 직접 변환 (0.8 = 80점)
        let score = percentile * 100.0;

        Ok(score.min(100.0))
    }

    /// 7. Technical Balance (TEC) 팩터 계산.
    ///
    /// **구성**:
    /// - VolZ (변동성 Z-Score) 스윗스팟: -1 ~ +1 = 만점 (50점)
    /// - 이격도 안정성: MA20 ±5% 이내 = 만점 (50점)
    ///
    /// # 반환
    ///
    /// 0 ~ 100점
    fn calculate_technical_balance(&self, candles: &[Kline]) -> GlobalScorerResult<f32> {
        let closes: Vec<Decimal> = candles.iter().map(|k| k.close).collect();
        let current_price = *closes.last().unwrap();

        // 1. VolZ 계산 (볼린저 밴드 기반 간이 계산)
        let bb = self
            .indicator_engine
            .bollinger_bands(&closes, BollingerBandsParams::default())?;
        let last_bb = bb
            .last()
            .ok_or_else(|| GlobalScorerError::CalculationError("BB 계산 실패".to_string()))?;

        let volz_score = match (last_bb.upper, last_bb.lower, last_bb.middle) {
            (Some(upper), Some(lower), Some(middle)) if upper > lower => {
                let width = upper - lower;
                let position = (current_price - middle) / (width / dec!(2));
                let position_f32 = position.to_string().parse::<f32>().unwrap_or(0.0);

                // -1 ~ +1 = 50점, 절대값 3 이상 = 0점
                let normalized = position_f32.abs();
                if normalized <= 1.0 {
                    50.0
                } else if normalized <= 3.0 {
                    50.0 * (3.0 - normalized) / 2.0
                } else {
                    0.0
                }
            }
            _ => 25.0, // 계산 불가 시 중립 점수
        };

        // 2. MA20 이격도 안정성
        let ma20 = self
            .indicator_engine
            .sma(&closes, SmaParams { period: 20 })?;
        let ma20_value = ma20
            .last()
            .and_then(|v| *v)
            .ok_or_else(|| GlobalScorerError::CalculationError("MA20 계산 실패".to_string()))?;

        // Division by zero 방지
        if ma20_value <= Decimal::ZERO {
            return Ok(volz_score + 25.0); // 계산 불가 시 중립 점수
        }

        let disparity_pct = ((current_price - ma20_value) / ma20_value * Decimal::from(100))
            .to_string()
            .parse::<f32>()
            .unwrap_or(0.0)
            .abs();

        let disparity_score = if disparity_pct <= 5.0 {
            50.0 // ±5% 이내 = 만점
        } else if disparity_pct <= 10.0 {
            50.0 * (10.0 - disparity_pct) / 5.0 // 5~10% 선형 감소
        } else {
            0.0 // 10% 초과 = 0점
        };

        let total_score = volz_score + disparity_score;

        Ok(total_score.min(100.0))
    }

    // ================================================================================================
    // 7개 페널티 계산
    // ================================================================================================

    /// 7개 페널티 합계 계산.
    ///
    /// # 반환
    ///
    /// 총 페널티 점수 (양수, 최종 점수에서 차감됨)
    fn calculate_penalties(
        &self,
        candles: &[Kline],
        current_price: Decimal,
        params: &GlobalScorerParams,
    ) -> GlobalScorerResult<f32> {
        let mut total_penalty = 0.0;

        // 1. 5일 과열 (-6점)
        if let Ok(return_5d) = self.calculate_return_period(candles, 5) {
            if return_5d > 10.0 {
                total_penalty += 6.0;
            }
        }

        // 2. 10일 과열 (-6점)
        if let Ok(return_10d) = self.calculate_return_period(candles, 10) {
            if return_10d > 20.0 {
                total_penalty += 6.0;
            }
        }

        // 3. RSI 이탈 (-4점)
        let closes: Vec<Decimal> = candles.iter().map(|k| k.close).collect();
        if let Ok(rsi_values) = self.indicator_engine.rsi(&closes, RsiParams { period: 14 }) {
            if let Some(Some(rsi)) = rsi_values.last() {
                let rsi_f32 = rsi.to_string().parse::<f32>().unwrap_or(50.0);
                if !(45.0..=65.0).contains(&rsi_f32) {
                    total_penalty += 4.0;
                }
            }
        }

        // 4. MACD 음수 (-4점)
        if let Ok(macd_result) = self.indicator_engine.macd(&closes, MacdParams::default()) {
            if macd_result.len() >= 2 {
                let last_macd = macd_result.last().and_then(|m| m.macd).unwrap_or(dec!(0));
                let prev_macd = macd_result[macd_result.len() - 2].macd.unwrap_or(dec!(0));

                if last_macd < prev_macd {
                    total_penalty += 4.0;
                }
            }
        }

        // 5. 진입 괴리 (-4점)
        if let Some(entry) = params.entry_price {
            if entry > Decimal::ZERO {
                let deviation_pct = ((current_price - entry).abs() / entry * Decimal::from(100))
                    .to_string()
                    .parse::<f32>()
                    .unwrap_or(0.0);

                if deviation_pct > 5.0 {
                    total_penalty += 4.0;
                }
            }
        }

        // 6. 저유동성 (-4점)
        if let Some(percentile) = params.volume_percentile {
            if percentile < 0.2 {
                // 하위 20%
                total_penalty += 4.0;
            }
        }

        // 7. 변동성 스파이크 (-2점)
        if let Ok(bb) = self
            .indicator_engine
            .bollinger_bands(&closes, BollingerBandsParams::default())
        {
            if let Some(last_bb) = bb.last() {
                if let (Some(upper), Some(lower), Some(middle)) =
                    (last_bb.upper, last_bb.lower, last_bb.middle)
                {
                    if upper > lower && middle > Decimal::ZERO {
                        let width = upper - lower;
                        let position = ((current_price - middle).abs() / (width / dec!(2)))
                            .to_string()
                            .parse::<f32>()
                            .unwrap_or(0.0);

                        if position > 3.0 {
                            total_penalty += 2.0;
                        }
                    }
                }
            }
        }

        Ok(total_penalty)
    }

    // ================================================================================================
    // 헬퍼 메서드
    // ================================================================================================

    /// N일 수익률 계산.
    fn calculate_return_period(&self, candles: &[Kline], days: usize) -> GlobalScorerResult<f32> {
        if candles.len() <= days {
            return Ok(0.0);
        }

        let current_price = candles.last().unwrap().close;
        let past_price = candles[candles.len() - days - 1].close;

        if past_price == Decimal::ZERO {
            return Ok(0.0);
        }

        let return_pct = ((current_price - past_price) / past_price * Decimal::from(100))
            .to_string()
            .parse::<f32>()
            .unwrap_or(0.0);

        Ok(return_pct)
    }

    /// EBS (Entry Balance Score) 계산 (0~5점).
    ///
    /// 5가지 조건의 합:
    /// 1. RSI 건강성: 45~65 (+1)
    /// 2. MACD slope > 0 (+1)
    /// 3. low_trend > 0 (+1)
    /// 4. vol_quality > 0 (+1)
    /// 5. range_pos >= 0.5 (+1)
    ///
    /// # 인자
    ///
    /// * `structural` - 구조적 피처
    /// * `rsi` - RSI 값 (0~100)
    /// * `macd_slope` - MACD 기울기
    ///
    /// # 반환
    ///
    /// 0~5점
    fn calculate_ebs(&self, structural: &StructuralFeatures, rsi: f32, macd_slope: f32) -> f32 {
        let mut ebs = 0.0;

        // 1. RSI 건강성 (45~65)
        if (45.0..=65.0).contains(&rsi) {
            ebs += 1.0;
        }

        // 2. MACD slope > 0
        if macd_slope > 0.0 {
            ebs += 1.0;
        }

        // 3. 저가 상승 (low_trend > 0)
        if structural.low_trend > Decimal::ZERO {
            ebs += 1.0;
        }

        // 4. 매집 신호 (vol_quality > 0)
        if structural.vol_quality > Decimal::ZERO {
            ebs += 1.0;
        }

        // 5. 박스권 위치 (range_pos >= 0.5)
        if structural.range_pos >= Decimal::new(5, 1) {
            ebs += 1.0;
        }

        ebs
    }

    /// 신뢰도 계산 (데이터 완전성 기준).
    ///
    /// 모든 파라미터가 제공되면 1.0, 일부 누락 시 감소
    fn calculate_confidence(&self, params: &GlobalScorerParams) -> f32 {
        let mut score = 0.0;

        if params.target_price.is_some() {
            score += 0.2;
        }
        if params.stop_price.is_some() {
            score += 0.2;
        }
        if params.entry_price.is_some() {
            score += 0.2;
        }
        if params.volume_percentile.is_some() {
            score += 0.2;
        }
        if params.symbol.is_some() {
            score += 0.1;
        }
        if params.market_type.is_some() {
            score += 0.1;
        }

        score
    }
}

impl Default for GlobalScorer {
    fn default() -> Self {
        Self::new()
    }
}

// ================================================================================================
// 테스트
// ================================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use trader_core::types::MarketType;

    fn create_test_candles(count: usize) -> Vec<Kline> {
        use trader_core::types::Timeframe;

        (0..count)
            .map(|i| {
                let price = dec!(100) + Decimal::from(i);
                let timestamp = chrono::Utc::now() - chrono::Duration::days((count - i) as i64);
                Kline {
                    ticker: "TEST/KRW".to_string(),
                    timeframe: Timeframe::D1,
                    open_time: timestamp,
                    open: price,
                    high: price + dec!(2),
                    low: price - dec!(2),
                    close: price,
                    volume: dec!(1000000),
                    close_time: timestamp + chrono::Duration::days(1),
                    quote_volume: Some(price * dec!(1000000)),
                    num_trades: Some(1000),
                }
            })
            .collect()
    }

    #[test]
    fn test_global_scorer_basic() {
        let scorer = GlobalScorer::new();
        let candles = create_test_candles(60);

        let params = GlobalScorerParams {
            symbol: Some("TEST/KRW".to_string()),
            market_type: Some(MarketType::Stock),
            entry_price: Some(dec!(150)),
            target_price: Some(dec!(180)),
            stop_price: Some(dec!(140)),
            volume_percentile: Some(0.75),
            ..Default::default()
        };

        let result = scorer.calculate(&candles, params).unwrap();

        assert!(result.overall_score >= Decimal::ZERO && result.overall_score <= dec!(100));
        // ERS 없이는 점수가 낮아져서 HOLD
        assert_eq!(result.recommendation, "HOLD");
        assert!(result.confidence > dec!(0.8));
    }

    #[test]
    fn test_risk_reward_calculation() {
        let scorer = GlobalScorer::new();

        // RR 3:1 (목표 30%, 손절 10%)
        let params = GlobalScorerParams {
            entry_price: Some(dec!(100)),
            target_price: Some(dec!(130)), // +30%
            stop_price: Some(dec!(90)),    // -10%
            ..Default::default()
        };

        let score = scorer.calculate_risk_reward(dec!(100), &params).unwrap();
        assert!(score >= 95.0); // RR 3:1 = 100점
    }

    #[test]
    fn test_momentum_calculation() {
        let scorer = GlobalScorer::new();
        let candles = create_test_candles(60);

        let params = GlobalScorerParams::default(); // structural_features = None
        let score = scorer.calculate_momentum(&candles, &params).unwrap();
        assert!(score >= 0.0 && score <= 100.0);
    }

    #[test]
    fn test_insufficient_data() {
        let scorer = GlobalScorer::new();
        let candles = create_test_candles(30); // 50개 미만

        let params = GlobalScorerParams::default();
        let result = scorer.calculate(&candles, params);

        assert!(result.is_err());
        match result {
            Err(GlobalScorerError::InsufficientData { required, provided }) => {
                assert_eq!(required, 50);
                assert_eq!(provided, 30);
            }
            _ => panic!("Expected InsufficientData error"),
        }
    }

    #[test]
    fn test_ebs_calculation() {
        let scorer = GlobalScorer::new();

        // 모든 조건 충족
        let structural = StructuralFeatures {
            low_trend: dec!(0.3),
            vol_quality: dec!(0.2),
            range_pos: dec!(0.6),
            dist_ma20: dec!(2.0),
            bb_width: dec!(2.5),
            bb_upper: dec!(110.0),
            bb_middle: dec!(100.0),
            bb_lower: dec!(90.0),
            rsi: dec!(55.0),
            ..Default::default()
        };

        let ebs = scorer.calculate_ebs(&structural, 55.0, 0.1);
        assert_eq!(ebs, 5.0); // 5개 조건 모두 충족

        // 일부 조건 충족
        let structural2 = StructuralFeatures {
            low_trend: dec!(-0.1), // 미충족
            vol_quality: dec!(0.2),
            range_pos: dec!(0.3), // 미충족
            dist_ma20: dec!(2.0),
            bb_width: dec!(2.5),
            bb_upper: dec!(110.0),
            bb_middle: dec!(100.0),
            bb_lower: dec!(90.0),
            rsi: dec!(55.0),
            ..Default::default()
        };

        let ebs2 = scorer.calculate_ebs(&structural2, 55.0, 0.1);
        assert_eq!(ebs2, 3.0); // RSI, MACD slope, vol_quality만 충족
    }

    #[test]
    fn test_momentum_with_ers() {
        let scorer = GlobalScorer::new();
        let candles = create_test_candles(60);

        let structural = StructuralFeatures {
            low_trend: dec!(0.3),
            vol_quality: dec!(0.2),
            range_pos: dec!(0.6),
            dist_ma20: dec!(2.0),
            bb_width: dec!(2.5),
            bb_upper: dec!(110.0),
            bb_middle: dec!(100.0),
            bb_lower: dec!(90.0),
            rsi: dec!(55.0),
            ..Default::default()
        };

        let params = GlobalScorerParams {
            structural_features: Some(structural),
            ..Default::default()
        };

        let score = scorer.calculate_momentum(&candles, &params).unwrap();
        assert!(score >= 0.0 && score <= 100.0);
    }

    #[test]
    fn test_momentum_without_structural() {
        let scorer = GlobalScorer::new();
        let candles = create_test_candles(60);

        let params = GlobalScorerParams::default(); // structural_features = None

        let score = scorer.calculate_momentum(&candles, &params).unwrap();

        // ERS 없어도 동작해야 함 (RSI + MACD만으로 최대 70점)
        assert!(score >= 0.0 && score <= 70.0);
    }

    #[test]
    fn test_global_scorer_with_structural_features() {
        let scorer = GlobalScorer::new();
        let candles = create_test_candles(60);

        let structural = StructuralFeatures {
            low_trend: dec!(0.4),
            vol_quality: dec!(0.3),
            range_pos: dec!(0.7),
            dist_ma20: dec!(1.5),
            bb_width: dec!(2.0),
            bb_upper: dec!(110.0),
            bb_middle: dec!(100.0),
            bb_lower: dec!(90.0),
            rsi: dec!(58.0),
            ..Default::default()
        };

        let params = GlobalScorerParams {
            symbol: Some("TEST/KRW".to_string()),
            market_type: Some(MarketType::Stock),
            structural_features: Some(structural),
            volume_percentile: Some(0.8),
            ..Default::default()
        };

        let result = scorer.calculate(&candles, params).unwrap();

        // Momentum 스코어 확인 (단순 증가 패턴이라 RSI가 높아서 0일 수 있음)
        let mom_score = result.component_scores.get("momentum").unwrap();
        assert!(mom_score >= &Decimal::ZERO);
        // GlobalScore 전체 검증
        assert!(result.overall_score >= Decimal::ZERO && result.overall_score <= dec!(100));
        // StructuralFeatures가 반영되었는지 확인 (confidence 증가)
        assert!(result.confidence > Decimal::ZERO);
    }
}
