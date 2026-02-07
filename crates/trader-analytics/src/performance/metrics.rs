//! 성과 지표 계산 모듈
//!
//! 트레이딩 전략의 성과를 측정하기 위한 다양한 지표를 제공합니다:
//! - 샤프 비율 (Sharpe Ratio): 위험 대비 수익률 측정
//! - 소르티노 비율 (Sortino Ratio): 하방 위험 대비 수익률 측정
//! - 최대 낙폭 (Maximum Drawdown): 고점 대비 최대 하락폭
//! - 승률 (Win Rate): 수익 거래 비율
//! - 프로핏 팩터 (Profit Factor): 총 수익 / 총 손실 비율
//! - 기대값 (Expectancy): 거래당 기대 수익
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! use trader_analytics::performance::metrics::{PerformanceMetrics, RoundTrip};
//! use rust_decimal_macros::dec;
//!
//! // 라운드트립 거래 데이터 생성
//! let round_trips = vec![...]; // 진입-청산 완료된 거래들
//!
//! // 성과 지표 계산 (초기 자본 1,000만원)
//! let metrics = PerformanceMetrics::from_round_trips(
//!     &round_trips,
//!     dec!(10000000),
//!     Some(0.035), // 위험 무위험 이자율 3.5%
//! );
//!
//! println!("승률: {}%", metrics.win_rate_pct);
//! println!("샤프 비율: {}", metrics.sharpe_ratio);
//! ```

use chrono::{DateTime, Duration, Utc};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use trader_core::{net_pnl, realized_pnl, Side, TradeInfo, TradeStatistics};
use uuid::Uuid;

/// 연간 거래일 수 (연율화 계산에 사용)
///
/// 일반적으로 주식 시장은 연간 약 252일 거래됩니다.
/// 암호화폐 시장(365일)의 경우 이 값을 조정할 수 있습니다.
pub const TRADING_DAYS_PER_YEAR: u32 = 252;

/// 기본 무위험 이자율 (연간, 예: 0.05 = 5%)
///
/// 샤프/소르티노 비율 계산 시 사용되는 기본 무위험 수익률입니다.
/// 미국 국채 수익률이나 한국 국채 수익률 등을 참고하여 설정합니다.
pub const DEFAULT_RISK_FREE_RATE: f64 = 0.05;

/// 라운드트립 거래 (진입부터 청산까지)
///
/// 하나의 완전한 거래 사이클을 나타냅니다.
/// 포지션을 열고(진입) 닫는(청산) 전체 과정을 포함합니다.
///
/// # 예시
///
/// - 롱 포지션: BTC 50,000 USDT에 매수 → 52,000 USDT에 매도 = +2,000 USDT 수익
/// - 숏 포지션: ETH 3,000 USDT에 매도(공매도) → 2,800 USDT에 매수(청산) = +200 USDT 수익
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundTrip {
    /// 고유 식별자
    pub id: Uuid,

    /// 거래 심볼 (예: "BTC/USDT", "ETH/USDT")
    pub symbol: String,

    /// 거래 방향
    /// - `Side::Buy`: 롱 포지션 (가격 상승 시 수익)
    /// - `Side::Sell`: 숏 포지션 (가격 하락 시 수익)
    pub side: Side,

    /// 진입 가격 (포지션 오픈 시 가격)
    pub entry_price: Decimal,

    /// 청산 가격 (포지션 종료 시 가격)
    pub exit_price: Decimal,

    /// 거래 수량
    pub quantity: Decimal,

    /// 총 수수료 (진입 + 청산 수수료 합계)
    pub fees: Decimal,

    /// 실현 손익 (수수료 차감 후 순수익)
    /// 양수 = 수익, 음수 = 손실
    pub pnl: Decimal,

    /// 수익률 (백분율)
    /// 예: 4.0 = 4% 수익
    pub return_pct: Decimal,

    /// 진입 시각 (UTC 기준)
    pub entry_time: DateTime<Utc>,

    /// 청산 시각 (UTC 기준)
    pub exit_time: DateTime<Utc>,

    /// 전략 ID (해당 거래를 생성한 전략 식별자)
    pub strategy_id: Option<String>,

    /// 진입 이유 (전략의 논리적 목적)
    /// 예: "RSI 과매도 진입", "그리드 레벨 1 매수"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_reason: Option<String>,

    /// 청산 이유 (전략의 논리적 목적)
    /// 예: "익절 청산 (4%)", "손절 청산 (2%)"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_reason: Option<String>,
}

impl RoundTrip {
    /// 새로운 라운드트립 거래를 생성합니다.
    ///
    /// 진입/청산 가격과 방향을 기반으로 PnL과 수익률을 자동 계산합니다.
    ///
    /// # 매개변수
    ///
    /// * `symbol` - 거래 심볼 (예: "BTC/USDT")
    /// * `side` - 거래 방향 (Buy = 롱, Sell = 숏)
    /// * `entry_price` - 진입 가격
    /// * `exit_price` - 청산 가격
    /// * `quantity` - 거래 수량
    /// * `fees` - 총 수수료
    /// * `entry_time` - 진입 시각
    /// * `exit_time` - 청산 시각
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// let trade = RoundTrip::new(
    ///     "BTC/USDT",
    ///     Side::Buy,
    ///     dec!(50000),  // 진입가
    ///     dec!(52000),  // 청산가
    ///     dec!(0.1),    // 수량 (0.1 BTC)
    ///     dec!(10),     // 수수료
    ///     entry_time,
    ///     exit_time,
    /// );
    /// // PnL = (52000 - 50000) * 0.1 - 10 = 190 USDT
    /// ```
    pub fn new(
        symbol: impl Into<String>,
        side: Side,
        entry_price: Decimal,
        exit_price: Decimal,
        quantity: Decimal,
        fees: Decimal,
        entry_time: DateTime<Utc>,
        exit_time: DateTime<Utc>,
    ) -> Self {
        // 공통 계산 모듈 사용 (레거시 제거)
        let gross_pnl = realized_pnl(entry_price, exit_price, quantity, side);
        let pnl = net_pnl(gross_pnl, fees);
        let return_pct = Self::calculate_return_pct(side, entry_price, exit_price);

        Self {
            id: Uuid::new_v4(),
            symbol: symbol.into(),
            side,
            entry_price,
            exit_price,
            quantity,
            fees,
            pnl,
            return_pct,
            entry_time,
            exit_time,
            strategy_id: None,
            entry_reason: None,
            exit_reason: None,
        }
    }

    /// 전략 ID를 설정합니다.
    ///
    /// # 예시
    /// ```rust,ignore
    /// let trade = RoundTrip::new(...).with_strategy("grid_trading_v1");
    /// ```
    pub fn with_strategy(mut self, strategy_id: impl Into<String>) -> Self {
        self.strategy_id = Some(strategy_id.into());
        self
    }

    /// 진입/청산 이유를 설정합니다.
    ///
    /// # 예시
    /// ```rust,ignore
    /// let trade = RoundTrip::new(...)
    ///     .with_reasons(Some("RSI 과매도 진입"), Some("익절 청산 (4%)"));
    /// ```
    pub fn with_reasons(
        mut self,
        entry_reason: Option<String>,
        exit_reason: Option<String>,
    ) -> Self {
        self.entry_reason = entry_reason;
        self.exit_reason = exit_reason;
        self
    }

    /// 수익률을 백분율로 계산합니다.
    ///
    /// ## 계산 공식
    ///
    /// - **롱**: ((청산가 - 진입가) / 진입가) × 100
    /// - **숏**: ((진입가 - 청산가) / 진입가) × 100
    ///
    /// ## 참고
    ///
    /// 수수료는 수익률 계산에 포함되지 않습니다 (순수 가격 변동만 반영).
    fn calculate_return_pct(side: Side, entry_price: Decimal, exit_price: Decimal) -> Decimal {
        if entry_price.is_zero() {
            return Decimal::ZERO;
        }

        match side {
            Side::Buy => ((exit_price - entry_price) / entry_price) * Decimal::from(100),
            Side::Sell => ((entry_price - exit_price) / entry_price) * Decimal::from(100),
        }
    }

    /// 이 거래가 수익 거래인지 확인합니다.
    ///
    /// # 반환값
    ///
    /// - `true`: PnL > 0 (수익)
    /// - `false`: PnL <= 0 (손실 또는 본전)
    pub fn is_winner(&self) -> bool {
        self.pnl > Decimal::ZERO
    }

    /// 포지션 보유 기간을 반환합니다.
    ///
    /// 진입 시각부터 청산 시각까지의 시간 차이입니다.
    pub fn holding_duration(&self) -> Duration {
        self.exit_time - self.entry_time
    }

    /// 포지션 보유 기간을 시간(hours) 단위로 반환합니다.
    ///
    /// # 예시
    ///
    /// 2일 동안 보유했다면 48.0을 반환합니다.
    pub fn holding_hours(&self) -> Decimal {
        let seconds = self.holding_duration().num_seconds();
        Decimal::from(seconds) / Decimal::from(3600)
    }

    /// 진입 시점의 명목 가치(notional value)를 반환합니다.
    ///
    /// 명목 가치 = 진입가 × 수량
    ///
    /// 실제 투자된 금액의 크기를 나타냅니다.
    pub fn entry_notional(&self) -> Decimal {
        self.entry_price * self.quantity
    }
}

/// TradeInfo trait 구현 (공통 통계 계산용)
impl TradeInfo for RoundTrip {
    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn pnl(&self) -> Option<Decimal> {
        // RoundTrip은 항상 청산 완료된 거래
        Some(self.pnl)
    }

    fn fees(&self) -> Decimal {
        self.fees
    }

    fn entry_time(&self) -> DateTime<Utc> {
        self.entry_time
    }

    fn exit_time(&self) -> Option<DateTime<Utc>> {
        Some(self.exit_time)
    }
}

/// 트레이딩 성과 지표
///
/// 백테스트나 실거래의 성과를 종합적으로 평가하기 위한 지표 모음입니다.
/// 수익성, 위험도, 효율성을 다각도로 분석합니다.
///
/// # 주요 지표 설명
///
/// ## 수익성 지표
/// - `total_return_pct`: 전체 수익률
/// - `annualized_return_pct`: 연율화 수익률
/// - `net_profit`: 순수익 (총수익 - 총손실 - 수수료)
///
/// ## 위험 지표
/// - `max_drawdown_pct`: 최대 낙폭 (고점 대비 최대 하락률)
/// - `sharpe_ratio`: 샤프 비율 (위험 대비 수익)
/// - `sortino_ratio`: 소르티노 비율 (하방 위험 대비 수익)
///
/// ## 거래 효율성 지표
/// - `win_rate_pct`: 승률
/// - `profit_factor`: 프로핏 팩터 (총수익/총손실)
/// - `expectancy`: 거래당 기대 수익
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// 총 수익률 (%)
    ///
    /// (순수익 / 초기자본) × 100
    pub total_return_pct: Decimal,

    /// 연율화 수익률 (%)
    ///
    /// 거래 기간을 1년으로 환산했을 때의 수익률입니다.
    /// 252 거래일 기준으로 계산됩니다.
    pub annualized_return_pct: Decimal,

    /// 샤프 비율 (Sharpe Ratio)
    ///
    /// 위험(변동성) 대비 초과 수익을 측정합니다.
    ///
    /// 공식: (평균 수익률 - 무위험 이자율) / 수익률 표준편차 × √252
    ///
    /// - 1.0 이상: 양호
    /// - 2.0 이상: 우수
    /// - 3.0 이상: 매우 우수
    pub sharpe_ratio: Decimal,

    /// 소르티노 비율 (Sortino Ratio)
    ///
    /// 하방 위험(손실 변동성)만 고려한 위험 조정 수익률입니다.
    /// 샤프 비율과 달리 손실만 위험으로 간주합니다.
    ///
    /// 공식: (평균 수익률 - 무위험 이자율) / 하방 편차 × √252
    ///
    /// 일반적으로 샤프 비율보다 높게 나옵니다.
    pub sortino_ratio: Decimal,

    /// 최대 낙폭 (Maximum Drawdown) (%)
    ///
    /// 고점에서 저점까지의 최대 하락폭입니다.
    /// 투자 기간 중 겪을 수 있는 최대 손실을 나타냅니다.
    ///
    /// 예: 10% = 자산이 고점에서 최대 10% 하락했음
    ///
    /// - 10% 이하: 보수적
    /// - 20% 이하: 적정
    /// - 30% 이상: 공격적/위험
    pub max_drawdown_pct: Decimal,

    /// 승률 (%)
    ///
    /// 수익 거래 수 / 총 거래 수 × 100
    ///
    /// 그리드 전략: 70-80% 목표
    /// 추세추종 전략: 40-50%도 가능 (평균 수익 > 평균 손실이면)
    pub win_rate_pct: Decimal,

    /// 프로핏 팩터 (Profit Factor)
    ///
    /// 총 수익 / 총 손실 비율입니다.
    ///
    /// - 1.0 미만: 손실
    /// - 1.0: 본전
    /// - 1.5 이상: 양호
    /// - 2.0 이상: 우수
    pub profit_factor: Decimal,

    /// 총 거래 횟수
    pub total_trades: usize,

    /// 수익 거래 횟수
    pub winning_trades: usize,

    /// 손실 거래 횟수
    pub losing_trades: usize,

    /// 평균 수익 (수익 거래만)
    ///
    /// 총 수익 / 수익 거래 횟수
    pub avg_win: Decimal,

    /// 평균 손실 (손실 거래만)
    ///
    /// 총 손실 / 손실 거래 횟수
    pub avg_loss: Decimal,

    /// 최대 수익 거래
    ///
    /// 단일 거래에서의 최대 수익액
    pub largest_win: Decimal,

    /// 최대 손실 거래
    ///
    /// 단일 거래에서의 최대 손실액 (양수로 표시)
    pub largest_loss: Decimal,

    /// 평균 보유 기간 (시간)
    ///
    /// 포지션을 보유한 평균 시간입니다.
    pub avg_holding_hours: Decimal,

    /// 총 수익 (수익 거래 합계)
    pub gross_profit: Decimal,

    /// 총 손실 (손실 거래 합계, 양수로 표시)
    pub gross_loss: Decimal,

    /// 총 수수료
    pub total_fees: Decimal,

    /// 순수익
    ///
    /// 총 수익 - 총 손실
    /// (수수료는 각 거래의 PnL에 이미 반영됨)
    pub net_profit: Decimal,

    /// 칼마 비율 (Calmar Ratio)
    ///
    /// 연율화 수익률 / 최대 낙폭
    ///
    /// 낙폭 대비 수익률을 측정합니다.
    /// 높을수록 효율적인 위험-수익 관계입니다.
    pub calmar_ratio: Decimal,

    /// 회복 계수 (Recovery Factor)
    ///
    /// 순수익 / 최대 낙폭 금액
    ///
    /// 낙폭을 얼마나 빨리 회복하는지 나타냅니다.
    pub recovery_factor: Decimal,

    /// 거래당 평균 수익률 (%)
    pub avg_return_per_trade: Decimal,

    /// 기대값 (Expectancy)
    ///
    /// 거래당 기대 수익입니다.
    ///
    /// 공식: (승률 × 평균 수익) - (패률 × 평균 손실)
    ///
    /// 양수: 장기적으로 수익
    /// 음수: 장기적으로 손실
    pub expectancy: Decimal,
}

impl PerformanceMetrics {
    /// 라운드트립 거래 목록에서 성과 지표를 계산합니다.
    ///
    /// # 매개변수
    ///
    /// * `round_trips` - 완료된 라운드트립 거래 목록
    /// * `initial_capital` - 초기 자본금 (수익률 계산용)
    /// * `risk_free_rate` - 연간 무위험 이자율 (기본값: 5%)
    ///   - 한국 국채 수익률 참고 시 약 3.5% 사용
    ///   - None이면 기본값 5% 사용
    ///
    /// # 반환값
    ///
    /// 모든 성과 지표가 계산된 `PerformanceMetrics` 구조체
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// let metrics = PerformanceMetrics::from_round_trips(
    ///     &completed_trades,
    ///     dec!(10_000_000),  // 초기 자본 1천만원
    ///     Some(0.035),       // 무위험 이자율 3.5%
    /// );
    ///
    /// if metrics.is_profitable() && metrics.sharpe_ratio > dec!(1.5) {
    ///     println!("전략 성과가 양호합니다!");
    /// }
    /// ```
    pub fn from_round_trips(
        round_trips: &[RoundTrip],
        initial_capital: Decimal,
        risk_free_rate: Option<f64>,
    ) -> Self {
        // 거래가 없으면 기본값 반환
        if round_trips.is_empty() {
            return Self::default();
        }

        let rf_rate = risk_free_rate.unwrap_or(DEFAULT_RISK_FREE_RATE);

        // === 공통 통계 모듈 사용 ===
        let stats = TradeStatistics::from_trades(round_trips);

        // === PerformanceMetrics 전용 데이터 수집 ===
        let mut gross_profit = Decimal::ZERO;
        let mut gross_loss = Decimal::ZERO;
        let mut returns: Vec<Decimal> = Vec::with_capacity(round_trips.len());

        for rt in round_trips {
            returns.push(rt.return_pct);

            if rt.pnl > Decimal::ZERO {
                gross_profit += rt.pnl;
            } else if rt.pnl < Decimal::ZERO {
                gross_loss += rt.pnl.abs();
            }
        }

        let net_profit = gross_profit - gross_loss;

        // TradeStatistics의 Duration을 Decimal hours로 변환
        let avg_holding_hours = Decimal::from(stats.avg_holding_period.num_hours())
            + Decimal::from(stats.avg_holding_period.num_minutes() % 60) / Decimal::from(60);

        // === 총 수익률 ===
        let total_return_pct = if initial_capital > Decimal::ZERO {
            (net_profit / initial_capital) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        // === 거래 기간 계산 ===
        let trading_days = Self::calculate_trading_days(round_trips);

        // === 연율화 수익률 ===
        let annualized_return_pct = Self::annualize_return(total_return_pct, trading_days);

        // === 자산 곡선 구축 (낙폭 계산용) ===
        let equity_curve = Self::build_equity_curve(round_trips, initial_capital);

        // === 최대 낙폭 ===
        let max_drawdown_pct = Self::calculate_max_drawdown(&equity_curve);

        // === 샤프 비율 ===
        let sharpe_ratio = Self::calculate_sharpe_ratio(&returns, rf_rate, trading_days);

        // === 소르티노 비율 ===
        let sortino_ratio = Self::calculate_sortino_ratio(&returns, rf_rate, trading_days);

        // === 칼마 비율 ===
        let calmar_ratio = if max_drawdown_pct > Decimal::ZERO {
            annualized_return_pct / max_drawdown_pct
        } else {
            Decimal::ZERO
        };

        // === 회복 계수 ===
        let max_drawdown_value =
            if initial_capital > Decimal::ZERO && max_drawdown_pct > Decimal::ZERO {
                initial_capital * max_drawdown_pct / Decimal::from(100)
            } else {
                Decimal::ZERO
            };

        let recovery_factor = if max_drawdown_value > Decimal::ZERO {
            net_profit / max_drawdown_value
        } else {
            Decimal::ZERO
        };

        // === 거래당 평균 수익률 ===
        let avg_return_per_trade = if stats.total_trades > 0 {
            returns.iter().copied().sum::<Decimal>() / Decimal::from(stats.total_trades)
        } else {
            Decimal::ZERO
        };

        Self {
            total_return_pct,
            annualized_return_pct,
            sharpe_ratio,
            sortino_ratio,
            max_drawdown_pct,
            win_rate_pct: stats.win_rate_pct,
            profit_factor: stats.profit_factor,
            total_trades: stats.total_trades,
            winning_trades: stats.winning_trades,
            losing_trades: stats.losing_trades,
            avg_win: stats.avg_win,
            avg_loss: stats.avg_loss,
            largest_win: stats.largest_win,
            largest_loss: stats.largest_loss,
            avg_holding_hours,
            gross_profit,
            gross_loss,
            total_fees: stats.total_fees,
            net_profit,
            calmar_ratio,
            recovery_factor,
            avg_return_per_trade,
            expectancy: stats.expectancy,
        }
    }

    /// 라운드트립 거래의 총 거래일 수를 계산합니다.
    ///
    /// 첫 번째 진입부터 마지막 청산까지의 일수를 반환합니다.
    fn calculate_trading_days(round_trips: &[RoundTrip]) -> u32 {
        if round_trips.is_empty() {
            return 0;
        }

        let first_entry = round_trips.iter().map(|rt| rt.entry_time).min().unwrap();
        let last_exit = round_trips.iter().map(|rt| rt.exit_time).max().unwrap();

        let duration = last_exit - first_entry;
        

        duration.num_days().max(1) as u32
    }

    /// 수익률을 연율화합니다.
    ///
    /// # 계산 방법
    ///
    /// 정확한 공식: (1 + r)^(252/days) - 1
    ///
    /// 여기서는 선형 근사를 사용합니다: r × (252 / days)
    /// (소규모 수익률에서 충분히 정확함)
    fn annualize_return(total_return_pct: Decimal, trading_days: u32) -> Decimal {
        if trading_days == 0 {
            return Decimal::ZERO;
        }

        let return_ratio = total_return_pct / Decimal::from(100);
        let days_ratio = Decimal::from(TRADING_DAYS_PER_YEAR) / Decimal::from(trading_days);
        let annualized_ratio = return_ratio * days_ratio;

        annualized_ratio * Decimal::from(100)
    }

    /// 자산 곡선(equity curve)을 구축합니다.
    ///
    /// 각 거래 후의 자산 가치를 순서대로 나열합니다.
    /// 최대 낙폭 계산에 사용됩니다.
    fn build_equity_curve(round_trips: &[RoundTrip], initial_capital: Decimal) -> Vec<Decimal> {
        let mut equity = initial_capital;
        let mut curve = vec![equity];

        // 청산 시간 순으로 정렬
        let mut sorted_trips: Vec<_> = round_trips.iter().collect();
        sorted_trips.sort_by_key(|rt| rt.exit_time);

        for rt in sorted_trips {
            equity += rt.pnl;
            curve.push(equity);
        }

        curve
    }

    /// 자산 곡선에서 최대 낙폭(MDD)을 계산합니다.
    ///
    /// # 계산 공식
    ///
    /// MDD = (고점 - 저점) / 고점 × 100%
    ///
    /// # 예시
    ///
    /// 자산이 1000만원 → 1200만원(고점) → 1080만원(저점) → 1300만원
    /// MDD = (1200 - 1080) / 1200 × 100 = 10%
    pub fn calculate_max_drawdown(equity_curve: &[Decimal]) -> Decimal {
        if equity_curve.is_empty() {
            return Decimal::ZERO;
        }

        let mut max_drawdown = Decimal::ZERO;
        let mut peak = equity_curve[0];

        for &equity in equity_curve {
            // 새로운 고점 갱신
            if equity > peak {
                peak = equity;
            }

            // 현재 낙폭 계산
            if peak > Decimal::ZERO {
                let drawdown = (peak - equity) / peak * Decimal::from(100);
                if drawdown > max_drawdown {
                    max_drawdown = drawdown;
                }
            }
        }

        max_drawdown
    }

    /// 샤프 비율을 계산합니다.
    ///
    /// 샤프 비율은 위험(표준편차) 한 단위당 초과 수익을 측정합니다.
    ///
    /// # 계산 공식
    ///
    /// Sharpe = (평균 수익률 - 무위험 이자율) / 표준편차 × √(연간 거래일)
    ///
    /// # 매개변수
    ///
    /// * `returns` - 각 거래의 수익률 목록 (백분율)
    /// * `risk_free_rate` - 연간 무위험 이자율 (예: 0.05 = 5%)
    /// * `trading_days` - 거래 기간 (일)
    ///
    /// # 해석
    ///
    /// - 1.0 이상: 양호 (위험 대비 적절한 수익)
    /// - 2.0 이상: 우수 (위험 대비 높은 수익)
    /// - 3.0 이상: 매우 우수 (헤지펀드 수준)
    pub fn calculate_sharpe_ratio(
        returns: &[Decimal],
        risk_free_rate: f64,
        trading_days: u32,
    ) -> Decimal {
        // 최소 2개의 수익률 데이터 필요 (표준편차 계산용)
        if returns.len() < 2 {
            return Decimal::ZERO;
        }

        let n = Decimal::from(returns.len());

        // 백분율을 비율로 변환 (1% → 0.01)
        let return_ratios: Vec<Decimal> = returns.iter().map(|r| *r / Decimal::from(100)).collect();

        // 평균 수익률
        let mean_return = return_ratios.iter().copied().sum::<Decimal>() / n;

        // 일일 무위험 이자율
        let daily_rf = Decimal::from_f64(risk_free_rate / TRADING_DAYS_PER_YEAR as f64)
            .unwrap_or(Decimal::ZERO);

        // 초과 수익률 (평균 - 무위험)
        let excess_return = mean_return - daily_rf;

        // 분산 계산: Σ(ri - mean)² / (n-1)
        let variance = return_ratios
            .iter()
            .map(|r| (*r - mean_return).powi(2))
            .sum::<Decimal>()
            / (n - Decimal::ONE);

        // 표준편차 = √분산
        let std_dev = Self::decimal_sqrt(variance);

        if std_dev.is_zero() {
            return Decimal::ZERO;
        }

        // 연율화 계수: √(거래일)
        let annualization =
            Self::decimal_sqrt(Decimal::from(trading_days.min(TRADING_DAYS_PER_YEAR)));

        (excess_return / std_dev) * annualization
    }

    /// 소르티노 비율을 계산합니다.
    ///
    /// 소르티노 비율은 하방 편차(손실만)를 위험으로 사용합니다.
    /// 샤프 비율과 달리 수익 변동성은 위험으로 간주하지 않습니다.
    ///
    /// # 계산 공식
    ///
    /// Sortino = (평균 수익률 - 무위험 이자율) / 하방 편차 × √(연간 거래일)
    ///
    /// 여기서 하방 편차 = √(음수 수익률²의 평균)
    ///
    /// # 해석
    ///
    /// 일반적으로 샤프 비율보다 높게 나옵니다 (수익 변동은 제외하므로).
    /// 손실 위험 대비 수익을 더 정확하게 측정합니다.
    pub fn calculate_sortino_ratio(
        returns: &[Decimal],
        risk_free_rate: f64,
        trading_days: u32,
    ) -> Decimal {
        if returns.len() < 2 {
            return Decimal::ZERO;
        }

        let n = Decimal::from(returns.len());

        // 비율로 변환
        let return_ratios: Vec<Decimal> = returns.iter().map(|r| *r / Decimal::from(100)).collect();

        // 평균 수익률
        let mean_return = return_ratios.iter().copied().sum::<Decimal>() / n;

        // 일일 무위험 이자율
        let daily_rf = Decimal::from_f64(risk_free_rate / TRADING_DAYS_PER_YEAR as f64)
            .unwrap_or(Decimal::ZERO);

        let excess_return = mean_return - daily_rf;

        // 하방 편차: 음수 수익률만 사용
        let downside_squared_sum: Decimal = return_ratios
            .iter()
            .filter(|&&r| r < Decimal::ZERO)
            .map(|r| r.powi(2))
            .sum();

        let negative_count = return_ratios.iter().filter(|&&r| r < Decimal::ZERO).count();

        // 음수 수익률이 없으면 (전부 수익)
        if negative_count == 0 {
            return if excess_return > Decimal::ZERO {
                Decimal::MAX // 하방 위험 없이 초과 수익 → 무한대
            } else {
                Decimal::ZERO
            };
        }

        let downside_variance = downside_squared_sum / Decimal::from(negative_count);
        let downside_dev = Self::decimal_sqrt(downside_variance);

        if downside_dev.is_zero() {
            return Decimal::ZERO;
        }

        let annualization =
            Self::decimal_sqrt(Decimal::from(trading_days.min(TRADING_DAYS_PER_YEAR)));

        (excess_return / downside_dev) * annualization
    }

    /// Decimal 타입의 제곱근을 뉴턴 방법으로 계산합니다.
    ///
    /// # 알고리즘
    ///
    /// 뉴턴-랩슨 방법 사용:
    /// 1. 초기 추정값 = value / 2
    /// 2. 반복: next = (guess + value/guess) / 2
    /// 3. 수렴할 때까지 반복 (최대 50회)
    ///
    /// # 정밀도
    ///
    /// 10^-10까지 정밀하게 계산합니다.
    fn decimal_sqrt(value: Decimal) -> Decimal {
        if value <= Decimal::ZERO {
            return Decimal::ZERO;
        }

        // 초기 추정값
        let mut guess = value / Decimal::TWO;
        let precision = Decimal::new(1, 10); // 0.0000000001

        for _ in 0..50 {
            let next_guess = (guess + value / guess) / Decimal::TWO;
            if (next_guess - guess).abs() < precision {
                return next_guess;
            }
            guess = next_guess;
        }

        guess
    }

    /// 전략이 수익성이 있는지 확인합니다.
    ///
    /// # 조건
    ///
    /// - 순수익 > 0
    /// - 프로핏 팩터 > 1
    pub fn is_profitable(&self) -> bool {
        self.net_profit > Decimal::ZERO && self.profit_factor > Decimal::ONE
    }

    /// 위험이 허용 범위 내인지 확인합니다.
    ///
    /// # 매개변수
    ///
    /// * `max_drawdown_limit` - 허용 최대 낙폭 (예: 20%)
    /// * `min_sharpe` - 최소 샤프 비율 (예: 1.0)
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// if metrics.has_acceptable_risk(dec!(20), dec!(1.5)) {
    ///     println!("위험 수준 적정");
    /// }
    /// ```
    pub fn has_acceptable_risk(&self, max_drawdown_limit: Decimal, min_sharpe: Decimal) -> bool {
        self.max_drawdown_pct <= max_drawdown_limit && self.sharpe_ratio >= min_sharpe
    }

    /// 성과 요약을 문자열로 반환합니다.
    ///
    /// 대시보드나 로그 출력용 한 줄 요약입니다.
    pub fn summary(&self) -> String {
        format!(
            "거래: {} | 승률: {:.1}% | PF: {:.2} | 샤프: {:.2} | MDD: {:.1}% | 순익: {:.2}",
            self.total_trades,
            self.win_rate_pct,
            self.profit_factor,
            self.sharpe_ratio,
            self.max_drawdown_pct,
            self.net_profit
        )
    }
}

/// 롤링(이동) 성과 지표 추적기
///
/// 실시간 스트리밍 환경에서 최근 N개 거래의 성과를 추적합니다.
/// 새 거래가 추가되면 가장 오래된 거래가 자동으로 제거됩니다.
///
/// # 사용 사례
///
/// - 실시간 대시보드에서 최근 100거래 성과 표시
/// - 롤링 샤프 비율 모니터링
/// - 실시간 낙폭 추적
///
/// # 예시
///
/// ```rust,ignore
/// let mut rolling = RollingMetrics::new(100, dec!(10_000_000)); // 최근 100거래
///
/// // 거래 결과 추가
/// rolling.add_return(dec!(1.5), dec!(10_150_000)); // 1.5% 수익, 새 자산가치
///
/// println!("롤링 승률: {}%", rolling.win_rate());
/// println!("현재 낙폭: {}%", rolling.current_drawdown());
/// ```
#[derive(Debug, Clone)]
pub struct RollingMetrics {
    /// 윈도우 크기 (추적할 최대 거래 수)
    window_size: usize,

    /// 최근 수익률 목록 (백분율)
    returns: VecDeque<Decimal>,

    /// 자산 곡선 (최근 자산 가치들)
    equity_curve: VecDeque<Decimal>,

    /// 현재 고점 자산 가치
    peak_equity: Decimal,

    /// 현재 낙폭 (%)
    current_drawdown: Decimal,

    /// 관측 기간 중 최대 낙폭 (%)
    max_drawdown: Decimal,

    /// 수익률 합계 (평균 계산용)
    return_sum: Decimal,

    /// 수익률 제곱 합계 (표준편차 계산용)
    return_sq_sum: Decimal,

    /// 윈도우 내 수익 거래 수
    wins: usize,

    /// 윈도우 내 손실 거래 수
    losses: usize,
}

impl RollingMetrics {
    /// 새로운 롤링 지표 추적기를 생성합니다.
    ///
    /// # 매개변수
    ///
    /// * `window_size` - 추적할 최대 거래 수
    /// * `initial_equity` - 초기 자산 가치
    pub fn new(window_size: usize, initial_equity: Decimal) -> Self {
        Self {
            window_size,
            returns: VecDeque::with_capacity(window_size),
            equity_curve: VecDeque::from([initial_equity]),
            peak_equity: initial_equity,
            current_drawdown: Decimal::ZERO,
            max_drawdown: Decimal::ZERO,
            return_sum: Decimal::ZERO,
            return_sq_sum: Decimal::ZERO,
            wins: 0,
            losses: 0,
        }
    }

    /// 새로운 수익률 데이터를 추가합니다.
    ///
    /// 윈도우가 가득 차면 가장 오래된 데이터가 자동 제거됩니다.
    ///
    /// # 매개변수
    ///
    /// * `return_pct` - 수익률 (백분율, 예: 1.5 = 1.5%)
    /// * `new_equity` - 거래 후 새 자산 가치
    pub fn add_return(&mut self, return_pct: Decimal, new_equity: Decimal) {
        // 윈도우 크기 초과 시 가장 오래된 데이터 제거
        if self.returns.len() >= self.window_size {
            if let Some(old_return) = self.returns.pop_front() {
                self.return_sum -= old_return;
                self.return_sq_sum -= old_return.powi(2);
                if old_return > Decimal::ZERO {
                    self.wins = self.wins.saturating_sub(1);
                } else if old_return < Decimal::ZERO {
                    self.losses = self.losses.saturating_sub(1);
                }
            }
            self.equity_curve.pop_front();
        }

        // 새 데이터 추가
        self.returns.push_back(return_pct);
        self.return_sum += return_pct;
        self.return_sq_sum += return_pct.powi(2);

        if return_pct > Decimal::ZERO {
            self.wins += 1;
        } else if return_pct < Decimal::ZERO {
            self.losses += 1;
        }

        // 자산 곡선 업데이트
        self.equity_curve.push_back(new_equity);

        // 낙폭 업데이트
        if new_equity > self.peak_equity {
            self.peak_equity = new_equity;
        }

        if self.peak_equity > Decimal::ZERO {
            self.current_drawdown =
                (self.peak_equity - new_equity) / self.peak_equity * Decimal::from(100);
            if self.current_drawdown > self.max_drawdown {
                self.max_drawdown = self.current_drawdown;
            }
        }
    }

    /// 롤링 평균 수익률을 반환합니다 (백분율).
    pub fn mean_return(&self) -> Decimal {
        if self.returns.is_empty() {
            return Decimal::ZERO;
        }
        self.return_sum / Decimal::from(self.returns.len())
    }

    /// 롤링 수익률 표준편차를 반환합니다 (백분율).
    pub fn std_dev(&self) -> Decimal {
        if self.returns.len() < 2 {
            return Decimal::ZERO;
        }

        let n = Decimal::from(self.returns.len());
        let mean = self.mean_return();
        // 분산 = E[X²] - E[X]²
        let variance = (self.return_sq_sum / n) - mean.powi(2);

        PerformanceMetrics::decimal_sqrt(variance.max(Decimal::ZERO))
    }

    /// 롤링 샤프 비율을 반환합니다.
    ///
    /// # 매개변수
    ///
    /// * `risk_free_rate` - 연간 무위험 이자율
    pub fn sharpe_ratio(&self, risk_free_rate: f64) -> Decimal {
        let std = self.std_dev();
        if std.is_zero() {
            return Decimal::ZERO;
        }

        let daily_rf = Decimal::from_f64(risk_free_rate / TRADING_DAYS_PER_YEAR as f64)
            .unwrap_or(Decimal::ZERO);
        let excess = self.mean_return() / Decimal::from(100) - daily_rf;
        let annualization = PerformanceMetrics::decimal_sqrt(Decimal::from(TRADING_DAYS_PER_YEAR));

        (excess / (std / Decimal::from(100))) * annualization
    }

    /// 롤링 승률을 반환합니다 (백분율).
    pub fn win_rate(&self) -> Decimal {
        let total = self.wins + self.losses;
        if total == 0 {
            return Decimal::ZERO;
        }
        Decimal::from(self.wins) / Decimal::from(total) * Decimal::from(100)
    }

    /// 현재 낙폭을 반환합니다 (백분율).
    ///
    /// 현재 자산이 고점 대비 얼마나 하락했는지를 나타냅니다.
    pub fn current_drawdown(&self) -> Decimal {
        self.current_drawdown
    }

    /// 관측 기간 중 최대 낙폭을 반환합니다 (백분율).
    pub fn max_drawdown(&self) -> Decimal {
        self.max_drawdown
    }

    /// 현재 윈도우에 있는 데이터 수를 반환합니다.
    pub fn count(&self) -> usize {
        self.returns.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    /// 테스트용 라운드트립 거래 데이터 생성
    fn create_test_round_trips() -> Vec<RoundTrip> {
        let base_time = Utc::now() - Duration::days(30);

        vec![
            // 수익 거래 1: BTC 롱, +190 USDT
            RoundTrip::new(
                "BTC/USDT",
                Side::Buy,
                dec!(50000),
                dec!(52000),
                dec!(0.1),
                dec!(10),
                base_time,
                base_time + Duration::days(2),
            ),
            // 손실 거래: BTC 롱, -110 USDT
            RoundTrip::new(
                "BTC/USDT",
                Side::Buy,
                dec!(52000),
                dec!(51000),
                dec!(0.1),
                dec!(10),
                base_time + Duration::days(3),
                base_time + Duration::days(5),
            ),
            // 수익 거래 2: BTC 롱, +290 USDT
            RoundTrip::new(
                "BTC/USDT",
                Side::Buy,
                dec!(51000),
                dec!(54000),
                dec!(0.1),
                dec!(10),
                base_time + Duration::days(6),
                base_time + Duration::days(10),
            ),
            // 수익 거래 3: ETH 롱, +195 USDT
            RoundTrip::new(
                "ETH/USDT",
                Side::Buy,
                dec!(3000),
                dec!(3200),
                dec!(1.0),
                dec!(5),
                base_time + Duration::days(11),
                base_time + Duration::days(15),
            ),
            // 수익 거래 4: ETH 숏, +95 USDT
            RoundTrip::new(
                "ETH/USDT",
                Side::Sell,
                dec!(3200),
                dec!(3100),
                dec!(1.0),
                dec!(5),
                base_time + Duration::days(16),
                base_time + Duration::days(20),
            ),
        ]
    }

    #[test]
    fn test_round_trip_pnl() {
        let rt = RoundTrip::new(
            "BTC/USDT",
            Side::Buy,
            dec!(50000),
            dec!(52000),
            dec!(0.1),
            dec!(10),
            Utc::now(),
            Utc::now() + Duration::hours(24),
        );

        // PnL = (52000 - 50000) * 0.1 - 10 = 200 - 10 = 190
        assert_eq!(rt.pnl, dec!(190));
        assert!(rt.is_winner());
    }

    #[test]
    fn test_round_trip_short() {
        let rt = RoundTrip::new(
            "BTC/USDT",
            Side::Sell,
            dec!(50000),
            dec!(48000),
            dec!(0.1),
            dec!(10),
            Utc::now(),
            Utc::now() + Duration::hours(12),
        );

        // 숏 PnL = (50000 - 48000) * 0.1 - 10 = 200 - 10 = 190
        assert_eq!(rt.pnl, dec!(190));
        assert!(rt.is_winner());
    }

    #[test]
    fn test_round_trip_losing() {
        let rt = RoundTrip::new(
            "BTC/USDT",
            Side::Buy,
            dec!(50000),
            dec!(49000),
            dec!(0.1),
            dec!(10),
            Utc::now(),
            Utc::now() + Duration::hours(6),
        );

        // PnL = (49000 - 50000) * 0.1 - 10 = -100 - 10 = -110
        assert_eq!(rt.pnl, dec!(-110));
        assert!(!rt.is_winner());
    }

    #[test]
    fn test_performance_metrics_basic() {
        let round_trips = create_test_round_trips();
        let metrics = PerformanceMetrics::from_round_trips(&round_trips, dec!(10000), None);

        assert_eq!(metrics.total_trades, 5);
        assert_eq!(metrics.winning_trades, 4);
        assert_eq!(metrics.losing_trades, 1);

        // 승률 = 4/5 * 100 = 80%
        assert_eq!(metrics.win_rate_pct, dec!(80));

        assert!(metrics.profit_factor > Decimal::ONE);
        assert!(metrics.net_profit > Decimal::ZERO);
    }

    #[test]
    fn test_max_drawdown() {
        let equity_curve = vec![
            dec!(10000),
            dec!(11000),
            dec!(12000), // 고점
            dec!(10800), // 여기까지 낙폭
            dec!(11500),
            dec!(12500), // 새 고점
            dec!(11250), // 낙폭
            dec!(13000),
        ];

        let max_dd = PerformanceMetrics::calculate_max_drawdown(&equity_curve);

        // 최대 낙폭: (12000 - 10800) / 12000 * 100 = 10%
        assert_eq!(max_dd, dec!(10));
    }

    #[test]
    fn test_sharpe_ratio() {
        // 일관된 양의 수익률
        let returns = vec![dec!(1.0), dec!(1.5), dec!(0.8), dec!(1.2), dec!(1.0)];
        let sharpe = PerformanceMetrics::calculate_sharpe_ratio(&returns, 0.05, 252);

        // 양의 수익률이면 양의 샤프 비율
        assert!(sharpe > Decimal::ZERO);
    }

    #[test]
    fn test_sortino_ratio() {
        // 양음 혼합 수익률
        let returns = vec![dec!(2.0), dec!(-1.0), dec!(1.5), dec!(0.5), dec!(-0.5)];
        let sortino = PerformanceMetrics::calculate_sortino_ratio(&returns, 0.05, 252);

        // 계산 가능해야 함
        assert!(sortino != Decimal::ZERO);
    }

    #[test]
    fn test_rolling_metrics() {
        let mut rolling = RollingMetrics::new(5, dec!(10000));

        // 수익률 추가
        rolling.add_return(dec!(1.0), dec!(10100));
        rolling.add_return(dec!(2.0), dec!(10302));
        rolling.add_return(dec!(-1.0), dec!(10199));
        rolling.add_return(dec!(1.5), dec!(10352));

        assert_eq!(rolling.count(), 4);
        assert!(rolling.mean_return() > Decimal::ZERO);
        assert_eq!(rolling.win_rate(), dec!(75)); // 4개 중 3개 수익
    }

    #[test]
    fn test_rolling_metrics_window() {
        let mut rolling = RollingMetrics::new(3, dec!(10000));

        // 윈도우 채우기
        rolling.add_return(dec!(1.0), dec!(10100));
        rolling.add_return(dec!(2.0), dec!(10302));
        rolling.add_return(dec!(1.0), dec!(10405));

        assert_eq!(rolling.count(), 3);

        // 하나 더 추가 - 첫 번째가 제거됨
        rolling.add_return(dec!(-1.0), dec!(10301));
        assert_eq!(rolling.count(), 3);

        // 평균 = (2.0 + 1.0 - 1.0) / 3 ≈ 0.67
        let mean = rolling.mean_return();
        assert!(mean > Decimal::ZERO && mean < dec!(1.0));
    }

    #[test]
    fn test_decimal_sqrt() {
        let sqrt_4 = PerformanceMetrics::decimal_sqrt(dec!(4));
        assert!((sqrt_4 - dec!(2)).abs() < dec!(0.0001));

        let sqrt_2 = PerformanceMetrics::decimal_sqrt(dec!(2));
        assert!((sqrt_2 - dec!(1.4142)).abs() < dec!(0.001));
    }

    #[test]
    fn test_empty_round_trips() {
        let metrics = PerformanceMetrics::from_round_trips(&[], dec!(10000), None);

        assert_eq!(metrics.total_trades, 0);
        assert_eq!(metrics.sharpe_ratio, Decimal::ZERO);
        assert_eq!(metrics.max_drawdown_pct, Decimal::ZERO);
    }

    #[test]
    fn test_expectancy() {
        let round_trips = create_test_round_trips();
        let metrics = PerformanceMetrics::from_round_trips(&round_trips, dec!(10000), None);

        // 기대값 = (0.8 * 평균수익) - (0.2 * 평균손실)
        // 수익 세트이므로 양수여야 함
        assert!(metrics.expectancy > Decimal::ZERO);
    }

    #[test]
    fn test_holding_hours() {
        let rt = RoundTrip::new(
            "BTC/USDT",
            Side::Buy,
            dec!(50000),
            dec!(51000),
            dec!(0.1),
            dec!(5),
            Utc::now(),
            Utc::now() + Duration::hours(48),
        );

        assert_eq!(rt.holding_hours(), dec!(48));
    }

    #[test]
    fn test_metrics_summary() {
        let round_trips = create_test_round_trips();
        let metrics = PerformanceMetrics::from_round_trips(&round_trips, dec!(10000), None);

        let summary = metrics.summary();
        assert!(summary.contains("거래: 5"));
        assert!(summary.contains("승률:"));
    }
}
