//! 워크플로우 체크포인트 관리 모듈.
//!
//! 장시간 실행되는 배치 작업의 중단/재개를 지원합니다.
//!
//! # 주요 기능
//!
//! - **체크포인트 저장**: 100개 처리마다 진행 상태 저장
//! - **중단점 재개**: 중단된 지점부터 이어서 처리
//! - **Stale 필터링**: N시간 이내 처리된 데이터 스킵

#![allow(clippy::type_complexity)]
//!
//! # 사용 예
//!
//! ```rust,ignore
//! // 워크플로우 시작 시
//! let resume_ticker = if options.resume {
//!     load_checkpoint(pool, "my_workflow").await?
//! } else {
//!     None
//! };
//!
//! // 처리 중 (100개마다)
//! save_checkpoint(pool, "my_workflow", &ticker, processed, "running").await?;
//!
//! // 완료 시
//! save_checkpoint(pool, "my_workflow", "", total, "completed").await?;
//! ```

use sqlx::PgPool;

use crate::Result;

/// 체크포인트 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointStatus {
    /// 실행 중
    Running,
    /// 중단됨 (재개 가능)
    Interrupted,
    /// 완료됨
    Completed,
    /// 유휴 상태
    Idle,
}

impl CheckpointStatus {
    /// 문자열로 변환
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Interrupted => "interrupted",
            Self::Completed => "completed",
            Self::Idle => "idle",
        }
    }
}

/// 체크포인트 저장.
///
/// # Arguments
/// * `pool` - DB 연결 풀
/// * `workflow` - 워크플로우 이름 (e.g., "naver_fundamental", "ohlcv_collect")
/// * `ticker` - 마지막 처리된 티커 (완료 시 빈 문자열)
/// * `total_processed` - 총 처리된 수
/// * `status` - 현재 상태
pub async fn save_checkpoint(
    pool: &PgPool,
    workflow: &str,
    ticker: &str,
    total_processed: i32,
    status: CheckpointStatus,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO sync_checkpoint (workflow_name, last_ticker, last_processed_at, total_processed, status, updated_at)
        VALUES ($1, $2, NOW(), $3, $4, NOW())
        ON CONFLICT (workflow_name)
        DO UPDATE SET
            last_ticker = EXCLUDED.last_ticker,
            last_processed_at = NOW(),
            total_processed = EXCLUDED.total_processed,
            status = EXCLUDED.status,
            updated_at = NOW()
        "#,
    )
    .bind(workflow)
    .bind(ticker)
    .bind(total_processed)
    .bind(status.as_str())
    .execute(pool)
    .await?;
    Ok(())
}

/// 체크포인트 로드 (중단된 워크플로우의 마지막 티커 반환).
///
/// # Returns
/// * `Some(ticker)` - 중단된 지점의 마지막 티커
/// * `None` - 중단점이 없거나 완료된 상태
pub async fn load_checkpoint(pool: &PgPool, workflow: &str) -> Result<Option<String>> {
    let result: Option<(Option<String>,)> = sqlx::query_as(
        r#"
        SELECT last_ticker
        FROM sync_checkpoint
        WHERE workflow_name = $1 AND status = 'interrupted'
        "#,
    )
    .bind(workflow)
    .fetch_optional(pool)
    .await?;

    Ok(result.and_then(|(t,)| t))
}

/// 현재 실행 중인 워크플로우를 "interrupted"로 마킹.
///
/// 프로세스 종료 시 호출하여 다음 실행에서 재개 가능하도록 합니다.
pub async fn mark_interrupted(pool: &PgPool, workflow: &str) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE sync_checkpoint
        SET status = 'interrupted', updated_at = NOW()
        WHERE workflow_name = $1 AND status = 'running'
        "#,
    )
    .bind(workflow)
    .execute(pool)
    .await?;
    Ok(())
}

/// 워크플로우 체크포인트 삭제 (완전 초기화).
pub async fn clear_checkpoint(pool: &PgPool, workflow: &str) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM sync_checkpoint
        WHERE workflow_name = $1
        "#,
    )
    .bind(workflow)
    .execute(pool)
    .await?;
    Ok(())
}

/// 모든 워크플로우의 체크포인트 상태 조회.
pub async fn list_checkpoints(pool: &PgPool) -> Result<Vec<CheckpointInfo>> {
    let rows: Vec<(
        String,
        Option<String>,
        Option<chrono::DateTime<chrono::Utc>>,
        i32,
        String,
    )> = sqlx::query_as(
        r#"
            SELECT workflow_name, last_ticker, last_processed_at, total_processed, status
            FROM sync_checkpoint
            ORDER BY workflow_name
            "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(workflow_name, last_ticker, last_processed_at, total_processed, status)| {
                CheckpointInfo {
                    workflow_name,
                    last_ticker,
                    last_processed_at,
                    total_processed,
                    status,
                }
            },
        )
        .collect())
}

/// 체크포인트 정보
#[derive(Debug)]
pub struct CheckpointInfo {
    pub workflow_name: String,
    pub last_ticker: Option<String>,
    pub last_processed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub total_processed: i32,
    pub status: String,
}
