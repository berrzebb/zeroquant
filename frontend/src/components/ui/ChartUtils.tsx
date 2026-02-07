/**
 * 차트 유틸리티 컴포넌트
 *
 * 툴팁, 범례 등 차트에서 공통으로 사용되는 컴포넌트입니다.
 */
import { type Component, type JSX, For, Show, createMemo } from 'solid-js'
// 중앙화된 포맷 함수 재export (성능 최적화된 버전)
export {
  formatNumber,
  formatPercent,
  formatCurrency as formatCurrencyStd,
  formatCurrencyCompact,
  getPnLColor,
  getPnLBgColor,
} from '../../utils/format'
import { formatCurrencyCompact } from '../../utils/format'

interface ChartTooltipProps {
  title?: string
  items: Array<{
    label: string
    value: string | number
    color?: string
    suffix?: string
  }>
  className?: string
}

export const ChartTooltip: Component<ChartTooltipProps> = (props) => {
  return (
    <div class={`
      bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700
      rounded-lg shadow-lg p-3 text-sm
      ${props.className || ''}
    `}>
      <Show when={props.title}>
        <div class="font-medium text-gray-900 dark:text-white mb-2 pb-2 border-b border-gray-200 dark:border-gray-700">
          {props.title}
        </div>
      </Show>
      <div class="space-y-1">
        <For each={props.items}>
          {(item) => (
            <div class="flex items-center justify-between gap-4">
              <div class="flex items-center gap-2">
                <Show when={item.color}>
                  <span
                    class="w-3 h-3 rounded-full"
                    style={{ background: item.color }}
                  />
                </Show>
                <span class="text-gray-600 dark:text-gray-400">{item.label}</span>
              </div>
              <span class="font-medium text-gray-900 dark:text-white">
                {item.value}{item.suffix || ''}
              </span>
            </div>
          )}
        </For>
      </div>
    </div>
  )
}

interface ChartLegendProps {
  items: Array<{
    label: string
    color: string
    value?: string | number
    active?: boolean
  }>
  layout?: 'horizontal' | 'vertical'
  onItemClick?: (index: number) => void
  className?: string
}

export const ChartLegend: Component<ChartLegendProps> = (props) => {
  const isHorizontal = createMemo(() => props.layout !== 'vertical')

  return (
    <div class={`
      flex ${isHorizontal() ? 'flex-row flex-wrap gap-4' : 'flex-col gap-2'}
      ${props.className || ''}
    `}>
      <For each={props.items}>
        {(item, index) => (
          <div
            class={`
              flex items-center gap-2 text-sm
              ${props.onItemClick ? 'cursor-pointer hover:opacity-80' : ''}
              ${item.active === false ? 'opacity-50' : ''}
            `}
            onClick={() => props.onItemClick?.(index())}
          >
            <span
              class="w-3 h-3 rounded-sm"
              style={{ background: item.color }}
            />
            <span class="text-gray-700 dark:text-gray-300">{item.label}</span>
            <Show when={item.value !== undefined}>
              <span class="font-medium text-gray-900 dark:text-white">
                {item.value}
              </span>
            </Show>
          </div>
        )}
      </For>
    </div>
  )
}

/**
 * 차트용 통화 포맷팅 (한국식 컴팩트: 억원, 만원)
 * @deprecated formatCurrencyCompact 사용 권장
 */
export const formatCurrency = formatCurrencyCompact

/**
 * 차트 색상 팔레트
 */
export const chartColors = {
  primary: ['#3B82F6', '#10B981', '#F59E0B', '#EF4444', '#8B5CF6', '#EC4899'],
  pastel: ['#93C5FD', '#6EE7B7', '#FCD34D', '#FCA5A5', '#C4B5FD', '#F9A8D4'],
  gradient: {
    blue: ['#3B82F6', '#1D4ED8'],
    green: ['#10B981', '#047857'],
    red: ['#EF4444', '#B91C1C'],
  },
}

export default ChartTooltip
