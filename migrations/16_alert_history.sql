-- =====================================================
-- MIGRATION 16: Alert History
-- 알림 발송 기록 관리
-- =====================================================

-- =====================================================
-- ALERT_HISTORY TABLE
-- 발송된 알림 기록 저장
-- =====================================================

CREATE TABLE IF NOT EXISTS alert_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- 연관 데이터
    rule_id UUID REFERENCES signal_alert_rule(id) ON DELETE SET NULL,
    signal_marker_id UUID REFERENCES signal_marker(id) ON DELETE SET NULL,

    -- 알림 유형
    -- SIGNAL: 신호 알림 (전략에서 발생)
    -- SYSTEM: 시스템 알림 (서버 상태, 스케줄 등)
    -- ERROR: 오류 알림 (장애, 예외 등)
    alert_type VARCHAR(20) NOT NULL DEFAULT 'SIGNAL',

    -- 알림 채널
    -- TELEGRAM, EMAIL, WEBHOOK, SMS
    channel VARCHAR(20) NOT NULL DEFAULT 'TELEGRAM',

    -- 알림 상태
    -- PENDING: 발송 대기
    -- SENT: 발송 완료
    -- FAILED: 발송 실패
    -- ACKNOWLEDGED: 사용자 확인
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING',

    -- 알림 내용
    title VARCHAR(200) NOT NULL,
    message TEXT NOT NULL,

    -- 메타데이터 (추가 정보)
    metadata JSONB NOT NULL DEFAULT '{}',

    -- 오류 정보 (실패 시)
    error_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,

    -- 타임스탬프
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sent_at TIMESTAMPTZ,
    acknowledged_at TIMESTAMPTZ,
    acknowledged_by VARCHAR(100),

    -- 제약조건
    CONSTRAINT valid_alert_type CHECK (alert_type IN ('SIGNAL', 'SYSTEM', 'ERROR')),
    CONSTRAINT valid_channel CHECK (channel IN ('TELEGRAM', 'EMAIL', 'WEBHOOK', 'SMS')),
    CONSTRAINT valid_status CHECK (status IN ('PENDING', 'SENT', 'FAILED', 'ACKNOWLEDGED'))
);

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_alert_history_rule_id
ON alert_history(rule_id);

CREATE INDEX IF NOT EXISTS idx_alert_history_signal_marker_id
ON alert_history(signal_marker_id);

CREATE INDEX IF NOT EXISTS idx_alert_history_status
ON alert_history(status);

CREATE INDEX IF NOT EXISTS idx_alert_history_alert_type
ON alert_history(alert_type);

CREATE INDEX IF NOT EXISTS idx_alert_history_channel
ON alert_history(channel);

CREATE INDEX IF NOT EXISTS idx_alert_history_created_at
ON alert_history(created_at DESC);

-- 복합 인덱스: 상태별 최근 알림 조회
CREATE INDEX IF NOT EXISTS idx_alert_history_status_created
ON alert_history(status, created_at DESC);

-- JSONB 인덱스: 메타데이터 검색
CREATE INDEX IF NOT EXISTS idx_alert_history_metadata
ON alert_history USING GIN(metadata);

-- 코멘트
COMMENT ON TABLE alert_history IS '알림 발송 기록';
COMMENT ON COLUMN alert_history.rule_id IS '알림 규칙 ID (signal_alert_rule FK)';
COMMENT ON COLUMN alert_history.signal_marker_id IS '신호 마커 ID (signal_marker FK)';
COMMENT ON COLUMN alert_history.alert_type IS '알림 유형: SIGNAL, SYSTEM, ERROR';
COMMENT ON COLUMN alert_history.channel IS '알림 채널: TELEGRAM, EMAIL, WEBHOOK, SMS';
COMMENT ON COLUMN alert_history.status IS '알림 상태: PENDING, SENT, FAILED, ACKNOWLEDGED';
COMMENT ON COLUMN alert_history.metadata IS '추가 메타데이터 (심볼, 가격, 전략 등)';
COMMENT ON COLUMN alert_history.retry_count IS '재시도 횟수';
COMMENT ON COLUMN alert_history.acknowledged_by IS '확인한 사용자 (텔레그램 username 등)';
