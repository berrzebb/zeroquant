//! 텔레그램 알림 서비스.
//!
//! Telegram Bot API를 통해 트레이딩 알림 및 업데이트를 전송합니다.

use crate::types::{
    Notification, NotificationError, NotificationEvent, NotificationPriority, NotificationResult,
    NotificationSender,
};
use async_trait::async_trait;
use rust_decimal::Decimal;
use tracing::{debug, error, info, warn};

/// 텔레그램 알림 전송 설정.
#[derive(Debug, Clone)]
pub struct TelegramConfig {
    /// @BotFather에서 받은 봇 토큰
    pub bot_token: String,
    /// 메시지를 보낼 채팅 ID
    pub chat_id: String,
    /// 전송 활성화 여부
    pub enabled: bool,
    /// 파싱 모드 (HTML 또는 MarkdownV2)
    pub parse_mode: String,
}

impl TelegramConfig {
    /// 새 텔레그램 설정을 생성합니다.
    pub fn new(bot_token: String, chat_id: String) -> Self {
        Self {
            bot_token,
            chat_id,
            enabled: true,
            parse_mode: "HTML".to_string(),
        }
    }

    /// 환경 변수에서 설정을 생성합니다.
    pub fn from_env() -> Option<Self> {
        let bot_token = std::env::var("TELEGRAM_BOT_TOKEN").ok()?;
        let chat_id = std::env::var("TELEGRAM_CHAT_ID").ok()?;
        let enabled = std::env::var("TELEGRAM_ENABLED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);

        Some(Self {
            bot_token,
            chat_id,
            enabled,
            parse_mode: "HTML".to_string(),
        })
    }
}

/// 텔레그램 알림 전송기.
pub struct TelegramSender {
    config: TelegramConfig,
    client: reqwest::Client,
}

impl TelegramSender {
    /// 새 텔레그램 전송기를 생성합니다.
    pub fn new(config: TelegramConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// 환경 변수에서 전송기를 생성합니다.
    pub fn from_env() -> Option<Self> {
        TelegramConfig::from_env().map(Self::new)
    }

    /// 알림을 텔레그램 메시지로 포맷합니다.
    fn format_message(&self, notification: &Notification) -> String {
        let priority_emoji = match notification.priority {
            NotificationPriority::Low => "ℹ️",
            NotificationPriority::Normal => "📊",
            NotificationPriority::High => "⚠️",
            NotificationPriority::Critical => "🚨",
        };

        let content = match &notification.event {
            NotificationEvent::OrderFilled {
                symbol,
                side,
                quantity,
                price,
                order_id,
            } => {
                let side_emoji = if side.to_lowercase() == "buy" {
                    "🟢"
                } else {
                    "🔴"
                };
                format!(
                    "{side_emoji} <b>주문 체결</b>\n\n\
                     심볼: <code>{symbol}</code>\n\
                     방향: {side}\n\
                     수량: {quantity}\n\
                     가격: {price}\n\
                     주문ID: <code>{order_id}</code>"
                )
            }

            NotificationEvent::PositionOpened {
                symbol,
                side,
                quantity,
                entry_price,
            } => {
                let side_emoji = if side.to_lowercase() == "buy" {
                    "🟢"
                } else {
                    "🔴"
                };
                format!(
                    "{side_emoji} <b>포지션 진입</b>\n\n\
                     심볼: <code>{symbol}</code>\n\
                     방향: {side}\n\
                     수량: {quantity}\n\
                     진입가: {entry_price}"
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
                let pnl_emoji = if *pnl >= Decimal::ZERO {
                    "💰"
                } else {
                    "📉"
                };
                let pnl_sign = if *pnl >= Decimal::ZERO { "+" } else { "" };
                format!(
                    "{pnl_emoji} <b>포지션 청산</b>\n\n\
                     심볼: <code>{symbol}</code>\n\
                     방향: {side}\n\
                     수량: {quantity}\n\
                     진입가: {entry_price}\n\
                     청산가: {exit_price}\n\
                     손익: <b>{pnl_sign}{pnl}</b> ({pnl_sign}{pnl_percent}%)"
                )
            }

            NotificationEvent::StopLossTriggered {
                symbol,
                quantity,
                trigger_price,
                loss,
            } => {
                format!(
                    "🛑 <b>손절 발동</b>\n\n\
                     심볼: <code>{symbol}</code>\n\
                     수량: {quantity}\n\
                     발동가: {trigger_price}\n\
                     손실: <b>-{loss}</b>"
                )
            }

            NotificationEvent::TakeProfitTriggered {
                symbol,
                quantity,
                trigger_price,
                profit,
            } => {
                format!(
                    "🎯 <b>익절 발동</b>\n\n\
                     심볼: <code>{symbol}</code>\n\
                     수량: {quantity}\n\
                     발동가: {trigger_price}\n\
                     수익: <b>+{profit}</b>"
                )
            }

            NotificationEvent::DailySummary {
                date,
                total_trades,
                winning_trades,
                total_pnl,
                win_rate,
            } => {
                let pnl_emoji = if *total_pnl >= Decimal::ZERO {
                    "💰"
                } else {
                    "📉"
                };
                let pnl_sign = if *total_pnl >= Decimal::ZERO { "+" } else { "" };
                format!(
                    "📅 <b>일일 요약</b> ({date})\n\n\
                     총 거래: {total_trades}건\n\
                     승리: {winning_trades}건\n\
                     승률: {win_rate}%\n\
                     총 손익: {pnl_emoji} <b>{pnl_sign}{total_pnl}</b>"
                )
            }

            NotificationEvent::RiskAlert {
                alert_type,
                message,
                current_value,
                threshold,
            } => {
                format!(
                    "⚠️ <b>리스크 경고</b>\n\n\
                     유형: {alert_type}\n\
                     메시지: {message}\n\
                     현재값: {current_value}\n\
                     임계값: {threshold}"
                )
            }

            NotificationEvent::StrategyStarted {
                strategy_id,
                strategy_name,
            } => {
                format!(
                    "▶️ <b>전략 시작</b>\n\n\
                     전략: {strategy_name}\n\
                     ID: <code>{strategy_id}</code>"
                )
            }

            NotificationEvent::StrategyStopped {
                strategy_id,
                strategy_name,
                reason,
            } => {
                format!(
                    "⏹️ <b>전략 중지</b>\n\n\
                     전략: {strategy_name}\n\
                     ID: <code>{strategy_id}</code>\n\
                     사유: {reason}"
                )
            }

            NotificationEvent::SystemError {
                error_code,
                message,
            } => {
                format!(
                    "🚨 <b>시스템 오류</b>\n\n\
                     코드: <code>{error_code}</code>\n\
                     메시지: {message}"
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
                indicators,
            } => {
                let signal_emoji = match signal_type.as_str() {
                    "ENTRY" | "Entry" => "🟢",
                    "EXIT" | "Exit" => "🔴",
                    "ALERT" | "Alert" => "🔔",
                    _ => "📍",
                };

                let strength_stars = "⭐".repeat((*strength * 5.0) as usize);
                let side_text = side
                    .as_ref()
                    .map(|s| format!("\n방향: {}", s))
                    .unwrap_or_default();

                // 주요 지표 추출 (RSI, MACD 등)
                let mut indicator_lines = Vec::new();
                if let Some(obj) = indicators.as_object() {
                    if let Some(rsi) = obj.get("rsi").and_then(|v| v.as_f64()) {
                        indicator_lines.push(format!("RSI: {:.1}", rsi));
                    }
                    if let Some(macd) = obj.get("macd").and_then(|v| v.as_str()) {
                        indicator_lines.push(format!("MACD: {}", macd));
                    }
                }
                let indicators_text = if indicator_lines.is_empty() {
                    String::new()
                } else {
                    format!("\n\n<i>{}</i>", indicator_lines.join(" | "))
                };

                format!(
                    "{signal_emoji} <b>{signal_type} 신호</b> {strength_stars}\n\n\
                     전략: {strategy_name}\n\
                     심볼: <code>{symbol}</code>{side_text}\n\
                     가격: {price}\n\
                     강도: {:.0}%\n\
                     이유: {reason}{indicators_text}",
                    strength * 100.0
                )
            }

            NotificationEvent::Custom { title, message } => {
                format!("{priority_emoji} <b>{title}</b>\n\n{message}")
            }

            NotificationEvent::RouteStateChanged {
                symbol,
                symbol_name,
                previous_state,
                new_state,
                macro_risk,
                macro_summary,
            } => {
                // 상태별 이모지
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

                let macro_text = match (macro_risk, macro_summary) {
                    (Some(risk), Some(summary)) => {
                        let risk_emoji = match risk.to_uppercase().as_str() {
                            "CRITICAL" => "🚨",
                            "HIGH" => "⚠️",
                            _ => "✅",
                        };
                        format!("\n\n<b>매크로 환경</b> {risk_emoji}\n{summary}")
                    }
                    _ => String::new(),
                };

                format!(
                    "{state_emoji} <b>RouteState 변경: {new_state}</b>\n\n\
                     심볼: <code>{symbol}</code>{name_text}\n\
                     상태 변경: {previous_state} → <b>{new_state}</b>{macro_text}"
                )
            }

            NotificationEvent::MacroAlert {
                risk_level,
                usd_krw,
                usd_change_pct,
                nasdaq_change_pct,
                recommendation,
            } => {
                let risk_emoji = match risk_level.to_uppercase().as_str() {
                    "CRITICAL" => "🚨",
                    "HIGH" => "⚠️",
                    _ => "✅",
                };

                format!(
                    "{risk_emoji} <b>매크로 환경 경고: {risk_level}</b>\n\n\
                     USD/KRW: {usd_krw}원 ({usd_change_pct}%)\n\
                     나스닥: {nasdaq_change_pct}%\n\n\
                     <i>{recommendation}</i>"
                )
            }

            NotificationEvent::MarketBreadthAlert {
                temperature,
                all_ratio,
                kospi_ratio,
                kosdaq_ratio,
                recommendation,
            } => {
                let temp_emoji = match temperature.to_uppercase().as_str() {
                    "OVERHEAT" => "🔥",
                    "COLD" => "🧊",
                    _ => "🌤",
                };

                format!(
                    "{temp_emoji} <b>시장 온도: {temperature}</b>\n\n\
                     전체: {all_ratio}%\n\
                     코스피: {kospi_ratio}%\n\
                     코스닥: {kosdaq_ratio}%\n\n\
                     <i>{recommendation}</i>"
                )
            }
        };

        let timestamp = notification.timestamp.format("%Y-%m-%d %H:%M:%S UTC");
        format!("{content}\n\n<i>🕐 {timestamp}</i>")
    }

    /// 텔레그램에 원시 메시지를 전송합니다.
    async fn send_message(&self, text: &str) -> NotificationResult<()> {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.config.bot_token
        );

        let params = serde_json::json!({
            "chat_id": self.config.chat_id,
            "text": text,
            "parse_mode": self.config.parse_mode,
            "disable_web_page_preview": true,
        });

        debug!(
            "Sending Telegram message to chat_id: {}",
            self.config.chat_id
        );

        let response = self
            .client
            .post(&url)
            .json(&params)
            .send()
            .await
            .map_err(NotificationError::NetworkError)?;

        if response.status().is_success() {
            info!("Telegram notification sent successfully");
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            // 요청 한도 제한 확인
            if status.as_u16() == 429 {
                warn!("Telegram rate limited");
                return Err(NotificationError::RateLimited(60));
            }

            error!("Failed to send Telegram message: {} - {}", status, body);
            Err(NotificationError::SendFailed(format!(
                "HTTP {}: {}",
                status, body
            )))
        }
    }
}

#[async_trait]
impl NotificationSender for TelegramSender {
    async fn send(&self, notification: &Notification) -> NotificationResult<()> {
        if !self.is_enabled() {
            debug!("Telegram notifications are disabled, skipping");
            return Ok(());
        }

        let message = self.format_message(notification);
        self.send_message(&message).await
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled && !self.config.bot_token.is_empty() && !self.config.chat_id.is_empty()
    }

    fn name(&self) -> &str {
        "telegram"
    }
}

/// 여러 전송기를 관리하는 알림 관리자.
pub struct NotificationManager {
    senders: Vec<Box<dyn NotificationSender>>,
}

impl NotificationManager {
    /// 새 알림 관리자를 생성합니다.
    pub fn new() -> Self {
        Self {
            senders: Vec::new(),
        }
    }

    /// 알림 전송기를 추가합니다.
    pub fn add_sender<S: NotificationSender + 'static>(&mut self, sender: S) {
        self.senders.push(Box::new(sender));
    }

    /// 활성화된 모든 전송기를 통해 알림을 전송합니다.
    pub async fn notify(&self, notification: &Notification) -> NotificationResult<()> {
        let mut last_error = None;

        for sender in &self.senders {
            if sender.is_enabled() {
                if let Err(e) = sender.send(notification).await {
                    error!("Failed to send notification via {}: {}", sender.name(), e);
                    last_error = Some(e);
                }
            }
        }

        if let Some(e) = last_error {
            // 모든 전송기가 실패한 경우에만 에러 반환
            if self.senders.iter().filter(|s| s.is_enabled()).count() == 1 {
                return Err(e);
            }
        }

        Ok(())
    }

    /// 주문 체결 알림을 전송합니다.
    pub async fn notify_order_filled(
        &self,
        symbol: &str,
        side: &str,
        quantity: Decimal,
        price: Decimal,
        order_id: &str,
    ) -> NotificationResult<()> {
        let notification = Notification::new(NotificationEvent::OrderFilled {
            symbol: symbol.to_string(),
            side: side.to_string(),
            quantity,
            price,
            order_id: order_id.to_string(),
        });
        self.notify(&notification).await
    }

    /// 포지션 청산 알림을 전송합니다.
    pub async fn notify_position_closed(
        &self,
        symbol: &str,
        side: &str,
        quantity: Decimal,
        entry_price: Decimal,
        exit_price: Decimal,
        pnl: Decimal,
        pnl_percent: Decimal,
    ) -> NotificationResult<()> {
        let priority = if pnl >= Decimal::ZERO {
            NotificationPriority::Normal
        } else {
            NotificationPriority::High
        };

        let notification = Notification::new(NotificationEvent::PositionClosed {
            symbol: symbol.to_string(),
            side: side.to_string(),
            quantity,
            entry_price,
            exit_price,
            pnl,
            pnl_percent,
        })
        .with_priority(priority);

        self.notify(&notification).await
    }

    /// 리스크 경고 알림을 전송합니다.
    pub async fn notify_risk_alert(
        &self,
        alert_type: &str,
        message: &str,
        current_value: Decimal,
        threshold: Decimal,
    ) -> NotificationResult<()> {
        let notification = Notification::new(NotificationEvent::RiskAlert {
            alert_type: alert_type.to_string(),
            message: message.to_string(),
            current_value,
            threshold,
        })
        .with_priority(NotificationPriority::Critical);

        self.notify(&notification).await
    }

    /// 시스템 오류 알림을 전송합니다.
    pub async fn notify_system_error(
        &self,
        error_code: &str,
        message: &str,
    ) -> NotificationResult<()> {
        let notification = Notification::new(NotificationEvent::SystemError {
            error_code: error_code.to_string(),
            message: message.to_string(),
        })
        .with_priority(NotificationPriority::Critical);

        self.notify(&notification).await
    }

    /// 신호 마커 알림을 전송합니다.
    ///
    /// # 인자
    /// - `signal_type`: 신호 유형 (Entry, Exit, Alert 등)
    /// - `symbol`: 거래 심볼
    /// - `side`: 거래 방향 (Buy/Sell, 선택)
    /// - `price`: 신호 발생 시점 가격
    /// - `strength`: 신호 강도 (0.0 ~ 1.0)
    /// - `reason`: 신호 생성 이유
    /// - `strategy_name`: 전략 이름
    /// - `indicators`: 지표 정보 (JSON)
    #[allow(clippy::too_many_arguments)]
    pub async fn notify_signal_alert(
        &self,
        signal_type: &str,
        symbol: &str,
        side: Option<&str>,
        price: Decimal,
        strength: f64,
        reason: &str,
        strategy_name: &str,
        indicators: serde_json::Value,
    ) -> NotificationResult<()> {
        // 신호 강도에 따라 우선순위 설정
        let priority = if strength >= 0.8 {
            NotificationPriority::High
        } else if strength >= 0.5 {
            NotificationPriority::Normal
        } else {
            NotificationPriority::Low
        };

        let notification = Notification::new(NotificationEvent::SignalAlert {
            signal_type: signal_type.to_string(),
            symbol: symbol.to_string(),
            side: side.map(|s| s.to_string()),
            price,
            strength,
            reason: reason.to_string(),
            strategy_name: strategy_name.to_string(),
            indicators,
        })
        .with_priority(priority);

        self.notify(&notification).await
    }

    /// RouteState 변경 알림을 전송합니다 (ATTACK 상태 진입 시).
    ///
    /// # 인자
    /// - `symbol`: 종목 코드
    /// - `symbol_name`: 종목명 (선택)
    /// - `previous_state`: 이전 상태
    /// - `new_state`: 새 상태
    /// - `macro_risk`: 매크로 위험 수준 (선택)
    /// - `macro_summary`: 매크로 환경 요약 (선택)
    pub async fn notify_route_state_changed(
        &self,
        symbol: &str,
        symbol_name: Option<&str>,
        previous_state: &str,
        new_state: &str,
        macro_risk: Option<&str>,
        macro_summary: Option<&str>,
    ) -> NotificationResult<()> {
        // ATTACK 또는 OVERHEAT 상태 진입 시 High 우선순위
        let upper_state = new_state.to_uppercase();
        let priority = if upper_state == "ATTACK" || upper_state == "OVERHEAT" {
            NotificationPriority::High
        } else {
            NotificationPriority::Normal
        };

        let notification = Notification::new(NotificationEvent::RouteStateChanged {
            symbol: symbol.to_string(),
            symbol_name: symbol_name.map(|s| s.to_string()),
            previous_state: previous_state.to_string(),
            new_state: new_state.to_string(),
            macro_risk: macro_risk.map(|s| s.to_string()),
            macro_summary: macro_summary.map(|s| s.to_string()),
        })
        .with_priority(priority);

        self.notify(&notification).await
    }

    /// 매크로 환경 경고 알림을 전송합니다.
    pub async fn notify_macro_alert(
        &self,
        risk_level: &str,
        usd_krw: &str,
        usd_change_pct: &str,
        nasdaq_change_pct: &str,
        recommendation: &str,
    ) -> NotificationResult<()> {
        let priority = match risk_level.to_uppercase().as_str() {
            "CRITICAL" => NotificationPriority::Critical,
            "HIGH" => NotificationPriority::High,
            _ => NotificationPriority::Normal,
        };

        let notification = Notification::new(NotificationEvent::MacroAlert {
            risk_level: risk_level.to_string(),
            usd_krw: usd_krw.to_string(),
            usd_change_pct: usd_change_pct.to_string(),
            nasdaq_change_pct: nasdaq_change_pct.to_string(),
            recommendation: recommendation.to_string(),
        })
        .with_priority(priority);

        self.notify(&notification).await
    }

    /// 시장 온도 알림을 전송합니다.
    pub async fn notify_market_breadth(
        &self,
        temperature: &str,
        all_ratio: &str,
        kospi_ratio: &str,
        kosdaq_ratio: &str,
        recommendation: &str,
    ) -> NotificationResult<()> {
        let priority = match temperature.to_uppercase().as_str() {
            "OVERHEAT" | "COLD" => NotificationPriority::High,
            _ => NotificationPriority::Normal,
        };

        let notification = Notification::new(NotificationEvent::MarketBreadthAlert {
            temperature: temperature.to_string(),
            all_ratio: all_ratio.to_string(),
            kospi_ratio: kospi_ratio.to_string(),
            kosdaq_ratio: kosdaq_ratio.to_string(),
            recommendation: recommendation.to_string(),
        })
        .with_priority(priority);

        self.notify(&notification).await
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_order_filled() {
        let config = TelegramConfig::new("test_token".to_string(), "123456".to_string());
        let sender = TelegramSender::new(config);

        let notification = Notification::new(NotificationEvent::OrderFilled {
            symbol: "BTC/USDT".to_string(),
            side: "buy".to_string(),
            quantity: Decimal::new(1, 2), // 0.01
            price: Decimal::new(50000, 0),
            order_id: "12345".to_string(),
        });

        let message = sender.format_message(&notification);
        assert!(message.contains("주문 체결"));
        assert!(message.contains("BTC/USDT"));
        assert!(message.contains("buy"));
    }

    #[test]
    fn test_format_position_closed_profit() {
        let config = TelegramConfig::new("test_token".to_string(), "123456".to_string());
        let sender = TelegramSender::new(config);

        let notification = Notification::new(NotificationEvent::PositionClosed {
            symbol: "ETH/USDT".to_string(),
            side: "buy".to_string(),
            quantity: Decimal::new(1, 0),
            entry_price: Decimal::new(3000, 0),
            exit_price: Decimal::new(3100, 0),
            pnl: Decimal::new(100, 0),
            pnl_percent: Decimal::new(333, 2), // 3.33%
        });

        let message = sender.format_message(&notification);
        assert!(message.contains("포지션 청산"));
        assert!(message.contains("💰")); // Profit emoji
        assert!(message.contains("+100"));
    }
}
