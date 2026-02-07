/**
 * 차트 공통 유틸리티
 *
 * lightweight-charts 및 ECharts에서 사용되는 공통 함수들을 제공합니다.
 * 이 파일은 차트 컴포넌트들의 코드 중복을 줄이기 위해 생성되었습니다.
 */

import type { Time } from 'lightweight-charts';

// ==================== 타입 정의 ====================

/** 시간 기반 데이터 포인트의 기본 인터페이스 */
export interface TimeBasedDataPoint {
  time: Time | string | number;
  [key: string]: unknown;
}

/** 차트 컬러 설정 */
export interface ChartColors {
  /** 배경색 */
  background: string;
  /** 텍스트 색상 */
  text: string;
  /** 그리드 색상 */
  grid: string;
  /** 상승 색상 (양수) */
  positive: string;
  /** 하락 색상 (음수) */
  negative: string;
  /** 메인 라인 색상 */
  line: string;
  /** 영역 채우기 색상 (상단) */
  areaTop: string;
  /** 영역 채우기 색상 (하단) */
  areaBottom: string;
  /** 볼륨 색상 */
  volume: string;
  /** 크로스헤어 색상 */
  crosshair: string;
}

/** 크로스헤어 위치 정보 */
export interface CrosshairPosition {
  /** 시간 값 (X축) */
  time: Time | null;
  /** 가격/값 (Y축) */
  price: number | null;
  /** 논리적 인덱스 (시간축 상의 위치) */
  logical: number | null;
  /** 화면 X 좌표 */
  x: number | null;
  /** 화면 Y 좌표 */
  y: number | null;
}

/** 차트 동기화 상태 (확장된 버전) */
export interface ChartSyncState {
  /** 보이는 시간 범위 (줌/팬 동기화) */
  range: { from: number; to: number } | null;
  /** 크로스헤어 위치 (크로스헤어 동기화) */
  crosshair: CrosshairPosition | null;
  /** 동기화 발생 소스 차트 ID */
  sourceId: string;
  /** 동기화 타입 ('range' | 'crosshair' | 'both') */
  syncType: 'range' | 'crosshair' | 'both';
}

// ==================== 기본 색상 ====================

/**
 * 다크 테마 기본 차트 색상
 *
 * 모든 차트에서 일관된 스타일을 유지하기 위한 기본값입니다.
 */
export const DEFAULT_CHART_COLORS: ChartColors = {
  background: '#1f2937', // gray-800 - 다크 테마 배경
  text: '#d1d5db',
  grid: '#374151',
  positive: '#22c55e',
  negative: '#ef4444',
  line: '#3b82f6',
  areaTop: 'rgba(59, 130, 246, 0.4)',
  areaBottom: 'rgba(59, 130, 246, 0.0)',
  volume: 'rgba(59, 130, 246, 0.5)',
  crosshair: '#9ca3af',
};

/**
 * 라이트 테마 차트 색상
 */
export const LIGHT_CHART_COLORS: ChartColors = {
  background: '#ffffff',
  text: '#374151',
  grid: '#e5e7eb',
  positive: '#16a34a',
  negative: '#dc2626',
  line: '#2563eb',
  areaTop: 'rgba(37, 99, 235, 0.3)',
  areaBottom: 'rgba(37, 99, 235, 0.0)',
  volume: 'rgba(37, 99, 235, 0.4)',
  crosshair: '#6b7280',
};

// ==================== 유틸리티 함수 ====================

/**
 * 시간 기반 데이터를 시간순으로 정렬
 *
 * lightweight-charts는 데이터가 시간순으로 정렬되어 있어야 합니다.
 * 이 함수는 string, number, Time 타입 모두를 처리합니다.
 *
 * @example
 * ```ts
 * const sorted = sortByTime(equityData);
 * equitySeries.setData(sorted);
 * ```
 */
export function sortByTime<T extends TimeBasedDataPoint>(data: T[]): T[] {
  if (!data || data.length === 0) return [];

  // 정렬
  const sorted = [...data].sort((a, b) => {
    const timeA = normalizeTime(a.time);
    const timeB = normalizeTime(b.time);
    return timeA.localeCompare(timeB);
  });

  // 중복 타임스탬프 제거 (LightweightCharts는 동일 시간 데이터 허용 안 함)
  // 중복 시 마지막 데이터 유지 (최신 데이터 우선)
  const seen = new Set<string>();
  const deduped: T[] = [];

  // 역순으로 순회하여 마지막 항목 우선
  for (let i = sorted.length - 1; i >= 0; i--) {
    const normalizedTime = normalizeTime(sorted[i].time);
    if (!seen.has(normalizedTime)) {
      seen.add(normalizedTime);
      deduped.unshift(sorted[i]);
    }
  }

  return deduped;
}

/**
 * 시간 값을 비교 가능한 문자열로 정규화
 */
export function normalizeTime(time: Time | string | number): string {
  if (typeof time === 'string') return time;
  if (typeof time === 'number') return time.toString();
  // Time 객체인 경우 (year, month, day)
  if (typeof time === 'object' && 'year' in time) {
    const { year, month, day } = time as { year: number; month: number; day: number };
    return `${year}-${String(month).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
  }
  return String(time);
}

/**
 * 차트 색상을 기본값과 병합
 *
 * @example
 * ```ts
 * const colors = mergeChartColors({ positive: '#00ff00' });
 * // 나머지는 DEFAULT_CHART_COLORS 사용
 * ```
 */
export function mergeChartColors(custom?: Partial<ChartColors>): ChartColors {
  return { ...DEFAULT_CHART_COLORS, ...custom };
}

/**
 * 값에 따라 양수/음수 색상 적용
 *
 * 히스토그램이나 P&L 차트에서 값에 따른 색상을 적용할 때 사용합니다.
 *
 * @example
 * ```ts
 * const coloredData = applyValueColors(pnlData, colors.positive, colors.negative);
 * ```
 */
export function applyValueColors<T extends { time: Time; value: number }>(
  data: T[],
  positiveColor: string,
  negativeColor: string
): Array<T & { color: string }> {
  return data.map((point) => ({
    ...point,
    color: point.value >= 0 ? positiveColor : negativeColor,
  }));
}

/**
 * 퍼센트 변화에 따른 색상 반환
 *
 * @param change 퍼센트 변화율
 * @param colors 차트 색상 설정
 */
export function getChangeColor(change: number, colors: ChartColors = DEFAULT_CHART_COLORS): string {
  if (change > 0) return colors.positive;
  if (change < 0) return colors.negative;
  return colors.text;
}

/**
 * 점수에 따른 그라데이션 색상 반환 (0-100)
 *
 * @param score 0-100 사이의 점수
 */
export function getScoreColor(score: number): string {
  if (score >= 80) return '#22c55e'; // 녹색 (매우 좋음)
  if (score >= 60) return '#84cc16'; // 라임 (좋음)
  if (score >= 40) return '#eab308'; // 노랑 (보통)
  if (score >= 20) return '#f97316'; // 주황 (나쁨)
  return '#ef4444'; // 빨강 (매우 나쁨)
}

// ==================== 차트 옵션 생성 ====================

/**
 * lightweight-charts 기본 옵션 생성
 *
 * 모든 lightweight-charts 컴포넌트에서 사용하는 공통 옵션을 생성합니다.
 *
 * @example
 * ```ts
 * const options = createChartOptions({
 *   width: containerRef.clientWidth,
 *   height: 300,
 *   colors: mergeChartColors(props.colors),
 * });
 * const chart = createChart(containerRef, options);
 * ```
 */
export function createChartOptions(config: {
  width: number;
  height: number;
  colors?: ChartColors;
  rightPriceScale?: {
    visible?: boolean;
    scaleMargins?: { top: number; bottom: number };
  };
  timeScale?: {
    visible?: boolean;
    timeVisible?: boolean;
    secondsVisible?: boolean;
  };
  crosshair?: {
    mode?: number;
    vertLine?: { visible?: boolean };
    horzLine?: { visible?: boolean };
  };
}) {
  const colors = config.colors || DEFAULT_CHART_COLORS;

  return {
    layout: {
      background: { type: 'solid' as const, color: colors.background },
      textColor: colors.text,
    },
    grid: {
      vertLines: { color: colors.grid },
      horzLines: { color: colors.grid },
    },
    width: config.width,
    height: config.height,
    crosshair: {
      mode: config.crosshair?.mode ?? 1,
      vertLine: {
        visible: config.crosshair?.vertLine?.visible ?? true,
        color: colors.crosshair,
        width: 1 as const,
        style: 2 as const,
      },
      horzLine: {
        visible: config.crosshair?.horzLine?.visible ?? true,
        color: colors.crosshair,
        width: 1 as const,
        style: 2 as const,
      },
    },
    timeScale: {
      borderColor: colors.grid,
      visible: config.timeScale?.visible ?? true,
      timeVisible: config.timeScale?.timeVisible ?? true,
      secondsVisible: config.timeScale?.secondsVisible ?? false,
    },
    rightPriceScale: {
      borderColor: colors.grid,
      visible: config.rightPriceScale?.visible ?? true,
      scaleMargins: config.rightPriceScale?.scaleMargins ?? { top: 0.1, bottom: 0.1 },
    },
    handleScroll: {
      mouseWheel: true,
      pressedMouseMove: true,
      horzTouchDrag: true,
      vertTouchDrag: false,
    },
    handleScale: {
      mouseWheel: true,
      pinch: true,
      axisPressedMouseMove: true,
    },
  };
}

// ==================== 시리즈 옵션 생성 ====================

/**
 * Area 시리즈 옵션 생성 (Equity Curve 등)
 */
export function createAreaSeriesOptions(colors: ChartColors = DEFAULT_CHART_COLORS) {
  return {
    lineColor: colors.line,
    topColor: colors.areaTop,
    bottomColor: colors.areaBottom,
    lineWidth: 2 as const,
    crosshairMarkerVisible: true,
    crosshairMarkerRadius: 4,
    lastValueVisible: true,
    priceLineVisible: false,
  };
}

/**
 * Line 시리즈 옵션 생성
 */
export function createLineSeriesOptions(
  color: string = DEFAULT_CHART_COLORS.line,
  lineWidth: 1 | 2 | 3 | 4 = 2
) {
  return {
    color,
    lineWidth,
    crosshairMarkerVisible: true,
    crosshairMarkerRadius: 4,
    lastValueVisible: true,
    priceLineVisible: false,
  };
}

/**
 * Histogram 시리즈 옵션 생성 (Drawdown, P&L 등)
 */
export function createHistogramSeriesOptions(colors: ChartColors = DEFAULT_CHART_COLORS) {
  return {
    color: colors.negative,
    priceFormat: {
      type: 'percent' as const,
    },
    priceLineVisible: false,
    lastValueVisible: true,
  };
}

/**
 * Candlestick 시리즈 옵션 생성
 */
export function createCandlestickSeriesOptions(colors: ChartColors = DEFAULT_CHART_COLORS) {
  return {
    upColor: colors.positive,
    downColor: colors.negative,
    borderUpColor: colors.positive,
    borderDownColor: colors.negative,
    wickUpColor: colors.positive,
    wickDownColor: colors.negative,
  };
}

/**
 * Volume 시리즈 옵션 생성
 */
export function createVolumeSeriesOptions(colors: ChartColors = DEFAULT_CHART_COLORS) {
  return {
    color: colors.volume,
    priceFormat: {
      type: 'volume' as const,
    },
    priceScaleId: 'volume',
    scaleMargins: {
      top: 0.8,
      bottom: 0,
    },
  };
}

// ==================== 데이터 검증 ====================

/**
 * 차트 데이터가 유효한지 검사
 */
export function isValidChartData<T extends TimeBasedDataPoint>(data: T[] | undefined | null): data is T[] {
  return Array.isArray(data) && data.length > 0;
}

/**
 * OHLCV 데이터가 유효한지 검사
 */
export function isValidOHLCVData(
  data: Array<{ open?: number; high?: number; low?: number; close?: number }> | undefined | null
): boolean {
  if (!Array.isArray(data) || data.length === 0) return false;
  return data.every(
    (d) =>
      typeof d.open === 'number' &&
      typeof d.high === 'number' &&
      typeof d.low === 'number' &&
      typeof d.close === 'number'
  );
}

// ==================== 포맷팅 ====================

/**
 * 차트 툴팁용 숫자 포맷팅
 */
export function formatChartValue(value: number, decimals: number = 2): string {
  if (Math.abs(value) >= 1_000_000_000) {
    return `${(value / 1_000_000_000).toFixed(decimals)}B`;
  }
  if (Math.abs(value) >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(decimals)}M`;
  }
  if (Math.abs(value) >= 1_000) {
    return `${(value / 1_000).toFixed(decimals)}K`;
  }
  return value.toFixed(decimals);
}

/**
 * 퍼센트 포맷팅
 */
export function formatChartPercent(value: number, decimals: number = 2): string {
  const sign = value >= 0 ? '+' : '';
  return `${sign}${value.toFixed(decimals)}%`;
}

// ==================== 고유 ID 생성 ====================

let chartIdCounter = 0;

/**
 * 차트 인스턴스용 고유 ID 생성
 *
 * 차트 동기화 시 소스 식별에 사용됩니다.
 */
export function generateChartId(prefix: string = 'chart'): string {
  chartIdCounter += 1;
  return `${prefix}-${chartIdCounter}-${Date.now().toString(36)}`;
}
