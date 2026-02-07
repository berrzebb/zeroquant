//! 한국투자증권 (KIS) API 설정.
//!
//! KIS API는 app_key와 app_secret을 사용한 OAuth 2.0 인증이 필요합니다.
//! 다중 계좌 지원:
//! - 모의투자
//! - 실전투자 일반
//! - 실전투자 ISA

use serde::{Deserialize, Serialize};
use std::fmt;

/// KIS API 환경 유형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum KisEnvironment {
    /// 실전투자
    Real,
    /// 모의투자
    #[default]
    Paper,
}

/// KIS 계좌 유형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum KisAccountType {
    /// 모의투자
    #[default]
    Paper,
    /// 실전투자 일반
    RealGeneral,
    /// 실전투자 ISA
    RealIsa,
}

impl KisAccountType {
    /// 이 계좌 유형의 환경 반환.
    pub fn environment(&self) -> KisEnvironment {
        match self {
            KisAccountType::Paper => KisEnvironment::Paper,
            KisAccountType::RealGeneral | KisAccountType::RealIsa => KisEnvironment::Real,
        }
    }

    /// 이 계좌 유형의 표시 이름 반환.
    pub fn display_name(&self) -> &'static str {
        match self {
            KisAccountType::Paper => "모의투자",
            KisAccountType::RealGeneral => "실전투자(일반)",
            KisAccountType::RealIsa => "실전투자(ISA)",
        }
    }

    /// 문자열에서 파싱.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "paper" | "mock" | "test" => Some(KisAccountType::Paper),
            "real_general" | "general" | "real" => Some(KisAccountType::RealGeneral),
            "real_isa" | "isa" => Some(KisAccountType::RealIsa),
            _ => None,
        }
    }
}


impl KisEnvironment {
    /// 이 환경의 REST API 기본 URL 반환.
    pub fn rest_base_url(&self) -> &str {
        match self {
            KisEnvironment::Real => "https://openapi.koreainvestment.com:9443",
            KisEnvironment::Paper => "https://openapivts.koreainvestment.com:29443",
        }
    }

    /// 이 환경의 WebSocket URL 반환.
    pub fn websocket_url(&self) -> &str {
        match self {
            KisEnvironment::Real => "ws://ops.koreainvestment.com:21000",
            KisEnvironment::Paper => "ws://ops.koreainvestment.com:31000",
        }
    }
}


/// KIS API 설정.
///
/// # 보안
/// - `Debug` 구현은 민감 정보(`app_key`, `app_secret`)를 마스킹합니다.
#[derive(Clone, Serialize, Deserialize)]
pub struct KisConfig {
    /// 앱키
    pub app_key: String,
    /// 앱시크릿
    pub app_secret: String,
    /// 계좌번호 - 형식: "XXXXXXXX-XX"
    pub account_no: String,
    /// 계좌상품코드 - 주식의 경우 일반적으로 "01"
    pub account_product_code: String,
    /// 계좌 유형 (모의투자/실전일반/실전ISA)
    pub account_type: KisAccountType,
    /// 환경 (실전/모의) - account_type에서 파생
    pub environment: KisEnvironment,
    /// HTS ID (실시간 시세 수신에 필요)
    pub hts_id: Option<String>,
    /// 요청 타임아웃 (초)
    pub timeout_secs: u64,
    /// 개인인증 활성화 (일부 엔드포인트에 필요)
    pub personalized: bool,
}

impl fmt::Debug for KisConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 앱키/시크릿 마스킹
        let masked_key = if self.app_key.len() > 8 {
            format!(
                "{}...{}",
                &self.app_key[..4],
                &self.app_key[self.app_key.len() - 4..]
            )
        } else {
            "***REDACTED***".to_string()
        };
        let masked_account = if self.account_no.len() > 6 {
            format!("{}****", &self.account_no[..4])
        } else {
            "***REDACTED***".to_string()
        };

        f.debug_struct("KisConfig")
            .field("app_key", &masked_key)
            .field("app_secret", &"***REDACTED***")
            .field("account_no", &masked_account)
            .field("account_product_code", &self.account_product_code)
            .field("account_type", &self.account_type)
            .field("environment", &self.environment)
            .field("hts_id", &self.hts_id)
            .field("timeout_secs", &self.timeout_secs)
            .field("personalized", &self.personalized)
            .finish()
    }
}

impl KisConfig {
    /// 새로운 KIS 설정 생성.
    pub fn new(
        app_key: String,
        app_secret: String,
        account_no: String,
        account_type: KisAccountType,
    ) -> Self {
        Self {
            app_key,
            app_secret,
            account_no,
            account_product_code: "01".to_string(),
            account_type,
            environment: account_type.environment(),
            hts_id: None,
            timeout_secs: 30,
            personalized: false,
        }
    }

    /// 계좌 유형 설정 및 환경 자동 업데이트.
    pub fn with_account_type(mut self, account_type: KisAccountType) -> Self {
        self.account_type = account_type;
        self.environment = account_type.environment();
        self
    }

    /// 환경 직접 설정 (account_type 기본값 무시).
    pub fn with_environment(mut self, env: KisEnvironment) -> Self {
        self.environment = env;
        self
    }

    /// 계좌상품코드 설정.
    pub fn with_product_code(mut self, code: String) -> Self {
        self.account_product_code = code;
        self
    }

    /// HTS ID 설정.
    pub fn with_hts_id(mut self, hts_id: String) -> Self {
        self.hts_id = Some(hts_id);
        self
    }

    /// 개인인증 활성화.
    pub fn with_personalized(mut self, enabled: bool) -> Self {
        self.personalized = enabled;
        self
    }

    /// 환경 변수에서 특정 계좌 유형의 설정 생성.
    ///
    /// # 인자
    /// * `account_type` - 로드할 계좌 유형 (Paper, RealGeneral, RealIsa)
    ///
    /// # 환경 변수
    /// - Paper: KIS_PAPER_APP_KEY, KIS_PAPER_APP_SECRET, KIS_PAPER_ACCOUNT_NUMBER, KIS_PAPER_ACCOUNT_CODE
    /// - RealGeneral: KIS_REAL_GENERAL_APP_KEY, KIS_REAL_GENERAL_APP_SECRET 등
    /// - RealIsa: KIS_REAL_ISA_APP_KEY, KIS_REAL_ISA_APP_SECRET 등
    /// - 공통: KIS_HTS_ID
    pub fn from_env_for_account(account_type: KisAccountType) -> Option<Self> {
        let prefix = match account_type {
            KisAccountType::Paper => "KIS_PAPER",
            KisAccountType::RealGeneral => "KIS_REAL_GENERAL",
            KisAccountType::RealIsa => "KIS_REAL_ISA",
        };

        let app_key = std::env::var(format!("{}_APP_KEY", prefix)).ok()?;
        let app_secret = std::env::var(format!("{}_APP_SECRET", prefix)).ok()?;
        let account_no = std::env::var(format!("{}_ACCOUNT_NUMBER", prefix)).ok()?;
        let account_product_code =
            std::env::var(format!("{}_ACCOUNT_CODE", prefix)).unwrap_or_else(|_| "01".to_string());
        let hts_id = std::env::var("KIS_HTS_ID").ok();

        Some(Self {
            app_key,
            app_secret,
            account_no,
            account_product_code,
            account_type,
            environment: account_type.environment(),
            hts_id,
            timeout_secs: 30,
            personalized: false,
        })
    }

    /// KIS_DEFAULT_ACCOUNT 환경 변수를 사용하여 설정 생성.
    ///
    /// # 환경 변수
    /// - KIS_DEFAULT_ACCOUNT: "paper" | "real_general" | "real_isa" (기본값: paper)
    pub fn from_env() -> Option<Self> {
        let default_account = std::env::var("KIS_DEFAULT_ACCOUNT")
            .ok()
            .and_then(|s| KisAccountType::parse(&s))
            .unwrap_or(KisAccountType::Paper);

        Self::from_env_for_account(default_account)
    }

    /// 환경 변수에서 설정된 모든 계좌 로드.
    ///
    /// 유효한 인증 정보가 설정된 모든 계좌에 대해
    /// (account_type, config) 쌍의 벡터를 반환합니다.
    pub fn load_all_accounts_from_env() -> Vec<(KisAccountType, Self)> {
        let mut accounts = Vec::new();

        for account_type in [
            KisAccountType::Paper,
            KisAccountType::RealGeneral,
            KisAccountType::RealIsa,
        ] {
            if let Some(config) = Self::from_env_for_account(account_type) {
                accounts.push((account_type, config));
            }
        }

        accounts
    }

    /// REST API 기본 URL 반환.
    pub fn rest_base_url(&self) -> &str {
        self.environment.rest_base_url()
    }

    /// WebSocket URL 반환.
    pub fn websocket_url(&self) -> &str {
        self.environment.websocket_url()
    }

    /// 하이픈 없는 계좌번호 반환 (API 호출용).
    pub fn account_no_plain(&self) -> String {
        self.account_no.replace("-", "")
    }

    /// 계좌번호 앞 8자리 반환 (CANO).
    pub fn cano(&self) -> &str {
        let plain = self.account_no.replace("-", "");
        if plain.len() >= 8 {
            &self.account_no[..8]
        } else {
            &self.account_no
        }
    }

    /// 계좌상품코드 반환 (ACNT_PRDT_CD).
    pub fn acnt_prdt_cd(&self) -> &str {
        &self.account_product_code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = KisConfig::new(
            "test_key".to_string(),
            "test_secret".to_string(),
            "12345678-01".to_string(),
            KisAccountType::Paper,
        );

        assert_eq!(config.app_key, "test_key");
        assert_eq!(config.environment, KisEnvironment::Paper);
        assert_eq!(config.account_type, KisAccountType::Paper);
        assert_eq!(config.account_product_code, "01");
    }

    #[test]
    fn test_account_types() {
        assert_eq!(KisAccountType::Paper.environment(), KisEnvironment::Paper);
        assert_eq!(
            KisAccountType::RealGeneral.environment(),
            KisEnvironment::Real
        );
        assert_eq!(KisAccountType::RealIsa.environment(), KisEnvironment::Real);

        assert_eq!(KisAccountType::Paper.display_name(), "모의투자");
        assert_eq!(KisAccountType::RealGeneral.display_name(), "실전투자(일반)");
        assert_eq!(KisAccountType::RealIsa.display_name(), "실전투자(ISA)");
    }

    #[test]
    fn test_account_type_parsing() {
        assert_eq!(
            KisAccountType::parse("paper"),
            Some(KisAccountType::Paper)
        );
        assert_eq!(
            KisAccountType::parse("real_general"),
            Some(KisAccountType::RealGeneral)
        );
        assert_eq!(
            KisAccountType::parse("real_isa"),
            Some(KisAccountType::RealIsa)
        );
        assert_eq!(
            KisAccountType::parse("isa"),
            Some(KisAccountType::RealIsa)
        );
        assert_eq!(KisAccountType::parse("invalid"), None);
    }

    #[test]
    fn test_environment_urls() {
        assert_eq!(
            KisEnvironment::Real.rest_base_url(),
            "https://openapi.koreainvestment.com:9443"
        );
        assert_eq!(
            KisEnvironment::Paper.rest_base_url(),
            "https://openapivts.koreainvestment.com:29443"
        );
    }

    #[test]
    fn test_account_parsing() {
        let config = KisConfig::new(
            "key".to_string(),
            "secret".to_string(),
            "12345678-01".to_string(),
            KisAccountType::RealGeneral,
        );

        assert_eq!(config.account_no_plain(), "1234567801");
        assert_eq!(config.cano(), "12345678");
        assert_eq!(config.acnt_prdt_cd(), "01");
        assert_eq!(config.environment, KisEnvironment::Real);
    }

    #[test]
    fn test_config_with_hts_id() {
        let config = KisConfig::new(
            "key".to_string(),
            "secret".to_string(),
            "12345678-01".to_string(),
            KisAccountType::Paper,
        )
        .with_hts_id("my_hts_id".to_string());

        assert_eq!(config.hts_id, Some("my_hts_id".to_string()));
    }
}
