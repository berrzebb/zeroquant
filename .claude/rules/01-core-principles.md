# 핵심 원칙

> 이 원칙들은 모든 코드 작성 시 최우선으로 고려되어야 합니다.

## 1. 레거시 코드 즉시 제거

- 코드 개선 시 불필요하거나 레거시가 된 코드는 **반드시 제거**
- 사용되지 않는 함수/타입/모듈은 즉시 삭제
- 주석 처리 대신 Git 히스토리 활용
- 임시 해결책(TODO, FIXME)은 반드시 이슈 등록 후 제거

```rust
// ❌ 주석 처리된 레거시 코드 금지
// fn old_calculate_price(price: f64) -> f64 { ... }

// ✅ 레거시 완전 제거, 개선된 함수로 대체
fn calculate_price(price: Decimal, tax_rate: Decimal) -> Decimal {
    price * (Decimal::ONE + tax_rate)
}
```

## 2. 거래소 중립적 코드

- 모든 코드는 특정 거래소에 의존하지 않고 **추상화된 인터페이스** 사용
- Binance, KIS, 시뮬레이션 등 다중 거래소 지원을 위해 필수

```rust
// ❌ 특정 거래소에 강결합 금지
pub async fn get_price(symbol: &str) -> f64 {
    binance_client.get_ticker(symbol).await.price
}

// ✅ ExchangeApi trait 사용
pub async fn get_price(
    exchange: &dyn ExchangeApi,
    symbol: &str
) -> Result<Decimal, ExchangeError> {
    exchange.get_ticker(symbol).await.map(|ticker| ticker.price)
}
```

## 3. 이후 작업 고려

- 새 필드 추가 시 마이그레이션 롤백 가능하게 설계
- API 응답 형식 변경 시 버전 관리 고려
- 새 전략 추가 시 백테스트 엔진 확장성 고려
