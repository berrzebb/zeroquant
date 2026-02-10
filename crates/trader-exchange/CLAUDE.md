# trader-exchange

> 7개 거래소 Provider 구현. ExchangeApi trait 기반 추상화.

## 거래소 목록

| 거래소 | Provider | 시장 | 상태 |
|--------|----------|------|------|
| KIS (한국투자증권) | `kis.rs` | KR/US | ✅ 주문 가능 |
| Upbit | `upbit.rs` | KR crypto | ✅ 주문 가능 |
| Bithumb | `bithumb.rs` | KR crypto | ✅ 주문 가능 |
| LS증권 | `ls_sec.rs` | KR | ✅ 주문 가능 |
| DB금융투자 | `db_investment.rs` | KR | 🔲 주문 미구현 (P1-C) |
| Binance | `binance.rs` | Global crypto | ✅ 주문 가능 |
| Mock | `mock.rs` | 시뮬레이션 | ✅ Paper Trading |

## 디렉터리 구조

```
src/
├── connector/          # 거래소별 HTTP/WS 클라이언트
│   ├── kis/
│   ├── upbit/
│   ├── bithumb/
│   ├── ls_sec/
│   ├── db_investment/
│   └── binance/
├── provider/           # ExchangeProvider trait 구현
│   ├── kis.rs
│   ├── upbit.rs
│   └── ...
└── mock/               # Mock 거래소 (시뮬레이션)
    ├── mock_streaming.rs
    └── mock_order_engine.rs
```

## 규칙

- 새 거래소 추가 시 `ExchangeProvider` + `OrderExecutionProvider` trait 구현
- 모든 가격/수량은 `Decimal` (거래소 API의 f64 응답도 즉시 변환)
- API 키는 DB 암호화 저장 (AES-256-GCM), 하드코딩 금지
- WebSocket 스트림: `MarketStream` trait 구현

> 자동화: `/add-exchange` 스킬 사용 권장
