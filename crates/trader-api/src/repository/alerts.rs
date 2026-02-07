//! 알림 히스토리 Repository
//!
//! 발송된 알림 기록을 관리합니다.
//!
//! # 주요 기능
//! - 알림 생성 및 상태 업데이트
//! - 알림 히스토리 조회 (필터링, 페이징)
//! - 알림 확인(acknowledge) 처리
//! - 통계 조회

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use tracing::{debug, info};
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

// ================================================================================================
// Enums
// ================================================================================================

/// 알림 유형
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "alerts/")]
#[serde(rename_all = "UPPERCASE")]
pub enum AlertType {
    /// 신호 알림 (전략에서 발생)
    Signal,
    /// 시스템 알림 (서버 상태, 스케줄 등)
    System,
    /// 오류 알림 (장애, 예외 등)
    Error,
}

impl std::fmt::Display for AlertType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertType::Signal => write!(f, "SIGNAL"),
            AlertType::System => write!(f, "SYSTEM"),
            AlertType::Error => write!(f, "ERROR"),
        }
    }
}

impl std::str::FromStr for AlertType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "SIGNAL" => Ok(AlertType::Signal),
            "SYSTEM" => Ok(AlertType::System),
            "ERROR" => Ok(AlertType::Error),
            _ => Err(format!("Invalid alert type: {}", s)),
        }
    }
}

/// 알림 채널
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "alerts/")]
#[serde(rename_all = "UPPERCASE")]
pub enum AlertChannel {
    Telegram,
    Email,
    Discord,
    Slack,
    Webhook,
    Sms,
}

impl std::fmt::Display for AlertChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertChannel::Telegram => write!(f, "TELEGRAM"),
            AlertChannel::Email => write!(f, "EMAIL"),
            AlertChannel::Discord => write!(f, "DISCORD"),
            AlertChannel::Slack => write!(f, "SLACK"),
            AlertChannel::Webhook => write!(f, "WEBHOOK"),
            AlertChannel::Sms => write!(f, "SMS"),
        }
    }
}

impl std::str::FromStr for AlertChannel {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "TELEGRAM" => Ok(AlertChannel::Telegram),
            "EMAIL" => Ok(AlertChannel::Email),
            "DISCORD" => Ok(AlertChannel::Discord),
            "SLACK" => Ok(AlertChannel::Slack),
            "WEBHOOK" => Ok(AlertChannel::Webhook),
            "SMS" => Ok(AlertChannel::Sms),
            _ => Err(format!("Invalid alert channel: {}", s)),
        }
    }
}

/// 알림 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "alerts/")]
#[serde(rename_all = "UPPERCASE")]
pub enum AlertStatus {
    /// 발송 대기
    Pending,
    /// 발송 완료
    Sent,
    /// 발송 실패
    Failed,
    /// 사용자 확인
    Acknowledged,
}

impl std::fmt::Display for AlertStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertStatus::Pending => write!(f, "PENDING"),
            AlertStatus::Sent => write!(f, "SENT"),
            AlertStatus::Failed => write!(f, "FAILED"),
            AlertStatus::Acknowledged => write!(f, "ACKNOWLEDGED"),
        }
    }
}

impl std::str::FromStr for AlertStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PENDING" => Ok(AlertStatus::Pending),
            "SENT" => Ok(AlertStatus::Sent),
            "FAILED" => Ok(AlertStatus::Failed),
            "ACKNOWLEDGED" => Ok(AlertStatus::Acknowledged),
            _ => Err(format!("Invalid alert status: {}", s)),
        }
    }
}

// ================================================================================================
// Entities
// ================================================================================================

/// 알림 히스토리 엔티티
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema, TS)]
#[ts(export, export_to = "alerts/")]
pub struct AlertHistory {
    pub id: Uuid,
    pub rule_id: Option<Uuid>,
    pub signal_marker_id: Option<Uuid>,
    pub alert_type: String,
    pub channel: String,
    pub status: String,
    pub title: String,
    pub message: String,
    #[ts(type = "Record<string, unknown>")]
    pub metadata: JsonValue,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub created_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub acknowledged_by: Option<String>,
}

/// 알림 생성 요청
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateAlertRequest {
    pub rule_id: Option<Uuid>,
    pub signal_marker_id: Option<Uuid>,
    #[serde(default = "default_alert_type")]
    pub alert_type: String,
    #[serde(default = "default_channel")]
    pub channel: String,
    pub title: String,
    pub message: String,
    #[serde(default)]
    pub metadata: Option<JsonValue>,
}

fn default_alert_type() -> String {
    "SIGNAL".to_string()
}

fn default_channel() -> String {
    "TELEGRAM".to_string()
}

/// 알림 조회 필터
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct AlertFilter {
    pub alert_type: Option<String>,
    pub channel: Option<String>,
    pub status: Option<String>,
    pub rule_id: Option<Uuid>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    #[serde(default = "default_limit")]
    pub limit: i32,
    #[serde(default)]
    pub offset: i32,
}

fn default_limit() -> i32 {
    50
}

/// 알림 통계
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema, TS)]
#[ts(export, export_to = "alerts/")]
pub struct AlertStats {
    pub total_count: i64,
    pub pending_count: i64,
    pub sent_count: i64,
    pub failed_count: i64,
    pub acknowledged_count: i64,
}

/// 알림 히스토리 응답
#[derive(Debug, Clone, Serialize, ToSchema, TS)]
#[ts(export, export_to = "alerts/")]
pub struct AlertHistoryResponse {
    pub alerts: Vec<AlertHistory>,
    pub total: i64,
    pub limit: i32,
    pub offset: i32,
}

// ================================================================================================
// Repository
// ================================================================================================

/// 알림 히스토리 Repository
pub struct AlertsRepository;

impl AlertsRepository {
    /// 알림 생성
    pub async fn create(
        pool: &PgPool,
        request: &CreateAlertRequest,
    ) -> Result<AlertHistory, sqlx::Error> {
        let metadata = request.metadata.clone().unwrap_or(JsonValue::Object(Default::default()));

        let alert = sqlx::query_as::<_, AlertHistory>(
            r#"
            INSERT INTO alert_history (
                rule_id, signal_marker_id, alert_type, channel,
                status, title, message, metadata
            )
            VALUES ($1, $2, $3, $4, 'PENDING', $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(&request.rule_id)
        .bind(&request.signal_marker_id)
        .bind(&request.alert_type)
        .bind(&request.channel)
        .bind(&request.title)
        .bind(&request.message)
        .bind(&metadata)
        .fetch_one(pool)
        .await?;

        debug!(alert_id = %alert.id, "알림 생성 완료");
        Ok(alert)
    }

    /// 알림 상태 업데이트 (발송 완료)
    pub async fn mark_sent(pool: &PgPool, id: Uuid) -> Result<AlertHistory, sqlx::Error> {
        let alert = sqlx::query_as::<_, AlertHistory>(
            r#"
            UPDATE alert_history
            SET status = 'SENT', sent_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .fetch_one(pool)
        .await?;

        debug!(alert_id = %id, "알림 발송 완료 표시");
        Ok(alert)
    }

    /// 알림 상태 업데이트 (발송 실패)
    pub async fn mark_failed(
        pool: &PgPool,
        id: Uuid,
        error_message: &str,
    ) -> Result<AlertHistory, sqlx::Error> {
        let alert = sqlx::query_as::<_, AlertHistory>(
            r#"
            UPDATE alert_history
            SET status = 'FAILED', error_message = $2, retry_count = retry_count + 1
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(error_message)
        .fetch_one(pool)
        .await?;

        debug!(alert_id = %id, error = error_message, "알림 발송 실패 표시");
        Ok(alert)
    }

    /// 알림 확인(acknowledge) 처리
    pub async fn acknowledge(
        pool: &PgPool,
        id: Uuid,
        acknowledged_by: Option<&str>,
    ) -> Result<AlertHistory, sqlx::Error> {
        let alert = sqlx::query_as::<_, AlertHistory>(
            r#"
            UPDATE alert_history
            SET status = 'ACKNOWLEDGED', acknowledged_at = NOW(), acknowledged_by = $2
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(acknowledged_by)
        .fetch_one(pool)
        .await?;

        info!(alert_id = %id, by = ?acknowledged_by, "알림 확인 처리");
        Ok(alert)
    }

    /// ID로 알림 조회
    pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Option<AlertHistory>, sqlx::Error> {
        sqlx::query_as::<_, AlertHistory>(
            r#"SELECT * FROM alert_history WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    /// 알림 목록 조회 (필터링 + 페이징)
    pub async fn list(
        pool: &PgPool,
        filter: &AlertFilter,
    ) -> Result<AlertHistoryResponse, sqlx::Error> {
        // 동적 WHERE 조건 빌드
        let mut conditions = vec!["1=1".to_string()];
        let mut param_idx = 1;

        if filter.alert_type.is_some() {
            conditions.push(format!("alert_type = ${}", param_idx));
            param_idx += 1;
        }
        if filter.channel.is_some() {
            conditions.push(format!("channel = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            conditions.push(format!("status = ${}", param_idx));
            param_idx += 1;
        }
        if filter.rule_id.is_some() {
            conditions.push(format!("rule_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.from_date.is_some() {
            conditions.push(format!("created_at >= ${}", param_idx));
            param_idx += 1;
        }
        if filter.to_date.is_some() {
            conditions.push(format!("created_at <= ${}", param_idx));
            param_idx += 1;
        }

        let where_clause = conditions.join(" AND ");

        // 데이터 쿼리
        let data_query = format!(
            r#"
            SELECT * FROM alert_history
            WHERE {}
            ORDER BY created_at DESC
            LIMIT ${} OFFSET ${}
            "#,
            where_clause, param_idx, param_idx + 1
        );

        // 바인딩 (수동으로 처리 - sqlx의 동적 쿼리 한계)
        // 실제 구현에서는 QueryBuilder 사용 권장
        let mut query = sqlx::query_as::<_, AlertHistory>(&data_query);

        if let Some(ref alert_type) = filter.alert_type {
            query = query.bind(alert_type);
        }
        if let Some(ref channel) = filter.channel {
            query = query.bind(channel);
        }
        if let Some(ref status) = filter.status {
            query = query.bind(status);
        }
        if let Some(rule_id) = filter.rule_id {
            query = query.bind(rule_id);
        }
        if let Some(from_date) = filter.from_date {
            query = query.bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            query = query.bind(to_date);
        }

        query = query.bind(filter.limit).bind(filter.offset);

        let alerts = query.fetch_all(pool).await?;

        // 총 개수 조회 (간단한 버전)
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM alert_history")
            .fetch_one(pool)
            .await?;

        Ok(AlertHistoryResponse {
            alerts,
            total: total.0,
            limit: filter.limit,
            offset: filter.offset,
        })
    }

    /// 최근 알림 조회
    pub async fn get_recent(
        pool: &PgPool,
        limit: i32,
    ) -> Result<Vec<AlertHistory>, sqlx::Error> {
        sqlx::query_as::<_, AlertHistory>(
            r#"
            SELECT * FROM alert_history
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// 미확인 알림 조회 (SENT 상태)
    pub async fn get_unacknowledged(
        pool: &PgPool,
        limit: i32,
    ) -> Result<Vec<AlertHistory>, sqlx::Error> {
        sqlx::query_as::<_, AlertHistory>(
            r#"
            SELECT * FROM alert_history
            WHERE status = 'SENT'
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// 대기 중인 알림 조회 (PENDING 상태)
    pub async fn get_pending(pool: &PgPool, limit: i32) -> Result<Vec<AlertHistory>, sqlx::Error> {
        sqlx::query_as::<_, AlertHistory>(
            r#"
            SELECT * FROM alert_history
            WHERE status = 'PENDING'
            ORDER BY created_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// 알림 통계 조회
    pub async fn get_stats(pool: &PgPool) -> Result<AlertStats, sqlx::Error> {
        sqlx::query_as::<_, AlertStats>(
            r#"
            SELECT
                COUNT(*) as total_count,
                COUNT(*) FILTER (WHERE status = 'PENDING') as pending_count,
                COUNT(*) FILTER (WHERE status = 'SENT') as sent_count,
                COUNT(*) FILTER (WHERE status = 'FAILED') as failed_count,
                COUNT(*) FILTER (WHERE status = 'ACKNOWLEDGED') as acknowledged_count
            FROM alert_history
            "#,
        )
        .fetch_one(pool)
        .await
    }

    /// 오래된 알림 삭제 (정리용)
    pub async fn delete_old(pool: &PgPool, days: i32) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM alert_history
            WHERE created_at < NOW() - INTERVAL '1 day' * $1
            AND status IN ('SENT', 'ACKNOWLEDGED', 'FAILED')
            "#,
        )
        .bind(days)
        .execute(pool)
        .await?;

        let deleted = result.rows_affected();
        if deleted > 0 {
            info!(deleted = deleted, days = days, "오래된 알림 삭제 완료");
        }

        Ok(deleted)
    }

    /// 특정 규칙의 알림 히스토리 조회
    pub async fn get_by_rule(
        pool: &PgPool,
        rule_id: Uuid,
        limit: i32,
    ) -> Result<Vec<AlertHistory>, sqlx::Error> {
        sqlx::query_as::<_, AlertHistory>(
            r#"
            SELECT * FROM alert_history
            WHERE rule_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(rule_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// 일괄 확인 처리 (ID 목록)
    pub async fn acknowledge_batch(
        pool: &PgPool,
        ids: &[Uuid],
        acknowledged_by: Option<&str>,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE alert_history
            SET status = 'ACKNOWLEDGED', acknowledged_at = NOW(), acknowledged_by = $2
            WHERE id = ANY($1) AND status = 'SENT'
            "#,
        )
        .bind(ids)
        .bind(acknowledged_by)
        .execute(pool)
        .await?;

        let count = result.rows_affected();
        info!(count = count, "일괄 알림 확인 처리");
        Ok(count)
    }

    /// 읽지 않은 알림 수 조회 (PENDING/SENT 상태)
    pub async fn count_unread(pool: &PgPool) -> Result<i64, sqlx::Error> {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM alert_history
            WHERE status IN ('PENDING', 'SENT')
            "#,
        )
        .fetch_one(pool)
        .await?;

        Ok(count.0)
    }

    /// 전체 읽음 처리 (모든 SENT 상태 알림)
    pub async fn acknowledge_all(pool: &PgPool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE alert_history
            SET status = 'ACKNOWLEDGED', acknowledged_at = NOW(), acknowledged_by = 'user'
            WHERE status = 'SENT'
            "#,
        )
        .execute(pool)
        .await?;

        let count = result.rows_affected();
        info!(count = count, "전체 알림 읽음 처리");
        Ok(count)
    }
}
