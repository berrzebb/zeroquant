import { createResource, createMemo, For, Show } from 'solid-js'
import { createStore } from 'solid-js/store'
import { useNavigate } from '@solidjs/router'
import { Play, Pause, Settings, TrendingUp, TrendingDown, RefreshCw, X, BarChart3, Activity, Trash2, Copy, AlertCircle } from 'lucide-solid'
import { PageLoader, ErrorState, EmptyState, FilterPanel, Button } from '../components/ui'
import { getStrategies, startStrategy, stopStrategy, getStrategyMeta, deleteStrategy, cloneStrategy } from '../api/client'
import type { Strategy } from '../api/client'
import { useToast } from '../components/Toast'
import { formatCurrency, getDefaultTimeframe } from '../utils/format'
import { AddStrategyModal } from '../components/AddStrategyModal'
import { SDUIEditModal } from '../components/SDUIEditModal'
import { SymbolDisplay } from '../components/SymbolDisplay'
import { createLogger } from '../utils/logger'

const { error: logError } = createLogger('Strategies')

// ==================== 타입 정의 ====================

/** 모달 상태 타입 */
interface ModalState {
  add: { open: boolean }
  delete: { open: boolean; strategy: Strategy | null; isLoading: boolean }
  clone: { open: boolean; strategy: Strategy | null; newName: string; isLoading: boolean }
  edit: { open: boolean; strategyId: string | null; strategyType: string | null }
}

/** UI 상태 타입 */
interface UIState {
  filter: 'all' | 'running' | 'stopped'
  togglingId: string | null
}

// ==================== 초기 상태 ====================

const initialModalState: ModalState = {
  add: { open: false },
  delete: { open: false, strategy: null, isLoading: false },
  clone: { open: false, strategy: null, newName: '', isLoading: false },
  edit: { open: false, strategyId: null, strategyType: null },
}

const initialUIState: UIState = {
  filter: 'all',
  togglingId: null,
}

export function Strategies() {
  const toast = useToast()
  const navigate = useNavigate()

  // ==================== createStore 기반 상태 관리 ====================
  const [modals, setModals] = createStore<ModalState>(initialModalState)
  const [ui, setUI] = createStore<UIState>(initialUIState)

  // 전략 템플릿 목록 가져오기 (SDUI API 사용)
  const [strategyTemplates] = createResource(async () => {
    const response = await getStrategyMeta()
    return response.strategies
  })

  // 전략 목록 가져오기
  const [strategies, { refetch }] = createResource(getStrategies)

  // ==================== 파생 상태 (createMemo) ====================

  /** 필터링된 전략 목록 - 필터 변경 시에만 재계산 */
  const filteredStrategies = createMemo(() => {
    const data = strategies()
    if (!data) return []
    const f = ui.filter
    if (f === 'all') return data
    if (f === 'running') return data.filter((s) => s.status === 'Running')
    return data.filter((s) => s.status === 'Stopped' || s.status === 'Error')
  })

  // ==================== 모달 헬퍼 함수 ====================

  /** 편집 모달 열기 */
  const openEditModal = (strategy: Strategy) => {
    setModals('edit', {
      open: true,
      strategyId: strategy.id,
      strategyType: strategy.strategyType,
    })
  }

  /** 편집 모달 닫기 */
  const closeEditModal = () => {
    setModals('edit', { open: false, strategyId: null, strategyType: null })
  }

  /** 삭제 모달 열기 */
  const openDeleteModal = (strategy: Strategy) => {
    setModals('delete', { open: true, strategy, isLoading: false })
  }

  /** 삭제 확인 */
  const handleConfirmDelete = async () => {
    const strategy = modals.delete.strategy
    if (!strategy) return

    setModals('delete', 'isLoading', true)
    try {
      await deleteStrategy(strategy.id)
      toast.success('전략 삭제 완료', `"${strategy.name}" 전략이 삭제되었습니다`)
      setModals('delete', { open: false, strategy: null, isLoading: false })
      refetch()
    } catch (error) {
      logError('Failed to delete strategy:', error)
      const errorMsg = error instanceof Error ? error.message : '전략 삭제에 실패했습니다'
      toast.error('전략 삭제 실패', errorMsg)
      setModals('delete', 'isLoading', false)
    }
  }

  /** 삭제 모달 닫기 */
  const closeDeleteModal = () => {
    setModals('delete', { open: false, strategy: null, isLoading: false })
  }

  /** 복제 모달 열기 */
  const openCloneModal = (strategy: Strategy) => {
    setModals('clone', {
      open: true,
      strategy,
      newName: `${strategy.name} (복사본)`,
      isLoading: false,
    })
  }

  /** 복제 확인 */
  const handleConfirmClone = async () => {
    const strategy = modals.clone.strategy
    const name = modals.clone.newName.trim()
    if (!strategy || !name) return

    setModals('clone', 'isLoading', true)
    try {
      const result = await cloneStrategy(strategy.id, name)
      toast.success('전략 복제 완료', `"${result.name || name}" 전략이 생성되었습니다`)
      setModals('clone', { open: false, strategy: null, newName: '', isLoading: false })
      refetch()
    } catch (error) {
      logError('Failed to clone strategy:', error)
      const errorMsg = error instanceof Error ? error.message : '전략 복제에 실패했습니다'
      toast.error('전략 복제 실패', errorMsg)
      setModals('clone', 'isLoading', false)
    }
  }

  /** 복제 모달 닫기 */
  const closeCloneModal = () => {
    setModals('clone', { open: false, strategy: null, newName: '', isLoading: false })
  }

  /** 추가 모달 열기/닫기 */
  const openAddModal = () => setModals('add', 'open', true)
  const closeAddModal = () => setModals('add', 'open', false)

  // ==================== 전략 토글 ====================

  const toggleStrategy = async (strategy: Strategy) => {
    setUI('togglingId', strategy.id)
    const isRunning = strategy.status === 'Running'
    try {
      if (isRunning) {
        await stopStrategy(strategy.id)
        toast.info('전략 중지됨', `"${strategy.name}" 전략이 중지되었습니다`)
      } else {
        await startStrategy(strategy.id)
        toast.success('전략 시작됨', `"${strategy.name}" 전략이 실행되었습니다`)
      }
      // 목록 새로고침
      refetch()
    } catch (error) {
      logError('Failed to toggle strategy:', error)
      const errorMsg = error instanceof Error ? error.message : '전략 상태 변경에 실패했습니다'
      toast.error(isRunning ? '전략 중지 실패' : '전략 시작 실패', errorMsg)
    } finally {
      setUI('togglingId', null)
    }
  }

  // ==================== 파생 상태 (createMemo) - 카운트 ====================

  /** 실행 중인 전략 수 */
  const runningCount = createMemo(() =>
    strategies()?.filter((s) => s.status === 'Running').length || 0
  )

  /** 중지된 전략 수 */
  const stoppedCount = createMemo(() =>
    strategies()?.filter((s) => s.status !== 'Running').length || 0
  )

  return (
    <div class="space-y-6">
      {/* 헤더 - 필터 버튼 + 액션 버튼 */}
      <FilterPanel>
        <div class="flex items-center justify-between w-full">
          {/* 필터 버튼 그룹 */}
          <div class="flex gap-2">
            <Button
              variant={ui.filter === 'all' ? 'primary' : 'secondary'}
              onClick={() => setUI('filter', 'all')}
            >
              전체 ({strategies()?.length || 0})
            </Button>
            <Button
              variant={ui.filter === 'running' ? 'primary' : 'secondary'}
              onClick={() => setUI('filter', 'running')}
            >
              🟢 실행 중 ({runningCount()})
            </Button>
            <Button
              variant={ui.filter === 'stopped' ? 'primary' : 'secondary'}
              onClick={() => setUI('filter', 'stopped')}
            >
              ⏸️ 중지됨 ({stoppedCount()})
            </Button>
          </div>

          {/* 액션 버튼 그룹 */}
          <div class="flex gap-2">
            <Button variant="primary" onClick={openAddModal}>
              ➕ 전략 추가
            </Button>
            <Button
              variant="secondary"
              onClick={() => refetch()}
              loading={strategies.loading}
            >
              🔄 새로고침
            </Button>
          </div>
        </div>
      </FilterPanel>

      {/* 로딩 상태 - 공통 컴포넌트 사용 */}
      <Show when={strategies.loading && !strategies()}>
        <PageLoader message="전략 목록을 불러오는 중..." />
      </Show>

      {/* 에러 상태 - 공통 컴포넌트 사용 */}
      <Show when={strategies.error}>
        <ErrorState
          title="데이터 로드 실패"
          message="전략 목록을 불러오는데 실패했습니다"
          onRetry={() => refetch()}
        />
      </Show>

      {/* 빈 상태 - 공통 컴포넌트 사용 */}
      <Show when={!strategies.loading && !strategies.error && (!strategies() || strategies()?.length === 0)}>
        <EmptyState
          icon="⚙️"
          title="등록된 전략이 없습니다"
          description="새로운 전략을 추가해 자동 매매를 시작하세요"
          action={
            <Button variant="primary" onClick={openAddModal}>
              + 전략 추가
            </Button>
          }
        />
      </Show>

      {/* Strategies Grid */}
      <Show when={filteredStrategies().length > 0}>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <For each={filteredStrategies()}>
            {(strategy) => (
              <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6">
                {/* Header */}
                <div class="flex items-start justify-between mb-4">
                  <div>
                    <div class="flex items-center gap-2 mb-1">
                      <h3 class="text-lg font-semibold text-[var(--color-text)]">
                        {strategy.name}
                      </h3>
                      <span
                        class={`px-2 py-0.5 text-xs rounded ${
                          strategy.market === 'KR'
                            ? 'bg-blue-500/20 text-blue-400'
                            : strategy.market === 'US'
                            ? 'bg-green-500/20 text-green-400'
                            : 'bg-orange-500/20 text-orange-400'
                        }`}
                      >
                        {strategy.market}
                      </span>
                    </div>
                    <div class="flex items-center gap-2">
                      <div
                        class={`w-2 h-2 rounded-full ${
                          strategy.status === 'Running'
                            ? 'bg-green-500 animate-pulse'
                            : strategy.status === 'Error'
                            ? 'bg-red-500'
                            : 'bg-gray-500'
                        }`}
                      />
                      <span class="text-sm text-[var(--color-text-muted)]">
                        {strategy.status === 'Running'
                          ? '실행 중'
                          : strategy.status === 'Error'
                          ? '오류'
                          : '중지됨'}
                      </span>
                    </div>
                  </div>
                  <div class="flex gap-1">
                    <button
                      class="p-2 rounded-lg hover:bg-[var(--color-surface-light)] transition-colors disabled:opacity-50"
                      onClick={() => toggleStrategy(strategy)}
                      disabled={ui.togglingId === strategy.id}
                      title={strategy.status === 'Running' ? '전략 중지' : '전략 시작'}
                    >
                      <Show when={ui.togglingId === strategy.id}>
                        <RefreshCw class="w-5 h-5 animate-spin text-[var(--color-text-muted)]" />
                      </Show>
                      <Show when={ui.togglingId !== strategy.id}>
                        <Show
                          when={strategy.status === 'Running'}
                          fallback={<Play class="w-5 h-5 text-green-500" />}
                        >
                          <Pause class="w-5 h-5 text-yellow-500" />
                        </Show>
                      </Show>
                    </button>
                    <button
                      class="p-2 rounded-lg hover:bg-[var(--color-surface-light)] transition-colors"
                      onClick={() => openEditModal(strategy)}
                      title="전략 설정"
                    >
                      <Settings class="w-5 h-5 text-[var(--color-text-muted)]" />
                    </button>
                    <button
                      class="p-2 rounded-lg hover:bg-blue-500/10 transition-colors"
                      onClick={() => openCloneModal(strategy)}
                      title="전략 복제"
                    >
                      <Copy class="w-5 h-5 text-blue-400" />
                    </button>
                    <button
                      class="p-2 rounded-lg hover:bg-red-500/10 transition-colors"
                      onClick={() => openDeleteModal(strategy)}
                      title="전략 삭제"
                    >
                      <Trash2 class="w-5 h-5 text-red-400" />
                    </button>
                  </div>
                </div>

                {/* Symbols & Timeframe */}
                <div class="flex flex-wrap items-center gap-2 mb-4">
                  {/* 타임프레임 배지 */}
                  <span class="px-2 py-0.5 text-xs bg-[var(--color-primary)]/20 text-[var(--color-primary)] rounded font-medium">
                    {strategy.timeframe || getDefaultTimeframe(strategy.strategyType)}
                  </span>
                  {/* 심볼 목록 */}
                  <For each={strategy.symbols}>
                    {(symbol) => (
                      <div class="px-2 py-1 text-xs bg-[var(--color-surface-light)] rounded">
                        <SymbolDisplay
                          ticker={symbol}
                          mode="full"
                          size="sm"
                          autoFetch={true}
                        />
                      </div>
                    )}
                  </For>
                </div>

                {/* Stats */}
                <Show
                  when={strategy.status !== 'Error'}
                  fallback={
                    <div class="flex items-center gap-2 p-3 bg-red-500/10 rounded-lg">
                      <AlertCircle class="w-5 h-5 text-red-500" />
                      <span class="text-sm text-red-500">
                        전략 실행 중 오류가 발생했습니다
                      </span>
                    </div>
                  }
                >
                  <div class="grid grid-cols-3 gap-4">
                    <div>
                      <div class="text-sm text-[var(--color-text-muted)] mb-1">손익</div>
                      <div
                        class={`font-semibold flex items-center gap-1 ${
                          strategy.pnl >= 0 ? 'text-green-500' : 'text-red-500'
                        }`}
                      >
                        <Show
                          when={strategy.pnl >= 0}
                          fallback={<TrendingDown class="w-4 h-4" />}
                        >
                          <TrendingUp class="w-4 h-4" />
                        </Show>
                        {formatCurrency(strategy.pnl)}
                      </div>
                    </div>
                    <div>
                      <div class="text-sm text-[var(--color-text-muted)] mb-1">승률</div>
                      <div class="font-semibold text-[var(--color-text)]">
                        {strategy.winRate.toFixed(1)}%
                      </div>
                    </div>
                    <div>
                      <div class="text-sm text-[var(--color-text-muted)] mb-1">거래</div>
                      <div class="font-semibold text-[var(--color-text)]">
                        {strategy.tradesCount}회
                      </div>
                    </div>
                  </div>
                </Show>

                {/* 빠른 액션 버튼 */}
                <div class="flex gap-2 mt-4 pt-4 border-t border-[var(--color-surface-light)]">
                  <button
                    class="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-sm bg-[var(--color-surface-light)] hover:bg-[var(--color-primary)]/20 text-[var(--color-text-muted)] hover:text-[var(--color-primary)] rounded-lg transition-colors"
                    onClick={() => navigate(`/backtest?strategy=${strategy.id}`)}
                    title="이 전략으로 백테스트"
                  >
                    <BarChart3 class="w-4 h-4" />
                    백테스트
                  </button>
                  <button
                    class="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-sm bg-[var(--color-surface-light)] hover:bg-purple-500/20 text-[var(--color-text-muted)] hover:text-purple-400 rounded-lg transition-colors"
                    onClick={() => navigate(`/simulation?strategy=${strategy.id}`)}
                    title="이 전략으로 시뮬레이션"
                  >
                    <Activity class="w-4 h-4" />
                    시뮬레이션
                  </button>
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* ==================== 전략 편집 모달 (SDUI 기반) ==================== */}
      <SDUIEditModal
        open={modals.edit.open}
        strategyId={modals.edit.strategyId}
        strategyType={modals.edit.strategyType}
        onClose={closeEditModal}
        onSuccess={refetch}
      />

      {/* ==================== 전략 추가 모달 ==================== */}
      <AddStrategyModal
        open={modals.add.open}
        onClose={closeAddModal}
        onSuccess={() => refetch()}
        templates={strategyTemplates() || []}
        templatesLoading={strategyTemplates.loading}
      />

      {/* ==================== 전략 삭제 확인 모달 ==================== */}
      <Show when={modals.delete.open}>
        <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
          {/* 배경 오버레이 */}
          <div
            class="absolute inset-0 bg-black/50"
            onClick={closeDeleteModal}
          />

          {/* 모달 컨텐츠 */}
          <div class="relative w-full max-w-md bg-[var(--color-bg)] rounded-2xl shadow-2xl overflow-hidden">
            {/* 헤더 */}
            <div class="flex items-center justify-between p-6 border-b border-[var(--color-surface-light)]">
              <div class="flex items-center gap-3">
                <div class="w-10 h-10 flex items-center justify-center bg-red-500/20 rounded-full">
                  <Trash2 class="w-5 h-5 text-red-500" />
                </div>
                <div>
                  <h2 class="text-lg font-semibold text-[var(--color-text)]">
                    전략 삭제
                  </h2>
                </div>
              </div>
              <button
                onClick={closeDeleteModal}
                class="p-2 hover:bg-[var(--color-surface)] rounded-lg transition-colors"
              >
                <X class="w-5 h-5" />
              </button>
            </div>

            {/* 본문 */}
            <div class="p-6">
              <p class="text-[var(--color-text)]">
                <span class="font-semibold">"{modals.delete.strategy?.name}"</span> 전략을 삭제하시겠습니까?
              </p>
              <p class="mt-2 text-sm text-[var(--color-text-muted)]">
                이 작업은 되돌릴 수 없습니다. 전략과 관련된 모든 설정이 영구적으로 삭제됩니다.
              </p>
            </div>

            {/* 푸터 */}
            <div class="flex items-center justify-end gap-3 p-6 border-t border-[var(--color-surface-light)]">
              <button
                onClick={closeDeleteModal}
                class="px-4 py-2 text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
                disabled={modals.delete.isLoading}
              >
                취소
              </button>
              <button
                onClick={handleConfirmDelete}
                disabled={modals.delete.isLoading}
                class="px-6 py-2 bg-red-500 text-white rounded-lg font-medium hover:bg-red-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
              >
                <Show when={modals.delete.isLoading}>
                  <RefreshCw class="w-4 h-4 animate-spin" />
                </Show>
                {modals.delete.isLoading ? '삭제 중...' : '삭제'}
              </button>
            </div>
          </div>
        </div>
      </Show>

      {/* ==================== 전략 복제 모달 ==================== */}
      <Show when={modals.clone.open}>
        <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
          {/* 배경 오버레이 */}
          <div
            class="absolute inset-0 bg-black/50"
            onClick={closeCloneModal}
          />

          {/* 모달 컨텐츠 */}
          <div class="relative w-full max-w-md bg-[var(--color-bg)] rounded-2xl shadow-2xl overflow-hidden">
            {/* 헤더 */}
            <div class="flex items-center justify-between p-6 border-b border-[var(--color-surface-light)]">
              <div class="flex items-center gap-3">
                <div class="w-10 h-10 flex items-center justify-center bg-blue-500/20 rounded-full">
                  <Copy class="w-5 h-5 text-blue-500" />
                </div>
                <div>
                  <h2 class="text-lg font-semibold text-[var(--color-text)]">
                    전략 복제
                  </h2>
                </div>
              </div>
              <button
                onClick={closeCloneModal}
                class="p-2 hover:bg-[var(--color-surface)] rounded-lg transition-colors"
              >
                <X class="w-5 h-5" />
              </button>
            </div>

            {/* 본문 */}
            <div class="p-6 space-y-4">
              <p class="text-[var(--color-text-muted)]">
                <span class="font-semibold text-[var(--color-text)]">"{modals.clone.strategy?.name}"</span> 전략을 복제합니다.
                모든 설정이 새 전략으로 복사됩니다.
              </p>

              <div>
                <label class="block text-sm font-medium text-[var(--color-text)] mb-2">
                  새 전략 이름
                </label>
                <input
                  type="text"
                  value={modals.clone.newName}
                  onInput={(e) => setModals('clone', 'newName', e.currentTarget.value)}
                  placeholder="전략 이름을 입력하세요"
                  class="w-full px-4 py-2.5 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-[var(--color-text)] placeholder:text-[var(--color-text-muted)] focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]"
                />
              </div>
            </div>

            {/* 푸터 */}
            <div class="flex items-center justify-end gap-3 p-6 border-t border-[var(--color-surface-light)]">
              <button
                onClick={closeCloneModal}
                class="px-4 py-2 text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
                disabled={modals.clone.isLoading}
              >
                취소
              </button>
              <button
                onClick={handleConfirmClone}
                disabled={modals.clone.isLoading || !modals.clone.newName.trim()}
                class="px-6 py-2 bg-[var(--color-primary)] text-white rounded-lg font-medium hover:bg-[var(--color-primary)]/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
              >
                <Show when={modals.clone.isLoading}>
                  <RefreshCw class="w-4 h-4 animate-spin" />
                </Show>
                {modals.clone.isLoading ? '복제 중...' : '복제'}
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  )
}

export default Strategies
