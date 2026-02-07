/**
 * 기회 맵 컴포넌트
 *
 * TOTAL Score vs TRIGGER Score 산점도로 종목 기회를 시각화합니다.
 * RouteState 기반 색상으로 진입 타이밍을 한눈에 파악할 수 있습니다.
 *
 * @example
 * ```tsx
 * <OpportunityMap
 *   symbols={[
 *     { symbol: 'AAPL', totalScore: 75, triggerScore: 80, routeState: 'ATTACK' },
 *     { symbol: 'MSFT', totalScore: 60, triggerScore: 45, routeState: 'ARMED' },
 *   ]}
 *   onSymbolClick={(symbol) => console.log(symbol)}
 * />
 * ```
 */
import { createMemo, createSignal, Show } from 'solid-js'
import type { Component, JSX } from 'solid-js'
import { EChart } from '../ui/EChart'
import type { EChartsOption } from 'echarts'

/** RouteState 타입 */
export type RouteState = 'ATTACK' | 'ARMED' | 'WATCH' | 'AVOID' | 'UNKNOWN'

/** 종목 데이터 */
export interface OpportunitySymbol {
  /** 심볼 */
  symbol: string
  /** TOTAL 점수 (X축) */
  totalScore: number
  /** TRIGGER 점수 (Y축) */
  triggerScore: number
  /** 상태 */
  routeState: RouteState
  /** 종목명 (선택) */
  name?: string
  /** 시가총액 또는 거래량 (점 크기) */
  size?: number
}

export interface OpportunityMapProps {
  /** 종목 데이터 배열 */
  symbols: OpportunitySymbol[]
  /** 클릭 핸들러 */
  onSymbolClick?: (symbol: string) => void
  /** 차트 높이 */
  height?: number
  /** 4분면 라벨 표시 */
  showQuadrantLabels?: boolean
  /** 점수 기준선 (기본: 50) */
  threshold?: number
  /** 제목 */
  title?: string
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

/** RouteState별 색상 */
const ROUTE_STATE_COLORS: Record<RouteState, string> = {
  ATTACK: '#22c55e',   // 초록 - 공격적 매수
  ARMED: '#3b82f6',    // 파랑 - 대기
  WATCH: '#f59e0b',    // 노랑 - 관망
  AVOID: '#ef4444',    // 빨강 - 회피
  UNKNOWN: '#6b7280',  // 회색 - 미정
}

/** RouteState별 라벨 */
const ROUTE_STATE_LABELS: Record<RouteState, string> = {
  ATTACK: '진입 신호',
  ARMED: '대기 중',
  WATCH: '관망',
  AVOID: '회피',
  UNKNOWN: '분석 중',
}

/** 4분면 라벨 */
const QUADRANT_LABELS = {
  topRight: { label: '🎯 최적 기회', description: '높은 TOTAL + 높은 TRIGGER' },
  topLeft: { label: '⚡ 타이밍 대기', description: '낮은 TOTAL + 높은 TRIGGER' },
  bottomRight: { label: '📊 펀더멘탈 강점', description: '높은 TOTAL + 낮은 TRIGGER' },
  bottomLeft: { label: '⏳ 관망 구간', description: '낮은 TOTAL + 낮은 TRIGGER' },
}

/**
 * OpportunityMap 컴포넌트
 */
export const OpportunityMap: Component<OpportunityMapProps> = (props) => {
  const height = () => props.height ?? 400
  const threshold = () => props.threshold ?? 50
  const showQuadrantLabels = () => props.showQuadrantLabels ?? true

  const [selectedSymbol, setSelectedSymbol] = createSignal<string | null>(null)

  // 시리즈 데이터 (RouteState별 그룹화)
  const seriesData = createMemo(() => {
    const groups: Record<RouteState, OpportunitySymbol[]> = {
      ATTACK: [],
      ARMED: [],
      WATCH: [],
      AVOID: [],
      UNKNOWN: [],
    }

    for (const sym of props.symbols) {
      const state = sym.routeState || 'UNKNOWN'
      groups[state].push(sym)
    }

    return groups
  })

  // 점 크기 정규화
  const normalizeSize = createMemo(() => {
    const sizes = props.symbols.filter(s => s.size).map(s => s.size!)
    if (sizes.length === 0) return () => 15

    const min = Math.min(...sizes)
    const max = Math.max(...sizes)
    const range = max - min || 1

    return (size: number | undefined) => {
      if (!size) return 15
      // 10 ~ 30 범위로 정규화
      return 10 + ((size - min) / range) * 20
    }
  })

  // ECharts 옵션
  const chartOption = createMemo((): EChartsOption => {
    const t = threshold()
    const groups = seriesData()
    const sizeNormalizer = normalizeSize()

    // 시리즈 생성 (각 RouteState별)
    const series = Object.entries(groups)
      .filter(([, items]) => items.length > 0)
      .map(([state, items]) => ({
        name: ROUTE_STATE_LABELS[state as RouteState],
        type: 'scatter' as const,
        data: items.map((item) => ({
          value: [item.totalScore, item.triggerScore],
          symbolSize: sizeNormalizer(item.size),
          itemStyle: {
            color: ROUTE_STATE_COLORS[state as RouteState],
            opacity: selectedSymbol() && selectedSymbol() !== item.symbol ? 0.3 : 0.8,
          },
          name: item.symbol,
          meta: item,
        })),
        emphasis: {
          itemStyle: {
            borderColor: '#fff',
            borderWidth: 2,
          },
        },
      }))

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
          const p = params as { data: { meta: OpportunitySymbol } }
          const item = p.data.meta

          return `
            <div style="font-weight: 600; font-size: 14px;">${item.symbol}</div>
            ${item.name ? `<div style="color: #9ca3af; font-size: 12px;">${item.name}</div>` : ''}
            <div style="margin-top: 8px; display: grid; grid-template-columns: auto 1fr; gap: 4px 12px;">
              <span style="color: #9ca3af;">TOTAL:</span>
              <span style="text-align: right; font-weight: 600;">${item.totalScore.toFixed(1)}</span>
              <span style="color: #9ca3af;">TRIGGER:</span>
              <span style="text-align: right; font-weight: 600;">${item.triggerScore.toFixed(1)}</span>
              <span style="color: #9ca3af;">상태:</span>
              <span style="text-align: right; color: ${ROUTE_STATE_COLORS[item.routeState]};">
                ${ROUTE_STATE_LABELS[item.routeState]}
              </span>
            </div>
          `
        },
      },
      legend: {
        data: Object.entries(ROUTE_STATE_LABELS)
          .filter(([state]) => groups[state as RouteState].length > 0)
          .map(([, label]) => label),
        top: 30,
        textStyle: { color: '#9ca3af' },
      },
      grid: {
        top: 80,
        right: 40,
        bottom: 60,
        left: 60,
      },
      xAxis: {
        type: 'value',
        name: 'TOTAL Score',
        nameLocation: 'middle',
        nameGap: 35,
        nameTextStyle: { color: '#9ca3af', fontSize: 12 },
        min: 0,
        max: 100,
        axisLine: { lineStyle: { color: '#4b5563' } },
        axisLabel: { color: '#9ca3af' },
        splitLine: { lineStyle: { color: '#374151', type: 'dashed' } },
      },
      yAxis: {
        type: 'value',
        name: 'TRIGGER Score',
        nameLocation: 'middle',
        nameGap: 45,
        nameTextStyle: { color: '#9ca3af', fontSize: 12 },
        min: 0,
        max: 100,
        axisLine: { lineStyle: { color: '#4b5563' } },
        axisLabel: { color: '#9ca3af' },
        splitLine: { lineStyle: { color: '#374151', type: 'dashed' } },
      },
      // 기준선
      markLine: {
        silent: true,
        lineStyle: { color: '#6b7280', type: 'dashed' },
        data: [
          { xAxis: t },
          { yAxis: t },
        ],
      } as unknown as undefined, // 타입 호환성
      // 4분면 라벨
      graphic: showQuadrantLabels()
        ? [
            // 우상단
            {
              type: 'text',
              right: 50,
              top: 90,
              style: {
                text: QUADRANT_LABELS.topRight.label,
                fill: '#22c55e',
                fontSize: 12,
                fontWeight: 'bold',
              },
            },
            // 좌상단
            {
              type: 'text',
              left: 70,
              top: 90,
              style: {
                text: QUADRANT_LABELS.topLeft.label,
                fill: '#f59e0b',
                fontSize: 12,
                fontWeight: 'bold',
              },
            },
            // 우하단
            {
              type: 'text',
              right: 50,
              bottom: 70,
              style: {
                text: QUADRANT_LABELS.bottomRight.label,
                fill: '#3b82f6',
                fontSize: 12,
                fontWeight: 'bold',
              },
            },
            // 좌하단
            {
              type: 'text',
              left: 70,
              bottom: 70,
              style: {
                text: QUADRANT_LABELS.bottomLeft.label,
                fill: '#6b7280',
                fontSize: 12,
                fontWeight: 'bold',
              },
            },
          ]
        : undefined,
      series,
    }
  })

  // 클릭 핸들러
  const handleClick = (params: unknown) => {
    const p = params as { data?: { meta?: OpportunitySymbol } }
    if (p.data?.meta && props.onSymbolClick) {
      setSelectedSymbol(p.data.meta.symbol)
      props.onSymbolClick(p.data.meta.symbol)
    }
  }

  return (
    <Show
      when={props.symbols.length > 0}
      fallback={
        <div
          class={`flex items-center justify-center bg-gray-800/50 rounded-xl ${props.class || ''}`}
          style={{ height: `${height()}px`, ...props.style }}
        >
          <div class="text-gray-500 text-sm">종목 데이터 없음</div>
        </div>
      }
    >
      <EChart
        option={chartOption()}
        height={height()}
        class={props.class}
        style={props.style}
        onClick={handleClick}
      />
    </Show>
  )
}

export default OpportunityMap
