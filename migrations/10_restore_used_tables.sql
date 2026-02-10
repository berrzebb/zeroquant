-- 10_restore_used_tables.sql
-- 잘못 삭제된 테이블 복원
--
-- 09_remove_legacy_tables.sql에서 실수로 삭제된 테이블 복원:
-- 1. symbols - timescale.rs, positions.rs에서 사용
-- 2. app_settings - credentials.rs, active_account.rs에서 사용

-- ============================================================================
-- 1. symbols 테이블 복원
-- ============================================================================

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

-- ============================================================================
-- 2. app_settings 테이블 복원
-- ============================================================================

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
    ('theme', 'dark', 'UI 테마 (dark, light)'),
    ('krx_api_info', 'https://openapi.krx.co.kr', 'KRX Open API 정보. API 키는 Settings > Credentials에서 등록하세요.')
ON CONFLICT (setting_key) DO NOTHING;

-- ============================================================================
-- 3. 외래 키 복원 (symbols 테이블 참조)
-- ============================================================================

-- orders 테이블에 symbols 외래 키 복원 (존재하는 경우)
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'orders') THEN
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.table_constraints
            WHERE constraint_name = 'orders_symbol_id_fkey' AND table_name = 'orders'
        ) THEN
            ALTER TABLE orders ADD CONSTRAINT orders_symbol_id_fkey
            FOREIGN KEY (symbol_id) REFERENCES symbols(id);
        END IF;
    END IF;
END $$;

-- positions 테이블에 symbols 외래 키 복원 (존재하는 경우)
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'positions') THEN
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.table_constraints
            WHERE constraint_name = 'positions_symbol_id_fkey' AND table_name = 'positions'
        ) THEN
            ALTER TABLE positions ADD CONSTRAINT positions_symbol_id_fkey
            FOREIGN KEY (symbol_id) REFERENCES symbols(id);
        END IF;
    END IF;
END $$;

-- trades 테이블에 symbols 외래 키 복원 (존재하는 경우)
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'trades') THEN
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.table_constraints
            WHERE constraint_name = 'trades_symbol_id_fkey' AND table_name = 'trades'
        ) THEN
            ALTER TABLE trades ADD CONSTRAINT trades_symbol_id_fkey
            FOREIGN KEY (symbol_id) REFERENCES symbols(id);
        END IF;
    END IF;
END $$;
