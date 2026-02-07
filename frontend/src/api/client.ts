import axios from 'axios';
import type {
  Position,
  Order,
  PortfolioSummary,
  MarketStatus,
  SupportedExchange,
  ExchangeCredential,
  TelegramSettings,
} from '../types';

// SDUI 타입 import
import type {
  StrategyUISchema,
  SchemaFragment,
  GetFragmentsResponse,
} from '../types/sdui';

// 자동 생성된 타입 import (ts-rs)
import type {
  // Journal 타입
  JournalPositionResponse,
  JournalPositionsResponse,
  ExecutionResponse,
  ExecutionsListResponse,
  PnLSummaryResponse,
  PositionsSummary,
  DailyPnLItem,
  DailyPnLResponse,
  SymbolPnLItem,
  SymbolPnLResponse,
  SyncResponse as JournalSyncResponseGenerated,
  ListExecutionsQuery,
  // Screening 타입
  ScreeningRequest as GeneratedScreeningRequest,
  ScreeningResponse as GeneratedScreeningResponse,
  ScreeningResultDto as GeneratedScreeningResultDto,
  MomentumQuery as GeneratedMomentumQuery,
  MomentumResponse as GeneratedMomentumResponse,
  MomentumResultDto as GeneratedMomentumResultDto,
  // Ranking 타입
  RankingResponse as GeneratedRankingResponse,
  RankedSymbol as GeneratedRankedSymbol,
  RankingQuery as GeneratedRankingQuery,
  FilterInfo,
  // Strategies 타입
  StrategyListItem,
  StrategiesListResponse,
  CreateStrategyRequest as GeneratedCreateStrategyRequest,
  CreateStrategyResponse as GeneratedCreateStrategyResponse,
  CloneStrategyRequest as GeneratedCloneStrategyRequest,
  CloneStrategyResponse as GeneratedCloneStrategyResponse,
  // Backtest 타입
  BacktestableStrategy,
  BacktestStrategiesResponse as GeneratedBacktestStrategiesResponse,
  BacktestMetricsResponse,
  TradeResultItem,
} from '../types/generated';

// ==================== 자동 생성 타입 재export (하위 호환성) ====================
// Journal
export type JournalPosition = JournalPositionResponse;
export type JournalExecution = ExecutionResponse;
export type { JournalPositionsResponse } from '../types/generated/journal';
export type JournalExecutionsResponse = ExecutionsListResponse;
export type JournalPnLSummary = PnLSummaryResponse;
export type { PositionsSummary, DailyPnLItem, DailyPnLResponse, SymbolPnLItem, SymbolPnLResponse } from '../types/generated/journal';
export type { ListExecutionsQuery };  // bigint 이슈로 ExecutionFilter와 별도 유지
// Screening
export type ScreeningResultDto = GeneratedScreeningResultDto;
export type ScreeningResponse = GeneratedScreeningResponse;
export type MomentumResultDto = GeneratedMomentumResultDto;
export type MomentumResponse = GeneratedMomentumResponse;
// Ranking
export type RankedSymbol = GeneratedRankedSymbol;
export type RankingApiResponse = GeneratedRankingResponse;
// Strategies
export type Strategy = StrategyListItem;
// Backtest
export type BacktestStrategy = BacktestableStrategy;
export type { BacktestMetricsResponse };  // 원본 생성 타입도 export

const api = axios.create({
  baseURL: '/api/v1',
  headers: {
    'Content-Type': 'application/json',
  },
});

// Add auth token to requests
api.interceptors.request.use((config) => {
  const token = localStorage.getItem('auth_token');
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// ==================== 헬스 체크 ====================

export const healthCheck = async () => {
  const response = await api.get('/health');
  return response.data;
};

// ==================== 활성 계정 관리 ====================

/** 활성 계정 정보 */
export interface ActiveAccount {
  credential_id: string | null;
  exchange_id: string | null;
  display_name: string | null;
  is_testnet: boolean;
}

/** 활성 계정 조회 */
export const getActiveAccount = async (): Promise<ActiveAccount> => {
  const response = await api.get('/credentials/active');
  return response.data;
};

/** 활성 계정 설정 */
export const setActiveAccount = async (credentialId: string | null): Promise<{ success: boolean; message: string }> => {
  const response = await api.put('/credentials/active', { credential_id: credentialId });
  return response.data;
};

// ==================== 포트폴리오 ====================

/** 포트폴리오 요약 조회 (활성 계정 기준) */
export const getPortfolioSummary = async (credentialId?: string): Promise<PortfolioSummary> => {
  const params = credentialId ? { credential_id: credentialId } : {};
  const response = await api.get('/portfolio/summary', { params });
  return response.data;
};

/** 거래소 중립적 계좌 정보 */
export interface AccountInfo {
  cashBalance: string;
  totalEvalAmount: string;
  totalProfitLoss: string;
  holdingsCount: number;
  currency: string;
  market: string;
}

/** 잔고 응답 */
export interface BalanceResponse {
  account: AccountInfo;
  totalValue: string;
}

/** 잔고 조회 (활성 계정 기준) */
export const getBalance = async (credentialId?: string): Promise<BalanceResponse> => {
  const params = credentialId ? { credential_id: credentialId } : {};
  const response = await api.get('/portfolio/balance', { params });
  return response.data;
};

export interface HoldingInfo {
  symbol: string;
  displayName?: string;  // "005930(삼성전자)" 형식
  name: string;
  quantity: string;
  avgPrice: string;
  currentPrice: string;
  evalAmount: string;
  profitLoss: string;
  profitLossRate: string;
  market: string;
}

export interface HoldingsResponse {
  /** 전체 보유 종목 (시장 구분은 각 HoldingInfo.market 필드 사용) */
  holdings: HoldingInfo[];
  totalCount: number;
}

/** 보유 종목 조회 (활성 계정 기준) */
export const getHoldings = async (credentialId?: string): Promise<HoldingsResponse> => {
  const params = credentialId ? { credential_id: credentialId } : {};
  const response = await api.get('/portfolio/holdings', { params });
  return response.data;
};

// ==================== 시장 통합 조회 ====================

/** 시장 온도 응답 */
export interface MarketBreadthResponse {
  /** 전체 시장 Above_MA20 비율 (%) */
  all: string;
  /** KOSPI Above_MA20 비율 (%) */
  kospi: string;
  /** KOSDAQ Above_MA20 비율 (%) */
  kosdaq: string;
  /** 시장 온도 (OVERHEAT/NEUTRAL/COLD) */
  temperature: string;
  /** 온도 아이콘 */
  temperatureIcon: string;
  /** 매매 권장사항 */
  recommendation: string;
  /** 계산 시각 (ISO 8601) */
  calculatedAt: string;
}

/** 매크로 환경 응답 */
export interface MacroEnvironmentResponse {
  /** KOSPI 지수 */
  kospi: string;
  /** KOSPI 전일 대비 변동률 (%) */
  kospiChangePct: number;
  /** KOSDAQ 지수 */
  kosdaq: string;
  /** KOSDAQ 전일 대비 변동률 (%) */
  kosdaqChangePct: number;
  /** USD/KRW 환율 */
  usdKrw: string;
  /** USD/KRW 전일 대비 변동률 (%) */
  usdChangePct: number;
  /** VIX 변동성 지수 */
  vix: string;
  /** VIX 전일 대비 변동률 (%) */
  vixChangePct: number;
  /** 나스닥 지수 */
  nasdaq: string;
  /** 나스닥 전일 대비 변동률 (%) */
  nasdaqChangePct: number;
  /** 위험도 수준 (SAFE/CAUTION/WARNING/CRITICAL) */
  riskLevel: string;
  /** 위험도 아이콘 */
  riskIcon: string;
  /** 조정된 EBS 기준 */
  adjustedEbs: number;
  /** 추천 종목 수 제한 */
  recommendationLimit: number;
  /** 요약 메시지 */
  summary: string;
}

/** 시장 통합 조회 응답 */
export interface MarketOverviewResponse {
  /** 한국 시장 상태 */
  kr: MarketStatus;
  /** 미국 시장 상태 */
  us: MarketStatus;
  /** 시장 온도 (데이터 없으면 null) */
  breadth?: MarketBreadthResponse;
  /** 매크로 환경 (데이터 없으면 null) */
  macro?: MacroEnvironmentResponse;
}

/** 시장 통합 조회 (status + breadth + macro) */
export const getMarketOverview = async (): Promise<MarketOverviewResponse> => {
  const response = await api.get('/market/overview');
  return response.data;
};

// 하위 호환 헬퍼 (개별 컴포넌트에서 breadth/macro만 필요할 때)
export const getMarketBreadth = async (): Promise<MarketBreadthResponse> => {
  const overview = await getMarketOverview();
  if (!overview.breadth) throw new Error('Market breadth data not available');
  return overview.breadth;
};

export const getMacroEnvironment = async (): Promise<MacroEnvironmentResponse> => {
  const overview = await getMarketOverview();
  if (!overview.macro) throw new Error('Macro environment data not available');
  return overview.macro;
};

// ==================== 캔들스틱 데이터 ====================

export interface CandleData {
  time: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

export interface KlinesResponse {
  symbol: string;
  timeframe: string;
  data: CandleData[];
}

export const getKlines = async (params: {
  symbol: string;
  timeframe?: string;
  limit?: number;
}): Promise<KlinesResponse> => {
  const response = await api.get('/market/klines', { params });
  return response.data;
};

// ==================== 다중 타임프레임 캔들스틱 (Multi-Timeframe) ====================

/** 다중 타임프레임 캔들 데이터 응답 */
export interface MultiTimeframeKlinesResponse {
  symbol: string;
  klines: Record<string, CandleData[]>;
}

/** 타임프레임 타입 */
export type Timeframe = '1m' | '5m' | '15m' | '30m' | '1h' | '4h' | '1d' | '1w' | '1M';

/**
 * 다중 타임프레임 캔들 데이터 조회.
 *
 * 여러 타임프레임의 캔들 데이터를 한 번에 조회합니다.
 *
 * @param symbol - 심볼 (예: "005930", "BTCUSDT")
 * @param timeframes - 조회할 타임프레임 목록 (예: ["1h", "4h", "1d"])
 * @param limit - 각 타임프레임당 캔들 개수 (기본값: 100)
 * @returns 타임프레임별 캔들 데이터
 *
 * @example
 * ```typescript
 * const data = await fetchMultiTimeframeKlines("BTCUSDT", ["1h", "4h", "1d"], 60);
 * // data.klines["1h"] - 1시간봉 60개
 * // data.klines["4h"] - 4시간봉 60개
 * // data.klines["1d"] - 일봉 60개
 * ```
 */
export const fetchMultiTimeframeKlines = async (
  symbol: string,
  timeframes: Timeframe[],
  limit: number = 100
): Promise<MultiTimeframeKlinesResponse> => {
  const response = await api.get('/market/klines/multi', {
    params: {
      symbol,
      timeframes: timeframes.join(','),
      limit,
    },
  });
  return response.data;
};

// ==================== 현재가 (Ticker) ====================

export interface TickerResponse {
  symbol: string;
  price: string;
  change24h: string;
  change24hPercent: string;
  high24h: string;
  low24h: string;
  volume24h: string;
  timestamp: number;
}

export const getTicker = async (symbol: string): Promise<TickerResponse> => {
  const response = await api.get('/market/ticker', { params: { symbol } });
  return response.data;
};

// ==================== 포지션 & 주문 ====================

export const getPositions = async (): Promise<Position[]> => {
  const response = await api.get('/positions');
  return response.data;
};

export const getOrders = async (): Promise<Order[]> => {
  const response = await api.get('/orders');
  return response.data;
};

export const placeOrder = async (order: {
  symbol: string;
  side: 'Buy' | 'Sell';
  type: 'Market' | 'Limit';
  quantity: number;
  price?: number;
}) => {
  const response = await api.post('/orders', order);
  return response.data;
};

export const cancelOrder = async (orderId: string) => {
  const response = await api.delete(`/orders/${orderId}`);
  return response.data;
};

// ==================== 전략 ====================

export const getStrategies = async (): Promise<Strategy[]> => {
  const response = await api.get('/strategies');
  // API returns { strategies: [...], total: N, running: N }
  return response.data.strategies || [];
};

export const startStrategy = async (strategyId: string) => {
  const response = await api.post(`/strategies/${strategyId}/start`);
  return response.data;
};

export const stopStrategy = async (strategyId: string) => {
  const response = await api.post(`/strategies/${strategyId}/stop`);
  return response.data;
};

/** 다중 타임프레임 설정 */
export interface MultiTimeframeConfig {
  /** Primary 타임프레임 (전략 실행 기준) */
  primary: Timeframe;
  /** Secondary 타임프레임 목록 (추세 확인용) */
  secondary: Array<{ timeframe: Timeframe; candle_count?: number }>;
}

/**
 * 전략 생성 요청.
 * 자동 생성된 타입(GeneratedCreateStrategyRequest)을 재export.
 *
 * 필드:
 * - strategy_type: 전략 타입 (예: "grid_trading", "rsi", "bollinger" 등)
 * - name: 전략 이름 (사용자 지정, 옵션)
 * - parameters: 전략 파라미터
 * - risk_config: 리스크 설정 (옵션)
 * - allocated_capital: 할당 자본 (옵션)
 * - risk_profile: 리스크 프로필 (conservative, default, aggressive, custom)
 * - multiTimeframeConfig: 다중 타임프레임 설정 (옵션)
 */
export type CreateStrategyRequest = GeneratedCreateStrategyRequest;

/**
 * 전략 생성 응답 타입.
 * 자동 생성된 타입(GeneratedCreateStrategyResponse)을 재export.
 */
export type CreateStrategyResponse = GeneratedCreateStrategyResponse;

export const createStrategy = async (request: CreateStrategyRequest): Promise<CreateStrategyResponse> => {
  const response = await api.post('/strategies', request);
  return response.data;
};

export const deleteStrategy = async (strategyId: string) => {
  const response = await api.delete(`/strategies/${strategyId}`);
  return response.data;
};

/**
 * 전략 복제 요청 타입.
 * 자동 생성된 타입(GeneratedCloneStrategyRequest)을 재export.
 *
 * 필드:
 * - new_name: 새 전략 이름
 * - override_params: 파라미터 오버라이드 (옵션)
 * - override_risk_config: 리스크 설정 오버라이드 (옵션)
 * - override_allocated_capital: 할당 자본 오버라이드 (옵션)
 */
export type CloneStrategyRequest = GeneratedCloneStrategyRequest;

/**
 * 전략 복제 응답 타입.
 * 자동 생성된 타입(GeneratedCloneStrategyResponse)을 재export.
 */
export type CloneStrategyResponse = GeneratedCloneStrategyResponse;

/** 전략 복제 */
export const cloneStrategy = async (strategyId: string, newName: string): Promise<CloneStrategyResponse> => {
  const response = await api.post(`/strategies/${strategyId}/clone`, { new_name: newName });
  return response.data;
};

// 전략 상세 응답 타입
export interface StrategyDetailResponse {
  id: string;
  strategy_type: string;
  name: string;
  version: string;
  description: string;
  running: boolean;
  stats: {
    signals_generated: number;
    orders_filled: number;
    market_data_processed: number;
    last_signal_time: string | null;
    last_error: string | null;
    started_at: string | null;
    total_runtime_secs: number;
  };
  state: Record<string, unknown>;
  config: Record<string, unknown>;
  /** 연결된 거래소 계정 ID */
  credentialId?: string;
  /** 대상 시장 (KR/US/CRYPTO) */
  market?: string;
}

// 전략 상세 조회
export const getStrategy = async (strategyId: string): Promise<StrategyDetailResponse> => {
  const response = await api.get(`/strategies/${strategyId}`);
  return response.data;
};

// 전략 설정 업데이트 요청 타입
export interface UpdateStrategyConfigRequest {
  config: Record<string, unknown>;
}

// 전략 설정 업데이트
export const updateStrategyConfig = async (
  strategyId: string,
  config: Record<string, unknown>
): Promise<{ success: boolean; strategy_id: string; action: string; message: string }> => {
  const response = await api.put(`/strategies/${strategyId}/config`, { config });
  return response.data;
};

/** 전략 심볼 목록 업데이트 응답 */
export interface UpdateSymbolsResponse {
  success: boolean;
  strategy_id: string;
  action: string;
  message: string;
}

/** 전략의 심볼 목록 업데이트 */
export const updateStrategySymbols = async (
  strategyId: string,
  symbols: string[]
): Promise<UpdateSymbolsResponse> => {
  const response = await api.put(`/strategies/${strategyId}/symbols`, { symbols });
  return response.data;
};

/** 전략 거래소 계정/시장 업데이트 요청 */
export interface UpdateCredentialRequest {
  credentialId?: string;
  market?: string;
}

/** 전략 거래소 계정/시장 업데이트 응답 */
export interface UpdateCredentialResponse {
  success: boolean;
  strategy_id: string;
  action: string;
  message: string;
}

/** 전략의 거래소 계정 및 시장 업데이트 */
export const updateStrategyCredential = async (
  strategyId: string,
  request: UpdateCredentialRequest
): Promise<UpdateCredentialResponse> => {
  const response = await api.put(`/strategies/${strategyId}/credential`, request);
  return response.data;
};

// ==================== 타임프레임 설정 ====================

/** 타임프레임 설정 응답 */
export interface TimeframeConfigResponse {
  strategy_id: string;
  primary_timeframe: Timeframe;
  is_multi_timeframe: boolean;
  multi_timeframe_config?: MultiTimeframeConfig;
  secondary_timeframes: Timeframe[];
}

/** 전략의 타임프레임 설정 조회 */
export const getStrategyTimeframeConfig = async (
  strategyId: string
): Promise<TimeframeConfigResponse> => {
  const response = await api.get(`/strategies/${strategyId}/timeframes`);
  return response.data;
};

/** 전략의 타임프레임 설정 업데이트 */
export const updateStrategyTimeframeConfig = async (
  strategyId: string,
  config: MultiTimeframeConfig | null
): Promise<TimeframeConfigResponse> => {
  const response = await api.put(`/strategies/${strategyId}/timeframes`, {
    multiTimeframeConfig: config,
  });
  return response.data;
};

// ==================== 백테스트 ====================

export interface BacktestRequest {
  strategy_id: string;
  symbol: string;
  start_date: string;
  end_date: string;
  initial_capital: number;
  commission_rate?: number;
  slippage_rate?: number;
  parameters?: Record<string, unknown>;
  /** 다중 타임프레임 설정 (옵션) */
  multi_timeframe_config?: MultiTimeframeConfig;
}

// 다중 자산 백테스트 요청 (CompoundMomentum, HAA, XAA, Stock Rotation 등)
export interface BacktestMultiRequest {
  strategy_id: string;
  symbols: string[];
  start_date: string;
  end_date: string;
  initial_capital: number;
  commission_rate?: number;
  slippage_rate?: number;
  parameters?: Record<string, unknown>;
  /** 다중 타임프레임 설정 (옵션) */
  multi_timeframe_config?: MultiTimeframeConfig;
}

// 다중 자산 백테스트 결과 (심볼별 데이터 포인트 포함)
export interface BacktestMultiResult extends Omit<BacktestResult, 'symbol'> {
  symbols: string[];
  data_points_by_symbol?: Record<string, number>;
}

// 다중 자산 전략 ID 목록
export const MULTI_ASSET_STRATEGIES = [
  'compound_momentum',
  'haa',
  'xaa',
  'stock_rotation',
  // 추가 다중 자산 전략들
  'all_weather',
  'momentum_power',
  'baa',
  'sector_momentum',
  'dual_momentum',
  'pension_bot',
  'market_cap_top',
];

// ==================== SDUI (Server Driven UI) 타입 ====================

/** UI 필드 타입 */
export type UiFieldType =
  | 'number'
  | 'text'
  | 'select'
  | 'boolean'
  | 'symbol_picker'
  | 'range'
  | 'split_levels'
  | 'symbol_category_group'
  | 'date'
  | 'timeframe';

/** 유효성 검사 규칙 */
export interface UiValidation {
  required?: boolean;
  min?: number;
  max?: number;
  step?: number;
  min_length?: number;
  max_length?: number;
  pattern?: string;
  min_items?: number;
  max_items?: number;
}

/** 선택 옵션 */
export interface UiSelectOption {
  label: string;
  value: unknown;
  description?: string;
}

/** 심볼 카테고리 정의 (자산배분 전략용) */
export interface SymbolCategory {
  /** 카테고리 키 (예: "canary_assets") */
  key: string;
  /** 카테고리 표시 이름 (예: "카나리아 자산") */
  label: string;
  /** 카테고리 설명 */
  description?: string;
  /** 기본 심볼 목록 */
  default_symbols: string[];
  /** 추천 심볼 목록 */
  suggested_symbols: string[];
  /** 최소 선택 수 */
  min_items?: number;
  /** 최대 선택 수 */
  max_items?: number;
  /** 표시 순서 */
  order: number;
}

/** 조건 연산자 */
export type UiConditionOperator = 'equals' | 'not_equals' | 'greater_than' | 'less_than' | 'contains';

/** 조건부 표시 규칙 */
export interface UiCondition {
  field: string;
  operator: UiConditionOperator;
  value: unknown;
}

/** UI 필드 정의 */
export interface UiField {
  key: string;
  label: string;
  field_type: UiFieldType;
  default_value?: unknown;
  placeholder?: string;
  help_text?: string;
  validation: UiValidation;
  options?: UiSelectOption[];
  /** 심볼 카테고리 목록 (symbol_category_group 타입용) */
  symbol_categories?: SymbolCategory[];
  group?: string;
  order: number;
  show_when?: UiCondition;
  unit?: string;
}

/** 필드 그룹 */
export interface UiFieldGroup {
  id: string;
  label: string;
  description?: string;
  order: number;
  collapsed?: boolean;
}

/** 레이아웃 힌트 */
export interface UiLayout {
  columns: number;
}

/** SDUI 스키마 */
export interface UiSchema {
  fields: UiField[];
  groups: UiFieldGroup[];
  layout?: UiLayout;
}

// ==================== 백테스트 전략 ====================

/** 전략 실행 주기 */
export type ExecutionSchedule = 'realtime' | 'on_candle_close' | 'daily' | 'weekly' | 'monthly';

/** 실행 주기 표시명 */
export const ExecutionScheduleLabel: Record<ExecutionSchedule, string> = {
  realtime: '실시간',
  on_candle_close: '캔들 완성 시',
  daily: '일 1회',
  weekly: '주 1회',
  monthly: '월 1회',
};

export interface BacktestStrategy {
  id: string;
  name: string;
  description: string;
  supported_symbols: string[];
  default_params: Record<string, unknown>;
  /** SDUI 스키마 (동적 폼 렌더링용) */
  ui_schema?: UiSchema;
  /** 전략 카테고리 */
  category?: string;
  /** 전략 태그 */
  tags?: string[];
  /** 실행 주기 */
  execution_schedule?: ExecutionSchedule;
  /** 실행 주기 상세 설명 (예: "장 시작 5분 후") */
  schedule_detail?: string;
  /** 작동 방식 상세 설명 */
  how_it_works?: string;
  /** 다중 타임프레임 전략 여부 */
  isMultiTimeframe?: boolean;
  /** 기본 다중 타임프레임 설정 */
  defaultMultiTimeframeConfig?: MultiTimeframeConfig;
}

export interface BacktestStrategiesResponse {
  strategies: BacktestStrategy[];
  total: number;
}

/**
 * 백테스트 성과 지표.
 * 자동 생성된 타입(BacktestMetricsResponse)을 재export.
 *
 * ⚠️ 주의: 모든 숫자 필드는 `number` 타입입니다 (이전에는 `string`이었음).
 * UI에서 포맷팅이 필요하면 `.toFixed()` 또는 유틸리티 함수 사용.
 *
 * 필드:
 * - total_return_pct: 총 수익률 (%)
 * - annualized_return_pct: 연율화 수익률 (%)
 * - net_profit: 순수익
 * - total_trades: 총 거래 수
 * - win_rate_pct: 승률 (%)
 * - profit_factor: 프로핏 팩터
 * - sharpe_ratio: 샤프 비율
 * - sortino_ratio: 소르티노 비율
 * - max_drawdown_pct: 최대 낙폭 (%)
 * - calmar_ratio: 칼마 비율
 * - avg_win: 평균 수익 거래
 * - avg_loss: 평균 손실 거래
 * - largest_win: 최대 수익 거래
 * - largest_loss: 최대 손실 거래
 */
export type BacktestMetrics = BacktestMetricsResponse;

export interface EquityCurvePoint {
  timestamp: number;
  equity: string;
  drawdown_pct: string;
}

export interface TradeHistoryItem {
  symbol: string;
  entry_time: string;
  exit_time: string;
  entry_price: string;
  exit_price: string;
  quantity: string;
  side: string;
  pnl: string;
  return_pct: string;
  /** 진입 이유 (전략의 논리적 목적) */
  entry_reason?: string;
  /** 청산 이유 (전략의 논리적 목적) */
  exit_reason?: string;
}

// TradeResultItem은 generated 타입 사용
export type { TradeResultItem } from '../types/generated/backtest';

export interface BacktestConfigSummary {
  initial_capital: string;
  commission_rate: string;
  slippage_rate: string;
  total_commission: string;
  total_slippage: string;
  data_points: number;
}

export interface BacktestResult {
  id: string;
  success: boolean;
  strategy_id: string;
  symbol: string;
  start_date: string;
  end_date: string;
  metrics: BacktestMetrics;
  equity_curve: EquityCurvePoint[];
  trades: TradeHistoryItem[];
  config_summary: BacktestConfigSummary;
  /** 백테스트에 사용된 타임프레임 설정 (다중 TF 백테스트 시) */
  timeframes_used?: MultiTimeframeConfig;
  /** 모든 거래 기록 (매수/매도 개별 거래 포함) - 매매일지용 */
  all_trades?: TradeResultItem[];
}

export const runBacktest = async (request: BacktestRequest): Promise<BacktestResult> => {
  const response = await api.post('/backtest/run', request);
  return response.data;
};

// 다중 자산 백테스트 실행 (CompoundMomentum, HAA, XAA, Stock Rotation 등)
export const runMultiBacktest = async (request: BacktestMultiRequest): Promise<BacktestMultiResult> => {
  const response = await api.post('/backtest/run-multi', request);
  return response.data;
};

export const getBacktestStrategies = async (): Promise<BacktestStrategiesResponse> => {
  const response = await api.get('/backtest/strategies');
  return response.data;
};

// ==================== SDUI (새로운 스키마 API) ====================

/** 전략 메타데이터 (SDUI) */
export interface StrategyMetaItem {
  id: string;
  aliases: string[];
  name: string;
  description: string;
  defaultTimeframe: string;
  secondaryTimeframes: string[];
  isMultiTimeframe: boolean;
  defaultTickers: string[];
  category: string;
  supportedMarkets: string[];
}

/** 전략 메타데이터 응답 */
export interface StrategyMetaResponse {
  strategies: StrategyMetaItem[];
}

/**
 * 전략 메타데이터 목록 조회 (SDUI)
 * GET /api/v1/strategies/meta
 */
export const getStrategyMeta = async (): Promise<StrategyMetaResponse> => {
  const response = await api.get('/strategies/meta');
  return response.data;
};

/**
 * 특정 전략의 SDUI 스키마 조회
 * GET /api/v1/strategies/{id}/schema
 */
export const getStrategySchema = async (strategyId: string): Promise<StrategyUISchema> => {
  const response = await api.get(`/strategies/${strategyId}/schema`);
  return response.data;
};

/**
 * Fragment 목록 조회 (SDUI)
 * GET /api/v1/schema/fragments
 */
export const getSchemaFragments = async (): Promise<GetFragmentsResponse> => {
  const response = await api.get('/schema/fragments');
  return response.data;
};

/**
 * Fragment 상세 조회 (SDUI)
 * GET /api/v1/schema/fragments/{id}/detail
 */
export const getSchemaFragmentDetail = async (fragmentId: string): Promise<SchemaFragment> => {
  const response = await api.get(`/schema/fragments/${fragmentId}/detail`);
  return response.data;
};

export const getBacktestResults = async (): Promise<BacktestResult[]> => {
  const response = await api.get('/backtest/results');
  return response.data.results || [];
};

export const getBacktestResult = async (id: string): Promise<BacktestResult> => {
  const response = await api.get(`/backtest/results/${id}`);
  return response.data;
};

/** 백테스트 결과 저장 요청 */
export interface SaveBacktestResultRequest {
  strategy_id: string;
  strategy_type: string;
  symbol: string;
  start_date: string;
  end_date: string;
  initial_capital: number;
  slippage_rate?: number;
  metrics: BacktestMetrics;
  config_summary: BacktestConfigSummary;
  equity_curve: EquityCurvePoint[];
  trades: TradeHistoryItem[];
  success: boolean;
  /** 백테스트에 사용된 타임프레임 설정 (다중 TF 백테스트 시) */
  timeframes_used?: MultiTimeframeConfig;
}

/** 백테스트 결과 저장 응답 */
export interface SaveBacktestResultResponse {
  id: string;
  message: string;
}

/** 백테스트 결과 목록 쿼리 파라미터 */
export interface ListBacktestResultsQuery {
  strategy_id?: string;
  strategy_type?: string;
  limit?: number;
  offset?: number;
}

/** 백테스트 결과 저장 */
export const saveBacktestResult = async (request: SaveBacktestResultRequest): Promise<SaveBacktestResultResponse> => {
  const response = await api.post('/backtest/results', request);
  return response.data;
};

/** 백테스트 결과 삭제 */
export const deleteBacktestResult = async (id: string): Promise<void> => {
  await api.delete(`/backtest/results/${id}`);
};

/** 저장된 백테스트 결과 목록 조회 (쿼리 파라미터 지원) */
export const listBacktestResults = async (query?: ListBacktestResultsQuery): Promise<{ results: BacktestResult[]; total: number }> => {
  const response = await api.get('/backtest/results', { params: query });
  return response.data;
};

// ==================== 시뮬레이션 ====================

/** 시뮬레이션 상태 enum */
export type SimulationStateEnum = 'stopped' | 'running' | 'paused';

/** 시뮬레이션 시작 요청 */
export interface SimulationStartRequest {
  strategy_id: string;
  /** 전략 파라미터 (JSON) */
  parameters?: Record<string, unknown>;
  /** 대상 심볼 목록 (미지정 시 전략 기본값 사용) */
  symbols?: string[];
  initial_balance?: number;
  /** 배속 (1.0 = 1초에 1캔들, 10.0 = 1초에 10캔들) */
  speed?: number;
  /** 시뮬레이션(백테스트) 시작 날짜 (YYYY-MM-DD) */
  start_date?: string;
  /** 시뮬레이션(백테스트) 종료 날짜 (YYYY-MM-DD) */
  end_date?: string;
  /** 수수료율 (기본값: 0.001 = 0.1%) */
  commission_rate?: number;
  /** 슬리피지율 (기본값: 0.0005 = 0.05%) */
  slippage_rate?: number;
}

/** 시뮬레이션 시작 응답 */
export interface SimulationStartResponse {
  success: boolean;
  message: string;
  started_at: string;
  /** 전체 캔들 수 (진행률 계산용) */
  total_candles: number;
}

/** 시뮬레이션 중지 응답 */
export interface SimulationStopResponse {
  success: boolean;
  message: string;
  final_equity: string;
  total_return_pct: string;
  total_trades: number;
}

/** 시뮬레이션 상태 응답 */
export interface SimulationStatusResponse {
  state: SimulationStateEnum;
  strategy_id: string | null;
  initial_balance: string;
  current_balance: string;
  total_equity: string;
  unrealized_pnl: string;
  realized_pnl: string;
  return_pct: string;
  position_count: number;
  trade_count: number;
  started_at: string | null;
  speed: number;
  /** 현재 시뮬레이션 시간 (배속 적용된 가상 시간) */
  current_simulation_time: string | null;
  /** 시뮬레이션(백테스트) 시작 날짜 (YYYY-MM-DD) */
  simulation_start_date: string | null;
  /** 시뮬레이션(백테스트) 종료 날짜 (YYYY-MM-DD) */
  simulation_end_date: string | null;
  /** 진행률 (0.0 ~ 100.0) */
  progress_pct: number;
  /** 현재 캔들 인덱스 */
  current_candle_index: number;
  /** 전체 캔들 수 */
  total_candles: number;
}

/** 시뮬레이션 포지션 */
export interface SimulationPosition {
  symbol: string;
  displayName?: string;  // "005930(삼성전자)" 형식
  side: string;  // "Long" | "Short"
  quantity: string;
  entry_price: string;
  current_price: string;
  unrealized_pnl: string;
  return_pct: string;
  entry_time: string;
}

/** 시뮬레이션 포지션 응답 */
export interface SimulationPositionsResponse {
  positions: SimulationPosition[];
  total_unrealized_pnl: string;
}

/** 시뮬레이션 거래 */
export interface SimulationTrade {
  id: string;
  symbol: string;
  displayName?: string;  // "005930(삼성전자)" 형식
  side: string;  // "Buy" | "Sell"
  quantity: string;
  price: string;
  commission: string;
  realized_pnl: string | null;
  timestamp: string;
}

/** 시뮬레이션 거래 내역 응답 */
export interface SimulationTradesResponse {
  trades: SimulationTrade[];
  total: number;
  total_realized_pnl: string;
  total_commission: string;
}

/** 시뮬레이션 자본 곡선 포인트 */
export interface SimulationEquityPoint {
  timestamp: string;
  equity: string;
  drawdown_pct: string;
}

/** 시뮬레이션 자본 곡선 응답 */
export interface SimulationEquityResponse {
  points: SimulationEquityPoint[];
  peak_equity: string;
  max_drawdown_pct: string;
}

/** 시뮬레이션 신호 마커 */
export interface SimulationSignalMarker {
  symbol: string;
  timestamp: string;
  signal_type: string;  // "BuyEntry" | "SellEntry" | "BuyExit" | "SellExit"
  price: string;
  strength: number;
  reason: string | null;
}

/** 시뮬레이션 신호 마커 응답 */
export interface SimulationSignalsResponse {
  signals: SimulationSignalMarker[];
  total: number;
}

/** 시뮬레이션 일시정지/재개 응답 */
export interface SimulationPauseResponse {
  success: boolean;
  state: SimulationStateEnum;
  message: string;
}

/** 시뮬레이션 리셋 응답 */
export interface SimulationResetResponse {
  success: boolean;
  message: string;
}

export const startSimulation = async (request: SimulationStartRequest): Promise<SimulationStartResponse> => {
  const response = await api.post('/simulation/start', request);
  return response.data;
};

export const stopSimulation = async (): Promise<SimulationStopResponse> => {
  const response = await api.post('/simulation/stop');
  return response.data;
};

export const pauseSimulation = async (): Promise<SimulationPauseResponse> => {
  const response = await api.post('/simulation/pause');
  return response.data;
};

export const resetSimulation = async (): Promise<SimulationResetResponse> => {
  const response = await api.post('/simulation/reset');
  return response.data;
};

export const getSimulationStatus = async (): Promise<SimulationStatusResponse> => {
  const response = await api.get('/simulation/status');
  return response.data;
};

export const getSimulationPositions = async (): Promise<SimulationPositionsResponse> => {
  const response = await api.get('/simulation/positions');
  return response.data;
};

export const getSimulationTrades = async (): Promise<SimulationTradesResponse> => {
  const response = await api.get('/simulation/trades');
  return response.data;
};

/** 시뮬레이션 자본 곡선 조회 */
export const getSimulationEquity = async (): Promise<SimulationEquityResponse> => {
  const response = await api.get('/simulation/equity');
  return response.data;
};

/** 시뮬레이션 신호 마커 조회 */
export const getSimulationSignals = async (): Promise<SimulationSignalsResponse> => {
  const response = await api.get('/simulation/signals');
  return response.data;
};

// ==================== 분석 (Analytics) ====================

export interface PerformanceResponse {
  currentEquity: string;
  initialCapital: string;
  totalPnl: string;
  totalReturnPct: string;
  cagrPct: string;
  maxDrawdownPct: string;
  currentDrawdownPct: string;
  peakEquity: string;
  periodDays: number;
  periodReturns: { period: string; returnPct: string }[];
  lastUpdated: string;
  // 포지션 기반 지표 (실제 투자 원금 대비)
  totalCostBasis?: string;      // 총 투자 원금
  positionPnl?: string;         // 포지션 손익 금액
  positionPnlPct?: string;      // 포지션 손익률 (%)
}

export interface ChartPointResponse {
  x: number;
  y: string;
  label?: string;
}

export interface EquityCurveResponse {
  data: ChartPointResponse[];
  count: number;
  period: string;
  startTime: string;
  endTime: string;
}

export interface ChartResponse {
  name: string;
  data: ChartPointResponse[];
  count: number;
  period: string;
}

export interface MonthlyReturnCell {
  year: number;
  month: number;
  returnPct: string;
  intensity: number;
}

export interface MonthlyReturnsResponse {
  data: MonthlyReturnCell[];
  count: number;
  yearRange: [number, number];
}

export const getPerformance = async (period?: string, credentialId?: string): Promise<PerformanceResponse> => {
  const params: Record<string, string> = {};
  if (period) params.period = period;
  if (credentialId) params.credential_id = credentialId;
  const response = await api.get('/analytics/performance', { params });
  return response.data;
};

export const getEquityCurve = async (period?: string, credentialId?: string): Promise<EquityCurveResponse> => {
  const params: Record<string, string> = {};
  if (period) params.period = period;
  if (credentialId) params.credential_id = credentialId;
  const response = await api.get('/analytics/equity-curve', { params });
  return response.data;
};

export const getCagrChart = async (period?: string, windowDays?: number): Promise<ChartResponse> => {
  const params: Record<string, string | number> = {};
  if (period) params.period = period;
  if (windowDays) params.window_days = windowDays;
  const response = await api.get('/analytics/charts/cagr', { params });
  return response.data;
};

export const getMddChart = async (period?: string, windowDays?: number): Promise<ChartResponse> => {
  const params: Record<string, string | number> = {};
  if (period) params.period = period;
  if (windowDays) params.window_days = windowDays;
  const response = await api.get('/analytics/charts/mdd', { params });
  return response.data;
};

export const getDrawdownChart = async (period?: string): Promise<ChartResponse> => {
  const params = period ? { period } : {};
  const response = await api.get('/analytics/charts/drawdown', { params });
  return response.data;
};

export const getMonthlyReturns = async (): Promise<MonthlyReturnsResponse> => {
  const response = await api.get('/analytics/monthly-returns');
  return response.data;
};

// 자산 곡선 동기화 요청
export interface SyncEquityCurveRequest {
  credential_id: string;
  start_date: string;  // YYYYMMDD
  end_date: string;    // YYYYMMDD
  use_market_prices?: boolean;  // 시장가 기반 자산 계산 (기본값: true)
}

// 자산 곡선 동기화 응답
export interface SyncEquityCurveResponse {
  success: boolean;
  message: string;
  synced_count: number;
  execution_count: number;
  start_date: string;
  end_date: string;
  synced_at: string;
}

// 자산 곡선 동기화 (거래소 체결 내역 기반)
export const syncEquityCurve = async (request: SyncEquityCurveRequest): Promise<SyncEquityCurveResponse> => {
  // 기본값: 시장가 기반 자산 계산 (현재 보유 포지션의 주식 가치만 추적)
  const requestWithDefaults = {
    ...request,
    use_market_prices: request.use_market_prices ?? true,
  };
  const response = await api.post('/analytics/sync-equity', requestWithDefaults);
  return response.data;
};

// 자산 곡선 캐시 삭제 응답
export interface ClearEquityCacheResponse {
  success: boolean;
  deleted_count: number;
  message: string;
}

// 자산 곡선 캐시 삭제
export const clearEquityCache = async (credentialId: string): Promise<ClearEquityCacheResponse> => {
  const response = await api.delete('/analytics/equity-cache', {
    data: { credential_id: credentialId },
  });
  return response.data;
};

// ==================== 기술적 지표 (Technical Indicators) ====================

/** OBV 지표 쿼리 */
export interface ObvQuery {
  /** 종목 코드 */
  ticker: string;
  /** 거래소 (KRX, BINANCE 등) */
  exchange?: string;
  /** 타임프레임 (1d, 1h, 15m 등) */
  timeframe?: string;
  /** 조회 기간 (일 수) */
  period?: number;
  /** 시그널 라인 기간 (기본: 20) */
  signal_period?: number;
  /** 변화율 반환 여부 */
  include_change?: boolean;
}

/** OBV 데이터 포인트 */
export interface ObvPoint {
  /** 타임스탬프 (ISO 8601) */
  timestamp: string;
  /** OBV 값 */
  obv: number;
  /** 시그널 라인 (SMA of OBV) */
  signal?: number;
  /** OBV 변화량 */
  change?: number;
}

/** OBV 응답 */
export interface ObvResponse {
  /** 종목 코드 */
  ticker: string;
  /** 파라미터 */
  params: {
    signal_period: number;
  };
  /** 데이터 포인트 */
  data: ObvPoint[];
}

/** OBV 지표 조회 */
export const getObvIndicator = async (query: ObvQuery): Promise<ObvResponse> => {
  const response = await api.get('/analytics/indicators/obv', { params: query });
  return response.data;
};

/** SuperTrend 지표 쿼리 */
export interface SuperTrendQuery {
  /** 종목 코드 */
  ticker: string;
  /** 거래소 (KRX, BINANCE 등) */
  exchange?: string;
  /** 타임프레임 (1d, 1h, 15m 등) */
  timeframe?: string;
  /** 조회 기간 (일 수) */
  period?: number;
  /** ATR 기간 (기본: 10) */
  atr_period?: number;
  /** ATR 배수 (기본: 3.0) */
  multiplier?: number;
}

/** SuperTrend 데이터 포인트 */
export interface SuperTrendPoint {
  /** 타임스탬프 (ISO 8601) */
  timestamp: string;
  /** SuperTrend 값 */
  value?: number;
  /** 추세 방향 (true: 상승, false: 하락) */
  is_uptrend: boolean;
  /** 매수 시그널 */
  buy_signal: boolean;
  /** 매도 시그널 */
  sell_signal: boolean;
}

/** SuperTrend 응답 */
export interface SuperTrendResponse {
  /** 종목 코드 */
  ticker: string;
  /** 파라미터 */
  params: {
    atr_period: number;
    multiplier: number;
  };
  /** 데이터 포인트 */
  data: SuperTrendPoint[];
}

/** SuperTrend 지표 조회 */
export const getSuperTrendIndicator = async (query: SuperTrendQuery): Promise<SuperTrendResponse> => {
  const response = await api.get('/analytics/indicators/supertrend', { params: query });
  return response.data;
};

// ==================== 알림 (Notifications) ====================

/** 알림 설정 응답 */
export interface NotificationSettingsResponse {
  telegram_enabled: boolean;
  telegram_configured: boolean;
}

/** 텔레그램 테스트 요청 */
export interface TelegramTestRequest {
  bot_token: string;
  chat_id: string;
}

/** 텔레그램 테스트 응답 */
export interface TelegramTestResponse {
  success: boolean;
  message: string;
}

/** 템플릿 정보 */
export interface TemplateInfo {
  id: string;
  name: string;
  description: string;
  priority: string;
}

/** 템플릿 목록 응답 */
export interface TemplateListResponse {
  templates: TemplateInfo[];
}

/** 템플릿 테스트 요청 */
export interface TemplateTestRequest {
  template_type: string;
}

export const getNotificationSettings = async (): Promise<NotificationSettingsResponse> => {
  const response = await api.get('/notifications/settings');
  return response.data;
};

export const getNotificationTemplates = async (): Promise<TemplateListResponse> => {
  const response = await api.get('/notifications/templates');
  return response.data;
};

export const testTelegram = async (request: TelegramTestRequest): Promise<TelegramTestResponse> => {
  const response = await api.post('/notifications/telegram/test', request);
  return response.data;
};

export const testTelegramEnv = async (): Promise<TelegramTestResponse> => {
  const response = await api.post('/notifications/telegram/test-env');
  return response.data;
};

export const testTelegramTemplate = async (request: TemplateTestRequest): Promise<TelegramTestResponse> => {
  const response = await api.post('/notifications/telegram/test-template', request);
  return response.data;
};

export const testAllTelegramTemplates = async (): Promise<TelegramTestResponse> => {
  const response = await api.post('/notifications/telegram/test-all-templates');
  return response.data;
};

// ==================== 알림 히스토리 (Alert History) ====================

/** 알림 히스토리 아이템 */
export interface AlertHistoryItem {
  id: string;
  ruleId?: string;
  signalMarkerId?: string;
  alertType: string;
  channel: string;
  symbol?: string;
  strategyId?: string;
  message: string;
  status: 'PENDING' | 'SENT' | 'READ' | 'FAILED';
  sentAt: string;
  readAt?: string;
  metadata?: Record<string, string>;
}

/** 알림 히스토리 응답 */
export interface AlertHistoryResponse {
  alerts: AlertHistoryItem[];
  total: number;
  unreadCount: number;
}

/** 알림 히스토리 조회 쿼리 */
export interface AlertHistoryQuery {
  limit?: number;
  offset?: number;
  status?: 'PENDING' | 'SENT' | 'READ' | 'FAILED';
  alertType?: string;
}

/** 알림 히스토리 조회 */
export const getAlertHistory = async (query?: AlertHistoryQuery): Promise<AlertHistoryResponse> => {
  const response = await api.get('/alerts/history', { params: query });
  return response.data;
};

/** 알림 읽음 처리 */
export const markAlertAsRead = async (alertId: string): Promise<void> => {
  await api.patch(`/alerts/history/${alertId}/read`);
};

/** 모든 알림 읽음 처리 */
export const markAllAlertsAsRead = async (): Promise<void> => {
  await api.patch('/alerts/history/read-all');
};

// ==================== 자격증명 관리 (Credentials) ====================

/** 지원되는 거래소 목록 응답 */
export interface SupportedExchangesResponse {
  exchanges: SupportedExchange[];
}

/** 등록된 자격증명 목록 응답 */
export interface CredentialsListResponse {
  credentials: ExchangeCredential[];
  total: number;
}

/** 자격증명 생성/수정 요청 */
export interface CredentialRequest {
  exchange_id: string;
  display_name: string;
  fields: Record<string, string>;
  /** 모의투자/테스트넷 여부 */
  is_testnet?: boolean;
}

/** 자격증명 응답 */
export interface CredentialResponse {
  success: boolean;
  message: string;
  credential?: ExchangeCredential;
}

/** 자격증명 테스트 요청 */
export interface CredentialTestRequest {
  exchange_id: string;
  fields: Record<string, string>;
}

/** 자격증명 테스트 응답 */
export interface CredentialTestResponse {
  success: boolean;
  message: string;
  details?: {
    balance_check?: boolean;
    permissions?: string[];
  };
}

/** 텔레그램 설정 요청 */
export interface TelegramSettingsRequest {
  bot_token: string;
  chat_id: string;
  display_name?: string;
}

/** 텔레그램 설정 응답 */
export interface TelegramSettingsResponse {
  success: boolean;
  message: string;
  settings?: TelegramSettings;
}

/** 지원되는 거래소 목록 조회 (필드 정보 포함) */
export const getSupportedExchanges = async (): Promise<SupportedExchangesResponse> => {
  const response = await api.get('/credentials/exchanges');
  return response.data;
};

/** 등록된 자격증명 목록 조회 */
export const listCredentials = async (): Promise<CredentialsListResponse> => {
  const response = await api.get('/credentials/exchanges/list');
  return response.data;
};

/** 새 자격증명 등록 */
export const createCredential = async (request: CredentialRequest): Promise<CredentialResponse> => {
  const response = await api.post('/credentials/exchanges', request);
  return response.data;
};

/** 기존 자격증명 수정 */
export const updateCredential = async (id: string, request: CredentialRequest): Promise<CredentialResponse> => {
  const response = await api.put(`/credentials/exchanges/${id}`, request);
  return response.data;
};

/** 자격증명 삭제 */
export const deleteCredential = async (id: string): Promise<{ success: boolean; message: string }> => {
  const response = await api.delete(`/credentials/exchanges/${id}`);
  return response.data;
};

/** 새 자격증명 테스트 (저장 전) */
export const testNewCredential = async (request: CredentialTestRequest): Promise<CredentialTestResponse> => {
  const response = await api.post('/credentials/exchanges/test', request);
  return response.data;
};

/** 기존 자격증명 테스트 */
export const testExistingCredential = async (id: string): Promise<CredentialTestResponse> => {
  const response = await api.post(`/credentials/exchanges/${id}/test`);
  return response.data;
};

/** 텔레그램 설정 조회 */
export const getTelegramSettings = async (): Promise<TelegramSettings> => {
  const response = await api.get('/credentials/telegram');
  return response.data;
};

/** 텔레그램 설정 저장 */
export const saveTelegramSettings = async (request: TelegramSettingsRequest): Promise<TelegramSettingsResponse> => {
  const response = await api.post('/credentials/telegram', request);
  return response.data;
};

/** 텔레그램 설정 삭제 */
export const deleteTelegramSettings = async (): Promise<{ success: boolean; message: string }> => {
  const response = await api.delete('/credentials/telegram');
  return response.data;
};

// ==================== Discord 설정 ====================

/** Discord 설정 요청 */
export interface DiscordSettingsRequest {
  webhook_url: string;
  display_name?: string;
  server_name?: string;
  channel_name?: string;
}

/** Discord 설정 응답 */
export interface DiscordSettings {
  id: string;
  webhook_url_masked: string;
  display_name?: string;
  server_name?: string;
  channel_name?: string;
  is_enabled: boolean;
  configured: boolean;
  last_message_at?: string;
  last_verified_at?: string;
  created_at: string;
  updated_at: string;
}

/** Discord 설정 조회 */
export const getDiscordSettings = async (): Promise<DiscordSettings> => {
  const response = await api.get('/credentials/discord');
  return { ...response.data, configured: !!response.data.id };
};

/** Discord 설정 저장 */
export const saveDiscordSettings = async (request: DiscordSettingsRequest): Promise<{ success: boolean; message: string }> => {
  const response = await api.post('/credentials/discord', request);
  return response.data;
};

/** Discord 설정 삭제 */
export const deleteDiscordSettings = async (): Promise<{ success: boolean; message: string }> => {
  const response = await api.delete('/credentials/discord');
  return response.data;
};

/** Discord 연결 테스트 (저장된 설정) */
export const testDiscordSettings = async (): Promise<{ success: boolean; message: string }> => {
  const response = await api.post('/credentials/discord/test');
  return response.data;
};

/** Discord 새 설정 테스트 (저장 전) */
export const testNewDiscordSettings = async (request: DiscordSettingsRequest): Promise<{ success: boolean; message: string }> => {
  const response = await api.post('/credentials/discord/test/new', request);
  return response.data;
};

// ==================== Slack 설정 ====================

/** Slack 설정 요청 */
export interface SlackSettingsRequest {
  webhook_url: string;
  display_name?: string;
  workspace_name?: string;
  channel_name?: string;
}

/** Slack 설정 응답 */
export interface SlackSettings {
  id: string;
  webhook_url_masked: string;
  display_name?: string;
  workspace_name?: string;
  channel_name?: string;
  is_enabled: boolean;
  configured: boolean;
  last_message_at?: string;
  last_verified_at?: string;
  created_at: string;
  updated_at: string;
}

/** Slack 설정 조회 */
export const getSlackSettings = async (): Promise<SlackSettings> => {
  const response = await api.get('/credentials/slack');
  return { ...response.data, configured: !!response.data.id };
};

/** Slack 설정 저장 */
export const saveSlackSettings = async (request: SlackSettingsRequest): Promise<{ success: boolean; message: string }> => {
  const response = await api.post('/credentials/slack', request);
  return response.data;
};

/** Slack 설정 삭제 */
export const deleteSlackSettings = async (): Promise<{ success: boolean; message: string }> => {
  const response = await api.delete('/credentials/slack');
  return response.data;
};

/** Slack 연결 테스트 (저장된 설정) */
export const testSlackSettings = async (): Promise<{ success: boolean; message: string }> => {
  const response = await api.post('/credentials/slack/test');
  return response.data;
};

/** Slack 새 설정 테스트 (저장 전) */
export const testNewSlackSettings = async (request: SlackSettingsRequest): Promise<{ success: boolean; message: string }> => {
  const response = await api.post('/credentials/slack/test/new', request);
  return response.data;
};

// ==================== Email 설정 ====================

/** Email 설정 요청 */
export interface EmailSettingsRequest {
  smtp_host: string;
  smtp_port: number;
  use_tls: boolean;
  username: string;
  password: string;
  from_email: string;
  from_name?: string;
  to_emails: string[];
}

/** Email 설정 응답 */
export interface EmailSettings {
  id: string;
  smtp_host: string;
  smtp_port: number;
  use_tls: boolean;
  username_masked: string;
  from_email: string;
  from_name?: string;
  to_emails: string[];
  is_enabled: boolean;
  configured: boolean;
  last_message_at?: string;
  last_verified_at?: string;
  created_at: string;
  updated_at: string;
}

/** Email 설정 조회 */
export const getEmailSettings = async (): Promise<EmailSettings> => {
  const response = await api.get('/credentials/email');
  return { ...response.data, configured: !!response.data.id };
};

/** Email 설정 저장 */
export const saveEmailSettings = async (request: EmailSettingsRequest): Promise<{ success: boolean; message: string }> => {
  const response = await api.post('/credentials/email', request);
  return response.data;
};

/** Email 설정 삭제 */
export const deleteEmailSettings = async (): Promise<{ success: boolean; message: string }> => {
  const response = await api.delete('/credentials/email');
  return response.data;
};

/** Email 연결 테스트 (저장된 설정) */
export const testEmailSettings = async (): Promise<{ success: boolean; message: string }> => {
  const response = await api.post('/credentials/email/test');
  return response.data;
};

/** Email 새 설정 테스트 (저장 전) */
export const testNewEmailSettings = async (request: EmailSettingsRequest): Promise<{ success: boolean; message: string }> => {
  const response = await api.post('/credentials/email/test/new', request);
  return response.data;
};

// ==================== SMS (Twilio) 설정 ====================

/** SMS 설정 요청 */
export interface SmsSettingsRequest {
  provider?: string;  // 기본값: 'twilio'
  account_sid: string;
  auth_token: string;
  from_number: string;
  to_numbers: string[];
}

/** SMS 설정 응답 */
export interface SmsSettings {
  id: string;
  provider: string;
  account_sid_masked: string;
  from_number: string;
  to_numbers: string[];
  is_enabled: boolean;
  configured: boolean;
  last_message_at?: string;
  last_verified_at?: string;
  created_at: string;
  updated_at: string;
}

/** SMS 설정 조회 */
export const getSmsSettings = async (): Promise<SmsSettings> => {
  const response = await api.get('/credentials/sms');
  return { ...response.data, configured: !!response.data.id };
};

/** SMS 설정 저장 */
export const saveSmsSettings = async (request: SmsSettingsRequest): Promise<{ success: boolean; message: string }> => {
  const response = await api.post('/credentials/sms', request);
  return response.data;
};

/** SMS 설정 삭제 */
export const deleteSmsSettings = async (): Promise<{ success: boolean; message: string }> => {
  const response = await api.delete('/credentials/sms');
  return response.data;
};

/** SMS 연결 테스트 (저장된 설정) */
export const testSmsSettings = async (): Promise<{ success: boolean; message: string }> => {
  const response = await api.post('/credentials/sms/test');
  return response.data;
};

/** SMS 새 설정 테스트 (저장 전) */
export const testNewSmsSettings = async (request: SmsSettingsRequest): Promise<{ success: boolean; message: string }> => {
  const response = await api.post('/credentials/sms/test/new', request);
  return response.data;
};

// ==================== 심볼 검색 ====================

/** 심볼 검색 결과 */
export interface SymbolSearchResult {
  ticker: string;
  name: string;
  market: string;
  yahooSymbol: string | null;
}

/** 심볼 검색 응답 */
export interface SymbolSearchResponse {
  results: SymbolSearchResult[];
  total: number;
}

/**
 * 심볼/회사명 검색
 * @param query 검색어 (티커 또는 회사명)
 * @param limit 최대 결과 수 (기본값: 10)
 */
export const searchSymbols = async (query: string, limit: number = 10): Promise<SymbolSearchResult[]> => {
  if (!query.trim()) return [];

  const params = new URLSearchParams({ q: query, limit: limit.toString() });
  const response = await api.get(`/dataset/search?${params}`);
  return response.data?.results || [];
};

/** 심볼 배치 조회 응답 */
export interface SymbolBatchResponse {
  symbols: SymbolSearchResult[];
  total: number;
}

/**
 * 여러 티커의 심볼 정보 일괄 조회
 * @param tickers 조회할 티커 목록 (최대 100개)
 * @returns 심볼 정보 배열
 */
export const getSymbolsBatch = async (tickers: string[]): Promise<SymbolSearchResult[]> => {
  if (tickers.length === 0) return [];

  const response = await api.post<SymbolBatchResponse>('/dataset/symbols/batch', { tickers });
  return response.data?.symbols || [];
};

// ==================== 매매일지 (Journal) ====================
// 타입은 types/generated/journal에서 import됨

/**
 * 체결 내역 조회 필터.
 *
 * NOTE: 자동 생성 타입(ListExecutionsQuery)은 limit/offset이 bigint이지만,
 * JSON 직렬화 호환성을 위해 프론트엔드에서는 number 사용.
 * 백엔드에서 i64 → i32로 변경하면 자동 생성 타입 사용 가능.
 *
 * @see ListExecutionsQuery (자동 생성된 타입)
 */
export interface ExecutionFilter {
  /** 종목 필터 */
  symbol?: string;
  /** 매수/매도 필터 (buy, sell) */
  side?: string;
  /** 전략 ID 필터 */
  strategy_id?: string;
  /** 시작 날짜 (ISO 8601) */
  start_date?: string;
  /** 종료 날짜 (ISO 8601) */
  end_date?: string;
  /** 페이지 크기 (기본 50) */
  limit?: number;
  /** 오프셋 (기본 0) */
  offset?: number;
}

/** 동기화 응답 (자동 생성 타입과 필드명 다름) */
export interface JournalSyncResponse {
  success: boolean;
  inserted: number;
  skipped: number;
  message: string;
}

/** 매매일지 포지션 조회 */
export const getJournalPositions = async (): Promise<JournalPositionsResponse> => {
  const response = await api.get('/journal/positions');
  return response.data;
};

/** 매매일지 체결 내역 조회 */
export const getJournalExecutions = async (filter?: ExecutionFilter): Promise<JournalExecutionsResponse> => {
  const response = await api.get('/journal/executions', { params: filter });
  return response.data;
};

/** PnL 요약 조회 */
export const getJournalPnLSummary = async (): Promise<JournalPnLSummary> => {
  const response = await api.get('/journal/pnl');
  return response.data;
};

/** 일별 손익 조회 */
export const getJournalDailyPnL = async (startDate?: string, endDate?: string): Promise<DailyPnLResponse> => {
  const params: Record<string, string> = {};
  if (startDate) params.start_date = startDate;
  if (endDate) params.end_date = endDate;
  const response = await api.get('/journal/pnl/daily', { params });
  return response.data;
};

/** 종목별 손익 조회 */
export const getJournalSymbolPnL = async (): Promise<SymbolPnLResponse> => {
  const response = await api.get('/journal/pnl/symbol');
  return response.data;
};

/** 체결 내역 메모/태그 수정 */
export const updateJournalExecution = async (
  id: string,
  data: { memo?: string; tags?: string[] }
): Promise<JournalExecution> => {
  const response = await api.patch(`/journal/executions/${id}`, data);
  return response.data;
};

/** 거래소 체결 내역 동기화 */
export const syncJournalExecutions = async (
  exchange?: string,
  startDate?: string,
  forceFullSync?: boolean
): Promise<JournalSyncResponse> => {
  const response = await api.post('/journal/sync', {
    exchange,
    start_date: startDate,
    force_full_sync: forceFullSync ?? false,
  });
  return response.data;
};

/** 캐시 삭제 응답 */
export interface ClearCacheResponse {
  success: boolean;
  deleted_count: number;
  message: string;
}

/** 체결 내역 캐시 삭제 */
export const clearJournalCache = async (): Promise<ClearCacheResponse> => {
  const response = await api.delete('/journal/cache');
  return response.data;
};

// ==================== 기간별 손익 API ====================

/** 주별 손익 항목 */
export interface WeeklyPnLItem {
  week_start: string;
  total_trades: number;
  buy_count: number;
  sell_count: number;
  total_volume: string;
  total_fees: string;
  realized_pnl: string;
  symbol_count: number;
  trading_days: number;
}

/** 주별 손익 응답 */
export interface WeeklyPnLResponse {
  weekly: WeeklyPnLItem[];
  total_weeks: number;
}

/** 월별 손익 항목 */
export interface MonthlyPnLItem {
  year: number;
  month: number;
  total_trades: number;
  buy_count: number;
  sell_count: number;
  total_volume: string;
  total_fees: string;
  realized_pnl: string;
  symbol_count: number;
  trading_days: number;
}

/** 월별 손익 응답 */
export interface MonthlyPnLResponse {
  monthly: MonthlyPnLItem[];
  total_months: number;
}

/** 연도별 손익 항목 */
export interface YearlyPnLItem {
  year: number;
  total_trades: number;
  buy_count: number;
  sell_count: number;
  total_volume: string;
  total_fees: string;
  realized_pnl: string;
  symbol_count: number;
  trading_days: number;
  trading_months: number;
}

/** 연도별 손익 응답 */
export interface YearlyPnLResponse {
  yearly: YearlyPnLItem[];
  total_years: number;
}

/** 누적 손익 포인트 */
export interface CumulativePnLPoint {
  date: string;
  cumulative_pnl: string;
  cumulative_fees: string;
  cumulative_trades: number;
  daily_pnl: string;
}

/** 누적 손익 응답 */
export interface CumulativePnLResponse {
  curve: CumulativePnLPoint[];
  total_points: number;
}

/** 투자 인사이트 응답 */
export interface TradingInsightsResponse {
  total_trades: number;
  buy_trades: number;
  sell_trades: number;
  unique_symbols: number;
  total_realized_pnl: string;
  total_fees: string;
  winning_trades: number;
  losing_trades: number;
  win_rate_pct: string;
  profit_factor: string | null;
  avg_win: string;
  avg_loss: string;
  largest_win: string;
  largest_loss: string;
  trading_period_days: number;
  active_trading_days: number;
  first_trade_at: string | null;
  last_trade_at: string | null;
  // 고급 통계 (연속 승/패, Max Drawdown)
  max_consecutive_wins: number | null;
  max_consecutive_losses: number | null;
  max_drawdown: string | null;
  max_drawdown_pct: string | null;
}

/** 전략별 성과 항목 */
export interface StrategyPerformanceItem {
  strategy_id: string;
  strategy_name: string;
  total_trades: number;
  buy_trades: number;
  sell_trades: number;
  unique_symbols: number;
  total_volume: string;
  total_fees: string;
  realized_pnl: string;
  winning_trades: number;
  losing_trades: number;
  win_rate_pct: string;
  profit_factor: string | null;
  avg_win: string;
  avg_loss: string;
  largest_win: string;
  largest_loss: string;
  active_trading_days: number;
  first_trade_at: string | null;
  last_trade_at: string | null;
}

/** 전략별 성과 응답 */
export interface StrategyPerformanceResponse {
  strategies: StrategyPerformanceItem[];
  total: number;
}

/** 주별 손익 조회 */
export const getJournalWeeklyPnL = async (): Promise<WeeklyPnLResponse> => {
  const response = await api.get('/journal/pnl/weekly');
  return response.data;
};

/** 월별 손익 조회 */
export const getJournalMonthlyPnL = async (): Promise<MonthlyPnLResponse> => {
  const response = await api.get('/journal/pnl/monthly');
  return response.data;
};

/** 연도별 손익 조회 */
export const getJournalYearlyPnL = async (): Promise<YearlyPnLResponse> => {
  const response = await api.get('/journal/pnl/yearly');
  return response.data;
};

/** 누적 손익 곡선 조회 */
export const getJournalCumulativePnL = async (): Promise<CumulativePnLResponse> => {
  const response = await api.get('/journal/pnl/cumulative');
  return response.data;
};

/** 투자 인사이트 조회 */
export const getJournalInsights = async (): Promise<TradingInsightsResponse> => {
  const response = await api.get('/journal/insights');
  return response.data;
};

/** 전략별 성과 조회 */
export const getJournalStrategyPerformance = async (): Promise<StrategyPerformanceResponse> => {
  const response = await api.get('/journal/strategies');
  return response.data;
};

// ==================== FIFO 원가 계산 ====================

/** FIFO 원가 계산 응답 */
export interface FifoCostBasisResponse {
  /** 심볼 */
  symbol: string;
  /** 총 보유 수량 */
  total_quantity: string;
  /** 평균 비용 (FIFO 기준) */
  average_cost: string;
  /** 평균 가격 */
  average_price: string;
  /** 총 비용 기준 */
  total_cost_basis: string;
  /** 시장 가치 (현재가 기준) */
  market_value?: string;
  /** 미실현 손익 */
  unrealized_pnl?: string;
  /** 미실현 손익률 (%) */
  unrealized_pnl_pct?: string;
  /** 총 실현 손익 */
  total_realized_pnl: string;
  /** 총 매도 금액 */
  total_sales: string;
  /** 매수 거래 수 */
  buy_count: number;
  /** 매도 거래 수 */
  sell_count: number;
  /** 현재 남은 로트 수 */
  lot_count: number;
}

/** FIFO 원가 계산 조회 */
export const getFifoCostBasis = async (
  symbol: string,
  market: string = 'KR',
  currentPrice?: string
): Promise<FifoCostBasisResponse> => {
  const params: Record<string, string> = { market };
  if (currentPrice) params.current_price = currentPrice;
  const response = await api.get(`/journal/cost-basis/${symbol}`, { params });
  return response.data;
};

// ==================== 관심종목 (Watchlist) ====================

/** 관심종목 그룹 */
export interface WatchlistGroup {
  id: string;
  name: string;
  description: string | null;
  color: string | null;
  is_default: boolean;
  created_at: string;
  updated_at: string;
}

/** 관심종목 그룹 (개수 포함) */
export interface WatchlistWithCount extends WatchlistGroup {
  item_count: number;
}

/** 관심종목 아이템 */
export interface WatchlistItem {
  id: string;
  watchlist_id: string;
  symbol: string;
  market: string;
  memo: string | null;
  target_price: string | null;
  stop_loss: string | null;
  added_at: string;
  updated_at: string;
}

/** 관심종목 그룹 목록 응답 */
export interface WatchlistListResponse {
  watchlists: WatchlistWithCount[];
  total: number;
}

/** 관심종목 그룹 상세 응답 */
export interface WatchlistDetailResponse {
  id: string;
  name: string;
  description: string | null;
  color: string | null;
  is_default: boolean;
  created_at: string;
  updated_at: string;
  items: WatchlistItem[];
  item_count: number;
}

/** 새 관심종목 아이템 */
export interface NewWatchlistItem {
  symbol: string;
  market: string;
  memo?: string | null;
  target_price?: string | null;
  stop_loss?: string | null;
}

/** 아이템 추가 응답 */
export interface AddItemsResponse {
  added: WatchlistItem[];
  count: number;
}

/** 관심종목 그룹 목록 조회 */
export const getWatchlists = async (): Promise<WatchlistListResponse> => {
  const response = await api.get('/watchlist');
  return response.data;
};

/** 관심종목 그룹 생성 */
export const createWatchlist = async (name: string, description?: string, color?: string): Promise<WatchlistGroup> => {
  const response = await api.post('/watchlist', { name, description, color });
  return response.data;
};

/** 관심종목 그룹 상세 조회 */
export const getWatchlistDetail = async (id: string): Promise<WatchlistDetailResponse> => {
  const response = await api.get(`/watchlist/${id}`);
  return response.data;
};

/** 관심종목 그룹 삭제 */
export const deleteWatchlist = async (id: string): Promise<void> => {
  await api.delete(`/watchlist/${id}`);
};

/** 관심종목에 아이템 추가 */
export const addWatchlistItems = async (watchlistId: string, items: NewWatchlistItem[]): Promise<AddItemsResponse> => {
  const response = await api.post(`/watchlist/${watchlistId}/items`, { items });
  return response.data;
};

/** 관심종목에서 아이템 삭제 */
export const removeWatchlistItem = async (watchlistId: string, symbol: string, market: string = 'KR'): Promise<void> => {
  await api.delete(`/watchlist/${watchlistId}/items/${symbol}`, { params: { market } });
};

/** 특정 종목이 포함된 관심종목 그룹 조회 */
export const findWatchlistsContainingSymbol = async (symbol: string, market: string = 'KR'): Promise<WatchlistGroup[]> => {
  const response = await api.get(`/watchlist/symbol/${symbol}`, { params: { market } });
  return response.data;
};

// ==================== 스크리닝 (Screening) ====================
// 타입은 types/generated/screening에서 import됨

/** 스크리닝 프리셋 (자동 생성 타입에 없음) */
export interface ScreeningPreset {
  id: string;
  name: string;
  description: string;
}

/** 프리셋 목록 응답 (자동 생성 타입에 없음) */
export interface PresetsListResponse {
  presets: ScreeningPreset[];
}

/** 커스텀 스크리닝 실행 */
export const runScreening = async (request: GeneratedScreeningRequest): Promise<GeneratedScreeningResponse> => {
  const response = await api.post('/screening', request);
  return response.data;
};

/** 스크리닝 프리셋 목록 조회 */
export const getScreeningPresets = async (): Promise<PresetsListResponse> => {
  const response = await api.get('/screening/presets');
  return response.data;
};

/** 프리셋 상세 정보 (필터 포함) */
export interface ScreeningPresetDetail {
  id: string;
  name: string;
  description: string | null;
  filters: Record<string, unknown>;
  is_default: boolean;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

/** 프리셋 목록 응답 (상세 정보 포함) */
export interface PresetsDetailListResponse {
  presets: ScreeningPresetDetail[];
  total: number;
}

/** 프리셋 생성 요청 */
export interface CreatePresetRequest {
  name: string;
  description?: string;
  filters: Record<string, unknown>;
}

/** 프리셋 저장 응답 */
export interface SavePresetResponse {
  success: boolean;
  preset: ScreeningPresetDetail;
  message: string;
}

/** 프리셋 삭제 응답 */
export interface DeletePresetResponse {
  success: boolean;
  message: string;
}

/** 프리셋 목록 조회 (상세 정보 포함) */
export const getScreeningPresetsDetail = async (): Promise<PresetsDetailListResponse> => {
  const response = await api.get('/screening/presets/all');
  return response.data;
};

/** 프리셋 저장 */
export const saveScreeningPreset = async (request: CreatePresetRequest): Promise<SavePresetResponse> => {
  const response = await api.post('/screening/presets', request);
  return response.data;
};

/** 프리셋 삭제 */
export const deleteScreeningPreset = async (id: string): Promise<DeletePresetResponse> => {
  const response = await api.delete(`/screening/presets/id/${id}`);
  return response.data;
};

/** 프리셋 스크리닝 실행 (offset 기반 페이징 지원) */
export const runPresetScreening = async (
  preset: string,
  market?: string,
  limit?: number,
  offset?: number
): Promise<GeneratedScreeningResponse> => {
  const params: Record<string, string | number> = {};
  if (market) params.market = market;
  if (limit !== undefined) params.limit = limit;
  if (offset !== undefined) params.offset = offset;
  const response = await api.get(`/screening/presets/${preset}`, { params });
  return response.data;
};

/** 모멘텀 스크리닝 실행 */
export const runMomentumScreening = async (query: GeneratedMomentumQuery): Promise<GeneratedMomentumResponse> => {
  const response = await api.get('/screening/momentum', { params: query });
  return response.data;
};

// ==================== Global Ranking (GlobalScore) ====================
// 타입은 types/generated/ranking에서 import됨

/**
 * 랭킹 조회 쿼리.
 * 자동 생성된 타입(GeneratedRankingQuery)을 재export.
 *
 * NOTE: 백엔드에서 i64 → i32로 변경하여 bigint 문제 해결됨.
 *
 * 필드:
 * - market: 시장 필터 (KR, US 등)
 * - grade: 등급 필터 (BUY, WATCH 등)
 * - min_score: 최소 점수
 * - limit: 반환 개수 (기본 50, 최대 500)
 * - route_state: RouteState 필터 (ATTACK, ARMED, WATCH, REST)
 */
export type RankingQuery = {
  market?: string;
  grade?: string;
  min_score?: string;
  limit?: number;
  route_state?: string;
};

/** 상위 랭킹 조회 */
export const getTopRanked = async (query?: RankingQuery): Promise<GeneratedRankingResponse> => {
  const response = await api.get('/ranking/top', { params: query });
  return response.data;
};

/** 모든 심볼 GlobalScore 계산 (관리자용) */
export const calculateGlobalScore = async (): Promise<{ processed: number; started_at: string; completed_at: string }> => {
  const response = await api.post('/ranking/global');
  return response.data;
};

// ==================== Score History (점수 히스토리) ====================

/** Score History 요약 항목 */
export interface ScoreHistorySummary {
  /** 종목 코드 */
  symbol: string;
  /** 날짜 (YYYY-MM-DD) */
  score_date: string;
  /** Global Score (0-100) */
  global_score: number | null;
  /** RouteState (ATTACK/ARMED/WATCH/REST/SIDELINE) */
  route_state: string | null;
  /** 전체 순위 */
  rank: number | null;
  /** 전일 대비 점수 변화 */
  score_change: number | null;
  /** 전일 대비 순위 변화 (양수=상승) */
  rank_change: number | null;
}

/** Score History 응답 */
export interface ScoreHistoryResponse {
  /** 종목 코드 */
  symbol: string;
  /** 히스토리 데이터 */
  history: ScoreHistorySummary[];
  /** 총 레코드 수 */
  total: number;
}

/** Score History 조회 쿼리 */
export interface ScoreHistoryQuery {
  /** 조회 일수 (기본 90, 최대 365) */
  days?: number;
}

/** 종목별 Score History 조회 */
export const getScoreHistory = async (ticker: string, query?: ScoreHistoryQuery): Promise<ScoreHistoryResponse> => {
  const response = await api.get(`/ranking/history/${ticker}`, { params: query });
  return response.data;
};

// ==================== Signals (신호 마커) ====================

/** 지표 필터 조건 연산자 */
export interface IndicatorCondition {
  $gte?: number;  // >=
  $lte?: number;  // <=
  $gt?: number;   // >
  $lt?: number;   // <
  $eq?: number;   // =
}

/** 지표 기반 신호 검색 요청 */
export interface SignalSearchRequest {
  /** 지표 필터 (JSONB 쿼리) - 예: { "rsi": { "$gte": 70 }, "macd": { "$gt": 0 } } */
  indicator_filter: Record<string, IndicatorCondition>;
  /** 신호 유형 필터 (선택) */
  signal_type?: string;
  /** 최대 결과 개수 (기본 100, 최대 1000) */
  limit?: number;
}

/** 심볼별 신호 조회 요청 */
export interface SymbolSignalsQuery {
  /** 심볼 (예: "005930") */
  symbol: string;
  /** 거래소 (예: "KRX") */
  exchange: string;
  /** 시작 시각 (ISO 8601) */
  start_time?: string;
  /** 종료 시각 (ISO 8601) */
  end_time?: string;
  /** 최대 결과 개수 */
  limit?: number;
}

/** 전략별 신호 조회 요청 */
export interface StrategySignalsQuery {
  /** 전략 ID */
  strategy_id: string;
  /** 시작 시각 (ISO 8601) */
  start_time?: string;
  /** 종료 시각 (ISO 8601) */
  end_time?: string;
  /** 최대 결과 개수 */
  limit?: number;
}

/** 신호 마커 DTO */
export interface SignalMarkerDto {
  id: string;
  symbol: string;
  timestamp: string;
  signal_type: string;
  side?: string;
  price: string;
  strength: number;
  indicators: Record<string, number | undefined>;
  reason: string;
  strategy_id: string;
  strategy_name: string;
  executed: boolean;
}

/** 신호 검색 응답 */
export interface SignalSearchResponse {
  total: number;
  signals: SignalMarkerDto[];
}

/** 백테스트 신호 응답 */
export interface BacktestSignalsResponse {
  backtest_id: string;
  strategy_id: string;
  strategy_type: string;
  symbol: string;
  total_trades: number;
  trades: unknown;  // JSON 형태
}

/** 지표 기반 신호 검색 (POST) */
export const searchSignals = async (request: SignalSearchRequest): Promise<SignalSearchResponse> => {
  const response = await api.post('/signals/search', request);
  return response.data;
};

/** 특정 심볼의 신호 조회 */
export const getSymbolSignals = async (query: SymbolSignalsQuery): Promise<SignalSearchResponse> => {
  const response = await api.get('/signals/by-symbol', { params: query });
  return response.data;
};

/** 특정 전략의 신호 조회 */
export const getStrategySignals = async (query: StrategySignalsQuery): Promise<SignalSearchResponse> => {
  const response = await api.get('/signals/by-strategy', { params: query });
  return response.data;
};

/** 백테스트 신호(거래) 조회 */
export const getBacktestSignals = async (backtestId: string): Promise<BacktestSignalsResponse> => {
  const response = await api.get(`/signals/markers/backtest/${backtestId}`);
  return response.data;
};

// ==================== Signal Performance (신호 성과 분석) ====================
import type {
  SignalPerformanceResponse,
  SignalSymbolStats,
  SignalReturnPoint,
} from '../types/generated/signals';

/** 신호 성과 요약 조회 (전체) */
export const getSignalPerformance = async (): Promise<SignalPerformanceResponse> => {
  const response = await api.get('/signals/performance');
  return response.data;
};

/** 신호-수익률 산점도 데이터 조회 */
export interface SignalScatterQuery {
  /** 특정 종목만 조회 */
  ticker?: string;
  /** 최대 결과 개수 (기본 500) */
  limit?: number;
}

export const getSignalScatterData = async (query?: SignalScatterQuery): Promise<SignalReturnPoint[]> => {
  const response = await api.get('/signals/performance/scatter', { params: query });
  return response.data;
};

/** 특정 심볼의 신호 성과 조회 */
export const getSymbolSignalPerformance = async (ticker: string): Promise<SignalSymbolStats | null> => {
  const response = await api.get(`/signals/performance/${ticker}`);
  return response.data;
};

// ==================== Sectors (섹터 분석) ====================

/** 섹터 RS (상대강도) DTO */
export interface SectorRsDto {
  sector: string;
  symbol_count: number;
  avg_return_pct: string;
  market_return: string;
  relative_strength: string;
  composite_score: string;
  rank: number;
  /** 5일 평균 수익률 (%) - SectorMomentumBar 용 */
  avg_return_5d_pct?: string;
  /** 섹터 총 시가총액 - SectorTreemap 용 */
  total_market_cap?: string;
}

/** 섹터 순위 응답 */
export interface SectorRankingResponse {
  total: number;
  days: number;
  market?: string;
  results: SectorRsDto[];
}

/** 섹터 순위 조회 */
export const getSectorRanking = async (
  market?: string,
  days?: number
): Promise<SectorRankingResponse> => {
  const params: Record<string, string | number> = {};
  if (market) params.market = market;
  if (days) params.days = days;
  const response = await api.get('/sectors/ranking', { params });
  return response.data;
};

// ==================== Paper Trading ====================

/** Paper Trading 계정 정보 */
export interface PaperTradingAccount {
  id: string;
  name: string;
  exchangeId: string;
  marketType: string;
  currency: string;
  initialBalance: string;
  currentBalance: string;
  positionValue: string;
  totalEquity: string;
  unrealizedPnl: string;
  realizedPnl: string;
  returnPct: string;
  strategyCount: number;
  isActive: boolean;
}

/** Paper Trading 계정 목록 응답 */
export interface PaperTradingAccountsResponse {
  accounts: PaperTradingAccount[];
  total: number;
}

/** Paper Trading 포지션 */
export interface PaperTradingPosition {
  symbol: string;
  side: string;
  quantity: string;
  entryPrice: string;
  currentPrice: string;
  marketValue: string;
  unrealizedPnl: string;
  returnPct: string;
  entryTime: string;
}

/** Paper Trading 포지션 응답 */
export interface PaperTradingPositionsResponse {
  positions: PaperTradingPosition[];
  total: number;
  totalValue: string;
  totalUnrealizedPnl: string;
}

/** Paper Trading 체결 */
export interface PaperTradingExecution {
  id: string;
  symbol: string;
  side: string;
  quantity: string;
  price: string;
  commission: string;
  realizedPnl: string | null;
  executedAt: string;
}

/** Paper Trading 체결 응답 */
export interface PaperTradingExecutionsResponse {
  executions: PaperTradingExecution[];
  total: number;
}

/** Paper Trading 계정 초기화 응답 */
export interface ResetAccountResponse {
  success: boolean;
  message: string;
}

/** Paper Trading 계정 목록 조회 */
export const getPaperTradingAccounts = async (): Promise<PaperTradingAccountsResponse> => {
  const response = await api.get('/paper-trading/accounts');
  return response.data;
};

/** Paper Trading 계정 상세 조회 */
export const getPaperTradingAccount = async (id: string): Promise<PaperTradingAccount> => {
  const response = await api.get(`/paper-trading/accounts/${id}`);
  return response.data;
};

/** Paper Trading 포지션 조회 */
export const getPaperTradingPositions = async (accountId: string): Promise<PaperTradingPositionsResponse> => {
  const response = await api.get(`/paper-trading/accounts/${accountId}/positions`);
  return response.data;
};

/** Paper Trading 체결 내역 조회 */
export const getPaperTradingExecutions = async (accountId: string): Promise<PaperTradingExecutionsResponse> => {
  const response = await api.get(`/paper-trading/accounts/${accountId}/executions`);
  return response.data;
};

/** Paper Trading 계정 초기화 */
export const resetPaperTradingAccount = async (accountId: string): Promise<ResetAccountResponse> => {
  const response = await api.post(`/paper-trading/accounts/${accountId}/reset`);
  return response.data;
};

// ==================== Paper Trading (전략 기반) ====================

/** Paper Trading 세션 상태 */
export interface PaperTradingSession {
  strategyId: string;
  credentialId: string;
  status: 'running' | 'stopped' | 'paused';
  initialBalance: string;
  currentBalance: string;
  positionCount: number;
  tradeCount: number;
  realizedPnl: string;
  unrealizedPnl: string;
  returnPct: string;
  startedAt: string | null;
}

/** Paper Trading 세션 목록 응답 */
export interface PaperTradingSessionsResponse {
  sessions: PaperTradingSession[];
  total: number;
}

/** Paper Trading 시작 요청 */
export interface PaperTradingStartRequest {
  credentialId: string;
  initialBalance?: number;
}

/** Paper Trading 액션 응답 */
export interface PaperTradingActionResponse {
  success: boolean;
  strategyId: string;
  action: string;
  message: string;
}

/** Paper Trading 세션 목록 조회 */
export const listPaperTradingSessions = async (): Promise<PaperTradingSessionsResponse> => {
  const response = await api.get('/paper-trading/strategies');
  return response.data;
};

/** 전략별 Paper Trading 상태 조회 */
export const getPaperTradingStatus = async (strategyId: string): Promise<PaperTradingSession> => {
  const response = await api.get(`/paper-trading/strategies/${strategyId}/status`);
  return response.data;
};

/** Paper Trading 시작 */
export const startPaperTrading = async (
  strategyId: string,
  request: PaperTradingStartRequest
): Promise<PaperTradingActionResponse> => {
  const response = await api.post(`/paper-trading/strategies/${strategyId}/start`, request);
  return response.data;
};

/** Paper Trading 중지 */
export const stopPaperTrading = async (strategyId: string): Promise<PaperTradingActionResponse> => {
  const response = await api.post(`/paper-trading/strategies/${strategyId}/stop`);
  return response.data;
};

/** Paper Trading 리셋 */
export const resetPaperTrading = async (strategyId: string): Promise<PaperTradingActionResponse> => {
  const response = await api.post(`/paper-trading/strategies/${strategyId}/reset`);
  return response.data;
};

/** 전략별 Paper Trading 포지션 조회 */
export const getStrategyPaperTradingPositions = async (
  strategyId: string
): Promise<PaperTradingPositionsResponse> => {
  const response = await api.get(`/paper-trading/strategies/${strategyId}/positions`);
  return response.data;
};

/** 전략별 Paper Trading 체결 내역 조회 */
export const getStrategyPaperTradingTrades = async (
  strategyId: string
): Promise<PaperTradingExecutionsResponse> => {
  const response = await api.get(`/paper-trading/strategies/${strategyId}/trades`);
  return response.data;
};

// ==================== 인증 ====================

export const login = async (username: string, password: string) => {
  const response = await api.post('/auth/login', { username, password });
  const { token } = response.data;
  localStorage.setItem('auth_token', token);
  return response.data;
};

export const logout = () => {
  localStorage.removeItem('auth_token');
};

export default api;
