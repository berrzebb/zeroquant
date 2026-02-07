import { createSignal, createEffect, createMemo, For, Show, onCleanup } from 'solid-js'
import { createQuery } from '@tanstack/solid-query'
import {
  Search, TrendingUp, LineChart, Table, Loader2, ArrowUp, ArrowDown,
  X, RefreshCw, Trash2, Settings2
} from 'lucide-solid'
import { type CandlestickDataPoint, type IndicatorOverlay } from './charts/PriceChart'
import { type SeparateIndicatorData } from './charts/SubPriceChart'
import { SyncedChartPanel } from './charts/SyncedChartPanel'
import {
  // Types
  type CandleItem,
  type IndicatorType,
  type IndicatorParams,
  type ActiveIndicator,
  // Constants
  INDICATOR_META,
  // Functions
  getTimeKey,
  calculateSMA,
  calculateEMA,
  calculateBollingerBands,
  calculateRSI,
  calculateMACD,
  calculateVolume,
  calculateStochastic,
  calculateATR,
  calculateATRPercent,
  calculateMomentumScore,
} from '../utils/indicators'

// ==================== 타입 ====================

export interface DatasetSummary {
  symbol: string
  displayName?: string  // "005930(삼성전자)" 형식
  timeframe: string
  firstTime: string | null
  lastTime: string | null
  candleCount: number
  lastUpdated: string | null
}

interface CandleDataResponse {
  symbol: string
  timeframe: string
  candles: CandleItem[]
  totalCount: number
}

type SortColumnType = 'time' | 'close' | 'change' | 'volume' | 'open' | 'high' | 'low'
type SortOrderType = 'desc' | 'asc'

// ==================== API ====================

const API_BASE = 'http://localhost:3000/api/v1'

async function fetchCandles(
  symbol: string,
  timeframe: string,
  limit: number,
  sortBy: SortColumnType = 'time',
  sortOrder: SortOrderType = 'desc'
): Promise<CandleDataResponse> {
  const serverSortBy = sortBy === 'change' ? 'time' : sortBy
  const params = new URLSearchParams({
    timeframe,
    limit: limit.toString(),
    sortBy: serverSortBy,
    sortOrder,
  })
  const res = await fetch(`${API_BASE}/dataset/${encodeURIComponent(symbol)}?${params}`)
  if (!res.ok) throw new Error('캔들 데이터 조회 실패')
  return res.json()
}

// ==================== 타임프레임 유틸 ====================

const timeframeText: Record<string, string> = {
  '1m': '1분', '5m': '5분', '15m': '15분', '30m': '30분',
  '1h': '1시간', '2h': '2시간', '4h': '4시간',
  '1d': '일봉', '1wk': '주봉', '1mo': '월봉',
}

// ==================== Props ====================

export interface SymbolPanelProps {
  symbol?: string
  symbolDisplayName?: string  // 티커 (이름) 형식의 표시 이름
  timeframe: string
  datasets: DatasetSummary[]
  cachedSymbols: string[]
  onSymbolChange: (symbol: string) => void
  onTimeframeChange: (tf: string) => void
  onRefresh: () => void
  onDelete: () => void
  isRefreshing: boolean
  compact?: boolean
}

// ==================== 컴포넌트 ====================

export function SymbolPanel(props: SymbolPanelProps) {
  const [viewMode, setViewMode] = createSignal<'chart' | 'table'>('chart')
  const [sortColumn, setSortColumn] = createSignal<SortColumnType>('time')
  const [sortOrder, setSortOrder] = createSignal<SortOrderType>('desc')

  // 통합 지표 목록 (오버레이 + 서브차트)
  const [activeIndicators, setActiveIndicators] = createSignal<ActiveIndicator[]>([
    { id: 'vol-1', type: 'volume', params: {}, enabled: true, isOverlay: false },
  ])

  // 지표 추가 모달 상태
  const [showIndicatorModal, setShowIndicatorModal] = createSignal(false)
  const [newIndicatorType, setNewIndicatorType] = createSignal<IndicatorType>('rsi')
  const [newIndicatorParams, setNewIndicatorParams] = createSignal<Record<string, unknown>>({})

  // 패널 내 심볼 검색 (자동완성)
  const [panelSearch, setPanelSearch] = createSignal('')
  const [showAutocomplete, setShowAutocomplete] = createSignal(false)
  const [selectedIndex, setSelectedIndex] = createSignal(-1)

  // 테이블 무한 스크롤 상태
  const [visibleRows, setVisibleRows] = createSignal(50)
  const ROWS_PER_LOAD = 50
  let tableEndRef: HTMLDivElement | undefined

  // 테이블 뷰로 전환 시 표시 행 수 리셋
  createEffect(() => {
    if (viewMode() === 'table') {
      setVisibleRows(50)
    }
  })

  // Intersection Observer로 무한 스크롤 구현
  createEffect(() => {
    if (viewMode() !== 'table' || !tableEndRef) return

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting) {
          const totalRows = tableData().length
          if (visibleRows() < totalRows) {
            setVisibleRows(prev => Math.min(prev + ROWS_PER_LOAD, totalRows))
          }
        }
      },
      { threshold: 0.1 }
    )

    observer.observe(tableEndRef)

    onCleanup(() => observer.disconnect())
  })

  // 새 지표 타입 선택 시 기본 파라미터 설정
  createEffect(() => {
    const type = newIndicatorType()
    const meta = INDICATOR_META[type]
    setNewIndicatorParams({ ...meta.defaultParams })
  })

  // 지표 추가 함수
  const addIndicator = () => {
    const type = newIndicatorType()
    const params = newIndicatorParams()
    const meta = INDICATOR_META[type]
    const newIndicator: ActiveIndicator = {
      id: `${type}-${Date.now()}`,
      type,
      params: params as IndicatorParams[typeof type],
      enabled: true,
      isOverlay: meta.isOverlay,
    }
    setActiveIndicators(prev => [...prev, newIndicator])
    setShowIndicatorModal(false)
  }

  // 지표 제거 함수
  const removeIndicator = (id: string) => {
    setActiveIndicators(prev => prev.filter(ind => ind.id !== id))
  }

  // 지표 토글 함수
  const toggleIndicator = (id: string) => {
    setActiveIndicators(prev => prev.map(ind =>
      ind.id === id ? { ...ind, enabled: !ind.enabled } : ind
    ))
  }

  // 자동완성 심볼 목록 (캐시된 심볼만 필터링)
  const autocompleteSymbols = createMemo(() => {
    const term = panelSearch().toUpperCase().trim()
    if (!term) return []
    // 캐시된 심볼 중 검색어와 매칭되는 것만 표시 (최대 8개)
    return props.cachedSymbols
      .filter(s => s.toUpperCase().includes(term))
      .slice(0, 8)
  })

  // 캔들 데이터 쿼리
  const candlesQuery = createQuery(() => ({
    queryKey: ['candles', props.symbol, props.timeframe, 500, sortColumn(), sortOrder()],
    queryFn: () => fetchCandles(props.symbol!, props.timeframe, 500, sortColumn(), sortOrder()),
    enabled: !!props.symbol && !!props.timeframe,
    staleTime: 30000,
  }))

  // 타임프레임 목록
  const availableTimeframes = createMemo(() => {
    const order = ['1m', '5m', '15m', '30m', '1h', '2h', '4h', '1d', '1wk', '1mo']
    return props.datasets
      .filter(d => d.symbol === props.symbol)
      .map(d => d.timeframe)
      .sort((a, b) => order.indexOf(a) - order.indexOf(b))
  })

  // 현재 데이터셋 정보
  const currentDataset = createMemo(() =>
    props.datasets.find(d => d.symbol === props.symbol && d.timeframe === props.timeframe)
  )

  // 일봉 이상 타임프레임인지 확인
  const isDailyOrHigher = createMemo(() => {
    const tf = props.timeframe
    return tf === '1d' || tf === '3d' || tf === '1wk' || tf === '1mo' || tf === 'd' || tf === 'w' || tf === 'M'
  })

  // 차트 데이터 (오름차순, 타임프레임에 따라 시간 형식 결정)
  const chartData = createMemo((): CandlestickDataPoint[] => {
    const candles = candlesQuery.data?.candles || []
    const daily = isDailyOrHigher()
    const uniqueMap = new Map<string | number, CandlestickDataPoint>()

    candles.forEach(c => {
      // 일봉 이상이면 날짜만 사용, 시간봉 이하면 Unix timestamp로 변환
      const timeKey = getTimeKey(c.time, daily)
      uniqueMap.set(timeKey, {
        time: timeKey,
        open: parseFloat(c.open),
        high: parseFloat(c.high),
        low: parseFloat(c.low),
        close: parseFloat(c.close),
      })
    })

    // 정렬: 일봉은 문자열 비교, 시간봉은 숫자 비교
    return Array.from(uniqueMap.values()).sort((a, b) => {
      if (typeof a.time === 'number' && typeof b.time === 'number') {
        return a.time - b.time
      }
      return (a.time as string).localeCompare(b.time as string)
    })
  })

  // 오버레이 지표 계산 (동적 시스템)
  const indicators = createMemo((): IndicatorOverlay[] => {
    const candles = candlesQuery.data?.candles || []
    if (candles.length === 0) return []

    // 오름차순 정렬 (지표 계산용)
    const sortedCandles = [...candles].sort((a, b) => a.time.localeCompare(b.time))
    const result: IndicatorOverlay[] = []
    const daily = isDailyOrHigher()

    // 활성화된 오버레이 지표만 계산
    for (const indicator of activeIndicators()) {
      if (!indicator.enabled || !indicator.isOverlay) continue

      const meta = INDICATOR_META[indicator.type]

      switch (indicator.type) {
        case 'sma': {
          const params = indicator.params as IndicatorParams['sma']
          const smaData = calculateSMA(sortedCandles, params.period, daily)
          if (smaData.length > 0) {
            result.push({
              id: indicator.id,
              name: `SMA ${params.period}`,
              data: smaData,
              color: meta.color,
              lineWidth: 1,
            })
          }
          break
        }

        case 'ema': {
          const params = indicator.params as IndicatorParams['ema']
          const emaData = calculateEMA(sortedCandles, params.period, daily)
          if (emaData.length > 0) {
            result.push({
              id: indicator.id,
              name: `EMA ${params.period}`,
              data: emaData,
              color: meta.color,
              lineWidth: 1,
            })
          }
          break
        }

        case 'bb': {
          const params = indicator.params as IndicatorParams['bb']
          const bb = calculateBollingerBands(sortedCandles, params.period, params.stdDev, daily)
          if (bb.middle.length > 0) {
            result.push({ id: `${indicator.id}-upper`, name: 'BB Upper', data: bb.upper, color: meta.color, lineWidth: 1 })
            result.push({ id: `${indicator.id}-middle`, name: 'BB Middle', data: bb.middle, color: meta.color, lineWidth: 1 })
            result.push({ id: `${indicator.id}-lower`, name: 'BB Lower', data: bb.lower, color: meta.color, lineWidth: 1 })
          }
          break
        }
      }
    }

    return result
  })

  // 서브 차트 지표 데이터 (동적 생성)
  const subIndicators = createMemo((): SeparateIndicatorData[] => {
    const candles = candlesQuery.data?.candles || []
    if (candles.length === 0) return []

    const sortedCandles = [...candles].sort((a, b) => a.time.localeCompare(b.time))
    const result: SeparateIndicatorData[] = []
    const daily = isDailyOrHigher()

    // 서브 차트 지표만 필터링 (isOverlay가 false인 것)
    for (const indicator of activeIndicators()) {
      if (!indicator.enabled || indicator.isOverlay) continue

      const meta = INDICATOR_META[indicator.type]

      switch (indicator.type) {
        case 'volume': {
          const volumeData = calculateVolume(sortedCandles, daily)
          result.push({
            id: indicator.id,
            type: 'volume',
            name: 'Volume',
            series: [{
              name: 'Volume',
              data: volumeData.data,
              color: meta.color,
              seriesType: 'bar',
            }],
          })
          break
        }

        case 'rsi': {
          const params = indicator.params as IndicatorParams['rsi']
          const rsiData = calculateRSI(sortedCandles, params.period, daily)
          if (rsiData.length > 0) {
            result.push({
              id: indicator.id,
              type: 'rsi',
              name: `RSI (${params.period})`,
              series: [{
                name: 'RSI',
                data: rsiData,
                color: meta.color,
                seriesType: 'line',
                lineWidth: 2,
              }],
              scaleRange: meta.scaleRange,
            })
          }
          break
        }

        case 'macd': {
          const params = indicator.params as IndicatorParams['macd']
          const macdData = calculateMACD(sortedCandles, params.fastPeriod, params.slowPeriod, params.signalPeriod, daily)
          if (macdData.macd.length > 0) {
            result.push({
              id: indicator.id,
              type: 'macd',
              name: `MACD (${params.fastPeriod}, ${params.slowPeriod}, ${params.signalPeriod})`,
              series: [
                { name: 'MACD', data: macdData.macd, color: '#3b82f6', seriesType: 'line', lineWidth: 2 },
                { name: 'Signal', data: macdData.signal, color: '#f97316', seriesType: 'line', lineWidth: 1 },
                { name: 'Histogram', data: macdData.histogram, color: '#22c55e', seriesType: 'bar' },
              ],
            })
          }
          break
        }

        case 'stochastic': {
          const params = indicator.params as IndicatorParams['stochastic']
          const stochData = calculateStochastic(sortedCandles, params.kPeriod, params.dPeriod, daily)
          if (stochData.k.length > 0) {
            result.push({
              id: indicator.id,
              type: 'stochastic',
              name: `Stochastic (${params.kPeriod}, ${params.dPeriod})`,
              series: [
                { name: '%K', data: stochData.k, color: '#f59e0b', seriesType: 'line', lineWidth: 2 },
                { name: '%D', data: stochData.d, color: '#a855f7', seriesType: 'line', lineWidth: 1 },
              ],
              scaleRange: meta.scaleRange,
            })
          }
          break
        }

        case 'atr': {
          const params = indicator.params as IndicatorParams['atr']
          const atrData = calculateATR(sortedCandles, params.period, daily)
          if (atrData.length > 0) {
            result.push({
              id: indicator.id,
              type: 'atr',
              name: `ATR (${params.period})`,
              series: [{
                name: 'ATR',
                data: atrData,
                color: meta.color,
                seriesType: 'line',
                lineWidth: 2,
              }],
            })
          }
          break
        }

        case 'atr_percent': {
          const params = indicator.params as IndicatorParams['atr_percent']
          const atrPctData = calculateATRPercent(sortedCandles, params.period, daily)
          if (atrPctData.length > 0) {
            result.push({
              id: indicator.id,
              type: 'atr_percent',
              name: `ATR% (${params.period})`,
              series: [{
                name: 'ATR%',
                data: atrPctData,
                color: meta.color,
                seriesType: 'line',
                lineWidth: 2,
              }],
            })
          }
          break
        }

        case 'momentum': {
          const params = indicator.params as IndicatorParams['momentum']
          const momData = calculateMomentumScore(sortedCandles, params.periods, daily)
          if (momData.length > 0) {
            result.push({
              id: indicator.id,
              type: 'momentum',
              name: `Momentum (${params.periods.join(', ')})`,
              series: [{
                name: 'Momentum',
                data: momData,
                color: meta.color,
                seriesType: 'line',
                lineWidth: 2,
              }],
            })
          }
          break
        }
      }
    }

    return result
  })

  // 테이블 데이터
  const tableData = createMemo(() => {
    const candles = candlesQuery.data?.candles || []
    if (sortColumn() === 'change') {
      return [...candles].sort((a, b) => {
        const changeA = (parseFloat(a.close) - parseFloat(a.open)) / parseFloat(a.open)
        const changeB = (parseFloat(b.close) - parseFloat(b.open)) / parseFloat(b.open)
        return sortOrder() === 'desc' ? changeB - changeA : changeA - changeB
      })
    }
    return candles
  })

  // 컬럼 정렬 핸들러
  const handleColumnSort = (column: SortColumnType) => {
    if (sortColumn() === column) {
      setSortOrder(prev => prev === 'desc' ? 'asc' : 'desc')
    } else {
      setSortColumn(column)
      setSortOrder('desc')
    }
  }

  // 날짜 포맷
  const formatDate = (dateStr: string | null) => {
    if (!dateStr) return '-'
    return new Date(dateStr).toLocaleDateString('ko-KR', { year: 'numeric', month: '2-digit', day: '2-digit' })
  }

  // 서브 차트 개수에 따른 메인 차트 높이 조절
  const subChartCount = () => subIndicators().length
  const chartHeight = () => {
    const base = props.compact ? 160 : 240
    // 서브차트가 있으면 메인 차트 높이를 줄임
    if (subChartCount() > 0) return Math.max(120, base - subChartCount() * 20)
    return base
  }
  const subChartHeight = () => props.compact ? 80 : 100

  // 심볼 선택 핸들러
  const handleSelectSymbol = (symbol: string) => {
    props.onSymbolChange(symbol)
    setPanelSearch('')
    setShowAutocomplete(false)
    setSelectedIndex(-1)
  }

  // 키보드 네비게이션 핸들러
  const handleKeyDown = (e: KeyboardEvent) => {
    const symbols = autocompleteSymbols()
    const len = symbols.length

    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSelectedIndex(prev => (prev + 1) % len)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSelectedIndex(prev => (prev - 1 + len) % len)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      const idx = selectedIndex()
      if (idx >= 0 && idx < len) {
        handleSelectSymbol(symbols[idx])
      } else if (panelSearch().trim()) {
        // 검색어가 있으면 그대로 사용 (새 심볼 다운로드용)
        handleSelectSymbol(panelSearch().trim().toUpperCase())
      }
    } else if (e.key === 'Escape') {
      setShowAutocomplete(false)
      setSelectedIndex(-1)
    }
  }

  // 빈 패널 안내 UI (헤더에서 심볼 검색)
  const EmptyPanelUI = () => (
    <div class="h-full flex flex-col items-center justify-center p-4 text-center">
      <Search class="w-10 h-10 text-[var(--color-text-muted)] mb-3 opacity-50" />
      <p class="text-sm text-[var(--color-text-muted)]">
        상단 헤더에서 심볼을 검색하세요
      </p>
    </div>
  )

  return (
    <Show when={props.symbol} fallback={<EmptyPanelUI />}>
    <div class="h-full flex flex-col gap-2 overflow-hidden">
      {/* 심볼 + 타임프레임 + 액션 */}
      <div class="flex items-center justify-between flex-shrink-0">
        <div class="flex items-center gap-2">
          {/* 심볼 표시 및 변경 */}
          <button
            onClick={() => props.onSymbolChange('')}
            class="px-2 py-1 text-xs font-mono font-semibold bg-[var(--color-primary)]/20
                   text-[var(--color-primary)] rounded hover:bg-[var(--color-primary)]/30
                   transition flex items-center gap-1"
            title="심볼 변경"
          >
            <TrendingUp class="w-3 h-3" />
            {props.symbolDisplayName || props.symbol}
            <X class="w-3 h-3 ml-1 opacity-60" />
          </button>
          {/* 타임프레임 */}
          <div class="flex items-center gap-0.5">
            <For each={availableTimeframes()}>
              {(tf) => (
                <button
                  onClick={() => props.onTimeframeChange(tf)}
                  class={`px-1.5 py-0.5 text-xs rounded transition
                          ${props.timeframe === tf
                            ? 'bg-[var(--color-primary)] text-white'
                            : 'bg-[var(--color-bg)] text-[var(--color-text-muted)] hover:text-[var(--color-text)]'}`}
                >
                  {timeframeText[tf] || tf}
                </button>
              )}
            </For>
          </div>
        </div>
        <div class="flex items-center gap-1">
          <button
            onClick={props.onRefresh}
            disabled={props.isRefreshing}
            class="p-1 hover:bg-[var(--color-surface-light)] rounded"
            title="새로고침"
          >
            <RefreshCw class={`w-3.5 h-3.5 text-[var(--color-text-muted)] ${props.isRefreshing ? 'animate-spin' : ''}`} />
          </button>
          <button
            onClick={props.onDelete}
            class="p-1 hover:bg-red-500/20 rounded"
            title="삭제"
          >
            <Trash2 class="w-3.5 h-3.5 text-red-400" />
          </button>
        </div>
      </div>

      {/* 데이터셋 정보 (컴팩트 모드 아닐 때만) */}
      <Show when={!props.compact && currentDataset()}>
        {(dataset) => (
          <div class="grid grid-cols-4 gap-2 text-xs flex-shrink-0">
            <div>
              <span class="text-[var(--color-text-muted)]">시작</span>
              <p class="text-[var(--color-text)]">{formatDate(dataset().firstTime)}</p>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">종료</span>
              <p class="text-[var(--color-text)]">{formatDate(dataset().lastTime)}</p>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">캔들</span>
              <p class="text-[var(--color-text)]">{dataset().candleCount.toLocaleString()}</p>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">업데이트</span>
              <p class="text-[var(--color-text)]">{formatDate(dataset().lastUpdated)}</p>
            </div>
          </div>
        )}
      </Show>

      {/* 뷰 모드 + 지표 토글 */}
      <div class="flex items-center justify-between flex-shrink-0 flex-wrap gap-1">
        <div class="flex gap-1">
          <button
            onClick={() => setViewMode('chart')}
            class={`px-2 py-1 text-xs rounded flex items-center gap-1 transition
                    ${viewMode() === 'chart'
                      ? 'bg-[var(--color-primary)] text-white'
                      : 'bg-[var(--color-bg)] text-[var(--color-text-muted)]'}`}
          >
            <LineChart class="w-3 h-3" />
            차트
          </button>
          <button
            onClick={() => setViewMode('table')}
            class={`px-2 py-1 text-xs rounded flex items-center gap-1 transition
                    ${viewMode() === 'table'
                      ? 'bg-[var(--color-primary)] text-white'
                      : 'bg-[var(--color-bg)] text-[var(--color-text-muted)]'}`}
          >
            <Table class="w-3 h-3" />
            테이블
          </button>
        </div>

        <Show when={viewMode() === 'chart'}>
          <div class="flex items-center gap-1 flex-wrap">
            {/* 오버레이 지표 토글 */}
            <Show when={activeIndicators().filter(i => i.isOverlay).length > 0}>
              <div class="flex gap-0.5 items-center">
                <For each={activeIndicators().filter(i => i.isOverlay)}>
                  {(ind) => (
                    <div class="flex items-center">
                      <button
                        onClick={() => toggleIndicator(ind.id)}
                        class={`px-1.5 py-0.5 text-xs rounded-l transition
                                ${ind.enabled
                                  ? `bg-opacity-30 text-opacity-100`
                                  : 'bg-[var(--color-bg)] text-[var(--color-text-muted)]'}`}
                        style={{
                          'background-color': ind.enabled ? `${INDICATOR_META[ind.type].color}30` : undefined,
                          color: ind.enabled ? INDICATOR_META[ind.type].color : undefined,
                        }}
                        title={INDICATOR_META[ind.type].description}
                      >
                        {INDICATOR_META[ind.type].name}
                        {Object.keys(ind.params).length > 0 && (
                          <span class="ml-0.5 opacity-70">
                            {Object.values(ind.params)[0]}
                          </span>
                        )}
                      </button>
                      <button
                        onClick={() => removeIndicator(ind.id)}
                        class="px-1 py-0.5 text-xs rounded-r bg-red-500/20 text-red-400 hover:bg-red-500/40 transition"
                        title="제거"
                      >
                        <X class="w-2.5 h-2.5" />
                      </button>
                    </div>
                  )}
                </For>
              </div>
              <span class="text-[var(--color-text-muted)] text-xs">|</span>
            </Show>
            {/* 서브 차트 지표 토글 */}
            <div class="flex gap-0.5 items-center">
              <For each={activeIndicators().filter(i => !i.isOverlay)}>
                {(ind) => (
                  <div class="flex items-center">
                    <button
                      onClick={() => toggleIndicator(ind.id)}
                      class={`px-1.5 py-0.5 text-xs rounded-l transition
                              ${ind.enabled
                                ? `bg-opacity-30 text-opacity-100`
                                : 'bg-[var(--color-bg)] text-[var(--color-text-muted)]'}`}
                      style={{
                        'background-color': ind.enabled ? `${INDICATOR_META[ind.type].color}30` : undefined,
                        color: ind.enabled ? INDICATOR_META[ind.type].color : undefined,
                      }}
                      title={INDICATOR_META[ind.type].description}
                    >
                      {INDICATOR_META[ind.type].name}
                    </button>
                    <button
                      onClick={() => removeIndicator(ind.id)}
                      class="px-1 py-0.5 text-xs rounded-r bg-red-500/20 text-red-400 hover:bg-red-500/40 transition"
                      title="제거"
                    >
                      <X class="w-2.5 h-2.5" />
                    </button>
                  </div>
                )}
              </For>
              {/* 지표 추가 버튼 */}
              <button
                onClick={() => setShowIndicatorModal(true)}
                class="px-1.5 py-0.5 text-xs rounded bg-[var(--color-primary)]/20 text-[var(--color-primary)]
                       hover:bg-[var(--color-primary)]/30 transition flex items-center gap-0.5"
                title="지표 추가"
              >
                <Settings2 class="w-3 h-3" />
                +
              </button>
            </div>
          </div>
        </Show>
      </div>

      {/* 지표 추가 모달 */}
      <Show when={showIndicatorModal()}>
        <div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center"
             onClick={(e) => { if (e.target === e.currentTarget) setShowIndicatorModal(false) }}>
          <div class="bg-[var(--color-surface)] rounded-xl p-4 w-80 max-w-[90vw] shadow-xl">
            <div class="flex items-center justify-between mb-4">
              <h3 class="text-sm font-semibold text-[var(--color-text)]">지표 추가</h3>
              <button
                onClick={() => setShowIndicatorModal(false)}
                class="p-1 hover:bg-[var(--color-surface-light)] rounded"
              >
                <X class="w-4 h-4 text-[var(--color-text-muted)]" />
              </button>
            </div>

            {/* 지표 타입 선택 */}
            <div class="mb-4">
              <label class="block text-xs text-[var(--color-text-muted)] mb-1">지표 종류</label>
              <select
                value={newIndicatorType()}
                onChange={(e) => setNewIndicatorType(e.currentTarget.value as IndicatorType)}
                style={{ "background-color": "#1a1a2e" }}
                class="w-full px-3 py-2 text-sm text-[var(--color-text)] rounded-lg border border-[var(--color-surface-light)]"
              >
                <optgroup label="오버레이 지표 (메인 차트)">
                  <For each={Object.entries(INDICATOR_META).filter(([_, m]) => m.isOverlay)}>
                    {([type, meta]) => (
                      <option value={type}>{meta.name} - {meta.description}</option>
                    )}
                  </For>
                </optgroup>
                <optgroup label="서브 차트 지표">
                  <For each={Object.entries(INDICATOR_META).filter(([_, m]) => !m.isOverlay)}>
                    {([type, meta]) => (
                      <option value={type}>{meta.name} - {meta.description}</option>
                    )}
                  </For>
                </optgroup>
              </select>
            </div>

            {/* 파라미터 입력 */}
            <div class="mb-4 space-y-3">
              <For each={Object.entries(INDICATOR_META[newIndicatorType()].paramLabels)}>
                {([key, label]) => (
                  <div>
                    <label class="block text-xs text-[var(--color-text-muted)] mb-1">{label}</label>
                    <Show
                      when={key === 'periods'}
                      fallback={
                        <input
                          type="number"
                          value={(newIndicatorParams() as Record<string, number>)[key] || 0}
                          onInput={(e) => setNewIndicatorParams(prev => ({
                            ...prev,
                            [key]: parseInt(e.currentTarget.value) || 0
                          }))}
                          class="w-full px-3 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                                 rounded-lg border border-[var(--color-surface-light)]"
                        />
                      }
                    >
                      <input
                        type="text"
                        value={((newIndicatorParams() as Record<string, number[]>)[key] || []).join(', ')}
                        onInput={(e) => setNewIndicatorParams(prev => ({
                          ...prev,
                          [key]: e.currentTarget.value.split(',').map(v => parseInt(v.trim())).filter(v => !isNaN(v))
                        }))}
                        placeholder="5, 10, 20"
                        class="w-full px-3 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                               rounded-lg border border-[var(--color-surface-light)]"
                      />
                    </Show>
                  </div>
                )}
              </For>
              <Show when={Object.keys(INDICATOR_META[newIndicatorType()].paramLabels).length === 0}>
                <p class="text-xs text-[var(--color-text-muted)]">설정 가능한 파라미터가 없습니다.</p>
              </Show>
            </div>

            {/* 미리보기 */}
            <div class="mb-4 p-2 bg-[var(--color-bg)] rounded-lg">
              <p class="text-xs text-[var(--color-text-muted)]">미리보기</p>
              <div class="flex items-center gap-2 mt-1">
                <span
                  class="w-3 h-0.5 rounded"
                  style={{ 'background-color': INDICATOR_META[newIndicatorType()].color }}
                />
                <span class="text-sm text-[var(--color-text)]" style={{ color: INDICATOR_META[newIndicatorType()].color }}>
                  {INDICATOR_META[newIndicatorType()].name}
                  <Show when={Object.keys(newIndicatorParams()).length > 0}>
                    {' '}({Object.values(newIndicatorParams()).map(v => Array.isArray(v) ? v.join(', ') : v).join(', ')})
                  </Show>
                </span>
              </div>
            </div>

            {/* 버튼 */}
            <div class="flex gap-2">
              <button
                onClick={() => setShowIndicatorModal(false)}
                class="flex-1 px-3 py-2 text-sm bg-[var(--color-bg)] text-[var(--color-text-muted)]
                       rounded-lg hover:bg-[var(--color-surface-light)] transition"
              >
                취소
              </button>
              <button
                onClick={addIndicator}
                class="flex-1 px-3 py-2 text-sm bg-[var(--color-primary)] text-white
                       rounded-lg hover:bg-[var(--color-primary-dark)] transition"
              >
                추가
              </button>
            </div>
          </div>
        </div>
      </Show>

      {/* 차트 뷰 */}
      <Show when={viewMode() === 'chart'}>
        <div class="flex-1 min-h-0 overflow-auto">
          <Show
            when={!candlesQuery.isLoading && chartData().length > 0}
            fallback={
              <div class="h-full flex items-center justify-center text-[var(--color-text-muted)]">
                {candlesQuery.isLoading ? (
                  <div class="flex items-center gap-2">
                    <Loader2 class="w-5 h-5 animate-spin" />
                    <span class="text-sm">차트 로딩...</span>
                  </div>
                ) : '데이터 없음'}
              </div>
            }
          >
            {/* 동기화된 차트 패널 (메인 + 서브 지표) */}
            <SyncedChartPanel
              data={chartData()}
              type="candlestick"
              mainHeight={chartHeight()}
              subHeight={subChartHeight()}
              indicators={indicators()}
              subIndicators={subIndicators()}
            />
          </Show>
        </div>
      </Show>

      {/* 테이블 뷰 */}
      <Show when={viewMode() === 'table'}>
        <div class="flex-1 min-h-0 overflow-auto">
          <table class="w-full text-xs">
            <thead class="bg-[var(--color-bg)] sticky top-0 z-10">
              <tr class="text-[var(--color-text-muted)] text-left">
                <th class="px-2 py-1.5 cursor-pointer" onClick={() => handleColumnSort('time')}>
                  <div class="flex items-center gap-1">
                    시간
                    <Show when={sortColumn() === 'time'}>
                      {sortOrder() === 'desc' ? <ArrowDown class="w-3 h-3" /> : <ArrowUp class="w-3 h-3" />}
                    </Show>
                  </div>
                </th>
                <th class="px-2 py-1.5 text-right cursor-pointer" onClick={() => handleColumnSort('close')}>
                  <div class="flex items-center justify-end gap-1">
                    종가
                    <Show when={sortColumn() === 'close'}>
                      {sortOrder() === 'desc' ? <ArrowDown class="w-3 h-3" /> : <ArrowUp class="w-3 h-3" />}
                    </Show>
                  </div>
                </th>
                <th class="px-2 py-1.5 text-right cursor-pointer" onClick={() => handleColumnSort('change')}>
                  <div class="flex items-center justify-end gap-1">
                    변동
                    <Show when={sortColumn() === 'change'}>
                      {sortOrder() === 'desc' ? <ArrowDown class="w-3 h-3" /> : <ArrowUp class="w-3 h-3" />}
                    </Show>
                  </div>
                </th>
                <th class="px-2 py-1.5 text-right cursor-pointer" onClick={() => handleColumnSort('volume')}>
                  <div class="flex items-center justify-end gap-1">
                    거래량
                    <Show when={sortColumn() === 'volume'}>
                      {sortOrder() === 'desc' ? <ArrowDown class="w-3 h-3" /> : <ArrowUp class="w-3 h-3" />}
                    </Show>
                  </div>
                </th>
              </tr>
            </thead>
            <tbody class="divide-y divide-[var(--color-surface-light)]">
              <For each={tableData().slice(0, visibleRows())}>
                {(candle) => {
                  const open = parseFloat(candle.open)
                  const close = parseFloat(candle.close)
                  const changePct = ((close - open) / open * 100).toFixed(2)
                  const isUp = close >= open
                  // 타임프레임에 따라 시간 표시 형식 결정
                  const timeDisplay = isDailyOrHigher() ? candle.time.split(' ')[0] : candle.time
                  return (
                    <tr class="hover:bg-[var(--color-surface-light)]">
                      <td class="px-2 py-1 text-[var(--color-text)] font-mono">{timeDisplay}</td>
                      <td class={`px-2 py-1 text-right font-mono ${isUp ? 'text-green-400' : 'text-red-400'}`}>
                        {close.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                      </td>
                      <td class={`px-2 py-1 text-right font-mono ${isUp ? 'text-green-400' : 'text-red-400'}`}>
                        {isUp ? '+' : ''}{changePct}%
                      </td>
                      <td class="px-2 py-1 text-right text-[var(--color-text-muted)] font-mono">
                        {parseInt(candle.volume).toLocaleString()}
                      </td>
                    </tr>
                  )
                }}
              </For>
            </tbody>
          </table>
          {/* 무한 스크롤 트리거 요소 */}
          <div
            ref={tableEndRef}
            class="h-4 flex items-center justify-center text-xs text-[var(--color-text-muted)]"
          >
            <Show when={visibleRows() < tableData().length}>
              <span class="opacity-50">스크롤하여 더 보기 ({visibleRows()}/{tableData().length})</span>
            </Show>
          </div>
        </div>
      </Show>
    </div>
    </Show>
  )
}
