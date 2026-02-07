# Trader API Documentation

## Overview

Multi-Market Trading Bot REST API 및 WebSocket 서버 문서입니다.

**Base URL:** `http://localhost:3000`
**API Version:** v1

---

## Authentication

### JWT Token

모든 보호된 엔드포인트는 JWT Bearer 토큰을 요구합니다.

```
Authorization: Bearer <token>
```

### Roles

| Role | Level | Description |
|------|-------|-------------|
| `admin` | 100 | 모든 권한 (시스템 관리 포함) |
| `trader` | 50 | 거래 및 전략 관리 |
| `viewer` | 10 | 읽기 전용 |

### Token Endpoints

#### POST /api/v1/auth/login
로그인 및 토큰 발급

**Request:**
```json
{
  "username": "string",
  "password": "string"
}
```

**Response:**
```json
{
  "access_token": "string",
  "refresh_token": "string",
  "expires_in": 1800,
  "token_type": "Bearer"
}
```

---

## Health Check

### GET /health
Liveness check

**Response:** `200 OK`
```json
{
  "status": "healthy",
  "timestamp": "2026-01-28T12:00:00Z"
}
```

### GET /health/ready
Readiness check (모든 컴포넌트 상태)

**Response:**
```json
{
  "status": "healthy",
  "components": {
    "database": { "status": "healthy", "latency_ms": 5 },
    "redis": { "status": "healthy", "latency_ms": 2 },
    "exchange": { "status": "healthy" }
  },
  "timestamp": "2026-01-28T12:00:00Z"
}
```

---

## Strategies API

### GET /api/v1/strategies
전략 목록 조회

**Response:**
```json
{
  "strategies": [
    {
      "id": "grid_btc",
      "name": "BTC Grid Trading",
      "version": "1.0.0",
      "running": true,
      "signals_generated": 150
    }
  ],
  "total": 1,
  "running": 1
}
```

### GET /api/v1/strategies/:id
특정 전략 상세 조회

**Response:**
```json
{
  "id": "grid_btc",
  "name": "BTC Grid Trading",
  "version": "1.0.0",
  "running": true,
  "stats": {
    "signals_generated": 150,
    "orders_submitted": 120,
    "orders_filled": 115
  }
}
```

### POST /api/v1/strategies/:id/start
전략 시작

**Response:**
```json
{
  "success": true,
  "strategy_id": "grid_btc",
  "action": "start",
  "message": "Strategy 'grid_btc' started successfully"
}
```

### POST /api/v1/strategies/:id/stop
전략 중지

### PUT /api/v1/strategies/:id/config
전략 설정 변경

**Request:**
```json
{
  "config": {
    "grid_spacing_pct": 1.5,
    "grid_levels": 10
  }
}
```

### GET /api/v1/strategies/stats
엔진 통계 조회

**Response:**
```json
{
  "total_strategies": 5,
  "running_strategies": 2,
  "total_signals_generated": 500,
  "total_orders_filled": 450,
  "total_market_data_processed": 100000
}
```

---

## Orders API

### GET /api/v1/orders
활성 주문 목록 조회

**Response:**
```json
{
  "orders": [
    {
      "id": "uuid",
      "exchange_order_id": "12345",
      "symbol": "BTC/USDT",
      "side": "Buy",
      "order_type": "Limit",
      "quantity": "0.1",
      "filled_quantity": "0.05",
      "price": "50000",
      "average_fill_price": "49950",
      "status": "PartiallyFilled",
      "strategy_id": "grid_btc",
      "created_at": "2026-01-28T12:00:00Z",
      "updated_at": "2026-01-28T12:05:00Z"
    }
  ],
  "total": 1
}
```

### GET /api/v1/orders/:id
특정 주문 상세 조회

### DELETE /api/v1/orders/:id
주문 취소

**Request Body (optional):**
```json
{
  "reason": "Manual cancellation"
}
```

**Response:**
```json
{
  "success": true,
  "order_id": "uuid",
  "message": "Order cancelled successfully"
}
```

### GET /api/v1/orders/stats
주문 통계

**Response:**
```json
{
  "total": 10,
  "by_status": {
    "pending": 2,
    "open": 5,
    "partially_filled": 3
  },
  "by_side": {
    "buy": 6,
    "sell": 4
  }
}
```

---

## Positions API

### GET /api/v1/positions
열린 포지션 목록 조회

**Response:**
```json
{
  "positions": [
    {
      "id": "uuid",
      "exchange": "binance",
      "symbol": "BTC/USDT",
      "side": "Buy",
      "quantity": "0.5",
      "entry_price": "50000",
      "current_price": "51000",
      "unrealized_pnl": "500",
      "realized_pnl": "0",
      "notional_value": "25500",
      "return_pct": "2.0",
      "strategy_id": "grid_btc",
      "opened_at": "2026-01-28T10:00:00Z",
      "updated_at": "2026-01-28T12:00:00Z"
    }
  ],
  "total": 1,
  "summary": {
    "total_positions": 1,
    "total_unrealized_pnl": "500",
    "total_realized_pnl": "0",
    "total_notional_value": "25500",
    "long_count": 1,
    "short_count": 0
  }
}
```

### GET /api/v1/positions/:symbol
특정 심볼 포지션 조회

### GET /api/v1/positions/summary
포지션 요약 통계

---

## WebSocket API

**Endpoint:** `ws://localhost:3000/ws`

### Connection Flow

1. WebSocket 연결 수립
2. 서버에서 `welcome` 메시지 전송
3. 클라이언트에서 `auth` 메시지로 인증 (선택)
4. 클라이언트에서 `subscribe` 메시지로 채널 구독
5. 서버에서 실시간 데이터 스트리밍

### Client → Server Messages

#### Subscribe
```json
{
  "type": "subscribe",
  "channels": ["market:BTC-USDT", "orders", "positions"]
}
```

#### Unsubscribe
```json
{
  "type": "unsubscribe",
  "channels": ["market:BTC-USDT"]
}
```

#### Ping
```json
{
  "type": "ping"
}
```

#### Auth
```json
{
  "type": "auth",
  "token": "jwt_token_here"
}
```

### Server → Client Messages

#### Welcome
```json
{
  "type": "welcome",
  "version": "0.1.0",
  "timestamp": 1706436000000
}
```

#### Subscribed
```json
{
  "type": "subscribed",
  "channels": ["market:BTC-USDT", "orders"]
}
```

#### Pong
```json
{
  "type": "pong",
  "timestamp": 1706436000000
}
```

#### Ticker
```json
{
  "type": "ticker",
  "symbol": "BTC-USDT",
  "price": "50000.00",
  "change_24h": "2.5",
  "volume_24h": "1000000",
  "high_24h": "51000",
  "low_24h": "49000",
  "timestamp": 1706436000000
}
```

#### Trade
```json
{
  "type": "trade",
  "symbol": "BTC-USDT",
  "trade_id": "123456",
  "price": "50000.00",
  "quantity": "0.1",
  "side": "Buy",
  "timestamp": 1706436000000
}
```

#### Order Update
```json
{
  "type": "order_update",
  "order_id": "uuid",
  "symbol": "BTC/USDT",
  "status": "Filled",
  "side": "Buy",
  "order_type": "Limit",
  "quantity": "0.1",
  "filled_quantity": "0.1",
  "price": "50000",
  "average_price": "49990",
  "timestamp": 1706436000000
}
```

#### Position Update
```json
{
  "type": "position_update",
  "symbol": "BTC/USDT",
  "side": "Buy",
  "quantity": "0.5",
  "entry_price": "50000",
  "current_price": "51000",
  "unrealized_pnl": "500",
  "return_pct": "2.0",
  "timestamp": 1706436000000
}
```

#### Strategy Update
```json
{
  "type": "strategy_update",
  "strategy_id": "grid_btc",
  "name": "BTC Grid Trading",
  "running": true,
  "event": "signal_generated",
  "data": { "signal_type": "Entry", "side": "Buy" },
  "timestamp": 1706436000000
}
```

#### Error
```json
{
  "type": "error",
  "code": "INVALID_CHANNEL",
  "message": "Unknown channel: xyz"
}
```

### Subscription Channels

| Channel | Description |
|---------|-------------|
| `market:{symbol}` | 특정 심볼의 시장 데이터 (ticker, trades) |
| `orders` | 주문 상태 업데이트 |
| `positions` | 포지션 업데이트 |
| `strategies` | 전략 상태 변경 |
| `all_markets` | 모든 시장 요약 데이터 |

---

## Error Responses

모든 에러는 동일한 형식으로 반환됩니다:

```json
{
  "code": "ERROR_CODE",
  "message": "Human readable error message"
}
```

### Common Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `MISSING_TOKEN` | 401 | 인증 토큰 누락 |
| `INVALID_TOKEN` | 401 | 유효하지 않은 토큰 |
| `TOKEN_EXPIRED` | 401 | 토큰 만료 |
| `INSUFFICIENT_PERMISSION` | 403 | 권한 부족 |
| `STRATEGY_NOT_FOUND` | 404 | 전략을 찾을 수 없음 |
| `ORDER_NOT_FOUND` | 404 | 주문을 찾을 수 없음 |
| `POSITION_NOT_FOUND` | 404 | 포지션을 찾을 수 없음 |
| `INVALID_ORDER_ID` | 400 | 잘못된 주문 ID 형식 |
| `ALREADY_RUNNING` | 400 | 전략이 이미 실행 중 |
| `NOT_RUNNING` | 400 | 전략이 실행 중이 아님 |
| `CANCEL_FAILED` | 500 | 주문 취소 실패 |

---

## Rate Limiting

- REST API: 1200 requests/minute
- WebSocket: 100 messages/second

Rate limit 초과 시 `429 Too Many Requests` 반환

---

## Testing

### Unit Tests
```bash
cargo test --workspace
```

### Integration Tests
```bash
cargo test --workspace --test '*'
```

### Current Test Status
- **Unit Tests:** 258 (258 passed)
- **Integration Tests:** 28 (28 passed)
- **Total:** 286 tests passing

---

---

## Multi Timeframe API

### GET /api/v1/market/klines/multi
다중 타임프레임 Kline 데이터 조회

**Query Parameters:**
| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| symbol | string | ✓ | 심볼 (예: "005930") |
| timeframes | string | ✓ | 타임프레임 목록 (쉼표 구분: "1m,5m,1h") |
| limit | number | | 각 타임프레임당 캔들 수 (기본: 100) |

**Response:**
```json
{
  "symbol": "005930",
  "data": {
    "1m": [{ "timestamp": 1706436000, "open": 50000, ... }],
    "5m": [...],
    "1h": [...]
  }
}
```

---

## Ranking API

### GET /api/v1/ranking
글로벌 스코어 랭킹 조회

**Query Parameters:**
| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| market | string | | 시장 필터 (KR, US, CRYPTO) |
| route_state | string | | RouteState 필터 (ATTACK, ARMED, WAIT) |
| limit | number | | 결과 수 (기본: 100) |

### GET /api/v1/ranking/7factor/{ticker}
7Factor 스코어 조회

**Response:**
```json
{
  "ticker": "005930",
  "factors": {
    "momentum": 75.5,
    "value": 62.3,
    "quality": 88.1,
    "volatility": 45.2,
    "liquidity": 92.0,
    "growth": 55.8,
    "sentiment": 70.0
  },
  "total_score": 72.4
}
```

### POST /api/v1/ranking/7factor/batch
7Factor 배치 조회

**Request:**
```json
{
  "tickers": ["005930", "000660", "035720"]
}
```

---

## Watchlist API

### GET /api/v1/watchlist
관심종목 목록 조회

### POST /api/v1/watchlist
관심종목 그룹 생성

**Request:**
```json
{
  "name": "반도체 관련주",
  "description": "반도체 섹터 관심종목"
}
```

### POST /api/v1/watchlist/{id}/items
관심종목 추가

**Request:**
```json
{
  "symbol": "005930"
}
```

### DELETE /api/v1/watchlist/{id}/items/{symbol}
관심종목 삭제

---

## Journal API

### GET /api/v1/journal/positions
보유 포지션 목록 조회

### GET /api/v1/journal/executions
체결 내역 조회

**Query Parameters:**
| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| from | string | | 시작 날짜 (YYYY-MM-DD) |
| to | string | | 종료 날짜 (YYYY-MM-DD) |
| symbol | string | | 심볼 필터 |
| side | string | | 매수/매도 필터 (buy, sell) |

### GET /api/v1/journal/cost-basis/{symbol}
FIFO 원가 계산 조회

**Response:**
```json
{
  "symbol": "005930",
  "average_cost": "65000",
  "total_cost": "650000",
  "realized_pnl": "15000",
  "lots": [
    { "quantity": 5, "cost": "64000", "date": "2026-01-15" },
    { "quantity": 5, "cost": "66000", "date": "2026-01-20" }
  ]
}
```

---

## Monitoring API

### GET /api/v1/monitoring/errors
에러 목록 조회

**Query Parameters:**
| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| severity | string | | 심각도 필터 (critical, error, warning) |
| category | string | | 카테고리 필터 (database, exchange, strategy) |

### GET /api/v1/monitoring/errors/critical
Critical 에러만 조회

### GET /api/v1/monitoring/stats
에러 통계 (심각도별/카테고리별)

### GET /api/v1/monitoring/summary
시스템 요약 (디버깅용)

---

## Symbols API

### GET /api/v1/symbols/failed
수집 실패한 심볼 목록 조회 (3회 연속 실패 시 자동 비활성화)

### POST /api/v1/symbols/{symbol}/reactivate
비활성화된 심볼 재활성화

### POST /api/v1/symbols/sync
심볼 동기화 실행

---

## Dataset API

### GET /api/v1/dataset
캐시된 데이터셋 목록 조회

### POST /api/v1/dataset/fetch
새 데이터셋 다운로드 요청

### GET /api/v1/dataset/search
심볼 검색 (티커/이름 패턴 매칭)

### POST /api/v1/dataset/symbols/batch
여러 티커의 심볼 정보 일괄 조회

### GET /api/v1/dataset/symbols/failed
수집 실패한 심볼 목록 조회

### GET /api/v1/dataset/symbols/stats
심볼 상태 통계 (비활성화/임계/경고)

### POST /api/v1/dataset/symbols/reactivate
비활성화된 심볼 재활성화

### DELETE /api/v1/dataset/symbols/{ticker}
심볼 삭제 + 관련 데이터 연쇄 정리 (delete_symbol_cascade DB 함수 호출)

**Query Parameters:**
| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| market | string | ✓ | 시장 코드 (KR, US, CRYPTO 등) |

**Response:**
```json
{
  "success": true,
  "ticker": "005930",
  "market": "KR",
  "deletedTables": [
    {"tableName": "ohlcv", "deletedCount": 1200},
    {"tableName": "ohlcv_metadata", "deletedCount": 3}
  ],
  "message": "심볼 '005930' (KR) 삭제 완료 (총 1203건)"
}
```

### POST /api/v1/dataset/symbols/cleanup-orphans
symbol_info에 없는 심볼의 고아 데이터 일괄 정리 (cleanup_orphan_symbol_data DB 함수 호출)

**Response:**
```json
{
  "success": true,
  "cleanedTables": [
    {"tableName": "ohlcv", "deletedCount": 500},
    {"tableName": "execution_cache", "deletedCount": 12}
  ],
  "totalDeleted": 512,
  "message": "고아 데이터 정리 완료 (총 512건 삭제)"
}
```

### GET /api/v1/dataset/{symbol}
특정 심볼의 캔들 데이터 조회 (정렬/페이지네이션 지원)

### DELETE /api/v1/dataset/{symbol}
특정 심볼 캐시 삭제 (ohlcv + ohlcv_metadata)

---

## Screening API

### POST /api/v1/screening/filter
조건 기반 종목 스크리닝

### GET /api/v1/screening/presets
스크리닝 프리셋 목록 조회

### POST /api/v1/screening/refresh-cache
스크리닝 캐시 새로고침 (Materialized View 갱신)

---

## Reality Check API

### GET /api/v1/reality-check/stats
전일 추천 종목의 익일 실제 성과 검증 통계

### GET /api/v1/reality-check/history
검증 이력 조회

**Query Parameters:**
| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| days | number | | 조회 기간 (기본: 30) |

---

## Notifications API

### POST /api/v1/notifications/telegram/test
Telegram 테스트 메시지 전송

---

## WebSocket Extensions

### Kline 브로드캐스트
실시간 캔들 데이터 스트리밍

**Subscribe:**
```json
{
  "type": "subscribe",
  "channels": ["kline:005930:1m", "kline:005930:5m"]
}
```

**Server Message:**
```json
{
  "type": "kline",
  "symbol": "005930",
  "timeframe": "1m",
  "data": {
    "timestamp": 1706436000,
    "open": "50000",
    "high": "50500",
    "low": "49800",
    "close": "50300",
    "volume": "1000000"
  }
}
```

---

## Changelog

### v0.6.0 (2026-02-04)
- Multi Timeframe API 추가
- 7Factor Scoring API 추가
- Watchlist API 추가
- Journal API 확장 (FIFO 원가 계산)
- WebSocket Kline 브로드캐스트 추가
- TypeScript 바인딩 자동 생성 (ts-rs)

### v0.4.4 (2026-01-31)
- OpenAPI/Swagger 문서화 추가
- StrategyType enum 추가
- Repository 패턴 확장 (9개 repository)
- 286 tests passing (258 unit + 28 integration)
