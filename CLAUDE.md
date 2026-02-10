# ZeroQuant

> v0.9.1 | 2026-02-10 | Rust 기반 다중 시장 자동화 트레이딩 시스템

## 핵심 규칙 (모든 작업에 적용)

| 규칙 | 설명 |
|------|------|
| **Decimal 필수** | `rust_decimal::Decimal` 사용. f64로 금융 계산 금지 |
| **unwrap() 금지** | 프로덕션에서 `unwrap()` / `expect()` 금지. `?` 또는 `unwrap_or` 사용 |
| **거래소 중립** | 특정 거래소 하드코딩 금지. trait 추상화 사용 |
| **레거시 즉시 제거** | 불필요 코드 즉시 삭제. "나중에 정리" 금지 |
| **주석 한글** | 모든 코드 주석은 한글로 작성 |
| **Clippy/ESLint 제로** | `#[allow(clippy::)]` 우회 금지, `any` 타입 금지, eslint-disable 금지 |
| **컨테이너 접속** | DB/Redis는 반드시 `podman exec -it <컨테이너명>` 사용 |
| **API 검증** | 외부 라이브러리 API는 Context7 MCP로 검증 후 사용 |

> 상세 규칙: `.claude/rules/` (12개 파일) | 원본: `docs/development_rules.md`

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

## 에이전트 분배 전략

> **토큰 절약 원칙**: 모든 작업을 Opus로 실행하지 않는다. 작업 성격에 맞는 모델과 에이전트를 분배한다.

### 서브에이전트 (단일 세션 내 위임)

| 작업 유형 | 서브에이전트 | 모델 | 근거 |
|-----------|-------------|------|------|
| 코드 탐색/구조 파악 | `Explore` (built-in) | haiku | 읽기 전용, 빠른 탐색 |
| 구현 계획 수립 | `Plan` (built-in) | sonnet | 계획은 sonnet으로 충분 |
| Rust 구현/리팩토링 | `rust-impl` (custom) | sonnet | 규칙 기반 구현은 sonnet |
| TypeScript 구현 | `ts-impl` (custom) | sonnet | 프론트엔드 작업 |
| 코드 리뷰 | `code-reviewer` (custom) | sonnet | 읽기 전용, 패턴 매칭 기반 |
| **UX/UI 리뷰** | **`ux-reviewer`** (custom) | **sonnet** | **접근성, 디자인 일관성, 상태 처리** |
| 빌드/테스트 검증 | `validator` (custom) | haiku | cargo check/test 실행 |
| **에러 디버깅** | **`debugger`** (custom) | **opus** | 근본 원인 분석, 깊은 추론 필요 |
| **복잡한 설계 판단** | **메인 세션** | **sonnet** | 아키텍처 결정, 트레이드오프 |

### 에이전트 팀 (다중 세션 병렬 협업)

> `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` 활성화됨. 복잡한 작업 시 팀 구성 가능.

| 시나리오 | 팀 구성 | 파이프라인 | 예상 비용 |
|----------|---------|-----------|----------|
| 크로스 레이어 기능 | `lead`(opus) → `rust-impl` + `ts-impl` + `validator` | API 구현 ∥ 프론트엔드 → 전체 검증 | ~$10-15 |
| 멀티 크레이트 변경 | `lead`(opus) → `rust-impl` × N | core → strategy ∥ api → 검증 | ~$8-12 |
| 구현 + 품질 보증 | `lead`(opus) → `rust-impl` + `code-reviewer` + `validator` | 구현 → 리뷰 → 검증 | ~$8-12 |
| 병렬 디버깅 | `lead`(opus) → `debugger`(opus) × N | 가설별 독립 조사 → 합의 | ~$15-25 |

### 활용 원칙

1. **단순 작업** → 서브에이전트 (메인 세션 내 위임, 결과만 반환)
2. **크로스 레이어/병렬 작업** → 에이전트 팀 (`lead`(opus)가 조율, 각자 독립 컨텍스트)
3. **검증은 항상 분리**: `validator`(haiku)에 위임하여 메인 컨텍스트 보호
4. **Opus는 전략적 사용**: `lead`(조율) + `debugger`(근본 원인 분석)만 opus. 나머지는 sonnet/haiku
5. **메모리 축적**: 모든 커스텀 에이전트는 `memory: project`로 학습 누적
6. **파일 충돌 방지**: 팀 모드에서 같은 파일을 두 팀원이 동시 수정하지 않도록 `lead`가 분배
7. **비용 관리**: 월 예산 $100~$200 내 운영. 팀 세션당 $10~$25 목표

### MCP 서버 활용

| MCP | 용도 | 토큰 |
|-----|------|------|
| Context7 | 외부 라이브러리 API 문서 검증 | - |
| Serena | 심볼 기반 코드 탐색 (find_symbol, find_referencing_symbols) | - |
| Playwright | 🥇 E2E 테스트, 크로스브라우저 검증, CI/CD | ~13.7k |
| Chrome DevTools | 🥈 성능 프로파일링, 네트워크 분석, Core Web Vitals | ~19.0k |

**브라우저 MCP 워크플로우**: Playwright(테스트/검증) → Chrome DevTools(성능 디버깅)

---

## 참조 문서 맵

| 작업 | 문서/스킬 |
|------|----------|
| 코딩 규칙 | `.claude/rules/` (12개 파일) |
| 전략 추가/수정 | `/add-strategy` 스킬 · `docs/STRATEGY_GUIDE.md` |
| API 엔드포인트 추가 | `/add-api` 스킬 · `docs/api.md` |
| 거래소 커넥터 추가 | `/add-exchange` 스킬 |
| DB 마이그레이션 | `/add-migration` 스킬 · `docs/migration_guide.md` |
| 프론트엔드 컴포넌트 | `/add-component` 스킬 |
| 커밋 워크플로우 | `/ship` 스킬 |
| 에러 진단 | `/diagnose` 스킬 |
| API 문서 크롤링 | `/crawl-api-spec` 스킬 |
| 환경 설정 | `docs/setup_guide.md` |
| 시스템 아키텍처 | `docs/architecture.md` |
| 작업 로드맵 | `.claude/plans/_index.md` |

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
