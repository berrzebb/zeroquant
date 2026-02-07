-- =============================================================================
-- 05_evaluation_ranking
-- global_score, reality_check, score_history
-- =============================================================================
-- 통합 마이그레이션 파일 (자동 생성)
-- 원본 파일: ["05_"]
-- =============================================================================

-- ---------------------------------------------------------------------------
-- Source: 05_evaluation_ranking
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS price_snapshot (
    snapshot_date DATE NOT NULL,                    -- 스냅샷 일자 (장 마감일)
    symbol VARCHAR(20) NOT NULL,                    -- 종목 코드

    close_price DECIMAL(20, 4) NOT NULL,            -- 종가
    volume BIGINT,                                  -- 거래량

    recommend_source VARCHAR(50),                   -- 추천 소스 (screening, strategy_xyz)
    recommend_rank INT,                             -- 추천 순위 (1~N)
    recommend_score DECIMAL(5, 2),                  -- 추천 점수 (0~100)

    expected_return DECIMAL(8, 4),                  -- 기대 수익률 (%)
    expected_holding_days INT,                      -- 예상 보유 기간

    market VARCHAR(20),                             -- 시장 (KR, US, CRYPTO)
    sector VARCHAR(50),                             -- 섹터

    created_at TIMESTAMPTZ DEFAULT NOW(),

    PRIMARY KEY (snapshot_date, symbol, recommend_source)
);

SELECT create_hypertable('price_snapshot', 'snapshot_date',
    chunk_time_interval => INTERVAL '1 month',
    if_not_exists => TRUE
);

CREATE INDEX IF NOT EXISTS idx_price_snapshot_symbol
    ON price_snapshot(symbol, snapshot_date DESC);

CREATE INDEX IF NOT EXISTS idx_price_snapshot_source
    ON price_snapshot(recommend_source, snapshot_date DESC);

CREATE INDEX IF NOT EXISTS idx_price_snapshot_rank
    ON price_snapshot(recommend_rank)
    WHERE recommend_rank <= 10;                     -- Top 10 추천만 인덱싱

ALTER TABLE price_snapshot SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol, recommend_source'
);

SELECT add_compression_policy('price_snapshot',
    INTERVAL '30 days',
    if_not_exists => TRUE
);

SELECT add_retention_policy('price_snapshot',
    INTERVAL '1 year',
    if_not_exists => TRUE
);

COMMENT ON TABLE price_snapshot IS 'Reality Check - 추천 종목 가격 스냅샷 (TimescaleDB Hypertable, 1년 보존)';

COMMENT ON COLUMN price_snapshot.recommend_source IS '추천 소스 (screening_momentum, strategy_rsi, strategy_sma 등)';

COMMENT ON COLUMN price_snapshot.recommend_score IS '추천 점수 (0~100, 높을수록 강한 추천)';

CREATE TABLE IF NOT EXISTS reality_check (
    check_date DATE NOT NULL,                       -- 검증 일자 (익일 장 마감일)
    recommend_date DATE NOT NULL,                   -- 추천 일자
    symbol VARCHAR(20) NOT NULL,

    recommend_source VARCHAR(50),
    recommend_rank INT,
    recommend_score DECIMAL(5, 2),

    entry_price DECIMAL(20, 4) NOT NULL,            -- 진입가 (추천일 종가)
    exit_price DECIMAL(20, 4) NOT NULL,             -- 청산가 (검증일 종가)

    actual_return DECIMAL(8, 4) NOT NULL,           -- 실제 수익률 (%)
    is_profitable BOOLEAN NOT NULL,                 -- 수익 여부

    entry_volume BIGINT,
    exit_volume BIGINT,
    volume_change DECIMAL(8, 4),                    -- 거래량 변화율 (%)

    expected_return DECIMAL(8, 4),                  -- 기대 수익률
    return_error DECIMAL(8, 4),                     -- 오차율 (actual - expected)

    max_profit DECIMAL(8, 4),                       -- 최대 수익률 (보유 기간 중)
    max_drawdown DECIMAL(8, 4),                     -- 최대 하락률
    volatility DECIMAL(8, 4),                       -- 변동성 (표준편차)

    market VARCHAR(20),
    sector VARCHAR(50),

    created_at TIMESTAMPTZ DEFAULT NOW(),

    PRIMARY KEY (check_date, symbol, recommend_source)
);

SELECT create_hypertable('reality_check', 'check_date',
    chunk_time_interval => INTERVAL '1 month',
    if_not_exists => TRUE
);

CREATE INDEX IF NOT EXISTS idx_reality_check_recommend_date
    ON reality_check(recommend_date, check_date);

CREATE INDEX IF NOT EXISTS idx_reality_check_symbol
    ON reality_check(symbol, check_date DESC);

CREATE INDEX IF NOT EXISTS idx_reality_check_source
    ON reality_check(recommend_source, check_date DESC);

CREATE INDEX IF NOT EXISTS idx_reality_check_profitable
    ON reality_check(is_profitable, check_date DESC);

CREATE INDEX IF NOT EXISTS idx_reality_check_return
    ON reality_check(actual_return DESC);

ALTER TABLE reality_check SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol, recommend_source'
);

SELECT add_compression_policy('reality_check',
    INTERVAL '30 days',
    if_not_exists => TRUE
);

SELECT add_retention_policy('reality_check',
    INTERVAL '1 year',
    if_not_exists => TRUE
);

COMMENT ON TABLE reality_check IS 'Reality Check - 추천 검증 결과 (TimescaleDB Hypertable, 1년 보존)';

COMMENT ON COLUMN reality_check.actual_return IS '실제 수익률 (%) = (exit_price - entry_price) / entry_price * 100';

COMMENT ON COLUMN reality_check.return_error IS '예측 오차 = 실제 수익률 - 기대 수익률';

CREATE OR REPLACE FUNCTION calculate_reality_check(
    p_recommend_date DATE,
    p_check_date DATE
) RETURNS TABLE (
    symbol VARCHAR(20),
    actual_return DECIMAL(8, 4),
    is_profitable BOOLEAN,
    processed_count INT
) AS $$
DECLARE
    v_processed INT := 0;
BEGIN
    INSERT INTO reality_check (
        check_date,
        recommend_date,
        symbol,
        recommend_source,
        recommend_rank,
        recommend_score,
        entry_price,
        exit_price,
        actual_return,
        is_profitable,
        entry_volume,
        exit_volume,
        volume_change,
        expected_return,
        return_error,
        market,
        sector
    )
    SELECT
        p_check_date,
        ps.snapshot_date,
        ps.symbol,
        ps.recommend_source,
        ps.recommend_rank,
        ps.recommend_score,
        ps.close_price AS entry_price,
        today.close AS exit_price,
        ROUND(((today.close - ps.close_price) / ps.close_price * 100)::NUMERIC, 4) AS actual_return,
        today.close >= ps.close_price AS is_profitable,
        ps.volume AS entry_volume,
        today.volume AS exit_volume,
        CASE
            WHEN ps.volume > 0 THEN ROUND(((today.volume::NUMERIC - ps.volume::NUMERIC) / ps.volume::NUMERIC * 100), 4)
            ELSE NULL
        END AS volume_change,
        ps.expected_return,
        CASE
            WHEN ps.expected_return IS NOT NULL
            THEN ROUND((((today.close - ps.close_price) / ps.close_price * 100) - ps.expected_return)::NUMERIC, 4)
            ELSE NULL
        END AS return_error,
        ps.market,
        ps.sector
    FROM price_snapshot ps
    INNER JOIN mv_latest_prices today ON ps.symbol = today.symbol
    WHERE ps.snapshot_date = p_recommend_date
        AND today.open_time::DATE = p_check_date
    ON CONFLICT (check_date, symbol, recommend_source) DO UPDATE SET
        exit_price = EXCLUDED.exit_price,
        actual_return = EXCLUDED.actual_return,
        is_profitable = EXCLUDED.is_profitable,
        exit_volume = EXCLUDED.exit_volume,
        volume_change = EXCLUDED.volume_change,
        return_error = EXCLUDED.return_error;

    GET DIAGNOSTICS v_processed = ROW_COUNT;

    RETURN QUERY
    SELECT
        rc.symbol,
        rc.actual_return,
        rc.is_profitable,
        v_processed
    FROM reality_check rc
    WHERE rc.check_date = p_check_date
    ORDER BY rc.actual_return DESC;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION calculate_reality_check IS '전일 추천 종목의 금일 성과를 자동 계산하여 reality_check 테이블에 저장';

CREATE OR REPLACE VIEW v_reality_check_daily_stats AS
SELECT
    check_date,
    COUNT(*) AS total_count,
    COUNT(*) FILTER (WHERE is_profitable) AS win_count,
    ROUND(COUNT(*) FILTER (WHERE is_profitable)::NUMERIC / COUNT(*) * 100, 2) AS win_rate,
    ROUND(AVG(actual_return), 4) AS avg_return,
    ROUND(AVG(actual_return) FILTER (WHERE is_profitable), 4) AS avg_win_return,
    ROUND(AVG(actual_return) FILTER (WHERE NOT is_profitable), 4) AS avg_loss_return,
    ROUND(MAX(actual_return), 4) AS max_return,
    ROUND(MIN(actual_return), 4) AS min_return,
    ROUND(STDDEV(actual_return), 4) AS return_stddev
FROM reality_check
GROUP BY check_date
ORDER BY check_date DESC;

CREATE OR REPLACE VIEW v_reality_check_source_stats AS
SELECT
    recommend_source,
    COUNT(*) AS total_count,
    COUNT(*) FILTER (WHERE is_profitable) AS win_count,
    ROUND(COUNT(*) FILTER (WHERE is_profitable)::NUMERIC / COUNT(*) * 100, 2) AS win_rate,
    ROUND(AVG(actual_return), 4) AS avg_return,
    ROUND(AVG(actual_return) FILTER (WHERE is_profitable), 4) AS avg_win_return,
    ROUND(AVG(actual_return) FILTER (WHERE NOT is_profitable), 4) AS avg_loss_return
FROM reality_check
GROUP BY recommend_source
ORDER BY avg_return DESC;

CREATE OR REPLACE VIEW v_reality_check_rank_stats AS
SELECT
    recommend_rank,
    COUNT(*) AS total_count,
    ROUND(COUNT(*) FILTER (WHERE is_profitable)::NUMERIC / COUNT(*) * 100, 2) AS win_rate,
    ROUND(AVG(actual_return), 4) AS avg_return
FROM reality_check
WHERE recommend_rank IS NOT NULL AND recommend_rank <= 10
GROUP BY recommend_rank
ORDER BY recommend_rank;

CREATE OR REPLACE VIEW v_reality_check_recent_trend AS
SELECT
    check_date,
    recommend_source,
    COUNT(*) AS count,
    ROUND(COUNT(*) FILTER (WHERE is_profitable)::NUMERIC / COUNT(*) * 100, 2) AS win_rate,
    ROUND(AVG(actual_return), 4) AS avg_return
FROM reality_check
WHERE check_date >= CURRENT_DATE - INTERVAL '30 days'
GROUP BY check_date, recommend_source
ORDER BY check_date DESC, recommend_source;

COMMENT ON VIEW v_reality_check_daily_stats IS '일별 승률, 평균 수익률 등 주요 통계';

COMMENT ON VIEW v_reality_check_source_stats IS '추천 소스(screening/전략)별 성과 비교';

COMMENT ON VIEW v_reality_check_rank_stats IS '추천 순위별 성과 분석 (Top 10)';

COMMENT ON VIEW v_reality_check_recent_trend IS '최근 30일 성과 추이 (일별/소스별)';

CREATE TABLE IF NOT EXISTS symbol_global_score (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    symbol_info_id UUID NOT NULL REFERENCES symbol_info(id) ON DELETE CASCADE,

    overall_score NUMERIC(5, 2) NOT NULL,           -- 0 ~ 100.00
    grade VARCHAR(10) NOT NULL,                     -- BUY, WATCH, HOLD, AVOID
    confidence VARCHAR(10),                         -- HIGH, MEDIUM, LOW

    component_scores JSONB NOT NULL,                -- { "risk_reward": 85.5, "t1": 70.2, ... }

    penalties JSONB,                                -- { "near_52w_high": true, "low_liquidity": true, ... }

    market VARCHAR(20) NOT NULL,                    -- KR, US, JP
    ticker VARCHAR(20) NOT NULL,                    -- 005930, AAPL

    calculated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT unique_symbol_global_score UNIQUE (symbol_info_id)
);

CREATE INDEX IF NOT EXISTS idx_global_score_ranking
    ON symbol_global_score(market, grade, overall_score DESC);

CREATE INDEX IF NOT EXISTS idx_global_score_ticker
    ON symbol_global_score(ticker);

CREATE INDEX IF NOT EXISTS idx_global_score_calculated
    ON symbol_global_score(calculated_at DESC);

CREATE INDEX IF NOT EXISTS idx_global_score_components
    ON symbol_global_score USING gin(component_scores);

COMMENT ON TABLE symbol_global_score IS 'GlobalScore 계산 결과 저장 (Phase 1-D.5)';

COMMENT ON COLUMN symbol_global_score.overall_score IS '종합 점수 (0~100)';

COMMENT ON COLUMN symbol_global_score.grade IS '투자 등급 (BUY/WATCH/HOLD/AVOID)';

COMMENT ON COLUMN symbol_global_score.component_scores IS '팩터별 점수 { risk_reward, t1, stop_loss, ... }';

COMMENT ON COLUMN symbol_global_score.penalties IS '페널티 플래그 { near_52w_high, low_liquidity, ... }';

COMMENT ON COLUMN symbol_global_score.calculated_at IS '계산 시점 (캐시 TTL 판단용)';

CREATE OR REPLACE FUNCTION upsert_global_score(
    p_symbol_info_id UUID,
    p_overall_score NUMERIC,
    p_grade VARCHAR,
    p_confidence VARCHAR,
    p_component_scores JSONB,
    p_penalties JSONB,
    p_market VARCHAR,
    p_ticker VARCHAR
) RETURNS UUID AS $$
DECLARE
    v_id UUID;
BEGIN
    INSERT INTO symbol_global_score (
        symbol_info_id,
        overall_score,
        grade,
        confidence,
        component_scores,
        penalties,
        market,
        ticker,
        calculated_at,
        updated_at
    ) VALUES (
        p_symbol_info_id,
        p_overall_score,
        p_grade,
        p_confidence,
        p_component_scores,
        p_penalties,
        p_market,
        p_ticker,
        NOW(),
        NOW()
    )
    ON CONFLICT (symbol_info_id) DO UPDATE SET
        overall_score = EXCLUDED.overall_score,
        grade = EXCLUDED.grade,
        confidence = EXCLUDED.confidence,
        component_scores = EXCLUDED.component_scores,
        penalties = EXCLUDED.penalties,
        calculated_at = NOW(),
        updated_at = NOW()
    RETURNING id INTO v_id;

    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION upsert_global_score IS 'GlobalScore UPSERT (있으면 UPDATE, 없으면 INSERT)';

INSERT INTO schema_migrations (version, filename, success, applied_at)
VALUES (12, '12_ranking_system.sql', true, NOW())
ON CONFLICT (version) DO NOTHING;

CREATE TABLE IF NOT EXISTS score_history (
    id SERIAL PRIMARY KEY,
    symbol VARCHAR(20) NOT NULL,
    score_date DATE NOT NULL,
    global_score DECIMAL(5,2),
    route_state VARCHAR(20),
    rank INTEGER,
    component_scores JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(symbol, score_date)
);

CREATE INDEX IF NOT EXISTS idx_score_history_symbol_date 
ON score_history(symbol, score_date DESC);

CREATE INDEX IF NOT EXISTS idx_score_history_date 
ON score_history(score_date DESC);

CREATE INDEX IF NOT EXISTS idx_score_history_score 
ON score_history(score_date, global_score DESC);

COMMENT ON TABLE score_history IS '종목별 Global Score, RouteState, 순위의 일별 히스토리';

COMMENT ON COLUMN score_history.symbol IS '종목 코드';

COMMENT ON COLUMN score_history.score_date IS '점수 계산 날짜';

COMMENT ON COLUMN score_history.global_score IS 'Global Score (0-100)';

COMMENT ON COLUMN score_history.route_state IS 'RouteState (Attack/Armed/Watch/Wait/Danger)';

COMMENT ON COLUMN score_history.rank IS '해당 날짜의 순위';

COMMENT ON COLUMN score_history.component_scores IS '7 Factor 개별 점수 (JSON)';

