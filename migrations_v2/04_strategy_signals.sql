-- =============================================================================
-- 04_strategy_signals
-- signal_marker, alert_rule, alert_history
-- =============================================================================
-- 통합 마이그레이션 파일 (자동 생성)
-- 원본 파일: ["04_", "14_", "15_", "16_"]
-- =============================================================================

-- ---------------------------------------------------------------------------
-- Source: 04_strategy_signals
-- ---------------------------------------------------------------------------

DO $$ BEGIN
    CREATE TYPE route_state AS ENUM (
    'ATTACK',                                       -- 진입 적기 (강한 매수 신호)
    'ARMED',                                        -- 대기 준비 (조건 충족 임박)
    'WAIT',                                         -- 관찰 중 (중립)
    'OVERHEAT',                                     -- 과열 (매수 회피)
    'NEUTRAL'                                       -- 중립 (기본값)
);
EXCEPTION WHEN duplicate_object THEN NULL;
END $$

COMMENT ON TYPE route_state IS '전략 진입 상태: ATTACK(진입 적기), ARMED(대기), WAIT(관찰), OVERHEAT(과열), NEUTRAL(중립)';

ALTER TABLE strategies ADD COLUMN IF NOT EXISTS strategy_type VARCHAR(50);

ALTER TABLE strategies ADD COLUMN IF NOT EXISTS symbols JSONB DEFAULT '[]';

ALTER TABLE strategies ADD COLUMN IF NOT EXISTS market VARCHAR(20) DEFAULT 'KR';

ALTER TABLE strategies ADD COLUMN IF NOT EXISTS timeframe VARCHAR(10) DEFAULT '1d';

ALTER TABLE strategies ADD COLUMN IF NOT EXISTS allocated_capital DECIMAL(30, 15);

ALTER TABLE strategies ADD COLUMN IF NOT EXISTS risk_profile VARCHAR(20) DEFAULT 'default';

CREATE INDEX IF NOT EXISTS idx_strategies_type ON strategies(strategy_type);

CREATE INDEX IF NOT EXISTS idx_strategies_active ON strategies(is_active) WHERE is_active = true;

CREATE INDEX IF NOT EXISTS idx_strategies_risk_profile ON strategies(risk_profile);

COMMENT ON COLUMN strategies.strategy_type IS '전략 구현 타입 (grid_trading, rsi_mean_reversion, sma_crossover 등)';

COMMENT ON COLUMN strategies.symbols IS '전략이 운영하는 심볼 배열 (JSONB)';

COMMENT ON COLUMN strategies.market IS '시장 타입: KR, US, CRYPTO';

COMMENT ON COLUMN strategies.timeframe IS '전략 실행 타임프레임 (1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w, 1M)';

COMMENT ON COLUMN strategies.allocated_capital IS '전략에 할당된 자본 (NULL = 전체 계좌 잔고 사용)';

COMMENT ON COLUMN strategies.risk_limits IS 'RiskConfig 설정을 담은 JSON 객체';

COMMENT ON COLUMN strategies.risk_profile IS '리스크 프로파일: conservative, default, aggressive, custom';

ALTER TABLE symbol_fundamental
ADD COLUMN IF NOT EXISTS route_state route_state DEFAULT 'NEUTRAL';

CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_route_state
ON symbol_fundamental(route_state)
WHERE route_state IN ('ATTACK', 'ARMED');

COMMENT ON COLUMN symbol_fundamental.route_state IS 'RouteState 진입 신호: ATTACK(강매수), ARMED(대기), WAIT(관찰), OVERHEAT(과열), NEUTRAL(중립)';

ALTER TABLE symbol_fundamental
ADD COLUMN IF NOT EXISTS ttm_squeeze BOOLEAN DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS ttm_squeeze_cnt INTEGER DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_ttm_squeeze
ON symbol_fundamental(ttm_squeeze, ttm_squeeze_cnt DESC)
WHERE ttm_squeeze = TRUE;

COMMENT ON COLUMN symbol_fundamental.ttm_squeeze IS 'TTM Squeeze 상태 (BB가 KC 내부에 있으면 true - 에너지 응축)';

COMMENT ON COLUMN symbol_fundamental.ttm_squeeze_cnt IS 'TTM Squeeze 연속 카운트 (에너지 응축 기간, 높을수록 큰 변동성 예상)';

ALTER TABLE symbol_fundamental
ADD COLUMN IF NOT EXISTS regime VARCHAR(20);

CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_regime
ON symbol_fundamental(regime)
WHERE regime IS NOT NULL;

COMMENT ON COLUMN symbol_fundamental.regime IS '시장 레짐: STRONG_UPTREND, CORRECTION, SIDEWAYS, BOTTOM_BOUNCE, DOWNTREND';

CREATE VIEW v_symbol_with_fundamental AS
SELECT
    si.id,
    si.ticker,
    si.name,
    si.name_en,
    si.market,
    si.exchange,
    si.sector,
    si.yahoo_symbol,
    si.is_active,
    sf.market_cap,
    sf.per,
    sf.pbr,
    sf.eps,
    sf.bps,
    sf.dividend_yield,
    sf.roe,
    sf.roa,
    sf.operating_margin,
    sf.debt_ratio,
    sf.week_52_high,
    sf.week_52_low,
    sf.avg_volume_10d,
    sf.revenue,
    sf.operating_income,
    sf.net_income,
    sf.revenue_growth_yoy,
    sf.earnings_growth_yoy,
    sf.route_state,
    sf.ttm_squeeze,
    sf.ttm_squeeze_cnt,
    sf.regime,
    sf.data_source AS fundamental_source,
    sf.fetched_at AS fundamental_fetched_at,
    sf.updated_at AS fundamental_updated_at
FROM symbol_info si
LEFT JOIN symbol_fundamental sf ON si.id = sf.symbol_info_id
WHERE si.is_active = true;

COMMENT ON VIEW v_symbol_with_fundamental IS '심볼 기본정보와 펀더멘털 통합 조회용 뷰 (route_state, ttm_squeeze, regime 포함)';

CREATE TABLE IF NOT EXISTS signal_marker (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    symbol_id UUID NOT NULL REFERENCES symbol_info(id) ON DELETE CASCADE,

    timestamp TIMESTAMPTZ NOT NULL,                 -- 신호 발생 시간
    signal_type VARCHAR(20) NOT NULL,               -- Entry, Exit, Alert, AddToPosition, ReducePosition, Scale
    side VARCHAR(10),                               -- Buy, Sell (Alert의 경우 nullable)
    price NUMERIC(20, 8) NOT NULL,                  -- 신호 발생 시 가격
    strength DOUBLE PRECISION NOT NULL DEFAULT 0.0, -- 신호 강도 (0.0 ~ 1.0)

    indicators JSONB NOT NULL DEFAULT '{}'::jsonb,  -- RSI, MACD, BB, RouteState 등

    reason TEXT NOT NULL DEFAULT '',                -- 신호 발생 이유 (사람이 읽을 수 있는 형태)

    strategy_id VARCHAR(100) NOT NULL,
    strategy_name VARCHAR(200) NOT NULL,

    executed BOOLEAN NOT NULL DEFAULT false,        -- 백테스트에서 실제 체결 여부

    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,    -- 확장용 메타데이터

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_signal_marker_symbol_timestamp
ON signal_marker(symbol_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_signal_marker_strategy
ON signal_marker(strategy_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_signal_marker_signal_type
ON signal_marker(signal_type);

CREATE INDEX IF NOT EXISTS idx_signal_marker_executed
ON signal_marker(executed);

CREATE INDEX IF NOT EXISTS idx_signal_marker_indicators
ON signal_marker USING GIN (indicators);

COMMENT ON TABLE signal_marker IS '백테스트 및 실거래 신호 마커 (차트 표시 및 분석용)';

COMMENT ON COLUMN signal_marker.indicators IS '기술적 지표 값 (JSONB): RSI, MACD, BB, RouteState 등';

COMMENT ON COLUMN signal_marker.reason IS '신호 발생 이유 (예: "RSI 과매도 + MACD 골든크로스")';

COMMENT ON COLUMN signal_marker.executed IS '백테스트에서 실제 체결 여부';

COMMENT ON COLUMN signal_marker.metadata IS '확장용 메타데이터 (슬리피지, 수수료, 거부 사유 등)';

CREATE TABLE IF NOT EXISTS signal_alert_rule (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    rule_name VARCHAR(100) NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,

    filter_conditions JSONB NOT NULL DEFAULT '{}',

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT unique_rule_name UNIQUE(rule_name)
);

CREATE INDEX IF NOT EXISTS idx_signal_alert_rule_enabled
ON signal_alert_rule(enabled);

CREATE INDEX IF NOT EXISTS idx_signal_alert_rule_created_at
ON signal_alert_rule(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_signal_alert_rule_filter_conditions
ON signal_alert_rule USING GIN(filter_conditions);

CREATE OR REPLACE FUNCTION update_signal_alert_rule_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_signal_alert_rule_timestamp
    BEFORE UPDATE ON signal_alert_rule
    FOR EACH ROW
    EXECUTE FUNCTION update_signal_alert_rule_timestamp();

INSERT INTO signal_alert_rule (rule_name, description, filter_conditions)
VALUES
    (
        'high_strength_signals',
        '강도 80% 이상 모든 신호',
        '{"min_strength": 0.8}'::jsonb
    ),
    (
        'entry_signals_only',
        '진입 신호만 (강도 70% 이상)',
        '{"min_strength": 0.7, "entry_only": true}'::jsonb
    )
ON CONFLICT (rule_name) DO NOTHING;

COMMENT ON TABLE signal_alert_rule IS '신호 마커 알림 규칙';

COMMENT ON COLUMN signal_alert_rule.rule_name IS '규칙 이름 (고유)';

COMMENT ON COLUMN signal_alert_rule.filter_conditions IS '필터 조건 (JSONB: min_strength, strategy_ids, symbols, entry_only)';

ALTER TABLE strategies
ADD COLUMN IF NOT EXISTS multi_timeframe_config JSONB DEFAULT NULL;

CREATE INDEX IF NOT EXISTS idx_strategies_multi_tf
ON strategies ((multi_timeframe_config IS NOT NULL))
WHERE multi_timeframe_config IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_strategies_multi_tf_gin
ON strategies USING GIN (multi_timeframe_config)
WHERE multi_timeframe_config IS NOT NULL;

COMMENT ON COLUMN strategies.multi_timeframe_config IS '다중 타임프레임 설정 (JSONB): primary TF와 secondary TF 목록';

ALTER TABLE backtest_results
ADD COLUMN IF NOT EXISTS timeframes_used JSONB DEFAULT NULL;

COMMENT ON COLUMN backtest_results.timeframes_used IS '백테스트에 사용된 타임프레임 설정 (JSONB)';

CREATE OR REPLACE VIEW v_multi_timeframe_strategies AS
SELECT
    s.id,
    s.name,
    s.strategy_type,
    s.timeframe AS primary_timeframe,
    s.multi_timeframe_config,
    s.multi_timeframe_config->>'primary' AS config_primary,
    COALESCE(jsonb_array_length(s.multi_timeframe_config->'secondary'), 0) AS secondary_count,
    CASE
        WHEN s.multi_timeframe_config IS NOT NULL THEN
            jsonb_build_array(s.multi_timeframe_config->>'primary') ||
            COALESCE(
                (SELECT jsonb_agg(elem->>'timeframe')
                 FROM jsonb_array_elements(s.multi_timeframe_config->'secondary') AS elem),
                '[]'::jsonb
            )
        ELSE
            jsonb_build_array(s.timeframe)
    END AS all_timeframes,
    s.market,
    s.symbols,
    s.is_active,
    s.created_at,
    s.updated_at
FROM strategies s
WHERE s.multi_timeframe_config IS NOT NULL
ORDER BY s.updated_at DESC;

COMMENT ON VIEW v_multi_timeframe_strategies IS '다중 타임프레임 설정이 있는 전략만 조회하는 뷰';

CREATE OR REPLACE FUNCTION timeframe_to_seconds(tf VARCHAR)
RETURNS INTEGER AS $$
BEGIN
    RETURN CASE tf
        WHEN '1m' THEN 60
        WHEN '3m' THEN 180
        WHEN '5m' THEN 300
        WHEN '15m' THEN 900
        WHEN '30m' THEN 1800
        WHEN '1h' THEN 3600
        WHEN '2h' THEN 7200
        WHEN '4h' THEN 14400
        WHEN '6h' THEN 21600
        WHEN '8h' THEN 28800
        WHEN '12h' THEN 43200
        WHEN '1d' THEN 86400
        WHEN '3d' THEN 259200
        WHEN '1w' THEN 604800
        WHEN '1M' THEN 2592000  -- 30일 기준
        ELSE 0
    END;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION timeframe_to_seconds(VARCHAR) IS '타임프레임 문자열을 초 단위로 변환';

CREATE OR REPLACE FUNCTION validate_multi_timeframe_config(config JSONB)
RETURNS BOOLEAN AS $$
DECLARE
    primary_seconds INTEGER;
    secondary_record RECORD;
BEGIN
    IF config IS NULL THEN
        RETURN TRUE;  -- NULL은 단일 TF 전략
    END IF;

    primary_seconds := timeframe_to_seconds(config->>'primary');

    IF primary_seconds = 0 THEN
        RETURN FALSE;  -- 유효하지 않은 Primary TF
    END IF;

    FOR secondary_record IN
        SELECT elem->>'timeframe' AS tf
        FROM jsonb_array_elements(config->'secondary') AS elem
    LOOP
        IF timeframe_to_seconds(secondary_record.tf) <= primary_seconds THEN
            RETURN FALSE;  -- Secondary는 Primary보다 커야 함
        END IF;
    END LOOP;

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION validate_multi_timeframe_config(JSONB) IS '다중 타임프레임 설정 유효성 검증 (Secondary > Primary)';

ALTER TABLE strategies
ADD CONSTRAINT chk_multi_timeframe_valid
CHECK (validate_multi_timeframe_config(multi_timeframe_config));

ALTER TABLE backtest_results
ADD COLUMN IF NOT EXISTS timeframes_used JSONB DEFAULT NULL;

COMMENT ON COLUMN backtest_results.timeframes_used IS '백테스트에 사용된 타임프레임 설정 (JSONB)';

-- ---------------------------------------------------------------------------
-- Source: 14_default_alert_rules
-- ---------------------------------------------------------------------------

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

COMMENT ON TABLE signal_alert_rule IS '신호 마커 알림 규칙 - 기본 규칙 포함';

-- ---------------------------------------------------------------------------
-- Source: 15_signal_performance
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS signal_performance (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    signal_id UUID NOT NULL REFERENCES signal_marker(id) ON DELETE CASCADE,

    symbol_id UUID NOT NULL REFERENCES symbol_info(id) ON DELETE CASCADE,
    ticker VARCHAR(50) NOT NULL,

    signal_price NUMERIC(20, 8) NOT NULL,           -- 신호 발생 시 가격
    price_1d NUMERIC(20, 8),                        -- 1일 후 가격
    price_3d NUMERIC(20, 8),                        -- 3일 후 가격
    price_5d NUMERIC(20, 8),                        -- 5일 후 가격
    price_10d NUMERIC(20, 8),                       -- 10일 후 가격
    price_20d NUMERIC(20, 8),                       -- 20일 후 가격

    return_1d NUMERIC(10, 4),                       -- 1일 수익률
    return_3d NUMERIC(10, 4),                       -- 3일 수익률
    return_5d NUMERIC(10, 4),                       -- 5일 수익률
    return_10d NUMERIC(10, 4),                      -- 10일 수익률
    return_20d NUMERIC(10, 4),                      -- 20일 수익률

    max_return NUMERIC(10, 4),                      -- 최대 수익률 (MFE)
    max_drawdown NUMERIC(10, 4),                    -- 최대 손실률 (MAE)

    signal_type VARCHAR(20) NOT NULL,               -- Entry, Exit 등
    side VARCHAR(10),                               -- Buy, Sell
    strength NUMERIC(5, 4) NOT NULL,                -- 신호 강도 (0.0 ~ 1.0)
    strategy_id VARCHAR(100) NOT NULL,

    is_winner BOOLEAN,                              -- 승리 여부 (return_5d > 0 기준)

    calculated_at TIMESTAMPTZ,                      -- 성과 계산 시점
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT unique_signal_performance UNIQUE(signal_id)
);

CREATE INDEX IF NOT EXISTS idx_signal_performance_symbol ON signal_performance(symbol_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_signal_performance_ticker ON signal_performance(ticker, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_signal_performance_signal_type ON signal_performance(signal_type, side);

CREATE INDEX IF NOT EXISTS idx_signal_performance_strategy ON signal_performance(strategy_id);

CREATE INDEX IF NOT EXISTS idx_signal_performance_strength ON signal_performance(strength);

CREATE INDEX IF NOT EXISTS idx_signal_performance_winner ON signal_performance(is_winner) WHERE is_winner IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_signal_performance_calculated ON signal_performance(calculated_at) WHERE calculated_at IS NULL;

COMMENT ON TABLE signal_performance IS '신호별 성과 추적 (신호 품질 분석용)';

COMMENT ON COLUMN signal_performance.return_1d IS '1일 수익률 (%, 매도 신호는 부호 반전)';

COMMENT ON COLUMN signal_performance.max_return IS '최대 유리 변동률 MFE (Maximum Favorable Excursion)';

COMMENT ON COLUMN signal_performance.max_drawdown IS '최대 불리 변동률 MAE (Maximum Adverse Excursion)';

COMMENT ON COLUMN signal_performance.is_winner IS '승리 여부 (5일 수익률 기준, 매수: >0, 매도: <0)';

CREATE OR REPLACE FUNCTION update_signal_performance_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_signal_performance_timestamp
    BEFORE UPDATE ON signal_performance
    FOR EACH ROW
    EXECUTE FUNCTION update_signal_performance_timestamp();

CREATE OR REPLACE VIEW v_signal_type_stats AS
SELECT
    signal_type,
    side,
    COUNT(*) as total_signals,
    COUNT(*) FILTER (WHERE is_winner = true) as win_count,
    COUNT(*) FILTER (WHERE is_winner = false) as loss_count,
    ROUND(
        100.0 * COUNT(*) FILTER (WHERE is_winner = true) / NULLIF(COUNT(*) FILTER (WHERE is_winner IS NOT NULL), 0),
        2
    ) as win_rate,
    ROUND(AVG(return_1d)::NUMERIC, 4) as avg_return_1d,
    ROUND(AVG(return_5d)::NUMERIC, 4) as avg_return_5d,
    ROUND(AVG(return_10d)::NUMERIC, 4) as avg_return_10d,
    ROUND(AVG(max_return)::NUMERIC, 4) as avg_max_return,
    ROUND(AVG(max_drawdown)::NUMERIC, 4) as avg_max_drawdown
FROM signal_performance
WHERE calculated_at IS NOT NULL
GROUP BY signal_type, side;

COMMENT ON VIEW v_signal_type_stats IS '신호 타입별 성과 통계 (승률, 평균 수익률)';

CREATE OR REPLACE VIEW v_signal_strength_stats AS
SELECT
    CASE
        WHEN strength >= 0.9 THEN '90-100'
        WHEN strength >= 0.8 THEN '80-90'
        WHEN strength >= 0.7 THEN '70-80'
        WHEN strength >= 0.6 THEN '60-70'
        ELSE '50-60'
    END as strength_range,
    side,
    COUNT(*) as total_signals,
    ROUND(
        100.0 * COUNT(*) FILTER (WHERE is_winner = true) / NULLIF(COUNT(*) FILTER (WHERE is_winner IS NOT NULL), 0),
        2
    ) as win_rate,
    ROUND(AVG(return_5d)::NUMERIC, 4) as avg_return_5d,
    ROUND(AVG(max_return)::NUMERIC, 4) as avg_max_return,
    ROUND(AVG(max_drawdown)::NUMERIC, 4) as avg_max_drawdown
FROM signal_performance
WHERE calculated_at IS NOT NULL
GROUP BY
    CASE
        WHEN strength >= 0.9 THEN '90-100'
        WHEN strength >= 0.8 THEN '80-90'
        WHEN strength >= 0.7 THEN '70-80'
        WHEN strength >= 0.6 THEN '60-70'
        ELSE '50-60'
    END,
    side
ORDER BY strength_range DESC;

COMMENT ON VIEW v_signal_strength_stats IS '신호 강도별 성과 통계 (강도-수익률 상관관계)';

CREATE OR REPLACE VIEW v_signal_symbol_stats AS
SELECT
    sp.ticker,
    si.name as symbol_name,
    si.market,
    COUNT(*) as total_signals,
    COUNT(*) FILTER (WHERE sp.side = 'Buy') as buy_count,
    COUNT(*) FILTER (WHERE sp.side = 'Sell') as sell_count,
    ROUND(
        100.0 * COUNT(*) FILTER (WHERE sp.is_winner = true) / NULLIF(COUNT(*) FILTER (WHERE sp.is_winner IS NOT NULL), 0),
        2
    ) as win_rate,
    ROUND(AVG(sp.return_5d)::NUMERIC, 4) as avg_return_5d,
    ROUND(AVG(sp.strength)::NUMERIC, 4) as avg_strength
FROM signal_performance sp
JOIN symbol_info si ON sp.symbol_id = si.id
WHERE sp.calculated_at IS NOT NULL
GROUP BY sp.ticker, si.name, si.market
ORDER BY total_signals DESC;

COMMENT ON VIEW v_signal_symbol_stats IS '심볼별 신호 성과 통계';

CREATE OR REPLACE VIEW v_signal_strategy_stats AS
SELECT
    strategy_id,
    COUNT(*) as total_signals,
    COUNT(*) FILTER (WHERE is_winner = true) as win_count,
    ROUND(
        100.0 * COUNT(*) FILTER (WHERE is_winner = true) / NULLIF(COUNT(*) FILTER (WHERE is_winner IS NOT NULL), 0),
        2
    ) as win_rate,
    ROUND(AVG(return_1d)::NUMERIC, 4) as avg_return_1d,
    ROUND(AVG(return_5d)::NUMERIC, 4) as avg_return_5d,
    ROUND(AVG(strength)::NUMERIC, 4) as avg_strength,
    ROUND(AVG(max_return)::NUMERIC, 4) as avg_mfe,
    ROUND(AVG(max_drawdown)::NUMERIC, 4) as avg_mae
FROM signal_performance
WHERE calculated_at IS NOT NULL
GROUP BY strategy_id
ORDER BY win_rate DESC NULLS LAST;

COMMENT ON VIEW v_signal_strategy_stats IS '전략별 신호 성과 통계';

-- ---------------------------------------------------------------------------
-- Source: 16_alert_history
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS alert_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    rule_id UUID REFERENCES signal_alert_rule(id) ON DELETE SET NULL,
    signal_marker_id UUID REFERENCES signal_marker(id) ON DELETE SET NULL,

    alert_type VARCHAR(20) NOT NULL DEFAULT 'SIGNAL',

    channel VARCHAR(20) NOT NULL DEFAULT 'TELEGRAM',

    status VARCHAR(20) NOT NULL DEFAULT 'PENDING',

    title VARCHAR(200) NOT NULL,
    message TEXT NOT NULL,

    metadata JSONB NOT NULL DEFAULT '{}',

    error_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sent_at TIMESTAMPTZ,
    acknowledged_at TIMESTAMPTZ,
    acknowledged_by VARCHAR(100),

    CONSTRAINT valid_alert_type CHECK (alert_type IN ('SIGNAL', 'SYSTEM', 'ERROR')),
    CONSTRAINT valid_channel CHECK (channel IN ('TELEGRAM', 'EMAIL', 'WEBHOOK', 'SMS')),
    CONSTRAINT valid_status CHECK (status IN ('PENDING', 'SENT', 'FAILED', 'ACKNOWLEDGED'))
);

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

CREATE INDEX IF NOT EXISTS idx_alert_history_status_created
ON alert_history(status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_alert_history_metadata
ON alert_history USING GIN(metadata);

COMMENT ON TABLE alert_history IS '알림 발송 기록';

COMMENT ON COLUMN alert_history.rule_id IS '알림 규칙 ID (signal_alert_rule FK)';

COMMENT ON COLUMN alert_history.signal_marker_id IS '신호 마커 ID (signal_marker FK)';

COMMENT ON COLUMN alert_history.alert_type IS '알림 유형: SIGNAL, SYSTEM, ERROR';

COMMENT ON COLUMN alert_history.channel IS '알림 채널: TELEGRAM, EMAIL, WEBHOOK, SMS';

COMMENT ON COLUMN alert_history.status IS '알림 상태: PENDING, SENT, FAILED, ACKNOWLEDGED';

COMMENT ON COLUMN alert_history.metadata IS '추가 메타데이터 (심볼, 가격, 전략 등)';

COMMENT ON COLUMN alert_history.retry_count IS '재시도 횟수';

COMMENT ON COLUMN alert_history.acknowledged_by IS '확인한 사용자 (텔레그램 username 등)';

