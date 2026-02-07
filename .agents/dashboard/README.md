# MCP Agent Progress Dashboard

실시간으로 MCP 에이전트의 진행 상황을 웹 브라우저에서 모니터링합니다.

## 🚀 빠른 시작

### 1. 의존성 설치

```bash
pip install -r requirements.txt
```

### 2. 서버 실행

```bash
python server.py
```

### 3. 브라우저에서 접속

```
http://localhost:8765
```

## 📊 기능

- ✅ **실시간 로그 스트리밍**: WebSocket을 통한 즉각적인 로그 표시
- 📈 **진행률 표시**: [n/m] 형식의 진행률을 시각적으로 표시
- 🎨 **색상 코딩**: 레벨별 색상으로 구분 (시작/성공/에러/경고/진행)
- 📊 **통계**: 총 로그 수, 에러 수, 경고 수 실시간 집계
- 🔄 **자동 스크롤**: 새 로그가 추가되면 자동으로 스크롤
- 🧹 **로그 클리어**: 화면 정리 가능
- 🔌 **자동 재연결**: 서버 재시작 시 자동으로 재연결

## 💡 사용 시나리오

### Scenario 1: MCP 에이전트 실행 전

```bash
# Terminal 1: 대시보드 서버 실행
cd .agents/dashboard
python server.py

# Terminal 2: 브라우저에서 http://localhost:8765 접속

# Terminal 3: MCP 에이전트 실행 (Claude Code)
# 이제 브라우저에서 실시간으로 진행 상황 확인 가능!
```

### Scenario 2: 백그라운드로 실행

```bash
# Windows PowerShell
Start-Process python -ArgumentList "server.py" -WindowStyle Hidden -WorkingDirectory ".agents/dashboard"

# Linux/Mac
nohup python .agents/dashboard/server.py &
```

## 🎯 대시보드 UI

```
┌────────────────────────────────────────────────────┐
│ 🤖 MCP Agent Dashboard        🟢 연결됨             │
├──────────────────┬─────────────────────────────────┤
│  📊 Statistics   │  📝 Live Logs                   │
│                  │                                  │
│  총 로그: 42     │  [14:23:15] 🚀 빌드 검증 시작  │
│  에러: 0         │  [14:23:15] 🔨 [1/4] Compile   │
│  경고: 2         │  [14:23:42] ✓ [1/4] 성공       │
│  마지막: 14:23   │  [14:23:42] 📎 [2/4] Clippy    │
│                  │  ...                             │
│  진행률          │                                  │
│  ████████░░ 80%  │  [Clear] [Auto-scroll: ON]      │
│  4 / 5 단계      │                                  │
└──────────────────┴─────────────────────────────────┘
```

## 🔧 설정

### 포트 변경

`server.py`의 마지막 라인 수정:

```python
uvicorn.run(app, host="127.0.0.1", port=8765)  # 포트 변경
```

### 로그 파일 경로

기본값: `~/.claude/logs/mcp-zeroquant-agents.log`

다른 경로 사용 시 `server.py`의 `LOG_PATH` 수정

## 🐛 문제 해결

### "Log file not found"

- MCP 서버가 실행 중인지 확인
- 로그 파일 경로가 올바른지 확인: `~/.claude/logs/mcp-zeroquant-agents.log`

### WebSocket 연결 실패

- 서버가 실행 중인지 확인: `http://localhost:8765/status`
- 방화벽에서 8765 포트가 차단되지 않았는지 확인

### 로그가 표시되지 않음

- 에이전트를 실행하여 로그 파일에 데이터 생성
- 브라우저 콘솔(F12)에서 에러 메시지 확인

## 📝 로그 형식

대시보드는 다음 형식의 로그를 파싱합니다:

```
[HH:MM:SS] 메시지
```

진행률 감지:
```
[14:23:42] 🔍 [2/5] 코딩 스타일 체크 중...
                 ^^^ 진행률 추출
```

## 🌟 향후 개선 계획

- [ ] 에이전트별 탭 분리
- [ ] 히스토리 저장 및 재생
- [ ] 알림 기능 (완료/에러 시)
- [ ] 다크 모드 / 라이트 모드 토글
- [ ] 로그 필터링 (레벨별)
- [ ] 로그 검색 기능
