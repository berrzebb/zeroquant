-- =====================================================
-- 02_data_management.sql (통합)
-- 데이터 수집 및 관리 스키마
-- =====================================================
-- 원본: 04_symbol_metadata.sql, 05_market_data.sql, 06_execution_tracking.sql
-- =====================================================

-- =====================================================
-- 04_symbol_metadata.sql
-- 심볼 메타데이터 및 펀더멘털 정보 관리
-- =====================================================
--
-- 원본 마이그레이션: 012_symbol_info.sql, 020_symbol_fundamental.sql,
--                  021_fix_fundamental_decimal_precision.sql,
--                  023_symbol_fetch_failure_tracking.sql,
--                  024_add_symbol_type.sql
--
-- 포함 내용:
-- - symbol_info: 심볼 기본 정보 (티커, 회사명, 시장)
-- - symbol_fundamental: 펀더멘털 데이터 (PER, PBR, 재무 지표 등)
-- - 데이터 수집 실패 추적 (fetch_fail_count, last_fetch_error)
-- - 심볼 타입 구분 (STOCK, ETF, ETN, WARRANT 등)
-- - 펀더멘털 통합 뷰 (v_symbol_with_fundamental)
-- - 수집 실패 관리 함수 및 뷰
--
-- =====================================================

-- =====================================================
-- SYMBOL_INFO TABLE
-- 심볼 기본 정보 (티커, 회사명, 시장)
-- =====================================================

CREATE TABLE IF NOT EXISTS symbol_info (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 심볼 식별
    ticker VARCHAR(20) NOT NULL,                    -- 티커 코드 (AAPL, 005930)
    name VARCHAR(200) NOT NULL,                     -- 회사명 (Apple Inc., 삼성전자)
    name_en VARCHAR(200),                           -- 영문 회사명 (Samsung Electronics)

    -- 시장 정보
    market VARCHAR(20) NOT NULL,                    -- KR, US, JP 등
    exchange VARCHAR(50),                           -- NYSE, NASDAQ, KRX, KOSDAQ
    sector VARCHAR(100),                            -- 섹터/업종

    -- Yahoo Finance 호환 심볼
    yahoo_symbol VARCHAR(30),                       -- 005930.KS, AAPL

    -- 심볼 타입 (024에서 추가)
    symbol_type VARCHAR(20) DEFAULT 'STOCK',        -- STOCK, ETF, ETN, WARRANT, REIT, PREFERRED

    -- 상태
    is_active BOOLEAN DEFAULT true,

    -- 데이터 수집 실패 추적 (023에서 추가)
    fetch_fail_count INTEGER DEFAULT 0,             -- 연속 데이터 수집 실패 횟수
    last_fetch_error TEXT,                          -- 마지막 수집 실패 오류 메시지
    last_fetch_attempt TIMESTAMPTZ,                 -- 마지막 데이터 수집 시도 시간

    -- 시스템 필드
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- 유일 제약
    CONSTRAINT unique_symbol_market UNIQUE (ticker, market)
);

-- 기본 인덱스
CREATE INDEX IF NOT EXISTS idx_symbol_info_ticker ON symbol_info(ticker);
CREATE INDEX IF NOT EXISTS idx_symbol_info_name ON symbol_info(name);
CREATE INDEX IF NOT EXISTS idx_symbol_info_market ON symbol_info(market);
CREATE INDEX IF NOT EXISTS idx_symbol_info_yahoo ON symbol_info(yahoo_symbol);
CREATE INDEX IF NOT EXISTS idx_symbol_info_type ON symbol_info(symbol_type);

-- 전문 검색용 인덱스 (티커 + 회사명)
CREATE INDEX IF NOT EXISTS idx_symbol_info_search
ON symbol_info USING gin(to_tsvector('simple', ticker || ' ' || name || ' ' || COALESCE(name_en, '')));

-- 실패 추적용 인덱스
CREATE INDEX IF NOT EXISTS idx_symbol_info_fail_count
    ON symbol_info(fetch_fail_count DESC)
    WHERE is_active = true AND fetch_fail_count > 0;

CREATE INDEX IF NOT EXISTS idx_symbol_info_last_attempt
    ON symbol_info(last_fetch_attempt DESC)
    WHERE is_active = true;

COMMENT ON TABLE symbol_info IS '심볼 메타데이터 - 티커와 회사명 매핑';
COMMENT ON COLUMN symbol_info.ticker IS '티커 코드 (예: AAPL, 005930)';
COMMENT ON COLUMN symbol_info.name IS '회사명 (예: Apple Inc., 삼성전자)';
COMMENT ON COLUMN symbol_info.yahoo_symbol IS 'Yahoo Finance 호환 심볼 (예: 005930.KS, AAPL)';
COMMENT ON COLUMN symbol_info.symbol_type IS '종목 유형 (STOCK, ETF, ETN, WARRANT, REIT, PREFERRED)';
COMMENT ON COLUMN symbol_info.fetch_fail_count IS '연속 데이터 수집 실패 횟수';
COMMENT ON COLUMN symbol_info.last_fetch_error IS '마지막 수집 실패 오류 메시지';
COMMENT ON COLUMN symbol_info.last_fetch_attempt IS '마지막 데이터 수집 시도 시간';

-- =====================================================
-- SYMBOL_FUNDAMENTAL TABLE
-- 심볼 펀더멘털 정보 (PER, PBR, 재무 지표 등)
-- 주의: DECIMAL 정밀도는 021의 수정사항 반영 (극단값 허용)
-- =====================================================

CREATE TABLE IF NOT EXISTS symbol_fundamental (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- symbol_info 연계 (FK)
    symbol_info_id UUID NOT NULL REFERENCES symbol_info(id) ON DELETE CASCADE,

    -- 기본 시장 데이터
    market_cap DECIMAL(30, 2),                      -- 시가총액 (원/달러) - 삼성전자 1000조+ 지원
    shares_outstanding BIGINT,                      -- 발행주식수
    float_shares BIGINT,                            -- 유통주식수

    -- 가격 관련
    week_52_high DECIMAL(20, 4),                    -- 52주 최고가
    week_52_low DECIMAL(20, 4),                     -- 52주 최저가
    avg_volume_10d BIGINT,                          -- 10일 평균 거래량
    avg_volume_3m BIGINT,                           -- 3개월 평균 거래량

    -- 밸류에이션 지표
    per DECIMAL(12, 4),                             -- PER (주가수익비율)
    forward_per DECIMAL(12, 4),                     -- Forward PER
    pbr DECIMAL(12, 4),                             -- PBR (주가순자산비율)
    psr DECIMAL(12, 4),                             -- PSR (주가매출비율)
    pcr DECIMAL(12, 4),                             -- PCR (주가현금흐름비율)
    ev_ebitda DECIMAL(12, 4),                       -- EV/EBITDA

    -- 주당 지표
    eps DECIMAL(20, 4),                             -- EPS (주당순이익)
    bps DECIMAL(20, 4),                             -- BPS (주당순자산)
    dps DECIMAL(20, 4),                             -- DPS (주당배당금)
    sps DECIMAL(20, 4),                             -- SPS (주당매출)

    -- 배당 관련 (021: DECIMAL(12, 4)로 확장)
    dividend_yield DECIMAL(12, 4),                  -- 배당수익률 (%)
    dividend_payout_ratio DECIMAL(12, 4),           -- 배당성향 (%)
    ex_dividend_date DATE,                          -- 배당락일

    -- 재무제표 요약 (최근 연간)
    revenue DECIMAL(20, 2),                         -- 매출액
    operating_income DECIMAL(20, 2),                -- 영업이익
    net_income DECIMAL(20, 2),                      -- 순이익
    total_assets DECIMAL(20, 2),                    -- 총자산
    total_liabilities DECIMAL(20, 2),               -- 총부채
    total_equity DECIMAL(20, 2),                    -- 자기자본

    -- 수익성 지표 (021: DECIMAL(12, 4)로 확장 - 극단값 허용)
    roe DECIMAL(12, 4),                             -- ROE (자기자본이익률) %
    roa DECIMAL(12, 4),                             -- ROA (총자산이익률) %
    operating_margin DECIMAL(12, 4),                -- 영업이익률 % (적자 기업의 큰 음수 허용)
    net_profit_margin DECIMAL(12, 4),               -- 순이익률 %
    gross_margin DECIMAL(12, 4),                    -- 매출총이익률 %

    -- 안정성 지표
    debt_ratio DECIMAL(12, 4),                      -- 부채비율 (%)
    current_ratio DECIMAL(12, 4),                   -- 유동비율 (%)
    quick_ratio DECIMAL(12, 4),                     -- 당좌비율 (%)
    interest_coverage DECIMAL(12, 4),               -- 이자보상배율

    -- 외국인 투자 지표
    foreign_ratio DECIMAL(12, 4),                   -- 외국인 소진율 (%)

    -- 성장성 지표 (021: DECIMAL(12, 4)로 확장 - 스타트업 고성장 허용)
    revenue_growth_yoy DECIMAL(12, 4),              -- 매출 성장률 YoY %
    earnings_growth_yoy DECIMAL(12, 4),             -- 이익 성장률 YoY % (적자→흑자 전환 시 극단값 허용)
    revenue_growth_3y DECIMAL(12, 4),               -- 매출 3년 CAGR %
    earnings_growth_3y DECIMAL(12, 4),              -- 이익 3년 CAGR %

    -- 메타데이터
    data_source VARCHAR(50),                        -- 데이터 소스 (KRX, Yahoo, etc.)
    fiscal_year_end VARCHAR(10),                    -- 회계연도 종료월 (예: "12")
    currency VARCHAR(10) DEFAULT 'KRW',             -- 통화 (KRW, USD, etc.)

    -- 시스템 필드
    fetched_at TIMESTAMPTZ DEFAULT NOW(),           -- 데이터 수집 시점
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- 유일 제약 (한 심볼당 하나의 Fundamental 레코드)
    CONSTRAINT unique_symbol_fundamental UNIQUE (symbol_info_id)
);

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_symbol_id ON symbol_fundamental(symbol_info_id);
CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_market_cap ON symbol_fundamental(market_cap DESC);
CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_per ON symbol_fundamental(per);
CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_roe ON symbol_fundamental(roe DESC);
CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_dividend_yield ON symbol_fundamental(dividend_yield DESC);

-- 스크리닝용 복합 인덱스
CREATE INDEX IF NOT EXISTS idx_symbol_fundamental_valuation
ON symbol_fundamental(per, pbr, dividend_yield)
WHERE per IS NOT NULL AND pbr IS NOT NULL;

COMMENT ON TABLE symbol_fundamental IS '심볼 펀더멘털 데이터 - 시가총액, PER, PBR, 재무 지표 등';
COMMENT ON COLUMN symbol_fundamental.symbol_info_id IS 'symbol_info 테이블 FK';
COMMENT ON COLUMN symbol_fundamental.market_cap IS '시가총액 (통화 단위)';
COMMENT ON COLUMN symbol_fundamental.per IS '주가수익비율 (Price to Earnings Ratio)';
COMMENT ON COLUMN symbol_fundamental.pbr IS '주가순자산비율 (Price to Book Ratio)';
COMMENT ON COLUMN symbol_fundamental.roe IS 'ROE (자기자본이익률) % - 극단적 값 허용';
COMMENT ON COLUMN symbol_fundamental.operating_margin IS '영업이익률 % - 적자 기업의 큰 음수 허용';
COMMENT ON COLUMN symbol_fundamental.revenue_growth_yoy IS '매출 성장률 YoY % - 스타트업 고성장 허용';
COMMENT ON COLUMN symbol_fundamental.earnings_growth_yoy IS '이익 성장률 YoY % - 적자→흑자 전환 시 극단값 허용';

-- =====================================================
-- VIEWS
-- =====================================================

-- 심볼 정보 + Fundamental 통합 뷰
CREATE OR REPLACE VIEW v_symbol_with_fundamental AS
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
    sf.data_source AS fundamental_source,
    sf.fetched_at AS fundamental_fetched_at,
    sf.updated_at AS fundamental_updated_at
FROM symbol_info si
LEFT JOIN symbol_fundamental sf ON si.id = sf.symbol_info_id
WHERE si.is_active = true;

COMMENT ON VIEW v_symbol_with_fundamental IS '심볼 기본정보와 펀더멘털 통합 조회용 뷰';

-- 실패 심볼 현황 뷰 (023)
CREATE OR REPLACE VIEW v_symbol_fetch_failures AS
SELECT
    si.id,
    si.ticker,
    si.name,
    si.market,
    si.exchange,
    si.yahoo_symbol,
    si.is_active,
    si.fetch_fail_count,
    si.last_fetch_error,
    si.last_fetch_attempt,
    CASE
        WHEN si.fetch_fail_count >= 3 THEN 'CRITICAL'
        WHEN si.fetch_fail_count >= 2 THEN 'WARNING'
        WHEN si.fetch_fail_count >= 1 THEN 'MINOR'
        ELSE 'OK'
    END AS failure_level
FROM symbol_info si
WHERE si.fetch_fail_count > 0
ORDER BY si.fetch_fail_count DESC, si.last_fetch_attempt DESC;

COMMENT ON VIEW v_symbol_fetch_failures IS '데이터 수집 실패 심볼 현황 뷰. 실패 횟수별 레벨 표시';

-- =====================================================
-- FUNCTIONS (023)
-- 데이터 수집 실패 추적 함수
-- =====================================================

-- 심볼 데이터 수집 실패 기록 함수
CREATE OR REPLACE FUNCTION record_symbol_fetch_failure(
    p_symbol_info_id UUID,
    p_error_message TEXT,
    p_max_failures INTEGER DEFAULT 3
)
RETURNS BOOLEAN AS $$
DECLARE
    v_new_count INTEGER;
    v_deactivated BOOLEAN := FALSE;
BEGIN
    UPDATE symbol_info
    SET fetch_fail_count = fetch_fail_count + 1,
        last_fetch_error = p_error_message,
        last_fetch_attempt = NOW(),
        updated_at = NOW()
    WHERE id = p_symbol_info_id
    RETURNING fetch_fail_count INTO v_new_count;

    -- N회 이상 실패 시 자동 비활성화
    IF v_new_count >= p_max_failures THEN
        UPDATE symbol_info
        SET is_active = FALSE,
            updated_at = NOW()
        WHERE id = p_symbol_info_id;
        v_deactivated := TRUE;
    END IF;

    RETURN v_deactivated;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION record_symbol_fetch_failure(UUID, TEXT, INTEGER) IS
'심볼 데이터 수집 실패 기록. max_failures 초과 시 자동 비활성화하고 TRUE 반환';

-- 심볼 데이터 수집 성공 시 실패 카운트 초기화 함수
CREATE OR REPLACE FUNCTION reset_symbol_fetch_failure(
    p_symbol_info_id UUID
)
RETURNS void AS $$
BEGIN
    UPDATE symbol_info
    SET fetch_fail_count = 0,
        last_fetch_error = NULL,
        last_fetch_attempt = NOW(),
        updated_at = NOW()
    WHERE id = p_symbol_info_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION reset_symbol_fetch_failure(UUID) IS
'심볼 데이터 수집 성공 시 실패 카운트 초기화';

-- ============ 05_market_data.sql ============

-- =====================================================
-- 05_market_data.sql
-- 시장 데이터 (OHLCV) 캐시 및 최신 가격 뷰
-- =====================================================
--
-- 원본 마이그레이션: 005_yahoo_candle_cache.sql, 009_rename_candle_cache.sql,
--                  022_latest_prices_materialized_view.sql
--
-- 포함 내용:
-- - ohlcv: OHLCV 캔들 데이터 (TimescaleDB Hypertable, 압축 정책)
-- - ohlcv_metadata: 심볼별 캐시 상태 메타데이터
-- - mv_latest_prices: 심볼별 최신 일봉 가격 (Materialized View)
-- - 캐시 갭 조회 함수, 통계 조회 함수
-- - 자동 메타데이터 업데이트 트리거
--
-- 데이터 소스: Yahoo Finance, KRX 등 통합
--
-- =====================================================

-- =====================================================
-- OHLCV TABLE
-- OHLCV 캔들 데이터 - TimescaleDB Hypertable
-- =====================================================

CREATE TABLE ohlcv (
    -- 복합 기본키: 심볼 + 타임프레임 + 시간
    symbol VARCHAR(50) NOT NULL,                    -- "AAPL", "005930.KS", "SPY", "BTC-USD" 등
    timeframe VARCHAR(10) NOT NULL,                 -- "1m", "5m", "15m", "30m", "1h", "1d", "1wk", "1mo"
    open_time TIMESTAMPTZ NOT NULL,                 -- 캔들 시작 시간 (UTC)

    -- OHLCV 데이터
    open DECIMAL(30, 15) NOT NULL,                  -- 시가 (Opening price)
    high DECIMAL(30, 15) NOT NULL,                  -- 고가 (Highest price)
    low DECIMAL(30, 15) NOT NULL,                   -- 저가 (Lowest price)
    close DECIMAL(30, 15) NOT NULL,                 -- 종가 (Closing price)
    volume DECIMAL(30, 15) NOT NULL,                -- 거래량 (Trading volume)

    -- 메타데이터
    close_time TIMESTAMPTZ,                         -- 캔들 종료 시간
    fetched_at TIMESTAMPTZ DEFAULT NOW(),           -- 데이터 가져온 시간

    PRIMARY KEY (symbol, timeframe, open_time)
);

-- TimescaleDB Hypertable 변환 (1개월 단위 청크)
SELECT create_hypertable('ohlcv', 'open_time',
    chunk_time_interval => INTERVAL '1 month',
    if_not_exists => TRUE
);

-- 심볼 + 타임프레임으로 최신 데이터 조회용 인덱스
CREATE INDEX idx_ohlcv_symbol_tf_time
    ON ohlcv(symbol, timeframe, open_time DESC);

-- 특정 심볼의 마지막 캐시 시간 조회용 인덱스
CREATE INDEX idx_ohlcv_last_fetch
    ON ohlcv(symbol, timeframe, fetched_at DESC);

-- 압축 정책 (30일 이상 데이터 압축)
ALTER TABLE ohlcv SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol, timeframe'
);
SELECT add_compression_policy('ohlcv', INTERVAL '30 days', if_not_exists => TRUE);

COMMENT ON TABLE ohlcv IS 'OHLCV(Open-High-Low-Close-Volume) 캔들 데이터. 다양한 데이터 소스(Yahoo Finance, KRX 등)에서 통합 관리. (TimescaleDB Hypertable)';
COMMENT ON COLUMN ohlcv.symbol IS '종목 심볼 (예: AAPL, 005930.KS, SPY, BTC-USD)';
COMMENT ON COLUMN ohlcv.timeframe IS '시간 간격: 1m, 5m, 15m, 30m, 1h, 1d, 1wk, 1mo';
COMMENT ON COLUMN ohlcv.open_time IS '캔들 시작 시간 (UTC)';
COMMENT ON COLUMN ohlcv.open IS '시가 (Opening price)';
COMMENT ON COLUMN ohlcv.high IS '고가 (Highest price)';
COMMENT ON COLUMN ohlcv.low IS '저가 (Lowest price)';
COMMENT ON COLUMN ohlcv.close IS '종가 (Closing price)';
COMMENT ON COLUMN ohlcv.volume IS '거래량 (Trading volume)';

-- =====================================================
-- OHLCV_METADATA TABLE
-- 심볼별 캐시 상태 메타데이터
-- =====================================================

CREATE TABLE ohlcv_metadata (
    symbol VARCHAR(50) NOT NULL,
    timeframe VARCHAR(10) NOT NULL,
    first_cached_time TIMESTAMPTZ,                  -- 가장 오래된 캐시 데이터 시간
    last_cached_time TIMESTAMPTZ,                   -- 가장 최근 캐시 데이터 시간
    last_updated_at TIMESTAMPTZ DEFAULT NOW(),      -- 마지막 업데이트 시간
    total_candles INT DEFAULT 0,                    -- 총 캔들 수

    PRIMARY KEY (symbol, timeframe)
);

COMMENT ON TABLE ohlcv_metadata IS '심볼/타임프레임별 OHLCV 데이터 메타정보. 증분 업데이트 및 캐시 상태 관리용.';

-- =====================================================
-- TRIGGERS
-- =====================================================

-- 메타데이터 자동 업데이트 트리거 함수
CREATE OR REPLACE FUNCTION update_ohlcv_metadata()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO ohlcv_metadata (symbol, timeframe, first_cached_time, last_cached_time, total_candles)
    VALUES (NEW.symbol, NEW.timeframe, NEW.open_time, NEW.open_time, 1)
    ON CONFLICT (symbol, timeframe) DO UPDATE SET
        first_cached_time = LEAST(ohlcv_metadata.first_cached_time, NEW.open_time),
        last_cached_time = GREATEST(ohlcv_metadata.last_cached_time, NEW.open_time),
        last_updated_at = NOW(),
        total_candles = (
            SELECT COUNT(*) FROM ohlcv
            WHERE symbol = NEW.symbol AND timeframe = NEW.timeframe
        );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 캔들 삽입 시 메타데이터 자동 업데이트
CREATE TRIGGER trigger_update_ohlcv_metadata
    AFTER INSERT ON ohlcv
    FOR EACH ROW
    EXECUTE FUNCTION update_ohlcv_metadata();

-- =====================================================
-- HELPER FUNCTIONS
-- =====================================================

-- 특정 심볼/타임프레임의 캐시 갭 확인 함수
CREATE OR REPLACE FUNCTION get_ohlcv_gaps(
    p_symbol VARCHAR(50),
    p_timeframe VARCHAR(10),
    p_start_time TIMESTAMPTZ,
    p_end_time TIMESTAMPTZ
)
RETURNS TABLE (
    gap_start TIMESTAMPTZ,
    gap_end TIMESTAMPTZ
) AS $$
BEGIN
    -- 간단한 갭 감지: 예상 간격보다 큰 공백이 있는 경우
    -- (상세 구현은 애플리케이션 레벨에서 처리)
    RETURN QUERY
    SELECT
        open_time AS gap_start,
        LEAD(open_time) OVER (ORDER BY open_time) AS gap_end
    FROM ohlcv
    WHERE symbol = p_symbol
      AND timeframe = p_timeframe
      AND open_time BETWEEN p_start_time AND p_end_time
    ORDER BY open_time;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_ohlcv_gaps(VARCHAR, VARCHAR, TIMESTAMPTZ, TIMESTAMPTZ) IS
'특정 심볼/타임프레임의 캐시 갭(빠진 구간) 확인';

-- 캐시 통계 조회 함수
CREATE OR REPLACE FUNCTION get_ohlcv_stats()
RETURNS TABLE (
    symbol VARCHAR(50),
    timeframe VARCHAR(10),
    first_time TIMESTAMPTZ,
    last_time TIMESTAMPTZ,
    candle_count BIGINT,
    last_updated TIMESTAMPTZ
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        m.symbol,
        m.timeframe,
        m.first_cached_time,
        m.last_cached_time,
        m.total_candles::BIGINT,
        m.last_updated_at
    FROM ohlcv_metadata m
    ORDER BY m.last_updated_at DESC;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_ohlcv_stats() IS
'전체 OHLCV 캐시 통계 조회 (심볼별 캔들 수, 시간 범위 등)';

-- =====================================================
-- MATERIALIZED VIEW: mv_latest_prices
-- 심볼별 최신 일봉 가격 (스크리닝 쿼리 최적화용)
-- =====================================================

-- 일봉 기준 심볼별 최신 가격 정보
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_latest_prices AS
SELECT DISTINCT ON (symbol)
    symbol,
    open_time,
    open,
    high,
    low,
    close,
    volume
FROM ohlcv
WHERE timeframe = '1d'
ORDER BY symbol, open_time DESC
WITH DATA;

-- 고유 인덱스 (REFRESH CONCURRENTLY 필수)
CREATE UNIQUE INDEX IF NOT EXISTS idx_mv_latest_prices_symbol
    ON mv_latest_prices(symbol);

-- 가격 조회용 인덱스
CREATE INDEX IF NOT EXISTS idx_mv_latest_prices_close
    ON mv_latest_prices(close);

-- 시간 조회용 인덱스
CREATE INDEX IF NOT EXISTS idx_mv_latest_prices_time
    ON mv_latest_prices(open_time DESC);

COMMENT ON MATERIALIZED VIEW mv_latest_prices IS
'심볼별 최신 일봉 가격. 스크리닝 쿼리 성능 최적화용. REFRESH MATERIALIZED VIEW CONCURRENTLY로 갱신.';

-- Materialized View 갱신 함수
CREATE OR REPLACE FUNCTION refresh_latest_prices()
RETURNS void AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY mv_latest_prices;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION refresh_latest_prices() IS
'mv_latest_prices 뷰 갱신. 새 데이터 입력 후 호출하거나 스케줄러로 주기적 실행.';

-- =====================================================
-- 사용 예시
-- =====================================================
--
-- 1. 최신 가격 조회 (스크리닝):
--    SELECT symbol, close as current_price FROM mv_latest_prices;
--
-- 2. 특정 심볼의 일봉 조회:
--    SELECT * FROM ohlcv WHERE symbol = 'AAPL' AND timeframe = '1d' ORDER BY open_time DESC LIMIT 100;
--
-- 3. 캐시 통계 확인:
--    SELECT * FROM get_ohlcv_stats();
--
-- 4. 최신 가격 뷰 갱신 (트레이딩 시간 종료 후):
--    SELECT refresh_latest_prices();
--
-- =====================================================

-- ============ 06_execution_tracking.sql ============

-- =====================================================
-- 06_execution_tracking.sql
-- 거래 체결 내역 캐시 시스템
-- =====================================================
--
-- 원본 마이그레이션: 011_execution_cache.sql
--
-- 포함 내용:
-- - execution_cache: 거래소 체결 내역 캐시 (거래소 중립)
-- - execution_cache_meta: 동기화 메타데이터 (마지막 동기화 정보)
-- - 증분 조회 지원: 마지막 저장일부터만 조회
--
-- 지원 거래소: KIS, Binance, Coinbase 등
--
-- =====================================================

-- =====================================================
-- EXECUTION_CACHE TABLE
-- 거래소 체결 내역 캐시
-- =====================================================

CREATE TABLE IF NOT EXISTS execution_cache (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- 계좌 식별
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,

    -- 거래소 식별
    exchange VARCHAR(50) NOT NULL,                  -- kis, binance, coinbase 등

    -- 체결 정보 (정규화)
    executed_at TIMESTAMPTZ NOT NULL,               -- 체결 일시

    -- 종목 정보
    symbol VARCHAR(50) NOT NULL,                    -- 거래소별 심볼 (005930, BTCUSDT)
    normalized_symbol VARCHAR(50),                  -- 정규화 심볼 (005930.KS, BTC/USDT)

    -- 체결 상세
    side VARCHAR(10) NOT NULL,                      -- buy, sell
    quantity DECIMAL(30, 15) NOT NULL,              -- 체결 수량
    price DECIMAL(30, 15) NOT NULL,                 -- 체결 단가
    amount DECIMAL(30, 15) NOT NULL,                -- 체결 금액 (quantity * price)

    -- 수수료
    fee DECIMAL(30, 15),                            -- 수수료
    fee_currency VARCHAR(20),                       -- 수수료 통화 (KRW, USDT 등)

    -- 주문/체결 ID
    order_id VARCHAR(100) NOT NULL,                 -- 거래소 주문 ID
    trade_id VARCHAR(100),                          -- 거래소 체결 ID

    -- 주문 유형
    order_type VARCHAR(20),                         -- market, limit, stop_limit 등

    -- 거래소별 원본 데이터
    raw_data JSONB,                                 -- 디버깅/확장용 (거래소 응답 원본)

    -- 시스템 필드
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_exec_cache_credential ON execution_cache(credential_id);
CREATE INDEX IF NOT EXISTS idx_exec_cache_exchange ON execution_cache(exchange);
CREATE INDEX IF NOT EXISTS idx_exec_cache_executed_at ON execution_cache(credential_id, executed_at DESC);
CREATE INDEX IF NOT EXISTS idx_exec_cache_symbol ON execution_cache(symbol);
CREATE INDEX IF NOT EXISTS idx_exec_cache_side ON execution_cache(side);

-- 중복 방지: 거래소 + 계좌 + 주문ID + 체결ID 조합 유일 (함수형 유니크 인덱스)
CREATE UNIQUE INDEX IF NOT EXISTS idx_exec_cache_unique
ON execution_cache(credential_id, exchange, order_id, COALESCE(trade_id, ''));

COMMENT ON TABLE execution_cache IS '체결 내역 캐시 - 거래소 중립적 증분 조회 지원';
COMMENT ON COLUMN execution_cache.exchange IS '거래소 식별자 (kis, binance, coinbase 등)';
COMMENT ON COLUMN execution_cache.normalized_symbol IS '정규화된 심볼 형식 (BTC/USDT, 005930.KS)';
COMMENT ON COLUMN execution_cache.raw_data IS '거래소 API 응답 원본 (디버깅 및 확장 필드 추출용)';

-- =====================================================
-- EXECUTION_CACHE_META TABLE
-- 체결 캐시 메타데이터 (동기화 상태 추적)
-- =====================================================

CREATE TABLE IF NOT EXISTS execution_cache_meta (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    credential_id UUID NOT NULL REFERENCES exchange_credentials(id) ON DELETE CASCADE,
    exchange VARCHAR(50) NOT NULL,                  -- 거래소별 메타데이터

    -- 동기화 범위
    earliest_date DATE,                             -- 가장 오래된 캐시 일자
    latest_date DATE,                               -- 가장 최근 캐시 일자 (다음 조회 시작점)

    -- 동기화 상태
    total_records INT DEFAULT 0,                    -- 총 레코드 수
    last_sync_at TIMESTAMPTZ,                       -- 마지막 동기화 시간
    last_sync_status VARCHAR(20),                   -- success, failed, partial
    last_sync_message TEXT,                         -- 상세 메시지 (에러 로그 등)

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- 계좌당 거래소별 하나의 메타데이터
    CONSTRAINT unique_cache_meta UNIQUE (credential_id, exchange)
);

COMMENT ON TABLE execution_cache_meta IS '체결 캐시 메타데이터 - 거래소별 마지막 동기화 정보';
COMMENT ON COLUMN execution_cache_meta.latest_date IS '가장 최근 캐시된 일자 - 다음 조회 시작점 (증분 조회용)';
COMMENT ON COLUMN execution_cache_meta.last_sync_status IS '마지막 동기화 상태: success, failed, partial';

-- =====================================================
-- 사용 예시
-- =====================================================
--
-- 1. 증분 조회 (마지막 동기화 이후 데이터만):
--    SELECT latest_date FROM execution_cache_meta
--    WHERE credential_id = ? AND exchange = 'kis';
--
-- 2. 특정 계좌의 최근 체결 조회:
--    SELECT * FROM execution_cache
--    WHERE credential_id = ?
--    ORDER BY executed_at DESC LIMIT 100;
--
-- 3. 종목별 체결 통계:
--    SELECT symbol, COUNT(*), SUM(amount)
--    FROM execution_cache
--    WHERE credential_id = ?
--    GROUP BY symbol;
--
-- =====================================================
