//! 공통 청산 설정 (모든 전략에서 사용).
//!
//! 손절/익절/트레일링 스탑 설정을 위한 공통 구조체입니다.
//! `#[fragment("risk.exit_config")]`와 함께 사용하여 UI 스키마에 리스크 관리 옵션을 추가합니다.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

/// 청산 설정 (손절/익절/트레일링 스탑).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitConfig {
    /// 손절 활성화 (기본값: true)
    #[serde(default = "default_stop_loss_enabled")]
    pub stop_loss_enabled: bool,

    /// 손절 비율 (%) (기본값: 2.0)
    #[serde(default = "default_stop_loss_pct")]
    pub stop_loss_pct: Decimal,

    /// 익절 활성화 (기본값: true)
    #[serde(default = "default_take_profit_enabled")]
    pub take_profit_enabled: bool,

    /// 익절 비율 (%) (기본값: 4.0)
    #[serde(default = "default_take_profit_pct")]
    pub take_profit_pct: Decimal,

    /// 트레일링 스톱 활성화 (기본값: false)
    #[serde(default = "default_trailing_stop_enabled")]
    pub trailing_stop_enabled: bool,

    /// 트레일링 시작 수익률 (%) (기본값: 2.0)
    #[serde(default = "default_trailing_trigger_pct")]
    pub trailing_trigger_pct: Decimal,

    /// 트레일링 스톱 비율 (%) (기본값: 1.0)
    #[serde(default = "default_trailing_stop_pct")]
    pub trailing_stop_pct: Decimal,

    /// 반대 신호 시 청산 (기본값: true)
    #[serde(default = "default_exit_on_opposite")]
    pub exit_on_opposite_signal: bool,
}

fn default_stop_loss_enabled() -> bool {
    true
}
fn default_stop_loss_pct() -> Decimal {
    dec!(2.0)
}
fn default_take_profit_enabled() -> bool {
    true
}
fn default_take_profit_pct() -> Decimal {
    dec!(4.0)
}
fn default_trailing_stop_enabled() -> bool {
    false
}
fn default_trailing_trigger_pct() -> Decimal {
    dec!(2.0)
}
fn default_trailing_stop_pct() -> Decimal {
    dec!(1.0)
}
fn default_exit_on_opposite() -> bool {
    true
}

impl Default for ExitConfig {
    fn default() -> Self {
        Self {
            stop_loss_enabled: default_stop_loss_enabled(),
            stop_loss_pct: default_stop_loss_pct(),
            take_profit_enabled: default_take_profit_enabled(),
            take_profit_pct: default_take_profit_pct(),
            trailing_stop_enabled: default_trailing_stop_enabled(),
            trailing_trigger_pct: default_trailing_trigger_pct(),
            trailing_stop_pct: default_trailing_stop_pct(),
            exit_on_opposite_signal: default_exit_on_opposite(),
        }
    }
}

impl ExitConfig {
    /// 손절 비율 반환 (활성화된 경우에만 Some).
    pub fn stop_loss(&self) -> Option<Decimal> {
        if self.stop_loss_enabled {
            Some(self.stop_loss_pct)
        } else {
            None
        }
    }

    /// 익절 비율 반환 (활성화된 경우에만 Some).
    pub fn take_profit(&self) -> Option<Decimal> {
        if self.take_profit_enabled {
            Some(self.take_profit_pct)
        } else {
            None
        }
    }

    /// 트레일링 스탑 설정 반환 (활성화된 경우에만 Some).
    pub fn trailing_stop(&self) -> Option<(Decimal, Decimal)> {
        if self.trailing_stop_enabled {
            Some((self.trailing_trigger_pct, self.trailing_stop_pct))
        } else {
            None
        }
    }

    // ============================================================================
    // 전략 유형별 프리셋 (각 전략에서 사용)
    // ============================================================================

    /// 단기 트레이딩용 프리셋.
    ///
    /// - 좁은 손절 (2%)
    /// - 적절한 익절 (4%)
    /// - 트레일링 스탑 비활성화
    /// - 반대 신호 청산 활성화
    ///
    /// 적용 대상: day_trading, sector_vb, momentum_surge
    pub fn for_day_trading() -> Self {
        Self {
            stop_loss_enabled: true,
            stop_loss_pct: dec!(2.0),
            take_profit_enabled: true,
            take_profit_pct: dec!(4.0),
            trailing_stop_enabled: false,
            trailing_trigger_pct: dec!(2.0),
            trailing_stop_pct: dec!(1.0),
            exit_on_opposite_signal: true,
        }
    }

    /// 평균회귀용 프리셋.
    ///
    /// - 중간 손절 (3%)
    /// - 넓은 익절 (6%)
    /// - 트레일링 스탑 비활성화
    /// - 반대 신호 청산 활성화
    ///
    /// 적용 대상: mean_reversion, range_trading, candle_pattern
    pub fn for_mean_reversion() -> Self {
        Self {
            stop_loss_enabled: true,
            stop_loss_pct: dec!(3.0),
            take_profit_enabled: true,
            take_profit_pct: dec!(6.0),
            trailing_stop_enabled: false,
            trailing_trigger_pct: dec!(3.0),
            trailing_stop_pct: dec!(1.5),
            exit_on_opposite_signal: true,
        }
    }

    /// 그리드/물타기용 프리셋.
    ///
    /// - 손절 활성화 (15%) - 과도한 하락 시 리스크 제한
    /// - 익절 활성화 (3%)
    /// - 트레일링 스탑 비활성화
    /// - 반대 신호 청산 비활성화
    ///
    /// 적용 대상: infinity_bot, grid_trading, magic_split
    pub fn for_grid_trading() -> Self {
        Self {
            stop_loss_enabled: true,
            stop_loss_pct: dec!(15.0), // 그리드/DCA 전략은 넓은 손절
            take_profit_enabled: true,
            take_profit_pct: dec!(3.0),
            trailing_stop_enabled: false,
            trailing_trigger_pct: dec!(2.0),
            trailing_stop_pct: dec!(1.0),
            exit_on_opposite_signal: false,
        }
    }

    /// 자산배분/로테이션용 프리셋.
    ///
    /// - 손절 비활성화 (리밸런싱으로 관리)
    /// - 익절 비활성화 (리밸런싱으로 관리)
    /// - 트레일링 스탑 비활성화
    /// - 반대 신호 청산 비활성화
    ///
    /// 적용 대상: asset_allocation, rotation, pension_bot
    pub fn for_rebalancing() -> Self {
        Self {
            stop_loss_enabled: false,
            stop_loss_pct: dec!(15.0),
            take_profit_enabled: false,
            take_profit_pct: dec!(30.0),
            trailing_stop_enabled: false,
            trailing_trigger_pct: dec!(5.0),
            trailing_stop_pct: dec!(2.0),
            exit_on_opposite_signal: false,
        }
    }

    /// 레버리지 ETF용 프리셋.
    ///
    /// - 손절 필수 (5%)
    /// - 넓은 익절 (10%)
    /// - 트레일링 스탑 활성화
    /// - 반대 신호 청산 활성화
    ///
    /// 적용 대상: us_3x_leverage, market_bothside
    pub fn for_leverage() -> Self {
        Self {
            stop_loss_enabled: true,
            stop_loss_pct: dec!(5.0),
            take_profit_enabled: true,
            take_profit_pct: dec!(10.0),
            trailing_stop_enabled: true,
            trailing_trigger_pct: dec!(5.0),
            trailing_stop_pct: dec!(2.0),
            exit_on_opposite_signal: true,
        }
    }

    /// 모멘텀용 프리셋.
    ///
    /// - 중간 손절 (5%)
    /// - 넓은 익절 (15%)
    /// - 트레일링 스탑 활성화
    /// - 반대 신호 청산 활성화
    ///
    /// 적용 대상: compound_momentum, momentum_power, rsi_multi_tf
    pub fn for_momentum() -> Self {
        Self {
            stop_loss_enabled: true,
            stop_loss_pct: dec!(5.0),
            take_profit_enabled: true,
            take_profit_pct: dec!(15.0),
            trailing_stop_enabled: true,
            trailing_trigger_pct: dec!(8.0),
            trailing_stop_pct: dec!(3.0),
            exit_on_opposite_signal: true,
        }
    }
}
