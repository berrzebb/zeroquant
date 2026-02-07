//! 텔레그램 봇 명령어 핸들러.
//!
//! 사용자로부터 명령어를 수신하고 처리합니다.
//! - `/portfolio` - 포트폴리오 현황 조회
//! - `/status` - 시스템 상태 조회
//! - `/stop` - 전략 중지
//! - `/report` - 일일/주간 리포트
//! - `/attack` - ATTACK 상태 종목 조회

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::telegram::TelegramConfig;
use crate::types::{NotificationError, NotificationResult};

/// 텔레그램 봇 업데이트 응답.
#[derive(Debug, Deserialize)]
struct TelegramUpdates {
    ok: bool,
    result: Vec<TelegramUpdate>,
}

/// 개별 업데이트.
#[derive(Debug, Deserialize)]
struct TelegramUpdate {
    update_id: i64,
    message: Option<TelegramMessage>,
}

/// 메시지 정보.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TelegramMessage {
    message_id: i64,
    from: Option<TelegramUser>,
    chat: TelegramChat,
    text: Option<String>,
    date: i64,
}

/// 사용자 정보.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TelegramUser {
    id: i64,
    username: Option<String>,
}

/// 채팅 정보.
#[derive(Debug, Deserialize)]
struct TelegramChat {
    id: i64,
}

/// 봇 명령어 타입.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BotCommand {
    /// 포트폴리오 현황
    Portfolio,
    /// 시스템 상태
    Status,
    /// 전략 중지
    Stop { strategy_id: Option<String> },
    /// 리포트 조회
    Report { period: ReportPeriod },
    /// ATTACK 상태 종목
    Attack,
    /// 도움말
    Help,
    /// 알 수 없는 명령어
    Unknown(String),
}

/// 리포트 기간.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ReportPeriod {
    #[default]
    Daily,
    Weekly,
    Monthly,
}

impl BotCommand {
    /// 텍스트에서 명령어 파싱.
    pub fn parse(text: &str) -> Self {
        let text = text.trim();

        // /명령어 형식 확인
        if !text.starts_with('/') {
            return BotCommand::Unknown(text.to_string());
        }

        let parts: Vec<&str> = text[1..].split_whitespace().collect();
        let command = parts.first().map(|s| s.to_lowercase());

        match command.as_deref() {
            Some("portfolio") | Some("p") => BotCommand::Portfolio,
            Some("status") | Some("s") => BotCommand::Status,
            Some("stop") => {
                let strategy_id = parts.get(1).map(|s| s.to_string());
                BotCommand::Stop { strategy_id }
            }
            Some("report") | Some("r") => {
                let period = match parts.get(1).map(|s| s.to_lowercase()).as_deref() {
                    Some("weekly") | Some("w") => ReportPeriod::Weekly,
                    Some("monthly") | Some("m") => ReportPeriod::Monthly,
                    _ => ReportPeriod::Daily,
                };
                BotCommand::Report { period }
            }
            Some("attack") | Some("a") => BotCommand::Attack,
            Some("help") | Some("h") | Some("start") => BotCommand::Help,
            _ => BotCommand::Unknown(text.to_string()),
        }
    }
}

/// 명령어 응답 데이터.
pub struct CommandResponse {
    /// 응답 텍스트 (HTML 형식)
    pub text: String,
    /// 파싱 모드
    pub parse_mode: String,
}

impl CommandResponse {
    /// HTML 형식 응답 생성.
    pub fn html(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            parse_mode: "HTML".to_string(),
        }
    }
}

/// 봇 명령어 핸들러 trait.
///
/// 각 명령어의 실제 로직을 구현합니다.
#[async_trait]
pub trait BotCommandHandler: Send + Sync {
    /// 포트폴리오 현황 조회.
    async fn handle_portfolio(&self) -> NotificationResult<CommandResponse>;

    /// 시스템 상태 조회.
    async fn handle_status(&self) -> NotificationResult<CommandResponse>;

    /// 전략 중지.
    async fn handle_stop(&self, strategy_id: Option<&str>) -> NotificationResult<CommandResponse>;

    /// 리포트 조회.
    async fn handle_report(&self, period: ReportPeriod) -> NotificationResult<CommandResponse>;

    /// ATTACK 상태 종목 조회.
    async fn handle_attack(&self) -> NotificationResult<CommandResponse>;
}

/// 텔레그램 봇 핸들러.
///
/// Long polling으로 업데이트를 수신하고 명령어를 처리합니다.
pub struct TelegramBotHandler<H: BotCommandHandler> {
    config: TelegramConfig,
    client: reqwest::Client,
    handler: Arc<H>,
    last_update_id: RwLock<i64>,
    /// 허용된 채팅 ID 목록 (보안을 위해)
    allowed_chat_ids: Vec<i64>,
}

impl<H: BotCommandHandler> TelegramBotHandler<H> {
    /// 새 봇 핸들러 생성.
    pub fn new(config: TelegramConfig, handler: Arc<H>) -> Self {
        // 설정된 chat_id를 허용 목록에 추가
        let chat_id: i64 = config.chat_id.parse().unwrap_or(0);

        Self {
            config,
            client: reqwest::Client::new(),
            handler,
            last_update_id: RwLock::new(0),
            allowed_chat_ids: vec![chat_id],
        }
    }

    /// 추가 허용 채팅 ID 설정.
    pub fn with_allowed_chat_ids(mut self, chat_ids: Vec<i64>) -> Self {
        self.allowed_chat_ids.extend(chat_ids);
        self
    }

    /// 봇 폴링 시작.
    ///
    /// 무한 루프로 업데이트를 수신합니다.
    pub async fn start_polling(&self) {
        info!("텔레그램 봇 폴링 시작");

        loop {
            match self.poll_updates().await {
                Ok(updates) => {
                    for update in updates {
                        if let Err(e) = self.process_update(update).await {
                            error!("업데이트 처리 실패: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("업데이트 폴링 실패: {}", e);
                    // 에러 발생 시 잠시 대기
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// 업데이트 폴링.
    async fn poll_updates(&self) -> NotificationResult<Vec<TelegramUpdate>> {
        let last_id = *self.last_update_id.read().await;

        let url = format!(
            "https://api.telegram.org/bot{}/getUpdates",
            self.config.bot_token
        );

        let params = serde_json::json!({
            "offset": last_id + 1,
            "timeout": 30,
            "allowed_updates": ["message"],
        });

        let response = self
            .client
            .post(&url)
            .json(&params)
            .timeout(Duration::from_secs(35))
            .send()
            .await
            .map_err(NotificationError::NetworkError)?;

        let updates: TelegramUpdates = response
            .json()
            .await
            .map_err(|e| NotificationError::SendFailed(e.to_string()))?;

        if !updates.ok {
            return Err(NotificationError::SendFailed(
                "텔레그램 API 응답 실패".to_string(),
            ));
        }

        // 마지막 업데이트 ID 갱신
        if let Some(last) = updates.result.last() {
            *self.last_update_id.write().await = last.update_id;
        }

        Ok(updates.result)
    }

    /// 개별 업데이트 처리.
    async fn process_update(&self, update: TelegramUpdate) -> NotificationResult<()> {
        let Some(message) = update.message else {
            return Ok(());
        };

        let chat_id = message.chat.id;

        // 허용된 채팅 ID 확인
        if !self.allowed_chat_ids.contains(&chat_id) {
            warn!(chat_id = chat_id, "허용되지 않은 채팅 ID에서 메시지 수신");
            return Ok(());
        }

        let Some(text) = message.text else {
            return Ok(());
        };

        debug!(
            chat_id = chat_id,
            text = %text,
            "명령어 수신"
        );

        // 명령어 파싱 및 처리
        let command = BotCommand::parse(&text);
        let response = self.execute_command(command).await?;

        // 응답 전송
        self.send_response(chat_id, &response).await
    }

    /// 명령어 실행.
    async fn execute_command(&self, command: BotCommand) -> NotificationResult<CommandResponse> {
        match command {
            BotCommand::Portfolio => self.handler.handle_portfolio().await,
            BotCommand::Status => self.handler.handle_status().await,
            BotCommand::Stop { strategy_id } => {
                self.handler.handle_stop(strategy_id.as_deref()).await
            }
            BotCommand::Report { period } => self.handler.handle_report(period).await,
            BotCommand::Attack => self.handler.handle_attack().await,
            BotCommand::Help => Ok(self.help_message()),
            BotCommand::Unknown(text) => Ok(CommandResponse::html(format!(
                "❓ <b>알 수 없는 명령어</b>\n\n\
                 입력: <code>{}</code>\n\n\
                 /help 명령어로 사용 가능한 명령어를 확인하세요.",
                text
            ))),
        }
    }

    /// 도움말 메시지 생성.
    fn help_message(&self) -> CommandResponse {
        CommandResponse::html(
            "🤖 <b>ZeroQuant 트레이딩 봇</b>\n\n\
             <b>사용 가능한 명령어:</b>\n\n\
             /portfolio (p) - 📊 포트폴리오 현황\n\
             /status (s) - 🔍 시스템 상태\n\
             /stop [전략ID] - ⏹️ 전략 중지\n\
             /report [daily|weekly|monthly] - 📈 리포트\n\
             /attack (a) - 🎯 ATTACK 상태 종목\n\
             /help (h) - ❓ 도움말\n\n\
             <i>예시: /report weekly</i>",
        )
    }

    /// 응답 메시지 전송.
    async fn send_response(
        &self,
        chat_id: i64,
        response: &CommandResponse,
    ) -> NotificationResult<()> {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.config.bot_token
        );

        let params = serde_json::json!({
            "chat_id": chat_id,
            "text": response.text,
            "parse_mode": response.parse_mode,
            "disable_web_page_preview": true,
        });

        let api_response = self
            .client
            .post(&url)
            .json(&params)
            .send()
            .await
            .map_err(NotificationError::NetworkError)?;

        if api_response.status().is_success() {
            debug!(chat_id = chat_id, "응답 전송 완료");
            Ok(())
        } else {
            let status = api_response.status();
            let body = api_response.text().await.unwrap_or_default();
            error!("응답 전송 실패: {} - {}", status, body);
            Err(NotificationError::SendFailed(format!(
                "HTTP {}: {}",
                status, body
            )))
        }
    }
}

/// 기본 봇 명령어 핸들러 (스텁 구현).
///
/// 실제 데이터 접근이 필요한 경우 이 trait을 구현하여 사용합니다.
pub struct DefaultBotHandler;

#[async_trait]
impl BotCommandHandler for DefaultBotHandler {
    async fn handle_portfolio(&self) -> NotificationResult<CommandResponse> {
        Ok(CommandResponse::html(
            "📊 <b>포트폴리오 현황</b>\n\n\
             <i>데이터 연동이 필요합니다.</i>",
        ))
    }

    async fn handle_status(&self) -> NotificationResult<CommandResponse> {
        Ok(CommandResponse::html(
            "🔍 <b>시스템 상태</b>\n\n\
             ✅ 봇 동작 중\n\
             <i>상세 상태는 데이터 연동 후 확인 가능합니다.</i>",
        ))
    }

    async fn handle_stop(&self, strategy_id: Option<&str>) -> NotificationResult<CommandResponse> {
        match strategy_id {
            Some(id) => Ok(CommandResponse::html(format!(
                "⏹️ <b>전략 중지 요청</b>\n\n\
                 전략 ID: <code>{}</code>\n\
                 <i>실제 중지 기능은 API 연동 후 가능합니다.</i>",
                id
            ))),
            None => Ok(CommandResponse::html(
                "⏹️ <b>전략 중지</b>\n\n\
                 사용법: /stop [전략ID]\n\
                 예시: /stop rsi_mean_reversion",
            )),
        }
    }

    async fn handle_report(&self, period: ReportPeriod) -> NotificationResult<CommandResponse> {
        let period_name = match period {
            ReportPeriod::Daily => "일일",
            ReportPeriod::Weekly => "주간",
            ReportPeriod::Monthly => "월간",
        };

        Ok(CommandResponse::html(format!(
            "📈 <b>{} 리포트</b>\n\n\
             <i>리포트 데이터 연동이 필요합니다.</i>",
            period_name
        )))
    }

    async fn handle_attack(&self) -> NotificationResult<CommandResponse> {
        Ok(CommandResponse::html(
            "🎯 <b>ATTACK 상태 종목</b>\n\n\
             <i>스크리닝 데이터 연동이 필요합니다.</i>",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_portfolio_command() {
        assert_eq!(BotCommand::parse("/portfolio"), BotCommand::Portfolio);
        assert_eq!(BotCommand::parse("/p"), BotCommand::Portfolio);
        assert_eq!(BotCommand::parse("  /portfolio  "), BotCommand::Portfolio);
    }

    #[test]
    fn test_parse_status_command() {
        assert_eq!(BotCommand::parse("/status"), BotCommand::Status);
        assert_eq!(BotCommand::parse("/s"), BotCommand::Status);
    }

    #[test]
    fn test_parse_stop_command() {
        assert_eq!(
            BotCommand::parse("/stop"),
            BotCommand::Stop { strategy_id: None }
        );
        assert_eq!(
            BotCommand::parse("/stop rsi_strategy"),
            BotCommand::Stop {
                strategy_id: Some("rsi_strategy".to_string())
            }
        );
    }

    #[test]
    fn test_parse_report_command() {
        assert_eq!(
            BotCommand::parse("/report"),
            BotCommand::Report {
                period: ReportPeriod::Daily
            }
        );
        assert_eq!(
            BotCommand::parse("/report weekly"),
            BotCommand::Report {
                period: ReportPeriod::Weekly
            }
        );
        assert_eq!(
            BotCommand::parse("/r m"),
            BotCommand::Report {
                period: ReportPeriod::Monthly
            }
        );
    }

    #[test]
    fn test_parse_attack_command() {
        assert_eq!(BotCommand::parse("/attack"), BotCommand::Attack);
        assert_eq!(BotCommand::parse("/a"), BotCommand::Attack);
    }

    #[test]
    fn test_parse_help_command() {
        assert_eq!(BotCommand::parse("/help"), BotCommand::Help);
        assert_eq!(BotCommand::parse("/start"), BotCommand::Help);
    }

    #[test]
    fn test_parse_unknown_command() {
        assert!(matches!(
            BotCommand::parse("/unknown"),
            BotCommand::Unknown(_)
        ));
        assert!(matches!(
            BotCommand::parse("not a command"),
            BotCommand::Unknown(_)
        ));
    }
}
