# ZeroQuant MCP 에이전트 모니터링 가이드

> **버전**: v0.6.3 | **업데이트**: 2026-02-03

## 🎯 개요

모든 ZeroQuant MCP 에이전트는 실시간 진행 상황 로깅을 지원합니다.
별도 터미널에서 로그 파일을 모니터링하여 긴 작업의 진행 상황을 확인할 수 있습니다.

## 📁 로그 파일 위치

**경로**: `~/.claude/logs/mcp-zeroquant-agents.log`

- Windows: `C:\Users\<사용자명>\.claude\logs\mcp-zeroquant-agents.log`
- Linux/macOS: `~/.claude/logs/mcp-zeroquant-agents.log`

## 🔍 실시간 모니터링 방법

### Windows (PowerShell)

```powershell
# 실시간 모니터링 (마지막 50줄부터)
Get-Content $env:USERPROFILE\.claude\logs\mcp-zeroquant-agents.log -Wait -Tail 50

# 전체 로그 보기
Get-Content $env:USERPROFILE\.claude\logs\mcp-zeroquant-agents.log
```

### Linux/macOS

```bash
# 실시간 모니터링
tail -f ~/.claude/logs/mcp-zeroquant-agents.log

# 마지막 100줄 보기
tail -n 100 ~/.claude/logs/mcp-zeroquant-agents.log
```

## 📊 에이전트별 진행 상황 로그

### 1. build_validator

**단계**:
```
🔨 [1/4] Cargo build 시작...
📎 [2/4] Cargo clippy 시작...
🧪 [3/4] Cargo test 시작 (최대 10분 소요)...
🎨 [4/4] Cargo fmt check 시작...
```

**예상 소요 시간**:
- `skip_tests=true`: ~30초
- 전체 빌드: 2~3분

**출력 예시**:
```
2026-02-03 14:20:00 - BuildValidator - INFO - 🔨 [1/4] Cargo build 시작...
2026-02-03 14:20:15 - BuildValidator - INFO - 📎 [2/4] Cargo clippy 시작...
2026-02-03 14:20:30 - BuildValidator - INFO - 🧪 [3/4] Cargo test 시작 (최대 10분 소요)...
2026-02-03 14:22:45 - BuildValidator - INFO - 🎨 [4/4] Cargo fmt check 시작...
```

---

### 2. security_reviewer

**단계**:
```
🔍 [1/4] 워크스페이스 스캔 시작...
🔍 [2/4] 하드코딩된 비밀 정보 검색 중...
   ✓ 하드코딩된 비밀 정보 없음 (또는 N개 발견)
🔍 [3/4] unwrap() 사용 검색 중...
   ✓ 125개 unwrap() 발견
🔍 [4/4] 워크스페이스 스캔 완료 ✅
```

**예상 소요 시간**:
- `target="staged"`: 5~10초
- `target="workspace"`: 25~40초 (최적화됨)

**출력 예시**:
```
2026-02-03 14:32:15 - SecurityReviewer - INFO - 🔍 [1/4] 워크스페이스 스캔 시작...
2026-02-03 14:32:20 - SecurityReviewer - INFO - 🔍 [2/4] 하드코딩된 비밀 정보 검색 중...
2026-02-03 14:32:25 - SecurityReviewer - INFO -    ✓ 하드코딩된 비밀 정보 없음
2026-02-03 14:32:30 - SecurityReviewer - INFO - 🔍 [3/4] unwrap() 사용 검색 중...
2026-02-03 14:32:35 - SecurityReviewer - INFO -    ✓ 125개 unwrap() 발견
2026-02-03 14:32:40 - SecurityReviewer - INFO - 🔍 [4/4] 워크스페이스 스캔 완료 ✅
```

---

### 3. code_reviewer

**단계**:
```
📋 코드 리뷰 시작...
🔍 [1/5] 코딩 스타일 체크 중...
🔍 [2/5] 보안 체크 중...
🔍 [3/5] 성능 체크 중...
🔍 [4/5] 테스트 커버리지 체크 중...
🔍 [5/5] 문서화 체크 중...
✅ 코드 리뷰 완료
```

**예상 소요 시간**: 5~15초

---

### 4. test_writer

**단계** (mode="generate"):
```
🧪 함수 테스트 생성 시작...
🔍 함수 시그니처 검색: <function_name>
📊 함수 시그니처 분석 중...
✍️ 테스트 코드 생성 중...
✅ 테스트 생성 완료
```

**예상 소요 시간**: 5~10초

---

### 5. release_manager

**단계**:
```
🚀 릴리즈 매니저 시작...
🔍 [1/5] 변경사항 분석 중...
📝 [2/5] 문서 업데이트 중...
✍️ [3/5] 커밋 메시지 생성 중...
📦 [4/5] 커밋 실행 중...
🚀 [5/5] 원격 저장소로 푸시 중...
✅ 릴리즈 매니저 완료
```

**예상 소요 시간**: 10~20초

---

### 6. code_architect

**단계**:
```
🏗️ 아키텍처 설계 시작...
🔍 기존 코드 패턴 분석 중...
✅ 아키텍처 설계 완료
```

**예상 소요 시간**: 10~30초

---

### 7. code_simplifier

**단계**:
```
🧹 코드 단순화 분석 시작...
🔍 [1/3] 중복 코드 검색 중...
🔍 [2/3] 복잡한 함수 검색 중...
🔍 [3/3] 레거시 코드 검색 중...
✅ 분석 완료
```

**예상 소요 시간**:
- `scope="file"`: 5~10초
- `scope="crate"`: 1~5분
- `scope="workspace"`: 4~8시간 ⚠️

---

### 8. ux_reviewer

**단계**:
```
🎨 UX 평가 시작...
🔍 [1/3] API 설계 평가 중...
🔍 [2/3] UI/UX 평가 중...
🔍 [3/3] CLI 사용성 평가 중...
✅ UX 평가 완료
```

**예상 소요 시간**: 15~30초

---

## 💡 팁

### 1. staged vs workspace

**빠른 작업** (권장):
```python
# 변경된 파일만 검토 (5-10초)
mcp__zeroquant-agents__security_reviewer(target="staged")
mcp__zeroquant-agents__code_reviewer(target="staged")
```

**전체 검토** (느림):
```python
# 전체 워크스페이스 검토 (2-3분)
mcp__zeroquant-agents__security_reviewer(target="workspace")
```

### 2. 긴 작업 시 로그 모니터링

예: `build_validator` 실행 중
```bash
# 터미널 1: Claude Code 세션
# 터미널 2: 로그 모니터링
tail -f ~/.claude/logs/mcp-zeroquant-agents.log
```

### 3. 로그 레벨

현재 로그 레벨: `INFO`

변경하려면 `.agents/zeroquant_mcp/server.py`:
```python
logging.basicConfig(
    level=logging.DEBUG,  # DEBUG, INFO, WARNING, ERROR
    ...
)
```

## 🔧 문제 해결

### 로그 파일이 생성되지 않음

**원인**: `~/.claude/logs/` 디렉토리 없음

**해결**:
```bash
# Linux/macOS
mkdir -p ~/.claude/logs

# Windows (PowerShell)
New-Item -ItemType Directory -Path "$env:USERPROFILE\.claude\logs" -Force
```

MCP 서버가 자동으로 디렉토리를 생성하도록 설정되어 있습니다 (`log_dir.mkdir(parents=True, exist_ok=True)`).

### 로그가 업데이트되지 않음

**원인**: MCP 서버가 재시작되지 않음

**해결**: Claude Code 재시작 또는 MCP 서버 재시작

## 📚 관련 문서

- [MCP 설치 가이드](./INSTALL.md)
- [에이전트 사용 가이드](./README.md)
- [CLAUDE.md](../CLAUDE.md) - 전체 프로젝트 컨텍스트
