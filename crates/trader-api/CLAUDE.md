# trader-api

> REST/WebSocket API 서버. 모든 crate를 의존하는 허브.

## 라우트 구조

```
src/routes/mod.rs (라우터 설정)
├── strategies.rs      # /api/v1/strategies (등록/시작/중지)
├── orders.rs          # /api/v1/orders
├── positions.rs       # /api/v1/positions
├── backtest/mod.rs    # /api/v1/backtest
├── simulation.rs      # /api/v1/simulation (Paper Trading)
├── screening.rs       # /api/v1/screening
├── ranking.rs         # /api/v1/ranking (GlobalScore)
├── journal.rs         # /api/v1/journal (매매일지)
├── dataset.rs         # /api/v1/dataset
├── watchlist.rs       # /api/v1/watchlist
├── credentials/       # /api/v1/credentials (API 키)
├── monitoring.rs      # /api/v1/monitoring
└── health.rs          # /health, /health/ready
```

## AppState

`src/state.rs` — DB pool, Redis, StrategyEngine, PositionTracker 등 공유 상태

## 핸들러 작성 패턴

```rust
#[utoipa::path(get, path = "/api/v1/resource/{id}", ...)]
async fn get_resource(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MyResponse>, ApiErrorResponse> {
    let data = MyRepository::find_by_id(&state.pool, &id)
        .await
        .map_err(|e| ApiErrorResponse::internal(e.to_string()))?;
    Ok(Json(data))
}
```

## 규칙

- 모든 핸들러는 `Result<_, ApiErrorResponse>` 반환
- 새 엔드포인트는 `#[utoipa::path]` OpenAPI 어노테이션 필수
- 입력 검증은 `validator::Validate` 적용
- 데이터 접근은 Repository 패턴 사용 (직접 SQL 금지)
- 응답 타입에 `#[derive(TS)]` + `#[ts(export)]` 추가 (프론트엔드 바인딩)

> 자동화: `/add-api` 스킬 사용 권장

## 관련 스킬

- `/add-api` — 새 엔드포인트 추가 체크리스트
