//! WebSocket 메시지 타입.
//!
//! 클라이언트-서버 간 교환되는 메시지 정의.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// WebSocket 에러.
#[derive(Debug, thiserror::Error)]
pub enum WsError {
    #[error("잘못된 메시지 형식: {0}")]
    InvalidMessage(String),
    #[error("알 수 없는 메시지 타입: {0}")]
    UnknownMessageType(String),
    #[error("직렬화 실패: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("내부 오류: {0}")]
    InternalError(String),
}

// ==================== 클라이언트 → 서버 메시지 ====================

/// 클라이언트에서 서버로 보내는 메시지.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// 채널 구독
    Subscribe {
        /// 구독할 채널 목록
        channels: Vec<String>,
    },
    /// 채널 구독 해제
    Unsubscribe {
        /// 구독 해제할 채널 목록
        channels: Vec<String>,
    },
    /// 핑 (연결 유지)
    Ping,
    /// 인증 (JWT 토큰)
    Auth {
        /// JWT 토큰
        token: String,
    },
}

impl ClientMessage {
    /// JSON 문자열에서 파싱.
    pub fn from_json(json: &str) -> Result<Self, WsError> {
        serde_json::from_str(json).map_err(|e| WsError::InvalidMessage(e.to_string()))
    }
}

// ==================== 서버 → 클라이언트 메시지 ====================

/// 서버에서 클라이언트로 보내는 메시지.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// 구독 확인
    Subscribed {
        /// 구독된 채널 목록
        channels: Vec<String>,
    },
    /// 구독 해제 확인
    Unsubscribed {
        /// 구독 해제된 채널 목록
        channels: Vec<String>,
    },
    /// 퐁 응답
    Pong {
        /// 서버 타임스탬프
        timestamp: i64,
    },
    /// 인증 결과
    AuthResult {
        /// 성공 여부
        success: bool,
        /// 메시지
        message: String,
        /// 사용자 ID (성공 시)
        #[serde(skip_serializing_if = "Option::is_none")]
        user_id: Option<String>,
    },
    /// 에러
    Error {
        /// 에러 코드
        code: String,
        /// 에러 메시지
        message: String,
    },
    /// 티커 데이터
    Ticker(TickerData),
    /// 체결 데이터
    Trade(TradeData),
    /// 호가창 데이터
    OrderBook(OrderBookData),
    /// 캔들스틱(Kline) 데이터
    Kline(KlineData),
    /// 주문 업데이트
    OrderUpdate(OrderUpdateData),
    /// 포지션 업데이트
    PositionUpdate(PositionUpdateData),
    /// 전략 상태 업데이트
    StrategyUpdate(StrategyUpdateData),
    /// 시뮬레이션 업데이트
    SimulationUpdate(SimulationUpdateData),
    /// 연결 환영 메시지
    Welcome {
        /// 서버 버전
        version: String,
        /// 서버 타임스탬프
        timestamp: i64,
    },
    /// 활성 계정 변경 알림
    ActiveAccountChanged(ActiveAccountChangedData),
    /// Signal 충돌 알림
    SignalConflict(SignalConflictData),
}

/// Signal 충돌 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalConflictData {
    /// 전략 ID
    pub strategy_id: String,
    /// 티커
    pub ticker: String,
    /// 신호 유형 (Entry, Exit 등)
    pub signal_type: String,
    /// 충돌 유형 (pending_order, duplicate_position, no_position, insufficient_balance)
    pub conflict_type: String,
    /// 충돌 메시지 (사람이 읽을 수 있는 형태)
    pub message: String,
    /// 타임스탬프
    pub timestamp: i64,
}

/// 활성 계정 변경 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveAccountChangedData {
    /// 새로 활성화된 credential ID (해제 시 None)
    pub credential_id: Option<String>,
    /// 거래소 ID (예: "kis", "mock")
    pub exchange_id: Option<String>,
    /// 거래소 이름 (예: "한국투자증권")
    pub exchange_name: Option<String>,
    /// 변경 타임스탬프
    pub timestamp: i64,
}

impl ServerMessage {
    /// JSON 문자열로 직렬화.
    pub fn to_json(&self) -> Result<String, WsError> {
        serde_json::to_string(self).map_err(WsError::from)
    }

    /// 에러 메시지 생성 헬퍼.
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        ServerMessage::Error {
            code: code.into(),
            message: message.into(),
        }
    }
}

// ==================== 데이터 타입 ====================

/// 티커 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerData {
    /// 심볼
    pub symbol: String,
    /// 현재가
    pub price: Decimal,
    /// 24시간 변화율 (%)
    pub change_24h: Decimal,
    /// 24시간 거래량
    pub volume_24h: Decimal,
    /// 최고가 (24시간)
    pub high_24h: Decimal,
    /// 최저가 (24시간)
    pub low_24h: Decimal,
    /// 타임스탬프
    pub timestamp: i64,
}

/// 캔들스틱(Kline) 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineData {
    /// 심볼
    pub symbol: String,
    /// 타임프레임 (예: "1m", "5m", "1h", "1d")
    pub timeframe: String,
    /// 시가
    pub open: Decimal,
    /// 고가
    pub high: Decimal,
    /// 저가
    pub low: Decimal,
    /// 종가
    pub close: Decimal,
    /// 거래량
    pub volume: Decimal,
    /// 캔들 시작 타임스탬프 (밀리초)
    pub open_time: i64,
    /// 캔들 종료 타임스탬프 (밀리초)
    pub close_time: i64,
    /// 캔들 완료 여부
    pub is_closed: bool,
}

/// 체결 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeData {
    /// 심볼
    pub symbol: String,
    /// 체결 ID
    pub trade_id: String,
    /// 체결 가격
    pub price: Decimal,
    /// 체결 수량
    pub quantity: Decimal,
    /// 매수/매도
    pub side: String,
    /// 타임스탬프
    pub timestamp: i64,
}

/// 주문 업데이트 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderUpdateData {
    /// 주문 ID
    pub order_id: String,
    /// 심볼
    pub symbol: String,
    /// 주문 상태
    pub status: String,
    /// 주문 방향
    pub side: String,
    /// 주문 유형
    pub order_type: String,
    /// 주문 수량
    pub quantity: Decimal,
    /// 체결 수량
    pub filled_quantity: Decimal,
    /// 주문 가격
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Decimal>,
    /// 평균 체결 가격
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_price: Option<Decimal>,
    /// 타임스탬프
    pub timestamp: i64,
}

/// 포지션 업데이트 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionUpdateData {
    /// 심볼
    pub symbol: String,
    /// 포지션 방향
    pub side: String,
    /// 수량
    pub quantity: Decimal,
    /// 진입가
    pub entry_price: Decimal,
    /// 현재가
    pub current_price: Decimal,
    /// 미실현 손익
    pub unrealized_pnl: Decimal,
    /// 수익률 (%)
    pub return_pct: Decimal,
    /// 타임스탬프
    pub timestamp: i64,
}

/// 전략 업데이트 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyUpdateData {
    /// 전략 ID
    pub strategy_id: String,
    /// 전략 이름
    pub name: String,
    /// 실행 상태
    pub running: bool,
    /// 이벤트 타입 (started, stopped, signal_generated, error)
    pub event: String,
    /// 추가 데이터
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    /// 타임스탬프
    pub timestamp: i64,
}

/// 호가창 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookData {
    /// 심볼
    pub symbol: String,
    /// 매수 호가 리스트
    pub bids: Vec<OrderBookLevel>,
    /// 매도 호가 리스트
    pub asks: Vec<OrderBookLevel>,
    /// 타임스탬프
    pub timestamp: i64,
}

/// 호가 레벨.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookLevel {
    /// 가격
    pub price: Decimal,
    /// 수량
    pub quantity: Decimal,
}

/// 시뮬레이션 업데이트 데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationUpdateData {
    /// 이벤트 타입 (started, stopped, paused, trade, position_update, status)
    pub event: String,
    /// 현재 상태 (running, paused, stopped)
    pub state: String,
    /// 현재 잔고
    pub balance: Decimal,
    /// 총 자산
    pub equity: Decimal,
    /// 미실현 손익
    pub unrealized_pnl: Decimal,
    /// 실현 손익
    pub realized_pnl: Decimal,
    /// 포지션 수
    pub position_count: usize,
    /// 거래 수
    pub trade_count: usize,
    /// 추가 데이터 (거래 정보, 포지션 정보 등)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    /// 타임스탬프
    pub timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_subscribe() {
        let json = r#"{"type": "subscribe", "channels": ["market:BTC-USDT", "orders"]}"#;
        let msg = ClientMessage::from_json(json).unwrap();

        match msg {
            ClientMessage::Subscribe { channels } => {
                assert_eq!(channels.len(), 2);
                assert_eq!(channels[0], "market:BTC-USDT");
            }
            _ => panic!("Expected Subscribe message"),
        }
    }

    #[test]
    fn test_client_message_ping() {
        let json = r#"{"type": "ping"}"#;
        let msg = ClientMessage::from_json(json).unwrap();

        assert!(matches!(msg, ClientMessage::Ping));
    }

    #[test]
    fn test_server_message_serialization() {
        let msg = ServerMessage::Pong {
            timestamp: 1234567890,
        };
        let json = msg.to_json().unwrap();

        assert!(json.contains("pong"));
        assert!(json.contains("1234567890"));
    }

    #[test]
    fn test_server_error_message() {
        let msg = ServerMessage::error("INVALID_CHANNEL", "Unknown channel");
        let json = msg.to_json().unwrap();

        assert!(json.contains("error"));
        assert!(json.contains("INVALID_CHANNEL"));
    }

    #[test]
    fn test_ticker_data() {
        use rust_decimal_macros::dec;

        let ticker = TickerData {
            symbol: "BTC-USDT".to_string(),
            price: dec!(50000.0),
            change_24h: dec!(2.5),
            volume_24h: dec!(1000000.0),
            high_24h: dec!(51000.0),
            low_24h: dec!(49000.0),
            timestamp: 1234567890,
        };

        let msg = ServerMessage::Ticker(ticker);
        let json = msg.to_json().unwrap();

        assert!(json.contains("ticker"));
        assert!(json.contains("BTC-USDT"));
    }
}
