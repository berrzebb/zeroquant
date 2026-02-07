/**
 * 재사용 가능한 심볼 검색 컴포넌트 (통합 버전)
 *
 * Dataset.tsx, DynamicForm.tsx, MultiPanelGrid.tsx, SDUIField.tsx 등에서
 * 동일한 심볼 검색 UI를 공유합니다.
 *
 * 기능:
 * - 단일/복수 선택 모드
 * - 시장 필터 (KR, US, CRYPTO, ALL)
 * - 로컬 필터링 (availableSymbols 제공 시)
 * - 선택된 심볼 이름 표시 옵션
 *
 * @example
 * ```tsx
 * // 단일 선택
 * <SymbolSearch
 *   value={symbol()}
 *   onSelect={(ticker, name) => setSymbol(ticker)}
 *   market="KR"
 *   showName
 * />
 *
 * // 복수 선택
 * <SymbolSearch
 *   value={symbols()}
 *   onChange={setSymbols}
 *   multiple
 *   maxItems={10}
 * />
 * ```
 */
import { createSignal, createEffect, on, For, Show, onCleanup } from 'solid-js'
import { Search, TrendingUp, Loader2, X, Plus } from 'lucide-solid'
import { searchSymbols, type SymbolSearchResult } from '../api/client'
import { SymbolDisplay } from './SymbolDisplay'

// ==================== 타입 ====================

/** 시장 필터 타입 */
export type MarketFilter = 'KR' | 'US' | 'CRYPTO' | 'ALL';

export interface SymbolSearchProps {
  /** 현재 선택된 심볼(들) - 단일 선택이면 string, 복수 선택이면 string[] */
  value?: string | string[]
  /** 심볼 선택 콜백 (단일) */
  onSelect?: (ticker: string, name?: string) => void
  /** 심볼 변경 콜백 (복수 - 전체 배열 전달) */
  onChange?: (symbols: string[]) => void
  /** 플레이스홀더 텍스트 */
  placeholder?: string
  /** 비활성화 여부 */
  disabled?: boolean
  /** 읽기 전용 */
  readOnly?: boolean
  /** 복수 선택 모드 여부 */
  multiple?: boolean
  /** 최대 선택 가능 수 (복수 모드용) */
  maxItems?: number
  /** 최소 선택 수 (복수 모드용) */
  minItems?: number
  /** 로컬 필터링을 위한 사전 정의된 심볼 목록 (선택적) */
  availableSymbols?: string[]
  /** 검색 결과 수 제한 */
  searchLimit?: number
  /** 크기 */
  size?: 'sm' | 'md'
  /** 검색 입력창 클래스 (추가) */
  inputClass?: string
  /** 자동 포커스 */
  autoFocus?: boolean
  /** 시장 필터 (KR, US, CRYPTO, ALL) */
  market?: MarketFilter
  /** 선택된 심볼의 이름도 표시할지 여부 (단일 모드) */
  showName?: boolean
  /** 에러 상태 */
  hasError?: boolean
  /** HTML id */
  id?: string
}

// ==================== 유틸리티 ====================

/**
 * 시장 필터 적용
 */
function filterByMarket(results: SymbolSearchResult[], market?: MarketFilter): SymbolSearchResult[] {
  if (!market || market === 'ALL') return results;

  return results.filter((r) => {
    switch (market) {
      case 'KR':
        return r.market === 'KOSPI' || r.market === 'KOSDAQ' || r.market === 'KRX';
      case 'US':
        return r.market === 'NYSE' || r.market === 'NASDAQ' || r.market === 'AMEX';
      case 'CRYPTO':
        return r.market === 'CRYPTO' || r.market === 'BINANCE';
      default:
        return true;
    }
  });
}

// ==================== 컴포넌트 ====================

export function SymbolSearch(props: SymbolSearchProps) {
  const [query, setQuery] = createSignal('')
  const [results, setResults] = createSignal<SymbolSearchResult[]>([])
  const [isLoading, setIsLoading] = createSignal(false)
  const [selectedIndex, setSelectedIndex] = createSignal(-1)
  const [showDropdown, setShowDropdown] = createSignal(false)
  const [isEditing, setIsEditing] = createSignal(false)
  // 선택된 심볼의 표시명 (티커 + 이름)
  const [selectedDisplay, setSelectedDisplay] = createSignal('')

  const isDisabled = () => props.disabled || props.readOnly;

  // 선택된 심볼 목록 (복수 모드용)
  const selectedSymbols = (): string[] => {
    if (!props.value) return []
    return Array.isArray(props.value) ? props.value : [props.value]
  }

  // 단일 선택 모드에서 현재 선택된 값
  const singleSelectedValue = (): string | null => {
    if (props.multiple) return null
    if (!props.value) return null
    return Array.isArray(props.value) ? props.value[0] || null : props.value
  }

  // props.value 변경 시 selectedDisplay 초기화 (명시적 의존성)
  createEffect(on(singleSelectedValue, (value) => {
    if (value && !selectedDisplay()) {
      // 초기 로드 시 표시명 설정 (showName이 true이고 이름이 없으면 티커만 표시)
      setSelectedDisplay(value);
    }
  }));

  // 디바운스 타이머
  let searchTimeout: ReturnType<typeof setTimeout> | null = null

  onCleanup(() => {
    if (searchTimeout) clearTimeout(searchTimeout)
  })

  // 검색 실행
  const performSearch = async (searchQuery: string) => {
    if (!searchQuery.trim()) {
      setResults([])
      return
    }

    setIsLoading(true)
    try {
      // API 검색 우선, 로컬 필터링 폴백
      if (props.availableSymbols && props.availableSymbols.length > 0) {
        // 로컬 필터링 (availableSymbols가 제공된 경우)
        const queryUpper = searchQuery.toUpperCase()
        const filtered = props.availableSymbols
          .filter(s => s.toUpperCase().includes(queryUpper))
          .slice(0, props.searchLimit || 10)
          .map(ticker => ({ ticker, name: '', market: '', yahooSymbol: null }))
        setResults(filtered)
      } else {
        // API 검색
        const apiResults = await searchSymbols(searchQuery, props.searchLimit || 10)
        // 시장 필터 적용
        const filteredResults = filterByMarket(apiResults, props.market);
        setResults(filteredResults)
      }
    } catch {
      setResults([])
    } finally {
      setIsLoading(false)
    }
  }

  // 입력 변경 핸들러 (디바운스)
  const handleInput = (value: string) => {
    setQuery(value)
    setSelectedIndex(-1)
    setShowDropdown(true)
    setSelectedDisplay('') // 입력 시 표시명 클리어

    if (searchTimeout) clearTimeout(searchTimeout)
    searchTimeout = setTimeout(() => performSearch(value), 250)
  }

  // 심볼 선택 핸들러
  const handleSelect = (ticker: string, name?: string) => {
    if (props.multiple) {
      // 복수 선택 모드
      const current = selectedSymbols()
      if (!current.includes(ticker)) {
        const maxItems = props.maxItems
        if (!maxItems || current.length < maxItems) {
          props.onChange?.([...current, ticker])
        }
      }
    } else {
      // 단일 선택 모드
      props.onSelect?.(ticker, name)
      // 선택된 표시명 설정 (showName이 true이고 이름이 있으면 "티커 (이름)" 형식)
      if (props.showName && name) {
        setSelectedDisplay(`${ticker} (${name})`)
      } else {
        setSelectedDisplay(ticker)
      }
      setIsEditing(false)
    }

    setQuery('')
    setResults([])
    setSelectedIndex(-1)
    setShowDropdown(false)
  }

  // 단일 선택 모드에서 편집 시작
  const startEditing = () => {
    if (!isDisabled()) {
      setIsEditing(true)
      setQuery('')
      setSelectedDisplay('')
    }
  }

  // 단일 선택 모드에서 선택 해제
  const handleClearSingle = () => {
    if (!isDisabled()) {
      props.onSelect?.('')
      setSelectedDisplay('')
      setIsEditing(true)
    }
  }

  // 심볼 제거 핸들러 (복수 모드용)
  const handleRemove = (ticker: string) => {
    if (props.multiple) {
      props.onChange?.(selectedSymbols().filter(s => s !== ticker))
    }
  }

  // 키보드 핸들러
  const handleKeyDown = (e: KeyboardEvent) => {
    const items = results()
    const len = items.length

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        setSelectedIndex(prev => len > 0 ? (prev + 1) % len : -1)
        break
      case 'ArrowUp':
        e.preventDefault()
        setSelectedIndex(prev => len > 0 ? (prev - 1 + len) % len : -1)
        break
      case 'Enter':
        e.preventDefault()
        const idx = selectedIndex()
        if (idx >= 0 && idx < len) {
          handleSelect(items[idx].ticker, items[idx].name)
        } else if (query().trim()) {
          // 검색어가 있으면 그대로 사용 (티커로 간주)
          handleSelect(query().trim().toUpperCase())
        }
        break
      case 'Escape':
        setShowDropdown(false)
        setSelectedIndex(-1)
        setResults([])
        // 편집 취소 시 원래 값으로 복원
        if (isEditing() && singleSelectedValue()) {
          setIsEditing(false)
        }
        break
    }
  }

  // 포커스 아웃 시 드롭다운 닫기
  const handleBlur = () => {
    // 약간의 딜레이 후 닫기 (클릭 이벤트가 먼저 처리되도록)
    setTimeout(() => {
      setShowDropdown(false)
      setSelectedIndex(-1)
      // 편집 중이었고 선택하지 않았으면 원래 값으로 복원
      if (isEditing() && singleSelectedValue()) {
        setIsEditing(false)
        if (!selectedDisplay()) {
          setSelectedDisplay(singleSelectedValue()!)
        }
      }
    }, 200)
  }

  // 사이즈별 스타일
  const sizeClasses = () => {
    if (props.size === 'sm') {
      return {
        input: 'px-2 py-1.5 text-sm',
        icon: 'w-3.5 h-3.5',
        tag: 'px-1.5 py-0.5 text-xs',
        button: 'p-1.5',
      }
    }
    return {
      input: 'px-3 py-2 text-sm',
      icon: 'w-4 h-4',
      tag: 'px-2 py-1 text-sm',
      button: 'p-2',
    }
  }

  const sizes = sizeClasses()

  // 표시할 값 결정
  const displayValue = () => {
    if (selectedDisplay()) return selectedDisplay();
    return singleSelectedValue() || '';
  }

  return (
    <div id={props.id} class="space-y-2">
      {/* 복수 선택 모드: 선택된 심볼 태그 - SymbolDisplay 사용 */}
      <Show when={props.multiple && selectedSymbols().length > 0}>
        <div class="flex flex-wrap gap-2">
          <For each={selectedSymbols()}>
            {(symbol) => (
              <span class={`inline-flex items-center gap-2 ${sizes.tag} bg-[var(--color-primary)]/20 text-[var(--color-primary)] rounded-lg`}>
                <SymbolDisplay
                  ticker={symbol}
                  mode="compact"
                  size="sm"
                />
                <button
                  type="button"
                  onClick={() => handleRemove(symbol)}
                  disabled={isDisabled()}
                  class="p-0.5 hover:bg-[var(--color-primary)]/20 rounded"
                >
                  <X class="w-3 h-3" />
                </button>
              </span>
            )}
          </For>
        </div>
      </Show>

      {/* 검색 입력 영역 */}
      <div class="relative">
        <div class="flex gap-2">
          <div class="relative flex-1">
            {/* 단일 선택 모드: 선택된 값 표시 또는 검색창 */}
            <Show when={!props.multiple && singleSelectedValue() && !isEditing()} fallback={
              <>
                <Search class={`absolute left-3 top-1/2 -translate-y-1/2 ${sizes.icon} text-[var(--color-text-muted)]`} />
                <input
                  type="text"
                  value={query()}
                  onInput={(e) => handleInput(e.currentTarget.value)}
                  onKeyDown={handleKeyDown}
                  onFocus={() => setShowDropdown(true)}
                  onBlur={handleBlur}
                  placeholder={props.placeholder || '종목 코드 또는 이름 검색'}
                  disabled={isDisabled()}
                  autofocus={props.autoFocus || isEditing()}
                  class={`w-full pl-9 ${sizes.input} ${props.inputClass || ''} bg-[var(--color-surface)] border ${props.hasError ? 'border-red-500' : 'border-[var(--color-surface-light)]'} rounded-lg text-[var(--color-text)] placeholder:text-[var(--color-text-muted)] focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)] disabled:opacity-50`}
                />
              </>
            }>
              {/* 선택된 심볼 표시 (단일 모드) - SymbolDisplay 사용 */}
              <div
                onClick={startEditing}
                class={`flex items-center gap-2 ${sizes.input} pl-3 pr-2 bg-[var(--color-surface)] border ${props.hasError ? 'border-red-500' : 'border-[var(--color-surface-light)]'} rounded-lg cursor-pointer hover:border-[var(--color-primary)] transition-colors ${isDisabled() ? 'opacity-50 cursor-not-allowed' : ''}`}
              >
                <SymbolDisplay
                  ticker={singleSelectedValue()!}
                  mode="full"
                  size="sm"
                  class="flex-1"
                />
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation()
                    handleClearSingle()
                  }}
                  disabled={isDisabled()}
                  class="p-1 hover:bg-[var(--color-surface-light)] rounded transition-colors"
                >
                  <X class={`${sizes.icon} text-[var(--color-text-muted)]`} />
                </button>
              </div>
            </Show>
          </div>
          {/* 복수 모드: 추가 버튼 */}
          <Show when={props.multiple}>
            <button
              type="button"
              onClick={() => {
                if (query().trim()) {
                  handleSelect(query().trim().toUpperCase())
                }
              }}
              disabled={isDisabled() || !query().trim()}
              class={`${sizes.button} bg-[var(--color-primary)] text-white rounded-lg hover:bg-[var(--color-primary)]/90 disabled:opacity-50 disabled:cursor-not-allowed`}
            >
              <Plus class={sizes.icon} />
            </button>
          </Show>
        </div>

        {/* 검색 결과 드롭다운 */}
        <Show when={showDropdown() && (results().length > 0 || isLoading())}>
          <div class="absolute top-full left-0 right-0 mt-1 bg-[var(--color-surface)] border border-[var(--color-border)] rounded-lg shadow-lg z-50 max-h-64 overflow-y-auto">
            {/* 로딩 표시 */}
            <Show when={isLoading()}>
              <div class="p-3 text-center">
                <Loader2 class="w-5 h-5 animate-spin mx-auto text-[var(--color-primary)]" />
              </div>
            </Show>

            {/* 검색 결과 */}
            <Show when={!isLoading()}>
              <For each={results()}>
                {(result, idx) => (
                  <button
                    type="button"
                    onClick={() => handleSelect(result.ticker, result.name)}
                    class={`w-full px-3 py-2 text-left flex items-center gap-2 hover:bg-[var(--color-surface-light)] transition-colors ${
                      idx() === selectedIndex() ? 'bg-[var(--color-surface-light)]' : ''
                    }`}
                  >
                    <TrendingUp class="w-4 h-4 text-[var(--color-primary)]" />
                    <span class="font-medium text-[var(--color-text)]">{result.ticker}</span>
                    <Show when={result.name}>
                      <span class="text-sm text-[var(--color-text-muted)] truncate">{result.name}</span>
                    </Show>
                    <Show when={result.market}>
                      <span class="ml-auto text-xs px-1.5 py-0.5 rounded bg-[var(--color-primary)]/10 text-[var(--color-primary)]">
                        {result.market}
                      </span>
                    </Show>
                  </button>
                )}
              </For>
            </Show>

            {/* 결과 없음 메시지 */}
            <Show when={!isLoading() && results().length === 0 && query().trim()}>
              <div class="px-3 py-2 text-sm text-[var(--color-text-muted)]">
                검색 결과가 없습니다
              </div>
            </Show>
          </div>
        </Show>
      </div>

      {/* 복수 모드: 제한 표시 */}
      <Show when={props.multiple && (props.minItems || props.maxItems)}>
        <p class="text-xs text-[var(--color-text-muted)]">
          {props.minItems && `최소 ${props.minItems}개`}
          {props.minItems && props.maxItems && ' ~ '}
          {props.maxItems && `최대 ${props.maxItems}개`}
          {' '}(현재 {selectedSymbols().length}개)
        </p>
      </Show>
    </div>
  )
}

export default SymbolSearch
