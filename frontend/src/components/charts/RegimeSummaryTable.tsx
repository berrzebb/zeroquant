/**
 * 레짐 요약 테이블 컴포넌트
 *
 * 시장 레짐(Bull/Bear/Sideways)별 성과를 테이블 형태로 표시합니다.
 * 각 레짐의 기간, 평균 수익률, 변동성, 최대 DD를 한눈에 비교할 수 있습니다.
 *
 * @example
 * ```tsx
 * <RegimeSummaryTable
 *   regimes={[
 *     { regime: 'Bull', days: 45, avgReturn: 2.3, volatility: 15, maxDrawdown: -8 },
 *     { regime: 'Bear', days: 30, avgReturn: -1.5, volatility: 22, maxDrawdown: -15 },
 *   ]}
 *   currentRegime="Bull"
 * />
 * ```
 */
import { createMemo, For, Show } from 'solid-js'
import type { Component, JSX } from 'solid-js'
import { EChart } from '../ui/EChart'
import { formatPercent } from '../../utils/format'
import type { EChartsOption } from 'echarts'

/** 레짐 타입 */
export type MarketRegime = 'Bull' | 'Bear' | 'Sideways'

/** 레짐 데이터 */
export interface RegimeData {
  /** 레짐 타입 */
  regime: MarketRegime
  /** 레짐 기간 (일수) */
  days: number
  /** 평균 수익률 (%) */
  avgReturn: number
  /** 변동성 (%) */
  volatility: number
  /** 최대 손실폭 (%) */
  maxDrawdown: number
  /** 시작일 (선택) */
  startDate?: string
  /** 종료일 (선택) */
  endDate?: string
}

/** 레짐 전환 이력 */
export interface RegimeTransition {
  /** 날짜 */
  date: string
  /** 이전 레짐 */
  fromRegime: MarketRegime
  /** 새 레짐 */
  toRegime: MarketRegime
}

export interface RegimeSummaryTableProps {
  /** 레짐 데이터 배열 */
  regimes: RegimeData[]
  /** 현재 레짐 */
  currentRegime?: MarketRegime
  /** 레짐 전환 히스토리 */
  transitions?: RegimeTransition[]
  /** 전환 히스토리 차트 표시 */
  showTransitionChart?: boolean
  /** 제목 */
  title?: string
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

/** 레짐별 설정 */
const REGIME_CONFIG: Record<MarketRegime, { icon: string; color: string; bgColor: string; label: string }> = {
  Bull: {
    icon: '🐂',
    color: '#22c55e',
    bgColor: 'rgba(34, 197, 94, 0.1)',
    label: '상승장',
  },
  Bear: {
    icon: '🐻',
    color: '#ef4444',
    bgColor: 'rgba(239, 68, 68, 0.1)',
    label: '하락장',
  },
  Sideways: {
    icon: '↔️',
    color: '#f59e0b',
    bgColor: 'rgba(245, 158, 11, 0.1)',
    label: '횡보장',
  },
}

/**
 * RegimeSummaryTable 컴포넌트
 */
export const RegimeSummaryTable: Component<RegimeSummaryTableProps> = (props) => {
  const showTransitionChart = () => props.showTransitionChart ?? true

  // 현재 레짐 찾기
  const currentRegimeData = createMemo(() =>
    props.regimes.find((r) => r.regime === props.currentRegime)
  )

  // 총 기간
  const totalDays = createMemo(() =>
    props.regimes.reduce((sum, r) => sum + r.days, 0)
  )

  // 레짐 비율 계산
  const regimeRatios = createMemo(() =>
    props.regimes.map((r) => ({
      ...r,
      ratio: (r.days / totalDays()) * 100,
    }))
  )

  // 전환 히스토리 차트 옵션
  const transitionChartOption = createMemo((): EChartsOption | null => {
    if (!props.transitions || props.transitions.length === 0) return null

    const dates = props.transitions.map((t) => t.date)
    const regimeValues = props.transitions.map((t) => {
      switch (t.toRegime) {
        case 'Bull': return 2
        case 'Sideways': return 1
        case 'Bear': return 0
      }
    })

    return {
      tooltip: {
        trigger: 'axis',
        backgroundColor: 'rgba(30, 30, 40, 0.95)',
        borderColor: '#374151',
        textStyle: { color: '#e5e7eb' },
        formatter: (params: unknown) => {
          const p = (params as { data: number; name: string }[])[0]
          const regimes: MarketRegime[] = ['Bear', 'Sideways', 'Bull']
          const regime = regimes[p.data]
          const config = REGIME_CONFIG[regime]
          return `
            <div>${p.name}</div>
            <div style="margin-top: 4px; color: ${config.color};">
              ${config.icon} ${config.label}
            </div>
          `
        },
      },
      grid: {
        top: 10,
        right: 10,
        bottom: 30,
        left: 50,
      },
      xAxis: {
        type: 'category',
        data: dates,
        axisLine: { lineStyle: { color: '#4b5563' } },
        axisLabel: { color: '#9ca3af', fontSize: 10 },
      },
      yAxis: {
        type: 'value',
        min: 0,
        max: 2,
        interval: 1,
        axisLine: { lineStyle: { color: '#4b5563' } },
        axisLabel: {
          color: '#9ca3af',
          formatter: (value: number) => {
            const labels = ['Bear', 'Sideways', 'Bull']
            return labels[value] || ''
          },
        },
        splitLine: { lineStyle: { color: '#374151', type: 'dashed' } },
      },
      series: [
        {
          type: 'line',
          data: regimeValues,
          step: 'end',
          lineStyle: { color: '#60a5fa', width: 2 },
          itemStyle: { color: '#60a5fa' },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: 'rgba(96, 165, 250, 0.3)' },
                { offset: 1, color: 'rgba(96, 165, 250, 0.05)' },
              ],
            },
          },
        },
      ],
    }
  })

  return (
    <div class={`bg-gray-800/50 rounded-xl overflow-hidden ${props.class || ''}`} style={props.style}>
      {/* 헤더 */}
      <Show when={props.title || props.currentRegime}>
        <div class="flex items-center justify-between p-4 border-b border-gray-700">
          <h3 class="text-sm font-semibold text-white">
            {props.title || '레짐별 성과 요약'}
          </h3>
          <Show when={props.currentRegime}>
            <div
              class="flex items-center gap-2 px-3 py-1 rounded-full text-sm font-medium"
              style={{
                'background-color': REGIME_CONFIG[props.currentRegime!].bgColor,
                color: REGIME_CONFIG[props.currentRegime!].color,
              }}
            >
              <span>{REGIME_CONFIG[props.currentRegime!].icon}</span>
              <span>현재: {REGIME_CONFIG[props.currentRegime!].label}</span>
            </div>
          </Show>
        </div>
      </Show>

      {/* 테이블 */}
      <div class="overflow-x-auto">
        <table class="w-full text-sm">
          <thead>
            <tr class="border-b border-gray-700">
              <th class="text-left p-3 text-gray-400 font-medium">레짐</th>
              <th class="text-right p-3 text-gray-400 font-medium">기간</th>
              <th class="text-right p-3 text-gray-400 font-medium">비중</th>
              <th class="text-right p-3 text-gray-400 font-medium">평균 수익률</th>
              <th class="text-right p-3 text-gray-400 font-medium">변동성</th>
              <th class="text-right p-3 text-gray-400 font-medium">최대 DD</th>
            </tr>
          </thead>
          <tbody>
            <For each={regimeRatios()}>
              {(regime) => {
                const config = REGIME_CONFIG[regime.regime]
                const isCurrent = regime.regime === props.currentRegime

                return (
                  <tr
                    class={`
                      border-b border-gray-700/50 transition-colors
                      ${isCurrent ? 'bg-blue-500/10' : 'hover:bg-gray-700/30'}
                    `}
                  >
                    {/* 레짐 */}
                    <td class="p-3">
                      <div class="flex items-center gap-2">
                        <span class="text-lg">{config.icon}</span>
                        <span style={{ color: config.color }} class="font-medium">
                          {config.label}
                        </span>
                        <Show when={isCurrent}>
                          <span class="px-1.5 py-0.5 rounded text-xs bg-blue-500 text-white">
                            현재
                          </span>
                        </Show>
                      </div>
                    </td>

                    {/* 기간 */}
                    <td class="p-3 text-right text-white font-mono">
                      {regime.days}일
                    </td>

                    {/* 비중 */}
                    <td class="p-3 text-right">
                      <div class="flex items-center justify-end gap-2">
                        <div class="w-16 h-2 bg-gray-700 rounded-full overflow-hidden">
                          <div
                            class="h-full rounded-full"
                            style={{
                              width: `${regime.ratio}%`,
                              'background-color': config.color,
                            }}
                          />
                        </div>
                        <span class="text-gray-400 font-mono text-xs w-10">
                          {regime.ratio.toFixed(0)}%
                        </span>
                      </div>
                    </td>

                    {/* 평균 수익률 */}
                    <td class="p-3 text-right font-mono font-medium">
                      <span class={regime.avgReturn >= 0 ? 'text-green-400' : 'text-red-400'}>
                        {formatPercent(regime.avgReturn)}
                      </span>
                    </td>

                    {/* 변동성 */}
                    <td class="p-3 text-right font-mono text-gray-300">
                      {regime.volatility.toFixed(1)}%
                    </td>

                    {/* 최대 DD */}
                    <td class="p-3 text-right font-mono text-red-400">
                      {formatPercent(regime.maxDrawdown, false)}
                    </td>
                  </tr>
                )
              }}
            </For>
          </tbody>
        </table>
      </div>

      {/* 전환 히스토리 차트 */}
      <Show when={showTransitionChart() && transitionChartOption()}>
        <div class="p-4 border-t border-gray-700">
          <h4 class="text-xs text-gray-400 mb-2">레짐 전환 히스토리</h4>
          <EChart option={transitionChartOption()!} height={120} />
        </div>
      </Show>
    </div>
  )
}

export default RegimeSummaryTable
