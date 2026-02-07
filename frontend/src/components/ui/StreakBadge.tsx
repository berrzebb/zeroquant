/**
 * Streak Badge 컴포넌트
 *
 * 연속 승/패 스트릭을 시각적으로 표시하는 배지 컴포넌트입니다.
 * 트레이딩 성과의 연속성을 보여주어 모멘텀을 파악할 수 있습니다.
 *
 * Note: 이 컴포넌트는 이전에 SurvivalBadge라는 이름이었으나,
 * charts/SurvivalBadge (랭킹 생존 티어)와 이름 충돌을 피하기 위해
 * StreakBadge로 변경되었습니다.
 *
 * @example
 * ```tsx
 * <StreakBadge
 *   currentStreak={5}
 *   maxStreak={12}
 *   type="wins"
 * />
 * ```
 */
import { createMemo, Show } from 'solid-js'
import type { Component, JSX } from 'solid-js'

/** 스트릭 타입 */
export type StreakType = 'wins' | 'losses' | 'auto'

/** 스트릭 레벨 (강도) */
type StreakLevel = 'cold' | 'warm' | 'hot' | 'fire'

/** 레벨별 스타일 정보 */
interface LevelStyle {
  bgColor: string
  textColor: string
  borderColor: string
  icon: string
  label: string
}

const WIN_LEVELS: Record<StreakLevel, LevelStyle> = {
  cold: {
    bgColor: 'bg-green-900/20',
    textColor: 'text-green-400',
    borderColor: 'border-green-700/50',
    icon: '📈',
    label: '연승',
  },
  warm: {
    bgColor: 'bg-green-900/30',
    textColor: 'text-green-300',
    borderColor: 'border-green-600/60',
    icon: '🔥',
    label: '연승',
  },
  hot: {
    bgColor: 'bg-green-900/40',
    textColor: 'text-green-200',
    borderColor: 'border-green-500/70',
    icon: '🔥🔥',
    label: '대연승',
  },
  fire: {
    bgColor: 'bg-gradient-to-r from-green-900/50 to-yellow-900/30',
    textColor: 'text-green-100',
    borderColor: 'border-green-400',
    icon: '🔥🔥🔥',
    label: '불타는 연승',
  },
}

const LOSS_LEVELS: Record<StreakLevel, LevelStyle> = {
  cold: {
    bgColor: 'bg-red-900/20',
    textColor: 'text-red-400',
    borderColor: 'border-red-700/50',
    icon: '📉',
    label: '연패',
  },
  warm: {
    bgColor: 'bg-red-900/30',
    textColor: 'text-red-300',
    borderColor: 'border-red-600/60',
    icon: '💔',
    label: '연패',
  },
  hot: {
    bgColor: 'bg-red-900/40',
    textColor: 'text-red-200',
    borderColor: 'border-red-500/70',
    icon: '😰',
    label: '대연패',
  },
  fire: {
    bgColor: 'bg-gradient-to-r from-red-900/50 to-orange-900/30',
    textColor: 'text-red-100',
    borderColor: 'border-red-400',
    icon: '🆘',
    label: '위기',
  },
}

/** 스트릭 수에 따른 레벨 결정 */
const getStreakLevel = (count: number): StreakLevel => {
  if (count >= 10) return 'fire'
  if (count >= 5) return 'hot'
  if (count >= 3) return 'warm'
  return 'cold'
}

export interface StreakBadgeProps {
  /** 현재 스트릭 수 */
  currentStreak: number
  /** 최대 스트릭 수 (기록) */
  maxStreak?: number
  /** 스트릭 타입 (wins/losses/auto) */
  type?: StreakType
  /** 컴팩트 모드 */
  compact?: boolean
  /** 최대 기록 표시 여부 */
  showMax?: boolean
  /** 애니메이션 효과 */
  animated?: boolean
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

/**
 * Streak Badge 컴포넌트
 *
 * 연속 승/패 스트릭을 시각적으로 표시합니다.
 * 3연속 이상부터 점점 강조되며, 10연속 이상은 "fire" 상태로 표시됩니다.
 */
export const StreakBadge: Component<StreakBadgeProps> = (props) => {
  // 스트릭 타입 결정
  const streakType = createMemo((): 'wins' | 'losses' => {
    if (props.type === 'auto') {
      // 자동 판단: 현재 스트릭 > 0이면 wins, 아니면 losses
      return props.currentStreak > 0 ? 'wins' : 'losses'
    }
    return props.type || 'wins'
  })

  // 절대값 스트릭 수
  const streakCount = createMemo(() => Math.abs(props.currentStreak))
  const maxStreakCount = createMemo(() => (props.maxStreak ? Math.abs(props.maxStreak) : 0))

  // 레벨 및 스타일
  const level = createMemo(() => getStreakLevel(streakCount()))
  const styles = createMemo(() => {
    const levelStyles = streakType() === 'wins' ? WIN_LEVELS : LOSS_LEVELS
    return levelStyles[level()]
  })

  // 스트릭이 0이면 표시하지 않음
  const shouldShow = createMemo(() => streakCount() > 0)

  // 최대 기록 비율 (퍼센트)
  const recordRatio = createMemo(() => {
    if (!maxStreakCount()) return 0
    return Math.min(100, (streakCount() / maxStreakCount()) * 100)
  })

  // 신기록 여부
  const isNewRecord = createMemo(() => {
    return maxStreakCount() > 0 && streakCount() >= maxStreakCount()
  })

  return (
    <Show when={shouldShow()}>
      <div
        class={`
          inline-flex items-center gap-2 px-3 py-2 rounded-lg border
          ${styles().bgColor} ${styles().borderColor}
          ${props.animated && level() !== 'cold' ? 'animate-pulse' : ''}
          ${props.class || ''}
        `}
        style={props.style}
      >
        {/* 아이콘 */}
        <span class={`text-base ${props.compact ? 'text-sm' : ''}`}>
          {styles().icon}
        </span>

        {/* 메인 내용 */}
        <div class="flex flex-col">
          {/* 현재 스트릭 */}
          <div class="flex items-center gap-1.5">
            <span class={`font-bold ${styles().textColor} ${props.compact ? 'text-sm' : 'text-lg'}`}>
              {streakCount()}
            </span>
            <span class={`text-xs ${styles().textColor} opacity-80`}>
              {styles().label}
            </span>
            <Show when={isNewRecord()}>
              <span class="text-xs bg-yellow-500 text-black px-1 py-0.5 rounded font-bold ml-1">
                NEW
              </span>
            </Show>
          </div>

          {/* 최대 기록 */}
          <Show when={props.showMax && maxStreakCount() > 0 && !props.compact}>
            <div class="flex items-center gap-1 mt-0.5">
              <span class="text-xs text-gray-400">최대:</span>
              <span class="text-xs text-gray-300">{maxStreakCount()}</span>
              <div class="flex-1 h-1 bg-gray-700 rounded-full ml-1 overflow-hidden">
                <div
                  class={`h-full transition-all duration-500 ${
                    streakType() === 'wins' ? 'bg-green-500' : 'bg-red-500'
                  }`}
                  style={{ width: `${recordRatio()}%` }}
                />
              </div>
            </div>
          </Show>
        </div>
      </div>
    </Show>
  )
}

/**
 * 듀얼 스트릭 배지 컴포넌트
 *
 * 승/패 스트릭을 동시에 표시합니다.
 */
export interface DualStreakBadgeProps {
  /** 현재 연승 수 */
  currentWins: number
  /** 최대 연승 기록 */
  maxWins?: number
  /** 현재 연패 수 */
  currentLosses: number
  /** 최대 연패 기록 */
  maxLosses?: number
  /** 컴팩트 모드 */
  compact?: boolean
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

export const DualStreakBadge: Component<DualStreakBadgeProps> = (props) => {
  return (
    <div
      class={`flex items-center gap-3 ${props.class || ''}`}
      style={props.style}
    >
      <Show when={props.currentWins > 0}>
        <StreakBadge
          currentStreak={props.currentWins}
          maxStreak={props.maxWins}
          type="wins"
          compact={props.compact}
          showMax
        />
      </Show>
      <Show when={props.currentLosses > 0}>
        <StreakBadge
          currentStreak={props.currentLosses}
          maxStreak={props.maxLosses}
          type="losses"
          compact={props.compact}
          showMax
        />
      </Show>
      <Show when={props.currentWins === 0 && props.currentLosses === 0}>
        <div class="inline-flex items-center gap-2 px-3 py-2 rounded-lg border border-gray-700 bg-gray-800/50">
          <span class="text-base">⚖️</span>
          <span class="text-sm text-gray-400">스트릭 없음</span>
        </div>
      </Show>
    </div>
  )
}

/**
 * 연속 기록 요약 카드
 *
 * 승/패 기록을 카드 형태로 요약하여 표시합니다.
 */
export interface StreakSummaryCardProps {
  /** 최대 연승 */
  maxWins: number
  /** 최대 연패 */
  maxLosses: number
  /** 현재 연승 (옵션) */
  currentWins?: number
  /** 현재 연패 (옵션) */
  currentLosses?: number
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

export const StreakSummaryCard: Component<StreakSummaryCardProps> = (props) => {
  return (
    <div
      class={`rounded-xl border border-gray-700 bg-gray-800/50 p-4 ${props.class || ''}`}
      style={props.style}
    >
      <h4 class="text-sm font-medium text-gray-300 mb-3">연속 기록</h4>
      <div class="grid grid-cols-2 gap-4">
        {/* 최대 연승 */}
        <div class="text-center">
          <div class="text-2xl font-bold text-green-400">{props.maxWins}</div>
          <div class="text-xs text-gray-400 mt-1">최대 연승</div>
          <Show when={props.currentWins !== undefined && props.currentWins > 0}>
            <div class="text-xs text-green-500 mt-0.5">
              현재 {props.currentWins}연승 중
            </div>
          </Show>
        </div>
        {/* 최대 연패 */}
        <div class="text-center">
          <div class="text-2xl font-bold text-red-400">{props.maxLosses}</div>
          <div class="text-xs text-gray-400 mt-1">최대 연패</div>
          <Show when={props.currentLosses !== undefined && props.currentLosses > 0}>
            <div class="text-xs text-red-500 mt-0.5">
              현재 {props.currentLosses}연패 중
            </div>
          </Show>
        </div>
      </div>
    </div>
  )
}

// 하위 호환성을 위한 별칭 (deprecated)
/** @deprecated StreakBadge로 이름 변경됨 */
export const SurvivalBadge = StreakBadge
/** @deprecated StreakBadgeProps로 이름 변경됨 */
export type SurvivalBadgeProps = StreakBadgeProps
/** @deprecated DualStreakBadge로 이름 변경됨 */
export const DualSurvivalBadge = DualStreakBadge
/** @deprecated DualStreakBadgeProps로 이름 변경됨 */
export type DualSurvivalBadgeProps = DualStreakBadgeProps

export default StreakBadge
