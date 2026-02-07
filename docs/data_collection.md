# 데이터 수집 및 내부 시스템 가이드

> **버전**: v0.8.1 | **최종 업데이트**: 2026-02-07

---

## 1. Standalone Collector

`trader-collector`는 ZeroQuant의 Standalone 데이터 수집 바이너리입니다. API 서버와 독립적으로 실행됩니다.

### 주요 기능

- **심볼 동기화**: KRX, Binance, Yahoo Finance에서 종목 목록 동기화
- **OHLCV 수집**: 일봉 데이터 수집 (KRX API / Yahoo Finance)
- **지표 동기화**: RouteState, MarketRegime, TTM Squeeze 등 분석 지표
- **GlobalScore 동기화**: 7Factor 기반 종합 점수 계산
- **KRX Fundamental**: PER/PBR/배당수익률/섹터 정보 (KRX API 활성화 시)

### 데이터 프로바이더 이중화

| 시장 | Primary | Fallback |
|------|---------|----------|
| 국내 주식 (KR) | KRX OPEN API | Yahoo Finance |
| 해외 주식 (US) | Yahoo Finance | - |
| 암호화폐 (CRYPTO) | Yahoo Finance | - |

### 빠른 시작

```bash
# 1. 환경변수 설정 (.env)
DATABASE_URL=postgresql://trader:trader_secret@localhost:5432/trader
PROVIDER_KRX_API_ENABLED=false
PROVIDER_YAHOO_ENABLED=true

# 2. 빌드
cargo build --release --bin trader-collector

# 3. 실행
./target/release/trader-collector run-all     # 전체 워크플로우 1회 실행
./target/release/trader-collector daemon      # 데몬 모드 (주기적 자동 실행)
```

### CLI 명령어

```bash
# ── 글로벌 옵션 ──────────────────────────────────────────
trader-collector --log-level debug <command>   # 로그 레벨 (trace, debug, info, warn, error)

# ── 심볼 동기화 ──────────────────────────────────────────
trader-collector sync-symbols

# ── OHLCV 수집 ───────────────────────────────────────────
trader-collector collect-ohlcv
trader-collector collect-ohlcv --symbols "005930,000660"  # 특정 심볼만
trader-collector collect-ohlcv --stale-hours 24           # 24시간 이상 지난 것만
trader-collector collect-ohlcv --resume                   # 중단점부터 재개

# ── Fundamental 동기화 ───────────────────────────────────
trader-collector sync-krx-fundamentals                    # KRX API (승인 필요)
trader-collector sync-naver-fundamentals                  # 네이버 금융 크롤링 (KR)
trader-collector sync-naver-fundamentals --ticker "005930" --stale-hours 24 --resume
trader-collector sync-yahoo-fundamentals                  # Yahoo Finance (US/글로벌)
trader-collector sync-yahoo-fundamentals --market "US" --batch-size 100 --resume

# ── 분석 지표 동기화 ─────────────────────────────────────
trader-collector sync-indicators
trader-collector sync-indicators --symbols "005930,000660" --stale-hours 12 --resume

# ── GlobalScore 동기화 ───────────────────────────────────
trader-collector sync-global-scores
trader-collector sync-global-scores --symbols "005930" --stale-hours 12 --resume

# ── 스크리닝 / 신호 성과 ────────────────────────────────
trader-collector refresh-screening                        # Materialized View 갱신
trader-collector sync-signal-performance                  # 신호 성과 계산
trader-collector sync-signal-performance --min-days 2 --max-days 30 --resume

# ── 전체 워크플로우 ──────────────────────────────────────
trader-collector run-all                                  # 전체 파이프라인 1회 실행
trader-collector run-all --ticker "005930"                # 특정 심볼만 (테스트용)

# ── 데몬 모드 ────────────────────────────────────────────
trader-collector daemon                                   # 주기적 자동 실행

# ── 체크포인트 관리 ──────────────────────────────────────
trader-collector checkpoint list                          # 체크포인트 상태 조회
trader-collector checkpoint clear naver_fundamental       # 특정 워크플로우 체크포인트 삭제
trader-collector checkpoint interrupt indicator_sync      # 실행 중 워크플로우 중단 마킹

# ── 스케줄러 상태 ────────────────────────────────────────
trader-collector scheduler-status                         # KR 시장 (기본)
trader-collector scheduler-status --market "US"           # US 시장
```

### 수집 워크플로우 (run-all)

```
Step 1/6: 심볼 동기화 (--ticker 모드 시 스킵)
  └── KRX/Binance/Yahoo에서 종목 목록 가져오기
  └── symbol_info 테이블 업데이트

Step 2/6: Fundamental 동기화
  └── KRX API (활성화 시) 또는 네이버 금융 크롤링 (KR)
  └── PER, PBR, ROE, 배당수익률, 시가총액, 섹터

Step 3/6: OHLCV 수집
  └── 일봉 데이터 수집 (Yahoo Finance)
  └── ohlcv_daily 테이블 저장

Step 4/6: 분석 지표 동기화
  └── RouteState, MarketRegime 계산
  └── TTM Squeeze, Trigger 감지
  └── symbol_indicator 테이블 저장

Step 5/6: GlobalScore 동기화
  └── 7Factor 점수 계산
  └── 종합 랭킹 생성
  └── global_score 테이블 저장

Step 6/6: 스크리닝 뷰 갱신
  └── Materialized View 리프레시
  └── symbol_info + fundamental + global_score 통합
```

### 데몬 모드 (daemon)

데몬은 3개 그룹을 독립적 주기로 실행합니다:

| 그룹 | 주기 환경변수 | 기본값 | 실행 내용 |
|------|-------------|--------|----------|
| **Group A** | `DAEMON_INTERVAL_MINUTES` | 60분 | 외부 API (심볼, Fundamental, OHLCV) |
| **Group B** | `RANKING_INTERVAL_MINUTES` | 15분 | 계산 파이프라인 (지표 → GlobalScore → 스크리닝 → 신호성과) |
| **Group C** | `REALTIME_INTERVAL_MINUTES` | 5분 | 매크로 데이터 (Redis 캐시) |

### 성능 기대값

| 작업 | 예상 시간 | 비고 |
|------|----------|------|
| 심볼 동기화 | ~1분 | KRX 2,500개 + Binance 300개 + Yahoo 500개 |
| Fundamental 동기화 | ~15분 | 네이버 크롤링 기준, 300ms 딜레이 |
| OHLCV 수집 (전체) | ~1.5시간 | 3,000개 종목, 500ms 딜레이 |
| OHLCV 수집 (증분) | ~5분 | stale 종목만 |
| 지표 동기화 | ~10분 | 3,000개 종목 |
| GlobalScore 동기화 | ~5분 | 3,000개 종목 |
| 스크리닝 뷰 갱신 | ~10초 | Materialized View 리프레시 |
| **전체 워크플로우** | **~2시간** | 첫 실행 시 |

### 운영 설정 (cron, systemd)

#### Cron 스케줄 예시

```cron
# 매일 오전 7시 전체 워크플로우 실행
0 7 * * * cd /path/to/trader && ./target/release/trader-collector run-all >> /var/log/collector.log 2>&1

# 1시간마다 증분 OHLCV 수집
0 * * * * cd /path/to/trader && ./target/release/trader-collector collect-ohlcv --stale-hours 2 >> /var/log/collector.log 2>&1
```

#### systemd 서비스 예시

```ini
# /etc/systemd/system/trader-collector.service
[Unit]
Description=ZeroQuant Data Collector
After=network.target postgresql.service

[Service]
Type=simple
User=trader
WorkingDirectory=/opt/zeroquant
ExecStart=/opt/zeroquant/target/release/trader-collector daemon
Restart=always
RestartSec=10
Environment=DATABASE_URL=postgresql://trader:secret@localhost:5432/trader
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

---

## 2. 내부 시스템

### 에러 추적 시스템

에러 발생 시 구조화된 로그를 수집하고 AI 디버깅에 활용합니다.

```rust
use trader_api::monitoring::{global_tracker, ErrorRecordBuilder, ErrorSeverity, ErrorCategory};

// 에러 기록
let record = ErrorRecordBuilder::new("데이터베이스 쿼리 실패")
    .severity(ErrorSeverity::Error)
    .category(ErrorCategory::Database)
    .entity("AAPL")
    .with_context("query", "SELECT * FROM ...")
    .raw_error(&e)
    .build();

global_tracker().record(record);
```

> API 엔드포인트: [Monitoring API](./api.md#monitoring-api) 참조

### 심볼 실패 추적

3회 연속 실패 시 자동으로 심볼이 비활성화됩니다.

> API 엔드포인트: [Symbols API](./api.md#symbols-api) 참조

---

## 3. 분석 시스템

### 스크리닝

Materialized View (`mv_latest_prices`)를 활용한 고성능 스크리닝.

> API 엔드포인트: [Screening API](./api.md#screening-api) 참조

### Global Score 랭킹

7Factor 기반 종합 점수로 종목 순위를 산출합니다.

**7Factor 팩터:**

| 팩터 | 가중치 | 설명 |
|------|--------|------|
| Momentum | 0.10 | ERS + MACD 기울기 + RSI 보너스 |
| Value | - | PER, PBR 기반 |
| Quality | - | ROE, 부채비율 |
| Volatility | - | ATR, VolZ 안정성 |
| Liquidity | 0.13 | 거래대금 퍼센타일 |
| Growth | - | 매출/이익 성장률 |
| Sentiment | - | 이격도, RSI 중립도 |

> API 엔드포인트: [Ranking API](./api.md#ranking-api) 참조

### Reality Check

전일 추천 종목의 익일 실제 성과를 자동 검증합니다.

**출력 지표:**
- 추천 종목 승률 (전체, 7일, 30일)
- 평균 수익률
- 레짐별 성과 (MarketRegime 연동)

> API 엔드포인트: [Reality Check API](./api.md#reality-check-api) 참조

### 관심종목 (Watchlist)

사용자별 관심종목 그룹을 관리합니다.

> API 엔드포인트: [Watchlist API](./api.md#watchlist-api) 참조

### Multi Timeframe

다중 타임프레임 캔들 데이터를 동시에 조회합니다.

> API 엔드포인트: [Multi Timeframe API](./api.md#multi-timeframe-api) 참조

---

## 4. 트러블슈팅

### KRX API 401 Unauthorized

```
KRX API가 비활성화되어 있습니다.
PROVIDER_KRX_API_ENABLED=true로 활성화하세요.
```

**원인**: KRX OPEN API 사용 권한이 없음
**해결**:
1. https://openapi.krx.co.kr 에서 API 사용 신청
2. 승인 후 `PROVIDER_KRX_API_ENABLED=true` 설정
3. 승인 전까지는 Yahoo Finance로 대체 운영

### CRYPTO 심볼 수집 실패

```
Yahoo Finance 심볼이 설정되지 않음: BTCUSDT
```

**원인**: yahoo_symbol 컬럼이 없는 CRYPTO 종목
**해결**: 해당 종목은 자동으로 비활성화됨. 정상 동작.

### DB 연결 실패

```bash
# Podman 컨테이너 상태 확인
podman ps | grep timescaledb

# 로그 확인
podman logs trader-timescaledb
```

---

## 체크리스트

**개발 전 확인:**
- [ ] Podman 컨테이너 (PostgreSQL) 실행 중
- [ ] `.env` 파일 설정 완료
- [ ] `PROVIDER_*` 환경변수 확인

**운영 전 확인:**
- [ ] 로그 레벨 설정 (info 권장)
- [ ] Cron/systemd 스케줄 설정
- [ ] 디스크 공간 확인 (OHLCV 데이터)
- [ ] 모니터링 알림 설정

---

## 참고 문서

- [설치/배포 가이드](./setup_guide.md) - 환경변수 상세 설정
- [아키텍처](./architecture.md) - 데이터 프로바이더 이중화 구조
- [KRX API 스펙](./krx_openapi_spec.md) - KRX OPEN API 명세
