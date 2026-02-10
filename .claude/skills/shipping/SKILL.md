---
name: ship
description: 커밋 워크플로우. CHANGELOG/설계 문서 자동 업데이트 후 커밋.
disable-model-invocation: true
user-invocable: true
argument-hint: "[커밋 메시지] [--dry-run | --docs-only]"
allowed-tools: Bash(git *), Bash(cargo *), Bash(npx *), Read, Grep, Edit, Write
context: fork
---

# Ship - 릴리즈 워크플로우

## 인수 처리

- `$ARGUMENTS` 가 비어있으면 → 전체 워크플로우 실행
- `$ARGUMENTS`에 `--dry-run` 포함 → 문서 업데이트 + 커밋 메시지 미리보기만 (실제 커밋/푸시 안함)
- `$ARGUMENTS`에 `--docs-only` 포함 → CHANGELOG/설계 문서만 업데이트 (커밋/푸시 수동)
- 그 외 텍스트 → 커밋 메시지 subject로 사용

---

## 워크플로우 개요

```
1. 변경사항 분석 → 2. CHANGELOG → 3. 설계 문서 갱신 → 4. 빌드/린트 검증 → 5. 커밋 메시지 확인 → 6. 커밋 (푸시는 명시 요청 시에만)
```

---

## 1단계: 변경사항 분석

다음 명령어를 **병렬** 실행하여 현재 상태를 파악합니다:

```bash
git status
git diff --cached --stat
git diff --cached --name-only
git log --oneline -5
```

### 분석 기준

변경 파일을 다음 카테고리로 분류합니다:

| 카테고리 | 경로 패턴 | CHANGELOG 섹션 |
|----------|-----------|---------------|
| API 서버 | `crates/trader-api/` | Added/Changed/Performance |
| Collector | `crates/trader-collector/` | Performance/Changed |
| Core/전략 | `crates/trader-core/`, `crates/trader-strategy/` | Added/Changed |
| 프론트엔드 | `frontend/` | Added/Changed |
| CLI | `crates/trader-cli/` | Added/Changed |
| 마이그레이션 | `migrations/`, `migrations_v2/` | Added (DB) |
| 문서 | `docs/`, `CLAUDE.md` | (CHANGELOG에 미포함) |

### 버전 결정 규칙

| 변경 유형 | 버전 증가 |
|-----------|-----------|
| Breaking API 변경 | Major (x.0.0) |
| 새 기능, 대규모 개선 | Minor (0.x.0) |
| 버그 수정, 성능 개선, 문서 | Patch (0.0.x) |

---

## 2단계: CHANGELOG.md 업데이트

**형식**: [Keep a Changelog](https://keepachangelog.com/) 기반

```markdown
## [x.y.z] - YYYY-MM-DD

> **한 줄 요약**: 이번 릴리즈의 핵심 내용

### Performance (성능 개선이 있는 경우)
- 변경 내용

### Added (새 기능)
- 변경 내용

### Changed (기존 기능 변경)
- 변경 내용

### Fixed (버그 수정)
- 변경 내용

### Removed (제거된 기능)
- 변경 내용
```

### 작성 규칙
- 각 항목은 **사용자 관점**에서 작성 (내부 구현 X, 효과 O)
- 파일명과 함수명을 `backtick`으로 표시
- 관련 파일 경로를 괄호 안에 표기 (예: `(global_score.rs)`)

---

## 3단계: 설계 문서 갱신

다음 문서들을 순서대로 확인하고 필요시 갱신합니다:

### 3-1. CLAUDE.md

| 항목 | 위치 | 갱신 조건 |
|------|------|-----------|
| 버전 | 헤더 `> v0.x.y` | 항상 |
| 마이그레이션 수 | `프로젝트 개요` 테이블 | 마이그레이션 추가 시 |
| 프로젝트 구조 | `프로젝트 구조` 섹션 | 새 crate/디렉토리 추가 시 |
| Crate 수, API 라우트 수 | 프로젝트 개요 수치 | 변경 시 |

### 3-2. docs/architecture.md

| 항목 | 갱신 조건 |
|------|-----------|
| 버전 헤더 | 항상 (버전 + 주요 변경 설명) |
| 시스템 구성도 | 새 컴포넌트 추가 시 |
| 기술 스택 테이블 | 의존성 변경 시 |
| 크레이트 설명 | 크레이트 기능 변경 시 |

### 3-3. docs/migration_guide.md

| 항목 | 갱신 조건 |
|------|-----------|
| 통합 그룹 테이블 | 새 마이그레이션 그룹 변경 시 |
| 검출 항목 | 새 검증 규칙 추가 시 |

### 3-4. docs/todo.md

| 항목 | 갱신 조건 |
|------|-----------|
| 완료 항목 | 구현 완료된 작업을 ✅ 처리 |
| 버전 정보 | 항상 |

### 3-5. 마이그레이션 정리 (마이그레이션 변경 시에만)

마이그레이션 파일이 추가/변경된 경우:

```bash
trader migrate verify
trader migrate consolidate --output migrations_v2
trader migrate verify --dir migrations_v2
```

---

## 4단계: 빌드 및 린트 검증

변경된 코드 종류에 따라 검증을 수행합니다:

### Rust 코드 변경 시

```powershell
# 1. 포맷 검사
cargo fmt --all -- --check
# 실패 시 → cargo fmt --all 실행 후 재스테이지

# 2. Clippy 검사 (경고를 에러로 처리)
cargo clippy --all-targets -- -D warnings
# 실패 시 → 경고 항목을 직접 수정 (#[allow] 사용 금지)

# 3. 빌드 검증
cargo build --release
# 실패 시 → 컴파일 에러 수정
```

### TypeScript 코드 변경 시

```powershell
Push-Location frontend
# 1. ESLint 검사
npx eslint src --max-warnings 0
# 실패 시 → 린트 에러 직접 수정 (eslint-disable 금지)

# 2. 타입 검사
npx tsc --noEmit
# 실패 시 → 타입 에러 수정
Pop-Location
```

### 검증 실패 시 처리

1. 에러를 수정한다
2. 수정 파일을 `git add`로 다시 스테이지한다
3. 4단계를 재실행하여 모든 검증이 통과할 때까지 반복한다

⚠️ `--dry-run` 모드에서도 검증은 수행하되, 수정은 보고만 한다.

---

## 5단계: 커밋 메시지 작성 및 확인

### 형식 (Conventional Commits)

```
<type>: <version> - <subject>

<body>

Co-Authored-By: Claude <noreply@anthropic.com>
```

**Type 선택 기준**:

| Type | 사용 조건 |
|------|-----------|
| `feat` | 새 기능 추가, 대규모 개선 |
| `fix` | 버그 수정 |
| `perf` | 성능 개선 |
| `refactor` | 리팩토링 (기능 변경 없음) |
| `docs` | 문서만 변경 |
| `test` | 테스트만 추가/수정 |
| `chore` | 빌드, CI, 의존성 등 |

### 사용자 확인

⚠️ **커밋 실행 전 반드시 사용자에게 메시지를 보여주고 확인을 받습니다.**

다음 내용을 사용자에게 제시합니다:
1. 변경 요약 (카테고리별 주요 변경사항)
2. 업데이트된 문서 목록
3. 제안하는 커밋 메시지 전문
4. 스테이징에 추가할 문서 파일 목록

사용자가 승인하면 커밋을 진행합니다.

⚠️ `--dry-run` 모드에서는 여기서 종료합니다.

---

## 6단계: 커밋

```powershell
# 업데이트된 문서 스테이징
git add CHANGELOG.md CLAUDE.md docs/architecture.md docs/migration_guide.md docs/todo.md

# PowerShell에서 멀티라인 커밋 메시지
git commit -m "<type>: <version> - <subject>" -m "<body>" -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

⚠️ **푸시는 자동 수행하지 않습니다.** 사용자가 명시적으로 "push 해줘" 또는 "푸시해"라고 요청한 경우에만 `git push`를 실행합니다.

---

## 사전 조건

1. ✅ 스테이지된 파일이 있어야 함 (`git add` 완료)
2. ✅ 빌드가 성공한 상태여야 함
3. ⚠️ main 브랜치 직접 커밋 시 경고 표시

## 안전 장치

- **커밋 전 사용자 확인 필수** — 자동 커밋 없음
- **푸시는 수동 요청 시에만** — 자동 푸시 없음
- **문서 업데이트 전 기존 내용 읽기** — 덮어쓰기 방지
- **마이그레이션 변경 시 verify 필수** — 검증 없이 통합 금지

---

**최종 업데이트**: 2026-02-10
**버전**: 3.0.0
