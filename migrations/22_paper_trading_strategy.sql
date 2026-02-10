-- Paper Trading 전략 기반 마이그레이션
-- mock_positions와 mock_executions에 strategy_id 추가하여 전략별 추적 가능하게 함

-- 1. mock_positions에 strategy_id 추가
ALTER TABLE mock_positions
ADD COLUMN IF NOT EXISTS strategy_id VARCHAR(100);

-- 2. mock_executions에 strategy_id 추가
ALTER TABLE mock_executions
ADD COLUMN IF NOT EXISTS strategy_id VARCHAR(100);

-- 3. Paper Trading 세션 테이블 (전략별 실행 상태 관리)
CREATE TABLE IF NOT EXISTS paper_trading_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id VARCHAR(100) NOT NULL,
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL DEFAULT 'stopped', -- running, stopped, paused
    initial_balance DECIMAL(20, 8) NOT NULL,
    current_balance DECIMAL(20, 8) NOT NULL,
    started_at TIMESTAMPTZ,
    stopped_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT paper_trading_sessions_unique UNIQUE (strategy_id)
);

-- 4. 인덱스 생성
CREATE INDEX IF NOT EXISTS idx_mock_positions_strategy ON mock_positions(strategy_id);
CREATE INDEX IF NOT EXISTS idx_mock_executions_strategy ON mock_executions(strategy_id);
CREATE INDEX IF NOT EXISTS idx_paper_trading_sessions_strategy ON paper_trading_sessions(strategy_id);
CREATE INDEX IF NOT EXISTS idx_paper_trading_sessions_credential ON paper_trading_sessions(credential_id);
CREATE INDEX IF NOT EXISTS idx_paper_trading_sessions_status ON paper_trading_sessions(status);

-- 5. 코멘트
COMMENT ON COLUMN mock_positions.strategy_id IS '포지션을 생성한 전략 ID';
COMMENT ON COLUMN mock_executions.strategy_id IS '체결을 생성한 전략 ID';
COMMENT ON TABLE paper_trading_sessions IS 'Paper Trading 전략별 세션 상태';
