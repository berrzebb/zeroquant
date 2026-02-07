/**
 * 뱃지 및 태그 컴포넌트
 *
 * 상태, 분류, 라벨 등을 시각적으로 표시합니다.
 */
import { type Component, type JSX, Show } from 'solid-js'

// ==================== Badge ====================

export type BadgeVariant = 'default' | 'primary' | 'success' | 'warning' | 'danger' | 'info'
export type BadgeSize = 'xs' | 'sm' | 'md'

export interface BadgeProps {
  children: JSX.Element
  /** 뱃지 스타일 변형 */
  variant?: BadgeVariant
  /** 크기 */
  size?: BadgeSize
  /** 둥근 모서리 (pill 스타일) */
  rounded?: boolean
  /** 외곽선 스타일 */
  outline?: boolean
  /** 아이콘 */
  icon?: string | JSX.Element
  /** 추가 CSS 클래스 */
  className?: string
}

/**
 * 뱃지 컴포넌트
 *
 * 상태나 분류를 작은 라벨로 표시합니다.
 *
 * @example
 * ```tsx
 * <Badge variant="success">활성</Badge>
 * <Badge variant="danger" icon="🔴">오류</Badge>
 * ```
 */
export const Badge: Component<BadgeProps> = (props) => {
  const variantClasses = {
    default: props.outline
      ? 'border border-gray-300 text-gray-700 dark:border-gray-600 dark:text-gray-300'
      : 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300',
    primary: props.outline
      ? 'border border-blue-300 text-blue-700 dark:border-blue-600 dark:text-blue-300'
      : 'bg-blue-100 text-blue-700 dark:bg-blue-900/50 dark:text-blue-300',
    success: props.outline
      ? 'border border-green-300 text-green-700 dark:border-green-600 dark:text-green-300'
      : 'bg-green-100 text-green-700 dark:bg-green-900/50 dark:text-green-300',
    warning: props.outline
      ? 'border border-yellow-300 text-yellow-700 dark:border-yellow-600 dark:text-yellow-300'
      : 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/50 dark:text-yellow-300',
    danger: props.outline
      ? 'border border-red-300 text-red-700 dark:border-red-600 dark:text-red-300'
      : 'bg-red-100 text-red-700 dark:bg-red-900/50 dark:text-red-300',
    info: props.outline
      ? 'border border-cyan-300 text-cyan-700 dark:border-cyan-600 dark:text-cyan-300'
      : 'bg-cyan-100 text-cyan-700 dark:bg-cyan-900/50 dark:text-cyan-300',
  }

  const sizeClasses = {
    xs: 'px-1.5 py-0.5 text-[10px]',
    sm: 'px-2 py-0.5 text-xs',
    md: 'px-2.5 py-1 text-sm',
  }

  return (
    <span
      class={`
        inline-flex items-center gap-1 font-medium
        ${props.rounded ? 'rounded-full' : 'rounded'}
        ${variantClasses[props.variant || 'default']}
        ${sizeClasses[props.size || 'sm']}
        ${props.className || ''}
      `}
    >
      <Show when={props.icon}>
        <span class="flex-shrink-0">
          {typeof props.icon === 'string' ? props.icon : props.icon}
        </span>
      </Show>
      {props.children}
    </span>
  )
}

// ==================== Status Badge ====================

export type StatusType = 'active' | 'inactive' | 'pending' | 'error' | 'success' | 'warning'

export interface StatusBadgeProps {
  /** 상태 타입 */
  status: StatusType
  /** 표시 텍스트 (기본값: 상태에 따라 자동 설정) */
  label?: string
  /** 크기 */
  size?: BadgeSize
  /** 추가 CSS 클래스 */
  className?: string
}

const statusConfig: Record<StatusType, { label: string; variant: BadgeVariant; icon: string }> = {
  active: { label: '활성', variant: 'success', icon: '●' },
  inactive: { label: '비활성', variant: 'default', icon: '○' },
  pending: { label: '대기중', variant: 'warning', icon: '◐' },
  error: { label: '오류', variant: 'danger', icon: '✕' },
  success: { label: '완료', variant: 'success', icon: '✓' },
  warning: { label: '경고', variant: 'warning', icon: '!' },
}

/**
 * 상태 뱃지
 *
 * 시스템 상태를 일관되게 표시합니다.
 *
 * @example
 * ```tsx
 * <StatusBadge status="active" />
 * <StatusBadge status="error" label="연결 실패" />
 * ```
 */
export const StatusBadge: Component<StatusBadgeProps> = (props) => {
  // status가 undefined/null이면 기본값 사용
  const config = () => statusConfig[props.status] || statusConfig['inactive']

  return (
    <Badge
      variant={config().variant}
      size={props.size}
      icon={config().icon}
      className={props.className}
    >
      {props.label || config().label}
    </Badge>
  )
}

// ==================== Market Badge ====================

export type MarketType = 'KR' | 'US' | 'CRYPTO' | string

export interface MarketBadgeProps {
  market: MarketType
  size?: BadgeSize
  className?: string
}

const marketConfig: Record<string, { label: string; color: string }> = {
  KR: { label: '🇰🇷 KRX', color: 'bg-blue-100 text-blue-700 dark:bg-blue-900/50 dark:text-blue-300' },
  US: { label: '🇺🇸 US', color: 'bg-green-100 text-green-700 dark:bg-green-900/50 dark:text-green-300' },
  CRYPTO: { label: '₿ 암호화폐', color: 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/50 dark:text-yellow-300' },
}

/**
 * 시장 뱃지
 *
 * 거래 시장을 표시합니다.
 */
export const MarketBadge: Component<MarketBadgeProps> = (props) => {
  // market이 undefined/null이면 기본값 사용
  const config = () => marketConfig[props.market] || { label: props.market || '-', color: 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300' }

  const sizeClasses = {
    xs: 'px-1.5 py-0.5 text-[10px]',
    sm: 'px-2 py-0.5 text-xs',
    md: 'px-2.5 py-1 text-sm',
  }

  return (
    <span
      class={`
        inline-flex items-center font-medium rounded
        ${config().color}
        ${sizeClasses[props.size || 'sm']}
        ${props.className || ''}
      `}
    >
      {config().label}
    </span>
  )
}

// ==================== Confidence Badge ====================

export type ConfidenceLevel = 'HIGH' | 'MEDIUM' | 'LOW'

export interface ConfidenceBadgeProps {
  level: ConfidenceLevel
  size?: BadgeSize
  className?: string
}

/**
 * 신뢰도 뱃지
 *
 * 분석 결과의 신뢰도를 표시합니다.
 */
export const ConfidenceBadge: Component<ConfidenceBadgeProps> = (props) => {
  const config: Record<ConfidenceLevel, { variant: BadgeVariant; label: string }> = {
    HIGH: { variant: 'success', label: '높음' },
    MEDIUM: { variant: 'warning', label: '보통' },
    LOW: { variant: 'danger', label: '낮음' },
  }

  // level이 undefined/null이면 기본값 사용
  const c = () => config[props.level] || { variant: 'default' as BadgeVariant, label: '-' }

  return (
    <Badge variant={c().variant} size={props.size} className={props.className}>
      {c().label}
    </Badge>
  )
}

export default Badge
