# Frontend — SolidJS + TypeScript

> Vite 개발 서버 (포트 5173). API 서버 (포트 3000)와 통신.

## 디렉터리 구조

```
src/
├── pages/              # 페이지 컴포넌트 (11개, Lazy Loading)
├── components/         # 공유 컴포넌트 (차트, 테이블, 모달 등)
├── stores/             # SolidJS store (상태 관리)
├── api/                # API 클라이언트
├── types/
│   └── generated/      # ts-rs 자동 생성 타입 (수동 편집 금지)
└── utils/              # 유틸리티
```

## 핵심 규칙

- **ts-rs 바인딩**: API 응답 타입은 `@/types/generated/`에서 import. 수동 타입 정의 금지
- **any 금지**: 구체적 타입 또는 `unknown` 사용
- **ESLint 제로**: 미사용 import/변수 금지, eslint-disable 금지
- **SolidJS 패턴**: `createStore` (복잡한 상태), `createMemo` (계산된 값), `createResource` (비동기)

## 상태 관리

```typescript
// createStore (복잡한 상태)
const [state, setState] = createStore<PageState>({ ... });

// createMemo (계산된 값)
const filtered = createMemo(() => items().filter(...));
```

## 에러 처리

```typescript
<Show when={resource.loading}><LoadingSpinner /></Show>
<Show when={resource.error}><ErrorBanner message={resource.error.message} /></Show>
<Show when={resource()}>{/* 성공 */}</Show>
```

## 빌드/검증

```bash
npx eslint src --max-warnings 0    # 린트
npx tsc --noEmit                   # 타입 체크
npm run build                      # 프로덕션 빌드
```

## E2E 테스트 & 브라우저 MCP

| 목적 | MCP | 에이전트 |
|------|-----|----------|
| E2E 테스트, UI 동작 검증 | **Playwright** | `ts-impl` |
| 성능 프로파일링, 네트워크 분석 | **Chrome DevTools** | `debugger` |

- E2E 테스트 파일: `e2e/` (Playwright 설정: `playwright.config.ts`)
- baseURL: `http://localhost:5173`
- 성능 이슈 → `debugger` 에이전트에 Chrome DevTools MCP 위임

> 자동화: `/add-component` 스킬 사용 권장
