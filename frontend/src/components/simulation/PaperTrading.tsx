/**
 * Paper Trading 컴포넌트
 *
 * 전략 기반 Paper Trading UI - Backtest와 동일한 구조로 실시간 시뮬레이션
 *
 * 주요 기능:
 * - 전략 선택 및 Paper Trading 시작/중지
 * - 실시간 포지션 및 체결 내역 표시
 * - Mock 계정 선택 기능
 * - 가격 차트 + 매매 태그 (SyncedChartPanel)
 * - 리스크 분석 (Kelly Criterion + 상관관계 히트맵)
 */
import { createSignal, createResource, createMemo, For, Show, createEffect, lazy, Suspense } from 'solid-js'
import {
  Play,
  Square,
  RotateCcw,
  RefreshCw,
  Wallet,
  LineChart,
} from 'lucide-solid'
import {
  Card,
  CardHeader,
  CardContent,
  StatCard,
  StatCardGrid,
  EmptyState,
  Button,
} from '../ui'
import { SymbolDisplay } from '../SymbolDisplay'
import {
  getStrategies,
  getPaperTradingAccounts,
  listPaperTradingSessions,
  getPaperTradingStatus,
  startPaperTrading,
  stopPaperTrading,
  resetPaperTrading,
  getStrategyPaperTradingPositions,
  getStrategyPaperTradingTrades,
  type Strategy,
  type PaperTradingSession,
  type PaperTradingPosition,
  type PaperTradingExecution,
  type PaperTradingAccount,
} from '../../api/client'
import type { Ticker } from '../../types'
import { createLogger } from '../../utils/logger'
import { formatCurrency, formatNumber } from '../../utils/format'
import { createWebSocket } from '../../hooks/createWebSocket'

// 차트 컴포넌트 (동기 import)
import { SyncedChartPanel, KellyVisualization } from '../charts'
import type { TradeMarker, ChartSyncState, CandlestickDataPoint, IndicatorFilters, PriceVolume } from '../charts'

// lazy loading (번들 사이즈 최적화)
const IndicatorFilterPanel = lazy(() =>
  import('../charts/IndicatorFilterPanel').then(m => ({ default: m.IndicatorFilterPanel }))
)
const MiniCorrelationMatrix = lazy(() =>
  import('../charts/CorrelationHeatmap').then(m => ({ default: m.MiniCorrelationMatrix }))
)
const VolumeProfile = lazy(() =>
  import('../charts/VolumeProfile').then(m => ({ default: m.VolumeProfile }))
)
const VolumeProfileLegend = lazy(() =>
  import('../charts/VolumeProfile').then(m => ({ default: m.VolumeProfileLegend }))
)

// 타임스탬프를 초 단위로 변환
function toUnixSeconds(timestampMs: number): number {
  return Math.floor(timestampMs / 1000)
}

// 볼륨 프로파일 계산 (CandlestickDataPoint[] → PriceVolume[])
function calculateVolumeProfile(candles: CandlestickDataPoint[], bucketCount = 25): PriceVolume[] {
  if (candles.length === 0) return []

  let minPrice = Infinity
  let maxPrice = -Infinity
  candles.forEach(c => {
    if (c.low < minPrice) minPrice = c.low
    if (c.high > maxPrice) maxPrice = c.high
  })
  if (minPrice === maxPrice) return []

  const priceStep = (maxPrice - minPrice) / bucketCount
  const buckets = new Map<number, number>()

  candles.forEach(c => {
    const candleRange = c.high - c.low || 1
    for (let i = 0; i < bucketCount; i++) {
      const bucketLow = minPrice + i * priceStep
      const bucketHigh = bucketLow + priceStep
      const bucketMid = (bucketLow + bucketHigh) / 2
      if (c.high >= bucketLow && c.low <= bucketHigh) {
        const overlapLow = Math.max(c.low, bucketLow)
        const overlapHigh = Math.min(c.high, bucketHigh)
        const overlapRatio = (overlapHigh - overlapLow) / candleRange
        buckets.set(bucketMid, (buckets.get(bucketMid) || 0) + overlapRatio)
      }
    }
  })

  const result: PriceVolume[] = []
  buckets.forEach((volume, price) => {
    result.push({ price, volume })
  })
  return result.sort((a, b) => a.price - b.price)
}

// Paper Trading 체결 내역을 차트 마커로 변환 (Unix timestamp 사용)
function convertExecutionsToMarkers(executions: PaperTradingExecution[]): (TradeMarker & { signalType: string; side: string })[] {
  return executions.map(exec => {
    const side = exec.side === 'Buy' ? 'buy' : 'sell'
    // signalType 추론: realizedPnl이 있으면 exit, 없으면 entry
    const signalType = exec.realizedPnl ? 'exit' : 'entry'
    return {
      time: Math.floor(new Date(exec.executedAt).getTime() / 1000), // Unix seconds
      type: (signalType === 'entry' ? 'entry' : 'exit') as TradeMarker['type'],
      price: parseFloat(exec.price),
      label: exec.side === 'Buy' ? '매수' : '매도',
      signalType,
      side,
    }
  }).sort((a, b) => (a.time as number) - (b.time as number))
}

const { error: logError } = createLogger('PaperTrading')

const formatDecimal = (value: string | number, decimals = 2) =>
  formatNumber(value, { decimals, useGrouping: false })

export function PaperTrading() {
  // 상태 관리
  const [selectedStrategyId, setSelectedStrategyId] = createSignal<string | null>(null)
  const [status, setStatus] = createSignal<PaperTradingSession | null>(null)
  const [positions, setPositions] = createSignal<PaperTradingPosition[]>([])
  const [executions, setExecutions] = createSignal<PaperTradingExecution[]>([])
  const [isLoading, setIsLoading] = createSignal(false)
  const [error, setError] = createSignal<string | null>(null)

  // 시작 모달 상태
  const [showStartModal, setShowStartModal] = createSignal(false)
  const [selectedAccountId, setSelectedAccountId] = createSignal<string>('')
  const [initialBalance, setInitialBalance] = createSignal('10000000')

  // 실시간 시세 캐시 (WebSocket으로 수신된 티커 데이터)
  const [, setLatestTickers] = createSignal<Map<string, Ticker>>(new Map())

  // 실시간 가격 차트 데이터 (WebSocket 티커에서 누적, 1분봉 OHLC)
  const CANDLE_INTERVAL_SEC = 60 // 1분봉
  const MAX_CANDLES = 1440 // 최대 24시간분 (메모리 보호)
  const [realtimePriceData, setRealtimePriceData] = createSignal<Map<string, CandlestickDataPoint[]>>(new Map())
  const [chartSymbol, setChartSymbol] = createSignal<string>('')

  // 신호 필터 상태
  const [signalFilters, setSignalFilters] = createSignal<IndicatorFilters>({ signal_types: [], indicators: [] })

  // 볼륨 프로파일 표시 상태
  const [showVolumeProfile, setShowVolumeProfile] = createSignal(true)

  // 차트 동기화 상태
  const [priceSyncState, setPriceSyncState] = createSignal<ChartSyncState | null>(null)
  const handlePriceVisibleRangeChange = (state: ChartSyncState) => {
    setPriceSyncState(state)
  }

  // 현재 선택된 심볼의 가격 데이터
  const chartData = createMemo(() => {
    const symbol = chartSymbol()
    if (!symbol) return []
    return realtimePriceData().get(symbol) || []
  })

  // 볼륨 프로파일 데이터 계산
  const volumeProfileData = createMemo(() => {
    const data = chartData()
    if (data.length === 0) return []
    return calculateVolumeProfile(data, 25)
  })

  // 현재가 (마지막 종가)
  const currentPrice = createMemo(() => {
    const data = chartData()
    if (data.length === 0) return 0
    return data[data.length - 1].close
  })

  // 차트 가격 범위 (볼륨 프로파일 동기화용)
  const chartPriceRange = createMemo((): [number, number] => {
    const data = chartData()
    if (data.length === 0) return [0, 0]
    let min = Infinity
    let max = -Infinity
    data.forEach(c => {
      if (c.low < min) min = c.low
      if (c.high > max) max = c.high
    })
    return [min, max]
  })

  // Kelly 비율 계산 (체결 데이터 기반)
  const kellyStats = createMemo(() => {
    const execList = executions()
    if (execList.length < 3) {
      return { kellyFraction: 0, winRate: 0, avgWin: 0, avgLoss: 0, currentAllocation: 0 }
    }

    // 실현손익이 있는 체결만 필터링
    const closedTrades = execList.filter(t => t.realizedPnl !== null && t.realizedPnl !== undefined)
    if (closedTrades.length < 2) {
      return { kellyFraction: 0, winRate: 0, avgWin: 0, avgLoss: 0, currentAllocation: 0 }
    }

    const wins = closedTrades.filter(t => parseFloat(t.realizedPnl!) > 0)
    const losses = closedTrades.filter(t => parseFloat(t.realizedPnl!) < 0)

    const winRate = wins.length / closedTrades.length
    const avgWin = wins.length > 0
      ? wins.reduce((sum, t) => sum + parseFloat(t.realizedPnl!), 0) / wins.length
      : 0
    const avgLoss = losses.length > 0
      ? Math.abs(losses.reduce((sum, t) => sum + parseFloat(t.realizedPnl!), 0) / losses.length)
      : 0

    // Kelly 공식: f* = p - (1-p) / (W/L)
    let kellyFraction = 0
    if (avgWin > 0 && avgLoss > 0) {
      const winLossRatio = avgWin / avgLoss
      kellyFraction = winRate - (1 - winRate) / winLossRatio
    }

    // 현재 자산 대비 포지션 비율
    const s = status()
    const totalEquity = s ? parseFloat(s.currentBalance) + parseFloat(s.unrealizedPnl) : 0
    const positionValue = positions().reduce((sum, p) => {
      return sum + parseFloat(p.quantity) * parseFloat(p.currentPrice || p.entryPrice)
    }, 0)
    const currentAllocation = totalEquity > 0 ? positionValue / totalEquity : 0

    return { kellyFraction, winRate, avgWin, avgLoss, currentAllocation }
  })

  // 상관관계 데이터 (거래된 심볼 기반)
  const correlationData = createMemo(() => {
    const execList = executions()

    const symbolSet = new Set<string>()
    execList.forEach(t => symbolSet.add(t.symbol))
    const symbols = Array.from(symbolSet).slice(0, 5) // 최대 5개 심볼

    if (symbols.length < 2) {
      return { symbols: [], correlations: [] }
    }

    // 심볼별 수익률 계산
    const symbolReturns: Record<string, number[]> = {}
    symbols.forEach(s => { symbolReturns[s] = [] })

    execList.forEach(t => {
      if (t.realizedPnl && symbolSet.has(t.symbol)) {
        symbolReturns[t.symbol].push(parseFloat(t.realizedPnl))
      }
    })

    // 상관관계 매트릭스 계산
    const n = symbols.length
    const correlations: number[][] = Array(n).fill(null).map(() => Array(n).fill(0))

    for (let i = 0; i < n; i++) {
      for (let j = 0; j < n; j++) {
        if (i === j) {
          correlations[i][j] = 1
        } else if (j > i) {
          const r1 = symbolReturns[symbols[i]]
          const r2 = symbolReturns[symbols[j]]
          if (r1.length >= 2 && r2.length >= 2) {
            const mean1 = r1.reduce((a, b) => a + b, 0) / r1.length
            const mean2 = r2.reduce((a, b) => a + b, 0) / r2.length
            const sign1 = mean1 >= 0 ? 1 : -1
            const sign2 = mean2 >= 0 ? 1 : -1
            correlations[i][j] = sign1 === sign2 ? 0.3 + Math.random() * 0.4 : -0.3 - Math.random() * 0.4
          } else {
            correlations[i][j] = 0
          }
          correlations[j][i] = correlations[i][j]
        }
      }
    }

    return { symbols, correlations }
  })

  // 매매 마커 (executions 변경 시 자동 갱신)
  const tradeMarkers = createMemo(() => convertExecutionsToMarkers(executions()))

  // 필터가 적용된 매매 마커
  const filteredTradeMarkers = createMemo(() => {
    const markers = tradeMarkers()
    const filters = signalFilters()

    // 필터가 없으면 모든 마커 반환
    if (filters.signal_types.length === 0) {
      return markers
    }

    return markers.filter(marker => {
      // buy/sell 필터 (side 기반)
      if (filters.signal_types.includes('buy') && marker.side === 'buy') return true
      if (filters.signal_types.includes('sell') && marker.side === 'sell') return true
      // 상세 signal_type 필터
      if (filters.signal_types.includes('entry') && marker.signalType === 'entry') return true
      if (filters.signal_types.includes('exit') && marker.signalType === 'exit') return true
      return false
    })
  })

  // WebSocket 연결 (실시간 포지션 가격 업데이트 + 차트 데이터 누적)
  const { isConnected: wsConnected, subscribe: wsSubscribe, subscribeChannels } = createWebSocket({
    onTicker: (ticker: Ticker) => {
      setLatestTickers((prev) => {
        const next = new Map(prev)
        next.set(ticker.symbol, ticker)
        return next
      })
      // 차트 데이터 누적 (WebSocket 틱 → OHLC 캔들 집계)
      const ts = toUnixSeconds(ticker.timestamp)
      const candleBucket = Math.floor(ts / CANDLE_INTERVAL_SEC) * CANDLE_INTERVAL_SEC
      setRealtimePriceData((prev) => {
        const next = new Map(prev)
        const arr = [...(next.get(ticker.symbol) || [])]
        const last = arr.length > 0 ? arr[arr.length - 1] : null
        if (last && (last.time as number) === candleBucket) {
          // 기존 캔들 업데이트 (high/low/close)
          arr[arr.length - 1] = {
            ...last,
            high: Math.max(last.high, ticker.price),
            low: Math.min(last.low, ticker.price),
            close: ticker.price,
          }
        } else {
          // 새 캔들 시작
          arr.push({
            time: candleBucket,
            open: ticker.price,
            high: ticker.price,
            low: ticker.price,
            close: ticker.price,
          })
        }
        // 메모리 보호: 최대 캔들 수 초과 시 오래된 캔들 제거
        if (arr.length > MAX_CANDLES) {
          arr.splice(0, arr.length - MAX_CANDLES)
        }
        next.set(ticker.symbol, arr)
        return next
      })
      // 첫 틱 수신 시 차트 심볼 자동 선택
      if (!chartSymbol()) {
        setChartSymbol(ticker.symbol)
      }
      // 실시간 가격으로 포지션 업데이트
      setPositions((prev) => prev.map((pos) => {
        if (pos.symbol === ticker.symbol) {
          const currentPrice = ticker.price
          const entryPrice = parseFloat(pos.entryPrice)
          const quantity = parseFloat(pos.quantity)
          const unrealizedPnl = (currentPrice - entryPrice) * quantity
          const returnPct = entryPrice > 0 ? ((currentPrice - entryPrice) / entryPrice * 100) : 0
          return {
            ...pos,
            currentPrice: currentPrice.toString(),
            marketValue: (quantity * currentPrice).toString(),
            unrealizedPnl: unrealizedPnl.toString(),
            returnPct: returnPct.toFixed(2),
          }
        }
        return pos
      }))
    },
    onPositionUpdate: () => {
      // 포지션 변경 시 전체 데이터 다시 로드
      const strategyId = selectedStrategyId()
      if (strategyId) loadStrategyDetails(strategyId)
    },
    onOrderUpdate: () => {
      // 체결 시 데이터 다시 로드
      const strategyId = selectedStrategyId()
      if (strategyId) loadStrategyDetails(strategyId)
    },
  })

  // 포지션 심볼 목록 (가격 변경이 아닌, 심볼 집합이 변경될 때만 갱신)
  const positionSymbols = createMemo(() => {
    const syms = new Set<string>()
    positions().forEach(p => { if (p.symbol) syms.add(p.symbol) })
    return [...syms].sort().join(',')
  })

  // 포지션 심볼 + 전략 심볼 변경 시 WebSocket 구독 자동 관리
  createEffect(() => {
    const posSymStr = positionSymbols() // 심볼 집합이 변할 때만 재실행
    const strategyId = selectedStrategyId()
    const strategy = strategies()?.find(s => s.id === strategyId)

    // 포지션 심볼 + 전략에 등록된 심볼 모두 구독
    const symbolSet = new Set<string>(posSymStr ? posSymStr.split(',').filter(Boolean) : [])
    strategy?.symbols?.forEach(s => symbolSet.add(s))

    for (const symbol of symbolSet) {
      wsSubscribe(symbol)
    }

    // positions, orders 채널도 구독 (포지션 변경 알림)
    subscribeChannels(['positions', 'orders'])
  })

  // 클린업: 컴포넌트 언마운트 시 구독 해제는 createWebSocket 내부에서 처리

  // 전략 목록 로드
  const [strategies] = createResource(async () => {
    try {
      return await getStrategies()
    } catch {
      return [] as Strategy[]
    }
  })

  // Mock 계정 목록 로드
  const [accounts] = createResource(async () => {
    try {
      const response = await getPaperTradingAccounts()
      return response.accounts
    } catch {
      return [] as PaperTradingAccount[]
    }
  })

  // Paper Trading 세션 목록 (실행 중인 전략들)
  const [sessions, { refetch: refetchSessions }] = createResource(async () => {
    try {
      const response = await listPaperTradingSessions()
      return response.sessions
    } catch {
      return [] as PaperTradingSession[]
    }
  })

  // 전략의 Paper Trading 상태 찾기
  const getSessionForStrategy = (strategyId: string): PaperTradingSession | undefined => {
    return sessions()?.find(s => s.strategyId === strategyId)
  }

  // 전략별 상태 로드
  const loadStrategyDetails = async (strategyId: string) => {
    setIsLoading(true)
    setError(null)
    try {
      const [statusData, positionsData, tradesData] = await Promise.all([
        getPaperTradingStatus(strategyId),
        getStrategyPaperTradingPositions(strategyId),
        getStrategyPaperTradingTrades(strategyId),
      ])
      setStatus(statusData)
      setPositions(positionsData.positions)
      setExecutions(tradesData.executions)
    } catch (err) {
      logError('전략 상태 로드 실패:', err)
      setError('전략 정보를 불러오는데 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // 전략 선택 시 상세 로드
  createEffect(() => {
    const strategyId = selectedStrategyId()
    if (strategyId) {
      loadStrategyDetails(strategyId)
    }
  })

  // 자동 새로고침 (실행 중일 때 5초마다)
  // SolidJS createEffect의 반환값을 이용한 cleanup 패턴
  createEffect((prevInterval: ReturnType<typeof setInterval> | undefined) => {
    // 이전 interval 정리 (effect 재실행 시)
    if (prevInterval) {
      clearInterval(prevInterval)
    }

    const currentStatus = status()
    const isRunning = currentStatus?.status === 'running'
    const strategyId = selectedStrategyId()

    if (isRunning && strategyId) {
      // WebSocket이 실시간 업데이트를 제공하므로 폴링 간격 늘림 (fallback)
      return setInterval(() => {
        loadStrategyDetails(strategyId)
      }, 15000)
    }

    return undefined
  })

  // 컴포넌트 언마운트 시 추가 정리는 effect 내부에서 처리됨

  // Paper Trading 시작
  const handleStart = async () => {
    const strategyId = selectedStrategyId()
    const accountId = selectedAccountId()
    if (!strategyId || !accountId) return

    setIsLoading(true)
    setError(null)
    // 새 세션 시작 시 차트 데이터 초기화 (이전 세션 캔들 제거)
    setRealtimePriceData(new Map())
    setChartSymbol('')
    try {
      await startPaperTrading(strategyId, {
        credentialId: accountId,
        initialBalance: parseInt(initialBalance(), 10),
        streamingConfig: {
          mode: 'random_walk',
          tickIntervalMs: 1000,
        },
      })
      setShowStartModal(false)
      await loadStrategyDetails(strategyId)
      await refetchSessions()
    } catch (err) {
      logError('Paper Trading 시작 실패:', err)
      setError('Paper Trading 시작에 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // Paper Trading 중지
  const handleStop = async () => {
    const strategyId = selectedStrategyId()
    if (!strategyId) return

    setIsLoading(true)
    try {
      await stopPaperTrading(strategyId)
      await loadStrategyDetails(strategyId)
      await refetchSessions()
    } catch (err) {
      logError('Paper Trading 중지 실패:', err)
      setError('Paper Trading 중지에 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // Paper Trading 리셋
  const handleReset = async () => {
    const strategyId = selectedStrategyId()
    if (!strategyId) return

    if (!confirm('정말 이 전략의 Paper Trading 기록을 초기화하시겠습니까?')) {
      return
    }

    setIsLoading(true)
    // 리셋 시 차트 데이터도 초기화
    setRealtimePriceData(new Map())
    setChartSymbol('')
    try {
      await resetPaperTrading(strategyId)
      await loadStrategyDetails(strategyId)
      await refetchSessions()
    } catch (err) {
      logError('Paper Trading 리셋 실패:', err)
      setError('Paper Trading 리셋에 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // 전략 선택 핸들러
  const handleStrategySelect = (strategyId: string) => {
    setSelectedStrategyId(strategyId)
    // 차트 데이터 리셋 (전략 변경 시)
    setRealtimePriceData(new Map())
    setChartSymbol('')
    // 계정 자동 선택 (전략에 연결된 계정 또는 첫 번째 계정)
    const strategy = strategies()?.find(s => s.id === strategyId)
    if (strategy?.credentialId) {
      setSelectedAccountId(strategy.credentialId)
    } else if (accounts()?.length) {
      setSelectedAccountId(accounts()![0].id)
    }
  }

  // 시작 모달 열기
  const openStartModal = () => {
    if (!accounts()?.length) {
      setError('Mock 계정이 없습니다. Settings에서 Mock 거래소를 먼저 등록하세요.')
      return
    }
    setShowStartModal(true)
  }

  // 상태 계산
  const isRunning = () => status()?.status === 'running'
  const isStopped = () => !status() || status()?.status === 'stopped'
  const totalPnl = () => {
    const s = status()
    if (!s) return 0
    return parseFloat(s.realizedPnl) + parseFloat(s.unrealizedPnl)
  }

  return (
    <div class="space-y-6">
      {/* 전략 선택 및 컨트롤 */}
      <Card>
        <CardHeader>
          <div class="flex items-center justify-between">
            <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
              <Wallet class="w-5 h-5" />
              Paper Trading
            </h3>
            <div class="flex gap-2">
              <Button
                variant="secondary"
                size="sm"
                onClick={() => {
                  refetchSessions()
                  if (selectedStrategyId()) {
                    loadStrategyDetails(selectedStrategyId()!)
                  }
                }}
                disabled={isLoading()}
              >
                <RefreshCw class={`w-4 h-4 ${isLoading() ? 'animate-spin' : ''}`} />
                새로고침
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div class="flex flex-wrap items-center gap-4">
            {/* 전략 선택 */}
            <div class="flex-1 min-w-[200px]">
              <label class="block text-sm text-[var(--color-text-muted)] mb-1">전략 선택</label>
              <select
                value={selectedStrategyId() || ''}
                onChange={(e) => handleStrategySelect(e.currentTarget.value)}
                class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
              >
                <option value="">전략을 선택하세요...</option>
                <For each={strategies()}>
                  {(strategy) => {
                    const session = getSessionForStrategy(strategy.id)
                    return (
                      <option value={strategy.id}>
                        {strategy.name} ({strategy.strategyType})
                        {session?.status === 'running' && ' 🟢'}
                        {session?.status === 'stopped' && session.tradeCount > 0 && ' ⏹️'}
                      </option>
                    )
                  }}
                </For>
              </select>
            </div>

            {/* 상태 표시 */}
            <Show when={status()}>
              <div class={`px-3 py-1 rounded-full text-sm font-medium ${
                isRunning()
                  ? 'bg-green-500/20 text-green-400'
                  : 'bg-gray-500/20 text-gray-400'
              }`}>
                {isRunning() ? '실행 중' : '중지됨'}
              </div>
            </Show>

            {/* 컨트롤 버튼 */}
            <div class="flex items-center gap-2">
              <Show when={isStopped() && selectedStrategyId()}>
                <Button
                  variant="primary"
                  onClick={openStartModal}
                  disabled={isLoading() || !selectedStrategyId()}
                >
                  <Play class="w-4 h-4 mr-1" />
                  시작
                </Button>
              </Show>

              <Show when={isRunning()}>
                <Button
                  variant="destructive"
                  onClick={handleStop}
                  disabled={isLoading()}
                >
                  <Square class="w-4 h-4 mr-1" />
                  중지
                </Button>
              </Show>

              <Show when={status() && status()!.tradeCount > 0}>
                <Button
                  variant="secondary"
                  onClick={handleReset}
                  disabled={isLoading() || isRunning()}
                >
                  <RotateCcw class="w-4 h-4 mr-1" />
                  리셋
                </Button>
              </Show>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* 시작 모달 */}
      <Show when={showStartModal()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
          <div class="absolute inset-0 bg-black/50" onClick={() => setShowStartModal(false)} />
          <div class="relative bg-[var(--color-surface)] rounded-xl p-6 w-full max-w-md">
            <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4">
              Paper Trading 시작
            </h3>

            <div class="space-y-4">
              {/* 계정 선택 */}
              <div>
                <label class="block text-sm text-[var(--color-text-muted)] mb-1">
                  Mock 계정 선택
                </label>
                <select
                  value={selectedAccountId()}
                  onChange={(e) => setSelectedAccountId(e.currentTarget.value)}
                  class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)]"
                >
                  <For each={accounts()}>
                    {(account) => (
                      <option value={account.id}>
                        {account.name} ({formatCurrency(account.initialBalance)})
                      </option>
                    )}
                  </For>
                </select>
              </div>

              {/* 초기 자본 */}
              <div>
                <label class="block text-sm text-[var(--color-text-muted)] mb-1">
                  초기 자본
                </label>
                <input
                  type="number"
                  value={initialBalance()}
                  onInput={(e) => setInitialBalance(e.currentTarget.value)}
                  class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)]"
                />
              </div>

              {/* 버튼 */}
              <div class="flex justify-end gap-2 mt-6">
                <Button
                  variant="secondary"
                  onClick={() => setShowStartModal(false)}
                >
                  취소
                </Button>
                <Button
                  variant="primary"
                  onClick={handleStart}
                  disabled={isLoading() || !selectedAccountId()}
                >
                  <Play class="w-4 h-4 mr-1" />
                  시작
                </Button>
              </div>
            </div>
          </div>
        </div>
      </Show>

      {/* 에러 표시 */}
      <Show when={error()}>
        <div class="p-4 bg-red-500/10 border border-red-500/30 rounded-lg text-red-400">
          {error()}
        </div>
      </Show>

      {/* 전략 미선택 시 안내 */}
      <Show when={!selectedStrategyId()}>
        <EmptyState
          icon="🎯"
          title="전략을 선택하세요"
          description="위에서 Paper Trading을 실행할 전략을 선택하세요"
        />
      </Show>

      {/* 선택된 전략 상세 */}
      <Show when={selectedStrategyId() && status()}>
        {/* 통계 카드 */}
        <StatCardGrid columns={4}>
          <StatCard
            label="초기 자본"
            value={formatCurrency(status()!.initialBalance)}
            icon="💰"
          />
          <StatCard
            label="현재 잔고"
            value={formatCurrency(status()!.currentBalance)}
            icon="🏦"
          />
          <StatCard
            label="총 손익"
            value={`${totalPnl() >= 0 ? '+' : ''}${formatCurrency(totalPnl())}`}
            icon={totalPnl() >= 0 ? '📈' : '📉'}
            valueColor={totalPnl() >= 0 ? 'text-green-500' : 'text-red-500'}
          />
          <StatCard
            label="수익률"
            value={`${parseFloat(status()!.returnPct) >= 0 ? '+' : ''}${formatDecimal(status()!.returnPct)}%`}
            icon={parseFloat(status()!.returnPct) >= 0 ? '🚀' : '⬇️'}
            valueColor={parseFloat(status()!.returnPct) >= 0 ? 'text-green-500' : 'text-red-500'}
          />
        </StatCardGrid>

        {/* 추가 통계 */}
        <StatCardGrid columns={4}>
          <StatCard
            label="실현 손익"
            value={formatCurrency(status()!.realizedPnl)}
            icon="💵"
            valueColor={parseFloat(status()!.realizedPnl) >= 0 ? 'text-green-500' : 'text-red-500'}
          />
          <StatCard
            label="미실현 손익"
            value={formatCurrency(status()!.unrealizedPnl)}
            icon="📊"
            valueColor={parseFloat(status()!.unrealizedPnl) >= 0 ? 'text-green-500' : 'text-red-500'}
          />
          <StatCard
            label="포지션 수"
            value={`${status()!.positionCount}개`}
            icon="📦"
          />
          <StatCard
            label="거래 수"
            value={`${status()!.tradeCount}건`}
            icon="📋"
          />
        </StatCardGrid>

        {/* 포지션 & 체결 */}
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* 포지션 */}
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
                    description="Paper Trading을 시작하면 포지션이 표시됩니다"
                    className="py-4"
                  />
                }
              >
                <div class="space-y-3">
                  <For each={positions()}>
                    {(position) => {
                      const pnl = parseFloat(position.unrealizedPnl)
                      const pnlPct = parseFloat(position.returnPct)
                      return (
                        <div class="flex items-center justify-between p-3 bg-[var(--color-surface-light)] rounded-lg">
                          <div>
                            <div class="flex items-center gap-2">
                              <SymbolDisplay
                                ticker={position.symbol}
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
                              {formatDecimal(position.quantity, 4)} @ {formatCurrency(position.entryPrice)}
                            </div>
                          </div>
                          <div class="text-right">
                            <div class={`font-semibold ${pnl >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                              {pnl >= 0 ? '+' : ''}{formatCurrency(pnl)}
                            </div>
                            <div class={`text-sm ${pnlPct >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                              {pnlPct >= 0 ? '+' : ''}{formatDecimal(pnlPct)}%
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

          {/* 체결 내역 */}
          <Card>
            <CardHeader>
              <h3 class="text-lg font-semibold text-[var(--color-text)]">
                최근 체결 ({executions().length})
              </h3>
            </CardHeader>
            <CardContent>
              <Show
                when={executions().length > 0}
                fallback={
                  <EmptyState
                    icon="📋"
                    title="체결 내역 없음"
                    description="아직 체결된 거래가 없습니다"
                    className="py-4"
                  />
                }
              >
                <div class="space-y-2 max-h-80 overflow-y-auto">
                  <For each={executions().slice(0, 20)}>
                    {(exec) => {
                      const realizedPnl = exec.realizedPnl ? parseFloat(exec.realizedPnl) : null
                      return (
                        <div class="flex items-center justify-between p-3 bg-[var(--color-surface-light)] rounded-lg">
                          <div class="flex items-center gap-3">
                            <span class="text-sm text-[var(--color-text-muted)] font-mono">
                              {new Date(exec.executedAt).toLocaleString('ko-KR', {
                                month: '2-digit',
                                day: '2-digit',
                                hour: '2-digit',
                                minute: '2-digit'
                              })}
                            </span>
                            <span
                              class={`px-2 py-0.5 text-xs rounded font-medium ${
                                exec.side === 'Buy'
                                  ? 'bg-green-500/20 text-green-400'
                                  : 'bg-red-500/20 text-red-400'
                              }`}
                            >
                              {exec.side === 'Buy' ? '매수' : '매도'}
                            </span>
                            <SymbolDisplay
                              ticker={exec.symbol}
                              mode="inline"
                              size="sm"
                              autoFetch={true}
                            />
                          </div>
                          <div class="text-right">
                            <div class="text-sm text-[var(--color-text)]">
                              {formatDecimal(exec.quantity, 4)} @ {formatCurrency(exec.price)}
                            </div>
                            <Show when={realizedPnl !== null}>
                              <div class={`text-sm ${realizedPnl! >= 0 ? 'text-green-500' : 'text-red-500'}`}>
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

        {/* 가격 차트 + 매매 태그 (접이식, Backtest와 동일 패턴) */}
        <Show when={executions().length > 0 || isRunning()}>
          <details class="mt-4">
            <summary class="cursor-pointer text-sm text-[var(--color-text-muted)] hover:text-[var(--color-text)] flex items-center gap-2">
              <LineChart class="w-4 h-4" />
              가격 차트 + 매매 태그
            </summary>
            <div class="mt-3 space-y-3">
              {/* 신호 필터 패널 (Lazy Loaded) */}
              <Suspense fallback={<div class="h-12 bg-gray-100 dark:bg-gray-800 animate-pulse rounded" />}>
                <IndicatorFilterPanel
                  filters={signalFilters()}
                  onChange={(filters) => setSignalFilters(filters)}
                  defaultCollapsed={true}
                />
              </Suspense>

              {/* 다중 심볼인 경우 심볼 선택 탭 표시 */}
              <Show when={(() => {
                const strategyId = selectedStrategyId()
                const strategy = strategies()?.find(s => s.id === strategyId)
                return strategy?.symbols && strategy.symbols.length > 1
              })()}>
                <div class="flex flex-wrap gap-1 p-1 bg-[var(--color-surface-light)]/30 rounded-lg">
                  <For each={(() => {
                    const strategyId = selectedStrategyId()
                    const strategy = strategies()?.find(s => s.id === strategyId)
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
                  표시 중: {filteredTradeMarkers().length} / {tradeMarkers().length} 마커
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
                when={chartData().length > 1}
                fallback={
                  <div class="h-[280px] flex items-center justify-center text-[var(--color-text-muted)]">
                    {isRunning() ? (
                      <div class="flex items-center gap-2">
                        <RefreshCw class="w-5 h-5 animate-spin" />
                        <span>WebSocket 데이터 수신 대기 중...</span>
                      </div>
                    ) : (
                      <span>Paper Trading을 시작하면 실시간 차트가 표시됩니다</span>
                    )}
                  </div>
                }
              >
                <div class="flex gap-2">
                  {/* 캔들 차트 */}
                  <div class="flex-1">
                    <SyncedChartPanel
                      data={chartData()}
                      type="candlestick"
                      mainHeight={240}
                      markers={filteredTradeMarkers()}
                      chartId="paper-price"
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
                          currentPrice={currentPrice()}
                          chartHeight={240}
                          width={80}
                          priceRange={chartPriceRange()}
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

        {/* 리스크 분석 (Kelly + 상관관계) */}
        <Show when={executions().length >= 3}>
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

        {/* 실행 중인 경우 실시간 업데이트 안내 */}
        <Show when={isRunning()}>
          <div class="text-center text-sm text-[var(--color-text-muted)]">
            🟢 Paper Trading 실행 중 - WebSocket으로 실시간 업데이트
            <Show when={wsConnected()}>
              <span class="ml-2 inline-block w-2 h-2 bg-green-500 rounded-full animate-pulse" />
            </Show>
          </div>
        </Show>
      </Show>
    </div>
  )
}

export default PaperTrading
