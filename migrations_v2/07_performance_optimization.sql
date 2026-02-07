-- =============================================================================
-- 07_performance_optimization
-- 인덱스, MV, Hypertable 정책
-- =============================================================================
-- 통합 마이그레이션 파일 (자동 생성)
-- 원본 파일: ["07_", "08_", "19_"]
-- =============================================================================

-- ---------------------------------------------------------------------------
-- Source: 07_performance_optimization
-- ---------------------------------------------------------------------------

DO $$
DECLARE
    has_data BOOLEAN;
    table_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'score_history'
    ) INTO table_exists;

    IF NOT table_exists THEN
        RAISE NOTICE 'score_history 테이블이 존재하지 않습니다. 새로 생성합니다.';
        RETURN;
    END IF;

    IF EXISTS (
        SELECT 1 FROM timescaledb_information.hypertables
        WHERE hypertable_name = 'score_history'
    ) THEN
        RAISE NOTICE 'score_history가 이미 Hypertable입니다.';
        RETURN;
    END IF;

    SELECT EXISTS (SELECT 1 FROM score_history LIMIT 1) INTO has_data;

    IF has_data THEN
        CREATE TABLE IF NOT EXISTS score_history_backup AS SELECT * FROM score_history;
        RAISE NOTICE 'score_history 데이터를 백업했습니다.';
    END IF;

    DROP TABLE IF EXISTS score_history CASCADE;
    RAISE NOTICE '기존 score_history 테이블을 삭제했습니다.';
END $$;

CREATE TABLE IF NOT EXISTS score_history (
    score_date DATE NOT NULL,
    symbol VARCHAR(20) NOT NULL,

    global_score DECIMAL(5,2),
    route_state VARCHAR(20),
    rank INTEGER,
    component_scores JSONB,

    created_at TIMESTAMPTZ DEFAULT NOW(),

    PRIMARY KEY (score_date, symbol)
);

SELECT create_hypertable(
    'score_history',
    'score_date',
    chunk_time_interval => INTERVAL '1 week',
    if_not_exists => TRUE,
    migrate_data => TRUE
);

ALTER TABLE score_history SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol',
    timescaledb.compress_orderby = 'score_date DESC'
);

SELECT add_compression_policy(
    'score_history',
    INTERVAL '30 days',
    if_not_exists => TRUE
);

SELECT add_retention_policy(
    'score_history',
    INTERVAL '1 year',
    if_not_exists => TRUE
);

CREATE INDEX IF NOT EXISTS idx_score_history_symbol_date
ON score_history(symbol, score_date DESC);

CREATE INDEX IF NOT EXISTS idx_score_history_date_score
ON score_history(score_date DESC, global_score DESC);

CREATE INDEX IF NOT EXISTS idx_score_history_score
ON score_history(score_date, global_score DESC);

COMMENT ON TABLE score_history IS '종목별 Global Score, RouteState, 순위의 일별 히스토리 (Hypertable)';

COMMENT ON COLUMN score_history.symbol IS '종목 코드';

COMMENT ON COLUMN score_history.score_date IS '점수 계산 날짜';

COMMENT ON COLUMN score_history.global_score IS 'Global Score (0-100)';

COMMENT ON COLUMN score_history.route_state IS 'RouteState (Attack/Armed/Watch/Wait/Danger)';

COMMENT ON COLUMN score_history.rank IS '해당 날짜의 순위';

COMMENT ON COLUMN score_history.component_scores IS '7 Factor 개별 점수 (JSON)';

CREATE INDEX IF NOT EXISTS idx_exec_cache_symbol_time
ON execution_cache(symbol, executed_at DESC);

CREATE INDEX IF NOT EXISTS idx_exec_cache_date_range
ON execution_cache(credential_id, executed_at, symbol);

CREATE INDEX IF NOT EXISTS idx_symbol_info_sector
ON symbol_info(sector)
WHERE sector IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_symbol_info_market_sector
ON symbol_info(market, sector)
WHERE is_active = true;

CREATE INDEX IF NOT EXISTS idx_global_score_grade_score
ON symbol_global_score(grade, overall_score DESC);

CREATE INDEX IF NOT EXISTS idx_global_score_market_score
ON symbol_global_score(market, overall_score DESC);

CREATE INDEX IF NOT EXISTS idx_global_score_calculated
ON symbol_global_score(calculated_at DESC);

CREATE MATERIALIZED VIEW mv_symbol_screening AS
SELECT
    si.id AS symbol_info_id,
    si.ticker,
    si.name,
    si.market,
    si.exchange,  -- KOSPI, KOSDAQ, NASDAQ 등 거래소 구분
    si.sector,
    si.symbol_type,
    si.yahoo_symbol,

    sf.market_cap,
    sf.per,
    sf.pbr,
    sf.roe,
    sf.eps,
    sf.dividend_yield,
    sf.week_52_high,
    sf.week_52_low,

    gs.overall_score AS global_score,
    gs.grade,
    gs.confidence,
    gs.component_scores,
    gs.calculated_at AS score_calculated_at,

    CASE
        WHEN sf.week_52_high > 0 AND sf.week_52_low > 0 THEN
            ROUND(((sf.week_52_high - sf.week_52_low) / sf.week_52_low * 100)::numeric, 2)
        ELSE NULL
    END AS year_range_pct,

    GREATEST(si.updated_at, sf.updated_at, gs.updated_at) AS last_updated

FROM symbol_info si
LEFT JOIN symbol_fundamental sf ON si.id = sf.symbol_info_id
LEFT JOIN symbol_global_score gs ON si.id = gs.symbol_info_id
WHERE si.is_active = true;

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

CREATE OR REPLACE FUNCTION refresh_mv_symbol_screening()
RETURNS void AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY mv_symbol_screening;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION refresh_mv_symbol_screening IS 'mv_symbol_screening 갱신 (CONCURRENTLY - 읽기 차단 없음)';

ALTER TABLE ohlcv SET (
    autovacuum_vacuum_threshold = 1000,
    autovacuum_vacuum_scale_factor = 0.05,
    autovacuum_analyze_threshold = 500,
    autovacuum_analyze_scale_factor = 0.02
);

ALTER TABLE execution_cache SET (
    autovacuum_vacuum_threshold = 500,
    autovacuum_vacuum_scale_factor = 0.05,
    autovacuum_analyze_threshold = 250,
    autovacuum_analyze_scale_factor = 0.02
);

ALTER TABLE symbol_global_score SET (
    autovacuum_vacuum_threshold = 100,
    autovacuum_vacuum_scale_factor = 0.1,
    autovacuum_analyze_threshold = 50,
    autovacuum_analyze_scale_factor = 0.05
);

ALTER TABLE score_history SET (
    autovacuum_vacuum_threshold = 500,
    autovacuum_vacuum_scale_factor = 0.05,
    autovacuum_analyze_threshold = 250,
    autovacuum_analyze_scale_factor = 0.02
);

ANALYZE symbol_info;

ANALYZE symbol_fundamental;

ANALYZE symbol_global_score;

ANALYZE execution_cache;

ANALYZE score_history;

INSERT INTO schema_migrations (version, filename, success, applied_at)
VALUES (100, '07_performance_optimization.sql', true, NOW())
ON CONFLICT (version) DO NOTHING;

-- ---------------------------------------------------------------------------
-- Source: 19_api_performance_optimization
-- ---------------------------------------------------------------------------

CREATE INDEX IF NOT EXISTS idx_position_snapshots_current
ON position_snapshots(credential_id, symbol, snapshot_time DESC)
WHERE quantity > 0;

COMMENT ON INDEX idx_position_snapshots_current IS
'현재 포지션 조회 최적화 (DISTINCT ON credential_id, symbol)';

CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_route_state_all
ON symbol_fundamental(symbol_info_id, route_state);

COMMENT ON INDEX idx_symbol_fundamental_route_state_all IS
'RouteState 기반 랭킹 필터 최적화 (전체 상태 커버)';

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_sector_rs AS
WITH sector_prices AS (
    SELECT
        sf.sector,
        sf.ticker,
        sf.market_cap,
        si.market,
        si.exchange,
        first_value(o.close) OVER (
            PARTITION BY sf.ticker
            ORDER BY o.open_time ASC
        ) as start_price,
        first_value(o.close) OVER (
            PARTITION BY sf.ticker
            ORDER BY o.open_time DESC
        ) as end_price
    FROM v_symbol_with_fundamental sf
    JOIN symbol_info si ON sf.id = si.id
    JOIN ohlcv o ON o.symbol = sf.ticker
    WHERE o.timeframe = '1d'
      AND o.open_time >= (CURRENT_DATE - INTERVAL '20 days')
      AND sf.sector IS NOT NULL
      AND sf.sector != ''
),
sector_returns AS (
    SELECT DISTINCT ON (sector, ticker, market, exchange)
        sector,
        ticker,
        market,
        exchange,
        market_cap,
        CASE
            WHEN start_price > 0
            THEN ((end_price - start_price) / start_price) * 100
            ELSE 0
        END as return_pct
    FROM sector_prices
),
sector_avg_returns AS (
    SELECT
        sector,
        market,
        exchange,
        COUNT(*) as symbol_count,
        AVG(return_pct) as avg_return_pct,
        SUM(market_cap) as total_market_cap
    FROM sector_returns
    GROUP BY sector, market, exchange
    HAVING COUNT(*) >= 3
),
market_avg AS (
    SELECT
        market,
        AVG(avg_return_pct) as market_return
    FROM sector_avg_returns
    GROUP BY market
)
SELECT
    s.sector,
    s.market,
    s.exchange,
    s.symbol_count,
    ROUND(s.avg_return_pct::numeric, 4) as avg_return_pct,
    ROUND(m.market_return::numeric, 4) as market_return,
    ROUND(CASE
        WHEN m.market_return > 0
        THEN s.avg_return_pct / m.market_return
        ELSE 1.0
    END::numeric, 4) as relative_strength,
    ROUND(CASE
        WHEN m.market_return > 0
        THEN (s.avg_return_pct / m.market_return) * 0.6 + (s.avg_return_pct / 10.0) * 0.4
        ELSE s.avg_return_pct / 10.0
    END::numeric, 4) as composite_score,
    s.total_market_cap,
    NOW() as calculated_at
FROM sector_avg_returns s
JOIN market_avg m ON s.market = m.market;

CREATE UNIQUE INDEX IF NOT EXISTS idx_mv_sector_rs_key
ON mv_sector_rs(sector, market, exchange);

CREATE INDEX IF NOT EXISTS idx_mv_sector_rs_composite
ON mv_sector_rs(composite_score DESC);

CREATE INDEX IF NOT EXISTS idx_mv_sector_rs_market
ON mv_sector_rs(market, composite_score DESC);

COMMENT ON MATERIALIZED VIEW mv_sector_rs IS
'섹터별 Relative Strength 사전 계산 - Collector에서 주기적 REFRESH';

CREATE OR REPLACE FUNCTION refresh_mv_sector_rs()
RETURNS void AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY mv_sector_rs;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION refresh_mv_sector_rs IS
'mv_sector_rs 갱신 (CONCURRENTLY - 읽기 차단 없음). OHLCV 업데이트 후 호출 권장.';

ANALYZE position_snapshots;

ANALYZE symbol_fundamental;

INSERT INTO schema_migrations (version, filename, success, applied_at)
VALUES (119, '19_api_performance_optimization.sql', true, NOW())
ON CONFLICT (version) DO NOTHING;

