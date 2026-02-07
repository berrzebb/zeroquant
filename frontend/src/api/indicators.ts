/**
 * 기술적 지표 API 클라이언트.
 *
 * Dashboard 차트에서 사용할 기술적 지표 데이터를 가져오는 API 함수들입니다.
 */

import api from './client';

// ==================== 타입 정의 ====================

/** 지표 카테고리 */
export type IndicatorCategory = '추세' | '모멘텀' | '변동성';

/** 지표 정보 */
export interface IndicatorInfo {
  /** 지표 ID (sma, ema, rsi, macd, bollinger, stochastic, atr) */
  id: string;
  /** 지표 이름 (한글) */
  name: string;
  /** 지표 설명 */
  description: string;
  /** 지표 카테고리 */
  category: IndicatorCategory;
  /** 기본 파라미터 */
  defaultParams: Record<string, number>;
  /** 오버레이 여부 (true: 가격 차트 위에 표시, false: 별도 패널에 표시) */
  overlay: boolean;
}

/** 사용 가능한 지표 목록 응답 */
export interface AvailableIndicatorsResponse {
  indicators: IndicatorInfo[];
}

/** 지표 데이터 포인트 */
export interface IndicatorPoint {
  /** 타임스탬프 (밀리초) */
  x: number;
  /** 값 (문자열, null 가능) */
  y: string | null;
}

/** 지표 시리즈 데이터 */
export interface IndicatorSeries {
  /** 시리즈 이름 (예: "macd", "signal", "histogram") */
  name: string;
  /** 데이터 포인트 배열 */
  data: IndicatorPoint[];
  /** 차트 색상 */
  color?: string;
  /** 시리즈 타입 (line, bar, area) */
  seriesType: 'line' | 'bar' | 'area';
}

/** 단일 지표 데이터 응답 */
export interface IndicatorDataResponse {
  /** 지표 ID */
  indicator: string;
  /** 지표 이름 */
  name: string;
  /** 심볼 */
  symbol: string;
  /** 사용된 파라미터 */
  params: Record<string, unknown>;
  /** 데이터 시리즈 배열 */
  series: IndicatorSeries[];
}

/** 다중 지표 계산 응답 */
export interface CalculateIndicatorsResponse {
  /** 심볼 */
  symbol: string;
  /** 기간 */
  period: string;
  /** 지표별 결과 */
  results: IndicatorDataResponse[];
}

/** 지표 설정 (다중 지표 계산 요청용) */
export interface IndicatorConfig {
  /** 지표 타입 (sma, ema, rsi, macd, bollinger, stochastic, atr) */
  type: string;
  /** 지표 파라미터 */
  params: Record<string, number>;
  /** 차트 색상 (선택적) */
  color?: string;
  /** 표시 이름 (선택적) */
  name?: string;
}

/** 다중 지표 계산 요청 */
export interface CalculateIndicatorsRequest {
  /** 심볼 */
  symbol: string;
  /** 기간 (1d, 1w, 1m, 3m, 6m, 1y, all) */
  period?: string;
  /** 계산할 지표 목록 */
  indicators: IndicatorConfig[];
}

// ==================== 개별 지표 파라미터 타입 ====================

/** SMA 파라미터 */
export interface SmaParams {
  symbol: string;
  period?: string;
  sma_period?: number;
}

/** EMA 파라미터 */
export interface EmaParams {
  symbol: string;
  period?: string;
  ema_period?: number;
}

/** RSI 파라미터 */
export interface RsiParams {
  symbol: string;
  period?: string;
  rsi_period?: number;
}

/** MACD 파라미터 */
export interface MacdParams {
  symbol: string;
  period?: string;
  fast_period?: number;
  slow_period?: number;
  signal_period?: number;
}

/** 볼린저 밴드 파라미터 */
export interface BollingerParams {
  symbol: string;
  period?: string;
  bb_period?: number;
  std_dev?: number;
}

/** 스토캐스틱 파라미터 */
export interface StochasticParams {
  symbol: string;
  period?: string;
  k_period?: number;
  d_period?: number;
}

/** ATR 파라미터 */
export interface AtrParams {
  symbol: string;
  period?: string;
  atr_period?: number;
}

// ==================== API 함수 ====================

/**
 * 사용 가능한 지표 목록을 가져옵니다.
 */
export const getAvailableIndicators = async (): Promise<IndicatorInfo[]> => {
  const response = await api.get<AvailableIndicatorsResponse>('/analytics/indicators');
  // API 응답의 snake_case를 camelCase로 변환
  return response.data.indicators.map((ind) => ({
    id: ind.id,
    name: ind.name,
    description: ind.description,
    category: ind.category as IndicatorCategory,
    defaultParams: ind.default_params as Record<string, number>,
    overlay: ind.overlay,
  }));
};

/**
 * SMA 지표 데이터를 가져옵니다.
 */
export const getSmaIndicator = async (params: SmaParams): Promise<IndicatorDataResponse> => {
  const response = await api.get<IndicatorDataResponse>('/analytics/indicators/sma', { params });
  return transformIndicatorResponse(response.data);
};

/**
 * EMA 지표 데이터를 가져옵니다.
 */
export const getEmaIndicator = async (params: EmaParams): Promise<IndicatorDataResponse> => {
  const response = await api.get<IndicatorDataResponse>('/analytics/indicators/ema', { params });
  return transformIndicatorResponse(response.data);
};

/**
 * RSI 지표 데이터를 가져옵니다.
 */
export const getRsiIndicator = async (params: RsiParams): Promise<IndicatorDataResponse> => {
  const response = await api.get<IndicatorDataResponse>('/analytics/indicators/rsi', { params });
  return transformIndicatorResponse(response.data);
};

/**
 * MACD 지표 데이터를 가져옵니다.
 */
export const getMacdIndicator = async (params: MacdParams): Promise<IndicatorDataResponse> => {
  const response = await api.get<IndicatorDataResponse>('/analytics/indicators/macd', { params });
  return transformIndicatorResponse(response.data);
};

/**
 * 볼린저 밴드 지표 데이터를 가져옵니다.
 */
export const getBollingerIndicator = async (params: BollingerParams): Promise<IndicatorDataResponse> => {
  const response = await api.get<IndicatorDataResponse>('/analytics/indicators/bollinger', { params });
  return transformIndicatorResponse(response.data);
};

/**
 * 스토캐스틱 지표 데이터를 가져옵니다.
 */
export const getStochasticIndicator = async (params: StochasticParams): Promise<IndicatorDataResponse> => {
  const response = await api.get<IndicatorDataResponse>('/analytics/indicators/stochastic', { params });
  return transformIndicatorResponse(response.data);
};

/**
 * ATR 지표 데이터를 가져옵니다.
 */
export const getAtrIndicator = async (params: AtrParams): Promise<IndicatorDataResponse> => {
  const response = await api.get<IndicatorDataResponse>('/analytics/indicators/atr', { params });
  return transformIndicatorResponse(response.data);
};

/**
 * 여러 지표를 한 번에 계산합니다.
 */
export const calculateIndicators = async (request: CalculateIndicatorsRequest): Promise<CalculateIndicatorsResponse> => {
  const response = await api.post<CalculateIndicatorsResponse>('/analytics/indicators/calculate', request);
  return {
    symbol: response.data.symbol,
    period: response.data.period,
    results: response.data.results.map(transformIndicatorResponse),
  };
};

// ==================== 유틸리티 함수 ====================

/**
 * API 응답을 변환합니다 (snake_case -> camelCase).
 */
function transformIndicatorResponse(data: any): IndicatorDataResponse {
  return {
    indicator: data.indicator,
    name: data.name,
    symbol: data.symbol,
    params: data.params,
    series: data.series.map((s: any) => ({
      name: s.name,
      data: s.data,
      color: s.color,
      seriesType: s.series_type as 'line' | 'bar' | 'area',
    })),
  };
}

/**
 * 지표 데이터를 LightweightCharts 형식으로 변환합니다.
 */
export function toChartData(series: IndicatorSeries): { time: number; value: number }[] {
  return series.data
    .filter((point) => point.y !== null)
    .map((point) => ({
      time: point.x / 1000, // 밀리초 -> 초
      value: parseFloat(point.y!),
    }));
}

/**
 * 지표 ID로 지표 정보를 찾습니다.
 */
export function findIndicatorById(indicators: IndicatorInfo[], id: string): IndicatorInfo | undefined {
  return indicators.find((ind) => ind.id === id);
}

/**
 * 카테고리별로 지표를 그룹화합니다.
 */
export function groupIndicatorsByCategory(indicators: IndicatorInfo[]): Record<IndicatorCategory, IndicatorInfo[]> {
  const grouped: Record<IndicatorCategory, IndicatorInfo[]> = {
    '추세': [],
    '모멘텀': [],
    '변동성': [],
  };

  for (const indicator of indicators) {
    if (grouped[indicator.category]) {
      grouped[indicator.category].push(indicator);
    }
  }

  return grouped;
}

// ==================== 지표 타입 분류 ====================

/** 오버레이 지표 목록 (가격 차트 위에 표시) */
const OVERLAY_INDICATORS = ['sma', 'ema', 'bollinger'];

/** 별도 패널 지표 목록 (가격 차트 아래에 별도 표시) */
const SEPARATE_PANEL_INDICATORS = ['rsi', 'macd', 'stochastic', 'atr'];

/** 지표별 Y축 범위 (별도 패널 지표용) */
export const INDICATOR_SCALE_RANGES: Record<string, { min: number; max: number; levels?: number[] }> = {
  rsi: { min: 0, max: 100, levels: [30, 70] },
  stochastic: { min: 0, max: 100, levels: [20, 80] },
  macd: { min: -100, max: 100 }, // 동적으로 조정됨
  atr: { min: 0, max: 100 }, // 동적으로 조정됨
};

/** 지표별 기본 색상 */
export const INDICATOR_DEFAULT_COLORS: Record<string, string | Record<string, string>> = {
  sma: '#3b82f6',
  ema: '#8b5cf6',
  rsi: '#f59e0b',
  macd: { macd: '#3b82f6', signal: '#ef4444', histogram: '#22c55e' },
  bollinger: { upper: '#6366f1', middle: '#a855f7', lower: '#6366f1' },
  stochastic: { k: '#3b82f6', d: '#ef4444' },
  atr: '#10b981',
};

/**
 * 지표가 오버레이 타입인지 확인합니다.
 * @param indicatorId 지표 ID
 * @returns true면 가격 차트 위에 오버레이, false면 별도 패널에 표시
 */
export function isOverlayIndicator(indicatorId: string): boolean {
  return OVERLAY_INDICATORS.includes(indicatorId.toLowerCase());
}

/**
 * 지표가 별도 패널 타입인지 확인합니다.
 * @param indicatorId 지표 ID
 * @returns true면 별도 패널에 표시
 */
export function isSeparatePanelIndicator(indicatorId: string): boolean {
  return SEPARATE_PANEL_INDICATORS.includes(indicatorId.toLowerCase());
}

/**
 * 지표의 Y축 범위를 가져옵니다.
 * @param indicatorId 지표 ID
 * @returns Y축 범위 (min, max, levels)
 */
export function getIndicatorScaleRange(indicatorId: string): { min: number; max: number; levels?: number[] } | undefined {
  return INDICATOR_SCALE_RANGES[indicatorId.toLowerCase()];
}

/**
 * 지표의 기본 색상을 가져옵니다.
 * @param indicatorId 지표 ID
 * @param seriesName 시리즈 이름 (MACD, Bollinger 등 다중 시리즈 지표용)
 * @returns 색상 코드
 */
export function getIndicatorDefaultColor(indicatorId: string, seriesName?: string): string {
  const colors = INDICATOR_DEFAULT_COLORS[indicatorId.toLowerCase()];
  if (!colors) return '#3b82f6';

  if (typeof colors === 'string') return colors;

  if (seriesName && colors[seriesName.toLowerCase()]) {
    return colors[seriesName.toLowerCase()];
  }

  // 첫 번째 색상 반환
  return Object.values(colors)[0];
}

// 기본 내보내기
export default {
  getAvailableIndicators,
  getSmaIndicator,
  getEmaIndicator,
  getRsiIndicator,
  getMacdIndicator,
  getBollingerIndicator,
  getStochasticIndicator,
  getAtrIndicator,
  calculateIndicators,
  isOverlayIndicator,
  isSeparatePanelIndicator,
  getIndicatorScaleRange,
  getIndicatorDefaultColor,
};
