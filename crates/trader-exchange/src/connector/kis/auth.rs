//! KIS OAuth 2.0 인증 모듈.
//!
//! 처리 기능:
//! - 접근 토큰 발급 및 갱신 (POST /oauth2/tokenP)
//! - 토큰 폐기 (POST /oauth2/revokeP)
//! - 해시 키 생성 (POST /uapi/hashkey)
//! - WebSocket 접속 키 (POST /oauth2/Approval)

use super::config::KisConfig;
use crate::ExchangeError;
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// 토큰 갱신 임계값 (남은 시간이 이 값보다 적으면 갱신).
const TOKEN_REFRESH_THRESHOLD_HOURS: i64 = 1;

/// KIS OAuth 토큰 응답.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    /// 접근 토큰
    pub access_token: String,
    /// 토큰 타입 (항상 "Bearer")
    pub token_type: String,
    /// 토큰 만료 시간 (초)
    pub expires_in: i64,
    /// 접근 토큰 만료 시각 (KIS 형식: "YYYY-MM-DD HH:MM:SS")
    pub access_token_token_expired: String,
}

/// KIS 해시 키 응답.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct HashkeyResponse {
    /// 생성된 해시 키
    pub hash: String,
}

/// KIS WebSocket 접속 승인 응답.
#[derive(Debug, Clone, Deserialize)]
pub struct ApprovalResponse {
    /// WebSocket 연결용 접속 키
    pub approval_key: String,
}

/// KIS API 오류 응답.
#[derive(Debug, Clone, Deserialize)]
pub struct KisErrorResponse {
    /// 응답 코드 (0 = 성공)
    pub rt_cd: String,
    /// 메시지 코드
    pub msg_cd: String,
    /// 메시지 내용
    pub msg1: String,
}

/// KIS OAuth 오류 응답 (토큰 발급 실패 시).
#[derive(Debug, Clone, Deserialize)]
pub struct KisOAuthErrorResponse {
    /// 에러 코드 (예: "EGW00103")
    pub error_code: String,
    /// 에러 설명 (예: "유효하지 않은 AppKey입니다.")
    pub error_description: String,
}

/// 만료 추적이 포함된 토큰 상태.
#[derive(Debug, Clone)]
pub struct TokenState {
    /// 접근 토큰
    pub access_token: String,
    /// 토큰 타입
    pub token_type: String,
    /// 만료 시각
    pub expires_at: DateTime<Utc>,
}

impl TokenState {
    /// 새 토큰 상태 생성.
    pub fn new(access_token: String, token_type: String, expires_at: DateTime<Utc>) -> Self {
        Self {
            access_token,
            token_type,
            expires_at,
        }
    }

    /// 토큰이 만료되었거나 곧 만료되는지 확인.
    pub fn is_expired_or_expiring(&self) -> bool {
        let threshold = Utc::now() + Duration::hours(TOKEN_REFRESH_THRESHOLD_HOURS);
        self.expires_at <= threshold
    }

    /// 토큰이 유효한지 확인.
    pub fn is_valid(&self) -> bool {
        self.expires_at > Utc::now()
    }

    /// 인증 헤더 값 반환.
    pub fn auth_header(&self) -> String {
        format!("{} {}", self.token_type, self.access_token)
    }
}

/// KIS OAuth 인증 관리자.
///
/// 자동 갱신을 포함한 토큰 수명 주기를 관리합니다.
pub struct KisOAuth {
    config: KisConfig,
    client: Client,
    token: Arc<RwLock<Option<TokenState>>>,
    websocket_key: Arc<RwLock<Option<String>>>,
    /// 토큰 갱신 API 호출 직렬화 (동시 발급 방지)
    refresh_lock: tokio::sync::Mutex<()>,
}

impl KisOAuth {
    /// 새로운 OAuth 관리자 생성.
    ///
    /// # Errors
    /// HTTP 클라이언트 생성에 실패하면 `ExchangeError::NetworkError`를 반환합니다.
    pub fn new(config: KisConfig) -> Result<Self, ExchangeError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| ExchangeError::NetworkError(format!("HTTP client 생성 실패: {}", e)))?;

        Ok(Self {
            config,
            client,
            token: Arc::new(RwLock::new(None)),
            websocket_key: Arc::new(RwLock::new(None)),
            refresh_lock: tokio::sync::Mutex::new(()),
        })
    }

    /// 초기 토큰 설정 (DB에서 로드한 토큰 사용).
    ///
    /// DB 기반 토큰 캐싱을 위해 사용합니다.
    /// 유효한 토큰이 있으면 API 호출 없이 재사용됩니다.
    pub async fn set_cached_token(&self, token: TokenState) {
        if token.is_valid() {
            info!(
                "Setting cached KIS token (expires at: {})",
                token.expires_at
            );
            let mut token_guard = self.token.write().await;
            *token_guard = Some(token);
        } else {
            debug!("Ignoring expired cached token");
        }
    }

    /// 현재 캐시된 토큰 반환 (API 호출 없이).
    ///
    /// DB에 저장할 때 사용합니다.
    pub async fn get_cached_token(&self) -> Option<TokenState> {
        let token_guard = self.token.read().await;
        token_guard.clone()
    }

    /// 토큰 갱신 후 새 토큰 반환 (DB 저장용).
    ///
    /// `refresh_token()`을 호출하고 결과를 반환합니다.
    /// 호출자는 반환된 토큰을 DB에 저장해야 합니다.
    pub async fn refresh_and_get_token(&self) -> Result<TokenState, ExchangeError> {
        self.refresh_token().await
    }

    /// 유효한 접근 토큰 반환, 필요시 갱신.
    pub async fn get_token(&self) -> Result<TokenState, ExchangeError> {
        // 유효한 토큰이 있는지 확인
        {
            let token_guard = self.token.read().await;
            if let Some(ref token) = *token_guard {
                if !token.is_expired_or_expiring() {
                    debug!("Using cached KIS token (expires at: {})", token.expires_at);
                    return Ok(token.clone());
                } else {
                    warn!(
                        "KIS token expired or expiring soon (expires at: {}), refreshing...",
                        token.expires_at
                    );
                }
            } else {
                info!("No cached KIS token found, requesting new token...");
            }
        }

        // 갱신 또는 새 토큰 발급 필요
        self.refresh_token().await
    }

    /// 접근 토큰 강제 갱신.
    ///
    /// 동시 호출 시 하나의 스레드만 실제 API를 호출하고,
    /// 나머지는 갱신된 토큰을 재사용합니다 (double-check locking).
    pub async fn refresh_token(&self) -> Result<TokenState, ExchangeError> {
        // 토큰 갱신 직렬화 (동시 호출 방지)
        let _refresh_guard = self.refresh_lock.lock().await;

        // Double-check: lock 대기 중 다른 스레드가 이미 갱신했으면 재사용
        {
            let token_guard = self.token.read().await;
            if let Some(ref token) = *token_guard {
                if !token.is_expired_or_expiring() {
                    debug!("토큰이 이미 갱신됨 (다른 스레드), 캐시 사용");
                    return Ok(token.clone());
                }
            }
        }

        // AppKey 유효성 검증
        if self.config.app_key.is_empty() || self.config.app_key.len() < 20 {
            error!(
                "유효하지 않은 AppKey: '{}' (길이: {})",
                self.config.app_key,
                self.config.app_key.len()
            );
            return Err(ExchangeError::Unauthorized(
                "KIS_APP_KEY 환경변수가 올바르게 설정되지 않았습니다. \
                한국투자증권에서 발급받은 AppKey를 설정하세요."
                    .to_string(),
            ));
        }

        if self.config.app_secret.is_empty() || self.config.app_secret.len() < 20 {
            error!(
                "유효하지 않은 AppSecret (길이: {})",
                self.config.app_secret.len()
            );
            return Err(ExchangeError::Unauthorized(
                "KIS_APP_SECRET 환경변수가 올바르게 설정되지 않았습니다.".to_string(),
            ));
        }

        info!(
            "Requesting new KIS access token... (AppKey: {}...)",
            &self.config.app_key.chars().take(8).collect::<String>()
        );

        let url = format!("{}/oauth2/tokenP", self.config.rest_base_url());

        #[derive(Serialize)]
        struct TokenRequest {
            grant_type: String,
            appkey: String,
            appsecret: String,
        }

        let request_body = TokenRequest {
            grant_type: "client_credentials".to_string(),
            appkey: self.config.app_key.clone(),
            appsecret: self.config.app_secret.clone(),
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json; charset=utf-8")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("Token request failed: {} - {}", status, body);

            // OAuth 에러 응답 파싱 시도
            if let Ok(oauth_error) = serde_json::from_str::<KisOAuthErrorResponse>(&body) {
                let error_msg = match oauth_error.error_code.as_str() {
                    "EGW00103" => format!(
                        "유효하지 않은 AppKey입니다. 환경변수(KIS_APP_KEY, KIS_APP_SECRET)를 확인하세요. AppKey: {}...",
                        &self.config.app_key.chars().take(8).collect::<String>()
                    ),
                    "EGW00102" => "AppKey가 만료되었습니다. 한국투자증권에서 새 AppKey를 발급받으세요.".to_string(),
                    "EGW00101" => "AppSecret이 일치하지 않습니다.".to_string(),
                    _ => format!("{} ({})", oauth_error.error_description, oauth_error.error_code),
                };

                error!("KIS OAuth 에러: {}", error_msg);
                return Err(ExchangeError::Unauthorized(error_msg));
            }

            // 일반 API 에러 응답 파싱 시도
            if let Ok(error_resp) = serde_json::from_str::<KisErrorResponse>(&body) {
                return Err(ExchangeError::ApiError {
                    code: error_resp.msg_cd.parse().unwrap_or(-1),
                    message: error_resp.msg1,
                });
            }

            // 파싱 실패 시 원본 응답 반환
            return Err(ExchangeError::Unauthorized(format!(
                "Token request failed: {}",
                body
            )));
        }

        let token_resp: TokenResponse = serde_json::from_str(&body).map_err(|e| {
            ExchangeError::ParseError(format!("Failed to parse token response: {}", e))
        })?;

        // Parse expiry time from KIS format ("YYYY-MM-DD HH:MM:SS")
        let expires_at = parse_kis_datetime(&token_resp.access_token_token_expired)
            .unwrap_or_else(|| Utc::now() + Duration::seconds(token_resp.expires_in));

        let token_state = TokenState {
            access_token: token_resp.access_token,
            token_type: token_resp.token_type,
            expires_at,
        };

        // Store the new token
        {
            let mut token_guard = self.token.write().await;
            *token_guard = Some(token_state.clone());
        }

        info!(
            "KIS access token obtained, expires at: {}",
            token_state.expires_at
        );

        Ok(token_state)
    }

    /// 현재 접근 토큰 폐기.
    pub async fn revoke_token(&self) -> Result<(), ExchangeError> {
        let token = {
            let token_guard = self.token.read().await;
            match &*token_guard {
                Some(t) => t.access_token.clone(),
                None => return Ok(()), // No token to revoke
            }
        };

        info!("Revoking KIS access token...");

        let url = format!("{}/oauth2/revokeP", self.config.rest_base_url());

        #[derive(Serialize)]
        struct RevokeRequest {
            appkey: String,
            appsecret: String,
            token: String,
        }

        let request_body = RevokeRequest {
            appkey: self.config.app_key.clone(),
            appsecret: self.config.app_secret.clone(),
            token,
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json; charset=utf-8")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if response.status().is_success() {
            let mut token_guard = self.token.write().await;
            *token_guard = None;
            info!("KIS access token revoked successfully");
        } else {
            warn!("Token revocation may have failed, clearing local state anyway");
            let mut token_guard = self.token.write().await;
            *token_guard = None;
        }

        Ok(())
    }

    /// POST 요청 본문에 대한 해시 키 생성.
    ///
    /// 해시 키는 특정 POST 엔드포인트(예: 주문 실행)에 필요합니다.
    /// 요청 본문의 SHA-512 해시입니다.
    pub async fn generate_hashkey(
        &self,
        body: &serde_json::Value,
    ) -> Result<String, ExchangeError> {
        // 참고: 해시 키 생성에는 인증 토큰이 필요하지 않으며,
        // 헤더에 app_key와 app_secret만 필요합니다
        let url = format!("{}/uapi/hashkey", self.config.rest_base_url());

        debug!("Generating hashkey for body: {}", body);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json; charset=utf-8")
            .header("appkey", &self.config.app_key)
            .header("appsecret", &self.config.app_secret)
            .json(body)
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let response_body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("Hashkey generation failed: {} - {}", status, response_body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: response_body,
            });
        }

        let hashkey_resp: HashkeyResponse = serde_json::from_str(&response_body).map_err(|e| {
            ExchangeError::ParseError(format!("Failed to parse hashkey response: {}", e))
        })?;

        debug!("Generated hashkey: {}", hashkey_resp.hash);

        Ok(hashkey_resp.hash)
    }

    /// WebSocket 접속 키 획득.
    ///
    /// 이 키는 실시간 데이터를 위한 WebSocket 연결 수립에 필요합니다.
    pub async fn get_websocket_key(&self) -> Result<String, ExchangeError> {
        // 유효한 키가 있는지 확인
        {
            let key_guard = self.websocket_key.read().await;
            if let Some(ref key) = *key_guard {
                return Ok(key.clone());
            }
        }

        info!("Requesting WebSocket approval key...");

        let url = format!("{}/oauth2/Approval", self.config.rest_base_url());

        #[derive(Serialize)]
        struct ApprovalRequest {
            grant_type: String,
            appkey: String,
            secretkey: String,
        }

        let request_body = ApprovalRequest {
            grant_type: "client_credentials".to_string(),
            appkey: self.config.app_key.clone(),
            secretkey: self.config.app_secret.clone(),
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json; charset=utf-8")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExchangeError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("WebSocket approval failed: {} - {}", status, body);
            return Err(ExchangeError::ApiError {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        let approval_resp: ApprovalResponse = serde_json::from_str(&body).map_err(|e| {
            ExchangeError::ParseError(format!("Failed to parse approval response: {}", e))
        })?;

        // 키 저장
        {
            let mut key_guard = self.websocket_key.write().await;
            *key_guard = Some(approval_resp.approval_key.clone());
        }

        info!("WebSocket approval key obtained");

        Ok(approval_resp.approval_key)
    }

    /// WebSocket 키 초기화 (재연결 시 호출).
    pub async fn clear_websocket_key(&self) {
        let mut key_guard = self.websocket_key.write().await;
        *key_guard = None;
    }

    /// 유효한 토큰이 있는지 확인.
    pub async fn has_valid_token(&self) -> bool {
        let token_guard = self.token.read().await;
        token_guard.as_ref().map(|t| t.is_valid()).unwrap_or(false)
    }

    /// 현재 토큰 만료 시각 반환.
    pub async fn token_expires_at(&self) -> Option<DateTime<Utc>> {
        let token_guard = self.token.read().await;
        token_guard.as_ref().map(|t| t.expires_at)
    }

    /// 인증된 요청을 위한 공통 헤더 생성.
    ///
    /// # Errors
    /// 헤더 값 파싱에 실패하면 `ExchangeError::ParseError`를 반환합니다.
    pub async fn build_headers(
        &self,
        tr_id: &str,
        hashkey: Option<&str>,
    ) -> Result<reqwest::header::HeaderMap, ExchangeError> {
        let token = self.get_token().await?;

        let mut headers = reqwest::header::HeaderMap::new();

        // 상수 문자열은 컴파일 타임에 검증되므로 unwrap() 안전
        headers.insert(
            "Content-Type",
            "application/json; charset=utf-8".parse().unwrap(),
        );

        // 동적 값들은 map_err로 에러 전파
        headers.insert(
            "authorization",
            token.auth_header().parse().map_err(|_| {
                ExchangeError::ParseError(
                    "authorization 헤더에 유효하지 않은 문자 포함".to_string(),
                )
            })?,
        );
        headers.insert(
            "appkey",
            self.config.app_key.parse().map_err(|_| {
                ExchangeError::ParseError("app_key에 유효하지 않은 문자 포함".to_string())
            })?,
        );
        headers.insert(
            "appsecret",
            self.config.app_secret.parse().map_err(|_| {
                ExchangeError::ParseError("app_secret에 유효하지 않은 문자 포함".to_string())
            })?,
        );
        headers.insert(
            "tr_id",
            tr_id.parse().map_err(|_| {
                ExchangeError::ParseError(format!("tr_id에 유효하지 않은 문자 포함: {}", tr_id))
            })?,
        );

        if let Some(hash) = hashkey {
            headers.insert(
                "hashkey",
                hash.parse().map_err(|_| {
                    ExchangeError::ParseError("hashkey에 유효하지 않은 문자 포함".to_string())
                })?,
            );
        }

        // 개인인증이 활성화된 경우 헤더 추가
        // 상수 문자열 "P"는 컴파일 타임에 검증되므로 unwrap() 안전
        if self.config.personalized {
            headers.insert(
                "custtype",
                "P".parse().unwrap(), // P = Personal
            );
        }

        Ok(headers)
    }

    /// 설정 반환.
    pub fn config(&self) -> &KisConfig {
        &self.config
    }
}

/// KIS 날짜시간 형식 파싱 ("YYYY-MM-DD HH:MM:SS").
fn parse_kis_datetime(s: &str) -> Option<DateTime<Utc>> {
    // KIS는 KST (한국 표준시, UTC+9) 사용
    use chrono::{NaiveDateTime, TimeZone};
    use chrono_tz::Asia::Seoul;

    let naive = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok()?;
    let kst = Seoul.from_local_datetime(&naive).single()?;
    Some(kst.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn test_token_state_expiry() {
        let token = TokenState {
            access_token: "test".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: Utc::now() + Duration::hours(24),
        };

        assert!(token.is_valid());
        assert!(!token.is_expired_or_expiring());
    }

    #[test]
    fn test_token_state_expiring() {
        let token = TokenState {
            access_token: "test".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: Utc::now() + Duration::minutes(30),
        };

        assert!(token.is_valid());
        assert!(token.is_expired_or_expiring()); // Within 1 hour threshold
    }

    #[test]
    fn test_token_auth_header() {
        let token = TokenState {
            access_token: "abc123".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: Utc::now() + Duration::hours(24),
        };

        assert_eq!(token.auth_header(), "Bearer abc123");
    }

    #[test]
    fn test_parse_kis_datetime() {
        let result = parse_kis_datetime("2026-01-28 15:30:00");
        assert!(result.is_some());

        let dt = result.unwrap();
        // KST is UTC+9, so 15:30 KST = 06:30 UTC
        assert_eq!(dt.hour(), 6);
        assert_eq!(dt.minute(), 30);
    }
}
