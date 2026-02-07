//! SQL 마이그레이션 파일 분석기.
//!
//! 정규식 기반으로 SQL 문장을 파싱하고 의존성 그래프를 생성합니다.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use super::models::*;

/// 마이그레이션 파일 분석기
#[derive(Debug, Default)]
pub struct MigrationAnalyzer {
    /// 시스템 테이블/함수 (의존성에서 제외)
    system_objects: HashSet<String>,
}

impl MigrationAnalyzer {
    /// 새 분석기 생성
    pub fn new() -> Self {
        let mut system_objects = HashSet::new();

        // PostgreSQL 시스템 객체
        for obj in [
            // 시스템 함수
            "now",
            "current_timestamp",
            "gen_random_uuid",
            "coalesce",
            "nullif",
            "greatest",
            "least",
            "count",
            "sum",
            "avg",
            "min",
            "max",
            "array_agg",
            "string_agg",
            "jsonb_agg",
            "row_number",
            "rank",
            "dense_rank",
            "lag",
            "lead",
            "first_value",
            "last_value",
            // TimescaleDB 함수
            "time_bucket",
            "create_hypertable",
            "add_retention_policy",
            "add_continuous_aggregate_policy",
            // 시스템 타입
            "uuid",
            "text",
            "varchar",
            "integer",
            "bigint",
            "smallint",
            "decimal",
            "numeric",
            "real",
            "double",
            "boolean",
            "timestamp",
            "timestamptz",
            "date",
            "time",
            "interval",
            "jsonb",
            "json",
            "bytea",
            "serial",
            "bigserial",
        ] {
            system_objects.insert(obj.to_lowercase());
        }

        Self { system_objects }
    }

    /// 디렉토리에서 마이그레이션 파일 스캔
    pub fn scan_directory(&self, dir: &Path) -> Result<Vec<MigrationFile>, String> {
        if !dir.exists() {
            return Err(format!("디렉토리가 존재하지 않습니다: {:?}", dir));
        }

        let mut files: Vec<MigrationFile> = Vec::new();

        let entries = fs::read_dir(dir)
            .map_err(|e| format!("디렉토리 읽기 실패: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "sql") {
                if let Some(migration) = self.parse_file(&path)? {
                    files.push(migration);
                }
            }
        }

        // 순서 번호로 정렬
        files.sort_by_key(|f| f.order);

        Ok(files)
    }

    /// 단일 마이그레이션 파일 파싱
    pub fn parse_file(&self, path: &Path) -> Result<Option<MigrationFile>, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("파일 읽기 실패 {:?}: {}", path, e))?;

        // 파일명에서 순서 번호 추출 (예: 01_core_foundation.sql → 1)
        let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let order = self.extract_order(filename);

        let mut file = MigrationFile::new(path.to_path_buf(), order, content.clone());

        // SQL 문장 파싱
        file.statements = self.parse_statements(&content);

        // 정의 및 의존성 추출
        for stmt in &file.statements {
            if stmt.statement_type.is_create() {
                file.defines.insert(stmt.object_name.to_lowercase());
            }
            for ref_obj in &stmt.references {
                let ref_lower = ref_obj.to_lowercase();
                if !self.system_objects.contains(&ref_lower) {
                    file.depends_on.insert(ref_lower);
                }
            }
        }

        // 자기 자신에 대한 의존성 제거
        for def in &file.defines {
            file.depends_on.remove(def);
        }

        Ok(Some(file))
    }

    /// 파일명에서 순서 번호 추출
    fn extract_order(&self, filename: &str) -> u32 {
        // 패턴: 01_name.sql, 1_name.sql 등
        let parts: Vec<&str> = filename.split('_').collect();
        if let Some(first) = parts.first() {
            first.parse().unwrap_or(0)
        } else {
            0
        }
    }

    /// SQL 내용에서 문장 파싱
    pub fn parse_statements(&self, content: &str) -> Vec<SqlStatement> {
        let mut statements = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let mut current_stmt = String::new();
        let mut stmt_start_line = 1;
        let mut in_block = false;
        let mut block_depth = 0;

        for (line_idx, line) in lines.iter().enumerate() {
            let line_num = line_idx + 1;
            let trimmed = line.trim();

            // 주석 제외
            if trimmed.starts_with("--") {
                continue;
            }

            // 빈 줄이고 현재 문장이 비어있으면 건너뜀
            if trimmed.is_empty() && current_stmt.is_empty() {
                continue;
            }

            // 새 문장 시작
            if current_stmt.is_empty() {
                stmt_start_line = line_num;
            }

            current_stmt.push_str(line);
            current_stmt.push('\n');

            // $$ 블록 처리 (함수, 트리거 등)
            let dollar_count = trimmed.matches("$$").count();
            if dollar_count > 0 {
                if !in_block {
                    in_block = true;
                    block_depth = 1;
                } else if dollar_count >= 2 {
                    // $$ ... $$ 같은 라인에 있으면 블록 종료
                    in_block = false;
                    block_depth = 0;
                } else {
                    block_depth += dollar_count;
                    if block_depth >= 2 {
                        in_block = false;
                        block_depth = 0;
                    }
                }
            }

            // BEGIN/END 블록 처리
            if !in_block {
                if trimmed.to_uppercase().starts_with("BEGIN") {
                    block_depth += 1;
                }
                if trimmed.to_uppercase().contains("END;") || trimmed.to_uppercase() == "END" {
                    block_depth = block_depth.saturating_sub(1);
                }
            }

            // 문장 종료 확인 (블록 내부가 아닐 때만)
            if !in_block && block_depth == 0 && trimmed.ends_with(';') {
                if let Some(mut stmt) = self.parse_single_statement(&current_stmt, stmt_start_line) {
                    stmt.end_line_number = line_num;
                    statements.push(stmt);
                }
                current_stmt.clear();
            }
        }

        // 마지막 문장 처리
        if !current_stmt.trim().is_empty() {
            if let Some(mut stmt) = self.parse_single_statement(&current_stmt, stmt_start_line) {
                stmt.end_line_number = lines.len();
                statements.push(stmt);
            }
        }

        statements
    }

    /// 단일 SQL 문장 파싱
    fn parse_single_statement(&self, sql: &str, line_number: usize) -> Option<SqlStatement> {
        let sql_upper = sql.to_uppercase();
        let sql_trimmed = sql.trim();

        // 빈 문장이거나 주석만 있는 경우 무시
        if sql_trimmed.is_empty() || sql_trimmed.starts_with("--") {
            return None;
        }

        let (stmt_type, object_name) = self.detect_statement_type(&sql_upper, sql)?;

        let mut stmt = SqlStatement::new(
            stmt_type,
            object_name,
            sql_trimmed.to_string(),
            line_number,
        );

        // 옵션 플래그 검출
        stmt.if_not_exists = sql_upper.contains("IF NOT EXISTS");
        stmt.if_exists = sql_upper.contains("IF EXISTS");
        stmt.cascade = sql_upper.contains("CASCADE");

        // 참조 객체 추출
        stmt.references = self.extract_references(sql);

        Some(stmt)
    }

    /// 문장 유형 및 대상 객체 검출
    fn detect_statement_type(&self, sql_upper: &str, sql_original: &str) -> Option<(StatementType, String)> {
        // CREATE TABLE
        if sql_upper.contains("CREATE TABLE") {
            let name = self.extract_object_name(sql_original, "TABLE")?;
            return Some((StatementType::CreateTable, name));
        }

        // CREATE OR REPLACE VIEW / CREATE VIEW
        if sql_upper.contains("CREATE") && sql_upper.contains("VIEW") && !sql_upper.contains("MATERIALIZED") {
            let name = self.extract_object_name(sql_original, "VIEW")?;
            return Some((StatementType::CreateView, name));
        }

        // CREATE MATERIALIZED VIEW
        if sql_upper.contains("CREATE MATERIALIZED VIEW") {
            let name = self.extract_object_name(sql_original, "MATERIALIZED VIEW")?;
            return Some((StatementType::CreateMaterializedView, name));
        }

        // CREATE INDEX
        if sql_upper.contains("CREATE INDEX") || sql_upper.contains("CREATE UNIQUE INDEX") {
            let name = self.extract_index_name(sql_original)?;
            return Some((StatementType::CreateIndex, name));
        }

        // CREATE FUNCTION
        if sql_upper.contains("CREATE FUNCTION") || sql_upper.contains("CREATE OR REPLACE FUNCTION") {
            let name = self.extract_function_name(sql_original)?;
            return Some((StatementType::CreateFunction, name));
        }

        // CREATE TRIGGER
        if sql_upper.contains("CREATE TRIGGER") || sql_upper.contains("CREATE OR REPLACE TRIGGER") {
            let name = self.extract_trigger_name(sql_original)?;
            return Some((StatementType::CreateTrigger, name));
        }

        // CREATE TYPE
        if sql_upper.contains("CREATE TYPE") {
            let name = self.extract_object_name(sql_original, "TYPE")?;
            return Some((StatementType::CreateType, name));
        }

        // CREATE EXTENSION
        if sql_upper.contains("CREATE EXTENSION") {
            let name = self.extract_extension_name(sql_original)?;
            return Some((StatementType::CreateExtension, name));
        }

        // DROP TABLE
        if sql_upper.contains("DROP TABLE") {
            let name = self.extract_drop_object_name(sql_original, "TABLE")?;
            return Some((StatementType::DropTable, name));
        }

        // DROP VIEW
        if sql_upper.contains("DROP VIEW") && !sql_upper.contains("MATERIALIZED") {
            let name = self.extract_drop_object_name(sql_original, "VIEW")?;
            return Some((StatementType::DropView, name));
        }

        // DROP MATERIALIZED VIEW
        if sql_upper.contains("DROP MATERIALIZED VIEW") {
            let name = self.extract_drop_object_name(sql_original, "MATERIALIZED VIEW")?;
            return Some((StatementType::DropMaterializedView, name));
        }

        // DROP INDEX
        if sql_upper.contains("DROP INDEX") {
            let name = self.extract_drop_object_name(sql_original, "INDEX")?;
            return Some((StatementType::DropIndex, name));
        }

        // DROP FUNCTION
        if sql_upper.contains("DROP FUNCTION") {
            let name = self.extract_drop_function_name(sql_original)?;
            return Some((StatementType::DropFunction, name));
        }

        // DROP TRIGGER
        if sql_upper.contains("DROP TRIGGER") {
            let name = self.extract_drop_object_name(sql_original, "TRIGGER")?;
            return Some((StatementType::DropTrigger, name));
        }

        // DROP TYPE
        if sql_upper.contains("DROP TYPE") {
            let name = self.extract_drop_object_name(sql_original, "TYPE")?;
            return Some((StatementType::DropType, name));
        }

        // ALTER TABLE
        if sql_upper.contains("ALTER TABLE") {
            let name = self.extract_alter_table_name(sql_original)?;
            return Some((StatementType::AlterTable, name));
        }

        // INSERT INTO
        if sql_upper.starts_with("INSERT") {
            let name = self.extract_insert_table_name(sql_original)?;
            return Some((StatementType::Insert, name));
        }

        // SELECT INTO / create_hypertable 등
        if sql_upper.contains("SELECT") && sql_upper.contains("CREATE_HYPERTABLE") {
            let name = self.extract_hypertable_name(sql_original)?;
            return Some((StatementType::SelectInto, name));
        }

        // 기타 문장
        let first_word = sql_upper.split_whitespace().next().unwrap_or("UNKNOWN");
        Some((StatementType::Other(first_word.to_string()), String::new()))
    }

    /// 객체명 추출 (CREATE TABLE/VIEW/TYPE 등)
    fn extract_object_name(&self, sql: &str, keyword: &str) -> Option<String> {
        let sql_upper = sql.to_uppercase();
        let keyword_upper = keyword.to_uppercase();

        // IF NOT EXISTS 처리
        let search_pattern = if sql_upper.contains("IF NOT EXISTS") {
            format!("{} IF NOT EXISTS", keyword_upper)
        } else if sql_upper.contains("OR REPLACE") {
            format!("OR REPLACE {}", keyword_upper)
        } else {
            keyword_upper.clone()
        };

        let pos = sql_upper.find(&search_pattern)?;
        let after = &sql[pos + search_pattern.len()..];
        let name = after
            .split_whitespace()
            .next()?
            .trim_matches(|c: char| c == '(' || c == '"' || c == ';');

        Some(self.clean_object_name(name))
    }

    /// DROP 문에서 객체명 추출
    fn extract_drop_object_name(&self, sql: &str, keyword: &str) -> Option<String> {
        let sql_upper = sql.to_uppercase();
        let pattern = format!("DROP {} IF EXISTS", keyword.to_uppercase());
        let pattern_no_if = format!("DROP {}", keyword.to_uppercase());

        let after = if sql_upper.contains(&pattern) {
            let pos = sql_upper.find(&pattern)?;
            &sql[pos + pattern.len()..]
        } else {
            let pos = sql_upper.find(&pattern_no_if)?;
            &sql[pos + pattern_no_if.len()..]
        };

        let name = after
            .split_whitespace()
            .next()?
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '_');

        Some(self.clean_object_name(name))
    }

    /// 인덱스명 추출
    fn extract_index_name(&self, sql: &str) -> Option<String> {
        let sql_upper = sql.to_uppercase();

        // CREATE UNIQUE INDEX IF NOT EXISTS idx_name ON ...
        // CREATE INDEX IF NOT EXISTS idx_name ON ...
        // CREATE INDEX idx_name ON ...
        let patterns = [
            "CREATE UNIQUE INDEX IF NOT EXISTS",
            "CREATE INDEX IF NOT EXISTS",
            "CREATE UNIQUE INDEX",
            "CREATE INDEX",
        ];

        for pattern in patterns {
            if sql_upper.contains(pattern) {
                let pos = sql_upper.find(pattern)?;
                let after = &sql[pos + pattern.len()..];
                let name = after
                    .split_whitespace()
                    .next()?
                    .trim_matches(|c: char| !c.is_alphanumeric() && c != '_');

                if !name.to_uppercase().starts_with("ON") {
                    return Some(self.clean_object_name(name));
                }
            }
        }

        None
    }

    /// 함수명 추출
    fn extract_function_name(&self, sql: &str) -> Option<String> {
        let sql_upper = sql.to_uppercase();

        let patterns = [
            "CREATE OR REPLACE FUNCTION",
            "CREATE FUNCTION",
        ];

        for pattern in patterns {
            if sql_upper.contains(pattern) {
                let pos = sql_upper.find(pattern)?;
                let after = &sql[pos + pattern.len()..];

                // 함수명은 ( 전까지
                let name = after
                    .split('(')
                    .next()?
                    .trim()
                    .trim_matches(|c: char| !c.is_alphanumeric() && c != '_');

                return Some(self.clean_object_name(name));
            }
        }

        None
    }

    /// DROP FUNCTION에서 함수명 추출
    fn extract_drop_function_name(&self, sql: &str) -> Option<String> {
        let sql_upper = sql.to_uppercase();

        let patterns = [
            "DROP FUNCTION IF EXISTS",
            "DROP FUNCTION",
        ];

        for pattern in patterns {
            if sql_upper.contains(pattern) {
                let pos = sql_upper.find(pattern)?;
                let after = &sql[pos + pattern.len()..];

                // 함수명은 ( 또는 ; 전까지
                let name = after
                    .split(|c: char| c == '(' || c == ';')
                    .next()?
                    .trim()
                    .trim_matches(|c: char| !c.is_alphanumeric() && c != '_');

                return Some(self.clean_object_name(name));
            }
        }

        None
    }

    /// 트리거명 추출
    fn extract_trigger_name(&self, sql: &str) -> Option<String> {
        let sql_upper = sql.to_uppercase();

        let patterns = [
            "CREATE OR REPLACE TRIGGER",
            "CREATE TRIGGER",
        ];

        for pattern in patterns {
            if sql_upper.contains(pattern) {
                let pos = sql_upper.find(pattern)?;
                let after = &sql[pos + pattern.len()..];
                let name = after
                    .split_whitespace()
                    .next()?
                    .trim_matches(|c: char| !c.is_alphanumeric() && c != '_');

                return Some(self.clean_object_name(name));
            }
        }

        None
    }

    /// 확장 이름 추출
    fn extract_extension_name(&self, sql: &str) -> Option<String> {
        let sql_upper = sql.to_uppercase();

        let patterns = [
            "CREATE EXTENSION IF NOT EXISTS",
            "CREATE EXTENSION",
        ];

        for pattern in patterns {
            if sql_upper.contains(pattern) {
                let pos = sql_upper.find(pattern)?;
                let after = &sql[pos + pattern.len()..];
                let name = after
                    .split_whitespace()
                    .next()?
                    .trim_matches(|c: char| c == '"' || c == '\'' || c == ';');

                return Some(self.clean_object_name(name));
            }
        }

        None
    }

    /// ALTER TABLE에서 테이블명 추출
    fn extract_alter_table_name(&self, sql: &str) -> Option<String> {
        let sql_upper = sql.to_uppercase();
        let pos = sql_upper.find("ALTER TABLE")?;
        let after = &sql[pos + "ALTER TABLE".len()..];

        // IF EXISTS 처리
        let name_part = if after.trim().to_uppercase().starts_with("IF EXISTS") {
            after.trim()["IF EXISTS".len()..].trim()
        } else {
            after.trim()
        };

        let name = name_part
            .split_whitespace()
            .next()?
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '_');

        Some(self.clean_object_name(name))
    }

    /// INSERT INTO에서 테이블명 추출
    fn extract_insert_table_name(&self, sql: &str) -> Option<String> {
        let sql_upper = sql.to_uppercase();
        let pos = sql_upper.find("INSERT INTO")?;
        let after = &sql[pos + "INSERT INTO".len()..];
        let name = after
            .split(|c: char| c == '(' || c.is_whitespace())
            .next()?
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '_');

        Some(self.clean_object_name(name))
    }

    /// create_hypertable에서 테이블명 추출
    fn extract_hypertable_name(&self, sql: &str) -> Option<String> {
        let sql_lower = sql.to_lowercase();
        let pos = sql_lower.find("create_hypertable")?;
        let after = &sql[pos..];

        // create_hypertable('table_name', ...)
        let start = after.find('(')?;
        let content = &after[start + 1..];
        let name = content
            .split(',')
            .next()?
            .trim()
            .trim_matches(|c: char| c == '\'' || c == '"');

        Some(self.clean_object_name(name))
    }

    /// 객체명 정리 (스키마 prefix 제거, 소문자 변환)
    fn clean_object_name(&self, name: &str) -> String {
        // public.table_name → table_name
        let name = if name.contains('.') {
            name.split('.').last().unwrap_or(name)
        } else {
            name
        };

        name.trim_matches(|c: char| c == '"' || c == '\'')
            .to_lowercase()
    }

    /// SQL에서 참조 객체 추출 (FROM, JOIN, REFERENCES 등)
    fn extract_references(&self, sql: &str) -> Vec<String> {
        let mut refs = HashSet::new();
        let sql_upper = sql.to_uppercase();

        // FROM 절
        self.extract_from_references(&sql_upper, sql, &mut refs);

        // JOIN 절
        self.extract_join_references(&sql_upper, sql, &mut refs);

        // REFERENCES 절 (외래키)
        self.extract_fk_references(&sql_upper, sql, &mut refs);

        // ON 절 (인덱스, 트리거 등)
        self.extract_on_references(&sql_upper, sql, &mut refs);

        refs.into_iter().collect()
    }

    fn extract_from_references(&self, sql_upper: &str, sql: &str, refs: &mut HashSet<String>) {
        for pos in sql_upper.match_indices("FROM ") {
            let after = &sql[pos.0 + 5..];
            if let Some(name) = after.split_whitespace().next() {
                let clean = self.clean_object_name(name);
                if !clean.is_empty() && !self.system_objects.contains(&clean) {
                    refs.insert(clean);
                }
            }
        }
    }

    fn extract_join_references(&self, sql_upper: &str, sql: &str, refs: &mut HashSet<String>) {
        let join_patterns = ["JOIN ", "INNER JOIN ", "LEFT JOIN ", "RIGHT JOIN ", "OUTER JOIN "];

        for pattern in join_patterns {
            for pos in sql_upper.match_indices(pattern) {
                let after = &sql[pos.0 + pattern.len()..];
                if let Some(name) = after.split_whitespace().next() {
                    let clean = self.clean_object_name(name);
                    if !clean.is_empty() && !self.system_objects.contains(&clean) {
                        refs.insert(clean);
                    }
                }
            }
        }
    }

    fn extract_fk_references(&self, sql_upper: &str, sql: &str, refs: &mut HashSet<String>) {
        for pos in sql_upper.match_indices("REFERENCES ") {
            let after = &sql[pos.0 + 11..];
            if let Some(name) = after.split(|c: char| c == '(' || c.is_whitespace()).next() {
                let clean = self.clean_object_name(name);
                if !clean.is_empty() && !self.system_objects.contains(&clean) {
                    refs.insert(clean);
                }
            }
        }
    }

    fn extract_on_references(&self, sql_upper: &str, sql: &str, refs: &mut HashSet<String>) {
        // CREATE INDEX ... ON table_name
        // CREATE TRIGGER ... ON table_name
        if sql_upper.contains("CREATE INDEX") || sql_upper.contains("CREATE TRIGGER") {
            for pos in sql_upper.match_indices(" ON ") {
                let after = &sql[pos.0 + 4..];
                if let Some(name) = after.split(|c: char| c == '(' || c.is_whitespace()).next() {
                    let clean = self.clean_object_name(name);
                    if !clean.is_empty() && !self.system_objects.contains(&clean) {
                        refs.insert(clean);
                    }
                }
            }
        }
    }

    /// 마이그레이션 파일들에서 의존성 그래프 생성
    pub fn build_dependency_graph(&self, files: &[MigrationFile]) -> DependencyGraph {
        let mut graph = DependencyGraph::new();

        // 파일별 정의 수집
        let mut object_to_file: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        for file in files {
            for stmt in &file.statements {
                if stmt.statement_type.is_create() && !stmt.object_name.is_empty() {
                    graph.add_definition(&stmt.object_name, &file.name, stmt.line_number);
                    object_to_file.insert(stmt.object_name.to_lowercase(), file.name.clone());
                }
            }
        }

        // 의존성 연결
        for file in files {
            for stmt in &file.statements {
                for ref_obj in &stmt.references {
                    let ref_lower = ref_obj.to_lowercase();
                    if !self.system_objects.contains(&ref_lower) {
                        // 객체 의존성
                        if !stmt.object_name.is_empty() {
                            graph.add_dependency(&stmt.object_name, &ref_lower);
                        }

                        // 파일 의존성
                        if let Some(def_file) = object_to_file.get(&ref_lower) {
                            graph.add_file_dependency(&file.name, def_file);
                        }
                    }
                }
            }
        }

        graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_create_table() {
        let analyzer = MigrationAnalyzer::new();
        let sql = "CREATE TABLE IF NOT EXISTS users (id SERIAL PRIMARY KEY, name TEXT);";
        let stmts = analyzer.parse_statements(sql);

        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0].statement_type, StatementType::CreateTable);
        assert_eq!(stmts[0].object_name, "users");
        assert!(stmts[0].if_not_exists);
    }

    #[test]
    fn test_parse_create_view() {
        let analyzer = MigrationAnalyzer::new();
        let sql = "CREATE OR REPLACE VIEW v_active_users AS SELECT * FROM users WHERE active = true;";
        let stmts = analyzer.parse_statements(sql);

        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0].statement_type, StatementType::CreateView);
        assert_eq!(stmts[0].object_name, "v_active_users");
        assert!(stmts[0].references.contains(&"users".to_string()));
    }

    #[test]
    fn test_parse_drop_cascade() {
        let analyzer = MigrationAnalyzer::new();
        let sql = "DROP TABLE IF EXISTS old_table CASCADE;";
        let stmts = analyzer.parse_statements(sql);

        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0].statement_type, StatementType::DropTable);
        assert_eq!(stmts[0].object_name, "old_table");
        assert!(stmts[0].if_exists);
        assert!(stmts[0].cascade);
    }

    #[test]
    fn test_parse_create_index() {
        let analyzer = MigrationAnalyzer::new();
        let sql = "CREATE INDEX IF NOT EXISTS idx_users_email ON users (email);";
        let stmts = analyzer.parse_statements(sql);

        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0].statement_type, StatementType::CreateIndex);
        assert_eq!(stmts[0].object_name, "idx_users_email");
        assert!(stmts[0].references.contains(&"users".to_string()));
    }

    #[test]
    fn test_extract_foreign_key_reference() {
        let analyzer = MigrationAnalyzer::new();
        let sql = "CREATE TABLE orders (
            id SERIAL PRIMARY KEY,
            user_id INTEGER REFERENCES users(id)
        );";
        let stmts = analyzer.parse_statements(sql);

        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].references.contains(&"users".to_string()));
    }

    #[test]
    fn test_extract_order_from_filename() {
        let analyzer = MigrationAnalyzer::new();

        assert_eq!(analyzer.extract_order("01_core_foundation.sql"), 1);
        assert_eq!(analyzer.extract_order("10_restore_tables.sql"), 10);
        assert_eq!(analyzer.extract_order("1_simple.sql"), 1);
    }

    #[test]
    fn test_build_dependency_graph() {
        let analyzer = MigrationAnalyzer::new();

        let mut file1 = MigrationFile::new("01.sql".into(), 1, String::new());
        file1.statements.push(SqlStatement {
            statement_type: StatementType::CreateTable,
            object_name: "users".to_string(),
            raw_sql: String::new(),
            line_number: 1,
            end_line_number: 1,
            if_not_exists: true,
            if_exists: false,
            cascade: false,
            references: Vec::new(),
        });

        let mut file2 = MigrationFile::new("02.sql".into(), 2, String::new());
        file2.statements.push(SqlStatement {
            statement_type: StatementType::CreateTable,
            object_name: "orders".to_string(),
            raw_sql: String::new(),
            line_number: 1,
            end_line_number: 1,
            if_not_exists: true,
            if_exists: false,
            cascade: false,
            references: vec!["users".to_string()],
        });

        let graph = analyzer.build_dependency_graph(&[file1, file2]);

        assert!(graph.definitions.contains_key("users"));
        assert!(graph.definitions.contains_key("orders"));
        assert!(graph.dependencies.get("orders").unwrap().contains("users"));
    }
}
