# SignalProcessor 공통 모듈 설계

> 상태: ✅ SimulatedExecutor 완료 (v0.7.2), 🔄 LiveExecutor 구현 중 (v0.8.0)
> 위치: `crates/trader-execution/src/signal_processor.rs`

## 목표
BacktestEngine과 SimulationEngine에서 중복되는 거래 로직을 공통 trait으로 추출

## 현재 문제
1. 동일한 거래 로직이 두 곳에서 중복 구현
2. 한 쪽 수정 시 다른 쪽 동기화 필요
3. 테스트/검증이 두 배로 필요

## 설계

### 위치
`crates/trader-execution/src/signal_processor.rs`

### 핵심 구조 (구현 완료)

```rust
use rust_decimal::Decimal;
use std::collections::HashMap;
use async_trait::async_trait;

/// Signal 처리 설정
#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub commission_rate: Decimal,      // 0.001 = 0.1%
    pub slippage_rate: Decimal,        // 0.0005 = 0.05%
    pub max_position_size_pct: Decimal, // 0.2 = 20%
    pub max_positions: usize,          // 10
    pub allow_short: bool,             // false
}

/// 포지션 정보 (Grid/Spread 전략 지원)
#[derive(Debug, Clone)]
pub struct ProcessorPosition {
    pub symbol: String,
    pub side: Side,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub entry_time: DateTime<Utc>,
    pub fees: Decimal,
    pub position_id: Option<String>,   // Grid 레벨별 구분
    pub group_id: Option<String>,      // 그룹 청산용
}

/// 거래 결과
#[derive(Debug, Clone)]
pub struct TradeResult {
    pub symbol: String,
    pub side: Side,
    pub quantity: Decimal,
    pub price: Decimal,
    pub commission: Decimal,
    pub slippage: Decimal,
    pub timestamp: DateTime<Utc>,
    pub realized_pnl: Option<Decimal>,
    pub is_partial: bool,
    pub metadata: HashMap<String, String>,
}

/// Signal 처리 trait (핵심 인터페이스)
#[async_trait]
pub trait SignalProcessor: Send + Sync {
    /// Signal 처리 → TradeResult 반환
    async fn process_signal(
        &mut self,
        signal: &Signal,
        current_price: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<TradeResult>, SignalProcessorError>;

    fn balance(&self) -> Decimal;
    fn positions(&self) -> &HashMap<String, ProcessorPosition>;
    fn trades(&self) -> &[TradeResult];
    fn total_commission(&self) -> Decimal;
    fn unrealized_pnl(&self, current_prices: &HashMap<String, Decimal>) -> Decimal;
    fn realized_pnl(&self) -> Decimal;
    fn total_equity(&self, current_prices: &HashMap<String, Decimal>) -> Decimal;
    fn reset(&mut self, initial_balance: Decimal);

    // 그룹 관련 메서드 (Grid/Spread 전략)
    fn positions_by_group(&self, group_id: &str) -> Vec<&ProcessorPosition>;
    fn group_unrealized_pnl(&self, group_id: &str, prices: &HashMap<String, Decimal>) -> Decimal;
}
```

### 구현된 사용 방법

#### SimulatedExecutor (구현 완료)
```rust
use trader_execution::{SignalProcessor, SimulatedExecutor, ProcessorConfig};

// 설정 생성
let config = ProcessorConfig {
    commission_rate: dec!(0.001),
    slippage_rate: dec!(0.0005),
    max_position_size_pct: dec!(0.2),
    max_positions: 10,
    allow_short: false,
};

// SimulatedExecutor 생성
let mut executor = SimulatedExecutor::new(config, dec!(10_000_000));

// Signal 처리
for signal in signals {
    if let Some(trade) = executor.process_signal(&signal, price, timestamp).await? {
        println!("체결: {} {} @ {}", trade.symbol, trade.side, trade.price);
    }
}

// 상태 조회
println!("잔고: {}", executor.balance());
println!("미실현 손익: {}", executor.unrealized_pnl(&current_prices));
println!("실현 손익: {}", executor.realized_pnl());
```

#### BacktestEngine 통합
```rust
pub struct BacktestEngine {
    config: BacktestConfig,
    executor: Box<dyn SignalProcessor>,  // trait object
    tracker: PerformanceTracker,
    equity_curve: Vec<EquityPoint>,
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        let processor_config = ProcessorConfig::from(&config);
        Self {
            config,
            executor: Box::new(SimulatedExecutor::new(
                processor_config,
                config.initial_capital,
            )),
            // ...
        }
    }
}
```

#### 실거래용 LiveExecutor (v0.8.0 구현 중)
```rust
pub struct LiveExecutor {
    config: ProcessorConfig,
    exchange_provider: Arc<dyn ExchangeProvider>,
    positions: HashMap<String, ProcessorPosition>,
    trades: Vec<TradeResult>,
    // ...
}

#[async_trait]
impl SignalProcessor for LiveExecutor {
    async fn process_signal(&mut self, signal: &Signal, ...) -> Result<...> {
        // 1. 거래소 제약조건 확인 (최소 주문량, 호가 단위)
        let constraints = self.exchange_provider.get_constraints(&signal.ticker).await?;

        // 2. 주문 생성 및 실행
        let order = OrderRequest::from_signal(signal, constraints)?;
        let result = self.exchange_provider.place_order(order).await?;

        // 3. 체결 대기 및 상태 업데이트
        self.update_position_from_fill(result)?;
        // ...
    }
}
```

## 구현 상태

### Phase 1: 준비 ✅
- [x] Grid 전략 문제 분석
- [x] Grid 전략 검증 및 수정 (position_id, group_id 지원)
- [x] 시뮬레이션 테스트 프로그램 완성 (sim-test CLI)

### Phase 2: 추출 ✅
- [x] `SignalProcessor` trait 생성 (`trader-execution`)
- [x] `SimulatedExecutor` 구현 (백테스트/시뮬레이션용)
- [x] 단위 테스트 작성 (4개 테스트 통과)

### Phase 3: 통합 ✅
- [x] BacktestEngine이 SignalProcessor 사용하도록 수정
- [x] SimulationEngine이 SignalProcessor 사용하도록 수정
- [x] 기존 테스트 통과 확인

### Phase 4: 정리 ✅
- [x] 중복 코드 제거
- [x] 문서화 (이 문서 + architecture.md)

### Phase 5: CandleProcessor 공통화 ✅

캔들 처리 로직(StrategyContext 업데이트, 시그널 생성, 포지션 동기화)을 BacktestEngine과 SimulationEngine 간 공통화.

- [x] `CandleProcessor` 구조체 생성 (`trader-analytics/src/backtest/candle_processor.rs`)
- [x] BacktestEngine의 루프 내부를 CandleProcessor 호출로 리팩토링
- [x] 레거시 `run()` 메서드 삭제, `run_with_context()` → `run()`으로 통합
- [x] SimulationEngine에 StrategyContext + CandleProcessor 통합
- [x] CLI 호출부 전환 (7곳)

```
CandleProcessor (trader-analytics)
├── update_context()      # StrategyContext 업데이트 (지표, klines, 스크리닝)
├── generate_signals()    # 시그널 생성 (멀티 심볼/멀티 TF + Entry/Exit 파티셔닝)
├── sync_positions()      # 전략에 포지션 상태 동기화
└── process_candle()      # 위 3개를 순차 실행 (편의 메서드)
```

**사용 패턴**:
```
BacktestEngine.run()              SimulationEngine.process_next_candle()
    │                                  │
    ├─ candle_processor               ├─ candle_processor
    │  .update_context()              │  .process_candle()  ← 편의 메서드
    │  .generate_signals()            │
    │                                 │
    ├─ self.process_signal() ←고유    ├─ self.process_signal() ←고유
    │  (PerformanceTracker 기록)      │  (SignalMarker 기록)
    │                                 │
    ├─ candle_processor               ├─ candle_processor
    │  .sync_positions()              │  .sync_positions()
    │                                 │
    └─ self.tracker.update_equity()   └─ self.update_equity_curve()
```

## 장점

1. **일관성**: 두 엔진이 동일한 거래 로직 + 동일한 캔들 처리 로직 사용
2. **유지보수**: StrategyContext 관련 수정 시 CandleProcessor 한 곳만 수정
3. **테스트**: 공통 로직 한 번만 테스트
4. **확장성**: 새로운 엔진 추가 시 재사용 가능

## 주의사항

1. BacktestEngine의 `tracker` 통합 - 성능 추적은 엔진 레벨에서 처리
2. SignalMarker 저장 - 엔진별로 별도 처리
3. 비동기 처리 - TradeExecutor는 동기 메서드로 유지
