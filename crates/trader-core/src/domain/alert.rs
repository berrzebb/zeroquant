//! 알림 규칙 및 조건 정의.
//!
//! 사용자가 특정 지표 조건에서 알림을 받을 수 있도록 규칙을 정의합니다.
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! use trader_core::domain::{AlertRule, AlertCondition, IndicatorFilter, ComparisonOperator};
//!
//! // RSI가 70 이상일 때 알림
//! let rule = AlertRule::new("rsi_overbought", "RSI 과매수 알림")
//!     .with_condition(AlertCondition::Indicator(IndicatorFilter {
//!         indicator: "rsi".to_string(),
//!         operator: ComparisonOperator::Gte,
//!         value: 70.0,
//!     }))
//!     .with_symbols(vec!["005930".to_string()]);
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 비교 연산자.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOperator {
    /// 같음 (==)
    Eq,
    /// 같지 않음 (!=)
    Ne,
    /// 보다 큼 (>)
    Gt,
    /// 보다 크거나 같음 (>=)
    Gte,
    /// 보다 작음 (<)
    Lt,
    /// 보다 작거나 같음 (<=)
    Lte,
    /// 사이 (between)
    Between,
    /// 크로스 업 (이전 < 기준, 현재 >= 기준)
    CrossAbove,
    /// 크로스 다운 (이전 >= 기준, 현재 < 기준)
    CrossBelow,
}

impl ComparisonOperator {
    /// 두 값을 비교하여 조건 충족 여부 반환.
    ///
    /// `between` 연산자의 경우 `upper_bound`가 필요합니다.
    pub fn evaluate(
        &self,
        current: f64,
        threshold: f64,
        previous: Option<f64>,
        upper_bound: Option<f64>,
    ) -> bool {
        match self {
            Self::Eq => (current - threshold).abs() < f64::EPSILON,
            Self::Ne => (current - threshold).abs() >= f64::EPSILON,
            Self::Gt => current > threshold,
            Self::Gte => current >= threshold,
            Self::Lt => current < threshold,
            Self::Lte => current <= threshold,
            Self::Between => {
                if let Some(upper) = upper_bound {
                    current >= threshold && current <= upper
                } else {
                    false
                }
            }
            Self::CrossAbove => {
                if let Some(prev) = previous {
                    prev < threshold && current >= threshold
                } else {
                    false
                }
            }
            Self::CrossBelow => {
                if let Some(prev) = previous {
                    prev >= threshold && current < threshold
                } else {
                    false
                }
            }
        }
    }
}

/// 지표 필터.
///
/// 특정 지표의 값을 조건과 비교합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct IndicatorFilter {
    /// 지표 이름 (rsi, macd, bb_upper 등)
    pub indicator: String,

    /// 비교 연산자
    pub operator: ComparisonOperator,

    /// 비교 값 (threshold)
    pub value: f64,

    /// 상한값 (Between 연산자용)
    #[serde(default)]
    pub upper_value: Option<f64>,
}

impl IndicatorFilter {
    /// 새 지표 필터 생성.
    pub fn new(indicator: impl Into<String>, operator: ComparisonOperator, value: f64) -> Self {
        Self {
            indicator: indicator.into(),
            operator,
            value,
            upper_value: None,
        }
    }

    /// Between 연산자용 상한값 설정.
    pub fn with_upper_value(mut self, upper: f64) -> Self {
        self.upper_value = Some(upper);
        self
    }

    /// 지표 값과 비교하여 조건 충족 여부 반환.
    ///
    /// # 인자
    ///
    /// * `current` - 현재 지표 값
    /// * `previous` - 이전 지표 값 (크로스 연산자용)
    pub fn evaluate(&self, current: f64, previous: Option<f64>) -> bool {
        self.operator
            .evaluate(current, self.value, previous, self.upper_value)
    }
}

/// 가격 조건.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PriceCondition {
    /// 비교 연산자
    pub operator: ComparisonOperator,

    /// 목표 가격
    pub price: Decimal,

    /// 상한 가격 (Between 연산자용)
    #[serde(default)]
    pub upper_price: Option<Decimal>,
}

/// 알림 조건.
///
/// 하나의 알림 조건을 정의합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AlertCondition {
    /// 지표 기반 조건
    Indicator(IndicatorFilter),

    /// 가격 기반 조건
    Price(PriceCondition),

    /// RouteState 변경 조건
    RouteStateChange {
        /// 목표 상태 (Attack, Armed, Neutral, Wait, Overheat)
        target_state: String,
    },

    /// GlobalScore 조건
    GlobalScore {
        /// 비교 연산자
        operator: ComparisonOperator,
        /// 목표 점수
        threshold: Decimal,
    },

    /// 복합 조건 (AND)
    And(Vec<AlertCondition>),

    /// 복합 조건 (OR)
    Or(Vec<AlertCondition>),
}

/// 알림 규칙 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum AlertRuleStatus {
    /// 활성 상태
    #[default]
    Active,
    /// 비활성 상태
    Inactive,
    /// 트리거됨 (일회성 규칙)
    Triggered,
    /// 만료됨
    Expired,
}

/// 알림 규칙.
///
/// 특정 조건이 충족되면 알림을 발생시키는 규칙입니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AlertRule {
    /// 규칙 ID
    pub id: Uuid,

    /// 사용자 ID
    pub user_id: String,

    /// 규칙 이름
    pub name: String,

    /// 규칙 설명
    #[serde(default)]
    pub description: Option<String>,

    /// 대상 심볼 목록 (비어있으면 모든 심볼)
    #[serde(default)]
    pub symbols: Vec<String>,

    /// 대상 거래소 (비어있으면 모든 거래소)
    #[serde(default)]
    pub exchange: Option<String>,

    /// 알림 조건
    pub conditions: AlertCondition,

    /// 규칙 상태
    #[serde(default)]
    pub status: AlertRuleStatus,

    /// 반복 여부 (false면 한 번만 트리거)
    #[serde(default)]
    pub repeatable: bool,

    /// 재알림 대기 시간 (초, 반복 규칙용)
    #[serde(default)]
    pub cooldown_seconds: Option<i64>,

    /// 마지막 트리거 시각
    #[serde(default)]
    pub last_triggered_at: Option<DateTime<Utc>>,

    /// 생성 시각
    pub created_at: DateTime<Utc>,

    /// 수정 시각
    pub updated_at: DateTime<Utc>,

    /// 만료 시각
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,

    /// 추가 메타데이터
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl AlertRule {
    /// 새 알림 규칙 생성.
    pub fn new(name: impl Into<String>, user_id: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id: user_id.into(),
            name: name.into(),
            description: None,
            symbols: Vec::new(),
            exchange: None,
            conditions: AlertCondition::And(Vec::new()),
            status: AlertRuleStatus::Active,
            repeatable: false,
            cooldown_seconds: None,
            last_triggered_at: None,
            created_at: now,
            updated_at: now,
            expires_at: None,
            metadata: HashMap::new(),
        }
    }

    /// 설명 설정.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 대상 심볼 설정.
    pub fn with_symbols(mut self, symbols: Vec<String>) -> Self {
        self.symbols = symbols;
        self
    }

    /// 거래소 설정.
    pub fn with_exchange(mut self, exchange: impl Into<String>) -> Self {
        self.exchange = Some(exchange.into());
        self
    }

    /// 조건 설정.
    pub fn with_condition(mut self, condition: AlertCondition) -> Self {
        self.conditions = condition;
        self
    }

    /// 반복 설정.
    pub fn with_repeatable(mut self, repeatable: bool, cooldown_seconds: Option<i64>) -> Self {
        self.repeatable = repeatable;
        self.cooldown_seconds = cooldown_seconds;
        self
    }

    /// 만료 시각 설정.
    pub fn with_expiration(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// 규칙이 활성 상태인지 확인.
    pub fn is_active(&self) -> bool {
        if self.status != AlertRuleStatus::Active {
            return false;
        }

        // 만료 확인
        if let Some(expires_at) = self.expires_at {
            if Utc::now() > expires_at {
                return false;
            }
        }

        true
    }

    /// 쿨다운 중인지 확인.
    pub fn is_in_cooldown(&self) -> bool {
        if !self.repeatable {
            return false;
        }

        if let (Some(last_triggered), Some(cooldown)) =
            (self.last_triggered_at, self.cooldown_seconds)
        {
            let elapsed = Utc::now().signed_duration_since(last_triggered);
            return elapsed.num_seconds() < cooldown;
        }

        false
    }

    /// 트리거 기록.
    pub fn record_trigger(&mut self) {
        self.last_triggered_at = Some(Utc::now());
        self.updated_at = Utc::now();

        if !self.repeatable {
            self.status = AlertRuleStatus::Triggered;
        }
    }
}

/// 알림 이벤트.
///
/// 알림 규칙이 트리거되었을 때 생성됩니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AlertEvent {
    /// 이벤트 ID
    pub id: Uuid,

    /// 규칙 ID
    pub rule_id: Uuid,

    /// 사용자 ID
    pub user_id: String,

    /// 심볼
    pub symbol: String,

    /// 거래소
    pub exchange: String,

    /// 트리거 시각
    pub triggered_at: DateTime<Utc>,

    /// 트리거된 조건 요약
    pub condition_summary: String,

    /// 현재 값 (지표/가격)
    pub current_value: Option<f64>,

    /// 목표 값
    pub threshold_value: Option<f64>,

    /// 읽음 여부
    #[serde(default)]
    pub is_read: bool,

    /// 추가 메타데이터
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl AlertEvent {
    /// 새 알림 이벤트 생성.
    pub fn new(
        rule_id: Uuid,
        user_id: impl Into<String>,
        symbol: impl Into<String>,
        exchange: impl Into<String>,
        condition_summary: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            rule_id,
            user_id: user_id.into(),
            symbol: symbol.into(),
            exchange: exchange.into(),
            triggered_at: Utc::now(),
            condition_summary: condition_summary.into(),
            current_value: None,
            threshold_value: None,
            is_read: false,
            metadata: HashMap::new(),
        }
    }

    /// 현재/목표 값 설정.
    pub fn with_values(mut self, current: f64, threshold: f64) -> Self {
        self.current_value = Some(current);
        self.threshold_value = Some(threshold);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comparison_operators() {
        assert!(ComparisonOperator::Gt.evaluate(75.0, 70.0, None, None));
        assert!(ComparisonOperator::Gte.evaluate(70.0, 70.0, None, None));
        assert!(!ComparisonOperator::Lt.evaluate(75.0, 70.0, None, None));

        // Between
        assert!(ComparisonOperator::Between.evaluate(75.0, 70.0, None, Some(80.0)));
        assert!(!ComparisonOperator::Between.evaluate(85.0, 70.0, None, Some(80.0)));

        // Cross
        assert!(ComparisonOperator::CrossAbove.evaluate(71.0, 70.0, Some(69.0), None));
        assert!(!ComparisonOperator::CrossAbove.evaluate(71.0, 70.0, Some(71.0), None));
    }

    #[test]
    fn test_indicator_filter() {
        let filter = IndicatorFilter::new("rsi", ComparisonOperator::Gte, 70.0);
        assert!(filter.evaluate(75.0, None));
        assert!(!filter.evaluate(65.0, None));
    }

    #[test]
    fn test_alert_rule_creation() {
        let rule = AlertRule::new("rsi_overbought", "user_123")
            .with_description("RSI 과매수 알림")
            .with_symbols(vec!["005930".to_string()])
            .with_condition(AlertCondition::Indicator(IndicatorFilter::new(
                "rsi",
                ComparisonOperator::Gte,
                70.0,
            )));

        assert_eq!(rule.name, "rsi_overbought");
        assert!(rule.is_active());
        assert!(!rule.is_in_cooldown());
    }

    #[test]
    fn test_alert_rule_cooldown() {
        let mut rule = AlertRule::new("test", "user_123").with_repeatable(true, Some(60));

        assert!(!rule.is_in_cooldown());

        rule.record_trigger();
        assert!(rule.is_in_cooldown()); // 60초 쿨다운
    }
}
