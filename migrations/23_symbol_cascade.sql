-- ============================================================================
-- 23_symbol_cascade.sql
-- symbol_info 기준 데이터 무결성 관리 함수
-- ============================================================================
-- symbol_info 테이블이 마스터임에도 FK 제약이 없는 테이블들의 고아 데이터를 관리
-- (ohlcv 등은 symbol VARCHAR만 보유, market 없음 → DB-level FK 불가)
-- ============================================================================

-- ============================================================================
-- 1) cleanup_orphan_symbol_data()
-- symbol_info에 없는 심볼의 관련 데이터를 모든 테이블에서 삭제
-- ============================================================================
CREATE OR REPLACE FUNCTION cleanup_orphan_symbol_data()
RETURNS TABLE(table_name TEXT, deleted_count BIGINT) AS $$
DECLARE
    _del BIGINT;
BEGIN
    -- ─── ticker 컬럼 사용 테이블 ───
    -- (symbol_global_score, signal_performance은 FK CASCADE로 자동 처리되므로 제외)

    -- score_history: symbol 컬럼
    DELETE FROM score_history WHERE symbol NOT IN (SELECT ticker FROM symbol_info);
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'score_history'; deleted_count := _del; RETURN NEXT;
    END IF;

    -- ─── symbol 컬럼 사용 테이블 (ohlcv 계열) ───

    -- ohlcv: 대용량 테이블, TimescaleDB hypertable
    DELETE FROM ohlcv WHERE symbol NOT IN (SELECT ticker FROM symbol_info);
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'ohlcv'; deleted_count := _del; RETURN NEXT;
    END IF;

    -- ohlcv_metadata
    DELETE FROM ohlcv_metadata WHERE symbol NOT IN (SELECT ticker FROM symbol_info);
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'ohlcv_metadata'; deleted_count := _del; RETURN NEXT;
    END IF;

    -- ─── 실행/포지션 관련 테이블 ───

    -- execution_cache
    DELETE FROM execution_cache WHERE symbol NOT IN (SELECT ticker FROM symbol_info);
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'execution_cache'; deleted_count := _del; RETURN NEXT;
    END IF;

    -- execution_cache_meta (symbol 컬럼 존재 시)
    IF EXISTS (SELECT 1 FROM information_schema.columns c WHERE c.table_name = 'execution_cache_meta' AND c.column_name = 'symbol') THEN
        DELETE FROM execution_cache_meta WHERE symbol NOT IN (SELECT ticker FROM symbol_info);
        GET DIAGNOSTICS _del = ROW_COUNT;
        IF _del > 0 THEN
            table_name := 'execution_cache_meta'; deleted_count := _del; RETURN NEXT;
        END IF;
    END IF;

    -- trade_executions
    DELETE FROM trade_executions WHERE symbol NOT IN (SELECT ticker FROM symbol_info);
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'trade_executions'; deleted_count := _del; RETURN NEXT;
    END IF;

    -- position_snapshots
    DELETE FROM position_snapshots WHERE symbol NOT IN (SELECT ticker FROM symbol_info);
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'position_snapshots'; deleted_count := _del; RETURN NEXT;
    END IF;

    -- positions
    DELETE FROM positions WHERE symbol NOT IN (SELECT ticker FROM symbol_info);
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'positions'; deleted_count := _del; RETURN NEXT;
    END IF;

    -- ─── 모의거래 테이블 ───

    -- mock_executions
    DELETE FROM mock_executions WHERE symbol NOT IN (SELECT ticker FROM symbol_info);
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'mock_executions'; deleted_count := _del; RETURN NEXT;
    END IF;

    -- mock_positions
    DELETE FROM mock_positions WHERE symbol NOT IN (SELECT ticker FROM symbol_info);
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'mock_positions'; deleted_count := _del; RETURN NEXT;
    END IF;

    -- ─── 기타 테이블 ───

    -- price_snapshot
    DELETE FROM price_snapshot WHERE symbol NOT IN (SELECT ticker FROM symbol_info);
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'price_snapshot'; deleted_count := _del; RETURN NEXT;
    END IF;

    -- reality_check
    DELETE FROM reality_check WHERE symbol NOT IN (SELECT ticker FROM symbol_info);
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'reality_check'; deleted_count := _del; RETURN NEXT;
    END IF;

    -- watchlist_item
    DELETE FROM watchlist_item WHERE symbol NOT IN (SELECT ticker FROM symbol_info);
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'watchlist_item'; deleted_count := _del; RETURN NEXT;
    END IF;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION cleanup_orphan_symbol_data() IS
'symbol_info에 없는 심볼의 고아 데이터를 모든 관련 테이블에서 삭제. 삭제된 테이블별 건수 반환.';


-- ============================================================================
-- 2) delete_symbol_cascade(p_ticker, p_market)
-- 특정 심볼의 symbol_info 삭제 + 관련 데이터 연쇄 정리
-- ============================================================================
CREATE OR REPLACE FUNCTION delete_symbol_cascade(p_ticker TEXT, p_market TEXT)
RETURNS TABLE(table_name TEXT, deleted_count BIGINT) AS $$
DECLARE
    _del BIGINT;
    _symbol_id UUID;
BEGIN
    -- symbol_info ID 조회
    SELECT id INTO _symbol_id FROM symbol_info WHERE ticker = p_ticker AND market = p_market;

    IF _symbol_id IS NULL THEN
        RAISE EXCEPTION 'symbol_info not found: ticker=%, market=%', p_ticker, p_market;
    END IF;

    -- ─── FK CASCADE 자동 삭제 테이블 (symbol_info.id 참조) ───
    -- symbol_fundamental, symbol_global_score, signal_marker, signal_performance
    -- → symbol_info 삭제 시 ON DELETE CASCADE로 자동 처리

    -- ─── VARCHAR symbol/ticker 참조 테이블 수동 삭제 ───

    DELETE FROM ohlcv WHERE symbol = p_ticker;
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'ohlcv'; deleted_count := _del; RETURN NEXT;
    END IF;

    DELETE FROM ohlcv_metadata WHERE symbol = p_ticker;
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'ohlcv_metadata'; deleted_count := _del; RETURN NEXT;
    END IF;

    DELETE FROM score_history WHERE symbol = p_ticker;
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'score_history'; deleted_count := _del; RETURN NEXT;
    END IF;

    DELETE FROM execution_cache WHERE symbol = p_ticker;
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'execution_cache'; deleted_count := _del; RETURN NEXT;
    END IF;

    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'execution_cache_meta' AND column_name = 'symbol') THEN
        DELETE FROM execution_cache_meta WHERE symbol = p_ticker;
        GET DIAGNOSTICS _del = ROW_COUNT;
        IF _del > 0 THEN
            table_name := 'execution_cache_meta'; deleted_count := _del; RETURN NEXT;
        END IF;
    END IF;

    DELETE FROM trade_executions WHERE symbol = p_ticker;
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'trade_executions'; deleted_count := _del; RETURN NEXT;
    END IF;

    DELETE FROM position_snapshots WHERE symbol = p_ticker;
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'position_snapshots'; deleted_count := _del; RETURN NEXT;
    END IF;

    DELETE FROM positions WHERE symbol = p_ticker;
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'positions'; deleted_count := _del; RETURN NEXT;
    END IF;

    DELETE FROM mock_executions WHERE symbol = p_ticker;
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'mock_executions'; deleted_count := _del; RETURN NEXT;
    END IF;

    DELETE FROM mock_positions WHERE symbol = p_ticker;
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'mock_positions'; deleted_count := _del; RETURN NEXT;
    END IF;

    DELETE FROM price_snapshot WHERE symbol = p_ticker;
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'price_snapshot'; deleted_count := _del; RETURN NEXT;
    END IF;

    DELETE FROM reality_check WHERE symbol = p_ticker;
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'reality_check'; deleted_count := _del; RETURN NEXT;
    END IF;

    DELETE FROM watchlist_item WHERE symbol = p_ticker;
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'watchlist_item'; deleted_count := _del; RETURN NEXT;
    END IF;

    -- ─── symbol_info 삭제 (FK CASCADE 테이블 자동 정리) ───
    DELETE FROM symbol_info WHERE id = _symbol_id;
    GET DIAGNOSTICS _del = ROW_COUNT;
    IF _del > 0 THEN
        table_name := 'symbol_info'; deleted_count := _del; RETURN NEXT;
    END IF;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION delete_symbol_cascade(TEXT, TEXT) IS
'특정 심볼(ticker+market)의 symbol_info 삭제 시 관련 데이터를 모든 테이블에서 연쇄 삭제. 삭제된 테이블별 건수 반환.';
