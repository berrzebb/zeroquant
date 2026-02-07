/**
 * 섹터 모멘텀 바 차트 컴포넌트
 *
 * 섹터별 5일 수익률을 수평 막대 차트로 표시합니다.
 * TOP 10 / BOTTOM 10 탭으로 전환하여 볼 수 있습니다.
 *
 * @example
 * ```tsx
 * <SectorMomentumBar
 *   sectors={[
 *     { name: '반도체', return5d: 5.2 },
 *     { name: '2차전지', return5d: 3.8 },
 *     { name: '건설', return5d: -2.1 },
 *   ]}
 *   limit={10}
 * />
 * ```
 */
import { createMemo, createSignal, Show } from 'solid-js'
import type { Component, JSX } from 'solid-js'
import { EChart } from '../ui/EChart'
import type { EChartsOption } from 'echarts'

/** 섹터 데이터 */
export interface SectorMomentum {
  /** 섹터명 */
  name: string
  /** 5일 수익률 (%) */
  return5d: number
  /** 추가 정보 (선택) */
  symbolCount?: number
  /** 시가총액 */
  marketCap?: number
}

export interface SectorMomentumBarProps {
  /** 섹터 데이터 배열 */
  sectors: SectorMomentum[]
  /** 표시 개수 (기본 10) */
  limit?: number
  /** 클릭 핸들러 */
  onSectorClick?: (sector: string) => void
  /** 차트 높이 (기본: limit * 35 + 80) */
  height?: number
  /** 제목 */
  title?: string
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

/** 탭 타입 */
type TabType = 'top' | 'bottom'

/**
 * SectorMomentumBar 컴포넌트
 */
export const SectorMomentumBar: Component<SectorMomentumBarProps> = (props) => {
  const limit = () => props.limit ?? 10
  const [activeTab, setActiveTab] = createSignal<TabType>('top')

  // 정렬된 섹터 데이터 (undefined 방어 처리)
  const sortedSectors = createMemo(() => {
    const sorted = [...props.sectors].sort((a, b) => (b.return5d ?? 0) - (a.return5d ?? 0))
    return sorted
  })

  // 표시할 섹터 (탭에 따라)
  const displaySectors = createMemo(() => {
    const sorted = sortedSectors()
    if (activeTab() === 'top') {
      return sorted.slice(0, limit())
    } else {
      return sorted.slice(-limit()).reverse()
    }
  })

  // 차트 높이 계산
  const chartHeight = createMemo(() =>
    props.height ?? Math.max(200, displaySectors().length * 35 + 80)
  )

  // ECharts 옵션
  const chartOption = createMemo((): EChartsOption => {
    const sectors = displaySectors()
    const isTop = activeTab() === 'top'

    // 섹터명 (Y축)
    const sectorNames = sectors.map((s) => s.name).reverse()
    // 수익률 (X축) - undefined 방어 처리
    const returns = sectors.map((s) => s.return5d ?? 0).reverse()

    // 최대/최소값으로 대칭 범위 설정
    const maxAbs = Math.max(...returns.map(Math.abs), 1)
    const xAxisMax = Math.ceil(maxAbs * 1.2)

    return {
      title: props.title
        ? {
            text: props.title,
            left: 'center',
            textStyle: { color: '#e5e7eb', fontSize: 14 },
          }
        : undefined,
      tooltip: {
        trigger: 'axis',
        axisPointer: { type: 'shadow' },
        backgroundColor: 'rgba(30, 30, 40, 0.95)',
        borderColor: '#374151',
        textStyle: { color: '#e5e7eb' },
        formatter: (params: unknown) => {
          const arr = params as { name: string; value: number }[]
          if (!arr || arr.length === 0) return ''
          const p = arr[0]
          if (!p || p.value === undefined || p.value === null) return ''

          const sector = sectors.find((s) => s.name === p.name)
          const value = typeof p.value === 'number' ? p.value : 0
          const color = value >= 0 ? '#22c55e' : '#ef4444'
          const sign = value >= 0 ? '+' : ''

          return `
            <div style="font-weight: 600;">${p.name || '알 수 없음'}</div>
            <div style="margin-top: 8px;">
              <span style="color: #9ca3af;">5일 수익률:</span>
              <span style="color: ${color}; font-weight: 600; margin-left: 8px;">
                ${sign}${value.toFixed(2)}%
              </span>
            </div>
            ${sector?.symbolCount ? `
              <div style="color: #9ca3af; margin-top: 4px;">
                종목 수: ${sector.symbolCount}개
              </div>
            ` : ''}
          `
        },
      },
      grid: {
        top: props.title ? 50 : 20,
        right: 60,
        bottom: 20,
        left: 80,
      },
      xAxis: {
        type: 'value',
        min: -xAxisMax,
        max: xAxisMax,
        axisLine: { lineStyle: { color: '#4b5563' } },
        axisLabel: {
          color: '#9ca3af',
          formatter: (value: number) => `${value}%`,
        },
        splitLine: { lineStyle: { color: '#374151', type: 'dashed' } },
      },
      yAxis: {
        type: 'category',
        data: sectorNames,
        axisLine: { lineStyle: { color: '#4b5563' } },
        axisLabel: {
          color: '#e5e7eb',
          fontSize: 11,
        },
      },
      series: [
        {
          type: 'bar',
          data: returns.map((value) => ({
            value,
            itemStyle: {
              color: value >= 0 ? '#22c55e' : '#ef4444',
              borderRadius: value >= 0 ? [0, 4, 4, 0] : [4, 0, 0, 4],
            },
          })),
          label: {
            show: true,
            position: 'right',
            formatter: (params: unknown) => {
              const p = params as { value: number }
              const sign = p.value >= 0 ? '+' : ''
              return `${sign}${p.value.toFixed(1)}%`
            },
            color: '#9ca3af',
            fontSize: 10,
          },
          barWidth: '60%',
        },
      ],
    }
  })

  // 클릭 핸들러
  const handleClick = (params: unknown) => {
    const p = params as { name?: string }
    if (p.name && props.onSectorClick) {
      props.onSectorClick(p.name)
    }
  }

  return (
    <div
      class={`bg-gray-800/50 rounded-xl overflow-hidden ${props.class || ''}`}
      style={props.style}
    >
      {/* 탭 헤더 */}
      <div class="flex items-center justify-between p-3 border-b border-gray-700">
        <div class="flex items-center gap-2">
          <span class="text-lg">📊</span>
          <h3 class="text-sm font-semibold text-white">섹터 모멘텀</h3>
        </div>
        <div class="flex rounded-lg overflow-hidden border border-gray-600">
          <button
            class={`
              px-3 py-1 text-xs font-medium transition-colors
              ${activeTab() === 'top'
                ? 'bg-green-500/20 text-green-400 border-r border-gray-600'
                : 'bg-gray-700/50 text-gray-400 hover:text-white border-r border-gray-600'}
            `}
            onClick={() => setActiveTab('top')}
          >
            TOP {limit()}
          </button>
          <button
            class={`
              px-3 py-1 text-xs font-medium transition-colors
              ${activeTab() === 'bottom'
                ? 'bg-red-500/20 text-red-400'
                : 'bg-gray-700/50 text-gray-400 hover:text-white'}
            `}
            onClick={() => setActiveTab('bottom')}
          >
            BOTTOM {limit()}
          </button>
        </div>
      </div>

      {/* 차트 */}
      <Show
        when={displaySectors().length > 0}
        fallback={
          <div
            class="flex items-center justify-center text-gray-500 text-sm"
            style={{ height: `${chartHeight()}px` }}
          >
            섹터 데이터 없음
          </div>
        }
      >
        <EChart
          option={chartOption()}
          height={chartHeight()}
          onClick={handleClick}
        />
      </Show>

      {/* 요약 정보 */}
      <div class="p-3 border-t border-gray-700 flex items-center justify-between text-xs text-gray-400">
        <span>
          총 {props.sectors.length}개 섹터
        </span>
        <span>
          평균: {props.sectors.length > 0
            ? (props.sectors.reduce((sum, s) => sum + (s.return5d ?? 0), 0) / props.sectors.length).toFixed(2)
            : '0.00'}%
        </span>
      </div>
    </div>
  )
}

export default SectorMomentumBar
