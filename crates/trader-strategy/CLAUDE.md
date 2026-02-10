# trader-strategy

> 16개 전략 구현 + Strategy trait + Registry

## 전략 등록 시스템

```
src/strategies/{name}.rs     — 전략 구현 파일
src/strategies/mod.rs        — 모듈 등록
src/registry.rs              — register_strategy! 매크로
src/traits.rs                — Strategy trait 정의
src/strategies/common/       — exit_config, indicators, position_sizing
```

## Strategy trait

```rust
pub trait Strategy {
    fn name(&self) -> &str;
    fn on_market_data(&mut self, ctx: &StrategyContext) -> Vec<Signal>;
    fn exit_config(&self) -> Option<ExitConfig> { None }
}
```

## 16개 전략 목록

AssetAllocation, CandlePattern, CompoundMomentum, DayTrading,
DCA(Grid/MagicSplit/InfinityBot), MarketBothside, MeanReversion,
MomentumPower, MomentumSurge, PensionBot, RangeTrading,
Rotation, RsiMultiTf, ScreeningBased, SectorVb, Us3xLeverage

## StrategyContext 활용 원칙

- **GlobalScore**: 고정 심볼 전략 → 미사용 / 동적 Universe 전략 → 스크리닝 필터
- **RouteState**: 전 전략 공통 Overheat만 차단
- 상세: `docs/STRATEGY_GUIDE.md`

## 전략 추가 시 필수 수정 5곳

1. `src/strategies/{name}.rs` — 전략 구현
2. `src/strategies/mod.rs` — 모듈 등록
3. `trader-api/src/routes/strategies.rs` — API 연동
4. `trader-api/src/routes/backtest/engine.rs` — 백테스트 연동
5. `config/sdui/strategy_schemas.json` + `frontend/src/pages/Strategies.tsx` — UI

> 자동화: `/add-strategy` 스킬 사용 권장
