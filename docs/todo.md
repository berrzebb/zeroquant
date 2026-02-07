# ZeroQuant TODO - 통합 로드맵

> **마지막 업데이트**: 2026-02-07
> **현재 버전**: v0.8.1
> **상세 계획**: `.claude/plans/warm-sniffing-waterfall.md`

---

## ✅ P0: 기반 기능 완성 (완료)

> **완료일**: 2026-02-04
> **원칙**: 전략 재설계 전에 모든 기반 모듈 100% 완성 필수

### 0.1 Backend 필수 (6개) - 모두 완료 ✅

| 항목 | 상태 | 위치 |
|------|:----:|------|
| ✅ **Trigger 필드 연동** | 100% | `trader-core/src/domain/context.rs:322, 670-678` |
| ✅ Sector RS (섹터 상대강도) | 100% | `trader-analytics/src/sector_rs.rs` |
| ✅ Survival Days (생존일 추적) | 100% | `trader-analytics/src/survival.rs` |
| ✅ Weekly MA20 (주봉 20선) | 100% | `trader-analytics/src/indicators/weekly_ma.rs` |
| ✅ Dynamic Route Tagging | 100% | `trader-analytics/src/route_state_calculator.rs` |
| ✅ Reality Check (추천 검증) | 100% | `trader-api/src/routes/reality_check.rs` |

### 0.2 Trigger 필드 연동 - 완료 ✅

- [x] `trader-core/src/domain/analytics_provider.rs` - ScreeningResult에 `trigger_score`, `trigger_label` 추가 (lines 101-104)
- [x] `trader-core/src/domain/context.rs` - StrategyContext에 `trigger_results: HashMap<String, TriggerResult>` 추가 (line 322)
- [x] `get_trigger(ticker)` 헬퍼 메서드 추가 (lines 670-672)
- [x] `update_trigger_results()` 메서드 추가 (lines 675-678)

---

## ✅ P0.3: 백엔드 기능 완성 (완료)

> **완료일**: 2026-02-04

| 항목 | FE | BE | 위치 |
|------|:--:|:--:|------|
| ✅ Volume Profile (매물대) | ✅ | ✅ | `trader-analytics/src/volume_profile.rs` |
| ✅ Correlation Heatmap | ✅ | ✅ | `trader-analytics/src/correlation.rs` |
| ✅ Score History | ✅ | ✅ | `trader-api/src/repository/score_history.rs`, `migrations/20_score_history.sql` |
| 🟡 Interactive Chart 오버레이 연동 | ✅ | 🟡 | Keltner, VWAP 추가 필요 |

---

## 🔄 P0.7: 전략 병합 및 일반화 (Day 7-9)

> 유사 패턴 전략 → 베이스 전략 + 설정으로 통합 (코드 ~58% 감소)

### ⚠️ 전략 재작성 규칙

> **핵심 원칙**: 그룹 전략 → 파생 전략 순서로 구현

1. **그룹 전략 먼저 생성** - 공통 로직을 담은 베이스 전략 구현
2. **파생 전략은 Config 기반** - 기존 전략은 그룹 전략의 Config 조합으로 재구현
3. **테스트 분리 필수** - `tests/` 디렉토리에 별도 파일로 테스트 작성
4. **Public API만 테스트** - Strategy trait 메서드 (initialize, on_market_data 등)만 테스트
5. **기존 전략 파일 제거** - 그룹 전략으로 통합 완료 시 원본 파일 삭제

```rust
// 예시: HAA 전략은 AssetAllocation 그룹 전략의 Config로 구현
let haa = AssetAllocationConfig::haa_default();
let strategy = AssetAllocationStrategy::new();
strategy.initialize(serde_json::to_value(haa)?).await?;
```

### 병합 대상 (4개 그룹)

| 그룹 | 대상 전략 | 통합명 | 상태 | 코드 감소 |
|:----:|----------|--------|:----:|:---------:|
| 1 | HAA, XAA, BAA, All Weather, Dual Momentum | `AssetAllocation` | ✅ 완료 | 64% |
| 2 | RSI, Bollinger | `MeanReversion` | ✅ 완료 | 72% |
| 5 | Grid, MagicSplit, InfinityBot | `DcaStrategy` | ✅ 완료 (v0.8.1) | - |
| 3 | Sector Momentum, Market Cap Top, Stock Rotation | `RotationStrategy` | ✅ 완료 | 72% |
| 4 | Volatility Breakout, SMA Crossover, Market Interest Day | `DayTrading` | ✅ 완료 | 57% |

### 완료된 그룹 전략

#### ✅ AssetAllocation (자산배분 그룹)
- **파일**: `crates/trader-strategy/src/strategies/asset_allocation.rs`
- **테스트**: `crates/trader-strategy/tests/asset_allocation_test.rs`
- **지원 Variant**: HAA, XAA, BAA, AllWeather, DualMomentum
- **Factory 메서드**: `haa_default()`, `xaa_default()`, `baa_default()`, `all_weather_default()`, `dual_momentum_default()`

#### ✅ MeanReversion (평균회귀 그룹)
- **파일**: `crates/trader-strategy/src/strategies/mean_reversion.rs`
- **테스트**: `crates/trader-strategy/tests/mean_reversion_test.rs` (32개 테스트)
- **지원 Variant**: RSI, Bollinger
- **Factory 메서드**: `rsi_default()`, `bollinger_default()`
- ⚠️ Grid, MagicSplit은 v0.8.1에서 `DcaStrategy`로 이전됨

#### ✅ RotationStrategy (로테이션 그룹)
- **파일**: `crates/trader-strategy/src/strategies/rotation.rs`
- **테스트**: `crates/trader-strategy/tests/rotation_test.rs` (41개 테스트)
- **지원 Variant**: SectorMomentum, StockMomentum, MarketCapTop
- **Factory 메서드**: `sector_momentum()`, `stock_rotation()`, `market_cap_top()`

#### ✅ DayTrading (일간모멘텀 그룹)
- **파일**: `crates/trader-strategy/src/strategies/day_trading.rs`
- **테스트**: `crates/trader-strategy/tests/day_trading_test.rs` (32개 테스트)
- **지원 Variant**: Breakout (변동성 돌파), Crossover (SMA 크로스오버), VolumeSurge (거래량 급증)
- **Factory 메서드**: `breakout()`, `crossover()`, `volume_surge()`

### 선행 작업

- [x] `common/momentum.rs` - MomentumCalculator 통합 (6곳 → 1곳) ✅
- [x] `common/rebalance.rs` - RebalanceCalculator 통합 (5곳 → 1곳) ✅

### 병합 제외 (독립 유지)

Candle Pattern, US 3X Leverage, Pension Portfolio, Compound Momentum, Small Cap Quant, Range Trading, Momentum Surge (총 7개)

> **v0.8.1 변경**: Grid, MagicSplit, InfinityBot은 `DcaStrategy` 그룹으로 통합됨 (`trader-strategy/src/strategies/dca.rs`)

---

## ✅ P2: 전략 핵심 재설계 + 테스트 (완료)

> **완료일**: 2026-02-05
> Python 코드 참조 금지 - 핵심 아이디어만 추출하여 독자 재구현

| 전략 | 재설계 내용 | 상태 |
|------|------------|:----:|
| MomentumPower | 리밸런싱 월간화 (30일), 모드 단순화 | ✅ |
| Infinity Bot v2.0 | 라운드 조건 MarketRegime 기반 단순화 | ✅ |
| Sector VB v2.0 | KST 시간대 수정, StrategyContext 완전 연동 | ✅ |
| US 3X Leverage v2.0 | MarketRegime/MacroRisk 기반 환경 판단 | ✅ |
| CompoundMomentum | 백테스트 엔진 업데이트 | ✅ |
| RangeTrading | 구간 경계 버그 수정 | ✅ |

### 재설계된 전략 구현 위치

| 전략 | 파일 위치 |
|------|----------|
| MomentumPower | `crates/trader-strategy/src/strategies/momentum_power.rs` |
| Infinity Bot v2.0 | `crates/trader-strategy/src/strategies/infinity_bot.rs` |
| Sector VB v2.0 | `crates/trader-strategy/src/strategies/sector_vb.rs` |
| US 3X Leverage v2.0 | `crates/trader-strategy/src/strategies/us_3x_leverage.rs` |
| CompoundMomentum | `crates/trader-strategy/src/strategies/compound_momentum.rs` |
| RangeTrading | `crates/trader-strategy/src/strategies/range_trading.rs` |

---

## ✅ P1.5: 실시간 WebSocket 연동 + Paper Trading (완료)

> **완료일**: 2026-02-07
> **버전**: v0.8.0

### KIS WebSocket 실시간 연동 (6-Phase)

| Phase | 내용 | 상태 |
|-------|------|:----:|
| 1 | 동적 구독 지원 (subscribe_live/unsubscribe_live) | ✅ |
| 2 | 순차 폴링 → 동시 수신 (Bridge Task + mpsc) | ✅ |
| 3 | Singleton 스트림 관리 (MarketStreamHandle) | ✅ |
| 4 | 전략 실행 시 스트림 자동 연결 | ✅ |
| 5 | 서버 시작 순서 최적화 | ✅ |
| 6 | 프론트엔드 WebSocket 브릿지 | ✅ |

### Exchange Provider 통합

| 항목 | 상태 |
|------|:----:|
| KIS KR/US → 통합 `kis.rs` | ✅ |
| MockExchangeProvider | ✅ |
| KIS Client 공통화 (`client.rs`) | ✅ |
| Core Domain 확장 (Account, ExchangeTypes) | ✅ |

### Paper Trading

| 항목 | 상태 |
|------|:----:|
| API 엔드포인트 (5개) | ✅ |
| Signal Processor 서비스 | ✅ |
| 프론트엔드 (PaperTrading.tsx) | ✅ |
| TypeScript 바인딩 (10개) | ✅ |

### Collector 확장

| 항목 | 상태 |
|------|:----:|
| Market Breadth 동기화 | ✅ |
| OHLCV 수집 개선 | ✅ |

---

## ✅ P1.6: StructuralFeatures 통합 + LiveExecutor + DCA 그룹 (완료)

> **완료일**: 2026-02-07
> **버전**: v0.8.1

### StructuralFeatures Decimal 통합

| 항목 | 상태 |
|------|:----:|
| trader-core Decimal 구조체 보강 (breakout_score, Default) | ✅ |
| StructuralFeaturesCalculator 3-arg from_candles (IndicatorEngine) | ✅ |
| indicators/structural.rs 레거시 struct/impl 제거 (-453줄) | ✅ |
| route_state_calculator Decimal 비교 전환 | ✅ |
| global_scorer Decimal 비교 전환 | ✅ |
| backtest/engine.rs IndicatorEngine 통합 | ✅ |
| 프론트엔드 타입 안전성 개선 (SymbolDetailModal, Screening) | ✅ |

### LiveExecutor

| 항목 | 상태 |
|------|:----:|
| LiveExecutor 구현 (1,026줄) | ✅ |
| OrderExecutionProvider trait (+95줄) | ✅ |
| SignalProcessor trait 확장 (+222줄) | ✅ |
| Mock Provider OrderExecutionProvider 구현 | ✅ |

### DCA 전략 그룹

| 항목 | 상태 |
|------|:----:|
| Grid/MagicSplit/InfinityBot → DcaStrategy 통합 | ✅ |
| Position ID / Group ID 시스템 | ✅ |
| SimulatedExecutor 개별 레벨 청산 버그 수정 | ✅ |
| is_grid_strategy() 레거시 함수 제거 | ✅ |

### GlobalScore 활용 재설계

| 항목 | 상태 |
|------|:----:|
| 고정 심볼/자체 모멘텀 전략 10개 GlobalScore 필터 제거 | ✅ |
| CandlePattern GlobalScore → 강도 조정 전환 (0.75x~1.25x) | ✅ |
| Rotation KR만 GlobalScore 필터 조건부 유지 | ✅ |
| RouteState 전략 전체 Overheat만 차단으로 완화 | ✅ |
| Grid `reset_threshold_pct` 신규 파라미터 (min_global_score 대체) | ✅ |
| InfinityBot 하락장 진입 허용 (DCA 특성 반영) | ✅ |
| MeanReversion variant fallback 버그 수정 | ✅ |

### 기타

| 항목 | 상태 |
|------|:----:|
| Strategy Watched Tickers (마이그레이션 + Repository + API) | ✅ |
| 백테스트 CLI TOML 설정 파일 지원 (+643줄) | ✅ |
| 멀티에셋 백테스트 (`run_multi_asset_backtest()`, StrategyContext 통합) | ✅ |
| 스마트 심볼 해석 (KR 종목코드 → `.KS`/`.KQ` 자동 변환) | ✅ |
| Signal Analysis Report (`generate_signal_analysis()`, 205줄) | ✅ |
| Grid warmup_candles 파라미터 (초기 관찰 기간) | ✅ |
| 백테스트 차트 자동 생성 | ✅ |
| Collector watchlist_helper 개선 | ✅ |
| CLI .env 자동 로딩 (dotenvy) | ✅ |

---

## P3: 프론트엔드 완성 (Day 17-19)

- [ ] Interactive Chart 오버레이 - Keltner, VWAP, RSI 서브차트 연동
- [ ] 7-Factor Radar 백엔드 데이터 연동
- [ ] Score History 차트 연동

---

## ✅ P4: 스크리닝 연동 (완료)

> **완료일**: 2026-02-05
> 재설계된 모든 전략에 StrategyContext 연동 완료

| 항목 | 적용 전략 | 상태 |
|------|----------|:----:|
| `min_global_score` Config | Sector VB, US 3X Leverage, Infinity Bot | ✅ |
| `RouteState::Attack/Armed` 필터 | Sector VB (진입 필터) | ✅ |
| `MacroEnvironment` 연동 | US 3X Leverage (Crisis 모드 자동 전환) | ✅ |
| `MarketRegime` 연동 | Sector VB, US 3X Leverage, Infinity Bot | ✅ |

### StrategyContext 헬퍼 메서드 (async)

```rust
// 모든 재설계 전략에서 사용 가능
ctx.get_global_score(ticker)      // GlobalScoreResult
ctx.get_route_state(ticker)       // RouteState
ctx.get_market_regime(ticker)     // MarketRegime
ctx.get_macro_environment()       // MacroEnvironment
ctx.get_trigger(ticker)           // TriggerResult
```

---

## P5: 문서 정리 (Day 22)

- [x] Python 참조 주석 모두 제거
- [ ] 각 전략 docstring에 핵심 개념만 기술
- [x] STRATEGY_GUIDE.md 통합 완료

---

## 6.1 통합 및 테스트 (미완료)

- [ ] 전략 추가 모달에 적용
- [ ] 백테스트 설정에 적용
- [ ] 스키마 없는 전략 fallback UI
- [ ] 브라우저 테스트 (Chrome, Firefox, Safari)
- [ ] 반응형 레이아웃 확인

---

---

# 📋 상세 구현 참조

> 이 섹션은 상단 작업 항목의 상세 구현 내용입니다.

## ✅ A~D: 기반 기능 구현 완료 (2026-02-04)

모든 기반 기능이 구현되었습니다:

| 항목 | 구현 파일 |
|------|----------|
| **Trigger 연동** | `trader-core/src/domain/context.rs:322, 670-678`, `analytics_provider.rs:101-104` |
| **Volume Profile** | `trader-analytics/src/volume_profile.rs` (POC, ValueArea 계산) |
| **Correlation** | `trader-analytics/src/correlation.rs` (Pearson 상관계수) |
| **Score History** | `trader-api/src/repository/score_history.rs`, `migrations/20_score_history.sql` |
| **Sector RS** | `trader-analytics/src/sector_rs.rs` (SectorRsCalculator) |
| **Survival Days** | `trader-analytics/src/survival.rs` (SurvivalTracker) |
| **Weekly MA20** | `trader-analytics/src/indicators/weekly_ma.rs` (resample_to_weekly, calculate_weekly_ma) |
| **Dynamic Route Tagging** | `trader-analytics/src/route_state_calculator.rs` (DynamicThresholds, calculate_dynamic) |
| **Reality Check** | `trader-api/src/routes/reality_check.rs` (5개 API 엔드포인트) |

---

## F. 전략 병합 상세

### F.1 AssetAllocation (자산배분 통합)

**대상**: HAA, XAA, BAA, All Weather, Dual Momentum

```rust
pub enum SelectionStrategy {
    TopNByMomentum { n: usize, weights: Option<Vec<Decimal>> },
    CanaryGated { canary_ticker: String, threshold: Decimal },
    DualMomentum { absolute: bool, relative: bool },
    SeasonalAdjusted { base_weights: HashMap<String, Decimal> },
}
```

### F.2 호환성 유지

```rust
// 기존 ID 유지 → 통합 전략으로 라우팅
registry.register_alias("haa", "asset_allocation", HaaConfig::default());
registry.register_alias("xaa", "asset_allocation", XaaConfig::default());
```

---

---

# ✅ 완료된 작업

> 이 섹션은 완료된 작업들의 기록입니다.

---

## ✅ P0.5: 전략 명칭 일반화 (완료)

**완료일**: 2026-02-04

README.md에서 확인 - 이미 일반화된 명칭 사용 중:
- 실시간: Grid Trading, RSI Mean Reversion, Bollinger Bands, Magic Split, Infinity Bot
- 일간: Volatility Breakout, SMA Crossover, Compound Momentum, Stock Rotation, Market Interest Day, Candle Pattern
- 월간: All Weather, HAA, XAA, Momentum Power, Market Cap Top, BAA, Dual Momentum, Pension Portfolio
- 섹터: Sector Momentum, Sector VB, US 3X Leverage
- 국내: Momentum Surge, Market BothSide, Small Cap Quant, Range Trading

**추가 작업 불필요** - 파일명/구조체명은 기존 유지 (내부 구현명과 외부 표시명 분리)

---

## Phase 2 프론트엔드 UI (완료)

### 2.1. Screening UI ✅
- 필터 조건 입력 폼, 프리셋 선택 UI
- 결과 테이블 (정렬/페이지네이션)
- RouteState 뱃지, 종목 상세 모달

### 2.2. Global Ranking UI ✅
- 시장별 필터, 레이더 차트, RouteState 필터링
- `RankingWidget.tsx` → Dashboard.tsx 통합

### 2.3. 캔들 차트 신호 시각화 ✅
- `SignalMarkerOverlay` 컴포넌트
- `IndicatorFilterPanel` 컴포넌트

---

## Phase 3 백엔드 API (완료)

### 3.1 관심종목 API ✅
- `watchlist` 테이블 마이그레이션
- API: `GET/POST /watchlist`, `POST/DELETE /watchlist/{id}/items`

### 3.2 전략 symbols 연결 API ✅
- `PUT /api/v1/strategies/{id}/symbols`

### 3.3 프리셋 저장/삭제 API ✅
- `POST /api/v1/screening/presets`
- `DELETE /api/v1/screening/presets/{id}`

### 3.4 7Factor 데이터 API ✅
- `SevenFactorCalculator` 구현 (7개 팩터 정규화)
- `GET /api/v1/ranking/7factor/{ticker}`

### 3.5 FIFO 원가 계산 API ✅
- `CostBasisTracker` 모듈
- `GET /api/v1/journal/cost-basis/{symbol}`

### 3.6 고급 거래 통계 API ✅
- `max_consecutive_wins/losses`, `max_drawdown` 계산

---

## Phase 4 시각화 컴포넌트 (완료)

| 컴포넌트 | 상태 |
|----------|:----:|
| FearGreedGauge | ✅ |
| MarketBreadthWidget | ✅ |
| SurvivalBadge | ✅ |
| ScoreWaterfall | ✅ |
| SectorTreemap | ✅ |
| KellyVisualization | ✅ |
| CorrelationHeatmap | ✅ |
| OpportunityMap | ✅ |
| KanbanBoard | ✅ |
| RegimeSummaryTable | ✅ |
| SectorMomentumBar | ✅ |
| VolumeProfile | ✅ |

---

## Phase 6 사용성 개선 (완료)

### 6.5 추가 기능 ✅
- `RankChangeIndicator.tsx` - 순위 변동 표시
- `FavoriteButton.tsx` - 종목 즐겨찾기 토글
- `ExportButton.tsx` - Excel 내보내기
- `AutoRefreshToggle.tsx` - 자동 갱신 토글

### 6.6 대시보드 추가 컴포넌트 연동 ✅
- ScoreWaterfall, RegimeSummaryTable, SectorTreemap, SectorMomentumBar

### 6.7 차트 시각화 개선 ✅
- `TradeConnectionOverlay.tsx` - 진입/청산 연결선
- `SignalCorrelationChart.tsx` - 신호-수익률 상관관계

### 6.8 Multi Timeframe UI ✅
- `MultiTimeframeSelector.tsx` - Primary/Secondary TF 선택
- `MultiTimeframeChart.tsx` - 멀티 TF 차트 동기화
- `useMultiTimeframeKlines.ts` - API 연동 훅

---

## 6.9 상태 관리 및 아키텍처 개선 (완료)

### 6.9.1 상태 관리 리팩토링 ✅ (2026-02-04)

| 페이지 | 변환 전 | 변환 후 | 감소율 |
|--------|---------|---------|--------|
| Strategies.tsx | ~15 signals | 4 stores | ~73% |
| TradingJournal.tsx | ~20 signals | 5 stores | ~75% |
| Screening.tsx | 29 signals | 4 stores | ~86% |
| Backtest.tsx | 19 signals | 4 stores | ~79% |
| Dashboard.tsx | 4 signals | 2 stores | 50% |

### 6.9.2 커스텀 훅 추출 ✅ (2026-02-04)
- useStrategies, useJournal, useScreening, useMarketSentiment

### 6.9.3 성능 최적화 ✅ (2026-02-04)
- Lazy Loading: 11개 페이지 모두 적용
- manualChunks: 번들 index.js 1,512 KB → 12.5 KB (**99% 감소**)
- VirtualizedTable, LazyImage, 디바운스/쓰로틀 훅

---

## Phase 1 핵심 기능 (완료)

### 1.4 Multiple KLine Period (다중 타임프레임) ✅ (2026-02-04)

**백엔드**:
- Strategy Trait 확장 - `multi_timeframe_config()`, `on_multi_timeframe_data()`
- StrategyMeta - `isMultiTimeframe` 필드
- `TimeframeAligner` 모듈 - Look-Ahead Bias 방지
- 백테스트 엔진 `run_multi_timeframe()` 메서드

**API**:
- `GET /api/v1/market/klines/multi`
- `GET/PUT /api/v1/strategies/{id}/timeframes`

**프론트엔드**:
- `MultiTimeframeSelector.tsx`, `MultiTimeframeChart.tsx`
- `useMultiTimeframeKlines.ts` (TTL 캐싱)

### 6.8.5 Multi Timeframe 후속 작업 ✅ (2026-02-04)
- 전략 생성/수정 시 TF 설정 저장
- 백테스트 설정 TF 선택 UI
- `BacktestRequest`에 `multi_timeframe_config` 필드

### 6.8.6 백테스트 API Multi Timeframe 지원 ✅ (2026-02-04)
- `MultiTimeframeRequest`, `SecondaryTimeframeConfig` API 타입
- `load_secondary_timeframe_klines()` 병렬 로드
- 통합 테스트 3건

---

## 7. 백엔드 API 상세 ✅ (완료)

**프론트엔드 연동 완료**:
- [x] 관심종목 UI (WatchlistSelectModal)
- [x] 전략 연결 UI (StrategyLinkModal)
- [x] 프리셋 저장/삭제 모달 UI (PresetModal)
- [x] 7Factor 레이더 차트 7축 확장
- [x] FIFO 원가 표시 (PositionDetailModal)
- [x] 고급 통계 표시 (TradingInsightsResponse)
