"""Build Validator Agent

빌드 및 테스트 검증을 실제로 수행합니다.
"""

import re
from typing import Any
from .base import BaseAgent


class BuildValidator(BaseAgent):
    """빌드 검증 에이전트"""

    async def execute(self, arguments: dict[str, Any]) -> str:
        """빌드 검증 실행"""
        self.log_progress("🚀 빌드 검증 시작")

        target = arguments.get("target", "workspace")
        package_name = arguments.get("package_name")
        skip_tests = arguments.get("skip_tests", False)
        skip_clippy = arguments.get("skip_clippy", False)
        verbose = arguments.get("verbose", False)  # 상세 출력 모드

        results = []
        all_passed = True

        # 1. Build
        results.append("# Build Validation Report\n")
        results.append(f"**Target**: {target}\n")
        if package_name:
            results.append(f"**Package**: {package_name}\n")
        results.append("\n---\n\n")

        # 1. Compilation
        self.log_progress("🔨 [1/4] Compilation 시작")
        build_result = self._run_build(target, package_name, verbose)
        results.append(build_result["report"])
        all_passed = all_passed and build_result["passed"]
        self.log_progress(f"✓ [1/4] Compilation {'성공' if build_result['passed'] else '실패'}")

        # 2. Clippy
        if not skip_clippy:
            self.log_progress("📎 [2/4] Clippy 시작")
            clippy_result = self._run_clippy(target, package_name, verbose)
            results.append(clippy_result["report"])
            all_passed = all_passed and clippy_result["passed"]
            self.log_progress(f"✓ [2/4] Clippy {'성공' if clippy_result['passed'] else '실패'}")

        # 3. Tests
        if not skip_tests:
            self.log_progress("🧪 [3/4] Tests 시작 (최대 10분 소요)")
            test_result = self._run_tests(target, package_name, verbose)
            results.append(test_result["report"])
            all_passed = all_passed and test_result["passed"]
            self.log_progress(f"✓ [3/4] Tests {'성공' if test_result['passed'] else '실패'}")

        # 4. Format check
        self.log_progress("🎨 [4/4] Format check 시작")
        fmt_result = self._run_fmt_check(verbose)
        results.append(fmt_result["report"])
        all_passed = all_passed and fmt_result["passed"]
        self.log_progress(f"✓ [4/4] Format check {'성공' if fmt_result['passed'] else '실패'}")

        # Summary
        if all_passed:
            summary = self.format_success(
                "Build Validation Passed",
                "모든 검증을 통과했습니다."
            )
        else:
            summary = self.format_error(
                "Build Validation Failed",
                "일부 검증이 실패했습니다. 위 리포트를 확인하세요."
            )

        results.insert(0, summary + "\n\n")

        # Progress log 추가
        results.append(self.get_progress_section())

        self.log_progress("✅ 빌드 검증 완료")

        return "\n".join(results)

    def _run_build(self, target: str, package_name: str | None, verbose: bool = False) -> dict:
        """Cargo build 실행"""

        cmd = ["cargo", "build"]

        if target == "workspace":
            cmd.append("--workspace")
        elif target == "package" and package_name:
            cmd.extend(["-p", package_name])

        returncode, stdout, stderr = self.run_command(cmd, stream_output=True)

        # 결과 분석
        if returncode == 0:
            # 크레이트 수 카운트
            crate_count = stdout.count("Compiling") + stdout.count("Finished")
            report = self.format_section(
                "✅ 1. Compilation",
                f"**Status**: Success\n"
                f"**Crates**: ~{crate_count} compiled\n"
            )
            passed = True
        else:
            # 에러 추출 (멀티라인 지원)
            error_blocks = self._extract_rust_errors(stderr)

            content = f"**Status**: Failed\n**Errors**: {len(error_blocks)}\n\n"

            if error_blocks:
                content += "**Top Errors**:\n```\n"
                content += "\n---\n".join(error_blocks[:3])  # 최대 3개 에러 블록
                content += "\n```\n"

            if verbose:
                content += "\n<details><summary>전체 stderr 출력</summary>\n\n```\n"
                content += stderr[:2000]  # 최대 2000자
                content += "\n```\n</details>\n"

            report = self.format_section("❌ 1. Compilation", content)
            passed = False

        return {"passed": passed, "report": report}

    def _run_clippy(self, target: str, package_name: str | None, verbose: bool = False) -> dict:
        """Cargo clippy 실행"""

        cmd = ["cargo", "clippy"]

        if target == "workspace":
            cmd.append("--workspace")
        elif target == "package" and package_name:
            cmd.extend(["-p", package_name])

        cmd.extend(["--", "-D", "warnings"])

        returncode, stdout, stderr = self.run_command(cmd, stream_output=True)

        # 경고 추출
        warning_blocks = self._extract_rust_warnings(stdout + stderr)

        if returncode == 0 and len(warning_blocks) == 0:
            report = self.format_section(
                "✅ 2. Clippy",
                f"**Status**: Pass\n"
                f"**Warnings**: 0\n"
            )
            passed = True
        else:
            content = f"**Status**: Warnings Found\n**Count**: {len(warning_blocks)}\n\n"

            if warning_blocks:
                content += "**Top Warnings**:\n```\n"
                content += "\n---\n".join(warning_blocks[:5])
                content += "\n```\n"

            if verbose:
                content += "\n<details><summary>전체 출력</summary>\n\n```\n"
                content += (stdout + stderr)[:3000]
                content += "\n```\n</details>\n"

            report = self.format_section("⚠️ 2. Clippy", content)
            passed = False

        return {"passed": passed, "report": report}

    def _run_tests(self, target: str, package_name: str | None, verbose: bool = False) -> dict:
        """Cargo test 실행"""

        cmd = ["cargo", "test"]

        if target == "workspace":
            cmd.append("--workspace")
        elif target == "package" and package_name:
            cmd.extend(["-p", package_name])

        returncode, stdout, stderr = self.run_command(cmd, timeout=600, stream_output=True)

        # 테스트 결과 파싱
        test_pattern = re.compile(r"test result: (\w+)\. (\d+) passed; (\d+) failed")
        match = test_pattern.search(stdout)

        if match:
            result = match.group(1)
            passed_count = match.group(2)
            failed_count = match.group(3)

            if result == "ok" or result == "PASSED":
                report = self.format_section(
                    "✅ 3. Tests",
                    f"**Status**: All Passed\n"
                    f"**Passed**: {passed_count}\n"
                    f"**Failed**: {failed_count}\n"
                )
                passed = True
            else:
                # 실패한 테스트 추출
                failed_tests = self._extract_failed_tests(stdout)
                content = (
                    f"**Status**: Some Failed\n"
                    f"**Passed**: {passed_count}\n"
                    f"**Failed**: {failed_count}\n\n"
                )

                if failed_tests:
                    content += "**Failed Tests**:\n```\n"
                    content += "\n".join(failed_tests[:5])
                    content += "\n```\n"

                if verbose:
                    content += "\n<details><summary>전체 테스트 출력</summary>\n\n```\n"
                    content += stdout[-3000:]  # 마지막 3000자
                    content += "\n```\n</details>\n"

                report = self.format_section("❌ 3. Tests", content)
                passed = False
        else:
            # 파싱 실패 시 실제 출력 일부 표시
            content = (
                f"**Status**: Could not parse test results\n"
                f"**Return Code**: {returncode}\n\n"
            )

            # 컴파일 에러가 있었는지 확인
            if "error" in stderr.lower() or "error" in stdout.lower():
                content += "⚠️ **컴파일 에러가 있을 수 있습니다.**\n\n"

            # stdout의 마지막 부분 표시 (실제 에러 메시지)
            content += "**Output Tail** (마지막 1000자):\n```\n"
            content += (stdout + stderr)[-1000:]
            content += "\n```\n"

            if verbose:
                content += "\n<details><summary>전체 출력</summary>\n\n"
                content += f"**stdout**:\n```\n{stdout[:2000]}\n```\n\n"
                content += f"**stderr**:\n```\n{stderr[:2000]}\n```\n"
                content += "</details>\n"

            report = self.format_section("⚠️ 3. Tests", content)
            passed = False

        return {"passed": passed, "report": report}

    def _run_fmt_check(self, verbose: bool = False) -> dict:
        """Cargo fmt check 실행"""
        
        cmd = ["cargo", "fmt", "--all", "--", "--check"]

        returncode, stdout, stderr = self.run_command(cmd)

        if returncode == 0:
            report = self.format_section(
                "✅ 4. Format Check",
                "**Status**: All files formatted correctly\n"
            )
            passed = True
        else:
            # 포맷 필요한 파일 추출
            unformatted = [
                line for line in stderr.split('\n')
                if line.startswith("Diff in")
            ]

            content = (
                f"**Status**: Some files need formatting\n"
                f"**Count**: {len(unformatted)}\n\n"
                f"**Fix**: `cargo fmt --all`\n"
            )

            if unformatted:
                content += "\n**Files**:\n```\n"
                content += "\n".join(unformatted[:10])
                content += "\n```\n"

            if verbose and stderr:
                content += "\n<details><summary>전체 diff</summary>\n\n```\n"
                content += stderr[:2000]
                content += "\n```\n</details>\n"

            report = self.format_section("⚠️ 4. Format Check", content)
            passed = False

        return {"passed": passed, "report": report}

    def _extract_rust_errors(self, text: str) -> list[str]:
        """Rust 컴파일 에러 블록 추출 (멀티라인 지원)"""
        error_blocks = []
        lines = text.split('\n')
        current_block = []
        in_error = False

        for line in lines:
            # 에러 시작 감지
            if 'error[E' in line or 'error:' in line:
                if current_block:
                    error_blocks.append('\n'.join(current_block))
                current_block = [line.strip()]
                in_error = True
            # 에러 블록 계속
            elif in_error and (line.startswith('  ') or line.startswith('\t') or '|' in line):
                current_block.append(line.rstrip())
            # 에러 블록 종료
            elif in_error and line.strip() == '':
                if current_block:
                    error_blocks.append('\n'.join(current_block))
                    current_block = []
                in_error = False

        # 마지막 블록 처리
        if current_block:
            error_blocks.append('\n'.join(current_block))

        return error_blocks

    def _extract_rust_warnings(self, text: str) -> list[str]:
        """Rust 경고 블록 추출 (멀티라인 지원)"""
        warning_blocks = []
        lines = text.split('\n')
        current_block = []
        in_warning = False

        for line in lines:
            # 경고 시작 감지
            if 'warning:' in line:
                if current_block:
                    warning_blocks.append('\n'.join(current_block))
                current_block = [line.strip()]
                in_warning = True
            # 경고 블록 계속
            elif in_warning and (line.startswith('  ') or line.startswith('\t') or '|' in line):
                current_block.append(line.rstrip())
            # 경고 블록 종료
            elif in_warning and line.strip() == '':
                if current_block:
                    warning_blocks.append('\n'.join(current_block))
                    current_block = []
                in_warning = False

        # 마지막 블록 처리
        if current_block:
            warning_blocks.append('\n'.join(current_block))

        return warning_blocks

    def _extract_failed_tests(self, text: str) -> list[str]:
        """실패한 테스트 추출"""
        failed = []
        for line in text.split('\n'):
            if line.strip().startswith('test ') and '... FAILED' in line:
                failed.append(line.strip())
        return failed
