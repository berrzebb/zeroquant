/**
 * useMarketSentiment 훅
 *
 * 시장 심리 지표 데이터를 제공합니다.
 * - 공포탐욕 지수 (Fear & Greed Index)
 * - 시장 온도 (Market Breadth)
 * - 섹터별 모멘텀
 * - 자동 갱신 (5분 간격)
 */
import { createResource, createMemo, onCleanup, createSignal, createEffect } from 'solid-js'
import { createStore } from 'solid-js/store'
import {
  getMarketBreadth,
  getSectorRanking,
  type MarketBreadthResponse,
} from '../api/client'

// ==================== 타입 정의 ====================

/** 공포탐욕 지수 레벨 */
export type FearGreedLevel = 'extreme_fear' | 'fear' | 'neutral' | 'greed' | 'extreme_greed'

/** 공포탐욕 지수 데이터 (Market Breadth 기반) */
export interface FearGreedData {
  /** 현재 값 (0-100) */
  value: number
  /** 레벨 */
  level: FearGreedLevel
  /** 레벨 라벨 (한글) */
  label: string
  /** 전일 대비 변화 */
  change: number
}

/** 시장 온도 데이터 */
export interface MarketTemperature {
  /** 전체 시장 */
  all: number
  /** KOSPI */
  kospi: number
  /** KOSDAQ */
  kosdaq: number
  /** 온도 상태 (cold/neutral/warm/hot) */
  status: 'cold' | 'neutral' | 'warm' | 'hot'
}

/** 섹터 모멘텀 데이터 */
export interface SectorMomentum {
  name: string
  returnPct: number
  rank: number
}

/** 훅 내부 상태 */
interface UseMarketSentimentState {
  /** 자동 갱신 활성화 여부 */
  autoRefresh: boolean
  /** 갱신 간격 (ms) */
  refreshInterval: number
  /** 에러 메시지 */
  error: string | null
  /** 마지막 갱신 시간 */
  lastUpdated: Date | null
}

/** 훅 반환 타입 */
export interface UseMarketSentimentReturn {
  // ==================== 데이터 ====================
  /** 공포탐욕 지수 */
  fearGreedIndex: () => FearGreedData | null
  /** 시장 온도 (원본) */
  marketBreadth: () => MarketBreadthResponse | undefined
  /** 시장 온도 (가공) */
  marketTemperature: () => MarketTemperature | null
  /** 섹터 모멘텀 TOP 10 */
  topSectors: () => SectorMomentum[]
  /** 섹터 모멘텀 BOTTOM 10 */
  bottomSectors: () => SectorMomentum[]

  // ==================== 상태 ====================
  /** 로딩 */
  loading: boolean
  /** 에러 */
  error: () => string | null
  /** 마지막 갱신 시간 */
  lastUpdated: () => Date | null

  // ==================== 자동 갱신 ====================
  /** 자동 갱신 활성화 여부 */
  autoRefresh: () => boolean
  /** 자동 갱신 토글 */
  setAutoRefresh: (enabled: boolean) => void
  /** 갱신 간격 설정 (ms) */
  setRefreshInterval: (ms: number) => void

  // ==================== 액션 ====================
  /** 데이터 새로고침 */
  refresh: () => Promise<void>
}

// ==================== 상수 ====================

const DEFAULT_REFRESH_INTERVAL = 5 * 60 * 1000 // 5분

// ==================== 유틸리티 함수 ====================

/**
 * Market Breadth 값을 Fear & Greed Index로 변환
 * - 20일선 상회 비율이 높으면 탐욕, 낮으면 공포
 */
function calculateFearGreed(breadth: MarketBreadthResponse): FearGreedData {
  const allValue = parseFloat(breadth.all) || 50

  // 0-100 스케일로 변환 (이미 비율이므로 그대로 사용)
  const value = Math.round(allValue)

  // 레벨 결정
  let level: FearGreedLevel
  let label: string
  if (value <= 20) {
    level = 'extreme_fear'
    label = '극단적 공포'
  } else if (value <= 40) {
    level = 'fear'
    label = '공포'
  } else if (value <= 60) {
    level = 'neutral'
    label = '중립'
  } else if (value <= 80) {
    level = 'greed'
    label = '탐욕'
  } else {
    level = 'extreme_greed'
    label = '극단적 탐욕'
  }

  // 전일 대비 변화 (서버에서 제공하지 않으므로 0)
  const change = 0

  return { value, level, label, change }
}

/**
 * Market Breadth를 온도 데이터로 변환
 */
function calculateTemperature(breadth: MarketBreadthResponse): MarketTemperature {
  const all = parseFloat(breadth.all) || 50
  const kospi = parseFloat(breadth.kospi) || 50
  const kosdaq = parseFloat(breadth.kosdaq) || 50

  // 온도 상태 결정
  let status: MarketTemperature['status']
  if (all < 30) {
    status = 'cold'
  } else if (all < 50) {
    status = 'neutral'
  } else if (all < 70) {
    status = 'warm'
  } else {
    status = 'hot'
  }

  return { all, kospi, kosdaq, status }
}

// ==================== 초기 상태 ====================

const initialState: UseMarketSentimentState = {
  autoRefresh: false,
  refreshInterval: DEFAULT_REFRESH_INTERVAL,
  error: null,
  lastUpdated: null,
}

// ==================== 훅 구현 ====================

export function useMarketSentiment(): UseMarketSentimentReturn {
  // 내부 상태
  const [state, setState] = createStore<UseMarketSentimentState>({ ...initialState })

  // 타이머 ID
  let refreshTimerId: ReturnType<typeof setInterval> | null = null

  // ==================== 리소스 정의 ====================

  // Market Breadth
  const [breadthResource, { refetch: refetchBreadth }] = createResource(async () => {
    try {
      const data = await getMarketBreadth()
      setState('lastUpdated', new Date())
      return data
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : '시장 온도 조회 실패'
      setState('error', errorMsg)
      throw err
    }
  })

  // 섹터 랭킹
  const [sectorResource, { refetch: refetchSector }] = createResource(async () => {
    try {
      return await getSectorRanking('KR', 5)
    } catch {
      // 섹터 데이터는 선택적이므로 에러 무시
      return null
    }
  })

  // ==================== 파생 데이터 ====================

  const fearGreedIndex = createMemo(() => {
    const breadth = breadthResource()
    if (!breadth) return null
    return calculateFearGreed(breadth)
  })

  const marketTemperature = createMemo(() => {
    const breadth = breadthResource()
    if (!breadth) return null
    return calculateTemperature(breadth)
  })

  const topSectors = createMemo((): SectorMomentum[] => {
    const data = sectorResource()
    if (!data?.sectors) return []
    return data.sectors
      .filter((s) => s.returnPct >= 0)
      .sort((a, b) => b.returnPct - a.returnPct)
      .slice(0, 10)
      .map((s, i) => ({ name: s.name, returnPct: s.returnPct, rank: i + 1 }))
  })

  const bottomSectors = createMemo((): SectorMomentum[] => {
    const data = sectorResource()
    if (!data?.sectors) return []
    return data.sectors
      .filter((s) => s.returnPct < 0)
      .sort((a, b) => a.returnPct - b.returnPct)
      .slice(0, 10)
      .map((s, i) => ({ name: s.name, returnPct: s.returnPct, rank: i + 1 }))
  })

  // ==================== 자동 갱신 ====================

  const startAutoRefresh = () => {
    if (refreshTimerId) {
      clearInterval(refreshTimerId)
    }
    refreshTimerId = setInterval(() => {
      refresh()
    }, state.refreshInterval)
  }

  const stopAutoRefresh = () => {
    if (refreshTimerId) {
      clearInterval(refreshTimerId)
      refreshTimerId = null
    }
  }

  const setAutoRefresh = (enabled: boolean) => {
    setState('autoRefresh', enabled)
    if (enabled) {
      startAutoRefresh()
    } else {
      stopAutoRefresh()
    }
  }

  const setRefreshInterval = (ms: number) => {
    setState('refreshInterval', ms)
    if (state.autoRefresh) {
      startAutoRefresh() // 간격 변경 시 타이머 재시작
    }
  }

  // 컴포넌트 언마운트 시 타이머 정리
  onCleanup(() => {
    stopAutoRefresh()
  })

  // ==================== 새로고침 ====================

  const refresh = async () => {
    setState('error', null)
    await Promise.all([refetchBreadth(), refetchSector()])
  }

  // ==================== 반환 ====================

  return {
    // 데이터
    fearGreedIndex,
    marketBreadth: () => breadthResource(),
    marketTemperature,
    topSectors,
    bottomSectors,

    // 상태
    loading: breadthResource.loading,
    error: () => state.error,
    lastUpdated: () => state.lastUpdated,

    // 자동 갱신
    autoRefresh: () => state.autoRefresh,
    setAutoRefresh,
    setRefreshInterval,

    // 액션
    refresh,
  }
}
