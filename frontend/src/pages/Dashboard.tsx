import { onMount, For, Show, createResource, lazy, Suspense } from 'solid-js'
import { createStore } from 'solid-js/store'
import {
  TrendingUp,
  TrendingDown,
  DollarSign,
  Activity,
  BarChart3,
  ArrowUpRight,
  ArrowDownRight,
  RefreshCw,
  AlertCircle,
  Bell,
  Bot,
  Play,
  Pause,
  Building2,
  Settings,
} from 'lucide-solid'
import { PageLoader, ErrorState, StatCard, StatCardGrid, Card, CardHeader, CardContent, EmptyState } from '../components/ui'
import { RankingWidget } from '../components/ranking'
import { createWebSocket } from '../hooks/createWebSocket'
import { getPortfolioSummary, getHoldings, getStrategies, getActiveAccount, getMarketOverview } from '../api/client'
import type { MarketOverviewResponse } from '../api/client'
// EChart 미사용 컴포넌트는 동기 import
import { PortfolioEquityChart, MarketBreadthWidget, SectorTreemap } from '../components/charts'
import type { RegimeData, MarketRegime } from '../components/charts'

// EChart 기반 컴포넌트는 lazy loading (번들 사이즈 최적화)
const FearGreedGauge = lazy(() =>
  import('../components/charts/FearGreedGauge').then(m => ({ default: m.FearGreedGauge }))
)
const SectorMomentumBar = lazy(() =>
  import('../components/charts/SectorMomentumBar').then(m => ({ default: m.SectorMomentumBar }))
)
const RegimeSummaryTable = lazy(() =>
  import('../components/charts/RegimeSummaryTable').then(m => ({ default: m.RegimeSummaryTable }))
)
import type { WsOrderUpdate, WsPositionUpdate, WsActiveAccountChanged, Strategy } from '../types'
import type { HoldingInfo, ActiveAccount } from '../api/client'
import { SymbolDisplay } from '../components/SymbolDisplay'
import { formatCurrency, formatPercent } from '../utils/format'

// Dashboard 상태 타입
interface UIState {
  isRefreshing: boolean
  showNotifications: boolean
}

interface NotificationState {
  orderUpdates: WsOrderUpdate[]
  positionUpdates: WsPositionUpdate[]
}

const initialUIState: UIState = {
  isRefreshing: false,
  showNotifications: false,
}

const initialNotificationState: NotificationState = {
  orderUpdates: [],
  positionUpdates: [],
}

export function Dashboard() {
  // 활성 계정 조회
  const [activeAccount, { refetch: refetchActiveAccount }] = createResource(async () => {
    try {
      return await getActiveAccount()
    } catch {
      return { credential_id: null, exchange_id: null, display_name: null, is_testnet: false } as ActiveAccount
    }
  })

  // API 데이터 로딩 - 활성 계정의 credential_id를 사용
  const [portfolio, { refetch: refetchPortfolio }] = createResource(
    () => activeAccount()?.credential_id,
    async (credentialId) => {
      return getPortfolioSummary(credentialId || undefined)
    }
  )
  const [holdings, { refetch: refetchHoldings }] = createResource(
    () => activeAccount()?.credential_id,
    async (credentialId) => {
      return getHoldings(credentialId || undefined)
    }
  )
  const [strategies, { refetch: refetchStrategies }] = createResource(getStrategies)

  // 시장 통합 조회 (status + breadth + macro)
  const [marketOverview, { refetch: refetchMarketOverview }] = createResource(async () => {
    try {
      return await getMarketOverview()
    } catch {
      return null
    }
  })

  // Store 기반 상태 관리
  const [ui, setUI] = createStore<UIState>({ ...initialUIState })
  const [notifications, setNotifications] = createStore<NotificationState>({ ...initialNotificationState })

  const { isConnected, subscribeChannels } = createWebSocket({
    onOrderUpdate: (order) => {
      setNotifications('orderUpdates', updates => [order, ...updates].slice(0, 10))
      refetchHoldings()
    },
    onPositionUpdate: (position) => {
      setNotifications('positionUpdates', updates => [position, ...updates].slice(0, 10))
      refetchPortfolio()
      refetchHoldings()
    },
    onActiveAccountChanged: (_data: WsActiveAccountChanged) => {
      // 활성 계정이 변경되면 모든 데이터를 갱신
      console.log('[Dashboard] Active account changed, refetching data...')
      refetchActiveAccount()
      refetchPortfolio()
      refetchHoldings()
      refetchStrategies()
      refetchMarketOverview()
    },
  })

  onMount(() => {
    // orders, positions, account 채널 구독
    subscribeChannels(['orders', 'positions', 'account'])
  })

  // 데이터 새로고침
  const handleRefresh = async () => {
    setUI('isRefreshing', true)
    try {
      await Promise.all([refetchActiveAccount(), refetchPortfolio(), refetchHoldings(), refetchStrategies(), refetchMarketOverview()])
    } finally {
      setUI('isRefreshing', false)
    }
  }

  // 실행 중인 전략 필터링
  const runningStrategies = () => {
    const all = strategies() || []
    return all.filter((s: Strategy) => s.status === 'running')
  }

  // 포지션 데이터 변환
  const positions = () => {
    const h = holdings()
    if (!h) return []

    const allHoldings = h.holdings || []
    return allHoldings.map((holding: HoldingInfo, index: number) => ({
      id: `${holding.market}-${index}`,
      symbol: holding.symbol,
      symbolName: holding.displayName || holding.name || null,
      side: 'Long' as const,
      quantity: parseFloat(holding.quantity) || 0,
      entryPrice: parseFloat(holding.avgPrice) || 0,
      currentPrice: parseFloat(holding.currentPrice) || 0,
      unrealizedPnl: parseFloat(holding.profitLoss) || 0,
      unrealizedPnlPercent: parseFloat(holding.profitLossRate) || 0,
      market: holding.market,
    }))
  }

  return (
    <div class="space-y-6">
      {/* Header with Connection Status & Refresh */}
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-4">
          {/* Connection Status */}
          <div class="flex items-center gap-2 text-sm">
            <div
              class={`w-2 h-2 rounded-full ${
                isConnected() ? 'bg-green-500' : 'bg-red-500'
              }`}
            />
            <span class="text-[var(--color-text-muted)]">
              {isConnected() ? '실시간 연결됨' : '연결 끊김'}
            </span>
          </div>

          {/* Market Status */}
          <Show when={marketOverview()}>
            {(overview) => (
              <>
                <div class="flex items-center gap-2 text-sm">
                  <span class={`px-2 py-0.5 rounded ${overview().kr.isOpen ? 'bg-green-500/20 text-green-400' : 'bg-gray-500/20 text-gray-400'}`}>
                    KR {overview().kr.isOpen ? '개장' : '폐장'}
                  </span>
                </div>
                <div class="flex items-center gap-2 text-sm">
                  <span class={`px-2 py-0.5 rounded ${overview().us.isOpen ? 'bg-green-500/20 text-green-400' : 'bg-gray-500/20 text-gray-400'}`}>
                    US {overview().us.isOpen ? (overview().us.session || '개장') : '폐장'}
                  </span>
                </div>
              </>
            )}
          </Show>

          {/* Active Account Display */}
          <Show when={activeAccount()}>
            <div class="flex items-center gap-2 text-sm border-l border-[var(--color-surface-light)] pl-4">
              <Building2 class="w-4 h-4 text-[var(--color-text-muted)]" />
              <Show
                when={activeAccount()?.credential_id}
                fallback={
                  <a href="/settings" class="text-[var(--color-text-muted)] hover:text-[var(--color-primary)] transition-colors flex items-center gap-1">
                    <span>계정 선택 안됨</span>
                    <Settings class="w-3 h-3" />
                  </a>
                }
              >
                <span class="text-[var(--color-text)]">{activeAccount()?.display_name}</span>
                <Show when={activeAccount()?.is_testnet}>
                  <span class="px-1.5 py-0.5 text-xs rounded bg-yellow-500/20 text-yellow-500">
                    모의
                  </span>
                </Show>
              </Show>
            </div>
          </Show>
        </div>

        <div class="flex items-center gap-2">
          {/* Notifications Button */}
          <div class="relative">
            <button
              onClick={() => setUI('showNotifications', !ui.showNotifications)}
              class="relative flex items-center gap-2 px-3 py-1.5 rounded-lg bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
            >
              <Bell class="w-4 h-4" />
              <Show when={notifications.orderUpdates.length > 0 || notifications.positionUpdates.length > 0}>
                <span class="absolute -top-1 -right-1 w-4 h-4 bg-red-500 rounded-full text-xs text-white flex items-center justify-center">
                  {notifications.orderUpdates.length + notifications.positionUpdates.length}
                </span>
              </Show>
            </button>

            {/* Notifications Dropdown */}
            <Show when={ui.showNotifications}>
              <div class="absolute right-0 top-full mt-2 w-80 bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] shadow-xl z-50 max-h-96 overflow-y-auto">
                <div class="p-3 border-b border-[var(--color-surface-light)]">
                  <h4 class="text-sm font-semibold text-[var(--color-text)]">실시간 알림</h4>
                </div>

                <Show when={notifications.orderUpdates.length > 0}>
                  <div class="p-2">
                    <div class="text-xs text-[var(--color-text-muted)] px-2 mb-1">주문 업데이트</div>
                    <For each={notifications.orderUpdates.slice(0, 5)}>
                      {(order) => (
                        <div class="p-2 rounded-lg hover:bg-[var(--color-surface-light)] transition-colors">
                          <div class="flex items-center justify-between">
                            <SymbolDisplay
                              ticker={order.symbol}
                              mode="inline"
                              size="sm"
                              autoFetch={true}
                              class="text-sm font-medium"
                            />
                            <span class={`text-xs px-1.5 py-0.5 rounded ${
                              order.status === 'filled' ? 'bg-green-500/20 text-green-400' :
                              order.status === 'cancelled' ? 'bg-red-500/20 text-red-400' :
                              'bg-yellow-500/20 text-yellow-400'
                            }`}>
                              {order.status}
                            </span>
                          </div>
                          <div class="text-xs text-[var(--color-text-muted)] mt-1">
                            {order.side === 'buy' ? '매수' : '매도'} {order.filled_quantity}/{order.quantity}
                            {order.price && ` @ ${order.price}`}
                          </div>
                        </div>
                      )}
                    </For>
                  </div>
                </Show>

                <Show when={notifications.positionUpdates.length > 0}>
                  <div class="p-2 border-t border-[var(--color-surface-light)]">
                    <div class="text-xs text-[var(--color-text-muted)] px-2 mb-1">포지션 업데이트</div>
                    <For each={notifications.positionUpdates.slice(0, 5)}>
                      {(position) => (
                        <div class="p-2 rounded-lg hover:bg-[var(--color-surface-light)] transition-colors">
                          <div class="flex items-center justify-between">
                            <SymbolDisplay
                              ticker={position.symbol}
                              mode="inline"
                              size="sm"
                              autoFetch={true}
                              class="text-sm font-medium"
                            />
                            <span class={`text-xs ${parseFloat(position.unrealized_pnl) >= 0 ? 'text-green-400' : 'text-red-400'}`}>
                              {parseFloat(position.return_pct) >= 0 ? '+' : ''}{position.return_pct}%
                            </span>
                          </div>
                          <div class="text-xs text-[var(--color-text-muted)] mt-1">
                            {position.side === 'long' ? '롱' : '숏'} {position.quantity} @ {position.current_price}
                          </div>
                        </div>
                      )}
                    </For>
                  </div>
                </Show>

                <Show when={notifications.orderUpdates.length === 0 && notifications.positionUpdates.length === 0}>
                  <div class="p-4 text-center text-[var(--color-text-muted)] text-sm">
                    새로운 알림이 없습니다.
                  </div>
                </Show>
              </div>
            </Show>
          </div>

          {/* Refresh Button */}
          <button
            onClick={handleRefresh}
            disabled={ui.isRefreshing}
            class="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors disabled:opacity-50"
          >
            <RefreshCw class={`w-4 h-4 ${ui.isRefreshing ? 'animate-spin' : ''}`} />
            새로고침
          </button>
        </div>
      </div>

      {/* 로딩 상태 - 공통 컴포넌트 사용 */}
      <Show when={portfolio.loading && !portfolio()}>
        <PageLoader message="포트폴리오 데이터를 불러오는 중..." />
      </Show>

      {/* 에러 상태 - 공통 컴포넌트 사용 */}
      <Show when={portfolio.error}>
        <ErrorState
          title="데이터 로드 실패"
          message="포트폴리오 데이터를 불러오는데 실패했습니다."
          onRetry={handleRefresh}
        />
      </Show>

      {/* Portfolio Summary Cards */}
      <Show when={!portfolio.loading && !portfolio.error}>
        {/* Testnet/Mock Account Banner */}
        <Show when={activeAccount()?.is_testnet}>
          <div class="flex items-center gap-2 p-3 bg-yellow-500/10 border border-yellow-500/30 rounded-lg">
            <AlertCircle class="w-5 h-5 text-yellow-500" />
            <span class="text-yellow-500 text-sm font-medium">
              모의투자 계정의 자산 정보입니다. 실제 자산과 다를 수 있습니다.
            </span>
          </div>
        </Show>

        {/* No Account Selected Info */}
        <Show when={!activeAccount()?.credential_id}>
          <div class="flex items-center justify-between p-4 bg-[var(--color-surface-light)] rounded-lg">
            <div class="flex items-center gap-2">
              <Building2 class="w-5 h-5 text-[var(--color-text-muted)]" />
              <span class="text-[var(--color-text-muted)]">
                거래소 계정이 선택되지 않았습니다. 샘플 데이터를 표시합니다.
              </span>
            </div>
            <a
              href="/settings"
              class="px-3 py-1.5 bg-[var(--color-primary)] text-white rounded-lg text-sm font-medium hover:bg-[var(--color-primary)]/90 transition-colors flex items-center gap-2"
            >
              <Settings class="w-4 h-4" />
              계정 설정
            </a>
          </div>
        </Show>

        {/* 포트폴리오 요약 카드 - 공통 StatCard 사용 */}
        <StatCardGrid columns={4}>
          <StatCard
            label="총 자산"
            value={formatCurrency(portfolio()?.totalValue || 0)}
            icon="💰"
            trend={(portfolio()?.totalPnlPercent || 0) >= 0 ? 'up' : 'down'}
            trendValue={formatPercent(portfolio()?.totalPnlPercent || 0)}
          />
          <StatCard
            label="일일 손익"
            value={`${(portfolio()?.dailyPnl || 0) >= 0 ? '+' : ''}${formatCurrency(portfolio()?.dailyPnl || 0)}`}
            icon="📈"
            valueColor={(portfolio()?.dailyPnl || 0) >= 0 ? 'text-green-500' : 'text-red-500'}
            trend={(portfolio()?.dailyPnlPercent || 0) >= 0 ? 'up' : 'down'}
            trendValue={formatPercent(portfolio()?.dailyPnlPercent || 0)}
          />
          <StatCard
            label="총 손익"
            value={`${(portfolio()?.totalPnl || 0) >= 0 ? '+' : ''}${formatCurrency(portfolio()?.totalPnl || 0)}`}
            icon="📊"
            valueColor={(portfolio()?.totalPnl || 0) >= 0 ? 'text-green-500' : 'text-red-500'}
          />
          <StatCard
            label="현금 잔고"
            value={formatCurrency(portfolio()?.cashBalance || 0)}
            icon="💵"
          />
        </StatCardGrid>

        {/* 시장 심리 지표 섹션 */}
        <div class="grid grid-cols-1 lg:grid-cols-3 gap-4">
          {/* Fear & Greed 게이지 (lazy loaded) */}
          <Card padding="md">
            <div class="text-sm text-[var(--color-text-muted)] mb-3">시장 심리 지수</div>
            <Suspense fallback={<div class="h-32 flex items-center justify-center text-[var(--color-text-muted)]">로딩 중...</div>}>
              <FearGreedGauge size="md" showChange />
            </Suspense>
          </Card>

          {/* Market Breadth 위젯 */}
          <div class="lg:col-span-2">
            <MarketBreadthWidget data={marketOverview()?.breadth ?? undefined} />
          </div>
        </div>

        {/* 시장 레짐 분석 섹션 */}
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {/* 레짐 요약 테이블 (lazy loaded) */}
          <Suspense fallback={<Card padding="md"><div class="h-48 flex items-center justify-center text-[var(--color-text-muted)]">로딩 중...</div></Card>}>
            <RegimeSummaryTable
              regimes={[
                { regime: 'Bull', days: 45, avgReturn: 2.3, volatility: 15.2, maxDrawdown: -8.5 },
                { regime: 'Sideways', days: 30, avgReturn: 0.5, volatility: 12.1, maxDrawdown: -5.2 },
                { regime: 'Bear', days: 15, avgReturn: -1.8, volatility: 22.4, maxDrawdown: -15.3 },
              ]}
              currentRegime="Sideways"
              title="시장 레짐 분석"
            />
          </Suspense>

          {/* 섹터 모멘텀 (lazy loaded) */}
          <Suspense fallback={<Card padding="md"><div class="h-48 flex items-center justify-center text-[var(--color-text-muted)]">로딩 중...</div></Card>}>
            <SectorMomentumBar
              sectors={[
                { name: '반도체', return5d: 5.2, symbolCount: 45 },
                { name: '2차전지', return5d: 3.8, symbolCount: 32 },
                { name: '바이오', return5d: 2.1, symbolCount: 58 },
                { name: '금융', return5d: -0.5, symbolCount: 41 },
                { name: '철강', return5d: -2.3, symbolCount: 23 },
              ]}
              title="섹터 모멘텀 (5일)"
            />
          </Suspense>
        </div>

        {/* 섹터 트리맵 */}
        <Card padding="md">
          <div class="text-sm text-[var(--color-text-muted)] mb-3">섹터별 수익률 맵</div>
          <SectorTreemap
            market="KR"
            metric="performance"
            height={300}
            showVisualMap
          />
        </Card>
      </Show>

      {/* Equity Curve - 실제 데이터 + 백테스트 데이터 지원 */}
      <PortfolioEquityChart
        height={280}
        showControls={true}
        defaultPeriod="3m"
        defaultSource="portfolio"
        credentialId={activeAccount()?.credential_id || undefined}
      />

      {/* Main Content Grid */}
      <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Positions */}
        <div class="lg:col-span-2 bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)]">
          <div class="p-4 border-b border-[var(--color-surface-light)] flex items-center justify-between">
            <h3 class="text-lg font-semibold text-[var(--color-text)]">
              보유 포지션
            </h3>
            <Show when={holdings()}>
              <span class="text-sm text-[var(--color-text-muted)]">
                {holdings()?.totalCount || 0}개 종목
              </span>
            </Show>
          </div>

          <Show when={holdings.loading}>
            <div class="flex items-center justify-center py-8">
              <RefreshCw class="w-5 h-5 animate-spin text-[var(--color-primary)]" />
            </div>
          </Show>

          <Show when={!holdings.loading && positions().length === 0}>
            <EmptyState
              icon="📭"
              title="보유 중인 종목이 없습니다"
              description="종목을 매수하면 여기에 표시됩니다."
            />
          </Show>

          <Show when={!holdings.loading && positions().length > 0}>
            <div class="overflow-x-auto">
              <table class="w-full">
                <thead>
                  <tr class="border-b border-[var(--color-surface-light)]">
                    <th class="text-left p-4 text-sm font-medium text-[var(--color-text-muted)]">
                      종목
                    </th>
                    <th class="text-right p-4 text-sm font-medium text-[var(--color-text-muted)]">
                      수량
                    </th>
                    <th class="text-right p-4 text-sm font-medium text-[var(--color-text-muted)]">
                      매입가
                    </th>
                    <th class="text-right p-4 text-sm font-medium text-[var(--color-text-muted)]">
                      현재가
                    </th>
                    <th class="text-right p-4 text-sm font-medium text-[var(--color-text-muted)]">
                      손익
                    </th>
                  </tr>
                </thead>
                <tbody>
                  <For each={positions()}>
                    {(position) => (
                      <tr class="border-b border-[var(--color-surface-light)] hover:bg-[var(--color-surface-light)]/50 transition-colors">
                        <td class="p-4">
                          <div class="flex items-center gap-2">
                            <span
                              class={`px-2 py-0.5 text-xs rounded ${
                                position.market === 'KR'
                                  ? 'bg-blue-500/20 text-blue-400'
                                  : position.market === 'US'
                                  ? 'bg-green-500/20 text-green-400'
                                  : 'bg-orange-500/20 text-orange-400'
                              }`}
                            >
                              {position.market}
                            </span>
                            <SymbolDisplay
                              ticker={position.symbol}
                              symbolName={position.symbolName}
                              mode="inline"
                              size="sm"
                              autoFetch={!position.symbolName}
                              class="font-medium"
                            />
                          </div>
                        </td>
                        <td class="p-4 text-right text-[var(--color-text)]">
                          {position.quantity.toLocaleString()}
                        </td>
                        <td class="p-4 text-right text-[var(--color-text)]">
                          {position.market === 'KR'
                            ? formatCurrency(position.entryPrice)
                            : formatCurrency(position.entryPrice, 'USD')}
                        </td>
                        <td class="p-4 text-right text-[var(--color-text)]">
                          {position.market === 'KR'
                            ? formatCurrency(position.currentPrice)
                            : formatCurrency(position.currentPrice, 'USD')}
                        </td>
                        <td class="p-4 text-right">
                          <div
                            class={
                              position.unrealizedPnl >= 0 ? 'text-green-500' : 'text-red-500'
                            }
                          >
                            <div class="font-medium">
                              {formatPercent(position.unrealizedPnlPercent)}
                            </div>
                            <div class="text-sm">
                              {position.market === 'KR'
                                ? formatCurrency(position.unrealizedPnl)
                                : formatCurrency(position.unrealizedPnl, 'USD')}
                            </div>
                          </div>
                        </td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </div>
          </Show>
        </div>

        {/* 사이드바: 전략 + TOP 10 랭킹 */}
        <div class="space-y-6">
          {/* Running Strategies */}
          <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)]">
            <div class="p-4 border-b border-[var(--color-surface-light)] flex items-center justify-between">
              <h3 class="text-lg font-semibold text-[var(--color-text)]">
                실행 중인 전략
              </h3>
              <Show when={strategies()}>
                <span class="text-sm text-[var(--color-text-muted)]">
                  {runningStrategies().length}개 실행 중
                </span>
              </Show>
            </div>

            <Show when={strategies.loading}>
              <div class="flex items-center justify-center py-8">
                <RefreshCw class="w-5 h-5 animate-spin text-[var(--color-primary)]" />
              </div>
            </Show>

            <Show when={!strategies.loading && runningStrategies().length === 0}>
              <EmptyState
                icon="🤖"
                title="실행 중인 전략이 없습니다"
                description="전략을 시작하면 여기에 표시됩니다."
                action={
                  <a href="/strategies" class="text-[var(--color-primary)] hover:underline">
                    전략 시작하기
                  </a>
                }
              />
            </Show>

            <Show when={!strategies.loading && runningStrategies().length > 0}>
              <div class="divide-y divide-[var(--color-surface-light)]">
                <For each={runningStrategies()}>
                  {(strategy: Strategy) => (
                    <div class="p-4 hover:bg-[var(--color-surface-light)]/50 transition-colors">
                      <div class="flex items-center justify-between">
                        <div class="flex items-center gap-3">
                          <div class="p-2 rounded-lg bg-green-500/20">
                            <Play class="w-4 h-4 text-green-400" />
                          </div>
                          <div>
                            <div class="font-medium text-[var(--color-text)]">{strategy.name}</div>
                            <div class="text-sm text-[var(--color-text-muted)] flex flex-wrap gap-1">
                              <Show when={strategy.symbols && strategy.symbols.length > 0} fallback={<span>심볼 없음</span>}>
                                <For each={strategy.symbols}>
                                  {(symbol) => (
                                    <SymbolDisplay
                                      ticker={symbol}
                                      mode="inline"
                                      size="sm"
                                      autoFetch={true}
                                    />
                                  )}
                                </For>
                              </Show>
                            </div>
                          </div>
                        </div>
                        <Show when={strategy.metrics}>
                          <div class="text-right">
                            <div class={`font-medium ${(strategy.metrics?.totalPnlPercent || 0) >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                              {formatPercent(strategy.metrics?.totalPnlPercent || 0)}
                            </div>
                            <div class="text-xs text-[var(--color-text-muted)]">
                              {strategy.metrics?.tradeCount || 0}회 거래
                            </div>
                          </div>
                        </Show>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>

          {/* TOP 10 Ranking Widget */}
          <RankingWidget
            limit={10}
            onViewMore={() => window.location.href = '/global-ranking'}
          />
        </div>
      </div>
    </div>
  )
}

export default Dashboard
