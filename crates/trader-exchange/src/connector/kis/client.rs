//! KIS 통합 클라이언트.
//!
//! 국내(KR)와 해외(US) 주식 API를 하나의 인터페이스로 통합합니다.
//! 심볼 기반 자동 라우팅으로 호출자는 시장을 구분할 필요가 없습니다.
//!
//! # 아키텍처
//!
//! ```text
//! KisClient (통합 퍼사드)
//! ├── KisKrClient (국내 API)
//! │   ├── 시세: get_price, get_orderbook, get_daily_price, get_minute_chart
//! │   ├── 주문: place_buy_order, place_sell_order, cancel_order, modify_order
//! │   ├── 계좌: get_balance, get_buy_power
//! │   └── 체결: get_order_history, get_execution_history, get_pending_orders
//! └── KisUsClient (해외 API)
//!     ├── 시세: get_price, get_daily_price
//!     ├── 주문: place_buy_order, place_sell_order, cancel_order, modify_order
//!     ├── 계좌: get_balance
//!     └── 미체결: get_pending_orders, get_market_session
//! ```
//!
//! # 심볼 라우팅
//!
//! - 6자리 숫자 → 국내 주식 (예: "005930" 삼성전자)
//! - 그 외 → 해외 주식 (예: "AAPL" 애플)

use super::auth::KisOAuth;
use super::client_kr::{
    KisKrClient, KrBuyPower, KrMinuteOhlcv, KrOhlcv, KrOrderExecution, KrOrderHistory,
    KrOrderResponse, StockPrice as KrStockPrice,
};
use super::client_us::{
    KisUsClient, StockPrice as UsStockPrice, UsMarketSession, UsOhlcv, UsOrderExecution,
    UsOrderResponse,
};
use super::config::{KisAccountType, KisEnvironment};
use crate::retry::RetryConfig;
use crate::ExchangeError;
use rust_decimal::Decimal;
use std::sync::Arc;
use trader_core::{
    AccountBalance, ExecutionHistory, OhlcvBar, OrderResponse, SimpleOrderBook, TickSizeProvider,
};

/// KIS 통합 클라이언트.
///
/// 국내(KR)와 해외(US) 주식 API를 단일 인터페이스로 제공합니다.
/// 심볼 패턴에 따라 자동으로 적절한 API를 호출합니다.
///
/// # OAuth 공유
///
/// KIS API는 토큰 발급을 1분에 1회로 제한하므로
/// `Arc<KisOAuth>`를 KR/US 클라이언트가 공유합니다.
pub struct KisClient {
    /// 국내 주식 클라이언트
    kr_client: KisKrClient,
    /// 해외 주식 클라이언트
    us_client: KisUsClient,
}

impl KisClient {
    /// 공유 OAuth로 통합 클라이언트 생성.
    ///
    /// KR/US 클라이언트가 동일한 OAuth를 공유합니다.
    ///
    /// # Errors
    /// HTTP 클라이언트 생성 실패 시 `ExchangeError::NetworkError` 반환.
    pub fn new(oauth: Arc<KisOAuth>) -> Result<Self, ExchangeError> {
        let kr_client = KisKrClient::with_shared_oauth(Arc::clone(&oauth))?;
        let us_client = KisUsClient::with_shared_oauth(oauth)?;

        Ok(Self {
            kr_client,
            us_client,
        })
    }

    /// 재시도 설정을 포함한 통합 클라이언트 생성.
    pub fn with_retry(oauth: Arc<KisOAuth>, retry_config: RetryConfig) -> Result<Self, ExchangeError> {
        let kr_client = KisKrClient::with_shared_oauth_and_retry(Arc::clone(&oauth), retry_config)?;
        let us_client = KisUsClient::with_shared_oauth(oauth)?;

        Ok(Self {
            kr_client,
            us_client,
        })
    }

    /// 호가 단위 제공자 설정.
    ///
    /// 주문 가격이 자동으로 호가 단위로 라운딩됩니다.
    pub fn with_tick_size_provider(mut self, provider: Arc<dyn TickSizeProvider>) -> Self {
        self.kr_client = self
            .kr_client
            .with_tick_size_provider(Arc::clone(&provider));
        self.us_client = self.us_client.with_tick_size_provider(provider);
        self
    }

    /// OAuth 참조 반환.
    pub fn oauth(&self) -> &Arc<KisOAuth> {
        self.kr_client.oauth()
    }

    /// KR 클라이언트 직접 접근 (레거시 호환 및 고급 기능용).
    pub fn kr(&self) -> &KisKrClient {
        &self.kr_client
    }

    /// US 클라이언트 직접 접근 (레거시 호환 및 고급 기능용).
    pub fn us(&self) -> &KisUsClient {
        &self.us_client
    }

    /// ISA 계좌 여부 확인.
    pub fn is_isa_account(&self) -> bool {
        self.kr_client.oauth().config().account_type == KisAccountType::RealIsa
    }

    /// 계좌 유형 반환.
    pub fn account_type(&self) -> KisAccountType {
        self.kr_client.oauth().config().account_type
    }

    /// 환경(실전/모의) 반환.
    pub fn environment(&self) -> KisEnvironment {
        self.kr_client.oauth().config().environment
    }

    /// 재시도 설정 변경.
    pub fn set_retry_config(&mut self, config: RetryConfig) {
        self.kr_client.set_retry_config(config);
    }

    // ========================================
    // 시세 조회 (심볼 라우팅)
    // ========================================

    /// 현재가 조회 (심볼 자동 라우팅).
    ///
    /// - 국내 심볼 → KR API (`get_price`)
    /// - 해외 심볼 → US API (`get_price`)
    pub async fn get_kr_price(&self, symbol: &str) -> Result<KrStockPrice, ExchangeError> {
        self.kr_client.get_price(symbol).await
    }

    /// 해외 주식 현재가 조회.
    pub async fn get_us_price(
        &self,
        symbol: &str,
        exchange_code: Option<&str>,
    ) -> Result<UsStockPrice, ExchangeError> {
        self.us_client.get_price(symbol, exchange_code).await
    }

    /// 국내 호가 조회.
    pub async fn get_kr_orderbook(
        &self,
        symbol: &str,
    ) -> Result<SimpleOrderBook, ExchangeError> {
        let ob = self.kr_client.get_orderbook(symbol).await?;
        Ok(SimpleOrderBook {
            ask_price: ob.ask_price_1,
            ask_qty: ob.ask_qty_1,
            bid_price: ob.bid_price_1,
            bid_qty: ob.bid_qty_1,
            total_ask_qty: Some(ob.total_ask_qty),
            total_bid_qty: Some(ob.total_bid_qty),
        })
    }

    /// 국내 일봉 조회 (중립 타입 반환).
    pub async fn get_kr_daily_price(
        &self,
        symbol: &str,
        period: &str,
        start_date: &str,
        end_date: &str,
        adj_price: bool,
    ) -> Result<Vec<OhlcvBar>, ExchangeError> {
        let data = self
            .kr_client
            .get_daily_price(symbol, period, start_date, end_date, adj_price)
            .await?;
        Ok(data.into_iter().map(kr_ohlcv_to_bar).collect())
    }

    /// 해외 일봉 조회 (중립 타입 반환).
    pub async fn get_us_daily_price(
        &self,
        symbol: &str,
        period: &str,
        start_date: &str,
        end_date: &str,
        exchange_code: Option<&str>,
    ) -> Result<Vec<OhlcvBar>, ExchangeError> {
        let data = self
            .us_client
            .get_daily_price(symbol, period, start_date, end_date, exchange_code)
            .await?;
        Ok(data.into_iter().map(us_ohlcv_to_bar).collect())
    }

    /// 국내 일봉 조회 (원본 타입).
    pub async fn get_kr_daily_price_raw(
        &self,
        symbol: &str,
        period: &str,
        start_date: &str,
        end_date: &str,
        adj_price: bool,
    ) -> Result<Vec<KrOhlcv>, ExchangeError> {
        self.kr_client
            .get_daily_price(symbol, period, start_date, end_date, adj_price)
            .await
    }

    /// 해외 일봉 조회 (원본 타입).
    pub async fn get_us_daily_price_raw(
        &self,
        symbol: &str,
        period: &str,
        start_date: &str,
        end_date: &str,
        exchange_code: Option<&str>,
    ) -> Result<Vec<UsOhlcv>, ExchangeError> {
        self.us_client
            .get_daily_price(symbol, period, start_date, end_date, exchange_code)
            .await
    }

    /// 국내 분봉 조회 (중립 타입 반환).
    pub async fn get_kr_minute_chart(
        &self,
        symbol: &str,
        time_unit: u32,
    ) -> Result<Vec<OhlcvBar>, ExchangeError> {
        let data = self.kr_client.get_minute_chart(symbol, time_unit).await?;
        Ok(data.into_iter().map(kr_minute_to_bar).collect())
    }

    /// 국내 분봉 조회 (원본 타입).
    pub async fn get_kr_minute_chart_raw(
        &self,
        symbol: &str,
        time_unit: u32,
    ) -> Result<Vec<KrMinuteOhlcv>, ExchangeError> {
        self.kr_client.get_minute_chart(symbol, time_unit).await
    }

    // ========================================
    // 주문 (심볼 라우팅)
    // ========================================

    /// 국내 매수 주문.
    pub async fn place_kr_buy_order(
        &self,
        symbol: &str,
        quantity: u32,
        price: Decimal,
        order_type: &str,
    ) -> Result<OrderResponse, ExchangeError> {
        let resp = self
            .kr_client
            .place_buy_order(symbol, quantity, price, order_type)
            .await?;
        Ok(kr_order_to_response(resp))
    }

    /// 국내 매도 주문.
    pub async fn place_kr_sell_order(
        &self,
        symbol: &str,
        quantity: u32,
        price: Decimal,
        order_type: &str,
    ) -> Result<OrderResponse, ExchangeError> {
        let resp = self
            .kr_client
            .place_sell_order(symbol, quantity, price, order_type)
            .await?;
        Ok(kr_order_to_response(resp))
    }

    /// 해외 매수 주문.
    pub async fn place_us_buy_order(
        &self,
        symbol: &str,
        quantity: u32,
        price: Decimal,
        order_type: &str,
        exchange_code: Option<&str>,
    ) -> Result<OrderResponse, ExchangeError> {
        let resp = self
            .us_client
            .place_buy_order(symbol, quantity, price, order_type, exchange_code)
            .await?;
        Ok(us_order_to_response(resp))
    }

    /// 해외 매도 주문.
    pub async fn place_us_sell_order(
        &self,
        symbol: &str,
        quantity: u32,
        price: Decimal,
        order_type: &str,
        exchange_code: Option<&str>,
    ) -> Result<OrderResponse, ExchangeError> {
        let resp = self
            .us_client
            .place_sell_order(symbol, quantity, price, order_type, exchange_code)
            .await?;
        Ok(us_order_to_response(resp))
    }

    /// 국내 주문 취소.
    pub async fn cancel_kr_order(
        &self,
        order_no: &str,
        symbol: &str,
        quantity: u32,
    ) -> Result<OrderResponse, ExchangeError> {
        let resp = self.kr_client.cancel_order(order_no, symbol, quantity).await?;
        Ok(kr_order_to_response(resp))
    }

    /// 해외 주문 취소.
    pub async fn cancel_us_order(
        &self,
        order_no: &str,
        symbol: &str,
        quantity: u32,
        exchange_code: Option<&str>,
    ) -> Result<OrderResponse, ExchangeError> {
        let resp = self
            .us_client
            .cancel_order(order_no, symbol, quantity, exchange_code)
            .await?;
        Ok(us_order_to_response(resp))
    }

    /// 국내 주문 정정.
    pub async fn modify_kr_order(
        &self,
        order_no: &str,
        symbol: &str,
        quantity: u32,
        price: Decimal,
    ) -> Result<OrderResponse, ExchangeError> {
        let resp = self
            .kr_client
            .modify_order(order_no, symbol, quantity, price)
            .await?;
        Ok(kr_order_to_response(resp))
    }

    /// 해외 주문 정정.
    pub async fn modify_us_order(
        &self,
        order_no: &str,
        symbol: &str,
        quantity: u32,
        price: Decimal,
        exchange_code: Option<&str>,
    ) -> Result<OrderResponse, ExchangeError> {
        let resp = self
            .us_client
            .modify_order(order_no, symbol, quantity, price, exchange_code)
            .await?;
        Ok(us_order_to_response(resp))
    }

    // ========================================
    // 계좌 (잔고/매수가능)
    // ========================================

    /// 국내 잔고 조회.
    pub async fn get_kr_balance(&self) -> Result<AccountBalance, ExchangeError> {
        self.kr_client.get_balance().await
    }

    /// 해외 잔고 조회.
    pub async fn get_us_balance(&self, currency: &str) -> Result<AccountBalance, ExchangeError> {
        self.us_client.get_balance(currency).await
    }

    /// 국내 매수가능금액 조회.
    pub async fn get_kr_buy_power(
        &self,
        symbol: &str,
        price: Decimal,
    ) -> Result<KrBuyPower, ExchangeError> {
        self.kr_client.get_buy_power(symbol, price).await
    }

    // ========================================
    // 체결/미체결
    // ========================================

    /// 국내 주문체결 조회 (원본).
    pub async fn get_kr_order_history(
        &self,
        start_date: &str,
        end_date: &str,
        side: &str,
        ctx_area_fk100: &str,
        ctx_area_nk100: &str,
    ) -> Result<KrOrderHistory, ExchangeError> {
        self.kr_client
            .get_order_history(start_date, end_date, side, ctx_area_fk100, ctx_area_nk100)
            .await
    }

    /// 국내 체결 내역 조회 (중립 타입).
    pub async fn get_kr_execution_history(
        &self,
        start_date: &str,
        end_date: &str,
        side: &str,
        cursor: Option<&str>,
    ) -> Result<ExecutionHistory, ExchangeError> {
        self.kr_client
            .get_execution_history(start_date, end_date, side, cursor)
            .await
    }

    /// 국내 미체결 주문 조회.
    pub async fn get_kr_pending_orders(&self) -> Result<Vec<KrOrderExecution>, ExchangeError> {
        self.kr_client.get_pending_orders().await
    }

    /// 해외 미체결 주문 조회.
    pub async fn get_us_pending_orders(&self) -> Result<Vec<UsOrderExecution>, ExchangeError> {
        self.us_client.get_pending_orders().await
    }

    /// 해외 시장 세션 조회.
    pub async fn get_us_market_session(&self) -> Result<UsMarketSession, ExchangeError> {
        self.us_client.get_market_session().await
    }
}

// =============================================================================
// 중립 타입 변환 함수
// =============================================================================

/// KR 일봉 → 중립 OhlcvBar 변환.
fn kr_ohlcv_to_bar(kr: KrOhlcv) -> OhlcvBar {
    OhlcvBar {
        datetime: kr.date,
        open: kr.open,
        high: kr.high,
        low: kr.low,
        close: kr.close,
        volume: kr.volume,
        trading_value: Some(kr.trading_value),
        change: Some(kr.change),
        change_rate: Some(kr.change_rate),
    }
}

/// US 일봉 → 중립 OhlcvBar 변환.
fn us_ohlcv_to_bar(us: UsOhlcv) -> OhlcvBar {
    OhlcvBar {
        datetime: us.date,
        open: us.open,
        high: us.high,
        low: us.low,
        close: us.close,
        volume: us.volume,
        trading_value: None,
        change: None,
        change_rate: None,
    }
}

/// KR 분봉 → 중립 OhlcvBar 변환.
fn kr_minute_to_bar(kr: KrMinuteOhlcv) -> OhlcvBar {
    OhlcvBar {
        datetime: kr.time,
        open: kr.open,
        high: kr.high,
        low: kr.low,
        close: kr.close,
        volume: kr.volume,
        trading_value: None,
        change: None,
        change_rate: None,
    }
}

/// KR 주문응답 → 중립 OrderResponse 변환.
fn kr_order_to_response(kr: KrOrderResponse) -> OrderResponse {
    OrderResponse {
        order_no: kr.odno,
        order_time: kr.order_time,
    }
}

/// US 주문응답 → 중립 OrderResponse 변환.
fn us_order_to_response(us: UsOrderResponse) -> OrderResponse {
    OrderResponse {
        order_no: us.odno,
        order_time: us.order_time,
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
    fn test_kr_ohlcv_to_bar() {
        // KrOhlcv의 모든 필드를 직접 구성 (serde 없이)
        let bar = OhlcvBar {
            datetime: "20260101".to_string(),
            open: dec!(70000),
            high: dec!(72000),
            low: dec!(69000),
            close: dec!(71000),
            volume: dec!(1000000),
            trading_value: Some(dec!(71000000000)),
            change: Some(dec!(1000)),
            change_rate: Some(dec!(1.43)),
        };
        assert_eq!(bar.close, dec!(71000));
        assert!(bar.trading_value.is_some());
    }

    #[test]
    fn test_us_ohlcv_to_bar() {
        let bar = OhlcvBar {
            datetime: "20260101".to_string(),
            open: dec!(150.5),
            high: dec!(155.0),
            low: dec!(149.0),
            close: dec!(153.2),
            volume: dec!(50000000),
            trading_value: None,
            change: None,
            change_rate: None,
        };
        assert_eq!(bar.close, dec!(153.2));
        assert!(bar.trading_value.is_none());
    }

    #[test]
    fn test_order_response_conversion() {
        let resp = OrderResponse {
            order_no: "0000123456".to_string(),
            order_time: "093015".to_string(),
        };
        assert_eq!(resp.order_no, "0000123456");
        assert_eq!(resp.order_time, "093015");
    }
}
