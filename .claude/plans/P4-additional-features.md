# Plan: [P4] 부가 기능 (Medium)

> P1~P3 안정화 후 필요에 따라 진행

## 선행 조건
- P1~P3 안정화

## 예상 규모
Medium

---

## P4-A: LS증권/DB증권 US 기능 확장

- [ ] LS증권 US 미체결 조회 (`COSAT00301` 또는 유사 TR)
- [ ] LS증권 US 시세 조회 (해외주식 TR)
- [ ] LS증권 US 잔고 (`COSOQ02701`) 파싱 활성화
- [ ] LS증권 US 주문/취소/정정
- [ ] DB증권 US 미체결 조회
- [ ] DB증권 US 시세 조회

## P4-B: Upbit Private WebSocket

> 실시간 주문 상태 추적

**대상**: `crates/trader-exchange/src/connector/upbit/websocket.rs`

- [ ] Private WS 엔드포인트 연결 (`wss://api.upbit.com/websocket/v1/private`)
- [ ] JWT 인증 헤더 추가
- [ ] `myOrder` 구독 → 주문 상태 실시간 추적
- [ ] `myAsset` 구독 → 자산 변동 실시간 반영

## P4-C: REST 확장

- [ ] Upbit `GET /v1/market/all` — 마켓 목록 자동 탐색
- [ ] Upbit `GET /v1/candles/{unit}` — OHLCV 데이터 수집
- [ ] LS증권 REST 호가 조회
- [ ] DB증권 REST 호가 조회
- [ ] 캔들/차트 데이터 REST 조회 (DB증권)

## 검증
```bash
cargo check -p trader-exchange
cargo test -p trader-exchange
cargo clippy -p trader-exchange -- -W warnings
```

## 관련 파일
- `crates/trader-exchange/src/connector/upbit/websocket.rs`
- `crates/trader-exchange/src/connector/ls_sec/`
- `crates/trader-exchange/src/connector/db_investment/`
