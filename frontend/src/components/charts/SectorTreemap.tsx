/**
 * Sector Treemap 컴포넌트
 *
 * 섹터별 시장 데이터를 트리맵으로 시각화합니다.
 * 섹터 상대강도, 시가총액 비중, 수익률 등을 한눈에 파악할 수 있습니다.
 *
 * @example
 * ```tsx
 * <SectorTreemap market="KR" metric="performance" />
 * // or
 * <SectorTreemap
 *   data={[
 *     { sector: '반도체', symbolCount: 15, avgReturn: 2.5, totalMarketCap: 500000 },
 *   ]}
 * />
 * ```
 */
import { createResource, createMemo, Show } from 'solid-js'
import type { Component, JSX } from 'solid-js'
import { TreemapChart, type TreemapDataItem } from '../ui/TreemapChart'
import { getSectorRanking, type SectorRsDto } from '../../api/client'
import { createLogger } from '../../utils/logger'

const { error: logError } = createLogger('SectorTreemap')

/** 섹터 데이터 항목 */
export interface SectorDataItem {
  /** 섹터명 */
  sector: string
  /** 종목 수 */
  symbolCount: number
  /** 평균 수익률 (%) */
  avgReturn: number
  /** 총 시가총액 (억원) */
  totalMarketCap?: number
  /** 상대강도 순위 */
  rsRank?: number
  /** RS 스코어 */
  rsScore?: number
}

/** 표시 메트릭 */
export type SectorMetric = 'performance' | 'market-cap' | 'symbol-count' | 'rs-score'

/** 섹터별 색상 테마 */
const SECTOR_COLORS: Record<string, string> = {
  '반도체': '#3b82f6',
  '2차전지': '#22c55e',
  '바이오': '#ec4899',
  '자동차': '#f59e0b',
  'IT': '#8b5cf6',
  '금융': '#06b6d4',
  '건설': '#84cc16',
  '화학': '#f97316',
  '철강': '#64748b',
  '유통': '#14b8a6',
  '음식료': '#a855f7',
  '기계': '#6366f1',
  '전기전자': '#0ea5e9',
  '의약품': '#d946ef',
  '섬유': '#78716c',
  '통신': '#22d3ee',
  '기타': '#9ca3af',
}

export interface SectorTreemapProps {
  /** 수동 데이터 (지정하지 않으면 API에서 로드) */
  data?: SectorDataItem[]
  /** 시장 (KR/US) */
  market?: string
  /** 기간 (일) */
  days?: number
  /** 표시 메트릭 */
  metric?: SectorMetric
  /** 차트 높이 */
  height?: number
  /** 제목 */
  title?: string
  /** 색상 범위 표시 */
  showVisualMap?: boolean
  /** 클릭 핸들러 */
  onSectorClick?: (sector: string) => void
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

/**
 * Sector Treemap 컴포넌트
 *
 * 섹터 분석 데이터를 트리맵으로 시각화합니다.
 */
export const SectorTreemap: Component<SectorTreemapProps> = (props) => {
  // API에서 섹터 데이터 로드
  const [sectorData] = createResource(
    () => ({
      market: props.market,
      days: props.days,
      hasManualData: !!props.data,
    }),
    async (params) => {
      if (params.hasManualData) return null
      try {
        const response = await getSectorRanking(params.market, params.days)
        return response.results
      } catch (error) {
        logError('섹터 데이터 조회 실패:', error)
        return null
      }
    }
  )

  // 데이터 변환
  const processedData = createMemo((): SectorDataItem[] => {
    if (props.data) {
      return props.data
    }

    const apiData = sectorData()
    if (!apiData) return []

    return apiData.map((item: SectorRsDto, index: number) => ({
      sector: item.sector,
      symbolCount: item.symbol_count,
      avgReturn: parseFloat(item.avg_return_pct) || 0,
      rsRank: index + 1,
      rsScore: parseFloat(item.rs_score) || 0,
      totalMarketCap: item.total_market_cap ? parseFloat(item.total_market_cap) / 100000000 : undefined,
    }))
  })

  // 메트릭에 따른 값 선택
  const getMetricValue = (item: SectorDataItem): number => {
    switch (props.metric) {
      case 'market-cap':
        return item.totalMarketCap || item.symbolCount
      case 'symbol-count':
        return item.symbolCount
      case 'rs-score':
        return Math.abs(item.rsScore || 0) * 10 + 10
      case 'performance':
      default:
        return item.symbolCount // 크기는 종목 수, 색상은 수익률
    }
  }

  // 트리맵 데이터 변환
  const treemapData = createMemo((): TreemapDataItem[] => {
    const data = processedData()
    if (!data.length) return []

    return data.map((item) => ({
      name: item.sector,
      value: getMetricValue(item),
      colorValue: item.avgReturn,
      meta: {
        symbolCount: item.symbolCount,
        avgReturn: item.avgReturn,
        rsRank: item.rsRank,
        totalMarketCap: item.totalMarketCap,
      },
    }))
  })

  // 툴팁 포맷터
  const tooltipFormatter = (item: TreemapDataItem): string => {
    const meta = item.meta as SectorDataItem | undefined
    if (!meta) return `<div style="font-weight: 600;">${item.name}</div>`

    const returnColor = (meta.avgReturn || 0) >= 0 ? '#22c55e' : '#ef4444'
    const returnSign = (meta.avgReturn || 0) >= 0 ? '+' : ''

    return `
      <div style="font-weight: 600; font-size: 14px;">${item.name}</div>
      <div style="margin-top: 8px; display: grid; grid-template-columns: 1fr 1fr; gap: 4px 12px;">
        <span style="color: #9ca3af;">종목 수:</span>
        <span style="text-align: right;">${meta.symbolCount}개</span>
        <span style="color: #9ca3af;">평균 수익률:</span>
        <span style="text-align: right; color: ${returnColor};">${returnSign}${(meta.avgReturn || 0).toFixed(2)}%</span>
        ${meta.rsRank !== undefined ? `
          <span style="color: #9ca3af;">RS 순위:</span>
          <span style="text-align: right;">${meta.rsRank}위</span>
        ` : ''}
        ${meta.totalMarketCap !== undefined ? `
          <span style="color: #9ca3af;">시가총액:</span>
          <span style="text-align: right;">${(meta.totalMarketCap / 10000).toFixed(1)}조원</span>
        ` : ''}
      </div>
    `
  }

  // 클릭 핸들러
  const handleClick = (item: TreemapDataItem) => {
    if (props.onSectorClick) {
      props.onSectorClick(item.name)
    }
  }

  // 메트릭별 제목
  const chartTitle = createMemo(() => {
    if (props.title) return props.title
    switch (props.metric) {
      case 'market-cap':
        return '섹터별 시가총액'
      case 'symbol-count':
        return '섹터별 종목 수'
      case 'rs-score':
        return '섹터 상대강도'
      case 'performance':
      default:
        return '섹터별 수익률'
    }
  })

  const isLoading = createMemo(() => sectorData.loading)
  const chartHeight = createMemo(() => props.height || 400)

  return (
    <div
      class={`rounded-xl overflow-hidden ${props.class || ''}`}
      style={props.style}
    >
      <Show
        when={!isLoading()}
        fallback={
          <div
            class="flex items-center justify-center bg-gray-800/50 rounded-xl"
            style={{ height: `${chartHeight()}px` }}
          >
            <div class="flex flex-col items-center gap-2">
              <div class="w-8 h-8 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" />
              <span class="text-sm text-gray-400">섹터 데이터 로딩 중...</span>
            </div>
          </div>
        }
      >
        <TreemapChart
          data={treemapData()}
          height={chartHeight()}
          title={chartTitle()}
          showVisualMap={props.showVisualMap ?? true}
          visualMapMin={-5}
          visualMapMax={5}
          tooltipFormatter={tooltipFormatter}
          onClick={handleClick}
        />
      </Show>
    </div>
  )
}

/**
 * 섹터 성과 요약 카드
 *
 * 섹터 데이터를 카드 형태로 요약하여 표시합니다.
 */
export interface SectorSummaryCardProps {
  /** 섹터명 */
  sector: string
  /** 종목 수 */
  symbolCount: number
  /** 평균 수익률 */
  avgReturn: number
  /** RS 순위 */
  rsRank?: number
  /** 클릭 핸들러 */
  onClick?: () => void
  /** 추가 클래스 */
  class?: string
}

export const SectorSummaryCard: Component<SectorSummaryCardProps> = (props) => {
  const returnColor = createMemo(() => props.avgReturn >= 0 ? 'text-green-400' : 'text-red-400')
  const sectorColor = createMemo(() => SECTOR_COLORS[props.sector] || SECTOR_COLORS['기타'])

  return (
    <div
      class={`
        p-4 rounded-xl border border-gray-700 bg-gray-800/50
        ${props.onClick ? 'cursor-pointer hover:bg-gray-700/50 transition-colors' : ''}
        ${props.class || ''}
      `}
      onClick={props.onClick}
    >
      <div class="flex items-center justify-between mb-2">
        <div class="flex items-center gap-2">
          <div
            class="w-3 h-3 rounded-full"
            style={{ 'background-color': sectorColor() }}
          />
          <span class="font-medium text-white">{props.sector}</span>
        </div>
        <Show when={props.rsRank !== undefined}>
          <span class="text-xs text-gray-400">#{props.rsRank}</span>
        </Show>
      </div>
      <div class="flex items-center justify-between text-sm">
        <span class="text-gray-400">{props.symbolCount}종목</span>
        <span class={`font-mono font-medium ${returnColor()}`}>
          {props.avgReturn >= 0 ? '+' : ''}{props.avgReturn.toFixed(2)}%
        </span>
      </div>
    </div>
  )
}

export default SectorTreemap
