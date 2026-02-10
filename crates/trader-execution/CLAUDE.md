# trader-execution

> Signal → 주문 실행. SignalProcessor trait 기반.

## 핵심 구조

| 타입 | 파일 | 역할 |
|------|------|------|
| SignalProcessor | `src/signal_processor.rs` | Signal → 주문 실행 추상화 trait |
| LiveExecutor | `src/live_executor.rs` | 실거래 주문 실행 (1,026줄) |
| SimulatedExecutor | `src/simulated_executor.rs` | 백테스트/페이퍼 체결 시뮬레이션 |
| TradeResult | `src/signal_processor.rs` | 체결 결과 (수량, 가격, 수수료, 실현손익) |
| ProcessorPosition | `src/signal_processor.rs` | 포지션 (position_id, group_id 포함) |

## 실행 모드

| 모드 | Executor | 데이터 소스 |
|------|----------|------------|
| 실거래 | LiveExecutor | ExchangeProvider |
| 페이퍼 트레이딩 | SimulatedExecutor | ExchangeProvider |
| 백테스트 | SimulatedExecutor | BacktestEngine |

## 규칙

- 주문 멱등성 보장 (request_id 기반 중복 방지)
- `Decimal` 타입 필수 (수량, 가격, 수수료 모두)
- `OrderExecutionProvider` trait 호출 (거래소 직접 호출 금지)
