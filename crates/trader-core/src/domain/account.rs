//! 거래소 중립적 계좌 및 잔고 타입.
//!
//! 다양한 거래소의 계좌 정보, 잔고, 보유 종목을 통일된 형식으로 표현합니다.
//! 각 거래소 커넥터는 자체 응답 타입을 이 타입으로 변환합니다.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// =============================================================================
// 보유 종목 (Holding)
// =============================================================================

/// 거래소 중립적 보유 종목.
///
/// 계좌에서 보유 중인 개별 자산/종목의 정보를 나타냅니다.
/// 현물 거래소의 잔고, 선물 거래소의 포지션 모두 이 타입으로 표현할 수 있습니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Holding {
    /// 종목 코드/심볼 (예: "005930", "AAPL", "BTC/USDT")
    pub symbol: String,
    /// 종목명/자산명 (예: "삼성전자", "Apple Inc.", "Bitcoin")
    pub asset_name: String,
    /// 보유 수량
    pub quantity: Decimal,
    /// 평균 매입가
    pub avg_price: Decimal,
    /// 현재가
    pub current_price: Decimal,
    /// 평가 금액 (quantity * current_price)
    pub eval_amount: Decimal,
    /// 평가 손익 (eval_amount - cost)
    pub profit_loss: Decimal,
    /// 평가 손익률 (%)
    pub profit_loss_rate: Decimal,
}

impl Holding {
    /// 새 보유 종목 생성.
    pub fn new(
        symbol: impl Into<String>,
        asset_name: impl Into<String>,
        quantity: Decimal,
        avg_price: Decimal,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            asset_name: asset_name.into(),
            quantity,
            avg_price,
            current_price: avg_price,
            eval_amount: quantity * avg_price,
            profit_loss: Decimal::ZERO,
            profit_loss_rate: Decimal::ZERO,
        }
    }

    /// 현재가 업데이트 및 손익 계산.
    pub fn update_price(&mut self, current_price: Decimal) {
        self.current_price = current_price;
        self.eval_amount = self.quantity * current_price;

        let cost = self.quantity * self.avg_price;
        self.profit_loss = self.eval_amount - cost;

        if cost > Decimal::ZERO {
            self.profit_loss_rate = (self.profit_loss / cost) * Decimal::from(100);
        }
    }

    /// 매입 금액 (cost).
    pub fn cost(&self) -> Decimal {
        self.quantity * self.avg_price
    }
}

// =============================================================================
// 계좌 잔고 (AccountBalance)
// =============================================================================

/// 거래소 중립적 계좌 잔고.
///
/// 거래소 계좌의 전체 잔고 정보를 나타냅니다.
/// 보유 종목, 현금 잔고, 총 평가액 등을 포함합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    /// 거래소 이름 (예: "KIS", "Binance", "Bybit")
    pub exchange: String,
    /// 보유 종목 목록
    pub holdings: Vec<Holding>,
    /// 현금/예수금 (사용 가능 금액)
    pub cash_balance: Decimal,
    /// 총 평가 금액 (현금 + 보유 종목 평가액)
    pub total_eval_amount: Option<Decimal>,
    /// 총 평가 손익
    pub total_profit_loss: Option<Decimal>,
    /// 기준 통화 (예: "KRW", "USD", "USDT")
    pub currency: String,
}

impl AccountBalance {
    /// 새 계좌 잔고 생성.
    pub fn new(exchange: impl Into<String>, currency: impl Into<String>) -> Self {
        Self {
            exchange: exchange.into(),
            holdings: Vec::new(),
            cash_balance: Decimal::ZERO,
            total_eval_amount: None,
            total_profit_loss: None,
            currency: currency.into(),
        }
    }

    /// 보유 종목 추가.
    pub fn add_holding(&mut self, holding: Holding) {
        self.holdings.push(holding);
    }

    /// 현금 잔고 설정.
    pub fn with_cash(mut self, cash: Decimal) -> Self {
        self.cash_balance = cash;
        self
    }

    /// 총 평가액 계산 (보유 종목 합계 + 현금).
    pub fn calculate_total(&mut self) {
        let holdings_value: Decimal = self.holdings.iter().map(|h| h.eval_amount).sum();
        self.total_eval_amount = Some(self.cash_balance + holdings_value);

        let total_pnl: Decimal = self.holdings.iter().map(|h| h.profit_loss).sum();
        self.total_profit_loss = Some(total_pnl);
    }

    /// 보유 종목 평가액 합계.
    pub fn holdings_value(&self) -> Decimal {
        self.holdings.iter().map(|h| h.eval_amount).sum()
    }

    /// 보유 종목 수.
    pub fn holdings_count(&self) -> usize {
        self.holdings.len()
    }
}

// =============================================================================
// 테스트
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_holding_creation() {
        let holding = Holding::new("005930", "삼성전자", dec!(100), dec!(70000));

        assert_eq!(holding.symbol, "005930");
        assert_eq!(holding.quantity, dec!(100));
        assert_eq!(holding.avg_price, dec!(70000));
        assert_eq!(holding.eval_amount, dec!(7000000));
    }

    #[test]
    fn test_holding_update_price() {
        let mut holding = Holding::new("005930", "삼성전자", dec!(100), dec!(70000));
        holding.update_price(dec!(75000));

        assert_eq!(holding.current_price, dec!(75000));
        assert_eq!(holding.eval_amount, dec!(7500000));
        assert_eq!(holding.profit_loss, dec!(500000));
        // 손익률: 500000 / 7000000 * 100 ≈ 7.14%
    }

    #[test]
    fn test_account_balance_calculation() {
        let mut balance = AccountBalance::new("KIS", "KRW").with_cash(dec!(1000000));

        let mut h1 = Holding::new("005930", "삼성전자", dec!(10), dec!(70000));
        h1.update_price(dec!(75000));
        balance.add_holding(h1);

        let mut h2 = Holding::new("000660", "SK하이닉스", dec!(5), dec!(150000));
        h2.update_price(dec!(140000));
        balance.add_holding(h2);

        balance.calculate_total();

        // 삼성전자: 10 * 75000 = 750000
        // SK하이닉스: 5 * 140000 = 700000
        // 현금: 1000000
        // 총합: 2450000
        assert_eq!(balance.total_eval_amount, Some(dec!(2450000)));
        assert_eq!(balance.holdings_count(), 2);
    }
}
