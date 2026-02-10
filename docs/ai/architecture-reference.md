# 설계 참조 (에이전트용)

> 원본: `docs/architecture.md` (1185줄), `docs/trade_executor_design.md` (239줄)
> 담당: `rust-impl`, `debugger`, `lead`

## 시스템 흐름

```
MarketData → StrategyEngine → Strategy.on_market_data() → Signal[]
                                                            │
                                                     SignalProcessor
                                               ┌───────────┴───────────┐
                                          SimulatedExecutor       LiveExecutor
```

## Crate 의존성 순서

```
trader-core (기반 — 모든 crate가 의존)
├── trader-exchange     (거래소 연동)
├── trader-strategy     (전략 엔진)
├── trader-execution    (주문 실행)
├── trader-risk         (리스크)
├── trader-data         (데이터 수집/저장)
├── trader-analytics    (백테스트, 분석)
├── trader-notification (알림)
├── trader-api          (REST/WS API)
├── trader-cli          (CLI)
└── trader-collector    (Standalone 수집기)
```

변경 시 의존성: `core` → `exchange/strategy/data` → `execution` → `api`

## 핵심 타입 위치

| 타입 | 위치 |
|------|------|
| Signal | `trader-core/src/domain/signal.rs` |
| MarketData | `trader-core/src/domain/market_data.rs` |
| StrategyContext | `trader-core/src/domain/strategy_context.rs` |
| Strategy trait | `trader-strategy/src/strategy.rs` |
| ExchangeApi trait | `trader-exchange/src/exchange_api.rs` |
| ExchangeProvider | `trader-exchange/src/provider/` |
| OrderExecutionProvider | `trader-exchange/src/provider/<exchange>_exchange.rs` |
| SignalProcessor | `trader-execution/src/signal_processor.rs` |
| AppState | `trader-api/src/state.rs` |
| ApiErrorResponse | `trader-api/src/error.rs` |
| Repository | `trader-data/src/repository/` |

## SignalProcessor 설계

```rust
pub struct ProcessorConfig {
    pub commission_rate: Decimal,       // 0.001
    pub slippage_rate: Decimal,         // 0.0005
    pub max_position_size_pct: Decimal, // 0.2
    pub max_positions: usize,           // 10
    pub allow_short: bool,              // false
}

pub struct ProcessorPosition {
    pub symbol: String,
    pub side: Side,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub position_id: Option<String>,    // Grid 레벨별
    pub group_id: Option<String>,       // 그룹 청산용
}
```

구현체: `SimulatedExecutor` (완료) / `LiveExecutor` (구현 중)

## 거래소 Provider 구조

```
trader-exchange/src/
├── connector/     ← WebSocket/REST 연결 (저수준)
│   ├── upbit.rs
│   ├── bithumb.rs
│   ├── kis.rs (한국투자증권)
│   ├── ls_sec.rs (LS증권)
│   └── db_investment.rs (DB증권)
├── provider/      ← ExchangeApi trait 구현 (고수준)
│   ├── upbit_provider.rs
│   ├── bithumb_provider.rs
│   ├── kis_provider.rs
│   ├── ls_sec_provider.rs
│   ├── db_investment_provider.rs
│   └── mock_exchange.rs
└── market_stream/ ← MarketStream trait (실시간)
```

## 전략 실행 모드

| 모드 | Executor | 데이터 | 용도 |
|------|----------|--------|------|
| 백테스트 | SimulatedExecutor | 과거 OHLCV | 전략 검증 |
| 페이퍼트레이딩 | SimulatedExecutor + Mock거래소 | 실시간 | 리스크 없는 테스트 |
| 실거래 | LiveExecutor + ExchangeProvider | 실시간 | 실전 운용 |

## StrategyContext 통합 아키텍처

```rust
pub struct StrategyContext {
    pub exchange: Arc<dyn ExchangeProvider>,
    pub analytics: Arc<dyn AnalyticsProvider>,
    pub route_state: Option<RouteState>,
    pub global_score: Option<Decimal>,
    pub market_breadth: Option<MarketBreadth>,
    pub macro_env: Option<MacroEnvironment>,
    pub structural: Option<StructuralFeatures>,
}
```
