-- 11_fix_watchlist_schema.sql
-- watchlist 테이블 스키마 수정
--
-- 문제: 코드는 name, description, color, icon 컬럼을 사용하지만
--       현재 테이블은 symbol, market, display_name 구조임
--
-- 해결: watchlist를 그룹(폴더) 테이블로 변경
--       개별 심볼은 watchlist_item 테이블에 저장

-- ============================================================================
-- 1. 기존 데이터 백업 (필요시)
-- ============================================================================
CREATE TABLE IF NOT EXISTS watchlist_backup AS SELECT * FROM watchlist;

-- ============================================================================
-- 2. watchlist 테이블 재생성
-- ============================================================================

-- 기존 외래 키 제약 조건 제거
ALTER TABLE IF EXISTS watchlist_item DROP CONSTRAINT IF EXISTS watchlist_item_watchlist_id_fkey;

-- 기존 테이블 삭제
DROP TABLE IF EXISTS watchlist CASCADE;

-- 새 스키마로 생성 (그룹/폴더 테이블)
CREATE TABLE watchlist (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(100) NOT NULL,              -- 그룹 이름
    description TEXT,                         -- 그룹 설명
    color VARCHAR(20) DEFAULT '#3B82F6',     -- 테마 색상 (hex)
    icon VARCHAR(50) DEFAULT 'star',          -- 아이콘 이름
    sort_order INTEGER DEFAULT 0,             -- 정렬 순서
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- 인덱스
CREATE INDEX idx_watchlist_sort ON watchlist(sort_order);
CREATE UNIQUE INDEX unique_watchlist_name ON watchlist(name);

COMMENT ON TABLE watchlist IS '관심종목 그룹 (폴더)';
COMMENT ON COLUMN watchlist.name IS '그룹 이름 (예: 모멘텀 종목, 배당주)';
COMMENT ON COLUMN watchlist.color IS '테마 색상 (hex, 예: #3B82F6)';
COMMENT ON COLUMN watchlist.icon IS '아이콘 이름 (예: star, heart, bookmark)';

-- ============================================================================
-- 3. watchlist_item 테이블 확인 및 외래 키 복원
-- ============================================================================

-- watchlist_item이 없으면 생성
CREATE TABLE IF NOT EXISTS watchlist_item (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    watchlist_id UUID NOT NULL,               -- 소속 그룹
    symbol VARCHAR(50) NOT NULL,              -- 심볼 (예: 005930, AAPL)
    market VARCHAR(10) NOT NULL,              -- 시장 (KR, US, CRYPTO)
    note TEXT,                                -- 메모
    sort_order INTEGER DEFAULT 0,             -- 그룹 내 정렬
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT watchlist_item_watchlist_id_fkey
        FOREIGN KEY (watchlist_id) REFERENCES watchlist(id) ON DELETE CASCADE
);

-- 인덱스 (이미 존재하면 무시)
CREATE INDEX IF NOT EXISTS idx_watchlist_item_watchlist ON watchlist_item(watchlist_id);
CREATE UNIQUE INDEX IF NOT EXISTS unique_watchlist_item ON watchlist_item(watchlist_id, symbol, market);

COMMENT ON TABLE watchlist_item IS '관심종목 그룹 내 개별 심볼';

-- ============================================================================
-- 4. 기본 그룹 생성
-- ============================================================================
INSERT INTO watchlist (name, description, color, icon, sort_order)
VALUES
    ('관심종목', '기본 관심종목 그룹', '#3B82F6', 'star', 0),
    ('모멘텀', '모멘텀 전략 종목', '#10B981', 'trending-up', 1),
    ('배당주', '배당 수익 종목', '#F59E0B', 'dollar-sign', 2)
ON CONFLICT (name) DO NOTHING;

-- ============================================================================
-- 5. 백업 테이블에서 데이터 복원 (기존 심볼을 기본 그룹으로 이동)
-- ============================================================================
DO $$
DECLARE
    default_group_id UUID;
BEGIN
    -- 기본 그룹 ID 조회
    SELECT id INTO default_group_id FROM watchlist WHERE name = '관심종목' LIMIT 1;

    -- 백업에서 심볼 복원 (중복 제외)
    IF default_group_id IS NOT NULL AND EXISTS (SELECT 1 FROM watchlist_backup LIMIT 1) THEN
        INSERT INTO watchlist_item (watchlist_id, symbol, market, sort_order)
        SELECT default_group_id, symbol, market, sort_order
        FROM watchlist_backup
        WHERE symbol IS NOT NULL AND market IS NOT NULL
        ON CONFLICT (watchlist_id, symbol, market) DO NOTHING;
    END IF;
END $$;

-- 백업 테이블 삭제 (복원 완료 후)
DROP TABLE IF EXISTS watchlist_backup;
