/**
 * SDUI 스키마 API 함수
 *
 * 전략 스키마 및 Fragment 조회 API를 호출합니다.
 * ts-rs 자동 생성 타입 사용
 */
import api from './client';
import type {
  StrategyUISchema,
  SchemaFragment,
} from '../types/generated/sdui';
import { createLogger } from '../utils/logger';

const { warn } = createLogger('Schema');

/**
 * Fragment 목록 응답 타입
 */
export interface GetFragmentsResponse {
  fragments: SchemaFragment[];
  total: number;
}

/**
 * 전략 스키마 조회
 *
 * @param strategyId 전략 ID (예: "grid", "rsi")
 * @returns 전략 UI 스키마 (ts-rs 자동 생성 타입)
 *
 * @example
 * ```typescript
 * const schema = await getStrategySchema('grid');
 * console.log(schema.name); // "그리드 전략"
 * console.log(schema.fragments); // [{ id: 'base_config', required: true }, ...]
 * ```
 */
export async function getStrategySchema(
  strategyId: string
): Promise<StrategyUISchema> {
  const response = await api.get(`/strategies/${strategyId}/schema`);
  return response.data;
}

/**
 * Fragment 목록 조회
 *
 * @param category 카테고리 필터 (선택적)
 * @returns Fragment 목록
 *
 * @example
 * ```typescript
 * // 모든 Fragment 조회
 * const all = await getFragments();
 *
 * // 특정 카테고리 Fragment 조회
 * const common = await getFragments('common');
 * ```
 */
export async function getFragments(
  category?: string
): Promise<GetFragmentsResponse> {
  const url = category
    ? `/schema/fragments/${category}`
    : '/schema/fragments';
  const response = await api.get(url);
  return response.data;
}

/**
 * Fragment 상세 조회
 *
 * @param fragmentId Fragment ID (예: "base_config")
 * @returns Fragment 상세 정보
 */
export async function getFragmentDetail(
  fragmentId: string
): Promise<SchemaFragment> {
  const response = await api.get(`/schema/fragments/${fragmentId}/detail`);
  return response.data;
}

/**
 * 여러 Fragment 상세 조회 (병렬)
 *
 * @param fragmentIds Fragment ID 배열
 * @returns Fragment 상세 정보 맵
 */
export async function getFragmentDetails(
  fragmentIds: string[]
): Promise<Map<string, SchemaFragment>> {
  const results = await Promise.all(
    fragmentIds.map(async (id) => {
      try {
        const fragment = await getFragmentDetail(id);
        return [id, fragment] as const;
      } catch {
        // Fragment를 찾을 수 없는 경우 무시
        warn(`Fragment not found: ${id}`);
        return null;
      }
    })
  );

  const map = new Map<string, SchemaFragment>();
  for (const result of results) {
    if (result) {
      map.set(result[0], result[1]);
    }
  }
  return map;
}
