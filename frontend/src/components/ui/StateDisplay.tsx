/**
 * 상태 표시 컴포넌트
 *
 * 빈 상태, 에러 상태, 성공 상태 등을 일관되게 표시합니다.
 */
import { type Component, type JSX, Show } from 'solid-js'

// ==================== Empty State ====================

export interface EmptyStateProps {
  /** 아이콘 (이모지 또는 컴포넌트) */
  icon?: string | JSX.Element
  /** 제목 */
  title: string
  /** 설명 */
  description?: string
  /** 액션 버튼 */
  action?: JSX.Element
  /** 추가 CSS 클래스 */
  className?: string
}

/**
 * 빈 상태 표시
 *
 * 데이터가 없을 때 사용자에게 안내 메시지를 표시합니다.
 *
 * @example
 * ```tsx
 * <EmptyState
 *   icon="📭"
 *   title="데이터가 없습니다"
 *   description="조건에 맞는 항목이 없습니다."
 *   action={<Button onClick={reset}>필터 초기화</Button>}
 * />
 * ```
 */
export const EmptyState: Component<EmptyStateProps> = (props) => {
  return (
    <div class={`text-center py-12 ${props.className || ''}`}>
      <Show when={props.icon}>
        <div class="text-5xl mb-4 opacity-50">
          {typeof props.icon === 'string' ? props.icon : props.icon}
        </div>
      </Show>
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-2">{props.title}</h3>
      <Show when={props.description}>
        <p class="text-sm text-gray-500 dark:text-gray-400 mb-4 max-w-md mx-auto">
          {props.description}
        </p>
      </Show>
      <Show when={props.action}>
        <div class="mt-4">{props.action}</div>
      </Show>
    </div>
  )
}

// ==================== Error State ====================

export interface ErrorStateProps {
  /** 에러 제목 */
  title?: string
  /** 에러 메시지 */
  message: string
  /** 재시도 핸들러 */
  onRetry?: () => void
  /** 재시도 버튼 텍스트 */
  retryText?: string
  /** 추가 CSS 클래스 */
  className?: string
}

/**
 * 에러 상태 표시
 *
 * 데이터 로드 실패 등의 에러 상황을 표시합니다.
 *
 * @example
 * ```tsx
 * <ErrorState
 *   title="데이터 로드 실패"
 *   message={error.message}
 *   onRetry={refetch}
 * />
 * ```
 */
export const ErrorState: Component<ErrorStateProps> = (props) => {
  return (
    <div class={`text-center py-12 ${props.className || ''}`}>
      <div class="text-5xl mb-4">❌</div>
      <h3 class="text-lg font-medium text-red-600 dark:text-red-400 mb-2">
        {props.title || '오류 발생'}
      </h3>
      <p class="text-sm text-gray-500 dark:text-gray-400 mb-4 max-w-md mx-auto">{props.message}</p>
      <Show when={props.onRetry}>
        <button
          onClick={props.onRetry}
          class="px-4 py-2 text-sm bg-gray-100 dark:bg-gray-700 hover:bg-gray-200
                 dark:hover:bg-gray-600 rounded-lg transition-colors text-gray-700 dark:text-gray-300"
        >
          {props.retryText || '다시 시도'}
        </button>
      </Show>
    </div>
  )
}

// ==================== Success State ====================

export interface SuccessStateProps {
  /** 제목 */
  title: string
  /** 설명 */
  description?: string
  /** 액션 버튼 */
  action?: JSX.Element
  /** 추가 CSS 클래스 */
  className?: string
}

/**
 * 성공 상태 표시
 *
 * 작업 완료 등의 성공 상황을 표시합니다.
 */
export const SuccessState: Component<SuccessStateProps> = (props) => {
  return (
    <div class={`text-center py-12 ${props.className || ''}`}>
      <div class="text-5xl mb-4">✅</div>
      <h3 class="text-lg font-medium text-green-600 dark:text-green-400 mb-2">{props.title}</h3>
      <Show when={props.description}>
        <p class="text-sm text-gray-500 dark:text-gray-400 mb-4 max-w-md mx-auto">
          {props.description}
        </p>
      </Show>
      <Show when={props.action}>
        <div class="mt-4">{props.action}</div>
      </Show>
    </div>
  )
}

// ==================== Warning State ====================

export interface WarningStateProps {
  /** 제목 */
  title: string
  /** 설명 */
  description?: string
  /** 액션 버튼 */
  action?: JSX.Element
  /** 추가 CSS 클래스 */
  className?: string
}

/**
 * 경고 상태 표시
 *
 * 주의가 필요한 상황을 표시합니다.
 */
export const WarningState: Component<WarningStateProps> = (props) => {
  return (
    <div class={`text-center py-12 ${props.className || ''}`}>
      <div class="text-5xl mb-4">⚠️</div>
      <h3 class="text-lg font-medium text-yellow-600 dark:text-yellow-400 mb-2">{props.title}</h3>
      <Show when={props.description}>
        <p class="text-sm text-gray-500 dark:text-gray-400 mb-4 max-w-md mx-auto">
          {props.description}
        </p>
      </Show>
      <Show when={props.action}>
        <div class="mt-4">{props.action}</div>
      </Show>
    </div>
  )
}

export default EmptyState
