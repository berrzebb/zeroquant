# Plan: [G] 프론트엔드 & UX

> 🟢 전 구간 병행 가능. 백엔드 작업과 독립.

## 선행 조건
- 없음 (전 구간 병행 가능)

## 예상 규모
Medium

---

## G-1: 차트 시각화 강화

- [ ] RouteState 구간 시각화 — ATTACK/WAIT/OVERHEAT 배경색 밴드 렌더링
- [ ] 비매매 지표 마커 — RSI 과매수/과매도(•), Golden/Dead Cross(x), TTM Squeeze(Bar)
- [ ] 캔들 패턴 라벨 — 48개 패턴 약어(H, E, D) 캔들 위 오버레이
- [ ] 줌 레벨 기반 마커 필터링 — Zoom Out 시 매매만, Zoom In 시 보조 마커 표시

## G-2: 툴팁 & 인터랙션 강화

- [ ] 매매 마커 호버 시 RSI/MACD/RouteState/Score 컨텍스트 툴팁
- [ ] `SignalDetailPopup` 확장 — 진입 근거 + 당시 지표 값 표시

## G-3: 시스템 UX 개선

- [ ] UTC → KST 타임존 변환 유틸리티 + 사용자 설정 연동
- [ ] 다크/라이트 테마 토글 (TailwindCSS `dark:` 활성화 + `localStorage`)
- [ ] 페이지 레벨 `<ErrorBoundary>` + 로딩 스켈레톤 시스템
- [ ] 모바일 반응형 레이아웃 검증 + 뷰포트 대응
- [ ] 스키마 없는 전략 fallback UI (JSON 에디터)
- [ ] 브라우저 호환성 테스트 (Chrome, Firefox, Safari)

## 관련 파일
- `frontend/src/`
- `frontend/src/pages/`
- `frontend/src/components/`
