"""Security Reviewer Agent

금융 트레이딩 시스템에 특화된 보안 검토 에이전트.
"""

import re
from typing import Any
from .base import BaseAgent


class SecurityReviewer(BaseAgent):
    """보안 검토 에이전트"""

    async def execute(self, arguments: dict[str, Any]) -> str:
        """보안 검토 실행"""
        self.log_progress("🔒 보안 검토 시작")

        target = arguments.get("target", "staged")
        severity_filter = arguments.get("severity", "all")  # all, critical, warning

        results = []
        results.append("# 🔒 Security Review Report\n\n")

        # Diff 가져오기
        self.log_progress(f"📄 변경사항 조회 중 (target: {target})")
        if target == "staged":
            diff = self._get_staged_diff()
        elif target == "commit":
            commit_hash = arguments.get("commit_hash", "HEAD")
            diff = self._get_commit_diff(commit_hash)
        elif target == "workspace":
            diff = None  # 전체 워크스페이스 스캔
        else:
            return self.format_error(
                "Unsupported Target",
                f"Target '{target}' is not supported"
            )

        # 보안 체크 항목
        issues = {
            "critical": [],
            "warning": [],
            "info": []
        }

        # 1. 코드 기반 체크
        if diff:
            self.log_progress("🔍 [1/3] 코드 보안 체크 중...")
            self._check_hardcoded_secrets(diff, issues)
            self._check_sql_injection(diff, issues)
            self._check_command_injection(diff, issues)
            self._check_sensitive_logging(diff, issues)
            self._check_unsafe_operations(diff, issues)
        else:
            # 워크스페이스 전체 스캔
            self.log_progress("🔍 [1/3] 워크스페이스 스캔 중...")
            self._scan_workspace(issues)

        # 2. 의존성 체크 (cargo audit)
        self.log_progress("🔍 [2/3] 의존성 취약점 체크 중...")
        self._check_dependencies(issues)

        # 3. 설정 파일 체크
        self.log_progress("🔍 [3/3] 설정 파일 체크 중...")
        self._check_config_files(issues)

        self.log_progress("✅ 보안 검토 완료")

        # 필터링
        if severity_filter != "all":
            issues = {severity_filter: issues.get(severity_filter, [])}

        # 결과 포맷팅
        total_issues = sum(len(v) for v in issues.values())

        if total_issues == 0:
            summary = self.format_success(
                "No Security Issues Found",
                "모든 보안 체크를 통과했습니다. ✅"
            )
        else:
            summary = self.format_warning(
                f"Security Issues Found: {total_issues}",
                f"Critical: {len(issues.get('critical', []))}, "
                f"Warning: {len(issues.get('warning', []))}, "
                f"Info: {len(issues.get('info', []))}"
            )

        results.insert(0, summary + "\n\n")

        # 이슈 상세
        if issues.get("critical"):
            results.append("## 🔴 Critical Issues\n\n")
            for idx, issue in enumerate(issues["critical"], 1):
                results.append(f"{idx}. **{issue['title']}**\n")
                results.append(f"   - Location: `{issue['location']}`\n")
                results.append(f"   - Risk: {issue['risk']}\n")
                results.append(f"   - Fix: {issue['fix']}\n\n")

        if issues.get("warning"):
            results.append("## 🟡 Warnings\n\n")
            for idx, issue in enumerate(issues["warning"], 1):
                results.append(f"{idx}. **{issue['title']}**\n")
                results.append(f"   - Location: `{issue['location']}`\n")
                results.append(f"   - Recommendation: {issue['fix']}\n\n")

        if issues.get("info"):
            results.append("## 🔵 Information\n\n")
            for idx, issue in enumerate(issues["info"], 1):
                results.append(f"{idx}. {issue['title']} - `{issue['location']}`\n")

        # Progress log 추가
        results.append(self.get_progress_section())

        return "\n".join(results)

    def _get_staged_diff(self) -> str:
        """스테이지된 변경사항"""
        self.logger.info("📄 Git diff 조회 중...")
        _, stdout, _ = self.run_command(["git", "diff", "--cached"], stream_output=True)
        return stdout

    def _get_commit_diff(self, commit_hash: str) -> str:
        """커밋 diff"""
        self.logger.info(f"📄 커밋 {commit_hash[:7]} diff 조회 중...")
        _, stdout, _ = self.run_command(["git", "show", commit_hash], stream_output=True)
        return stdout

    def _check_hardcoded_secrets(self, diff: str, issues: dict):
        """하드코딩된 비밀 정보 체크"""
        patterns = [
            (r'api[_-]?key\s*[:=]\s*["\']([a-zA-Z0-9\-_]{20,})["\']', "API Key"),
            (r'secret[_-]?key\s*[:=]\s*["\']([^"\']+)["\']', "Secret Key"),
            (r'password\s*[:=]\s*["\']([^"\']+)["\']', "Password"),
            (r'token\s*[:=]\s*["\']([a-zA-Z0-9\-_\.]{20,})["\']', "Token"),
            (r'(sk|pk)_live_[a-zA-Z0-9]{24,}', "API Key (Live)"),
            (r'Bearer\s+[a-zA-Z0-9\-_\.]{20,}', "Bearer Token"),
        ]

        for pattern, name in patterns:
            matches = re.finditer(pattern, diff, re.IGNORECASE)
            for match in matches:
                issues["critical"].append({
                    "title": f"{name} 하드코딩 발견",
                    "location": f"Line {diff[:match.start()].count(chr(10)) + 1}",
                    "risk": "인증 정보 유출 위험. Git 히스토리에 영구 저장됨.",
                    "fix": "환경 변수나 암호화된 설정 파일 사용 (dotenv, AWS Secrets Manager)"
                })

    def _check_sql_injection(self, diff: str, issues: dict):
        """SQL Injection 위험 체크"""
        # 동적 쿼리 조립 패턴
        patterns = [
            r'format!\s*\(\s*["\']SELECT.*?\{',
            r'format!\s*\(\s*["\']INSERT.*?\{',
            r'format!\s*\(\s*["\']UPDATE.*?\{',
            r'format!\s*\(\s*["\']DELETE.*?\{',
            r'\+\s*["\']SELECT',
            r'\+\s*["\']INSERT',
        ]

        for pattern in patterns:
            matches = re.finditer(pattern, diff, re.IGNORECASE)
            for match in matches:
                issues["critical"].append({
                    "title": "SQL Injection 위험",
                    "location": f"Line {diff[:match.start()].count(chr(10)) + 1}",
                    "risk": "사용자 입력이 쿼리에 직접 삽입되면 DB 탈취 가능",
                    "fix": "SQLx의 bind() 또는 query!() 매크로 사용"
                })

    def _check_command_injection(self, diff: str, issues: dict):
        """Command Injection 체크"""
        patterns = [
            r'Command::new\([^)]*format!',
            r'std::process::Command.*?\{',
            r'shell\s*=\s*True',  # Python
        ]

        for pattern in patterns:
            matches = re.finditer(pattern, diff)
            for match in matches:
                issues["critical"].append({
                    "title": "Command Injection 위험",
                    "location": f"Line {diff[:match.start()].count(chr(10)) + 1}",
                    "risk": "임의 시스템 명령 실행 가능",
                    "fix": "입력값 검증 및 화이트리스트 사용"
                })

    def _check_sensitive_logging(self, diff: str, issues: dict):
        """민감 데이터 로깅 체크"""
        patterns = [
            (r'(println!|log::info!|log::debug!).*password', "Password"),
            (r'(println!|log::info!|log::debug!).*api[_-]?key', "API Key"),
            (r'(println!|log::info!|log::debug!).*secret', "Secret"),
            (r'(println!|log::info!|log::debug!).*token', "Token"),
        ]

        for pattern, name in patterns:
            matches = re.finditer(pattern, diff, re.IGNORECASE)
            for match in matches:
                issues["warning"].append({
                    "title": f"{name} 로깅 발견",
                    "location": f"Line {diff[:match.start()].count(chr(10)) + 1}",
                    "fix": "민감 정보는 로그에서 마스킹 (예: ****)"
                })

    def _check_unsafe_operations(self, diff: str, issues: dict):
        """안전하지 않은 연산 체크"""
        patterns = [
            (r'\.unwrap\(\)', "unwrap() 사용", "? 연산자로 에러 전파"),
            (r'\.expect\([^)]*\)', "expect() 사용", "Result 타입으로 반환"),
            (r'unsafe\s*\{', "unsafe 블록", "안전성 검토 필요"),
            (r'transmute', "transmute 사용", "대안 검토 필요"),
        ]

        for pattern, title, fix in patterns:
            matches = re.finditer(pattern, diff)
            for match in matches:
                issues["warning"].append({
                    "title": title,
                    "location": f"Line {diff[:match.start()].count(chr(10)) + 1}",
                    "fix": fix
                })

    def _check_dependencies(self, issues: dict):
        """의존성 취약점 체크 (cargo audit)"""
        returncode, stdout, stderr = self.run_command(
            ["cargo", "audit"],
            timeout=60,
            stream_output=True
        )

        if returncode != 0:
            # 취약점 발견
            vulnerabilities = re.findall(r'(RUSTSEC-\d{4}-\d{4})', stdout + stderr)
            if vulnerabilities:
                for vuln_id in vulnerabilities[:5]:  # 최대 5개
                    issues["critical"].append({
                        "title": f"의존성 취약점: {vuln_id}",
                        "location": "Cargo.toml",
                        "risk": "알려진 보안 취약점이 있는 크레이트 사용",
                        "fix": "`cargo update` 또는 크레이트 버전 업그레이드"
                    })

    def _check_config_files(self, issues: dict):
        """설정 파일 보안 체크"""
        # .env 파일이 .gitignore에 있는지
        gitignore_path = self.project_root / ".gitignore"
        if gitignore_path.exists():
            gitignore_content = gitignore_path.read_text(encoding="utf-8")
            if ".env" not in gitignore_content:
                issues["warning"].append({
                    "title": ".env 파일이 .gitignore에 없음",
                    "location": ".gitignore",
                    "fix": ".gitignore에 .env 추가"
                })

        # CORS 설정 체크 (간단 버전)
        api_routes_path = self.project_root / "crates" / "trader-api" / "src" / "routes" / "mod.rs"
        if api_routes_path.exists():
            content = api_routes_path.read_text(encoding="utf-8")
            if "CorsLayer" not in content and "cors" not in content.lower():
                issues["info"].append({
                    "title": "CORS 설정 확인 필요",
                    "location": "crates/trader-api/src/routes/mod.rs"
                })

    def _scan_workspace(self, issues: dict):
        """워크스페이스 전체 스캔 (최적화 버전)"""
        self.logger.info("🔍 [1/4] 워크스페이스 스캔 시작...")
        
        # 모든 critical 패턴을 하나의 정규식으로 통합 (성능 개선)
        combined_pattern = r"(api[_-]?key|password|secret[_-]?key|token)\s*[:=]\s*[\"']"
        
        self.logger.info("🔍 [2/4] 하드코딩된 비밀 정보 검색 중...")
        returncode, stdout, _ = self.run_command(
            ["rg", combined_pattern, "--type", "rust", "-n", "-i", "-m", "30"],
            timeout=15  # 30초 → 15초
        )
        
        if returncode == 0 and stdout.strip():
            lines = stdout.strip().split('\n')
            for line in lines[:15]:  # 최대 15개
                parts = line.split(':', 2)
                if len(parts) >= 2:
                    location = f"{parts[0]}:{parts[1]}"
                    # 패턴 분석
                    if "api" in line.lower():
                        title = "API Key 하드코딩 가능성"
                        severity = "critical"
                    elif "password" in line.lower():
                        title = "Password 하드코딩 가능성"
                        severity = "critical"
                    else:
                        title = "Secret 하드코딩 가능성"
                        severity = "warning"
                    
                    issues[severity].append({
                        "title": title,
                        "location": location,
                        "fix": "환경 변수 사용 권장 (.env, AWS Secrets Manager)"
                    })
            self.logger.info(f"   ✓ {len(lines)} 개 잠재적 이슈 발견")
        else:
            self.logger.info("   ✓ 하드코딩된 비밀 정보 없음")

        # unwrap() 체크는 샘플만 (info) - 빠른 실행
        self.logger.info("🔍 [3/4] unwrap() 사용 검색 중...")
        returncode, stdout, _ = self.run_command(
            ["rg", r"\.unwrap\(\)", "--type", "rust", "-c"],  # -c: count only
            timeout=10  # 15초 → 10초
        )

        if returncode == 0 and stdout.strip():
            # 파일별 카운트 합산
            counts = [int(line.split(':')[1]) for line in stdout.strip().split('\n') if ':' in line]
            unwrap_count = sum(counts)
            
            if unwrap_count > 0:
                issues["info"].append({
                    "title": f"unwrap() 사용 ({unwrap_count}개 발견)",
                    "location": "워크스페이스 전체",
                    "fix": "프로덕션 코드에서는 ? 연산자 또는 Result 반환 권장"
                })
                self.logger.info(f"   ✓ {unwrap_count}개 unwrap() 발견")
        else:
            self.logger.info("   ✓ unwrap() 검색 완료")

        self.logger.info("🔍 [4/4] 워크스페이스 스캔 완료 ✅")
