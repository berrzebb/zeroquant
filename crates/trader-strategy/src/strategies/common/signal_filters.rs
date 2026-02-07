//! 신호 필터링 및 확인.
//!
//! 이 모듈은 전략에서 생성된 신호를 필터링하고 검증하는 기능을 제공합니다.
//!
//! ## 두 가지 필터링 레이어
//!
//! 1. **기술적 필터링** (VolumeFilter, TrendFilter 등)
//!    - 거래량, 추세 방향 등 기술적 조건 확인
//!    - Signal 생성 전에 적용
//!
//! 2. **충돌 검증** (validate_signals_with_context)
//!    - 미체결 주문, 중복 포지션 등 실행 가능성 확인
//!    - Signal 생성 후, 반환 전에 적용

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use trader_core::domain::{Signal, SignalConflictError, StrategyContext};
use tracing::warn;

/// 신호 강도.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalStrength {
    /// 약한 신호
    Weak,
    /// 중간 신호
    Medium,
    /// 강한 신호
    Strong,
}

/// 필터링된 신호.
#[derive(Debug, Clone)]
pub struct FilteredSignal {
    /// 원본 신호가 유효한지
    pub is_valid: bool,
    /// 신호 강도
    pub strength: SignalStrength,
    /// 필터링 이유 (거부된 경우)
    pub reason: Option<String>,
}

/// 신호 필터 trait.
pub trait SignalFilter: Send + Sync + std::fmt::Debug {
    /// 신호를 필터링합니다.
    ///
    /// # Arguments
    /// * `signal` - 원본 신호 (매수/매도 여부)
    /// * `context` - 필터링에 필요한 컨텍스트 데이터
    ///
    /// # Returns
    /// 필터링된 신호
    fn filter(&self, signal: bool, context: &SignalContext) -> FilteredSignal;
}

/// 신호 필터링 컨텍스트.
///
/// 필터링에 필요한 모든 데이터를 포함합니다.
#[derive(Debug, Clone)]
pub struct SignalContext {
    /// 현재가
    pub current_price: Decimal,
    /// 거래량
    pub volume: Decimal,
    /// 평균 거래량
    pub avg_volume: Option<Decimal>,
    /// RSI 값
    pub rsi: Option<Decimal>,
    /// MACD 히스토그램
    pub macd_histogram: Option<Decimal>,
    /// 추세 방향 (1: 상승, -1: 하락, 0: 중립)
    pub trend: i8,
}

/// 거래량 필터.
///
/// 거래량이 평균보다 낮으면 신호를 거부합니다.
#[derive(Debug, Clone)]
pub struct VolumeFilter {
    /// 최소 거래량 배수 (평균 대비)
    pub min_volume_ratio: Decimal,
}

impl VolumeFilter {
    pub fn new(min_volume_ratio: Decimal) -> Self {
        Self { min_volume_ratio }
    }
}

impl SignalFilter for VolumeFilter {
    fn filter(&self, signal: bool, context: &SignalContext) -> FilteredSignal {
        if !signal {
            return FilteredSignal {
                is_valid: false,
                strength: SignalStrength::Weak,
                reason: Some("원본 신호 없음".to_string()),
            };
        }

        if let Some(avg_volume) = context.avg_volume {
            let volume_ratio = context.volume / avg_volume;

            if volume_ratio < self.min_volume_ratio {
                return FilteredSignal {
                    is_valid: false,
                    strength: SignalStrength::Weak,
                    reason: Some(format!(
                        "거래량 부족: {:.2}x (최소 {:.2}x)",
                        volume_ratio, self.min_volume_ratio
                    )),
                };
            }

            // 거래량이 충분하면 강도 계산
            let strength = if volume_ratio >= self.min_volume_ratio * dec!(2) {
                SignalStrength::Strong
            } else if volume_ratio >= self.min_volume_ratio * dec!(1.5) {
                SignalStrength::Medium
            } else {
                SignalStrength::Weak
            };

            FilteredSignal {
                is_valid: true,
                strength,
                reason: None,
            }
        } else {
            // 평균 거래량 정보가 없으면 통과
            FilteredSignal {
                is_valid: true,
                strength: SignalStrength::Medium,
                reason: None,
            }
        }
    }
}

/// 추세 필터.
///
/// 추세 방향과 신호 방향이 일치하지 않으면 거부합니다.
#[derive(Debug, Clone)]
pub struct TrendFilter {
    /// 추세 필터 활성화
    pub enabled: bool,
}

impl TrendFilter {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

impl SignalFilter for TrendFilter {
    fn filter(&self, signal: bool, context: &SignalContext) -> FilteredSignal {
        if !self.enabled {
            return FilteredSignal {
                is_valid: signal,
                strength: SignalStrength::Medium,
                reason: None,
            };
        }

        if !signal {
            return FilteredSignal {
                is_valid: false,
                strength: SignalStrength::Weak,
                reason: Some("원본 신호 없음".to_string()),
            };
        }

        // 추세와 일치하는지 확인
        let is_aligned = context.trend > 0; // 상승 추세일 때만 매수 신호 허용

        if !is_aligned {
            return FilteredSignal {
                is_valid: false,
                strength: SignalStrength::Weak,
                reason: Some("추세 불일치".to_string()),
            };
        }

        FilteredSignal {
            is_valid: true,
            strength: SignalStrength::Strong,
            reason: None,
        }
    }
}

/// 복합 필터.
///
/// 여러 필터를 순차적으로 적용합니다.
#[derive(Debug)]
pub struct CompositeFilter {
    filters: Vec<Box<dyn SignalFilter>>,
}

impl CompositeFilter {
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
        }
    }

    pub fn add_filter(mut self, filter: Box<dyn SignalFilter>) -> Self {
        self.filters.push(filter);
        self
    }
}

impl SignalFilter for CompositeFilter {
    fn filter(&self, signal: bool, context: &SignalContext) -> FilteredSignal {
        let current_signal = signal;
        let mut min_strength = SignalStrength::Strong;

        for filter in &self.filters {
            let result = filter.filter(current_signal, context);

            if !result.is_valid {
                return result;
            }

            // 가장 약한 강도로 설정
            if result.strength == SignalStrength::Weak {
                min_strength = SignalStrength::Weak;
            } else if result.strength == SignalStrength::Medium
                && min_strength != SignalStrength::Weak
            {
                min_strength = SignalStrength::Medium;
            }
        }

        FilteredSignal {
            is_valid: true,
            strength: min_strength,
            reason: None,
        }
    }
}

impl Default for CompositeFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// 확인 신호 패턴.
///
/// N개의 연속된 신호가 동일 방향일 때만 유효한 신호로 인정합니다.
#[derive(Debug, Clone)]
pub struct ConfirmationPattern {
    /// 필요한 확인 횟수
    pub required_confirmations: usize,
    /// 현재까지의 확인 카운트
    confirmations: usize,
}

impl ConfirmationPattern {
    pub fn new(required_confirmations: usize) -> Self {
        Self {
            required_confirmations,
            confirmations: 0,
        }
    }

    /// 신호를 추가하고 확인 여부를 반환합니다.
    pub fn add_signal(&mut self, signal: bool) -> bool {
        if signal {
            self.confirmations += 1;
        } else {
            self.confirmations = 0;
        }

        self.confirmations >= self.required_confirmations
    }

    /// 확인 카운트를 리셋합니다.
    pub fn reset(&mut self) {
        self.confirmations = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_filter_pass() {
        let filter = VolumeFilter::new(dec!(1.5));
        let context = SignalContext {
            current_price: dec!(100),
            volume: dec!(2500), // 2.5배 = 1.5 * 1.5보다 큼, Medium 강도
            avg_volume: Some(dec!(1000)),
            rsi: None,
            macd_histogram: None,
            trend: 1,
        };

        let result = filter.filter(true, &context);
        assert!(result.is_valid);
        assert_eq!(result.strength, SignalStrength::Medium);
    }

    #[test]
    fn test_volume_filter_reject() {
        let filter = VolumeFilter::new(dec!(2.0));
        let context = SignalContext {
            current_price: dec!(100),
            volume: dec!(1000),
            avg_volume: Some(dec!(1000)),
            rsi: None,
            macd_histogram: None,
            trend: 1,
        };

        let result = filter.filter(true, &context);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_trend_filter_aligned() {
        let filter = TrendFilter::new(true);
        let context = SignalContext {
            current_price: dec!(100),
            volume: dec!(1000),
            avg_volume: None,
            rsi: None,
            macd_histogram: None,
            trend: 1, // 상승 추세
        };

        let result = filter.filter(true, &context); // 매수 신호
        assert!(result.is_valid);
    }

    #[test]
    fn test_trend_filter_not_aligned() {
        let filter = TrendFilter::new(true);
        let context = SignalContext {
            current_price: dec!(100),
            volume: dec!(1000),
            avg_volume: None,
            rsi: None,
            macd_histogram: None,
            trend: -1, // 하락 추세
        };

        let result = filter.filter(true, &context); // 매수 신호
        assert!(!result.is_valid);
    }

    #[test]
    fn test_composite_filter() {
        let filter = CompositeFilter::new()
            .add_filter(Box::new(VolumeFilter::new(dec!(1.0))))
            .add_filter(Box::new(TrendFilter::new(true)));

        let context = SignalContext {
            current_price: dec!(100),
            volume: dec!(1500),
            avg_volume: Some(dec!(1000)),
            rsi: None,
            macd_histogram: None,
            trend: 1,
        };

        let result = filter.filter(true, &context);
        assert!(result.is_valid);
    }

    #[test]
    fn test_confirmation_pattern() {
        let mut pattern = ConfirmationPattern::new(3);

        assert!(!pattern.add_signal(true)); // 1
        assert!(!pattern.add_signal(true)); // 2
        assert!(pattern.add_signal(true)); // 3 - 확인됨
        assert!(pattern.add_signal(true)); // 계속 확인됨

        pattern.add_signal(false); // 리셋
        assert!(!pattern.add_signal(true)); // 다시 1부터 시작
    }
}

// ============================================================================
// 충돌 검증 (StrategyContext 기반)
// ============================================================================

/// Signal 충돌 검증 결과.
#[derive(Debug)]
pub struct ValidationResult {
    /// 유효한 Signal 목록
    pub valid_signals: Vec<Signal>,
    /// 충돌한 Signal 목록 (Signal, 충돌 이유)
    pub conflicts: Vec<(Signal, SignalConflictError)>,
}

impl ValidationResult {
    /// 모든 Signal이 유효한지 확인.
    pub fn all_valid(&self) -> bool {
        self.conflicts.is_empty()
    }

    /// 충돌 개수 반환.
    pub fn conflict_count(&self) -> usize {
        self.conflicts.len()
    }
}

/// StrategyContext를 사용하여 Signal들의 충돌을 검증합니다.
///
/// 전략에서 Signal을 생성한 후, 반환하기 전에 이 함수를 호출하여
/// 미체결 주문, 중복 포지션 등의 충돌을 필터링합니다.
///
/// # Arguments
///
/// * `signals` - 검증할 Signal 목록
/// * `context` - StrategyContext (read lock 획득됨)
///
/// # Returns
///
/// `ValidationResult` - 유효한 Signal과 충돌 목록
///
/// # Example
///
/// ```ignore
/// let signals = vec![
///     Signal::entry("my_strategy", ticker.clone(), Side::Buy),
///     Signal::exit("my_strategy", other_ticker.clone(), Side::Sell),
/// ];
///
/// let ctx_lock = self.context.read();
/// let result = validate_signals_with_context(signals, &ctx_lock);
///
/// if !result.conflicts.is_empty() {
///     for (signal, error) in &result.conflicts {
///         warn!("Signal 충돌: {} - {}", signal.ticker, error);
///     }
/// }
///
/// result.valid_signals  // 유효한 Signal만 반환
/// ```
pub fn validate_signals_with_context(
    signals: Vec<Signal>,
    context: &StrategyContext,
) -> ValidationResult {
    let mut valid_signals = Vec::new();
    let mut conflicts = Vec::new();

    for signal in signals {
        match context.can_execute_signal(&signal) {
            Ok(()) => valid_signals.push(signal),
            Err(e) => {
                warn!(
                    ticker = %signal.ticker,
                    signal_type = ?signal.signal_type,
                    error = %e,
                    "Signal 충돌 감지"
                );
                conflicts.push((signal, e));
            }
        }
    }

    ValidationResult {
        valid_signals,
        conflicts,
    }
}

/// Signal 단일 검증 (충돌 여부만 확인).
///
/// `validate_signals_with_context`의 단일 Signal 버전입니다.
///
/// # Returns
///
/// * `Ok(())` - 실행 가능
/// * `Err(SignalConflictError)` - 충돌 발생
pub fn can_execute_signal(
    signal: &Signal,
    context: &StrategyContext,
) -> Result<(), SignalConflictError> {
    context.can_execute_signal(signal)
}
