/**
 * 헤더 시장 인디케이터 컴포넌트
 *
 * KRX, NYSE, Crypto 시장 상태를 실시간으로 표시합니다.
 * 시장 온도(Market Breadth)와 매크로 환경(USD/KRW, NASDAQ)을 제공합니다.
 */
import { createResource, createMemo, Show, onCleanup } from 'solid-js'
import type { Component, JSX } from 'solid-js'
import {
  getMarketOverview,
  type MarketBreadthResponse,
  type MacroEnvironmentResponse,
} from '../../api/client'

// ==================== 타입 정의 ====================

interface MarketIndicator {
  market: string
  label: string
  isOpen: boolean
  session?: string
}

// ==================== 유틸리티 함수 ====================

/** 온도에 따른 색상 반환 */
function getTemperatureColor(temp: string): string {
  switch (temp.toUpperCase()) {
    case 'OVERHEAT':
      return 'text-red-500'
    case 'COLD':
      return 'text-blue-500'
    default:
      return 'text-yellow-500'
  }
}

/** 위험도에 따른 색상 반환 */
function getRiskColor(risk: string): string {
  switch (risk.toUpperCase()) {
    case 'CRITICAL':
      return 'text-red-500'
    case 'WARNING':
      return 'text-orange-500'
    case 'CAUTION':
      return 'text-yellow-500'
    case 'SAFE':
      return 'text-green-500'
    default:
      return 'text-[var(--color-text-muted)]'
  }
}

/** 변동률 색상 반환 */
function getChangeColor(change: number): string {
  if (change > 0) return 'text-red-500'
  if (change < 0) return 'text-blue-500'
  return 'text-[var(--color-text-muted)]'
}

/** 변동률 포맷팅 */
function formatChange(change: number): string {
  const sign = change > 0 ? '+' : ''
  return `${sign}${change.toFixed(2)}%`
}

/** 세션 라벨 변환 */
function getSessionLabel(session?: string): string {
  switch (session) {
    case 'PreMarket':
      return '프리'
    case 'AfterHours':
      return '야간'
    case 'Regular':
      return ''
    default:
      return ''
  }
}

// ==================== 서브 컴포넌트 ====================

/** 단일 시장 인디케이터 */
const MarketStatusIndicator: Component<{
  market: string
  label: string
  isOpen: boolean
  session?: string
}> = (props) => {
  const sessionLabel = createMemo(() => getSessionLabel(props.session))

  return (
    <div class="flex items-center gap-2 px-3 py-1.5 rounded-full bg-[var(--color-surface-light)]">
      <div
        class={`w-2 h-2 rounded-full ${
          props.isOpen ? 'bg-green-500 animate-pulse' : 'bg-red-500'
        }`}
      />
      <span class="text-sm text-[var(--color-text)]">
        {props.label}
        <Show when={sessionLabel()}>
          <span class="ml-1 text-xs text-[var(--color-text-muted)]">
            ({sessionLabel()})
          </span>
        </Show>
      </span>
    </div>
  )
}

/** 시장 온도 표시 */
const MarketTemperatureIndicator: Component<{
  data: MarketBreadthResponse | undefined
  loading: boolean
}> = (props) => {
  return (
    <Show when={!props.loading && props.data}>
      <div class="flex items-center gap-2 px-3 py-1.5 rounded-full bg-[var(--color-surface-light)]">
        <span class="text-base">{props.data?.temperatureIcon}</span>
        <span class={`text-sm font-medium ${getTemperatureColor(props.data?.temperature || 'NEUTRAL')}`}>
          {parseFloat(props.data?.all || '0').toFixed(1)}%
        </span>
      </div>
    </Show>
  )
}

/** 매크로 환경 표시 */
const MacroEnvironmentIndicator: Component<{
  data: MacroEnvironmentResponse | undefined
  loading: boolean
}> = (props) => {
  return (
    <Show when={!props.loading && props.data}>
      {/* KOSPI */}
      <div class="flex items-center gap-2 px-3 py-1.5 rounded-full bg-[var(--color-surface-light)]">
        <span class="text-xs text-[var(--color-text-muted)]">KOSPI</span>
        <span class="text-sm font-medium text-[var(--color-text)]">
          {parseFloat(props.data?.kospi || '0').toLocaleString('ko-KR', { maximumFractionDigits: 2 })}
        </span>
        <span class={`text-xs font-medium ${getChangeColor(props.data?.kospiChangePct || 0)}`}>
          {formatChange(props.data?.kospiChangePct || 0)}
        </span>
      </div>

      {/* KOSDAQ */}
      <div class="flex items-center gap-2 px-3 py-1.5 rounded-full bg-[var(--color-surface-light)]">
        <span class="text-xs text-[var(--color-text-muted)]">KOSDAQ</span>
        <span class="text-sm font-medium text-[var(--color-text)]">
          {parseFloat(props.data?.kosdaq || '0').toLocaleString('ko-KR', { maximumFractionDigits: 2 })}
        </span>
        <span class={`text-xs font-medium ${getChangeColor(props.data?.kosdaqChangePct || 0)}`}>
          {formatChange(props.data?.kosdaqChangePct || 0)}
        </span>
      </div>

      {/* USD/KRW */}
      <div class="flex items-center gap-2 px-3 py-1.5 rounded-full bg-[var(--color-surface-light)]">
        <span class="text-xs text-[var(--color-text-muted)]">USD</span>
        <span class="text-sm font-medium text-[var(--color-text)]">
          ₩{parseFloat(props.data?.usdKrw || '0').toLocaleString('ko-KR', { maximumFractionDigits: 0 })}
        </span>
        <span class={`text-xs font-medium ${getChangeColor(props.data?.usdChangePct || 0)}`}>
          {formatChange(props.data?.usdChangePct || 0)}
        </span>
      </div>

      {/* VIX */}
      <div class="flex items-center gap-2 px-3 py-1.5 rounded-full bg-[var(--color-surface-light)]">
        <span class="text-xs text-[var(--color-text-muted)]">VIX</span>
        <span class="text-sm font-medium text-[var(--color-text)]">
          {parseFloat(props.data?.vix || '0').toFixed(2)}
        </span>
        <span class={`text-xs font-medium ${getChangeColor(props.data?.vixChangePct || 0)}`}>
          {formatChange(props.data?.vixChangePct || 0)}
        </span>
      </div>

      {/* 위험도 표시 */}
      <div class="flex items-center gap-1 px-3 py-1.5 rounded-full bg-[var(--color-surface-light)]">
        <span class="text-base">{props.data?.riskIcon}</span>
        <span class={`text-xs font-medium ${getRiskColor(props.data?.riskLevel || '')}`}>
          {props.data?.riskLevel}
        </span>
      </div>
    </Show>
  )
}

// ==================== 메인 컴포넌트 ====================

export interface HeaderIndicatorsProps {
  /** 컴팩트 모드 */
  compact?: boolean
  /** 시장 온도 표시 */
  showTemperature?: boolean
  /** 매크로 환경 표시 */
  showMacro?: boolean
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

/**
 * 헤더 시장 인디케이터
 *
 * KRX, NYSE, Crypto 시장 상태와 시장 온도, 매크로 환경을 표시합니다.
 * 1분마다 자동 갱신됩니다.
 */
export const HeaderIndicators: Component<HeaderIndicatorsProps> = (props) => {
  // 시장 통합 조회 (1회 호출로 status + breadth + macro 모두 반환)
  const [overview, { refetch: refetchOverview }] = createResource(
    () => true,
    () => getMarketOverview().catch(() => null)
  )

  // 1분마다 자동 갱신
  const refreshInterval = setInterval(() => {
    refetchOverview()
  }, 60000)

  onCleanup(() => clearInterval(refreshInterval))

  // 시장 인디케이터 목록 생성
  const indicators = createMemo<MarketIndicator[]>(() => {
    const data = overview()
    const list: MarketIndicator[] = []

    // KRX (한국)
    list.push({
      market: 'KR',
      label: 'KRX',
      isOpen: data?.kr.isOpen ?? false,
      session: data?.kr.session,
    })

    // NYSE (미국)
    list.push({
      market: 'US',
      label: 'NYSE',
      isOpen: data?.us.isOpen ?? false,
      session: data?.us.session,
    })

    // Crypto (24시간)
    list.push({
      market: 'CRYPTO',
      label: 'Crypto',
      isOpen: true,
    })

    return list
  })

  return (
    <div
      class={`flex items-center gap-3 ${props.class || ''}`}
      style={props.style}
    >
      {/* 시장 상태 인디케이터 */}
      {indicators().map((ind) => (
        <MarketStatusIndicator
          market={ind.market}
          label={ind.label}
          isOpen={ind.isOpen}
          session={ind.session}
        />
      ))}

      {/* 시장 온도 */}
      <Show when={props.showTemperature !== false}>
        <MarketTemperatureIndicator
          data={overview()?.breadth}
          loading={overview.loading}
        />
      </Show>

      {/* 매크로 환경 */}
      <Show when={props.showMacro !== false}>
        <MacroEnvironmentIndicator
          data={overview()?.macro}
          loading={overview.loading}
        />
      </Show>
    </div>
  )
}

export default HeaderIndicators
