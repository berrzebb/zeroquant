//! 비밀번호 해싱 유틸리티.
//!
//! Argon2 기반 비밀번호 해싱 및 검증.

#![allow(dead_code)] // 향후 인증 시스템에서 사용 예정

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

/// 비밀번호 처리 에러.
#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("비밀번호 해싱 실패")]
    HashingFailed,
    #[error("비밀번호 검증 실패")]
    VerificationFailed,
    #[error("잘못된 해시 형식")]
    InvalidHashFormat,
}

/// 비밀번호 해싱.
///
/// Argon2id 알고리즘을 사용하여 비밀번호를 해싱합니다.
/// 솔트는 자동으로 생성됩니다.
///
/// # Arguments
///
/// * `password` - 해싱할 평문 비밀번호
///
/// # Returns
///
/// PHC 형식의 해시 문자열 (솔트 포함)
///
/// # Example
///
/// ```rust,ignore
/// let hash = hash_password("my_secure_password").unwrap();
/// // "$argon2id$v=19$m=19456,t=2,p=1$..."
/// ```
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| PasswordError::HashingFailed)?;

    Ok(hash.to_string())
}

/// 비밀번호 검증.
///
/// 저장된 해시와 입력된 비밀번호를 비교합니다.
///
/// # Arguments
///
/// * `password` - 검증할 평문 비밀번호
/// * `hash` - 저장된 PHC 형식 해시
///
/// # Returns
///
/// 비밀번호가 일치하면 Ok(()), 불일치하면 Err
///
/// # Example
///
/// ```rust,ignore
/// let hash = hash_password("my_password").unwrap();
/// assert!(verify_password("my_password", &hash).is_ok());
/// assert!(verify_password("wrong_password", &hash).is_err());
/// ```
pub fn verify_password(password: &str, hash: &str) -> Result<(), PasswordError> {
    let parsed_hash = PasswordHash::new(hash).map_err(|_| PasswordError::InvalidHashFormat)?;

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| PasswordError::VerificationFailed)
}

/// 비밀번호 강도 검증.
///
/// 최소 요구사항을 충족하는지 확인합니다.
///
/// # 요구사항
///
/// - 최소 8자 이상
/// - 최소 1개의 숫자 포함
/// - 최소 1개의 영문자 포함
///
/// # Returns
///
/// 유효하면 Ok(()), 유효하지 않으면 에러 메시지와 함께 Err
pub fn validate_password_strength(password: &str) -> Result<(), &'static str> {
    if password.len() < 8 {
        return Err("비밀번호는 최소 8자 이상이어야 합니다");
    }

    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err("비밀번호에 최소 1개의 숫자가 포함되어야 합니다");
    }

    if !password.chars().any(|c| c.is_ascii_alphabetic()) {
        return Err("비밀번호에 최소 1개의 영문자가 포함되어야 합니다");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let password = "TestPassword123!";
        let hash = hash_password(password).unwrap();

        // 해시 형식 확인 (argon2id)
        assert!(hash.starts_with("$argon2id$"));

        // 올바른 비밀번호 검증
        assert!(verify_password(password, &hash).is_ok());

        // 잘못된 비밀번호 검증
        assert!(verify_password("WrongPassword123!", &hash).is_err());
    }

    #[test]
    fn test_different_passwords_different_hashes() {
        let hash1 = hash_password("Password1").unwrap();
        let hash2 = hash_password("Password1").unwrap();

        // 같은 비밀번호라도 솔트가 다르므로 해시가 다름
        assert_ne!(hash1, hash2);

        // 하지만 둘 다 검증 가능
        assert!(verify_password("Password1", &hash1).is_ok());
        assert!(verify_password("Password1", &hash2).is_ok());
    }

    #[test]
    fn test_invalid_hash_format() {
        let result = verify_password("password", "not-a-valid-hash");
        assert!(matches!(result, Err(PasswordError::InvalidHashFormat)));
    }

    #[test]
    fn test_password_strength_validation() {
        // 유효한 비밀번호
        assert!(validate_password_strength("Password1").is_ok());
        assert!(validate_password_strength("abcd1234").is_ok());
        assert!(validate_password_strength("Complex!Pass99").is_ok());

        // 너무 짧음
        assert!(validate_password_strength("Pass1").is_err());

        // 숫자 없음
        assert!(validate_password_strength("Password").is_err());

        // 영문자 없음
        assert!(validate_password_strength("12345678").is_err());
    }

    #[test]
    fn test_empty_password() {
        assert!(validate_password_strength("").is_err());
    }

    #[test]
    fn test_unicode_password() {
        // 유니코드 비밀번호도 해싱 가능
        let password = "한글패스워드123";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash).is_ok());
    }
}
