/**
 * useMultiTimeframeKlines - 다중 타임프레임 Kline 데이터 훅.
 *
 * 기능:
 * - 타임프레임별 캐싱 (TTL 기반)
 * - 로딩 상태 관리
 * - 에러 처리 (부분 실패 시 성공한 TF 데이터는 유지)
 * - 자동 새로고침 옵션
 */
import { createSignal, createEffect, on, onCleanup, batch } from 'solid-js'
import { fetchMultiTimeframeKlines, type Timeframe, type CandleData } from '../api/client'

/** 캐시 엔트리 */
interface CacheEntry {
  data: CandleData[]
  timestamp: number
  symbol: string
}

/** 타임프레임별 TTL (ms) */
const TTL_BY_TIMEFRAME: Record<Timeframe, number> = {
  '1m': 30 * 1000,      // 30초
  '5m': 60 * 1000,      // 1분
  '15m': 2 * 60 * 1000, // 2분
  '30m': 5 * 60 * 1000, // 5분
  '1h': 10 * 60 * 1000, // 10분
  '4h': 30 * 60 * 1000, // 30분
  '1d': 60 * 60 * 1000, // 1시간
  '1w': 4 * 60 * 60 * 1000,  // 4시간
  '1M': 24 * 60 * 60 * 1000, // 24시간
}

/** 에러 상태 */
export interface TimeframeError {
  timeframe: Timeframe
  message: string
  code?: string
}

/** 훅 옵션 */
export interface UseMultiTimeframeKlinesOptions {
  /** 자동 새로고침 활성화 */
  autoRefresh?: boolean
  /** 새로고침 간격 (ms) */
  refreshInterval?: number
  /** 캐시 사용 여부 */
  useCache?: boolean
  /** 커스텀 TTL (전체 적용) */
  customTtl?: number
}

/** 훅 반환 타입 */
export interface UseMultiTimeframeKlinesReturn {
  /** 타임프레임별 Kline 데이터 */
  data: () => Record<Timeframe, CandleData[]>
  /** 로딩 중인 타임프레임 목록 */
  loading: () => Set<Timeframe>
  /** 전체 로딩 상태 (하나라도 로딩 중이면 true) */
  isLoading: () => boolean
  /** 에러 목록 */
  errors: () => TimeframeError[]
  /** 수동 새로고침 */
  refresh: (timeframes?: Timeframe[]) => Promise<void>
  /** 캐시 초기화 */
  clearCache: (timeframes?: Timeframe[]) => void
  /** 마지막 업데이트 시간 */
  lastUpdated: () => Record<Timeframe, number>
}

// 전역 캐시 (심볼+타임프레임 기반)
const globalCache = new Map<string, CacheEntry>()

/** 캐시 키 생성 */
function getCacheKey(symbol: string, timeframe: Timeframe): string {
  return `${symbol}:${timeframe}`
}

/** 캐시에서 유효한 데이터 조회 */
function getFromCache(
  symbol: string,
  timeframe: Timeframe,
  customTtl?: number
): CandleData[] | null {
  const key = getCacheKey(symbol, timeframe)
  const entry = globalCache.get(key)

  if (!entry || entry.symbol !== symbol) {
    return null
  }

  const ttl = customTtl ?? TTL_BY_TIMEFRAME[timeframe]
  const isExpired = Date.now() - entry.timestamp > ttl

  if (isExpired) {
    globalCache.delete(key)
    return null
  }

  return entry.data
}

/** 캐시에 데이터 저장 */
function setToCache(symbol: string, timeframe: Timeframe, data: CandleData[]): void {
  const key = getCacheKey(symbol, timeframe)
  globalCache.set(key, {
    data,
    timestamp: Date.now(),
    symbol,
  })
}

/**
 * 다중 타임프레임 Kline 데이터 훅
 *
 * @example
 * ```tsx
 * const { data, loading, errors, refresh } = useMultiTimeframeKlines(
 *   () => symbol(),
 *   () => ['5m', '1h', '1d'],
 *   100,
 *   { autoRefresh: true, refreshInterval: 30000 }
 * );
 *
 * // 데이터 사용
 * const primaryData = () => data()['5m'] || [];
 * const hourlyData = () => data()['1h'] || [];
 * ```
 */
export function useMultiTimeframeKlines(
  symbol: () => string,
  timeframes: () => Timeframe[],
  limit: number = 100,
  options: UseMultiTimeframeKlinesOptions = {}
): UseMultiTimeframeKlinesReturn {
  const {
    autoRefresh = false,
    refreshInterval = 60000,
    useCache = true,
    customTtl,
  } = options

  // 상태
  const [data, setData] = createSignal<Record<Timeframe, CandleData[]>>({} as Record<Timeframe, CandleData[]>)
  const [loading, setLoading] = createSignal<Set<Timeframe>>(new Set())
  const [errors, setErrors] = createSignal<TimeframeError[]>([])
  const [lastUpdated, setLastUpdated] = createSignal<Record<Timeframe, number>>({} as Record<Timeframe, number>)

  // 자동 새로고침 타이머
  let refreshTimer: number | null = null

  // 전체 로딩 상태
  const isLoading = () => loading().size > 0

  /**
   * 데이터 로드 (캐시 확인 후 필요시 API 호출)
   */
  const loadData = async (tfs?: Timeframe[]) => {
    const currentSymbol = symbol()
    const targetTimeframes = tfs || timeframes()

    if (!currentSymbol || targetTimeframes.length === 0) {
      return
    }

    // 캐시에서 유효한 데이터 확인
    const cachedData: Record<Timeframe, CandleData[]> = {} as Record<Timeframe, CandleData[]>
    const uncachedTimeframes: Timeframe[] = []

    if (useCache) {
      for (const tf of targetTimeframes) {
        const cached = getFromCache(currentSymbol, tf, customTtl)
        if (cached) {
          cachedData[tf] = cached
        } else {
          uncachedTimeframes.push(tf)
        }
      }

      // 캐시된 데이터 즉시 반영
      if (Object.keys(cachedData).length > 0) {
        setData(prev => ({ ...prev, ...cachedData }))
      }
    } else {
      uncachedTimeframes.push(...targetTimeframes)
    }

    // API 호출이 필요한 타임프레임이 없으면 종료
    if (uncachedTimeframes.length === 0) {
      return
    }

    // 로딩 상태 설정
    setLoading(prev => {
      const next = new Set(prev)
      uncachedTimeframes.forEach(tf => next.add(tf))
      return next
    })

    // 기존 에러 초기화 (재시도하는 TF만)
    setErrors(prev => prev.filter(e => !uncachedTimeframes.includes(e.timeframe)))

    try {
      const response = await fetchMultiTimeframeKlines(
        currentSymbol,
        uncachedTimeframes,
        limit
      )

      // 응답 데이터 처리
      const newData: Record<Timeframe, CandleData[]> = {} as Record<Timeframe, CandleData[]>
      const newErrors: TimeframeError[] = []
      const now = Date.now()

      for (const tf of uncachedTimeframes) {
        const tfData = response.klines[tf]

        if (tfData && tfData.length > 0) {
          newData[tf] = tfData
          // 캐시에 저장
          if (useCache) {
            setToCache(currentSymbol, tf, tfData)
          }
        } else {
          // 데이터가 없거나 빈 배열인 경우
          newErrors.push({
            timeframe: tf,
            message: `${tf} 데이터 없음`,
            code: 'NO_DATA',
          })
        }
      }

      // 상태 업데이트 (batch로 묶어서 한번에 렌더링)
      batch(() => {
        setData(prev => ({ ...prev, ...newData }))
        setLastUpdated(prev => {
          const updated = { ...prev }
          for (const tf of Object.keys(newData) as Timeframe[]) {
            updated[tf] = now
          }
          return updated
        })
        if (newErrors.length > 0) {
          setErrors(prev => [...prev, ...newErrors])
        }
      })
    } catch (error) {
      // 전체 요청 실패
      const errorMessage = error instanceof Error ? error.message : '알 수 없는 오류'
      const newErrors = uncachedTimeframes.map(tf => ({
        timeframe: tf,
        message: errorMessage,
        code: 'FETCH_ERROR',
      }))

      setErrors(prev => [...prev, ...newErrors])
    } finally {
      // 로딩 상태 해제
      setLoading(prev => {
        const next = new Set(prev)
        uncachedTimeframes.forEach(tf => next.delete(tf))
        return next
      })
    }
  }

  /**
   * 수동 새로고침 (캐시 무시)
   */
  const refresh = async (tfs?: Timeframe[]) => {
    const targetTimeframes = tfs || timeframes()
    const currentSymbol = symbol()

    // 캐시 삭제
    for (const tf of targetTimeframes) {
      globalCache.delete(getCacheKey(currentSymbol, tf))
    }

    // 데이터 로드
    await loadData(targetTimeframes)
  }

  /**
   * 캐시 초기화
   */
  const clearCache = (tfs?: Timeframe[]) => {
    const currentSymbol = symbol()
    const targetTimeframes = tfs || timeframes()

    for (const tf of targetTimeframes) {
      globalCache.delete(getCacheKey(currentSymbol, tf))
    }
  }

  // 심볼 또는 타임프레임 변경 시 데이터 로드
  createEffect(
    on(
      () => [symbol(), timeframes()] as const,
      () => {
        loadData()
      }
    )
  )

  // 자동 새로고침 설정
  createEffect(
    on(
      () => autoRefresh,
      (enabled) => {
        if (refreshTimer) {
          clearInterval(refreshTimer)
          refreshTimer = null
        }

        if (enabled) {
          refreshTimer = window.setInterval(() => {
            refresh()
          }, refreshInterval)
        }
      }
    )
  )

  // 클린업
  onCleanup(() => {
    if (refreshTimer) {
      clearInterval(refreshTimer)
    }
  })

  return {
    data,
    loading,
    isLoading,
    errors,
    refresh,
    clearCache,
    lastUpdated,
  }
}

/**
 * 전역 캐시 초기화 (테스트용)
 */
export function clearGlobalKlinesCache(): void {
  globalCache.clear()
}
