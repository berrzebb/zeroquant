/**
 * 즐겨찾기 버튼 컴포넌트
 *
 * 종목 즐겨찾기 토글 기능을 제공합니다.
 * localStorage를 사용하여 즐겨찾기 목록을 저장합니다.
 */
import { type Component, createSignal, createEffect, on } from 'solid-js'
import { createLogger } from '../../utils/logger'

const { warn } = createLogger('FavoriteButton')

const STORAGE_KEY = 'zeroquant_favorites'

export interface FavoriteButtonProps {
  /** 종목 티커 */
  ticker: string
  /** 크기 ('xs' | 'sm' | 'md') */
  size?: 'xs' | 'sm' | 'md'
  /** 변경 시 콜백 */
  onChange?: (ticker: string, isFavorite: boolean) => void
  /** 버튼 스타일 (아이콘만 vs 텍스트 포함) */
  variant?: 'icon' | 'text'
}

const SIZE_CLASSES = {
  xs: 'w-4 h-4',
  sm: 'w-5 h-5',
  md: 'w-6 h-6',
}

const BUTTON_CLASSES = {
  xs: 'p-0.5',
  sm: 'p-1',
  md: 'p-1.5',
}

/**
 * localStorage에서 즐겨찾기 목록 로드
 */
export function getFavorites(): Set<string> {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (stored) {
      return new Set(JSON.parse(stored))
    }
  } catch (e) {
    warn('즐겨찾기 로드 실패:', e)
  }
  return new Set()
}

/**
 * localStorage에 즐겨찾기 목록 저장
 */
function saveFavorites(favorites: Set<string>): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(Array.from(favorites)))
  } catch (e) {
    warn('즐겨찾기 저장 실패:', e)
  }
}

/**
 * 즐겨찾기 추가
 */
export function addFavorite(ticker: string): void {
  const favorites = getFavorites()
  favorites.add(ticker)
  saveFavorites(favorites)
}

/**
 * 즐겨찾기 제거
 */
export function removeFavorite(ticker: string): void {
  const favorites = getFavorites()
  favorites.delete(ticker)
  saveFavorites(favorites)
}

/**
 * 즐겨찾기 여부 확인
 */
export function isFavorite(ticker: string): boolean {
  return getFavorites().has(ticker)
}

/**
 * 즐겨찾기 토글
 */
export function toggleFavorite(ticker: string): boolean {
  const favorites = getFavorites()
  const newState = !favorites.has(ticker)
  if (newState) {
    favorites.add(ticker)
  } else {
    favorites.delete(ticker)
  }
  saveFavorites(favorites)
  return newState
}

/**
 * 즐겨찾기 버튼 컴포넌트
 */
export const FavoriteButton: Component<FavoriteButtonProps> = (props) => {
  const size = () => props.size || 'sm'
  const [isFav, setIsFav] = createSignal(false)

  // 초기 상태 로드
  createEffect(on(() => props.ticker, (ticker) => {
    setIsFav(isFavorite(ticker))
  }))

  const handleClick = (e: Event) => {
    e.stopPropagation() // 행 클릭 이벤트 방지
    const newState = toggleFavorite(props.ticker)
    setIsFav(newState)
    props.onChange?.(props.ticker, newState)
  }

  return (
    <button
      type="button"
      onClick={handleClick}
      class={`
        ${BUTTON_CLASSES[size()]}
        rounded-full transition-colors duration-150
        hover:bg-yellow-100 dark:hover:bg-yellow-900/30
        focus:outline-none focus:ring-2 focus:ring-yellow-400 focus:ring-offset-1
        ${isFav()
          ? 'text-yellow-500'
          : 'text-gray-300 dark:text-gray-600 hover:text-yellow-400'
        }
      `}
      title={isFav() ? '즐겨찾기 해제' : '즐겨찾기 추가'}
      aria-label={isFav() ? '즐겨찾기 해제' : '즐겨찾기 추가'}
    >
      <svg
        class={SIZE_CLASSES[size()]}
        fill={isFav() ? 'currentColor' : 'none'}
        viewBox="0 0 24 24"
        stroke="currentColor"
        stroke-width="2"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z"
        />
      </svg>
    </button>
  )
}

export default FavoriteButton
