/**
 * 포맷팅 유틸리티 모듈
 *
 * 통화, 퍼센트, 숫자 등 다양한 포맷팅 함수를 제공합니다.
 * Intl.NumberFormat 객체를 캐싱하여 성능을 최적화합니다.
 *
 * @example
 * ```tsx
 * import { formatCurrency, formatPercent, formatNumber } from '../utils/format'
 *
 * formatCurrency(1234567)           // "₩1,234,567"
 * formatCurrency(1234.56, 'USD')    // "$1,234.56"
 * formatPercent(12.345)             // "+12.35%"
 * formatNumber(1234567, { compact: true }) // "1.2M"
 * ```
 */

// ==================== Intl.NumberFormat 캐시 (성능 최적화) ====================
// 매번 생성하지 않고 재사용하여 성능 향상

const KRW_FORMATTER = new Intl.NumberFormat('ko-KR', {
  style: 'currency',
  currency: 'KRW',
  maximumFractionDigits: 0,
})

const USD_FORMATTER = new Intl.NumberFormat('en-US', {
  style: 'currency',
  currency: 'USD',
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
})

const NUMBER_FORMATTER = new Intl.NumberFormat('ko-KR', {
  maximumFractionDigits: 4,
})

// ==================== 통화 포맷팅 ====================

/**
 * 통화 포맷 (KRW 또는 USD)
 *
 * @param value - 숫자 또는 문자열 값
 * @param currency - 통화 유형 (기본값: KRW)
 * @returns 포맷된 통화 문자열 (예: "₩1,234,567", "$1,234.56")
 *
 * @example
 * formatCurrency(1234567)           // "₩1,234,567"
 * formatCurrency(1234.56, 'USD')    // "$1,234.56"
 * formatCurrency("1000000")         // "₩1,000,000"
 */
export function formatCurrency(
  value: string | number | null | undefined,
  currency: 'KRW' | 'USD' = 'KRW'
): string {
  const numValue = typeof value === 'string' ? parseFloat(value) : (value ?? 0)
  if (isNaN(numValue)) return currency === 'KRW' ? '₩0' : '$0.00'

  return currency === 'KRW'
    ? KRW_FORMATTER.format(numValue)
    : USD_FORMATTER.format(numValue)
}

/**
 * 통화를 컴팩트하게 포맷 (한국어)
 *
 * @param value - 숫자 값
 * @returns 포맷된 문자열 (예: "1.2억원", "500만원")
 *
 * @example
 * formatCurrencyCompact(123456789) // "1.2억원"
 * formatCurrencyCompact(50000000)  // "5,000만원"
 * formatCurrencyCompact(12345)     // "12,345원"
 */
export function formatCurrencyCompact(
  value: string | number | null | undefined
): string {
  const numValue = typeof value === 'string' ? parseFloat(value) : (value ?? 0)
  if (isNaN(numValue)) return '₩0'

  const absValue = Math.abs(numValue)
  const sign = numValue < 0 ? '-' : ''

  if (absValue >= 1e8) {
    return `${sign}${(absValue / 1e8).toFixed(1)}억원`
  }
  if (absValue >= 1e4) {
    return `${sign}${Math.round(absValue / 1e4).toLocaleString('ko-KR')}만원`
  }
  return `${sign}${absValue.toLocaleString('ko-KR')}원`
}

// ==================== 퍼센트 포맷팅 ====================

/**
 * 퍼센트 포맷 (+/- 부호 포함)
 *
 * @param value - 숫자 또는 문자열 값
 * @param showSign - 양수일 때 + 부호 표시 여부 (기본값: true)
 * @returns 포맷된 퍼센트 문자열 (예: "+12.35%", "-5.00%")
 *
 * @example
 * formatPercent(12.345)           // "+12.35%"
 * formatPercent(-5)               // "-5.00%"
 * formatPercent(0)                // "+0.00%"
 * formatPercent(12.345, false)    // "12.35%"
 */
export function formatPercent(
  value: string | number | null | undefined,
  showSign = true
): string {
  const numValue = typeof value === 'string' ? parseFloat(value) : (value ?? 0)
  if (isNaN(numValue)) return showSign ? '+0.00%' : '0.00%'
  const sign = showSign && numValue > 0 ? '+' : ''
  return `${sign}${numValue.toFixed(2)}%`
}

// ==================== 숫자 포맷팅 ====================

export interface FormatNumberOptions {
  /** 소수점 자릿수 (기본값: 2) */
  decimals?: number
  /** 접두사 */
  prefix?: string
  /** 접미사 */
  suffix?: string
  /** 컴팩트 표기법 사용 (K, M, B) */
  compact?: boolean
  /** 천 단위 구분자 사용 (기본값: true) */
  useGrouping?: boolean
}

/**
 * 숫자 포맷
 *
 * @param value - 숫자 값
 * @param options - 포맷 옵션
 * @returns 포맷된 숫자 문자열
 *
 * @example
 * formatNumber(1234567)                           // "1,234,567.00"
 * formatNumber(1234567, { compact: true })        // "1.2M"
 * formatNumber(1234.5678, { decimals: 3 })        // "1,234.568"
 * formatNumber(1234, { prefix: '₩', suffix: '원' }) // "₩1,234원"
 */
export function formatNumber(
  value: number | string | null | undefined,
  options?: FormatNumberOptions
): string {
  const {
    decimals = 2,
    prefix = '',
    suffix = '',
    compact = false,
    useGrouping = true,
  } = options || {}

  const numValue = typeof value === 'string' ? parseFloat(value) : (value ?? 0)
  if (isNaN(numValue)) return `${prefix}0${suffix}`

  if (compact) {
    const absValue = Math.abs(numValue)
    const sign = numValue < 0 ? '-' : ''
    if (absValue >= 1e9) {
      return `${prefix}${sign}${(absValue / 1e9).toFixed(1)}B${suffix}`
    }
    if (absValue >= 1e6) {
      return `${prefix}${sign}${(absValue / 1e6).toFixed(1)}M${suffix}`
    }
    if (absValue >= 1e3) {
      return `${prefix}${sign}${(absValue / 1e3).toFixed(1)}K${suffix}`
    }
  }

  const formatted = useGrouping
    ? numValue.toLocaleString('ko-KR', {
        minimumFractionDigits: decimals,
        maximumFractionDigits: decimals,
      })
    : numValue.toFixed(decimals)

  return `${prefix}${formatted}${suffix}`
}

/**
 * 수량 포맷 (천 단위 구분자 + 최대 4자리 소수점)
 *
 * @param value - 숫자 또는 문자열 값
 * @returns 포맷된 수량 문자열
 *
 * @example
 * formatQuantity(1234.5678) // "1,234.5678"
 * formatQuantity(1000)      // "1,000"
 */
export function formatQuantity(value: string | number | null | undefined): string {
  const numValue = typeof value === 'string' ? parseFloat(value) : (value ?? 0)
  if (isNaN(numValue)) return '0'
  return NUMBER_FORMATTER.format(numValue)
}

// ==================== 날짜 포맷팅 ====================

/**
 * 날짜 포맷 (YYYY.MM.DD)
 *
 * @param dateStr - 날짜 문자열 또는 Date 객체
 * @returns 포맷된 날짜 문자열
 *
 * @example
 * formatDate('2024-01-15')           // "2024.01.15"
 * formatDate(new Date())              // "2024.02.06"
 */
export function formatDate(dateStr: string | Date | null | undefined): string {
  if (!dateStr) return '-'
  const date = typeof dateStr === 'string' ? new Date(dateStr) : dateStr
  if (isNaN(date.getTime())) return '-'
  return date.toLocaleDateString('ko-KR', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  })
}

/**
 * 날짜시간 포맷 (YYYY.MM.DD HH:mm)
 *
 * @param dateStr - 날짜 문자열 또는 Date 객체
 * @returns 포맷된 날짜시간 문자열
 *
 * @example
 * formatDateTime('2024-01-15T14:30:00') // "2024.01.15 14:30"
 */
export function formatDateTime(dateStr: string | Date | null | undefined): string {
  if (!dateStr) return '-'
  const date = typeof dateStr === 'string' ? new Date(dateStr) : dateStr
  if (isNaN(date.getTime())) return '-'
  return date.toLocaleString('ko-KR', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

/**
 * 상대적 시간 포맷 (예: "3분 전", "2시간 전")
 *
 * @param dateStr - 날짜 문자열 또는 Date 객체
 * @returns 상대적 시간 문자열
 *
 * @example
 * formatRelativeTime(new Date(Date.now() - 60000))  // "1분 전"
 * formatRelativeTime(new Date(Date.now() - 3600000)) // "1시간 전"
 */
export function formatRelativeTime(dateStr: string | Date | null | undefined): string {
  if (!dateStr) return '-'
  const date = typeof dateStr === 'string' ? new Date(dateStr) : dateStr
  if (isNaN(date.getTime())) return '-'

  const now = Date.now()
  const diff = now - date.getTime()
  const seconds = Math.floor(diff / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  const days = Math.floor(hours / 24)

  if (days > 0) return `${days}일 전`
  if (hours > 0) return `${hours}시간 전`
  if (minutes > 0) return `${minutes}분 전`
  return '방금 전'
}

// ==================== 색상 유틸리티 ====================

/**
 * 손익 색상 클래스 반환
 *
 * @param value - 숫자 값
 * @returns Tailwind CSS 색상 클래스
 *
 * @example
 * getPnLColor(100)  // "text-green-500"
 * getPnLColor(-50)  // "text-red-500"
 * getPnLColor(0)    // "text-gray-500"
 */
export function getPnLColor(value: string | number | null | undefined): string {
  if (value === null || value === undefined) return 'text-gray-500'
  const numValue = typeof value === 'string' ? parseFloat(value) : value
  if (isNaN(numValue)) return 'text-gray-500'
  if (numValue > 0) return 'text-green-500'
  if (numValue < 0) return 'text-red-500'
  return 'text-gray-500'
}

/**
 * 손익 배경 색상 클래스 반환
 */
export function getPnLBgColor(value: string | number | null | undefined): string {
  if (value === null || value === undefined) return 'bg-gray-400'
  const numValue = typeof value === 'string' ? parseFloat(value) : value
  if (isNaN(numValue)) return 'bg-gray-400'
  if (numValue > 0) return 'bg-green-500'
  if (numValue < 0) return 'bg-red-500'
  return 'bg-gray-400'
}

// ==================== 전략 관련 유틸리티 ====================

/**
 * 전략 타입에 따른 기본 타임프레임 반환
 *
 * @param strategyType - 전략 타입 식별자
 * @returns 기본 타임프레임 문자열 (예: "1m", "15m", "1d")
 */
export function getDefaultTimeframe(strategyType: string): string {
  switch (strategyType) {
    // 실시간 전략: 1m
    case 'grid':
    case 'grid_trading':
    case 'magic_split':
    case 'split':
    case 'infinity_bot':
    case 'trailing_stop':
      return '1m'
    // 분 기반 전략: 15m
    case 'rsi':
    case 'rsi_mean_reversion':
    case 'bollinger':
    case 'bollinger_bands':
    case 'sma':
    case 'sma_crossover':
    case 'ma_crossover':
    case 'candle_pattern':
      return '15m'
    // 일간 전략: 1d
    case 'volatility_breakout':
    case 'volatility':
    case 'snow':
    case 'momentum_power':
    case 'stock_rotation':
    case 'rotation':
    case 'market_interest_day':
    case 'compound_momentum':
    case 'haa':
    case 'xaa':
    case 'all_weather':
    case 'market_cap_top':
      return '1d'
    default:
      return '1d'
  }
}
