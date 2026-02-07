//! 알림 타입 및 trait 정의.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use trader_core::SignalMarker;

/// 알림 우선순위 레벨.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum NotificationPriority {
    /// 낮은 우선순위 (정보성)
    Low,
    /// 일반 우선순위 (일반 업데이트)
    #[default]
    Normal,
    /// 높은 우선순위 (중요 이벤트)
    High,
    /// 긴급 우선순위 (즉시 대응 필요)
    Critical,
}


/// 알림 이벤트 타입.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NotificationEvent {
    /// 주문 체결 알림
    OrderFilled {
        symbol: String,
        side: String,
        quantity: Decimal,
        price: Decimal,
        order_id: String,
    },
    /// 포지션 진입
    PositionOpened {
        symbol: String,
        side: String,
        quantity: Decimal,
        entry_price: Decimal,
    },
    /// 포지션 청산
    PositionClosed {
        symbol: String,
        side: String,
        quantity: Decimal,
        entry_price: Decimal,
        exit_price: Decimal,
        pnl: Decimal,
        pnl_percent: Decimal,
    },
    /// 손절 발동
    StopLossTriggered {
        symbol: String,
        quantity: Decimal,
        trigger_price: Decimal,
        loss: Decimal,
    },
    /// 익절 발동
    TakeProfitTriggered {
        symbol: String,
        quantity: Decimal,
        trigger_price: Decimal,
        profit: Decimal,
    },
    /// 일일 요약
    DailySummary {
        date: String,
        total_trades: u32,
        winning_trades: u32,
        total_pnl: Decimal,
        win_rate: Decimal,
    },
    /// 리스크 경고
    RiskAlert {
        alert_type: String,
        message: String,
        current_value: Decimal,
        threshold: Decimal,
    },
    /// 전략 시작
    StrategyStarted {
        strategy_id: String,
        strategy_name: String,
    },
    /// 전략 중지
    StrategyStopped {
        strategy_id: String,
        strategy_name: String,
        reason: String,
    },
    /// 시스템 오류
    SystemError { error_code: String, message: String },
    /// 신호 마커 알림 (백테스트/실거래 신호)
    SignalAlert {
        signal_type: String,
        symbol: String,
        side: Option<String>,
        price: Decimal,
        strength: f64,
        reason: String,
        strategy_name: String,
        indicators: serde_json::Value,
    },
    /// 사용자 정의 알림
    Custom { title: String, message: String },
    /// RouteState 변경 알림 (Attack 진입 등)
    RouteStateChanged {
        symbol: String,
        symbol_name: Option<String>,
        previous_state: String,
        new_state: String,
        /// 매크로 환경 정보 (선택)
        macro_risk: Option<String>,
        macro_summary: Option<String>,
    },
    /// 매크로 환경 경고 알림
    MacroAlert {
        risk_level: String,
        usd_krw: String,
        usd_change_pct: String,
        nasdaq_change_pct: String,
        recommendation: String,
    },
    /// 시장 온도 알림
    MarketBreadthAlert {
        temperature: String,
        all_ratio: String,
        kospi_ratio: String,
        kosdaq_ratio: String,
        recommendation: String,
    },
}

impl From<SignalMarker> for NotificationEvent {
    /// SignalMarker를 알림 이벤트로 변환합니다.
    ///
    /// SignalMarker의 reason 필드가 그대로 NotificationEvent::SignalAlert의 reason으로 전달됩니다.
    fn from(marker: SignalMarker) -> Self {
        NotificationEvent::SignalAlert {
            signal_type: format!("{:?}", marker.signal_type),
            symbol: marker.ticker,
            side: marker.side.map(|s| format!("{:?}", s)),
            price: marker.price,
            strength: marker.strength,
            reason: marker.reason,
            strategy_name: marker.strategy_name,
            indicators: serde_json::to_value(&marker.indicators).unwrap_or_default(),
        }
    }
}

/// 알림 메시지.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// 고유 알림 ID
    pub id: String,
    /// 알림 이벤트
    pub event: NotificationEvent,
    /// 우선순위 레벨
    pub priority: NotificationPriority,
    /// 타임스탬프
    pub timestamp: DateTime<Utc>,
    /// 추가 메타데이터
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl Notification {
    /// 새 알림을 생성합니다.
    pub fn new(event: NotificationEvent) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            event,
            priority: NotificationPriority::Normal,
            timestamp: Utc::now(),
            metadata: serde_json::Value::Null,
        }
    }

    /// 우선순위 레벨을 설정합니다.
    pub fn with_priority(mut self, priority: NotificationPriority) -> Self {
        self.priority = priority;
        self
    }

    /// 메타데이터를 설정합니다.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// 알림 작업용 Result 타입.
pub type NotificationResult<T> = Result<T, NotificationError>;

/// 알림 에러.
#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[error("알림 전송 실패: {0}")]
    SendFailed(String),

    #[error("잘못된 설정: {0}")]
    InvalidConfig(String),

    #[error("요청 한도 초과: {0}초 후 재시도")]
    RateLimited(u64),

    #[error("네트워크 에러: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("직렬화 에러: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// 알림 전송기 trait.
#[async_trait]
pub trait NotificationSender: Send + Sync {
    /// 알림을 전송합니다.
    async fn send(&self, notification: &Notification) -> NotificationResult<()>;

    /// 전송기가 활성화되어 있는지 확인합니다.
    fn is_enabled(&self) -> bool;

    /// 전송기 이름을 반환합니다.
    fn name(&self) -> &str;
}
