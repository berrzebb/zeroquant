/**
 * 손익 막대 차트 컴포넌트
 *
 * 손익 데이터를 히스토그램으로 시각화합니다.
 * - 양수 손익: 초록색
 * - 음수 손익: 빨간색
 *
 * BaseChart를 기반으로 하여 코드 중복을 최소화합니다.
 *
 * @example
 * ```tsx
 * // 기본 사용
 * <PnLBarChart data={pnlData()} height={200} />
 *
 * // 동기화 사용
 * const sync = useChartSync();
 * <PnLBarChart data={pnlData()} {...sync.bindChart('pnl')} />
 * ```
 */

import { type Component } from 'solid-js';
import { BaseChart, type BaseDataPoint } from './BaseChart';
import type { ChartSyncState } from '../../utils/chartUtils';

// ==================== 타입 정의 ====================

export interface PnLDataPoint {
  time: string; // YYYY-MM-DD 형식
  value: number;
  color?: string;
}

export interface PnLBarChartProps {
  /** 손익 데이터 */
  data: PnLDataPoint[];
  /** 차트 높이 (픽셀) */
  height?: number;
  /** 커스텀 색상 */
  colors?: {
    background?: string;
    text?: string;
    grid?: string;
    positiveColor?: string;
    negativeColor?: string;
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
 * P&L 히스토그램 차트
 *
 * BaseChart를 래핑하여 손익 전용 설정을 적용합니다.
 * - colorByValue로 양수/음수 자동 색상 구분
 * - 정수 포맷
 */
export const PnLBarChart: Component<PnLBarChartProps> = (props) => {
  // 색상 변환 (기존 API 호환) - undefined 필드는 제외하여 기본값 사용
  const getColors = () => {
    const colors: Record<string, string> = {
      positive: props.colors?.positiveColor || '#22c55e',
      negative: props.colors?.negativeColor || '#ef4444',
    };
    if (props.colors?.background) colors.background = props.colors.background;
    if (props.colors?.text) colors.text = props.colors.text;
    if (props.colors?.grid) colors.grid = props.colors.grid;
    return colors;
  };

  return (
    <BaseChart
      type="histogram"
      data={props.data as BaseDataPoint[]}
      height={props.height || 200}
      colors={getColors()}
      colorByValue
      syncState={props.syncState}
      syncMode={props.syncMode || 'full'}
      onVisibleRangeChange={props.onVisibleRangeChange}
      onCrosshairMove={props.onCrosshairMove}
      rightPriceScale={{
        scaleMargins: { top: 0.1, bottom: 0.1 },
      }}
      timeScale={{
        timeVisible: false,
      }}
    />
  );
};

export default PnLBarChart;
