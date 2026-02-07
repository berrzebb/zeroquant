/**
 * Fear & Greed 게이지 컴포넌트
 *
 * Market Breadth 데이터를 기반으로 시장 심리 지수를 반원형 게이지로 시각화합니다.
 * 0~100 스케일의 5단계 색상 구분과 바늘 애니메이션을 지원합니다.
 *
 * @example
 * ```tsx
 * <FearGreedGauge value={75} label="탐욕" />
 * // or
 * <FearGreedGauge />  // 자동으로 API에서 데이터 로드
 * ```
 */
import { createResource, createMemo, Show } from 'solid-js'
import type { Component, JSX } from 'solid-js'
import { EChart, CHART_COLORS } from '../ui/EChart'
import { getMarketBreadth } from '../../api/client'
import { createLogger } from '../../utils/logger'
import type { EChartsOption } from 'echarts'

const { error: logError } = createLogger('FearGreedGauge')

/** Fear & Greed 레벨 정의 */
export type FearGreedLevel = 'extreme-fear' | 'fear' | 'neutral' | 'greed' | 'extreme-greed'

/** Fear & Greed 레벨 정보 */
interface LevelInfo {
  label: string
  color: string
  icon: string
  description: string
}

/** 레벨별 정보 매핑 */
const LEVEL_INFO: Record<FearGreedLevel, LevelInfo> = {
  'extreme-fear': {
    label: '극단적 공포',
    color: '#ef4444', // red
    icon: '😱',
    description: '매수 기회 가능성',
  },
  fear: {
    label: '공포',
    color: '#f97316', // orange
    icon: '😰',
    description: '신중한 접근 필요',
  },
  neutral: {
    label: '중립',
    color: '#eab308', // yellow
    icon: '😐',
    description: '시장 관망',
  },
  greed: {
    label: '탐욕',
    color: '#84cc16', // lime
    icon: '😊',
    description: '차익실현 고려',
  },
  'extreme-greed': {
    label: '극단적 탐욕',
    color: '#22c55e', // green
    icon: '🤑',
    description: '과열 주의',
  },
}

/** 값에 따른 레벨 결정 */
export const getLevel = (value: number): FearGreedLevel => {
  if (value <= 20) return 'extreme-fear'
  if (value <= 40) return 'fear'
  if (value <= 60) return 'neutral'
  if (value <= 80) return 'greed'
  return 'extreme-greed'
}

/** 레벨 정보 조회 */
export const getLevelInfo = (level: FearGreedLevel): LevelInfo => LEVEL_INFO[level]

export interface FearGreedGaugeProps {
  /** 수동 값 지정 (0~100). 지정하지 않으면 API에서 자동 로드 */
  value?: number
  /** 표시할 라벨 (지정하지 않으면 자동 결정) */
  label?: string
  /** 차트 높이 (기본: 200px) */
  height?: number
  /** 로딩 중 표시 여부 */
  loading?: boolean
  /** 컴팩트 모드 (작은 크기) */
  compact?: boolean
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
  /** 자동 새로고침 간격 (ms, 기본: 5분) */
  refreshInterval?: number
}

/**
 * Fear & Greed 게이지 컴포넌트
 *
 * Market Breadth의 20일선 상회 비율을 시장 심리 지수로 변환하여 표시합니다.
 * - 낮은 비율 (< 35%) → 공포 (0~40)
 * - 중간 비율 (35~65%) → 중립 (40~60)
 * - 높은 비율 (> 65%) → 탐욕 (60~100)
 */
export const FearGreedGauge: Component<FearGreedGaugeProps> = (props) => {
  // API에서 데이터 로드 (value가 지정되지 않은 경우)
  const [breadthData] = createResource(
    () => props.value === undefined,
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

  // 게이지 값 계산 (0~100)
  const gaugeValue = createMemo(() => {
    if (props.value !== undefined) {
      return Math.max(0, Math.min(100, props.value))
    }

    const data = breadthData()
    if (!data) return 50 // 기본값

    // Market Breadth의 "all" 비율을 그대로 사용 (이미 0~100 스케일)
    const allRatio = parseFloat(data.all) || 50
    return allRatio
  })

  // 현재 레벨
  const currentLevel = createMemo(() => getLevel(gaugeValue()))
  const levelInfo = createMemo(() => getLevelInfo(currentLevel()))

  // 라벨
  const displayLabel = createMemo(() => props.label || levelInfo().label)

  // ECharts 옵션
  const chartOption = createMemo((): EChartsOption => {
    const value = gaugeValue()
    const level = currentLevel()
    const info = levelInfo()

    return {
      series: [
        {
          type: 'gauge',
          startAngle: 180,
          endAngle: 0,
          center: ['50%', '75%'],
          radius: props.compact ? '90%' : '100%',
          min: 0,
          max: 100,
          splitNumber: 5,
          // 게이지 배경 색상 구간
          axisLine: {
            lineStyle: {
              width: props.compact ? 15 : 25,
              color: [
                [0.2, '#ef4444'],  // 극단적 공포 (0~20)
                [0.4, '#f97316'],  // 공포 (20~40)
                [0.6, '#eab308'],  // 중립 (40~60)
                [0.8, '#84cc16'],  // 탐욕 (60~80)
                [1, '#22c55e'],    // 극단적 탐욕 (80~100)
              ],
            },
          },
          // 바늘 스타일
          pointer: {
            icon: 'triangle',
            length: props.compact ? '60%' : '70%',
            width: props.compact ? 8 : 12,
            offsetCenter: [0, '-5%'],
            itemStyle: {
              color: 'auto',
            },
          },
          // 중심점
          anchor: {
            show: true,
            showAbove: true,
            size: props.compact ? 15 : 20,
            itemStyle: {
              borderWidth: 3,
              borderColor: info.color,
              color: '#1f2937',
            },
          },
          // 눈금
          axisTick: {
            length: props.compact ? 6 : 10,
            lineStyle: {
              color: 'auto',
              width: 1.5,
            },
          },
          // 눈금 라벨
          axisLabel: {
            color: '#9ca3af',
            fontSize: props.compact ? 10 : 12,
            distance: props.compact ? -25 : -35,
            rotate: 'tangential',
            formatter: (value: number) => {
              if (value === 0) return '공포'
              if (value === 50) return '중립'
              if (value === 100) return '탐욕'
              return ''
            },
          },
          // 분할선
          splitLine: {
            length: props.compact ? 12 : 18,
            lineStyle: {
              color: 'auto',
              width: 2,
            },
          },
          // 중앙 수치 표시
          detail: {
            valueAnimation: true,
            fontSize: props.compact ? 24 : 36,
            fontWeight: 'bold',
            color: info.color,
            offsetCenter: [0, props.compact ? '-15%' : '-20%'],
            formatter: `{value}`,
          },
          // 레이블
          title: {
            fontSize: props.compact ? 12 : 16,
            color: '#e5e7eb',
            offsetCenter: [0, props.compact ? '15%' : '20%'],
          },
          data: [
            {
              value: Math.round(value),
              name: displayLabel(),
            },
          ],
        },
      ],
      backgroundColor: 'transparent',
    }
  })

  const isLoading = createMemo(() => props.loading || breadthData.loading)

  const chartHeight = createMemo(() => props.height || (props.compact ? 150 : 200))

  return (
    <div
      class={`relative ${props.class || ''}`}
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
              <span class="text-sm text-gray-400">로딩 중...</span>
            </div>
          </div>
        }
      >
        <EChart
          option={chartOption()}
          height={chartHeight()}
          theme="dark"
        />

        {/* 하단 레벨 인디케이터 */}
        <Show when={!props.compact}>
          <div class="flex justify-center mt-2">
            <div
              class="flex items-center gap-2 px-3 py-1.5 rounded-full text-sm"
              style={{
                'background-color': `${levelInfo().color}20`,
                color: levelInfo().color,
              }}
            >
              <span class="text-lg">{levelInfo().icon}</span>
              <span class="font-medium">{levelInfo().description}</span>
            </div>
          </div>
        </Show>
      </Show>

      {/* 마지막 업데이트 시간 */}
      <Show when={breadthData() && !props.compact}>
        <div class="text-xs text-gray-500 text-center mt-2">
          {new Date(breadthData()!.calculatedAt).toLocaleString('ko-KR')}
        </div>
      </Show>
    </div>
  )
}

export default FearGreedGauge
