-- 12_sync_checkpoint.sql
-- 워크플로우 체크포인트 테이블 생성
--
-- 용도:
-- - trader-collector의 장시간 배치 작업 진행 상태 추적
-- - 중단된 작업 재개 지점 저장
-- - 워크플로우 상태 모니터링
--
-- 사용처: crates/trader-collector/src/modules/checkpoint.rs

-- ============================================================================
-- 1. sync_checkpoint 테이블 생성
-- ============================================================================

CREATE TABLE IF NOT EXISTS sync_checkpoint (
    workflow_name VARCHAR(100) PRIMARY KEY,          -- 워크플로우 이름 (e.g., 'naver_fundamental', 'ohlcv_collect')
    last_ticker VARCHAR(50),                         -- 마지막 처리된 티커 (재개 지점)
    last_processed_at TIMESTAMPTZ,                   -- 마지막 처리 시간
    total_processed INTEGER DEFAULT 0,               -- 총 처리된 항목 수
    status VARCHAR(20) DEFAULT 'idle',               -- 상태: running, interrupted, completed, idle
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_sync_checkpoint_status ON sync_checkpoint(status);
CREATE INDEX IF NOT EXISTS idx_sync_checkpoint_updated ON sync_checkpoint(updated_at);

-- 주석
COMMENT ON TABLE sync_checkpoint IS '워크플로우 체크포인트 (배치 작업 중단/재개 지원)';
COMMENT ON COLUMN sync_checkpoint.workflow_name IS '워크플로우 고유 식별자 (e.g., naver_fundamental, ohlcv_collect, indicator_sync)';
COMMENT ON COLUMN sync_checkpoint.last_ticker IS '마지막 처리된 티커 (중단 시 재개 지점)';
COMMENT ON COLUMN sync_checkpoint.status IS '상태: running(실행중), interrupted(중단됨), completed(완료), idle(유휴)';
