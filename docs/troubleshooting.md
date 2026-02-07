# Trader Bot 트러블슈팅 가이드

## 목차

1. [빠른 진단](#빠른-진단)
2. [일반적인 에러 및 해결책](#일반적인-에러-및-해결책)
3. [서비스별 문제 해결](#서비스별-문제-해결)
4. [성능 문제 진단](#성능-문제-진단)
5. [네트워크 문제](#네트워크-문제)
6. [데이터베이스 문제](#데이터베이스-문제)

---

## 빠른 진단

### 시스템 상태 한눈에 확인

```bash
# 1. 모든 컨테이너 상태
docker compose ps

# 2. API 헬스체크
curl -s http://localhost:3000/health | jq

# 3. 최근 에러 로그
docker compose logs --tail=50 trader-api | grep -i error

# 4. 리소스 사용량
docker stats --no-stream

# 5. 디스크 공간
df -h
```

### 문제 분류

| 증상 | 가능한 원인 | 바로가기 |
|------|------------|----------|
| API 응답 없음 | 서버 다운, 포트 충돌 | [시나리오 1](#시나리오-1-api-서버가-시작되지-않음) |
| 느린 응답 | DB 연결, 리소스 부족 | [시나리오 5](#시나리오-5-api-응답이-느림) |
| 429 에러 | Rate limit 초과 | [시나리오 3](#시나리오-3-rate-limit-초과-429-에러) |
| 거래소 연결 실패 | API 키, 네트워크 | [시나리오 6](#시나리오-6-거래소-연결-실패) |
| WebSocket 끊김 | 네트워크, 타임아웃 | [시나리오 8](#시나리오-8-websocket-연결이-자주-끊김) |

---

## 일반적인 에러 및 해결책

### 시나리오 1: API 서버가 시작되지 않음

**증상:**
- `docker compose ps`에서 trader-api가 "Exited" 또는 "Restarting" 상태
- `curl http://localhost:3000/health` 연결 거부

**진단:**

```bash
# 컨테이너 로그 확인
docker compose logs trader-api

# 종료 코드 확인
docker compose ps -a | grep trader-api
```

**가능한 원인 및 해결책:**

| 원인 | 에러 메시지 | 해결책 |
|------|------------|--------|
| 포트 충돌 | "Address already in use" | `lsof -i :3000` 후 프로세스 종료 |
| 환경변수 누락 | "JWT_SECRET not set" | `.env` 파일 확인 |
| DB 연결 실패 | "Connection refused to timescaledb" | DB 서비스 상태 확인 |
| 메모리 부족 | "Out of memory" | 시스템 메모리 확인, 컨테이너 제한 증가 |

```bash
# 포트 충돌 해결
sudo lsof -i :3000
sudo kill -9 <PID>

# 환경변수 확인
docker compose config | grep JWT_SECRET

# DB 상태 확인
docker compose exec timescaledb pg_isready -U trader
```

---

### 시나리오 2: 데이터베이스 연결 오류

**증상:**
- "Connection refused" 또는 "Connection timed out" 에러
- API가 시작되지만 요청 처리 실패

**진단:**

```bash
# DB 컨테이너 상태
docker compose ps timescaledb

# DB 로그
docker compose logs timescaledb

# 연결 테스트
docker compose exec timescaledb pg_isready -U trader -d trader
```

**해결책:**

```bash
# 1. DB 서비스 재시작
docker compose restart timescaledb

# 2. 연결 문자열 확인
docker compose exec trader-api env | grep DATABASE_URL

# 3. 네트워크 확인
docker network inspect trader-network

# 4. 볼륨 문제 시 초기화 (주의: 데이터 손실)
docker compose down -v
docker compose up -d
```

---

### 시나리오 3: Rate Limit 초과 (429 에러)

**증상:**
- HTTP 429 "Too Many Requests" 응답
- `Retry-After` 헤더 포함

**진단:**

```bash
# Rate limit 로그 확인
docker compose logs trader-api | grep "Rate limit exceeded"

# 메트릭 확인
curl -s http://localhost:3000/metrics | grep rate_limit
```

**해결책:**

```bash
# 1. 현재 설정 확인
docker compose exec trader-api env | grep RATE_LIMIT

# 2. Rate limit 증가 (.env 수정)
RATE_LIMIT_RPM=2400  # 분당 2400회로 증가

# 3. 설정 적용
docker compose up -d --force-recreate trader-api

# 4. 클라이언트 측 대응
# Retry-After 헤더 값만큼 대기 후 재시도
```

**클라이언트 코드 예시:**

```javascript
async function fetchWithRetry(url, options, maxRetries = 3) {
  for (let i = 0; i < maxRetries; i++) {
    const response = await fetch(url, options);
    if (response.status === 429) {
      const retryAfter = response.headers.get('Retry-After') || 1;
      await new Promise(r => setTimeout(r, retryAfter * 1000));
      continue;
    }
    return response;
  }
  throw new Error('Max retries exceeded');
}
```

---

### 시나리오 4: Circuit Breaker 열림 (서비스 차단)

**증상:**
- 거래소 API 호출 실패
- "Circuit breaker is open" 에러

**진단:**

```bash
# Circuit breaker 상태 로그
docker compose logs trader-api | grep -i circuit

# 거래소 연결 상태
docker compose logs trader-api | grep -i binance
```

**Circuit Breaker 상태:**

| 상태 | 설명 | 조치 |
|------|------|------|
| Closed | 정상 동작 | 없음 |
| Open | 요청 차단됨 | `reset_timeout` 대기 (기본 30초) |
| HalfOpen | 테스트 요청 허용 | 성공하면 Closed로 전환 |

**해결책:**

```bash
# 1. 거래소 상태 확인 (외부)
curl -s https://api.binance.com/api/v3/ping

# 2. 네트워크 연결 확인
docker compose exec trader-api ping api.binance.com

# 3. API 키 유효성 확인
# Binance 콘솔에서 API 키 상태 확인

# 4. 대기 후 자동 복구
# Circuit breaker는 30초 후 자동으로 HalfOpen 상태로 전환
```

---

### 시나리오 5: API 응답이 느림

**증상:**
- 요청 처리 시간 > 1초
- 간헐적인 타임아웃

**진단:**

```bash
# 레이턴시 메트릭 확인
curl -s http://localhost:3000/metrics | grep http_request_duration

# 컨테이너 리소스 사용량
docker stats trader-api timescaledb redis

# DB 슬로우 쿼리 확인
docker compose exec timescaledb psql -U trader -d trader -c "
SELECT query, calls, mean_time, total_time
FROM pg_stat_statements
ORDER BY mean_time DESC LIMIT 10;
"
```

**해결책:**

```bash
# 1. 리소스 제한 증가 (docker-compose.yml)
deploy:
  resources:
    limits:
      cpus: '2'
      memory: 2G

# 2. DB 인덱스 확인
docker compose exec timescaledb psql -U trader -d trader -c "
SELECT indexname, indexdef FROM pg_indexes WHERE tablename = 'orders';
"

# 3. Redis 캐시 확인
docker compose exec redis redis-cli INFO stats | grep hits

# 4. 연결 풀 조정
DATABASE_MAX_CONNECTIONS=20
REDIS_MAX_CONNECTIONS=10
```

---

### 시나리오 6: 거래소 연결 실패

**증상:**
- "Exchange connection failed" 에러
- 주문이 전송되지 않음

**진단:**

```bash
# 거래소 API 로그
docker compose logs trader-api | grep -i "binance\|exchange"

# 환경변수 확인 (시크릿 마스킹)
docker compose exec trader-api env | grep -i binance | sed 's/=.*/=***/'
```

**가능한 원인:**

| 원인 | 확인 방법 | 해결책 |
|------|----------|--------|
| API 키 오류 | 로그에 "Invalid API key" | 키 재발급 |
| IP 제한 | 로그에 "IP not whitelisted" | Binance에서 IP 추가 |
| 권한 부족 | 로그에 "Permission denied" | API 권한 설정 |
| 네트워크 차단 | ping 실패 | 방화벽 확인 |
| Testnet 설정 | 실거래 API + Testnet 키 | BINANCE_TESTNET 확인 |

```bash
# API 키 테스트
curl -H "X-MBX-APIKEY: $BINANCE_API_KEY" \
  "https://api.binance.com/api/v3/account"

# Testnet 테스트
curl -H "X-MBX-APIKEY: $BINANCE_TESTNET_API_KEY" \
  "https://testnet.binance.vision/api/v3/account"
```

---

### 시나리오 7: 인증 실패 (401 Unauthorized)

**증상:**
- 모든 API 요청에서 401 응답
- "Invalid token" 또는 "Token expired" 에러

**진단:**

```bash
# 인증 관련 로그
docker compose logs trader-api | grep -i "auth\|jwt\|token"

# JWT 시크릿 설정 확인
docker compose exec trader-api env | grep JWT
```

**해결책:**

```bash
# 1. JWT 시크릿 일치 확인
# 서버와 클라이언트가 같은 시크릿을 사용하는지 확인

# 2. 토큰 만료 시간 확인
JWT_EXPIRY_HOURS=24  # 기본값

# 3. 토큰 디코딩 (jwt.io에서 확인)
echo $TOKEN | cut -d'.' -f2 | base64 -d | jq

# 4. 새 토큰 발급
curl -X POST http://localhost:3000/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "password"}'
```

---

### 시나리오 8: WebSocket 연결이 자주 끊김

**증상:**
- WebSocket 연결이 자주 끊어짐
- "Connection closed" 에러

**진단:**

```bash
# WebSocket 관련 로그
docker compose logs trader-api | grep -i websocket

# 연결 수 메트릭
curl -s http://localhost:3000/metrics | grep websocket
```

**해결책:**

```bash
# 1. Ping/Pong 간격 확인
# 클라이언트에서 30초마다 ping 전송

# 2. 프록시 타임아웃 설정 (Nginx)
proxy_read_timeout 300;
proxy_connect_timeout 300;
proxy_send_timeout 300;

# 3. 로드밸런서 설정
# Sticky session 활성화

# 4. 클라이언트 재연결 로직
const ws = new WebSocket('ws://localhost:3000/ws');
ws.onclose = () => {
  setTimeout(() => reconnect(), 5000);
};
```

---

### 시나리오 9: 디스크 공간 부족

**증상:**
- "No space left on device" 에러
- 서비스 중단

**진단:**

```bash
# 디스크 사용량
df -h

# Docker 볼륨 사용량
docker system df -v

# 큰 파일 찾기
du -sh /var/lib/docker/volumes/* | sort -h | tail -10
```

**해결책:**

```bash
# 1. Docker 정리
docker system prune -af
docker volume prune -f

# 2. 오래된 로그 삭제
docker compose logs --tail=0  # 로그 truncate

# 3. 오래된 데이터 삭제 (TimescaleDB)
docker compose exec timescaledb psql -U trader -d trader -c "
SELECT drop_chunks('klines', interval '30 days');
"

# 4. 백업 파일 정리
find /var/backups/trader -mtime +30 -delete
```

---

### 시나리오 10: 메모리 부족 (OOM)

**증상:**
- 컨테이너가 갑자기 종료됨
- "Killed" 또는 "OOMKilled" 상태

**진단:**

```bash
# OOM 확인
docker inspect trader-api | jq '.[0].State.OOMKilled'

# 메모리 사용량 확인
docker stats --no-stream

# 시스템 메모리
free -h
```

**해결책:**

```bash
# 1. 메모리 제한 증가 (docker-compose.yml)
deploy:
  resources:
    limits:
      memory: 2G
    reservations:
      memory: 1G

# 2. 스왑 추가 (호스트)
sudo fallocate -l 4G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile

# 3. 메모리 누수 확인
# 시간에 따른 메모리 증가 패턴 확인
docker stats trader-api

# 4. Redis 메모리 정책 확인
docker compose exec redis redis-cli CONFIG GET maxmemory-policy
```

---

### 시나리오 11: CORS 에러

**증상:**
- 브라우저에서 "CORS policy" 에러
- Preflight 요청 실패

**진단:**

```bash
# CORS 설정 확인
docker compose exec trader-api env | grep CORS

# OPTIONS 요청 테스트
curl -X OPTIONS http://localhost:3000/api/v1/health \
  -H "Origin: http://localhost:5173" \
  -H "Access-Control-Request-Method: GET" \
  -v
```

**해결책:**

```bash
# 1. CORS_ORIGINS 설정 (.env)
CORS_ORIGINS=http://localhost:5173,https://your-dashboard.com

# 2. 개발 모드 (모든 origin 허용)
# CORS_ORIGINS를 설정하지 않으면 모든 origin 허용

# 3. 설정 적용
docker compose up -d --force-recreate trader-api
```

---

### 시나리오 12: 주문 실행 실패

**증상:**
- 주문이 거래소에 전송되지 않음
- "Order rejected" 에러

**진단:**

```bash
# 주문 관련 로그
docker compose logs trader-api | grep -i order

# 리스크 매니저 로그
docker compose logs trader-api | grep -i risk
```

**가능한 원인:**

| 원인 | 에러 메시지 | 해결책 |
|------|------------|--------|
| 잔고 부족 | "Insufficient balance" | 거래소 잔고 확인 |
| 포지션 한도 초과 | "Position limit exceeded" | 리스크 설정 조정 |
| 일일 손실 한도 | "Daily loss limit reached" | 다음 날까지 대기 |
| 최소 주문량 미달 | "Minimum quantity not met" | 주문량 증가 |
| 가격 제한 | "Price outside limits" | 가격 조정 |

```bash
# 리스크 설정 확인
docker compose exec trader-api env | grep RISK

# 포지션 상태 확인
curl -s http://localhost:3000/api/v1/positions | jq
```

---

## 성능 문제 진단

### CPU 사용량 높음

```bash
# 프로세스별 CPU
docker exec trader-api top -b -n 1

# 프로파일링 (개발 환경)
RUST_LOG=trace docker compose up trader-api
```

### 메모리 누수 의심

```bash
# 시간별 메모리 추적
while true; do
  docker stats --no-stream trader-api >> memory_log.txt
  sleep 60
done

# 분석
cat memory_log.txt | awk '{print $4}' | sort -u
```

### 네트워크 지연

```bash
# 컨테이너 간 지연
docker compose exec trader-api ping timescaledb

# 외부 API 지연
docker compose exec trader-api curl -w "@curl-format.txt" \
  -o /dev/null -s "https://api.binance.com/api/v3/ping"
```

---

## 긴급 복구 절차

### 전체 서비스 복구

```bash
# 1. 모든 서비스 중지
docker compose down

# 2. 볼륨 백업 (선택)
docker run --rm -v trader_timescaledb_data:/data -v $(pwd):/backup \
  alpine tar cvf /backup/db_emergency.tar /data

# 3. 서비스 재시작
docker compose up -d

# 4. 헬스체크
curl http://localhost:3000/health
```

### 롤백

```bash
# 이전 이미지로 롤백
docker tag trader-api:latest trader-api:broken
docker tag trader-api:previous trader-api:latest
docker compose up -d
```

---

## 지원 요청

문제가 해결되지 않으면 다음 정보와 함께 지원을 요청하세요:

1. **에러 로그**: `docker compose logs trader-api > logs.txt`
2. **시스템 정보**: `docker info`
3. **설정 (시크릿 제외)**: `docker compose config`
4. **메트릭**: `curl http://localhost:3000/metrics > metrics.txt`

---

## 참고 문서

- [설치/배포 가이드](./setup_guide.md)
- [운영 가이드](./operations.md)
