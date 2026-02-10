import { createResource, createEffect, For, Show, createMemo, lazy, Suspense } from 'solid-js'
import { createStore } from 'solid-js/store'
import { useSearchParams, A } from '@solidjs/router'
import { Play, Calendar, ChartBar, Settings2, RefreshCw, AlertCircle, Info, X, ChevronDown, ChevronUp, LineChart, TrendingUp, TrendingDown, Clock, BarChart3 } from 'lucide-solid'
import type { EquityDataPoint, DrawdownDataPoint, CandlestickDataPoint, TradeMarker, IndicatorFilters, PriceVolume } from '../components/charts'
import { useChartSync } from '../hooks/useLightweightChart'

// Lazy load heavy chart components
const EquityCurve = lazy(() =>
  import('../components/charts/EquityCurve').then(m => ({ default: m.EquityCurve }))
)
const DrawdownChart = lazy(() =>
  import('../components/charts/DrawdownChart').then(m => ({ default: m.DrawdownChart }))
)
const SyncedChartPanel = lazy(() =>
  import('../components/charts/SyncedChartPanel').then(m => ({ default: m.SyncedChartPanel }))
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
  EmptyState,
  Button,
  DateRangePicker,
} from '../components/ui'
import {
  runBacktest,
  runMultiBacktest,
  getStrategies,
  getBacktestStrategies,
  listBacktestResults,
  saveBacktestResult,
  deleteBacktestResult,
  MULTI_ASSET_STRATEGIES,
  type BacktestRequest,
  type BacktestMultiRequest,
  type BacktestResult,
  type MultiTimeframeConfig,
  type Timeframe,
  type BacktestStrategy,
} from '../api/client'
import type { Strategy } from '../types'
import { SymbolDisplay } from '../components/SymbolDisplay'
import { MultiTimeframeSelector } from '../components/strategy/MultiTimeframeSelector'
import { createLogger } from '../utils/logger'
import { formatCurrency, formatPercent } from '../utils/format'

const { log, warn, error: logError } = createLogger('Backtest')

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
async function fetchCandlesForBacktest(
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

// 캔들 데이터에서 볼륨 프로파일 데이터 생성
function calculateVolumeProfile(candles: CandleItem[], bucketCount = 20): PriceVolume[] {
  if (candles.length === 0) return []

  // 가격 범위 계산
  let minPrice = Infinity
  let maxPrice = -Infinity
  candles.forEach(c => {
    const low = parseFloat(c.low)
    const high = parseFloat(c.high)
    if (low < minPrice) minPrice = low
    if (high > maxPrice) maxPrice = high
  })

  if (minPrice === maxPrice) return []

  // 가격 버킷 생성
  const priceStep = (maxPrice - minPrice) / bucketCount
  const buckets: Map<number, number> = new Map()

  // 각 캔들의 거래량을 해당 가격 범위 버킷에 분배
  candles.forEach(c => {
    const low = parseFloat(c.low)
    const high = parseFloat(c.high)
    const volume = parseFloat(c.volume)
    const candleRange = high - low || 1

    // 캔들이 걸치는 버킷들에 비례 분배
    for (let i = 0; i < bucketCount; i++) {
      const bucketLow = minPrice + i * priceStep
      const bucketHigh = bucketLow + priceStep
      const bucketMid = (bucketLow + bucketHigh) / 2

      // 캔들이 이 버킷과 겹치는지 확인
      if (high >= bucketLow && low <= bucketHigh) {
        // 겹치는 비율 계산
        const overlapLow = Math.max(low, bucketLow)
        const overlapHigh = Math.min(high, bucketHigh)
        const overlapRatio = (overlapHigh - overlapLow) / candleRange

        const currentVolume = buckets.get(bucketMid) || 0
        buckets.set(bucketMid, currentVolume + volume * overlapRatio)
      }
    }
  })

  // PriceVolume 배열로 변환
  const result: PriceVolume[] = []
  buckets.forEach((volume, price) => {
    result.push({ price, volume })
  })

  return result.sort((a, b) => a.price - b.price)
}

// 백테스트 거래 내역을 차트 마커로 변환 (roundtrip용 - 레거시)
function convertTradesToMarkers(trades: BacktestResult['trades']): TradeMarker[] {
  const markers: TradeMarker[] = []

  for (const trade of trades) {
    // entry_time 또는 entry_date 사용 (API 응답 필드명에 따라)
    const entryTime = trade.entry_time || trade.entry_date
    if (!entryTime) continue

    // 진입 마커 (ISO 날짜 형식에서 날짜만 추출)
    const entryDateStr = entryTime.split('T')[0]
    markers.push({
      time: entryDateStr,
      type: trade.side === 'Buy' ? 'buy' : 'sell',
      price: parseFloat(trade.entry_price),
      label: trade.side === 'Buy' ? '매수' : '매도',
    })

    // 청산 마커
    const exitTime = trade.exit_time || trade.exit_date
    if (exitTime) {
      const exitDateStr = exitTime.split('T')[0]
      markers.push({
        time: exitDateStr,
        type: trade.side === 'Buy' ? 'sell' : 'buy', // 청산은 반대 방향
        price: parseFloat(trade.exit_price),
        label: '청산',
      })
    }
  }

  // 시간순 정렬
  return markers.sort((a, b) =>
    (a.time as string).localeCompare(b.time as string)
  )
}

// 신호 타입별 라벨 매핑
const SIGNAL_TYPE_LABELS: Record<string, string> = {
  entry: '진입',
  exit: '청산',
  add_to_position: '추가매수',
  reduce_position: '부분청산',
  scale: '스케일',
  alert: '알림',
}

// 신호 타입을 마커 타입으로 변환
function signalTypeToMarkerType(signalType: string, side: string): TradeMarker['type'] {
  // entry/add_to_position → 해당 방향 (buy/sell)
  // exit/reduce_position → 해당 방향 (buy/sell)
  if (signalType === 'entry' || signalType === 'add_to_position') {
    return 'entry'
  }
  if (signalType === 'exit' || signalType === 'reduce_position') {
    return 'exit'
  }
  // 기본: side 기반
  return side === 'buy' ? 'buy' : 'sell'
}

// all_trades를 차트 마커로 변환 (모든 개별 신호 표시)
function convertAllTradesToMarkers(allTrades: BacktestResult['all_trades']): (TradeMarker & { signalType: string; side: string })[] {
  if (!allTrades) return []

  return allTrades.map(trade => {
    const dateStr = trade.timestamp.split('T')[0]
    const label = SIGNAL_TYPE_LABELS[trade.signal_type] || trade.signal_type

    return {
      time: dateStr,
      type: signalTypeToMarkerType(trade.signal_type, trade.side),
      price: parseFloat(trade.price),
      label: `${label} (${trade.side === 'buy' ? '매수' : '매도'})`,
      signalType: trade.signal_type,
      side: trade.side,
    }
  }).sort((a, b) => (a.time as string).localeCompare(b.time as string))
}

// 자산 곡선 데이터를 차트 컴포넌트 형식으로 변환 (시간순 정렬 + 중복 제거 필수)
function convertEquityCurve(result: BacktestResult): EquityDataPoint[] {
  const sorted = result.equity_curve
    .slice()
    .sort((a, b) => a.timestamp - b.timestamp)
    .map(point => ({
      time: new Date(point.timestamp * 1000).toISOString().split('T')[0],
      value: parseFloat(point.equity),
    }))

  // 중복 타임스탬프 제거 (같은 날짜면 마지막 값 유지)
  const uniqueMap = new Map<string, EquityDataPoint>()
  for (const point of sorted) {
    uniqueMap.set(point.time, point)
  }
  const result_array = Array.from(uniqueMap.values())

  // 디버깅: 급격한 변동 감지 (10% 이상 변동시 로그)
  let hasAnomalies = false
  for (let i = 1; i < result_array.length; i++) {
    const prev = result_array[i - 1]
    const curr = result_array[i]
    const changePercent = ((curr.value - prev.value) / prev.value) * 100
    if (Math.abs(changePercent) > 10) {
      hasAnomalies = true
      warn(
        `급격한 변동 감지: ${prev.time} -> ${curr.time}, ` +
        `${prev.value.toLocaleString()} -> ${curr.value.toLocaleString()} (${changePercent.toFixed(2)}%)`
      )
      // 해당 날짜 주변의 원본 데이터 출력
      const anomalyDate = curr.time
      const rawDataAroundAnomaly = sorted.filter(p =>
        p.time >= prev.time && p.time <= curr.time
      )
      log(`${anomalyDate} 주변 원본 데이터:`, rawDataAroundAnomaly)
    }
  }

  if (hasAnomalies) {
    log('전체 원본 포인트 수:', result.equity_curve.length)
    log('중복 제거 후 포인트 수:', result_array.length)
    log('같은 날짜에 여러 포인트가 있는 경우가 있는지 확인하세요.')
  }

  return result_array
}

// 드로우다운 곡선 데이터를 차트 컴포넌트 형식으로 변환 (시간순 정렬 + 중복 제거 필수)
function convertDrawdownCurve(result: BacktestResult): DrawdownDataPoint[] {
  const sorted = result.equity_curve
    .slice()
    .sort((a, b) => a.timestamp - b.timestamp)
    .map(point => ({
      time: new Date(point.timestamp * 1000).toISOString().split('T')[0],
      value: parseFloat(point.drawdown_pct),
    }))

  // 중복 타임스탬프 제거 (같은 날짜면 마지막 값 유지)
  const uniqueMap = new Map<string, DrawdownDataPoint>()
  for (const point of sorted) {
    uniqueMap.set(point.time, point)
  }
  return Array.from(uniqueMap.values())
}

// 저장된 결과와 DB ID를 매핑하기 위한 인터페이스
interface StoredBacktestResult extends BacktestResult {
  dbId?: string  // DB에 저장된 ID
}

// 백테스트 결과 카드 컴포넌트 (차트 동기화를 위해 분리)
interface BacktestResultCardProps {
  result: StoredBacktestResult
  strategies: Strategy[] | undefined
  index: number
  onDelete: (index: number) => void | Promise<void>
}

// BacktestResultCard 상태 타입
interface CardUIState {
  isExpanded: boolean
  showPriceChart: boolean
  showVolumeProfile: boolean
  selectedSymbol: string
  isLoadingCandles: boolean
}

interface CardChartState {
  signalFilters: IndicatorFilters
}

interface CardCacheState {
  candleData: Record<string, CandlestickDataPoint[]>
  rawCandleData: Record<string, CandleItem[]>
}

const initialCardUIState: CardUIState = {
  isExpanded: false,
  showPriceChart: false,
  showVolumeProfile: true,
  selectedSymbol: '',
  isLoadingCandles: false,
}

const initialCardChartState: CardChartState = {
  signalFilters: { signal_types: [], indicators: [] },
}

const initialCardCacheState: CardCacheState = {
  candleData: {},
  rawCandleData: {},
}

function BacktestResultCard(props: BacktestResultCardProps) {
  // Store 기반 상태 관리
  const [ui, setUI] = createStore<CardUIState>({ ...initialCardUIState })
  const [chartState, setChartState] = createStore<CardChartState>({ ...initialCardChartState })
  const [cache, setCache] = createStore<CardCacheState>({ ...initialCardCacheState })

  // 차트 동기화 훅 사용 (createSignal 기반으로 리액티비티 보장)
  const chartSync = useChartSync({ mode: 'full' })

  // 다중 심볼 목록 파싱
  const symbols = createMemo(() =>
    props.result.symbol.split(',').map(s => s.trim()).filter(s => s)
  )

  // 선택된 심볼 초기화
  createEffect(() => {
    const syms = symbols()
    if (syms.length > 0 && !ui.selectedSymbol) {
      setUI('selectedSymbol', syms[0])
    }
  })

  // 현재 선택된 심볼의 캔들 데이터
  const candleData = createMemo(() => cache.candleData[ui.selectedSymbol] || [])
  // 현재 선택된 심볼의 원본 캔들 데이터 (볼륨 포함)
  const rawCandleData = createMemo(() => cache.rawCandleData[ui.selectedSymbol] || [])

  // 볼륨 프로파일 데이터 계산
  const volumeProfileData = createMemo(() => {
    const raw = rawCandleData()
    if (raw.length === 0) return []
    return calculateVolumeProfile(raw, 25)
  })

  // 현재가 (마지막 종가)
  const currentPrice = createMemo(() => {
    const data = candleData()
    if (data.length === 0) return 0
    return data[data.length - 1].close
  })

  // 차트 가격 범위 (볼륨 프로파일 동기화용)
  const chartPriceRange = createMemo((): [number, number] => {
    const data = candleData()
    if (data.length === 0) return [0, 0]
    let min = Infinity
    let max = -Infinity
    data.forEach(c => {
      if (c.low < min) min = c.low
      if (c.high > max) max = c.high
    })
    return [min, max]
  })

  // 매매 마커 (선택된 심볼만 필터링) - all_trades 사용으로 모든 신호 표시
  const tradeMarkers = createMemo(() => {
    const selected = ui.selectedSymbol
    const allTrades = props.result.all_trades

    // all_trades가 있으면 사용, 없으면 roundtrip 기반 레거시 방식
    if (allTrades && allTrades.length > 0) {
      // 심볼 필터링
      const filteredTrades = allTrades.filter(t => {
        const tradeSymbol = t.symbol.split('/')[0] // "122630/KRW" → "122630"
        return tradeSymbol === selected || t.symbol === selected
      })
      return convertAllTradesToMarkers(filteredTrades)
    }

    // 레거시: roundtrip 기반
    const filteredTrades = props.result.trades.filter(t => {
      const tradeSymbol = t.symbol.split('/')[0]
      return tradeSymbol === selected || t.symbol === selected
    })
    return convertTradesToMarkers(filteredTrades).map(m => ({
      ...m,
      signalType: m.type === 'buy' || m.type === 'entry' ? 'entry' : 'exit',
      side: m.type === 'buy' || m.type === 'entry' ? 'buy' : 'sell',
    }))
  })

  // 필터가 적용된 매매 마커
  const filteredTradeMarkers = createMemo(() => {
    const markers = tradeMarkers()
    const { signalFilters } = chartState

    // 필터가 없으면 모든 마커 반환
    if (signalFilters.signal_types.length === 0) {
      return markers
    }

    // signal_type 및 side 필터 적용
    return markers.filter(marker => {
      const extMarker = marker as TradeMarker & { signalType?: string; side?: string }
      const signalType = extMarker.signalType
      const side = extMarker.side

      // buy/sell 필터 (side 기반)
      if (signalFilters.signal_types.includes('buy') && side === 'buy') return true
      if (signalFilters.signal_types.includes('sell') && side === 'sell') return true

      // 상세 signal_type 필터
      if (signalType) {
        if (signalFilters.signal_types.includes('entry') && signalType === 'entry') return true
        if (signalFilters.signal_types.includes('exit') && signalType === 'exit') return true
        if (signalFilters.signal_types.includes('add_to_position') && signalType === 'add_to_position') return true
        if (signalFilters.signal_types.includes('reduce_position') && signalType === 'reduce_position') return true
        if (signalFilters.signal_types.includes('scale') && signalType === 'scale') return true
        if (signalFilters.signal_types.includes('alert') && signalType === 'alert') return true
      }

      return false
    })
  })

  // 가격 차트 데이터 로드 (선택된 심볼)
  const loadCandleData = async (symbol?: string) => {
    const targetSymbol = symbol || ui.selectedSymbol
    if (!targetSymbol) return

    // 이미 캐시에 있으면 스킵
    if (cache.candleData[targetSymbol]?.length > 0) return
    if (ui.isLoadingCandles) return

    setUI('isLoadingCandles', true)
    try {
      const data = await fetchCandlesForBacktest(
        targetSymbol,
        props.result.start_date,
        props.result.end_date
      )
      if (data) {
        // 차트용 변환 데이터 저장
        setCache('candleData', targetSymbol, convertCandlesToChartData(data.candles))
        // 원본 캔들 데이터도 저장 (볼륨 프로파일용)
        setCache('rawCandleData', targetSymbol, data.candles)
      }
    } catch (err) {
      logError('캔들 데이터 로드 실패:', err)
    } finally {
      setUI('isLoadingCandles', false)
    }
  }

  // 심볼 선택 핸들러
  const handleSymbolSelect = (symbol: string) => {
    setUI('selectedSymbol', symbol)
    // 선택된 심볼의 캔들 데이터 로드
    if (ui.showPriceChart) {
      loadCandleData(symbol)
    }
  }

  const equityCurve = () => convertEquityCurve(props.result)
  const drawdownCurve = () => convertDrawdownCurve(props.result)

  // 최종 자본 = equity_curve의 마지막 값 (미실현 손익 포함)
  // 차트와 지표가 일관되게 표시되도록 함
  const initialCapitalNum = () => parseFloat(props.result.config_summary.initial_capital)
  const finalCapital = () => {
    const curve = equityCurve()
    if (curve.length > 0) {
      return curve[curve.length - 1].value
    }
    // equity_curve가 없으면 기존 방식 사용
    return initialCapitalNum() + parseFloat(props.result.metrics.net_profit)
  }

  // 총 수익률도 equity_curve 기준으로 재계산
  const totalReturnPct = () => {
    const initial = initialCapitalNum()
    const final = finalCapital()
    if (initial <= 0) return parseFloat(props.result.metrics.total_return_pct)
    return ((final - initial) / initial) * 100
  }

  const handleDelete = (e: MouseEvent) => {
    e.stopPropagation()
    props.onDelete(props.index)
  }

  const toggleExpand = () => {
    setUI('isExpanded', !ui.isExpanded)
  }

  return (
    <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] overflow-hidden transition-all duration-200">
      {/* 클릭 가능한 헤더 */}
      <div
        class="p-6 cursor-pointer hover:bg-[var(--color-surface-light)]/30 transition-colors"
        onClick={toggleExpand}
      >
        <div class="flex items-start justify-between">
          <div class="flex items-center gap-3">
            {/* 펼침 아이콘 */}
            <div class="text-[var(--color-text-muted)]">
              <Show when={ui.isExpanded} fallback={<ChevronDown class="w-5 h-5" />}>
                <ChevronUp class="w-5 h-5" />
              </Show>
            </div>
            <div>
              <h4 class="text-lg font-semibold text-[var(--color-text)]">
                {props.strategies?.find((s: Strategy) => s.id === props.result.strategy_id)?.name || props.result.strategy_id}
              </h4>
              <div class="flex items-center gap-3 mt-1 text-sm text-[var(--color-text-muted)]">
                <div class="flex flex-wrap gap-1">
                  <For each={props.result.symbol.split(',').map(s => s.trim()).filter(s => s)}>
                    {(symbol) => (
                      <SymbolDisplay
                        ticker={symbol}
                        mode="inline"
                        size="sm"
                        autoFetch={true}
                      />
                    )}
                  </For>
                </div>
                <span class="flex items-center gap-1">
                  <Calendar class="w-4 h-4" />
                  {props.result.start_date} ~ {props.result.end_date}
                </span>
              </div>
            </div>
          </div>
          <div class="flex items-center gap-2">
            {/* 요약 수익률 (접혀있을 때도 보임) */}
            <div
              class={`px-3 py-1 rounded-lg text-sm font-semibold ${
                totalReturnPct() >= 0 ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400'
              }`}
            >
              {formatPercent(totalReturnPct())}
            </div>
            <span
              class={`px-3 py-1 rounded-full text-sm font-medium ${
                props.result.success
                  ? 'bg-green-500/20 text-green-400'
                  : 'bg-red-500/20 text-red-400'
              }`}
            >
              {props.result.success ? '완료' : '실패'}
            </span>
            {/* 삭제 버튼 */}
            <button
              onClick={handleDelete}
              class="p-1.5 rounded-lg hover:bg-red-500/20 text-[var(--color-text-muted)] hover:text-red-400 transition-colors"
              title="결과 삭제"
            >
              <X class="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* 접혀있을 때도 보이는 핵심 지표 요약 */}
        <Show when={!ui.isExpanded}>
          <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-8 gap-3 mt-4 ml-8 text-sm">
            <div>
              <span class="text-[var(--color-text-muted)]">초기자본</span>
              <div class="font-medium text-[var(--color-text)]">{formatCurrency(props.result.config_summary.initial_capital)}</div>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">최종자본</span>
              <div class="font-medium text-[var(--color-text)]">{formatCurrency(finalCapital())}</div>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">수익률</span>
              <div class={`font-medium ${totalReturnPct() >= 0 ? 'text-green-400' : 'text-red-400'}`}>{formatPercent(totalReturnPct())}</div>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">CAGR</span>
              <div class={`font-medium ${parseFloat(props.result.metrics.annualized_return_pct) >= 0 ? 'text-green-400' : 'text-red-400'}`}>{formatPercent(props.result.metrics.annualized_return_pct)}</div>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">MDD</span>
              <div class="font-medium text-red-400">{parseFloat(props.result.metrics.max_drawdown_pct).toFixed(2)}%</div>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">샤프</span>
              <div class="font-medium text-[var(--color-text)]">{parseFloat(props.result.metrics.sharpe_ratio).toFixed(2)}</div>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">승률</span>
              <div class="font-medium text-[var(--color-text)]">{parseFloat(props.result.metrics.win_rate_pct).toFixed(1)}%</div>
            </div>
            <div>
              <span class="text-[var(--color-text-muted)]">거래</span>
              <div class="font-medium text-[var(--color-text)]">{props.result.metrics.total_trades}회</div>
            </div>
          </div>
        </Show>
      </div>

      {/* 펼쳐진 상세 내용 */}
      <Show when={ui.isExpanded}>
        <div class="px-6 pb-6 border-t border-[var(--color-surface-light)]">
          {/* 성과 지표 그리드 */}
          <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-8 gap-4 pt-4">
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">초기 자본</div>
              <div class="font-semibold text-[var(--color-text)]">
                {formatCurrency(props.result.config_summary.initial_capital)}
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">최종 자본</div>
              <div class="font-semibold text-[var(--color-text)]">
                {formatCurrency(finalCapital())}
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">총 수익률</div>
              <div
                class={`font-semibold flex items-center gap-1 ${
                  totalReturnPct() >= 0 ? 'text-green-500' : 'text-red-500'
                }`}
              >
                <Show
                  when={totalReturnPct() >= 0}
                  fallback={<TrendingDown class="w-4 h-4" />}
                >
                  <TrendingUp class="w-4 h-4" />
                </Show>
                {formatPercent(totalReturnPct())}
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">CAGR</div>
              <div
                class={`font-semibold ${
                  parseFloat(props.result.metrics.annualized_return_pct) >= 0 ? 'text-green-500' : 'text-red-500'
                }`}
              >
                {formatPercent(props.result.metrics.annualized_return_pct)}
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">MDD</div>
              <div class="font-semibold text-red-500">
                {parseFloat(props.result.metrics.max_drawdown_pct).toFixed(2)}%
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">샤프 비율</div>
              <div class="font-semibold text-[var(--color-text)]">
                {parseFloat(props.result.metrics.sharpe_ratio).toFixed(2)}
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">승률</div>
              <div class="font-semibold text-[var(--color-text)]">
                {parseFloat(props.result.metrics.win_rate_pct).toFixed(1)}%
              </div>
            </div>
            <div>
              <div class="text-sm text-[var(--color-text-muted)] mb-1">총 거래</div>
              <div class="font-semibold text-[var(--color-text)]">
                {props.result.metrics.total_trades}회
              </div>
            </div>
          </div>

          {/* 추가 성과 지표 (접이식) */}
          <details class="mt-4">
            <summary class="cursor-pointer text-sm text-[var(--color-text-muted)] hover:text-[var(--color-text)]">
              상세 지표 보기
            </summary>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-4 mt-3 pt-3 border-t border-[var(--color-surface-light)]">
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">순수익</div>
                <div class={`font-semibold ${parseFloat(props.result.metrics.net_profit) >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                  {formatCurrency(props.result.metrics.net_profit)}
                </div>
              </div>
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">Profit Factor</div>
                <div class="font-semibold text-[var(--color-text)]">
                  {parseFloat(props.result.metrics.profit_factor).toFixed(2)}
                </div>
              </div>
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">소르티노 비율</div>
                <div class="font-semibold text-[var(--color-text)]">
                  {parseFloat(props.result.metrics.sortino_ratio).toFixed(2)}
                </div>
              </div>
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">칼마 비율</div>
                <div class="font-semibold text-[var(--color-text)]">
                  {parseFloat(props.result.metrics.calmar_ratio).toFixed(2)}
                </div>
              </div>
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">평균 수익</div>
                <div class="font-semibold text-green-500">
                  {formatCurrency(props.result.metrics.avg_win)}
                </div>
              </div>
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">평균 손실</div>
                <div class="font-semibold text-red-500">
                  {formatCurrency(props.result.metrics.avg_loss)}
                </div>
              </div>
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">총 수수료</div>
                <div class="font-semibold text-[var(--color-text)]">
                  {formatCurrency(props.result.config_summary.total_commission)}
                </div>
              </div>
              <div>
                <div class="text-sm text-[var(--color-text-muted)] mb-1">데이터 포인트</div>
                <div class="font-semibold text-[var(--color-text)]">
                  {props.result.config_summary.data_points}개
                </div>
              </div>
            </div>
          </details>

          {/* 가격 차트 + 매매 태그 (접이식) */}
          <Show when={props.result.trades.length > 0}>
            <details
              class="mt-4"
              onToggle={(e) => {
                if ((e.target as HTMLDetailsElement).open) {
                  setUI('showPriceChart', true)
                  loadCandleData()
                }
              }}
            >
              <summary class="cursor-pointer text-sm text-[var(--color-text-muted)] hover:text-[var(--color-text)] flex items-center gap-2">
                <LineChart class="w-4 h-4" />
                가격 차트 + 매매 태그
              </summary>
              <div class="mt-3 space-y-3">
                {/* 신호 필터 패널 (Lazy Loaded) */}
                <Suspense fallback={<div class="h-12 bg-gray-100 dark:bg-gray-800 animate-pulse rounded" />}>
                  <IndicatorFilterPanel
                    filters={chartState.signalFilters}
                    onChange={(filters) => setChartState('signalFilters', filters)}
                    defaultCollapsed={true}
                  />
                </Suspense>

                {/* 다중 심볼인 경우 심볼 선택 탭 표시 */}
                <Show when={symbols().length > 1}>
                  <div class="flex flex-wrap gap-1 p-1 bg-[var(--color-surface-light)]/30 rounded-lg">
                    <For each={symbols()}>
                      {(symbol) => (
                        <button
                          class={`px-3 py-1.5 text-xs font-medium rounded-md transition-all ${
                            ui.selectedSymbol === symbol
                              ? 'bg-[var(--color-primary)] text-white shadow-sm'
                              : 'text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)] hover:text-[var(--color-text)]'
                          }`}
                          onClick={(e) => {
                            e.stopPropagation()
                            handleSymbolSelect(symbol)
                          }}
                        >
                          {symbol}
                        </button>
                      )}
                    </For>
                  </div>
                </Show>

                {/* 필터 상태 요약 */}
                <Show when={chartState.signalFilters.signal_types.length > 0}>
                  <div class="text-xs text-[var(--color-text-muted)]">
                    표시 중: {filteredTradeMarkers().length} / {tradeMarkers().length} 마커
                  </div>
                </Show>

                {/* 볼륨 프로파일 토글 */}
                <div class="flex items-center gap-2 mb-2">
                  <label class="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)] cursor-pointer">
                    <input
                      type="checkbox"
                      checked={ui.showVolumeProfile}
                      onChange={(e) => setUI('showVolumeProfile', e.currentTarget.checked)}
                      class="w-3.5 h-3.5 rounded border-gray-500 text-blue-500 focus:ring-blue-500"
                    />
                    볼륨 프로파일 표시
                  </label>
                </div>

                <Show
                  when={!ui.isLoadingCandles && candleData().length > 0}
                  fallback={
                    <div class="h-[280px] flex items-center justify-center text-[var(--color-text-muted)]">
                      {ui.isLoadingCandles ? (
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
                    {/* 캔들 차트 (Lazy Loaded) */}
                    <div class="flex-1">
                      <Suspense fallback={<div class="h-[240px] bg-gray-100 dark:bg-gray-800 animate-pulse rounded" />}>
                        <SyncedChartPanel
                          data={candleData()}
                          type="candlestick"
                          mainHeight={240}
                          markers={filteredTradeMarkers()}
                          chartId="price"
                          {...chartSync.bindChart('price')}
                        />
                      </Suspense>
                    </div>

                    {/* 볼륨 프로파일 (Lazy Loaded) */}
                    <Show when={ui.showVolumeProfile && volumeProfileData().length > 0}>
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

          {/* 자산 곡선 & 드로우다운 차트 (Lazy Loaded, 동기화됨) */}
          <Show when={equityCurve().length > 0}>
            <div class="mt-4 space-y-4">
              <div>
                <h5 class="text-sm font-medium text-[var(--color-text-muted)] mb-2">자산 곡선</h5>
                <Suspense fallback={<div class="h-[200px] bg-gray-100 dark:bg-gray-800 animate-pulse rounded" />}>
                  <EquityCurve
                    data={equityCurve()}
                    height={200}
                    chartId="equity"
                    {...chartSync.bindChart('equity')}
                  />
                </Suspense>
              </div>
              <Show when={drawdownCurve().length > 0}>
                <div>
                  <h5 class="text-sm font-medium text-[var(--color-text-muted)] mb-2">드로우다운</h5>
                  <Suspense fallback={<div class="h-[150px] bg-gray-100 dark:bg-gray-800 animate-pulse rounded" />}>
                    <DrawdownChart
                      data={drawdownCurve()}
                      height={150}
                      chartId="drawdown"
                      {...chartSync.bindChart('drawdown')}
                    />
                  </Suspense>
                </div>
              </Show>
            </div>
          </Show>

          {/* 매매일지 버튼 - TradingJournal 페이지로 라우팅 (DB 저장된 결과만) */}
          <Show when={props.result.dbId && (props.result.trades.length > 0 || (props.result.all_trades && props.result.all_trades.length > 0))}>
            <div class="mt-4">
              <A
                href={`/journal?source=backtest&id=${props.result.dbId || props.result.id}&strategy=${encodeURIComponent(
                  props.strategies?.find(s => s.id === props.result.strategy_id)?.name || props.result.strategy_id
                )}`}
                class="inline-flex items-center gap-2 px-4 py-2 bg-[var(--color-primary)]/10 hover:bg-[var(--color-primary)]/20 text-[var(--color-primary)] rounded-lg transition-colors text-sm font-medium"
              >
                <BarChart3 class="w-4 h-4" />
                매매일지 열기
                <span class="text-xs opacity-75">
                  (라운드트립 {props.result.trades.length}건
                  {props.result.all_trades && props.result.all_trades.length > 0 && (
                    <> / 전체 {props.result.all_trades.length}건</>
                  )})
                </span>
              </A>
            </div>
          </Show>
        </div>
      </Show>
    </div>
  )
}

// 메인 Backtest 컴포넌트 상태 타입
interface FormState {
  selectedStrategy: string
  startDate: string
  endDate: string
  initialCapital: string
  slippageRate: string
}

interface BacktestUIState {
  isRunning: boolean
  error: string | null
}

interface MultiTfState {
  config: MultiTimeframeConfig | null
  enabled: boolean
}

export function Backtest() {
  // 기본 날짜 설정 (1년 전 ~ 오늘)
  const today = new Date().toISOString().split('T')[0]
  const oneYearAgo = new Date(Date.now() - 365 * 24 * 60 * 60 * 1000).toISOString().split('T')[0]

  // Store 기반 상태 관리
  const [form, setForm] = createStore<FormState>({
    selectedStrategy: '',
    startDate: oneYearAgo,
    endDate: today,
    initialCapital: '10000000',
    slippageRate: '0.05',
  })
  const [ui, setUI] = createStore<BacktestUIState>({
    isRunning: false,
    error: null,
  })
  const [multiTf, setMultiTf] = createStore<MultiTfState>({
    config: null,
    enabled: false,
  })
  const [results, setResults] = createStore<{ items: StoredBacktestResult[] }>({ items: [] })

  // 등록된 전략 목록 가져오기 (전략 페이지에서 등록된 전략만 표시)
  const [strategies] = createResource(async () => {
    return await getStrategies()
  })

  // 저장된 백테스트 결과 불러오기
  const [savedResults] = createResource(async () => {
    try {
      const response = await listBacktestResults({ limit: 100 })
      // DB 결과를 StoredBacktestResult 형태로 변환
      return response.results.map(r => ({
        ...r,
        dbId: r.id
      })) as StoredBacktestResult[]
    } catch (err) {
      logError('저장된 결과 불러오기 실패:', err)
      return []
    }
  })

  // 저장된 결과가 로드되면 상태에 반영
  createEffect(() => {
    const saved = savedResults()
    if (saved && saved.length > 0) {
      setResults('items', saved)
    }
  })

  // URL 파라미터 읽기 (전략 페이지에서 바로 이동 시)
  const [searchParams] = useSearchParams()

  // URL에서 전략 ID가 있으면 자동 선택
  createEffect(() => {
    const strategyId = searchParams.strategy
    if (strategyId && strategies() && strategies()!.length > 0) {
      const found = strategies()!.find(s => s.id === strategyId)
      if (found) {
        setForm('selectedStrategy', strategyId)
      }
    }
  })

  // 백테스트 전략 템플릿 (isMultiTimeframe 확인용)
  const [templates] = createResource(async () => {
    try {
      const response = await getBacktestStrategies()
      return response.strategies
    } catch {
      return []
    }
  })

  const handleRunBacktest = async (e: Event) => {
    e.preventDefault()
    setUI('error', null)

    const strategyType = getSelectedStrategyType()

    const strategyInfo = getSelectedStrategyInfo()
    if (!form.selectedStrategy || !strategyType || !strategyInfo) {
      setUI('error', '전략을 선택해주세요')
      return
    }
    if (!strategyInfo.symbols || strategyInfo.symbols.length === 0) {
      setUI('error', '전략에 심볼이 등록되어 있지 않습니다')
      return
    }
    if (!form.startDate) {
      setUI('error', '시작일을 선택해주세요')
      return
    }
    if (!form.endDate) {
      setUI('error', '종료일을 선택해주세요')
      return
    }

    setUI('isRunning', true)

    try {
      // 다중 자산 전략인지 확인 (strategyType 기준)
      const isMultiAssetStrategy = MULTI_ASSET_STRATEGIES.includes(strategyType)

      // 전략에 등록된 심볼 사용
      const symbols = strategyInfo.symbols

      // 슬리피지를 소수점으로 변환 (0.05% → 0.0005)
      const slippage = parseFloat(form.slippageRate) / 100

      let resultToSave: BacktestResult
      let symbolStr: string

      // 다중 타임프레임 설정 (활성화된 경우만)
      const multiTfConfigToUse = multiTf.enabled && multiTf.config ? multiTf.config : undefined

      if (isMultiAssetStrategy) {
        const request: BacktestMultiRequest = {
          strategy_id: strategyType,
          symbols,
          start_date: form.startDate,
          end_date: form.endDate,
          initial_capital: parseInt(form.initialCapital, 10),
          slippage_rate: slippage,
          multi_timeframe_config: multiTfConfigToUse,
        }

        const result = await runMultiBacktest(request)

        // 다중 자산 결과를 단일 자산 형식으로 변환하여 표시
        symbolStr = result.symbols.join(', ')
        resultToSave = {
          ...result,
          symbol: symbolStr,
        }
      } else {
        // 단일 자산 전략 (첫 번째 심볼 사용)
        const request: BacktestRequest = {
          strategy_id: strategyType,
          symbol: symbols[0],
          start_date: form.startDate,
          end_date: form.endDate,
          initial_capital: parseInt(form.initialCapital, 10),
          slippage_rate: slippage,
          multi_timeframe_config: multiTfConfigToUse,
        }

        resultToSave = await runBacktest(request)
        symbolStr = symbols[0]
      }

      // DB에 결과 저장
      try {
        const saveResponse = await saveBacktestResult({
          strategy_id: form.selectedStrategy,  // 등록된 전략 ID
          strategy_type: strategyType,      // 전략 타입 (sma_crossover, bollinger 등)
          symbol: symbolStr,
          start_date: form.startDate,
          end_date: form.endDate,
          initial_capital: parseInt(form.initialCapital, 10),
          slippage_rate: slippage,
          metrics: resultToSave.metrics,
          config_summary: resultToSave.config_summary,
          equity_curve: resultToSave.equity_curve,
          trades: resultToSave.trades,
          success: resultToSave.success,
        })

        // DB ID를 결과에 추가하여 저장
        const storedResult: StoredBacktestResult = {
          ...resultToSave,
          dbId: saveResponse.id,
        }
        setResults('items', items => [storedResult, ...items])
        log('백테스트 결과 저장됨:', saveResponse.id)
      } catch (saveErr) {
        logError('백테스트 결과 저장 실패:', saveErr)
        // 저장 실패해도 결과는 표시 (dbId 없이)
        setResults('items', items => [resultToSave, ...items])
      }
    } catch (err) {
      logError('백테스트 실행 실패:', err)
      setUI('error', err instanceof Error ? err.message : '백테스트 실행에 실패했습니다')
    } finally {
      setUI('isRunning', false)
    }
  }

  // 전략 선택
  const handleStrategyChange = (strategyId: string) => {
    setForm('selectedStrategy', strategyId)
    // 다중 타임프레임 설정 초기화
    const strategy = strategies()?.find((s: Strategy) => s.id === strategyId)
    if (strategy) {
      const template = templates()?.find(t => t.id === strategy.strategyType)
      if (template?.isMultiTimeframe && template.defaultMultiTimeframeConfig) {
        setMultiTf('config', template.defaultMultiTimeframeConfig)
        setMultiTf('enabled', true)
      } else {
        setMultiTf('config', null)
        setMultiTf('enabled', false)
      }
    }
  }

  // 선택된 전략 정보 가져오기
  const getSelectedStrategyInfo = (): Strategy | undefined => {
    return strategies()?.find((s: Strategy) => s.id === form.selectedStrategy)
  }

  // 선택된 전략의 strategyType 가져오기 (백테스트 API에서 사용)
  const getSelectedStrategyType = (): string | undefined => {
    return getSelectedStrategyInfo()?.strategyType
  }

  // 선택된 전략의 템플릿 가져오기 (isMultiTimeframe 확인용)
  const getSelectedTemplate = (): BacktestStrategy | undefined => {
    const strategyType = getSelectedStrategyType()
    if (!strategyType) return undefined
    return templates()?.find(t => t.id === strategyType)
  }

  return (
    <div class="space-y-6">
      {/* 백테스트 설정 */}
      <Card>
        <CardHeader>
          <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
            <Settings2 class="w-5 h-5" />
            백테스트 설정
          </h3>
        </CardHeader>
        <CardContent>
          {/* 에러 메시지 */}
          <Show when={ui.error}>
            <div class="mb-4 p-3 rounded-lg bg-red-500/20 border border-red-500/30 text-red-400 flex items-center gap-2">
              <AlertCircle class="w-5 h-5 flex-shrink-0" />
              <span>{ui.error}</span>
            </div>
          </Show>

          {/* 등록된 전략 없음 안내 */}
          <Show when={!strategies.loading && (!strategies() || strategies()!.length === 0)}>
            <div class="mb-4 p-4 rounded-lg bg-blue-500/20 border border-blue-500/30 text-blue-300 flex items-start gap-3">
              <Info class="w-5 h-5 flex-shrink-0 mt-0.5" />
              <div>
                <p class="font-medium">등록된 전략이 없습니다</p>
                <p class="text-sm mt-1 text-blue-300/80">
                  백테스트를 실행하려면 먼저 전략 페이지에서 전략을 등록하세요.
                  등록된 전략의 파라미터로 백테스트가 실행됩니다.
                </p>
              </div>
            </div>
          </Show>

        <form onSubmit={handleRunBacktest} class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {/* 전략 선택 */}
          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">전략</label>
            <Show
              when={!strategies.loading}
              fallback={
                <div class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] text-[var(--color-text-muted)]">
                  로딩 중...
                </div>
              }
            >
              <select
                value={form.selectedStrategy}
                onChange={(e) => handleStrategyChange(e.currentTarget.value)}
                class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
              >
                <option value="">전략 선택</option>
                <For each={strategies()}>
                  {(strategy: Strategy) => (
                    <option value={strategy.id}>
                      {strategy.name} ({strategy.strategyType})
                    </option>
                  )}
                </For>
              </select>
            </Show>
            <Show when={getSelectedStrategyInfo()}>
              <p class="mt-1 text-xs text-[var(--color-text-muted)]">
                전략 타입: {getSelectedStrategyInfo()?.strategyType}
              </p>
            </Show>
          </div>

          {/* 심볼 (읽기 전용 - 전략에서 가져옴) */}
          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">심볼</label>
            <div class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)]/50 border border-[var(--color-surface-light)] text-[var(--color-text-muted)]">
              <Show
                when={getSelectedStrategyInfo()?.symbols && getSelectedStrategyInfo()!.symbols.length > 0}
                fallback={<span class="text-[var(--color-text-muted)]/50">전략을 선택하세요</span>}
              >
                <div class="flex flex-wrap gap-1">
                  <For each={getSelectedStrategyInfo()?.symbols}>
                    {(symbol) => (
                      <SymbolDisplay
                        ticker={symbol}
                        mode="inline"
                        size="sm"
                        autoFetch={true}
                      />
                    )}
                  </For>
                </div>
              </Show>
            </div>
          </div>

          {/* 초기 자본 */}
          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">초기 자본 (KRW)</label>
            <input
              type="number"
              value={form.initialCapital}
              onInput={(e) => setForm('initialCapital', e.currentTarget.value)}
              class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
              placeholder="10000000"
              min="100000"
            />
          </div>

          {/* 슬리피지 */}
          <div>
            <label class="block text-sm text-[var(--color-text-muted)] mb-1">슬리피지 (%)</label>
            <input
              type="number"
              value={form.slippageRate}
              onInput={(e) => setForm('slippageRate', e.currentTarget.value)}
              class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
              placeholder="0.05"
              min="0"
              max="5"
              step="0.01"
            />
            <p class="mt-1 text-xs text-[var(--color-text-muted)]">
              거래 시 예상 체결가와의 차이
            </p>
          </div>

          {/* 다중 타임프레임 설정 (지원 전략만 표시) */}
          <Show when={getSelectedTemplate()?.isMultiTimeframe}>
            <div class="md:col-span-2 lg:col-span-3">
              <div class="p-4 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg">
                <div class="flex items-center justify-between mb-4">
                  <div class="flex items-center gap-2">
                    <Clock class="w-5 h-5 text-[var(--color-primary)]" />
                    <span class="font-medium text-[var(--color-text)]">다중 타임프레임 백테스트</span>
                  </div>
                  <label class="flex items-center gap-2 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={multiTf.enabled}
                      onChange={(e) => {
                        const enabled = e.currentTarget.checked
                        setMultiTf('enabled', enabled)
                        if (enabled && !multiTf.config) {
                          // 기본값 설정
                          const defaultConfig = getSelectedTemplate()?.defaultMultiTimeframeConfig
                          if (defaultConfig) {
                            setMultiTf('config', defaultConfig)
                          } else {
                            setMultiTf('config', {
                              primary: '5m',
                              secondary: [],
                            })
                          }
                        }
                      }}
                      class="w-4 h-4 text-[var(--color-primary)] rounded focus:ring-[var(--color-primary)]"
                    />
                    <span class="text-sm text-[var(--color-text-muted)]">활성화</span>
                  </label>
                </div>
                <Show when={multiTf.enabled}>
                  <MultiTimeframeSelector
                    primaryTimeframe={multiTf.config?.primary || '5m'}
                    secondaryTimeframes={(multiTf.config?.secondary || []).map(s => s.timeframe)}
                    onPrimaryChange={(tf) => {
                      setMultiTf('config', prev => prev ? {
                        ...prev,
                        primary: tf,
                        // Primary보다 작은 Secondary는 제거
                        secondary: prev.secondary.filter(s => {
                          const tfOrder: Timeframe[] = ['1m', '5m', '15m', '30m', '1h', '4h', '1d', '1w', '1M']
                          return tfOrder.indexOf(s.timeframe) > tfOrder.indexOf(tf)
                        }),
                      } : { primary: tf, secondary: [] })
                    }}
                    onSecondaryChange={(tfs) => {
                      setMultiTf('config', prev => prev ? {
                        ...prev,
                        secondary: tfs.map(tf => ({ timeframe: tf, candle_count: 100 })),
                      } : { primary: '5m', secondary: tfs.map(tf => ({ timeframe: tf, candle_count: 100 })) })
                    }}
                    maxSecondary={3}
                  />
                  <p class="mt-3 text-xs text-[var(--color-text-muted)]">
                    Primary 타임프레임 캔들이 완료될 때마다 Secondary 데이터와 함께 전략을 평가합니다.
                  </p>
                </Show>
              </div>
            </div>
          </Show>

          {/* 기간 선택 */}
          <div class="col-span-2">
            <DateRangePicker
              label="백테스트 기간"
              startDate={form.startDate || oneYearAgo}
              endDate={form.endDate || today}
              onStartDateChange={(v) => setForm('startDate', v)}
              onEndDateChange={(v) => setForm('endDate', v)}
              showPresets
              size="md"
            />
          </div>

          {/* 실행 버튼 */}
          <div class="flex items-end">
            <Button
              type="submit"
              disabled={ui.isRunning}
              className="w-full flex items-center justify-center gap-2"
            >
              <Show when={ui.isRunning} fallback={<Play class="w-5 h-5" />}>
                <div class="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin" />
              </Show>
              {ui.isRunning ? '실행 중...' : '백테스트 실행'}
            </Button>
          </div>
        </form>
        </CardContent>
      </Card>

      {/* 결과 */}
      <div class="space-y-4">
        <div class="flex items-center justify-between">
          <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
            <ChartBar class="w-5 h-5" />
            백테스트 결과
          </h3>
          <Show when={results.items.length > 0}>
            <button
              onClick={async () => {
                // DB에서 모든 결과 삭제
                const currentResults = results.items
                for (const result of currentResults) {
                  if (result.id) {
                    try {
                      await deleteBacktestResult(result.id)
                    } catch (e) {
                      logError('결과 삭제 실패:', result.id, e)
                    }
                  }
                }
                // 로컬 상태 초기화
                setResults('items', [])
              }}
              class="text-sm text-[var(--color-text-muted)] hover:text-[var(--color-text)] flex items-center gap-1"
            >
              <RefreshCw class="w-4 h-4" />
              결과 초기화
            </button>
          </Show>
        </div>

        <Show
          when={results.items.length > 0}
          fallback={
            <Card>
              <CardContent>
                <EmptyState
                  icon="📊"
                  title="아직 백테스트 결과가 없습니다"
                  description="위에서 전략과 설정을 선택하고 백테스트를 실행해주세요"
                />
              </CardContent>
            </Card>
          }
        >
          <div class="grid grid-cols-1 gap-4">
            <For each={results.items}>
              {(result, index) => (
                <BacktestResultCard
                  result={result}
                  strategies={strategies()}
                  index={index()}
                  onDelete={async (idx) => {
                    const target = results.items[idx] as StoredBacktestResult
                    // DB에 저장된 결과라면 API 호출하여 삭제
                    if (target.dbId) {
                      try {
                        await deleteBacktestResult(target.dbId)
                        log('백테스트 결과 삭제됨:', target.dbId)
                      } catch (err) {
                        logError('백테스트 결과 삭제 실패:', err)
                        // 삭제 실패해도 UI에서는 제거
                      }
                    }
                    setResults('items', items => items.filter((_, i) => i !== idx))
                  }}
                />
              )}
            </For>
          </div>
        </Show>
      </div>
    </div>
  )
}

export default Backtest
