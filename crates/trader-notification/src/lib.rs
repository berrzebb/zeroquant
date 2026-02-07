//! # Trader Notification
//!
//! 트레이딩 알림 서비스.
//!
//! 지원 채널:
//! - Telegram (Bot API)
//! - Email (SMTP)
//! - Discord (Webhook)
//! - Slack (Incoming Webhook)
//! - SMS (Twilio)
//!
//! # 텔레그램 봇 명령어
//!
//! 봇 명령어 핸들러를 통해 다음 명령어를 지원합니다:
//! - `/portfolio` - 포트폴리오 현황
//! - `/status` - 시스템 상태
//! - `/stop` - 전략 중지
//! - `/report` - 리포트 조회
//! - `/attack` - ATTACK 상태 종목

pub mod bot_handler;
pub mod discord;
pub mod email;
pub mod slack;
pub mod sms;
pub mod telegram;
pub mod types;

pub use bot_handler::*;
pub use discord::*;
pub use email::*;
pub use slack::*;
pub use sms::*;
pub use telegram::*;
pub use types::*;
