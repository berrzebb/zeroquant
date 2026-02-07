# Trader Collector

Standalone data collector for ZeroQuant trading system.

## 기능

- **심볼 동기화**: KRX, Binance, Yahoo Finance에서 종목 정보 동기화
- **OHLCV 수집**: 일봉 데이터 수집 (Yahoo Finance, 체크포인트/재개 지원)
- **Fundamental 수집**: 네이버 금융 (KR), Yahoo Finance (US/글로벌), KRX API
- **분석 지표**: RouteState, MarketRegime, TTM Squeeze 계산
- **GlobalScore**: 7Factor 종합 점수 랭킹
- **스크리닝**: Materialized View 기반 고성능 종목 필터
- **신호 성과**: 과거 신호의 N일 후 수익률 추적

## 빠른 시작

### 1. 환경변수 설정

```bash
cp .env.example .env
# .env 파일 수정 (DATABASE_URL 등)
```

### 2. 빌드

```bash
cargo build --bin trader-collector --release
```

### 3. 실행

```bash
# 전체 워크플로우 1회 실행 (심볼 → Fundamental → OHLCV → 지표 → Score → 스크리닝)
./target/release/trader-collector run-all

# 특정 심볼만 테스트
./target/release/trader-collector run-all --ticker "005930"

# 데몬 모드 (3그룹 주기적 자동 실행)
./target/release/trader-collector daemon
```

## 주요 CLI 명령어

```bash
# 개별 명령어
trader-collector sync-symbols                    # 심볼 동기화
trader-collector collect-ohlcv --resume           # OHLCV 수집 (재개)
trader-collector sync-naver-fundamentals          # 네이버 Fundamental (KR)
trader-collector sync-yahoo-fundamentals          # Yahoo Fundamental (US)
trader-collector sync-indicators --resume         # 분석 지표
trader-collector sync-global-scores --resume      # GlobalScore
trader-collector refresh-screening                # 스크리닝 뷰 갱신
trader-collector sync-signal-performance          # 신호 성과

# 체크포인트 관리
trader-collector checkpoint list                  # 상태 조회
trader-collector checkpoint clear <WORKFLOW>      # 초기화
trader-collector scheduler-status                 # 스케줄러 상태
```

전체 CLI 레퍼런스: `docs/data_collection.md` 참조

## 데몬 모드

| 그룹 | 주기 환경변수 | 기본값 | 내용 |
|------|-------------|--------|------|
| **A** | `DAEMON_INTERVAL_MINUTES` | 60분 | 외부 API (심볼, Fundamental, OHLCV) |
| **B** | `RANKING_INTERVAL_MINUTES` | 15분 | 계산 (지표 → Score → 스크리닝 → 신호성과) |
| **C** | `REALTIME_INTERVAL_MINUTES` | 5분 | 매크로 데이터 |

## 환경변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `DATABASE_URL` | (필수) | PostgreSQL 연결 문자열 |
| `REDIS_URL` | - | Redis 캐시 URL (매크로 데이터용) |
| `SYMBOL_SYNC_MIN_COUNT` | 100 | 최소 심볼 수 |
| `SYMBOL_SYNC_KRX` | true | KRX 동기화 활성화 |
| `SYMBOL_SYNC_BINANCE` | false | Binance 동기화 활성화 |
| `OHLCV_BATCH_SIZE` | 50 | 배치당 심볼 수 |
| `OHLCV_REQUEST_DELAY_MS` | 500 | API 요청 간 딜레이 (ms) |
| `NAVER_FUNDAMENTAL_ENABLED` | true | 네이버 금융 크롤러 활성화 |
| `DAEMON_INTERVAL_MINUTES` | 60 | Group A 실행 주기 (분) |
| `RANKING_INTERVAL_MINUTES` | 15 | Group B 실행 주기 (분) |

전체 환경변수 목록: `docs/setup_guide.md` 참조

## 문서

- **데이터 수집 가이드**: `docs/data_collection.md`
- **환경설정**: `docs/setup_guide.md`

## 개발

```bash
# 테스트 실행
cargo test --bin trader-collector

# 로그 레벨 조정
./trader-collector --log-level debug collect-ohlcv
```

## 라이선스

MIT
