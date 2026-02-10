# API 라우트 참조 (에이전트용)

> 원본: `docs/api.md` (835줄) | 담당: `rust-impl`, `ts-impl`, `debugger`
> Base URL: `http://localhost:3000` | Auth: JWT Bearer

## 인증

```
POST /api/v1/auth/login     → { access_token, refresh_token, expires_in }
POST /api/v1/auth/refresh   → { access_token }
POST /api/v1/auth/logout
```

Roles: `admin`(100) > `trader`(50) > `viewer`(10)

## 라우트 맵

### Health
```
GET /health          → { status, timestamp }
GET /health/ready    → { status, components: { database, redis, exchange } }
```

### Strategies
```
GET    /api/v1/strategies              → 전략 목록
GET    /api/v1/strategies/:id          → 전략 상세
POST   /api/v1/strategies              → 전략 생성
PUT    /api/v1/strategies/:id          → 전략 수정
DELETE /api/v1/strategies/:id          → 전략 삭제
POST   /api/v1/strategies/:id/start    → 전략 시작
POST   /api/v1/strategies/:id/stop     → 전략 중지
PUT    /api/v1/strategies/:id/config   → 설정 변경
GET    /api/v1/strategies/stats        → 엔진 통계
```

### Orders
```
GET    /api/v1/orders          → 활성 주문 목록
GET    /api/v1/orders/:id      → 주문 상세
DELETE /api/v1/orders/:id      → 주문 취소
GET    /api/v1/orders/stats    → 주문 통계
GET    /api/v1/orders/history  → 체결 내역
```

### Positions
```
GET    /api/v1/positions          → 포지션 목록
GET    /api/v1/positions/:id      → 포지션 상세
POST   /api/v1/positions/:id/close → 포지션 청산
GET    /api/v1/positions/stats     → 포지션 통계
```

### Backtest
```
POST   /api/v1/backtest/run      → 백테스트 실행
GET    /api/v1/backtest/results  → 결과 목록
GET    /api/v1/backtest/:id      → 결과 상세
```

### Market Data
```
GET    /api/v1/market/symbols         → 심볼 목록
GET    /api/v1/market/ohlcv/:symbol   → OHLCV 데이터
GET    /api/v1/market/quote/:symbol   → 실시간 시세
```

### Watchlist
```
GET    /api/v1/watchlist       → 관심 종목 목록
POST   /api/v1/watchlist       → 종목 추가
DELETE /api/v1/watchlist/:id   → 종목 삭제
```

### Alerts
```
GET    /api/v1/alerts          → 알림 규칙 목록
POST   /api/v1/alerts          → 알림 규칙 생성
PUT    /api/v1/alerts/:id      → 알림 수정
DELETE /api/v1/alerts/:id      → 알림 삭제
GET    /api/v1/alerts/history  → 알림 이력
```

### Analytics
```
GET    /api/v1/analytics/global-score     → GlobalScore 조회
GET    /api/v1/analytics/market-breadth   → MarketBreadth
GET    /api/v1/analytics/screening        → 스크리닝 결과
```

### Settings
```
GET    /api/v1/settings            → 설정 조회
PUT    /api/v1/settings            → 설정 변경
GET    /api/v1/settings/exchanges  → 거래소 설정
PUT    /api/v1/settings/exchanges/:id → 거래소 설정 변경
```

### WebSocket
```
WS /ws/market       → 실시간 시세 스트림
WS /ws/orders       → 주문 상태 스트림
WS /ws/signals      → 전략 시그널 스트림
WS /ws/alerts       → 알림 스트림
```

## 핸들러 패턴 (rust-impl 참조)

```rust
// 위치: crates/trader-api/src/routes/<도메인>.rs

async fn handler(
    State(state): State<AppState>,
    // 선택: Path, Query, Json 추출자
) -> Result<Json<Response>, ApiErrorResponse> {
    // 1. 입력 검증
    // 2. Repository 호출
    // 3. 응답 변환
}
```

## 프론트엔드 API 클라이언트 (ts-impl 참조)

```typescript
// 위치: frontend/src/api/client.ts
// 타입: frontend/src/api/types/generated/ (ts-rs 자동 생성)
import { createResource } from "solid-js";
const [data] = createResource(() => api.get("/api/v1/strategies"));
```
