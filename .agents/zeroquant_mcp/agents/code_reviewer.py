"""Code Reviewer Agent"""

import re
from typing import Any
from .base import BaseAgent


class CodeReviewer(BaseAgent):
    """코드 리뷰 에이전트"""

    async def execute(self, arguments: dict[str, Any]) -> str:
        """코드 리뷰 실행"""
        self.log_progress("🚀 코드 리뷰 시작")

        target = arguments.get("target", "staged")

        results = []
        results.append("# Code Review Report\n\n")

        # Git diff 가져오기
        self.log_progress(f"📄 변경사항 조회 중 (target: {target})")
        if target == "staged":
            diff = self._get_staged_diff()
        elif target == "commit":
            commit_hash = arguments.get("commit_hash", "HEAD")
            diff = self._get_commit_diff(commit_hash)
        else:
            return self.format_error(
                "Unsupported Target",
                f"Target '{target}' is not yet implemented"
            )

        if not diff:
            return self.format_warning(
                "No Changes",
                "변경사항이 없습니다."
            )

        # 분석 항목별 체크
        self.log_progress("🔍 [1/5] 코딩 스타일 체크 중...")
        coding_style = self._check_coding_style(diff)

        self.log_progress("🔍 [2/5] 보안 체크 중...")
        security = self._check_security(diff)

        self.log_progress("🔍 [3/5] 성능 체크 중...")
        performance = self._check_performance(diff)

        self.log_progress("🔍 [4/5] 테스트 커버리지 체크 중...")
        tests = self._check_tests(diff)

        self.log_progress("🔍 [5/5] 문서화 체크 중...")
        documentation = self._check_documentation(diff)
        
        checks = {
            "코딩 스타일": coding_style,
            "보안": security,
            "성능": performance,
            "테스트 커버리지": tests,
            "문서화": documentation,
        }
        
        self.log_progress("✅ 코드 리뷰 완료")

        passed_count = sum(1 for c in checks.values() if c["passed"])
        total_count = len(checks)

        # 요약
        if passed_count == total_count:
            results.append(self.format_success(
                f"All Checks Passed ({passed_count}/{total_count})",
                ""
            ))
        else:
            results.append(self.format_warning(
                f"Some Issues Found ({passed_count}/{total_count} passed)",
                ""
            ))

        results.append("\n---\n\n")

        # 각 체크 결과
        for idx, (name, result) in enumerate(checks.items(), 1):
            status = "✅" if result["passed"] else "⚠️"
            results.append(f"## {status} {idx}. {name}\n\n")
            results.append(f"**Status**: {'Pass' if result['passed'] else 'Issues Found'}\n\n")

            if not result["passed"] and result.get("issues"):
                results.append("**Issues**:\n")
                for issue in result["issues"][:5]:  # 최대 5개
                    results.append(f"- {issue}\n")
                results.append("\n")

        # Progress log 추가
        results.append(self.get_progress_section())

        return "\n".join(results)

    def _get_staged_diff(self) -> str:
        """스테이지된 변경사항 가져오기"""
        _, stdout, _ = self.run_command(["git", "diff", "--cached"], stream_output=True)
        return stdout

    def _get_commit_diff(self, commit_hash: str) -> str:
        """커밋 diff 가져오기"""
        self.logger.info(f"📄 커밋 {commit_hash[:7]} diff 조회 중...")
        _, stdout, _ = self.run_command(["git", "show", commit_hash], stream_output=True)
        return stdout

    def _check_coding_style(self, diff: str) -> dict:
        """코딩 스타일 체크"""
        issues = []

        # unwrap() 체크
        if re.search(r'\.unwrap\(\)', diff):
            issues.append("`unwrap()` 사용 발견 (프로덕션 코드에서 금지)")

        # f64 체크 (금융 계산)
        if re.search(r':\s*f64', diff):
            issues.append("`f64` 타입 사용 (금융 계산은 Decimal 사용 필수)")

        # 주석 체크 (한글이 아닌 경우)
        comment_pattern = re.compile(r'//\s*([a-zA-Z].*)')
        matches = comment_pattern.findall(diff)
        if matches and not any(ord(c) >= 0x1100 for m in matches for c in m):
            issues.append("주석이 한글이 아닙니다")

        return {
            "passed": len(issues) == 0,
            "issues": issues
        }

    def _check_security(self, diff: str) -> dict:
        """보안 체크"""
        issues = []

        # SQL Injection 위험
        if re.search(r'format!\s*\(\s*["\']SELECT', diff, re.IGNORECASE):
            issues.append("동적 SQL 쿼리 조립 발견 (SQL Injection 위험)")

        # API 키 하드코딩
        if re.search(r'(api_key|api-key|apiKey)\s*=\s*["\'][^"\']+["\']', diff):
            issues.append("API 키 하드코딩 가능성")

        # unwrap() on Result
        if re.search(r'\.unwrap\(\)', diff):
            issues.append("에러 처리 누락 (unwrap 대신 ? 사용)")

        return {
            "passed": len(issues) == 0,
            "issues": issues
        }

    def _check_performance(self, diff: str) -> dict:
        """성능 체크"""
        issues = []

        # 불필요한 clone
        clone_count = len(re.findall(r'\.clone\(\)', diff))
        if clone_count > 5:
            issues.append(f"과도한 `.clone()` 사용 ({clone_count}회)")

        # String 할당
        if re.search(r'\.to_string\(\)', diff):
            issues.append("String 할당 최적화 가능 (&str 사용 고려)")

        return {
            "passed": len(issues) == 0,
            "issues": issues
        }

    def _check_tests(self, diff: str) -> dict:
        """테스트 커버리지 체크"""
        issues = []

        # 새 함수가 추가되었는지 체크
        fn_pattern = re.compile(r'\+\s*pub\s+fn\s+(\w+)')
        new_functions = fn_pattern.findall(diff)

        # 테스트가 추가되었는지 체크
        test_pattern = re.compile(r'\+\s*#\[test\]')
        new_tests = len(test_pattern.findall(diff))

        if len(new_functions) > 0 and new_tests == 0:
            issues.append(f"새 함수 {len(new_functions)}개 추가됐지만 테스트 없음")

        return {
            "passed": len(issues) == 0,
            "issues": issues
        }

    def _check_documentation(self, diff: str) -> dict:
        """문서화 체크"""
        issues = []

        # 공개 함수에 문서 주석이 있는지
        fn_pattern = re.compile(r'\+\s*pub\s+fn\s+(\w+)')
        new_functions = fn_pattern.findall(diff)

        doc_pattern = re.compile(r'\+\s*///\s')
        doc_count = len(doc_pattern.findall(diff))

        if len(new_functions) > 0 and doc_count == 0:
            issues.append(f"새 공개 함수 {len(new_functions)}개에 문서 주석 없음")

        return {
            "passed": len(issues) == 0,
            "issues": issues
        }
