//! 테스트용 모의 데이터 simulator.
//!
//! 테스트용 모의 시세 데이터를 생성합니다.

use std::collections::HashMap;
use std::time::Duration;

use chrono::Utc;
use rand::Rng;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use tokio::time::interval;
use tracing::{debug, info};

use super::messages::{ServerMessage, TickerData};
use super::subscriptions::SharedSubscriptionManager;

/// 심볼별 가격 정보.
#[derive(Debug, Clone)]
struct SymbolPrice {
    base_price: Decimal,
    current_price: Decimal,
    high_24h: Decimal,
    low_24h: Decimal,
    volume_24h: Decimal,
}

/// 모의 데이터 시뮬레이터.
pub struct MockDataSimulator {
    subscriptions: SharedSubscriptionManager,
    prices: HashMap<String, SymbolPrice>,
}

impl MockDataSimulator {
    /// 새로운 시뮬레이터 생성.
    pub fn new(subscriptions: SharedSubscriptionManager) -> Self {
        let mut prices = HashMap::new();

        // 한국 ETF
        prices.insert(
            "KODEX-200".to_string(),
            SymbolPrice {
                base_price: dec!(36500),
                current_price: dec!(36500),
                high_24h: dec!(37000),
                low_24h: dec!(36000),
                volume_24h: dec!(1500000),
            },
        );

        prices.insert(
            "KODEX-레버리지".to_string(),
            SymbolPrice {
                base_price: dec!(18250),
                current_price: dec!(18250),
                high_24h: dec!(18500),
                low_24h: dec!(18000),
                volume_24h: dec!(2500000),
            },
        );

        // 한국 주요 주식
        prices.insert(
            "005930".to_string(),
            SymbolPrice {
                // 삼성전자
                base_price: dec!(161500),
                current_price: dec!(161500),
                high_24h: dec!(163000),
                low_24h: dec!(160000),
                volume_24h: dec!(12000000),
            },
        );

        prices.insert(
            "삼성전자".to_string(),
            SymbolPrice {
                // 삼성전자 (이름)
                base_price: dec!(161500),
                current_price: dec!(161500),
                high_24h: dec!(163000),
                low_24h: dec!(160000),
                volume_24h: dec!(12000000),
            },
        );

        prices.insert(
            "000660".to_string(),
            SymbolPrice {
                // SK하이닉스
                base_price: dec!(178000),
                current_price: dec!(178000),
                high_24h: dec!(180000),
                low_24h: dec!(175000),
                volume_24h: dec!(3500000),
            },
        );

        prices.insert(
            "SK하이닉스".to_string(),
            SymbolPrice {
                base_price: dec!(178000),
                current_price: dec!(178000),
                high_24h: dec!(180000),
                low_24h: dec!(175000),
                volume_24h: dec!(3500000),
            },
        );

        prices.insert(
            "035720".to_string(),
            SymbolPrice {
                // 카카오
                base_price: dec!(42500),
                current_price: dec!(42500),
                high_24h: dec!(43500),
                low_24h: dec!(41500),
                volume_24h: dec!(2800000),
            },
        );

        prices.insert(
            "035420".to_string(),
            SymbolPrice {
                // 네이버
                base_price: dec!(185000),
                current_price: dec!(185000),
                high_24h: dec!(188000),
                low_24h: dec!(182000),
                volume_24h: dec!(1200000),
            },
        );

        // 미국 ETF (2026년 1월 기준)
        prices.insert(
            "SPY".to_string(),
            SymbolPrice {
                base_price: dec!(605.50),
                current_price: dec!(605.50),
                high_24h: dec!(608.00),
                low_24h: dec!(602.00),
                volume_24h: dec!(50000000),
            },
        );

        prices.insert(
            "QQQ".to_string(),
            SymbolPrice {
                base_price: dec!(528.30),
                current_price: dec!(528.30),
                high_24h: dec!(532.00),
                low_24h: dec!(525.00),
                volume_24h: dec!(35000000),
            },
        );

        prices.insert(
            "TQQQ".to_string(),
            SymbolPrice {
                base_price: dec!(85.40),
                current_price: dec!(85.40),
                high_24h: dec!(87.00),
                low_24h: dec!(84.00),
                volume_24h: dec!(80000000),
            },
        );

        // 암호화폐 (2026년 1월 기준)
        prices.insert(
            "BTC-USDT".to_string(),
            SymbolPrice {
                base_price: dec!(105000),
                current_price: dec!(105000),
                high_24h: dec!(107000),
                low_24h: dec!(103000),
                volume_24h: dec!(500000000),
            },
        );

        prices.insert(
            "ETH-USDT".to_string(),
            SymbolPrice {
                base_price: dec!(3350),
                current_price: dec!(3350),
                high_24h: dec!(3400),
                low_24h: dec!(3300),
                volume_24h: dec!(200000000),
            },
        );

        Self {
            subscriptions,
            prices,
        }
    }

    /// 알려지지 않은 심볼에 대해 동적으로 가격 생성.
    fn get_or_create_price(&mut self, symbol: &str) -> &mut SymbolPrice {
        if !self.prices.contains_key(symbol) {
            // 심볼 패턴에 따라 적절한 기본 가격 설정
            let base_price = if symbol.chars().all(|c| c.is_ascii_digit()) {
                // 6자리 숫자 = 한국 주식 (대략 50,000원)
                dec!(50000)
            } else if symbol.contains("USDT") || symbol.contains("USD") {
                // 암호화폐
                dec!(100)
            } else if symbol.chars().all(|c| c.is_ascii_uppercase()) && symbol.len() <= 5 {
                // 미국 주식/ETF
                dec!(150)
            } else {
                // 기타 (한국어 이름 등)
                dec!(50000)
            };

            let high = base_price * dec!(1.02);
            let low = base_price * dec!(0.98);

            info!(symbol = %symbol, base_price = %base_price, "Created dynamic price for new symbol");

            self.prices.insert(
                symbol.to_string(),
                SymbolPrice {
                    base_price,
                    current_price: base_price,
                    high_24h: high,
                    low_24h: low,
                    volume_24h: dec!(1000000),
                },
            );
        }
        self.prices.get_mut(symbol).unwrap()
    }

    /// 시뮬레이터 시작.
    ///
    /// 백그라운드에서 주기적으로 가격 데이터를 업데이트하고 브로드캐스트합니다.
    pub async fn run(mut self, update_interval: Duration) {
        info!(
            "Mock data simulator started with interval {:?}",
            update_interval
        );

        let mut ticker = interval(update_interval);

        loop {
            ticker.tick().await;
            // 구독된 심볼에 대해 동적으로 가격 생성
            self.ensure_subscribed_symbols().await;
            self.update_prices();
            self.broadcast_tickers();
        }
    }

    /// 구독된 모든 심볼에 대해 가격 데이터가 있는지 확인하고 없으면 생성.
    async fn ensure_subscribed_symbols(&mut self) {
        let subscribed_symbols = self.subscriptions.get_subscribed_market_symbols().await;

        for symbol in subscribed_symbols {
            // 심볼이 없으면 동적으로 생성 (get_or_create_price 호출)
            let _ = self.get_or_create_price(&symbol);
        }
    }

    /// 가격 업데이트.
    fn update_prices(&mut self) {
        let mut rng = rand::thread_rng();

        for (symbol, price) in self.prices.iter_mut() {
            // 랜덤 가격 변동 (-0.5% ~ +0.5%)
            let change_pct = rng.gen_range(-0.005..0.005);
            let change = price.current_price * Decimal::try_from(change_pct).unwrap_or(dec!(0));
            price.current_price += change;

            // 최저/최고가 업데이트
            if price.current_price > price.high_24h {
                price.high_24h = price.current_price;
            }
            if price.current_price < price.low_24h {
                price.low_24h = price.current_price;
            }

            // 거래량 랜덤 추가
            let volume_change = Decimal::try_from(rng.gen_range(0.0..0.001)).unwrap_or(dec!(0));
            price.volume_24h += price.volume_24h * volume_change;

            debug!(
                symbol = %symbol,
                price = %price.current_price,
                "Price updated"
            );
        }
    }

    /// 티커 데이터 브로드캐스트.
    fn broadcast_tickers(&self) {
        let timestamp = Utc::now().timestamp_millis();

        for (symbol, price) in &self.prices {
            // 24시간 변화율 계산
            let change_24h =
                ((price.current_price - price.base_price) / price.base_price) * dec!(100);

            let ticker = TickerData {
                symbol: symbol.clone(),
                price: price.current_price,
                change_24h,
                volume_24h: price.volume_24h,
                high_24h: price.high_24h,
                low_24h: price.low_24h,
                timestamp,
            };

            let message = ServerMessage::Ticker(ticker);

            // 브로드캐스트 (에러 무시 - 구독자가 없을 수 있음)
            let _ = self.subscriptions.broadcast(message);
        }
    }
}

/// 시뮬레이터를 백그라운드로 시작.
pub fn start_simulator(subscriptions: SharedSubscriptionManager) {
    let simulator = MockDataSimulator::new(subscriptions);

    tokio::spawn(async move {
        simulator.run(Duration::from_secs(1)).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::websocket::subscriptions::create_subscription_manager;

    #[test]
    fn test_simulator_creation() {
        let subscriptions = create_subscription_manager(100);
        let simulator = MockDataSimulator::new(subscriptions);

        assert!(simulator.prices.contains_key("BTC-USDT"));
        assert!(simulator.prices.contains_key("SPY"));
        assert!(simulator.prices.contains_key("KODEX-200"));
    }

    #[tokio::test]
    async fn test_price_update() {
        let subscriptions = create_subscription_manager(100);
        let mut simulator = MockDataSimulator::new(subscriptions);

        let original_price = simulator.prices.get("BTC-USDT").unwrap().current_price;

        // 여러 번 업데이트
        for _ in 0..10 {
            simulator.update_prices();
        }

        let new_price = simulator.prices.get("BTC-USDT").unwrap().current_price;

        // 가격이 변경되었는지 확인 (매우 작은 확률로 같을 수 있음)
        // 변동폭 확인: -5% ~ +5% 이내
        let change_ratio = (new_price - original_price) / original_price;
        assert!(change_ratio > dec!(-0.05) && change_ratio < dec!(0.05));
    }
}
