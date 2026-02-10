# 거래소 API 스펙 및 구현 현황

> **마지막 업데이트**: 2026-02-10
> **목적**: 각 거래소의 API 제공 기능과 현재 구현 수준을 비교하여 누락 기능 파악
> **참조**: `docs/todo_v1.5.md` (구현 계획)

---

## 거래소별 세부 API 명세

> 각 거래소의 필드 수준 상세 스펙은 아래 문서를 참조합니다.
> 이 문서는 **구현 현황과 갭 분석**에만 집중합니다.

| 거래소 | 스펙 문서 | 비고 |
|--------|----------|------|
| KIS (한국투자증권) | [`krx_openapi_spec.md`](krx_openapi_spec.md) | 기준 구현 (가장 성숙) |
| Upbit (업비트) | [`upbit_openapi_spec.md`](upbit_openapi_spec.md) | 2026-02-10 크롤링 |
| LS증권 | [`ls_openapi_spec.md`](ls_openapi_spec.md) | 2026-02-10 크롤링 |
| DB증권 | [`db_openapi_spec.md`](db_openapi_spec.md) | 2026-02-10 크롤링, 일부 TR코드 추정 |
| Bithumb (빗썸) | [`bithumb_openapi_spec.md`](bithumb_openapi_spec.md) | 2026-02-10 크롤링 |

---

## 목차

1. [Upbit (업비트)](#1-upbit-업비트)
2. [LS증권 (LS Securities)](#2-ls증권-ls-securities)
3. [DB증권 (DB금융투자)](#3-db증권-db금융투자)
4. [Bithumb (빗썸)](#4-bithumb-빗썸)
5. [거래소 간 비교 매트릭스](#5-거래소-간-비교-매트릭스)

---

## 1. Upbit (업비트)

### 1.1 기본 정보

| 항목 | 값 |
|------|-----|
| REST Base URL | `https://api.upbit.com/v1` |
| WebSocket (Public) | `wss://api.upbit.com/websocket/v1` |
| WebSocket (Private) | `wss://api.upbit.com/websocket/v1/private` |
| 인증 | JWT (HS256), `Authorization: Bearer {token}`, SHA512 query_hash |
| Rate Limit (주문) | 초당 8회 (계정 단위) |
| Rate Limit (기본) | 초당 30회 (계정), 초당 10회 (IP, Quotation) |
| WS Idle Timeout | 120초 (PING/PONG 유지) |
| 구현 파일 | `trader-exchange/src/connector/upbit/` |
| 상세 스펙 | [`upbit_openapi_spec.md`](upbit_openapi_spec.md) |

### 1.2 REST API 구현 현황

| API | 메서드 | 경로 | 구현 상태 | 비고 |
|-----|--------|------|----------|------|
| 전체 계좌 조회 | `GET` | `/accounts` | ✅ | `fetch_account()`, `fetch_positions()` |
| 미체결 주문 | `GET` | `/orders/open` | ✅ | `fetch_pending_orders()` |
| 현재가 조회 | `GET` | `/ticker` | ✅ | `get_quote()`, `get_quotes()` |
| **주문 생성** | `POST` | `/orders` | ❌ | **Critical** — smp_type, time_in_force(ioc/fok/post_only) 지원 |
| **주문 취소** | `DELETE` | `/order` | ❌ | **Critical** — uuid 또는 identifier로 취소 |
| 주문 조회 | `GET` | `/order` | ❌ | High — 체결 목록(trades[]) 포함 |
| 종료 주문 조회 | `GET` | `/orders/closed` | ❌ | High — 체결/취소 이력 (최대 7일) |
| 주문 가능 정보 | `GET` | `/orders/chance` | ❌ | Medium — 수수료, 주문유형, 잔고 조회 |
| 마켓 목록 | `GET` | `/market/all` | ❌ | Low |
| 캔들 조회 | `GET` | `/candles/{unit}` | ❌ | Low — 초/분/일/주/월 지원 |
| 호가 조회 | `GET` | `/orderbook` | ❌ | Low |
| 체결 틱 | `GET` | `/trades/ticks` | ❌ | Low |

> 주문 정정 API 없음 → cancel + re-place 패턴 필수

### 1.3 WebSocket 구현 현황

| 타입 | 설명 | 구현 상태 | 비고 |
|------|------|----------|------|
| `ticker` | 현재가 (33 필드) | ✅ | `parse_ticker()` |
| `orderbook` | 호가 (최대 30단계) | ⚠️ 파싱 미완 | enum에 `Orderbook(Value)` 존재 |
| `trade` | 체결 (18 필드) | ❌ | 구독도 안 됨 |
| `candle.{unit}` | 실시간 캔들 (12 필드) | ❌ | 초/분봉 |
| `myOrder` (Private) | 내 주문/체결 (29 필드) | ❌ | JWT 인증 필수 |
| `myAsset` (Private) | 내 자산 변동 (6 필드) | ❌ | JWT 인증, codes 미지원 |

---

## 2. LS증권 (LS Securities)

### 2.1 기본 정보

| 항목 | 값 |
|------|-----|
| REST Base URL | `https://openapi.ls-sec.co.kr:8080` |
| WebSocket URL | `wss://openapi.ls-sec.co.kr:8080/ws/stock` |
| 인증 | OAuth2 (password grant), `Bearer {token}` |
| 토큰 유효기간 | 24시간 |
| 구현 파일 | `trader-exchange/src/connector/ls_sec/` |
| 상세 스펙 | [`ls_openapi_spec.md`](ls_openapi_spec.md) |

> **WebSocket URL 수정**: 기존 `9443/websocket` → 크롤링 확인 결과 `8080/ws/stock`

### 2.2 REST API 구현 현황

| TR코드 | 용도 | 경로 | 구현 상태 | 비고 |
|--------|------|------|----------|------|
| OAuth2 Token | 토큰 발급 | `/oauth2/token` | ✅ + 캐싱 | password grant |
| OAuth2 Revoke | 토큰 폐기 | `/oauth2/revoke` | ❌ | Low |
| `t1101` | KR 현재가 | `/stock/quote` | ⚠️ 3필드만 | high/low/open/volume 추가 필요 |
| `t1104` | KR 호가 | `/stock/orderbook` | ❌ | Medium — 10/30호가 |
| `t1201` | KR 일별 시세 | `/stock/daily-quote` | ❌ | Low — OHLCV |
| `t1301` | KR 분봉 시세 | `/stock/minute-candles` | ❌ | Low — 1/5/15/30/60분 |
| `CSPAQ12200` | KR 잔고 | - | ✅ | `fetch_account()` |
| `CSPAQ12300` | KR 보유종목 | - | ✅ | `fetch_kr_positions()` |
| `COSOQ00201` | US 보유종목 | - | ✅ | `fetch_us_positions()` |
| `t0424` | KR 미체결 | `/stock/orders` | ✅ | `fetch_pending_orders()` |
| `t0425` | KR 체결 내역 | `/stock/executions` | ❌ | High |
| **`CSPAT00601`** | **KR 매수 주문** | `/stock/order` | ❌ | **Critical** |
| **`CSPAT00701`** | **KR 매도 주문** | `/stock/order` | ❌ | **Critical** |
| `CSPAT00602/00702` | KR 주문 정정 | `/stock/order/modify` | ❌ | **Critical** |
| `CSPAT00603/00703` | KR 주문 취소 | `/stock/order/{order_number}` | ❌ | **Critical** |
| US 주문/조회 TR | 해외 주문/시세 | - | ❌ | Medium~High |

### 2.3 WebSocket 구현 현황

| TR코드 | 타입 | 설명 | 구현 상태 | 비고 |
|--------|------|------|----------|------|
| `S3_` | 실시간 | 국내 체결가 | ✅ 파싱 | 일부 필드 ZERO |
| `H1_` | 실시간 | 국내 호가 | ⚠️ 구독 O, 파싱 X | 10호가 |
| `HDF` | 실시간 | 해외 체결가 | ✅ 파싱 | 일부 필드 ZERO |
| 해외 호가 | 실시간 | 해외 호가 | ❌ | - |

> **TR코드 수정**: 기존 문서에서 H1_=체결/H2_=호가로 기록되었으나,
> 크롤링 결과 **S3_=체결, H1_=호가**로 확인됨. 구현 코드와 대조 필요.

---

## 3. DB증권 (DB금융투자)

### 3.1 기본 정보

| 항목 | 값 |
|------|-----|
| REST Base URL | `https://openapi.dbsec.co.kr:8443` |
| WebSocket (운영) | `wss://openapi.dbsec.co.kr:7070/websocket` |
| WebSocket (모의투자) | `wss://openapi.dbsec.co.kr:17070/websocket` |
| WS Format | JSON (`application/json;charset=utf-8`) |
| 인증 | OAuth2 (client_credentials), `Bearer {token}` |
| 토큰 유효기간 | 24시간 |
| 구현 파일 | `trader-exchange/src/connector/db_investment/` |
| 상세 스펙 | [`db_openapi_spec.md`](db_openapi_spec.md) |

### 3.2 REST API 구현 현황

| API/TR코드 | 경로 | 구현 상태 | 비고 |
|-----------|------|----------|------|
| OAuth2 Token | `/oauth2/token` | ✅ + 캐싱 | client_credentials |
| KR 현재가 | `/api/v1/quote/kr-stock/inquiry/price` | ⚠️ 3필드만 | Prpr/UpDnPrc/UpDnRate만. OpenPrice/HighPrice/LowPrice 추가 필요 |
| KR 호가 | `/api/v1/quote/kr-stock/inquiry/orderbook` | ❌ | Medium |
| KR OHLCV | `/api/v1/quote/kr-stock/inquiry/ohlcv` | ❌ | Low |
| KR 체결내역 | `/api/v1/quote/kr-stock/inquiry/daily-chart` | ❌ | High |
| KR 잔고 (CSPAQ03420) | `/api/v1/trading/kr-stock/inquiry/balance` | ✅ | `fetch_account()`, `fetch_kr_positions()` |
| US 잔고 | `/api/v1/trading/overseas-stock/inquiry/balance-margin` | ✅ | `fetch_us_positions()` |
| KR 미체결 | `/api/v1/trading/kr-stock/inquiry/open-orders` | ✅ | TR: CSPAQ36610 (추정) |
| **KR 주문 (CSPAT00600)** | `/api/v1/trading/kr-stock/order` | ❌ | **Critical** |
| **KR 주문 정정** | `/api/v1/trading/kr-stock/order-modify` | ❌ | **Critical** |
| **KR 주문 취소** | `/api/v1/trading/kr-stock/order-cancel` | ❌ | **Critical** |
| US 현재가 | `/api/v1/quote/us-stock/inquiry/price` | ❌ | Medium |
| US 주문 | (확인 필요) | ❌ | High |

### 3.3 WebSocket 구현 현황

현재 **완전 Placeholder** — `connect()`가 에러만 전송하고 종료.

| TR코드 | 설명 | 구현 상태 |
|--------|------|----------|
| `V60` | 실시간 체결 (Prpr, VolumeQty, ChangeRate 등) | ❌ |
| `V20` | 실시간 호가 (BidPrice/AskPrice 1~5) | ❌ |
| `V22` | 주문체결 통보 (OrdNo, ExecQty, ExecPrc) | ❌ |

> 구독: `{"tr_cd": "V60", "tr_key": "005930"}`
> 해제: `{"tr_cd": "V60", "tr_key": "005930", "unsubscribe": true}`

---

## 4. Bithumb (빗썸)

### 4.1 기본 정보

| 항목 | 값 |
|------|-----|
| REST Base URL | `https://api.bithumb.com` |
| WebSocket (Public) | `wss://pubwss.bithumb.com/pubws` |
| WebSocket (Private) | `wss://ws.bithumb.com/ws` |
| 인증 (REST) | HMAC-SHA512 (API-Key / API-Sign / API-Nonce 헤더) |
| Rate Limit | 900 req/min (15 req/sec), Public/Private 동일 |
| WS Max Connections | 10개/IP |
| WS Ping Interval | 30초 |
| 구현 파일 | `trader-exchange/src/connector/bithumb/` |
| 상세 스펙 | [`bithumb_openapi_spec.md`](bithumb_openapi_spec.md) |

> **URL 수정**: 기존 `wss://api.bithumb.com/pub/ws` → Public/Private 분리 확인

### 4.2 REST API 구현 현황

| API | 메서드 | 경로 | 구현 상태 | 비고 |
|-----|--------|------|----------|------|
| 마켓 코드 조회 | `GET` | `/v1/market/all` | ❌ | Low |
| 현재가 조회 | `GET` | `/v1/ticker` | ✅ | `get_quote()`, `get_quotes()` |
| 호가 조회 | `GET` | `/v1/orderbook` | ❌ | Medium |
| 체결 내역 (Public) | `GET` | `/v1/trades` | ❌ | Low |
| 캔들 조회 | `GET` | `/v1/candles/{timeframe}` | ❌ | Low — 1m/5m/15m/30m/1h/4h/일/주/월 |
| 계좌 조회 | `GET` | `/v1/accounts` | ✅ | `fetch_account()` |
| 보유 자산 | `GET` | `/v1/balances` | ✅ | `fetch_positions()` |
| 미체결 주문 | `GET` | `/v1/orders/open` | ✅ | `fetch_pending_orders()` |
| **주문 생성** | `POST` | `/v1/orders` | ❌ | **Critical** — limit/price/market, time_in_force(ioc/fok/gtc) |
| **주문 취소** | `DELETE` | `/v1/orders/{uuid}` | ❌ | **Critical** |
| 주문 조회 | `GET` | `/v1/orders/{uuid}` | ❌ | High |
| 거래 내역 (Private) | `GET` | `/v1/trades` | ❌ | High |

### 4.3 WebSocket 구현 현황

| 타입 | Endpoint | 설명 | 구현 상태 |
|------|----------|------|----------|
| `ticker` | Public | 현재가 (16 필드) | ✅ |
| `trade` | Public | 체결 (9 필드) | ❌ |
| `orderbook` | Public | 호가 | ❌ |
| `balance` | Private | 내 자산 변동 | ❌ |
| `order` | Private | 내 주문 실시간 | ❌ |

> 구독: `{"type": "ticker", "symbols": ["KRW-BTC"]}`
> Private WS 인증: `{"type": "authorization", "api_key": "...", "api_secret": "...", "api_nonce": "..."}`

---

## 5. 거래소 간 비교 매트릭스

### 5.1 기능별 구현율

| 기능 | KIS | Binance | Upbit | LS증권 | DB증권 | Bithumb |
|------|:---:|:-------:|:-----:|:------:|:------:|:-------:|
| **REST 주문** | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| **REST 취소** | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| **REST 정정** | ✅ | ❌ | ❌¹ | ❌ | ❌ | ❌ |
| REST 계좌 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| REST 포지션 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| REST 미체결 | ✅ | ✅ | ✅ | ✅ KR | ✅ KR | ✅ |
| REST 체결내역 | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| REST 현재가 (완전) | ✅ | ✅ | ✅ | ⚠️ | ⚠️ | ✅ |
| **WS Ticker** | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |
| **WS OrderBook** | ✅ | ✅ | ⚠️ | ⚠️ | ❌ | ❌ |
| **WS Trade** | ✅ | ✅ | ❌ | ✅ | ❌ | ❌ |
| WS 재연결 | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |
| WS Connected/DC | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |

> ¹ Upbit: 주문 정정 API 없음 (cancel + re-place 패턴)

### 5.2 거래소별 종합 점수 (API 기준)

| 거래소 | 조회 | 주문 | WS 실시간 | 종합 | 등급 |
|--------|:----:|:----:|:---------:|:----:|:----:|
| **KIS** | 100% | 100% | 95% | **98%** | A |
| **Binance** | 100% | 90% | 80% | **90%** | A- |
| **Upbit** | 90% | 0% | 40% | **43%** | C |
| **Bithumb** | 90% | 0% | 35% | **42%** | C |
| **LS증권** | 75% | 0% | 45% | **40%** | C |
| **DB증권** | 80% | 0% | 5% | **28%** | D |

> 점수 산출: 조회(30%) + 주문(40%) + WS(30%). 주문 비중이 높은 이유는 실거래 필수 기능이기 때문.

---

## 부록: 공통 타입 참조

### QuoteData (trader-core)

```rust
pub struct QuoteData {
    pub symbol: String,
    pub current_price: Decimal,
    pub price_change: Decimal,
    pub change_percent: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub open: Decimal,
    pub prev_close: Decimal,
    pub volume: Decimal,
    pub trading_value: Decimal,
    pub timestamp: DateTime<Utc>,
}
```

### MarketEvent (trader-exchange)

```rust
pub enum MarketEvent {
    Ticker(Ticker),
    OrderBook(OrderBook),
    Trade(TradeTick),
    Kline(Kline),
    Connected,
    Disconnected,
    Error(String),
}
```
