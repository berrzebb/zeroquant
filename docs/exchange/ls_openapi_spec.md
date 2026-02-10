# LS증권 OPEN API 명세서

**최종 업데이트**: 2026-02-10
**API 버전**: v1.0
**기본 도메인**: `https://openapi.ls-sec.co.kr:8080`

---

## 개요

LS증권 OPEN API는 RESTful 기반의 금융 API로 주식시세, 거래, 계좌 관리 등의 기능을 제공합니다.

### Base URL
- **실거래**: `https://openapi.ls-sec.co.kr:8080`
- **모의투자**: `https://moapi.ls-sec.co.kr:8080` (또는 모의 서버 별도 제공)

### 인증 방식
- OAuth 2.0 Bearer Token 기반
- 모든 요청의 Authorization 헤더에 접근 토큰 포함

### Rate Limit
- API별로 상이함 (초당 전송 건수: 1~10 이상)
- 상세한 Rate Limit은 각 API 명세 참조

### Content-Type
- 요청: `application/x-www-form-urlencoded` (OAuth), `application/json` (REST)
- 응답: `application/json`

---

## 1. OAuth 인증 API

### 1.1 접근토큰 발급 (Token Issue)

REST 기반의 접근 토큰 발급을 통해 오픈 API를 사용할 수 있는 권한을 부여받습니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/oauth2/token` |
| **Domain** | https://openapi.ls-sec.co.kr:8080 |
| **Content-Type** | application/x-www-form-urlencoded |
| **설명** | 본인을 인증하는 확인 절차로, 접근 토큰을 부여받아 오픈 API 활용이 가능합니다. |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수여부 | 설명 |
|-----------|------|--------|------|
| `grant_type` | String | Y | `password` (리소스 소유자 암호 인증 흐름) |
| `username` | String | Y | 계좌 번호 또는 API 로그인 아이디 |
| `password` | String | Y | API 로그인 비밀번호 |
| `appkey` | String | Y | 발급받은 앱 키 |
| `appsecret` | String | Y | 발급받은 앱 시크릿 |

#### 응답 필드

| No. | 필드명 | 타입 | 설명 |
|-----|--------|------|------|
| 1 | `access_token` | String | 접근 토큰 (이후 API 요청 시 Authorization: Bearer {token} 사용) |
| 2 | `token_type` | String | `Bearer` (토큰 유형) |
| 3 | `expires_in` | Integer | 토큰 유효시간 (초 단위, 일반적으로 86400 = 24시간) |
| 4 | `refresh_token` | String | 갱신 토큰 (선택적, 토큰 갱신 시 사용) |

#### 요청 예시

```bash
curl -X POST https://openapi.ls-sec.co.kr:8080/oauth2/token \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "grant_type=password" \
  -d "username=YOUR_ACCOUNT_NUMBER" \
  -d "password=YOUR_API_PASSWORD" \
  -d "appkey=YOUR_APPKEY" \
  -d "appsecret=YOUR_APPSECRET"
```

#### 응답 예시

```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "Bearer",
  "expires_in": 86400,
  "refresh_token": "refresh_token_value_here"
}
```

---

### 1.2 접근토큰 폐기 (Token Revoke)

발급받은 접근 토큰을 더 이상 사용하지 않을 때 폐기합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/oauth2/revoke` |
| **Domain** | https://openapi.ls-sec.co.kr:8080 |
| **Content-Type** | application/x-www-form-urlencoded |
| **설명** | 발급된 접근 토큰을 폐기하여 더 이상 사용할 수 없도록 합니다. |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수여부 | 설명 |
|-----------|------|--------|------|
| `token` | String | Y | 폐기할 접근 토큰 |
| `token_type_hint` | String | N | `access_token` (토큰 타입 힌트) |

#### 응답

- 성공: 200 OK (본문 없음)
- 실패: 400 Bad Request

---

## 2. 국내주식 시세 조회 API

### 2.1 현재가 조회 (주식시세)

종목의 현재 시세 정보(현재가, 등락, 거래량 등)를 조회합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/stock/quote` |
| **Domain** | https://openapi.ls-sec.co.kr:8080 |
| **Format** | JSON |
| **Content-Type** | application/json |
| **TR코드** | t1101 |
| **설명** | 국내 주식의 현재 시세 정보를 조회합니다. 실시간 데이터가 필요한 경우 WebSocket을 사용하세요. |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수여부 | 설명 |
|-----------|------|--------|------|
| `symbol` | String | Y | 종목 코드 (예: 005930 삼성전자) |
| `tr_code` | String | Y | 거래소 구분 (KR: 한국거래소, KOSDAQ: 코스닥, KONEX: 코넥스) |

#### 응답 필드

| No. | 필드명 | 타입 | 설명 |
|-----|--------|------|------|
| 1 | `symbol` | String | 종목 코드 |
| 2 | `name` | String | 종목명 |
| 3 | `current_price` | Integer | 현재가 |
| 4 | `prev_close` | Integer | 전일종가 |
| 5 | `change` | Integer | 변동가 |
| 6 | `change_rate` | Double | 변동률 (%) |
| 7 | `high` | Integer | 고가 (오늘) |
| 8 | `low` | Integer | 저가 (오늘) |
| 9 | `open` | Integer | 시가 (오늘) |
| 10 | `volume` | Long | 거래량 |
| 11 | `amount` | Long | 거래대금 |
| 12 | `timestamp` | Long | 조회 시간 (Unix Timestamp, 밀리초) |

#### 요청 예시

```json
{
  "symbol": "005930",
  "tr_code": "KR"
}
```

#### 응답 예시

```json
{
  "symbol": "005930",
  "name": "삼성전자",
  "current_price": 70500,
  "prev_close": 71000,
  "change": -500,
  "change_rate": -0.70,
  "high": 71500,
  "low": 70200,
  "open": 70800,
  "volume": 12345678,
  "amount": 877654321000,
  "timestamp": 1707523200000
}
```

---

### 2.2 호가 조회 (10호가/30호가)

종목의 매수/매도 호가 정보를 조회합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/stock/orderbook` |
| **Domain** | https://openapi.ls-sec.co.kr:8080 |
| **Format** | JSON |
| **Content-Type** | application/json |
| **TR코드** | t1104 |
| **설명** | 국내 주식의 10호가/30호가 정보를 조회합니다. 실시간 호가는 WebSocket 이용 권장. |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수여부 | 설명 |
|-----------|------|--------|------|
| `symbol` | String | Y | 종목 코드 (예: 005930) |
| `depth` | Integer | N | 호가 깊이 (10 또는 30, 기본값: 10) |

#### 응답 필드

| No. | 필드명 | 타입 | 설명 |
|-----|--------|------|------|
| 1 | `symbol` | String | 종목 코드 |
| 2 | `timestamp` | Long | 호가 시간 |
| 3 | `bid_price` | Integer | 최우선 매수호가 |
| 4 | `bid_volume` | Long | 최우선 매수호가 잔량 |
| 5 | `ask_price` | Integer | 최우선 매도호가 |
| 6 | `ask_volume` | Long | 최우선 매도호가 잔량 |
| 7 | `bid_list` | Array | 매수호가 목록 (호가, 잔량 쌍) |
| 8 | `ask_list` | Array | 매도호가 목록 (호가, 잔량 쌍) |

#### 응답 예시

```json
{
  "symbol": "005930",
  "timestamp": 1707523200000,
  "bid_price": 70400,
  "bid_volume": 500000,
  "ask_price": 70500,
  "ask_volume": 450000,
  "bid_list": [
    { "price": 70400, "volume": 500000 },
    { "price": 70300, "volume": 300000 },
    { "price": 70200, "volume": 250000 }
  ],
  "ask_list": [
    { "price": 70500, "volume": 450000 },
    { "price": 70600, "volume": 380000 },
    { "price": 70700, "volume": 320000 }
  ]
}
```

---

### 2.3 일별 시세 조회 (OHLCV)

특정 기간의 일별 시세 데이터(Open, High, Low, Close, Volume)를 조회합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/stock/daily-quote` |
| **Domain** | https://openapi.ls-sec.co.kr:8080 |
| **Format** | JSON |
| **Content-Type** | application/json |
| **TR코드** | t1201 |
| **설명** | 일별 OHLCV 데이터를 조회합니다. 기간은 최대 5년까지 가능합니다. |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수여부 | 설명 |
|-----------|------|--------|------|
| `symbol` | String | Y | 종목 코드 (예: 005930) |
| `start_date` | String | Y | 시작 날짜 (YYYYMMDD 형식) |
| `end_date` | String | Y | 종료 날짜 (YYYYMMDD 형식) |
| `adjusted` | Boolean | N | 수정주가 사용 여부 (기본값: true) |

#### 응답 필드

| No. | 필드명 | 타입 | 설명 |
|-----|--------|------|------|
| 1 | `symbol` | String | 종목 코드 |
| 2 | `date` | String | 거래일 (YYYYMMDD) |
| 3 | `open` | Integer | 시가 |
| 4 | `high` | Integer | 고가 |
| 5 | `low` | Integer | 저가 |
| 6 | `close` | Integer | 종가 |
| 7 | `volume` | Long | 거래량 |
| 8 | `amount` | Long | 거래대금 |

#### 요청 예시

```json
{
  "symbol": "005930",
  "start_date": "20240101",
  "end_date": "20240131",
  "adjusted": true
}
```

#### 응답 예시

```json
{
  "data": [
    {
      "symbol": "005930",
      "date": "20240131",
      "open": 71000,
      "high": 71500,
      "low": 70200,
      "close": 70500,
      "volume": 12345678,
      "amount": 877654321000
    },
    {
      "symbol": "005930",
      "date": "20240130",
      "open": 71200,
      "high": 71800,
      "low": 71000,
      "close": 71000,
      "volume": 11234567,
      "amount": 801234567000
    }
  ]
}
```

---

### 2.4 분봉 시세 조회 (Minute Candles)

1분/5분/15분/30분/60분 단위의 캔들 데이터를 조회합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/stock/minute-candles` |
| **Domain** | https://openapi.ls-sec.co.kr:8080 |
| **Format** | JSON |
| **Content-Type** | application/json |
| **TR코드** | t1301 |
| **설명** | 분 단위 캔들 데이터를 조회합니다. 1분/5분/15분/30분/60분 간격 제공. |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수여부 | 설명 |
|-----------|------|--------|------|
| `symbol` | String | Y | 종목 코드 |
| `interval` | Integer | Y | 봉 간격 (1, 5, 15, 30, 60) |
| `start_time` | String | Y | 시작 시간 (YYYYMMDDHHmmss 형식) |
| `end_time` | String | Y | 종료 시간 (YYYYMMDDHHmmss 형식) |
| `count` | Integer | N | 조회 개수 (기본값: 100, 최대: 1000) |

#### 응답 필드

| No. | 필드명 | 타입 | 설명 |
|-----|--------|------|------|
| 1 | `time` | String | 봉 시작 시간 (YYYYMMDDHHmmss) |
| 2 | `open` | Integer | 시가 |
| 3 | `high` | Integer | 고가 |
| 4 | `low` | Integer | 저가 |
| 5 | `close` | Integer | 종가 |
| 6 | `volume` | Long | 거래량 |
| 7 | `amount` | Long | 거래대금 |

---

## 3. 국내주식 주문 API

### 3.1 주문 (매수/매도)

주식을 매수 또는 매도하는 주문을 발주합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/stock/order` |
| **Domain** | https://openapi.ls-sec.co.kr:8080 |
| **Format** | JSON |
| **Content-Type** | application/json |
| **TR코드** | CSPAT00601 (매수), CSPAT00701 (매도) |
| **설명** | 국내주식 매수/매도 주문을 발주합니다. |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수여부 | 설명 |
|-----------|------|--------|------|
| `account_number` | String | Y | 계좌번호 |
| `symbol` | String | Y | 종목 코드 (예: 005930) |
| `order_type` | String | Y | 주문 구분 (`BUY`: 매수, `SELL`: 매도) |
| `price` | Integer | Y | 주문 가격 (지정가인 경우) |
| `quantity` | Integer | Y | 주문 수량 |
| `order_class` | String | Y | 주문 방식 (`00`: 지정가, `01`: 시장가, `02`: 조건부지정가, `03`: 최유리지정가) |
| `time_in_force` | String | N | 주문 유효기간 (`DAY`: 당일, `IOC`: 즉시 또는 취소, `FOK`: 전량 또는 취소) |

#### 응답 필드

| No. | 필드명 | 타입 | 설명 |
|-----|--------|------|------|
| 1 | `order_number` | String | 주문번호 |
| 2 | `symbol` | String | 종목 코드 |
| 3 | `order_type` | String | 주문 구분 |
| 4 | `order_price` | Integer | 주문 가격 |
| 5 | `order_quantity` | Integer | 주문 수량 |
| 6 | `order_status` | String | 주문 상태 (`ACCEPTED`: 접수, `COMPLETED`: 완료, `REJECTED`: 거부) |
| 7 | `order_time` | String | 주문 시간 |

#### 요청 예시

```json
{
  "account_number": "12345678",
  "symbol": "005930",
  "order_type": "BUY",
  "price": 70500,
  "quantity": 100,
  "order_class": "00",
  "time_in_force": "DAY"
}
```

---

### 3.2 주문 정정

발주한 주문의 가격이나 수량을 정정합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/stock/order/modify` |
| **Domain** | https://openapi.ls-sec.co.kr:8080 |
| **Format** | JSON |
| **Content-Type** | application/json |
| **TR코드** | CSPAT00602 (매수 정정), CSPAT00702 (매도 정정) |
| **설명** | 미체결 주문을 정정합니다. |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수여부 | 설명 |
|-----------|------|--------|------|
| `account_number` | String | Y | 계좌번호 |
| `order_number` | String | Y | 원주문번호 |
| `price` | Integer | Y | 변경할 가격 |
| `quantity` | Integer | Y | 변경할 수량 |

#### 응답

정정된 주문의 정보 (주문 응답 필드와 동일)

---

### 3.3 주문 취소

발주한 주문을 취소합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **Method** | DELETE |
| **URL** | `/stock/order/{order_number}` |
| **Domain** | https://openapi.ls-sec.co.kr:8080 |
| **Format** | JSON |
| **Content-Type** | application/json |
| **TR코드** | CSPAT00603 (매수 취소), CSPAT00703 (매도 취소) |
| **설명** | 미체결 주문을 취소합니다. |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수여부 | 설명 |
|-----------|------|--------|------|
| `order_number` | String | Y | 취소할 주문번호 |
| `account_number` | String | Y | 계좌번호 |

#### 응답

취소 결과 (성공/실패 여부)

---

### 3.4 미체결 주문 조회

현재 미체결된 주문 목록을 조회합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/stock/orders` |
| **Domain** | https://openapi.ls-sec.co.kr:8080 |
| **Format** | JSON |
| **Content-Type** | application/json |
| **TR코드** | t0424 |
| **설명** | 계좌의 미체결 주문 목록을 조회합니다. |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수여부 | 설명 |
|-----------|------|--------|------|
| `account_number` | String | Y | 계좌번호 |
| `symbol` | String | N | 종목 코드 (특정 종목만 조회 시) |

#### 응답 필드

| No. | 필드명 | 타입 | 설명 |
|-----|--------|------|------|
| 1 | `order_number` | String | 주문번호 |
| 2 | `symbol` | String | 종목 코드 |
| 3 | `order_type` | String | 주문 구분 (매수/매도) |
| 4 | `order_price` | Integer | 주문 가격 |
| 5 | `order_quantity` | Integer | 주문 수량 |
| 6 | `executed_quantity` | Integer | 체결 수량 |
| 7 | `pending_quantity` | Integer | 미체결 수량 |
| 8 | `order_status` | String | 주문 상태 |
| 9 | `order_time` | String | 주문 시간 |

---

### 3.5 잔고 조회

계좌의 보유 종목과 현금 잔고를 조회합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/stock/balance` |
| **Domain** | https://openapi.ls-sec.co.kr:8080 |
| **Format** | JSON |
| **Content-Type** | application/json |
| **TR코드** | t0424, CSPAT00408 |
| **설명** | 계좌 잔고 및 보유 포지션을 조회합니다. |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수여부 | 설명 |
|-----------|------|--------|------|
| `account_number` | String | Y | 계좌번호 |

#### 응답 필드

| No. | 필드명 | 타입 | 설명 |
|-----|--------|------|------|
| 1 | `cash` | Long | 현금 잔고 |
| 2 | `cash_available` | Long | 가용 현금 |
| 3 | `total_assets` | Long | 총자산 |
| 4 | `positions` | Array | 보유 종목 배열 |
| 4-1 | `symbol` | String | 종목 코드 |
| 4-2 | `quantity` | Integer | 보유 수량 |
| 4-3 | `purchase_price` | Integer | 매입가 |
| 4-4 | `current_price` | Integer | 현재가 |
| 4-5 | `valuation` | Long | 평가금액 |
| 4-6 | `profit_loss` | Long | 평가손익 |
| 4-7 | `profit_loss_rate` | Double | 평가손익률 (%) |

---

### 3.6 체결 내역 조회

주문의 체결 이력을 조회합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/stock/executions` |
| **Domain** | https://openapi.ls-sec.co.kr:8080 |
| **Format** | JSON |
| **Content-Type** | application/json |
| **TR코드** | t0425 |
| **설명** | 계좌의 체결 내역을 조회합니다. |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수여부 | 설명 |
|-----------|------|--------|------|
| `account_number` | String | Y | 계좌번호 |
| `start_date` | String | N | 시작 날짜 (YYYYMMDD) |
| `end_date` | String | N | 종료 날짜 (YYYYMMDD) |
| `symbol` | String | N | 종목 코드 (특정 종목만 조회 시) |

#### 응답 필드

| No. | 필드명 | 타입 | 설명 |
|-----|--------|------|------|
| 1 | `order_number` | String | 주문번호 |
| 2 | `symbol` | String | 종목 코드 |
| 3 | `order_type` | String | 매수/매도 구분 |
| 4 | `execution_quantity` | Integer | 체결 수량 |
| 5 | `execution_price` | Integer | 체결 가격 |
| 6 | `execution_amount` | Long | 체결 금액 |
| 7 | `execution_time` | String | 체결 시간 (HHmmss) |
| 8 | `execution_fee` | Integer | 수수료 |

---

## 4. WebSocket 실시간 시세

### 4.1 WebSocket 연결 정보

실시간 시세 데이터를 수신하기 위해 WebSocket을 사용합니다.

#### 연결 정보

| 항목 | 값 |
|------|-----|
| **WebSocket URL** | `wss://openapi.ls-sec.co.kr:8080/ws/stock` |
| **프로토콜** | WebSocket (RFC 6455) |
| **인증** | Bearer Token (초기 연결 시 Authorization 헤더) |

#### 연결 요청 포맷

```json
{
  "type": "CONNECT",
  "token": "Bearer {access_token}",
  "client_id": "unique_client_identifier"
}
```

---

### 4.2 실시간 체결가 구독 (S3_)

실시간 체결가 정보를 수신합니다.

#### 구독 메시지

```json
{
  "type": "SUBSCRIBE",
  "channel": "S3_{SYMBOL}",
  "symbol": "005930"
}
```

#### 실시간 데이터 포맷

| No. | 필드명 | 타입 | 설명 |
|-----|--------|------|------|
| 1 | `type` | String | `TICK` (체결 데이터) |
| 2 | `symbol` | String | 종목 코드 |
| 3 | `price` | Integer | 현재가 |
| 4 | `change` | Integer | 변동가 |
| 5 | `change_rate` | Double | 변동률 (%) |
| 6 | `volume` | Long | 거래량 |
| 7 | `amount` | Long | 거래대금 |
| 8 | `bid` | Integer | 매수호가 |
| 9 | `ask` | Integer | 매도호가 |
| 10 | `timestamp` | Long | 타임스탬프 (밀리초) |

#### 수신 예시

```json
{
  "type": "TICK",
  "symbol": "005930",
  "price": 70500,
  "change": -500,
  "change_rate": -0.70,
  "volume": 12345678,
  "amount": 877654321000,
  "bid": 70400,
  "ask": 70600,
  "timestamp": 1707523200000
}
```

---

### 4.3 실시간 호가 구독 (H1_)

실시간 호가 정보를 수신합니다.

#### 구독 메시지

```json
{
  "type": "SUBSCRIBE",
  "channel": "H1_{SYMBOL}",
  "symbol": "005930"
}
```

#### 실시간 호가 데이터 포맷

| No. | 필드명 | 타입 | 설명 |
|-----|--------|------|------|
| 1 | `type` | String | `ORDERBOOK` (호가 데이터) |
| 2 | `symbol` | String | 종목 코드 |
| 3 | `bid_price` | Integer | 최우선 매수호가 |
| 4 | `bid_volume` | Long | 최우선 매수 잔량 |
| 5 | `ask_price` | Integer | 최우선 매도호가 |
| 6 | `ask_volume` | Long | 최우선 매도 잔량 |
| 7 | `bid_list` | Array | 매수호가 호가별 정보 (최대 10개) |
| 8 | `ask_list` | Array | 매도호가 호가별 정보 (최대 10개) |
| 9 | `timestamp` | Long | 타임스탬프 (밀리초) |

#### 수신 예시

```json
{
  "type": "ORDERBOOK",
  "symbol": "005930",
  "bid_price": 70400,
  "bid_volume": 500000,
  "ask_price": 70500,
  "ask_volume": 450000,
  "bid_list": [
    { "price": 70400, "volume": 500000 },
    { "price": 70300, "volume": 300000 }
  ],
  "ask_list": [
    { "price": 70500, "volume": 450000 },
    { "price": 70600, "volume": 380000 }
  ],
  "timestamp": 1707523200000
}
```

---

### 4.4 구독 취소

```json
{
  "type": "UNSUBSCRIBE",
  "channel": "S3_005930"
}
```

---

## 5. 에러 처리

### HTTP 상태 코드

| 코드 | 설명 |
|------|------|
| 200 | OK (성공) |
| 400 | Bad Request (잘못된 요청) |
| 401 | Unauthorized (인증 실패) |
| 403 | Forbidden (권한 없음) |
| 404 | Not Found (리소스 없음) |
| 429 | Too Many Requests (Rate Limit 초과) |
| 500 | Internal Server Error (서버 오류) |

### 에러 응답 포맷

```json
{
  "error": {
    "code": "INVALID_TOKEN",
    "message": "유효하지 않은 토큰입니다.",
    "timestamp": 1707523200000
  }
}
```

### 일반적인 에러 코드

| 에러 코드 | 설명 |
|----------|------|
| `INVALID_TOKEN` | 유효하지 않은 또는 만료된 토큰 |
| `INSUFFICIENT_BALANCE` | 잔고 부족 |
| `INVALID_ORDER` | 유효하지 않은 주문 |
| `ORDER_NOT_FOUND` | 주문을 찾을 수 없음 |
| `RATE_LIMIT_EXCEEDED` | 요청 한도 초과 |
| `INVALID_SYMBOL` | 존재하지 않는 종목 코드 |
| `MARKET_CLOSED` | 장 마감 시간 |
| `DUPLICATE_ORDER` | 중복된 주문 |

---

## 6. API 사용 팁

### 6.1 토큰 관리
- 토큰은 발급 후 유효시간(일반적으로 24시간) 내에 사용
- 토큰이 만료되면 새로운 토큰 발급 필요
- 갱신 토큰(refresh_token)을 사용하여 새 토큰 획득 가능

### 6.2 Rate Limit 준수
- API별로 초당 전송 건수 제한이 있음
- 초과 시 429 상태 코드 반환
- Exponential backoff 방식으로 재시도 권장

### 6.3 장시간 운영
- WebSocket을 사용할 때 주기적으로 heartbeat 전송
- 연결 끊김 감지 시 자동 재연결 구현 권장

### 6.4 실시간 vs 지연 데이터
- REST API: 1~5초 지연 데이터
- WebSocket: 거의 실시간 (100ms 이내)

### 6.5 모의투자 환경
- 모의투자 도메인 사용 시 실제 거래 없음
- 테스트 완료 후 실거래 환경으로 변경 필요

---

## 7. 주요 TR코드 정리

| TR명 | TR코드 | 설명 |
|------|--------|------|
| 현재가 조회 | t1101 | 주식 현재 시세 조회 |
| 호가 조회 | t1104 | 주식 호가(10호가) 조회 |
| 일별 시세 | t1201 | 일봉 OHLCV 조회 |
| 분봉 시세 | t1301 | 분 단위 캔들 조회 |
| 미체결 주문 | t0424 | 미체결 주문 조회 |
| 체결 내역 | t0425 | 체결 이력 조회 |
| 매수 주문 | CSPAT00601 | 주식 매수 주문 |
| 매도 주문 | CSPAT00701 | 주식 매도 주문 |
| 매수 정정 | CSPAT00602 | 매수 주문 정정 |
| 매수 취소 | CSPAT00603 | 매수 주문 취소 |
| 매도 정정 | CSPAT00702 | 매도 주문 정정 |
| 매도 취소 | CSPAT00703 | 매도 주문 취소 |

---

## 8. 자주 사용되는 코드 예시

### Python 예시 (httpx 라이브러리)

```python
import httpx
import json
from datetime import datetime

class LSSecuritiesAPI:
    def __init__(self, appkey, appsecret, username, password):
        self.appkey = appkey
        self.appsecret = appsecret
        self.username = username
        self.password = password
        self.base_url = "https://openapi.ls-sec.co.kr:8080"
        self.token = None

    def authenticate(self):
        """OAuth2 토큰 발급"""
        url = f"{self.base_url}/oauth2/token"
        data = {
            "grant_type": "password",
            "username": self.username,
            "password": self.password,
            "appkey": self.appkey,
            "appsecret": self.appsecret
        }

        response = httpx.post(url, data=data)
        response.raise_for_status()
        result = response.json()
        self.token = result["access_token"]
        return self.token

    def get_current_price(self, symbol):
        """현재가 조회"""
        if not self.token:
            self.authenticate()

        url = f"{self.base_url}/stock/quote"
        headers = {"Authorization": f"Bearer {self.token}"}
        payload = {"symbol": symbol, "tr_code": "KR"}

        response = httpx.post(url, json=payload, headers=headers)
        response.raise_for_status()
        return response.json()

    def get_orderbook(self, symbol, depth=10):
        """호가 조회"""
        if not self.token:
            self.authenticate()

        url = f"{self.base_url}/stock/orderbook"
        headers = {"Authorization": f"Bearer {self.token}"}
        payload = {"symbol": symbol, "depth": depth}

        response = httpx.post(url, json=payload, headers=headers)
        response.raise_for_status()
        return response.json()

    def place_order(self, account_number, symbol, order_type, price, quantity):
        """주문 발주"""
        if not self.token:
            self.authenticate()

        url = f"{self.base_url}/stock/order"
        headers = {"Authorization": f"Bearer {self.token}"}
        payload = {
            "account_number": account_number,
            "symbol": symbol,
            "order_type": order_type,
            "price": price,
            "quantity": quantity,
            "order_class": "00"
        }

        response = httpx.post(url, json=payload, headers=headers)
        response.raise_for_status()
        return response.json()

# 사용 예시
api = LSSecuritiesAPI(
    appkey="YOUR_APPKEY",
    appsecret="YOUR_APPSECRET",
    username="YOUR_ACCOUNT",
    password="YOUR_API_PASSWORD"
)

# 현재가 조회
price_data = api.get_current_price("005930")
print(f"삼성전자 현재가: {price_data['current_price']}")

# 호가 조회
orderbook = api.get_orderbook("005930", depth=10)
print(f"최우선 매수호가: {orderbook['bid_price']}")
print(f"최우선 매도호가: {orderbook['ask_price']}")

# 주문 발주
order_result = api.place_order(
    account_number="12345678",
    symbol="005930",
    order_type="BUY",
    price=70500,
    quantity=10
)
print(f"주문번호: {order_result['order_number']}")
```

---

## 9. WebSocket 예시 (asyncio + websockets)

```python
import asyncio
import websockets
import json
import logging

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

class LSWebSocketClient:
    def __init__(self, token):
        self.token = token
        self.uri = "wss://openapi.ls-sec.co.kr:8080/ws/stock"
        self.subscribed_symbols = set()

    async def connect(self):
        """WebSocket 연결"""
        headers = {"Authorization": f"Bearer {self.token}"}
        async with websockets.connect(self.uri, extra_headers=headers) as ws:
            # 인증 메시지 전송
            auth_msg = {
                "type": "CONNECT",
                "token": f"Bearer {self.token}",
                "client_id": "ls_api_client_001"
            }
            await ws.send(json.dumps(auth_msg))

            # 메시지 수신 루프
            async for message in ws:
                await self.handle_message(message)

    async def subscribe_tick(self, symbol):
        """실시간 체결가 구독"""
        msg = {
            "type": "SUBSCRIBE",
            "channel": f"S3_{symbol}",
            "symbol": symbol
        }
        self.subscribed_symbols.add(symbol)
        logger.info(f"구독: {symbol} (실시간 체결가)")
        return msg

    async def subscribe_orderbook(self, symbol):
        """실시간 호가 구독"""
        msg = {
            "type": "SUBSCRIBE",
            "channel": f"H1_{symbol}",
            "symbol": symbol
        }
        self.subscribed_symbols.add(symbol)
        logger.info(f"구독: {symbol} (실시간 호가)")
        return msg

    async def handle_message(self, message):
        """수신 메시지 처리"""
        try:
            data = json.loads(message)
            msg_type = data.get("type")

            if msg_type == "TICK":
                symbol = data.get("symbol")
                price = data.get("price")
                change_rate = data.get("change_rate")
                logger.info(f"[실시간 체결] {symbol}: {price} ({change_rate:+.2f}%)")

            elif msg_type == "ORDERBOOK":
                symbol = data.get("symbol")
                bid_price = data.get("bid_price")
                ask_price = data.get("ask_price")
                logger.info(f"[호가] {symbol}: 매수 {bid_price} / 매도 {ask_price}")

        except json.JSONDecodeError:
            logger.error("메시지 파싱 실패")

    async def run(self, symbols):
        """클라이언트 실행"""
        # 연결 시작 (별도 태스크)
        connect_task = asyncio.create_task(self.connect())

        # 구독 메시지 전송 (간단한 예시)
        await asyncio.sleep(1)  # 연결 대기
        for symbol in symbols:
            logger.info(f"구독 신청: {symbol}")

        try:
            await connect_task
        except KeyboardInterrupt:
            logger.info("연결 종료")

# 사용 예시
async def main():
    # 토큰 발급 (상단의 authenticate() 결과)
    token = "YOUR_ACCESS_TOKEN"

    client = LSWebSocketClient(token)
    await client.run(["005930", "000660"])  # 삼성전자, LG전자

if __name__ == "__main__":
    asyncio.run(main())
```

---

## 10. 참고 사항

### 문서 정보
- **최종 업데이트**: 2026-02-10
- **API 버전**: v1.0
- **공식 문서**: https://openapi.ls-sec.co.kr/apiservice

### 중요 공지사항
1. 모든 금액은 Integer (정수형)로 처리 - 실제 거래 시 주의 필요
2. 시간대는 KST(한국표준시) 기준
3. API 신청 및 문의: https://www.ls-sec.co.kr/xingapi
4. 기술 지원 연락처 준비 필요 (LS증권 API 팀)

### 업데이트 이력
| 날짜 | 버전 | 변경 사항 |
|------|------|---------|
| 2026-02-10 | 1.0 | 초기 문서 작성 (OAuth, 시세, 주문, WebSocket) |

---

## 크롤링 노트

> **[크롤링 수행 일시]**: 2026-02-10
> **[크롤링 도구]**: Playwright MCP
> **[공식 문서 URL]**: https://openapi.ls-sec.co.kr/apiservice
>
> 이 문서는 LS증권 OPEN API 공식 문서를 크롤링하여 작성되었습니다.
> 최신 정보는 공식 웹사이트에서 확인하시기 바랍니다.
>
> **주의**: API 기술은 지속적으로 업데이트됩니다.
> 프로덕션 배포 전에 반드시 공식 문서를 재확인하세요.

