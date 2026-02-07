//! 마이그레이션 분석을 위한 데이터 모델.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// SQL 문장 유형
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StatementType {
    /// CREATE TABLE
    CreateTable,
    /// CREATE VIEW
    CreateView,
    /// CREATE MATERIALIZED VIEW
    CreateMaterializedView,
    /// CREATE INDEX
    CreateIndex,
    /// CREATE FUNCTION
    CreateFunction,
    /// CREATE TRIGGER
    CreateTrigger,
    /// CREATE TYPE (ENUM 등)
    CreateType,
    /// CREATE EXTENSION
    CreateExtension,
    /// DROP TABLE
    DropTable,
    /// DROP VIEW
    DropView,
    /// DROP MATERIALIZED VIEW
    DropMaterializedView,
    /// DROP INDEX
    DropIndex,
    /// DROP FUNCTION
    DropFunction,
    /// DROP TRIGGER
    DropTrigger,
    /// DROP TYPE
    DropType,
    /// ALTER TABLE
    AlterTable,
    /// INSERT INTO
    Insert,
    /// SELECT INTO (TimescaleDB hypertable 등)
    SelectInto,
    /// 기타 문장
    Other(String),
}

impl StatementType {
    /// DROP 문장인지 확인
    pub fn is_drop(&self) -> bool {
        matches!(
            self,
            StatementType::DropTable
                | StatementType::DropView
                | StatementType::DropMaterializedView
                | StatementType::DropIndex
                | StatementType::DropFunction
                | StatementType::DropTrigger
                | StatementType::DropType
        )
    }

    /// CREATE 문장인지 확인
    pub fn is_create(&self) -> bool {
        matches!(
            self,
            StatementType::CreateTable
                | StatementType::CreateView
                | StatementType::CreateMaterializedView
                | StatementType::CreateIndex
                | StatementType::CreateFunction
                | StatementType::CreateTrigger
                | StatementType::CreateType
                | StatementType::CreateExtension
        )
    }
}

/// 파싱된 SQL 문장
#[derive(Debug, Clone)]
pub struct SqlStatement {
    /// 문장 유형
    pub statement_type: StatementType,
    /// 대상 객체 이름 (테이블명, 뷰명 등)
    pub object_name: String,
    /// 원본 SQL
    pub raw_sql: String,
    /// 파일 내 시작 라인 번호 (1-based)
    pub line_number: usize,
    /// 파일 내 종료 라인 번호 (1-based)
    pub end_line_number: usize,
    /// IF NOT EXISTS 사용 여부
    pub if_not_exists: bool,
    /// IF EXISTS 사용 여부
    pub if_exists: bool,
    /// CASCADE 사용 여부
    pub cascade: bool,
    /// 참조하는 다른 객체들 (FROM, JOIN, REFERENCES 등에서 추출)
    pub references: Vec<String>,
}

impl SqlStatement {
    /// 새 SQL 문장 생성
    pub fn new(
        statement_type: StatementType,
        object_name: String,
        raw_sql: String,
        line_number: usize,
    ) -> Self {
        Self {
            statement_type,
            object_name,
            raw_sql,
            line_number,
            end_line_number: line_number,
            if_not_exists: false,
            if_exists: false,
            cascade: false,
            references: Vec::new(),
        }
    }
}

/// 마이그레이션 파일 정보
#[derive(Debug, Clone)]
pub struct MigrationFile {
    /// 파일 경로
    pub path: PathBuf,
    /// 파일명 (확장자 제외)
    pub name: String,
    /// 마이그레이션 순서 번호 (파일명에서 추출)
    pub order: u32,
    /// 파일 내용
    pub content: String,
    /// 파싱된 SQL 문장들
    pub statements: Vec<SqlStatement>,
    /// 정의하는 객체들
    pub defines: HashSet<String>,
    /// 참조하는 객체들
    pub depends_on: HashSet<String>,
}

impl MigrationFile {
    /// 새 마이그레이션 파일 생성
    pub fn new(path: PathBuf, order: u32, content: String) -> Self {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Self {
            path,
            name,
            order,
            content,
            statements: Vec::new(),
            defines: HashSet::new(),
            depends_on: HashSet::new(),
        }
    }
}

/// 의존성 그래프
#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    /// 객체별 정의 위치 (객체명 → (파일명, 라인번호) 목록)
    pub definitions: HashMap<String, Vec<(String, usize)>>,
    /// 객체별 의존 대상 (객체명 → 의존 객체명 목록)
    pub dependencies: HashMap<String, HashSet<String>>,
    /// 파일별 의존 관계 (파일명 → 의존 파일명 목록)
    pub file_dependencies: HashMap<String, HashSet<String>>,
}

impl DependencyGraph {
    /// 새 그래프 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// 정의 추가
    pub fn add_definition(&mut self, object: &str, file: &str, line: usize) {
        self.definitions
            .entry(object.to_lowercase())
            .or_default()
            .push((file.to_string(), line));
    }

    /// 의존성 추가
    pub fn add_dependency(&mut self, object: &str, depends_on: &str) {
        self.dependencies
            .entry(object.to_lowercase())
            .or_default()
            .insert(depends_on.to_lowercase());
    }

    /// 파일 의존성 추가
    pub fn add_file_dependency(&mut self, file: &str, depends_on_file: &str) {
        if file != depends_on_file {
            self.file_dependencies
                .entry(file.to_string())
                .or_default()
                .insert(depends_on_file.to_string());
        }
    }

    /// 중복 정의된 객체 찾기
    pub fn find_duplicates(&self) -> Vec<(String, Vec<(String, usize)>)> {
        self.definitions
            .iter()
            .filter(|(_, locations)| locations.len() > 1)
            .map(|(name, locs)| (name.clone(), locs.clone()))
            .collect()
    }

    /// 순환 의존성 검출 (DFS 기반)
    pub fn find_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = Vec::new();
        let mut rec_set = HashSet::new();

        for node in self.dependencies.keys() {
            if !visited.contains(node) {
                self.dfs_cycle(node, &mut visited, &mut rec_stack, &mut rec_set, &mut cycles);
            }
        }

        cycles
    }

    fn dfs_cycle(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut Vec<String>,
        rec_set: &mut HashSet<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node.to_string());
        rec_stack.push(node.to_string());
        rec_set.insert(node.to_string());

        if let Some(deps) = self.dependencies.get(node) {
            for dep in deps {
                if !visited.contains(dep) {
                    self.dfs_cycle(dep, visited, rec_stack, rec_set, cycles);
                } else if rec_set.contains(dep) {
                    // 순환 발견
                    let cycle_start = rec_stack.iter().position(|x| x == dep).unwrap();
                    let cycle: Vec<String> = rec_stack[cycle_start..].to_vec();
                    cycles.push(cycle);
                }
            }
        }

        rec_stack.pop();
        rec_set.remove(node);
    }
}

/// 검증 결과 심각도
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// 정보 (권장사항)
    Info,
    /// 경고 (수정 권장)
    Warning,
    /// 에러 (수정 필수)
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "INFO"),
            Severity::Warning => write!(f, "WARNING"),
            Severity::Error => write!(f, "ERROR"),
        }
    }
}

/// 검증 결과 항목
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// 심각도
    pub severity: Severity,
    /// 문제 코드
    pub code: String,
    /// 문제 설명
    pub message: String,
    /// 관련 파일
    pub file: Option<String>,
    /// 관련 라인 번호
    pub line: Option<usize>,
    /// 관련 객체명
    pub object: Option<String>,
    /// 권장 해결 방법
    pub suggestion: Option<String>,
}

impl ValidationIssue {
    /// 새 이슈 생성
    pub fn new(severity: Severity, code: &str, message: &str) -> Self {
        Self {
            severity,
            code: code.to_string(),
            message: message.to_string(),
            file: None,
            line: None,
            object: None,
            suggestion: None,
        }
    }

    /// 파일 정보 추가
    pub fn with_file(mut self, file: &str) -> Self {
        self.file = Some(file.to_string());
        self
    }

    /// 라인 정보 추가
    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    /// 객체 정보 추가
    pub fn with_object(mut self, object: &str) -> Self {
        self.object = Some(object.to_string());
        self
    }

    /// 해결 방법 추가
    pub fn with_suggestion(mut self, suggestion: &str) -> Self {
        self.suggestion = Some(suggestion.to_string());
        self
    }
}

impl std::fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.code, self.message)?;

        if let Some(ref file) = self.file {
            write!(f, "\n  파일: {}", file)?;
            if let Some(line) = self.line {
                write!(f, ":{}", line)?;
            }
        }

        if let Some(ref obj) = self.object {
            write!(f, "\n  객체: {}", obj)?;
        }

        if let Some(ref suggestion) = self.suggestion {
            write!(f, "\n  해결: {}", suggestion)?;
        }

        Ok(())
    }
}

/// 검증 보고서
#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    /// 발견된 이슈들
    pub issues: Vec<ValidationIssue>,
    /// 분석된 파일 수
    pub files_analyzed: usize,
    /// 총 SQL 문장 수
    pub total_statements: usize,
    /// 의존성 그래프
    pub graph: DependencyGraph,
}

impl ValidationReport {
    /// 새 보고서 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// 이슈 추가
    pub fn add_issue(&mut self, issue: ValidationIssue) {
        self.issues.push(issue);
    }

    /// 에러 수
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count()
    }

    /// 경고 수
    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count()
    }

    /// 검증 통과 여부
    pub fn is_valid(&self) -> bool {
        self.error_count() == 0
    }

    /// 심각도별 정렬된 이슈 목록
    pub fn sorted_issues(&self) -> Vec<&ValidationIssue> {
        let mut sorted: Vec<_> = self.issues.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }
}

impl std::fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "═══════════════════════════════════════════════════════════════")?;
        writeln!(f, "                    마이그레이션 검증 보고서")?;
        writeln!(f, "═══════════════════════════════════════════════════════════════")?;
        writeln!(f)?;
        writeln!(f, "📊 요약")?;
        writeln!(f, "  분석 파일: {} 개", self.files_analyzed)?;
        writeln!(f, "  SQL 문장: {} 개", self.total_statements)?;
        writeln!(f)?;
        writeln!(
            f,
            "  🔴 에러: {} 개  🟡 경고: {} 개  🔵 정보: {} 개",
            self.error_count(),
            self.warning_count(),
            self.issues.len() - self.error_count() - self.warning_count()
        )?;
        writeln!(f)?;

        if self.issues.is_empty() {
            writeln!(f, "✅ 문제가 발견되지 않았습니다.")?;
        } else {
            writeln!(f, "───────────────────────────────────────────────────────────────")?;
            writeln!(f, "🔍 발견된 이슈")?;
            writeln!(f, "───────────────────────────────────────────────────────────────")?;
            for (i, issue) in self.sorted_issues().iter().enumerate() {
                writeln!(f)?;
                writeln!(f, "{}. {}", i + 1, issue)?;
            }
        }

        writeln!(f)?;
        writeln!(f, "═══════════════════════════════════════════════════════════════")?;

        Ok(())
    }
}

/// 통합 계획 파일
#[derive(Debug, Clone)]
pub struct ConsolidationFile {
    /// 파일명
    pub name: String,
    /// 설명
    pub description: String,
    /// 포함할 내용 (원본 파일 → SQL 목록)
    pub sources: Vec<(String, Vec<String>)>,
    /// 최종 생성될 SQL
    pub content: String,
}

/// 통합 계획
#[derive(Debug, Clone, Default)]
pub struct ConsolidationPlan {
    /// 생성할 파일들
    pub files: Vec<ConsolidationFile>,
    /// 제거할 원본 파일들
    pub files_to_remove: Vec<String>,
    /// 통합 전 총 라인 수
    pub original_lines: usize,
    /// 통합 후 예상 라인 수
    pub consolidated_lines: usize,
}

impl ConsolidationPlan {
    /// 새 계획 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// 감소율 계산 (%)
    pub fn reduction_percentage(&self) -> f64 {
        if self.original_lines == 0 {
            return 0.0;
        }
        (1.0 - (self.consolidated_lines as f64 / self.original_lines as f64)) * 100.0
    }
}

impl std::fmt::Display for ConsolidationPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "═══════════════════════════════════════════════════════════════")?;
        writeln!(f, "                    마이그레이션 통합 계획")?;
        writeln!(f, "═══════════════════════════════════════════════════════════════")?;
        writeln!(f)?;
        writeln!(f, "📊 통합 효과")?;
        writeln!(f, "  통합 전: {} 파일, {} 줄", self.files_to_remove.len(), self.original_lines)?;
        writeln!(f, "  통합 후: {} 파일, {} 줄", self.files.len(), self.consolidated_lines)?;
        writeln!(f, "  감소율: {:.1}%", self.reduction_percentage())?;
        writeln!(f)?;

        writeln!(f, "───────────────────────────────────────────────────────────────")?;
        writeln!(f, "📁 생성될 파일")?;
        writeln!(f, "───────────────────────────────────────────────────────────────")?;
        for (i, file) in self.files.iter().enumerate() {
            writeln!(f)?;
            writeln!(f, "{}. {} - {}", i + 1, file.name, file.description)?;
            for (source, _) in &file.sources {
                writeln!(f, "   ← {}", source)?;
            }
        }

        writeln!(f)?;
        writeln!(f, "═══════════════════════════════════════════════════════════════")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_graph_duplicates() {
        let mut graph = DependencyGraph::new();
        graph.add_definition("users", "01.sql", 10);
        graph.add_definition("users", "05.sql", 20);

        let dups = graph.find_duplicates();
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].0, "users");
    }

    #[test]
    fn test_dependency_graph_cycles() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("a", "b");
        graph.add_dependency("b", "c");
        graph.add_dependency("c", "a");

        let cycles = graph.find_cycles();
        assert!(!cycles.is_empty());
    }

    #[test]
    fn test_validation_report_display() {
        let mut report = ValidationReport::new();
        report.files_analyzed = 5;
        report.total_statements = 100;
        report.add_issue(
            ValidationIssue::new(Severity::Error, "DUP001", "중복 정의")
                .with_file("01.sql")
                .with_line(10)
                .with_object("users"),
        );

        let output = format!("{}", report);
        assert!(output.contains("에러: 1"));
        assert!(output.contains("DUP001"));
    }
}
