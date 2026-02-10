-- 08_remove_klines_table.sql
-- klines 테이블 제거 (ohlcv 테이블로 통합)
--
-- 변경 이유:
-- 1. klines 테이블(7,575행)과 ohlcv 테이블(6.8M행)이 중복 역할
-- 2. KlineRepository가 제거되어 klines 테이블이 더 이상 사용되지 않음
-- 3. OhlcvCache가 표준 OHLCV 데이터 저장소로 통합됨

-- klines 테이블 삭제
DROP TABLE IF EXISTS klines CASCADE;

-- klines 관련 인덱스가 있다면 함께 삭제됨 (CASCADE로 처리)
