//! 인증 및 권한 부여.
//!
//! JWT 기반 인증 및 역할 기반 접근 제어(RBAC)를 제공합니다.
//!
//! # 구성 요소
//!
//! - [`Claims`]: JWT 페이로드 구조체
//! - [`Role`]: 사용자 역할 (Admin, Trader, Viewer)
//! - [`JwtAuth`]: Axum 미들웨어용 JWT 검증 추출기
//! - 토큰 생성/검증 함수
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! // 보호된 라우트에서 JwtAuth 추출기 사용
//! async fn protected_handler(
//!     JwtAuth(claims): JwtAuth,
//! ) -> impl IntoResponse {
//!     format!("Hello, {}!", claims.sub)
//! }
//! ```

mod jwt;
mod middleware;
mod password;
mod roles;

pub use jwt::{create_token, decode_token, Claims, RefreshClaims, TokenPair};
pub use middleware::{require_role, JwtAuth, JwtAuthError};
pub use password::{hash_password, verify_password, PasswordError};
pub use roles::{Permission, Role};
