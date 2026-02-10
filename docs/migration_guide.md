# 마이그레이션 관리 가이드

> 마지막 업데이트: 2026-02-10
> 담당 에이전트: `db-reviewer` | 참조: `rust-impl`, `lead`

## Quick Reference

> CLI 바이너리: `target/release/trader.exe` (사전 빌드됨, cargo build 불필요)
> 빌드 에러 시에만: `cargo build --release -p trader-cli`

```bash
# 검증 (마이그레이션 변경 시 필수)
./target/release/trader.exe migrate verify --verbose

# 의존성 그래프
./target/release/trader.exe migrate graph --format text

# 통합 미리보기
./target/release/trader.exe migrate consolidate --dry-run

# 적용 상태 확인
./target/release/trader.exe migrate status --db-url "postgres://trader:trader@localhost:5432/trader"
```

---

## CLI 명령어

### verify — 마이그레이션 검증

```bash
./target/release/trader.exe migrate verify                    # 기본 검증
./target/release/trader.exe migrate verify --verbose          # 상세 출력
./target/release/trader.exe migrate verify --dir migrations_v2  # v2 디렉토리
```

| 코드 | 의미 | 심각도 | 조치 |
|------|------|--------|------|
| `DUP001` | 중복 정의 | 🔴 Critical | 최신 정의만 남기고 제거 |
| `CASC001` | CASCADE 사용 | 🔴 Critical | 명시적으로 의존 객체 먼저 삭제 |
| `CIRC001` | 순환 의존성 (A→B→C→A) | 🔴 Critical | 의존성 구조 재설계 |
| `DCPAT001` | DROP 후 CREATE | 🔴 Critical | ALTER 또는 IF NOT EXISTS 사용 |
| `DATA001/002/003` | 데이터 안전성 경고 | 🟡 Warning | 백업 후 진행 |
| `IDEM001` | IF NOT EXISTS 누락 | 🟡 Warning | `CREATE TABLE IF NOT EXISTS`로 변경 |
| `IDEM002` | IF EXISTS 누락 | 🟡 Warning | `DROP TABLE IF EXISTS`로 변경 |

### consolidate — 마이그레이션 통합

```bash
./target/release/trader.exe migrate consolidate --dry-run           # 미리보기
./target/release/trader.exe migrate consolidate --output migrations_v2  # 실제 통합
```

통합 그룹:

| # | 파일 | 내용 |
|---|------|------|
| 01 | core_foundation | Extensions, ENUM, symbols, credentials |
| 02 | data_management | symbol_info, ohlcv, fundamental |
| 03 | trading_analytics | trade_executions, position_snapshots, 뷰 |
| 04 | strategy_signals | signal_marker, alert_rule, alert_history |
| 05 | evaluation_ranking | global_score, reality_check |
| 06 | user_settings | watchlist, preset, notification |
| 07 | performance_optimization | 인덱스, MV, Hypertable |
| 08 | paper_trading | Mock 거래소, 전략-계정, 미체결 주문 |
| 09 | strategy_watched_tickers | 전략별 관심 종목, Collector 연동 |
| 10 | symbol_cascade | 연쇄 삭제 + 고아 데이터 정리 함수 |

### graph — 의존성 그래프

```bash
./target/release/trader.exe migrate graph --format text             # 텍스트
./target/release/trader.exe migrate graph > dependency.md           # Mermaid (기본)
./target/release/trader.exe migrate graph --format dot > dep.dot    # Graphviz DOT
```

### apply — 마이그레이션 적용

```bash
# 테스트 DB에서 먼저 검증 (필수)
./target/release/trader.exe migrate apply --db-url "postgres://test:test@localhost/test_db" --dir migrations_v2

# 운영 적용
./target/release/trader.exe migrate apply --dir migrations_v2
```

> ⚠️ apply는 자동으로 verify를 먼저 실행. 에러 시 중단됨.

### status — 적용 상태

```bash
./target/release/trader.exe migrate status --db-url "postgres://trader:trader@localhost:5432/trader"
```

---

## 데이터 안전 원칙

### 통합 마이그레이션 안전 장치
1. **IF NOT EXISTS**: 이미 존재하는 객체 건너뜀
2. **DROP 제외**: 통합 시 DROP 문장 자동 제외
3. **OR REPLACE**: 뷰는 안전하게 재생성

### 스키마 변경 절차

```bash
# 1. 스키마 백업
podman exec trader-timescaledb pg_dump -U trader -s trader > schema_backup.sql

# 2. 데이터 백업 (필요 시)
podman exec trader-timescaledb pg_dump -U trader -t symbols -t ohlcv trader > data_backup.sql

# 3. 테스트 적용
trader migrate apply --db-url "postgres://test:test@localhost/test_db" --dir migrations_v2

# 4. 스키마 비교 후 운영 적용
trader migrate apply --dir migrations_v2
```

### 롤백

```bash
podman exec -i trader-timescaledb psql -U trader -d trader < schema_backup.sql
podman exec -i trader-timescaledb psql -U trader -d trader < data_backup.sql
```

---

## 체크리스트

### 적용 전
- [ ] `./target/release/trader.exe migrate verify` 통과
- [ ] 데이터베이스 백업 완료
- [ ] 테스트 DB에서 먼저 실행
- [ ] CASCADE 사용 부분 영향 분석

### 적용 후
- [ ] 모든 테이블 접근 가능
- [ ] 주요 쿼리 정상 동작
- [ ] 애플리케이션 정상 작동

---

## Rust API 참조

```rust
use trader_core::migration::{
    MigrationAnalyzer,        // SQL 파싱 및 의존성 분석
    MigrationValidator,       // 검증 수행
    MigrationConsolidator,    // 통합 계획 생성
    generate_safety_checklist, // 안전 체크리스트 생성
};

let analyzer = MigrationAnalyzer::new();
let files = analyzer.scan_directory(Path::new("migrations"))?;
let validator = MigrationValidator::new(&files);
let report = validator.validate();
```

---

## 관련 문서

- [아키텍처](./architecture.md)
- [개발 규칙](./development_rules.md)
- [설치/배포 가이드](./setup_guide.md)
- [DB 규칙](../.claude/rules/05-database.md)
