-- =====================================================
-- 03_trading_analytics.sql (통합)
-- 거래 분석 및 저널 스키마
-- =====================================================
-- 원본: 07_trading_journal.sql, 08_portfolio_analytics.sql, 17_journal_executions_view.sql
-- =====================================================

-- =====================================================
-- 07_trading_journal.sql
-- 트레이딩 저널 시스템 (체결 내역 및 포지션 스냅샷)
-- =====================================================
--
-- 원본 마이그레이션: 015_trading_journal.sql, 016_positions_credential_id.sql
--
-- 포함 내용:
-- - trade_executions: 매매일지용 체결 내역 (메모, 태그 지원)
-- - position_snapshots: 시간별 포지션 스냅샷
-- - positions 테이블 확장 (credential_id, symbol_name, symbol 컬럼 추가)
--
-- 주의: 분석용 뷰들은 08_portfolio_analytics.sql에서 생성됩니다.
--
-- =====================================================

-- =====================================================
-- TRADE_EXECUTIONS TABLE
-- 매매일지용 체결 내역
-- =====================================================

CREATE TABLE trade_executions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 자격증명 연결 (어떤 계좌의 거래인지)
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,

    -- 거래 기본 정보
    exchange VARCHAR(50) NOT NULL,
    symbol VARCHAR(50) NOT NULL,                    -- "BTC/USDT", "005930" 등
    symbol_name VARCHAR(100),                       -- "삼성전자", "Bitcoin" 등 (표시용)

    -- 거래 유형
    side order_side NOT NULL,                       -- buy, sell
    order_type order_type NOT NULL,                 -- market, limit 등

    -- 수량 및 가격
    quantity DECIMAL(30, 15) NOT NULL,
    price DECIMAL(30, 15) NOT NULL,                 -- 체결가
    notional_value DECIMAL(30, 15) NOT NULL,        -- 거래대금 (quantity * price)

    -- 수수료
    fee DECIMAL(30, 15) DEFAULT 0,
    fee_currency VARCHAR(20),

    -- 포지션 영향
    position_effect VARCHAR(20),                    -- open, close, add, reduce
    realized_pnl DECIMAL(30, 15),                   -- 실현 손익 (청산 시)

    -- 주문 연결 (선택적)
    order_id UUID REFERENCES orders(id) ON DELETE SET NULL,
    exchange_order_id VARCHAR(100),
    exchange_trade_id VARCHAR(100),

    -- 전략 연결 (선택적)
    strategy_id VARCHAR(100),
    strategy_name VARCHAR(200),

    -- 체결 시간
    executed_at TIMESTAMPTZ NOT NULL,

    -- 메모 및 태그 (매매일지용)
    memo TEXT,                                      -- 사용자 메모
    tags JSONB DEFAULT '[]',                        -- 태그 배열 ["손절", "스윙"] 등

    -- 메타데이터
    metadata JSONB DEFAULT '{}',

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- 인덱스
CREATE INDEX idx_trade_executions_credential_time
    ON trade_executions(credential_id, executed_at DESC);

CREATE INDEX idx_trade_executions_symbol
    ON trade_executions(credential_id, symbol, executed_at DESC);

CREATE INDEX idx_trade_executions_strategy
    ON trade_executions(strategy_id, executed_at DESC)
    WHERE strategy_id IS NOT NULL;

CREATE INDEX idx_trade_executions_date
    ON trade_executions(credential_id, (executed_at::date));

COMMENT ON TABLE trade_executions IS '매매일지용 체결 내역. 거래 기록과 메모, 태그를 저장하여 트레이딩 분석 지원.';
COMMENT ON COLUMN trade_executions.position_effect IS '포지션 영향: open(신규진입), close(청산), add(추가매수), reduce(부분청산)';
COMMENT ON COLUMN trade_executions.tags IS '사용자 정의 태그. 예: ["손절", "스윙", "단타"]';

-- updated_at 트리거
CREATE TRIGGER update_trade_executions_updated_at
    BEFORE UPDATE ON trade_executions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- =====================================================
-- POSITION_SNAPSHOTS TABLE
-- 포지션 스냅샷 (시간별 추적)
-- =====================================================

CREATE TABLE position_snapshots (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 자격증명 연결
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,

    -- 스냅샷 시간
    snapshot_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 종목 정보
    exchange VARCHAR(50) NOT NULL,
    symbol VARCHAR(50) NOT NULL,
    symbol_name VARCHAR(100),

    -- 포지션 유형
    side order_side NOT NULL,                       -- buy(롱), sell(숏)

    -- 수량 및 가격
    quantity DECIMAL(30, 15) NOT NULL,
    entry_price DECIMAL(30, 15) NOT NULL,           -- 가중평균 매입가
    current_price DECIMAL(30, 15),                  -- 현재가

    -- 평가 금액
    cost_basis DECIMAL(30, 15) NOT NULL,            -- 매입 원가 (entry_price * quantity)
    market_value DECIMAL(30, 15),                   -- 평가 금액 (current_price * quantity)

    -- 손익
    unrealized_pnl DECIMAL(30, 15) DEFAULT 0,       -- 미실현 손익
    unrealized_pnl_pct DECIMAL(10, 4) DEFAULT 0,    -- 수익률 (%)
    realized_pnl DECIMAL(30, 15) DEFAULT 0,         -- 누적 실현 손익

    -- 포트폴리오 비중
    weight_pct DECIMAL(10, 4),                      -- 포트폴리오 내 비중 (%)

    -- 첫 매수 및 최근 거래
    first_trade_at TIMESTAMPTZ,
    last_trade_at TIMESTAMPTZ,
    trade_count INT DEFAULT 0,                      -- 해당 종목 거래 횟수

    -- 전략 연결 (선택적)
    strategy_id VARCHAR(100),

    -- 메타데이터
    metadata JSONB DEFAULT '{}',

    created_at TIMESTAMPTZ DEFAULT NOW(),

    -- 동일 credential, symbol, 시간대에 중복 방지
    UNIQUE(credential_id, symbol, snapshot_time)
);

-- 인덱스
CREATE INDEX idx_position_snapshots_credential_time
    ON position_snapshots(credential_id, snapshot_time DESC);

CREATE INDEX idx_position_snapshots_symbol
    ON position_snapshots(credential_id, symbol, snapshot_time DESC);

CREATE INDEX idx_position_snapshots_latest
    ON position_snapshots(credential_id, snapshot_time DESC)
    WHERE quantity > 0;

COMMENT ON TABLE position_snapshots IS '포지션 스냅샷. 시간별 포지션 상태를 기록하여 포지션 변화 추적.';
COMMENT ON COLUMN position_snapshots.entry_price IS '가중평균 매입가. (sum(price * quantity) / sum(quantity))';
COMMENT ON COLUMN position_snapshots.weight_pct IS '포트폴리오 내 비중. 총 자산 대비 해당 종목 비율.';

-- =====================================================
-- POSITIONS TABLE 확장
-- credential_id, symbol_name, symbol 컬럼 추가 (016)
-- =====================================================

-- 1. credential_id 컬럼 추가
ALTER TABLE positions
ADD COLUMN IF NOT EXISTS credential_id UUID REFERENCES exchange_credentials(id);

-- 2. symbol_name 컬럼 추가 (종목명 표시용)
ALTER TABLE positions
ADD COLUMN IF NOT EXISTS symbol_name VARCHAR(200);

-- 3. symbol 컬럼 추가 (심볼 문자열 직접 저장 - 거래소 중립)
ALTER TABLE positions
ADD COLUMN IF NOT EXISTS symbol VARCHAR(50);

-- 4. 기존 인덱스 삭제 후 재생성 (credential_id 포함)
DROP INDEX IF EXISTS idx_positions_open;
CREATE INDEX idx_positions_open_credential
ON positions (credential_id, exchange, symbol_id)
WHERE closed_at IS NULL;

-- 5. credential_id로 빠른 조회를 위한 인덱스
CREATE INDEX IF NOT EXISTS idx_positions_credential
ON positions (credential_id)
WHERE closed_at IS NULL;

-- 6. symbol로 조회를 위한 인덱스 (symbol_id 대신 사용 가능)
CREATE INDEX IF NOT EXISTS idx_positions_symbol
ON positions (credential_id, symbol)
WHERE closed_at IS NULL;

COMMENT ON COLUMN positions.credential_id IS '거래소 자격증명 ID (exchange_credentials.id)';
COMMENT ON COLUMN positions.symbol_name IS '종목명 (표시용)';
COMMENT ON COLUMN positions.symbol IS '심볼 코드 (예: 005930, AAPL)';

-- =====================================================
-- 사용 예시
-- =====================================================
--
-- 1. 특정 계좌의 최근 체결 내역 조회:
--    SELECT * FROM trade_executions
--    WHERE credential_id = ?
--    ORDER BY executed_at DESC LIMIT 100;
--
-- 2. 현재 보유 포지션 조회:
--    SELECT DISTINCT ON (credential_id, symbol) *
--    FROM position_snapshots
--    WHERE quantity > 0
--    ORDER BY credential_id, symbol, snapshot_time DESC;
--
-- 3. 특정 종목의 거래 이력 조회:
--    SELECT * FROM trade_executions
--    WHERE credential_id = ? AND symbol = '005930'
--    ORDER BY executed_at;
--
-- =====================================================

-- ============ 08_portfolio_analytics.sql ============

-- =====================================================
-- 08_portfolio_analytics.sql
-- 포트폴리오 분석 및 백테스팅 시스템
-- =====================================================
--
-- 원본 마이그레이션: 007_portfolio_equity_history.sql, 010_backtest_results.sql,
--                  030_add_missing_views.sql (분석 뷰들)
--
-- 포함 내용:
-- - portfolio_equity_history: 자산 곡선 데이터
-- - backtest_results: 백테스트 결과 저장
-- - 분석 뷰들: 전략 성과, 종목별 손익, 현재 포지션 등 (030에서 추가된 뷰 8개)
--
-- =====================================================

-- =====================================================
-- PORTFOLIO_EQUITY_HISTORY TABLE
-- 자산 곡선(Equity Curve) 데이터
-- =====================================================

CREATE TABLE portfolio_equity_history (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 자격증명 연결 (어떤 계좌의 데이터인지)
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,

    -- 스냅샷 시간
    snapshot_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 자산 가치
    total_equity DECIMAL(30, 15) NOT NULL,          -- 총 자산 (현금 + 평가금액)
    cash_balance DECIMAL(30, 15) NOT NULL,          -- 현금 잔고
    securities_value DECIMAL(30, 15) NOT NULL,      -- 유가증권 평가금액

    -- 수익/손실
    total_pnl DECIMAL(30, 15) DEFAULT 0,            -- 총 손익
    daily_pnl DECIMAL(30, 15) DEFAULT 0,            -- 당일 손익

    -- 메타데이터
    currency VARCHAR(10) DEFAULT 'KRW',             -- 통화
    market VARCHAR(10) DEFAULT 'KR',                -- 시장 (KR, US)
    account_type VARCHAR(20),                       -- 계좌 유형 (real, paper)

    metadata JSONB DEFAULT '{}',

    created_at TIMESTAMPTZ DEFAULT NOW(),

    -- 동일 credential, 동일 시간대에 중복 방지
    UNIQUE(credential_id, snapshot_time)
);

-- 인덱스
CREATE INDEX idx_equity_history_credential_time
    ON portfolio_equity_history(credential_id, snapshot_time DESC);

CREATE INDEX idx_equity_history_time
    ON portfolio_equity_history(snapshot_time DESC);

CREATE INDEX idx_equity_history_credential_time_asc
    ON portfolio_equity_history(credential_id, snapshot_time ASC);

CREATE INDEX idx_equity_history_month
    ON portfolio_equity_history(credential_id, (date_trunc('month', snapshot_time)));

CREATE INDEX idx_equity_history_year
    ON portfolio_equity_history(credential_id, (date_trunc('year', snapshot_time)));

COMMENT ON TABLE portfolio_equity_history IS '포트폴리오 자산 가치 히스토리. 자산 곡선(Equity Curve) 차트와 성과 분석에 사용됨.';
COMMENT ON COLUMN portfolio_equity_history.total_equity IS '총 자산 가치 (현금 + 유가증권 평가금액)';
COMMENT ON COLUMN portfolio_equity_history.daily_pnl IS '당일 손익. KIS API의 일별 손익 데이터 또는 전일 대비 계산값.';

-- =====================================================
-- BACKTEST_RESULTS TABLE
-- 백테스트 결과 저장
-- =====================================================

CREATE TABLE IF NOT EXISTS backtest_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- 전략 정보
    strategy_id VARCHAR(100) NOT NULL,              -- strategies 테이블의 id 참조
    strategy_type VARCHAR(50) NOT NULL,             -- 전략 타입 (sma_crossover, bollinger 등)

    -- 백테스트 설정
    symbol VARCHAR(500) NOT NULL,                   -- 심볼 (다중 자산은 콤마 구분)
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    initial_capital DECIMAL(20, 2) NOT NULL,
    slippage_rate DECIMAL(10, 6) DEFAULT 0.0005,

    -- 결과 (JSONB로 저장하여 유연성 확보)
    metrics JSONB NOT NULL,                         -- 성과 지표
    config_summary JSONB NOT NULL,                  -- 설정 요약
    equity_curve JSONB NOT NULL DEFAULT '[]',       -- 자산 곡선
    trades JSONB NOT NULL DEFAULT '[]',             -- 거래 내역

    -- 상태
    success BOOLEAN NOT NULL DEFAULT TRUE,
    error_message TEXT,

    -- 메타데이터
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ                          -- soft delete
);

-- 인덱스
CREATE INDEX idx_backtest_results_strategy
    ON backtest_results(strategy_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_backtest_results_type
    ON backtest_results(strategy_type, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_backtest_results_symbol
    ON backtest_results(symbol, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_backtest_results_recent
    ON backtest_results(created_at DESC)
    WHERE deleted_at IS NULL;

COMMENT ON TABLE backtest_results IS '백테스트 결과 저장 테이블. 전략별 백테스트 수행 결과를 영구 저장합니다.';
COMMENT ON COLUMN backtest_results.metrics IS '성과 지표 JSON: total_return_pct, annualized_return_pct, max_drawdown_pct, sharpe_ratio 등';
COMMENT ON COLUMN backtest_results.equity_curve IS '자산 곡선 JSON 배열: [{timestamp, equity, drawdown_pct}, ...]';
COMMENT ON COLUMN backtest_results.trades IS '거래 내역 JSON 배열: [{symbol, side, entry_price, exit_price, quantity, pnl, return_pct}, ...]';
COMMENT ON COLUMN backtest_results.deleted_at IS '소프트 삭제 시간. NULL이면 활성 상태';

-- =====================================================
-- ANALYTICAL VIEWS (030에서 추가된 뷰들)
-- =====================================================

-- 현재 포지션 뷰
CREATE VIEW public.journal_current_positions AS
 SELECT DISTINCT ON (position_snapshots.credential_id, position_snapshots.symbol) position_snapshots.id,
    position_snapshots.credential_id,
    position_snapshots.snapshot_time,
    position_snapshots.exchange,
    position_snapshots.symbol,
    position_snapshots.symbol_name,
    position_snapshots.side,
    position_snapshots.quantity,
    position_snapshots.entry_price,
    position_snapshots.current_price,
    position_snapshots.cost_basis,
    position_snapshots.market_value,
    position_snapshots.unrealized_pnl,
    position_snapshots.unrealized_pnl_pct,
    position_snapshots.realized_pnl,
    position_snapshots.weight_pct,
    position_snapshots.first_trade_at,
    position_snapshots.last_trade_at,
    position_snapshots.trade_count,
    position_snapshots.strategy_id
   FROM public.position_snapshots
  WHERE (position_snapshots.quantity > (0)::numeric)
  ORDER BY position_snapshots.credential_id, position_snapshots.symbol, position_snapshots.snapshot_time DESC;

-- 일별 자산 집계 뷰
CREATE VIEW public.portfolio_daily_equity AS
 SELECT portfolio_equity_history.credential_id,
    (date_trunc('day'::text, portfolio_equity_history.snapshot_time))::date AS date,
    (array_agg(portfolio_equity_history.total_equity ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS closing_equity,
    (array_agg(portfolio_equity_history.cash_balance ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS closing_cash,
    (array_agg(portfolio_equity_history.securities_value ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS closing_securities,
    (array_agg(portfolio_equity_history.total_pnl ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS total_pnl,
    (array_agg(portfolio_equity_history.daily_pnl ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS daily_pnl,
    max(portfolio_equity_history.total_equity) AS high_equity,
    min(portfolio_equity_history.total_equity) AS low_equity,
    count(*) AS snapshot_count
   FROM public.portfolio_equity_history
  GROUP BY portfolio_equity_history.credential_id, ((date_trunc('day'::text, portfolio_equity_history.snapshot_time))::date);

-- 월별 수익률 뷰
CREATE VIEW public.portfolio_monthly_returns AS
 WITH monthly_data AS (
         SELECT portfolio_equity_history.credential_id,
            (date_trunc('month'::text, portfolio_equity_history.snapshot_time))::date AS month,
            (array_agg(portfolio_equity_history.total_equity ORDER BY portfolio_equity_history.snapshot_time))[1] AS opening_equity,
            (array_agg(portfolio_equity_history.total_equity ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS closing_equity
           FROM public.portfolio_equity_history
          GROUP BY portfolio_equity_history.credential_id, ((date_trunc('month'::text, portfolio_equity_history.snapshot_time))::date)
        )
 SELECT monthly_data.credential_id,
    monthly_data.month,
    monthly_data.opening_equity,
    monthly_data.closing_equity,
        CASE
            WHEN (monthly_data.opening_equity > (0)::numeric) THEN (((monthly_data.closing_equity - monthly_data.opening_equity) / monthly_data.opening_equity) * (100)::numeric)
            ELSE (0)::numeric
        END AS return_pct
   FROM monthly_data;

-- 전략별 월간 성과 뷰
CREATE VIEW public.v_strategy_monthly_performance AS
 SELECT ec.credential_id,
    COALESCE(te.strategy_id, 'manual'::character varying) AS strategy_id,
    COALESCE(te.strategy_name, '수동 거래'::character varying) AS strategy_name,
    (EXTRACT(year FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)))::integer AS year,
    (EXTRACT(month FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)))::integer AS month,
    count(*) AS total_trades,
    COALESCE(sum(ec.amount), (0)::numeric) AS total_volume,
    COALESCE(sum(te.realized_pnl), (0)::numeric) AS realized_pnl,
    count(*) FILTER (WHERE (te.realized_pnl > (0)::numeric)) AS winning_trades,
    count(*) FILTER (WHERE (te.realized_pnl < (0)::numeric)) AS losing_trades
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))))
  GROUP BY ec.credential_id, COALESCE(te.strategy_id, 'manual'::character varying), COALESCE(te.strategy_name, '수동 거래'::character varying), (EXTRACT(year FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text))), (EXTRACT(month FROM (ec.executed_at AT TIME ZONE 'Asia/Seoul'::text)));

COMMENT ON VIEW public.v_strategy_monthly_performance IS '전략별 월간 성과 추이 뷰';

-- 전략별 성과 분석 뷰
CREATE VIEW public.v_strategy_performance AS
 SELECT ec.credential_id,
    COALESCE(te.strategy_id, 'manual'::character varying) AS strategy_id,
    COALESCE(te.strategy_name, '수동 거래'::character varying) AS strategy_name,
    count(*) AS total_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'buy'::text)) AS buy_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'sell'::text)) AS sell_trades,
    count(DISTINCT ec.symbol) AS unique_symbols,
    COALESCE(sum(ec.amount), (0)::numeric) AS total_volume,
    COALESCE(sum(ec.fee), (0)::numeric) AS total_fees,
    COALESCE(sum(te.realized_pnl), (0)::numeric) AS realized_pnl,
    count(*) FILTER (WHERE (te.realized_pnl > (0)::numeric)) AS winning_trades,
    count(*) FILTER (WHERE (te.realized_pnl < (0)::numeric)) AS losing_trades,
        CASE
            WHEN (count(*) FILTER (WHERE (te.realized_pnl IS NOT NULL)) > 0) THEN round((((count(*) FILTER (WHERE (te.realized_pnl > (0)::numeric)))::numeric * (100)::numeric) / (NULLIF(count(*) FILTER (WHERE (te.realized_pnl IS NOT NULL)), 0))::numeric), 2)
            ELSE (0)::numeric
        END AS win_rate_pct,
    COALESCE(avg(te.realized_pnl) FILTER (WHERE (te.realized_pnl > (0)::numeric)), (0)::numeric) AS avg_win,
    COALESCE(abs(avg(te.realized_pnl) FILTER (WHERE (te.realized_pnl < (0)::numeric))), (0)::numeric) AS avg_loss,
        CASE
            WHEN (COALESCE(abs(sum(te.realized_pnl) FILTER (WHERE (te.realized_pnl < (0)::numeric))), (0)::numeric) > (0)::numeric) THEN round((COALESCE(sum(te.realized_pnl) FILTER (WHERE (te.realized_pnl > (0)::numeric)), (0)::numeric) / abs(COALESCE(sum(te.realized_pnl) FILTER (WHERE (te.realized_pnl < (0)::numeric)), (1)::numeric))), 2)
            ELSE NULL::numeric
        END AS profit_factor,
    max(te.realized_pnl) AS largest_win,
    min(te.realized_pnl) AS largest_loss,
    count(DISTINCT date((ec.executed_at AT TIME ZONE 'Asia/Seoul'::text))) AS active_trading_days,
    min(ec.executed_at) AS first_trade_at,
    max(ec.executed_at) AS last_trade_at
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))))
  GROUP BY ec.credential_id, COALESCE(te.strategy_id, 'manual'::character varying), COALESCE(te.strategy_name, '수동 거래'::character varying);

COMMENT ON VIEW public.v_strategy_performance IS '전략별 성과 분석 뷰';

-- 종목별 손익 뷰
CREATE VIEW public.v_symbol_pnl AS
 SELECT ec.credential_id,
    ec.symbol,
    max((ec.normalized_symbol)::text) AS symbol_name,
    count(*) AS total_trades,
    COALESCE(sum(ec.quantity) FILTER (WHERE ((ec.side)::text = 'buy'::text)), (0)::numeric) AS total_buy_qty,
    COALESCE(sum(ec.quantity) FILTER (WHERE ((ec.side)::text = 'sell'::text)), (0)::numeric) AS total_sell_qty,
    COALESCE(sum(ec.amount) FILTER (WHERE ((ec.side)::text = 'buy'::text)), (0)::numeric) AS total_buy_value,
    COALESCE(sum(ec.amount) FILTER (WHERE ((ec.side)::text = 'sell'::text)), (0)::numeric) AS total_sell_value,
    COALESCE(sum(ec.fee), (0)::numeric) AS total_fees,
    COALESCE(sum(te.realized_pnl), (0)::numeric) AS realized_pnl,
    min(ec.executed_at) AS first_trade_at,
    max(ec.executed_at) AS last_trade_at
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))))
  GROUP BY ec.credential_id, ec.symbol;

COMMENT ON VIEW public.v_symbol_pnl IS '종목별 손익 집계 뷰';

-- 전체 PnL 요약 뷰
CREATE VIEW public.v_total_pnl AS
 SELECT ec.credential_id,
    COALESCE(sum(te.realized_pnl), (0)::numeric) AS total_realized_pnl,
    COALESCE(sum(ec.fee), (0)::numeric) AS total_fees,
    count(*) AS total_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'buy'::text)) AS buy_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'sell'::text)) AS sell_trades,
    count(*) FILTER (WHERE (te.realized_pnl > (0)::numeric)) AS winning_trades,
    count(*) FILTER (WHERE (te.realized_pnl < (0)::numeric)) AS losing_trades,
    COALESCE(sum(ec.amount), (0)::numeric) AS total_volume,
    min(ec.executed_at) AS first_trade_at,
    max(ec.executed_at) AS last_trade_at
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))))
  GROUP BY ec.credential_id;

COMMENT ON VIEW public.v_total_pnl IS '전체 PnL 요약 뷰';

-- 트레이딩 인사이트 뷰
CREATE VIEW public.v_trading_insights AS
 SELECT ec.credential_id,
    count(*) AS total_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'buy'::text)) AS buy_trades,
    count(*) FILTER (WHERE ((ec.side)::text = 'sell'::text)) AS sell_trades,
    count(DISTINCT ec.symbol) AS unique_symbols,
    COALESCE(sum(te.realized_pnl), (0)::numeric) AS total_realized_pnl,
    COALESCE(sum(ec.fee), (0)::numeric) AS total_fees,
    count(*) FILTER (WHERE (te.realized_pnl > (0)::numeric)) AS winning_trades,
    count(*) FILTER (WHERE (te.realized_pnl < (0)::numeric)) AS losing_trades,
        CASE
            WHEN (count(*) FILTER (WHERE (te.realized_pnl IS NOT NULL)) > 0) THEN round((((count(*) FILTER (WHERE (te.realized_pnl > (0)::numeric)))::numeric * (100)::numeric) / (NULLIF(count(*) FILTER (WHERE (te.realized_pnl IS NOT NULL)), 0))::numeric), 2)
            ELSE (0)::numeric
        END AS win_rate_pct,
    COALESCE(avg(te.realized_pnl) FILTER (WHERE (te.realized_pnl > (0)::numeric)), (0)::numeric) AS avg_win,
    COALESCE(abs(avg(te.realized_pnl) FILTER (WHERE (te.realized_pnl < (0)::numeric))), (0)::numeric) AS avg_loss,
        CASE
            WHEN (COALESCE(abs(sum(te.realized_pnl) FILTER (WHERE (te.realized_pnl < (0)::numeric))), (0)::numeric) > (0)::numeric) THEN round((COALESCE(sum(te.realized_pnl) FILTER (WHERE (te.realized_pnl > (0)::numeric)), (0)::numeric) / abs(COALESCE(sum(te.realized_pnl) FILTER (WHERE (te.realized_pnl < (0)::numeric)), (1)::numeric))), 2)
            ELSE NULL::numeric
        END AS profit_factor,
    (EXTRACT(day FROM (max(ec.executed_at) - min(ec.executed_at))))::integer AS trading_period_days,
    count(DISTINCT date((ec.executed_at AT TIME ZONE 'Asia/Seoul'::text))) AS active_trading_days,
    max(te.realized_pnl) AS largest_win,
    min(te.realized_pnl) AS largest_loss,
    min(ec.executed_at) AS first_trade_at,
    max(ec.executed_at) AS last_trade_at
   FROM (public.execution_cache ec
     LEFT JOIN public.trade_executions te ON (((te.credential_id = ec.credential_id) AND ((te.exchange)::text = (ec.exchange)::text) AND ((te.exchange_trade_id)::text = (ec.trade_id)::text))))
  GROUP BY ec.credential_id;

COMMENT ON VIEW public.v_trading_insights IS '투자 인사이트 통계 뷰';

-- ============ 17_journal_executions_view.sql ============

-- =====================================================
-- 17_journal_executions_view.sql
-- 통합 체결 내역 뷰 (execution_cache + trade_executions)
-- =====================================================
--
-- 이 뷰는 거래소에서 수집된 체결 내역(execution_cache)과
-- 사용자가 추가한 메모/태그(trade_executions)를 통합합니다.
--
-- =====================================================

-- 기존 뷰가 있으면 삭제
DROP VIEW IF EXISTS v_journal_executions;

-- 통합 체결 내역 뷰 생성
-- IMPORTANT: side 컬럼을 ::text로 캐스팅하여 sqlx VARCHAR/TEXT 호환성 확보
CREATE VIEW v_journal_executions AS
SELECT
    ec.id,
    ec.credential_id,
    ec.exchange,
    ec.symbol,
    ec.normalized_symbol AS symbol_name,
    ec.side::text AS side,                          -- VARCHAR -> TEXT 캐스팅 (sqlx 호환)
    COALESCE(ec.order_type, 'market') AS order_type,
    ec.quantity,
    ec.price,
    ec.amount AS notional_value,
    ec.fee,
    ec.fee_currency,
    te.position_effect,
    te.realized_pnl,
    te.order_id,
    ec.order_id AS exchange_order_id,
    ec.trade_id AS exchange_trade_id,
    te.strategy_id,
    te.strategy_name,
    ec.executed_at,
    te.memo,
    te.tags,
    COALESCE(te.metadata, '{}'::jsonb) AS metadata,
    ec.created_at,
    te.updated_at
FROM execution_cache ec
LEFT JOIN trade_executions te
    ON te.credential_id = ec.credential_id
    AND te.exchange = ec.exchange
    AND te.exchange_trade_id = ec.trade_id
ORDER BY ec.executed_at DESC;

COMMENT ON VIEW v_journal_executions IS '통합 체결 내역 뷰. execution_cache(거래소 데이터)와 trade_executions(메모/태그)를 결합.';

-- =====================================================
-- 일별 손익 뷰 (기존 v_daily_pnl이 없으면 생성)
-- =====================================================
CREATE OR REPLACE VIEW v_daily_pnl AS
SELECT
    ec.credential_id,
    (ec.executed_at AT TIME ZONE 'Asia/Seoul')::date AS trade_date,
    COUNT(*) AS total_trades,
    COUNT(*) FILTER (WHERE ec.side = 'buy') AS buy_count,
    COUNT(*) FILTER (WHERE ec.side = 'sell') AS sell_count,
    COALESCE(SUM(ec.amount), 0) AS total_volume,
    COALESCE(SUM(ec.fee), 0) AS total_fees,
    COALESCE(SUM(te.realized_pnl), 0) AS realized_pnl,
    COUNT(DISTINCT ec.symbol) AS symbol_count
FROM execution_cache ec
LEFT JOIN trade_executions te
    ON te.credential_id = ec.credential_id
    AND te.exchange = ec.exchange
    AND te.exchange_trade_id = ec.trade_id
GROUP BY ec.credential_id, (ec.executed_at AT TIME ZONE 'Asia/Seoul')::date;

COMMENT ON VIEW v_daily_pnl IS '일별 거래 요약 뷰';

-- =====================================================
-- 주별 손익 뷰
-- =====================================================
CREATE OR REPLACE VIEW v_weekly_pnl AS
SELECT
    ec.credential_id,
    date_trunc('week', ec.executed_at AT TIME ZONE 'Asia/Seoul')::date AS week_start,
    COUNT(*) AS total_trades,
    COUNT(*) FILTER (WHERE ec.side = 'buy') AS buy_count,
    COUNT(*) FILTER (WHERE ec.side = 'sell') AS sell_count,
    COALESCE(SUM(ec.amount), 0) AS total_volume,
    COALESCE(SUM(ec.fee), 0) AS total_fees,
    COALESCE(SUM(te.realized_pnl), 0) AS realized_pnl,
    COUNT(DISTINCT ec.symbol) AS symbol_count,
    COUNT(DISTINCT (ec.executed_at AT TIME ZONE 'Asia/Seoul')::date) AS trading_days
FROM execution_cache ec
LEFT JOIN trade_executions te
    ON te.credential_id = ec.credential_id
    AND te.exchange = ec.exchange
    AND te.exchange_trade_id = ec.trade_id
GROUP BY ec.credential_id, date_trunc('week', ec.executed_at AT TIME ZONE 'Asia/Seoul');

COMMENT ON VIEW v_weekly_pnl IS '주별 거래 요약 뷰';

-- =====================================================
-- 월별 손익 뷰
-- =====================================================
CREATE OR REPLACE VIEW v_monthly_pnl AS
SELECT
    ec.credential_id,
    EXTRACT(year FROM ec.executed_at AT TIME ZONE 'Asia/Seoul')::integer AS year,
    EXTRACT(month FROM ec.executed_at AT TIME ZONE 'Asia/Seoul')::integer AS month,
    COUNT(*) AS total_trades,
    COUNT(*) FILTER (WHERE ec.side = 'buy') AS buy_count,
    COUNT(*) FILTER (WHERE ec.side = 'sell') AS sell_count,
    COALESCE(SUM(ec.amount), 0) AS total_volume,
    COALESCE(SUM(ec.fee), 0) AS total_fees,
    COALESCE(SUM(te.realized_pnl), 0) AS realized_pnl,
    COUNT(DISTINCT ec.symbol) AS symbol_count,
    COUNT(DISTINCT (ec.executed_at AT TIME ZONE 'Asia/Seoul')::date) AS trading_days
FROM execution_cache ec
LEFT JOIN trade_executions te
    ON te.credential_id = ec.credential_id
    AND te.exchange = ec.exchange
    AND te.exchange_trade_id = ec.trade_id
GROUP BY ec.credential_id,
         EXTRACT(year FROM ec.executed_at AT TIME ZONE 'Asia/Seoul'),
         EXTRACT(month FROM ec.executed_at AT TIME ZONE 'Asia/Seoul');

COMMENT ON VIEW v_monthly_pnl IS '월별 거래 요약 뷰';

-- =====================================================
-- 연도별 손익 뷰
-- =====================================================
CREATE OR REPLACE VIEW v_yearly_pnl AS
SELECT
    ec.credential_id,
    EXTRACT(year FROM ec.executed_at AT TIME ZONE 'Asia/Seoul')::integer AS year,
    COUNT(*) AS total_trades,
    COUNT(*) FILTER (WHERE ec.side = 'buy') AS buy_count,
    COUNT(*) FILTER (WHERE ec.side = 'sell') AS sell_count,
    COALESCE(SUM(ec.amount), 0) AS total_volume,
    COALESCE(SUM(ec.fee), 0) AS total_fees,
    COALESCE(SUM(te.realized_pnl), 0) AS realized_pnl,
    COUNT(DISTINCT ec.symbol) AS symbol_count,
    COUNT(DISTINCT (ec.executed_at AT TIME ZONE 'Asia/Seoul')::date) AS trading_days,
    COUNT(DISTINCT EXTRACT(month FROM ec.executed_at AT TIME ZONE 'Asia/Seoul')) AS trading_months
FROM execution_cache ec
LEFT JOIN trade_executions te
    ON te.credential_id = ec.credential_id
    AND te.exchange = ec.exchange
    AND te.exchange_trade_id = ec.trade_id
GROUP BY ec.credential_id,
         EXTRACT(year FROM ec.executed_at AT TIME ZONE 'Asia/Seoul');

COMMENT ON VIEW v_yearly_pnl IS '연도별 거래 요약 뷰';

-- =====================================================
-- 누적 손익 뷰
-- =====================================================
DROP VIEW IF EXISTS v_cumulative_pnl CASCADE;

CREATE VIEW v_cumulative_pnl AS
WITH daily AS (
    SELECT
        credential_id,
        trade_date,
        total_trades,
        realized_pnl,
        total_fees
    FROM v_daily_pnl
)
SELECT
    d.credential_id,
    d.trade_date,
    d.total_trades,
    d.realized_pnl,
    d.total_fees,
    SUM(d.realized_pnl) OVER (PARTITION BY d.credential_id ORDER BY d.trade_date) AS cumulative_pnl,
    SUM(d.total_fees) OVER (PARTITION BY d.credential_id ORDER BY d.trade_date) AS cumulative_fees,
    SUM(d.total_trades) OVER (PARTITION BY d.credential_id ORDER BY d.trade_date)::bigint AS cumulative_trades
FROM daily d;

COMMENT ON VIEW v_cumulative_pnl IS '누적 손익 추이 뷰';
