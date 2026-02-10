# 전략 참조 (에이전트용)

> 원본: `docs/STRATEGY_GUIDE.md` (678줄) | 담당: `rust-impl`
> Strategy trait: `crates/trader-strategy/src/strategies/`

## 심볼 유형 분류

| 유형 | GlobalScore 활용 | 전략 |
|------|-----------------|------|
| **단일 티커** | 강도 조정만 (0.75~1.25배) | RSI, Grid, Bollinger, MagicSplit, InfinityBot, Candle, VolBreakout, DayTrading, MomentumSurge, RangeTrading, RSI-MultiTF |
| **고정 리스트** | 미사용 (자체 모멘텀) | HAA, AllWeather, PensionBot, CompoundMomentum, XAA |
| **동적 Universe** | 필터 + 강도 조정 | StockRotation(KR), SmallCapQuant |

## StrategyContext 데이터

| 데이터 | 전 전략 | 기술적 | 자산배분 |
|--------|---------|--------|---------|
| RouteState (Overheat 차단) | ✅ | ✅ | - |
| GlobalScore (강도 조정) | 유형별 | ✅ | - |
| MarketBreadth | 선택적 | - | - |
| MacroEnvironment | - | - | ✅ |
| StructuralFeatures | - | ✅ | - |

## GlobalScore 강도 배율

```
score >= 90 → ×1.25 | >= 80 → ×1.15 | >= 70 → ×1.00 | >= 60 → ×0.85 | < 60 → ×0.75
```

## 전략 목록 (16개)

### 단일 티커 (11개)

| 전략 | 핵심 로직 | 주요 파라미터 |
|------|----------|--------------|
| RSI Mean Reversion | RSI 14 과매수/과매도 | period:14, oversold:30, overbought:70 |
| Bollinger Bands | BB(20, 2σ) 이탈/복귀 | period:20, std_dev:2.0 |
| Grid Trading | 그리드 레벨별 분할매매 | spacing_pct:1.0, levels:10 |
| Magic Split | 하락 시 레벨별 분할진입 | split_levels:10, drop_pct:3.0 |
| Infinity Bot | 50라운드 무한매수 | max_rounds:50, round_pct:2.0 |
| Candle Pattern | 35개 캔들 패턴 감지 | min_score:0.6, confirmation:1 |
| Volatility Breakout | 전일 변동성 K 돌파 | k_factor:0.5, lookback:20 |
| Day Trading | 단기 모멘텀 + RSI | rsi_period:14, momentum:10 |
| Momentum Surge | 급등 모멘텀 감지 | surge_threshold:3.0, volume_mult:2.0 |
| Range Trading | 박스권 상하단 매매 | range_period:20, upper:80, lower:20 |
| RSI Multi-TF | 다중 타임프레임 RSI | timeframes:[1h,4h,1d], confirmation:2 |

### 고정 리스트 (3개)

| 전략 | 실행 주기 | 자산 |
|------|----------|------|
| HAA | 월 1회 | 카나리아: TIP → 공격: SPY,IWM,VEA,VWO,TLT,IEF,PDBC,VNQ → 방어: IEF,BIL |
| AllWeather | 분기 1회 | SPY 30%, TLT 40%, IEF 15%, GLD 7.5%, DBC 7.5% |
| PensionBot | 월 1회 | 연금저축 ETF (4계절 + 모멘텀) |

### 동적 Universe (2개)

| 전략 | 핵심 로직 | 종목 선택 |
|------|----------|----------|
| StockRotation(KR) | 국내주식 섹터 로테이션 | GlobalScore Top-N 스크리닝 |
| SmallCapQuant | 소형주 퀀트 | 다중팩터 스크리닝 |

## Position ID 시스템

Grid/Split/InfinityBot에서 사용:
```
position_id: "{ticker}_{strategy}_L{level}"
group_id: "{strategy}_{base_price}_{timestamp}"
```

## Strategy trait 구현 필수

```rust
#[async_trait]
impl Strategy for MyStrategy {
    fn name(&self) -> &str;
    fn strategy_type(&self) -> StrategyType;
    async fn on_market_data(&mut self, ctx: &StrategyContext, data: &MarketData) -> Vec<Signal>;
    fn get_params(&self) -> serde_json::Value;
    fn update_params(&mut self, params: serde_json::Value) -> Result<()>;
}
```

Registry 등록: `crates/trader-strategy/src/registry.rs`
