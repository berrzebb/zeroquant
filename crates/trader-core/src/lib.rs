//! # Trader Core
//!
//! 트레이딩 봇의 핵심 도메인 모델 및 타입을 제공합니다.
//!
//! 이 크레이트는 트레이딩 시스템 전반에서 사용되는 기본 타입을 제공합니다:
//! - 주문 및 주문 관리 타입
//! - 포지션 추적
//! - 거래 기록
//! - 시장 데이터 구조체
//! - 심볼 및 시장 유형 정의
//! - 설정 관리
//! - 로깅 인프라
//! - 자격증명 암호화

pub mod cache;
pub mod config;
pub mod crypto;
pub mod domain;
pub mod error;
pub mod logging;
pub mod migration;
pub mod types;

pub use cache::{ExchangeCache, ExchangeCacheConfig, TtlCache};
pub use config::*;
pub use crypto::{CredentialEncryptor, CryptoError, ExchangeCredentials};
pub use domain::*;
pub use error::*;
pub use logging::*;
pub use types::*;
