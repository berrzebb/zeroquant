//! 공통 청산 및 리스크 관리 설정 (모든 전략에서 사용).
//!
//! 리스크 관리 타입별 섹션 구조:
//! - **StopLossConfig**: 손절 (Fixed% / ATR 기반)
//! - **TakeProfitConfig**: 익절 (Fixed%)
//! - **TrailingStopConfig**: 트레일링 스톱 (Fixed/ATR/Step/ParabolicSar)
//! - **ProfitLockConfig**: 수익 잠금 (threshold 달성 시 lock%)
//! - **DailyLossLimitConfig**: 일일 손실 한도
//!
//! `#[fragment("risk.exit_config")]`와 함께 사용하여 UI 스키마에 리스크 관리 옵션을 추가합니다.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use trader_core::{Side, Signal, SignalType};

// ============================================================================
// 열거형
// ============================================================================

/// 손절 모드.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum StopLossMode {
    /// 고정 퍼센트 기반 손절
    #[default]
    Fixed,
    /// ATR (평균 진폭 범위) 기반 손절
    AtrBased,
}

/// 트레일링 스톱 모드.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum TrailingMode {
    /// 고정 퍼센트 트레일링
    #[default]
    FixedPercentage,
    /// ATR 기반 트레일링
    AtrBased,
    /// 단계별 트레일링 (수익 구간별 다른 트레일 %)
    Step,
    /// Parabolic SAR 트레일링
    ParabolicSar,
}

/// 단계별 트레일링의 수익 구간 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepLevel {
    /// 수익률 구간 (%) - 이 수익률 이상일 때 적용
    pub profit_pct: Decimal,
    /// 트레일 거리 (%) - 고점 대비 이 비율 이하 하락 시 청산
    pub trail_pct: Decimal,
}

// ============================================================================
// 섹션 구조체
// ============================================================================

/// 손절 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopLossConfig {
    /// 손절 활성화
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// 손절 모드
    #[serde(default)]
    pub mode: StopLossMode,

    /// 손절 비율 (%) - Fixed 모드에서 사용
    #[serde(default = "default_stop_loss_pct")]
    pub pct: Decimal,

    /// ATR 배수 - AtrBased 모드에서 사용
    #[serde(default = "default_atr_multiplier")]
    pub atr_multiplier: Decimal,

    /// ATR 기간 - AtrBased 모드에서 사용
    #[serde(default = "default_atr_period")]
    pub atr_period: usize,
}

impl Default for StopLossConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: StopLossMode::Fixed,
            pct: default_stop_loss_pct(),
            atr_multiplier: default_atr_multiplier(),
            atr_period: default_atr_period(),
        }
    }
}

/// 익절 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeProfitConfig {
    /// 익절 활성화
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// 익절 비율 (%)
    #[serde(default = "default_take_profit_pct")]
    pub pct: Decimal,
}

impl Default for TakeProfitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pct: default_take_profit_pct(),
        }
    }
}

/// 트레일링 스톱 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrailingStopConfig {
    /// 트레일링 스톱 활성화
    #[serde(default)]
    pub enabled: bool,

    /// 트레일링 모드
    #[serde(default)]
    pub mode: TrailingMode,

    /// 트레일링 시작 수익률 (%) - FixedPercentage/Step 모드
    #[serde(default = "default_trailing_trigger_pct")]
    pub trigger_pct: Decimal,

    /// 트레일링 스톱 비율 (%) - FixedPercentage 모드
    #[serde(default = "default_trailing_stop_pct")]
    pub stop_pct: Decimal,

    /// ATR 배수 - AtrBased 모드
    #[serde(default = "default_trailing_atr_multiplier")]
    pub atr_multiplier: Decimal,

    /// 단계별 수익 구간 - Step 모드
    #[serde(default)]
    pub step_levels: Vec<StepLevel>,
}

impl Default for TrailingStopConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: TrailingMode::FixedPercentage,
            trigger_pct: default_trailing_trigger_pct(),
            stop_pct: default_trailing_stop_pct(),
            atr_multiplier: default_trailing_atr_multiplier(),
            step_levels: Vec::new(),
        }
    }
}

/// 수익 잠금 설정.
///
/// 수익이 threshold를 달성하면, 그 이후 lock_pct 이하로 하락할 수 없게 합니다.
/// 예: threshold=5%, lock=80% → 5% 수익 달성 후, 수익의 80% (4%) 이하로 내려가면 청산.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfitLockConfig {
    /// 수익 잠금 활성화
    #[serde(default)]
    pub enabled: bool,

    /// 수익 잠금 시작 임계값 (%) - 이 수익률 달성 시 잠금 시작
    #[serde(default = "default_profit_lock_threshold")]
    pub threshold_pct: Decimal,

    /// 잠금 비율 (%) - 달성 수익 대비 보호 비율
    #[serde(default = "default_profit_lock_pct")]
    pub lock_pct: Decimal,
}

impl Default for ProfitLockConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_pct: default_profit_lock_threshold(),
            lock_pct: default_profit_lock_pct(),
        }
    }
}

/// 일일 손실 한도 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyLossLimitConfig {
    /// 일일 손실 한도 활성화
    #[serde(default)]
    pub enabled: bool,

    /// 일일 최대 손실 비율 (%) - 계좌 대비
    #[serde(default = "default_daily_max_loss")]
    pub max_loss_pct: Decimal,
}

impl Default for DailyLossLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_loss_pct: default_daily_max_loss(),
        }
    }
}

// ============================================================================
// ExitConfig 메인 구조체
// ============================================================================

/// 리스크 관리 통합 설정.
///
/// 각 섹션은 `enabled: bool`로 독립적으로 활성화/비활성화 가능합니다.
/// 전략 엔진 레벨에서 일괄 처리됩니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitConfig {
    /// 손절 설정
    #[serde(default)]
    pub stop_loss: StopLossConfig,

    /// 익절 설정
    #[serde(default)]
    pub take_profit: TakeProfitConfig,

    /// 트레일링 스톱 설정
    #[serde(default)]
    pub trailing_stop: TrailingStopConfig,

    /// 수익 잠금 설정
    #[serde(default)]
    pub profit_lock: ProfitLockConfig,

    /// 일일 손실 한도 설정
    #[serde(default)]
    pub daily_loss_limit: DailyLossLimitConfig,

    /// 반대 신호 시 청산
    #[serde(default = "default_true")]
    pub exit_on_opposite_signal: bool,
}

// ============================================================================
// 기본값 함수들
// ============================================================================

fn default_true() -> bool {
    true
}
fn default_stop_loss_pct() -> Decimal {
    dec!(2.0)
}
fn default_take_profit_pct() -> Decimal {
    dec!(4.0)
}
fn default_atr_multiplier() -> Decimal {
    dec!(2.0)
}
fn default_atr_period() -> usize {
    14
}
fn default_trailing_trigger_pct() -> Decimal {
    dec!(2.0)
}
fn default_trailing_stop_pct() -> Decimal {
    dec!(1.0)
}
fn default_trailing_atr_multiplier() -> Decimal {
    dec!(2.0)
}
fn default_profit_lock_threshold() -> Decimal {
    dec!(5.0)
}
fn default_profit_lock_pct() -> Decimal {
    dec!(80.0)
}
fn default_daily_max_loss() -> Decimal {
    dec!(3.0)
}

// ============================================================================
// Default, 헬퍼 메서드, 프리셋
// ============================================================================

impl Default for ExitConfig {
    fn default() -> Self {
        Self {
            stop_loss: StopLossConfig::default(),
            take_profit: TakeProfitConfig::default(),
            trailing_stop: TrailingStopConfig::default(),
            profit_lock: ProfitLockConfig::default(),
            daily_loss_limit: DailyLossLimitConfig::default(),
            exit_on_opposite_signal: true,
        }
    }
}

impl ExitConfig {
    /// 손절 비율 반환 (활성화된 경우에만 Some).
    pub fn stop_loss(&self) -> Option<Decimal> {
        if self.stop_loss.enabled {
            Some(self.stop_loss.pct)
        } else {
            None
        }
    }

    /// 익절 비율 반환 (활성화된 경우에만 Some).
    pub fn take_profit(&self) -> Option<Decimal> {
        if self.take_profit.enabled {
            Some(self.take_profit.pct)
        } else {
            None
        }
    }

    /// 트레일링 스탑 설정 반환 (활성화된 경우에만 Some).
    /// 반환값: (trigger_pct, stop_pct)
    pub fn trailing_stop(&self) -> Option<(Decimal, Decimal)> {
        if self.trailing_stop.enabled {
            Some((self.trailing_stop.trigger_pct, self.trailing_stop.stop_pct))
        } else {
            None
        }
    }

    // ========================================================================
    // Signal 인리치먼트 (엔진/executor 공통)
    // ========================================================================

    /// Signal에 ExitConfig 기반 리스크 관리 정보를 적용.
    ///
    /// Entry 신호에 SL/TP 가격을 설정하고, 트레일링/수익잠금/일일한도 설정을
    /// signal.metadata에 JSON으로 직렬화하여 executor에 전달합니다.
    ///
    /// 백테스트(CandleProcessor), 페이퍼(SimulationEngine), 실거래(StrategyEngine) 모두에서
    /// 동일하게 호출되어 실행 경로에 무관하게 리스크 관리가 적용됩니다.
    pub fn enrich_signal(&self, signal: &mut Signal, entry_price: Decimal) {
        // Entry 신호에만 SL/TP 적용
        if signal.signal_type != SignalType::Entry
            && signal.signal_type != SignalType::AddToPosition
        {
            return;
        }

        let is_long = signal.side == Side::Buy;

        // 손절 가격 설정 (전략이 이미 설정한 경우 덮어쓰지 않음)
        if signal.stop_loss.is_none() && self.stop_loss.enabled {
            let sl_price = match self.stop_loss.mode {
                StopLossMode::Fixed => {
                    let pct = self.stop_loss.pct / dec!(100);
                    if is_long {
                        entry_price * (Decimal::ONE - pct)
                    } else {
                        entry_price * (Decimal::ONE + pct)
                    }
                }
                StopLossMode::AtrBased => {
                    // ATR 기반 SL은 실시간 ATR 값이 필요하므로 metadata에 설정 정보만 저장
                    // executor에서 ATR 값을 받아 실제 가격을 계산
                    if let Ok(config_json) = serde_json::to_value(&self.stop_loss) {
                        signal
                            .metadata
                            .insert("atr_stop_loss".to_string(), config_json);
                    }
                    return; // ATR 모드는 가격을 직접 설정하지 않음
                }
            };
            signal.stop_loss = Some(sl_price);
        }

        // 익절 가격 설정 (전략이 이미 설정한 경우 덮어쓰지 않음)
        if signal.take_profit.is_none() && self.take_profit.enabled {
            let pct = self.take_profit.pct / dec!(100);
            let tp_price = if is_long {
                entry_price * (Decimal::ONE + pct)
            } else {
                entry_price * (Decimal::ONE - pct)
            };
            signal.take_profit = Some(tp_price);
        }

        // 트레일링 스톱 설정을 metadata에 저장 (executor에서 실시간 처리)
        if self.trailing_stop.enabled {
            if let Ok(config_json) = serde_json::to_value(&self.trailing_stop) {
                signal
                    .metadata
                    .insert("trailing_stop".to_string(), config_json);
            }
        }

        // 수익 잠금 설정을 metadata에 저장
        if self.profit_lock.enabled {
            if let Ok(config_json) = serde_json::to_value(&self.profit_lock) {
                signal
                    .metadata
                    .insert("profit_lock".to_string(), config_json);
            }
        }

        // 일일 손실 한도 설정을 metadata에 저장
        if self.daily_loss_limit.enabled {
            if let Ok(config_json) = serde_json::to_value(&self.daily_loss_limit) {
                signal
                    .metadata
                    .insert("daily_loss_limit".to_string(), config_json);
            }
        }

        // 반대 신호 청산 설정
        if self.exit_on_opposite_signal {
            signal.metadata.insert(
                "exit_on_opposite".to_string(),
                serde_json::Value::Bool(true),
            );
        }
    }

    /// 여러 Signal에 ExitConfig를 일괄 적용.
    pub fn enrich_signals(&self, signals: &mut [Signal], entry_price: Decimal) {
        for signal in signals.iter_mut() {
            self.enrich_signal(signal, entry_price);
        }
    }

    // ========================================================================
    // 전략 유형별 프리셋
    // ========================================================================

    /// 단기 트레이딩용 프리셋.
    ///
    /// - 좁은 손절 (2%)
    /// - 적절한 익절 (4%)
    /// - 트레일링 스탑/수익잠금/일일한도 비활성화
    /// - 반대 신호 청산 활성화
    ///
    /// 적용 대상: day_trading, sector_vb, momentum_surge
    pub fn for_day_trading() -> Self {
        Self {
            stop_loss: StopLossConfig {
                enabled: true,
                mode: StopLossMode::Fixed,
                pct: dec!(2.0),
                ..Default::default()
            },
            take_profit: TakeProfitConfig {
                enabled: true,
                pct: dec!(4.0),
            },
            trailing_stop: TrailingStopConfig::default(),
            profit_lock: ProfitLockConfig::default(),
            daily_loss_limit: DailyLossLimitConfig::default(),
            exit_on_opposite_signal: true,
        }
    }

    /// 평균회귀용 프리셋.
    ///
    /// - 중간 손절 (3%)
    /// - 넓은 익절 (6%)
    /// - 트레일링 스탑/수익잠금/일일한도 비활성화
    /// - 반대 신호 청산 활성화
    ///
    /// 적용 대상: mean_reversion, range_trading, candle_pattern
    pub fn for_mean_reversion() -> Self {
        Self {
            stop_loss: StopLossConfig {
                enabled: true,
                mode: StopLossMode::Fixed,
                pct: dec!(3.0),
                ..Default::default()
            },
            take_profit: TakeProfitConfig {
                enabled: true,
                pct: dec!(6.0),
            },
            trailing_stop: TrailingStopConfig {
                trigger_pct: dec!(3.0),
                stop_pct: dec!(1.5),
                ..Default::default()
            },
            profit_lock: ProfitLockConfig::default(),
            daily_loss_limit: DailyLossLimitConfig::default(),
            exit_on_opposite_signal: true,
        }
    }

    /// 그리드/물타기용 프리셋.
    ///
    /// - 넓은 손절 (15%) - 과도한 하락 시 리스크 제한
    /// - 좁은 익절 (3%)
    /// - 트레일링 스탑/수익잠금/일일한도 비활성화
    /// - 반대 신호 청산 비활성화
    ///
    /// 적용 대상: infinity_bot, grid_trading, magic_split
    pub fn for_grid_trading() -> Self {
        Self {
            stop_loss: StopLossConfig {
                enabled: true,
                mode: StopLossMode::Fixed,
                pct: dec!(15.0),
                ..Default::default()
            },
            take_profit: TakeProfitConfig {
                enabled: true,
                pct: dec!(3.0),
            },
            trailing_stop: TrailingStopConfig::default(),
            profit_lock: ProfitLockConfig::default(),
            daily_loss_limit: DailyLossLimitConfig::default(),
            exit_on_opposite_signal: false,
        }
    }

    /// 자산배분/로테이션용 프리셋.
    ///
    /// - 손절/익절 비활성화 (리밸런싱으로 관리)
    /// - 트레일링 스탑/수익잠금/일일한도 비활성화
    /// - 반대 신호 청산 비활성화
    ///
    /// 적용 대상: asset_allocation, rotation, pension_bot
    pub fn for_rebalancing() -> Self {
        Self {
            stop_loss: StopLossConfig {
                enabled: false,
                mode: StopLossMode::Fixed,
                pct: dec!(15.0),
                ..Default::default()
            },
            take_profit: TakeProfitConfig {
                enabled: false,
                pct: dec!(30.0),
            },
            trailing_stop: TrailingStopConfig {
                trigger_pct: dec!(5.0),
                stop_pct: dec!(2.0),
                ..Default::default()
            },
            profit_lock: ProfitLockConfig::default(),
            daily_loss_limit: DailyLossLimitConfig::default(),
            exit_on_opposite_signal: false,
        }
    }

    /// 레버리지 ETF용 프리셋.
    ///
    /// - 손절 필수 (5%)
    /// - 넓은 익절 (10%)
    /// - 트레일링 스탑 활성화 (5% 트리거, 2% 스탑)
    /// - 반대 신호 청산 활성화
    ///
    /// 적용 대상: us_3x_leverage, market_bothside
    pub fn for_leverage() -> Self {
        Self {
            stop_loss: StopLossConfig {
                enabled: true,
                mode: StopLossMode::Fixed,
                pct: dec!(5.0),
                ..Default::default()
            },
            take_profit: TakeProfitConfig {
                enabled: true,
                pct: dec!(10.0),
            },
            trailing_stop: TrailingStopConfig {
                enabled: true,
                mode: TrailingMode::FixedPercentage,
                trigger_pct: dec!(5.0),
                stop_pct: dec!(2.0),
                ..Default::default()
            },
            profit_lock: ProfitLockConfig::default(),
            daily_loss_limit: DailyLossLimitConfig::default(),
            exit_on_opposite_signal: true,
        }
    }

    /// 모멘텀용 프리셋.
    ///
    /// - 중간 손절 (5%)
    /// - 넓은 익절 (15%)
    /// - 트레일링 스탑 활성화 (8% 트리거, 3% 스탑)
    /// - 반대 신호 청산 활성화
    ///
    /// 적용 대상: compound_momentum, momentum_power, rsi_multi_tf
    pub fn for_momentum() -> Self {
        Self {
            stop_loss: StopLossConfig {
                enabled: true,
                mode: StopLossMode::Fixed,
                pct: dec!(5.0),
                ..Default::default()
            },
            take_profit: TakeProfitConfig {
                enabled: true,
                pct: dec!(15.0),
            },
            trailing_stop: TrailingStopConfig {
                enabled: true,
                mode: TrailingMode::FixedPercentage,
                trigger_pct: dec!(8.0),
                stop_pct: dec!(3.0),
                ..Default::default()
            },
            profit_lock: ProfitLockConfig::default(),
            daily_loss_limit: DailyLossLimitConfig::default(),
            exit_on_opposite_signal: true,
        }
    }
}
