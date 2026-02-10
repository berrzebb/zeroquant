/**
 * 매매 일지 페이지
 *
 * PRD 2.6에 따라 체결 내역, 보유 현황, 손익 분석 기능을 제공합니다.
 * 컴포넌트가 journal/ 폴더로 분리되어 모듈화되었습니다.
 *
 * 상태 관리: createStore를 사용하여 관련 상태를 그룹화
 * - filters: 필터 및 페이지네이션 상태
 * - loading: 로딩 상태
 * - modal: 모달 상태
 *
 * 데이터 소스:
 * - 실환경 (기본): /journal
 * - 백테스트: /journal?source=backtest&id=xxx
 */
import { createResource, Show, createMemo, lazy, Suspense, createEffect } from 'solid-js'
import { createStore } from 'solid-js/store'
import { useSearchParams, useNavigate } from '@solidjs/router'
import { BookOpen, BarChart3, LineChart, PieChart, Lightbulb, ArrowLeft } from 'lucide-solid'
import {
  PageHeader,
  StatCard,
  StatCardGrid,
  Button,
  Card,
  CardHeader,
  CardContent,
  formatCurrency,
  getPnLColor,
} from '../components/ui'
import {
  getJournalPositions,
  getJournalExecutions,
  getJournalPnLSummary,
  getJournalDailyPnL,
  getJournalSymbolPnL,
  getJournalWeeklyPnL,
  getJournalMonthlyPnL,
  getJournalYearlyPnL,
  getJournalCumulativePnL,
  getJournalInsights,
  getJournalStrategyPerformance,
  syncJournalExecutions,
  clearJournalCache,
  getBacktestResult,
  getBacktestStrategies,
} from '../api/client'
import type { ExecutionFilter, BacktestResult, SymbolPnLItem } from '../api/client'

// Lazy load heavy components
const PositionsTable = lazy(() =>
  import('../components/journal/PositionsTable').then(m => ({ default: m.PositionsTable }))
)
const ExecutionsTable = lazy(() =>
  import('../components/journal/ExecutionsTable').then(m => ({ default: m.ExecutionsTable }))
)
const SymbolPnLTable = lazy(() =>
  import('../components/journal/SymbolPnLTable').then(m => ({ default: m.SymbolPnLTable }))
)
const PnLAnalysisPanel = lazy(() =>
  import('../components/journal/PnLAnalysisPanel').then(m => ({ default: m.PnLAnalysisPanel }))
)
const StrategyInsightsPanel = lazy(() =>
  import('../components/journal/StrategyInsightsPanel').then(m => ({ default: m.StrategyInsightsPanel }))
)
const PositionDonutChart = lazy(() =>
  import('../components/journal/PositionDonutChart').then(m => ({ default: m.PositionDonutChart }))
)
const PositionDetailModal = lazy(() =>
  import('../components/journal/PositionDetailModal').then(m => ({ default: m.PositionDetailModal }))
)
import type { JournalPosition } from '../api/client'
import { createLogger } from '../utils/logger'

const { log, warn, error: logError } = createLogger('TradingJournal')

// ==================== 데이터 소스 타입 ====================

/** 데이터 소스 타입 */
type DataSourceType = 'live' | 'backtest'

// ==================== 백테스트 데이터 변환 함수 ====================

/** 백테스트 결과를 PnL 요약 형식으로 변환 */
function convertBacktestToPnLSummary(result: BacktestResult) {
  const trades = result.trades
  const pnls = trades.map(t => parseFloat(t.pnl))
  const wins = pnls.filter(p => p > 0)
  const totalPnl = pnls.reduce((a, b) => a + b, 0)
  const totalFees = result.all_trades
    ? result.all_trades.reduce((sum, t) => sum + parseFloat(t.commission), 0)
    : 0

  return {
    net_pnl: totalPnl.toFixed(0),
    total_trades: result.all_trades?.length || trades.length * 2,
    win_rate: trades.length > 0 ? ((wins.length / trades.length) * 100).toFixed(2) : '0.00',
    total_fees: totalFees.toFixed(0),
  }
}

/** 백테스트 결과를 일별 손익 형식으로 변환 */
function convertBacktestToDailyPnL(result: BacktestResult) {
  const dailyMap = new Map<string, { pnl: number; count: number }>()

  for (const trade of result.trades) {
    const date = trade.exit_time?.split('T')[0]
    if (!date) continue
    const pnl = parseFloat(trade.pnl)
    const existing = dailyMap.get(date) || { pnl: 0, count: 0 }
    dailyMap.set(date, { pnl: existing.pnl + pnl, count: existing.count + 1 })
  }

  const sorted = Array.from(dailyMap.entries()).sort((a, b) => a[0].localeCompare(b[0]))
  return sorted.map(([date, data]) => ({
    date,
    realized_pnl: data.pnl.toFixed(0),
    trade_count: data.count,
    fees: '0',
  }))
}

/** 백테스트 결과를 주별 손익 형식으로 변환 */
function convertBacktestToWeeklyPnL(result: BacktestResult) {
  const weeklyMap = new Map<string, { pnl: number; count: number }>()

  for (const trade of result.trades) {
    const date = trade.exit_time?.split('T')[0]
    if (!date) continue
    const d = new Date(date)
    // 주 시작일 (월요일 기준)
    const dayOfWeek = d.getDay()
    const diff = d.getDate() - dayOfWeek + (dayOfWeek === 0 ? -6 : 1)
    const weekStart = new Date(d.setDate(diff)).toISOString().split('T')[0]

    const pnl = parseFloat(trade.pnl)
    const existing = weeklyMap.get(weekStart) || { pnl: 0, count: 0 }
    weeklyMap.set(weekStart, { pnl: existing.pnl + pnl, count: existing.count + 1 })
  }

  return Array.from(weeklyMap.entries())
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([weekStart, data]) => ({
      week_start: weekStart,
      realized_pnl: data.pnl.toFixed(0),
      trade_count: data.count,
      fees: '0',
    }))
}

/** 백테스트 결과를 월별 손익 형식으로 변환 */
function convertBacktestToMonthlyPnL(result: BacktestResult) {
  const monthlyMap = new Map<string, { pnl: number; count: number }>()

  for (const trade of result.trades) {
    const date = trade.exit_time?.split('T')[0]
    if (!date) continue
    const [year, month] = date.split('-')
    const key = `${year}-${month}`

    const pnl = parseFloat(trade.pnl)
    const existing = monthlyMap.get(key) || { pnl: 0, count: 0 }
    monthlyMap.set(key, { pnl: existing.pnl + pnl, count: existing.count + 1 })
  }

  return Array.from(monthlyMap.entries())
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([key, data]) => {
      const [year, month] = key.split('-')
      return {
        year: parseInt(year),
        month: parseInt(month),
        realized_pnl: data.pnl.toFixed(0),
        trade_count: data.count,
        fees: '0',
      }
    })
}

/** 백테스트 결과를 연도별 손익 형식으로 변환 */
function convertBacktestToYearlyPnL(result: BacktestResult) {
  const yearlyMap = new Map<number, { pnl: number; count: number }>()

  for (const trade of result.trades) {
    const date = trade.exit_time?.split('T')[0]
    if (!date) continue
    const year = parseInt(date.split('-')[0])

    const pnl = parseFloat(trade.pnl)
    const existing = yearlyMap.get(year) || { pnl: 0, count: 0 }
    yearlyMap.set(year, { pnl: existing.pnl + pnl, count: existing.count + 1 })
  }

  return Array.from(yearlyMap.entries())
    .sort((a, b) => a[0] - b[0])
    .map(([year, data]) => ({
      year,
      realized_pnl: data.pnl.toFixed(0),
      trade_count: data.count,
      fees: '0',
    }))
}

/** 백테스트 결과를 누적 손익 형식으로 변환 */
function convertBacktestToCumulativePnL(result: BacktestResult) {
  // equity_curve가 있으면 사용
  if (result.equity_curve && result.equity_curve.length > 0) {
    const initialEquity = parseFloat(result.equity_curve[0].equity)
    return result.equity_curve.map(point => ({
      date: new Date(point.timestamp * 1000).toISOString().split('T')[0],
      cumulative_pnl: (parseFloat(point.equity) - initialEquity).toFixed(0),
      cumulative_fees: '0',
      cumulative_trades: 0, // 정확한 값 필요 시 계산 로직 추가
    }))
  }

  // 폴백: 라운드트립에서 누적 계산
  const dailyPnL = convertBacktestToDailyPnL(result)
  let cumulative = 0
  let cumulativeTrades = 0
  return dailyPnL.map(day => {
    cumulative += parseFloat(day.realized_pnl)
    cumulativeTrades += day.trade_count
    return {
      date: day.date,
      cumulative_pnl: cumulative.toFixed(0),
      cumulative_fees: '0',
      cumulative_trades: cumulativeTrades,
    }
  })
}

/** 백테스트 결과를 종목별 손익 형식으로 변환 (SymbolPnLItem 호환) */
function convertBacktestToSymbolPnL(result: BacktestResult): SymbolPnLItem[] {
  const symbolMap = new Map<string, {
    totalTrades: number
    buyQty: number; sellQty: number
    buyValue: number; sellValue: number
    fees: number; pnl: number
    firstTradeAt: string | null; lastTradeAt: string | null
  }>()

  // all_trades(개별 체결)가 있으면 매수/매도 수량·금액·수수료 집계
  if (result.all_trades) {
    for (const t of result.all_trades) {
      const qty = parseFloat(t.quantity)
      const price = parseFloat(t.price)
      const value = qty * price
      const fee = parseFloat(t.commission)
      const existing = symbolMap.get(t.symbol) || {
        totalTrades: 0, buyQty: 0, sellQty: 0,
        buyValue: 0, sellValue: 0, fees: 0, pnl: 0,
        firstTradeAt: null, lastTradeAt: null,
      }
      existing.totalTrades += 1
      existing.fees += fee
      if (t.side === 'buy') {
        existing.buyQty += qty
        existing.buyValue += value
      } else {
        existing.sellQty += qty
        existing.sellValue += value
      }
      if (!existing.firstTradeAt || t.timestamp < existing.firstTradeAt) existing.firstTradeAt = t.timestamp
      if (!existing.lastTradeAt || t.timestamp > existing.lastTradeAt) existing.lastTradeAt = t.timestamp
      symbolMap.set(t.symbol, existing)
    }
  }

  // trades(라운드트립)에서 실현손익 집계
  for (const trade of result.trades) {
    const pnl = parseFloat(trade.pnl)
    const existing = symbolMap.get(trade.symbol)
    if (existing) {
      existing.pnl += pnl
    } else {
      // all_trades 없는 경우 폴백
      const qty = parseFloat(trade.quantity)
      const entryPrice = parseFloat(trade.entry_price)
      const exitPrice = parseFloat(trade.exit_price)
      symbolMap.set(trade.symbol, {
        totalTrades: 1,
        buyQty: qty, sellQty: qty,
        buyValue: qty * entryPrice, sellValue: qty * exitPrice,
        fees: 0, pnl,
        firstTradeAt: trade.entry_time, lastTradeAt: trade.exit_time,
      })
    }
  }

  return Array.from(symbolMap.entries()).map(([symbol, d]) => ({
    symbol,
    symbol_name: null,
    total_trades: BigInt(d.totalTrades),
    total_buy_qty: d.buyQty.toFixed(4),
    total_sell_qty: d.sellQty.toFixed(4),
    total_buy_value: d.buyValue.toFixed(0),
    total_sell_value: d.sellValue.toFixed(0),
    total_fees: d.fees.toFixed(0),
    realized_pnl: d.pnl.toFixed(0),
    first_trade_at: d.firstTradeAt,
    last_trade_at: d.lastTradeAt,
  }))
}

/** 백테스트 결과를 인사이트 형식으로 변환 */
function convertBacktestToInsights(result: BacktestResult) {
  const trades = result.trades
  const pnls = trades.map(t => parseFloat(t.pnl))
  const wins = pnls.filter(p => p > 0)
  const losses = pnls.filter(p => p < 0)
  const totalPnl = pnls.reduce((a, b) => a + b, 0)
  const totalFees = result.all_trades
    ? result.all_trades.reduce((sum, t) => sum + parseFloat(t.commission), 0)
    : 0

  const avgWin = wins.length > 0 ? wins.reduce((a, b) => a + b, 0) / wins.length : 0
  const avgLoss = losses.length > 0 ? Math.abs(losses.reduce((a, b) => a + b, 0) / losses.length) : 0
  const profitFactor = avgLoss > 0 && losses.length > 0
    ? (avgWin * wins.length) / (avgLoss * losses.length)
    : wins.length > 0 ? Infinity : 0

  // 연속 승/패 계산
  let maxConsecutiveWins = 0
  let maxConsecutiveLosses = 0
  let currentWins = 0
  let currentLosses = 0

  for (const pnl of pnls) {
    if (pnl > 0) {
      currentWins++
      currentLosses = 0
      maxConsecutiveWins = Math.max(maxConsecutiveWins, currentWins)
    } else if (pnl < 0) {
      currentLosses++
      currentWins = 0
      maxConsecutiveLosses = Math.max(maxConsecutiveLosses, currentLosses)
    }
  }

  // 거래 기간 계산
  const dates = trades
    .map(t => t.entry_time?.split('T')[0])
    .filter(Boolean)
    .sort()
  const tradingPeriodDays = dates.length >= 2
    ? Math.ceil((new Date(dates[dates.length - 1]!).getTime() - new Date(dates[0]!).getTime()) / (1000 * 60 * 60 * 24))
    : 0
  const uniqueDates = new Set(dates)

  // 고유 종목 수
  const uniqueSymbols = new Set(trades.map(t => t.symbol)).size

  const totalAllTrades = result.all_trades?.length || trades.length * 2
  const buyTrades = result.all_trades
    ? result.all_trades.filter(t => t.side === 'buy' || t.side === 'Buy').length
    : trades.length
  const sellTrades = result.all_trades
    ? result.all_trades.filter(t => t.side === 'sell' || t.side === 'Sell').length
    : trades.length
  const netPnl = (totalPnl - totalFees).toFixed(0)

  return {
    total_trades: totalAllTrades,
    buy_trades: buyTrades,
    sell_trades: sellTrades,
    winning_trades: wins.length,
    losing_trades: losses.length,
    unique_symbols: uniqueSymbols,
    total_realized_pnl: totalPnl.toFixed(0),
    win_rate_pct: trades.length > 0 ? ((wins.length / trades.length) * 100).toFixed(2) : '0.00',
    total_pnl: totalPnl.toFixed(0),
    total_fees: totalFees.toFixed(0),
    net_pnl: netPnl,
    avg_win: avgWin.toFixed(0),
    avg_loss: avgLoss.toFixed(0),
    largest_win: wins.length > 0 ? Math.max(...wins).toFixed(0) : '0',
    largest_loss: losses.length > 0 ? Math.min(...losses).toFixed(0) : '0',
    profit_factor: Number.isFinite(profitFactor) ? profitFactor.toFixed(2) : '∞',
    max_consecutive_wins: maxConsecutiveWins,
    max_consecutive_losses: maxConsecutiveLosses,
    max_drawdown: result.metrics?.max_drawdown_pct || '0',
    max_drawdown_pct: result.metrics?.max_drawdown_pct || '0',
    trading_period_days: tradingPeriodDays,
    active_trading_days: uniqueDates.size,
    first_trade_at: dates.length > 0 ? dates[0]! : null,
    last_trade_at: dates.length > 0 ? dates[dates.length - 1]! : null,
  }
}

/** 백테스트 결과를 체결 내역 형식으로 변환 (JournalExecution 호환) */
function convertBacktestToExecutions(result: BacktestResult) {
  // all_trades가 있으면 사용
  if (result.all_trades && result.all_trades.length > 0) {
    return result.all_trades.map((t, i) => ({
      id: `exec-${i}`,
      exchange: 'backtest',
      symbol: t.symbol,
      symbol_name: null,
      side: t.side,
      order_type: t.signal_type || 'market',
      quantity: String(t.quantity),
      price: String(t.price),
      notional_value: String(Number(t.price) * Number(t.quantity)),
      fee: t.commission ? String(t.commission) : null,
      fee_currency: null,
      position_effect: t.signal_type || null,
      realized_pnl: t.realized_pnl != null ? String(t.realized_pnl) : null,
      strategy_id: result.strategy_id || null,
      strategy_name: null,
      executed_at: t.timestamp,
      memo: null,
      tags: null,
      // 백테스트 전용 필드 (하위 호환)
      timestamp: t.timestamp,
      signal_type: t.signal_type,
      is_partial: t.is_partial,
    }))
  }

  // 폴백: 라운드트립에서 추출
  const executions: Record<string, unknown>[] = []
  result.trades.forEach((trade, i) => {
    executions.push({
      id: `exec-entry-${i}`,
      exchange: 'backtest',
      symbol: trade.symbol,
      symbol_name: null,
      side: trade.side === 'Buy' ? 'buy' : 'sell',
      order_type: 'market',
      quantity: String(trade.quantity),
      price: String(trade.entry_price),
      notional_value: String(Number(trade.entry_price) * Number(trade.quantity)),
      fee: null,
      fee_currency: null,
      position_effect: 'entry',
      realized_pnl: null,
      strategy_id: result.strategy_id || null,
      strategy_name: null,
      executed_at: trade.entry_time,
      memo: null,
      tags: null,
      timestamp: trade.entry_time,
      signal_type: 'entry',
      is_partial: false,
    })
    if (trade.exit_time) {
      executions.push({
        id: `exec-exit-${i}`,
        exchange: 'backtest',
        symbol: trade.symbol,
        symbol_name: null,
        side: trade.side === 'Buy' ? 'sell' : 'buy',
        order_type: 'market',
        quantity: String(trade.quantity),
        price: String(trade.exit_price),
        notional_value: String(Number(trade.exit_price) * Number(trade.quantity)),
        fee: null,
        fee_currency: null,
        position_effect: 'exit',
        realized_pnl: trade.pnl != null ? String(trade.pnl) : null,
        strategy_id: result.strategy_id || null,
        strategy_name: null,
        executed_at: trade.exit_time,
        memo: null,
        tags: null,
        timestamp: trade.exit_time,
        signal_type: 'exit',
        is_partial: false,
      })
    }
  })
  return executions.sort((a, b) => (String(a.executed_at || '')).localeCompare(String(b.executed_at || '')))
}

// ==================== 타입 정의 ====================

/** 탭 타입 (5개로 통합) */
type TabType = 'positions' | 'executions' | 'pnl-analysis' | 'symbols' | 'strategy-insights'

/** 필터 상태 타입 */
interface FilterState {
  symbol: string
  side: string
  startDate: string
  endDate: string
  currentPage: number
  pageSize: number
}

/** 로딩 상태 타입 */
interface LoadingState {
  isRefreshing: boolean
  isSyncing: boolean
}

/** 모달 상태 타입 */
interface ModalState {
  position: {
    open: boolean
    data: JournalPosition | null
  }
}

/** UI 상태 타입 */
interface UIState {
  activeTab: TabType
}

// ==================== 초기 상태 ====================

const initialFilterState: FilterState = {
  symbol: '',
  side: '',
  startDate: '',
  endDate: '',
  currentPage: 1,
  pageSize: 50,
}

const initialLoadingState: LoadingState = {
  isRefreshing: false,
  isSyncing: false,
}

const initialModalState: ModalState = {
  position: { open: false, data: null },
}

const initialUIState: UIState = {
  activeTab: 'positions',
}

// ==================== 유틸리티 함수 ====================

/** 필터가 있는 경우의 안전한 wrapper */
const safeFetchWithArg = <T, A>(fetcher: (arg: A) => Promise<T>, fallback: T) => async (arg: A): Promise<T> => {
  try {
    return await fetcher(arg)
  } catch (error) {
    warn('API fetch failed:', error)
    return fallback
  }
}

export function TradingJournal() {
  // ==================== URL 파라미터 및 라우터 ====================
  const [searchParams] = useSearchParams<{ source?: string; id?: string; strategy?: string }>()
  const navigate = useNavigate()

  // 데이터 소스 결정
  const dataSource = createMemo<DataSourceType>(() =>
    searchParams.source === 'backtest' ? 'backtest' : 'live'
  )
  const isBacktest = createMemo(() => dataSource() === 'backtest')
  const backtestId = createMemo(() => searchParams.id || null)
  const strategyName = createMemo(() => searchParams.strategy || null)

  // ==================== createStore 기반 상태 관리 ====================
  const [filters, setFilters] = createStore<FilterState>(initialFilterState)
  const [loading, setLoading] = createStore<LoadingState>(initialLoadingState)
  const [modal, setModal] = createStore<ModalState>(initialModalState)
  const [ui, setUI] = createStore<UIState>(initialUIState)

  // 백테스트 모드에서는 체결 내역 탭으로 시작
  createEffect(() => {
    if (isBacktest() && ui.activeTab === 'positions') {
      setUI('activeTab', 'executions')
    }
  })

  // ==================== 모달 헬퍼 함수 ====================

  /** 포지션 상세 모달 열기 */
  const openPositionModal = (position: JournalPosition) => {
    setModal('position', { open: true, data: position })
  }

  /** 포지션 상세 모달 닫기 */
  const closePositionModal = () => {
    setModal('position', { open: false, data: null })
  }

  // ==================== 백테스트 데이터 로드 ====================
  const [backtestResult, { refetch: refetchBacktest }] = createResource(
    backtestId,
    async (id) => {
      if (!id) return null
      try {
        return await getBacktestResult(id)
      } catch (error) {
        warn('백테스트 결과 로드 실패:', error)
        return null
      }
    }
  )

  // 전략 목록 (전략명 조회용)
  const [strategies] = createResource(
    () => isBacktest(),
    async (shouldFetch) => {
      if (!shouldFetch) return []
      try {
        const response = await getBacktestStrategies()
        return response.strategies
      } catch {
        return []
      }
    }
  )

  // 백테스트 전략명 조회
  const backtestStrategyName = createMemo(() => {
    if (!isBacktest()) return null
    // URL 파라미터에서 전략명이 있으면 사용
    if (strategyName()) return strategyName()
    // 백테스트 결과에서 전략 ID로 조회
    const result = backtestResult()
    if (!result) return null
    const strategy = strategies()?.find(s => s.id === result.strategy_id)
    return strategy?.name || result.strategy_id
  })

  // ==================== 실환경 데이터 로드 (에러 발생 시 빈 데이터 반환) ====================
  const [positions, { refetch: refetchPositions }] = createResource(
    () => !isBacktest(),
    safeFetchWithArg(async () => getJournalPositions(), { positions: [], summary: null })
  )
  const [livePnlSummary, { refetch: refetchPnL }] = createResource(
    () => !isBacktest(),
    safeFetchWithArg(async () => getJournalPnLSummary(), null)
  )
  const [liveDailyPnL, { refetch: refetchDaily }] = createResource(
    () => !isBacktest(),
    safeFetchWithArg(async () => getJournalDailyPnL(), { daily: [] })
  )
  const [liveSymbolPnL, { refetch: refetchSymbols }] = createResource(
    () => !isBacktest(),
    safeFetchWithArg(async () => getJournalSymbolPnL(), { symbols: [] })
  )

  // 기간별 손익 데이터
  const [liveWeeklyPnL, { refetch: refetchWeekly }] = createResource(
    () => !isBacktest(),
    safeFetchWithArg(async () => getJournalWeeklyPnL(), { weekly: [] })
  )
  const [liveMonthlyPnL, { refetch: refetchMonthly }] = createResource(
    () => !isBacktest(),
    safeFetchWithArg(async () => getJournalMonthlyPnL(), { monthly: [] })
  )
  const [liveYearlyPnL, { refetch: refetchYearly }] = createResource(
    () => !isBacktest(),
    safeFetchWithArg(async () => getJournalYearlyPnL(), { yearly: [] })
  )
  const [liveCumulativePnL, { refetch: refetchCumulative }] = createResource(
    () => !isBacktest(),
    safeFetchWithArg(async () => getJournalCumulativePnL(), { curve: [] })
  )

  // 전략 성과 및 인사이트
  const [liveStrategyPerformance, { refetch: refetchStrategies }] = createResource(
    () => !isBacktest(),
    safeFetchWithArg(async () => getJournalStrategyPerformance(), { strategies: [] })
  )
  const [liveInsights, { refetch: refetchInsights }] = createResource(
    () => !isBacktest(),
    safeFetchWithArg(async () => getJournalInsights(), null)
  )

  // ==================== 통합 데이터 접근자 (실환경/백테스트 분기) ====================
  const pnlSummary = createMemo(() => {
    if (isBacktest()) {
      const result = backtestResult()
      return result ? convertBacktestToPnLSummary(result) : null
    }
    return livePnlSummary()
  })

  const dailyPnL = createMemo(() => {
    if (isBacktest()) {
      const result = backtestResult()
      return result ? { daily: convertBacktestToDailyPnL(result) } : { daily: [] }
    }
    return liveDailyPnL() || { daily: [] }
  })

  const weeklyPnL = createMemo(() => {
    if (isBacktest()) {
      const result = backtestResult()
      return result ? { weekly: convertBacktestToWeeklyPnL(result) } : { weekly: [] }
    }
    return liveWeeklyPnL() || { weekly: [] }
  })

  const monthlyPnL = createMemo(() => {
    if (isBacktest()) {
      const result = backtestResult()
      return result ? { monthly: convertBacktestToMonthlyPnL(result) } : { monthly: [] }
    }
    return liveMonthlyPnL() || { monthly: [] }
  })

  const yearlyPnL = createMemo(() => {
    if (isBacktest()) {
      const result = backtestResult()
      return result ? { yearly: convertBacktestToYearlyPnL(result) } : { yearly: [] }
    }
    return liveYearlyPnL() || { yearly: [] }
  })

  const cumulativePnL = createMemo(() => {
    if (isBacktest()) {
      const result = backtestResult()
      return result ? { curve: convertBacktestToCumulativePnL(result) } : { curve: [] }
    }
    return liveCumulativePnL() || { curve: [] }
  })

  const symbolPnL = createMemo(() => {
    if (isBacktest()) {
      const result = backtestResult()
      return result ? { symbols: convertBacktestToSymbolPnL(result) } : { symbols: [] }
    }
    return liveSymbolPnL() || { symbols: [] }
  })

  const insights = createMemo(() => {
    if (isBacktest()) {
      const result = backtestResult()
      return result ? convertBacktestToInsights(result) : null
    }
    return liveInsights()
  })

  const strategyPerformance = createMemo(() => {
    if (isBacktest()) {
      const result = backtestResult()
      if (!result) return { strategies: [], total: 0 }
      const ins = insights()
      const summary = convertBacktestToPnLSummary(result)
      const trades = result.trades
      const pnls = trades.map(t => parseFloat(t.pnl))
      const wins = pnls.filter(p => p > 0)
      const losses = pnls.filter(p => p < 0)
      const totalPnl = pnls.reduce((a, b) => a + b, 0)
      const avgWin = wins.length > 0 ? wins.reduce((a, b) => a + b, 0) / wins.length : 0
      const avgLoss = losses.length > 0 ? Math.abs(losses.reduce((a, b) => a + b, 0) / losses.length) : 0

      // 거래 날짜 계산
      const dates = trades
        .map(t => t.entry_time?.split('T')[0])
        .filter(Boolean)
        .sort()
      const uniqueDates = new Set(dates)

      // 총 거래량
      const totalVolume = result.all_trades
        ? result.all_trades.reduce((sum, t) => sum + parseFloat(t.quantity) * parseFloat(t.price), 0)
        : 0

      return {
        strategies: [{
          strategy_id: result.strategy_id,
          strategy_name: backtestStrategyName() || result.strategy_id,
          total_trades: summary.total_trades,
          buy_trades: result.all_trades
            ? result.all_trades.filter(t => t.side === 'buy' || t.side === 'Buy').length
            : trades.length,
          sell_trades: result.all_trades
            ? result.all_trades.filter(t => t.side === 'sell' || t.side === 'Sell').length
            : trades.length,
          unique_symbols: new Set(trades.map(t => t.symbol)).size,
          total_volume: totalVolume.toFixed(0),
          total_fees: summary.total_fees,
          realized_pnl: totalPnl.toFixed(0),
          winning_trades: wins.length,
          losing_trades: losses.length,
          win_rate_pct: trades.length > 0 ? ((wins.length / trades.length) * 100).toFixed(2) : '0.00',
          profit_factor: ins?.profit_factor || '0',
          avg_win: avgWin.toFixed(0),
          avg_loss: avgLoss.toFixed(0),
          largest_win: wins.length > 0 ? Math.max(...wins).toFixed(0) : '0',
          largest_loss: losses.length > 0 ? Math.min(...losses).toFixed(0) : '0',
          active_trading_days: uniqueDates.size,
          first_trade_at: dates.length > 0 ? dates[0]! : null,
          last_trade_at: dates.length > 0 ? dates[dates.length - 1]! : null,
        }],
        total: 1,
      }
    }
    return liveStrategyPerformance() || { strategies: [], total: 0 }
  })

  // ==================== 파생 상태 (createMemo) ====================

  /** 체결 내역 필터 (페이지네이션 + 날짜 필터 포함) */
  const executionFilter = createMemo<ExecutionFilter>(() => ({
    symbol: filters.symbol || undefined,
    side: filters.side || undefined,
    start_date: filters.startDate || undefined,
    end_date: filters.endDate || undefined,
    limit: filters.pageSize,
    offset: (filters.currentPage - 1) * filters.pageSize,
  }))

  // ==================== 필터 핸들러 ====================

  /** 필터 변경 시 페이지 자동 초기화 */
  const updateFilter = <K extends keyof FilterState>(key: K, value: FilterState[K]) => {
    setFilters({ [key]: value, currentPage: 1 } as Partial<FilterState>)
  }

  /** 심볼 필터 변경 */
  const handleSymbolFilterChange = (value: string) => updateFilter('symbol', value)

  /** 매매 방향 필터 변경 */
  const handleSideFilterChange = (value: string) => updateFilter('side', value)

  /** 시작일 필터 변경 */
  const handleStartDateChange = (value: string) => updateFilter('startDate', value)

  /** 종료일 필터 변경 */
  const handleEndDateChange = (value: string) => updateFilter('endDate', value)

  /** 페이지 변경 */
  const handlePageChange = (page: number) => setFilters('currentPage', page)

  // 실환경 체결 내역 (API)
  const [liveExecutions, { refetch: refetchExecutions }] = createResource(
    () => !isBacktest() ? executionFilter() : null,
    safeFetchWithArg(
      async (filter: ExecutionFilter | null) => filter ? getJournalExecutions(filter) : { executions: [] },
      { executions: [] }
    )
  )

  // 통합 체결 내역 (실환경/백테스트 분기)
  const executions = createMemo(() => {
    if (isBacktest()) {
      const result = backtestResult()
      if (!result) return { executions: [], total: 0 }

      let allExecutions = convertBacktestToExecutions(result)

      // 필터 적용
      if (filters.symbol) {
        allExecutions = allExecutions.filter(e =>
          e.symbol.toLowerCase().includes(filters.symbol.toLowerCase())
        )
      }
      if (filters.side) {
        allExecutions = allExecutions.filter(e => e.side === filters.side)
      }
      if (filters.startDate) {
        allExecutions = allExecutions.filter(e => e.timestamp >= filters.startDate)
      }
      if (filters.endDate) {
        allExecutions = allExecutions.filter(e => e.timestamp <= filters.endDate + 'T23:59:59')
      }

      // 페이지네이션
      const total = allExecutions.length
      const start = (filters.currentPage - 1) * filters.pageSize
      const paged = allExecutions.slice(start, start + filters.pageSize)

      return { executions: paged, total }
    }
    return liveExecutions() || { executions: [] }
  })

  // ==================== 데이터 로드 핸들러 ====================

  /** 새로고침 */
  const handleRefresh = async () => {
    setLoading('isRefreshing', true)
    try {
      if (isBacktest()) {
        // 백테스트 모드: 백테스트 결과만 새로고침
        await refetchBacktest()
      } else {
        // 실환경 모드: 모든 데이터 새로고침
        await Promise.all([
          refetchPositions(),
          refetchPnL(),
          refetchDaily(),
          refetchSymbols(),
          refetchExecutions(),
          refetchWeekly(),
          refetchMonthly(),
          refetchYearly(),
          refetchCumulative(),
          refetchStrategies(),
          refetchInsights(),
        ])
      }
    } finally {
      setLoading('isRefreshing', false)
    }
  }

  /** 동기화 */
  const handleSync = async (forceFullSync: boolean = false) => {
    setLoading('isSyncing', true)
    try {
      if (forceFullSync) {
        // 강제 동기화: 캐시 초기화 후 전체 내역 조회
        log('강제 동기화 시작: 캐시 초기화 포함')
      }
      const result = await syncJournalExecutions(undefined, undefined, forceFullSync)
      if (result.success) {
        await handleRefresh()
      }
    } catch (error) {
      logError('Sync failed:', error)
    } finally {
      setLoading('isSyncing', false)
    }
  }

  /** 캐시 초기화 */
  const handleClearCache = async () => {
    if (!confirm('캐시를 초기화하시겠습니까?\n\n초기화 후 다음 동기화 시 전체 체결 내역을 다시 조회합니다.')) {
      return
    }
    try {
      const result = await clearJournalCache()
      log('캐시 초기화 완료:', result.message)
      alert(`캐시 초기화 완료: ${result.deleted_count}건 삭제`)
    } catch (error) {
      logError('캐시 초기화 실패:', error)
      alert('캐시 초기화 실패')
    }
  }

  // ==================== UI 컴포넌트 ====================

  /** 뒤로가기 핸들러 */
  const handleGoBack = () => {
    navigate(-1)
  }

  /** 액션 버튼 컴포넌트 (실환경/백테스트 분기) */
  const HeaderActions = () => (
    <div class="flex items-center gap-3">
      <Show when={isBacktest()}>
        {/* 백테스트 모드: 뒤로가기 버튼 */}
        <Button variant="ghost" onClick={handleGoBack}>
          <ArrowLeft class="w-4 h-4 mr-1" />
          돌아가기
        </Button>
        <span class="px-2 py-1 text-xs bg-blue-500/20 text-blue-400 rounded-full">
          백테스트
        </span>
      </Show>
      <Show when={!isBacktest()}>
        {/* 실환경 모드: 동기화 버튼들 */}
        <Button variant="primary" onClick={() => handleSync(false)} disabled={loading.isSyncing} loading={loading.isSyncing}>
          🔄 동기화
        </Button>
        <Button
          variant="secondary"
          onClick={() => handleSync(true)}
          disabled={loading.isSyncing}
          title="캐시를 초기화하고 전체 체결 내역을 다시 조회합니다 (ISA 계좌 등)"
        >
          🔄 강제 동기화
        </Button>
        <Button variant="ghost" onClick={handleClearCache} disabled={loading.isSyncing}>
          🗑️ 캐시 초기화
        </Button>
      </Show>
      <Button variant="secondary" onClick={handleRefresh} disabled={loading.isRefreshing} loading={loading.isRefreshing}>
        🔃 새로고침
      </Button>
    </div>
  )

  /** 페이지 제목 */
  const pageTitle = createMemo(() => {
    if (isBacktest()) {
      const name = backtestStrategyName()
      return name ? `매매일지 - ${name}` : '매매일지 (백테스트)'
    }
    return '매매일지'
  })

  /** 페이지 설명 */
  const pageDescription = createMemo(() => {
    if (isBacktest()) {
      return '백테스트 결과의 체결 내역과 손익을 분석합니다'
    }
    return '체결 내역과 손익을 분석합니다'
  })

  return (
    <div class="space-y-6">
      {/* 헤더 - 공통 컴포넌트 사용 */}
      <PageHeader
        title={pageTitle()}
        icon="📘"
        description={pageDescription()}
        actions={<HeaderActions />}
      />

      {/* PnL 요약 카드 - 공통 컴포넌트 사용 */}
      <StatCardGrid columns={4}>
        <StatCard
          label="총 실현손익"
          value={pnlSummary() ? formatCurrency(pnlSummary()!.net_pnl) : '-'}
          icon="💰"
          valueColor={getPnLColor(pnlSummary()?.net_pnl || '0')}
        />
        <StatCard
          label="총 거래"
          value={pnlSummary()?.total_trades || 0}
          icon="📊"
        />
        <StatCard
          label="승률"
          value={`${pnlSummary()?.win_rate || '0.00'}%`}
          icon="📈"
        />
        <StatCard
          label="총 수수료"
          value={pnlSummary() ? formatCurrency(pnlSummary()!.total_fees) : '-'}
          icon="⚠️"
          valueColor="text-orange-400"
        />
      </StatCardGrid>

      {/* 탭 네비게이션 (백테스트: 4개, 실환경: 5개) */}
      <div class="bg-gray-800 rounded-xl">
        <div class="flex overflow-x-auto border-b border-gray-700 scrollbar-thin scrollbar-thumb-gray-700">
          {/* 보유 현황 탭 - 실환경에서만 표시 */}
          <Show when={!isBacktest()}>
            <button
              type="button"
              onClick={() => setUI('activeTab', 'positions')}
              class={`flex items-center gap-2 px-5 py-4 text-sm font-medium transition-colors whitespace-nowrap ${
                ui.activeTab === 'positions'
                  ? 'text-blue-400 border-b-2 border-blue-400'
                  : 'text-gray-400 hover:text-gray-300'
              }`}
            >
              <BookOpen class="w-4 h-4" />
              보유 현황
            </button>
          </Show>
          <button
            type="button"
            onClick={() => setUI('activeTab', 'executions')}
            class={`flex items-center gap-2 px-5 py-4 text-sm font-medium transition-colors whitespace-nowrap ${
              ui.activeTab === 'executions'
                ? 'text-blue-400 border-b-2 border-blue-400'
                : 'text-gray-400 hover:text-gray-300'
            }`}
          >
            <BarChart3 class="w-4 h-4" />
            체결 내역
          </button>
          <button
            type="button"
            onClick={() => setUI('activeTab', 'pnl-analysis')}
            class={`flex items-center gap-2 px-5 py-4 text-sm font-medium transition-colors whitespace-nowrap ${
              ui.activeTab === 'pnl-analysis'
                ? 'text-green-400 border-b-2 border-green-400'
                : 'text-gray-400 hover:text-gray-300'
            }`}
          >
            <LineChart class="w-4 h-4" />
            손익 분석
          </button>
          <button
            type="button"
            onClick={() => setUI('activeTab', 'symbols')}
            class={`flex items-center gap-2 px-5 py-4 text-sm font-medium transition-colors whitespace-nowrap ${
              ui.activeTab === 'symbols'
                ? 'text-purple-400 border-b-2 border-purple-400'
                : 'text-gray-400 hover:text-gray-300'
            }`}
          >
            <PieChart class="w-4 h-4" />
            종목별
          </button>
          <button
            type="button"
            onClick={() => setUI('activeTab', 'strategy-insights')}
            class={`flex items-center gap-2 px-5 py-4 text-sm font-medium transition-colors whitespace-nowrap ${
              ui.activeTab === 'strategy-insights'
                ? 'text-yellow-400 border-b-2 border-yellow-400'
                : 'text-gray-400 hover:text-gray-300'
            }`}
          >
            <Lightbulb class="w-4 h-4" />
            전략 분석
          </button>
        </div>

        {/* 탭 컨텐츠 (Lazy Loaded Components) */}
        <div class="p-4">
          <Show when={ui.activeTab === 'positions'}>
            <div class="space-y-4">
              {/* 포지션 비중 도넛 차트 (클릭 시 상세 모달) */}
              <Suspense fallback={<div class="h-[200px] bg-gray-100 dark:bg-gray-800 animate-pulse rounded" />}>
                <PositionDonutChart
                  positions={positions()?.positions || []}
                  onSymbolClick={openPositionModal}
                />
              </Suspense>
              {/* 포지션 테이블 (클릭 시 상세 모달) */}
              <Suspense fallback={<div class="h-[300px] bg-gray-100 dark:bg-gray-800 animate-pulse rounded" />}>
                <PositionsTable
                  positions={positions()?.positions || []}
                  onRowClick={openPositionModal}
                />
              </Suspense>
            </div>
          </Show>
          <Show when={ui.activeTab === 'executions'}>
            <Suspense fallback={<div class="h-[400px] bg-gray-100 dark:bg-gray-800 animate-pulse rounded" />}>
              <ExecutionsTable
                executions={executions()?.executions || []}
                onRefetch={refetchExecutions}
                symbolFilter={filters.symbol}
                setSymbolFilter={handleSymbolFilterChange}
                sideFilter={filters.side}
                setSideFilter={handleSideFilterChange}
                total={executions()?.total || 0}
                currentPage={filters.currentPage}
                pageSize={filters.pageSize}
                onPageChange={handlePageChange}
                startDate={filters.startDate}
                endDate={filters.endDate}
                setStartDate={handleStartDateChange}
                setEndDate={handleEndDateChange}
                isBacktest={isBacktest()}
              />
            </Suspense>
          </Show>
          <Show when={ui.activeTab === 'pnl-analysis'}>
            <Suspense fallback={<div class="h-[500px] bg-gray-100 dark:bg-gray-800 animate-pulse rounded" />}>
              <PnLAnalysisPanel
                cumulativeData={cumulativePnL()?.curve || []}
                dailyData={dailyPnL()?.daily || []}
                weeklyData={weeklyPnL()?.weekly || []}
                monthlyData={monthlyPnL()?.monthly || []}
                yearlyData={yearlyPnL()?.yearly || []}
                insights={insights()}
              />
            </Suspense>
          </Show>
          <Show when={ui.activeTab === 'symbols'}>
            <Suspense fallback={<div class="h-[300px] bg-gray-100 dark:bg-gray-800 animate-pulse rounded" />}>
              <SymbolPnLTable symbols={symbolPnL()?.symbols || []} />
            </Suspense>
          </Show>
          <Show when={ui.activeTab === 'strategy-insights'}>
            <Suspense fallback={<div class="h-[400px] bg-gray-100 dark:bg-gray-800 animate-pulse rounded" />}>
              <StrategyInsightsPanel
                insights={insights() || null}
                strategies={strategyPerformance()?.strategies || []}
              />
            </Suspense>
          </Show>
        </div>
      </div>

      {/* 포지션 요약 (보유 현황 탭에서만) */}
      <Show when={ui.activeTab === 'positions' && positions()?.summary}>
        <Card padding="lg">
          <CardHeader>
            <h3 class="text-lg font-semibold text-gray-900 dark:text-white">포지션 요약</h3>
          </CardHeader>
          <CardContent>
            <div class="grid grid-cols-2 md:grid-cols-5 gap-4">
              <div>
                <div class="text-gray-500 dark:text-gray-400 text-sm mb-1">보유 종목 수</div>
                <div class="text-gray-900 dark:text-white font-medium">{positions()?.summary.total_positions || 0}</div>
              </div>
              <div>
                <div class="text-gray-500 dark:text-gray-400 text-sm mb-1">총 매입금액</div>
                <div class="text-gray-900 dark:text-white font-medium">
                  {positions()?.summary ? formatCurrency(positions()!.summary.total_cost_basis) : '-'}
                </div>
              </div>
              <div>
                <div class="text-gray-500 dark:text-gray-400 text-sm mb-1">총 평가금액</div>
                <div class="text-gray-900 dark:text-white font-medium">
                  {positions()?.summary ? formatCurrency(positions()!.summary.total_market_value) : '-'}
                </div>
              </div>
              <div>
                <div class="text-gray-500 dark:text-gray-400 text-sm mb-1">평가손익</div>
                <div class={`font-medium ${getPnLColor(parseFloat(positions()?.summary?.total_unrealized_pnl || '0'))}`}>
                  {positions()?.summary ? formatCurrency(positions()!.summary.total_unrealized_pnl) : '-'}
                </div>
              </div>
              <div>
                <div class="text-gray-500 dark:text-gray-400 text-sm mb-1">수익률</div>
                <div class={`font-medium ${getPnLColor(parseFloat(positions()?.summary?.total_unrealized_pnl_pct || '0'))}`}>
                  {positions()?.summary ? `${positions()!.summary.total_unrealized_pnl_pct}%` : '-'}
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      </Show>

      {/* 포지션 상세 모달 (Lazy Loaded) */}
      <Suspense fallback={null}>
        <PositionDetailModal
          isOpen={modal.position.open}
          position={modal.position.data}
          onClose={closePositionModal}
        />
      </Suspense>
    </div>
  )
}

export default TradingJournal
