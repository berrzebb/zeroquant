/**
 * MultiTimeframeChart - 다중 타임프레임 차트 동기화 컴포넌트.
 *
 * 기능:
 * - 메인 차트 (Primary TF): 전략 실행의 기준 타임프레임
 * - 서브 차트 패널 (Secondary TF별): 상위 타임프레임으로 추세 확인
 * - 차트 간 시간축 동기화 (TF별 해상도 차이 고려)
 * - 크로스헤어 시간 동기화
 * - 패널 접힘/펼침 기능
 * - 레이아웃 옵션 (세로/가로 분할)
 */
import { onMount, onCleanup, createEffect, on, For, Show, createSignal, createMemo } from 'solid-js'
import {
  createChart,
  ColorType,
  CandlestickSeries,
  LineSeries,
} from 'lightweight-charts'
import type {
  IChartApi,
  ISeriesApi,
  CandlestickData,
  LineData,
  Time,
  LogicalRange,
  MouseEventParams,
} from 'lightweight-charts'
import type { CandlestickDataPoint, LineDataPoint, IndicatorOverlay } from './PriceChart'
import type { Timeframe } from '../../api/client'

/** 레이아웃 모드 */
export type LayoutMode = 'vertical' | 'horizontal' | 'grid'

/** 타임프레임별 데이터 */
export interface TimeframeData {
  timeframe: Timeframe
  data: CandlestickDataPoint[]
  indicators?: IndicatorOverlay[]
}

/** 멀티 타임프레임 차트 Props */
export interface MultiTimeframeChartProps {
  /** Primary 타임프레임 (메인 차트) */
  primaryTimeframe: Timeframe
  /** Primary 타임프레임 데이터 */
  primaryData: CandlestickDataPoint[]
  /** Primary 차트 지표 오버레이 */
  primaryIndicators?: IndicatorOverlay[]
  /** Secondary 타임프레임 데이터 목록 */
  secondaryData: TimeframeData[]
  /** 메인 차트 높이 */
  mainHeight?: number
  /** 서브 차트 높이 */
  subHeight?: number
  /** 심볼명 (헤더 표시용) */
  symbol?: string
  /** 커스텀 색상 */
  colors?: {
    background?: string
    text?: string
    grid?: string
    upColor?: string
    downColor?: string
  }
  /** 차트 타입 */
  chartType?: 'candlestick' | 'line'
  /** 로딩 상태 */
  loading?: boolean
  /** 레이아웃 모드 */
  layout?: LayoutMode
  /** 패널 접힘/펼침 가능 여부 */
  collapsible?: boolean
  /** 초기 접힌 패널 인덱스 목록 */
  initialCollapsed?: number[]
  /** 레이아웃 변경 콜백 */
  onLayoutChange?: (layout: LayoutMode) => void
}

/** 타임프레임 라벨 변환 */
function getTimeframeLabel(tf: Timeframe): string {
  const labels: Record<Timeframe, string> = {
    '1m': '1분',
    '5m': '5분',
    '15m': '15분',
    '30m': '30분',
    '1h': '1시간',
    '4h': '4시간',
    '1d': '일봉',
    '1w': '주봉',
    '1M': '월봉',
  }
  return labels[tf] || tf
}

/** 데이터 정렬 및 중복 타임스탬프 제거 */
function deduplicateAndSort<T extends { time: string | number }>(data: T[]): T[] {
  const sorted = [...data].sort((a, b) => {
    const timeA = typeof a.time === 'string' ? a.time : a.time.toString()
    const timeB = typeof b.time === 'string' ? b.time : b.time.toString()
    return timeA.localeCompare(timeB)
  })

  const uniqueMap = new Map<string, T>()
  for (const item of sorted) {
    const timeKey = typeof item.time === 'string' ? item.time : item.time.toString()
    uniqueMap.set(timeKey, item)
  }

  return Array.from(uniqueMap.values())
}

/** Primary 타임프레임의 시간을 Secondary 타임프레임의 가장 가까운 시간으로 매핑 */
function findNearestTime(
  targetTime: Time,
  secondaryData: CandlestickDataPoint[]
): Time | null {
  if (secondaryData.length === 0) return null

  const targetStr = typeof targetTime === 'string' ? targetTime : String(targetTime)

  // 이진 탐색으로 가장 가까운 시간 찾기
  let left = 0
  let right = secondaryData.length - 1

  while (left < right) {
    const mid = Math.floor((left + right) / 2)
    const midTimeStr = typeof secondaryData[mid].time === 'string'
      ? secondaryData[mid].time
      : String(secondaryData[mid].time)

    if (midTimeStr < targetStr) {
      left = mid + 1
    } else {
      right = mid
    }
  }

  // left 인덱스가 가장 가까운 시간
  if (left >= secondaryData.length) left = secondaryData.length - 1

  return secondaryData[left].time as Time
}

export function MultiTimeframeChart(props: MultiTimeframeChartProps) {
  // 차트 인스턴스
  let mainChartContainer: HTMLDivElement | undefined
  let mainChart: IChartApi | undefined
  let mainSeries: ISeriesApi<'Candlestick'> | ISeriesApi<'Line'> | undefined
  const mainIndicatorSeries = new Map<string, ISeriesApi<'Line'>>()

  // 서브 차트들 (Secondary TF별)
  const subChartRefs: HTMLDivElement[] = []
  const subCharts: IChartApi[] = []
  const subSeries: (ISeriesApi<'Candlestick'> | ISeriesApi<'Line'> | undefined)[] = []
  const subIndicatorSeries: Map<string, ISeriesApi<'Line'>>[] = []

  // 동기화 플래그 (무한 루프 방지)
  let isSyncing = false

  // 현재 크로스헤어 시간 (디버깅/표시용)
  const [crosshairTime, setCrosshairTime] = createSignal<string | null>(null)

  // 패널 접힘 상태 (인덱스별)
  const [collapsedPanels, setCollapsedPanels] = createSignal<Set<number>>(
    new Set(props.initialCollapsed || [])
  )

  // 레이아웃 모드
  const [layoutMode, setLayoutMode] = createSignal<LayoutMode>(props.layout || 'vertical')

  // 패널 토글 함수
  const togglePanel = (index: number) => {
    setCollapsedPanels(prev => {
      const newSet = new Set(prev)
      if (newSet.has(index)) {
        newSet.delete(index)
      } else {
        newSet.add(index)
      }
      return newSet
    })
  }

  // 레이아웃 변경 함수
  const changeLayout = (mode: LayoutMode) => {
    setLayoutMode(mode)
    props.onLayoutChange?.(mode)
    // 레이아웃 변경 후 차트 리사이즈
    requestAnimationFrame(() => {
      if (mainChart && mainChartContainer) {
        mainChart.applyOptions({ width: mainChartContainer.clientWidth })
      }
      subCharts.forEach((chart, i) => {
        const container = subChartRefs[i]
        if (chart && container) {
          chart.applyOptions({ width: container.clientWidth })
        }
      })
    })
  }

  // 모든 패널 펼치기/접기
  const expandAll = () => setCollapsedPanels(new Set())
  const collapseAll = () => {
    const allIndices = props.secondaryData.map((_, i) => i)
    setCollapsedPanels(new Set(allIndices))
  }

  // 접힌 패널 높이
  const collapsedHeight = 32

  const defaultColors = {
    background: 'transparent',
    text: '#d1d5db',
    grid: '#374151',
    upColor: '#22c55e',
    downColor: '#ef4444',
  }

  const getColors = () => ({
    ...defaultColors,
    ...props.colors,
  })

  // 시간 축 동기화 함수 (Primary → Secondary)
  const syncTimeScale = (sourceChart: IChartApi, logicalRange: LogicalRange | null) => {
    if (isSyncing || !logicalRange) return
    isSyncing = true

    const allCharts = [mainChart, ...subCharts].filter(Boolean) as IChartApi[]
    for (const chart of allCharts) {
      if (chart !== sourceChart) {
        // 다른 타임프레임이므로 LogicalRange 직접 적용
        // (시간 기반 범위가 아닌 캔들 개수 기반이므로 근사적으로 동기화)
        chart.timeScale().setVisibleLogicalRange(logicalRange)
      }
    }

    isSyncing = false
  }

  // 크로스헤어 시간 동기화 함수
  const syncCrosshair = (
    sourceChart: IChartApi,
    sourceSeries: ISeriesApi<'Candlestick'> | ISeriesApi<'Line'> | undefined,
    sourceData: CandlestickDataPoint[],
    param: MouseEventParams
  ) => {
    if (isSyncing) return
    isSyncing = true

    const time = param.time
    if (time) {
      setCrosshairTime(typeof time === 'string' ? time : String(time))
    } else {
      setCrosshairTime(null)
    }

    // 메인 차트가 소스가 아닌 경우 메인 차트에 동기화
    if (mainChart && sourceChart !== mainChart && mainSeries && param.time) {
      const nearestTime = findNearestTime(param.time, props.primaryData)
      if (nearestTime) {
        mainChart.setCrosshairPosition(NaN, nearestTime, mainSeries)
      }
    } else if (mainChart && sourceChart !== mainChart) {
      mainChart.clearCrosshairPosition()
    }

    // 서브 차트들에 동기화
    for (let i = 0; i < subCharts.length; i++) {
      const chart = subCharts[i]
      const series = subSeries[i]
      const secondaryDataItem = props.secondaryData[i]

      if (chart && series && sourceChart !== chart) {
        if (param.time && secondaryDataItem?.data) {
          // Primary 시간을 Secondary의 가장 가까운 시간으로 매핑
          const nearestTime = findNearestTime(param.time, secondaryDataItem.data)
          if (nearestTime) {
            chart.setCrosshairPosition(NaN, nearestTime, series)
          }
        } else {
          chart.clearCrosshairPosition()
        }
      }
    }

    isSyncing = false
  }

  // 메인 차트 (Primary TF) 생성
  const createMainChart = () => {
    if (!mainChartContainer) return

    const colors = getColors()

    mainChart = createChart(mainChartContainer, {
      layout: {
        background: { type: ColorType.Solid, color: colors.background },
        textColor: colors.text,
      },
      grid: {
        vertLines: { color: colors.grid },
        horzLines: { color: colors.grid },
      },
      width: mainChartContainer.clientWidth,
      height: props.mainHeight || 300,
      crosshair: { mode: 1 },
      timeScale: {
        borderColor: colors.grid,
        timeVisible: true,
        secondsVisible: false,
      },
      rightPriceScale: { borderColor: colors.grid },
    })

    // 시리즈 생성
    if (props.chartType === 'line') {
      mainSeries = mainChart.addSeries(LineSeries, {
        color: colors.upColor,
        lineWidth: 2,
      })
    } else {
      mainSeries = mainChart.addSeries(CandlestickSeries, {
        upColor: colors.upColor,
        downColor: colors.downColor,
        borderUpColor: colors.upColor,
        borderDownColor: colors.downColor,
        wickUpColor: colors.upColor,
        wickDownColor: colors.downColor,
      })
    }

    // 데이터 설정
    if (props.primaryData && props.primaryData.length > 0) {
      const uniqueData = deduplicateAndSort(props.primaryData)
      mainSeries.setData(uniqueData as CandlestickData[] | LineData[])
    }

    // 동기화 이벤트 구독
    mainChart.timeScale().subscribeVisibleLogicalRangeChange((range) => {
      syncTimeScale(mainChart!, range)
    })

    mainChart.subscribeCrosshairMove((param) => {
      syncCrosshair(mainChart!, mainSeries, props.primaryData, param)
    })

    // 초기 fit
    requestAnimationFrame(() => {
      mainChart?.timeScale().fitContent()
    })
  }

  // 서브 차트 (Secondary TF) 생성
  const createSubChart = (
    container: HTMLDivElement,
    tfData: TimeframeData,
    index: number
  ) => {
    const colors = getColors()

    const chart = createChart(container, {
      layout: {
        background: { type: ColorType.Solid, color: colors.background },
        textColor: colors.text,
      },
      grid: {
        vertLines: { color: colors.grid },
        horzLines: { color: colors.grid },
      },
      width: container.clientWidth,
      height: props.subHeight || 150,
      crosshair: { mode: 1 },
      timeScale: {
        borderColor: colors.grid,
        timeVisible: true,
        secondsVisible: false,
      },
      rightPriceScale: {
        borderColor: colors.grid,
        scaleMargins: { top: 0.1, bottom: 0.1 },
      },
    })

    subCharts[index] = chart

    // 시리즈 생성
    let series: ISeriesApi<'Candlestick'> | ISeriesApi<'Line'>
    if (props.chartType === 'line') {
      series = chart.addSeries(LineSeries, {
        color: colors.upColor,
        lineWidth: 2,
      })
    } else {
      series = chart.addSeries(CandlestickSeries, {
        upColor: colors.upColor,
        downColor: colors.downColor,
        borderUpColor: colors.upColor,
        borderDownColor: colors.downColor,
        wickUpColor: colors.upColor,
        wickDownColor: colors.downColor,
      })
    }

    subSeries[index] = series

    // 데이터 설정
    if (tfData.data && tfData.data.length > 0) {
      const uniqueData = deduplicateAndSort(tfData.data)
      series.setData(uniqueData as CandlestickData[] | LineData[])
    }

    // 지표 오버레이
    if (!subIndicatorSeries[index]) {
      subIndicatorSeries[index] = new Map()
    }

    if (tfData.indicators) {
      for (const indicator of tfData.indicators) {
        const lineWidth = (indicator.lineWidth && indicator.lineWidth >= 1 && indicator.lineWidth <= 4
          ? indicator.lineWidth
          : 2) as 1 | 2 | 3 | 4

        const indicatorLine = chart.addSeries(LineSeries, {
          color: indicator.color,
          lineWidth,
          priceScaleId: indicator.priceScaleId || 'right',
          lastValueVisible: true,
          priceLineVisible: false,
        })

        if (indicator.data && indicator.data.length > 0) {
          const uniqueIndicatorData = deduplicateAndSort(indicator.data)
          indicatorLine.setData(uniqueIndicatorData as LineData[])
        }

        subIndicatorSeries[index].set(indicator.id, indicatorLine)
      }
    }

    // 동기화 이벤트 구독
    chart.timeScale().subscribeVisibleLogicalRangeChange((range) => {
      syncTimeScale(chart, range)
    })

    chart.subscribeCrosshairMove((param) => {
      syncCrosshair(chart, series, tfData.data, param)
    })

    return chart
  }

  onMount(() => {
    createMainChart()

    // 리사이즈 핸들러
    const handleResize = () => {
      if (mainChart && mainChartContainer) {
        mainChart.applyOptions({ width: mainChartContainer.clientWidth })
      }
      subCharts.forEach((chart, i) => {
        const container = subChartRefs[i]
        if (chart && container) {
          chart.applyOptions({ width: container.clientWidth })
        }
      })
    }

    window.addEventListener('resize', handleResize)

    onCleanup(() => {
      window.removeEventListener('resize', handleResize)
      mainChart?.remove()
      subCharts.forEach(chart => chart?.remove())
    })
  })

  // Primary 데이터 변경 시 업데이트
  createEffect(
    on(
      () => props.primaryData,
      (data) => {
        if (mainSeries && data && data.length > 0) {
          const uniqueData = deduplicateAndSort(data)
          mainSeries.setData(uniqueData as CandlestickData[] | LineData[])
          mainChart?.timeScale().fitContent()
        }
      }
    )
  )

  // Primary 지표 오버레이 변경 시 업데이트
  createEffect(
    on(
      () => props.primaryIndicators,
      (indicators) => {
        if (!mainChart) return

        // 기존 지표 제거
        const currentIds = new Set(indicators?.map(i => i.id) || [])
        for (const [id, series] of mainIndicatorSeries.entries()) {
          if (!currentIds.has(id)) {
            mainChart.removeSeries(series)
            mainIndicatorSeries.delete(id)
          }
        }

        // 새 지표 추가/업데이트
        if (indicators) {
          for (const indicator of indicators) {
            let series = mainIndicatorSeries.get(indicator.id)

            if (!series) {
              const lineWidth = (indicator.lineWidth && indicator.lineWidth >= 1 && indicator.lineWidth <= 4
                ? indicator.lineWidth
                : 2) as 1 | 2 | 3 | 4
              series = mainChart.addSeries(LineSeries, {
                color: indicator.color,
                lineWidth,
                priceScaleId: indicator.priceScaleId || 'right',
                lastValueVisible: true,
                priceLineVisible: false,
              })
              mainIndicatorSeries.set(indicator.id, series)
            }

            if (indicator.data && indicator.data.length > 0) {
              const uniqueData = deduplicateAndSort(indicator.data)
              series.setData(uniqueData as LineData[])
            }
          }
        }
      }
    )
  )

  // Secondary 데이터 변경 시 서브 차트 재생성
  createEffect(
    on(
      () => props.secondaryData,
      (secondaryData) => {
        // 기존 서브 차트 제거
        subCharts.forEach(chart => chart?.remove())
        subCharts.length = 0
        subSeries.length = 0
        subIndicatorSeries.forEach(map => map.clear())
        subIndicatorSeries.length = 0

        // 메인 차트의 시간 범위 동기화
        if (mainChart && secondaryData && secondaryData.length > 0) {
          requestAnimationFrame(() => {
            const range = mainChart?.timeScale().getVisibleLogicalRange()
            if (range) {
              subCharts.forEach(chart => {
                chart?.timeScale().setVisibleLogicalRange(range)
              })
            }
          })
        }
      }
    )
  )

  // 레이아웃에 따른 컨테이너 클래스
  const containerClass = createMemo(() => {
    switch (layoutMode()) {
      case 'horizontal':
        return 'flex flex-row gap-2 overflow-x-auto'
      case 'grid':
        return 'grid grid-cols-2 gap-2'
      default:
        return 'flex flex-col gap-2'
    }
  })

  // 레이아웃에 따른 차트 너비
  const chartWidthStyle = createMemo(() => {
    switch (layoutMode()) {
      case 'horizontal':
        return { 'min-width': '400px', flex: '1 0 auto' }
      case 'grid':
        return {}
      default:
        return {}
    }
  })

  return (
    <div class="flex flex-col gap-2">
      {/* 헤더 + 툴바 */}
      <div class="flex items-center justify-between px-2">
        <div class="flex items-center gap-2">
          <Show when={props.symbol}>
            <span class="text-sm font-medium text-[var(--color-text)]">
              {props.symbol}
            </span>
          </Show>
          <Show when={crosshairTime()}>
            <span class="text-xs text-[var(--color-text-muted)]">
              {crosshairTime()}
            </span>
          </Show>
        </div>

        {/* 레이아웃 및 접기 컨트롤 */}
        <div class="flex items-center gap-2">
          {/* 레이아웃 선택 */}
          <div class="flex items-center gap-1 bg-[var(--color-surface)] rounded p-0.5">
            <button
              class={`p-1 rounded transition-colors ${layoutMode() === 'vertical' ? 'bg-[var(--color-surface-light)] text-[var(--color-primary)]' : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'}`}
              onClick={() => changeLayout('vertical')}
              title="세로 레이아웃"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
              </svg>
            </button>
            <button
              class={`p-1 rounded transition-colors ${layoutMode() === 'horizontal' ? 'bg-[var(--color-surface-light)] text-[var(--color-primary)]' : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'}`}
              onClick={() => changeLayout('horizontal')}
              title="가로 레이아웃"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6v12M12 6v12M20 6v12" />
              </svg>
            </button>
            <button
              class={`p-1 rounded transition-colors ${layoutMode() === 'grid' ? 'bg-[var(--color-surface-light)] text-[var(--color-primary)]' : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'}`}
              onClick={() => changeLayout('grid')}
              title="그리드 레이아웃"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 5a1 1 0 011-1h4a1 1 0 011 1v4a1 1 0 01-1 1H5a1 1 0 01-1-1V5zM14 5a1 1 0 011-1h4a1 1 0 011 1v4a1 1 0 01-1 1h-4a1 1 0 01-1-1V5zM4 15a1 1 0 011-1h4a1 1 0 011 1v4a1 1 0 01-1 1H5a1 1 0 01-1-1v-4zM14 15a1 1 0 011-1h4a1 1 0 011 1v4a1 1 0 01-1 1h-4a1 1 0 01-1-1v-4z" />
              </svg>
            </button>
          </div>

          {/* 모두 접기/펼치기 */}
          <Show when={props.collapsible && props.secondaryData.length > 0}>
            <div class="flex items-center gap-1">
              <button
                class="p-1 rounded text-[var(--color-text-muted)] hover:text-[var(--color-text)] hover:bg-[var(--color-surface)] transition-colors"
                onClick={expandAll}
                title="모두 펼치기"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4" />
                </svg>
              </button>
              <button
                class="p-1 rounded text-[var(--color-text-muted)] hover:text-[var(--color-text)] hover:bg-[var(--color-surface)] transition-colors"
                onClick={collapseAll}
                title="모두 접기"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 9V4.5M9 9H4.5M9 9L3.75 3.75M9 15v4.5M9 15H4.5M9 15l-5.25 5.25M15 9h4.5M15 9V4.5M15 9l5.25-5.25M15 15h4.5M15 15v4.5m0-4.5l5.25 5.25" />
                </svg>
              </button>
            </div>
          </Show>
        </div>
      </div>

      {/* 로딩 오버레이 */}
      <Show when={props.loading}>
        <div class="absolute inset-0 flex items-center justify-center bg-[var(--color-surface)]/80 z-10">
          <div class="flex items-center gap-2">
            <div class="w-4 h-4 border-2 border-[var(--color-primary)] border-t-transparent rounded-full animate-spin" />
            <span class="text-sm text-[var(--color-text-muted)]">로딩 중...</span>
          </div>
        </div>
      </Show>

      {/* 차트 컨테이너 */}
      <div class={containerClass()}>
        {/* 메인 차트 (Primary TF) */}
        <div class="relative" style={chartWidthStyle()}>
          <div class="absolute top-2 left-2 z-10 flex items-center gap-1 px-2 py-0.5 bg-[var(--color-surface)]/80 rounded text-xs font-medium text-[var(--color-primary)]">
            <span class="w-2 h-2 rounded-full bg-[var(--color-primary)]" />
            {getTimeframeLabel(props.primaryTimeframe)} (Primary)
          </div>
          <div
            ref={mainChartContainer}
            class="w-full rounded-lg overflow-hidden border border-[var(--color-surface-light)]"
            style={{ height: `${props.mainHeight || 300}px` }}
          />
        </div>

        {/* 서브 차트들 (Secondary TF) */}
        <For each={props.secondaryData}>
          {(tfData, index) => {
            const isCollapsed = () => collapsedPanels().has(index())

            return (
              <div class="relative" style={chartWidthStyle()}>
                {/* 패널 헤더 (항상 표시) */}
                <div
                  class={`flex items-center justify-between px-2 py-1 bg-[var(--color-surface)]/80 rounded-t-lg border border-b-0 border-[var(--color-surface-light)] ${props.collapsible ? 'cursor-pointer hover:bg-[var(--color-surface)]' : ''}`}
                  onClick={() => props.collapsible && togglePanel(index())}
                >
                  <div class="flex items-center gap-1 text-xs font-medium text-[var(--color-text-muted)]">
                    <span class="w-2 h-2 rounded-full bg-[var(--color-text-muted)]" />
                    {getTimeframeLabel(tfData.timeframe)}
                  </div>

                  <div class="flex items-center gap-2">
                    {/* 지표 레전드 (축소 시에는 숨김) */}
                    <Show when={!isCollapsed() && tfData.indicators && tfData.indicators.length > 0}>
                      <div class="flex gap-2">
                        <For each={tfData.indicators}>
                          {(indicator) => (
                            <span
                              class="inline-flex items-center gap-1 text-xs"
                              style={{ color: indicator.color }}
                            >
                              <span
                                class="w-2 h-0.5 rounded"
                                style={{ 'background-color': indicator.color }}
                              />
                              {indicator.name}
                            </span>
                          )}
                        </For>
                      </div>
                    </Show>

                    {/* 접기/펼치기 버튼 */}
                    <Show when={props.collapsible}>
                      <button
                        class="p-0.5 rounded text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
                        onClick={(e) => {
                          e.stopPropagation()
                          togglePanel(index())
                        }}
                        title={isCollapsed() ? '펼치기' : '접기'}
                      >
                        <svg
                          class={`w-4 h-4 transition-transform ${isCollapsed() ? 'rotate-180' : ''}`}
                          fill="none"
                          stroke="currentColor"
                          viewBox="0 0 24 24"
                        >
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 15l7-7 7 7" />
                        </svg>
                      </button>
                    </Show>
                  </div>
                </div>

                {/* 차트 컨테이너 (접히면 숨김) */}
                <div
                  ref={(el) => {
                    subChartRefs[index()] = el
                    requestAnimationFrame(() => {
                      if (el && !subCharts[index()] && !isCollapsed()) {
                        createSubChart(el, tfData, index())
                      }
                    })
                  }}
                  class={`w-full rounded-b-lg overflow-hidden border border-t-0 border-[var(--color-surface-light)] transition-all duration-200 ${isCollapsed() ? 'hidden' : ''}`}
                  style={{ height: isCollapsed() ? '0px' : `${props.subHeight || 150}px` }}
                />
              </div>
            )
          }}
        </For>
      </div>

      {/* 빈 상태 */}
      <Show when={!props.primaryData || props.primaryData.length === 0}>
        <div class="flex items-center justify-center h-40 text-[var(--color-text-muted)]">
          <span>데이터가 없습니다</span>
        </div>
      </Show>
    </div>
  )
}
