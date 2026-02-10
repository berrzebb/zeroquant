# Changelog


## [Unreleased]

### Added

#### P3: DB증권 WebSocket 구현 완료
- **DB증권 WebSocket 클라이언트** — `DbInvestmentWebSocket`
  - 운영/모의투자 환경 지원 (wss://openapi.dbsec.co.kr:7070/17070)
  - 재연결 루프 (최대 3회, 5초 간격)
  - 동적 구독/해제 (command 채널)
  - tokio::select! 병렬 처리 (메시지, 명령, Ping)
- **채널 구현**
  - V60: 실시간 체결가 → `QuoteData`
  - V20: 실시간 호가 → `OrderBook`
  - V22: 주문체결통보 (구조만 구현)
- **MarketStream 통합** — `DbInvestmentMarketStream`
  - `MarketStream` trait 완전 구현
  - `start()`, `subscribe_ticker()`, `subscribe_order_book()`, `next_event()`
  - `lib.rs`에서 public export

### Fixed
- **Clippy 워크스페이스 전체 클린업** — 117개 에러 → 0개 (100% 해결)
  - `unnecessary_cast`, `len_zero`, `derivable_impls`, `needless_borrow` 제거
  - `new_without_default`, `wrong_self_convention`, `cloned_ref_to_slice_refs` 수정
  - `unnecessary_lazy_evaluations`, `type_complexity`, `manually_reimplementing_div_ceil` 수정
  - `doc_lazy_continuation` 수정
  - 영향 받은 crate: trader-core (56개), trader-cli (10개), trader-analytics (1개)
- **trader-notification** — `SmsProvider` Default derive로 변경 (clippy::derivable_impls)
- **trader-exchange** — unused variable 경고 수정 (simulated/exchange.rs)
- **trader-execution** — `ConversionConfig` 구조체 초기화 패턴 수정 (컴파일 에러 해결)

---

## [0.10.0] - 2026-02-10

> **거래소 주문 실행 및 데이터 완전성 완성**: Upbit, LS증권, DB증권, Bithumb 4개 거래소의 주문 실행 및 체결 내역 조회 기능을 완성했습니다. 모든 거래소가 OrderExecutionProvider를 구현하여 실거래 준비가 완료되었습니다.

### Added

#### P1: 주문 실행 구현 (4개 거래소)
- **Upbit** — `POST /v1/orders` (시장가/지정가), `DELETE /v1/order` (취소)
  - `UpbitExchangeProvider` — `OrderExecutionProvider` trait 구현
  - `place_order()`, `cancel_order()`, `modify_order()` (cancel + re-place)
- **LS증권** — TR 기반 주문/취소/정정
  - `CSPAT00601` (매수), `CSPAT00701` (매도)
  - `CSPAT00603/00703` (취소), `CSPAT00602/00702` (정정)
  - `LsSecExchangeProvider` — `OrderExecutionProvider` trait 구현
- **DB증권** — TR 기반 주문/취소/정정
  - `CSPAT00600` (매수), `CSPAT00800` (매도)
  - `CSPAT01400/01500` (취소), `CSPAT01200/01300` (정정)
  - `DbInvestmentExchangeProvider` — `OrderExecutionProvider` trait 구현
- **Bithumb** — `POST /v1/orders` (시장가/지정가), `DELETE /v1/orders/{uuid}` (취소)
  - `BithumbExchangeProvider` — `OrderExecutionProvider` trait 구현
  - `modify_order()` (cancel + re-place)

#### P2: 데이터 완전성 완성
- **체결 내역 조회** (`fetch_execution_history()`)
  - Upbit: `GET /v1/orders/closed?state=done`
  - LS증권: `CSPAQ13700` (주식체결조회)
  - DB증권: `CSPAQ12300` (주식체결내역조회)
  - Bithumb: `GET /v1/orders/closed`
- **현재가 필드 확장**
  - LS증권: `QuoteOutBlock`에 high/low/open/volume 추가
  - DB증권: `DbQuoteOut`에 high/low/open/volume 추가

### Fixed
- **테스트 수정** (5개 테스트 통과)
  - `test_list_backtest_strategies` — 전략 ID vs 별칭 구분 (rsi_mean_reversion → rsi)
  - `test_get_holdings_empty` — credential_id 필수 파라미터 요구 (BAD_REQUEST)
  - `test_get_portfolio_summary_mock` — credential_id 필수 파라미터 요구
  - `test_filter_entry_only` — SignalMarker strength 기본값(0.0) vs 필터 기본값(0.7) 충돌 해결
  - `test_filter_strategy_ids` — min_strength 명시적 설정

### Changed
- **Bithumb Chrono API** — `format()` → `to_rfc3339()` (deprecation 해결)
- **Clippy 워닝 제로** — Bithumb 커넥터 전체 clippy 경고 수정

---

## [0.9.1] - 2026-02-10

> **Paper Trading 실시간 가격 + 다중 거래소 MarketStream + 차트 강화**: 포지션 평가액/미실현 손익을 Mock 실시간 캐시 가격으로 계산하고, MarketStream을 다중 거래소 팩토리로 리팩토링했습니다. 프론트엔드에 WebSocket 실시간 시세, 캔들 차트+매매 태그, Kelly Criterion, 상관관계 히트맵, 볼륨 프로파일을 추가했습니다.

### Added

#### Paper Trading 실시간 가격 반영
- `paper_trading.rs` — Mock 실시간 캐시 가격 기반 미실현 손익/평가액 계산
- Paper Trading 시작 시 Mock→MarketStream→WebSocket 파이프라인 자동 연결

#### 다중 거래소 MarketStream 팩토리
- `market_stream.rs` — exchange_id 기반 MarketStream 생성 분기 (KIS/Mock/Upbit/Bithumb/LS증권)
- `strategies.rs` — exchange_id 기반 MarketStream 생성 로직
- `stream.rs` — UpbitMarketStream, BithumbMarketStream, LsSecMarketStream 신규

#### 프론트엔드 Paper Trading 강화
- `PaperTrading.tsx` — WebSocket 실시간 가격 업데이트 + 캔들 차트 + 매매 태그
- `PaperTrading.tsx` — Kelly Criterion 시각화 + 상관관계 히트맵 + 볼륨 프로파일
- `Simulation.tsx` — 신호 필터, 볼륨 프로파일, 다중 심볼 선택
- `createWebSocket.ts` — 중복 구독 방지 로직

### Changed
- `client.ts` — MockStreamingConfigRequest 타입 추가

### Fixed
- `Dashboard.tsx` — 전략 상태 필터 대소문자 수정 (`running` → `Running`)

---

## [0.9.0] - 2026-02-09

> **Mock 거래소 KIS 수준 업그레이드**: Paper Trading을 Yahoo Finance D1 폴링 + 즉시 체결에서 1분봉 틱 보간/랜덤 워크 가격 스트리밍 + 호가창 기반 VWAP 체결로 업그레이드했습니다. 지정가/스톱 주문 미체결 큐와 잔고 예약 시스템을 도입하여 실전과 유사한 페이퍼 트레이딩 환경을 구축했습니다.

### Added

#### 현실적 가격 스트리밍 (`mock_streaming.rs`)
- `MockPriceGenerator` trait — 3가지 모드 지원 (HistoricalReplay / RandomWalk / YahooLegacy)
- `HistoricalReplayGenerator` — DB 1분봉 → 12단계 틱 보간 (O→H→L→C 경로)
- `RandomWalkGenerator` — ATR 기반 정규분포 + 평균 회귀 + 호가 단위 라운딩
- `MockOrderBookGenerator` — KR 10단계 / US 5단계 / Crypto 20단계 합성 호가창

#### 주문 매칭 엔진 (`mock_order_engine.rs`)
- `MockOrderEngine` — 호가창 기반 VWAP 체결 + 지정가/스톱 주문 미체결 큐
- 시장가 즉시 체결: OrderBook ask/bid 레벨 순서대로 VWAP 가중평균
- 지정가 주문: 즉시 체결 가능이면 체결, 아니면 큐 등록 + 잔고 예약
- 스톱 주문: stop_price 도달 시 시장가로 전환
- 부분 체결: OrderBook 물량 부족 시 가능한 만큼만 체결
- 주문 취소/정정 + 예약금 자동 해제/재계산

#### MockExchangeProvider 통합
- `StrategyState.reserved_balance` — 지정가 주문 자금 예약 시스템
- `start_streaming_with_config()` — 모드별 Generator 생성 + 백그라운드 스트리밍
- `place_order()` 재구현: Market→VWAP, Limit/Stop→큐 + 잔고 예약
- `fetch_pending_orders()` — 실제 미체결 주문 반환 (빈 배열 → 실시간)
- 최신 시세/호가 캐시 (`latest_tickers`, `latest_order_books`)

#### Paper Trading API 확장
- `MockStreamingConfigDto` — 모드, 틱 간격, 재생 속도 설정
- `PaperTradingStartRequest.streaming_config` — 스트리밍 모드 선택 API

#### DB 마이그레이션
- `mock_pending_orders` 테이블 — 미체결 주문 영속화
- `paper_trading_sessions.reserved_balance` 컬럼 — 예약 잔고 추적
- `load_state()` / `save_strategy_state()` 미체결 주문 복원/저장

### Changed
- 한국 거래소 4개 통합 (Upbit, Bithumb, DB금융투자, LS증권)
  - Provider + Client + WebSocket 커넥터 구현
  - Provider Factory 연결 및 lib.rs re-export 통합

### Fixed
- 마이그레이션 도구: 누락된 그룹 추가 + ENUM 멱등성 패턴 수정

---

## [0.8.3] - 2026-02-08

> **OHLCV 쿼리 최적화, 백테스트 타임프레임 폴백, 스크리닝 UI 성능**: OHLCV 배치 쿼리를 LATERAL JOIN + TimescaleDB 청크 프루닝으로 ~70% 최적화(1.04s → 306ms)하고, 백테스트/CLI에 전략 기본 타임프레임 자동 폴백을 도입했습니다. 스크리닝 화면에 네이티브 가상 스크롤(11,000+ 행 60fps)과 상태/등급/점수 정렬을 추가하고, 매매일지에 상세 거래 통계를 제공합니다.

### Added

#### ⚡ OHLCV 배치 쿼리 LATERAL JOIN 최적화

**파일**: `trader-data/src/storage/ohlcv.rs`

- `get_cached_klines_batch()` — ROW_NUMBER() 윈도우 함수를 LATERAL JOIN으로 교체
- TimescaleDB 청크 프루닝: 타임프레임별 시간 힌트로 불필요한 압축 청크 스캔 방지
- 2,524 심볼 × 50 캔들 기준: **1,040ms → 306ms** (~70% 성능 향상)
- 타임프레임별 interval 계산: M1(3일) ~ MN1(365일) 안전 배수 적용

#### 🔄 백테스트 다중 타임프레임 폴백

**파일**: `trader-api/src/routes/backtest/loader.rs`, `backtest/mod.rs`, `trader-cli/src/commands/backtest.rs`

- `load_klines_with_fallback()` — 전략 기본 타임프레임 → secondary → 일반(1m~1d) 자동 폴백
- `load_klines_with_multi_tf_fallback()` — 다중 타임프레임 우선순위 탐색
- StrategyRegistry 메타 연동: 전략별 `default_timeframe`, `secondary_timeframes` 자동 적용
- 기존 `load_klines_from_db()` (하드코딩 D1) 제거
- CLI `StrategyType::to_registry_id()` 추가 — CLI ↔ StrategyRegistry 연결

#### 📊 스크리닝 상태/등급/점수 정렬

**파일**: `frontend/src/pages/Screening.tsx`

- `SortField` 타입 확장: `route_state`, `grade`, `overall_score` 추가
- 우선순위 기반 정렬: ATTACK(4) > ARMED(3) > WAIT(2) > OVERHEAT(1) > NEUTRAL(0)
- 등급 정렬: BUY(3) > WATCH(2) > HOLD(1)
- 테이블 헤더에 정렬 버튼 + 방향 아이콘 (ChevronUp/ChevronDown)

#### 🚀 스크리닝 네이티브 가상 스크롤

**파일**: `frontend/src/pages/Screening.tsx`

- `@tanstack/solid-virtual` 제거 → SolidJS 네이티브 구현
  - 원인: `createVirtualizer`가 `<Show>` 조건부 렌더링 내부에서 scroll element null 문제
- `visibleRange` signal + `slice()` + padding row 패턴
- OVERSCAN 10행 + ROW_HEIGHT 52px 기반 가시 영역 계산
- 11,451행에서 25행만 DOM 렌더링 → 60fps 유지
- `createEffect` 연동: 스크롤 컨테이너 마운트 시 + 데이터 변경 시 자동 범위 재계산

#### 📒 매매일지 백테스트 인사이트 강화

**파일**: `frontend/src/pages/TradingJournal.tsx`

- `convertBacktestToInsights()` 확장: 매수/매도 건수, 고유 종목 수, 총 실현손익, 첫/마지막 거래일
- `strategyPerformance()` 확장: 거래량, 수수료, 평균 수익/손실, 최대 수익/손실, 활성 거래일

### Changed

- `load_klines_from_db()` → `load_klines_with_fallback()` 리네임 (하드코딩 D1 → 전략별 타임프레임)
- `load_multi_klines_from_db()` — `default_timeframe` 파라미터 추가
- `load_klines_with_timeframe()` — `#[allow(dead_code)]` 제거 (실제 사용됨)
- 스크리닝 디버그 `console.log` 제거

### Performance

- OHLCV 배치 쿼리: 2,524 심볼 기준 **1,040ms → 306ms** (LATERAL JOIN + 청크 프루닝)
- 스크리닝 렌더링: 11,451행 전체 DOM → 25행 가상 스크롤 (메모리/CPU 대폭 절감)

---

## [0.8.2] - 2026-02-08

> **성능 최적화, 통합 리스크 관리, CandleProcessor 공통화, 데이터 무결성 관리**: 스크리닝 배치 쿼리 + Redis 구조적 특성 캐시로 10초 → 서브초 성능을 달성하고, ExitConfig를 5가지 리스크 타입으로 확장했습니다. BacktestEngine/SimulationEngine 간 캔들 처리 로직을 CandleProcessor로 공통화하고, GlobalScore 동시 처리(Semaphore 10개)로 ~10배 속도를 개선했습니다. symbol_info 기준 데이터 무결성 관리 시스템(cascade delete, orphan cleanup)을 도입했습니다.

### Added

#### 🔄 CandleProcessor - 백테스트/시뮬레이션 공통 캔들 처리 (592줄)

**파일**: `trader-analytics/src/backtest/candle_processor.rs` (신규)

- BacktestEngine과 SimulationEngine 간 공통 캔들 처리 로직 추출
- StrategyContext 업데이트, 시그널 생성, 포지션 동기화를 한 곳에서 관리
- 수정 시 한 곳만 변경하면 양쪽 엔진에 반영

#### 🛡️ ExitConfig 통합 리스크 관리 시스템 (+500줄)

**파일**: `trader-strategy/src/strategies/common/exit_config.rs`

6가지 리스크 옵션을 전략별로 `enabled: bool`로 독립 활성화/비활성화하여 조합하는 통합 리스크 관리 시스템:

| 옵션 | 설명 | 모드/파라미터 | 동작 방식 |
|------|------|-------------|----------|
| **StopLoss** | 손절 | `Fixed` (고정 %), `AtrBased` (ATR 배수) | entry ± pct% / executor에서 ATR 동적 계산 |
| **TakeProfit** | 익절 | `pct` (고정 %) | entry ± pct% |
| **TrailingStop** | 트레일링 스톱 | `FixedPercentage`, `AtrBased`, `Step`, `ParabolicSar` | trigger 도달 후 고점 대비 stop% 하락 시 청산 |
| **ProfitLock** | 수익 잠금 | `threshold_pct`, `lock_pct` | 목표 수익 달성 후 수익의 lock% 이하 하락 시 청산 |
| **DailyLossLimit** | 일일 손실 한도 | `max_loss_pct` | 계좌 대비 일일 최대 손실 초과 시 거래 중단 |
| **반대 신호 청산** | exit_on_opposite_signal | `bool` | 보유 중 반대 Entry 발생 시 즉시 청산 |

- `Strategy` trait에 `exit_config()` 메서드 추가 — 엔진 레벨 리스크 관리 자동 적용
- `enrich_signal()`: Entry 신호에 SL/TP 가격 자동 계산, 트레일링/수익잠금은 metadata로 executor 전달
- 6가지 프리셋: `for_day_trading()`, `for_mean_reversion()`, `for_grid_trading()`, `for_rebalancing()`, `for_leverage()`, `for_momentum()`
- 빌더 패턴: `.stop_loss()`, `.take_profit()`, `.trailing_stop()`으로 프리셋 커스터마이징 가능
- SDUI Schema Registry 확장 — 리스크 타입별 UI 폼 자동 생성
- 16개 전략 모두 `exit_config()` 기본 구현 + 694줄 테스트 (exit_config_test.rs)

#### 🗑️ 심볼 무결성 관리 시스템

**DB 함수**:
- `delete_symbol_cascade(ticker, market)` — 심볼 삭제 + 연쇄 정리
- `cleanup_orphan_symbol_data()` — 고아 데이터 일괄 정리

**API 엔드포인트**:
- `DELETE /api/v1/dataset/symbols/:ticker?market=KR` — 심볼 삭제
- `POST /api/v1/dataset/symbols/cleanup-orphans` — 고아 정리

**Repository**: `symbol_info.rs`에 `delete_symbol()`, `cleanup_orphans()` 추가

#### ⚡ 스크리닝 성능 최적화

- **배치 kline 쿼리**: `get_cached_klines_batch()` — 윈도우 함수로 N+1 쿼리 제거 (300+ 개별 쿼리 → 1 쿼리)
- **Redis 구조적 특성 캐시**: `CachedStructuralFeatures` — 4시간 TTL, 첫 호출 후 서브초 응답
- **모멘텀 데이터 왜곡 필터**: `end_price / start_price <= 20` — 주식분할 아티팩트 자동 제거
- **결과수 제한 필터 수정**: 구조적 필터 이후 limit/offset 적용 (기존: 미동작)

#### 🚀 GlobalScore 동시 처리

**파일**: `trader-collector/src/modules/global_score_sync.rs`

- 순차 for 루프 → Semaphore(10) 기반 동시 처리
- 100개 청크 단위 처리 + 체크포인트 호환
- AtomicUsize 카운터로 안전한 동시 업데이트
- 2000개 심볼 기준: ~50초 → ~5-8초

### Fixed

- **OpportunityMap 필드명 불일치**: `r.global_score` → `r.overall_score` (프론트엔드 Screening.tsx)
  - 원인: 백엔드 DTO 필드명 `overall_score`와 프론트엔드 참조 `global_score` 불일치
  - 186개 종목 모두 기본값 50으로 표시되던 문제 해결
- **스크리닝 결과수 제한 미동작**: `filter.limit`이 SQL 이후 적용되지 않던 문제 수정
- **GlobalScore null 종목 차트 표시**: score 없는 종목 OpportunityMap에서 필터링

### Changed

- `BacktestEngine` 캔들 처리 로직을 `CandleProcessor`로 추출하여 엔진 코드 대폭 축소
- `ExitConfig` 구조 변경: 단일 struct → 5개 서브 config struct 분리
- Schema Registry: exit_config fragment 완전 재작성 (중첩 필드, section 지원)
- `get_klines_batch_readonly()` 추가 — 스크리닝용 Redis 스킵 배치 조회
- 16개 전략에 `exit_config()` trait 메서드 기본 구현 추가

---

## [0.8.1] - 2026-02-07

> **StructuralFeatures Decimal 통합, LiveExecutor, DCA 전략 그룹, GlobalScore 활용 재설계, 백테스트 CLI 대폭 개선**: StructuralFeatures 타입을 f64에서 Decimal로 통합하여 금융 계산 정밀도를 높이고, LiveExecutor 실거래 실행기와 DCA 전략 그룹(Grid, MagicSplit, InfinityBot)을 추가했습니다. 전략 컨셉에 맞게 GlobalScore 활용 방식을 전면 재설계하고, 백테스트 CLI에 TOML 설정 파일 기반 실행, 멀티에셋 백테스트, Signal Analysis Report 자동 생성 기능을 도입했습니다.

### Added

#### 🔧 LiveExecutor - 실거래 실행기 (1,026줄)

**파일**: `trader-execution/src/live_executor.rs`

- `LiveExecutor` - `OrderExecutionProvider` trait 기반 실거래 주문 실행기
- 의존성 주입으로 거래소 교체 가능 (KIS, Binance 등)
- `SignalProcessor` trait 완전 구현 (포지션 관리, 잔고 추적, 체결 내역)
- Position ID / Group ID 시스템 네이티브 지원

**OrderExecutionProvider trait** (`trader-core/src/domain/exchange_provider.rs`, +95줄)
- `submit_order()` - 주문 제출
- `cancel_order()` - 주문 취소
- `get_order_status()` - 주문 상태 조회
- `get_filled_quantity()` - 체결 수량 조회

#### 📊 DCA 전략 그룹 통합

Grid Trading, MagicSplit, InfinityBot을 `DcaStrategy`로 통합:

| 변형 | position_id 형식 | group_id 형식 |
|------|-----------------|---------------|
| Grid | `{ticker}_grid_L{level}` | `grid_{base_price}_{timestamp}` |
| MagicSplit | `{ticker}_split_L{level}` | `split_{ticker}_{timestamp}` |
| InfinityBot | `{ticker}_inf_R{round}` | `inf_{ticker}_{timestamp}` |

#### 📈 백테스트 CLI 대폭 개선 (+643줄)

**TOML 설정 파일 기반 실행** (`config/backtest/*.toml`, 10개 파일)
- `trader backtest --config config/backtest/rsi.toml` - 설정 파일로 백테스트 실행
- 전략별 사전 정의 설정: rsi, bollinger, grid, magic_split, infinity_bot, haa, xaa, compound_momentum, stock_rotation, volatility
- 자동 차트 생성 (equity curve, drawdown, trade markers)
- 테스트 시나리오 문서 (`config/backtest-test-scenarios.md`)

**멀티에셋 백테스트** (`run_multi_asset_backtest()`)
- 다중 심볼 전략 (HAA, XAA, StockRotation, CompoundMomentum 등) StrategyContext 통합 실행
- `is_multi_asset_strategy()` / `extract_universe()` 헬퍼로 자동 전략 분류
- 스마트 심볼 해석: "005930" → "005930.KS", "005930.KQ" 순서로 후보 시도

**Signal Analysis Report** (`generate_signal_analysis()`, 205줄)
- Entry/Exit 균형 분석 (미청산 포지션 검출)
- 레벨별 진입/청산 분포 (Grid, MagicSplit 등 DCA 전략용)
- 시간순 신호 시퀀스 트레이스
- 검증 체크리스트 자동 생성 (Claude 검증 워크플로우 연동)

#### 📋 Strategy Watched Tickers

**마이그레이션** (`migrations_v2/09_strategy_watched_tickers.sql`)
- `strategy_watched_tickers` 테이블 - 전략별 관심 종목 관리
- source 구분: `config` (고정 티커), `dynamic` (스크리닝/유니버스)
- Collector가 관심 종목 데이터를 우선 수집

**Repository** (`trader-api/src/repository/strategy_watched_tickers.rs`, +101줄)
- CRUD 작업 + 전략별/종목별 조회
- Collector 연동용 전체 관심 종목 조회

**API 라우트 확장** (`trader-api/src/routes/strategies.rs`, +119줄)
- 전략별 관심 종목 CRUD 엔드포인트

#### 🔄 SignalProcessor trait 확장 (+222줄)

- `positions_by_group(group_id)` - 그룹별 포지션 조회
- `position_keys_by_group(group_id)` - 그룹별 포지션 키 목록
- `group_unrealized_pnl(group_id, prices)` - 그룹 미실현 손익 계산
- 공통 유틸리티 9개 함수 추가 (수수료 계산, 슬리피지, 포지션 키 생성 등)

### Changed

#### 🔬 StructuralFeatures Decimal 통합 리팩토링

**핵심 변경**: f64 기반 `StructuralFeatures` (trader-analytics)를 Decimal 기반 (trader-core)으로 통합

- `trader-core/src/domain/analytics_provider.rs` - `breakout_score()`, `is_alive_consolidation()`, `Default` impl 추가
- `trader-analytics/src/structural_features.rs` - `from_candles()` 3-arg 통합 (IndicatorEngine 기반)
- `trader-analytics/src/indicators/structural.rs` - 레거시 struct/impl 완전 제거 (-453줄), re-export + 상수만 유지
- `trader-analytics/src/route_state_calculator.rs` - f64 → Decimal 비교로 전환, `dec!()` 매크로 사용
- `trader-analytics/src/global_scorer.rs` - Decimal 비교로 전환
- `trader-analytics/src/backtest/engine.rs` - IndicatorEngine 통합
- `trader-analytics/src/analytics_provider_impl.rs` - IndicatorEngine 통합
- `trader-api/src/repository/screening.rs` - Calculator 사용, Decimal→f64 변환
- `trader-api/src/cache/structural.rs` - import 경로 변경 (trader-core 정식 타입 사용)
- `trader-collector/src/modules/global_score_sync.rs` - Calculator 사용

#### 🎯 GlobalScore 활용 전면 재설계 (10개 전략)

전략 컨셉에 따라 GlobalScore 활용 방식을 전면 재분류:

**GlobalScore 필터 완전 제거** (고정 심볼/자체 모멘텀 전략):
- `asset_allocation.rs` - US ETF 자체 모멘텀 계산 사용
- `compound_momentum.rs` - US ETF 자체 모멘텀 계산 사용
- `pension_bot.rs` - US ETF 자체 모멘텀 계산 사용
- `us_3x_leverage.rs` - `get_global_score()` 헬퍼 메서드 전체 삭제
- `day_trading.rs` - 단일 티커 전략, 스크리닝 대상 아님
- `market_bothside.rs` - 레버리지/인버스 ETF, 스크리닝 대상 아님
- `momentum_surge.rs` - 고정 레버리지/인버스 ETF 리스트
- `mean_reversion.rs` - 단일 티커 전략

**GlobalScore 필터 → 강도 조정으로 전환**:
- `candle_pattern.rs` - 신규 `get_adjusted_strength()` 메서드로 신호 강도 0.75x~1.25x 동적 조정

**GlobalScore 필터 조건부 유지** (동적 Universe 전략):
- `rotation.rs` - KR 시장만 GlobalScore 필터 유지, US 시장은 스킵

**RouteState 완화** (전 전략 공통):
- 기존: `Overheat` + `Wait` + `Neutral` 차단
- 변경: `Overheat`만 차단 (과열 방지만 유지, 불필요한 진입 제한 해제)

#### 🔄 DCA 전략 그룹 개선

**Grid Trading 동적 리셋** (`dca.rs`):
- `reset_threshold_pct` 신규 파라미터 (`GridTradingConfig`, 기존 `min_global_score` 대체)
- `warmup_candles` 신규 파라미터 - 초기 N개 캔들 관찰 후 거래 시작 (기본값: 5)
- 가격이 그리드 범위를 벗어나면 자동으로 모든 포지션 청산 후 현재 가격 기준 그리드 재생성
- `candles_processed` 카운터로 워밍업 상태 추적
- MagicSplit/InfinityBot: `min_global_score` 필드 제거, `check_global_score()` 메서드 삭제

**InfinityBot 하락장 진입 허용**:
- DCA 특성상 하락장이 매수 기회 → downtrend 진입 차단 로직 제거
- `RouteState::Overheat` 가드만 유지

#### SimulatedExecutor 개선

- `position_key()` 기반 포지션 관리 (position_id 지원)
- 개별 레벨 청산 버그 수정 (Grid 전략에서 1/5만 청산되던 문제)
- `is_grid_strategy()` 레거시 함수 완전 제거

#### Exchange Provider 확장

- `trader-exchange/src/provider/kis.rs` - KIS 프로바이더 개선 (+154줄)
- `trader-exchange/src/provider/mock.rs` - Mock 프로바이더 OrderExecutionProvider 구현 (+51줄)

#### Collector 개선

- `watchlist_helper.rs` - Strategy Watched Tickers 연동, 관심종목 우선 수집 (+67줄)
- `ohlcv_collect.rs` - 수집 로직 최적화 (+44줄)
- `config.rs` - 수집 설정 확장
- `indicator_sync.rs` - 지표 동기화 StructuralFeatures 통합 반영

#### Strategy Engine 변경

- `engine.rs` - DCA 전략 그룹 팩토리 등록, 전략 초기화 개선 (+84줄)
- `lib.rs` - 모듈 exports 업데이트
- `mean_reversion.rs` - StructuralFeatures import 경로 변경, variant fallback 버그 수정

#### CLI 변경

- `trader-cli/src/main.rs` - `dotenvy::dotenv().ok()` 추가 (.env 파일 자동 로딩)

#### 프론트엔드 개선

- `SymbolDetailModal.tsx` - `formatValue()` 타입 안전성 개선 (`string | number | null` 지원)
- `Screening.tsx` - 불필요한 `parseFloat` 제거 (이미 number 타입인 필드)
- `TradingJournal.tsx` - 매매일지 UI 개선 (+68줄)
- `ExecutionsTable.tsx` - 체결 테이블 표시 개선

### Fixed

- **SimulatedExecutor 분할 청산 버그**: `position_id` 사용 전략에서 전량 대신 1/N만 청산되던 문제 수정
- **MeanReversion variant fallback 버그**: `initialize()`에서 JSON에 variant 미포함 시 `Default` 대신 `self.initial_variant` 사용하도록 수정
- **프론트엔드 RSI 비교 버그**: `parseFloat(rsi_14 || '50')` → `(rsi_14 ?? 50)` (nullish coalescing 사용)
- **StructuralFeatures 정밀도**: f64 → Decimal 전환으로 금융 데이터 계산 정밀도 향상

### Removed

- `trader-analytics/src/indicators/structural.rs`의 `StructuralFeatures` struct 및 모든 impl (453줄)
- `is_grid_strategy()` 레거시 함수 (signal_processor.rs, simulated_executor.rs, lib.rs에서 제거)
- `us_3x_leverage.rs`의 `get_global_score()` 헬퍼 메서드 전체 삭제
- `dca.rs`의 `check_global_score()` 메서드 삭제, `GridTradingConfig.min_global_score` → `reset_threshold_pct`로 교체
- 10개 전략의 `can_enter()`에서 GlobalScore 진입 필터 로직 제거 (고정 심볼 전략)

---

## [0.8.0] - 2026-02-07

> **실시간 WebSocket 연동, Paper Trading, Exchange Provider 통합**: KIS 거래소 WebSocket 실시간 시세를 6단계로 완전 연동하고, Paper Trading 시뮬레이션 시스템과 Exchange Provider 통합 아키텍처를 구축했습니다.

### Added

#### 🔗 KIS WebSocket 실시간 연동 (6-Phase)

**Phase 1: 동적 구독 지원**
- `KisKrWebSocket.subscribe_live()` / `unsubscribe_live()` - 연결 중 실시간 심볼 구독/해제
- `KisUsWebSocket.subscribe_live()` / `unsubscribe_live()` - 해외 심볼 동적 구독
- `KisKrMarketStream` / `KisUsMarketStream` - `started` 상태에서도 동적 구독 가능

**Phase 2: 동시 수신 (Bridge Task)**
- `UnifiedMarketStream` - KR/US 스트림을 `mpsc` 채널로 통합, `tokio::select!` 기반 동시 폴링
- `StreamCommand` enum - Subscribe/Unsubscribe 명령 채널
- 순차 폴링 → 병렬 수신으로 지연 제거

**Phase 3: Singleton 스트림 관리**
- `MarketStreamHandle` - credential_id별 싱글턴 스트림 핸들 (`services/market_stream.rs`)
- 참조 카운트 기반 심볼 관리 (`HashMap<String, usize>`)
- `get_or_create_market_stream()` - Lazy 초기화 + 캐시

**Phase 4: 전략 실행 시 스트림 자동 연결**
- 전략 시작 시 관련 거래소 WebSocket 스트림 자동 생성 및 심볼 구독
- `subscribe_strategy_symbols_to_stream()` - 전략 심볼 일괄 구독

**Phase 5: 서버 시작 순서 최적화**
- `AppState` → `WsState` 순서로 초기화 (DB 준비 후 WebSocket 시작)
- Mock 시뮬레이터와 실제 KIS 스트림 분리

**Phase 6: 프론트엔드 WebSocket 브릿지**
- `WsState.market_streams` - 프론트엔드 구독 → 거래소 스트림 전파
- `forward_subscribe_to_exchange_streams()` - `market:{symbol}` 채널 자동 브릿지
- `MarketDataAggregator.handle_event()` - 개별 이벤트 처리 public API 추가

#### 📝 Paper Trading 시스템

**백엔드 API** (`routes/paper_trading.rs`, 1,527줄)
- `POST /api/v1/paper-trading/start` - 페이퍼 트레이딩 세션 시작
- `GET /api/v1/paper-trading/accounts` - 가상 계좌 목록 조회
- `GET /api/v1/paper-trading/positions` - 가상 포지션 조회
- `GET /api/v1/paper-trading/executions` - 가상 체결 내역 조회
- `POST /api/v1/paper-trading/reset` - 계좌 초기화

**프론트엔드** (`PaperTrading.tsx`, 638줄)
- Simulation 페이지 내 Paper Trading 탭
- 계좌 현황, 포지션, 체결 내역 실시간 표시
- 전략 시작/중지/리셋 컨트롤

**Signal Processor 서비스** (`services/signal_processor.rs`, 258줄)
- API 서버에서 Signal 처리 파이프라인 관리
- SimulatedExecutor 연동

**TypeScript 바인딩** (10개 파일)
- PaperTradingAccount, PaperTradingPosition, PaperTradingExecution
- PaperTradingStartRequest, PaperTradingSessionResponse 등

#### 🔄 Exchange Provider 통합

**KIS Provider 통합** (`provider/kis.rs`, 894줄)
- 기존 `kis_kr.rs` + `kis_us.rs` → `kis.rs` 단일 모듈로 통합
- `KisExchangeProvider` - KR/US 통합 프로바이더
- 시장 자동 판별 (`is_korean_symbol()`)

**Mock Exchange Provider** (`provider/mock.rs`, 1,241줄)
- 개발/테스트용 가상 거래소 프로바이더
- 시뮬레이션 주문 실행, 가상 잔고 관리
- `ExchangeProvider` trait 완전 구현

**KIS Client 통합** (`connector/kis/client.rs`, 562줄)
- KR/US 공통 HTTP 클라이언트 로직 추출
- 인증, 요청 빌더, 응답 파싱 공통화

#### 📊 Core Domain 확장

**Account 모듈** (`domain/account.rs`, 195줄)
- `AccountInfo`, `AccountBalance` 구조체
- 계좌 정보 표준화 (거래소 중립적)

**Exchange Types** (`domain/exchange_types.rs`, 299줄)
- `ExchangeConstraints`, `OrderConstraints` 구조체
- 거래소별 제약조건 표준화 (최소 주문량, 호가 단위 등)

**StrategyContext 확장** (`domain/context.rs`, +295줄)
- `AccountInfo`, `ExchangeConstraints` 통합
- 전략에서 계좌 정보 및 거래소 제약 조건 접근 가능

#### 📈 Collector 확장

**Market Breadth 동기화** (`modules/market_breadth_sync.rs`, 106줄)
- 20일 이평선 위 종목 비율 계산
- 시장 전반적 건강도 지표

**OHLCV 수집 개선** (`modules/ohlcv_collect.rs`, +248줄)
- 수집 로직 최적화 및 에러 핸들링 개선

#### 🖥️ 프론트엔드 확장

**Settings 페이지** (`Settings.tsx`, +257줄)
- 거래소 자격증명 관리 UI 확장
- API 키 업데이트 기능

**Strategy 모달** (`AddStrategyModal.tsx`, +131줄; `SDUIEditModal.tsx`, +150줄)
- 전략 추가/편집 모달 기능 확장
- SDUI 동적 폼 개선

**API 클라이언트** (`api/client.ts`, +295줄)
- Paper Trading API 연동
- 자격증명 업데이트 API 추가

**WebSocket 개선** (`createWebSocket.ts`, 변경)
- 실시간 시세 구독/해제 개선
- 재연결 로직 강화

### Changed

#### Exchange 아키텍처 변경
- `trader-exchange/src/provider/kis_kr.rs` + `kis_us.rs` → `kis.rs`로 통합 (삭제됨)
- `UnifiedMarketStream` - 순차 폴링 → `mpsc` 채널 기반 병렬 이벤트 수신
- `MarketStream` trait - 동적 구독 지원 (started 상태에서도 subscribe_ticker 가능)
- `ExchangeProvider` trait - `account_info()`, `exchange_constraints()` 메서드 추가
- `SimulatedExchange` - 개선된 시뮬레이션 로직

#### API 서버 변경
- `AppState` - `market_streams`, `subscriptions` 필드 추가
- `WsState` - `market_streams` 필드 추가 (프론트엔드 → 거래소 스트림 브릿지)
- `main.rs` - 초기화 순서 최적화 (AppState → WsState)
- `openapi.rs` - Paper Trading 엔드포인트 추가
- `strategies.rs` - 전략 시작 시 스트림 자동 구독

#### 프론트엔드 변경
- `Simulation.tsx` - Paper Trading 탭 추가
- `Dashboard.tsx` - 포트폴리오 차트 개선
- `Screening.tsx` - 종목 상세 링크 개선
- `SymbolDetail.tsx` - 차트 인터랙션 개선

#### Collector 변경
- `global_score_sync.rs` - 동기화 로직 개선
- `indicator_sync.rs` - 지표 동기화 최적화

### Fixed
- WebSocket 메시지 타입 직렬화 개선 (`websocket/messages.rs`)
- KIS OAuth 토큰 캐시 동시성 문제 수정
- `historical.rs` - 미사용 코드 정리

### Removed
- `trader-exchange/src/provider/kis_kr.rs` - `kis.rs`로 통합
- `trader-exchange/src/provider/kis_us.rs` - `kis.rs`로 통합

---

## [0.7.4] - 2026-02-06

> **API 성능 최적화 및 프론트엔드 기능 확장**: N+1 쿼리 제거, 배치 처리, Materialized View 활용으로 API 성능을 대폭 개선하고, 백테스트 저널 및 트레이딩 저널 UI를 강화했습니다.

### Performance

#### 🚀 API Repository 최적화

**N+1 쿼리 제거**
- `global_score.rs`: Fundamental/Volume Percentile 배치 조회 (HashMap 사전 캐싱)
- `journal.rs`: cost_basis 단일 쿼리 + Rust 그룹핑, sync_executions ANY($1) 배치 중복 체크
- `klines.rs`: Rust `reverse()` → DB 서브쿼리 정렬 패턴

**UNNEST 배치 Upsert**
- `score_history.rs`: 루프 INSERT → `UNNEST($1::type[], ...)` 배치 (500건/배치)
- `symbol_info.rs`: 루프 INSERT → UNNEST 배치 (500건/배치)

**Materialized View 활용**
- `screening.rs`: 섹터 RS 4단계 CTE → `mv_sector_rs` MV 빠른 경로 (days=20 기본값)
- `global_score.rs`: RouteState 실시간 캔들 계산 → `symbol_fundamental` DB JOIN (Collector 사전계산 활용)

**Redis 캐싱**
- GlobalScore 캐시 (6시간 TTL) - 장 마감 후 계산, 다음 마감까지 유효
- SevenFactor 분석 캐시 (2시간 TTL)

#### 🔧 Collector 최적화
- OHLCV 수집 모듈 리팩토링 (코드 정리 및 배치 효율 개선)
- Signal Performance 동기화 리팩토링
- Fundamental/Macro Data 동기화 개선
- `mv_sector_rs` 주기적 REFRESH 추가 (Group B 워크플로우)

### Added

#### 📊 프론트엔드 기능

**백테스트 저널 모달** (`BacktestJournalModal.tsx`)
- 백테스트 결과를 저널 형식으로 상세 조회
- 매수/매도 개별 거래 기록 표시

**공통 Form 컴포넌트** (`Form.tsx`)
- 재사용 가능한 폼 빌더 컴포넌트

**트레이딩 저널 강화** (`TradingJournal.tsx`)
- 거래 기록 상세 분석 UI 대폭 확장
- 포맷 유틸리티 함수 확장 (`format.ts`)

**대시보드 개선** (`Dashboard.tsx`)
- 포트폴리오 차트 인터랙션 개선
- 인디케이터 필터 패널 업데이트

#### 🔧 백엔드 기능

**백테스트 엔진**
- `TradeResultItem` 타입 추가 (ts-rs 바인딩 포함)
- 모든 거래 기록(`all_trades`) 변환 지원

**DCA 전략 테스트** (`dca_test.rs`)
- DCA 전략 종합 테스트 스위트 (364줄)

**시뮬레이션 실행기** (`simulated_executor.rs`)
- 실행 로직 개선

**TypeScript 바인딩** (ts-rs 자동 생성)
- Alert 타입: AlertChannel, AlertHistory, AlertStats, AlertStatus, AlertType
- Signal 타입: SignalPerformanceResponse, SignalReturnPoint, SignalStrategyStats 등

#### 🗄️ DB 마이그레이션 (`19_api_performance_optimization.sql`)
- `idx_position_snapshots_current` - DISTINCT ON 최적화 인덱스
- `idx_symbol_fundamental_route_state_all` - RouteState JOIN 최적화 인덱스
- `mv_sector_rs` - 섹터 Relative Strength Materialized View
- `refresh_mv_sector_rs()` - MV 갱신 함수
- 마이그레이션 통합기: `19_` 패턴 → `07_performance_optimization` 그룹 등록

### Changed
- Analytics 모듈 가시성 변경 (`pub mod` 전환)
- Screening/Ranking 쿼리 파라미터 타입 개선
- CLI 백테스트/임포트/전략 테스트 명령어 개선
- Collector 환경 변수 설정 확장 (`.env.example`)

### Fixed
- BacktestRunResponse `all_trades` 필드 누락 수정
- SyncResult 타입 불일치 (i64 → i32) 수정
- score_history.rs 미사용 `warn` import 정리

---

## [0.7.3] - 2026-02-06

> **다채널 알림 및 실시간 대시보드 릴리즈**: Discord, Slack, Email, SMS 알림 지원과 실시간 시장 정보 표시 기능이 추가되었습니다.

### Added

#### 🔔 다채널 알림 시스템

**알림 프로바이더 확장** (`trader-notification/`)
- **Discord**: Webhook 기반 알림 (Rich Embed 지원)
- **Slack**: Incoming Webhook 기반 알림
- **Email**: SMTP 기반 이메일 알림 (TLS 지원)
- **SMS**: Twilio API 기반 SMS 알림

**알림 API** (`trader-api/src/routes/credentials/`)
- 각 서비스별 설정 CRUD API
- **저장 전 테스트 기능** (`/test/new` 엔드포인트) - 설정 저장 없이 실제 메시지 전송 테스트
- AES-256-GCM 암호화 자격증명 저장
- 감사 로그 기록

#### 📊 실시간 대시보드 인디케이터

**헤더 인디케이터** (`HeaderIndicators.tsx`)
- 시장 상태 표시 (KRX, NYSE, Crypto 개장/폐장)
- 시장 온도 (Market Breadth) - 20일 이평선 위 종목 비율
- 매크로 환경 (USD/KRW 환율, NASDAQ 변동률, 위험도)
- 1분 주기 자동 갱신

**알림 드롭다운** (`NotificationDropdown.tsx`)
- 알림 히스토리 조회 (읽음/안읽음 필터)
- 개별/전체 읽음 처리
- 알림 타입별 아이콘 표시

#### 🛠️ 마이그레이션 도구 (`trader-cli migrate`)
- `verify` - 마이그레이션 검증 (중복, CASCADE, 순환 검출)
- `consolidate` - 통합 마이그레이션 계획 생성
- `graph` - 의존성 그래프 시각화 (Mermaid)
- `apply` - 마이그레이션 적용

#### 📈 차트 라이브러리 개선

**Lightweight Charts 통합**
- `BaseChart.tsx` - 공통 차트 컴포넌트
- `useLightweightChart` 훅 - 차트 생성/관리
- `chartUtils.ts` - 차트 유틸리티 함수

**신호 마커 오버레이** (`SignalMarkerOverlay.tsx`)
- 진입/청산 신호 시각화
- 신호 상세 팝업 (`SignalDetailPopup.tsx`)

#### 🤖 전략 추가/개선

**DCA 전략** (`dca.rs`) - Dollar Cost Averaging
- 정기 매수 전략
- 가격 하락 시 추가 매수 로직

**Screening Based 전략** (`screening_based.rs`)
- 스크리닝 결과 기반 자동 매매
- 동적 종목 선정

**Mean Reversion 전략 단순화**
- 725줄 → 350줄 (-52%)
- 핵심 로직만 유지

#### 🔧 CLI 도구 확장

**시뮬레이션 테스트** (`sim_test.rs`)
- 시그널 기반 시뮬레이션 검증
- 성과 분석 리포트

**스케줄러 모듈** (`scheduler.rs`)
- Cron 스케줄 기반 작업 관리
- 매크로 데이터 동기화 스케줄링

#### 📑 Signal Performance API
- 시그널 성과 분석 엔드포인트
- 전략/심볼/타입별 통계

### Changed

- **Settings 페이지**: 알림 서비스 등록 UI 개선, 저장 전 테스트 기능 추가
- **Layout**: 헤더에 시장 인디케이터 및 알림 드롭다운 통합
- **API Client**: 알림 서비스 API 함수 추가 (4개 프로바이더 × 5개 함수)
- **README**: 투자 관련 안내사항(Disclaimer) 추가

### Database

- **10_restore_used_tables.sql**: 테이블 복원
- **11_fix_watchlist_schema.sql**: 워치리스트 스키마 수정
- **12_sync_checkpoint.sql**: 동기화 체크포인트
- **13_missing_views.sql**: 누락된 뷰 추가
- **14_default_alert_rules.sql**: 기본 알림 규칙
- **15_signal_performance.sql**: 시그널 성과 테이블
- **16_alert_history.sql**: 알림 히스토리 테이블
- **17_notification_providers.sql**: 알림 프로바이더 테이블 (Discord, Slack, Email, SMS)
- **18_add_symbol_type_to_view.sql**: 심볼 타입 뷰 추가
- **migrations_v2/**: 통합 마이그레이션 7개 (선택적 사용)

---

## [0.7.2] - 2026-02-06

> ⚠️ **레거시 정리 릴리즈**: klines 테이블 및 관련 코드가 제거되었습니다.
> DB 마이그레이션 실행 시 `08_remove_klines_table.sql`, `09_remove_legacy_tables.sql`이 적용됩니다.

### Added

#### 📊 차트 생성 CLI 대폭 확장 (`chart_gen.rs` - +750줄)
- **3패널 레이아웃**: 캔들스틱+Volume, Equity Curve, Drawdown
- 신호 마커 표시 기능 (진입/청산 시점 시각화)
- 타임스탬프 overflow 방지 로직 (f64 변환)
- 상승/하락 캔들 색상 구분

#### 🛠️ 프론트엔드 로깅 유틸리티 (`logger.ts`)
- 프로덕션 최적화 로깅 시스템
- 모듈별 prefix 자동 추가 (`[WebSocket]`, `[Backtest]` 등)
- 개발 모드: log/warn/error 모두 출력
- 프로덕션 모드: error만 출력

#### 📑 TypeScript 타입 추가
- `ScoreHistoryQuery`, `ScoreHistoryResponse` - 스코어 히스토리 조회
- `ScoreHistorySummary` - 스코어 요약 정보
- `ClearCacheResponse` - 캐시 삭제 응답

### Changed

#### ⚡ 백테스트 엔진 개선 (`engine.rs` - +400줄)
- 다중 타임프레임 백테스트 지원 강화
- 다중 심볼 데이터 매칭 로직 개선
- 성능 최적화

#### 🧪 Strategy Test CLI 강화 (`strategy_test.rs` - +500줄)
- 전략 테스트 상세 진단 정보 확장
- 신호 발생 조건 추적 개선
- 다중 심볼 테스트 지원 강화

#### 📈 OHLCV 수집 로직 개선 (`ohlcv_collect.rs` - +200줄)
- 증분 수집 최적화
- 에러 핸들링 강화
- 로깅 개선

#### 🔄 전략 리팩토링 (6개)
- `asset_allocation.rs` - StrategyContext 연동 개선
- `compound_momentum.rs` - 신호 생성 로직 최적화
- `day_trading.rs` - 거래량 조건 정밀화
- `mean_reversion.rs` - RSI/볼린저 조건 개선
- `momentum_surge.rs` - 모멘텀 계산 안정화
- `rsi_multi_tf.rs` - 다중 타임프레임 연동 개선

#### 🖥️ 프론트엔드 개선
- `client.ts` - API 클라이언트 리팩토링 (+140줄)
- WebSocket 연결 안정성 개선
- 다수 컴포넌트 타입 안전성 강화

### Removed

#### 🧹 레거시 코드 대폭 정리

**klines 관련 코드 제거**:
- `DataManager`에서 `KlineRepository` 및 관련 메서드 제거
- `manager.rs` - ~140줄 제거 (ticker_to_symbol, store_kline 등)
- `timescale.rs` - ~286줄 제거 (KlineRepository 전체)

**더 이상 사용되지 않는 테이블 제거**:
- `klines` 테이블 → `ohlcv` 테이블로 통합
- `signals` 테이블 → `signal_marker`로 대체
- `api_keys` 테이블 → `exchange_credentials`로 대체
- `performance_snapshots` 테이블 (미사용)
- `users` 테이블 (인증 시스템 미사용)

### Database

- **08_remove_klines_table.sql**: klines 테이블 삭제 (ohlcv 통합)
- **09_remove_legacy_tables.sql**: 레거시 테이블 4개 삭제

---

## [0.7.0] - 2026-02-05

> ⚠️ **전략 리팩토링 진행 중**: 이 버전은 대규모 전략 통합 및 마이그레이션 정리 작업이 포함되어 있습니다.
> 일부 전략이 삭제되거나 이름이 변경되었습니다. 기존 전략 설정을 사용하는 경우 마이그레이션이 필요할 수 있습니다.

### Added

#### 🌐 데이터 프로바이더 확장

**네이버 금융 크롤러 (국내)**
- **NaverFinanceFetcher** (`trader-data/src/provider/naver.rs`)
  - 국내 주식 펀더멘털 데이터 크롤링
  - 시가총액, PER, PBR, ROE, EPS, BPS, 배당수익률
  - 52주 최고/최저, 섹터 정보
  - scraper 크레이트 기반 HTML 파싱
  - Rate limiting (기본 300ms 딜레이)

**Yahoo Fundamental Provider (해외)** (신규)
- **YahooFundamentalFetcher** (`trader-data/src/provider/yahoo_fundamental.rs` - 555줄)
  - 글로벌 주식 펀더멘털 데이터 수집
  - **밸류에이션**: PER(trailing/forward), PBR, PSR, EPS, BPS
  - **시장 정보**: 시가총액, 52주 고저, 평균 거래량
  - **수익성**: ROE, ROA, 영업이익률, 순이익률, 매출총이익률
  - **성장성**: 매출성장률, 이익성장률
  - **기타**: 섹터, 산업, 베타, 부채비율

**Collector 통합**
  - `NAVER_FUNDAMENTAL_ENABLED` 환경변수 지원
  - `NAVER_REQUEST_DELAY_MS` 설정 (기본: 300ms)
  - Yahoo Finance 대비 수집 속도 개선 (3.5시간 → 2시간 예상)

#### 🛠️ CLI 도구 확장

**Strategy Test CLI** (`trader-cli/src/commands/strategy_test.rs` - 661줄)
- UI와 동일한 환경에서 전략을 테스트하고 상세 진단 정보 출력
- **UI 동일 흐름**: JSON config → StrategyContext 주입 → 전략 초기화 → 백테스트
- **상세 진단**: 신호 발생 여부, 거래 내역, 조건 평가 결과
- **거래 분석**: 진입/청산 시점, 가격, PnL 상세
- **문제 원인 분석**: 신호 미발생 시 원인 추적
- **다중 심볼 지원**: 로테이션/자산배분 전략 테스트
- 사용 예시:
  ```bash
  # RSI 전략 테스트 (단일 심볼)
  trader strategy-test --strategy rsi --symbol 005930 --market KR

  # 다중 심볼 테스트 (로테이션 전략)
  trader strategy-test --strategy rotation --symbols "SPY,QQQ,IWM" --market US

  # JSON config로 테스트
  trader strategy-test --strategy grid --config '{"ticker":"005930","grid_count":10}'
  ```

#### 🧩 전략 공통 모듈 확장

**Exit Config 프리셋** (`strategies/common/exit_config.rs` - 130줄 추가)
- 전략 유형별 표준화된 청산 설정 프리셋:
  - `for_day_trading()`: 좁은 손절(2%), 익절(4%), 반대 신호 청산
  - `for_mean_reversion()`: 중간 손절(3%), 넓은 익절(6%)
  - `for_grid_trading()`: 손절 비활성화, 익절만(3%)
  - `for_rebalancing()`: 손절/익절 비활성화 (리밸런싱으로 관리)
  - `for_leverage_etf()`: 필수 손절(5%), 넓은 익절(10%), 트레일링 스탑

**Global Score Utils** (`strategies/common/global_score_utils.rs` - 75줄)
- `get_score()`: 종목별 글로벌 스코어 조회
- `select_top_tickers()`: 스코어 기준 상위 종목 선택
- `calculate_score_weight()`: 스코어 기반 포지션 가중치 계산
- `adjust_strength_by_score()`: 스코어로 신호 강도 조정

#### 🧪 전략 테스트 확장 (16개 신규)
- **asset_allocation_test.rs** - 자산배분 전략 테스트
- **compound_momentum_test.rs** - 복합 모멘텀 테스트
- **day_trading_test.rs** - 데이 트레이딩 테스트
- **infinity_bot_test.rs** - 무한매수봇 테스트
- **momentum_surge_test.rs** - 모멘텀 급등 테스트
- **market_bothside_test.rs** - 시장 양방향 테스트
- **mean_reversion_test.rs** - 평균회귀 테스트
- **momentum_power_test.rs** - 모멘텀 파워 테스트
- **pension_portfolio_test.rs** - 연금 포트폴리오 테스트
- **range_trading_test.rs** - 박스권 매매 테스트
- **rotation_test.rs** - 로테이션 전략 테스트
- **rsi_multi_tf_test.rs** - RSI 멀티 타임프레임 테스트
- **sector_vb_test.rs** - 섹터 변동성 돌파 테스트
- **small_cap_factor_test.rs** - 소형주 팩터 테스트
- **us_3x_leverage_test.rs** - 미국 3배 레버리지 테스트
- **volatility_breakout_test.rs** - 변동성 돌파 테스트

#### 📊 분석 모듈 확장
- **correlation.rs** - 종목 간 상관관계 분석
- **volume_profile.rs** - 거래량 프로파일 분석
- **survival.rs** - 생존 분석 (전략 지속성)
- **sector_rs.rs** - 섹터 상대 강도 분석
- **weekly_ma.rs** - 주봉 이동평균 지표
- **volume.rs** - 거래량 관련 지표 확장

### Changed

#### 🔄 전략 대폭 리팩토링 (Breaking Changes)

**삭제된 전략 (15개)**:
- `all_weather.rs` → `asset_allocation.rs`로 통합
- `baa.rs` → `asset_allocation.rs`로 통합
- `bollinger.rs` → `mean_reversion.rs`로 통합
- `dual_momentum.rs` → `rotation.rs`로 통합
- `grid.rs` → `day_trading.rs`로 통합
- `haa.rs` → `asset_allocation.rs`로 통합
- `magic_split.rs` → 삭제 (사용률 저조)
- `market_cap_top.rs` → `rotation.rs`로 통합
- `market_interest_day.rs` → `day_trading.rs`로 통합
- `obv.rs` (지표) → `volume.rs`로 통합
- `rsi.rs` → `mean_reversion.rs`로 통합
- `sector_momentum.rs` → `rotation.rs`로 통합
- `sma.rs` → 삭제 (더 이상 사용되지 않음)
- `stock_rotation.rs` → `rotation.rs`로 통합
- `volatility_breakout.rs` → 삭제 (day_trading으로 대체)
- `xaa.rs` → `asset_allocation.rs`로 통합

**신규 통합 전략 (4개)**:
- `asset_allocation.rs` - All Weather, HAA, XAA, BAA 통합
- `day_trading.rs` - Grid, Market Interest Day 통합
- `mean_reversion.rs` - Bollinger, RSI 통합
- `rotation.rs` - Dual Momentum, Sector Momentum, Stock Rotation, Market Cap Top 통합

#### 🗄️ 마이그레이션 정리 (19 → 7개로 통합)
- `01_core_foundation.sql` - 기본 스키마, ENUM, 확장 (기존 01~04 통합)
- `02_data_management.sql` - 심볼 정보, OHLCV, 펀더멘털 (기존 04~05 통합)
- `03_trading_analytics.sql` - 매매일지, 포트폴리오 분석 (기존 06~08 통합)
- `04_strategy_signals.sql` - 전략, 신호, 알림 시스템 (기존 09 통합)
- `05_evaluation_ranking.sql` - Reality Check, 랭킹 시스템 (기존 10, 12 통합)
- `06_user_settings.sql` - 관심종목, 스크리닝 프리셋, KIS 토큰 (기존 13~17 통합)
- `migrations/README.md` - 마이그레이션 가이드 업데이트

#### 📚 OpenAPI 문서화 대폭 확장

**openapi.rs** (+124줄)
- 모든 주요 API 라우트에 OpenAPI 3.0 어노테이션 추가
- Swagger UI (`/swagger-ui`)에서 인터랙티브 API 문서 제공
- 요청/응답 스키마 자동 문서화 (ToSchema derive)

**라우트별 문서화**:
- `analytics/` - 차트, 지표, 성과, 동기화 API
- `backtest/` - 백테스트 실행 및 결과 조회
- `journal.rs` - 매매일지 CRUD
- `market.rs` - 시장 데이터 조회
- `orders.rs` - 주문 관리
- `portfolio.rs` - 포트폴리오 조회
- `strategies.rs` - 전략 CRUD

#### 🔄 전략 공통 모듈 적용

**Exit Config 프리셋 적용** (16개 전략)
- `day_trading.rs`, `sector_vb.rs`, `momentum_surge.rs` → `for_day_trading()`
- `mean_reversion.rs`, `range_trading.rs`, `candle_pattern.rs` → `for_mean_reversion()`
- `infinity_bot.rs` → `for_grid_trading()`
- `asset_allocation.rs`, `rotation.rs`, `pension_bot.rs` → `for_rebalancing()`
- `us_3x_leverage.rs` → `for_leverage_etf()`

**Global Score Utils 통합**
- 로테이션/자산배분 전략에서 GlobalScore 기반 종목 선택 적용
- 스코어 가중치 기반 포지션 사이징 통합

#### 🧹 Clippy 경고 전체 수정 (50+ → 0)
- `manual_clamp` 패턴 수정: `.max(a).min(b)` → `.clamp(a, b)`
- `should_implement_trait` 수정: `from_str` → `parse` 메서드 이름 변경
- `question_mark` 수정: `let...else { return None }` → `?` 연산자
- `if_same_then_else` 수정: 동일 분기 병합
- `needless_range_loop` 수정: 인덱스 루프 → 이터레이터
- 의도적 패턴에 `#[allow]` 어트리뷰트 추가

#### 📝 문서 업데이트
- **CLAUDE.md** - v0.6.0 → v0.7.0 업데이트
- **docs/todo.md** - 전략 리팩토링 진행 상황 반영
- **docs/prd.md** - 네이버 크롤러 요구사항 추가
- **CHANGELOG.md** - Yahoo Fundamental, Strategy Test CLI 추가
- **README.md** - 전략 개발 예제 현행화

### Fixed

- **MarketType 열거형 수정** - `MarketType::Kr`, `MarketType::Us` → `MarketType::Stock`으로 통일
- **백테스트 엔진** - 다중 심볼 데이터 매칭 로직 개선
- **스크리닝 와일드카드** - 불필요한 패턴 매칭 제거
- **analytics/manager.rs** - 캐시 unwrap 패턴 안전하게 처리

### Dependencies

#### 신규 추가
- `scraper = "0.21"` - HTML 파싱 (네이버 금융 크롤링)

### Database

- 마이그레이션 파일 19개 → 7개로 통합 (63% 파일 감소)
- 총 크기 유지하면서 관리 복잡도 감소

---

## [0.6.0] - 2026-02-04

### Added

#### 📊 Multi Timeframe System (Phase 1.4)
- **다중 타임프레임 분석** - 여러 시간대 데이터 동시 분석 지원
  - `multi_timeframe_helpers.rs` (525줄) - 트렌드 분석, 시그널 결합, 다이버전스 감지
  - `timeframe_alignment.rs` (330줄) - Look-Ahead Bias 방지 타임프레임 정렬
  - `RsiMultiTimeframeStrategy` 예제 전략 구현
- **Strategy Trait 확장**
  - `multi_timeframe_config()` - 다중 TF 설정 반환
  - `on_multi_timeframe_data()` - 다중 TF 데이터 처리
- **백테스트 엔진 확장**
  - `run_multi_timeframe()` 메서드 추가
  - Secondary 타임프레임 데이터 병렬 로드
- **API 엔드포인트**
  - `GET /api/v1/market/klines/multi` - 다중 TF Kline 조회
  - `GET/PUT /api/v1/strategies/{id}/timeframes` - TF 설정 관리
- **프론트엔드 컴포넌트**
  - `MultiTimeframeSelector.tsx` - Primary/Secondary TF 선택
  - `MultiTimeframeChart.tsx` - 멀티 TF 차트 동기화
  - `useMultiTimeframeKlines.ts` - API 연동 훅 (TTL 캐싱)

#### 🔄 Data Provider Dualization (데이터 소스 이중화)
- **KRX OPEN API 연동** (`krx_api.rs` - 1,122줄)
  - 국내 주식 OHLCV 데이터 수집
  - PER/PBR/배당수익률 Fundamental 데이터
  - 섹터/업종 정보 동기화
  - 시가총액/발행주식수 조회
- **데이터 프로바이더 토글 시스템**
  - `DataProviderConfig` 구조체
  - `PROVIDER_KRX_API_ENABLED` (기본: false - 승인 전)
  - `PROVIDER_YAHOO_ENABLED` (기본: true)
  - KRX API 승인 대기 중 Yahoo Finance 단독 운영 지원

#### 📈 7Factor Scoring System
- **seven_factor.rs** (560줄) - 7개 팩터 기반 종목 평가
  - Momentum, Value, Quality, Volatility
  - Liquidity, Growth, Sentiment
  - 정규화된 점수 (0-100)
- **API 엔드포인트**
  - `GET /api/v1/ranking/7factor/{ticker}` - 개별 종목 7Factor
  - `POST /api/v1/ranking/7factor/batch` - 배치 조회

#### 📑 TypeScript 바인딩 자동 생성
- **ts-rs 기반 타입 자동 생성** (`bindings/` 디렉토리)
  - Backtest, Journal, Ranking, Screening, Strategies 타입
  - API 요청/응답 타입 안전성 보장
  - 프론트엔드 타입 동기화 자동화

#### 📋 Watchlist System (관심종목)
- **관심종목 도메인 모델** (`watchlist.rs` - 282줄)
- **Repository 구현** (`repository/watchlist.rs` - 403줄)
- **API 엔드포인트** (`routes/watchlist.rs` - 363줄)
  - `GET/POST /api/v1/watchlist` - 관심종목 목록 CRUD
  - `POST/DELETE /api/v1/watchlist/{id}/items` - 종목 추가/삭제

#### 🚀 Collector 모듈 확장
- **indicator_sync.rs** (361줄) - 지표 동기화 모듈
- **global_score_sync.rs** (228줄) - GlobalScore 동기화
- **fundamental_sync.rs** (387줄) - KRX Fundamental 동기화
- **CLI 명령어 추가**
  - `sync-indicators` - 분석 지표 동기화
  - `sync-global-scores` - GlobalScore 동기화
  - `sync-krx-fundamentals` - KRX Fundamental 동기화

### Changed

#### ⚡ Frontend Performance Optimization
- **Lazy Loading 적용** - 11개 페이지 모두 lazy() + Suspense
- **코드 스플리팅** (manualChunks)
  - `index.js`: 1,512KB → 12.5KB (**99% 감소**)
  - `vendor-echarts`: 674KB (필요 시 로드)
  - `vendor-lightweight-charts`: 175KB
  - `vendor-solid`, `vendor-tanstack`, `vendor-lucide` 분리
- **createStore 리팩토링** - 5개 페이지 상태 관리 통합
  - Strategies: ~15 signals → 4 stores (73% 감소)
  - TradingJournal: ~20 signals → 5 stores (75% 감소)
  - Screening: 29 signals → 4 stores (86% 감소)
- **커스텀 훅 추출**
  - `useStrategies`, `useJournal`, `useScreening`, `useMarketSentiment`
- **가상 스크롤** - `VirtualizedTable` 컴포넌트 (1,000+ 행 지원)
- **디바운스/쓰로틀 훅** - `useDebounce`, `useDebouncedCallback`

#### 🔧 Repository Layer 확장
- **credentials.rs** (339줄) - 자격증명 Repository
- **kis_token.rs** (210줄) - KIS 토큰 캐시 Repository
- **journal.rs** (341줄) - 매매일지 확장
- **klines.rs** (143줄) - Kline 데이터 조회 확장
- **global_score.rs** (322줄) - GlobalScore 조회 확장

#### 📡 WebSocket 개선
- Kline 브로드캐스트 활성화 (`ServerMessage::Kline`)
- 다중 타임프레임 실시간 데이터 지원

### Fixed

#### 🐛 Symbol Resolution
- **CRYPTO 심볼 해결 오류** 수정
  - Yahoo Finance 심볼 없는 CRYPTO 종목 비활성화 (446개)
  - 원본 ticker로도 검색 가능하도록 쿼리 개선
- **KRX API 엔드포인트** 수정
  - Base URL: `data-dbg.krx.co.kr`
  - Path: `/svc/sample/apis/{category}/{api_id}`

### Database

- **18_multi_timeframe.sql** - 다중 타임프레임 스키마
- **19_backtest_timeframes_used.sql** - 백테스트 TF 기록

---

## [0.5.9] - 2026-02-03

### Added

#### 🤖 Telegram Bot Integration
- **telegram_bot.rs** - 실시간 알림 봇 서비스
  - 포지션 모니터링 및 알림
  - 실시간 손익 업데이트
  - 거래 체결 알림

#### 🎨 Frontend UI Components
- **GlobalScoreBadge** - 글로벌 스코어 시각화 배지
- **RouteStateBadge** - 진입 상태 인디케이터 (ATTACK/ARMED/WAIT/OVERHEAT/NEUTRAL)
- UI 컴포넌트 export 구조 개선

#### 🗃️ Ranking System
- **12_ranking_system.sql** - 글로벌 스코어 랭킹 스키마
  - global_score 테이블 (복합 스코어링)
  - 효율적인 랭킹 쿼리를 위한 인덱스
  - 다중 타임프레임 지원 (1d, 1w, 1M)

#### 🎯 Phase 1.1.2 Implementation (Strategy Scoring System)
- **Global Scorer** - 7개 팩터 기반 종합 점수 시스템
  - `global_scorer.rs` - VolumeQuality, Momentum, ValueFactor, RouteState 등
  - 페널티 시스템: LiquidityGate, MarketRegime 필터
- **RouteState Calculator** - 진입 적기 판단 (ATTACK/ARMED/WAIT/OVERHEAT/NEUTRAL)
  - TTM Squeeze 해제 + 모멘텀 + RSI + Range 종합 판단
- **Market Regime Calculator** - 5단계 추세 분류 (STRONG_UPTREND → DOWNTREND)
- **Trigger System** - 진입 트리거 자동 감지
  - SqueezeBreak, BoxBreakout, VolumeSpike, GoldenCross 등
- **Signal System** - 백테스트/실거래 신호 저장 및 알림
  - `signal_marker` - 신호 마커 저장 (차트 표시용)
  - `signal_alert_rule` - 알림 규칙 관리 (JSONB 필터)
- **Reality Check System** - 추천 종목 실제 성과 검증
  - `price_snapshot` - 전일 추천 스냅샷 (TimescaleDB Hypertable)
  - `reality_check` - 익일 성과 자동 계산
  - 4개 분석 뷰 (일별 승률, 소스별, 랭크별, 최근 추이)
- **Advanced Indicators** - 추가 기술적 지표
  - Hull Moving Average (HMA)
  - On-Balance Volume (OBV)
  - SuperTrend
  - Candle Patterns (Hammer, ShootingStar, Engulfing 등)
  - Structural Analysis (Higher High/Low, Lower High/Low)

#### 📊 Agent Dashboard
- `.agents/dashboard/` - 실시간 에이전트 모니터링 웹 UI
  - Flask 기반 서버 (`server.py`)
  - 로그 파일 실시간 스트리밍
  - PowerShell/Bash 모니터링 스크립트

### Changed

#### 🚀 Strategy Enhancements
- **전체 26개 전략 업데이트**
  - 새로운 컨텍스트 통합
  - 개선된 포지션 사이징 로직
  - 글로벌 스코어 통합
  - 향상된 스크리닝 기능

#### 🔧 Core Infrastructure
- **analytics_provider.rs** - 확장된 분석 인터페이스
- **context.rs** - 글로벌 스코어가 포함된 풍부한 컨텍스트
- **alert.rs** - 새로운 알림 도메인 모델
- Symbol 타입 개선

#### 📡 Exchange & Data
- KIS 커넥터 개선 (한국/미국)
- 향상된 히스토리컬 데이터 캐싱
- 개선된 OHLCV 스토리지
- 펀더멘털 데이터 캐시 업데이트

#### 🔄 Migration Consolidation (33 → 11 files)
- 기능별 그룹화로 관리 복잡도 67% 감소
  - `01_foundation.sql` - 기본 스키마, ENUM 타입
  - `02_credentials_system.sql` - 거래소 자격증명
  - `03_application_config.sql` - 설정
  - `04_symbol_metadata.sql` - 심볼 정보, 펀더멘털
  - `05_market_data.sql` - OHLCV, 가격 뷰
  - `06_execution_tracking.sql` - 체결 캐시
  - `07_trading_journal.sql` - 매매일지
  - `08_portfolio_analytics.sql` - 포트폴리오 분석
  - `09_strategy_system.sql` - 전략, 신호, 알림 규칙
  - `10_reality_check.sql` - 추천 검증 시스템
  - `11_migration_tracking.sql` - 이력 추적 (34개 기록)
- `migrations/README.md` - 통합 가이드 추가
- 총 크기 43% 절감 (200KB → 114.5KB)

#### 📝 Documentation Cleanup
- 구현 완료된 문서 9개 제거 (~167KB)
  - `ttm_squeeze_implementation.md`
  - `reality_check_implementation_summary.md`
  - `sector_rs_implementation.md`, `sector_rs_test_guide.md`
  - `standalone_collector_design.md`
  - `phase_1b6_implementation_report.md`
  - `quant_trading_audit.md`
  - `strategy_logic_validation_report.md`
  - `tech_debt_verification_report.md`
- Phase 1.4.2 문서 보존 (Multiple KLine Period - 미구현)

### Previous Changes

- crates/trader-analytics/src/indicators/mod.rs
- crates/trader-analytics/src/indicators/momentum.rs
- crates/trader-analytics/src/indicators/trend.rs
- crates/trader-analytics/src/indicators/volatility.rs
- crates/trader-analytics/src/journal_integration.rs

프로젝트의 모든 주요 변경 사항을 기록합니다.

형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.0.0/)를 따르며,
[Semantic Versioning](https://semver.org/lang/ko/)을 준수합니다.

## [0.5.8] - 2026-02-03

### Added

#### 🚀 Standalone Data Collector (Major Feature)
- **새로운 `trader-collector` crate** - API 서버와 독립적으로 동작하는 데이터 수집 바이너리
  - CLI 인터페이스: `sync-symbols`, `collect-ohlcv`, `run-all`, `daemon`
  - 환경변수 기반 설정 (`config.rs` - 140줄)
  - 배치 처리 및 Rate Limiting
  - 전체 24,631개 STOCK/ETF 종목 수집 지원
- **데몬 모드** - 주기적 자동 수집
  - `DAEMON_INTERVAL_MINUTES` 설정 (기본: 60분)
  - Ctrl+C 우아한 종료 (`tokio::signal::ctrl_c()`)
  - 에러 발생 시 다음 주기 재시도
- **스케줄링 지원**
  - Cron 예제 (`scripts/collector.cron`)
  - systemd service/timer 파일
  - 최적화된 환경변수 템플릿 (`.env.collector.optimized`)
- **모니터링 및 통계**
  - `CollectionStats` - 성공/실패/스킵 통계
  - tracing 기반 구조화 로깅
  - 진행률 및 예상 시간 표시

#### 🔄 Yahoo Finance API 전환
- **KRX API 차단 대응** - `data.krx.co.kr` 403 Forbidden 해결
  - `CachedHistoricalDataProvider` 사용
  - KRX fallback to Yahoo Finance 자동 전환
  - 한국 주식 `.KS`/`.KQ` 접미사 지원
- **증분 수집 최적화**
  - 마지막 캔들 시간 이후 데이터만 조회
  - 갭 감지 및 경고
  - `cache_freshness` 기반 업데이트 판단
- **성능 개선**
  - 200ms 딜레이 기준 전체 수집 1.4시간
  - 증분 수집 시 95%+ 캐시 히트

#### 🏷️ Symbol Type 분류 시스템
- **마이그레이션 024** - `symbol_info.symbol_type` 컬럼 추가
  - `STOCK`, `ETF`, `ETN`, `WARRANT`, `REIT`, `PREFERRED` 분류
  - ETN 자동 필터링 (223개 종목)
  - 정규식 패턴 기반 분류 (`^[0-9]{4}[A-Z][0-9]$`)
- **수집 최적화**
  - `WHERE symbol_type IN ('STOCK', 'ETF')` 필터
  - 특수 증권 자동 제외 (ETN, 워런트, 옵션)
  - 403 에러 종목 자동 스킵

#### 📚 문서화
- **설계 문서**
  - `docs/standalone_collector_design.md` (700+ 줄)
  - `docs/collector_quick_start.md` (350+ 줄)
  - `docs/collector_env_example.env` (70+ 줄)
- **스크립트 예제**
  - `scripts/collector.cron` - Cron 스케줄
  - `scripts/trader-collector.service` - systemd service
  - `scripts/trader-collector.timer` - systemd timer

### Changed

#### 🔧 Collector 모듈 수정
- **OHLCV 수집** (`ohlcv_collect.rs`)
  - `KrxDataSource` → `CachedHistoricalDataProvider` 전환
  - LIMIT 제거 - 전체 종목 수집 가능
  - Yahoo Finance 우선 사용
  - 날짜 범위 파싱 로직 추가

#### ⚙️ 환경변수 최적화
- `OHLCV_REQUEST_DELAY_MS`: 500ms → 200ms (권장)
- `OHLCV_BATCH_SIZE`: 50 → 무제한 (LIMIT 제거)
- `DAEMON_INTERVAL_MINUTES`: 60 (신규)

### Removed

#### 🧹 API 서버 정리
- **trader-api**
  - `src/tasks/` 디렉토리 전체 제거 (5개 파일)
    - `fundamental.rs`, `symbol_sync.rs`
    - `krx_csv_sync.rs`, `eod_csv_sync.rs`
  - `src/routes/dataset.rs` - CSV 동기화 섹션 제거 (330줄)
  - `lib.rs` - tasks 모듈 re-export 제거
  - `main.rs` - Fundamental collector 시작 코드 제거 (25줄)
- **trader-cli**
  - `src/commands/sync_csv.rs` 제거
  - `Commands::SyncCsv` enum variant 제거
  - SyncCsv 핸들러 제거 (132줄)

### Fixed

- **KRX API 403 에러** - Yahoo Finance로 전환하여 해결
- **ETN 수집 실패** - symbol_type 필터링으로 해결
- **배치 제한** - LIMIT 제거하여 전체 종목 수집 가능

### Performance

- **수집 속도**: 3.4시간 → 1.4시간 (200ms 딜레이 기준)
- **증분 수집**: 첫 실행 후 95%+ 캐시 히트
- **API 안정성**: Yahoo Finance 99.9% 성공률

### Documentation

- Phase 0 TODO 업데이트 - Standalone Collector 완료 표시
- 새로운 환경변수 문서화
- Cron/systemd 배포 가이드

---

## [0.5.7] - 2026-02-02

### Added

#### 🎯 전략 스키마 시스템 (Major Feature)
- **Proc Macro 기반 메타데이터 추출** (`trader-strategy-macro`)
  - `#[strategy_metadata]` 매크로로 컴파일 타임 스키마 생성
  - 런타임 리플렉션 없이 타입 안전성 확보
  - 266줄의 proc macro 구현
- **SchemaRegistry** (`schema_registry.rs` - 694줄)
  - 전략별 파라미터 스키마 중앙 관리
  - JSON Schema 자동 생성
  - 프론트엔드 SDUI(Server-Driven UI) 지원
- **SchemaComposer** (`schema_composer.rs` - 279줄)
  - 공통 컴포넌트 조합으로 스키마 구성
  - 재사용 가능한 스키마 프래그먼트
- **API 엔드포인트** (`routes/schema.rs` - 189줄)
  - `GET /api/strategies/schema` - 전체 전략 스키마 조회
  - `GET /api/strategies/:name/schema` - 개별 전략 스키마
  - 26개 전략 모두 스키마 자동 등록

#### 🧩 공통 전략 컴포넌트 추출
- **indicators.rs** (349줄) - 기술 지표 계산
  - SMA, EMA, RSI, MACD, Bollinger Bands
  - ATR, Stochastic, ADX, CCI
  - 26개 전략에서 중복 제거
- **position_sizing.rs** (286줄) - 포지션 사이징
  - FixedAmount, FixedRatio, RiskBased
  - VolatilityAdjusted, KellyFraction
  - 일관된 포지션 계산 로직
- **risk_checks.rs** (291줄) - 리스크 관리
  - `check_max_position_size()` - 최대 포지션 검증
  - `check_concentration_limit()` - 집중도 한도
  - `check_loss_limit()` - 손실 한도
  - `check_volatility_limit()` - 변동성 필터
- **signal_filters.rs** (372줄) - 신호 필터링
  - 거래량, 변동성, 시간, 추세 필터
  - 중복 신호 제거 로직
  - 전략 간 일관성 확보

#### 📐 도메인 레이어 강화
- **calculations.rs** (374줄) - 금융 계산
  - `calculate_returns()` - 수익률 계산
  - `calculate_pnl()` - 손익 계산
  - `calculate_position_value()` - 포지션 가치
  - `calculate_commission()` - 수수료 계산
  - Decimal 타입으로 정밀 계산
- **statistics.rs** (514줄) - 통계 함수
  - 샤프 비율, 소르티노 비율, 최대 낙폭
  - 승률, Profit Factor, Calmar Ratio
  - 백테스트와 실거래 공통 사용
- **tick_size.rs** (335줄) - 틱 사이즈 관리
  - 시장별 최소 호가 단위 정의
  - `round_to_tick_size()` - 주문가 보정
  - KRX, 미국 주식, 선물/옵션 지원
- **schema.rs** (343줄) - 도메인 스키마
  - 공통 데이터 구조 정의
  - DTO와 도메인 모델 분리

#### 🛠️ CLI 도구 확장
- **fetch_symbols** (365줄)
  - 거래소별 심볼 목록 가져오기
  - `--exchange krx|binance|yahoo` 옵션
  - DB 직접 저장 지원
- **list_symbols** (244줄)
  - 심볼 목록 조회 및 필터링
  - `--market`, `--active`, `--format` 옵션
  - CSV/JSON 출력 지원
- **sync_csv** (120줄)
  - KRX CSV 파일 동기화
  - 증분 업데이트 지원

#### 📊 Analytics 확장
- **journal_integration.rs** (280줄)
  - 매매 일지와 백테스트 통합
  - 실거래 결과 자동 기록
  - 성과 비교 분석 지원

### Changed

#### 전략 리팩토링 (26개 전략)
- **공통 로직 제거**: 모든 전략에서 중복 코드 제거
- **모듈 임포트 통합**: `use super::common::*` 패턴 적용
- **스키마 어노테이션**: 모든 전략에 `#[strategy_metadata]` 추가
- **코드 감소**: 평균 전략당 ~50줄 감소

#### API 라우트 리팩토링
- **strategies.rs**: 163줄 감소
  - 스키마 로직을 `schema.rs`로 분리
  - 라우트 구조 단순화
- **dataset.rs**: 62줄 수정
  - 불필요한 import 제거
  - 타입 정리

#### Symbol 타입 확장
- **Yahoo 심볼 변환 로직** (`symbol.rs` - 107줄 추가)
  - `to_yahoo_symbol()` 메서드
  - KRX 심볼 자동 변환 (.KS/.KQ 접미사)
  - 캐싱 및 폴백 처리

#### 매칭 엔진 개선
- **틱 사이즈 적용** (`matching_engine.rs`)
  - 주문 가격을 시장별 틱 사이즈로 보정
  - 실거래와 동일한 체결 로직

### Documentation

- **tick_size_guide.md** (245줄)
  - 시장별 틱 사이즈 가이드
  - 코드 예시 및 주의사항
- **development_rules.md** (299줄 추가)
  - v1.1 업데이트: 180+ 규칙 체계화
  - 레거시 코드 제거 정책
  - 금융 계산 규칙 (Decimal 필수)
  - 에러 처리 규칙 (unwrap 금지)
- **prd.md** (67줄 추가)
  - 전략 스키마 시스템 명세
  - CLI 도구 문서화
- **CLAUDE.md** 업데이트
  - 버전 v0.5.7 반영
  - 핵심 규칙 요약 확장

### Technical Debt Removed

- **지표 계산 중복**: 26개 전략 → indicators 모듈로 통합
- **포지션 사이징 중복**: 개별 구현 → position_sizing 모듈로 통합
- **리스크 체크 산재**: 불일치하는 로직 → risk_checks 모듈로 표준화
- **스키마 수동 관리**: 하드코딩된 스키마 → Proc macro 자동 생성

---

## [0.5.5] - 2026-02-01

### Added

#### 🔄 API 재시도 시스템 (P0)
- **RetryConfig** (`trader-exchange/src/retry.rs`)
  - 지수 백오프 기반 재시도 로직
  - `with_retry()`, `with_retry_context()`, `with_retry_if()` 유틸리티
  - 에러별 대기 시간 자동 적용 (`retry_delay_ms()`)
  - 빠른/적극적/무재시도 프리셋 지원
- **KIS 클라이언트 통합** (`client_kr.rs`)
  - `execute_get_with_retry()`, `execute_post_with_retry()` 구현
  - 네트워크 오류, Rate Limit, 타임아웃 자동 재시도

#### 💰 비용 기준 및 FIFO 실현손익 (P1)
- **CostBasisTracker** (`repository/cost_basis.rs`)
  - 로트(Lot) 기반 FIFO 추적
  - 가중평균 매입가 자동 계산 (물타기 반영)
  - `sell()` 메서드로 FIFO 기반 실현손익 계산
  - 미실현 손익, 평균 보유 기간 계산
- **JournalRepository 확장**
  - `calculate_cost_basis()` - 종목별 비용 기준 조회
  - `calculate_all_cost_basis()` - 전체 종목 비용 기준
  - `get_cost_basis_tracker()` - 상세 분석용 추적기 반환

#### 📊 동적 슬리피지 모델 (P2)
- **SlippageModel** (`backtest/slippage.rs`)
  - **Fixed**: 고정 비율 슬리피지 (기본 0.05%)
  - **Linear**: 기본 슬리피지 + 거래량 기반 시장 충격
  - **VolatilityBased**: ATR/캔들 범위 기반 동적 계산
  - **Tiered**: 거래 금액 구간별 차등 슬리피지
- **BacktestConfig 확장**
  - `with_slippage_model()` 빌더 메서드
  - serde 기본값 함수 분리 (설정 파일화)

#### 🛡️ 서킷 브레이커 에러 카테고리 (P1)
- **ErrorCategory** (`circuit_breaker.rs`)
  - Network, RateLimit, Timeout, Service 분류
  - 카테고리별 독립적 실패 카운트
- **CategoryThresholds** 설정
  - 카테고리별 차등 임계치 (Rate Limit은 더 관대)
  - `conservative()`, `aggressive()` 프리셋
- **메트릭 확장**
  - `tripped_by` - 서킷 오픈 원인 카테고리
  - `category_failures` - 카테고리별 현재 실패 수

#### 🔗 포지션 동기화 (P1)
- **PositionSynchronizer** (`strategies/common/position_sync.rs`)
  - 전략 내부 포지션과 실제 포지션 동기화
  - `on_order_filled()`, `on_position_update()` 콜백 연동
- **볼린저 전략 통합**
  - 체결/포지션 이벤트 시 내부 상태 동기화

### Changed

#### 보안 수정 (P0)
- **SQL Injection 수정** (`repository/screening.rs`)
  - `screen_momentum()` 동적 쿼리를 파라미터화된 쿼리로 변경
  - `$3::text IS NULL OR si.market = $3` 패턴 적용

#### 백테스트 설정 개선 (P2)
- **BacktestConfig 기본값 함수화** (`backtest/engine.rs`)
  - `default_initial_capital()`, `default_commission_rate()` 등 분리
  - serde default 어트리뷰트로 JSON/YAML 설정 파일 지원

#### KIS 클라이언트 개선
- **토큰 갱신 지원**: 매 재시도마다 헤더 새로 빌드
- **에러 코드 세분화**: HTTP 429 → RateLimited, 401 → Unauthorized

#### 종목명 업데이트 로직 개선
- CSV에서 한글 이름이 설정된 경우 Yahoo Finance 영문 이름으로 덮어쓰지 않음

### Documentation

- `docs/infrastructure.md` - Podman 컨테이너 인프라 가이드
- `docs/agent_guidelines.md` - AI 에이전트 가이드라인 (Context7 사용법)
- `docs/system_usage.md` - 모니터링, CSV 동기화 시스템 사용법
- `CLAUDE.md` - 세션 컨텍스트 문서 간소화 (상세 내용은 별도 문서로 분리)

---

## [0.5.4] - 2026-02-01

### Added

#### ⚡ 스크리닝 쿼리 성능 최적화
- **Materialized View** (`mv_latest_prices`)
  - 심볼별 최신 일봉 가격을 미리 계산하여 저장
  - 스크리닝 쿼리 성능 1.5초+ → 수십ms로 개선
  - `refresh_latest_prices()` 함수로 갱신 지원

#### 🛡️ 심볼 데이터 수집 실패 추적
- **자동 비활성화 시스템** (`symbol_info` 컬럼 추가)
  - `fetch_fail_count`: 연속 실패 횟수 기록
  - `last_fetch_error`: 마지막 에러 메시지
  - `last_fetch_attempt`: 마지막 시도 시간
  - 3회 이상 연속 실패 시 자동 비활성화

- **DB 함수**
  - `record_symbol_fetch_failure()`: 실패 기록 및 자동 비활성화
  - `reset_symbol_fetch_failure()`: 성공 시 카운트 초기화

- **실패 심볼 관리 뷰**
  - `v_symbol_fetch_failures`: 실패 심볼 현황 (레벨별 분류)

#### 🔧 심볼 상태 관리 API
- `GET /api/v1/dataset/symbols/failed` - 실패한 심볼 목록 조회
- `GET /api/v1/dataset/symbols/stats` - 심볼 통계 (활성/비활성/실패)
- `POST /api/v1/dataset/symbols/reactivate` - 비활성화된 심볼 재활성화

### Changed

#### 심볼 캐시 관리 개선
- `AppState.clear_symbol_cache()`: CSV 동기화 후 캐시 자동 클리어
- `AppState.symbol_cache_size()`: 캐시 크기 조회
- 동기화 시 최신 DB 데이터가 즉시 반영되도록 개선

### Database

- `migrations/022_latest_prices_materialized_view.sql` - 최신 가격 Materialized View
- `migrations/023_symbol_fetch_failure_tracking.sql` - 심볼 수집 실패 추적

---

## [0.5.3] - 2026-02-01

### Added

#### 🔍 모니터링 및 에러 추적 시스템
- **ErrorTracker** (`monitoring/error_tracker.rs`)
  - AI 디버깅을 위한 구조화된 에러 로그 수집
  - 에러 심각도별 분류 (Warning, Error, Critical)
  - 에러 카테고리별 분류 (Database, ExternalApi, DataConversion, Authentication, Network, BusinessLogic, System)
  - 메모리 기반 에러 히스토리 보관 (최대 1000개)
  - 에러 발생 위치, 컨텍스트, 스택 트레이스 자동 수집
  - Critical 에러 발생 시 Telegram 알림 지원

- **모니터링 API** (`routes/monitoring.rs`)
  - `GET /api/v1/monitoring/errors` - 에러 목록 조회 (심각도/카테고리 필터)
  - `GET /api/v1/monitoring/errors/critical` - Critical 에러 조회
  - `GET /api/v1/monitoring/errors/:id` - 특정 에러 상세
  - `GET /api/v1/monitoring/stats` - 에러 통계 (심각도별/카테고리별 집계)
  - `GET /api/v1/monitoring/summary` - 시스템 모니터링 요약
  - `POST /api/v1/monitoring/stats/reset` - 통계 초기화
  - `DELETE /api/v1/monitoring/errors` - 에러 히스토리 삭제

#### 📊 CSV 기반 심볼 동기화
- **KRX CSV 동기화** (`tasks/krx_csv_sync.rs`)
  - `data/krx_codes.csv`에서 종목 코드 동기화
  - `data/krx_sector_map.csv`에서 업종 정보 업데이트
  - KOSPI/KOSDAQ 자동 판별 (0으로 시작: KOSPI, 1~4로 시작: KOSDAQ)
  - Yahoo Finance 심볼 자동 생성 (.KS/.KQ 접미사)

- **EODData CSV 동기화** (`tasks/eod_csv_sync.rs`)
  - NYSE, NASDAQ, AMEX, LSE, TSX, ASX, HKEX, SGX 등 해외 거래소 지원
  - 거래소별 Market 코드 자동 매핑 (US, GB, CA, AU, HK, SG 등)
  - 배치 upsert로 대량 심볼 동기화

- **데이터 파일**
  - `data/krx_codes.csv` - KRX 종목 코드 (KOSPI/KOSDAQ)
  - `data/krx_sector_map.csv` - KRX 업종 매핑

#### 🛠️ Python 스크래퍼
- `scripts/scrape_eoddata_symbols.py` - EODData 심볼 스크래핑 도구
- `scripts/requirements-scraper.txt` - 스크래퍼 의존성

#### 📄 문서
- `docs/fulltest_workflow.md` - 전체 테스트 워크플로우 가이드

### Changed

#### Fundamental 캐시 개선
- `cache/fundamental.rs`: 데이터 변환 로직 개선

### Database

- `migrations/021_fix_fundamental_decimal_precision.sql`
  - Decimal 정밀도 확장: `DECIMAL(8,4)` → `DECIMAL(12,4)`
  - 극단적 성장률 지원 (스타트업/바이오텍: 21,000%+ 성장률)
  - 영향 컬럼: ROE, ROA, 영업이익률, 순이익률, 매출/이익 성장률, 배당 관련

---

## [0.5.2] - 2026-01-31

### Added

#### 🔄 백그라운드 데이터 수집 시스템
- **FundamentalCollector** (`tasks/fundamental.rs`)
  - Yahoo Finance에서 펀더멘털 데이터 자동 수집
  - 설정 가능한 수집 주기 및 배치 처리
  - Rate limiting 기반 API 요청 관리
  - OHLCV 캔들 데이터 증분 업데이트 지원
- **SymbolSyncTask** (`tasks/symbol_sync.rs`)
  - KRX (KOSPI/KOSDAQ) 종목 자동 동기화
  - Binance USDT 거래 페어 동기화
  - Yahoo Finance 주요 지수 종목 동기화
  - 최소 심볼 수 기반 자동 실행 조건

#### 📊 프론트엔드 스크리닝 페이지
- **Screening.tsx** - 종목 스크리닝 UI 구현
  - 프리셋 스크리닝 (가치주, 고배당, 성장주 등)
  - 커스텀 필터 조합
  - 결과 테이블 및 정렬

#### 🛠️ 환경 변수 확장 (.env.example)
- `FUNDAMENTAL_COLLECT_*`: 펀더멘털 수집 설정 (활성화, 주기, 배치 크기)
- `SYMBOL_SYNC_*`: 심볼 동기화 설정 (KRX, Binance, Yahoo)

### Changed

#### 브랜딩 통일
- Web UI 타이틀을 "ZeroQuant │ 퀀트 트레이딩 플랫폼"으로 통일
- 사이드바 로고 텍스트 "Zero Quant" → "ZeroQuant"로 변경

#### 데이터 캐시 확장
- **FundamentalCache** (`cache/fundamental.rs`) - 펀더멘털 데이터 캐싱
- **SymbolInfoProvider** 확장 - 심볼 정보 조회 기능 강화

---

## [0.5.1] - 2026-01-31

### Added

#### 🔍 종목 스크리닝 (Symbol Screening) - 백엔드 API
- **ScreeningRepository** (`repository/screening.rs`, 592줄)
  - Fundamental + OHLCV 기반 종목 필터링
  - 다양한 조건 조합 지원 (시가총액, PER, PBR, ROE, 배당수익률 등)
- **스크리닝 API** (`routes/screening.rs`, 574줄)
  - `POST /api/v1/screening` - 커스텀 스크리닝 실행
  - `GET /api/v1/screening/presets` - 프리셋 목록 조회
  - `GET /api/v1/screening/presets/{preset}` - 프리셋 스크리닝 실행
  - `GET /api/v1/screening/momentum` - 모멘텀 기반 스크리닝
- **사전 정의 프리셋 6종**
  - `value`: 저PER + 저PBR 가치주
  - `dividend`: 고배당주 (배당수익률 3%+)
  - `growth`: 고ROE 성장주 (ROE 15%+)
  - `snowball`: 스노우볼 전략 (저PBR + 고배당)
  - `large_cap`: 대형주 (시가총액 상위)
  - `near_52w_low`: 52주 신저가 근접 종목

#### Symbol Fundamental 확장
- **SymbolFundamentalRepository** (`repository/symbol_fundamental.rs`, 459줄)
  - 종목 기본정보 CRUD
  - 섹터별/시장별 조회
- **SymbolInfoRepository 확장** (439줄 추가)
  - 시장 정보, 섹터 정보 조회
  - 종목 검색 기능 강화

### Changed

#### 전략 개선
- `momentum_surge.rs`: 조건 로직 개선
- `market_bothside.rs`: 양방향 매매 조건 정밀화
- `sector_vb.rs`: 섹터별 변동성 돌파 조건 개선
- `us_3x_leverage.rs`: 레버리지 조건 최적화

#### 백테스트/분석 개선
- `analytics/charts.rs`: 차트 데이터 생성 개선
- `analytics/performance.rs`: 성과 지표 계산 확장
- `backtest/loader.rs`, `backtest/mod.rs`: 데이터 로딩 최적화

#### 프론트엔드 개선
- `Backtest.tsx`: 백테스트 UI 개선
- `PortfolioEquityChart.tsx`: 차트 렌더링 최적화
- `Dashboard.tsx`: 대시보드 개선

#### 코드 품질
- `.rustfmt.toml`: Rust 코드 포맷팅 규칙 추가
  - `max_width = 100`
  - `use_small_heuristics = "Max"`
  - `imports_granularity = "Crate"`

---

## [0.5.0] - 2026-01-31

### Added

#### 📒 매매일지 (Trading Journal) - 신규 기능
- **체결 내역 관리** (`routes/journal.rs`, `repository/journal.rs`)
  - 거래소 API에서 체결 내역 자동 동기화
  - 기간별 조회 (일별/주별/월별/전체)
  - 종목별/전략별 필터링
- **손익 분석 (PnL Analysis)**
  - 실현/미실현 손익 계산
  - 누적 손익 차트 (`PnLBarChart.tsx`)
  - 종목별 손익 분석 (`SymbolPnLTable.tsx`)
- **포지션 추적**
  - 보유 현황 대시보드 (`PositionsTable.tsx`)
  - 물타기 자동 계산 (평균 매입가 갱신)
  - 포지션 이력 조회
- **전략 인사이트** (`StrategyInsightsPanel.tsx`)
  - 전략별 성과 분석
  - 매매 패턴 분석 (빈도, 성공률, 평균 보유 기간)
- **DB 마이그레이션 6개 추가**
  - `015_trading_journal.sql`: 매매일지 기본 테이블
  - `016_positions_credential_id.sql`: 포지션-계정 연결
  - `017_journal_views.sql`: 분석용 뷰
  - `018_journal_period_views.sql`: 기간별 분석 뷰
  - `019_fix_cumulative_pnl_types.sql`: 타입 수정
  - `020_symbol_fundamental.sql`: 종목 기본정보

#### Repository 패턴 확장
- **JournalRepository** (`repository/journal.rs`, 993줄)
  - 체결 내역 CRUD
  - 손익 집계 쿼리
  - 기간별 통계 조회
- **KlinesRepository** (`repository/klines.rs`, 481줄)
  - OHLCV 데이터 접근 계층
  - 시계열 쿼리 최적화

#### 프론트엔드 컴포넌트
- **TradingJournal.tsx** (344줄): 매매일지 메인 페이지
- **SymbolDisplay.tsx** (203줄): 종목 표시 컴포넌트
- **PnLBarChart.tsx** (167줄): 손익 막대 차트
- **ExecutionsTable.tsx** (208줄): 체결 내역 테이블
- **PnLAnalysisPanel.tsx** (216줄): 손익 분석 패널
- **StrategyInsightsPanel.tsx** (242줄): 전략 인사이트 패널

#### 문서화
- **development_rules.md** (561줄): 개발 규칙 문서 신규 작성
  - Context7 API 검증 절차
  - unwrap() 안전 패턴
  - Repository 패턴 가이드
  - 전략 추가 체크리스트
- **prd.md**: PRD 문서 위치 이동 및 업데이트
- **docs/*.md**: 운영/배포/모니터링 문서 현행화

### Changed

#### 전략 개선
- **bollinger.rs**: 밴드 계산 로직 개선
- **grid.rs**: 그리드 간격 계산 최적화
- **rsi.rs**: RSI 신호 생성 로직 개선
- **volatility_breakout.rs**: 돌파 조건 정밀화

#### 백엔드 개선
- `routes/portfolio.rs`: 포트폴리오 조회 API 확장
- `repository/positions.rs`: 포지션 Repository 확장 (239줄 추가)
- `repository/orders.rs`: 주문 Repository 개선
- `main.rs`: Journal 라우트 등록

#### 프론트엔드 개선
- `App.tsx`: Trading Journal 라우트 추가
- `Layout.tsx`: 매매일지 메뉴 추가
- `client.ts`: Journal API 클라이언트 추가 (357줄 추가)
- `format.ts`: 포맷팅 유틸리티 확장 (80줄 추가)

#### KIS 거래소 연동
- `kis/auth.rs`: 인증 로직 개선 (40줄 변경)

### Database

- 마이그레이션 14개 → 20개 (6개 추가)
- 매매일지 관련 테이블 및 뷰 추가

---

## [0.4.4] - 2026-01-31

### Added

#### OpenAPI/Swagger 문서화
- **utoipa 통합**: REST API 자동 문서화
  - `openapi.rs`: OpenAPI 3.0 스펙 중앙 집계
  - Swagger UI (`/swagger-ui`) 경로에서 인터랙티브 문서 제공
  - 모든 주요 엔드포인트 태그 분류 (strategies, backtest, portfolio 등)
- **응답/요청 스키마**: ToSchema derive로 타입 자동 문서화
  - `HealthResponse`, `ComponentHealth`, `StrategiesListResponse` 등
  - 에러 응답 스키마 표준화 (`ApiError`)

#### 타입 안전성 강화
- **StrategyType enum** (`types/strategy_type.rs`): 전략 타입 열거형 추가
  - 26개 전략 타입 정의 (rsi_mean_reversion, grid, bollinger_bands 등)
  - serde 직렬화/역직렬화 지원
  - OpenAPI 스키마 자동 생성

#### 백테스트 API 개선
- **OpenAPI 어노테이션**: 백테스트 엔드포인트 문서화
  - `run_backtest`, `get_backtest_strategies` 등 핸들러
  - 요청/응답 타입 스키마 정의

### Changed

#### API 구조 개선
- `routes/mod.rs`: OpenAPI 스키마 타입 re-export
- `routes/health.rs`: 헬스 체크 OpenAPI 어노테이션 추가
- `routes/strategies.rs`: 전략 목록 API 문서화
- `routes/credentials/types.rs`: 자격증명 타입 OpenAPI 스키마

#### 거래소 커넥터
- `binance.rs`: 타임아웃 설정 개선
- `kis/config.rs`: 설정 타입 강화

### Dependencies

#### 신규 추가
- `utoipa = "5.3"`: OpenAPI 스키마 생성
- `utoipa-swagger-ui = "9.0"`: Swagger UI 서빙
- `utoipa-axum = "0.2"`: Axum 라우터 통합

---

## [0.4.3] - 2026-01-31

### Added

#### 통합 에러 핸들링 시스템
- **ApiErrorResponse** (`error.rs`): 모든 API 엔드포인트의 에러 응답 통합
  - 일관된 에러 코드, 메시지, 타임스탬프 제공
  - 기존 분산된 에러 타입들 통합 (strategies, backtest, simulation, ml)
  - 에러 상세 정보 및 요청 컨텍스트 포함

#### Repository 패턴 확장
- **신규 Repository 모듈 5개 추가**:
  - `repository/portfolio.rs`: 포트폴리오 데이터 접근
  - `repository/orders.rs`: 주문 이력 관리
  - `repository/positions.rs`: 포지션 데이터 관리
  - `repository/equity_history.rs`: 자산 이력 조회
  - `repository/backtest_results.rs`: 백테스트 결과 저장/조회

#### 프론트엔드 컴포넌트 분리
- **AddStrategyModal.tsx**: 전략 추가 모달 분리
- **EditStrategyModal.tsx**: 전략 편집 모달 분리
- **SymbolPanel.tsx**: 심볼 패널 컴포넌트
- **format.ts**: 포맷팅 유틸리티
- **indicators.ts**: 기술적 지표 계산 유틸리티

### Changed

#### 대형 파일 모듈화
- **analytics.rs (2,678줄) → 7개 모듈로 분리**:
  ```
  routes/analytics/
  ├── mod.rs        (라우터)
  ├── charts.rs     (차트 데이터)
  ├── indicators.rs (지표 계산)
  ├── manager.rs    (매니저)
  ├── performance.rs(성과 분석)
  ├── sync.rs       (동기화)
  └── types.rs      (타입 정의)
  ```

- **credentials.rs (1,615줄) → 5개 모듈로 분리**:
  ```
  routes/credentials/
  ├── mod.rs           (라우터)
  ├── active_account.rs(활성 계정)
  ├── exchange.rs      (거래소 자격증명)
  ├── telegram.rs      (텔레그램 설정)
  └── types.rs         (타입 정의)
  ```

- **Dataset.tsx, Strategies.tsx**: 컴포넌트 분리로 1,400+ 줄 감소

#### 모듈 재배치
- **trailing_stop.rs**: `trader-strategy` → `trader-risk` 크레이트로 이동
  - 리스크 관리 로직의 올바른 위치 배치

#### 인프라 개선
- **Docker → Podman 마이그레이션 지원**
  - README.md: Podman 설치 및 사용법 추가
  - docker-compose.yml: Podman 호환 주석 추가
  - 명령어 매핑 테이블 제공

### Improved

#### 코드 품질
- 에러 처리 일관성 향상 (unwrap() 사용 감소)
- 모듈별 관심사 분리로 유지보수성 향상
- Repository 패턴으로 데이터 접근 계층 표준화

---

## [0.4.2] - 2026-01-31

### Fixed

#### 다중 자산 전략 심볼 비교 버그 수정
- **심볼 비교 로직 통일**: `data.symbol.clone()` → `data.symbol.to_string()`
  - 영향 받은 전략 (10개):
    - `all_weather.rs`: All Weather 포트폴리오
    - `baa.rs`: Bold Asset Allocation
    - `dual_momentum.rs`: Dual Momentum
    - `momentum_surge.rs`: 모멘텀 급등
    - `market_bothside.rs`: 시장 양방향
    - `market_cap_top.rs`: 시가총액 상위
    - `sector_momentum.rs`: 섹터 모멘텀
    - `sector_vb.rs`: 섹터 변동성 돌파
    - `momentum_power.rs`: Momentum Power 전략
    - `us_3x_leverage.rs`: 미국 3배 레버리지

#### 백테스트 가격 매칭 버그 수정
- **다중 자산 가격 데이터 매칭**: `engine.rs`에서 현재 심볼에 맞는 가격 데이터만 필터링
- 이전: 모든 심볼 데이터에서 첫 번째 데이터 사용 (잘못된 가격)
- 이후: 심볼별 정확한 가격 데이터 매칭

### Added

#### 전략 통합 테스트
- **strategy_integration.rs**: 28개 전략 통합 테스트 (1,753줄)
  - 모든 백테스트 대상 전략 자동 검증
  - 다중 심볼 전략 테스트 커버리지 추가
  - 실행 시간: ~15분 (병렬 실행)

#### 차트 컴포넌트
- **SyncedChartPanel.tsx**: 동기화된 차트 패널 개선
  - 다중 심볼 동시 표시 지원
  - 줌/팬 동기화 기능

### Changed

#### 프론트엔드
- `Backtest.tsx`: 다중 자산 전략 결과 표시 개선
- `Simulation.tsx`: 전략 선택 UI/UX 개선
- `Strategies.tsx`: 전략 목록 필터링 및 정렬 개선
- `client.ts`: API 클라이언트 타입 안전성 강화

#### 백엔드
- `backtest/engine.rs`: 다중 자산 백테스트 엔진 로직 개선
- `backtest/loader.rs`: 데이터 로딩 최적화
- `strategies.rs`: 전략 Repository 쿼리 개선
- `simulation.rs`: 시뮬레이션 라우트 리팩토링

---

## [0.4.1] - 2026-01-31

### Added

#### SDUI (Server-Driven UI) 전략 스키마
- **전략 UI 스키마** (`config/sdui/strategy_schemas.json`)
  - 27개 전략별 동적 폼 스키마 정의
  - 필드 타입, 검증 규칙, 기본값 포함
  - 프론트엔드에서 서버 스키마 기반 동적 폼 렌더링

#### 유틸리티 모듈 (`utils/`)
- `format.rs`: 숫자, 날짜, 통화 포맷팅 함수
- `response.rs`: API 응답 헬퍼 (성공/에러 응답 표준화)
- `serde_helpers.rs`: Serde 직렬화 헬퍼 함수

#### 전략 기본값
- **defaults.rs**: 전략별 기본 파라미터 정의
- 신규 전략 생성 시 합리적인 기본값 제공

#### 심볼 검색 컴포넌트
- **SymbolSearch.tsx**: 실시간 심볼 검색 UI
- 자동완성, 최근 검색 기록, 시장 필터

#### E2E 테스트
- **risk-management-ui.spec.ts**: 리스크 관리 UI Playwright 테스트
- **playwright.config.ts**: E2E 테스트 설정
- **regression_baseline.json**: 회귀 테스트 베이스라인

#### DB 마이그레이션
- `014_strategy_risk_capital.sql`: 전략 리스크/자본 설정 컬럼 추가

### Changed

#### 백테스트 모듈 리팩토링
- **모듈 분리**: `backtest.rs` (3,854줄) → 5개 모듈로 분리
  - `backtest/mod.rs`: 라우터 및 핸들러
  - `backtest/engine.rs`: 백테스트 실행 엔진
  - `backtest/loader.rs`: 데이터 로더
  - `backtest/types.rs`: 타입 정의
  - `backtest/ui_schema.rs`: UI 스키마 생성
- 코드 가독성 및 유지보수성 향상

#### 프론트엔드 개선
- `Backtest.tsx`: SDUI 스키마 기반 동적 폼 통합
- `Simulation.tsx`: 심볼 검색 컴포넌트 통합
- `Strategies.tsx`: 전략 생성/편집 UI 개선
- `DynamicForm.tsx`: 스키마 기반 폼 렌더링 개선

#### API 개선
- `strategies.rs`: 전략 CRUD API 확장 (리스크/자본 설정)
- `equity_history.rs`: N+1 쿼리 최적화 (배치 쿼리)

---

## [0.4.0] - 2026-01-31

### Added

#### ML 훈련 파이프라인
- **Python ML 훈련 스크립트** (`scripts/train_ml_model.py`)
  - XGBoost, LightGBM, RandomForest 모델 지원
  - DB에서 OHLCV 데이터 자동 로드
  - 기술적 지표 기반 피처 엔지니어링 (30+ 피처)
  - ONNX 포맷으로 모델 내보내기
- **ML 모듈 구조** (`scripts/ml/`)
  - `data_fetcher.py`: TimescaleDB에서 데이터 가져오기
  - `feature_engineering.py`: RSI, MACD, Bollinger, ATR 등 피처 생성
  - `model_trainer.py`: 하이퍼파라미터 튜닝, 교차 검증
- **ML Docker 이미지** (`Dockerfile.ml`)
  - Python 3.11 + 과학 계산 라이브러리
  - `docker-compose --profile ml` 로 실행
- **Python 프로젝트 설정** (`pyproject.toml`)
  - uv 패키지 매니저 지원
  - 의존성: pandas, scikit-learn, xgboost, lightgbm, onnx

#### ML API 확장
- **ML 서비스 레이어** (`ml/service.rs`): 예측 로직 분리
- **ML API 엔드포인트** (`routes/ml.rs`): 모델 목록, 예측 API 확장
- **예측기 개선** (`predictor.rs`): 다중 모델 지원

#### Execution Cache
- **실행 캐시 Repository** (`execution_cache.rs`): 전략 실행 상태 캐싱

### Changed
- `Dataset.tsx`: 데이터셋 페이지 UI/UX 개선
- `MultiPanelGrid.tsx`: 차트 패널 레이아웃 개선
- `patterns.rs`: 패턴 인식 API 개선
- `state.rs`: AppState ML 서비스 통합

---

## [0.3.0] - 2026-01-30

### Added

#### 10개 신규 전략 추가 (총 27개)
- **BAA** (Bold Asset Allocation): 카나리아 자산 기반 공격/수비 모드 전환
- **Dual Momentum**: 절대/상대 모멘텀 기반 자산 배분 (Gary Antonacci)
- **Momentum Surge**: 모멘텀 급등 전략
- **Market BothSide**: 양방향 매매
- **Pension Bot** (연금봇): 연금 계좌 자동 운용 (MDD 최소화)
- **Sector Momentum**: 섹터 ETF 로테이션 전략
- **Sector VB**: 섹터별 변동성 돌파
- **Small Cap Quant**: 소형주 퀀트 팩터 전략
- **Range Trading**: 구간별 분할 매매
- **US 3X Leverage**: 미국 3배 레버리지 ETF 전략 (TQQQ/SOXL)

#### Symbol Info Provider
- **종목 정보 캐싱** (`symbol_info.rs`): KIS API 종목 정보 조회/캐싱
- 종목명, 시장 구분, 가격 정보, 거래 단위 등 메타데이터 관리
- DB 마이그레이션: `012_symbol_info.sql`

#### Docker 빌드 최적화
- **sccache**: Rust 증분 빌드 캐시 (재빌드 시 50-80% 시간 단축)
- **mold 링커**: lld보다 2-3배 빠른 링킹
- Crate 수정 빈도별 빌드 순서 최적화
- 개발 스크립트 추가: `scripts/dev-build.ps1`, `scripts/docker-build.ps1`

#### 아키텍처 문서
- **architecture.md**: 시스템 아키텍처 상세 문서화
- Crate 간 의존성, 데이터 흐름, 배포 구조 설명

#### 테스트 자동화
- **전략 테스트 스크립트** (`scripts/test_all_strategies.py`)
- 모든 전략 백테스트 자동 검증

### Changed

#### API 개선
- `analytics.rs`: 성과 분석 API 확장 (기간별 통계, 상세 메트릭)
- `backtest.rs`: 결과 저장/조회 API 개선
- `dataset.rs`: 다중 심볼 지원, 배치 다운로드
- `equity_history.rs`: 자산 이력 조회 API 추가

#### 프론트엔드
- `Dataset.tsx`: 다중 심볼 관리, 배치 작업 UI
- `MultiPanelGrid.tsx`: 차트 패널 레이아웃 개선
- `PortfolioEquityChart.tsx`: 성과 차트 시각화 개선
- `Strategies.tsx`: 신규 전략 지원

#### 데이터 레이어
- `historical.rs`: 캐시 효율성 개선
- `ohlcv.rs`: 저장소 최적화

### Database Migrations
- `011_execution_cache.sql`: 실행 캐시 테이블
- `012_symbol_info.sql`: 종목 정보 테이블
- `013_strategy_timeframe.sql`: 전략 타임프레임 설정

---

## [0.2.0] - 2026-01-30

### Added

#### 데이터셋 관리 시스템
- **데이터셋 페이지** (`Dataset.tsx`): OHLCV 데이터 조회/다운로드/관리 UI
  - Yahoo Finance에서 심볼 데이터 다운로드
  - 캔들 수 또는 날짜 범위 지정 다운로드
  - 무한 스크롤링 테이블 (Intersection Observer API)
  - 실시간 차트 시각화 (멀티 타임프레임 지원)
- **데이터셋 API** (`dataset.rs`): OHLCV 데이터 CRUD 엔드포인트
- **OHLCV 저장소 리팩토링**: `yahoo_cache.rs` → `ohlcv.rs`로 이름 변경

#### 백테스트 결과 저장
- **백테스트 결과 API** (`backtest_results.rs`): 백테스트 결과 저장/조회
- **DB 마이그레이션**: `010_backtest_results.sql` - 결과 테이블 추가
- 과거 백테스트 결과 조회 및 비교 기능

#### 전략 워크플로우 개선
- **등록된 전략 기반 백테스트**: 전략 페이지에서 먼저 등록 → 백테스트/시뮬레이션에서 선택
- **전략 Repository 패턴** (`repository/strategies.rs`): 데이터 접근 계층 분리
- **전략 자동 로드**: 서버 시작 시 DB에서 저장된 전략 자동 로드
- **strategy_type 필드 추가**: 전략 타입 구분 (`volatility_breakout`, `grid` 등)
- **symbols 필드 추가**: 전략별 대상 심볼 목록 저장

#### 차트 시스템 개선
- **동기화된 차트 패널** (`SyncedChartPanel.tsx`): 다중 차트 동기화 지원
- **멀티 패널 그리드** (`MultiPanelGrid.tsx`): 차트 패널 레이아웃 관리
- **PriceChart 개선**: 1시간 타임프레임 Unix timestamp 변환 수정

### Changed

#### 프론트엔드
- `Backtest.tsx`: 등록된 전략 선택 방식으로 전환, 파라미터 입력 폼 제거
- `Simulation.tsx`: 동일한 전략 선택 방식 적용
- `Strategies.tsx`: strategy_type, symbols 필드 지원
- `App.tsx`: Dataset 페이지 라우트 추가
- `Layout.tsx`: 데이터셋 메뉴 추가

#### 백엔드
- `backtest.rs`: 등록된 전략 ID 기반 실행 지원
- `historical.rs`: 지표 계산에 isDailyOrHigher 파라미터 추가
- `volatility_breakout.rs`: is_new_period 날짜 비교 로직 개선

### Removed
- `docs/prd.md`: 불필요한 대용량 PRD 문서 제거 (38,000+ 토큰)
- docker-compose.yml에서 불필요한 설정 제거

### Database Migrations
- `008_strategies_type_and_symbols.sql`: 전략 타입/심볼 컬럼 추가
- `009_rename_candle_cache.sql`: 테이블명 리네이밍
- `010_backtest_results.sql`: 백테스트 결과 테이블

---

## [0.1.0] - 2026-01-30

### Added

#### 핵심 시스템
- Rust 기반 모듈형 아키텍처 구축 (10개 crate)
- 비동기 런타임 (Tokio) 기반 고성능 처리
- PostgreSQL (TimescaleDB) + Redis 데이터 저장소

#### 거래소 연동
- **Binance**: 현물 거래, WebSocket 실시간 시세
- **한국투자증권 (KIS)**:
  - OAuth 2.0 인증 (자동 토큰 갱신)
  - 국내/해외 주식 주문 (매수/매도/정정/취소)
  - WebSocket 실시간 연동 (국내/해외)
  - 모의투자 계좌 지원
  - 휴장일 관리 시스템
- Yahoo Finance 데이터 연동

#### 전략 시스템 (17개 전략)
- **실시간 전략**: Grid Trading, RSI, Bollinger Bands, Magic Split, Infinity Bot, Trailing Stop
- **일간 전략**: Volatility Breakout, SMA Crossover, Momentum Power, Stock Rotation, Market Interest Day, Candle Pattern
- **월간 자산배분**: All Weather, HAA, XAA, Compound Momentum, Market Cap Top
- 플러그인 기반 동적 전략 로딩
- Strategy trait 기반 확장 가능한 구조

#### 백테스트 시스템
- 단일 자산 전략 백테스트 (6종 검증 완료)
- 시뮬레이션 거래소 (매칭 엔진)
- 성과 지표 계산 (Sharpe Ratio, MDD, Win Rate 등)

#### ML/AI 기능
- 패턴 인식 엔진 (47가지: 캔들스틱 25개 + 차트 22개)
- 피처 엔지니어링 (25-30개 기술 지표)
- ONNX Runtime 추론 시스템
- Python 훈련 파이프라인 (XGBoost, LightGBM, RandomForest)

#### 리스크 관리
- 자동 스톱로스/테이크프로핏
- 포지션 크기 제한
- 일일 손실 한도
- ATR 기반 변동성 필터
- Circuit Breaker 패턴

#### Web API & 대시보드
- Axum 기반 REST API
- WebSocket 실시간 통신
- SolidJS + TypeScript 프론트엔드
- 실시간 포트폴리오 모니터링
- 전략 관리 UI (시작/중지/설정)
- 백테스트 실행 및 결과 시각화
- 설정 화면 (API 키, 텔레그램, 리스크)
- 포트폴리오 분석 차트 (Equity Curve, Drawdown)

#### 알림 시스템
- Telegram 알림 연동
- 체결/신호/리스크 경고 알림

#### 인프라
- Docker / Docker Compose 지원
- Prometheus / Grafana 모니터링 설정
- 데이터베이스 마이그레이션 시스템

### Security
- API 키 AES-256-GCM 암호화 저장
- JWT 기반 인증
- CORS 설정

---

## 로드맵

### [0.8.0] - 예정
- 추가 거래소 통합 (Coinbase, 키움증권)
- WebSocket 이벤트 브로드캐스트 완성
- 성능 최적화 및 부하 테스트

### [0.9.0] - 예정
- 실시간 알림 대시보드
- 포트폴리오 리밸런싱 자동화
- 다중 계좌 지원
