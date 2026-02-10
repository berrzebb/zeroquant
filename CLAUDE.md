# ZeroQuant

> v0.9.1 | 2026-02-10 | Rust 기반 다중 시장 자동화 트레이딩 시스템

## 코딩 규칙

> `.claude/rules/` (11개 파일) 자동 로드됨. 원본: `docs/development_rules.md`

---

## 아키텍처

```
trader-core (기반 - 모든 crate가 의존)
├── trader-exchange     (거래소 연동 - 7개 거래소)
├── trader-strategy     (전략 엔진 - 16개 전략)
├── trader-execution    (주문 실행 - Live/Simulated)
├── trader-risk         (리스크 관리)
├── trader-data         (데이터 수집/저장)
├── trader-analytics    (백테스트, 성과 분석)
├── trader-notification (알림)
├── trader-api          (REST/WS API - 30+ 라우트)
├── trader-cli          (CLI)
└── trader-collector    (Standalone 수집기)

frontend/              (SolidJS + TypeScript)
```

### 핵심 실행 흐름

```
MarketData → StrategyEngine → Strategy.on_market_data() → Signal[]
                                                            │
                                                     SignalProcessor
                                               ┌───────────┴───────────┐
                                          SimulatedExecutor       LiveExecutor
```

### 인프라

| 서비스 | 포트 | 접속 |
|--------|------|------|
| API | 3000 | `http://localhost:3000` |
| TimescaleDB | 5432 | `podman exec -it trader-timescaledb psql -U trader -d trader` |
| Redis | 6379 | `podman exec -it trader-redis redis-cli` |
| Frontend | 5173 | `http://localhost:5173` |

---

## 에이전트 분배

> Opus는 `debugger`만. 나머지 sonnet/haiku. 팀 구성 상세: `.claude/agents/lead.md`

| 에이전트 | 모델 | 역할 |
|----------|------|------|
| `rust-impl` | sonnet | Rust 구현 |
| `ts-impl` | sonnet | TS/SolidJS 구현 |
| `validator` | haiku | 빌드/테스트 검증 |
| `code-reviewer` | sonnet | 코드 리뷰 (readonly) |
| `ux-reviewer` | sonnet | UX/접근성 검증 (readonly) |
| `db-reviewer` | sonnet | DB/SQL 마이그레이션 |
| `debugger` | opus | 근본 원인 분석 |
| `lead` | sonnet | 팀 조율 (delegate) |

**MCP**: Context7(API 검증) · Serena(심볼 탐색) · Playwright(E2E) · Chrome DevTools(성능)

---

## 참조 문서 맵

| 작업 | 문서/스킬 |
|------|----------|
| 코딩 규칙 | `.claude/rules/` (11개 파일) |
| 전략 추가/수정 | `/add-strategy` 스킬 · `docs/STRATEGY_GUIDE.md` |
| API 엔드포인트 추가 | `/add-api` 스킬 · `docs/api.md` |
| 거래소 커넥터 추가 | `/add-exchange` 스킬 |
| DB 마이그레이션 | `/add-migration` 스킬 · `docs/migration_guide.md` · `db-reviewer` 에이전트 |
| 프론트엔드 컴포넌트 | `/add-component` 스킬 |
| 커밋 워크플로우 | `/ship` 스킬 |
| 에러 진단 | `/diagnose` 스킬 |
| API 문서 크롤링 | `/crawl-api-spec` 스킬 |
| 환경 설정 | `docs/setup_guide.md` |
| 시스템 아키텍처 | `docs/architecture.md` |
| 작업 로드맵 | `.claude/plans/_index.md` |

### 에이전트 전용 압축 문서 (`docs/ai/`)

> 사람용 `docs/`를 에이전트가 빠르게 참조할 수 있도록 압축한 문서. 에이전트는 원본 대신 이 문서를 우선 참조.

| 문서 | 원본 | 대상 에이전트 |
|------|------|--------------|
| `docs/ai/architecture-reference.md` | `architecture.md` + `trade_executor_design.md` | rust-impl, debugger, code-reviewer, lead |
| `docs/ai/api-reference.md` | `api.md` | rust-impl, ts-impl, debugger, code-reviewer |
| `docs/ai/strategy-reference.md` | `STRATEGY_GUIDE.md` | rust-impl |
| `docs/ai/troubleshooting-reference.md` | `troubleshooting.md` + `operations.md` | debugger |
| `docs/ai/infra-reference.md` | `setup_guide.md` + `operations.md` + `data_collection.md` | validator, debugger, rust-impl |

---

## 컴포넌트별 상세 (lazy-loaded)

> 아래 디렉터리의 파일을 읽을 때 해당 `CLAUDE.md`가 자동 로드됩니다.

| 컴포넌트 | CLAUDE.md 위치 | 내용 |
|----------|---------------|------|
| API 서버 | `crates/trader-api/CLAUDE.md` | 라우트 구조, AppState, 핸들러 패턴 |
| 전략 엔진 | `crates/trader-strategy/CLAUDE.md` | 16개 전략, Registry, Strategy trait |
| 거래소 연동 | `crates/trader-exchange/CLAUDE.md` | 7개 Provider, ExchangeApi trait |
| 주문 실행 | `crates/trader-execution/CLAUDE.md` | SignalProcessor, Live/Simulated |
| 코어 도메인 | `crates/trader-core/CLAUDE.md` | Signal, StrategyContext, MarketData |
| 분석/백테스트 | `crates/trader-analytics/CLAUDE.md` | BacktestEngine, GlobalScore |
| 프론트엔드 | `frontend/CLAUDE.md` | SolidJS, 컴포넌트, API 연동 |
