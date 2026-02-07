/**
 * 폼 컴포넌트
 *
 * 입력 필드, 선택 박스, 필터 패널 등 폼 관련 컴포넌트를 제공합니다.
 */
import { type Component, type JSX, For, Show } from 'solid-js'

// ==================== Select ====================

export interface SelectOption {
  value: string
  label: string
  disabled?: boolean
}

export interface SelectProps {
  /** 현재 선택된 값 */
  value: string
  /** 값 변경 핸들러 */
  onChange: (value: string) => void
  /** 옵션 목록 */
  options: SelectOption[]
  /** 라벨 */
  label?: string
  /** 플레이스홀더 */
  placeholder?: string
  /** 비활성화 여부 */
  disabled?: boolean
  /** 크기 */
  size?: 'sm' | 'md' | 'lg'
  /** 추가 CSS 클래스 */
  className?: string
}

/**
 * 선택 박스 컴포넌트
 *
 * @example
 * ```tsx
 * <Select
 *   label="시장"
 *   value={market()}
 *   onChange={setMarket}
 *   options={[
 *     { value: '', label: '전체' },
 *     { value: 'KR', label: '한국' },
 *   ]}
 * />
 * ```
 */
export const Select: Component<SelectProps> = (props) => {
  const sizeClasses = {
    sm: 'px-2 py-1 text-xs',
    md: 'px-3 py-2 text-sm',
    lg: 'px-4 py-3 text-base',
  }

  return (
    <div class={`flex flex-col gap-1 ${props.className || ''}`}>
      <Show when={props.label}>
        <label class="text-xs font-medium text-gray-600 dark:text-gray-400">{props.label}</label>
      </Show>
      <select
        value={props.value}
        onChange={(e) => props.onChange(e.currentTarget.value)}
        disabled={props.disabled}
        class={`
          border border-gray-300 dark:border-gray-600 rounded-md
          bg-white dark:bg-gray-700 text-gray-900 dark:text-white
          focus:ring-2 focus:ring-blue-500 focus:border-blue-500
          disabled:opacity-50 disabled:cursor-not-allowed
          ${sizeClasses[props.size || 'md']}
        `}
      >
        <Show when={props.placeholder}>
          <option value="" disabled>
            {props.placeholder}
          </option>
        </Show>
        <For each={props.options}>
          {(opt) => (
            <option value={opt.value} disabled={opt.disabled}>
              {opt.label}
            </option>
          )}
        </For>
      </select>
    </div>
  )
}

// ==================== Input ====================

export interface InputProps {
  /** 현재 값 */
  value: string | number
  /** 값 변경 핸들러 */
  onInput: (value: string) => void
  /** 입력 타입 */
  type?: 'text' | 'number' | 'email' | 'password' | 'search'
  /** 라벨 */
  label?: string
  /** 플레이스홀더 */
  placeholder?: string
  /** 비활성화 여부 */
  disabled?: boolean
  /** 크기 */
  size?: 'sm' | 'md' | 'lg'
  /** 최소값 (number 타입) */
  min?: number
  /** 최대값 (number 타입) */
  max?: number
  /** 단계 (number 타입) */
  step?: number
  /** 너비 클래스 */
  width?: string
  /** 추가 CSS 클래스 */
  className?: string
}

/**
 * 입력 필드 컴포넌트
 *
 * @example
 * ```tsx
 * <Input
 *   label="최소 점수"
 *   type="number"
 *   value={minScore()}
 *   onInput={setMinScore}
 *   min={0}
 *   max={100}
 *   width="w-24"
 * />
 * ```
 */
export const Input: Component<InputProps> = (props) => {
  const sizeClasses = {
    sm: 'px-2 py-1 text-xs',
    md: 'px-3 py-2 text-sm',
    lg: 'px-4 py-3 text-base',
  }

  return (
    <div class={`flex flex-col gap-1 ${props.className || ''}`}>
      <Show when={props.label}>
        <label class="text-xs font-medium text-gray-600 dark:text-gray-400">{props.label}</label>
      </Show>
      <input
        type={props.type || 'text'}
        value={props.value}
        onInput={(e) => props.onInput(e.currentTarget.value)}
        placeholder={props.placeholder}
        disabled={props.disabled}
        min={props.min}
        max={props.max}
        step={props.step}
        class={`
          border border-gray-300 dark:border-gray-600 rounded-md
          bg-white dark:bg-gray-700 text-gray-900 dark:text-white
          focus:ring-2 focus:ring-blue-500 focus:border-blue-500
          disabled:opacity-50 disabled:cursor-not-allowed
          ${sizeClasses[props.size || 'md']}
          ${props.width || ''}
        `}
      />
    </div>
  )
}

// ==================== Filter Panel ====================

export interface FilterPanelProps {
  children: JSX.Element
  /** 추가 CSS 클래스 */
  className?: string
}

/**
 * 필터 패널 컨테이너
 *
 * 여러 필터 컴포넌트를 그룹화하여 표시합니다.
 *
 * @example
 * ```tsx
 * <FilterPanel>
 *   <Select ... />
 *   <Select ... />
 *   <Input ... />
 * </FilterPanel>
 * ```
 */
export const FilterPanel: Component<FilterPanelProps> = (props) => {
  return (
    <div
      class={`
        flex flex-wrap gap-4 p-4 bg-gray-50 dark:bg-gray-800/50 rounded-lg
        ${props.className || ''}
      `}
    >
      {props.children}
    </div>
  )
}

// ==================== Button ====================

export interface ButtonProps {
  children: JSX.Element
  /** 버튼 타입 */
  type?: 'button' | 'submit' | 'reset'
  /** 버튼 스타일 변형 */
  variant?: 'primary' | 'secondary' | 'danger' | 'ghost'
  /** 크기 */
  size?: 'sm' | 'md' | 'lg'
  /** 비활성화 여부 */
  disabled?: boolean
  /** 로딩 상태 */
  loading?: boolean
  /** 클릭 핸들러 */
  onClick?: () => void
  /** 추가 CSS 클래스 */
  className?: string
}

/**
 * 버튼 컴포넌트
 *
 * @example
 * ```tsx
 * <Button variant="primary" onClick={submit} loading={isLoading()}>
 *   저장
 * </Button>
 * ```
 */
export const Button: Component<ButtonProps> = (props) => {
  const variantClasses = {
    primary: 'bg-blue-600 hover:bg-blue-700 text-white',
    secondary: 'bg-gray-100 hover:bg-gray-200 dark:bg-gray-700 dark:hover:bg-gray-600 text-gray-700 dark:text-gray-300',
    danger: 'bg-red-600 hover:bg-red-700 text-white',
    ghost: 'hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-700 dark:text-gray-300',
  }

  const sizeClasses = {
    sm: 'px-2 py-1 text-xs',
    md: 'px-4 py-2 text-sm',
    lg: 'px-6 py-3 text-base',
  }

  return (
    <button
      type={props.type || 'button'}
      onClick={props.onClick}
      disabled={props.disabled || props.loading}
      class={`
        font-medium rounded-lg transition-colors
        disabled:opacity-50 disabled:cursor-not-allowed
        flex items-center justify-center gap-2
        ${variantClasses[props.variant || 'primary']}
        ${sizeClasses[props.size || 'md']}
        ${props.className || ''}
      `}
    >
      <Show when={props.loading}>
        <span class="animate-spin">⏳</span>
      </Show>
      {props.children}
    </button>
  )
}

// ==================== Search Input ====================

export interface SearchInputProps {
  /** 현재 값 */
  value: string
  /** 값 변경 핸들러 */
  onInput: (value: string) => void
  /** 플레이스홀더 */
  placeholder?: string
  /** 검색 실행 핸들러 (Enter 키) */
  onSearch?: (value: string) => void
  /** 크기 */
  size?: 'sm' | 'md' | 'lg'
  /** 추가 CSS 클래스 */
  className?: string
}

/**
 * 검색 입력 필드
 *
 * 검색 아이콘과 함께 표시되는 입력 필드입니다.
 */
export const SearchInput: Component<SearchInputProps> = (props) => {
  const sizeClasses = {
    sm: 'pl-8 pr-3 py-1 text-xs',
    md: 'pl-10 pr-4 py-2 text-sm',
    lg: 'pl-12 pr-5 py-3 text-base',
  }

  const iconSizes = {
    sm: 'w-3 h-3 left-2.5',
    md: 'w-4 h-4 left-3',
    lg: 'w-5 h-5 left-4',
  }

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Enter' && props.onSearch) {
      props.onSearch(props.value)
    }
  }

  return (
    <div class={`relative ${props.className || ''}`}>
      <svg
        class={`absolute top-1/2 -translate-y-1/2 text-gray-400 ${iconSizes[props.size || 'md']}`}
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
        />
      </svg>
      <input
        type="search"
        value={props.value}
        onInput={(e) => props.onInput(e.currentTarget.value)}
        onKeyDown={handleKeyDown}
        placeholder={props.placeholder || '검색...'}
        class={`
          w-full border border-gray-300 dark:border-gray-600 rounded-md
          bg-white dark:bg-gray-700 text-gray-900 dark:text-white
          focus:ring-2 focus:ring-blue-500 focus:border-blue-500
          ${sizeClasses[props.size || 'md']}
        `}
      />
    </div>
  )
}

// ==================== Date Input ====================

export interface DatePreset {
  /** 프리셋 라벨 */
  label: string
  /** 날짜 계산 함수 (Date 반환) */
  getValue: () => Date
}

/** 기본 날짜 프리셋 */
export const DEFAULT_DATE_PRESETS: DatePreset[] = [
  { label: '오늘', getValue: () => new Date() },
  { label: '1주전', getValue: () => new Date(Date.now() - 7 * 24 * 60 * 60 * 1000) },
  { label: '1개월전', getValue: () => new Date(Date.now() - 30 * 24 * 60 * 60 * 1000) },
  { label: '3개월전', getValue: () => new Date(Date.now() - 90 * 24 * 60 * 60 * 1000) },
  { label: '1년전', getValue: () => new Date(Date.now() - 365 * 24 * 60 * 60 * 1000) },
]

export interface DateInputProps {
  /** 현재 값 (YYYY-MM-DD 형식) */
  value: string
  /** 값 변경 핸들러 */
  onChange: (value: string) => void
  /** 라벨 */
  label?: string
  /** 비활성화 여부 */
  disabled?: boolean
  /** 크기 */
  size?: 'sm' | 'md' | 'lg'
  /** 최소 날짜 */
  min?: string
  /** 최대 날짜 */
  max?: string
  /** 프리셋 버튼 표시 여부 */
  showPresets?: boolean
  /** 커스텀 프리셋 (기본: DEFAULT_DATE_PRESETS) */
  presets?: DatePreset[]
  /** 추가 CSS 클래스 */
  className?: string
}

/**
 * 날짜 입력 컴포넌트
 *
 * 캘린더 아이콘과 프리셋 버튼을 지원하는 날짜 입력 필드입니다.
 *
 * @example
 * ```tsx
 * <DateInput
 *   label="시작일"
 *   value={startDate()}
 *   onChange={setStartDate}
 *   showPresets
 * />
 * ```
 */
export const DateInput: Component<DateInputProps> = (props) => {
  const sizeClasses = {
    sm: 'px-1 py-1 text-xs',
    md: 'px-1.5 py-2 text-sm',
    lg: 'px-2 py-3 text-base',
  }

  const yearWidth = { sm: 'w-12', md: 'w-14', lg: 'w-16' }
  const monthDayWidth = { sm: 'w-8', md: 'w-10', lg: 'w-12' }

  const presetSizeClasses = {
    sm: 'px-1.5 py-0.5 text-xs',
    md: 'px-2 py-1 text-xs',
    lg: 'px-2.5 py-1.5 text-sm',
  }

  const iconSizes = {
    sm: 'w-3 h-3',
    md: 'w-4 h-4',
    lg: 'w-5 h-5',
  }

  // 날짜 파싱 (YYYY-MM-DD)
  const parseDate = () => {
    if (!props.value) {
      const now = new Date()
      return { year: now.getFullYear(), month: now.getMonth() + 1, day: now.getDate() }
    }
    const [year, month, day] = props.value.split('-').map(Number)
    return { year: year || new Date().getFullYear(), month: month || 1, day: day || 1 }
  }

  // 날짜 포맷팅 (Date 객체로 정규화하여 오버플로 자동 처리)
  const formatDate = (year: number, month: number, day: number): string => {
    const date = new Date(year, month - 1, day)
    const y = date.getFullYear()
    const m = String(date.getMonth() + 1).padStart(2, '0')
    const d = String(date.getDate()).padStart(2, '0')
    return `${y}-${m}-${d}`
  }

  // 날짜 변경 및 min/max 검증
  const updateDate = (year: number, month: number, day: number) => {
    const newValue = formatDate(year, month, day)
    if (props.min && newValue < props.min) return
    if (props.max && newValue > props.max) return
    props.onChange(newValue)
  }

  const handlePresetClick = (preset: DatePreset) => {
    const date = preset.getValue()
    props.onChange(date.toISOString().split('T')[0])
  }

  // 년도 입력 (5자리 이상 → 오버플로 처리: 20245 → 2024년 5월)
  const handleYearInput = (value: string) => {
    const numValue = parseInt(value) || 0
    const { month, day } = parseDate()

    if (value.length > 4) {
      // 5자리 이상: 앞 4자리는 년, 나머지는 월로
      const yearPart = parseInt(value.slice(0, 4))
      const monthPart = parseInt(value.slice(4))
      if (monthPart > 0) {
        updateDate(yearPart, monthPart, day)
      } else {
        updateDate(yearPart, month, day)
      }
    } else {
      updateDate(numValue, month, day)
    }
  }

  // 월 입력 (3자리 이상 → 오버플로 처리: 123 → 12월 3일)
  const handleMonthInput = (value: string) => {
    const numValue = parseInt(value) || 0
    const { year, day } = parseDate()

    if (value.length > 2) {
      // 3자리 이상: 앞 2자리는 월, 나머지는 일로
      const monthPart = parseInt(value.slice(0, 2))
      const dayPart = parseInt(value.slice(2))
      if (dayPart > 0) {
        updateDate(year, monthPart, dayPart)
      } else {
        updateDate(year, monthPart, day)
      }
    } else if (numValue > 12) {
      // 13 이상은 다음 해로 넘김
      const extraYears = Math.floor((numValue - 1) / 12)
      const newMonth = ((numValue - 1) % 12) + 1
      updateDate(year + extraYears, newMonth, day)
    } else {
      updateDate(year, numValue || 1, day)
    }
  }

  // 일 입력 (해당 월의 마지막 날을 넘으면 다음 달로)
  const handleDayInput = (value: string) => {
    const numValue = parseInt(value) || 1
    const { year, month } = parseDate()
    // Date 객체가 자동으로 오버플로 처리
    updateDate(year, month, numValue)
  }

  // 휠로 값 조절
  const handleYearWheel = (e: WheelEvent) => {
    if (props.disabled) return
    e.preventDefault()
    const { year, month, day } = parseDate()
    const delta = e.deltaY < 0 ? 1 : -1
    updateDate(year + delta, month, day)
  }

  const handleMonthWheel = (e: WheelEvent) => {
    if (props.disabled) return
    e.preventDefault()
    const { year, month, day } = parseDate()
    const delta = e.deltaY < 0 ? 1 : -1
    updateDate(year, month + delta, day)
  }

  const handleDayWheel = (e: WheelEvent) => {
    if (props.disabled) return
    e.preventDefault()
    const { year, month, day } = parseDate()
    const delta = e.deltaY < 0 ? 1 : -1
    updateDate(year, month, day + delta)
  }

  const presets = props.presets || DEFAULT_DATE_PRESETS

  const inputBaseClass = `
    text-center border border-gray-300 dark:border-gray-600 rounded
    bg-white dark:bg-gray-700 text-gray-900 dark:text-white
    focus:ring-2 focus:ring-blue-500 focus:border-blue-500
    disabled:opacity-50 disabled:cursor-not-allowed
  `

  return (
    <div class={`flex flex-col gap-1 ${props.className || ''}`}>
      <Show when={props.label}>
        <label class="text-xs font-medium text-gray-600 dark:text-gray-400">{props.label}</label>
      </Show>
      <div class="flex items-center gap-2">
        <div class="flex items-center gap-0.5">
          <svg
            class={`text-gray-400 mr-1 ${iconSizes[props.size || 'md']}`}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"
            />
          </svg>
          {/* 년 */}
          <input
            type="text"
            inputMode="numeric"
            value={parseDate().year}
            onInput={(e) => handleYearInput(e.currentTarget.value.replace(/\D/g, ''))}
            onWheel={handleYearWheel}
            disabled={props.disabled}
            class={`${inputBaseClass} ${sizeClasses[props.size || 'md']} ${yearWidth[props.size || 'md']}`}
            title="년 (휠로 조절 가능)"
          />
          <span class="text-gray-400 dark:text-gray-500">-</span>
          {/* 월 */}
          <input
            type="text"
            inputMode="numeric"
            value={String(parseDate().month).padStart(2, '0')}
            onInput={(e) => handleMonthInput(e.currentTarget.value.replace(/\D/g, ''))}
            onWheel={handleMonthWheel}
            disabled={props.disabled}
            class={`${inputBaseClass} ${sizeClasses[props.size || 'md']} ${monthDayWidth[props.size || 'md']}`}
            title="월 (휠로 조절 가능)"
          />
          <span class="text-gray-400 dark:text-gray-500">-</span>
          {/* 일 */}
          <input
            type="text"
            inputMode="numeric"
            value={String(parseDate().day).padStart(2, '0')}
            onInput={(e) => handleDayInput(e.currentTarget.value.replace(/\D/g, ''))}
            onWheel={handleDayWheel}
            disabled={props.disabled}
            class={`${inputBaseClass} ${sizeClasses[props.size || 'md']} ${monthDayWidth[props.size || 'md']}`}
            title="일 (휠로 조절 가능)"
          />
        </div>
        <Show when={props.showPresets && !props.disabled}>
          <div class="flex gap-1 flex-wrap">
            <For each={presets}>
              {(preset) => (
                <button
                  type="button"
                  onClick={() => handlePresetClick(preset)}
                  class={`
                    rounded bg-gray-100 dark:bg-gray-700
                    text-gray-600 dark:text-gray-300
                    hover:bg-gray-200 dark:hover:bg-gray-600
                    transition-colors whitespace-nowrap
                    ${presetSizeClasses[props.size || 'md']}
                  `}
                >
                  {preset.label}
                </button>
              )}
            </For>
          </div>
        </Show>
      </div>
    </div>
  )
}

// ==================== Date Range Picker ====================

export interface DateRangePreset {
  /** 프리셋 라벨 */
  label: string
  /** 시작일 계산 함수 */
  getStartDate: () => Date
  /** 종료일 계산 함수 */
  getEndDate: () => Date
}

/** 기본 날짜 범위 프리셋 */
export const DEFAULT_RANGE_PRESETS: DateRangePreset[] = [
  {
    label: '최근 1주',
    getStartDate: () => new Date(Date.now() - 7 * 24 * 60 * 60 * 1000),
    getEndDate: () => new Date()
  },
  {
    label: '최근 1개월',
    getStartDate: () => new Date(Date.now() - 30 * 24 * 60 * 60 * 1000),
    getEndDate: () => new Date()
  },
  {
    label: '최근 3개월',
    getStartDate: () => new Date(Date.now() - 90 * 24 * 60 * 60 * 1000),
    getEndDate: () => new Date()
  },
  {
    label: '최근 6개월',
    getStartDate: () => new Date(Date.now() - 180 * 24 * 60 * 60 * 1000),
    getEndDate: () => new Date()
  },
  {
    label: '최근 1년',
    getStartDate: () => new Date(Date.now() - 365 * 24 * 60 * 60 * 1000),
    getEndDate: () => new Date()
  },
]

export interface DateRangePickerProps {
  /** 시작일 (YYYY-MM-DD 형식) */
  startDate: string
  /** 종료일 (YYYY-MM-DD 형식) */
  endDate: string
  /** 시작일 변경 핸들러 */
  onStartDateChange: (value: string) => void
  /** 종료일 변경 핸들러 */
  onEndDateChange: (value: string) => void
  /** 라벨 (시작일/종료일 공통) */
  label?: string
  /** 시작일 라벨 */
  startLabel?: string
  /** 종료일 라벨 */
  endLabel?: string
  /** 비활성화 여부 */
  disabled?: boolean
  /** 크기 */
  size?: 'sm' | 'md' | 'lg'
  /** 최소 날짜 */
  minDate?: string
  /** 최대 날짜 */
  maxDate?: string
  /** 프리셋 버튼 표시 여부 */
  showPresets?: boolean
  /** 커스텀 프리셋 (기본: DEFAULT_RANGE_PRESETS) */
  presets?: DateRangePreset[]
  /** 세로 레이아웃 여부 */
  vertical?: boolean
  /** 추가 CSS 클래스 */
  className?: string
}

/**
 * 날짜 범위 선택 컴포넌트
 *
 * 시작일과 종료일을 함께 선택하는 컴포넌트입니다.
 * 프리셋 버튼으로 빠르게 기간을 선택할 수 있습니다.
 * 각 년/월/일 필드에서 휠로 값 조절 가능하고, 오버플로우 자동 처리됩니다.
 *
 * @example
 * ```tsx
 * <DateRangePicker
 *   startDate={form.startDate}
 *   endDate={form.endDate}
 *   onStartDateChange={(v) => setForm('startDate', v)}
 *   onEndDateChange={(v) => setForm('endDate', v)}
 *   showPresets
 * />
 * ```
 */
export const DateRangePicker: Component<DateRangePickerProps> = (props) => {
  const sizeClasses = {
    sm: 'px-1 py-1 text-xs',
    md: 'px-1.5 py-2 text-sm',
    lg: 'px-2 py-3 text-base',
  }

  const yearWidth = { sm: 'w-12', md: 'w-14', lg: 'w-16' }
  const monthDayWidth = { sm: 'w-8', md: 'w-10', lg: 'w-12' }

  const presetSizeClasses = {
    sm: 'px-1.5 py-0.5 text-xs',
    md: 'px-2 py-1 text-xs',
    lg: 'px-2.5 py-1.5 text-sm',
  }

  const iconSizes = {
    sm: 'w-3 h-3',
    md: 'w-4 h-4',
    lg: 'w-5 h-5',
  }

  // 날짜 파싱 (YYYY-MM-DD)
  const parseDate = (value: string) => {
    if (!value) {
      const now = new Date()
      return { year: now.getFullYear(), month: now.getMonth() + 1, day: now.getDate() }
    }
    const [year, month, day] = value.split('-').map(Number)
    return { year: year || new Date().getFullYear(), month: month || 1, day: day || 1 }
  }

  // 날짜 포맷팅 (Date 객체로 정규화하여 오버플로 자동 처리)
  const formatDate = (year: number, month: number, day: number): string => {
    const date = new Date(year, month - 1, day)
    const y = date.getFullYear()
    const m = String(date.getMonth() + 1).padStart(2, '0')
    const d = String(date.getDate()).padStart(2, '0')
    return `${y}-${m}-${d}`
  }

  // 시작일 변경 (min/max 및 종료일 검증)
  const updateStartDate = (year: number, month: number, day: number) => {
    const newValue = formatDate(year, month, day)
    if (props.minDate && newValue < props.minDate) return
    if (props.maxDate && newValue > props.maxDate) return
    // 시작일은 종료일을 넘을 수 없음
    if (props.endDate && newValue > props.endDate) return
    props.onStartDateChange(newValue)
  }

  // 종료일 변경 (min/max 및 시작일 검증)
  const updateEndDate = (year: number, month: number, day: number) => {
    const newValue = formatDate(year, month, day)
    if (props.minDate && newValue < props.minDate) return
    if (props.maxDate && newValue > props.maxDate) return
    // 종료일은 시작일보다 앞설 수 없음
    if (props.startDate && newValue < props.startDate) return
    props.onEndDateChange(newValue)
  }

  const handlePresetClick = (preset: DateRangePreset) => {
    const start = preset.getStartDate()
    const end = preset.getEndDate()
    props.onStartDateChange(start.toISOString().split('T')[0])
    props.onEndDateChange(end.toISOString().split('T')[0])
  }

  // ===== 시작일 핸들러 =====
  const handleStartYearInput = (value: string) => {
    const numValue = parseInt(value) || 0
    const { month, day } = parseDate(props.startDate)
    if (value.length > 4) {
      const yearPart = parseInt(value.slice(0, 4))
      const monthPart = parseInt(value.slice(4))
      if (monthPart > 0) {
        updateStartDate(yearPart, monthPart, day)
      } else {
        updateStartDate(yearPart, month, day)
      }
    } else {
      updateStartDate(numValue, month, day)
    }
  }

  const handleStartMonthInput = (value: string) => {
    const numValue = parseInt(value) || 0
    const { year, day } = parseDate(props.startDate)
    if (value.length > 2) {
      const monthPart = parseInt(value.slice(0, 2))
      const dayPart = parseInt(value.slice(2))
      if (dayPart > 0) {
        updateStartDate(year, monthPart, dayPart)
      } else {
        updateStartDate(year, monthPart, day)
      }
    } else if (numValue > 12) {
      const extraYears = Math.floor((numValue - 1) / 12)
      const newMonth = ((numValue - 1) % 12) + 1
      updateStartDate(year + extraYears, newMonth, day)
    } else {
      updateStartDate(year, numValue || 1, day)
    }
  }

  const handleStartDayInput = (value: string) => {
    const numValue = parseInt(value) || 1
    const { year, month } = parseDate(props.startDate)
    updateStartDate(year, month, numValue)
  }

  const handleStartYearWheel = (e: WheelEvent) => {
    if (props.disabled) return
    e.preventDefault()
    const { year, month, day } = parseDate(props.startDate)
    const delta = e.deltaY < 0 ? 1 : -1
    updateStartDate(year + delta, month, day)
  }

  const handleStartMonthWheel = (e: WheelEvent) => {
    if (props.disabled) return
    e.preventDefault()
    const { year, month, day } = parseDate(props.startDate)
    const delta = e.deltaY < 0 ? 1 : -1
    updateStartDate(year, month + delta, day)
  }

  const handleStartDayWheel = (e: WheelEvent) => {
    if (props.disabled) return
    e.preventDefault()
    const { year, month, day } = parseDate(props.startDate)
    const delta = e.deltaY < 0 ? 1 : -1
    updateStartDate(year, month, day + delta)
  }

  // ===== 종료일 핸들러 =====
  const handleEndYearInput = (value: string) => {
    const numValue = parseInt(value) || 0
    const { month, day } = parseDate(props.endDate)
    if (value.length > 4) {
      const yearPart = parseInt(value.slice(0, 4))
      const monthPart = parseInt(value.slice(4))
      if (monthPart > 0) {
        updateEndDate(yearPart, monthPart, day)
      } else {
        updateEndDate(yearPart, month, day)
      }
    } else {
      updateEndDate(numValue, month, day)
    }
  }

  const handleEndMonthInput = (value: string) => {
    const numValue = parseInt(value) || 0
    const { year, day } = parseDate(props.endDate)
    if (value.length > 2) {
      const monthPart = parseInt(value.slice(0, 2))
      const dayPart = parseInt(value.slice(2))
      if (dayPart > 0) {
        updateEndDate(year, monthPart, dayPart)
      } else {
        updateEndDate(year, monthPart, day)
      }
    } else if (numValue > 12) {
      const extraYears = Math.floor((numValue - 1) / 12)
      const newMonth = ((numValue - 1) % 12) + 1
      updateEndDate(year + extraYears, newMonth, day)
    } else {
      updateEndDate(year, numValue || 1, day)
    }
  }

  const handleEndDayInput = (value: string) => {
    const numValue = parseInt(value) || 1
    const { year, month } = parseDate(props.endDate)
    updateEndDate(year, month, numValue)
  }

  const handleEndYearWheel = (e: WheelEvent) => {
    if (props.disabled) return
    e.preventDefault()
    const { year, month, day } = parseDate(props.endDate)
    const delta = e.deltaY < 0 ? 1 : -1
    updateEndDate(year + delta, month, day)
  }

  const handleEndMonthWheel = (e: WheelEvent) => {
    if (props.disabled) return
    e.preventDefault()
    const { year, month, day } = parseDate(props.endDate)
    const delta = e.deltaY < 0 ? 1 : -1
    updateEndDate(year, month + delta, day)
  }

  const handleEndDayWheel = (e: WheelEvent) => {
    if (props.disabled) return
    e.preventDefault()
    const { year, month, day } = parseDate(props.endDate)
    const delta = e.deltaY < 0 ? 1 : -1
    updateEndDate(year, month, day + delta)
  }

  const presets = props.presets || DEFAULT_RANGE_PRESETS

  const inputBaseClass = `
    text-center border border-gray-300 dark:border-gray-600 rounded
    bg-white dark:bg-gray-700 text-gray-900 dark:text-white
    focus:ring-2 focus:ring-blue-500 focus:border-blue-500
    disabled:opacity-50 disabled:cursor-not-allowed
  `

  const CalendarIcon = () => (
    <svg
      class={`text-gray-400 mr-1 ${iconSizes[props.size || 'md']}`}
      fill="none"
      stroke="currentColor"
      viewBox="0 0 24 24"
    >
      <path
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-width="2"
        d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"
      />
    </svg>
  )

  const startParsed = () => parseDate(props.startDate)
  const endParsed = () => parseDate(props.endDate)

  return (
    <div class={`flex flex-col gap-2 ${props.className || ''}`}>
      <Show when={props.label}>
        <label class="text-xs font-medium text-gray-600 dark:text-gray-400">{props.label}</label>
      </Show>

      <div class={`flex ${props.vertical ? 'flex-col' : 'flex-wrap items-center'} gap-2`}>
        {/* 시작일 */}
        <div class="flex flex-col gap-1">
          <Show when={props.startLabel}>
            <label class="text-xs text-gray-500 dark:text-gray-400">{props.startLabel}</label>
          </Show>
          <div class="flex items-center gap-0.5">
            <CalendarIcon />
            {/* 시작일: 년 */}
            <input
              type="text"
              inputMode="numeric"
              value={startParsed().year}
              onInput={(e) => handleStartYearInput(e.currentTarget.value.replace(/\D/g, ''))}
              onWheel={handleStartYearWheel}
              disabled={props.disabled}
              class={`${inputBaseClass} ${sizeClasses[props.size || 'md']} ${yearWidth[props.size || 'md']}`}
              title="시작년 (휠로 조절 가능)"
            />
            <span class="text-gray-400 dark:text-gray-500">-</span>
            {/* 시작일: 월 */}
            <input
              type="text"
              inputMode="numeric"
              value={String(startParsed().month).padStart(2, '0')}
              onInput={(e) => handleStartMonthInput(e.currentTarget.value.replace(/\D/g, ''))}
              onWheel={handleStartMonthWheel}
              disabled={props.disabled}
              class={`${inputBaseClass} ${sizeClasses[props.size || 'md']} ${monthDayWidth[props.size || 'md']}`}
              title="시작월 (휠로 조절 가능)"
            />
            <span class="text-gray-400 dark:text-gray-500">-</span>
            {/* 시작일: 일 */}
            <input
              type="text"
              inputMode="numeric"
              value={String(startParsed().day).padStart(2, '0')}
              onInput={(e) => handleStartDayInput(e.currentTarget.value.replace(/\D/g, ''))}
              onWheel={handleStartDayWheel}
              disabled={props.disabled}
              class={`${inputBaseClass} ${sizeClasses[props.size || 'md']} ${monthDayWidth[props.size || 'md']}`}
              title="시작일 (휠로 조절 가능)"
            />
          </div>
        </div>

        {/* 구분자 */}
        <span class="text-gray-400 dark:text-gray-500 self-end pb-2">~</span>

        {/* 종료일 */}
        <div class="flex flex-col gap-1">
          <Show when={props.endLabel}>
            <label class="text-xs text-gray-500 dark:text-gray-400">{props.endLabel}</label>
          </Show>
          <div class="flex items-center gap-0.5">
            <CalendarIcon />
            {/* 종료일: 년 */}
            <input
              type="text"
              inputMode="numeric"
              value={endParsed().year}
              onInput={(e) => handleEndYearInput(e.currentTarget.value.replace(/\D/g, ''))}
              onWheel={handleEndYearWheel}
              disabled={props.disabled}
              class={`${inputBaseClass} ${sizeClasses[props.size || 'md']} ${yearWidth[props.size || 'md']}`}
              title="종료년 (휠로 조절 가능)"
            />
            <span class="text-gray-400 dark:text-gray-500">-</span>
            {/* 종료일: 월 */}
            <input
              type="text"
              inputMode="numeric"
              value={String(endParsed().month).padStart(2, '0')}
              onInput={(e) => handleEndMonthInput(e.currentTarget.value.replace(/\D/g, ''))}
              onWheel={handleEndMonthWheel}
              disabled={props.disabled}
              class={`${inputBaseClass} ${sizeClasses[props.size || 'md']} ${monthDayWidth[props.size || 'md']}`}
              title="종료월 (휠로 조절 가능)"
            />
            <span class="text-gray-400 dark:text-gray-500">-</span>
            {/* 종료일: 일 */}
            <input
              type="text"
              inputMode="numeric"
              value={String(endParsed().day).padStart(2, '0')}
              onInput={(e) => handleEndDayInput(e.currentTarget.value.replace(/\D/g, ''))}
              onWheel={handleEndDayWheel}
              disabled={props.disabled}
              class={`${inputBaseClass} ${sizeClasses[props.size || 'md']} ${monthDayWidth[props.size || 'md']}`}
              title="종료일 (휠로 조절 가능)"
            />
          </div>
        </div>

        {/* 프리셋 버튼 */}
        <Show when={props.showPresets && !props.disabled}>
          <div class={`flex gap-1 flex-wrap ${props.vertical ? '' : 'ml-2'}`}>
            <For each={presets}>
              {(preset) => (
                <button
                  type="button"
                  onClick={() => handlePresetClick(preset)}
                  class={`
                    rounded bg-gray-100 dark:bg-gray-700
                    text-gray-600 dark:text-gray-300
                    hover:bg-blue-100 dark:hover:bg-blue-900
                    hover:text-blue-600 dark:hover:text-blue-400
                    transition-colors whitespace-nowrap
                    ${presetSizeClasses[props.size || 'md']}
                  `}
                >
                  {preset.label}
                </button>
              )}
            </For>
          </div>
        </Show>
      </div>
    </div>
  )
}

export default Select
