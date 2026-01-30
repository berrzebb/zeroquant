# ZeroQuant 코드 개선 제안 - 빠른 참조 가이드

> **최종 업데이트**: 2026-01-30  
> **상세 문서**: [CODE_ANALYSIS_2026_01_30.md](CODE_ANALYSIS_2026_01_30.md)

---

## 🎯 Top 5 긴급 개선사항

### 1. ⚡ Async Sleep 전환 (2시간, 🔴🔴🔴)
```rust
// ❌ Before: thread::sleep(Duration::from_millis(60));
// ✅ After:  tokio::time::sleep(Duration::from_millis(60)).await;
```
**효과**: 처리량 +50-200%

### 2. 💣 Unwrap 제거 (1일, 🔴🔴🔴)
```rust
// ❌ Before: let value = parse(s).unwrap();
// ✅ After:  let value = parse(s).context("Parse failed")?;
```
**효과**: 패닉 방지, 안정성 대폭 향상

### 3. 📊 DB 인덱스 추가 (2시간, 🔴🔴)
```sql
CREATE INDEX CONCURRENTLY idx_ohlcv_symbol_timeframe_timestamp 
ON ohlcv(symbol, timeframe, timestamp DESC);
```
**효과**: 쿼리 성능 10-100배 향상

### 4. 🔒 자격증명 암호화 검증 (4시간, 🔴🔴)
```rust
// 암호화 후 복호화 테스트 추가
let encrypted = encrypt_credentials(&creds)?;
let decrypted = decrypt_credentials(&encrypted)?;
assert_eq!(decrypted, creds);
```
**효과**: 보안 강화

### 5. 📂 큰 파일 분리 (2일, 🔴)
```
backtest.rs (3,854 LOC) → backtest/ 모듈 (400 LOC/파일)
├── mod.rs, schema.rs, config.rs
└── handlers/{run.rs, results.rs, compare.rs}
```
**효과**: 유지보수성 +50%

---

## 📋 실행 체크리스트

### Week 1 (긴급)
- [ ] `grep -r "thread::sleep" crates/` → 모두 `tokio::time::sleep`로 변경
- [ ] `grep -r "\.unwrap()" crates/ --exclude-dir=tests` → 적절한 에러 처리
- [ ] `migrations/014_add_performance_indexes.sql` 작성 및 실행
- [ ] `credentials.rs`에 암호화 검증 로직 추가
- [ ] Circuit breaker 테스트를 `#[tokio::test]`로 변환

### Week 2-3 (중요)
- [ ] `backtest.rs` → `backtest/` 모듈로 분리
- [ ] `analytics.rs` → `analytics/` 모듈로 분리
- [ ] Symbol clone을 Arc<String> 또는 참조로 최적화
- [ ] 모든 리스트 API에 pagination 추가
- [ ] N+1 쿼리를 JOIN으로 통합
- [ ] `lazy_static` → `OnceLock` 마이그레이션

### Week 4 (권장)
- [ ] 전략 레지스트리 구현
- [ ] HTTP 상태 코드 표준화
- [ ] 캐시 헤더 추가
- [ ] 전략 통합 테스트 추가

---

## 🚀 예상 성능 개선

| 항목 | 현재 | 개선 후 | 증가율 |
|------|------|---------|--------|
| API 응답 | 500ms | 100ms | **-80%** |
| 처리량 | 100/s | 500/s | **+400%** |
| 메모리 | 1GB | 600MB | **-40%** |
| DB 쿼리 | 200ms | 20ms | **-90%** |

---

## 🔗 주요 파일 위치

### 긴급 수정 필요
- `crates/trader-exchange/src/circuit_breaker.rs:60-70`
- `crates/trader-risk/src/{limits,manager}.rs`
- `crates/trader-api/src/routes/credentials.rs:67-86`
- `migrations/` (새 인덱스 스크립트 필요)

### 리팩토링 대상
- `crates/trader-api/src/routes/backtest.rs` (3,854 LOC)
- `crates/trader-api/src/routes/analytics.rs` (2,678 LOC)
- `crates/trader-api/src/routes/credentials.rs` (1,615 LOC)

### 최적화 대상
- `crates/trader-risk/src/stop_loss.rs` (clone 최적화)
- `crates/trader-strategy/src/engine.rs` (레지스트리 패턴)
- `crates/trader-api/src/routes/{ml,simulation}.rs` (OnceLock)

---

## 📖 빠른 코드 스니펫

### Async Sleep 패턴
```rust
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_with_delay() {
    sleep(Duration::from_millis(100)).await;
}
```

### 에러 처리 패턴
```rust
use anyhow::{Context, Result};

fn parse_decimal(s: &str) -> Result<Decimal> {
    s.parse::<Decimal>()
        .with_context(|| format!("Failed to parse: '{}'", s))
}
```

### Pagination 패턴
```rust
#[derive(Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    page: u32,
    #[serde(default = "default_per_page")]
    per_page: u32,
}

pub async fn list_items(
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Item>>> {
    let items = query()
        .limit(params.per_page)
        .offset((params.page - 1) * params.per_page)
        .fetch_all()
        .await?;
    // ...
}
```

### DB 인덱스 스크립트
```sql
-- 복합 인덱스
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_ohlcv_lookup 
ON ohlcv(symbol, timeframe, timestamp DESC);

-- 부분 인덱스
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_strategies_active 
ON strategies(user_id, is_active) 
WHERE is_active = true;
```

---

## 💡 추가 팁

### 성능 프로파일링
```bash
# CPU 프로파일링
cargo install cargo-flamegraph
cargo flamegraph --bin trader-api

# 메모리 프로파일링
cargo install cargo-valgrind
cargo valgrind run --bin trader-api
```

### 벤치마킹
```bash
# HTTP 벤치마크
wrk -t12 -c400 -d30s http://localhost:3000/api/v1/strategies

# DB 쿼리 분석
EXPLAIN (ANALYZE, BUFFERS) SELECT * FROM ohlcv WHERE ...;
```

### 코드 품질 체크
```bash
# Clippy
cargo clippy --all-targets --all-features -- -D warnings

# 보안 감사
cargo audit

# 미사용 의존성
cargo machete
```

---

**다음 단계**: [상세 문서](CODE_ANALYSIS_2026_01_30.md) 참고하여 구현 시작
