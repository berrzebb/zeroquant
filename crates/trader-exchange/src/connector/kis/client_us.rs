//! KIS 해외 주식 REST API 클라이언트.
//!
//! 이 모듈은 한국투자증권 API를 통해 미국 주식/ETF 거래를 위한
//! REST API 클라이언트를 제공합니다.
//!
//! # 지원 기능
//!
//! - 현재가 조회
//! - 기간별 시세 조회
//! - 매수/매도 주문
//! - 주문 정정/취소
//! - 잔고 조회
//! - 주/야간 구분 확인
//!
//! # 거래소 코드 (EXCD)
//!
//! - `NASD`: NASDAQ
//! - `NYSE`: 뉴욕증권거래소
//! - `AMEX`: 미국증권거래소

use super::auth::KisOAuth;
use super::config::KisEnvironment;
use super::exchange_code;
use super::tr_id;
use crate::ExchangeError;
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use trader_core::{RoundMethod, TickSizeProvider};

/// 미국 시장 세션 유형.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsMarketSession {
    /// 주간 세션 (23:30-06:00 KST, 서머타임: 22:30-05:00 KST)
    Day,
    /// 야간/연장 세션
    Night,
    /// 장 마감
    Closed,
}

/// KIS 해외 주식 REST API 클라이언트.
///
/// `KisOAuth`를 `Arc`로 공유하여 동일한 `app_key`를 사용하는 여러 클라이언트가
/// 토큰을 공유할 수 있습니다. KIS API는 토큰 발급을 1분에 1회로 제한하므로
/// 토큰 공유가 필수적입니다.
pub struct KisUsClient {
    oauth: Arc<KisOAuth>,
    client: Client,
    /// 호가 단위 제공자 (옵션, 설정 시 주문 가격 자동 라운딩)
    tick_size_provider: Option<Arc<dyn TickSizeProvider>>,
}

impl KisUsClient {
    /// 새로운 해외 주식 클라이언트 생성 (소유권 이전).
    ///
    /// 단일 클라이언트만 사용하는 경우 이 메서드를 사용합니다.
    ///
    /// # Errors
    /// HTTP 클라이언트 생성에 실패하면 `ExchangeError::NetworkError`를 반환합니다.
    pub fn new(oauth: KisOAuth) -> Result<Self, ExchangeError> {
        Self::with_shared_oauth(Arc::new(oauth))
    }

    /// 공유된 OAuth로 해외 주식 클라이언트 생성.
    ///
    /// 동일한 `app_key`를 사용하는 여러 클라이언트(국내/해외, 실계좌/모의투자)가
    /// 토큰을 공유하려면 이 메서드를 사용합니다.
    ///
    /// # Errors
    /// HTTP 클라이언트 생성에 실패하면 `ExchangeError::NetworkError`를 반환합니다.
    pub fn with_shared_oauth(oauth: Arc<KisOAuth>) -> Result<Self, ExchangeError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(oauth.config().timeout_secs))
            .build()
            .map_err(|e| ExchangeError::NetworkError(format!("HTTP client 생성 실패: {}", e)))?;

        Ok(Self {
            oauth,
            client,
            tick_size_provider: None,
        })
    }

    /// 호가 단위 제공자를 설정합니다.
    ///
    /// 설정 시 주문 가격이 자동으로 호가 단위로 라운딩됩니다.
    /// - 매수 주문: Floor (내림) - 보수적으로 더 낮은 가격
    /// - 매도 주문: Ceil (올림) - 보수적으로 더 높은 가격
    ///
    /// # Arguments
    /// * `provider` - 호가 단위 제공자 (예: `UsEquityTickSize`)
    pub fn with_tick_size_provider(mut self, provider: Arc<dyn TickSizeProvider>) -> Self {
        self.tick_size_provider = Some(provider);
        self
    }

    /// 내부 OAuth 참조 반환 (토큰 캐싱용).
    pub fn oauth(&self) -> &Arc<KisOAuth> {
        &self.oauth
    }

    /// 환경에 따른 적절한 tr_id 반환.
    fn get_tr_id<'a>(&self, real_id: &'a str, paper_id: &'a str) -> &'a str {
        match self.oauth.config().environment {
            KisEnvironment::Real => real_id,
            KisEnvironment::Paper => paper_id,
        }
    }

    /// 심볼을 거래소 코드로 변환.
    ///
    /// 일반적인 매핑:
    /// - 대부분의 ETF와 기술주: NASD (NASDAQ)
    /// - KO, JNJ 등 대형주: NYSE
    /// - 일부 소형주: AMEX
    pub fn get_exchange_code(symbol: &str) -> &'static str {
        // 일반적인 NASDAQ 심볼
        let nasdaq_symbols = [
            "AAPL", "MSFT", "GOOGL", "GOOG", "AMZN", "META", "NVDA", "TSLA", "QQQ", "TQQQ", "SQQQ",
            "SPY", "VOO", "IVV", "VTI", "SCHD", "TLT", "IEF", "SHY", "BIL", "VEA", "VWO", "EFA",
            "EEM",
        ];

        // 일반적인 NYSE 심볼
        let nyse_symbols = [
            "KO", "JNJ", "PG", "JPM", "V", "MA", "WMT", "DIS", "HD", "BAC", "XOM", "CVX", "PFE",
            "MRK", "ABT", "UNH",
        ];

        let upper = symbol.to_uppercase();

        if nasdaq_symbols.iter().any(|s| *s == upper) {
            exchange_code::NASDAQ
        } else if nyse_symbols.iter().any(|s| *s == upper) {
            exchange_code::NYSE
        } else {
            // ETF와 기술주는 기본적으로 NASDAQ
            exchange_code::NASDAQ
        }
    }

    // ========================================
    // Market Data APIs (시세 조회)
    // ========================================

    /// 해외주식 현재가 상세 조회.
    ///
    /// # 인자
    /// * `symbol` - 종목 심볼 (예: "AAPL", "SPY")
    /// * `exchange_code` - 거래소 코드 (NASD, NYSE, AMEX). None이면 자동 감지.
    pub async fn get_price(
        &self,
        symbol: &str,
        exchange_code: Option<&str>,
    ) -> Result<StockPrice, ExchangeError> {
        let tr_id = self.get_tr_id(tr_id::US_PRICE_DETAIL_REAL, tr_id::US_PRICE_DETAIL_PAPER);
        let url = format!(
            "{}/uapi/overseas-price/v1/quotations/price-detail",
            self.oauth.config().rest_base_url()
        );

        let excd = exchange_code.unwrap_or_else(|| Self::get_exchange_code(symbol));
        let headers = self.oauth.build_headers(tr_id, None).await?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[("AUTH", ""), ("EXCD", excd), ("SYMB", symbol)])
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("US price inquiry failed: {} - {}", status, body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        debug!("US price response: {}", body);

        let resp: KisUsPriceResponse = serde_json::from_str(&body).map_err(|e| {
            ExchangeError::ParseError(format!("Failed to parse price response: {}", e))
        })?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        Ok(resp.output)
    }

    /// 해외주식 기간별 시세 조회.
    ///
    /// # 인자
    /// * `symbol` - 종목 심볼
    /// * `period` - 기간 유형: "D" (일별), "W" (주별), "M" (월별)
    /// * `start_date` - 시작일 (YYYYMMDD)
    /// * `end_date` - 종료일 (YYYYMMDD)
    /// * `exchange_code` - 거래소 코드. None이면 자동 감지.
    pub async fn get_daily_price(
        &self,
        symbol: &str,
        period: &str,
        _start_date: &str,
        end_date: &str,
        exchange_code: Option<&str>,
    ) -> Result<Vec<UsOhlcv>, ExchangeError> {
        let tr_id = self.get_tr_id(tr_id::US_DAILY_PRICE_REAL, tr_id::US_DAILY_PRICE_PAPER);
        let url = format!(
            "{}/uapi/overseas-price/v1/quotations/dailyprice",
            self.oauth.config().rest_base_url()
        );

        let excd = exchange_code.unwrap_or_else(|| Self::get_exchange_code(symbol));
        let headers = self.oauth.build_headers(tr_id, None).await?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[
                ("AUTH", ""),
                ("EXCD", excd),
                ("SYMB", symbol),
                ("GUBN", period), // D: daily, W: weekly, M: monthly
                ("BYMD", end_date),
                ("MODP", "1"), // 수정주가 반영
            ])
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("US daily price inquiry failed: {} - {}", status, body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        debug!("US daily price response: {}", body);

        let resp: KisUsDailyPriceResponse = serde_json::from_str(&body).map_err(|e| {
            ExchangeError::ParseError(format!("Failed to parse daily price response: {}", e))
        })?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        Ok(resp.output2)
    }

    // ========================================
    // Trading APIs (주문)
    // ========================================

    /// 해외주식 매수 주문.
    ///
    /// # 인자
    /// * `symbol` - 종목 심볼
    /// * `quantity` - 주문 수량
    /// * `price` - 주문 가격 (시장가 주문의 경우 0)
    /// * `order_type` - "00" 지정가, "31" 시장가 (거래소별 상이)
    /// * `exchange_code` - 거래소 코드. None이면 자동 감지.
    pub async fn place_buy_order(
        &self,
        symbol: &str,
        quantity: u32,
        price: Decimal,
        order_type: &str,
        exchange_code: Option<&str>,
    ) -> Result<UsOrderResponse, ExchangeError> {
        self.place_order(symbol, quantity, price, order_type, exchange_code, true)
            .await
    }

    /// 해외주식 매도 주문.
    pub async fn place_sell_order(
        &self,
        symbol: &str,
        quantity: u32,
        price: Decimal,
        order_type: &str,
        exchange_code: Option<&str>,
    ) -> Result<UsOrderResponse, ExchangeError> {
        self.place_order(symbol, quantity, price, order_type, exchange_code, false)
            .await
    }

    /// 내부 주문 실행.
    async fn place_order(
        &self,
        symbol: &str,
        quantity: u32,
        price: Decimal,
        order_type: &str,
        exchange_code: Option<&str>,
        is_buy: bool,
    ) -> Result<UsOrderResponse, ExchangeError> {
        // 호가 단위 라운딩 (시장가 주문이 아닌 경우)
        let rounded_price = if !price.is_zero() {
            if let Some(ref provider) = self.tick_size_provider {
                // 매수: Floor (내림) - 보수적으로 더 낮은 가격
                // 매도: Ceil (올림) - 보수적으로 더 높은 가격
                let method = if is_buy {
                    RoundMethod::Floor
                } else {
                    RoundMethod::Ceil
                };
                let adjusted = provider.round_to_tick(price, method);
                if adjusted != price {
                    warn!(
                        "주문 가격 호가 단위 조정: {} -> {} (종목: {}, 방향: {})",
                        price,
                        adjusted,
                        symbol,
                        if is_buy { "매수" } else { "매도" }
                    );
                }
                adjusted
            } else {
                price
            }
        } else {
            price
        };

        let tr_id = if is_buy {
            self.get_tr_id(tr_id::US_BUY_REAL, tr_id::US_BUY_PAPER)
        } else {
            self.get_tr_id(tr_id::US_SELL_REAL, tr_id::US_SELL_PAPER)
        };

        let url = format!(
            "{}/uapi/overseas-stock/v1/trading/order",
            self.oauth.config().rest_base_url()
        );

        let excd = exchange_code.unwrap_or_else(|| Self::get_exchange_code(symbol));

        // 매수/매도에 따른 사이드 코드 결정 (API 호출용 예비)
        let _side_cd = if is_buy { "buy" } else { "sell" };

        // 요청 본문 구성 (라운딩된 가격 사용)
        let body = serde_json::json!({
            "CANO": self.oauth.config().cano(),
            "ACNT_PRDT_CD": self.oauth.config().acnt_prdt_cd(),
            "OVRS_EXCG_CD": excd,
            "PDNO": symbol,
            "ORD_QTY": quantity.to_string(),
            "OVRS_ORD_UNPR": rounded_price.to_string(),
            "ORD_SVR_DVSN_CD": "0",
            "ORD_DVSN": order_type,
        });

        let hashkey = self.oauth.generate_hashkey(&body).await?;
        let headers = self.oauth.build_headers(tr_id, Some(&hashkey)).await?;

        info!(
            "Placing US {} order: {} x {} @ {} ({}, type: {})",
            if is_buy { "BUY" } else { "SELL" },
            symbol,
            quantity,
            price,
            excd,
            order_type
        );

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let response_body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("US order failed: {} - {}", status, response_body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: response_body,
            });
        }

        debug!("US order response: {}", response_body);

        let resp: KisUsOrderApiResponse = serde_json::from_str(&response_body).map_err(|e| {
            ExchangeError::ParseError(format!("Failed to parse order response: {}", e))
        })?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        info!(
            "US order placed successfully: order_no={}, symbol={}",
            resp.output.odno, symbol
        );

        Ok(resp.output)
    }

    /// 해외주식 주문 취소.
    pub async fn cancel_order(
        &self,
        order_no: &str,
        symbol: &str,
        quantity: u32,
        exchange_code: Option<&str>,
    ) -> Result<UsOrderResponse, ExchangeError> {
        self.modify_or_cancel_order(
            order_no,
            symbol,
            quantity,
            Decimal::ZERO,
            exchange_code,
            true,
        )
        .await
    }

    /// 해외주식 주문 정정.
    pub async fn modify_order(
        &self,
        order_no: &str,
        symbol: &str,
        quantity: u32,
        price: Decimal,
        exchange_code: Option<&str>,
    ) -> Result<UsOrderResponse, ExchangeError> {
        self.modify_or_cancel_order(order_no, symbol, quantity, price, exchange_code, false)
            .await
    }

    /// 내부 정정/취소 주문.
    async fn modify_or_cancel_order(
        &self,
        order_no: &str,
        symbol: &str,
        quantity: u32,
        price: Decimal,
        exchange_code: Option<&str>,
        is_cancel: bool,
    ) -> Result<UsOrderResponse, ExchangeError> {
        let tr_id = if is_cancel {
            self.get_tr_id(tr_id::US_CANCEL_REAL, tr_id::US_CANCEL_PAPER)
        } else {
            self.get_tr_id(tr_id::US_MODIFY_REAL, tr_id::US_MODIFY_PAPER)
        };

        let url = format!(
            "{}/uapi/overseas-stock/v1/trading/order-rvsecncl",
            self.oauth.config().rest_base_url()
        );

        let excd = exchange_code.unwrap_or_else(|| Self::get_exchange_code(symbol));

        let body = serde_json::json!({
            "CANO": self.oauth.config().cano(),
            "ACNT_PRDT_CD": self.oauth.config().acnt_prdt_cd(),
            "OVRS_EXCG_CD": excd,
            "PDNO": symbol,
            "ORGN_ODNO": order_no,
            "RVSE_CNCL_DVSN_CD": if is_cancel { "02" } else { "01" },
            "ORD_QTY": quantity.to_string(),
            "OVRS_ORD_UNPR": price.to_string(),
        });

        let hashkey = self.oauth.generate_hashkey(&body).await?;
        let headers = self.oauth.build_headers(tr_id, Some(&hashkey)).await?;

        info!(
            "US order {}: order_no={}, symbol={}",
            if is_cancel { "CANCEL" } else { "MODIFY" },
            order_no,
            symbol
        );

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let response_body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!(
                "US order modify/cancel failed: {} - {}",
                status, response_body
            );
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: response_body,
            });
        }

        let resp: KisUsOrderApiResponse = serde_json::from_str(&response_body)
            .map_err(|e| ExchangeError::ParseError(format!("Failed to parse response: {}", e)))?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        Ok(resp.output)
    }

    // ========================================
    // Account APIs (계좌)
    // ========================================

    /// 해외주식 잔고 조회.
    ///
    /// # 인자
    /// * `currency` - 통화 코드: "USD", "HKD", "CNY" 등
    pub async fn get_balance(&self, currency: &str) -> Result<trader_core::AccountBalance, ExchangeError> {
        let tr_id = self.get_tr_id(tr_id::US_BALANCE_REAL, tr_id::US_BALANCE_PAPER);
        let url = format!(
            "{}/uapi/overseas-stock/v1/trading/inquire-balance",
            self.oauth.config().rest_base_url()
        );

        let headers = self.oauth.build_headers(tr_id, None).await?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[
                ("CANO", self.oauth.config().cano()),
                ("ACNT_PRDT_CD", self.oauth.config().acnt_prdt_cd()),
                ("OVRS_EXCG_CD", "NASD"), // Default to NASDAQ
                ("TR_CRCY_CD", currency),
                ("CTX_AREA_FK200", ""),
                ("CTX_AREA_NK200", ""),
            ])
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("US balance inquiry failed: {} - {}", status, body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        debug!("US balance response: {}", body);

        let resp: KisUsBalanceResponse = serde_json::from_str(&body).map_err(|e| {
            ExchangeError::ParseError(format!("Failed to parse balance response: {}", e))
        })?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        // AccountBalance로 직접 변환
        let mut balance = trader_core::AccountBalance::new("KIS", currency);

        // 보유 종목 변환
        for h in resp.output1 {
            let mut holding = trader_core::Holding::new(
                &h.symbol,
                &h.name,
                h.quantity,
                h.avg_price,
            );
            holding.update_price(h.current_price);
            balance.add_holding(holding);
        }

        // 요약 정보 적용
        if let Some(summary) = resp.output2 {
            balance.total_eval_amount = summary.total_eval_amount;
            balance.total_profit_loss = summary.total_profit_loss;
        }

        Ok(balance)
    }

    /// 해외 주식 미체결 주문 조회.
    ///
    /// 당일 미체결 주문만 조회합니다.
    ///
    /// # 참고
    /// - TR ID: TTTT3039R (실전), VTTT3039R (모의)
    /// - 엔드포인트: /uapi/overseas-stock/v1/trading/inquire-nccs
    pub async fn get_pending_orders(&self) -> Result<Vec<UsOrderExecution>, ExchangeError> {
        // 당일 날짜 생성 (KST 기준)
        let now = chrono::Utc::now() + chrono::Duration::hours(9);
        let today = now.format("%Y%m%d").to_string();

        let tr_id = self.get_tr_id(
            tr_id::US_PENDING_ORDERS_REAL,
            tr_id::US_PENDING_ORDERS_PAPER,
        );

        let url = format!(
            "{}/uapi/overseas-stock/v1/trading/inquire-nccs",
            self.oauth.config().rest_base_url()
        );

        let headers = self.oauth.build_headers(tr_id, None).await?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[
                ("CANO", self.oauth.config().cano()),
                ("ACNT_PRDT_CD", self.oauth.config().acnt_prdt_cd()),
                ("INQR_STRT_DT", today.as_str()), // 조회 시작일
                ("INQR_END_DT", today.as_str()),  // 조회 종료일
                ("INQR_DVSN_CD", "00"),           // 조회구분: 00=전체
                ("OVRS_EXCG_CD", ""),             // 거래소코드 (공백=전체)
                ("PDNO", ""),                     // 종목코드 (공백=전체)
                ("CCLD_DVSN", "02"),              // 체결구분: 02=미체결
                ("SORT_SQN", "DS"),               // 정렬순서: DS=내림차순
                ("ORD_DT", ""),                   // 주문일자 (공백=당일)
                ("CTX_AREA_FK200", ""),           // 연속조회키
                ("CTX_AREA_NK200", ""),           // 연속조회키
            ])
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("US pending orders inquiry failed: {} - {}", status, body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        debug!("US pending orders response: {}", body);

        let resp: KisUsOrderHistoryResponse = serde_json::from_str(&body).map_err(|e| {
            ExchangeError::ParseError(format!("Failed to parse pending orders response: {}", e))
        })?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        Ok(resp.output)
    }

    /// 해외주식 주야간원장 구분 조회.
    ///
    /// 현재 시장 세션 유형을 반환합니다.
    pub async fn get_market_session(&self) -> Result<UsMarketSession, ExchangeError> {
        let tr_id = self.get_tr_id(tr_id::US_DAY_NIGHT_REAL, tr_id::US_DAY_NIGHT_PAPER);
        let url = format!(
            "{}/uapi/overseas-stock/v1/trading/dayornight",
            self.oauth.config().rest_base_url()
        );

        let headers = self.oauth.build_headers(tr_id, None).await?;

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[
                ("CANO", self.oauth.config().cano()),
                ("ACNT_PRDT_CD", self.oauth.config().acnt_prdt_cd()),
            ])
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("US day/night check failed: {} - {}", status, body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        debug!("US day/night response: {}", body);

        let resp: KisUsDayNightResponse = serde_json::from_str(&body).map_err(|e| {
            ExchangeError::ParseError(format!("Failed to parse day/night response: {}", e))
        })?;

        if resp.rt_cd != "0" {
            return Err(ExchangeError::ApiError {
                code: resp.msg_cd.parse().unwrap_or(-1),
                message: resp.msg1,
            });
        }

        // psbl_yn: "Y" = tradeable (day session), "N" = not tradeable
        let session = match resp.output.psbl_yn.as_str() {
            "Y" => UsMarketSession::Day,
            "N" => UsMarketSession::Night,
            _ => UsMarketSession::Closed,
        };

        Ok(session)
    }

    /// 주간 세션 여부 확인.
    pub async fn is_day_session(&self) -> Result<bool, ExchangeError> {
        let session = self.get_market_session().await?;
        Ok(session == UsMarketSession::Day)
    }
}

// ========================================
// 응답 타입
// ========================================

/// 미국 주식 시세 데이터.
#[derive(Debug, Clone, Deserialize)]
pub struct StockPrice {
    /// 현재가
    #[serde(rename = "last", deserialize_with = "deserialize_decimal")]
    pub current_price: Decimal,
    /// 전일 종가
    #[serde(rename = "base", deserialize_with = "deserialize_decimal")]
    pub prev_close: Decimal,
    /// 전일대비
    #[serde(rename = "diff", deserialize_with = "deserialize_decimal")]
    pub price_change: Decimal,
    /// 등락률 (%)
    #[serde(rename = "rate", deserialize_with = "deserialize_decimal")]
    pub change_rate: Decimal,
    /// 당일 고가
    #[serde(rename = "high", deserialize_with = "deserialize_decimal")]
    pub high: Decimal,
    /// 당일 저가
    #[serde(rename = "low", deserialize_with = "deserialize_decimal")]
    pub low: Decimal,
    /// 당일 시가
    #[serde(rename = "open", deserialize_with = "deserialize_decimal")]
    pub open: Decimal,
    /// 거래량
    #[serde(rename = "tvol", deserialize_with = "deserialize_decimal")]
    pub volume: Decimal,
    /// 거래대금
    #[serde(rename = "tamt", deserialize_with = "deserialize_decimal")]
    pub trading_value: Decimal,
    /// 52주 최고가
    #[serde(rename = "h52p", deserialize_with = "deserialize_decimal")]
    pub high_52w: Decimal,
    /// 52주 최저가
    #[serde(rename = "l52p", deserialize_with = "deserialize_decimal")]
    pub low_52w: Decimal,
    /// PER (표시용, 예: "1")
    #[serde(rename = "perx", default)]
    pub per: String,
    /// PBR
    #[serde(rename = "pbrx", default)]
    pub pbr: String,
    /// EPS
    #[serde(rename = "epsx", default)]
    pub eps: String,
    /// BPS
    #[serde(rename = "bpsx", default)]
    pub bps: String,
}

/// 미국 OHLCV 데이터 (일별/주별/월별).
#[derive(Debug, Clone, Deserialize)]
pub struct UsOhlcv {
    /// 날짜 (YYYYMMDD)
    #[serde(rename = "xymd")]
    pub date: String,
    /// 종가
    #[serde(rename = "clos", deserialize_with = "deserialize_decimal")]
    pub close: Decimal,
    /// 시가
    #[serde(rename = "open", deserialize_with = "deserialize_decimal")]
    pub open: Decimal,
    /// 고가
    #[serde(rename = "high", deserialize_with = "deserialize_decimal")]
    pub high: Decimal,
    /// 저가
    #[serde(rename = "low", deserialize_with = "deserialize_decimal")]
    pub low: Decimal,
    /// 거래량
    #[serde(rename = "tvol", deserialize_with = "deserialize_decimal")]
    pub volume: Decimal,
}

/// 미국 주문 응답.
#[derive(Debug, Clone, Deserialize)]
pub struct UsOrderResponse {
    /// 주문번호
    #[serde(rename = "ODNO")]
    pub odno: String,
    /// 주문시간
    #[serde(rename = "ORD_TMD", default)]
    pub order_time: String,
}

/// API 응답 파싱용 내부 타입.
#[derive(Debug, Deserialize)]
struct UsHolding {
    #[serde(rename = "ovrs_pdno")]
    symbol: String,
    #[serde(rename = "ovrs_item_name")]
    name: String,
    #[serde(rename = "ovrs_cblc_qty", deserialize_with = "deserialize_decimal")]
    quantity: Decimal,
    #[serde(rename = "pchs_avg_pric", deserialize_with = "deserialize_decimal")]
    avg_price: Decimal,
    #[serde(rename = "now_pric2", deserialize_with = "deserialize_decimal")]
    current_price: Decimal,
}

/// API 응답 파싱용 내부 타입.
#[derive(Debug, Deserialize)]
struct UsAccountSummary {
    #[serde(
        rename = "tot_evlu_pfls_amt",
        deserialize_with = "deserialize_decimal_opt"
    )]
    total_eval_amount: Option<Decimal>,
    #[serde(rename = "ovrs_tot_pfls", deserialize_with = "deserialize_decimal_opt")]
    total_profit_loss: Option<Decimal>,
}

/// 주/야간 세션 정보.
#[derive(Debug, Clone, Deserialize)]
pub struct UsDayNightInfo {
    /// 거래가능여부 ("Y" 또는 "N")
    #[serde(rename = "PSBL_YN")]
    pub psbl_yn: String,
}

/// 해외 주식 체결 내역 (미체결 주문 조회).
///
/// # 참고
/// - 필드명은 KIS API 패턴을 따름 (`ft_` 접두사는 foreign trade)
/// - 가격 필드는 `*_unpr3` 패턴 (소수점 3자리)
/// - TR ID: TTTT3039R (실전), VTTT3039R (모의)
#[derive(Debug, Clone, Deserialize)]
pub struct UsOrderExecution {
    /// 주문일자 (YYYYMMDD)
    #[serde(rename = "ord_dt")]
    pub order_date: String,

    /// 주문번호
    #[serde(rename = "odno")]
    pub order_no: String,

    /// 원주문번호
    #[serde(rename = "orgn_odno", default)]
    pub original_order_no: String,

    /// 주문시각 (HHMMSS)
    #[serde(rename = "ord_tmd")]
    pub order_time: String,

    /// 매수/매도 구분 (01=매도, 02=매수)
    #[serde(rename = "sll_buy_dvsn_cd")]
    pub side_code: String,

    /// 종목코드
    #[serde(rename = "pdno")]
    pub symbol: String,

    /// 종목명
    #[serde(rename = "prdt_name", default)]
    pub name: String,

    /// 거래소 코드 (NASD, NYSE, AMEX 등)
    #[serde(rename = "ovrs_excg_cd")]
    pub exchange_code: String,

    /// 주문수량
    #[serde(rename = "ft_ord_qty", deserialize_with = "deserialize_decimal")]
    pub order_qty: Decimal,

    /// 주문단가
    #[serde(rename = "ft_ord_unpr3", deserialize_with = "deserialize_decimal")]
    pub order_price: Decimal,

    /// 체결수량
    #[serde(rename = "ft_ccld_qty", deserialize_with = "deserialize_decimal")]
    pub filled_qty: Decimal,

    /// 체결평균가
    #[serde(rename = "ft_ccld_unpr3", deserialize_with = "deserialize_decimal")]
    pub avg_price: Decimal,

    /// 체결금액
    #[serde(rename = "ft_ccld_amt3", deserialize_with = "deserialize_decimal")]
    pub filled_amount: Decimal,

    /// 주문구분명
    #[serde(rename = "ord_dvsn_name", default)]
    pub order_type_name: String,

    /// 취소여부 ("Y" 또는 "N")
    #[serde(rename = "cncl_yn", default)]
    pub cancel_yn: String,

    /// 정정취소구분명
    #[serde(rename = "rvse_cncl_dvsn_name", default)]
    pub modify_cancel_name: String,
}

// ========================================
// API 응답 래퍼
// ========================================

#[derive(Debug, Deserialize)]
struct KisUsPriceResponse {
    rt_cd: String,
    msg_cd: String,
    msg1: String,
    output: StockPrice,
}

#[derive(Debug, Deserialize)]
struct KisUsDailyPriceResponse {
    rt_cd: String,
    msg_cd: String,
    msg1: String,
    output2: Vec<UsOhlcv>,
}

#[derive(Debug, Deserialize)]
struct KisUsOrderApiResponse {
    rt_cd: String,
    msg_cd: String,
    msg1: String,
    output: UsOrderResponse,
}

#[derive(Debug, Deserialize)]
struct KisUsBalanceResponse {
    rt_cd: String,
    msg_cd: String,
    msg1: String,
    output1: Vec<UsHolding>,
    output2: Option<UsAccountSummary>,
}

#[derive(Debug, Deserialize)]
struct KisUsDayNightResponse {
    rt_cd: String,
    msg_cd: String,
    msg1: String,
    output: UsDayNightInfo,
}

/// 해외 주식 미체결 주문 조회 응답.
#[derive(Debug, Deserialize)]
struct KisUsOrderHistoryResponse {
    rt_cd: String,
    msg_cd: String,
    msg1: String,
    /// 주문 내역 목록
    output: Vec<UsOrderExecution>,
}

// ========================================
// 유틸리티 함수
// ========================================

/// 문자열을 Decimal로 역직렬화.
fn deserialize_decimal<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    if s.is_empty() || s == "-" {
        return Ok(Decimal::ZERO);
    }
    s.parse::<Decimal>()
        .map_err(|_| serde::de::Error::custom(format!("Invalid decimal: {}", s)))
}

/// 문자열을 Option<Decimal>로 역직렬화.
fn deserialize_decimal_opt<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(s) if !s.is_empty() && s != "-" => s
            .parse::<Decimal>()
            .map(Some)
            .map_err(|_| serde::de::Error::custom(format!("Invalid decimal: {}", s))),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exchange_code_detection() {
        assert_eq!(KisUsClient::get_exchange_code("AAPL"), "NAS");
        assert_eq!(KisUsClient::get_exchange_code("SPY"), "NAS");
        assert_eq!(KisUsClient::get_exchange_code("TQQQ"), "NAS");
        assert_eq!(KisUsClient::get_exchange_code("KO"), "NYS");
        assert_eq!(KisUsClient::get_exchange_code("JNJ"), "NYS");
        // Unknown symbols default to NASDAQ
        assert_eq!(KisUsClient::get_exchange_code("UNKNOWN"), "NAS");
    }

    #[test]
    fn test_deserialize_decimal() {
        let json = r#"{"value": "123.45"}"#;
        #[derive(Deserialize)]
        struct Test {
            #[serde(deserialize_with = "deserialize_decimal")]
            value: Decimal,
        }
        let result: Test = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, Decimal::new(12345, 2));
    }

    #[test]
    fn test_deserialize_empty_decimal() {
        let json = r#"{"value": ""}"#;
        #[derive(Deserialize)]
        struct Test {
            #[serde(deserialize_with = "deserialize_decimal")]
            value: Decimal,
        }
        let result: Test = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, Decimal::ZERO);
    }

    #[test]
    fn test_market_session_enum() {
        assert_eq!(UsMarketSession::Day, UsMarketSession::Day);
        assert_ne!(UsMarketSession::Day, UsMarketSession::Night);
    }
}
