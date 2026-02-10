# Bithumb OPEN API 명세서

> **버전**: v2.1.5 | **최종 업데이트**: 2026-02-10
> **공식 문서**: https://apidocs.bithumb.com/
> **기본 URL**: https://api.bithumb.com

---

## 개요

Bithumb OPEN API는 REST와 WebSocket 두 가지 프로토콜로 제공되며, 공개 시세 데이터와 비공개 거래 기능을 지원합니다.

### 기본 정보

| 항목 | 값 |
|------|-----|
| **Base URL (REST)** | https://api.bithumb.com |
| **API 버전** | v2.1.5 |
| **인증 방식** | API Key + Secret (HMAC-SHA512) |
| **응답 형식** | JSON |
| **Request Method** | GET, POST |
| **Content-Type** | application/json |

### 인증 (Authentication)

#### Private API 인증 방식

Private API는 API Key와 Secret을 이용한 HMAC-SHA512 서명이 필요합니다.

**필수 헤더**:
```
API-Key: {your-api-key}
API-Sign: {hmac-sha512-signature}
API-Nonce: {unix-timestamp-ms}
```

**서명 생성 (SHA-512)**:
```
API-Sign = BASE64(HMAC-SHA512(API-Secret, request-body))
```

**예제**:
```
POST /v1/accounts
Body: {account parameters}
API-Sign = HMAC-SHA512 암호화(Secret, Body)
```

### Rate Limit (요청 제한)

| API 유형 | 제한 |
|---------|------|
| **Public API** | 900 req/min (15 req/sec) |
| **Private API** | 900 req/min (15 req/sec) |
| **WebSocket** | 10 connections/IP |

**차단 시 응답**: HTTP 429 Too Many Requests

---

## 1. Public API - 시세 조회

Public API는 인증 없이 시장 데이터를 조회할 수 있습니다.

---

### 1.1 마켓 코드 조회

빗썸에서 거래 가능한 마켓과 가상자산 정보를 조회합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **메서드** | GET |
| **URL** | /v1/market/all |
| **설명** | 거래 가능한 전체 마켓 및 종목 정보 조회 |
| **Rate Limit** | 공개 API 표준 |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| `market` | String | O | 마켓 코드 (KRW, BTC, USDT 등) |

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | String | 마켓 코드 (예: KRW-BTC, BTC-ETH) |
| 2 | korean_name | String | 한글 이름 |
| 3 | english_name | String | 영문 이름 |
| 4 | market_event | Object | 마켓 이벤트 정보 |
| 4.1 | caution | Object | 주의 사항 |
| 4.1.1 | caution_unified | String | 통합 주의 코드 |
| 4.1.2 | caution_display_order | Number | 표시 순서 |

#### 응답 예시

```json
[
  {
    "market": "KRW-BTC",
    "korean_name": "비트코인",
    "english_name": "Bitcoin",
    "market_event": {
      "caution": {
        "caution_unified": "CAUTION",
        "caution_display_order": 0
      }
    }
  }
]
```

---

### 1.2 현재가 정보 조회 (Ticker)

요청 시점 종목의 실시간 시세 스냅샷을 제공합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **메서드** | GET |
| **URL** | /v1/ticker |
| **설명** | 현재가(최근 체결가) 및 시가, 고가, 저가, 거래량 조회 |
| **Rate Limit** | 공개 API 표준 |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| `markets` | String | O | 반점으로 구분되는 마켓 코드 (예: KRW-BTC,KRW-ETH) |

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | String | 종목 구분 코드 |
| 2 | trade_date | String | 최근 거래 일자(UTC, yyyyMMdd) |
| 3 | trade_time | String | 최근 거래 시각(UTC, HHmmss) |
| 4 | trade_date_kst | String | 최근 거래 일자(KST, yyyyMMdd) |
| 5 | trade_time_kst | String | 최근 거래 시각(KST, HHmmss) |
| 6 | trade_timestamp | Number | 최근 거래 일시(Unix Timestamp, Long) |
| 7 | opening_price | Number | 시가(Decimal로 사용 권장) |
| 8 | high_price | Number | 고가(Decimal로 사용 권장) |
| 9 | low_price | Number | 저가(Decimal로 사용 권장) |
| 10 | trade_price | Number | 종가(현재가, Decimal로 사용 권장) |
| 11 | prev_closing_price | Number | 전일 종가(KST 0시 기준, Decimal로 사용 권장) |
| 12 | change | String | 변화 상태 (EVEN: 보합, RISE: 상승, FALL: 하락) |
| 13 | change_price | Number | 변화액의 절대값(Decimal로 사용 권장) |
| 14 | change_rate | Number | 변화율의 절대값(Decimal로 사용 권장) |
| 15 | signed_change_price | Number | 부호가 있는 변화액(Decimal로 사용 권장) |
| 16 | signed_change_rate | Number | 부호가 있는 변화율(Decimal로 사용 권장) |
| 17 | trade_volume | Number | 가장 최근 거래량(Decimal로 사용 권장) |
| 18 | acc_trade_price | Number | 누적 거래대금(KST 0시 기준, Decimal로 사용 권장) |
| 19 | acc_trade_price_24h | Number | 24시간 누적 거래대금(Decimal로 사용 권장) |
| 20 | acc_trade_volume | Number | 누적 거래량(KST 0시 기준, Decimal로 사용 권장) |
| 21 | acc_trade_volume_24h | Number | 24시간 누적 거래량(Decimal로 사용 권장) |
| 22 | highest_52_week_price | Number | 52주 신고가(Decimal로 사용 권장) |
| 23 | highest_52_week_date | String | 52주 신고가 달성일(yyyy-MM-dd) |
| 24 | lowest_52_week_price | Number | 52주 신저가(Decimal로 사용 권장) |
| 25 | lowest_52_week_date | String | 52주 신저가 달성일(yyyy-MM-dd) |
| 26 | timestamp | Number | 타임스탬프(Unix Timestamp, Long) |

#### 응답 예시

```json
[
  {
    "market": "KRW-BTC",
    "trade_date": "20260210",
    "trade_time": "073015",
    "trade_date_kst": "20260210",
    "trade_time_kst": "163015",
    "trade_timestamp": 1707489015000,
    "opening_price": 95000000,
    "high_price": 96500000,
    "low_price": 94000000,
    "trade_price": 95500000,
    "prev_closing_price": 95000000,
    "change": "RISE",
    "change_price": 500000,
    "change_rate": 0.00526,
    "signed_change_price": 500000,
    "signed_change_rate": 0.00526,
    "trade_volume": 0.5,
    "acc_trade_price": 10000000000,
    "acc_trade_price_24h": 15000000000,
    "acc_trade_volume": 105,
    "acc_trade_volume_24h": 150,
    "highest_52_week_price": 98000000,
    "highest_52_week_date": "2025-12-15",
    "lowest_52_week_price": 60000000,
    "lowest_52_week_date": "2025-06-20",
    "timestamp": 1707489015000
  }
]
```

#### 주의 사항

- **변화율**: change_price, change_rate, signed_change_price, signed_change_rate는 **전일종가 기준**의 변화값입니다.
- **금융 계산**: 모든 가격/수량 필드는 **Decimal** 또는 **String** 타입으로 사용하여 부동소수점 오차를 방지하세요.

---

### 1.3 호가 정보 조회 (Orderbook)

실시간 매도/매수 호가 정보를 조회합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **메서드** | GET |
| **URL** | /v1/orderbook |
| **설명** | 실시간 호가(bid/ask) 및 잔량 조회 |
| **Rate Limit** | 공개 API 표준 |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| `markets` | String/Array | O | 마켓 코드 목록 (예: KRW-BTC,BTC-ETH) |

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | String | 마켓 코드 |
| 2 | timestamp | Number | 호가 생성 시각(Unix Timestamp, Long) |
| 3 | total_ask_size | Number | 호가 매도 총 잔량(Decimal로 사용 권장) |
| 4 | total_bid_size | Number | 호가 매수 총 잔량(Decimal로 사용 권장) |
| 5 | orderbook_units | Array | 호가 단위 배열 |
| 5.1 | ask_price | Number | 매도호가(Decimal로 사용 권장) |
| 5.2 | bid_price | Number | 매수호가(Decimal로 사용 권장) |
| 5.3 | ask_size | Number | 매도 잔량(Decimal로 사용 권장) |
| 5.4 | bid_size | Number | 매수 잔량(Decimal로 사용 권장) |

#### 호가 정보 깊이

- **다중 마켓**: 1~15호가 정보 제공
- **단일 마켓**: 1~30호가 정보 제공

#### 응답 예시

```json
[
  {
    "market": "KRW-BTC",
    "timestamp": 1707489015000,
    "total_ask_size": 10.5,
    "total_bid_size": 9.3,
    "orderbook_units": [
      {
        "ask_price": 95500000,
        "bid_price": 95400000,
        "ask_size": 0.5,
        "bid_size": 0.8
      },
      {
        "ask_price": 95600000,
        "bid_price": 95300000,
        "ask_size": 1.2,
        "bid_size": 1.5
      }
    ]
  }
]
```

---

### 1.4 체결 내역 조회 (Trades)

최근 체결 거래 내역을 조회합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **메서드** | GET |
| **URL** | /v1/trades |
| **설명** | 최근 체결 내역 조회 |
| **Rate Limit** | 공개 API 표준 |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| `market` | String | O | 마켓 코드 (예: KRW-BTC) |
| `daysAgo` | Number | X | 며칠 전부터 (기본값: 1, 최대: 30) |
| `to` | Number | X | 시작점 기준 (Unix Timestamp) |
| `count` | Number | X | 조회 개수 (기본값: 30, 최대: 500) |
| `orderAscending` | Boolean | X | 오름차순 정렬 여부 (기본값: false) |

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | String | 마켓 코드 |
| 2 | trade_id | Number | 거래 ID |
| 3 | price | Number | 체결 가격(Decimal로 사용 권장) |
| 4 | size | Number | 체결 수량(Decimal로 사용 권장) |
| 5 | total_price | Number | 체결 대금(Decimal로 사용 권장) |
| 6 | trade_date | String | 체결 일자(yyyyMMdd) |
| 7 | trade_time | String | 체결 시각(HHmmss) |
| 8 | trade_timestamp | Number | 체결 일시(Unix Timestamp, Long) |
| 9 | ask_bid | String | 매도/매수 구분 (ASK: 매도, BID: 매수) |
| 10 | sequential_id | Number | 순차 ID |

#### 응답 예시

```json
[
  {
    "market": "KRW-BTC",
    "trade_id": 123456789,
    "price": 95500000,
    "size": 0.1,
    "total_price": 9550000,
    "trade_date": "20260210",
    "trade_time": "073015",
    "trade_timestamp": 1707489015000,
    "ask_bid": "ASK",
    "sequential_id": 9999999
  }
]
```

---

### 1.5 캔들 데이터 조회 (Candles)

OHLCV 캔들 데이터를 여러 타임프레임으로 조회할 수 있습니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **메서드** | GET |
| **URL** | /v1/candles/{timeframe} |
| **설명** | 캔들 데이터 조회 (1분, 5분, 15분, 30분, 1시간, 4시간, 1일, 1주, 1개월) |
| **Rate Limit** | 공개 API 표준 |

#### 지원 타임프레임

| 타임프레임 | URL | 설명 |
|-----------|-----|------|
| 1분 | `/v1/candles/minutes/1` | 1분 캔들 |
| 5분 | `/v1/candles/minutes/5` | 5분 캔들 |
| 15분 | `/v1/candles/minutes/15` | 15분 캔들 |
| 30분 | `/v1/candles/minutes/30` | 30분 캔들 |
| 1시간 | `/v1/candles/hours/1` | 1시간 캔들 |
| 4시간 | `/v1/candles/hours/4` | 4시간 캔들 |
| 1일 | `/v1/candles/days` | 일봉 |
| 1주 | `/v1/candles/weeks` | 주봉 |
| 1개월 | `/v1/candles/months` | 월봉 |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| `market` | String | O | 마켓 코드 (예: KRW-BTC) |
| `to` | String | X | 끝 시간 (ISO 8601 형식: YYYY-MM-DDTHH:mm:ss) |
| `count` | Number | X | 조회 개수 (기본값: 100, 최대: 1000) |

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | String | 마켓 코드 |
| 2 | candle_date_time_utc | String | 캔들 시작 시간(UTC, ISO 8601) |
| 3 | candle_date_time_kst | String | 캔들 시작 시간(KST, ISO 8601) |
| 4 | opening_price | Number | 시가(Decimal로 사용 권장) |
| 5 | high_price | Number | 고가(Decimal로 사용 권장) |
| 6 | low_price | Number | 저가(Decimal로 사용 권장) |
| 7 | trade_price | Number | 종가(Decimal로 사용 권장) |
| 8 | candle_acc_trade_price | Number | 누적 거래대금(Decimal로 사용 권장) |
| 9 | candle_acc_trade_volume | Number | 누적 거래량(Decimal로 사용 권장) |
| 10 | timestamp | Number | 캔들 종료 시간(Unix Timestamp, Long) |

#### 응답 예시

```json
[
  {
    "market": "KRW-BTC",
    "candle_date_time_utc": "2026-02-10T07:00:00",
    "candle_date_time_kst": "2026-02-10T16:00:00",
    "opening_price": 95000000,
    "high_price": 96500000,
    "low_price": 94500000,
    "trade_price": 95800000,
    "candle_acc_trade_price": 50000000000,
    "candle_acc_trade_volume": 525,
    "timestamp": 1707490800000
  }
]
```

---

## 2. Private API - 거래 및 자산 관리

Private API는 API Key/Secret 인증이 필요합니다.

---

### 2.1 계좌 조회 (Account Info)

전체 계좌 정보와 자산 조회

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **메서드** | GET |
| **URL** | /v1/accounts |
| **설명** | 전체 계좌 정보 조회 (자산, 보유 코인, 평가금액) |
| **인증** | 필수 (API Key + Secret) |
| **Rate Limit** | 개인 API 표준 (900 req/min) |

#### 요청 파라미터

없음 (헤더의 API Key/Secret 사용)

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | member_level | Number | 회원 레벨 |
| 2 | account_id | Number | 계좌 ID |
| 3 | wallet_address | String | 지갑 주소 |
| 4 | identifiers | Array | 식별자 목록 |
| 5 | created_at | Number | 계좌 생성 시간(Unix Timestamp) |
| 6 | currency | String | 기본 통화 (KRW) |
| 7 | balance | Number | 계좌 잔액(KRW, Decimal로 사용 권장) |
| 8 | locked | Number | 락된 금액(Decimal로 사용 권장) |
| 9 | available | Number | 사용 가능 금액(Decimal로 사용 권장) |
| 10 | total_xlm_trust_limit | Number | XLM 신뢰 한계 |

#### 응답 예시

```json
[
  {
    "member_level": 1,
    "account_id": 12345678,
    "wallet_address": "0x1234567890abcdef",
    "identifiers": ["KRW"],
    "created_at": 1234567890000,
    "currency": "KRW",
    "balance": "1000000",
    "locked": "100000",
    "available": "900000",
    "total_xlm_trust_limit": 0
  }
]
```

---

### 2.2 보유 자산 조회 (Balances)

특정 코인의 보유량 및 거래 가능 여부 조회

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **메서드** | GET |
| **URL** | /v1/balances |
| **설명** | 보유 코인별 잔액, 락된 금액, 사용 가능 금액 조회 |
| **인증** | 필수 (API Key + Secret) |
| **Rate Limit** | 개인 API 표준 |

#### 요청 파라미터

없음 (헤더의 API Key/Secret 사용)

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | currency | String | 자산 코드 (BTC, ETH, KRW 등) |
| 2 | balance | Number | 보유량(Decimal로 사용 권장) |
| 3 | locked | Number | 락된 량(주문/거래 중, Decimal로 사용 권장) |
| 4 | available | Number | 사용 가능한 량(Decimal로 사용 권장) |
| 5 | avg_buy_price | Number | 평균 매입가(Decimal로 사용 권장) |
| 6 | total_buy_price | Number | 총 매입액(Decimal로 사용 권장) |

#### 응답 예시

```json
[
  {
    "currency": "BTC",
    "balance": "1.5",
    "locked": "0.3",
    "available": "1.2",
    "avg_buy_price": "45000000",
    "total_buy_price": "67500000"
  },
  {
    "currency": "KRW",
    "balance": "5000000",
    "locked": "1000000",
    "available": "4000000",
    "avg_buy_price": "0",
    "total_buy_price": "0"
  }
]
```

---

### 2.3 주문 생성 (Create Order)

매수/매도 주문을 생성합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **메서드** | POST |
| **URL** | /v1/orders |
| **설명** | 지정가/시장가 주문 생성 |
| **인증** | 필수 (API Key + Secret) |
| **Rate Limit** | 개인 API 표준 |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| `market` | String | O | 마켓 코드 (예: KRW-BTC) |
| `side` | String | O | 주문 방향 (buy: 매수, sell: 매도) |
| `ord_type` | String | O | 주문 유형 (limit: 지정가, price: 시장가-원화, market: 시장가-수량) |
| `price` | Number/String | X | 주문 가격(ord_type=limit일 때 필수, Decimal 권장) |
| `volume` | Number/String | X | 주문 수량(ord_type!=price일 때 필수, Decimal 권장) |
| `time_in_force` | String | X | 유효기간 (ioc: 즉시체결, fok: 즉시전량체결, gtc: 무기한) |

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | uuid | String | 주문 고유 ID |
| 2 | side | String | 주문 방향 (buy, sell) |
| 3 | ord_type | String | 주문 유형 (limit, price, market) |
| 4 | price | Number | 주문 가격(Decimal로 사용 권장) |
| 5 | state | String | 주문 상태 (wait: 대기중, done: 완료, cancel: 취소됨) |
| 6 | market | String | 마켓 코드 |
| 7 | created_at | String | 주문 생성 시간(ISO 8601) |
| 8 | volume | Number | 주문 수량(Decimal로 사용 권장) |
| 9 | remaining_volume | Number | 미체결 수량(Decimal로 사용 권장) |
| 10 | reserved_fee | Number | 예약 수수료(Decimal로 사용 권장) |
| 11 | remaining_fee | Number | 남은 수수료(Decimal로 사용 권장) |
| 12 | paid_fee | Number | 지불한 수수료(Decimal로 사용 권장) |
| 13 | locked | Number | 락된 금액(Decimal로 사용 권장) |
| 14 | executed_volume | Number | 체결된 수량(Decimal로 사용 권장) |
| 15 | trades_count | Number | 거래 수 |
| 16 | user_id | Number | 사용자 ID |
| 17 | identifier | String | 식별자 |
| 18 | avg_price | Number | 평균 체결가(Decimal로 사용 권장) |

#### 주문 유형

| 유형 | 설명 | 필수 파라미터 |
|------|------|--------------|
| **limit** | 지정가 주문 | price, volume |
| **price** | 시장가 주문 (원화 기준) | price |
| **market** | 시장가 주문 (수량 기준) | volume |

#### 응답 예시

```json
{
  "uuid": "cfc0dae5-5dde-4edb-b0f5-8f1f8b7c3c4e",
  "side": "buy",
  "ord_type": "limit",
  "price": "95000000",
  "state": "wait",
  "market": "KRW-BTC",
  "created_at": "2026-02-10T16:30:15+09:00",
  "volume": "0.1",
  "remaining_volume": "0.1",
  "reserved_fee": "950000",
  "remaining_fee": "950000",
  "paid_fee": "0",
  "locked": "9500000",
  "executed_volume": "0",
  "trades_count": 0,
  "user_id": 12345678,
  "identifier": "KRW",
  "avg_price": "0"
}
```

---

### 2.4 주문 취소 (Cancel Order)

미체결 주문을 취소합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **메서드** | DELETE |
| **URL** | /v1/orders/{uuid} |
| **설명** | 주문 취소 (대기 중인 주문만 취소 가능) |
| **인증** | 필수 (API Key + Secret) |
| **Rate Limit** | 개인 API 표준 |

#### 경로 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| `uuid` | String | O | 주문 고유 ID |

#### 응답 필드

주문 생성 응답과 동일하며, 상태가 `cancel`로 변경됩니다.

#### 응답 예시

```json
{
  "uuid": "cfc0dae5-5dde-4edb-b0f5-8f1f8b7c3c4e",
  "side": "buy",
  "ord_type": "limit",
  "price": "95000000",
  "state": "cancel",
  "market": "KRW-BTC",
  "created_at": "2026-02-10T16:30:15+09:00",
  "volume": "0.1",
  "remaining_volume": "0.1",
  "reserved_fee": "950000",
  "remaining_fee": "950000",
  "paid_fee": "0",
  "locked": "0",
  "executed_volume": "0",
  "trades_count": 0,
  "user_id": 12345678,
  "identifier": "KRW",
  "avg_price": "0"
}
```

---

### 2.5 주문 조회 (Get Order)

특정 주문의 상세 정보를 조회합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **메서드** | GET |
| **URL** | /v1/orders/{uuid} |
| **설명** | 특정 주문의 상세 정보 조회 |
| **인증** | 필수 (API Key + Secret) |
| **Rate Limit** | 개인 API 표준 |

#### 경로 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| `uuid` | String | O | 주문 고유 ID |

#### 응답 필드

주문 생성/취소 응답과 동일합니다.

---

### 2.6 미체결 주문 목록 (Open Orders)

현재 미체결 주문 목록을 조회합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **메서드** | GET |
| **URL** | /v1/orders/open |
| **설명** | 미체결 주문 목록 조회 |
| **인증** | 필수 (API Key + Secret) |
| **Rate Limit** | 개인 API 표준 |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| `market` | String | X | 특정 마켓만 조회 (예: KRW-BTC) |
| `state` | String | X | 주문 상태 (wait: 대기중, done: 완료) |
| `page` | Number | X | 페이지 번호 (기본값: 1) |
| `limit` | Number | X | 페이지당 개수 (기본값: 100, 최대: 1000) |
| `order_by` | String | X | 정렬 순서 (asc, desc) |

#### 응답 필드

주문 개별 조회와 동일한 필드를 배열로 반환합니다.

#### 응답 예시

```json
[
  {
    "uuid": "cfc0dae5-5dde-4edb-b0f5-8f1f8b7c3c4e",
    "side": "buy",
    "ord_type": "limit",
    "price": "95000000",
    "state": "wait",
    "market": "KRW-BTC",
    "created_at": "2026-02-10T16:30:15+09:00",
    "volume": "0.1",
    "remaining_volume": "0.05",
    "reserved_fee": "950000",
    "remaining_fee": "475000",
    "paid_fee": "475000",
    "locked": "4750000",
    "executed_volume": "0.05",
    "trades_count": 1,
    "user_id": 12345678,
    "identifier": "KRW",
    "avg_price": "95000000"
  }
]
```

---

### 2.7 거래 내역 조회 (Trades)

계좌의 체결 거래 내역을 조회합니다.

#### 기본 정보

| 항목 | 값 |
|------|-----|
| **메서드** | GET |
| **URL** | /v1/trades |
| **설명** | 개인 계좌의 거래 내역 조회 |
| **인증** | 필수 (API Key + Secret) |
| **Rate Limit** | 개인 API 표준 |

#### 요청 파라미터

| 파라미터명 | 타입 | 필수 | 설명 |
|-----------|------|------|------|
| `market` | String | X | 특정 마켓 조회 (예: KRW-BTC) |
| `limit` | Number | X | 조회 개수 (기본값: 100, 최대: 1000) |
| `page` | Number | X | 페이지 번호 |
| `order_by` | String | X | 정렬 순서 (asc, desc) |

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | String | 마켓 코드 |
| 2 | uuid | String | 주문 ID |
| 3 | price | Number | 체결 가격(Decimal로 사용 권장) |
| 4 | volume | Number | 체결 수량(Decimal로 사용 권장) |
| 5 | funds | Number | 체결 대금(Decimal로 사용 권장) |
| 6 | side | String | 매도/매수 (buy, sell) |
| 7 | created_at | String | 거래 시간(ISO 8601) |
| 8 | commission | Number | 수수료(Decimal로 사용 권장) |
| 9 | ask_fee | Number | 수수료율(Decimal로 사용 권장) |

---

## 3. WebSocket API

WebSocket을 통해 실시간 시세 및 거래 정보를 수신할 수 있습니다.

---

### 3.1 WebSocket 기본 정보

#### 연결 정보

| 항목 | 값 |
|------|-----|
| **Public WebSocket** | wss://pubwss.bithumb.com/pubws |
| **Private WebSocket** | wss://ws.bithumb.com/ws |
| **Max Connections** | 10개/IP |
| **Ping Interval** | 30초 |
| **Message Format** | JSON |

#### 인증 (Private WebSocket만)

Private WebSocket 구독 시 다음과 같이 인증합니다:

```json
{
  "type": "authorization",
  "api_key": "{your-api-key}",
  "api_secret": "{your-api-secret}",
  "api_nonce": "{unix-timestamp-ms}"
}
```

---

### 3.2 Public WebSocket - 현재가 (Ticker)

실시간 현재가 정보를 구독합니다.

#### 구독 요청

```json
{
  "type": "ticker",
  "symbols": ["KRW-BTC", "KRW-ETH"]
}
```

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | type | String | "ticker" |
| 2 | code | String | 마켓 코드 |
| 3 | trade_price | Number | 현재 거래가(Decimal로 사용 권장) |
| 4 | trade_volume | Number | 거래량(Decimal로 사용 권장) |
| 5 | trade_timestamp | Number | 거래 시간(Unix Timestamp) |
| 6 | acc_trade_price_24h | Number | 24시간 거래대금(Decimal로 사용 권장) |
| 7 | acc_trade_volume_24h | Number | 24시간 거래량(Decimal로 사용 권장) |
| 8 | highest_52_week_price | Number | 52주 신고가(Decimal로 사용 권장) |
| 9 | lowest_52_week_price | Number | 52주 신저가(Decimal로 사용 권장) |
| 10 | opening_price | Number | 시가(Decimal로 사용 권장) |
| 11 | high_price | Number | 고가(Decimal로 사용 권장) |
| 12 | low_price | Number | 저가(Decimal로 사용 권장) |
| 13 | prev_closing_price | Number | 전일 종가(Decimal로 사용 권장) |
| 14 | change | String | 변화 상태 (RISE, FALL, EVEN) |
| 15 | change_price | Number | 변화액(Decimal로 사용 권장) |
| 16 | change_rate | Number | 변화율(Decimal로 사용 권장) |

#### 응답 예시

```json
{
  "type": "ticker",
  "code": "KRW-BTC",
  "trade_price": "95500000",
  "trade_volume": "0.1",
  "trade_timestamp": 1707489015000,
  "acc_trade_price_24h": "15000000000",
  "acc_trade_volume_24h": "150",
  "highest_52_week_price": "98000000",
  "lowest_52_week_price": "60000000",
  "opening_price": "95000000",
  "high_price": "96500000",
  "low_price": "94000000",
  "prev_closing_price": "95000000",
  "change": "RISE",
  "change_price": "500000",
  "change_rate": "0.00526"
}
```

---

### 3.3 Public WebSocket - 체결 (Trades)

실시간 체결 정보를 구독합니다.

#### 구독 요청

```json
{
  "type": "trade",
  "symbols": ["KRW-BTC"]
}
```

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | type | String | "trade" |
| 2 | code | String | 마켓 코드 |
| 3 | trade_price | Number | 체결 가격(Decimal로 사용 권장) |
| 4 | trade_volume | Number | 체결 수량(Decimal로 사용 권장) |
| 5 | ask_bid | String | 매도/매수 (ASK, BID) |
| 6 | trade_timestamp | Number | 체결 시간(Unix Timestamp) |
| 7 | trade_date | String | 체결 일자(yyyyMMdd) |
| 8 | trade_time | String | 체결 시각(HHmmss) |
| 9 | sequential_id | Number | 순차 ID |

#### 응답 예시

```json
{
  "type": "trade",
  "code": "KRW-BTC",
  "trade_price": "95500000",
  "trade_volume": "0.1",
  "ask_bid": "BID",
  "trade_timestamp": 1707489015000,
  "trade_date": "20260210",
  "trade_time": "163015",
  "sequential_id": 9999999
}
```

---

### 3.4 Public WebSocket - 호가 (Orderbook)

실시간 호가 정보를 구독합니다.

#### 구독 요청

```json
{
  "type": "orderbook",
  "symbols": ["KRW-BTC"]
}
```

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | type | String | "orderbook" |
| 2 | code | String | 마켓 코드 |
| 3 | timestamp | Number | 호가 시간(Unix Timestamp) |
| 4 | total_ask_size | Number | 매도 총 잔량(Decimal로 사용 권장) |
| 5 | total_bid_size | Number | 매수 총 잔량(Decimal로 사용 권장) |
| 6 | orderbook_units | Array | 호가 배열 |
| 6.1 | ask_price | Number | 매도호가(Decimal로 사용 권장) |
| 6.2 | bid_price | Number | 매수호가(Decimal로 사용 권장) |
| 6.3 | ask_size | Number | 매도 잔량(Decimal로 사용 권장) |
| 6.4 | bid_size | Number | 매수 잔량(Decimal로 사용 권장) |

#### 응답 예시

```json
{
  "type": "orderbook",
  "code": "KRW-BTC",
  "timestamp": 1707489015000,
  "total_ask_size": "10.5",
  "total_bid_size": "9.3",
  "orderbook_units": [
    {
      "ask_price": "95500000",
      "bid_price": "95400000",
      "ask_size": "0.5",
      "bid_size": "0.8"
    }
  ]
}
```

---

### 3.5 Private WebSocket - 내 자산 (Account Balance)

계좌의 보유 자산 변동을 실시간으로 수신합니다.

#### 구독 요청 (인증 필수)

```json
{
  "type": "balance"
}
```

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | type | String | "balance" |
| 2 | currency | String | 코인/화폐 코드 |
| 3 | balance | Number | 잔액(Decimal로 사용 권장) |
| 4 | locked | Number | 락된 금액(Decimal로 사용 권장) |
| 5 | available | Number | 사용 가능 금액(Decimal로 사용 권장) |
| 6 | timestamp | Number | 업데이트 시간(Unix Timestamp) |

---

### 3.6 Private WebSocket - 주문 (Orders)

개인 계좌의 주문 생성/체결/취소 이벤트를 실시간으로 수신합니다.

#### 구독 요청 (인증 필수)

```json
{
  "type": "order"
}
```

#### 응답 필드

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | type | String | "order" |
| 2 | uuid | String | 주문 고유 ID |
| 3 | market | String | 마켓 코드 |
| 4 | side | String | 매도/매수 (buy, sell) |
| 5 | ord_type | String | 주문 유형 (limit, market, price) |
| 6 | price | Number | 주문 가격(Decimal로 사용 권장) |
| 7 | state | String | 주문 상태 (wait, done, cancel) |
| 8 | volume | Number | 주문 수량(Decimal로 사용 권장) |
| 9 | executed_volume | Number | 체결된 수량(Decimal로 사용 권장) |
| 10 | remaining_volume | Number | 미체결 수량(Decimal로 사용 권장) |
| 11 | created_at | String | 주문 생성 시간(ISO 8601) |
| 12 | timestamp | Number | 업데이트 시간(Unix Timestamp) |

---

### 3.7 WebSocket 연결 관리

#### Ping/Pong

서버는 30초마다 ping을 전송합니다. 클라이언트는 pong으로 응답해야 합니다.

```json
// 서버 → 클라이언트
{
  "type": "ping"
}

// 클라이언트 → 서버
{
  "type": "pong"
}
```

#### 구독 취소

```json
{
  "type": "unsubscribe",
  "symbols": ["KRW-BTC"]
}
```

#### 연결 종료

일반적인 WebSocket close 프로토콜을 사용합니다.

---

## 4. 에러 처리

### HTTP 상태 코드

| 코드 | 설명 | 처리 방법 |
|------|------|----------|
| **200** | 성공 | 데이터 사용 |
| **400** | 잘못된 요청 | 파라미터 검증 후 재요청 |
| **401** | 인증 실패 | API Key/Secret 확인 |
| **403** | 접근 금지 | IP 화이트리스트 확인 |
| **404** | 리소스 없음 | URL 및 리소스 ID 확인 |
| **429** | Rate Limit 초과 | 요청 대기 후 재시도 |
| **500** | 서버 에러 | 잠시 후 재시도 |
| **503** | 서비스 점검 | 공지사항 확인 |

### 에러 응답 예시

```json
{
  "error": {
    "name": "validation_error",
    "message": "invalid parameter: market",
    "code": "ERR_INVALID_PARAMETER"
  }
}
```

---

## 5. 구현 가이드

### Rust에서의 구현 팁

```rust
use rust_decimal::Decimal;
use reqwest::Client;

// 1. 가격/수량은 항상 Decimal 또는 String 사용
let price: Decimal = Decimal::from_str("95000000")?;

// 2. API 응답에서 숫자 필드는 String으로 받아서 Decimal로 변환
#[derive(Deserialize)]
struct TickerResponse {
    market: String,
    trade_price: String,  // String으로 받음
    #[serde(deserialize_with = "deserialize_decimal")]
    trade_volume: Decimal,
}

// 3. Unix Timestamp는 i64/u64 사용
let timestamp: i64 = 1707489015000;
let datetime = chrono::DateTime::from_timestamp_millis(timestamp);

// 4. 모든 API 요청에 에러 처리
match client.get(url).send().await {
    Ok(response) => { /* 처리 */ },
    Err(e) => { /* 에러 처리 */ }
}
```

### 인증 헤더 생성 (HMAC-SHA512)

```rust
use hmac::{Hmac, Mac};
use sha2::Sha512;
use base64::encode;

fn create_api_sign(secret: &str, body: &str) -> String {
    type HmacSha512 = Hmac<Sha512>;
    let mut mac = HmacSha512::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(body.as_bytes());
    encode(mac.finalize().into_bytes())
}
```

---

## 6. 주의 사항

### 금융 계산

- **모든 가격/수량 필드**: `Decimal` 또는 `String` 타입 사용 (f64 금지)
- **손실율 최소화**: API 응답에서 받은 String을 바로 Decimal로 변환

### 실시간 데이터 처리

- **WebSocket**: 연결 유지를 위해 30초마다 pong 응답 필수
- **Ticker 구독**: 심볼 수에 따라 트래픽 증가 고려

### Rate Limit 대응

- **Request Queue**: 요청을 대기열에서 처리 (초당 15개 이하)
- **Exponential Backoff**: 429 에러 발생 시 지수 백오프 사용

### 보안

- **API Secret 노출 방지**: 환경 변수로 관리
- **HTTPS만 사용**: 모든 REST API 요청은 HTTPS
- **WSS만 사용**: WebSocket은 wss:// 프로토콜만 사용

---

## 7. API 엔드포인트 요약 테이블

### Public API

| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/v1/market/all` | GET | 마켓 코드 조회 |
| `/v1/ticker` | GET | 현재가 정보 조회 |
| `/v1/orderbook` | GET | 호가 정보 조회 |
| `/v1/trades` | GET | 체결 내역 조회 |
| `/v1/candles/minutes/1` | GET | 1분 캔들 |
| `/v1/candles/minutes/5` | GET | 5분 캔들 |
| `/v1/candles/minutes/15` | GET | 15분 캔들 |
| `/v1/candles/minutes/30` | GET | 30분 캔들 |
| `/v1/candles/hours/1` | GET | 1시간 캔들 |
| `/v1/candles/hours/4` | GET | 4시간 캔들 |
| `/v1/candles/days` | GET | 일봉 |
| `/v1/candles/weeks` | GET | 주봉 |
| `/v1/candles/months` | GET | 월봉 |

### Private API

| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/v1/accounts` | GET | 계좌 정보 조회 |
| `/v1/balances` | GET | 보유 자산 조회 |
| `/v1/orders` | POST | 주문 생성 |
| `/v1/orders/{uuid}` | GET | 주문 조회 |
| `/v1/orders/{uuid}` | DELETE | 주문 취소 |
| `/v1/orders/open` | GET | 미체결 주문 조회 |
| `/v1/trades` | GET | 거래 내역 조회 |

### WebSocket

| 주소 | 유형 | 설명 |
|------|------|------|
| `wss://pubwss.bithumb.com/pubws` | Public | 공개 시세 정보 |
| `wss://ws.bithumb.com/ws` | Private | 개인 계좌 정보 (인증 필수) |

---

## 참고 자료

- **공식 문서**: https://apidocs.bithumb.com/
- **API Key 발급**: https://support.bithumb.com/hc/ko/articles/52815899880345
- **문의**: https://www.bithumb.com/react/legacy/customer_support/question
- **이용약관**: https://www.bithumb.com/react/terms/info-api

---

**최종 업데이트**: 2026-02-10
