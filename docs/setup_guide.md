# ZeroQuant 설치 및 배포 가이드

> **버전**: v0.8.1 | **최종 업데이트**: 2026-02-07

---

## 1. 사전 요구사항

### 필수 소프트웨어

| 소프트웨어 | 최소 버전 | 확인 명령어 |
|-----------|----------|------------|
| Docker/Podman | 24.0+ | `docker --version` / `podman --version` |
| Docker Compose | 2.20+ | `docker compose version` |
| Git | 2.30+ | `git --version` |
| Rust | latest stable | `rustc --version` |
| Node.js | 18+ | `node --version` |

### 시스템 요구사항

**개발 환경:**
- CPU: 4 코어 이상
- RAM: 8GB 이상
- 디스크: 50GB 이상 (SSD 권장)

**프로덕션 환경:**
- CPU: 4 코어 이상
- RAM: 8GB 이상
- 디스크: 50GB 이상 (SSD 권장)

---

## 2. 인프라 구성

### 컨테이너 정보

> ⚠️ **PostgreSQL과 Redis는 Podman/Docker 컨테이너에서 실행됩니다.**
> 로컬 `psql` 또는 `redis-cli` 명령어를 직접 사용하지 마세요.

| 서비스 | 컨테이너명 | 포트 | 이미지 |
|--------|------------|------|--------|
| PostgreSQL | `trader-timescaledb` | 5432 | timescale/timescaledb:latest-pg15 |
| Redis | `trader-redis` | 6379 | redis:7-alpine |

### 네트워크 포트

| 서비스 | 포트 | 용도 |
|--------|------|------|
| Trader API | 3000 | REST API / WebSocket |
| PostgreSQL | 5432 | 데이터베이스 (TimescaleDB) |
| Redis | 6379 | 캐시 |
| Frontend | 5173 | Vite 개발 서버 |

### 접속 정보

| 항목 | 값 |
|------|-----|
| DB 사용자 | `trader` |
| DB 비밀번호 | `trader_secret` |
| DB 이름 | `trader` |

### 인프라 명령어

```bash
# 인프라 시작/중지
podman compose up -d          # 시작
podman compose down           # 중지
podman compose logs -f        # 로그 확인

# PostgreSQL 접속 (컨테이너 내부)
podman exec -it trader-timescaledb psql -U trader -d trader

# Redis 접속 (컨테이너 내부)
podman exec -it trader-redis redis-cli

# 컨테이너 상태 확인
podman ps
```

### ❌ 잘못된 사용 / ✅ 올바른 사용

```bash
# ❌ 로컬 psql/redis-cli 직접 사용 (설치되어 있지 않거나 연결 실패)
psql -U trader -d trader
redis-cli

# ✅ 컨테이너를 통한 접속
podman exec -it trader-timescaledb psql -U trader -d trader
podman exec -it trader-redis redis-cli
```

### 자주 사용하는 DB 쿼리

```bash
# 컨테이너 내부에서 SQL 실행
podman exec -it trader-timescaledb psql -U trader -d trader -c "SELECT COUNT(*) FROM symbol_info;"

# 테이블 목록 확인
podman exec -it trader-timescaledb psql -U trader -d trader -c "\dt"

# 마이그레이션 상태 확인
podman exec -it trader-timescaledb psql -U trader -d trader -c "SELECT * FROM _sqlx_migrations ORDER BY installed_on DESC LIMIT 5;"
```

---

## 3. 환경 설정 (.env)

### 설정 파일 생성

```bash
cp .env.example .env
```

### 필수 환경변수 (3개)

| 변수 | 설명 |
|------|------|
| `DATABASE_URL` | PostgreSQL 연결 URL |
| `ENCRYPTION_MASTER_KEY` | API 키 암호화용 마스터 키 (base64, 32bytes) |
| `JWT_SECRET` | JWT 토큰 서명용 시크릿 (32자 이상) |

### 전체 .env 예시

```bash
# ============================================================
# 필수: 데이터베이스 및 캐시
# ============================================================
DATABASE_URL=postgresql://trader:trader_secret@localhost:5432/trader
DATABASE_MAX_CONNECTIONS=10
REDIS_URL=redis://localhost:6379

# ============================================================
# 필수: 인증 및 암호화
# ============================================================
JWT_SECRET=your-super-secret-jwt-key-change-in-production
ENCRYPTION_MASTER_KEY=your-32-byte-encryption-key-here-base64

# ============================================================
# API 서버 설정 (trader-api)
# ============================================================
API_HOST=127.0.0.1               # 바인딩 주소
API_PORT=3000                    # 바인딩 포트
CORS_ORIGINS=http://localhost:5173  # 허용할 CORS origin (쉼표 구분)
RATE_LIMIT_RPM=1200              # 분당 최대 요청 수
RATE_LIMIT_DISABLED=false        # Rate Limit 비활성화
INITIAL_BALANCE=10000            # 초기 시뮬레이션 잔고
USE_REAL_EXCHANGE=false          # 실거래 모드 여부
ENABLE_MOCK_DATA=true            # Mock 데이터 시뮬레이터

# ============================================================
# 로깅
# ============================================================
RUST_LOG=info                    # 로그 레벨 (info, debug, trace)
LOG_FORMAT=pretty                # 로그 형식 (pretty, json, compact)
ENVIRONMENT=development          # 환경 (development, production)

# ============================================================
# 데이터 프로바이더 토글 (trader-collector)
# ============================================================
PROVIDER_KRX_API_ENABLED=false   # KRX OPEN API (승인 필요)
PROVIDER_YAHOO_ENABLED=true      # Yahoo Finance
NAVER_FUNDAMENTAL_ENABLED=true   # 네이버 금융 크롤러 (KR 펀더멘털)
NAVER_REQUEST_DELAY_MS=300       # 네이버 요청 딜레이 (ms)

# ============================================================
# 심볼 동기화
# ============================================================
SYMBOL_SYNC_MIN_COUNT=100        # 이 수 이하면 자동 동기화
SYMBOL_SYNC_KRX=true
SYMBOL_SYNC_BINANCE=false        # Binance 심볼 동기화
SYMBOL_SYNC_YAHOO=true
SYMBOL_SYNC_YAHOO_MAX=500

# ============================================================
# OHLCV 수집
# ============================================================
OHLCV_BATCH_SIZE=50              # 배치당 심볼 수
OHLCV_STALE_DAYS=1               # 갱신 기준 (일)
OHLCV_REQUEST_DELAY_MS=500       # 요청 딜레이 (ms)
OHLCV_CONCURRENT_LIMIT=5         # 동시 수집 심볼 수
OHLCV_TIMEFRAMES=1d              # 수집 타임프레임 (쉼표 구분)
OHLCV_TARGET_MARKETS=            # 대상 시장 (쉼표 구분, 빈값=전체)
OHLCV_MAX_RETENTION_YEARS=3      # 최대 보존 기간 (년)

# ============================================================
# Fundamental 수집
# ============================================================
FUNDAMENTAL_BATCH_SIZE=100       # 배치당 심볼 수
FUNDAMENTAL_STALE_DAYS=7         # 갱신 기준 (일)
FUNDAMENTAL_REQUEST_DELAY_MS=50  # 요청 딜레이 (ms)
FUNDAMENTAL_INCLUDE_OHLCV=true   # Fundamental 수집 시 OHLCV 함께 수집

# ============================================================
# 신호 성과 계산
# ============================================================
SIGNAL_PERFORMANCE_BATCH_SIZE=100
SIGNAL_PERFORMANCE_MIN_DAYS=1    # 발생 후 최소 경과 일수
SIGNAL_PERFORMANCE_MAX_DAYS=20   # 최대 추적 일수

# ============================================================
# 데몬 모드 (trader-collector daemon)
# ============================================================
DAEMON_INTERVAL_MINUTES=60       # Group A: 외부 API (OHLCV, Fundamental)
RANKING_INTERVAL_MINUTES=15      # Group B: 랭킹 (Score, Screening)
REALTIME_INTERVAL_MINUTES=5      # Group C: 실시간 지표 (Indicator, Macro)

# ============================================================
# 스케줄링 (시장 시간 기반)
# ============================================================
SCHEDULING_ENABLED=false         # 스케줄링 활성화
SCHEDULING_KRX_DELAY_MINUTES=60  # KRX 장마감 후 대기 (분)
SCHEDULING_SKIP_WEEKENDS=true
SCHEDULING_SKIP_HOLIDAYS=true

# ============================================================
# 관심종목
# ============================================================
PRIORITIZE_WATCHLIST=true        # 관심종목 우선 수집
```

### 시크릿 생성

```bash
# JWT 시크릿 생성
openssl rand -base64 32

# 암호화 마스터 키 생성
openssl rand -base64 32
```

> **참고**: 거래소 API 키 (Binance 등), 알림 설정 (Telegram, Discord, Slack, Email, SMS)은 환경변수가 아닌 **웹 UI**를 통해 설정합니다. UI에서 입력한 키는 AES-256-GCM으로 암호화되어 데이터베이스에 저장됩니다.

---

## 4. 개발 환경 설정

### 저장소 클론 및 빌드

```bash
git clone https://github.com/your-org/trader.git
cd trader
```

### 서비스 시작

```bash
# 1. 인프라 서비스 시작 (TimescaleDB, Redis)
docker compose up -d timescaledb redis

# 2. API 서버 실행 (별도 터미널)
export DATABASE_URL=postgresql://trader:trader_secret@localhost:5432/trader
export REDIS_URL=redis://localhost:6379
cargo run --bin trader-api --features ml --release

# 3. 프론트엔드 실행 (별도 터미널)
cd frontend && npm run dev
```

### 상태 확인

```bash
# 모든 컨테이너 상태 확인
docker compose ps

# 헬스체크
curl http://localhost:3000/health

# 상세 헬스체크
curl http://localhost:3000/health/ready
```

---

## 5. 프로덕션 배포

### Docker 이미지 빌드

```bash
# 프로덕션 이미지 빌드
docker build -t trader-api:latest .

# 버전 태그 추가
docker tag trader-api:latest trader-api:v1.0.0
```

### 프로덕션 환경변수

```bash
# .env.production
ENVIRONMENT=production
RUST_LOG=info,trader_api=info

# 보안
JWT_SECRET=<강력한-랜덤-문자열>
ENCRYPTION_MASTER_KEY=<강력한-암호화-키-base64>

# CORS (허용된 도메인만)
CORS_ORIGINS=https://your-dashboard.com

# 데이터베이스 (강력한 비밀번호)
DATABASE_URL=postgresql://trader:<strong-password>@timescaledb:5432/trader

# 거래소 API 키, 텔레그램, Discord 설정은 웹 UI에서 관리
```

### 프로덕션 실행

```bash
# 인프라만 Docker로 실행
docker compose --env-file .env.production up -d timescaledb redis

# API 서버는 systemd 서비스로 관리 (권장)
sudo systemctl start trader-api

# 또는 직접 실행
./target/release/trader-api
```

### systemd 서비스 설정

```ini
# /etc/systemd/system/trader-api.service
[Unit]
Description=ZeroQuant Trading API
After=network.target postgresql.service

[Service]
Type=simple
User=trader
WorkingDirectory=/opt/zeroquant
ExecStart=/opt/zeroquant/target/release/trader-api
Restart=always
RestartSec=10
EnvironmentFile=/opt/zeroquant/.env.production

[Install]
WantedBy=multi-user.target
```

### Nginx 리버스 프록시

```nginx
upstream trader_api {
    server localhost:3000;
}

server {
    listen 443 ssl http2;
    server_name api.your-domain.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    # API 요청
    location /api {
        proxy_pass http://trader_api;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }

    # WebSocket
    location /ws {
        proxy_pass http://trader_api;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }

    # Health check (내부 접근만)
    location /health {
        allow 10.0.0.0/8;
        deny all;
        proxy_pass http://trader_api;
    }
}
```

---

## 6. 프로덕션 체크리스트

### 보안

- [ ] JWT_SECRET 변경 (32자 이상)
- [ ] ENCRYPTION_KEY 변경
- [ ] 데이터베이스 비밀번호 변경
- [ ] CORS_ORIGINS 설정 (허용된 도메인만)
- [ ] HTTPS 설정 (TLS 인증서)
- [ ] 방화벽 규칙 설정
- [ ] Rate Limiting 활성화

### 데이터베이스

- [ ] 백업 스크립트 설정
- [ ] 자동 백업 크론잡 등록
- [ ] 복구 절차 테스트
- [ ] TimescaleDB 압축 정책 설정

### 모니터링

- [ ] 로그 수집 설정 (tracing 기반)
- [ ] 텔레그램 알림 연동
- [ ] 외부 알림 연동 (Discord 등)

### 네트워크

- [ ] 내부 포트 외부 접근 차단 (5432, 6379)
- [ ] API 포트만 외부 노출 (3000)
- [ ] 로드밸런서 설정 (선택)

### 운영

- [ ] 로그 로테이션 설정
- [ ] 디스크 모니터링
- [ ] 자동 재시작 정책 확인
- [ ] 장애 복구 절차 문서화

---

## 참고 문서

- [운영 가이드](./operations.md) - 일상 운영, 모니터링, 백업
- [데이터 수집](./data_collection.md) - Collector, 내부 시스템
- [아키텍처](./architecture.md) - 시스템 구조
