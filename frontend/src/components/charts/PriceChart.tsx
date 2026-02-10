/**
 * 가격 차트 컴포넌트
 *
 * 캔들스틱 또는 라인 차트로 가격 데이터를 시각화합니다.
 * - 지표 오버레이 지원 (이동평균선, RSI 등)
 * - 거래 마커 지원 (진입/청산 포인트)
 * - 차트 간 동기화 지원 (크로스헤어, 줌/팬, 시간축)
 *
 * 공통 유틸리티와 useLightweightChart 훅을 사용하여 코드 중복을 최소화합니다.
 *
 * @example
 * ```tsx
 * // 기본 캔들스틱 차트
 * <PriceChart data={ohlcData()} type="candlestick" height={400} />
 *
 * // 지표 오버레이 포함
 * <PriceChart
 *   data={ohlcData()}
 *   indicators={[
 *     { id: 'sma20', name: 'SMA 20', data: smaData, color: '#3b82f6' },
 *   ]}
 * />
 *
 * // 동기화 사용
 * const sync = useChartSync();
 * <PriceChart data={priceData()} {...sync.bindChart('price')} />
 * ```
 */

import { createEffect, on } from 'solid-js';
import {
  CandlestickSeries,
  LineSeries,
  createSeriesMarkers,
} from 'lightweight-charts';
import type {
  IChartApi,
  ISeriesApi,
  CandlestickData,
  LineData,
  SeriesMarker,
  Time,
  ISeriesMarkersPluginApi,
} from 'lightweight-charts';
import { useLightweightChart } from '../../hooks/useLightweightChart';
import {
  sortByTime,
  type ChartColors,
  type ChartSyncState,
} from '../../utils/chartUtils';

// ==================== 타입 정의 ====================

/** 시간 범위 타입 (차트 동기화용) */
export interface TimeRange {
  from: Time;
  to: Time;
}

export interface CandlestickDataPoint {
  time: string | number;
  open: number;
  high: number;
  low: number;
  close: number;
}

export interface LineDataPoint {
  time: string | number;
  value: number;
}

/** 지표 오버레이 데이터. */
export interface IndicatorOverlay {
  id: string;
  name: string;
  data: LineDataPoint[];
  color: string;
  lineWidth?: number;
  priceScaleId?: 'left' | 'right';
}

/** 거래 마커 타입. */
export type TradeMarkerType = 'entry' | 'exit' | 'buy' | 'sell' | 'stop' | 'target';

/** 거래 마커 데이터. */
export interface TradeMarker {
  time: string | number;
  type: TradeMarkerType;
  price?: number;
  label?: string;
}

export interface PriceChartProps {
  /** 가격 데이터 */
  data: CandlestickDataPoint[] | LineDataPoint[];
  /** 차트 타입 */
  type?: 'candlestick' | 'line';
  /** 차트 높이 (픽셀) */
  height?: number;
  /** 거래량 표시 여부 */
  showVolume?: boolean;
  /** 지표 오버레이 목록. */
  indicators?: IndicatorOverlay[];
  /** 거래 마커 목록 (진입/청산 표시). */
  markers?: TradeMarker[];
  /** 커스텀 색상 */
  colors?: {
    background?: string;
    text?: string;
    grid?: string;
    upColor?: string;
    downColor?: string;
    lineColor?: string;
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

// ==================== 헬퍼 함수 ====================

/** 마커 타입별 설정 반환. */
function getMarkerConfig(type: TradeMarkerType): {
  position: 'aboveBar' | 'belowBar' | 'inBar';
  color: string;
  shape: 'circle' | 'arrowUp' | 'arrowDown' | 'square';
  defaultLabel: string;
} {
  switch (type) {
    case 'entry':
    case 'buy':
      return { position: 'belowBar', color: '#22c55e', shape: 'arrowUp', defaultLabel: 'BUY' };
    case 'exit':
    case 'sell':
      return { position: 'aboveBar', color: '#ef4444', shape: 'arrowDown', defaultLabel: 'SELL' };
    case 'stop':
      return { position: 'aboveBar', color: '#f97316', shape: 'circle', defaultLabel: 'STOP' };
    case 'target':
      return { position: 'aboveBar', color: '#3b82f6', shape: 'square', defaultLabel: 'TP' };
    default:
      return { position: 'inBar', color: '#6b7280', shape: 'circle', defaultLabel: '' };
  }
}

// ==================== 컴포넌트 ====================

export function PriceChart(props: PriceChartProps) {
  // 시리즈 및 플러그인 참조
  let mainSeries: ISeriesApi<'Candlestick'> | ISeriesApi<'Line'> | undefined;
  let markersPlugin: ISeriesMarkersPluginApi<Time> | undefined;
  const indicatorSeries = new Map<string, ISeriesApi<'Line'>>();

  // 차트 인스턴스 참조 (지표 관리용)
  let chartInstance: IChartApi | undefined;

  // 기본 색상
  const defaultUpColor = '#22c55e';
  const defaultDownColor = '#ef4444';
  const defaultLineColor = '#3b82f6';

  // 색상 병합
  const getChartColors = (): Partial<ChartColors> => ({
    background: props.colors?.background,
    text: props.colors?.text,
    grid: props.colors?.grid,
    positive: props.colors?.upColor || defaultUpColor,
    negative: props.colors?.downColor || defaultDownColor,
    line: props.colors?.lineColor || defaultLineColor,
  });

  // 통합 차트 훅 사용
  const { setContainerRef } = useLightweightChart({
    height: props.height || 400,
    colors: getChartColors(),
    syncState: props.syncState,
    syncMode: props.syncMode || 'full',
    onVisibleRangeChange: props.onVisibleRangeChange,
    onCrosshairMove: props.onCrosshairMove,
    timeScale: {
      timeVisible: true,
      secondsVisible: false,
    },
    onReady: (chartApi, mergedColors) => {
      chartInstance = chartApi;

      // 차트 타입에 따라 시리즈 생성
      if (props.type === 'line') {
        mainSeries = chartApi.addSeries(LineSeries, {
          color: mergedColors.line,
          lineWidth: 2,
        });
      } else {
        mainSeries = chartApi.addSeries(CandlestickSeries, {
          upColor: mergedColors.positive,
          downColor: mergedColors.negative,
          borderUpColor: mergedColors.positive,
          borderDownColor: mergedColors.negative,
          wickUpColor: mergedColors.positive,
          wickDownColor: mergedColors.negative,
        });
      }

      // 초기 데이터 설정
      if (props.data && props.data.length > 0) {
        const sortedData = sortByTime(props.data);
        mainSeries.setData(sortedData as CandlestickData[] | LineData[]);
        // DOM 렌더링 완료 후 fitContent
        requestAnimationFrame(() => {
          chartApi.timeScale().fitContent();
        });
      }
    },
  });

  // 데이터 업데이트
  createEffect(
    on(
      () => props.data,
      (data) => {
        if (!mainSeries || !data || data.length === 0) return;
        const sortedData = sortByTime(data);
        mainSeries.setData(sortedData as CandlestickData[] | LineData[]);
        chart()?.timeScale().fitContent();
      }
    )
  );

  // 지표 오버레이 업데이트
  createEffect(
    on(
      () => props.indicators,
      (indicators) => {
        const currentChart = chartInstance;
        if (!currentChart) return;

        // 더 이상 존재하지 않는 지표 시리즈 제거
        const currentIds = new Set(indicators?.map((i) => i.id) || []);
        for (const [id, series] of indicatorSeries.entries()) {
          if (!currentIds.has(id)) {
            currentChart.removeSeries(series);
            indicatorSeries.delete(id);
          }
        }

        // 지표 시리즈 추가 또는 업데이트
        if (indicators) {
          for (const indicator of indicators) {
            let series = indicatorSeries.get(indicator.id);

            if (!series) {
              // 새 시리즈 생성
              const lineWidth = (
                indicator.lineWidth && indicator.lineWidth >= 1 && indicator.lineWidth <= 4
                  ? indicator.lineWidth
                  : 2
              ) as 1 | 2 | 3 | 4;

              series = currentChart.addSeries(LineSeries, {
                color: indicator.color,
                lineWidth,
                priceScaleId: indicator.priceScaleId || 'right',
                lastValueVisible: true,
                priceLineVisible: false,
              });
              indicatorSeries.set(indicator.id, series);
            }

            // 시리즈 데이터 업데이트
            if (indicator.data && indicator.data.length > 0) {
              const sortedData = sortByTime(indicator.data);
              series.setData(sortedData as LineData[]);
            }
          }
        }
      }
    )
  );

  // 거래 마커 업데이트 (Lightweight Charts v5 API)
  createEffect(
    on(
      () => props.markers,
      (markers) => {
        if (!mainSeries) return;

        // TradeMarker를 SeriesMarker 형식으로 변환
        const seriesMarkers: SeriesMarker<Time>[] = (markers || [])
          .map((marker) => {
            const config = getMarkerConfig(marker.type);
            return {
              time: marker.time as Time,
              position: config.position,
              color: config.color,
              shape: config.shape,
              text: marker.label || config.defaultLabel,
            };
          })
          .sort((a, b) => {
            const timeA = typeof a.time === 'string' ? a.time : String(a.time);
            const timeB = typeof b.time === 'string' ? b.time : String(b.time);
            return timeA.localeCompare(timeB);
          });

        // 기존 마커 플러그인 초기화 후 새로 생성
        if (markersPlugin) {
          markersPlugin.setMarkers([]);
        }

        if (seriesMarkers.length > 0) {
          markersPlugin = createSeriesMarkers(mainSeries, seriesMarkers);
        }
      }
    )
  );

  return (
    <div
      ref={setContainerRef}
      class="w-full rounded-lg overflow-hidden"
      style={{ height: `${props.height || 400}px` }}
    />
  );
}

export default PriceChart;
