# 테스트 규칙

## 1. 테스트 파일 위치

```
crates/trader-strategy/src/strategies/
  rsi.rs
  rsi_test.rs     # 또는 mod.rs 내 #[cfg(test)]

tests/
  integration/
    backtest_test.rs
    api_test.rs
```

## 2. 테스트 명명

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_should_generate_buy_signal_when_rsi_below_threshold() {
        // Given: RSI가 30 이하인 상황
        // When: 신호 생성 호출
        // Then: 매수 신호 반환
    }
}
```

## 3. 테스트에서만 unwrap() 허용

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_something() {
        let result = some_function().unwrap();  // 테스트에서는 허용
        assert_eq!(result, expected);
    }
}
```

## 4. 브라우저 MCP 테스트 규칙

> Playwright MCP와 Chrome DevTools MCP는 역할이 다르다. 혼용 금지.

| 상황 | 사용할 MCP | 이유 |
|------|-----------|------|
| E2E 테스트 작성/실행 | **Playwright** | 크로스브라우저, accessibility tree 기반 안정적 셀렉터 |
| UI 컴포넌트 동작 검증 | **Playwright** | snapshot → click → assert 패턴 |
| 페이지 성능 분석 | **Chrome DevTools** | Core Web Vitals, 렌더 트레이스 |
| API 네트워크 디버깅 | **Chrome DevTools** | 헤더/페이로드/타이밍 분석 |
| 콘솔 에러 확인 | **Chrome DevTools** | 전체 콘솔 로그 접근 |
| CI/CD 파이프라인 | **Playwright** | headless 모드 지원 |

### Playwright MCP 사용 패턴

```
1. browser_navigate → http://localhost:5173/페이지경로
2. browser_snapshot → 현재 DOM 상태 확인 (accessibility tree)
3. browser_click / browser_fill_form → 사용자 인터랙션
4. browser_snapshot → 결과 상태 검증
5. browser_take_screenshot → 시각적 확인 (필요 시)
```

### Chrome DevTools MCP 사용 패턴

```
1. navigate_page → http://localhost:5173/페이지경로
2. performance_start_trace → 성능 측정 시작
3. (사용자 인터랙션 수행)
4. performance_stop_trace → 트레이스 종료
5. performance_analyze_insight → Core Web Vitals 분석
6. list_network_requests → API 호출 분석
```

### 금지 사항

- ❌ 단순 E2E 테스트에 Chrome DevTools 사용 (토큰 낭비: 19k vs 13.7k)
- ❌ 성능 프로파일링에 Playwright 사용 (trace 기능 없음)
- ❌ 두 MCP를 같은 세션에서 동시에 활성화 (브라우저 포트 충돌 가능)
