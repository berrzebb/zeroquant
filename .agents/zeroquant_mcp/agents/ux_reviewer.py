"""UX Reviewer Agent"""

import re
from typing import Any
from pathlib import Path
from .base import BaseAgent


class UXReviewer(BaseAgent):
    """UX 평가 에이전트"""

    async def execute(self, arguments: dict[str, Any]) -> str:
        """UX 평가 실행"""
        self.log_progress("🎨 UX 평가 시작")

        target = arguments.get("target", "all")

        results = []
        results.append("# UX Review Report\n\n")

        scores = {}

        # API 평가
        if target in ["all", "api"]:
            self.log_progress("🔍 [1/3] API 설계 평가 중")
            api_endpoints = arguments.get("api_endpoints", [])
            scores["API 설계"] = self._evaluate_api(api_endpoints)

        # UI 평가
        if target in ["all", "ui"]:
            self.log_progress("🔍 [2/3] UI/UX 평가 중")
            ui_components = arguments.get("ui_components", [])
            scores["UI/UX"] = self._evaluate_ui(ui_components)

        # CLI 평가
        if target in ["all", "cli"]:
            self.log_progress("🔍 [3/3] CLI 사용성 평가 중")
            cli_commands = arguments.get("cli_commands", [])
            scores["CLI 사용성"] = self._evaluate_cli(cli_commands)

        self.log_progress("✅ UX 평가 완료")

        # 점수 계산
        if scores:
            total_score = sum(s["score"] for s in scores.values()) / len(scores)
            results.append(f"**전체 점수**: {total_score:.0f}/100\n\n")
            results.append("---\n\n")

            # 각 카테고리 결과
            for category, result in scores.items():
                score = result["score"]
                emoji = "✅" if score >= 85 else "⚠️" if score >= 70 else "❌"

                results.append(f"## {emoji} {category}\n\n")
                results.append(f"**점수**: {score}/100\n\n")

                if result.get("issues"):
                    results.append("**이슈**:\n")
                    for issue in result["issues"]:
                        results.append(f"- {issue}\n")
                    results.append("\n")

                if result.get("strengths"):
                    results.append("**강점**:\n")
                    for strength in result["strengths"]:
                        results.append(f"- ✨ {strength}\n")
                    results.append("\n")

        else:
            results.append(self.format_warning(
                "No Target Specified",
                "평가할 대상을 지정하세요."
            ))

        results.append(self.get_progress_section())

        return "\n".join(results)

    def _evaluate_api(self, endpoints: list[str]) -> dict:
        """API 설계 평가"""
        issues = []
        strengths = []
        score = 100

        # API 라우트 파일 분석
        api_dir = self.project_root / "crates" / "trader-api" / "src" / "routes"

        if api_dir.exists():
            # RESTful 원칙 체크
            route_files = list(api_dir.glob("*.rs"))

            for file in route_files:
                try:
                    content = file.read_text(encoding='utf-8')

                    # 일관된 응답 구조 체크
                    if 'Json(' in content and 'ApiResponse' not in content:
                        issues.append(f"{file.name}: ApiResponse 래퍼 미사용")
                        score -= 5

                    # 에러 처리 체크
                    if '.unwrap()' in content:
                        issues.append(f"{file.name}: unwrap() 사용 (에러 처리 누락)")
                        score -= 10

                except Exception:
                    pass

            strengths.append(f"{len(route_files)}개 라우트 파일 구조화")

        return {
            "score": max(0, score),
            "issues": issues,
            "strengths": strengths
        }

    def _evaluate_ui(self, components: list[str]) -> dict:
        """UI/UX 평가"""
        issues = []
        strengths = []
        score = 100

        # 프론트엔드 디렉토리
        frontend_dir = self.project_root / "frontend" / "src"

        if frontend_dir.exists():
            # 컴포넌트 파일 찾기
            component_files = list(frontend_dir.rglob("*.tsx")) + list(frontend_dir.rglob("*.jsx"))

            for file in component_files[:10]:  # 최대 10개
                try:
                    content = file.read_text(encoding='utf-8')

                    # 로딩 상태 체크
                    if 'useState' in content and 'Loading' not in content and 'loading' not in content:
                        issues.append(f"{file.name}: 로딩 상태 처리 없음")
                        score -= 5

                    # 에러 상태 체크
                    if 'fetch' in content and 'error' not in content.lower():
                        issues.append(f"{file.name}: 에러 처리 없음")
                        score -= 5

                    # 접근성 체크
                    if '<button' in content and 'aria-label' not in content:
                        issues.append(f"{file.name}: aria-label 없음")
                        score -= 3

                except Exception:
                    pass

            strengths.append(f"SolidJS + TypeScript 사용")

        return {
            "score": max(0, score),
            "issues": issues,
            "strengths": strengths
        }

    def _evaluate_cli(self, commands: list[str]) -> dict:
        """CLI 사용성 평가"""
        issues = []
        strengths = []
        score = 100

        # CLI 디렉토리
        cli_dir = self.project_root / "crates" / "trader-cli" / "src"

        if cli_dir.exists():
            # 명령어 파일 확인
            command_dir = cli_dir / "commands"
            if command_dir.exists():
                command_files = list(command_dir.glob("*.rs"))

                for file in command_files:
                    try:
                        content = file.read_text(encoding='utf-8')

                        # 도움말 체크
                        if 'clap' in content and 'help' not in content.lower():
                            issues.append(f"{file.name}: 도움말 없음")
                            score -= 5

                        # Examples 체크
                        if 'Command' in content and 'example' not in content.lower():
                            issues.append(f"{file.name}: 사용 예시 없음")
                            score -= 3

                    except Exception:
                        pass

                strengths.append(f"{len(command_files)}개 CLI 명령어 구현")

        return {
            "score": max(0, score),
            "issues": issues,
            "strengths": strengths
        }
