/**
 * 공통 트레이딩 차트 섹션 컴포넌트
 *
 * Backtest, Simulation, PaperTrading에서 공통으로 사용하는
 * 캔들 차트 + 매매 마커 + 볼륨 프로파일 + 신호 필터 UI를 통합한다.
 *
 * <details> 래핑 없이 내부 콘텐츠만 렌더링한다.
 * 토글/조건부 렌더링은 각 페이지에서 개별적으로 처리.
 */
import { Show, For, Suspense, lazy } from 'solid-js'
import { RefreshCw } from 'lucide-solid'
import type { CandlestickDataPoint, TradeMarker, ChartSyncState, IndicatorFilters, PriceVolume } from './index'

const SyncedChartPanel = lazy(() =>
  import('./SyncedChartPanel').then(m => ({ default: m.SyncedChartPanel }))
)
const IndicatorFilterPanel = lazy(() =>
  import('./IndicatorFilterPanel').then(m => ({ default: m.IndicatorFilterPanel }))
)
const VolumeProfile = lazy(() =>
  import('./VolumeProfile').then(m => ({ default: m.VolumeProfile }))
)
const VolumeProfileLegend = lazy(() =>
  import('./VolumeProfile').then(m => ({ default: m.VolumeProfileLegend }))
)

export interface TradingChartSectionProps {
  // 데이터 (getter 함수로 통일)
  candleData: () => CandlestickDataPoint[]
  markers: () => (TradeMarker & { signalType?: string; side?: string })[]
  totalMarkerCount: () => number
  volumeProfileData: () => PriceVolume[]
  currentPrice: () => number
  chartPriceRange: () => [number, number]

  // 로딩 상태
  isLoading: () => boolean
  loadingMessage?: string
  emptyMessage?: string

  // 신호 필터
  signalFilters: IndicatorFilters | (() => IndicatorFilters)
  onSignalFiltersChange: (filters: IndicatorFilters) => void

  // 볼륨 프로파일 토글
  showVolumeProfile: boolean | (() => boolean)
  onShowVolumeProfileChange: (show: boolean) => void

  // 심볼 선택 (다중 심볼)
  symbols?: () => string[]
  selectedSymbol?: () => string
  onSymbolSelect?: (symbol: string) => void

  // 차트 동기화 -- 패턴 A (useChartSync)
  chartSyncBind?: Record<string, unknown>
  // 차트 동기화 — 패턴 B (createSignal)
  chartId?: string
  syncState?: () => ChartSyncState | null
  onVisibleRangeChange?: (state: ChartSyncState) => void

  // 레이아웃
  chartHeight?: number
  volumeProfileWidth?: number
}

/** getter/값 통합 리졸버 */
function resolve<T>(v: T | (() => T)): T {
  return typeof v === 'function' ? (v as () => T)() : v
}

export function TradingChartSection(props: TradingChartSectionProps) {
  const height = () => props.chartHeight ?? 240
  const vpWidth = () => props.volumeProfileWidth ?? 80

  const filters = () => resolve(props.signalFilters)
  const vpVisible = () => resolve(props.showVolumeProfile)

  return (
    <div class="space-y-3">
      {/* 신호 필터 패널 */}
      <Suspense fallback={<div class="h-12 bg-gray-100 dark:bg-gray-800 animate-pulse rounded" />}>
        <IndicatorFilterPanel
          filters={filters()}
          onChange={(f) => props.onSignalFiltersChange(f)}
          defaultCollapsed={true}
        />
      </Suspense>

      {/* 다중 심볼 선택 탭 */}
      <Show when={props.symbols && (props.symbols()?.length ?? 0) > 1}>
        <div class="flex flex-wrap gap-1 p-1 bg-[var(--color-surface-light)]/30 rounded-lg">
          <For each={props.symbols!()}>
            {(symbol) => (
              <button
                class={`px-3 py-1.5 text-xs font-medium rounded-md transition-all ${
                  props.selectedSymbol?.() === symbol
                    ? 'bg-[var(--color-primary)] text-white shadow-sm'
                    : 'text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)] hover:text-[var(--color-text)]'
                }`}
                onClick={(e) => {
                  e.stopPropagation()
                  props.onSymbolSelect?.(symbol)
                }}
              >
                {symbol}
              </button>
            )}
          </For>
        </div>
      </Show>

      {/* 필터 상태 요약 */}
      <Show when={filters().signal_types.length > 0}>
        <div class="text-xs text-[var(--color-text-muted)]">
          표시 중: {props.markers().length} / {props.totalMarkerCount()} 마커
        </div>
      </Show>

      {/* 볼륨 프로파일 토글 */}
      <div class="flex items-center gap-2 mb-2">
        <label class="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)] cursor-pointer">
          <input
            type="checkbox"
            checked={vpVisible()}
            onChange={(e) => props.onShowVolumeProfileChange(e.currentTarget.checked)}
            class="w-3.5 h-3.5 rounded border-gray-500 text-blue-500 focus:ring-blue-500"
          />
          볼륨 프로파일 표시
        </label>
      </div>

      {/* 캔들 차트 + 볼륨 프로파일 */}
      <Show
        when={!props.isLoading() && props.candleData().length > 0}
        fallback={
          <div class="h-[280px] flex items-center justify-center text-[var(--color-text-muted)]">
            {props.isLoading() ? (
              <div class="flex items-center gap-2">
                <RefreshCw class="w-5 h-5 animate-spin" />
                <span>{props.loadingMessage || '차트 데이터 로딩 중...'}</span>
              </div>
            ) : (
              <span>{props.emptyMessage || '차트 데이터가 없습니다 (데이터셋을 먼저 다운로드하세요)'}</span>
            )}
          </div>
        }
      >
        <div class="flex gap-2">
          {/* 캔들 차트 */}
          <div class="flex-1">
            <Suspense fallback={<div class={`h-[${height()}px] bg-gray-100 dark:bg-gray-800 animate-pulse rounded`} />}>
              {/* 패턴 A (chartSyncBind) vs 패턴 B (syncState) */}
              <Show
                when={props.chartSyncBind}
                fallback={
                  <SyncedChartPanel
                    data={props.candleData()}
                    type="candlestick"
                    mainHeight={height()}
                    markers={props.markers()}
                    chartId={props.chartId ?? 'trading-chart'}
                    syncState={props.syncState}
                    onVisibleRangeChange={props.onVisibleRangeChange}
                  />
                }
              >
                <SyncedChartPanel
                  data={props.candleData()}
                  type="candlestick"
                  mainHeight={height()}
                  markers={props.markers()}
                  chartId="price"
                  {...props.chartSyncBind!}
                />
              </Show>
            </Suspense>
          </div>

          {/* 볼륨 프로파일 */}
          <Show when={vpVisible() && props.volumeProfileData().length > 0}>
            <div class="flex flex-col">
              <Suspense fallback={<div class={`h-[${height()}px] w-[${vpWidth()}px] bg-gray-100 dark:bg-gray-800 animate-pulse rounded`} />}>
                <VolumeProfile
                  priceVolumes={props.volumeProfileData()}
                  currentPrice={props.currentPrice()}
                  chartHeight={height()}
                  width={vpWidth()}
                  priceRange={props.chartPriceRange()}
                  showPoc={true}
                  showValueArea={true}
                />
                <VolumeProfileLegend class="mt-1" />
              </Suspense>
            </div>
          </Show>
        </div>
      </Show>
    </div>
  )
}
