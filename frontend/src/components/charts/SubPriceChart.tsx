/**
 * SubPriceChart - 별도 패널 지표 차트 컴포넌트.
 *
 * RSI, MACD, Stochastic 등 가격과 다른 스케일을 가진 지표를 별도 패널로 표시합니다.
 * 공통 유틸리티와 useLightweightChart 훅을 사용하여 코드 중복을 최소화합니다.
 *
 * @example
 * ```tsx
 * // 기본 사용
 * <SubPriceChart
 *   indicator={{
 *     id: 'rsi',
 *     type: 'rsi',
 *     name: 'RSI (14)',
 *     series: [{ name: 'RSI', data: rsiData, color: '#8b5cf6', seriesType: 'line' }],
 *     scaleRange: { min: 0, max: 100, levels: [30, 70] },
 *   }}
 * />
 *
 * // 동기화 사용
 * const sync = useChartSync();
 * <SubPriceChart indicator={rsiIndicator} {...sync.bindChart('rsi')} />
 * ```
 */

import { createEffect, on, For, Show } from 'solid-js';
import { LineSeries, HistogramSeries } from 'lightweight-charts';
import type { IChartApi, ISeriesApi, LineData, HistogramData, Time } from 'lightweight-charts';
import { useLightweightChart } from '../../hooks/useLightweightChart';
import {
  sortByTime,
  type ChartColors,
  type ChartSyncState,
} from '../../utils/chartUtils';
import type { LineDataPoint } from './PriceChart';

// ==================== 타입 정의 ====================

/** 지표 시리즈 데이터 */
export interface IndicatorSeriesData {
  name: string;
  data: LineDataPoint[];
  color: string;
  seriesType: 'line' | 'bar' | 'area';
  lineWidth?: number;
}

/** 별도 패널 지표 데이터 */
export interface SeparateIndicatorData {
  id: string;
  type: string;
  name: string;
  series: IndicatorSeriesData[];
  scaleRange?: { min: number; max: number; levels?: number[] };
}

export interface SubPriceChartProps {
  /** 지표 데이터 */
  indicator: SeparateIndicatorData;
  /** 차트 높이 */
  height?: number;
  /** 커스텀 색상 */
  colors?: {
    background?: string;
    text?: string;
    grid?: string;
  };
  /** 차트 ID (동기화용) */
  chartId?: string;
  /** 동기화 상태 (다른 차트에서 전달) */
  syncState?: () => ChartSyncState | null;
  /** 시간 범위 변경 콜백 (다른 차트에 전파) */
  onVisibleRangeChange?: (state: ChartSyncState) => void;
  /** 크로스헤어 이동 콜백 (다른 차트에 전파) */
  onCrosshairMove?: (state: ChartSyncState) => void;
  /** 동기화 모드 ('range' | 'crosshair' | 'full') */
  syncMode?: 'range' | 'crosshair' | 'full';
}

// ==================== 컴포넌트 ====================

export function SubPriceChart(props: SubPriceChartProps) {
  // 시리즈 맵
  const seriesMap = new Map<string, ISeriesApi<'Line'> | ISeriesApi<'Histogram'>>();

  // 차트 인스턴스 참조
  let chartInstance: IChartApi | undefined;

  // 색상 병합
  const getChartColors = (): Partial<ChartColors> => ({
    background: props.colors?.background,
    text: props.colors?.text,
    grid: props.colors?.grid,
  });

  // 지표 시리즈 생성
  const createIndicatorSeries = (chartApi: IChartApi) => {
    const indicator = props.indicator;
    if (!indicator.series) return;

    for (const series of indicator.series) {
      const lineWidth = (
        series.lineWidth && series.lineWidth >= 1 && series.lineWidth <= 4
          ? series.lineWidth
          : 2
      ) as 1 | 2 | 3 | 4;

      let chartSeries: ISeriesApi<'Line'> | ISeriesApi<'Histogram'>;

      if (series.seriesType === 'bar') {
        // 히스토그램 (MACD 히스토그램 등)
        chartSeries = chartApi.addSeries(HistogramSeries, {
          color: series.color,
          priceFormat: {
            type: 'price',
            precision: 4,
            minMove: 0.0001,
          },
        });
      } else {
        // 라인 (RSI, MACD 라인, Stochastic 등)
        chartSeries = chartApi.addSeries(LineSeries, {
          color: series.color,
          lineWidth,
          lastValueVisible: true,
          priceLineVisible: false,
        });
      }

      // 데이터 설정
      if (series.data && series.data.length > 0) {
        const sortedData = sortByTime(series.data);

        if (series.seriesType === 'bar') {
          // 히스토그램 데이터 - 양수/음수에 따라 색상 변경
          const histogramData = sortedData.map((d) => ({
            time: d.time as Time,
            value: d.value,
            color: d.value >= 0 ? '#22c55e' : '#ef4444',
          }));
          chartSeries.setData(histogramData as HistogramData[]);
        } else {
          chartSeries.setData(sortedData as LineData[]);
        }
      }

      seriesMap.set(series.name, chartSeries);
    }

    // 기준선 추가 (RSI 30/70, Stochastic 20/80 등)
    if (indicator.scaleRange?.levels) {
      const firstSeries = indicator.series[0];
      if (firstSeries?.data?.length > 0) {
        const sortedTimeData = sortByTime(firstSeries.data);
        for (const level of indicator.scaleRange.levels) {
          const levelSeries = chartApi.addSeries(LineSeries, {
            color: '#6b7280',
            lineWidth: 1,
            lineStyle: 2, // Dashed
            lastValueVisible: false,
            priceLineVisible: false,
          });

          const levelData = sortedTimeData.map((d) => ({
            time: d.time as Time,
            value: level,
          }));
          levelSeries.setData(levelData as LineData[]);
        }
      }
    }

    // 차트 스케일 조정
    requestAnimationFrame(() => {
      chartApi.timeScale().fitContent();
    });
  };

  // 통합 차트 훅 사용
  const { setContainerRef, chart, chartId, colors } = useLightweightChart({
    height: props.height || 120,
    colors: getChartColors(),
    syncState: props.syncState,
    syncMode: props.syncMode || 'full',
    onVisibleRangeChange: props.onVisibleRangeChange,
    onCrosshairMove: props.onCrosshairMove,
    rightPriceScale: {
      scaleMargins: { top: 0.1, bottom: 0.1 },
    },
    timeScale: {
      timeVisible: true,
      secondsVisible: false,
    },
    onReady: (chartApi, mergedColors) => {
      chartInstance = chartApi;
      createIndicatorSeries(chartApi);
    },
  });

  // 데이터 변경 시 업데이트
  createEffect(
    on(
      () => props.indicator,
      (indicator) => {
        const currentChart = chartInstance;
        if (!currentChart || !indicator) return;

        // 기존 시리즈 제거
        for (const series of seriesMap.values()) {
          try {
            currentChart.removeSeries(series);
          } catch {
            // 이미 제거된 시리즈 무시
          }
        }
        seriesMap.clear();

        // 새 시리즈 생성
        createIndicatorSeries(currentChart);
      }
    )
  );

  return (
    <div class="mt-2">
      {/* 지표 이름 레이블 */}
      <div class="flex items-center gap-2 mb-1 px-2">
        <span class="text-xs font-medium text-[var(--color-text-muted)]">
          {props.indicator.name}
        </span>
        <div class="flex gap-2">
          <For each={props.indicator.series}>
            {(series) => (
              <span
                class="inline-flex items-center gap-1 text-xs"
                style={{ color: series.color }}
              >
                <span
                  class="w-2 h-0.5 rounded"
                  style={{ 'background-color': series.color }}
                />
                {series.name}
              </span>
            )}
          </For>
        </div>
      </div>

      {/* 차트 컨테이너 */}
      <div
        ref={setContainerRef}
        class="w-full rounded-lg overflow-hidden border border-[var(--color-surface-light)]"
        style={{ height: `${props.height || 120}px` }}
      />

      {/* 기준선 레이블 (있는 경우) */}
      <Show when={props.indicator.scaleRange?.levels}>
        <div class="flex gap-2 mt-1 px-2">
          <For each={props.indicator.scaleRange?.levels}>
            {(level) => (
              <span class="text-xs text-[var(--color-text-muted)]">{level}</span>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}

export default SubPriceChart;
