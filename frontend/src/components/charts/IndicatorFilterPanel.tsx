/**
 * 신호 필터링 패널 컴포넌트
 *
 * 캔들 차트에 표시되는 신호를 필터링하는 UI를 제공합니다.
 * RSI, MACD, RouteState, 전략 등 다양한 조건으로 필터링할 수 있습니다.
 */
import { type Component, createSignal, For, Show, onMount } from 'solid-js'
import { ChevronDown, ChevronUp, Filter, RotateCcw, Bookmark, Save, Trash2 } from 'lucide-solid'
import type { RouteStateType } from '../../types'
import { createLogger } from '../../utils/logger'
import { DateRangePicker } from '../ui'

const { error: logError } = createLogger('IndicatorFilterPanel')

// ==================== 타입 정의 ====================

export interface FilterPreset {
  /** 프리셋 ID */
  id: string
  /** 프리셋 이름 */
  name: string
  /** 필터 설정 */
  filters: IndicatorFilters
  /** 생성 시간 */
  createdAt: string
}

export interface IndicatorFilters {
  /** 신호 타입 필터 (매수/매도/알림 + 상세 신호 타입) */
  signal_types: ('buy' | 'sell' | 'alert' | 'entry' | 'exit' | 'add_to_position' | 'reduce_position' | 'scale')[]
  /** 지표 필터 (RSI, MACD, Bollinger, Volume) */
  indicators: string[]
  /** RSI 범위 필터 [min, max] */
  rsi_range?: [number, number]
  /** MACD 크로스 유형 */
  macd_type?: 'golden' | 'dead' | 'all'
  /** Bollinger Band 위치 */
  bollinger_position?: 'upper' | 'lower' | 'all'
  /** Volume 급증 배율 */
  volume_ratio_min?: number
  /** RouteState 필터 */
  route_states?: RouteStateType[]
  /** 전략 필터 */
  strategies?: string[]
  /** 날짜 범위 필터 */
  date_range?: [string, string]
}

export interface IndicatorFilterPanelProps {
  /** 현재 필터 상태 */
  filters: IndicatorFilters
  /** 필터 변경 콜백 */
  onChange: (filters: IndicatorFilters) => void
  /** 사용 가능한 전략 목록 */
  availableStrategies?: { id: string; name: string }[]
  /** 초기 접힘 상태 */
  defaultCollapsed?: boolean
  /** 컴팩트 모드 (작은 화면용) */
  compact?: boolean
  /** 프리셋 저장 키 (localStorage 키 prefix) */
  presetStorageKey?: string
}

// ==================== 프리셋 유틸리티 ====================

const PRESET_STORAGE_PREFIX = 'indicator_filter_presets_'

function loadPresets(storageKey: string): FilterPreset[] {
  try {
    const stored = localStorage.getItem(PRESET_STORAGE_PREFIX + storageKey)
    return stored ? JSON.parse(stored) : []
  } catch {
    return []
  }
}

function savePresets(storageKey: string, presets: FilterPreset[]): void {
  try {
    localStorage.setItem(PRESET_STORAGE_PREFIX + storageKey, JSON.stringify(presets))
  } catch (e) {
    logError('Failed to save presets:', e)
  }
}

function generatePresetId(): string {
  return `preset_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`
}

// ==================== 상수 ====================

const SIGNAL_TYPES = [
  { value: 'buy' as const, label: '매수', color: 'text-green-500' },
  { value: 'sell' as const, label: '매도', color: 'text-red-500' },
  { value: 'entry' as const, label: '진입', color: 'text-emerald-500' },
  { value: 'exit' as const, label: '청산', color: 'text-orange-500' },
  { value: 'add_to_position' as const, label: '추가매수', color: 'text-cyan-500' },
  { value: 'reduce_position' as const, label: '부분청산', color: 'text-amber-500' },
  { value: 'scale' as const, label: '스케일', color: 'text-purple-500' },
  { value: 'alert' as const, label: '알림', color: 'text-yellow-500' },
]

const INDICATORS = [
  { value: 'RSI', label: 'RSI', hasRange: true },
  { value: 'MACD', label: 'MACD', hasType: true },
  { value: 'Bollinger', label: '볼린저밴드', hasPosition: true },
  { value: 'Volume', label: '거래량', hasRatio: true },
]

const ROUTE_STATES: RouteStateType[] = ['ATTACK', 'ARMED', 'NEUTRAL', 'WAIT', 'OVERHEAT']

const ROUTE_STATE_COLORS: Record<RouteStateType, string> = {
  ATTACK: 'bg-red-500/20 text-red-400 border-red-500/30',
  ARMED: 'bg-orange-500/20 text-orange-400 border-orange-500/30',
  NEUTRAL: 'bg-gray-500/20 text-gray-400 border-gray-500/30',
  WAIT: 'bg-blue-500/20 text-blue-400 border-blue-500/30',
  OVERHEAT: 'bg-purple-500/20 text-purple-400 border-purple-500/30',
}

// ==================== 메인 컴포넌트 ====================

/**
 * 신호 필터링 패널
 *
 * @example
 * ```tsx
 * <IndicatorFilterPanel
 *   filters={currentFilters()}
 *   onChange={setFilters}
 *   availableStrategies={strategies}
 * />
 * ```
 */
export const IndicatorFilterPanel: Component<IndicatorFilterPanelProps> = (props) => {
  const [isCollapsed, setIsCollapsed] = createSignal(props.defaultCollapsed ?? false)
  const [presets, setPresets] = createSignal<FilterPreset[]>([])
  const [showPresetInput, setShowPresetInput] = createSignal(false)
  const [newPresetName, setNewPresetName] = createSignal('')

  const storageKey = () => props.presetStorageKey ?? 'default'

  // 초기 로드
  onMount(() => {
    setPresets(loadPresets(storageKey()))
  })

  // 프리셋 저장
  const savePreset = () => {
    const name = newPresetName().trim()
    if (!name) return

    const newPreset: FilterPreset = {
      id: generatePresetId(),
      name,
      filters: { ...props.filters },
      createdAt: new Date().toISOString(),
    }

    const updated = [...presets(), newPreset]
    setPresets(updated)
    savePresets(storageKey(), updated)
    setNewPresetName('')
    setShowPresetInput(false)
  }

  // 프리셋 불러오기
  const applyPreset = (preset: FilterPreset) => {
    props.onChange({ ...preset.filters })
  }

  // 프리셋 삭제
  const deletePreset = (presetId: string) => {
    const updated = presets().filter((p) => p.id !== presetId)
    setPresets(updated)
    savePresets(storageKey(), updated)
  }

  // 필터 업데이트 헬퍼
  const updateFilter = <K extends keyof IndicatorFilters>(
    key: K,
    value: IndicatorFilters[K]
  ) => {
    props.onChange({ ...props.filters, [key]: value })
  }

  // 배열 필터 토글 헬퍼
  const toggleArrayItem = <T,>(arr: T[], item: T): T[] => {
    return arr.includes(item) ? arr.filter((i) => i !== item) : [...arr, item]
  }

  // 신호 타입 토글
  const toggleSignalType = (type: IndicatorFilters['signal_types'][number]) => {
    updateFilter('signal_types', toggleArrayItem(props.filters.signal_types, type))
  }

  // 지표 토글
  const toggleIndicator = (indicator: string) => {
    updateFilter('indicators', toggleArrayItem(props.filters.indicators, indicator))
  }

  // RouteState 토글
  const toggleRouteState = (state: RouteStateType) => {
    const current = props.filters.route_states || []
    updateFilter('route_states', toggleArrayItem(current, state))
  }

  // 전략 토글
  const toggleStrategy = (strategyId: string) => {
    const current = props.filters.strategies || []
    updateFilter('strategies', toggleArrayItem(current, strategyId))
  }

  // 필터 초기화
  const resetFilters = () => {
    props.onChange({
      signal_types: [],
      indicators: [],
      rsi_range: undefined,
      macd_type: undefined,
      bollinger_position: undefined,
      volume_ratio_min: undefined,
      route_states: undefined,
      strategies: undefined,
      date_range: undefined,
    })
  }

  // 활성 필터 개수
  const activeFilterCount = () => {
    let count = 0
    if (props.filters.signal_types.length > 0) count++
    if (props.filters.indicators.length > 0) count++
    if (props.filters.route_states?.length) count++
    if (props.filters.strategies?.length) count++
    if (props.filters.rsi_range) count++
    if (props.filters.date_range) count++
    return count
  }

  return (
    <div class="bg-[var(--color-surface)] rounded-lg border border-[var(--color-surface-light)]">
      {/* 헤더 (클릭하여 접기/펼치기) */}
      <button
        type="button"
        onClick={() => setIsCollapsed(!isCollapsed())}
        class="w-full px-4 py-3 flex items-center justify-between hover:bg-[var(--color-surface-light)]/30 transition-colors"
      >
        <div class="flex items-center gap-2">
          <Filter class="w-4 h-4 text-[var(--color-text-muted)]" />
          <span class="text-sm font-medium text-[var(--color-text)]">신호 필터</span>
          <Show when={activeFilterCount() > 0}>
            <span class="px-1.5 py-0.5 bg-blue-500/20 text-blue-400 text-xs rounded-full">
              {activeFilterCount()}
            </span>
          </Show>
        </div>
        <div class="flex items-center gap-2">
          <Show when={activeFilterCount() > 0}>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation()
                resetFilters()
              }}
              class="p-1 hover:bg-[var(--color-surface-light)] rounded text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
              title="필터 초기화"
            >
              <RotateCcw class="w-3.5 h-3.5" />
            </button>
          </Show>
          {isCollapsed() ? (
            <ChevronDown class="w-4 h-4 text-[var(--color-text-muted)]" />
          ) : (
            <ChevronUp class="w-4 h-4 text-[var(--color-text-muted)]" />
          )}
        </div>
      </button>

      {/* 필터 내용 */}
      <Show when={!isCollapsed()}>
        <div class="px-4 pb-4 space-y-4 border-t border-[var(--color-surface-light)]">
          {/* 1. 신호 타입 체크박스 */}
          <div class="pt-4">
            <label class="text-xs text-[var(--color-text-muted)] mb-2 block">신호 타입</label>
            <div class="flex flex-wrap gap-2">
              <For each={SIGNAL_TYPES}>
                {(signalType) => (
                  <button
                    type="button"
                    onClick={() => toggleSignalType(signalType.value)}
                    class={`px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors ${
                      props.filters.signal_types.includes(signalType.value)
                        ? `${signalType.color} bg-current/10 border-current/30`
                        : 'text-[var(--color-text-muted)] border-[var(--color-surface-light)] hover:border-[var(--color-text-muted)]'
                    }`}
                  >
                    {signalType.label}
                  </button>
                )}
              </For>
            </div>
          </div>

          {/* 2. 지표 선택 */}
          <div>
            <label class="text-xs text-[var(--color-text-muted)] mb-2 block">지표</label>
            <div class="flex flex-wrap gap-2">
              <For each={INDICATORS}>
                {(indicator) => (
                  <button
                    type="button"
                    onClick={() => toggleIndicator(indicator.value)}
                    class={`px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors ${
                      props.filters.indicators.includes(indicator.value)
                        ? 'text-blue-400 bg-blue-500/10 border-blue-500/30'
                        : 'text-[var(--color-text-muted)] border-[var(--color-surface-light)] hover:border-[var(--color-text-muted)]'
                    }`}
                  >
                    {indicator.label}
                  </button>
                )}
              </For>
            </div>

            {/* RSI 범위 슬라이더 */}
            <Show when={props.filters.indicators.includes('RSI')}>
              <div class="mt-3 p-3 bg-[var(--color-surface-light)]/30 rounded-lg">
                <label class="text-xs text-[var(--color-text-muted)] mb-2 block">
                  RSI 범위: {props.filters.rsi_range?.[0] ?? 0} - {props.filters.rsi_range?.[1] ?? 100}
                </label>
                <div class="flex items-center gap-2">
                  <input
                    type="range"
                    min="0"
                    max="100"
                    value={props.filters.rsi_range?.[0] ?? 0}
                    onInput={(e) => {
                      const min = parseInt(e.currentTarget.value)
                      const max = props.filters.rsi_range?.[1] ?? 100
                      updateFilter('rsi_range', [min, Math.max(min, max)])
                    }}
                    class="flex-1 h-1.5 bg-[var(--color-surface-light)] rounded-lg appearance-none cursor-pointer"
                  />
                  <input
                    type="range"
                    min="0"
                    max="100"
                    value={props.filters.rsi_range?.[1] ?? 100}
                    onInput={(e) => {
                      const max = parseInt(e.currentTarget.value)
                      const min = props.filters.rsi_range?.[0] ?? 0
                      updateFilter('rsi_range', [Math.min(min, max), max])
                    }}
                    class="flex-1 h-1.5 bg-[var(--color-surface-light)] rounded-lg appearance-none cursor-pointer"
                  />
                </div>
              </div>
            </Show>

            {/* MACD 타입 선택 */}
            <Show when={props.filters.indicators.includes('MACD')}>
              <div class="mt-3 p-3 bg-[var(--color-surface-light)]/30 rounded-lg">
                <label class="text-xs text-[var(--color-text-muted)] mb-2 block">MACD 크로스</label>
                <div class="flex gap-2">
                  <For each={[
                    { value: 'golden', label: '골든크로스' },
                    { value: 'dead', label: '데드크로스' },
                    { value: 'all', label: '전체' },
                  ] as const}>
                    {(option) => (
                      <button
                        type="button"
                        onClick={() => updateFilter('macd_type', option.value)}
                        class={`px-2 py-1 text-xs rounded border transition-colors ${
                          props.filters.macd_type === option.value
                            ? 'text-blue-400 bg-blue-500/10 border-blue-500/30'
                            : 'text-[var(--color-text-muted)] border-[var(--color-surface-light)]'
                        }`}
                      >
                        {option.label}
                      </button>
                    )}
                  </For>
                </div>
              </div>
            </Show>

            {/* Bollinger 위치 선택 */}
            <Show when={props.filters.indicators.includes('Bollinger')}>
              <div class="mt-3 p-3 bg-[var(--color-surface-light)]/30 rounded-lg">
                <label class="text-xs text-[var(--color-text-muted)] mb-2 block">볼린저밴드 위치</label>
                <div class="flex gap-2">
                  <For each={[
                    { value: 'upper', label: '상단' },
                    { value: 'lower', label: '하단' },
                    { value: 'all', label: '전체' },
                  ] as const}>
                    {(option) => (
                      <button
                        type="button"
                        onClick={() => updateFilter('bollinger_position', option.value)}
                        class={`px-2 py-1 text-xs rounded border transition-colors ${
                          props.filters.bollinger_position === option.value
                            ? 'text-blue-400 bg-blue-500/10 border-blue-500/30'
                            : 'text-[var(--color-text-muted)] border-[var(--color-surface-light)]'
                        }`}
                      >
                        {option.label}
                      </button>
                    )}
                  </For>
                </div>
              </div>
            </Show>

            {/* Volume 배율 입력 */}
            <Show when={props.filters.indicators.includes('Volume')}>
              <div class="mt-3 p-3 bg-[var(--color-surface-light)]/30 rounded-lg">
                <label class="text-xs text-[var(--color-text-muted)] mb-2 block">
                  거래량 급증 배율 (최소 {props.filters.volume_ratio_min ?? 1}x)
                </label>
                <input
                  type="range"
                  min="1"
                  max="5"
                  step="0.5"
                  value={props.filters.volume_ratio_min ?? 1}
                  onInput={(e) => updateFilter('volume_ratio_min', parseFloat(e.currentTarget.value))}
                  class="w-full h-1.5 bg-[var(--color-surface-light)] rounded-lg appearance-none cursor-pointer"
                />
              </div>
            </Show>
          </div>

          {/* 3. RouteState 필터 */}
          <div>
            <label class="text-xs text-[var(--color-text-muted)] mb-2 block">RouteState</label>
            <div class="flex flex-wrap gap-2">
              <For each={ROUTE_STATES}>
                {(state) => (
                  <button
                    type="button"
                    onClick={() => toggleRouteState(state)}
                    class={`px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors ${
                      props.filters.route_states?.includes(state)
                        ? ROUTE_STATE_COLORS[state]
                        : 'text-[var(--color-text-muted)] border-[var(--color-surface-light)] hover:border-[var(--color-text-muted)]'
                    }`}
                  >
                    {state}
                  </button>
                )}
              </For>
            </div>
          </div>

          {/* 4. 전략 필터 (전략 목록이 있을 경우) */}
          <Show when={props.availableStrategies && props.availableStrategies.length > 0}>
            <div>
              <label class="text-xs text-[var(--color-text-muted)] mb-2 block">전략</label>
              <div class="flex flex-wrap gap-2">
                <For each={props.availableStrategies}>
                  {(strategy) => (
                    <button
                      type="button"
                      onClick={() => toggleStrategy(strategy.id)}
                      class={`px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors ${
                        props.filters.strategies?.includes(strategy.id)
                          ? 'text-purple-400 bg-purple-500/10 border-purple-500/30'
                          : 'text-[var(--color-text-muted)] border-[var(--color-surface-light)] hover:border-[var(--color-text-muted)]'
                      }`}
                    >
                      {strategy.name}
                    </button>
                  )}
                </For>
              </div>
            </div>
          </Show>

          {/* 5. 날짜 범위 필터 */}
          <div>
            <DateRangePicker
              label="날짜 범위"
              startDate={props.filters.date_range?.[0] ?? ''}
              endDate={props.filters.date_range?.[1] ?? ''}
              onStartDateChange={(start) => {
                const end = props.filters.date_range?.[1] ?? ''
                if (start) {
                  updateFilter('date_range', [start, end || start])
                } else {
                  updateFilter('date_range', undefined)
                }
              }}
              onEndDateChange={(end) => {
                const start = props.filters.date_range?.[0] ?? ''
                if (end) {
                  updateFilter('date_range', [start || end, end])
                } else if (!start) {
                  updateFilter('date_range', undefined)
                }
              }}
              showPresets
              size="sm"
            />
          </div>

          {/* 6. 프리셋 저장/불러오기 */}
          <div class="pt-4 border-t border-[var(--color-surface-light)]">
            <div class="flex items-center justify-between mb-2">
              <label class="text-xs text-[var(--color-text-muted)]">필터 프리셋</label>
              <Show when={activeFilterCount() > 0}>
                <button
                  type="button"
                  onClick={() => setShowPresetInput(!showPresetInput())}
                  class="flex items-center gap-1 px-2 py-1 text-xs text-blue-400 hover:bg-blue-500/10 rounded transition-colors"
                >
                  <Save class="w-3 h-3" />
                  저장
                </button>
              </Show>
            </div>

            {/* 프리셋 저장 입력 */}
            <Show when={showPresetInput()}>
              <div class="flex gap-2 mb-3">
                <input
                  type="text"
                  value={newPresetName()}
                  onInput={(e) => setNewPresetName(e.currentTarget.value)}
                  onKeyDown={(e) => e.key === 'Enter' && savePreset()}
                  placeholder="프리셋 이름 입력"
                  class="flex-1 px-3 py-1.5 text-xs rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-blue-500"
                />
                <button
                  type="button"
                  onClick={savePreset}
                  disabled={!newPresetName().trim()}
                  class="px-3 py-1.5 text-xs bg-blue-500 text-white rounded-lg hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  저장
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setShowPresetInput(false)
                    setNewPresetName('')
                  }}
                  class="px-2 py-1.5 text-xs text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
                >
                  취소
                </button>
              </div>
            </Show>

            {/* 프리셋 목록 */}
            <Show
              when={presets().length > 0}
              fallback={
                <div class="text-xs text-[var(--color-text-muted)] text-center py-2">
                  저장된 프리셋이 없습니다
                </div>
              }
            >
              <div class="space-y-1">
                <For each={presets()}>
                  {(preset) => (
                    <div class="flex items-center gap-2 p-2 rounded-lg hover:bg-[var(--color-surface-light)]/50 group">
                      <Bookmark class="w-3.5 h-3.5 text-[var(--color-text-muted)]" />
                      <button
                        type="button"
                        onClick={() => applyPreset(preset)}
                        class="flex-1 text-left text-xs text-[var(--color-text)] hover:text-blue-400 transition-colors"
                      >
                        {preset.name}
                      </button>
                      <button
                        type="button"
                        onClick={() => deletePreset(preset.id)}
                        class="p-1 text-[var(--color-text-muted)] hover:text-red-400 opacity-0 group-hover:opacity-100 transition-all"
                        title="프리셋 삭제"
                      >
                        <Trash2 class="w-3.5 h-3.5" />
                      </button>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>
        </div>
      </Show>
    </div>
  )
}

export default IndicatorFilterPanel
