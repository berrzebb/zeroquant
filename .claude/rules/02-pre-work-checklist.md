# 작업 전 필수 확인

## 라이브러리 API 검증 (Context7 사용)

> **핵심 원칙**: 학습 데이터 기반 추측으로 코드 작성 금지

1. `resolve-library-id`로 라이브러리 ID 획득
2. `query-docs`로 구체적인 API 패턴 조회
3. 버전 확인: `Cargo.toml`, `package.json`

### 주의해야 할 라이브러리

- **Tokio**: select!, spawn, channel API 변경 빈번
- **Axum**: 0.6 → 0.7에서 Router, State API 변경됨
- **SQLx**: query!, query_as! 매크로 동작 확인 필요
- **SolidJS**: reactivity 패턴 확인

### ❌ 금지 사항

1. **버전 미확인 코드 작성**: 반드시 Context7에서 확인한 버전 명시
2. **Deprecated API 사용**: 현재 권장 API를 Context7/공식 문서에서 확인 후 사용
3. **추측 기반 import 경로**: 실제 코드베이스 또는 docs.rs에서 확인
4. **Feature flag 미확인 사용**: `Cargo.toml`의 features 섹션 확인 후 사용

## 코드 탐색 도구 우선순위

> **핵심 원칙**: 코드베이스 탐색 시 Serena MCP의 semantic tools를 우선 사용

### Serena 우선 사용 (심볼 기반 탐색)

```
find_symbol → 심볼 검색
find_referencing_symbols → 참조 탐색
get_symbols_overview → 파일 심볼 개요
```

### Grep 제한적 사용 (문자열 패턴 매칭)

Grep은 다음 경우에만 사용:
1. 로그 메시지나 에러 메시지 검색
2. 특정 문자열 리터럴 찾기
3. 정규표현식 패턴 매칭이 필수인 경우

## UI-API 필드 매칭

> 모든 작업 시 UI와 API 필드 매칭을 반드시 확인

```typescript
// 프론트엔드 타입과 백엔드 응답이 일치해야 함
interface BacktestResult {
  total_return: number;  // API 응답 필드명
  sharpe_ratio: number;
}
```
