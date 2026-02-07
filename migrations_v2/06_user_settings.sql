-- =============================================================================
-- 06_user_settings
-- watchlist, preset, notification, checkpoint
-- =============================================================================
-- 통합 마이그레이션 파일 (자동 생성)
-- 원본 파일: ["06_", "11_", "12_", "17_"]
-- =============================================================================

-- ---------------------------------------------------------------------------
-- Source: 06_user_settings
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,                    -- 마이그레이션 버전 번호
    filename VARCHAR(255) NOT NULL,                 -- 마이그레이션 파일명
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),  -- 적용 시간
    checksum VARCHAR(64),                           -- SHA-256 해시 (향후 구현)
    execution_time_ms INTEGER,                      -- 실행 시간 (ms)
    success BOOLEAN NOT NULL DEFAULT true,          -- 적용 성공 여부
    error_message TEXT                              -- 실패 시 에러 메시지
);

CREATE INDEX IF NOT EXISTS idx_schema_migrations_applied ON schema_migrations(applied_at DESC);

CREATE INDEX IF NOT EXISTS idx_schema_migrations_failed ON schema_migrations(success) WHERE success = false;

COMMENT ON TABLE schema_migrations IS '마이그레이션 적용 이력 추적';

COMMENT ON COLUMN schema_migrations.version IS '마이그레이션 버전 번호 (파일명의 숫자)';

COMMENT ON COLUMN schema_migrations.filename IS '마이그레이션 파일명 (예: 001_initial_schema.sql)';

COMMENT ON COLUMN schema_migrations.checksum IS 'SHA-256 체크섬 (무결성 검증용, 향후 구현)';

COMMENT ON COLUMN schema_migrations.success IS '적용 성공 여부 (실패 시 false)';

INSERT INTO schema_migrations (version, filename, success, applied_at) VALUES
(1, '001_initial_schema.sql', true, '2025-01-27 00:00:00'),
(2, '002_encrypted_credentials.sql', true, '2025-01-29 00:00:00'),
(3, '003_fix_credentials_unique_constraint.sql', true, '2025-01-29 00:00:00'),
(4, '004_watchlist.sql', true, '2025-01-29 00:00:00'),
(5, '005_yahoo_candle_cache.sql', true, '2025-01-30 00:00:00'),
(6, '006_app_settings.sql', true, '2025-01-29 00:00:00'),
(7, '007_portfolio_equity_history.sql', true, '2025-01-30 00:00:00'),
(8, '008_strategies_type_and_symbols.sql', true, '2025-01-30 00:00:00'),
(9, '009_rename_candle_cache.sql', true, '2025-01-30 00:00:00'),
(10, '010_backtest_results.sql', true, '2025-01-30 00:00:00'),
(11, '011_execution_cache.sql', true, '2025-01-30 00:00:00'),
(12, '012_symbol_info.sql', true, '2025-01-30 00:00:00'),
(13, '013_strategy_timeframe.sql', true, '2025-01-30 00:00:00'),
(14, '014_strategy_risk_capital.sql', true, '2025-01-31 00:00:00'),
(15, '015_trading_journal.sql', true, '2025-01-31 00:00:00'),
(16, '016_positions_credential_id.sql', true, '2025-01-31 00:00:00'),
(17, '017_journal_views.sql', true, '2025-01-31 00:00:00'),
(18, '018_journal_period_views.sql', true, '2025-01-31 00:00:00'),
(19, '019_fix_cumulative_pnl_types.sql', true, '2025-01-31 00:00:00'),
(20, '020_symbol_fundamental.sql', true, '2025-01-31 00:00:00'),
(21, '021_fix_fundamental_decimal_precision.sql', true, '2025-02-01 00:00:00'),
(22, '022_latest_prices_materialized_view.sql', true, '2025-02-01 00:00:00'),
(23, '023_symbol_fetch_failure_tracking.sql', true, '2025-02-01 00:00:00'),
(24, '024_add_symbol_type.sql', true, '2025-02-03 00:00:00'),
(25, '025_add_route_state.sql', true, '2025-02-03 00:00:00'),
(26, '026_add_ttm_squeeze.sql', true, '2025-02-03 00:00:00'),
(27, '027_add_market_regime.sql', true, '2025-02-03 00:00:00'),
(28, '028_reality_check_system.sql', true, '2025-02-03 00:00:00'),
(29, '029_signal_marker.sql', true, '2025-02-03 00:00:00'),
(30, '030_add_missing_views.sql', true, NOW()),
(31, '031_add_strategy_presets.sql', true, NOW()),
(32, '032_fix_hypertable_declarations.sql', true, NOW()),
(33, '033_migration_tracking.sql', true, NOW()),
(34, '034_signal_alert_rules.sql', true, NOW())
ON CONFLICT (version) DO NOTHING;

CREATE TABLE IF NOT EXISTS watchlist (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    name VARCHAR(100) NOT NULL,                      -- 그룹 이름 (예: "모멘텀 종목", "저평가 주")
    description TEXT,                                -- 설명

    sort_order INTEGER NOT NULL DEFAULT 0,           -- 표시 순서

    color VARCHAR(20),                               -- 색상 코드 (#FF5733)
    icon VARCHAR(50),                                -- 아이콘 이름 (star, chart, etc)

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT unique_watchlist_name UNIQUE (name)
);

CREATE INDEX IF NOT EXISTS idx_watchlist_sort ON watchlist(sort_order);

COMMENT ON TABLE watchlist IS '관심종목 그룹 (Phase 3.1)';

COMMENT ON COLUMN watchlist.name IS '그룹 이름';

COMMENT ON COLUMN watchlist.sort_order IS '표시 순서 (낮을수록 먼저)';

CREATE TABLE IF NOT EXISTS watchlist_item (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    watchlist_id UUID NOT NULL REFERENCES watchlist(id) ON DELETE CASCADE,

    symbol VARCHAR(20) NOT NULL,                     -- 종목 코드 (005930, AAPL)
    market VARCHAR(20) NOT NULL DEFAULT 'KR',        -- 시장 (KR, US)

    memo TEXT,                                       -- 사용자 메모

    target_price NUMERIC(20, 4),                     -- 목표가
    stop_price NUMERIC(20, 4),                       -- 손절가
    alert_enabled BOOLEAN DEFAULT false,             -- 알림 활성화

    sort_order INTEGER NOT NULL DEFAULT 0,

    added_price NUMERIC(20, 4),                      -- 추가 시점 가격

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT unique_watchlist_symbol UNIQUE (watchlist_id, symbol, market)
);

CREATE INDEX IF NOT EXISTS idx_watchlist_item_watchlist ON watchlist_item(watchlist_id);

CREATE INDEX IF NOT EXISTS idx_watchlist_item_symbol ON watchlist_item(symbol, market);

CREATE INDEX IF NOT EXISTS idx_watchlist_item_sort ON watchlist_item(watchlist_id, sort_order);

COMMENT ON TABLE watchlist_item IS '관심종목 아이템 (Phase 3.1)';

COMMENT ON COLUMN watchlist_item.symbol IS '종목 코드';

COMMENT ON COLUMN watchlist_item.market IS '시장 (KR/US)';

COMMENT ON COLUMN watchlist_item.target_price IS '목표가';

COMMENT ON COLUMN watchlist_item.stop_price IS '손절가';

COMMENT ON COLUMN watchlist_item.added_price IS '추가 시점 가격';

INSERT INTO watchlist (name, description, sort_order, icon, color)
VALUES
    ('기본', '기본 관심종목 목록', 0, 'star', '#FFD700'),
    ('모멘텀', '모멘텀 상위 종목', 1, 'trending-up', '#10B981'),
    ('가치주', '저평가 가치 종목', 2, 'search', '#3B82F6')
ON CONFLICT (name) DO NOTHING;

INSERT INTO schema_migrations (version, filename, success, applied_at)
VALUES (13, '13_watchlist.sql', true, NOW())
ON CONFLICT (version) DO NOTHING;

CREATE TABLE IF NOT EXISTS screening_preset (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    name VARCHAR(100) NOT NULL,                       -- 프리셋 이름
    description TEXT,                                 -- 설명

    filters JSONB NOT NULL DEFAULT '{}'::jsonb,       -- ScreeningRequest 형식

    is_default BOOLEAN DEFAULT false,

    sort_order INTEGER NOT NULL DEFAULT 0,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT unique_preset_name UNIQUE (name)
);

CREATE INDEX IF NOT EXISTS idx_screening_preset_sort ON screening_preset(sort_order, name);

CREATE INDEX IF NOT EXISTS idx_screening_preset_default ON screening_preset(is_default);

COMMENT ON TABLE screening_preset IS '스크리닝 프리셋 (Phase 3.3)';

COMMENT ON COLUMN screening_preset.name IS '프리셋 이름';

COMMENT ON COLUMN screening_preset.filters IS '필터 설정 JSON';

COMMENT ON COLUMN screening_preset.is_default IS '기본 프리셋 (삭제 불가)';

INSERT INTO screening_preset (name, description, filters, is_default, sort_order)
VALUES
    ('가치주', '저PER, 저PBR, 적정 ROE를 가진 저평가 종목',
     '{"max_per": "15", "max_pbr": "1.5", "min_roe": "5"}'::jsonb, true, 0),
    ('고배당주', '배당수익률 3% 이상, 안정적인 수익성',
     '{"min_dividend_yield": "3", "min_roe": "5"}'::jsonb, true, 1),
    ('성장주', '매출/이익 20% 이상 성장, 높은 ROE',
     '{"min_revenue_growth": "20", "min_earnings_growth": "20", "min_roe": "10"}'::jsonb, true, 2),
    ('스노우볼', '저PBR + 고배당 + 낮은 부채비율의 안정 성장주',
     '{"max_pbr": "1.0", "min_dividend_yield": "2", "max_debt_ratio": "100"}'::jsonb, true, 3),
    ('대형주', '시가총액 10조원 이상 우량 대형주',
     '{"min_market_cap": "10000000000000"}'::jsonb, true, 4),
    ('52주 신저가 근접', '52주 저가 근처에서 거래되는 수익성 있는 종목',
     '{"max_distance_from_52w_high": "-30", "min_roe": "5"}'::jsonb, true, 5)
ON CONFLICT (name) DO NOTHING;

INSERT INTO schema_migrations (version, filename, success, applied_at)
VALUES (14, '14_screening_presets.sql', true, NOW())
ON CONFLICT (version) DO NOTHING;

COMMENT ON TABLE exchange_credentials IS
    '거래소 및 데이터 제공자 API 자격증명 (AES-256-GCM 암호화). KRX Open API도 여기서 관리.';

INSERT INTO app_settings (setting_key, setting_value, description)
VALUES (
    'krx_api_info',
    'https://openapi.krx.co.kr',
    'KRX Open API 정보. API 키는 Settings > Credentials에서 등록하세요.'
)
ON CONFLICT (setting_key) DO NOTHING;

CREATE TABLE IF NOT EXISTS kis_token_cache (
    id SERIAL PRIMARY KEY,

    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,
    environment VARCHAR(10) NOT NULL DEFAULT 'real',  -- 'real' or 'paper'

    access_token TEXT NOT NULL,
    token_type VARCHAR(20) NOT NULL DEFAULT 'Bearer',
    expires_at TIMESTAMPTZ NOT NULL,

    websocket_key TEXT,
    websocket_key_expires_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT kis_token_cache_unique UNIQUE (credential_id, environment)
);

CREATE INDEX IF NOT EXISTS idx_kis_token_cache_credential
    ON kis_token_cache(credential_id);

CREATE INDEX IF NOT EXISTS idx_kis_token_cache_expires
    ON kis_token_cache(expires_at);

COMMENT ON TABLE kis_token_cache IS 'KIS OAuth 토큰 캐시. 1분당 1회 발급 제한 대응.';

COMMENT ON COLUMN kis_token_cache.credential_id IS '거래소 자격증명 ID (exchange_credentials.id)';

COMMENT ON COLUMN kis_token_cache.environment IS '환경: real(실전) 또는 paper(모의)';

COMMENT ON COLUMN kis_token_cache.access_token IS 'KIS 접근 토큰';

COMMENT ON COLUMN kis_token_cache.expires_at IS '토큰 만료 시각 (UTC)';

COMMENT ON COLUMN kis_token_cache.websocket_key IS 'WebSocket 접속 승인키';

CREATE OR REPLACE FUNCTION cleanup_expired_kis_tokens()
RETURNS void AS $$
BEGIN
    DELETE FROM kis_token_cache
    WHERE expires_at < NOW() - INTERVAL '1 hour';
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION cleanup_expired_kis_tokens IS '만료된 KIS 토큰 정리 (1시간 이상 만료된 토큰 삭제)';

-- ---------------------------------------------------------------------------
-- Source: 11_fix_watchlist_schema
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS watchlist_backup AS SELECT * FROM watchlist;

ALTER TABLE IF EXISTS watchlist_item DROP CONSTRAINT IF EXISTS watchlist_item_watchlist_id_fkey;

CREATE TABLE IF NOT EXISTS watchlist (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(100) NOT NULL,              -- 그룹 이름
    description TEXT,                         -- 그룹 설명
    color VARCHAR(20) DEFAULT '#3B82F6',     -- 테마 색상 (hex)
    icon VARCHAR(50) DEFAULT 'star',          -- 아이콘 이름
    sort_order INTEGER DEFAULT 0,             -- 정렬 순서
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_watchlist_sort ON watchlist(sort_order);

CREATE UNIQUE INDEX IF NOT EXISTS unique_watchlist_name ON watchlist(name);

COMMENT ON TABLE watchlist IS '관심종목 그룹 (폴더)';

COMMENT ON COLUMN watchlist.name IS '그룹 이름 (예: 모멘텀 종목, 배당주)';

COMMENT ON COLUMN watchlist.color IS '테마 색상 (hex, 예: #3B82F6)';

COMMENT ON COLUMN watchlist.icon IS '아이콘 이름 (예: star, heart, bookmark)';

CREATE TABLE IF NOT EXISTS watchlist_item (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    watchlist_id UUID NOT NULL,               -- 소속 그룹
    symbol VARCHAR(50) NOT NULL,              -- 심볼 (예: 005930, AAPL)
    market VARCHAR(10) NOT NULL,              -- 시장 (KR, US, CRYPTO)
    note TEXT,                                -- 메모
    sort_order INTEGER DEFAULT 0,             -- 그룹 내 정렬
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT watchlist_item_watchlist_id_fkey
        FOREIGN KEY (watchlist_id) REFERENCES watchlist(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_watchlist_item_watchlist ON watchlist_item(watchlist_id);

CREATE UNIQUE INDEX IF NOT EXISTS unique_watchlist_item ON watchlist_item(watchlist_id, symbol, market);

COMMENT ON TABLE watchlist_item IS '관심종목 그룹 내 개별 심볼';

INSERT INTO watchlist (name, description, color, icon, sort_order)
VALUES
    ('관심종목', '기본 관심종목 그룹', '#3B82F6', 'star', 0),
    ('모멘텀', '모멘텀 전략 종목', '#10B981', 'trending-up', 1),
    ('배당주', '배당 수익 종목', '#F59E0B', 'dollar-sign', 2)
ON CONFLICT (name) DO NOTHING;

DO $$
DECLARE
    default_group_id UUID;
BEGIN
    SELECT id INTO default_group_id FROM watchlist WHERE name = '관심종목' LIMIT 1;

    IF default_group_id IS NOT NULL AND EXISTS (SELECT 1 FROM watchlist_backup LIMIT 1) THEN
        INSERT INTO watchlist_item (watchlist_id, symbol, market, sort_order)
        SELECT default_group_id, symbol, market, sort_order
        FROM watchlist_backup
        WHERE symbol IS NOT NULL AND market IS NOT NULL
        ON CONFLICT (watchlist_id, symbol, market) DO NOTHING;
    END IF;
END $$;

-- ---------------------------------------------------------------------------
-- Source: 12_sync_checkpoint
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS sync_checkpoint (
    workflow_name VARCHAR(100) PRIMARY KEY,          -- 워크플로우 이름 (e.g., 'naver_fundamental', 'ohlcv_collect')
    last_ticker VARCHAR(50),                         -- 마지막 처리된 티커 (재개 지점)
    last_processed_at TIMESTAMPTZ,                   -- 마지막 처리 시간
    total_processed INTEGER DEFAULT 0,               -- 총 처리된 항목 수
    status VARCHAR(20) DEFAULT 'idle',               -- 상태: running, interrupted, completed, idle
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_sync_checkpoint_status ON sync_checkpoint(status);

CREATE INDEX IF NOT EXISTS idx_sync_checkpoint_updated ON sync_checkpoint(updated_at);

COMMENT ON TABLE sync_checkpoint IS '워크플로우 체크포인트 (배치 작업 중단/재개 지원)';

COMMENT ON COLUMN sync_checkpoint.workflow_name IS '워크플로우 고유 식별자 (e.g., naver_fundamental, ohlcv_collect, indicator_sync)';

COMMENT ON COLUMN sync_checkpoint.last_ticker IS '마지막 처리된 티커 (중단 시 재개 지점)';

COMMENT ON COLUMN sync_checkpoint.status IS '상태: running(실행중), interrupted(중단됨), completed(완료), idle(유휴)';

-- ---------------------------------------------------------------------------
-- Source: 17_notification_providers
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS email_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    smtp_host VARCHAR(255) NOT NULL,
    smtp_port INT NOT NULL DEFAULT 587,
    use_tls BOOLEAN NOT NULL DEFAULT true,
    encrypted_username BYTEA NOT NULL,
    encryption_nonce_username BYTEA NOT NULL,
    encrypted_password BYTEA NOT NULL,
    encryption_nonce_password BYTEA NOT NULL,
    from_email VARCHAR(255) NOT NULL,
    from_name VARCHAR(100),
    to_emails JSONB NOT NULL DEFAULT '[]',
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    notification_settings JSONB,
    last_message_at TIMESTAMPTZ,
    last_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_email_single_setting ON email_settings((1));

COMMENT ON TABLE email_settings IS 'SMTP 이메일 알림 설정';

COMMENT ON COLUMN email_settings.encrypted_username IS 'AES-256-GCM으로 암호화된 SMTP 사용자명';

COMMENT ON COLUMN email_settings.encrypted_password IS 'AES-256-GCM으로 암호화된 SMTP 비밀번호';

COMMENT ON COLUMN email_settings.to_emails IS '수신자 이메일 주소 배열 (JSON)';

CREATE TABLE IF NOT EXISTS discord_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    encrypted_webhook_url BYTEA NOT NULL,
    encryption_nonce_webhook BYTEA NOT NULL,
    display_name VARCHAR(100),
    server_name VARCHAR(100),
    channel_name VARCHAR(100),
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    notification_settings JSONB,
    last_message_at TIMESTAMPTZ,
    last_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_discord_single_setting ON discord_settings((1));

COMMENT ON TABLE discord_settings IS 'Discord Webhook 알림 설정';

COMMENT ON COLUMN discord_settings.encrypted_webhook_url IS 'AES-256-GCM으로 암호화된 Discord Webhook URL';

CREATE TABLE IF NOT EXISTS slack_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    encrypted_webhook_url BYTEA NOT NULL,
    encryption_nonce_webhook BYTEA NOT NULL,
    display_name VARCHAR(100),
    workspace_name VARCHAR(100),
    channel_name VARCHAR(100),
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    notification_settings JSONB,
    last_message_at TIMESTAMPTZ,
    last_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_slack_single_setting ON slack_settings((1));

COMMENT ON TABLE slack_settings IS 'Slack Webhook 알림 설정';

COMMENT ON COLUMN slack_settings.encrypted_webhook_url IS 'AES-256-GCM으로 암호화된 Slack Webhook URL';

CREATE TABLE IF NOT EXISTS sms_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider VARCHAR(50) NOT NULL DEFAULT 'twilio',
    encrypted_account_sid BYTEA NOT NULL,
    encryption_nonce_sid BYTEA NOT NULL,
    encrypted_auth_token BYTEA NOT NULL,
    encryption_nonce_token BYTEA NOT NULL,
    from_number VARCHAR(20) NOT NULL,
    to_numbers JSONB NOT NULL DEFAULT '[]',
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    notification_settings JSONB,
    last_message_at TIMESTAMPTZ,
    last_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_sms_single_setting ON sms_settings((1));

COMMENT ON TABLE sms_settings IS 'SMS 알림 설정 (Twilio)';

COMMENT ON COLUMN sms_settings.encrypted_account_sid IS 'AES-256-GCM으로 암호화된 Twilio Account SID';

COMMENT ON COLUMN sms_settings.encrypted_auth_token IS 'AES-256-GCM으로 암호화된 Twilio Auth Token';

COMMENT ON COLUMN sms_settings.to_numbers IS '수신자 전화번호 배열 (JSON, E.164 형식)';

CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_email_settings_updated_at
    BEFORE UPDATE ON email_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_discord_settings_updated_at
    BEFORE UPDATE ON discord_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_slack_settings_updated_at
    BEFORE UPDATE ON slack_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_sms_settings_updated_at
    BEFORE UPDATE ON sms_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

