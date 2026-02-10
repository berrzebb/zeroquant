//! 마이그레이션 검증기.
//!
//! 중복 정의, DROP CASCADE, 순환 의존성 등의 문제를 검출합니다.

use std::collections::{HashMap, HashSet};

use super::analyzer::MigrationAnalyzer;
use super::models::*;

/// 마이그레이션 검증기
pub struct MigrationValidator<'a> {
    files: &'a [MigrationFile],
    analyzer: MigrationAnalyzer,
}

impl<'a> MigrationValidator<'a> {
    /// 새 검증기 생성
    pub fn new(files: &'a [MigrationFile]) -> Self {
        Self {
            files,
            analyzer: MigrationAnalyzer::new(),
        }
    }

    /// 전체 검증 수행
    pub fn validate(&self) -> ValidationReport {
        let mut report = ValidationReport::new();
        report.files_analyzed = self.files.len();
        report.total_statements = self.files.iter().map(|f| f.statements.len()).sum();

        // 의존성 그래프 생성
        report.graph = self.analyzer.build_dependency_graph(self.files);

        // 각 검증 수행
        self.check_duplicate_definitions(&mut report);
        self.check_cascade_usage(&mut report);
        self.check_circular_dependencies(&mut report);
        self.check_idempotency(&mut report);
        self.check_drop_create_pattern(&mut report);
        self.check_view_dependencies(&mut report);
        self.check_missing_if_not_exists(&mut report);
        self.check_data_safety(&mut report);

        report
    }

    /// 중복 정의 검사
    fn check_duplicate_definitions(&self, report: &mut ValidationReport) {
        let duplicates = report.graph.find_duplicates();

        for (object, locations) in duplicates {
            // 같은 파일 내 중복은 무시 (OR REPLACE 패턴)
            let unique_files: HashSet<_> = locations.iter().map(|(f, _)| f.as_str()).collect();
            if unique_files.len() < 2 {
                continue;
            }

            let mut issue = ValidationIssue::new(
                Severity::Warning,
                "DUP001",
                &format!(
                    "'{}' 객체가 {} 곳에서 정의됨",
                    object,
                    locations.len()
                ),
            )
            .with_object(&object);

            let locations_str: Vec<String> = locations
                .iter()
                .map(|(f, l)| format!("{}:{}", f, l))
                .collect();
            issue.suggestion = Some(format!(
                "하나의 정의로 통합 권장. 위치: {}",
                locations_str.join(", ")
            ));

            report.add_issue(issue);
        }
    }

    /// DROP CASCADE 사용 검사 (FK ON DELETE CASCADE는 의도된 설계이므로 제외)
    fn check_cascade_usage(&self, report: &mut ValidationReport) {
        for file in self.files {
            for stmt in &file.statements {
                // DDL CASCADE (DROP ... CASCADE) 만 보고
                if stmt.cascade {
                    let severity = if stmt.statement_type.is_drop() {
                        // DROP ... CASCADE는 데이터 손실 위험
                        Severity::Warning
                    } else {
                        // DO $$ 블록 내 DROP ... CASCADE 등
                        Severity::Info
                    };

                    let issue = ValidationIssue::new(
                        severity,
                        "CASC001",
                        "CASCADE 사용 - 의존 객체가 자동 삭제될 수 있음"                        ,
                    )
                    .with_file(&file.name)
                    .with_line(stmt.line_number)
                    .with_object(&stmt.object_name)
                    .with_suggestion(
                        "명시적 삭제 순서 권장. CASCADE 제거 후 수동 정리.",
                    );

                    report.add_issue(issue);
                }
                // FK ON DELETE/UPDATE CASCADE는 의도된 참조 무결성 설계이므로 보고하지 않음
            }
        }
    }

    /// 순환 의존성 검사
    fn check_circular_dependencies(&self, report: &mut ValidationReport) {
        let cycles = report.graph.find_cycles();

        for cycle in cycles {
            if cycle.len() < 2 {
                continue;
            }

            let cycle_str = cycle.join(" → ");
            let issue = ValidationIssue::new(
                Severity::Error,
                "CIRC001",
                &format!("순환 의존성 발견: {} → {}", cycle_str, cycle[0]),
            )
            .with_suggestion("객체 정의 순서 재정렬 또는 의존성 분리 필요.");

            report.add_issue(issue);
        }
    }

    /// 멱등성 검사 (IF NOT EXISTS 누락)
    fn check_idempotency(&self, report: &mut ValidationReport) {
        for file in self.files {
            for stmt in &file.statements {
                match &stmt.statement_type {
                    StatementType::CreateTable
                    | StatementType::CreateIndex
                    | StatementType::CreateType => {
                        if !stmt.if_not_exists {
                            let issue = ValidationIssue::new(
                                Severity::Info,
                                "IDEM001",
                                "IF NOT EXISTS 누락 - 재실행 시 오류 발생 가능",
                            )
                            .with_file(&file.name)
                            .with_line(stmt.line_number)
                            .with_object(&stmt.object_name)
                            .with_suggestion("CREATE ... IF NOT EXISTS 사용 권장.");

                            report.add_issue(issue);
                        }
                    }
                    StatementType::DropTable
                    | StatementType::DropView
                    | StatementType::DropIndex
                    | StatementType::DropFunction
                    | StatementType::DropType => {
                        if !stmt.if_exists {
                            let issue = ValidationIssue::new(
                                Severity::Info,
                                "IDEM002",
                                "IF EXISTS 누락 - 재실행 시 오류 발생 가능",
                            )
                            .with_file(&file.name)
                            .with_line(stmt.line_number)
                            .with_object(&stmt.object_name)
                            .with_suggestion("DROP ... IF EXISTS 사용 권장.");

                            report.add_issue(issue);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// DROP 후 CREATE 패턴 검사 (데이터 손실 위험)
    fn check_drop_create_pattern(&self, report: &mut ValidationReport) {
        // 파일 순서대로 DROP/CREATE 추적
        let mut dropped_objects: HashMap<String, (String, usize)> = HashMap::new();
        let mut created_objects: HashMap<String, (String, usize)> = HashMap::new();

        for file in self.files {
            for stmt in &file.statements {
                let obj_lower = stmt.object_name.to_lowercase();

                match &stmt.statement_type {
                    StatementType::DropTable => {
                        dropped_objects.insert(obj_lower, (file.name.clone(), stmt.line_number));
                    }
                    StatementType::CreateTable => {
                        // 이전에 DROP된 테이블을 다시 CREATE
                        if let Some((drop_file, drop_line)) = dropped_objects.get(&obj_lower) {
                            // 같은 파일 내 DROP → CREATE는 허용 (마이그레이션 패턴)
                            if drop_file != &file.name {
                                let issue = ValidationIssue::new(
                                    Severity::Warning,
                                    "DCPAT001",
                                    &format!(
                                        "DROP → CREATE 패턴: '{}' 테이블이 삭제 후 재생성됨",
                                        stmt.object_name
                                    ),
                                )
                                .with_file(&file.name)
                                .with_line(stmt.line_number)
                                .with_object(&stmt.object_name)
                                .with_suggestion(&format!(
                                    "데이터 손실 위험. DROP 위치: {}:{}. ALTER TABLE 사용 검토.",
                                    drop_file, drop_line
                                ));

                                report.add_issue(issue);
                            }
                        }
                        created_objects.insert(obj_lower, (file.name.clone(), stmt.line_number));
                    }
                    _ => {}
                }
            }
        }
    }

    /// 뷰 의존성 검사
    fn check_view_dependencies(&self, report: &mut ValidationReport) {
        // 이슈 목록을 먼저 수집
        let mut issues = Vec::new();

        // 각 뷰가 참조하는 테이블/뷰 확인
        for file in self.files {
            for stmt in &file.statements {
                if matches!(
                    stmt.statement_type,
                    StatementType::CreateView | StatementType::CreateMaterializedView
                ) {
                    // 참조 객체가 정의되어 있는지 확인
                    for ref_obj in &stmt.references {
                        let ref_lower = ref_obj.to_lowercase();

                        // 시스템 객체 제외
                        if self.is_system_object(&ref_lower) {
                            continue;
                        }

                        // 정의 위치 확인
                        if let Some(defs) = report.graph.definitions.get(&ref_lower) {
                            // 현재 파일보다 나중에 정의된 객체 참조
                            let current_order = self.get_file_order(&file.name);
                            for (def_file, _) in defs {
                                let def_order = self.get_file_order(def_file);
                                if def_order > current_order {
                                    let issue = ValidationIssue::new(
                                        Severity::Warning,
                                        "VDEP001",
                                        &format!(
                                            "뷰 '{}'가 나중에 정의되는 '{}' 참조",
                                            stmt.object_name, ref_obj
                                        ),
                                    )
                                    .with_file(&file.name)
                                    .with_line(stmt.line_number)
                                    .with_suggestion(&format!(
                                        "'{}'를 먼저 정의하거나 뷰 정의 순서 변경 필요. 참조 위치: {}",
                                        ref_obj, def_file
                                    ));

                                    issues.push(issue);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 수집한 이슈 추가
        for issue in issues {
            report.add_issue(issue);
        }
    }

    /// IF NOT EXISTS 누락 상세 검사
    fn check_missing_if_not_exists(&self, report: &mut ValidationReport) {
        let mut table_creates: HashMap<String, Vec<(String, usize)>> = HashMap::new();

        for file in self.files {
            for stmt in &file.statements {
                if stmt.statement_type == StatementType::CreateTable {
                    let obj_lower = stmt.object_name.to_lowercase();
                    table_creates
                        .entry(obj_lower)
                        .or_default()
                        .push((file.name.clone(), stmt.line_number));
                }
            }
        }

        // 여러 번 생성되는 테이블 중 IF NOT EXISTS 없는 경우
        for (table, locations) in table_creates {
            if locations.len() > 1 {
                for (file_name, line) in &locations {
                    // 해당 문장 찾기
                    if let Some(file) = self.files.iter().find(|f| &f.name == file_name) {
                        if let Some(stmt) = file.statements.iter().find(|s| s.line_number == *line) {
                            if !stmt.if_not_exists {
                                let issue = ValidationIssue::new(
                                    Severity::Warning,
                                    "MULT001",
                                    &format!(
                                        "테이블 '{}'가 여러 번 정의되나 IF NOT EXISTS 누락",
                                        table
                                    ),
                                )
                                .with_file(file_name)
                                .with_line(*line)
                                .with_object(&table)
                                .with_suggestion(
                                    "중복 정의 제거 또는 IF NOT EXISTS 추가 필요.",
                                );

                                report.add_issue(issue);
                            }
                        }
                    }
                }
            }
        }
    }

    /// 데이터 안전성 검사
    fn check_data_safety(&self, report: &mut ValidationReport) {
        for file in self.files {
            for stmt in &file.statements {
                // 테이블 삭제 시 데이터 백업 권장
                if matches!(stmt.statement_type, StatementType::DropTable) {
                    // CASCADE 없이 DROP하는 경우는 양호
                    if !stmt.cascade {
                        continue;
                    }

                    let issue = ValidationIssue::new(
                        Severity::Warning,
                        "DATA001",
                        &format!(
                            "테이블 '{}' 삭제 - 데이터 손실 가능",
                            stmt.object_name
                        ),
                    )
                    .with_file(&file.name)
                    .with_line(stmt.line_number)
                    .with_object(&stmt.object_name)
                    .with_suggestion(
                        "삭제 전 데이터 백업 또는 INSERT INTO ... SELECT 로 마이그레이션 권장.",
                    );

                    report.add_issue(issue);
                }

                // ALTER TABLE DROP COLUMN
                if stmt.statement_type == StatementType::AlterTable {
                    let sql_upper = stmt.raw_sql.to_uppercase();
                    if sql_upper.contains("DROP COLUMN") {
                        let issue = ValidationIssue::new(
                            Severity::Warning,
                            "DATA002",
                            "ALTER TABLE DROP COLUMN - 컬럼 데이터 손실",
                        )
                        .with_file(&file.name)
                        .with_line(stmt.line_number)
                        .with_object(&stmt.object_name)
                        .with_suggestion("삭제 전 데이터 백업 필요. 롤백 시 복구 불가.");

                        report.add_issue(issue);
                    }

                    // 컬럼 타입 변경
                    if sql_upper.contains("ALTER COLUMN") && sql_upper.contains("TYPE") {
                        let issue = ValidationIssue::new(
                            Severity::Info,
                            "DATA003",
                            "ALTER COLUMN TYPE - 데이터 변환 주의",
                        )
                        .with_file(&file.name)
                        .with_line(stmt.line_number)
                        .with_object(&stmt.object_name)
                        .with_suggestion("타입 변환 시 데이터 손실 가능. USING 절로 변환 로직 명시 권장.");

                        report.add_issue(issue);
                    }
                }
            }
        }
    }

    /// 시스템 객체 여부 확인
    fn is_system_object(&self, name: &str) -> bool {
        let system_names = [
            "pg_", "information_schema", "now", "current_timestamp",
            "time_bucket", "create_hypertable", "gen_random_uuid",
        ];

        system_names.iter().any(|s| name.starts_with(s) || name == *s)
    }

    /// 파일 순서 번호 조회
    fn get_file_order(&self, file_name: &str) -> u32 {
        self.files
            .iter()
            .find(|f| f.name == file_name)
            .map(|f| f.order)
            .unwrap_or(0)
    }
}

/// 안전한 마이그레이션 체크리스트 생성
pub fn generate_safety_checklist(report: &ValidationReport) -> String {
    let mut checklist = String::new();

    checklist.push_str("═══════════════════════════════════════════════════════════════\n");
    checklist.push_str("                    안전한 마이그레이션 체크리스트\n");
    checklist.push_str("═══════════════════════════════════════════════════════════════\n\n");

    checklist.push_str("□ 1. 실행 전 확인\n");
    checklist.push_str("   □ 데이터베이스 백업 완료\n");
    checklist.push_str("   □ 테스트 환경에서 먼저 실행\n");
    checklist.push_str("   □ 롤백 계획 수립\n");
    checklist.push('\n');

    // CASCADE 사용 시 추가 체크
    let cascade_count = report
        .issues
        .iter()
        .filter(|i| i.code == "CASC001")
        .count();

    if cascade_count > 0 {
        checklist.push_str(&format!(
            "□ 2. CASCADE 사용 확인 ({} 건)\n",
            cascade_count
        ));
        checklist.push_str("   □ CASCADE로 삭제될 의존 객체 목록 확인\n");
        checklist.push_str("   □ 해당 데이터 백업 또는 마이그레이션\n");
        checklist.push('\n');
    }

    // 데이터 손실 위험 시
    let data_issues: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.code.starts_with("DATA") || i.code == "DCPAT001")
        .collect();

    if !data_issues.is_empty() {
        checklist.push_str(&format!(
            "□ 3. 데이터 안전성 ({} 건)\n",
            data_issues.len()
        ));
        for issue in data_issues {
            if let Some(ref obj) = issue.object {
                checklist.push_str(&format!("   □ '{}' 데이터 백업\n", obj));
            }
        }
        checklist.push('\n');
    }

    checklist.push_str("□ 4. 실행 후 확인\n");
    checklist.push_str("   □ 모든 테이블 접근 가능 확인\n");
    checklist.push_str("   □ 주요 쿼리 정상 동작 확인\n");
    checklist.push_str("   □ 애플리케이션 정상 작동 확인\n");
    checklist.push('\n');

    checklist.push_str("═══════════════════════════════════════════════════════════════\n");

    checklist
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_file(name: &str, order: u32, statements: Vec<SqlStatement>) -> MigrationFile {
        let defines: HashSet<String> = statements
            .iter()
            .filter(|s| s.statement_type.is_create())
            .map(|s| s.object_name.clone())
            .collect();

        MigrationFile {
            path: name.into(),
            name: name.to_string(),
            order,
            content: String::new(),
            statements,
            defines,
            depends_on: HashSet::new(),
        }
    }

    #[test]
    fn test_duplicate_detection() {
        let files = vec![
            create_test_file(
                "01",
                1,
                vec![SqlStatement::new(
                    StatementType::CreateTable,
                    "users".to_string(),
                    String::new(),
                    1,
                )],
            ),
            create_test_file(
                "02",
                2,
                vec![SqlStatement::new(
                    StatementType::CreateTable,
                    "users".to_string(),
                    String::new(),
                    1,
                )],
            ),
        ];

        let validator = MigrationValidator::new(&files);
        let report = validator.validate();

        assert!(report.issues.iter().any(|i| i.code == "DUP001"));
    }

    #[test]
    fn test_cascade_detection() {
        let mut stmt = SqlStatement::new(
            StatementType::DropTable,
            "old_table".to_string(),
            "DROP TABLE old_table CASCADE".to_string(),
            1,
        );
        stmt.cascade = true;
        stmt.fk_cascade = false;

        let files = vec![create_test_file("01", 1, vec![stmt])];

        let validator = MigrationValidator::new(&files);
        let report = validator.validate();

        assert!(report.issues.iter().any(|i| i.code == "CASC001"));
    }

    #[test]
    fn test_fk_cascade_not_reported() {
        let mut stmt = SqlStatement::new(
            StatementType::CreateTable,
            "child_table".to_string(),
            "CREATE TABLE child_table (id UUID, parent_id UUID REFERENCES parent(id) ON DELETE CASCADE)".to_string(),
            1,
        );
        stmt.cascade = false;
        stmt.fk_cascade = true;
        stmt.if_not_exists = true;

        let files = vec![create_test_file("01", 1, vec![stmt])];

        let validator = MigrationValidator::new(&files);
        let report = validator.validate();

        // FK CASCADE는 CASC001로 보고되지 않아야 함
        assert!(!report.issues.iter().any(|i| i.code == "CASC001"));
    }

    #[test]
    fn test_safety_checklist() {
        let mut report = ValidationReport::new();
        report.add_issue(ValidationIssue::new(
            Severity::Warning,
            "CASC001",
            "CASCADE 사용",
        ));
        report.add_issue(
            ValidationIssue::new(Severity::Warning, "DATA001", "데이터 손실")
                .with_object("users"),
        );

        let checklist = generate_safety_checklist(&report);

        assert!(checklist.contains("CASCADE"));
        assert!(checklist.contains("'users' 데이터 백업"));
    }
}
