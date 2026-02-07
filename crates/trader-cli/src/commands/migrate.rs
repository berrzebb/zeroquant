//! 마이그레이션 관리 CLI 명령어.
//!
//! # 사용법
//!
//! ```bash
//! # 현재 마이그레이션 검증
//! trader migrate verify
//! trader migrate verify --verbose
//!
//! # 통합 계획 생성 (dry-run)
//! trader migrate consolidate --dry-run
//!
//! # 통합 실행
//! trader migrate consolidate --output migrations_v2
//!
//! # 의존성 그래프 시각화
//! trader migrate graph --format mermaid > graph.md
//!
//! # 마이그레이션 적용 (sqlx 래퍼)
//! trader migrate apply --db-url "postgres://..." --dir migrations_v2
//! ```

use std::path::PathBuf;

use trader_core::migration::{
    MigrationAnalyzer, MigrationConsolidator, MigrationValidator,
    DependencyGraph, generate_safety_checklist,
};

/// 마이그레이션 설정
#[derive(Debug, Clone)]
pub struct MigrateConfig {
    /// 마이그레이션 디렉토리
    pub migrations_dir: PathBuf,
    /// 출력 디렉토리 (통합 시)
    pub output_dir: Option<PathBuf>,
    /// 상세 출력
    pub verbose: bool,
    /// Dry-run 모드
    pub dry_run: bool,
    /// 그래프 출력 형식
    pub graph_format: GraphFormat,
    /// 데이터베이스 URL (apply 시)
    pub db_url: Option<String>,
}

impl Default for MigrateConfig {
    fn default() -> Self {
        Self {
            migrations_dir: PathBuf::from("migrations"),
            output_dir: None,
            verbose: false,
            dry_run: false,
            graph_format: GraphFormat::Mermaid,
            db_url: None,
        }
    }
}

/// 그래프 출력 형식
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphFormat {
    /// Mermaid 다이어그램
    Mermaid,
    /// DOT (Graphviz)
    Dot,
    /// 텍스트
    Text,
}

impl GraphFormat {
    /// 문자열에서 파싱
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "mermaid" | "md" => Some(Self::Mermaid),
            "dot" | "graphviz" => Some(Self::Dot),
            "text" | "txt" => Some(Self::Text),
            _ => None,
        }
    }
}

/// 마이그레이션 검증 실행
pub fn run_verify(config: &MigrateConfig) -> Result<bool, String> {
    println!("\n🔍 마이그레이션 검증 시작...\n");

    let analyzer = MigrationAnalyzer::new();
    let files = analyzer.scan_directory(&config.migrations_dir)?;

    if files.is_empty() {
        return Err(format!(
            "마이그레이션 파일을 찾을 수 없습니다: {:?}",
            config.migrations_dir
        ));
    }

    println!("📁 {} 개 마이그레이션 파일 발견", files.len());

    if config.verbose {
        for file in &files {
            println!("   {:02}. {} ({} 문장)", file.order, file.name, file.statements.len());
        }
        println!();
    }

    let validator = MigrationValidator::new(&files);
    let report = validator.validate();

    // 보고서 출력
    println!("{}", report);

    // 안전 체크리스트 출력 (문제가 있을 때만)
    if !report.is_valid() || report.warning_count() > 0 {
        println!("{}", generate_safety_checklist(&report));
    }

    if config.verbose {
        // 의존성 그래프 요약
        println!("\n📊 의존성 그래프 요약");
        println!("  정의된 객체: {} 개", report.graph.definitions.len());
        println!("  의존 관계: {} 개", report.graph.dependencies.len());

        let duplicates = report.graph.find_duplicates();
        if !duplicates.is_empty() {
            println!("\n  ⚠️ 중복 정의 객체:");
            for (obj, locs) in &duplicates {
                let loc_strs: Vec<_> = locs.iter().map(|(f, l)| format!("{}:{}", f, l)).collect();
                println!("    - {}: {}", obj, loc_strs.join(", "));
            }
        }
    }

    Ok(report.is_valid())
}

/// 마이그레이션 통합 실행
pub fn run_consolidate(config: &MigrateConfig) -> Result<(), String> {
    println!("\n📦 마이그레이션 통합 시작...\n");

    let analyzer = MigrationAnalyzer::new();
    let files = analyzer.scan_directory(&config.migrations_dir)?;

    if files.is_empty() {
        return Err("마이그레이션 파일을 찾을 수 없습니다".to_string());
    }

    let consolidator = MigrationConsolidator::new();
    let plan = consolidator.plan(&files);

    if config.dry_run {
        // Dry-run 결과 출력
        println!("{}", consolidator.dry_run(&plan));
        println!("\n✅ Dry-run 완료. 실제 적용하려면 --dry-run 제거 후 재실행.");
        return Ok(());
    }

    // 출력 디렉토리 결정
    let output_dir = config
        .output_dir
        .as_ref()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("migrations_v2"));

    // 통합 실행
    println!("{}", plan);
    consolidator.execute(&plan, &output_dir)?;

    println!("\n✅ 통합 완료!");
    println!("   출력 디렉토리: {:?}", output_dir);
    println!("   생성된 파일: {} 개", plan.files.len());
    println!("   감소율: {:.1}%", plan.reduction_percentage());

    // 다음 단계 안내
    println!("\n📝 다음 단계:");
    println!("   1. 통합된 마이그레이션 검토: cat {:?}/*.sql", output_dir);
    println!("   2. 테스트 DB에서 검증: trader migrate apply --db-url <TEST_DB> --dir {:?}", output_dir);
    println!("   3. 스키마 비교 확인 후 운영 적용");

    Ok(())
}

/// 의존성 그래프 출력
pub fn run_graph(config: &MigrateConfig) -> Result<String, String> {
    let analyzer = MigrationAnalyzer::new();
    let files = analyzer.scan_directory(&config.migrations_dir)?;
    let graph = analyzer.build_dependency_graph(&files);

    let output = match config.graph_format {
        GraphFormat::Mermaid => generate_mermaid_graph(&graph, &files),
        GraphFormat::Dot => generate_dot_graph(&graph, &files),
        GraphFormat::Text => generate_text_graph(&graph, &files),
    };

    Ok(output)
}

/// Mermaid 다이어그램 생성
fn generate_mermaid_graph(graph: &DependencyGraph, files: &[trader_core::migration::MigrationFile]) -> String {
    let mut output = String::new();

    output.push_str("```mermaid\n");
    output.push_str("graph TD\n");
    output.push_str("    subgraph \"마이그레이션 파일 의존성\"\n");

    // 파일 노드
    for file in files {
        output.push_str(&format!(
            "        {}[\"{}\"]\n",
            file.name.replace(['.', '-'], "_"),
            file.name
        ));
    }

    // 파일 간 의존성 엣지
    for (file, deps) in &graph.file_dependencies {
        for dep in deps {
            output.push_str(&format!(
                "        {} --> {}\n",
                file.replace(['.', '-'], "_"),
                dep.replace(['.', '-'], "_")
            ));
        }
    }

    output.push_str("    end\n");
    output.push_str("```\n\n");

    // 객체 의존성 (주요 객체만)
    output.push_str("```mermaid\n");
    output.push_str("graph LR\n");
    output.push_str("    subgraph \"주요 객체 의존성\"\n");

    let mut shown = std::collections::HashSet::new();
    for (obj, deps) in &graph.dependencies {
        if deps.len() > 0 && !obj.starts_with("idx_") && !obj.starts_with("v_") {
            for dep in deps {
                if !dep.starts_with("idx_") && shown.len() < 50 {
                    output.push_str(&format!(
                        "        {} --> {}\n",
                        obj.replace(['.', '-'], "_"),
                        dep.replace(['.', '-'], "_")
                    ));
                    shown.insert((obj.clone(), dep.clone()));
                }
            }
        }
    }

    output.push_str("    end\n");
    output.push_str("```\n");

    output
}

/// DOT 그래프 생성
fn generate_dot_graph(graph: &DependencyGraph, files: &[trader_core::migration::MigrationFile]) -> String {
    let mut output = String::new();

    output.push_str("digraph MigrationDependencies {\n");
    output.push_str("    rankdir=LR;\n");
    output.push_str("    node [shape=box];\n\n");

    // 파일 서브그래프
    output.push_str("    subgraph cluster_files {\n");
    output.push_str("        label=\"Migration Files\";\n");
    for file in files {
        output.push_str(&format!("        \"{}\";\n", file.name));
    }
    output.push_str("    }\n\n");

    // 파일 의존성
    for (file, deps) in &graph.file_dependencies {
        for dep in deps {
            output.push_str(&format!("    \"{}\" -> \"{}\";\n", file, dep));
        }
    }

    output.push_str("}\n");

    output
}

/// 텍스트 그래프 생성
fn generate_text_graph(graph: &DependencyGraph, files: &[trader_core::migration::MigrationFile]) -> String {
    let mut output = String::new();

    output.push_str("═══════════════════════════════════════════════════════════════\n");
    output.push_str("                    마이그레이션 의존성 그래프\n");
    output.push_str("═══════════════════════════════════════════════════════════════\n\n");

    output.push_str("📁 파일별 의존성\n");
    output.push_str("───────────────────────────────────────────────────────────────\n");

    for file in files {
        output.push_str(&format!("\n{} (순서: {})\n", file.name, file.order));

        if let Some(deps) = graph.file_dependencies.get(&file.name) {
            if deps.is_empty() {
                output.push_str("  └── (의존성 없음)\n");
            } else {
                for dep in deps {
                    output.push_str(&format!("  └── {}\n", dep));
                }
            }
        } else {
            output.push_str("  └── (의존성 없음)\n");
        }
    }

    output.push_str("\n\n📊 객체 정의\n");
    output.push_str("───────────────────────────────────────────────────────────────\n");

    let mut sorted_defs: Vec<_> = graph.definitions.iter().collect();
    sorted_defs.sort_by_key(|(name, _)| name.as_str());

    for (obj, locations) in sorted_defs {
        if !obj.starts_with("idx_") {
            let loc_strs: Vec<_> = locations.iter().map(|(f, l)| format!("{}:{}", f, l)).collect();
            output.push_str(&format!("  {} @ {}\n", obj, loc_strs.join(", ")));
        }
    }

    output.push_str("\n═══════════════════════════════════════════════════════════════\n");

    output
}

/// 마이그레이션 적용
///
/// 이 함수는 sqlx migrate run 명령을 래핑합니다.
/// 실제 적용 전 항상 검증을 수행합니다.
pub async fn run_apply(config: &MigrateConfig) -> Result<(), String> {
    println!("\n🚀 마이그레이션 적용 시작...\n");

    // 1. 먼저 검증 수행
    println!("1️⃣ 마이그레이션 검증 중...");
    let verify_config = MigrateConfig {
        migrations_dir: config.migrations_dir.clone(),
        verbose: false,
        ..Default::default()
    };

    let is_valid = run_verify(&verify_config)?;
    if !is_valid {
        return Err("검증 실패: 에러를 수정한 후 다시 시도하세요.".to_string());
    }

    println!("\n✅ 검증 통과\n");

    // 2. 데이터베이스 URL 확인
    let db_url = config.db_url.clone()
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .ok_or("DATABASE_URL이 설정되지 않았습니다. --db-url 옵션 사용")?;

    println!("2️⃣ 데이터베이스 연결 확인...");
    println!("   URL: {}...{}", &db_url[..20.min(db_url.len())], &db_url[db_url.len().saturating_sub(20)..]);

    // 3. sqlx migrate run 실행
    println!("\n3️⃣ 마이그레이션 실행...");

    let migrations_path = config.migrations_dir.to_string_lossy().to_string();

    let output = std::process::Command::new("sqlx")
        .args([
            "migrate",
            "run",
            "--source",
            &migrations_path,
            "--database-url",
            &db_url,
        ])
        .output()
        .map_err(|e| format!("sqlx 실행 실패: {}. sqlx-cli가 설치되어 있는지 확인하세요.", e))?;

    if output.status.success() {
        println!("\n✅ 마이그레이션 적용 완료!");
        println!("{}", String::from_utf8_lossy(&output.stdout));
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("마이그레이션 실패:\n{}", stderr))
    }
}

/// 마이그레이션 상태 확인
pub async fn run_status(config: &MigrateConfig) -> Result<(), String> {
    let db_url = config.db_url.clone()
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .ok_or("DATABASE_URL이 설정되지 않았습니다")?;

    let migrations_path = config.migrations_dir.to_string_lossy().to_string();

    let output = std::process::Command::new("sqlx")
        .args([
            "migrate",
            "info",
            "--source",
            &migrations_path,
            "--database-url",
            &db_url,
        ])
        .output()
        .map_err(|e| format!("sqlx 실행 실패: {}", e))?;

    println!("{}", String::from_utf8_lossy(&output.stdout));

    if !output.status.success() {
        println!("{}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_format_parse() {
        assert_eq!(GraphFormat::parse("mermaid"), Some(GraphFormat::Mermaid));
        assert_eq!(GraphFormat::parse("DOT"), Some(GraphFormat::Dot));
        assert_eq!(GraphFormat::parse("text"), Some(GraphFormat::Text));
        assert_eq!(GraphFormat::parse("invalid"), None);
    }

    #[test]
    fn test_default_config() {
        let config = MigrateConfig::default();
        assert_eq!(config.migrations_dir, PathBuf::from("migrations"));
        assert!(!config.verbose);
        assert!(!config.dry_run);
    }
}
