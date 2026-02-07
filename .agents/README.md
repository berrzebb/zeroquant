# ZeroQuant MCP Agents

> **Version**: 2.0.0
> **Last Updated**: 2026-02-03

ZeroQuant 프로젝트의 자동화 에이전트를 제공하는 **MCP (Model Context Protocol) 서버**입니다.

---

## 🚀 빠른 시작

### 1. 설치

상세 가이드: [`INSTALL.md`](INSTALL.md) 참조

```bash
# 의존성 설치
cd .agents
pip install -r zeroquant_mcp/requirements.txt

# Claude Code 설정에 MCP 서버 추가
# ~/.claude/config.json
```

### 2. 사용 예시

**빌드 검증**:
```python
mcp__zeroquant-agents__build_validator()
```

**코드 리뷰**:
```python
mcp__zeroquant-agents__code_reviewer(target="staged")
```

**보안 검사**:
```python
mcp__zeroquant-agents__security_reviewer(target="workspace")
```

---

## 🤖 사용 가능한 Agent

| Agent | 기능 | 실행 시간 | CI/CD |
|-------|------|--------:|:-----:|
| **build_validator** | cargo build/clippy/test/fmt 자동 실행 | 2-5분 | ✅ |
| **code_reviewer** | 코드 품질 자동 리뷰 (6개 항목) | 10-30분 | ✅ |
| **code_architect** | 아키텍처 설계 문서 생성 | 2-4시간 | ⚠️ |
| **code_simplifier** | 중복/복잡도/레거시 자동 분석 | 4-8시간 | ⚠️ |
| **ux_reviewer** | UX 자동 평가 (점수 산출) | 30-60분 | ⚠️ |
| **release_manager** | 릴리즈 자동화 (문서+커밋+푸시) | 5-10분 | ✅ |
| **security_reviewer** | 보안 취약점 자동 검출 (금융 특화) | 10-20분 | ✅ |
| **test_writer** | 테스트 자동 생성 및 커버리지 분석 | 15-30분 | ⚠️ |

---

## 📚 사용 가이드

### 일반적인 워크플로우

#### 1. 커밋 전 체크 (필수)

**풀 체크 (권장)**:
```python
# 1단계: 보안 취약점 검출
mcp__zeroquant-agents__security_reviewer(target="staged")

# 2단계: 빌드 및 테스트
mcp__zeroquant-agents__build_validator()

# 3단계: 코드 품질 리뷰
mcp__zeroquant-agents__code_reviewer(target="staged")
```

**빠른 체크**:
```python
mcp__zeroquant-agents__build_validator()
```

#### 2. 릴리즈 자동화

```python
# Preview 모드 (실제 변경 없이 미리보기)
mcp__zeroquant-agents__release_manager(mode="preview")

# 실제 릴리즈 (CHANGELOG 업데이트 + 커밋 + 푸시)
mcp__zeroquant-agents__release_manager(mode="full")
```

✅ 자동으로 수행:
- 변경사항 분석 (git diff)
- CHANGELOG.md 업데이트
- docs/todo.md 타임스탬프 업데이트
- Conventional Commits 커밋 메시지 생성
- 커밋 및 푸시

#### 3. 코드베이스 정리

```python
# 워크스페이스 전체 분석
mcp__zeroquant-agents__code_simplifier(scope="workspace")

# 특정 크레이트만
mcp__zeroquant-agents__code_simplifier(
    scope="crate",
    crate_name="trader-strategy"
)
```

✅ 자동으로 찾기:
- 중복 코드 (2,000줄 목표)
- 복잡도 초과 함수 (CC > 10)
- 레거시 코드 (주석, TODO, dead_code)

#### 4. 테스트 자동화

**새 함수 테스트 생성**:
```python
mcp__zeroquant-agents__test_writer(
    target="function",
    function_path="crates/trader-core/src/pnl.rs::calculate_pnl"
)
```

**커버리지 분석**:
```python
mcp__zeroquant-agents__test_writer(
    mode="check-coverage",
    crate_name="trader-strategy"
)
```

---

## 📖 Agent 상세 설명

### build_validator

**파라미터**:
- `target`: `"workspace"` (기본) | `"package"`
- `package_name`: 특정 패키지 이름 (target=package일 때)
- `skip_clippy`: Clippy 생략 여부 (기본: false)
- `skip_tests`: 테스트 생략 여부 (기본: false)
- `verbose`: 상세 출력 모드 (기본: false) ⭐ NEW

**출력**:
- ✅/❌ 각 단계별 상태
- 컴파일 에러/경고 메시지
- 테스트 결과
- 포맷 체크 결과

**v2.0 개선사항**:
- 멀티라인 에러 메시지 지원
- 파싱 실패 시 raw 출력 표시
- Verbose 모드로 전체 로그 확인 가능

### code_reviewer

**파라미터**:
- `target`: `"staged"` (기본) | `"commit"` | `"pr"` | `"files"`
- `commit_hash`: 커밋 해시 (target=commit일 때)
- `pr_number`: PR 번호 (target=pr일 때)
- `files`: 파일 경로 목록 (target=files일 때)

**체크 항목**:
1. 코딩 스타일 (Decimal, unwrap, 거래소 중립)
2. 보안 (SQL Injection, API 키)
3. 성능 (clone, 비동기)
4. 테스트 커버리지
5. 문서화 (Rustdoc)
6. Git 히스토리

### security_reviewer

**파라미터**:
- `target`: `"staged"` (기본) | `"commit"` | `"workspace"`
- `commit_hash`: 커밋 해시 (target=commit일 때)
- `severity`: `"all"` (기본) | `"critical"` | `"warning"`

**검출 항목**:
- 🔴 **Critical**: API 키/비밀번호 하드코딩, SQL Injection
- 🟡 **Warning**: unwrap() 사용, 민감 데이터 로깅

### release_manager

**파라미터**:
- `mode`: `"full"` (기본) | `"docs-only"` | `"preview"`
- `custom_message`: 커스텀 커밋 메시지 (선택)
- `skip_push`: 푸시 생략 여부 (기본: false)

**자동화 작업**:
- 변경사항 파일 분류 (Core/Feature/Docs/Test/CI)
- CHANGELOG.md 업데이트 (Keep a Changelog 형식)
- docs/todo.md 타임스탬프 업데이트
- Conventional Commits 메시지 생성
- 트랜잭션 기반 (에러 시 롤백)

### test_writer

**파라미터**:
- `target`: `"function"` | `"file"` | `"crate"` | `"coverage"`
- `function_path`: 함수 경로 (예: "crates/.../file.rs::fn_name")
- `file_path`: 파일 경로
- `crate_name`: 크레이트 이름
- `mode`: `"generate"` (기본) | `"analyze"` | `"check-coverage"`

**생성 내용**:
- 성공 케이스 테스트
- 에러 케이스 테스트 (Result 타입)
- Edge case 테스트 (0, 음수, 최대값)
- Mock 데이터 제안

---

## 🔧 고급 사용법

### Verbose 모드 (v2.0)

상세한 에러 분석이 필요할 때:

```python
mcp__zeroquant-agents__build_validator(verbose=True)
```

출력:
- 기본: 요약된 에러 메시지 (상위 3개)
- Verbose: 전체 stdout/stderr 포함 (`<details>` 태그)

### 특정 패키지만 검증

```python
# trader-strategy만 빌드+테스트
mcp__zeroquant-agents__build_validator(
    target="package",
    package_name="trader-strategy"
)
```

### 커스텀 릴리즈 메시지

```python
mcp__zeroquant-agents__release_manager(
    mode="full",
    custom_message="feat(strategy): Add XAA strategy implementation"
)
```

---

## 🆚 Task Tool vs MCP Agent

### 이전 (Task Tool 방식)

```python
Task(
    subagent_type="general-purpose",
    description="빌드 검증",
    prompt="""
    당신은 build-validator 에이전트입니다.

    1. cargo build --workspace
    2. cargo clippy --workspace
    3. cargo test --workspace
    4. cargo fmt --check

    결과를 리포트하세요.
    """
)
```

### 현재 (MCP Agent)

```python
mcp__zeroquant-agents__build_validator()
```

**장점**:
- ✅ **즉시 실행**: 프롬프트 작성 불필요
- ✅ **일관된 결과**: 표준화된 체크리스트
- ✅ **자동화 가능**: CI/CD 통합 용이
- ✅ **빠른 피드백**: 2-5분 내 완료

---

## 📊 프로젝트 구조

```
.agents/
├── README.md                    # 이 파일
├── INSTALL.md                   # 설치 가이드
└── zeroquant_mcp/
    ├── server.py                # MCP 서버 엔트리포인트
    ├── requirements.txt         # Python 의존성
    └── agents/
        ├── base.py              # 기본 Agent 클래스
        ├── build_validator.py   # 빌드 검증
        ├── code_reviewer.py     # 코드 리뷰
        ├── code_architect.py    # 아키텍처 설계
        ├── code_simplifier.py   # 코드 단순화
        ├── ux_reviewer.py       # UX 평가
        ├── release_manager.py   # 릴리즈 자동화
        ├── security_reviewer.py # 보안 검사
        └── test_writer.py       # 테스트 생성
```

---

## 🔄 버전 히스토리

### v2.0.0 (2026-02-03)

**Breaking Changes**:
- 템플릿 기반 → MCP 서버 기반으로 전환
- Task tool 방식 제거

**New Features**:
- ✨ `release_manager`: 릴리즈 자동화
- ✨ `security_reviewer`: 보안 취약점 검출
- ✨ `test_writer`: 테스트 자동 생성
- ✨ `verbose` 모드: 상세 출력 옵션

**Improvements**:
- 🔧 멀티라인 에러 메시지 지원
- 🔧 파싱 실패 시 raw 출력 표시
- 🔧 실제 Python 구현으로 성능 향상

### v1.0.0 (2026-02-03)

- 초기 버전 (Task tool 기반)
- 5개 에이전트 등록

---

## 📖 참고 문서

- **설치 가이드**: `INSTALL.md`
- **진행 상황 모니터링**: `MONITORING.md` 📊 ⭐ NEW
- **에이전트 상세**: `docs/specialized_agents.md`
- **개발 규칙**: `docs/development_rules.md`
- **시스템 가이드**: `CLAUDE.md`

---

## 💡 팁

### 1. 커밋 전 필수 체크

```python
# 보안 → 빌드 → 리뷰 순서
mcp__zeroquant-agents__security_reviewer(target="staged")
mcp__zeroquant-agents__build_validator()
mcp__zeroquant-agents__code_reviewer(target="staged")
```

### 2. 월간 코드 정리

```python
# 매월 1일 실행
mcp__zeroquant-agents__code_simplifier(scope="workspace")
```

### 3. 릴리즈 자동화

```python
# 작업 완료 후
mcp__zeroquant-agents__release_manager(mode="preview")  # 미리보기
mcp__zeroquant-agents__release_manager(mode="full")     # 실제 릴리즈
```

### 4. 상세 분석이 필요할 때

```python
# Verbose 모드로 전체 로그 확인
mcp__zeroquant-agents__build_validator(verbose=True)
```

---

**Questions?**
- 설치: `INSTALL.md`
- 사용법: `CLAUDE.md` § 전용 Agent MCP
- 이슈: GitHub Issues
