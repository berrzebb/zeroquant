"""Code Simplifier Agent"""

import re
from typing import Any
from pathlib import Path
from .base import BaseAgent


class CodeSimplifier(BaseAgent):
    """코드 단순화 분석 에이전트"""

    async def execute(self, arguments: dict[str, Any]) -> str:
        """코드 단순화 분석 실행"""
        self.log_progress("🧹 코드 단순화 분석 시작")

        scope = arguments.get("scope", "workspace")
        priority = arguments.get("priority", "all")

        results = []
        results.append("# Code Simplification Report\n\n")

        # 분석 대상 결정
        if scope == "workspace":
            target_path = self.project_root
        elif scope == "crate":
            crate_name = arguments.get("crate_name")
            target_path = self.project_root / "crates" / crate_name
        else:
            return self.format_error(
                "Unsupported Scope",
                f"Scope '{scope}' is not yet fully implemented"
            )

        results.append(f"**분석 범위**: {scope}\n")
        results.append(f"**경로**: {target_path}\n\n")
        results.append("---\n\n")

        # 분석 실행
        issues = []

        # 1. 중복 코드
        self.log_progress("🔍 [1/3] 중복 코드 검색 중")
        duplicates = self._find_duplicates(target_path)
        if duplicates:
            issues.append({
                "priority": "high",
                "category": "중복 코드",
                "count": len(duplicates),
                "details": duplicates[:3]  # 최대 3개
            })

        # 2. 복잡도
        self.log_progress("🔍 [2/3] 복잡한 함수 검색 중")
        complex_functions = self._find_complex_functions(target_path)
        if complex_functions:
            issues.append({
                "priority": "medium",
                "category": "복잡도 초과",
                "count": len(complex_functions),
                "details": complex_functions[:3]
            })

        # 3. 레거시 코드
        self.log_progress("🔍 [3/3] 레거시 코드 검색 중")
        legacy_code = self._find_legacy_code(target_path)

        if legacy_code:
            issues.append({
                "priority": "low",
                "category": "레거시 코드",
                "count": len(legacy_code),
                "details": legacy_code[:3]
            })

        self.log_progress("✅ 코드 단순화 분석 완료")

        # 우선순위 필터
        if priority != "all":
            issues = [i for i in issues if i["priority"] == priority]

        # 결과 출력
        if not issues:
            results.append(self.format_success(
                "No Issues Found",
                "코드베이스가 깔끔합니다!"
            ))
        else:
            for issue in issues:
                priority_emoji = {
                    "high": "🔴",
                    "medium": "🟡",
                    "low": "🟢"
                }[issue["priority"]]

                results.append(f"## {priority_emoji} {issue['category']}\n\n")
                results.append(f"**우선순위**: {issue['priority']}\n")
                results.append(f"**발견**: {issue['count']}개\n\n")

                results.append("**예시**:\n")
                for detail in issue["details"]:
                    results.append(f"- {detail}\n")
                results.append("\n")

        results.append(self.get_progress_section())

        return "\n".join(results)

    def _find_duplicates(self, path: Path) -> list[str]:
        """중복 코드 찾기"""
        duplicates = []

        # unwrap() 사용
        _, stdout, _ = self.run_command([
            "rg",
            "-n",
            "--type", "rust",
            r"\.unwrap\(\)",
            str(path)
        ], stream_output=True)

        if stdout.strip():
            lines = stdout.strip().split('\n')
            duplicates.append(f"`unwrap()` {len(lines)}회 사용")

        # clone() 패턴
        _, stdout, _ = self.run_command([
            "rg",
            "-n",
            "--type", "rust",
            r"\.clone\(\)",
            str(path)
        ], stream_output=True)

        if stdout.strip():
            lines = stdout.strip().split('\n')
            if len(lines) > 50:
                duplicates.append(f"`clone()` {len(lines)}회 사용 (과도)")

        return duplicates

    def _find_complex_functions(self, path: Path) -> list[str]:
        """복잡한 함수 찾기"""
        complex = []

        # 100줄 이상 함수 찾기 (간단한 휴리스틱)
        rust_files = list(path.rglob("*.rs"))

        for file in rust_files[:10]:  # 최대 10개 파일
            try:
                content = file.read_text(encoding='utf-8')
                lines = content.split('\n')

                in_function = False
                fn_start = 0
                fn_name = ""

                for i, line in enumerate(lines):
                    if re.match(r'\s*(?:pub\s+)?fn\s+(\w+)', line):
                        fn_name = re.match(r'\s*(?:pub\s+)?fn\s+(\w+)', line).group(1)
                        fn_start = i
                        in_function = True
                    elif in_function and line.strip() == '}':
                        fn_length = i - fn_start
                        if fn_length > 100:
                            complex.append(
                                f"`{file.relative_to(path)}: {fn_name}()` - {fn_length}줄"
                            )
                        in_function = False

            except Exception:
                pass

        return complex

    def _find_legacy_code(self, path: Path) -> list[str]:
        """레거시 코드 찾기"""
        legacy = []

        # 주석 처리된 코드
        _, stdout, _ = self.run_command([
            "rg",
            "-n",
            "--type", "rust",
            r"^//\s*fn\s+\w+",
            str(path)
        ], stream_output=True)

        if stdout.strip():
            lines = stdout.strip().split('\n')
            legacy.append(f"주석 처리된 함수 {len(lines)}개")

        # TODO/FIXME
        _, stdout, _ = self.run_command([
            "rg",
            "-n",
            "--type", "rust",
            r"//\s*(TODO|FIXME)",
            str(path)
        ], stream_output=True)

        if stdout.strip():
            lines = stdout.strip().split('\n')
            legacy.append(f"TODO/FIXME {len(lines)}개")

        return legacy
