use tokio::sync::mpsc;

use trader_core::QuoteData;

#[derive(Debug, Clone)]
pub enum DbWsMessage {
    Trade(QuoteData),
    Error(String),
}

#[derive(Debug)]
pub enum DbWsCommand {
    Subscribe(String),
    Unsubscribe(String),
}

pub struct DbInvestmentWebSocket {
    tx: mpsc::Sender<DbWsMessage>,
    rx: Option<mpsc::Receiver<DbWsMessage>>,
}

impl Default for DbInvestmentWebSocket {
    fn default() -> Self {
        Self::new()
    }
}

impl DbInvestmentWebSocket {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(100);
        Self {
            tx,
            rx: Some(rx),
        }
    }

    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<DbWsMessage>> {
        self.rx.take()
    }

    pub async fn connect(&mut self) {
        // Placeholder as DB Investment public OpenAPI doesn't seem to provide a public WebSocket
        // for individual token tickers easily via standard API.
        let _ = self.tx.send(DbWsMessage::Error("WebSocket not supported for DB Investment".into())).await;
    }
}
