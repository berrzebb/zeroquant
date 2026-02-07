/**
 * 칸반 보드 컴포넌트
 *
 * ATTACK / ARMED / WATCH 3열 레이아웃으로 종목 상태를 관리합니다.
 * 드래그 앤 드롭으로 상태 변경이 가능합니다.
 *
 * @example
 * ```tsx
 * <KanbanBoard
 *   symbols={[
 *     { symbol: 'AAPL', name: 'Apple', routeState: 'ATTACK', score: 85 },
 *     { symbol: 'MSFT', name: 'Microsoft', routeState: 'ARMED', score: 72 },
 *   ]}
 *   onStateChange={(symbol, newState) => console.log(symbol, newState)}
 * />
 * ```
 */
import { createMemo, createSignal, For, Show } from 'solid-js'
import type { Component, JSX } from 'solid-js'

/** RouteState 타입 */
export type RouteState = 'ATTACK' | 'ARMED' | 'WATCH'

/** 종목 카드 데이터 */
export interface KanbanSymbol {
  /** 심볼 */
  symbol: string
  /** 종목명 */
  name: string
  /** 현재 상태 */
  routeState: RouteState
  /** 점수 */
  score: number
  /** 현재가 */
  price?: number
  /** 등락률 */
  changeRate?: number
  /** 스파크라인 데이터 (최근 가격) */
  sparkline?: number[]
}

export interface KanbanBoardProps {
  /** 종목 데이터 배열 */
  symbols: KanbanSymbol[]
  /** 상태 변경 핸들러 */
  onStateChange?: (symbol: string, newState: RouteState) => void
  /** 카드 클릭 핸들러 */
  onCardClick?: (symbol: string) => void
  /** 드래그 앤 드롭 활성화 */
  enableDragDrop?: boolean
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

/** 열 설정 */
const COLUMNS: { state: RouteState; label: string; icon: string; color: string; bgColor: string }[] = [
  {
    state: 'ATTACK',
    label: '진입 신호',
    icon: '🎯',
    color: '#22c55e',
    bgColor: 'rgba(34, 197, 94, 0.1)',
  },
  {
    state: 'ARMED',
    label: '대기 중',
    icon: '⚡',
    color: '#3b82f6',
    bgColor: 'rgba(59, 130, 246, 0.1)',
  },
  {
    state: 'WATCH',
    label: '관망',
    icon: '👀',
    color: '#f59e0b',
    bgColor: 'rgba(245, 158, 11, 0.1)',
  },
]

/**
 * 미니 스파크라인 SVG
 */
const Sparkline: Component<{ data: number[]; color: string; width?: number; height?: number }> = (props) => {
  const width = () => props.width ?? 60
  const height = () => props.height ?? 20

  const path = createMemo(() => {
    const data = props.data
    if (data.length < 2) return ''

    const min = Math.min(...data)
    const max = Math.max(...data)
    const range = max - min || 1

    const xStep = width() / (data.length - 1)
    const points = data.map((value, i) => {
      const x = i * xStep
      const y = height() - ((value - min) / range) * height()
      return `${x},${y}`
    })

    return `M ${points.join(' L ')}`
  })

  return (
    <svg width={width()} height={height()} class="overflow-visible">
      <path
        d={path()}
        fill="none"
        stroke={props.color}
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
  )
}

/**
 * 종목 카드 컴포넌트
 */
const SymbolCard: Component<{
  symbol: KanbanSymbol
  columnColor: string
  onClick?: () => void
  draggable?: boolean
  onDragStart?: (e: DragEvent) => void
  onDragEnd?: (e: DragEvent) => void
}> = (props) => {
  const changeColor = createMemo(() => {
    const rate = props.symbol.changeRate
    if (rate === undefined) return 'text-gray-400'
    return rate >= 0 ? 'text-green-400' : 'text-red-400'
  })

  const sparklineColor = createMemo(() => {
    const data = props.symbol.sparkline
    if (!data || data.length < 2) return '#6b7280'
    return data[data.length - 1] >= data[0] ? '#22c55e' : '#ef4444'
  })

  return (
    <div
      class={`
        p-3 rounded-lg border border-gray-700 bg-gray-800/80
        hover:bg-gray-700/80 hover:border-gray-600
        transition-all cursor-pointer
        ${props.draggable ? 'cursor-grab active:cursor-grabbing' : ''}
      `}
      onClick={props.onClick}
      draggable={props.draggable}
      onDragStart={props.onDragStart}
      onDragEnd={props.onDragEnd}
    >
      {/* 헤더: 심볼 + 점수 */}
      <div class="flex items-center justify-between mb-2">
        <div class="flex items-center gap-2">
          <span class="font-semibold text-white">{props.symbol.symbol}</span>
          <span class="text-xs text-gray-400 truncate max-w-[80px]">{props.symbol.name}</span>
        </div>
        <div
          class="px-2 py-0.5 rounded text-xs font-bold"
          style={{
            'background-color': `${props.columnColor}20`,
            color: props.columnColor,
          }}
        >
          {props.symbol.score}
        </div>
      </div>

      {/* 가격 정보 */}
      <Show when={props.symbol.price !== undefined}>
        <div class="flex items-center justify-between text-sm">
          <span class="text-gray-300 font-mono">
            {props.symbol.price?.toLocaleString()}
          </span>
          <span class={`font-mono ${changeColor()}`}>
            {props.symbol.changeRate !== undefined && (
              <>
                {props.symbol.changeRate >= 0 ? '+' : ''}
                {props.symbol.changeRate.toFixed(2)}%
              </>
            )}
          </span>
        </div>
      </Show>

      {/* 스파크라인 */}
      <Show when={props.symbol.sparkline && props.symbol.sparkline.length > 1}>
        <div class="mt-2 flex justify-center">
          <Sparkline
            data={props.symbol.sparkline!}
            color={sparklineColor()}
            width={80}
            height={24}
          />
        </div>
      </Show>
    </div>
  )
}

/**
 * KanbanBoard 컴포넌트
 */
export const KanbanBoard: Component<KanbanBoardProps> = (props) => {
  const enableDragDrop = () => props.enableDragDrop ?? true

  const [draggedSymbol, setDraggedSymbol] = createSignal<string | null>(null)
  const [dragOverColumn, setDragOverColumn] = createSignal<RouteState | null>(null)

  // 열별 종목 그룹화 (점수 순 정렬)
  const groupedSymbols = createMemo(() => {
    const groups: Record<RouteState, KanbanSymbol[]> = {
      ATTACK: [],
      ARMED: [],
      WATCH: [],
    }

    for (const sym of props.symbols) {
      if (groups[sym.routeState]) {
        groups[sym.routeState].push(sym)
      }
    }

    // 점수 순 정렬
    for (const state of Object.keys(groups) as RouteState[]) {
      groups[state].sort((a, b) => b.score - a.score)
    }

    return groups
  })

  // 드래그 핸들러
  const handleDragStart = (symbol: string) => (e: DragEvent) => {
    setDraggedSymbol(symbol)
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = 'move'
      e.dataTransfer.setData('text/plain', symbol)
    }
  }

  const handleDragEnd = () => {
    setDraggedSymbol(null)
    setDragOverColumn(null)
  }

  const handleDragOver = (state: RouteState) => (e: DragEvent) => {
    e.preventDefault()
    setDragOverColumn(state)
  }

  const handleDragLeave = () => {
    setDragOverColumn(null)
  }

  const handleDrop = (newState: RouteState) => (e: DragEvent) => {
    e.preventDefault()
    const symbol = draggedSymbol()
    if (symbol && props.onStateChange) {
      props.onStateChange(symbol, newState)
    }
    setDraggedSymbol(null)
    setDragOverColumn(null)
  }

  return (
    <div
      class={`grid grid-cols-3 gap-4 ${props.class || ''}`}
      style={props.style}
    >
      <For each={COLUMNS}>
        {(column) => {
          const items = () => groupedSymbols()[column.state]
          const isDragOver = () => dragOverColumn() === column.state

          return (
            <div
              class={`
                flex flex-col rounded-xl border transition-all
                ${isDragOver() ? 'border-blue-500 ring-2 ring-blue-500/30' : 'border-gray-700'}
              `}
              style={{ 'background-color': column.bgColor }}
              onDragOver={enableDragDrop() ? handleDragOver(column.state) : undefined}
              onDragLeave={enableDragDrop() ? handleDragLeave : undefined}
              onDrop={enableDragDrop() ? handleDrop(column.state) : undefined}
            >
              {/* 열 헤더 */}
              <div
                class="flex items-center justify-between p-3 border-b border-gray-700"
              >
                <div class="flex items-center gap-2">
                  <span class="text-lg">{column.icon}</span>
                  <span
                    class="font-semibold"
                    style={{ color: column.color }}
                  >
                    {column.label}
                  </span>
                </div>
                <span
                  class="px-2 py-0.5 rounded-full text-xs font-bold"
                  style={{
                    'background-color': column.color,
                    color: '#1f2937',
                  }}
                >
                  {items().length}
                </span>
              </div>

              {/* 카드 목록 */}
              <div class="flex-1 p-3 space-y-2 overflow-y-auto max-h-[500px]">
                <Show
                  when={items().length > 0}
                  fallback={
                    <div class="flex items-center justify-center h-24 text-gray-500 text-sm">
                      종목 없음
                    </div>
                  }
                >
                  <For each={items()}>
                    {(symbol) => (
                      <SymbolCard
                        symbol={symbol}
                        columnColor={column.color}
                        onClick={() => props.onCardClick?.(symbol.symbol)}
                        draggable={enableDragDrop()}
                        onDragStart={handleDragStart(symbol.symbol)}
                        onDragEnd={handleDragEnd}
                      />
                    )}
                  </For>
                </Show>
              </div>
            </div>
          )
        }}
      </For>
    </div>
  )
}

export default KanbanBoard
