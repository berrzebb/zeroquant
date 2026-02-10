# Plan: [F] 관측성 & 아키텍처 확장

> 🔵 선행: A~D 안정화 후. 전략 50개+ 또는 고빈도 처리 시 도입.
> 병렬: F1~F4 관측성은 조기 착수 가능, F5~F6 아키텍처는 후반.

## 선행 조건
- A~D 그룹 안정화
- F1~F4는 조기 착수 가능

## 예상 규모
Large

---

## F-1: 분산 트레이싱 (OpenTelemetry)

- [ ] `opentelemetry` + `tracing-opentelemetry` 의존성 추가
- [ ] API → Strategy → Exchange → DB 요청 상관관계 추적
- [ ] Jaeger/Zipkin 연동 설정

## F-2: Collector 헬스 메트릭

- [ ] 수집 성공/실패 카운트, API 할당량 잔여를 `/health/ready` JSON에 포함
- [ ] 수집 주기 이상 감지 시 기존 알림 채널(Telegram/Discord)로 발송

## F-3: DB 연결풀 & 슬로우 쿼리 모니터링

- [ ] 연결풀 사용률(active/idle/max)을 `/health/ready` JSON에 포함
- [ ] `pg_stat_statements` 기반 슬로우 쿼리 자동 감지 + 알림 (Telegram/Discord)
- [ ] Redis `maxmemory` 환경변수화 (`docker-compose.yml`)

## F-4: 에러 트래커 영속화

- [ ] 인메모리 `DashMap` → DB 영속 저장 병행
- [ ] 에러 이력 조회 API + 재시작 후에도 이력 유지

## F-5: Actor Model 전환

- [ ] 전략별 독립 Tokio Task + mpsc 채널 메시지 기반 통신
- [ ] `StrategyContext`의 `Arc<RwLock<>>` 제거 → 전략 로컬 상태
- [ ] 락 경합 벤치마크 (전환 전/후 비교)

## F-6: Event Bus (Pub/Sub)

- [ ] 시스템 이벤트 정의: `MarketEvent`, `SignalEvent`, `OrderEvent`, `SystemAlert`
- [ ] 전략 → `SignalEvent` 발행, `OrderExecutor` 구독 처리
- [ ] Audit Logger, Dashboard 등 신규 컨슈머를 구독만으로 추가

## 관련 파일
- `crates/trader-api/src/`
- `crates/trader-core/src/`
- `crates/trader-collector/`
- `docker-compose.yml`
