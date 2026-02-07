/**
 * ECharts 범용 래퍼 컴포넌트
 *
 * Apache ECharts를 SolidJS에서 사용할 수 있도록 래핑합니다.
 * 파이/도넛 차트, 트리맵, 히트맵 등 다양한 차트를 지원합니다.
 */
import { onMount, onCleanup, createEffect, on } from 'solid-js'
import type { Component, JSX } from 'solid-js'
import * as echarts from 'echarts/core'
import { PieChart, TreemapChart, BarChart, LineChart, HeatmapChart, GaugeChart, ScatterChart, CandlestickChart } from 'echarts/charts'
import {
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
  VisualMapComponent,
  GraphicComponent,
  MarkLineComponent,
  MarkPointComponent,
} from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import type { EChartsOption, ECharts } from 'echarts'

// ECharts 모듈 등록 (tree-shaking을 위해 필요한 것만 등록)
echarts.use([
  PieChart,
  TreemapChart,
  BarChart,
  LineChart,
  HeatmapChart,
  GaugeChart,
  ScatterChart,       // 기회맵(OpportunityMap) 등에 사용
  CandlestickChart,   // 캔들스틱 차트 (종목 상세)
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
  VisualMapComponent,
  GraphicComponent,
  MarkLineComponent,  // 마크라인 (기준선 표시)
  MarkPointComponent, // 마크포인트 (포인트 강조)
  CanvasRenderer,
])

export interface EChartProps {
  /** ECharts 옵션 */
  option: EChartsOption
  /** 차트 너비 (기본: 100%) */
  width?: string | number
  /** 차트 높이 (기본: 300px) */
  height?: string | number
  /** 테마 (기본: dark) */
  theme?: 'dark' | 'light' | object
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
  /** 차트 클릭 이벤트 */
  onClick?: (params: unknown) => void
  /** 차트 인스턴스 콜백 */
  onInit?: (chart: ECharts) => void
}

/**
 * ECharts 범용 컴포넌트
 *
 * @example
 * ```tsx
 * <EChart
 *   option={{
 *     series: [{
 *       type: 'pie',
 *       data: [{ value: 100, name: 'A' }, { value: 200, name: 'B' }]
 *     }]
 *   }}
 *   height={300}
 * />
 * ```
 */
export const EChart: Component<EChartProps> = (props) => {
  let containerRef: HTMLDivElement | undefined
  let chartInstance: ECharts | undefined

  // 스타일 계산
  const getStyle = (): JSX.CSSProperties => ({
    width: typeof props.width === 'number' ? `${props.width}px` : (props.width || '100%'),
    height: typeof props.height === 'number' ? `${props.height}px` : (props.height || '300px'),
    ...props.style,
  })

  onMount(() => {
    if (!containerRef) return

    // DOM 레이아웃이 완료된 후 차트 초기화 (라우트 전환 시 clientWidth=0 방지)
    const initChart = () => {
      if (!containerRef || containerRef.clientWidth === 0 || containerRef.clientHeight === 0) {
        // 레이아웃이 아직 완료되지 않은 경우 한 번 더 대기
        requestAnimationFrame(initChart)
        return
      }

      chartInstance = echarts.init(containerRef, props.theme || 'dark')
      chartInstance.setOption(props.option)

      // 클릭 이벤트 등록
      if (props.onClick) {
        chartInstance.on('click', props.onClick)
      }

      // 콜백 호출
      if (props.onInit) {
        props.onInit(chartInstance)
      }
    }

    requestAnimationFrame(initChart)

    // 리사이즈 핸들러
    const handleResize = () => {
      chartInstance?.resize()
    }
    window.addEventListener('resize', handleResize)

    // ResizeObserver로 컨테이너 크기 변화 감지
    const resizeObserver = new ResizeObserver(() => {
      chartInstance?.resize()
    })
    resizeObserver.observe(containerRef)

    onCleanup(() => {
      window.removeEventListener('resize', handleResize)
      resizeObserver.disconnect()
      chartInstance?.dispose()
    })
  })

  // 옵션 변경 시 업데이트
  createEffect(
    on(
      () => props.option,
      (option) => {
        if (chartInstance && option) {
          chartInstance.setOption(option, true)
        }
      },
      { defer: true }
    )
  )

  return (
    <div
      ref={containerRef}
      class={props.class}
      style={getStyle()}
    />
  )
}

// 다크 테마 기본 색상 (ZeroQuant 스타일)
export const CHART_COLORS = {
  primary: '#3b82f6',
  success: '#22c55e',
  warning: '#f59e0b',
  danger: '#ef4444',
  info: '#06b6d4',
  purple: '#8b5cf6',
  pink: '#ec4899',
  lime: '#84cc16',
  orange: '#f97316',
  indigo: '#6366f1',
}

// 미리 정의된 색상 팔레트
export const CHART_PALETTE = [
  CHART_COLORS.primary,
  CHART_COLORS.success,
  CHART_COLORS.warning,
  CHART_COLORS.danger,
  CHART_COLORS.purple,
  CHART_COLORS.pink,
  CHART_COLORS.info,
  CHART_COLORS.lime,
  CHART_COLORS.orange,
  CHART_COLORS.indigo,
]

export default EChart
