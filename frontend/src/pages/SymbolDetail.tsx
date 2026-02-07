/**
 * 종목 상세 페이지
 *
 * 개별 종목의 상세 정보, 가격 차트, 신호 내역을 표시합니다.
 */
import { createSignal, createResource, createMemo, For, Show, onMount } from 'solid-js'
import { useParams, useSearchParams } from '@solidjs/router'
import { ArrowLeft, TrendingUp, TrendingDown, RefreshCw, Calendar, BarChart3, Activity, LineChart } from 'lucide-solid'
import {
  Card,
  CardHeader,
  CardContent,
  StatCard,
  StatCardGrid,
  EmptyState,
  ErrorState,
  PageHeader,
  Button,
  SignalMarkerOverlay,
} from '../components/ui'
import { SyncedChartPanel, VolumeProfile, VolumeProfileLegend, ScoreHistoryChart } from '../components/charts'
import type { CandlestickDataPoint, TradeMarker, PriceVolume } from '../components/charts'
import type { SignalMarker, SignalIndicators } from '../types'
import {
  getSymbolSignals,
  getScoreHistory,
  getSymbolSignalPerformance,
  getSignalScatterData,
  getKlines,
  type SignalMarkerDto,
  type SymbolSignalsQuery,
  type ScoreHistorySummary,
  type CandleData,
} from '../api/client'
import type { SignalSymbolStats, SignalReturnPoint } from '../types/generated/signals'
import { SymbolDisplay } from '../components/SymbolDisplay'
import { createLogger } from '../utils/logger'
import { formatCurrency, formatNumber, formatDate, formatDateTime } from '../utils/format'

const { error: logError } = createLogger('SymbolDetail')

// ==================== API ====================

/** 캔들 데이터 조회 (Redis 캐시 기반 /market/klines 사용) */
async function fetchCandles(
  symbol: string,
  days: number = 90
): Promise<CandleData[] | null> {
  try {
    const response = await getKlines({ symbol, timeframe: '1d', limit: days })
    return response.data
  } catch {
    return null
  }
}

function convertCandlesToChartData(candles: CandleData[]): CandlestickDataPoint[] {
  const uniqueMap = new Map<string, CandlestickDataPoint>()

  candles.forEach(c => {
    uniqueMap.set(c.time, {
      time: c.time,
      open: c.open,
      high: c.high,
      low: c.low,
      close: c.close,
    })
  })

  return Array.from(uniqueMap.values()).sort((a, b) =>
    (a.time as string).localeCompare(b.time as string)
  )
}

function calculateVolumeProfile(candles: CandleData[], bucketCount = 20): PriceVolume[] {
  if (candles.length === 0) return []

  let minPrice = Infinity
  let maxPrice = -Infinity
  candles.forEach(c => {
    if (c.low < minPrice) minPrice = c.low
    if (c.high > maxPrice) maxPrice = c.high
  })

  if (minPrice === maxPrice) return []

  const priceStep = (maxPrice - minPrice) / bucketCount
  const buckets: Map<number, number> = new Map()

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

        const currentVolume = buckets.get(bucketMid) || 0
        buckets.set(bucketMid, currentVolume + c.volume * overlapRatio)
      }
    }
  })

  const result: PriceVolume[] = []
  buckets.forEach((volume, price) => {
    result.push({ price, volume })
  })

  return result.sort((a, b) => a.price - b.price)
}

function convertSignalsToMarkers(signals: SignalMarkerDto[]): TradeMarker[] {
  return signals.map(signal => ({
    time: signal.timestamp.split('T')[0],
    type: signal.side === 'Buy' ? 'buy' : signal.side === 'Sell' ? 'sell' : 'buy',
    price: parseFloat(signal.price),
    label: signal.side || signal.signal_type,
  })).sort((a, b) => (a.time as string).localeCompare(b.time as string))
}

function convertToSignalMarker(dto: SignalMarkerDto): SignalMarker {
  return {
    id: dto.id,
    symbol: dto.symbol,
    timestamp: dto.timestamp,
    signal_type: dto.signal_type,
    side: dto.side as 'Buy' | 'Sell' | undefined,
    price: parseFloat(dto.price),
    strength: parseFloat(dto.strength),
    executed: dto.executed,
    reason: dto.reason,
    indicators: dto.indicators as SignalIndicators,
  }
}

// ==================== 메인 컴포넌트 ====================

export function SymbolDetail() {
  const params = useParams<{ symbol: string }>()
  const [searchParams] = useSearchParams()

  // 상태 관리
  const [candleData, setCandleData] = createSignal<CandlestickDataPoint[]>([])
  const [rawCandles, setRawCandles] = createSignal<CandleData[]>([])
  const [isLoadingCandles, setIsLoadingCandles] = createSignal(false)
  const [showVolumeProfile, setShowVolumeProfile] = createSignal(true)
  const [selectedDays, setSelectedDays] = createSignal(90)
  const [signalFilter, setSignalFilter] = createSignal<'all' | 'buy' | 'sell'>('all')

  // 현재 심볼 (URL 파라미터에서)
  const symbol = () => params.symbol || ''
  const exchange = () => searchParams.exchange || 'KRX'

  // Score History 데이터 로드
  const [scoreHistoryResource] = createResource(
    () => ({ symbol: symbol(), days: selectedDays() }),
    async ({ symbol, days }) => {
      if (!symbol) return { symbol: '', history: [], total: 0 }

      try {
        return await getScoreHistory(symbol, { days })
      } catch (err) {
        logError('점수 히스토리 조회 실패:', err)
        return { symbol: '', history: [], total: 0 }
      }
    }
  )

  // 신호 데이터 로드
  const [signalsResource] = createResource(
    () => ({ symbol: symbol(), exchange: exchange() }),
    async ({ symbol, exchange }) => {
      if (!symbol) return { total: 0, signals: [] }

      // 최근 90일 신호 조회
      const startDate = new Date()
      startDate.setDate(startDate.getDate() - selectedDays())

      const query: SymbolSignalsQuery = {
        symbol,
        exchange,
        start_time: startDate.toISOString(),
        limit: 100,
      }

      try {
        return await getSymbolSignals(query)
      } catch (err) {
        logError('신호 조회 실패:', err)
        return { total: 0, signals: [] }
      }
    }
  )

  // 신호 성과 데이터 로드
  const [signalPerformanceResource] = createResource(
    () => symbol(),
    async (ticker) => {
      if (!ticker) return null

      try {
        return await getSymbolSignalPerformance(ticker)
      } catch (err) {
        logError('신호 성과 조회 실패:', err)
        return null
      }
    }
  )

  // 신호-수익률 산점도 데이터
  const [scatterDataResource] = createResource(
    () => symbol(),
    async (ticker) => {
      if (!ticker) return []

      try {
        return await getSignalScatterData({ ticker, limit: 100 })
      } catch (err) {
        logError('산점도 데이터 조회 실패:', err)
        return []
      }
    }
  )

  // 캔들 데이터 로드
  const loadCandleData = async () => {
    if (!symbol()) return

    setIsLoadingCandles(true)
    try {
      const candles = await fetchCandles(symbol(), selectedDays())
      if (candles && candles.length > 0) {
        setCandleData(convertCandlesToChartData(candles))
        setRawCandles(candles)
      }
    } catch (err) {
      logError('캔들 데이터 로드 실패:', err)
    } finally {
      setIsLoadingCandles(false)
    }
  }

  // 초기 로드
  onMount(() => {
    loadCandleData()
  })

  // 볼륨 프로파일 계산
  const volumeProfileData = createMemo(() => {
    const raw = rawCandles()
    if (raw.length === 0) return []
    return calculateVolumeProfile(raw, 25)
  })

  // 현재가
  const currentPrice = createMemo(() => {
    const data = candleData()
    if (data.length === 0) return 0
    return data[data.length - 1].close
  })

  // 가격 범위
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

  // 가격 변동
  const priceChange = createMemo(() => {
    const data = candleData()
    if (data.length < 2) return { value: 0, percent: 0 }
    const first = data[0].close
    const last = data[data.length - 1].close
    return {
      value: last - first,
      percent: ((last - first) / first) * 100,
    }
  })

  // 필터링된 신호
  const filteredSignals = createMemo(() => {
    const signals = signalsResource()?.signals || []
    const filter = signalFilter()
    if (filter === 'all') return signals
    return signals.filter(s => s.side?.toLowerCase() === filter)
  })

  // 차트용 마커
  const tradeMarkers = createMemo(() => {
    return convertSignalsToMarkers(filteredSignals())
  })

  // SignalMarkerOverlay용 변환
  const signalMarkers = createMemo((): SignalMarker[] => {
    return filteredSignals().map(convertToSignalMarker)
  })

  // 신호 통계
  const signalStats = createMemo(() => {
    const signals = signalsResource()?.signals || []

    const buyCount = signals.filter(s => s.side === 'Buy').length
    const sellCount = signals.filter(s => s.side === 'Sell').length
    const executedCount = signals.filter(s => s.executed).length

    // 타입별 카운트
    const typeCount: Record<string, number> = {}
    signals.forEach(s => {
      typeCount[s.signal_type] = (typeCount[s.signal_type] || 0) + 1
    })

    return { buyCount, sellCount, executedCount, typeCount, total: signals.length }
  })

  return (
    <div class="space-y-6">
      {/* 페이지 헤더 */}
      <PageHeader
        title={
          <div class="flex items-center gap-3">
            <a
              href="/screening"
              class="p-2 hover:bg-[var(--color-surface-light)] rounded-lg transition-colors"
              title="스크리닝으로 돌아가기"
            >
              <ArrowLeft class="w-5 h-5" />
            </a>
            <SymbolDisplay
              ticker={symbol()}
              mode="full"
              size="lg"
              autoFetch={true}
            />
          </div>
        }
        description={`${exchange()} · 최근 ${selectedDays()}일`}
        actions={
          <div class="flex items-center gap-2">
            {/* 기간 선택 */}
            <select
              value={selectedDays()}
              onChange={(e) => {
                setSelectedDays(parseInt(e.currentTarget.value))
                loadCandleData()
              }}
              class="px-3 py-2 text-sm rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)]"
            >
              <option value="30">30일</option>
              <option value="60">60일</option>
              <option value="90">90일</option>
              <option value="180">180일</option>
              <option value="365">1년</option>
            </select>

            <Button
              variant="secondary"
              onClick={loadCandleData}
              loading={isLoadingCandles()}
            >
              <RefreshCw class="w-4 h-4" />
            </Button>
          </div>
        }
      />

      {/* 통계 카드 */}
      <StatCardGrid columns={4}>
        <StatCard
          label="현재가"
          value={formatCurrency(currentPrice())}
          icon="💰"
        />
        <StatCard
          label={`${selectedDays()}일 변동`}
          value={`${priceChange().value >= 0 ? '+' : ''}${formatCurrency(priceChange().value)}`}
          icon={priceChange().value >= 0 ? '📈' : '📉'}
          valueColor={priceChange().value >= 0 ? 'text-green-500' : 'text-red-500'}
        />
        <StatCard
          label="변동률"
          value={`${priceChange().percent >= 0 ? '+' : ''}${formatNumber(priceChange().percent)}%`}
          icon={priceChange().percent >= 0 ? '🚀' : '⬇️'}
          valueColor={priceChange().percent >= 0 ? 'text-green-500' : 'text-red-500'}
        />
        <StatCard
          label="신호 발생"
          value={`${signalStats().total}건`}
          icon="🔔"
        />
      </StatCardGrid>

      {/* 가격 차트 + 신호 마커 */}
      <Card>
        <CardHeader>
          <div class="flex items-center justify-between">
            <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
              <BarChart3 class="w-5 h-5" />
              가격 차트
            </h3>
            <div class="flex items-center gap-4">
              {/* 신호 필터 */}
              <div class="flex items-center gap-1">
                {(['all', 'buy', 'sell'] as const).map(filter => (
                  <button
                    class={`px-3 py-1 text-xs rounded-lg transition-colors ${
                      signalFilter() === filter
                        ? filter === 'buy'
                          ? 'bg-green-500/20 text-green-400 border border-green-500/30'
                          : filter === 'sell'
                          ? 'bg-red-500/20 text-red-400 border border-red-500/30'
                          : 'bg-[var(--color-primary)]/20 text-[var(--color-primary)] border border-[var(--color-primary)]/30'
                        : 'bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                    }`}
                    onClick={() => setSignalFilter(filter)}
                  >
                    {filter === 'all' ? '전체' : filter === 'buy' ? '매수' : '매도'}
                    <span class="ml-1 opacity-70">
                      ({filter === 'all' ? signalStats().total : filter === 'buy' ? signalStats().buyCount : signalStats().sellCount})
                    </span>
                  </button>
                ))}
              </div>

              {/* 볼륨 프로파일 토글 */}
              <label class="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)] cursor-pointer">
                <input
                  type="checkbox"
                  checked={showVolumeProfile()}
                  onChange={(e) => setShowVolumeProfile(e.currentTarget.checked)}
                  class="w-3.5 h-3.5 rounded"
                />
                볼륨 프로파일
              </label>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <Show
            when={!isLoadingCandles() && candleData().length > 0}
            fallback={
              <EmptyState
                icon={isLoadingCandles() ? '⏳' : '📊'}
                title={isLoadingCandles() ? '차트 로딩 중...' : '차트 데이터 없음'}
                description={isLoadingCandles() ? undefined : '데이터셋을 먼저 다운로드하세요'}
                className="h-[300px] flex flex-col items-center justify-center"
              />
            }
          >
            <div class="flex gap-2">
              {/* 캔들 차트 */}
              <div class="flex-1">
                <SyncedChartPanel
                  data={candleData()}
                  type="candlestick"
                  mainHeight={300}
                  markers={tradeMarkers()}
                />
              </div>

              {/* 볼륨 프로파일 */}
              <Show when={showVolumeProfile() && volumeProfileData().length > 0}>
                <div class="flex flex-col">
                  <VolumeProfile
                    priceVolumes={volumeProfileData()}
                    currentPrice={currentPrice()}
                    chartHeight={300}
                    width={80}
                    priceRange={chartPriceRange()}
                    showPoc={true}
                    showValueArea={true}
                  />
                  <VolumeProfileLegend class="mt-1" />
                </div>
              </Show>
            </div>
          </Show>
        </CardContent>
      </Card>

      <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* 신호 내역 */}
        <Card>
          <CardHeader>
            <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
              <Activity class="w-5 h-5" />
              최근 신호 내역
            </h3>
          </CardHeader>
          <CardContent>
            <Show
              when={filteredSignals().length > 0}
              fallback={
                <EmptyState
                  icon="🔔"
                  title="신호 없음"
                  description="선택한 기간에 발생한 신호가 없습니다"
                  className="py-8"
                />
              }
            >
              <div class="space-y-2 max-h-[400px] overflow-y-auto">
                <For each={[...filteredSignals()].reverse().slice(0, 20)}>
                  {(signal) => {
                    const isBuy = signal.side === 'Buy'
                    return (
                      <div class="flex items-center justify-between p-3 bg-[var(--color-surface-light)] rounded-lg">
                        <div class="flex items-center gap-3">
                          <span
                            class={`px-2 py-1 text-xs rounded font-medium ${
                              isBuy
                                ? 'bg-green-500/20 text-green-400'
                                : 'bg-red-500/20 text-red-400'
                            }`}
                          >
                            {isBuy ? '매수' : '매도'}
                          </span>
                          <div>
                            <div class="text-sm text-[var(--color-text)]">
                              {signal.signal_type}
                            </div>
                            <div class="text-xs text-[var(--color-text-muted)]">
                              {formatDateTime(signal.timestamp)}
                            </div>
                          </div>
                        </div>
                        <div class="text-right">
                          <div class="text-sm font-medium text-[var(--color-text)]">
                            {formatCurrency(signal.price)}
                          </div>
                          <div class="text-xs text-[var(--color-text-muted)]">
                            강도: {formatNumber(parseFloat(signal.strength) * 100, 0)}%
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

        {/* 신호 통계 */}
        <Card>
          <CardHeader>
            <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
              <Calendar class="w-5 h-5" />
              신호 발생 통계
            </h3>
          </CardHeader>
          <CardContent>
            <Show
              when={signalStats().total > 0}
              fallback={
                <EmptyState
                  icon="📊"
                  title="통계 없음"
                  description="신호 데이터가 없어 통계를 계산할 수 없습니다"
                  className="py-8"
                />
              }
            >
              <div class="space-y-4">
                {/* 매수/매도 비율 */}
                <div>
                  <div class="text-sm text-[var(--color-text-muted)] mb-2">매수/매도 비율</div>
                  <div class="flex gap-2 items-center">
                    <div
                      class="h-4 bg-green-500/50 rounded-l"
                      style={{
                        width: `${(signalStats().buyCount / signalStats().total) * 100}%`,
                        'min-width': signalStats().buyCount > 0 ? '20px' : '0px',
                      }}
                    />
                    <div
                      class="h-4 bg-red-500/50 rounded-r"
                      style={{
                        width: `${(signalStats().sellCount / signalStats().total) * 100}%`,
                        'min-width': signalStats().sellCount > 0 ? '20px' : '0px',
                      }}
                    />
                  </div>
                  <div class="flex justify-between text-xs mt-1">
                    <span class="text-green-400">매수 {signalStats().buyCount}건</span>
                    <span class="text-red-400">매도 {signalStats().sellCount}건</span>
                  </div>
                </div>

                {/* 체결률 */}
                <div>
                  <div class="text-sm text-[var(--color-text-muted)] mb-2">체결률</div>
                  <div class="flex items-center gap-2">
                    <div class="flex-1 h-2 bg-[var(--color-surface)] rounded-full overflow-hidden">
                      <div
                        class="h-full bg-blue-500"
                        style={{
                          width: `${(signalStats().executedCount / signalStats().total) * 100}%`,
                        }}
                      />
                    </div>
                    <span class="text-sm text-[var(--color-text)]">
                      {formatNumber((signalStats().executedCount / signalStats().total) * 100, 1)}%
                    </span>
                  </div>
                  <div class="text-xs text-[var(--color-text-muted)] mt-1">
                    {signalStats().executedCount}건 체결 / {signalStats().total}건 발생
                  </div>
                </div>

                {/* 신호 타입별 통계 */}
                <div>
                  <div class="text-sm text-[var(--color-text-muted)] mb-2">신호 타입별</div>
                  <div class="space-y-2">
                    <For each={Object.entries(signalStats().typeCount).sort((a, b) => b[1] - a[1])}>
                      {([type, count]) => (
                        <div class="flex items-center justify-between p-2 bg-[var(--color-surface-light)] rounded-lg">
                          <span class="text-sm text-[var(--color-text)]">{type}</span>
                          <div class="flex items-center gap-2">
                            <div
                              class="h-1.5 bg-[var(--color-primary)] rounded"
                              style={{
                                width: `${(count / signalStats().total) * 60}px`,
                              }}
                            />
                            <span class="text-sm font-medium text-[var(--color-text)]">
                              {count}건
                            </span>
                          </div>
                        </div>
                      )}
                    </For>
                  </div>
                </div>
              </div>
            </Show>
          </CardContent>
        </Card>
      </div>

      {/* 신호 성과 분석 */}
      <Card>
        <CardHeader>
          <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
            <TrendingUp class="w-5 h-5" />
            신호 성과 분석
          </h3>
        </CardHeader>
        <CardContent>
          <Show
            when={!signalPerformanceResource.loading && signalPerformanceResource()}
            fallback={
              <Show
                when={signalPerformanceResource.loading}
                fallback={
                  <EmptyState
                    icon="📊"
                    title="성과 데이터 없음"
                    description="이 종목의 신호 성과 기록이 아직 없습니다. 신호 발생 후 일정 기간이 지나면 성과가 계산됩니다."
                    className="py-8"
                  />
                }
              >
                <div class="py-8 text-center text-[var(--color-text-muted)]">
                  성과 데이터 로딩 중...
                </div>
              </Show>
            }
          >
            {(perf) => (
              <div class="space-y-6">
                {/* 핵심 지표 */}
                <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                  <div class="p-4 bg-[var(--color-surface-light)] rounded-lg">
                    <div class="text-xs text-[var(--color-text-muted)] mb-1">승률</div>
                    <div class={`text-xl font-bold ${
                      (perf().win_rate ?? 0) >= 50 ? 'text-green-500' : 'text-red-500'
                    }`}>
                      {perf().win_rate != null ? `${perf().win_rate.toFixed(1)}%` : '-'}
                    </div>
                  </div>
                  <div class="p-4 bg-[var(--color-surface-light)] rounded-lg">
                    <div class="text-xs text-[var(--color-text-muted)] mb-1">평균 5일 수익률</div>
                    <div class={`text-xl font-bold ${
                      (perf().avg_return_5d ?? 0) >= 0 ? 'text-green-500' : 'text-red-500'
                    }`}>
                      {perf().avg_return_5d != null ? `${perf().avg_return_5d >= 0 ? '+' : ''}${perf().avg_return_5d.toFixed(2)}%` : '-'}
                    </div>
                  </div>
                  <div class="p-4 bg-[var(--color-surface-light)] rounded-lg">
                    <div class="text-xs text-[var(--color-text-muted)] mb-1">평균 신호 강도</div>
                    <div class="text-xl font-bold text-[var(--color-primary)]">
                      {perf().avg_strength != null ? `${(perf().avg_strength * 100).toFixed(0)}%` : '-'}
                    </div>
                  </div>
                  <div class="p-4 bg-[var(--color-surface-light)] rounded-lg">
                    <div class="text-xs text-[var(--color-text-muted)] mb-1">분석된 신호</div>
                    <div class="text-xl font-bold text-[var(--color-text)]">
                      {perf().total_signals}건
                    </div>
                  </div>
                </div>

                {/* 매수/매도 성과 비교 */}
                <div class="grid grid-cols-2 gap-4">
                  <div class="p-4 bg-green-500/10 border border-green-500/30 rounded-lg">
                    <div class="flex items-center gap-2 mb-2">
                      <TrendingUp class="w-4 h-4 text-green-500" />
                      <span class="text-sm font-medium text-green-500">매수 신호</span>
                    </div>
                    <div class="text-2xl font-bold text-green-500 mb-1">
                      {perf().buy_count}건
                    </div>
                    <div class="text-xs text-[var(--color-text-muted)]">
                      전체 신호의 {((perf().buy_count / perf().total_signals) * 100).toFixed(0)}%
                    </div>
                  </div>
                  <div class="p-4 bg-red-500/10 border border-red-500/30 rounded-lg">
                    <div class="flex items-center gap-2 mb-2">
                      <TrendingDown class="w-4 h-4 text-red-500" />
                      <span class="text-sm font-medium text-red-500">매도 신호</span>
                    </div>
                    <div class="text-2xl font-bold text-red-500 mb-1">
                      {perf().sell_count}건
                    </div>
                    <div class="text-xs text-[var(--color-text-muted)]">
                      전체 신호의 {((perf().sell_count / perf().total_signals) * 100).toFixed(0)}%
                    </div>
                  </div>
                </div>

                {/* 신호-수익률 산점도 */}
                <Show when={scatterDataResource()?.length ?? 0 > 0}>
                  <div>
                    <div class="text-sm font-medium text-[var(--color-text)] mb-3">
                      신호 강도 vs 5일 수익률 (상관관계)
                    </div>
                    <div class="h-[200px] bg-[var(--color-surface-light)] rounded-lg p-4 relative overflow-hidden">
                      {/* 축 레이블 */}
                      <div class="absolute left-2 top-1/2 -translate-y-1/2 text-xs text-[var(--color-text-muted)] transform -rotate-90 origin-center">
                        수익률 (%)
                      </div>
                      <div class="absolute bottom-2 left-1/2 -translate-x-1/2 text-xs text-[var(--color-text-muted)]">
                        신호 강도
                      </div>
                      {/* 0선 */}
                      <div class="absolute left-8 right-4 top-1/2 border-t border-dashed border-[var(--color-text-muted)]/30" />
                      {/* 산점도 포인트 */}
                      <svg class="absolute inset-0 w-full h-full" viewBox="0 0 100 100" preserveAspectRatio="none">
                        <For each={scatterDataResource() ?? []}>
                          {(point) => {
                            // x: 강도 0-1 → 10-90
                            const x = 10 + point.strength * 80
                            // y: 수익률 -10% ~ +10% → 90-10 (반전)
                            const ret = point.return_5d ?? 0
                            const y = 50 - (ret / 10) * 40 // +10% → 10, -10% → 90
                            const color = point.side === 'Buy' ? '#22c55e' : '#ef4444'
                            return (
                              <circle
                                cx={x}
                                cy={Math.max(5, Math.min(95, y))}
                                r="2"
                                fill={color}
                                opacity="0.7"
                              />
                            )
                          }}
                        </For>
                      </svg>
                      {/* 범례 */}
                      <div class="absolute top-2 right-2 flex items-center gap-3 text-xs">
                        <div class="flex items-center gap-1">
                          <div class="w-2 h-2 rounded-full bg-green-500" />
                          <span class="text-[var(--color-text-muted)]">매수</span>
                        </div>
                        <div class="flex items-center gap-1">
                          <div class="w-2 h-2 rounded-full bg-red-500" />
                          <span class="text-[var(--color-text-muted)]">매도</span>
                        </div>
                      </div>
                    </div>
                    <div class="text-xs text-[var(--color-text-muted)] mt-2 text-center">
                      높은 강도의 신호가 더 좋은 수익률로 이어지는지 확인하세요
                    </div>
                  </div>
                </Show>
              </div>
            )}
          </Show>
        </CardContent>
      </Card>

      {/* Score History 차트 */}
      <Card>
        <CardHeader>
          <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
            <LineChart class="w-5 h-5" />
            Global Score 추이
          </h3>
        </CardHeader>
        <CardContent>
          <Show
            when={!scoreHistoryResource.loading && (scoreHistoryResource()?.history?.length ?? 0) > 0}
            fallback={
              <EmptyState
                icon={scoreHistoryResource.loading ? '⏳' : '📈'}
                title={scoreHistoryResource.loading ? '점수 히스토리 로딩 중...' : '점수 기록 없음'}
                description={scoreHistoryResource.loading ? undefined : '이 종목의 점수 기록이 아직 없습니다'}
                className="h-[200px] flex flex-col items-center justify-center"
              />
            }
          >
            <ScoreHistoryChart
              data={scoreHistoryResource()?.history ?? []}
              height={200}
              showRank={true}
            />
          </Show>
        </CardContent>
      </Card>
    </div>
  )
}

export default SymbolDetail
