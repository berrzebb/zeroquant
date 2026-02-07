/**
 * Score Waterfall 차트 컴포넌트
 *
 * 점수의 구성 요소를 워터폴 차트로 시각화합니다.
 * 각 요소가 총점에 어떻게 기여하는지 보여줍니다.
 *
 * @example
 * ```tsx
 * <ScoreWaterfall
 *   data={[
 *     { name: '모멘텀', value: 15 },
 *     { name: '가치', value: 12 },
 *     { name: '품질', value: -5 },
 *     { name: '변동성', value: 8 },
 *   ]}
 *   total={30}
 * />
 * ```
 */
import { createMemo, Show } from 'solid-js'
import type { Component, JSX } from 'solid-js'
import { EChart, CHART_COLORS } from '../ui/EChart'
import type { EChartsOption } from 'echarts'

/** 워터폴 데이터 항목 */
export interface WaterfallDataItem {
  /** 항목 이름 */
  name: string
  /** 값 (양수 또는 음수) */
  value: number
  /** 커스텀 색상 (옵션) */
  color?: string
}

export interface ScoreWaterfallProps {
  /** 데이터 항목 배열 */
  data: WaterfallDataItem[]
  /** 총점 (표시용, 계산되지 않으면 데이터에서 합산) */
  total?: number
  /** 차트 높이 (기본: 300px) */
  height?: number
  /** 제목 */
  title?: string
  /** 가로 모드 */
  horizontal?: boolean
  /** 시작 라벨 */
  startLabel?: string
  /** 종료 라벨 */
  endLabel?: string
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

/** 기본 색상 */
const COLORS = {
  positive: '#22c55e', // 녹색
  negative: '#ef4444', // 빨간색
  total: '#3b82f6', // 파란색
  transparent: 'transparent',
}

/**
 * Score Waterfall 컴포넌트
 *
 * 점수 구성 요소를 워터폴 형식으로 시각화합니다.
 * - 양수 값: 녹색 바
 * - 음수 값: 빨간색 바
 * - 합계: 파란색 바
 */
export const ScoreWaterfall: Component<ScoreWaterfallProps> = (props) => {
  // 총점 계산
  const calculatedTotal = createMemo(() => {
    if (props.total !== undefined) return props.total
    return props.data.reduce((sum, item) => sum + item.value, 0)
  })

  // 워터폴 데이터 생성
  const waterfallData = createMemo(() => {
    const items = props.data
    const result: {
      categories: string[]
      stack: number[]   // 투명 스택 (누적 시작점)
      values: number[]  // 실제 값
      colors: string[]  // 바 색상
    } = {
      categories: [],
      stack: [],
      values: [],
      colors: [],
    }

    let cumulative = 0

    // 시작점 (0)
    result.categories.push(props.startLabel || '시작')
    result.stack.push(0)
    result.values.push(0)
    result.colors.push(COLORS.transparent)

    // 각 항목
    items.forEach((item) => {
      result.categories.push(item.name)

      if (item.value >= 0) {
        // 양수: 현재 누적값에서 시작
        result.stack.push(cumulative)
        result.values.push(item.value)
        result.colors.push(item.color || COLORS.positive)
      } else {
        // 음수: 새 누적값에서 시작 (값을 빼고)
        result.stack.push(cumulative + item.value)
        result.values.push(Math.abs(item.value))
        result.colors.push(item.color || COLORS.negative)
      }

      cumulative += item.value
    })

    // 합계
    result.categories.push(props.endLabel || '합계')
    result.stack.push(0)
    result.values.push(Math.max(0, calculatedTotal()))
    result.colors.push(COLORS.total)

    return result
  })

  // ECharts 옵션
  const chartOption = createMemo((): EChartsOption => {
    const data = waterfallData()
    const isHorizontal = props.horizontal

    return {
      title: props.title
        ? {
            text: props.title,
            left: 'center',
            textStyle: {
              color: '#e5e7eb',
              fontSize: 14,
            },
          }
        : undefined,
      tooltip: {
        trigger: 'axis',
        axisPointer: {
          type: 'shadow',
        },
        backgroundColor: 'rgba(17, 24, 39, 0.9)',
        borderColor: 'rgba(55, 65, 81, 0.8)',
        textStyle: {
          color: '#e5e7eb',
        },
        formatter: (params: any) => {
          const idx = params[0].dataIndex
          if (idx === 0) return `${data.categories[idx]}: 0`
          if (idx === data.categories.length - 1) {
            return `${data.categories[idx]}: <b>${calculatedTotal().toFixed(1)}</b>`
          }
          const item = props.data[idx - 1]
          const sign = item.value >= 0 ? '+' : ''
          return `${item.name}: <b style="color: ${item.value >= 0 ? COLORS.positive : COLORS.negative}">${sign}${item.value.toFixed(1)}</b>`
        },
      },
      grid: {
        top: props.title ? 50 : 20,
        bottom: isHorizontal ? 30 : 50,
        left: isHorizontal ? 80 : 50,
        right: 20,
        containLabel: true,
      },
      xAxis: isHorizontal
        ? {
            type: 'value',
            axisLine: { lineStyle: { color: '#4b5563' } },
            axisTick: { show: false },
            axisLabel: { color: '#9ca3af', fontSize: 11 },
            splitLine: { lineStyle: { color: '#374151', type: 'dashed' } },
          }
        : {
            type: 'category',
            data: data.categories,
            axisLine: { lineStyle: { color: '#4b5563' } },
            axisTick: { show: false },
            axisLabel: {
              color: '#9ca3af',
              fontSize: 11,
              rotate: data.categories.length > 6 ? 45 : 0,
            },
          },
      yAxis: isHorizontal
        ? {
            type: 'category',
            data: data.categories,
            axisLine: { lineStyle: { color: '#4b5563' } },
            axisTick: { show: false },
            axisLabel: { color: '#9ca3af', fontSize: 11 },
          }
        : {
            type: 'value',
            axisLine: { lineStyle: { color: '#4b5563' } },
            axisTick: { show: false },
            axisLabel: { color: '#9ca3af', fontSize: 11 },
            splitLine: { lineStyle: { color: '#374151', type: 'dashed' } },
          },
      series: [
        // 투명 스택 (시작점)
        {
          name: 'Stack',
          type: 'bar',
          stack: 'Total',
          silent: true,
          itemStyle: {
            color: 'transparent',
          },
          emphasis: {
            disabled: true,
          },
          data: data.stack,
        },
        // 실제 값
        {
          name: 'Value',
          type: 'bar',
          stack: 'Total',
          data: data.values.map((val, idx) => ({
            value: val,
            itemStyle: {
              color: data.colors[idx],
              borderRadius: [4, 4, 4, 4],
            },
          })),
          label: {
            show: true,
            position: isHorizontal ? 'right' : 'top',
            color: '#e5e7eb',
            fontSize: 11,
            formatter: (params: any) => {
              const idx = params.dataIndex
              if (idx === 0) return ''
              if (idx === data.categories.length - 1) {
                return calculatedTotal().toFixed(1)
              }
              const item = props.data[idx - 1]
              const sign = item.value >= 0 ? '+' : ''
              return `${sign}${item.value.toFixed(1)}`
            },
          },
          barWidth: '50%',
          barMaxWidth: 40,
        },
      ],
      backgroundColor: 'transparent',
    }
  })

  const chartHeight = createMemo(() => props.height || 300)

  return (
    <div
      class={`rounded-xl overflow-hidden ${props.class || ''}`}
      style={props.style}
    >
      <Show
        when={props.data.length > 0}
        fallback={
          <div
            class="flex items-center justify-center bg-gray-800/50 rounded-xl"
            style={{ height: `${chartHeight()}px` }}
          >
            <span class="text-gray-400">데이터가 없습니다</span>
          </div>
        }
      >
        <EChart
          option={chartOption()}
          height={chartHeight()}
          theme="dark"
        />
      </Show>
    </div>
  )
}

/**
 * 7Factor 점수 워터폴
 *
 * 7가지 요소의 점수 기여도를 워터폴 차트로 표시합니다.
 */
export interface Factor7WaterfallProps {
  /** 모멘텀 점수 */
  momentum: number
  /** 가치 점수 */
  value: number
  /** 품질 점수 */
  quality: number
  /** 변동성 점수 */
  volatility: number
  /** 유동성 점수 */
  liquidity: number
  /** 성장성 점수 */
  growth: number
  /** 심리 점수 */
  sentiment: number
  /** 차트 높이 */
  height?: number
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

export const Factor7Waterfall: Component<Factor7WaterfallProps> = (props) => {
  const data = createMemo((): WaterfallDataItem[] => [
    { name: '모멘텀', value: props.momentum, color: '#3b82f6' },
    { name: '가치', value: props.value, color: '#8b5cf6' },
    { name: '품질', value: props.quality, color: '#22c55e' },
    { name: '변동성', value: props.volatility, color: '#f59e0b' },
    { name: '유동성', value: props.liquidity, color: '#06b6d4' },
    { name: '성장성', value: props.growth, color: '#ec4899' },
    { name: '심리', value: props.sentiment, color: '#84cc16' },
  ])

  const total = createMemo(() =>
    props.momentum +
    props.value +
    props.quality +
    props.volatility +
    props.liquidity +
    props.growth +
    props.sentiment
  )

  return (
    <ScoreWaterfall
      data={data()}
      total={total()}
      height={props.height}
      title="7Factor 점수 구성"
      startLabel="기준"
      endLabel="총점"
      class={props.class}
      style={props.style}
    />
  )
}

export default ScoreWaterfall
