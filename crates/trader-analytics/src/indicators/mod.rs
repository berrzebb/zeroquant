//! 기술적 지표 모듈.
//!
//! 이 모듈은 트레이딩 전략에서 사용되는 다양한 기술적 지표를 제공합니다.
//! ta-rs 라이브러리를 기반으로 구현되었으며, 추가적인 커스텀 지표도 포함합니다.
//!
//! # 지원 지표
//!
//! ## 추세 지표 (Trend Indicators)
//! - **SMA**: 단순 이동평균 (Simple Moving Average)
//! - **EMA**: 지수 이동평균 (Exponential Moving Average)
//! - **MACD**: 이동평균 수렴/확산 (Moving Average Convergence Divergence)
//! - **HMA**: Hull 이동평균 (Hull Moving Average)
//! - **SuperTrend**: 추세 추종 지표
//!
//! ## 모멘텀 지표 (Momentum Indicators)
//! - **RSI**: 상대강도지수 (Relative Strength Index)
//! - **Stochastic**: 스토캐스틱 오실레이터
//! - **모멘텀 점수**: 다기간 평균 모멘텀
//!
//! ## 변동성 지표 (Volatility Indicators)
//! - **Bollinger Bands**: 볼린저 밴드
//! - **ATR**: 평균 실제 범위 (Average True Range)
//! - **Keltner Channel**: 켈트너 채널
//! - **TTM Squeeze**: TTM Squeeze 지표
//!
//! ## 거래량 지표 (Volume Indicators)
//! - **OBV**: 거래량 균형 지표 (On-Balance Volume)
//! - **VWAP**: 거래량 가중 평균 가격 (Volume Weighted Average Price)
//!
//! ## 패턴 인식 (Pattern Recognition)
//! - **Candle Patterns**: 캔들스틱 패턴 감지 (망치형, 장악형 등)
//!
//! # 사용 예시
//!
//! ```ignore
//! use trader_analytics::indicators::{IndicatorEngine, SmaParams, RsiParams};
//!
//! let engine = IndicatorEngine::new();
//!
//! // SMA 계산
//! let sma = engine.sma(&prices, SmaParams { period: 20 })?;
//!
//! // RSI 계산
//! let rsi = engine.rsi(&prices, RsiParams { period: 14 })?;
//! ```

pub mod candle_patterns;
pub mod hma;
pub mod momentum;
pub mod structural;
pub mod supertrend;
pub mod trend;
pub mod volatility;
pub mod volume;
pub mod weekly_ma;

use rust_decimal::Decimal;
use thiserror::Error;

pub use candle_patterns::{
    CandlePatternIndicator, CandlePatternParams, CandlePatternResult, CandlePatternType,
};
pub use hma::{HmaIndicator, HmaParams};
pub use momentum::{MomentumCalculator, RsiParams, StochasticParams, StochasticResult};
pub use structural::StructuralFeatures;
pub use supertrend::{SuperTrendIndicator, SuperTrendParams, SuperTrendResult};
pub use trend::{EmaParams, MacdParams, MacdResult, SmaParams, TrendIndicators};
pub use volatility::{
    AtrParams, BollingerBandsParams, BollingerBandsResult, KeltnerChannelParams,
    KeltnerChannelResult, TtmSqueezeParams, TtmSqueezeResult, VolatilityIndicators,
};
pub use volume::{ObvIndicator, ObvParams, ObvResult, VwapIndicator, VwapParams, VwapResult};
pub use weekly_ma::{
    calculate_weekly_ma, detect_weekly_ma_cross, get_current_weekly_ma_distance,
    map_weekly_ma_to_daily, resample_to_weekly, WeeklyMaResult,
};

/// 지표 계산 오류.
#[derive(Debug, Error)]
pub enum IndicatorError {
    /// 데이터 부족 오류
    #[error("데이터가 부족합니다: 필요 {required}개, 제공 {provided}개")]
    InsufficientData { required: usize, provided: usize },

    /// 잘못된 파라미터
    #[error("잘못된 파라미터: {0}")]
    InvalidParameter(String),

    /// 계산 오류
    #[error("계산 오류: {0}")]
    CalculationError(String),
}

/// 지표 계산 결과 타입.
pub type IndicatorResult<T> = Result<T, IndicatorError>;

/// 통합 지표 엔진.
///
/// 모든 기술적 지표 계산을 위한 통합 인터페이스를 제공합니다.
/// 내부적으로 ta-rs 라이브러리와 커스텀 구현을 사용합니다.
#[derive(Debug, Default)]
pub struct IndicatorEngine {
    trend: TrendIndicators,
    momentum: MomentumCalculator,
    volatility: VolatilityIndicators,
    hma: HmaIndicator,
    obv: ObvIndicator,
    vwap: VwapIndicator,
    supertrend: SuperTrendIndicator,
    candle_patterns: CandlePatternIndicator,
}

impl IndicatorEngine {
    /// 새로운 지표 엔진 생성.
    pub fn new() -> Self {
        Self::default()
    }

    // ==================== 추세 지표 ====================

    /// 단순 이동평균 (SMA) 계산.
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `params` - SMA 파라미터 (기간)
    ///
    /// # 반환
    /// 계산된 SMA 값들의 벡터 (처음 period-1개는 None)
    pub fn sma(
        &self,
        prices: &[Decimal],
        params: SmaParams,
    ) -> IndicatorResult<Vec<Option<Decimal>>> {
        self.trend.sma(prices, params)
    }

    /// 지수 이동평균 (EMA) 계산.
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `params` - EMA 파라미터 (기간)
    ///
    /// # 반환
    /// 계산된 EMA 값들의 벡터
    pub fn ema(
        &self,
        prices: &[Decimal],
        params: EmaParams,
    ) -> IndicatorResult<Vec<Option<Decimal>>> {
        self.trend.ema(prices, params)
    }

    /// MACD (Moving Average Convergence Divergence) 계산.
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `params` - MACD 파라미터 (단기, 장기, 시그널 기간)
    ///
    /// # 반환
    /// MACD 라인, 시그널 라인, 히스토그램
    pub fn macd(&self, prices: &[Decimal], params: MacdParams) -> IndicatorResult<Vec<MacdResult>> {
        self.trend.macd(prices, params)
    }

    // ==================== 모멘텀 지표 ====================

    /// RSI (Relative Strength Index) 계산.
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `params` - RSI 파라미터 (기간, 기본값 14)
    ///
    /// # 반환
    /// 0-100 사이의 RSI 값들
    pub fn rsi(
        &self,
        prices: &[Decimal],
        params: RsiParams,
    ) -> IndicatorResult<Vec<Option<Decimal>>> {
        self.momentum.rsi(prices, params)
    }

    /// 스토캐스틱 오실레이터 계산.
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - 스토캐스틱 파라미터 (%K, %D 기간)
    ///
    /// # 반환
    /// %K, %D 값들
    pub fn stochastic(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: StochasticParams,
    ) -> IndicatorResult<Vec<StochasticResult>> {
        self.momentum.stochastic(high, low, close, params)
    }

    /// 다기간 모멘텀 점수 계산.
    ///
    /// 모멘텀 = (1개월 + 3개월 + 6개월 + 12개월) / 4
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `lookback_periods` - 참조 기간들 (일 단위)
    ///
    /// # 반환
    /// 현재 가격 기준 모멘텀 점수
    pub fn momentum_score(
        &self,
        prices: &[Decimal],
        lookback_periods: &[usize],
    ) -> IndicatorResult<Decimal> {
        self.momentum.momentum_score(prices, lookback_periods)
    }

    // ==================== 변동성 지표 ====================

    /// 볼린저 밴드 계산.
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `params` - 볼린저 밴드 파라미터 (기간, 표준편차 배수)
    ///
    /// # 반환
    /// 상단, 중간, 하단 밴드 값들
    pub fn bollinger_bands(
        &self,
        prices: &[Decimal],
        params: BollingerBandsParams,
    ) -> IndicatorResult<Vec<BollingerBandsResult>> {
        self.volatility.bollinger_bands(prices, params)
    }

    /// ATR (Average True Range) 계산.
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - ATR 파라미터 (기간, 기본값 14)
    ///
    /// # 반환
    /// ATR 값들
    pub fn atr(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: AtrParams,
    ) -> IndicatorResult<Vec<Option<Decimal>>> {
        self.volatility.atr(high, low, close, params)
    }

    /// Keltner Channel 계산.
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - Keltner Channel 파라미터
    ///
    /// # 반환
    /// Keltner Channel 값들
    pub fn keltner_channel(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: KeltnerChannelParams,
    ) -> IndicatorResult<Vec<KeltnerChannelResult>> {
        self.volatility.keltner_channel(high, low, close, params)
    }

    /// TTM Squeeze 계산.
    ///
    /// John Carter의 TTM Squeeze 지표.
    /// Bollinger Bands가 Keltner Channel 내부로 들어가면 에너지 응축(squeeze) 상태.
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - TTM Squeeze 파라미터
    ///
    /// # 반환
    /// TTM Squeeze 상태 및 모멘텀 정보
    pub fn ttm_squeeze(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: TtmSqueezeParams,
    ) -> IndicatorResult<Vec<TtmSqueezeResult>> {
        self.volatility.ttm_squeeze(high, low, close, params)
    }

    // ==================== 유틸리티 ====================

    /// 골든 크로스 감지.
    ///
    /// 단기 이동평균이 장기 이동평균을 상향 돌파하는 시점을 감지합니다.
    ///
    /// # 인자
    /// * `short_ma` - 단기 이동평균 값들
    /// * `long_ma` - 장기 이동평균 값들
    ///
    /// # 반환
    /// 각 시점에서 골든 크로스 발생 여부
    pub fn detect_golden_cross(
        &self,
        short_ma: &[Option<Decimal>],
        long_ma: &[Option<Decimal>],
    ) -> Vec<bool> {
        self.trend.detect_golden_cross(short_ma, long_ma)
    }

    /// 데드 크로스 감지.
    ///
    /// 단기 이동평균이 장기 이동평균을 하향 돌파하는 시점을 감지합니다.
    ///
    /// # 인자
    /// * `short_ma` - 단기 이동평균 값들
    /// * `long_ma` - 장기 이동평균 값들
    ///
    /// # 반환
    /// 각 시점에서 데드 크로스 발생 여부
    pub fn detect_dead_cross(
        &self,
        short_ma: &[Option<Decimal>],
        long_ma: &[Option<Decimal>],
    ) -> Vec<bool> {
        self.trend.detect_dead_cross(short_ma, long_ma)
    }

    // ==================== 추가 지표 ====================

    /// HMA (Hull Moving Average) 계산.
    ///
    /// HMA는 빠른 반응속도와 낮은 휩소를 특징으로 하는 이동평균입니다.
    ///
    /// # 인자
    /// * `prices` - 가격 데이터 (종가)
    /// * `params` - HMA 파라미터 (기간)
    ///
    /// # 반환
    /// 계산된 HMA 값들의 벡터
    pub fn hma(
        &self,
        prices: &[Decimal],
        params: HmaParams,
    ) -> IndicatorResult<Vec<Option<Decimal>>> {
        self.hma.calculate(prices, params)
    }

    /// OBV (On-Balance Volume) 계산.
    ///
    /// 거래량 기반으로 스마트 머니의 자금 흐름을 추적합니다.
    ///
    /// # 인자
    /// * `close` - 종가 데이터
    /// * `volume` - 거래량 데이터
    /// * `params` - OBV 파라미터
    ///
    /// # 반환
    /// OBV 값과 변화량
    pub fn obv(
        &self,
        close: &[Decimal],
        volume: &[Decimal],
        params: ObvParams,
    ) -> IndicatorResult<Vec<ObvResult>> {
        self.obv.calculate(close, volume, params)
    }

    /// OBV 다이버전스 감지.
    ///
    /// 가격과 OBV의 방향이 반대인 경우를 감지합니다.
    ///
    /// # 인자
    /// * `close` - 종가 데이터
    /// * `obv_results` - OBV 계산 결과
    /// * `lookback` - 비교 기간 (기본: 5)
    ///
    /// # 반환
    /// 각 시점에서 약세 다이버전스 발생 여부
    pub fn obv_divergence(
        &self,
        close: &[Decimal],
        obv_results: &[ObvResult],
        lookback: usize,
    ) -> IndicatorResult<Vec<bool>> {
        self.obv.detect_divergence(close, obv_results, lookback)
    }

    // ==================== VWAP ====================

    /// VWAP (Volume Weighted Average Price) 계산.
    ///
    /// 거래량 가중 평균 가격을 계산합니다.
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `volume` - 거래량 데이터
    /// * `params` - VWAP 파라미터
    ///
    /// # 반환
    /// VWAP 값, 상/하단 밴드, 괴리율
    pub fn vwap(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        volume: &[Decimal],
        params: VwapParams,
    ) -> IndicatorResult<Vec<VwapResult>> {
        self.vwap.calculate(high, low, close, volume, params)
    }

    /// VWAP 돌파 감지.
    ///
    /// 가격이 VWAP을 상향/하향 돌파하는 경우를 감지합니다.
    ///
    /// # 인자
    /// * `close` - 종가 데이터
    /// * `vwap_results` - VWAP 계산 결과
    ///
    /// # 반환
    /// 각 시점의 돌파 방향 (1: 상향, -1: 하향, 0: 없음)
    pub fn vwap_crossover(
        &self,
        close: &[Decimal],
        vwap_results: &[VwapResult],
    ) -> IndicatorResult<Vec<i8>> {
        self.vwap.detect_crossover(close, vwap_results)
    }

    /// SuperTrend 지표 계산.
    ///
    /// ATR 기반 추세 추종 지표로 명확한 매수/매도 시그널을 제공합니다.
    ///
    /// # 인자
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - SuperTrend 파라미터
    ///
    /// # 반환
    /// SuperTrend 값과 시그널
    pub fn supertrend(
        &self,
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: SuperTrendParams,
    ) -> IndicatorResult<Vec<SuperTrendResult>> {
        self.supertrend.calculate(high, low, close, params)
    }

    /// 캔들 패턴 감지.
    ///
    /// 망치형, 장악형 등 주요 캔들스틱 패턴을 감지합니다.
    ///
    /// # 인자
    /// * `open` - 시가 데이터
    /// * `high` - 고가 데이터
    /// * `low` - 저가 데이터
    /// * `close` - 종가 데이터
    /// * `params` - 캔들 패턴 파라미터
    ///
    /// # 반환
    /// 각 시점에서 감지된 패턴과 신뢰도
    pub fn candle_patterns(
        &self,
        open: &[Decimal],
        high: &[Decimal],
        low: &[Decimal],
        close: &[Decimal],
        params: CandlePatternParams,
    ) -> IndicatorResult<Vec<CandlePatternResult>> {
        self.candle_patterns.detect(open, high, low, close, params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_prices() -> Vec<Decimal> {
        vec![
            dec!(100.0),
            dec!(102.0),
            dec!(101.0),
            dec!(103.0),
            dec!(105.0),
            dec!(104.0),
            dec!(106.0),
            dec!(108.0),
            dec!(107.0),
            dec!(109.0),
            dec!(111.0),
            dec!(110.0),
            dec!(112.0),
            dec!(114.0),
            dec!(113.0),
        ]
    }

    #[test]
    fn test_sma_calculation() {
        let engine = IndicatorEngine::new();
        let prices = sample_prices();

        let sma = engine.sma(&prices, SmaParams { period: 5 }).unwrap();

        // 처음 4개는 None (데이터 부족)
        assert!(sma[0].is_none());
        assert!(sma[3].is_none());

        // 5번째부터 값이 있어야 함
        assert!(sma[4].is_some());
    }

    #[test]
    fn test_rsi_calculation() {
        let engine = IndicatorEngine::new();
        let prices = sample_prices();

        let rsi = engine.rsi(&prices, RsiParams { period: 14 }).unwrap();

        // RSI 값이 0-100 범위인지 확인
        for value in rsi.iter().flatten() {
            assert!(*value >= Decimal::ZERO);
            assert!(*value <= dec!(100));
        }
    }

    #[test]
    fn test_insufficient_data_error() {
        let engine = IndicatorEngine::new();
        let prices = vec![dec!(100.0), dec!(101.0)];

        let result = engine.sma(&prices, SmaParams { period: 20 });
        assert!(result.is_err());
    }

    #[test]
    fn test_hma_calculation() {
        let engine = IndicatorEngine::new();
        let prices = sample_prices();

        let hma = engine.hma(&prices, HmaParams { period: 9 }).unwrap();

        // 처음 몇 개는 None
        assert!(hma[0].is_none());

        // 충분한 데이터가 있으면 값이 계산됨
        assert!(hma[hma.len() - 1].is_some());
    }

    #[test]
    fn test_obv_calculation() {
        let engine = IndicatorEngine::new();
        let close = vec![dec!(100.0), dec!(102.0), dec!(101.0), dec!(103.0)];
        let volume = vec![dec!(1000.0), dec!(1500.0), dec!(1200.0), dec!(1800.0)];

        let obv = engine.obv(&close, &volume, ObvParams::default()).unwrap();

        assert_eq!(obv.len(), close.len());
        // 첫 번째는 변화 없음
        assert_eq!(obv[0].change, 0);
        // 두 번째는 가격 상승 -> 거래량 추가
        assert_eq!(obv[1].change, 1500);
    }

    #[test]
    fn test_supertrend_calculation() {
        let engine = IndicatorEngine::new();
        let high = vec![
            dec!(102.0),
            dec!(104.0),
            dec!(103.0),
            dec!(106.0),
            dec!(108.0),
            dec!(107.0),
            dec!(110.0),
            dec!(112.0),
            dec!(111.0),
            dec!(114.0),
            dec!(116.0),
            dec!(115.0),
        ];
        let low = vec![
            dec!(98.0),
            dec!(100.0),
            dec!(99.0),
            dec!(102.0),
            dec!(104.0),
            dec!(103.0),
            dec!(106.0),
            dec!(108.0),
            dec!(107.0),
            dec!(110.0),
            dec!(112.0),
            dec!(111.0),
        ];
        let close = vec![
            dec!(100.0),
            dec!(102.0),
            dec!(101.0),
            dec!(104.0),
            dec!(106.0),
            dec!(105.0),
            dec!(108.0),
            dec!(110.0),
            dec!(109.0),
            dec!(112.0),
            dec!(114.0),
            dec!(113.0),
        ];

        let result = engine
            .supertrend(&high, &low, &close, SuperTrendParams::default())
            .unwrap();

        assert_eq!(result.len(), high.len());
        // 충분한 데이터가 있으면 값이 계산됨
        assert!(result[result.len() - 1].value.is_some());
    }

    #[test]
    fn test_candle_patterns_detection() {
        let engine = IndicatorEngine::new();
        // 도지 패턴: 시가 = 종가
        let open = vec![dec!(100.0)];
        let high = vec![dec!(102.0)];
        let low = vec![dec!(98.0)];
        let close = vec![dec!(100.0)];

        let result = engine
            .candle_patterns(&open, &high, &low, &close, CandlePatternParams::default())
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].pattern, CandlePatternType::Doji);
    }
}
