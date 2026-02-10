/**
 * 차트 데이터 공통 유틸리티
 *
 * Backtest, Simulation, PaperTrading 3개 페이지에서 공통으로 사용하는
 * 캔들 데이터 변환, 볼륨 프로파일 계산, 차트 헬퍼 함수들을 통합한다.
 */
import type { CandlestickDataPoint, PriceVolume, TradeMarker, IndicatorFilters } from '../components/charts'

// API 기본 URL
const API_BASE = '/api/v1'

// ─────────────────────────── 타입 ───────────────────────────

/** API에서 반환하는 원본 캔들 데이터 */
export interface CandleItem {
  time: string
  open: string
  high: string
  low: string
  close: string
  volume: string
}

/** 캔들 데이터 API 응답 */
export interface CandleDataResponse {
  symbol: string
  timeframe: string
  candles: CandleItem[]
  totalCount: number
}

// ─────────────────────────── 날짜 유틸리티 ───────────────────────────

/** 두 날짜 간 일수 계산 (inclusive) */
export function daysBetween(startDate: string, endDate: string): number {
  const start = new Date(startDate)
  const end = new Date(endDate)
  const diffTime = Math.abs(end.getTime() - start.getTime())
  return Math.ceil(diffTime / (1000 * 60 * 60 * 24)) + 1
}

// ─────────────────────────── 데이터 Fetch ───────────────────────────

/** 기간에 해당하는 캔들 데이터를 API에서 조회 */
export async function fetchCandlesForPeriod(
  symbol: string,
  startDate: string,
  endDate: string
): Promise<CandleDataResponse | null> {
  try {
    const days = daysBetween(startDate, endDate)
    const limit = Math.ceil(days * 1.2)

    const params = new URLSearchParams({
      timeframe: '1d',
      limit: limit.toString(),
      sortBy: 'time',
      sortOrder: 'asc',
    })
    const res = await fetch(`${API_BASE}/dataset/${encodeURIComponent(symbol)}?${params}`)
    if (!res.ok) return null
    const data: CandleDataResponse = await res.json()

    // 기간에 해당하는 캔들만 필터링
    const filtered = data.candles.filter(c => {
      const date = c.time.split(' ')[0]
      return date >= startDate && date <= endDate
    })

    return { ...data, candles: filtered, totalCount: filtered.length }
  } catch {
    return null
  }
}

// ─────────────────────────── 데이터 변환 ───────────────────────────

/** CandleItem[] → CandlestickDataPoint[] (차트 표시용) */
export function convertCandlesToChartData(candles: CandleItem[]): CandlestickDataPoint[] {
  const uniqueMap = new Map<string, CandlestickDataPoint>()

  candles.forEach(c => {
    const timeKey = c.time.split(' ')[0]
    uniqueMap.set(timeKey, {
      time: timeKey,
      open: parseFloat(c.open),
      high: parseFloat(c.high),
      low: parseFloat(c.low),
      close: parseFloat(c.close),
    })
  })

  return Array.from(uniqueMap.values()).sort((a, b) =>
    (a.time as string).localeCompare(b.time as string)
  )
}

// ─────────────────────────── 볼륨 프로파일 ───────────────────────────

/** CandleItem[]에서 볼륨 프로파일 계산 (거래량 가중치 적용) */
export function calculateVolumeProfileFromCandles(candles: CandleItem[], bucketCount = 25): PriceVolume[] {
  if (candles.length === 0) return []

  let minPrice = Infinity
  let maxPrice = -Infinity
  candles.forEach(c => {
    const low = parseFloat(c.low)
    const high = parseFloat(c.high)
    if (low < minPrice) minPrice = low
    if (high > maxPrice) maxPrice = high
  })
  if (minPrice === maxPrice) return []

  const priceStep = (maxPrice - minPrice) / bucketCount
  const buckets = new Map<number, number>()

  candles.forEach(c => {
    const low = parseFloat(c.low)
    const high = parseFloat(c.high)
    const volume = parseFloat(c.volume)
    const candleRange = high - low || 1
    for (let i = 0; i < bucketCount; i++) {
      const bucketLow = minPrice + i * priceStep
      const bucketHigh = bucketLow + priceStep
      const bucketMid = (bucketLow + bucketHigh) / 2
      if (high >= bucketLow && low <= bucketHigh) {
        const overlapLow = Math.max(low, bucketLow)
        const overlapHigh = Math.min(high, bucketHigh)
        const overlapRatio = (overlapHigh - overlapLow) / candleRange
        buckets.set(bucketMid, (buckets.get(bucketMid) || 0) + volume * overlapRatio)
      }
    }
  })

  const result: PriceVolume[] = []
  buckets.forEach((volume, price) => {
    result.push({ price, volume })
  })
  return result.sort((a, b) => a.price - b.price)
}

/** CandlestickDataPoint[]에서 볼륨 프로파일 계산 (volume 없이 비율만) */
export function calculateVolumeProfileFromChart(candles: CandlestickDataPoint[], bucketCount = 25): PriceVolume[] {
  if (candles.length === 0) return []

  let minPrice = Infinity
  let maxPrice = -Infinity
  candles.forEach(c => {
    if (c.low < minPrice) minPrice = c.low
    if (c.high > maxPrice) maxPrice = c.high
  })
  if (minPrice === maxPrice) return []

  const priceStep = (maxPrice - minPrice) / bucketCount
  const buckets = new Map<number, number>()

  candles.forEach(c => {
    const candleRange = c.high - c.low || 1
    for (let i = 0; i < bucketCount; i++) {
      const bucketLow = minPrice + i * priceStep
      const bucketHigh = bucketLow + priceStep
      const bucketMid = (bucketLow + bucketHigh) / 2
      if (c.high >= bucketLow && c.low <= bucketHigh) {
        const overlapLow = Math.max(c.low, bucketLow)
        const overlapHigh = Math.min(c.high, bucketHigh)
        const overlapRatio = (overlapHigh - overlapLow) / candleRange
        buckets.set(bucketMid, (buckets.get(bucketMid) || 0) + overlapRatio)
      }
    }
  })

  const result: PriceVolume[] = []
  buckets.forEach((volume, price) => {
    result.push({ price, volume })
  })
  return result.sort((a, b) => a.price - b.price)
}

// ─────────────────────────── 차트 헬퍼 ───────────────────────────

/** 캔들 데이터에서 현재가(마지막 종가) 추출 */
export function getCurrentPrice(data: CandlestickDataPoint[]): number {
  if (data.length === 0) return 0
  return data[data.length - 1].close
}

/** 캔들 데이터에서 가격 범위 [min, max] 추출 */
export function getChartPriceRange(data: CandlestickDataPoint[]): [number, number] {
  if (data.length === 0) return [0, 0]
  let min = Infinity
  let max = -Infinity
  data.forEach(c => {
    if (c.low < min) min = c.low
    if (c.high > max) max = c.high
  })
  return [min, max]
}

/** 신호 필터를 기반으로 마커 필터링 */
export function filterMarkersBySignal(
  markers: (TradeMarker & { signalType?: string; side?: string })[],
  filters: IndicatorFilters
): (TradeMarker & { signalType?: string; side?: string })[] {
  if (filters.signal_types.length === 0) return markers

  return markers.filter(marker => {
    const signalType = marker.signalType
    const side = marker.side

    // buy/sell 필터 (side 기반)
    if (filters.signal_types.includes('buy') && side === 'buy') return true
    if (filters.signal_types.includes('sell') && side === 'sell') return true

    // 상세 signal_type 필터
    if (signalType) {
      const types = filters.signal_types as string[]
      if (types.includes('entry') && signalType === 'entry') return true
      if (types.includes('exit') && signalType === 'exit') return true
      if (types.includes('add_to_position') && signalType === 'add_to_position') return true
      if (types.includes('reduce_position') && signalType === 'reduce_position') return true
      if (types.includes('scale') && signalType === 'scale') return true
      if (filters.signal_types.includes('alert') && signalType === 'alert') return true
    }

    return false
  })
}
