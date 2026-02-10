-- =====================================================
-- 14_default_alert_rules.sql
-- 기본 알림 규칙 추가
-- =====================================================

-- ATTACK 상태 진입 시 알림 규칙
INSERT INTO signal_alert_rule (rule_name, description, filter_conditions)
VALUES
    (
        'attack_state_entry',
        'ATTACK 상태 진입 시 알림 (진입 적기)',
        '{
            "route_state": "ATTACK",
            "notify_on_state_change": true,
            "description": "종목이 ATTACK 상태로 전환될 때 알림"
        }'::jsonb
    ),
    (
        'rsi_oversold',
        'RSI 과매도 (RSI < 25) 알림',
        '{
            "indicator": "rsi",
            "operator": "lt",
            "value": 25,
            "description": "RSI가 25 미만으로 과매도 구간 진입 시 알림"
        }'::jsonb
    ),
    (
        'rsi_overbought',
        'RSI 과매수 (RSI > 75) 알림',
        '{
            "indicator": "rsi",
            "operator": "gt",
            "value": 75,
            "description": "RSI가 75 초과로 과매수 구간 진입 시 알림"
        }'::jsonb
    ),
    (
        'high_strength_entry',
        '고강도 진입 신호 (강도 > 80%, Entry만)',
        '{
            "min_strength": 0.8,
            "entry_only": true,
            "description": "강도 80% 이상의 진입 신호만 알림"
        }'::jsonb
    ),
    (
        'overheat_warning',
        'OVERHEAT 상태 경고 (과열 주의)',
        '{
            "route_state": "OVERHEAT",
            "notify_on_state_change": true,
            "description": "종목이 OVERHEAT 상태로 전환될 때 경고 알림"
        }'::jsonb
    )
ON CONFLICT (rule_name) DO UPDATE SET
    description = EXCLUDED.description,
    filter_conditions = EXCLUDED.filter_conditions,
    updated_at = NOW();

-- 코멘트 추가
COMMENT ON TABLE signal_alert_rule IS '신호 마커 알림 규칙 - 기본 규칙 포함';
