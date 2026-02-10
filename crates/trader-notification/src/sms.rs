//! SMS 알림 서비스 (Twilio).
//!
//! Twilio API를 통해 SMS 트레이딩 알림을 전송합니다.

use crate::types::{
    Notification, NotificationError, NotificationEvent, NotificationPriority, NotificationResult,
    NotificationSender,
};
use async_trait::async_trait;
use rust_decimal::Decimal;
use tracing::{debug, error, info, warn};

/// SMS 프로바이더 타입.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SmsProvider {
    /// Twilio SMS API
    #[default]
    Twilio,
}

/// SMS 알림 전송 설정.
#[derive(Debug, Clone)]
pub struct SmsConfig {
    /// SMS 프로바이더
    pub provider: SmsProvider,
    /// Twilio Account SID
    pub account_sid: String,
    /// Twilio Auth Token
    pub auth_token: String,
    /// 발신 전화번호 (E.164 형식, 예: +15551234567)
    pub from_number: String,
    /// 수신 전화번호 목록 (E.164 형식)
    pub to_numbers: Vec<String>,
    /// 전송 활성화 여부
    pub enabled: bool,
}

impl SmsConfig {
    /// 새 SMS 설정을 생성합니다 (Twilio).
    pub fn new_twilio(
        account_sid: String,
        auth_token: String,
        from_number: String,
        to_numbers: Vec<String>,
    ) -> Self {
        Self {
            provider: SmsProvider::Twilio,
            account_sid,
            auth_token,
            from_number,
            to_numbers,
            enabled: true,
        }
    }

    /// 환경 변수에서 설정을 생성합니다.
    pub fn from_env() -> Option<Self> {
        let account_sid = std::env::var("TWILIO_ACCOUNT_SID").ok()?;
        let auth_token = std::env::var("TWILIO_AUTH_TOKEN").ok()?;
        let from_number = std::env::var("TWILIO_FROM_NUMBER").ok()?;
        let to_numbers: Vec<String> = std::env::var("TWILIO_TO_NUMBERS")
            .ok()?
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        let enabled = std::env::var("SMS_ENABLED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);

        Some(Self {
            provider: SmsProvider::Twilio,
            account_sid,
            auth_token,
            from_number,
            to_numbers,
            enabled,
        })
    }
}

/// SMS 알림 전송기.
pub struct SmsSender {
    config: SmsConfig,
    client: reqwest::Client,
}

impl SmsSender {
    /// 새 SMS 전송기를 생성합니다.
    pub fn new(config: SmsConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// 환경 변수에서 전송기를 생성합니다.
    pub fn from_env() -> Option<Self> {
        SmsConfig::from_env().map(Self::new)
    }

    /// 알림을 SMS 텍스트로 포맷합니다 (160자 이내 권장).
    fn format_message(&self, notification: &Notification) -> String {
        let priority_prefix = match notification.priority {
            NotificationPriority::Low => "",
            NotificationPriority::Normal => "",
            NotificationPriority::High => "[HIGH] ",
            NotificationPriority::Critical => "[CRITICAL] ",
        };

        let content = match &notification.event {
            NotificationEvent::OrderFilled {
                symbol,
                side,
                quantity,
                price,
                ..
            } => {
                format!(
                    "주문체결: {} {} {}@{}",
                    symbol, side, quantity, price
                )
            }

            NotificationEvent::PositionClosed {
                symbol,
                pnl,
                pnl_percent,
                ..
            } => {
                let pnl_sign = if *pnl >= Decimal::ZERO { "+" } else { "" };
                format!(
                    "청산: {} PnL: {}{} ({}{}%)",
                    symbol, pnl_sign, pnl, pnl_sign, pnl_percent
                )
            }

            NotificationEvent::StopLossTriggered { symbol, loss, .. } => {
                format!("손절발동: {} -{}원", symbol, loss)
            }

            NotificationEvent::TakeProfitTriggered { symbol, profit, .. } => {
                format!("익절발동: {} +{}원", symbol, profit)
            }

            NotificationEvent::SignalAlert {
                signal_type,
                symbol,
                side,
                price,
                strength,
                strategy_name,
                ..
            } => {
                let side_text = side.as_ref().map(|s| format!(" {}", s)).unwrap_or_default();
                format!(
                    "{} 신호: {} {}{} @{} ({:.0}%) - {}",
                    signal_type, symbol, strategy_name, side_text, price, strength * 100.0, strategy_name
                )
            }

            NotificationEvent::SystemError { error_code, message } => {
                // 메시지를 80자로 제한
                let short_msg = if message.len() > 80 {
                    format!("{}...", &message[..77])
                } else {
                    message.clone()
                };
                format!("오류 {}: {}", error_code, short_msg)
            }

            NotificationEvent::RiskAlert {
                alert_type,
                current_value,
                threshold,
                ..
            } => {
                format!(
                    "리스크경고: {} 현재:{} 임계:{}",
                    alert_type, current_value, threshold
                )
            }

            NotificationEvent::RouteStateChanged {
                symbol,
                previous_state,
                new_state,
                ..
            } => {
                format!("상태변경: {} {}→{}", symbol, previous_state, new_state)
            }

            NotificationEvent::Custom { title, message } => {
                // 제목 + 메시지를 합쳐서 140자로 제한
                let combined = format!("{}: {}", title, message);
                if combined.len() > 140 {
                    format!("{}...", &combined[..137])
                } else {
                    combined
                }
            }

            NotificationEvent::DailySummary {
                date,
                total_trades,
                total_pnl,
                win_rate,
                ..
            } => {
                let pnl_sign = if *total_pnl >= Decimal::ZERO { "+" } else { "" };
                format!(
                    "일일요약({}): {}건 PnL:{}{} 승률:{}%",
                    date, total_trades, pnl_sign, total_pnl, win_rate
                )
            }

            // 나머지 이벤트들은 간단하게 처리
            _ => {
                format!("{:?}", notification.event)
                    .chars()
                    .take(140)
                    .collect()
            }
        };

        // 최종 메시지 160자 제한
        let full_message = format!("{}{}", priority_prefix, content);
        if full_message.len() > 160 {
            format!("{}...", &full_message.chars().take(157).collect::<String>())
        } else {
            full_message
        }
    }

    /// Twilio API를 통해 SMS를 전송합니다.
    async fn send_twilio(&self, message: &str) -> NotificationResult<()> {
        let url = format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json",
            self.config.account_sid
        );

        for to_number in &self.config.to_numbers {
            let params = [
                ("From", self.config.from_number.as_str()),
                ("To", to_number.as_str()),
                ("Body", message),
            ];

            debug!("Sending SMS to {}", to_number);

            let response = self
                .client
                .post(&url)
                .basic_auth(&self.config.account_sid, Some(&self.config.auth_token))
                .form(&params)
                .send()
                .await
                .map_err(NotificationError::NetworkError)?;

            if response.status().is_success() {
                debug!("SMS 전송 성공: {}", to_number);
            } else {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();

                // 요청 한도 제한 확인
                if status.as_u16() == 429 {
                    warn!("Twilio rate limited");
                    return Err(NotificationError::RateLimited(60));
                }

                error!("Twilio SMS 전송 실패 ({}): {} - {}", to_number, status, body);
                return Err(NotificationError::SendFailed(format!(
                    "HTTP {}: {}",
                    status, body
                )));
            }
        }

        info!("SMS 알림 전송 완료: {} 수신자", self.config.to_numbers.len());
        Ok(())
    }

    /// 테스트 SMS를 전송합니다.
    pub async fn send_test(&self) -> NotificationResult<()> {
        let message = "[ZeroQuant] SMS 알림 설정 완료. 트레이딩 알림을 받으실 수 있습니다.";
        self.send_twilio(message).await
    }
}

#[async_trait]
impl NotificationSender for SmsSender {
    async fn send(&self, notification: &Notification) -> NotificationResult<()> {
        if !self.is_enabled() {
            debug!("SMS 알림이 비활성화되어 있습니다");
            return Ok(());
        }

        let message = self.format_message(notification);

        match self.config.provider {
            SmsProvider::Twilio => self.send_twilio(&message).await,
        }
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
            && !self.config.account_sid.is_empty()
            && !self.config.auth_token.is_empty()
            && !self.config.from_number.is_empty()
            && !self.config.to_numbers.is_empty()
    }

    fn name(&self) -> &str {
        "sms"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sms_config_new() {
        let config = SmsConfig::new_twilio(
            "AC12345".to_string(),
            "token123".to_string(),
            "+15551234567".to_string(),
            vec!["+15559876543".to_string()],
        );

        assert_eq!(config.provider, SmsProvider::Twilio);
        assert!(config.enabled);
        assert_eq!(config.from_number, "+15551234567");
    }

    #[test]
    fn test_format_message_short() {
        let config = SmsConfig::new_twilio(
            "AC12345".to_string(),
            "token".to_string(),
            "+1555".to_string(),
            vec!["+1666".to_string()],
        );
        let sender = SmsSender::new(config);

        let notification = Notification::new(NotificationEvent::StopLossTriggered {
            symbol: "AAPL".to_string(),
            quantity: Decimal::new(10, 0),
            trigger_price: Decimal::new(15000, 2),
            loss: Decimal::new(5000, 2),
        });

        let message = sender.format_message(&notification);
        assert!(message.len() <= 160);
        assert!(message.contains("손절발동"));
        assert!(message.contains("AAPL"));
    }

    #[test]
    fn test_format_message_truncation() {
        let config = SmsConfig::new_twilio(
            "AC12345".to_string(),
            "token".to_string(),
            "+1555".to_string(),
            vec!["+1666".to_string()],
        );
        let sender = SmsSender::new(config);

        // 긴 메시지 테스트
        let long_message = "A".repeat(200);
        let notification = Notification::new(NotificationEvent::Custom {
            title: "테스트".to_string(),
            message: long_message,
        });

        let message = sender.format_message(&notification);
        assert!(message.len() <= 160);
        assert!(message.ends_with("..."));
    }

    #[test]
    fn test_critical_priority_prefix() {
        let config = SmsConfig::new_twilio(
            "AC12345".to_string(),
            "token".to_string(),
            "+1555".to_string(),
            vec!["+1666".to_string()],
        );
        let sender = SmsSender::new(config);

        let notification = Notification::new(NotificationEvent::SystemError {
            error_code: "E001".to_string(),
            message: "테스트 오류".to_string(),
        })
        .with_priority(NotificationPriority::Critical);

        let message = sender.format_message(&notification);
        assert!(message.starts_with("[CRITICAL]"));
    }
}
