//! 리스크 한도 정의 및 일일 손실 추적.
//!
//! 제공 기능:
//! - 리스크 한도 설정
//! - 일일 손익 추적
//! - 시간대별 일일 초기화 (UTC/KST/EST 지원)
//!
//! # 시간대 지원
//!
//! 각 시장의 거래일 기준으로 일일 한도를 초기화합니다:
//! - **UTC**: UTC 자정 기준 (기본값)
//! - **KST**: 한국 시간 09:00 (장 개시) 기준
//! - **EST**: 미국 동부 시간 09:30 (장 개시) 기준

use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 거래 시간대 설정.
///
/// 일일 한도 초기화 기준 시간을 결정합니다.
/// 각 시장의 장 개시 시간을 기준으로 합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum TradingTimezone {
    /// UTC 자정 기준 (기본값)
    #[default]
    Utc,
    /// 한국 시간 (KST, UTC+9) - 09:00 장 개시 기준
    /// 실제 초기화: UTC 00:00 (KST 09:00)
    Kst,
    /// 미국 동부 시간 (EST, UTC-5 / EDT, UTC-4) - 09:30 장 개시 기준
    /// 실제 초기화: UTC 14:30 (EST 09:30) 또는 UTC 13:30 (EDT 09:30)
    Est,
}

impl TradingTimezone {
    /// 현재 시간 기준으로 거래일 날짜를 계산합니다.
    ///
    /// 각 시장의 장 개시 시간을 기준으로 거래일을 결정합니다.
    pub fn current_trading_date(&self) -> chrono::NaiveDate {
        let now = Utc::now();

        match self {
            TradingTimezone::Utc => now.date_naive(),

            TradingTimezone::Kst => {
                // KST는 UTC+9, 장 개시 09:00 KST = 00:00 UTC
                // 00:00 UTC 이전이면 전날 거래일
                let kst_hour = (now.hour() + 9) % 24;
                let crossed_day = now.hour() + 9 >= 24;

                if crossed_day {
                    now.date_naive() + chrono::Duration::days(1)
                } else if kst_hour < 9 {
                    // KST 09:00 이전 = 이전 거래일
                    now.date_naive()
                } else {
                    now.date_naive()
                }
            }

            TradingTimezone::Est => {
                // EST는 UTC-5 (DST 기간에는 UTC-4)
                // 장 개시 09:30 EST = 14:30 UTC (또는 13:30 UTC during DST)
                // 간단히 14:30 UTC를 기준으로 사용 (DST 무시)
                let market_open_hour = 14;
                let market_open_minute = 30;

                let current_minutes = now.hour() * 60 + now.minute();
                let market_open_minutes = market_open_hour * 60 + market_open_minute;

                if current_minutes < market_open_minutes {
                    // 장 개시 전 = 전날 거래일
                    now.date_naive() - chrono::Duration::days(1)
                } else {
                    now.date_naive()
                }
            }
        }
    }

    /// 시간대 이름 반환.
    pub fn name(&self) -> &'static str {
        match self {
            TradingTimezone::Utc => "UTC",
            TradingTimezone::Kst => "KST (Korea)",
            TradingTimezone::Est => "EST (US East)",
        }
    }
}

/// 리스크 한도 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    /// 거래당 최대 포지션 크기 (계좌의 % 기준)
    pub max_position_pct: Decimal,
    /// 최대 총 노출 (계좌의 % 기준)
    pub max_total_exposure_pct: Decimal,
    /// 최대 일일 손실 (절대값)
    pub max_daily_loss: Decimal,
    /// 최대 일일 손실 (계좌의 % 기준)
    pub max_daily_loss_pct: Decimal,
    /// 기본 손절 비율
    pub default_stop_loss_pct: Decimal,
    /// 기본 익절 비율
    pub default_take_profit_pct: Decimal,
    /// 최대 오픈 포지션 수
    pub max_open_positions: usize,
    /// 거래 일시 중지를 위한 변동성 임계값
    pub volatility_threshold: Decimal,
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_position_pct: Decimal::new(5, 0),        // 5%
            max_total_exposure_pct: Decimal::new(70, 0), // 70%
            max_daily_loss: Decimal::new(100000, 2),     // 1000.00
            max_daily_loss_pct: Decimal::new(3, 0),      // 3%
            default_stop_loss_pct: Decimal::new(2, 0),   // 2%
            default_take_profit_pct: Decimal::new(5, 0), // 5%
            max_open_positions: 10,
            volatility_threshold: Decimal::new(10, 0), // 10%
        }
    }
}

/// 단일 손익 기록.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLRecord {
    /// 손익 이벤트의 타임스탬프
    pub timestamp: DateTime<Utc>,
    /// 이 손익과 관련된 심볼
    pub symbol: String,
    /// 실현 손익 금액 (양수 = 이익, 음수 = 손실)
    pub amount: Decimal,
    /// 이 손익과 관련된 거래 ID 또는 주문 ID
    pub trade_id: Option<String>,
    /// 설명 또는 메모
    pub description: Option<String>,
}

impl PnLRecord {
    /// 새 손익 기록 생성.
    pub fn new(symbol: impl Into<String>, amount: Decimal) -> Self {
        Self {
            timestamp: Utc::now(),
            symbol: symbol.into(),
            amount,
            trade_id: None,
            description: None,
        }
    }

    /// 특정 타임스탬프로 새 손익 기록 생성.
    pub fn with_timestamp(
        timestamp: DateTime<Utc>,
        symbol: impl Into<String>,
        amount: Decimal,
    ) -> Self {
        Self {
            timestamp,
            symbol: symbol.into(),
            amount,
            trade_id: None,
            description: None,
        }
    }

    /// 기록에 거래 ID 추가.
    pub fn with_trade_id(mut self, trade_id: impl Into<String>) -> Self {
        self.trade_id = Some(trade_id.into());
        self
    }

    /// 기록에 설명 추가.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 손실인지 확인.
    pub fn is_loss(&self) -> bool {
        self.amount < Decimal::ZERO
    }

    /// 이익인지 확인.
    pub fn is_profit(&self) -> bool {
        self.amount > Decimal::ZERO
    }
}

/// 일일 손실 한도 상태.
#[derive(Debug, Clone)]
pub struct DailyLimitStatus {
    /// 거래 허용 여부
    pub can_trade: bool,
    /// 현재 일일 손익
    pub daily_pnl: Decimal,
    /// 허용된 최대 일일 손실
    pub max_daily_loss: Decimal,
    /// 한도 도달 전 남은 손실 허용량
    pub remaining_allowance: Decimal,
    /// 일일 한도 사용 비율
    pub limit_usage_pct: f64,
    /// 오늘의 거래 횟수
    pub trade_count: usize,
    /// 한도에 근접할 때 경고 메시지
    pub warning: Option<String>,
}

impl DailyLimitStatus {
    /// 한도에 근접했는지 확인 (>70% 사용).
    pub fn is_approaching_limit(&self) -> bool {
        self.limit_usage_pct >= 70.0
    }

    /// 한도를 초과했는지 확인.
    pub fn is_limit_exceeded(&self) -> bool {
        !self.can_trade
    }
}

/// 일일 손실 한도 모니터링 및 적용을 위한 추적기.
///
/// 이 추적기의 기능:
/// - 하루 동안의 모든 손익 이벤트 기록
/// - 설정된 시간대(UTC/KST/EST) 기준 자동 초기화
/// - 일일 손실 한도의 실시간 상태 제공
/// - 상세 분석을 위한 심볼별 손익 추적
///
/// # 거래소 중립 설계
///
/// 시간대 설정으로 다양한 시장을 지원합니다:
/// - 한국 시장 (KOSPI/KOSDAQ): `TradingTimezone::Kst`
/// - 미국 시장 (NYSE/NASDAQ): `TradingTimezone::Est`
/// - 글로벌/암호화폐: `TradingTimezone::Utc`
#[derive(Debug, Clone)]
pub struct DailyLossTracker {
    /// 허용된 최대 일일 손실 (절대값)
    max_daily_loss: Decimal,
    /// 시작 잔액 대비 최대 일일 손실 비율
    max_daily_loss_pct: f64,
    /// 비율 계산을 위한 시작 잔액
    starting_balance: Decimal,
    /// 거래 시간대 (일일 초기화 기준)
    timezone: TradingTimezone,
    /// 현재 거래일 (초기화 감지용)
    current_date: chrono::NaiveDate,
    /// 오늘의 모든 손익 기록
    records: Vec<PnLRecord>,
    /// 심볼별 손익 요약
    symbol_pnl: HashMap<String, Decimal>,
    /// 캐시된 일일 총 손익
    daily_total: Decimal,
    /// 한도 초과로 인한 거래 일시 중지 여부
    trading_paused: bool,
}

impl DailyLossTracker {
    /// 새 일일 손실 추적기 생성.
    ///
    /// # Arguments
    /// * `max_daily_loss` - 절대값 기준 최대 일일 손실
    /// * `max_daily_loss_pct` - 비율 기준 최대 일일 손실 (예: 3%는 3.0)
    /// * `starting_balance` - 비율 계산을 위한 당일 시작 계좌 잔액
    pub fn new(
        max_daily_loss: Decimal,
        max_daily_loss_pct: f64,
        starting_balance: Decimal,
    ) -> Self {
        Self::with_timezone(
            max_daily_loss,
            max_daily_loss_pct,
            starting_balance,
            TradingTimezone::default(),
        )
    }

    /// 시간대를 지정하여 새 일일 손실 추적기 생성.
    ///
    /// # Arguments
    /// * `max_daily_loss` - 절대값 기준 최대 일일 손실
    /// * `max_daily_loss_pct` - 비율 기준 최대 일일 손실
    /// * `starting_balance` - 비율 계산을 위한 당일 시작 계좌 잔액
    /// * `timezone` - 일일 초기화 기준 시간대
    ///
    /// # 거래소별 시간대 예시
    /// - 한국 시장: `TradingTimezone::Kst`
    /// - 미국 시장: `TradingTimezone::Est`
    /// - 암호화폐: `TradingTimezone::Utc`
    pub fn with_timezone(
        max_daily_loss: Decimal,
        max_daily_loss_pct: f64,
        starting_balance: Decimal,
        timezone: TradingTimezone,
    ) -> Self {
        Self {
            max_daily_loss,
            max_daily_loss_pct,
            starting_balance,
            timezone,
            current_date: timezone.current_trading_date(),
            records: Vec::new(),
            symbol_pnl: HashMap::new(),
            daily_total: Decimal::ZERO,
            trading_paused: false,
        }
    }

    /// RiskConfig에서 생성.
    pub fn from_config(config: &crate::config::RiskConfig, starting_balance: Decimal) -> Self {
        Self::new(
            Decimal::from_f64_retain(
                config.max_daily_loss_pct * starting_balance.to_f64().unwrap_or(0.0) / 100.0,
            )
            .unwrap_or(Decimal::ZERO),
            config.max_daily_loss_pct,
            starting_balance,
        )
    }

    /// RiskConfig에서 시간대를 지정하여 생성.
    pub fn from_config_with_timezone(
        config: &crate::config::RiskConfig,
        starting_balance: Decimal,
        timezone: TradingTimezone,
    ) -> Self {
        Self::with_timezone(
            Decimal::from_f64_retain(
                config.max_daily_loss_pct * starting_balance.to_f64().unwrap_or(0.0) / 100.0,
            )
            .unwrap_or(Decimal::ZERO),
            config.max_daily_loss_pct,
            starting_balance,
            timezone,
        )
    }

    /// 현재 설정된 시간대 반환.
    pub fn timezone(&self) -> TradingTimezone {
        self.timezone
    }

    /// 새로운 거래일을 위한 초기화가 필요한지 확인하고 필요시 수행.
    fn check_and_reset(&mut self) {
        let today = self.timezone.current_trading_date();
        if today != self.current_date {
            self.reset_daily(today);
        }
    }

    /// 새로운 날을 위한 일일 추적 초기화.
    fn reset_daily(&mut self, new_date: chrono::NaiveDate) {
        self.current_date = new_date;
        self.records.clear();
        self.symbol_pnl.clear();
        self.daily_total = Decimal::ZERO;
        self.trading_paused = false;
    }

    /// 손익 이벤트 기록.
    ///
    /// # Arguments
    /// * `record` - 추가할 손익 기록
    ///
    /// # Returns
    /// 업데이트된 일일 한도 상태
    pub fn record_pnl(&mut self, record: PnLRecord) -> DailyLimitStatus {
        // 날짜 변경 확인
        self.check_and_reset();

        // 합계 업데이트
        self.daily_total += record.amount;

        // 심볼별 추적 업데이트
        let symbol_total = self
            .symbol_pnl
            .entry(record.symbol.clone())
            .or_insert(Decimal::ZERO);
        *symbol_total += record.amount;

        // 기록 저장
        self.records.push(record);

        // 한도 초과 확인
        self.check_limit_breach();

        self.get_status()
    }

    /// 이익 기록.
    pub fn record_profit(&mut self, symbol: &str, amount: Decimal) -> DailyLimitStatus {
        let record = PnLRecord::new(symbol, amount.abs());
        self.record_pnl(record)
    }

    /// 손실 기록.
    pub fn record_loss(&mut self, symbol: &str, amount: Decimal) -> DailyLimitStatus {
        let record = PnLRecord::new(symbol, -amount.abs());
        self.record_pnl(record)
    }

    /// 일일 손실 한도 초과 여부 확인.
    fn check_limit_breach(&mut self) {
        if self.daily_total < Decimal::ZERO {
            let loss = self.daily_total.abs();

            // 절대값 한도 확인
            if loss >= self.max_daily_loss {
                self.trading_paused = true;
                return;
            }

            // 비율 한도 확인
            if self.starting_balance > Decimal::ZERO {
                let loss_pct = (loss / self.starting_balance * Decimal::from(100))
                    .to_f64()
                    .unwrap_or(0.0);
                if loss_pct >= self.max_daily_loss_pct {
                    self.trading_paused = true;
                }
            }
        }
    }

    /// 현재 일일 한도 상태 조회.
    pub fn get_status(&mut self) -> DailyLimitStatus {
        // 날짜 변경 확인
        self.check_and_reset();

        let effective_limit = self.get_effective_limit();
        let current_loss = if self.daily_total < Decimal::ZERO {
            self.daily_total.abs()
        } else {
            Decimal::ZERO
        };

        let remaining = effective_limit - current_loss;
        let limit_usage_pct = if effective_limit > Decimal::ZERO {
            (current_loss / effective_limit * Decimal::from(100))
                .to_f64()
                .unwrap_or(0.0)
        } else {
            0.0
        };

        let warning = if limit_usage_pct >= 90.0 {
            Some(format!(
                "CRITICAL: Daily loss limit is {:.1}% used. Trading will be paused at 100%.",
                limit_usage_pct
            ))
        } else if limit_usage_pct >= 70.0 {
            Some(format!(
                "WARNING: Daily loss limit is {:.1}% used. Consider reducing position sizes.",
                limit_usage_pct
            ))
        } else {
            None
        };

        DailyLimitStatus {
            can_trade: !self.trading_paused,
            daily_pnl: self.daily_total,
            max_daily_loss: effective_limit,
            remaining_allowance: remaining.max(Decimal::ZERO),
            limit_usage_pct,
            trade_count: self.records.len(),
            warning,
        }
    }

    /// 유효 일일 손실 한도 계산 (절대값과 비율 기반 중 최솟값).
    fn get_effective_limit(&self) -> Decimal {
        let pct_limit = if self.starting_balance > Decimal::ZERO {
            Decimal::from_f64_retain(
                self.starting_balance.to_f64().unwrap_or(0.0) * self.max_daily_loss_pct / 100.0,
            )
            .unwrap_or(self.max_daily_loss)
        } else {
            self.max_daily_loss
        };

        self.max_daily_loss.min(pct_limit)
    }

    /// 거래 허용 여부 확인.
    pub fn can_trade(&mut self) -> bool {
        self.check_and_reset();
        !self.trading_paused
    }

    /// 일일 손익 조회.
    pub fn daily_pnl(&mut self) -> Decimal {
        self.check_and_reset();
        self.daily_total
    }

    /// 특정 심볼의 손익 조회.
    pub fn symbol_pnl(&mut self, symbol: &str) -> Decimal {
        self.check_and_reset();
        self.symbol_pnl
            .get(symbol)
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    /// 오늘의 모든 손익 기록 조회.
    pub fn records(&mut self) -> &[PnLRecord] {
        self.check_and_reset();
        &self.records
    }

    /// 오늘의 거래 횟수 조회.
    pub fn trade_count(&mut self) -> usize {
        self.check_and_reset();
        self.records.len()
    }

    /// 수동으로 추적기 초기화 (테스트 또는 수동 재정의용).
    pub fn force_reset(&mut self) {
        self.reset_daily(Utc::now().date_naive());
    }

    /// 시작 잔액 업데이트 (동적 조정용).
    pub fn update_starting_balance(&mut self, balance: Decimal) {
        self.starting_balance = balance;
    }

    /// 거래 일시 중지 재정의 (관리자 기능).
    pub fn override_pause(&mut self, allow_trading: bool) {
        self.trading_paused = !allow_trading;
    }

    /// 수익 거래 횟수 조회.
    pub fn winning_trades(&mut self) -> usize {
        self.check_and_reset();
        self.records.iter().filter(|r| r.is_profit()).count()
    }

    /// 손실 거래 횟수 조회.
    pub fn losing_trades(&mut self) -> usize {
        self.check_and_reset();
        self.records.iter().filter(|r| r.is_loss()).count()
    }

    /// 승률 조회.
    pub fn win_rate(&mut self) -> Decimal {
        self.check_and_reset();
        let total = self.records.len();
        if total == 0 {
            return Decimal::ZERO;
        }
        (Decimal::from(self.winning_trades()) / Decimal::from(total)) * Decimal::from(100)
    }

    /// 총 이익 조회 (모든 양의 손익 합계).
    pub fn total_profit(&mut self) -> Decimal {
        self.check_and_reset();
        self.records
            .iter()
            .filter(|r| r.is_profit())
            .map(|r| r.amount)
            .sum()
    }

    /// 총 손실 조회 (모든 음의 손익 합계, 양수로 반환).
    pub fn total_loss(&mut self) -> Decimal {
        self.check_and_reset();
        self.records
            .iter()
            .filter(|r| r.is_loss())
            .map(|r| r.amount.abs())
            .sum()
    }
}

// Decimal 변환에 필요
use rust_decimal::prelude::ToPrimitive;

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_daily_loss_tracker_creation() {
        let mut tracker = DailyLossTracker::new(dec!(300), 3.0, dec!(10000));

        assert!(tracker.can_trade());
        let status = tracker.get_status();
        assert_eq!(status.max_daily_loss, dec!(300));
    }

    #[test]
    fn test_record_profit() {
        let mut tracker = DailyLossTracker::new(dec!(300), 3.0, dec!(10000));

        let status = tracker.record_profit("BTC/USDT", dec!(100));

        assert!(status.can_trade);
        assert_eq!(status.daily_pnl, dec!(100));
        assert_eq!(status.trade_count, 1);
    }

    #[test]
    fn test_record_loss() {
        let mut tracker = DailyLossTracker::new(dec!(300), 3.0, dec!(10000));

        let status = tracker.record_loss("BTC/USDT", dec!(100));

        assert!(status.can_trade);
        assert_eq!(status.daily_pnl, dec!(-100));
        assert_eq!(status.trade_count, 1);
    }

    #[test]
    fn test_daily_limit_breach_absolute() {
        let mut tracker = DailyLossTracker::new(dec!(300), 10.0, dec!(10000));

        // 첫 번째 손실 - 괜찮음
        tracker.record_loss("BTC/USDT", dec!(100));
        assert!(tracker.can_trade());

        // 두 번째 손실 - 여전히 괜찮음
        tracker.record_loss("ETH/USDT", dec!(100));
        assert!(tracker.can_trade());

        // 세 번째 손실 - $300 한도 초과
        let status = tracker.record_loss("SOL/USDT", dec!(150));

        assert!(!status.can_trade);
        assert!(status.is_limit_exceeded());
    }

    #[test]
    fn test_daily_limit_breach_percentage() {
        // 10000의 3% = 300
        let mut tracker = DailyLossTracker::new(dec!(1000), 3.0, dec!(10000));

        // 350 손실은 3% (300)를 초과
        let status = tracker.record_loss("BTC/USDT", dec!(350));

        assert!(!status.can_trade);
    }

    #[test]
    fn test_effective_limit_uses_minimum() {
        // 절대값 한도: 500, 비율 한도: 10000의 3% = 300
        let tracker = DailyLossTracker::new(dec!(500), 3.0, dec!(10000));

        let effective = tracker.get_effective_limit();
        assert_eq!(effective, dec!(300)); // 더 낮은 비율 한도를 사용해야 함
    }

    #[test]
    fn test_approaching_limit_warning() {
        let mut tracker = DailyLossTracker::new(dec!(300), 3.0, dec!(10000));

        // 210 손실은 300 한도의 70%
        let status = tracker.record_loss("BTC/USDT", dec!(210));

        assert!(status.is_approaching_limit());
        assert!(status.warning.is_some());
        assert!(status.warning.unwrap().contains("WARNING"));
    }

    #[test]
    fn test_critical_warning_near_limit() {
        let mut tracker = DailyLossTracker::new(dec!(300), 3.0, dec!(10000));

        // 275 손실은 300 한도의 약 92%
        let status = tracker.record_loss("BTC/USDT", dec!(275));

        assert!(status.warning.is_some());
        assert!(status.warning.unwrap().contains("CRITICAL"));
    }

    #[test]
    fn test_symbol_pnl_tracking() {
        let mut tracker = DailyLossTracker::new(dec!(1000), 10.0, dec!(10000));

        tracker.record_profit("BTC/USDT", dec!(100));
        tracker.record_loss("BTC/USDT", dec!(30));
        tracker.record_profit("ETH/USDT", dec!(50));

        assert_eq!(tracker.symbol_pnl("BTC/USDT"), dec!(70));
        assert_eq!(tracker.symbol_pnl("ETH/USDT"), dec!(50));
        assert_eq!(tracker.symbol_pnl("SOL/USDT"), dec!(0)); // 거래 없음
    }

    #[test]
    fn test_win_rate_calculation() {
        let mut tracker = DailyLossTracker::new(dec!(1000), 10.0, dec!(10000));

        tracker.record_profit("BTC/USDT", dec!(100));
        tracker.record_profit("ETH/USDT", dec!(50));
        tracker.record_loss("SOL/USDT", dec!(30));
        tracker.record_profit("DOGE/USDT", dec!(20));

        let win_rate = tracker.win_rate();
        assert_eq!(win_rate, dec!(75)); // 4개 중 3개 승리 = 75%
    }

    #[test]
    fn test_pnl_record_builder() {
        let record = PnLRecord::new("BTC/USDT", dec!(100))
            .with_trade_id("trade_123")
            .with_description("Closed long position");

        assert_eq!(record.symbol, "BTC/USDT");
        assert_eq!(record.amount, dec!(100));
        assert_eq!(record.trade_id, Some("trade_123".to_string()));
        assert_eq!(record.description, Some("Closed long position".to_string()));
        assert!(record.is_profit());
        assert!(!record.is_loss());
    }

    #[test]
    fn test_force_reset() {
        let mut tracker = DailyLossTracker::new(dec!(300), 3.0, dec!(10000));

        tracker.record_loss("BTC/USDT", dec!(350)); // 한도 초과
        assert!(!tracker.can_trade());

        tracker.force_reset();

        assert!(tracker.can_trade());
        assert_eq!(tracker.daily_pnl(), dec!(0));
        assert_eq!(tracker.trade_count(), 0);
    }

    #[test]
    fn test_override_pause() {
        let mut tracker = DailyLossTracker::new(dec!(300), 3.0, dec!(10000));

        tracker.record_loss("BTC/USDT", dec!(350)); // 한도 초과
        assert!(!tracker.can_trade());

        tracker.override_pause(true); // 관리자 재정의

        assert!(tracker.can_trade());
    }

    #[test]
    fn test_total_profit_and_loss() {
        let mut tracker = DailyLossTracker::new(dec!(1000), 10.0, dec!(10000));

        tracker.record_profit("BTC/USDT", dec!(100));
        tracker.record_profit("ETH/USDT", dec!(50));
        tracker.record_loss("SOL/USDT", dec!(30));
        tracker.record_loss("DOGE/USDT", dec!(20));

        assert_eq!(tracker.total_profit(), dec!(150));
        assert_eq!(tracker.total_loss(), dec!(50));
        assert_eq!(tracker.daily_pnl(), dec!(100)); // 순 손익
    }
}
