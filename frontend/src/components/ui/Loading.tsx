/**
 * 로딩 관련 컴포넌트
 *
 * Spinner, Skeleton, LoadingOverlay 등 로딩 상태 표시 컴포넌트입니다.
 */
import { type Component, type JSX, Show, createMemo } from 'solid-js'
import { Loader2 } from 'lucide-solid'

interface SpinnerProps {
  size?: 'sm' | 'md' | 'lg' | 'xl'
  className?: string
  color?: 'primary' | 'white' | 'gray'
}

const spinnerSizes = {
  sm: 'w-4 h-4',
  md: 'w-6 h-6',
  lg: 'w-8 h-8',
  xl: 'w-12 h-12',
}

const spinnerColors = {
  primary: 'text-blue-500',
  white: 'text-white',
  gray: 'text-gray-400',
}

export const Spinner: Component<SpinnerProps> = (props) => {
  const size = createMemo(() => spinnerSizes[props.size || 'md'])
  const color = createMemo(() => spinnerColors[props.color || 'primary'])

  return (
    <Loader2 class={`animate-spin ${size()} ${color()} ${props.className || ''}`} />
  )
}

interface SkeletonProps {
  className?: string
  variant?: 'text' | 'circular' | 'rectangular'
  width?: string
  height?: string
  animation?: 'pulse' | 'wave' | 'none'
}

export const Skeleton: Component<SkeletonProps> = (props) => {
  const variant = createMemo(() => {
    switch (props.variant) {
      case 'circular': return 'rounded-full'
      case 'rectangular': return 'rounded-md'
      default: return 'rounded'
    }
  })

  const animation = createMemo(() => {
    switch (props.animation) {
      case 'wave': return 'animate-shimmer bg-gradient-to-r from-gray-200 via-gray-300 to-gray-200 bg-[length:200%_100%]'
      case 'none': return ''
      default: return 'animate-pulse'
    }
  })

  return (
    <div
      class={`
        bg-gray-200 dark:bg-gray-700 ${variant()} ${animation()}
        ${props.className || ''}
      `}
      style={{
        width: props.width || '100%',
        height: props.height || '1rem',
      }}
    />
  )
}

interface LoadingOverlayProps {
  isLoading: boolean
  children: JSX.Element
  message?: string
  blur?: boolean
}

export const LoadingOverlay: Component<LoadingOverlayProps> = (props) => {
  return (
    <div class="relative">
      {props.children}
      <Show when={props.isLoading}>
        <div
          class={`
            absolute inset-0 flex flex-col items-center justify-center
            bg-white/80 dark:bg-gray-900/80
            ${props.blur ? 'backdrop-blur-sm' : ''}
            z-50 rounded-lg
          `}
        >
          <Spinner size="lg" />
          <Show when={props.message}>
            <p class="mt-2 text-sm text-gray-600 dark:text-gray-400">{props.message}</p>
          </Show>
        </div>
      </Show>
    </div>
  )
}

/**
 * 전체 페이지 로딩 스피너
 */
export const PageLoader: Component<{ message?: string }> = (props) => {
  return (
    <div class="flex flex-col items-center justify-center min-h-[400px]">
      <Spinner size="xl" />
      <Show when={props.message}>
        <p class="mt-4 text-gray-600 dark:text-gray-400">{props.message}</p>
      </Show>
    </div>
  )
}

/**
 * 인라인 로딩 상태
 */
export const InlineLoader: Component<{ text?: string }> = (props) => {
  return (
    <span class="inline-flex items-center gap-2 text-gray-500">
      <Spinner size="sm" color="gray" />
      <span>{props.text || '로딩 중...'}</span>
    </span>
  )
}

export default Spinner
