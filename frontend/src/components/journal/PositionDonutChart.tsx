/**
 * 포지션 비중 도넛 차트 컴포넌트
 *
 * ECharts 기반으로 포트폴리오 비중을 시각화합니다.
 */
import { createMemo, Show } from 'solid-js'
import type { JournalPosition } from '../../api/client'
import { EChart, CHART_PALETTE, formatCurrency } from '../ui'
import type { EChartsOption } from 'echarts'

interface PositionDonutChartProps {
  positions: JournalPosition[]
  /** 차트 크기 (기본: 280) */
  size?: number
  /** 제목 표시 여부 */
  showTitle?: boolean
  /** 종목 클릭 시 호출 (상세 모달용) */
  onSymbolClick?: (position: JournalPosition) => void
}

interface ChartDataItem {
  name: string
  value: number
  symbolName: string | null
}

export function PositionDonutChart(props: PositionDonutChartProps) {
  const size = () => props.size || 280

  // 차트 클릭 핸들러
  const handleChartClick = (params: unknown) => {
    if (!props.onSymbolClick) return

    const p = params as { name?: string }
    if (!p.name) return

    // 클릭된 종목 찾기
    const position = props.positions.find((pos) => pos.symbol === p.name)
    if (position) {
      props.onSymbolClick(position)
    }
  }

  // 차트 데이터 계산
  const chartData = createMemo((): ChartDataItem[] => {
    const positions = props.positions.filter((p) => parseFloat(p.market_value || '0') > 0)
    if (positions.length === 0) return []

    return positions.map((position) => ({
      name: position.symbol,
      value: parseFloat(position.market_value || '0'),
      symbolName: position.symbol_name || null,
    }))
  })

  // 총 평가금액
  const totalValue = createMemo(() => {
    return props.positions.reduce((sum, p) => sum + parseFloat(p.market_value || '0'), 0)
  })

  // ECharts 옵션
  const chartOption = createMemo((): EChartsOption => ({
    tooltip: {
      trigger: 'item',
      backgroundColor: 'rgba(30, 30, 40, 0.95)',
      borderColor: '#374151',
      textStyle: {
        color: '#e5e7eb',
      },
      formatter: (params: unknown) => {
        const p = params as { name: string; value: number; percent: number; data: ChartDataItem }
        const symbolName = p.data.symbolName ? ` (${p.data.symbolName})` : ''
        return `
          <div style="font-weight: 600;">${p.name}${symbolName}</div>
          <div style="margin-top: 4px;">
            ${formatCurrency(p.value)}
            <span style="color: #9ca3af; margin-left: 8px;">${p.percent.toFixed(1)}%</span>
          </div>
        `
      },
    },
    legend: {
      type: 'scroll',
      orient: 'vertical',
      right: 10,
      top: 'center',
      textStyle: {
        color: '#9ca3af',
        fontSize: 12,
      },
      pageTextStyle: {
        color: '#9ca3af',
      },
      formatter: (name: string) => {
        const item = chartData().find((d) => d.name === name)
        if (!item) return name
        const percent = ((item.value / totalValue()) * 100).toFixed(1)
        return `${name}  ${percent}%`
      },
    },
    series: [
      {
        name: '포지션 비중',
        type: 'pie',
        radius: ['50%', '75%'],
        center: ['35%', '50%'],
        avoidLabelOverlap: false,
        itemStyle: {
          borderRadius: 4,
          borderColor: '#1f2937',
          borderWidth: 2,
        },
        label: {
          show: false,
        },
        emphasis: {
          label: {
            show: true,
            fontSize: 14,
            fontWeight: 'bold',
            color: '#fff',
          },
          itemStyle: {
            shadowBlur: 10,
            shadowOffsetX: 0,
            shadowColor: 'rgba(0, 0, 0, 0.5)',
          },
        },
        labelLine: {
          show: false,
        },
        data: chartData(),
        color: CHART_PALETTE,
      },
    ],
    // 중앙 텍스트 (graphic 사용)
    graphic: [
      {
        type: 'group',
        left: '35%',
        top: 'center',
        children: [
          {
            type: 'text',
            left: 'center',
            top: -15,
            style: {
              text: '총 평가액',
              fontSize: 12,
              fill: '#9ca3af',
              textAlign: 'center',
            },
          },
          {
            type: 'text',
            left: 'center',
            top: 5,
            style: {
              text: formatCurrency(totalValue()),
              fontSize: 14,
              fontWeight: 'bold',
              fill: '#fff',
              textAlign: 'center',
            },
          },
        ],
      },
    ],
  }))

  return (
    <Show
      when={chartData().length > 0}
      fallback={
        <div
          class="flex items-center justify-center bg-gray-800/50 rounded-xl"
          style={{ width: `${size()}px`, height: `${size()}px` }}
        >
          <span class="text-gray-500 text-sm">보유 종목 없음</span>
        </div>
      }
    >
      <div class="bg-gray-800/30 rounded-xl p-4">
        {props.showTitle !== false && (
          <h4 class="text-sm font-medium text-gray-300 mb-2">포지션 비중</h4>
        )}
        <EChart
          option={chartOption()}
          height={size()}
          onClick={props.onSymbolClick ? handleChartClick : undefined}
          class={props.onSymbolClick ? 'cursor-pointer' : ''}
        />
      </div>
    </Show>
  )
}

export default PositionDonutChart
