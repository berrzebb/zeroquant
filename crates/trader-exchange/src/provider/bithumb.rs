//! Bithumb ExchangeProvider + MarketDataProvider 구현.
//!
//! BithumbClient를 래핑하여 거래소 중립적인 인터페이스를 제공합니다.

use crate::connector::bithumb::BithumbClient;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info};
use trader_core::cache::ExchangeCache;
use rust_decimal::Decimal;
use trader_core::domain::{
    ExchangeProvider, ExecutionHistoryRequest, ExecutionHistoryResponse, MarketDataProvider,
    OrderExecutionProvider, OrderRequest, OrderResponse, OrderType,
    PendingOrder, ProviderError, QuoteData, Side, StrategyAccountInfo, StrategyPositionInfo,
};

/// Bithumb ExchangeProvider 구현.
///
/// BithumbClient를 래핑하여 캐싱 레이어를 추가합니다.
pub struct BithumbExchangeProvider {
    client: Arc<BithumbClient>,
    cache: Arc<ExchangeCache>,
}

/// 하위 호환성을 위한 타입 별칭.
pub type BithumbProvider = BithumbExchangeProvider;

impl BithumbExchangeProvider {
    /// 새 BithumbExchangeProvider 생성.
    pub fn new(client: Arc<BithumbClient>) -> Self {
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
impl ExchangeProvider for BithumbExchangeProvider {
    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
        if let Some(cached) = self.cache.get_account().await {
            debug!("Bithumb 계좌 정보 캐시 히트");
            return Ok(cached);
        }

        let result = self.client.fetch_account().await?;
        self.cache.set_account(result.clone()).await;
        Ok(result)
    }

    async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        if let Some(cached) = self.cache.get_positions().await {
            debug!("Bithumb 포지션 캐시 히트");
            return Ok(cached);
        }

        let result = self.client.fetch_positions().await?;
        self.cache.set_positions(result.clone()).await;
        Ok(result)
    }

    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError> {
        if let Some(cached) = self.cache.get_pending_orders().await {
            debug!("Bithumb 미체결 주문 캐시 히트");
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
        // Bithumb API는 날짜 필터를 지원하지 않으므로 limit만 사용
        // cursor가 있으면 페이지네이션, 없으면 첫 페이지 (limit: 100)
        let limit = if request.cursor.is_some() {
            // 실제로는 cursor를 page 번호로 해석하거나 추가 로직 필요
            // 여기서는 단순히 100개만 반환
            100
        } else {
            100
        };

        // Bithumb은 특정 심볼 필터를 지원하지만, ExecutionHistoryRequest에는 심볼 필드가 없음
        // 전체 거래 내역 조회
        let trades = self.client.fetch_trades(None, limit).await?;

        // Bithumb API는 limit만 지원하고 cursor 기반 페이지네이션이 명시되지 않음
        // 추가 페이지가 있을 수 있으나 여기서는 단순화
        Ok(ExecutionHistoryResponse {
            trades,
            next_cursor: None,
        })
    }

    fn exchange_name(&self) -> &str {
        "bithumb"
    }
}

// ==================== MarketDataProvider ====================

#[async_trait]
impl MarketDataProvider for BithumbExchangeProvider {
    async fn get_quote(&self, symbol: &str) -> Result<QuoteData, ProviderError> {
        self.client.get_quote(symbol).await
    }

    async fn get_quotes(&self, symbols: &[String]) -> Vec<QuoteData> {
        self.client.get_quotes(symbols).await
    }

    fn provider_name(&self) -> &str {
        "bithumb"
    }
}

// ==================== OrderExecutionProvider ====================

#[async_trait]
impl OrderExecutionProvider for BithumbExchangeProvider {
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
                // Bithumb 미지원 → 지정가로 대체
                ("limit", true, true)
            }
            OrderType::TrailingStop => {
                return Err(ProviderError::Unsupported(
                    "Bithumb은 트레일링 스톱 주문을 지원하지 않습니다".to_string(),
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
            "Bithumb 주문 생성"
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
        info!(order_id = order_id, "Bithumb 주문 취소");

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
        // Bithumb은 주문 정정 API 미제공 → cancel + re-place 패턴
        info!(
            order_id = order_id,
            ticker = ticker,
            "Bithumb 주문 정정 (cancel + re-place)"
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
        "bithumb"
    }
}
