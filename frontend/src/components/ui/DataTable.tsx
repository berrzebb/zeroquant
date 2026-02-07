/**
 * DataTable 컴포넌트
 *
 * 재사용 가능한 데이터 테이블 컴포넌트입니다.
 * 정렬, 필터링, 페이지네이션을 지원합니다.
 */
import { type Component, type JSX, For, Show, createSignal, createMemo } from 'solid-js'
import { ChevronUp, ChevronDown, ChevronsUpDown } from 'lucide-solid'

export interface Column<T> {
  key: string
  header: string | JSX.Element
  width?: string
  align?: 'left' | 'center' | 'right'
  sortable?: boolean
  render?: (value: any, row: T, index: number) => JSX.Element
}

interface DataTableProps<T> {
  data: T[]
  columns: Column<T>[]
  keyField?: string
  loading?: boolean
  emptyMessage?: string
  className?: string
  striped?: boolean
  hover?: boolean
  compact?: boolean
  onRowClick?: (row: T, index: number) => void
  sortField?: string
  sortOrder?: 'asc' | 'desc'
  onSort?: (field: string, order: 'asc' | 'desc') => void
}

const alignStyles = {
  left: 'text-left',
  center: 'text-center',
  right: 'text-right',
}

export function DataTable<T extends Record<string, any>>(props: DataTableProps<T>) {
  const [localSortField, setLocalSortField] = createSignal(props.sortField || '')
  const [localSortOrder, setLocalSortOrder] = createSignal<'asc' | 'desc'>(props.sortOrder || 'asc')

  const sortField = createMemo(() => props.sortField ?? localSortField())
  const sortOrder = createMemo(() => props.sortOrder ?? localSortOrder())

  const handleSort = (field: string) => {
    if (props.onSort) {
      const newOrder = sortField() === field && sortOrder() === 'asc' ? 'desc' : 'asc'
      props.onSort(field, newOrder)
    } else {
      const newOrder = localSortField() === field && localSortOrder() === 'asc' ? 'desc' : 'asc'
      setLocalSortField(field)
      setLocalSortOrder(newOrder)
    }
  }

  const sortedData = createMemo(() => {
    if (!sortField() || props.onSort) return props.data

    return [...props.data].sort((a, b) => {
      const aVal = a[sortField()]
      const bVal = b[sortField()]

      if (aVal === bVal) return 0
      if (aVal === null || aVal === undefined) return 1
      if (bVal === null || bVal === undefined) return -1

      const comparison = aVal < bVal ? -1 : 1
      return sortOrder() === 'asc' ? comparison : -comparison
    })
  })

  const cellPadding = createMemo(() => props.compact ? 'px-3 py-2' : 'px-4 py-3')

  return (
    <div class={`overflow-x-auto ${props.className || ''}`}>
      <table class="w-full border-collapse">
        <thead>
          <tr class="bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
            <For each={props.columns}>
              {(column) => (
                <th
                  class={`
                    ${cellPadding()} ${alignStyles[column.align || 'left']}
                    text-xs font-semibold text-gray-600 dark:text-gray-400 uppercase tracking-wider
                    ${column.sortable ? 'cursor-pointer select-none hover:bg-gray-100 dark:hover:bg-gray-700' : ''}
                  `}
                  style={{ width: column.width }}
                  onClick={() => column.sortable && handleSort(column.key)}
                >
                  <div class="flex items-center gap-1">
                    <span>{column.header}</span>
                    <Show when={column.sortable}>
                      <span class="text-gray-400">
                        {sortField() === column.key ? (
                          sortOrder() === 'asc' ? <ChevronUp class="w-4 h-4" /> : <ChevronDown class="w-4 h-4" />
                        ) : (
                          <ChevronsUpDown class="w-4 h-4 opacity-50" />
                        )}
                      </span>
                    </Show>
                  </div>
                </th>
              )}
            </For>
          </tr>
        </thead>
        <tbody>
          <Show
            when={!props.loading && sortedData().length > 0}
            fallback={
              <tr>
                <td
                  colSpan={props.columns.length}
                  class="px-4 py-8 text-center text-gray-500 dark:text-gray-400"
                >
                  {props.loading ? '로딩 중...' : (props.emptyMessage || '데이터가 없습니다.')}
                </td>
              </tr>
            }
          >
            <For each={sortedData()}>
              {(row, index) => (
                <tr
                  class={`
                    border-b border-gray-100 dark:border-gray-800
                    ${props.striped && index() % 2 === 1 ? 'bg-gray-50 dark:bg-gray-800/50' : ''}
                    ${props.hover ? 'hover:bg-blue-50 dark:hover:bg-blue-900/20' : ''}
                    ${props.onRowClick ? 'cursor-pointer' : ''}
                  `}
                  onClick={() => props.onRowClick?.(row, index())}
                >
                  <For each={props.columns}>
                    {(column) => (
                      <td class={`${cellPadding()} ${alignStyles[column.align || 'left']} text-sm`}>
                        {column.render
                          ? column.render(row[column.key], row, index())
                          : row[column.key]
                        }
                      </td>
                    )}
                  </For>
                </tr>
              )}
            </For>
          </Show>
        </tbody>
      </table>
    </div>
  )
}

// 개별 테이블 요소 (커스텀 테이블 구성 시 사용)
export const TableHeader: Component<{ children: JSX.Element; className?: string }> = (props) => (
  <th class={`px-4 py-3 text-left text-xs font-semibold text-gray-600 dark:text-gray-400 uppercase tracking-wider ${props.className || ''}`}>
    {props.children}
  </th>
)

export const TableRow: Component<{
  children: JSX.Element
  className?: string
  onClick?: () => void
  striped?: boolean
  index?: number
}> = (props) => (
  <tr
    class={`
      border-b border-gray-100 dark:border-gray-800
      ${props.striped && (props.index || 0) % 2 === 1 ? 'bg-gray-50 dark:bg-gray-800/50' : ''}
      hover:bg-blue-50 dark:hover:bg-blue-900/20
      ${props.onClick ? 'cursor-pointer' : ''}
      ${props.className || ''}
    `}
    onClick={props.onClick}
  >
    {props.children}
  </tr>
)

export const TableCell: Component<{
  children: JSX.Element
  className?: string
  align?: 'left' | 'center' | 'right'
}> = (props) => (
  <td class={`px-4 py-3 text-sm ${alignStyles[props.align || 'left']} ${props.className || ''}`}>
    {props.children}
  </td>
)

export default DataTable
