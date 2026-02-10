/**
 * 통합 차트 컴포넌트 (BaseChart)
 *
 * 모든 lightweight-charts 기반 차트의 기반이 되는 통합 컴포넌트입니다.
 * 단일 API로 다양한 차트 타입(area, line, histogram, candlestick)을 지원합니다.
 *
 * 주요 특징:
 * - 차트 타입별 시리즈 자동 생성
 * - 데이터 자동 정렬 및 변환
 * - 리사이즈 자동 처리
 * - 차트 간 동기화 지원
 * - 로딩/빈 상태 자동 처리
 *
 * @example
 * ```tsx
 * // Area 차트 (Equity Curve)
 * <BaseChart
 *   type="area"
 *   data={equityData()}
 *   height={300}
 *   title="포트폴리오 가치"
 * />
 *
 * // Histogram 차트 (Drawdown)
 * <BaseChart
 *   type="histogram"
 *   data={drawdownData()}
 *   height={150}
 *   title="드로다운"
 *   colorByValue
 * />
 *
 * // 차트 동기화
 * const { syncState, handleRangeChange } = useChartSync();
 *
 * <BaseChart type="area" data={equity} syncState={syncState} onVisibleRangeChange={handleRangeChange} />
 * <BaseChart type="histogram" data={drawdown} syncState={syncState} onVisibleRangeChange={handleRangeChange} />
 * ```
 */

import { type Component, Show, createEffect, on, createMemo } from 'solid-js';
import type { IChartApi, ISeriesApi, SeriesType, Time } from 'lightweight-charts';
import { AreaSeries, LineSeries, HistogramSeries, CandlestickSeries } from 'lightweight-charts';
import { useLightweightChart } from '../../hooks/useLightweightChart';
import {
  type ChartColors,
  type ChartSyncState,
  sortByTime,
  applyValueColors,
  mergeChartColors,
  createAreaSeriesOptions,
  createLineSeriesOptions,
  createHistogramSeriesOptions,
  createCandlestickSeriesOptions,
  isValidChartData,
} from '../../utils/chartUtils';
import { Spinner } from '../ui/Loading';

// ==================== 타입 정의 ====================

/** 지원하는 차트 타입 */
export type ChartType = 'area' | 'line' | 'histogram' | 'candlestick';

/** 기본 데이터 포인트 */
export interface BaseDataPoint {
  time: Time | string | number;
  value?: number;
  // OHLCV (candlestick용)
  open?: number;
  high?: number;
  low?: number;
  close?: number;
  volume?: number;
  // 색상 오버라이드
  color?: string;
}

/** BaseChart Props */
export interface BaseChartProps<T extends BaseDataPoint = BaseDataPoint> {
  /** 차트 타입 */
  type: ChartType;
  /** 차트 데이터 */
  data: T[] | undefined;
  /** 차트 높이 (픽셀) */
  height?: number;
  /** 차트 제목 (선택사항) */
  title?: string;
  /** 커스텀 색상 */
  colors?: Partial<ChartColors>;
  /** 로딩 상태 */
  loading?: boolean;
  /** 빈 데이터 메시지 */
  emptyMessage?: string;
  /** 값에 따라 색상 적용 (histogram에서 양수/음수 구분) */
  colorByValue?: boolean;
  /** 차트 동기화 상태 */
  syncState?: () => ChartSyncState | null;
  /** 시간 범위 변경 콜백 */
  onVisibleRangeChange?: (state: ChartSyncState) => void;
  /** 크로스헤어 이동 콜백 */
  onCrosshairMove?: (state: ChartSyncState) => void;
  /** 동기화 모드 ('range' | 'crosshair' | 'full') */
  syncMode?: 'range' | 'crosshair' | 'full';
  /** 시리즈 라인 색상 (line/area 타입에서 사용) */
  lineColor?: string;
  /** 라인 굵기 */
  lineWidth?: 1 | 2 | 3 | 4;
  /** 데이터 포인트 클릭 콜백 */
  onDataPointClick?: (point: T) => void;
  /** 차트 준비 완료 콜백 (고급 커스터마이징용) */
  onChartReady?: (chart: IChartApi, series: ISeriesApi<SeriesType>) => void;
  /** 크로스헤어 모드 (0: hidden, 1: normal, 2: magnet) */
  crosshairMode?: number;
  /** 오른쪽 가격 축 설정 */
  rightPriceScale?: {
    visible?: boolean;
    scaleMargins?: { top: number; bottom: number };
  };
  /** 시간 축 설정 */
  timeScale?: {
    visible?: boolean;
    timeVisible?: boolean;
  };
  /** 가격 포맷 ('price' | 'percent' | 'volume') */
  priceFormat?: 'price' | 'percent' | 'volume';
  /** 추가 CSS 클래스 */
  class?: string;
}

// ==================== 컴포넌트 ====================

/**
 * 통합 차트 컴포넌트
 */
export const BaseChart: Component<BaseChartProps> = (props) => {
  const height = () => props.height || 300;
  const colors = createMemo(() => mergeChartColors(props.colors));

  // 시리즈 참조 저장
  let seriesRef: ISeriesApi<SeriesType> | null = null;

  // 차트 훅 사용
  // 주의: props를 동적으로 접근하기 위해 콜백을 래핑합니다
  // solid-js에서 훅에 전달되는 값은 생성 시점에 캡처되므로,
  // 최신 props에 접근하려면 함수로 래핑해야 합니다
  const { setContainerRef, fitContent } = useLightweightChart({
    height: height(),
    colors: props.colors,
    crosshairMode: props.crosshairMode,
    rightPriceScale: props.rightPriceScale,
    timeScale: props.timeScale,
    syncState: props.syncState,
    syncMode: props.syncMode || 'full',
    // 콜백을 래핑하여 최신 props에 접근
    onVisibleRangeChange: (state) => {
      props.onVisibleRangeChange?.(state);
    },
    onCrosshairMove: (state) => {
      props.onCrosshairMove?.(state);
    },
    onReady: (chartInstance, mergedColors) => {
      // 차트 타입에 따라 시리즈 생성
      const series = createSeriesForType(chartInstance, props.type, mergedColors, props);
      seriesRef = series;

      // 초기 데이터 설정
      if (isValidChartData(props.data)) {
        const processedData = processData(props.data, props.type, mergedColors, props.colorByValue);
        series.setData(processedData as Parameters<typeof series.setData>[0]);
        chartInstance.timeScale().fitContent();
      }

      // 사용자 콜백 호출
      props.onChartReady?.(chartInstance, series);
    },
  });

  // 데이터 변경 시 업데이트
  createEffect(
    on(
      () => props.data,
      (data) => {
        if (!seriesRef || !isValidChartData(data)) return;
        const processedData = processData(data, props.type, colors(), props.colorByValue);
        seriesRef.setData(processedData as Parameters<typeof seriesRef.setData>[0]);
        fitContent();
      }
    )
  );

  return (
    <div class={`w-full ${props.class || ''}`}>
      {/* 제목 */}
      <Show when={props.title}>
        <h3 class="text-sm font-medium text-gray-300 mb-2">{props.title}</h3>
      </Show>

      {/* 차트 컨테이너 */}
      <Show
        when={!props.loading && isValidChartData(props.data)}
        fallback={
          <div
            class="flex items-center justify-center bg-gray-800/30 rounded-lg"
            style={{ height: `${height()}px` }}
          >
            <Show when={props.loading} fallback={<span class="text-gray-500">{props.emptyMessage || '데이터 없음'}</span>}>
              <Spinner size="md" />
              <span class="ml-2 text-gray-400">로딩 중...</span>
            </Show>
          </div>
        }
      >
        <div ref={setContainerRef} class="w-full" />
      </Show>
    </div>
  );
};

// ==================== 헬퍼 함수 ====================

/**
 * 차트 타입에 따른 시리즈 생성
 */
function createSeriesForType(
  chart: IChartApi,
  type: ChartType,
  colors: ChartColors,
  props: BaseChartProps
): ISeriesApi<SeriesType> {
  switch (type) {
    case 'area':
      return chart.addSeries(AreaSeries, {
        ...createAreaSeriesOptions(colors),
        lineColor: props.lineColor || colors.line,
      });

    case 'line':
      return chart.addSeries(
        LineSeries,
        createLineSeriesOptions(props.lineColor || colors.line, props.lineWidth || 2)
      );

    case 'histogram': {
      const priceFormat = props.priceFormat === 'percent'
        ? { type: 'percent' as const }
        : props.priceFormat === 'volume'
          ? { type: 'volume' as const }
          : { type: 'price' as const, precision: 0, minMove: 1 };
      return chart.addSeries(HistogramSeries, {
        ...createHistogramSeriesOptions(colors),
        color: props.colorByValue ? undefined : colors.negative,
        priceFormat,
      });
    }

    case 'candlestick':
      return chart.addSeries(CandlestickSeries, createCandlestickSeriesOptions(colors));

    default:
      throw new Error(`Unsupported chart type: ${type}`);
  }
}

/**
 * 데이터 전처리 (정렬 + 색상 적용)
 */
function processData<T extends BaseDataPoint>(
  data: T[],
  type: ChartType,
  colors: ChartColors,
  colorByValue?: boolean
): unknown[] {
  // 시간순 정렬
  const sorted = sortByTime(data);

  // histogram + colorByValue: 양수/음수 색상 적용
  if (type === 'histogram' && colorByValue) {
    return applyValueColors(
      sorted.map((d) => ({ time: d.time as Time, value: d.value || 0 })),
      colors.positive,
      colors.negative
    );
  }

  // candlestick: OHLC 형식 변환
  if (type === 'candlestick') {
    return sorted.map((d) => ({
      time: d.time,
      open: d.open || 0,
      high: d.high || 0,
      low: d.low || 0,
      close: d.close || 0,
    }));
  }

  // area/line: value만 추출
  return sorted.map((d) => ({
    time: d.time,
    value: d.value || d.close || 0,
  }));
}

// ==================== 특화 컴포넌트 (편의성) ====================

/**
 * Area 차트 (Equity Curve 용)
 */
export const AreaChart: Component<Omit<BaseChartProps, 'type'>> = (props) => (
  <BaseChart {...props} type="area" />
);

/**
 * Line 차트
 */
export const LineChart: Component<Omit<BaseChartProps, 'type'>> = (props) => (
  <BaseChart {...props} type="line" />
);

/**
 * Histogram 차트 (P&L 용 - 양수/음수 색상 구분)
 */
export const HistogramChart: Component<Omit<BaseChartProps, 'type'>> = (props) => (
  <BaseChart {...props} type="histogram" colorByValue />
);

/**
 * Drawdown Histogram 차트 (퍼센트 포맷)
 */
export const DrawdownHistogram: Component<Omit<BaseChartProps, 'type' | 'priceFormat'>> = (props) => (
  <BaseChart {...props} type="histogram" priceFormat="percent" />
);

/**
 * Candlestick 차트
 */
export const CandlestickChart: Component<Omit<BaseChartProps, 'type'>> = (props) => (
  <BaseChart {...props} type="candlestick" />
);

export default BaseChart;
