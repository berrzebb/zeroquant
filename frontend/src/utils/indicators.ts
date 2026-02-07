/**
 * Technical Indicator Calculation Utilities
 *
 * This module provides pure functions for calculating various technical indicators
 * used in financial chart analysis.
 */

import type { LineDataPoint } from '../components/charts/PriceChart'

// ==================== Types ====================

/** Candle data structure for indicator calculations */
export interface CandleItem {
  time: string
  open: string
  high: string
  low: string
  close: string
  volume: string
}

/** Overlay indicator types (drawn on main chart) */
export type OverlayIndicatorType = 'sma' | 'ema' | 'bb'

/** Sub-chart indicator types (drawn in separate panels) */
export type SubIndicatorType = 'volume' | 'rsi' | 'macd' | 'stochastic' | 'atr' | 'atr_percent' | 'momentum'

/** All indicator types */
export type IndicatorType = OverlayIndicatorType | SubIndicatorType

/** Indicator parameter definitions */
export interface IndicatorParams {
  // Overlay indicators
  sma: { period: number }
  ema: { period: number }
  bb: { period: number; stdDev: number }
  // Sub-chart indicators
  volume: Record<string, never>
  rsi: { period: number }
  macd: { fastPeriod: number; slowPeriod: number; signalPeriod: number }
  stochastic: { kPeriod: number; dPeriod: number }
  atr: { period: number }
  atr_percent: { period: number }
  momentum: { periods: number[] }
}

/** Active indicator configuration */
export interface ActiveIndicator<T extends IndicatorType = IndicatorType> {
  id: string
  type: T
  params: IndicatorParams[T]
  enabled: boolean
  /** Whether this is an overlay indicator (drawn on main chart) */
  isOverlay: boolean
}

/** Indicator metadata (for UI) */
export interface IndicatorMeta {
  type: IndicatorType
  name: string
  description: string
  defaultParams: IndicatorParams[IndicatorType]
  paramLabels: Record<string, string>
  scaleRange?: { min: number; max: number; levels?: number[] }
  color: string
  /** Whether this is an overlay indicator */
  isOverlay: boolean
}

/** Bollinger Bands result */
export interface BollingerBandsResult {
  upper: LineDataPoint[]
  middle: LineDataPoint[]
  lower: LineDataPoint[]
}

/** MACD result */
export interface MACDResult {
  macd: LineDataPoint[]
  signal: LineDataPoint[]
  histogram: LineDataPoint[]
}

/** Stochastic result */
export interface StochasticResult {
  k: LineDataPoint[]
  d: LineDataPoint[]
}

/** Volume result */
export interface VolumeResult {
  data: LineDataPoint[]
  colors: string[]
}

// ==================== Indicator Metadata ====================

/** Indicator metadata definitions */
export const INDICATOR_META: Record<IndicatorType, IndicatorMeta> = {
  // ========== Overlay Indicators (drawn on main chart) ==========
  sma: {
    type: 'sma',
    name: 'SMA',
    description: '단순이동평균 (Simple Moving Average)',
    defaultParams: { period: 20 },
    paramLabels: { period: '기간' },
    color: '#f59e0b',
    isOverlay: true,
  },
  ema: {
    type: 'ema',
    name: 'EMA',
    description: '지수이동평균 (Exponential Moving Average)',
    defaultParams: { period: 12 },
    paramLabels: { period: '기간' },
    color: '#ec4899',
    isOverlay: true,
  },
  bb: {
    type: 'bb',
    name: 'BB',
    description: '볼린저 밴드 (Bollinger Bands)',
    defaultParams: { period: 20, stdDev: 2 },
    paramLabels: { period: '기간', stdDev: '표준편차 배수' },
    color: '#06b6d4',
    isOverlay: true,
  },
  // ========== Sub-chart Indicators (drawn in separate panels) ==========
  volume: {
    type: 'volume',
    name: 'Volume',
    description: '거래량',
    defaultParams: {},
    paramLabels: {},
    color: '#6b7280',
    isOverlay: false,
  },
  rsi: {
    type: 'rsi',
    name: 'RSI',
    description: '상대강도지수 (Relative Strength Index)',
    defaultParams: { period: 14 },
    paramLabels: { period: '기간' },
    scaleRange: { min: 0, max: 100, levels: [30, 70] },
    color: '#8b5cf6',
    isOverlay: false,
  },
  macd: {
    type: 'macd',
    name: 'MACD',
    description: '이동평균수렴확산 (Moving Average Convergence Divergence)',
    defaultParams: { fastPeriod: 12, slowPeriod: 26, signalPeriod: 9 },
    paramLabels: { fastPeriod: '빠른 기간', slowPeriod: '느린 기간', signalPeriod: '시그널 기간' },
    color: '#3b82f6',
    isOverlay: false,
  },
  stochastic: {
    type: 'stochastic',
    name: 'Stochastic',
    description: '스토캐스틱 오실레이터 (%K, %D)',
    defaultParams: { kPeriod: 14, dPeriod: 3 },
    paramLabels: { kPeriod: '%K 기간', dPeriod: '%D 기간' },
    scaleRange: { min: 0, max: 100, levels: [20, 80] },
    color: '#f59e0b',
    isOverlay: false,
  },
  atr: {
    type: 'atr',
    name: 'ATR',
    description: '평균진정범위 (Average True Range)',
    defaultParams: { period: 14 },
    paramLabels: { period: '기간' },
    color: '#ef4444',
    isOverlay: false,
  },
  atr_percent: {
    type: 'atr_percent',
    name: 'ATR%',
    description: '평균진정범위 백분율 (ATR / 종가 × 100)',
    defaultParams: { period: 14 },
    paramLabels: { period: '기간' },
    color: '#ec4899',
    isOverlay: false,
  },
  momentum: {
    type: 'momentum',
    name: 'Momentum',
    description: '다중 기간 모멘텀 점수',
    defaultParams: { periods: [5, 10, 20] },
    paramLabels: { periods: '기간들 (쉼표 구분)' },
    color: '#22c55e',
    isOverlay: false,
  },
}

// ==================== Time Key Utility ====================

/**
 * Returns appropriate time key based on timeframe (Lightweight Charts compatible)
 * @param time - Time string in "YYYY-MM-DD HH:mm:ss" or "YYYY-MM-DD" format
 * @param isDailyOrHigher - Whether the timeframe is daily or higher
 * @returns Date string for daily+ timeframes, Unix timestamp (seconds) for intraday
 */
export function getTimeKey(time: string, isDailyOrHigher: boolean): string | number {
  if (isDailyOrHigher) {
    // Daily or higher: "YYYY-MM-DD" format
    return time.split(' ')[0]
  } else {
    // Intraday: Convert to Unix timestamp (seconds)
    // Parse "2025-10-30 04:00:00" format
    const date = new Date(time.replace(' ', 'T'))
    return Math.floor(date.getTime() / 1000)
  }
}

// ==================== Indicator Calculation Functions ====================

/**
 * Calculate Simple Moving Average (SMA)
 * @param data - Array of candle data
 * @param period - Number of periods for averaging
 * @param isDailyOrHigher - Whether the timeframe is daily or higher
 * @returns Array of SMA data points
 */
export function calculateSMA(data: CandleItem[], period: number, isDailyOrHigher = true): LineDataPoint[] {
  if (data.length < period) return []
  const result: LineDataPoint[] = []
  for (let i = period - 1; i < data.length; i++) {
    let sum = 0
    for (let j = 0; j < period; j++) {
      sum += parseFloat(data[i - j].close)
    }
    result.push({
      time: getTimeKey(data[i].time, isDailyOrHigher),
      value: sum / period,
    })
  }
  return result
}

/**
 * Calculate Exponential Moving Average (EMA)
 * @param data - Array of candle data
 * @param period - Number of periods for averaging
 * @param isDailyOrHigher - Whether the timeframe is daily or higher
 * @returns Array of EMA data points
 */
export function calculateEMA(data: CandleItem[], period: number, isDailyOrHigher = true): LineDataPoint[] {
  if (data.length < period) return []
  const k = 2 / (period + 1)
  const result: LineDataPoint[] = []

  // First EMA starts as SMA
  let sum = 0
  for (let i = 0; i < period; i++) {
    sum += parseFloat(data[i].close)
  }
  let ema = sum / period
  result.push({ time: getTimeKey(data[period - 1].time, isDailyOrHigher), value: ema })

  for (let i = period; i < data.length; i++) {
    ema = parseFloat(data[i].close) * k + ema * (1 - k)
    result.push({ time: getTimeKey(data[i].time, isDailyOrHigher), value: ema })
  }
  return result
}

/**
 * Calculate Bollinger Bands
 * @param data - Array of candle data
 * @param period - Number of periods for the moving average (default: 20)
 * @param stdDev - Standard deviation multiplier (default: 2)
 * @param isDailyOrHigher - Whether the timeframe is daily or higher
 * @returns Object containing upper, middle, and lower band data points
 */
export function calculateBollingerBands(
  data: CandleItem[],
  period: number = 20,
  stdDev: number = 2,
  isDailyOrHigher = true
): BollingerBandsResult {
  if (data.length < period) return { upper: [], middle: [], lower: [] }

  const middle: LineDataPoint[] = []
  const upper: LineDataPoint[] = []
  const lower: LineDataPoint[] = []

  for (let i = period - 1; i < data.length; i++) {
    let sum = 0
    const values: number[] = []
    for (let j = 0; j < period; j++) {
      const val = parseFloat(data[i - j].close)
      sum += val
      values.push(val)
    }
    const mean = sum / period
    const variance = values.reduce((acc, v) => acc + Math.pow(v - mean, 2), 0) / period
    const std = Math.sqrt(variance)

    const time = getTimeKey(data[i].time, isDailyOrHigher)
    middle.push({ time, value: mean })
    upper.push({ time, value: mean + stdDev * std })
    lower.push({ time, value: mean - stdDev * std })
  }

  return { upper, middle, lower }
}

/**
 * Calculate Relative Strength Index (RSI)
 * @param data - Array of candle data
 * @param period - Number of periods for RSI calculation (default: 14)
 * @param isDailyOrHigher - Whether the timeframe is daily or higher
 * @returns Array of RSI data points
 */
export function calculateRSI(data: CandleItem[], period: number = 14, isDailyOrHigher = true): LineDataPoint[] {
  if (data.length < period + 1) return []

  const result: LineDataPoint[] = []
  const gains: number[] = []
  const losses: number[] = []

  // Calculate price changes
  for (let i = 1; i < data.length; i++) {
    const change = parseFloat(data[i].close) - parseFloat(data[i - 1].close)
    gains.push(change > 0 ? change : 0)
    losses.push(change < 0 ? Math.abs(change) : 0)
  }

  // Calculate first average
  let avgGain = gains.slice(0, period).reduce((a, b) => a + b, 0) / period
  let avgLoss = losses.slice(0, period).reduce((a, b) => a + b, 0) / period

  for (let i = period; i < gains.length; i++) {
    if (i === period) {
      // First RSI value
      const rs = avgLoss === 0 ? 100 : avgGain / avgLoss
      const rsi = 100 - (100 / (1 + rs))
      result.push({ time: getTimeKey(data[i + 1].time, isDailyOrHigher), value: rsi })
    } else {
      // Smoothed moving average
      avgGain = (avgGain * (period - 1) + gains[i]) / period
      avgLoss = (avgLoss * (period - 1) + losses[i]) / period
      const rs = avgLoss === 0 ? 100 : avgGain / avgLoss
      const rsi = 100 - (100 / (1 + rs))
      result.push({ time: getTimeKey(data[i + 1].time, isDailyOrHigher), value: rsi })
    }
  }

  return result
}

/**
 * Calculate Moving Average Convergence Divergence (MACD)
 * @param data - Array of candle data
 * @param fastPeriod - Fast EMA period (default: 12)
 * @param slowPeriod - Slow EMA period (default: 26)
 * @param signalPeriod - Signal line EMA period (default: 9)
 * @param isDailyOrHigher - Whether the timeframe is daily or higher
 * @returns Object containing MACD line, signal line, and histogram data points
 */
export function calculateMACD(
  data: CandleItem[],
  fastPeriod: number = 12,
  slowPeriod: number = 26,
  signalPeriod: number = 9,
  isDailyOrHigher = true
): MACDResult {
  if (data.length < slowPeriod + signalPeriod) return { macd: [], signal: [], histogram: [] }

  // EMA calculation helper
  const getEMA = (values: number[], period: number): number[] => {
    const k = 2 / (period + 1)
    const ema: number[] = []
    let sum = 0
    for (let i = 0; i < period; i++) {
      sum += values[i]
    }
    ema.push(sum / period)
    for (let i = period; i < values.length; i++) {
      ema.push(values[i] * k + ema[ema.length - 1] * (1 - k))
    }
    return ema
  }

  const closes = data.map(d => parseFloat(d.close))
  const fastEMA = getEMA(closes, fastPeriod)
  const slowEMA = getEMA(closes, slowPeriod)

  // MACD line (fastEMA - slowEMA)
  const macdLine: number[] = []
  const offset = slowPeriod - fastPeriod
  for (let i = 0; i < slowEMA.length; i++) {
    macdLine.push(fastEMA[i + offset] - slowEMA[i])
  }

  // Signal line (9-day EMA of MACD)
  const signalLine = getEMA(macdLine, signalPeriod)

  // Generate results
  const macd: LineDataPoint[] = []
  const signal: LineDataPoint[] = []
  const histogram: LineDataPoint[] = []

  const startIdx = slowPeriod - 1 + signalPeriod - 1
  for (let i = 0; i < signalLine.length; i++) {
    const dataIdx = startIdx + i
    if (dataIdx >= data.length) break
    const time = getTimeKey(data[dataIdx].time, isDailyOrHigher)
    const macdVal = macdLine[i + signalPeriod - 1]
    const signalVal = signalLine[i]

    macd.push({ time, value: macdVal })
    signal.push({ time, value: signalVal })
    histogram.push({ time, value: macdVal - signalVal })
  }

  return { macd, signal, histogram }
}

/**
 * Generate Volume data
 * @param data - Array of candle data
 * @param isDailyOrHigher - Whether the timeframe is daily or higher
 * @returns Object containing volume data points and corresponding colors
 */
export function calculateVolume(data: CandleItem[], isDailyOrHigher = true): VolumeResult {
  const result: LineDataPoint[] = []
  const colors: string[] = []

  for (const candle of data) {
    const isUp = parseFloat(candle.close) >= parseFloat(candle.open)
    result.push({
      time: getTimeKey(candle.time, isDailyOrHigher),
      value: parseInt(candle.volume),
    })
    colors.push(isUp ? '#22c55e' : '#ef4444')
  }

  return { data: result, colors }
}

/**
 * Calculate Stochastic Oscillator (%K and %D)
 * @param data - Array of candle data
 * @param kPeriod - %K period (default: 14)
 * @param dPeriod - %D period (default: 3)
 * @param isDailyOrHigher - Whether the timeframe is daily or higher
 * @returns Object containing %K and %D data points
 */
export function calculateStochastic(
  data: CandleItem[],
  kPeriod: number = 14,
  dPeriod: number = 3,
  isDailyOrHigher = true
): StochasticResult {
  if (data.length < kPeriod) return { k: [], d: [] }

  const kValues: LineDataPoint[] = []

  // %K calculation: (Current Close - N-period Lowest Low) / (N-period Highest High - N-period Lowest Low) * 100
  for (let i = kPeriod - 1; i < data.length; i++) {
    let lowestLow = Infinity
    let highestHigh = -Infinity

    for (let j = 0; j < kPeriod; j++) {
      const high = parseFloat(data[i - j].high)
      const low = parseFloat(data[i - j].low)
      if (high > highestHigh) highestHigh = high
      if (low < lowestLow) lowestLow = low
    }

    const close = parseFloat(data[i].close)
    const range = highestHigh - lowestLow
    const k = range === 0 ? 50 : ((close - lowestLow) / range) * 100

    kValues.push({
      time: getTimeKey(data[i].time, isDailyOrHigher),
      value: k,
    })
  }

  // %D calculation: dPeriod moving average of %K
  const dValues: LineDataPoint[] = []
  if (kValues.length >= dPeriod) {
    for (let i = dPeriod - 1; i < kValues.length; i++) {
      let sum = 0
      for (let j = 0; j < dPeriod; j++) {
        sum += kValues[i - j].value
      }
      dValues.push({
        time: kValues[i].time,
        value: sum / dPeriod,
      })
    }
  }

  return { k: kValues, d: dValues }
}

/**
 * Calculate Average True Range (ATR)
 * @param data - Array of candle data
 * @param period - Number of periods for ATR calculation (default: 14)
 * @param isDailyOrHigher - Whether the timeframe is daily or higher
 * @returns Array of ATR data points
 */
export function calculateATR(data: CandleItem[], period: number = 14, isDailyOrHigher = true): LineDataPoint[] {
  if (data.length < period + 1) return []

  const trValues: number[] = []

  // True Range calculation
  for (let i = 1; i < data.length; i++) {
    const high = parseFloat(data[i].high)
    const low = parseFloat(data[i].low)
    const prevClose = parseFloat(data[i - 1].close)

    const tr = Math.max(
      high - low,                    // Current high - low
      Math.abs(high - prevClose),    // |Current high - Previous close|
      Math.abs(low - prevClose)      // |Current low - Previous close|
    )
    trValues.push(tr)
  }

  // ATR calculation (first is simple average, then exponential moving average)
  const result: LineDataPoint[] = []
  let atr = trValues.slice(0, period).reduce((a, b) => a + b, 0) / period

  result.push({
    time: getTimeKey(data[period].time, isDailyOrHigher),
    value: atr,
  })

  for (let i = period; i < trValues.length; i++) {
    atr = (atr * (period - 1) + trValues[i]) / period
    result.push({
      time: getTimeKey(data[i + 1].time, isDailyOrHigher),
      value: atr,
    })
  }

  return result
}

/**
 * Calculate ATR Percent (ATR as percentage of closing price)
 * @param data - Array of candle data
 * @param period - Number of periods for ATR calculation (default: 14)
 * @param isDailyOrHigher - Whether the timeframe is daily or higher
 * @returns Array of ATR percent data points
 */
export function calculateATRPercent(data: CandleItem[], period: number = 14, isDailyOrHigher = true): LineDataPoint[] {
  const atrData = calculateATR(data, period, isDailyOrHigher)
  if (atrData.length === 0) return []

  // Find matching closing price for each ATR data point
  const result: LineDataPoint[] = []
  for (const atrPoint of atrData) {
    const candle = data.find(d => getTimeKey(d.time, isDailyOrHigher) === atrPoint.time)
    if (candle) {
      const close = parseFloat(candle.close)
      result.push({
        time: atrPoint.time,
        value: (atrPoint.value / close) * 100,
      })
    }
  }

  return result
}

/**
 * Calculate Momentum Score (multi-period returns sum)
 * @param data - Array of candle data
 * @param periods - Array of periods for momentum calculation (default: [5, 10, 20])
 * @param isDailyOrHigher - Whether the timeframe is daily or higher
 * @returns Array of momentum score data points
 */
export function calculateMomentumScore(
  data: CandleItem[],
  periods: number[] = [5, 10, 20],
  isDailyOrHigher = true
): LineDataPoint[] {
  const maxPeriod = Math.max(...periods)
  if (data.length < maxPeriod + 1) return []

  const result: LineDataPoint[] = []

  for (let i = maxPeriod; i < data.length; i++) {
    let score = 0
    const currentClose = parseFloat(data[i].close)

    for (const period of periods) {
      const pastClose = parseFloat(data[i - period].close)
      const returns = ((currentClose - pastClose) / pastClose) * 100
      score += returns
    }

    // Normalize: divide by number of periods for average return
    result.push({
      time: getTimeKey(data[i].time, isDailyOrHigher),
      value: score / periods.length,
    })
  }

  return result
}
