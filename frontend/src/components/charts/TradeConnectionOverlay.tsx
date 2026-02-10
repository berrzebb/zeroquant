/**
 * 거래 연결 오버레이 컴포넌트
 *
 * 차트 위에 거래의 진입/청산 포인트를 연결선으로 표시하고,
 * 손익 구간을 배경색으로 시각화합니다.
 */
import { type Component, For, Show, createMemo, createSignal } from 'solid-js'
import { formatPercent, formatQuantity } from '../../utils/format'

// ==================== 타입 정의 ====================

export interface TradeConnection {
  /** 거래 ID */
  id: string
  /** 종목 코드 */
  symbol: string
  /** 진입 시간 (ISO 8601) */
  entryTime: string
  /** 청산 시간 (ISO 8601) */
  exitTime: string
  /** 진입 가격 */
  entryPrice: number
  /** 청산 가격 */
  exitPrice: number
  /** 매수/매도 방향 */
  side: 'Buy' | 'Sell'
  /** 손익 금액 */
  pnl: number
  /** 손익률 (%) */
  returnPct: number
}

export interface TradeConnectionOverlayProps {
  /** 거래 연결 데이터 */
  trades: TradeConnection[]
  /** 컨테이너 너비 (px) */
  width: number
  /** 컨테이너 높이 (px) */
  height: number
  /** 가격 범위 (min, max) */
  priceRange: { min: number; max: number }
  /** 시간 범위 (시작, 종료 타임스탬프) */
  timeRange: { start: number; end: number }
  /** 연결선 표시 여부 */
  showConnections?: boolean
  /** 배경색 표시 여부 */
  showBackground?: boolean
  /** 연결선 두께 */
  lineWidth?: number
  /** 배경 투명도 (0~1) */
  backgroundOpacity?: number
  /** 거래 클릭 핸들러 */
  onTradeClick?: (trade: TradeConnection) => void
  /** 호버 시 툴팁 표시 */
  showTooltip?: boolean
}

// ==================== 유틸리티 ====================

/**
 * 가격을 Y 좌표로 변환
 */
function priceToY(price: number, range: { min: number; max: number }, height: number): number {
  const { min, max } = range
  if (max === min) return height / 2
  return height - ((price - min) / (max - min)) * height
}

/**
 * 타임스탬프를 X 좌표로 변환
 */
function timeToX(
  timestamp: number,
  range: { start: number; end: number },
  width: number
): number {
  const { start, end } = range
  if (end === start) return width / 2
  return ((timestamp - start) / (end - start)) * width
}

/**
 * 손익에 따른 색상 반환
 */
function getPnLColor(pnl: number, isBackground: boolean = false): string {
  const opacity = isBackground ? 0.15 : 1
  if (pnl > 0) {
    return isBackground ? `rgba(34, 197, 94, ${opacity})` : '#22c55e' // 초록
  } else if (pnl < 0) {
    return isBackground ? `rgba(239, 68, 68, ${opacity})` : '#ef4444' // 빨강
  }
  return isBackground ? `rgba(156, 163, 175, ${opacity})` : '#9ca3af' // 회색
}

// ==================== 컴포넌트 ====================

/**
 * 거래 연결 오버레이 컴포넌트
 */
export const TradeConnectionOverlay: Component<TradeConnectionOverlayProps> = (props) => {
  const [hoveredTrade, setHoveredTrade] = createSignal<string | null>(null)
  const [tooltipPos, setTooltipPos] = createSignal({ x: 0, y: 0 })

  // 기본값 설정
  const showConnections = () => props.showConnections ?? true
  const showBackground = () => props.showBackground ?? true
  const lineWidth = () => props.lineWidth ?? 2
  const backgroundOpacity = () => props.backgroundOpacity ?? 0.15
  const showTooltip = () => props.showTooltip ?? true

  // 표시 범위 내 거래 필터링
  const visibleTrades = createMemo(() => {
    const { start, end } = props.timeRange
    return props.trades.filter((trade) => {
      const entryTs = new Date(trade.entryTime).getTime()
      const exitTs = new Date(trade.exitTime).getTime()
      // 거래 구간이 표시 범위와 겹치는지 확인
      return exitTs >= start && entryTs <= end
    })
  })

  // 거래별 좌표 계산
  const tradeCoordinates = createMemo(() => {
    return visibleTrades().map((trade) => {
      const entryTs = new Date(trade.entryTime).getTime()
      const exitTs = new Date(trade.exitTime).getTime()

      const x1 = timeToX(entryTs, props.timeRange, props.width)
      const y1 = priceToY(trade.entryPrice, props.priceRange, props.height)
      const x2 = timeToX(exitTs, props.timeRange, props.width)
      const y2 = priceToY(trade.exitPrice, props.priceRange, props.height)

      return {
        trade,
        x1: Math.max(0, Math.min(x1, props.width)),
        y1: Math.max(0, Math.min(y1, props.height)),
        x2: Math.max(0, Math.min(x2, props.width)),
        y2: Math.max(0, Math.min(y2, props.height)),
      }
    })
  })

  // 마우스 이벤트 핸들러
  const handleMouseEnter = (tradeId: string, e: MouseEvent) => {
    if (!showTooltip()) return
    setHoveredTrade(tradeId)
    setTooltipPos({ x: e.clientX, y: e.clientY })
  }

  const handleMouseLeave = () => {
    setHoveredTrade(null)
  }

  const handleMouseMove = (e: MouseEvent) => {
    if (hoveredTrade()) {
      setTooltipPos({ x: e.clientX, y: e.clientY })
    }
  }

  const handleClick = (trade: TradeConnection) => {
    props.onTradeClick?.(trade)
  }

  // 호버된 거래 정보
  const hoveredTradeInfo = createMemo(() => {
    const id = hoveredTrade()
    if (!id) return null
    return props.trades.find((t) => t.id === id) || null
  })

  return (
    <div class="relative" style={{ width: `${props.width}px`, height: `${props.height}px` }}>
      {/* SVG 오버레이 */}
      <svg
        class="absolute inset-0 pointer-events-none"
        width={props.width}
        height={props.height}
        viewBox={`0 0 ${props.width} ${props.height}`}
      >
        {/* 배경색 영역 */}
        <Show when={showBackground()}>
          <For each={tradeCoordinates()}>
            {(coord) => {
              const { trade, x1, x2 } = coord
              const minX = Math.min(x1, x2)
              const rectWidth = Math.abs(x2 - x1)
              const color = getPnLColor(trade.pnl, true)

              return (
                <g>
                  {/* 손익 구간 배경 사각형 */}
                  <rect
                    x={minX}
                    y={0}
                    width={Math.max(rectWidth, 2)}
                    height={props.height}
                    fill={color}
                    style={{ opacity: backgroundOpacity() }}
                    class="pointer-events-auto cursor-pointer"
                    onMouseEnter={(e) => handleMouseEnter(trade.id, e)}
                    onMouseLeave={handleMouseLeave}
                    onMouseMove={handleMouseMove}
                    onClick={() => handleClick(trade)}
                  />

                  {/* 진입/청산 세로 점선 */}
                  <line
                    x1={x1}
                    y1={0}
                    x2={x1}
                    y2={props.height}
                    stroke={trade.side === 'Buy' ? '#22c55e' : '#ef4444'}
                    stroke-width={1}
                    stroke-dasharray="4,4"
                    opacity={0.5}
                  />
                  <line
                    x1={x2}
                    y1={0}
                    x2={x2}
                    y2={props.height}
                    stroke={trade.pnl >= 0 ? '#22c55e' : '#ef4444'}
                    stroke-width={1}
                    stroke-dasharray="4,4"
                    opacity={0.5}
                  />
                </g>
              )
            }}
          </For>
        </Show>

        {/* 연결선 */}
        <Show when={showConnections()}>
          <For each={tradeCoordinates()}>
            {({ trade, x1, y1, x2, y2 }) => {
              const color = getPnLColor(trade.pnl)
              const isHovered = hoveredTrade() === trade.id

              return (
                <g>
                  {/* 연결선 (곡선) */}
                  <path
                    d={`M ${x1} ${y1} C ${(x1 + x2) / 2} ${y1}, ${(x1 + x2) / 2} ${y2}, ${x2} ${y2}`}
                    fill="none"
                    stroke={color}
                    stroke-width={isHovered ? lineWidth() + 1 : lineWidth()}
                    stroke-linecap="round"
                    opacity={isHovered ? 1 : 0.7}
                    class="pointer-events-auto cursor-pointer transition-all"
                    onMouseEnter={(e) => handleMouseEnter(trade.id, e)}
                    onMouseLeave={handleMouseLeave}
                    onMouseMove={handleMouseMove}
                    onClick={() => handleClick(trade)}
                  />

                  {/* 진입점 마커 */}
                  <circle
                    cx={x1}
                    cy={y1}
                    r={isHovered ? 6 : 4}
                    fill={trade.side === 'Buy' ? '#22c55e' : '#ef4444'}
                    stroke="white"
                    stroke-width={2}
                    class="pointer-events-auto cursor-pointer transition-all"
                    onMouseEnter={(e) => handleMouseEnter(trade.id, e)}
                    onMouseLeave={handleMouseLeave}
                    onClick={() => handleClick(trade)}
                  />

                  {/* 청산점 마커 */}
                  <circle
                    cx={x2}
                    cy={y2}
                    r={isHovered ? 6 : 4}
                    fill={color}
                    stroke="white"
                    stroke-width={2}
                    class="pointer-events-auto cursor-pointer transition-all"
                    onMouseEnter={(e) => handleMouseEnter(trade.id, e)}
                    onMouseLeave={handleMouseLeave}
                    onClick={() => handleClick(trade)}
                  />

                  {/* 손익 라벨 (연결선 중간) */}
                  <Show when={isHovered}>
                    <g transform={`translate(${(x1 + x2) / 2}, ${(y1 + y2) / 2 - 10})`}>
                      <rect
                        x={-30}
                        y={-12}
                        width={60}
                        height={24}
                        rx={4}
                        fill={trade.pnl >= 0 ? '#22c55e' : '#ef4444'}
                        opacity={0.9}
                      />
                      <text
                        x={0}
                        y={4}
                        text-anchor="middle"
                        font-size="11"
                        font-weight="bold"
                        fill="white"
                      >
                        {formatPercent(trade.returnPct)}
                      </text>
                    </g>
                  </Show>
                </g>
              )
            }}
          </For>
        </Show>
      </svg>

      {/* 툴팁 */}
      <Show when={showTooltip() && hoveredTradeInfo()}>
        {(trade) => (
          <div
            class="fixed z-50 px-3 py-2 bg-gray-900/95 text-white text-xs rounded-lg shadow-lg pointer-events-none"
            style={{
              left: `${tooltipPos().x + 10}px`,
              top: `${tooltipPos().y - 60}px`,
            }}
          >
            <div class="flex items-center gap-2 mb-1">
              <span class={`font-bold ${trade().pnl >= 0 ? 'text-green-400' : 'text-red-400'}`}>
                {trade().symbol}
              </span>
              <span class={`px-1.5 py-0.5 rounded text-[10px] ${
                trade().side === 'Buy' ? 'bg-green-500/30 text-green-300' : 'bg-red-500/30 text-red-300'
              }`}>
                {trade().side === 'Buy' ? '매수' : '매도'}
              </span>
            </div>
            <div class="space-y-0.5 text-gray-300">
              <div class="flex justify-between gap-4">
                <span>진입:</span>
                <span class="font-medium text-white">{formatQuantity(trade().entryPrice)}</span>
              </div>
              <div class="flex justify-between gap-4">
                <span>청산:</span>
                <span class="font-medium text-white">{formatQuantity(trade().exitPrice)}</span>
              </div>
              <div class="flex justify-between gap-4 pt-1 border-t border-gray-700">
                <span>손익:</span>
                <span class={`font-bold ${trade().pnl >= 0 ? 'text-green-400' : 'text-red-400'}`}>
                  {formatQuantity(trade().pnl)} ({formatPercent(trade().returnPct)})
                </span>
              </div>
            </div>
          </div>
        )}
      </Show>
    </div>
  )
}

// ==================== 유틸리티 함수 ====================

/**
 * BacktestResult의 trades를 TradeConnection 형식으로 변환
 */
export function convertBacktestTrades(
  trades: Array<{
    symbol: string
    entry_time: string
    exit_time: string
    entry_price: string
    exit_price: string
    side: string
    pnl: string
    return_pct: string
  }>
): TradeConnection[] {
  return trades.map((trade, index) => ({
    id: `trade-${index}`,
    symbol: trade.symbol,
    entryTime: trade.entry_time,
    exitTime: trade.exit_time,
    entryPrice: parseFloat(trade.entry_price),
    exitPrice: parseFloat(trade.exit_price),
    side: trade.side as 'Buy' | 'Sell',
    pnl: parseFloat(trade.pnl),
    returnPct: parseFloat(trade.return_pct),
  }))
}

export default TradeConnectionOverlay
