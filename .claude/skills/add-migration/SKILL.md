---
name: add-migration
description: 번호가 매겨진 SQL 마이그레이션 파일 생성. 테이블/인덱스/뷰 추가 워크플로우.
disable-model-invocation: true
user-invocable: true
argument-hint: "<설명_snake_case> [테이블명|기능명]"
allowed-tools: Read, Grep, Write, Bash(podman *)
---

# DB 마이그레이션 추가 워크플로우

`$ARGUMENTS` 마이그레이션을 생성합니다.

---

## 1단계: 다음 번호 결정

```bash
ls migrations/ | sort | tail -1
```

현재 마지막 번호 + 1을 사용합니다.

---

## 2단계: 마이그레이션 파일 생성

**위치**: `migrations/<번호>_$ARGUMENTS[0].sql`

### 작성 규칙

```sql
-- 마이그레이션: <설명>
-- 날짜: YYYY-MM-DD

-- 1. 테이블 생성 (IF NOT EXISTS 필수)
CREATE TABLE IF NOT EXISTS <테이블명> (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- 가격/수량 필드는 NUMERIC(20,8) 사용 (FLOAT 금지)
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- 2. 인덱스 (성능 최적화)
CREATE INDEX IF NOT EXISTS idx_<테이블>_<컬럼>
    ON <테이블> (<컬럼>);

-- 3. TimescaleDB 하이퍼테이블 (시계열 데이터인 경우)
-- SELECT create_hypertable('<테이블>', 'created_at', if_not_exists => TRUE);
```

### 체크포인트
- [ ] `IF NOT EXISTS` / `IF EXISTS` 사용 (멱등성)
- [ ] 가격/수량은 `NUMERIC(20,8)` (FLOAT 금지)
- [ ] 시계열 데이터 → TimescaleDB `create_hypertable` 고려
- [ ] 외래키는 `ON DELETE CASCADE` 또는 `SET NULL` 명시
- [ ] 한글 주석으로 설명

---

## 3단계: 적용 및 검증

```powershell
# 1. 마이그레이션 적용
podman exec -i trader-timescaledb psql -U trader -d trader -f /dev/stdin < migrations/<번호>_$ARGUMENTS[0].sql

# 2. 테이블 확인
podman exec trader-timescaledb psql -U trader -d trader -c "\dt <테이블명>"

# 3. 스키마 확인
podman exec trader-timescaledb psql -U trader -d trader -c "\d <테이블명>"
```

---

## 4단계: Rust 모델 연동 (필요시)

해당 테이블에 대한 Rust 구조체가 필요한 경우:

1. `crates/trader-core/src/domain/` 또는 해당 crate에 모델 추가
2. `sqlx::FromRow` derive 추가
3. Repository 함수 구현

---

## 5단계: docs 갱신

`docs/migration_guide.md`에 새 마이그레이션 정보 추가.
