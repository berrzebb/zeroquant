-- =============================================================================
-- 01_core_foundation
-- Extensions, ENUM, symbols, credentials
-- =============================================================================
-- 통합 마이그레이션 파일 (자동 생성)
-- 원본 파일: ["01_"]
-- =============================================================================

-- ---------------------------------------------------------------------------
-- Source: 01_core_foundation
-- ---------------------------------------------------------------------------

CREATE EXTENSION IF NOT EXISTS timescaledb;

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

DO $$ BEGIN
    CREATE TYPE market_type AS ENUM ('crypto', 'stock', 'forex', 'futures');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$

DO $$ BEGIN
    CREATE TYPE order_side AS ENUM ('buy', 'sell');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$

DO $$ BEGIN
    CREATE TYPE order_type AS ENUM ('market', 'limit', 'stop', 'stop_limit', 'trailing_stop');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$

DO $$ BEGIN
    CREATE TYPE order_status AS ENUM ('pending', 'open', 'partially_filled', 'filled', 'cancelled', 'rejected', 'expired');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$

DO $$ BEGIN
    CREATE TYPE order_time_in_force AS ENUM ('gtc', 'ioc', 'fok', 'day');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$

DO $$ BEGIN
    CREATE TYPE signal_type AS ENUM ('entry', 'exit', 'add_to_position', 'reduce_position', 'scale');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$

CREATE TABLE IF NOT EXISTS symbols (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    base VARCHAR(20) NOT NULL,                      -- 기준 자산 (예: BTC, AAPL)
    quote VARCHAR(20) NOT NULL,                     -- 결제 자산 (예: USD, KRW)
    market_type market_type NOT NULL,               -- 시장 유형
    exchange VARCHAR(50) NOT NULL,                  -- 거래소 (binance, kis 등)
    exchange_symbol VARCHAR(50),                    -- 거래소별 심볼 코드
    is_active BOOLEAN DEFAULT true,                 -- 활성화 여부
    min_quantity DECIMAL(30, 15),                   -- 최소 주문 수량
    max_quantity DECIMAL(30, 15),                   -- 최대 주문 수량
    quantity_step DECIMAL(30, 15),                  -- 수량 단위
    min_notional DECIMAL(30, 15),                   -- 최소 주문 금액
    price_precision INT,                            -- 가격 소수점 자릿수
    quantity_precision INT,                         -- 수량 소수점 자릿수
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(base, quote, market_type, exchange)
);

CREATE INDEX IF NOT EXISTS idx_symbols_active ON symbols(is_active) WHERE is_active = true;

CREATE INDEX IF NOT EXISTS idx_symbols_exchange ON symbols(exchange);

COMMENT ON TABLE symbols IS '거래 가능한 심볼(종목) 메타데이터';

COMMENT ON COLUMN symbols.min_notional IS '최소 주문 금액 (quantity * price)';

CREATE TABLE IF NOT EXISTS trade_ticks (
    time TIMESTAMPTZ NOT NULL,                      -- 체결 시간
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    exchange_trade_id VARCHAR(100) NOT NULL,        -- 거래소 거래 ID
    price DECIMAL(30, 15) NOT NULL,
    quantity DECIMAL(30, 15) NOT NULL,
    is_buyer_maker BOOLEAN,                         -- 매수자가 메이커인지 여부
    PRIMARY KEY (symbol_id, time, exchange_trade_id)
);

SELECT create_hypertable('trade_ticks', 'time', chunk_time_interval => INTERVAL '1 day');

CREATE INDEX IF NOT EXISTS idx_trade_ticks_symbol ON trade_ticks(symbol_id, time DESC);

ALTER TABLE trade_ticks SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol_id'
);

SELECT add_compression_policy('trade_ticks', INTERVAL '7 days');

COMMENT ON TABLE trade_ticks IS '거래소 실시간 체결 데이터 (TimescaleDB Hypertable)';

CREATE TABLE IF NOT EXISTS orders (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    exchange VARCHAR(50) NOT NULL,                  -- 주문 거래소
    exchange_order_id VARCHAR(100),                 -- 거래소 주문 ID
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    side order_side NOT NULL,                       -- 매수/매도
    order_type order_type NOT NULL,                 -- 주문 타입
    status order_status NOT NULL DEFAULT 'pending', -- 주문 상태
    time_in_force order_time_in_force DEFAULT 'gtc',
    quantity DECIMAL(30, 15) NOT NULL,              -- 주문 수량
    filled_quantity DECIMAL(30, 15) DEFAULT 0,      -- 체결 수량
    price DECIMAL(30, 15),                          -- 지정가 (limit 주문)
    stop_price DECIMAL(30, 15),                     -- 스톱 가격 (stop 주문)
    average_fill_price DECIMAL(30, 15),             -- 평균 체결가
    strategy_id VARCHAR(100),                       -- 전략 ID (자동 주문)
    client_order_id VARCHAR(100),                   -- 클라이언트 주문 ID
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    filled_at TIMESTAMPTZ,                          -- 완전 체결 시간
    cancelled_at TIMESTAMPTZ,                       -- 취소 시간
    metadata JSONB DEFAULT '{}'                     -- 추가 메타데이터
);

CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status) WHERE status IN ('pending', 'open', 'partially_filled');

CREATE INDEX IF NOT EXISTS idx_orders_strategy ON orders(strategy_id) WHERE strategy_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_orders_symbol ON orders(symbol_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_orders_exchange ON orders(exchange, exchange_order_id);

COMMENT ON TABLE orders IS '주문 정보 (실시간 주문 추적)';

CREATE TABLE IF NOT EXISTS trades (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID NOT NULL REFERENCES orders(id),
    exchange VARCHAR(50) NOT NULL,
    exchange_trade_id VARCHAR(100) NOT NULL,        -- 거래소 거래 ID
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    side order_side NOT NULL,
    quantity DECIMAL(30, 15) NOT NULL,              -- 체결 수량
    price DECIMAL(30, 15) NOT NULL,                 -- 체결 가격
    fee DECIMAL(30, 15) DEFAULT 0,                  -- 수수료
    fee_currency VARCHAR(20),                       -- 수수료 통화
    is_maker BOOLEAN DEFAULT false,                 -- 메이커 거래 여부
    executed_at TIMESTAMPTZ NOT NULL,               -- 체결 시간
    created_at TIMESTAMPTZ DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_trades_order ON trades(order_id);

CREATE INDEX IF NOT EXISTS idx_trades_symbol ON trades(symbol_id, executed_at DESC);

CREATE INDEX IF NOT EXISTS idx_trades_executed ON trades(executed_at DESC);

COMMENT ON TABLE trades IS '실제 체결된 거래 내역';

CREATE TABLE IF NOT EXISTS positions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    exchange VARCHAR(50) NOT NULL,
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    side order_side NOT NULL,                       -- 롱/숏 (crypto는 buy만)
    quantity DECIMAL(30, 15) NOT NULL,              -- 보유 수량
    entry_price DECIMAL(30, 15) NOT NULL,           -- 평균 진입가
    current_price DECIMAL(30, 15),                  -- 현재가
    unrealized_pnl DECIMAL(30, 15) DEFAULT 0,       -- 미실현 손익
    realized_pnl DECIMAL(30, 15) DEFAULT 0,         -- 실현 손익
    strategy_id VARCHAR(100),                       -- 전략 ID
    opened_at TIMESTAMPTZ DEFAULT NOW(),            -- 포지션 오픈 시간
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    closed_at TIMESTAMPTZ,                          -- 포지션 종료 시간
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_positions_open ON positions(exchange, symbol_id) WHERE closed_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_positions_strategy ON positions(strategy_id) WHERE strategy_id IS NOT NULL;

COMMENT ON TABLE positions IS '보유 포지션 정보 (미청산 포지션 추적)';

CREATE TABLE IF NOT EXISTS signals (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    strategy_id VARCHAR(100) NOT NULL,
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    side order_side NOT NULL,                       -- 매수/매도
    signal_type signal_type NOT NULL,               -- 시그널 타입
    strength DECIMAL(5, 4) NOT NULL CHECK (strength >= 0 AND strength <= 1), -- 시그널 강도 (0~1)
    suggested_price DECIMAL(30, 15),                -- 제안 가격
    stop_loss DECIMAL(30, 15),                      -- 손절가
    take_profit DECIMAL(30, 15),                    -- 익절가
    created_at TIMESTAMPTZ DEFAULT NOW(),
    processed_at TIMESTAMPTZ,                       -- 처리 완료 시간
    order_id UUID REFERENCES orders(id),            -- 생성된 주문 ID
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_signals_unprocessed ON signals(created_at) WHERE processed_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_signals_strategy ON signals(strategy_id, created_at DESC);

COMMENT ON TABLE signals IS '전략에서 생성된 매매 시그널';

COMMENT ON COLUMN signals.strength IS '시그널 강도 (0.0 ~ 1.0)';

CREATE TABLE IF NOT EXISTS strategies (
    id VARCHAR(100) PRIMARY KEY,                    -- 전략 고유 ID
    name VARCHAR(200) NOT NULL,                     -- 전략 이름
    description TEXT,                               -- 전략 설명
    version VARCHAR(20),                            -- 전략 버전
    is_active BOOLEAN DEFAULT false,                -- 활성화 여부
    config JSONB DEFAULT '{}',                      -- 전략 설정
    risk_limits JSONB DEFAULT '{}',                 -- 리스크 제한 설정
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    last_started_at TIMESTAMPTZ,                    -- 마지막 시작 시간
    last_stopped_at TIMESTAMPTZ                     -- 마지막 중지 시간
);

COMMENT ON TABLE strategies IS '전략 메타데이터 및 실행 상태';

CREATE TABLE IF NOT EXISTS performance_snapshots (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    strategy_id VARCHAR(100) REFERENCES strategies(id),
    snapshot_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    total_trades INT DEFAULT 0,                     -- 총 거래 수
    winning_trades INT DEFAULT 0,                   -- 승리 거래 수
    losing_trades INT DEFAULT 0,                    -- 패배 거래 수
    total_pnl DECIMAL(30, 15) DEFAULT 0,            -- 총 손익
    total_fees DECIMAL(30, 15) DEFAULT 0,           -- 총 수수료
    max_drawdown DECIMAL(10, 4),                    -- 최대 낙폭 (%)
    sharpe_ratio DECIMAL(10, 4),                    -- 샤프 비율
    win_rate DECIMAL(5, 4),                         -- 승률 (0~1)
    profit_factor DECIMAL(10, 4),                   -- 수익 팩터
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_performance_strategy ON performance_snapshots(strategy_id, snapshot_time DESC);

COMMENT ON TABLE performance_snapshots IS '전략별 성과 스냅샷 (시계열 추적)';

CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type VARCHAR(50) NOT NULL,                -- 이벤트 타입 (order_created 등)
    entity_type VARCHAR(50),                        -- 엔티티 타입 (order, strategy 등)
    entity_id UUID,                                 -- 엔티티 ID
    user_id VARCHAR(100),                           -- 사용자 ID
    details JSONB DEFAULT '{}',                     -- 상세 정보
    ip_address INET,                                -- IP 주소
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_time ON audit_logs(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_entity ON audit_logs(entity_type, entity_id);

COMMENT ON TABLE audit_logs IS '시스템 감사 로그 (모든 주요 이벤트 추적)';

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,            -- bcrypt 해시
    role VARCHAR(20) DEFAULT 'trader',              -- 역할 (trader, admin 등)
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    last_login_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);

CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

COMMENT ON TABLE users IS 'API 인증 사용자 정보';

CREATE TABLE IF NOT EXISTS api_keys (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID REFERENCES users(id),
    exchange VARCHAR(50) NOT NULL,                  -- 거래소 (binance, kis 등)
    name VARCHAR(100) NOT NULL,                     -- API 키 별칭
    api_key_encrypted BYTEA NOT NULL,               -- 암호화된 API Key
    api_secret_encrypted BYTEA NOT NULL,            -- 암호화된 API Secret
    passphrase_encrypted BYTEA,                     -- 암호화된 Passphrase (일부 거래소)
    permissions JSONB DEFAULT '["read"]',           -- 권한 (read, trade, withdraw)
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    last_used_at TIMESTAMPTZ                        -- 마지막 사용 시간
);

CREATE INDEX IF NOT EXISTS idx_api_keys_user ON api_keys(user_id, is_active);

COMMENT ON TABLE api_keys IS '거래소 API 키 (AES-256-GCM 암호화 저장)';

CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_symbols_updated_at BEFORE UPDATE ON symbols
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_orders_updated_at BEFORE UPDATE ON orders
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_positions_updated_at BEFORE UPDATE ON positions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_strategies_updated_at BEFORE UPDATE ON strategies
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_api_keys_updated_at BEFORE UPDATE ON api_keys
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

SELECT add_retention_policy('klines', INTERVAL '2 years');

SELECT add_retention_policy('trade_ticks', INTERVAL '6 months');

CREATE TABLE IF NOT EXISTS exchange_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    exchange_id VARCHAR(50) NOT NULL,               -- 'binance', 'kis', 'coinbase' 등
    exchange_name VARCHAR(100) NOT NULL,            -- 표시용 이름 (예: "KIS 일반계좌", "KIS ISA")
    market_type VARCHAR(20) NOT NULL,               -- 'crypto', 'stock_kr', 'stock_us', 'forex'

    encrypted_credentials BYTEA NOT NULL,

    encryption_version INT NOT NULL DEFAULT 1,      -- 암호화 버전 (키 로테이션용)
    encryption_nonce BYTEA NOT NULL,                -- AES-GCM nonce (12바이트, 각 암호화마다 고유)

    is_active BOOLEAN NOT NULL DEFAULT true,
    is_testnet BOOLEAN NOT NULL DEFAULT false,      -- 테스트넷 여부
    permissions JSONB,                              -- 권한 정보: ["read", "trade", "withdraw"]

    settings JSONB DEFAULT '{}',                    -- 거래소별 추가 설정

    last_used_at TIMESTAMPTZ,                       -- 마지막 사용 시간
    last_verified_at TIMESTAMPTZ,                   -- 마지막 연결 테스트 시간
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT unique_exchange_account UNIQUE (exchange_id, market_type, is_testnet, exchange_name)
);

CREATE INDEX IF NOT EXISTS idx_exchange_credentials_active ON exchange_credentials(is_active) WHERE is_active = true;

CREATE INDEX IF NOT EXISTS idx_exchange_credentials_exchange ON exchange_credentials(exchange_id);

COMMENT ON TABLE exchange_credentials IS '거래소 API 자격증명 (AES-256-GCM 암호화)';

COMMENT ON COLUMN exchange_credentials.encrypted_credentials IS 'AES-256-GCM으로 암호화된 JSON (api_key, api_secret 등)';

COMMENT ON COLUMN exchange_credentials.encryption_nonce IS 'AES-GCM nonce (12바이트, 각 암호화마다 고유)';

COMMENT ON COLUMN exchange_credentials.exchange_name IS '계좌 구분용 이름 (동일 거래소에서 일반/ISA 등 여러 계좌 허용)';

COMMENT ON CONSTRAINT unique_exchange_account ON exchange_credentials IS '거래소별 계좌 구분 (동일 거래소에서 일반/ISA 등 여러 계좌 허용)';

CREATE OR REPLACE FUNCTION update_exchange_credentials_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_exchange_credentials_updated_at
    BEFORE UPDATE ON exchange_credentials
    FOR EACH ROW
    EXECUTE FUNCTION update_exchange_credentials_updated_at();

CREATE TABLE IF NOT EXISTS telegram_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    encrypted_bot_token BYTEA NOT NULL,
    encryption_nonce_token BYTEA NOT NULL,          -- Bot Token용 nonce

    encrypted_chat_id BYTEA NOT NULL,
    encryption_nonce_chat BYTEA NOT NULL,           -- Chat ID용 nonce

    encryption_version INT NOT NULL DEFAULT 1,

    is_enabled BOOLEAN NOT NULL DEFAULT true,
    notification_settings JSONB DEFAULT '{
        "trade_executed": true,
        "order_filled": true,
        "position_opened": true,
        "position_closed": true,
        "stop_loss_triggered": true,
        "take_profit_triggered": true,
        "daily_summary": true,
        "error_alerts": true,
        "risk_warnings": true
    }',

    bot_username VARCHAR(100),                      -- @username (연결 테스트 후 저장)
    chat_type VARCHAR(20),                          -- 'private', 'group', 'supergroup'

    last_message_at TIMESTAMPTZ,                    -- 마지막 메시지 전송 시간
    last_verified_at TIMESTAMPTZ,                   -- 마지막 연결 테스트 시간
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_telegram_single_setting ON telegram_settings((1));

COMMENT ON TABLE telegram_settings IS '텔레그램 봇 설정 (AES-256-GCM 암호화)';

COMMENT ON COLUMN telegram_settings.notification_settings IS '알림 유형별 활성화 설정 (JSONB)';

CREATE TRIGGER trigger_telegram_settings_updated_at
    BEFORE UPDATE ON telegram_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_exchange_credentials_updated_at();

CREATE TABLE IF NOT EXISTS strategy_presets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    strategy_type VARCHAR(100) NOT NULL,            -- 'grid_trading', 'ma_crossover', 'rsi_mean_reversion' 등
    preset_name VARCHAR(200) NOT NULL,              -- 사용자 정의 이름
    description TEXT,

    parameters JSONB NOT NULL,

    schema_version INT NOT NULL DEFAULT 1,

    is_default BOOLEAN NOT NULL DEFAULT false,      -- 기본 프리셋 여부
    is_public BOOLEAN NOT NULL DEFAULT false,       -- 공유 프리셋 여부

    tags VARCHAR(50)[] DEFAULT '{}',                -- 태그: ["btc", "aggressive", "tested"]
    performance_metrics JSONB,                      -- 백테스트 결과 저장

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_strategy_presets_type ON strategy_presets(strategy_type);

CREATE INDEX IF NOT EXISTS idx_strategy_presets_default ON strategy_presets(is_default) WHERE is_default = true;

CREATE INDEX IF NOT EXISTS idx_strategy_presets_tags ON strategy_presets USING GIN(tags);

COMMENT ON TABLE strategy_presets IS '전략 파라미터 프리셋 (백테스트 결과 저장 가능)';

COMMENT ON COLUMN strategy_presets.parameters IS '전략별 파라미터 (JSONB, 구조는 strategy_type에 따라 상이)';

COMMENT ON COLUMN strategy_presets.performance_metrics IS '백테스트 결과 (sharpe_ratio, max_drawdown 등)';

CREATE TRIGGER trigger_strategy_presets_updated_at
    BEFORE UPDATE ON strategy_presets
    FOR EACH ROW
    EXECUTE FUNCTION update_exchange_credentials_updated_at();

CREATE TABLE IF NOT EXISTS credential_access_logs (
    id BIGSERIAL PRIMARY KEY,

    credential_type VARCHAR(50) NOT NULL,           -- 'exchange', 'telegram'
    credential_id UUID NOT NULL,

    action VARCHAR(50) NOT NULL,                    -- 'create', 'read', 'update', 'delete', 'verify', 'use'
    accessor_ip VARCHAR(45),                        -- IPv4/IPv6
    user_agent TEXT,

    success BOOLEAN NOT NULL DEFAULT true,
    error_message TEXT,

    accessed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

SELECT create_hypertable('credential_access_logs', 'accessed_at', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_credential_access_logs_type_id ON credential_access_logs(credential_type, credential_id);

CREATE INDEX IF NOT EXISTS idx_credential_access_logs_action ON credential_access_logs(action);

SELECT add_retention_policy('credential_access_logs', INTERVAL '90 days', if_not_exists => TRUE);

COMMENT ON TABLE credential_access_logs IS '자격증명 접근 감사 로그 (TimescaleDB Hypertable, 90일 보존)';

COMMENT ON COLUMN credential_access_logs.action IS '접근 유형: create, read, update, delete, verify, use';

CREATE TABLE IF NOT EXISTS watchlist (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    symbol VARCHAR(50) NOT NULL,                    -- 종목 코드
    market VARCHAR(10) NOT NULL,                    -- 시장: 'KR', 'US', 'crypto'
    display_name VARCHAR(100),                      -- 표시 이름
    sort_order INT DEFAULT 0,                       -- 정렬 순서
    is_active BOOLEAN DEFAULT true,                 -- 활성화 여부
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(symbol, market)                          -- 동일 시장에서 중복 방지
);

CREATE INDEX IF NOT EXISTS idx_watchlist_active ON watchlist(is_active) WHERE is_active = true;

CREATE INDEX IF NOT EXISTS idx_watchlist_sort ON watchlist(sort_order);

COMMENT ON TABLE watchlist IS '사용자 관심 종목 관리';

COMMENT ON COLUMN watchlist.market IS '시장 구분: KR(한국), US(미국), crypto(암호화폐)';

COMMENT ON COLUMN watchlist.display_name IS 'WebSocket 표시용 이름 (하이픈 사용)';

INSERT INTO watchlist (symbol, market, display_name, sort_order) VALUES
    ('069500', 'KR', 'KODEX-200', 1),
    ('122630', 'KR', 'KODEX-레버리지', 2),
    ('SPY', 'US', 'SPY', 3),
    ('QQQ', 'US', 'QQQ', 4),
    ('TQQQ', 'US', 'TQQQ', 5)
ON CONFLICT (symbol, market) DO NOTHING;

CREATE TABLE IF NOT EXISTS app_settings (
    setting_key VARCHAR(100) PRIMARY KEY,           -- 설정 키
    setting_value TEXT NOT NULL DEFAULT '',         -- 설정 값
    description TEXT,                               -- 설정 설명
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_app_settings_key ON app_settings(setting_key);

COMMENT ON TABLE app_settings IS '애플리케이션 전역 설정 (key-value 저장소)';

COMMENT ON COLUMN app_settings.setting_key IS '설정 키 (예: active_credential_id, default_currency)';

COMMENT ON COLUMN app_settings.setting_value IS '설정 값 (문자열, JSONB로 저장 가능)';

INSERT INTO app_settings (setting_key, setting_value, description)
VALUES
    ('active_credential_id', '', '대시보드에 표시할 활성 거래소 계정 ID (UUID)'),
    ('default_currency', 'KRW', '기본 통화 (KRW, USD 등)'),
    ('theme', 'dark', 'UI 테마 (dark, light)')
ON CONFLICT (setting_key) DO NOTHING;

