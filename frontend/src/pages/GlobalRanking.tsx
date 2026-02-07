/**
 * Global Ranking 페이지
 *
 * GlobalScore 기반 종목 랭킹을 조회하고 표시합니다.
 * 시장별 필터링, 등급별 필터링, 상세 점수 분석을 제공합니다.
 */
import { type Component, createSignal, createMemo, For, Show, lazy, Suspense, createEffect, on } from 'solid-js'
import {
  Card,
  CardHeader,
  CardContent,
  GlobalScoreBadge,
  GlobalScoreBar,
  VirtualizedTable,
  PageLoader,
  formatNumber,
  PageHeader,
  FilterPanel,
  Select,
  Input,
  Button,
  StatCard,
  StatCardGrid,
  EmptyState,
  ErrorState,
  MarketBadge,
  ConfidenceBadge,
  RadarChart,
  RankChangeIndicator,
  FavoriteButton,
  getFavorites,
  ExportButton,
  AutoRefreshToggle,
} from '../components/ui'
import type { VirtualColumn } from '../components/ui'

// Lazy load heavy chart component
const ScoreWaterfall = lazy(() =>
  import('../components/charts/ScoreWaterfall').then(m => ({ default: m.ScoreWaterfall }))
)
import type { WaterfallDataItem } from '../components/charts'
import type { ExportColumn } from '../components/ui'
import type { ConfidenceLevel as BadgeConfLevel } from '../components/ui'
import type { RankedSymbol, RankingResponse, ComponentScores } from '../types'
import { createLogger } from '../utils/logger'

const { log } = createLogger('GlobalRanking')

// ==================== API 함수 ====================

const API_BASE = '/api/v1'

// 페이지 크기 (무한 스크롤)
const PAGE_SIZE = 50

async function fetchRanking(params: {
  market?: string
  grade?: string
  min_score?: string
  limit?: number
  offset?: number
  route_state?: string
}): Promise<RankingResponse> {
  const searchParams = new URLSearchParams()
  if (params.market) searchParams.set('market', params.market)
  if (params.grade) searchParams.set('grade', params.grade)
  if (params.min_score) searchParams.set('min_score', params.min_score)
  if (params.limit) searchParams.set('limit', params.limit.toString())
  if (params.offset !== undefined) searchParams.set('offset', params.offset.toString())
  if (params.route_state) searchParams.set('route_state', params.route_state)

  const response = await fetch(`${API_BASE}/ranking/top?${searchParams}`)
  if (!response.ok) {
    throw new Error(`랭킹 조회 실패: ${response.statusText}`)
  }
  return response.json()
}

// ==================== 상수 ====================

const MARKET_OPTIONS = [
  { value: '', label: '전체 시장' },
  { value: 'KR', label: '🇰🇷 한국 (KRX)' },
  { value: 'US', label: '🇺🇸 미국 (NYSE/NASDAQ)' },
  { value: 'CRYPTO', label: '₿ 암호화폐' },
]

const GRADE_OPTIONS = [
  { value: '', label: '전체 등급' },
  { value: 'EXCELLENT', label: '🏆 EXCELLENT' },
  { value: 'BUY', label: '🟢 BUY' },
  { value: 'WATCH', label: '👀 WATCH' },
  { value: 'HOLD', label: '⏸️ HOLD' },
  { value: 'CAUTION', label: '⚠️ CAUTION' },
  { value: 'AVOID', label: '🔴 AVOID' },
]

const ROUTE_STATE_OPTIONS = [
  { value: '', label: '전체 상태' },
  { value: 'ATTACK', label: '🔥 ATTACK' },
  { value: 'ARMED', label: '⚡ ARMED' },
  { value: 'WATCH', label: '👁️ WATCH' },
  { value: 'REST', label: '😴 REST' },
]

// ==================== 서브 컴포넌트 ====================

/**
 * 구성 점수 표시 컴포넌트 (레이더 차트 + 텍스트)
 */
const ComponentScoreDisplay: Component<{ scores: ComponentScores; showChart?: boolean }> = (props) => {
  const entries = createMemo(() => {
    const result: { key: string; value: number }[] = []
    const scores = props.scores
    for (const key in scores) {
      if (scores[key] !== undefined) {
        result.push({ key, value: scores[key] as number })
      }
    }
    return result
  })

  // 차트 없이 텍스트만 표시 (테이블용)
  if (!props.showChart) {
    return (
      <div class="flex flex-wrap gap-2">
        <For each={entries()}>
          {(entry) => (
            <div class="flex items-center gap-1.5 text-xs">
              <span class="text-gray-500 dark:text-gray-400 capitalize">{entry.key}:</span>
              <span class="font-medium text-gray-700 dark:text-gray-300">
                {formatNumber(entry.value, { decimals: 0 })}
              </span>
            </div>
          )}
        </For>
      </div>
    )
  }

  // 레이더 차트 + 텍스트 표시
  return (
    <div class="flex items-center gap-4">
      <RadarChart data={props.scores} size={100} showLabels={false} />
      <div class="flex flex-col gap-1">
        <For each={entries()}>
          {(entry) => (
            <div class="flex items-center gap-1.5 text-xs">
              <span class="text-gray-500 dark:text-gray-400 capitalize w-16">{entry.key}:</span>
              <div class="w-16 h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                <div
                  class="h-full bg-blue-500 rounded-full transition-all"
                  style={{ width: `${entry.value}%` }}
                />
              </div>
              <span class="font-medium text-gray-700 dark:text-gray-300 w-6 text-right">
                {formatNumber(entry.value, { decimals: 0 })}
              </span>
            </div>
          )}
        </For>
      </div>
    </div>
  )
}

/**
 * TOP 10 랭킹 카드 (레이더 차트 포함)
 */
const TopRankCard: Component<{ symbol: RankedSymbol; rank: number; onClick?: () => void }> = (props) => {
  const medalEmoji = () => {
    switch (props.rank) {
      case 1: return '🥇'
      case 2: return '🥈'
      case 3: return '🥉'
      default: return `#${props.rank}`
    }
  }

  // 등급별 레이더 차트 색상 (메모이제이션으로 불필요한 객체 생성 방지)
  const chartColors = createMemo(() => {
    const score = props.symbol.overall_score
    if (score >= 80) return { fill: 'rgba(34, 197, 94, 0.3)', stroke: 'rgb(34, 197, 94)' } // 초록
    if (score >= 60) return { fill: 'rgba(59, 130, 246, 0.3)', stroke: 'rgb(59, 130, 246)' } // 파랑
    if (score >= 40) return { fill: 'rgba(234, 179, 8, 0.3)', stroke: 'rgb(234, 179, 8)' } // 노랑
    return { fill: 'rgba(239, 68, 68, 0.3)', stroke: 'rgb(239, 68, 68)' } // 빨강
  })

  return (
    <div
      onClick={() => props.onClick?.()}
      class={`
        p-4 rounded-lg border transition-shadow hover:shadow-md cursor-pointer
        ${props.rank <= 3
          ? 'border-yellow-300 dark:border-yellow-600 bg-yellow-50 dark:bg-yellow-900/20'
          : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800'}
      `}
    >
      {/* 상단: 순위, 종목명, 점수 */}
      <div class="flex items-start justify-between mb-2">
        <div class="flex items-center gap-2">
          <span class="text-lg font-bold">{medalEmoji()}</span>
          <div>
            <div class="flex items-center gap-1.5">
              <span class="font-semibold text-gray-900 dark:text-white">{props.symbol.ticker}</span>
              <RankChangeIndicator change={props.symbol.rank_change} size="xs" />
            </div>
            <div class="text-xs text-gray-500 dark:text-gray-400 truncate max-w-[100px]">
              {props.symbol.name}
            </div>
          </div>
        </div>
        <GlobalScoreBadge score={props.symbol.overall_score} size="sm" />
      </div>

      {/* 중앙: 레이더 차트 */}
      <div class="flex justify-center my-2">
        <RadarChart
          data={props.symbol.component_scores}
          size={80}
          fillColor={chartColors().fill}
          strokeColor={chartColors().stroke}
          showLabels={false}
        />
      </div>

      {/* 하단: 점수 바 & 시장 정보 */}
      <GlobalScoreBar score={props.symbol.overall_score} showLabel={false} height={4} />
      <div class="mt-2 flex justify-between text-xs text-gray-500 dark:text-gray-400">
        <span>{props.symbol.market}</span>
        <span>{formatNumber(props.symbol.overall_score, { decimals: 1 })}점</span>
      </div>
    </div>
  )
}

// ==================== 메인 페이지 ====================

const GlobalRanking: Component = () => {
  // 필터 상태
  const [market, setMarket] = createSignal('')
  const [grade, setGrade] = createSignal('')
  const [minScore, setMinScore] = createSignal('')
  const [routeState, setRouteState] = createSignal('')

  // 선택된 종목 (점수 분석용)
  const [selectedSymbol, setSelectedSymbol] = createSignal<RankedSymbol | null>(null)

  // 선택된 종목의 워터폴 데이터 생성 (7팩터 시스템)
  const waterfallData = createMemo((): WaterfallDataItem[] => {
    const symbol = selectedSymbol()
    if (!symbol) return []

    const scores = symbol.component_scores
    const items: WaterfallDataItem[] = []

    // 7팩터 구성 점수를 워터폴 데이터로 변환 (가중치 순)
    // risk_reward(25%), target_room(18%), liquidity(13%), stop_room(12%), entry_proximity(12%), momentum(10%), technical_balance(10%)
    if (scores.risk_reward !== undefined) items.push({ name: '리스크보상', value: scores.risk_reward, color: '#3b82f6' })
    if (scores.target_room !== undefined) items.push({ name: '목표여력', value: scores.target_room, color: '#22c55e' })
    if (scores.liquidity !== undefined) items.push({ name: '유동성', value: scores.liquidity, color: '#f59e0b' })
    if (scores.stop_room !== undefined) items.push({ name: '손절여력', value: scores.stop_room, color: '#ef4444' })
    if (scores.entry_proximity !== undefined) items.push({ name: '진입근접', value: scores.entry_proximity, color: '#8b5cf6' })
    if (scores.momentum !== undefined) items.push({ name: '모멘텀', value: scores.momentum, color: '#06b6d4' })
    if (scores.technical_balance !== undefined) items.push({ name: '기술균형', value: scores.technical_balance, color: '#84cc16' })
    // penalties는 마이너스 값이므로 별도 처리 (표시 안함 or 음수로 표시)
    // if (scores.penalties !== undefined) items.push({ name: '패널티', value: scores.penalties, color: '#dc2626' })

    return items
  })

  // ==================== 무한 스크롤 상태 ====================
  const [rankingResults, setRankingResults] = createSignal<RankedSymbol[]>([])
  const [rankingTotal, setRankingTotal] = createSignal(0)
  const [rankingOffset, setRankingOffset] = createSignal(0)
  const [isLoading, setIsLoading] = createSignal(false)
  const [isLoadingMore, setIsLoadingMore] = createSignal(false)
  const [error, setError] = createSignal<Error | null>(null)

  // 데이터 로드 함수
  const loadRankingData = async (reset = true) => {
    if (reset) {
      setRankingResults([])
      setRankingOffset(0)
      setIsLoading(true)
    } else {
      setIsLoadingMore(true)
    }
    setError(null)

    try {
      const offset = reset ? 0 : rankingOffset()
      const response = await fetchRanking({
        market: market() || undefined,
        grade: grade() || undefined,
        min_score: minScore() || undefined,
        route_state: routeState() || undefined,
        limit: PAGE_SIZE,
        offset,
      })

      if (reset) {
        setRankingResults(response.symbols)
      } else {
        setRankingResults(prev => [...prev, ...response.symbols])
      }
      setRankingTotal(response.total)
      setRankingOffset(offset + response.symbols.length)
    } catch (e) {
      setError(e as Error)
      log('랭킹 로드 실패:', e)
    } finally {
      setIsLoading(false)
      setIsLoadingMore(false)
    }
  }

  // 더 로드
  const loadMore = () => {
    if (!isLoadingMore() && rankingResults().length < rankingTotal()) {
      loadRankingData(false)
    }
  }

  // 새로고침
  const refetch = () => loadRankingData(true)

  // 더 로드할 데이터가 있는지
  const hasMore = createMemo(() => rankingResults().length < rankingTotal())

  // 초기 로드 및 필터 변경 시 데이터 리로드
  createEffect(
    on(
      () => [market(), grade(), minScore(), routeState()],
      () => {
        loadRankingData(true)
      }
    )
  )

  // TOP 10 계산
  const top10 = createMemo(() => {
    const symbols = rankingResults()
    if (!symbols.length) return []
    return symbols.slice(0, 10)
  })

  // 통계 계산
  const stats = createMemo(() => {
    const symbols = rankingResults()
    if (!symbols.length) {
      return { total: 0, avgScore: 0, buyCount: 0, maxScore: 0 }
    }
    const total = rankingTotal()
    const avgScore = symbols.reduce((sum, s) => sum + s.overall_score, 0) / symbols.length
    const buyCount = symbols.filter((s) => s.grade === 'EXCELLENT' || s.grade === 'BUY').length
    const maxScore = Math.max(...symbols.map((s) => s.overall_score))
    return { total, avgScore, buyCount, maxScore }
  })

  // 즐겨찾기 변경 핸들러
  const handleFavoriteChange = (ticker: string, isFavorite: boolean) => {
    log(`${ticker} 즐겨찾기 ${isFavorite ? '추가' : '해제'}`)
  }

  // 테이블 컬럼 정의 (VirtualizedTable용)
  const columns = createMemo((): VirtualColumn<RankedSymbol>[] => [
    {
      key: 'favorite',
      header: '★',
      width: 40,
      render: (row) => (
        <FavoriteButton ticker={row.ticker} size="xs" onChange={handleFavoriteChange} />
      ),
    },
    {
      key: 'rank',
      header: '순위',
      width: 60,
      render: (_, index) => (
        <span class="font-medium text-gray-500 dark:text-gray-400">{index + 1}</span>
      ),
    },
    {
      key: 'rank_change',
      header: '변동',
      width: 60,
      render: (row) => (
        <RankChangeIndicator change={row.rank_change ?? 0} size="xs" />
      ),
    },
    {
      key: 'ticker',
      header: '종목코드',
      width: 150,
      render: (row) => (
        <div>
          <div class="font-medium text-gray-900 dark:text-white">{row.ticker}</div>
          <div class="text-xs text-gray-500 dark:text-gray-400 truncate max-w-[150px]">
            {row.name || '-'}
          </div>
        </div>
      ),
    },
    {
      key: 'market',
      header: '시장',
      width: 100,
      render: (row) => <MarketBadge market={row.market} size="xs" />,
    },
    {
      key: 'overall_score',
      header: '점수',
      width: 120,
      render: (row) => (
        row.overall_score !== undefined && row.overall_score !== null
          ? <GlobalScoreBar score={row.overall_score} showLabel height={6} />
          : <span class="text-gray-400">-</span>
      ),
    },
    {
      key: 'grade',
      header: '등급',
      width: 110,
      render: (row) => (
        row.overall_score !== undefined && row.overall_score !== null
          ? <GlobalScoreBadge score={row.overall_score} size="sm" />
          : <span class="text-gray-400">-</span>
      ),
    },
    {
      key: 'confidence',
      header: '신뢰도',
      width: 80,
      render: (row) => (
        row.confidence
          ? <ConfidenceBadge level={row.confidence as BadgeConfLevel} size="xs" />
          : <span class="text-gray-400">-</span>
      ),
    },
    {
      key: 'component_scores',
      header: '구성 점수',
      render: (row) => <ComponentScoreDisplay scores={row.component_scores} />,
    },
    {
      key: 'calculated_at',
      header: '계산 시간',
      width: 140,
      render: (row) => {
        if (!row.calculated_at) return <span class="text-gray-400">-</span>
        const date = new Date(row.calculated_at)
        if (isNaN(date.getTime())) return <span class="text-gray-400">-</span>
        return (
          <span class="text-xs text-gray-500 dark:text-gray-400">
            {date.toLocaleDateString('ko-KR')} {date.toLocaleTimeString('ko-KR', { hour: '2-digit', minute: '2-digit' })}
          </span>
        )
      },
    },
  ])

  // Excel 내보내기 컬럼 정의 (7팩터 시스템)
  const exportColumns: ExportColumn<RankedSymbol>[] = [
    { header: '순위', accessor: (_, i) => (i ?? 0) + 1 },
    { header: '티커', accessor: 'ticker' },
    { header: '종목명', accessor: 'name' },
    { header: '시장', accessor: 'market' },
    { header: '점수', accessor: (row) => row.overall_score.toFixed(1) },
    { header: '등급', accessor: 'grade' },
    { header: '신뢰도', accessor: (row) => row.confidence || '-' },
    { header: '리스크보상', accessor: (row) => row.component_scores.risk_reward?.toFixed(0) || '-' },
    { header: '목표여력', accessor: (row) => row.component_scores.target_room?.toFixed(0) || '-' },
    { header: '손절여력', accessor: (row) => row.component_scores.stop_room?.toFixed(0) || '-' },
    { header: '진입근접', accessor: (row) => row.component_scores.entry_proximity?.toFixed(0) || '-' },
    { header: '모멘텀', accessor: (row) => row.component_scores.momentum?.toFixed(0) || '-' },
    { header: '유동성', accessor: (row) => row.component_scores.liquidity?.toFixed(0) || '-' },
    { header: '기술균형', accessor: (row) => row.component_scores.technical_balance?.toFixed(0) || '-' },
    { header: '계산일시', accessor: (row) => new Date(row.calculated_at).toLocaleString('ko-KR') },
  ]

  // 헤더 액션 버튼들
  const HeaderActions = () => (
    <div class="flex items-center gap-2">
      <AutoRefreshToggle
        onRefresh={() => refetch()}
        isRefreshing={isLoading()}
        size="sm"
      />
      <ExportButton
        data={rankingResults()}
        columns={exportColumns}
        filename={`global_ranking_${new Date().toISOString().slice(0, 10)}`}
        label="Excel"
        size="sm"
        variant="secondary"
      />
      <Button
        variant="primary"
        onClick={() => refetch()}
        disabled={isLoading()}
        loading={isLoading()}
      >
        🔄 새로고침
      </Button>
    </div>
  )

  return (
    <div class="p-6 space-y-6">
      {/* 페이지 헤더 - 공통 컴포넌트 사용 */}
      <PageHeader
        title="Global Ranking"
        icon="🏆"
        description="GlobalScore 7팩터 랭킹 - 리스크보상, 목표여력, 손절여력, 진입근접, 모멘텀, 유동성, 기술균형을 종합 평가합니다."
        actions={<HeaderActions />}
      />

      {/* 필터 - 공통 컴포넌트 사용 */}
      <FilterPanel>
        <Select
          label="시장"
          value={market()}
          onChange={setMarket}
          options={MARKET_OPTIONS}
        />
        <Select
          label="등급"
          value={grade()}
          onChange={setGrade}
          options={GRADE_OPTIONS}
        />
        <Select
          label="상태"
          value={routeState()}
          onChange={setRouteState}
          options={ROUTE_STATE_OPTIONS}
        />
        <Input
          label="최소 점수"
          type="number"
          value={minScore()}
          onInput={setMinScore}
          min={0}
          max={100}
          step={5}
          placeholder="0"
          width="w-24"
        />
      </FilterPanel>

      {/* 로딩 상태 */}
      <Show when={isLoading() && rankingResults().length === 0}>
        <PageLoader message="랭킹 데이터를 불러오는 중..." />
      </Show>

      {/* 에러 상태 - 공통 컴포넌트 사용 */}
      <Show when={error()}>
        <Card>
          <CardContent>
            <ErrorState
              title="데이터 로드 실패"
              message={error()?.message || '알 수 없는 오류'}
              onRetry={() => refetch()}
            />
          </CardContent>
        </Card>
      </Show>

      {/* 데이터 표시 */}
      <Show when={rankingResults().length > 0 || !isLoading()}>
        {/* 요약 통계 - 공통 StatCard 사용 */}
        <StatCardGrid columns={4}>
          <StatCard label="총 종목" value={stats().total} icon="📊" />
          <StatCard
            label="평균 점수"
            value={stats().avgScore > 0 ? formatNumber(stats().avgScore, { decimals: 1 }) : '-'}
            icon="📈"
            valueColor="text-blue-600 dark:text-blue-400"
          />
          <StatCard
            label="BUY 이상"
            value={stats().buyCount}
            icon="🟢"
            valueColor="text-green-600 dark:text-green-400"
          />
          <StatCard
            label="최고 점수"
            value={stats().maxScore > 0 ? formatNumber(stats().maxScore, { decimals: 1 }) : '-'}
            icon="🏅"
            valueColor="text-purple-600 dark:text-purple-400"
          />
        </StatCardGrid>

        {/* TOP 10 하이라이트 + 점수 분석 패널 */}
        <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* TOP 10 카드 그리드 */}
          <div class="lg:col-span-2">
            <Card>
              <CardHeader>
                <div class="flex items-center gap-2">
                  <span class="text-xl">🏅</span>
                  <span class="font-semibold text-gray-900 dark:text-white">TOP 10</span>
                  <span class="text-xs text-gray-500 dark:text-gray-400">(클릭하여 점수 분석)</span>
                </div>
              </CardHeader>
              <CardContent>
                <Show
                  when={top10().length > 0}
                  fallback={
                    <EmptyState
                      icon="📭"
                      title="조건에 맞는 종목이 없습니다"
                      description="필터 조건을 변경해 보세요."
                    />
                  }
                >
                  <div class="grid grid-cols-2 md:grid-cols-5 gap-4">
                    <For each={top10()}>
                      {(symbol, index) => (
                        <TopRankCard
                          symbol={symbol}
                          rank={index() + 1}
                          onClick={() => setSelectedSymbol(symbol)}
                        />
                      )}
                    </For>
                  </div>
                </Show>
              </CardContent>
            </Card>
          </div>

          {/* 점수 분석 패널 (ScoreWaterfall) */}
          <div class="lg:col-span-1">
            <Card>
              <CardHeader>
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-2">
                    <span class="text-xl">📊</span>
                    <span class="font-semibold text-gray-900 dark:text-white">점수 분석</span>
                  </div>
                  <Show when={selectedSymbol()}>
                    <button
                      onClick={() => setSelectedSymbol(null)}
                      class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
                      title="닫기"
                    >
                      ✕
                    </button>
                  </Show>
                </div>
              </CardHeader>
              <CardContent>
                <Show
                  when={selectedSymbol()}
                  fallback={
                    <div class="text-center py-8 text-gray-500 dark:text-gray-400">
                      <p class="text-sm">종목을 선택하면</p>
                      <p class="text-sm">점수 구성을 분석합니다</p>
                    </div>
                  }
                >
                  {(symbol) => (
                    <div class="space-y-4">
                      {/* 종목 정보 */}
                      <div class="flex items-center justify-between">
                        <div>
                          <div class="font-semibold text-gray-900 dark:text-white">{symbol().ticker}</div>
                          <div class="text-xs text-gray-500 dark:text-gray-400">{symbol().name}</div>
                        </div>
                        <GlobalScoreBadge score={symbol().overall_score} size="md" />
                      </div>

                      {/* 워터폴 차트 (Lazy Loaded) */}
                      <Suspense fallback={<div class="h-[250px] bg-gray-100 dark:bg-gray-800 animate-pulse rounded-lg" />}>
                        <ScoreWaterfall
                          data={waterfallData()}
                          total={symbol().overall_score}
                          height={250}
                          title="점수 구성"
                          startLabel="기준"
                          endLabel="총점"
                        />
                      </Suspense>

                      {/* 상세 점수 표 */}
                      <div class="text-xs space-y-1">
                        <For each={waterfallData()}>
                          {(item) => {
                            const value = Number(item.value ?? 0)
                            return (
                              <div class="flex justify-between items-center py-1 border-b border-gray-100 dark:border-gray-700">
                                <span class="text-gray-600 dark:text-gray-400">{item.name}</span>
                                <span
                                  class={`font-medium ${
                                    value >= 0
                                      ? 'text-green-600 dark:text-green-400'
                                      : 'text-red-600 dark:text-red-400'
                                  }`}
                                >
                                  {value >= 0 ? '+' : ''}{value.toFixed(1)}
                                </span>
                              </div>
                            )
                          }}
                        </For>
                      </div>
                    </div>
                  )}
                </Show>
              </CardContent>
            </Card>
          </div>
        </div>

        {/* 전체 랭킹 테이블 - VirtualizedTable로 무한 스크롤 지원 */}
        <Card>
          <CardHeader>
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-2">
                <span class="text-xl">📊</span>
                <span class="font-semibold text-gray-900 dark:text-white">전체 랭킹</span>
              </div>
              <span class="text-sm text-gray-500 dark:text-gray-400">
                {rankingResults().length} / {rankingTotal()}개 종목
              </span>
            </div>
          </CardHeader>
          <CardContent>
            <VirtualizedTable
              data={rankingResults()}
              columns={columns()}
              height={600}
              rowHeight={60}
              onRowClick={(row) => setSelectedSymbol(row)}
              onEndReached={loadMore}
              hasMore={hasMore()}
              isLoading={isLoadingMore()}
              loadingMessage="추가 데이터 로딩 중..."
              endReachedThreshold={200}
            />
            <Show when={rankingResults().length === 0 && !isLoading()}>
              <EmptyState
                icon="📭"
                title="조건에 맞는 종목이 없습니다"
                description="필터 조건을 변경해 보세요."
              />
            </Show>
          </CardContent>
        </Card>
      </Show>
    </div>
  )
}

export default GlobalRanking
