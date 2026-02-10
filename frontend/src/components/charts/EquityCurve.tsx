/**
 * Equity Curve 차트 컴포넌트
 *
 * 포트폴리오 가치 변화를 Area 차트로 시각화합니다.
 * 선택적으로 벤치마크 라인을 오버레이할 수 있습니다.
 *
 * BaseChart를 기반으로 하여 코드 중복을 최소화합니다.
 *
 * @example
 * ```tsx
 * // 기본 사용
 * <EquityCurve data={equityData()} height={300} />
 *
 * // 벤치마크 오버레이
 * <EquityCurve
 *   data={equityData()}
 *   showBenchmark
 *   benchmarkData={spyData()}
 * />
 *
 * // 동기화 사용
 * const sync = useChartSync();
 * <EquityCurve data={equityData()} {...sync.bindChart('equity')} />
 * ```
 */

import { createEffect, on, type Component } from 'solid-js';
import type { IChartApi, ISeriesApi, LineData } from 'lightweight-charts';
import { LineSeries } from 'lightweight-charts';
import { BaseChart, type BaseDataPoint } from './BaseChart';
import { sortByTime, type ChartSyncState } from '../../utils/chartUtils';

// ==================== 타입 정의 ====================

export interface EquityDataPoint {
  time: string | number;
  value: number;
}

// 차트 동기화를 위한 공유 타입 (하위 호환성)
export type { ChartSyncState };

export interface EquityCurveProps {
  /** 차트 데이터 */
  data: EquityDataPoint[];
  /** 차트 높이 (픽셀) */
  height?: number;
  /** 벤치마크 표시 여부 */
  showBenchmark?: boolean;
  /** 벤치마크 데이터 */
  benchmarkData?: EquityDataPoint[];
  /** 커스텀 색상 */
  colors?: {
    background?: string;
    text?: string;
    grid?: string;
    equityColor?: string;
    benchmarkColor?: string;
    positiveArea?: string;
    negativeArea?: string;
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

/**
 * Equity Curve (Area) 차트
 *
 * BaseChart를 래핑하여 포트폴리오 가치 변화 전용 설정을 적용합니다.
 * - Area 차트 타입
 * - 선택적 벤치마크 라인 오버레이
 */
export const EquityCurve: Component<EquityCurveProps> = (props) => {
  // 벤치마크 시리즈 참조
  let benchmarkSeries: ISeriesApi<'Line'> | undefined;

  // 색상 변환 (기존 API 호환) - undefined 필드는 제외하여 기본값 사용
  const getColors = () => {
    const colors: Record<string, string> = {
      line: props.colors?.equityColor || '#3b82f6',
      areaTop: props.colors?.positiveArea || 'rgba(59, 130, 246, 0.2)',
      areaBottom: 'transparent',
    };
    if (props.colors?.background) colors.background = props.colors.background;
    if (props.colors?.text) colors.text = props.colors.text;
    if (props.colors?.grid) colors.grid = props.colors.grid;
    return colors;
  };

  const getBenchmarkColor = () => props.colors?.benchmarkColor || '#9ca3af';

  // 벤치마크 시리즈 생성/업데이트
  const handleChartReady = (chart: IChartApi) => {
    // 벤치마크 라인 추가
    if (props.showBenchmark) {
      benchmarkSeries = chart.addSeries(LineSeries, {
        color: getBenchmarkColor(),
        lineWidth: 1,
        lineStyle: 2, // Dashed
        lastValueVisible: true,
        priceLineVisible: false,
      });

      // 초기 벤치마크 데이터 설정
      if (props.benchmarkData && props.benchmarkData.length > 0) {
        const sortedBenchmark = sortByTime(props.benchmarkData);
        benchmarkSeries.setData(sortedBenchmark as LineData[]);
      }
    }
  };

  // 벤치마크 데이터 업데이트
  createEffect(
    on(
      () => props.benchmarkData,
      (data) => {
        if (!benchmarkSeries || !data || data.length === 0) return;
        const sortedData = sortByTime(data);
        benchmarkSeries.setData(sortedData as LineData[]);
      }
    )
  );

  return (
    <BaseChart
      type="area"
      data={props.data as BaseDataPoint[]}
      height={props.height || 300}
      colors={getColors()}
      syncState={props.syncState}
      syncMode={props.syncMode || 'full'}
      onVisibleRangeChange={props.onVisibleRangeChange}
      onCrosshairMove={props.onCrosshairMove}
      onChartReady={handleChartReady}
    />
  );
};

export default EquityCurve;
