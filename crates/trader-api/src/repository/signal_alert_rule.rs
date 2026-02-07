//! 신호 알림 규칙 리포지토리.
//!
//! 알림 규칙 CRUD 및 필터 조건 관리를 제공합니다.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{ApiErrorResponse, ApiResult};
use crate::services::SignalAlertFilter;
use axum::http::StatusCode;
use axum::Json;

/// 신호 알림 규칙 엔티티.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SignalAlertRule {
    /// 규칙 ID
    pub id: Uuid,
    /// 규칙 이름
    pub rule_name: String,
    /// 규칙 설명
    pub description: Option<String>,
    /// 활성화 여부
    pub enabled: bool,
    /// 필터 조건 (JSON)
    pub filter_conditions: JsonValue,
    /// 생성 시각
    pub created_at: DateTime<Utc>,
    /// 수정 시각
    pub updated_at: DateTime<Utc>,
}

impl SignalAlertRule {
    /// filter_conditions를 SignalAlertFilter로 변환.
    pub fn to_filter(&self) -> Result<SignalAlertFilter, serde_json::Error> {
        serde_json::from_value(self.filter_conditions.clone())
    }
}

/// 신호 알림 규칙 생성 요청.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct CreateAlertRuleRequest {
    /// 규칙 이름
    pub rule_name: String,
    /// 규칙 설명
    pub description: Option<String>,
    /// 활성화 여부 (기본 true)
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 필터 조건
    pub filter_conditions: SignalAlertFilter,
}

fn default_enabled() -> bool {
    true
}

/// 신호 알림 규칙 수정 요청.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct UpdateAlertRuleRequest {
    /// 규칙 이름 (선택)
    pub rule_name: Option<String>,
    /// 규칙 설명 (선택)
    pub description: Option<String>,
    /// 활성화 여부 (선택)
    pub enabled: Option<bool>,
    /// 필터 조건 (선택)
    pub filter_conditions: Option<SignalAlertFilter>,
}

/// 신호 알림 규칙 리포지토리.
pub struct SignalAlertRuleRepository {
    pool: PgPool,
}

impl SignalAlertRuleRepository {
    /// 새 리포지토리 생성.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 규칙 생성.
    pub async fn create(&self, req: CreateAlertRuleRequest) -> ApiResult<SignalAlertRule> {
        let filter_json = serde_json::to_value(&req.filter_conditions).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse::new(
                    "SERIALIZATION_ERROR",
                    format!("Failed to serialize filter: {}", e),
                )),
            )
        })?;

        let rule = sqlx::query_as!(
            SignalAlertRule,
            r#"
            INSERT INTO signal_alert_rule (rule_name, description, enabled, filter_conditions)
            VALUES ($1, $2, $3, $4)
            RETURNING id, rule_name, description, enabled, filter_conditions, created_at, updated_at
            "#,
            req.rule_name,
            req.description,
            req.enabled,
            filter_json
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if let Some(db_err) = e.as_database_error() {
                if db_err.is_unique_violation() {
                    return (
                        StatusCode::CONFLICT,
                        Json(ApiErrorResponse::new(
                            "DUPLICATE_RULE",
                            format!("Rule '{}' already exists", req.rule_name),
                        )),
                    );
                }
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DB_ERROR", e.to_string())),
            )
        })?;

        Ok(rule)
    }

    /// 모든 규칙 조회.
    pub async fn list(&self, enabled_only: bool) -> ApiResult<Vec<SignalAlertRule>> {
        let rules = if enabled_only {
            sqlx::query_as!(
                SignalAlertRule,
                r#"
                SELECT id, rule_name, description, enabled, filter_conditions, created_at, updated_at
                FROM signal_alert_rule
                WHERE enabled = true
                ORDER BY created_at DESC
                "#
            )
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as!(
                SignalAlertRule,
                r#"
                SELECT id, rule_name, description, enabled, filter_conditions, created_at, updated_at
                FROM signal_alert_rule
                ORDER BY created_at DESC
                "#
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse::new("DB_ERROR", e.to_string()))
        ))?;

        Ok(rules)
    }

    /// ID로 규칙 조회.
    pub async fn find_by_id(&self, id: Uuid) -> ApiResult<SignalAlertRule> {
        let rule = sqlx::query_as!(
            SignalAlertRule,
            r#"
            SELECT id, rule_name, description, enabled, filter_conditions, created_at, updated_at
            FROM signal_alert_rule
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DB_ERROR", e.to_string())),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse::new(
                    "NOT_FOUND",
                    format!("Rule {} not found", id),
                )),
            )
        })?;

        Ok(rule)
    }

    /// 이름으로 규칙 조회.
    pub async fn find_by_name(&self, name: &str) -> ApiResult<SignalAlertRule> {
        let rule = sqlx::query_as!(
            SignalAlertRule,
            r#"
            SELECT id, rule_name, description, enabled, filter_conditions, created_at, updated_at
            FROM signal_alert_rule
            WHERE rule_name = $1
            "#,
            name
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DB_ERROR", e.to_string())),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse::new(
                    "NOT_FOUND",
                    format!("Rule '{}' not found", name),
                )),
            )
        })?;

        Ok(rule)
    }

    /// 규칙 수정.
    pub async fn update(
        &self,
        id: Uuid,
        req: UpdateAlertRuleRequest,
    ) -> ApiResult<SignalAlertRule> {
        // 기존 규칙 조회
        let existing = self.find_by_id(id).await?;

        // 수정할 필드 준비
        let rule_name = req.rule_name.unwrap_or(existing.rule_name);
        let description = req.description.or(existing.description);
        let enabled = req.enabled.unwrap_or(existing.enabled);
        let filter_json = if let Some(filter) = req.filter_conditions {
            serde_json::to_value(&filter).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ApiErrorResponse::new(
                        "SERIALIZATION_ERROR",
                        format!("Failed to serialize filter: {}", e),
                    )),
                )
            })?
        } else {
            existing.filter_conditions
        };

        // 수정 실행
        let rule = sqlx::query_as!(
            SignalAlertRule,
            r#"
            UPDATE signal_alert_rule
            SET rule_name = $2, description = $3, enabled = $4, filter_conditions = $5
            WHERE id = $1
            RETURNING id, rule_name, description, enabled, filter_conditions, created_at, updated_at
            "#,
            id,
            rule_name,
            description,
            enabled,
            filter_json
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if let Some(db_err) = e.as_database_error() {
                if db_err.is_unique_violation() {
                    return (
                        StatusCode::CONFLICT,
                        Json(ApiErrorResponse::new(
                            "DUPLICATE_RULE",
                            format!("Rule '{}' already exists", rule_name),
                        )),
                    );
                }
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DB_ERROR", e.to_string())),
            )
        })?;

        Ok(rule)
    }

    /// 규칙 삭제.
    pub async fn delete(&self, id: Uuid) -> ApiResult<()> {
        let result = sqlx::query!(
            r#"
            DELETE FROM signal_alert_rule
            WHERE id = $1
            "#,
            id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new("DB_ERROR", e.to_string())),
            )
        })?;

        if result.rows_affected() == 0 {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse::new(
                    "NOT_FOUND",
                    format!("Rule {} not found", id),
                )),
            ));
        }

        Ok(())
    }

    /// 활성화된 모든 규칙의 필터 조회.
    pub async fn get_enabled_filters(&self) -> ApiResult<Vec<SignalAlertFilter>> {
        let rules = self.list(true).await?;

        let filters: Result<Vec<_>, _> = rules.iter().map(|r| r.to_filter()).collect();

        filters.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiErrorResponse::new(
                    "DESERIALIZATION_ERROR",
                    format!("Failed to deserialize filter: {}", e),
                )),
            )
        })
    }
}
