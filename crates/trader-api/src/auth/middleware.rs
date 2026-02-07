//! Axum용 JWT 인증 미들웨어.
//!
//! Axum 핸들러에서 사용할 JWT 인증 추출기 및 미들웨어.

#![allow(dead_code)] // 향후 인증 시스템에서 사용 예정

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use super::{decode_token, Claims, Role};

/// JWT 인증 추출기.
///
/// Axum 핸들러에서 인증된 사용자 정보를 추출합니다.
///
/// # 사용 예시
///
/// ```rust,ignore
/// async fn protected_handler(
///     JwtAuth(claims): JwtAuth,
/// ) -> impl IntoResponse {
///     format!("Authenticated user: {}", claims.username)
/// }
/// ```
#[derive(Debug, Clone)]
pub struct JwtAuth(pub Claims);

/// JWT 인증 에러.
#[derive(Debug, thiserror::Error)]
pub enum JwtAuthError {
    #[error("인증 토큰이 필요합니다")]
    MissingToken,
    #[error("잘못된 Authorization 헤더 형식")]
    InvalidAuthHeader,
    #[error("토큰이 만료되었습니다")]
    TokenExpired,
    #[error("유효하지 않은 토큰")]
    InvalidToken,
    #[error("권한이 부족합니다")]
    InsufficientPermission,
}

impl IntoResponse for JwtAuthError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            JwtAuthError::MissingToken => (StatusCode::UNAUTHORIZED, "MISSING_TOKEN"),
            JwtAuthError::InvalidAuthHeader => (StatusCode::UNAUTHORIZED, "INVALID_AUTH_HEADER"),
            JwtAuthError::TokenExpired => (StatusCode::UNAUTHORIZED, "TOKEN_EXPIRED"),
            JwtAuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "INVALID_TOKEN"),
            JwtAuthError::InsufficientPermission => {
                (StatusCode::FORBIDDEN, "INSUFFICIENT_PERMISSION")
            }
        };

        let body = Json(json!({
            "error": {
                "code": code,
                "message": self.to_string()
            }
        }));

        (status, body).into_response()
    }
}

/// JWT 비밀 키 저장소.
///
/// 애플리케이션 전역에서 JWT 비밀 키에 접근하기 위한 구조체.
#[derive(Clone)]
pub struct JwtConfig {
    pub secret: String,
}

impl<S> FromRequestParts<S> for JwtAuth
where
    S: Send + Sync,
{
    type Rejection = JwtAuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Authorization 헤더에서 토큰 추출
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or(JwtAuthError::MissingToken)?;

        // Bearer 토큰 형식 확인
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(JwtAuthError::InvalidAuthHeader)?;

        // Extensions에서 JWT secret 가져오기
        let jwt_secret = parts
            .extensions
            .get::<JwtConfig>()
            .map(|c| c.secret.clone())
            .unwrap_or_else(|| {
                // 개발/테스트 환경용 기본 시크릿 (프로덕션에서는 반드시 설정 필요)
                std::env::var("JWT_SECRET")
                    .unwrap_or_else(|_| "development-secret-key-change-in-production".to_string())
            });

        // 토큰 검증
        let token_data = decode_token(token, &jwt_secret).map_err(|e| match e {
            super::jwt::JwtError::TokenExpired => JwtAuthError::TokenExpired,
            _ => JwtAuthError::InvalidToken,
        })?;

        Ok(JwtAuth(token_data.claims))
    }
}

/// 특정 역할 이상의 권한을 요구하는 미들웨어 생성자.
///
/// # Arguments
///
/// * `required_role` - 필요한 최소 역할
/// * `claims` - 검증된 JWT Claims
///
/// # Returns
///
/// 권한이 충분하면 Ok(()), 부족하면 Err(JwtAuthError)
pub fn require_role(required_role: Role, claims: &Claims) -> Result<(), JwtAuthError> {
    if claims.has_role(required_role) {
        Ok(())
    } else {
        Err(JwtAuthError::InsufficientPermission)
    }
}

/// 선택적 JWT 인증 추출기.
///
/// 토큰이 있으면 검증하고, 없으면 None을 반환합니다.
/// 공개 API에서 인증 여부에 따라 다른 응답을 제공할 때 유용합니다.
#[derive(Debug, Clone)]
pub struct OptionalJwtAuth(pub Option<Claims>);

impl<S> FromRequestParts<S> for OptionalJwtAuth
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match JwtAuth::from_request_parts(parts, state).await {
            Ok(JwtAuth(claims)) => Ok(OptionalJwtAuth(Some(claims))),
            Err(_) => Ok(OptionalJwtAuth(None)),
        }
    }
}

/// Admin 권한을 요구하는 추출기.
#[derive(Debug, Clone)]
pub struct AdminAuth(pub Claims);

impl<S> FromRequestParts<S> for AdminAuth
where
    S: Send + Sync,
{
    type Rejection = JwtAuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let JwtAuth(claims) = JwtAuth::from_request_parts(parts, state).await?;
        require_role(Role::Admin, &claims)?;
        Ok(AdminAuth(claims))
    }
}

/// Trader 이상 권한을 요구하는 추출기.
#[derive(Debug, Clone)]
pub struct TraderAuth(pub Claims);

impl<S> FromRequestParts<S> for TraderAuth
where
    S: Send + Sync,
{
    type Rejection = JwtAuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let JwtAuth(claims) = JwtAuth::from_request_parts(parts, state).await?;
        require_role(Role::Trader, &claims)?;
        Ok(TraderAuth(claims))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_require_role_admin() {
        use super::super::jwt::Claims;

        let admin_claims = Claims::new("admin", "admin", Role::Admin, 60);
        let trader_claims = Claims::new("trader", "trader", Role::Trader, 60);
        let viewer_claims = Claims::new("viewer", "viewer", Role::Viewer, 60);

        // Admin은 모든 역할 접근 가능
        assert!(require_role(Role::Admin, &admin_claims).is_ok());
        assert!(require_role(Role::Trader, &admin_claims).is_ok());
        assert!(require_role(Role::Viewer, &admin_claims).is_ok());

        // Trader는 Admin 접근 불가
        assert!(require_role(Role::Admin, &trader_claims).is_err());
        assert!(require_role(Role::Trader, &trader_claims).is_ok());
        assert!(require_role(Role::Viewer, &trader_claims).is_ok());

        // Viewer는 Viewer만
        assert!(require_role(Role::Admin, &viewer_claims).is_err());
        assert!(require_role(Role::Trader, &viewer_claims).is_err());
        assert!(require_role(Role::Viewer, &viewer_claims).is_ok());
    }

    #[test]
    fn test_jwt_auth_error_responses() {
        let errors = vec![
            JwtAuthError::MissingToken,
            JwtAuthError::InvalidAuthHeader,
            JwtAuthError::TokenExpired,
            JwtAuthError::InvalidToken,
            JwtAuthError::InsufficientPermission,
        ];

        for error in errors {
            let response = error.into_response();
            let status = response.status();

            match status {
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {}
                _ => panic!("Unexpected status code: {}", status),
            }
        }
    }
}
