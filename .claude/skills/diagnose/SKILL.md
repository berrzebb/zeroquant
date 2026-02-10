---
name: diagnose
description: 에러 진단 워크플로우. 빌드/런타임/DB/API/프론트엔드 에러 체계적 분류 및 수정.
user-invocable: true
argument-hint: "[에러 메시지 또는 증상 설명]"
allowed-tools: Read, Grep, Edit, Bash(cargo *), Bash(npm *), Bash(podman *), Bash(git *), Bash(curl *)
context: fork
  agent: debugger
---

# 체계적 에러 진단

`$ARGUMENTS` 문제를 진단합니다.

---

## 1단계: 에러 분류

전달된 에러를 다음 카테고리로 분류합니다:

| 카테고리 | 키워드 | 진단 경로 |
|----------|--------|-----------|
| 컴파일 에러 | `error[E`, `cannot find`, `trait bound` | → Rust 진단 |
| 런타임 에러 | `panic`, `thread 'main'`, `SIGABRT` | → 런타임 진단 |
| DB 에러 | `sqlx`, `relation does not exist`, `connection` | → DB 진단 |
| API 에러 | `4xx`, `5xx`, `timeout`, `connection refused` | → API 진단 |
| 프론트엔드 에러 | `TypeError`, `ESLint`, `tsc` | → TS 진단 |
| 컨테이너 에러 | `podman`, `container`, `port` | → 인프라 진단 |

---

## 2단계: 카테고리별 진단

### Rust 컴파일 에러

```powershell
# 전체 빌드로 에러 컨텍스트 확인
cargo check 2>&1 | Select-Object -First 50

# 특정 패키지만
cargo check -p <package> --message-format=short
```

에러 코드별 일반적 원인:
- `E0277` (trait bound) → 타입 불일치, derive 누락
- `E0308` (mismatched types) → Decimal/f64 혼용 확인
- `E0433` (unresolved import) → use 경로 확인
- `E0599` (method not found) → 트레이트 import 누락

### DB 연결/마이그레이션 에러

```powershell
# 1. 컨테이너 상태 확인
podman ps --filter "name=trader"

# 2. DB 접속 테스트
podman exec trader-timescaledb pg_isready -U trader

# 3. 마이그레이션 상태 확인
podman exec trader-timescaledb psql -U trader -d trader -c "SELECT * FROM _sqlx_migrations ORDER BY installed_on DESC LIMIT 5"
```

### API 에러

```powershell
# 1. API 서버 상태
curl -s http://localhost:3000/health | ConvertFrom-Json

# 2. 특정 엔드포인트 테스트
curl -s http://localhost:3000/api/v1/<endpoint> | ConvertFrom-Json
```

### 프론트엔드 에러

```powershell
Push-Location frontend
npx tsc --noEmit 2>&1 | Select-Object -First 30
npx eslint src --max-warnings 0 2>&1 | Select-Object -First 30
Pop-Location
```

---

## 3단계: 수정 및 검증

1. 원인 파악 후 수정
2. 수정 후 동일 명령으로 재검증
3. 관련 테스트 실행으로 회귀 확인
4. 결과를 사용자에게 보고
