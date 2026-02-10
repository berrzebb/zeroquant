//! LS Securities ExchangeProvider + MarketDataProvider 구현.
//!
//! LsSecClient를 래핑하여 거래소 중립적인 인터페이스를 제공합니다.

use crate::connector::ls_sec::LsSecClient;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info};
use rust_decimal::Decimal;
use trader_core::cache::ExchangeCache;
use trader_core::domain::{
    ExchangeProvider, ExecutionHistoryRequest, ExecutionHistoryResponse, MarketDataProvider,
    PendingOrder, ProviderError, QuoteData, StrategyAccountInfo, StrategyPositionInfo,
    OrderRequest, OrderResponse, OrderType, Side, OrderExecutionProvider,
};

/// LS Securities ExchangeProvider 구현.
///
/// LsSecClient를 래핑하여 캐싱 레이어를 추가합니다.
pub struct LsSecExchangeProvider {
    client: Arc<LsSecClient>,
    cache: Arc<ExchangeCache>,
}

/// 하위 호환성을 위한 타입 별칭.
pub type LsSecProvider = LsSecExchangeProvider;

impl LsSecExchangeProvider {
    /// 새 LsSecExchangeProvider 생성.
    pub fn new(client: Arc<LsSecClient>) -> Self {
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
impl ExchangeProvider for LsSecExchangeProvider {
    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
        if let Some(cached) = self.cache.get_account().await {
            debug!("LS Securities 계좌 정보 캐시 히트");
            return Ok(cached);
        }

        let result = self.client.fetch_account().await?;
        self.cache.set_account(result.clone()).await;
        Ok(result)
    }

    async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        if let Some(cached) = self.cache.get_positions().await {
            debug!("LS Securities 포지션 캐시 히트");
            return Ok(cached);
        }

        let result = self.client.fetch_positions().await?;
        self.cache.set_positions(result.clone()).await;
        Ok(result)
    }

    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError> {
        if let Some(cached) = self.cache.get_pending_orders().await {
            debug!("LS Securities 미체결 주문 캐시 히트");
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
        info!(
            start_date = %request.start_date,
            end_date = %request.end_date,
            "LS증권 체결 내역 조회"
        );

        // LS증권 API는 symbol 파라미터를 지원하지만 ExecutionHistoryRequest에는 없음
        // 필요 시 확장 가능
        let trades = self.client.fetch_execution_history(
            &request.start_date,
            &request.end_date,
            None,  // symbol
        ).await?;

        // LS증권 API는 페이지네이션을 지원하지 않으므로 next_cursor는 None
        Ok(ExecutionHistoryResponse {
            trades,
            next_cursor: None,
        })
    }

    fn exchange_name(&self) -> &str {
        "ls_securities"
    }
}

// ==================== MarketDataProvider ====================

#[async_trait]
impl MarketDataProvider for LsSecExchangeProvider {
    async fn get_quote(&self, symbol: &str) -> Result<QuoteData, ProviderError> {
        self.client.get_quote(symbol).await
    }

    async fn get_quotes(&self, symbols: &[String]) -> Vec<QuoteData> {
        self.client.get_quotes(symbols).await
    }

    fn provider_name(&self) -> &str {
        "ls_securities"
    }
}

// ==================== OrderExecutionProvider ====================

#[async_trait]
impl OrderExecutionProvider for LsSecExchangeProvider {
    async fn place_order(
        &self,
        request: &OrderRequest,
    ) -> Result<OrderResponse, ProviderError> {
        // OrderType → LS 주문 구분 코드 변환
        let order_class = match request.order_type {
            OrderType::Market => "01",
            OrderType::Limit => "00",
            OrderType::StopLoss | OrderType::StopLossLimit => "00",
            OrderType::TakeProfit | OrderType::TakeProfitLimit => "00",
            OrderType::TrailingStop => {
                return Err(ProviderError::Unsupported(
                    "LS증권은 트레일링 스톱 주문을 지원하지 않습니다".to_string(),
                ));
            }
        };

        // Decimal 수량 → u32 변환 (소수점 절사)
        let quantity = request.quantity
            .to_string()
            .parse::<f64>()
            .map(|v| v.floor() as u32)
            .map_err(|e| ProviderError::Parse(format!("수량 변환 실패: {}", e)))?;

        if quantity == 0 {
            return Err(ProviderError::Api(
                "주문 수량은 1 이상이어야 합니다".to_string(),
            ));
        }

        // 가격 결정 (시장가인 경우 0)
        let price = match request.order_type {
            OrderType::Market => Decimal::ZERO,
            _ => request.price.or(request.stop_price).unwrap_or(Decimal::ZERO),
        };

        info!(
            ticker = %request.ticker,
            side = ?request.side,
            quantity = quantity,
            price = %price,
            order_class = order_class,
            "LS증권 주문 생성"
        );

        let response = self.client.place_order(
            &request.ticker,
            request.side,
            quantity,
            price,
            order_class,
        ).await?;

        // 캐시 무효화 (주문 후 포지션/계좌 변동)
        self.cache.invalidate_all().await;

        Ok(response)
    }

    async fn cancel_order(&self, order_id: &str, ticker: &str) -> Result<(), ProviderError> {
        info!(order_id = order_id, ticker = ticker, "LS증권 주문 취소");

        // 취소 시 원래 매수/매도 방향을 알아야 TR 코드를 결정할 수 있음
        // 미체결 주문 조회를 통해 방향 파악
        let pending = self.client.fetch_pending_orders().await?;
        let original_order = pending.iter().find(|o| o.order_id == order_id);

        let (side, qty) = match original_order {
            Some(order) => (order.side, order.quantity),
            None => {
                // 주문을 찾을 수 없으면 매수 취소로 시도 후 실패 시 매도 취소 시도
                let qty_u32 = 0u32; // 전량 취소
                match self.client.cancel_order(order_id, ticker, Side::Buy, qty_u32).await {
                    Ok(_res) => {
                        self.cache.invalidate_all().await;
                        return Ok(());
                    }
                    Err(_) => {
                        let _res = self.client.cancel_order(order_id, ticker, Side::Sell, qty_u32).await?;
                        self.cache.invalidate_all().await;
                        return Ok(());
                    }
                }
            }
        };

        let qty_u32 = qty.to_string()
            .parse::<f64>()
            .map(|v| v.floor() as u32)
            .unwrap_or(0);

        self.client.cancel_order(order_id, ticker, side, qty_u32).await?;
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
        info!(
            order_id = order_id,
            ticker = ticker,
            quantity = ?quantity,
            price = ?price,
            "LS증권 주문 정정"
        );

        // 원래 주문 방향 파악
        let pending = self.client.fetch_pending_orders().await?;
        let original_order = pending.iter().find(|o| o.order_id == order_id)
            .ok_or_else(|| ProviderError::Api(format!(
                "정정할 주문을 찾을 수 없습니다: {}", order_id
            )))?;

        let side = original_order.side;

        let qty = quantity
            .map(|q| q.to_string().parse::<f64>().map(|v| v.floor() as u32).unwrap_or(0))
            .unwrap_or_else(|| {
                original_order.quantity.to_string()
                    .parse::<f64>()
                    .map(|v| v.floor() as u32)
                    .unwrap_or(0)
            });

        let order_price = price.unwrap_or(original_order.price);

        let response = self.client.modify_order(
            order_id, ticker, side, qty, order_price
        ).await?;

        self.cache.invalidate_all().await;

        Ok(response)
    }

    fn exchange_name(&self) -> &str {
        "ls_securities"
    }
}
