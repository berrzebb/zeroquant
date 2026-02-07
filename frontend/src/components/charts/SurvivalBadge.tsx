/**
 * 생존 뱃지 컴포넌트
 *
 * 연속 상위권 일수에 따라 티어를 표시하는 뱃지입니다.
 * 티어: Bronze (1-6일) → Silver (7-13일) → Gold (14-29일) → Platinum (30일+)
 */
import { type Component, createMemo, Show } from 'solid-js'

// ==================== 타입 정의 ====================

export type SurvivalTier = 'bronze' | 'silver' | 'gold' | 'platinum'

export interface SurvivalBadgeProps {
  /** 연속 상위권 일수 */
  days: number
  /** 강제 티어 지정 (선택사항) */
  tier?: SurvivalTier
  /** 크기 */
  size?: 'sm' | 'md' | 'lg'
  /** 일수 표시 여부 */
  showDays?: boolean
  /** 툴팁 표시 여부 */
  showTooltip?: boolean
  /** 클릭 핸들러 */
  onClick?: () => void
}

// ==================== 상수 ====================

const TIER_CONFIG: Record<SurvivalTier, {
  label: string
  color: string
  bgColor: string
  borderColor: string
  icon: string
  minDays: number
}> = {
  bronze: {
    label: 'Bronze',
    color: '#CD7F32',
    bgColor: 'rgba(205, 127, 50, 0.15)',
    borderColor: 'rgba(205, 127, 50, 0.4)',
    icon: '🥉',
    minDays: 1,
  },
  silver: {
    label: 'Silver',
    color: '#C0C0C0',
    bgColor: 'rgba(192, 192, 192, 0.15)',
    borderColor: 'rgba(192, 192, 192, 0.4)',
    icon: '🥈',
    minDays: 7,
  },
  gold: {
    label: 'Gold',
    color: '#FFD700',
    bgColor: 'rgba(255, 215, 0, 0.15)',
    borderColor: 'rgba(255, 215, 0, 0.4)',
    icon: '🥇',
    minDays: 14,
  },
  platinum: {
    label: 'Platinum',
    color: '#E5E4E2',
    bgColor: 'rgba(229, 228, 226, 0.2)',
    borderColor: 'rgba(229, 228, 226, 0.5)',
    icon: '💎',
    minDays: 30,
  },
}

const SIZE_CONFIG = {
  sm: { badge: 'px-1.5 py-0.5 text-xs', icon: 'text-sm' },
  md: { badge: 'px-2 py-1 text-sm', icon: 'text-base' },
  lg: { badge: 'px-3 py-1.5 text-base', icon: 'text-lg' },
}

// ==================== 유틸리티 ====================

/**
 * 일수에 따른 티어 계산
 */
export function getTierFromDays(days: number): SurvivalTier {
  if (days >= 30) return 'platinum'
  if (days >= 14) return 'gold'
  if (days >= 7) return 'silver'
  return 'bronze'
}

/**
 * 다음 티어까지 남은 일수
 */
export function getDaysToNextTier(days: number): { nextTier: SurvivalTier | null; remaining: number } {
  if (days >= 30) return { nextTier: null, remaining: 0 }
  if (days >= 14) return { nextTier: 'platinum', remaining: 30 - days }
  if (days >= 7) return { nextTier: 'gold', remaining: 14 - days }
  return { nextTier: 'silver', remaining: 7 - days }
}

// ==================== 컴포넌트 ====================

/**
 * 생존 뱃지 컴포넌트
 *
 * @example
 * ```tsx
 * <SurvivalBadge days={15} size="md" showDays />
 * ```
 */
export const SurvivalBadge: Component<SurvivalBadgeProps> = (props) => {
  const size = () => props.size ?? 'md'
  const showDays = () => props.showDays ?? true
  const showTooltip = () => props.showTooltip ?? true

  // 티어 계산
  const tier = createMemo(() => props.tier ?? getTierFromDays(props.days))
  const config = createMemo(() => TIER_CONFIG[tier()])
  const sizeClass = createMemo(() => SIZE_CONFIG[size()])

  // 다음 티어 정보
  const nextTierInfo = createMemo(() => getDaysToNextTier(props.days))

  // 툴팁 텍스트
  const tooltipText = createMemo(() => {
    const info = nextTierInfo()
    if (info.nextTier === null) {
      return `${props.days}일 연속 상위권 유지 - 최고 등급!`
    }
    return `${props.days}일 연속 상위권 유지\n${info.nextTier.toUpperCase()}까지 ${info.remaining}일 남음`
  })

  return (
    <div
      class={`
        inline-flex items-center gap-1 rounded-full font-medium
        transition-all duration-200
        ${sizeClass().badge}
        ${props.onClick ? 'cursor-pointer hover:scale-105' : ''}
      `}
      style={{
        color: config().color,
        'background-color': config().bgColor,
        'border': `1px solid ${config().borderColor}`,
      }}
      onClick={props.onClick}
      title={showTooltip() ? tooltipText() : undefined}
    >
      {/* 티어 아이콘 */}
      <span class={sizeClass().icon}>{config().icon}</span>

      {/* 일수 표시 */}
      <Show when={showDays()}>
        <span>{props.days}일</span>
      </Show>

      {/* 티어 라벨 (큰 사이즈에서만) */}
      <Show when={size() === 'lg'}>
        <span class="opacity-70">{config().label}</span>
      </Show>
    </div>
  )
}

// ==================== 서브 컴포넌트 ====================

export interface SurvivalProgressProps {
  /** 현재 일수 */
  days: number
  /** 너비 */
  width?: number
}

/**
 * 생존 진행률 바
 *
 * 다음 티어까지의 진행률을 시각화합니다.
 */
export const SurvivalProgress: Component<SurvivalProgressProps> = (props) => {
  const width = () => props.width ?? 120

  const tier = createMemo(() => getTierFromDays(props.days))
  const config = createMemo(() => TIER_CONFIG[tier()])
  const nextTierInfo = createMemo(() => getDaysToNextTier(props.days))

  // 현재 티어 내 진행률 계산
  const progress = createMemo(() => {
    const currentTier = tier()
    const minDays = TIER_CONFIG[currentTier].minDays
    const nextInfo = nextTierInfo()

    if (nextInfo.nextTier === null) {
      return 100 // 플래티넘은 100%
    }

    const nextMinDays = TIER_CONFIG[nextInfo.nextTier].minDays
    const range = nextMinDays - minDays
    const current = props.days - minDays

    return Math.min(100, Math.round((current / range) * 100))
  })

  return (
    <div class="flex items-center gap-2">
      {/* 뱃지 */}
      <SurvivalBadge days={props.days} size="sm" showDays={false} />

      {/* 프로그레스 바 */}
      <div
        class="h-2 rounded-full overflow-hidden"
        style={{
          width: `${width()}px`,
          'background-color': 'rgba(128, 128, 128, 0.2)',
        }}
      >
        <div
          class="h-full rounded-full transition-all duration-500"
          style={{
            width: `${progress()}%`,
            'background-color': config().color,
          }}
        />
      </div>

      {/* 일수 */}
      <span class="text-xs text-[var(--color-text-muted)]">{props.days}일</span>
    </div>
  )
}

// ==================== 티어 목록 컴포넌트 ====================

export interface TierLegendProps {
  /** 현재 티어 하이라이트 */
  currentTier?: SurvivalTier
  /** 컴팩트 모드 */
  compact?: boolean
}

/**
 * 티어 범례
 */
export const TierLegend: Component<TierLegendProps> = (props) => {
  const tiers: SurvivalTier[] = ['bronze', 'silver', 'gold', 'platinum']

  return (
    <div class={`flex ${props.compact ? 'gap-2' : 'gap-4'}`}>
      {tiers.map((tier) => {
        const config = TIER_CONFIG[tier]
        const isActive = props.currentTier === tier

        return (
          <div
            class={`flex items-center gap-1 ${isActive ? 'font-medium' : 'opacity-60'}`}
            style={{ color: config.color }}
          >
            <span>{config.icon}</span>
            <Show when={!props.compact}>
              <span class="text-xs">{config.label}</span>
              <span class="text-xs opacity-50">({config.minDays}일+)</span>
            </Show>
          </div>
        )
      })}
    </div>
  )
}

export default SurvivalBadge
