-- 09_remove_legacy_tables.sql
-- 레거시 테이블 정리 (더 이상 사용되지 않는 테이블 제거)
--
-- 변경 이유:
-- 1. signals → signal_marker로 대체됨
-- 2. api_keys → exchange_credentials로 대체됨
-- 3. performance_snapshots, users → 코드에서 미사용
--
-- 보존하는 테이블:
-- - symbols: timescale.rs, positions.rs에서 사용
-- - app_settings: active_credential_id 설정 저장
-- - orders, trades, trade_ticks: 실거래 시스템에서 필요
-- - audit_logs, credential_access_logs: 향후 보안/감사 로그용

-- ============================================================================
-- 1. signals 테이블 제거 (signal_marker로 대체됨)
-- ============================================================================
DROP TABLE IF EXISTS signals CASCADE;

-- ============================================================================
-- 2. api_keys 테이블 제거 (exchange_credentials로 대체됨)
-- ============================================================================
DROP TABLE IF EXISTS api_keys CASCADE;

-- ============================================================================
-- 3. performance_snapshots 테이블 제거 (미사용)
-- ============================================================================
DROP TABLE IF EXISTS performance_snapshots CASCADE;

-- ============================================================================
-- 4. users 테이블 제거 (현재 인증 시스템 미사용)
-- ============================================================================
DROP TABLE IF EXISTS users CASCADE;

-- ============================================================================
-- 참고: 다음 테이블은 활발히 사용되므로 보존됨
-- ============================================================================
-- symbols: 거래소 심볼 관리 (timescale.rs)
-- app_settings: 애플리케이션 설정 (active_credential_id 등)
-- orders: 거래소 주문 관리 (실거래 시 필수)
-- trades: 체결 내역 관리 (실거래 시 필수)
-- trade_ticks: 실시간 틱 데이터 (고빈도 전략용)
-- audit_logs: 시스템 감사 로그 (규정 준수)
-- credential_access_logs: API 키 접근 추적 (보안)
