//! JWT 토큰 처리.
//!
//! Access Token 및 Refresh Token 생성/검증 로직.

#![allow(dead_code)] // 향후 인증 시스템에서 사용 예정

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};

use super::Role;

/// JWT Access Token 페이로드.
///
/// 사용자 인증 정보와 권한을 포함합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject - 사용자 ID
    pub sub: String,
    /// 사용자 이름
    pub username: String,
    /// 사용자 역할
    pub role: Role,
    /// Issued At - 토큰 발급 시간 (Unix timestamp)
    pub iat: i64,
    /// Expiration - 토큰 만료 시간 (Unix timestamp)
    pub exp: i64,
    /// JWT ID - 토큰 고유 식별자
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
}

impl Claims {
    /// 새로운 Claims 생성.
    ///
    /// # Arguments
    ///
    /// * `user_id` - 사용자 ID
    /// * `username` - 사용자 이름
    /// * `role` - 사용자 역할
    /// * `expires_in` - 만료 시간 (분)
    pub fn new(
        user_id: impl Into<String>,
        username: impl Into<String>,
        role: Role,
        expires_in_minutes: i64,
    ) -> Self {
        let now = Utc::now();
        Self {
            sub: user_id.into(),
            username: username.into(),
            role,
            iat: now.timestamp(),
            exp: (now + Duration::minutes(expires_in_minutes)).timestamp(),
            jti: Some(uuid::Uuid::new_v4().to_string()),
        }
    }

    /// 토큰이 만료되었는지 확인.
    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp() > self.exp
    }

    /// 특정 권한을 가지는지 확인.
    pub fn has_permission(&self, permission: super::Permission) -> bool {
        self.role.has_permission(permission)
    }

    /// 특정 역할 이상인지 확인.
    pub fn has_role(&self, required_role: Role) -> bool {
        self.role.level() >= required_role.level()
    }
}

/// Refresh Token 페이로드.
///
/// Access Token 갱신에 사용됩니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshClaims {
    /// Subject - 사용자 ID
    pub sub: String,
    /// Issued At
    pub iat: i64,
    /// Expiration
    pub exp: i64,
    /// JWT ID
    pub jti: String,
    /// Token type
    pub token_type: String,
}

impl RefreshClaims {
    /// 새로운 Refresh Claims 생성.
    ///
    /// # Arguments
    ///
    /// * `user_id` - 사용자 ID
    /// * `expires_in_days` - 만료 시간 (일)
    pub fn new(user_id: impl Into<String>, expires_in_days: i64) -> Self {
        let now = Utc::now();
        Self {
            sub: user_id.into(),
            iat: now.timestamp(),
            exp: (now + Duration::days(expires_in_days)).timestamp(),
            jti: uuid::Uuid::new_v4().to_string(),
            token_type: "refresh".to_string(),
        }
    }
}

/// Access Token + Refresh Token 페어.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    /// Access Token
    pub access_token: String,
    /// Refresh Token
    pub refresh_token: String,
    /// Access Token 만료 시간 (초)
    pub expires_in: i64,
    /// 토큰 타입 (항상 "Bearer")
    pub token_type: String,
}

/// JWT 토큰 생성 에러.
#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    #[error("토큰 인코딩 실패: {0}")]
    EncodingError(#[from] jsonwebtoken::errors::Error),
    #[error("토큰 디코딩 실패")]
    DecodingError,
    #[error("토큰이 만료되었습니다")]
    TokenExpired,
    #[error("잘못된 토큰 형식")]
    InvalidToken,
}

/// Access Token 생성.
///
/// # Arguments
///
/// * `claims` - JWT 페이로드
/// * `secret` - 비밀 키
///
/// # Returns
///
/// 인코딩된 JWT 문자열
pub fn create_token(claims: &Claims, secret: &str) -> Result<String, JwtError> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(JwtError::from)
}

/// Refresh Token 생성.
///
/// # Arguments
///
/// * `claims` - Refresh Claims
/// * `secret` - 비밀 키
pub fn create_refresh_token(claims: &RefreshClaims, secret: &str) -> Result<String, JwtError> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(JwtError::from)
}

/// Access Token + Refresh Token 쌍 생성.
///
/// # Arguments
///
/// * `user_id` - 사용자 ID
/// * `username` - 사용자 이름
/// * `role` - 사용자 역할
/// * `secret` - JWT 비밀 키
/// * `access_expires_minutes` - Access Token 만료 시간 (분)
/// * `refresh_expires_days` - Refresh Token 만료 시간 (일)
pub fn create_token_pair(
    user_id: &str,
    username: &str,
    role: Role,
    secret: &str,
    access_expires_minutes: i64,
    refresh_expires_days: i64,
) -> Result<TokenPair, JwtError> {
    let access_claims = Claims::new(user_id, username, role, access_expires_minutes);
    let refresh_claims = RefreshClaims::new(user_id, refresh_expires_days);

    let access_token = create_token(&access_claims, secret)?;
    let refresh_token = create_refresh_token(&refresh_claims, secret)?;

    Ok(TokenPair {
        access_token,
        refresh_token,
        expires_in: access_expires_minutes * 60,
        token_type: "Bearer".to_string(),
    })
}

/// JWT 토큰 디코딩 및 검증.
///
/// # Arguments
///
/// * `token` - JWT 토큰 문자열
/// * `secret` - 비밀 키
///
/// # Returns
///
/// 디코딩된 Claims
pub fn decode_token(token: &str, secret: &str) -> Result<TokenData<Claims>, JwtError> {
    let mut validation = Validation::default();
    validation.validate_exp = true;

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::TokenExpired,
        jsonwebtoken::errors::ErrorKind::InvalidToken => JwtError::InvalidToken,
        _ => JwtError::DecodingError,
    })
}

/// Refresh Token 디코딩 및 검증.
pub fn decode_refresh_token(
    token: &str,
    secret: &str,
) -> Result<TokenData<RefreshClaims>, JwtError> {
    let mut validation = Validation::default();
    validation.validate_exp = true;

    decode::<RefreshClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::TokenExpired,
        jsonwebtoken::errors::ErrorKind::InvalidToken => JwtError::InvalidToken,
        _ => JwtError::DecodingError,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "test-secret-key-for-jwt-testing-minimum-32-chars";

    #[test]
    fn test_create_and_decode_token() {
        let claims = Claims::new("user123", "testuser", Role::Trader, 60);

        let token = create_token(&claims, TEST_SECRET).unwrap();
        assert!(!token.is_empty());

        let decoded = decode_token(&token, TEST_SECRET).unwrap();
        assert_eq!(decoded.claims.sub, "user123");
        assert_eq!(decoded.claims.username, "testuser");
        assert_eq!(decoded.claims.role, Role::Trader);
    }

    #[test]
    fn test_create_token_pair() {
        let pair = create_token_pair(
            "user123",
            "testuser",
            Role::Admin,
            TEST_SECRET,
            30, // 30분
            7,  // 7일
        )
        .unwrap();

        assert!(!pair.access_token.is_empty());
        assert!(!pair.refresh_token.is_empty());
        assert_eq!(pair.token_type, "Bearer");
        assert_eq!(pair.expires_in, 30 * 60);

        // Access token 검증
        let access = decode_token(&pair.access_token, TEST_SECRET).unwrap();
        assert_eq!(access.claims.sub, "user123");
        assert_eq!(access.claims.role, Role::Admin);

        // Refresh token 검증
        let refresh = decode_refresh_token(&pair.refresh_token, TEST_SECRET).unwrap();
        assert_eq!(refresh.claims.sub, "user123");
        assert_eq!(refresh.claims.token_type, "refresh");
    }

    #[test]
    fn test_claims_permissions() {
        let claims = Claims::new("user123", "trader", Role::Trader, 60);

        assert!(claims.has_permission(super::super::Permission::ManageOrders));
        assert!(claims.has_permission(super::super::Permission::ViewDashboard));
        assert!(!claims.has_permission(super::super::Permission::ManageUsers));
    }

    #[test]
    fn test_claims_has_role() {
        let admin_claims = Claims::new("admin", "admin", Role::Admin, 60);
        let trader_claims = Claims::new("trader", "trader", Role::Trader, 60);

        assert!(admin_claims.has_role(Role::Trader));
        assert!(admin_claims.has_role(Role::Viewer));
        assert!(trader_claims.has_role(Role::Viewer));
        assert!(!trader_claims.has_role(Role::Admin));
    }

    #[test]
    fn test_invalid_token() {
        let result = decode_token("invalid.token.here", TEST_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_secret() {
        let claims = Claims::new("user123", "testuser", Role::Viewer, 60);
        let token = create_token(&claims, TEST_SECRET).unwrap();

        let result = decode_token(&token, "wrong-secret-key-for-testing-minimum-32-chars");
        assert!(result.is_err());
    }
}
