/**
 * 신호 상세 팝업 컴포넌트
 *
 * 신호 마커 클릭 시 상세 정보를 표시하는 모달 팝업입니다.
 * ESC 키 또는 외부 클릭으로 닫을 수 있습니다.
 */
import { type Component, Show, For, onMount, onCleanup, createMemo } from 'solid-js'
import { X, TrendingUp, TrendingDown, AlertCircle, Clock, Target, Activity, Zap } from 'lucide-solid'
import type { SignalMarker, RouteStateType } from '../../types'
import { formatNumber, formatDateTime } from '../../utils/format'

// ==================== 타입 정의 ====================

export interface SignalDetailPopupProps {
  /** 표시할 마커 */
  marker: SignalMarker
  /** 팝업 위치 (화면 좌표) */
  position: { x: number; y: number }
  /** 닫기 콜백 */
  onClose: () => void
  /** RouteState 정보 (선택) */
  routeState?: RouteStateType
}

// ==================== 상수 ====================

const ROUTE_STATE_COLORS: Record<RouteStateType, { bg: string; text: string; label: string }> = {
  ATTACK: { bg: 'bg-red-500/20', text: 'text-red-400', label: '공격' },
  ARMED: { bg: 'bg-orange-500/20', text: 'text-orange-400', label: '준비' },
  NEUTRAL: { bg: 'bg-gray-500/20', text: 'text-gray-400', label: '중립' },
  WAIT: { bg: 'bg-blue-500/20', text: 'text-blue-400', label: '대기' },
  OVERHEAT: { bg: 'bg-purple-500/20', text: 'text-purple-400', label: '과열' },
}

const SIGNAL_TYPE_INFO: Record<string, { icon: typeof TrendingUp; color: string; label: string }> = {
  buy: { icon: TrendingUp, color: 'text-green-400', label: '매수' },
  sell: { icon: TrendingDown, color: 'text-red-400', label: '매도' },
  alert: { icon: AlertCircle, color: 'text-yellow-400', label: '알림' },
}

// ==================== 유틸리티 ====================

// 소수점 값(0.05)을 퍼센트로 변환 (5.0%)
function formatPercentDecimal(value: number): string {
  return `${(value * 100).toFixed(1)}%`
}

// ==================== 서브 컴포넌트 ====================

interface InfoRowProps {
  label: string
  value: string | number
  valueClass?: string
}

const InfoRow: Component<InfoRowProps> = (props) => (
  <div class="flex justify-between items-center py-1.5">
    <span class="text-[var(--color-text-muted)] text-sm">{props.label}</span>
    <span class={`text-sm font-medium ${props.valueClass || 'text-[var(--color-text)]'}`}>
      {props.value}
    </span>
  </div>
)

interface IndicatorBadgeProps {
  name: string
  value: number
  decimals?: number
}

const IndicatorBadge: Component<IndicatorBadgeProps> = (props) => (
  <div class="flex items-center gap-1.5 px-2 py-1 bg-[var(--color-surface-light)]/50 rounded text-xs">
    <span class="text-[var(--color-text-muted)]">{props.name}</span>
    <span class="text-[var(--color-text)] font-medium">
      {formatNumber(props.value, { decimals: props.decimals ?? 2 })}
    </span>
  </div>
)

// ==================== 메인 컴포넌트 ====================

/**
 * 신호 상세 팝업
 *
 * @example
 * ```tsx
 * <Show when={selectedMarker()}>
 *   <SignalDetailPopup
 *     marker={selectedMarker()!}
 *     position={{ x: 100, y: 200 }}
 *     onClose={() => setSelectedMarker(null)}
 *   />
 * </Show>
 * ```
 */
export const SignalDetailPopup: Component<SignalDetailPopupProps> = (props) => {
  let popupRef: HTMLDivElement | undefined
  let clickListenerTimeout: ReturnType<typeof setTimeout> | null = null

  // ESC 키로 닫기
  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      props.onClose()
    }
  }

  // 외부 클릭으로 닫기
  const handleClickOutside = (e: MouseEvent) => {
    if (popupRef && !popupRef.contains(e.target as Node)) {
      props.onClose()
    }
  }

  onMount(() => {
    document.addEventListener('keydown', handleKeyDown)
    // 약간의 딜레이 후 외부 클릭 리스너 등록 (클릭으로 열린 직후 바로 닫히는 것 방지)
    clickListenerTimeout = setTimeout(() => {
      document.addEventListener('click', handleClickOutside)
    }, 100)
  })

  onCleanup(() => {
    if (clickListenerTimeout) clearTimeout(clickListenerTimeout)
    document.removeEventListener('keydown', handleKeyDown)
    document.removeEventListener('click', handleClickOutside)
  })

  // 신호 타입 정보
  const signalInfo = createMemo(() => {
    const type = props.marker.signal_type.toLowerCase()
    return SIGNAL_TYPE_INFO[type] || SIGNAL_TYPE_INFO.alert
  })

  // 팝업 위치 계산 (화면 경계 고려)
  const popupStyle = createMemo(() => {
    const { x, y } = props.position
    const popupWidth = 320
    const popupHeight = 400

    // 화면 경계 체크
    const viewportWidth = window.innerWidth
    const viewportHeight = window.innerHeight

    let left = x + 10
    let top = y + 10

    // 오른쪽 경계 초과 시 왼쪽에 표시
    if (left + popupWidth > viewportWidth - 20) {
      left = x - popupWidth - 10
    }

    // 하단 경계 초과 시 위쪽에 표시
    if (top + popupHeight > viewportHeight - 20) {
      top = y - popupHeight - 10
    }

    // 음수 방지
    left = Math.max(10, left)
    top = Math.max(10, top)

    return {
      left: `${left}px`,
      top: `${top}px`,
    }
  })

  // 지표 목록
  const indicatorList = createMemo(() => {
    const entries: { name: string; value: number; decimals?: number }[] = []
    const ind = props.marker.indicators

    if (ind.rsi !== undefined) entries.push({ name: 'RSI', value: ind.rsi, decimals: 1 })
    if (ind.macd !== undefined) entries.push({ name: 'MACD', value: ind.macd, decimals: 4 })
    if (ind.macd_signal !== undefined) entries.push({ name: 'Signal', value: ind.macd_signal, decimals: 4 })
    if (ind.macd_histogram !== undefined) entries.push({ name: 'Hist', value: ind.macd_histogram, decimals: 4 })
    if (ind.volume_ratio !== undefined) entries.push({ name: '거래량비', value: ind.volume_ratio, decimals: 1 })
    if (ind.atr !== undefined) entries.push({ name: 'ATR', value: ind.atr, decimals: 2 })
    if (ind.sma_20 !== undefined) entries.push({ name: 'SMA20', value: ind.sma_20, decimals: 0 })
    if (ind.sma_50 !== undefined) entries.push({ name: 'SMA50', value: ind.sma_50, decimals: 0 })
    if (ind.bb_upper !== undefined) entries.push({ name: 'BB상단', value: ind.bb_upper, decimals: 0 })
    if (ind.bb_lower !== undefined) entries.push({ name: 'BB하단', value: ind.bb_lower, decimals: 0 })

    return entries
  })

  const SignalIcon = signalInfo().icon

  return (
    <div
      ref={popupRef}
      class="fixed z-[100] w-80 bg-[var(--color-surface)] border border-[var(--color-surface-light)]
             rounded-xl shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-200"
      style={popupStyle()}
      onClick={(e) => e.stopPropagation()}
    >
      {/* 헤더 */}
      <div class="flex items-center justify-between px-4 py-3 bg-[var(--color-surface-light)]/30 border-b border-[var(--color-surface-light)]">
        <div class="flex items-center gap-2">
          <SignalIcon class={`w-5 h-5 ${signalInfo().color}`} />
          <span class="font-semibold text-[var(--color-text)]">{props.marker.symbol}</span>
          <span class={`px-2 py-0.5 rounded text-xs font-medium ${signalInfo().color} bg-current/10`}>
            {signalInfo().label}
          </span>
        </div>
        <button
          onClick={props.onClose}
          class="p-1 rounded-lg hover:bg-[var(--color-surface-light)] text-[var(--color-text-muted)]
                 hover:text-[var(--color-text)] transition-colors"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      {/* 본문 */}
      <div class="p-4 space-y-4 max-h-[400px] overflow-y-auto">
        {/* 기본 정보 */}
        <div class="space-y-1">
          <InfoRow
            label="발생 시간"
            value={formatDateTime(props.marker.timestamp)}
          />
          <InfoRow
            label="가격"
            value={formatNumber(props.marker.price, { decimals: 0 })}
          />
          <InfoRow
            label="신호 강도"
            value={formatPercentDecimal(props.marker.strength)}
            valueClass={props.marker.strength >= 0.7 ? 'text-green-400' : props.marker.strength >= 0.4 ? 'text-yellow-400' : 'text-red-400'}
          />
        </div>

        {/* 강도 프로그레스 바 */}
        <div>
          <div class="flex justify-between text-xs mb-1">
            <span class="text-[var(--color-text-muted)]">강도</span>
            <span class="text-[var(--color-text)]">{(props.marker.strength * 100).toFixed(0)}%</span>
          </div>
          <div class="h-2 bg-[var(--color-surface-light)] rounded-full overflow-hidden">
            <div
              class={`h-full rounded-full transition-all ${
                props.marker.strength >= 0.7 ? 'bg-green-500' :
                props.marker.strength >= 0.4 ? 'bg-yellow-500' : 'bg-red-500'
              }`}
              style={{ width: `${props.marker.strength * 100}%` }}
            />
          </div>
        </div>

        {/* RouteState */}
        <Show when={props.routeState}>
          {(state) => {
            const stateInfo = ROUTE_STATE_COLORS[state()]
            return (
              <div class="flex items-center gap-2">
                <Activity class="w-4 h-4 text-[var(--color-text-muted)]" />
                <span class="text-sm text-[var(--color-text-muted)]">RouteState</span>
                <span class={`px-2 py-0.5 rounded text-xs font-medium ${stateInfo.bg} ${stateInfo.text}`}>
                  {state()} ({stateInfo.label})
                </span>
              </div>
            )
          }}
        </Show>

        {/* 전략 정보 */}
        <Show when={props.marker.strategy_name || props.marker.strategy_id}>
          <div class="flex items-center gap-2">
            <Target class="w-4 h-4 text-[var(--color-text-muted)]" />
            <span class="text-sm text-[var(--color-text-muted)]">전략</span>
            <span class="text-sm text-[var(--color-text)] font-medium">
              {props.marker.strategy_name || props.marker.strategy_id}
            </span>
          </div>
        </Show>

        {/* 실행 여부 */}
        <div class="flex items-center gap-2">
          <Zap class="w-4 h-4 text-[var(--color-text-muted)]" />
          <span class="text-sm text-[var(--color-text-muted)]">실행 여부</span>
          <Show
            when={props.marker.executed}
            fallback={
              <span class="px-2 py-0.5 rounded text-xs font-medium bg-gray-500/20 text-gray-400">
                미체결
              </span>
            }
          >
            <span class="px-2 py-0.5 rounded text-xs font-medium bg-green-500/20 text-green-400">
              ✓ 체결됨
            </span>
          </Show>
        </div>

        {/* 지표 정보 */}
        <Show when={indicatorList().length > 0}>
          <div class="pt-3 border-t border-[var(--color-surface-light)]">
            <div class="flex items-center gap-2 mb-2">
              <Clock class="w-4 h-4 text-[var(--color-text-muted)]" />
              <span class="text-sm text-[var(--color-text-muted)]">지표 값</span>
            </div>
            <div class="flex flex-wrap gap-1.5">
              <For each={indicatorList()}>
                {(ind) => (
                  <IndicatorBadge
                    name={ind.name}
                    value={ind.value}
                    decimals={ind.decimals}
                  />
                )}
              </For>
            </div>
          </div>
        </Show>

        {/* 사유 */}
        <Show when={props.marker.reason}>
          <div class="pt-3 border-t border-[var(--color-surface-light)]">
            <div class="text-xs text-[var(--color-text-muted)] mb-1">신호 사유</div>
            <p class="text-sm text-[var(--color-text)] bg-[var(--color-surface-light)]/30 rounded-lg p-2">
              {props.marker.reason}
            </p>
          </div>
        </Show>
      </div>

      {/* 푸터 */}
      <div class="px-4 py-2 bg-[var(--color-surface-light)]/20 border-t border-[var(--color-surface-light)] text-xs text-[var(--color-text-muted)]">
        ESC 또는 외부 클릭으로 닫기
      </div>
    </div>
  )
}

export default SignalDetailPopup
