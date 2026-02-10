# Plan: [C] 포트폴리오 분석 & 리스크 고도화

> 🟡 선행: B1(보정 데이터), B6(환율 서비스) 완료 필수.
> 병렬: D와 동시 진행 가능. 전체 Rust 구현 (`argmin` 크레이트).

## 선행 조건
- B-1: 기업 이벤트 처리 (보정 데이터)
- B-6: FX 환율 서비스

## 예상 규모
Large

---

## C-1: 포트폴리오 최적화 (Global Optimizer)

- [ ] `trader-analytics/src/optimizer/` 모듈 신규
- [ ] Mean-Variance Optimization — 샤프 비율 최대화 (`argmin`)
- [ ] Risk Parity — 리스크 균등 기여 비중
- [ ] Minimum Variance — 포트폴리오 변동성 최소화
- [ ] 입력: 자산별 기대 수익률 벡터 + 공분산 행렬 (FX 변환 적용)
- [ ] API: `POST /api/v1/portfolio/optimize`, `GET /api/v1/portfolio/efficient-frontier`
- [ ] `AssetAllocation` 전략과 최적 비중 벡터 연동

## C-2: 실시간 VaR (Value at Risk)

- [ ] Parametric VaR — 공분산 행렬 기반 정규분포 가정 (95%, 99%)
- [ ] Historical VaR — TimescaleDB 과거 수익률 시뮬레이션 기반
- [ ] `RiskManager` 파이프라인에 VaR 한도 검증 단계 추가 (`trader-risk/`)
- [ ] VaR 초과 시 신규 진입 강제 차단

## C-3: 섹터/팩터 노출 제한

- [ ] `RiskConfig`에 `max_sector_weight`, `factor_tilt_limit` 필드 추가 (`trader-risk/`)
- [ ] 포트폴리오 레벨 섹터 비중 검증 로직 (`RiskManager::validate_order()` 확장)
- [ ] 특정 팩터(모멘텀, 가치 등) 쏠림 제한

## C-4: 성과 기여도 분석 (Attribution)

- [ ] Brinson Model — 자산배분 효과 vs 종목선정 효과 분해
- [ ] Beta 분석 — 벤치마크(KOSPI/SPY) 대비 민감도 + 상관계수
- [ ] 섹터 기여도 — 섹터 비중 확대/축소로 인한 손익 분해
- [ ] API: `GET /api/v1/portfolio/attribution`

## C-5: 거래 비용 분석 (TCA)

- [ ] `reality_check` 테이블에 `theory_price`, `exec_price`, `slippage_bps` 컬럼 추가
- [ ] Implementation Shortfall 계산: (신호 시점 중간가) - (실제 평균 체결가)
- [ ] Slippage 분류: 호가 공백 손실 vs 통신 지연 손실
- [ ] Market Impact 측정: 주문 직후 호가 변동 분석

## 관련 파일
- `crates/trader-analytics/src/`
- `crates/trader-risk/`
- `crates/trader-core/src/domain/`
- `migrations/`
