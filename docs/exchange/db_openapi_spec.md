# DB금융투자(DB증권) OPEN API 명세서

> 최종 업데이트: 2026-02-10
> 회사명 변경: 2025-04-01 부터 "DB금융투자" → "DB증권"

---

## 개요

DB금융투자(현 DB증권)는 RESTful HTTP API와 WebSocket을 통한 실시간 데이터 스트림을 제공하는 Open API 플랫폼입니다.

### 기본 정보

| 항목 | 값 |
|------|-----|
| **Base URL (API)** | `https://openapi.dbsec.co.kr:8443` |
| **WebSocket URL (실운영)** | `wss://openapi.dbsec.co.kr:7070/websocket` |
| **WebSocket URL (모의투자)** | `wss://openapi.dbsec.co.kr:17070/websocket` |
| **공식 포털** | https://openapi.db-fi.com |
| **포털 (API 카테고리)** | https://openapi.dbsec.co.kr/about-openapi |
| **API 포맷** | JSON |
| **인증 방식** | OAuth2 (Bearer Token) |
| **접근토큰 유효기간** | 24시간 (개인/법인 동일) |

---

## 1. 인증 API

### 1.1 접근토큰 발급 (OAuth2)

국내주식 및 해외주식 거래를 위해 먼저 접근토큰(Access Token)을 발급받아야 합니다.

#### 요청

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/oauth2/token` |
| **전체 URL** | `https://openapi.dbsec.co.kr:8443/oauth2/token` |
| **Content-Type** | `application/json` |
| **설명** | OAuth2 클라이언트 자격증명 방식으로 접근토큰 발급 |

#### 입력 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| grant_type | String | Y | 고정값: "client_credentials" |
| appkey | String | Y | 발급받은 App Key |
| appsecret | String | Y | 발급받은 App Secret |

#### 요청 예시 (Python)

```python
import requests
import json

url = "https://openapi.dbsec.co.kr:8443/oauth2/token"

payload = {
    "grant_type": "client_credentials",
    "appkey": "YOUR_APP_KEY",
    "appsecret": "YOUR_APP_SECRET"
}

headers = {
    "Content-Type": "application/json"
}

response = requests.post(url, json=payload, headers=headers, verify=False)
token_data = response.json()
access_token = token_data['access_token']
```

#### 응답 (성공)

| 필드명 | 타입 | 설명 |
|--------|------|------|
| access_token | String | 사용할 접근 토큰 |
| token_type | String | 토큰 유형 (Bearer) |
| expires_in | Integer | 토큰 유효시간 (초) |

```json
{
  "access_token": "eyJhbGc...",
  "token_type": "Bearer",
  "expires_in": 86400
}
```

### 1.2 접근토큰 갱신

접근토큰 만료 시 동일한 appkey/appsecret으로 재발급받습니다.

#### 요청

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/oauth2/token` |
| **설명** | 토큰 갱신 (동일한 엔드포인트 사용) |

---

## 2. 국내주식 시세 조회 API

### 2.1 현재가 조회

특정 종목의 현재 시세 정보를 조회합니다.

#### 요청

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/api/v1/quote/kr-stock/inquiry/price` |
| **TR 코드** | `CSPAQ21600` (추정) |
| **설명** | 국내주식 현재가 조회 |

#### 입력 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| InputCondMrktDivCode | String | Y | 시장 구분 (0: 전체, 1: 코스피, 2: 코스닥) |
| InputIscd1 | String | Y | 종목코드 (예: 005930) |

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | Prpr | Decimal | 현재가 |
| 2 | YdayClpr | Decimal | 전일 종가 |
| 3 | UpDnPrc | Decimal | 상승/하락가 |
| 4 | UpDnRate | Decimal | 상승/하락률 (%) |
| 5 | TrdvHrs | String | 누적 거래량 |
| 6 | AccTrdvol | Integer | 누적 거래량 |
| 7 | AccTrdval | Decimal | 누적 거래대금 |
| 8 | OpenPrice | Decimal | 시가 |
| 9 | HighPrice | Decimal | 고가 |
| 10 | LowPrice | Decimal | 저가 |

### 2.2 호가 조회

특정 종목의 실시간 호가 정보를 조회합니다.

#### 요청

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/api/v1/quote/kr-stock/inquiry/orderbook` |
| **TR 코드** | `CSPAQ00300` (추정) |
| **설명** | 국내주식 호가(Order Book) 조회 |

#### 입력 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| InputCondMrktDivCode | String | Y | 시장 구분 |
| InputIscd1 | String | Y | 종목코드 |

#### 응답 필드 (매도/매수 호가)

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | BidPrice1 | Decimal | 매수1호가 |
| 2 | BidQty1 | Integer | 매수1호가 수량 |
| 3 | AskPrice1 | Decimal | 매도1호가 |
| 4 | AskQty1 | Integer | 매도1호가 수량 |
| 5-12 | BidPrice/Qty 2-5 | Decimal/Int | 매수 2~5호가 |
| 13-20 | AskPrice/Qty 2-5 | Decimal/Int | 매도 2~5호가 |

### 2.3 체결 내역 조회

특정 기간의 체결 내역을 조회합니다.

#### 요청

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/api/v1/quote/kr-stock/inquiry/daily-chart` |
| **TR 코드** | `CSPAQ35600` (추정) |
| **설명** | 국내주식 일별 체결 내역 조회 |

#### 입력 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| InputCondMrktDivCode | String | Y | 시장 구분 |
| InputIscd1 | String | Y | 종목코드 |
| InputDate | String | Y | 조회 기준일 (YYYYMMDD) |
| InputPeriodDivCode | String | Y | 기간 (1: 일, 2: 주, 3: 월) |

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | Date | String | 거래일자 (YYYYMMDD) |
| 2 | Open | Decimal | 시가 |
| 3 | High | Decimal | 고가 |
| 4 | Low | Decimal | 저가 |
| 5 | Close | Decimal | 종가 |
| 6 | Volume | Integer | 거래량 |
| 7 | Amount | Decimal | 거래대금 |

### 2.4 기간별 시세 (OHLCV)

#### 요청

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/api/v1/quote/kr-stock/inquiry/ohlcv` |
| **설명** | OHLCV(Open/High/Low/Close/Volume) 데이터 조회 |

#### 입력 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| InputIscd | String | Y | 종목코드 |
| InputStartDate | String | Y | 조회 시작일 (YYYYMMDD) |
| InputEndDate | String | Y | 조회 종료일 (YYYYMMDD) |
| InputPeriodDivCode | String | Y | 기간 (1: 일, 2: 주, 3: 월, 4: 연) |

---

## 3. 국내주식 주문 API

### 3.1 주문 (매수/매도)

새로운 주문을 접수합니다.

#### 요청

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/api/v1/trading/kr-stock/order` |
| **TR 코드** | `CSPAT00600` |
| **설명** | 국내주식 주문 |
| **인증** | Bearer Token 필수 |

#### 입력 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| InputCondMrktDivCode | String | Y | 시장 (1: 코스피, 2: 코스닥) |
| InputIscd | String | Y | 종목코드 (예: 005930) |
| InputQty | Integer | Y | 주문 수량 |
| InputPrc | Decimal | Y | 주문 가격 |
| InputDvsnCode | String | Y | 매매구분 (1: 매수, 2: 매도) |
| InputOrdPtnCode | String | Y | 호가 유형 (00: 지정가, 01: 시장가) |
| InputBnsTpCode | String | N | 거래구분 (01: 일반, 02: 신용) |

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | OrdNo | String | 주문번호 |
| 2 | OrdSttCd | String | 주문상태코드 |
| 3 | TrdMktNo | String | 거래시장번호 |

### 3.2 주문 정정

이미 접수된 미체결 주문을 정정합니다.

#### 요청

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/api/v1/trading/kr-stock/order-modify` |
| **TR 코드** | `CSPAT00601` (추정) |
| **설명** | 국내주식 주문 정정 |

#### 입력 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| InputOrdNo | String | Y | 정정할 주문번호 |
| InputOrdQty | Integer | Y | 정정할 수량 |
| InputOrdPrc | Decimal | Y | 정정할 가격 |

### 3.3 주문 취소

이미 접수된 미체결 주문을 취소합니다.

#### 요청

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/api/v1/trading/kr-stock/order-cancel` |
| **TR 코드** | `CSPAT00602` (추정) |
| **설명** | 국내주식 주문 취소 |

#### 입력 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| InputOrdNo | String | Y | 취소할 주문번호 |

### 3.4 미체결 조회

현재 미체결된 주문 목록을 조회합니다.

#### 요청

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/api/v1/trading/kr-stock/inquiry/open-orders` |
| **TR 코드** | `CSPAQ36610` (추정) |
| **설명** | 국내주식 미체결 조회 |

#### 입력 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| InputCondMrktDivCode | String | Y | 시장 구분 |

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | OrdNo | String | 주문번호 |
| 2 | OrdDt | String | 주문일자 |
| 3 | OrdTm | String | 주문시각 |
| 4 | IscdNm | String | 종목명 |
| 5 | Iscd | String | 종목코드 |
| 6 | OrdQty | Integer | 주문수량 |
| 7 | OrdPrc | Decimal | 주문가격 |
| 8 | ExecQty | Integer | 체결수량 |
| 9 | OrdSttCd | String | 주문상태코드 |
| 10 | DvsnCode | String | 매매구분 (1: 매수, 2: 매도) |

### 3.5 잔고 조회

현재 보유한 주식 잔고 및 포지션 정보를 조회합니다.

#### 요청

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/api/v1/trading/kr-stock/inquiry/balance` |
| **TR 코드** | `CSPAQ03420` |
| **설명** | 국내주식 잔고 조회 |
| **인증** | Bearer Token 필수 |

#### 입력 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| 입력값 | - | - | 입력값 없음 (Body: empty) |

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | IsuNo | String | 종목코드 |
| 2 | IsuNm | String | 종목명 |
| 3 | BalQty | Integer | 잔고수량 |
| 4 | PurchPrc | Decimal | 평균 매입가 |
| 5 | NowPrc | Decimal | 현재가 |
| 6 | EvalAmt | Decimal | 평가금액 |
| 7 | EvalPnlAmt | Decimal | 평가손익금액 |
| 8 | EvalPnlRate | Decimal | 평가손익률 (%) |

---

## 4. WebSocket 실시간 데이터 API

### 4.1 WebSocket 연결

WebSocket을 통해 실시간 시세 데이터를 수신합니다.

#### 연결 정보

| 항목 | 값 |
|------|-----|
| **실운영 URL** | `wss://openapi.dbsec.co.kr:7070/websocket` |
| **모의투자 URL** | `wss://openapi.dbsec.co.kr:17070/websocket` |
| **프로토콜** | WebSocket (RFC 6455) |
| **메시지 포맷** | JSON |
| **인증** | ACCESS_TOKEN (Header 포함) |

### 4.2 WebSocket 연결 예시 (Python)

```python
import websocket
import json

# WebSocket 연결
def on_open(ws):
    # 접근토큰이 있는 경우 header에 포함
    auth_header = {
        "Authorization": f"Bearer {access_token}",
        "Content-Type": "application/json"
    }
    print("WebSocket 연결 성공")

def on_message(ws, message):
    data = json.loads(message)
    print(f"수신 데이터: {data}")

def on_error(ws, error):
    print(f"에러: {error}")

def on_close(ws, close_status_code, close_msg):
    print("WebSocket 연결 종료")

ws_url = "wss://openapi.dbsec.co.kr:7070/websocket"
ws = websocket.WebSocketApp(
    ws_url,
    on_open=on_open,
    on_message=on_message,
    on_error=on_error,
    on_close=on_close
)

ws.run_forever()
```

### 4.3 실시간 체결 구독

#### 요청 메시지 포맷

| 필드명 | 타입 | 필수 | 설명 |
|--------|------|------|------|
| tr_cd | String | Y | 트랜잭션 코드 (예: V60) |
| tr_key | String | Y | 조회 대상 종목코드 (예: 005930) |

#### 요청 예시

```json
{
  "tr_cd": "V60",
  "tr_key": "005930"
}
```

#### 응답 예시 (실시간 체결)

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | Prpr | Decimal | 현재가 |
| 2 | VolumeQty | Integer | 거래량 |
| 3 | VolumeValue | Decimal | 거래대금 |
| 4 | Time | String | 시각 (HHMMSS) |
| 5 | ChangeRate | Decimal | 변화율 (%) |

```json
{
  "tr_cd": "V60",
  "tr_key": "005930",
  "Prpr": 70500,
  "VolumeQty": 1500,
  "VolumeValue": 105750000,
  "Time": "143025",
  "ChangeRate": 1.23
}
```

### 4.4 실시간 호가 구독

#### 요청 예시

| 필드명 | 값 | 설명 |
|--------|-----|------|
| tr_cd | V20 | 호가 데이터 코드 |
| tr_key | 005930 | 종목코드 |

```json
{
  "tr_cd": "V20",
  "tr_key": "005930"
}
```

#### 응답 필드 (호가 정보)

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | BidPrice1 | Decimal | 매수1호가 |
| 2 | BidQty1 | Integer | 매수1호가 수량 |
| 3 | AskPrice1 | Decimal | 매도1호가 |
| 4 | AskQty1 | Integer | 매도1호가 수량 |
| 5-20 | Bid/Ask Price/Qty 2-5 | Decimal/Int | 매수/매도 2~5호가 및 수량 |

### 4.5 실시간 주문 체결 통보 (체결 알림)

주문이 체결되었을 때 실시간으로 체결 내용을 수신합니다.

#### 요청 코드

| tr_cd | 설명 |
|-------|------|
| V22 | 주문 체결 실시간 통보 |

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | OrdNo | String | 주문번호 |
| 2 | ExecNo | String | 체결번호 |
| 3 | ExecQty | Integer | 체결수량 |
| 4 | ExecPrc | Decimal | 체결가 |
| 5 | IscdNm | String | 종목명 |
| 6 | ExecTime | String | 체결시각 |

### 4.6 WebSocket 구독 취소

```json
{
  "tr_cd": "V60",
  "tr_key": "005930",
  "unsubscribe": true
}
```

---

## 5. 해외주식 API

### 5.1 해외주식 현재가 조회

#### 요청

| 항목 | 값 |
|------|-----|
| **Method** | POST |
| **URL** | `/api/v1/quote/us-stock/inquiry/price` |
| **설명** | 해외(미국) 주식 현재가 조회 |

#### 입력 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| InputTicker | String | Y | 종목코드 (예: AAPL) |
| InputExchCd | String | Y | 거래소코드 (NAS: 나스닥, NYS: NYSE) |

### 5.2 해외주식 실시간 체결 (WebSocket)

#### 요청 코드

| tr_cd | 설명 |
|-------|------|
| V60 | 해외주식 실시간 체결 (V60) |

#### 요청 예시

```json
{
  "tr_cd": "V60",
  "tr_key": "AAPL"
}
```

---

## 6. API 공통 사항

### 6.1 요청 헤더

모든 API 요청에 포함해야 할 헤더:

```
Authorization: Bearer {ACCESS_TOKEN}
Content-Type: application/json; charset=utf-8
```

### 6.2 응답 상태 코드

| 상태 코드 | 설명 |
|----------|------|
| 200 | 요청 성공 |
| 400 | 잘못된 요청 (파라미터 오류) |
| 401 | 인증 실패 (토큰 만료 또는 무효) |
| 403 | 접근 거부 |
| 404 | 리소스 미존재 |
| 500 | 서버 오류 |

### 6.3 에러 응답 포맷

```json
{
  "error_code": "INVALID_PARAMETER",
  "message": "입력 파라미터가 유효하지 않습니다."
}
```

---

## 7. API 요약 테이블

### 시세 조회 API

| 기능 | Method | URL | TR 코드 | WebSocket |
|------|--------|-----|---------|-----------|
| 현재가 조회 | POST | `/api/v1/quote/kr-stock/inquiry/price` | CSPAQ21600 | - |
| 호가 조회 | POST | `/api/v1/quote/kr-stock/inquiry/orderbook` | CSPAQ00300 | V20 |
| 체결 내역 | POST | `/api/v1/quote/kr-stock/inquiry/daily-chart` | CSPAQ35600 | - |
| OHLCV 조회 | POST | `/api/v1/quote/kr-stock/inquiry/ohlcv` | - | - |
| 해외 현재가 | POST | `/api/v1/quote/us-stock/inquiry/price` | - | - |
| 실시간 체결 | - | - | - | V60 |

### 주문/잔고 API

| 기능 | Method | URL | TR 코드 |
|------|--------|-----|---------|
| 주문 | POST | `/api/v1/trading/kr-stock/order` | CSPAT00600 |
| 주문 정정 | POST | `/api/v1/trading/kr-stock/order-modify` | CSPAT00601 |
| 주문 취소 | POST | `/api/v1/trading/kr-stock/order-cancel` | CSPAT00602 |
| 미체결 조회 | POST | `/api/v1/trading/kr-stock/inquiry/open-orders` | CSPAQ36610 |
| 잔고 조회 | POST | `/api/v1/trading/kr-stock/inquiry/balance` | CSPAQ03420 |

---

## 8. 참고 사항

### 8.1 API 포털 안내

- **공식 홈페이지**: https://openapi.db-fi.com
- **API 가이드**: https://openapi.dbsec.co.kr/about-openapi
- **테스트베드 샘플**: https://openapi.dbsec.co.kr/testbed-sample

### 8.2 개발자 문서

더 자세한 API 명세, 샘플 코드, 에러 처리 방법은 공식 [DB금융투자 OPEN API](https://openapi.db-fi.com) 사이트에서 확인할 수 있습니다.

GitHub 예제: https://gist.github.com/jayu108/2627e46eb246e1085d532430094042bb

### 8.3 WebSocket 주의사항

- WebSocket 메시지 송수신은 JSON 형식
- 토큰 만료 시 자동으로 재연결 구현 필수
- 대량 데이터 구독 시 연결 제한 있을 수 있음

### 8.4 API 사용 제한

- **호출 빈도**: 초당 API 호출 제한 있음 (공식 문서 확인 필요)
- **토큰 유효기간**: 24시간 (매일 재발급 권장)
- **거래 시간**: 09:00 ~ 15:30 (국내주식)

### 8.5 보안 안내

- appkey/appsecret은 절대 공개하지 않기
- 클라이언트 정보는 환경 변수로 관리
- WebSocket 연결 시 TLS 1.2 이상 사용

---

## 크롤링 불가 항목

다음 정보는 공식 DB금융투자 OPEN API 포털에서 직접 확인이 필요합니다:

- [크롤링 불가] 상세한 에러 코드 목록
- [크롤링 불가] Rate Limiting 정책 및 할당량
- [크롤링 불가] 고급 필터 및 조건 파라미터
- [크롤링 불가] 이벤트 기반 API (체결 알림 상세 스펙)
- [크롤링 불가] 신용거래, 선물옵션 API
- [크롤링 불가] 해외 거래소별 상세 스펙 (나스닥, NYSE, AMEX 등)

---

*문서 마지막 업데이트: 2026-02-10*
*이 문서는 웹 검색을 통해 공개된 정보를 기반으로 작성되었습니다.*
*최신 정보는 [DB금융투자 공식 API 포털](https://openapi.db-fi.com)에서 확인하시기 바랍니다.*
