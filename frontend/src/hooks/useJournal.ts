/**
 * useJournal 훅
 *
 * 매매일지 데이터 조회 기능을 제공합니다.
 * - 포지션 목록
 * - 체결 내역 (필터링/페이지네이션)
 * - 손익 통계 (일별/주별/월별/연도별/누적)
 * - 투자 인사이트
 */
import { createResource, createMemo } from 'solid-js'
import { createStore } from 'solid-js/store'
import {
  getJournalPositions,
  getJournalExecutions,
  getJournalPnLSummary,
  getJournalDailyPnL,
  getJournalWeeklyPnL,
  getJournalMonthlyPnL,
  getJournalYearlyPnL,
  getJournalCumulativePnL,
  getJournalInsights,
  getJournalStrategyPerformance,
  type JournalPositionsResponse,
  type JournalExecutionsResponse,
  type JournalPnLSummary,
  type DailyPnLResponse,
  type WeeklyPnLResponse,
  type MonthlyPnLResponse,
  type YearlyPnLResponse,
  type CumulativePnLResponse,
  type TradingInsightsResponse,
  type StrategyPerformanceResponse,
  type ExecutionFilter,
} from '../api/client'

// ==================== 타입 정의 ====================

/** 필터 상태 */
export interface JournalFilterState {
  /** 시작일 */
  startDate: string
  /** 종료일 */
  endDate: string
  /** 종목 필터 */
  symbol: string
  /** 매매 구분 (buy/sell/all) */
  side: 'buy' | 'sell' | 'all'
  /** 페이지 번호 (1부터 시작) */
  page: number
  /** 페이지 크기 */
  pageSize: number
}

/** 훅 내부 상태 */
interface UseJournalState {
  /** 현재 필터 */
  filter: JournalFilterState
  /** 에러 메시지 */
  error: string | null
  /** 새로고침 중 여부 */
  isRefreshing: boolean
}

/** 훅 반환 타입 */
export interface UseJournalReturn {
  // ==================== 데이터 ====================
  /** 포지션 목록 */
  positions: () => JournalPositionsResponse | undefined
  /** 체결 내역 */
  executions: () => JournalExecutionsResponse | undefined
  /** PnL 요약 */
  pnlSummary: () => JournalPnLSummary | undefined
  /** 일별 손익 */
  dailyPnL: () => DailyPnLResponse | undefined
  /** 주별 손익 */
  weeklyPnL: () => WeeklyPnLResponse | undefined
  /** 월별 손익 */
  monthlyPnL: () => MonthlyPnLResponse | undefined
  /** 연도별 손익 */
  yearlyPnL: () => YearlyPnLResponse | undefined
  /** 누적 손익 */
  cumulativePnL: () => CumulativePnLResponse | undefined
  /** 투자 인사이트 */
  insights: () => TradingInsightsResponse | undefined
  /** 전략별 성과 */
  strategyPerformance: () => StrategyPerformanceResponse | undefined

  // ==================== 로딩 상태 ====================
  /** 포지션 로딩 */
  positionsLoading: boolean
  /** 체결 내역 로딩 */
  executionsLoading: boolean
  /** PnL 요약 로딩 */
  pnlLoading: boolean
  /** 인사이트 로딩 */
  insightsLoading: boolean
  /** 전체 새로고침 중 여부 */
  isRefreshing: () => boolean

  // ==================== 필터 ====================
  /** 현재 필터 상태 */
  filter: () => JournalFilterState
  /** 필터 업데이트 */
  setFilter: <K extends keyof JournalFilterState>(key: K, value: JournalFilterState[K]) => void
  /** 필터 초기화 */
  resetFilter: () => void

  // ==================== 에러 ====================
  /** 에러 메시지 */
  error: () => string | null

  // ==================== 액션 ====================
  /** 전체 데이터 새로고침 */
  refresh: () => Promise<void>
  /** 체결 내역만 새로고침 */
  refreshExecutions: () => void
  /** 포지션만 새로고침 */
  refreshPositions: () => void
  /** 손익 데이터 새로고침 */
  refreshPnL: () => void

  // ==================== 파생 데이터 ====================
  /** 총 포지션 수 */
  totalPositions: () => number
  /** 총 체결 수 */
  totalExecutions: () => number
  /** 총 손익 */
  totalPnL: () => number
  /** 승률 */
  winRate: () => number
}

// ==================== 초기 상태 ====================

const getDefaultFilter = (): JournalFilterState => {
  const today = new Date()
  const oneMonthAgo = new Date(today)
  oneMonthAgo.setMonth(oneMonthAgo.getMonth() - 1)

  return {
    startDate: oneMonthAgo.toISOString().split('T')[0],
    endDate: today.toISOString().split('T')[0],
    symbol: '',
    side: 'all',
    page: 1,
    pageSize: 50,
  }
}

const initialState: UseJournalState = {
  filter: getDefaultFilter(),
  error: null,
  isRefreshing: false,
}

// ==================== 훅 구현 ====================

export function useJournal(): UseJournalReturn {
  // 내부 상태
  const [state, setState] = createStore<UseJournalState>({ ...initialState })

  // ==================== 리소스 정의 ====================

  // 포지션 목록
  const [positionsResource, { refetch: refetchPositions }] = createResource(
    getJournalPositions
  )

  // 체결 내역 (필터 의존)
  const [executionsResource, { refetch: refetchExecutions }] = createResource(
    () => state.filter,
    async (filter) => {
      const apiFilter: ExecutionFilter = {}
      if (filter.startDate) apiFilter.start_date = filter.startDate
      if (filter.endDate) apiFilter.end_date = filter.endDate
      if (filter.symbol) apiFilter.symbol = filter.symbol
      if (filter.side !== 'all') apiFilter.side = filter.side
      apiFilter.page = filter.page
      apiFilter.page_size = filter.pageSize
      return getJournalExecutions(apiFilter)
    }
  )

  // PnL 요약
  const [pnlSummaryResource, { refetch: refetchPnLSummary }] = createResource(
    getJournalPnLSummary
  )

  // 일별 손익
  const [dailyPnLResource, { refetch: refetchDailyPnL }] = createResource(
    () => ({ start: state.filter.startDate, end: state.filter.endDate }),
    async ({ start, end }) => getJournalDailyPnL(start, end)
  )

  // 주별 손익
  const [weeklyPnLResource, { refetch: refetchWeeklyPnL }] = createResource(
    getJournalWeeklyPnL
  )

  // 월별 손익
  const [monthlyPnLResource, { refetch: refetchMonthlyPnL }] = createResource(
    getJournalMonthlyPnL
  )

  // 연도별 손익
  const [yearlyPnLResource, { refetch: refetchYearlyPnL }] = createResource(
    getJournalYearlyPnL
  )

  // 누적 손익
  const [cumulativePnLResource, { refetch: refetchCumulativePnL }] = createResource(
    getJournalCumulativePnL
  )

  // 투자 인사이트
  const [insightsResource, { refetch: refetchInsights }] = createResource(
    getJournalInsights
  )

  // 전략별 성과
  const [strategyPerformanceResource, { refetch: refetchStrategyPerformance }] = createResource(
    getJournalStrategyPerformance
  )

  // ==================== 필터 관리 ====================

  const setFilter = <K extends keyof JournalFilterState>(
    key: K,
    value: JournalFilterState[K]
  ) => {
    // 필터 변경 시 페이지를 1로 리셋 (페이지 자체 변경은 제외)
    if (key !== 'page') {
      setState('filter', 'page', 1)
    }
    setState('filter', key, value)
  }

  const resetFilter = () => {
    setState('filter', getDefaultFilter())
  }

  // ==================== 새로고침 ====================

  const refresh = async () => {
    setState('isRefreshing', true)
    setState('error', null)

    try {
      await Promise.all([
        refetchPositions(),
        refetchExecutions(),
        refetchPnLSummary(),
        refetchDailyPnL(),
        refetchWeeklyPnL(),
        refetchMonthlyPnL(),
        refetchYearlyPnL(),
        refetchCumulativePnL(),
        refetchInsights(),
        refetchStrategyPerformance(),
      ])
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : '데이터 새로고침에 실패했습니다'
      setState('error', errorMsg)
    } finally {
      setState('isRefreshing', false)
    }
  }

  const refreshPnL = () => {
    refetchPnLSummary()
    refetchDailyPnL()
    refetchWeeklyPnL()
    refetchMonthlyPnL()
    refetchYearlyPnL()
    refetchCumulativePnL()
  }

  // ==================== 파생 데이터 ====================

  const totalPositions = createMemo(() =>
    positionsResource()?.positions?.length ?? 0
  )

  const totalExecutions = createMemo(() =>
    executionsResource()?.total ?? 0
  )

  const totalPnL = createMemo(() => {
    const summary = pnlSummaryResource()
    if (!summary) return 0
    return summary.totalRealizedPnL ?? 0
  })

  const winRate = createMemo(() => {
    const summary = pnlSummaryResource()
    if (!summary || !summary.totalTrades || summary.totalTrades === 0) return 0
    return (summary.winningTrades / summary.totalTrades) * 100
  })

  // ==================== 반환 ====================

  return {
    // 데이터
    positions: () => positionsResource(),
    executions: () => executionsResource(),
    pnlSummary: () => pnlSummaryResource(),
    dailyPnL: () => dailyPnLResource(),
    weeklyPnL: () => weeklyPnLResource(),
    monthlyPnL: () => monthlyPnLResource(),
    yearlyPnL: () => yearlyPnLResource(),
    cumulativePnL: () => cumulativePnLResource(),
    insights: () => insightsResource(),
    strategyPerformance: () => strategyPerformanceResource(),

    // 로딩 상태
    positionsLoading: positionsResource.loading,
    executionsLoading: executionsResource.loading,
    pnlLoading: pnlSummaryResource.loading,
    insightsLoading: insightsResource.loading,
    isRefreshing: () => state.isRefreshing,

    // 필터
    filter: () => state.filter,
    setFilter,
    resetFilter,

    // 에러
    error: () => state.error,

    // 액션
    refresh,
    refreshExecutions: refetchExecutions,
    refreshPositions: refetchPositions,
    refreshPnL,

    // 파생 데이터
    totalPositions,
    totalExecutions,
    totalPnL,
    winRate,
  }
}
