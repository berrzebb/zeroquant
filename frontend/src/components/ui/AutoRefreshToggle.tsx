/**
 * 자동 갱신 토글 컴포넌트
 *
 * 데이터의 자동 갱신 주기를 설정할 수 있는 드롭다운 버튼입니다.
 * 30초, 1분, 5분 중 선택하거나 끌 수 있습니다.
 */
import { type Component, createSignal, createEffect, onCleanup, Show } from 'solid-js'

export type RefreshInterval = 0 | 30 | 60 | 300

export interface AutoRefreshToggleProps {
  /** 갱신 콜백 함수 */
  onRefresh: () => void
  /** 초기 갱신 주기 (초, 0=끔) */
  initialInterval?: RefreshInterval
  /** 크기 */
  size?: 'sm' | 'md'
  /** 갱신 중 여부 */
  isRefreshing?: boolean
  /** 비활성화 */
  disabled?: boolean
}

const INTERVAL_OPTIONS: { value: RefreshInterval; label: string }[] = [
  { value: 0, label: '끔' },
  { value: 30, label: '30초' },
  { value: 60, label: '1분' },
  { value: 300, label: '5분' },
]

/**
 * 자동 갱신 토글 컴포넌트
 */
export const AutoRefreshToggle: Component<AutoRefreshToggleProps> = (props) => {
  const [interval, setInterval_] = createSignal<RefreshInterval>(props.initialInterval || 0)
  const [isOpen, setIsOpen] = createSignal(false)
  const [countdown, setCountdown] = createSignal(0)

  let timerRef: ReturnType<typeof setInterval> | null = null
  let countdownRef: ReturnType<typeof setInterval> | null = null

  // 타이머 정리 함수
  const clearTimers = () => {
    if (timerRef) {
      clearInterval(timerRef)
      timerRef = null
    }
    if (countdownRef) {
      clearInterval(countdownRef)
      countdownRef = null
    }
  }

  // 갱신 주기 변경 시 타이머 재설정
  createEffect(() => {
    const sec = interval()
    clearTimers()

    if (sec > 0) {
      setCountdown(sec)

      // 카운트다운 타이머
      countdownRef = setInterval(() => {
        setCountdown(prev => {
          if (prev <= 1) {
            return sec
          }
          return prev - 1
        })
      }, 1000)

      // 갱신 타이머
      timerRef = setInterval(() => {
        props.onRefresh()
      }, sec * 1000)
    } else {
      setCountdown(0)
    }
  })

  // 컴포넌트 언마운트 시 타이머 정리
  onCleanup(clearTimers)

  const handleSelect = (value: RefreshInterval) => {
    setInterval_(value)
    setIsOpen(false)
  }

  const size = () => props.size || 'md'

  const buttonClasses = () => {
    const base = `
      inline-flex items-center gap-1.5 font-medium rounded-lg
      bg-gray-100 dark:bg-gray-700
      text-gray-700 dark:text-gray-200
      hover:bg-gray-200 dark:hover:bg-gray-600
      transition-colors duration-150
      focus:outline-none focus:ring-2 focus:ring-gray-400 focus:ring-offset-1
      disabled:opacity-50 disabled:cursor-not-allowed
    `
    const sizeClass = size() === 'sm' ? 'px-2 py-1.5 text-xs' : 'px-3 py-2 text-sm'
    return `${base} ${sizeClass}`
  }

  const currentLabel = () => {
    const opt = INTERVAL_OPTIONS.find(o => o.value === interval())
    return opt?.label || '끔'
  }

  return (
    <div class="relative">
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen())}
        disabled={props.disabled}
        class={buttonClasses()}
        title="자동 갱신 설정"
      >
        {/* 회전 아이콘 (갱신 중일 때 회전) */}
        <svg
          class={`w-4 h-4 ${props.isRefreshing ? 'animate-spin' : ''}`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          stroke-width="2"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
          />
        </svg>

        {/* 현재 설정 표시 */}
        <span>{currentLabel()}</span>

        {/* 카운트다운 표시 */}
        <Show when={interval() > 0 && countdown() > 0}>
          <span class="text-xs text-gray-500 dark:text-gray-400 tabular-nums">
            ({countdown()}s)
          </span>
        </Show>

        {/* 드롭다운 화살표 */}
        <svg class="w-3.5 h-3.5 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {/* 드롭다운 메뉴 */}
      <Show when={isOpen()}>
        <div
          class="absolute right-0 mt-1 w-28 bg-white dark:bg-gray-800 rounded-lg shadow-lg border border-gray-200 dark:border-gray-700 z-50"
        >
          {INTERVAL_OPTIONS.map(option => (
            <button
              type="button"
              onClick={() => handleSelect(option.value)}
              class={`
                w-full px-3 py-2 text-left text-sm
                hover:bg-gray-100 dark:hover:bg-gray-700
                first:rounded-t-lg last:rounded-b-lg
                ${interval() === option.value
                  ? 'text-blue-600 dark:text-blue-400 font-medium'
                  : 'text-gray-700 dark:text-gray-300'
                }
              `}
            >
              {option.label}
              <Show when={interval() === option.value}>
                <span class="float-right text-blue-500">✓</span>
              </Show>
            </button>
          ))}
        </div>
      </Show>

      {/* 바깥 클릭 시 닫기 */}
      <Show when={isOpen()}>
        <div
          class="fixed inset-0 z-40"
          onClick={() => setIsOpen(false)}
        />
      </Show>
    </div>
  )
}

export default AutoRefreshToggle
