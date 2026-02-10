/**
 * 백테스트 매매일지 팝업 모달
 *
 * 실제 매매일지 화면 수준의 기능을 팝업으로 제공합니다.
 * - 라운드트립 뷰: 완료된 거래 (진입+청산 쌍)
 * - 모든 거래 뷰: 개별 거래 (매수/매도 각각)
 */
import { createSignal, createMemo, For, Show, type Component } from 'solid-js'
import { X, TrendingUp, TrendingDown, ArrowRightLeft, List, BarChart3, ChevronLeft, ChevronRight } from 'lucide-solid'
import { SymbolDisplay } from '../SymbolDisplay'
import { formatCurrency, formatDateTime } from '../../utils/format'
import type { TradeHistoryItem, TradeResultItem } from '../../api/client'

// ==================== 타입 ====================

interface BacktestJournalModalProps {
  /** 모달 열림 상태 */
  isOpen: boolean
  /** 모달 닫기 핸들러 */
  onClose: () => void
  /** 완료된 거래 (라운드트립) */
  trades: TradeHistoryItem[]
  /** 모든 거래 (개별 매수/매도) */
  allTrades?: TradeResultItem[]
  /** 전략 ID */
  strategyId?: string
  /** 종목 */
  symbol?: string
}

type ViewTab = 'roundtrip' | 'all'
type SortField = 'timestamp' | 'symbol' | 'side' | 'pnl' | 'price'
type SortOrder = 'asc' | 'desc'

// ==================== 메인 컴포넌트 ====================

export const BacktestJournalModal: Component<BacktestJournalModalProps> = (props) => {
  // 탭 상태
  const [activeTab, setActiveTab] = createSignal<ViewTab>('roundtrip')

  // 필터 상태
  const [symbolFilter, setSymbolFilter] = createSignal('')
  const [sideFilter, setSideFilter] = createSignal('')

  // 정렬 상태
  const [sortField, setSortField] = createSignal<SortField>('timestamp')
  const [sortOrder, setSortOrder] = createSignal<SortOrder>('desc')

  // 페이지네이션
  const pageSize = 20
  const [currentPage, setCurrentPage] = createSignal(1)

  // 탭 변경 시 페이지 리셋
  const handleTabChange = (tab: ViewTab) => {
    setActiveTab(tab)
    setCurrentPage(1)
  }

  // ==================== 라운드트립 뷰 ====================

  const filteredRoundTrips = createMemo(() => {
    let result = [...props.trades]

    // 종목 필터
    const symbolQuery = symbolFilter().toLowerCase()
    if (symbolQuery) {
      result = result.filter(t => t.symbol.toLowerCase().includes(symbolQuery))
    }

    // 매매방향 필터
    const side = sideFilter()
    if (side) {
      result = result.filter(t => t.side.toLowerCase() === side.toLowerCase())
    }

    // 정렬
    const order = sortOrder()
    result.sort((a, b) => {
      const cmp = new Date(a.entry_time).getTime() - new Date(b.entry_time).getTime()
      return order === 'asc' ? cmp : -cmp
    })

    return result
  })

  // ==================== 모든 거래 뷰 ====================

  const filteredAllTrades = createMemo(() => {
    if (!props.allTrades) return []

    let result = [...props.allTrades]

    // 종목 필터
    const symbolQuery = symbolFilter().toLowerCase()
    if (symbolQuery) {
      result = result.filter(t => t.symbol.toLowerCase().includes(symbolQuery))
    }

    // 매매방향 필터
    const side = sideFilter()
    if (side) {
      result = result.filter(t => t.side.toLowerCase() === side.toLowerCase())
    }

    // 정렬
    const order = sortOrder()
    const field = sortField()
    result.sort((a, b) => {
      let cmp = 0
      switch (field) {
        case 'timestamp':
          cmp = new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
          break
        case 'symbol':
          cmp = a.symbol.localeCompare(b.symbol)
          break
        case 'side':
          cmp = a.side.localeCompare(b.side)
          break
        case 'price':
          cmp = parseFloat(a.price) - parseFloat(b.price)
          break
        case 'pnl':
          cmp = parseFloat(a.realized_pnl || '0') - parseFloat(b.realized_pnl || '0')
          break
      }
      return order === 'asc' ? cmp : -cmp
    })

    return result
  })

  // 현재 탭의 필터링된 항목
  const currentItems = () => activeTab() === 'roundtrip' ? filteredRoundTrips() : filteredAllTrades()
  const totalPages = () => Math.ceil(currentItems().length / pageSize)

  // 페이지네이션
  const paginatedItems = createMemo(() => {
    const start = (currentPage() - 1) * pageSize
    return currentItems().slice(start, start + pageSize)
  })

  // 통계
  const statistics = createMemo(() => {
    if (activeTab() === 'roundtrip') {
      const trades = filteredRoundTrips()
      const pnls = trades.map(t => parseFloat(t.pnl))
      const wins = pnls.filter(p => p > 0)
      const losses = pnls.filter(p => p < 0)

      return {
        total: trades.length,
        wins: wins.length,
        losses: losses.length,
        winRate: trades.length > 0 ? (wins.length / trades.length) * 100 : 0,
        totalPnL: pnls.reduce((a, b) => a + b, 0),
        buys: trades.filter(t => t.side.toLowerCase() === 'buy').length,
        sells: trades.filter(t => t.side.toLowerCase() === 'sell').length,
      }
    } else {
      const trades = filteredAllTrades()
      const pnls = trades
        .filter(t => t.realized_pnl)
        .map(t => parseFloat(t.realized_pnl || '0'))

      return {
        total: trades.length,
        wins: pnls.filter(p => p > 0).length,
        losses: pnls.filter(p => p < 0).length,
        winRate: pnls.length > 0 ? (pnls.filter(p => p > 0).length / pnls.length) * 100 : 0,
        totalPnL: pnls.reduce((a, b) => a + b, 0),
        buys: trades.filter(t => t.side === 'buy').length,
        sells: trades.filter(t => t.side === 'sell').length,
      }
    }
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

  // ESC 키로 닫기
  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') props.onClose()
  }

  return (
    <Show when={props.isOpen}>
      <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
        onClick={(e) => e.target === e.currentTarget && props.onClose()}
        onKeyDown={handleKeyDown}
      >
        <div class="bg-[var(--color-surface)] rounded-2xl shadow-2xl w-[95vw] max-w-6xl max-h-[90vh] overflow-hidden flex flex-col">
          {/* 헤더 */}
          <div class="flex items-center justify-between px-6 py-4 border-b border-[var(--color-surface-light)]">
            <div class="flex items-center gap-3">
              <BarChart3 class="w-5 h-5 text-[var(--color-primary)]" />
              <h2 class="text-lg font-semibold text-[var(--color-text)]">
                매매일지
              </h2>
              <Show when={props.strategyId}>
                <span class="text-sm text-[var(--color-text-muted)]">
                  ({props.strategyId})
                </span>
              </Show>
            </div>
            <button
              onClick={props.onClose}
              class="p-2 rounded-lg hover:bg-[var(--color-surface-light)] transition-colors"
            >
              <X class="w-5 h-5" />
            </button>
          </div>

          {/* 탭 + 필터 */}
          <div class="px-6 py-3 border-b border-[var(--color-surface-light)] flex items-center justify-between flex-wrap gap-3">
            {/* 탭 */}
            <div class="flex rounded-lg overflow-hidden border border-[var(--color-surface-light)]">
              <button
                onClick={() => handleTabChange('roundtrip')}
                class={`px-4 py-2 flex items-center gap-2 text-sm font-medium transition-colors ${
                  activeTab() === 'roundtrip'
                    ? 'bg-[var(--color-primary)] text-white'
                    : 'bg-[var(--color-surface)] text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)]'
                }`}
              >
                <ArrowRightLeft class="w-4 h-4" />
                라운드트립 ({props.trades.length})
              </button>
              <button
                onClick={() => handleTabChange('all')}
                disabled={!props.allTrades || props.allTrades.length === 0}
                class={`px-4 py-2 flex items-center gap-2 text-sm font-medium transition-colors ${
                  activeTab() === 'all'
                    ? 'bg-[var(--color-primary)] text-white'
                    : 'bg-[var(--color-surface)] text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)]'
                } disabled:opacity-50 disabled:cursor-not-allowed`}
              >
                <List class="w-4 h-4" />
                모든 거래 ({props.allTrades?.length || 0})
              </button>
            </div>

            {/* 필터 */}
            <div class="flex gap-3 items-center">
              <input
                type="text"
                placeholder="종목 검색..."
                value={symbolFilter()}
                onInput={(e) => {
                  setSymbolFilter(e.currentTarget.value)
                  setCurrentPage(1)
                }}
                class="px-3 py-1.5 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-sm text-[var(--color-text)] placeholder-[var(--color-text-muted)] focus:outline-none focus:border-[var(--color-primary)] w-32"
              />
              <select
                value={sideFilter()}
                onChange={(e) => {
                  setSideFilter(e.currentTarget.value)
                  setCurrentPage(1)
                }}
                class="px-3 py-1.5 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-sm text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
              >
                <option value="">전체</option>
                <option value="buy">매수</option>
                <option value="sell">매도</option>
              </select>
            </div>
          </div>

          {/* 통계 요약 */}
          <div class="px-6 py-3 grid grid-cols-2 md:grid-cols-6 gap-3 border-b border-[var(--color-surface-light)] bg-[var(--color-surface-light)]/20">
            <div class="text-center">
              <div class="text-xs text-[var(--color-text-muted)]">총 거래</div>
              <div class="text-lg font-semibold">{statistics().total}건</div>
            </div>
            <div class="text-center">
              <div class="text-xs text-[var(--color-text-muted)]">매수</div>
              <div class="text-lg font-semibold text-green-500">{statistics().buys}건</div>
            </div>
            <div class="text-center">
              <div class="text-xs text-[var(--color-text-muted)]">매도</div>
              <div class="text-lg font-semibold text-red-500">{statistics().sells}건</div>
            </div>
            <div class="text-center">
              <div class="text-xs text-[var(--color-text-muted)]">승률</div>
              <div class={`text-lg font-semibold ${statistics().winRate >= 50 ? 'text-green-500' : 'text-red-500'}`}>
                {statistics().winRate.toFixed(1)}%
              </div>
            </div>
            <div class="text-center">
              <div class="text-xs text-[var(--color-text-muted)]">승/패</div>
              <div class="text-lg font-semibold">
                <span class="text-green-500">{statistics().wins}</span>
                <span class="text-[var(--color-text-muted)]">/</span>
                <span class="text-red-500">{statistics().losses}</span>
              </div>
            </div>
            <div class="text-center">
              <div class="text-xs text-[var(--color-text-muted)]">총 손익</div>
              <div class={`text-lg font-semibold ${statistics().totalPnL >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                {statistics().totalPnL >= 0 ? '+' : ''}{formatCurrency(statistics().totalPnL)}
              </div>
            </div>
          </div>

          {/* 테이블 */}
          <div class="flex-1 overflow-auto px-6 py-4">
            <Show when={activeTab() === 'roundtrip'}>
              <RoundTripTable
                trades={paginatedItems() as TradeHistoryItem[]}
                sortField={sortField()}
                sortOrder={sortOrder()}
                onSort={toggleSort}
              />
            </Show>

            <Show when={activeTab() === 'all'}>
              <AllTradesTable
                trades={paginatedItems() as TradeResultItem[]}
                sortField={sortField()}
                sortOrder={sortOrder()}
                onSort={toggleSort}
              />
            </Show>

            {/* 빈 상태 */}
            <Show when={currentItems().length === 0}>
              <div class="text-center py-12 text-[var(--color-text-muted)]">
                <BarChart3 class="w-12 h-12 mx-auto mb-3 opacity-50" />
                <p>
                  {symbolFilter() || sideFilter()
                    ? '필터 조건에 맞는 거래가 없습니다.'
                    : '거래 내역이 없습니다.'}
                </p>
              </div>
            </Show>
          </div>

          {/* 페이지네이션 */}
          <Show when={totalPages() > 1}>
            <div class="px-6 py-3 border-t border-[var(--color-surface-light)] flex items-center justify-between">
              <div class="text-sm text-[var(--color-text-muted)]">
                {currentItems().length}건 중 {(currentPage() - 1) * pageSize + 1}-{Math.min(currentPage() * pageSize, currentItems().length)}
              </div>
              <div class="flex items-center gap-2">
                <button
                  onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                  disabled={currentPage() === 1}
                  class="p-2 rounded hover:bg-[var(--color-surface-light)] disabled:opacity-30 disabled:cursor-not-allowed"
                >
                  <ChevronLeft class="w-4 h-4" />
                </button>
                <span class="text-sm px-2">
                  {currentPage()} / {totalPages()}
                </span>
                <button
                  onClick={() => setCurrentPage(p => Math.min(totalPages(), p + 1))}
                  disabled={currentPage() === totalPages()}
                  class="p-2 rounded hover:bg-[var(--color-surface-light)] disabled:opacity-30 disabled:cursor-not-allowed"
                >
                  <ChevronRight class="w-4 h-4" />
                </button>
              </div>
            </div>
          </Show>
        </div>
      </div>
    </Show>
  )
}

// ==================== 서브 컴포넌트 ====================

/** 라운드트립 테이블 */
const RoundTripTable: Component<{
  trades: TradeHistoryItem[]
  sortField: SortField
  sortOrder: SortOrder
  onSort: (field: SortField) => void
}> = (props) => {
  return (
    <table class="w-full text-sm">
      <thead>
        <tr class="text-left text-[var(--color-text-muted)] border-b border-[var(--color-surface-light)]">
          <th class="pb-2 cursor-pointer hover:text-[var(--color-text)]" onClick={() => props.onSort('timestamp')}>
            진입시각 {props.sortField === 'timestamp' && (props.sortOrder === 'asc' ? '↑' : '↓')}
          </th>
          <th class="pb-2 cursor-pointer hover:text-[var(--color-text)]" onClick={() => props.onSort('symbol')}>
            종목 {props.sortField === 'symbol' && (props.sortOrder === 'asc' ? '↑' : '↓')}
          </th>
          <th class="pb-2">방향</th>
          <th class="pb-2">진입가</th>
          <th class="pb-2">청산가</th>
          <th class="pb-2">수량</th>
          <th class="pb-2">사유</th>
          <th class="pb-2 text-right cursor-pointer hover:text-[var(--color-text)]" onClick={() => props.onSort('pnl')}>
            손익 {props.sortField === 'pnl' && (props.sortOrder === 'asc' ? '↑' : '↓')}
          </th>
          <th class="pb-2 text-right">수익률</th>
        </tr>
      </thead>
      <tbody>
        <For each={props.trades}>
          {(trade) => {
            const pnl = parseFloat(trade.pnl)
            const returnPct = parseFloat(trade.return_pct)
            return (
              <tr class="border-b border-[var(--color-surface-light)]/50 hover:bg-[var(--color-surface-light)]/30">
                <td class="py-2.5 text-xs text-[var(--color-text-muted)]">
                  {formatDateTime(trade.entry_time)}
                </td>
                <td class="py-2.5">
                  <SymbolDisplay ticker={trade.symbol} mode="inline" size="sm" autoFetch />
                </td>
                <td class={`py-2.5 font-medium ${trade.side === 'Buy' ? 'text-green-500' : 'text-red-500'}`}>
                  {trade.side === 'Buy' ? '매수' : '매도'}
                </td>
                <td class="py-2.5">{formatCurrency(trade.entry_price)}</td>
                <td class="py-2.5">{formatCurrency(trade.exit_price)}</td>
                <td class="py-2.5">{parseFloat(trade.quantity).toFixed(4)}</td>
                <td class="py-2.5 text-xs text-[var(--color-text-muted)] max-w-[120px] truncate" title={trade.entry_reason || trade.exit_reason || '-'}>
                  {trade.entry_reason || trade.exit_reason || '-'}
                </td>
                <td class={`py-2.5 text-right font-medium ${pnl >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                  {pnl >= 0 ? '+' : ''}{formatCurrency(trade.pnl)}
                </td>
                <td class={`py-2.5 text-right ${returnPct >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                  {returnPct >= 0 ? '+' : ''}{returnPct.toFixed(2)}%
                </td>
              </tr>
            )
          }}
        </For>
      </tbody>
    </table>
  )
}

/** 신호 유형을 한글로 변환 */
const getSignalTypeLabel = (signalType: string): string => {
  const labels: Record<string, string> = {
    entry: '진입',
    exit: '청산',
    add_to_position: '추가매수',
    reduce_position: '부분청산',
    scale: '스케일',
    alert: '알림',
  }
  return labels[signalType] || signalType
}

/** 모든 거래 테이블 */
const AllTradesTable: Component<{
  trades: TradeResultItem[]
  sortField: SortField
  sortOrder: SortOrder
  onSort: (field: SortField) => void
}> = (props) => {
  return (
    <table class="w-full text-sm">
      <thead>
        <tr class="text-left text-[var(--color-text-muted)] border-b border-[var(--color-surface-light)]">
          <th class="pb-2 cursor-pointer hover:text-[var(--color-text)]" onClick={() => props.onSort('timestamp')}>
            체결시각 {props.sortField === 'timestamp' && (props.sortOrder === 'asc' ? '↑' : '↓')}
          </th>
          <th class="pb-2 cursor-pointer hover:text-[var(--color-text)]" onClick={() => props.onSort('symbol')}>
            종목 {props.sortField === 'symbol' && (props.sortOrder === 'asc' ? '↑' : '↓')}
          </th>
          <th class="pb-2 cursor-pointer hover:text-[var(--color-text)]" onClick={() => props.onSort('side')}>
            방향 {props.sortField === 'side' && (props.sortOrder === 'asc' ? '↑' : '↓')}
          </th>
          <th class="pb-2">신호</th>
          <th class="pb-2 cursor-pointer hover:text-[var(--color-text)]" onClick={() => props.onSort('price')}>
            체결가 {props.sortField === 'price' && (props.sortOrder === 'asc' ? '↑' : '↓')}
          </th>
          <th class="pb-2">수량</th>
          <th class="pb-2">수수료</th>
          <th class="pb-2 text-right cursor-pointer hover:text-[var(--color-text)]" onClick={() => props.onSort('pnl')}>
            실현손익 {props.sortField === 'pnl' && (props.sortOrder === 'asc' ? '↑' : '↓')}
          </th>
        </tr>
      </thead>
      <tbody>
        <For each={props.trades}>
          {(trade) => {
            const pnl = trade.realized_pnl ? parseFloat(trade.realized_pnl) : null
            const isBuy = trade.side === 'buy'
            return (
              <tr class="border-b border-[var(--color-surface-light)]/50 hover:bg-[var(--color-surface-light)]/30">
                <td class="py-2.5 text-xs text-[var(--color-text-muted)]">
                  {formatDateTime(trade.timestamp)}
                </td>
                <td class="py-2.5">
                  <SymbolDisplay ticker={trade.symbol} mode="inline" size="sm" autoFetch />
                </td>
                <td class={`py-2.5 font-medium ${isBuy ? 'text-green-500' : 'text-red-500'}`}>
                  <span class="flex items-center gap-1">
                    {isBuy ? <TrendingUp class="w-3.5 h-3.5" /> : <TrendingDown class="w-3.5 h-3.5" />}
                    {isBuy ? '매수' : '매도'}
                  </span>
                </td>
                <td class="py-2.5 text-xs">
                  <span class={`px-1.5 py-0.5 rounded ${
                    trade.signal_type === 'entry' ? 'bg-blue-500/20 text-blue-400' :
                    trade.signal_type === 'exit' ? 'bg-orange-500/20 text-orange-400' :
                    trade.signal_type === 'add_to_position' ? 'bg-green-500/20 text-green-400' :
                    trade.signal_type === 'reduce_position' ? 'bg-yellow-500/20 text-yellow-400' :
                    'bg-gray-500/20 text-gray-400'
                  }`}>
                    {getSignalTypeLabel(trade.signal_type)}
                  </span>
                </td>
                <td class="py-2.5">{formatCurrency(trade.price)}</td>
                <td class="py-2.5">{parseFloat(trade.quantity).toFixed(4)}</td>
                <td class="py-2.5 text-xs text-[var(--color-text-muted)]">
                  {formatCurrency(trade.commission)}
                </td>
                <td class={`py-2.5 text-right font-medium ${
                  pnl === null ? 'text-[var(--color-text-muted)]' :
                  pnl >= 0 ? 'text-green-500' : 'text-red-500'
                }`}>
                  {pnl !== null ? (
                    <>{pnl >= 0 ? '+' : ''}{formatCurrency(trade.realized_pnl!)}</>
                  ) : (
                    <span class="text-xs">-</span>
                  )}
                </td>
              </tr>
            )
          }}
        </For>
      </tbody>
    </table>
  )
}

export default BacktestJournalModal
