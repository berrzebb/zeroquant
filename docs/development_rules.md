# ZeroQuant 개발 규칙

> 최종 업데이트: 2026-02-04
> 버전: 1.3
> 이 문서는 신규 기능 추가 시 반드시 확인해야 하는 규칙과 고려사항을 정의합니다.

---

## 📋 목차

1. [핵심 원칙](#핵심-원칙)
2. [작업 전 필수 확인](#작업-전-필수-확인)
3. [Rust 백엔드 규칙](#rust-백엔드-규칙)
4. [TypeScript 프론트엔드 규칙](#typescript-프론트엔드-규칙)
5. [데이터베이스 규칙](#데이터베이스-규칙)
6. [API 설계 규칙](#api-설계-규칙)
7. [보안 규칙](#보안-규칙)
8. [테스트 규칙](#테스트-규칙)
9. [금융 계산 규칙](#금융-계산-규칙)
10. [모니터링 및 로깅](#모니터링-및-로깅)
11. [코드 리뷰 체크리스트](#코드-리뷰-체크리스트)
12. [전략 추가 체크리스트](#전략-추가-체크리스트)

---

## 핵심 원칙

> **이 원칙들은 모든 코드 작성 시 최우선으로 고려되어야 합니다.**

### 1. 레거시 코드 즉시 제거

**규칙**: 코드 개선 시 불필요하거나 레거시가 된 코드는 반드시 제거합니다.

```rust
// ❌ 나쁜 예 - 주석 처리된 레거시 코드
// fn old_calculate_price(price: f64) -> f64 {
//     price * 1.1
// }

fn calculate_price_v2(price: Decimal, tax_rate: Decimal) -> Decimal {
    price * (Decimal::ONE + tax_rate)
}

// ✅ 좋은 예 - 레거시 완전 제거, 개선된 함수로 대체
fn calculate_price(price: Decimal, tax_rate: Decimal) -> Decimal {
    price * (Decimal::ONE + tax_rate)
}
```

**기술 부채 방지**:
- 사용되지 않는 함수/타입/모듈은 즉시 삭제
- 주석 처리 대신 Git 히스토리 활용
- 임시 해결책(TODO, FIXME)은 반드시 이슈 등록 후 제거

### 2. 거래소 중립적 코드

**규칙**: 모든 코드는 특정 거래소에 의존하지 않고 추상화된 인터페이스를 사용합니다.

```rust
// ❌ 나쁜 예 - Binance에 강하게 결합
pub async fn get_price(symbol: &str) -> f64 {
    binance_client.get_ticker(symbol).await.price
}

// ✅ 좋은 예 - ExchangeApi trait 사용
pub async fn get_price(
    exchange: &dyn ExchangeApi,
    symbol: &str
) -> Result<Decimal, ExchangeError> {
    exchange.get_ticker(symbol).await
        .map(|ticker| ticker.price)
}
```

**이유**: Binance, KIS, 시뮬레이션 등 다중 거래소 지원을 위해 필수

### 3. 이후 작업 고려

**규칙**: 현재 작업이 향후 확장이나 리팩토링에 미치는 영향을 항상 고려합니다.

- 새 필드 추가 시 마이그레이션 롤백 가능하게 설계
- API 응답 형식 변경 시 버전 관리 고려
- 새 전략 추가 시 백테스트 엔진 확장성 고려

---

## 작업 전 필수 확인

### 라이브러리 API 검증 (Context7 사용)

> **핵심 원칙**: 학습 데이터 기반 추측으로 코드 작성 금지

```
1. resolve-library-id로 라이브러리 ID 획득
2. query-docs로 구체적인 API 패턴 조회
3. 버전 확인: Cargo.toml, package.json
```

**주의해야 할 라이브러리**:
- **Tokio**: select!, spawn, channel API 변경 빈번
- **Axum**: 0.6 → 0.7에서 Router, State API 변경됨
- **SQLx**: query!, query_as! 매크로 동작 확인 필요
- **SolidJS**: reactivity 패턴 확인

**❌ 금지 사항**:

1. **버전 미확인 코드 작성**
   - ❌ "tokio 1.x에서는 이렇게 합니다" (버전 미명시)
   - ✅ "tokio 1.35 기준으로 Context7에서 확인한 패턴입니다"

2. **Deprecated API 사용**
   - ❌ 학습 데이터에 있던 과거 API 사용
   - ✅ 현재 권장 API를 Context7/공식 문서에서 확인 후 사용

3. **추측 기반 import 경로**
   - ❌ `use tokio::something::Maybe;` (존재 여부 불확실)
   - ✅ 실제 코드베이스 또는 docs.rs에서 import 경로 확인

4. **Feature flag 미확인 사용**
   - ❌ tokio의 "full" feature에 포함되어 있을 것으로 가정
   - ✅ Cargo.toml의 features 섹션 확인 후 사용

### 코드 탐색 도구 우선순위

> **핵심 원칙**: 코드베이스 탐색 시 Serena MCP의 semantic tools를 우선 사용

**Serena 우선 사용 (심볼 기반 탐색)**:
```rust
// ✅ 좋은 예 - Serena의 semantic tools 사용
mcp__serena__find_symbol(name_path_pattern="MyStrategy", relative_path="crates/trader-strategy")
mcp__serena__find_referencing_symbols(name_path="calculate_price", relative_path="...")
mcp__serena__get_symbols_overview(relative_path="crates/trader-api/src/routes/strategies.rs")
```

**Grep 제한적 사용 (문자열 패턴 매칭)**:
```bash
# ℹ️ Grep은 다음 경우에만 사용
# 1. 로그 메시지나 에러 메시지 검색
# 2. 특정 문자열 리터럴 찾기
# 3. 정규표현식 패턴 매칭이 필수인 경우
```

**이유**:
- Serena는 클래스, 함수, 메서드 등 심볼 단위로 탐색 가능
- 코드 구조와 의존성을 정확히 파악 가능
- Grep은 단순 텍스트 검색으로 컨텍스트 부족

### UI-API 필드 매칭

> **모든 작업 시 UI와 API 필드 매칭을 반드시 확인**

```typescript
// 프론트엔드 타입과 백엔드 응답이 일치해야 함
interface BacktestResult {
  total_return: number;  // API 응답 필드명
  sharpe_ratio: number;
  // ...
}
```

---

## Rust 백엔드 규칙

### 1. 에러 핸들링 - unwrap() 금지

> **`unwrap()` 사용 금지** (테스트 코드 제외)

**✅ 안전한 패턴**:

```rust
// 1. let-else 조기 반환
let Some(value) = optional else {
    return Ok(Vec::new());
};

// 2. ok_or() 에러 전파
let value = optional.ok_or(MyError::NotFound)?;

// 3. unwrap_or() 기본값
let value = optional.unwrap_or_default();

// 4. unwrap_or_else() 계산된 기본값
let timestamp = parse_result.unwrap_or_else(|_| Utc::now());
```

**❌ 금지 패턴**:

```rust
// 프로덕션 코드에서 패닉 발생 가능
let value = option.unwrap();
let result = fallible_fn().unwrap();
```

### 2. Repository 패턴 사용

> **새로운 데이터 접근 로직은 Repository로 분리**

```rust
// repository/my_entity.rs
pub struct MyEntityRepository;

impl MyEntityRepository {
    pub async fn find_by_id(pool: &PgPool, id: &str) -> Result<Option<MyEntity>, sqlx::Error> {
        sqlx::query_as!(MyEntity, "SELECT * FROM my_entities WHERE id = $1", id)
            .fetch_optional(pool)
            .await
    }

    pub async fn create(pool: &PgPool, input: &CreateInput) -> Result<MyEntity, sqlx::Error> {
        // 트랜잭션 필요 시 사용
        let mut tx = pool.begin().await?;
        // ... 작업 ...
        tx.commit().await?;
        Ok(entity)
    }
}
```

**Repository 목록 (9개 구현됨)**:
- `backtest_results.rs`
- `equity_history.rs`
- `execution_cache.rs`
- `orders.rs`
- `portfolio.rs`
- `positions.rs`
- `strategies.rs`
- `symbol_info.rs`
- `klines.rs`

### 3. 비동기 패턴

**락 홀드 최소화**:

```rust
// ✅ 좋은 예 - 빠르게 락 해제
let data = {
    let guard = state.data.read().await;
    guard.clone()
};  // 락 해제
// 락 없이 처리
process_data(data);

// ❌ 나쁜 예 - 락을 잡고 I/O 수행
let guard = state.data.read().await;
let result = expensive_io_operation(&guard).await;  // 락 홀드 중 I/O
```

**CPU 집약 작업 분리**:

```rust
// spawn_blocking으로 CPU 작업 분리
let result = tokio::task::spawn_blocking(move || {
    // CPU 집약적 계산
    heavy_computation()
}).await?;
```

### 4. 입력 검증

> **모든 API 입력에 검증 함수 적용**

```rust
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct CreateRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,

    #[validate(range(min = 100, max = 1_000_000_000))]
    pub initial_capital: f64,

    #[validate(custom(function = "validate_date_format"))]
    pub start_date: String,
}

// 커스텀 검증 함수
fn validate_date_format(date: &str) -> Result<(), ValidationError> {
    if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
        return Err(ValidationError::new("invalid_date"));
    }
    Ok(())
}
```

### 5. 에러 응답 타입

> **통합 에러 타입 `ApiErrorResponse` 사용**

```rust
use crate::error::ApiErrorResponse;

// 핸들러에서 사용
async fn my_handler() -> Result<Json<MyResponse>, ApiErrorResponse> {
    let data = my_service()
        .await
        .map_err(|e| ApiErrorResponse::internal(e.to_string()))?;
    Ok(Json(data))
}
```

### 6. 트랜잭션 사용

> **다중 쿼리 시 트랜잭션 필수**

```rust
pub async fn complex_operation(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    // 작업 1
    sqlx::query!("UPDATE table1 SET ...")
        .execute(&mut *tx)
        .await?;

    // 작업 2
    sqlx::query!("INSERT INTO table2 ...")
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}
```

### 7. 주석 규칙

> **모든 주석은 한글로 작성**

**API 검증 주석 작성 예시**:

```rust
// API 검증: Context7 조회 (2026-01-31)
// Tokio 1.35, Axum 0.7.4 기준
// 참조: https://docs.rs/tokio/latest/tokio/macro.select.html
tokio::select! {
    // ...
}
```

```typescript
// API 검증: Context7 조회 (2026-01-31)
// SolidJS 1.8 기준
// 참조: https://docs.solidjs.com/concepts/stores
const [state, setState] = createStore({ ... });
```

**일반 주석 예시**:

```rust
/// 백테스트 결과를 저장합니다.
///
/// # Arguments
/// * `pool` - 데이터베이스 연결 풀
/// * `result` - 저장할 백테스트 결과
///
/// # Returns
/// 저장된 결과의 ID를 반환합니다.
pub async fn save_result(pool: &PgPool, result: &BacktestResult) -> Result<String, sqlx::Error> {
    // 기존 결과가 있으면 업데이트, 없으면 삽입
    // ...
}
```

---

## TypeScript 프론트엔드 규칙

### 0. TypeScript 바인딩 (ts-rs) ⭐ v0.6.0

> **핵심 원칙**: Rust 타입 → TypeScript 타입 자동 변환으로 API 타입 안전성 확보

**적용 대상**:
- API 요청/응답 DTO
- Domain 모델 (Signal, Order, Position 등)
- 전략 스키마 타입

**Rust 측 어노테이션**:

```rust
use ts_rs::TS;

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StrategyResponse {
    pub id: i32,
    pub name: String,
    pub running: bool,
}
```

**생성 파일 위치**: `frontend/src/types/generated/`

**빌드 명령**:
```bash
# TypeScript 바인딩 생성
cargo test --features ts-binding

# 또는 빌드 시 자동 생성
cargo build --features generate-ts
```

**규칙**:
- 새로운 API 응답 타입은 반드시 `#[derive(TS)]` 추가
- `#[ts(export)]`로 자동 내보내기 활성화
- 프론트엔드에서는 수동 타입 정의 대신 generated 타입 사용

**사용 예시 (프론트엔드)**:

```typescript
// ❌ 금지 - 수동 타입 정의 (Rust와 동기화 어려움)
interface StrategyResponse {
  id: number;
  name: string;
  running: boolean;
}

// ✅ 권장 - 자동 생성된 타입 사용
import { StrategyResponse } from '@/types/generated/StrategyResponse';
```

---

### 1. SolidJS 상태 관리

**createStore 사용 (복잡한 상태)**:

```typescript
import { createStore } from 'solid-js/store';

interface PageState {
  filter: 'all' | 'running' | 'stopped';
  modals: {
    add: { open: boolean };
    edit: { open: boolean; id: string | null };
  };
  loading: boolean;
}

const [state, setState] = createStore<PageState>({
  filter: 'all',
  modals: { add: { open: false }, edit: { open: false, id: null } },
  loading: false,
});
```

**createMemo 사용 (계산된 값)**:

```typescript
const filteredItems = createMemo(() => {
  return items().filter(item => item.status === state.filter);
});
```

### 2. 타입 안전성

**any 사용 금지**:

```typescript
// ❌ 금지
const data: any = response.data;

// ✅ 명시적 타입 정의
interface ApiResponse {
  strategies: Strategy[];
  total: number;
}
const data: ApiResponse = response.data;
```

**리터럴 타입 사용**:

```typescript
type OrderStatus = 'pending' | 'filled' | 'cancelled' | 'rejected';
type OrderSide = 'buy' | 'sell';
type Timeframe = '1m' | '5m' | '15m' | '1h' | '4h' | '1d' | '1w' | '1M';
```

### 3. 에러 처리

```typescript
<Show when={resource.loading}>
  <LoadingSpinner />
</Show>

<Show when={resource.error}>
  <ErrorBanner
    message={resource.error.message}
    onRetry={() => refetch()}
  />
</Show>

<Show when={resource()}>
  {/* 성공 시 렌더링 */}
</Show>
```

---

## 데이터베이스 규칙

### 1. 마이그레이션 명명

```
migrations/
  001_initial_schema.sql
  002_add_encrypted_credentials.sql
  ...
  014_add_my_feature.sql  # 순번 + 설명
```

### 2. 인덱스 필수 확인

```sql
-- 자주 조회하는 컬럼에 인덱스 추가
CREATE INDEX idx_orders_strategy_id ON orders(strategy_id);
CREATE INDEX idx_orders_created_at ON orders(created_at);
```

### 3. TimescaleDB Hypertable

> **시계열 데이터는 Hypertable로 생성**

```sql
-- 일반 테이블 생성
CREATE TABLE klines (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    open DECIMAL(18,8),
    -- ...
);

-- Hypertable로 변환
SELECT create_hypertable('klines', 'time');
```

---

## API 설계 규칙

### 1. 엔드포인트 명명

```
GET    /api/v1/resources          # 목록 조회
GET    /api/v1/resources/:id      # 단건 조회
POST   /api/v1/resources          # 생성
PUT    /api/v1/resources/:id      # 전체 수정
PATCH  /api/v1/resources/:id      # 부분 수정
DELETE /api/v1/resources/:id      # 삭제
```

### 2. OpenAPI 문서화

> **새 엔드포인트는 utoipa 어노테이션 필수**

```rust
#[utoipa::path(
    get,
    path = "/api/v1/my-resource/{id}",
    params(
        ("id" = String, Path, description = "리소스 ID")
    ),
    responses(
        (status = 200, description = "성공", body = MyResponse),
        (status = 404, description = "리소스 없음")
    ),
    tag = "my-resource"
)]
async fn get_my_resource(Path(id): Path<String>) -> impl IntoResponse {
    // ...
}
```

### 3. 응답 형식 통일

```rust
// 성공 응답
{
    "data": { ... },
    "meta": { "total": 100, "page": 1 }
}

// 에러 응답
{
    "error": {
        "code": "VALIDATION_ERROR",
        "message": "유효하지 않은 입력입니다",
        "details": { ... }
    }
}
```

---

## 보안 규칙

### 1. API 키 관리

> **환경변수 대신 웹 UI를 통한 암호화 저장**

- 거래소 API 키 → Settings 페이지에서 설정
- 텔레그램 봇 토큰 → Settings 페이지에서 설정
- 모든 민감 정보 → AES-256-GCM 암호화 저장

### 2. 민감 정보 로깅 방지

```rust
// ❌ 금지 - API 키 로깅
tracing::info!("API Key: {}", api_key);

// ✅ 마스킹 처리
tracing::info!("API Key: {}***", &api_key[..4]);

// 또는 secrecy 크레이트 사용
use secrecy::{Secret, ExposeSecret};
let api_key: Secret<String> = Secret::new(key);
```

### 3. 입력 검증

모든 외부 입력에 대해:
- 길이 제한
- 형식 검증
- 범위 검증
- SQL Injection 방지 (prepared statement 사용)

---

## 테스트 규칙

### 1. 테스트 파일 위치

```
crates/trader-strategy/src/strategies/
  rsi.rs
  rsi_test.rs     # 또는 mod.rs 내 #[cfg(test)]

tests/
  integration/
    backtest_test.rs
    api_test.rs
```

### 2. 테스트 명명

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_should_generate_buy_signal_when_rsi_below_threshold() {
        // Given: RSI가 30 이하인 상황
        // When: 신호 생성 호출
        // Then: 매수 신호 반환
    }
}
```

### 3. 테스트에서만 unwrap() 허용

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_something() {
        let result = some_function().unwrap();  // 테스트에서는 허용
        assert_eq!(result, expected);
    }
}
```

---

## 전략 추가 체크리스트

새 전략 추가 시 다음 5곳을 수정해야 합니다:

```
□ 1. crates/trader-strategy/src/strategies/mod.rs
   □ pub mod your_strategy;
   □ pub use your_strategy::*;

□ 2. crates/trader-api/src/routes/strategies.rs
   □ create_strategy_instance() - 전략 인스턴스 생성
   □ get_strategy_default_name() - 한글 이름
   □ get_strategy_default_timeframe() - 기본 타임프레임
   □ get_strategy_default_symbols() - 권장 심볼

□ 3. crates/trader-api/src/routes/backtest/engine.rs
   □ import 추가
   □ run_strategy_backtest() 또는 run_multi_strategy_backtest()

□ 4. config/sdui/strategy_schemas.json
   □ strategies 객체에 전략 스키마 추가 (~50줄)

□ 5. frontend/src/pages/Strategies.tsx
   □ getDefaultTimeframe() switch 문에 case 추가
```

---

## 금융 계산 규칙

### 1. Decimal 타입 사용 필수

> **f64 사용 금지**: 금액, 가격, 수량 계산에는 반드시 `rust_decimal::Decimal` 사용

```rust
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// ❌ 금지 - 부동소수점 오차
let price: f64 = 0.1 + 0.2;  // 0.30000000000000004

// ✅ 필수 - Decimal 사용
let price = dec!(0.1) + dec!(0.2);  // 정확히 0.3
```

**적용 대상**:
- 주문 가격 (`order_price`)
- 수량 (`quantity`)
- 잔고 (`balance`)
- 손익 계산 (`pnl`)
- 수수료 (`fee`)

### 2. 타임스탬프 UTC 강제

```rust
use chrono::{DateTime, Utc};

// ✅ 모든 시간은 UTC로 저장
let timestamp: DateTime<Utc> = Utc::now();

// ❌ 로컬 타임존 사용 금지
let local = Local::now();  // 타임존 혼동 가능
```

### 3. Idempotency (멱등성) 보장

> **주문 API는 중복 실행 시 동일한 결과를 보장해야 합니다.**

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

### 4. NewType 패턴으로 타입 안전성

```rust
// 타입 혼동 방지
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StrategyId(pub String);

// ✅ 컴파일 타임에 타입 검증
fn cancel_order(order_id: OrderId) { ... }
cancel_order(strategy_id);  // 컴파일 에러!
```

---

## 모니터링 및 로깅

> **원칙**: 프로젝트 특성상 과도한 모니터링 시스템 지양. 실용적이고 간소화된 접근 방식 사용.

### 1. 구조화 로깅 (tracing)

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

**로그 레벨 기준**:
- `error!`: 즉시 대응 필요 (주문 실패, DB 연결 끊김)
- `warn!`: 주의 필요 (API 재시도, 비정상 데이터)
- `info!`: 주요 이벤트 (주문 체결, 전략 시작/중지)
- `debug!`: 디버깅 정보 (파라미터 값, 중간 계산)
- `trace!`: 상세 추적 (루프 내부, 모든 함수 호출)

**로그 레벨 제어**:
```bash
# 환경변수로 제어
RUST_LOG=info,trader_api=info
RUST_LOG=debug,trader_strategy=debug  # 특정 모듈 상세 로깅
```

### 2. 헬스체크

```rust
// /health 엔드포인트로 기본 상태 확인
// /health/ready 엔드포인트로 컴포넌트 상태 확인 (DB, Redis, Exchange)
```

**확인 명령**:
```bash
curl http://localhost:3000/health/ready
```

### 3. 알림 (Telegram/Discord)

**웹 UI에서 관리** (암호화 저장):
- 봇 토큰, Chat ID 설정
- 테스트 메시지 전송 기능

**알림 대상**:
- ❌ 에러: 주문 실패, DB 연결 끊김, API 키 만료
- ⚠️ 경고: 잔고 부족, 리스크 한계 도달
- ✅ 정보: 일일 손익 보고, 전략 성과 요약

> 상세 가이드: `docs/operations.md` 참조

---

## 코드 리뷰 체크리스트

### Pull Request 체크리스트

**기본 검증**:
- [ ] 모든 테스트 통과 (`cargo test`)
- [ ] Clippy 경고 없음 (`cargo clippy -- -D warnings`)
- [ ] 포맷팅 준수 (`cargo fmt -- --check`)
- [ ] 빌드 성공 (`cargo build --release`)

**코드 품질**:
- [ ] `unwrap()` 사용 없음 (테스트 제외)
- [ ] 레거시 코드 제거 완료
- [ ] 주석은 한글로 작성
- [ ] 공개 API에 Rustdoc 주석 추가

**아키텍처**:
- [ ] 거래소 중립적 코드 (trait 사용)
- [ ] Repository 패턴 준수
- [ ] 에러 타입 명확히 정의
- [ ] 비동기 작업 적절히 처리

**보안**:
- [ ] API 키 하드코딩 없음
- [ ] 민감 정보 로깅 없음
- [ ] 입력 검증 적용
- [ ] SQL Injection 방지 (prepared statement)

**금융 계산**:
- [ ] `Decimal` 타입 사용 (f64 금지)
- [ ] UTC 타임스탬프 사용
- [ ] Idempotency 보장 (필요시)

**테스트**:
- [ ] 단위 테스트 추가
- [ ] 엣지 케이스 테스트
- [ ] 금융 계산은 property-based 테스트 (proptest)

**문서**:
- [ ] README 업데이트 (필요시)
- [ ] API 문서 업데이트 (docs/api.md)
- [ ] CHANGELOG 업데이트

---

## 코드 품질 도구

### rustfmt

```bash
# 포맷팅 실행
cargo fmt --all

# 체크만 (CI용)
cargo fmt --all -- --check
```

### clippy

```bash
# 린트 실행
cargo clippy --all-targets --all-features

# 경고를 에러로 처리 (CI용)
cargo clippy --all-targets --all-features -- -D warnings
```

### 빠른 검증 명령어

```bash
# Rust 의존성 버전 확인
cargo tree -p tokio
cargo tree -p axum
cargo tree -p sqlx

# Node.js 의존성 버전 확인
npm ls solid-js
npm ls vite

# Rust 문서 로컬 생성 (오프라인 참조)
cargo doc --open --no-deps
```

### 의존성 보안 검사

```bash
cargo audit
```

### Pre-commit Hook 설정

```bash
# .git/hooks/pre-commit 파일 생성
cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
echo "==> 포맷 검사..."
cargo fmt --all -- --check || exit 1

echo "==> Clippy 검사..."
cargo clippy --all-targets -- -D warnings || exit 1

echo "==> 빌드 검사..."
cargo check --all || exit 1

echo "✅ 모든 검사 통과!"
EOF

chmod +x .git/hooks/pre-commit
```

> **참고**: 프로젝트 특성상 과도한 CI/CD는 지양합니다. 로컬 검증 중심으로 운영합니다.

---

## 참고 문서

| 문서 | 위치 | 용도 |
|------|------|------|
| TODO | `docs/todo.md` | 현재 작업 상태 |
| PRD | `docs/prd.md` | 제품 요구사항 정의 |
| 아키텍처 | `docs/architecture.md` | 시스템 구조 |
| API 문서 | `docs/api.md` | REST API 명세 |
| CLAUDE.md | 루트 | 세션 컨텍스트 |

---

*이 문서를 신규 기능 구현 전 반드시 검토하세요.*
