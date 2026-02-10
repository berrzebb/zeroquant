/**
 * 심볼 표시 컴포넌트
 *
 * 티커명을 받아 3줄 형태로 표시합니다:
 * - 티커
 * - 심볼명 (한글명)
 * - 거래소
 *
 * 심볼 정보가 없으면 티커만 표시합니다.
 * 모든 심볼 텍스트 표시는 이 컴포넌트를 통해 수행합니다.
 */
import { createSignal, createEffect, on, Show } from 'solid-js'
import { searchSymbols, type SymbolSearchResult } from '../api/client'
import { createLogger } from '../utils/logger'

const { warn } = createLogger('SymbolDisplay')

// ==================== 타입 ====================

export interface SymbolDisplayProps {
  /** 티커 심볼 (필수) */
  ticker: string
  /** 이미 알고 있는 심볼명 (API 호출 없이 사용) */
  symbolName?: string | null
  /** 이미 알고 있는 거래소 */
  exchange?: string | null
  /** 표시 모드: 'full' (3줄), 'compact' (1줄), 'inline' (티커(심볼명) 형식) */
  mode?: 'full' | 'compact' | 'inline'
  /** 크기: 'sm', 'md', 'lg' */
  size?: 'sm' | 'md' | 'lg'
  /** 심볼 정보를 자동으로 API에서 가져올지 여부 */
  autoFetch?: boolean
  /** 추가 CSS 클래스 */
  class?: string
}

// 심볼 정보 캐시 (세션 동안 유지)
const symbolCache = new Map<string, SymbolSearchResult | null>()

// ==================== 컴포넌트 ====================

export function SymbolDisplay(props: SymbolDisplayProps) {
  const [symbolInfo, setSymbolInfo] = createSignal<SymbolSearchResult | null>(null)
  const [isLoading, setIsLoading] = createSignal(false)

  // 심볼 정보 가져오기 (캐시 우선)
  const fetchSymbolInfo = async (ticker: string) => {
    // 이미 symbolName이 제공되면 API 호출 안 함
    if (props.symbolName !== undefined) {
      return
    }

    // 캐시 확인
    if (symbolCache.has(ticker)) {
      setSymbolInfo(symbolCache.get(ticker) || null)
      return
    }

    // autoFetch가 false면 API 호출 안 함
    if (props.autoFetch === false) {
      return
    }

    setIsLoading(true)
    try {
      const results = await searchSymbols(ticker, 1)
      const exactMatch = results.find(r => r.ticker.toUpperCase() === ticker.toUpperCase())
      const result = exactMatch || results[0] || null

      // 캐시에 저장
      symbolCache.set(ticker, result)
      setSymbolInfo(result)
    } catch (error) {
      warn(`심볼 정보 조회 실패: ${ticker}`, error)
      symbolCache.set(ticker, null) // 실패도 캐시 (반복 요청 방지)
    } finally {
      setIsLoading(false)
    }
  }

  // 티커 변경 시 심볼 정보 가져오기 (명시적 의존성)
  createEffect(on(() => props.ticker, (ticker) => {
    if (ticker) {
      fetchSymbolInfo(ticker)
    }
  }))

  // 최종 표시할 정보
  const displayName = () => props.symbolName ?? symbolInfo()?.name ?? null
  const displayExchange = () => props.exchange ?? symbolInfo()?.market ?? null
  const displayTicker = () => props.ticker

  // 크기별 스타일
  const sizeStyles = () => {
    switch (props.size) {
      case 'sm':
        return {
          ticker: 'text-xs font-medium',
          name: 'text-[10px]',
          exchange: 'text-[10px]',
        }
      case 'lg':
        return {
          ticker: 'text-base font-semibold',
          name: 'text-sm',
          exchange: 'text-xs',
        }
      default: // 'md'
        return {
          ticker: 'text-sm font-medium',
          name: 'text-xs',
          exchange: 'text-xs',
        }
    }
  }

  // 모드별 렌더링
  const renderContent = () => {
    const mode = props.mode || 'full'
    const styles = sizeStyles()

    // 인라인 모드: "티커(심볼명)" 형식
    if (mode === 'inline') {
      const name = displayName()
      if (name && name !== displayTicker()) {
        return (
          <span class={`${styles.ticker} text-white`}>
            {displayTicker()}
            <span class="text-gray-400">({name})</span>
          </span>
        )
      }
      return <span class={`${styles.ticker} text-white`}>{displayTicker()}</span>
    }

    // 컴팩트 모드: 1줄 (티커 + 심볼명 + 거래소)
    if (mode === 'compact') {
      const name = displayName()
      const exchange = displayExchange()
      return (
        <div class="flex items-center gap-2">
          <span class={`${styles.ticker} text-white`}>{displayTicker()}</span>
          <Show when={name && name !== displayTicker()}>
            <span class={`${styles.name} text-gray-400`}>{name}</span>
          </Show>
          <Show when={exchange}>
            <span class={`${styles.exchange} text-gray-500 px-1 py-0.5 rounded bg-gray-700`}>
              {exchange}
            </span>
          </Show>
        </div>
      )
    }

    // 풀 모드: 3줄 (티커, 심볼명, 거래소)
    return (
      <div class="flex flex-col">
        <span class={`${styles.ticker} text-white`}>{displayTicker()}</span>
        <Show when={displayName() && displayName() !== displayTicker()}>
          <span class={`${styles.name} text-gray-400 truncate`}>{displayName()}</span>
        </Show>
        <Show when={displayExchange()}>
          <span class={`${styles.exchange} text-gray-500`}>{displayExchange()}</span>
        </Show>
      </div>
    )
  }

  return (
    <div class={props.class}>
      <Show when={isLoading()} fallback={renderContent()}>
        <div class="flex items-center gap-1">
          <span class={`${sizeStyles().ticker} text-white`}>{displayTicker()}</span>
          <span class="text-gray-500 text-xs animate-pulse">...</span>
        </div>
      </Show>
    </div>
  )
}

// ==================== 유틸리티 함수 ====================

/**
 * 심볼 캐시 초기화
 * (새로운 데이터가 필요할 때 호출)
 */
export function clearSymbolCache() {
  symbolCache.clear()
}

/**
 * 특정 심볼을 캐시에 미리 등록
 * (API 응답에서 심볼 정보를 받았을 때 사용)
 */
export function cacheSymbolInfo(ticker: string, name: string, market?: string) {
  symbolCache.set(ticker, {
    ticker,
    name,
    market: market || '',
    yahooSymbol: null,
  })
}

/**
 * 여러 심볼을 한 번에 캐시에 등록
 */
export function cacheSymbolInfoBatch(symbols: Array<{ ticker: string; name: string; market?: string }>) {
  symbols.forEach(s => cacheSymbolInfo(s.ticker, s.name, s.market))
}

export default SymbolDisplay
