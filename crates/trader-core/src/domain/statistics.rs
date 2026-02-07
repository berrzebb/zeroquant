//! 거래 통계 계산 공통 로직.
//!
//! 백테스트와 매매일지에서 공유하는 통계 지표를 제공합니다.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use super::calculations::net_pnl;

/// 거래 통계 집계.
///
/// 승률, Profit Factor, 평균 손익 등 거래 성과를 요약합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeStatistics {
    /// 총 거래 횟수
    pub total_trades: usize,
    /// 수익 거래 횟수
    pub winning_trades: usize,
    /// 손실 거래 횟수
    pub losing_trades: usize,
    /// 승률 (백분율, 예: 65.5 = 65.5%)
    pub win_rate_pct: Decimal,
    /// 총 수익 (수익 거래만)
    pub gross_profit: Decimal,
    /// 총 손실 (손실 거래만, 양수)
    pub gross_loss: Decimal,
    /// 순손익 (수익 - 손실)
    pub net_profit: Decimal,
    /// Profit Factor (총수익 / 총손실)
    pub profit_factor: Decimal,
    /// 평균 수익 (수익 거래만)
    pub avg_win: Decimal,
    /// 평균 손실 (손실 거래만, 양수)
    pub avg_loss: Decimal,
    /// 최대 수익 거래
    pub largest_win: Decimal,
    /// 최대 손실 거래 (양수)
    pub largest_loss: Decimal,
    /// 평균 보유 기간
    pub avg_holding_period: Duration,
    /// 기대값 (승률×평균수익 - 패률×평균손실)
    pub expectancy: Decimal,
    /// 총 수수료
    pub total_fees: Decimal,
}

impl Default for TradeStatistics {
    fn default() -> Self {
        Self {
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            win_rate_pct: Decimal::ZERO,
            gross_profit: Decimal::ZERO,
            gross_loss: Decimal::ZERO,
            net_profit: Decimal::ZERO,
            profit_factor: Decimal::ZERO,
            avg_win: Decimal::ZERO,
            avg_loss: Decimal::ZERO,
            largest_win: Decimal::ZERO,
            largest_loss: Decimal::ZERO,
            avg_holding_period: Duration::zero(),
            expectancy: Decimal::ZERO,
            total_fees: Decimal::ZERO,
        }
    }
}

impl TradeStatistics {
    /// 빈 통계 생성.
    pub fn new() -> Self {
        Self::default()
    }

    /// 거래 목록으로부터 통계 계산.
    ///
    /// # Type Parameters
    ///
    /// * `T` - UnifiedTrade trait을 구현한 타입
    ///
    /// # Arguments
    ///
    /// * `trades` - 거래 목록
    ///
    /// # Returns
    ///
    /// 계산된 통계
    pub fn from_trades<T: TradeInfo>(trades: &[T]) -> Self {
        if trades.is_empty() {
            return Self::default();
        }

        let mut stats = Self::new();
        stats.total_trades = trades.len();

        let mut total_holding_duration = Duration::zero();
        let mut winning_pnls = Vec::new();
        let mut losing_pnls = Vec::new();

        for trade in trades {
            // PnL이 없으면 스킵 (미청산 거래)
            let Some(pnl) = trade.pnl() else {
                stats.total_trades -= 1;
                continue;
            };

            // 수수료 누적
            stats.total_fees += trade.fees();

            // 순손익 계산
            let net = net_pnl(pnl, trade.fees());
            stats.net_profit += net;

            // 수익/손실 분류
            if net > Decimal::ZERO {
                stats.winning_trades += 1;
                stats.gross_profit += net;
                winning_pnls.push(net);

                if net > stats.largest_win {
                    stats.largest_win = net;
                }
            } else if net < Decimal::ZERO {
                stats.losing_trades += 1;
                let loss = net.abs();
                stats.gross_loss += loss;
                losing_pnls.push(loss);

                if loss > stats.largest_loss {
                    stats.largest_loss = loss;
                }
            }

            // 보유 기간 누적
            if let Some(duration) = trade.holding_duration() {
                total_holding_duration += duration;
            }
        }

        // 승률 계산
        if stats.total_trades > 0 {
            stats.win_rate_pct = (Decimal::from(stats.winning_trades)
                / Decimal::from(stats.total_trades))
                * dec!(100);
        }

        // Profit Factor 계산
        if stats.gross_loss > Decimal::ZERO {
            stats.profit_factor = stats.gross_profit / stats.gross_loss;
        } else if stats.gross_profit > Decimal::ZERO {
            // 손실 없이 수익만 있으면 무한대 (실무에서는 큰 값으로 표현)
            stats.profit_factor = dec!(999999);
        }

        // 평균 수익/손실 계산
        if !winning_pnls.is_empty() {
            let sum: Decimal = winning_pnls.iter().sum();
            stats.avg_win = sum / Decimal::from(winning_pnls.len());
        }

        if !losing_pnls.is_empty() {
            let sum: Decimal = losing_pnls.iter().sum();
            stats.avg_loss = sum / Decimal::from(losing_pnls.len());
        }

        // 평균 보유 기간
        if stats.total_trades > 0 {
            stats.avg_holding_period = total_holding_duration / stats.total_trades as i32;
        }

        // 기대값 계산: (승률 × 평균수익) - (패률 × 평균손실)
        if stats.total_trades > 0 {
            let win_prob = Decimal::from(stats.winning_trades) / Decimal::from(stats.total_trades);
            let loss_prob = Decimal::from(stats.losing_trades) / Decimal::from(stats.total_trades);
            stats.expectancy = (win_prob * stats.avg_win) - (loss_prob * stats.avg_loss);
        }

        stats
    }

    /// 평균 거래당 수익.
    pub fn avg_trade_pnl(&self) -> Decimal {
        if self.total_trades > 0 {
            self.net_profit / Decimal::from(self.total_trades)
        } else {
            Decimal::ZERO
        }
    }

    /// 손익비 (평균수익 / 평균손실).
    pub fn profit_loss_ratio(&self) -> Decimal {
        if self.avg_loss > Decimal::ZERO {
            self.avg_win / self.avg_loss
        } else if self.avg_win > Decimal::ZERO {
            dec!(999999) // 손실 없으면 무한대
        } else {
            Decimal::ZERO
        }
    }

    /// 수익 거래 비율 (백분율).
    pub fn winning_rate(&self) -> Decimal {
        self.win_rate_pct
    }

    /// 손실 거래 비율 (백분율).
    pub fn losing_rate(&self) -> Decimal {
        if self.total_trades > 0 {
            (Decimal::from(self.losing_trades) / Decimal::from(self.total_trades)) * dec!(100)
        } else {
            Decimal::ZERO
        }
    }
}

/// 거래 정보를 제공하는 trait.
///
/// RoundTrip, TradeExecutionRecord 등 다양한 거래 타입에서
/// 통계 계산에 필요한 정보를 추출하기 위한 인터페이스입니다.
pub trait TradeInfo {
    /// 거래 심볼.
    fn symbol(&self) -> &str;

    /// 실현 손익 (수수료 제외).
    ///
    /// # Returns
    ///
    /// - `Some(pnl)`: 청산된 거래의 손익
    /// - `None`: 미청산 거래
    fn pnl(&self) -> Option<Decimal>;

    /// 수수료 (진입 + 청산).
    fn fees(&self) -> Decimal;

    /// 진입 시각.
    fn entry_time(&self) -> DateTime<Utc>;

    /// 청산 시각.
    ///
    /// # Returns
    ///
    /// - `Some(time)`: 청산 완료
    /// - `None`: 미청산
    fn exit_time(&self) -> Option<DateTime<Utc>>;

    /// 보유 기간.
    fn holding_duration(&self) -> Option<Duration> {
        self.exit_time()
            .map(|exit| exit.signed_duration_since(self.entry_time()))
    }

    /// 보유 시간 (시간 단위).
    fn holding_hours(&self) -> Option<f64> {
        self.holding_duration()
            .map(|d| d.num_seconds() as f64 / 3600.0)
    }
}

/// 기간별 통계.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodStatistics {
    /// 기간 시작일
    pub start_date: DateTime<Utc>,
    /// 기간 종료일
    pub end_date: DateTime<Utc>,
    /// 거래 통계
    pub stats: TradeStatistics,
}

impl PeriodStatistics {
    /// 새로운 기간별 통계 생성.
    pub fn new(start_date: DateTime<Utc>, end_date: DateTime<Utc>, stats: TradeStatistics) -> Self {
        Self {
            start_date,
            end_date,
            stats,
        }
    }

    /// 기간 길이 (일수).
    pub fn period_days(&self) -> i64 {
        self.end_date
            .signed_duration_since(self.start_date)
            .num_days()
    }

    /// 일평균 거래 횟수.
    pub fn avg_trades_per_day(&self) -> f64 {
        let days = self.period_days().max(1) as f64;
        self.stats.total_trades as f64 / days
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 테스트용 간단한 거래 타입
    #[derive(Debug, Clone)]
    struct MockTrade {
        symbol: String,
        pnl: Option<Decimal>,
        fees: Decimal,
        entry_time: DateTime<Utc>,
        exit_time: Option<DateTime<Utc>>,
    }

    impl TradeInfo for MockTrade {
        fn symbol(&self) -> &str {
            &self.symbol
        }

        fn pnl(&self) -> Option<Decimal> {
            self.pnl
        }

        fn fees(&self) -> Decimal {
            self.fees
        }

        fn entry_time(&self) -> DateTime<Utc> {
            self.entry_time
        }

        fn exit_time(&self) -> Option<DateTime<Utc>> {
            self.exit_time
        }
    }

    #[test]
    fn test_empty_trades() {
        let trades: Vec<MockTrade> = vec![];
        let stats = TradeStatistics::from_trades(&trades);

        assert_eq!(stats.total_trades, 0);
        assert_eq!(stats.winning_trades, 0);
        assert_eq!(stats.losing_trades, 0);
        assert_eq!(stats.win_rate_pct, Decimal::ZERO);
    }

    #[test]
    fn test_all_winning_trades() {
        let now = Utc::now();
        let trades = vec![
            MockTrade {
                symbol: "BTC/USDT".to_string(),
                pnl: Some(dec!(100)),
                fees: dec!(5),
                entry_time: now,
                exit_time: Some(now + Duration::hours(1)),
            },
            MockTrade {
                symbol: "ETH/USDT".to_string(),
                pnl: Some(dec!(200)),
                fees: dec!(10),
                entry_time: now,
                exit_time: Some(now + Duration::hours(2)),
            },
        ];

        let stats = TradeStatistics::from_trades(&trades);

        assert_eq!(stats.total_trades, 2);
        assert_eq!(stats.winning_trades, 2);
        assert_eq!(stats.losing_trades, 0);
        assert_eq!(stats.win_rate_pct, dec!(100));
        assert_eq!(stats.gross_profit, dec!(285)); // (100-5) + (200-10)
        assert_eq!(stats.gross_loss, Decimal::ZERO);
        assert_eq!(stats.net_profit, dec!(285));
        assert_eq!(stats.total_fees, dec!(15));
        assert_eq!(stats.profit_factor, dec!(999999)); // 손실 없음
    }

    #[test]
    fn test_mixed_trades() {
        let now = Utc::now();
        let trades = vec![
            // 수익 거래
            MockTrade {
                symbol: "BTC/USDT".to_string(),
                pnl: Some(dec!(150)),
                fees: dec!(5),
                entry_time: now,
                exit_time: Some(now + Duration::hours(1)),
            },
            // 손실 거래
            MockTrade {
                symbol: "ETH/USDT".to_string(),
                pnl: Some(dec!(-50)),
                fees: dec!(3),
                entry_time: now,
                exit_time: Some(now + Duration::hours(2)),
            },
            // 수익 거래
            MockTrade {
                symbol: "SOL/USDT".to_string(),
                pnl: Some(dec!(100)),
                fees: dec!(2),
                entry_time: now,
                exit_time: Some(now + Duration::hours(3)),
            },
        ];

        let stats = TradeStatistics::from_trades(&trades);

        assert_eq!(stats.total_trades, 3);
        assert_eq!(stats.winning_trades, 2);
        assert_eq!(stats.losing_trades, 1);

        // 승률: 2/3 * 100 = 66.666...%
        assert!((stats.win_rate_pct - dec!(66.666666666666666666666666667)).abs() < dec!(0.0001));

        // 총 수익: (150-5) + (100-2) = 243
        assert_eq!(stats.gross_profit, dec!(243));

        // 총 손실: |-50-3| = 53
        assert_eq!(stats.gross_loss, dec!(53));

        // 순손익: 243 - 53 = 190
        assert_eq!(stats.net_profit, dec!(190));

        // Profit Factor: 243 / 53 ≈ 4.58
        assert!((stats.profit_factor - dec!(4.584905660377358490566037736)).abs() < dec!(0.0001));

        // 평균 수익: 243 / 2 = 121.5
        assert_eq!(stats.avg_win, dec!(121.5));

        // 평균 손실: 53 / 1 = 53
        assert_eq!(stats.avg_loss, dec!(53));

        // 총 수수료
        assert_eq!(stats.total_fees, dec!(10));
    }

    #[test]
    fn test_expectancy() {
        let now = Utc::now();
        let trades = vec![
            MockTrade {
                symbol: "BTC/USDT".to_string(),
                pnl: Some(dec!(100)),
                fees: dec!(0),
                entry_time: now,
                exit_time: Some(now + Duration::hours(1)),
            },
            MockTrade {
                symbol: "ETH/USDT".to_string(),
                pnl: Some(dec!(-50)),
                fees: dec!(0),
                entry_time: now,
                exit_time: Some(now + Duration::hours(2)),
            },
        ];

        let stats = TradeStatistics::from_trades(&trades);

        // 기대값: (0.5 * 100) - (0.5 * 50) = 50 - 25 = 25
        assert_eq!(stats.expectancy, dec!(25));
    }

    #[test]
    fn test_skip_unclosed_trades() {
        let now = Utc::now();
        let trades = vec![
            // 청산된 거래
            MockTrade {
                symbol: "BTC/USDT".to_string(),
                pnl: Some(dec!(100)),
                fees: dec!(5),
                entry_time: now,
                exit_time: Some(now + Duration::hours(1)),
            },
            // 미청산 거래 (pnl = None)
            MockTrade {
                symbol: "ETH/USDT".to_string(),
                pnl: None,
                fees: dec!(3),
                entry_time: now,
                exit_time: None,
            },
        ];

        let stats = TradeStatistics::from_trades(&trades);

        // 미청산 거래는 통계에서 제외
        assert_eq!(stats.total_trades, 1);
        assert_eq!(stats.winning_trades, 1);
    }

    #[test]
    fn test_profit_loss_ratio() {
        let stats = TradeStatistics {
            avg_win: dec!(100),
            avg_loss: dec!(50),
            ..Default::default()
        };

        // 손익비: 100 / 50 = 2.0
        assert_eq!(stats.profit_loss_ratio(), dec!(2));
    }

    #[test]
    fn test_avg_trade_pnl() {
        let stats = TradeStatistics {
            total_trades: 5,
            net_profit: dec!(250),
            ..Default::default()
        };

        // 평균 거래당 수익: 250 / 5 = 50
        assert_eq!(stats.avg_trade_pnl(), dec!(50));
    }
}
