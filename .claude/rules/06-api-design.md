# API 설계 규칙

## 1. 엔드포인트 명명

```
GET    /api/v1/resources          # 목록 조회
GET    /api/v1/resources/:id      # 단건 조회
POST   /api/v1/resources          # 생성
PUT    /api/v1/resources/:id      # 전체 수정
PATCH  /api/v1/resources/:id      # 부분 수정
DELETE /api/v1/resources/:id      # 삭제
```

## 2. OpenAPI 문서화

> 새 엔드포인트는 utoipa 어노테이션 필수

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

## 3. 응답 형식 통일

### 성공 응답

```json
{
    "data": { ... },
    "meta": { "total": 100, "page": 1 }
}
```

### 에러 응답

```json
{
    "error": {
        "code": "VALIDATION_ERROR",
        "message": "유효하지 않은 입력입니다",
        "details": { ... }
    }
}
```
