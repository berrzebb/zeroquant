-- =====================================================
-- 07_performance_optimization.sql
-- 데이터베이스 성능 최적화
-- =====================================================
-- 포함 내용:
-- 1. score_history → TimescaleDB Hypertable 변환
-- 2. 누락된 인덱스 추가 (조회 성능 향상)
-- 3. Materialized View 생성 (스크리닝 쿼리 최적화)
-- 4. Autovacuum 튜닝 (대량 업데이트 테이블)
-- =====================================================

-- =====================================================
-- 1. SCORE_HISTORY HYPERTABLE 변환
-- =====================================================
-- 기존 score_history 테이블을 TimescaleDB Hypertable로 변환
-- 압축: 30일 이후 자동 압축, 보관: 1년 후 자동 삭제
-- =====================================================

-- 1.1 기존 테이블 마이그레이션 준비
-- PRIMARY KEY 변경: id (SERIAL) → (score_date, symbol) 복합키
-- Hypertable은 time 컬럼이 PRIMARY KEY에 포함되어야 함

-- 기존 테이블 백업 및 재생성
DO $$
DECLARE
    has_data BOOLEAN;
    table_exists BOOLEAN;
BEGIN
    -- 테이블 존재 여부 확인
    SELECT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'score_history'
    ) INTO table_exists;

    IF NOT table_exists THEN
        RAISE NOTICE 'score_history 테이블이 존재하지 않습니다. 새로 생성합니다.';
        RETURN;
    END IF;

    -- 이미 Hypertable인지 확인
    IF EXISTS (
        SELECT 1 FROM timescaledb_information.hypertables
        WHERE hypertable_name = 'score_history'
    ) THEN
        RAISE NOTICE 'score_history가 이미 Hypertable입니다.';
        RETURN;
    END IF;

    -- 데이터 존재 여부 확인
    SELECT EXISTS (SELECT 1 FROM score_history LIMIT 1) INTO has_data;

    IF has_data THEN
        -- 기존 데이터 백업
        CREATE TABLE IF NOT EXISTS score_history_backup AS SELECT * FROM score_history;
        RAISE NOTICE 'score_history 데이터를 백업했습니다.';
    END IF;

    -- 기존 테이블 삭제
    DROP TABLE IF EXISTS score_history CASCADE;
    RAISE NOTICE '기존 score_history 테이블을 삭제했습니다.';
END $$;

-- 1.2 새 테이블 생성 (Hypertable 호환 스키마)
CREATE TABLE IF NOT EXISTS score_history (
    -- Hypertable은 시간 컬럼을 PRIMARY KEY에 포함해야 함
    score_date DATE NOT NULL,
    symbol VARCHAR(20) NOT NULL,

    -- 점수 데이터
    global_score DECIMAL(5,2),
    route_state VARCHAR(20),
    rank INTEGER,
    component_scores JSONB,

    -- 메타데이터
    created_at TIMESTAMPTZ DEFAULT NOW(),

    -- PRIMARY KEY: (score_date, symbol) - 시간 기반 파티셔닝 지원
    PRIMARY KEY (score_date, symbol)
);

-- 1.3 Hypertable로 변환
SELECT create_hypertable(
    'score_history',
    'score_date',
    chunk_time_interval => INTERVAL '1 week',
    if_not_exists => TRUE,
    migrate_data => TRUE
);

-- 1.4 압축 정책 설정 (30일 이후 자동 압축)
ALTER TABLE score_history SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol',
    timescaledb.compress_orderby = 'score_date DESC'
);

-- 압축 정책 추가 (30일 이상 된 청크 압축)
SELECT add_compression_policy(
    'score_history',
    INTERVAL '30 days',
    if_not_exists => TRUE
);

-- 1.5 보관 정책 설정 (1년 후 자동 삭제)
SELECT add_retention_policy(
    'score_history',
    INTERVAL '1 year',
    if_not_exists => TRUE
);

-- 1.6 인덱스 생성 (조회 최적화)
-- 심볼별 최신 점수 조회용
CREATE INDEX IF NOT EXISTS idx_score_history_symbol_date
ON score_history(symbol, score_date DESC);

-- 날짜별 전체 순위 조회용
CREATE INDEX IF NOT EXISTS idx_score_history_date_score
ON score_history(score_date DESC, global_score DESC);

-- 점수 기반 필터링용
CREATE INDEX IF NOT EXISTS idx_score_history_score
ON score_history(score_date, global_score DESC);

-- 1.7 백업 데이터 복원 (있는 경우)
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'score_history_backup'
    ) THEN
        INSERT INTO score_history (score_date, symbol, global_score, route_state, rank, component_scores, created_at)
        SELECT score_date, symbol, global_score, route_state, rank, component_scores, created_at
        FROM score_history_backup
        ON CONFLICT (score_date, symbol) DO NOTHING;

        -- 백업 테이블 삭제
        DROP TABLE score_history_backup;
        RAISE NOTICE 'score_history 데이터를 복원하고 백업을 삭제했습니다.';
    END IF;
END $$;

COMMENT ON TABLE score_history IS '종목별 Global Score, RouteState, 순위의 일별 히스토리 (Hypertable)';
COMMENT ON COLUMN score_history.symbol IS '종목 코드';
COMMENT ON COLUMN score_history.score_date IS '점수 계산 날짜';
COMMENT ON COLUMN score_history.global_score IS 'Global Score (0-100)';
COMMENT ON COLUMN score_history.route_state IS 'RouteState (Attack/Armed/Watch/Wait/Danger)';
COMMENT ON COLUMN score_history.rank IS '해당 날짜의 순위';
COMMENT ON COLUMN score_history.component_scores IS '7 Factor 개별 점수 (JSON)';

-- =====================================================
-- 2. 누락된 인덱스 추가
-- =====================================================

-- 2.1 execution_cache: 심볼+시간 복합 인덱스
-- 특정 종목의 체결 내역 조회 성능 향상
CREATE INDEX IF NOT EXISTS idx_exec_cache_symbol_time
ON execution_cache(symbol, executed_at DESC);

-- execution_cache: 날짜 범위 조회 최적화 (executed_at::date 대신 직접 컬럼 사용)
CREATE INDEX IF NOT EXISTS idx_exec_cache_date_range
ON execution_cache(credential_id, executed_at, symbol);

-- 2.2 symbol_info: 섹터 인덱스
-- 섹터별 종목 필터링 성능 향상
CREATE INDEX IF NOT EXISTS idx_symbol_info_sector
ON symbol_info(sector)
WHERE sector IS NOT NULL;

-- symbol_info: 시장+섹터 복합 인덱스
CREATE INDEX IF NOT EXISTS idx_symbol_info_market_sector
ON symbol_info(market, sector)
WHERE is_active = true;

-- 2.3 symbol_global_score: Grade + Score 복합 인덱스
-- 랭킹 조회 최적화 (BUY 등급 중 높은 점수 순)
CREATE INDEX IF NOT EXISTS idx_global_score_grade_score
ON symbol_global_score(grade, overall_score DESC);

-- symbol_global_score: 시장별 랭킹 조회
CREATE INDEX IF NOT EXISTS idx_global_score_market_score
ON symbol_global_score(market, overall_score DESC);

-- symbol_global_score: 최근 계산된 점수 조회
CREATE INDEX IF NOT EXISTS idx_global_score_calculated
ON symbol_global_score(calculated_at DESC);

-- =====================================================
-- 3. MATERIALIZED VIEW: 스크리닝 최적화
-- =====================================================
-- 자주 사용되는 스크리닝 쿼리를 미리 계산하여 저장
-- 30분마다 갱신 권장 (cron 또는 pg_cron)
-- =====================================================

-- 기존 뷰 삭제 후 재생성
DROP MATERIALIZED VIEW IF EXISTS mv_symbol_screening CASCADE;

CREATE MATERIALIZED VIEW mv_symbol_screening AS
SELECT
    -- 기본 심볼 정보
    si.id AS symbol_info_id,
    si.ticker,
    si.name,
    si.market,
    si.exchange,  -- KOSPI, KOSDAQ, NASDAQ 등 거래소 구분
    si.sector,
    si.symbol_type,
    si.yahoo_symbol,

    -- 펀더멘털 데이터
    sf.market_cap,
    sf.per,
    sf.pbr,
    sf.roe,
    sf.eps,
    sf.dividend_yield,
    sf.week_52_high,
    sf.week_52_low,

    -- Global Score
    gs.overall_score AS global_score,
    gs.grade,
    gs.confidence,
    gs.component_scores,
    gs.calculated_at AS score_calculated_at,

    -- 계산된 메트릭
    CASE
        WHEN sf.week_52_high > 0 AND sf.week_52_low > 0 THEN
            ROUND(((sf.week_52_high - sf.week_52_low) / sf.week_52_low * 100)::numeric, 2)
        ELSE NULL
    END AS year_range_pct,

    -- 최종 업데이트 시간
    GREATEST(si.updated_at, sf.updated_at, gs.updated_at) AS last_updated

FROM symbol_info si
LEFT JOIN symbol_fundamental sf ON si.id = sf.symbol_info_id
LEFT JOIN symbol_global_score gs ON si.id = gs.symbol_info_id
WHERE si.is_active = true;

-- Materialized View 인덱스
CREATE UNIQUE INDEX IF NOT EXISTS idx_mv_screening_symbol_id
ON mv_symbol_screening(symbol_info_id);

CREATE INDEX IF NOT EXISTS idx_mv_screening_ticker
ON mv_symbol_screening(ticker);

CREATE INDEX IF NOT EXISTS idx_mv_screening_market
ON mv_symbol_screening(market);

CREATE INDEX IF NOT EXISTS idx_mv_screening_exchange
ON mv_symbol_screening(exchange)
WHERE exchange IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_mv_screening_market_exchange
ON mv_symbol_screening(market, exchange);

CREATE INDEX IF NOT EXISTS idx_mv_screening_sector
ON mv_symbol_screening(sector)
WHERE sector IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_mv_screening_global_score
ON mv_symbol_screening(global_score DESC NULLS LAST);

CREATE INDEX IF NOT EXISTS idx_mv_screening_grade
ON mv_symbol_screening(grade)
WHERE grade IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_mv_screening_market_score
ON mv_symbol_screening(market, global_score DESC NULLS LAST);

COMMENT ON MATERIALIZED VIEW mv_symbol_screening IS '스크리닝용 통합 Materialized View - 주기적 REFRESH 필요';

-- 갱신 함수 생성
CREATE OR REPLACE FUNCTION refresh_mv_symbol_screening()
RETURNS void AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY mv_symbol_screening;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION refresh_mv_symbol_screening IS 'mv_symbol_screening 갱신 (CONCURRENTLY - 읽기 차단 없음)';

-- =====================================================
-- 4. AUTOVACUUM 튜닝
-- =====================================================
-- 대량 업데이트가 자주 발생하는 테이블에 대해
-- autovacuum 임계값을 낮춰서 더 자주 실행되도록 설정
-- =====================================================

-- 4.1 ohlcv: 일봉 데이터 (매일 수천 건 INSERT)
ALTER TABLE ohlcv SET (
    autovacuum_vacuum_threshold = 1000,
    autovacuum_vacuum_scale_factor = 0.05,
    autovacuum_analyze_threshold = 500,
    autovacuum_analyze_scale_factor = 0.02
);

-- 4.2 execution_cache: 체결 데이터 (거래 시 대량 INSERT)
ALTER TABLE execution_cache SET (
    autovacuum_vacuum_threshold = 500,
    autovacuum_vacuum_scale_factor = 0.05,
    autovacuum_analyze_threshold = 250,
    autovacuum_analyze_scale_factor = 0.02
);

-- 4.3 symbol_global_score: 점수 데이터 (배치 UPDATE)
ALTER TABLE symbol_global_score SET (
    autovacuum_vacuum_threshold = 100,
    autovacuum_vacuum_scale_factor = 0.1,
    autovacuum_analyze_threshold = 50,
    autovacuum_analyze_scale_factor = 0.05
);

-- 4.4 score_history: 히스토리 데이터 (일일 INSERT)
-- Hypertable의 경우 청크별로 적용됨
ALTER TABLE score_history SET (
    autovacuum_vacuum_threshold = 500,
    autovacuum_vacuum_scale_factor = 0.05,
    autovacuum_analyze_threshold = 250,
    autovacuum_analyze_scale_factor = 0.02
);

-- =====================================================
-- 5. 통계 수집 (쿼리 플래너 최적화)
-- =====================================================
-- 인덱스 생성 후 통계 갱신으로 쿼리 플래너가 최적의 실행 계획 선택

ANALYZE symbol_info;
ANALYZE symbol_fundamental;
ANALYZE symbol_global_score;
ANALYZE execution_cache;
ANALYZE score_history;

-- =====================================================
-- 마이그레이션 기록
-- =====================================================
INSERT INTO schema_migrations (version, filename, success, applied_at)
VALUES (100, '07_performance_optimization.sql', true, NOW())
ON CONFLICT (version) DO NOTHING;

-- =====================================================
-- 사용 가이드
-- =====================================================
--
-- 1. Materialized View 갱신 (30분마다 권장):
--    SELECT refresh_mv_symbol_screening();
--    또는
--    REFRESH MATERIALIZED VIEW CONCURRENTLY mv_symbol_screening;
--
-- 2. Hypertable 압축 상태 확인:
--    SELECT * FROM timescaledb_information.compression_settings
--    WHERE hypertable_name = 'score_history';
--
-- 3. 청크 정보 확인:
--    SELECT * FROM timescaledb_information.chunks
--    WHERE hypertable_name = 'score_history';
--
-- 4. 압축 통계 확인:
--    SELECT * FROM hypertable_compression_stats('score_history');
--
-- 5. 수동 압축 실행 (필요 시):
--    SELECT compress_chunk(chunk) FROM show_chunks('score_history') chunk
--    WHERE chunk < NOW() - INTERVAL '30 days';
--
-- =====================================================
