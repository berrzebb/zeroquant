-- 알림 프로바이더 테이블 생성
-- Email, Discord, Slack, SMS(Twilio) 지원

-- ================================================================================================
-- Email 설정 테이블
-- SMTP를 통한 이메일 알림 설정
-- ================================================================================================
CREATE TABLE IF NOT EXISTS email_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- SMTP 서버 설정
    smtp_host VARCHAR(255) NOT NULL,
    smtp_port INT NOT NULL DEFAULT 587,
    use_tls BOOLEAN NOT NULL DEFAULT true,
    -- 암호화된 인증 정보
    encrypted_username BYTEA NOT NULL,
    encryption_nonce_username BYTEA NOT NULL,
    encrypted_password BYTEA NOT NULL,
    encryption_nonce_password BYTEA NOT NULL,
    -- 발신자 정보
    from_email VARCHAR(255) NOT NULL,
    from_name VARCHAR(100),
    -- 수신자 목록 (JSON 배열)
    to_emails JSONB NOT NULL DEFAULT '[]',
    -- 상태
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    notification_settings JSONB,
    -- 메타데이터
    last_message_at TIMESTAMPTZ,
    last_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 단일 설정만 허용 (계정당 하나의 이메일 설정)
CREATE UNIQUE INDEX IF NOT EXISTS idx_email_single_setting ON email_settings((1));

COMMENT ON TABLE email_settings IS 'SMTP 이메일 알림 설정';
COMMENT ON COLUMN email_settings.encrypted_username IS 'AES-256-GCM으로 암호화된 SMTP 사용자명';
COMMENT ON COLUMN email_settings.encrypted_password IS 'AES-256-GCM으로 암호화된 SMTP 비밀번호';
COMMENT ON COLUMN email_settings.to_emails IS '수신자 이메일 주소 배열 (JSON)';

-- ================================================================================================
-- Discord 설정 테이블
-- Discord Webhook을 통한 알림 설정
-- ================================================================================================
CREATE TABLE IF NOT EXISTS discord_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- 암호화된 Webhook URL
    encrypted_webhook_url BYTEA NOT NULL,
    encryption_nonce_webhook BYTEA NOT NULL,
    -- 표시 정보
    display_name VARCHAR(100),
    server_name VARCHAR(100),
    channel_name VARCHAR(100),
    -- 상태
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    notification_settings JSONB,
    -- 메타데이터
    last_message_at TIMESTAMPTZ,
    last_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 단일 설정만 허용
CREATE UNIQUE INDEX IF NOT EXISTS idx_discord_single_setting ON discord_settings((1));

COMMENT ON TABLE discord_settings IS 'Discord Webhook 알림 설정';
COMMENT ON COLUMN discord_settings.encrypted_webhook_url IS 'AES-256-GCM으로 암호화된 Discord Webhook URL';

-- ================================================================================================
-- Slack 설정 테이블
-- Slack Incoming Webhook을 통한 알림 설정
-- ================================================================================================
CREATE TABLE IF NOT EXISTS slack_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- 암호화된 Webhook URL
    encrypted_webhook_url BYTEA NOT NULL,
    encryption_nonce_webhook BYTEA NOT NULL,
    -- 표시 정보
    display_name VARCHAR(100),
    workspace_name VARCHAR(100),
    channel_name VARCHAR(100),
    -- 상태
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    notification_settings JSONB,
    -- 메타데이터
    last_message_at TIMESTAMPTZ,
    last_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 단일 설정만 허용
CREATE UNIQUE INDEX IF NOT EXISTS idx_slack_single_setting ON slack_settings((1));

COMMENT ON TABLE slack_settings IS 'Slack Webhook 알림 설정';
COMMENT ON COLUMN slack_settings.encrypted_webhook_url IS 'AES-256-GCM으로 암호화된 Slack Webhook URL';

-- ================================================================================================
-- SMS 설정 테이블 (Twilio)
-- Twilio를 통한 SMS 알림 설정
-- ================================================================================================
CREATE TABLE IF NOT EXISTS sms_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- 프로바이더 (확장성을 위해)
    provider VARCHAR(50) NOT NULL DEFAULT 'twilio',
    -- 암호화된 Twilio 인증 정보
    encrypted_account_sid BYTEA NOT NULL,
    encryption_nonce_sid BYTEA NOT NULL,
    encrypted_auth_token BYTEA NOT NULL,
    encryption_nonce_token BYTEA NOT NULL,
    -- 전화번호
    from_number VARCHAR(20) NOT NULL,
    to_numbers JSONB NOT NULL DEFAULT '[]',
    -- 상태
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    notification_settings JSONB,
    -- 메타데이터
    last_message_at TIMESTAMPTZ,
    last_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 단일 설정만 허용
CREATE UNIQUE INDEX IF NOT EXISTS idx_sms_single_setting ON sms_settings((1));

COMMENT ON TABLE sms_settings IS 'SMS 알림 설정 (Twilio)';
COMMENT ON COLUMN sms_settings.encrypted_account_sid IS 'AES-256-GCM으로 암호화된 Twilio Account SID';
COMMENT ON COLUMN sms_settings.encrypted_auth_token IS 'AES-256-GCM으로 암호화된 Twilio Auth Token';
COMMENT ON COLUMN sms_settings.to_numbers IS '수신자 전화번호 배열 (JSON, E.164 형식)';

-- ================================================================================================
-- updated_at 트리거 함수 (이미 존재하지 않는 경우에만 생성)
-- ================================================================================================
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- 각 테이블에 트리거 적용
DROP TRIGGER IF EXISTS update_email_settings_updated_at ON email_settings;
CREATE TRIGGER update_email_settings_updated_at
    BEFORE UPDATE ON email_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_discord_settings_updated_at ON discord_settings;
CREATE TRIGGER update_discord_settings_updated_at
    BEFORE UPDATE ON discord_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_slack_settings_updated_at ON slack_settings;
CREATE TRIGGER update_slack_settings_updated_at
    BEFORE UPDATE ON slack_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_sms_settings_updated_at ON sms_settings;
CREATE TRIGGER update_sms_settings_updated_at
    BEFORE UPDATE ON sms_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
