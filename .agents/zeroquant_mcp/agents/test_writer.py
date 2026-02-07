"""Test Writer Agent

자동으로 테스트 스켈레톤을 생성하고 커버리지를 분석하는 에이전트.
"""

import re
from typing import Any
from pathlib import Path
from .base import BaseAgent


class TestWriter(BaseAgent):
    """테스트 작성 에이전트"""

    async def execute(self, arguments: dict[str, Any]) -> str:
        """테스트 생성/분석 실행"""
        self.log_progress("🧪 테스트 작성 시작")

        target = arguments.get("target", "function")  # function, file, crate, coverage
        mode = arguments.get("mode", "generate")  # generate, analyze, check-coverage

        results = []
        results.append("# ✅ Test Writer Report\n\n")

        if mode == "check-coverage":
            # 커버리지 분석 모드
            return self._analyze_coverage(arguments)

        elif mode == "analyze":
            # 테스트 가능성 분석
            return self._analyze_testability(arguments)

        elif mode == "generate":
            # 테스트 생성
            if target == "function":
                return self._generate_function_test(arguments)
            elif target == "file":
                return self._generate_file_tests(arguments)
            elif target == "crate":
                return self._generate_crate_tests(arguments)
            else:
                return self.format_error(
                    "Unsupported Target",
                    f"Target '{target}' is not supported"
                )

        return "\n".join(results)

    def _generate_function_test(self, arguments: dict[str, Any]) -> str:
        """함수에 대한 테스트 생성"""
        self.log_progress("🧪 [1/4] 함수 테스트 생성 시작")

        function_path = arguments.get("function_path")  # "file.rs::function_name"

        if not function_path or "::" not in function_path:
            return self.format_error(
                "Invalid Function Path",
                "function_path 형식: 'path/to/file.rs::function_name'"
            )

        file_path, function_name = function_path.rsplit("::", 1)
        full_path = self.project_root / file_path

        if not full_path.exists():
            return self.format_error("File Not Found", f"파일을 찾을 수 없습니다: {file_path}")

        # 파일 읽기
        try:
            content = full_path.read_text(encoding="utf-8")
        except Exception as e:
            return self.format_error("Read Error", str(e))

        # 함수 시그니처 찾기
        self.log_progress(f"🔍 [2/4] 함수 시그니처 검색: {function_name}")
        function_sig = self._find_function_signature(content, function_name)
        if not function_sig:
            return self.format_error(
                "Function Not Found",
                f"함수를 찾을 수 없습니다: {function_name}"
            )

        # 함수 분석
        self.log_progress("📊 [3/4] 함수 시그니처 분석 중")
        analysis = self._analyze_function(function_sig, content)

        # 테스트 생성
        self.log_progress("✍️ [4/4] 테스트 코드 생성 중")
        test_code = self._generate_test_code(function_name, analysis)

        self.log_progress("✅ 테스트 생성 완료")

        results = []
        results.append("# ✅ Test Generated\n\n")
        results.append(f"**Function**: `{function_name}`\n")
        results.append(f"**File**: `{file_path}`\n\n")

        results.append("## 📊 분석 결과\n\n")
        results.append(f"- **파라미터**: {len(analysis['params'])}개\n")
        results.append(f"- **반환 타입**: `{analysis['return_type']}`\n")
        results.append(f"- **에러 처리**: {'Result 사용' if analysis['returns_result'] else '없음'}\n\n")

        results.append("## 🧪 생성된 테스트\n\n")
        results.append("```rust\n")
        results.append(test_code)
        results.append("\n```\n\n")

        results.append("## 💡 추가 테스트 제안\n\n")
        for suggestion in analysis.get('suggestions', []):
            results.append(f"- {suggestion}\n")

        results.append(self.get_progress_section())

        return "\n".join(results)

    def _find_function_signature(self, content: str, function_name: str) -> str | None:
        """함수 시그니처 찾기"""
        # pub fn function_name(...) -> ... 패턴
        pattern = rf'(pub\s+)?fn\s+{re.escape(function_name)}\s*[<(].*?(?:\{{|;)'
        match = re.search(pattern, content, re.DOTALL)

        if match:
            sig = match.group(0)
            # { 또는 ; 제거
            sig = sig.rstrip('{').rstrip(';').strip()
            return sig
        return None

    def _analyze_function(self, signature: str, file_content: str) -> dict:
        """함수 분석"""
        analysis = {
            'params': [],
            'return_type': 'void',
            'returns_result': False,
            'suggestions': []
        }

        # 파라미터 추출
        params_match = re.search(r'\((.*?)\)', signature, re.DOTALL)
        if params_match:
            params_str = params_match.group(1)
            # self 제외
            params = [p.strip() for p in params_str.split(',') if p.strip() and 'self' not in p.strip()]
            analysis['params'] = params

        # 반환 타입 추출
        return_match = re.search(r'->\s*([^{;]+)', signature)
        if return_match:
            return_type = return_match.group(1).strip()
            analysis['return_type'] = return_type
            analysis['returns_result'] = 'Result' in return_type

        # 테스트 제안 생성
        if analysis['returns_result']:
            analysis['suggestions'].append("Ok 케이스와 Err 케이스 모두 테스트")

        if any('Decimal' in p for p in analysis['params']):
            analysis['suggestions'].append("경계값 테스트 (0, 음수, 최대값)")

        if any('String' in p or '&str' in p for p in analysis['params']):
            analysis['suggestions'].append("빈 문자열 테스트")

        if any('Option' in p for p in analysis['params']):
            analysis['suggestions'].append("None 케이스 테스트")

        return analysis

    def _generate_test_code(self, function_name: str, analysis: dict) -> str:
        """테스트 코드 생성"""
        lines = []
        lines.append("#[cfg(test)]")
        lines.append("mod tests {")
        lines.append("    use super::*;")

        # Decimal이 있으면 매크로 import
        if any('Decimal' in str(p) for p in analysis['params']):
            lines.append("    use rust_decimal_macros::dec;")

        lines.append("")

        # 기본 성공 케이스
        lines.append(f"    #[test]")
        lines.append(f"    fn test_{function_name}_success() {{")
        lines.append(f"        // TODO: 적절한 테스트 데이터 준비")

        # 파라미터 예시 생성
        param_examples = []
        for param in analysis['params']:
            if 'Decimal' in param:
                param_examples.append("dec!(100.0)")
            elif 'String' in param:
                param_examples.append('"test".to_string()')
            elif '&str' in param:
                param_examples.append('"test"')
            elif 'i32' in param or 'i64' in param:
                param_examples.append("42")
            elif 'bool' in param:
                param_examples.append("true")
            else:
                param_examples.append("/* TODO */")

        params_str = ", ".join(param_examples)
        lines.append(f"        let result = {function_name}({params_str});")

        if analysis['returns_result']:
            lines.append("        assert!(result.is_ok());")
            lines.append("        // assert_eq!(result.unwrap(), expected_value);")
        else:
            lines.append("        // assert_eq!(result, expected_value);")

        lines.append("    }")
        lines.append("")

        # Result 타입이면 에러 케이스 추가
        if analysis['returns_result']:
            lines.append(f"    #[test]")
            lines.append(f"    fn test_{function_name}_error_case() {{")
            lines.append(f"        // TODO: 에러를 발생시킬 입력 준비")
            lines.append(f"        let result = {function_name}(/* invalid input */);")
            lines.append("        assert!(result.is_err());")
            lines.append("    }")
            lines.append("")

        # Edge case 테스트
        if any('Decimal' in str(p) for p in analysis['params']):
            lines.append(f"    #[test]")
            lines.append(f"    fn test_{function_name}_edge_cases() {{")
            lines.append("        // Zero")
            lines.append(f"        let result = {function_name}(dec!(0.0));")
            lines.append("        // TODO: assertion")
            lines.append("")
            lines.append("        // Negative")
            lines.append(f"        let result = {function_name}(dec!(-100.0));")
            lines.append("        // TODO: assertion")
            lines.append("    }")

        lines.append("}")

        return "\n".join(lines)

    def _analyze_coverage(self, arguments: dict[str, Any]) -> str:
        """테스트 커버리지 분석"""
        self.log_progress("📊 [1/3] 커버리지 분석 시작")

        crate_name = arguments.get("crate_name")

        results = []
        results.append("# 📊 Test Coverage Analysis\n\n")

        # cargo test 실행하여 테스트 수 확인
        if crate_name:
            cmd = ["cargo", "test", "-p", crate_name, "--", "--list"]
        else:
            cmd = ["cargo", "test", "--workspace", "--", "--list"]

        self.log_progress("🧪 [2/3] 테스트 목록 조회 중 (최대 1분 소요)")
        returncode, stdout, _ = self.run_command(cmd, timeout=60, stream_output=True)

        if returncode == 0:
            # 테스트 수 카운트
            test_count = stdout.count(": test")
            results.append(f"**총 테스트 수**: {test_count}개\n\n")

        # ripgrep으로 pub fn 함수 찾기
        if crate_name:
            search_path = self.project_root / "crates" / crate_name
        else:
            search_path = self.project_root / "crates"

        returncode, stdout, _ = self.run_command([
            "rg",
            r"^\s*pub\s+fn\s+\w+",
            str(search_path),
            "--type", "rust",
            "-c"  # count only
        ])

        if returncode == 0:
            # 파일별 함수 수
            file_counts = stdout.strip().split('\n')
            total_functions = sum(int(line.split(':')[-1]) for line in file_counts if ':' in line)
            results.append(f"**공개 함수 수**: {total_functions}개\n\n")

            if test_count > 0 and total_functions > 0:
                coverage_pct = (test_count / total_functions) * 100
                results.append(f"**예상 커버리지**: {coverage_pct:.1f}%\n\n")

        # 테스트 없는 파일 찾기
        self.log_progress("🔍 [3/3] 테스트 없는 모듈 검색 중")
        results.append("## ⚠️ 테스트 없는 모듈\n\n")

        returncode, stdout, _ = self.run_command([
            "rg",
            r"pub\s+fn\s+\w+",
            str(search_path),
            "--type", "rust",
            "--files-without-match", "test"
        ])

        if returncode == 0 and stdout.strip():
            untested_files = stdout.strip().split('\n')[:10]
            for file in untested_files:
                rel_path = Path(file).relative_to(self.project_root)
                results.append(f"- `{rel_path}`\n")

        self.log_progress("✅ 커버리지 분석 완료")
        results.append(self.get_progress_section())

        return "\n".join(results)

    def _analyze_testability(self, arguments: dict[str, Any]) -> str:
        """테스트 가능성 분석"""
        self.log_progress("🔍 테스트 가능성 분석 시작")

        file_path = arguments.get("file_path")

        if not file_path:
            return self.format_error("Missing Parameter", "file_path가 필요합니다")

        full_path = self.project_root / file_path
        if not full_path.exists():
            return self.format_error("File Not Found", f"{file_path}")

        try:
            content = full_path.read_text(encoding="utf-8")
        except Exception as e:
            return self.format_error("Read Error", str(e))

        results = []
        results.append("# 🔍 Testability Analysis\n\n")
        results.append(f"**File**: `{file_path}`\n\n")

        # 공개 함수 찾기
        pub_functions = re.findall(r'pub\s+fn\s+(\w+)', content)
        results.append(f"**공개 함수**: {len(pub_functions)}개\n\n")

        # 복잡도 높은 함수 (줄 수로 간단 추정)
        complex_functions = []
        for func_name in pub_functions:
            pattern = rf'pub\s+fn\s+{re.escape(func_name)}.*?\n\}}'
            match = re.search(pattern, content, re.DOTALL)
            if match:
                lines = match.group(0).count('\n')
                if lines > 50:  # 50줄 이상
                    complex_functions.append((func_name, lines))

        if complex_functions:
            results.append("## ⚠️ 복잡한 함수 (우선 테스트 권장)\n\n")
            for func, lines in sorted(complex_functions, key=lambda x: x[1], reverse=True):
                results.append(f"- `{func}` ({lines} lines)\n")
            results.append("\n")

        # 기존 테스트 확인
        has_tests = "#[cfg(test)]" in content or "#[test]" in content
        results.append(f"**기존 테스트 존재**: {'예' if has_tests else '아니오'}\n\n")

        if not has_tests:
            results.append("💡 **제안**: 테스트 모듈 생성 후 주요 함수부터 테스트 작성\n")

        self.log_progress("✅ 테스트 가능성 분석 완료")
        results.append(self.get_progress_section())

        return "\n".join(results)

    def _generate_file_tests(self, arguments: dict[str, Any]) -> str:
        """파일 전체에 대한 테스트 생성"""
        file_path = arguments.get("file_path")

        if not file_path:
            return self.format_error("Missing Parameter", "file_path가 필요합니다")

        # TODO: 구현
        return self.format_warning(
            "Not Implemented",
            "파일 단위 테스트 생성은 아직 구현되지 않았습니다. "
            "target='function'을 사용하세요."
        )

    def _generate_crate_tests(self, arguments: dict[str, Any]) -> str:
        """크레이트 전체에 대한 테스트 분석"""
        crate_name = arguments.get("crate_name")

        if not crate_name:
            return self.format_error("Missing Parameter", "crate_name이 필요합니다")

        # 커버리지 분석으로 위임
        return self._analyze_coverage({"crate_name": crate_name})
