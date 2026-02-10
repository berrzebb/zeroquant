-- Mock 거래소 상태 테이블
-- UI에서 등록한 Mock 거래소의 잔고/포지션/체결 내역을 영속화합니다.

-- Mock 거래소 잔고 상태
CREATE TABLE IF NOT EXISTS mock_exchange_state (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,
    current_balance DECIMAL(20, 8) NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT mock_exchange_state_unique UNIQUE (credential_id)
);

-- Mock 포지션
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

-- Mock 체결 내역
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

-- 인덱스 생성
CREATE INDEX IF NOT EXISTS idx_mock_exchange_state_credential ON mock_exchange_state(credential_id);
CREATE INDEX IF NOT EXISTS idx_mock_positions_credential ON mock_positions(credential_id);
CREATE INDEX IF NOT EXISTS idx_mock_executions_credential ON mock_executions(credential_id);
CREATE INDEX IF NOT EXISTS idx_mock_executions_executed_at ON mock_executions(credential_id, executed_at DESC);

COMMENT ON TABLE mock_exchange_state IS 'Mock 거래소 잔고 상태';
COMMENT ON TABLE mock_positions IS 'Mock 거래소 보유 포지션';
COMMENT ON TABLE mock_executions IS 'Mock 거래소 체결 내역';
