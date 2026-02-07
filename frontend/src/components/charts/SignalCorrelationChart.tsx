/**
 * 신호-수익률 상관관계 차트 컴포넌트
 *
 * 신호 강도와 실제 수익률 간의 상관관계를 산점도로 시각화합니다.
 * 신호의 예측력을 평가하는 데 사용됩니다.
 */
import { type Component, createMemo, createSignal, Show } from 'solid-js'
import { EChart, CHART_COLORS } from '../ui/EChart'
import type { EChartsOption, ScatterSeriesOption } from 'echarts'

// ==================== 타입 정의 ====================

export interface SignalDataPoint {
  /** 신호 ID */
  id: string
  /** 종목 코드 */
  symbol: string
  /** 신호 강도 (0~1) */
  strength: number
  /** 실제 수익률 (%) */
  actualReturn: number
  /** 신호 유형 */
  signalType?: string
  /** 매수/매도 방향 */
  side: 'Buy' | 'Sell'
  /** 신호 발생 시간 */
  timestamp: string
}

export interface SignalCorrelationChartProps {
  /** 신호 데이터 포인트 배열 */
  data: SignalDataPoint[]
  /** 차트 높이 */
  height?: number
  /** 제목 */
  title?: string
  /** X축 레이블 */
  xAxisLabel?: string
  /** Y축 레이블 */
  yAxisLabel?: string
  /** 회귀선 표시 여부 */
  showRegression?: boolean
  /** 기준선 표시 여부 (0% 수익률) */
  showZeroLine?: boolean
  /** 데이터 포인트 클릭 핸들러 */
  onPointClick?: (point: SignalDataPoint) => void
  /** 매수/매도 구분 표시 */
  separateBuySell?: boolean
}

// ==================== 유틸리티 ====================

/**
 * 선형 회귀 계산 (최소제곱법)
 */
function calculateRegression(
  points: Array<{ x: number; y: number }>
): { slope: number; intercept: number; r2: number } {
  const n = points.length
  if (n < 2) return { slope: 0, intercept: 0, r2: 0 }

  let sumX = 0, sumY = 0, sumXY = 0, sumXX = 0, sumYY = 0
  for (const p of points) {
    sumX += p.x
    sumY += p.y
    sumXY += p.x * p.y
    sumXX += p.x * p.x
    sumYY += p.y * p.y
  }

  const slope = (n * sumXY - sumX * sumY) / (n * sumXX - sumX * sumX)
  const intercept = (sumY - slope * sumX) / n

  // R² 계산
  const meanY = sumY / n
  let ssTotal = 0, ssResidual = 0
  for (const p of points) {
    const predicted = slope * p.x + intercept
    ssTotal += (p.y - meanY) ** 2
    ssResidual += (p.y - predicted) ** 2
  }
  const r2 = ssTotal > 0 ? 1 - ssResidual / ssTotal : 0

  return { slope, intercept, r2 }
}

/**
 * 상관계수 계산 (Pearson)
 */
function calculateCorrelation(points: Array<{ x: number; y: number }>): number {
  const n = points.length
  if (n < 2) return 0

  let sumX = 0, sumY = 0, sumXY = 0, sumXX = 0, sumYY = 0
  for (const p of points) {
    sumX += p.x
    sumY += p.y
    sumXY += p.x * p.y
    sumXX += p.x * p.x
    sumYY += p.y * p.y
  }

  const numerator = n * sumXY - sumX * sumY
  const denominator = Math.sqrt((n * sumXX - sumX * sumX) * (n * sumYY - sumY * sumY))
  return denominator > 0 ? numerator / denominator : 0
}

// ==================== 컴포넌트 ====================

/**
 * 신호-수익률 상관관계 차트
 */
export const SignalCorrelationChart: Component<SignalCorrelationChartProps> = (props) => {
  const [hoveredPoint, setHoveredPoint] = createSignal<SignalDataPoint | null>(null)

  // 기본값
  const height = () => props.height ?? 400
  const showRegression = () => props.showRegression ?? true
  const showZeroLine = () => props.showZeroLine ?? true
  const separateBuySell = () => props.separateBuySell ?? true

  // 통계 계산
  const stats = createMemo(() => {
    const points = props.data.map((d) => ({ x: d.strength * 100, y: d.actualReturn }))
    const regression = calculateRegression(points)
    const correlation = calculateCorrelation(points)

    const buyPoints = props.data.filter((d) => d.side === 'Buy')
    const sellPoints = props.data.filter((d) => d.side === 'Sell')
    const avgBuyReturn = buyPoints.length > 0
      ? buyPoints.reduce((sum, d) => sum + d.actualReturn, 0) / buyPoints.length
      : 0
    const avgSellReturn = sellPoints.length > 0
      ? sellPoints.reduce((sum, d) => sum + d.actualReturn, 0) / sellPoints.length
      : 0

    return {
      regression,
      correlation,
      totalPoints: points.length,
      avgBuyReturn,
      avgSellReturn,
      buyCount: buyPoints.length,
      sellCount: sellPoints.length,
    }
  })

  // ECharts 옵션
  const chartOption = createMemo((): EChartsOption => {
    const buyData: number[][] = []
    const sellData: number[][] = []
    const allData: number[][] = []

    for (const d of props.data) {
      const point = [d.strength * 100, d.actualReturn]
      allData.push(point)
      if (d.side === 'Buy') {
        buyData.push(point)
      } else {
        sellData.push(point)
      }
    }

    // X축 범위 계산
    const xMin = 0
    const xMax = 100

    // Y축 범위 계산
    const yValues = props.data.map((d) => d.actualReturn)
    const yMin = yValues.length > 0 ? Math.min(...yValues, -10) : -10
    const yMax = yValues.length > 0 ? Math.max(...yValues, 10) : 10
    const yPadding = (yMax - yMin) * 0.1

    // 시리즈 구성
    const series: ScatterSeriesOption[] = []

    if (separateBuySell()) {
      // 매수/매도 분리 표시
      series.push({
        name: '매수 신호',
        type: 'scatter',
        data: buyData,
        symbolSize: 10,
        itemStyle: {
          color: CHART_COLORS.green,
          opacity: 0.7,
        },
        emphasis: {
          itemStyle: {
            opacity: 1,
            shadowBlur: 10,
            shadowColor: 'rgba(34, 197, 94, 0.5)',
          },
        },
      })
      series.push({
        name: '매도 신호',
        type: 'scatter',
        data: sellData,
        symbolSize: 10,
        itemStyle: {
          color: CHART_COLORS.red,
          opacity: 0.7,
        },
        emphasis: {
          itemStyle: {
            opacity: 1,
            shadowBlur: 10,
            shadowColor: 'rgba(239, 68, 68, 0.5)',
          },
        },
      })
    } else {
      // 전체 통합 표시
      series.push({
        name: '신호',
        type: 'scatter',
        data: allData,
        symbolSize: (data: number[]) => {
          // 수익률 크기에 따라 마커 크기 조정
          return Math.min(Math.max(Math.abs(data[1]) / 2 + 5, 5), 20)
        },
        itemStyle: {
          color: (params: { data: number[] }) => {
            return params.data[1] >= 0 ? CHART_COLORS.green : CHART_COLORS.red
          },
          opacity: 0.7,
        },
      })
    }

    // 회귀선 추가
    if (showRegression() && stats().totalPoints >= 2) {
      const { slope, intercept } = stats().regression
      const lineData = [
        [xMin, slope * xMin + intercept],
        [xMax, slope * xMax + intercept],
      ]
      series.push({
        name: '회귀선',
        type: 'line',
        data: lineData,
        symbol: 'none',
        lineStyle: {
          color: CHART_COLORS.purple,
          width: 2,
          type: 'dashed',
        },
      } as unknown as ScatterSeriesOption)
    }

    return {
      title: props.title ? {
        text: props.title,
        left: 'center',
        textStyle: {
          fontSize: 14,
          fontWeight: 'bold',
        },
      } : undefined,
      tooltip: {
        trigger: 'item',
        formatter: (params: { data: number[]; seriesName: string }) => {
          if (!params.data || params.data.length < 2) return ''
          const [strength, returnPct] = params.data
          return `
            <div class="text-sm">
              <div class="font-semibold">${params.seriesName}</div>
              <div>신호 강도: ${strength.toFixed(1)}%</div>
              <div>실제 수익률: ${returnPct >= 0 ? '+' : ''}${returnPct.toFixed(2)}%</div>
            </div>
          `
        },
      },
      legend: separateBuySell() ? {
        data: ['매수 신호', '매도 신호', '회귀선'],
        bottom: 0,
      } : undefined,
      grid: {
        left: 60,
        right: 30,
        top: props.title ? 50 : 30,
        bottom: separateBuySell() ? 60 : 40,
      },
      xAxis: {
        type: 'value',
        name: props.xAxisLabel ?? '신호 강도 (%)',
        nameLocation: 'middle',
        nameGap: 30,
        min: xMin,
        max: xMax,
        splitLine: {
          lineStyle: {
            type: 'dashed',
            opacity: 0.3,
          },
        },
      },
      yAxis: {
        type: 'value',
        name: props.yAxisLabel ?? '실제 수익률 (%)',
        nameLocation: 'middle',
        nameGap: 45,
        min: yMin - yPadding,
        max: yMax + yPadding,
        splitLine: {
          lineStyle: {
            type: 'dashed',
            opacity: 0.3,
          },
        },
        axisLabel: {
          formatter: (value: number) => `${value >= 0 ? '+' : ''}${value}%`,
        },
      },
      series,
      // 0% 기준선
      ...(showZeroLine() ? {
        markLine: {
          silent: true,
          data: [{ yAxis: 0 }],
          lineStyle: {
            color: '#6b7280',
            type: 'solid',
            width: 1,
          },
          label: {
            show: false,
          },
        },
      } : {}),
    }
  })

  // 상관관계 강도 해석
  const correlationInterpretation = createMemo(() => {
    const r = Math.abs(stats().correlation)
    if (r >= 0.7) return { text: '강한 상관', color: 'text-green-500' }
    if (r >= 0.4) return { text: '중간 상관', color: 'text-yellow-500' }
    if (r >= 0.2) return { text: '약한 상관', color: 'text-orange-500' }
    return { text: '거의 없음', color: 'text-gray-500' }
  })

  return (
    <div class="space-y-4">
      {/* 통계 요약 */}
      <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
        <div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-3 text-center">
          <div class="text-xs text-gray-500 dark:text-gray-400">상관계수 (r)</div>
          <div class={`text-lg font-bold ${correlationInterpretation().color}`}>
            {stats().correlation.toFixed(3)}
          </div>
          <div class="text-xs text-gray-400">{correlationInterpretation().text}</div>
        </div>

        <div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-3 text-center">
          <div class="text-xs text-gray-500 dark:text-gray-400">결정계수 (R²)</div>
          <div class="text-lg font-bold text-blue-500">
            {(stats().regression.r2 * 100).toFixed(1)}%
          </div>
          <div class="text-xs text-gray-400">설명력</div>
        </div>

        <div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-3 text-center">
          <div class="text-xs text-gray-500 dark:text-gray-400">매수 평균 수익</div>
          <div class={`text-lg font-bold ${stats().avgBuyReturn >= 0 ? 'text-green-500' : 'text-red-500'}`}>
            {stats().avgBuyReturn >= 0 ? '+' : ''}{stats().avgBuyReturn.toFixed(2)}%
          </div>
          <div class="text-xs text-gray-400">{stats().buyCount}건</div>
        </div>

        <div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-3 text-center">
          <div class="text-xs text-gray-500 dark:text-gray-400">매도 평균 수익</div>
          <div class={`text-lg font-bold ${stats().avgSellReturn >= 0 ? 'text-green-500' : 'text-red-500'}`}>
            {stats().avgSellReturn >= 0 ? '+' : ''}{stats().avgSellReturn.toFixed(2)}%
          </div>
          <div class="text-xs text-gray-400">{stats().sellCount}건</div>
        </div>
      </div>

      {/* 산점도 차트 */}
      <Show
        when={props.data.length > 0}
        fallback={
          <div class="flex items-center justify-center h-64 bg-gray-50 dark:bg-gray-800 rounded-lg">
            <p class="text-gray-500 dark:text-gray-400">표시할 데이터가 없습니다</p>
          </div>
        }
      >
        <EChart option={chartOption()} height={height()} />
      </Show>

      {/* 해석 가이드 */}
      <div class="text-xs text-gray-500 dark:text-gray-400 bg-gray-50 dark:bg-gray-800 rounded-lg p-3">
        <div class="font-medium mb-1">📊 해석 가이드</div>
        <ul class="space-y-0.5">
          <li>• <strong>양의 상관</strong>: 신호 강도가 높을수록 수익률도 높음 (신호 신뢰도 높음)</li>
          <li>• <strong>음의 상관</strong>: 신호 강도가 높을수록 수익률이 낮음 (역지표로 활용 가능)</li>
          <li>• <strong>무상관</strong>: 신호 강도와 수익률 간 관계 없음 (신호 개선 필요)</li>
        </ul>
      </div>
    </div>
  )
}

export default SignalCorrelationChart
