# ZeroQuant 운영 가이드

> **버전**: v0.8.1 | **최종 업데이트**: 2026-02-07

---

## 1. 상태 확인

### 컨테이너 상태

```bash
# 모든 컨테이너 상태 확인
docker compose ps

# 디스크 사용량 확인
df -h

# Docker 볼륨 사용량
docker system df -v
```

### 헬스체크

```bash
# Liveness 체크
curl -s http://localhost:3000/health | jq
```

**응답:**
```json
{
  "status": "healthy",
  "timestamp": "2026-01-31T12:00:00Z"
}
```

```bash
# Readiness 체크 (모든 컴포넌트 상태)
curl -s http://localhost:3000/health/ready | jq
```

**응답:**
```json
{
  "status": "healthy",
  "components": {
    "database": { "status": "healthy", "latency_ms": 5 },
    "redis": { "status": "healthy", "latency_ms": 2 },
    "exchange": { "status": "healthy" }
  },
  "timestamp": "2026-01-31T12:00:00Z"
}
```

### 자동화된 헬스체크 스크립트

```bash
#!/bin/bash
# scripts/healthcheck.sh

ENDPOINT="http://localhost:3000/health/ready"
TIMEOUT=5

response=$(curl -s -w "\n%{http_code}" --max-time $TIMEOUT $ENDPOINT)
http_code=$(echo "$response" | tail -n1)
body=$(echo "$response" | head -n -1)

if [ "$http_code" -eq 200 ]; then
    echo "✅ API healthy"
    exit 0
else
    echo "❌ API unhealthy (HTTP $http_code)"
    echo "$body"
    exit 1
fi
```

---

## 2. 로그 관리

### RUST_LOG 설정

환경변수 `RUST_LOG`로 로그 레벨을 제어합니다:

```bash
# 기본 (프로덕션 권장)
RUST_LOG=info,trader_api=info

# 디버깅
RUST_LOG=debug,trader_api=debug

# 상세 추적
RUST_LOG=trace,trader_api=trace

# 특정 모듈만 상세 로깅
RUST_LOG=info,trader_strategy=debug,trader_exchange=debug
```

**로그 레벨 기준:**
- `error`: 즉시 대응 필요 (주문 실패, DB 연결 끊김)
- `warn`: 주의 필요 (API 재시도, 비정상 데이터)
- `info`: 주요 이벤트 (주문 체결, 전략 시작/중지)
- `debug`: 디버깅 정보 (파라미터 값, 중간 계산)
- `trace`: 상세 추적 (루프 내부, 모든 함수 호출)

### 실시간 로그 조회

```bash
# API 서버 로그 (로컬 실행 시 터미널에 직접 출력)
# 에러만 필터링
cargo run --bin trader-api 2>&1 | grep -i error

# Docker로 실행 시
docker compose logs -f trader-api
docker compose logs --tail=100 trader-api
docker compose logs --since="2025-01-28T10:00:00" trader-api

# 모든 서비스 로그
docker compose logs -f

# 로그 파일로 저장
docker compose logs trader-api > logs/trader-api-$(date +%Y%m%d).log

# 로그를 파일로 리다이렉트 (로컬 실행)
cargo run --bin trader-api --release 2>&1 | tee -a logs/trader-api.log
```

### 로그 포맷

tracing 라이브러리를 사용하여 구조화된 로그를 생성합니다:

```
2026-01-31T12:00:00.123Z  INFO trader_api::routes::backtest: Backtest started strategy_id="rsi_mean_reversion" symbol="005930"
2026-01-31T12:00:01.456Z  INFO trader_api::routes::backtest: Backtest completed trades=15 total_return=2.5%
```

### 로그 로테이션

`docker-compose.yml`에 로깅 옵션 추가:

```yaml
trader-api:
  logging:
    driver: "json-file"
    options:
      max-size: "100m"
      max-file: "5"
```

### 로그 분석 명령어

```bash
# 에러 로그만 필터링
docker compose logs trader-api | grep -i error

# 특정 전략 로그
docker compose logs trader-api | grep "strategy_id="

# 주문 관련 로그
docker compose logs trader-api | grep -E "order|trade|fill"

# Rate limit 초과 확인
docker compose logs trader-api | grep "Rate limit exceeded"
```

---

## 3. 알림 설정

모든 알림 설정은 웹 UI를 통해 관리됩니다. API 키는 AES-256-GCM으로 암호화되어 데이터베이스에 저장됩니다.

### Telegram 설정

1. Telegram에서 @BotFather에게 `/newbot` 명령
2. 봇 이름과 username 설정
3. 발급된 API 토큰 복사

**Chat ID 확인:**
```bash
# 봇에게 메시지 전송 후
curl "https://api.telegram.org/bot<TOKEN>/getUpdates"
```

**UI에서 설정:**
1. 웹 대시보드 접속 → **Settings** 페이지
2. **Notifications** 섹션에서 Telegram 설정
3. Bot Token과 Chat ID 입력
4. "저장" 버튼 클릭 (자동 암호화 저장)

**테스트:**
```bash
curl -X POST http://localhost:3000/api/v1/notifications/telegram/test
```

### Discord 설정

웹 UI의 Settings → Notifications 섹션에서 Discord 웹훅 URL을 설정할 수 있습니다.

### 알림 유형

| 이벤트 | 알림 | 설명 |
|--------|------|------|
| 주문 체결 | ✅ | 매수/매도 체결 시 |
| 손절/익절 | ✅ | SL/TP 발동 시 |
| 일일 손실 한도 | ✅ | 한도 도달 시 경고 |
| 전략 시작/중지 | ✅ | 상태 변경 시 |
| API 오류 | ✅ | 거래소 연결 실패 등 |

---

## 4. 서비스 관리

### 서비스 시작

```bash
# 인프라 서비스 시작 (TimescaleDB, Redis)
docker compose up -d timescaledb redis

# API 서버 시작 (별도 터미널)
export DATABASE_URL=postgresql://trader:trader_secret@localhost:5432/trader
export REDIS_URL=redis://localhost:6379
cargo run --bin trader-api --features ml --release

# 프론트엔드 시작 (별도 터미널)
cd frontend && npm run dev
```

### 서비스 중지

```bash
# API 서버 중지: Ctrl+C로 graceful shutdown

# 인프라 서비스 중지 (데이터 유지)
docker compose down

# 특정 서비스만 중지
docker compose stop timescaledb
docker compose stop redis
```

### 서비스 재시작

```bash
# API 서버 재시작: Ctrl+C 후 다시 실행
cargo run --bin trader-api --features ml --release

# 인프라 서비스 재시작
docker compose restart timescaledb redis
```

### Graceful Shutdown

Trader API는 SIGTERM/SIGINT 신호를 받으면 graceful shutdown을 수행합니다:

1. 새로운 요청 거부
2. 진행 중인 요청 완료 대기
3. WebSocket 연결 종료
4. 리소스 정리

```bash
# 로컬 실행 시: Ctrl+C로 graceful shutdown
# 최대 30초 대기 후 종료
```

### 설정 변경 후 적용

```bash
# 환경변수 변경 시: API 서버 재시작

# 코드 변경 시: 재빌드 후 실행
cargo build --bin trader-api --features ml --release
./target/release/trader-api

# API 키/알림 설정: 웹 UI에서 변경 (재시작 불필요)
```

---

## 5. 성능 모니터링

### PostgreSQL 모니터링

```bash
# 연결 상태
docker compose exec timescaledb pg_isready -U trader

# 활성 쿼리 확인
docker compose exec timescaledb psql -U trader -d trader -c "
SELECT pid, now() - pg_stat_activity.query_start AS duration, query
FROM pg_stat_activity
WHERE state = 'active' AND query NOT LIKE '%pg_stat_activity%'
ORDER BY duration DESC
LIMIT 10;
"

# 테이블 크기
docker compose exec timescaledb psql -U trader -d trader -c "
SELECT relname, pg_size_pretty(pg_total_relation_size(relid))
FROM pg_catalog.pg_statio_user_tables
ORDER BY pg_total_relation_size(relid) DESC
LIMIT 10;
"

# TimescaleDB 청크 정보
docker compose exec timescaledb psql -U trader -d trader -c "
SELECT hypertable_name, chunk_name, range_start, range_end
FROM timescaledb_information.chunks
ORDER BY range_end DESC
LIMIT 10;
"
```

### Redis 모니터링

```bash
# Redis 상태 확인
docker compose exec redis redis-cli INFO

# 메모리 사용량
docker compose exec redis redis-cli INFO memory | grep used_memory_human

# 키 개수
docker compose exec redis redis-cli DBSIZE

# 연결 수
docker compose exec redis redis-cli INFO clients | grep connected_clients
```

### 시스템 리소스 모니터링

```bash
# Docker 컨테이너 리소스 사용량
docker stats --no-stream

# 디스크 사용량
df -h

# Docker 볼륨 사용량
docker system df -v
```

### 빠른 진단 스크립트

```bash
#!/bin/bash
# scripts/diagnose.sh

echo "=== System Status ==="
echo ""

echo "1. API Health:"
curl -s http://localhost:3000/health | jq . || echo "API not responding"
echo ""

echo "2. Docker Containers:"
docker compose ps
echo ""

echo "3. Database:"
docker compose exec -T timescaledb pg_isready -U trader
echo ""

echo "4. Redis:"
docker compose exec -T redis redis-cli ping
echo ""

echo "5. Disk Usage:"
df -h | head -5
echo ""

echo "6. Docker Stats:"
docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}"
```

---

## 6. 백업 및 복구

### 데이터베이스 백업

```bash
# 수동 백업 생성
docker compose exec timescaledb pg_dump -U trader -d trader > backups/trader_$(date +%Y%m%d_%H%M%S).sql

# 압축 백업
docker compose exec timescaledb pg_dump -U trader -d trader | gzip > backups/trader_$(date +%Y%m%d_%H%M%S).sql.gz

# 특정 테이블만 백업
docker compose exec timescaledb pg_dump -U trader -d trader -t orders -t positions > backups/orders_positions_$(date +%Y%m%d).sql
```

### 데이터베이스 복구

```bash
# 서비스 중지 (데이터 무결성을 위해)
docker compose stop trader-api

# 백업 복원
cat backups/trader_20250128_100000.sql | docker compose exec -T timescaledb psql -U trader -d trader

# 압축 백업 복원
gunzip -c backups/trader_20250128_100000.sql.gz | docker compose exec -T timescaledb psql -U trader -d trader

# 서비스 재시작
docker compose start trader-api
```

### Redis 백업

```bash
# RDB 스냅샷 생성
docker compose exec redis redis-cli BGSAVE

# 스냅샷 파일 복사
docker compose cp redis:/data/dump.rdb backups/redis_$(date +%Y%m%d).rdb
```

### Redis 복구

```bash
# 서비스 중지
docker compose stop redis

# 스냅샷 복원
docker compose cp backups/redis_20250128.rdb redis:/data/dump.rdb

# 서비스 시작
docker compose start redis
```

### 자동 백업 스크립트

`scripts/backup.sh`:

```bash
#!/bin/bash
set -e

BACKUP_DIR="/var/backups/trader"
DATE=$(date +%Y%m%d_%H%M%S)

# 백업 디렉토리 생성
mkdir -p $BACKUP_DIR

# PostgreSQL 백업
docker compose exec -T timescaledb pg_dump -U trader -d trader | gzip > $BACKUP_DIR/db_$DATE.sql.gz

# Redis 백업
docker compose exec redis redis-cli BGSAVE
sleep 5
docker compose cp redis:/data/dump.rdb $BACKUP_DIR/redis_$DATE.rdb

# 30일 이상된 백업 삭제
find $BACKUP_DIR -name "*.sql.gz" -mtime +30 -delete
find $BACKUP_DIR -name "*.rdb" -mtime +30 -delete

echo "Backup completed: $DATE"
```

### 크론잡 등록

```bash
# 매일 새벽 3시 백업
0 3 * * * /opt/trader/scripts/backup.sh >> /var/log/trader-backup.log 2>&1
```

---

## 7. 유지보수

### 정기 점검 (주간)

```bash
# 1. Docker 이미지 업데이트 확인
docker compose pull

# 2. 사용하지 않는 이미지 정리
docker image prune -f

# 3. 볼륨 정리 (주의: 사용 중인 볼륨 확인)
docker volume prune -f

# 4. 네트워크 정리
docker network prune -f

# 5. 시스템 전체 정리
docker system prune -f
```

### 버전 업그레이드

```bash
# 1. 현재 상태 백업
./scripts/backup.sh

# 2. 새 버전 빌드
git pull origin main
docker compose build trader-api

# 3. 롤링 업데이트
docker compose up -d trader-api

# 4. 헬스체크 확인
curl http://localhost:3000/health

# 5. 문제 발생 시 롤백
docker compose down
docker tag trader-api:previous trader-api:latest
docker compose up -d
```

### TimescaleDB 유지보수

```bash
# 압축 정책 확인
docker compose exec timescaledb psql -U trader -d trader -c "SELECT * FROM timescaledb_information.jobs;"

# 수동 압축 실행
docker compose exec timescaledb psql -U trader -d trader -c "CALL run_job(1000);"

# 오래된 데이터 삭제 (90일 이상)
docker compose exec timescaledb psql -U trader -d trader -c "SELECT drop_chunks('klines', interval '90 days');"
```

### 스케일링

#### 수직 스케일링 (리소스 증가)

`docker-compose.yml`에서 리소스 제한 설정:

```yaml
trader-api:
  deploy:
    resources:
      limits:
        cpus: '2'
        memory: 2G
      reservations:
        cpus: '1'
        memory: 1G
```

#### 수평 스케일링 (복제)

> 참고: 현재 구조에서 trader-api는 상태를 가지고 있어 복제 시 주의가 필요합니다.

```bash
# 복제본 수 조정 (로드밸런서 필요)
docker compose up -d --scale trader-api=3
```

### 보안 업데이트

```bash
# 1. 베이스 이미지 업데이트
docker compose pull

# 2. 이미지 재빌드
docker compose build --no-cache

# 3. 서비스 재시작
docker compose up -d
```

---

## 참고 문서

- [설치/배포 가이드](./setup_guide.md) - 인프라, 환경설정, 배포
- [데이터 수집](./data_collection.md) - Collector, 내부 시스템
- [아키텍처](./architecture.md) - 시스템 구조
