// SDUI 타입 (Server-Driven UI)
export * from './sdui';

// 자동 생성된 타입 re-export (ts-rs)
export * from './generated';

// Market data types
export interface Ticker {
  symbol: string;
  price: number;
  change: number;
  changePercent: number;
  high: number;
  low: number;
  volume: number;
  timestamp: number;
}

export interface Position {
  id: string;
  symbol: string;
  displayName?: string;  // "005930(삼성전자)" 형식
  side: 'Long' | 'Short';
  quantity: number;
  entryPrice: number;
  currentPrice: number;
  unrealizedPnl: number;
  unrealizedPnlPercent: number;
  market: 'KR' | 'US' | 'CRYPTO';
}

export interface Order {
  id: string;
  symbol: string;
  displayName?: string;  // "005930(삼성전자)" 형식
  side: 'Buy' | 'Sell';
  type: 'Market' | 'Limit' | 'StopLoss' | 'TakeProfit';
  quantity: number;
  price?: number;
  filledQuantity: number;
  status: 'Pending' | 'PartiallyFilled' | 'Filled' | 'Cancelled' | 'Rejected';
  createdAt: string;
}

// Strategy 타입은 types/generated/strategies/StrategyListItem에서 re-export됨
// 아래는 API 응답과 다른 포맷을 사용하는 레거시 컴포넌트용 타입
export interface LegacyStrategy {
  id: string;
  strategyType: string;  // 전략 타입 (예: "rsi", "grid_trading", "sma")
  name: string;
  status: 'Running' | 'Stopped' | 'Error';
  market: 'KR' | 'US' | 'CRYPTO';
  symbols: string[];
  timeframe: string;  // 타임프레임 (예: "1m", "15m", "1d")
  pnl: number;
  winRate: number;
  tradesCount: number;
}

export interface PortfolioSummary {
  totalValue: number;
  totalPnl: number;
  totalPnlPercent: number;
  dailyPnl: number;
  dailyPnlPercent: number;
  cashBalance: number;
  marginUsed: number;
}

export interface MarketStatus {
  market: 'KR' | 'US';
  isOpen: boolean;
  nextOpen?: string;
  nextClose?: string;
  session?: 'Regular' | 'PreMarket' | 'AfterHours';
}

// WebSocket message types
export type WsMessage =
  | WsWelcome
  | WsSubscribed
  | WsUnsubscribed
  | WsPong
  | WsAuthResult
  | WsError
  | WsTicker
  | WsOrderUpdate
  | WsPositionUpdate
  | WsStrategyUpdate
  | WsActiveAccountChanged;

export interface WsWelcome {
  type: 'welcome';
  version: string;
  timestamp: number;
}

export interface WsSubscribed {
  type: 'subscribed';
  channels: string[];
}

export interface WsUnsubscribed {
  type: 'unsubscribed';
  channels: string[];
}

export interface WsPong {
  type: 'pong';
  timestamp: number;
}

export interface WsAuthResult {
  type: 'auth_result';
  success: boolean;
  message: string;
  user_id?: string;
}

export interface WsError {
  type: 'error';
  code: string;
  message: string;
}

export interface WsTicker {
  type: 'ticker';
  symbol: string;
  price: string;
  change_24h: string;
  volume_24h: string;
  high_24h: string;
  low_24h: string;
  timestamp: number;
}

export interface WsOrderUpdate {
  type: 'order_update';
  order_id: string;
  symbol: string;
  status: string;
  side: string;
  order_type: string;
  quantity: string;
  filled_quantity: string;
  price?: string;
  average_price?: string;
  timestamp: number;
}

export interface WsPositionUpdate {
  type: 'position_update';
  symbol: string;
  side: string;
  quantity: string;
  entry_price: string;
  current_price: string;
  unrealized_pnl: string;
  return_pct: string;
  timestamp: number;
}

export interface WsStrategyUpdate {
  type: 'strategy_update';
  strategy_id: string;
  name: string;
  running: boolean;
  event: string;
  data?: unknown;
  timestamp: number;
}

export interface WsActiveAccountChanged {
  type: 'active_account_changed';
  credential_id: string | null;
  exchange_id: string | null;
  exchange_name: string | null;
  timestamp: number;
}

export interface Notification {
  id: string;
  type: 'info' | 'success' | 'warning' | 'error';
  title: string;
  message: string;
  timestamp: string;
}

// ==================== 자격증명 관리 타입 ====================

/** 자격증명 필드 정의 */
export interface CredentialField {
  name: string;
  label: string;
  field_type: 'text' | 'password' | 'select' | 'number';
  placeholder?: string;
  options?: string[];
  help_text?: string;
}

/** 지원되는 거래소 정보 */
export interface SupportedExchange {
  exchange_id: string;
  display_name: string;
  market_type: string;  // 'crypto', 'stock_kr', 'stock_us', 'forex', 'mock'
  supports_testnet: boolean;  // 모의투자/테스트넷 지원 여부
  required_fields: CredentialField[];
  optional_fields: CredentialField[];
  description?: string;  // 거래소 설명
  docs_url?: string;  // API 문서 URL
}

/** 저장된 거래소 자격증명 */
export interface ExchangeCredential {
  id: string;
  exchange_id: string;
  display_name: string;
  /** 시장 유형: 'crypto', 'stock_kr', 'stock_us', 'forex', 'mock' */
  market_type: string;
  is_active: boolean;
  is_testnet: boolean;
  /** 데이터 제공자 여부 (true이면 시세 데이터만 제공, 활성 계정으로 설정 불가) */
  is_data_provider: boolean;
  created_at: string;
  last_tested_at?: string;
  masked_api_key?: string;
}

/** 텔레그램 설정 정보 */
export interface TelegramSettings {
  configured: boolean;
  display_name?: string;
  masked_token?: string;
  masked_chat_id?: string;
  created_at?: string;
  updated_at?: string;
  last_tested_at?: string;
}

// ==================== Global Ranking 타입 ====================
// 기본 타입은 types/generated/ranking에서 re-export됨
// 아래는 UI 편의를 위한 리터럴 타입 (자동 생성 타입의 string을 더 구체화)

/** GlobalScore 등급 (UI 표시용) */
export type GlobalScoreGrade = 'EXCELLENT' | 'BUY' | 'WATCH' | 'HOLD' | 'CAUTION' | 'AVOID';

/** 신뢰도 레벨 (UI 표시용) */
export type ConfidenceLevel = 'HIGH' | 'MEDIUM' | 'LOW';

// ==================== RouteState 타입 ====================

/** RouteState 상태 */
export type RouteStateType = 'ATTACK' | 'ARMED' | 'NEUTRAL' | 'WAIT' | 'OVERHEAT';

/** RouteState 정보 */
export interface RouteStateInfo {
  state: RouteStateType;
  strength: number;
  updated_at: string;
}

// ==================== Signal Marker 타입 ====================

/** 신호 지표 정보 */
export interface SignalIndicators {
  rsi?: number;
  macd?: number;
  macd_signal?: number;
  macd_histogram?: number;
  sma_20?: number;
  sma_50?: number;
  sma_200?: number;
  bb_upper?: number;
  bb_middle?: number;
  bb_lower?: number;
  volume_ratio?: number;
  atr?: number;
  [key: string]: number | undefined;
}

/** 신호 마커 */
export interface SignalMarker {
  id: string;
  symbol: string;
  timestamp: string;
  signal_type: string;
  side?: 'Buy' | 'Sell';
  price: number;
  strength: number;
  indicators: SignalIndicators;
  reason: string;
  strategy_id: string;
  strategy_name: string;
  executed: boolean;
}
