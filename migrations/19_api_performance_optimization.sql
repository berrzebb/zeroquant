-- =====================================================
-- 19_api_performance_optimization.sql
-- API 서버 성능 최적화: 인덱스 + Materialized View
-- =====================================================
-- 포함 내용:
-- 1. position_snapshots: DISTINCT ON 최적화 인덱스
-- 2. symbol_fundamental: RouteState JOIN 최적화 인덱스
-- 3. 섹터 Relative Strength Materialized View
-- 4. 통계 갱신
-- =====================================================

-- =====================================================
-- 1. POSITION_SNAPSHOTS: DISTINCT ON 최적화 인덱스
-- =====================================================
-- get_current_positions()에서 DISTINCT ON (credential_id, symbol)
-- ORDER BY credential_id, symbol, snapshot_time DESC 사용
-- 기존 idx_position_snapshots_latest는 symbol이 없어 인덱스 효율이 낮음

CREATE INDEX IF NOT EXISTS idx_position_snapshots_current
ON position_snapshots(credential_id, symbol, snapshot_time DESC)
WHERE quantity > 0;

COMMENT ON INDEX idx_position_snapshots_current IS
'현재 포지션 조회 최적화 (DISTINCT ON credential_id, symbol)';

-- =====================================================
-- 2. SYMBOL_FUNDAMENTAL: RouteState JOIN 인덱스
-- =====================================================
-- get_top_ranked()에서 route_state 필터 시
-- symbol_fundamental JOIN + route_state::text 비교에 사용
-- 기존 인덱스는 WHERE route_state <> NEUTRAL 조건이 있어
-- 모든 상태를 커버하지 못함

CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_route_state_all
ON symbol_fundamental(symbol_info_id, route_state);

COMMENT ON INDEX idx_symbol_fundamental_route_state_all IS
'RouteState 기반 랭킹 필터 최적화 (전체 상태 커버)';

-- =====================================================
-- 3. 섹터 RELATIVE STRENGTH MATERIALIZED VIEW
-- =====================================================
-- calculate_sector_rs()의 4개 CTE (sector_prices → sector_returns
-- → sector_avg_returns → market_avg)를 사전 계산
-- Collector에서 주기적으로 REFRESH하여 API 응답 속도 향상

-- 3.1 KR 시장 섹터 RS
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

-- MV 인덱스
CREATE UNIQUE INDEX IF NOT EXISTS idx_mv_sector_rs_key
ON mv_sector_rs(sector, market, exchange);

CREATE INDEX IF NOT EXISTS idx_mv_sector_rs_composite
ON mv_sector_rs(composite_score DESC);

CREATE INDEX IF NOT EXISTS idx_mv_sector_rs_market
ON mv_sector_rs(market, composite_score DESC);

COMMENT ON MATERIALIZED VIEW mv_sector_rs IS
'섹터별 Relative Strength 사전 계산 - Collector에서 주기적 REFRESH';

-- 갱신 함수
CREATE OR REPLACE FUNCTION refresh_mv_sector_rs()
RETURNS void AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY mv_sector_rs;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION refresh_mv_sector_rs IS
'mv_sector_rs 갱신 (CONCURRENTLY - 읽기 차단 없음). OHLCV 업데이트 후 호출 권장.';

-- =====================================================
-- 4. 통계 갱신
-- =====================================================
ANALYZE position_snapshots;
ANALYZE symbol_fundamental;

-- =====================================================
-- 마이그레이션 기록
-- =====================================================
INSERT INTO schema_migrations (version, filename, success, applied_at)
VALUES (119, '19_api_performance_optimization.sql', true, NOW())
ON CONFLICT (version) DO NOTHING;

-- =====================================================
-- 사용 가이드
-- =====================================================
--
-- 1. 섹터 RS 갱신 (OHLCV 수집 후 권장):
--    SELECT refresh_mv_sector_rs();
--
-- 2. 갱신 확인:
--    SELECT * FROM mv_sector_rs ORDER BY composite_score DESC;
--
-- =====================================================
