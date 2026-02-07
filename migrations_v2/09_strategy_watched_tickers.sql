-- =============================================================================
-- 09_strategy_watched_tickers
-- 전략별 관심 종목 관리 (Collector 우선순위 연동)
-- =============================================================================

-- 전략이 관심을 가지는 종목 목록.
-- 고정 티커(config)와 동적 티커(스크리닝/유니버스)를 모두 지원합니다.
-- Collector가 이 테이블을 읽어 해당 종목의 OHLCV/지표/스코어 데이터를
-- 우선적으로 업데이트합니다.
CREATE TABLE IF NOT EXISTS strategy_watched_tickers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id VARCHAR(100) NOT NULL,
    ticker VARCHAR(50) NOT NULL,
    source VARCHAR(20) NOT NULL DEFAULT 'config',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT strategy_watched_tickers_unique UNIQUE (strategy_id, ticker)
);

CREATE INDEX IF NOT EXISTS idx_strategy_watched_tickers_strategy
    ON strategy_watched_tickers(strategy_id);

CREATE INDEX IF NOT EXISTS idx_strategy_watched_tickers_ticker
    ON strategy_watched_tickers(ticker);

COMMENT ON TABLE strategy_watched_tickers IS '전략별 관심 종목 (Collector 우선순위 연동)';
COMMENT ON COLUMN strategy_watched_tickers.strategy_id IS '전략 ID';
COMMENT ON COLUMN strategy_watched_tickers.ticker IS '종목 코드';
COMMENT ON COLUMN strategy_watched_tickers.source IS '출처: config(고정), dynamic(스크리닝/유니버스)';
