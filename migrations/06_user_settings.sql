-- =====================================================
-- 06_user_settings.sql (통합)
-- 사용자 설정 및 거래소 통합 스키마
-- =====================================================
-- 원본: 11_migration_tracking.sql, 13_watchlist.sql, 14_screening_presets.sql, 
--       15_krx_api_settings.sql, 16_kis_token_cache.sql
-- =====================================================

-- =====================================================
-- 11_migration_tracking.sql
-- 마이그레이션 적용 이력 추적 시스템
-- =====================================================
--
-- 원본 마이그레이션: 033_migration_tracking.sql
--
-- 포함 내용:
-- - schema_migrations: 마이그레이션 적용 이력 추적
-- - 기존 34개 마이그레이션 기록 (이미 적용된 것으로 표시)
--
-- 기능:
-- - 어떤 마이그레이션이 적용되었는지 추적
-- - 실패한 마이그레이션 기록
-- - 실행 시간 측정
-- - SHA-256 체크섬 (향후 무결성 검증용)
--
-- =====================================================

-- =====================================================
-- SCHEMA_MIGRATIONS TABLE
-- 마이그레이션 적용 이력 추적
-- =====================================================

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,                    -- 마이그레이션 버전 번호
    filename VARCHAR(255) NOT NULL,                 -- 마이그레이션 파일명
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),  -- 적용 시간
    checksum VARCHAR(64),                           -- SHA-256 해시 (향후 구현)
    execution_time_ms INTEGER,                      -- 실행 시간 (ms)
    success BOOLEAN NOT NULL DEFAULT true,          -- 적용 성공 여부
    error_message TEXT                              -- 실패 시 에러 메시지
);

-- 인덱스
CREATE INDEX idx_schema_migrations_applied ON schema_migrations(applied_at DESC);
CREATE INDEX idx_schema_migrations_failed ON schema_migrations(success) WHERE success = false;

COMMENT ON TABLE schema_migrations IS '마이그레이션 적용 이력 추적';
COMMENT ON COLUMN schema_migrations.version IS '마이그레이션 버전 번호 (파일명의 숫자)';
COMMENT ON COLUMN schema_migrations.filename IS '마이그레이션 파일명 (예: 001_initial_schema.sql)';
COMMENT ON COLUMN schema_migrations.checksum IS 'SHA-256 체크섬 (무결성 검증용, 향후 구현)';
COMMENT ON COLUMN schema_migrations.success IS '적용 성공 여부 (실패 시 false)';

-- =====================================================
-- 기존 마이그레이션 기록
-- 현재까지 적용된 마이그레이션을 기록합니다
-- =====================================================

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

-- =====================================================
-- 사용 예시
-- =====================================================
--
-- 1. 적용된 마이그레이션 조회:
--    SELECT * FROM schema_migrations WHERE success = true ORDER BY version;
--
-- 2. 실패한 마이그레이션 조회:
--    SELECT * FROM schema_migrations WHERE success = false;
--
-- 3. 최근 적용된 마이그레이션:
--    SELECT * FROM schema_migrations ORDER BY applied_at DESC LIMIT 10;
--
-- 4. 새 마이그레이션 기록:
--    INSERT INTO schema_migrations (version, filename, success, execution_time_ms)
--    VALUES (34, '034_new_feature.sql', true, 1250);
--
-- =====================================================

-- ============ 13_watchlist.sql ============

-- =====================================================
-- 13_watchlist.sql
-- 관심종목 관리 시스템
-- =====================================================
--
-- 포함 내용:
-- - watchlist: 관심종목 그룹 테이블
-- - watchlist_item: 관심종목 아이템 테이블
-- - 인덱스: 조회 최적화
--
-- =====================================================

-- =====================================================
-- WATCHLIST TABLE
-- 관심종목 그룹
-- =====================================================

CREATE TABLE IF NOT EXISTS watchlist (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 기본 정보
    name VARCHAR(100) NOT NULL,                      -- 그룹 이름 (예: "모멘텀 종목", "저평가 주")
    description TEXT,                                -- 설명

    -- 정렬 순서
    sort_order INTEGER NOT NULL DEFAULT 0,           -- 표시 순서

    -- 색상/아이콘 (UI용)
    color VARCHAR(20),                               -- 색상 코드 (#FF5733)
    icon VARCHAR(50),                                -- 아이콘 이름 (star, chart, etc)

    -- 시스템 필드
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- 유일 제약 (이름 중복 방지)
    CONSTRAINT unique_watchlist_name UNIQUE (name)
);

-- 인덱스
CREATE INDEX idx_watchlist_sort ON watchlist(sort_order);

COMMENT ON TABLE watchlist IS '관심종목 그룹 (Phase 3.1)';
COMMENT ON COLUMN watchlist.name IS '그룹 이름';
COMMENT ON COLUMN watchlist.sort_order IS '표시 순서 (낮을수록 먼저)';

-- =====================================================
-- WATCHLIST_ITEM TABLE
-- 관심종목 아이템 (개별 종목)
-- =====================================================

CREATE TABLE IF NOT EXISTS watchlist_item (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 관계
    watchlist_id UUID NOT NULL REFERENCES watchlist(id) ON DELETE CASCADE,

    -- 종목 정보
    symbol VARCHAR(20) NOT NULL,                     -- 종목 코드 (005930, AAPL)
    market VARCHAR(20) NOT NULL DEFAULT 'KR',        -- 시장 (KR, US)

    -- 메모
    memo TEXT,                                       -- 사용자 메모

    -- 추가 정보
    target_price NUMERIC(20, 4),                     -- 목표가
    stop_price NUMERIC(20, 4),                       -- 손절가
    alert_enabled BOOLEAN DEFAULT false,             -- 알림 활성화

    -- 정렬 순서
    sort_order INTEGER NOT NULL DEFAULT 0,

    -- 추가 시점 가격 (비교용)
    added_price NUMERIC(20, 4),                      -- 추가 시점 가격

    -- 시스템 필드
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- 유일 제약 (그룹 내 종목 중복 방지)
    CONSTRAINT unique_watchlist_symbol UNIQUE (watchlist_id, symbol, market)
);

-- 인덱스
CREATE INDEX idx_watchlist_item_watchlist ON watchlist_item(watchlist_id);
CREATE INDEX idx_watchlist_item_symbol ON watchlist_item(symbol, market);
CREATE INDEX idx_watchlist_item_sort ON watchlist_item(watchlist_id, sort_order);

COMMENT ON TABLE watchlist_item IS '관심종목 아이템 (Phase 3.1)';
COMMENT ON COLUMN watchlist_item.symbol IS '종목 코드';
COMMENT ON COLUMN watchlist_item.market IS '시장 (KR/US)';
COMMENT ON COLUMN watchlist_item.target_price IS '목표가';
COMMENT ON COLUMN watchlist_item.stop_price IS '손절가';
COMMENT ON COLUMN watchlist_item.added_price IS '추가 시점 가격';

-- =====================================================
-- 기본 관심종목 그룹 생성
-- =====================================================

INSERT INTO watchlist (name, description, sort_order, icon, color)
VALUES
    ('기본', '기본 관심종목 목록', 0, 'star', '#FFD700'),
    ('모멘텀', '모멘텀 상위 종목', 1, 'trending-up', '#10B981'),
    ('가치주', '저평가 가치 종목', 2, 'search', '#3B82F6')
ON CONFLICT (name) DO NOTHING;

-- =====================================================
-- schema_migrations 기록
-- =====================================================

INSERT INTO schema_migrations (version, filename, success, applied_at)
VALUES (13, '13_watchlist.sql', true, NOW())
ON CONFLICT (version) DO NOTHING;

-- ============ 14_screening_presets.sql ============

-- =====================================================
-- 14_screening_presets.sql
-- 스크리닝 프리셋 저장
-- =====================================================
--
-- 사용자 정의 스크리닝 필터 프리셋을 저장합니다.
--
-- =====================================================

-- =====================================================
-- SCREENING_PRESET TABLE
-- 사용자 정의 스크리닝 프리셋
-- =====================================================

CREATE TABLE IF NOT EXISTS screening_preset (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 프리셋 기본 정보
    name VARCHAR(100) NOT NULL,                       -- 프리셋 이름
    description TEXT,                                 -- 설명

    -- 필터 설정 (JSONB)
    filters JSONB NOT NULL DEFAULT '{}'::jsonb,       -- ScreeningRequest 형식

    -- 기본 프리셋 여부 (삭제 불가)
    is_default BOOLEAN DEFAULT false,

    -- 정렬 순서
    sort_order INTEGER NOT NULL DEFAULT 0,

    -- 시스템 필드
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- 유일 제약 (이름 중복 방지)
    CONSTRAINT unique_preset_name UNIQUE (name)
);

-- 인덱스
CREATE INDEX idx_screening_preset_sort ON screening_preset(sort_order, name);
CREATE INDEX idx_screening_preset_default ON screening_preset(is_default);

COMMENT ON TABLE screening_preset IS '스크리닝 프리셋 (Phase 3.3)';
COMMENT ON COLUMN screening_preset.name IS '프리셋 이름';
COMMENT ON COLUMN screening_preset.filters IS '필터 설정 JSON';
COMMENT ON COLUMN screening_preset.is_default IS '기본 프리셋 (삭제 불가)';

-- =====================================================
-- 기본 프리셋 마이그레이션
-- =====================================================

INSERT INTO screening_preset (name, description, filters, is_default, sort_order)
VALUES
    -- 가치주
    ('가치주', '저PER, 저PBR, 적정 ROE를 가진 저평가 종목',
     '{"max_per": "15", "max_pbr": "1.5", "min_roe": "5"}'::jsonb, true, 0),
    -- 고배당주
    ('고배당주', '배당수익률 3% 이상, 안정적인 수익성',
     '{"min_dividend_yield": "3", "min_roe": "5"}'::jsonb, true, 1),
    -- 성장주
    ('성장주', '매출/이익 20% 이상 성장, 높은 ROE',
     '{"min_revenue_growth": "20", "min_earnings_growth": "20", "min_roe": "10"}'::jsonb, true, 2),
    -- 스노우볼
    ('스노우볼', '저PBR + 고배당 + 낮은 부채비율의 안정 성장주',
     '{"max_pbr": "1.0", "min_dividend_yield": "2", "max_debt_ratio": "100"}'::jsonb, true, 3),
    -- 대형주
    ('대형주', '시가총액 10조원 이상 우량 대형주',
     '{"min_market_cap": "10000000000000"}'::jsonb, true, 4),
    -- 52주 신저가 근접
    ('52주 신저가 근접', '52주 저가 근처에서 거래되는 수익성 있는 종목',
     '{"max_distance_from_52w_high": "-30", "min_roe": "5"}'::jsonb, true, 5)
ON CONFLICT (name) DO NOTHING;

-- =====================================================
-- schema_migrations 기록
-- =====================================================

INSERT INTO schema_migrations (version, filename, success, applied_at)
VALUES (14, '14_screening_presets.sql', true, NOW())
ON CONFLICT (version) DO NOTHING;

-- ============ 15_krx_api_settings.sql ============

-- =====================================================
-- 15_krx_api_settings.sql
-- KRX Open API Credential 지원
-- =====================================================
--
-- KRX Open API 인증키를 exchange_credentials 시스템으로 관리합니다.
-- UI에서 credential 등록 시 암호화되어 저장됩니다.
--
-- 저장 형식:
-- - exchange_id: 'krx'
-- - market_type: 'data_provider'
-- - encrypted_credentials: {"api_key": "YOUR_AUTH_KEY"} (암호화됨)
--
-- 사용법:
-- 1. Settings UI에서 "KRX Open API" credential 등록
-- 2. API Key 입력 후 저장 (자동 암호화)
-- 3. KrxApiClient.from_credential() 으로 사용
--
-- =====================================================

-- KRX credential이 이미 있을 수 있으므로, 중복 방지를 위한 코멘트만 추가
-- (실제 데이터는 UI에서 사용자가 등록)

COMMENT ON TABLE exchange_credentials IS
    '거래소 및 데이터 제공자 API 자격증명 (AES-256-GCM 암호화). KRX Open API도 여기서 관리.';

-- app_settings에 KRX 관련 안내 추가 (선택적)
INSERT INTO app_settings (setting_key, setting_value, description)
VALUES (
    'krx_api_info',
    'https://openapi.krx.co.kr',
    'KRX Open API 정보. API 키는 Settings > Credentials에서 등록하세요.'
)
ON CONFLICT (setting_key) DO NOTHING;

-- ============ 16_kis_token_cache.sql ============

-- =====================================================
-- 16_kis_token_cache.sql
-- KIS OAuth 토큰 DB 캐싱
-- =====================================================
--
-- KIS API는 토큰 발급에 1분당 1회 제한이 있습니다.
-- 토큰을 DB에 저장하여 서버 재시작 시에도 유효한 토큰을 재사용합니다.
--
-- =====================================================

CREATE TABLE IF NOT EXISTS kis_token_cache (
    id SERIAL PRIMARY KEY,

    -- 토큰 식별 (credential_id + environment)
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,
    environment VARCHAR(10) NOT NULL DEFAULT 'real',  -- 'real' or 'paper'

    -- 토큰 정보
    access_token TEXT NOT NULL,
    token_type VARCHAR(20) NOT NULL DEFAULT 'Bearer',
    expires_at TIMESTAMPTZ NOT NULL,

    -- WebSocket 접속키 (옵션)
    websocket_key TEXT,
    websocket_key_expires_at TIMESTAMPTZ,

    -- 메타데이터
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 하나의 credential + environment 조합당 하나의 토큰만 저장
    CONSTRAINT kis_token_cache_unique UNIQUE (credential_id, environment)
);

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_kis_token_cache_credential
    ON kis_token_cache(credential_id);
CREATE INDEX IF NOT EXISTS idx_kis_token_cache_expires
    ON kis_token_cache(expires_at);

-- 코멘트
COMMENT ON TABLE kis_token_cache IS 'KIS OAuth 토큰 캐시. 1분당 1회 발급 제한 대응.';
COMMENT ON COLUMN kis_token_cache.credential_id IS '거래소 자격증명 ID (exchange_credentials.id)';
COMMENT ON COLUMN kis_token_cache.environment IS '환경: real(실전) 또는 paper(모의)';
COMMENT ON COLUMN kis_token_cache.access_token IS 'KIS 접근 토큰';
COMMENT ON COLUMN kis_token_cache.expires_at IS '토큰 만료 시각 (UTC)';
COMMENT ON COLUMN kis_token_cache.websocket_key IS 'WebSocket 접속 승인키';

-- 만료된 토큰 자동 정리 함수 (선택적)
CREATE OR REPLACE FUNCTION cleanup_expired_kis_tokens()
RETURNS void AS $$
BEGIN
    DELETE FROM kis_token_cache
    WHERE expires_at < NOW() - INTERVAL '1 hour';
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION cleanup_expired_kis_tokens IS '만료된 KIS 토큰 정리 (1시간 이상 만료된 토큰 삭제)';
