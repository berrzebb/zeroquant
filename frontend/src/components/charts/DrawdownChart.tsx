/**
 * Drawdown 차트 컴포넌트
 *
 * 포트폴리오 드로다운을 히스토그램으로 시각화합니다.
 * BaseChart를 기반으로 하여 코드 중복을 최소화합니다.
 *
 * @example
 * ```tsx
 * // 기본 사용
 * <DrawdownChart data={drawdownData()} height={200} />
 *
 * // 동기화 사용
 * const sync = useChartSync();
 * <DrawdownChart data={drawdownData()} {...sync.bindChart('drawdown')} />
 * ```
 */

import { type Component } from 'solid-js';
import { BaseChart, type BaseDataPoint } from './BaseChart';
import type { ChartSyncState } from '../../utils/chartUtils';

// ==================== 타입 정의 ====================

export interface DrawdownDataPoint {
  time: string | number;
  value: number;
}

export interface DrawdownChartProps {
  /** 드로다운 데이터 */
  data: DrawdownDataPoint[];
  /** 차트 높이 (픽셀) */
  height?: number;
  /** 최대 드로다운 라인 (선택사항, 추후 구현) */
  maxDrawdownLine?: number;
  /** 커스텀 색상 */
  colors?: {
    background?: string;
    text?: string;
    grid?: string;
    drawdownColor?: string;
    maxDrawdownColor?: string;
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
 * Drawdown 히스토그램 차트
 *
 * BaseChart를 래핑하여 드로다운 전용 설정을 적용합니다.
 * - 퍼센트 포맷
 * - 음수 값 색상 (빨간색)
 */
export const DrawdownChart: Component<DrawdownChartProps> = (props) => {
  // 색상 변환 (기존 API 호환) - undefined 필드는 제외하여 기본값 사용
  const getColors = () => {
    const colors: Record<string, string> = {
      negative: props.colors?.drawdownColor || '#ef4444',
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
      priceFormat="percent"
      syncState={props.syncState}
      syncMode={props.syncMode || 'full'}
      onVisibleRangeChange={props.onVisibleRangeChange}
      onCrosshairMove={props.onCrosshairMove}
      rightPriceScale={{
        scaleMargins: { top: 0.1, bottom: 0.1 },
      }}
    />
  );
};

export default DrawdownChart;
