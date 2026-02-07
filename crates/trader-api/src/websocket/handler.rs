//! WebSocket 연결 handler.
//!
//! Axum WebSocket 엔드포인트 및 메시지 처리.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::messages::{ClientMessage, ServerMessage};
use super::subscriptions::SharedSubscriptionManager;
use crate::auth::{decode_token, Claims};
use crate::metrics::{decrement_websocket_connections, increment_websocket_connections};
use crate::services::MarketStreamHandle;
use crate::state::AppState;

/// WebSocket 상태.
///
/// 구독 관리자를 포함한 WebSocket 서버 상태.
#[derive(Clone)]
pub struct WsState {
    /// 구독 관리자
    pub subscriptions: SharedSubscriptionManager,
    /// JWT 시크릿 (인증용)
    pub jwt_secret: String,
    /// credential_id별 MarketStream 핸들 (실시간 구독 전달용).
    ///
    /// 프론트엔드에서 `market:{symbol}` 구독 시 거래소 스트림에도 구독을 전달합니다.
    pub market_streams: Option<Arc<RwLock<HashMap<Uuid, Arc<MarketStreamHandle>>>>>,
}

impl WsState {
    /// 새로운 WebSocket 상태 생성.
    pub fn new(subscriptions: SharedSubscriptionManager, jwt_secret: impl Into<String>) -> Self {
        Self {
            subscriptions,
            jwt_secret: jwt_secret.into(),
            market_streams: None,
        }
    }

    /// MarketStream 핸들 설정.
    pub fn with_market_streams(
        mut self,
        streams: Arc<RwLock<HashMap<Uuid, Arc<MarketStreamHandle>>>>,
    ) -> Self {
        self.market_streams = Some(streams);
        self
    }
}

/// WebSocket 업그레이드 핸들러.
///
/// HTTP 연결을 WebSocket으로 업그레이드합니다.
///
/// # 엔드포인트
///
/// `GET /ws`
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(ws_state): State<WsState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, ws_state))
}

/// WebSocket 연결 처리.
async fn handle_socket(socket: WebSocket, state: WsState) {
    let session_id = uuid::Uuid::new_v4().to_string();
    info!("WebSocket connected: {}", session_id);

    // WebSocket 연결 메트릭 증가
    increment_websocket_connections();

    // 구독 관리자에 세션 등록
    let mut broadcast_rx = state.subscriptions.register(&session_id).await;

    // WebSocket 스트림 분리
    let (mut sender, mut receiver) = socket.split();

    // 환영 메시지 전송
    let welcome = ServerMessage::Welcome {
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: Utc::now().timestamp_millis(),
    };
    if let Ok(json) = welcome.to_json() {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // 클라이언트 메시지 수신 태스크
    let session_id_clone = session_id.clone();
    let state_clone = state.clone();
    let receive_task = tokio::spawn(async move {
        while let Some(result) = receiver.next().await {
            match result {
                Ok(msg) => {
                    if !handle_client_message(&session_id_clone, msg, &state_clone).await {
                        break;
                    }
                }
                Err(e) => {
                    warn!("WebSocket receive error: {}", e);
                    break;
                }
            }
        }
    });

    // 브로드캐스트 메시지 전송 태스크
    let session_id_clone = session_id.clone();
    let state_clone = state.clone();
    let send_task = tokio::spawn(async move {
        loop {
            match broadcast_rx.recv().await {
                Ok(msg) => {
                    // 이 세션이 메시지를 수신해야 하는지 확인
                    if state_clone
                        .subscriptions
                        .should_session_receive(&session_id_clone, &msg)
                        .await
                    {
                        if let Ok(json) = msg.to_json() {
                            if sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("WebSocket lagged by {} messages", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    // 하나의 태스크가 종료되면 다른 것도 종료
    tokio::select! {
        _ = receive_task => {
            debug!("Receive task ended for session: {}", session_id);
        }
        _ = send_task => {
            debug!("Send task ended for session: {}", session_id);
        }
    }

    // 세션 정리
    state.subscriptions.unregister(&session_id).await;

    // WebSocket 연결 메트릭 감소
    decrement_websocket_connections();

    info!("WebSocket disconnected: {}", session_id);
}

/// 클라이언트 메시지 처리.
///
/// # Returns
///
/// `true`면 연결 유지, `false`면 연결 종료
async fn handle_client_message(session_id: &str, msg: Message, state: &WsState) -> bool {
    match msg {
        Message::Text(text) => {
            match ClientMessage::from_json(&text) {
                Ok(client_msg) => process_client_message(session_id, client_msg, state).await,
                Err(e) => {
                    warn!("Invalid message from {}: {}", session_id, e);
                    // 에러 응답 브로드캐스트 (해당 세션에만 전달됨)
                    let _ = state
                        .subscriptions
                        .broadcast(ServerMessage::error("INVALID_MESSAGE", e.to_string()));
                    true // 연결은 유지
                }
            }
        }
        Message::Binary(_) => {
            warn!("Binary messages not supported");
            true
        }
        Message::Ping(_) => true,
        Message::Pong(_) => true,
        Message::Close(_) => {
            debug!("Close message received from {}", session_id);
            false
        }
    }
}

/// 파싱된 클라이언트 메시지 처리.
async fn process_client_message(session_id: &str, msg: ClientMessage, state: &WsState) -> bool {
    match msg {
        ClientMessage::Subscribe { channels } => {
            let subscribed = state.subscriptions.subscribe(session_id, &channels).await;
            debug!("Session {} subscribed to: {:?}", session_id, subscribed);

            // 거래소 스트림에 심볼 구독 전달
            if let Some(ref market_streams) = state.market_streams {
                forward_subscribe_to_exchange_streams(market_streams, &subscribed).await;
            }

            let response = ServerMessage::Subscribed {
                channels: subscribed,
            };
            let _ = state.subscriptions.broadcast(response);
            true
        }

        ClientMessage::Unsubscribe { channels } => {
            let unsubscribed = state.subscriptions.unsubscribe(session_id, &channels).await;
            debug!(
                "Session {} unsubscribed from: {:?}",
                session_id, unsubscribed
            );

            let response = ServerMessage::Unsubscribed {
                channels: unsubscribed,
            };
            let _ = state.subscriptions.broadcast(response);
            true
        }

        ClientMessage::Ping => {
            let response = ServerMessage::Pong {
                timestamp: Utc::now().timestamp_millis(),
            };
            let _ = state.subscriptions.broadcast(response);
            true
        }

        ClientMessage::Auth { token } => {
            match decode_token(&token, &state.jwt_secret) {
                Ok(token_data) => {
                    let claims: Claims = token_data.claims;
                    state
                        .subscriptions
                        .authenticate(session_id, &claims.sub)
                        .await;

                    info!(
                        "Session {} authenticated as user {}",
                        session_id, claims.sub
                    );

                    let response = ServerMessage::AuthResult {
                        success: true,
                        message: "Authenticated successfully".to_string(),
                        user_id: Some(claims.sub),
                    };
                    let _ = state.subscriptions.broadcast(response);
                }
                Err(e) => {
                    warn!("Auth failed for session {}: {}", session_id, e);

                    let response = ServerMessage::AuthResult {
                        success: false,
                        message: format!("Authentication failed: {}", e),
                        user_id: None,
                    };
                    let _ = state.subscriptions.broadcast(response);
                }
            }
            true
        }
    }
}

/// WebSocket 라우터 생성.
///
/// AppState와 함께 사용할 수 있는 WebSocket 라우터.
/// 별도의 WsState가 필요합니다.
pub fn websocket_router(ws_state: WsState) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(websocket_handler))
        .with_state(ws_state)
}

/// 독립적인 WebSocket 라우터 생성.
///
/// WsState만으로 동작하는 라우터.
pub fn standalone_websocket_router(ws_state: WsState) -> Router {
    Router::new()
        .route("/", get(websocket_handler))
        .with_state(ws_state)
}

/// 프론트엔드 구독을 거래소 스트림에 전달.
///
/// `market:{symbol}` 형식의 채널에서 심볼을 추출하여,
/// 모든 활성 거래소 스트림에 구독을 요청합니다.
/// 이미 구독 중인 심볼이면 참조 카운트만 증가합니다.
async fn forward_subscribe_to_exchange_streams(
    market_streams: &Arc<RwLock<HashMap<Uuid, Arc<MarketStreamHandle>>>>,
    channels: &[String],
) {
    // market:{symbol} 형식에서 심볼 추출
    let symbols: Vec<&str> = channels
        .iter()
        .filter_map(|ch| ch.strip_prefix("market:"))
        .collect();

    if symbols.is_empty() {
        return;
    }

    let streams = market_streams.read().await;
    if streams.is_empty() {
        return;
    }

    for symbol in symbols {
        for handle in streams.values() {
            if let Err(e) = handle.subscribe(symbol).await {
                debug!(
                    symbol = %symbol,
                    credential_id = %handle.credential_id(),
                    error = %e,
                    "거래소 스트림 구독 전달 실패"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::subscriptions::{create_subscription_manager, Subscription};
    use super::*;

    #[test]
    fn test_ws_state_creation() {
        let subscriptions = create_subscription_manager(100);
        let state = WsState::new(subscriptions, "test-secret");

        assert_eq!(state.jwt_secret, "test-secret");
    }

    #[tokio::test]
    async fn test_subscription_manager_integration() {
        let subscriptions = create_subscription_manager(100);
        let _state = WsState::new(subscriptions.clone(), "test-secret");

        // 세션 등록
        let _rx = subscriptions.register("test-session").await;
        assert_eq!(subscriptions.client_count().await, 1);

        // 구독
        subscriptions
            .subscribe("test-session", &["orders".to_string()])
            .await;

        assert_eq!(
            subscriptions.subscriber_count(&Subscription::Orders).await,
            1
        );
    }
}
