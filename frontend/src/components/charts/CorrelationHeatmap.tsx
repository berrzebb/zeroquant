/**
 * 상관관계 히트맵 컴포넌트
 *
 * 종목 간 상관관계를 N x N 히트맵으로 시각화합니다.
 * 포트폴리오 다변화 분석에 활용됩니다.
 *
 * @example
 * ```tsx
 * <CorrelationHeatmap
 *   symbols={['AAPL', 'MSFT', 'GOOGL']}
 *   correlations={[
 *     [1.0, 0.85, 0.72],
 *     [0.85, 1.0, 0.68],
 *     [0.72, 0.68, 1.0]
 *   ]}
 * />
 * ```
 */
import { createMemo, For, Show } from 'solid-js'
import type { Component, JSX } from 'solid-js'
import { EChart } from '../ui/EChart'
import type { EChartsOption } from 'echarts'

export interface CorrelationHeatmapProps {
  /** 종목 심볼 배열 */
  symbols: string[]
  /** 상관관계 행렬 (N x N) - 값 범위: -1 ~ +1 */
  correlations: number[][]
  /** 셀 클릭 핸들러 */
  onCellClick?: (i: number, j: number, value: number) => void
  /** 차트 높이 (기본값: symbols.length * 40 + 100) */
  height?: number
  /** 색상 스케일 (기본: 빨강-흰색-파랑) */
  colorScale?: [string, string, string]
  /** 값 표시 여부 */
  showValues?: boolean
  /** 제목 */
  title?: string
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

/**
 * 상관관계 값에 따른 색상 계산
 */
function getCorrelationColor(value: number, colorScale: [string, string, string]): string {
  // -1 ~ 0 ~ +1 범위를 0 ~ 0.5 ~ 1로 정규화
  const normalized = (value + 1) / 2

  if (normalized < 0.5) {
    // -1 ~ 0: 빨강에서 흰색으로
    return colorScale[0]
  } else {
    // 0 ~ +1: 흰색에서 파랑으로
    return colorScale[2]
  }
}

/**
 * 상관관계 해석 텍스트
 */
function getCorrelationLabel(value: number): string {
  const abs = Math.abs(value)
  if (abs >= 0.9) return '매우 강한'
  if (abs >= 0.7) return '강한'
  if (abs >= 0.5) return '중간'
  if (abs >= 0.3) return '약한'
  return '매우 약한'
}

/**
 * CorrelationHeatmap 컴포넌트
 */
export const CorrelationHeatmap: Component<CorrelationHeatmapProps> = (props) => {
  const colorScale = () => props.colorScale ?? ['#ef4444', '#f5f5f5', '#3b82f6']
  const showValues = () => props.showValues ?? true
  const chartHeight = () => props.height ?? Math.max(300, props.symbols.length * 40 + 100)

  // 히트맵 데이터 변환 (ECharts 형식)
  const heatmapData = createMemo(() => {
    const data: [number, number, number][] = []
    const n = props.symbols.length

    for (let i = 0; i < n; i++) {
      for (let j = 0; j < n; j++) {
        data.push([j, i, props.correlations[i]?.[j] ?? 0])
      }
    }

    return data
  })

  // ECharts 옵션
  const chartOption = createMemo((): EChartsOption => {
    const symbols = props.symbols
    const n = symbols.length

    return {
      title: props.title
        ? {
            text: props.title,
            left: 'center',
            textStyle: { color: '#e5e7eb', fontSize: 16 },
          }
        : undefined,
      tooltip: {
        backgroundColor: 'rgba(30, 30, 40, 0.95)',
        borderColor: '#374151',
        textStyle: { color: '#e5e7eb' },
        formatter: (params: unknown) => {
          const p = params as { data: [number, number, number] }
          const [j, i, value] = p.data
          const sym1 = symbols[i]
          const sym2 = symbols[j]

          if (i === j) {
            return `<div style="font-weight: 600">${sym1}</div>
                    <div style="color: #9ca3af; margin-top: 4px">자기 상관 (항상 1.0)</div>`
          }

          const direction = value >= 0 ? '양의' : '음의'
          const strength = getCorrelationLabel(value)
          const color = value >= 0 ? '#3b82f6' : '#ef4444'

          return `<div style="font-weight: 600">${sym1} ↔ ${sym2}</div>
                  <div style="margin-top: 8px">
                    <span style="color: #9ca3af">상관계수:</span>
                    <span style="color: ${color}; font-weight: 600; margin-left: 8px">${value >= 0 ? '+' : ''}${value.toFixed(3)}</span>
                  </div>
                  <div style="color: #9ca3af; margin-top: 4px">
                    ${strength} ${direction} 상관관계
                  </div>`
        },
      },
      grid: {
        top: props.title ? 60 : 30,
        right: 120,
        bottom: 80,
        left: 80,
      },
      xAxis: {
        type: 'category',
        data: symbols,
        position: 'bottom',
        axisLine: { lineStyle: { color: '#4b5563' } },
        axisLabel: {
          color: '#9ca3af',
          rotate: 45,
          fontSize: 11,
        },
        splitArea: { show: false },
      },
      yAxis: {
        type: 'category',
        data: symbols,
        axisLine: { lineStyle: { color: '#4b5563' } },
        axisLabel: {
          color: '#9ca3af',
          fontSize: 11,
        },
        splitArea: { show: false },
      },
      visualMap: {
        type: 'continuous',
        min: -1,
        max: 1,
        calculable: true,
        orient: 'vertical',
        right: 10,
        top: 'center',
        itemHeight: 150,
        inRange: {
          color: colorScale(),
        },
        text: ['+1.0', '-1.0'],
        textStyle: { color: '#9ca3af' },
      },
      series: [
        {
          type: 'heatmap',
          data: heatmapData(),
          label: {
            show: showValues(),
            formatter: (params: unknown) => {
              const p = params as { data: [number, number, number] }
              const value = p.data[2]
              // 대각선은 빈 문자열
              if (p.data[0] === p.data[1]) return ''
              return value.toFixed(2)
            },
            fontSize: n > 8 ? 9 : 11,
            color: '#1f2937',
          },
          emphasis: {
            itemStyle: {
              borderColor: '#60a5fa',
              borderWidth: 2,
            },
          },
          itemStyle: {
            borderColor: '#1f2937',
            borderWidth: 1,
          },
        },
      ],
    }
  })

  // 클릭 핸들러
  const handleClick = (params: unknown) => {
    if (props.onCellClick) {
      const p = params as { data: [number, number, number] }
      const [j, i, value] = p.data
      props.onCellClick(i, j, value)
    }
  }

  return (
    <Show
      when={props.symbols.length > 0 && props.correlations.length > 0}
      fallback={
        <div
          class={`flex items-center justify-center bg-gray-800/50 rounded-xl ${props.class || ''}`}
          style={{ height: `${chartHeight()}px`, ...props.style }}
        >
          <div class="text-gray-500 text-sm">상관관계 데이터 없음</div>
        </div>
      }
    >
      <EChart
        option={chartOption()}
        height={chartHeight()}
        class={props.class}
        style={props.style}
        onClick={handleClick}
      />
    </Show>
  )
}

/**
 * 미니 상관관계 매트릭스 (간단한 테이블 형식)
 */
export interface MiniCorrelationMatrixProps {
  /** 종목 심볼 배열 */
  symbols: string[]
  /** 상관관계 행렬 */
  correlations: number[][]
  /** 셀 클릭 핸들러 */
  onCellClick?: (i: number, j: number) => void
  /** 추가 클래스 */
  class?: string
}

export const MiniCorrelationMatrix: Component<MiniCorrelationMatrixProps> = (props) => {
  const getCellColor = (value: number) => {
    if (value >= 0.7) return 'bg-blue-500'
    if (value >= 0.3) return 'bg-blue-400/60'
    if (value >= -0.3) return 'bg-gray-500'
    if (value >= -0.7) return 'bg-red-400/60'
    return 'bg-red-500'
  }

  return (
    <div class={`overflow-x-auto ${props.class || ''}`}>
      <table class="text-xs">
        <thead>
          <tr>
            <th class="p-1" />
            <For each={props.symbols}>
              {(sym) => (
                <th class="p-1 text-gray-400 font-medium">{sym}</th>
              )}
            </For>
          </tr>
        </thead>
        <tbody>
          <For each={props.symbols}>
            {(sym, i) => (
              <tr>
                <td class="p-1 text-gray-400 font-medium">{sym}</td>
                <For each={props.correlations[i()]}>
                  {(value, j) => (
                    <td
                      class={`p-1 text-center cursor-pointer hover:opacity-80 transition-opacity ${getCellColor(value)}`}
                      onClick={() => props.onCellClick?.(i(), j())}
                      title={`${props.symbols[i()]} ↔ ${props.symbols[j()]}: ${value.toFixed(2)}`}
                    >
                      <span class="text-white font-mono">
                        {i() === j() ? '-' : value.toFixed(2)}
                      </span>
                    </td>
                  )}
                </For>
              </tr>
            )}
          </For>
        </tbody>
      </table>
    </div>
  )
}

export default CorrelationHeatmap
