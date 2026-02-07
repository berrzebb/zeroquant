//! 거래소 정보 제공자 추상화.
//!
//! 다양한 거래소로부터 계좌 정보, 포지션, 미체결 주문을 조회하기 위한
//! 거래소 중립적인 인터페이스를 제공합니다.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{PendingOrder, StrategyAccountInfo, StrategyPositionInfo, Trade};

// =============================================================================
// 요청/응답 타입
// =============================================================================

/// 체결 내역 조회 요청.
#[derive(Debug, Clone)]
pub struct ExecutionHistoryRequest {
    /// 조회 시작 날짜 (YYYYMMDD 형식)
    pub start_date: String,
    /// 조회 종료 날짜 (YYYYMMDD 형식)
    pub end_date: String,
    /// 매수/매도 구분 (거래소별로 다름, 예: "00"=전체, "01"=매도, "02"=매수)
    pub side: Option<String>,
    /// 페이지네이션 커서 (거래소별로 다름)
    pub cursor: Option<String>,
}

impl ExecutionHistoryRequest {
    /// 새 요청 생성.
    pub fn new(start_date: impl Into<String>, end_date: impl Into<String>) -> Self {
        Self {
            start_date: start_date.into(),
            end_date: end_date.into(),
            side: None,
            cursor: None,
        }
    }

    /// 매수/매도 구분 설정.
    pub fn with_side(mut self, side: impl Into<String>) -> Self {
        self.side = Some(side.into());
        self
    }

    /// 커서 설정.
    pub fn with_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }
}

/// 체결 내역 조회 응답.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionHistoryResponse {
    /// 체결 내역 목록
    pub trades: Vec<Trade>,
    /// 다음 페이지 커서 (없으면 None)
    pub next_cursor: Option<String>,
}

// =============================================================================
// 에러 타입
// =============================================================================

/// ExchangeProvider 에러.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// 네트워크 에러
    #[error("네트워크 에러: {0}")]
    Network(String),

    /// 인증 실패
    #[error("인증 실패: {0}")]
    Authentication(String),

    /// API 에러
    #[error("API 에러: {0}")]
    Api(String),

    /// 파싱 에러
    #[error("파싱 에러: {0}")]
    Parse(String),

    /// 지원하지 않는 기능
    #[error("지원하지 않는 기능: {0}")]
    Unsupported(String),

    /// 기타 에러
    #[error("기타 에러: {0}")]
    Other(String),
}

// =============================================================================
// ExchangeProvider Trait
// =============================================================================

/// 거래소 정보 제공자 trait.
///
/// 거래소로부터 실시간 계좌 정보, 포지션, 미체결 주문을 조회합니다.
/// 각 거래소별로 이 trait를 구현하여 거래소 중립적인 코드를 작성할 수 있습니다.
///
/// # 구현 예시
///
/// ```ignore
/// pub struct BinanceProvider {
///     client: Arc<BinanceClient>,
/// }
///
/// #[async_trait]
/// impl ExchangeProvider for BinanceProvider {
///     async fn fetch_account(&self) -> Result<AccountInfo, ProviderError> {
///         // Binance API 호출 및 변환
///     }
///
///     // ... 나머지 메서드 구현
/// }
/// ```
#[async_trait]
pub trait ExchangeProvider: Send + Sync {
    /// 계좌 정보 조회.
    ///
    /// 총 자산, 사용 가능 금액, 증거금, 미실현 손익 등을 조회합니다.
    ///
    /// # Errors
    ///
    /// - `ProviderError::Network`: 네트워크 연결 실패
    /// - `ProviderError::Authentication`: 인증 실패 (API 키 오류 등)
    /// - `ProviderError::Api`: 거래소 API 에러
    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError>;

    /// 현재 보유 포지션 조회.
    ///
    /// 모든 보유 포지션의 상세 정보를 조회합니다.
    /// 현물 거래소의 경우 보유 자산을 포지션으로 변환합니다.
    ///
    /// # Returns
    ///
    /// 포지션 목록. 포지션이 없으면 빈 벡터 반환.
    ///
    /// # Errors
    ///
    /// - `ProviderError::Network`: 네트워크 연결 실패
    /// - `ProviderError::Authentication`: 인증 실패
    /// - `ProviderError::Api`: 거래소 API 에러
    async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError>;

    /// 미체결 주문 조회.
    ///
    /// 현재 대기 중이거나 부분 체결된 주문 목록을 조회합니다.
    ///
    /// # Returns
    ///
    /// 미체결 주문 목록. 미체결 주문이 없으면 빈 벡터 반환.
    ///
    /// # Errors
    ///
    /// - `ProviderError::Network`: 네트워크 연결 실패
    /// - `ProviderError::Authentication`: 인증 실패
    /// - `ProviderError::Api`: 거래소 API 에러
    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError>;

    /// 거래소 이름 반환.
    ///
    /// 로깅 및 디버깅 목적으로 사용됩니다.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let provider = BinanceProvider::new(client);
    /// assert_eq!(provider.exchange_name(), "Binance");
    /// ```
    fn exchange_name(&self) -> &str;

    /// 체결 내역 조회.
    ///
    /// 지정된 기간 동안의 체결 내역을 조회합니다.
    /// 거래소마다 페이지네이션 방식이 다를 수 있으므로 커서를 지원합니다.
    ///
    /// # Arguments
    ///
    /// * `request` - 체결 내역 조회 요청 (날짜, side, 커서 포함)
    ///
    /// # Returns
    ///
    /// 체결 내역 목록 및 다음 페이지 커서.
    ///
    /// # Errors
    ///
    /// - `ProviderError::Network`: 네트워크 연결 실패
    /// - `ProviderError::Authentication`: 인증 실패
    /// - `ProviderError::Api`: 거래소 API 에러
    /// - `ProviderError::Unsupported`: 거래소가 체결 내역 조회를 지원하지 않음
    ///
    /// # 기본 구현
    ///
    /// 기본적으로 `Unsupported` 에러를 반환합니다.
    /// 거래소별로 이 메서드를 구현하여 체결 내역 조회를 지원할 수 있습니다.
    async fn fetch_execution_history(
        &self,
        _request: &ExecutionHistoryRequest,
    ) -> Result<ExecutionHistoryResponse, ProviderError> {
        Err(ProviderError::Unsupported(
            "이 거래소는 체결 내역 조회를 지원하지 않습니다".to_string(),
        ))
    }
}

// =============================================================================
// MarketDataProvider Trait
// =============================================================================

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// 시세 데이터 조회 결과.
///
/// 거래소별로 다른 시세 데이터를 통일된 형식으로 제공합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteData {
    /// 종목 심볼
    pub symbol: String,
    /// 현재가
    pub current_price: Decimal,
    /// 전일대비 가격 변동
    pub price_change: Decimal,
    /// 전일대비 변동률 (%)
    pub change_percent: Decimal,
    /// 당일 고가
    pub high: Decimal,
    /// 당일 저가
    pub low: Decimal,
    /// 당일 시가
    pub open: Decimal,
    /// 전일 종가
    pub prev_close: Decimal,
    /// 거래량
    pub volume: Decimal,
    /// 거래대금
    pub trading_value: Decimal,
    /// 조회 시각
    pub timestamp: DateTime<Utc>,
}

impl QuoteData {
    /// 새 시세 데이터 생성.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol: impl Into<String>,
        current_price: Decimal,
        price_change: Decimal,
        change_percent: Decimal,
        high: Decimal,
        low: Decimal,
        open: Decimal,
        prev_close: Decimal,
        volume: Decimal,
        trading_value: Decimal,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            current_price,
            price_change,
            change_percent,
            high,
            low,
            open,
            prev_close,
            volume,
            trading_value,
            timestamp: Utc::now(),
        }
    }
}

/// 시세 데이터 제공자 trait.
///
/// 거래소 또는 데이터 소스로부터 실시간 시세를 조회합니다.
/// 현재 선택된 거래소에 연결되어 해당 거래소의 시세 데이터를 제공합니다.
///
/// # 구현 예시
///
/// ```ignore
/// pub struct KisExchangeProvider {
///     client: Arc<KisClient>,
/// }
///
/// #[async_trait]
/// impl MarketDataProvider for KisExchangeProvider {
///     async fn get_quote(&self, symbol: &str) -> Result<QuoteData, ProviderError> {
///         let price = self.client.kr().get_price(symbol).await?;
///         Ok(price.into())
///     }
///
///     fn provider_name(&self) -> &str { "한국투자증권" }
/// }
/// ```
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    /// 현재가 조회.
    ///
    /// 지정된 심볼의 현재 시세 데이터를 조회합니다.
    ///
    /// # Arguments
    ///
    /// * `symbol` - 조회할 종목 심볼 (예: "005930", "AAPL")
    ///
    /// # Errors
    ///
    /// - `ProviderError::Network`: 네트워크 연결 실패
    /// - `ProviderError::Api`: 거래소 API 에러
    /// - `ProviderError::Parse`: 응답 파싱 실패
    async fn get_quote(&self, symbol: &str) -> Result<QuoteData, ProviderError>;

    /// 데이터 제공자 이름 반환.
    ///
    /// 로깅 및 디버깅 목적으로 사용됩니다.
    fn provider_name(&self) -> &str;

    /// 여러 종목의 현재가 일괄 조회.
    ///
    /// 기본 구현은 순차적으로 개별 조회합니다.
    /// 거래소가 일괄 조회 API를 제공하면 오버라이드하여 최적화할 수 있습니다.
    ///
    /// # Arguments
    ///
    /// * `symbols` - 조회할 종목 심볼 목록
    ///
    /// # Returns
    ///
    /// 성공한 조회 결과 목록. 개별 종목 조회 실패는 해당 종목만 제외됩니다.
    async fn get_quotes(&self, symbols: &[String]) -> Vec<QuoteData> {
        let mut results = Vec::new();
        for symbol in symbols {
            if let Ok(quote) = self.get_quote(symbol).await {
                results.push(quote);
            }
        }
        results
    }
}

// =============================================================================
// 테스트
// =============================================================================


// =============================================================================
// OrderExecutionProvider Trait
// =============================================================================

use super::OrderRequest;

/// 주문 실행 제공자 trait.
///
/// 거래소별 주문 실행을 추상화합니다.
/// `ExchangeProvider`(조회 전용)와 분리하여 주문 실행만 담당합니다.
///
/// # 설계 원칙
///
/// - **관심사 분리**: 시세 조회(`ExchangeProvider`)와 주문 실행을 분리
/// - **거래소 중립성**: KIS, Binance 등 다양한 거래소를 동일한 인터페이스로 사용
/// - **테스트 용이성**: `MockOrderProvider`로 단위 테스트 가능
///
/// # 구현 예시
///
/// ```ignore
/// #[async_trait]
/// impl OrderExecutionProvider for KisExchangeProvider {
///     async fn place_order(&self, request: &OrderRequest) -> Result<OrderResponse, ProviderError> {
///         // KIS API를 통한 주문 제출
///     }
///     // ...
/// }
/// ```
#[async_trait]
pub trait OrderExecutionProvider: Send + Sync {
    /// 주문 제출.
    ///
    /// `OrderRequest`를 거래소에 전달하여 주문을 생성합니다.
    /// 시장가/지정가/스톱 주문을 지원합니다.
    ///
    /// # Arguments
    ///
    /// * `request` - 주문 요청 (종목, 방향, 수량, 가격 등)
    ///
    /// # Returns
    ///
    /// 거래소로부터 받은 주문 응답 (주문번호, 주문시간).
    ///
    /// # Errors
    ///
    /// - `ProviderError::Api`: 거래소 API 에러 (자금 부족, 수량 초과 등)
    /// - `ProviderError::Network`: 네트워크 연결 실패
    /// - `ProviderError::Authentication`: 인증 실패
    async fn place_order(
        &self,
        request: &OrderRequest,
    ) -> Result<super::OrderResponse, ProviderError>;

    /// 주문 취소.
    ///
    /// 미체결 주문을 취소합니다.
    ///
    /// # Arguments
    ///
    /// * `order_id` - 취소할 주문번호
    /// * `ticker` - 종목 심볼 (거래소별 필요 여부 다름)
    ///
    /// # Errors
    ///
    /// - `ProviderError::Api`: 이미 체결되었거나 존재하지 않는 주문
    async fn cancel_order(&self, order_id: &str, ticker: &str) -> Result<(), ProviderError>;

    /// 주문 정정.
    ///
    /// 미체결 주문의 수량이나 가격을 변경합니다.
    ///
    /// # Arguments
    ///
    /// * `order_id` - 정정할 주문번호
    /// * `ticker` - 종목 심볼
    /// * `quantity` - 새 수량 (None이면 변경 없음)
    /// * `price` - 새 가격 (None이면 변경 없음)
    ///
    /// # Errors
    ///
    /// - `ProviderError::Api`: 주문 정정 실패
    /// - `ProviderError::Unsupported`: 정정 미지원 거래소
    async fn modify_order(
        &self,
        order_id: &str,
        ticker: &str,
        quantity: Option<Decimal>,
        price: Option<Decimal>,
    ) -> Result<super::OrderResponse, ProviderError>;

    /// 거래소 이름.
    fn exchange_name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::order::Side;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    /// 테스트용 MockProvider.
    struct MockProvider {
        name: String,
        should_fail: bool,
    }

    #[async_trait]
    impl ExchangeProvider for MockProvider {
        async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
            if self.should_fail {
                return Err(ProviderError::Network("Mock network error".to_string()));
            }
            Ok(StrategyAccountInfo {
                total_balance: dec!(10000),
                available_balance: dec!(5000),
                margin_used: Decimal::ZERO,
                unrealized_pnl: dec!(100),
                currency: "USD".to_string(),
            })
        }

        async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
            if self.should_fail {
                return Err(ProviderError::Api("Mock API error".to_string()));
            }
            let ticker = "BTC/USDT".to_string();
            let pos = StrategyPositionInfo::new(ticker, Side::Buy, dec!(0.5), dec!(50000));
            Ok(vec![pos])
        }

        async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError> {
            if self.should_fail {
                return Err(ProviderError::Authentication("Mock auth error".to_string()));
            }
            Ok(vec![])
        }

        fn exchange_name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_mock_provider_success() {
        let provider = MockProvider {
            name: "MockExchange".to_string(),
            should_fail: false,
        };

        // exchange_name 테스트
        assert_eq!(provider.exchange_name(), "MockExchange");

        // fetch_account 테스트
        let account = provider.fetch_account().await.unwrap();
        assert_eq!(account.total_balance, dec!(10000));
        assert_eq!(account.currency, "USD");

        // fetch_positions 테스트
        let positions = provider.fetch_positions().await.unwrap();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].ticker, "BTC/USDT");

        // fetch_pending_orders 테스트
        let orders = provider.fetch_pending_orders().await.unwrap();
        assert_eq!(orders.len(), 0);
    }

    #[tokio::test]
    async fn test_mock_provider_errors() {
        let provider = MockProvider {
            name: "MockExchange".to_string(),
            should_fail: true,
        };

        // Network error
        let result = provider.fetch_account().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::Network(_)));

        // API error
        let result = provider.fetch_positions().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::Api(_)));

        // Authentication error
        let result = provider.fetch_pending_orders().await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProviderError::Authentication(_)
        ));
    }
}
