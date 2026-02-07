//! WebSocket subscription 관리.
//!
//! 클라이언트 구독 관리 및 메시지 브로드캐스트.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use super::messages::ServerMessage;

/// 구독 채널 타입.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Subscription {
    /// 특정 심볼의 시장 데이터
    Market(String),
    /// 주문 업데이트
    Orders,
    /// 포지션 업데이트
    Positions,
    /// 전략 업데이트
    Strategies,
    /// 모든 시장 데이터 (요약)
    AllMarkets,
    /// 시뮬레이션 업데이트
    Simulation,
    /// 계정 변경 알림 (활성 거래소 변경 등)
    Account,
}

impl Subscription {
    /// 문자열에서 구독 채널 파싱.
    ///
    /// # 형식
    ///
    /// - `market:{symbol}` - 특정 심볼의 시장 데이터
    /// - `orders` - 주문 업데이트
    /// - `positions` - 포지션 업데이트
    /// - `strategies` - 전략 업데이트
    /// - `all_markets` - 모든 시장 요약
    pub fn from_channel(channel: &str) -> Option<Self> {
        if let Some(symbol) = channel.strip_prefix("market:") {
            Some(Subscription::Market(symbol.to_uppercase()))
        } else {
            match channel.to_lowercase().as_str() {
                "orders" => Some(Subscription::Orders),
                "positions" => Some(Subscription::Positions),
                "strategies" => Some(Subscription::Strategies),
                "all_markets" => Some(Subscription::AllMarkets),
                "simulation" => Some(Subscription::Simulation),
                "account" => Some(Subscription::Account),
                _ => None,
            }
        }
    }

    /// 구독 채널을 문자열로 변환.
    pub fn to_channel(&self) -> String {
        match self {
            Subscription::Market(symbol) => format!("market:{}", symbol),
            Subscription::Orders => "orders".to_string(),
            Subscription::Positions => "positions".to_string(),
            Subscription::Strategies => "strategies".to_string(),
            Subscription::AllMarkets => "all_markets".to_string(),
            Subscription::Simulation => "simulation".to_string(),
            Subscription::Account => "account".to_string(),
        }
    }

    /// 메시지가 이 구독에 해당하는지 확인.
    pub fn matches(&self, message: &ServerMessage) -> bool {
        match (self, message) {
            (Subscription::Market(symbol), ServerMessage::Ticker(data)) => {
                data.symbol.to_uppercase() == *symbol
            }
            (Subscription::Market(symbol), ServerMessage::Trade(data)) => {
                data.symbol.to_uppercase() == *symbol
            }
            (Subscription::Orders, ServerMessage::OrderUpdate(_)) => true,
            (Subscription::Positions, ServerMessage::PositionUpdate(_)) => true,
            (Subscription::Strategies, ServerMessage::StrategyUpdate(_)) => true,
            (Subscription::AllMarkets, ServerMessage::Ticker(_)) => true,
            (Subscription::Simulation, ServerMessage::SimulationUpdate(_)) => true,
            (Subscription::Account, ServerMessage::ActiveAccountChanged(_)) => true,
            _ => false,
        }
    }
}

/// 클라이언트 세션 정보.
#[derive(Debug)]
pub struct ClientSession {
    /// 세션 ID
    pub id: String,
    /// 구독 중인 채널 목록
    pub subscriptions: HashSet<Subscription>,
    /// 인증 여부
    pub authenticated: bool,
    /// 사용자 ID (인증된 경우)
    pub user_id: Option<String>,
}

impl ClientSession {
    /// 새로운 세션 생성.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            subscriptions: HashSet::new(),
            authenticated: false,
            user_id: None,
        }
    }

    /// 채널 구독 추가.
    pub fn subscribe(&mut self, subscription: Subscription) {
        self.subscriptions.insert(subscription);
    }

    /// 채널 구독 해제.
    pub fn unsubscribe(&mut self, subscription: &Subscription) {
        self.subscriptions.remove(subscription);
    }

    /// 메시지를 수신해야 하는지 확인.
    pub fn should_receive(&self, message: &ServerMessage) -> bool {
        self.subscriptions.iter().any(|sub| sub.matches(message))
    }

    /// 인증 설정.
    pub fn authenticate(&mut self, user_id: impl Into<String>) {
        self.authenticated = true;
        self.user_id = Some(user_id.into());
    }
}

/// 구독 관리자.
///
/// 모든 WebSocket 클라이언트의 구독을 관리하고 메시지를 브로드캐스트합니다.
pub struct SubscriptionManager {
    /// 메시지 브로드캐스트 채널
    broadcast_tx: broadcast::Sender<ServerMessage>,
    /// 클라이언트 세션 목록
    sessions: RwLock<HashMap<String, ClientSession>>,
}

impl SubscriptionManager {
    /// 새로운 구독 관리자 생성.
    ///
    /// # Arguments
    ///
    /// * `capacity` - 브로드캐스트 채널 버퍼 크기
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            broadcast_tx: tx,
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// 새 클라이언트 세션 등록.
    ///
    /// # Returns
    ///
    /// 브로드캐스트 수신기
    pub async fn register(&self, session_id: &str) -> broadcast::Receiver<ServerMessage> {
        let session = ClientSession::new(session_id);
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.to_string(), session);
        self.broadcast_tx.subscribe()
    }

    /// 클라이언트 세션 제거.
    pub async fn unregister(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
    }

    /// 채널 구독 추가.
    pub async fn subscribe(&self, session_id: &str, channels: &[String]) -> Vec<String> {
        let mut sessions = self.sessions.write().await;
        let mut subscribed = Vec::new();

        if let Some(session) = sessions.get_mut(session_id) {
            for channel in channels {
                if let Some(subscription) = Subscription::from_channel(channel) {
                    session.subscribe(subscription);
                    subscribed.push(channel.clone());
                }
            }
        }

        subscribed
    }

    /// 채널 구독 해제.
    pub async fn unsubscribe(&self, session_id: &str, channels: &[String]) -> Vec<String> {
        let mut sessions = self.sessions.write().await;
        let mut unsubscribed = Vec::new();

        if let Some(session) = sessions.get_mut(session_id) {
            for channel in channels {
                if let Some(subscription) = Subscription::from_channel(channel) {
                    session.unsubscribe(&subscription);
                    unsubscribed.push(channel.clone());
                }
            }
        }

        unsubscribed
    }

    /// 세션 인증.
    pub async fn authenticate(&self, session_id: &str, user_id: &str) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.authenticate(user_id);
        }
    }

    /// 메시지 브로드캐스트.
    ///
    /// 구독 중인 모든 클라이언트에게 메시지를 전송합니다.
    #[allow(clippy::result_large_err)]
    pub fn broadcast(
        &self,
        message: ServerMessage,
    ) -> Result<usize, broadcast::error::SendError<ServerMessage>> {
        self.broadcast_tx.send(message)
    }

    /// 특정 구독 채널에만 메시지 브로드캐스트.
    #[allow(clippy::result_large_err)]
    pub fn broadcast_to_channel(
        &self,
        _subscription: &Subscription,
        message: ServerMessage,
    ) -> Result<usize, broadcast::error::SendError<ServerMessage>> {
        // 브로드캐스트 채널은 모든 수신자에게 전송
        // 각 클라이언트에서 구독 여부를 확인하여 필터링
        self.broadcast_tx.send(message)
    }

    /// 세션이 메시지를 수신해야 하는지 확인.
    pub async fn should_session_receive(&self, session_id: &str, message: &ServerMessage) -> bool {
        let sessions = self.sessions.read().await;
        sessions
            .get(session_id)
            .map(|s| s.should_receive(message))
            .unwrap_or(false)
    }

    /// 연결된 클라이언트 수.
    pub async fn client_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// 특정 채널 구독자 수.
    pub async fn subscriber_count(&self, subscription: &Subscription) -> usize {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .filter(|s| s.subscriptions.contains(subscription))
            .count()
    }

    /// 현재 구독된 모든 시장 심볼 목록 반환.
    pub async fn get_subscribed_market_symbols(&self) -> HashSet<String> {
        let sessions = self.sessions.read().await;
        let mut symbols = HashSet::new();

        for session in sessions.values() {
            for subscription in &session.subscriptions {
                if let Subscription::Market(symbol) = subscription {
                    symbols.insert(symbol.clone());
                }
            }
        }

        symbols
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new(1024) // 기본 버퍼 크기
    }
}

/// 공유 가능한 구독 관리자 타입.
pub type SharedSubscriptionManager = Arc<SubscriptionManager>;

/// 새로운 공유 구독 관리자 생성.
pub fn create_subscription_manager(capacity: usize) -> SharedSubscriptionManager {
    Arc::new(SubscriptionManager::new(capacity))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_from_channel() {
        assert_eq!(
            Subscription::from_channel("market:BTC-USDT"),
            Some(Subscription::Market("BTC-USDT".to_string()))
        );
        assert_eq!(
            Subscription::from_channel("orders"),
            Some(Subscription::Orders)
        );
        assert_eq!(
            Subscription::from_channel("positions"),
            Some(Subscription::Positions)
        );
        assert_eq!(Subscription::from_channel("unknown"), None);
    }

    #[test]
    fn test_subscription_to_channel() {
        assert_eq!(
            Subscription::Market("BTC-USDT".to_string()).to_channel(),
            "market:BTC-USDT"
        );
        assert_eq!(Subscription::Orders.to_channel(), "orders");
    }

    #[test]
    fn test_client_session() {
        let mut session = ClientSession::new("session-1");

        assert!(!session.authenticated);
        assert!(session.subscriptions.is_empty());

        session.subscribe(Subscription::Orders);
        session.subscribe(Subscription::Market("BTC-USDT".to_string()));

        assert_eq!(session.subscriptions.len(), 2);

        session.authenticate("user-123");
        assert!(session.authenticated);
        assert_eq!(session.user_id, Some("user-123".to_string()));
    }

    #[tokio::test]
    async fn test_subscription_manager() {
        let manager = SubscriptionManager::new(100);

        // 세션 등록
        let _rx = manager.register("session-1").await;
        assert_eq!(manager.client_count().await, 1);

        // 구독 추가
        let subscribed = manager
            .subscribe(
                "session-1",
                &["market:BTC-USDT".to_string(), "orders".to_string()],
            )
            .await;
        assert_eq!(subscribed.len(), 2);

        // 구독자 수 확인
        assert_eq!(manager.subscriber_count(&Subscription::Orders).await, 1);

        // 세션 제거
        manager.unregister("session-1").await;
        assert_eq!(manager.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_broadcast() {
        use super::super::messages::TickerData;
        use rust_decimal_macros::dec;

        let manager = SubscriptionManager::new(100);
        let mut rx = manager.register("session-1").await;

        manager
            .subscribe("session-1", &["market:BTC-USDT".to_string()])
            .await;

        // 메시지 브로드캐스트
        let ticker = ServerMessage::Ticker(TickerData {
            symbol: "BTC-USDT".to_string(),
            price: dec!(50000.0),
            change_24h: dec!(2.5),
            volume_24h: dec!(1000000.0),
            high_24h: dec!(51000.0),
            low_24h: dec!(49000.0),
            timestamp: 1234567890,
        });

        manager.broadcast(ticker).unwrap();

        // 수신 확인
        let received = rx.try_recv().unwrap();
        assert!(matches!(received, ServerMessage::Ticker(_)));
    }
}
