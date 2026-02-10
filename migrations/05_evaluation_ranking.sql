-- =====================================================
-- 05_evaluation_ranking.sql (통합)
-- 검증 및 랭킹 시스템 스키마
-- =====================================================
-- 원본: 10_reality_check.sql, 12_ranking_system.sql, 20_score_history.sql
-- =====================================================

-- =====================================================
-- 10_reality_check.sql
-- Reality Check 추천 검증 시스템
-- =====================================================
--
-- 원본 마이그레이션: 028_reality_check_system.sql, 032_fix_hypertable_declarations.sql
--
-- 포함 내용:
-- - price_snapshot: 추천 종목 가격 스냅샷 (TimescaleDB Hypertable)
-- - reality_check: 추천 종목 실제 성과 검증 결과 (TimescaleDB Hypertable)
-- - 자동 계산 함수 (calculate_reality_check)
-- - 통계 집계 뷰 4개 (일별, 소스별, 랭크별, 최근 추이)
-- - Hypertable 압축 및 보존 정책
--
-- 목적: 전일 추천 종목의 익일 실제 성과 자동 검증
-- 활용: 전략 신뢰도 측정, 백테스트 vs 실거래 괴리 분석
--
-- =====================================================

-- =====================================================
-- PRICE_SNAPSHOT TABLE
-- 추천 종목 가격 스냅샷 - TimescaleDB Hypertable
-- =====================================================

CREATE TABLE IF NOT EXISTS price_snapshot (
    snapshot_date DATE NOT NULL,                    -- 스냅샷 일자 (장 마감일)
    symbol VARCHAR(20) NOT NULL,                    -- 종목 코드

    -- 가격 정보
    close_price DECIMAL(20, 4) NOT NULL,            -- 종가
    volume BIGINT,                                  -- 거래량

    -- 추천 정보
    recommend_source VARCHAR(50),                   -- 추천 소스 (screening, strategy_xyz)
    recommend_rank INT,                             -- 추천 순위 (1~N)
    recommend_score DECIMAL(5, 2),                  -- 추천 점수 (0~100)

    -- 예측 정보 (선택적)
    expected_return DECIMAL(8, 4),                  -- 기대 수익률 (%)
    expected_holding_days INT,                      -- 예상 보유 기간

    -- 메타데이터
    market VARCHAR(20),                             -- 시장 (KR, US, CRYPTO)
    sector VARCHAR(50),                             -- 섹터

    -- 시스템 필드
    created_at TIMESTAMPTZ DEFAULT NOW(),

    PRIMARY KEY (snapshot_date, symbol, recommend_source)
);

-- TimescaleDB Hypertable 변환 (1개월 단위 청크)
SELECT create_hypertable('price_snapshot', 'snapshot_date',
    chunk_time_interval => INTERVAL '1 month',
    if_not_exists => TRUE
);

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_price_snapshot_symbol
    ON price_snapshot(symbol, snapshot_date DESC);
CREATE INDEX IF NOT EXISTS idx_price_snapshot_source
    ON price_snapshot(recommend_source, snapshot_date DESC);
CREATE INDEX IF NOT EXISTS idx_price_snapshot_rank
    ON price_snapshot(recommend_rank)
    WHERE recommend_rank <= 10;                     -- Top 10 추천만 인덱싱

-- 압축 설정 (032)
ALTER TABLE price_snapshot SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol, recommend_source'
);

-- 압축 정책: 30일 이상 데이터 압축
SELECT add_compression_policy('price_snapshot',
    INTERVAL '30 days',
    if_not_exists => TRUE
);

-- 보존 정책: 1년 이상 데이터 삭제
SELECT add_retention_policy('price_snapshot',
    INTERVAL '1 year',
    if_not_exists => TRUE
);

COMMENT ON TABLE price_snapshot IS 'Reality Check - 추천 종목 가격 스냅샷 (TimescaleDB Hypertable, 1년 보존)';
COMMENT ON COLUMN price_snapshot.recommend_source IS '추천 소스 (screening_momentum, strategy_rsi, strategy_sma 등)';
COMMENT ON COLUMN price_snapshot.recommend_score IS '추천 점수 (0~100, 높을수록 강한 추천)';

-- =====================================================
-- REALITY_CHECK TABLE
-- 추천 종목 실제 성과 검증 결과 - TimescaleDB Hypertable
-- =====================================================

CREATE TABLE IF NOT EXISTS reality_check (
    check_date DATE NOT NULL,                       -- 검증 일자 (익일 장 마감일)
    recommend_date DATE NOT NULL,                   -- 추천 일자
    symbol VARCHAR(20) NOT NULL,

    -- 추천 정보 (price_snapshot에서 복사)
    recommend_source VARCHAR(50),
    recommend_rank INT,
    recommend_score DECIMAL(5, 2),

    -- 가격 정보
    entry_price DECIMAL(20, 4) NOT NULL,            -- 진입가 (추천일 종가)
    exit_price DECIMAL(20, 4) NOT NULL,             -- 청산가 (검증일 종가)

    -- 성과 지표
    actual_return DECIMAL(8, 4) NOT NULL,           -- 실제 수익률 (%)
    is_profitable BOOLEAN NOT NULL,                 -- 수익 여부

    -- 거래량 변화
    entry_volume BIGINT,
    exit_volume BIGINT,
    volume_change DECIMAL(8, 4),                    -- 거래량 변화율 (%)

    -- 예측 vs 실제 비교 (선택적)
    expected_return DECIMAL(8, 4),                  -- 기대 수익률
    return_error DECIMAL(8, 4),                     -- 오차율 (actual - expected)

    -- 시계열 정보 (OHLCV 기반 계산 가능)
    max_profit DECIMAL(8, 4),                       -- 최대 수익률 (보유 기간 중)
    max_drawdown DECIMAL(8, 4),                     -- 최대 하락률
    volatility DECIMAL(8, 4),                       -- 변동성 (표준편차)

    -- 메타데이터
    market VARCHAR(20),
    sector VARCHAR(50),

    -- 시스템 필드
    created_at TIMESTAMPTZ DEFAULT NOW(),

    PRIMARY KEY (check_date, symbol, recommend_source)
);

-- TimescaleDB Hypertable 변환 (1개월 단위 청크)
SELECT create_hypertable('reality_check', 'check_date',
    chunk_time_interval => INTERVAL '1 month',
    if_not_exists => TRUE
);

-- 인덱스
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

-- 압축 설정 (032)
ALTER TABLE reality_check SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol, recommend_source'
);

-- 압축 정책: 30일 이상 데이터 압축
SELECT add_compression_policy('reality_check',
    INTERVAL '30 days',
    if_not_exists => TRUE
);

-- 보존 정책: 1년 이상 데이터 삭제
SELECT add_retention_policy('reality_check',
    INTERVAL '1 year',
    if_not_exists => TRUE
);

COMMENT ON TABLE reality_check IS 'Reality Check - 추천 검증 결과 (TimescaleDB Hypertable, 1년 보존)';
COMMENT ON COLUMN reality_check.actual_return IS '실제 수익률 (%) = (exit_price - entry_price) / entry_price * 100';
COMMENT ON COLUMN reality_check.return_error IS '예측 오차 = 실제 수익률 - 기대 수익률';

-- =====================================================
-- FUNCTIONS
-- =====================================================

-- Reality Check 계산 함수: 전일 추천 종목의 금일 성과 자동 계산
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
    -- 전일 스냅샷과 금일 가격을 조인하여 성과 계산
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

-- =====================================================
-- ANALYTICAL VIEWS
-- =====================================================

-- 일별 승률 및 평균 수익률
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

-- 추천 소스별 성과
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

-- 랭크별 성과 (Top 10 추천의 성과 차이)
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

-- 최근 30일 추이
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

-- =====================================================
-- 사용 예시
-- =====================================================
--
-- 1. 스냅샷 저장 (매일 장 마감 후 실행):
--    INSERT INTO price_snapshot (snapshot_date, symbol, close_price, recommend_source, recommend_rank, recommend_score)
--    SELECT CURRENT_DATE, ticker, current_price, 'screening_momentum', ROW_NUMBER() OVER (ORDER BY score DESC), score
--    FROM screening_result WHERE score >= 70 LIMIT 20;
--
-- 2. Reality Check 계산 (익일 장 마감 후 실행):
--    SELECT * FROM calculate_reality_check(CURRENT_DATE - INTERVAL '1 day', CURRENT_DATE);
--
-- 3. 통계 조회:
--    SELECT * FROM v_reality_check_daily_stats LIMIT 7;        -- 최근 7일 성과
--    SELECT * FROM v_reality_check_source_stats;               -- 소스별 성과
--    SELECT * FROM v_reality_check_rank_stats;                 -- 랭크별 성과
--    SELECT * FROM v_reality_check_recent_trend WHERE recommend_source = 'screening_momentum';
--
-- =====================================================

-- ============ 12_ranking_system.sql ============

-- =====================================================
-- 12_ranking_system.sql
-- GlobalScore 랭킹 시스템
-- =====================================================
--
-- 포함 내용:
-- - symbol_global_score: GlobalScore 계산 결과 저장
-- - 인덱스: 랭킹 조회 최적화
-- - 함수: 스코어 업데이트 헬퍼
--
-- =====================================================

-- =====================================================
-- SYMBOL_GLOBAL_SCORE TABLE
-- GlobalScore 계산 결과 저장
-- =====================================================

CREATE TABLE IF NOT EXISTS symbol_global_score (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- symbol_info 연계 (FK)
    symbol_info_id UUID NOT NULL REFERENCES symbol_info(id) ON DELETE CASCADE,

    -- GlobalScore 결과
    overall_score NUMERIC(5, 2) NOT NULL,           -- 0 ~ 100.00
    grade VARCHAR(10) NOT NULL,                     -- BUY, WATCH, HOLD, AVOID
    confidence VARCHAR(10),                         -- HIGH, MEDIUM, LOW

    -- 팩터 점수 (JSONB)
    component_scores JSONB NOT NULL,                -- { "risk_reward": 85.5, "t1": 70.2, ... }

    -- 페널티 (JSONB)
    penalties JSONB,                                -- { "near_52w_high": true, "low_liquidity": true, ... }

    -- 시장 정보 (비정규화 - 조회 최적화)
    market VARCHAR(20) NOT NULL,                    -- KR, US, JP
    ticker VARCHAR(20) NOT NULL,                    -- 005930, AAPL

    -- 계산 시점 (캐시 TTL 용)
    calculated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 시스템 필드
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- 유일 제약 (심볼당 1개 레코드)
    CONSTRAINT unique_symbol_global_score UNIQUE (symbol_info_id)
);

-- 인덱스: 랭킹 조회 최적화
CREATE INDEX idx_global_score_ranking
    ON symbol_global_score(market, grade, overall_score DESC);

CREATE INDEX idx_global_score_ticker
    ON symbol_global_score(ticker);

CREATE INDEX idx_global_score_calculated
    ON symbol_global_score(calculated_at DESC);

-- JSONB GIN 인덱스 (컴포넌트 필터링용)
CREATE INDEX idx_global_score_components
    ON symbol_global_score USING gin(component_scores);

COMMENT ON TABLE symbol_global_score IS 'GlobalScore 계산 결과 저장 (Phase 1-D.5)';
COMMENT ON COLUMN symbol_global_score.overall_score IS '종합 점수 (0~100)';
COMMENT ON COLUMN symbol_global_score.grade IS '투자 등급 (BUY/WATCH/HOLD/AVOID)';
COMMENT ON COLUMN symbol_global_score.component_scores IS '팩터별 점수 { risk_reward, t1, stop_loss, ... }';
COMMENT ON COLUMN symbol_global_score.penalties IS '페널티 플래그 { near_52w_high, low_liquidity, ... }';
COMMENT ON COLUMN symbol_global_score.calculated_at IS '계산 시점 (캐시 TTL 판단용)';

-- =====================================================
-- 스코어 업데이트 함수 (UPSERT 헬퍼)
-- =====================================================

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

-- =====================================================
-- schema_migrations 기록
-- =====================================================

INSERT INTO schema_migrations (version, filename, success, applied_at)
VALUES (12, '12_ranking_system.sql', true, NOW())
ON CONFLICT (version) DO NOTHING;

-- ============ 20_score_history.sql ============

-- Score History 테이블
-- 종목별 Global Score, RouteState, 순위의 일별 히스토리 저장

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

-- 인덱스: 종목+날짜 조회 최적화
CREATE INDEX IF NOT EXISTS idx_score_history_symbol_date 
ON score_history(symbol, score_date DESC);

-- 인덱스: 날짜별 조회 (전체 순위 조회)
CREATE INDEX IF NOT EXISTS idx_score_history_date 
ON score_history(score_date DESC);

-- 인덱스: 점수 기반 필터링
CREATE INDEX IF NOT EXISTS idx_score_history_score 
ON score_history(score_date, global_score DESC);

COMMENT ON TABLE score_history IS '종목별 Global Score, RouteState, 순위의 일별 히스토리';
COMMENT ON COLUMN score_history.symbol IS '종목 코드';
COMMENT ON COLUMN score_history.score_date IS '점수 계산 날짜';
COMMENT ON COLUMN score_history.global_score IS 'Global Score (0-100)';
COMMENT ON COLUMN score_history.route_state IS 'RouteState (Attack/Armed/Watch/Wait/Danger)';
COMMENT ON COLUMN score_history.rank IS '해당 날짜의 순위';
COMMENT ON COLUMN score_history.component_scores IS '7 Factor 개별 점수 (JSON)';
