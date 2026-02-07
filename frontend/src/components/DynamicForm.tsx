/**
 * Server Driven UI (SDUI) 기반 동적 폼 렌더러
 *
 * 서버에서 전달받은 UI 스키마를 기반으로 폼을 동적으로 렌더링합니다.
 * 새로운 전략이 추가되어도 프론트엔드 코드 수정 없이 폼이 자동 생성됩니다.
 */
import { createSignal, For, Show, createMemo } from 'solid-js'
import { ChevronDown, ChevronRight, HelpCircle, Plus, X } from 'lucide-solid'
import type { UiSchema, UiField, UiFieldGroup, UiValidation, UiSelectOption, SymbolCategory } from '../api/client'
import { SymbolSearch } from './SymbolSearch'

interface DynamicFormProps {
  schema: UiSchema
  values: Record<string, unknown>
  onChange: (key: string, value: unknown) => void
  errors?: Record<string, string>
  disabled?: boolean
}

export function DynamicForm(props: DynamicFormProps) {
  // 그룹별 접힘 상태
  const [collapsedGroups, setCollapsedGroups] = createSignal<Set<string>>(
    new Set(props.schema.groups.filter(g => g.collapsed).map(g => g.id))
  )

  // 그룹별 필드 매핑
  const fieldsByGroup = createMemo(() => {
    const map = new Map<string, UiField[]>()
    const ungrouped: UiField[] = []

    // 정렬된 필드
    const sortedFields = [...props.schema.fields].sort((a, b) => a.order - b.order)

    for (const field of sortedFields) {
      if (field.group) {
        if (!map.has(field.group)) {
          map.set(field.group, [])
        }
        map.get(field.group)!.push(field)
      } else {
        ungrouped.push(field)
      }
    }

    return { map, ungrouped }
  })

  // 정렬된 그룹
  const sortedGroups = createMemo(() =>
    [...props.schema.groups].sort((a, b) => a.order - b.order)
  )

  // 그룹 토글
  const toggleGroup = (groupId: string) => {
    setCollapsedGroups(prev => {
      const next = new Set(prev)
      if (next.has(groupId)) {
        next.delete(groupId)
      } else {
        next.add(groupId)
      }
      return next
    })
  }

  // 조건부 표시 체크
  const shouldShowField = (field: UiField): boolean => {
    if (!field.show_when) return true

    const { field: refField, operator, value } = field.show_when
    const currentValue = props.values[refField]

    switch (operator) {
      case 'equals':
        return currentValue === value
      case 'not_equals':
        return currentValue !== value
      case 'greater_than':
        return (currentValue as number) > (value as number)
      case 'less_than':
        return (currentValue as number) < (value as number)
      case 'contains':
        if (Array.isArray(currentValue)) {
          return currentValue.includes(value)
        }
        return String(currentValue).includes(String(value))
      default:
        return true
    }
  }

  // 레이아웃 클래스
  const layoutClass = createMemo(() => {
    const cols = props.schema.layout?.columns || 1
    if (cols === 1) return ''
    if (cols === 2) return 'grid grid-cols-1 md:grid-cols-2 gap-4'
    return `grid grid-cols-1 md:grid-cols-${cols} gap-4`
  })

  return (
    <div class="space-y-6">
      {/* 그룹화되지 않은 필드 */}
      <Show when={fieldsByGroup().ungrouped.length > 0}>
        <div class={layoutClass()}>
          <For each={fieldsByGroup().ungrouped}>
            {(field) => (
              <Show when={shouldShowField(field)}>
                <FieldRenderer
                  field={field}
                  value={props.values[field.key]}
                  onChange={(v) => props.onChange(field.key, v)}
                  error={props.errors?.[field.key]}
                  disabled={props.disabled}
                />
              </Show>
            )}
          </For>
        </div>
      </Show>

      {/* 그룹화된 필드 */}
      <For each={sortedGroups()}>
        {(group) => {
          const fields = () => fieldsByGroup().map.get(group.id) || []
          const isCollapsed = () => collapsedGroups().has(group.id)

          return (
            <Show when={fields().length > 0}>
              <div class="border border-[var(--color-surface-light)] rounded-lg overflow-hidden">
                {/* 그룹 헤더 */}
                <button
                  type="button"
                  class="w-full flex items-center justify-between p-4 bg-[var(--color-surface)] hover:bg-[var(--color-surface-light)] transition-colors"
                  onClick={() => toggleGroup(group.id)}
                >
                  <div class="flex items-center gap-2">
                    <Show when={isCollapsed()} fallback={<ChevronDown class="w-4 h-4" />}>
                      <ChevronRight class="w-4 h-4" />
                    </Show>
                    <span class="font-medium text-[var(--color-text)]">{group.label}</span>
                    <Show when={group.description}>
                      <span class="text-sm text-[var(--color-text-muted)]">
                        ({group.description})
                      </span>
                    </Show>
                  </div>
                  <span class="text-sm text-[var(--color-text-muted)]">
                    {fields().length}개 설정
                  </span>
                </button>

                {/* 그룹 내용 */}
                <Show when={!isCollapsed()}>
                  <div class={`p-4 space-y-4 bg-[var(--color-bg)] ${layoutClass()}`}>
                    <For each={fields()}>
                      {(field) => (
                        <Show when={shouldShowField(field)}>
                          <FieldRenderer
                            field={field}
                            value={props.values[field.key]}
                            onChange={(v) => props.onChange(field.key, v)}
                            error={props.errors?.[field.key]}
                            disabled={props.disabled}
                          />
                        </Show>
                      )}
                    </For>
                  </div>
                </Show>
              </div>
            </Show>
          )
        }}
      </For>
    </div>
  )
}

// ==================== 필드 렌더러 ====================

interface FieldRendererProps {
  field: UiField
  value: unknown
  onChange: (value: unknown) => void
  error?: string
  disabled?: boolean
}

function FieldRenderer(props: FieldRendererProps) {
  const getValue = () => props.value ?? props.field.default_value

  return (
    <div class="space-y-1">
      {/* 레이블 */}
      <label class="flex items-center gap-2 text-sm font-medium text-[var(--color-text)]">
        {props.field.label}
        <Show when={props.field.validation.required}>
          <span class="text-red-500">*</span>
        </Show>
        <Show when={props.field.help_text}>
          <span class="group relative">
            <HelpCircle class="w-4 h-4 text-[var(--color-text-muted)] cursor-help" />
            <span class="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-2 text-xs bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg shadow-lg opacity-0 group-hover:opacity-100 transition-opacity whitespace-nowrap z-10">
              {props.field.help_text}
            </span>
          </span>
        </Show>
        <Show when={props.field.unit}>
          <span class="text-[var(--color-text-muted)] font-normal">({props.field.unit})</span>
        </Show>
      </label>

      {/* 입력 필드 (타입별 렌더링) */}
      <div>
        {renderFieldByType(props.field, getValue(), props.onChange, props.disabled)}
      </div>

      {/* 에러 메시지 */}
      <Show when={props.error}>
        <p class="text-xs text-red-500">{props.error}</p>
      </Show>
    </div>
  )
}

// 타입별 필드 렌더링
function renderFieldByType(
  field: UiField,
  value: unknown,
  onChange: (value: unknown) => void,
  disabled?: boolean
) {
  switch (field.field_type) {
    case 'number':
      return (
        <NumberField
          value={value as number}
          onChange={onChange}
          validation={field.validation}
          placeholder={field.placeholder}
          disabled={disabled}
        />
      )

    case 'text':
      return (
        <TextField
          value={value as string}
          onChange={onChange}
          validation={field.validation}
          placeholder={field.placeholder}
          disabled={disabled}
        />
      )

    case 'select':
      return (
        <SelectField
          value={value}
          onChange={onChange}
          options={field.options || []}
          disabled={disabled}
        />
      )

    case 'boolean':
      return (
        <BooleanField
          value={value as boolean}
          onChange={onChange}
          disabled={disabled}
        />
      )

    case 'range':
      return (
        <RangeField
          value={value as number}
          onChange={onChange}
          validation={field.validation}
          disabled={disabled}
        />
      )

    case 'symbol_picker':
      return (
        <SymbolPickerField
          value={value as string[]}
          onChange={onChange}
          validation={field.validation}
          disabled={disabled}
        />
      )

    case 'symbol_category_group':
      return (
        <SymbolCategoryGroupField
          value={value as Record<string, string[]>}
          onChange={onChange}
          categories={field.symbol_categories || []}
          disabled={disabled}
        />
      )

    default:
      return (
        <TextField
          value={String(value ?? '')}
          onChange={onChange}
          validation={field.validation}
          placeholder={field.placeholder}
          disabled={disabled}
        />
      )
  }
}

// ==================== 개별 필드 컴포넌트 ====================

interface NumberFieldProps {
  value: number
  onChange: (value: number) => void
  validation: UiValidation
  placeholder?: string
  disabled?: boolean
}

function NumberField(props: NumberFieldProps) {
  return (
    <input
      type="number"
      value={props.value ?? ''}
      onInput={(e) => props.onChange(parseFloat(e.currentTarget.value) || 0)}
      min={props.validation.min}
      max={props.validation.max}
      step={props.validation.step}
      placeholder={props.placeholder}
      disabled={props.disabled}
      class="w-full px-3 py-2 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-[var(--color-text)] placeholder:text-[var(--color-text-muted)] focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)] disabled:opacity-50"
    />
  )
}

interface TextFieldProps {
  value: string
  onChange: (value: string) => void
  validation: UiValidation
  placeholder?: string
  disabled?: boolean
}

function TextField(props: TextFieldProps) {
  return (
    <input
      type="text"
      value={props.value ?? ''}
      onInput={(e) => props.onChange(e.currentTarget.value)}
      minLength={props.validation.min_length}
      maxLength={props.validation.max_length}
      pattern={props.validation.pattern}
      placeholder={props.placeholder}
      disabled={props.disabled}
      class="w-full px-3 py-2 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-[var(--color-text)] placeholder:text-[var(--color-text-muted)] focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)] disabled:opacity-50"
    />
  )
}

interface SelectFieldProps {
  value: unknown
  onChange: (value: unknown) => void
  options: UiSelectOption[]
  disabled?: boolean
}

function SelectField(props: SelectFieldProps) {
  return (
    <select
      value={JSON.stringify(props.value ?? '')}
      onChange={(e) => {
        try {
          props.onChange(JSON.parse(e.currentTarget.value))
        } catch {
          props.onChange(e.currentTarget.value)
        }
      }}
      disabled={props.disabled}
      class="w-full px-3 py-2 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-[var(--color-text)] focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)] disabled:opacity-50"
    >
      <For each={props.options}>
        {(option) => (
          <option value={JSON.stringify(option.value)}>{option.label}</option>
        )}
      </For>
    </select>
  )
}

interface BooleanFieldProps {
  value: boolean
  onChange: (value: boolean) => void
  disabled?: boolean
}

function BooleanField(props: BooleanFieldProps) {
  return (
    <label class="flex items-center gap-2 cursor-pointer">
      <input
        type="checkbox"
        checked={props.value ?? false}
        onChange={(e) => props.onChange(e.currentTarget.checked)}
        disabled={props.disabled}
        class="w-5 h-5 rounded border-[var(--color-surface-light)] text-[var(--color-primary)] focus:ring-[var(--color-primary)]"
      />
      <span class="text-sm text-[var(--color-text-muted)]">
        {props.value ? '활성화' : '비활성화'}
      </span>
    </label>
  )
}

interface RangeFieldProps {
  value: number
  onChange: (value: number) => void
  validation: UiValidation
  disabled?: boolean
}

function RangeField(props: RangeFieldProps) {
  return (
    <div class="flex items-center gap-4">
      <input
        type="range"
        value={props.value ?? props.validation.min ?? 0}
        onInput={(e) => props.onChange(parseFloat(e.currentTarget.value))}
        min={props.validation.min}
        max={props.validation.max}
        step={props.validation.step}
        disabled={props.disabled}
        class="flex-1 h-2 bg-[var(--color-surface-light)] rounded-lg appearance-none cursor-pointer"
      />
      <span class="min-w-[4rem] text-center font-medium text-[var(--color-text)]">
        {props.value ?? props.validation.min ?? 0}
      </span>
    </div>
  )
}

interface SymbolPickerFieldProps {
  value: string[]
  onChange: (value: string[]) => void
  validation: UiValidation
  disabled?: boolean
}

/**
 * 심볼 선택 필드 (복수 선택, API 검색 지원)
 *
 * Dataset.tsx와 동일한 심볼 검색 API를 사용하여
 * 티커 및 회사명 검색을 지원합니다.
 */
function SymbolPickerField(props: SymbolPickerFieldProps) {
  return (
    <SymbolSearch
      value={props.value || []}
      onChange={props.onChange}
      multiple={true}
      disabled={props.disabled}
      minItems={props.validation.min_items}
      maxItems={props.validation.max_items}
      placeholder="심볼/회사명 검색 (예: SPY, 삼성전자)"
    />
  )
}

// ==================== 심볼 카테고리 그룹 ====================

interface SymbolCategoryGroupFieldProps {
  value: Record<string, string[]>
  onChange: (value: Record<string, string[]>) => void
  categories: SymbolCategory[]
  disabled?: boolean
}

/**
 * 여러 카테고리로 구성된 심볼 그룹 선택기
 * HAA, XAA 등 자산배분 전략에서 카나리아/공격/방어 자산 등을 분리하여 선택
 */
function SymbolCategoryGroupField(props: SymbolCategoryGroupFieldProps) {
  // 카테고리 정렬
  const sortedCategories = () =>
    [...props.categories].sort((a, b) => a.order - b.order)

  // 특정 카테고리의 현재 값 가져오기
  const getCategoryValue = (key: string, defaultSymbols: string[]) => {
    const current = props.value?.[key]
    if (current && current.length > 0) return current
    return defaultSymbols
  }

  // 카테고리 값 업데이트
  const updateCategory = (categoryKey: string, symbols: string[]) => {
    props.onChange({
      ...props.value,
      [categoryKey]: symbols
    })
  }

  return (
    <div class="space-y-4">
      <For each={sortedCategories()}>
        {(category) => (
          <SymbolCategoryItem
            category={category}
            value={getCategoryValue(category.key, category.default_symbols)}
            onChange={(symbols) => updateCategory(category.key, symbols)}
            disabled={props.disabled}
          />
        )}
      </For>
    </div>
  )
}

interface SymbolCategoryItemProps {
  category: SymbolCategory
  value: string[]
  onChange: (value: string[]) => void
  disabled?: boolean
}

/**
 * 단일 카테고리 심볼 선택기
 *
 * HAA, XAA 등 자산배분 전략에서 사용되며,
 * 추천 심볼 목록과 API 검색을 모두 지원합니다.
 */
function SymbolCategoryItem(props: SymbolCategoryItemProps) {
  const [showSuggestions, setShowSuggestions] = createSignal(false)
  const values = () => props.value || []

  // 심볼 추가
  const addSymbol = (symbol: string) => {
    const sym = symbol.trim().toUpperCase()
    if (sym && !values().includes(sym)) {
      const maxItems = props.category.max_items
      if (!maxItems || values().length < maxItems) {
        props.onChange([...values(), sym])
      }
    }
  }

  // 추가되지 않은 제안 심볼
  const availableSuggestions = () =>
    props.category.suggested_symbols.filter(s => !values().includes(s))

  return (
    <div class="border border-[var(--color-surface-light)] rounded-lg overflow-hidden">
      {/* 카테고리 헤더 */}
      <div class="px-4 py-2 bg-[var(--color-surface)]">
        <div class="flex items-center justify-between">
          <span class="font-medium text-[var(--color-text)]">
            {props.category.label}
          </span>
          <span class="text-xs text-[var(--color-text-muted)]">
            {values().length}개 선택
            {props.category.min_items && ` (최소 ${props.category.min_items}개)`}
            {props.category.max_items && ` (최대 ${props.category.max_items}개)`}
          </span>
        </div>
        <Show when={props.category.description}>
          <p class="text-xs text-[var(--color-text-muted)] mt-1">
            {props.category.description}
          </p>
        </Show>
      </div>

      {/* 심볼 검색 및 선택 */}
      <div
        class="p-3 space-y-2"
        onFocusIn={() => setShowSuggestions(true)}
        onFocusOut={() => setTimeout(() => setShowSuggestions(false), 200)}
      >
        <SymbolSearch
          value={values()}
          onChange={props.onChange}
          multiple={true}
          disabled={props.disabled}
          minItems={props.category.min_items}
          maxItems={props.category.max_items}
          placeholder="심볼/회사명 검색"
          size="sm"
        />

        {/* 추천 심볼 */}
        <Show when={availableSuggestions().length > 0 && showSuggestions()}>
          <div class="pt-2 border-t border-[var(--color-surface-light)]">
            <p class="text-xs text-[var(--color-text-muted)] mb-1.5">추천 심볼:</p>
            <div class="flex flex-wrap gap-1.5">
              <For each={availableSuggestions()}>
                {(symbol) => (
                  <button
                    type="button"
                    onClick={() => addSymbol(symbol)}
                    disabled={props.disabled}
                    class="px-2 py-0.5 text-xs bg-[var(--color-surface)] text-[var(--color-text-muted)] rounded hover:bg-[var(--color-surface-light)] hover:text-[var(--color-text)] transition-colors"
                  >
                    + {symbol}
                  </button>
                )}
              </For>
            </div>
          </div>
        </Show>
      </div>
    </div>
  )
}
