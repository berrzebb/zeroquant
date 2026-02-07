//! 전략의 트레이딩 시그널.
//!
//! 이 모듈은 전략이 생성하는 매매 신호 관련 타입을 정의합니다:
//! - `SignalType` - 신호 유형 (진입, 청산 등)
//! - `Signal` - 매매 신호 엔티티
//! - `SignalValidation` - 신호 검증 결과

use crate::domain::{RouteState, Side};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 수행할 액션의 종류를 나타내는 신호 유형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa-support", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "ts-rs-support", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-rs-support", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    /// 새 포지션 진입
    Entry,
    /// 기존 포지션 청산
    Exit,
    /// 알림 (실행하지 않음)
    Alert,
    /// 기존 포지션에 추가 (물타기)
    AddToPosition,
    /// 기존 포지션 축소 (부분 청산)
    ReducePosition,
    /// 스케일 인/아웃
    Scale,
}

impl std::fmt::Display for SignalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalType::Entry => write!(f, "ENTRY"),
            SignalType::Exit => write!(f, "EXIT"),
            SignalType::Alert => write!(f, "ALERT"),
            SignalType::AddToPosition => write!(f, "ADD_TO_POSITION"),
            SignalType::ReducePosition => write!(f, "REDUCE_POSITION"),
            SignalType::Scale => write!(f, "SCALE"),
        }
    }
}

/// 전략이 생성한 트레이딩 신호.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    /// 고유 신호 ID
    pub id: Uuid,
    /// 이 신호를 생성한 전략
    pub strategy_id: String,
    /// 거래 ticker
    pub ticker: String,
    /// 신호 방향 (매수/매도)
    pub side: Side,
    /// 신호 유형
    pub signal_type: SignalType,
    /// 신호 강도 (0.0 ~ 1.0)
    pub strength: f64,
    /// 제안 진입 가격 (선택)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_price: Option<rust_decimal::Decimal>,
    /// 제안 손절가 (선택)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss: Option<rust_decimal::Decimal>,
    /// 제안 익절가 (선택)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit: Option<rust_decimal::Decimal>,
    /// 신호 생성 타임스탬프
    pub timestamp: DateTime<Utc>,
    /// 추가 메타데이터
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// 포지션 ID (스프레드/그리드 전략용)
    /// None이면 ticker를 포지션 키로 사용 (기존 동작)
    /// Some이면 해당 ID로 독립적인 포지션 관리
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_id: Option<String>,
    /// 그룹 ID (관련 포지션들을 묶는 상위 식별자)
    /// 예: 그리드 세션, 분할매수 세션, 리밸런싱 세션
    /// 그룹 단위 청산, 손익 추적에 사용
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
}

impl Signal {
    /// 새 신호를 생성합니다.
    pub fn new(
        strategy_id: impl Into<String>,
        ticker: String,
        side: Side,
        signal_type: SignalType,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            strategy_id: strategy_id.into(),
            ticker,
            side,
            signal_type,
            strength: 1.0,
            suggested_price: None,
            stop_loss: None,
            take_profit: None,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            position_id: None,
            group_id: None,
        }
    }

    /// 진입 신호를 생성합니다.
    pub fn entry(strategy_id: impl Into<String>, ticker: String, side: Side) -> Self {
        Self::new(strategy_id, ticker, side, SignalType::Entry)
    }

    /// 청산 신호를 생성합니다.
    pub fn exit(strategy_id: impl Into<String>, ticker: String, side: Side) -> Self {
        Self::new(strategy_id, ticker, side, SignalType::Exit)
    }

    /// 신호 강도를 설정합니다.
    pub fn with_strength(mut self, strength: f64) -> Self {
        self.strength = strength.clamp(0.0, 1.0);
        self
    }

    /// 제안 가격 수준을 설정합니다.
    pub fn with_prices(
        mut self,
        entry: Option<rust_decimal::Decimal>,
        stop_loss: Option<rust_decimal::Decimal>,
        take_profit: Option<rust_decimal::Decimal>,
    ) -> Self {
        self.suggested_price = entry;
        self.stop_loss = stop_loss;
        self.take_profit = take_profit;
        self
    }

    /// 메타데이터를 추가합니다.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// 강한 신호인지 확인합니다 (강도 >= 0.7).
    pub fn is_strong(&self) -> bool {
        self.strength >= 0.7
    }

    /// 진입 신호인지 확인합니다.
    pub fn is_entry(&self) -> bool {
        self.signal_type == SignalType::Entry
    }

    /// 청산 신호인지 확인합니다.
    pub fn is_exit(&self) -> bool {
        self.signal_type == SignalType::Exit
    }

    /// 포지션 ID를 설정합니다 (스프레드/그리드 전략용).
    ///
    /// 같은 position_id를 가진 Entry/Exit 신호는 페어로 관리됩니다.
    /// 예: 그리드 레벨 3의 매수/매도가 "grid_L3" ID를 공유
    pub fn with_position_id(mut self, position_id: impl Into<String>) -> Self {
        self.position_id = Some(position_id.into());
        self
    }

    /// Executor에서 포지션을 식별하는 키를 반환합니다.
    ///
    /// position_id가 있으면 해당 ID 사용, 없으면 ticker 사용 (기존 동작)
    pub fn position_key(&self) -> String {
        self.position_id
            .clone()
            .unwrap_or_else(|| self.ticker.clone())
    }

    /// 그룹 ID를 설정합니다 (관련 포지션 묶기).
    ///
    /// 그룹 단위 청산이나 손익 추적에 사용됩니다.
    /// 예: 그리드 세션 전체, 분할매수 세션 전체
    pub fn with_group_id(mut self, group_id: impl Into<String>) -> Self {
        self.group_id = Some(group_id.into());
        self
    }
}

/// 신호 검증 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalValidation {
    /// 신호 유효 여부
    pub is_valid: bool,
    /// 검증 메시지
    pub messages: Vec<String>,
    /// 수정된 신호 (조정이 이루어진 경우)
    pub modified_signal: Option<Signal>,
}

impl SignalValidation {
    /// 유효한 결과를 생성합니다.
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            messages: vec![],
            modified_signal: None,
        }
    }

    /// 무효한 결과를 생성합니다.
    pub fn invalid(reason: impl Into<String>) -> Self {
        Self {
            is_valid: false,
            messages: vec![reason.into()],
            modified_signal: None,
        }
    }

    /// 메시지를 추가합니다.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.messages.push(message.into());
        self
    }

    /// 수정된 신호를 설정합니다.
    pub fn with_modified_signal(mut self, signal: Signal) -> Self {
        self.modified_signal = Some(signal);
        self
    }
}

// ==================== SignalMarker (신호 마커) ====================

/// 기술 신호 마커 - 캔들 차트에 표시할 신호 정보.
///
/// Signal과 달리 SignalMarker는 백테스트와 실거래에서 발생한
/// 신호를 저장하고 분석하기 위한 확장된 정보를 포함합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa-support", derive(utoipa::ToSchema))]
pub struct SignalMarker {
    /// 고유 ID
    pub id: Uuid,
    /// 거래 심볼
    pub ticker: String,
    /// 신호 발생 시각
    pub timestamp: DateTime<Utc>,
    /// 신호 유형 (Entry, Exit, Alert 등)
    pub signal_type: SignalType,
    /// 신호 방향 (매수/매도)
    pub side: Option<Side>,
    /// 신호 발생 시점 가격
    pub price: Decimal,
    /// 신호 강도 (0.0 ~ 1.0)
    pub strength: f64,

    /// 신호 생성에 사용된 지표 값들
    pub indicators: SignalIndicators,

    /// 신호 생성 이유 (사람이 읽을 수 있는 형태)
    pub reason: String,

    /// 전략 ID
    pub strategy_id: String,
    /// 전략 이름
    pub strategy_name: String,

    /// 실행 여부 (백테스트에서 실제 체결되었는지)
    pub executed: bool,

    /// 메타데이터 (확장용)
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl SignalMarker {
    /// 새 신호 마커 생성.
    pub fn new(
        ticker: String,
        timestamp: DateTime<Utc>,
        signal_type: SignalType,
        price: Decimal,
        strategy_id: impl Into<String>,
        strategy_name: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            ticker,
            timestamp,
            signal_type,
            side: None,
            price,
            strength: 0.0,
            indicators: SignalIndicators::default(),
            reason: String::new(),
            strategy_id: strategy_id.into(),
            strategy_name: strategy_name.into(),
            executed: false,
            metadata: HashMap::new(),
        }
    }

    /// Signal로부터 SignalMarker 생성.
    ///
    /// Signal의 metadata에서 reason, variant 등의 정보를 추출하여
    /// 전략의 논리적 목적이 드러나는 reason 문자열을 생성합니다.
    pub fn from_signal(
        signal: &Signal,
        price: Decimal,
        timestamp: DateTime<Utc>,
        strategy_name: impl Into<String>,
    ) -> Self {
        // metadata에서 reason 추출 (전략별 논리적 목적)
        let reason = Self::build_reason(signal);

        Self {
            id: Uuid::new_v4(),
            ticker: signal.ticker.clone(),
            timestamp,
            signal_type: signal.signal_type,
            side: Some(signal.side),
            price,
            strength: signal.strength,
            indicators: SignalIndicators::default(),
            reason,
            strategy_id: signal.strategy_id.clone(),
            strategy_name: strategy_name.into(),
            executed: false,
            metadata: signal.metadata.clone(),
        }
    }

    /// Signal의 metadata에서 reason 문자열을 생성합니다.
    ///
    /// 우선순위:
    /// 1. metadata["reason"] (명시적 이유)
    /// 2. variant + signal_type 조합 (전략 변형 정보)
    /// 3. signal_type 기본값
    fn build_reason(signal: &Signal) -> String {
        // 1. 명시적 reason이 있으면 사용
        if let Some(reason) = signal.metadata.get("reason").and_then(|v| v.as_str()) {
            return Self::format_reason(reason, signal);
        }

        // 2. variant 정보가 있으면 조합
        let variant = signal
            .metadata
            .get("variant")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let level = signal
            .metadata
            .get("level")
            .and_then(|v| v.as_i64());

        // 신호 유형별 기본 메시지 생성
        let base_msg = match signal.signal_type {
            SignalType::Entry => "진입",
            SignalType::Exit => "청산",
            SignalType::AddToPosition => "추가 매수",
            SignalType::ReducePosition => "부분 청산",
            SignalType::Scale => "규모 조절",
            SignalType::Alert => "알림",
        };

        // 방향 문자열
        let side_str = match signal.side {
            Side::Buy => "매수",
            Side::Sell => "매도",
        };

        // variant가 있으면 조합
        if !variant.is_empty() {
            let variant_name = Self::format_variant(variant);
            if let Some(lvl) = level {
                format!("{} 레벨 {} {} ({})", variant_name, lvl, base_msg, side_str)
            } else {
                format!("{} {} ({})", variant_name, base_msg, side_str)
            }
        } else {
            format!("{} ({})", base_msg, side_str)
        }
    }

    /// reason 코드를 사람이 읽기 쉬운 형태로 변환합니다.
    fn format_reason(reason: &str, signal: &Signal) -> String {
        let side_str = match signal.side {
            Side::Buy => "매수",
            Side::Sell => "매도",
        };

        match reason {
            "stop_loss" => format!("손절 청산 ({})", side_str),
            "take_profit" => format!("익절 청산 ({})", side_str),
            "rsi_overbought" => format!("RSI 과매수 청산 ({})", side_str),
            "rsi_oversold" => format!("RSI 과매도 진입 ({})", side_str),
            "rsi_neutral" => format!("RSI 중립점 청산 ({})", side_str),
            "bb_lower_touch" => format!("볼린저 하단 터치 진입 ({})", side_str),
            "bb_upper_touch" => format!("볼린저 상단 터치 청산 ({})", side_str),
            "grid_buy" => format!("그리드 매수 ({})", side_str),
            "grid_sell" => format!("그리드 매도 ({})", side_str),
            "target_reached" => format!("목표가 도달 ({})", side_str),
            "rebalance" => format!("리밸런싱 ({})", side_str),
            "momentum" => format!("모멘텀 ({})", side_str),
            _ => format!("{} ({})", reason, side_str),
        }
    }

    /// variant 코드를 사람이 읽기 쉬운 형태로 변환합니다.
    fn format_variant(variant: &str) -> &str {
        match variant {
            "rsi" => "RSI",
            "bollinger" => "볼린저",
            "grid" => "그리드",
            "magic_split" => "매직 분할",
            "volatility_breakout" => "변동성 돌파",
            "sma_crossover" => "SMA 교차",
            "sector_momentum" => "섹터 모멘텀",
            _ => variant,
        }
    }

    /// 신호 방향 설정.
    pub fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// 신호 강도 설정.
    pub fn with_strength(mut self, strength: f64) -> Self {
        self.strength = strength.clamp(0.0, 1.0);
        self
    }

    /// 지표 정보 설정.
    pub fn with_indicators(mut self, indicators: SignalIndicators) -> Self {
        self.indicators = indicators;
        self
    }

    /// 신호 이유 설정.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = reason.into();
        self
    }

    /// 실행 여부 설정.
    pub fn with_executed(mut self, executed: bool) -> Self {
        self.executed = executed;
        self
    }

    /// 메타데이터 추가.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// 강한 신호인지 확인 (강도 >= 0.8).
    pub fn is_strong(&self) -> bool {
        self.strength >= 0.8
    }

    /// 진입 신호인지 확인.
    pub fn is_entry(&self) -> bool {
        self.signal_type == SignalType::Entry
    }

    /// 청산 신호인지 확인.
    pub fn is_exit(&self) -> bool {
        self.signal_type == SignalType::Exit
    }
}

/// 신호 생성에 사용된 기술적 지표 값들.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa-support", derive(utoipa::ToSchema))]
pub struct SignalIndicators {
    // ===== 추세 지표 =====
    /// SMA (단기)
    pub sma_short: Option<Decimal>,
    /// SMA (장기)
    pub sma_long: Option<Decimal>,
    /// EMA (단기)
    pub ema_short: Option<Decimal>,
    /// EMA (장기)
    pub ema_long: Option<Decimal>,

    // ===== 모멘텀 지표 =====
    /// RSI (14일)
    pub rsi: Option<f64>,
    /// MACD
    pub macd: Option<Decimal>,
    /// MACD 시그널
    pub macd_signal: Option<Decimal>,
    /// MACD 히스토그램
    pub macd_histogram: Option<Decimal>,

    // ===== 변동성 지표 =====
    /// 볼린저 밴드 상단
    pub bb_upper: Option<Decimal>,
    /// 볼린저 밴드 중간
    pub bb_middle: Option<Decimal>,
    /// 볼린저 밴드 하단
    pub bb_lower: Option<Decimal>,
    /// ATR (Average True Range)
    pub atr: Option<Decimal>,

    // ===== TTM Squeeze =====
    /// Squeeze 상태 (압축 중)
    pub squeeze_on: Option<bool>,
    /// Squeeze 모멘텀
    pub squeeze_momentum: Option<Decimal>,

    // ===== 구조적 피처 =====
    /// RouteState (매매 단계)
    pub route_state: Option<RouteState>,
    /// 박스권 내 위치 (0.0 ~ 1.0)
    pub range_pos: Option<f64>,
    /// 거래량 품질
    pub vol_quality: Option<f64>,
    /// 돌파 점수
    pub breakout_score: Option<f64>,
}

impl SignalIndicators {
    /// 빈 지표 정보 생성.
    pub fn new() -> Self {
        Self::default()
    }

    /// RSI 설정.
    pub fn with_rsi(mut self, rsi: f64) -> Self {
        self.rsi = Some(rsi);
        self
    }

    /// MACD 설정.
    pub fn with_macd(mut self, macd: Decimal, signal: Decimal, histogram: Decimal) -> Self {
        self.macd = Some(macd);
        self.macd_signal = Some(signal);
        self.macd_histogram = Some(histogram);
        self
    }

    /// 볼린저 밴드 설정.
    pub fn with_bollinger_bands(mut self, upper: Decimal, middle: Decimal, lower: Decimal) -> Self {
        self.bb_upper = Some(upper);
        self.bb_middle = Some(middle);
        self.bb_lower = Some(lower);
        self
    }

    /// RouteState 설정.
    pub fn with_route_state(mut self, state: RouteState) -> Self {
        self.route_state = Some(state);
        self
    }

    /// TTM Squeeze 설정.
    pub fn with_squeeze(mut self, on: bool, momentum: Decimal) -> Self {
        self.squeeze_on = Some(on);
        self.squeeze_momentum = Some(momentum);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_creation() {
        let symbol = "BTC/USDT".to_string();
        let signal = Signal::entry("grid_trading", symbol, Side::Buy)
            .with_strength(0.85)
            .with_metadata("reason", serde_json::json!("grid_level_hit"));

        assert_eq!(signal.strategy_id, "grid_trading");
        assert_eq!(signal.signal_type, SignalType::Entry);
        assert_eq!(signal.strength, 0.85);
        assert!(signal.is_strong());
        assert!(signal.is_entry());
    }

    #[test]
    fn test_signal_strength_clamping() {
        let symbol = "ETH/USDT".to_string();
        let signal = Signal::exit("rsi_strategy", symbol, Side::Sell).with_strength(1.5);

        assert_eq!(signal.strength, 1.0);
    }

    #[test]
    fn test_signal_marker_creation() {
        use rust_decimal_macros::dec;

        let symbol = "BTC/USDT".to_string();
        let marker = SignalMarker::new(
            symbol,
            Utc::now(),
            SignalType::Entry,
            dec!(50000),
            "rsi_strategy",
            "RSI 평균회귀",
        )
        .with_side(Side::Buy)
        .with_strength(0.9)
        .with_reason("RSI 과매도 (25)")
        .with_indicators(SignalIndicators::new().with_rsi(25.0));

        assert!(marker.is_strong());
        assert!(marker.is_entry());
        assert_eq!(marker.reason, "RSI 과매도 (25)");
        assert_eq!(marker.indicators.rsi, Some(25.0));
    }
}
