# 모니터링 및 로깅

> **원칙**: 프로젝트 특성상 과도한 모니터링 시스템 지양. 실용적이고 간소화된 접근 방식 사용.

## 1. 구조화 로깅 (tracing)

```rust
use tracing::{info, warn, error, instrument};

#[instrument(skip(pool), fields(symbol = %symbol, quantity = %quantity))]
pub async fn place_market_order(
    pool: &PgPool,
    symbol: &str,
    quantity: Decimal
) -> Result<OrderId> {
    info!("시장가 주문 시작");
    match execute_order(pool, symbol, quantity).await {
        Ok(order_id) => {
            info!(?order_id, "주문 성공");
            Ok(order_id)
        }
        Err(e) => {
            error!(?e, "주문 실패");
            Err(e)
        }
    }
}
```

### 로그 레벨 기준

| 레벨 | 용도 |
|------|------|
| `error!` | 즉시 대응 필요 (주문 실패, DB 연결 끊김) |
| `warn!` | 주의 필요 (API 재시도, 비정상 데이터) |
| `info!` | 주요 이벤트 (주문 체결, 전략 시작/중지) |
| `debug!` | 디버깅 정보 (파라미터 값, 중간 계산) |
| `trace!` | 상세 추적 (루프 내부, 모든 함수 호출) |

### 로그 레벨 제어

```bash
RUST_LOG=info,trader_api=info
RUST_LOG=debug,trader_strategy=debug  # 특정 모듈 상세 로깅
```

## 2. 헬스체크

- `/health` 엔드포인트로 기본 상태 확인
- `/health/ready` 엔드포인트로 컴포넌트 상태 확인 (DB, Redis, Exchange)

```bash
curl http://localhost:3000/health/ready
```

## 3. 알림 (Telegram/Discord)

**웹 UI에서 관리** (암호화 저장): 봇 토큰, Chat ID 설정, 테스트 메시지 전송 기능

### 알림 대상

- ❌ 에러: 주문 실패, DB 연결 끊김, API 키 만료
- ⚠️ 경고: 잔고 부족, 리스크 한계 도달
- ✅ 정보: 일일 손익 보고, 전략 성과 요약

> 상세 가이드: `docs/operations.md` 참조
