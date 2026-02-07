//! Discord 알림 서비스.
//!
//! Discord Webhook을 통해 트레이딩 알림 및 업데이트를 전송합니다.

use crate::types::{
    Notification, NotificationError, NotificationEvent, NotificationPriority, NotificationResult,
    NotificationSender,
};
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::json;
use tracing::{debug, error, info, warn};

/// Discord 알림 전송 설정.
#[derive(Debug, Clone)]
pub struct DiscordConfig {
    /// Discord Webhook URL
    pub webhook_url: String,
    /// 표시 이름 (봇 이름으로 표시)
    pub display_name: Option<String>,
    /// 서버 이름 (메타데이터용)
    pub server_name: Option<String>,
    /// 채널 이름 (메타데이터용)
    pub channel_name: Option<String>,
    /// 전송 활성화 여부
    pub enabled: bool,
}

impl DiscordConfig {
    /// 새 Discord 설정을 생성합니다.
    pub fn new(webhook_url: String) -> Self {
        Self {
            webhook_url,
            display_name: None,
            server_name: None,
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
        let webhook_url = std::env::var("DISCORD_WEBHOOK_URL").ok()?;
        let display_name = std::env::var("DISCORD_DISPLAY_NAME").ok();
        let enabled = std::env::var("DISCORD_ENABLED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);

        Some(Self {
            webhook_url,
            display_name,
            server_name: None,
            channel_name: None,
            enabled,
        })
    }
}

/// Discord 알림 전송기.
pub struct DiscordSender {
    config: DiscordConfig,
    client: reqwest::Client,
}

impl DiscordSender {
    /// 새 Discord 전송기를 생성합니다.
    pub fn new(config: DiscordConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// 환경 변수에서 전송기를 생성합니다.
    pub fn from_env() -> Option<Self> {
        DiscordConfig::from_env().map(Self::new)
    }

    /// 우선순위에 따른 색상을 반환합니다 (Discord embed color는 decimal 값 사용).
    fn get_priority_color(&self, priority: &NotificationPriority) -> u32 {
        match priority {
            NotificationPriority::Low => 0x6c757d,      // 회색
            NotificationPriority::Normal => 0x007bff,   // 파랑
            NotificationPriority::High => 0xfd7e14,     // 주황
            NotificationPriority::Critical => 0xdc3545, // 빨강
        }
    }

    /// 알림을 Discord Embed로 포맷합니다.
    fn format_embed(&self, notification: &Notification) -> serde_json::Value {
        let color = self.get_priority_color(&notification.priority);
        let timestamp = notification.timestamp.to_rfc3339();

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
                    "title": format!("{} 주문 체결", side_emoji),
                    "color": if side.to_lowercase() == "buy" { 0x28a745 } else { 0xdc3545 },
                    "fields": [
                        { "name": "심볼", "value": format!("`{}`", symbol), "inline": true },
                        { "name": "방향", "value": side, "inline": true },
                        { "name": "수량", "value": quantity.to_string(), "inline": true },
                        { "name": "가격", "value": price.to_string(), "inline": true },
                        { "name": "주문ID", "value": format!("`{}`", order_id), "inline": false }
                    ],
                    "timestamp": timestamp
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
                let pnl_color = if *pnl >= Decimal::ZERO { 0x28a745 } else { 0xdc3545 };
                let pnl_sign = if *pnl >= Decimal::ZERO { "+" } else { "" };
                json!({
                    "title": format!("{} 포지션 청산", pnl_emoji),
                    "color": pnl_color,
                    "fields": [
                        { "name": "심볼", "value": format!("`{}`", symbol), "inline": true },
                        { "name": "방향", "value": side, "inline": true },
                        { "name": "수량", "value": quantity.to_string(), "inline": true },
                        { "name": "진입가", "value": entry_price.to_string(), "inline": true },
                        { "name": "청산가", "value": exit_price.to_string(), "inline": true },
                        { "name": "손익", "value": format!("**{}{}** ({}{}%)", pnl_sign, pnl, pnl_sign, pnl_percent), "inline": true }
                    ],
                    "timestamp": timestamp
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
                let signal_color = match signal_type.as_str() {
                    "ENTRY" | "Entry" => 0x28a745,
                    "EXIT" | "Exit" => 0xdc3545,
                    _ => 0xffc107,
                };

                let strength_bar = "█".repeat((strength * 10.0) as usize);
                let empty_bar = "░".repeat(10 - (strength * 10.0) as usize);

                let mut fields = vec![
                    json!({ "name": "전략", "value": strategy_name, "inline": true }),
                    json!({ "name": "심볼", "value": format!("`{}`", symbol), "inline": true }),
                ];

                if let Some(s) = side {
                    fields.push(json!({ "name": "방향", "value": s, "inline": true }));
                }

                fields.extend([
                    json!({ "name": "가격", "value": price.to_string(), "inline": true }),
                    json!({ "name": "신호 강도", "value": format!("{}{} {:.0}%", strength_bar, empty_bar, strength * 100.0), "inline": false }),
                    json!({ "name": "이유", "value": reason, "inline": false }),
                ]);

                json!({
                    "title": format!("{} {} 신호", signal_emoji, signal_type),
                    "color": signal_color,
                    "fields": fields,
                    "timestamp": timestamp
                })
            }

            NotificationEvent::StopLossTriggered {
                symbol,
                quantity,
                trigger_price,
                loss,
            } => {
                json!({
                    "title": "🛑 손절 발동",
                    "color": 0xdc3545,
                    "fields": [
                        { "name": "심볼", "value": format!("`{}`", symbol), "inline": true },
                        { "name": "수량", "value": quantity.to_string(), "inline": true },
                        { "name": "발동가", "value": trigger_price.to_string(), "inline": true },
                        { "name": "손실", "value": format!("**-{}**", loss), "inline": true }
                    ],
                    "timestamp": timestamp
                })
            }

            NotificationEvent::TakeProfitTriggered {
                symbol,
                quantity,
                trigger_price,
                profit,
            } => {
                json!({
                    "title": "🎯 익절 발동",
                    "color": 0x28a745,
                    "fields": [
                        { "name": "심볼", "value": format!("`{}`", symbol), "inline": true },
                        { "name": "수량", "value": quantity.to_string(), "inline": true },
                        { "name": "발동가", "value": trigger_price.to_string(), "inline": true },
                        { "name": "수익", "value": format!("**+{}**", profit), "inline": true }
                    ],
                    "timestamp": timestamp
                })
            }

            NotificationEvent::SystemError { error_code, message } => {
                json!({
                    "title": "🚨 시스템 오류",
                    "color": 0xdc3545,
                    "fields": [
                        { "name": "오류 코드", "value": format!("`{}`", error_code), "inline": true },
                        { "name": "메시지", "value": message, "inline": false }
                    ],
                    "timestamp": timestamp
                })
            }

            NotificationEvent::RiskAlert {
                alert_type,
                message,
                current_value,
                threshold,
            } => {
                json!({
                    "title": "⚠️ 리스크 경고",
                    "color": 0xfd7e14,
                    "fields": [
                        { "name": "유형", "value": alert_type, "inline": true },
                        { "name": "메시지", "value": message, "inline": false },
                        { "name": "현재값", "value": current_value.to_string(), "inline": true },
                        { "name": "임계값", "value": threshold.to_string(), "inline": true }
                    ],
                    "timestamp": timestamp
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

                let mut fields = vec![
                    json!({ "name": "심볼", "value": format!("`{}`{}", symbol, symbol_name.as_ref().map(|n| format!(" ({})", n)).unwrap_or_default()), "inline": true }),
                    json!({ "name": "상태 변경", "value": format!("{} → **{}**", previous_state, new_state), "inline": true }),
                ];

                if let (Some(risk), Some(summary)) = (macro_risk, macro_summary) {
                    fields.push(json!({ "name": "매크로 환경", "value": format!("**{}**\n{}", risk, summary), "inline": false }));
                }

                json!({
                    "title": format!("{} RouteState 변경: {}", state_emoji, new_state),
                    "color": color,
                    "fields": fields,
                    "timestamp": timestamp
                })
            }

            NotificationEvent::Custom { title, message } => {
                json!({
                    "title": title,
                    "description": message,
                    "color": color,
                    "timestamp": timestamp
                })
            }

            // 나머지 이벤트들을 위한 기본 처리
            _ => {
                json!({
                    "title": format!("{:?}", notification.event),
                    "color": color,
                    "timestamp": timestamp
                })
            }
        }
    }

    /// Discord Webhook을 통해 메시지를 전송합니다.
    async fn send_webhook(&self, embed: serde_json::Value) -> NotificationResult<()> {
        let mut payload = json!({
            "embeds": [embed],
        });

        // 봇 이름 설정
        if let Some(ref name) = self.config.display_name {
            payload["username"] = json!(name);
        }

        debug!("Sending Discord webhook message");

        let response = self
            .client
            .post(&self.config.webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(NotificationError::NetworkError)?;

        if response.status().is_success() {
            info!("Discord 알림 전송 완료");
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            // 요청 한도 제한 확인
            if status.as_u16() == 429 {
                warn!("Discord rate limited");
                return Err(NotificationError::RateLimited(60));
            }

            error!("Discord webhook 전송 실패: {} - {}", status, body);
            Err(NotificationError::SendFailed(format!(
                "HTTP {}: {}",
                status, body
            )))
        }
    }

    /// 테스트 메시지를 전송합니다.
    pub async fn send_test(&self) -> NotificationResult<()> {
        let embed = json!({
            "title": "✓ Discord 알림 설정 완료",
            "description": "ZeroQuant 트레이딩 봇의 Discord 알림이 정상적으로 설정되었습니다.\n이제 트레이딩 알림을 이 채널로 받으실 수 있습니다.",
            "color": 0x28a745,
            "footer": { "text": "ZeroQuant Trading Bot" }
        });

        self.send_webhook(embed).await
    }
}

#[async_trait]
impl NotificationSender for DiscordSender {
    async fn send(&self, notification: &Notification) -> NotificationResult<()> {
        if !self.is_enabled() {
            debug!("Discord 알림이 비활성화되어 있습니다");
            return Ok(());
        }

        let embed = self.format_embed(notification);
        self.send_webhook(embed).await
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled && !self.config.webhook_url.is_empty()
    }

    fn name(&self) -> &str {
        "discord"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discord_config_new() {
        let config = DiscordConfig::new("https://discord.com/api/webhooks/123/abc".to_string());
        assert!(config.webhook_url.contains("discord.com"));
        assert!(config.enabled);
        assert!(config.display_name.is_none());
    }

    #[test]
    fn test_priority_colors() {
        let config = DiscordConfig::new("https://example.com".to_string());
        let sender = DiscordSender::new(config);

        assert_eq!(sender.get_priority_color(&NotificationPriority::Low), 0x6c757d);
        assert_eq!(sender.get_priority_color(&NotificationPriority::Critical), 0xdc3545);
    }

    #[test]
    fn test_format_embed_signal() {
        let config = DiscordConfig::new("https://example.com".to_string());
        let sender = DiscordSender::new(config);

        let notification = Notification::new(NotificationEvent::SignalAlert {
            signal_type: "Entry".to_string(),
            symbol: "AAPL".to_string(),
            side: Some("Buy".to_string()),
            price: Decimal::new(15000, 2),
            strength: 0.8,
            reason: "RSI 과매도".to_string(),
            strategy_name: "momentum".to_string(),
            indicators: serde_json::json!({}),
        });

        let embed = sender.format_embed(&notification);
        assert!(embed["title"].as_str().unwrap().contains("Entry"));
        assert_eq!(embed["color"], 0x28a745); // 녹색 (Entry)
    }
}
