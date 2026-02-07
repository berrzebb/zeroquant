//! 실시간 데이터 스트리밍을 위한 WebSocket 서버.
//!
//! 실시간 시장 데이터, 주문, 포지션 업데이트를 위한 WebSocket 서버.
//!
//! # 구독 채널
//!
//! - `market:{symbol}` - 특정 심볼의 시장 데이터 (ticker, trades)
//! - `orders` - 주문 상태 업데이트
//! - `positions` - 포지션 업데이트
//! - `strategies` - 전략 상태 변경
//!
//! # 메시지 형식
//!
//! 모든 메시지는 JSON 형식으로 교환됩니다.
//!
//! ## 클라이언트 → 서버
//!
//! ```json
//! {"type": "subscribe", "channels": ["market:BTC-USDT", "orders"]}
//! {"type": "unsubscribe", "channels": ["market:BTC-USDT"]}
//! {"type": "ping"}
//! ```
//!
//! ## 서버 → 클라이언트
//!
//! ```json
//! {"type": "ticker", "data": {...}}
//! {"type": "order_update", "data": {...}}
//! {"type": "pong"}
//! ```

pub mod aggregator;
pub mod handler;
pub mod messages;
pub mod simulator;
pub mod subscriptions;

pub use aggregator::{start_aggregator, MarketDataAggregator};
pub use handler::{standalone_websocket_router, websocket_handler, websocket_router, WsState};
pub use messages::{
    ActiveAccountChangedData, ClientMessage, OrderBookData, OrderBookLevel, OrderUpdateData,
    PositionUpdateData, ServerMessage, SignalConflictData, SimulationUpdateData,
    StrategyUpdateData, TickerData, TradeData, WsError,
};
pub use simulator::{start_simulator, MockDataSimulator};
pub use subscriptions::{
    create_subscription_manager, SharedSubscriptionManager, Subscription, SubscriptionManager,
};
