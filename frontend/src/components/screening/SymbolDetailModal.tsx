/**
 * 종목 상세 모달 컴포넌트
 *
 * 스크리닝 결과에서 종목을 클릭했을 때 표시되는 상세 정보 모달입니다.
 * 탭 구성: 개요, 지표, 차트
 */
import { Show, createSignal, createResource, onMount, onCleanup, For } from 'solid-js'
import { X, Star, Link2, TrendingUp, BarChart2, Activity, Target, LineChart } from 'lucide-solid'
import type { ScreeningResultDto, CandleData } from '../../api/client'
import { getKlines } from '../../api/client'
import { RouteStateBadge } from '../ui'
import { PriceChart, type CandlestickDataPoint, type LineDataPoint } from '../charts/PriceChart'
import { SyncedChartPanel } from '../charts/SyncedChartPanel'
import type { SeparateIndicatorData } from '../charts/SubPriceChart'
import { createLogger } from '../../utils/logger'

const { log, error: logError } = createLogger('SymbolDetailModal')

interface SymbolDetailModalProps {
  /** 표시 여부 */
  isOpen: boolean
  /** 종목 데이터 */
  symbol: ScreeningResultDto | null
  /** 닫기 핸들러 */
  onClose: () => void
  /** 관심종목 추가 핸들러 */
  onAddWatchlist?: (symbol: string) => void
  /** 전략 연결 핸들러 */
  onLinkStrategy?: (symbol: string) => void
}

type TabType = 'overview' | 'indicators' | 'chart'

// 숫자 포맷팅 유틸 (string | number 모두 지원)
const formatValue = (value: string | number | null | undefined, suffix: string = '', decimals: number = 2): string => {
  if (value === null || value === undefined) return '-'
  const num = typeof value === 'number' ? value : parseFloat(value)
  if (isNaN(num)) return '-'
  return `${num.toFixed(decimals)}${suffix}`
}

const formatMarketCap = (value: string | null | undefined): string => {
  if (!value) return '-'
  const num = parseFloat(value)
  if (isNaN(num)) return '-'
  if (num >= 1e12) return `${(num / 1e12).toFixed(1)}조`
  if (num >= 1e8) return `${(num / 1e8).toFixed(0)}억`
  if (num >= 1e4) return `${(num / 1e4).toFixed(0)}만`
  return num.toLocaleString()
}

const formatPrice = (value: string | null | undefined): string => {
  if (!value) return '-'
  const num = parseFloat(value)
  if (isNaN(num)) return '-'
  return num.toLocaleString('ko-KR')
}

// 지표 카드 컴포넌트
interface MetricCardProps {
  label: string
  value: string
  subValue?: string
  valueColor?: string
}

function MetricCard(props: MetricCardProps) {
  return (
    <div class="bg-gray-800/50 rounded-lg p-3">
      <div class="text-xs text-gray-400 mb-1">{props.label}</div>
      <div class={`text-lg font-semibold ${props.valueColor || 'text-white'}`}>
        {props.value}
      </div>
      {props.subValue && (
        <div class="text-xs text-gray-500 mt-0.5">{props.subValue}</div>
      )}
    </div>
  )
}

// 지표 행 컴포넌트
interface MetricRowProps {
  label: string
  value: string
  valueColor?: string
}

function MetricRow(props: MetricRowProps) {
  return (
    <div class="flex justify-between items-center py-2 border-b border-gray-700/50 last:border-0">
      <span class="text-sm text-gray-400">{props.label}</span>
      <span class={`text-sm font-medium ${props.valueColor || 'text-white'}`}>{props.value}</span>
    </div>
  )
}

// 지표 색상 결정
const getValueColor = (value: string | null | undefined, thresholds: { good: number; bad: number }, inverse: boolean = false): string => {
  if (!value) return 'text-gray-400'
  const num = parseFloat(value)
  if (isNaN(num)) return 'text-gray-400'

  if (inverse) {
    if (num <= thresholds.good) return 'text-green-400'
    if (num >= thresholds.bad) return 'text-red-400'
  } else {
    if (num >= thresholds.good) return 'text-green-400'
    if (num <= thresholds.bad) return 'text-red-400'
  }
  return 'text-white'
}

export function SymbolDetailModal(props: SymbolDetailModalProps) {
  let modalRef: HTMLDivElement | undefined
  const [activeTab, setActiveTab] = createSignal<TabType>('overview')

  // 캔들 데이터 로드
  const [candleData] = createResource(
    () => props.isOpen && props.symbol ? props.symbol.ticker : null,
    async (ticker): Promise<CandleData[]> => {
      if (!ticker) return []
      try {
        log('[Chart] Loading candles for:', ticker)
        const response = await getKlines({ symbol: ticker, timeframe: '1d', limit: 60 })
        log('[Chart] Response:', response)
        const data = response.data || []
        log('[Chart] Candle count:', data.length)
        return data
      } catch (error) {
        logError('[Chart] Error loading candles:', error)
        return []
      }
    }
  )

  // ESC 키로 닫기
  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') props.onClose()
  }

  onMount(() => {
    document.addEventListener('keydown', handleKeyDown)
  })

  onCleanup(() => {
    document.removeEventListener('keydown', handleKeyDown)
  })

  // 배경 클릭으로 닫기
  const handleBackdropClick = (e: MouseEvent) => {
    if (e.target === modalRef) props.onClose()
  }

  // 탭 목록
  const tabs: { id: TabType; label: string; icon: typeof TrendingUp }[] = [
    { id: 'overview', label: '개요', icon: BarChart2 },
    { id: 'indicators', label: '지표', icon: Activity },
    { id: 'chart', label: '차트', icon: LineChart },
  ]

  // CandleData를 캔들스틱 차트용 데이터로 변환
  const toCandlestickData = (): CandlestickDataPoint[] => {
    const data = candleData() || []
    return data.map((d) => ({
      time: d.time.split('T')[0], // "2024-01-15T00:00:00" → "2024-01-15"
      open: d.open,
      high: d.high,
      low: d.low,
      close: d.close,
    }))
  }

  // CandleData를 라인 차트용 데이터로 변환 (종가 기준)
  const toLineData = (): LineDataPoint[] => {
    const data = candleData() || []
    return data.map((d) => ({
      time: d.time.split('T')[0],
      value: d.close,
    }))
  }

  // 볼륨 데이터를 서브 지표 형태로 변환
  const toVolumeIndicator = (): SeparateIndicatorData[] => {
    const data = candleData() || []
    if (data.length === 0) return []

    return [{
      id: 'volume',
      type: 'volume',
      name: '거래량',
      series: [{
        name: 'Volume',
        data: data.map((d) => ({
          time: d.time.split('T')[0],
          value: d.volume,
        })),
        color: '#6366f1',
        seriesType: 'bar' as const,
      }],
    }]
  }

  return (
    <Show when={props.isOpen && props.symbol}>
      <div
        ref={modalRef}
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
        onClick={handleBackdropClick}
      >
        <div class="bg-gray-900 border border-gray-700 rounded-xl shadow-2xl w-full max-w-2xl max-h-[90vh] overflow-hidden flex flex-col">
          {/* 헤더 */}
          <div class="flex items-center justify-between px-6 py-4 border-b border-gray-700 shrink-0">
            <div class="flex items-center gap-3">
              <div>
                <div class="flex items-center gap-2">
                  <h2 class="text-xl font-bold text-white">{props.symbol!.name}</h2>
                  <Show when={props.symbol!.route_state}>
                    <RouteStateBadge state={props.symbol!.route_state as "ATTACK" | "ARMED" | "WAIT" | "OVERHEAT" | "NEUTRAL"} size="sm" />
                  </Show>
                </div>
                <div class="flex items-center gap-2 mt-1">
                  <span class="text-sm text-gray-400 font-mono">{props.symbol!.ticker}</span>
                  <span class="text-xs px-1.5 py-0.5 bg-gray-700 rounded text-gray-300">
                    {props.symbol!.market}
                  </span>
                  {props.symbol!.sector && (
                    <span class="text-xs text-gray-500">{props.symbol!.sector}</span>
                  )}
                </div>
              </div>
            </div>
            <button
              type="button"
              onClick={props.onClose}
              class="p-2 hover:bg-gray-800 rounded-lg transition-colors"
            >
              <X class="w-5 h-5 text-gray-400" />
            </button>
          </div>

          {/* 탭 네비게이션 */}
          <div class="flex border-b border-gray-700 shrink-0">
            <For each={tabs}>
              {(tab) => (
                <button
                  type="button"
                  onClick={() => setActiveTab(tab.id)}
                  class={`flex items-center gap-2 px-5 py-3 text-sm font-medium transition-colors ${
                    activeTab() === tab.id
                      ? 'text-blue-400 border-b-2 border-blue-400'
                      : 'text-gray-400 hover:text-gray-300'
                  }`}
                >
                  <tab.icon class="w-4 h-4" />
                  {tab.label}
                </button>
              )}
            </For>
          </div>

          {/* 탭 컨텐츠 */}
          <div class="flex-1 overflow-y-auto p-6">
            {/* 개요 탭 */}
            <Show when={activeTab() === 'overview'}>
              <div class="space-y-6">
                {/* 가격 정보 */}
                <div class="flex items-end gap-4">
                  <div>
                    <div class="text-3xl font-bold text-white">
                      ₩{formatPrice(props.symbol!.current_price)}
                    </div>
                    <div class="text-sm text-gray-400 mt-1">현재가</div>
                  </div>
                  <Show when={props.symbol!.overall_score}>
                    <div class="ml-auto text-right">
                      <div class={`text-2xl font-bold ${
                        parseFloat(props.symbol!.overall_score || '0') >= 70 ? 'text-green-400' :
                        parseFloat(props.symbol!.overall_score || '0') >= 50 ? 'text-yellow-400' : 'text-gray-400'
                      }`}>
                        {parseFloat(props.symbol!.overall_score || '0').toFixed(0)}점
                      </div>
                      <div class="text-sm text-gray-400">종합 점수</div>
                    </div>
                  </Show>
                </div>

                {/* 미니 차트 */}
                <div class="bg-gray-800/30 rounded-lg p-4">
                  <div class="text-sm text-gray-400 mb-2">최근 60일</div>
                  <Show
                    when={!candleData.loading && (candleData() || []).length > 0}
                    fallback={
                      <div class="h-24 flex items-center justify-center text-gray-500 text-sm">
                        {candleData.loading ? '차트 로딩 중...' : '차트 데이터 없음'}
                      </div>
                    }
                  >
                    <PriceChart
                      data={toLineData()}
                      type="line"
                      height={100}
                      colors={{ background: 'transparent', grid: '#374151' }}
                    />
                  </Show>
                </div>

                {/* 52주 고/저가 */}
                <Show when={props.symbol!.week_52_high || props.symbol!.week_52_low}>
                  <div class="bg-gray-800/30 rounded-lg p-4">
                    <div class="text-sm text-gray-400 mb-3">52주 범위</div>
                    <div class="flex items-center gap-4">
                      <div class="text-center">
                        <div class="text-xs text-gray-500 mb-1">최저</div>
                        <div class="text-sm text-red-400">₩{formatPrice(props.symbol!.week_52_low)}</div>
                      </div>
                      <div class="flex-1 h-2 bg-gray-700 rounded-full relative">
                        <div
                          class="absolute h-full bg-gradient-to-r from-red-500 via-yellow-500 to-green-500 rounded-full"
                          style={{
                            width: `${Math.min(100, Math.max(0, 100 - parseFloat(props.symbol!.distance_from_52w_high || '50')))}%`
                          }}
                        />
                      </div>
                      <div class="text-center">
                        <div class="text-xs text-gray-500 mb-1">최고</div>
                        <div class="text-sm text-green-400">₩{formatPrice(props.symbol!.week_52_high)}</div>
                      </div>
                    </div>
                  </div>
                </Show>

                {/* 기본 정보 */}
                <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
                  <MetricCard label="시가총액" value={formatMarketCap(props.symbol!.market_cap)} />
                  <MetricCard
                    label="PER"
                    value={formatValue(props.symbol!.per, '배', 1)}
                    valueColor={getValueColor(props.symbol!.per, { good: 15, bad: 30 }, true)}
                  />
                  <MetricCard
                    label="PBR"
                    value={formatValue(props.symbol!.pbr, '배', 2)}
                    valueColor={getValueColor(props.symbol!.pbr, { good: 1.5, bad: 3 }, true)}
                  />
                  <MetricCard
                    label="ROE"
                    value={formatValue(props.symbol!.roe, '%', 1)}
                    valueColor={getValueColor(props.symbol!.roe, { good: 15, bad: 5 })}
                  />
                </div>
              </div>
            </Show>

            {/* 지표 탭 */}
            <Show when={activeTab() === 'indicators'}>
              <div class="space-y-6">
                {/* 기술적 지표 */}
                <div>
                  <div class="text-sm font-medium text-gray-300 mb-3 flex items-center gap-2">
                    <Target class="w-4 h-4" />
                    기술적 지표
                  </div>
                  <div class="bg-gray-800/30 rounded-lg p-4">
                    <MetricRow
                      label="RSI (14)"
                      value={formatValue(props.symbol!.rsi_14, '', 1)}
                      valueColor={
                        (props.symbol!.rsi_14 ?? 50) >= 70 ? 'text-red-400' :
                        (props.symbol!.rsi_14 ?? 50) <= 30 ? 'text-green-400' : 'text-white'
                      }
                    />
                    <MetricRow label="MA20 이격도" value={formatValue(props.symbol!.dist_ma20, '%', 1)} />
                    <MetricRow label="볼린저밴드 폭" value={formatValue(props.symbol!.bb_width, '%', 1)} />
                    <MetricRow label="Range Position" value={formatValue(props.symbol!.range_pos, '', 2)} />
                    <MetricRow label="Low Trend" value={formatValue(props.symbol!.low_trend, '', 2)} />
                    <MetricRow label="Vol Quality" value={formatValue(props.symbol!.vol_quality, '', 2)} />
                  </div>
                </div>

                {/* Global Score 구성요소 */}
                <div>
                  <div class="text-sm font-medium text-gray-300 mb-3 flex items-center gap-2">
                    <Activity class="w-4 h-4" />
                    Global Score 구성요소
                  </div>
                  <div class="bg-gray-800/30 rounded-lg p-4">
                    <MetricRow
                      label="종합 점수"
                      value={formatValue(props.symbol!.overall_score, '점', 0)}
                      valueColor={
                        parseFloat(props.symbol!.overall_score || '0') >= 70 ? 'text-green-400' :
                        parseFloat(props.symbol!.overall_score || '0') >= 50 ? 'text-yellow-400' : 'text-white'
                      }
                    />
                    <MetricRow
                      label="등급"
                      value={props.symbol!.grade || '-'}
                      valueColor={
                        props.symbol!.grade === 'S' ? 'text-purple-400' :
                        props.symbol!.grade === 'A' ? 'text-green-400' :
                        props.symbol!.grade === 'B' ? 'text-blue-400' :
                        props.symbol!.grade === 'C' ? 'text-yellow-400' :
                        props.symbol!.grade === 'D' ? 'text-orange-400' :
                        props.symbol!.grade === 'F' ? 'text-red-400' : 'text-gray-400'
                      }
                    />
                    <MetricRow label="Trigger Score" value={formatValue(props.symbol!.trigger_score, '', 1)} />
                    <MetricRow label="Trigger Label" value={props.symbol!.trigger_label || '-'} />
                    <MetricRow label="Breakout Score" value={formatValue(props.symbol!.breakout_score, '', 1)} />
                    <MetricRow label="섹터 RS" value={formatValue(props.symbol!.sector_rs, '', 2)} />
                    <MetricRow label="섹터 순위" value={props.symbol!.sector_rank?.toString() || '-'} />
                    <MetricRow label="TTM Squeeze" value={props.symbol!.ttm_squeeze ? '🔴 발동' : '⚪ 해제'} />
                    <MetricRow label="Squeeze 횟수" value={props.symbol!.ttm_squeeze_cnt?.toString() || '-'} />
                  </div>
                </div>

                {/* 펀더멘털 지표 */}
                <div>
                  <div class="text-sm font-medium text-gray-300 mb-3 flex items-center gap-2">
                    <BarChart2 class="w-4 h-4" />
                    펀더멘털 지표
                  </div>
                  <div class="bg-gray-800/30 rounded-lg p-4">
                    <MetricRow label="ROA" value={formatValue(props.symbol!.roa, '%', 1)} />
                    <MetricRow label="영업이익률" value={formatValue(props.symbol!.operating_margin, '%', 1)} />
                    <MetricRow label="부채비율" value={formatValue(props.symbol!.debt_ratio, '%', 0)} />
                    <MetricRow label="배당수익률" value={formatValue(props.symbol!.dividend_yield, '%', 2)} />
                    <MetricRow label="EPS" value={formatValue(props.symbol!.eps, '원', 0)} />
                    <MetricRow label="매출성장률" value={formatValue(props.symbol!.revenue_growth_yoy, '%', 1)} />
                    <MetricRow label="이익성장률" value={formatValue(props.symbol!.earnings_growth_yoy, '%', 1)} />
                  </div>
                </div>
              </div>
            </Show>

            {/* 차트 탭 */}
            <Show when={activeTab() === 'chart'}>
              <div class="space-y-4">
                <div class="text-sm text-gray-400">최근 60일 캔들 차트 + 거래량</div>
                <Show
                  when={!candleData.loading && (candleData() || []).length > 0}
                  fallback={
                    <div class="h-80 flex items-center justify-center text-gray-500 text-sm bg-gray-800/30 rounded-lg">
                      {candleData.loading ? '차트 로딩 중...' : '차트 데이터 없음'}
                    </div>
                  }
                >
                  <SyncedChartPanel
                    data={toCandlestickData()}
                    type="candlestick"
                    mainHeight={280}
                    subHeight={80}
                    subIndicators={toVolumeIndicator()}
                    colors={{ background: 'transparent', grid: '#374151' }}
                  />
                </Show>
              </div>
            </Show>
          </div>

          {/* 푸터 (액션 버튼) */}
          <div class="flex items-center justify-end gap-3 px-6 py-4 border-t border-gray-700 shrink-0">
            <button
              type="button"
              onClick={() => props.onAddWatchlist?.(props.symbol!.ticker)}
              class="flex items-center gap-2 px-4 py-2 bg-yellow-600/20 hover:bg-yellow-600/30 text-yellow-400 rounded-lg transition-colors"
            >
              <Star class="w-4 h-4" />
              관심종목 추가
            </button>
            <button
              type="button"
              onClick={() => props.onLinkStrategy?.(props.symbol!.ticker)}
              class="flex items-center gap-2 px-4 py-2 bg-blue-600/20 hover:bg-blue-600/30 text-blue-400 rounded-lg transition-colors"
            >
              <Link2 class="w-4 h-4" />
              전략 연결
            </button>
          </div>
        </div>
      </div>
    </Show>
  )
}

export default SymbolDetailModal
