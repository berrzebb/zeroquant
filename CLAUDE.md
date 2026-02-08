# ZeroQuant - AI 세션 컨텍스트

> v0.8.3 | 2026-02-08

## 핵심 규칙 (모든 작업에 적용)

| 규칙 | 위반 시 영향 |
|------|-------------|
| **Decimal 필수** | `rust_decimal::Decimal` 사용. f64로 금융 계산 금지 |
| **unwrap() 금지** | 프로덕션 코드에서 `unwrap()` / `expect()` 사용 금지. `?` 또는 `unwrap_or` 사용 |
| **방어적 코딩** | 모든 에러 케이스 처리. Result/Option 활용, panic 방지 |
| **거래소 중립** | 특정 거래소 하드코딩 금지. trait 추상화 사용 |
| **레거시 즉시 제거** | 개선 시 불필요 코드 즉시 삭제. "나중에 정리" 금지 |
| **주석 한글** | 모든 코드 주석은 한글로 작성 |
| **API 검증** | 외부 라이브러리 API는 Context7로 검증 후 사용 |
| **컨테이너 접속** | DB/Redis는 반드시 `podman exec -it <컨테이너명>` 사용. `psql`, `redis-cli`, `pg_dump` 직접 실행 금지 |

> 상세 규칙 (180+개): `docs/development_rules.md`

---

## 아키텍처 맵

**Rust 기반 다중 시장 자동화 트레이딩 시스템** | 11 Crates | 16 전략 | 30+ API 라우트

### Crate 의존성 구조

```
trader-core (기반 - 모든 crate가 의존)
├── trader-exchange     (거래소 연동)
├── trader-strategy     (전략 엔진)
├── trader-execution    (주문 실행)
├── trader-risk         (리스크 관리)
├── trader-data         (데이터 수집/저장)
├── trader-analytics    (ML, 백테스트, 성과 분석) ← trader-data 의존
├── trader-notification (알림)
├── trader-api          (REST/WS API) ← 위 전체 의존하는 허브
├── trader-cli          (CLI) ← trader-api 의존
└── trader-collector    (Standalone 수집기) ← trader-core, trader-data 의존
```

### 핵심 Trait 위치

| Trait | 파일 | 역할 |
|-------|------|------|
| **Strategy** | `trader-strategy/src/traits.rs` | 전략 인터페이스 (on_market_data → Signal[]) |
| **SignalProcessor** | `trader-execution/src/signal_processor.rs` | Signal → 주문 실행 추상화 |
| **ExchangeProvider** | `trader-core/src/domain/exchange_provider.rs` | 계좌/포지션/주문 조회 |
| **AnalyticsProvider** | `trader-core/src/domain/analytics_provider.rs` | GlobalScore, Screening, RouteState 제공 |

### 핵심 도메인 타입 위치

| 타입 | 파일 | 설명 |
|------|------|------|
| **Signal** | `trader-core/src/domain/signal.rs` | 매매 신호 (Entry/Exit/AddToPosition/ReducePosition) |
| **StrategyContext** | `trader-core/src/domain/context.rs` | 전략 주입 컨텍스트 (계좌+분석+시장 데이터) |
| **MarketData** | `trader-core/src/domain/market_data.rs` | 시장 데이터 (Kline, Ticker, OrderBook) |
| **TradeResult** | `trader-execution/src/signal_processor.rs` | 체결 결과 (수량, 가격, 수수료, 실현손익) |
| **ProcessorPosition** | `trader-execution/src/signal_processor.rs` | 포지션 (position_id, group_id 포함) |
| **GlobalScoreResult** | `trader-core/src/domain/analytics_provider.rs` | 종합 점수 (0~100) |
| **RouteState** | `trader-core/src/domain/route_state.rs` | 진입 상태 (ATTACK/ARMED/WAIT/OVERHEAT) |

### 실행 흐름

```
ExchangeProvider/BacktestEngine → StrategyEngine → Strategy.on_market_data()
                                                          │
                                                     Signal[]
                                                          │
                                                          ▼
                                                   SignalProcessor
                                               ┌──────────┴──────────┐
                                          SimulatedExecutor     LiveExecutor
                                          (백테스트/페이퍼)      (실거래)
                                               │                     │
                                               ▼                     ▼
                                          TradeResult            거래소 주문
```

| 데이터 소스 | Signal 처리 | 결과 |
|------------|-------------|------|
| ExchangeProvider | LiveExecutor | **실거래** |
| ExchangeProvider | SimulatedExecutor | **페이퍼 트레이딩** |
| BacktestEngine | SimulatedExecutor | **백테스트** |

### 전략 등록 시스템

```
전략 구현 파일: trader-strategy/src/strategies/{name}.rs
전략 모듈 등록: trader-strategy/src/strategies/mod.rs
레지스트리:     trader-strategy/src/registry.rs (register_strategy! 매크로)
공통 모듈:     trader-strategy/src/strategies/common/ (exit_config, indicators, position_sizing)
```

**16개 전략**: AssetAllocation, CandlePattern, CompoundMomentum, DayTrading, DCA(Grid/MagicSplit/InfinityBot), MarketBothside, MeanReversion, MomentumPower, MomentumSurge, PensionBot, RangeTrading, Rotation, RsiMultiTf, ScreeningBased, SectorVb, Us3xLeverage

### API 라우트 구조

```
trader-api/src/routes/mod.rs (라우터 설정)
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

**AppState**: `trader-api/src/state.rs` (DB pool, Redis, StrategyEngine, PositionTracker 등)

### StrategyContext 구조

```
StrategyContext (Arc<RwLock<>>로 전략에 주입)
├── 계좌 데이터 (1~5초 갱신)
│   ├── account, positions, pending_orders, exchange_constraints
├── 분석 데이터 (1~10분 갱신)
│   ├── global_scores, route_states, screening_results
│   ├── structural_features, market_regime, market_breadth, macro_environment
└── 시장 데이터
    └── klines_by_timeframe (멀티 타임프레임 캔들)
```

**StrategyContext 활용 원칙** (v0.8.1):
- **GlobalScore**: 고정 심볼 → 미사용 / 동적 Universe → 스크리닝 필터
- **RouteState**: 전 전략 공통 Overheat만 차단
- **상세**: `docs/STRATEGY_GUIDE.md`

---

## 작업별 참조 문서

| 작업 유형 | 참조 문서 | 핵심 내용 |
|----------|----------|----------|
| **기능 구현** | `docs/development_rules.md` | 코딩 규칙, 금지 사항, API 검증 |
| **전략 추가/수정** | `docs/STRATEGY_GUIDE.md` | 전략별 파라미터, GlobalScore 활용, StrategyContext |
| **API 엔드포인트** | `docs/api.md` | REST/WebSocket 전체 명세 (단일 소스) |
| **환경 설정** | `docs/setup_guide.md` | 환경변수 72개, .env 예시, 프로덕션 배포 |
| **데이터 수집** | `docs/data_collection.md` | Collector CLI 15개 명령어, 데몬 3그룹, 체크포인트 |
| **DB 마이그레이션** | `docs/migration_guide.md` | 검증/통합 도구, 안전한 적용 절차 |
| **운영/모니터링** | `docs/operations.md` | 로그, 알림, 백업, 성능 모니터링 |
| **시스템 아키텍처** | `docs/architecture.md` | 전체 구조, Provider 패턴, WebSocket 스트림 |
| **현재 TODO** | `docs/todo.md` | 진행 중/남은 작업 |
| **제품 요구사항** | `docs/prd.md` | PRD |

---

## 인프라 접속 (필수 숙지)

> **PostgreSQL과 Redis는 Podman 컨테이너 내부에서만 실행됨.**
> 호스트에 psql, redis-cli가 설치되어 있지 않으므로 반드시 `podman exec`을 사용해야 함.

```bash
# ✅ 올바른 접속 방법 (항상 이 방식 사용)
podman exec -it trader-timescaledb psql -U trader -d trader
podman exec -it trader-redis redis-cli

# ✅ SQL 쿼리 직접 실행
podman exec -it trader-timescaledb psql -U trader -d trader -c "SELECT COUNT(*) FROM symbol_info;"

# ✅ 테이블 목록 확인
podman exec -it trader-timescaledb psql -U trader -d trader -c "\dt"

# ❌ 절대 금지 (로컬에 설치되어 있지 않아 실패함)
psql -U trader -d trader
redis-cli
pg_dump trader
```

| 항목 | 값 |
|------|-----|
| 컨테이너 (DB) | `trader-timescaledb` |
| 컨테이너 (Redis) | `trader-redis` |
| DB 사용자/비밀번호 | `trader` / `trader_secret` |
| DB 이름 | `trader` |

| 서비스 | 포트 | 용도 |
|--------|------|------|
| Trader API | 3000 | REST/WebSocket |
| PostgreSQL | 5432 | TimescaleDB |
| Redis | 6379 | 캐시 |
| Frontend | 5173 | Vite 개발 서버 |

---

## 도구 사용 가이드

### 코드 탐색 우선순위

```
1순위: Serena MCP (심볼 기반 시맨틱 탐색)
  → find_symbol, find_referencing_symbols, get_symbols_overview
  → 클래스/함수/trait 정의와 참조 관계 파악에 최적

2순위: Task(Explore) 에이전트 (광범위 탐색)
  → 구조 파악, 패턴 분석, 3개 이상 쿼리가 필요한 탐색

3순위: Glob/Grep (단순 검색)
  → 파일명 패턴, 문자열 리터럴 매칭
```

### 서브에이전트 활용

| 상황 | 도구 |
|------|------|
| 코드 구조 파악, 패턴 분석 | `Task(Explore)` |
| 구현 계획 수립 | `Task(Plan)` 또는 `EnterPlanMode` |
| Git/빌드/테스트 | `Task(Bash)` |
| 독립된 작업 여러 개 | **병렬** `Task` 호출 (한 메시지에 복수) |
| 외부 API 검증 | Context7: `resolve-library-id` → `query-docs` |

### 커밋 전 검증 (MCP Agent)

```
mcp__zeroquant-agents__build_validator()          # 빌드 + clippy + test
mcp__zeroquant-agents__security_reviewer(target="staged")  # 보안 검토
mcp__zeroquant-agents__code_reviewer(target="staged")      # 코드 품질
```

> MCP Agent 상세: `.agents/README.md`

---

## 테스트 규칙

- **파일 분리**: `tests/{module_name}_test.rs` (소스 파일 내 `#[cfg(test)]`는 private 로직만)
- **Public API만 테스트**: trait 메서드/public 함수의 입력→출력 검증
- **전체 케이스 커버**: 정상, 경계값(0/최대/음수/빈값), 에러, 엣지케이스, 상태전이
- **리팩토링 시 테스트도 재작성**: 기존 테스트 폐기 후 새 public API 기준 재작성

---

## 마이그레이션

```bash
trader migrate verify [--verbose]                    # 검증
trader migrate consolidate --output migrations_v2    # 통합
trader migrate apply --dir migrations_v2             # 적용
```

> 상세: `docs/migration_guide.md`
