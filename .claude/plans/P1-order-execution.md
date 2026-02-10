# Plan: [P1] 주문 실행 API 구현 (Critical) ✅ 완료

> 실거래 필수 기능. KIS/Binance만 주문 가능한 현재 상태를 해소.
> **전체 완료**: P1-A(Upbit), P1-B(LS증권), P1-C(DB증권), P1-D(Bithumb) 모두 구현됨.

## 선행 조건
- 없음 (즉시 착수 가능)

## 예상 규모
Medium

---

## ✅ P1-C: DB증권 주문 실행 (완료)

> 참조: `docs/exchange/db_openapi_spec.md` §3

**대상 파일**: `crates/trader-exchange/src/connector/db_investment/client.rs`, `crates/trader-exchange/src/provider/db_investment.rs`

- [x] `place_order()` 구현
  - KR: TR `CSPAT00600` (확정) — `POST /api/v1/trading/kr-stock/order`
  - body: `{InputCondMrktDivCode(1:코스피/2:코스닥), InputIscd, InputQty, InputPrc, InputDvsnCode(1:매수/2:매도), InputOrdPtnCode(00:지정가/01:시장가)}`
  - 응답: `OrdNo`(주문번호), `OrdSttCd`(주문상태코드)
- [x] `cancel_order()` 구현
  - KR: TR `CSPAT00602` — `POST /api/v1/trading/kr-stock/order-cancel`
  - body: `{InputOrdNo}`
- [x] `modify_order()` 구현
  - KR: TR `CSPAT00601` — `POST /api/v1/trading/kr-stock/order-modify`
  - body: `{InputOrdNo, InputOrdQty, InputOrdPrc}`
- [x] `OrderExecutionProvider for DbInvestmentExchangeProvider` trait impl 추가

**검증 결과**: ✅ cargo check/test/clippy 통과

## 검증
```bash
cargo check -p trader-exchange
cargo test -p trader-exchange
cargo clippy -p trader-exchange -- -W warnings
```

## 관련 파일
- `crates/trader-exchange/src/connector/db_investment/client.rs`
- `crates/trader-exchange/src/provider/db_investment.rs`
