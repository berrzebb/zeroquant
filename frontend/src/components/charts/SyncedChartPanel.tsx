/**
 * SyncedChartPanel - 메인 차트와 보조 지표 차트를 동기화하여 관리하는 복합 컴포넌트.
 *
 * 기능:
 * - 메인 캔들스틱/라인 차트와 여러 서브 지표 차트 통합 렌더링
 * - 모든 차트 간 시간 축(Time Scale) 동기화
 * - 크로스헤어 위치 동기화
 * - 외부 차트와의 완전 동기화 (useChartSync 연동)
 *
 * @example
 * ```tsx
 * // 기본 사용
 * <SyncedChartPanel
 *   data={candlestickData}
 *   indicators={overlayIndicators}
 *   subIndicators={[rsiIndicator, macdIndicator]}
 * />
 *
 * // 외부 차트 동기화
 * const sync = useChartSync();
 * <SyncedChartPanel data={data} {...sync.bindChart('main')} />
 * <EquityCurve data={equity} {...sync.bindChart('equity')} />
 * ```
 */
import { onMount, onCleanup, createEffect, on, For, Show } from 'solid-js';
import {
  createChart,
  ColorType,
  CandlestickSeries,
  LineSeries,
  HistogramSeries,
  createSeriesMarkers,
} from 'lightweight-charts';
import type {
  IChartApi,
  ISeriesApi,
  CandlestickData,
  LineData,
  HistogramData,
  SeriesMarker,
  Time,
  ISeriesMarkersPluginApi,
  LogicalRange,
  MouseEventParams,
} from 'lightweight-charts';
import type { CandlestickDataPoint, LineDataPoint, IndicatorOverlay, TradeMarker, TradeMarkerType } from './PriceChart';
import type { SeparateIndicatorData } from './SubPriceChart';
import {
  sortByTime,
  type ChartSyncState,
  type CrosshairPosition,
} from '../../utils/chartUtils';

// ChartSyncState re-export (다른 컴포넌트에서 import 편의)
export type { ChartSyncState } from '../../utils/chartUtils';

// ==================== 유틸리티 함수 ====================

/** 마커 타입별 설정 반환 */
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

// ==================== 타입 정의 ====================

interface SyncedChartPanelProps {
  /** 메인 차트 데이터 (캔들스틱 또는 라인) */
  data: CandlestickDataPoint[] | LineDataPoint[];
  /** 메인 차트 타입 */
  type?: 'candlestick' | 'line';
  /** 메인 차트 높이 */
  mainHeight?: number;
  /** 서브 차트 높이 */
  subHeight?: number;
  /** 메인 차트 오버레이 지표 (SMA, EMA, BB 등) */
  indicators?: IndicatorOverlay[];
  /** 서브 차트 지표 목록 (RSI, MACD, Volume 등) */
  subIndicators?: SeparateIndicatorData[];
  /** 거래 마커 */
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
  // ==================== 동기화 props ====================
  /** 차트 고유 ID (동기화 무한 루프 방지용) */
  chartId?: string;
  /** 외부 동기화 상태 (다른 차트에서 변경된 범위) */
  syncState?: () => ChartSyncState | null;
  /** 범위 변경 시 부모에게 알림 */
  onVisibleRangeChange?: (state: ChartSyncState) => void;
  /** 크로스헤어 이동 시 부모에게 알림 */
  onCrosshairMove?: (state: ChartSyncState) => void;
  /** 동기화 모드 ('range' | 'crosshair' | 'full') */
  syncMode?: 'range' | 'crosshair' | 'full';
}

// ==================== 컴포넌트 ====================

export function SyncedChartPanel(props: SyncedChartPanelProps) {
  // 차트 인스턴스 저장
  let mainChartContainer: HTMLDivElement | undefined;
  let mainChart: IChartApi | undefined;
  let mainSeries: ISeriesApi<'Candlestick'> | ISeriesApi<'Line'> | undefined;
  let markersPlugin: ISeriesMarkersPluginApi<Time> | undefined;
  const mainIndicatorSeries = new Map<string, ISeriesApi<'Line'>>();

  // 서브 차트들
  const subChartRefs: HTMLDivElement[] = [];
  const subCharts: IChartApi[] = [];
  const subSeriesMap: Map<string, (ISeriesApi<'Line'> | ISeriesApi<'Histogram'>)[]> = new Map();
  // 서브 차트별 첫 번째 시리즈 (크로스헤어 동기화용)
  const subFirstSeries: (ISeriesApi<'Line'> | ISeriesApi<'Histogram'> | undefined)[] = [];

  // 동기화 플래그 (무한 루프 방지)
  let isSyncing = false;
  // 외부 업데이트 플래그 (외부 차트 동기화 무한 루프 방지)
  let isExternalUpdate = false;
  // 고유 차트 ID
  const chartId = props.chartId || 'synced-' + Math.random().toString(36).substr(2, 9);
  // 동기화 모드
  const syncMode = () => props.syncMode || 'full';

  const defaultColors = {
    background: '#1f2937',  // gray-800 다크 테마 배경
    text: '#d1d5db',
    grid: '#374151',
    upColor: '#22c55e',
    downColor: '#ef4444',
    lineColor: '#3b82f6',
  };

  const getColors = () => ({
    ...defaultColors,
    ...props.colors,
  });

  // ==================== 동기화 함수 ====================

  // 시간 축 동기화 함수 (내부 + 외부 동기화)
  const syncTimeScale = (sourceChart: IChartApi, logicalRange: LogicalRange | null) => {
    if (isSyncing || isExternalUpdate || !logicalRange) return;
    isSyncing = true;

    // 내부 차트들 동기화 (메인 + 서브 차트)
    const allCharts = [mainChart, ...subCharts].filter(Boolean) as IChartApi[];
    for (const chart of allCharts) {
      if (chart !== sourceChart) {
        chart.timeScale().setVisibleLogicalRange(logicalRange);
      }
    }

    // 외부 차트 동기화를 위해 부모에게 범위 변경 알림
    const currentSyncMode = syncMode();
    if (currentSyncMode === 'range' || currentSyncMode === 'full') {
      props.onVisibleRangeChange?.({
        range: logicalRange,
        crosshair: null,
        sourceId: chartId,
        syncType: 'range',
      });
    }

    isSyncing = false;
  };

  // 크로스헤어 동기화 함수 (내부 + 외부)
  const syncCrosshair = (sourceChart: IChartApi, param: MouseEventParams) => {
    if (isSyncing) return;
    isSyncing = true;

    // 메인 차트가 소스가 아닌 경우 메인 차트에 동기화
    if (mainChart && sourceChart !== mainChart && mainSeries) {
      if (param.time) {
        mainChart.setCrosshairPosition(NaN, param.time, mainSeries);
      } else {
        mainChart.clearCrosshairPosition();
      }
    }

    // 서브 차트들에 동기화
    for (let i = 0; i < subCharts.length; i++) {
      const chart = subCharts[i];
      const series = subFirstSeries[i];
      if (chart && series && sourceChart !== chart) {
        if (param.time) {
          chart.setCrosshairPosition(NaN, param.time, series);
        } else {
          chart.clearCrosshairPosition();
        }
      }
    }

    // 외부 차트 동기화를 위해 부모에게 크로스헤어 이동 알림
    if (syncMode() === 'crosshair' || syncMode() === 'full') {
      const crosshair: CrosshairPosition = {
        time: param.time ?? null,
        price: null,
        logical: param.logical ?? null,
        x: param.point?.x ?? null,
        y: param.point?.y ?? null,
      };

      // 시리즈 값 추출
      if (param.seriesData && param.seriesData.size > 0) {
        const firstSeriesData = param.seriesData.values().next().value;
        if (firstSeriesData && 'value' in firstSeriesData) {
          crosshair.price = firstSeriesData.value;
        } else if (firstSeriesData && 'close' in firstSeriesData) {
          crosshair.price = firstSeriesData.close;
        }
      }

      props.onCrosshairMove?.({
        range: null,
        crosshair,
        sourceId: chartId,
        syncType: 'crosshair',
      });
    }

    isSyncing = false;
  };

  // ==================== 차트 생성 ====================

  // 메인 차트 생성
  const createMainChart = () => {
    if (!mainChartContainer) return;

    const colors = getColors();

    mainChart = createChart(mainChartContainer, {
      layout: {
        background: { type: ColorType.Solid, color: colors.background },
        textColor: colors.text,
      },
      grid: {
        vertLines: { color: colors.grid },
        horzLines: { color: colors.grid },
      },
      width: mainChartContainer.clientWidth,
      height: props.mainHeight || 240,
      crosshair: { mode: 1 },
      timeScale: {
        borderColor: colors.grid,
        timeVisible: true,
        secondsVisible: false,
      },
      rightPriceScale: { borderColor: colors.grid },
    });

    // 시리즈 생성
    if (props.type === 'line') {
      mainSeries = mainChart.addSeries(LineSeries, {
        color: colors.lineColor,
        lineWidth: 2,
      });
    } else {
      mainSeries = mainChart.addSeries(CandlestickSeries, {
        upColor: colors.upColor,
        downColor: colors.downColor,
        borderUpColor: colors.upColor,
        borderDownColor: colors.downColor,
        wickUpColor: colors.upColor,
        wickDownColor: colors.downColor,
      });
    }

    // 데이터 설정 (정렬)
    if (props.data && props.data.length > 0) {
      const sortedData = sortByTime(props.data as CandlestickDataPoint[]);
      mainSeries.setData(sortedData as CandlestickData[] | LineData[]);
    }

    // 초기 마커 설정
    if (props.markers && props.markers.length > 0) {
      const seriesMarkers: SeriesMarker<Time>[] = sortByTime(
        props.markers.map((marker) => {
          const config = getMarkerConfig(marker.type);
          return {
            time: marker.time as Time,
            position: config.position,
            color: config.color,
            shape: config.shape,
            text: marker.label || config.defaultLabel,
          };
        })
      );

      if (seriesMarkers.length > 0) {
        markersPlugin = createSeriesMarkers(mainSeries, seriesMarkers);
      }
    }

    // 동기화 이벤트 구독
    mainChart.timeScale().subscribeVisibleLogicalRangeChange((range) => {
      syncTimeScale(mainChart!, range);
    });

    mainChart.subscribeCrosshairMove((param) => {
      syncCrosshair(mainChart!, param);
    });

    // 초기 fit
    requestAnimationFrame(() => {
      mainChart?.timeScale().fitContent();
    });
  };

  // 서브 차트 생성
  const createSubChart = (container: HTMLDivElement, indicator: SeparateIndicatorData, index: number) => {
    const colors = getColors();

    const chart = createChart(container, {
      layout: {
        background: { type: ColorType.Solid, color: colors.background },
        textColor: colors.text,
      },
      grid: {
        vertLines: { color: colors.grid },
        horzLines: { color: colors.grid },
      },
      width: container.clientWidth,
      height: props.subHeight || 100,
      crosshair: { mode: 1 },
      timeScale: {
        borderColor: colors.grid,
        timeVisible: true,
        secondsVisible: false,
        visible: true,
      },
      rightPriceScale: {
        borderColor: colors.grid,
        scaleMargins: { top: 0.1, bottom: 0.1 },
      },
    });

    subCharts[index] = chart;

    // 시리즈 생성
    const seriesList: (ISeriesApi<'Line'> | ISeriesApi<'Histogram'>)[] = [];

    for (const series of indicator.series) {
      const lineWidth = (series.lineWidth && series.lineWidth >= 1 && series.lineWidth <= 4
        ? series.lineWidth
        : 2) as 1 | 2 | 3 | 4;

      let chartSeries: ISeriesApi<'Line'> | ISeriesApi<'Histogram'>;

      if (series.seriesType === 'bar') {
        chartSeries = chart.addSeries(HistogramSeries, {
          color: series.color,
          priceFormat: { type: 'price', precision: 4, minMove: 0.0001 },
        });
      } else {
        chartSeries = chart.addSeries(LineSeries, {
          color: series.color,
          lineWidth,
          lastValueVisible: true,
          priceLineVisible: false,
        });
      }

      // 데이터 설정 (정렬)
      if (series.data && series.data.length > 0) {
        const sortedData = sortByTime(series.data);

        if (series.seriesType === 'bar') {
          const histogramData = sortedData.map(d => ({
            time: d.time as Time,
            value: d.value,
            color: d.value >= 0 ? '#22c55e' : '#ef4444',
          }));
          chartSeries.setData(histogramData as HistogramData[]);
        } else {
          chartSeries.setData(sortedData as LineData[]);
        }
      }

      seriesList.push(chartSeries);

      // 첫 번째 시리즈를 크로스헤어 동기화용으로 저장
      if (seriesList.length === 1) {
        subFirstSeries[index] = chartSeries;
      }
    }

    // 기준선 추가 (RSI 30/70, Stochastic 20/80 등)
    if (indicator.scaleRange?.levels && indicator.series[0]?.data?.length > 0) {
      const sortedTimeData = sortByTime(indicator.series[0].data);
      for (const level of indicator.scaleRange.levels) {
        const levelSeries = chart.addSeries(LineSeries, {
          color: '#6b7280',
          lineWidth: 1,
          lineStyle: 2,
          lastValueVisible: false,
          priceLineVisible: false,
        });
        const levelData = sortedTimeData.map(d => ({
          time: d.time as Time,
          value: level,
        }));
        levelSeries.setData(levelData as LineData[]);
      }
    }

    subSeriesMap.set(indicator.id, seriesList);

    // 동기화 이벤트 구독
    chart.timeScale().subscribeVisibleLogicalRangeChange((range) => {
      syncTimeScale(chart, range);
    });

    chart.subscribeCrosshairMove((param) => {
      syncCrosshair(chart, param);
    });

    return chart;
  };

  // ==================== 라이프사이클 ====================

  onMount(() => {
    createMainChart();

    // 리사이즈 핸들러
    const handleResize = () => {
      if (mainChart && mainChartContainer) {
        mainChart.applyOptions({ width: mainChartContainer.clientWidth });
      }
      subCharts.forEach((chart, i) => {
        const container = subChartRefs[i];
        if (chart && container) {
          chart.applyOptions({ width: container.clientWidth });
        }
      });
    };

    window.addEventListener('resize', handleResize);

    onCleanup(() => {
      window.removeEventListener('resize', handleResize);
      mainChart?.remove();
      subCharts.forEach(chart => chart?.remove());
    });
  });

  // ==================== 외부 동기화 Effect ====================

  // 다른 차트에서 범위/크로스헤어가 변경되면 이 차트도 동기화
  createEffect(
    on(
      () => props.syncState?.(),
      (syncState) => {
        if (!mainChart || !syncState) return;

        // 자신이 소스면 무시 (무한 루프 방지)
        if (syncState.sourceId === chartId) return;

        isExternalUpdate = true;

        // syncMode 기반으로 동기화 여부 결정
        const currentSyncMode = syncMode();
        const shouldSyncRange = currentSyncMode === 'range' || currentSyncMode === 'full';
        const shouldSyncCrosshair = currentSyncMode === 'crosshair' || currentSyncMode === 'full';

        // 시간 범위 동기화
        if (syncState.range && shouldSyncRange) {
          mainChart.timeScale().setVisibleLogicalRange(syncState.range);
          subCharts.forEach(chart => {
            chart?.timeScale().setVisibleLogicalRange(syncState.range!);
          });
        }

        // 크로스헤어 동기화
        if (syncState.crosshair?.time && shouldSyncCrosshair) {
          if (mainSeries) {
            mainChart.setCrosshairPosition(NaN, syncState.crosshair.time, mainSeries);
          }
          subCharts.forEach((chart, i) => {
            const series = subFirstSeries[i];
            if (chart && series) {
              chart.setCrosshairPosition(NaN, syncState.crosshair!.time!, series);
            }
          });
        }

        setTimeout(() => { isExternalUpdate = false; }, 0);
      }
    )
  );

  // ==================== 데이터 업데이트 Effects ====================

  // 메인 데이터 변경 시 업데이트
  createEffect(
    on(
      () => props.data,
      (data) => {
        if (mainSeries && data && data.length > 0) {
          const sortedData = sortByTime(data as CandlestickDataPoint[]);
          mainSeries.setData(sortedData as CandlestickData[] | LineData[]);
          mainChart?.timeScale().fitContent();
        }
      }
    )
  );

  // 오버레이 지표 변경 시 업데이트
  createEffect(
    on(
      () => props.indicators,
      (indicators) => {
        if (!mainChart) return;

        // 기존 지표 제거
        const currentIds = new Set(indicators?.map(i => i.id) || []);
        for (const [id, series] of mainIndicatorSeries.entries()) {
          if (!currentIds.has(id)) {
            mainChart.removeSeries(series);
            mainIndicatorSeries.delete(id);
          }
        }

        // 새 지표 추가/업데이트
        if (indicators) {
          for (const indicator of indicators) {
            let series = mainIndicatorSeries.get(indicator.id);

            if (!series) {
              const lineWidth = (indicator.lineWidth && indicator.lineWidth >= 1 && indicator.lineWidth <= 4
                ? indicator.lineWidth
                : 2) as 1 | 2 | 3 | 4;
              series = mainChart.addSeries(LineSeries, {
                color: indicator.color,
                lineWidth,
                priceScaleId: indicator.priceScaleId || 'right',
                lastValueVisible: true,
                priceLineVisible: false,
              });
              mainIndicatorSeries.set(indicator.id, series);
            }

            if (indicator.data && indicator.data.length > 0) {
              const sortedData = sortByTime(indicator.data);
              series.setData(sortedData as LineData[]);
            }
          }
        }
      }
    )
  );

  // 거래 마커 변경 시 업데이트
  createEffect(
    on(
      () => props.markers,
      (markers) => {
        if (!mainSeries) return;

        const seriesMarkers: SeriesMarker<Time>[] = sortByTime(
          (markers || []).map((marker) => {
            const config = getMarkerConfig(marker.type);
            return {
              time: marker.time as Time,
              position: config.position,
              color: config.color,
              shape: config.shape,
              text: marker.label || config.defaultLabel,
            };
          })
        );

        if (markersPlugin) {
          markersPlugin.setMarkers([]);
        }

        if (seriesMarkers.length > 0) {
          markersPlugin = createSeriesMarkers(mainSeries, seriesMarkers);
        }
      }
    )
  );

  // 서브 지표 변경 시 서브 차트 재생성
  createEffect(
    on(
      () => props.subIndicators,
      (subIndicators) => {
        // 기존 서브 차트 제거
        subCharts.forEach(chart => chart?.remove());
        subCharts.length = 0;
        subFirstSeries.length = 0;
        subSeriesMap.clear();

        // 새 서브 차트 생성은 For 루프에서 처리됨
        // 여기서는 메인 차트의 시간 범위를 가져와서 동기화
        if (mainChart && subIndicators && subIndicators.length > 0) {
          requestAnimationFrame(() => {
            const range = mainChart?.timeScale().getVisibleLogicalRange();
            if (range) {
              subCharts.forEach(chart => {
                chart?.timeScale().setVisibleLogicalRange(range);
              });
            }
          });
        }
      }
    )
  );

  // ==================== 렌더링 ====================

  return (
    <div class="flex flex-col gap-0">
      {/* 메인 차트 */}
      <div
        ref={mainChartContainer}
        class="w-full rounded-t-lg overflow-hidden border border-[var(--color-surface-light)]"
        style={{ height: `${props.mainHeight || 240}px` }}
      />

      {/* 서브 지표 차트들 */}
      <For each={props.subIndicators}>
        {(indicator, index) => (
          <div class="border-x border-b border-[var(--color-surface-light)] last:rounded-b-lg overflow-hidden">
            {/* 지표 레이블 */}
            <div class="flex items-center gap-2 px-2 py-1 bg-[var(--color-surface)]/50">
              <span class="text-xs font-medium text-[var(--color-text-muted)]">
                {indicator.name}
              </span>
              <div class="flex gap-2">
                <For each={indicator.series}>
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
              ref={(el) => {
                subChartRefs[index()] = el;
                // 약간의 지연 후 차트 생성 (DOM이 준비된 후)
                requestAnimationFrame(() => {
                  if (el && !subCharts[index()]) {
                    createSubChart(el, indicator, index());
                  }
                });
              }}
              class="w-full"
              style={{ height: `${props.subHeight || 100}px` }}
            />
            {/* 기준선 레이블 */}
            <Show when={indicator.scaleRange?.levels}>
              <div class="flex gap-2 px-2 py-0.5 bg-[var(--color-surface)]/50">
                <For each={indicator.scaleRange?.levels}>
                  {(level) => (
                    <span class="text-xs text-[var(--color-text-muted)]">{level}</span>
                  )}
                </For>
              </div>
            </Show>
          </div>
        )}
      </For>
    </div>
  );
}

export default SyncedChartPanel;
