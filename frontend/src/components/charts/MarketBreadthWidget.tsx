/**
 * Market Breadth 위젯 컴포넌트
 *
 * KOSPI, KOSDAQ, 전체 시장의 20일 이동평균선 상회 종목 비율을 표시합니다.
 * 시장 온도(과열/중립/냉각)를 시각적으로 표현합니다.
 *
 * @example
 * ```tsx
 * <MarketBreadthWidget />
 * ```
 */
import { createResource, createMemo, Show, For } from 'solid-js'
import type { Component, JSX } from 'solid-js'
import { getMarketBreadth, type MarketBreadthResponse } from '../../api/client'
import { createLogger } from '../../utils/logger'

const { error: logError } = createLogger('MarketBreadthWidget')

/** 온도 레벨 */
type TemperatureLevel = 'OVERHEAT' | 'NEUTRAL' | 'COLD'

/** 온도별 스타일 정보 */
interface TemperatureStyle {
  bgColor: string
  textColor: string
  barColor: string
  icon: string
  label: string
}

const TEMPERATURE_STYLES: Record<TemperatureLevel, TemperatureStyle> = {
  OVERHEAT: {
    bgColor: 'bg-red-900/20',
    textColor: 'text-red-400',
    barColor: 'bg-red-500',
    icon: '🔥',
    label: '과열',
  },
  NEUTRAL: {
    bgColor: 'bg-yellow-900/20',
    textColor: 'text-yellow-400',
    barColor: 'bg-yellow-500',
    icon: '🌤',
    label: '중립',
  },
  COLD: {
    bgColor: 'bg-blue-900/20',
    textColor: 'text-blue-400',
    barColor: 'bg-blue-500',
    icon: '🧊',
    label: '냉각',
  },
}

/** 시장 데이터 항목 */
interface MarketItem {
  key: 'all' | 'kospi' | 'kosdaq'
  label: string
  value: number
}

export interface MarketBreadthWidgetProps {
  /** 수동 데이터 지정 (지정하지 않으면 API에서 자동 로드) */
  data?: MarketBreadthResponse
  /** 컴팩트 모드 */
  compact?: boolean
  /** 온도 표시 숨김 */
  hideTemperature?: boolean
  /** 추천 표시 숨김 */
  hideRecommendation?: boolean
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

/**
 * 프로그레스 바 컴포넌트
 */
const ProgressBar: Component<{
  value: number
  color: string
  height?: number
  showLabel?: boolean
}> = (props) => {
  const percentage = createMemo(() => Math.max(0, Math.min(100, props.value)))

  // 값에 따른 색상 결정
  const barColor = createMemo(() => {
    const v = percentage()
    if (v >= 65) return 'bg-red-500'
    if (v >= 35) return 'bg-yellow-500'
    return 'bg-blue-500'
  })

  return (
    <div class="relative w-full">
      <div
        class="w-full bg-gray-700 rounded-full overflow-hidden"
        style={{ height: `${props.height || 8}px` }}
      >
        <div
          class={`h-full transition-all duration-500 ease-out ${barColor()}`}
          style={{ width: `${percentage()}%` }}
        />
      </div>
      <Show when={props.showLabel}>
        <div class="absolute inset-0 flex items-center justify-center">
          <span class="text-xs font-medium text-white drop-shadow-lg">
            {percentage().toFixed(1)}%
          </span>
        </div>
      </Show>
    </div>
  )
}

/**
 * Market Breadth 위젯
 *
 * 시장별 20일선 상회 비율을 표시하는 대시보드 위젯입니다.
 */
export const MarketBreadthWidget: Component<MarketBreadthWidgetProps> = (props) => {
  // API에서 데이터 로드
  const [breadthData] = createResource(
    () => !props.data,
    async (shouldFetch) => {
      if (!shouldFetch) return null
      try {
        return await getMarketBreadth()
      } catch (error) {
        logError('Market Breadth 조회 실패:', error)
        return null
      }
    }
  )

  // 사용할 데이터
  const data = createMemo(() => props.data || breadthData())

  // 온도 스타일
  const tempStyle = createMemo((): TemperatureStyle => {
    const d = data()
    if (!d) return TEMPERATURE_STYLES.NEUTRAL
    const temp = d.temperature as TemperatureLevel
    return TEMPERATURE_STYLES[temp] || TEMPERATURE_STYLES.NEUTRAL
  })

  // 시장별 데이터
  const markets = createMemo((): MarketItem[] => {
    const d = data()
    if (!d) return []
    return [
      { key: 'all', label: '전체', value: parseFloat(d.all) || 0 },
      { key: 'kospi', label: 'KOSPI', value: parseFloat(d.kospi) || 0 },
      { key: 'kosdaq', label: 'KOSDAQ', value: parseFloat(d.kosdaq) || 0 },
    ]
  })

  const isLoading = createMemo(() => breadthData.loading)

  return (
    <div
      class={`rounded-xl border border-gray-700 bg-gray-800/50 overflow-hidden ${props.class || ''}`}
      style={props.style}
    >
      {/* 헤더 */}
      <div class="flex items-center justify-between px-4 py-3 border-b border-gray-700">
        <div class="flex items-center gap-2">
          <span class="text-lg">📊</span>
          <h3 class="font-medium text-white">시장 온도</h3>
        </div>
        <Show when={data() && !props.hideTemperature}>
          <div
            class={`flex items-center gap-1.5 px-2.5 py-1 rounded-full text-sm ${tempStyle().bgColor} ${tempStyle().textColor}`}
          >
            <span>{tempStyle().icon}</span>
            <span class="font-medium">{tempStyle().label}</span>
          </div>
        </Show>
      </div>

      {/* 본문 */}
      <div class="p-4">
        <Show
          when={!isLoading()}
          fallback={
            <div class="flex items-center justify-center py-8">
              <div class="flex flex-col items-center gap-2">
                <div class="w-6 h-6 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" />
                <span class="text-sm text-gray-400">로딩 중...</span>
              </div>
            </div>
          }
        >
          <Show
            when={data()}
            fallback={
              <div class="text-center py-8 text-gray-500">
                <p>데이터를 불러올 수 없습니다</p>
              </div>
            }
          >
            {/* 시장별 프로그레스 바 */}
            <div class="space-y-4">
              <For each={markets()}>
                {(market) => (
                  <div>
                    <div class="flex items-center justify-between mb-1.5">
                      <span class="text-sm text-gray-300">{market.label}</span>
                      <span
                        class={`text-sm font-mono font-medium ${
                          market.value >= 65
                            ? 'text-red-400'
                            : market.value >= 35
                            ? 'text-yellow-400'
                            : 'text-blue-400'
                        }`}
                      >
                        {market.value.toFixed(1)}%
                      </span>
                    </div>
                    <ProgressBar value={market.value} color={tempStyle().barColor} />
                  </div>
                )}
              </For>
            </div>

            {/* 온도 구간 범례 */}
            <Show when={!props.compact}>
              <div class="flex justify-center gap-4 mt-4 pt-3 border-t border-gray-700/50">
                <div class="flex items-center gap-1.5 text-xs">
                  <div class="w-2.5 h-2.5 rounded-full bg-blue-500" />
                  <span class="text-gray-400">&lt;35% 냉각</span>
                </div>
                <div class="flex items-center gap-1.5 text-xs">
                  <div class="w-2.5 h-2.5 rounded-full bg-yellow-500" />
                  <span class="text-gray-400">35-65% 중립</span>
                </div>
                <div class="flex items-center gap-1.5 text-xs">
                  <div class="w-2.5 h-2.5 rounded-full bg-red-500" />
                  <span class="text-gray-400">&gt;65% 과열</span>
                </div>
              </div>
            </Show>

            {/* 추천 */}
            <Show when={data()?.recommendation && !props.hideRecommendation}>
              <div class="mt-4 p-3 bg-gray-700/30 rounded-lg">
                <p class="text-sm text-gray-300">
                  <span class="text-gray-400">💡 추천: </span>
                  {data()!.recommendation}
                </p>
              </div>
            </Show>
          </Show>
        </Show>
      </div>

      {/* 푸터 - 업데이트 시간 */}
      <Show when={data()?.calculatedAt && !props.compact}>
        <div class="px-4 py-2 border-t border-gray-700/50 bg-gray-800/30">
          <p class="text-xs text-gray-500 text-center">
            업데이트: {new Date(data()!.calculatedAt).toLocaleString('ko-KR')}
          </p>
        </div>
      </Show>
    </div>
  )
}

export default MarketBreadthWidget
