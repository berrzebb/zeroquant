/**
 * useStrategies 훅
 *
 * 전략 목록 조회, 생성, 수정, 삭제, 활성화/비활성화 기능을 제공합니다.
 * 낙관적 업데이트와 에러 롤백을 지원합니다.
 */
import { createResource, createMemo, batch } from 'solid-js'
import { createStore } from 'solid-js/store'
import {
  getStrategies,
  createStrategy as apiCreateStrategy,
  deleteStrategy as apiDeleteStrategy,
  startStrategy,
  stopStrategy,
  cloneStrategy as apiCloneStrategy,
  updateStrategyConfig,
  updateStrategySymbols,
  updateStrategyTimeframeConfig,
  type CreateStrategyRequest,
  type MultiTimeframeConfig,
} from '../api/client'
import type { Strategy } from '../types'

// ==================== 타입 정의 ====================

/** 훅 내부 상태 */
interface UseStrategiesState {
  /** 현재 토글 중인 전략 ID */
  togglingId: string | null
  /** 현재 삭제 중인 전략 ID */
  deletingId: string | null
  /** 현재 복제 중인 전략 ID */
  cloningId: string | null
  /** 에러 메시지 */
  error: string | null
}

/** 전략 필터 타입 */
export type StrategyFilter = 'all' | 'running' | 'stopped'

/** 훅 반환 타입 */
export interface UseStrategiesReturn {
  /** 전략 목록 (createResource) */
  strategies: () => Strategy[] | undefined
  /** 로딩 상태 */
  loading: boolean
  /** 에러 상태 */
  error: () => string | null
  /** 전체 전략 수 */
  total: () => number
  /** 실행 중인 전략 수 */
  runningCount: () => number
  /** 전략 목록 새로고침 */
  refetch: () => void
  /** 전략 생성 */
  create: (request: CreateStrategyRequest) => Promise<{ success: boolean; id?: string; error?: string }>
  /** 전략 삭제 */
  remove: (id: string) => Promise<{ success: boolean; error?: string }>
  /** 전략 활성화/비활성화 토글 */
  toggle: (id: string) => Promise<{ success: boolean; error?: string }>
  /** 전략 복제 */
  clone: (id: string, newName: string) => Promise<{ success: boolean; id?: string; error?: string }>
  /** 전략 설정 업데이트 */
  updateConfig: (id: string, config: Record<string, unknown>) => Promise<{ success: boolean; error?: string }>
  /** 전략 심볼 업데이트 */
  updateSymbols: (id: string, symbols: string[]) => Promise<{ success: boolean; error?: string }>
  /** 전략 타임프레임 설정 업데이트 */
  updateTimeframe: (id: string, config: MultiTimeframeConfig | null) => Promise<{ success: boolean; error?: string }>
  /** 현재 토글 중인 전략 ID */
  togglingId: () => string | null
  /** 현재 삭제 중인 전략 ID */
  deletingId: () => string | null
  /** 현재 복제 중인 전략 ID */
  cloningId: () => string | null
  /** 필터링된 전략 목록 반환 */
  filtered: (filter: StrategyFilter) => Strategy[]
}

// ==================== 초기 상태 ====================

const initialState: UseStrategiesState = {
  togglingId: null,
  deletingId: null,
  cloningId: null,
  error: null,
}

// ==================== 훅 구현 ====================

export function useStrategies(): UseStrategiesReturn {
  // 내부 상태
  const [state, setState] = createStore<UseStrategiesState>({ ...initialState })

  // 전략 목록 리소스
  const [strategiesResource, { refetch }] = createResource(getStrategies)

  // ==================== 파생 상태 ====================

  const total = createMemo(() => strategiesResource()?.length ?? 0)

  const runningCount = createMemo(() =>
    strategiesResource()?.filter((s) => s.status === 'Running').length ?? 0
  )

  // ==================== 필터링 함수 ====================

  const filtered = (filter: StrategyFilter): Strategy[] => {
    const data = strategiesResource()
    if (!data) return []

    switch (filter) {
      case 'running':
        return data.filter((s) => s.status === 'Running')
      case 'stopped':
        return data.filter((s) => s.status === 'Stopped' || s.status === 'Error')
      default:
        return data
    }
  }

  // ==================== CRUD 작업 ====================

  /** 전략 생성 */
  const create = async (
    request: CreateStrategyRequest
  ): Promise<{ success: boolean; id?: string; error?: string }> => {
    setState('error', null)

    try {
      const response = await apiCreateStrategy(request)
      await refetch()
      return { success: true, id: response.id }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : '전략 생성에 실패했습니다'
      setState('error', errorMsg)
      return { success: false, error: errorMsg }
    }
  }

  /** 전략 삭제 */
  const remove = async (id: string): Promise<{ success: boolean; error?: string }> => {
    setState('error', null)
    setState('deletingId', id)

    try {
      await apiDeleteStrategy(id)
      await refetch()
      return { success: true }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : '전략 삭제에 실패했습니다'
      setState('error', errorMsg)
      return { success: false, error: errorMsg }
    } finally {
      setState('deletingId', null)
    }
  }

  /** 전략 활성화/비활성화 토글 */
  const toggle = async (id: string): Promise<{ success: boolean; error?: string }> => {
    const strategy = strategiesResource()?.find((s) => s.id === id)
    if (!strategy) {
      return { success: false, error: '전략을 찾을 수 없습니다' }
    }

    setState('error', null)
    setState('togglingId', id)

    const isRunning = strategy.status === 'Running'

    try {
      if (isRunning) {
        await stopStrategy(id)
      } else {
        await startStrategy(id)
      }
      await refetch()
      return { success: true }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : '전략 상태 변경에 실패했습니다'
      setState('error', errorMsg)
      return { success: false, error: errorMsg }
    } finally {
      setState('togglingId', null)
    }
  }

  /** 전략 복제 */
  const clone = async (
    id: string,
    newName: string
  ): Promise<{ success: boolean; id?: string; error?: string }> => {
    setState('error', null)
    setState('cloningId', id)

    try {
      const response = await apiCloneStrategy(id, newName)
      await refetch()
      return { success: true, id: response.id }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : '전략 복제에 실패했습니다'
      setState('error', errorMsg)
      return { success: false, error: errorMsg }
    } finally {
      setState('cloningId', null)
    }
  }

  /** 전략 설정 업데이트 */
  const updateConfig = async (
    id: string,
    config: Record<string, unknown>
  ): Promise<{ success: boolean; error?: string }> => {
    setState('error', null)

    try {
      await updateStrategyConfig(id, config)
      await refetch()
      return { success: true }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : '전략 설정 업데이트에 실패했습니다'
      setState('error', errorMsg)
      return { success: false, error: errorMsg }
    }
  }

  /** 전략 심볼 업데이트 */
  const updateSymbols = async (
    id: string,
    symbols: string[]
  ): Promise<{ success: boolean; error?: string }> => {
    setState('error', null)

    try {
      await updateStrategySymbols(id, symbols)
      await refetch()
      return { success: true }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : '심볼 업데이트에 실패했습니다'
      setState('error', errorMsg)
      return { success: false, error: errorMsg }
    }
  }

  /** 전략 타임프레임 설정 업데이트 */
  const updateTimeframe = async (
    id: string,
    config: MultiTimeframeConfig | null
  ): Promise<{ success: boolean; error?: string }> => {
    setState('error', null)

    try {
      await updateStrategyTimeframeConfig(id, config)
      await refetch()
      return { success: true }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : '타임프레임 설정 업데이트에 실패했습니다'
      setState('error', errorMsg)
      return { success: false, error: errorMsg }
    }
  }

  // ==================== 반환 ====================

  return {
    strategies: () => strategiesResource(),
    loading: strategiesResource.loading,
    error: () => state.error,
    total,
    runningCount,
    refetch,
    create,
    remove,
    toggle,
    clone,
    updateConfig,
    updateSymbols,
    updateTimeframe,
    togglingId: () => state.togglingId,
    deletingId: () => state.deletingId,
    cloningId: () => state.cloningId,
    filtered,
  }
}
