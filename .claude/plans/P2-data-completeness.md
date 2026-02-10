# Plan: [P2] 데이터 완전성 (High) ✅ 완료

> 시세, 체결내역, 호가 데이터 누락 해소. P1과 병렬 진행 가능.
> **전체 완료**: P2-A, P2-B, P2-C, P2-D 모두 구현됨.

## 선행 조건
- 없음 (P1과 병렬 가능)

## 예상 규모
Medium-Large

---

## ✅ P2-B: 체결 내역 조회 (완료)

> 매매일지, Performance Tracker에 필요

### Upbit
- [x] `GET /v1/orders/closed?state=done` 호출 구현
- [x] `fetch_execution_history()` 구현
- [x] 날짜 변환 (YYYYMMDD → ISO 8601)
- [x] KST → UTC 시간 변환

**구현**: `connector/upbit/client.rs` (L260-358), `provider/upbit.rs` (L81-145)

### LS증권
- [x] TR 코드 `t0425` 사용
- [x] `fetch_execution_history()` 구현
- [x] 체결시간 파싱 (HHmmss → DateTime<Utc>)

**구현**: `connector/ls_sec/client.rs`, `provider/ls_sec.rs`

### DB증권
- [x] `api/v1/trading/kr-stock/inquiry/transaction-history` (ExecYn=1 체결)
- [x] `fetch_execution_history()` 구현
- [x] A 접두사 제거 (A005930 → 005930)

**구현**: `connector/db_investment/client.rs`, `provider/db_investment.rs`

### Bithumb
- [x] `GET /v1/trades` API 구현 (§2.7)
- [x] `fetch_execution_history()` 구현
- [x] `BithumbTrade` → `Trade` 변환

**구현**: `connector/bithumb/client.rs` (L287-365), `provider/bithumb.rs` (L80-89)

**검증 결과**: ✅ cargo check/clippy 전체 통과, 워닝 0개

---

## ✅ P2-C: WebSocket Orderbook 파싱 (완료)

> Upbit/LS증권에서 호가 데이터가 수신되지만 파싱 안 되는 문제 → **해결 완료**

### Upbit Orderbook

**대상**: `crates/trader-exchange/src/connector/upbit/websocket.rs`, `stream.rs`

- [x] `send_subscription()`에 orderbook 타입 추가 (`{"type": "orderbook", "codes": [...]}`)
- [x] `UpbitWsCommand`에 `SubscribeOrderbook(Vec<String>)` 추가
- [x] `parse_orderbook()` 메서드 구현
  - `orderbook_units[].ask_price/bid_price/ask_size/bid_size` 파싱
  - `total_ask_size`, `total_bid_size` 파싱
  - → `OrderBook` 구조체 변환
- [x] `next_event()`에서 `Orderbook(Value)` → `MarketEvent::OrderBook` 매핑
- [x] `subscribe_order_book()` — `NotSupported` → 실제 구독 로직

### LS증권 H1_ 호가

> **TR코드 확인 완료**: S3_=체결, H1_/H2_=호가로 구현됨

**대상**: `crates/trader-exchange/src/connector/ls_sec/websocket.rs`, `stream.rs`

- [x] 현재 코드의 H1_/S3_ 매핑 확인 (체결/호가 역할 검증)
- [x] `handle_message()`에서 호가 TR 처리 추가 (H1_/H2_)
- [x] `parse_orderbook()` 메서드 구현
  - 캐럿(^) 구분 필드에서 매도/매수 10호가+잔량 파싱
  - → `OrderBook` 구조체 변환
- [x] `LsWsMessage::Orderbook(Value)` → `LsWsMessage::Orderbook(OrderBook)` 타입 변경
- [x] `next_event()`에서 `Orderbook` → `MarketEvent::OrderBook` 매핑

**검증 결과**: ✅ cargo check/test/clippy 통과 (105 tests passed)

---

## ✅ P2-D: Upbit Trade WebSocket 구독 (완료)

**대상**: `crates/trader-exchange/src/connector/upbit/websocket.rs`, `stream.rs`

- [x] `UpbitWsMessage`에 `Trade(TradeTick)` variant 추가
- [x] `UpbitWsCommand`에 `SubscribeTrade(Vec<String>)` 추가
- [x] `send_subscription()`에 trade 타입 추가
- [x] `parse_trade()` 메서드 구현
  - `trade_price`, `trade_volume`, `ask_bid` 파싱
  - `sequential_id` 파싱
  - → `TradeTick` 구조체 변환
- [x] `next_event()`에서 `Trade` → `MarketEvent::Trade` 매핑
- [x] `subscribe_trades()` 독립 구독 구현

**검증 결과**: ✅ cargo check/test 통과, 195줄 추가/수정

## 검증
```bash
cargo check -p trader-exchange
cargo test -p trader-exchange
cargo clippy -p trader-exchange -- -W warnings
```

## 관련 파일
- `crates/trader-exchange/src/connector/upbit/websocket.rs`
- `crates/trader-exchange/src/connector/ls_sec/websocket.rs`
- `crates/trader-exchange/src/stream.rs`
- 각 거래소 `client.rs` + `provider.rs`
