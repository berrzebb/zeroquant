# ZeroQuant MCP Server 설치 가이드

## 1. 의존성 설치

```bash
cd D:\Trader\.agents
pip install -r zeroquant_mcp/requirements.txt
```

## 2. Claude Code 설정

### Windows

`C:\Users\[사용자명]\.claude\config.json` 파일을 열고 다음 추가:

```json
{
  "mcpServers": {
    "zeroquant-agents": {
      "command": "python",
      "args": [
        "D:\\Trader\\.agents\\zeroquant_mcp\\server.py"
      ]
    }
  }
}
```

### 설정 파일이 없는 경우

```bash
# 디렉토리 생성
mkdir -p ~/.claude

# 설정 파일 생성
echo '{
  "mcpServers": {
    "zeroquant-agents": {
      "command": "python",
      "args": ["D:\\\\Trader\\\\.agents\\\\zeroquant_mcp\\\\server.py"]
    }
  }
}' > ~/.claude/config.json
```

## 3. Claude Code 재시작

설정 적용을 위해 Claude Code를 완전히 종료하고 재시작합니다.

## 4. 확인

Claude Code에서 MCP 도구를 확인:

```
사용 가능한 도구 목록 표시
```

다음 도구들이 나타나야 합니다:
- `mcp__zeroquant-agents__build_validator`
- `mcp__zeroquant-agents__code_reviewer`
- `mcp__zeroquant-agents__code_architect`
- `mcp__zeroquant-agents__code_simplifier`
- `mcp__zeroquant-agents__ux_reviewer`
- `mcp__zeroquant-agents__release_manager`
- `mcp__zeroquant-agents__security_reviewer`
- `mcp__zeroquant-agents__test_writer`

## 5. 사용 예시

### 빌드 검증

```
mcp__zeroquant-agents__build_validator()
```

또는 특정 패키지만:

```
mcp__zeroquant-agents__build_validator(
    target="package",
    package_name="trader-strategy"
)
```

### 코드 리뷰

스테이지된 변경사항 리뷰:

```
mcp__zeroquant-agents__code_reviewer(target="staged")
```

특정 커밋 리뷰:

```
mcp__zeroquant-agents__code_reviewer(
    target="commit",
    commit_hash="a1b2c3d"
)
```

### 아키텍처 설계

```
mcp__zeroquant-agents__code_architect(
    feature_name="StrategyContext",
    requirements="전략 간 공유 컨텍스트 구현. 거래소 정보와 분석 결과를 통합.",
    constraints="동시성 안전, 성능 < 1ms"
)
```

### 코드 단순화 분석

워크스페이스 전체:

```
mcp__zeroquant-agents__code_simplifier(scope="workspace")
```

특정 크레이트만:

```
mcp__zeroquant-agents__code_simplifier(
    scope="crate",
    crate_name="trader-strategy"
)
```

### UX 평가

전체 평가:

```
mcp__zeroquant-agents__ux_reviewer(target="all")
```

API만:

```
mcp__zeroquant-agents__ux_reviewer(
    target="api",
    api_endpoints=["/api/strategies", "/api/backtest"]
)
```

### 릴리즈 자동화

전체 워크플로우 (변경사항 분석 → 문서 업데이트 → 커밋 → 푸시):

```
mcp__zeroquant-agents__release_manager(mode="full")
```

미리보기 (실제 변경 없음):

```
mcp__zeroquant-agents__release_manager(mode="preview")
```

문서만 업데이트 (커밋은 수동):

```
mcp__zeroquant-agents__release_manager(mode="docs-only")
```

커스텀 커밋 메시지:

```
mcp__zeroquant-agents__release_manager(
    mode="full",
    custom_message="긴급 보안 패치"
)
```

### 보안 검토

스테이지된 코드 검토:

```
mcp__zeroquant-agents__security_reviewer(target="staged")
```

워크스페이스 전체 스캔:

```
mcp__zeroquant-agents__security_reviewer(target="workspace")
```

Critical 이슈만 필터링:

```
mcp__zeroquant-agents__security_reviewer(
    target="staged",
    severity="critical"
)
```

### 테스트 생성

특정 함수에 대한 테스트 생성:

```
mcp__zeroquant-agents__test_writer(
    target="function",
    mode="generate",
    function_path="crates/trader-core/src/pnl.rs::calculate_pnl"
)
```

커버리지 분석:

```
mcp__zeroquant-agents__test_writer(
    mode="check-coverage",
    crate_name="trader-strategy"
)
```

테스트 가능성 분석:

```
mcp__zeroquant-agents__test_writer(
    mode="analyze",
    file_path="crates/trader-core/src/types/symbol.rs"
)
```

## 트러블슈팅

### MCP 서버가 나타나지 않는 경우

1. **Python 경로 확인**:
   ```bash
   which python  # Linux/Mac
   where python  # Windows
   ```

2. **로그 확인**:
   Claude Code 로그에서 MCP 서버 로드 에러 확인

3. **수동 테스트**:
   ```bash
   python D:\Trader\.agents\zeroquant_mcp\server.py
   ```

### 도구 실행이 실패하는 경우

1. **프로젝트 경로 확인**:
   server.py의 PROJECT_ROOT가 올바른지 확인

2. **Cargo 설치 확인**:
   ```bash
   cargo --version
   ```

3. **권한 확인**:
   프로젝트 디렉토리에 대한 읽기/쓰기 권한 확인

## 고급 설정

### 로깅 활성화

`server.py`의 로깅 레벨을 DEBUG로 변경:

```python
logging.basicConfig(level=logging.DEBUG)
```

### 타임아웃 조정

`base.py`의 `run_command` 메서드에서 timeout 파라미터 조정 가능.

---

**문제가 계속되면**: GitHub Issues에 리포트
