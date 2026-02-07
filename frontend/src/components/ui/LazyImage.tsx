/**
 * LazyImage - 이미지 지연 로딩 컴포넌트
 *
 * Intersection Observer를 사용하여 뷰포트에 진입할 때만 이미지를 로드합니다.
 * 초기 페이지 로드 시간을 단축하고 대역폭을 절약합니다.
 */
import { createSignal, createEffect, onCleanup, Show, type JSX } from 'solid-js'

// ==================== 타입 정의 ====================

export interface LazyImageProps {
  /** 이미지 소스 URL */
  src: string
  /** 대체 텍스트 */
  alt: string
  /** 이미지 너비 */
  width?: number | string
  /** 이미지 높이 */
  height?: number | string
  /** 추가 클래스 */
  class?: string
  /** 플레이스홀더 이미지 또는 색상 */
  placeholder?: string
  /** 로드 실패 시 대체 이미지 */
  fallback?: string
  /** 로드 완료 콜백 */
  onLoad?: () => void
  /** 로드 실패 콜백 */
  onError?: () => void
  /** 뷰포트 진입 전 미리 로드할 거리 (px) */
  rootMargin?: string
  /** 스타일 */
  style?: JSX.CSSProperties
}

// ==================== 컴포넌트 ====================

export function LazyImage(props: LazyImageProps) {
  let imgRef: HTMLDivElement | undefined
  const [isLoaded, setIsLoaded] = createSignal(false)
  const [isError, setIsError] = createSignal(false)
  const [isInView, setIsInView] = createSignal(false)

  createEffect(() => {
    if (!imgRef) return

    const observer = new IntersectionObserver(
      (entries) => {
        const [entry] = entries
        if (entry.isIntersecting) {
          setIsInView(true)
          observer.disconnect()
        }
      },
      {
        rootMargin: props.rootMargin || '50px',
        threshold: 0,
      }
    )

    observer.observe(imgRef)

    onCleanup(() => observer.disconnect())
  })

  const handleLoad = () => {
    setIsLoaded(true)
    props.onLoad?.()
  }

  const handleError = () => {
    setIsError(true)
    props.onError?.()
  }

  const containerStyle = (): JSX.CSSProperties => ({
    width: typeof props.width === 'number' ? `${props.width}px` : props.width,
    height: typeof props.height === 'number' ? `${props.height}px` : props.height,
    'background-color': props.placeholder || 'var(--color-surface-light)',
    ...props.style,
  })

  return (
    <div
      ref={imgRef}
      class={`lazy-image-container relative overflow-hidden ${props.class || ''}`}
      style={containerStyle()}
    >
      {/* 플레이스홀더 / 로딩 상태 */}
      <Show when={!isLoaded() && !isError()}>
        <div class="absolute inset-0 flex items-center justify-center bg-[var(--color-surface-light)]">
          <Show when={isInView()}>
            <div class="w-6 h-6 border-2 border-[var(--color-primary)] border-t-transparent rounded-full animate-spin" />
          </Show>
        </div>
      </Show>

      {/* 실제 이미지 (뷰포트 진입 시 로드) */}
      <Show when={isInView()}>
        <Show
          when={!isError()}
          fallback={
            <Show
              when={props.fallback}
              fallback={
                <div class="absolute inset-0 flex items-center justify-center bg-[var(--color-surface-light)] text-[var(--color-text-muted)]">
                  <span class="text-2xl">🖼️</span>
                </div>
              }
            >
              <img
                src={props.fallback}
                alt={props.alt}
                class={`w-full h-full object-cover ${isLoaded() ? 'opacity-100' : 'opacity-0'} transition-opacity duration-300`}
                onLoad={handleLoad}
              />
            </Show>
          }
        >
          <img
            src={props.src}
            alt={props.alt}
            class={`w-full h-full object-cover ${isLoaded() ? 'opacity-100' : 'opacity-0'} transition-opacity duration-300`}
            loading="lazy"
            onLoad={handleLoad}
            onError={handleError}
          />
        </Show>
      </Show>
    </div>
  )
}

/**
 * 네이티브 lazy loading이 적용된 간단한 이미지
 * IntersectionObserver 없이 브라우저 기본 기능만 사용
 */
export function NativeLazyImage(
  props: Omit<LazyImageProps, 'rootMargin' | 'placeholder'>
) {
  const [isError, setIsError] = createSignal(false)

  return (
    <Show
      when={!isError()}
      fallback={
        <Show
          when={props.fallback}
          fallback={
            <div
              class={`flex items-center justify-center bg-[var(--color-surface-light)] text-[var(--color-text-muted)] ${props.class || ''}`}
              style={{
                width: typeof props.width === 'number' ? `${props.width}px` : props.width,
                height: typeof props.height === 'number' ? `${props.height}px` : props.height,
                ...props.style,
              }}
            >
              <span class="text-2xl">🖼️</span>
            </div>
          }
        >
          <img
            src={props.fallback}
            alt={props.alt}
            class={props.class}
            width={props.width}
            height={props.height}
            style={props.style}
          />
        </Show>
      }
    >
      <img
        src={props.src}
        alt={props.alt}
        class={props.class}
        width={props.width}
        height={props.height}
        style={props.style}
        loading="lazy"
        onLoad={props.onLoad}
        onError={() => {
          setIsError(true)
          props.onError?.()
        }}
      />
    </Show>
  )
}

export default LazyImage
