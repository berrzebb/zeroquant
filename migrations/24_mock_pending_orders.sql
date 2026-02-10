-- Mock 미체결 주문 영속화 마이그레이션
-- Paper Trading 재시작 시 지정가/스톱 주문과 잔고 예약을 복원합니다.

-- 1. Mock 미체결 주문 테이블
CREATE TABLE IF NOT EXISTS mock_pending_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,
    strategy_id VARCHAR(100) NOT NULL,
    order_id VARCHAR(100) NOT NULL UNIQUE,
    symbol VARCHAR(50) NOT NULL,
    side VARCHAR(10) NOT NULL,
    order_type VARCHAR(30) NOT NULL,
    quantity DECIMAL(20, 8) NOT NULL,
    remaining_quantity DECIMAL(20, 8) NOT NULL,
    price DECIMAL(20, 8),
    stop_price DECIMAL(20, 8),
    reserved_amount DECIMAL(20, 8) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL
);

-- 2. Paper Trading 세션에 예약 잔고 추가
ALTER TABLE paper_trading_sessions
ADD COLUMN IF NOT EXISTS reserved_balance DECIMAL(20, 8) NOT NULL DEFAULT 0;

-- 3. 인덱스 생성
CREATE INDEX IF NOT EXISTS idx_mock_pending_orders_credential ON mock_pending_orders(credential_id);
CREATE INDEX IF NOT EXISTS idx_mock_pending_orders_strategy ON mock_pending_orders(strategy_id);
CREATE INDEX IF NOT EXISTS idx_mock_pending_orders_symbol ON mock_pending_orders(symbol);

-- 4. 코멘트
COMMENT ON TABLE mock_pending_orders IS 'Mock 거래소 미체결 주문 (지정가/스톱)';
COMMENT ON COLUMN mock_pending_orders.reserved_amount IS '주문에 예약된 잔고 금액';
COMMENT ON COLUMN paper_trading_sessions.reserved_balance IS '전략의 총 예약 잔고 (미체결 지정가 주문)';
