/**
 * useDebounce - 디바운스 훅
 *
 * 입력값의 변경을 지연시켜 불필요한 API 호출이나 재계산을 방지합니다.
 * 검색 입력, 필터 변경 등에 유용합니다.
 */
import { createSignal, createEffect, onCleanup, type Accessor } from 'solid-js'

/**
 * 값의 디바운스된 버전을 반환합니다.
 *
 * @param value - 디바운스할 값 (시그널 또는 접근자)
 * @param delay - 지연 시간 (ms), 기본값 300ms
 * @returns 디바운스된 값 접근자
 *
 * @example
 * ```tsx
 * const [search, setSearch] = createSignal('')
 * const debouncedSearch = useDebounce(search, 500)
 *
 * // debouncedSearch()는 search()가 변경된 후 500ms 뒤에 업데이트됨
 * createEffect(() => {
 *   fetchResults(debouncedSearch())
 * })
 * ```
 */
export function useDebounce<T>(value: Accessor<T>, delay = 300): Accessor<T> {
  const [debouncedValue, setDebouncedValue] = createSignal<T>(value())

  createEffect(() => {
    const currentValue = value()
    const timer = setTimeout(() => {
      setDebouncedValue(() => currentValue)
    }, delay)

    onCleanup(() => clearTimeout(timer))
  })

  return debouncedValue
}

/**
 * 콜백의 디바운스된 버전을 반환합니다.
 *
 * @param callback - 디바운스할 콜백 함수
 * @param delay - 지연 시간 (ms), 기본값 300ms
 * @returns 디바운스된 콜백과 취소 함수
 *
 * @example
 * ```tsx
 * const { debouncedFn, cancel } = useDebouncedCallback(
 *   (query: string) => fetchSearch(query),
 *   500
 * )
 *
 * // 입력할 때마다 호출해도 마지막 호출만 실행됨
 * <input onInput={(e) => debouncedFn(e.target.value)} />
 * ```
 */
export function useDebouncedCallback<T extends (...args: unknown[]) => unknown>(
  callback: T,
  delay = 300
): { debouncedFn: T; cancel: () => void } {
  let timeoutId: ReturnType<typeof setTimeout> | null = null

  const debouncedFn = ((...args: Parameters<T>) => {
    if (timeoutId) {
      clearTimeout(timeoutId)
    }
    timeoutId = setTimeout(() => {
      callback(...args)
      timeoutId = null
    }, delay)
  }) as T

  const cancel = () => {
    if (timeoutId) {
      clearTimeout(timeoutId)
      timeoutId = null
    }
  }

  // 컴포넌트 언마운트 시 정리
  onCleanup(cancel)

  return { debouncedFn, cancel }
}

/**
 * 쓰로틀된 콜백을 반환합니다.
 * 디바운스와 달리 지정된 간격마다 실행을 보장합니다.
 *
 * @param callback - 쓰로틀할 콜백 함수
 * @param delay - 최소 간격 (ms), 기본값 300ms
 * @returns 쓰로틀된 콜백
 *
 * @example
 * ```tsx
 * const throttledScroll = useThrottledCallback(
 *   () => console.log('scroll'),
 *   100
 * )
 *
 * <div onScroll={throttledScroll} />
 * ```
 */
export function useThrottledCallback<T extends (...args: unknown[]) => unknown>(
  callback: T,
  delay = 300
): T {
  let lastRun = 0
  let timeoutId: ReturnType<typeof setTimeout> | null = null

  const throttledFn = ((...args: Parameters<T>) => {
    const now = Date.now()

    if (now - lastRun >= delay) {
      lastRun = now
      callback(...args)
    } else {
      // 마지막 호출 보장
      if (timeoutId) clearTimeout(timeoutId)
      timeoutId = setTimeout(() => {
        lastRun = Date.now()
        callback(...args)
        timeoutId = null
      }, delay - (now - lastRun))
    }
  }) as T

  onCleanup(() => {
    if (timeoutId) clearTimeout(timeoutId)
  })

  return throttledFn
}

export default useDebounce
