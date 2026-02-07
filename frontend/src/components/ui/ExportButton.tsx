/**
 * 데이터 내보내기 버튼 컴포넌트
 *
 * CSV/Excel 형식으로 데이터를 내보내는 기능을 제공합니다.
 * UTF-8 BOM을 사용하여 한글 인코딩을 지원합니다.
 */
import { type Component, type JSX, Show } from 'solid-js'

export interface ExportColumn<T> {
  /** 컬럼 헤더 */
  header: string
  /** 데이터 접근자 (함수일 경우 row와 index를 받음) */
  accessor: keyof T | ((row: T, index?: number) => string | number | undefined)
}

export interface ExportButtonProps<T> {
  /** 내보낼 데이터 배열 */
  data: T[]
  /** 컬럼 정의 */
  columns: ExportColumn<T>[]
  /** 파일명 (확장자 제외) */
  filename?: string
  /** 버튼 텍스트 */
  label?: string
  /** 버튼 아이콘 */
  icon?: JSX.Element
  /** 버튼 스타일 */
  variant?: 'primary' | 'secondary' | 'ghost'
  /** 크기 */
  size?: 'sm' | 'md'
  /** 비활성화 */
  disabled?: boolean
}

/**
 * 값을 CSV 셀 형식으로 변환
 */
function formatCsvValue(value: unknown): string {
  if (value === null || value === undefined) {
    return ''
  }

  const str = String(value)

  // 쉼표, 따옴표, 줄바꿈이 포함된 경우 따옴표로 감싸기
  if (str.includes(',') || str.includes('"') || str.includes('\n') || str.includes('\r')) {
    return `"${str.replace(/"/g, '""')}"`
  }

  return str
}

/**
 * 데이터를 CSV 문자열로 변환
 */
export function dataToCSV<T>(data: T[], columns: ExportColumn<T>[]): string {
  // 헤더 행
  const headers = columns.map(col => formatCsvValue(col.header)).join(',')

  // 데이터 행
  const rows = data.map((row, index) => {
    return columns.map(col => {
      let value: unknown
      if (typeof col.accessor === 'function') {
        value = col.accessor(row, index)
      } else {
        value = row[col.accessor]
      }
      return formatCsvValue(value)
    }).join(',')
  })

  return [headers, ...rows].join('\r\n')
}

/**
 * CSV 다운로드 실행
 */
export function downloadCSV(csvContent: string, filename: string): void {
  // UTF-8 BOM 추가 (Excel에서 한글 인코딩 문제 방지)
  const BOM = '\uFEFF'
  const blob = new Blob([BOM + csvContent], { type: 'text/csv;charset=utf-8;' })

  // 다운로드 링크 생성
  const link = document.createElement('a')
  const url = URL.createObjectURL(blob)
  link.href = url
  link.download = `${filename}.csv`

  // 클릭 시뮬레이션 및 정리
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
  URL.revokeObjectURL(url)
}

/**
 * 데이터 내보내기 버튼 컴포넌트
 */
export function ExportButton<T>(props: ExportButtonProps<T>): JSX.Element {
  const handleExport = () => {
    if (props.data.length === 0) return

    const csvContent = dataToCSV(props.data, props.columns)
    const filename = props.filename || `export_${new Date().toISOString().slice(0, 10)}`
    downloadCSV(csvContent, filename)
  }

  const variant = () => props.variant || 'secondary'
  const size = () => props.size || 'md'

  const baseClasses = `
    inline-flex items-center gap-1.5 font-medium rounded-lg
    transition-colors duration-150
    focus:outline-none focus:ring-2 focus:ring-offset-1
    disabled:opacity-50 disabled:cursor-not-allowed
  `

  const variantClasses = {
    primary: 'bg-blue-600 text-white hover:bg-blue-700 focus:ring-blue-500',
    secondary: 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-600 focus:ring-gray-500',
    ghost: 'text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 focus:ring-gray-500',
  }

  const sizeClasses = {
    sm: 'px-2.5 py-1.5 text-xs',
    md: 'px-3 py-2 text-sm',
  }

  return (
    <button
      type="button"
      onClick={handleExport}
      disabled={props.disabled || props.data.length === 0}
      class={`${baseClasses} ${variantClasses[variant()]} ${sizeClasses[size()]}`}
      title={props.data.length === 0 ? '내보낼 데이터가 없습니다' : 'CSV로 내보내기'}
    >
      <Show
        when={props.icon}
        fallback={
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 10v6m0 0l-3-3m3 3l3-3m2 8H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
        }
      >
        {props.icon}
      </Show>
      <span>{props.label || 'Excel 내보내기'}</span>
    </button>
  )
}

export default ExportButton
