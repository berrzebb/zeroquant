//! 알림 route.
//!
//! 알림 설정 조회 및 테스트 endpoint.

use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error};
use utoipa::ToSchema;

use crate::state::AppState;
use rust_decimal_macros::dec;
use trader_notification::{
    Notification, NotificationEvent, NotificationPriority, NotificationSender, TelegramConfig,
    TelegramSender,
};

/// 텔레그램 테스트 요청.
#[derive(Debug, Deserialize, ToSchema)]
pub struct TelegramTestRequest {
    /// Bot Token
    pub bot_token: String,
    /// Chat ID
    pub chat_id: String,
}

/// 텔레그램 테스트 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramTestResponse {
    /// 성공 여부
    pub success: bool,
    /// 메시지
    pub message: String,
}

/// 알림 설정 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationSettingsResponse {
    /// 텔레그램 활성화 여부
    pub telegram_enabled: bool,
    /// 텔레그램 설정 여부
    pub telegram_configured: bool,
}

/// 템플릿 테스트 요청.
#[derive(Debug, Deserialize, ToSchema)]
pub struct TemplateTestRequest {
    /// 템플릿 타입
    pub template_type: String,
}

/// 사용 가능한 템플릿 목록 응답.
#[derive(Debug, Serialize, ToSchema)]
pub struct TemplateListResponse {
    /// 템플릿 목록
    pub templates: Vec<TemplateInfo>,
}

/// 템플릿 정보.
#[derive(Debug, Serialize, ToSchema)]
pub struct TemplateInfo {
    /// 템플릿 ID
    pub id: String,
    /// 템플릿 이름
    pub name: String,
    /// 설명
    pub description: String,
    /// 우선순위
    pub priority: String,
}

/// 텔레그램 연결 테스트.
///
/// `POST /api/v1/notifications/test/telegram`
#[utoipa::path(
    post,
    path = "/api/v1/notifications/test/telegram",
    tag = "notifications",
    request_body = TelegramTestRequest,
    responses(
        (status = 200, description = "텔레그램 연결 테스트 성공", body = TelegramTestResponse),
        (status = 400, description = "잘못된 입력 또는 연결 실패", body = TelegramTestResponse)
    )
)]
pub async fn test_telegram(Json(payload): Json<TelegramTestRequest>) -> impl IntoResponse {
    debug!(
        "Testing Telegram connection for chat_id: {}",
        payload.chat_id
    );

    // 입력 검증
    if payload.bot_token.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(TelegramTestResponse {
                success: false,
                message: "Bot Token이 비어있습니다.".to_string(),
            }),
        );
    }

    if payload.chat_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(TelegramTestResponse {
                success: false,
                message: "Chat ID가 비어있습니다.".to_string(),
            }),
        );
    }

    // 설정 생성
    let config = TelegramConfig::new(payload.bot_token.clone(), payload.chat_id.clone());
    let sender = TelegramSender::new(config);

    // 테스트 메시지 생성
    let notification = Notification::new(NotificationEvent::Custom {
        title: "연결 테스트".to_string(),
        message: "🎉 텔레그램 봇이 정상적으로 연결되었습니다!\n\nTrader Bot이 이 채팅으로 알림을 보낼 준비가 되었습니다.".to_string(),
    });

    // 메시지 전송
    match sender.send(&notification).await {
        Ok(_) => {
            debug!("Telegram test message sent successfully");
            (
                StatusCode::OK,
                Json(TelegramTestResponse {
                    success: true,
                    message: "테스트 메시지를 전송했습니다. 텔레그램을 확인해주세요.".to_string(),
                }),
            )
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            error!("Failed to send Telegram test message: {}", error_msg);

            // 에러 유형별 메시지
            let user_message = if error_msg.contains("401") {
                "Bot Token이 유효하지 않습니다. @BotFather에서 발급받은 토큰을 확인하세요."
                    .to_string()
            } else if error_msg.contains("400") || error_msg.contains("chat not found") {
                "Chat ID가 유효하지 않습니다. 봇에게 먼저 메시지를 보내거나 그룹에 추가하세요."
                    .to_string()
            } else if error_msg.contains("403") {
                "봇이 채팅에 메시지를 보낼 권한이 없습니다.".to_string()
            } else {
                format!("메시지 전송 실패: {}", error_msg)
            };

            (
                StatusCode::BAD_REQUEST,
                Json(TelegramTestResponse {
                    success: false,
                    message: user_message,
                }),
            )
        }
    }
}

/// 알림 설정 조회.
///
/// `GET /api/v1/notifications/settings`
#[utoipa::path(
    get,
    path = "/api/v1/notifications/settings",
    tag = "notifications",
    responses(
        (status = 200, description = "알림 설정 조회 성공", body = NotificationSettingsResponse)
    )
)]
pub async fn get_notification_settings() -> impl IntoResponse {
    let telegram_enabled = std::env::var("TELEGRAM_ENABLED")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);

    let telegram_configured =
        std::env::var("TELEGRAM_BOT_TOKEN").is_ok() && std::env::var("TELEGRAM_CHAT_ID").is_ok();

    Json(NotificationSettingsResponse {
        telegram_enabled,
        telegram_configured,
    })
}

/// 환경 변수 기반 텔레그램 테스트.
///
/// `POST /api/v1/notifications/test/telegram/env`
///
/// .env 파일에 설정된 TELEGRAM_BOT_TOKEN과 TELEGRAM_CHAT_ID를 사용하여 테스트합니다.
#[utoipa::path(
    post,
    path = "/api/v1/notifications/test/telegram/env",
    tag = "notifications",
    responses(
        (status = 200, description = "환경 변수 기반 텔레그램 테스트 성공", body = TelegramTestResponse),
        (status = 400, description = "환경 변수 미설정 또는 연결 실패", body = TelegramTestResponse)
    )
)]
pub async fn test_telegram_env() -> impl IntoResponse {
    debug!("Testing Telegram connection using environment variables");

    // 환경 변수에서 설정 읽기
    let bot_token = match std::env::var("TELEGRAM_BOT_TOKEN") {
        Ok(token) if !token.is_empty() => token,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(TelegramTestResponse {
                    success: false,
                    message: "TELEGRAM_BOT_TOKEN 환경 변수가 설정되지 않았습니다.".to_string(),
                }),
            );
        }
    };

    let chat_id = match std::env::var("TELEGRAM_CHAT_ID") {
        Ok(id) if !id.is_empty() => id.trim().to_string(), // 공백 제거
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(TelegramTestResponse {
                    success: false,
                    message: "TELEGRAM_CHAT_ID 환경 변수가 설정되지 않았습니다.".to_string(),
                }),
            );
        }
    };

    debug!(
        "Using Bot Token: {}...{}",
        &bot_token[..10.min(bot_token.len())],
        &bot_token[bot_token.len().saturating_sub(4)..]
    );
    debug!("Using Chat ID: {}", chat_id);

    // 설정 생성
    let config = TelegramConfig::new(bot_token, chat_id);
    let sender = TelegramSender::new(config);

    // 테스트 메시지 생성
    let notification = Notification::new(NotificationEvent::Custom {
        title: "연결 테스트".to_string(),
        message: "🎉 텔레그램 봇이 정상적으로 연결되었습니다!\n\nTrader Bot이 이 채팅으로 알림을 보낼 준비가 되었습니다.\n\n(환경 변수 기반 테스트)".to_string(),
    });

    // 메시지 전송
    match sender.send(&notification).await {
        Ok(_) => {
            debug!("Telegram test message sent successfully (env-based)");
            (
                StatusCode::OK,
                Json(TelegramTestResponse {
                    success: true,
                    message: "테스트 메시지를 전송했습니다. 텔레그램을 확인해주세요.".to_string(),
                }),
            )
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            error!(
                "Failed to send Telegram test message (env-based): {}",
                error_msg
            );

            let user_message = if error_msg.contains("401") {
                "Bot Token이 유효하지 않습니다. .env 파일의 TELEGRAM_BOT_TOKEN을 확인하세요."
                    .to_string()
            } else if error_msg.contains("400") || error_msg.contains("chat not found") {
                "Chat ID가 유효하지 않습니다. .env 파일의 TELEGRAM_CHAT_ID를 확인하세요."
                    .to_string()
            } else if error_msg.contains("403") {
                "봇이 채팅에 메시지를 보낼 권한이 없습니다.".to_string()
            } else {
                format!("메시지 전송 실패: {}", error_msg)
            };

            (
                StatusCode::BAD_REQUEST,
                Json(TelegramTestResponse {
                    success: false,
                    message: user_message,
                }),
            )
        }
    }
}

/// 사용 가능한 템플릿 목록 조회.
///
/// `GET /api/v1/notifications/templates`
#[utoipa::path(
    get,
    path = "/api/v1/notifications/templates",
    tag = "notifications",
    responses(
        (status = 200, description = "템플릿 목록 조회 성공", body = TemplateListResponse)
    )
)]
pub async fn get_templates() -> impl IntoResponse {
    let templates = vec![
        TemplateInfo {
            id: "order_filled".to_string(),
            name: "주문 체결".to_string(),
            description: "주문이 체결되었을 때 발송되는 알림".to_string(),
            priority: "normal".to_string(),
        },
        TemplateInfo {
            id: "position_opened".to_string(),
            name: "포지션 진입".to_string(),
            description: "새로운 포지션이 열렸을 때 발송되는 알림".to_string(),
            priority: "normal".to_string(),
        },
        TemplateInfo {
            id: "position_closed".to_string(),
            name: "포지션 청산".to_string(),
            description: "포지션이 청산되었을 때 발송되는 알림 (수익/손실 포함)".to_string(),
            priority: "normal".to_string(),
        },
        TemplateInfo {
            id: "stop_loss".to_string(),
            name: "손절 발동".to_string(),
            description: "손절가에 도달하여 자동 청산되었을 때 발송되는 알림".to_string(),
            priority: "high".to_string(),
        },
        TemplateInfo {
            id: "take_profit".to_string(),
            name: "익절 발동".to_string(),
            description: "목표가에 도달하여 수익 실현했을 때 발송되는 알림".to_string(),
            priority: "normal".to_string(),
        },
        TemplateInfo {
            id: "daily_summary".to_string(),
            name: "일일 요약".to_string(),
            description: "하루 거래 결과를 요약하여 발송되는 알림".to_string(),
            priority: "low".to_string(),
        },
        TemplateInfo {
            id: "risk_alert".to_string(),
            name: "리스크 경고".to_string(),
            description: "리스크 한도에 근접하거나 초과했을 때 발송되는 알림".to_string(),
            priority: "critical".to_string(),
        },
        TemplateInfo {
            id: "strategy_started".to_string(),
            name: "전략 시작".to_string(),
            description: "전략이 활성화되었을 때 발송되는 알림".to_string(),
            priority: "normal".to_string(),
        },
        TemplateInfo {
            id: "strategy_stopped".to_string(),
            name: "전략 중지".to_string(),
            description: "전략이 중지되었을 때 발송되는 알림".to_string(),
            priority: "high".to_string(),
        },
        TemplateInfo {
            id: "system_error".to_string(),
            name: "시스템 오류".to_string(),
            description: "시스템 오류가 발생했을 때 발송되는 알림".to_string(),
            priority: "critical".to_string(),
        },
    ];

    Json(TemplateListResponse { templates })
}

/// 샘플 알림 이벤트 생성.
fn create_sample_event(template_type: &str) -> Option<(NotificationEvent, NotificationPriority)> {
    match template_type {
        "order_filled" => Some((
            NotificationEvent::OrderFilled {
                symbol: "KODEX 200".to_string(),
                side: "Buy".to_string(),
                quantity: dec!(100),
                price: dec!(35500),
                order_id: "ORD-2026-001234".to_string(),
            },
            NotificationPriority::Normal,
        )),
        "position_opened" => Some((
            NotificationEvent::PositionOpened {
                symbol: "삼성전자".to_string(),
                side: "Long".to_string(),
                quantity: dec!(50),
                entry_price: dec!(72500),
            },
            NotificationPriority::Normal,
        )),
        "position_closed" => Some((
            NotificationEvent::PositionClosed {
                symbol: "TIGER 미국S&P500".to_string(),
                side: "Long".to_string(),
                quantity: dec!(30),
                entry_price: dec!(15200),
                exit_price: dec!(15850),
                pnl: dec!(19500),
                pnl_percent: dec!(4.28),
            },
            NotificationPriority::Normal,
        )),
        "stop_loss" => Some((
            NotificationEvent::StopLossTriggered {
                symbol: "KODEX 레버리지".to_string(),
                quantity: dec!(200),
                trigger_price: dec!(17800),
                loss: dec!(45000),
            },
            NotificationPriority::High,
        )),
        "take_profit" => Some((
            NotificationEvent::TakeProfitTriggered {
                symbol: "AAPL".to_string(),
                quantity: dec!(10),
                trigger_price: dec!(195.50),
                profit: dec!(125.00),
            },
            NotificationPriority::Normal,
        )),
        "daily_summary" => Some((
            NotificationEvent::DailySummary {
                date: "2026-01-29".to_string(),
                total_trades: 15,
                winning_trades: 11,
                total_pnl: dec!(287500),
                win_rate: dec!(73.33),
            },
            NotificationPriority::Low,
        )),
        "risk_alert" => Some((
            NotificationEvent::RiskAlert {
                alert_type: "일일 손실 한도".to_string(),
                message: "일일 손실 한도의 80%에 도달했습니다. 추가 거래에 주의하세요.".to_string(),
                current_value: dec!(80000),
                threshold: dec!(100000),
            },
            NotificationPriority::Critical,
        )),
        "strategy_started" => Some((
            NotificationEvent::StrategyStarted {
                strategy_id: "STR-HAA-001".to_string(),
                strategy_name: "HAA 전략 (미국 ETF)".to_string(),
            },
            NotificationPriority::Normal,
        )),
        "strategy_stopped" => Some((
            NotificationEvent::StrategyStopped {
                strategy_id: "STR-GRID-002".to_string(),
                strategy_name: "그리드 트레이딩 (BTC/USDT)".to_string(),
                reason: "수동 중지 - 사용자 요청".to_string(),
            },
            NotificationPriority::High,
        )),
        "system_error" => Some((
            NotificationEvent::SystemError {
                error_code: "ERR_CONN_001".to_string(),
                message: "거래소 연결이 끊어졌습니다. 자동 재연결을 시도합니다.".to_string(),
            },
            NotificationPriority::Critical,
        )),
        _ => None,
    }
}

/// 템플릿 테스트.
///
/// `POST /api/v1/notifications/test/template`
///
/// 지정된 템플릿 타입으로 샘플 알림을 발송합니다.
#[utoipa::path(
    post,
    path = "/api/v1/notifications/test/template",
    tag = "notifications",
    request_body = TemplateTestRequest,
    responses(
        (status = 200, description = "템플릿 테스트 성공", body = TelegramTestResponse),
        (status = 400, description = "알 수 없는 템플릿 또는 전송 실패", body = TelegramTestResponse)
    )
)]
pub async fn test_template(Json(payload): Json<TemplateTestRequest>) -> impl IntoResponse {
    debug!("Testing template: {}", payload.template_type);

    // 환경 변수에서 설정 읽기
    let bot_token = match std::env::var("TELEGRAM_BOT_TOKEN") {
        Ok(token) if !token.is_empty() => token,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(TelegramTestResponse {
                    success: false,
                    message: "TELEGRAM_BOT_TOKEN 환경 변수가 설정되지 않았습니다.".to_string(),
                }),
            );
        }
    };

    let chat_id = match std::env::var("TELEGRAM_CHAT_ID") {
        Ok(id) if !id.is_empty() => id.trim().to_string(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(TelegramTestResponse {
                    success: false,
                    message: "TELEGRAM_CHAT_ID 환경 변수가 설정되지 않았습니다.".to_string(),
                }),
            );
        }
    };

    // 샘플 이벤트 생성
    let (event, priority) = match create_sample_event(&payload.template_type) {
        Some(e) => e,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(TelegramTestResponse {
                    success: false,
                    message: format!(
                        "알 수 없는 템플릿 타입: {}. 사용 가능: order_filled, position_opened, position_closed, stop_loss, take_profit, daily_summary, risk_alert, strategy_started, strategy_stopped, system_error",
                        payload.template_type
                    ),
                }),
            );
        }
    };

    // 설정 생성 및 발송
    let config = TelegramConfig::new(bot_token, chat_id);
    let sender = TelegramSender::new(config);

    let notification = Notification::new(event).with_priority(priority);

    match sender.send(&notification).await {
        Ok(_) => {
            debug!(
                "Template test message sent successfully: {}",
                payload.template_type
            );
            (
                StatusCode::OK,
                Json(TelegramTestResponse {
                    success: true,
                    message: format!(
                        "'{}' 템플릿 테스트 메시지를 전송했습니다.",
                        payload.template_type
                    ),
                }),
            )
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            error!("Failed to send template test message: {}", error_msg);

            (
                StatusCode::BAD_REQUEST,
                Json(TelegramTestResponse {
                    success: false,
                    message: format!("메시지 전송 실패: {}", error_msg),
                }),
            )
        }
    }
}

/// 모든 템플릿 테스트.
///
/// `POST /api/v1/notifications/test/templates/all`
///
/// 모든 템플릿 타입으로 샘플 알림을 순차적으로 발송합니다.
#[utoipa::path(
    post,
    path = "/api/v1/notifications/test/templates/all",
    tag = "notifications",
    responses(
        (status = 200, description = "모든 템플릿 테스트 성공", body = TelegramTestResponse),
        (status = 206, description = "일부 템플릿 전송 실패", body = TelegramTestResponse),
        (status = 400, description = "환경 변수 미설정", body = TelegramTestResponse)
    )
)]
pub async fn test_all_templates() -> impl IntoResponse {
    debug!("Testing all templates");

    // 환경 변수에서 설정 읽기
    let bot_token = match std::env::var("TELEGRAM_BOT_TOKEN") {
        Ok(token) if !token.is_empty() => token,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(TelegramTestResponse {
                    success: false,
                    message: "TELEGRAM_BOT_TOKEN 환경 변수가 설정되지 않았습니다.".to_string(),
                }),
            );
        }
    };

    let chat_id = match std::env::var("TELEGRAM_CHAT_ID") {
        Ok(id) if !id.is_empty() => id.trim().to_string(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(TelegramTestResponse {
                    success: false,
                    message: "TELEGRAM_CHAT_ID 환경 변수가 설정되지 않았습니다.".to_string(),
                }),
            );
        }
    };

    let config = TelegramConfig::new(bot_token, chat_id);
    let sender = TelegramSender::new(config);

    let template_types = [
        "order_filled",
        "position_opened",
        "position_closed",
        "stop_loss",
        "take_profit",
        "daily_summary",
        "risk_alert",
        "strategy_started",
        "strategy_stopped",
        "system_error",
    ];

    let mut success_count = 0;
    let mut failed_templates = Vec::new();

    for template_type in &template_types {
        if let Some((event, priority)) = create_sample_event(template_type) {
            let notification = Notification::new(event).with_priority(priority);

            match sender.send(&notification).await {
                Ok(_) => {
                    success_count += 1;
                    debug!("Template sent: {}", template_type);
                }
                Err(e) => {
                    error!("Failed to send template {}: {}", template_type, e);
                    failed_templates.push(template_type.to_string());
                }
            }

            // Rate limiting 방지 - 각 메시지 사이에 잠시 대기
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    if failed_templates.is_empty() {
        (
            StatusCode::OK,
            Json(TelegramTestResponse {
                success: true,
                message: format!("모든 {} 템플릿 테스트를 완료했습니다.", success_count),
            }),
        )
    } else {
        (
            StatusCode::PARTIAL_CONTENT,
            Json(TelegramTestResponse {
                success: false,
                message: format!(
                    "{}/{}개 성공. 실패: {}",
                    success_count,
                    template_types.len(),
                    failed_templates.join(", ")
                ),
            }),
        )
    }
}

/// 알림 라우터 생성.
pub fn notifications_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/settings", get(get_notification_settings))
        .route("/templates", get(get_templates))
        .route("/telegram/test", post(test_telegram))
        .route("/telegram/test-env", post(test_telegram_env))
        .route("/telegram/test-template", post(test_template))
        .route("/telegram/test-all-templates", post(test_all_templates))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_test_request_deserialization() {
        let json = r#"{"bot_token": "123:ABC", "chat_id": "12345"}"#;
        let req: TelegramTestRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.bot_token, "123:ABC");
        assert_eq!(req.chat_id, "12345");
    }
}
