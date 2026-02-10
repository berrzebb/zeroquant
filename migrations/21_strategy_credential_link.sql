-- 전략에 credential_id 추가 (전략-계정 연결)
-- 각 전략이 특정 거래소 계정에 연결되어 독립적으로 실행 가능
-- NULL 허용으로 기존 전략과 하위 호환성 유지

ALTER TABLE strategies
ADD COLUMN credential_id UUID REFERENCES exchange_credentials(id) ON DELETE SET NULL;

-- 인덱스 추가 (계정별 전략 조회 최적화)
CREATE INDEX idx_strategies_credential_id ON strategies(credential_id);

COMMENT ON COLUMN strategies.credential_id IS '전략이 연결된 거래소 계정 ID (NULL이면 기본 활성 계정 사용)';
