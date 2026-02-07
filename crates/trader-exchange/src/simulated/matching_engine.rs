//! 시뮬레이션 거래소를 위한 주문 매칭 엔진.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::sync::Arc;
use trader_core::{Kline, OrderRequest, OrderType, RoundMethod, Side, TickSizeProvider};

/// 주문 체결 유형.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FillType {
    /// 전량 체결
    Full,
    /// 부분 체결
    Partial,
    /// 미체결 (예: 지정가 주문이 도달하지 않음)
    None,
}

/// 주문 매칭 결과.
#[derive(Debug, Clone)]
pub struct OrderMatch {
    /// 주문 ID
    pub order_id: String,
    /// 체결 유형
    pub fill_type: FillType,
    /// 체결 수량
    pub filled_quantity: Decimal,
    /// 평균 체결 가격
    pub fill_price: Decimal,
    /// 지불된 수수료
    pub commission: Decimal,
    /// 수수료 자산
    pub commission_asset: String,
    /// 체결 타임스탬프
    pub timestamp: DateTime<Utc>,
}

/// 매칭 엔진의 대기 주문.
#[derive(Debug, Clone)]
pub struct PendingOrder {
    /// 주문 ID
    pub order_id: String,
    /// 심볼
    pub symbol: String,
    /// 주문 방향
    pub side: Side,
    /// 주문 유형
    pub order_type: OrderType,
    /// 원래 수량
    pub original_quantity: Decimal,
    /// 잔여 수량 (미체결)
    pub remaining_quantity: Decimal,
    /// 지정가 (지정가 주문용)
    pub price: Option<Decimal>,
    /// 스탑 가격 (스탑 주문용)
    pub stop_price: Option<Decimal>,
    /// 생성 타임스탬프
    pub created_at: DateTime<Utc>,
}

/// 시뮬레이션 거래소를 위한 주문 매칭 엔진.
pub struct MatchingEngine {
    /// 심볼별 대기 주문
    pending_orders: HashMap<String, Vec<PendingOrder>>,
    /// 수수료율 (예: 0.1%의 경우 0.001)
    fee_rate: Decimal,
    /// 슬리피지율 (예: 0.05%의 경우 0.0005)
    slippage_rate: Decimal,
    /// 호가 단위 제공자 (옵션)
    tick_size_provider: Option<Arc<dyn TickSizeProvider>>,
    /// 주문 ID 카운터
    next_order_id: u64,
}

impl MatchingEngine {
    /// 새로운 매칭 엔진을 생성합니다.
    pub fn new(fee_rate: Decimal, slippage_rate: Decimal) -> Self {
        Self {
            pending_orders: HashMap::new(),
            fee_rate,
            slippage_rate,
            tick_size_provider: None,
            next_order_id: 1,
        }
    }

    /// ticker String에서 quote 통화를 추출합니다 (예: "BTC/USDT" -> "USDT").
    fn parse_quote(ticker: &str) -> String {
        ticker.split('/').nth(1).unwrap_or("USDT").to_string()
    }

    /// 호가 단위 제공자를 설정합니다.
    pub fn with_tick_size_provider(mut self, provider: Arc<dyn TickSizeProvider>) -> Self {
        self.tick_size_provider = Some(provider);
        self
    }

    /// 가격을 호가 단위로 라운딩합니다.
    fn round_price(&self, price: Decimal, method: RoundMethod) -> Decimal {
        if let Some(provider) = &self.tick_size_provider {
            provider.round_to_tick(price, method)
        } else {
            price
        }
    }

    /// 다음 주문 ID를 생성합니다.
    pub fn generate_order_id(&mut self) -> String {
        let id = self.next_order_id;
        self.next_order_id += 1;
        format!("SIM-{:010}", id)
    }

    /// 새로운 주문을 제출합니다.
    /// 시장가 주문의 경우 즉시 매칭 결과를 반환합니다.
    pub fn submit_order(
        &mut self,
        request: &OrderRequest,
        current_price: Decimal,
        timestamp: DateTime<Utc>,
    ) -> OrderMatch {
        let order_id = self.generate_order_id();

        match request.order_type {
            OrderType::Market => {
                // 시장가 주문은 슬리피지와 함께 즉시 체결됩니다
                let slippage = current_price * self.slippage_rate;
                let raw_fill_price = match request.side {
                    Side::Buy => current_price + slippage,
                    Side::Sell => current_price - slippage,
                };

                // 호가 단위로 라운딩 (매수는 올림, 매도는 내림)
                let fill_price = match request.side {
                    Side::Buy => self.round_price(raw_fill_price, RoundMethod::Ceil),
                    Side::Sell => self.round_price(raw_fill_price, RoundMethod::Floor),
                };

                let commission = request.quantity * fill_price * self.fee_rate;
                let commission_asset = Self::parse_quote(&request.ticker);

                OrderMatch {
                    order_id,
                    fill_type: FillType::Full,
                    filled_quantity: request.quantity,
                    fill_price,
                    commission,
                    commission_asset,
                    timestamp,
                }
            }
            OrderType::Limit => {
                // 지정가 주문: 즉시 체결 가능 여부 확인
                let raw_limit_price = request.price.unwrap_or(current_price);

                // 지정가를 호가 단위로 라운딩 (매수는 내림, 매도는 올림)
                let limit_price = match request.side {
                    Side::Buy => self.round_price(raw_limit_price, RoundMethod::Floor),
                    Side::Sell => self.round_price(raw_limit_price, RoundMethod::Ceil),
                };

                let can_fill = match request.side {
                    Side::Buy => current_price <= limit_price,
                    Side::Sell => current_price >= limit_price,
                };

                if can_fill {
                    // 지정가로 체결 (트레이더에게 더 유리함)
                    let commission = request.quantity * limit_price * self.fee_rate;
                    let commission_asset = Self::parse_quote(&request.ticker);

                    OrderMatch {
                        order_id,
                        fill_type: FillType::Full,
                        filled_quantity: request.quantity,
                        fill_price: limit_price,
                        commission,
                        commission_asset,
                        timestamp,
                    }
                } else {
                    // 대기 주문에 추가
                    let pending = PendingOrder {
                        order_id: order_id.clone(),
                        symbol: request.ticker.clone(),
                        side: request.side,
                        order_type: request.order_type,
                        original_quantity: request.quantity,
                        remaining_quantity: request.quantity,
                        price: Some(limit_price),
                        stop_price: None,
                        created_at: timestamp,
                    };

                    self.pending_orders
                        .entry(request.ticker.clone())
                        .or_default()
                        .push(pending);

                    OrderMatch {
                        order_id,
                        fill_type: FillType::None,
                        filled_quantity: dec!(0),
                        fill_price: dec!(0),
                        commission: dec!(0),
                        commission_asset: Self::parse_quote(&request.ticker),
                        timestamp,
                    }
                }
            }
            OrderType::StopLoss | OrderType::StopLossLimit => {
                // 대기 주문에 추가 (스탑 가격 도달 시 트리거됨)
                let pending = PendingOrder {
                    order_id: order_id.clone(),
                    symbol: request.ticker.clone(),
                    side: request.side,
                    order_type: request.order_type,
                    original_quantity: request.quantity,
                    remaining_quantity: request.quantity,
                    price: request.price,
                    stop_price: request.stop_price,
                    created_at: timestamp,
                };

                self.pending_orders
                    .entry(request.ticker.clone())
                    .or_default()
                    .push(pending);

                OrderMatch {
                    order_id,
                    fill_type: FillType::None,
                    filled_quantity: dec!(0),
                    fill_price: dec!(0),
                    commission: dec!(0),
                    commission_asset: Self::parse_quote(&request.ticker),
                    timestamp,
                }
            }
            OrderType::TakeProfit | OrderType::TakeProfitLimit => {
                // 대기 주문에 추가 (이익실현 가격 도달 시 트리거됨)
                let pending = PendingOrder {
                    order_id: order_id.clone(),
                    symbol: request.ticker.clone(),
                    side: request.side,
                    order_type: request.order_type,
                    original_quantity: request.quantity,
                    remaining_quantity: request.quantity,
                    price: request.price,
                    stop_price: request.stop_price,
                    created_at: timestamp,
                };

                self.pending_orders
                    .entry(request.ticker.clone())
                    .or_default()
                    .push(pending);

                OrderMatch {
                    order_id,
                    fill_type: FillType::None,
                    filled_quantity: dec!(0),
                    fill_price: dec!(0),
                    commission: dec!(0),
                    commission_asset: Self::parse_quote(&request.ticker),
                    timestamp,
                }
            }
            OrderType::TrailingStop => {
                // 트레일링 스탑은 완전히 구현되지 않음 - 손절매로 처리
                let pending = PendingOrder {
                    order_id: order_id.clone(),
                    symbol: request.ticker.clone(),
                    side: request.side,
                    order_type: request.order_type,
                    original_quantity: request.quantity,
                    remaining_quantity: request.quantity,
                    price: request.price,
                    stop_price: request.stop_price,
                    created_at: timestamp,
                };

                self.pending_orders
                    .entry(request.ticker.clone())
                    .or_default()
                    .push(pending);

                OrderMatch {
                    order_id,
                    fill_type: FillType::None,
                    filled_quantity: dec!(0),
                    fill_price: dec!(0),
                    commission: dec!(0),
                    commission_asset: Self::parse_quote(&request.ticker),
                    timestamp,
                }
            }
        }
    }

    /// 새로운 Kline을 처리하고 주문 체결을 확인합니다.
    /// 매칭된 주문 목록을 반환합니다.
    pub fn process_kline(&mut self, symbol: &String, kline: &Kline) -> Vec<OrderMatch> {
        let mut matches = Vec::new();
        let mut to_remove = Vec::new();

        if let Some(orders) = self.pending_orders.get(symbol) {
            for (idx, order) in orders.iter().enumerate() {
                if let Some(match_result) = self.try_match_order(order, kline) {
                    if match_result.fill_type == FillType::Full {
                        to_remove.push(idx);
                    }
                    matches.push(match_result);
                }
            }
        }

        // 체결된 주문 제거 (인덱스 유지를 위해 역순으로)
        if let Some(orders) = self.pending_orders.get_mut(symbol) {
            for idx in to_remove.into_iter().rev() {
                orders.remove(idx);
            }
        }

        matches
    }

    /// 대기 주문을 Kline과 매칭 시도합니다.
    fn try_match_order(&self, order: &PendingOrder, kline: &Kline) -> Option<OrderMatch> {
        let high = kline.high;
        let low = kline.low;

        match order.order_type {
            OrderType::Limit => {
                let limit_price = order.price?;

                let should_fill = match order.side {
                    Side::Buy => low <= limit_price,
                    Side::Sell => high >= limit_price,
                };

                if should_fill {
                    let commission = order.remaining_quantity * limit_price * self.fee_rate;

                    Some(OrderMatch {
                        order_id: order.order_id.clone(),
                        fill_type: FillType::Full,
                        filled_quantity: order.remaining_quantity,
                        fill_price: limit_price,
                        commission,
                        commission_asset: order.symbol.clone(),
                        timestamp: kline.close_time,
                    })
                } else {
                    None
                }
            }
            OrderType::StopLoss | OrderType::StopLossLimit => {
                let stop_price = order.stop_price?;

                // 손절매는 가격이 스탑 레벨을 교차할 때 트리거됩니다
                let should_trigger = match order.side {
                    // 손절매 매도는 가격이 스탑 가격 이하로 떨어질 때 트리거됩니다
                    Side::Sell => low <= stop_price,
                    // 손절매 매수는 가격이 스탑 가격 이상으로 오를 때 트리거됩니다
                    Side::Buy => high >= stop_price,
                };

                if should_trigger {
                    // 스탑 가격으로 체결 (시장 스탑의 경우 슬리피지 포함)
                    let fill_price = if order.order_type == OrderType::StopLoss {
                        let slippage = stop_price * self.slippage_rate;
                        match order.side {
                            Side::Sell => stop_price - slippage,
                            Side::Buy => stop_price + slippage,
                        }
                    } else {
                        order.price.unwrap_or(stop_price)
                    };

                    let commission = order.remaining_quantity * fill_price * self.fee_rate;

                    Some(OrderMatch {
                        order_id: order.order_id.clone(),
                        fill_type: FillType::Full,
                        filled_quantity: order.remaining_quantity,
                        fill_price,
                        commission,
                        commission_asset: Self::parse_quote(&order.symbol),
                        timestamp: kline.close_time,
                    })
                } else {
                    None
                }
            }
            OrderType::TakeProfit | OrderType::TakeProfitLimit => {
                let stop_price = order.stop_price?;

                // 이익실현은 가격이 목표에 도달할 때 트리거됩니다
                let should_trigger = match order.side {
                    // 이익실현 매도는 가격이 목표 이상으로 오를 때 트리거됩니다
                    Side::Sell => high >= stop_price,
                    // 이익실현 매수는 가격이 목표 이하로 떨어질 때 트리거됩니다
                    Side::Buy => low <= stop_price,
                };

                if should_trigger {
                    let fill_price = if order.order_type == OrderType::TakeProfit {
                        stop_price
                    } else {
                        order.price.unwrap_or(stop_price)
                    };

                    let commission = order.remaining_quantity * fill_price * self.fee_rate;

                    Some(OrderMatch {
                        order_id: order.order_id.clone(),
                        fill_type: FillType::Full,
                        filled_quantity: order.remaining_quantity,
                        fill_price,
                        commission,
                        commission_asset: Self::parse_quote(&order.symbol),
                        timestamp: kline.close_time,
                    })
                } else {
                    None
                }
            }
            OrderType::Market | OrderType::TrailingStop => {
                // 시장가 주문은 대기 상태가 아니어야 합니다
                None
            }
        }
    }

    /// 대기 주문을 취소합니다.
    pub fn cancel_order(&mut self, symbol: &String, order_id: &str) -> bool {
        if let Some(orders) = self.pending_orders.get_mut(symbol) {
            if let Some(pos) = orders.iter().position(|o| o.order_id == order_id) {
                orders.remove(pos);
                return true;
            }
        }
        false
    }

    /// 심볼의 모든 대기 주문을 가져옵니다.
    pub fn get_pending_orders(&self, symbol: Option<&String>) -> Vec<&PendingOrder> {
        match symbol {
            Some(s) => self
                .pending_orders
                .get(s)
                .map(|orders| orders.iter().collect())
                .unwrap_or_default(),
            None => self
                .pending_orders
                .values()
                .flat_map(|orders| orders.iter())
                .collect(),
        }
    }

    /// 특정 대기 주문을 가져옵니다.
    pub fn get_order(&self, symbol: &String, order_id: &str) -> Option<&PendingOrder> {
        self.pending_orders
            .get(symbol)?
            .iter()
            .find(|o| o.order_id == order_id)
    }

    /// 모든 대기 주문을 초기화합니다.
    pub fn clear(&mut self) {
        self.pending_orders.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trader_core::{TimeInForce, Timeframe};

    fn create_test_symbol() -> String {
        "BTC/USDT".to_string()
    }

    fn create_test_kline(open: f64, high: f64, low: f64, close: f64) -> Kline {
        let now = Utc::now();
        Kline {
            ticker: create_test_symbol(),
            timeframe: Timeframe::M1,
            open_time: now,
            close_time: now + chrono::Duration::minutes(1),
            open: Decimal::from_f64_retain(open).unwrap(),
            high: Decimal::from_f64_retain(high).unwrap(),
            low: Decimal::from_f64_retain(low).unwrap(),
            close: Decimal::from_f64_retain(close).unwrap(),
            volume: dec!(100),
            quote_volume: Some(Decimal::from_f64_retain(close * 100.0).unwrap()),
            num_trades: Some(50),
        }
    }

    #[test]
    fn test_market_order_buy() {
        let mut engine = MatchingEngine::new(dec!(0.001), dec!(0.0005));
        let symbol = create_test_symbol();
        let current_price = dec!(50000);

        let request = OrderRequest {
            ticker: symbol.to_string(),
            side: Side::Buy,
            order_type: OrderType::Market,
            quantity: dec!(0.1),
            price: None,
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: None,
            strategy_id: None,
        };

        let timestamp = Utc::now();
        let result = engine.submit_order(&request, current_price, timestamp);

        assert_eq!(result.fill_type, FillType::Full);
        assert_eq!(result.filled_quantity, dec!(0.1));
        // 슬리피지로 인해 가격이 더 높아야 함
        assert!(result.fill_price > current_price);
        assert!(result.commission > dec!(0));
    }

    #[test]
    fn test_limit_order_immediate_fill() {
        let mut engine = MatchingEngine::new(dec!(0.001), dec!(0.0005));
        let symbol = create_test_symbol();
        let current_price = dec!(50000);

        // 현재 가격 이상의 지정가 매수 -> 즉시 체결
        let request = OrderRequest {
            ticker: symbol.to_string(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            quantity: dec!(0.1),
            price: Some(dec!(51000)),
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: None,
            strategy_id: None,
        };

        let result = engine.submit_order(&request, current_price, Utc::now());

        assert_eq!(result.fill_type, FillType::Full);
        assert_eq!(result.fill_price, dec!(51000));
    }

    #[test]
    fn test_limit_order_pending() {
        let mut engine = MatchingEngine::new(dec!(0.001), dec!(0.0005));
        let symbol = create_test_symbol();
        let current_price = dec!(50000);

        // 현재 가격 이하의 지정가 매수 -> 대기
        let request = OrderRequest {
            ticker: symbol.to_string(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            quantity: dec!(0.1),
            price: Some(dec!(49000)),
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: None,
            strategy_id: None,
        };

        let result = engine.submit_order(&request, current_price, Utc::now());

        assert_eq!(result.fill_type, FillType::None);
        assert_eq!(engine.get_pending_orders(Some(&symbol)).len(), 1);
    }

    #[test]
    fn test_limit_order_fill_on_kline() {
        let mut engine = MatchingEngine::new(dec!(0.001), dec!(0.0005));
        let symbol = create_test_symbol();
        let current_price = dec!(50000);

        // 49000에 지정가 매수
        let request = OrderRequest {
            ticker: symbol.to_string(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            quantity: dec!(0.1),
            price: Some(dec!(49000)),
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: None,
            strategy_id: None,
        };

        engine.submit_order(&request, current_price, Utc::now());

        // 저가가 48500인 Kline -> 체결되어야 함
        let kline = create_test_kline(50000.0, 50500.0, 48500.0, 49500.0);
        let matches = engine.process_kline(&symbol, &kline);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].fill_type, FillType::Full);
        assert_eq!(matches[0].fill_price, dec!(49000));
    }

    #[test]
    fn test_stop_loss_order() {
        let mut engine = MatchingEngine::new(dec!(0.001), dec!(0.0005));
        let symbol = create_test_symbol();
        let current_price = dec!(50000);

        // 48000에 손절매 매도
        let request = OrderRequest {
            ticker: symbol.to_string(),
            side: Side::Sell,
            order_type: OrderType::StopLoss,
            quantity: dec!(0.1),
            price: None,
            stop_price: Some(dec!(48000)),
            time_in_force: TimeInForce::GTC,
            client_order_id: None,
            strategy_id: None,
        };

        engine.submit_order(&request, current_price, Utc::now());

        // 가격이 47500으로 하락 -> 손절매 트리거
        let kline = create_test_kline(49000.0, 49500.0, 47500.0, 47800.0);
        let matches = engine.process_kline(&symbol, &kline);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].fill_type, FillType::Full);
        // 체결 가격은 스탑 가격에서 슬리피지를 뺀 값이어야 함
        assert!(matches[0].fill_price < dec!(48000));
    }

    #[test]
    fn test_cancel_order() {
        let mut engine = MatchingEngine::new(dec!(0.001), dec!(0.0005));
        let symbol = create_test_symbol();

        let request = OrderRequest {
            ticker: symbol.to_string(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            quantity: dec!(0.1),
            price: Some(dec!(49000)),
            stop_price: None,
            time_in_force: TimeInForce::GTC,
            client_order_id: None,
            strategy_id: None,
        };

        let result = engine.submit_order(&request, dec!(50000), Utc::now());
        assert_eq!(engine.get_pending_orders(Some(&symbol)).len(), 1);

        let cancelled = engine.cancel_order(&symbol, &result.order_id);
        assert!(cancelled);
        assert_eq!(engine.get_pending_orders(Some(&symbol)).len(), 0);
    }
}
