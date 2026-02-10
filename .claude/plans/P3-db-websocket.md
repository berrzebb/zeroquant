# Plan: [P3] DB증권 WebSocket 구현 (High)

> 현재 완전 placeholder. KIS/Upbit 패턴 참조하여 구현.
> **WebSocket URL 확인 완료**: `wss://openapi.dbsec.co.kr:7070/websocket` (운영), `:17070` (모의투자)

## 선행 조건
- 없음 (WS URL 확인 완료, 즉시 착수 가능)

## 예상 규모
Large

---

## P3-A: WebSocket 기반 구조

**대상**: `crates/trader-exchange/src/connector/db_investment/websocket.rs`

- [x] DB증권 API 문서에서 WebSocket URL 확인
  - 운영: `wss://openapi.dbsec.co.kr:7070/websocket`
  - 모의투자: `wss://openapi.dbsec.co.kr:17070/websocket`
  - Format: JSON (`application/json;charset=utf-8`)
- [ ] `DbWsMessage` enum 확장
  - `Trade(QuoteData)`, `Orderbook(serde_json::Value)`, `ConnectionStatus(bool)`, `Error(String)`
- [ ] `DbWsCommand` enum 확장
  - `Subscribe(String)`, `SubscribeOrderbook(String)`, `Unsubscribe(String)`
- [ ] `command_tx` / `command_rx` 채널 추가
- [ ] `subscribed_tickers: Arc<RwLock<Vec<String>>>` 추가
- [ ] `connect()` → 재연결 루프 (KIS 패턴)
- [ ] `run_session()` → 단일 세션 (WS 연결, select! 루프)
- [ ] 세션 초기화 메시지 전송
- [ ] ConnectionStatus 이벤트 발행

## P3-B: 체결가/호가 파싱

- [ ] 체결가 메시지 파싱 → `QuoteData` 변환
- [ ] 호가 메시지 파싱 → `OrderBook` 변환 (구조 확인 후)

## P3-C: MarketStream 통합

**대상**: `crates/trader-exchange/src/stream.rs`

- [ ] `DbInvestmentMarketStream` 구조체 구현
- [ ] `MarketStream for DbInvestmentMarketStream` trait impl
- [ ] `next_event()`에서 ConnectionStatus/Trade/Orderbook 매핑
- [ ] `subscribe_ticker()`, `subscribe_order_book()` 구현
- [ ] `UnifiedMarketStream`에 DB증권 라우팅 추가

## 검증
```bash
cargo check -p trader-exchange
cargo test -p trader-exchange
cargo clippy -p trader-exchange -- -W warnings
```

## 관련 파일
- `crates/trader-exchange/src/connector/db_investment/websocket.rs`
- `crates/trader-exchange/src/stream.rs`
