/**
 * GlobalScore 뱃지 컴포넌트
 *
 * 0-100 점수에 따라 색상이 변하는 뱃지를 표시합니다.
 * 점수대별 추천 등급도 함께 표시 가능합니다.
 */
import { type Component, createMemo, Show } from 'solid-js'
import {
  TrendingUp,
  TrendingDown,
  Minus,
  Star,
  AlertTriangle,
} from 'lucide-solid'

interface GlobalScoreBadgeProps {
  score: number
  showGrade?: boolean
  showIcon?: boolean
  size?: 'sm' | 'md' | 'lg'
  className?: string
}

// 점수대별 설정
const getScoreConfig = (score: number) => {
  if (score >= 90) {
    return {
      bg: 'bg-green-100 dark:bg-green-900/30',
      text: 'text-green-700 dark:text-green-400',
      border: 'border-green-300 dark:border-green-700',
      icon: Star,
      grade: 'EXCELLENT',
      gradeKo: '최상',
    }
  }
  if (score >= 80) {
    return {
      bg: 'bg-emerald-100 dark:bg-emerald-900/30',
      text: 'text-emerald-700 dark:text-emerald-400',
      border: 'border-emerald-300 dark:border-emerald-700',
      icon: TrendingUp,
      grade: 'BUY',
      gradeKo: '매수',
    }
  }
  if (score >= 70) {
    return {
      bg: 'bg-lime-100 dark:bg-lime-900/30',
      text: 'text-lime-700 dark:text-lime-400',
      border: 'border-lime-300 dark:border-lime-700',
      icon: TrendingUp,
      grade: 'WATCH',
      gradeKo: '관심',
    }
  }
  if (score >= 60) {
    return {
      bg: 'bg-yellow-100 dark:bg-yellow-900/30',
      text: 'text-yellow-700 dark:text-yellow-400',
      border: 'border-yellow-300 dark:border-yellow-700',
      icon: Minus,
      grade: 'HOLD',
      gradeKo: '보유',
    }
  }
  if (score >= 50) {
    return {
      bg: 'bg-orange-100 dark:bg-orange-900/30',
      text: 'text-orange-700 dark:text-orange-400',
      border: 'border-orange-300 dark:border-orange-700',
      icon: AlertTriangle,
      grade: 'CAUTION',
      gradeKo: '주의',
    }
  }
  return {
    bg: 'bg-red-100 dark:bg-red-900/30',
    text: 'text-red-700 dark:text-red-400',
    border: 'border-red-300 dark:border-red-700',
    icon: TrendingDown,
    grade: 'AVOID',
    gradeKo: '회피',
  }
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

export const GlobalScoreBadge: Component<GlobalScoreBadgeProps> = (props) => {
  // score가 undefined/null이면 0으로 처리
  const score = () => props.score ?? 0
  const config = createMemo(() => getScoreConfig(score()))
  const size = createMemo(() => sizeStyles[props.size || 'md'])
  const showGrade = () => props.showGrade !== false
  const showIcon = () => props.showIcon !== false

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
      <span>{score().toFixed(0)}</span>
      <Show when={showGrade()}>
        <span class="text-xs opacity-75">({config().grade})</span>
      </Show>
    </span>
  )
}

/**
 * GlobalScore 진행 바
 */
export const GlobalScoreBar: Component<{
  score: number
  showLabel?: boolean
  height?: 'sm' | 'md' | 'lg'
  className?: string
}> = (props) => {
  // score가 undefined/null이면 0으로 처리
  const score = () => props.score ?? 0
  const config = createMemo(() => getScoreConfig(score()))

  const barColor = createMemo(() => {
    const s = score()
    if (s >= 80) return 'bg-green-500'
    if (s >= 70) return 'bg-lime-500'
    if (s >= 60) return 'bg-yellow-500'
    if (s >= 50) return 'bg-orange-500'
    return 'bg-red-500'
  })

  const heightClass = createMemo(() => {
    switch (props.height || 'md') {
      case 'sm': return 'h-1'
      case 'lg': return 'h-3'
      default: return 'h-2'
    }
  })

  return (
    <div class={`flex items-center gap-2 ${props.className || ''}`}>
      <div class={`flex-1 bg-gray-200 dark:bg-gray-700 rounded-full ${heightClass()}`}>
        <div
          class={`${barColor()} ${heightClass()} rounded-full transition-all duration-300`}
          style={{ width: `${Math.min(100, Math.max(0, score()))}%` }}
        />
      </div>
      <Show when={props.showLabel !== false}>
        <span class={`text-sm font-medium ${config().text}`}>
          {score().toFixed(0)}
        </span>
      </Show>
    </div>
  )
}

export default GlobalScoreBadge
