//! 백테스트-저널 통합 모듈.
//!
//! 백테스트 결과를 매매일지(Journal) 형식으로 변환하여,
//! Journal의 SQL 뷰 기반 분석 인프라를 재활용합니다.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::performance::RoundTrip;
use trader_core::{Side, TradeInfo};

/// 매매일지 체결 내역 입력 (trader-api의 TradeExecutionInput 간소화 버전).
///
/// DB 전용 필드(credential_id, UUID)를 제외한 핵심 거래 데이터만 포함합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalTradeInput {
    /// 거래소 이름
    pub exchange: String,
    /// 심볼 (예: "BTC/USDT")
    pub symbol: String,
    /// 심볼 한글명 (선택)
    pub symbol_name: Option<String>,
    /// 매수/매도 방향
    pub side: Side,
    /// 주문 유형 (MARKET, LIMIT 등)
    pub order_type: String,
    /// 거래 수량
    pub quantity: Decimal,
    /// 체결 가격
    pub price: Decimal,
    /// 수수료
    pub fee: Decimal,
    /// 포지션 효과 (OPEN, CLOSE)
    pub position_effect: String,
    /// 실현 손익 (청산 시에만)
    pub realized_pnl: Option<Decimal>,
    /// 전략 ID
    pub strategy_id: String,
    /// 전략 이름
    pub strategy_name: Option<String>,
    /// 체결 시각
    pub executed_at: DateTime<Utc>,
    /// 메모 (백테스트 표시용)
    pub memo: Option<String>,
}

impl TradeInfo for JournalTradeInput {
    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn pnl(&self) -> Option<Decimal> {
        self.realized_pnl
    }

    fn fees(&self) -> Decimal {
        self.fee
    }

    fn entry_time(&self) -> DateTime<Utc> {
        self.executed_at
    }

    fn exit_time(&self) -> Option<DateTime<Utc>> {
        // JournalTradeInput은 단일 체결 내역.
        // realized_pnl이 있으면 청산 체결 (exit_time = executed_at)
        // realized_pnl이 없으면 진입 체결 (exit_time = None)
        if self.realized_pnl.is_some() {
            Some(self.executed_at)
        } else {
            None
        }
    }
}

/// 백테스트 리포트의 거래 내역을 매매일지 형식으로 변환합니다.
///
/// 각 RoundTrip은 2개의 체결 내역으로 분해됩니다:
/// 1. 진입 체결 (position_effect = "OPEN")
/// 2. 청산 체결 (position_effect = "CLOSE", realized_pnl 포함)
///
/// # 인수
///
/// * `round_trips` - 백테스트 완료 거래 목록
/// * `exchange` - 거래소 이름 (예: "BACKTEST")
/// * `strategy_id` - 전략 ID
/// * `strategy_name` - 전략 이름
///
/// # 반환값
///
/// 매매일지에 저장할 수 있는 체결 내역 목록 (시간순 정렬)
///
/// # 예시
///
/// ```rust,ignore
/// use trader_analytics::journal_integration::*;
///
/// let trades = export_backtest_to_journal(
///     &report.trades,
///     "BACKTEST",
///     "rsi_mean_reversion",
///     Some("RSI 평균회귀"),
/// );
///
/// // Journal DB에 저장
/// for trade in trades {
///     journal_repo.insert_execution(&trade).await?;
/// }
/// ```
pub fn export_backtest_to_journal(
    round_trips: &[RoundTrip],
    exchange: &str,
    strategy_id: &str,
    strategy_name: Option<&str>,
) -> Vec<JournalTradeInput> {
    let mut executions = Vec::with_capacity(round_trips.len() * 2);

    for rt in round_trips {
        // 진입 수수료 = 총 수수료의 절반 (근사치)
        let entry_fee = rt.fees / Decimal::from(2);
        let exit_fee = rt.fees - entry_fee;

        // 1. 진입 체결
        executions.push(JournalTradeInput {
            exchange: exchange.to_string(),
            symbol: rt.symbol.clone(),
            symbol_name: None,
            side: rt.side,
            order_type: "MARKET".to_string(),
            quantity: rt.quantity,
            price: rt.entry_price,
            fee: entry_fee,
            position_effect: "OPEN".to_string(),
            realized_pnl: None, // 진입 시에는 손익 없음
            strategy_id: strategy_id.to_string(),
            strategy_name: strategy_name.map(|s| s.to_string()),
            executed_at: rt.entry_time,
            memo: Some(format!("백테스트 진입 (RoundTrip ID: {})", rt.id)),
        });

        // 2. 청산 체결
        let close_side = match rt.side {
            Side::Buy => Side::Sell, // 롱 포지션 청산 = 매도
            Side::Sell => Side::Buy, // 숏 포지션 청산 = 매수
        };

        executions.push(JournalTradeInput {
            exchange: exchange.to_string(),
            symbol: rt.symbol.clone(),
            symbol_name: None,
            side: close_side,
            order_type: "MARKET".to_string(),
            quantity: rt.quantity,
            price: rt.exit_price,
            fee: exit_fee,
            position_effect: "CLOSE".to_string(),
            realized_pnl: Some(rt.pnl), // 청산 시 실현 손익
            strategy_id: strategy_id.to_string(),
            strategy_name: strategy_name.map(|s| s.to_string()),
            executed_at: rt.exit_time,
            memo: Some(format!(
                "백테스트 청산 (RoundTrip ID: {}, 손익: {:.2})",
                rt.id, rt.pnl
            )),
        });
    }

    // 시간순 정렬
    executions.sort_by_key(|e| e.executed_at);

    executions
}

/// 백테스트 거래를 매매일지에 일괄 저장하기 위한 헬퍼 함수.
///
/// 백테스트 결과를 "BACKTEST" 거래소로 표시하여 저장합니다.
///
/// # 인수
///
/// * `round_trips` - 백테스트 완료 거래
/// * `strategy_id` - 전략 ID
/// * `strategy_name` - 전략 이름 (선택)
///
/// # 반환값
///
/// 매매일지 저장용 체결 내역
pub fn export_backtest_trades(
    round_trips: &[RoundTrip],
    strategy_id: &str,
    strategy_name: Option<&str>,
) -> Vec<JournalTradeInput> {
    export_backtest_to_journal(round_trips, "BACKTEST", strategy_id, strategy_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;

    #[test]
    fn test_export_single_roundtrip() {
        let now = Utc::now();
        let rt = RoundTrip::new(
            "BTC/USDT",
            Side::Buy,
            dec!(50000),
            dec!(52000),
            dec!(0.1),
            dec!(10),
            now,
            now + chrono::Duration::hours(2),
        );

        let trades = export_backtest_trades(&[rt], "test_strategy", Some("테스트 전략"));

        assert_eq!(trades.len(), 2);

        // 진입 체결 검증
        let entry = &trades[0];
        assert_eq!(entry.symbol, "BTC/USDT");
        assert_eq!(entry.side, Side::Buy);
        assert_eq!(entry.price, dec!(50000));
        assert_eq!(entry.quantity, dec!(0.1));
        assert_eq!(entry.position_effect, "OPEN");
        assert_eq!(entry.realized_pnl, None);
        assert_eq!(entry.fee, dec!(5)); // 수수료의 절반

        // 청산 체결 검증
        let exit = &trades[1];
        assert_eq!(exit.symbol, "BTC/USDT");
        assert_eq!(exit.side, Side::Sell); // 롱 청산은 매도
        assert_eq!(exit.price, dec!(52000));
        assert_eq!(exit.quantity, dec!(0.1));
        assert_eq!(exit.position_effect, "CLOSE");
        assert!(exit.realized_pnl.is_some());
        assert_eq!(exit.fee, dec!(5)); // 나머지 절반
    }

    #[test]
    fn test_export_short_position() {
        let now = Utc::now();
        let rt = RoundTrip::new(
            "ETH/USDT",
            Side::Sell, // 숏 포지션
            dec!(3000),
            dec!(2800),
            dec!(1.0),
            dec!(6),
            now,
            now + chrono::Duration::hours(1),
        );

        let trades = export_backtest_trades(&[rt], "short_test", None);

        assert_eq!(trades.len(), 2);

        // 진입 = 매도
        assert_eq!(trades[0].side, Side::Sell);
        assert_eq!(trades[0].position_effect, "OPEN");

        // 청산 = 매수
        assert_eq!(trades[1].side, Side::Buy);
        assert_eq!(trades[1].position_effect, "CLOSE");
    }

    #[test]
    fn test_multiple_roundtrips_sorted() {
        let now = Utc::now();

        let rt1 = RoundTrip::new(
            "BTC/USDT",
            Side::Buy,
            dec!(50000),
            dec!(51000),
            dec!(0.1),
            dec!(10),
            now,
            now + chrono::Duration::hours(1),
        );

        let rt2 = RoundTrip::new(
            "ETH/USDT",
            Side::Buy,
            dec!(3000),
            dec!(3100),
            dec!(1.0),
            dec!(6),
            now + chrono::Duration::minutes(30),
            now + chrono::Duration::hours(2),
        );

        let trades = export_backtest_trades(&[rt1, rt2], "multi_test", None);

        assert_eq!(trades.len(), 4);

        // 시간순 정렬 확인
        assert!(trades[0].executed_at <= trades[1].executed_at);
        assert!(trades[1].executed_at <= trades[2].executed_at);
        assert!(trades[2].executed_at <= trades[3].executed_at);

        // 순서: BTC 진입 -> ETH 진입 -> BTC 청산 -> ETH 청산
        assert_eq!(trades[0].symbol, "BTC/USDT");
        assert_eq!(trades[0].position_effect, "OPEN");
        assert_eq!(trades[1].symbol, "ETH/USDT");
        assert_eq!(trades[1].position_effect, "OPEN");
    }
}
