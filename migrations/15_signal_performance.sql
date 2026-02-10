-- =====================================================
-- 15_signal_performance.sql
-- 신호 성과 추적 테이블
-- =====================================================
-- 신호 발생 후 실제 수익률을 추적하여 신호 품질 분석 지원
-- =====================================================

-- =====================================================
-- SIGNAL_PERFORMANCE TABLE
-- 신호별 성과 데이터 (신호 발생 후 N일 수익률)
-- =====================================================

CREATE TABLE IF NOT EXISTS signal_performance (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- 신호 참조
    signal_id UUID NOT NULL REFERENCES signal_marker(id) ON DELETE CASCADE,

    -- 심볼 정보 (조인 없이 빠른 조회용)
    symbol_id UUID NOT NULL REFERENCES symbol_info(id) ON DELETE CASCADE,
    ticker VARCHAR(50) NOT NULL,

    -- 가격 정보
    signal_price NUMERIC(20, 8) NOT NULL,           -- 신호 발생 시 가격
    price_1d NUMERIC(20, 8),                        -- 1일 후 가격
    price_3d NUMERIC(20, 8),                        -- 3일 후 가격
    price_5d NUMERIC(20, 8),                        -- 5일 후 가격
    price_10d NUMERIC(20, 8),                       -- 10일 후 가격
    price_20d NUMERIC(20, 8),                       -- 20일 후 가격

    -- 수익률 (%)
    return_1d NUMERIC(10, 4),                       -- 1일 수익률
    return_3d NUMERIC(10, 4),                       -- 3일 수익률
    return_5d NUMERIC(10, 4),                       -- 5일 수익률
    return_10d NUMERIC(10, 4),                      -- 10일 수익률
    return_20d NUMERIC(10, 4),                      -- 20일 수익률

    -- 최대 수익/손실 (기간 내)
    max_return NUMERIC(10, 4),                      -- 최대 수익률 (MFE)
    max_drawdown NUMERIC(10, 4),                    -- 최대 손실률 (MAE)

    -- 신호 메타데이터 (분석용 캐시)
    signal_type VARCHAR(20) NOT NULL,               -- Entry, Exit 등
    side VARCHAR(10),                               -- Buy, Sell
    strength NUMERIC(5, 4) NOT NULL,                -- 신호 강도 (0.0 ~ 1.0)
    strategy_id VARCHAR(100) NOT NULL,

    -- 성공 여부 판정
    is_winner BOOLEAN,                              -- 승리 여부 (return_5d > 0 기준)

    -- 메타데이터
    calculated_at TIMESTAMPTZ,                      -- 성과 계산 시점
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 유니크 제약조건 (신호당 하나의 성과)
    CONSTRAINT unique_signal_performance UNIQUE(signal_id)
);

-- 인덱스
CREATE INDEX idx_signal_performance_symbol ON signal_performance(symbol_id, created_at DESC);
CREATE INDEX idx_signal_performance_ticker ON signal_performance(ticker, created_at DESC);
CREATE INDEX idx_signal_performance_signal_type ON signal_performance(signal_type, side);
CREATE INDEX idx_signal_performance_strategy ON signal_performance(strategy_id);
CREATE INDEX idx_signal_performance_strength ON signal_performance(strength);
CREATE INDEX idx_signal_performance_winner ON signal_performance(is_winner) WHERE is_winner IS NOT NULL;
CREATE INDEX idx_signal_performance_calculated ON signal_performance(calculated_at) WHERE calculated_at IS NULL;

-- 코멘트
COMMENT ON TABLE signal_performance IS '신호별 성과 추적 (신호 품질 분석용)';
COMMENT ON COLUMN signal_performance.return_1d IS '1일 수익률 (%, 매도 신호는 부호 반전)';
COMMENT ON COLUMN signal_performance.max_return IS '최대 유리 변동률 MFE (Maximum Favorable Excursion)';
COMMENT ON COLUMN signal_performance.max_drawdown IS '최대 불리 변동률 MAE (Maximum Adverse Excursion)';
COMMENT ON COLUMN signal_performance.is_winner IS '승리 여부 (5일 수익률 기준, 매수: >0, 매도: <0)';

-- updated_at 자동 업데이트 트리거
CREATE OR REPLACE FUNCTION update_signal_performance_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_signal_performance_timestamp
    BEFORE UPDATE ON signal_performance
    FOR EACH ROW
    EXECUTE FUNCTION update_signal_performance_timestamp();

-- =====================================================
-- 신호 성과 통계 뷰
-- =====================================================

-- 신호 타입별 통계 뷰
CREATE OR REPLACE VIEW v_signal_type_stats AS
SELECT
    signal_type,
    side,
    COUNT(*) as total_signals,
    COUNT(*) FILTER (WHERE is_winner = true) as win_count,
    COUNT(*) FILTER (WHERE is_winner = false) as loss_count,
    ROUND(
        100.0 * COUNT(*) FILTER (WHERE is_winner = true) / NULLIF(COUNT(*) FILTER (WHERE is_winner IS NOT NULL), 0),
        2
    ) as win_rate,
    ROUND(AVG(return_1d)::NUMERIC, 4) as avg_return_1d,
    ROUND(AVG(return_5d)::NUMERIC, 4) as avg_return_5d,
    ROUND(AVG(return_10d)::NUMERIC, 4) as avg_return_10d,
    ROUND(AVG(max_return)::NUMERIC, 4) as avg_max_return,
    ROUND(AVG(max_drawdown)::NUMERIC, 4) as avg_max_drawdown
FROM signal_performance
WHERE calculated_at IS NOT NULL
GROUP BY signal_type, side;

COMMENT ON VIEW v_signal_type_stats IS '신호 타입별 성과 통계 (승률, 평균 수익률)';

-- 신호 강도별 통계 뷰
CREATE OR REPLACE VIEW v_signal_strength_stats AS
SELECT
    CASE
        WHEN strength >= 0.9 THEN '90-100'
        WHEN strength >= 0.8 THEN '80-90'
        WHEN strength >= 0.7 THEN '70-80'
        WHEN strength >= 0.6 THEN '60-70'
        ELSE '50-60'
    END as strength_range,
    side,
    COUNT(*) as total_signals,
    ROUND(
        100.0 * COUNT(*) FILTER (WHERE is_winner = true) / NULLIF(COUNT(*) FILTER (WHERE is_winner IS NOT NULL), 0),
        2
    ) as win_rate,
    ROUND(AVG(return_5d)::NUMERIC, 4) as avg_return_5d,
    ROUND(AVG(max_return)::NUMERIC, 4) as avg_max_return,
    ROUND(AVG(max_drawdown)::NUMERIC, 4) as avg_max_drawdown
FROM signal_performance
WHERE calculated_at IS NOT NULL
GROUP BY
    CASE
        WHEN strength >= 0.9 THEN '90-100'
        WHEN strength >= 0.8 THEN '80-90'
        WHEN strength >= 0.7 THEN '70-80'
        WHEN strength >= 0.6 THEN '60-70'
        ELSE '50-60'
    END,
    side
ORDER BY strength_range DESC;

COMMENT ON VIEW v_signal_strength_stats IS '신호 강도별 성과 통계 (강도-수익률 상관관계)';

-- 심볼별 신호 성과 뷰
CREATE OR REPLACE VIEW v_signal_symbol_stats AS
SELECT
    sp.ticker,
    si.name as symbol_name,
    si.market,
    COUNT(*) as total_signals,
    COUNT(*) FILTER (WHERE sp.side = 'Buy') as buy_count,
    COUNT(*) FILTER (WHERE sp.side = 'Sell') as sell_count,
    ROUND(
        100.0 * COUNT(*) FILTER (WHERE sp.is_winner = true) / NULLIF(COUNT(*) FILTER (WHERE sp.is_winner IS NOT NULL), 0),
        2
    ) as win_rate,
    ROUND(AVG(sp.return_5d)::NUMERIC, 4) as avg_return_5d,
    ROUND(AVG(sp.strength)::NUMERIC, 4) as avg_strength
FROM signal_performance sp
JOIN symbol_info si ON sp.symbol_id = si.id
WHERE sp.calculated_at IS NOT NULL
GROUP BY sp.ticker, si.name, si.market
ORDER BY total_signals DESC;

COMMENT ON VIEW v_signal_symbol_stats IS '심볼별 신호 성과 통계';

-- 전략별 신호 성과 뷰
CREATE OR REPLACE VIEW v_signal_strategy_stats AS
SELECT
    strategy_id,
    COUNT(*) as total_signals,
    COUNT(*) FILTER (WHERE is_winner = true) as win_count,
    ROUND(
        100.0 * COUNT(*) FILTER (WHERE is_winner = true) / NULLIF(COUNT(*) FILTER (WHERE is_winner IS NOT NULL), 0),
        2
    ) as win_rate,
    ROUND(AVG(return_1d)::NUMERIC, 4) as avg_return_1d,
    ROUND(AVG(return_5d)::NUMERIC, 4) as avg_return_5d,
    ROUND(AVG(strength)::NUMERIC, 4) as avg_strength,
    ROUND(AVG(max_return)::NUMERIC, 4) as avg_mfe,
    ROUND(AVG(max_drawdown)::NUMERIC, 4) as avg_mae
FROM signal_performance
WHERE calculated_at IS NOT NULL
GROUP BY strategy_id
ORDER BY win_rate DESC NULLS LAST;

COMMENT ON VIEW v_signal_strategy_stats IS '전략별 신호 성과 통계';
