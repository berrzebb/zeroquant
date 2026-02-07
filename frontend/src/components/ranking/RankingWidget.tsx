/**
 * TOP 10 랭킹 위젯 컴포넌트
 *
 * 대시보드에 표시할 수 있는 컴팩트한 랭킹 위젯입니다.
 * GlobalRanking API를 사용하여 TOP 10 종목을 표시합니다.
 */
import { createResource, For, Show } from 'solid-js'
import { RefreshCw, TrendingUp, ChevronRight } from 'lucide-solid'
import { GlobalScoreBadge } from '../ui'

// ts-rs에서 자동 생성된 타입 사용
import type { RankedSymbol, RankingResponse } from '../../types/generated/ranking'

interface RankingWidgetProps {
  /** 표시할 종목 수 (기본: 10) */
  limit?: number
  /** 시장 필터 */
  market?: string
  /** 자동 갱신 간격 (ms, 0이면 비활성화) */
  refreshInterval?: number
  /** "더 보기" 클릭 핸들러 */
  onViewMore?: () => void
  /** 종목 클릭 핸들러 */
  onSymbolClick?: (symbol: RankedSymbol) => void
}

// ==================== API ====================

const API_BASE = '/api/v1'

async function fetchTopRanking(limit: number, market?: string): Promise<RankingResponse> {
  const params = new URLSearchParams()
  params.set('limit', limit.toString())
  if (market) params.set('market', market)

  const response = await fetch(`${API_BASE}/ranking/top?${params}`)
  if (!response.ok) {
    throw new Error(`랭킹 조회 실패: ${response.statusText}`)
  }
  return response.json()
}

// ==================== RouteState 스타일 ====================

const ROUTE_STATE_STYLES: Record<string, { bg: string; text: string }> = {
  Attack: { bg: 'bg-red-500/20', text: 'text-red-400' },
  Armed: { bg: 'bg-orange-500/20', text: 'text-orange-400' },
  Watch: { bg: 'bg-yellow-500/20', text: 'text-yellow-400' },
  Rest: { bg: 'bg-gray-500/20', text: 'text-gray-400' },
}

// ==================== 컴포넌트 ====================

export function RankingWidget(props: RankingWidgetProps) {
  const limit = () => props.limit || 10

  // 데이터 로드
  const [ranking, { refetch }] = createResource(
    () => ({ limit: limit(), market: props.market }),
    ({ limit, market }) => fetchTopRanking(limit, market)
  )

  // 자동 갱신 (선택적)
  // TODO: 자동 갱신 구현 시 setInterval 사용

  return (
    <div class="bg-[var(--color-surface)] rounded-xl overflow-hidden">
      {/* 헤더 */}
      <div class="flex items-center justify-between px-4 py-3 border-b border-[var(--color-surface-light)]">
        <div class="flex items-center gap-2">
          <TrendingUp class="w-5 h-5 text-[var(--color-primary)]" />
          <h3 class="font-semibold text-[var(--color-text)]">TOP {limit()}</h3>
        </div>
        <div class="flex items-center gap-2">
          <button
            onClick={() => refetch()}
            disabled={ranking.loading}
            class="p-1.5 rounded-lg hover:bg-[var(--color-surface-light)] transition text-[var(--color-text-muted)]"
            title="새로고침"
          >
            <RefreshCw class={`w-4 h-4 ${ranking.loading ? 'animate-spin' : ''}`} />
          </button>
          <Show when={props.onViewMore}>
            <button
              onClick={props.onViewMore}
              class="flex items-center gap-1 text-sm text-[var(--color-primary)] hover:text-[var(--color-primary-light)] transition"
            >
              더 보기
              <ChevronRight class="w-4 h-4" />
            </button>
          </Show>
        </div>
      </div>

      {/* 로딩 상태 */}
      <Show when={ranking.loading && !ranking()}>
        <div class="p-4 space-y-2">
          <For each={Array(5)}>
            {() => (
              <div class="flex items-center gap-3 animate-pulse">
                <div class="w-6 h-6 rounded bg-[var(--color-surface-light)]" />
                <div class="flex-1 h-4 rounded bg-[var(--color-surface-light)]" />
                <div class="w-12 h-4 rounded bg-[var(--color-surface-light)]" />
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* 에러 상태 */}
      <Show when={ranking.error}>
        <div class="p-4 text-center text-[var(--color-text-muted)]">
          <p class="text-sm">데이터를 불러오지 못했습니다.</p>
          <button
            onClick={() => refetch()}
            class="mt-2 text-sm text-[var(--color-primary)] hover:underline"
          >
            다시 시도
          </button>
        </div>
      </Show>

      {/* 랭킹 목록 */}
      <Show when={ranking() && !ranking.loading}>
        <div class="divide-y divide-[var(--color-surface-light)]">
          <For each={ranking()?.symbols || []}>
            {(item, index) => (
              <button
                onClick={() => props.onSymbolClick?.(item)}
                class="w-full flex items-center gap-3 px-4 py-2.5 hover:bg-[var(--color-surface-light)]/50 transition text-left"
              >
                {/* 순위 */}
                <span class={`w-6 h-6 flex items-center justify-center rounded-full text-xs font-bold
                              ${index() < 3
                                ? 'bg-[var(--color-primary)] text-white'
                                : 'bg-[var(--color-surface-light)] text-[var(--color-text-muted)]'}`}>
                  {index() + 1}
                </span>

                {/* 종목 정보 */}
                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-2">
                    <span class="font-medium text-[var(--color-text)] truncate">{item.ticker}</span>
                    <span class="text-xs px-1.5 py-0.5 rounded bg-[var(--color-primary)]/10 text-[var(--color-primary)]">
                      {item.exchange}
                    </span>
                    <Show when={item.route_state}>
                      {(() => {
                        const style = ROUTE_STATE_STYLES[item.route_state!] || ROUTE_STATE_STYLES.Rest
                        return (
                          <span class={`text-xs px-1.5 py-0.5 rounded ${style.bg} ${style.text}`}>
                            {item.route_state}
                          </span>
                        )
                      })()}
                    </Show>
                  </div>
                  <p class="text-xs text-[var(--color-text-muted)] truncate">{item.name}</p>
                </div>

                {/* 점수 */}
                <GlobalScoreBadge score={item.overall_score} size="sm" />
              </button>
            )}
          </For>
        </div>

        {/* 빈 상태 */}
        <Show when={(ranking()?.symbols || []).length === 0}>
          <div class="p-8 text-center text-[var(--color-text-muted)]">
            <p class="text-sm">표시할 종목이 없습니다.</p>
          </div>
        </Show>
      </Show>
    </div>
  )
}

export default RankingWidget
