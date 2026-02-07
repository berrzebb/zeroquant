/**
 * useScreening 훅
 *
 * 스크리닝 기능을 제공합니다.
 * - 필터 상태 관리
 * - 스크리닝 실행
 * - 프리셋 CRUD
 */
import { createResource, createMemo, batch } from 'solid-js'
import { createStore } from 'solid-js/store'
import {
  runScreening,
  getScreeningPresets,
  getScreeningPresetsDetail,
  saveScreeningPreset,
  deleteScreeningPreset,
  type ScreeningRequest,
  type ScreeningResponse,
  type ScreeningResultDto,
  type PresetsListResponse,
  type PresetsDetailListResponse,
  type ScreeningPresetDetail,
} from '../api/client'

// ==================== 타입 정의 ====================

/** 스크리닝 필터 상태 */
export interface ScreeningFilterState {
  market: string
  exchange: string
  minScore: number
  maxScore: number
  routeState: string[]
  sectors: string[]
  rsiMin: number
  rsiMax: number
  sortField: string
  sortOrder: 'asc' | 'desc'
}

/** 훅 내부 상태 */
interface UseScreeningState {
  /** 현재 필터 */
  filter: ScreeningFilterState
  /** 검색 중 여부 */
  isSearching: boolean
  /** 에러 메시지 */
  error: string | null
  /** 마지막 검색 결과 */
  lastResults: ScreeningResponse | null
}

/** 훅 반환 타입 */
export interface UseScreeningReturn {
  // ==================== 데이터 ====================
  /** 스크리닝 결과 */
  results: () => ScreeningResultDto[]
  /** 총 결과 수 */
  total: () => number
  /** 필터 요약 */
  filterSummary: () => string
  /** 프리셋 목록 */
  presets: () => PresetsDetailListResponse | undefined
  /** 프리셋 목록 (간단) */
  presetList: () => PresetsListResponse | undefined

  // ==================== 상태 ====================
  /** 검색 중 여부 */
  isSearching: () => boolean
  /** 프리셋 로딩 */
  presetsLoading: boolean
  /** 에러 메시지 */
  error: () => string | null

  // ==================== 필터 ====================
  /** 현재 필터 상태 */
  filter: () => ScreeningFilterState
  /** 필터 업데이트 */
  setFilter: <K extends keyof ScreeningFilterState>(key: K, value: ScreeningFilterState[K]) => void
  /** 필터 일괄 업데이트 */
  setFilters: (updates: Partial<ScreeningFilterState>) => void
  /** 필터 초기화 */
  resetFilter: () => void

  // ==================== 액션 ====================
  /** 스크리닝 실행 */
  search: () => Promise<{ success: boolean; error?: string }>
  /** 프리셋 저장 */
  savePreset: (name: string) => Promise<{ success: boolean; id?: string; error?: string }>
  /** 프리셋 삭제 */
  deletePreset: (id: string) => Promise<{ success: boolean; error?: string }>
  /** 프리셋 로드 (필터에 적용) */
  loadPreset: (preset: ScreeningPresetDetail) => void
  /** 프리셋 목록 새로고침 */
  refreshPresets: () => void
}

// ==================== 초기 상태 ====================

const getDefaultFilter = (): ScreeningFilterState => ({
  market: '',
  exchange: '',
  minScore: 0,
  maxScore: 100,
  routeState: [],
  sectors: [],
  rsiMin: 0,
  rsiMax: 100,
  sortField: 'total',
  sortOrder: 'desc',
})

const initialState: UseScreeningState = {
  filter: getDefaultFilter(),
  isSearching: false,
  error: null,
  lastResults: null,
}

// ==================== 훅 구현 ====================

export function useScreening(): UseScreeningReturn {
  // 내부 상태
  const [state, setState] = createStore<UseScreeningState>({ ...initialState })

  // 프리셋 목록 리소스
  const [presetsResource, { refetch: refetchPresets }] = createResource(
    getScreeningPresetsDetail
  )

  const [presetListResource] = createResource(getScreeningPresets)

  // ==================== 필터 관리 ====================

  const setFilter = <K extends keyof ScreeningFilterState>(
    key: K,
    value: ScreeningFilterState[K]
  ) => {
    setState('filter', key, value)
  }

  const setFilters = (updates: Partial<ScreeningFilterState>) => {
    batch(() => {
      Object.entries(updates).forEach(([key, value]) => {
        setState('filter', key as keyof ScreeningFilterState, value as never)
      })
    })
  }

  const resetFilter = () => {
    setState('filter', getDefaultFilter())
    setState('lastResults', null)
  }

  // ==================== 스크리닝 실행 ====================

  const search = async (): Promise<{ success: boolean; error?: string }> => {
    setState('isSearching', true)
    setState('error', null)

    try {
      const request: ScreeningRequest = {
        market: state.filter.market || undefined,
        exchange: state.filter.exchange || undefined,
        min_score: state.filter.minScore,
        max_score: state.filter.maxScore,
        route_states: state.filter.routeState.length > 0 ? state.filter.routeState : undefined,
        sectors: state.filter.sectors.length > 0 ? state.filter.sectors : undefined,
        rsi_min: state.filter.rsiMin,
        rsi_max: state.filter.rsiMax,
        sort_by: state.filter.sortField,
        sort_order: state.filter.sortOrder,
      }

      const response = await runScreening(request)
      setState('lastResults', response)
      return { success: true }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : '스크리닝 실행에 실패했습니다'
      setState('error', errorMsg)
      return { success: false, error: errorMsg }
    } finally {
      setState('isSearching', false)
    }
  }

  // ==================== 프리셋 관리 ====================

  const savePreset = async (
    name: string
  ): Promise<{ success: boolean; id?: string; error?: string }> => {
    setState('error', null)

    try {
      const response = await saveScreeningPreset({
        name,
        filters: {
          market: state.filter.market,
          exchange: state.filter.exchange,
          minScore: state.filter.minScore,
          maxScore: state.filter.maxScore,
          routeState: state.filter.routeState,
          sectors: state.filter.sectors,
          rsiMin: state.filter.rsiMin,
          rsiMax: state.filter.rsiMax,
          sortField: state.filter.sortField,
          sortOrder: state.filter.sortOrder,
        },
      })
      await refetchPresets()
      return { success: true, id: response.id }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : '프리셋 저장에 실패했습니다'
      setState('error', errorMsg)
      return { success: false, error: errorMsg }
    }
  }

  const deletePreset = async (id: string): Promise<{ success: boolean; error?: string }> => {
    setState('error', null)

    try {
      await deleteScreeningPreset(id)
      await refetchPresets()
      return { success: true }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : '프리셋 삭제에 실패했습니다'
      setState('error', errorMsg)
      return { success: false, error: errorMsg }
    }
  }

  const loadPreset = (preset: ScreeningPresetDetail) => {
    const filters = preset.filters
    if (filters) {
      setFilters({
        market: filters.market ?? '',
        exchange: filters.exchange ?? '',
        minScore: filters.minScore ?? 0,
        maxScore: filters.maxScore ?? 100,
        routeState: filters.routeState ?? [],
        sectors: filters.sectors ?? [],
        rsiMin: filters.rsiMin ?? 0,
        rsiMax: filters.rsiMax ?? 100,
        sortField: filters.sortField ?? 'total',
        sortOrder: (filters.sortOrder as 'asc' | 'desc') ?? 'desc',
      })
    }
  }

  // ==================== 파생 데이터 ====================

  const results = createMemo(() => state.lastResults?.results ?? [])
  const total = createMemo(() => state.lastResults?.total ?? 0)
  const filterSummary = createMemo(() => state.lastResults?.filter_summary ?? '')

  // ==================== 반환 ====================

  return {
    // 데이터
    results,
    total,
    filterSummary,
    presets: () => presetsResource(),
    presetList: () => presetListResource(),

    // 상태
    isSearching: () => state.isSearching,
    presetsLoading: presetsResource.loading,
    error: () => state.error,

    // 필터
    filter: () => state.filter,
    setFilter,
    setFilters,
    resetFilter,

    // 액션
    search,
    savePreset,
    deletePreset,
    loadPreset,
    refreshPresets: refetchPresets,
  }
}
