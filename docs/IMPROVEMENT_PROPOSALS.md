# ZeroQuant 개선 제안서

이 문서는 ZeroQuant 프로젝트의 코드 품질, 운영성, 확장성을 향상시키기 위한 구체적인 개선 제안을 담고 있습니다.

---

## 🏆 우선순위 분류

- 🔴 **P0 (Critical)**: 보안/안정성 문제, 즉시 해결 필요
- 🟠 **P1 (High)**: 운영 효율성 크게 향상, 1-2주 내 처리
- 🟡 **P2 (Medium)**: 개발 생산성 향상, 1-2개월 내 처리
- 🟢 **P3 (Low)**: Nice-to-have, 장기 계획

---

## 🔴 P0: Critical Issues

### 1. 환경변수 기본값 제거
**문제점**:
```yaml
# docker-compose.yml
- JWT_SECRET=${JWT_SECRET:-your-super-secret-jwt-key-change-in-production}
- ENCRYPTION_MASTER_KEY=${ENCRYPTION_MASTER_KEY:-MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=}
```
- 프로덕션 환경에서 기본값 사용 시 심각한 보안 위험
- 모든 배포 인스턴스가 동일한 키 사용 가능

**해결 방안**:
```yaml
# docker-compose.yml
- JWT_SECRET=${JWT_SECRET:?JWT_SECRET environment variable is required}
- ENCRYPTION_MASTER_KEY=${ENCRYPTION_MASTER_KEY:?ENCRYPTION_MASTER_KEY is required}
```
- 필수 변수 미설정 시 컨테이너 시작 실패 (Fail-fast)
- README에 키 생성 가이드 추가:
  ```bash
  # JWT_SECRET 생성 (최소 32바이트)
  openssl rand -hex 32
  
  # ENCRYPTION_MASTER_KEY 생성 (32바이트 Base64)
  openssl rand -base64 32
  ```

**예상 효과**: 실수로 인한 보안 사고 방지

---

### 2. Unwrap 제거 및 에러 핸들링 개선
**문제점**:
```rust
// 일부 코드에서 unwrap() 사용
let value = some_option.unwrap();  // 패닉 발생 가능
```
- 프로덕션 환경에서 예상치 못한 패닉 → 서비스 중단

**해결 방안**:
```rust
// Option 처리
let value = some_option.ok_or(Error::ValueNotFound)?;

// Result 처리
let result = operation().map_err(|e| Error::OperationFailed(e))?;

// Default 값 사용
let value = some_option.unwrap_or_default();
```

**작업 범위**:
1. `grep -r "unwrap()" crates/` 실행
2. 각 케이스 분석 후 적절한 에러 핸들링으로 변경
3. Clippy 린트 활성화: `#![deny(clippy::unwrap_used)]`

**예상 효과**: 런타임 안정성 향상, 디버깅 용이

---

## 🟠 P1: High Priority

### 3. CI/CD 파이프라인 구축
**문제점**:
- `.github/workflows/` 디렉토리 없음
- 수동 테스트/빌드 → 인간 실수 가능
- 코드 품질 검증 자동화 부재

**해결 방안**: GitHub Actions 워크플로우 추가

#### 3.1. 기본 CI 파이프라인
```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - name: Format check
        run: cargo fmt --all -- --check
      - name: Clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

  test:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: timescale/timescaledb:latest-pg15
        env:
          POSTGRES_PASSWORD: postgres
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
      redis:
        image: redis:7-alpine
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Test
        run: cargo test --all-features --workspace
        env:
          DATABASE_URL: postgresql://postgres:postgres@localhost/test
          REDIS_URL: redis://localhost:6379

  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Build
        run: cargo build --release --all-features
```

#### 3.2. Docker 이미지 빌드/푸시
```yaml
# .github/workflows/docker.yml
name: Docker Build

on:
  push:
    tags: ['v*']

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: docker/setup-buildx-action@v3
      - uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - uses: docker/build-push-action@v5
        with:
          push: true
          tags: ghcr.io/${{ github.repository }}:${{ github.ref_name }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

**예상 효과**:
- 코드 품질 자동 검증
- 배포 시간 단축 (자동화)
- 버그 조기 발견

---

### 4. 통합 테스트 강화
**문제점**:
- 단위 테스트는 많지만 통합 테스트 부족
- API 엔드포인트 E2E 테스트 없음
- 거래소 연동 mock 테스트 부족

**해결 방안**:

#### 4.1. API 통합 테스트
```rust
// crates/trader-api/tests/integration_test.rs
#[tokio::test]
async fn test_strategy_lifecycle() {
    let app = test_app().await;
    
    // 1. 전략 생성
    let response = app.post("/api/v1/strategies")
        .json(&json!({
            "name": "grid",
            "config": { "symbol": "BTCUSDT" }
        }))
        .send()
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    
    // 2. 전략 시작
    let strategy_id = response.json::<Strategy>().id;
    let response = app.post(&format!("/api/v1/strategies/{}/start", strategy_id))
        .send()
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    
    // 3. 상태 확인
    let response = app.get(&format!("/api/v1/strategies/{}", strategy_id))
        .send()
        .await;
    let strategy = response.json::<Strategy>();
    assert_eq!(strategy.status, "running");
}
```

#### 4.2. Mock 거래소 테스트
```rust
// crates/trader-exchange/tests/binance_mock.rs
#[tokio::test]
async fn test_order_placement_with_mock() {
    let mut server = mockito::Server::new_async().await;
    let mock = server.mock("POST", "/api/v3/order")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"orderId": 123, "status": "FILLED"}"#)
        .create();
    
    let exchange = BinanceExchange::new_with_base_url(server.url());
    let order = exchange.place_order(/* ... */).await.unwrap();
    
    assert_eq!(order.id, 123);
    mock.assert();
}
```

**예상 효과**:
- 회귀 버그 조기 발견
- 리팩토링 안정성 향상

---

### 5. API 자동 문서화 (OpenAPI/Swagger)
**문제점**:
- 수동 문서 관리 (`docs/api.md`) → 실제 코드와 불일치 가능
- API 스펙 변경 시 문서 업데이트 누락

**해결 방안**: `utoipa` crate 도입

```rust
// Cargo.toml
[dependencies]
utoipa = { version = "4", features = ["axum_extras"] }
utoipa-swagger-ui = { version = "6", features = ["axum"] }

// src/main.rs
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::strategies::list_strategies,
        routes::strategies::create_strategy,
    ),
    components(schemas(Strategy, Order))
)]
struct ApiDoc;

let app = Router::new()
    .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
    .route("/api/v1/strategies", get(list_strategies));
```

**예상 효과**:
- API 문서 항상 최신 유지
- 프론트엔드 개발자 생산성 향상
- API 클라이언트 자동 생성 가능

---

### 6. 전략 파일 리팩토링 (900+ 라인)
**문제점**:
- `xaa.rs` (1,103 라인), `candle_pattern.rs` (958 라인) 등 대형 파일
- 단일 파일에서 설정, 비즈니스 로직, 상태 관리 모두 처리

**해결 방안**: 모듈 분리

```
strategies/
├── xaa/
│   ├── mod.rs          # pub use exports
│   ├── config.rs       # XaaConfig 정의
│   ├── state.rs        # XaaState 상태 관리
│   ├── logic.rs        # 핵심 알고리즘
│   └── rebalance.rs    # 리밸런싱 로직
└── candle_pattern/
    ├── mod.rs
    ├── config.rs
    ├── recognizer.rs   # 패턴 인식
    └── patterns/       # 개별 패턴 (hammer, engulfing 등)
        ├── hammer.rs
        ├── engulfing.rs
        └── ...
```

**원칙**:
- 파일당 300 라인 이하
- 하나의 책임만 (Single Responsibility)
- 테스트 용이성 향상

**예상 효과**:
- 코드 가독성 향상
- 유지보수 용이
- 병렬 개발 가능

---

## 🟡 P2: Medium Priority

### 7. 데이터베이스 마이그레이션 자동화
**문제점**:
- Docker 컨테이너 초기화 시 `/docker-entrypoint-initdb.d`에 의존
- 스키마 버전 관리 없음
- 롤백 불가능

**해결 방안**: `sqlx-cli` 마이그레이션 사용

```bash
# 설치
cargo install sqlx-cli --no-default-features --features postgres

# 마이그레이션 생성
sqlx migrate add create_users_table

# 적용
sqlx migrate run --database-url $DATABASE_URL

# 롤백
sqlx migrate revert
```

```rust
// src/main.rs
#[tokio::main]
async fn main() -> Result<()> {
    let pool = PgPoolOptions::new()
        .connect(&env::var("DATABASE_URL")?)
        .await?;
    
    // 자동 마이그레이션
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;
    
    // ...
}
```

**예상 효과**:
- 스키마 버전 추적
- 프로덕션 배포 안정성 향상

---

### 8. 설정 파일 외부화
**문제점**:
- 일부 상수가 코드에 하드코딩
  ```rust
  const MAX_POSITION_SIZE: f64 = 10000.0;
  const DEFAULT_LEVERAGE: u8 = 1;
  ```
- 환경별 설정 변경 시 재컴파일 필요

**해결 방안**: `config/` 디렉토리 활용

```toml
# config/default.toml
[trading]
max_position_size = 10000.0
default_leverage = 1
max_daily_loss_pct = 5.0

[risk]
stop_loss_pct = 2.0
take_profit_pct = 5.0

# config/production.toml (상속)
[trading]
max_position_size = 50000.0
```

```rust
use config::{Config, File, Environment};

let settings = Config::builder()
    .add_source(File::with_name("config/default"))
    .add_source(File::with_name(&format!("config/{}", env)).required(false))
    .add_source(Environment::with_prefix("APP"))
    .build()?;
```

**예상 효과**:
- 환경별 설정 분리 (dev/staging/prod)
- 재배포 없이 설정 변경

---

### 9. 로깅 구조화 (Structured Logging)
**문제점**:
- 현재: `tracing-subscriber` 사용하지만 JSON 포맷 활용 부족
- 로그 검색/필터링 어려움

**해결 방안**: JSON 로깅 + 컨텍스트 추가

```rust
// Cargo.toml
[dependencies]
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }

// src/main.rs
let subscriber = tracing_subscriber::fmt()
    .json()
    .with_current_span(false)
    .with_span_list(true)
    .with_target(true)
    .finish();

// 사용
tracing::info!(
    strategy_id = %strategy.id,
    symbol = %symbol,
    action = "buy",
    quantity = %qty,
    "Order placed"
);
```

**로그 쿼리 예시** (Grafana Loki):
```
{job="trader-api"} | json | action="buy" | symbol="BTCUSDT"
```

**예상 효과**:
- 운영 이슈 디버깅 시간 단축
- 메트릭 추출 용이

---

### 10. 캐싱 전략 개선
**문제점**:
- Redis 사용하지만 TTL 관리 부족
- 캐시 무효화 전략 불명확

**해결 방안**: 계층적 캐싱 + TTL

```rust
pub struct CacheService {
    redis: ConnectionManager,
}

impl CacheService {
    // 시세 (10초 TTL)
    pub async fn get_price(&self, symbol: &str) -> Option<Decimal> {
        self.get_with_ttl(&format!("price:{}", symbol), Duration::from_secs(10)).await
    }
    
    // 전략 설정 (1시간 TTL)
    pub async fn get_strategy_config(&self, id: Uuid) -> Option<Value> {
        self.get_with_ttl(&format!("strategy:{}:config", id), Duration::from_secs(3600)).await
    }
    
    // 캐시 무효화
    pub async fn invalidate_strategy(&self, id: Uuid) -> Result<()> {
        self.redis.del(&format!("strategy:{id}:*")).await?;
        Ok(())
    }
}
```

**예상 효과**:
- DB 부하 감소
- 응답 속도 향상

---

### 11. 프론트엔드 타입 안정성 강화
**문제점**:
- API 응답 타입이 수동으로 정의됨 (`frontend/src/types/`)
- 백엔드 변경 시 프론트엔드 타입 불일치 가능

**해결 방안**: `ts-rs` crate로 TypeScript 타입 자동 생성

```rust
// Cargo.toml
[dependencies]
ts-rs = "8"

// src/domain/strategy.rs
use ts_rs::TS;

#[derive(Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/types/generated/")]
pub struct Strategy {
    pub id: Uuid,
    pub name: String,
    pub status: StrategyStatus,
}
```

```bash
# 빌드 시 자동 생성
cargo build

# frontend/src/types/generated/Strategy.ts 생성됨
export interface Strategy {
  id: string;
  name: string;
  status: StrategyStatus;
}
```

**예상 효과**:
- 타입 불일치 에러 사전 방지
- 프론트엔드 개발 속도 향상

---

### 12. 백테스트 성능 최적화
**문제점**:
- 대용량 과거 데이터 로딩 시 메모리 부족 가능
- 백테스트 속도 느림 (단일 스레드)

**해결 방안**:

#### 12.1. 데이터 청크 로딩
```rust
// 메모리 절약: 배치 단위 로딩
pub async fn load_candles_batched(
    &self,
    symbol: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    batch_size: usize,
) -> Result<impl Stream<Item = Result<Vec<Candle>>>> {
    // Stream API 사용으로 메모리 효율 향상
}
```

#### 12.2. 병렬 백테스트
```rust
use rayon::prelude::*;

pub async fn run_parallel_backtest(
    strategies: Vec<StrategyConfig>,
    data: Arc<Vec<Candle>>,
) -> Vec<BacktestResult> {
    strategies.par_iter()
        .map(|config| run_backtest(config, &data))
        .collect()
}
```

**예상 효과**:
- 백테스트 시간 50% 이상 단축
- 대용량 데이터 처리 가능

---

## 🟢 P3: Low Priority

### 13. 분산 아키텍처 지원
**목표**: 수평 확장 가능한 시스템

**설계 방안**:

```
┌─────────────┐      ┌─────────────┐
│  API Server │      │  API Server │
│  (Worker 1) │      │  (Worker 2) │
└──────┬──────┘      └──────┬──────┘
       │                    │
       └─────────┬──────────┘
                 │
       ┌─────────▼──────────┐
       │  Redis Pub/Sub     │  ← 이벤트 브로커
       │  + Distributed Lock│
       └─────────┬──────────┘
                 │
       ┌─────────▼──────────┐
       │    PostgreSQL      │
       │   (Read Replica)   │
       └────────────────────┘
```

**핵심 변경**:
1. **분산 락**: `redis-rs` distributed lock
   ```rust
   let lock = redis.get_lock("strategy:12345", Duration::from_secs(60)).await?;
   if lock.acquired() {
       // 전략 실행
   }
   ```

2. **이벤트 기반 통신**: Redis Pub/Sub or Kafka
   ```rust
   pubsub.publish("strategy.signal", &signal).await?;
   ```

**예상 효과**:
- 무정지 배포 (rolling update)
- 고가용성 (HA)

---

### 14. ML 모델 버전 관리
**목표**: ONNX 모델 A/B 테스트 및 롤백

**해결 방안**:
```
models/
├── candle_pattern_v1.onnx
├── candle_pattern_v2.onnx (실험)
└── metadata.json
```

```json
// models/metadata.json
{
  "candle_pattern": {
    "production": "candle_pattern_v1.onnx",
    "staging": "candle_pattern_v2.onnx",
    "accuracy": { "v1": 0.85, "v2": 0.87 }
  }
}
```

**예상 효과**:
- 모델 성능 비교 용이
- 배포 안전성 향상

---

### 15. 다중 계정 지원
**목표**: 사용자별 독립된 트레이딩 환경

**데이터베이스 스키마**:
```sql
CREATE TABLE users (
    id UUID PRIMARY KEY,
    email VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMP NOT NULL
);

ALTER TABLE strategies ADD COLUMN user_id UUID REFERENCES users(id);
ALTER TABLE orders ADD COLUMN user_id UUID REFERENCES users(id);
```

**API 변경**:
```rust
// 인증된 사용자 컨텍스트
async fn list_strategies(
    Extension(user): Extension<AuthUser>,
    State(db): State<PgPool>,
) -> Result<Json<Vec<Strategy>>> {
    let strategies = sqlx::query_as!(
        Strategy,
        "SELECT * FROM strategies WHERE user_id = $1",
        user.id
    )
    .fetch_all(&db)
    .await?;
    
    Ok(Json(strategies))
}
```

**예상 효과**:
- SaaS 제품화 가능
- 비즈니스 모델 확장

---

### 16. WebAssembly (WASM) 전략 지원
**목표**: 브라우저에서 백테스트 실행

**아키텍처**:
```
Rust 전략 코드
     ↓
wasm-pack build
     ↓
strategy.wasm
     ↓
Frontend 로드 → 백테스트 실행 (브라우저)
```

**장점**:
- 서버 부하 감소
- 빠른 피드백 루프
- 오프라인 백테스트 가능

**단점**:
- WASM 크기 제한
- 디버깅 어려움

---

## 📊 구현 로드맵

### Phase 1: 보안 및 안정성 (1주)
- [ ] P0-1: 환경변수 기본값 제거
- [ ] P0-2: Unwrap 제거 (50% 이상)
- [ ] P1-3: CI 파이프라인 기본 구축

### Phase 2: 운영 효율성 (2주)
- [ ] P1-4: 통합 테스트 추가 (주요 API 10개)
- [ ] P1-5: OpenAPI 문서 자동화
- [ ] P2-7: DB 마이그레이션 자동화
- [ ] P2-9: 로깅 구조화

### Phase 3: 개발 생산성 (4주)
- [ ] P1-6: 전략 파일 리팩토링 (상위 3개)
- [ ] P2-8: 설정 파일 외부화
- [ ] P2-10: 캐싱 전략 개선
- [ ] P2-11: 프론트엔드 타입 생성 자동화

### Phase 4: 성능 및 확장성 (진행 중)
- [ ] P2-12: 백테스트 병렬화
- [ ] P3-13: 분산 아키텍처 설계
- [ ] P3-14: ML 모델 버전 관리

### Phase 5: 비즈니스 확장 (장기)
- [ ] P3-15: 다중 계정 지원
- [ ] P3-16: WASM 전략 지원

---

## 🎯 기대 효과

### 기술적 이점
| 개선 항목 | 기대 효과 | 측정 지표 |
|----------|----------|---------|
| CI/CD 파이프라인 | 배포 시간 70% 단축 | 배포 소요 시간 |
| 통합 테스트 | 버그 발견율 50% 향상 | 프로덕션 버그 수 |
| 캐싱 최적화 | 응답 시간 30% 개선 | P95 latency |
| 백테스트 병렬화 | 처리 시간 60% 단축 | 백테스트 완료 시간 |

### 비즈니스 이점
- **출시 속도**: 기능 개발 → 배포 주기 단축
- **안정성**: 보안 사고 리스크 감소
- **확장성**: 사용자/트래픽 증가 대응
- **비용**: 인프라 비용 최적화 (캐싱)

---

## 📝 결론

이 제안서는 ZeroQuant 프로젝트를 **개인 프로젝트 수준에서 프로덕션급 시스템**으로 발전시키기 위한 로드맵을 제시합니다.

### 핵심 메시지
1. **지금 해야 할 것** (P0): 보안 강화 (환경변수, unwrap 제거)
2. **단기 목표** (P1): 운영 자동화 (CI/CD, 테스트)
3. **중기 목표** (P2): 개발 생산성 (리팩토링, 설정 관리)
4. **장기 비전** (P3): 확장성 및 비즈니스 모델

각 제안은 독립적으로 실행 가능하며, 우선순위에 따라 순차적으로 진행하시면 됩니다.

**질문이나 추가 논의가 필요한 사항은 GitHub Issues에 등록해주세요!** 🚀
