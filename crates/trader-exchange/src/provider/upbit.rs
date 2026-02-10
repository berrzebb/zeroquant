//! Upbit ExchangeProvider + MarketDataProvider 구현.
//!
//! UpbitClient를 래핑하여 거래소 중립적인 인터페이스를 제공합니다.

use crate::connector::upbit::UpbitClient;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::debug;
use trader_core::cache::ExchangeCache;
use rust_decimal::Decimal;
use tracing::info;
use trader_core::domain::{
    ExchangeProvider, ExecutionHistoryRequest, ExecutionHistoryResponse, MarketDataProvider,
    OrderExecutionProvider, OrderRequest, OrderResponse, OrderType,
    PendingOrder, ProviderError, QuoteData, Side, StrategyAccountInfo, StrategyPositionInfo,
};

/// Upbit ExchangeProvider 구현.
///
/// UpbitClient를 래핑하여 캐싱 레이어를 추가합니다.
pub struct UpbitExchangeProvider {
    client: Arc<UpbitClient>,
    cache: Arc<ExchangeCache>,
}

/// 하위 호환성을 위한 타입 별칭.
pub type UpbitProvider = UpbitExchangeProvider;

impl UpbitExchangeProvider {
    /// 새 UpbitExchangeProvider 생성.
    pub fn new(client: Arc<UpbitClient>) -> Self {
        Self {
            client,
            cache: Arc::new(ExchangeCache::with_defaults()),
        }
    }

    /// 공용 캐시 참조 반환.
    pub fn exchange_cache(&self) -> Arc<ExchangeCache> {
        Arc::clone(&self.cache)
    }
}

// ==================== ExchangeProvider ====================

#[async_trait]
impl ExchangeProvider for UpbitExchangeProvider {
    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
        if let Some(cached) = self.cache.get_account().await {
            debug!("Upbit 계좌 정보 캐시 히트");
            return Ok(cached);
        }

        let result = self.client.fetch_account().await?;
        self.cache.set_account(result.clone()).await;
        Ok(result)
    }

    async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        if let Some(cached) = self.cache.get_positions().await {
            debug!("Upbit 포지션 캐시 히트");
            return Ok(cached);
        }

        let result = self.client.fetch_positions().await?;
        self.cache.set_positions(result.clone()).await;
        Ok(result)
    }

    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError> {
        if let Some(cached) = self.cache.get_pending_orders().await {
            debug!("Upbit 미체결 주문 캐시 히트");
            return Ok(cached);
        }

        let result = self.client.fetch_pending_orders().await?;
        self.cache.set_pending_orders(result.clone()).await;
        Ok(result)
    }

    async fn fetch_execution_history(
        &self,
        request: &ExecutionHistoryRequest,
    ) -> Result<ExecutionHistoryResponse, ProviderError> {
        use chrono::DateTime;
        use std::str::FromStr;
        use uuid::Uuid;
        use trader_core::domain::Trade;

        // 기본 조회 개수: 100 (최대 1000)
        let limit = 100;

        let order_details = self.client.fetch_execution_history(
            &request.start_date,
            &request.end_date,
            limit,
        ).await?;

        let mut trades = Vec::new();

        for detail in order_details {
            // Side 변환
            let side = if detail.side == "bid" { Side::Buy } else { Side::Sell };

            // 체결 수량
            let quantity = detail.executed_volume
                .and_then(|v| Decimal::from_str(&v).ok())
                .unwrap_or_default();

            // 체결 평균가 (주문 가격 사용, 실제로는 executed_funds / executed_volume)
            let price = detail.price
                .and_then(|p| Decimal::from_str(&p).ok())
                .unwrap_or_default();

            // 수수료
            let fee = detail.paid_fee
                .and_then(|f| Decimal::from_str(&f).ok())
                .unwrap_or_default();

            // 체결 시각 파싱 (KST → UTC)
            let executed_at = DateTime::parse_from_rfc3339(&format!("{}+09:00", detail.created_at))
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(chrono::Utc::now);

            trades.push(Trade {
                id: Uuid::new_v4(),
                order_id: Uuid::new_v4(), // Upbit UUID는 문자열이므로 새 UUID 생성
                exchange: "upbit".to_string(),
                exchange_trade_id: detail.uuid.clone(),
                ticker: detail.market.clone(),
                side,
                quantity,
                price,
                fee,
                fee_currency: "KRW".to_string(), // Upbit은 KRW 마켓 기준
                executed_at,
                is_maker: false, // 주문 수준에서는 maker/taker 구분 불가
                metadata: serde_json::json!({
                    "order_type": detail.ord_type,
                    "state": detail.state,
                    "trades_count": detail.trades_count,
                }),
            });
        }

        Ok(ExecutionHistoryResponse {
            trades,
            next_cursor: None, // Upbit은 커서 기반 페이지네이션 미지원 (limit만 지원)
        })
    }

    fn exchange_name(&self) -> &str {
        "upbit"
    }
}

// ==================== MarketDataProvider ====================

#[async_trait]
impl MarketDataProvider for UpbitExchangeProvider {
    async fn get_quote(&self, symbol: &str) -> Result<QuoteData, ProviderError> {
        self.client.get_quote(symbol).await
    }

    async fn get_quotes(&self, symbols: &[String]) -> Vec<QuoteData> {
        self.client.get_quotes(symbols).await
    }

    fn provider_name(&self) -> &str {
        "upbit"
    }
}

// ==================== OrderExecutionProvider ====================

#[async_trait]
impl OrderExecutionProvider for UpbitExchangeProvider {
    async fn place_order(
        &self,
        request: &OrderRequest,
    ) -> Result<OrderResponse, ProviderError> {
        // Side 변환: Buy → bid, Sell → ask
        let side = match request.side {
            Side::Buy => "bid",
            Side::Sell => "ask",
        };

        // OrderType 변환
        let (ord_type, needs_volume, needs_price) = match request.order_type {
            OrderType::Limit => ("limit", true, true),
            OrderType::Market => {
                match request.side {
                    // 시장가 매수: price(총액) 필수, volume 불필요
                    Side::Buy => ("price", false, true),
                    // 시장가 매도: volume 필수, price 불필요
                    Side::Sell => ("market", true, false),
                }
            }
            OrderType::StopLoss | OrderType::StopLossLimit
            | OrderType::TakeProfit | OrderType::TakeProfitLimit => {
                // Upbit 미지원 → 지정가로 대체
                ("limit", true, true)
            }
            OrderType::TrailingStop => {
                return Err(ProviderError::Unsupported(
                    "Upbit은 트레일링 스톱 주문을 지원하지 않습니다".to_string(),
                ));
            }
        };

        // 수량/가격 문자열 변환
        let volume_str = if needs_volume {
            Some(request.quantity.to_string())
        } else {
            None
        };

        let price_str = if needs_price {
            request.price.map(|p| p.to_string())
        } else {
            None
        };

        info!(
            ticker = %request.ticker,
            side = side,
            ord_type = ord_type,
            volume = ?volume_str,
            price = ?price_str,
            "Upbit 주문 생성"
        );

        let result = self.client.place_order(
            &request.ticker,
            side,
            ord_type,
            volume_str.as_deref(),
            price_str.as_deref(),
        ).await?;

        // 캐시 무효화
        self.cache.invalidate_all().await;

        Ok(OrderResponse {
            order_no: result.uuid,
            order_time: result.created_at,
        })
    }

    async fn cancel_order(&self, order_id: &str, _ticker: &str) -> Result<(), ProviderError> {
        info!(order_id = order_id, "Upbit 주문 취소");

        self.client.cancel_order(order_id).await?;

        // 캐시 무효화
        self.cache.invalidate_all().await;

        Ok(())
    }

    async fn modify_order(
        &self,
        order_id: &str,
        ticker: &str,
        quantity: Option<Decimal>,
        price: Option<Decimal>,
    ) -> Result<OrderResponse, ProviderError> {
        // Upbit는 주문 정정 API 미제공 → cancel + re-place 패턴
        info!(
            order_id = order_id,
            ticker = ticker,
            "Upbit 주문 정정 (cancel + re-place)"
        );

        // 1단계: 기존 주문 조회 (side, ord_type 파악)
        let original = self.client.get_order(order_id).await?;

        // 2단계: 기존 주문 취소
        self.client.cancel_order(order_id).await?;

        // 3단계: 새 주문 생성 (원래 주문 정보 + 변경된 수량/가격)
        let volume = quantity
            .map(|q| q.to_string())
            .or(original.remaining_volume.clone());
        let new_price = price
            .map(|p| p.to_string())
            .or(original.price.clone());

        let result = self.client.place_order(
            &original.market,
            &original.side,
            &original.ord_type,
            volume.as_deref(),
            new_price.as_deref(),
        ).await?;

        // 캐시 무효화
        self.cache.invalidate_all().await;

        Ok(OrderResponse {
            order_no: result.uuid,
            order_time: result.created_at,
        })
    }

    fn exchange_name(&self) -> &str {
        "upbit"
    }
}
