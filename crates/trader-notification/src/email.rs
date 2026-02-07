//! 이메일 알림 서비스.
//!
//! SMTP를 통해 트레이딩 알림 및 업데이트를 전송합니다.

use crate::types::{
    Notification, NotificationError, NotificationEvent, NotificationPriority, NotificationResult,
    NotificationSender,
};
use async_trait::async_trait;
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use rust_decimal::Decimal;
use tracing::{debug, error, info};

/// 이메일 알림 전송 설정.
#[derive(Debug, Clone)]
pub struct EmailConfig {
    /// SMTP 서버 호스트
    pub smtp_host: String,
    /// SMTP 서버 포트
    pub smtp_port: u16,
    /// TLS 사용 여부
    pub use_tls: bool,
    /// SMTP 사용자명
    pub username: String,
    /// SMTP 비밀번호
    pub password: String,
    /// 발신자 이메일 주소
    pub from_email: String,
    /// 발신자 이름 (선택)
    pub from_name: Option<String>,
    /// 수신자 이메일 주소 목록
    pub to_emails: Vec<String>,
    /// 전송 활성화 여부
    pub enabled: bool,
}

impl EmailConfig {
    /// 새 이메일 설정을 생성합니다.
    pub fn new(
        smtp_host: String,
        smtp_port: u16,
        username: String,
        password: String,
        from_email: String,
        to_emails: Vec<String>,
    ) -> Self {
        Self {
            smtp_host,
            smtp_port,
            use_tls: true,
            username,
            password,
            from_email,
            from_name: None,
            to_emails,
            enabled: true,
        }
    }

    /// 발신자 이름을 설정합니다.
    pub fn with_from_name(mut self, name: String) -> Self {
        self.from_name = Some(name);
        self
    }

    /// 환경 변수에서 설정을 생성합니다.
    pub fn from_env() -> Option<Self> {
        let smtp_host = std::env::var("EMAIL_SMTP_HOST").ok()?;
        let smtp_port = std::env::var("EMAIL_SMTP_PORT")
            .ok()?
            .parse::<u16>()
            .ok()?;
        let username = std::env::var("EMAIL_USERNAME").ok()?;
        let password = std::env::var("EMAIL_PASSWORD").ok()?;
        let from_email = std::env::var("EMAIL_FROM").ok()?;
        let from_name = std::env::var("EMAIL_FROM_NAME").ok();
        let to_emails: Vec<String> = std::env::var("EMAIL_TO")
            .ok()?
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        let enabled = std::env::var("EMAIL_ENABLED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);

        Some(Self {
            smtp_host,
            smtp_port,
            use_tls: true,
            username,
            password,
            from_email,
            from_name,
            to_emails,
            enabled,
        })
    }
}

/// 이메일 알림 전송기.
pub struct EmailSender {
    config: EmailConfig,
}

impl EmailSender {
    /// 새 이메일 전송기를 생성합니다.
    pub fn new(config: EmailConfig) -> Self {
        Self { config }
    }

    /// 환경 변수에서 전송기를 생성합니다.
    pub fn from_env() -> Option<Self> {
        EmailConfig::from_env().map(Self::new)
    }

    /// 알림을 이메일 제목으로 변환합니다.
    fn format_subject(&self, notification: &Notification) -> String {
        let priority_prefix = match notification.priority {
            NotificationPriority::Low => "[INFO]",
            NotificationPriority::Normal => "[ALERT]",
            NotificationPriority::High => "[HIGH]",
            NotificationPriority::Critical => "[CRITICAL]",
        };

        match &notification.event {
            NotificationEvent::OrderFilled { symbol, side, .. } => {
                format!("{} 주문 체결: {} {}", priority_prefix, symbol, side)
            }
            NotificationEvent::PositionOpened { symbol, side, .. } => {
                format!("{} 포지션 진입: {} {}", priority_prefix, symbol, side)
            }
            NotificationEvent::PositionClosed { symbol, pnl, .. } => {
                let pnl_sign = if *pnl >= Decimal::ZERO { "+" } else { "" };
                format!("{} 포지션 청산: {} ({}{})", priority_prefix, symbol, pnl_sign, pnl)
            }
            NotificationEvent::StopLossTriggered { symbol, .. } => {
                format!("{} 손절 발동: {}", priority_prefix, symbol)
            }
            NotificationEvent::TakeProfitTriggered { symbol, .. } => {
                format!("{} 익절 발동: {}", priority_prefix, symbol)
            }
            NotificationEvent::DailySummary { date, .. } => {
                format!("{} 일일 요약: {}", priority_prefix, date)
            }
            NotificationEvent::RiskAlert { alert_type, .. } => {
                format!("{} 리스크 경고: {}", priority_prefix, alert_type)
            }
            NotificationEvent::StrategyStarted { strategy_name, .. } => {
                format!("{} 전략 시작: {}", priority_prefix, strategy_name)
            }
            NotificationEvent::StrategyStopped { strategy_name, .. } => {
                format!("{} 전략 중지: {}", priority_prefix, strategy_name)
            }
            NotificationEvent::SystemError { error_code, .. } => {
                format!("{} 시스템 오류: {}", priority_prefix, error_code)
            }
            NotificationEvent::SignalAlert { symbol, signal_type, .. } => {
                format!("{} {} 신호: {}", priority_prefix, signal_type, symbol)
            }
            NotificationEvent::Custom { title, .. } => {
                format!("{} {}", priority_prefix, title)
            }
            NotificationEvent::RouteStateChanged { symbol, new_state, .. } => {
                format!("{} 상태 변경: {} → {}", priority_prefix, symbol, new_state)
            }
            NotificationEvent::MacroAlert { risk_level, .. } => {
                format!("{} 매크로 경고: {}", priority_prefix, risk_level)
            }
            NotificationEvent::MarketBreadthAlert { temperature, .. } => {
                format!("{} 시장 온도: {}", priority_prefix, temperature)
            }
        }
    }

    /// 알림을 HTML 메시지로 포맷합니다.
    fn format_html_body(&self, notification: &Notification) -> String {
        let priority_color = match notification.priority {
            NotificationPriority::Low => "#6c757d",
            NotificationPriority::Normal => "#007bff",
            NotificationPriority::High => "#fd7e14",
            NotificationPriority::Critical => "#dc3545",
        };

        let content = match &notification.event {
            NotificationEvent::OrderFilled {
                symbol,
                side,
                quantity,
                price,
                order_id,
            } => {
                let side_color = if side.to_lowercase() == "buy" { "#28a745" } else { "#dc3545" };
                format!(
                    r#"<h2 style="color: {};">주문 체결</h2>
                    <table style="border-collapse: collapse; width: 100%;">
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>심볼</strong></td><td style="padding: 8px; border: 1px solid #ddd;"><code>{}</code></td></tr>
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>방향</strong></td><td style="padding: 8px; border: 1px solid #ddd; color: {};">{}</td></tr>
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>수량</strong></td><td style="padding: 8px; border: 1px solid #ddd;">{}</td></tr>
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>가격</strong></td><td style="padding: 8px; border: 1px solid #ddd;">{}</td></tr>
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>주문ID</strong></td><td style="padding: 8px; border: 1px solid #ddd;"><code>{}</code></td></tr>
                    </table>"#,
                    side_color, symbol, side_color, side, quantity, price, order_id
                )
            }

            NotificationEvent::PositionClosed {
                symbol,
                side,
                quantity,
                entry_price,
                exit_price,
                pnl,
                pnl_percent,
            } => {
                let pnl_color = if *pnl >= Decimal::ZERO { "#28a745" } else { "#dc3545" };
                let pnl_sign = if *pnl >= Decimal::ZERO { "+" } else { "" };
                format!(
                    r#"<h2>포지션 청산</h2>
                    <table style="border-collapse: collapse; width: 100%;">
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>심볼</strong></td><td style="padding: 8px; border: 1px solid #ddd;"><code>{}</code></td></tr>
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>방향</strong></td><td style="padding: 8px; border: 1px solid #ddd;">{}</td></tr>
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>수량</strong></td><td style="padding: 8px; border: 1px solid #ddd;">{}</td></tr>
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>진입가</strong></td><td style="padding: 8px; border: 1px solid #ddd;">{}</td></tr>
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>청산가</strong></td><td style="padding: 8px; border: 1px solid #ddd;">{}</td></tr>
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>손익</strong></td><td style="padding: 8px; border: 1px solid #ddd; color: {};"><strong>{}{}</strong> ({}{}%)</td></tr>
                    </table>"#,
                    symbol, side, quantity, entry_price, exit_price, pnl_color, pnl_sign, pnl, pnl_sign, pnl_percent
                )
            }

            NotificationEvent::SignalAlert {
                signal_type,
                symbol,
                side,
                price,
                strength,
                reason,
                strategy_name,
                ..
            } => {
                let signal_color = match signal_type.as_str() {
                    "ENTRY" | "Entry" => "#28a745",
                    "EXIT" | "Exit" => "#dc3545",
                    _ => "#ffc107",
                };
                let side_text = side.as_ref().map(|s| format!("<tr><td style=\"padding: 8px; border: 1px solid #ddd;\"><strong>방향</strong></td><td style=\"padding: 8px; border: 1px solid #ddd;\">{}</td></tr>", s)).unwrap_or_default();

                format!(
                    r#"<h2 style="color: {};">{} 신호</h2>
                    <table style="border-collapse: collapse; width: 100%;">
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>전략</strong></td><td style="padding: 8px; border: 1px solid #ddd;">{}</td></tr>
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>심볼</strong></td><td style="padding: 8px; border: 1px solid #ddd;"><code>{}</code></td></tr>
                        {}
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>가격</strong></td><td style="padding: 8px; border: 1px solid #ddd;">{}</td></tr>
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>강도</strong></td><td style="padding: 8px; border: 1px solid #ddd;">{:.0}%</td></tr>
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>이유</strong></td><td style="padding: 8px; border: 1px solid #ddd;">{}</td></tr>
                    </table>"#,
                    signal_color, signal_type, strategy_name, symbol, side_text, price, strength * 100.0, reason
                )
            }

            NotificationEvent::SystemError { error_code, message } => {
                format!(
                    r#"<h2 style="color: #dc3545;">시스템 오류</h2>
                    <table style="border-collapse: collapse; width: 100%;">
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>오류 코드</strong></td><td style="padding: 8px; border: 1px solid #ddd;"><code>{}</code></td></tr>
                        <tr><td style="padding: 8px; border: 1px solid #ddd;"><strong>메시지</strong></td><td style="padding: 8px; border: 1px solid #ddd;">{}</td></tr>
                    </table>"#,
                    error_code, message
                )
            }

            NotificationEvent::Custom { title, message } => {
                format!(
                    r#"<h2>{}</h2>
                    <p>{}</p>"#,
                    title, message
                )
            }

            // 간단한 형식으로 나머지 이벤트 처리
            _ => format!("<p>{:?}</p>", notification.event),
        };

        let timestamp = notification.timestamp.format("%Y-%m-%d %H:%M:%S UTC");

        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; padding: 20px; background-color: #f5f5f5; }}
        .container {{ max-width: 600px; margin: 0 auto; background-color: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
        .priority-badge {{ display: inline-block; padding: 4px 8px; border-radius: 4px; color: white; background-color: {}; font-size: 12px; margin-bottom: 16px; }}
        .footer {{ margin-top: 20px; padding-top: 16px; border-top: 1px solid #eee; color: #666; font-size: 12px; }}
    </style>
</head>
<body>
    <div class="container">
        <span class="priority-badge">{:?}</span>
        {}
        <div class="footer">
            <p>ZeroQuant Trading Bot</p>
            <p>{}</p>
        </div>
    </div>
</body>
</html>"#,
            priority_color,
            notification.priority,
            content,
            timestamp
        )
    }

    /// 이메일을 전송합니다.
    async fn send_email(&self, subject: &str, html_body: &str) -> NotificationResult<()> {
        // 발신자 Mailbox 생성
        let from_mailbox: Mailbox = if let Some(ref name) = self.config.from_name {
            format!("{} <{}>", name, self.config.from_email)
                .parse()
                .map_err(|e| NotificationError::InvalidConfig(format!("잘못된 발신자 주소: {}", e)))?
        } else {
            self.config
                .from_email
                .parse()
                .map_err(|e| NotificationError::InvalidConfig(format!("잘못된 발신자 주소: {}", e)))?
        };

        // 각 수신자에게 이메일 전송
        for to_email in &self.config.to_emails {
            let to_mailbox: Mailbox = to_email
                .parse()
                .map_err(|e| NotificationError::InvalidConfig(format!("잘못된 수신자 주소: {}", e)))?;

            let email = Message::builder()
                .from(from_mailbox.clone())
                .to(to_mailbox)
                .subject(subject)
                .header(ContentType::TEXT_HTML)
                .body(html_body.to_string())
                .map_err(|e| NotificationError::SendFailed(format!("이메일 생성 실패: {}", e)))?;

            // SMTP 전송기 생성
            let creds = Credentials::new(
                self.config.username.clone(),
                self.config.password.clone(),
            );

            let mailer: AsyncSmtpTransport<Tokio1Executor> = if self.config.use_tls {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.smtp_host)
                    .map_err(|e| NotificationError::SendFailed(format!("SMTP 연결 실패: {}", e)))?
                    .port(self.config.smtp_port)
                    .credentials(creds)
                    .build()
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.config.smtp_host)
                    .port(self.config.smtp_port)
                    .credentials(creds)
                    .build()
            };

            // 이메일 전송
            match mailer.send(email).await {
                Ok(_) => {
                    debug!("이메일 전송 성공: {}", to_email);
                }
                Err(e) => {
                    error!("이메일 전송 실패 ({}): {}", to_email, e);
                    return Err(NotificationError::SendFailed(format!(
                        "이메일 전송 실패: {}",
                        e
                    )));
                }
            }
        }

        info!("이메일 알림 전송 완료: {} 수신자", self.config.to_emails.len());
        Ok(())
    }

    /// 테스트 이메일을 전송합니다.
    pub async fn send_test(&self) -> NotificationResult<()> {
        let subject = "[ZeroQuant] 이메일 알림 테스트";
        let html_body = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; padding: 20px;">
    <div style="max-width: 600px; margin: 0 auto; background-color: #f8f9fa; padding: 20px; border-radius: 8px;">
        <h2 style="color: #28a745;">✓ 이메일 알림 설정 완료</h2>
        <p>ZeroQuant 트레이딩 봇의 이메일 알림이 정상적으로 설정되었습니다.</p>
        <p>이제 트레이딩 알림을 이 이메일로 받으실 수 있습니다.</p>
    </div>
</body>
</html>"#;

        self.send_email(subject, html_body).await
    }
}

#[async_trait]
impl NotificationSender for EmailSender {
    async fn send(&self, notification: &Notification) -> NotificationResult<()> {
        if !self.is_enabled() {
            debug!("이메일 알림이 비활성화되어 있습니다");
            return Ok(());
        }

        let subject = self.format_subject(notification);
        let html_body = self.format_html_body(notification);
        self.send_email(&subject, &html_body).await
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
            && !self.config.smtp_host.is_empty()
            && !self.config.username.is_empty()
            && !self.config.to_emails.is_empty()
    }

    fn name(&self) -> &str {
        "email"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_config_new() {
        let config = EmailConfig::new(
            "smtp.gmail.com".to_string(),
            587,
            "user@gmail.com".to_string(),
            "password".to_string(),
            "from@gmail.com".to_string(),
            vec!["to@example.com".to_string()],
        );

        assert_eq!(config.smtp_host, "smtp.gmail.com");
        assert_eq!(config.smtp_port, 587);
        assert!(config.use_tls);
        assert!(config.enabled);
    }

    #[test]
    fn test_format_subject() {
        let config = EmailConfig::new(
            "smtp.test.com".to_string(),
            587,
            "user".to_string(),
            "pass".to_string(),
            "from@test.com".to_string(),
            vec!["to@test.com".to_string()],
        );
        let sender = EmailSender::new(config);

        let notification = Notification::new(NotificationEvent::SystemError {
            error_code: "E001".to_string(),
            message: "테스트 오류".to_string(),
        })
        .with_priority(NotificationPriority::Critical);

        let subject = sender.format_subject(&notification);
        assert!(subject.contains("[CRITICAL]"));
        assert!(subject.contains("시스템 오류"));
        assert!(subject.contains("E001"));
    }
}
