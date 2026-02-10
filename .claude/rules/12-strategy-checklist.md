# 전략 추가 체크리스트

새 전략 추가 시 다음 **5곳**을 반드시 수정해야 합니다:

## 1. crates/trader-strategy/src/strategies/mod.rs

```rust
pub mod your_strategy;
pub use your_strategy::*;
```

## 2. crates/trader-api/src/routes/strategies.rs

- `create_strategy_instance()` - 전략 인스턴스 생성
- `get_strategy_default_name()` - 한글 이름
- `get_strategy_default_timeframe()` - 기본 타임프레임
- `get_strategy_default_symbols()` - 권장 심볼

## 3. crates/trader-api/src/routes/backtest/engine.rs

- import 추가
- `run_strategy_backtest()` 또는 `run_multi_strategy_backtest()`

## 4. config/sdui/strategy_schemas.json

- `strategies` 객체에 전략 스키마 추가 (~50줄)

## 5. frontend/src/pages/Strategies.tsx

- `getDefaultTimeframe()` switch 문에 case 추가
