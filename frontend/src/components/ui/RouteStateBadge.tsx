/**
 * RouteState 뱃지 컴포넌트
 *
 * RouteState 값에 따라 색상과 아이콘이 다른 뱃지를 표시합니다.
 * - ATTACK: 빨간색, 공격적 진입 신호
 * - ARMED: 주황색, 준비 상태
 * - NEUTRAL: 회색, 중립
 * - WAIT: 파란색, 대기
 * - OVERHEAT: 보라색, 과열
 */
import { type Component, Show, createMemo } from 'solid-js'
import {
  Zap,
  Target,
  Circle,
  Clock,
  Flame,
  HelpCircle,
} from 'lucide-solid'

export type RouteStateType = 'ATTACK' | 'ARMED' | 'NEUTRAL' | 'WAIT' | 'OVERHEAT' | string

interface RouteStateBadgeProps {
  state: RouteStateType
  size?: 'sm' | 'md' | 'lg'
  showLabel?: boolean
  showIcon?: boolean
  className?: string
}

// RouteState별 스타일 설정
const stateConfig: Record<string, {
  bg: string
  text: string
  border: string
  icon: typeof Zap
  label: string
  labelKo: string
}> = {
  ATTACK: {
    bg: 'bg-red-100 dark:bg-red-900/30',
    text: 'text-red-700 dark:text-red-400',
    border: 'border-red-300 dark:border-red-700',
    icon: Zap,
    label: 'ATTACK',
    labelKo: '공격',
  },
  ARMED: {
    bg: 'bg-orange-100 dark:bg-orange-900/30',
    text: 'text-orange-700 dark:text-orange-400',
    border: 'border-orange-300 dark:border-orange-700',
    icon: Target,
    label: 'ARMED',
    labelKo: '준비',
  },
  NEUTRAL: {
    bg: 'bg-gray-100 dark:bg-gray-800',
    text: 'text-gray-600 dark:text-gray-400',
    border: 'border-gray-300 dark:border-gray-600',
    icon: Circle,
    label: 'NEUTRAL',
    labelKo: '중립',
  },
  WAIT: {
    bg: 'bg-blue-100 dark:bg-blue-900/30',
    text: 'text-blue-700 dark:text-blue-400',
    border: 'border-blue-300 dark:border-blue-700',
    icon: Clock,
    label: 'WAIT',
    labelKo: '대기',
  },
  OVERHEAT: {
    bg: 'bg-purple-100 dark:bg-purple-900/30',
    text: 'text-purple-700 dark:text-purple-400',
    border: 'border-purple-300 dark:border-purple-700',
    icon: Flame,
    label: 'OVERHEAT',
    labelKo: '과열',
  },
}

// 크기별 스타일
const sizeStyles = {
  sm: {
    badge: 'px-1.5 py-0.5 text-xs',
    icon: 'w-3 h-3',
    gap: 'gap-0.5',
  },
  md: {
    badge: 'px-2 py-1 text-sm',
    icon: 'w-4 h-4',
    gap: 'gap-1',
  },
  lg: {
    badge: 'px-3 py-1.5 text-base',
    icon: 'w-5 h-5',
    gap: 'gap-1.5',
  },
}

export const RouteStateBadge: Component<RouteStateBadgeProps> = (props) => {
  const config = createMemo(() => {
    const upperState = (props.state || '').toUpperCase()
    return stateConfig[upperState] || {
      bg: 'bg-gray-100 dark:bg-gray-800',
      text: 'text-gray-500',
      border: 'border-gray-300',
      icon: HelpCircle,
      label: props.state || 'UNKNOWN',
      labelKo: '알 수 없음',
    }
  })

  const size = createMemo(() => sizeStyles[props.size || 'md'])
  const showIcon = () => props.showIcon !== false
  const showLabel = () => props.showLabel !== false

  const Icon = () => {
    const IconComponent = config().icon
    return <IconComponent class={size().icon} />
  }

  return (
    <span
      class={`
        inline-flex items-center ${size().gap} ${size().badge}
        ${config().bg} ${config().text} ${config().border}
        border rounded-full font-medium
        ${props.className || ''}
      `}
    >
      <Show when={showIcon()}>
        <Icon />
      </Show>
      <Show when={showLabel()}>
        <span>{config().label}</span>
      </Show>
    </span>
  )
}

/**
 * RouteState 색상 점 (테이블에서 간단히 표시할 때)
 */
export const RouteStateDot: Component<{ state: RouteStateType; className?: string }> = (props) => {
  const dotColor = createMemo(() => {
    const upperState = (props.state || '').toUpperCase()
    switch (upperState) {
      case 'ATTACK': return 'bg-red-500'
      case 'ARMED': return 'bg-orange-500'
      case 'NEUTRAL': return 'bg-gray-400'
      case 'WAIT': return 'bg-blue-500'
      case 'OVERHEAT': return 'bg-purple-500'
      default: return 'bg-gray-300'
    }
  })

  return (
    <span
      class={`inline-block w-2 h-2 rounded-full ${dotColor()} ${props.className || ''}`}
      title={props.state}
    />
  )
}

export default RouteStateBadge
