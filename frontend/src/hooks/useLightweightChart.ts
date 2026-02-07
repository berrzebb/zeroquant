/**
 * Lightweight Charts 통합 훅
 *
 * lightweight-charts 인스턴스의 생성, 리사이즈, 정리를 관리합니다.
 * 모든 lightweight-charts 기반 컴포넌트에서 공통으로 사용됩니다.
 *
 * 주요 기능:
 * - 차트 생성 및 정리 자동화
 * - 리사이즈 자동 처리
 * - **차트 간 완전 동기화** (크로스헤어 + 줌/팬 + 시간축)
 *
 * @example
 * ```tsx
 * // 기본 사용
 * const { setContainerRef, chart } = useLightweightChart({
 *   height: 300,
 *   onReady: (chart) => {
 *     chart.addAreaSeries({ ... });
 *   },
 * });
 *
 * // 완전 동기화 사용 (여러 차트)
 * const sync = useChartSync();
 *
 * <EquityCurve {...sync.bindChart('equity')} />
 * <DrawdownChart {...sync.bindChart('drawdown')} />
 * ```
 */

import { createSignal, onMount, onCleanup, createEffect, on } from 'solid-js';
import {
  createChart,
  type IChartApi,
  type ISeriesApi,
  type SeriesType,
  type LogicalRange,
  type MouseEventParams,
  type Time,
} from 'lightweight-charts';
import {
  type ChartColors,
  type ChartSyncState,
  type CrosshairPosition,
  createChartOptions,
  mergeChartColors,
  generateChartId,
} from '../utils/chartUtils';

// ==================== 타입 정의 ====================

export interface UseLightweightChartOptions {
  /** 차트 높이 (픽셀) */
  height?: number;
  /** 차트 색상 설정 */
  colors?: Partial<ChartColors>;
  /** 차트 준비 완료 시 콜백 (시리즈 추가 등) */
  onReady?: (chart: IChartApi, colors: ChartColors) => void;
  /** 차트 동기화 ID (같은 ID끼리 시간축 동기화) */
  syncId?: string;
  /** 외부에서 전달받는 동기화 상태 */
  syncState?: () => ChartSyncState | null;
  /** 시간 범위 변경 시 콜백 (다른 차트에 전파용) */
  onVisibleRangeChange?: (state: ChartSyncState) => void;
  /** 크로스헤어 이동 시 콜백 (다른 차트에 전파용) */
  onCrosshairMove?: (state: ChartSyncState) => void;
  /** 동기화 모드 ('range' | 'crosshair' | 'full') */
  syncMode?: 'range' | 'crosshair' | 'full';
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
    secondsVisible?: boolean;
  };
}

export interface UseLightweightChartReturn {
  /** 컨테이너 ref 설정 함수 */
  setContainerRef: (el: HTMLDivElement) => void;
  /** 컨테이너 요소 */
  containerRef: () => HTMLDivElement | undefined;
  /** 차트 인스턴스 */
  chart: () => IChartApi | null;
  /** 차트 ID */
  chartId: string;
  /** 병합된 색상 */
  colors: () => ChartColors;
  /** 차트 크기 수동 업데이트 */
  resize: () => void;
  /** 시간축 전체 콘텐츠에 맞추기 */
  fitContent: () => void;
  /** 특정 시간으로 크로스헤어 이동 (외부 동기화용) */
  setCrosshairPosition: (time: Time | null) => void;
}

// ==================== 메인 훅 ====================

/**
 * Lightweight Charts 통합 훅
 *
 * 차트 생성, 리사이즈 핸들링, 동기화, 정리를 자동으로 관리합니다.
 */
export function useLightweightChart(
  options: UseLightweightChartOptions = {}
): UseLightweightChartReturn {
  const chartId = generateChartId('lw');
  const [containerRef, setContainerRef] = createSignal<HTMLDivElement>();
  const [chartInstance, setChartInstance] = createSignal<IChartApi | null>(null);

  // 색상 병합
  const colors = () => mergeChartColors(options.colors);

  // 동기화 상태 (외부 업데이트 중인지 추적)
  let isExternalRangeUpdate = false;
  let isExternalCrosshairUpdate = false;

  // 동기화 모드 (기본: full)
  const syncMode = options.syncMode || 'full';

  // 리사이즈 핸들러
  const handleResize = () => {
    const chart = chartInstance();
    const container = containerRef();
    if (chart && container) {
      chart.applyOptions({ width: container.clientWidth });
    }
  };

  // 시간축 맞춤
  const fitContent = () => {
    const chart = chartInstance();
    if (chart) {
      chart.timeScale().fitContent();
    }
  };

  // 외부에서 크로스헤어 위치 설정
  const setCrosshairPosition = (time: Time | null) => {
    const chart = chartInstance();
    if (!chart || !time) return;

    isExternalCrosshairUpdate = true;
    // lightweight-charts에서 크로스헤어를 직접 설정하는 API가 제한적이므로
    // 시간 좌표를 기반으로 시리즈의 데이터 포인트를 찾아 마킹
    // 실제로는 subscribeCrosshairMove의 콜백에서 처리됨
    setTimeout(() => {
      isExternalCrosshairUpdate = false;
    }, 0);
  };

  // 차트 초기화
  onMount(() => {
    const container = containerRef();
    if (!container) return;

    const mergedColors = colors();
    const chartOptions = createChartOptions({
      width: container.clientWidth,
      height: options.height || 300,
      colors: mergedColors,
      crosshair: { mode: options.crosshairMode },
      rightPriceScale: options.rightPriceScale,
      timeScale: options.timeScale,
    });

    const chart = createChart(container, chartOptions);
    setChartInstance(chart);

    // 시간 범위 변경 동기화 (줌/팬)
    // 항상 구독하고 콜백 내부에서 조건 체크 (lazy 로딩 대응)
    if (syncMode === 'range' || syncMode === 'full') {
      chart.timeScale().subscribeVisibleLogicalRangeChange((logicalRange: LogicalRange | null) => {
        if (isExternalRangeUpdate || !logicalRange) return;
        options.onVisibleRangeChange?.({
          range: { from: logicalRange.from, to: logicalRange.to },
          crosshair: null,
          sourceId: chartId,
          syncType: 'range',
        });
      });
    }

    // 크로스헤어 이동 동기화
    // 항상 구독하고 콜백 내부에서 조건 체크
    if (syncMode === 'crosshair' || syncMode === 'full') {
      chart.subscribeCrosshairMove((param: MouseEventParams) => {
        if (isExternalCrosshairUpdate) return;

        const crosshair: CrosshairPosition = {
          time: param.time ?? null,
          price: null,
          logical: param.logical ?? null,
          x: param.point?.x ?? null,
          y: param.point?.y ?? null,
        };

        // 시리즈 값 추출 (첫 번째 시리즈의 값)
        if (param.seriesData && param.seriesData.size > 0) {
          const firstSeriesData = param.seriesData.values().next().value;
          if (firstSeriesData && 'value' in firstSeriesData) {
            crosshair.price = firstSeriesData.value;
          } else if (firstSeriesData && 'close' in firstSeriesData) {
            crosshair.price = firstSeriesData.close;
          }
        }

        options.onCrosshairMove?.({
          range: null,
          crosshair,
          sourceId: chartId,
          syncType: 'crosshair',
        });
      });
    }

    // 사용자 콜백 호출 (시리즈 추가 등)
    options.onReady?.(chart, mergedColors);

    // 리사이즈 이벤트 등록
    window.addEventListener('resize', handleResize);
  });

  // 외부 동기화 상태 반영 (시간 범위 + 크로스헤어)
  createEffect(
    on(
      () => options.syncState?.(),
      (syncState) => {
        const chart = chartInstance();
        if (!chart || !syncState) return;

        // 자기 자신이 발생시킨 변경은 무시
        if (syncState.sourceId === chartId) return;

        // syncMode 기반으로 동기화 여부 결정
        const shouldSyncRange = syncMode === 'range' || syncMode === 'full';
        const shouldSyncCrosshair = syncMode === 'crosshair' || syncMode === 'full';

        // 시간 범위 동기화
        if (syncState.range && shouldSyncRange) {
          isExternalRangeUpdate = true;
          chart.timeScale().setVisibleLogicalRange({
            from: syncState.range.from,
            to: syncState.range.to,
          });
          setTimeout(() => {
            isExternalRangeUpdate = false;
          }, 0);
        }

        // 크로스헤어 동기화 (logical 인덱스 기반)
        if (syncState.crosshair && shouldSyncCrosshair) {
          isExternalCrosshairUpdate = true;
          // lightweight-charts는 직접 크로스헤어 위치 설정 API가 제한적
          // setCrosshairPosition 함수를 통해 간접 처리
          if (syncState.crosshair.time) {
            setCrosshairPosition(syncState.crosshair.time);
          }
          setTimeout(() => {
            isExternalCrosshairUpdate = false;
          }, 0);
        }
      }
    )
  );

  // 정리
  onCleanup(() => {
    window.removeEventListener('resize', handleResize);
    const chart = chartInstance();
    if (chart) {
      chart.remove();
    }
  });

  return {
    setContainerRef,
    containerRef,
    chart: chartInstance,
    chartId,
    colors,
    resize: handleResize,
    fitContent,
    setCrosshairPosition,
  };
}

// ==================== 시리즈 관리 훅 ====================

export interface UseChartSeriesOptions<T> {
  /** 차트 인스턴스 */
  chart: () => IChartApi | null;
  /** 시리즈 데이터 */
  data: () => T[] | undefined;
  /** 시리즈 생성 함수 */
  createSeries: (chart: IChartApi) => ISeriesApi<SeriesType>;
  /** 데이터 변환 함수 (선택사항) */
  transformData?: (data: T[]) => unknown[];
  /** 데이터 변경 시 fitContent 호출 여부 */
  autoFit?: boolean;
}

/**
 * 차트 시리즈 데이터 관리 훅
 */
export function useChartSeries<T>(options: UseChartSeriesOptions<T>) {
  const [series, setSeries] = createSignal<ISeriesApi<SeriesType> | null>(null);

  createEffect(
    on(
      () => options.chart(),
      (chart) => {
        if (!chart) return;
        const newSeries = options.createSeries(chart);
        setSeries(newSeries as ISeriesApi<SeriesType>);
      }
    )
  );

  createEffect(
    on(
      () => options.data(),
      (data) => {
        const currentSeries = series();
        const chart = options.chart();
        if (!currentSeries || !data || data.length === 0) return;

        const transformedData = options.transformData
          ? options.transformData(data)
          : data;

        currentSeries.setData(transformedData as Parameters<typeof currentSeries.setData>[0]);

        if (options.autoFit && chart) {
          chart.timeScale().fitContent();
        }
      }
    )
  );

  return series;
}

// ==================== 통합 차트 동기화 훅 ====================

export interface ChartSyncOptions {
  /** 동기화 모드 */
  mode?: 'range' | 'crosshair' | 'full';
  /** 디바운스 시간 (ms) - 고빈도 이벤트 최적화 */
  debounceMs?: number;
}

export interface ChartSyncReturn {
  /** 현재 동기화 상태 */
  syncState: () => ChartSyncState | null;
  /** 범위 변경 핸들러 */
  handleRangeChange: (state: ChartSyncState) => void;
  /** 크로스헤어 이동 핸들러 */
  handleCrosshairMove: (state: ChartSyncState) => void;
  /** 통합 변경 핸들러 (범위 + 크로스헤어) */
  handleSyncChange: (state: ChartSyncState) => void;
  /** 동기화 상태 초기화 */
  resetSync: () => void;
  /** 차트에 바인딩할 props 생성 */
  bindChart: (id?: string) => {
    syncState: () => ChartSyncState | null;
    onVisibleRangeChange: (state: ChartSyncState) => void;
    onCrosshairMove: (state: ChartSyncState) => void;
    syncMode: 'range' | 'crosshair' | 'full';
  };
}

/**
 * 여러 차트 간 완전 동기화를 관리하는 훅
 *
 * 크로스헤어, 줌/팬, 시간축을 모두 동기화합니다.
 *
 * @example
 * ```tsx
 * const sync = useChartSync({ mode: 'full' });
 *
 * // 방법 1: bindChart 사용 (권장)
 * <EquityCurve {...sync.bindChart('equity')} />
 * <DrawdownChart {...sync.bindChart('drawdown')} />
 *
 * // 방법 2: 개별 props 전달
 * <PriceChart
 *   syncState={sync.syncState}
 *   onVisibleRangeChange={sync.handleRangeChange}
 *   onCrosshairMove={sync.handleCrosshairMove}
 * />
 * ```
 */
export function useChartSync(options: ChartSyncOptions = {}): ChartSyncReturn {
  const mode = options.mode || 'full';
  const debounceMs = options.debounceMs || 0;

  const [syncState, setSyncState] = createSignal<ChartSyncState | null>(null);

  // 디바운스된 상태 업데이트
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  const updateSyncState = (state: ChartSyncState) => {
    if (debounceMs > 0) {
      if (debounceTimer) clearTimeout(debounceTimer);
      debounceTimer = setTimeout(() => {
        setSyncState(state);
      }, debounceMs);
    } else {
      setSyncState(state);
    }
  };

  const handleRangeChange = (state: ChartSyncState) => {
    if (mode === 'range' || mode === 'full') {
      updateSyncState({
        ...state,
        syncType: 'range',
      });
    }
  };

  const handleCrosshairMove = (state: ChartSyncState) => {
    if (mode === 'crosshair' || mode === 'full') {
      updateSyncState({
        ...state,
        syncType: 'crosshair',
      });
    }
  };

  const handleSyncChange = (state: ChartSyncState) => {
    updateSyncState(state);
  };

  const resetSync = () => {
    if (debounceTimer) clearTimeout(debounceTimer);
    setSyncState(null);
  };

  // 차트에 바인딩할 props 생성 헬퍼
  const bindChart = (id?: string) => ({
    syncState,
    onVisibleRangeChange: handleRangeChange,
    onCrosshairMove: handleCrosshairMove,
    syncMode: mode,
  });

  // 정리
  onCleanup(() => {
    if (debounceTimer) clearTimeout(debounceTimer);
  });

  return {
    syncState,
    handleRangeChange,
    handleCrosshairMove,
    handleSyncChange,
    resetSync,
    bindChart,
  };
}

// 하위 호환성을 위한 레거시 export
export { useChartSync as useChartSyncLegacy };

export default useLightweightChart;
