/**
 * 전략 스키마 조회 훅
 *
 * 백엔드에서 전략 UI 스키마를 조회하고 캐싱합니다.
 * SDUIRenderer에서 사용됩니다.
 * ts-rs 자동 생성 타입 사용
 */
import { createSignal, createEffect, onCleanup } from 'solid-js';
import type {
  StrategyUISchema,
  SchemaFragment,
  FieldSchema,
} from '../types/generated/sdui';
import {
  getStrategySchema,
  getFragmentDetails,
} from '../api/schema';

/**
 * 렌더링용 섹션 정보 (Fragment + fields 결합)
 *
 * SDUISection 컴포넌트에서 사용합니다.
 */
export interface RenderableSection {
  /** 섹션 ID (Fragment ID 또는 'custom') */
  id: string;
  /** 섹션 이름 */
  name: string;
  /** 섹션 설명 */
  description?: string | null;
  /** 필수 여부 (접기 불가) */
  required: boolean;
  /** 접힘 가능 여부 */
  collapsible: boolean;
  /** 포함된 필드 목록 */
  fields: FieldSchema[];
  /** 표시 순서 */
  order: number;
}

// ==================== 캐시 ====================

/**
 * 스키마 캐시 (메모리 캐싱)
 *
 * 동일 세션 내에서 스키마 재조회 방지
 */
const schemaCache = new Map<string, StrategyUISchema>();

/**
 * Fragment 캐시
 */
const fragmentCache = new Map<string, SchemaFragment>();

/**
 * 캐시 TTL (5분)
 */
const CACHE_TTL = 5 * 60 * 1000;

/**
 * 캐시 타임스탬프
 */
const cacheTimestamps = new Map<string, number>();

/**
 * 캐시 유효성 확인
 */
function isCacheValid(key: string): boolean {
  const timestamp = cacheTimestamps.get(key);
  if (!timestamp) return false;
  return Date.now() - timestamp < CACHE_TTL;
}

/**
 * 캐시 설정
 */
function setCache<T>(
  cache: Map<string, T>,
  key: string,
  value: T
): void {
  cache.set(key, value);
  cacheTimestamps.set(key, Date.now());
}

// ==================== 훅 타입 ====================

/**
 * useStrategySchema 훅 반환 타입
 */
export interface UseStrategySchemaReturn {
  /** 스키마 (로딩 중이면 null) */
  schema: () => StrategyUISchema | null;

  /** 렌더링용 섹션 목록 (Fragment + custom_fields 결합) */
  sections: () => RenderableSection[];

  /** 로딩 상태 */
  loading: () => boolean;

  /** 에러 메시지 */
  error: () => string | null;

  /** 강제 재조회 */
  refetch: () => Promise<void>;

  /** 캐시 무효화 */
  invalidateCache: () => void;
}

// ==================== 훅 구현 ====================

/**
 * 전략 스키마 조회 훅
 *
 * @param strategyId 전략 ID (반응형 가능)
 * @returns 스키마, 섹션, 로딩/에러 상태, refetch 함수
 *
 * @example
 * ```tsx
 * const { schema, sections, loading, error } = useStrategySchema(strategyId);
 *
 * return (
 *   <Show when={!loading()} fallback={<Spinner />}>
 *     <Show when={schema()} fallback={<ErrorState message={error()} />}>
 *       <For each={sections()}>
 *         {(section) => <SDUISection section={section} />}
 *       </For>
 *     </Show>
 *   </Show>
 * );
 * ```
 */
export function useStrategySchema(
  strategyId: () => string | undefined
): UseStrategySchemaReturn {
  // 상태
  const [schema, setSchema] = createSignal<StrategyUISchema | null>(null);
  const [sections, setSections] = createSignal<RenderableSection[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  // 현재 요청 취소 플래그
  let aborted = false;

  /**
   * 스키마 조회 및 섹션 빌드
   */
  async function fetchSchema(): Promise<void> {
    const id = strategyId();
    if (!id) {
      setSchema(null);
      setSections([]);
      setLoading(false);
      return;
    }

    // 캐시 확인
    if (schemaCache.has(id) && isCacheValid(id)) {
      const cachedSchema = schemaCache.get(id)!;
      setSchema(cachedSchema);
      await buildSections(cachedSchema);
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    aborted = false;

    try {
      // 스키마 조회
      const fetchedSchema = await getStrategySchema(id);

      if (aborted) return;

      // 캐시 저장
      setCache(schemaCache, id, fetchedSchema);
      setSchema(fetchedSchema);

      // 섹션 빌드
      await buildSections(fetchedSchema);
    } catch (err) {
      if (aborted) return;

      const message = err instanceof Error
        ? err.message
        : '스키마를 불러오는데 실패했습니다.';
      setError(message);
      setSchema(null);
      setSections([]);
    } finally {
      if (!aborted) {
        setLoading(false);
      }
    }
  }

  /**
   * 렌더링용 섹션 빌드
   *
   * Fragment 참조를 실제 Fragment 데이터로 변환하고,
   * custom_fields를 별도 섹션으로 추가합니다.
   */
  async function buildSections(schema: StrategyUISchema): Promise<void> {
    const resultSections: RenderableSection[] = [];

    // 1. Fragment 섹션 빌드
    if (schema.fragments.length > 0) {
      const fragmentIds = schema.fragments.map((ref) => ref.id);

      // 캐시에 없는 Fragment만 조회
      const uncachedIds = fragmentIds.filter(
        (id) => !fragmentCache.has(id) || !isCacheValid(`fragment:${id}`)
      );

      if (uncachedIds.length > 0) {
        const fetchedFragments = await getFragmentDetails(uncachedIds);
        for (const [id, fragment] of fetchedFragments) {
          setCache(fragmentCache, id, fragment);
          cacheTimestamps.set(`fragment:${id}`, Date.now());
        }
      }

      // 섹션 생성
      for (let i = 0; i < schema.fragments.length; i++) {
        const ref = schema.fragments[i];
        const fragment = fragmentCache.get(ref.id);

        if (fragment) {
          resultSections.push({
            id: ref.id,
            name: fragment.name,
            description: fragment.description,
            required: ref.required,
            collapsible: !ref.required,
            fields: fragment.fields,
            order: fragment.order ?? i,
          });
        }
      }
    }

    // 2. Custom fields를 section별로 그룹화
    if (schema.custom_fields.length > 0) {
      // section별로 필드 그룹화
      const sectionGroups = new Map<string, FieldSchema[]>();

      for (const field of schema.custom_fields) {
        const sectionId = field.section ?? 'custom';
        if (!sectionGroups.has(sectionId)) {
          sectionGroups.set(sectionId, []);
        }
        sectionGroups.get(sectionId)!.push(field);
      }

      // 섹션 이름 매핑
      const sectionNames: Record<string, string> = {
        asset: '자산 선택',
        indicator: '기술적 지표',
        filter: '필터 조건',
        timing: '타이밍 설정',
        sizing: '포지션 사이징',
        custom: `${schema.name} 설정`,
      };

      // 섹션 순서
      const sectionOrder: Record<string, number> = {
        asset: 0,
        indicator: 1,
        filter: 2,
        timing: 3,
        sizing: 4,
        custom: 99,
      };

      // 각 그룹을 섹션으로 변환
      for (const [sectionId, fields] of sectionGroups.entries()) {
        // asset, indicator는 핵심 설정이므로 기본 펼침
        const isCoreSetting = sectionId === 'asset' || sectionId === 'indicator';
        resultSections.push({
          id: sectionId,
          name: sectionNames[sectionId] ?? sectionId,
          description: null,
          required: isCoreSetting,  // 핵심 설정만 필수 (기본 펼침)
          collapsible: true,        // 모든 섹션 접힘/펼침 가능
          fields,
          order: resultSections.length + (sectionOrder[sectionId] ?? 50),
        });
      }
    }

    // 정렬 후 설정
    resultSections.sort((a, b) => a.order - b.order);
    setSections(resultSections);
  }

  /**
   * 강제 재조회
   */
  async function refetch(): Promise<void> {
    const id = strategyId();
    if (id) {
      // 캐시 무효화
      schemaCache.delete(id);
      cacheTimestamps.delete(id);
    }
    await fetchSchema();
  }

  /**
   * 캐시 무효화
   */
  function invalidateCache(): void {
    const id = strategyId();
    if (id) {
      schemaCache.delete(id);
      cacheTimestamps.delete(id);
    }
  }

  // strategyId 변경 시 자동 조회
  createEffect(() => {
    const id = strategyId();
    if (id !== undefined) {
      fetchSchema();
    }
  });

  // 컴포넌트 언마운트 시 요청 취소
  onCleanup(() => {
    aborted = true;
  });

  return {
    schema,
    sections,
    loading,
    error,
    refetch,
    invalidateCache,
  };
}

// ==================== 유틸리티 함수 ====================

/**
 * 모든 필드에 기본값 적용
 *
 * @param sections 섹션 목록
 * @param defaults 기본값 맵
 * @returns 기본값이 적용된 값 맵
 */
export function applyDefaults(
  sections: RenderableSection[],
  defaults: Record<string, unknown>
): Record<string, unknown> {
  const values: Record<string, unknown> = { ...defaults };

  // 섹션의 모든 필드 순회
  for (const section of sections) {
    for (const field of section.fields) {
      // 값이 없고 기본값이 있으면 적용
      if (values[field.name] === undefined && field.default !== undefined) {
        values[field.name] = field.default;
      }
    }
  }

  return values;
}

/**
 * 필드명으로 필드 스키마 찾기
 *
 * @param sections 섹션 목록
 * @param fieldName 필드명
 * @returns 필드 스키마 또는 undefined
 */
export function findField(
  sections: RenderableSection[],
  fieldName: string
): FieldSchema | undefined {
  for (const section of sections) {
    const field = section.fields.find((f) => f.name === fieldName);
    if (field) return field;
  }
  return undefined;
}

/**
 * 캐시 전체 초기화 (디버깅/테스트용)
 */
export function clearSchemaCache(): void {
  schemaCache.clear();
  fragmentCache.clear();
  cacheTimestamps.clear();
}

export default useStrategySchema;
