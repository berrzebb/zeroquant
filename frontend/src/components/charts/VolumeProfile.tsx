/**
 * 볼륨 프로파일 (매물대) 컴포넌트
 *
 * 가격대별 거래량을 수평 막대 차트로 표시합니다.
 * 캔들 차트 Y축과 동기화하여 오버레이로 사용할 수 있습니다.
 *
 * @example
 * ```tsx
 * <VolumeProfile
 *   priceVolumes={[
 *     { price: 50000, volume: 1000000 },
 *     { price: 51000, volume: 1500000 },
 *   ]}
 *   currentPrice={51500}
 *   chartHeight={400}
 * />
 * ```
 */
import { createMemo, For, Show } from 'solid-js'
import type { Component, JSX } from 'solid-js'

/** 가격대별 거래량 데이터 */
export interface PriceVolume {
  /** 가격 */
  price: number
  /** 거래량 */
  volume: number
}

export interface VolumeProfileProps {
  /** 가격대별 거래량 데이터 */
  priceVolumes: PriceVolume[]
  /** 현재가 */
  currentPrice: number
  /** 차트 높이 */
  chartHeight: number
  /** 차트 너비 (기본값: 100) */
  width?: number
  /** 가격 범위 (min, max) - 지정하지 않으면 자동 계산 */
  priceRange?: [number, number]
  /** POC (Point of Control) 강조 */
  showPoc?: boolean
  /** Value Area 표시 (70% 거래량 영역) */
  showValueArea?: boolean
  /** 막대 색상 */
  barColor?: string
  /** POC 색상 */
  pocColor?: string
  /** Value Area 색상 */
  valueAreaColor?: string
  /** 현재가 라인 색상 */
  currentPriceColor?: string
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

/**
 * Value Area 계산 (70% 거래량 영역)
 */
function calculateValueArea(
  priceVolumes: PriceVolume[],
  pocIndex: number,
  targetRatio = 0.7
): { low: number; high: number } {
  if (priceVolumes.length === 0) {
    return { low: 0, high: 0 }
  }

  const totalVolume = priceVolumes.reduce((sum, pv) => sum + pv.volume, 0)
  const targetVolume = totalVolume * targetRatio

  let includedVolume = priceVolumes[pocIndex].volume
  let lowIndex = pocIndex
  let highIndex = pocIndex

  // POC에서 시작하여 양방향으로 확장
  while (includedVolume < targetVolume && (lowIndex > 0 || highIndex < priceVolumes.length - 1)) {
    const nextLowVolume = lowIndex > 0 ? priceVolumes[lowIndex - 1].volume : 0
    const nextHighVolume = highIndex < priceVolumes.length - 1 ? priceVolumes[highIndex + 1].volume : 0

    if (nextLowVolume >= nextHighVolume && lowIndex > 0) {
      lowIndex--
      includedVolume += priceVolumes[lowIndex].volume
    } else if (highIndex < priceVolumes.length - 1) {
      highIndex++
      includedVolume += priceVolumes[highIndex].volume
    } else if (lowIndex > 0) {
      lowIndex--
      includedVolume += priceVolumes[lowIndex].volume
    }
  }

  return {
    low: priceVolumes[lowIndex].price,
    high: priceVolumes[highIndex].price,
  }
}

/**
 * VolumeProfile 컴포넌트
 */
export const VolumeProfile: Component<VolumeProfileProps> = (props) => {
  const width = () => props.width ?? 100
  const showPoc = () => props.showPoc ?? true
  const showValueArea = () => props.showValueArea ?? true
  const barColor = () => props.barColor ?? 'rgba(59, 130, 246, 0.5)'
  const pocColor = () => props.pocColor ?? '#f59e0b'
  const valueAreaColor = () => props.valueAreaColor ?? 'rgba(34, 197, 94, 0.2)'
  const currentPriceColor = () => props.currentPriceColor ?? '#ef4444'

  // 정렬된 데이터 (가격 오름차순)
  const sortedData = createMemo(() =>
    [...props.priceVolumes].sort((a, b) => a.price - b.price)
  )

  // 가격 범위
  const priceRange = createMemo((): [number, number] => {
    if (props.priceRange) return props.priceRange
    const data = sortedData()
    if (data.length === 0) return [0, 0]
    return [data[0].price, data[data.length - 1].price]
  })

  // 최대 거래량 (막대 너비 정규화용)
  const maxVolume = createMemo(() =>
    Math.max(...props.priceVolumes.map((pv) => pv.volume), 1)
  )

  // POC (Point of Control) - 최대 거래량 가격대
  const pocData = createMemo(() => {
    const data = sortedData()
    if (data.length === 0) return null

    let maxVol = 0
    let pocIndex = 0
    data.forEach((pv, i) => {
      if (pv.volume > maxVol) {
        maxVol = pv.volume
        pocIndex = i
      }
    })

    return {
      price: data[pocIndex].price,
      index: pocIndex,
    }
  })

  // Value Area (70% 거래량 영역)
  const valueArea = createMemo(() => {
    const poc = pocData()
    if (!poc) return null
    return calculateValueArea(sortedData(), poc.index)
  })

  // 가격을 Y 좌표로 변환
  const priceToY = (price: number): number => {
    const [minPrice, maxPrice] = priceRange()
    if (maxPrice === minPrice) return props.chartHeight / 2
    // Y축은 위에서 아래로 (높은 가격이 위)
    return props.chartHeight - ((price - minPrice) / (maxPrice - minPrice)) * props.chartHeight
  }

  // 볼륨을 너비로 변환
  const volumeToWidth = (volume: number): number => {
    return (volume / maxVolume()) * width()
  }

  // 막대 높이 계산
  const barHeight = createMemo(() => {
    const data = sortedData()
    if (data.length < 2) return 10
    // 가격 간격 기반 막대 높이
    const priceStep = data.length > 1 ? data[1].price - data[0].price : 1
    const [minPrice, maxPrice] = priceRange()
    const priceRangeValue = maxPrice - minPrice || 1
    return Math.max(2, (priceStep / priceRangeValue) * props.chartHeight * 0.9)
  })

  return (
    <div
      class={`relative ${props.class || ''}`}
      style={{
        width: `${width()}px`,
        height: `${props.chartHeight}px`,
        ...props.style,
      }}
    >
      {/* Value Area 배경 */}
      <Show when={showValueArea() && valueArea()}>
        {(va) => {
          const yHigh = priceToY(va().high)
          const yLow = priceToY(va().low)
          return (
            <div
              class="absolute left-0 right-0"
              style={{
                top: `${yHigh}px`,
                height: `${yLow - yHigh}px`,
                'background-color': valueAreaColor(),
              }}
              title={`Value Area: ${va().low.toLocaleString()} ~ ${va().high.toLocaleString()}`}
            />
          )
        }}
      </Show>

      {/* 볼륨 막대들 */}
      <For each={sortedData()}>
        {(pv) => {
          const y = priceToY(pv.price)
          const barW = volumeToWidth(pv.volume)
          const isPoc = pocData()?.price === pv.price

          return (
            <div
              class="absolute transition-all hover:opacity-80"
              style={{
                left: '0',
                top: `${y - barHeight() / 2}px`,
                width: `${barW}px`,
                height: `${barHeight()}px`,
                'background-color': isPoc && showPoc() ? pocColor() : barColor(),
                'border-radius': '0 2px 2px 0',
              }}
              title={`${pv.price.toLocaleString()}: ${pv.volume.toLocaleString()}`}
            />
          )
        }}
      </For>

      {/* 현재가 라인 */}
      <div
        class="absolute left-0 right-0 h-0.5"
        style={{
          top: `${priceToY(props.currentPrice)}px`,
          'background-color': currentPriceColor(),
        }}
      >
        <div
          class="absolute right-0 -top-2 px-1 text-xs font-mono rounded"
          style={{
            'background-color': currentPriceColor(),
            color: 'white',
          }}
        >
          {props.currentPrice.toLocaleString()}
        </div>
      </div>

      {/* POC 라벨 */}
      <Show when={showPoc() && pocData()}>
        {(poc) => (
          <div
            class="absolute left-0 text-xs font-medium px-1 rounded"
            style={{
              top: `${priceToY(poc().price) - 8}px`,
              'background-color': pocColor(),
              color: '#1f2937',
            }}
          >
            POC
          </div>
        )}
      </Show>
    </div>
  )
}

/**
 * 볼륨 프로파일 범례 컴포넌트
 */
export interface VolumeProfileLegendProps {
  pocPrice?: number
  valueAreaLow?: number
  valueAreaHigh?: number
  class?: string
}

export const VolumeProfileLegend: Component<VolumeProfileLegendProps> = (props) => {
  return (
    <div class={`flex items-center gap-4 text-xs ${props.class || ''}`}>
      <div class="flex items-center gap-1">
        <div class="w-3 h-3 rounded" style={{ 'background-color': 'rgba(59, 130, 246, 0.5)' }} />
        <span class="text-gray-400">거래량</span>
      </div>
      <Show when={props.pocPrice !== undefined}>
        <div class="flex items-center gap-1">
          <div class="w-3 h-3 rounded" style={{ 'background-color': '#f59e0b' }} />
          <span class="text-gray-400">POC: {props.pocPrice?.toLocaleString()}</span>
        </div>
      </Show>
      <Show when={props.valueAreaLow !== undefined && props.valueAreaHigh !== undefined}>
        <div class="flex items-center gap-1">
          <div class="w-3 h-3 rounded" style={{ 'background-color': 'rgba(34, 197, 94, 0.3)' }} />
          <span class="text-gray-400">
            VA: {props.valueAreaLow?.toLocaleString()} ~ {props.valueAreaHigh?.toLocaleString()}
          </span>
        </div>
      </Show>
    </div>
  )
}

export default VolumeProfile
