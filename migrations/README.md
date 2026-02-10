# ZeroQuant Database Migrations

## 통합 마이그레이션 구조 (v2.0)

20개의 개별 마이그레이션을 6개의 기능별 그룹으로 통합했습니다.

### 파일 구조

| 파일 | 설명 | 원본 |
|------|------|------|
| `01_core_foundation.sql` | 핵심 기초 (Extensions, ENUM, 핵심 테이블, 자격증명) | 01, 02, 03 |
| `02_data_management.sql` | 데이터 관리 (심볼 메타, OHLCV, 체결 캐시) | 04, 05, 06 |
| `03_trading_analytics.sql` | 거래 분석 (저널, 포트폴리오, 분석 뷰) | 07, 08, 17 |
| `04_strategy_signals.sql` | 전략 시스템 (전략, 신호, 다중 타임프레임) | 09, 18, 19 |
| `05_evaluation_ranking.sql` | 검증/랭킹 (Reality Check, GlobalScore, 히스토리) | 10, 12, 20 |
| `06_user_settings.sql` | 사용자 설정 (관심종목, 프리셋, 거래소 통합) | 11, 13, 14, 15, 16 |
| `07_performance_optimization.sql` | 성능 최적화 (Hypertable, 인덱스, MV, Autovacuum) | 신규 |

### 실행 순서

반드시 **번호 순서대로** 실행해야 합니다 (의존성 보장):

```bash
psql -U trader -d trader -f 01_core_foundation.sql
psql -U trader -d trader -f 02_data_management.sql
psql -U trader -d trader -f 03_trading_analytics.sql
psql -U trader -d trader -f 04_strategy_signals.sql
psql -U trader -d trader -f 05_evaluation_ranking.sql
psql -U trader -d trader -f 06_user_settings.sql
psql -U trader -d trader -f 07_performance_optimization.sql
```

### 주요 테이블

#### 핵심 (01)
- `symbols`, `klines`, `trade_ticks`, `orders`, `trades`, `positions`
- `strategies`, `signals`, `users`, `api_keys`
- `exchange_credentials`, `telegram_settings`

#### 데이터 (02)
- `symbol_info`, `symbol_fundamental`
- `ohlcv`, `ohlcv_metadata`
- `execution_cache`, `execution_cache_meta`

#### 분석 (03)
- `trade_executions`, `position_snapshots`
- `portfolio_equity_history`, `backtest_results`
- 14+ 분석 뷰 (v_journal_executions, v_daily_pnl 등)

#### 전략 (04)
- `signal_marker`, `signal_alert_rule`
- 다중 타임프레임 지원 (strategies.multi_timeframe_config)

#### 평가 (05)
- `price_snapshot`, `reality_check`
- `symbol_global_score`, `score_history`

#### 설정 (06)
- `watchlist`, `watchlist_item`
- `screening_preset`
- `kis_token_cache`

#### 성능 최적화 (07)
- `score_history` → Hypertable 변환 (압축, 보관 정책)
- 인덱스 추가: `execution_cache`, `symbol_info`, `symbol_global_score`
- `mv_symbol_screening` Materialized View
- Autovacuum 튜닝: `ohlcv`, `execution_cache`, `symbol_global_score`

### TimescaleDB Hypertables

- `klines` (1주 청크, 2년 보존)
- `trade_ticks` (1일 청크, 6개월 보존)
- `ohlcv` (1주 청크, 2년 보존)
- `credential_access_logs` (90일 보존)
- `price_snapshot`, `reality_check` (1일 청크)
- `score_history` (1주 청크, 30일 압축, 1년 보존)

### Materialized Views

- `mv_symbol_screening` - 스크리닝 통합 뷰 (주기적 갱신 필요)
  ```sql
  -- 갱신 방법 (30분마다 권장)
  SELECT refresh_mv_symbol_screening();
  ```

### 버전 정보

- **v2.1** (2026-02-05): 성능 최적화 마이그레이션 추가 (07)
- **v2.0** (2026-02-05): 20개 → 6개 통합
- **v1.0**: 개별 마이그레이션 (01-20)
