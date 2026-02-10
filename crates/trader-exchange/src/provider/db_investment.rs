//! DB Investment ExchangeProvider + MarketDataProvider 구현.
//!
//! DbInvestmentClient를 래핑하여 거래소 중립적인 인터페이스를 제공합니다.

use crate::connector::db_investment::DbInvestmentClient;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use rust_decimal::Decimal;
use std::sync::Arc;
use tracing::{debug, info};
use trader_core::cache::ExchangeCache;
use trader_core::domain::{
    ExchangeProvider, ExecutionHistoryRequest, ExecutionHistoryResponse, MarketDataProvider,
    OrderExecutionProvider, OrderRequest, OrderResponse, OrderType, PendingOrder, ProviderError,
    QuoteData, StrategyAccountInfo, StrategyPositionInfo, Trade,
};
use uuid::Uuid;

/// DB Investment ExchangeProvider 구현.
///
/// DbInvestmentClient를 래핑하여 캐싱 레이어를 추가합니다.
pub struct DbInvestmentExchangeProvider {
    client: Arc<DbInvestmentClient>,
    cache: Arc<ExchangeCache>,
}

/// 하위 호환성을 위한 타입 별칭.
pub type DbInvestmentProvider = DbInvestmentExchangeProvider;

impl DbInvestmentExchangeProvider {
    /// 새 DbInvestmentExchangeProvider 생성.
    pub fn new(client: Arc<DbInvestmentClient>) -> Self {
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
impl ExchangeProvider for DbInvestmentExchangeProvider {
    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
        if let Some(cached) = self.cache.get_account().await {
            debug!("DB Investment 계좌 정보 캐시 히트");
            return Ok(cached);
        }

        let result = self.client.fetch_account().await?;
        self.cache.set_account(result.clone()).await;
        Ok(result)
    }

    async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        if let Some(cached) = self.cache.get_positions().await {
            debug!("DB Investment 포지션 캐시 히트");
            return Ok(cached);
        }

        let result = self.client.fetch_positions().await?;
        self.cache.set_positions(result.clone()).await;
        Ok(result)
    }

    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError> {
        if let Some(cached) = self.cache.get_pending_orders().await {
            debug!("DB Investment 미체결 주문 캐시 히트");
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
        // 기본 조회 개수: 100건
        let limit = request.cursor.as_ref()
            .and_then(|c| c.parse::<usize>().ok())
            .unwrap_or(100);

        let executions = self.client.fetch_execution_history(limit).await?;

        let mut trades = Vec::new();
        for exec in executions {
            // 체결시각 파싱 (HHMMSS 형식 → DateTime)
            let executed_at = if exec.exec_time.len() == 6 {
                // HHMMSS → HH:MM:SS
                let today = chrono::Utc::now().format("%Y%m%d").to_string();
                let datetime_str = format!(
                    "{} {}:{}:{}",
                    today,
                    &exec.exec_time[0..2],
                    &exec.exec_time[2..4],
                    &exec.exec_time[4..6]
                );
                NaiveDateTime::parse_from_str(&datetime_str, "%Y%m%d %H:%M:%S")
                    .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
                    .unwrap_or_else(|_| Utc::now())
            } else {
                chrono::Utc::now()
            };

            trades.push(Trade {
                id: Uuid::new_v4(),
                order_id: Uuid::new_v4(), // 주문번호 → UUID 변환 필요 시 별도 매핑
                exchange: "db_investment".to_string(),
                exchange_trade_id: exec.exec_no.clone(),
                ticker: exec.ticker.clone(),
                side: exec.side,
                quantity: exec.exec_qty,
                price: exec.exec_prc,
                fee: exec.fee,
                fee_currency: "KRW".to_string(),
                executed_at,
                is_maker: false, // DB증권 API는 메이커/테이커 구분 제공 안 함
                metadata: serde_json::json!({
                    "order_no": exec.order_no,
                    "exec_no": exec.exec_no,
                }),
            });
        }

        Ok(ExecutionHistoryResponse {
            trades,
            next_cursor: None, // DB증권은 페이지네이션 미지원 (추후 확장 가능)
        })
    }

    fn exchange_name(&self) -> &str {
        "db_investment"
    }
}

// ==================== MarketDataProvider ====================

#[async_trait]
impl MarketDataProvider for DbInvestmentExchangeProvider {
    async fn get_quote(&self, symbol: &str) -> Result<QuoteData, ProviderError> {
        self.client.get_quote(symbol).await
    }

    async fn get_quotes(&self, symbols: &[String]) -> Vec<QuoteData> {
        self.client.get_quotes(symbols).await
    }

    fn provider_name(&self) -> &str {
        "db_investment"
    }
}

// ==================== OrderExecutionProvider ====================

#[async_trait]
impl OrderExecutionProvider for DbInvestmentExchangeProvider {
    async fn place_order(
        &self,
        request: &OrderRequest,
    ) -> Result<OrderResponse, ProviderError> {
        // OrderType → DB증권 호가 유형 코드 변환
        let order_class = match request.order_type {
            OrderType::Market => "01",
            OrderType::Limit => "00",
            OrderType::StopLoss | OrderType::StopLossLimit => "00",
            OrderType::TakeProfit | OrderType::TakeProfitLimit => "00",
            OrderType::TrailingStop => {
                return Err(ProviderError::Unsupported(
                    "DB증권은 트레일링 스톱 주문을 지원하지 않습니다".to_string(),
                ));
            }
        };

        // Decimal 수량 → u32 변환 (소수점 절사)
        let quantity = request
            .quantity
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
            "DB증권 주문 생성"
        );

        let response = self
            .client
            .place_order(&request.ticker, request.side, quantity, price, order_class)
            .await?;

        // 캐시 무효화 (주문 후 포지션/계좌 변동)
        self.cache.invalidate_all().await;

        Ok(response)
    }

    async fn cancel_order(&self, order_id: &str, _ticker: &str) -> Result<(), ProviderError> {
        info!(order_id = order_id, "DB증권 주문 취소");

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
        info!(
            order_id = order_id,
            ticker = ticker,
            quantity = ?quantity,
            price = ?price,
            "DB증권 주문 정정"
        );

        // 원래 주문 조회하여 정정할 값 결정
        let pending = self.client.fetch_pending_orders().await?;
        let original_order = pending
            .iter()
            .find(|o| o.order_id == order_id)
            .ok_or_else(|| {
                ProviderError::Api(format!("정정할 주문을 찾을 수 없습니다: {}", order_id))
            })?;

        let qty = quantity
            .map(|q| {
                q.to_string()
                    .parse::<f64>()
                    .map(|v| v.floor() as u32)
                    .unwrap_or(0)
            })
            .unwrap_or_else(|| {
                original_order
                    .quantity
                    .to_string()
                    .parse::<f64>()
                    .map(|v| v.floor() as u32)
                    .unwrap_or(0)
            });

        let order_price = price.unwrap_or(original_order.price);

        let response = self.client.modify_order(order_id, qty, order_price).await?;

        // 캐시 무효화
        self.cache.invalidate_all().await;

        Ok(response)
    }

    fn exchange_name(&self) -> &str {
        "db_investment"
    }
}
