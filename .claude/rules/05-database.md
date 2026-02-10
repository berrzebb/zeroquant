# 데이터베이스 규칙

## 1. 마이그레이션 명명

```
migrations/
  001_initial_schema.sql
  002_add_encrypted_credentials.sql
  ...
  014_add_my_feature.sql  # 순번 + 설명
```

## 2. 인덱스 필수 확인

자주 조회하는 컬럼에 인덱스 추가:

```sql
CREATE INDEX idx_orders_strategy_id ON orders(strategy_id);
CREATE INDEX idx_orders_created_at ON orders(created_at);
```

## 3. TimescaleDB Hypertable

> 시계열 데이터는 Hypertable로 생성

```sql
-- 일반 테이블 생성
CREATE TABLE klines (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    open DECIMAL(18,8),
    -- ...
);

-- Hypertable로 변환
SELECT create_hypertable('klines', 'time');
```
