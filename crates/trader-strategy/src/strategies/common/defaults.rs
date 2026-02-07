//! 트레이딩 전략을 위한 기본 상수 정의.
//!
//! 이 모듈은 다양한 전략에서 사용되는 기본값을 제공합니다:
//!
//! - [`IndicatorDefaults`]: 기술적 지표 기본값 (SMA, RSI, BB, ATR, MACD)
//! - [`RiskDefaults`]: 리스크 관리 기본값 (포지션 사이즈, 손절/익절)
//! - [`GridDefaults`]: 그리드 전략 기본값
//! - [`MomentumDefaults`]: 모멘텀 전략 기본값
//! - [`AllocationDefaults`]: 자산 배분 기본값
//!
//! # Example
//!
//! ```
//! use trader_strategy::strategies::common::defaults::{IndicatorDefaults, RiskDefaults};
//!
//! // 기술적 지표 기본값 사용
//! let rsi_period = IndicatorDefaults::RSI_PERIOD;
//! assert_eq!(rsi_period, 14);
//!
//! // 리스크 관리 기본값 사용
//! let stop_loss = RiskDefaults::STOP_LOSS_PCT;
//! assert_eq!(stop_loss, 2.0);
//! ```

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// 기술적 지표 기본값.
///
/// SMA, RSI, Bollinger Bands, ATR, MACD 등 주요 기술적 지표의
/// 표준 파라미터 값을 정의합니다.
pub struct IndicatorDefaults;

impl IndicatorDefaults {
    // === SMA (Simple Moving Average) ===
    /// 단기 이동평균 기간 (10일)
    pub const SMA_SHORT_PERIOD: usize = 10;
    /// 장기 이동평균 기간 (20일)
    pub const SMA_LONG_PERIOD: usize = 20;

    // === RSI (Relative Strength Index) ===
    /// RSI 계산 기간 (14일)
    pub const RSI_PERIOD: usize = 14;
    /// RSI 과매도 임계값 (30.0)
    pub const RSI_OVERSOLD: f64 = 30.0;
    /// RSI 과매수 임계값 (70.0)
    pub const RSI_OVERBOUGHT: f64 = 70.0;

    // === Bollinger Bands ===
    /// 볼린저 밴드 기간 (20일)
    pub const BB_PERIOD: usize = 20;
    /// 볼린저 밴드 표준편차 배수 (2.0)
    pub const BB_STD_DEV: f64 = 2.0;

    // === ATR (Average True Range) ===
    /// ATR 계산 기간 (14일)
    pub const ATR_PERIOD: usize = 14;

    // === MACD (Moving Average Convergence Divergence) ===
    /// MACD 빠른 이동평균 기간 (12일)
    pub const MACD_FAST: usize = 12;
    /// MACD 느린 이동평균 기간 (26일)
    pub const MACD_SLOW: usize = 26;
    /// MACD 시그널 라인 기간 (9일)
    pub const MACD_SIGNAL: usize = 9;
}

/// 리스크 관리 기본값.
///
/// 포지션 사이징, 손절/익절, 최대 허용 손실 등
/// 리스크 관리에 필요한 기본값을 정의합니다.
pub struct RiskDefaults;

impl RiskDefaults {
    /// 기본 포지션 사이즈 (100,000 KRW/USD)
    pub const DEFAULT_POSITION_SIZE: Decimal = dec!(100_000);
    /// 손절 비율 (2.0%)
    pub const STOP_LOSS_PCT: f64 = 2.0;
    /// 익절 비율 (5.0%)
    pub const TAKE_PROFIT_PCT: f64 = 5.0;
    /// 최대 허용 손실률 (10.0%)
    pub const MAX_DRAWDOWN_PCT: f64 = 10.0;
}

/// 그리드 전략 기본값.
///
/// 그리드 트레이딩 전략에서 사용하는 기본 파라미터를 정의합니다.
pub struct GridDefaults;

impl GridDefaults {
    /// 그리드 개수 (10개)
    pub const NUM_GRIDS: usize = 10;
    /// 그리드 간격 비율 (1.0%)
    pub const GRID_SPACING_PCT: f64 = 1.0;
}

/// 모멘텀 전략 기본값.
///
/// 모멘텀 기반 자산 배분 전략에서 사용하는 기본 파라미터를 정의합니다.
pub struct MomentumDefaults;

impl MomentumDefaults {
    /// 모멘텀 계산 기간 (20일)
    pub const MOMENTUM_PERIOD: usize = 20;
    /// 리밸런싱 주기 (30일)
    pub const REBALANCE_FREQUENCY_DAYS: u32 = 30;
    /// 상위 자산 개수 (4개)
    pub const TOP_N_ASSETS: usize = 4;
}

/// 자산 배분 기본값.
///
/// 포트폴리오 자산 배분 전략에서 사용하는 기본 파라미터를 정의합니다.
pub struct AllocationDefaults;

impl AllocationDefaults {
    /// 최소 자산 배분 비율 (5.0%)
    pub const MIN_ALLOCATION_PCT: f64 = 5.0;
    /// 최대 자산 배분 비율 (40.0%)
    pub const MAX_ALLOCATION_PCT: f64 = 40.0;
    /// 현금 보유 비율 (5.0%)
    pub const CASH_RESERVE_PCT: f64 = 5.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indicator_defaults() {
        // SMA
        assert_eq!(IndicatorDefaults::SMA_SHORT_PERIOD, 10);
        assert_eq!(IndicatorDefaults::SMA_LONG_PERIOD, 20);

        // RSI
        assert_eq!(IndicatorDefaults::RSI_PERIOD, 14);
        assert!((IndicatorDefaults::RSI_OVERSOLD - 30.0).abs() < f64::EPSILON);
        assert!((IndicatorDefaults::RSI_OVERBOUGHT - 70.0).abs() < f64::EPSILON);

        // Bollinger Bands
        assert_eq!(IndicatorDefaults::BB_PERIOD, 20);
        assert!((IndicatorDefaults::BB_STD_DEV - 2.0).abs() < f64::EPSILON);

        // ATR
        assert_eq!(IndicatorDefaults::ATR_PERIOD, 14);

        // MACD
        assert_eq!(IndicatorDefaults::MACD_FAST, 12);
        assert_eq!(IndicatorDefaults::MACD_SLOW, 26);
        assert_eq!(IndicatorDefaults::MACD_SIGNAL, 9);
    }

    #[test]
    fn test_risk_defaults() {
        assert_eq!(RiskDefaults::DEFAULT_POSITION_SIZE, dec!(100_000));
        assert!((RiskDefaults::STOP_LOSS_PCT - 2.0).abs() < f64::EPSILON);
        assert!((RiskDefaults::TAKE_PROFIT_PCT - 5.0).abs() < f64::EPSILON);
        assert!((RiskDefaults::MAX_DRAWDOWN_PCT - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_grid_defaults() {
        assert_eq!(GridDefaults::NUM_GRIDS, 10);
        assert!((GridDefaults::GRID_SPACING_PCT - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_momentum_defaults() {
        assert_eq!(MomentumDefaults::MOMENTUM_PERIOD, 20);
        assert_eq!(MomentumDefaults::REBALANCE_FREQUENCY_DAYS, 30);
        assert_eq!(MomentumDefaults::TOP_N_ASSETS, 4);
    }

    #[test]
    fn test_allocation_defaults() {
        assert!((AllocationDefaults::MIN_ALLOCATION_PCT - 5.0).abs() < f64::EPSILON);
        assert!((AllocationDefaults::MAX_ALLOCATION_PCT - 40.0).abs() < f64::EPSILON);
        assert!((AllocationDefaults::CASH_RESERVE_PCT - 5.0).abs() < f64::EPSILON);
    }
}
