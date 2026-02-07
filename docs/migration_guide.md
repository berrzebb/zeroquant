# 마이그레이션 관리 가이드

> 마지막 업데이트: 2026-02-07

## 개요

ZeroQuant는 Rust 기반 마이그레이션 분석 및 검증 도구를 제공합니다. 이 도구를 사용하여:

- 마이그레이션 파일의 중복/CASCADE/순환 의존성 검출
- 여러 마이그레이션을 논리적 그룹으로 통합
- 데이터 안전성을 보장하면서 마이그레이션 적용

## CLI 명령어

### 검증 (verify)

현재 마이그레이션 파일의 문제점을 검출합니다.

```bash
# 기본 검증
trader migrate verify

# 상세 출력
trader migrate verify --verbose

# 다른 디렉토리 검증
trader migrate verify --dir migrations_v2
```

**검출 항목:**
- `DUP001`: 중복 정의 (같은 객체가 여러 파일에서 정의)
- `CASC001`: CASCADE 사용 (의존 객체 자동 삭제 위험)
- `CIRC001`: 순환 의존성 (A→B→C→A)
- `IDEM001/002`: IF NOT EXISTS / IF EXISTS 누락
- `DCPAT001`: DROP 후 CREATE 패턴 (데이터 손실 위험)
- `DATA001/002/003`: 데이터 안전성 경고

### 통합 (consolidate)

여러 마이그레이션 파일을 논리적 그룹으로 통합합니다.

```bash
# Dry-run (실제 파일 생성 없이 미리보기)
trader migrate consolidate --dry-run

# 실제 통합 (migrations_v2 디렉토리에 생성)
trader migrate consolidate --output migrations_v2
```

**통합 그룹:**
| # | 파일 | 내용 |
|---|------|------|
| 01 | core_foundation | Extensions, ENUM, symbols, credentials |
| 02 | data_management | symbol_info, ohlcv, fundamental |
| 03 | trading_analytics | trade_executions, position_snapshots, 뷰 |
| 04 | strategy_signals | signal_marker, alert_rule, alert_history |
| 05 | evaluation_ranking | global_score, reality_check |
| 06 | user_settings | watchlist, preset, notification |
| 07 | performance_optimization | 인덱스, MV(screening, sector_rs), Hypertable |
| 08 | paper_trading | Mock 거래소, 전략-계정 연결, Paper Trading 세션 |
| 09 | strategy_watched_tickers | 전략별 관심 종목, Collector 우선순위 연동 |
| 10 | symbol_cascade | Symbol 연쇄 삭제 + 고아 데이터 정리 DB 함수 |

### 의존성 그래프 (graph)

마이그레이션 간 의존성을 시각화합니다.

```bash
# Mermaid 다이어그램 (기본)
trader migrate graph > dependency.md

# DOT 형식 (Graphviz)
trader migrate graph --format dot > dependency.dot

# 텍스트 형식
trader migrate graph --format text
```

### 적용 (apply)

통합 마이그레이션을 데이터베이스에 적용합니다.

```bash
# 테스트 DB에서 검증
trader migrate apply --db-url "postgres://test:test@localhost/test_db" --dir migrations_v2

# 환경변수 사용 (DATABASE_URL)
export DATABASE_URL="postgres://..."
trader migrate apply --dir migrations_v2
```

**주의:**
- 적용 전 자동으로 검증을 수행합니다
- 에러가 있으면 적용이 중단됩니다
- 운영 환경 적용 전 반드시 테스트 DB에서 먼저 검증하세요

### 상태 확인 (status)

현재 마이그레이션 적용 상태를 확인합니다.

```bash
trader migrate status --db-url "postgres://..."
```

## 데이터 안전 마이그레이션

### 기존 데이터가 있는 경우

통합 마이그레이션은 데이터를 보존하면서 안전하게 적용됩니다:

1. **IF NOT EXISTS 사용**: 이미 존재하는 객체는 건너뜀
2. **DROP 문장 제거**: 통합 시 DROP 문장은 제외됨
3. **OR REPLACE 뷰**: 뷰는 안전하게 재생성

### 스키마 변경이 필요한 경우

```bash
# 1. 현재 스키마 백업
pg_dump -s trader > schema_backup.sql

# 2. 데이터 백업 (중요 테이블)
pg_dump -t symbols -t ohlcv -t trade_executions trader > data_backup.sql

# 3. 테스트 DB에서 통합 마이그레이션 적용
trader migrate apply --db-url "postgres://test_db..." --dir migrations_v2

# 4. 스키마 비교
diff <(psql -c "\d+" original_db) <(psql -c "\d+" test_db)

# 5. 문제 없으면 운영 적용
trader migrate apply --dir migrations_v2
```

### 롤백이 필요한 경우

```bash
# 스키마 복원
psql trader < schema_backup.sql

# 데이터 복원
psql trader < data_backup.sql
```

## 검증 체크리스트

마이그레이션 적용 전:

- [ ] 데이터베이스 백업 완료
- [ ] 테스트 환경에서 먼저 실행
- [ ] `trader migrate verify` 통과
- [ ] CASCADE 사용 부분 검토

마이그레이션 적용 후:

- [ ] 모든 테이블 접근 가능 확인
- [ ] 주요 쿼리 정상 동작 확인
- [ ] 애플리케이션 정상 작동 확인

## 문제 해결

### 중복 정의 오류

```
[WARNING] DUP001: 'v_symbol_with_fundamental' 객체가 3 곳에서 정의됨
```

**해결:** 가장 최신 버전의 정의만 남기고 나머지 제거, 또는 통합 마이그레이션 사용

### CASCADE 경고

```
[WARNING] CASC001: CASCADE 사용 - 의존 객체가 자동 삭제될 수 있음
```

**해결:** CASCADE 대신 명시적으로 의존 객체를 먼저 삭제

### IF NOT EXISTS 누락

```
[INFO] IDEM001: IF NOT EXISTS 누락 - 재실행 시 오류 발생 가능
```

**해결:** `CREATE TABLE` → `CREATE TABLE IF NOT EXISTS`로 변경

## API 참조

### Rust 모듈

```rust
use trader_core::migration::{
    MigrationAnalyzer,      // SQL 파싱 및 의존성 분석
    MigrationValidator,     // 검증 수행
    MigrationConsolidator,  // 통합 계획 생성
    generate_safety_checklist, // 안전 체크리스트 생성
};

// 사용 예시
let analyzer = MigrationAnalyzer::new();
let files = analyzer.scan_directory(Path::new("migrations"))?;
let validator = MigrationValidator::new(&files);
let report = validator.validate();

println!("{}", report);
```

## 관련 문서

- [아키텍처](./architecture.md)
- [개발 규칙](./development_rules.md)
- [설치/배포 가이드](./setup_guide.md)
