//! 한국투자증권 통합 Provider.
//!
//! KIS 국내/해외 주식을 하나의 Provider로 통합합니다.
//! - ExchangeProvider: 계좌, 포지션, 주문 관리
//! - MarketDataProvider: 실시간 시세 조회
//!
//! # 아키텍처
//!
//! ```text
//! KisExchangeProvider
//! ├── ExchangeProvider 구현
//! │   ├── fetch_account() - 국내/해외 통합 계좌
//! │   ├── fetch_positions() - 국내/해외 통합 포지션
//! │   └── fetch_pending_orders() - 국내/해외 통합 주문
//! ├── MarketDataProvider 구현
//! │   └── get_quote(symbol) - 심볼 패턴으로 국내/해외 분기
//! └── 내부
//!     ├── client: KisClient (KR/US 통합)
//!     └── oauth: Arc<KisOAuth> (공유)
//! ```

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::connector::kis::client::KisClient;
use crate::connector::kis::client_kr::KrOrderExecution;
use crate::connector::kis::config::KisAccountType;
use trader_core::cache::{ExchangeCache, TtlCache};
use trader_core::domain::{
    ExchangeProvider, ExecutionHistoryRequest, ExecutionHistoryResponse, MarketDataProvider,
    OrderExecution, OrderExecutionProvider, OrderRequest, OrderResponse, OrderType, PendingOrder,
    ProviderError, QuoteData, Side, StrategyAccountInfo, StrategyPositionInfo, Trade,
};
use trader_core::types::{MarketType, Symbol};
use trader_core::OrderStatusType;
use uuid::Uuid;

// ==================== 캐시 설정 ====================

/// 체결 내역 캐시 TTL (10분, ISA 전용)
const ORDER_HISTORY_CACHE_TTL: Duration = Duration::from_secs(600);

/// 심볼이 한국 주식인지 확인.
fn is_korean_symbol(symbol: &str) -> bool {
    symbol.len() == 6 && symbol.chars().all(|c| c.is_ascii_digit())
}

/// KIS 날짜/시간 파싱 (YYYYMMDD + HHMMSS → DateTime<Utc>).
fn parse_kis_datetime(date: &str, time: &str) -> Result<chrono::DateTime<chrono::Utc>, String> {
    if time.is_empty() {
        return Ok(chrono::Utc::now());
    }

    let datetime_str = format!("{} {}", date, time);
    let naive = chrono::NaiveDateTime::parse_from_str(&datetime_str, "%Y%m%d %H%M%S")
        .map_err(|e| format!("날짜 파싱 실패: {}", e))?;

    let kst_offset = chrono::FixedOffset::east_opt(9 * 3600).ok_or("KST offset 생성 실패")?;
    let kst_datetime = kst_offset
        .from_local_datetime(&naive)
        .single()
        .ok_or("KST datetime 변환 실패")?;

    Ok(kst_datetime.with_timezone(&chrono::Utc))
}

/// 한국투자증권 통합 Provider.
///
/// 국내/해외 주식을 하나의 인터페이스로 통합합니다.
/// ExchangeProvider와 MarketDataProvider를 모두 구현합니다.
///
/// 내부적으로 `KisClient`를 사용하여 KR/US API에 접근합니다.
pub struct KisExchangeProvider {
    /// 통합 클라이언트 (KR/US)
    client: Arc<KisClient>,
    /// 체결 내역 캐시 (ISA 계좌 전용, KIS-specific 타입)
    order_history_cache: TtlCache<Vec<KrOrderExecution>>,
    /// 거래소 공용 캐시 (계좌, 포지션, 미체결 주문)
    cache: Arc<ExchangeCache>,
}

/// 하위 호환성을 위한 타입 별칭.
pub type KisProvider = KisExchangeProvider;

impl KisExchangeProvider {
    /// KisClient로 Provider 생성.
    pub fn new(client: Arc<KisClient>) -> Self {
        Self {
            client,
            order_history_cache: TtlCache::new(ORDER_HISTORY_CACHE_TTL),
            cache: Arc::new(ExchangeCache::with_defaults()),
        }
    }

    /// 공용 캐시 참조 반환.
    ///
    /// 외부에서 캐시를 공유해야 하는 경우 사용합니다.
    pub fn exchange_cache(&self) -> Arc<ExchangeCache> {
        Arc::clone(&self.cache)
    }

    /// 모든 캐시 무효화.
    ///
    /// 주문 제출/취소/정정 후 자동 호출되어
    /// 다음 동기화 사이클에서 최신 데이터를 조회합니다.
    pub async fn invalidate_cache(&self) {
        self.order_history_cache.invalidate().await;
        self.cache.invalidate_all().await;
    }

    /// ISA 계좌 여부 확인.
    fn is_isa_account(&self) -> bool {
        self.client.is_isa_account()
    }

    /// 계좌 유형 반환.
    pub fn account_type(&self) -> KisAccountType {
        self.client.account_type()
    }

    /// 모든 체결 내역을 페이지네이션으로 조회 (캐시 활용).
    async fn fetch_all_order_history(&self) -> Result<Vec<KrOrderExecution>, ProviderError> {
        use chrono::{Datelike, NaiveDate};

        // 캐시 확인
        if let Some(cached) = self.order_history_cache.get().await {
            if let Some(remaining) = self.order_history_cache.remaining_ttl_secs().await {
                info!("체결 내역 캐시 히트: {} 건, 남은 TTL: {:.1}초", cached.len(), remaining);
            }
            return Ok(cached);
        }

        info!("체결 내역 조회 시작 (캐시 미스)");

        let mut all_executions = Vec::new();
        let today = chrono::Utc::now().date_naive();

        // 최근 2년간 조회 (년도별 분할)
        let mut current_year = today.year();
        let start_year = current_year - 2;

        while current_year >= start_year {
            let year_start = NaiveDate::from_ymd_opt(current_year, 1, 1).unwrap();
            let year_end = NaiveDate::from_ymd_opt(current_year, 12, 31)
                .unwrap()
                .min(today);

            let start_str = year_start.format("%Y%m%d").to_string();
            let end_str = year_end.format("%Y%m%d").to_string();

            debug!("체결 내역 조회: {} ~ {}", start_str, end_str);

            // 연속 조회 키를 사용한 페이지네이션
            let mut ctx_fk = String::new();
            let mut ctx_nk = String::new();
            let mut prev_ctx_nk = String::new();
            let mut page = 0;
            const MAX_PAGES: usize = 100; // 무한 루프 방지
            const API_DELAY_MS: u64 = 200;

            loop {
                // Rate Limiting
                if page > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(API_DELAY_MS)).await;
                }
                page += 1;

                if page > MAX_PAGES {
                    warn!("최대 페이지 수 도달: {}", MAX_PAGES);
                    break;
                }

                let history = match self
                    .client.kr()
                    .get_order_history(&start_str, &end_str, "00", &ctx_fk, &ctx_nk)
                    .await
                {
                    Ok(h) => h,
                    Err(e) => {
                        let error_msg = e.to_string();
                        // Rate Limit 에러 시 재시도
                        if error_msg.contains("초당")
                            || error_msg.contains("건수")
                            || error_msg.contains("exceeded")
                        {
                            warn!("Rate limit hit, waiting 2 seconds...");
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            match self
                                .client.kr()
                                .get_order_history(&start_str, &end_str, "00", &ctx_fk, &ctx_nk)
                                .await
                            {
                                Ok(h) => h,
                                Err(_) => break, // 재시도 실패 시 다음 범위로
                            }
                        } else {
                            // 오래된 날짜 범위에서 데이터 없음은 정상적인 상황
                            let is_no_data = error_msg.contains("no data")
                                || error_msg.contains("조회 결과가 없습니다")
                                || error_msg.contains("MDATETIME");

                            if is_no_data {
                                debug!("날짜 범위 {} ~ {}: 조회 결과 없음 (정상)", start_str, end_str);
                            } else {
                                warn!("날짜 범위 {} ~ {} 조회 실패: {}", start_str, end_str, e);
                            }
                            break;
                        }
                    }
                };

                let count = history.executions.len();
                all_executions.extend(history.executions);

                if count > 0 {
                    debug!(
                        "날짜 범위 {} ~ {}, 페이지 {}: {} 건 발견",
                        start_str, end_str, page, count
                    );
                }

                // 종료 조건들
                if !history.has_more {
                    break;
                }
                if prev_ctx_nk == history.ctx_area_nk100 && !prev_ctx_nk.is_empty() {
                    break;
                }
                if history.ctx_area_nk100.is_empty() {
                    break;
                }

                prev_ctx_nk = ctx_nk.clone();
                ctx_fk = history.ctx_area_fk100;
                ctx_nk = history.ctx_area_nk100;
            }

            current_year -= 1;
        }

        info!("체결 내역 조회 완료: 총 {} 건", all_executions.len());

        // 캐시 저장
        self.order_history_cache.set(all_executions.clone()).await;

        Ok(all_executions)
    }

    /// KrOrderExecution → 거래소 중립 OrderExecution 변환.
    fn kr_execution_to_neutral(exec: &KrOrderExecution) -> OrderExecution {
        OrderExecution {
            order_date: exec.order_date.clone(),
            order_no: exec.order_no.clone(),
            original_order_no: exec.original_order_no.clone(),
            order_time: exec.order_time.clone(),
            side: if exec.side_code == "02" {
                "buy".to_string()
            } else {
                "sell".to_string()
            },
            symbol: exec.stock_code.clone(),
            name: exec.stock_name.clone(),
            order_qty: exec.order_qty,
            order_price: exec.order_price,
            filled_qty: exec.filled_qty,
            avg_price: exec.avg_price,
            filled_amount: exec.filled_amount,
            order_type: exec.order_type_name.clone(),
            is_cancelled: exec.cancel_yn == "Y",
        }
    }

    /// 동기화용 체결 내역 조회.
    ///
    /// 특정 날짜 범위의 체결 내역을 거래소 중립 타입으로 반환합니다.
    /// ISA 계좌는 1년 단위, 일반 계좌는 3개월 단위로 분할 조회합니다.
    ///
    /// # Arguments
    ///
    /// * `start_date` - 시작일 (YYYYMMDD 형식)
    /// * `end_date` - 종료일 (YYYYMMDD 형식)
    /// * `is_testnet` - 모의투자 계좌 여부 (Rate Limiting에 영향)
    ///
    /// # Returns
    ///
    /// 거래소 중립 체결 내역 목록
    pub async fn fetch_execution_history_for_sync(
        &self,
        start_date: &str,
        end_date: &str,
        is_testnet: bool,
    ) -> Result<Vec<OrderExecution>, ProviderError> {
        use chrono::NaiveDate;

        let start = NaiveDate::parse_from_str(start_date, "%Y%m%d")
            .map_err(|e| ProviderError::Api(format!("날짜 파싱 실패 (start): {}", e)))?;
        let end = NaiveDate::parse_from_str(end_date, "%Y%m%d")
            .map_err(|e| ProviderError::Api(format!("날짜 파싱 실패 (end): {}", e)))?;

        // Rate Limit 설정 (실계좌: 200ms, 모의계좌: 520ms)
        let api_delay_ms: u64 = if is_testnet { 520 } else { 200 };

        // ISA 계좌: 1년 단위, 일반 계좌: 3개월 단위 분할
        let max_days = if self.is_isa_account() { 365 } else { 90 };

        // 날짜 범위 분할
        let mut date_ranges: Vec<(String, String)> = Vec::new();
        let mut current_start = start;

        while current_start <= end {
            let current_end = std::cmp::min(
                current_start + chrono::Duration::days(max_days - 1),
                end,
            );
            date_ranges.push((
                current_start.format("%Y%m%d").to_string(),
                current_end.format("%Y%m%d").to_string(),
            ));
            current_start = current_end + chrono::Duration::days(1);
        }

        debug!(
            "동기화용 체결 내역 조회: {} ~ {}, {} 청크 ({})",
            start_date,
            end_date,
            date_ranges.len(),
            if self.is_isa_account() { "ISA" } else { "일반" }
        );

        let mut all_executions = Vec::new();
        const MAX_PAGES: usize = 50;

        for (range_idx, (range_start, range_end)) in date_ranges.iter().enumerate() {
            debug!(
                "청크 {}/{}: {} ~ {}",
                range_idx + 1,
                date_ranges.len(),
                range_start,
                range_end
            );

            let mut ctx_fk = String::new();
            let mut ctx_nk = String::new();
            let mut prev_ctx_nk = String::new();
            let mut page = 0;

            loop {
                // Rate Limiting
                if page > 0 || range_idx > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(api_delay_ms)).await;
                }
                page += 1;

                if page > MAX_PAGES {
                    warn!("최대 페이지 수 도달: {}", MAX_PAGES);
                    break;
                }

                let history = match self
                    .client.kr()
                    .get_order_history(range_start, range_end, "00", &ctx_fk, &ctx_nk)
                    .await
                {
                    Ok(h) => h,
                    Err(e) => {
                        let error_msg = e.to_string();
                        // Rate Limit 에러 시 재시도
                        if error_msg.contains("초당")
                            || error_msg.contains("건수")
                            || error_msg.contains("exceeded")
                        {
                            warn!("Rate limit hit, waiting 2 seconds...");
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            match self
                                .client.kr()
                                .get_order_history(range_start, range_end, "00", &ctx_fk, &ctx_nk)
                                .await
                            {
                                Ok(h) => h,
                                Err(_) => break,
                            }
                        } else {
                            let is_no_data = error_msg.contains("no data")
                                || error_msg.contains("조회 결과가 없습니다")
                                || error_msg.contains("MDATETIME");

                            if is_no_data {
                                debug!("날짜 범위 {} ~ {}: 조회 결과 없음", range_start, range_end);
                            } else {
                                warn!("날짜 범위 {} ~ {} 조회 실패: {}", range_start, range_end, e);
                            }
                            break;
                        }
                    }
                };

                let count = history.executions.len();
                all_executions.extend(history.executions);

                if count > 0 {
                    debug!(
                        "청크 {} 페이지 {}: {} 건",
                        range_idx + 1,
                        page,
                        count
                    );
                }

                // 종료 조건
                if !history.has_more {
                    break;
                }
                if prev_ctx_nk == history.ctx_area_nk100 && !prev_ctx_nk.is_empty() {
                    break;
                }
                if history.ctx_area_nk100.is_empty() {
                    break;
                }

                prev_ctx_nk = ctx_nk.clone();
                ctx_fk = history.ctx_area_fk100;
                ctx_nk = history.ctx_area_nk100;
            }
        }

        info!(
            "동기화용 체결 내역 조회 완료: {} ~ {}, 총 {} 건",
            start_date,
            end_date,
            all_executions.len()
        );

        // KrOrderExecution → 거래소 중립 OrderExecution 변환
        let neutral_executions = all_executions
            .iter()
            .map(Self::kr_execution_to_neutral)
            .collect();

        Ok(neutral_executions)
    }

    /// 잔고 정보 조회 (동기화용).
    ///
    /// ISA 계좌의 경우에도 가능한 정보를 반환합니다.
    pub async fn get_balance_for_sync(&self) -> Result<StrategyAccountInfo, ProviderError> {
        self.fetch_account().await
    }

    /// 통합 클라이언트 접근.
    ///
    /// 체결 내역 원본 데이터가 필요한 경우 사용합니다.
    pub fn client(&self) -> &KisClient {
        &self.client
    }


}

#[async_trait]
impl ExchangeProvider for KisExchangeProvider {
    async fn fetch_account(&self) -> Result<StrategyAccountInfo, ProviderError> {
        // 캐시 확인
        if let Some(cached) = self.cache.get_account().await {
            debug!("계좌 정보 캐시 히트");
            return Ok(cached);
        }

        // ISA 계좌는 체결 내역 + 포지션 현재가 기반으로 계산
        let account_info = if self.is_isa_account() {
            debug!("ISA 계좌: 체결 내역 + 포지션 현재가 기반 계좌 조회");

            // 포지션 조회 (현재가 포함)
            let positions = self.fetch_positions().await?;

            // 투자 원금 (매입 총액) 및 현재 평가액 계산
            let mut total_cost = Decimal::ZERO;
            let mut total_eval = Decimal::ZERO;
            let mut total_unrealized_pnl = Decimal::ZERO;

            for pos in &positions {
                let cost = pos.avg_entry_price * pos.quantity;
                let eval = pos.current_price * pos.quantity;
                total_cost += cost;
                total_eval += eval;
                total_unrealized_pnl += pos.unrealized_pnl;
            }

            info!(
                "ISA 계좌 요약: 투자원금={}, 평가액={}, 미실현손익={}, 종목수={}",
                total_cost, total_eval, total_unrealized_pnl, positions.len()
            );

            StrategyAccountInfo {
                total_balance: total_eval,
                available_balance: Decimal::ZERO, // ISA는 잔여 현금 조회 불가
                margin_used: Decimal::ZERO,
                unrealized_pnl: total_unrealized_pnl,
                currency: "KRW".to_string(),
            }
        } else {
            // 일반 계좌: get_balance() API 호출
            let balance = self
                .client.kr()
                .get_balance()
                .await
                .map_err(|e| ProviderError::Api(format!("잔고 조회 실패: {}", e)))?;

            // AccountBalance에서 직접 계산
            let holdings_value = balance.holdings_value();
            let total_balance = balance
                .total_eval_amount
                .unwrap_or(balance.cash_balance + holdings_value);
            let unrealized_pnl = balance
                .total_profit_loss
                .unwrap_or_else(|| balance.holdings.iter().map(|h| h.profit_loss).sum());

            StrategyAccountInfo {
                total_balance,
                available_balance: balance.cash_balance,
                margin_used: Decimal::ZERO,
                unrealized_pnl,
                currency: balance.currency,
            }
        };

        // 캐시 저장
        self.cache.set_account(account_info.clone()).await;

        Ok(account_info)
    }

    async fn fetch_positions(&self) -> Result<Vec<StrategyPositionInfo>, ProviderError> {
        // 캐시 확인
        if let Some(cached) = self.cache.get_positions().await {
            debug!("포지션 캐시 히트: {} 건", cached.len());
            return Ok(cached);
        }

        let positions: Vec<StrategyPositionInfo> = if self.is_isa_account() {
            // ISA: 체결 내역 기반으로 포지션 계산
            debug!("ISA 계좌: 체결 내역 기반 포지션 계산");

            let executions = self.fetch_all_order_history().await?;

            // 종목별 포지션 집계 (수량, 총 비용)
            use std::collections::HashMap;
            let mut position_map: HashMap<String, (Decimal, Decimal)> = HashMap::new();

            for exec in &executions {
                let entry = position_map
                    .entry(exec.stock_code.clone())
                    .or_insert((Decimal::ZERO, Decimal::ZERO));

                if exec.side_code == "02" {
                    // 매수: 수량 증가, 비용 증가
                    entry.0 += exec.filled_qty;
                    entry.1 += exec.avg_price * exec.filled_qty;
                } else {
                    // 매도: 수량 감소, 비율에 따라 비용 감소
                    entry.0 -= exec.filled_qty;
                    if entry.0 > Decimal::ZERO {
                        // 비례적으로 평균단가 조정
                        let ratio = entry.0 / (entry.0 + exec.filled_qty);
                        entry.1 *= ratio;
                    } else {
                        entry.1 = Decimal::ZERO;
                    }
                }
            }

            // 양수 포지션만 필터링
            let mut positions: Vec<StrategyPositionInfo> = position_map
                .into_iter()
                .filter(|(_, (qty, _))| *qty > Decimal::ZERO)
                .map(|(code, (qty, cost))| {
                    let avg_price = if qty > Decimal::ZERO {
                        cost / qty
                    } else {
                        Decimal::ZERO
                    };
                    StrategyPositionInfo::new(code, Side::Buy, qty, avg_price)
                })
                .collect();

            // ISA: 각 종목의 현재가를 시세 API로 조회하여 손익 계산
            for position in &mut positions {
                match self.client.kr().get_price(&position.ticker).await {
                    Ok(price) => {
                        position.update_price(price.current_price);
                        debug!(
                            "ISA 포지션 현재가 업데이트: {} 매입가={} 현재가={} 손익률={}%",
                            position.ticker, position.avg_entry_price,
                            position.current_price, position.unrealized_pnl_pct
                        );
                    }
                    Err(e) => {
                        warn!(
                            "ISA 포지션 현재가 조회 실패 (매입가 유지): {} - {}",
                            position.ticker, e
                        );
                    }
                }
                // Rate Limit 방지: 종목 간 200ms 대기
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }

            positions
        } else {
            // 일반 계좌: get_balance()로 보유종목 조회
            let balance = self
                .client.kr()
                .get_balance()
                .await
                .map_err(|e| ProviderError::Api(format!("보유종목 조회 실패: {}", e)))?;

            balance
                .holdings
                .iter()
                .filter(|h| h.quantity > Decimal::ZERO)
                .map(|h| {
                    let mut position = StrategyPositionInfo::new(
                        h.symbol.clone(),
                        Side::Buy,
                        h.quantity,
                        h.avg_price,
                    );
                    position.update_price(h.current_price);
                    position
                })
                .collect()
        };

        // 캐시 저장
        self.cache.set_positions(positions.clone()).await;

        Ok(positions)
    }

    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>, ProviderError> {
        // 캐시 확인
        if let Some(cached) = self.cache.get_pending_orders().await {
            debug!("미체결 주문 캐시 히트: {} 건", cached.len());
            return Ok(cached);
        }

        // 미체결 주문 조회 (국내)
        let orders = self
            .client.kr()
            .get_pending_orders()
            .await
            .map_err(|e| ProviderError::Api(format!("미체결 주문 조회 실패: {}", e)))?;

        let mut result = Vec::new();

        for order in orders {
            // 매수/매도 구분 변환 (01=매도, 02=매수)
            let side = match order.side_code.as_str() {
                "01" => Side::Sell,
                "02" => Side::Buy,
                _ => continue, // 알 수 없는 side는 스킵
            };

            // 심볼 생성
            let symbol = Symbol::new(&order.stock_code, "KRW", MarketType::Stock);

            // 주문 상태 결정
            let status = if order.filled_qty > Decimal::ZERO {
                OrderStatusType::PartiallyFilled
            } else {
                OrderStatusType::Open
            };

            // 주문 시각 파싱
            let created_at = parse_kis_datetime(&order.order_date, &order.order_time)
                .unwrap_or_else(|_| Utc::now());

            let pending = PendingOrder {
                order_id: order.order_no,
                ticker: symbol.to_string(),
                side,
                price: order.order_price,
                quantity: order.order_qty,
                filled_quantity: order.filled_qty,
                status,
                created_at,
            };

            result.push(pending);
        }

        // 캐시 저장
        self.cache.set_pending_orders(result.clone()).await;

        Ok(result)
    }

    fn exchange_name(&self) -> &str {
        "한국투자증권"
    }

    async fn fetch_execution_history(
        &self,
        request: &ExecutionHistoryRequest,
    ) -> Result<ExecutionHistoryResponse, ProviderError> {
        // cursor를 ctx_fk와 ctx_nk로 분리 ("|"로 구분)
        let (ctx_fk, ctx_nk) = if let Some(ref cursor) = request.cursor {
            let parts: Vec<&str> = cursor.split('|').collect();
            if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (String::new(), String::new())
            }
        } else {
            (String::new(), String::new())
        };

        let history = self
            .client.kr()
            .get_order_history(&request.start_date, &request.end_date, "00", &ctx_fk, &ctx_nk)
            .await
            .map_err(|e| ProviderError::Api(format!("체결 내역 조회 실패: {}", e)))?;

        let trades: Vec<Trade> = history
            .executions
            .into_iter()
            .filter_map(|exec| {
                let side = if exec.side_code == "02" {
                    Side::Buy
                } else {
                    Side::Sell
                };

                let executed_at = parse_kis_datetime(&exec.order_date, &exec.order_time).ok()?;

                Some(
                    Trade::new(
                        Uuid::new_v4(),
                        "kis",
                        exec.order_no.clone(),
                        exec.stock_code,
                        side,
                        exec.filled_qty,
                        exec.avg_price,
                    )
                    .with_executed_at(executed_at),
                )
            })
            .collect();

        // next_cursor는 ctx_fk|ctx_nk 형식으로 조합
        let next_cursor = if history.has_more && !history.ctx_area_nk100.is_empty() {
            Some(format!("{}|{}", history.ctx_area_fk100, history.ctx_area_nk100))
        } else {
            None
        };

        Ok(ExecutionHistoryResponse {
            trades,
            next_cursor,
        })
    }
}

#[async_trait]
impl MarketDataProvider for KisExchangeProvider {
    async fn get_quote(&self, symbol: &str) -> Result<QuoteData, ProviderError> {
        if is_korean_symbol(symbol) {
            // 국내 주식 시세 조회
            debug!(symbol = %symbol, "국내 주식 시세 조회");
            let price = self
                .client.kr()
                .get_price(symbol)
                .await
                .map_err(|e| ProviderError::Api(e.to_string()))?;

            Ok(QuoteData {
                symbol: symbol.to_string(),
                current_price: price.current_price,
                price_change: price.price_change,
                change_percent: price.change_rate,
                high: price.high,
                low: price.low,
                open: price.open,
                prev_close: price.prev_close,
                volume: price.volume,
                trading_value: price.trading_value,
                timestamp: Utc::now(),
            })
        } else {
            // 해외 주식 시세 조회
            debug!(symbol = %symbol, "해외 주식 시세 조회");
            let price = self
                .client.us()
                .get_price(symbol, None)
                .await
                .map_err(|e| ProviderError::Api(e.to_string()))?;

            Ok(QuoteData {
                symbol: symbol.to_string(),
                current_price: price.current_price,
                price_change: price.price_change,
                change_percent: price.change_rate,
                high: price.high,
                low: price.low,
                open: price.open,
                prev_close: price.prev_close,
                volume: price.volume,
                trading_value: price.trading_value,
                timestamp: Utc::now(),
            })
        }
    }

    fn provider_name(&self) -> &str {
        "한국투자증권"
    }
}

// =============================================================================
// OrderExecutionProvider 구현
// =============================================================================

#[async_trait]
impl OrderExecutionProvider for KisExchangeProvider {
    async fn place_order(&self, request: &OrderRequest) -> Result<OrderResponse, ProviderError> {
        let is_korean = is_korean_symbol(&request.ticker);

        // OrderType → KIS 주문 구분 코드 변환
        let order_type_code = match request.order_type {
            OrderType::Market => "01",
            OrderType::Limit => "00",
            // KIS는 손절/익절을 별도 주문 유형으로 지원하지 않음 → 지정가로 변환
            OrderType::StopLoss | OrderType::StopLossLimit => "00",
            OrderType::TakeProfit | OrderType::TakeProfitLimit => "00",
            OrderType::TrailingStop => {
                return Err(ProviderError::Unsupported(
                    "KIS는 트레일링 스톱 주문을 지원하지 않습니다".to_string(),
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
            _ => request
                .price
                .or(request.stop_price)
                .unwrap_or(Decimal::ZERO),
        };

        let response = if is_korean {
            match request.side {
                Side::Buy => {
                    self.client
                        .place_kr_buy_order(&request.ticker, quantity, price, order_type_code)
                        .await
                }
                Side::Sell => {
                    self.client
                        .place_kr_sell_order(&request.ticker, quantity, price, order_type_code)
                        .await
                }
            }
        } else {
            match request.side {
                Side::Buy => {
                    self.client
                        .place_us_buy_order(
                            &request.ticker,
                            quantity,
                            price,
                            order_type_code,
                            None,
                        )
                        .await
                }
                Side::Sell => {
                    self.client
                        .place_us_sell_order(
                            &request.ticker,
                            quantity,
                            price,
                            order_type_code,
                            None,
                        )
                        .await
                }
            }
        };

        // 캐시 무효화 (주문 후 포지션/계좌 변동)
        self.invalidate_cache().await;

        response.map_err(|e| ProviderError::Api(format!("주문 실패: {}", e)))
    }

    async fn cancel_order(&self, order_id: &str, ticker: &str) -> Result<(), ProviderError> {
        let is_korean = is_korean_symbol(ticker);

        let result = if is_korean {
            self.client.cancel_kr_order(order_id, ticker, 0).await
        } else {
            self.client
                .cancel_us_order(order_id, ticker, 0, None)
                .await
        };

        self.invalidate_cache().await;

        result
            .map(|_| ())
            .map_err(|e| ProviderError::Api(format!("주문 취소 실패: {}", e)))
    }

    async fn modify_order(
        &self,
        order_id: &str,
        ticker: &str,
        quantity: Option<Decimal>,
        price: Option<Decimal>,
    ) -> Result<OrderResponse, ProviderError> {
        let is_korean = is_korean_symbol(ticker);

        let qty = quantity
            .map(|q| {
                q.to_string()
                    .parse::<f64>()
                    .map(|v| v.floor() as u32)
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        let order_price = price.unwrap_or(Decimal::ZERO);

        let response = if is_korean {
            self.client
                .modify_kr_order(order_id, ticker, qty, order_price)
                .await
        } else {
            self.client
                .modify_us_order(order_id, ticker, qty, order_price, None)
                .await
        };

        self.invalidate_cache().await;

        response.map_err(|e| ProviderError::Api(format!("주문 정정 실패: {}", e)))
    }

    fn exchange_name(&self) -> &str {
        "한국투자증권"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_korean_symbol() {
        // 한국 주식 코드 (6자리 숫자)
        assert!(is_korean_symbol("005930")); // 삼성전자
        assert!(is_korean_symbol("000660")); // SK하이닉스
        assert!(is_korean_symbol("035720")); // 카카오

        // 미국 주식 티커
        assert!(!is_korean_symbol("AAPL"));
        assert!(!is_korean_symbol("MSFT"));
        assert!(!is_korean_symbol("SPY"));

        // 잘못된 형식
        assert!(!is_korean_symbol("00593")); // 5자리
        assert!(!is_korean_symbol("0059300")); // 7자리
        assert!(!is_korean_symbol("A05930")); // 문자 포함
    }
}
