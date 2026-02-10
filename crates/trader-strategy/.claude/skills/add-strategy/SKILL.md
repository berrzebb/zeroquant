---
name: add-strategy
description: 전략 추가 5곳 수정 체크리스트. 전략명을 인수로 전달.
disable-model-invocation: true
user-invocable: true
argument-hint: "<전략명_snake_case> [전략 설명]"
allowed-tools: Read, Grep, Edit, Write, Bash(cargo *)
context: fork
  agent: rust-impl
---

# 전략 추가 워크플로우

새 전략 `$ARGUMENTS[0]`을 추가합니다.

⚠️ **반드시 5곳을 모두 수정해야 합니다.** 하나라도 빠지면 런타임 에러가 발생합니다.

---

## 0단계: 기존 전략 참조

먼저 가장 유사한 기존 전략을 참고 자료로 읽습니다:

```bash
# 기존 전략 목록 확인
ls crates/trader-strategy/src/strategies/
```

추천 참조 전략: `rsi.rs` (가장 기본적인 구조)

---

## 1단계: 전략 구현 파일 생성

**위치**: `crates/trader-strategy/src/strategies/$ARGUMENTS[0].rs`

### 필수 구현 사항

```rust
use crate::traits::Strategy;
use trader_core::domain::{
    context::StrategyContext,
    signal::Signal,
};
use rust_decimal::Decimal;  // f64 절대 금지!

pub struct YourStrategy {
    // 파라미터 필드 (모두 Decimal)
}

impl Strategy for YourStrategy {
    fn name(&self) -> &str { "your_strategy" }

    fn on_market_data(
        &mut self,
        ctx: &StrategyContext,
    ) -> Vec<Signal> {
        // 전략 로직
        vec![]
    }
}
```

### 체크포인트
- [ ] `rust_decimal::Decimal` 사용 (f64 금지)
- [ ] `unwrap()` 미사용
- [ ] 모든 주석 한글
- [ ] `#[derive(TS)]` + `#[ts(export)]` 추가 (응답 타입이 있으면)

---

## 2단계: 모듈 등록

**위치**: `crates/trader-strategy/src/strategies/mod.rs`

```rust
pub mod $ARGUMENTS[0];
pub use $ARGUMENTS[0]::*;
```

---

## 3단계: API 라우트 연동

**위치**: `crates/trader-api/src/routes/strategies.rs`

3곳을 수정합니다:
1. `create_strategy_instance()` — match 분기에 전략 추가
2. `get_strategy_default_name()` — 한글 이름 반환
3. `get_strategy_default_timeframe()` — 기본 타임프레임
4. `get_strategy_default_symbols()` — 권장 심볼 목록

---

## 4단계: 백테스트 엔진 연동

**위치**: `crates/trader-api/src/routes/backtest/engine.rs`

1. import 추가
2. `run_strategy_backtest()` 또는 `run_multi_strategy_backtest()` match 분기에 추가

---

## 5단계: UI 스키마 및 프론트엔드

### 5-1. SDUI 스키마
**위치**: `config/sdui/strategy_schemas.json`

`strategies` 객체에 전략 스키마 추가 (~50줄):
- 전략 파라미터별 UI 컴포넌트 정의
- `type`, `min`, `max`, `default`, `step` 등

### 5-2. 프론트엔드 타임프레임
**위치**: `frontend/src/pages/Strategies.tsx`

`getDefaultTimeframe()` switch 문에 case 추가

---

## 6단계: 검증

```powershell
# 1. 컴파일 확인
cargo check -p trader-strategy
cargo check -p trader-api

# 2. 테스트 실행
cargo test -p trader-strategy -- $ARGUMENTS[0]

# 3. Clippy
cargo clippy -p trader-strategy -p trader-api -- -D warnings
```

모든 검증 통과 후 사용자에게 결과를 보고합니다.
