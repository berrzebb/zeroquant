//! Slack 알림 서비스.
//!
//! Slack Incoming Webhook을 통해 트레이딩 알림 및 업데이트를 전송합니다.

use crate::types::{
    Notification, NotificationError, NotificationEvent, NotificationPriority, NotificationResult,
    NotificationSender,
};
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::json;
use tracing::{debug, error, info, warn};

/// Slack 알림 전송 설정.
#[derive(Debug, Clone)]
pub struct SlackConfig {
    /// Slack Incoming Webhook URL
    pub webhook_url: String,
    /// 표시 이름 (메타데이터용)
    pub display_name: Option<String>,
    /// 워크스페이스 이름 (메타데이터용)
    pub workspace_name: Option<String>,
    /// 채널 이름 (메타데이터용)
    pub channel_name: Option<String>,
    /// 전송 활성화 여부
    pub enabled: bool,
}

impl SlackConfig {
    /// 새 Slack 설정을 생성합니다.
    pub fn new(webhook_url: String) -> Self {
        Self {
            webhook_url,
            display_name: None,
            workspace_name: None,
            channel_name: None,
            enabled: true,
        }
    }

    /// 표시 이름을 설정합니다.
    pub fn with_display_name(mut self, name: String) -> Self {
        self.display_name = Some(name);
        self
    }

    /// 환경 변수에서 설정을 생성합니다.
    pub fn from_env() -> Option<Self> {
        let webhook_url = std::env::var("SLACK_WEBHOOK_URL").ok()?;
        let display_name = std::env::var("SLACK_DISPLAY_NAME").ok();
        let enabled = std::env::var("SLACK_ENABLED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);

        Some(Self {
            webhook_url,
            display_name,
            workspace_name: None,
            channel_name: None,
            enabled,
        })
    }
}

/// Slack 알림 전송기.
pub struct SlackSender {
    config: SlackConfig,
    client: reqwest::Client,
}

impl SlackSender {
    /// 새 Slack 전송기를 생성합니다.
    pub fn new(config: SlackConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// 환경 변수에서 전송기를 생성합니다.
    pub fn from_env() -> Option<Self> {
        SlackConfig::from_env().map(Self::new)
    }

    /// 우선순위에 따른 이모지를 반환합니다.
    fn get_priority_emoji(&self, priority: &NotificationPriority) -> &'static str {
        match priority {
            NotificationPriority::Low => "ℹ️",
            NotificationPriority::Normal => "📊",
            NotificationPriority::High => "⚠️",
            NotificationPriority::Critical => "🚨",
        }
    }

    /// 알림을 Slack Block Kit 형식으로 포맷합니다.
    fn format_blocks(&self, notification: &Notification) -> serde_json::Value {
        let timestamp = notification.timestamp.format("%Y-%m-%d %H:%M:%S UTC");
        let priority_emoji = self.get_priority_emoji(&notification.priority);

        match &notification.event {
            NotificationEvent::OrderFilled {
                symbol,
                side,
                quantity,
                price,
                order_id,
            } => {
                let side_emoji = if side.to_lowercase() == "buy" { "🟢" } else { "🔴" };
                json!({
                    "blocks": [
                        {
                            "type": "header",
                            "text": { "type": "plain_text", "text": format!("{} 주문 체결", side_emoji), "emoji": true }
                        },
                        {
                            "type": "section",
                            "fields": [
                                { "type": "mrkdwn", "text": format!("*심볼*\n`{}`", symbol) },
                                { "type": "mrkdwn", "text": format!("*방향*\n{}", side) },
                                { "type": "mrkdwn", "text": format!("*수량*\n{}", quantity) },
                                { "type": "mrkdwn", "text": format!("*가격*\n{}", price) }
                            ]
                        },
                        {
                            "type": "context",
                            "elements": [
                                { "type": "mrkdwn", "text": format!("주문ID: `{}` | {}", order_id, timestamp) }
                            ]
                        }
                    ]
                })
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
                let pnl_emoji = if *pnl >= Decimal::ZERO { "💰" } else { "📉" };
                let pnl_sign = if *pnl >= Decimal::ZERO { "+" } else { "" };
                json!({
                    "blocks": [
                        {
                            "type": "header",
                            "text": { "type": "plain_text", "text": format!("{} 포지션 청산", pnl_emoji), "emoji": true }
                        },
                        {
                            "type": "section",
                            "fields": [
                                { "type": "mrkdwn", "text": format!("*심볼*\n`{}`", symbol) },
                                { "type": "mrkdwn", "text": format!("*방향*\n{}", side) },
                                { "type": "mrkdwn", "text": format!("*수량*\n{}", quantity) },
                                { "type": "mrkdwn", "text": format!("*진입가*\n{}", entry_price) },
                                { "type": "mrkdwn", "text": format!("*청산가*\n{}", exit_price) },
                                { "type": "mrkdwn", "text": format!("*손익*\n*{}{}* ({}{}%)", pnl_sign, pnl, pnl_sign, pnl_percent) }
                            ]
                        },
                        {
                            "type": "context",
                            "elements": [
                                { "type": "mrkdwn", "text": timestamp.to_string() }
                            ]
                        }
                    ]
                })
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
                let signal_emoji = match signal_type.as_str() {
                    "ENTRY" | "Entry" => "🟢",
                    "EXIT" | "Exit" => "🔴",
                    "ALERT" | "Alert" => "🔔",
                    _ => "📍",
                };

                // 진행률 바 생성
                let filled = (strength * 10.0) as usize;
                let strength_bar = format!(
                    "{}{}",
                    "█".repeat(filled),
                    "░".repeat(10 - filled)
                );

                let mut fields = vec![
                    json!({ "type": "mrkdwn", "text": format!("*전략*\n{}", strategy_name) }),
                    json!({ "type": "mrkdwn", "text": format!("*심볼*\n`{}`", symbol) }),
                ];

                if let Some(s) = side {
                    fields.push(json!({ "type": "mrkdwn", "text": format!("*방향*\n{}", s) }));
                }

                fields.extend([
                    json!({ "type": "mrkdwn", "text": format!("*가격*\n{}", price) }),
                    json!({ "type": "mrkdwn", "text": format!("*신호 강도*\n{} {:.0}%", strength_bar, strength * 100.0) }),
                ]);

                json!({
                    "blocks": [
                        {
                            "type": "header",
                            "text": { "type": "plain_text", "text": format!("{} {} 신호", signal_emoji, signal_type), "emoji": true }
                        },
                        {
                            "type": "section",
                            "fields": fields
                        },
                        {
                            "type": "section",
                            "text": { "type": "mrkdwn", "text": format!("*이유:* {}", reason) }
                        },
                        {
                            "type": "context",
                            "elements": [
                                { "type": "mrkdwn", "text": timestamp.to_string() }
                            ]
                        }
                    ]
                })
            }

            NotificationEvent::StopLossTriggered {
                symbol,
                quantity,
                trigger_price,
                loss,
            } => {
                json!({
                    "blocks": [
                        {
                            "type": "header",
                            "text": { "type": "plain_text", "text": "🛑 손절 발동", "emoji": true }
                        },
                        {
                            "type": "section",
                            "fields": [
                                { "type": "mrkdwn", "text": format!("*심볼*\n`{}`", symbol) },
                                { "type": "mrkdwn", "text": format!("*수량*\n{}", quantity) },
                                { "type": "mrkdwn", "text": format!("*발동가*\n{}", trigger_price) },
                                { "type": "mrkdwn", "text": format!("*손실*\n*-{}*", loss) }
                            ]
                        },
                        {
                            "type": "context",
                            "elements": [
                                { "type": "mrkdwn", "text": timestamp.to_string() }
                            ]
                        }
                    ]
                })
            }

            NotificationEvent::TakeProfitTriggered {
                symbol,
                quantity,
                trigger_price,
                profit,
            } => {
                json!({
                    "blocks": [
                        {
                            "type": "header",
                            "text": { "type": "plain_text", "text": "🎯 익절 발동", "emoji": true }
                        },
                        {
                            "type": "section",
                            "fields": [
                                { "type": "mrkdwn", "text": format!("*심볼*\n`{}`", symbol) },
                                { "type": "mrkdwn", "text": format!("*수량*\n{}", quantity) },
                                { "type": "mrkdwn", "text": format!("*발동가*\n{}", trigger_price) },
                                { "type": "mrkdwn", "text": format!("*수익*\n*+{}*", profit) }
                            ]
                        },
                        {
                            "type": "context",
                            "elements": [
                                { "type": "mrkdwn", "text": timestamp.to_string() }
                            ]
                        }
                    ]
                })
            }

            NotificationEvent::SystemError { error_code, message } => {
                json!({
                    "blocks": [
                        {
                            "type": "header",
                            "text": { "type": "plain_text", "text": "🚨 시스템 오류", "emoji": true }
                        },
                        {
                            "type": "section",
                            "fields": [
                                { "type": "mrkdwn", "text": format!("*오류 코드*\n`{}`", error_code) }
                            ]
                        },
                        {
                            "type": "section",
                            "text": { "type": "mrkdwn", "text": format!("*메시지:* {}", message) }
                        },
                        {
                            "type": "context",
                            "elements": [
                                { "type": "mrkdwn", "text": timestamp.to_string() }
                            ]
                        }
                    ]
                })
            }

            NotificationEvent::RiskAlert {
                alert_type,
                message,
                current_value,
                threshold,
            } => {
                json!({
                    "blocks": [
                        {
                            "type": "header",
                            "text": { "type": "plain_text", "text": "⚠️ 리스크 경고", "emoji": true }
                        },
                        {
                            "type": "section",
                            "fields": [
                                { "type": "mrkdwn", "text": format!("*유형*\n{}", alert_type) },
                                { "type": "mrkdwn", "text": format!("*현재값*\n{}", current_value) },
                                { "type": "mrkdwn", "text": format!("*임계값*\n{}", threshold) }
                            ]
                        },
                        {
                            "type": "section",
                            "text": { "type": "mrkdwn", "text": message.clone() }
                        },
                        {
                            "type": "context",
                            "elements": [
                                { "type": "mrkdwn", "text": timestamp.to_string() }
                            ]
                        }
                    ]
                })
            }

            NotificationEvent::RouteStateChanged {
                symbol,
                symbol_name,
                previous_state,
                new_state,
                macro_risk,
                macro_summary,
            } => {
                let state_emoji = match new_state.to_uppercase().as_str() {
                    "ATTACK" => "🚀",
                    "ARMED" => "⚡",
                    "WAIT" => "👀",
                    "OVERHEAT" => "🔥",
                    _ => "😐",
                };

                let name_text = symbol_name
                    .as_ref()
                    .map(|n| format!(" ({})", n))
                    .unwrap_or_default();

                let mut blocks = vec![
                    json!({
                        "type": "header",
                        "text": { "type": "plain_text", "text": format!("{} RouteState 변경: {}", state_emoji, new_state), "emoji": true }
                    }),
                    json!({
                        "type": "section",
                        "fields": [
                            { "type": "mrkdwn", "text": format!("*심볼*\n`{}`{}", symbol, name_text) },
                            { "type": "mrkdwn", "text": format!("*상태 변경*\n{} → *{}*", previous_state, new_state) }
                        ]
                    }),
                ];

                if let (Some(risk), Some(summary)) = (macro_risk, macro_summary) {
                    blocks.push(json!({
                        "type": "section",
                        "text": { "type": "mrkdwn", "text": format!("*매크로 환경 ({})*\n{}", risk, summary) }
                    }));
                }

                blocks.push(json!({
                    "type": "context",
                    "elements": [
                        { "type": "mrkdwn", "text": timestamp.to_string() }
                    ]
                }));

                json!({ "blocks": blocks })
            }

            NotificationEvent::Custom { title, message } => {
                json!({
                    "blocks": [
                        {
                            "type": "header",
                            "text": { "type": "plain_text", "text": format!("{} {}", priority_emoji, title), "emoji": true }
                        },
                        {
                            "type": "section",
                            "text": { "type": "mrkdwn", "text": message.clone() }
                        },
                        {
                            "type": "context",
                            "elements": [
                                { "type": "mrkdwn", "text": timestamp.to_string() }
                            ]
                        }
                    ]
                })
            }

            // 나머지 이벤트들을 위한 기본 처리
            _ => {
                json!({
                    "blocks": [
                        {
                            "type": "section",
                            "text": { "type": "mrkdwn", "text": format!("{} {:?}", priority_emoji, notification.event) }
                        },
                        {
                            "type": "context",
                            "elements": [
                                { "type": "mrkdwn", "text": timestamp.to_string() }
                            ]
                        }
                    ]
                })
            }
        }
    }

    /// Slack Webhook을 통해 메시지를 전송합니다.
    async fn send_webhook(&self, payload: serde_json::Value) -> NotificationResult<()> {
        debug!("Sending Slack webhook message");

        let response = self
            .client
            .post(&self.config.webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(NotificationError::NetworkError)?;

        if response.status().is_success() {
            info!("Slack 알림 전송 완료");
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            // 요청 한도 제한 확인
            if status.as_u16() == 429 {
                warn!("Slack rate limited");
                return Err(NotificationError::RateLimited(60));
            }

            error!("Slack webhook 전송 실패: {} - {}", status, body);
            Err(NotificationError::SendFailed(format!(
                "HTTP {}: {}",
                status, body
            )))
        }
    }

    /// 테스트 메시지를 전송합니다.
    pub async fn send_test(&self) -> NotificationResult<()> {
        let payload = json!({
            "blocks": [
                {
                    "type": "header",
                    "text": { "type": "plain_text", "text": "✓ Slack 알림 설정 완료", "emoji": true }
                },
                {
                    "type": "section",
                    "text": { "type": "mrkdwn", "text": "ZeroQuant 트레이딩 봇의 Slack 알림이 정상적으로 설정되었습니다.\n이제 트레이딩 알림을 이 채널로 받으실 수 있습니다." }
                },
                {
                    "type": "context",
                    "elements": [
                        { "type": "mrkdwn", "text": "ZeroQuant Trading Bot" }
                    ]
                }
            ]
        });

        self.send_webhook(payload).await
    }
}

#[async_trait]
impl NotificationSender for SlackSender {
    async fn send(&self, notification: &Notification) -> NotificationResult<()> {
        if !self.is_enabled() {
            debug!("Slack 알림이 비활성화되어 있습니다");
            return Ok(());
        }

        let payload = self.format_blocks(notification);
        self.send_webhook(payload).await
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled && !self.config.webhook_url.is_empty()
    }

    fn name(&self) -> &str {
        "slack"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slack_config_new() {
        let config = SlackConfig::new("https://hooks.slack.com/services/T00/B00/xxx".to_string());
        assert!(config.webhook_url.contains("hooks.slack.com"));
        assert!(config.enabled);
        assert!(config.display_name.is_none());
    }

    #[test]
    fn test_priority_emoji() {
        let config = SlackConfig::new("https://example.com".to_string());
        let sender = SlackSender::new(config);

        assert_eq!(sender.get_priority_emoji(&NotificationPriority::Low), "ℹ️");
        assert_eq!(sender.get_priority_emoji(&NotificationPriority::Critical), "🚨");
    }

    #[test]
    fn test_format_blocks_system_error() {
        let config = SlackConfig::new("https://example.com".to_string());
        let sender = SlackSender::new(config);

        let notification = Notification::new(NotificationEvent::SystemError {
            error_code: "E001".to_string(),
            message: "테스트 오류".to_string(),
        });

        let blocks = sender.format_blocks(&notification);
        assert!(blocks["blocks"].is_array());

        let header = &blocks["blocks"][0];
        assert_eq!(header["type"], "header");
        assert!(header["text"]["text"].as_str().unwrap().contains("시스템 오류"));
    }
}
