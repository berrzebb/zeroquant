//! 실시간 성과 추적 모듈
//!
//! 거래가 발생할 때마다 실시간으로 성과를 추적하고 메트릭스를 업데이트합니다.
//!
//! # 주요 기능
//!
//! - **거래 기록**: 개별 거래(Trade)를 기록하고 저장
//! - **RoundTrip 매칭**: 진입과 청산 거래를 매칭하여 완전한 거래 사이클 생성
//! - **자산 곡선 추적**: 시간에 따른 자산 가치 변화 기록
//! - **실시간 메트릭스**: 증분 계산으로 성과 지표 실시간 업데이트
//! - **이벤트 발생**: 성과 임계값 도달 시 알림 이벤트 발생
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! use trader_analytics::performance::tracker::PerformanceTracker;
//! use rust_decimal_macros::dec;
//!
//! // 트래커 생성 (초기 자본 1,000만원)
//! let mut tracker = PerformanceTracker::new(dec!(10_000_000));
//!
//! // 거래 기록 (trader-core의 Trade 사용)
//! tracker.record_trade(buy_trade);
//! tracker.record_trade(sell_trade);
//!
//! // 성과 지표 조회
//! let metrics = tracker.get_metrics();
//! println!("현재 승률: {}%", metrics.win_rate_pct);
//!
//! // 자산 곡선 조회
//! for (time, equity) in tracker.get_equity_curve() {
//!     println!("{}: {} USDT", time, equity);
//! }
//! ```

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use thiserror::Error;
use tokio::sync::mpsc;
use trader_core::{Side, Trade};
use uuid::Uuid;

use super::metrics::{PerformanceMetrics, RollingMetrics, RoundTrip, DEFAULT_RISK_FREE_RATE};

/// 성과 추적 오류
#[derive(Debug, Error)]
pub enum TrackerError {
    /// 초기 자본이 유효하지 않음
    #[error("초기 자본은 0보다 커야 합니다: {0}")]
    InvalidInitialCapital(Decimal),

    /// 거래 데이터가 유효하지 않음
    #[error("유효하지 않은 거래 데이터: {0}")]
    InvalidTrade(String),

    /// 매칭할 진입 거래를 찾을 수 없음
    #[error("매칭할 진입 거래를 찾을 수 없음: 심볼={symbol}, 방향={side:?}")]
    NoMatchingEntry { symbol: String, side: Side },

    /// 이벤트 전송 실패
    #[error("이벤트 전송 실패: {0}")]
    EventSendError(String),
}

/// 성과 추적 결과 타입
pub type TrackerResult<T> = Result<T, TrackerError>;

/// 성과 이벤트
///
/// 특정 성과 조건이 충족되면 발생하는 이벤트입니다.
/// 알림, 자동 대응 등에 활용됩니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceEvent {
    /// 새로운 라운드트립 완료
    RoundTripCompleted {
        round_trip: Box<RoundTrip>,
        current_equity: Decimal,
    },

    /// 일일 손실 한도 도달
    DailyLossLimitReached {
        loss_amount: Decimal,
        limit: Decimal,
        timestamp: DateTime<Utc>,
    },

    /// 최대 낙폭 경고
    DrawdownAlert {
        current_drawdown_pct: Decimal,
        threshold_pct: Decimal,
        timestamp: DateTime<Utc>,
    },

    /// 연속 손실 경고
    ConsecutiveLossAlert {
        consecutive_losses: usize,
        threshold: usize,
        timestamp: DateTime<Utc>,
    },

    /// 새로운 고점 달성
    NewEquityHigh {
        new_high: Decimal,
        previous_high: Decimal,
        timestamp: DateTime<Utc>,
    },

    /// 수익 목표 달성
    ProfitTargetReached {
        profit_pct: Decimal,
        target_pct: Decimal,
        timestamp: DateTime<Utc>,
    },
}

/// 미체결 포지션 (진입만 된 상태)
///
/// 아직 청산되지 않은 포지션을 추적합니다.
/// 청산 거래가 들어오면 RoundTrip으로 변환됩니다.
#[derive(Debug, Clone)]
struct OpenPosition {
    /// 진입 거래 ID (추적 및 디버깅용)
    #[allow(dead_code)]
    trade_id: Uuid,
    /// 심볼
    symbol: String,
    /// 포지션 방향 (Buy = 롱, Sell = 숏)
    side: Side,
    /// 진입 가격
    entry_price: Decimal,
    /// 수량
    quantity: Decimal,
    /// 수수료
    fee: Decimal,
    /// 진입 시각
    entry_time: DateTime<Utc>,
    /// 전략 ID
    strategy_id: Option<String>,
    /// 진입 이유 (전략의 논리적 목적)
    entry_reason: Option<String>,
}

/// 자산 곡선의 한 지점
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    /// 시각
    pub timestamp: DateTime<Utc>,
    /// 자산 가치
    pub equity: Decimal,
    /// 낙폭 (%)
    pub drawdown_pct: Decimal,
}

/// 성과 임계값 설정
///
/// 이벤트 발생 조건을 설정합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// 일일 손실 한도 (금액)
    pub daily_loss_limit: Option<Decimal>,

    /// 최대 낙폭 경고 임계값 (%)
    pub max_drawdown_alert_pct: Option<Decimal>,

    /// 연속 손실 경고 횟수
    pub consecutive_loss_alert: Option<usize>,

    /// 수익 목표 (%)
    pub profit_target_pct: Option<Decimal>,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            daily_loss_limit: None,
            max_drawdown_alert_pct: Some(Decimal::from(15)), // 15% 낙폭 시 경고
            consecutive_loss_alert: Some(5),                 // 5연패 시 경고
            profit_target_pct: None,
        }
    }
}

/// 성과 추적기
///
/// 실시간으로 거래 성과를 추적하고 메트릭스를 계산합니다.
///
/// # 설계 원칙
///
/// 1. **증분 계산**: 새 거래마다 전체 재계산 대신 증분 업데이트
/// 2. **메모리 효율**: 오래된 데이터 자동 정리 (설정 가능)
/// 3. **스레드 안전**: 내부 상태 변경 메서드는 &mut self
/// 4. **이벤트 기반**: 임계값 도달 시 이벤트 발생
pub struct PerformanceTracker {
    /// 초기 자본
    initial_capital: Decimal,

    /// 현재 자산 가치
    current_equity: Decimal,

    /// 고점 자산 가치 (낙폭 계산용)
    peak_equity: Decimal,

    /// 완료된 라운드트립 목록
    round_trips: Vec<RoundTrip>,

    /// 미체결 포지션 (심볼+방향 → 포지션)
    /// 키: "BTC/USDT:Buy" 형식
    open_positions: HashMap<String, VecDeque<OpenPosition>>,

    /// 자산 곡선 기록
    equity_curve: Vec<EquityPoint>,

    /// 롤링 메트릭스 (최근 N개 거래)
    rolling_metrics: RollingMetrics,

    /// 오늘의 손익 (일일 손실 한도 추적용)
    daily_pnl: Decimal,

    /// 일일 PnL 리셋 날짜
    daily_reset_date: DateTime<Utc>,

    /// 연속 손실 횟수
    consecutive_losses: usize,

    /// 연속 수익 횟수
    consecutive_wins: usize,

    /// 임계값 설정
    thresholds: PerformanceThresholds,

    /// 이벤트 전송 채널
    event_sender: Option<mpsc::UnboundedSender<PerformanceEvent>>,

    /// 무위험 이자율 (연율화 계산용)
    risk_free_rate: f64,

    /// 롤링 윈도우 크기
    rolling_window_size: usize,

    /// 자산 곡선 최대 보관 기간 (일)
    max_equity_history_days: Option<u32>,
}

impl PerformanceTracker {
    /// 새로운 성과 추적기를 생성합니다.
    ///
    /// # 매개변수
    ///
    /// * `initial_capital` - 초기 자본금
    ///
    /// # 패닉
    ///
    /// 초기 자본이 0 이하이면 패닉이 발생합니다.
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// let tracker = PerformanceTracker::new(dec!(10_000_000));
    /// ```
    pub fn new(initial_capital: Decimal) -> Self {
        assert!(
            initial_capital > Decimal::ZERO,
            "초기 자본은 0보다 커야 합니다"
        );

        let now = Utc::now();

        Self {
            initial_capital,
            current_equity: initial_capital,
            peak_equity: initial_capital,
            round_trips: Vec::new(),
            open_positions: HashMap::new(),
            equity_curve: vec![EquityPoint {
                timestamp: now,
                equity: initial_capital,
                drawdown_pct: Decimal::ZERO,
            }],
            rolling_metrics: RollingMetrics::new(100, initial_capital),
            daily_pnl: Decimal::ZERO,
            daily_reset_date: now,
            consecutive_losses: 0,
            consecutive_wins: 0,
            thresholds: PerformanceThresholds::default(),
            event_sender: None,
            risk_free_rate: DEFAULT_RISK_FREE_RATE,
            rolling_window_size: 100,
            max_equity_history_days: Some(365), // 기본 1년
        }
    }

    /// 빌더 패턴: 임계값 설정
    pub fn with_thresholds(mut self, thresholds: PerformanceThresholds) -> Self {
        self.thresholds = thresholds;
        self
    }

    /// 빌더 패턴: 이벤트 채널 설정
    pub fn with_event_sender(mut self, sender: mpsc::UnboundedSender<PerformanceEvent>) -> Self {
        self.event_sender = Some(sender);
        self
    }

    /// 빌더 패턴: 무위험 이자율 설정
    pub fn with_risk_free_rate(mut self, rate: f64) -> Self {
        self.risk_free_rate = rate;
        self
    }

    /// 빌더 패턴: 자산 곡선 히스토리 제한 해제 (백테스팅용)
    ///
    /// 백테스팅 시 과거 데이터의 자산 곡선이 삭제되지 않도록 합니다.
    /// cleanup_old_data()가 현재 시간 기준으로 데이터를 삭제하므로,
    /// 과거 데이터로 백테스트할 때 이 메서드를 호출해야 합니다.
    pub fn without_equity_history_limit(mut self) -> Self {
        self.max_equity_history_days = None;
        self
    }

    /// 빌더 패턴: 시작 시간 설정 (백테스팅용)
    ///
    /// 백테스팅 시 첫 번째 equity point의 timestamp를 백테스트 시작 시간으로 설정합니다.
    /// 이 메서드를 호출하지 않으면 `Utc::now()`가 사용되어 차트가 왜곡될 수 있습니다.
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// let start_time = klines.first().unwrap().open_time;
    /// let tracker = PerformanceTracker::new(capital)
    ///     .with_start_time(start_time)
    ///     .without_equity_history_limit();
    /// ```
    pub fn with_start_time(mut self, start_time: DateTime<Utc>) -> Self {
        // 첫 번째 equity point의 timestamp를 업데이트
        if let Some(first_point) = self.equity_curve.first_mut() {
            first_point.timestamp = start_time;
        }
        self.daily_reset_date = start_time;
        self
    }

    /// 빌더 패턴: 롤링 윈도우 크기 설정
    pub fn with_rolling_window(mut self, window_size: usize) -> Self {
        self.rolling_window_size = window_size;
        self.rolling_metrics = RollingMetrics::new(window_size, self.initial_capital);
        self
    }

    /// 거래를 기록합니다.
    ///
    /// 거래 유형에 따라:
    /// - **진입 거래**: 미체결 포지션으로 저장
    /// - **청산 거래**: 매칭되는 진입 거래를 찾아 RoundTrip 생성
    ///
    /// # 매개변수
    ///
    /// * `trade` - 기록할 거래 (trader-core::Trade)
    /// * `is_entry` - 진입 거래 여부 (true = 진입, false = 청산)
    /// * `strategy_id` - 전략 ID (선택)
    ///
    /// # 반환값
    ///
    /// 청산 거래인 경우 생성된 RoundTrip을 반환합니다.
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// // 진입 거래 기록
    /// tracker.record_trade(&buy_trade, true, Some("grid_v1"))?;
    ///
    /// // 청산 거래 기록 (RoundTrip 반환)
    /// let round_trip = tracker.record_trade(&sell_trade, false, None)?;
    /// ```
    pub fn record_trade(
        &mut self,
        trade: &Trade,
        is_entry: bool,
        strategy_id: Option<String>,
    ) -> TrackerResult<Option<RoundTrip>> {
        // 일일 리셋 확인
        self.check_daily_reset();

        if is_entry {
            // 진입 거래: 미체결 포지션에 추가
            self.add_open_position(trade, strategy_id);
            Ok(None)
        } else {
            // 청산 거래: 매칭하여 RoundTrip 생성
            // 매칭할 진입 거래가 없으면 None 반환 (분할 매수/매도 전략)
            self.close_position(trade)
        }
    }

    /// 자산 가치를 수동으로 업데이트합니다.
    ///
    /// 미실현 손익 반영이나 외부 자금 변동 시 사용합니다.
    ///
    /// # 매개변수
    ///
    /// * `timestamp` - 업데이트 시각
    /// * `equity` - 새로운 자산 가치
    pub fn update_equity(&mut self, timestamp: DateTime<Utc>, equity: Decimal) {
        self.current_equity = equity;

        // 고점 업데이트
        if equity > self.peak_equity {
            let previous_high = self.peak_equity;
            self.peak_equity = equity;

            // 새 고점 이벤트
            self.emit_event(PerformanceEvent::NewEquityHigh {
                new_high: equity,
                previous_high,
                timestamp,
            });
        }

        // 낙폭 계산
        let drawdown_pct = if self.peak_equity > Decimal::ZERO {
            (self.peak_equity - equity) / self.peak_equity * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        // 자산 곡선에 추가
        self.equity_curve.push(EquityPoint {
            timestamp,
            equity,
            drawdown_pct,
        });

        // 낙폭 경고 확인
        self.check_drawdown_alert(drawdown_pct, timestamp);

        // 오래된 데이터 정리
        self.cleanup_old_data();
    }

    /// 초기 시간을 설정합니다 (백테스팅용)
    ///
    /// 첫 번째 equity point의 timestamp를 백테스트 시작 시간으로 설정합니다.
    /// `run()` 시작 시 호출하여 equity curve의 시작 시간을 올바르게 설정합니다.
    pub fn set_initial_timestamp(&mut self, timestamp: DateTime<Utc>) {
        if let Some(first_point) = self.equity_curve.first_mut() {
            first_point.timestamp = timestamp;
        }
        self.daily_reset_date = timestamp;
    }

    /// 현재 성과 지표를 계산하여 반환합니다.
    ///
    /// 모든 완료된 라운드트립을 기반으로 전체 성과 지표를 계산합니다.
    pub fn get_metrics(&self) -> PerformanceMetrics {
        PerformanceMetrics::from_round_trips(
            &self.round_trips,
            self.initial_capital,
            Some(self.risk_free_rate),
        )
    }

    /// 롤링 성과 지표를 반환합니다.
    ///
    /// 최근 N개 거래의 성과를 반환합니다 (N = 롤링 윈도우 크기).
    pub fn get_rolling_metrics(&self) -> &RollingMetrics {
        &self.rolling_metrics
    }

    /// 완료된 라운드트립 목록을 반환합니다.
    pub fn get_round_trips(&self) -> &[RoundTrip] {
        &self.round_trips
    }

    /// 최근 N개의 라운드트립을 반환합니다.
    pub fn get_recent_round_trips(&self, n: usize) -> &[RoundTrip] {
        let start = self.round_trips.len().saturating_sub(n);
        &self.round_trips[start..]
    }

    /// 자산 곡선을 반환합니다.
    pub fn get_equity_curve(&self) -> &[EquityPoint] {
        &self.equity_curve
    }

    /// 시간 범위로 필터링된 자산 곡선을 반환합니다.
    pub fn get_equity_curve_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<&EquityPoint> {
        self.equity_curve
            .iter()
            .filter(|p| p.timestamp >= start && p.timestamp <= end)
            .collect()
    }

    /// 현재 자산 가치를 반환합니다.
    pub fn current_equity(&self) -> Decimal {
        self.current_equity
    }

    /// 초기 자본을 반환합니다.
    pub fn initial_capital(&self) -> Decimal {
        self.initial_capital
    }

    /// 총 수익률을 반환합니다 (%).
    pub fn total_return_pct(&self) -> Decimal {
        if self.initial_capital.is_zero() {
            return Decimal::ZERO;
        }
        (self.current_equity - self.initial_capital) / self.initial_capital * Decimal::from(100)
    }

    /// 현재 낙폭을 반환합니다 (%).
    pub fn current_drawdown_pct(&self) -> Decimal {
        if self.peak_equity.is_zero() {
            return Decimal::ZERO;
        }
        (self.peak_equity - self.current_equity) / self.peak_equity * Decimal::from(100)
    }

    /// 최대 낙폭을 반환합니다 (%).
    pub fn max_drawdown_pct(&self) -> Decimal {
        self.equity_curve
            .iter()
            .map(|p| p.drawdown_pct)
            .max()
            .unwrap_or(Decimal::ZERO)
    }

    /// 오늘의 손익을 반환합니다.
    pub fn daily_pnl(&self) -> Decimal {
        self.daily_pnl
    }

    /// 연속 손실 횟수를 반환합니다.
    pub fn consecutive_losses(&self) -> usize {
        self.consecutive_losses
    }

    /// 연속 수익 횟수를 반환합니다.
    pub fn consecutive_wins(&self) -> usize {
        self.consecutive_wins
    }

    /// 미체결 포지션 수를 반환합니다.
    pub fn open_positions_count(&self) -> usize {
        self.open_positions.values().map(|v| v.len()).sum()
    }

    /// 전략별 성과를 반환합니다.
    pub fn get_metrics_by_strategy(&self, strategy_id: &str) -> PerformanceMetrics {
        let filtered: Vec<_> = self
            .round_trips
            .iter()
            .filter(|rt| rt.strategy_id.as_deref() == Some(strategy_id))
            .cloned()
            .collect();

        PerformanceMetrics::from_round_trips(
            &filtered,
            self.initial_capital,
            Some(self.risk_free_rate),
        )
    }

    /// 심볼별 성과를 반환합니다.
    pub fn get_metrics_by_symbol(&self, symbol: &str) -> PerformanceMetrics {
        let filtered: Vec<_> = self
            .round_trips
            .iter()
            .filter(|rt| rt.symbol == symbol)
            .cloned()
            .collect();

        PerformanceMetrics::from_round_trips(
            &filtered,
            self.initial_capital,
            Some(self.risk_free_rate),
        )
    }

    /// 시간 범위별 성과를 반환합니다.
    pub fn get_metrics_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> PerformanceMetrics {
        let filtered: Vec<_> = self
            .round_trips
            .iter()
            .filter(|rt| rt.entry_time >= start && rt.exit_time <= end)
            .cloned()
            .collect();

        PerformanceMetrics::from_round_trips(
            &filtered,
            self.initial_capital,
            Some(self.risk_free_rate),
        )
    }

    /// 성과 요약 문자열을 반환합니다.
    pub fn summary(&self) -> String {
        let metrics = self.get_metrics();
        format!(
            "자산: {} | 수익: {:.2}% | 거래: {} | 승률: {:.1}% | 낙폭: {:.1}%",
            self.current_equity,
            self.total_return_pct(),
            self.round_trips.len(),
            metrics.win_rate_pct,
            self.current_drawdown_pct()
        )
    }

    // ===== Private Methods =====

    /// 진입 거래를 미체결 포지션에 추가
    fn add_open_position(&mut self, trade: &Trade, strategy_id: Option<String>) {
        let key = Self::position_key(&trade.ticker.clone(), trade.side);

        // Trade.metadata에서 reason 추출
        let entry_reason = trade
            .metadata
            .get("reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let position = OpenPosition {
            trade_id: trade.id,
            symbol: trade.ticker.clone(),
            side: trade.side,
            entry_price: trade.price,
            quantity: trade.quantity,
            fee: trade.fee,
            entry_time: trade.executed_at,
            strategy_id,
            entry_reason,
        };

        self.open_positions
            .entry(key)
            .or_default()
            .push_back(position);
    }

    /// 청산 거래를 처리하고 RoundTrip 생성
    /// 매칭할 진입 거래가 없으면 None 반환 (분할 매수/매도 전략 지원)
    fn close_position(&mut self, exit_trade: &Trade) -> TrackerResult<Option<RoundTrip>> {
        // 반대 방향의 진입 거래 찾기
        let entry_side = match exit_trade.side {
            Side::Buy => Side::Sell, // 매수로 청산 = 숏 포지션 종료
            Side::Sell => Side::Buy, // 매도로 청산 = 롱 포지션 종료
        };

        let key = Self::position_key(&exit_trade.ticker.clone(), entry_side);

        let open_position = match self
            .open_positions
            .get_mut(&key)
            .and_then(|positions| positions.pop_front())
        {
            Some(pos) => pos,
            None => {
                // 분할 매수/매도 전략에서 매칭 실패는 정상적인 경우일 수 있음
                tracing::debug!(
                    symbol = %exit_trade.ticker,
                    side = ?entry_side,
                    "매칭할 진입 거래 없음 (분할 매수/매도 전략)"
                );
                return Ok(None);
            }
        };

        // exit_trade.metadata에서 exit_reason 추출
        let exit_reason = exit_trade
            .metadata
            .get("reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // RoundTrip 생성
        let total_fees = open_position.fee + exit_trade.fee;
        let round_trip = RoundTrip::new(
            &open_position.symbol,
            open_position.side,
            open_position.entry_price,
            exit_trade.price,
            open_position.quantity.min(exit_trade.quantity), // 수량 매칭
            total_fees,
            open_position.entry_time,
            exit_trade.executed_at,
        );

        // 전략 ID 추가
        let round_trip = if let Some(strategy_id) = open_position.strategy_id {
            round_trip.with_strategy(strategy_id)
        } else {
            round_trip
        };

        // 진입/청산 이유 추가
        let round_trip = round_trip.with_reasons(open_position.entry_reason.clone(), exit_reason.clone());

        // 디버그: RoundTrip reason 확인
        tracing::debug!(
            symbol = %round_trip.symbol,
            entry_reason = ?round_trip.entry_reason,
            exit_reason = ?round_trip.exit_reason,
            "RoundTrip completed with reasons"
        );

        // 상태 업데이트
        self.update_on_round_trip_complete(&round_trip);

        Ok(Some(round_trip))
    }

    /// RoundTrip 완료 시 상태 업데이트
    fn update_on_round_trip_complete(&mut self, round_trip: &RoundTrip) {
        let pnl = round_trip.pnl;
        let now = round_trip.exit_time;

        // 자산 및 일일 PnL 업데이트
        self.current_equity += pnl;
        self.daily_pnl += pnl;

        // 연속 승패 업데이트
        if pnl > Decimal::ZERO {
            self.consecutive_wins += 1;
            self.consecutive_losses = 0;
        } else if pnl < Decimal::ZERO {
            self.consecutive_losses += 1;
            self.consecutive_wins = 0;
        }

        // 고점 업데이트
        if self.current_equity > self.peak_equity {
            let previous_high = self.peak_equity;
            self.peak_equity = self.current_equity;

            self.emit_event(PerformanceEvent::NewEquityHigh {
                new_high: self.current_equity,
                previous_high,
                timestamp: now,
            });
        }

        // 낙폭 계산
        let drawdown_pct = if self.peak_equity > Decimal::ZERO {
            (self.peak_equity - self.current_equity) / self.peak_equity * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        // 자산 곡선 업데이트
        self.equity_curve.push(EquityPoint {
            timestamp: now,
            equity: self.current_equity,
            drawdown_pct,
        });

        // 롤링 메트릭스 업데이트
        self.rolling_metrics
            .add_return(round_trip.return_pct, self.current_equity);

        // 라운드트립 저장
        self.round_trips.push(round_trip.clone());

        // 이벤트 발생
        self.emit_event(PerformanceEvent::RoundTripCompleted {
            round_trip: Box::new(round_trip.clone()),
            current_equity: self.current_equity,
        });

        // 임계값 확인
        self.check_thresholds(now);
    }

    /// 포지션 키 생성 ("SYMBOL:SIDE" 형식)
    fn position_key(symbol: &str, side: Side) -> String {
        format!("{}:{:?}", symbol, side)
    }

    /// 일일 리셋 확인 (날짜가 바뀌면 일일 PnL 리셋)
    fn check_daily_reset(&mut self) {
        let today = Utc::now().date_naive();
        let reset_date = self.daily_reset_date.date_naive();

        if today > reset_date {
            self.daily_pnl = Decimal::ZERO;
            self.daily_reset_date = Utc::now();
        }
    }

    /// 임계값 확인 및 이벤트 발생
    fn check_thresholds(&mut self, timestamp: DateTime<Utc>) {
        // 일일 손실 한도
        if let Some(limit) = self.thresholds.daily_loss_limit {
            if self.daily_pnl < -limit {
                self.emit_event(PerformanceEvent::DailyLossLimitReached {
                    loss_amount: self.daily_pnl.abs(),
                    limit,
                    timestamp,
                });
            }
        }

        // 연속 손실 경고
        if let Some(threshold) = self.thresholds.consecutive_loss_alert {
            if self.consecutive_losses >= threshold {
                self.emit_event(PerformanceEvent::ConsecutiveLossAlert {
                    consecutive_losses: self.consecutive_losses,
                    threshold,
                    timestamp,
                });
            }
        }

        // 수익 목표 달성
        if let Some(target) = self.thresholds.profit_target_pct {
            let current_return = self.total_return_pct();
            if current_return >= target {
                self.emit_event(PerformanceEvent::ProfitTargetReached {
                    profit_pct: current_return,
                    target_pct: target,
                    timestamp,
                });
            }
        }
    }

    /// 낙폭 경고 확인
    fn check_drawdown_alert(&mut self, drawdown_pct: Decimal, timestamp: DateTime<Utc>) {
        if let Some(threshold) = self.thresholds.max_drawdown_alert_pct {
            if drawdown_pct >= threshold {
                self.emit_event(PerformanceEvent::DrawdownAlert {
                    current_drawdown_pct: drawdown_pct,
                    threshold_pct: threshold,
                    timestamp,
                });
            }
        }
    }

    /// 이벤트 발생
    fn emit_event(&self, event: PerformanceEvent) {
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(event);
        }
    }

    /// 오래된 데이터 정리
    fn cleanup_old_data(&mut self) {
        if let Some(days) = self.max_equity_history_days {
            let cutoff = Utc::now() - Duration::days(days as i64);
            self.equity_curve.retain(|p| p.timestamp >= cutoff);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_trade(side: Side, price: Decimal, quantity: Decimal, fee: Decimal) -> Trade {
        Trade::new(
            Uuid::new_v4(),
            "binance",
            Uuid::new_v4().to_string(),
            "BTC/USDT".to_string(),
            side,
            quantity,
            price,
        )
        .with_fee(fee, "USDT")
        .with_executed_at(Utc::now())
    }

    #[test]
    fn test_tracker_creation() {
        let tracker = PerformanceTracker::new(dec!(10000));

        assert_eq!(tracker.initial_capital(), dec!(10000));
        assert_eq!(tracker.current_equity(), dec!(10000));
        assert_eq!(tracker.total_return_pct(), Decimal::ZERO);
    }

    #[test]
    #[should_panic(expected = "초기 자본은 0보다 커야 합니다")]
    fn test_tracker_zero_capital() {
        let _ = PerformanceTracker::new(Decimal::ZERO);
    }

    #[test]
    fn test_record_entry_trade() {
        let mut tracker = PerformanceTracker::new(dec!(10000));

        let trade = create_test_trade(Side::Buy, dec!(50000), dec!(0.1), dec!(5));
        let result = tracker.record_trade(&trade, true, Some("test_strategy".to_string()));

        assert!(result.is_ok());
        assert!(result.unwrap().is_none()); // 진입은 RoundTrip 미생성
        assert_eq!(tracker.open_positions_count(), 1);
    }

    #[test]
    fn test_record_exit_trade() {
        let mut tracker = PerformanceTracker::new(dec!(10000));

        // 진입 (롱)
        let entry = create_test_trade(Side::Buy, dec!(50000), dec!(0.1), dec!(5));
        tracker.record_trade(&entry, true, None).unwrap();

        // 청산 (수익)
        let exit = create_test_trade(Side::Sell, dec!(52000), dec!(0.1), dec!(5));
        let result = tracker.record_trade(&exit, false, None);

        assert!(result.is_ok());
        let round_trip = result.unwrap().unwrap();

        // PnL = (52000 - 50000) * 0.1 - 10 = 190
        assert_eq!(round_trip.pnl, dec!(190));
        assert_eq!(tracker.open_positions_count(), 0);
        assert_eq!(tracker.current_equity(), dec!(10190));
    }

    #[test]
    fn test_short_position() {
        let mut tracker = PerformanceTracker::new(dec!(10000));

        // 진입 (숏)
        let entry = create_test_trade(Side::Sell, dec!(50000), dec!(0.1), dec!(5));
        tracker.record_trade(&entry, true, None).unwrap();

        // 청산 (수익 - 가격 하락)
        let exit = create_test_trade(Side::Buy, dec!(48000), dec!(0.1), dec!(5));
        let result = tracker.record_trade(&exit, false, None);

        let round_trip = result.unwrap().unwrap();

        // 숏 PnL = (50000 - 48000) * 0.1 - 10 = 190
        assert_eq!(round_trip.pnl, dec!(190));
    }

    #[test]
    fn test_no_matching_entry() {
        let mut tracker = PerformanceTracker::new(dec!(10000));

        // 진입 없이 청산 시도
        // 분할 매수/매도 전략에서 매칭 실패는 정상 → Ok(None) 반환
        let exit = create_test_trade(Side::Sell, dec!(52000), dec!(0.1), dec!(5));
        let result = tracker.record_trade(&exit, false, None);

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_equity_curve() {
        let mut tracker = PerformanceTracker::new(dec!(10000));

        // 첫 번째 거래
        let entry1 = create_test_trade(Side::Buy, dec!(100), dec!(10), dec!(1));
        tracker.record_trade(&entry1, true, None).unwrap();

        let exit1 = create_test_trade(Side::Sell, dec!(110), dec!(10), dec!(1));
        tracker.record_trade(&exit1, false, None).unwrap();

        // PnL = (110 - 100) * 10 - 2 = 98
        // 자산 = 10000 + 98 = 10098
        assert_eq!(tracker.current_equity(), dec!(10098));

        let curve = tracker.get_equity_curve();
        assert!(curve.len() >= 2); // 초기 + 거래 후
    }

    #[test]
    fn test_drawdown_calculation() {
        let mut tracker = PerformanceTracker::new(dec!(10000));

        // 수익 거래 (고점 생성)
        let entry1 = create_test_trade(Side::Buy, dec!(100), dec!(10), dec!(1));
        tracker.record_trade(&entry1, true, None).unwrap();

        let exit1 = create_test_trade(Side::Sell, dec!(120), dec!(10), dec!(1));
        tracker.record_trade(&exit1, false, None).unwrap();

        // 자산 = 10000 + (20 * 10 - 2) = 10198 (고점)
        let peak = tracker.peak_equity;
        assert_eq!(peak, dec!(10198));

        // 손실 거래
        let entry2 = create_test_trade(Side::Buy, dec!(120), dec!(10), dec!(1));
        tracker.record_trade(&entry2, true, None).unwrap();

        let exit2 = create_test_trade(Side::Sell, dec!(100), dec!(10), dec!(1));
        tracker.record_trade(&exit2, false, None).unwrap();

        // 자산 = 10198 + (-20 * 10 - 2) = 9996
        // 낙폭 = (10198 - 9996) / 10198 * 100 ≈ 1.98%
        let dd = tracker.current_drawdown_pct();
        assert!(dd > Decimal::ZERO);
    }

    #[test]
    fn test_consecutive_tracking() {
        let mut tracker = PerformanceTracker::new(dec!(10000));

        // 3연승
        for _ in 0..3 {
            let entry = create_test_trade(Side::Buy, dec!(100), dec!(1), dec!(0));
            tracker.record_trade(&entry, true, None).unwrap();

            let exit = create_test_trade(Side::Sell, dec!(110), dec!(1), dec!(0));
            tracker.record_trade(&exit, false, None).unwrap();
        }

        assert_eq!(tracker.consecutive_wins(), 3);
        assert_eq!(tracker.consecutive_losses(), 0);

        // 1패 (연승 리셋)
        let entry = create_test_trade(Side::Buy, dec!(100), dec!(1), dec!(0));
        tracker.record_trade(&entry, true, None).unwrap();

        let exit = create_test_trade(Side::Sell, dec!(90), dec!(1), dec!(0));
        tracker.record_trade(&exit, false, None).unwrap();

        assert_eq!(tracker.consecutive_wins(), 0);
        assert_eq!(tracker.consecutive_losses(), 1);
    }

    #[test]
    fn test_metrics_calculation() {
        let mut tracker = PerformanceTracker::new(dec!(10000));

        // 여러 거래 수행
        for i in 0..5 {
            let entry = create_test_trade(Side::Buy, dec!(100), dec!(1), dec!(0));
            tracker.record_trade(&entry, true, None).unwrap();

            let exit_price = if i % 2 == 0 { dec!(110) } else { dec!(95) };
            let exit = create_test_trade(Side::Sell, exit_price, dec!(1), dec!(0));
            tracker.record_trade(&exit, false, None).unwrap();
        }

        let metrics = tracker.get_metrics();

        assert_eq!(metrics.total_trades, 5);
        assert_eq!(metrics.winning_trades, 3);
        assert_eq!(metrics.losing_trades, 2);
        // 승률 = 3/5 * 100 = 60%
        assert_eq!(metrics.win_rate_pct, dec!(60));
    }

    #[test]
    fn test_rolling_metrics() {
        let mut tracker = PerformanceTracker::new(dec!(10000)).with_rolling_window(3);

        // 3개 거래
        for _ in 0..3 {
            let entry = create_test_trade(Side::Buy, dec!(100), dec!(1), dec!(0));
            tracker.record_trade(&entry, true, None).unwrap();

            let exit = create_test_trade(Side::Sell, dec!(110), dec!(1), dec!(0));
            tracker.record_trade(&exit, false, None).unwrap();
        }

        let rolling = tracker.get_rolling_metrics();
        assert_eq!(rolling.count(), 3);
        assert!(rolling.mean_return() > Decimal::ZERO);
        assert_eq!(rolling.win_rate(), dec!(100)); // 전부 수익
    }

    #[test]
    fn test_filter_by_strategy() {
        let mut tracker = PerformanceTracker::new(dec!(10000));

        // 전략 A
        for _ in 0..3 {
            let entry = create_test_trade(Side::Buy, dec!(100), dec!(1), dec!(0));
            tracker
                .record_trade(&entry, true, Some("strategy_a".to_string()))
                .unwrap();

            let exit = create_test_trade(Side::Sell, dec!(110), dec!(1), dec!(0));
            tracker.record_trade(&exit, false, None).unwrap();
        }

        // 전략 B
        for _ in 0..2 {
            let entry = create_test_trade(Side::Buy, dec!(100), dec!(1), dec!(0));
            tracker
                .record_trade(&entry, true, Some("strategy_b".to_string()))
                .unwrap();

            let exit = create_test_trade(Side::Sell, dec!(90), dec!(1), dec!(0));
            tracker.record_trade(&exit, false, None).unwrap();
        }

        let metrics_a = tracker.get_metrics_by_strategy("strategy_a");
        assert_eq!(metrics_a.total_trades, 3);
        assert_eq!(metrics_a.winning_trades, 3);

        let metrics_b = tracker.get_metrics_by_strategy("strategy_b");
        assert_eq!(metrics_b.total_trades, 2);
        assert_eq!(metrics_b.losing_trades, 2);
    }

    #[test]
    fn test_update_equity_manual() {
        let mut tracker = PerformanceTracker::new(dec!(10000));

        // 자산 수동 업데이트
        tracker.update_equity(Utc::now(), dec!(11000));

        assert_eq!(tracker.current_equity(), dec!(11000));
        assert_eq!(tracker.peak_equity, dec!(11000));

        // 자산 감소 (낙폭 발생)
        tracker.update_equity(Utc::now(), dec!(10500));

        assert_eq!(tracker.current_equity(), dec!(10500));
        assert_eq!(tracker.peak_equity, dec!(11000)); // 고점 유지

        // 낙폭 = (11000 - 10500) / 11000 * 100 ≈ 4.54%
        let dd = tracker.current_drawdown_pct();
        assert!(dd > dec!(4) && dd < dec!(5));
    }

    #[test]
    fn test_summary() {
        let mut tracker = PerformanceTracker::new(dec!(10000));

        let entry = create_test_trade(Side::Buy, dec!(100), dec!(10), dec!(1));
        tracker.record_trade(&entry, true, None).unwrap();

        let exit = create_test_trade(Side::Sell, dec!(110), dec!(10), dec!(1));
        tracker.record_trade(&exit, false, None).unwrap();

        let summary = tracker.summary();
        assert!(summary.contains("자산:"));
        assert!(summary.contains("수익:"));
        assert!(summary.contains("거래:"));
    }

    #[tokio::test]
    async fn test_event_emission() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let mut tracker = PerformanceTracker::new(dec!(10000)).with_event_sender(tx);

        // 거래 수행
        let entry = create_test_trade(Side::Buy, dec!(100), dec!(1), dec!(0));
        tracker.record_trade(&entry, true, None).unwrap();

        let exit = create_test_trade(Side::Sell, dec!(110), dec!(1), dec!(0));
        tracker.record_trade(&exit, false, None).unwrap();

        // 이벤트 확인 (여러 이벤트 중 RoundTripCompleted 찾기)
        let mut found_round_trip = false;
        while let Ok(event) = rx.try_recv() {
            if let PerformanceEvent::RoundTripCompleted { round_trip, .. } = event {
                assert!(round_trip.pnl > Decimal::ZERO);
                found_round_trip = true;
                break;
            }
        }
        assert!(found_round_trip, "RoundTripCompleted 이벤트를 찾을 수 없음");
    }
}
