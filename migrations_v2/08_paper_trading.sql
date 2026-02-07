-- =============================================================================
-- 08_paper_trading
-- Mock 거래소, 전략-계정 연결, Paper Trading 세션
-- =============================================================================
-- 통합 마이그레이션 파일 (자동 생성)
-- 원본 파일: ["20_", "21_", "22_"]
-- =============================================================================

-- ---------------------------------------------------------------------------
-- Source: 20_mock_exchange_state
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS mock_exchange_state (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,
    current_balance DECIMAL(20, 8) NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT mock_exchange_state_unique UNIQUE (credential_id)
);

CREATE TABLE IF NOT EXISTS mock_positions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,
    symbol VARCHAR(50) NOT NULL,
    side VARCHAR(10) NOT NULL,
    quantity DECIMAL(20, 8) NOT NULL,
    entry_price DECIMAL(20, 8) NOT NULL,
    entry_time TIMESTAMPTZ NOT NULL,
    CONSTRAINT mock_positions_unique UNIQUE (credential_id, symbol)
);

CREATE TABLE IF NOT EXISTS mock_executions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,
    symbol VARCHAR(50) NOT NULL,
    side VARCHAR(10) NOT NULL,
    quantity DECIMAL(20, 8) NOT NULL,
    price DECIMAL(20, 8) NOT NULL,
    commission DECIMAL(20, 8) NOT NULL DEFAULT 0,
    realized_pnl DECIMAL(20, 8),
    executed_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_mock_exchange_state_credential ON mock_exchange_state(credential_id);

CREATE INDEX IF NOT EXISTS idx_mock_positions_credential ON mock_positions(credential_id);

CREATE INDEX IF NOT EXISTS idx_mock_executions_credential ON mock_executions(credential_id);

CREATE INDEX IF NOT EXISTS idx_mock_executions_executed_at ON mock_executions(credential_id, executed_at DESC);

COMMENT ON TABLE mock_exchange_state IS 'Mock 거래소 잔고 상태';

COMMENT ON TABLE mock_positions IS 'Mock 거래소 보유 포지션';

COMMENT ON TABLE mock_executions IS 'Mock 거래소 체결 내역';

-- ---------------------------------------------------------------------------
-- Source: 21_strategy_credential_link
-- ---------------------------------------------------------------------------

ALTER TABLE strategies
ADD COLUMN credential_id UUID REFERENCES exchange_credentials(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_strategies_credential_id ON strategies(credential_id);

COMMENT ON COLUMN strategies.credential_id IS '전략이 연결된 거래소 계정 ID (NULL이면 기본 활성 계정 사용)';

-- ---------------------------------------------------------------------------
-- Source: 22_paper_trading_strategy
-- ---------------------------------------------------------------------------

ALTER TABLE mock_positions
ADD COLUMN IF NOT EXISTS strategy_id VARCHAR(100);

ALTER TABLE mock_executions
ADD COLUMN IF NOT EXISTS strategy_id VARCHAR(100);

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

CREATE INDEX IF NOT EXISTS idx_mock_positions_strategy ON mock_positions(strategy_id);

CREATE INDEX IF NOT EXISTS idx_mock_executions_strategy ON mock_executions(strategy_id);

CREATE INDEX IF NOT EXISTS idx_paper_trading_sessions_strategy ON paper_trading_sessions(strategy_id);

CREATE INDEX IF NOT EXISTS idx_paper_trading_sessions_credential ON paper_trading_sessions(credential_id);

CREATE INDEX IF NOT EXISTS idx_paper_trading_sessions_status ON paper_trading_sessions(status);

COMMENT ON COLUMN mock_positions.strategy_id IS '포지션을 생성한 전략 ID';

COMMENT ON COLUMN mock_executions.strategy_id IS '체결을 생성한 전략 ID';

COMMENT ON TABLE paper_trading_sessions IS 'Paper Trading 전략별 세션 상태';

