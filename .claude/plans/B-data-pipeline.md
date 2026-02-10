# Plan: [B] 데이터 파이프라인 & 무결성

> 🔴 모든 분석·백테스트·전략의 신뢰성 기반. C~G의 선행 조건.
> 병렬: A, G와 동시 진행 가능.
> DB 마이그레이션은 개별 `.sql` 작성 후 `trader migrate consolidate`로 병합.

## 선행 조건
- 없음 (독립 착수 가능)

## 예상 규모
Large

---

## B-1: 기업 이벤트 처리 (Corporate Action Handler)

- [ ] `corporate_actions` 테이블 신설 (`event_type`, `symbol`, `ex_date`, `split_factor`, `dividend_amount`)
- [ ] `ohlcv` 테이블에 `adj_close`, `split_factor`, `dividend` 컬럼 추가
- [ ] Backward Adjust 로직 구현 (`trader-data/src/`)
- [ ] Yahoo Finance/KRX에서 Split/Dividend 이벤트 수집기 추가 (`trader-collector/`)
- [ ] `CandleProcessor`가 보정 데이터를 사용하도록 수정 (`trader-analytics/src/`)
- [ ] API 엔드포인트: `POST /api/v1/data/adjust-corporate-actions`, `GET /api/v1/data/events/{symbol}`

## B-2: 시점 데이터 관리 (Point-in-Time)

- [ ] `symbol_fundamental` 테이블에 `announce_date DATE`, `report_period VARCHAR(10)` 추가
- [ ] 펀더멘털 수집기에 공시일 파싱 로직 추가 (`trader-collector/`)
- [ ] 백테스트 쿼리에 `WHERE announce_date <= backtest_time` 조건 강제 (`trader-analytics/`)
- [ ] 기존 펀더멘털 데이터에 대한 `announce_date` 백필(backfill) 스크립트

## B-3: 생존 편향 방지 (Survivorship Bias)

- [ ] `symbol_info` 테이블에 `is_active BOOLEAN DEFAULT TRUE`, `delisted_date DATE` 추가
- [ ] KRX/Yahoo에서 상폐 종목 정보 수집 로직 추가 (`trader-collector/`)
- [ ] 백테스트 유니버스 구성 시 `delisted_date > backtest_time` 종목 포함
- [ ] 시뮬레이션 중 `delisted_date` 도달 시 잔여 포지션 강제 청산 로직 (`trader-analytics/`)

## B-4: 데이터 갭 감지 & 복구

- [ ] OHLCV 갭 감지 모듈 신규 (`trader-data/src/gap_detector.rs`)
- [ ] 거래일 캘린더 대비 누락 일자 스캔 쿼리
- [ ] 감지된 갭에 대한 자동 재수집 트리거 (`trader-collector/`)
- [ ] 갭 상태 리포트 API: `GET /api/v1/data/gaps`

## B-5: Collector 복원력 강화

- [ ] Dead-letter 큐 (실패 심볼 재시도) 구현 (`trader-collector/`)
- [ ] 재시도 정책: 지수 백오프, 최대 3회, 실패 시 알림 발송
- [ ] Collector 헬스 상태를 `/health/ready` 응답에 통합 (마지막 실행 시각, 성공/실패 카운트)

## B-6: FX 환율 서비스

- [ ] `FxRateProvider` trait 정의 (`trader-core/src/domain/`)
- [ ] Yahoo Finance/한국은행 API 기반 환율 수집기 구현 (`trader-data/`)
- [ ] Redis 캐시 (TTL 1시간) + DB 히스토리 저장
- [ ] 포트폴리오 P&L 산출 시 통화 통합 변환 적용

## B-7: 거래소 중립 마켓 캘린더

- [ ] `MarketCalendar` trait 정의 (`trader-core/src/domain/`)
- [ ] KRX, NYSE/NASDAQ, Binance 별 구현 (공휴일, 반일 거래, 점검 시간)
- [ ] 전략·수집기에서 `is_market_open()` 호출을 trait 기반으로 교체

## B-8: Clock Trait 도입

- [ ] `Clock` trait 정의 (`trader-core/src/domain/clock.rs`): `fn now(&self) -> DateTime<Utc>`
- [ ] `SystemClock` 구현 (실시간), `ManualClock` 구현 (백테스트/테스트용)
- [ ] 코드 전반의 `Utc::now()` 직접 호출을 `Clock` trait 호출로 교체
- [ ] 백테스트 엔진에 `ManualClock` 주입, 시간 진행을 엔진이 제어

## 관련 파일
- `crates/trader-data/src/`
- `crates/trader-collector/`
- `crates/trader-analytics/src/`
- `crates/trader-core/src/domain/`
- `migrations/`
