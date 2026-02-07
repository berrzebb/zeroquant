//! 마이그레이션 분석 및 검증 도구.
//!
//! SQL 마이그레이션 파일을 파싱하고, 의존성 그래프를 생성하며,
//! 중복 정의, DROP CASCADE, 순환 의존성 등의 문제를 검출합니다.
//!
//! # 사용 예시
//!
//! ```ignore
//! use trader_core::migration::{MigrationAnalyzer, MigrationValidator};
//!
//! let analyzer = MigrationAnalyzer::new();
//! let files = analyzer.scan_directory("migrations")?;
//! let validator = MigrationValidator::new(&files);
//! let report = validator.validate()?;
//! ```

pub mod analyzer;
pub mod consolidator;
pub mod models;
pub mod validator;

pub use analyzer::MigrationAnalyzer;
pub use consolidator::MigrationConsolidator;
pub use models::*;
pub use validator::{MigrationValidator, generate_safety_checklist};
