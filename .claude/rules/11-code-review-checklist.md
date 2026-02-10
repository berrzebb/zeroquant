# 코드 리뷰 체크리스트

## Pull Request 체크리스트

### 기본 검증

- [ ] 모든 테스트 통과 (`cargo test`)
- [ ] Clippy 경고 없음 (`cargo clippy -- -D warnings`)
- [ ] 포맷팅 준수 (`cargo fmt -- --check`)
- [ ] 빌드 성공 (`cargo build --release`)

### 코드 품질

- [ ] `unwrap()` 사용 없음 (테스트 제외)
- [ ] 레거시 코드 제거 완료
- [ ] 주석은 한글로 작성
- [ ] 공개 API에 Rustdoc 주석 추가

### 아키텍처

- [ ] 거래소 중립적 코드 (trait 사용)
- [ ] Repository 패턴 준수
- [ ] 에러 타입 명확히 정의
- [ ] 비동기 작업 적절히 처리

### 보안

- [ ] API 키 하드코딩 없음
- [ ] 민감 정보 로깅 없음
- [ ] 입력 검증 적용
- [ ] SQL Injection 방지 (prepared statement)

### 금융 계산

- [ ] `Decimal` 타입 사용 (f64 금지)
- [ ] UTC 타임스탬프 사용
- [ ] Idempotency 보장 (필요시)

### 테스트

- [ ] 단위 테스트 추가
- [ ] 엣지 케이스 테스트
- [ ] 금융 계산은 property-based 테스트 (proptest)

### 문서

- [ ] README 업데이트 (필요시)
- [ ] API 문서 업데이트 (docs/api.md)
- [ ] CHANGELOG 업데이트

> 빌드/린트 검증 명령어는 `validator` 에이전트 참조
