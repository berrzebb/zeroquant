//! KIS OAuth 토큰 DB 캐싱 Repository.
//!
//! KIS API의 1분당 1회 토큰 발급 제한을 우회하기 위해
//! 토큰을 DB에 저장하고 서버 재시작 시에도 재사용합니다.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::{debug, error, info};
use trader_exchange::connector::kis::TokenState;
use uuid::Uuid;

/// KIS 토큰 캐시 DB 행.
#[derive(Debug, sqlx::FromRow)]
pub struct KisTokenCacheRow {
    pub id: i32,
    pub credential_id: Uuid,
    pub environment: String,
    pub access_token: String,
    pub token_type: String,
    pub expires_at: DateTime<Utc>,
    pub websocket_key: Option<String>,
    pub websocket_key_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// KIS 토큰 캐시 Repository.
pub struct KisTokenRepository;

impl KisTokenRepository {
    /// DB에서 유효한 토큰 조회.
    ///
    /// 만료 1시간 전까지 유효한 토큰만 반환합니다.
    pub async fn load_valid_token(
        pool: &PgPool,
        credential_id: Uuid,
        environment: &str,
    ) -> Option<TokenState> {
        let row: Option<KisTokenCacheRow> = sqlx::query_as(
            r#"
            SELECT id, credential_id, environment, access_token, token_type,
                   expires_at, websocket_key, websocket_key_expires_at,
                   created_at, updated_at
            FROM kis_token_cache
            WHERE credential_id = $1
              AND environment = $2
              AND expires_at > NOW() + INTERVAL '1 hour'
            "#,
        )
        .bind(credential_id)
        .bind(environment)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        match row {
            Some(r) => {
                info!(
                    "DB에서 유효한 KIS 토큰 로드: credential_id={}, expires_at={}",
                    credential_id, r.expires_at
                );
                Some(TokenState::new(r.access_token, r.token_type, r.expires_at))
            }
            None => {
                debug!(
                    "DB에 유효한 KIS 토큰 없음: credential_id={}, environment={}",
                    credential_id, environment
                );
                None
            }
        }
    }

    /// DB에서 토큰 조회 (완화된 조건).
    ///
    /// `load_valid_token()`은 만료 1시간 전까지만 유효하다고 판단하지만,
    /// 이 함수는 아직 만료되지 않은 토큰이면 반환합니다.
    /// rate limit 에러(EGW00133) 발생 시 fallback으로 사용합니다.
    pub async fn load_any_valid_token(
        pool: &PgPool,
        credential_id: Uuid,
        environment: &str,
    ) -> Option<TokenState> {
        let row: Option<KisTokenCacheRow> = sqlx::query_as(
            r#"
            SELECT id, credential_id, environment, access_token, token_type,
                   expires_at, websocket_key, websocket_key_expires_at,
                   created_at, updated_at
            FROM kis_token_cache
            WHERE credential_id = $1
              AND environment = $2
              AND expires_at > NOW()
            "#,
        )
        .bind(credential_id)
        .bind(environment)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        match row {
            Some(r) => {
                info!(
                    "DB에서 fallback KIS 토큰 로드 (완화 조건): credential_id={}, expires_at={}",
                    credential_id, r.expires_at
                );
                Some(TokenState::new(r.access_token, r.token_type, r.expires_at))
            }
            None => {
                debug!(
                    "DB에 fallback KIS 토큰 없음: credential_id={}, environment={}",
                    credential_id, environment
                );
                None
            }
        }
    }

    /// 토큰을 DB에 저장 (upsert).
    ///
    /// 동일한 credential_id + environment 조합이 있으면 업데이트합니다.
    pub async fn save_token(
        pool: &PgPool,
        credential_id: Uuid,
        environment: &str,
        token: &TokenState,
    ) -> Result<(), String> {
        let result = sqlx::query(
            r#"
            INSERT INTO kis_token_cache
                (credential_id, environment, access_token, token_type, expires_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            ON CONFLICT (credential_id, environment)
            DO UPDATE SET
                access_token = EXCLUDED.access_token,
                token_type = EXCLUDED.token_type,
                expires_at = EXCLUDED.expires_at,
                updated_at = NOW()
            "#,
        )
        .bind(credential_id)
        .bind(environment)
        .bind(&token.access_token)
        .bind(&token.token_type)
        .bind(token.expires_at)
        .execute(pool)
        .await;

        match result {
            Ok(_) => {
                info!(
                    "KIS 토큰 DB 저장 완료: credential_id={}, expires_at={}",
                    credential_id, token.expires_at
                );
                Ok(())
            }
            Err(e) => {
                error!("KIS 토큰 DB 저장 실패: {}", e);
                Err(e.to_string())
            }
        }
    }

    /// WebSocket 키를 DB에 저장.
    pub async fn save_websocket_key(
        pool: &PgPool,
        credential_id: Uuid,
        environment: &str,
        websocket_key: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<(), String> {
        let result = sqlx::query(
            r#"
            UPDATE kis_token_cache
            SET websocket_key = $1,
                websocket_key_expires_at = $2,
                updated_at = NOW()
            WHERE credential_id = $3 AND environment = $4
            "#,
        )
        .bind(websocket_key)
        .bind(expires_at)
        .bind(credential_id)
        .bind(environment)
        .execute(pool)
        .await;

        match result {
            Ok(_) => {
                debug!("WebSocket 키 저장 완료: credential_id={}", credential_id);
                Ok(())
            }
            Err(e) => {
                error!("WebSocket 키 저장 실패: {}", e);
                Err(e.to_string())
            }
        }
    }

    /// 토큰 삭제 (로그아웃/폐기 시).
    pub async fn delete_token(
        pool: &PgPool,
        credential_id: Uuid,
        environment: &str,
    ) -> Result<(), String> {
        let result = sqlx::query(
            r#"
            DELETE FROM kis_token_cache
            WHERE credential_id = $1 AND environment = $2
            "#,
        )
        .bind(credential_id)
        .bind(environment)
        .execute(pool)
        .await;

        match result {
            Ok(_) => {
                info!("KIS 토큰 삭제 완료: credential_id={}", credential_id);
                Ok(())
            }
            Err(e) => {
                error!("KIS 토큰 삭제 실패: {}", e);
                Err(e.to_string())
            }
        }
    }

    /// 만료된 토큰 정리.
    pub async fn cleanup_expired_tokens(pool: &PgPool) -> Result<u64, String> {
        let result = sqlx::query(
            r#"
            DELETE FROM kis_token_cache
            WHERE expires_at < NOW() - INTERVAL '1 hour'
            "#,
        )
        .execute(pool)
        .await;

        match result {
            Ok(r) => {
                let count = r.rows_affected();
                if count > 0 {
                    info!("만료된 KIS 토큰 {} 삭제됨", count);
                }
                Ok(count)
            }
            Err(e) => {
                error!("만료된 토큰 정리 실패: {}", e);
                Err(e.to_string())
            }
        }
    }
}
