/**
 * ScoreHistoryChart - GlobalScore 히스토리 차트 컴포넌트
 *
 * GlobalScore 시간별 변화를 라인 차트와 변화량 히스토그램으로 시각화합니다.
 * 공통 유틸리티와 useLightweightChart 훅을 사용하여 코드 중복을 최소화합니다.
 *
 * @example
 * ```tsx
 * // 기본 사용
 * <ScoreHistoryChart data={scoreHistory} height={200} />
 *
 * // 동기화 사용
 * const sync = useChartSync();
 * <ScoreHistoryChart data={scoreHistory} {...sync.bindChart('score')} showRank />
 * ```
 */

import { createEffect, on, createMemo } from 'solid-js';
import { LineSeries, HistogramSeries } from 'lightweight-charts';
import type { IChartApi, ISeriesApi, LineData, HistogramData } from 'lightweight-charts';
import { useLightweightChart } from '../../hooks/useLightweightChart';
import {
  sortByTime,
  type ChartColors,
  type ChartSyncState,
} from '../../utils/chartUtils';
import type { ScoreHistorySummary } from '../../api/client';

// ==================== 타입 정의 ====================

export interface ScoreHistoryChartProps {
  /** 점수 히스토리 데이터 */
  data: ScoreHistorySummary[];
  /** 차트 높이 (픽셀) */
  height?: number;
  /** 랭크 표시 여부 */
  showRank?: boolean;
  /** 커스텀 색상 */
  colors?: {
    background?: string;
    text?: string;
    grid?: string;
    scoreColor?: string;
    rankColor?: string;
    positiveChange?: string;
    negativeChange?: string;
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

// ==================== 상수 ====================

// RouteState에 따른 색상 매핑
const ROUTE_STATE_COLORS: Record<string, string> = {
  'Attack': 'rgba(34, 197, 94, 0.15)',    // 녹색 (적극적 진입)
  'Armed': 'rgba(59, 130, 246, 0.15)',    // 파랑 (준비 상태)
  'Weak': 'rgba(234, 179, 8, 0.15)',      // 노랑 (약세)
  'Exit': 'rgba(239, 68, 68, 0.15)',      // 빨강 (청산)
  'Wait': 'rgba(107, 114, 128, 0.1)',     // 회색 (관망)
};

// ==================== 컴포넌트 ====================

export function ScoreHistoryChart(props: ScoreHistoryChartProps) {
  // 시리즈 참조
  let scoreSeries: ISeriesApi<'Line'> | undefined;
  let changeHistogram: ISeriesApi<'Histogram'> | undefined;
  let chartInstance: IChartApi | undefined;

  // 기본 색상
  const defaultScoreColor = '#8b5cf6';      // 보라색 (Global Score)
  const defaultPositiveChange = '#22c55e';  // 녹색 (점수 상승)
  const defaultNegativeChange = '#ef4444';  // 빨강 (점수 하락)

  // 색상 병합 (undefined 값은 제외하여 기본값이 유지되도록 함)
  const getChartColors = (): Partial<ChartColors> => {
    const colors: Partial<ChartColors> = {
      line: props.colors?.scoreColor || defaultScoreColor,
      positive: props.colors?.positiveChange || defaultPositiveChange,
      negative: props.colors?.negativeChange || defaultNegativeChange,
    };
    if (props.colors?.background) colors.background = props.colors.background;
    if (props.colors?.text) colors.text = props.colors.text;
    if (props.colors?.grid) colors.grid = props.colors.grid;
    return colors;
  };

  // 차트 데이터 업데이트 함수
  const updateChartData = (data: ScoreHistorySummary[]) => {
    if (!scoreSeries || !changeHistogram) return;

    // sortByTime은 time 필드를 가진 객체에 작동하므로, 변환 필요
    const mappedData = data.map(d => ({
      ...d,
      time: d.score_date,
    }));
    const sortedData = sortByTime(mappedData);

    const positiveColor = props.colors?.positiveChange || defaultPositiveChange;
    const negativeColor = props.colors?.negativeChange || defaultNegativeChange;

    // Global Score 라인 데이터
    const scoreData: LineData[] = sortedData
      .filter(d => d.global_score !== null && d.global_score !== undefined)
      .map(d => ({
        time: d.score_date,
        value: d.global_score!,
      }));

    // 점수 변화 히스토그램 데이터
    const changeData: HistogramData[] = sortedData
      .filter(d => d.score_change !== null && d.score_change !== undefined)
      .map(d => ({
        time: d.score_date,
        value: d.score_change!,
        color: d.score_change! >= 0 ? positiveColor : negativeColor,
      }));

    scoreSeries.setData(scoreData);
    changeHistogram.setData(changeData);
    chartInstance?.timeScale().fitContent();
  };

  // 통합 차트 훅 사용
  const { setContainerRef, chart, chartId, colors } = useLightweightChart({
    height: props.height || 200,
    colors: getChartColors(),
    syncState: props.syncState,
    syncMode: props.syncMode || 'full',
    onVisibleRangeChange: props.onVisibleRangeChange,
    onCrosshairMove: props.onCrosshairMove,
    rightPriceScale: {
      scaleMargins: { top: 0.1, bottom: 0.3 }, // 히스토그램 공간 확보
    },
    timeScale: {
      timeVisible: true,
    },
    onReady: (chartApi, mergedColors) => {
      chartInstance = chartApi;

      // Global Score 라인 차트
      scoreSeries = chartApi.addSeries(LineSeries, {
        color: mergedColors.line,
        lineWidth: 2,
        priceFormat: {
          type: 'custom',
          formatter: (price: number) => price.toFixed(1),
        },
      });

      // 점수 변화 히스토그램 (하단)
      changeHistogram = chartApi.addSeries(HistogramSeries, {
        priceFormat: {
          type: 'custom',
          formatter: (price: number) => (price >= 0 ? '+' : '') + price.toFixed(1),
        },
        priceScaleId: 'change',
      });

      // 히스토그램용 별도 스케일
      chartApi.priceScale('change').applyOptions({
        scaleMargins: {
          top: 0.8,
          bottom: 0,
        },
      });

      // 초기 데이터 설정
      if (props.data && props.data.length > 0) {
        updateChartData(props.data);
      }
    },
  });

  // 데이터 변경 감지
  createEffect(
    on(
      () => props.data,
      (data) => {
        if (data && data.length > 0) {
          updateChartData(data);
        }
      }
    )
  );

  // 마지막 점수 및 변화량 계산
  const lastData = createMemo(() => {
    if (!props.data || props.data.length === 0) return null;
    const mappedData = props.data.map(d => ({
      ...d,
      time: d.score_date,
    }));
    const sorted = sortByTime(mappedData);
    return sorted[sorted.length - 1];
  });

  // 색상 getter
  const chartColors = createMemo(() => ({
    scoreColor: props.colors?.scoreColor || defaultScoreColor,
    positiveChange: props.colors?.positiveChange || defaultPositiveChange,
    negativeChange: props.colors?.negativeChange || defaultNegativeChange,
  }));

  return (
    <div class="relative">
      {/* 차트 컨테이너 */}
      <div
        ref={setContainerRef}
        class="w-full rounded-lg overflow-hidden"
        style={{ height: `${props.height || 200}px` }}
      />

      {/* 범례 및 현재 값 표시 */}
      <div class="absolute top-2 left-2 flex gap-4 text-xs">
        <div class="flex items-center gap-1">
          <div class="w-3 h-0.5" style={{ background: chartColors().scoreColor }} />
          <span class="text-gray-400">Global Score</span>
          {lastData()?.global_score !== null && lastData()?.global_score !== undefined && (
            <span class="font-semibold text-white ml-1">
              {lastData()?.global_score?.toFixed(1)}
            </span>
          )}
        </div>

        {lastData()?.score_change !== null && lastData()?.score_change !== undefined && (
          <div class="flex items-center gap-1">
            <span
              class="font-semibold"
              style={{
                color: (lastData()?.score_change ?? 0) >= 0
                  ? chartColors().positiveChange
                  : chartColors().negativeChange
              }}
            >
              {(lastData()?.score_change ?? 0) >= 0 ? '+' : ''}
              {lastData()?.score_change?.toFixed(1)}
            </span>
          </div>
        )}

        {lastData()?.route_state && (
          <div class="flex items-center gap-1">
            <div
              class="px-1.5 py-0.5 rounded text-xs font-medium"
              style={{
                background: ROUTE_STATE_COLORS[lastData()?.route_state ?? 'Wait'] ?? ROUTE_STATE_COLORS.Wait,
                color: '#fff'
              }}
            >
              {lastData()?.route_state}
            </div>
          </div>
        )}
      </div>

      {/* Rank 표시 (선택적) */}
      {props.showRank && lastData()?.rank !== null && lastData()?.rank !== undefined && (
        <div class="absolute top-2 right-2 text-xs">
          <span class="text-gray-400">Rank: </span>
          <span class="font-semibold text-amber-500">#{lastData()?.rank}</span>
          {lastData()?.rank_change !== null && lastData()?.rank_change !== undefined && lastData()?.rank_change !== 0 && (
            <span
              class="ml-1"
              style={{
                color: (lastData()?.rank_change ?? 0) < 0  // 랭크는 낮을수록 좋음
                  ? chartColors().positiveChange
                  : chartColors().negativeChange
              }}
            >
              ({(lastData()?.rank_change ?? 0) < 0 ? '' : '+'}{lastData()?.rank_change})
            </span>
          )}
        </div>
      )}
    </div>
  );
}

export default ScoreHistoryChart;
