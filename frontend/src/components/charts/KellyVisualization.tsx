/**
 * Kelly Criterion 시각화 컴포넌트
 *
 * 켈리 공식 기반 자금 관리 비율을 시각적으로 표시합니다.
 * 이론적 최적 배분, 현재 배분, 위험 한도를 한눈에 비교할 수 있습니다.
 *
 * @example
 * ```tsx
 * <KellyVisualization
 *   kellyFraction={0.25}
 *   currentAllocation={0.15}
 *   maxRisk={0.5}
 * />
 * ```
 */
import { createMemo, Show } from 'solid-js'
import type { Component, JSX } from 'solid-js'

export interface KellyVisualizationProps {
  /** 켈리 비율 (0.0 ~ 1.0+) - 이론적 최적 배분 비율 */
  kellyFraction: number
  /** 현재 배분 비율 (0.0 ~ 1.0) */
  currentAllocation: number
  /** 최대 위험 한도 (0.0 ~ 1.0, 기본값 0.5) */
  maxRisk?: number
  /** 반 켈리 표시 여부 */
  showHalfKelly?: boolean
  /** 차트 높이 */
  height?: number
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

/** 배분 상태 타입 */
type AllocationStatus = 'optimal' | 'under' | 'over' | 'danger'

/**
 * 배분 상태 계산
 */
function getAllocationStatus(
  current: number,
  kelly: number,
  maxRisk: number
): AllocationStatus {
  if (current > maxRisk) return 'danger'
  if (current > kelly * 1.1) return 'over'
  if (current < kelly * 0.5) return 'under'
  return 'optimal'
}

/**
 * 상태별 색상
 */
const STATUS_COLORS: Record<AllocationStatus, string> = {
  optimal: '#22c55e',  // 초록 - 최적 범위
  under: '#f59e0b',    // 노랑 - 과소 배분
  over: '#f97316',     // 주황 - 과대 배분
  danger: '#ef4444',   // 빨강 - 위험 한도 초과
}

/**
 * 상태별 메시지
 */
const STATUS_MESSAGES: Record<AllocationStatus, string> = {
  optimal: '최적 배분 범위',
  under: '과소 배분 - 수익 기회 손실',
  over: '과대 배분 - 리스크 상승',
  danger: '위험 한도 초과!',
}

/**
 * Kelly Visualization 컴포넌트
 */
export const KellyVisualization: Component<KellyVisualizationProps> = (props) => {
  const maxRisk = () => props.maxRisk ?? 0.5
  const height = () => props.height ?? 80
  const showHalfKelly = () => props.showHalfKelly ?? true

  // 켈리 비율을 표시 범위 내로 제한 (최대 150%)
  const displayKelly = createMemo(() => Math.min(props.kellyFraction, 1.5))
  const halfKelly = createMemo(() => displayKelly() / 2)

  // 배분 상태 계산
  const status = createMemo(() =>
    getAllocationStatus(props.currentAllocation, props.kellyFraction, maxRisk())
  )

  // 퍼센트 위치 계산 (0~100%)
  const kellyPosition = createMemo(() => Math.min(displayKelly() * 100, 100))
  const halfKellyPosition = createMemo(() => Math.min(halfKelly() * 100, 100))
  const currentPosition = createMemo(() => Math.min(props.currentAllocation * 100, 100))
  const maxRiskPosition = createMemo(() => Math.min(maxRisk() * 100, 100))

  // 켈리 공식 툴팁 내용
  const kellyTooltip = createMemo(() => {
    const k = props.kellyFraction
    return `켈리 비율: ${(k * 100).toFixed(1)}%

f* = (p × b - q) / b

여기서:
• f* = 최적 배분 비율
• p = 승률
• q = 패율 (1 - p)
• b = 손익비 (평균 이익/평균 손실)

실전에서는 Half Kelly (${(k * 50).toFixed(1)}%)를
사용하여 변동성을 줄이는 것이 일반적입니다.`
  })

  return (
    <div
      class={`bg-gray-800/50 rounded-xl p-4 ${props.class || ''}`}
      style={props.style}
    >
      {/* 헤더 */}
      <div class="flex items-center justify-between mb-4">
        <div class="flex items-center gap-2">
          <span class="text-lg">📊</span>
          <h3 class="text-sm font-semibold text-white">켈리 자금관리</h3>
        </div>
        <div
          class="px-2 py-1 rounded text-xs font-medium"
          style={{
            'background-color': `${STATUS_COLORS[status()]}20`,
            color: STATUS_COLORS[status()],
          }}
        >
          {STATUS_MESSAGES[status()]}
        </div>
      </div>

      {/* 메인 바 차트 */}
      <div class="relative" style={{ height: `${height()}px` }}>
        {/* 배경 바 */}
        <div class="absolute inset-0 bg-gray-700 rounded-lg overflow-hidden">
          {/* 위험 영역 (maxRisk 이후) */}
          <div
            class="absolute top-0 bottom-0 bg-red-500/20"
            style={{
              left: `${maxRiskPosition()}%`,
              right: '0',
            }}
          />

          {/* 안전 영역 (0 ~ 반켈리) */}
          <Show when={showHalfKelly()}>
            <div
              class="absolute top-0 bottom-0 bg-blue-500/10"
              style={{
                left: '0',
                width: `${halfKellyPosition()}%`,
              }}
            />
          </Show>

          {/* 최적 영역 (반켈리 ~ 켈리) */}
          <div
            class="absolute top-0 bottom-0 bg-green-500/20"
            style={{
              left: `${halfKellyPosition()}%`,
              width: `${kellyPosition() - halfKellyPosition()}%`,
            }}
          />
        </div>

        {/* 눈금 */}
        <div class="absolute bottom-0 left-0 right-0 h-6 flex">
          {[0, 25, 50, 75, 100].map((tick) => (
            <div
              class="absolute text-xs text-gray-500"
              style={{ left: `${tick}%`, transform: 'translateX(-50%)' }}
            >
              {tick}%
            </div>
          ))}
        </div>

        {/* 마커들 */}
        <div class="absolute top-0 left-0 right-0" style={{ height: `${height() - 24}px` }}>
          {/* 반 켈리 마커 */}
          <Show when={showHalfKelly()}>
            <div
              class="absolute top-0 bottom-0 w-0.5 bg-blue-400"
              style={{ left: `${halfKellyPosition()}%` }}
              title={`Half Kelly: ${(halfKelly() * 100).toFixed(1)}%`}
            >
              <div class="absolute -top-1 left-1/2 -translate-x-1/2 w-3 h-3 bg-blue-400 rotate-45" />
              <div class="absolute -top-6 left-1/2 -translate-x-1/2 whitespace-nowrap text-xs text-blue-400 font-medium">
                ½K
              </div>
            </div>
          </Show>

          {/* 켈리 마커 */}
          <div
            class="absolute top-0 bottom-0 w-0.5 bg-green-400"
            style={{ left: `${kellyPosition()}%` }}
            title={kellyTooltip()}
          >
            <div class="absolute -top-1 left-1/2 -translate-x-1/2 w-3 h-3 bg-green-400 rotate-45" />
            <div class="absolute -top-6 left-1/2 -translate-x-1/2 whitespace-nowrap text-xs text-green-400 font-medium">
              Kelly
            </div>
          </div>

          {/* 최대 위험 한도 마커 */}
          <div
            class="absolute top-0 bottom-0 w-0.5 bg-red-400 border-dashed"
            style={{ left: `${maxRiskPosition()}%` }}
            title={`최대 위험 한도: ${(maxRisk() * 100).toFixed(0)}%`}
          >
            <div class="absolute -top-1 left-1/2 -translate-x-1/2 w-3 h-3 bg-red-400 rotate-45" />
            <div class="absolute -top-6 left-1/2 -translate-x-1/2 whitespace-nowrap text-xs text-red-400 font-medium">
              Limit
            </div>
          </div>

          {/* 현재 배분 마커 */}
          <div
            class="absolute top-0 bottom-0 w-1 rounded"
            style={{
              left: `${currentPosition()}%`,
              'background-color': STATUS_COLORS[status()],
              transform: 'translateX(-50%)',
            }}
          >
            <div
              class="absolute -bottom-1 left-1/2 -translate-x-1/2 w-4 h-4 rounded-full border-2 border-white"
              style={{ 'background-color': STATUS_COLORS[status()] }}
            />
          </div>
        </div>
      </div>

      {/* 하단 정보 */}
      <div class="mt-4 grid grid-cols-3 gap-4 text-center">
        <div>
          <div class="text-xs text-gray-400">현재 배분</div>
          <div
            class="text-lg font-mono font-bold"
            style={{ color: STATUS_COLORS[status()] }}
          >
            {(props.currentAllocation * 100).toFixed(1)}%
          </div>
        </div>
        <div>
          <div class="text-xs text-gray-400">켈리 최적</div>
          <div class="text-lg font-mono font-bold text-green-400">
            {(props.kellyFraction * 100).toFixed(1)}%
          </div>
        </div>
        <div>
          <div class="text-xs text-gray-400">권장 (½K)</div>
          <div class="text-lg font-mono font-bold text-blue-400">
            {(props.kellyFraction * 50).toFixed(1)}%
          </div>
        </div>
      </div>

      {/* 경고 메시지 */}
      <Show when={status() === 'danger'}>
        <div class="mt-3 p-2 bg-red-500/10 border border-red-500/30 rounded-lg">
          <div class="flex items-center gap-2 text-red-400 text-sm">
            <span>⚠️</span>
            <span>현재 배분이 최대 위험 한도를 초과했습니다. 포지션 축소를 권장합니다.</span>
          </div>
        </div>
      </Show>

      <Show when={status() === 'over' && status() !== 'danger'}>
        <div class="mt-3 p-2 bg-orange-500/10 border border-orange-500/30 rounded-lg">
          <div class="flex items-center gap-2 text-orange-400 text-sm">
            <span>⚡</span>
            <span>켈리 비율을 초과한 배분입니다. 변동성에 주의하세요.</span>
          </div>
        </div>
      </Show>
    </div>
  )
}

export default KellyVisualization
