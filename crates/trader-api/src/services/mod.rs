//! 백그라운드 서비스 모듈.
//!
//! 전략 실행, 컨텍스트 동기화 등 백그라운드에서 실행되는 서비스들을 제공합니다.

pub mod context_sync;
pub mod market_stream;
pub mod signal_alert;
pub mod signal_processor;
pub mod telegram_bot;

pub use context_sync::start_context_sync_service;
pub use market_stream::{get_or_create_market_stream, MarketStreamHandle};
pub use signal_alert::{SignalAlertFilter, SignalAlertService};
pub use signal_processor::{SignalProcessingService, start_signal_processing_service};
pub use telegram_bot::ApiBotHandler;
