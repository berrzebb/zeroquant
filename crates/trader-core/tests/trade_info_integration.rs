//! TradeInfo trait 통합 테스트
//!
//! 다양한 타입에서 TradeInfo trait을 통해 통계를 계산하는 예시

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use trader_core::{TradeInfo, TradeStatistics};

/// 테스트용 거래 구조체
#[derive(Debug, Clone)]
struct TestTrade {
    symbol: String,
    pnl: Option<Decimal>,
    fees: Decimal,
    entry_time: chrono::DateTime<Utc>,
    exit_time: Option<chrono::DateTime<Utc>>,
}

impl TradeInfo for TestTrade {
    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn pnl(&self) -> Option<Decimal> {
        self.pnl
    }

    fn fees(&self) -> Decimal {
        self.fees
    }

    fn entry_time(&self) -> chrono::DateTime<Utc> {
        self.entry_time
    }

    fn exit_time(&self) -> Option<chrono::DateTime<Utc>> {
        self.exit_time
    }
}

#[test]
fn test_trade_statistics_from_trades() {
    let now = Utc::now();

    let trades = vec![
        // 승리 거래 1
        TestTrade {
            symbol: "BTC/USDT".to_string(),
            pnl: Some(dec!(100)),
            fees: dec!(5),
            entry_time: now,
            exit_time: Some(now + Duration::hours(2)),
        },
        // 승리 거래 2
        TestTrade {
            symbol: "ETH/USDT".to_string(),
            pnl: Some(dec!(50)),
            fees: dec!(3),
            entry_time: now + Duration::hours(3),
            exit_time: Some(now + Duration::hours(6)),
        },
        // 패배 거래
        TestTrade {
            symbol: "BTC/USDT".to_string(),
            pnl: Some(dec!(-30)),
            fees: dec!(2),
            entry_time: now + Duration::hours(7),
            exit_time: Some(now + Duration::hours(8)),
        },
    ];

    // TradeInfo trait을 통해 통계 계산
    let stats = TradeStatistics::from_trades(&trades);

    // 검증
    assert_eq!(stats.total_trades, 3);
    assert_eq!(stats.winning_trades, 2);
    assert_eq!(stats.losing_trades, 1);

    // 순손익 = (100 - 5) + (50 - 3) + (-30 - 2) = 95 + 47 - 32 = 110
    // pnl에서 각 거래의 수수료를 차감
    assert_eq!(stats.net_profit, dec!(110));

    // 총 수수료 = 5 + 3 + 2 = 10
    assert_eq!(stats.total_fees, dec!(10));

    // 승률 = (2 / 3) * 100 = 66.66...%
    assert!(stats.win_rate_pct > dec!(66.0) && stats.win_rate_pct < dec!(67.0));

    // 평균 승리 = (95 + 47) / 2 = 71
    assert_eq!(stats.avg_win, dec!(71));

    // 평균 손실 = 32 / 1 = 32 (양수로 표현)
    assert_eq!(stats.avg_loss, dec!(32));

    // 최대 승리 = 95 (100 - 5)
    assert_eq!(stats.largest_win, dec!(95));

    // 최대 손실 = 32 (|-30 - 2| = 32, 양수로 표현)
    assert_eq!(stats.largest_loss, dec!(32));

    // 프로핏 팩터 = 총이익 / 총손실 = 142 / 32 = 4.4375
    assert!((stats.profit_factor - dec!(4.4375)).abs() < dec!(0.01));

    println!("✅ TradeInfo 통합 테스트 성공!");
    println!("   총 거래: {}", stats.total_trades);
    println!("   승률: {:.2}%", stats.win_rate_pct);
    println!("   순손익: {:.2}", stats.net_profit);
    println!("   프로핏 팩터: {:.2}", stats.profit_factor);
}

#[test]
fn test_trade_statistics_empty_trades() {
    let trades: Vec<TestTrade> = vec![];
    let stats = TradeStatistics::from_trades(&trades);

    assert_eq!(stats.total_trades, 0);
    assert_eq!(stats.winning_trades, 0);
    assert_eq!(stats.losing_trades, 0);
    assert_eq!(stats.net_profit, Decimal::ZERO);
}

#[test]
fn test_trade_statistics_only_winning_trades() {
    let now = Utc::now();

    let trades = vec![
        TestTrade {
            symbol: "BTC/USDT".to_string(),
            pnl: Some(dec!(100)),
            fees: dec!(5),
            entry_time: now,
            exit_time: Some(now + Duration::hours(1)),
        },
        TestTrade {
            symbol: "ETH/USDT".to_string(),
            pnl: Some(dec!(50)),
            fees: dec!(3),
            entry_time: now + Duration::hours(2),
            exit_time: Some(now + Duration::hours(3)),
        },
    ];

    let stats = TradeStatistics::from_trades(&trades);

    assert_eq!(stats.total_trades, 2);
    assert_eq!(stats.winning_trades, 2);
    assert_eq!(stats.losing_trades, 0);
    assert_eq!(stats.win_rate_pct, dec!(100.0));
    // 순손익 = (100 - 5) + (50 - 3) = 95 + 47 = 142
    assert_eq!(stats.net_profit, dec!(142));
}

#[test]
fn test_trade_statistics_with_unclosed_trades() {
    let now = Utc::now();

    let trades = vec![
        // 청산된 거래
        TestTrade {
            symbol: "BTC/USDT".to_string(),
            pnl: Some(dec!(100)),
            fees: dec!(5),
            entry_time: now,
            exit_time: Some(now + Duration::hours(2)),
        },
        // 미청산 거래 (pnl = None)
        TestTrade {
            symbol: "ETH/USDT".to_string(),
            pnl: None,
            fees: dec!(3),
            entry_time: now + Duration::hours(3),
            exit_time: None,
        },
    ];

    let stats = TradeStatistics::from_trades(&trades);

    // 미청산 거래는 통계에서 제외됨 (pnl이 None인 거래)
    assert_eq!(stats.total_trades, 1); // 청산된 거래만 집계
                                       // pnl이 있는 거래만 집계
    assert_eq!(stats.winning_trades, 1);
    // 순손익 = 100 - 5 = 95
    assert_eq!(stats.net_profit, dec!(95));
}

#[test]
fn test_holding_duration_calculation() {
    let now = Utc::now();

    let trade = TestTrade {
        symbol: "BTC/USDT".to_string(),
        pnl: Some(dec!(100)),
        fees: dec!(5),
        entry_time: now,
        exit_time: Some(now + Duration::hours(24)), // 24시간 보유
    };

    // TradeInfo trait의 holding_hours() 메서드 테스트
    let holding_hours = trade.holding_hours();
    assert!(holding_hours.is_some());
    assert!((holding_hours.unwrap() - 24.0).abs() < 0.1);
}
