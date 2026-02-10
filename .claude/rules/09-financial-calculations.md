# 금융 계산 규칙

## 1. Decimal 타입 사용 필수

> f64 사용 금지: 금액, 가격, 수량 계산에는 반드시 `rust_decimal::Decimal` 사용

```rust
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// ❌ 부동소수점 오차 발생
let price: f64 = 0.1 + 0.2;  // 0.30000000000000004

// ✅ Decimal 사용
let price = dec!(0.1) + dec!(0.2);  // 정확히 0.3
```

**적용 대상**: 주문 가격, 수량, 잔고, 손익 계산, 수수료

## 2. 타임스탬프 UTC 강제

```rust
use chrono::{DateTime, Utc};

// ✅ 모든 시간은 UTC로 저장
let timestamp: DateTime<Utc> = Utc::now();

// ❌ 로컬 타임존 사용 금지
let local = Local::now();
```

## 3. Idempotency (멱등성) 보장

> 주문 API는 중복 실행 시 동일한 결과를 보장해야 합니다.

```rust
pub async fn place_order(
    pool: &PgPool,
    request_id: &str,  // 클라이언트가 제공하는 고유 ID
    order: &OrderRequest
) -> Result<OrderId, OrderError> {
    // 이미 처리된 request_id인지 확인
    if let Some(existing) = find_by_request_id(pool, request_id).await? {
        return Ok(existing.order_id);  // 중복 요청, 기존 결과 반환
    }
    // 새 주문 처리
    let order_id = create_order(pool, order).await?;
    save_request_id(pool, request_id, &order_id).await?;
    Ok(order_id)
}
```

## 4. NewType 패턴으로 타입 안전성

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StrategyId(pub String);

// ✅ 컴파일 타임에 타입 검증
fn cancel_order(order_id: OrderId) { ... }
cancel_order(strategy_id);  // 컴파일 에러!
```
