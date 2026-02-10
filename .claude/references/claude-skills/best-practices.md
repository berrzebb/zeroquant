# Claude Agent Skills 작성 지침

## 1. 핵심 원칙

- **간결함 우선**: Context window는 공유 자원 (시스템 프롬프트, 대화 기록, 다른 Skills 메타데이터와 공유)
- **Claude가 이미 아는 내용은 생략**: "Claude가 정말 이 설명이 필요한가?" 질문할 것
- **적절한 자유도 설정**:
  - High: 텍스트 기반 지침 (여러 접근법이 유효할 때)
  - Medium: 매개변수가 있는 pseudocode (선호 패턴 존재, 변형 허용)
  - Low: 특정 스크립트, 매개변수 최소화 (작업이 취약하고 일관성 중요할 때)

---

## 2. SKILL.md 구조

### 메타데이터 요구사항

| 필드 | 제한 | 규칙 |
|------|------|------|
| `name` | 64자 이내 | 소문자, 숫자, 하이픈만 사용. 예약어 금지 (anthropic, claude) |
| `description` | 1024자 이내 | 비어있으면 안 됨. **3인칭**으로 작성 |

### description 작성 규칙

- **3인칭 필수**: description은 시스템 프롬프트에 주입됨
  - ✅ "Processes Excel files and generates reports"
  - ❌ "I can help you process Excel files"
  - ❌ "You can use this to process Excel files"
- **구체적으로 작성**: 무엇을 하는지 + 언제 사용하는지 포함

```yaml
# 좋은 예시
# Good example
description: Extract text and tables from PDF files, fill forms, merge documents. Use when working with PDF files or when the user mentions PDFs, forms, or document extraction.

# 나쁜 예시
# Bad example
description: Helps with documents
```

---

## 3. 명명 규칙

- **gerund 형식 권장** (동사 + ing):
  - ✅ `processing-pdfs`, `analyzing-spreadsheets`, `managing-databases`
- **대안 허용**:
  - 명사구: `pdf-processing`, `spreadsheet-analysis`
  - 동작 지향: `process-pdfs`, `analyze-spreadsheets`
- **피할 것**:
  - 모호한 이름: `helper`, `utils`, `tools`
  - 과도하게 일반적: `documents`, `data`, `files`
  - 예약어: `anthropic-helper`, `claude-tools`

---

## 4. 점진적 공개 (Progressive Disclosure)

- SKILL.md는 **목차 역할** (500줄 이내 권장)
- 상세 내용은 별도 파일로 분리
- **참조는 1단계 깊이만** (SKILL.md → reference.md ✅, SKILL.md → advanced.md → details.md ❌)

### 디렉토리 구조 예시

```
pdf/
├── SKILL.md              # 메인 지침 (트리거 시 로드)
├── FORMS.md              # 폼 작성 가이드 (필요 시 로드)
├── reference.md          # API 참조 (필요 시 로드)
├── examples.md           # 사용 예시 (필요 시 로드)
└── scripts/
    ├── analyze_form.py   # 유틸리티 스크립트 (실행, 로드 X)
    └── validate.py       # 검증 스크립트
```

### 참조 패턴

```markdown
# SKILL.md

**기본 사용**: [SKILL.md 내 지침]
**고급 기능**: See [advanced.md](advanced.md)
**API 참조**: See [reference.md](reference.md)
**예시**: See [examples.md](examples.md)
```

---

## 5. 워크플로우 패턴

### 복잡한 작업은 체크리스트로 분리

```markdown
## PDF 폼 작성 워크플로우
## PDF Form Filling Workflow

다음 체크리스트를 복사하여 진행 상황을 추적하세요:
Copy this checklist and track your progress:

```
Task Progress:
- [ ] Step 1: 폼 분석 (analyze_form.py 실행)
- [ ] Step 2: 필드 매핑 생성 (fields.json 편집)
- [ ] Step 3: 매핑 검증 (validate_fields.py 실행)
- [ ] Step 4: 폼 작성 (fill_form.py 실행)
- [ ] Step 5: 출력 검증 (verify_output.py 실행)
```
```

### 피드백 루프 구현

- **일반 패턴**: 검증기 실행 → 오류 수정 → 반복
- 검증이 실패하면 이전 단계로 돌아가도록 명시

---

## 6. 공통 패턴

### 템플릿 패턴

```markdown
## 보고서 구조
## Report Structure

ALWAYS use this exact template structure:

```markdown
# [분석 제목]
# [Analysis Title]

## Executive summary
[핵심 발견 사항 한 문단 요약]
[One-paragraph overview of key findings]

## Key findings
- 발견 1 (지원 데이터 포함)
- Finding 1 with supporting data
```
```

### 예시 패턴 (input/output 쌍)

```markdown
## 커밋 메시지 형식
## Commit Message Format

**Example 1:**
Input: Added user authentication with JWT tokens
Output:
```
feat(auth): implement JWT-based authentication

Add login endpoint and token validation middleware
```
```

### 조건부 워크플로우

```markdown
## 문서 수정 워크플로우
## Document Modification Workflow

1. 수정 유형 결정:
1. Determine the modification type:

   **새 콘텐츠 생성?** → "생성 워크플로우" 따르기
   **Creating new content?** → Follow "Creation workflow" below
   **기존 콘텐츠 편집?** → "편집 워크플로우" 따르기
   **Editing existing content?** → Follow "Editing workflow" below
```

---

## 7. 피해야 할 패턴

| 패턴 | 문제 | 해결책 |
|------|------|--------|
| Windows 경로 (`\`) | Unix에서 오류 발생 | 항상 `/` 사용 |
| 시간 의존적 정보 | 곧 outdated 됨 | "old patterns" 섹션 사용 |
| 너무 많은 옵션 | 혼란 유발 | 기본값 제공 + 대안 하나만 |
| 깊은 참조 중첩 | 부분 읽기 위험 | 1단계 깊이만 |
| 도구 설치 가정 | 실행 실패 | 명시적으로 설치 지침 포함 |

### 옵션 제시 예시

```markdown
# 나쁜 예시 (너무 많은 선택지)
# Bad example (too many choices)
"pypdf, pdfplumber, PyMuPDF, pdf2image 중 하나 사용..."
"You can use pypdf, or pdfplumber, or PyMuPDF, or pdf2image..."

# 좋은 예시 (기본값 + 대안)
# Good example (default + alternative)
"텍스트 추출에는 pdfplumber 사용:
"Use pdfplumber for text extraction:
```python
import pdfplumber
```
OCR이 필요한 스캔된 PDF는 pdf2image와 pytesseract 사용."
For scanned PDFs requiring OCR, use pdf2image with pytesseract instead."
```

---

## 8. 실행 가능한 코드 포함 시

### 에러 처리 명시

```python
def process_file(path):
    """파일 처리, 없으면 생성"""
    """Process a file, creating it if it doesn't exist."""
    try:
        with open(path) as f:
            return f.read()
    except FileNotFoundError:
        # 실패하지 않고 기본 콘텐츠로 파일 생성
        # Create file with default content instead of failing
        print(f"File {path} not found, creating default")
        with open(path, 'w') as f:
            f.write('')
        return ''
```

### 매직 넘버 금지

```python
# 좋은 예시: 자체 문서화
# Good example: Self-documenting
REQUEST_TIMEOUT = 30  # HTTP 요청은 보통 30초 내 완료
                      # HTTP requests typically complete within 30 seconds
MAX_RETRIES = 3       # 3회 재시도로 안정성과 속도 균형
                      # Three retries balances reliability vs speed

# 나쁜 예시: 매직 넘버
# Bad example: Magic numbers
TIMEOUT = 47  # 왜 47? / Why 47?
```

### 검증 가능한 중간 출력

- **패턴**: 계획 생성 → 계획 검증 → 실행 → 확인
- 검증 스크립트는 구체적인 오류 메시지 제공

---

## 9. MCP 도구 참조

- **완전한 도구 이름 사용**: `ServerName:tool_name`

```markdown
Use the BigQuery:bigquery_schema tool to retrieve table schemas.
Use the GitHub:create_issue tool to create issues.
```

---

## 10. 체크리스트

### 핵심 품질

- [ ] description이 구체적이고 키 용어 포함
- [ ] description에 무엇을 하는지 + 언제 사용하는지 포함
- [ ] SKILL.md 본문 500줄 이내
- [ ] 추가 세부사항은 별도 파일로 분리
- [ ] 시간 의존적 정보 없음 (또는 "old patterns" 섹션에)
- [ ] 일관된 용어 사용
- [ ] 예시가 구체적 (추상적 X)
- [ ] 파일 참조 1단계 깊이
- [ ] 점진적 공개 적절히 사용
- [ ] 워크플로우에 명확한 단계

### 코드 및 스크립트

- [ ] 스크립트가 Claude에 떠넘기지 않고 문제 해결
- [ ] 에러 처리 명시적이고 도움됨
- [ ] 매직 넘버 없음 (모든 값 정당화)
- [ ] 필수 패키지 지침에 나열
- [ ] Windows 경로 없음 (모두 `/`)
- [ ] 중요 작업에 검증/확인 단계
- [ ] 품질 중요 작업에 피드백 루프

### 테스트

- [ ] 최소 3개 평가 생성
- [ ] Haiku, Sonnet, Opus로 테스트
- [ ] 실제 사용 시나리오로 테스트

---

## 11. 공식 문서

- [Skills Overview](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/overview)
- [Skills Quickstart](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/quickstart)
- [Skills in Claude Code](https://code.claude.com/docs/en/skills)