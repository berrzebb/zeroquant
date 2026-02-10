# Rust 백엔드 규칙

## 1. 에러 핸들링 - unwrap() 금지

> `unwrap()` 사용 금지 (테스트 코드 제외)

### ✅ 안전한 패턴

```rust
// let-else 조기 반환
let Some(value) = optional else {
    return Ok(Vec::new());
};

// ok_or() 에러 전파
let value = optional.ok_or(MyError::NotFound)?;

// unwrap_or() 기본값
let value = optional.unwrap_or_default();

// unwrap_or_else() 계산된 기본값
let timestamp = parse_result.unwrap_or_else(|_| Utc::now());
```

### ❌ 금지 패턴

```rust
let value = option.unwrap();
let result = fallible_fn().unwrap();
```

## 2. Repository 패턴 사용

> 새로운 데이터 접근 로직은 Repository로 분리

```rust
pub struct MyEntityRepository;

impl MyEntityRepository {
    pub async fn find_by_id(pool: &PgPool, id: &str) -> Result<Option<MyEntity>, sqlx::Error> {
        sqlx::query_as!(MyEntity, "SELECT * FROM my_entities WHERE id = $1", id)
            .fetch_optional(pool)
            .await
    }
}
```

**기존 Repository 목록**: `backtest_results`, `equity_history`, `execution_cache`, `orders`, `portfolio`, `positions`, `strategies`, `symbol_info`, `klines`

## 3. 비동기 패턴

### 락 홀드 최소화

```rust
// ✅ 빠르게 락 해제
let data = {
    let guard = state.data.read().await;
    guard.clone()
};  // 락 해제 후 처리
process_data(data);

// ❌ 락을 잡고 I/O 수행 금지
```

### CPU 집약 작업 분리

```rust
let result = tokio::task::spawn_blocking(move || {
    heavy_computation()
}).await?;
```

## 4. 입력 검증

> 모든 API 입력에 `validator::Validate` 적용

```rust
#[derive(Deserialize, Validate)]
pub struct CreateRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,

    #[validate(range(min = 100, max = 1_000_000_000))]
    pub initial_capital: f64,

    #[validate(custom(function = "validate_date_format"))]
    pub start_date: String,
}
```

## 5. 에러 응답 타입

> 통합 에러 타입 `ApiErrorResponse` 사용

```rust
async fn my_handler() -> Result<Json<MyResponse>, ApiErrorResponse> {
    let data = my_service()
        .await
        .map_err(|e| ApiErrorResponse::internal(e.to_string()))?;
    Ok(Json(data))
}
```

## 6. 트랜잭션 사용

> 다중 쿼리 시 트랜잭션 필수

```rust
let mut tx = pool.begin().await?;
sqlx::query!("UPDATE table1 SET ...").execute(&mut *tx).await?;
sqlx::query!("INSERT INTO table2 ...").execute(&mut *tx).await?;
tx.commit().await?;
```

## 7. 주석 규칙

> 모든 주석은 한글로 작성

### API 검증 주석

```rust
// API 검증: Context7 조회 (2026-01-31)
// Tokio 1.35, Axum 0.7.4 기준
// 참조: https://docs.rs/tokio/latest/tokio/macro.select.html
```

### Rustdoc 주석

```rust
/// 백테스트 결과를 저장합니다.
///
/// # Arguments
/// * `pool` - 데이터베이스 연결 풀
/// * `result` - 저장할 백테스트 결과
///
/// # Returns
/// 저장된 결과의 ID를 반환합니다.
```

## 8. Clippy 워닝 제로 정책

> `#[allow(clippy::...)]`로 워닝을 우회하지 않는다. 코드를 직접 수정한다.

### 자주 발생하는 clippy 워닝과 올바른 수정

| 워닝 | ❌ 잘못된 대응 | ✅ 올바른 수정 |
|------|---------------|---------------|
| `type_complexity` | `#[allow]` | type alias 사용 |
| `new_without_default` | `#[allow]` | Default 구현 |
| `map().flatten()` | 그대로 사용 | `and_then()` |
| Copy 타입에 `.clone()` | 그대로 사용 | 직접 전달 |
| `len() > 0` | 그대로 사용 | `!is_empty()` |
| 수동 범위 비교 | 그대로 사용 | `.contains()` |

### `#[allow]` 허용 예외 (극히 제한적)

- `#[allow(clippy::too_many_arguments)]`: 구조체 분리가 비현실적일 때만
- `#![allow(unexpected_cfgs)]`: 테스트 전용 feature flag (파일 최상단)
