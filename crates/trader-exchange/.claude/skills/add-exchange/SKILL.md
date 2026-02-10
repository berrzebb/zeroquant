---
name: add-exchange
description: 새 거래소 커넥터 추가 체크리스트. connector + provider + traits 구현.
disable-model-invocation: true
user-invocable: true
argument-hint: "<거래소명_snake_case> [시장유형: kr|us|crypto|global]"
allowed-tools: Read, Grep, Edit, Write, Bash(cargo *)
context: fork
  agent: rust-impl
---

# 거래소 커넥터 추가 워크플로우

새 거래소 `$ARGUMENTS[0]`을 추가합니다.

---

## 0단계: 기존 거래소 참조

```bash
# 가장 유사한 기존 거래소 확인
ls crates/trader-exchange/src/connector/
ls crates/trader-exchange/src/provider/
```

추천 참조: `upbit/` (가장 간결한 구조)

---

## 1단계: Connector 구현

**위치**: `crates/trader-exchange/src/connector/$ARGUMENTS[0]/`

### 필수 파일 구조

```
connector/$ARGUMENTS[0]/
├── mod.rs         # pub 모듈 선언
├── client.rs      # HTTP 클라이언트 (REST API 호출)
├── models.rs      # API 요청/응답 타입
└── websocket.rs   # WebSocket 스트림 (선택)
```

### client.rs 필수 메서드

```rust
pub struct ExchangeClient {
    base_url: String,
    client: reqwest::Client,
}

impl ExchangeClient {
    // 시세 조회
    pub async fn get_ticker(&self, symbol: &str) -> Result<TickerResponse>;
    pub async fn get_orderbook(&self, symbol: &str) -> Result<OrderbookResponse>;
    pub async fn get_candles(&self, symbol: &str, interval: &str) -> Result<Vec<CandleResponse>>;

    // 주문 (인증 필요)
    pub async fn place_order(&self, order: &OrderRequest) -> Result<OrderResponse>;
    pub async fn cancel_order(&self, order_id: &str) -> Result<()>;
    pub async fn get_balance(&self) -> Result<BalanceResponse>;
}
```

### 체크포인트
- [ ] 모든 가격/수량은 `Decimal` (API의 f64/String 응답 즉시 변환)
- [ ] API 키 하드코딩 금지 (DB 암호화 저장 참조)
- [ ] Rate Limit 준수 (`circuit_breaker.rs`, `retry.rs` 활용)
- [ ] 에러 타입은 `ExchangeError` 사용

---

## 2단계: Provider 구현

**위치**: `crates/trader-exchange/src/provider/$ARGUMENTS[0].rs`

```rust
use crate::traits::{ExchangeProvider, OrderExecutionProvider};

pub struct ExchangeProviderImpl { /* ... */ }

impl ExchangeProvider for ExchangeProviderImpl {
    async fn get_ticker(&self, symbol: &str) -> Result<Ticker>;
    async fn get_orderbook(&self, symbol: &str) -> Result<Orderbook>;
    // ...
}

impl OrderExecutionProvider for ExchangeProviderImpl {
    async fn place_order(&self, order: OrderRequest) -> Result<OrderResult>;
    async fn cancel_order(&self, order_id: &str) -> Result<()>;
    async fn get_balance(&self) -> Result<Balance>;
}
```

---

## 3단계: 모듈 등록

1. `crates/trader-exchange/src/connector/mod.rs` — `pub mod $ARGUMENTS[0];`
2. `crates/trader-exchange/src/provider/mod.rs` — `pub mod $ARGUMENTS[0];`
3. `crates/trader-exchange/src/lib.rs` — 팩토리 함수에 match 분기 추가

---

## 4단계: WebSocket 스트림 (선택)

`MarketStream` trait 구현:

```rust
impl MarketStream for ExchangeStream {
    async fn subscribe_ticker(&mut self, symbols: &[String]) -> Result<()>;
    async fn subscribe_orderbook(&mut self, symbols: &[String]) -> Result<()>;
}
```

---

## 5단계: API 명세 문서

**위치**: `docs/exchange/$ARGUMENTS[0]_openapi_spec.md`

> `/crawl-api-spec` 스킬로 자동 생성 가능

---

## 6단계: 검증

```powershell
cargo check -p trader-exchange
cargo clippy -p trader-exchange -- -D warnings
cargo test -p trader-exchange -- $ARGUMENTS[0]
```
