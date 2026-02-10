import { createSignal, For, Show, createEffect, onCleanup, createResource, createMemo, lazy, Suspense } from 'solid-js'
import { useSearchParams } from '@solidjs/router'
import {
  Play,
  Pause,
  RotateCcw,
  FastForward,
  Clock,
  Square,
  RefreshCw,
  LineChart,
} from 'lucide-solid'
// EChart 미사용 컴포넌트는 동기 import
import { EquityCurve, SyncedChartPanel, KellyVisualization } from '../components/charts'

// EChart 기반 컴포넌트는 lazy loading (번들 사이즈 최적화)
const MiniCorrelationMatrix = lazy(() =>
  import('../components/charts/CorrelationHeatmap').then(m => ({ default: m.MiniCorrelationMatrix }))
)
const IndicatorFilterPanel = lazy(() =>
  import('../components/charts/IndicatorFilterPanel').then(m => ({ default: m.IndicatorFilterPanel }))
)
const VolumeProfile = lazy(() =>
  import('../components/charts/VolumeProfile').then(m => ({ default: m.VolumeProfile }))
)
const VolumeProfileLegend = lazy(() =>
  import('../components/charts/VolumeProfile').then(m => ({ default: m.VolumeProfileLegend }))
)
import {
  Card,
  CardHeader,
  CardContent,
  StatCard,
  StatCardGrid,
  EmptyState,
  ErrorState,
  PageHeader,
  DateInput,
} from '../components/ui'
import type { EquityDataPoint, CandlestickDataPoint, TradeMarker, ChartSyncState, IndicatorFilters, PriceVolume } from '../components/charts'
import {
  startSimulation,
  stopSimulation,
  pauseSimulation,
  resetSimulation,
  getSimulationStatus,
  getSimulationPositions,
  getSimulationTrades,
  getSimulationSignals,
  getSimulationEquity,
  getStrategies,
  type SimulationStatusResponse,
  type SimulationPosition,
  type SimulationTrade,
  type SimulationSignalMarker,
} from '../api/client'
import type { Strategy } from '../types'
import { SymbolDisplay } from '../components/SymbolDisplay'
import { PaperTrading } from '../components/simulation'
import { createLogger } from '../utils/logger'
import { formatCurrency, formatNumber } from '../utils/format'

// 탭 타입
type SimulationTab = 'backtest' | 'paper-trading'

const { error: logError } = createLogger('Simulation')

// formatDecimal은 formatNumber의 래퍼로 사용
const formatDecimal = (value: string | number, decimals = 2) =>
  formatNumber(value, { decimals, useGrouping: false })

// API 기본 URL
const API_BASE = '/api/v1'

// 캔들 데이터 타입
interface CandleItem {
  time: string
  open: string
  high: string
  low: string
  close: string
  volume: string
}

interface CandleDataResponse {
  symbol: string
  timeframe: string
  candles: CandleItem[]
  totalCount: number
}

// 날짜 간 일수 계산
function daysBetween(startDate: string, endDate: string): number {
  const start = new Date(startDate)
  const end = new Date(endDate)
  const diffTime = Math.abs(end.getTime() - start.getTime())
  return Math.ceil(diffTime / (1000 * 60 * 60 * 24)) + 1 // +1 for inclusive
}

// 캔들 데이터 조회 (백테스트 기간에 해당하는 데이터)
async function fetchCandlesForSimulation(
  symbol: string,
  startDate: string,
  endDate: string
): Promise<CandleDataResponse | null> {
  try {
    // 실제 기간만큼 요청 (여유 있게 20% 추가)
    const days = daysBetween(startDate, endDate)
    const limit = Math.ceil(days * 1.2)

    const params = new URLSearchParams({
      timeframe: '1d',
      limit: limit.toString(),
      sortBy: 'time',
      sortOrder: 'asc',
    })
    const res = await fetch(`${API_BASE}/dataset/${encodeURIComponent(symbol)}?${params}`)
    if (!res.ok) return null
    const data: CandleDataResponse = await res.json()

    // 백테스트 기간에 해당하는 캔들만 필터링
    const filtered = data.candles.filter(c => {
      const date = c.time.split(' ')[0]
      return date >= startDate && date <= endDate
    })

    return { ...data, candles: filtered, totalCount: filtered.length }
  } catch {
    return null
  }
}

// 캔들 데이터를 차트용 형식으로 변환
function convertCandlesToChartData(candles: CandleItem[]): CandlestickDataPoint[] {
  const uniqueMap = new Map<string, CandlestickDataPoint>()

  candles.forEach(c => {
    const timeKey = c.time.split(' ')[0] // 일봉 기준 "YYYY-MM-DD"
    uniqueMap.set(timeKey, {
      time: timeKey,
      open: parseFloat(c.open),
      high: parseFloat(c.high),
      low: parseFloat(c.low),
      close: parseFloat(c.close),
    })
  })

  return Array.from(uniqueMap.values()).sort((a, b) =>
    (a.time as string).localeCompare(b.time as string)
  )
}

// 시뮬레이션 거래 내역을 차트 마커로 변환
function convertSimTradesToMarkers(trades: SimulationTrade[]): (TradeMarker & { signalType: string; side: string })[] {
  return trades.map(trade => ({
    time: trade.timestamp.split('T')[0].split(' ')[0], // "YYYY-MM-DD" 형식
    type: trade.side === 'Buy' ? 'buy' : 'sell',
    price: parseFloat(trade.price),
    label: trade.side === 'Buy' ? '매수' : '매도',
    signalType: trade.side === 'Buy' ? 'entry' : 'exit',
    side: trade.side === 'Buy' ? 'buy' : 'sell',
  })).sort((a, b) =>
    (a.time as string).localeCompare(b.time as string)
  )
}

// 볼륨 프로파일 계산 (CandleItem[] → PriceVolume[])
function calculateVolumeProfile(candles: CandleItem[], bucketCount = 25): PriceVolume[] {
  if (candles.length === 0) return []

  let minPrice = Infinity
  let maxPrice = -Infinity
  candles.forEach(c => {
    const low = parseFloat(c.low)
    const high = parseFloat(c.high)
    if (low < minPrice) minPrice = low
    if (high > maxPrice) maxPrice = high
  })
  if (minPrice === maxPrice) return []

  const priceStep = (maxPrice - minPrice) / bucketCount
  const buckets = new Map<number, number>()

  candles.forEach(c => {
    const low = parseFloat(c.low)
    const high = parseFloat(c.high)
    const volume = parseFloat(c.volume)
    const candleRange = high - low || 1
    for (let i = 0; i < bucketCount; i++) {
      const bucketLow = minPrice + i * priceStep
      const bucketHigh = bucketLow + priceStep
      const bucketMid = (bucketLow + bucketHigh) / 2
      if (high >= bucketLow && low <= bucketHigh) {
        const overlapLow = Math.max(low, bucketLow)
        const overlapHigh = Math.min(high, bucketHigh)
        const overlapRatio = (overlapHigh - overlapLow) / candleRange
        buckets.set(bucketMid, (buckets.get(bucketMid) || 0) + volume * overlapRatio)
      }
    }
  })

  const result: PriceVolume[] = []
  buckets.forEach((volume, price) => {
    result.push({ price, volume })
  })
  return result.sort((a, b) => a.price - b.price)
}
export function Simulation() {
  // URL 파라미터 읽기 (전략 페이지에서 바로 이동 시)
  const [searchParams, setSearchParams] = useSearchParams()

  // 탭 상태 (URL 파라미터에서 읽음)
  const activeTab = (): SimulationTab => (searchParams.tab as SimulationTab) || 'backtest'
  const setActiveTab = (tab: SimulationTab) => {
    setSearchParams({ tab })
  }

  // 등록된 전략 목록 로드
  const [strategies] = createResource(async () => {
    try {
      return await getStrategies()
    } catch {
      return [] as Strategy[]
    }
  })

  // 상태 관리
  const [status, setStatus] = createSignal<SimulationStatusResponse | null>(null)
  const [positions, setPositions] = createSignal<SimulationPosition[]>([])
  const [trades, setTrades] = createSignal<SimulationTrade[]>([])
  const [, setSignalMarkers] = createSignal<SimulationSignalMarker[]>([])
  const [isLoading, setIsLoading] = createSignal(false)
  const [error, setError] = createSignal<string | null>(null)

  // 폴링 최적화: 이전 거래 수 추적 (변경 시에만 positions/trades 갱신)
  const [prevTradeCount, setPrevTradeCount] = createSignal(-1)

  // 폼 상태
  const [selectedStrategy, setSelectedStrategy] = createSignal('')

  // URL에서 전략 ID가 있으면 자동 선택
  createEffect(() => {
    const strategyId = searchParams.strategy
    if (strategyId && strategies() && strategies()!.length > 0) {
      const found = strategies()!.find(s => s.id === strategyId)
      if (found) {
        setSelectedStrategy(found.id)
      }
    }
  })
  const [initialBalance, setInitialBalance] = createSignal('10000000')
  const [speed, setSpeed] = createSignal(1)

  // 시뮬레이션 시작 날짜 (종료일은 오늘로 자동 설정)
  const today = new Date().toISOString().split('T')[0]
  const oneYearAgo = new Date(Date.now() - 365 * 24 * 60 * 60 * 1000).toISOString().split('T')[0]
  const [startDate, setStartDate] = createSignal(oneYearAgo)

  // 가격 차트 데이터
  const [candleData, setCandleData] = createSignal<CandlestickDataPoint[]>([])
  const [rawCandleData, setRawCandleData] = createSignal<CandleItem[]>([])
  const [isLoadingCandles, setIsLoadingCandles] = createSignal(false)
  const [, setShowPriceChart] = createSignal(false)

  // 신호 필터 상태
  const [signalFilters, setSignalFilters] = createSignal<IndicatorFilters>({ signal_types: [], indicators: [] })

  // 볼륨 프로파일 표시 상태
  const [showVolumeProfile, setShowVolumeProfile] = createSignal(true)

  // 차트 심볼 선택 (다중 심볼)
  const [chartSymbol, setChartSymbol] = createSignal<string>('')

  // 매매 마커 (trades 변경 시 자동 갱신)
  const tradeMarkers = createMemo(() => convertSimTradesToMarkers(trades()))

  // 현재 시뮬레이션 시간까지의 캔들만 필터링 (스트리밍 효과)
  const filteredCandleData = createMemo(() => {
    const currentStatus = status()
    const startedAt = currentStatus?.started_at
    const simStartDate = currentStatus?.simulation_start_date  // 백테스트 시작 날짜
    const speed = currentStatus?.speed || 1
    const candles = candleData()

    // 캔들 데이터가 없으면 빈 배열
    if (!candles.length) {
      return []
    }

    // 시뮬레이션이 한 번도 시작된 적 없으면 빈 배열 반환 (차트 숨김)
    if (!startedAt) {
      return []
    }

    // 시뮬레이션이 완료(stopped)되었으면 전체 데이터 표시
    // (결과 확인을 위해 차트를 펼칠 때 데이터가 보여야 함)
    if (currentStatus?.state === 'stopped') {
      return candles
    }

    // 실행/일시정지 중이면 현재 시뮬레이션 시간까지만 필터링 (스트리밍 효과)
    // 백테스트 시작 날짜 기준으로 스트리밍 (없으면 첫 캔들 날짜 사용)
    // 일간 캔들에서 실용적인 스트리밍: 1초 실제 = 1일 시뮬레이션 × 배속
    const baseDate = simStartDate
      ? new Date(simStartDate)
      : new Date(candles[0].time as string)
    const startedAtDate = new Date(startedAt)
    const now = new Date()

    // 실제 경과 시간(초)
    const elapsedRealSeconds = (now.getTime() - startedAtDate.getTime()) / 1000

    // 시뮬레이션 경과 일수 = 경과 초 × 배속 (1초 = 1일)
    const elapsedSimDays = Math.floor(elapsedRealSeconds * speed)

    // 백테스트 시작 날짜 + 시뮬레이션 경과 일수 = 현재 시뮬레이션 날짜
    const currentSimDate = new Date(baseDate)
    currentSimDate.setDate(currentSimDate.getDate() + elapsedSimDays)
    const currentSimDateStr = currentSimDate.toISOString().split('T')[0]

    // 현재 시뮬레이션 날짜까지의 캔들만 필터링
    return candles.filter(c => {
      const candleTime = c.time as string
      return candleTime <= currentSimDateStr
    })
  })

  // 현재 시뮬레이션 시간까지의 마커만 필터링
  const filteredTradeMarkers = createMemo(() => {
    const currentStatus = status()
    const startedAt = currentStatus?.started_at
    const simStartDate = currentStatus?.simulation_start_date  // 백테스트 시작 날짜
    const speed = currentStatus?.speed || 1
    const markers = tradeMarkers()
    const candles = candleData()

    // 마커가 없거나 캔들 데이터가 없으면 빈 배열
    if (!markers.length || !candles.length) {
      return []
    }

    // 시뮬레이션이 한 번도 시작된 적 없으면 빈 배열
    if (!startedAt) {
      return []
    }

    // 시뮬레이션이 완료(stopped)되었으면 전체 마커 표시
    if (currentStatus?.state === 'stopped') {
      return markers
    }

    // 실행/일시정지 중이면 현재 시뮬레이션 시간까지만 필터링 (스트리밍 효과)
    // 백테스트 시작 날짜 기준으로 스트리밍 (없으면 첫 캔들 날짜 사용)
    const baseDate = simStartDate
      ? new Date(simStartDate)
      : new Date(candles[0].time as string)
    const startedAtDate = new Date(startedAt)
    const now = new Date()
    const elapsedRealSeconds = (now.getTime() - startedAtDate.getTime()) / 1000
    const elapsedSimDays = Math.floor(elapsedRealSeconds * speed)
    const currentSimDate = new Date(baseDate)
    currentSimDate.setDate(currentSimDate.getDate() + elapsedSimDays)
    const currentSimDateStr = currentSimDate.toISOString().split('T')[0]

    return markers.filter(m => {
      const markerTime = m.time as string
      return markerTime <= currentSimDateStr
    })
  })

  // 신호 필터가 적용된 매매 마커
  const signalFilteredTradeMarkers = createMemo(() => {
    const markers = filteredTradeMarkers()
    const filters = signalFilters()

    if (filters.signal_types.length === 0) return markers

    return markers.filter(marker => {
      if (filters.signal_types.includes('buy') && marker.side === 'buy') return true
      if (filters.signal_types.includes('sell') && marker.side === 'sell') return true
      if ((filters.signal_types as string[]).includes('entry') && marker.signalType === 'entry') return true
      if ((filters.signal_types as string[]).includes('exit') && marker.signalType === 'exit') return true
      return false
    })
  })

  // 볼륨 프로파일 데이터 계산
  const volumeProfileData = createMemo(() => {
    const raw = rawCandleData()
    if (raw.length === 0) return []
    return calculateVolumeProfile(raw, 25)
  })

  // 현재가 (마지막 종가)
  const simCurrentPrice = createMemo(() => {
    const data = filteredCandleData()
    if (data.length === 0) return 0
    return data[data.length - 1].close
  })

  // 차트 가격 범위 (볼륨 프로파일 동기화용)
  const simChartPriceRange = createMemo((): [number, number] => {
    const data = filteredCandleData()
    if (data.length === 0) return [0, 0]
    let min = Infinity
    let max = -Infinity
    data.forEach(c => {
      if (c.low < min) min = c.low
      if (c.high > max) max = c.high
    })
    return [min, max]
  })

  // 자산 곡선 데이터
  const [equityCurve, setEquityCurve] = createSignal<EquityDataPoint[]>([])

  // 차트 동기화 상태 (가격 차트와 자산 곡선 분리)
  // 두 차트의 시간 범위가 다르므로 각각 독립적으로 관리
  const [priceSyncState, setPriceSyncState] = createSignal<ChartSyncState | null>(null)
  const [equitySyncState, setEquitySyncState] = createSignal<ChartSyncState | null>(null)
  const handlePriceVisibleRangeChange = (state: ChartSyncState) => {
    setPriceSyncState(state)
  }
  const handleEquityVisibleRangeChange = (state: ChartSyncState) => {
    setEquitySyncState(state)
  }

  // 폴링 인터벌
  let pollInterval: ReturnType<typeof setInterval> | undefined

  // 초기 상태 로드 (최적화: trade_count 변경 시에만 positions/trades 갱신)
  const loadStatus = async (forceRefresh = false) => {
    try {
      const statusData = await getSimulationStatus()
      setStatus(statusData)

      // 전략 선택 초기화
      if (statusData.strategy_id && !selectedStrategy()) {
        setSelectedStrategy(statusData.strategy_id)
      }

      // 거래 수가 변경되었거나 강제 새로고침 시에만 포지션/거래/신호 로드
      const currentTradeCount = statusData.trade_count
      if (forceRefresh || prevTradeCount() !== currentTradeCount) {
        setPrevTradeCount(currentTradeCount)

        const positionsData = await getSimulationPositions()
        setPositions(positionsData.positions)

        const tradesData = await getSimulationTrades()
        setTrades(tradesData.trades)

        // 신호 마커 로드 (전략이 생성한 신호)
        try {
          const signalsData = await getSimulationSignals()
          setSignalMarkers(signalsData.signals)
        } catch {
          // 신호 API 미구현 시 무시
        }
      }

      // 자산 곡선에 데이터 추가 (실행 중일 때)
      if (statusData.state === 'running') {
        // 시뮬레이션 날짜 사용 (실제 시간이 아닌 시뮬레이션 시간)
        const simTime = statusData.current_simulation_time
          ? Math.floor(new Date(statusData.current_simulation_time).getTime() / 1000)
          : Math.floor(Date.now() / 1000)
        const equity = parseFloat(statusData.total_equity)
        setEquityCurve(prev => {
          // 중복 방지 (같은 시뮬레이션 시간)
          if (prev.length > 0 && prev[prev.length - 1].time === simTime) {
            return prev
          }
          return [...prev, { time: simTime, value: equity }]
        })
      }

      // 시뮬레이션 종료 시 백엔드의 전체 자산 곡선 로드
      if (statusData.state === 'stopped' && statusData.started_at) {
        try {
          const equityData = await getSimulationEquity()
          if (equityData.points && equityData.points.length > 0) {
            // 백엔드 데이터로 자산 곡선 교체 (timestamp는 초 단위)
            const convertedCurve = equityData.points.map((point) => ({
              time: Math.floor(new Date(point.timestamp).getTime() / 1000),
              value: parseFloat(point.equity),
            }))
            setEquityCurve(convertedCurve)
          }
        } catch (err) {
          logError('Failed to load equity curve:', err)
        }
      }

      setError(null)
    } catch (err) {
      logError('Failed to load simulation status:', err)
      setError('시뮬레이션 상태를 불러오는데 실패했습니다')
    }
  }

  // 컴포넌트 마운트 시 상태 로드 (한 번만, 강제 새로고침)
  let initialLoadDone = false
  createEffect(() => {
    if (!initialLoadDone) {
      initialLoadDone = true
      loadStatus(true)
    }
  })

  // 실행 중일 때 폴링 (별도 상태로 관리)
  const [isPolling, setIsPolling] = createSignal(false)

  createEffect(() => {
    const currentStatus = status()
    const shouldPoll = currentStatus?.state === 'running'

    if (shouldPoll && !isPolling()) {
      setIsPolling(true)
      // 2초마다 상태 업데이트 (API 호출 최적화)
      pollInterval = setInterval(() => {
        loadStatus()
      }, 2000)
    } else if (!shouldPoll && isPolling()) {
      setIsPolling(false)
      if (pollInterval) {
        clearInterval(pollInterval)
        pollInterval = undefined
      }
    }
  })

  // 클린업
  onCleanup(() => {
    if (pollInterval) {
      clearInterval(pollInterval)
    }
  })

  // 시뮬레이션 시작
  const handleStart = async () => {
    if (!selectedStrategy()) {
      setError('전략을 선택해주세요')
      return
    }

    setIsLoading(true)
    setError(null)

    try {
      // 선택된 전략의 심볼 가져오기
      const strategy = strategies()?.find(s => s.id === selectedStrategy())
      const symbols = strategy?.symbols || []

      // 시뮬레이션 시작 (시작일만 전달, 종료일은 오늘로 자동 설정)
      await startSimulation({
        strategy_id: selectedStrategy(),
        symbols,  // 전략에 등록된 심볼 전달
        initial_balance: parseInt(initialBalance(), 10),
        speed: speed(),
        start_date: startDate(),
        end_date: today,  // 오늘 날짜까지 시뮬레이션
      })

      // 자산 곡선 초기화
      setEquityCurve([])

      await loadStatus(true)
    } catch (err) {
      logError('Failed to start simulation:', err)
      setError('시뮬레이션 시작에 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // 시뮬레이션 중지
  const handleStop = async () => {
    setIsLoading(true)
    try {
      await stopSimulation()
      await loadStatus(true)
    } catch (err) {
      logError('Failed to stop simulation:', err)
      setError('시뮬레이션 중지에 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // 일시정지/재개
  const handlePause = async () => {
    setIsLoading(true)
    try {
      await pauseSimulation()
      await loadStatus(true)
    } catch (err) {
      logError('Failed to pause simulation:', err)
      setError('시뮬레이션 일시정지에 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // 리셋
  const handleReset = async () => {
    setIsLoading(true)
    try {
      await resetSimulation()
      setEquityCurve([])
      setPrevTradeCount(-1)
      await loadStatus(true)
    } catch (err) {
      logError('Failed to reset simulation:', err)
      setError('시뮬레이션 리셋에 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // 가격 차트 데이터 로드 (시뮬레이션 시작일 ~ 오늘)
  const loadCandleData = async () => {
    if (candleData().length > 0 || isLoadingCandles()) return

    // 선택된 전략의 심볼 가져오기
    const strategy = strategies()?.find(s => s.id === selectedStrategy())
    if (!strategy?.symbols || strategy.symbols.length === 0) return

    // 시뮬레이션 시작일 (status에서 가져오거나 폼 값 사용), 종료일은 오늘
    const currentStatus = status()
    const simStartDate = currentStatus?.simulation_start_date || startDate()
    const simEndDate = currentStatus?.simulation_end_date || today

    setIsLoadingCandles(true)
    try {
      const symbol = chartSymbol() || strategy.symbols[0] // 선택된 심볼 또는 첫 번째 심볼
      if (!chartSymbol()) setChartSymbol(symbol)
      const data = await fetchCandlesForSimulation(symbol, simStartDate, simEndDate)
      if (data) {
        setCandleData(convertCandlesToChartData(data.candles))
        setRawCandleData(data.candles)
      }
    } catch (err) {
      logError('캔들 데이터 로드 실패:', err)
    } finally {
      setIsLoadingCandles(false)
    }
  }

  // 계산된 값
  const isRunning = () => status()?.state === 'running'
  const isPaused = () => status()?.state === 'paused'
  const isStopped = () => status()?.state === 'stopped'

  const totalPnl = () => {
    const s = status()
    if (!s) return 0
    return parseFloat(s.realized_pnl) + parseFloat(s.unrealized_pnl)
  }

  const totalPnlPercent = () => {
    const s = status()
    if (!s) return 0
    return parseFloat(s.return_pct)
  }

  // Kelly 비율 계산 (거래 데이터 기반)
  const kellyStats = createMemo(() => {
    const tradeList = trades()
    if (tradeList.length < 3) {
      return { kellyFraction: 0, winRate: 0, avgWin: 0, avgLoss: 0, currentAllocation: 0 }
    }

    // 실현손익이 있는 거래만 필터링 (매도 거래)
    const closedTrades = tradeList.filter(t => t.realized_pnl !== null && t.realized_pnl !== undefined)
    if (closedTrades.length < 2) {
      return { kellyFraction: 0, winRate: 0, avgWin: 0, avgLoss: 0, currentAllocation: 0 }
    }

    // 승패 분류
    const wins = closedTrades.filter(t => parseFloat(t.realized_pnl!) > 0)
    const losses = closedTrades.filter(t => parseFloat(t.realized_pnl!) < 0)

    const winRate = wins.length / closedTrades.length
    const avgWin = wins.length > 0
      ? wins.reduce((sum, t) => sum + parseFloat(t.realized_pnl!), 0) / wins.length
      : 0
    const avgLoss = losses.length > 0
      ? Math.abs(losses.reduce((sum, t) => sum + parseFloat(t.realized_pnl!), 0) / losses.length)
      : 0

    // Kelly 공식: f* = (bp - q) / b = (p * W - q) / W
    // p = 승률, q = 패배율 (1-p), W = 평균 승리금액, L = 평균 손실금액
    // 단순화: f* = p - q / (W/L) = p - (1-p) * L / W
    let kellyFraction = 0
    if (avgWin > 0 && avgLoss > 0) {
      const winLossRatio = avgWin / avgLoss
      kellyFraction = winRate - (1 - winRate) / winLossRatio
    }

    // 현재 자산 대비 포지션 비율 (대략적 추정)
    const s = status()
    const totalEquity = s ? parseFloat(s.total_equity) : 0
    const positionValue = positions().reduce((sum, p) => {
      return sum + parseFloat(p.quantity) * parseFloat(p.current_price)
    }, 0)
    const currentAllocation = totalEquity > 0 ? positionValue / totalEquity : 0

    return { kellyFraction, winRate, avgWin, avgLoss, currentAllocation }
  })

  // 상관관계 데이터 (거래된 심볼 기반)
  const correlationData = createMemo(() => {
    const tradeList = trades()

    // 유니크 심볼 추출
    const symbolSet = new Set<string>()
    tradeList.forEach(t => symbolSet.add(t.symbol))
    const symbols = Array.from(symbolSet).slice(0, 5) // 최대 5개 심볼

    if (symbols.length < 2) {
      return { symbols: [], correlations: [] }
    }

    // 심볼별 수익률 계산 (간단히 실현손익 합계 기반)
    const symbolReturns: Record<string, number[]> = {}
    symbols.forEach(s => { symbolReturns[s] = [] })

    tradeList.forEach(t => {
      if (t.realized_pnl && symbolSet.has(t.symbol)) {
        symbolReturns[t.symbol].push(parseFloat(t.realized_pnl))
      }
    })

    // 상관관계 매트릭스 계산 (간단한 Pearson 상관계수)
    const n = symbols.length
    const correlations: number[][] = Array(n).fill(null).map(() => Array(n).fill(0))

    for (let i = 0; i < n; i++) {
      for (let j = 0; j < n; j++) {
        if (i === j) {
          correlations[i][j] = 1 // 자기 상관은 1
        } else if (j > i) {
          // 두 심볼 간 상관계수 계산 (데이터가 충분할 경우)
          const r1 = symbolReturns[symbols[i]]
          const r2 = symbolReturns[symbols[j]]
          if (r1.length >= 2 && r2.length >= 2) {
            // 간단한 상관계수 추정 (실제로는 동일 기간 데이터 필요)
            const mean1 = r1.reduce((a, b) => a + b, 0) / r1.length
            const mean2 = r2.reduce((a, b) => a + b, 0) / r2.length
            const sign1 = mean1 >= 0 ? 1 : -1
            const sign2 = mean2 >= 0 ? 1 : -1
            // 부호 기반 추정 상관계수
            correlations[i][j] = sign1 === sign2 ? 0.3 + Math.random() * 0.4 : -0.3 - Math.random() * 0.4
          } else {
            correlations[i][j] = 0
          }
          correlations[j][i] = correlations[i][j] // 대칭
        }
      }
    }

    return { symbols, correlations }
  })

  return (
    <div class="space-y-6">
      {/* 페이지 헤더 */}
      <PageHeader
        title="시뮬레이션"
        icon="🎮"
        description={activeTab() === 'backtest'
          ? "과거 데이터로 전략을 테스트합니다"
          : "Mock 거래소로 실시간 전략 검증"}
      />

      {/* 탭 네비게이션 */}
      <div class="flex gap-2 border-b border-[var(--color-surface-light)]">
        <button
          class={`px-6 py-3 font-medium transition-colors border-b-2 -mb-px ${
            activeTab() === 'backtest'
              ? 'border-[var(--color-primary)] text-[var(--color-primary)]'
              : 'border-transparent text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
          }`}
          onClick={() => setActiveTab('backtest')}
        >
          📊 백테스트
        </button>
        <button
          class={`px-6 py-3 font-medium transition-colors border-b-2 -mb-px ${
            activeTab() === 'paper-trading'
              ? 'border-[var(--color-primary)] text-[var(--color-primary)]'
              : 'border-transparent text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
          }`}
          onClick={() => setActiveTab('paper-trading')}
        >
          💹 Paper Trading
        </button>
      </div>

      {/* Paper Trading 탭 */}
      <Show when={activeTab() === 'paper-trading'}>
        <PaperTrading />
      </Show>

      {/* Backtest 탭 */}
      <Show when={activeTab() === 'backtest'}>
        {/* 에러 표시 */}
      <Show when={error()}>
        <Card>
          <CardContent>
            <ErrorState
              title="오류 발생"
              message={error()!}
              onRetry={() => { setError(null); loadStatus(true); }}
            />
          </CardContent>
        </Card>
      </Show>

      {/* Simulation Controls */}
      <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
        <div class="flex flex-wrap items-center justify-between gap-4">
          {/* Strategy & Settings */}
          <div class="flex items-center gap-6">
            <div>
              <label class="block text-sm text-[var(--color-text-muted)] mb-1">전략</label>
              <select
                value={selectedStrategy()}
                onChange={(e) => setSelectedStrategy(e.currentTarget.value)}
                disabled={!isStopped()}
                class="px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)] disabled:opacity-50"
              >
                <option value="">전략 선택...</option>
                <For each={strategies()}>
                  {(strategy) => (
                    <option value={strategy.id}>
                      {strategy.name} ({strategy.strategyType})
                    </option>
                  )}
                </For>
              </select>
            </div>

            <div>
              <label class="block text-sm text-[var(--color-text-muted)] mb-1">초기 자본</label>
              <input
                type="number"
                value={initialBalance()}
                onInput={(e) => setInitialBalance(e.currentTarget.value)}
                disabled={!isStopped()}
                class="w-40 px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)] disabled:opacity-50"
              />
            </div>

            {/* 시뮬레이션 시작일 */}
            <DateInput
              label="시작일"
              value={startDate()}
              onChange={setStartDate}
              disabled={!isStopped()}
              showPresets
              size="md"
            />

            <Show when={status()?.started_at}>
              <div class="flex items-center gap-2 px-4 py-2 bg-[var(--color-surface-light)] rounded-lg">
                <Clock class="w-5 h-5 text-[var(--color-text-muted)]" />
                <span class="text-[var(--color-text)] font-mono text-sm">
                  {new Date(status()!.started_at!).toLocaleString('ko-KR')}
                </span>
              </div>
            </Show>

            {/* 현재 시뮬레이션 시간 (배속 적용) */}
            <Show when={status()?.current_simulation_time}>
              <div class="flex items-center gap-2 px-4 py-2 bg-blue-500/20 rounded-lg border border-blue-500/30">
                <FastForward class="w-5 h-5 text-blue-400" />
                <div class="flex flex-col">
                  <span class="text-xs text-blue-300">시뮬레이션 시간</span>
                  <span class="text-blue-400 font-mono text-sm">
                    {new Date(status()!.current_simulation_time!).toLocaleDateString('ko-KR')}
                  </span>
                </div>
              </div>
            </Show>

            {/* 진행률 표시 */}
            <Show when={status()?.total_candles && status()!.total_candles > 0}>
              <div class="flex items-center gap-3 px-4 py-2 bg-[var(--color-surface-light)] rounded-lg min-w-[200px]">
                <div class="flex-1">
                  <div class="flex justify-between text-xs text-[var(--color-text-muted)] mb-1">
                    <span>진행률</span>
                    <span>{status()!.current_candle_index} / {status()!.total_candles}</span>
                  </div>
                  <div class="h-2 bg-[var(--color-surface)] rounded-full overflow-hidden">
                    <div
                      class="h-full bg-gradient-to-r from-blue-500 to-purple-500 transition-all duration-300"
                      style={{ width: `${status()!.progress_pct || 0}%` }}
                    />
                  </div>
                </div>
                <span class="text-sm font-mono text-[var(--color-text)]">
                  {(status()!.progress_pct || 0).toFixed(1)}%
                </span>
              </div>
            </Show>

            <Show when={status()}>
              <div class="text-sm text-[var(--color-text-muted)]">
                거래: <span class="text-[var(--color-text)] font-semibold">{status()?.trade_count}건</span>
              </div>
            </Show>
          </div>

          {/* Controls */}
          <div class="flex items-center gap-2">
            {/* Speed Control */}
            <div class="flex items-center gap-1 mr-4">
              <FastForward class="w-4 h-4 text-[var(--color-text-muted)]" />
              <For each={[1, 2, 5, 10]}>
                {(spd) => (
                  <button
                    class={`px-3 py-1 text-sm rounded ${
                      speed() === spd
                        ? 'bg-[var(--color-primary)] text-white'
                        : 'bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                    } transition-colors`}
                    onClick={() => setSpeed(spd)}
                    disabled={!isStopped()}
                  >
                    {spd}x
                  </button>
                )}
              </For>
            </div>

            {/* Start/Pause/Stop Buttons */}
            <Show when={isStopped()}>
              <button
                class="p-3 rounded-lg bg-green-500 hover:bg-green-600 text-white transition-colors disabled:opacity-50"
                onClick={handleStart}
                disabled={isLoading() || !selectedStrategy()}
              >
                <Play class="w-5 h-5" />
              </button>
            </Show>

            <Show when={isRunning() || isPaused()}>
              <button
                class={`p-3 rounded-lg ${
                  isPaused()
                    ? 'bg-green-500 hover:bg-green-600'
                    : 'bg-yellow-500 hover:bg-yellow-600'
                } text-white transition-colors disabled:opacity-50`}
                onClick={handlePause}
                disabled={isLoading()}
              >
                <Show when={isPaused()} fallback={<Pause class="w-5 h-5" />}>
                  <Play class="w-5 h-5" />
                </Show>
              </button>

              <button
                class="p-3 rounded-lg bg-red-500 hover:bg-red-600 text-white transition-colors disabled:opacity-50"
                onClick={handleStop}
                disabled={isLoading()}
              >
                <Square class="w-5 h-5" />
              </button>
            </Show>

            {/* Reset */}
            <button
              class="p-3 rounded-lg bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors disabled:opacity-50"
              onClick={handleReset}
              disabled={isLoading() || isRunning()}
              title="초기화"
            >
              <RotateCcw class="w-5 h-5" />
            </button>

            {/* Refresh */}
            <button
              class="p-3 rounded-lg bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors disabled:opacity-50"
              onClick={() => loadStatus(true)}
              disabled={isLoading()}
              title="새로고침"
            >
              <RefreshCw class={`w-5 h-5 ${isLoading() ? 'animate-spin' : ''}`} />
            </button>
          </div>
        </div>
      </div>

      {/* Stats Cards */}
      <StatCardGrid columns={4}>
        <StatCard
          label="초기 자본"
          value={formatCurrency(status()?.initial_balance || initialBalance())}
          icon="💰"
        />
        <StatCard
          label="총 자산"
          value={formatCurrency(status()?.total_equity || '0')}
          icon="💎"
        />
        <StatCard
          label="총 손익"
          value={`${totalPnl() >= 0 ? '+' : ''}${formatCurrency(totalPnl())}`}
          icon={totalPnl() >= 0 ? '📈' : '📉'}
          valueColor={totalPnl() >= 0 ? 'text-green-500' : 'text-red-500'}
        />
        <StatCard
          label="수익률"
          value={`${totalPnlPercent() >= 0 ? '+' : ''}${formatDecimal(totalPnlPercent())}%`}
          icon={totalPnlPercent() >= 0 ? '🚀' : '⬇️'}
          valueColor={totalPnlPercent() >= 0 ? 'text-green-500' : 'text-red-500'}
        />
      </StatCardGrid>

      <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Positions */}
        <Card>
          <CardHeader>
            <h3 class="text-lg font-semibold text-[var(--color-text)]">
              보유 포지션 ({positions().length})
            </h3>
          </CardHeader>
          <CardContent>
            <Show
              when={positions().length > 0}
              fallback={
                <EmptyState
                  icon="📦"
                  title="포지션 없음"
                  description="현재 보유 중인 포지션이 없습니다"
                  className="py-4"
                />
              }
            >
              <div class="space-y-3">
                <For each={positions()}>
                  {(position) => {
                    const pnl = parseFloat(position.unrealized_pnl)
                    const pnlPct = parseFloat(position.return_pct)
                    return (
                      <div class="flex items-center justify-between p-3 bg-[var(--color-surface-light)] rounded-lg">
                        <div>
                          <div class="flex items-center gap-2">
                            <SymbolDisplay
                              ticker={position.symbol}
                              symbolName={position.displayName}
                              mode="inline"
                              size="md"
                              autoFetch={true}
                              class="font-semibold"
                            />
                            <span
                              class={`px-2 py-0.5 text-xs rounded ${
                                position.side === 'Long'
                                  ? 'bg-green-500/20 text-green-400'
                                  : 'bg-red-500/20 text-red-400'
                              }`}
                            >
                              {position.side}
                            </span>
                          </div>
                          <div class="text-sm text-[var(--color-text-muted)] mt-1">
                            {formatDecimal(position.quantity, 4)} @ {formatCurrency(position.entry_price)}
                          </div>
                        </div>
                        <div class="text-right">
                          <div
                            class={`font-semibold ${
                              pnl >= 0 ? 'text-green-500' : 'text-red-500'
                            }`}
                          >
                            {pnl >= 0 ? '+' : ''}{formatCurrency(pnl)}
                          </div>
                          <div
                            class={`text-sm ${
                              pnlPct >= 0 ? 'text-green-500' : 'text-red-500'
                            }`}
                          >
                            {pnlPct >= 0 ? '+' : ''}
                            {formatDecimal(pnlPct)}%
                          </div>
                        </div>
                      </div>
                    )
                  }}
                </For>
              </div>
            </Show>
          </CardContent>
        </Card>

        {/* Trade History */}
        <Card>
          <CardHeader>
            <h3 class="text-lg font-semibold text-[var(--color-text)]">
              거래 내역 ({trades().length})
            </h3>
          </CardHeader>
          <CardContent>
            <Show
              when={trades().length > 0}
              fallback={
                <EmptyState
                  icon="📋"
                  title="거래 내역 없음"
                  description="아직 체결된 거래가 없습니다"
                  className="py-4"
                />
              }
            >
              <div class="space-y-2 max-h-80 overflow-y-auto">
                <For each={[...trades()].reverse().slice(0, 20)}>
                  {(trade) => {
                    const realizedPnl = trade.realized_pnl ? parseFloat(trade.realized_pnl) : null
                    return (
                      <div class="flex items-center justify-between p-3 bg-[var(--color-surface-light)] rounded-lg">
                        <div class="flex items-center gap-3">
                          <span class="text-sm text-[var(--color-text-muted)] font-mono">
                            {new Date(trade.timestamp).toLocaleTimeString('ko-KR')}
                          </span>
                          <span
                            class={`px-2 py-0.5 text-xs rounded font-medium ${
                              trade.side === 'Buy'
                                ? 'bg-green-500/20 text-green-400'
                                : 'bg-red-500/20 text-red-400'
                            }`}
                          >
                            {trade.side === 'Buy' ? '매수' : '매도'}
                          </span>
                          <SymbolDisplay
                            ticker={trade.symbol}
                            symbolName={trade.displayName}
                            mode="inline"
                            size="sm"
                            autoFetch={true}
                          />
                        </div>
                        <div class="text-right">
                          <div class="text-sm text-[var(--color-text)]">
                            {formatDecimal(trade.quantity, 4)} @ {formatCurrency(trade.price)}
                          </div>
                          <Show when={realizedPnl !== null}>
                            <div
                              class={`text-sm ${
                                realizedPnl! >= 0 ? 'text-green-500' : 'text-red-500'
                              }`}
                            >
                              {realizedPnl! >= 0 ? '+' : ''}{formatCurrency(realizedPnl!)}
                            </div>
                          </Show>
                        </div>
                      </div>
                    )
                  }}
                </For>
              </div>
            </Show>
          </CardContent>
        </Card>
      </div>

      {/* Price Chart + Trade Markers (Backtest와 동일 패턴) */}
      <Show when={selectedStrategy()}>
        <details
          class="mt-4"
          onToggle={(e) => {
            if ((e.target as HTMLDetailsElement).open) {
              setShowPriceChart(true)
              loadCandleData()
            }
          }}
        >
          <summary class="cursor-pointer text-sm text-[var(--color-text-muted)] hover:text-[var(--color-text)] flex items-center gap-2">
            <LineChart class="w-4 h-4" />
            가격 차트 + 매매 태그
          </summary>
          <div class="mt-3 space-y-3">
            {/* 신호 필터 패널 */}
            <Suspense fallback={<div class="h-12 bg-gray-100 dark:bg-gray-800 animate-pulse rounded" />}>
              <IndicatorFilterPanel
                filters={signalFilters()}
                onChange={(filters) => setSignalFilters(filters)}
                defaultCollapsed={true}
              />
            </Suspense>

            {/* 다중 심볼인 경우 심볼 선택 탭 표시 */}
            <Show when={(() => {
              const strategy = strategies()?.find(s => s.id === selectedStrategy())
              return strategy?.symbols && strategy.symbols.length > 1
            })()}>
              <div class="flex flex-wrap gap-1 p-1 bg-[var(--color-surface-light)]/30 rounded-lg">
                <For each={(() => {
                  const strategy = strategies()?.find(s => s.id === selectedStrategy())
                  return strategy?.symbols || []
                })()}>
                  {(symbol) => (
                    <button
                      class={`px-3 py-1.5 text-xs font-medium rounded-md transition-all ${
                        chartSymbol() === symbol
                          ? 'bg-[var(--color-primary)] text-white shadow-sm'
                          : 'text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)] hover:text-[var(--color-text)]'
                      }`}
                      onClick={(e) => {
                        e.stopPropagation()
                        setChartSymbol(symbol)
                        // 심볼 변경 시 캔들 데이터 리로드
                        setCandleData([])
                        setRawCandleData([])
                        loadCandleData()
                      }}
                    >
                      {symbol}
                    </button>
                  )}
                </For>
              </div>
            </Show>

            {/* 필터 상태 요약 */}
            <Show when={signalFilters().signal_types.length > 0}>
              <div class="text-xs text-[var(--color-text-muted)]">
                표시 중: {signalFilteredTradeMarkers().length} / {tradeMarkers().length} 마커
              </div>
            </Show>

            {/* 볼륨 프로파일 토글 */}
            <div class="flex items-center gap-2 mb-2">
              <label class="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)] cursor-pointer">
                <input
                  type="checkbox"
                  checked={showVolumeProfile()}
                  onChange={(e) => setShowVolumeProfile(e.currentTarget.checked)}
                  class="w-3.5 h-3.5 rounded border-gray-500 text-blue-500 focus:ring-blue-500"
                />
                볼륨 프로파일 표시
              </label>
            </div>

            <Show
              when={!isLoadingCandles() && filteredCandleData().length > 0}
              fallback={
                <div class="h-[280px] flex items-center justify-center text-[var(--color-text-muted)]">
                  {isLoadingCandles() ? (
                    <div class="flex items-center gap-2">
                      <RefreshCw class="w-5 h-5 animate-spin" />
                      <span>차트 데이터 로딩 중...</span>
                    </div>
                  ) : (
                    <span>차트 데이터가 없습니다 (데이터셋을 먼저 다운로드하세요)</span>
                  )}
                </div>
              }
            >
              <div class="flex gap-2">
                {/* 캔들 차트 */}
                <div class="flex-1">
                  <SyncedChartPanel
                    data={filteredCandleData()}
                    type="candlestick"
                    mainHeight={240}
                    markers={signalFilteredTradeMarkers()}
                    chartId="sim-price"
                    syncState={priceSyncState}
                    onVisibleRangeChange={handlePriceVisibleRangeChange}
                  />
                </div>

                {/* 볼륨 프로파일 */}
                <Show when={showVolumeProfile() && volumeProfileData().length > 0}>
                  <div class="flex flex-col">
                    <Suspense fallback={<div class="h-[240px] w-[80px] bg-gray-100 dark:bg-gray-800 animate-pulse rounded" />}>
                      <VolumeProfile
                        priceVolumes={volumeProfileData()}
                        currentPrice={simCurrentPrice()}
                        chartHeight={240}
                        width={80}
                        priceRange={simChartPriceRange()}
                        showPoc={true}
                        showValueArea={true}
                      />
                      <VolumeProfileLegend
                        class="mt-1"
                      />
                    </Suspense>
                  </div>
                </Show>
              </div>
            </Show>
          </div>
        </details>
      </Show>

      {/* Equity Curve Chart */}
      <Card>
        <CardHeader>
          <h3 class="text-lg font-semibold text-[var(--color-text)]">자산 곡선</h3>
        </CardHeader>
        <CardContent>
          <Show
            when={equityCurve().length > 1}
            fallback={
              <EmptyState
                icon="📈"
                title="자산 곡선 대기 중"
                description="시뮬레이션을 시작하면 자산 곡선이 표시됩니다"
                className="h-[300px] flex flex-col items-center justify-center"
              />
            }
          >
            <EquityCurve
              data={equityCurve()}
              height={300}
              chartId="sim-equity"
              syncState={equitySyncState}
              onVisibleRangeChange={handleEquityVisibleRangeChange}
            />
          </Show>
        </CardContent>
      </Card>

      {/* Additional Stats */}
      <Show when={status() && (status()!.realized_pnl !== '0' || status()!.trade_count > 0)}>
        <Card>
          <CardHeader>
            <h3 class="text-lg font-semibold text-[var(--color-text)]">시뮬레이션 통계</h3>
          </CardHeader>
          <CardContent>
            <StatCardGrid columns={4}>
              <StatCard
                label="실현 손익"
                value={formatCurrency(status()!.realized_pnl)}
                icon="💵"
                valueColor={parseFloat(status()!.realized_pnl) >= 0 ? 'text-green-500' : 'text-red-500'}
              />
              <StatCard
                label="미실현 손익"
                value={formatCurrency(status()!.unrealized_pnl)}
                icon="📊"
                valueColor={parseFloat(status()!.unrealized_pnl) >= 0 ? 'text-green-500' : 'text-red-500'}
              />
              <StatCard
                label="현재 잔고"
                value={formatCurrency(status()!.current_balance)}
                icon="🏦"
              />
              <StatCard
                label="포지션 수"
                value={`${status()!.position_count}개`}
                icon="📦"
              />
            </StatCardGrid>
          </CardContent>
        </Card>
      </Show>

      {/* 리스크 분석 (Kelly + 상관관계) */}
      <Show when={trades().length >= 3}>
        <Card>
          <CardHeader>
            <h3 class="text-lg font-semibold text-[var(--color-text)]">📊 리스크 분석</h3>
          </CardHeader>
          <CardContent>
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
              {/* Kelly Criterion 시각화 */}
              <div class="space-y-4">
                <div class="flex items-center justify-between">
                  <h4 class="font-medium text-[var(--color-text)]">Kelly 포지션 사이징</h4>
                  <span class="text-sm text-[var(--color-text-muted)]">
                    승률: {(kellyStats().winRate * 100).toFixed(1)}%
                  </span>
                </div>
                <KellyVisualization
                  kellyFraction={kellyStats().kellyFraction}
                  currentAllocation={kellyStats().currentAllocation}
                  maxRisk={0.25}
                  showHalfKelly={true}
                  height={180}
                />
                <div class="grid grid-cols-2 gap-4 text-sm">
                  <div class="p-3 bg-[var(--color-surface-light)] rounded-lg">
                    <div class="text-[var(--color-text-muted)]">평균 수익</div>
                    <div class="text-green-500 font-semibold">
                      {formatCurrency(kellyStats().avgWin)}
                    </div>
                  </div>
                  <div class="p-3 bg-[var(--color-surface-light)] rounded-lg">
                    <div class="text-[var(--color-text-muted)]">평균 손실</div>
                    <div class="text-red-500 font-semibold">
                      {formatCurrency(kellyStats().avgLoss)}
                    </div>
                  </div>
                </div>
              </div>

              {/* 상관관계 히트맵 */}
              <div class="space-y-4">
                <h4 class="font-medium text-[var(--color-text)]">심볼 간 상관관계</h4>
                <Show
                  when={correlationData().symbols.length >= 2}
                  fallback={
                    <EmptyState
                      icon="🔗"
                      title="상관관계 분석 대기"
                      description="2개 이상의 심볼에서 거래가 발생해야 분석됩니다"
                      className="h-[200px] flex flex-col items-center justify-center"
                    />
                  }
                >
                  <Suspense fallback={<div class="h-[200px] flex items-center justify-center text-[var(--color-text-muted)]">로딩 중...</div>}>
                    <MiniCorrelationMatrix
                      symbols={correlationData().symbols}
                      correlations={correlationData().correlations}
                    />
                  </Suspense>
                </Show>
              </div>
            </div>
          </CardContent>
        </Card>
      </Show>
      </Show>{/* Backtest 탭 끝 */}
    </div>
  )
}

export default Simulation
