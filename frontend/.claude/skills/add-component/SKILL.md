---
name: add-component
description: SolidJS 컴포넌트/페이지 생성. ts-rs 바인딩, 에러 처리, 반응형 패턴 포함.
disable-model-invocation: true
user-invocable: true
argument-hint: "<컴포넌트명> [page|component|feature]"
allowed-tools: Read, Grep, Edit, Write, Bash(npm *), Bash(npx *)
context: fork
  agent: ts-impl
---

# SolidJS 컴포넌트 추가 워크플로우

`$ARGUMENTS[0]`을 생성합니다.

---

## 0단계: 타입 분류

| 인수 | 위치 | 설명 |
|------|------|------|
| `page` | `src/pages/` | 라우트 페이지 (lazy import) |
| `component` | `src/components/` | 재사용 공유 컴포넌트 |
| `feature` | `src/features/` | 도메인 기능 모듈 |

(미지정 시 `component`로 기본 처리)

---

## 1단계: 컴포넌트 파일 생성

### Page인 경우

**위치**: `src/pages/$ARGUMENTS[0].tsx`

```tsx
import { createResource, Show } from "solid-js";
import type { MyType } from "@/types/generated/MyType";

export default function $ARGUMENTS[0]Page() {
  const [data] = createResource(fetchData);

  return (
    <div class="p-4">
      <h1 class="text-xl font-bold mb-4">페이지 제목</h1>
      <Show when={data.loading}>
        <div class="animate-pulse">로딩 중...</div>
      </Show>
      <Show when={data.error}>
        <div class="text-red-500">에러: {data.error.message}</div>
      </Show>
      <Show when={data()}>
        {/* 데이터 렌더링 */}
      </Show>
    </div>
  );
}

async function fetchData(): Promise<MyType> {
  const res = await fetch("/api/v1/...");
  if (!res.ok) throw new Error(`API 에러: ${res.status}`);
  return res.json();
}
```

### Component인 경우

**위치**: `src/components/$ARGUMENTS[0].tsx`

```tsx
import type { Component } from "solid-js";

interface $ARGUMENTS[0]Props {
  // props 정의
}

export const $ARGUMENTS[0]: Component<$ARGUMENTS[0]Props> = (props) => {
  return (
    <div>
      {/* 컴포넌트 내용 */}
    </div>
  );
};
```

---

## 2단계: 라우트 등록 (Page인 경우)

**위치**: `src/App.tsx`

```tsx
const $ARGUMENTS[0]Page = lazy(() => import("./pages/$ARGUMENTS[0]"));

// Route 추가
<Route path="/<route>" component={$ARGUMENTS[0]Page} />
```

---

## 3단계: ts-rs 타입 확인

API 연동이 필요한 경우:

```bash
# Rust 측 타입에 #[derive(TS)] #[ts(export)] 확인
grep -r "pub struct.*Response" crates/trader-api/src/routes/

# 바인딩 생성
cargo test -p trader-api export_bindings
cp crates/trader-api/bindings/*.ts frontend/src/api/types/generated/
```

⚠️ `src/types/generated/` 파일을 수동 편집하지 마세요.

---

## 4단계: 검증

```powershell
cd frontend
npx tsc --noEmit
npx eslint src --max-warnings 0
npm run build
```

### 체크포인트
- [ ] `@/types/generated/` import (수동 타입 정의 금지)
- [ ] `any` 타입 미사용
- [ ] `<Show>` / `<For>` 제어 흐름 사용
- [ ] `<ErrorBoundary>` 또는 에러 상태 처리
- [ ] 한글 주석
- [ ] Tailwind CSS 유틸리티 클래스
