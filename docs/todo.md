# ZeroQuant TODO - 통합 로드맵

> **마지막 업데이트**: 2026-02-08
> **현재 버전**: v0.8.3

---

## 남은 작업

### Mock 거래소 KIS 수준 업그레이드 (v0.9.0)

#### Phase 1: 현실적 가격 스트리밍 (`mock_streaming.rs` 신규)
- [ ] MockPriceMode enum (HistoricalReplay / RandomWalk / YahooLegacy)
- [ ] MockStreamingConfig 설정 구조체
- [ ] HistoricalReplayGenerator (DB 1분봉 → 틱 보간 재생)
- [ ] RandomWalkGenerator (ATR 기반 정규분포 + 평균 회귀 + 호가 단위 라운딩)
- [ ] MockOrderBookGenerator (KR 10단계 호가, US 1단계 호가)

#### Phase 5: 주문 매칭 엔진 (`mock_order_engine.rs` 신규)
- [ ] MockOrderEngine 구조체 (미체결 큐 + 매칭 로직)
- [ ] 시장가 즉시 체결 (OrderBook VWAP)
- [ ] 지정가 주문 매칭 (Limit Buy: ask ≤ limit, Limit Sell: bid ≥ limit)
- [ ] 스톱 주문 (StopLoss/TakeProfit/StopLossLimit/TakeProfitLimit)
- [ ] 부분 체결 (OrderBook 호가 잔량 기반)
- [ ] 주문 취소/정정

#### Phase 2+6: MockExchangeProvider 통합 (`mock.rs` 수정)
- [ ] StrategyState에 reserved_balance 추가 (잔고 예약 시스템)
- [ ] MockState에 order_engine 추가
- [ ] place_order() 재구현 (Market=즉시, Limit/Stop=큐)
- [ ] cancel_order() / modify_order() 재구현
- [ ] fetch_pending_orders() 실제 미체결 주문 반환
- [ ] start_streaming_with_config() (기존 start_streaming 위임)
- [ ] 스트리밍 루프에 주문 매칭 통합 (매 틱마다 on_price_tick)
- [ ] 최신 시세/호가 캐시 (latest_tickers, latest_order_books)

#### Phase 3: 모듈 등록 (`mod.rs`)
- [ ] mock_streaming, mock_order_engine 모듈 등록 + export

#### Phase 4: Paper Trading API 통합 (`paper_trading.rs`)
- [ ] PaperTradingStartRequest에 streaming_config 추가
- [ ] kline 데이터 로드 (모드별: 1분봉/D1/None)

#### DB 마이그레이션
- [ ] mock_pending_orders 테이블 생성
- [ ] load_state() 미체결 주문 복원

---

### 프론트엔드 통합 및 테스트

- [ ] 스키마 없는 전략 fallback UI (JSON 에디터)
- [ ] 브라우저 테스트 (Chrome, Firefox, Safari)
- [ ] 반응형 레이아웃 확인

---

---

# 완료된 작업 아카이브

> 이하 섹션은 완료된 작업들의 기록입니다. 최신순 정렬.

---

## ✅ v0.8.3 — 쿼리 최적화 + 백테스트 타임프레임 폴백 + UI 성능 (2026-02-08)

| 항목 | 상태 |
|------|:----:|
| OHLCV LATERAL JOIN + TimescaleDB 청크 프루닝 (1,040ms → 306ms) | ✅ |
| 백테스트 다중 타임프레임 폴백 (primary → secondary → 일반) | ✅ |
| StrategyRegistry 메타 연동 + CLI `to_registry_id()` | ✅ |
| 스크리닝 상태/등급/점수 정렬 (우선순위 기반) | ✅ |
| 네이티브 가상 스크롤 (11,451행 → 25행 DOM, 60fps) | ✅ |
| 매매일지 백테스트 인사이트 강화 (매수/매도, 거래량, 최대 수익/손실) | ✅ |

---

## ✅ v0.8.2 — 성능 최적화 + 리스크 관리 + 데이터 무결성 (2026-02-08)

### ExitConfig 통합 리스크 관리

| 항목 | 상태 |
|------|:----:|
| ExitConfig 5가지 리스크 타입 (StopLoss/TakeProfit/TrailingStop/ProfitLock/DailyLossLimit) | ✅ |
| TrailingStop 4가지 모드 (FixedPercentage, AtrBased, Step, ParabolicSar) | ✅ |
| Strategy trait `exit_config()` + `enrich_signal()` 자동 적용 | ✅ |
| 6가지 프리셋 + SDUI Schema Registry fragment 재작성 | ✅ |
| 16개 전략 `exit_config()` 기본 구현 + 694줄 테스트 | ✅ |

### CandleProcessor + 스크리닝 + GlobalScore

| 항목 | 상태 |
|------|:----:|
| BacktestEngine/SimulationEngine 공통 캔들 처리 추출 (592줄) | ✅ |
| 배치 kline 쿼리 + Redis 구조적 특성 캐시 (10초 → 서브초) | ✅ |
| GlobalScore Semaphore(10) 동시 처리 (~50초 → ~5-8초) | ✅ |
| 심볼 무결성 관리 (cascade delete + orphan cleanup + save_klines 검증) | ✅ |

---

## ✅ v0.8.1 — StructuralFeatures 통합 + LiveExecutor + DCA 그룹 (2026-02-07)

### StructuralFeatures Decimal 통합

| 항목 | 상태 |
|------|:----:|
| trader-core Decimal 구조체 보강 (breakout_score, Default) | ✅ |
| StructuralFeaturesCalculator 3-arg from_candles (IndicatorEngine) | ✅ |
| indicators/structural.rs 레거시 struct/impl 제거 (-453줄) | ✅ |
| route_state_calculator / global_scorer Decimal 비교 전환 | ✅ |
| backtest/engine.rs IndicatorEngine 통합 | ✅ |

### LiveExecutor

| 항목 | 상태 |
|------|:----:|
| LiveExecutor 구현 (1,026줄) + OrderExecutionProvider trait (+95줄) | ✅ |
| SignalProcessor trait 확장 (+222줄) + Mock Provider 구현 | ✅ |

### DCA 전략 그룹 + GlobalScore 재설계

| 항목 | 상태 |
|------|:----:|
| Grid/MagicSplit/InfinityBot → DcaStrategy 통합 + Position ID / Group ID | ✅ |
| 고정 심볼 10개 전략 GlobalScore 필터 제거, CandlePattern 강도 조정 전환 | ✅ |
| Rotation KR만 필터 유지, RouteState 전체 Overheat만 차단 | ✅ |
| 백테스트 CLI TOML + 멀티에셋 + Signal Analysis Report + 차트 생성 | ✅ |
| symbol_info 데이터 무결성 관리 (cascade delete + orphan cleanup) | ✅ |

---

## ✅ v0.8.0 — 실시간 WebSocket + Paper Trading (2026-02-07)

| 항목 | 상태 |
|------|:----:|
| KIS WebSocket 동적 구독 (6-Phase: subscribe → Bridge Task → Singleton → 자동 연결 → 시작 순서 → FE 브릿지) | ✅ |
| Exchange Provider 통합 (KIS KR/US → `kis.rs`, Mock, Client 공통화) | ✅ |
| Paper Trading (API 5개 + Signal Processor + FE + TypeScript 바인딩 10개) | ✅ |
| Collector 확장 (Market Breadth 동기화, OHLCV 수집 개선) | ✅ |

---

## ✅ 전략 재설계 + 병합 (2026-02-04 ~ 2026-02-05)

### 전략 병합 (5개 그룹, 코드 ~58% 감소)

| 그룹 | 대상 전략 | 통합명 | 코드 감소 |
|:----:|----------|--------|:---------:|
| 1 | HAA, XAA, BAA, All Weather, Dual Momentum | `AssetAllocation` | 64% |
| 2 | RSI, Bollinger | `MeanReversion` | 72% |
| 3 | Sector Momentum, Market Cap Top, Stock Rotation | `RotationStrategy` | 72% |
| 4 | Volatility Breakout, SMA Crossover, Market Interest Day | `DayTrading` | 57% |
| 5 | Grid, MagicSplit, InfinityBot | `DcaStrategy` (v0.8.1) | - |

> 병합 제외 (독립 유지): Candle Pattern, US 3X Leverage, Pension Portfolio, Compound Momentum, Small Cap Quant, Range Trading, Momentum Surge

### 전략 핵심 재설계

| 전략 | 재설계 내용 |
|------|------------|
| MomentumPower | 리밸런싱 월간화 (30일), 모드 단순화 |
| Infinity Bot v2.0 | 라운드 조건 MarketRegime 기반 단순화 |
| Sector VB v2.0 | KST 시간대 수정, StrategyContext 완전 연동 |
| US 3X Leverage v2.0 | MarketRegime/MacroRisk 기반 환경 판단 |
| CompoundMomentum | 백테스트 엔진 업데이트 |
| RangeTrading | 구간 경계 버그 수정 |

---

## ✅ 기반 기능 + 프론트엔드 + 백엔드 API (2026-02-04)

### Backend 기반 기능

| 항목 | 구현 파일 |
|------|----------|
| Trigger 연동 | `context.rs:322, 670-678`, `analytics_provider.rs:101-104` |
| Volume Profile | `volume_profile.rs` |
| Correlation | `correlation.rs` |
| Score History | `score_history.rs` |
| Sector RS | `sector_rs.rs` |
| Survival Days | `survival.rs` |
| Weekly MA20 | `indicators/weekly_ma.rs` |
| Dynamic Route Tagging | `route_state_calculator.rs` |
| Reality Check | `routes/reality_check.rs` |
| Keltner Channel | `indicators/volatility.rs` |
| VWAP | `indicators/volume.rs` |

### 프론트엔드 완성

| 항목 | 상태 |
|------|:----:|
| Screening UI (필터, 프리셋, RouteState 뱃지, 종목 상세) | ✅ |
| Global Ranking UI (시장별 필터, 레이더 차트) | ✅ |
| 캔들 차트 신호 시각화 (SignalMarkerOverlay, IndicatorFilterPanel) | ✅ |
| 7-Factor Radar 연동 (`RadarChart.tsx` + `GET /ranking/7factor`) | ✅ |
| Score History 차트 연동 (`ScoreHistoryChart.tsx` + `SymbolDetail.tsx`) | ✅ |
| SDUI 전략 모달 (`SDUIEditModal.tsx` + `AddStrategyModal.tsx` + `SDUIRenderer/`) | ✅ |
| 백테스트 설정 (`StrategyRegistry` 연동 + 타임프레임 폴백) | ✅ |

### 시각화 컴포넌트 (12개)

FearGreedGauge, MarketBreadthWidget, SurvivalBadge, ScoreWaterfall, SectorTreemap, KellyVisualization, CorrelationHeatmap, OpportunityMap, KanbanBoard, RegimeSummaryTable, SectorMomentumBar, VolumeProfile

### 백엔드 API

| 항목 | 구현 |
|------|------|
| 관심종목 | `GET/POST /watchlist`, `POST/DELETE /watchlist/{id}/items` |
| 전략 symbols 연결 | `PUT /api/v1/strategies/{id}/symbols` |
| 프리셋 저장/삭제 | `POST/DELETE /api/v1/screening/presets` |
| 7Factor 데이터 | `GET /api/v1/ranking/7factor/{ticker}`, batch |
| FIFO 원가 계산 | `GET /api/v1/journal/cost-basis/{symbol}` |
| 고급 거래 통계 | `max_consecutive_wins/losses`, `max_drawdown` |

### 사용성 개선

| 항목 | 상태 |
|------|:----:|
| RankChangeIndicator, FavoriteButton, ExportButton, AutoRefreshToggle | ✅ |
| 대시보드 추가 컴포넌트 (ScoreWaterfall, RegimeSummary, SectorTreemap) | ✅ |
| Multi Timeframe UI (Selector, Chart, useMultiTimeframeKlines) | ✅ |
| 상태 관리 리팩토링 (signals → stores, 50~86% 감소) | ✅ |
| Lazy Loading 11페이지 + manualChunks (1,512KB → 12.5KB, 99% 감소) | ✅ |

### 문서 정리

- [x] Python 참조 주석 모두 제거
- [x] 각 전략 docstring에 핵심 개념만 기술
- [x] STRATEGY_GUIDE.md 통합 완료

### 스크리닝 연동

| 항목 | 적용 전략 |
|------|----------|
| `min_global_score` Config | Sector VB, US 3X Leverage, Infinity Bot |
| `RouteState::Attack/Armed` 필터 | Sector VB (진입 필터) |
| `MacroEnvironment` 연동 | US 3X Leverage (Crisis 모드 자동 전환) |
| `MarketRegime` 연동 | Sector VB, US 3X Leverage, Infinity Bot |
