/**
 * 체결 내역 테이블/타임라인 컴포넌트
 *
 * 테이블 뷰와 타임라인 뷰를 지원합니다.
 * 페이지네이션 및 날짜 범위 필터를 지원합니다.
 */
import { createSignal, For, Show } from 'solid-js'
import { Edit3, Check, X, List, Clock, ChevronLeft, ChevronRight, Calendar } from 'lucide-solid'
import { SymbolDisplay } from '../SymbolDisplay'
import { EmptyState, DateRangePicker } from '../ui'
import { formatCurrency, formatQuantity, formatDateTime, getPnLColor } from '../../utils/format'
import type { JournalExecution } from '../../api/client'
import { updateJournalExecution } from '../../api/client'
import { createLogger } from '../../utils/logger'

const { error: logError } = createLogger('ExecutionsTable')

type ViewMode = 'table' | 'timeline'

interface ExecutionsTableProps {
  executions: JournalExecution[]
  onRefetch: () => void
  symbolFilter: string
  setSymbolFilter: (value: string) => void
  sideFilter: string
  setSideFilter: (value: string) => void
  /** 전체 항목 수 (페이지네이션용) */
  total?: number
  /** 현재 페이지 (1부터 시작) */
  currentPage?: number
  /** 페이지당 항목 수 */
  pageSize?: number
  /** 페이지 변경 핸들러 */
  onPageChange?: (page: number) => void
  /** 시작 날짜 필터 */
  startDate?: string
  /** 종료 날짜 필터 */
  endDate?: string
  /** 시작 날짜 변경 핸들러 */
  setStartDate?: (date: string) => void
  /** 종료 날짜 변경 핸들러 */
  setEndDate?: (date: string) => void
  /** 백테스트 모드 여부 (메모 편집 비활성화) */
  isBacktest?: boolean
}

export function ExecutionsTable(props: ExecutionsTableProps) {
  // 뷰 모드 상태
  const [viewMode, setViewMode] = createSignal<ViewMode>('table')

  // 메모 편집 상태
  const [editingMemoId, setEditingMemoId] = createSignal<string | null>(null)
  const [editingMemoText, setEditingMemoText] = createSignal('')
  const [isSavingMemo, setIsSavingMemo] = createSignal(false)

  // 메모 편집 시작
  const startEditingMemo = (id: string, currentMemo: string | null) => {
    setEditingMemoId(id)
    setEditingMemoText(currentMemo || '')
  }

  // 메모 편집 취소
  const cancelEditingMemo = () => {
    setEditingMemoId(null)
    setEditingMemoText('')
  }

  // 메모 저장
  const saveMemo = async (id: string) => {
    setIsSavingMemo(true)
    try {
      await updateJournalExecution(id, { memo: editingMemoText() })
      setEditingMemoId(null)
      setEditingMemoText('')
      props.onRefetch()
    } catch (error) {
      logError('Failed to save memo:', error)
    } finally {
      setIsSavingMemo(false)
    }
  }

  // 날짜별로 체결 내역 그룹화 (타임라인용)
  const groupedByDate = () => {
    const groups: Record<string, JournalExecution[]> = {}
    for (const exec of props.executions) {
      // executed_at 우선, 백테스트 데이터의 timestamp 폴백
      const dateTime = exec.executed_at || (exec as unknown as Record<string, string>).timestamp || ''
      const date = dateTime.split('T')[0] || 'unknown'
      if (!groups[date]) groups[date] = []
      groups[date].push(exec)
    }
    return Object.entries(groups).sort((a, b) => b[0].localeCompare(a[0]))
  }

  // 날짜 포맷팅
  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr)
    const today = new Date()
    const yesterday = new Date(today)
    yesterday.setDate(yesterday.getDate() - 1)

    if (dateStr === today.toISOString().split('T')[0]) return '오늘'
    if (dateStr === yesterday.toISOString().split('T')[0]) return '어제'

    return date.toLocaleDateString('ko-KR', {
      year: 'numeric',
      month: 'long',
      day: 'numeric',
      weekday: 'short',
    })
  }

  // 시간만 추출
  const formatTime = (datetime: string) => {
    return new Date(datetime).toLocaleTimeString('ko-KR', {
      hour: '2-digit',
      minute: '2-digit',
    })
  }

  return (
    <div>
      {/* 필터 + 뷰 모드 전환 */}
      <div class="mb-4 flex items-center justify-between flex-wrap gap-4">
        <div class="flex gap-4 flex-wrap items-center">
          {/* 종목 검색 */}
          <input
            type="text"
            placeholder="종목 검색..."
            value={props.symbolFilter}
            onInput={(e) => props.setSymbolFilter(e.currentTarget.value)}
            class="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-white placeholder-gray-500 focus:outline-none focus:border-blue-500"
          />
          {/* 매수/매도 필터 */}
          <select
            value={props.sideFilter}
            onChange={(e) => props.setSideFilter(e.currentTarget.value)}
            class="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-white focus:outline-none focus:border-blue-500"
          >
            <option value="">전체</option>
            <option value="buy">매수</option>
            <option value="sell">매도</option>
          </select>

          {/* 날짜 범위 필터 */}
          <Show when={props.setStartDate && props.setEndDate}>
            <div class="flex items-center gap-2">
              <Calendar class="w-4 h-4 text-gray-400" />
              <DateRangePicker
                startDate={props.startDate || ''}
                endDate={props.endDate || ''}
                onStartDateChange={(v) => props.setStartDate?.(v)}
                onEndDateChange={(v) => props.setEndDate?.(v)}
                showPresets
                size="sm"
              />
              <Show when={props.startDate || props.endDate}>
                <button
                  type="button"
                  onClick={() => {
                    props.setStartDate?.('')
                    props.setEndDate?.('')
                  }}
                  class="px-2 py-1.5 text-xs text-gray-400 hover:text-white transition"
                  title="날짜 필터 초기화"
                >
                  ✕
                </button>
              </Show>
            </div>
          </Show>
        </div>

        {/* 뷰 모드 전환 버튼 */}
        <div class="flex gap-1 bg-gray-800 rounded-lg p-1">
          <button
            type="button"
            onClick={() => setViewMode('table')}
            class={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm transition-colors ${
              viewMode() === 'table'
                ? 'bg-blue-500/20 text-blue-400'
                : 'text-gray-400 hover:text-gray-300'
            }`}
          >
            <List class="w-4 h-4" />
            테이블
          </button>
          <button
            type="button"
            onClick={() => setViewMode('timeline')}
            class={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm transition-colors ${
              viewMode() === 'timeline'
                ? 'bg-blue-500/20 text-blue-400'
                : 'text-gray-400 hover:text-gray-300'
            }`}
          >
            <Clock class="w-4 h-4" />
            타임라인
          </button>
        </div>
      </div>

      {/* 테이블 뷰 */}
      <Show when={viewMode() === 'table'}>
        <div class="overflow-x-auto">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-b border-gray-700">
                <th class="text-left py-3 px-4 font-medium text-gray-400">시간</th>
                <th class="text-left py-3 px-4 font-medium text-gray-400">종목</th>
                <th class="text-center py-3 px-4 font-medium text-gray-400">매매</th>
                <th class="text-right py-3 px-4 font-medium text-gray-400">수량</th>
                <th class="text-right py-3 px-4 font-medium text-gray-400">가격</th>
                <th class="text-right py-3 px-4 font-medium text-gray-400">거래대금</th>
                <th class="text-right py-3 px-4 font-medium text-gray-400">수수료</th>
                <th class="text-right py-3 px-4 font-medium text-gray-400">실현손익</th>
                <th class="text-left py-3 px-4 font-medium text-gray-400 min-w-48">메모</th>
              </tr>
            </thead>
            <tbody>
              <For each={props.executions}>
                {(exec: JournalExecution) => (
                  <tr class="border-b border-gray-800 hover:bg-gray-800/50">
                    <td class="py-3 px-4 text-gray-400 text-xs">
                      {formatDateTime(exec.executed_at || (exec as unknown as Record<string, string>).timestamp || '')}
                    </td>
                    <td class="py-3 px-4">
                      <SymbolDisplay
                        ticker={exec.symbol}
                        symbolName={exec.symbol_name}
                        exchange={exec.exchange}
                        mode="full"
                        size="sm"
                        autoFetch={true}
                      />
                    </td>
                    <td class="text-center py-3 px-4">
                      <span
                        class={`px-2 py-1 rounded text-xs font-medium ${
                          exec.side === 'buy'
                            ? 'bg-green-500/20 text-green-400'
                            : 'bg-red-500/20 text-red-400'
                        }`}
                      >
                        {exec.side === 'buy' ? '매수' : '매도'}
                      </span>
                    </td>
                    <td class="text-right py-3 px-4 text-gray-300">
                      {formatQuantity(exec.quantity)}
                    </td>
                    <td class="text-right py-3 px-4 text-gray-300">
                      {formatCurrency(exec.price)}
                    </td>
                    <td class="text-right py-3 px-4 text-gray-300">
                      {formatCurrency(exec.notional_value)}
                    </td>
                    <td class="text-right py-3 px-4 text-gray-500">
                      {exec.fee ? formatCurrency(exec.fee) : '-'}
                    </td>
                    <td class={`text-right py-3 px-4 font-medium ${getPnLColor(exec.realized_pnl)}`}>
                      {exec.realized_pnl ? formatCurrency(exec.realized_pnl) : '-'}
                    </td>
                    <td class="py-3 px-4 min-w-48">
                      <Show when={!props.isBacktest && editingMemoId() === exec.id} fallback={
                        <div class="flex items-center gap-2 group">
                          <Show when={exec.memo}>
                            <span class="text-xs text-gray-400 truncate max-w-32" title={exec.memo || ''}>
                              {exec.memo}
                            </span>
                          </Show>
                          <Show when={exec.tags && exec.tags.length > 0}>
                            <div class="flex gap-1">
                              <For each={exec.tags}>
                                {(tag: string) => (
                                  <span class="px-1.5 py-0.5 bg-blue-500/20 text-blue-400 text-xs rounded">
                                    {tag}
                                  </span>
                                )}
                              </For>
                            </div>
                          </Show>
                          {/* 백테스트 모드에서는 메모 편집 버튼 숨김 */}
                          <Show when={!props.isBacktest}>
                            <button
                              onClick={() => startEditingMemo(exec.id, exec.memo)}
                              class="opacity-0 group-hover:opacity-100 p-1 hover:bg-gray-700 rounded transition-opacity"
                              title="메모 편집"
                            >
                              <Edit3 class="w-3 h-3 text-gray-400" />
                            </button>
                          </Show>
                        </div>
                      }>
                        <div class="flex items-center gap-2">
                          <input
                            type="text"
                            value={editingMemoText()}
                            onInput={(e) => setEditingMemoText(e.currentTarget.value)}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') saveMemo(exec.id)
                              if (e.key === 'Escape') cancelEditingMemo()
                            }}
                            class="flex-1 px-2 py-1 text-xs bg-gray-700 border border-gray-600 rounded text-white focus:outline-none focus:border-blue-500"
                            placeholder="메모 입력..."
                            autofocus
                          />
                          <button
                            onClick={() => saveMemo(exec.id)}
                            disabled={isSavingMemo()}
                            class="p-1 hover:bg-green-600/20 rounded text-green-400 disabled:opacity-50"
                            title="저장"
                          >
                            <Check class="w-4 h-4" />
                          </button>
                          <button
                            onClick={cancelEditingMemo}
                            class="p-1 hover:bg-red-600/20 rounded text-red-400"
                            title="취소"
                          >
                            <X class="w-4 h-4" />
                          </button>
                        </div>
                      </Show>
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </div>
      </Show>

      {/* 타임라인 뷰 */}
      <Show when={viewMode() === 'timeline'}>
        <div class="space-y-6">
          <For each={groupedByDate()}>
            {([date, executions]) => (
              <div>
                {/* 날짜 헤더 */}
                <div class="flex items-center gap-3 mb-3">
                  <div class="text-sm font-semibold text-gray-300">
                    {formatDate(date)}
                  </div>
                  <div class="flex-1 h-px bg-gray-700" />
                  <div class="text-xs text-gray-500">
                    {executions.length}건
                  </div>
                </div>

                {/* 타임라인 아이템 */}
                <div class="relative pl-6 space-y-4">
                  {/* 타임라인 세로선 */}
                  <div class="absolute left-2 top-2 bottom-2 w-px bg-gray-700" />

                  <For each={executions}>
                    {(exec: JournalExecution) => (
                      <div class="relative">
                        {/* 타임라인 포인트 */}
                        <div
                          class={`absolute -left-4 top-3 w-3 h-3 rounded-full border-2 ${
                            exec.side === 'buy'
                              ? 'bg-green-500/30 border-green-500'
                              : 'bg-red-500/30 border-red-500'
                          }`}
                        />

                        {/* 카드 */}
                        <div class="bg-gray-800/50 rounded-lg p-4 border border-gray-700/50 hover:border-gray-600/50 transition-colors">
                          <div class="flex items-start justify-between gap-4">
                            {/* 왼쪽: 종목 정보 */}
                            <div class="flex-1">
                              <div class="flex items-center gap-3 mb-2">
                                <span
                                  class={`px-2 py-0.5 rounded text-xs font-medium ${
                                    exec.side === 'buy'
                                      ? 'bg-green-500/20 text-green-400'
                                      : 'bg-red-500/20 text-red-400'
                                  }`}
                                >
                                  {exec.side === 'buy' ? '매수' : '매도'}
                                </span>
                                <SymbolDisplay
                                  ticker={exec.symbol}
                                  symbolName={exec.symbol_name}
                                  exchange={exec.exchange}
                                  mode="full"
                                  size="sm"
                                  autoFetch={true}
                                />
                                <span class="text-xs text-gray-500">
                                  {formatTime(exec.executed_at || (exec as unknown as Record<string, string>).timestamp || '')}
                                </span>
                              </div>

                              <div class="grid grid-cols-4 gap-4 text-sm">
                                <div>
                                  <div class="text-xs text-gray-500 mb-0.5">수량</div>
                                  <div class="text-gray-300">{formatQuantity(exec.quantity)}</div>
                                </div>
                                <div>
                                  <div class="text-xs text-gray-500 mb-0.5">가격</div>
                                  <div class="text-gray-300">{formatCurrency(exec.price)}</div>
                                </div>
                                <div>
                                  <div class="text-xs text-gray-500 mb-0.5">거래대금</div>
                                  <div class="text-gray-300">{formatCurrency(exec.notional_value)}</div>
                                </div>
                                <Show when={exec.realized_pnl}>
                                  <div>
                                    <div class="text-xs text-gray-500 mb-0.5">실현손익</div>
                                    <div class={`font-medium ${getPnLColor(exec.realized_pnl)}`}>
                                      {formatCurrency(exec.realized_pnl)}
                                    </div>
                                  </div>
                                </Show>
                              </div>

                              {/* 메모 & 태그 */}
                              <Show when={exec.memo || (exec.tags && exec.tags.length > 0)}>
                                <div class="mt-2 pt-2 border-t border-gray-700/50 flex items-center gap-2 flex-wrap">
                                  <Show when={exec.memo}>
                                    <span class="text-xs text-gray-400 italic">
                                      "{exec.memo}"
                                    </span>
                                  </Show>
                                  <Show when={exec.tags && exec.tags.length > 0}>
                                    <For each={exec.tags}>
                                      {(tag: string) => (
                                        <span class="px-1.5 py-0.5 bg-blue-500/20 text-blue-400 text-xs rounded">
                                          {tag}
                                        </span>
                                      )}
                                    </For>
                                  </Show>
                                </div>
                              </Show>
                            </div>
                          </div>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* 빈 상태 */}
      <Show when={props.executions.length === 0}>
        <EmptyState
          icon="📋"
          title="체결 내역이 없습니다"
          description="조건에 맞는 거래 기록이 없습니다."
        />
      </Show>

      {/* 페이지네이션 */}
      <Show when={props.total && props.total > 0 && props.onPageChange}>
        {(() => {
          const total = props.total || 0
          const pageSize = props.pageSize || 50
          const currentPage = props.currentPage || 1
          const totalPages = Math.ceil(total / pageSize)

          return (
            <div class="mt-4 flex items-center justify-between border-t border-gray-700 pt-4">
              {/* 페이지 정보 */}
              <div class="text-sm text-gray-400">
                총 {total.toLocaleString()}건 중 {((currentPage - 1) * pageSize + 1).toLocaleString()}~
                {Math.min(currentPage * pageSize, total).toLocaleString()}건 표시
              </div>

              {/* 페이지 버튼 */}
              <div class="flex items-center gap-2">
                {/* 이전 버튼 */}
                <button
                  type="button"
                  onClick={() => props.onPageChange?.(currentPage - 1)}
                  disabled={currentPage <= 1}
                  class="flex items-center gap-1 px-3 py-1.5 bg-gray-800 border border-gray-700 rounded-lg text-sm text-gray-300 hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed transition"
                >
                  <ChevronLeft class="w-4 h-4" />
                  이전
                </button>

                {/* 페이지 번호 */}
                <div class="flex items-center gap-1">
                  <For each={getPageNumbers(currentPage, totalPages)}>
                    {(pageNum) => (
                      <Show
                        when={pageNum !== '...'}
                        fallback={<span class="px-2 text-gray-500">...</span>}
                      >
                        <button
                          type="button"
                          onClick={() => props.onPageChange?.(Number(pageNum))}
                          class={`w-8 h-8 rounded-lg text-sm transition ${
                            currentPage === Number(pageNum)
                              ? 'bg-blue-500 text-white'
                              : 'bg-gray-800 text-gray-300 hover:bg-gray-700'
                          }`}
                        >
                          {pageNum}
                        </button>
                      </Show>
                    )}
                  </For>
                </div>

                {/* 다음 버튼 */}
                <button
                  type="button"
                  onClick={() => props.onPageChange?.(currentPage + 1)}
                  disabled={currentPage >= totalPages}
                  class="flex items-center gap-1 px-3 py-1.5 bg-gray-800 border border-gray-700 rounded-lg text-sm text-gray-300 hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed transition"
                >
                  다음
                  <ChevronRight class="w-4 h-4" />
                </button>
              </div>
            </div>
          )
        })()}
      </Show>
    </div>
  )
}

// 페이지 번호 배열 생성 헬퍼
function getPageNumbers(currentPage: number, totalPages: number): (number | string)[] {
  const pages: (number | string)[] = []

  if (totalPages <= 7) {
    for (let i = 1; i <= totalPages; i++) pages.push(i)
  } else {
    pages.push(1)
    if (currentPage > 3) pages.push('...')
    for (let i = Math.max(2, currentPage - 1); i <= Math.min(totalPages - 1, currentPage + 1); i++) {
      pages.push(i)
    }
    if (currentPage < totalPages - 2) pages.push('...')
    pages.push(totalPages)
  }

  return pages
}

export default ExecutionsTable
