# trader-core

> 모든 crate가 의존하는 기반 모듈. 도메인 타입, trait 정의.

## 핵심 타입 위치

| 타입 | 파일 | 설명 |
|------|------|------|
| Signal | `src/domain/signal.rs` | 매매 신호 (Entry/Exit/AddToPosition/ReducePosition) |
| StrategyContext | `src/domain/context.rs` | 전략 주입 컨텍스트 (계좌+분석+시장 데이터) |
| MarketData | `src/domain/market_data.rs` | Kline, Ticker, OrderBook |
| GlobalScoreResult | `src/domain/analytics_provider.rs` | 종합 점수 (0~100) |
| RouteState | `src/domain/route_state.rs` | 진입 상태 (ATTACK/ARMED/WAIT/OVERHEAT) |

## 핵심 Trait 위치

| Trait | 파일 | 역할 |
|-------|------|------|
| ExchangeProvider | `src/domain/exchange_provider.rs` | 계좌/포지션/주문 조회 |
| AnalyticsProvider | `src/domain/analytics_provider.rs` | GlobalScore, Screening, RouteState |

## StrategyContext 구조

```
StrategyContext (Arc<RwLock<>>로 전략에 주입)
├── 계좌 데이터 (1~5초 갱신)
│   ├── account, positions, pending_orders, exchange_constraints
├── 분석 데이터 (1~10분 갱신)
│   ├── global_scores, route_states, screening_results
│   ├── structural_features, market_regime, market_breadth, macro_environment
└── 시장 데이터
    └── klines_by_timeframe (멀티 타임프레임 캔들)
```

## 규칙

- 모든 금융 수치는 `rust_decimal::Decimal` 사용
- 시간은 `DateTime<Utc>` 강제 (`Local::now()` 금지)
- 새 도메인 타입 추가 시 NewType 패턴 권장 (`OrderId(String)`, `StrategyId(String)`)
