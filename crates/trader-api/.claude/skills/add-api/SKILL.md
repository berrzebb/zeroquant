---
name: add-api
description: API 엔드포인트 추가 체크리스트. 라우터, 핸들러, OpenAPI, TS 바인딩 포함.
disable-model-invocation: true
user-invocable: true
argument-hint: "<리소스명> [GET|POST|PUT|PATCH|DELETE]"
allowed-tools: Read, Grep, Edit, Write, Bash(cargo *)
context: fork
  agent: rust-impl
---

# API 엔드포인트 추가 워크플로우

새 엔드포인트를 `$ARGUMENTS` 사양으로 추가합니다.

---

## 0단계: 기존 구조 파악

```bash
# 기존 라우트 모듈 확인
ls crates/trader-api/src/routes/

# 라우터 등록 위치 확인
grep -n "Router" crates/trader-api/src/routes/mod.rs
```

---

## 1단계: 핸들러 구현

**위치**: `crates/trader-api/src/routes/<리소스>.rs` (신규 또는 기존 파일)

### 필수 패턴

```rust
use axum::{extract::*, Json};
use crate::error::ApiErrorResponse;

/// 리소스 조회
#[utoipa::path(
    get,
    path = "/api/v1/<리소스>/{id}",
    params(("id" = String, Path, description = "리소스 ID")),
    responses(
        (status = 200, description = "성공", body = MyResponse),
        (status = 404, description = "리소스 없음")
    ),
    tag = "<리소스>"
)]
pub async fn get_resource(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MyResponse>, ApiErrorResponse> {
    // Repository 패턴 사용
    let data = MyRepository::find_by_id(&state.pool, &id)
        .await
        .map_err(|e| ApiErrorResponse::internal(e.to_string()))?
        .ok_or_else(|| ApiErrorResponse::not_found("리소스를 찾을 수 없습니다"))?;
    Ok(Json(data))
}
```

### 체크포인트
- [ ] `#[utoipa::path]` 어노테이션 (OpenAPI 문서)
- [ ] `Result<_, ApiErrorResponse>` 반환
- [ ] `validator::Validate` 적용 (입력에)
- [ ] Repository 패턴 사용 (직접 SQL 금지)
- [ ] 금액 관련 필드는 `Decimal` 사용

---

## 2단계: 응답 타입 정의

```rust
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MyResponse {
    pub id: String,
    // ...
}
```

- [ ] `#[derive(TS)]` + `#[ts(export)]` (프론트엔드 타입 자동 생성)

---

## 3단계: 라우터 등록

**위치**: `crates/trader-api/src/routes/mod.rs`

```rust
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // ... 기존 라우트
        .route("/api/v1/<리소스>", get(get_resources))
        .route("/api/v1/<리소스>/:id", get(get_resource))
        .with_state(state)
}
```

---

## 4단계: 검증

```powershell
# 1. 컴파일 확인
cargo check -p trader-api

# 2. Clippy
cargo clippy -p trader-api -- -D warnings

# 3. 타입 바인딩 생성 (TS 타입이 있는 경우)
cargo test --features ts-binding

# 4. OpenAPI 스펙 확인
# API 실행 후 http://localhost:3000/api-docs 에서 확인
```
