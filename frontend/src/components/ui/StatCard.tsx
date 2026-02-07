/**
 * 통계 카드 컴포넌트
 *
 * 대시보드, 랭킹 페이지 등에서 주요 통계를 표시하는 데 사용합니다.
 * 숫자 값과 라벨, 선택적으로 트렌드 표시를 지원합니다.
 */
import { type Component, type JSX, Show } from 'solid-js'

export interface StatCardProps {
  /** 통계 라벨 */
  label: string
  /** 통계 값 (숫자 또는 문자열) */
  value: string | number
  /** 아이콘 (이모지 또는 컴포넌트) */
  icon?: string | JSX.Element
  /** 값의 색상 클래스 */
  valueColor?: string
  /** 트렌드 표시 (상승/하락/변동없음) */
  trend?: 'up' | 'down' | 'neutral'
  /** 트렌드 값 (예: "+5.2%") */
  trendValue?: string
  /** 추가 CSS 클래스 */
  className?: string
  /** 클릭 핸들러 */
  onClick?: () => void
}

/**
 * 통계 카드
 *
 * 숫자 데이터를 강조하여 표시하는 카드 컴포넌트입니다.
 *
 * @example
 * ```tsx
 * <StatCard
 *   label="총 종목"
 *   value={150}
 *   icon="📊"
 *   trend="up"
 *   trendValue="+12%"
 * />
 * ```
 */
export const StatCard: Component<StatCardProps> = (props) => {
  const trendColors = {
    up: 'text-green-500',
    down: 'text-red-500',
    neutral: 'text-gray-500',
  }

  const trendIcons = {
    up: '↑',
    down: '↓',
    neutral: '→',
  }

  return (
    <div
      class={`
        bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700
        p-4 transition-shadow
        ${props.onClick ? 'cursor-pointer hover:shadow-md' : ''}
        ${props.className || ''}
      `}
      onClick={props.onClick}
    >
      <div class="flex items-start justify-between">
        <div class="flex-1">
          <div class="text-sm text-gray-500 dark:text-gray-400 mb-1">{props.label}</div>
          <div
            class={`text-2xl font-bold ${props.valueColor || 'text-gray-900 dark:text-white'}`}
          >
            {props.value}
          </div>
          <Show when={props.trend && props.trendValue}>
            <div class={`text-sm mt-1 flex items-center gap-1 ${trendColors[props.trend!]}`}>
              <span>{trendIcons[props.trend!]}</span>
              <span>{props.trendValue}</span>
            </div>
          </Show>
        </div>
        <Show when={props.icon}>
          <div class="text-2xl opacity-80">
            {typeof props.icon === 'string' ? props.icon : props.icon}
          </div>
        </Show>
      </div>
    </div>
  )
}

export interface StatCardGridProps {
  children: JSX.Element
  /** 그리드 컬럼 수 (기본: 4) */
  columns?: 2 | 3 | 4 | 5 | 6
  /** 추가 CSS 클래스 */
  className?: string
}

/**
 * 통계 카드 그리드
 *
 * 여러 StatCard를 일관된 그리드 레이아웃으로 배치합니다.
 */
export const StatCardGrid: Component<StatCardGridProps> = (props) => {
  const gridCols = {
    2: 'grid-cols-1 sm:grid-cols-2',
    3: 'grid-cols-1 sm:grid-cols-2 lg:grid-cols-3',
    4: 'grid-cols-2 md:grid-cols-4',
    5: 'grid-cols-2 md:grid-cols-3 lg:grid-cols-5',
    6: 'grid-cols-2 md:grid-cols-3 lg:grid-cols-6',
  }

  return (
    <div class={`grid gap-4 ${gridCols[props.columns || 4]} ${props.className || ''}`}>
      {props.children}
    </div>
  )
}

export default StatCard
