# 트러블슈팅 참조 (에이전트용)

> 원본: `docs/troubleshooting.md` (618줄) | 담당: `debugger`
> 관련: `docs/operations.md` (572줄)

## 빠른 진단 명령

```bash
# 컨테이너 상태
podman compose ps

# API 헬스체크
curl -s http://localhost:3000/health | jq
curl -s http://localhost:3000/health/ready | jq

# 최근 에러 로그
podman compose logs --tail=50 trader-api 2>&1 | Select-String "error|panic"

# 리소스 사용량
podman stats --no-stream
```

## 증상 → 원인 → 해결 맵

### API 서버

| 증상 | 원인 | 해결 |
|------|------|------|
| API 응답 없음 | 포트 충돌 | `netstat -ano | findstr :3000` → 프로세스 종료 |
| API 응답 없음 | DB 연결 실패 | `podman exec trader-timescaledb pg_isready -U trader` |
| API 응답 없음 | 환경변수 누락 | `.env` 파일 확인 (JWT_SECRET, DATABASE_URL) |
| 느린 응답 | N+1 쿼리 | `EXPLAIN ANALYZE` 실행, 인덱스 확인 |
| 느린 응답 | 커넥션 풀 고갈 | `max_connections` 확인, 풀 크기 조정 |
| 429 에러 | Rate limit | `config/default.toml`의 rate_limit 설정 확인 |

### 데이터베이스

| 증상 | 원인 | 해결 |
|------|------|------|
| Connection refused | 컨테이너 다운 | `podman compose up -d trader-timescaledb` |
| Connection timed out | 네트워크 | `podman network inspect` |
| relation does not exist | 마이그레이션 미적용 | `./target/release/trader.exe migrate status` |
| 디스크 풀 | Hypertable 데이터 누적 | retention policy 확인, 압축 정책 적용 |

### 거래소 연동

| 증상 | 원인 | 해결 |
|------|------|------|
| 거래소 연결 실패 | API 키 만료/잘못됨 | 웹 UI Settings에서 키 재설정 |
| 거래소 연결 실패 | IP 화이트리스트 | 거래소 API 관리 페이지에서 IP 등록 |
| Rate limit 초과 | 요청 빈도 과다 | ExchangeProvider의 rate_limiter 설정 확인 |
| WebSocket 끊김 | 네트워크 불안정 | reconnect 로직 확인, keepalive_interval 조정 |
| 주문 실패 | 잔고 부족/최소수량 | 거래소별 min_quantity, min_notional 확인 |

### 프론트엔드

| 증상 | 원인 | 해결 |
|------|------|------|
| 타입 에러 | ts-rs 바인딩 불일치 | `cargo test -p trader-api export_bindings` 재실행 |
| 빌드 실패 | 의존성 | `cd frontend && rm -rf node_modules && npm install` |
| CORS 에러 | API 설정 | `config/default.toml`의 cors 설정 확인 |
| WebSocket 끊김 | 프록시 타임아웃 | vite.config.ts의 ws proxy 설정 확인 |

### Collector

| 증상 | 원인 | 해결 |
|------|------|------|
| 심볼 동기화 실패 | API 키 없음 | `.env`에 PROVIDER_YAHOO_ENABLED=true 확인 |
| OHLCV 데이터 누락 | 주말/휴일 | `ohlcv` 테이블에서 날짜 범위 확인 |
| GlobalScore 0 | 지표 미계산 | `trader-collector sync-indicators` 먼저 실행 |

## Rust 에러 코드 빠른 참조

| 코드 | 원인 | 일반적 해결 |
|------|------|------------|
| E0277 | trait bound 불일치 | derive 누락 확인, From/Into 구현 |
| E0308 | 타입 불일치 | Decimal/f64 혼용 확인 |
| E0433 | unresolved import | use 경로, Cargo.toml 의존성 확인 |
| E0599 | method not found | trait import 누락 확인 |
| E0382 | moved value | clone() 추가 또는 참조 사용 |

## 성능 진단

```bash
# DB 느린 쿼리 (podman exec 필수)
podman exec -it trader-timescaledb psql -U trader -d trader -c \
  "SELECT query, mean_exec_time, calls FROM pg_stat_statements ORDER BY mean_exec_time DESC LIMIT 5;"

# 인덱스 사용률
podman exec -it trader-timescaledb psql -U trader -d trader -c \
  "SELECT tablename, indexname, idx_scan FROM pg_stat_user_indexes WHERE idx_scan = 0;"

# Hypertable 크기
podman exec -it trader-timescaledb psql -U trader -d trader -c \
  "SELECT hypertable_name, pg_size_pretty(hypertable_size(format('%I.%I', hypertable_schema, hypertable_name)::regclass)) FROM timescaledb_information.hypertables;"
```
