# 인프라/운영 참조 (에이전트용)

> 원본: `docs/setup_guide.md` (428줄), `docs/operations.md` (572줄), `docs/data_collection.md` (321줄)
> 담당: `validator`, `debugger`, `rust-impl`

## 인프라 구성

| 서비스 | 컨테이너명 | 포트 | 이미지 |
|--------|-----------|------|--------|
| TimescaleDB | `trader-timescaledb` | 5432 | timescale/timescaledb:latest-pg15 |
| Redis | `trader-redis` | 6379 | redis:7-alpine |
| API 서버 | (로컬 실행) | 3000 | - |
| Frontend | (로컬 실행) | 5173 | - |

## 컨테이너 명령 (podman)

```bash
# 시작/중지
podman compose up -d
podman compose down

# 상태 확인
podman compose ps

# DB 접속 (직접 psql/redis-cli 금지)
podman exec -it trader-timescaledb psql -U trader -d trader
podman exec -it trader-redis redis-cli

# 로그
podman compose logs --tail=100 trader-timescaledb
```

## 빌드 및 실행

```bash
# API 서버
cargo run --bin trader-api --release
# 또는 빌드된 바이너리
./target/release/trader-api.exe

# Collector
./target/release/trader-collector.exe run-all
./target/release/trader-collector.exe daemon

# Frontend
cd frontend && npm run dev

# CLI
./target/release/trader.exe <command>
```

## 환경변수 (.env)

```
DATABASE_URL=postgresql://trader:trader_secret@localhost:5432/trader
REDIS_URL=redis://localhost:6379
JWT_SECRET=<secret>
RUST_LOG=info,trader_api=info
```

## Collector CLI

```bash
# 심볼 동기화
./target/release/trader-collector.exe sync-symbols

# OHLCV 수집
./target/release/trader-collector.exe sync-ohlcv

# 지표 계산
./target/release/trader-collector.exe sync-indicators

# GlobalScore 계산
./target/release/trader-collector.exe sync-scores

# 전체 워크플로우
./target/release/trader-collector.exe run-all

# 데몬 모드
./target/release/trader-collector.exe daemon
```

## 데이터 프로바이더

| 시장 | Primary | Fallback |
|------|---------|----------|
| 국내 주식 (KR) | KRX OPEN API | Yahoo Finance |
| 해외 주식 (US) | Yahoo Finance | - |
| 암호화폐 (CRYPTO) | Yahoo Finance | - |

## 마이그레이션 적용

```bash
# 상태 확인
./target/release/trader.exe migrate status --db-url "postgres://trader:trader@localhost:5432/trader"

# 검증
./target/release/trader.exe migrate verify --verbose

# 적용 (테스트 DB 먼저)
./target/release/trader.exe migrate apply --db-url "..." --dir migrations
```

## 헬스체크

```bash
curl -s http://localhost:3000/health | jq
curl -s http://localhost:3000/health/ready | jq
```

## 기술 스택

| 영역 | 기술 |
|------|------|
| 언어 | Rust stable (1.93+), TypeScript |
| 비동기 | Tokio |
| 웹 | Axum 0.7+ |
| DB | SQLx 0.8+ (async, compile-time checked) |
| 시계열 | TimescaleDB 2.x (PostgreSQL 15 확장) |
| 캐시 | Redis 7.x |
| 프론트엔드 | SolidJS 1.8+, TailwindCSS, Vite 5.x |
| 차트 | Lightweight Charts 4.x |
| 데이터 | Polars, ta-rs |
