-- =====================================================
-- 18. 스크리닝 뷰에 symbol_type 추가 (ETF 필터링 지원)
-- =====================================================
-- 목적: v_symbol_with_fundamental 뷰에 symbol_type 필드를 추가하여
--       ETF, ETN 등을 스크리닝에서 필터링할 수 있도록 함
-- 주의: 기존 04_strategy_signals.sql의 뷰 정의를 기반으로 symbol_type 추가

-- 기존 뷰 삭제 후 재생성 (컬럼 추가를 위해)
DROP VIEW IF EXISTS v_symbol_with_fundamental CASCADE;

-- 뷰 재생성 (symbol_type 추가 + 기존 모든 필드 유지)
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
    si.symbol_type,  -- ETF 필터링용 추가
    -- Fundamental 데이터
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
    -- 전략 관련 컬럼 (025, 026, 027)
    sf.route_state,
    sf.ttm_squeeze,
    sf.ttm_squeeze_cnt,
    sf.regime,
    -- 메타데이터
    sf.data_source AS fundamental_source,
    sf.fetched_at AS fundamental_fetched_at,
    sf.updated_at AS fundamental_updated_at
FROM symbol_info si
LEFT JOIN symbol_fundamental sf ON si.id = sf.symbol_info_id
WHERE si.is_active = true;

COMMENT ON VIEW v_symbol_with_fundamental IS '심볼 기본정보와 펀더멘털 통합 조회용 뷰 (symbol_type, route_state, ttm_squeeze, regime 포함)';
