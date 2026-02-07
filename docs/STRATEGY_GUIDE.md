# ZeroQuant 전략 가이드

> **Version**: 1.0.0
> **Last Updated**: 2026-02-07

---

## 1. 핵심 원칙

### 1.1 심볼 유형별 분류

| 심볼 유형 | 설명 | GlobalScore 활용 | 예시 |
|----------|------|-----------------|------|
| **단일 티커** | 사용자가 지정한 1개 종목 | 강도 조정만 (0.75~1.25배) | RSI, Grid, Bollinger |
| **고정 리스트** | 사전 정의된 ETF/종목 목록 | 미사용 (자체 모멘텀) | HAA, AllWeather, PensionBot |
| **동적 Universe** | 스크리닝으로 선택되는 종목군 | 필터 + 강도 조정 | StockRotation(KR), SmallCapQuant |

### 1.2 StrategyContext 데이터 활용

| 데이터 | 용도 | 적용 전략 |
|--------|------|----------|
| **RouteState** | Overheat 차단 | 전 전략 공통 |
| **GlobalScore** | 강도 조정 / 스크리닝 | 심볼 유형에 따라 |
| **MarketBreadth** | 시장 전체 상태 | 선택적 |
| **MacroEnvironment** | 거시 환경 | 자산배분 전략 |
| **StructuralFeatures** | 추세/변동성 | 기술적 전략 |

### 1.3 GlobalScore 강도 조정 배율

```rust
fn adjust_strength_by_score(base: f64, score: Option<Decimal>) -> f64 {
    match score {
        Some(s) if s >= 90 => base * 1.25,  // 매우 강한 신호
        Some(s) if s >= 80 => base * 1.15,  // 강한 신호
        Some(s) if s >= 70 => base * 1.00,  // 기본
        Some(s) if s >= 60 => base * 0.85,  // 약한 신호
        Some(_) => base * 0.75,              // 보수적
        None => base,
    }
}
```

---

## 2. 전략별 상세

### 2.1 단일 티커 전략 (GlobalScore 강도 조정만)

#### RSI Mean Reversion

| 항목 | 값 |
|------|-----|
| **실행 주기** | 캔들 완성 시 |
| **핵심 로직** | RSI 14 과매수/과매도 |
| **GlobalScore** | 강도 조정만 (필터 X) |
| **RouteState** | Overheat 차단 |

```rust
// 파라미터
period: 14
oversold_threshold: 30.0
overbought_threshold: 70.0
use_ema_smoothing: true
cooldown_candles: 5
```

#### Bollinger Bands

| 항목 | 값 |
|------|-----|
| **실행 주기** | 캔들 완성 시 |
| **핵심 로직** | BB(20, 2σ) 이탈/복귀 |
| **GlobalScore** | 강도 조정만 |
| **StructuralFeatures** | 추세 확인 |

```rust
period: 20
std_dev_multiplier: 2.0
```

#### Grid Trading

| 항목 | 값 |
|------|-----|
| **실행 주기** | 가격 변동 시 |
| **핵심 로직** | 그리드 레벨별 분할 매수/매도 |
| **GlobalScore** | 강도 조정만 |
| **MarketRegime** | 레짐별 그리드 간격 조정 |

```rust
grid_spacing_pct: 1.0
grid_levels: 10
dynamic_spacing: false  // ATR 기반 동적 간격
atr_period: 14
reset_threshold_pct: 5.0
```

**Position ID 시스템**:
```rust
position_id: "{ticker}_grid_L{level}"
group_id: "grid_{base_price}_{timestamp}"
```

#### Magic Split (분할매수)

| 항목 | 값 |
|------|-----|
| **실행 주기** | 가격 변동 시 |
| **핵심 로직** | 하락 시 레벨별 분할 진입 |
| **GlobalScore** | 강도 조정만 |
| **MarketRegime** | 레짐별 진입 속도 조절 |

```rust
split_levels: 10
drop_pct_per_level: 3.0
amount_per_level: Decimal
take_profit_pct: 5.0
```

**Position ID 시스템**:
```rust
position_id: "{ticker}_split_L{level}"
group_id: "split_{ticker}_{timestamp}"
```

#### Infinity Bot (무한매수)

| 항목 | 값 |
|------|-----|
| **실행 주기** | 가격 변동 시 |
| **핵심 로직** | 50라운드 무한매수 |
| **GlobalScore** | 강도 조정만 |
| **MarketRegime** | Downtrend에서 진입 속도 조절 |

```rust
max_rounds: 50
round_pct: 2.0
dip_trigger_pct: 2.0
take_profit_pct: 3.0
ma_period: 20
```

#### Candle Pattern

| 항목 | 값 |
|------|-----|
| **실행 주기** | 캔들 완성 시 |
| **핵심 로직** | 35개 캔들 패턴 감지 |
| **GlobalScore** | `get_adjusted_strength()` 사용 |
| **StructuralFeatures** | `range_pos`로 패턴 위치 검증 |

```rust
patterns: Vec<CandlePatternType>  // 35개 패턴
min_pattern_score: 0.6
confirmation_candles: 1
```

#### Volatility Breakout

| 항목 | 값 |
|------|-----|
| **실행 주기** | 장 시작 5분 후 |
| **핵심 로직** | 전일 변동성 K 돌파 |
| **GlobalScore** | 강도 조정만 |
| **MacroEnvironment** | VIX 고점 시 비활성화 |

```rust
k_factor: 0.5
lookback_days: 20
entry_time: "09:05"
exit_time: "15:20"
```

#### Day Trading

| 항목 | 값 |
|------|-----|
| **실행 주기** | 캔들 완성 시 |
| **핵심 로직** | 단기 모멘텀 + RSI |
| **GlobalScore** | 미사용 (단일 티커) |
| **RouteState** | 진입 타이밍 필터 |

```rust
rsi_period: 14
rsi_oversold: 30.0
rsi_overbought: 70.0
momentum_period: 10
```

#### Momentum Surge

| 항목 | 값 |
|------|-----|
| **실행 주기** | 캔들 완성 시 |
| **핵심 로직** | 급등 모멘텀 감지 |
| **GlobalScore** | 미사용 |
| **StructuralFeatures** | 돌파 확인 |

```rust
surge_threshold: 3.0  // 3% 급등
volume_multiplier: 2.0
confirmation_period: 3
```

#### Range Trading

| 항목 | 값 |
|------|-----|
| **실행 주기** | 캔들 완성 시 |
| **핵심 로직** | 박스권 상하단 매매 |
| **GlobalScore** | 강도 조정만 (필터 X) |
| **StructuralFeatures** | `range_pos` 활용 |

```rust
range_period: 20
upper_percentile: 80
lower_percentile: 20
```

#### RSI Multi-TF

| 항목 | 값 |
|------|-----|
| **실행 주기** | 캔들 완성 시 |
| **핵심 로직** | 다중 타임프레임 RSI 컨펌 |
| **GlobalScore** | 강도 조정만 (필터 X) |

```rust
timeframes: ["1h", "4h", "1d"]
rsi_period: 14
confirmation_threshold: 2  // N개 이상 일치 시
```

---

### 2.2 고정 리스트 전략 (GlobalScore 미사용)

#### HAA (Hierarchical Asset Allocation)

| 항목 | 값 |
|------|-----|
| **실행 주기** | 월 1회 |
| **핵심 로직** | TIP 카나리아 기반 공격/방어 전환 |
| **GlobalScore** | 미사용 (자체 모멘텀) |
| **MacroEnvironment** | VIX 추가 카나리아 |

```rust
canary_assets: ["TIP"]
offensive_assets: ["SPY", "IWM", "VEA", "VWO", "TLT", "IEF", "PDBC", "VNQ"]
defensive_assets: ["IEF", "BIL"]
offensive_top_n: 4
defensive_top_n: 1
```

**모멘텀 계산**:
```rust
periods: [20, 60, 120, 240]  // 1M, 3M, 6M, 12M
momentum_score = returns.iter().sum() / 4.0
```

#### XAA (Extended Asset Allocation)

| 항목 | 값 |
|------|-----|
| **실행 주기** | 월 1회 |
| **핵심 로직** | TOP 4 모멘텀 선택 |
| **GlobalScore** | 미사용 (자체 모멘텀) |
| **MacroEnvironment** | VIX 수준별 집중/분산 |

```rust
offensive_assets: ["QQQ", "SPY", "VEA", "VWO", "TLT", "LQD", "GLD", "VNQ"]
defensive_assets: ["SHY", "IEF"]
top_n: 4
```

#### BAA (Bold Asset Allocation)

| 항목 | 값 |
|------|-----|
| **실행 주기** | 월 1회 |
| **핵심 로직** | 듀얼 카나리아 (SPY, VWO) |
| **GlobalScore** | 미사용 (자체 모멘텀) |
| **MarketBreadth** | 장기 건강도 확인 |

```rust
canary_aggressive: ["SPY"]
canary_conservative: ["VWO"]
offensive_assets: [...]
defensive_assets: ["IEF", "BIL", "LQD"]
```

#### All Weather

| 항목 | 값 |
|------|-----|
| **실행 주기** | 월 1회 |
| **핵심 로직** | 계절성 자산배분 |
| **GlobalScore** | 미사용 |
| **MacroEnvironment** | 사분면별 비중 조정 |

```rust
// 정적 비중
stocks: 30%     // VTI
long_bonds: 40% // TLT
mid_bonds: 15%  // IEI
gold: 7.5%      // GLD
commodities: 7.5% // DJP
```

#### Dual Momentum

| 항목 | 값 |
|------|-----|
| **실행 주기** | 월 1회 |
| **핵심 로직** | 한국주식 + 미국국채 |
| **GlobalScore** | 미사용 (자체 모멘텀) |

```rust
domestic_assets: ["KODEX200", "TIGER200"]
foreign_assets: ["TLT", "IEF"]
lookback_period: 12  // 12개월 모멘텀
```

#### Pension Bot

| 항목 | 값 |
|------|-----|
| **실행 주기** | 월 1회 |
| **핵심 로직** | 연금 정적+동적 배분 |
| **GlobalScore** | 미사용 |
| **MacroEnvironment** | 위험 조절 |

```rust
static_ratio: 0.6   // 60% 정적
dynamic_ratio: 0.4  // 40% 동적 모멘텀
```

#### US 3X Leverage

| 항목 | 값 |
|------|-----|
| **실행 주기** | 일간 |
| **핵심 로직** | 3배 레버리지/인버스 전환 |
| **GlobalScore** | 미사용 |
| **MarketRegime** | 레짐 기반 방향 결정 |
| **RouteState** | Attack/Armed 구분 |

```rust
leverage_long: "TQQQ"   // 3x 롱
leverage_short: "SQQQ"  // 3x 숏
neutral: "QQQ"          // 1x
```

#### Compound Momentum

| 항목 | 값 |
|------|-----|
| **실행 주기** | 월 1회 |
| **핵심 로직** | 복합 모멘텀 전환 |
| **GlobalScore** | 미사용 |

```rust
assets: ["SPY", "QQQ", "IWM", "EFA", "EEM"]
lookback_periods: [1, 3, 6, 12]  // 월
```

#### Market BothSide

| 항목 | 값 |
|------|-----|
| **실행 주기** | 일간 |
| **핵심 로직** | KOSPI 레버리지/인버스 양방향 |
| **GlobalScore** | 미사용 (지수 기반) |
| **MarketRegime** | 레짐별 방향 결정 |

```rust
leverage_etf: "KODEX 레버리지"
inverse_etf: "KODEX 인버스"
```

#### Sector Momentum (US)

| 항목 | 값 |
|------|-----|
| **실행 주기** | 월 1회 |
| **핵심 로직** | 섹터 RS 기반 로테이션 |
| **GlobalScore** | 미사용 (11개 고정 섹터) |
| **MarketBreadth** | 경기 사이클별 선호 섹터 |

```rust
sector_etfs: ["XLK", "XLF", "XLV", "XLY", "XLP", "XLE", "XLI", "XLB", "XLU", "XLRE", "XLC"]
top_n: 3
```

---

### 2.3 동적 Universe 전략 (GlobalScore 필터 사용)

#### Stock Rotation (KR)

| 항목 | 값 |
|------|-----|
| **실행 주기** | 일/주 |
| **핵심 로직** | KOSDAQ 모멘텀 순위 교체 |
| **GlobalScore** | 필터 + 순위 조정 |
| **MarketRegime** | 레짐별 보유 종목 수 조정 |

```rust
universe: "KOSDAQ 전체"
min_global_score: 50.0  // KR만 필터 적용
top_n: 10
rebalance_frequency: "weekly"
```

**참고**: US Stock Rotation은 고정 리스트 사용으로 GlobalScore 미적용

#### Market Cap TOP

| 항목 | 값 |
|------|-----|
| **실행 주기** | 월말 |
| **핵심 로직** | 미국 시총 상위 10 |
| **GlobalScore** | 가중치 조정용 |

```rust
top_n: 10
weight_method: "score_weighted"  // 균등 대신 품질 가중
```

#### Small Cap Quant

| 항목 | 값 |
|------|-----|
| **실행 주기** | 일간 |
| **핵심 로직** | 코스닥 소형주 퀀트 |
| **GlobalScore** | 필터 (55점 이상) + 유동성 40점 이상 |
| **StructuralFeatures** | `vol_quality`, `low_trend` 매집 종목 우선 |

```rust
min_global_score: 55.0
min_liquidity_score: 40.0
top_n: 10
```

#### MomentumPower

| 항목 | 값 |
|------|-----|
| **실행 주기** | 캔들 완성 시 |
| **핵심 로직** | 동적 유니버스 모멘텀 |
| **GlobalScore** | 필터 + 강도 조정 |
| **RouteState** | Attack 시 진입, Safe 시 청산 |

```rust
min_global_score: 60.0
momentum_period: 20
```

**중요**: 모드 변경 시 (Attack → Safe) 자산 청산 신호 발행 필수

#### Sector VB

| 항목 | 값 |
|------|-----|
| **실행 주기** | 장 시작 5분 후 |
| **핵심 로직** | 섹터 ETF 변동성 돌파 |
| **GlobalScore** | 섹터 순위 재정렬 |
| **MarketBreadth** | 경기 사이클별 선호 섹터 |

```rust
sector_etfs: ["XLK", "XLF", ...]  // 고정 리스트지만 순위 조정
k_factor: 0.5
```

---

## 3. 데이터 연동 매트릭스

### 3.1 RouteState 활용 (전 전략 공통)

| RouteState | 의미 | 전략 대응 |
|------------|------|----------|
| **Attack** | 강한 상승 모멘텀 | 적극 진입, 포지션 확대 |
| **Armed** | 잠재적 기회 | 진입 대기, 소량 진입 |
| **Wait** | 관망 | 신규 진입 금지 |
| **Overheat** | 과열 | 진입 금지, 부분 청산 고려 |

```rust
// 모든 전략에서 공통 적용
if ctx.get_route_state(ticker) == Some(RouteState::Overheat) {
    return vec![];  // 진입 금지
}
```

### 3.2 GlobalScore 활용 요약

| 전략 그룹 | 활용 방식 | 설명 |
|----------|----------|------|
| **단일 티커** | 강도만 | `get_adjusted_strength()` 0.75~1.25배 |
| **고정 리스트** | 미사용 | 자체 모멘텀 계산 |
| **동적 Universe (KR)** | 필터+강도 | `min_global_score` 필터 적용 |
| **동적 Universe (US)** | 강도만 | 미국 주식은 필터 스킵 |

### 3.3 MacroEnvironment 활용

| 지표 | 적용 전략 | 활용 방식 |
|------|----------|----------|
| **VIX** | HAA, XAA, BAA | 카나리아 추가 지표 |
| **VIX > 40** | InfinityBot | 조기 손절 |
| **Yield Curve** | BAA | 역전 시 방어 전환 |
| **USD/KRW** | KR 전략 | 1400 이상 시 보수적 |

### 3.4 MarketBreadth 활용

| 지표 | 적용 전략 | 활용 방식 |
|------|----------|----------|
| **above_ma20_pct** | Market Interest Day | 30% 이하 시 비활성화 |
| **above_ma50_pct** | BAA, HAA | 장기 건강도 |
| **sector_rotation** | Sector Momentum, SectorVB | 경기 사이클별 선호 섹터 |

### 3.5 StructuralFeatures 활용

| 피처 | 설명 | 활용 전략 |
|------|------|----------|
| **low_trend** | 저점 추세 | MeanReversion, CandlePattern |
| **vol_quality** | 매집 거래량 | SmallCapQuant, MarketInterestDay |
| **range_pos** | 박스권 위치 | RangeTrading, CandlePattern |
| **squeeze_on** | TTM Squeeze 상태 | Grid, MomentumSurge |

---

## 4. ExitConfig 프리셋

### 4.1 전략 유형별 프리셋

| 프리셋 | 손절 | 익절 | 트레일링 | 적용 전략 |
|--------|------|------|---------|----------|
| **for_day_trading()** | 2% | 4% | X | DayTrading, SectorVB, MomentumSurge |
| **for_mean_reversion()** | 3% | 6% | X | RSI, Bollinger, CandlePattern |
| **for_grid_trading()** | 15% | 3% | X | Grid, MagicSplit, InfinityBot |
| **for_rebalancing()** | X | X | X | AssetAllocation, Rotation, PensionBot |
| **for_leverage()** | 5% | 10% | O | Us3xLeverage, MarketBothSide |
| **for_momentum()** | 5% | 15% | O | CompoundMomentum, MomentumPower |

### 4.2 백테스트 설정 예시

```toml
# config/backtest/grid.toml
name = "Grid Trading Backtest"
strategy_type = "grid"

[parameters]
ticker = "005930"
spacing_pct = 2.0
levels = 10

[parameters.exit_config]
stop_loss_enabled = true
stop_loss_pct = 15.0
take_profit_enabled = true
take_profit_pct = 3.0
```

---

## 5. Position ID / Group ID 시스템

### 5.1 개요

스프레드 기반 전략(Grid, MagicSplit, InfinityBot)을 위한 2계층 포지션 식별 체계:

```
Signal {
    ticker: "005930"               // 실제 거래 심볼
    position_id: "005930_grid_L1"  // 개별 포지션 식별
    group_id: "grid_55000_1707..." // 관련 포지션 그룹
}
```

### 5.2 전략별 ID 형식

| 전략 | position_id | group_id |
|------|-------------|----------|
| Grid | `{ticker}_grid_L{level}` | `grid_{base_price}_{ts}` |
| MagicSplit | `{ticker}_split_L{level}` | `split_{ticker}_{ts}` |
| InfinityBot | `{ticker}_inf_R{round}` | `inf_{ticker}_{ts}` |

### 5.3 사용 패턴

```rust
// 레벨별 독립 포지션 생성
Signal::entry("grid", ticker, Side::Buy)
    .with_position_id(format!("{}_grid_L{}", ticker, level))
    .with_group_id(session_group_id)

// 특정 레벨만 청산
Signal::exit("grid", ticker, Side::Sell)
    .with_position_id(format!("{}_grid_L{}", ticker, level))

// 그룹 전체 청산
let keys = executor.position_keys_by_group("grid_session_1");
for key in keys { /* 각각 청산 */ }
```

---

## 6. 전략 실행 아키텍처

```
┌─────────────────────────────────────────────────┐
│  데이터 발행 (DataProvider) - 교체 가능         │
│  • 실환경: ExchangeProvider (실시간 데이터)     │
│  • 백테스트: BacktestEngine (과거 데이터)       │
└─────────────────────────────────────────────────┘
                    │
                    ▼
             ┌─────────────┐
             │   전략      │ → Signal 발행
             └─────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│  Signal 처리 (SignalProcessor) - 교체 가능      │
│  • 거래소: OrderExecutor (실제 주문)            │
│  • 시뮬레이션: SimulatedExecutor (가상 체결)    │
└─────────────────────────────────────────────────┘
```

| 데이터 발행 | Signal 처리 | 결과 |
|------------|------------|------|
| 실환경 | OrderExecutor | **실거래** |
| 실환경 | SimulatedExecutor | **페이퍼 트레이딩** |
| 백테스트 | SimulatedExecutor | **백테스트** |

---

## 7. 구현 체크리스트

### 7.1 전 전략 공통

- [ ] RouteState.Overheat 차단
- [ ] ExitConfig 파라미터화 (하드코딩 금지)
- [ ] ProcessorConfig에 전략 파라미터 전달

### 7.2 단일 티커 전략

- [ ] GlobalScore 필터 제거 (강도 조정만)
- [ ] `get_adjusted_strength()` 적용

### 7.3 고정 리스트 전략

- [ ] 자체 모멘텀 계산 로직 완성
- [ ] MacroEnvironment 연동 (해당 시)

### 7.4 동적 Universe 전략

- [ ] GlobalScore 필터 적용 (KR)
- [ ] GlobalScore 강도 조정 적용
- [ ] 스크리닝 함수 연동

---

## 8. 변경 이력

| 버전 | 날짜 | 변경 내용 |
|------|------|----------|
| 1.0.0 | 2026-02-07 | 초기 문서 생성 |

---

## 참고 문서

| 문서 | 위치 | 용도 |
|------|------|------|
| **MEMORY.md** | `~/.claude/projects/.../memory/MEMORY.md` | 작업 메모리 (최신 원칙) |
| **development_rules.md** | `docs/development_rules.md` | 개발 규칙 |
| **todo.md** | `docs/todo.md` | 현재 작업 목록 |
