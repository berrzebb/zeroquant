//! Binance ExchangeProvider + MarketDataProvider + OrderExecutionProvider 구현.
//!
//! BinanceClient를 래핑하여 거래소 중립적인 인터페이스를 제공합니다.
//!
//! # 아키텍처
//!
//! ```text
//! BinanceExchangeProvider
//! ├── ExchangeProvider 구현
//! │   ├── fetch_account() - USDT 기준 계좌
//! │   ├── fetch_positions() - 보유 자산 → 포지션 변환
//! │   ├── fetch_pending_orders() - 미체결 주문
//! │   └── fetch_execution_history() - 체결 내역
//! ├── MarketDataProvider 구현
//! │   └── get_quote(symbol) - 24hr 시세
//! ├── OrderExecutionProvider 구현
//! │   ├── place_order() - 주문 제출
//! │   ├── cancel_order() - 주문 취소
//! │   └── modify_order() - Unsupported (Spot 미지원)
//! └── 내부
//!     ├── client: Arc<BinanceClient>
//!     └── cache: Arc<ExchangeCache>
//! ```

use crate::connector::binance::BinanceClient;
use crate::ExchangeError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::sync::Arc;
use tracing::{debug, info, warn};
use trader_core::cache::ExchangeCache;
use trader_core::domain::{
    ExchangeProvider, ExecutionHistoryRequest, ExecutionHistoryResponse, MarketDataProvider,
    OrderExecutionProvider, OrderResponse, PendingOrder, ProviderError, QuoteData, Side,
    StrategyAccountInfo, StrategyPositionInfo, Trade,
};
use uuid::Uuid;

// ==================== 에러 변환 ====================

/// ExchangeError → ProviderError 변환.
fn to_provider_error(e: ExchangeError) -> ProviderError {
    match e {
        ExchangeError::Unauthorized(msg) => ProviderError::Authentication(msg),
        ExchangeError::NetworkError(msg) | ExchangeError::Disconnected(msg) => {
            ProviderError::Network(msg)
        }
        ExchangeError::RateLimited => ProviderError::Api("Rate limit exceeded".to_string()),
        ExchangeError::ParseError(msg) => ProviderError::Parse(msg),
        ExchangeError::NotSupported(msg) => ProviderError::Unsupported(msg),
        other => ProviderError::Api(other.to_string()),
    }
}

// ==================== Provider ====================

/// Binance ExchangeProvider + MarketDataProvider + OrderExecutionProvider 구현.
///
/// BinanceClient를 래핑하여 거래소 중립적인 인터페이스를 제공합니다.
/// ExchangeCache를 통해 반복 API 호출을 줄이고, 주문 실행 후 자동 무효화합니다.
pub struct BinanceExchangeProvider {
    /// Binance REST API 클라이언트
    client: Arc<BinanceClient>,
    /// 거래소 공용 캐시 (계좌, 포지션, 미체결 주문)
    cache: Arc<ExchangeCache>,
}

/// 하위 호환성을 위한 타입 별칭.
pub type BinanceProvider = BinanceExchangeProvider;

impl BinanceExchangeProvider {
    /// 새 BinanceExchangeProvider 생성.
    pub fn new(client: Arc<BinanceClient>) -> Self {
        Self {
            client,
            cache: Arc::new(ExchangeCache::with_defaults()),
        }
    }

    /// BinanceClient에서 생성.
    pub fn from_client(client: BinanceClient) -> Self {
        Self::new(Arc::new(client))
    }

    /// 공용 캐시 참조 반환.
    ///
    /// 외부에서 캐시를 공유해야 하는 경우 사용합니다.
    /// (예: LiveExecutor에서 주문 후 캐시 무효화)
    pub fn exchange_cache(&self) -> Arc<ExchangeCache> {
        Arc::clone(&self.cache)
    }

    /// 모든 캐시 무효화.
    ///
    /// 주문 제출/취소 후 자동 호출되어
    /// 다음 동기화 사이클에서 최신 데이터를 조회합니다.
    async fn invalidate_cache(&self) {
        self.cache.invalidate_all().await;
    }
}

// ==================== ExchangeProvider ====================

#[async_trait]
impl ExchangeProvider for BinanceExchangeProvider {
    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
        // 캐시 확인
        if let Some(cached) = self.cache.get_account().await {
            debug!("Binance 계좌 정보 캐시 히트");
            return Ok(cached);
        }

        let account_info = self
            .client
            .get_account()
            .await
            .map_err(to_provider_error)?;

        // 총 자산 계산 (USDT 기준)
        let mut total_balance = Decimal::ZERO;
        let mut available_balance = Decimal::ZERO;

        for balance in &account_info.balances {
            if balance.asset == "USDT" {
                total_balance += balance.free + balance.locked;
                available_balance += balance.free;
            }
        }

        let result = StrategyAccountInfo {
            total_balance,
            available_balance,
            margin_used: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            currency: "USDT".to_string(),
        };

        // 캐시 저장
        self.cache.set_account(result.clone()).await;
        Ok(result)
    }

    async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        // 캐시 확인
        if let Some(cached) = self.cache.get_positions().await {
            debug!("Binance 포지션 캐시 히트");
            return Ok(cached);
        }

        // Binance Spot은 포지션 개념이 없으므로 보유 자산을 포지션으로 변환
        let account_info = self
            .client
            .get_account()
            .await
            .map_err(to_provider_error)?;

        let mut positions = Vec::new();

        for balance in account_info.balances {
            if balance.free > Decimal::ZERO || balance.locked > Decimal::ZERO {
                let total_qty = balance.free + balance.locked;

                // USDT는 기준 통화이므로 스킵
                if balance.asset == "USDT" {
                    continue;
                }

                let ticker_str = format!("{}/USDT", balance.asset);

                // 현재가 조회 (실패 시 해당 자산 스킵)
                let ticker = match self.client.get_ticker(&ticker_str).await {
                    Ok(t) => t,
                    Err(e) => {
                        warn!(
                            "Binance 시세 조회 실패 ({}), 포지션 스킵: {}",
                            ticker_str, e
                        );
                        continue;
                    }
                };

                // 포지션 생성 (현물은 매수만 가능)
                let mut position = StrategyPositionInfo::new(
                    ticker_str,
                    Side::Buy,
                    total_qty,
                    ticker.last, // 진입가는 현재가로 근사 (실제 평균 매수가 미제공)
                );

                position.update_price(ticker.last);
                positions.push(position);
            }
        }

        // 캐시 저장
        self.cache.set_positions(positions.clone()).await;
        Ok(positions)
    }

    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError> {
        // 캐시 확인
        if let Some(cached) = self.cache.get_pending_orders().await {
            debug!("Binance 미체결 주문 캐시 히트");
            return Ok(cached);
        }

        let orders = self
            .client
            .get_open_orders(None)
            .await
            .map_err(to_provider_error)?;

        let mut pending_orders = Vec::new();

        for order in orders {
            let Some(ticker) = order.ticker else {
                continue;
            };
            let Some(side) = order.side else {
                continue;
            };
            let Some(quantity) = order.quantity else {
                continue;
            };
            let Some(price) = order.price else {
                continue;
            };

            pending_orders.push(PendingOrder {
                order_id: order.order_id,
                ticker,
                side,
                price,
                quantity,
                filled_quantity: order.filled_quantity,
                status: order.status,
                created_at: order.updated_at,
            });
        }

        // 캐시 저장
        self.cache.set_pending_orders(pending_orders.clone()).await;
        Ok(pending_orders)
    }

    async fn fetch_execution_history(
        &self,
        request: &ExecutionHistoryRequest,
    ) -> Result<ExecutionHistoryResponse, ProviderError> {
        // cursor 형식: "SYMBOL|LAST_ID" (예: "BTC/USDT|12345")
        // 첫 요청 시 cursor가 없으면 보유 자산 기반으로 조회
        let (symbol, from_id) = if let Some(ref cursor) = request.cursor {
            let parts: Vec<&str> = cursor.splitn(2, '|').collect();
            if parts.len() == 2 {
                (
                    parts[0].to_string(),
                    parts[1].parse::<u64>().ok(),
                )
            } else {
                return Err(ProviderError::Parse(format!(
                    "잘못된 cursor 형식: {}",
                    cursor
                )));
            }
        } else {
            // cursor가 없으면 첫 요청 - 보유 자산 목록에서 첫 심볼 사용
            let account = self.client.get_account().await.map_err(to_provider_error)?;

            let first_symbol = account
                .balances
                .iter()
                .find(|b| {
                    b.asset != "USDT"
                        && (b.free > Decimal::ZERO || b.locked > Decimal::ZERO)
                })
                .map(|b| format!("{}/USDT", b.asset));

            match first_symbol {
                Some(s) => (s, None),
                None => {
                    // 보유 자산 없음
                    return Ok(ExecutionHistoryResponse {
                        trades: vec![],
                        next_cursor: None,
                    });
                }
            }
        };

        // 날짜 → Unix ms 변환 (from_id 사용 시 날짜 필터 불필요)
        let start_time = if from_id.is_some() {
            None
        } else {
            parse_date_to_millis(&request.start_date)
        };
        let end_time = if from_id.is_some() {
            None
        } else {
            parse_date_to_millis(&request.end_date)
        };

        let my_trades = self
            .client
            .get_my_trades(&symbol, start_time, end_time, from_id, Some(500))
            .await
            .map_err(to_provider_error)?;

        let trades: Vec<Trade> = my_trades
            .iter()
            .map(|t| {
                let side = if t.is_buyer { Side::Buy } else { Side::Sell };
                let price: Decimal = t.price.parse().unwrap_or(Decimal::ZERO);
                let qty: Decimal = t.qty.parse().unwrap_or(Decimal::ZERO);
                let fee: Decimal = t.commission.parse().unwrap_or(Decimal::ZERO);
                let executed_at = DateTime::from_timestamp_millis(t.time)
                    .unwrap_or_else(Utc::now);

                Trade {
                    id: Uuid::new_v4(),
                    order_id: Uuid::nil(), // Binance order_id는 숫자, UUID가 아님
                    exchange: "Binance".to_string(),
                    exchange_trade_id: t.id.to_string(),
                    ticker: symbol.clone(),
                    side,
                    quantity: qty,
                    price,
                    fee,
                    fee_currency: t.commission_asset.clone(),
                    executed_at,
                    is_maker: t.is_maker,
                    metadata: serde_json::json!({
                        "binance_order_id": t.order_id,
                        "quote_qty": t.quote_qty,
                    }),
                }
            })
            .collect();

        // 다음 페이지 커서: 마지막 거래 ID 기반
        let next_cursor = if trades.len() >= 500 {
            my_trades
                .last()
                .map(|t| format!("{}|{}", symbol, t.id))
        } else {
            // 현재 심볼 완료, 다음 심볼로 이동
            let account = self.client.get_account().await.map_err(to_provider_error)?;
            let current_asset = symbol.split('/').next().unwrap_or("");

            let next_symbol = account
                .balances
                .iter()
                .filter(|b| {
                    b.asset != "USDT"
                        && b.asset != current_asset
                        && (b.free > Decimal::ZERO || b.locked > Decimal::ZERO)
                })
                .map(|b| format!("{}/USDT", b.asset))
                .find(|s| s > &symbol); // 알파벳 순서로 다음 심볼

            next_symbol.map(|s| format!("{}|0", s))
        };

        Ok(ExecutionHistoryResponse {
            trades,
            next_cursor,
        })
    }

    fn exchange_name(&self) -> &str {
        "Binance"
    }
}

// ==================== MarketDataProvider ====================

#[async_trait]
impl MarketDataProvider for BinanceExchangeProvider {
    async fn get_quote(&self, symbol: &str) -> Result<QuoteData, ProviderError> {
        let ticker = self
            .client
            .get_ticker(symbol)
            .await
            .map_err(to_provider_error)?;

        Ok(QuoteData {
            symbol: symbol.to_string(),
            current_price: ticker.last,
            price_change: ticker.change_24h,
            change_percent: ticker.change_24h_percent,
            high: ticker.high_24h,
            low: ticker.low_24h,
            open: ticker.last - ticker.change_24h, // 시가 = 현재가 - 24h 변동
            prev_close: ticker.last - ticker.change_24h,
            volume: ticker.volume_24h,
            trading_value: Decimal::ZERO, // Binance Ticker에 quote_volume 미포함
            timestamp: Utc::now(),
        })
    }

    fn provider_name(&self) -> &str {
        "Binance"
    }
}

// ==================== OrderExecutionProvider ====================

#[async_trait]
impl OrderExecutionProvider for BinanceExchangeProvider {
    async fn place_order(
        &self,
        request: &trader_core::domain::OrderRequest,
    ) -> Result<OrderResponse, ProviderError> {
        info!(
            "Binance 주문 제출: {} {} {} @ {:?}",
            request.side, request.quantity, request.ticker, request.price
        );

        let order_id = self
            .client
            .place_order(request)
            .await
            .map_err(to_provider_error)?;

        // 주문 성공 후 캐시 무효화
        self.invalidate_cache().await;

        Ok(OrderResponse {
            order_no: order_id,
            order_time: Utc::now().format("%H%M%S").to_string(),
        })
    }

    async fn cancel_order(&self, order_id: &str, ticker: &str) -> Result<(), ProviderError> {
        info!("Binance 주문 취소: {} ({})", order_id, ticker);

        self.client
            .cancel_order(ticker, order_id)
            .await
            .map_err(to_provider_error)?;

        // 취소 성공 후 캐시 무효화
        self.invalidate_cache().await;

        Ok(())
    }

    async fn modify_order(
        &self,
        _order_id: &str,
        _ticker: &str,
        _quantity: Option<Decimal>,
        _price: Option<Decimal>,
    ) -> Result<OrderResponse, ProviderError> {
        // Binance Spot API는 주문 정정(amend)을 지원하지 않음.
        // 상위 레이어(LiveExecutor)에서 cancel + re-place로 처리해야 함.
        Err(ProviderError::Unsupported(
            "Binance Spot은 주문 정정을 지원하지 않습니다. cancel + place로 처리하세요."
                .to_string(),
        ))
    }

    fn exchange_name(&self) -> &str {
        "Binance"
    }
}

// ==================== 유틸리티 ====================

/// YYYYMMDD 형식의 날짜 문자열을 Unix 밀리초로 변환.
fn parse_date_to_millis(date_str: &str) -> Option<u64> {
    if date_str.len() != 8 {
        return None;
    }

    let naive = chrono::NaiveDate::parse_from_str(date_str, "%Y%m%d").ok()?;
    let datetime = naive
        .and_hms_opt(0, 0, 0)?;
    let utc = chrono::TimeZone::from_utc_datetime(&Utc, &datetime);
    Some(utc.timestamp_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date_to_millis() {
        let millis = parse_date_to_millis("20260101");
        assert!(millis.is_some());

        // 잘못된 형식
        assert!(parse_date_to_millis("2026-01-01").is_none());
        assert!(parse_date_to_millis("").is_none());
    }

    #[test]
    fn test_to_provider_error() {
        let err = to_provider_error(ExchangeError::Unauthorized("bad key".to_string()));
        assert!(matches!(err, ProviderError::Authentication(_)));

        let err = to_provider_error(ExchangeError::NetworkError("timeout".to_string()));
        assert!(matches!(err, ProviderError::Network(_)));

        let err = to_provider_error(ExchangeError::RateLimited);
        assert!(matches!(err, ProviderError::Api(_)));

        let err = to_provider_error(ExchangeError::NotSupported("nope".to_string()));
        assert!(matches!(err, ProviderError::Unsupported(_)));
    }

    #[test]
    fn test_binance_provider_creation() {
        // BinanceClient는 실제 API 키가 필요하므로 구조만 테스트
        // 통합 테스트는 mock을 사용
    }
}
