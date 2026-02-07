//! # 암호화 모듈
//!
//! AES-256-GCM을 사용한 자격증명 암호화/복호화 기능을 제공합니다.
//!
//! ## 보안 고려사항
//! - 마스터 키는 환경변수 또는 보안 저장소에서 로드
//! - 각 암호화마다 고유한 nonce (12바이트) 사용
//! - 암호화된 데이터와 nonce를 함께 저장

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use secrecy::SecretString;
use thiserror::Error;

/// 암호화 에러
#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Invalid master key length: expected 32 bytes, got {0}")]
    InvalidKeyLength(usize),

    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Invalid nonce length: expected 12 bytes, got {0}")]
    InvalidNonceLength(usize),

    #[error("Base64 decode error: {0}")]
    Base64DecodeError(#[from] base64::DecodeError),

    #[error("UTF-8 decode error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("Master key not configured")]
    MasterKeyNotConfigured,
}

/// AES-256-GCM nonce 크기 (바이트)
pub const NONCE_SIZE: usize = 12;

/// AES-256 키 크기 (바이트)
pub const KEY_SIZE: usize = 32;

/// 자격증명 암호화 관리자
pub struct CredentialEncryptor {
    cipher: Aes256Gcm,
}

impl CredentialEncryptor {
    /// 마스터 키로 암호화 관리자 생성
    ///
    /// # Arguments
    /// * `master_key` - 32바이트 마스터 키 (환경변수에서 로드)
    ///
    /// # Example
    /// ```ignore
    /// let key = std::env::var("ENCRYPTION_MASTER_KEY")?;
    /// let encryptor = CredentialEncryptor::new(&key)?;
    /// ```
    pub fn new(master_key: &str) -> Result<Self, CryptoError> {
        let key_bytes = Self::decode_key(master_key)?;
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

        Ok(Self { cipher })
    }

    /// Base64로 인코딩된 마스터 키 디코드
    fn decode_key(master_key: &str) -> Result<Vec<u8>, CryptoError> {
        use base64::Engine;
        let key_bytes = base64::engine::general_purpose::STANDARD.decode(master_key)?;

        if key_bytes.len() != KEY_SIZE {
            return Err(CryptoError::InvalidKeyLength(key_bytes.len()));
        }

        Ok(key_bytes)
    }

    /// 랜덤 nonce 생성
    pub fn generate_nonce() -> [u8; NONCE_SIZE] {
        let mut nonce = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce);
        nonce
    }

    /// 문자열 암호화
    ///
    /// # Returns
    /// * `(encrypted_data, nonce)` - 암호화된 데이터와 사용된 nonce
    pub fn encrypt(&self, plaintext: &str) -> Result<(Vec<u8>, [u8; NONCE_SIZE]), CryptoError> {
        let nonce_bytes = Self::generate_nonce();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

        Ok((ciphertext, nonce_bytes))
    }

    /// 암호화된 데이터 복호화
    pub fn decrypt(&self, ciphertext: &[u8], nonce: &[u8]) -> Result<String, CryptoError> {
        if nonce.len() != NONCE_SIZE {
            return Err(CryptoError::InvalidNonceLength(nonce.len()));
        }

        let nonce = Nonce::from_slice(nonce);

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))?;

        String::from_utf8(plaintext).map_err(CryptoError::from)
    }

    /// JSON 암호화 (자격증명 구조체용)
    pub fn encrypt_json<T: serde::Serialize>(
        &self,
        data: &T,
    ) -> Result<(Vec<u8>, [u8; NONCE_SIZE]), CryptoError> {
        let json = serde_json::to_string(data)
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;
        self.encrypt(&json)
    }

    /// 암호화된 JSON 복호화
    pub fn decrypt_json<T: serde::de::DeserializeOwned>(
        &self,
        ciphertext: &[u8],
        nonce: &[u8],
    ) -> Result<T, CryptoError> {
        let json = self.decrypt(ciphertext, nonce)?;
        serde_json::from_str(&json).map_err(|e| CryptoError::DecryptionFailed(e.to_string()))
    }
}

/// 새로운 마스터 키 생성 (초기 설정용)
///
/// # Example
/// ```
/// let key = trader_core::crypto::generate_master_key();
/// println!("ENCRYPTION_MASTER_KEY={}", key);
/// ```
pub fn generate_master_key() -> String {
    use base64::Engine;
    let mut key = [0u8; KEY_SIZE];
    OsRng.fill_bytes(&mut key);
    base64::engine::general_purpose::STANDARD.encode(key)
}

/// 거래소 자격증명 구조체
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExchangeCredentials {
    pub api_key: String,
    pub api_secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional: Option<std::collections::HashMap<String, String>>,
}

impl ExchangeCredentials {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self {
            api_key,
            api_secret,
            passphrase: None,
            additional: None,
        }
    }

    pub fn with_passphrase(mut self, passphrase: String) -> Self {
        self.passphrase = Some(passphrase);
        self
    }
}

/// SecretString에서 안전하게 자격증명 생성
impl From<ExchangeCredentials> for SecretString {
    fn from(creds: ExchangeCredentials) -> Self {
        SecretString::new(serde_json::to_string(&creds).unwrap_or_default().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_encryptor() -> CredentialEncryptor {
        let key = generate_master_key();
        CredentialEncryptor::new(&key).unwrap()
    }

    #[test]
    fn test_encrypt_decrypt_string() {
        let encryptor = test_encryptor();
        let plaintext = "my-secret-api-key-12345";

        let (ciphertext, nonce) = encryptor.encrypt(plaintext).unwrap();
        let decrypted = encryptor.decrypt(&ciphertext, &nonce).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_encrypt_decrypt_json() {
        let encryptor = test_encryptor();
        let creds = ExchangeCredentials::new("api_key_123".to_string(), "secret_456".to_string())
            .with_passphrase("pass_789".to_string());

        let (ciphertext, nonce) = encryptor.encrypt_json(&creds).unwrap();
        let decrypted: ExchangeCredentials = encryptor.decrypt_json(&ciphertext, &nonce).unwrap();

        assert_eq!(creds.api_key, decrypted.api_key);
        assert_eq!(creds.api_secret, decrypted.api_secret);
        assert_eq!(creds.passphrase, decrypted.passphrase);
    }

    #[test]
    fn test_invalid_key_length() {
        use base64::Engine;
        let short_key = base64::engine::general_purpose::STANDARD.encode([0u8; 16]);
        let result = CredentialEncryptor::new(&short_key);
        assert!(matches!(result, Err(CryptoError::InvalidKeyLength(16))));
    }

    #[test]
    fn test_wrong_nonce_fails() {
        let encryptor = test_encryptor();
        let plaintext = "test";

        let (ciphertext, _nonce) = encryptor.encrypt(plaintext).unwrap();
        let wrong_nonce = [0u8; NONCE_SIZE];

        let result = encryptor.decrypt(&ciphertext, &wrong_nonce);
        assert!(matches!(result, Err(CryptoError::DecryptionFailed(_))));
    }

    #[test]
    fn test_generate_master_key() {
        let key1 = generate_master_key();
        let key2 = generate_master_key();

        // 키가 서로 다름 (랜덤)
        assert_ne!(key1, key2);

        // 키 길이 검증 (32바이트 = ~44 Base64 문자)
        assert!(key1.len() >= 40);

        // 생성된 키로 encryptor 생성 가능
        assert!(CredentialEncryptor::new(&key1).is_ok());
    }
}
