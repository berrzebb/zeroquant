import { createSignal, createResource, createEffect, Show, For } from 'solid-js'
import { TrendingUp, TrendingDown, Calendar, Database, TestTube, RefreshCw, Download, X, CheckCircle, AlertCircle, Trash2 } from 'lucide-solid'
import { EquityCurve } from './EquityCurve'
import type { EquityDataPoint } from './EquityCurve'
import { getEquityCurve, getPerformance, getBacktestResults, listCredentials, syncJournalExecutions, clearEquityCache } from '../../api/client'
import type { EquityCurveResponse, PerformanceResponse, BacktestResult, JournalSyncResponse } from '../../api/client'
import type { ExchangeCredential } from '../../types'
import { createLogger } from '../../utils/logger'
import { formatCurrency, formatPercent } from '../../utils/format'

const { error: logError } = createLogger('PortfolioEquityChart')

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

interface PortfolioEquityChartProps {
  height?: number
  showControls?: boolean
  defaultPeriod?: string
  defaultSource?: DataSource
  backtestId?: string
  /** 활성 계정 ID - 계좌별 자산곡선 조회에 사용 */
  credentialId?: string
}

export function PortfolioEquityChart(props: PortfolioEquityChartProps) {
  const [period, setPeriod] = createSignal(props.defaultPeriod || '3m')
  const [dataSource, setDataSource] = createSignal<DataSource>(props.defaultSource || 'portfolio')
  const [selectedBacktestId, setSelectedBacktestId] = createSignal(props.backtestId || '')

  // 동기화 관련 상태
  const [showSyncModal, setShowSyncModal] = createSignal(false)
  const [syncCredentialId, setSyncCredentialId] = createSignal('')
  const [syncLoading, setSyncLoading] = createSignal(false)
  const [syncResult, setSyncResult] = createSignal<{ success: boolean; message: string } | null>(null)
  const [forceFullSync, setForceFullSync] = createSignal(false)

  // 포트폴리오 데이터 조회 (credentialId가 변경되면 자동으로 다시 조회)
  const [equityCurveData, { refetch: refetchEquity }] = createResource(
    () => ({ period: period(), source: dataSource(), credentialId: props.credentialId }),
    async ({ period, source, credentialId }) => {
      if (source !== 'portfolio') return null
      try {
        return await getEquityCurve(period, credentialId)
      } catch (e) {
        logError('Equity curve fetch error:', e)
        return null
      }
    }
  )

  // 성과 지표 조회 (credentialId가 변경되면 자동으로 다시 조회)
  const [performanceData, { refetch: refetchPerformance }] = createResource(
    () => ({ period: period(), source: dataSource(), credentialId: props.credentialId }),
    async ({ period, source, credentialId }) => {
      if (source !== 'portfolio') return null
      try {
        return await getPerformance(period, credentialId)
      } catch (e) {
        logError('Performance fetch error:', e)
        return null
      }
    }
  )

  // 백테스트 결과 목록
  const [backtestResults] = createResource(async () => {
    try {
      return await getBacktestResults()
    } catch {
      return []
    }
  })

  // 백테스트 결과가 로드되면 첫 번째 결과를 자동 선택
  createEffect(() => {
    const results = backtestResults()
    if (results && results.length > 0 && !selectedBacktestId()) {
      setSelectedBacktestId(results[0].id)
    }
  })

  // 자격증명 목록 (mock 계정 제외)
  const [credentials] = createResource(async () => {
    try {
      const res = await listCredentials()
      return res.credentials.filter((c: ExchangeCredential) => c.exchange_id !== 'mock')
    } catch {
      return []
    }
  })

  // 동기화 핸들러 (매매일지와 통합된 엔드포인트 사용)
  const handleSync = async () => {
    const credId = syncCredentialId()
    if (!credId) {
      setSyncResult({ success: false, message: '자격증명을 선택해주세요.' })
      return
    }

    // 선택된 credential에서 exchange_id 추출
    const creds = credentials()
    const selectedCred = creds?.find((c: ExchangeCredential) => c.id === credId)
    if (!selectedCred) {
      setSyncResult({ success: false, message: '선택된 자격증명을 찾을 수 없습니다.' })
      return
    }

    setSyncLoading(true)
    setSyncResult(null)

    try {
      // 매매일지 통합 동기화 API 호출
      const result: JournalSyncResponse = await syncJournalExecutions(
        selectedCred.exchange_id,
        undefined, // startDate는 백엔드에서 자동 감지
        forceFullSync()
      )
      setSyncResult({
        success: result.success,
        message: result.success
          ? `체결 내역 동기화 완료 (신규 ${result.inserted}건, 기존 ${result.skipped}건)`
          : result.message,
      })
      if (result.success) {
        // 동기화 성공 시 차트 데이터 새로고침
        refetchEquity()
        refetchPerformance()
        // 강제 동기화 후 체크 해제
        if (forceFullSync()) {
          setForceFullSync(false)
        }
      }
    } catch (e: unknown) {
      const errorMessage = e instanceof Error ? e.message : '동기화 중 오류가 발생했습니다.'
      setSyncResult({ success: false, message: errorMessage })
    } finally {
      setSyncLoading(false)
    }
  }

  // 자산 곡선 캐시 초기화 핸들러
  const handleClearCache = async () => {
    const credId = syncCredentialId()
    if (!credId) {
      setSyncResult({ success: false, message: '자격증명을 선택해주세요.' })
      return
    }

    if (!confirm('자산 곡선 캐시를 삭제하시겠습니까?\n삭제 후 다음 동기화 시 자산 곡선을 다시 계산합니다.')) {
      return
    }

    setSyncLoading(true)
    try {
      const result = await clearEquityCache(credId)
      setSyncResult({
        success: result.success,
        message: result.message,
      })
      if (result.success) {
        // 캐시 삭제 성공 시 차트 데이터 새로고침
        refetchEquity()
        refetchPerformance()
      }
    } catch (e: unknown) {
      const errorMessage = e instanceof Error ? e.message : '캐시 삭제 중 오류가 발생했습니다.'
      setSyncResult({ success: false, message: errorMessage })
    } finally {
      setSyncLoading(false)
    }
  }

  // 동기화 모달 열기
  const openSyncModal = () => {
    setSyncResult(null)
    // 첫 번째 KIS 자격증명 자동 선택
    const creds = credentials()
    if (creds && creds.length > 0 && !syncCredentialId()) {
      setSyncCredentialId(creds[0].id)
    }
    setShowSyncModal(true)
  }

  // 선택된 백테스트 데이터
  const selectedBacktest = () => {
    if (dataSource() !== 'backtest') return null
    const results = backtestResults() || []
    const id = selectedBacktestId()
    return results.find((r: BacktestResult) => r.id === id) || results[0]
  }

  // 기간에 따른 날짜 필터 계산 (밀리초 반환)
  const getPeriodStartTime = (): number => {
    const now = Date.now()
    const selectedPeriod = period()
    switch (selectedPeriod) {
      case '1w': return now - 7 * 24 * 60 * 60 * 1000
      case '1m': return now - 30 * 24 * 60 * 60 * 1000
      case '3m': return now - 90 * 24 * 60 * 60 * 1000
      case '6m': return now - 180 * 24 * 60 * 60 * 1000
      case '1y': return now - 365 * 24 * 60 * 60 * 1000
      case 'all':
      default: return 0
    }
  }

  // EquityCurve 컴포넌트용 데이터 변환
  const chartData = (): EquityDataPoint[] => {
    if (dataSource() === 'portfolio') {
      const data = equityCurveData()
      if (!data?.data) return []
      return data.data.map((point) => ({
        time: Math.floor(point.x / 1000), // milliseconds → seconds (lightweight-charts expects UNIX timestamp)
        value: parseFloat(point.y),
      }))
    } else {
      const backtest = selectedBacktest()
      if (!backtest?.equity_curve) return []

      // 기간 필터 적용 (timestamp는 초 단위)
      const periodStartSec = getPeriodStartTime() / 1000
      const filteredCurve = periodStartSec > 0
        ? backtest.equity_curve.filter(point => point.timestamp >= periodStartSec)
        : backtest.equity_curve

      return filteredCurve.map((point) => ({
        time: point.timestamp, // 백테스트는 이미 초 단위
        value: parseFloat(point.equity),
      }))
    }
  }

  // 현재 성과 지표
  const metrics = () => {
    if (dataSource() === 'portfolio') {
      const perf = performanceData()
      if (!perf) return null
      return {
        currentEquity: parseFloat(perf.current_equity || perf.currentEquity || '0'),
        totalReturn: parseFloat(perf.total_return_pct || perf.totalReturnPct || '0'),
        maxDrawdown: parseFloat(perf.max_drawdown_pct || perf.maxDrawdownPct || '0'),
        cagr: parseFloat(perf.cagr_pct || perf.cagrPct || '0'),
        // 포지션 기반 지표
        totalCostBasis: perf.total_cost_basis || perf.totalCostBasis
          ? parseFloat(perf.total_cost_basis || perf.totalCostBasis || '0')
          : undefined,
        positionPnl: perf.position_pnl || perf.positionPnl
          ? parseFloat(perf.position_pnl || perf.positionPnl || '0')
          : undefined,
        positionPnlPct: perf.position_pnl_pct || perf.positionPnlPct
          ? parseFloat(perf.position_pnl_pct || perf.positionPnlPct || '0')
          : undefined,
      }
    } else {
      const backtest = selectedBacktest()
      if (!backtest?.metrics || !backtest?.equity_curve) return null

      // 기간 필터 적용된 데이터로 지표 재계산
      const periodStartSec = getPeriodStartTime() / 1000
      const filteredCurve = periodStartSec > 0
        ? backtest.equity_curve.filter(point => point.timestamp >= periodStartSec)
        : backtest.equity_curve

      if (filteredCurve.length === 0) return null

      const startEquity = parseFloat(filteredCurve[0].equity)
      const endEquity = parseFloat(filteredCurve[filteredCurve.length - 1].equity)

      // 수익률 계산
      const totalReturn = startEquity > 0 ? ((endEquity - startEquity) / startEquity) * 100 : 0

      // MDD 계산
      let peak = startEquity
      let maxDrawdown = 0
      for (const point of filteredCurve) {
        const equity = parseFloat(point.equity)
        if (equity > peak) peak = equity
        const drawdown = ((peak - equity) / peak) * 100
        if (drawdown > maxDrawdown) maxDrawdown = drawdown
      }

      // 기간별 CAGR 계산 (연환산 수익률)
      const days = filteredCurve.length
      const years = days / 365
      const cagr = years > 0 && startEquity > 0
        ? (Math.pow(endEquity / startEquity, 1 / years) - 1) * 100
        : 0

      return {
        currentEquity: endEquity,
        totalReturn,
        maxDrawdown,
        cagr,
        totalCostBasis: undefined,
        positionPnl: undefined,
        positionPnlPct: undefined,
      }
    }
  }

  const handleRefresh = () => {
    if (dataSource() === 'portfolio') {
      refetchEquity()
      refetchPerformance()
    }
  }

  const isLoading = () =>
    (dataSource() === 'portfolio' && (equityCurveData.loading || performanceData.loading)) ||
    (dataSource() === 'backtest' && backtestResults.loading)

  return (
    <div class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)]">
      {/* Header with Controls */}
      <div class="p-4 border-b border-[var(--color-surface-light)]">
        <div class="flex items-center justify-between flex-wrap gap-3">
          <div class="flex items-center gap-3">
            <h3 class="text-lg font-semibold text-[var(--color-text)]">자산 곡선</h3>

            {/* Data Source Selector */}
            <Show when={props.showControls !== false}>
              <div class="flex items-center gap-1 bg-[var(--color-surface-light)] rounded-lg p-0.5">
                <button
                  onClick={() => setDataSource('portfolio')}
                  class={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm transition-colors ${
                    dataSource() === 'portfolio'
                      ? 'bg-[var(--color-primary)] text-white'
                      : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                  }`}
                >
                  <Database class="w-3.5 h-3.5" />
                  실제 포트폴리오
                </button>
                <button
                  onClick={() => setDataSource('backtest')}
                  class={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm transition-colors ${
                    dataSource() === 'backtest'
                      ? 'bg-[var(--color-primary)] text-white'
                      : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                  }`}
                >
                  <TestTube class="w-3.5 h-3.5" />
                  백테스트
                </button>
              </div>
            </Show>
          </div>

          <div class="flex items-center gap-2">
            {/* Backtest Selector (when backtest source is selected) */}
            <Show when={dataSource() === 'backtest' && (backtestResults()?.length || 0) > 0}>
              <select
                value={selectedBacktestId()}
                onChange={(e) => setSelectedBacktestId(e.currentTarget.value)}
                class="bg-[var(--color-surface-light)] text-[var(--color-text)] border border-[var(--color-surface-light)] rounded-lg px-3 py-1.5 text-sm"
              >
                <For each={backtestResults()}>
                  {(result: BacktestResult) => (
                    <option value={result.id}>
                      {result.strategy_id} - {result.symbol} ({result.start_date} ~ {result.end_date})
                    </option>
                  )}
                </For>
              </select>
            </Show>

            {/* Period Selector (for both portfolio and backtest) */}
            <Show when={props.showControls !== false}>
              <div class="flex items-center gap-1">
                <Calendar class="w-4 h-4 text-[var(--color-text-muted)]" />
                <For each={PERIOD_OPTIONS}>
                  {(option) => (
                    <button
                      onClick={() => setPeriod(option.value)}
                      class={`px-2 py-1 rounded text-sm transition-colors ${
                        period() === option.value
                          ? 'bg-[var(--color-primary)] text-white'
                          : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)] hover:bg-[var(--color-surface-light)]'
                      }`}
                    >
                      {option.label}
                    </button>
                  )}
                </For>
              </div>
            </Show>

            {/* Sync & Refresh Buttons */}
            <Show when={dataSource() === 'portfolio'}>
              <div class="flex items-center gap-1">
                {/* Sync Button */}
                <button
                  onClick={openSyncModal}
                  disabled={isLoading() || !credentials() || credentials()!.length === 0}
                  class="p-1.5 rounded-lg hover:bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors disabled:opacity-50"
                  title="거래소 데이터 동기화"
                >
                  <Download class="w-4 h-4" />
                </button>
                {/* Refresh Button */}
                <button
                  onClick={handleRefresh}
                  disabled={isLoading()}
                  class="p-1.5 rounded-lg hover:bg-[var(--color-surface-light)] text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors disabled:opacity-50"
                  title="새로고침"
                >
                  <RefreshCw class={`w-4 h-4 ${isLoading() ? 'animate-spin' : ''}`} />
                </button>
              </div>
            </Show>
          </div>
        </div>

        {/* Metrics Summary */}
        <Show when={metrics()}>
          {/* 포지션 기반 지표 (실제 투자 원금 대비) - 있을 경우만 표시 */}
          <Show when={dataSource() === 'portfolio' && metrics()!.totalCostBasis !== undefined}>
            <div class="grid grid-cols-3 gap-4 mt-4 pb-3 border-b border-[var(--color-surface-light)]">
              <div class="text-center">
                <div class="text-xs text-[var(--color-text-muted)] mb-1">투자 원금</div>
                <div class="text-lg font-semibold text-[var(--color-text)]">
                  {formatCurrency(metrics()!.totalCostBasis!)}
                </div>
              </div>
              <div class="text-center">
                <div class="text-xs text-[var(--color-text-muted)] mb-1">현재 평가액</div>
                <div class="text-lg font-semibold text-[var(--color-text)]">
                  {formatCurrency(metrics()!.currentEquity)}
                </div>
              </div>
              <div class="text-center">
                <div class="text-xs text-[var(--color-text-muted)] mb-1">포지션 손익</div>
                <div class={`text-lg font-semibold flex items-center justify-center gap-1 ${
                  (metrics()!.positionPnlPct ?? 0) >= 0 ? 'text-green-500' : 'text-red-500'
                }`}>
                  {(metrics()!.positionPnlPct ?? 0) >= 0 ? (
                    <TrendingUp class="w-4 h-4" />
                  ) : (
                    <TrendingDown class="w-4 h-4" />
                  )}
                  {formatPercent(metrics()!.positionPnlPct ?? 0)}
                  <span class="text-xs ml-1">({formatCurrency(metrics()!.positionPnl ?? 0)})</span>
                </div>
              </div>
            </div>
          </Show>

          {/* 기간 기반 지표 */}
          <div class="grid grid-cols-4 gap-4 mt-3">
            <div class="text-center">
              <div class="text-xs text-[var(--color-text-muted)] mb-1">현재 자산</div>
              <div class="text-lg font-semibold text-[var(--color-text)]">
                {formatCurrency(metrics()!.currentEquity)}
              </div>
            </div>
            <div class="text-center">
              <div class="text-xs text-[var(--color-text-muted)] mb-1">기간 수익률</div>
              <div class={`text-lg font-semibold flex items-center justify-center gap-1 ${
                metrics()!.totalReturn >= 0 ? 'text-green-500' : 'text-red-500'
              }`}>
                {metrics()!.totalReturn >= 0 ? (
                  <TrendingUp class="w-4 h-4" />
                ) : (
                  <TrendingDown class="w-4 h-4" />
                )}
                {formatPercent(metrics()!.totalReturn)}
              </div>
            </div>
            <div class="text-center">
              <div class="text-xs text-[var(--color-text-muted)] mb-1">MDD</div>
              <div class="text-lg font-semibold text-red-500">
                -{Math.abs(metrics()!.maxDrawdown).toFixed(2)}%
              </div>
            </div>
            <div class="text-center">
              <div class="text-xs text-[var(--color-text-muted)] mb-1">CAGR</div>
              <div class={`text-lg font-semibold ${
                metrics()!.cagr >= 0 ? 'text-green-500' : 'text-red-500'
              }`}>
                {formatPercent(metrics()!.cagr)}
              </div>
            </div>
          </div>
        </Show>
      </div>

      {/* Chart Area */}
      <div class="p-4">
        <Show
          when={!isLoading() && chartData().length > 0}
          fallback={
            <div class="flex items-center justify-center" style={{ height: `${props.height || 300}px` }}>
              <Show
                when={isLoading()}
                fallback={
                  <div class="text-center text-[var(--color-text-muted)]">
                    <Show
                      when={dataSource() === 'portfolio'}
                      fallback={
                        <div>
                          <TestTube class="w-12 h-12 mx-auto mb-3 opacity-50" />
                          <p>백테스트 결과가 없습니다.</p>
                          <a href="/backtest" class="text-[var(--color-primary)] hover:underline">
                            백테스트 실행하기
                          </a>
                        </div>
                      }
                    >
                      <Database class="w-12 h-12 mx-auto mb-3 opacity-50" />
                      <p>아직 포트폴리오 데이터가 없습니다.</p>
                      <p class="text-sm mt-1">포트폴리오 조회 시 데이터가 자동으로 축적됩니다.</p>
                    </Show>
                  </div>
                }
              >
                <RefreshCw class="w-6 h-6 animate-spin text-[var(--color-primary)]" />
                <span class="ml-2 text-[var(--color-text-muted)]">데이터 로딩 중...</span>
              </Show>
            </div>
          }
        >
          <EquityCurve
            data={chartData()}
            height={props.height || 300}
            colors={{
              equityColor: dataSource() === 'portfolio' ? '#3b82f6' : '#10b981',
              positiveArea: dataSource() === 'portfolio' ? 'rgba(59, 130, 246, 0.2)' : 'rgba(16, 185, 129, 0.2)',
            }}
          />
        </Show>
      </div>

      {/* Data Source Indicator */}
      <div class="px-4 pb-4">
        <div class="flex items-center justify-between text-xs text-[var(--color-text-muted)]">
          <div class="flex items-center gap-2">
            <div class={`w-2 h-2 rounded-full ${
              dataSource() === 'portfolio' ? 'bg-blue-500' : 'bg-green-500'
            }`} />
            <span>
              {dataSource() === 'portfolio' ? '실제 포트폴리오 데이터' : '백테스트 시뮬레이션 데이터'}
            </span>
          </div>
          <Show when={dataSource() === 'portfolio' && equityCurveData()}>
            <span>
              {equityCurveData()?.count || 0}개 데이터 포인트 •
              {equityCurveData()?.startTime && ` ${new Date(equityCurveData()!.startTime).toLocaleDateString('ko-KR')}`}
              {equityCurveData()?.endTime && ` ~ ${new Date(equityCurveData()!.endTime).toLocaleDateString('ko-KR')}`}
            </span>
          </Show>
          <Show when={dataSource() === 'backtest' && selectedBacktest()}>
            <span>
              {selectedBacktest()?.equity_curve?.length || 0}개 데이터 포인트 •
              {selectedBacktest()?.start_date} ~ {selectedBacktest()?.end_date}
            </span>
          </Show>
        </div>
      </div>

      {/* Sync Modal */}
      <Show when={showSyncModal()}>
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={() => setShowSyncModal(false)}>
          <div
            class="bg-[var(--color-surface)] rounded-xl border border-[var(--color-surface-light)] p-6 w-full max-w-md shadow-xl"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Modal Header */}
            <div class="flex items-center justify-between mb-4">
              <h3 class="text-lg font-semibold text-[var(--color-text)]">거래소 데이터 동기화</h3>
              <button
                onClick={() => setShowSyncModal(false)}
                class="p-1 rounded hover:bg-[var(--color-surface-light)] text-[var(--color-text-muted)]"
              >
                <X class="w-5 h-5" />
              </button>
            </div>

            {/* Modal Body */}
            <div class="space-y-4">
              <p class="text-sm text-[var(--color-text-muted)]">
                거래소 체결 내역을 기반으로 자산 곡선 데이터를 복원합니다.
              </p>

              {/* Credential Selector */}
              <div>
                <label class="block text-sm font-medium text-[var(--color-text)] mb-1">
                  거래소 계정
                </label>
                <select
                  value={syncCredentialId()}
                  onChange={(e) => setSyncCredentialId(e.currentTarget.value)}
                  class="w-full bg-[var(--color-surface-light)] text-[var(--color-text)] border border-[var(--color-surface-light)] rounded-lg px-3 py-2 text-sm"
                >
                  <Show when={!credentials() || credentials()!.length === 0}>
                    <option value="">등록된 KIS 계정이 없습니다</option>
                  </Show>
                  <For each={credentials()}>
                    {(cred: ExchangeCredential) => (
                      <option value={cred.id}>
                        {cred.display_name} {cred.is_testnet ? '(모의투자)' : '(실계좌)'}
                      </option>
                    )}
                  </For>
                </select>
              </div>

              {/* 강제 전체 동기화 옵션 */}
              <div class="flex items-center gap-3">
                <label class="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={forceFullSync()}
                    onChange={(e) => setForceFullSync(e.currentTarget.checked)}
                    class="w-4 h-4 rounded border-gray-600 bg-[var(--color-surface-light)] text-[var(--color-primary)] focus:ring-[var(--color-primary)]"
                  />
                  <span class="text-sm text-[var(--color-text)]">강제 전체 동기화</span>
                </label>
                <span class="text-xs text-[var(--color-text-muted)]">
                  (캐시 초기화 후 전체 내역 다시 조회)
                </span>
              </div>

              {/* Info: 자동 감지 안내 */}
              <div class="flex items-start gap-2 p-3 rounded-lg bg-blue-500/10 text-blue-400">
                <Calendar class="w-5 h-5 flex-shrink-0 mt-0.5" />
                <span class="text-sm">
                  {forceFullSync()
                    ? '캐시를 초기화하고 전체 체결 내역을 다시 동기화합니다.'
                    : '마지막 동기화 이후의 체결 내역만 가져옵니다.'}
                </span>
              </div>

              {/* Result Message */}
              <Show when={syncResult()}>
                <div class={`flex items-center gap-2 p-3 rounded-lg ${
                  syncResult()!.success
                    ? 'bg-green-500/10 text-green-500'
                    : 'bg-red-500/10 text-red-500'
                }`}>
                  {syncResult()!.success ? (
                    <CheckCircle class="w-5 h-5 flex-shrink-0" />
                  ) : (
                    <AlertCircle class="w-5 h-5 flex-shrink-0" />
                  )}
                  <span class="text-sm">{syncResult()!.message}</span>
                </div>
              </Show>
            </div>

            {/* Modal Footer */}
            <div class="flex justify-between items-center mt-6">
              {/* 캐시 삭제 버튼 (왼쪽) */}
              <button
                onClick={handleClearCache}
                disabled={syncLoading()}
                class="flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-red-400 hover:bg-red-500/10 transition-colors disabled:opacity-50"
                title="자산 곡선 캐시 삭제"
              >
                <Trash2 class="w-4 h-4" />
                캐시 삭제
              </button>

              {/* 닫기/동기화 버튼 (오른쪽) */}
              <div class="flex gap-2">
                <button
                  onClick={() => setShowSyncModal(false)}
                  class="px-4 py-2 rounded-lg text-sm text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)] transition-colors"
                >
                  닫기
                </button>
                <button
                  onClick={handleSync}
                  disabled={syncLoading() || !syncCredentialId()}
                  class="flex items-center gap-2 px-4 py-2 rounded-lg text-sm bg-[var(--color-primary)] text-white hover:bg-[var(--color-primary-hover)] transition-colors disabled:opacity-50"
                >
                  {syncLoading() ? (
                    <>
                      <RefreshCw class="w-4 h-4 animate-spin" />
                      동기화 중...
                    </>
                  ) : (
                    <>
                      <Download class="w-4 h-4" />
                      동기화 시작
                    </>
                  )}
                </button>
              </div>
            </div>
          </div>
        </div>
      </Show>
    </div>
  )
}
