# TypeScript 프론트엔드 규칙

## 0. TypeScript 바인딩 (ts-rs) ⭐

> **핵심 원칙**: Rust 타입 → TypeScript 타입 자동 변환으로 API 타입 안전성 확보

### Rust 측 어노테이션

```rust
use ts_rs::TS;

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StrategyResponse {
    pub id: i32,
    pub name: String,
    pub running: bool,
}
```

### 규칙

- 새로운 API 응답 타입은 반드시 `#[derive(TS)]` 추가
- `#[ts(export)]`로 자동 내보내기 활성화
- 생성 파일 위치: `frontend/src/types/generated/`
- 빌드 명령: `cargo test --features ts-binding`

```typescript
// ❌ 수동 타입 정의 금지 (Rust와 동기화 어려움)
interface StrategyResponse { id: number; name: string; running: boolean; }

// ✅ 자동 생성된 타입 사용
import { StrategyResponse } from '@/types/generated/StrategyResponse';
```

## 1. SolidJS 상태 관리

### createStore (복잡한 상태)

```typescript
import { createStore } from 'solid-js/store';

const [state, setState] = createStore<PageState>({
  filter: 'all',
  modals: { add: { open: false }, edit: { open: false, id: null } },
  loading: false,
});
```

### createMemo (계산된 값)

```typescript
const filteredItems = createMemo(() => {
  return items().filter(item => item.status === state.filter);
});
```

## 2. 타입 안전성

### any 사용 금지

```typescript
// ❌ 금지
const data: any = response.data;

// ✅ 명시적 타입 정의
const data: ApiResponse = response.data;
```

### 리터럴 타입 사용

```typescript
type OrderStatus = 'pending' | 'filled' | 'cancelled' | 'rejected';
type OrderSide = 'buy' | 'sell';
type Timeframe = '1m' | '5m' | '15m' | '1h' | '4h' | '1d' | '1w' | '1M';
```

## 3. ESLint 워닝/에러 제로 정책

> 코드 작성 시점부터 ESLint 에러가 0이어야 한다.

### no-unused-vars

```typescript
// ❌ 사용하지 않는 import 금지
import { createSignal, onMount, onCleanup } from 'solid-js'; // onMount, onCleanup 미사용

// ✅ 사용하는 것만 import
import { createSignal } from 'solid-js';

// ✅ 미사용 부분은 _ 접두사
const [_value, setValue] = createSignal(0);

// ✅ catch 에러 변수 미사용 시 생략
} catch { console.log('failed'); }
```

### no-explicit-any

```typescript
// ❌ any 사용 금지
const data: any = response.data;

// ✅ 구체적 타입 또는 unknown 사용
const data: ApiResponse = response.data;

// ✅ 불가피한 경우 Record 사용
function handler(event: Record<string, unknown>) { ... }
```

### eslint-disable 주석 사용 금지

- `// eslint-disable-next-line` 주석으로 에러를 우회하지 않는다
- 코드를 직접 수정하여 근본적으로 해결한다

## 4. 에러 처리

```typescript
<Show when={resource.loading}>
  <LoadingSpinner />
</Show>

<Show when={resource.error}>
  <ErrorBanner message={resource.error.message} onRetry={() => refetch()} />
</Show>

<Show when={resource()}>
  {/* 성공 시 렌더링 */}
</Show>
```
