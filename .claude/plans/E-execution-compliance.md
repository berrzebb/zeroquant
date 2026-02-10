# Plan: [E] 실행 계층 & 컴플라이언스

> 🟠 E1~E3: AUM 증가 시 단계적 도입. E4~E6: 라이브 운영 시작과 함께 도입.
> 병렬: C, D와 독립 진행 가능.

## 선행 조건
- E1~E3: AUM 기반 판단
- E4~E6: 라이브 운영 시작 시

## 예상 규모
Medium-Large

---

## E-1: 스마트 주문 집행 (Algo Execution)

- [ ] `ExecutionAlgo` trait 정의 (`trader-execution/src/algo/`)
- [ ] TWAP — 시간 분할 매매 (`duration`, `slice_count`)
- [ ] Iceberg — 빙산 주문 (`visible_qty`, `variance`)
- [ ] POV — 거래량 연동 (`participation_rate`)
- [ ] Parent Order → Child Order 분할 + 순차 전송 로직

## E-2: 내부 상계 시스템 (Internal Netting)

- [ ] 중앙 `OrderManager` 신규 — 전략별 신호 주기적 수집 (예: 1분)
- [ ] 동일 심볼 매수/매도 상계 처리 후 순 주문만 거래소 전송
- [ ] 상계 로그 기록 (절감 수수료·슬리피지 추적)

## E-3: Smart Order Router

- [ ] 전략 → `Intent` (무엇을, 몇 주, 긴급도) 발행
- [ ] SOR → `Intent` → 실제 `Order[]` 변환 (알고리즘 선택·분할)
- [ ] `LiveExecutor`에서 의사결정/집행 로직 분리

## E-4: 불변 감사 로그 (Audit Trail)

- [ ] `audit_log` append-only 테이블 (INSERT만 허용, UPDATE/DELETE 차단)
- [ ] 모든 주문 생성·체결·취소 이벤트 자동 기록
- [ ] 감사 로그 조회 API: `GET /api/v1/audit/trades`

## E-5: 세금 Lot 추적

- [ ] FIFO/LIFO/특정 Lot 지정 방식의 취득원가 계산 모듈 (`trader-analytics/`)
- [ ] 기존 `GET /api/v1/journal/cost-basis/{symbol}` 확장
- [ ] 연간 양도소득세 리포트 생성 API

## E-6: 전략 상태 영속화 (Graceful Shutdown)

- [ ] `StrategyState` 직렬화 → DB/파일 저장 (`on_shutdown` 훅)
- [ ] DCA 그리드 레벨, 트레일링 스톱 고점, 인메모리 상태 대상
- [ ] 재시작 시 마지막 저장 상태에서 복원

## 관련 파일
- `crates/trader-execution/src/`
- `crates/trader-analytics/src/`
- `crates/trader-api/src/routes/`
- `migrations/`
