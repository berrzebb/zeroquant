/**
 * 순위 변동 표시 컴포넌트
 *
 * 순위 변동을 화살표와 변동폭으로 시각화합니다.
 * - 상승: 초록색 ↑ + 변동폭
 * - 하락: 빨간색 ↓ + 변동폭
 * - 변동없음: 회색 -
 * - 신규: 파란색 NEW
 */
import { type Component, Show } from 'solid-js'

export interface RankChangeIndicatorProps {
  /** 순위 변동 (양수: 상승, 음수: 하락, 0: 변동없음, undefined/null: 신규) */
  change?: number | null
  /** 크기 ('xs' | 'sm' | 'md') */
  size?: 'xs' | 'sm' | 'md'
  /** 숫자만 표시 (화살표 없음) */
  compactMode?: boolean
  /** 배경색 표시 여부 */
  showBackground?: boolean
}

const SIZE_CLASSES = {
  xs: 'text-[10px] px-1 py-0.5',
  sm: 'text-xs px-1.5 py-0.5',
  md: 'text-sm px-2 py-1',
}

const ARROW_SIZES = {
  xs: 'w-2.5 h-2.5',
  sm: 'w-3 h-3',
  md: 'w-4 h-4',
}

/**
 * 순위 변동 표시 컴포넌트
 */
export const RankChangeIndicator: Component<RankChangeIndicatorProps> = (props) => {
  const size = () => props.size || 'sm'
  const showBg = () => props.showBackground ?? false

  // 신규 종목
  if (props.change === undefined || props.change === null) {
    return (
      <span
        class={`
          inline-flex items-center gap-0.5 font-medium
          text-blue-600 dark:text-blue-400
          ${SIZE_CLASSES[size()]}
          ${showBg() ? 'bg-blue-100 dark:bg-blue-900/30 rounded' : ''}
        `}
      >
        NEW
      </span>
    )
  }

  // 변동 없음
  if (props.change === 0) {
    return (
      <span
        class={`
          inline-flex items-center justify-center font-medium
          text-gray-400 dark:text-gray-500
          ${SIZE_CLASSES[size()]}
          ${showBg() ? 'bg-gray-100 dark:bg-gray-800 rounded' : ''}
        `}
      >
        -
      </span>
    )
  }

  // 상승
  if (props.change > 0) {
    return (
      <span
        class={`
          inline-flex items-center gap-0.5 font-medium
          text-green-600 dark:text-green-400
          ${SIZE_CLASSES[size()]}
          ${showBg() ? 'bg-green-100 dark:bg-green-900/30 rounded' : ''}
        `}
      >
        <Show when={!props.compactMode}>
          <svg
            class={ARROW_SIZES[size()]}
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            stroke-width="3"
          >
            <path stroke-linecap="round" stroke-linejoin="round" d="M5 15l7-7 7 7" />
          </svg>
        </Show>
        <span>{props.change}</span>
      </span>
    )
  }

  // 하락
  return (
    <span
      class={`
        inline-flex items-center gap-0.5 font-medium
        text-red-600 dark:text-red-400
        ${SIZE_CLASSES[size()]}
        ${showBg() ? 'bg-red-100 dark:bg-red-900/30 rounded' : ''}
      `}
    >
      <Show when={!props.compactMode}>
        <svg
          class={ARROW_SIZES[size()]}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          stroke-width="3"
        >
          <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
        </svg>
      </Show>
      <span>{Math.abs(props.change)}</span>
    </span>
  )
}

export default RankChangeIndicator
