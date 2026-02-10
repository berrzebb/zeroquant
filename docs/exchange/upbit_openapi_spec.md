# Upbit OPEN API 명세서

> 문서 생성일: 2026-02-10
> 크롤링 소스: https://docs.upbit.com/kr/reference
> API 버전: v1.6.1

## 개요

Upbit Open API는 업비트 거래소에서 제공하는 공식 API 서비스입니다.

### API 호출 정보

| 항목 | 값 |
|------|-----|
| Base URL | `https://api.upbit.com/v1` |
| WebSocket (Quotation) | `wss://api.upbit.com/websocket/v1` |
| WebSocket (Exchange) | `wss://api.upbit.com/websocket/v1/private` |
| 인증 방식 | JWT (HS256), Authorization: Bearer {token} |
| 응답 형식 | JSON |
| TLS | 1.2 이상 필수 (1.3 권장) |

### 인증 (JWT)

| Key | 설명 | 필수 |
|-----|------|------|
| access_key | API Key의 Access key | 필수 |
| nonce | 무작위 UUID 문자열 (매 요청마다 새 값) | 필수 |
| query_hash | 요청 쿼리 파라미터/본문의 Query String Hash값 | 조건부 (파라미터 있을 때) |
| query_hash_alg | Hash 알고리즘 (기본: SHA512) | 선택 |

### Rate Limit 정책

| 그룹 | 정책 | 측정 단위 | 대상 API |
|------|------|----------|----------|
| Quotation market | 초당 10회 | IP | 페어 목록 조회 |
| Quotation candle | 초당 10회 | IP | 캔들 조회 (초/분/일/주/월/연) |
| Quotation trade | 초당 10회 | IP | 최근 체결 내역 조회 |
| Quotation ticker | 초당 10회 | IP | 현재가 조회 |
| Quotation orderbook | 초당 10회 | IP | 호가 정보 조회 |
| Exchange default | 초당 30회 | 계정 | 잔고, 주문조회, 취소, 출금, 입금 등 |
| Exchange order | 초당 8회 | 계정 | 주문 생성, 취소 후 재주문 |
| Exchange order-test | 초당 8회 | 계정 | 주문 생성 테스트 |
| Exchange order-cancel-all | 2초당 1회 | 계정 | 주문 일괄 취소 |
| WebSocket connect | 초당 5회 | IP/계정 | WebSocket 연결 요청 |
| WebSocket message | 초당 5회, 분당 100회 | IP/계정 | WebSocket 데이터 요청 |

### API 권한 그룹

| 권한 그룹 | REST API | WebSocket |
|----------|----------|-----------|
| (무권한) | 서비스 상태 조회, API Key 목록 조회 | - |
| 자산조회 | 계정 잔고 조회 | myAsset 구독 |
| 주문하기 | 주문 생성/취소 | - |
| 주문조회 | 주문 가능정보/조회 | myOrder 구독 |
| 출금하기 | 디지털자산/원화 출금 | - |
| 출금조회 | 출금 가능정보/조회 | - |
| 입금하기 | 원화 입금, 트래블룰 검증 | - |
| 입금조회 | 입금 주소/조회 | - |

---

## 1. Quotation API - 시세 조회 (6개 API)

> Public API - 인증 없이 조회 가능

### 1.1 페어 목록 조회

| 항목 | 값 |
|------|-----|
| Method | `GET` |
| URL | `/v1/market/all` |
| 설명 | 업비트에서 지원하는 모든 페어 목록 조회 |
| Rate Limit | 초당 10회 (market 그룹) |

**입력 파라미터**

| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| is_details | boolean | X | 상세 정보 포함 여부 (유의종목 등). 기본값 false |

**출력 필드**

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | string | 페어 코드 (예: "KRW-BTC") |
| 2 | korean_name | string | 디지털 자산 한글명 |
| 3 | english_name | string | 디지털 자산 영문명 |
| 4 | market_event | object | 종목 경보 정보 (is_details=true 시) |

---

### 1.2 페어 단위 현재가 조회

| 항목 | 값 |
|------|-----|
| Method | `GET` |
| URL | `/v1/ticker` |
| 설명 | 지정한 페어의 현재가(Ticker) 스냅샷 조회 |
| Rate Limit | 초당 10회 (ticker 그룹) |

**입력 파라미터**

| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| markets | string | O | 페어 목록 (쉼표 구분). 예: KRW-BTC,KRW-ETH |

**출력 필드**

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | string | 페어 코드 |
| 2 | trade_date | string | 최근 체결 일자 (UTC, yyyyMMdd) |
| 3 | trade_time | string | 최근 체결 시각 (UTC, HHmmss) |
| 4 | trade_date_kst | string | 최근 체결 일자 (KST, yyyyMMdd) |
| 5 | trade_time_kst | string | 최근 체결 시각 (KST, HHmmss) |
| 6 | trade_timestamp | int64 | 체결 시각 타임스탬프 (ms) |
| 7 | opening_price | double | 시가 |
| 8 | high_price | double | 고가 |
| 9 | low_price | double | 저가 |
| 10 | trade_price | double | 현재가 (종가) |
| 11 | prev_closing_price | double | 전일 종가 (UTC 0시 기준) |
| 12 | change | string | 가격 변동 상태 (EVEN/RISE/FALL) |
| 13 | change_price | double | 전일 대비 가격 변화 절대값 |
| 14 | change_rate | double | 전일 대비 등락률 절대값 |
| 15 | signed_change_price | double | 전일 대비 가격 변화 (부호 포함) |
| 16 | signed_change_rate | double | 전일 대비 등락률 (부호 포함) |
| 17 | trade_volume | double | 최근 거래 수량 |
| 18 | acc_trade_price | double | 누적 거래금액 (UTC 0시 기준) |
| 19 | acc_trade_price_24h | double | 24시간 누적 거래금액 |
| 20 | acc_trade_volume | double | 누적 거래량 (UTC 0시 기준) |
| 21 | acc_trade_volume_24h | double | 24시간 누적 거래량 |
| 22 | highest_52_week_price | double | 52주 신고가 |
| 23 | highest_52_week_date | string | 52주 신고가 달성일 (yyyy-MM-dd) |
| 24 | lowest_52_week_price | double | 52주 신저가 |
| 25 | lowest_52_week_date | string | 52주 신저가 달성일 (yyyy-MM-dd) |
| 26 | timestamp | int64 | 현재가 반영 시각 타임스탬프 (ms) |

---

### 1.3 호가 조회

| 항목 | 값 |
|------|-----|
| Method | `GET` |
| URL | `/v1/orderbook` |
| 설명 | 지정 종목의 실시간 호가(Orderbook) 정보 조회 |
| Rate Limit | 초당 10회 (orderbook 그룹) |

**입력 파라미터**

| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| markets | string | O | 페어 목록 (쉼표 구분) |
| level | string | X | 호가 모아보기 단위 (KRW 마켓만). 기본값 0 |
| count | integer | X | 호가 쌍 개수. 기본값 30 |

**출력 필드**

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | string | 페어 코드 |
| 2 | timestamp | int64 | 조회 시각 타임스탬프 (ms) |
| 3 | total_ask_size | double | 전체 매도 잔량 합계 |
| 4 | total_bid_size | double | 전체 매수 잔량 합계 |
| 5 | orderbook_units | array | 호가 배열 (1~30호가) |
| 5.1 | orderbook_units.ask_price | double | 매도 호가 |
| 5.2 | orderbook_units.bid_price | double | 매수 호가 |
| 5.3 | orderbook_units.ask_size | double | 매도 잔량 |
| 5.4 | orderbook_units.bid_size | double | 매수 잔량 |
| 5.5 | orderbook_units.level | double | 적용 가격 단위. 기본값 0 |

---

### 1.4 최근 체결 내역 조회

| 항목 | 값 |
|------|-----|
| Method | `GET` |
| URL | `/v1/trades/ticks` |
| 설명 | 지정 페어의 최근 체결 목록 조회 |
| Rate Limit | 초당 10회 (trade 그룹) |

**입력 파라미터**

| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| market | string | O | 페어 코드 |
| to | string | X | 조회 종료 시각 (HHmmss 또는 HH:mm:ss) |
| count | integer | X | 조회 개수. 최대 500, 기본값 1 |
| cursor | string | X | Pagination 커서 (sequential_id 값) |
| days_ago | integer | X | 조회 대상 일자 offset (1~7). 빈값=당일 |

**출력 필드**

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | string | 페어 코드 |
| 2 | trade_date_utc | string | 체결 일자 (UTC, yyyy-MM-dd) |
| 3 | trade_time_utc | string | 체결 시각 (UTC, HH:mm:ss) |
| 4 | timestamp | int64 | 체결 시각 타임스탬프 (ms) |
| 5 | trade_price | double | 체결 가격 |
| 6 | trade_volume | double | 체결 수량 |
| 7 | prev_closing_price | double | 전일 종가 |
| 8 | change_price | double | 전일 대비 가격 변화 |
| 9 | ask_bid | string | 매수/매도 구분 (ASK/BID) |
| 10 | sequential_id | integer | 체결 유일 식별자 |

---

### 1.5 초(Second) 캔들 조회

| 항목 | 값 |
|------|-----|
| Method | `GET` |
| URL | `/v1/candles/seconds` |
| 설명 | 초 단위 캔들 목록 조회 (최대 3개월 이내) |
| Rate Limit | 초당 10회 (candle 그룹) |

**입력 파라미터**

| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| market | string | O | 페어 코드 |
| to | string | X | 조회 종료 시각 (ISO 8601). 미지정 시 최신 |
| count | integer | X | 캔들 개수. 최대 200, 기본값 1 |

**출력 필드**

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | string | 페어 코드 |
| 2 | candle_date_time_utc | string | 캔들 시작 시각 (UTC) |
| 3 | candle_date_time_kst | string | 캔들 시작 시각 (KST) |
| 4 | opening_price | double | 시가 |
| 5 | high_price | double | 고가 |
| 6 | low_price | double | 저가 |
| 7 | trade_price | double | 종가 |
| 8 | timestamp | int64 | 마지막 틱 타임스탬프 (ms) |
| 9 | candle_acc_trade_price | double | 누적 거래금액 |
| 10 | candle_acc_trade_volume | double | 누적 거래량 |

---

### 1.6 분(Minute) 캔들 조회

| 항목 | 값 |
|------|-----|
| Method | `GET` |
| URL | `/v1/candles/minutes/{unit}` |
| 설명 | 분 단위 캔들 목록 조회. unit: 1,3,5,10,15,30,60,240 |
| Rate Limit | 초당 10회 (candle 그룹) |

**입력 파라미터**

| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| unit | int32 (path) | O | 분봉 단위 (1/3/5/10/15/30/60/240) |
| market | string | O | 페어 코드 |
| to | string | X | 조회 종료 시각 (ISO 8601) |
| count | integer | X | 캔들 개수. 최대 200, 기본값 1 |

**출력 필드**

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | string | 페어 코드 |
| 2 | candle_date_time_utc | string | 캔들 시작 시각 (UTC) |
| 3 | candle_date_time_kst | string | 캔들 시작 시각 (KST) |
| 4 | opening_price | double | 시가 |
| 5 | high_price | double | 고가 |
| 6 | low_price | double | 저가 |
| 7 | trade_price | double | 종가 |
| 8 | timestamp | int64 | 마지막 틱 타임스탬프 (ms) |
| 9 | candle_acc_trade_price | double | 누적 거래금액 |
| 10 | candle_acc_trade_volume | double | 누적 거래량 |
| 11 | unit | integer | 캔들 집계 단위 (분) |

---

## 2. Exchange API - 거래 및 자산 관리 (7개 API)

> Private API - JWT 인증 필수

### 2.1 계정 잔고 조회

| 항목 | 값 |
|------|-----|
| Method | `GET` |
| URL | `/v1/accounts` |
| 설명 | 보유 자산 목록과 잔고 조회 |
| Rate Limit | 초당 30회 (Exchange default 그룹) |
| 권한 | 자산조회 |

**입력 파라미터**: 없음

**출력 필드**

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | currency | string | 통화 코드 |
| 2 | balance | string | 주문 가능 수량/금액 |
| 3 | locked | string | 출금/주문에 잠긴 잔액 |
| 4 | avg_buy_price | string | 매수 평균가 |
| 5 | avg_buy_price_modified | boolean | 매수 평균가 수정 여부 |
| 6 | unit_currency | string | 평균가 기준 통화 (KRW/BTC/USDT) |

---

### 2.2 페어별 주문 가능 정보 조회

| 항목 | 값 |
|------|-----|
| Method | `GET` |
| URL | `/v1/orders/chance` |
| 설명 | 지정 페어의 주문 가능 정보 (수수료, 주문유형, 잔고 등) |
| Rate Limit | 초당 30회 (Exchange default 그룹) |
| 권한 | 주문조회 |

**입력 파라미터**

| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| market | string | O | 페어 코드 |

**출력 필드**

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | bid_fee | string | 매수 수수료율 |
| 2 | ask_fee | string | 매도 수수료율 |
| 3 | maker_bid_fee | string | 매수 maker 수수료율 |
| 4 | maker_ask_fee | string | 매도 maker 수수료율 |
| 5 | market | object | 마켓 정보 (order_sides, bid_types, ask_types, max_total 등) |
| 6 | bid_account | object | 호가 자산 계좌 정보 |
| 7 | ask_account | object | 기준 자산 계좌 정보 |

---

### 2.3 주문 생성

| 항목 | 값 |
|------|-----|
| Method | `POST` |
| URL | `/v1/orders` |
| 설명 | 매수/매도 주문 생성 (JSON Body) |
| Rate Limit | 초당 8회 (order 그룹) |
| 권한 | 주문하기 |

**입력 파라미터 (Body)**

| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| market | string | O | 대상 페어 |
| side | string (enum) | O | 주문 방향: `ask` (매도) / `bid` (매수) |
| volume | string | 조건부 | 주문 수량 (지정가, 시장가 매도, 최유리 매도 시 필수) |
| price | string | 조건부 | 주문 단가/총액 (지정가, 시장가 매수, 최유리 매수 시 필수) |
| ord_type | string (enum) | O | 주문 유형: `limit`/`price`/`market`/`best` |
| identifier | string | X | 클라이언트 주문 식별자 (유일값, 재사용 불가) |
| time_in_force | string (enum) | 조건부 | 체결 조건: `ioc`/`fok`/`post_only` (best 주문 시 ioc/fok 필수) |
| smp_type | string (enum) | X | 자전거래 방지: `cancel_maker`/`cancel_taker`/`reduce` |

**출력 필드**

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | string | 페어 코드 |
| 2 | uuid | string | 주문 유일 식별자 |
| 3 | side | string | 주문 방향 (ask/bid) |
| 4 | ord_type | string | 주문 유형 (limit/price/market/best) |
| 5 | price | string | 주문 단가/총액 |
| 6 | state | string | 주문 상태 (wait/watch/done/cancel) |
| 7 | created_at | string | 주문 생성 시각 (KST) |
| 8 | volume | string | 주문 수량 |
| 9 | remaining_volume | string | 잔여 수량 |
| 10 | executed_volume | string | 체결 수량 |
| 11 | reserved_fee | string | 예약 수수료 |
| 12 | remaining_fee | string | 남은 수수료 |
| 13 | paid_fee | string | 사용 수수료 |
| 14 | locked | string | 사용 중 비용 |
| 15 | trades_count | integer | 체결 건수 |

---

### 2.4 개별 주문 취소

| 항목 | 값 |
|------|-----|
| Method | `DELETE` |
| URL | `/v1/order` |
| 설명 | UUID 또는 Identifier로 주문 취소 |
| Rate Limit | 초당 30회 (Exchange default 그룹) |
| 권한 | 주문하기 |

**입력 파라미터**

| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| uuid | string | 조건부 | 취소할 주문의 UUID (둘 중 하나 필수) |
| identifier | string | 조건부 | 클라이언트 지정 주문 식별자 (둘 중 하나 필수) |

**출력 필드**

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | string | 페어 코드 |
| 2 | uuid | string | 주문 유일 식별자 |
| 3 | side | string | 주문 방향 (ask/bid) |
| 4 | ord_type | string | 주문 유형 |
| 5 | price | string | 주문 단가/총액 |
| 6 | state | string | 주문 상태 (wait/watch) |
| 7 | created_at | string | 주문 생성 시각 |
| 8 | volume | string | 주문 수량 |
| 9 | remaining_volume | string | 잔여 수량 |
| 10 | executed_volume | string | 체결 수량 |
| 11 | reserved_fee | string | 예약 수수료 |
| 12 | remaining_fee | string | 남은 수수료 |
| 13 | paid_fee | string | 사용 수수료 |
| 14 | locked | string | 사용 중 비용 |
| 15 | time_in_force | string | 체결 옵션 (fok/ioc/post_only) |
| 16 | identifier | string | 클라이언트 주문 식별자 |
| 17 | smp_type | string | 자전거래 방지 모드 |
| 18 | prevented_volume | string | 자전거래 방지 취소 수량 |
| 19 | prevented_locked | string | 자전거래 방지 해제 자산 |
| 20 | trades_count | integer | 체결 건수 |

---

### 2.5 개별 주문 조회

| 항목 | 값 |
|------|-----|
| Method | `GET` |
| URL | `/v1/order` |
| 설명 | UUID 또는 Identifier로 단일 주문 상세 조회 (체결 목록 포함) |
| Rate Limit | 초당 30회 (Exchange default 그룹) |
| 권한 | 주문조회 |

**입력 파라미터**

| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| uuid | string | 조건부 | 조회할 주문의 UUID (둘 중 하나 필수) |
| identifier | string | 조건부 | 클라이언트 지정 주문 식별자 (둘 중 하나 필수) |

**출력 필드**

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | string | 페어 코드 |
| 2 | uuid | string | 주문 유일 식별자 |
| 3 | side | string | 주문 방향 |
| 4 | ord_type | string | 주문 유형 |
| 5 | price | string | 주문 단가/총액 |
| 6 | state | string | 주문 상태 (wait/watch/done/cancel) |
| 7 | created_at | string | 주문 생성 시각 |
| 8 | volume | string | 주문 수량 |
| 9 | remaining_volume | string | 잔여 수량 |
| 10 | executed_volume | string | 체결 수량 |
| 11 | reserved_fee | string | 예약 수수료 |
| 12 | remaining_fee | string | 남은 수수료 |
| 13 | paid_fee | string | 사용 수수료 |
| 14 | locked | string | 사용 중 비용 |
| 15 | time_in_force | string | 체결 옵션 |
| 16 | smp_type | string | 자전거래 방지 모드 |
| 17 | prevented_volume | string | 자전거래 방지 취소 수량 |
| 18 | prevented_locked | string | 자전거래 방지 해제 자산 |
| 19 | identifier | string | 클라이언트 주문 식별자 |
| 20 | trades_count | integer | 체결 건수 |
| 21 | trades | array | 체결 목록 |
| 21.1 | trades.market | string | 페어 코드 |
| 21.2 | trades.uuid | string | 체결 유일 식별자 |
| 21.3 | trades.price | string | 체결 단가 |
| 21.4 | trades.volume | string | 체결 수량 |
| 21.5 | trades.funds | string | 체결 총액 |
| 21.6 | trades.trend | string | 체결 시세 흐름 (up/down) |
| 21.7 | trades.created_at | string | 체결 시각 |
| 21.8 | trades.side | string | 체결 방향 (ask/bid) |

---

### 2.6 체결 대기 주문 목록 조회

| 항목 | 값 |
|------|-----|
| Method | `GET` |
| URL | `/v1/orders/open` |
| 설명 | 체결 대기 중인 주문 목록 조회 |
| Rate Limit | 초당 30회 (Exchange default 그룹) |
| 권한 | 주문조회 |

**입력 파라미터**

| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| market | string | X | 페어 코드 필터 |
| state | string (enum) | X | 주문 상태 필터: `wait`/`watch`. 기본값 wait |
| states[] | array | X | 주문 상태 배열 (state와 동시 사용 불가) |
| page | integer | X | 페이지 번호. 기본값 1 |
| limit | integer | X | 요청 개수. 기본값 100, 최대 100 |
| order_by | string (enum) | X | 정렬: `asc`/`desc`. 기본값 desc |

**출력 필드**

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | string | 페어 코드 |
| 2 | uuid | string | 주문 유일 식별자 |
| 3 | side | string | 주문 방향 |
| 4 | ord_type | string | 주문 유형 |
| 5 | price | string | 주문 단가/총액 |
| 6 | state | string | 주문 상태 (wait/watch) |
| 7 | created_at | string | 주문 생성 시각 |
| 8 | volume | string | 주문 수량 |
| 9 | remaining_volume | string | 잔여 수량 |
| 10 | executed_volume | string | 체결 수량 |
| 11 | executed_funds | string | 체결 금액 |
| 12 | reserved_fee | string | 예약 수수료 |
| 13 | remaining_fee | string | 남은 수수료 |
| 14 | paid_fee | string | 사용 수수료 |
| 15 | locked | string | 사용 중 비용 |
| 16 | time_in_force | string | 체결 옵션 |
| 17 | identifier | string | 클라이언트 주문 식별자 |
| 18 | smp_type | string | 자전거래 방지 모드 |
| 19 | prevented_volume | string | 자전거래 방지 취소 수량 |
| 20 | prevented_locked | string | 자전거래 방지 해제 자산 |
| 21 | trades_count | integer | 체결 건수 |

---

### 2.7 종료 주문 목록 조회

| 항목 | 값 |
|------|-----|
| Method | `GET` |
| URL | `/v1/orders/closed` |
| 설명 | 종료 주문 목록 조회 (체결 완료 + 취소). 최대 7일 구간 |
| Rate Limit | 초당 30회 (Exchange default 그룹) |
| 권한 | 주문조회 |

**입력 파라미터**

| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| market | string | X | 페어 코드 필터 |
| state | string (enum) | X | 주문 상태: `done,cancel`/`done`/`cancel`. 기본값 done,cancel |
| states[] | array | X | 주문 상태 배열 (state와 동시 사용 불가) |
| start_time | string | X | 조회 시작 시각 (ISO 8601 또는 ms 타임스탬프) |
| end_time | string | X | 조회 종료 시각 (ISO 8601 또는 ms 타임스탬프) |
| limit | integer | X | 요청 개수. 기본값 100, 최대 1000 |
| order_by | string (enum) | X | 정렬: `asc`/`desc`. 기본값 desc |

**출력 필드**

| No | 필드명 | 타입 | 설명 |
|----|--------|------|------|
| 1 | market | string | 페어 코드 |
| 2 | uuid | string | 주문 유일 식별자 |
| 3 | side | string | 주문 방향 |
| 4 | ord_type | string | 주문 유형 |
| 5 | price | string | 주문 단가/총액 |
| 6 | state | string | 주문 상태 (done/cancel) |
| 7 | created_at | string | 주문 생성 시각 |
| 8 | volume | string | 주문 수량 |
| 9 | remaining_volume | string | 잔여 수량 |
| 10 | executed_volume | string | 체결 수량 |
| 11 | executed_funds | string | 체결 금액 |
| 12 | reserved_fee | string | 예약 수수료 |
| 13 | remaining_fee | string | 남은 수수료 |
| 14 | paid_fee | string | 사용 수수료 |
| 15 | locked | string | 사용 중 비용 |
| 16 | time_in_force | string | 체결 옵션 |
| 17 | identifier | string | 클라이언트 주문 식별자 |
| 18 | smp_type | string | 자전거래 방지 모드 |
| 19 | prevented_volume | string | 자전거래 방지 취소 수량 |
| 20 | prevented_locked | string | 자전거래 방지 해제 자산 |
| 21 | trades_count | integer | 체결 건수 |

---

## 3. WebSocket API (6개 타입)

### WebSocket 연결 정보

| 항목 | 값 |
|------|-----|
| Quotation Endpoint | `wss://api.upbit.com/websocket/v1` |
| Exchange Endpoint | `wss://api.upbit.com/websocket/v1/private` |
| 인증 | Exchange: Authorization: Bearer {JWT} 헤더 필수 |
| Idle Timeout | 120초 (PING/PONG으로 연결 유지) |
| 데이터 포맷 | DEFAULT / SIMPLE (축약형) / JSON_LIST / SIMPLE_LIST |

### 요청 메시지 형식

```json
[
  { "ticket": "UUID-고유값" },
  { "type": "ticker", "codes": ["KRW-BTC","KRW-ETH"] },
  { "format": "DEFAULT" }
]
```

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| ticket | String | O | 요청 티켓 고유 식별자 (UUID) |
| type | String | O | 데이터 타입: ticker/trade/orderbook/candle.{unit}/myOrder/myAsset |
| codes | List | 조건부 | 페어 목록 (Quotation 필수, myOrder 선택, myAsset 미지원) |
| level | Double | X | 호가 모아보기 단위 (orderbook만) |
| is_only_snapshot | Boolean | X | 스냅샷만 수신. 기본 false |
| is_only_realtime | Boolean | X | 실시간만 수신. 기본 false |
| format | String | O | 포맷: DEFAULT/SIMPLE/JSON_LIST/SIMPLE_LIST |

---

### 3.1 현재가 (Ticker)

| 항목 | 값 |
|------|-----|
| type | `ticker` |
| Endpoint | Quotation (`wss://api.upbit.com/websocket/v1`) |
| 인증 | 불필요 |

**구독 데이터 명세**

| No | 필드명 | 축약형 | 타입 | 설명 |
|----|--------|--------|------|------|
| 1 | type | ty | String | `ticker` |
| 2 | code | cd | String | 페어 코드 (KRW-BTC) |
| 3 | opening_price | op | Double | 시가 |
| 4 | high_price | hp | Double | 고가 |
| 5 | low_price | lp | Double | 저가 |
| 6 | trade_price | tp | Double | 현재가 |
| 7 | prev_closing_price | pcp | Double | 전일 종가 |
| 8 | change | c | String | 변동 방향 (RISE/EVEN/FALL) |
| 9 | change_price | cp | Double | 전일 대비 가격 변동 절대값 |
| 10 | signed_change_price | scp | Double | 전일 대비 가격 변동 (부호) |
| 11 | change_rate | cr | Double | 전일 대비 등락률 절대값 |
| 12 | signed_change_rate | scr | Double | 전일 대비 등락률 (부호) |
| 13 | trade_volume | tv | Double | 최근 거래량 |
| 14 | acc_trade_volume | atv | Double | 누적 거래량 (UTC 0시) |
| 15 | acc_trade_volume_24h | atv24h | Double | 24시간 누적 거래량 |
| 16 | acc_trade_price | atp | Double | 누적 거래대금 (UTC 0시) |
| 17 | acc_trade_price_24h | atp24h | Double | 24시간 누적 거래대금 |
| 18 | trade_date | tdt | String | 최근 거래 일자 (UTC, yyyyMMdd) |
| 19 | trade_time | ttm | String | 최근 거래 시각 (UTC, HHmmss) |
| 20 | trade_timestamp | ttms | Long | 체결 타임스탬프 (ms) |
| 21 | ask_bid | ab | String | 매수/매도 (ASK/BID) |
| 22 | acc_ask_volume | aav | Double | 누적 매도량 |
| 23 | acc_bid_volume | abv | Double | 누적 매수량 |
| 24 | highest_52_week_price | h52wp | Double | 52주 최고가 |
| 25 | highest_52_week_date | h52wdt | String | 52주 최고가 달성일 |
| 26 | lowest_52_week_price | l52wp | Double | 52주 최저가 |
| 27 | lowest_52_week_date | l52wdt | String | 52주 최저가 달성일 |
| 28 | market_state | ms | String | 거래상태 (PREVIEW/ACTIVE/DELISTED) |
| 29 | is_trading_suspended | its | Boolean | 거래 정지 여부 (Deprecated) |
| 30 | delisting_date | dd | Date | 거래지원 종료일 |
| 31 | market_warning | mw | String | 유의 종목 (NONE/CAUTION, Deprecated) |
| 32 | timestamp | tms | Long | 타임스탬프 (ms) |
| 33 | stream_type | st | String | 스트림 타입 (SNAPSHOT/REALTIME) |

---

### 3.2 체결 (Trade)

| 항목 | 값 |
|------|-----|
| type | `trade` |
| Endpoint | Quotation (`wss://api.upbit.com/websocket/v1`) |
| 인증 | 불필요 |

**구독 데이터 명세**

| No | 필드명 | 축약형 | 타입 | 설명 |
|----|--------|--------|------|------|
| 1 | type | ty | String | `trade` |
| 2 | code | cd | String | 페어 코드 |
| 3 | trade_price | tp | Double | 체결 가격 |
| 4 | trade_volume | tv | Double | 체결량 |
| 5 | ask_bid | ab | String | 매수/매도 (ASK/BID) |
| 6 | prev_closing_price | pcp | Double | 전일 종가 |
| 7 | change | c | String | 변동 방향 (RISE/EVEN/FALL) |
| 8 | change_price | cp | Double | 전일 대비 가격 변동 절대값 |
| 9 | trade_date | td | String | 체결 일자 (UTC, yyyy-MM-dd) |
| 10 | trade_time | ttm | String | 체결 시각 (UTC, HH:mm:ss) |
| 11 | trade_timestamp | ttms | Long | 체결 타임스탬프 (ms) |
| 12 | timestamp | tms | Long | 타임스탬프 (ms) |
| 13 | sequential_id | sid | Long | 체결 번호 (Unique) |
| 14 | best_ask_price | bap | Double | 최우선 매도 호가 |
| 15 | best_ask_size | bas | Double | 최우선 매도 잔량 |
| 16 | best_bid_price | bbp | Double | 최우선 매수 호가 |
| 17 | best_bid_size | bbs | Double | 최우선 매수 잔량 |
| 18 | stream_type | st | String | 스트림 타입 (SNAPSHOT/REALTIME) |

---

### 3.3 호가 (Orderbook)

| 항목 | 값 |
|------|-----|
| type | `orderbook` |
| Endpoint | Quotation (`wss://api.upbit.com/websocket/v1`) |
| 인증 | 불필요 |

**요청 시 추가 옵션**
- `level`: 모아보기 단위 (KRW 마켓만)
- 호가 개수: codes에 `.{unit}` 추가 (예: `KRW-BTC.15`). 지원: 1, 5, 15, 30 (기본 30)

**구독 데이터 명세**

| No | 필드명 | 축약형 | 타입 | 설명 |
|----|--------|--------|------|------|
| 1 | type | ty | String | `orderbook` |
| 2 | code | cd | String | 페어 코드 |
| 3 | total_ask_size | tas | Double | 매도 총 잔량 |
| 4 | total_bid_size | tbs | Double | 매수 총 잔량 |
| 5 | orderbook_units | obu | List | 호가 배열 |
| 5.1 | orderbook_units.ask_price | obu.ap | Double | 매도 호가 |
| 5.2 | orderbook_units.bid_price | obu.bp | Double | 매수 호가 |
| 5.3 | orderbook_units.ask_size | obu.as | Double | 매도 잔량 |
| 5.4 | orderbook_units.bid_size | obu.bs | Double | 매수 잔량 |
| 6 | timestamp | tms | Long | 타임스탬프 (ms) |
| 7 | level | lv | Double | 모아보기 단위 (기본 0) |
| 8 | stream_type | st | String | 스트림 타입 (SNAPSHOT/REALTIME) |

---

### 3.4 캔들 (Candle)

| 항목 | 값 |
|------|-----|
| type | `candle.{unit}` (candle.1s, candle.1m, candle.3m, candle.5m, candle.10m, candle.15m, candle.30m, candle.60m, candle.240m) |
| Endpoint | Quotation (`wss://api.upbit.com/websocket/v1`) |
| 인증 | 불필요 |
| 전송 주기 | 1초 (체결 발생 시에만) |

**구독 데이터 명세**

| No | 필드명 | 축약형 | 타입 | 설명 |
|----|--------|--------|------|------|
| 1 | type | ty | String | candle.1s/1m/3m/5m/10m/15m/30m/60m/240m |
| 2 | code | cd | String | 페어 코드 |
| 3 | candle_date_time_utc | cdttmu | String | 캔들 시작 시각 (UTC) |
| 4 | candle_date_time_kst | cdttmk | String | 캔들 시작 시각 (KST) |
| 5 | opening_price | op | Double | 시가 |
| 6 | high_price | hp | Double | 고가 |
| 7 | low_price | lp | Double | 저가 |
| 8 | trade_price | tp | Double | 종가 |
| 9 | candle_acc_trade_volume | catv | Double | 누적 거래량 |
| 10 | candle_acc_trade_price | catp | Double | 누적 거래금액 |
| 11 | timestamp | tms | Long | 타임스탬프 (ms) |
| 12 | stream_type | st | String | 스트림 타입 (SNAPSHOT/REALTIME) |

---

### 3.5 내 주문 및 체결 (MyOrder)

| 항목 | 값 |
|------|-----|
| type | `myOrder` |
| Endpoint | Exchange (`wss://api.upbit.com/websocket/v1/private`) |
| 인증 | JWT Bearer 필수 (주문조회 권한) |
| codes | 선택 (생략 시 전체 마켓 구독) |

**구독 데이터 명세**

| No | 필드명 | 축약형 | 타입 | 설명 |
|----|--------|--------|------|------|
| 1 | type | ty | String | `myOrder` |
| 2 | code | cd | String | 페어 코드 |
| 3 | uuid | uid | String | 주문 유일 식별자 |
| 4 | ask_bid | ab | String | 매수/매도 (ASK/BID) |
| 5 | order_type | ot | String | 주문 타입 (limit/price/market/best) |
| 6 | state | s | String | 주문 상태 (wait/watch/trade/done/cancel/prevented) |
| 7 | trade_uuid | tuid | String | 체결 유일 식별자 |
| 8 | price | p | Double | 주문/체결 가격 |
| 9 | avg_price | ap | Double | 평균 체결 가격 |
| 10 | volume | v | Double | 주문량/체결량 (state:trade 시 체결량) |
| 11 | remaining_volume | rv | Double | 주문 잔량 |
| 12 | executed_volume | ev | Double | 체결 수량 |
| 13 | trades_count | tc | Integer | 체결 수 |
| 14 | reserved_fee | rsf | Double | 예약 수수료 |
| 15 | remaining_fee | rmf | Double | 남은 수수료 |
| 16 | paid_fee | pf | Double | 사용 수수료 |
| 17 | locked | l | Double | 사용 중 비용 |
| 18 | executed_funds | ef | Double | 체결 금액 |
| 19 | time_in_force | tif | String | 체결 조건 (ioc/fok/post_only) |
| 20 | trade_fee | tf | Double | 체결 시 수수료 (state:trade 아닐 때 null) |
| 21 | is_maker | im | Boolean | 메이커 여부 (state:trade 아닐 때 null) |
| 22 | identifier | id | String | 클라이언트 주문 식별자 |
| 23 | smp_type | smpt | String | 자전거래 방지 (reduce/cancel_maker/cancel_taker) |
| 24 | prevented_volume | pv | Double | 자전거래 방지 취소 수량 |
| 25 | prevented_locked | pl | Double | 자전거래 방지 해제 자산 |
| 26 | trade_timestamp | ttms | Long | 체결 타임스탬프 (ms) |
| 27 | order_timestamp | otms | Long | 주문 타임스탬프 (ms) |
| 28 | timestamp | tms | Long | 타임스탬프 (ms) |
| 29 | stream_type | st | String | 스트림 타입 (REALTIME/SNAPSHOT) |

---

### 3.6 내 자산 (MyAsset)

| 항목 | 값 |
|------|-----|
| type | `myAsset` |
| Endpoint | Exchange (`wss://api.upbit.com/websocket/v1/private`) |
| 인증 | JWT Bearer 필수 (자산조회 권한) |
| codes | 미지원 (codes 포함 시 WRONG_FORMAT 에러) |
| 전송 조건 | 자산 변동 발생 시에만 전송 |

**구독 데이터 명세**

| No | 필드명 | 축약형 | 타입 | 설명 |
|----|--------|--------|------|------|
| 1 | type | ty | String | `myAsset` |
| 2 | asset_uuid | astuid | String | 자산 고유 식별자 |
| 3 | assets | ast | List | 자산 목록 |
| 3.1 | assets.currency | ast.cu | String | 화폐 코드 |
| 3.2 | assets.balance | ast.b | Double | 주문가능 수량 |
| 3.3 | assets.locked | ast.l | Double | 주문 중 묶인 수량 |
| 4 | asset_timestamp | asttms | Long | 자산 타임스탬프 (ms) |
| 5 | timestamp | tms | Long | 타임스탬프 (ms) |
| 6 | stream_type | st | String | 스트림 타입 (REALTIME) |

---

## API 요약 테이블

### REST API

| No | 카테고리 | API명 | Method | URL | 인증 |
|----|----------|-------|--------|-----|------|
| 1 | Quotation | 페어 목록 조회 | GET | /v1/market/all | X |
| 2 | Quotation | 현재가 조회 | GET | /v1/ticker | X |
| 3 | Quotation | 호가 조회 | GET | /v1/orderbook | X |
| 4 | Quotation | 최근 체결 내역 | GET | /v1/trades/ticks | X |
| 5 | Quotation | 초 캔들 조회 | GET | /v1/candles/seconds | X |
| 6 | Quotation | 분 캔들 조회 | GET | /v1/candles/minutes/{unit} | X |
| 7 | Exchange | 잔고 조회 | GET | /v1/accounts | O (자산조회) |
| 8 | Exchange | 주문 가능 정보 | GET | /v1/orders/chance | O (주문조회) |
| 9 | Exchange | 주문 생성 | POST | /v1/orders | O (주문하기) |
| 10 | Exchange | 주문 취소 | DELETE | /v1/order | O (주문하기) |
| 11 | Exchange | 주문 조회 | GET | /v1/order | O (주문조회) |
| 12 | Exchange | 체결대기 주문 | GET | /v1/orders/open | O (주문조회) |
| 13 | Exchange | 종료 주문 | GET | /v1/orders/closed | O (주문조회) |

### WebSocket API

| No | 타입 | Endpoint | 인증 | 설명 |
|----|------|----------|------|------|
| 1 | ticker | Quotation | X | 현재가 실시간 스트림 (33 필드) |
| 2 | trade | Quotation | X | 체결 실시간 스트림 (18 필드) |
| 3 | orderbook | Quotation | X | 호가 실시간 스트림 (최대 30단계) |
| 4 | candle.{unit} | Quotation | X | 캔들 실시간 스트림 (초/분봉) |
| 5 | myOrder | Exchange | O | 내 주문/체결 실시간 (29 필드) |
| 6 | myAsset | Exchange | O | 내 자산 변동 실시간 (6 필드) |
