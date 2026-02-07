/**
 * 트리맵 차트 컴포넌트
 *
 * ECharts 기반으로 계층적 데이터를 시각화합니다.
 * 섹터맵, 포트폴리오 비중 등에 사용됩니다.
 */
import { createMemo, Show } from 'solid-js'
import { EChart } from './EChart'
import type { EChartsOption } from 'echarts'

// 트리맵 데이터 인터페이스
export interface TreemapDataItem {
  /** 항목 이름 */
  name: string
  /** 값 (영역 크기 결정) */
  value: number
  /** 하위 항목 */
  children?: TreemapDataItem[]
  /** 색상 값 (colorField가 지정된 경우 visualMap에 사용) */
  colorValue?: number
  /** 추가 메타데이터 */
  meta?: Record<string, unknown>
}

export interface TreemapChartProps {
  /** 트리맵 데이터 */
  data: TreemapDataItem[]
  /** 차트 높이 (기본: 400) */
  height?: number
  /** 차트 제목 */
  title?: string
  /** 색상 범위 (수익률 등) 표시 여부 */
  showVisualMap?: boolean
  /** visualMap 최소값 */
  visualMapMin?: number
  /** visualMap 최대값 */
  visualMapMax?: number
  /** 색상 범위 (min to max) */
  colorRange?: [string, string, string]
  /** 툴팁 포맷터 */
  tooltipFormatter?: (item: TreemapDataItem) => string
  /** 클릭 핸들러 */
  onClick?: (item: TreemapDataItem) => void
  /** 추가 클래스 */
  class?: string
}

/**
 * 트리맵 차트 컴포넌트
 *
 * @example
 * ```tsx
 * <TreemapChart
 *   data={[
 *     { name: '기술', value: 100, children: [
 *       { name: 'AAPL', value: 50, colorValue: 2.5 },
 *       { name: 'MSFT', value: 50, colorValue: -1.2 },
 *     ]},
 *   ]}
 *   showVisualMap
 *   visualMapMin={-5}
 *   visualMapMax={5}
 * />
 * ```
 */
// colorValue를 RGB 색상으로 변환하는 함수
function getColorByValue(
  value: number,
  min: number,
  max: number,
  colors: [string, string, string]
): string {
  // value를 0~1 범위로 정규화
  const range = max - min
  const normalized = range === 0 ? 0.5 : Math.max(0, Math.min(1, (value - min) / range))

  // 빨강(0) -> 회색(0.5) -> 초록(1)
  const hexToRgb = (hex: string) => {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex)
    return result
      ? [parseInt(result[1], 16), parseInt(result[2], 16), parseInt(result[3], 16)]
      : [128, 128, 128]
  }

  const [r1, g1, b1] = hexToRgb(colors[0]) // red
  const [r2, g2, b2] = hexToRgb(colors[1]) // gray
  const [r3, g3, b3] = hexToRgb(colors[2]) // green

  let r: number, g: number, b: number

  if (normalized <= 0.5) {
    // red -> gray
    const t = normalized * 2
    r = Math.round(r1 + (r2 - r1) * t)
    g = Math.round(g1 + (g2 - g1) * t)
    b = Math.round(b1 + (b2 - b1) * t)
  } else {
    // gray -> green
    const t = (normalized - 0.5) * 2
    r = Math.round(r2 + (r3 - r2) * t)
    g = Math.round(g2 + (g3 - g2) * t)
    b = Math.round(b2 + (b3 - b2) * t)
  }

  return `rgb(${r}, ${g}, ${b})`
}

export function TreemapChart(props: TreemapChartProps) {
  const height = () => props.height || 400

  // 색상 범위 기본값 (빨강 - 회색 - 초록)
  const colorRange = () => props.colorRange || ['#ef4444', '#4b5563', '#22c55e']

  // ECharts 옵션
  const chartOption = createMemo((): EChartsOption => {
    const options: EChartsOption = {
      tooltip: {
        backgroundColor: 'rgba(30, 30, 40, 0.95)',
        borderColor: '#374151',
        textStyle: {
          color: '#e5e7eb',
        },
        formatter: (params: unknown) => {
          const p = params as { data: TreemapDataItem; value: number; treePathInfo: { name: string }[] }
          if (props.tooltipFormatter) {
            return props.tooltipFormatter(p.data)
          }
          // 기본 툴팁
          const path = p.treePathInfo?.map((i) => i.name).join(' > ') || p.data.name
          const colorVal = p.data.colorValue !== undefined ? p.data.colorValue.toFixed(2) : ''
          return `
            <div style="font-weight: 600;">${path}</div>
            <div style="margin-top: 4px;">
              <span style="color: #9ca3af;">크기:</span> ${p.value.toLocaleString()}
              ${colorVal ? `<br/><span style="color: #9ca3af;">변화:</span> ${colorVal}%` : ''}
            </div>
          `
        },
      },
      series: [
        {
          type: 'treemap',
          roam: false,
          nodeClick: false,
          visibleMin: 300,
          top: 30,
          bottom: props.showVisualMap ? 45 : 5,
          left: 5,
          right: 5,
          breadcrumb: {
            show: false,
          },
          label: {
            show: true,
            formatter: '{b}',
            fontSize: 12,
            color: '#fff',
            textShadowBlur: 2,
            textShadowColor: 'rgba(0,0,0,0.5)',
          },
          upperLabel: {
            show: true,
            height: 24,
            color: '#fff',
            backgroundColor: 'rgba(0,0,0,0.3)',
          },
          itemStyle: {
            borderColor: '#1f2937',
            borderWidth: 2,
            gapWidth: 2,
          },
          levels: [
            {
              // 최상위 레벨 (섹터)
              itemStyle: {
                borderColor: '#374151',
                borderWidth: 3,
                gapWidth: 3,
              },
              upperLabel: {
                show: true,
                height: 28,
                fontSize: 14,
                fontWeight: 'bold',
              },
            },
            {
              // 두 번째 레벨 (업종)
              itemStyle: {
                borderColor: '#4b5563',
                borderWidth: 2,
                gapWidth: 2,
              },
              upperLabel: {
                show: true,
                height: 22,
                fontSize: 12,
              },
            },
            {
              // 세 번째 레벨 (종목)
              itemStyle: {
                borderColor: '#6b7280',
                borderWidth: 1,
                gapWidth: 1,
              },
              label: {
                show: true,
                fontSize: 10,
              },
            },
          ],
          data: props.showVisualMap
            ? props.data.map((item) => ({
                ...item,
                itemStyle: {
                  color: getColorByValue(
                    item.colorValue ?? 0,
                    props.visualMapMin ?? -5,
                    props.visualMapMax ?? 5,
                    colorRange() as [string, string, string]
                  ),
                },
              }))
            : props.data,
        },
      ],
    }

    // visualMap 추가 (색상 범위 레전드) - 차트 하단에 수평 배치
    if (props.showVisualMap) {
      const minVal = props.visualMapMin ?? -5
      const maxVal = props.visualMapMax ?? 5
      options.visualMap = {
        type: 'piecewise',
        show: true,
        min: minVal,
        max: maxVal,
        splitNumber: 5,
        calculable: false,
        seriesIndex: 0,
        pieces: [
          { min: minVal, max: minVal * 0.6, label: `${minVal}%`, color: colorRange()[0] },
          { min: minVal * 0.6, max: minVal * 0.2, label: '', color: '#b45555' },
          { min: minVal * 0.2, max: maxVal * 0.2, label: '0%', color: colorRange()[1] },
          { min: maxVal * 0.2, max: maxVal * 0.6, label: '', color: '#55b455' },
          { min: maxVal * 0.6, max: maxVal, label: `+${maxVal}%`, color: colorRange()[2] },
        ],
        textStyle: {
          color: '#9ca3af',
          fontSize: 10,
        },
        orient: 'horizontal',
        left: 'center',
        bottom: 5,
        itemWidth: 25,
        itemHeight: 10,
        itemGap: 5,
      }
    }

    // 제목 추가
    if (props.title) {
      options.title = {
        text: props.title,
        left: 'center',
        textStyle: {
          color: '#e5e7eb',
          fontSize: 16,
        },
      }
    }

    return options
  })

  // 클릭 핸들러
  const handleClick = (params: unknown) => {
    if (props.onClick) {
      const p = params as { data: TreemapDataItem }
      props.onClick(p.data)
    }
  }

  return (
    <Show
      when={props.data.length > 0}
      fallback={
        <div
          class={`flex items-center justify-center bg-gray-800/50 rounded-xl ${props.class || ''}`}
          style={{ height: `${height()}px` }}
        >
          <span class="text-gray-500 text-sm">데이터 없음</span>
        </div>
      }
    >
      <EChart
        option={chartOption()}
        height={height()}
        class={props.class}
        onClick={handleClick}
      />
    </Show>
  )
}

export default TreemapChart
