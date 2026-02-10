# trader-analytics

> 백테스트 엔진, GlobalScore, 성과 분석, ML 특성 계산

## 주요 모듈

| 모듈 | 역할 |
|------|------|
| `src/backtest/engine.rs` | BacktestEngine — CandleProcessor 기반 |
| `src/scoring/global_scorer.rs` | GlobalScore 계산 (0~100) |
| `src/scoring/route_state_calculator.rs` | RouteState 판정 (ATTACK/ARMED/WAIT/OVERHEAT) |
| `src/indicators/` | 기술 지표 (RSI, MACD, Bollinger, Keltner, VWAP 등) |
| `src/structural_features.rs` | 구조적 특성 (IndicatorEngine 기반) |

## 백테스트 흐름

```
BacktestEngine
  → CandleProcessor (캔들 전처리)
  → Strategy.on_market_data() (전략 호출)
  → SimulatedExecutor (체결 시뮬레이션)
  → TradeResult[] (결과 수집)
```

## 규칙

- 백테스트 시 Look-ahead bias 방지 (미래 데이터 참조 금지)
- 모든 지표 계산은 `Decimal` 기반
- 새 지표 추가 시 `src/indicators/` 하위에 모듈 추가
