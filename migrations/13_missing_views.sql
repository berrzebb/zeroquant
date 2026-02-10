-- 13_missing_views.sql
-- DB에 존재하지만 마이그레이션에 누락된 뷰/MV 동기화
--
-- 누락 원인: 초기 DB 셋업 시 수동 생성되었거나 스크립트에서 직접 생성됨
-- 목적: 새 환경에서 마이그레이션만으로 동일한 스키마 재현 가능하도록 보장

-- ============================================================================
-- 1. Journal 관련 뷰 (매매일지 분석용)
-- ============================================================================

-- journal_current_positions: 현재 보유 포지션 (최신 스냅샷)
CREATE OR REPLACE VIEW journal_current_positions AS
SELECT DISTINCT ON (position_snapshots.credential_id, position_snapshots.symbol)
    position_snapshots.id,
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
FROM position_snapshots
WHERE position_snapshots.quantity > 0::numeric
ORDER BY position_snapshots.credential_id, position_snapshots.symbol, position_snapshots.snapshot_time DESC;

COMMENT ON VIEW journal_current_positions IS '현재 보유 중인 포지션 (수량 > 0, 최신 스냅샷)';

-- journal_daily_summary: 일별 거래 요약
CREATE OR REPLACE VIEW journal_daily_summary AS
SELECT
    trade_executions.credential_id,
    date(trade_executions.executed_at) AS trade_date,
    count(*) AS total_trades,
    count(*) FILTER (WHERE trade_executions.side = 'buy'::order_side) AS buy_count,
    count(*) FILTER (WHERE trade_executions.side = 'sell'::order_side) AS sell_count,
    sum(trade_executions.notional_value) AS total_volume,
    sum(trade_executions.fee) AS total_fees,
    sum(trade_executions.realized_pnl) FILTER (WHERE trade_executions.realized_pnl IS NOT NULL) AS realized_pnl,
    count(DISTINCT trade_executions.symbol) AS symbol_count
FROM trade_executions
GROUP BY trade_executions.credential_id, date(trade_executions.executed_at);

COMMENT ON VIEW journal_daily_summary IS '일별 거래 요약 (매수/매도 건수, 거래량, 실현손익)';

-- journal_symbol_pnl: 종목별 손익 집계
CREATE OR REPLACE VIEW journal_symbol_pnl AS
SELECT
    trade_executions.credential_id,
    trade_executions.symbol,
    trade_executions.symbol_name,
    count(*) AS total_trades,
    sum(trade_executions.quantity) FILTER (WHERE trade_executions.side = 'buy'::order_side) AS total_buy_qty,
    sum(trade_executions.quantity) FILTER (WHERE trade_executions.side = 'sell'::order_side) AS total_sell_qty,
    sum(trade_executions.notional_value) FILTER (WHERE trade_executions.side = 'buy'::order_side) AS total_buy_value,
    sum(trade_executions.notional_value) FILTER (WHERE trade_executions.side = 'sell'::order_side) AS total_sell_value,
    sum(trade_executions.fee) AS total_fees,
    sum(COALESCE(trade_executions.realized_pnl, 0::numeric)) AS realized_pnl,
    min(trade_executions.executed_at) AS first_trade_at,
    max(trade_executions.executed_at) AS last_trade_at
FROM trade_executions
GROUP BY trade_executions.credential_id, trade_executions.symbol, trade_executions.symbol_name;

COMMENT ON VIEW journal_symbol_pnl IS '종목별 매매 손익 집계 (trade_executions 기반)';

-- ============================================================================
-- 2. Portfolio 관련 뷰 (포트폴리오 분석용)
-- ============================================================================

-- portfolio_daily_equity: 일별 자산 추이
CREATE OR REPLACE VIEW portfolio_daily_equity AS
SELECT
    portfolio_equity_history.credential_id,
    (date_trunc('day', portfolio_equity_history.snapshot_time))::date AS date,
    (array_agg(portfolio_equity_history.total_equity ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS closing_equity,
    (array_agg(portfolio_equity_history.cash_balance ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS closing_cash,
    (array_agg(portfolio_equity_history.securities_value ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS closing_securities,
    (array_agg(portfolio_equity_history.total_pnl ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS total_pnl,
    (array_agg(portfolio_equity_history.daily_pnl ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS daily_pnl,
    max(portfolio_equity_history.total_equity) AS high_equity,
    min(portfolio_equity_history.total_equity) AS low_equity,
    count(*) AS snapshot_count
FROM portfolio_equity_history
GROUP BY portfolio_equity_history.credential_id, (date_trunc('day', portfolio_equity_history.snapshot_time))::date;

COMMENT ON VIEW portfolio_daily_equity IS '일별 종가 기준 자산 추이 (시가/종가/고가/저가)';

-- portfolio_monthly_returns: 월별 수익률
CREATE OR REPLACE VIEW portfolio_monthly_returns AS
WITH monthly_data AS (
    SELECT
        portfolio_equity_history.credential_id,
        (date_trunc('month', portfolio_equity_history.snapshot_time))::date AS month,
        (array_agg(portfolio_equity_history.total_equity ORDER BY portfolio_equity_history.snapshot_time))[1] AS opening_equity,
        (array_agg(portfolio_equity_history.total_equity ORDER BY portfolio_equity_history.snapshot_time DESC))[1] AS closing_equity
    FROM portfolio_equity_history
    GROUP BY portfolio_equity_history.credential_id, (date_trunc('month', portfolio_equity_history.snapshot_time))::date
)
SELECT
    monthly_data.credential_id,
    monthly_data.month,
    monthly_data.opening_equity,
    monthly_data.closing_equity,
    CASE
        WHEN monthly_data.opening_equity > 0::numeric
        THEN ((monthly_data.closing_equity - monthly_data.opening_equity) / monthly_data.opening_equity) * 100::numeric
        ELSE 0::numeric
    END AS return_pct
FROM monthly_data;

COMMENT ON VIEW portfolio_monthly_returns IS '월별 수익률 (시가/종가 기준)';

-- ============================================================================
-- 3. 전략 성과 분석 뷰
-- ============================================================================

-- v_strategy_performance: 전략별 전체 성과
CREATE OR REPLACE VIEW v_strategy_performance AS
SELECT
    ec.credential_id,
    COALESCE(te.strategy_id, 'manual'::varchar) AS strategy_id,
    COALESCE(te.strategy_name, '수동 거래'::varchar) AS strategy_name,
    count(*) AS total_trades,
    count(*) FILTER (WHERE ec.side::text = 'buy') AS buy_trades,
    count(*) FILTER (WHERE ec.side::text = 'sell') AS sell_trades,
    count(DISTINCT ec.symbol) AS unique_symbols,
    COALESCE(sum(ec.amount), 0::numeric) AS total_volume,
    COALESCE(sum(ec.fee), 0::numeric) AS total_fees,
    COALESCE(sum(te.realized_pnl), 0::numeric) AS realized_pnl,
    count(*) FILTER (WHERE te.realized_pnl > 0::numeric) AS winning_trades,
    count(*) FILTER (WHERE te.realized_pnl < 0::numeric) AS losing_trades,
    CASE
        WHEN count(*) FILTER (WHERE te.realized_pnl IS NOT NULL) > 0
        THEN round((count(*) FILTER (WHERE te.realized_pnl > 0::numeric)::numeric * 100::numeric) /
             NULLIF(count(*) FILTER (WHERE te.realized_pnl IS NOT NULL), 0)::numeric, 2)
        ELSE 0::numeric
    END AS win_rate_pct,
    COALESCE(avg(te.realized_pnl) FILTER (WHERE te.realized_pnl > 0::numeric), 0::numeric) AS avg_win,
    COALESCE(abs(avg(te.realized_pnl) FILTER (WHERE te.realized_pnl < 0::numeric)), 0::numeric) AS avg_loss,
    CASE
        WHEN COALESCE(abs(sum(te.realized_pnl) FILTER (WHERE te.realized_pnl < 0::numeric)), 0::numeric) > 0::numeric
        THEN round(COALESCE(sum(te.realized_pnl) FILTER (WHERE te.realized_pnl > 0::numeric), 0::numeric) /
             abs(COALESCE(sum(te.realized_pnl) FILTER (WHERE te.realized_pnl < 0::numeric), 1::numeric)), 2)
        ELSE NULL::numeric
    END AS profit_factor,
    max(te.realized_pnl) AS largest_win,
    min(te.realized_pnl) AS largest_loss,
    count(DISTINCT date(ec.executed_at AT TIME ZONE 'Asia/Seoul')) AS active_trading_days,
    min(ec.executed_at) AS first_trade_at,
    max(ec.executed_at) AS last_trade_at
FROM execution_cache ec
LEFT JOIN trade_executions te ON te.credential_id = ec.credential_id
    AND te.exchange::text = ec.exchange::text
    AND te.exchange_trade_id::text = ec.trade_id::text
GROUP BY ec.credential_id, COALESCE(te.strategy_id, 'manual'::varchar), COALESCE(te.strategy_name, '수동 거래'::varchar);

COMMENT ON VIEW v_strategy_performance IS '전략별 전체 성과 요약 (승률, Profit Factor, 총 손익 등)';

-- v_strategy_monthly_performance: 전략별 월간 성과
CREATE OR REPLACE VIEW v_strategy_monthly_performance AS
SELECT
    ec.credential_id,
    COALESCE(te.strategy_id, 'manual'::varchar) AS strategy_id,
    COALESCE(te.strategy_name, '수동 거래'::varchar) AS strategy_name,
    EXTRACT(year FROM ec.executed_at AT TIME ZONE 'Asia/Seoul')::integer AS year,
    EXTRACT(month FROM ec.executed_at AT TIME ZONE 'Asia/Seoul')::integer AS month,
    count(*) AS total_trades,
    COALESCE(sum(ec.amount), 0::numeric) AS total_volume,
    COALESCE(sum(te.realized_pnl), 0::numeric) AS realized_pnl,
    count(*) FILTER (WHERE te.realized_pnl > 0::numeric) AS winning_trades,
    count(*) FILTER (WHERE te.realized_pnl < 0::numeric) AS losing_trades
FROM execution_cache ec
LEFT JOIN trade_executions te ON te.credential_id = ec.credential_id
    AND te.exchange::text = ec.exchange::text
    AND te.exchange_trade_id::text = ec.trade_id::text
GROUP BY ec.credential_id,
    COALESCE(te.strategy_id, 'manual'::varchar),
    COALESCE(te.strategy_name, '수동 거래'::varchar),
    EXTRACT(year FROM ec.executed_at AT TIME ZONE 'Asia/Seoul'),
    EXTRACT(month FROM ec.executed_at AT TIME ZONE 'Asia/Seoul');

COMMENT ON VIEW v_strategy_monthly_performance IS '전략별 월간 성과 (년/월별 거래량, 손익, 승패)';

-- ============================================================================
-- 4. 종합 분석 뷰
-- ============================================================================

-- v_symbol_pnl: 종목별 손익 (execution_cache 기반)
CREATE OR REPLACE VIEW v_symbol_pnl AS
SELECT
    ec.credential_id,
    ec.symbol,
    max(ec.normalized_symbol::text) AS symbol_name,
    count(*) AS total_trades,
    COALESCE(sum(ec.quantity) FILTER (WHERE ec.side::text = 'buy'), 0::numeric) AS total_buy_qty,
    COALESCE(sum(ec.quantity) FILTER (WHERE ec.side::text = 'sell'), 0::numeric) AS total_sell_qty,
    COALESCE(sum(ec.amount) FILTER (WHERE ec.side::text = 'buy'), 0::numeric) AS total_buy_value,
    COALESCE(sum(ec.amount) FILTER (WHERE ec.side::text = 'sell'), 0::numeric) AS total_sell_value,
    COALESCE(sum(ec.fee), 0::numeric) AS total_fees,
    COALESCE(sum(te.realized_pnl), 0::numeric) AS realized_pnl,
    min(ec.executed_at) AS first_trade_at,
    max(ec.executed_at) AS last_trade_at
FROM execution_cache ec
LEFT JOIN trade_executions te ON te.credential_id = ec.credential_id
    AND te.exchange::text = ec.exchange::text
    AND te.exchange_trade_id::text = ec.trade_id::text
GROUP BY ec.credential_id, ec.symbol;

COMMENT ON VIEW v_symbol_pnl IS '종목별 매매 손익 집계 (execution_cache + trade_executions 조인)';

-- v_total_pnl: 전체 손익 요약
CREATE OR REPLACE VIEW v_total_pnl AS
SELECT
    ec.credential_id,
    COALESCE(sum(te.realized_pnl), 0::numeric) AS total_realized_pnl,
    COALESCE(sum(ec.fee), 0::numeric) AS total_fees,
    count(*) AS total_trades,
    count(*) FILTER (WHERE ec.side::text = 'buy') AS buy_trades,
    count(*) FILTER (WHERE ec.side::text = 'sell') AS sell_trades,
    count(*) FILTER (WHERE te.realized_pnl > 0::numeric) AS winning_trades,
    count(*) FILTER (WHERE te.realized_pnl < 0::numeric) AS losing_trades,
    COALESCE(sum(ec.amount), 0::numeric) AS total_volume,
    min(ec.executed_at) AS first_trade_at,
    max(ec.executed_at) AS last_trade_at
FROM execution_cache ec
LEFT JOIN trade_executions te ON te.credential_id = ec.credential_id
    AND te.exchange::text = ec.exchange::text
    AND te.exchange_trade_id::text = ec.trade_id::text
GROUP BY ec.credential_id;

COMMENT ON VIEW v_total_pnl IS '계정별 전체 손익 요약 (총 실현손익, 수수료, 거래 통계)';

-- v_trading_insights: 트레이딩 인사이트 (종합 분석)
CREATE OR REPLACE VIEW v_trading_insights AS
SELECT
    ec.credential_id,
    count(*) AS total_trades,
    count(*) FILTER (WHERE ec.side::text = 'buy') AS buy_trades,
    count(*) FILTER (WHERE ec.side::text = 'sell') AS sell_trades,
    count(DISTINCT ec.symbol) AS unique_symbols,
    COALESCE(sum(te.realized_pnl), 0::numeric) AS total_realized_pnl,
    COALESCE(sum(ec.fee), 0::numeric) AS total_fees,
    count(*) FILTER (WHERE te.realized_pnl > 0::numeric) AS winning_trades,
    count(*) FILTER (WHERE te.realized_pnl < 0::numeric) AS losing_trades,
    CASE
        WHEN count(*) FILTER (WHERE te.realized_pnl IS NOT NULL) > 0
        THEN round((count(*) FILTER (WHERE te.realized_pnl > 0::numeric)::numeric * 100::numeric) /
             NULLIF(count(*) FILTER (WHERE te.realized_pnl IS NOT NULL), 0)::numeric, 2)
        ELSE 0::numeric
    END AS win_rate_pct,
    COALESCE(avg(te.realized_pnl) FILTER (WHERE te.realized_pnl > 0::numeric), 0::numeric) AS avg_win,
    COALESCE(abs(avg(te.realized_pnl) FILTER (WHERE te.realized_pnl < 0::numeric)), 0::numeric) AS avg_loss,
    CASE
        WHEN COALESCE(abs(sum(te.realized_pnl) FILTER (WHERE te.realized_pnl < 0::numeric)), 0::numeric) > 0::numeric
        THEN round(COALESCE(sum(te.realized_pnl) FILTER (WHERE te.realized_pnl > 0::numeric), 0::numeric) /
             abs(COALESCE(sum(te.realized_pnl) FILTER (WHERE te.realized_pnl < 0::numeric), 1::numeric)), 2)
        ELSE NULL::numeric
    END AS profit_factor,
    EXTRACT(day FROM max(ec.executed_at) - min(ec.executed_at))::integer AS trading_period_days,
    count(DISTINCT date(ec.executed_at AT TIME ZONE 'Asia/Seoul')) AS active_trading_days,
    max(te.realized_pnl) AS largest_win,
    min(te.realized_pnl) AS largest_loss,
    min(ec.executed_at) AS first_trade_at,
    max(ec.executed_at) AS last_trade_at
FROM execution_cache ec
LEFT JOIN trade_executions te ON te.credential_id = ec.credential_id
    AND te.exchange::text = ec.exchange::text
    AND te.exchange_trade_id::text = ec.trade_id::text
GROUP BY ec.credential_id;

COMMENT ON VIEW v_trading_insights IS '트레이딩 종합 인사이트 (승률, Profit Factor, 거래 기간, 활성 거래일 등)';

-- ============================================================================
-- 5. Materialized View
-- ============================================================================

-- mv_latest_prices: 최신 일봉 가격 (스크리닝 최적화용)
DROP MATERIALIZED VIEW IF EXISTS mv_latest_prices CASCADE;

CREATE MATERIALIZED VIEW mv_latest_prices AS
SELECT DISTINCT ON (ohlcv.symbol)
    ohlcv.symbol,
    ohlcv.open_time,
    ohlcv.open,
    ohlcv.high,
    ohlcv.low,
    ohlcv.close,
    ohlcv.volume
FROM ohlcv
WHERE ohlcv.timeframe::text = '1d'
ORDER BY ohlcv.symbol, ohlcv.open_time DESC;

-- 인덱스
CREATE UNIQUE INDEX IF NOT EXISTS idx_mv_latest_prices_symbol ON mv_latest_prices(symbol);

COMMENT ON MATERIALIZED VIEW mv_latest_prices IS '심볼별 최신 일봉 가격 (스크리닝 쿼리 최적화용)';

-- ============================================================================
-- 6. MV 새로고침 함수
-- ============================================================================

-- mv_latest_prices 새로고침 함수
CREATE OR REPLACE FUNCTION refresh_latest_prices()
RETURNS void AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY mv_latest_prices;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION refresh_latest_prices() IS 'mv_latest_prices Materialized View 갱신 (OHLCV 업데이트 후 호출)';
