# Claude Rules 작성 지침

## 핵심 원칙

- **모듈화**: 하나의 파일에 하나의 주제
- **명확성**: 구체적이고 실행 가능한 지침
- **계층 구조**: 하위 디렉토리로 관련 규칙 그룹화
- **조건부 적용**: 필요시 `paths` 필드로 적용 범위 제한

---

## 1. Memory 계층 구조

| 우선순위 | 위치 | 용도 | 범위 |
|---------|------|------|------|
| 1 (최고) | `./CLAUDE.local.md` | 개인 프로젝트 설정 | 개인 (gitignore) |
| 2 | `./.claude/rules/*.md` | 모듈화된 규칙 | 팀 공유 |
| 3 | `./.claude/CLAUDE.md` | 프로젝트 메인 설정 | 팀 공유 |
| 4 | `./CLAUDE.md` | 프로젝트 루트 설정 | 팀 공유 |
| 5 | `~/.claude/rules/*.md` | 개인 전역 규칙 | 개인 |
| 6 (최저) | `~/.claude/CLAUDE.md` | 개인 전역 설정 | 개인 |

**규칙:** 하위 레벨 설정이 상위 레벨을 오버라이드

---

## 2. 디렉토리 구조

### 2.1 권장 구조

```
your-project/
├── .claude/
│   ├── CLAUDE.md              # 프로젝트 메인 설정
│   ├── rules/
│   │   ├── code-style.md      # 코드 스타일
│   │   ├── testing.md         # 테스트 규칙
│   │   ├── security.md        # 보안 규칙
│   │   ├── frontend/          # 프론트엔드 규칙
│   │   │   ├── react.md
│   │   │   └── styles.md
│   │   └── backend/           # 백엔드 규칙
│   │       ├── api.md
│   │       └── database.md
│   └── skills/                # Agent Skills
│       └── my-skill/
│           └── SKILL.md
└── CLAUDE.local.md            # 개인 설정 (gitignore)
```

### 2.2 파일 자동 로드

- `.claude/rules/` 하위 모든 `.md` 파일 재귀적 로드
- 하위 디렉토리도 자동 탐색
- 심볼릭 링크 지원 (순환 참조 자동 감지)

---

## 3. 규칙 파일 작성

### 3.1 기본 형식

```markdown
# [주제] 지침

## 1. 섹션명

- 구체적인 지침 1
- 구체적인 지침 2

## 2. 다음 섹션

- 관련 지침
```

### 3.2 조건부 규칙 (Path-Specific)

YAML frontmatter로 적용 범위 지정:

```markdown
---
paths:
  - "src/api/**/*.ts"
  - "src/controllers/**/*.ts"
---

# API 개발 규칙

- 모든 엔드포인트에 입력 검증 필수
- 표준 에러 응답 형식 사용
- OpenAPI 문서 주석 포함
```

**주의:** `paths` 필드가 없으면 모든 파일에 적용

### 3.3 Glob 패턴

| 패턴 | 설명 |
|------|------|
| `**/*.ts` | 모든 TypeScript 파일 |
| `src/**/*` | src 하위 모든 파일 |
| `*.md` | 루트의 마크다운 파일 |
| `src/components/*.tsx` | 특정 디렉토리 |
| `**/*.{ts,tsx}` | 다중 확장자 |
| `{src,lib}/**/*.ts` | 다중 디렉토리 |

---

## 4. Import 문법

### 4.1 파일 참조

`@path/to/file` 문법으로 다른 파일 참조:

```markdown
# 프로젝트 설정

프로젝트 개요는 @README.md 참조
사용 가능한 명령어는 @package.json 참조

## 추가 지침

- Git 워크플로우: @docs/git-instructions.md
- 개인 설정: @~/.claude/my-preferences.md
```

### 4.2 Import 규칙

- 상대 경로, 절대 경로 모두 지원
- 코드 블록 내부에서는 평가되지 않음
- 재귀 import 지원 (최대 5단계 깊이)
- `/memory` 명령으로 로드된 메모리 확인

---

## 5. 작성 모범 사례

### 5.1 DO (권장)

```markdown
# 코드 스타일 지침

## 1. 들여쓰기

- 2칸 스페이스 사용
- 탭 문자 금지

## 2. 네이밍

- 변수: camelCase
- 상수: UPPER_SNAKE_CASE
- 클래스: PascalCase
```

### 5.2 DON'T (피해야 할 것)

```markdown
# 지침

코드를 잘 작성하세요.
포맷을 맞추세요.
```

**문제점:** 모호하고 실행 불가능한 지침

---

## 6. 파일명 규칙

### 6.1 권장 패턴

| 파일명 | 용도 |
|--------|------|
| `code-style.md` | 코드 스타일 규칙 |
| `testing.md` | 테스트 규칙 |
| `security.md` | 보안 규칙 |
| `api-design.md` | API 설계 규칙 |
| `preferences.md` | 개인 환경설정 |

### 6.2 명명 규칙

- 소문자와 하이픈 사용: `code-style.md`
- 내용을 설명하는 이름 사용
- 일반적인 규칙: 단수형 (`testing.md`, `security.md`)

---

## 7. 심볼릭 링크 활용

### 7.1 공유 규칙

```bash
# 공유 규칙 디렉토리 링크
ln -s ~/shared-claude-rules .claude/rules/shared

# 개별 파일 링크
ln -s ~/company-standards/security.md .claude/rules/security.md
```

### 7.2 사용 사례

- 조직 표준 규칙 공유
- 여러 프로젝트에서 공통 규칙 재사용
- 개인 규칙을 프로젝트에 연결

---

## 8. 상위 디렉토리 상속

### 8.1 동작 방식

```
parent-folder/
├── CLAUDE.md          ← 먼저 로드
└── child-project/
    ├── CLAUDE.md      ← 나중에 로드 (오버라이드)
    └── .claude/
        └── rules/     ← 가장 높은 우선순위
```

### 8.2 활용 예

```
dotnet-with-claudecode/       # .claude 설정 포함
├── .claude/
│   └── rules/
└── repos/                    # 하위 프로젝트들
    ├── project-a/            # 상위 .claude 상속
    └── project-b/            # 상위 .claude 상속
```

---

## 9. 유용한 명령어

| 명령어 | 설명 |
|--------|------|
| `/init` | CLAUDE.md 초기화 |
| `/memory` | 로드된 메모리 확인 및 편집 |

---

## 10. 체크리스트

### 규칙 파일 작성 시

- [ ] 하나의 파일에 하나의 주제만 다룸
- [ ] 파일명이 내용을 설명함
- [ ] 구체적이고 실행 가능한 지침 작성
- [ ] 필요시에만 `paths` frontmatter 사용
- [ ] 관련 규칙은 하위 디렉토리로 그룹화
- [ ] 불릿 포인트로 구조화
- [ ] 예시 코드 포함 (필요시)

### 프로젝트 설정 시

- [ ] `.claude/rules/` 디렉토리 구조 설계
- [ ] 공유 규칙과 개인 규칙 분리
- [ ] `CLAUDE.local.md`를 `.gitignore`에 추가
- [ ] 팀원과 규칙 공유 방법 결정

---

## 11. 참고 자료

- [Claude Code Memory Documentation](https://code.claude.com/docs/en/memory)
- [Modular Rules Guide](https://code.claude.com/docs/en/memory#modular-rules-with-claude/rules/)