/**
 * Paper Trading 컴포넌트
 *
 * 전략 기반 Paper Trading UI - Backtest와 동일한 구조로 실시간 시뮬레이션
 *
 * 주요 기능:
 * - 전략 선택 및 Paper Trading 시작/중지
 * - 실시간 포지션 및 체결 내역 표시
 * - Mock 계정 선택 기능
 */
import { createSignal, createResource, For, Show, createEffect } from 'solid-js'
import {
  Play,
  Square,
  RotateCcw,
  RefreshCw,
  Wallet,
  TrendingUp,
  TrendingDown,
} from 'lucide-solid'
import {
  Card,
  CardHeader,
  CardContent,
  StatCard,
  StatCardGrid,
  EmptyState,
  Button,
} from '../ui'
import { SymbolDisplay } from '../SymbolDisplay'
import {
  getStrategies,
  getPaperTradingAccounts,
  listPaperTradingSessions,
  getPaperTradingStatus,
  startPaperTrading,
  stopPaperTrading,
  resetPaperTrading,
  getStrategyPaperTradingPositions,
  getStrategyPaperTradingTrades,
  type PaperTradingSession,
  type PaperTradingPosition,
  type PaperTradingExecution,
  type PaperTradingAccount,
} from '../../api/client'
import type { Strategy } from '../../types'
import { createLogger } from '../../utils/logger'
import { formatCurrency, formatNumber } from '../../utils/format'

const { error: logError } = createLogger('PaperTrading')

const formatDecimal = (value: string | number, decimals = 2) =>
  formatNumber(value, { decimals, useGrouping: false })

export function PaperTrading() {
  // 상태 관리
  const [selectedStrategyId, setSelectedStrategyId] = createSignal<string | null>(null)
  const [status, setStatus] = createSignal<PaperTradingSession | null>(null)
  const [positions, setPositions] = createSignal<PaperTradingPosition[]>([])
  const [executions, setExecutions] = createSignal<PaperTradingExecution[]>([])
  const [isLoading, setIsLoading] = createSignal(false)
  const [error, setError] = createSignal<string | null>(null)

  // 시작 모달 상태
  const [showStartModal, setShowStartModal] = createSignal(false)
  const [selectedAccountId, setSelectedAccountId] = createSignal<string>('')
  const [initialBalance, setInitialBalance] = createSignal('10000000')

  // 전략 목록 로드
  const [strategies] = createResource(async () => {
    try {
      return await getStrategies()
    } catch {
      return [] as Strategy[]
    }
  })

  // Mock 계정 목록 로드
  const [accounts] = createResource(async () => {
    try {
      const response = await getPaperTradingAccounts()
      return response.accounts
    } catch {
      return [] as PaperTradingAccount[]
    }
  })

  // Paper Trading 세션 목록 (실행 중인 전략들)
  const [sessions, { refetch: refetchSessions }] = createResource(async () => {
    try {
      const response = await listPaperTradingSessions()
      return response.sessions
    } catch {
      return [] as PaperTradingSession[]
    }
  })

  // 전략의 Paper Trading 상태 찾기
  const getSessionForStrategy = (strategyId: string): PaperTradingSession | undefined => {
    return sessions()?.find(s => s.strategyId === strategyId)
  }

  // 전략별 상태 로드
  const loadStrategyDetails = async (strategyId: string) => {
    setIsLoading(true)
    setError(null)
    try {
      const [statusData, positionsData, tradesData] = await Promise.all([
        getPaperTradingStatus(strategyId),
        getStrategyPaperTradingPositions(strategyId),
        getStrategyPaperTradingTrades(strategyId),
      ])
      setStatus(statusData)
      setPositions(positionsData.positions)
      setExecutions(tradesData.executions)
    } catch (err) {
      logError('전략 상태 로드 실패:', err)
      setError('전략 정보를 불러오는데 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // 전략 선택 시 상세 로드
  createEffect(() => {
    const strategyId = selectedStrategyId()
    if (strategyId) {
      loadStrategyDetails(strategyId)
    }
  })

  // 자동 새로고침 (실행 중일 때 5초마다)
  // SolidJS createEffect의 반환값을 이용한 cleanup 패턴
  createEffect((prevInterval: ReturnType<typeof setInterval> | undefined) => {
    // 이전 interval 정리 (effect 재실행 시)
    if (prevInterval) {
      clearInterval(prevInterval)
    }

    const currentStatus = status()
    const isRunning = currentStatus?.status === 'running'
    const strategyId = selectedStrategyId()

    if (isRunning && strategyId) {
      // 새 interval 생성 및 반환 (다음 effect 실행 시 정리됨)
      return setInterval(() => {
        loadStrategyDetails(strategyId)
      }, 5000)
    }

    return undefined
  })

  // 컴포넌트 언마운트 시 추가 정리는 effect 내부에서 처리됨

  // Paper Trading 시작
  const handleStart = async () => {
    const strategyId = selectedStrategyId()
    const accountId = selectedAccountId()
    if (!strategyId || !accountId) return

    setIsLoading(true)
    setError(null)
    try {
      await startPaperTrading(strategyId, {
        credentialId: accountId,
        initialBalance: parseInt(initialBalance(), 10),
      })
      setShowStartModal(false)
      await loadStrategyDetails(strategyId)
      await refetchSessions()
    } catch (err) {
      logError('Paper Trading 시작 실패:', err)
      setError('Paper Trading 시작에 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // Paper Trading 중지
  const handleStop = async () => {
    const strategyId = selectedStrategyId()
    if (!strategyId) return

    setIsLoading(true)
    try {
      await stopPaperTrading(strategyId)
      await loadStrategyDetails(strategyId)
      await refetchSessions()
    } catch (err) {
      logError('Paper Trading 중지 실패:', err)
      setError('Paper Trading 중지에 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // Paper Trading 리셋
  const handleReset = async () => {
    const strategyId = selectedStrategyId()
    if (!strategyId) return

    if (!confirm('정말 이 전략의 Paper Trading 기록을 초기화하시겠습니까?')) {
      return
    }

    setIsLoading(true)
    try {
      await resetPaperTrading(strategyId)
      await loadStrategyDetails(strategyId)
      await refetchSessions()
    } catch (err) {
      logError('Paper Trading 리셋 실패:', err)
      setError('Paper Trading 리셋에 실패했습니다')
    } finally {
      setIsLoading(false)
    }
  }

  // 전략 선택 핸들러
  const handleStrategySelect = (strategyId: string) => {
    setSelectedStrategyId(strategyId)
    // 계정 자동 선택 (전략에 연결된 계정 또는 첫 번째 계정)
    const strategy = strategies()?.find(s => s.id === strategyId)
    if (strategy?.credentialId) {
      setSelectedAccountId(strategy.credentialId)
    } else if (accounts()?.length) {
      setSelectedAccountId(accounts()![0].id)
    }
  }

  // 시작 모달 열기
  const openStartModal = () => {
    if (!accounts()?.length) {
      setError('Mock 계정이 없습니다. Settings에서 Mock 거래소를 먼저 등록하세요.')
      return
    }
    setShowStartModal(true)
  }

  // 상태 계산
  const isRunning = () => status()?.status === 'running'
  const isStopped = () => !status() || status()?.status === 'stopped'
  const totalPnl = () => {
    const s = status()
    if (!s) return 0
    return parseFloat(s.realizedPnl) + parseFloat(s.unrealizedPnl)
  }

  return (
    <div class="space-y-6">
      {/* 전략 선택 및 컨트롤 */}
      <Card>
        <CardHeader>
          <div class="flex items-center justify-between">
            <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
              <Wallet class="w-5 h-5" />
              Paper Trading
            </h3>
            <div class="flex gap-2">
              <Button
                variant="secondary"
                size="sm"
                onClick={() => {
                  refetchSessions()
                  if (selectedStrategyId()) {
                    loadStrategyDetails(selectedStrategyId()!)
                  }
                }}
                disabled={isLoading()}
              >
                <RefreshCw class={`w-4 h-4 ${isLoading() ? 'animate-spin' : ''}`} />
                새로고침
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div class="flex flex-wrap items-center gap-4">
            {/* 전략 선택 */}
            <div class="flex-1 min-w-[200px]">
              <label class="block text-sm text-[var(--color-text-muted)] mb-1">전략 선택</label>
              <select
                value={selectedStrategyId() || ''}
                onChange={(e) => handleStrategySelect(e.currentTarget.value)}
                class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
              >
                <option value="">전략을 선택하세요...</option>
                <For each={strategies()}>
                  {(strategy) => {
                    const session = getSessionForStrategy(strategy.id)
                    return (
                      <option value={strategy.id}>
                        {strategy.name} ({strategy.strategyType})
                        {session?.status === 'running' && ' 🟢'}
                        {session?.status === 'stopped' && session.tradeCount > 0 && ' ⏹️'}
                      </option>
                    )
                  }}
                </For>
              </select>
            </div>

            {/* 상태 표시 */}
            <Show when={status()}>
              <div class={`px-3 py-1 rounded-full text-sm font-medium ${
                isRunning()
                  ? 'bg-green-500/20 text-green-400'
                  : 'bg-gray-500/20 text-gray-400'
              }`}>
                {isRunning() ? '실행 중' : '중지됨'}
              </div>
            </Show>

            {/* 컨트롤 버튼 */}
            <div class="flex items-center gap-2">
              <Show when={isStopped() && selectedStrategyId()}>
                <Button
                  variant="primary"
                  onClick={openStartModal}
                  disabled={isLoading() || !selectedStrategyId()}
                >
                  <Play class="w-4 h-4 mr-1" />
                  시작
                </Button>
              </Show>

              <Show when={isRunning()}>
                <Button
                  variant="destructive"
                  onClick={handleStop}
                  disabled={isLoading()}
                >
                  <Square class="w-4 h-4 mr-1" />
                  중지
                </Button>
              </Show>

              <Show when={status() && status()!.tradeCount > 0}>
                <Button
                  variant="secondary"
                  onClick={handleReset}
                  disabled={isLoading() || isRunning()}
                >
                  <RotateCcw class="w-4 h-4 mr-1" />
                  리셋
                </Button>
              </Show>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* 시작 모달 */}
      <Show when={showStartModal()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
          <div class="absolute inset-0 bg-black/50" onClick={() => setShowStartModal(false)} />
          <div class="relative bg-[var(--color-surface)] rounded-xl p-6 w-full max-w-md">
            <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4">
              Paper Trading 시작
            </h3>

            <div class="space-y-4">
              {/* 계정 선택 */}
              <div>
                <label class="block text-sm text-[var(--color-text-muted)] mb-1">
                  Mock 계정 선택
                </label>
                <select
                  value={selectedAccountId()}
                  onChange={(e) => setSelectedAccountId(e.currentTarget.value)}
                  class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)]"
                >
                  <For each={accounts()}>
                    {(account) => (
                      <option value={account.id}>
                        {account.name} ({formatCurrency(account.initialBalance)})
                      </option>
                    )}
                  </For>
                </select>
              </div>

              {/* 초기 자본 */}
              <div>
                <label class="block text-sm text-[var(--color-text-muted)] mb-1">
                  초기 자본
                </label>
                <input
                  type="number"
                  value={initialBalance()}
                  onInput={(e) => setInitialBalance(e.currentTarget.value)}
                  class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)]"
                />
              </div>

              {/* 버튼 */}
              <div class="flex justify-end gap-2 mt-6">
                <Button
                  variant="secondary"
                  onClick={() => setShowStartModal(false)}
                >
                  취소
                </Button>
                <Button
                  variant="primary"
                  onClick={handleStart}
                  disabled={isLoading() || !selectedAccountId()}
                >
                  <Play class="w-4 h-4 mr-1" />
                  시작
                </Button>
              </div>
            </div>
          </div>
        </div>
      </Show>

      {/* 에러 표시 */}
      <Show when={error()}>
        <div class="p-4 bg-red-500/10 border border-red-500/30 rounded-lg text-red-400">
          {error()}
        </div>
      </Show>

      {/* 전략 미선택 시 안내 */}
      <Show when={!selectedStrategyId()}>
        <EmptyState
          icon="🎯"
          title="전략을 선택하세요"
          description="위에서 Paper Trading을 실행할 전략을 선택하세요"
        />
      </Show>

      {/* 선택된 전략 상세 */}
      <Show when={selectedStrategyId() && status()}>
        {/* 통계 카드 */}
        <StatCardGrid columns={4}>
          <StatCard
            label="초기 자본"
            value={formatCurrency(status()!.initialBalance)}
            icon="💰"
          />
          <StatCard
            label="현재 잔고"
            value={formatCurrency(status()!.currentBalance)}
            icon="🏦"
          />
          <StatCard
            label="총 손익"
            value={`${totalPnl() >= 0 ? '+' : ''}${formatCurrency(totalPnl())}`}
            icon={totalPnl() >= 0 ? '📈' : '📉'}
            valueColor={totalPnl() >= 0 ? 'text-green-500' : 'text-red-500'}
          />
          <StatCard
            label="수익률"
            value={`${parseFloat(status()!.returnPct) >= 0 ? '+' : ''}${formatDecimal(status()!.returnPct)}%`}
            icon={parseFloat(status()!.returnPct) >= 0 ? '🚀' : '⬇️'}
            valueColor={parseFloat(status()!.returnPct) >= 0 ? 'text-green-500' : 'text-red-500'}
          />
        </StatCardGrid>

        {/* 추가 통계 */}
        <StatCardGrid columns={4}>
          <StatCard
            label="실현 손익"
            value={formatCurrency(status()!.realizedPnl)}
            icon="💵"
            valueColor={parseFloat(status()!.realizedPnl) >= 0 ? 'text-green-500' : 'text-red-500'}
          />
          <StatCard
            label="미실현 손익"
            value={formatCurrency(status()!.unrealizedPnl)}
            icon="📊"
            valueColor={parseFloat(status()!.unrealizedPnl) >= 0 ? 'text-green-500' : 'text-red-500'}
          />
          <StatCard
            label="포지션 수"
            value={`${status()!.positionCount}개`}
            icon="📦"
          />
          <StatCard
            label="거래 수"
            value={`${status()!.tradeCount}건`}
            icon="📋"
          />
        </StatCardGrid>

        {/* 포지션 & 체결 */}
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* 포지션 */}
          <Card>
            <CardHeader>
              <h3 class="text-lg font-semibold text-[var(--color-text)]">
                보유 포지션 ({positions().length})
              </h3>
            </CardHeader>
            <CardContent>
              <Show
                when={positions().length > 0}
                fallback={
                  <EmptyState
                    icon="📦"
                    title="포지션 없음"
                    description="Paper Trading을 시작하면 포지션이 표시됩니다"
                    className="py-4"
                  />
                }
              >
                <div class="space-y-3">
                  <For each={positions()}>
                    {(position) => {
                      const pnl = parseFloat(position.unrealizedPnl)
                      const pnlPct = parseFloat(position.returnPct)
                      return (
                        <div class="flex items-center justify-between p-3 bg-[var(--color-surface-light)] rounded-lg">
                          <div>
                            <div class="flex items-center gap-2">
                              <SymbolDisplay
                                ticker={position.symbol}
                                mode="inline"
                                size="md"
                                autoFetch={true}
                                class="font-semibold"
                              />
                              <span
                                class={`px-2 py-0.5 text-xs rounded ${
                                  position.side === 'Long'
                                    ? 'bg-green-500/20 text-green-400'
                                    : 'bg-red-500/20 text-red-400'
                                }`}
                              >
                                {position.side}
                              </span>
                            </div>
                            <div class="text-sm text-[var(--color-text-muted)] mt-1">
                              {formatDecimal(position.quantity, 4)} @ {formatCurrency(position.entryPrice)}
                            </div>
                          </div>
                          <div class="text-right">
                            <div class={`font-semibold ${pnl >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                              {pnl >= 0 ? '+' : ''}{formatCurrency(pnl)}
                            </div>
                            <div class={`text-sm ${pnlPct >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                              {pnlPct >= 0 ? '+' : ''}{formatDecimal(pnlPct)}%
                            </div>
                          </div>
                        </div>
                      )
                    }}
                  </For>
                </div>
              </Show>
            </CardContent>
          </Card>

          {/* 체결 내역 */}
          <Card>
            <CardHeader>
              <h3 class="text-lg font-semibold text-[var(--color-text)]">
                최근 체결 ({executions().length})
              </h3>
            </CardHeader>
            <CardContent>
              <Show
                when={executions().length > 0}
                fallback={
                  <EmptyState
                    icon="📋"
                    title="체결 내역 없음"
                    description="아직 체결된 거래가 없습니다"
                    className="py-4"
                  />
                }
              >
                <div class="space-y-2 max-h-80 overflow-y-auto">
                  <For each={executions().slice(0, 20)}>
                    {(exec) => {
                      const realizedPnl = exec.realizedPnl ? parseFloat(exec.realizedPnl) : null
                      return (
                        <div class="flex items-center justify-between p-3 bg-[var(--color-surface-light)] rounded-lg">
                          <div class="flex items-center gap-3">
                            <span class="text-sm text-[var(--color-text-muted)] font-mono">
                              {new Date(exec.executedAt).toLocaleString('ko-KR', {
                                month: '2-digit',
                                day: '2-digit',
                                hour: '2-digit',
                                minute: '2-digit'
                              })}
                            </span>
                            <span
                              class={`px-2 py-0.5 text-xs rounded font-medium ${
                                exec.side === 'Buy'
                                  ? 'bg-green-500/20 text-green-400'
                                  : 'bg-red-500/20 text-red-400'
                              }`}
                            >
                              {exec.side === 'Buy' ? '매수' : '매도'}
                            </span>
                            <SymbolDisplay
                              ticker={exec.symbol}
                              mode="inline"
                              size="sm"
                              autoFetch={true}
                            />
                          </div>
                          <div class="text-right">
                            <div class="text-sm text-[var(--color-text)]">
                              {formatDecimal(exec.quantity, 4)} @ {formatCurrency(exec.price)}
                            </div>
                            <Show when={realizedPnl !== null}>
                              <div class={`text-sm ${realizedPnl! >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                                {realizedPnl! >= 0 ? '+' : ''}{formatCurrency(realizedPnl!)}
                              </div>
                            </Show>
                          </div>
                        </div>
                      )
                    }}
                  </For>
                </div>
              </Show>
            </CardContent>
          </Card>
        </div>

        {/* 실행 중인 경우 실시간 업데이트 안내 */}
        <Show when={isRunning()}>
          <div class="text-center text-sm text-[var(--color-text-muted)]">
            🟢 Paper Trading 실행 중 - 5초마다 자동 업데이트
          </div>
        </Show>
      </Show>
    </div>
  )
}

export default PaperTrading
