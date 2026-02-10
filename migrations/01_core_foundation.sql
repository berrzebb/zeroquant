-- =====================================================
-- 01_core_foundation.sql (통합)
-- ZeroQuant 핵심 기초 스키마
-- =====================================================
-- 원본: 01_foundation.sql, 02_credentials_system.sql, 03_application_config.sql
-- =====================================================

-- =====================================================
-- 01_foundation.sql
-- ZeroQuant 트레이딩 시스템 기본 스키마
-- =====================================================
--
-- 원본 마이그레이션: 001_initial_schema.sql
--
-- 포함 내용:
-- - PostgreSQL Extensions (TimescaleDB, UUID)
-- - 기본 ENUM 타입 정의
-- - 핵심 테이블: symbols, users, api_keys, strategies
-- - 시계열 데이터: klines, trade_ticks (Hypertables)
-- - 거래 추적: orders, trades, positions, signals
-- - 성과 분석: performance_snapshots
-- - 감사 로그: audit_logs
-- - 자동 타임스탬프 업데이트 함수 및 트리거
-- - 데이터 보존 정책
--
-- =====================================================

-- =====================================================
-- EXTENSIONS
-- =====================================================

-- TimescaleDB: 시계열 데이터 최적화
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- UUID 생성 함수
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- =====================================================
-- ENUM TYPES
-- =====================================================

-- 시장 유형
CREATE TYPE market_type AS ENUM ('crypto', 'stock', 'forex', 'futures');

-- 주문 방향
CREATE TYPE order_side AS ENUM ('buy', 'sell');

-- 주문 타입
CREATE TYPE order_type AS ENUM ('market', 'limit', 'stop', 'stop_limit', 'trailing_stop');

-- 주문 상태
CREATE TYPE order_status AS ENUM ('pending', 'open', 'partially_filled', 'filled', 'cancelled', 'rejected', 'expired');

-- 주문 유효 기간
CREATE TYPE order_time_in_force AS ENUM ('gtc', 'ioc', 'fok', 'day');

-- 시그널 타입
CREATE TYPE signal_type AS ENUM ('entry', 'exit', 'add_to_position', 'reduce_position', 'scale');

-- =====================================================
-- SYMBOLS TABLE
-- 거래 가능한 심볼 정보 (종목, 통화쌍 등)
-- =====================================================

CREATE TABLE symbols (
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

CREATE INDEX idx_symbols_active ON symbols(is_active) WHERE is_active = true;
CREATE INDEX idx_symbols_exchange ON symbols(exchange);

COMMENT ON TABLE symbols IS '거래 가능한 심볼(종목) 메타데이터';
COMMENT ON COLUMN symbols.min_notional IS '최소 주문 금액 (quantity * price)';

-- =====================================================
-- KLINES (OHLCV) TABLE
-- 시계열 캔들 데이터 - TimescaleDB Hypertable
-- =====================================================

CREATE TABLE klines (
    time TIMESTAMPTZ NOT NULL,                      -- 캔들 시작 시간
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    timeframe VARCHAR(10) NOT NULL,                 -- 타임프레임 (1m, 5m, 1h, 1d 등)
    open DECIMAL(30, 15) NOT NULL,
    high DECIMAL(30, 15) NOT NULL,
    low DECIMAL(30, 15) NOT NULL,
    close DECIMAL(30, 15) NOT NULL,
    volume DECIMAL(30, 15) NOT NULL,
    quote_volume DECIMAL(30, 15),                   -- 거래대금 (volume * price)
    num_trades INT,                                 -- 거래 건수
    PRIMARY KEY (symbol_id, timeframe, time)
);

-- TimescaleDB Hypertable 변환 (1주 단위 청크)
SELECT create_hypertable('klines', 'time', chunk_time_interval => INTERVAL '1 week');

CREATE INDEX idx_klines_symbol_time ON klines(symbol_id, time DESC);

-- 압축 정책: 30일 이상 데이터 압축
ALTER TABLE klines SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol_id, timeframe'
);
SELECT add_compression_policy('klines', INTERVAL '30 days');

COMMENT ON TABLE klines IS 'OHLCV 캔들 데이터 (TimescaleDB Hypertable)';

-- =====================================================
-- TRADE_TICKS TABLE
-- 거래소 실시간 체결 데이터 - TimescaleDB Hypertable
-- =====================================================

CREATE TABLE trade_ticks (
    time TIMESTAMPTZ NOT NULL,                      -- 체결 시간
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    exchange_trade_id VARCHAR(100) NOT NULL,        -- 거래소 거래 ID
    price DECIMAL(30, 15) NOT NULL,
    quantity DECIMAL(30, 15) NOT NULL,
    is_buyer_maker BOOLEAN,                         -- 매수자가 메이커인지 여부
    PRIMARY KEY (symbol_id, time, exchange_trade_id)
);

-- TimescaleDB Hypertable 변환 (1일 단위 청크)
SELECT create_hypertable('trade_ticks', 'time', chunk_time_interval => INTERVAL '1 day');

CREATE INDEX idx_trade_ticks_symbol ON trade_ticks(symbol_id, time DESC);

-- 압축 정책: 7일 이상 데이터 압축
ALTER TABLE trade_ticks SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol_id'
);
SELECT add_compression_policy('trade_ticks', INTERVAL '7 days');

COMMENT ON TABLE trade_ticks IS '거래소 실시간 체결 데이터 (TimescaleDB Hypertable)';

-- =====================================================
-- ORDERS TABLE
-- 주문 정보
-- =====================================================

CREATE TABLE orders (
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

CREATE INDEX idx_orders_status ON orders(status) WHERE status IN ('pending', 'open', 'partially_filled');
CREATE INDEX idx_orders_strategy ON orders(strategy_id) WHERE strategy_id IS NOT NULL;
CREATE INDEX idx_orders_symbol ON orders(symbol_id, created_at DESC);
CREATE INDEX idx_orders_exchange ON orders(exchange, exchange_order_id);

COMMENT ON TABLE orders IS '주문 정보 (실시간 주문 추적)';

-- =====================================================
-- TRADES TABLE
-- 실제 체결된 거래 내역
-- =====================================================

CREATE TABLE trades (
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

CREATE INDEX idx_trades_order ON trades(order_id);
CREATE INDEX idx_trades_symbol ON trades(symbol_id, executed_at DESC);
CREATE INDEX idx_trades_executed ON trades(executed_at DESC);

COMMENT ON TABLE trades IS '실제 체결된 거래 내역';

-- =====================================================
-- POSITIONS TABLE
-- 보유 포지션 정보
-- =====================================================

CREATE TABLE positions (
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

CREATE INDEX idx_positions_open ON positions(exchange, symbol_id) WHERE closed_at IS NULL;
CREATE INDEX idx_positions_strategy ON positions(strategy_id) WHERE strategy_id IS NOT NULL;

COMMENT ON TABLE positions IS '보유 포지션 정보 (미청산 포지션 추적)';

-- =====================================================
-- SIGNALS TABLE
-- 전략에서 생성된 매매 시그널
-- =====================================================

CREATE TABLE signals (
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

CREATE INDEX idx_signals_unprocessed ON signals(created_at) WHERE processed_at IS NULL;
CREATE INDEX idx_signals_strategy ON signals(strategy_id, created_at DESC);

COMMENT ON TABLE signals IS '전략에서 생성된 매매 시그널';
COMMENT ON COLUMN signals.strength IS '시그널 강도 (0.0 ~ 1.0)';

-- =====================================================
-- STRATEGIES TABLE
-- 전략 메타데이터 및 상태
-- =====================================================

CREATE TABLE strategies (
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

-- =====================================================
-- PERFORMANCE_SNAPSHOTS TABLE
-- 전략별 성과 스냅샷
-- =====================================================

CREATE TABLE performance_snapshots (
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

CREATE INDEX idx_performance_strategy ON performance_snapshots(strategy_id, snapshot_time DESC);

COMMENT ON TABLE performance_snapshots IS '전략별 성과 스냅샷 (시계열 추적)';

-- =====================================================
-- AUDIT_LOGS TABLE
-- 시스템 감사 로그
-- =====================================================

CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type VARCHAR(50) NOT NULL,                -- 이벤트 타입 (order_created 등)
    entity_type VARCHAR(50),                        -- 엔티티 타입 (order, strategy 등)
    entity_id UUID,                                 -- 엔티티 ID
    user_id VARCHAR(100),                           -- 사용자 ID
    details JSONB DEFAULT '{}',                     -- 상세 정보
    ip_address INET,                                -- IP 주소
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_audit_logs_time ON audit_logs(created_at DESC);
CREATE INDEX idx_audit_logs_entity ON audit_logs(entity_type, entity_id);

COMMENT ON TABLE audit_logs IS '시스템 감사 로그 (모든 주요 이벤트 추적)';

-- =====================================================
-- USERS TABLE
-- API 인증 사용자
-- =====================================================

CREATE TABLE users (
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

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email);

COMMENT ON TABLE users IS 'API 인증 사용자 정보';

-- =====================================================
-- API_KEYS TABLE
-- 거래소 API 키 (암호화 저장)
-- =====================================================

CREATE TABLE api_keys (
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

CREATE INDEX idx_api_keys_user ON api_keys(user_id, is_active);

COMMENT ON TABLE api_keys IS '거래소 API 키 (AES-256-GCM 암호화 저장)';

-- =====================================================
-- FUNCTIONS AND TRIGGERS
-- =====================================================

-- 자동 updated_at 업데이트 함수
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Triggers: 자동 타임스탬프 업데이트
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

-- =====================================================
-- DATA RETENTION POLICIES
-- =====================================================

-- klines: 2년 보존
SELECT add_retention_policy('klines', INTERVAL '2 years');

-- trade_ticks: 6개월 보존
SELECT add_retention_policy('trade_ticks', INTERVAL '6 months');

-- ============ 02_credentials_system.sql ============

-- =====================================================
-- 02_credentials_system.sql
-- 암호화된 자격증명 및 전략 프리셋 관리
-- =====================================================
--
-- 원본 마이그레이션: 002_encrypted_credentials.sql, 003_fix_credentials_unique_constraint.sql
--
-- 포함 내용:
-- - exchange_credentials: 거래소 API 자격증명 (AES-256-GCM 암호화)
-- - telegram_settings: 텔레그램 봇 설정 (암호화)
-- - strategy_presets: 전략 파라미터 프리셋 (031과 통합)
-- - credential_access_logs: 자격증명 접근 감사 로그 (Hypertable)
-- - 여러 거래소 계좌 지원 (예: KIS 일반계좌, ISA계좌)
--
-- 보안 기능:
-- - AES-256-GCM 암호화 (nonce 12바이트)
-- - 암호화 버전 관리 (키 로테이션 지원)
-- - 접근 감사 로그 (90일 보존)
--
-- =====================================================

-- =====================================================
-- EXCHANGE_CREDENTIALS TABLE
-- 거래소 API 자격증명 (암호화 저장)
-- =====================================================

CREATE TABLE IF NOT EXISTS exchange_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- 거래소 정보
    exchange_id VARCHAR(50) NOT NULL,               -- 'binance', 'kis', 'coinbase' 등
    exchange_name VARCHAR(100) NOT NULL,            -- 표시용 이름 (예: "KIS 일반계좌", "KIS ISA")
    market_type VARCHAR(20) NOT NULL,               -- 'crypto', 'stock_kr', 'stock_us', 'forex'

    -- 암호화된 자격증명 (JSON 형태로 암호화)
    -- 예: {"api_key": "xxx", "api_secret": "yyy", "passphrase": "zzz"}
    encrypted_credentials BYTEA NOT NULL,

    -- 암호화 메타데이터
    encryption_version INT NOT NULL DEFAULT 1,      -- 암호화 버전 (키 로테이션용)
    encryption_nonce BYTEA NOT NULL,                -- AES-GCM nonce (12바이트, 각 암호화마다 고유)

    -- 상태 및 메타데이터
    is_active BOOLEAN NOT NULL DEFAULT true,
    is_testnet BOOLEAN NOT NULL DEFAULT false,      -- 테스트넷 여부
    permissions JSONB,                              -- 권한 정보: ["read", "trade", "withdraw"]

    -- 추가 설정
    settings JSONB DEFAULT '{}',                    -- 거래소별 추가 설정

    -- 감사 정보
    last_used_at TIMESTAMPTZ,                       -- 마지막 사용 시간
    last_verified_at TIMESTAMPTZ,                   -- 마지막 연결 테스트 시간
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 여러 계좌 지원: exchange_name 포함하여 구분
    -- (예: KIS 일반계좌, KIS ISA계좌)
    CONSTRAINT unique_exchange_account UNIQUE (exchange_id, market_type, is_testnet, exchange_name)
);

-- 인덱스
CREATE INDEX idx_exchange_credentials_active ON exchange_credentials(is_active) WHERE is_active = true;
CREATE INDEX idx_exchange_credentials_exchange ON exchange_credentials(exchange_id);

COMMENT ON TABLE exchange_credentials IS '거래소 API 자격증명 (AES-256-GCM 암호화)';
COMMENT ON COLUMN exchange_credentials.encrypted_credentials IS 'AES-256-GCM으로 암호화된 JSON (api_key, api_secret 등)';
COMMENT ON COLUMN exchange_credentials.encryption_nonce IS 'AES-GCM nonce (12바이트, 각 암호화마다 고유)';
COMMENT ON COLUMN exchange_credentials.exchange_name IS '계좌 구분용 이름 (동일 거래소에서 일반/ISA 등 여러 계좌 허용)';
COMMENT ON CONSTRAINT unique_exchange_account ON exchange_credentials IS '거래소별 계좌 구분 (동일 거래소에서 일반/ISA 등 여러 계좌 허용)';

-- 자동 updated_at 업데이트 함수
CREATE OR REPLACE FUNCTION update_exchange_credentials_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 업데이트 트리거
CREATE TRIGGER trigger_exchange_credentials_updated_at
    BEFORE UPDATE ON exchange_credentials
    FOR EACH ROW
    EXECUTE FUNCTION update_exchange_credentials_updated_at();

-- =====================================================
-- TELEGRAM_SETTINGS TABLE
-- 텔레그램 봇 설정 (암호화 저장)
-- =====================================================

CREATE TABLE IF NOT EXISTS telegram_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- 암호화된 Bot Token
    encrypted_bot_token BYTEA NOT NULL,
    encryption_nonce_token BYTEA NOT NULL,          -- Bot Token용 nonce

    -- 암호화된 Chat ID (개인 또는 그룹)
    encrypted_chat_id BYTEA NOT NULL,
    encryption_nonce_chat BYTEA NOT NULL,           -- Chat ID용 nonce

    -- 암호화 버전
    encryption_version INT NOT NULL DEFAULT 1,

    -- 알림 설정
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

    -- 메타데이터
    bot_username VARCHAR(100),                      -- @username (연결 테스트 후 저장)
    chat_type VARCHAR(20),                          -- 'private', 'group', 'supergroup'

    -- 감사 정보
    last_message_at TIMESTAMPTZ,                    -- 마지막 메시지 전송 시간
    last_verified_at TIMESTAMPTZ,                   -- 마지막 연결 테스트 시간
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 단일 텔레그램 설정만 허용 (필요시 여러 개 허용 가능하도록 변경)
CREATE UNIQUE INDEX idx_telegram_single_setting ON telegram_settings((1));

COMMENT ON TABLE telegram_settings IS '텔레그램 봇 설정 (AES-256-GCM 암호화)';
COMMENT ON COLUMN telegram_settings.notification_settings IS '알림 유형별 활성화 설정 (JSONB)';

-- 업데이트 트리거
CREATE TRIGGER trigger_telegram_settings_updated_at
    BEFORE UPDATE ON telegram_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_exchange_credentials_updated_at();

-- =====================================================
-- STRATEGY_PRESETS TABLE
-- 전략 파라미터 프리셋
-- =====================================================

CREATE TABLE IF NOT EXISTS strategy_presets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- 전략 정보
    strategy_type VARCHAR(100) NOT NULL,            -- 'grid_trading', 'ma_crossover', 'rsi_mean_reversion' 등
    preset_name VARCHAR(200) NOT NULL,              -- 사용자 정의 이름
    description TEXT,

    -- 파라미터 (JSON)
    -- 예: {"symbol": "BTC/USDT", "grid_levels": 10, "grid_spacing_pct": 1.0, ...}
    parameters JSONB NOT NULL,

    -- 파라미터 스키마 버전 (호환성 관리)
    schema_version INT NOT NULL DEFAULT 1,

    -- 상태
    is_default BOOLEAN NOT NULL DEFAULT false,      -- 기본 프리셋 여부
    is_public BOOLEAN NOT NULL DEFAULT false,       -- 공유 프리셋 여부

    -- 메타데이터
    tags VARCHAR(50)[] DEFAULT '{}',                -- 태그: ["btc", "aggressive", "tested"]
    performance_metrics JSONB,                      -- 백테스트 결과 저장

    -- 감사 정보
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 인덱스
CREATE INDEX idx_strategy_presets_type ON strategy_presets(strategy_type);
CREATE INDEX idx_strategy_presets_default ON strategy_presets(is_default) WHERE is_default = true;
CREATE INDEX idx_strategy_presets_tags ON strategy_presets USING GIN(tags);

COMMENT ON TABLE strategy_presets IS '전략 파라미터 프리셋 (백테스트 결과 저장 가능)';
COMMENT ON COLUMN strategy_presets.parameters IS '전략별 파라미터 (JSONB, 구조는 strategy_type에 따라 상이)';
COMMENT ON COLUMN strategy_presets.performance_metrics IS '백테스트 결과 (sharpe_ratio, max_drawdown 등)';

-- 업데이트 트리거
CREATE TRIGGER trigger_strategy_presets_updated_at
    BEFORE UPDATE ON strategy_presets
    FOR EACH ROW
    EXECUTE FUNCTION update_exchange_credentials_updated_at();

-- =====================================================
-- CREDENTIAL_ACCESS_LOGS TABLE
-- 자격증명 접근 감사 로그 - TimescaleDB Hypertable
-- =====================================================

CREATE TABLE IF NOT EXISTS credential_access_logs (
    id BIGSERIAL PRIMARY KEY,

    -- 대상 정보
    credential_type VARCHAR(50) NOT NULL,           -- 'exchange', 'telegram'
    credential_id UUID NOT NULL,

    -- 접근 정보
    action VARCHAR(50) NOT NULL,                    -- 'create', 'read', 'update', 'delete', 'verify', 'use'
    accessor_ip VARCHAR(45),                        -- IPv4/IPv6
    user_agent TEXT,

    -- 결과
    success BOOLEAN NOT NULL DEFAULT true,
    error_message TEXT,

    -- 타임스탬프
    accessed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- TimescaleDB Hypertable 변환 (1주 단위 청크)
SELECT create_hypertable('credential_access_logs', 'accessed_at', if_not_exists => TRUE);

-- 인덱스
CREATE INDEX idx_credential_access_logs_type_id ON credential_access_logs(credential_type, credential_id);
CREATE INDEX idx_credential_access_logs_action ON credential_access_logs(action);

-- 90일 이후 데이터 자동 삭제 정책
SELECT add_retention_policy('credential_access_logs', INTERVAL '90 days', if_not_exists => TRUE);

COMMENT ON TABLE credential_access_logs IS '자격증명 접근 감사 로그 (TimescaleDB Hypertable, 90일 보존)';
COMMENT ON COLUMN credential_access_logs.action IS '접근 유형: create, read, update, delete, verify, use';

-- ============ 03_application_config.sql ============

-- =====================================================
-- 03_application_config.sql
-- 애플리케이션 설정 및 관심 종목 관리
-- =====================================================
--
-- 원본 마이그레이션: 004_watchlist.sql, 006_app_settings.sql
--
-- 포함 내용:
-- - watchlist: 사용자 관심 종목 관리
-- - app_settings: 애플리케이션 전역 설정 (key-value)
-- - 기본 데이터: 기본 관심 종목 5개, 기본 설정 3개
--
-- =====================================================

-- =====================================================
-- WATCHLIST TABLE
-- 사용자 관심 종목 관리
-- =====================================================

CREATE TABLE watchlist (
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

CREATE INDEX idx_watchlist_active ON watchlist(is_active) WHERE is_active = true;
CREATE INDEX idx_watchlist_sort ON watchlist(sort_order);

COMMENT ON TABLE watchlist IS '사용자 관심 종목 관리';
COMMENT ON COLUMN watchlist.market IS '시장 구분: KR(한국), US(미국), crypto(암호화폐)';
COMMENT ON COLUMN watchlist.display_name IS 'WebSocket 표시용 이름 (하이픈 사용)';

-- 기본 관심 종목 추가
-- 주의: display_name은 WebSocket 시뮬레이터 심볼과 일치해야 함 (하이픈 사용)
INSERT INTO watchlist (symbol, market, display_name, sort_order) VALUES
    ('069500', 'KR', 'KODEX-200', 1),
    ('122630', 'KR', 'KODEX-레버리지', 2),
    ('SPY', 'US', 'SPY', 3),
    ('QQQ', 'US', 'QQQ', 4),
    ('TQQQ', 'US', 'TQQQ', 5)
ON CONFLICT (symbol, market) DO NOTHING;

-- =====================================================
-- APP_SETTINGS TABLE
-- 애플리케이션 전역 설정 (key-value)
-- =====================================================

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

-- 기본 설정값 삽입
INSERT INTO app_settings (setting_key, setting_value, description)
VALUES
    ('active_credential_id', '', '대시보드에 표시할 활성 거래소 계정 ID (UUID)'),
    ('default_currency', 'KRW', '기본 통화 (KRW, USD 등)'),
    ('theme', 'dark', 'UI 테마 (dark, light)')
ON CONFLICT (setting_key) DO NOTHING;
