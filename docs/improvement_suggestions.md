# ZeroQuant 프로젝트 개선 제안서 (개인 사용 최적화)

> 작성일: 2026-01-30
> 버전: 2.0 (개인 프로젝트 맞춤)
> 분석 대상: ZeroQuant v0.3.0

---

## 📋 목차

1. [개요](#개요)
2. [현실적인 개선 사항](#1-현실적인-개선-사항)
3. [코드 품질 개선](#2-코드-품질-개선)
4. [필수 기능 개선](#3-필수-기능-개선)
5. [실용적인 모니터링](#4-실용적인-모니터링)
6. [기본 보안](#5-기본-보안)
7. [성능 개선](#6-성능-개선)
8. [우선순위 요약](#7-우선순위-요약)

---

## 개요

ZeroQuant는 **개인 사용을 위한** Rust 기반 트레이딩 시스템입니다. 27개의 전략과 ML 패턴 인식을 갖추고 있으며, 이미 실용적으로 잘 작동하는 시스템입니다.

본 문서는 **개인 프로젝트의 특성을 고려하여** 과도하지 않으면서도 실질적인 도움이 되는 개선 사항만을 제안합니다.

### ⚠️ 개인 프로젝트에 적합하지 않은 것들 (제외)

- ❌ **마이크로서비스 아키텍처** - 과도한 복잡성, 운영 부담
- ❌ **Kafka, RabbitMQ 같은 메시지 큐** - 개인 사용에는 불필요
- ❌ **서비스 디스커버리 (Consul, etcd)** - 단일 서버면 충분
- ❌ **복잡한 분산 시스템** - 유지보수 부담만 증가
- ❌ **팀 협업 도구** - 혼자 사용하는 프로젝트

### 현재 프로젝트 강점 ✅

- 잘 구조화된 크레이트 기반 모놀리스 (이게 최적!)
- 27개의 검증된 전략
- 강력한 리스크 관리
- 웹 대시보드
- 다중 거래소 지원

---

## 1. 현실적인 개선 사항

### 1.1 현재 모놀리식 구조 유지 ✅ 권장

**결론**: 지금의 단일 서비스 구조가 개인 프로젝트에 **최적**입니다.

**이유**:
- 배포 간단 (Docker 컨테이너 하나)
- 디버깅 쉬움
- 운영 부담 최소
- 성능 충분
- 복잡성 낮음

**현재 구조 (유지)**:
```
trader-api (단일 프로세스)
  ├── Strategy Engine
  ├── Risk Manager
  ├── Order Executor
  └── Data Manager
  
→ 이대로 완벽합니다!
```

---

### 1.2 간단한 모듈 분리만 고려 🟢 낮음

**필요시에만**: 특정 모듈이 너무 무거워질 때

**예시**: 백테스트 엔진이 메인 API를 느리게 만들 경우
```
옵션 1: 별도 프로세스로 백테스트 실행 (tokio::spawn)
옵션 2: CLI 도구로 백테스트 분리 (이미 있음!)
```

**구현**:
```rust
// 현재 코드에서 이미 가능
tokio::spawn(async move {
    run_backtest(request).await
});
```

---

### 1.3 간단한 이벤트 로깅 🟡 중간

**복잡한 이벤트 버스 대신**: 단순 로깅 + DB 저장

**구현**:
```rust
// 간단한 이벤트 로거
pub struct EventLogger {
    db: PgPool,
}

impl EventLogger {
    pub async fn log_trade(&self, trade: &Trade) {
        sqlx::query!(
            "INSERT INTO trade_events (symbol, side, quantity, price) VALUES ($1, $2, $3, $4)",
            trade.symbol, trade.side, trade.quantity, trade.price
        ).execute(&self.db).await.ok();
        
        tracing::info!("Trade executed: {:?}", trade);
    }
}
```

**장점**:
- 구현 간단
- 디버깅 쉬움
- 충분히 유용

---

## 2. 코드 품질 개선

### 2.1 대형 파일 리팩토링 🔴 높음

**문제**: 일부 파일이 너무 큼 → 유지보수 어려움

**실용적인 접근**:
- 급하지 않으면 천천히 하나씩
- 버그 수정하면서 같이 리팩토링
- 완벽주의 버리고 점진적으로

**우선순위**:
1. `backtest.rs` (3,323줄) - 가장 큼, 자주 수정
2. `analytics.rs` (2,325줄) - 자주 사용
3. 나머지는 급하지 않음

**리팩토링 방법**:
```
backtest.rs → backtest/
  ├── mod.rs (200줄) - 진입점
  ├── engine.rs (800줄) - 실행 로직
  ├── loader.rs (500줄) - 데이터 로딩
  └── metrics.rs (600줄) - 성과 계산
```

**시간 투자**: 주말 하루면 충분

---

### 2.2 테스트 추가 🟡 중간

**현재**: 전략 테스트 107개 ✅

**추가하면 좋은 것**:
- 리스크 매니저 테스트 (중요한 부분)
- API 엔드포인트 테스트 (회귀 방지)
- 백테스트 시나리오 테스트

**실용적 접근**:
```rust
#[tokio::test]
async fn test_risk_manager_stops_on_loss_limit() {
    // 핵심 기능만 테스트
    let mut manager = RiskManager::new(config);
    manager.record_loss(Decimal::from(9500));
    assert!(!manager.can_trade()); // 일일 한도 초과
}
```

**목표**: 완벽한 커버리지보다 **핵심 기능 보호**

---

### 2.3 린터 설정 🟢 낮음

**5분이면 끝**:

```bash
# Clippy 설정
cargo clippy --all-targets -- -D warnings

# Rustfmt 설정
cargo fmt --all

# .github/workflows/ci.yml 추가
- name: Lint
  run: cargo clippy --all-targets
```

**장점**: 실수 방지, 코드 일관성

---

## 3. 필수 기능 개선

### 3.1 전략별 리스크 설정 🔴 높음 (TODO 명시)

**현재 문제**: 모든 전략에 같은 리스크 설정

**개선**:
```rust
// 전략별 리스크 설정
pub struct StrategyConfig {
    pub name: String,
    pub parameters: Value,
    pub risk_config: RiskConfig, // 전략마다 다르게!
}

// 예시
let aggressive = RiskConfig {
    stop_loss_pct: Some(10.0),
    position_size_pct: 5.0,
};

let conservative = RiskConfig {
    stop_loss_pct: Some(3.0),
    position_size_pct: 2.0,
};
```

**구현 시간**: 2-3시간
**효과**: 전략 유연성 대폭 향상

---

### 3.2 백테스트 UI 개선 🔴 높음 (TODO 명시)

**현재 문제**: 매번 파라미터 재입력

**개선**:
1. 전략 페이지에서 전략 등록 (파라미터 포함)
2. 백테스트 페이지에서 등록된 전략 선택
3. 심볼/기간만 입력하고 실행

**구현**:
- 프론트엔드 수정 (1-2시간)
- 백엔드 API 약간 수정 (1시간)

**효과**: 사용성 크게 향상

---

### 3.3 매매 일지 🟡 중간 (TODO 명시)

**필요성**: 거래 내역 추적 및 분석

**최소 기능**:
```rust
// 간단한 일지 구조
pub struct JournalEntry {
    pub date: DateTime<Utc>,
    pub symbol: String,
    pub action: String, // "BUY", "SELL"
    pub quantity: Decimal,
    pub price: Decimal,
    pub strategy: String,
    pub pnl: Option<Decimal>,
    pub notes: String,
}

// 간단한 API
POST /api/v1/journal/entries
GET  /api/v1/journal/entries?symbol=BTC&from=2024-01-01
```

**구현 시간**: 4-5시간
**효과**: 투자 패턴 파악 가능

---

### 3.4 전략 복사 기능 🟢 낮음

**편의 기능**:
```rust
// API
POST /api/v1/strategies/{id}/clone
{
  "new_name": "RSI 공격적",
  "override_params": {
    "oversold": 20,
    "overbought": 80
  }
}
```

**구현 시간**: 1-2시간
**효과**: 전략 실험 편리

---

## 4. 실용적인 모니터링

### 4.1 기본 메트릭 🟡 중간

**과도한 APM 대신**: 간단한 Prometheus + Grafana

**최소 구성**:
```rust
use prometheus::{Counter, Histogram, register_counter, register_histogram};

lazy_static! {
    static ref TRADES_TOTAL: Counter = register_counter!(
        "trades_total", "Total number of trades"
    ).unwrap();
    
    static ref API_LATENCY: Histogram = register_histogram!(
        "api_request_duration_seconds", "API request latency"
    ).unwrap();
}
```

**Grafana 대시보드** (30분이면 설정):
- 일일 수익률
- 전략별 승률
- API 응답 시간
- 데이터베이스 성능

**구현 시간**: 2-3시간
**효과**: 시스템 상태 한눈에 파악

---

### 4.2 간단한 헬스 체크 🟢 낮음

**현재 있음** ✅ 개선만 필요

```rust
// 개선된 헬스 체크
#[derive(Serialize)]
struct HealthCheck {
    status: String,
    database: bool,
    redis: bool,
    exchange: bool,
    uptime: Duration,
}

async fn health_detail() -> Json<HealthCheck> {
    Json(HealthCheck {
        status: "healthy",
        database: check_db().await,
        redis: check_redis().await,
        exchange: check_binance().await,
        uptime: get_uptime(),
    })
}
```

**구현 시간**: 30분

---

### 4.3 로깅 개선 🟢 낮음

**현재**: tracing 사용 중 ✅

**개선**:
```rust
// 구조화된 로그
tracing::info!(
    symbol = %order.symbol,
    side = ?order.side,
    quantity = %order.quantity,
    price = %order.price,
    "Order placed"
);

// 로그 레벨 조정
ERROR: 거래 실패, 시스템 오류
WARN:  리스크 경고, API 지연
INFO:  거래 체결, 전략 상태 변경
DEBUG: 신호 생성, 계산 과정
```

**설정 파일** (.env):
```
RUST_LOG=trader_api=info,trader_strategy=debug
```

**구현 시간**: 30분

---

## 5. 기본 보안

### 5.1 API 키 관리 강화 🟡 중간

**현재**: AES-256-GCM ✅ 좋음

**추가 개선**:
```rust
// API 키에 만료일 추가
pub struct ApiCredential {
    pub exchange: String,
    pub api_key: String,
    pub secret_key: String,
    pub expires_at: Option<DateTime<Utc>>, // 추가
    pub is_active: bool, // 추가
}

// 정기적으로 체크
async fn check_expired_credentials() {
    let expired = get_expired_credentials().await;
    for cred in expired {
        notify_telegram("API 키 만료: {}", cred.exchange);
    }
}
```

**구현 시간**: 1-2시간

---

### 5.2 간단한 Rate Limiting 🟢 낮음

**개인 사용이지만**: 거래소 Rate Limit 보호

```rust
use governor::{Quota, RateLimiter};

// 거래소별 Rate Limiter
pub struct ExchangeLimiter {
    binance: RateLimiter,
    kis: RateLimiter,
}

impl ExchangeLimiter {
    pub fn new() -> Self {
        Self {
            binance: RateLimiter::direct(Quota::per_second(10)),
            kis: RateLimiter::direct(Quota::per_second(5)),
        }
    }
}
```

**구현 시간**: 1시간
**효과**: IP 밴 방지

---

### 5.3 백업 자동화 🟡 중간

**중요**: 데이터 손실 방지

**간단한 스크립트**:
```bash
#!/bin/bash
# backup.sh

DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="/home/backups"

# PostgreSQL 백업
pg_dump $DATABASE_URL > $BACKUP_DIR/db_$DATE.sql

# 설정 백업
cp .env $BACKUP_DIR/env_$DATE.backup
cp -r config/ $BACKUP_DIR/config_$DATE/

# 압축
tar -czf $BACKUP_DIR/backup_$DATE.tar.gz $BACKUP_DIR/*_$DATE.*

# 오래된 백업 삭제 (30일)
find $BACKUP_DIR -name "*.tar.gz" -mtime +30 -delete
```

**Cron 설정**:
```cron
0 2 * * * /home/trader/backup.sh
```

**구현 시간**: 30분
**효과**: 마음의 평화

---

## 6. 성능 개선

### 6.1 데이터베이스 인덱스 🟡 중간

**자주 조회하는 쿼리 최적화**:

```sql
-- 주문 조회 (심볼 + 시간)
CREATE INDEX IF NOT EXISTS idx_orders_symbol_created 
ON orders(symbol, created_at DESC);

-- 백테스트 결과 조회
CREATE INDEX IF NOT EXISTS idx_backtest_strategy 
ON backtest_results(strategy_name, created_at DESC);

-- 포지션 조회
CREATE INDEX IF NOT EXISTS idx_positions_symbol 
ON positions(symbol, status);
```

**확인**:
```sql
EXPLAIN ANALYZE
SELECT * FROM orders WHERE symbol = 'BTC/USDT' AND created_at > NOW() - INTERVAL '7 days';
```

**구현 시간**: 30분
**효과**: 쿼리 10배 이상 빠름

---

### 6.2 Redis 캐싱 (선택적) 🟢 낮음

**필요한 경우에만**:
- 실시간 시세 (1초 TTL)
- 포트폴리오 정보 (5초 TTL)

**간단한 구현**:
```rust
use redis::AsyncCommands;

pub struct SimpleCache {
    redis: redis::aio::ConnectionManager,
}

impl SimpleCache {
    pub async fn get_or_fetch<F, T>(
        &self,
        key: &str,
        ttl: usize,
        fetch: F,
    ) -> Result<T>
    where
        F: FnOnce() -> Future<Output = Result<T>>,
        T: Serialize + Deserialize,
    {
        // 캐시 확인
        if let Ok(cached) = self.redis.get::<_, String>(key).await {
            return Ok(serde_json::from_str(&cached)?);
        }
        
        // 없으면 가져오기
        let data = fetch().await?;
        let json = serde_json::to_string(&data)?;
        self.redis.set_ex(key, json, ttl).await?;
        Ok(data)
    }
}
```

**구현 시간**: 2시간

---

### 6.3 병렬 백테스트 🟢 낮음

**여러 파라미터 조합 테스트 시 유용**:

```rust
use rayon::prelude::*;

// 파라미터 그리드 탐색
let results: Vec<BacktestResult> = parameter_combinations
    .par_iter()
    .map(|params| {
        run_backtest(strategy, params)
    })
    .collect();

// 최적 파라미터 찾기
let best = results.iter()
    .max_by(|a, b| a.sharpe_ratio.cmp(&b.sharpe_ratio))
    .unwrap();
```

**구현 시간**: 1시간
**효과**: 파라미터 최적화 시간 단축

---

## 7. 우선순위 요약

### 🔴 즉시 구현 (높은 가치, 적은 공수)

| 항목 | 시간 | 효과 | 비고 |
|------|------|------|------|
| 전략별 리스크 설정 | 2-3시간 | 매우 높음 | TODO 명시 |
| 백테스트 UI 개선 | 2-3시간 | 높음 | TODO 명시 |
| 데이터베이스 인덱스 | 30분 | 높음 | 즉시 효과 |
| 린터 설정 | 5분 | 중간 | 실수 방지 |

**총 시간**: 하루면 충분 (6-7시간)

---

### 🟡 다음에 구현 (유용하지만 급하지 않음)

| 항목 | 시간 | 효과 |
|------|------|------|
| 매매 일지 | 4-5시간 | 중간 |
| 대형 파일 리팩토링 (backtest.rs) | 4-5시간 | 중간 |
| Prometheus + Grafana | 2-3시간 | 중간 |
| API 키 관리 개선 | 1-2시간 | 중간 |
| 백업 자동화 | 30분 | 중간 |
| 간단한 테스트 추가 | 2-3시간 | 중간 |

**총 시간**: 주말 하루 (14-18시간)

---

### 🟢 여유있을 때 (있으면 좋음)

| 항목 | 시간 | 효과 |
|------|------|------|
| 전략 복사 기능 | 1-2시간 | 낮음 |
| Rate Limiting | 1시간 | 낮음 |
| Redis 캐싱 | 2시간 | 낮음 |
| 병렬 백테스트 | 1시간 | 낮음 |
| 헬스 체크 개선 | 30분 | 낮음 |
| 로깅 개선 | 30분 | 낮음 |
| 이벤트 로깅 | 2시간 | 낮음 |

**총 시간**: 8-10시간

---

## 실용적인 로드맵

### Week 1: 핵심 개선 (6-7시간)
```
토요일:
- ✅ 전략별 리스크 설정 (2-3시간)
- ✅ 백테스트 UI 개선 (2-3시간)
- ✅ DB 인덱스 추가 (30분)
- ✅ Clippy/Rustfmt 설정 (5분)
```

### Week 2-3: 유용한 기능 (14-18시간)
```
주말 1:
- ✅ 매매 일지 구현 (4-5시간)
- ✅ Grafana 대시보드 (2-3시간)
- ✅ 백업 자동화 (30분)

주말 2:
- ✅ backtest.rs 리팩토링 (4-5시간)
- ✅ API 키 관리 개선 (1-2시간)
- ✅ 테스트 추가 (2-3시간)
```

### 이후: 여유있을 때
- 나머지 낮은 우선순위 항목들
- 천천히 하나씩

---

## 하지 말아야 할 것들 ❌

### 1. 마이크로서비스로 분리
- **이유**: 복잡도 10배 증가, 운영 부담, 디버깅 어려움
- **대안**: 지금 구조 그대로 유지

### 2. Kafka, RabbitMQ 도입
- **이유**: 개인 프로젝트에 과도함
- **대안**: 간단한 이벤트 로깅으로 충분

### 3. 복잡한 CI/CD 파이프라인
- **이유**: 혼자 쓰는데 불필요
- **대안**: Docker Compose로 배포하면 끝

### 4. 완벽한 테스트 커버리지 추구
- **이유**: 시간 대비 효과 낮음
- **대안**: 핵심 기능만 테스트

### 5. 엔터프라이즈급 모니터링
- **이유**: 설정 복잡, 유지보수 부담
- **대안**: Grafana 기본 대시보드면 충분

---

## 결론

### ✅ 핵심 메시지

1. **현재 구조가 최적입니다** - 모놀리스 유지!
2. **점진적 개선** - 한 번에 하나씩
3. **실용성 우선** - 과도한 엔지니어링 지양
4. **빠른 효과** - 적은 투자로 큰 개선

### 📅 권장 순서

```
1주차: 핵심 기능 (6-7시간)
  → 전략별 리스크, 백테스트 UI, DB 인덱스

2-3주차: 유용한 기능 (14-18시간)
  → 매매 일지, 리팩토링, 모니터링

이후: 여유있을 때
  → 편의 기능들
```

### 🎯 기대 효과

- **사용성**: 백테스트/리스크 설정 편리
- **성능**: DB 쿼리 10배 빠름
- **안정성**: 백업, 모니터링으로 안심
- **유지보수**: 리팩토링으로 코드 관리 쉬움

**개인 프로젝트는 단순하게!** 🚀

---

*작성일: 2026-01-30*
*버전: 2.0 - 개인 사용 최적화*
*작성자: GitHub Copilot Agent*
