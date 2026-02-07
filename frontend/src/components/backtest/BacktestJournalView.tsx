/**
 * 백테스트 결과를 매매일지 형태로 표시하는 컴포넌트
 *
 * 주요 기능:
 * - 거래 내역 테이블/타임라인 뷰
 * - 종목/매매방향 필터링
 * - 페이지네이션
 * - 거래 통계 요약
 */
import { createSignal, createMemo, For, Show } from 'solid-js'
import { List, Clock, ChevronLeft, ChevronRight, TrendingUp, TrendingDown, Target, BarChart3 } from 'lucide-solid'
import { SymbolDisplay } from '../SymbolDisplay'
import { formatCurrency, formatPercent, formatDateTime } from '../../utils/format'
import type { TradeHistoryItem } from '../../api/client'

type ViewMode = 'table' | 'timeline'
type SortField = 'entry_time' | 'exit_time' | 'pnl' | 'return_pct' | 'symbol'
type SortOrder = 'asc' | 'desc'

interface BacktestJournalViewProps {
  /** 거래 내역 */
  trades: TradeHistoryItem[]
  /** 전략 ID (표시용) */
  strategyId?: string
  /** 기본 페이지 크기 */
  defaultPageSize?: number
}

export function BacktestJournalView(props: BacktestJournalViewProps) {
  // 뷰 모드
  const [viewMode, setViewMode] = createSignal<ViewMode>('table')

  // 필터 상태
  const [symbolFilter, setSymbolFilter] = createSignal('')
  const [sideFilter, setSideFilter] = createSignal('')

  // 정렬 상태
  const [sortField, setSortField] = createSignal<SortField>('entry_time')
  const [sortOrder, setSortOrder] = createSignal<SortOrder>('desc')

  // 페이지네이션 상태
  const pageSize = props.defaultPageSize || 20
  const [currentPage, setCurrentPage] = createSignal(1)

  // 필터링된 거래 목록
  const filteredTrades = createMemo(() => {
    let result = [...props.trades]

    // 종목 필터
    const symbolQuery = symbolFilter().toLowerCase()
    if (symbolQuery) {
      result = result.filter(t =>
        t.symbol.toLowerCase().includes(symbolQuery)
      )
    }

    // 매매방향 필터
    const side = sideFilter()
    if (side) {
      result = result.filter(t => t.side.toLowerCase() === side.toLowerCase())
    }

    // 정렬
    const field = sortField()
    const order = sortOrder()
    result.sort((a, b) => {
      let cmp = 0
      switch (field) {
        case 'entry_time':
        case 'exit_time':
          cmp = new Date(a[field]).getTime() - new Date(b[field]).getTime()
          break
        case 'pnl':
        case 'return_pct':
          cmp = parseFloat(a[field]) - parseFloat(b[field])
          break
        case 'symbol':
          cmp = a.symbol.localeCompare(b.symbol)
          break
      }
      return order === 'asc' ? cmp : -cmp
    })

    return result
  })

  // 페이지네이션된 거래 목록
  const paginatedTrades = createMemo(() => {
    const start = (currentPage() - 1) * pageSize
    return filteredTrades().slice(start, start + pageSize)
  })

  // 총 페이지 수
  const totalPages = createMemo(() => Math.ceil(filteredTrades().length / pageSize))

  // 통계 계산
  const statistics = createMemo(() => {
    const trades = filteredTrades()
    if (trades.length === 0) {
      return {
        totalTrades: 0,
        winningTrades: 0,
        losingTrades: 0,
        winRate: 0,
        totalPnL: 0,
        avgPnL: 0,
        avgWin: 0,
        avgLoss: 0,
        largestWin: 0,
        largestLoss: 0,
        avgHoldingTime: 0,
      }
    }

    const pnls = trades.map(t => parseFloat(t.pnl))
    const wins = pnls.filter(p => p > 0)
    const losses = pnls.filter(p => p < 0)
    const holdingTimes = trades.map(t => {
      const entry = new Date(t.entry_time).getTime()
      const exit = new Date(t.exit_time).getTime()
      return (exit - entry) / (1000 * 60 * 60) // 시간 단위
    })

    return {
      totalTrades: trades.length,
      winningTrades: wins.length,
      losingTrades: losses.length,
      winRate: trades.length > 0 ? (wins.length / trades.length) * 100 : 0,
      totalPnL: pnls.reduce((a, b) => a + b, 0),
      avgPnL: pnls.reduce((a, b) => a + b, 0) / trades.length,
      avgWin: wins.length > 0 ? wins.reduce((a, b) => a + b, 0) / wins.length : 0,
      avgLoss: losses.length > 0 ? losses.reduce((a, b) => a + b, 0) / losses.length : 0,
      largestWin: wins.length > 0 ? Math.max(...wins) : 0,
      largestLoss: losses.length > 0 ? Math.min(...losses) : 0,
      avgHoldingTime: holdingTimes.reduce((a, b) => a + b, 0) / holdingTimes.length,
    }
  })

  // 날짜별 그룹핑 (타임라인용)
  const groupedByDate = createMemo(() => {
    const groups: Record<string, TradeHistoryItem[]> = {}
    for (const trade of filteredTrades()) {
      const date = trade.entry_time.split('T')[0]
      if (!groups[date]) groups[date] = []
      groups[date].push(trade)
    }
    return Object.entries(groups).sort((a, b) => b[0].localeCompare(a[0]))
  })

  // 정렬 토글
  const toggleSort = (field: SortField) => {
    if (sortField() === field) {
      setSortOrder(prev => prev === 'asc' ? 'desc' : 'asc')
    } else {
      setSortField(field)
      setSortOrder('desc')
    }
  }

  // 날짜 포맷
  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr)
    return date.toLocaleDateString('ko-KR', {
      year: 'numeric',
      month: 'long',
      day: 'numeric',
      weekday: 'short',
    })
  }

  // 보유 시간 포맷
  const formatHoldingTime = (hours: number) => {
    if (hours < 1) return `${Math.round(hours * 60)}분`
    if (hours < 24) return `${hours.toFixed(1)}시간`
    return `${Math.round(hours / 24)}일`
  }

  // 페이지 변경
  const goToPage = (page: number) => {
    if (page >= 1 && page <= totalPages()) {
      setCurrentPage(page)
    }
  }

  return (
    <div class="space-y-4">
      {/* 통계 요약 카드 */}
      <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
        <div class="p-3 bg-[var(--color-surface-light)]/30 rounded-lg">
          <div class="flex items-center gap-2 text-xs text-[var(--color-text-muted)] mb-1">
            <BarChart3 class="w-3.5 h-3.5" />
            총 거래
          </div>
          <div class="text-lg font-semibold">{statistics().totalTrades}건</div>
          <div class="text-xs text-[var(--color-text-muted)]">
            승 {statistics().winningTrades} / 패 {statistics().losingTrades}
          </div>
        </div>

        <div class="p-3 bg-[var(--color-surface-light)]/30 rounded-lg">
          <div class="flex items-center gap-2 text-xs text-[var(--color-text-muted)] mb-1">
            <Target class="w-3.5 h-3.5" />
            승률
          </div>
          <div class={`text-lg font-semibold ${statistics().winRate >= 50 ? 'text-green-500' : 'text-red-500'}`}>
            {statistics().winRate.toFixed(1)}%
          </div>
        </div>

        <div class="p-3 bg-[var(--color-surface-light)]/30 rounded-lg">
          <div class="flex items-center gap-2 text-xs text-[var(--color-text-muted)] mb-1">
            <TrendingUp class="w-3.5 h-3.5" />
            총 손익
          </div>
          <div class={`text-lg font-semibold ${statistics().totalPnL >= 0 ? 'text-green-500' : 'text-red-500'}`}>
            {formatCurrency(statistics().totalPnL)}
          </div>
        </div>

        <div class="p-3 bg-[var(--color-surface-light)]/30 rounded-lg">
          <div class="flex items-center gap-2 text-xs text-[var(--color-text-muted)] mb-1">
            <Clock class="w-3.5 h-3.5" />
            평균 보유
          </div>
          <div class="text-lg font-semibold">
            {formatHoldingTime(statistics().avgHoldingTime)}
          </div>
        </div>
      </div>

      {/* 상세 통계 (접이식) */}
      <details class="text-sm">
        <summary class="cursor-pointer text-[var(--color-text-muted)] hover:text-[var(--color-text)]">
          상세 통계 보기
        </summary>
        <div class="mt-2 grid grid-cols-2 md:grid-cols-4 gap-3 text-xs">
          <div class="p-2 bg-[var(--color-surface-light)]/20 rounded">
            <span class="text-[var(--color-text-muted)]">평균 수익:</span>
            <span class="ml-1 text-green-500">{formatCurrency(statistics().avgWin)}</span>
          </div>
          <div class="p-2 bg-[var(--color-surface-light)]/20 rounded">
            <span class="text-[var(--color-text-muted)]">평균 손실:</span>
            <span class="ml-1 text-red-500">{formatCurrency(statistics().avgLoss)}</span>
          </div>
          <div class="p-2 bg-[var(--color-surface-light)]/20 rounded">
            <span class="text-[var(--color-text-muted)]">최대 수익:</span>
            <span class="ml-1 text-green-500">{formatCurrency(statistics().largestWin)}</span>
          </div>
          <div class="p-2 bg-[var(--color-surface-light)]/20 rounded">
            <span class="text-[var(--color-text-muted)]">최대 손실:</span>
            <span class="ml-1 text-red-500">{formatCurrency(statistics().largestLoss)}</span>
          </div>
        </div>
      </details>

      {/* 필터 + 뷰 모드 전환 */}
      <div class="flex items-center justify-between flex-wrap gap-3">
        <div class="flex gap-3 flex-wrap items-center">
          {/* 종목 검색 */}
          <input
            type="text"
            placeholder="종목 검색..."
            value={symbolFilter()}
            onInput={(e) => {
              setSymbolFilter(e.currentTarget.value)
              setCurrentPage(1)
            }}
            class="px-3 py-1.5 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-sm text-[var(--color-text)] placeholder-[var(--color-text-muted)] focus:outline-none focus:border-blue-500 w-32"
          />
          {/* 매수/매도 필터 */}
          <select
            value={sideFilter()}
            onChange={(e) => {
              setSideFilter(e.currentTarget.value)
              setCurrentPage(1)
            }}
            class="px-3 py-1.5 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-sm text-[var(--color-text)] focus:outline-none focus:border-blue-500"
          >
            <option value="">전체</option>
            <option value="buy">매수</option>
            <option value="sell">매도</option>
          </select>
        </div>

        {/* 뷰 모드 토글 */}
        <div class="flex rounded-lg overflow-hidden border border-[var(--color-surface-light)]">
          <button
            onClick={() => setViewMode('table')}
            class={`px-3 py-1.5 flex items-center gap-1.5 text-xs ${
              viewMode() === 'table'
                ? 'bg-blue-600 text-white'
                : 'bg-[var(--color-surface)] text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)]'
            }`}
          >
            <List class="w-3.5 h-3.5" />
            테이블
          </button>
          <button
            onClick={() => setViewMode('timeline')}
            class={`px-3 py-1.5 flex items-center gap-1.5 text-xs ${
              viewMode() === 'timeline'
                ? 'bg-blue-600 text-white'
                : 'bg-[var(--color-surface)] text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)]'
            }`}
          >
            <Clock class="w-3.5 h-3.5" />
            타임라인
          </button>
        </div>
      </div>

      {/* 테이블 뷰 */}
      <Show when={viewMode() === 'table'}>
        <div class="overflow-x-auto">
          <table class="w-full text-sm">
            <thead>
              <tr class="text-left text-[var(--color-text-muted)] border-b border-[var(--color-surface-light)]">
                <th
                  class="pb-2 cursor-pointer hover:text-[var(--color-text)]"
                  onClick={() => toggleSort('entry_time')}
                >
                  진입시각 {sortField() === 'entry_time' && (sortOrder() === 'asc' ? '↑' : '↓')}
                </th>
                <th
                  class="pb-2 cursor-pointer hover:text-[var(--color-text)]"
                  onClick={() => toggleSort('symbol')}
                >
                  종목 {sortField() === 'symbol' && (sortOrder() === 'asc' ? '↑' : '↓')}
                </th>
                <th class="pb-2">방향</th>
                <th class="pb-2">진입가</th>
                <th class="pb-2">청산가</th>
                <th class="pb-2">수량</th>
                <th class="pb-2">사유</th>
                <th class="pb-2">보유시간</th>
                <th
                  class="pb-2 cursor-pointer hover:text-[var(--color-text)] text-right"
                  onClick={() => toggleSort('pnl')}
                >
                  손익 {sortField() === 'pnl' && (sortOrder() === 'asc' ? '↑' : '↓')}
                </th>
                <th
                  class="pb-2 cursor-pointer hover:text-[var(--color-text)] text-right"
                  onClick={() => toggleSort('return_pct')}
                >
                  수익률 {sortField() === 'return_pct' && (sortOrder() === 'asc' ? '↑' : '↓')}
                </th>
              </tr>
            </thead>
            <tbody>
              <For each={paginatedTrades()}>
                {(trade) => {
                  const holdingHours = (new Date(trade.exit_time).getTime() - new Date(trade.entry_time).getTime()) / (1000 * 60 * 60)
                  const pnl = parseFloat(trade.pnl)
                  const returnPct = parseFloat(trade.return_pct)

                  return (
                    <tr class="border-b border-[var(--color-surface-light)] hover:bg-[var(--color-surface-light)]/30">
                      <td class="py-2 text-xs text-[var(--color-text-muted)]">
                        {formatDateTime(trade.entry_time)}
                      </td>
                      <td class="py-2">
                        <SymbolDisplay
                          ticker={trade.symbol}
                          mode="inline"
                          size="sm"
                          autoFetch={true}
                        />
                      </td>
                      <td class={`py-2 font-medium ${trade.side === 'Buy' ? 'text-green-500' : 'text-red-500'}`}>
                        {trade.side === 'Buy' ? '매수' : '매도'}
                      </td>
                      <td class="py-2">{formatCurrency(trade.entry_price)}</td>
                      <td class="py-2">{formatCurrency(trade.exit_price)}</td>
                      <td class="py-2">{parseFloat(trade.quantity).toFixed(4)}</td>
                      <td class="py-2 text-xs text-[var(--color-text-muted)]" title={`진입: ${trade.entry_reason || '-'}\n청산: ${trade.exit_reason || '-'}`}>
                        {trade.entry_reason || trade.exit_reason || '-'}
                      </td>
                      <td class="py-2 text-xs text-[var(--color-text-muted)]">
                        {formatHoldingTime(holdingHours)}
                      </td>
                      <td class={`py-2 text-right font-medium ${pnl >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                        {pnl >= 0 ? '+' : ''}{formatCurrency(trade.pnl)}
                      </td>
                      <td class={`py-2 text-right ${returnPct >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                        {returnPct >= 0 ? '+' : ''}{returnPct.toFixed(2)}%
                      </td>
                    </tr>
                  )
                }}
              </For>
            </tbody>
          </table>
        </div>

        {/* 페이지네이션 */}
        <Show when={totalPages() > 1}>
          <div class="flex items-center justify-between mt-4">
            <div class="text-xs text-[var(--color-text-muted)]">
              {filteredTrades().length}건 중 {(currentPage() - 1) * pageSize + 1}-{Math.min(currentPage() * pageSize, filteredTrades().length)}
            </div>
            <div class="flex items-center gap-2">
              <button
                onClick={() => goToPage(currentPage() - 1)}
                disabled={currentPage() === 1}
                class="p-1.5 rounded hover:bg-[var(--color-surface-light)] disabled:opacity-30 disabled:cursor-not-allowed"
              >
                <ChevronLeft class="w-4 h-4" />
              </button>
              <span class="text-sm">
                {currentPage()} / {totalPages()}
              </span>
              <button
                onClick={() => goToPage(currentPage() + 1)}
                disabled={currentPage() === totalPages()}
                class="p-1.5 rounded hover:bg-[var(--color-surface-light)] disabled:opacity-30 disabled:cursor-not-allowed"
              >
                <ChevronRight class="w-4 h-4" />
              </button>
            </div>
          </div>
        </Show>
      </Show>

      {/* 타임라인 뷰 */}
      <Show when={viewMode() === 'timeline'}>
        <div class="space-y-4">
          <For each={groupedByDate()}>
            {([date, trades]) => {
              const dailyPnL = trades.reduce((sum, t) => sum + parseFloat(t.pnl), 0)
              return (
                <div class="border-l-2 border-[var(--color-surface-light)] pl-4">
                  {/* 날짜 헤더 */}
                  <div class="flex items-center justify-between mb-2">
                    <h4 class="font-medium text-sm">{formatDate(date)}</h4>
                    <div class="flex items-center gap-3 text-xs">
                      <span class="text-[var(--color-text-muted)]">{trades.length}건</span>
                      <span class={dailyPnL >= 0 ? 'text-green-500' : 'text-red-500'}>
                        {dailyPnL >= 0 ? '+' : ''}{formatCurrency(dailyPnL)}
                      </span>
                    </div>
                  </div>

                  {/* 해당 날짜 거래 목록 */}
                  <div class="space-y-2">
                    <For each={trades}>
                      {(trade) => {
                        const pnl = parseFloat(trade.pnl)
                        const returnPct = parseFloat(trade.return_pct)
                        const entryTime = new Date(trade.entry_time).toLocaleTimeString('ko-KR', { hour: '2-digit', minute: '2-digit' })

                        return (
                          <div class="flex items-center gap-3 p-2 bg-[var(--color-surface-light)]/20 rounded-lg text-sm">
                            <div class="text-xs text-[var(--color-text-muted)] w-12">{entryTime}</div>
                            <div class={`w-8 text-xs font-medium ${trade.side === 'Buy' ? 'text-green-500' : 'text-red-500'}`}>
                              {trade.side === 'Buy' ? '매수' : '매도'}
                            </div>
                            <div class="flex-1">
                              <SymbolDisplay
                                ticker={trade.symbol}
                                mode="inline"
                                size="sm"
                                autoFetch={true}
                              />
                            </div>
                            <div class="text-xs text-[var(--color-text-muted)]">
                              {formatCurrency(trade.entry_price)} → {formatCurrency(trade.exit_price)}
                            </div>
                            <div class={`text-right font-medium min-w-[80px] ${pnl >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                              {pnl >= 0 ? '+' : ''}{formatCurrency(trade.pnl)}
                            </div>
                            <div class={`text-right text-xs min-w-[50px] ${returnPct >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                              {returnPct >= 0 ? '+' : ''}{returnPct.toFixed(2)}%
                            </div>
                          </div>
                        )
                      }}
                    </For>
                  </div>
                </div>
              )
            }}
          </For>
        </div>
      </Show>

      {/* 빈 상태 */}
      <Show when={filteredTrades().length === 0}>
        <div class="text-center py-8 text-[var(--color-text-muted)]">
          <Show when={props.trades.length === 0} fallback="필터 조건에 맞는 거래가 없습니다.">
            거래 내역이 없습니다.
          </Show>
        </div>
      </Show>
    </div>
  )
}

export default BacktestJournalView
