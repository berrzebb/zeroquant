import { createSignal, createResource, Show, For, createMemo } from 'solid-js'
import {
  TrendingUp,
  TrendingDown,
  Calendar,
  Database,
  TestTube,
  RefreshCw,
  BarChart3,
  Activity,
} from 'lucide-solid'
import { EquityCurve } from './EquityCurve'
import { DrawdownChart } from './DrawdownChart'
import { MetricsChart } from './MetricsChart'
import type { EquityDataPoint, ChartSyncState } from './EquityCurve'
import type { DrawdownDataPoint } from './DrawdownChart'
import type { MetricDataPoint } from './MetricsChart'
import {
  getEquityCurve,
  getPerformance,
  getMonthlyReturns,
  getCagrChart,
  getMddChart,
  getDrawdownChart,
  getBacktestResults,
} from '../../api/client'
import type { BacktestResult } from '../../api/client'
import { createLogger } from '../../utils/logger'
import { formatCurrency, formatPercent } from '../../utils/format'

const { error: logError } = createLogger('AnalyticsDashboard')

// 기간 옵션
const PERIOD_OPTIONS = [
  { value: '1w', label: '1주' },
  { value: '1m', label: '1개월' },
  { value: '3m', label: '3개월' },
  { value: '6m', label: '6개월' },
  { value: '1y', label: '1년' },
  { value: 'all', label: '전체' },
]

// 데이터 소스 타입
type DataSource = 'portfolio' | 'backtest'

interface AnalyticsDashboardProps {
  defaultPeriod?: string
  defaultSource?: DataSource
  backtestId?: string
  compact?: boolean
}

// 월별 수익률 히트맵 컴포넌트
function MonthlyReturnsHeatmap(props: { data: { year: number; month: number; returnPct: string; intensity: number }[] }) {
  const months = ['1월', '2월', '3월', '4월', '5월', '6월', '7월', '8월', '9월', '10월', '11월', '12월']

  const years = createMemo(() => {
    const yearsSet = new Set(props.data.map((d) => d.year))
    return Array.from(yearsSet).sort((a, b) => a - b)
  })

  const getCell = (year: number, month: number) => {
    return props.data.find((d) => d.year === year && d.month === month)
  }

  const getCellColor = (returnPct: number) => {
    if (returnPct > 10) return 'bg-green-600'
    if (returnPct > 5) return 'bg-green-500'
    if (returnPct > 2) return 'bg-green-400'
    if (returnPct > 0) return 'bg-green-300'
    if (returnPct === 0) return 'bg-gray-600'
    if (returnPct > -2) return 'bg-red-300'
    if (returnPct > -5) return 'bg-red-400'
    if (returnPct > -10) return 'bg-red-500'
    return 'bg-red-600'
  }

  return (
    <div class="overflow-x-auto">
      <table class="w-full text-xs">
        <thead>
          <tr>
            <th class="p-1 text-left text-[var(--color-text-muted)]">연도</th>
            <For each={months}>
              {(month) => (
                <th class="p-1 text-center text-[var(--color-text-muted)]">{month}</th>
              )}
            </For>
            <th class="p-1 text-center text-[var(--color-text-muted)]">합계</th>
          </tr>
        </thead>
        <tbody>
          <For each={years()}>
            {(year) => {
              const yearData = props.data.filter((d) => d.year === year)
              const yearTotal = yearData.reduce((sum, d) => sum + parseFloat(d.returnPct), 0)
              return (
                <tr>
                  <td class="p-1 font-medium text-[var(--color-text)]">{year}</td>
                  <For each={Array.from({ length: 12 }, (_, i) => i + 1)}>
                    {(month) => {
                      const cell = getCell(year, month)
                      const returnPct = cell ? parseFloat(cell.returnPct) : 0
                      return (
                        <td class="p-1">
                          <Show
                            when={cell}
                            fallback={<div class="w-full h-6 bg-gray-700 rounded" />}
                          >
                            <div
                              class={`w-full h-6 rounded flex items-center justify-center ${getCellColor(returnPct)}`}
                              title={`${year}년 ${month}월: ${formatPercent(returnPct)}`}
                            >
                              <span class="text-white font-medium">
                                {formatPercent(returnPct, false).replace('%', '')}
                              </span>
                            </div>
                          </Show>
                        </td>
                      )
                    }}
                  </For>
                  <td class="p-1">
                    <div
                      class={`w-full h-6 rounded flex items-center justify-center font-medium ${
                        yearTotal >= 0 ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400'
                      }`}
                    >
                      {formatPercent(yearTotal)}
                    </div>
                  </td>
                </tr>
              )
            }}
          </For>
        </tbody>
      </table>
    </div>
  )
}

export function AnalyticsDashboard(props: AnalyticsDashboardProps) {
  const [period, setPeriod] = createSignal(props.defaultPeriod || '3m')
  const [dataSource, setDataSource] = createSignal<DataSource>(props.defaultSource || 'portfolio')
  const [selectedBacktestId, setSelectedBacktestId] = createSignal(props.backtestId || '')
  const [chartSync, setChartSync] = createSignal<ChartSyncState | null>(null)

  // 포트폴리오 데이터 조회
  const [equityCurve, { refetch: refetchEquity }] = createResource(
    () => ({ period: period(), source: dataSource() }),
    async ({ period, source }) => {
      if (source !== 'portfolio') return null
      try {
        return await getEquityCurve(period)
      } catch (e) {
        logError('Equity curve error:', e)
        return null
      }
    }
  )

  const [performance, { refetch: refetchPerformance }] = createResource(
    () => ({ period: period(), source: dataSource() }),
    async ({ period, source }) => {
      if (source !== 'portfolio') return null
      try {
        return await getPerformance(period)
      } catch (e) {
        logError('Performance error:', e)
        return null
      }
    }
  )

  const [monthlyReturns] = createResource(
    () => dataSource(),
    async (source) => {
      if (source !== 'portfolio') return null
      try {
        return await getMonthlyReturns()
      } catch (e) {
        logError('Monthly returns error:', e)
        return null
      }
    }
  )

  const [drawdownData] = createResource(
    () => ({ period: period(), source: dataSource() }),
    async ({ period, source }) => {
      if (source !== 'portfolio') return null
      try {
        return await getDrawdownChart(period)
      } catch (e) {
        logError('Drawdown error:', e)
        return null
      }
    }
  )

  // 백테스트 결과
  const [backtestResults] = createResource(async () => {
    try {
      return await getBacktestResults()
    } catch {
      return []
    }
  })

  const selectedBacktest = () => {
    if (dataSource() !== 'backtest') return null
    const results = backtestResults() || []
    const id = selectedBacktestId()
    return results.find((r: BacktestResult) => r.id === id) || results[0]
  }

  // 차트 데이터 변환
  const equityChartData = (): EquityDataPoint[] => {
    if (dataSource() === 'portfolio') {
      const data = equityCurve()
      if (!data?.data) return []
      return data.data.map((p) => ({ time: p.x, value: parseFloat(p.y) }))
    } else {
      const bt = selectedBacktest()
      if (!bt?.equity_curve) return []
      return bt.equity_curve.map((p) => ({ time: p.timestamp, value: parseFloat(p.equity) }))
    }
  }

  const drawdownChartData = (): DrawdownDataPoint[] => {
    if (dataSource() === 'portfolio') {
      const data = drawdownData()
      if (!data?.data) return []
      return data.data.map((p) => ({ time: p.x, value: -Math.abs(parseFloat(p.y)) }))
    } else {
      const bt = selectedBacktest()
      if (!bt?.equity_curve) return []
      return bt.equity_curve.map((p) => ({
        time: p.timestamp,
        value: -Math.abs(parseFloat(p.drawdown_pct)),
      }))
    }
  }

  const monthlyData = () => {
    if (dataSource() === 'portfolio') {
      return monthlyReturns()?.data || []
    }
    // 백테스트에서는 월별 수익률 직접 계산 (간소화)
    return []
  }

  const metrics = () => {
    if (dataSource() === 'portfolio') {
      const perf = performance()
      if (!perf) return null
      return {
        currentEquity: parseFloat(perf.current_equity || perf.currentEquity || '0'),
        initialCapital: parseFloat(perf.initial_capital || perf.initialCapital || '0'),
        totalPnl: parseFloat(perf.total_pnl || perf.totalPnl || '0'),
        totalReturn: parseFloat(perf.total_return_pct || perf.totalReturnPct || '0'),
        maxDrawdown: parseFloat(perf.max_drawdown_pct || perf.maxDrawdownPct || '0'),
        cagr: parseFloat(perf.cagr_pct || perf.cagrPct || '0'),
        periodDays: perf.period_days || perf.periodDays || 0,
      }
    } else {
      const bt = selectedBacktest()
      if (!bt?.metrics) return null
      const lastEquity = bt.equity_curve?.[bt.equity_curve.length - 1]?.equity || '0'
      return {
        currentEquity: parseFloat(lastEquity),
        initialCapital: parseFloat(bt.config_summary?.initial_capital || '10000000'),
        totalPnl: parseFloat(bt.metrics.net_profit),
        totalReturn: parseFloat(bt.metrics.total_return_pct),
        maxDrawdown: parseFloat(bt.metrics.max_drawdown_pct),
        cagr: parseFloat(bt.metrics.annualized_return_pct),
        periodDays: bt.equity_curve?.length || 0,
      }
    }
  }

  const handleRefresh = () => {
    refetchEquity()
    refetchPerformance()
  }

  const isLoading = () =>
    (dataSource() === 'portfolio' && (equityCurve.loading || performance.loading)) ||
    (dataSource() === 'backtest' && backtestResults.loading)

  return (
    <div class="space-y-6">
      {/* Header Controls */}
      <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-4">
        <div class="flex items-center justify-between flex-wrap gap-4">
          <div class="flex items-center gap-4">
            <h2 class="text-xl font-bold text-[var(--color-text)]">포트폴리오 분석</h2>

            {/* Data Source Toggle */}
            <div class="flex items-center gap-1 bg-[var(--color-surface-light)] rounded-lg p-0.5">
              <button
                onClick={() => setDataSource('portfolio')}
                class={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                  dataSource() === 'portfolio'
                    ? 'bg-[var(--color-primary)] text-white'
                    : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                }`}
              >
                <Database class="w-4 h-4" />
                실제 데이터
              </button>
              <button
                onClick={() => setDataSource('backtest')}
                class={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                  dataSource() === 'backtest'
                    ? 'bg-[var(--color-primary)] text-white'
                    : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                }`}
              >
                <TestTube class="w-4 h-4" />
                백테스트
              </button>
            </div>
          </div>

          <div class="flex items-center gap-3">
            {/* Backtest Selector */}
            <Show when={dataSource() === 'backtest' && (backtestResults()?.length || 0) > 0}>
              <select
                value={selectedBacktestId()}
                onChange={(e) => setSelectedBacktestId(e.currentTarget.value)}
                class="bg-[var(--color-surface-light)] text-[var(--color-text)] border-none rounded-lg px-3 py-1.5 text-sm"
              >
                <For each={backtestResults()}>
                  {(result: BacktestResult) => (
                    <option value={result.id}>
                      {result.strategy_id} - {result.symbol}
                    </option>
                  )}
                </For>
              </select>
            </Show>

            {/* Period Selector */}
            <Show when={dataSource() === 'portfolio'}>
              <div class="flex items-center gap-1 bg-[var(--color-surface-light)] rounded-lg p-0.5">
                <For each={PERIOD_OPTIONS}>
                  {(opt) => (
                    <button
                      onClick={() => setPeriod(opt.value)}
                      class={`px-2.5 py-1 rounded text-sm transition-colors ${
                        period() === opt.value
                          ? 'bg-[var(--color-surface)] text-[var(--color-text)]'
                          : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                      }`}
                    >
                      {opt.label}
                    </button>
                  )}
                </For>
              </div>

              <button
                onClick={handleRefresh}
                disabled={isLoading()}
                class="p-2 rounded-lg hover:bg-[var(--color-surface-light)] transition-colors"
              >
                <RefreshCw class={`w-4 h-4 text-[var(--color-text-muted)] ${isLoading() ? 'animate-spin' : ''}`} />
              </button>
            </Show>
          </div>
        </div>
      </div>

      {/* Metrics Summary Cards */}
      <Show when={metrics()}>
        <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
          <div class="bg-[var(--color-surface)] rounded-xl p-4 border border-[var(--color-surface-light)]">
            <div class="text-xs text-[var(--color-text-muted)] mb-1">현재 자산</div>
            <div class="text-xl font-bold text-[var(--color-text)]">
              {formatCurrency(metrics()!.currentEquity)}
            </div>
          </div>
          <div class="bg-[var(--color-surface)] rounded-xl p-4 border border-[var(--color-surface-light)]">
            <div class="text-xs text-[var(--color-text-muted)] mb-1">초기 자본</div>
            <div class="text-xl font-bold text-[var(--color-text)]">
              {formatCurrency(metrics()!.initialCapital)}
            </div>
          </div>
          <div class="bg-[var(--color-surface)] rounded-xl p-4 border border-[var(--color-surface-light)]">
            <div class="text-xs text-[var(--color-text-muted)] mb-1">총 손익</div>
            <div class={`text-xl font-bold ${metrics()!.totalPnl >= 0 ? 'text-green-500' : 'text-red-500'}`}>
              {formatCurrency(metrics()!.totalPnl)}
            </div>
          </div>
          <div class="bg-[var(--color-surface)] rounded-xl p-4 border border-[var(--color-surface-light)]">
            <div class="text-xs text-[var(--color-text-muted)] mb-1">총 수익률</div>
            <div class={`text-xl font-bold flex items-center gap-1 ${
              metrics()!.totalReturn >= 0 ? 'text-green-500' : 'text-red-500'
            }`}>
              {metrics()!.totalReturn >= 0 ? <TrendingUp class="w-5 h-5" /> : <TrendingDown class="w-5 h-5" />}
              {formatPercent(metrics()!.totalReturn)}
            </div>
          </div>
          <div class="bg-[var(--color-surface)] rounded-xl p-4 border border-[var(--color-surface-light)]">
            <div class="text-xs text-[var(--color-text-muted)] mb-1">MDD</div>
            <div class="text-xl font-bold text-red-500">
              -{Math.abs(metrics()!.maxDrawdown).toFixed(2)}%
            </div>
          </div>
          <div class="bg-[var(--color-surface)] rounded-xl p-4 border border-[var(--color-surface-light)]">
            <div class="text-xs text-[var(--color-text-muted)] mb-1">CAGR</div>
            <div class={`text-xl font-bold ${metrics()!.cagr >= 0 ? 'text-green-500' : 'text-red-500'}`}>
              {formatPercent(metrics()!.cagr)}
            </div>
          </div>
        </div>
      </Show>

      {/* Charts Grid */}
      <div class={`grid gap-6 ${props.compact ? 'grid-cols-1' : 'grid-cols-1 lg:grid-cols-2'}`}>
        {/* Equity Curve */}
        <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-4">
          <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4 flex items-center gap-2">
            <Activity class="w-5 h-5 text-[var(--color-primary)]" />
            자산 곡선
          </h3>
          <Show
            when={equityChartData().length > 0}
            fallback={
              <div class="h-[250px] flex items-center justify-center text-[var(--color-text-muted)]">
                <Show when={isLoading()} fallback={<span>데이터 없음</span>}>
                  <RefreshCw class="w-5 h-5 animate-spin mr-2" />
                  로딩 중...
                </Show>
              </div>
            }
          >
            <EquityCurve
              data={equityChartData()}
              height={250}
              chartId="analytics-equity"
              syncState={chartSync}
              onVisibleRangeChange={setChartSync}
              colors={{
                equityColor: dataSource() === 'portfolio' ? '#3b82f6' : '#10b981',
                positiveArea: dataSource() === 'portfolio' ? 'rgba(59, 130, 246, 0.2)' : 'rgba(16, 185, 129, 0.2)',
              }}
            />
          </Show>
        </div>

        {/* Drawdown Chart */}
        <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-4">
          <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4 flex items-center gap-2">
            <TrendingDown class="w-5 h-5 text-red-500" />
            Drawdown
          </h3>
          <Show
            when={drawdownChartData().length > 0}
            fallback={
              <div class="h-[250px] flex items-center justify-center text-[var(--color-text-muted)]">
                <Show when={isLoading()} fallback={<span>데이터 없음</span>}>
                  <RefreshCw class="w-5 h-5 animate-spin mr-2" />
                  로딩 중...
                </Show>
              </div>
            }
          >
            <DrawdownChart
              data={drawdownChartData()}
              height={250}
              chartId="analytics-drawdown"
              syncState={chartSync}
              onVisibleRangeChange={setChartSync}
            />
          </Show>
        </div>
      </div>

      {/* Monthly Returns Heatmap */}
      <Show when={dataSource() === 'portfolio' && monthlyData().length > 0}>
        <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-4">
          <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4 flex items-center gap-2">
            <BarChart3 class="w-5 h-5 text-[var(--color-primary)]" />
            월별 수익률
          </h3>
          <MonthlyReturnsHeatmap data={monthlyData()} />
        </div>
      </Show>

      {/* Backtest Trade Summary */}
      <Show when={dataSource() === 'backtest' && selectedBacktest()?.metrics}>
        <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-4">
          <h3 class="text-lg font-semibold text-[var(--color-text)] mb-4 flex items-center gap-2">
            <BarChart3 class="w-5 h-5 text-[var(--color-primary)]" />
            백테스트 상세 지표
          </h3>
          <div class="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
            <div>
              <div class="text-[var(--color-text-muted)]">총 거래 수</div>
              <div class="text-lg font-semibold text-[var(--color-text)]">
                {selectedBacktest()?.metrics.total_trades}
              </div>
            </div>
            <div>
              <div class="text-[var(--color-text-muted)]">승률</div>
              <div class="text-lg font-semibold text-green-500">
                {selectedBacktest()?.metrics.win_rate_pct}%
              </div>
            </div>
            <div>
              <div class="text-[var(--color-text-muted)]">Profit Factor</div>
              <div class="text-lg font-semibold text-[var(--color-text)]">
                {selectedBacktest()?.metrics.profit_factor}
              </div>
            </div>
            <div>
              <div class="text-[var(--color-text-muted)]">Sharpe Ratio</div>
              <div class="text-lg font-semibold text-[var(--color-text)]">
                {selectedBacktest()?.metrics.sharpe_ratio}
              </div>
            </div>
            <div>
              <div class="text-[var(--color-text-muted)]">평균 수익</div>
              <div class="text-lg font-semibold text-green-500">
                {formatCurrency(selectedBacktest()?.metrics.avg_win || '0')}
              </div>
            </div>
            <div>
              <div class="text-[var(--color-text-muted)]">평균 손실</div>
              <div class="text-lg font-semibold text-red-500">
                {formatCurrency(selectedBacktest()?.metrics.avg_loss || '0')}
              </div>
            </div>
            <div>
              <div class="text-[var(--color-text-muted)]">최대 수익</div>
              <div class="text-lg font-semibold text-green-500">
                {formatCurrency(selectedBacktest()?.metrics.largest_win || '0')}
              </div>
            </div>
            <div>
              <div class="text-[var(--color-text-muted)]">최대 손실</div>
              <div class="text-lg font-semibold text-red-500">
                {formatCurrency(selectedBacktest()?.metrics.largest_loss || '0')}
              </div>
            </div>
          </div>
        </div>
      </Show>

      {/* Data Source Info */}
      <div class="text-center text-xs text-[var(--color-text-muted)]">
        <span class={`inline-block w-2 h-2 rounded-full mr-1 ${
          dataSource() === 'portfolio' ? 'bg-blue-500' : 'bg-green-500'
        }`} />
        {dataSource() === 'portfolio'
          ? `실제 포트폴리오 데이터 (${equityChartData().length}개 포인트)`
          : `백테스트 시뮬레이션: ${selectedBacktest()?.strategy_id} - ${selectedBacktest()?.symbol}`
        }
      </div>
    </div>
  )
}
