//! 마이그레이션 통합기.
//!
//! 여러 마이그레이션 파일을 논리적 그룹으로 통합하고,
//! 안전한 마이그레이션 SQL을 생성합니다.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::fs;

use super::models::*;

/// 통합 대상 파일 그룹
#[derive(Debug, Clone, Default)]
pub struct ConsolidationGroup {
    /// 그룹 이름 (생성될 파일명)
    pub name: String,
    /// 설명
    pub description: String,
    /// 포함할 원본 파일 패턴 (순서대로)
    pub source_patterns: Vec<String>,
    /// 원본 파일 없이 직접 포함할 SQL (신규 스키마 등)
    pub static_content: Option<String>,
}


/// 기본 통합 그룹 정의
pub fn default_consolidation_groups() -> Vec<ConsolidationGroup> {
    vec![
        ConsolidationGroup {
            name: "01_core_foundation".to_string(),
            description: "Extensions, ENUM, symbols, credentials".to_string(),
            source_patterns: vec!["01_".to_string()],
            static_content: None,
        },
        ConsolidationGroup {
            name: "02_data_management".to_string(),
            description: "symbol_info, ohlcv, fundamental, v_symbol_with_fundamental".to_string(),
            source_patterns: vec!["02_".to_string(), "18_".to_string()],
            static_content: None,
        },
        ConsolidationGroup {
            name: "03_trading_analytics".to_string(),
            description: "trade_executions, position_snapshots, 분석 뷰".to_string(),
            source_patterns: vec!["03_".to_string()],
            static_content: None,
        },
        ConsolidationGroup {
            name: "04_strategy_signals".to_string(),
            description: "signal_marker, alert_rule, alert_history".to_string(),
            source_patterns: vec!["04_".to_string(), "14_".to_string(), "15_".to_string(), "16_".to_string()],
            static_content: None,
        },
        ConsolidationGroup {
            name: "05_evaluation_ranking".to_string(),
            description: "global_score, reality_check, score_history".to_string(),
            source_patterns: vec!["05_".to_string()],
            static_content: None,
        },
        ConsolidationGroup {
            name: "06_user_settings".to_string(),
            description: "watchlist, preset, notification, checkpoint".to_string(),
            source_patterns: vec!["06_".to_string(), "11_".to_string(), "12_".to_string(), "17_".to_string()],
            static_content: None,
        },
        ConsolidationGroup {
            name: "07_performance_optimization".to_string(),
            description: "인덱스, MV, Hypertable 정책".to_string(),
            source_patterns: vec!["07_".to_string(), "08_".to_string(), "19_".to_string()],
            static_content: None,
        },
        ConsolidationGroup {
            name: "08_paper_trading".to_string(),
            description: "Mock 거래소, 전략-계정 연결, Paper Trading 세션".to_string(),
            source_patterns: vec!["20_".to_string(), "21_".to_string(), "22_".to_string(), "24_".to_string()],
            static_content: None,
        },
        ConsolidationGroup {
            name: "09_strategy_watched_tickers".to_string(),
            description: "전략별 관심 종목, Collector 우선순위 연동".to_string(),
            source_patterns: vec![],
            static_content: Some(STRATEGY_WATCHED_TICKERS_SQL.to_string()),
        },
        ConsolidationGroup {
            name: "10_symbol_cascade".to_string(),
            description: "Symbol 연쇄 삭제 + 고아 데이터 정리 DB 함수".to_string(),
            source_patterns: vec!["23_".to_string()],
            static_content: None,
        },
    ]
}

/// 09_strategy_watched_tickers 정적 SQL (원본 마이그레이션에 없는 신규 스키마)
const STRATEGY_WATCHED_TICKERS_SQL: &str = r#"-- 전략이 관심을 가지는 종목 목록.
-- 고정 티커(config)와 동적 티커(스크리닝/유니버스)를 모두 지원합니다.
-- Collector가 이 테이블을 읽어 해당 종목의 OHLCV/지표/스코어 데이터를
-- 우선적으로 업데이트합니다.
CREATE TABLE IF NOT EXISTS strategy_watched_tickers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id VARCHAR(100) NOT NULL,
    ticker VARCHAR(50) NOT NULL,
    source VARCHAR(20) NOT NULL DEFAULT 'config',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT strategy_watched_tickers_unique UNIQUE (strategy_id, ticker)
);

CREATE INDEX IF NOT EXISTS idx_strategy_watched_tickers_strategy
    ON strategy_watched_tickers(strategy_id);

CREATE INDEX IF NOT EXISTS idx_strategy_watched_tickers_ticker
    ON strategy_watched_tickers(ticker);

COMMENT ON TABLE strategy_watched_tickers IS '전략별 관심 종목 (Collector 우선순위 연동)';
COMMENT ON COLUMN strategy_watched_tickers.strategy_id IS '전략 ID';
COMMENT ON COLUMN strategy_watched_tickers.ticker IS '종목 코드';
COMMENT ON COLUMN strategy_watched_tickers.source IS '출처: config(고정), dynamic(스크리닝/유니버스)';"#;

/// 마이그레이션 통합기
pub struct MigrationConsolidator {
    /// 통합 그룹 정의
    groups: Vec<ConsolidationGroup>,
    /// 제외할 파일 패턴 (레거시 삭제/복원 등)
    exclude_patterns: Vec<String>,
}

impl Default for MigrationConsolidator {
    fn default() -> Self {
        Self::new()
    }
}

impl MigrationConsolidator {
    /// 새 통합기 생성
    pub fn new() -> Self {
        Self {
            groups: default_consolidation_groups(),
            exclude_patterns: vec![
                "09_".to_string(),  // remove_legacy_tables
                "10_".to_string(),  // restore_used_tables
                "13_".to_string(),  // missing_views (중복)
            ],
        }
    }

    /// 커스텀 그룹으로 생성
    pub fn with_groups(groups: Vec<ConsolidationGroup>) -> Self {
        Self {
            groups,
            exclude_patterns: Vec::new(),
        }
    }

    /// 제외 패턴 추가
    pub fn exclude_pattern(&mut self, pattern: &str) {
        self.exclude_patterns.push(pattern.to_string());
    }

    /// 통합 계획 생성
    pub fn plan(&self, files: &[MigrationFile]) -> ConsolidationPlan {
        let mut plan = ConsolidationPlan::new();

        // 원본 파일 라인 수 계산
        plan.original_lines = files.iter().map(|f| f.content.lines().count()).sum();

        // 사용된 파일 추적
        let mut used_files: HashSet<String> = HashSet::new();

        // 각 그룹별 통합 파일 생성
        for group in &self.groups {
            let mut sources: Vec<(String, Vec<String>)> = Vec::new();
            let mut combined_content = String::new();

            // 헤더 추가
            combined_content.push_str("-- =============================================================================\n");
            combined_content.push_str(&format!("-- {}\n", group.name));
            combined_content.push_str(&format!("-- {}\n", group.description));
            combined_content.push_str("-- =============================================================================\n");
            combined_content.push_str("-- 통합 마이그레이션 파일 (자동 생성)\n");
            combined_content.push_str(&format!("-- 원본 파일: {:?}\n", group.source_patterns));
            combined_content.push_str("-- =============================================================================\n\n");

            // 매칭되는 파일들 수집
            for file in files {
                // 제외 패턴 확인
                if self.exclude_patterns.iter().any(|p| file.name.starts_with(p)) {
                    continue;
                }

                // 그룹 패턴 매칭
                if group.source_patterns.iter().any(|p| file.name.starts_with(p)) {
                    used_files.insert(file.name.clone());

                    // 파일 내용 정리 및 추가
                    let cleaned = self.clean_file_content(file);

                    if !cleaned.is_empty() {
                        combined_content.push_str("-- ---------------------------------------------------------------------------\n");
                        combined_content.push_str(&format!("-- Source: {}\n", file.name));
                        combined_content.push_str("-- ---------------------------------------------------------------------------\n\n");
                        combined_content.push_str(&cleaned);
                        combined_content.push_str("\n\n");

                        sources.push((file.name.clone(), vec![cleaned]));
                    }
                }
            }

            if !sources.is_empty() {
                plan.files.push(ConsolidationFile {
                    name: format!("{}.sql", group.name),
                    description: group.description.clone(),
                    sources,
                    content: combined_content,
                });
            } else if let Some(static_sql) = &group.static_content {
                // 원본 파일 없이 정적 콘텐츠로 생성 (신규 스키마)
                combined_content.push_str(static_sql);
                combined_content.push('\n');
                plan.files.push(ConsolidationFile {
                    name: format!("{}.sql", group.name),
                    description: group.description.clone(),
                    sources: vec![("(static)".to_string(), vec![static_sql.clone()])],
                    content: combined_content,
                });
            }
        }

        // 사용된 파일 목록
        plan.files_to_remove = files
            .iter()
            .map(|f| f.name.clone())
            .filter(|n| used_files.contains(n) || self.exclude_patterns.iter().any(|p| n.starts_with(p)))
            .collect();

        // 통합 후 라인 수
        plan.consolidated_lines = plan.files.iter().map(|f| f.content.lines().count()).sum();

        plan
    }

    /// 파일 내용 정리 (중복 제거, 멱등성 보장)
    fn clean_file_content(&self, file: &MigrationFile) -> String {
        let mut result = String::new();
        let mut seen_creates: HashSet<String> = HashSet::new();

        for stmt in &file.statements {
            let obj_lower = stmt.object_name.to_lowercase();

            // DROP 문은 통합 시 제외 (IF NOT EXISTS로 대체)
            if stmt.statement_type.is_drop() {
                continue;
            }

            // 중복 CREATE 방지
            if stmt.statement_type.is_create() && !obj_lower.is_empty() {
                if seen_creates.contains(&obj_lower) {
                    continue;
                }
                seen_creates.insert(obj_lower);
            }

            // 멱등성 보장을 위한 SQL 수정
            let modified_sql = self.ensure_idempotency(stmt);
            result.push_str(&modified_sql);
            result.push_str("\n\n");
        }

        result.trim().to_string()
    }

    /// 멱등성 보장을 위한 SQL 수정
    fn ensure_idempotency(&self, stmt: &SqlStatement) -> String {
        let sql = stmt.raw_sql.trim().to_string();

        match &stmt.statement_type {
            StatementType::CreateTable => {
                if !stmt.if_not_exists {
                    // CREATE TABLE → CREATE TABLE IF NOT EXISTS
                    let sql_upper = sql.to_uppercase();
                    if let Some(pos) = sql_upper.find("CREATE TABLE") {
                        let insert_pos = pos + "CREATE TABLE".len();
                        let mut modified = sql.clone();
                        modified.insert_str(insert_pos, " IF NOT EXISTS");
                        return modified;
                    }
                }
            }
            StatementType::CreateIndex => {
                if !stmt.if_not_exists {
                    let sql_upper = sql.to_uppercase();
                    // CREATE INDEX → CREATE INDEX IF NOT EXISTS
                    // CREATE UNIQUE INDEX → CREATE UNIQUE INDEX IF NOT EXISTS
                    if let Some(pos) = sql_upper.find("CREATE UNIQUE INDEX") {
                        let insert_pos = pos + "CREATE UNIQUE INDEX".len();
                        let mut modified = sql.clone();
                        modified.insert_str(insert_pos, " IF NOT EXISTS");
                        return modified;
                    } else if let Some(pos) = sql_upper.find("CREATE INDEX") {
                        let insert_pos = pos + "CREATE INDEX".len();
                        let mut modified = sql.clone();
                        modified.insert_str(insert_pos, " IF NOT EXISTS");
                        return modified;
                    }
                }
            }
            StatementType::CreateType => {
                // pg_type 존재 확인 + ALTER TYPE ADD VALUE IF NOT EXISTS 패턴
                if !sql.to_uppercase().contains("DO $$") && !stmt.if_not_exists {
                    return self.generate_enum_idempotent_sql(&sql, &stmt.object_name);
                }
            }
            StatementType::CreateView => {
                // CREATE VIEW → CREATE OR REPLACE VIEW
                let sql_upper = sql.to_uppercase();
                if !sql_upper.contains("OR REPLACE") {
                    if let Some(pos) = sql_upper.find("CREATE VIEW") {
                        let insert_pos = pos + "CREATE".len();
                        let mut modified = sql.clone();
                        modified.insert_str(insert_pos, " OR REPLACE");
                        return modified;
                    }
                }
            }
            StatementType::CreateFunction => {
                // CREATE FUNCTION → CREATE OR REPLACE FUNCTION
                let sql_upper = sql.to_uppercase();
                if !sql_upper.contains("OR REPLACE") {
                    if let Some(pos) = sql_upper.find("CREATE FUNCTION") {
                        let insert_pos = pos + "CREATE".len();
                        let mut modified = sql.clone();
                        modified.insert_str(insert_pos, " OR REPLACE");
                        return modified;
                    }
                }
            }
            StatementType::CreateExtension => {
                if !stmt.if_not_exists {
                    let sql_upper = sql.to_uppercase();
                    if let Some(pos) = sql_upper.find("CREATE EXTENSION") {
                        let insert_pos = pos + "CREATE EXTENSION".len();
                        let mut modified = sql.clone();
                        modified.insert_str(insert_pos, " IF NOT EXISTS");
                        return modified;
                    }
                }
            }
            _ => {}
        }

        sql
    }

    /// ENUM 타입의 멱등성 SQL 생성
    /// pg_type으로 존재 확인 후 CREATE, 그리고 ALTER TYPE ADD VALUE IF NOT EXISTS
    fn generate_enum_idempotent_sql(&self, sql: &str, type_name: &str) -> String {
        let create_sql = sql.trim_end_matches(';');

        // ENUM 값 추출 시도
        let enum_values = self.extract_enum_values(sql);

        let mut result = format!(
            "DO $$\nBEGIN\n    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = '{}') THEN\n        {};\n    END IF;\nEND $$;",
            type_name, create_sql
        );

        // ENUM 값이 있으면 ALTER TYPE ADD VALUE IF NOT EXISTS 추가
        if !enum_values.is_empty() {
            result.push_str("\n\n-- Ensure all values exist (for upgrades)");
            for val in &enum_values {
                result.push_str(&format!(
                    "\nALTER TYPE {} ADD VALUE IF NOT EXISTS '{}';",
                    type_name, val
                ));
            }
        }

        result
    }

    /// SQL에서 ENUM 값 추출
    fn extract_enum_values(&self, sql: &str) -> Vec<String> {
        // "AS ENUM ('val1', 'val2', ...)" 패턴에서 값 추출
        let sql_upper = sql.to_uppercase();
        if let Some(enum_pos) = sql_upper.find("AS ENUM") {
            let after_enum = &sql[enum_pos..];
            if let Some(paren_start) = after_enum.find('(') {
                if let Some(paren_end) = after_enum.find(')') {
                    let values_str = &after_enum[paren_start + 1..paren_end];
                    return values_str
                        .split(',')
                        .map(|v| v.trim().trim_matches('\'').trim().to_string())
                        .filter(|v| !v.is_empty())
                        .collect();
                }
            }
        }
        Vec::new()
    }

    /// 통합 파일을 디렉토리에 저장
    pub fn execute(&self, plan: &ConsolidationPlan, output_dir: &Path) -> Result<(), String> {
        // 출력 디렉토리 생성
        fs::create_dir_all(output_dir)
            .map_err(|e| format!("디렉토리 생성 실패: {}", e))?;

        // 각 통합 파일 저장
        for file in &plan.files {
            let file_path = output_dir.join(&file.name);
            fs::write(&file_path, &file.content)
                .map_err(|e| format!("파일 저장 실패 {:?}: {}", file_path, e))?;
        }

        Ok(())
    }

    /// Dry-run 결과 출력
    pub fn dry_run(&self, plan: &ConsolidationPlan) -> String {
        let mut output = String::new();

        output.push_str(&format!("{}", plan));

        output.push_str("\n\n");
        output.push_str("📄 생성될 파일 미리보기 (처음 50줄)\n");
        output.push_str("───────────────────────────────────────────────────────────────\n");

        for file in &plan.files {
            output.push_str(&format!("\n### {} ###\n", file.name));
            for (i, line) in file.content.lines().take(50).enumerate() {
                output.push_str(&format!("{:4} | {}\n", i + 1, line));
            }
            if file.content.lines().count() > 50 {
                output.push_str("      ... (생략)\n");
            }
        }

        output
    }
}

/// 데이터 보존 마이그레이션 SQL 생성
///
/// 기존 데이터를 유지하면서 스키마를 변경하는 SQL을 생성합니다.
pub struct SafeMigrationBuilder {
    statements: Vec<String>,
}

impl Default for SafeMigrationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SafeMigrationBuilder {
    /// 새 빌더 생성
    pub fn new() -> Self {
        Self {
            statements: Vec::new(),
        }
    }

    /// 트랜잭션 시작
    pub fn begin_transaction(&mut self) -> &mut Self {
        self.statements.push("BEGIN;".to_string());
        self
    }

    /// 트랜잭션 커밋
    pub fn commit(&mut self) -> &mut Self {
        self.statements.push("COMMIT;".to_string());
        self
    }

    /// 테이블 존재 시 컬럼 추가 (안전)
    pub fn add_column_if_not_exists(
        &mut self,
        table: &str,
        column: &str,
        data_type: &str,
        default: Option<&str>,
    ) -> &mut Self {
        let default_clause = default.map(|d| format!(" DEFAULT {}", d)).unwrap_or_default();

        self.statements.push(format!(
            r#"DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = '{}' AND column_name = '{}'
    ) THEN
        ALTER TABLE {} ADD COLUMN {}{}{};
    END IF;
END $$;"#,
            table, column, table, column, data_type, default_clause
        ));

        self
    }

    /// 테이블 리네임 (데이터 보존)
    pub fn rename_table(&mut self, old_name: &str, new_name: &str) -> &mut Self {
        self.statements.push(format!(
            r#"DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = '{}')
       AND NOT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = '{}') THEN
        ALTER TABLE {} RENAME TO {};
    END IF;
END $$;"#,
            old_name, new_name, old_name, new_name
        ));

        self
    }

    /// 데이터 마이그레이션 (old_table → new_table)
    pub fn migrate_data(
        &mut self,
        source_table: &str,
        target_table: &str,
        column_mapping: &HashMap<String, String>,
    ) -> &mut Self {
        let source_cols: Vec<_> = column_mapping.keys().collect();
        let target_cols: Vec<_> = column_mapping.values().collect();

        self.statements.push(format!(
            r#"INSERT INTO {} ({})
SELECT {}
FROM {}
ON CONFLICT DO NOTHING;"#,
            target_table,
            target_cols.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
            source_cols.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
            source_table
        ));

        self
    }

    /// 뷰 재생성 (OR REPLACE)
    pub fn recreate_view(&mut self, view_name: &str, view_sql: &str) -> &mut Self {
        self.statements.push(format!(
            "CREATE OR REPLACE VIEW {} AS\n{};",
            view_name, view_sql
        ));
        self
    }

    /// 최종 SQL 생성
    pub fn build(&self) -> String {
        self.statements.join("\n\n")
    }

    /// 롤백 가능한 마이그레이션 생성 (up/down)
    pub fn with_rollback(
        &mut self,
        up_sql: &str,
        down_sql: &str,
    ) -> &mut Self {
        self.statements.push(format!(
            r#"-- UP (적용)
{}

-- DOWN (롤백) - 주석 해제하여 사용
-- {}"#,
            up_sql,
            down_sql.replace('\n', "\n-- ")
        ));

        self
    }
}

/// 마이그레이션 적용 결과
#[derive(Debug, Clone)]
pub struct ApplyResult {
    /// 성공 여부
    pub success: bool,
    /// 적용된 파일 수
    pub files_applied: usize,
    /// 적용된 문장 수
    pub statements_executed: usize,
    /// 오류 목록
    pub errors: Vec<String>,
    /// 경고 목록
    pub warnings: Vec<String>,
}

impl ApplyResult {
    /// 새 결과 생성
    pub fn new() -> Self {
        Self {
            success: true,
            files_applied: 0,
            statements_executed: 0,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// 오류 추가
    pub fn add_error(&mut self, error: &str) {
        self.success = false;
        self.errors.push(error.to_string());
    }

    /// 경고 추가
    pub fn add_warning(&mut self, warning: &str) {
        self.warnings.push(warning.to_string());
    }
}

impl Default for ApplyResult {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_migration_builder() {
        let mut builder = SafeMigrationBuilder::new();
        builder
            .begin_transaction()
            .add_column_if_not_exists("users", "email", " TEXT", None)
            .add_column_if_not_exists("users", "created_at", " TIMESTAMPTZ", Some("NOW()"))
            .commit();

        let sql = builder.build();
        assert!(sql.contains("BEGIN;"));
        assert!(sql.contains("IF NOT EXISTS"));
        assert!(sql.contains("COMMIT;"));
    }

    #[test]
    fn test_consolidation_plan() {
        let consolidator = MigrationConsolidator::new();

        // 파일명이 그룹 패턴과 일치해야 함 (01_, 02_)
        let mut files = vec![
            MigrationFile::new("01_core_foundation.sql".into(), 1, "CREATE TABLE test;".to_string()),
            MigrationFile::new("02_data_management.sql".into(), 2, "CREATE TABLE data;".to_string()),
        ];

        // 파일에 statements 추가
        files[0].statements.push(SqlStatement::new(
            StatementType::CreateTable,
            "test".to_string(),
            "CREATE TABLE test;".to_string(),
            1,
        ));
        files[1].statements.push(SqlStatement::new(
            StatementType::CreateTable,
            "data".to_string(),
            "CREATE TABLE data;".to_string(),
            1,
        ));

        let plan = consolidator.plan(&files);

        assert!(!plan.files.is_empty(), "통합 파일이 생성되어야 함");
        // original_lines > 0 일 때만 reduction_percentage 검사
        if plan.original_lines > 0 {
            let pct = plan.reduction_percentage();
            assert!(!pct.is_nan(), "reduction_percentage가 NaN이 아니어야 함");
        }
    }

    #[test]
    fn test_ensure_idempotency() {
        let consolidator = MigrationConsolidator::new();

        // CREATE TABLE → IF NOT EXISTS
        let stmt = SqlStatement::new(
            StatementType::CreateTable,
            "users".to_string(),
            "CREATE TABLE users (id INT);".to_string(),
            1,
        );
        let result = consolidator.ensure_idempotency(&stmt);
        assert!(result.contains("IF NOT EXISTS"));

        // CREATE TYPE (ENUM) → pg_type check + ALTER TYPE ADD VALUE IF NOT EXISTS
        let stmt = SqlStatement::new(
            StatementType::CreateType,
            "market_type".to_string(),
            "CREATE TYPE market_type AS ENUM ('KOSPI', 'KOSDAQ', 'ETF')".to_string(),
            1,
        );
        let result = consolidator.ensure_idempotency(&stmt);
        assert!(result.contains("pg_type"), "should use pg_type check");
        assert!(result.contains("IF NOT EXISTS (SELECT 1 FROM pg_type"), "should check pg_type");
        assert!(result.contains("ALTER TYPE market_type ADD VALUE IF NOT EXISTS 'KOSPI'"));
        assert!(result.contains("ALTER TYPE market_type ADD VALUE IF NOT EXISTS 'KOSDAQ'"));
        assert!(result.contains("ALTER TYPE market_type ADD VALUE IF NOT EXISTS 'ETF'"));
        assert!(!result.contains("EXCEPTION"), "should not use DO/EXCEPTION pattern");

        // CREATE VIEW → OR REPLACE
        let stmt = SqlStatement::new(
            StatementType::CreateView,
            "v_test".to_string(),
            "CREATE VIEW v_test AS SELECT 1".to_string(),
            1,
        );
        let result = consolidator.ensure_idempotency(&stmt);
        assert!(result.contains("OR REPLACE"), "should use OR REPLACE for views");

        // CREATE FUNCTION → OR REPLACE
        let stmt = SqlStatement::new(
            StatementType::CreateFunction,
            "my_func".to_string(),
            "CREATE FUNCTION my_func() RETURNS void AS $$ BEGIN END; $$ LANGUAGE plpgsql".to_string(),
            1,
        );
        let result = consolidator.ensure_idempotency(&stmt);
        assert!(result.contains("OR REPLACE"), "should use OR REPLACE for functions");
    }

    #[test]
    fn test_default_groups() {
        let groups = default_consolidation_groups();
        assert_eq!(groups.len(), 10);
        assert_eq!(groups[0].name, "01_core_foundation");
        assert_eq!(groups[8].name, "09_strategy_watched_tickers");
        assert!(groups[8].static_content.is_some(), "group 09 should have static content");
        assert_eq!(groups[9].name, "10_symbol_cascade");
        assert_eq!(groups[9].source_patterns, vec!["23_"]);
    }

    #[test]
    fn test_static_content_group() {
        let consolidator = MigrationConsolidator::new();

        // Plan with no matching source files — static content should still generate
        let files: Vec<MigrationFile> = vec![];
        let plan = consolidator.plan(&files);

        // Group 09 has static content, should still appear even with no source files
        let group09 = plan.files.iter().find(|f| f.name.contains("09_"));
        assert!(group09.is_some(), "group 09 should be generated from static content");
        let g09 = group09.unwrap();
        assert!(g09.content.contains("strategy_watched_tickers"));
    }

    #[test]
    fn test_extract_enum_values() {
        let consolidator = MigrationConsolidator::new();

        let values = consolidator.extract_enum_values(
            "CREATE TYPE market_type AS ENUM ('KOSPI', 'KOSDAQ', 'ETF', 'GLOBAL', 'CRYPTO')",
        );
        assert_eq!(values, vec!["KOSPI", "KOSDAQ", "ETF", "GLOBAL", "CRYPTO"]);

        // Empty case
        let values = consolidator.extract_enum_values("CREATE TYPE custom_type AS (x INT, y INT)");
        assert!(values.is_empty());
    }
}
