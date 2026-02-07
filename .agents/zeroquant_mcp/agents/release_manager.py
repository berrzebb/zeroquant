"""Release Manager Agent

Ship skill을 자동화한 릴리즈 매니저.
변경사항 분석 → 문서 업데이트 → 커밋/푸시를 자동으로 수행합니다.
"""

import re
from datetime import datetime
from typing import Any
from pathlib import Path
from .base import BaseAgent


class ReleaseManager(BaseAgent):
    """릴리즈 자동화 에이전트"""

    async def execute(self, arguments: dict[str, Any]) -> str:
        """릴리즈 워크플로우 실행"""
        self.log_progress("🚀 릴리즈 매니저 시작")
        
        mode = arguments.get("mode", "full")  # full, docs-only, preview
        custom_message = arguments.get("custom_message")
        skip_push = arguments.get("skip_push", False)

        results = []
        results.append("# 🚀 Release Manager Report\n\n")

        # 사전 조건 확인
        precheck = self._precheck()
        if not precheck["passed"]:
            return self.format_error(
                "Pre-check Failed",
                precheck["message"]
            )

        # 1. 변경사항 분석
        self.log_progress("🔍 [1/5] 변경사항 분석 중...")
        results.append("## 🔍 1. 변경사항 분석\n\n")
        changes = self._analyze_changes()
        results.append(self._format_changes(changes))

        if not changes["files"]:
            return self.format_warning(
                "No Changes",
                "스테이지된 파일이 없습니다."
            )

        # 2. 문서 업데이트
        self.log_progress("📝 [2/5] 문서 업데이트 중...")
        results.append("\n## 📝 2. 문서 업데이트\n\n")

        if mode == "preview":
            results.append("**Mode**: Preview (실제 변경 없음)\n\n")

        doc_updates = self._update_documents(changes, dry_run=(mode == "preview"))
        results.append(self._format_doc_updates(doc_updates))

        # 3. 커밋 메시지 생성
        self.log_progress("✍️ [3/5] 커밋 메시지 생성 중...")
        results.append("\n## ✍️ 3. 커밋 메시지\n\n")
        commit_msg = self._generate_commit_message(changes, custom_message)
        results.append(f"```\n{commit_msg}\n```\n\n")

        # 4. 커밋 실행
        if mode != "preview" and mode != "docs-only":
            self.log_progress("📦 [4/5] 커밋 실행 중...")
            results.append("## 📦 4. 커밋\n\n")
            commit_result = self._commit(commit_msg, doc_updates["updated_files"])
            results.append(commit_result)

            # 5. 푸시
            if not skip_push:
                self.log_progress("🚀 [5/5] 원격 저장소로 푸시 중...")
                results.append("\n## 🚀 5. 푸시\n\n")
                push_result = self._push()
                results.append(push_result)

        # 요약
        self.log_progress("✅ 릴리즈 매니저 완료")
        
        if mode == "preview":
            summary = self.format_success(
                "Preview Complete",
                "실제 변경은 이루어지지 않았습니다. mode='full'로 실행하세요."
            )
        elif mode == "docs-only":
            summary = self.format_success(
                "Documents Updated",
                "문서가 업데이트되었습니다. 수동으로 커밋하세요."
            )
        else:
            summary = self.format_success(
                "Release Complete",
                "변경사항이 성공적으로 배포되었습니다! ✨"
            )

        results.insert(0, summary + "\n\n")

        # Progress log 추가
        results.append(self.get_progress_section())

        return "\n".join(results)

    def _precheck(self) -> dict:
        """사전 조건 확인"""
        # Git 저장소 확인
        returncode, _, _ = self.run_command(["git", "rev-parse", "--git-dir"])
        if returncode != 0:
            return {"passed": False, "message": "Git 저장소가 아닙니다."}

        # 스테이지된 파일 확인
        returncode, stdout, _ = self.run_command(["git", "diff", "--cached", "--name-only"])
        if returncode == 0 and not stdout.strip():
            return {"passed": False, "message": "스테이지된 파일이 없습니다."}

        # 원격 저장소 확인
        returncode, _, _ = self.run_command(["git", "remote", "get-url", "origin"])
        if returncode != 0:
            return {"passed": False, "message": "원격 저장소가 설정되지 않았습니다."}

        return {"passed": True, "message": "OK"}

    def _analyze_changes(self) -> dict:
        """변경사항 분석"""
        self.logger.info("📊 변경사항 분석 중...")
        # 스테이지된 파일 목록
        _, stdout, _ = self.run_command(["git", "diff", "--cached", "--name-only"])
        files = [f.strip() for f in stdout.strip().split('\n') if f.strip()]

        # 통계
        _, stat_output, _ = self.run_command(["git", "diff", "--cached", "--stat"])

        # 변경 유형 분류
        change_types = self._classify_changes(files)

        # Diff 내용
        self.logger.info("📄 Diff 조회 중...")
        _, diff_output, _ = self.run_command(["git", "diff", "--cached"], stream_output=True)

        return {
            "files": files,
            "stat": stat_output,
            "types": change_types,
            "diff": diff_output
        }

    def _classify_changes(self, files: list[str]) -> dict:
        """파일 변경 유형 분류"""
        types = {
            "feat": [],
            "fix": [],
            "docs": [],
            "refactor": [],
            "test": [],
            "chore": []
        }

        for file in files:
            if file.startswith("docs/"):
                types["docs"].append(file)
            elif file.startswith("tests/") or "test" in file:
                types["test"].append(file)
            elif "fix" in file.lower():
                types["fix"].append(file)
            elif file.startswith("crates/"):
                # 소스 코드 변경 - 기본적으로 feat
                types["feat"].append(file)
            else:
                types["chore"].append(file)

        # 빈 리스트 제거
        return {k: v for k, v in types.items() if v}

    def _format_changes(self, changes: dict) -> str:
        """변경사항 포맷팅"""
        lines = []
        lines.append(f"**파일 수**: {len(changes['files'])}\n\n")

        if changes["types"]:
            lines.append("**변경 유형**:\n")
            for change_type, files in changes["types"].items():
                lines.append(f"- `{change_type}`: {len(files)}개 파일\n")
            lines.append("\n")

        lines.append("**통계**:\n```\n")
        lines.append(changes["stat"][:500])  # 최대 500자
        lines.append("\n```\n")

        return "".join(lines)

    def _update_documents(self, changes: dict, dry_run: bool = False) -> dict:
        """문서 자동 업데이트"""
        updated_files = []
        results = []

        # CHANGELOG.md 업데이트
        changelog_result = self._update_changelog(changes, dry_run)
        if changelog_result["updated"]:
            updated_files.append("CHANGELOG.md")
            results.append(changelog_result["message"])

        # docs/todo.md 업데이트
        todo_result = self._update_todo(changes, dry_run)
        if todo_result["updated"]:
            updated_files.append("docs/todo.md")
            results.append(todo_result["message"])

        return {
            "updated_files": updated_files,
            "results": results
        }

    def _update_changelog(self, changes: dict, dry_run: bool = False) -> dict:
        """CHANGELOG.md 업데이트"""
        changelog_path = self.project_root / "CHANGELOG.md"

        if not changelog_path.exists():
            return {"updated": False, "message": "⚠️ CHANGELOG.md가 없습니다."}

        try:
            content = changelog_path.read_text(encoding="utf-8")
        except Exception as e:
            return {"updated": False, "message": f"⚠️ CHANGELOG.md 읽기 실패: {e}"}

        # 새 엔트리 생성
        today = datetime.now().strftime("%Y-%m-%d")
        new_entry_lines = [f"\n## [Unreleased] - {today}\n\n"]

        # 변경 유형별로 항목 추가
        if "feat" in changes["types"]:
            new_entry_lines.append("### Added\n")
            for file in changes["types"]["feat"][:5]:  # 최대 5개
                new_entry_lines.append(f"- {file}\n")
            new_entry_lines.append("\n")

        if "fix" in changes["types"]:
            new_entry_lines.append("### Fixed\n")
            for file in changes["types"]["fix"][:5]:
                new_entry_lines.append(f"- {file}\n")
            new_entry_lines.append("\n")

        if "refactor" in changes["types"]:
            new_entry_lines.append("### Changed\n")
            for file in changes["types"]["refactor"][:5]:
                new_entry_lines.append(f"- {file}\n")
            new_entry_lines.append("\n")

        new_entry = "".join(new_entry_lines)

        # 첫 번째 ## 헤더 다음에 삽입
        match = re.search(r'(# Changelog\s*\n)', content)
        if match:
            insert_pos = match.end()
            new_content = content[:insert_pos] + new_entry + content[insert_pos:]
        else:
            # Changelog 헤더가 없으면 맨 위에 추가
            new_content = f"# Changelog\n{new_entry}\n{content}"

        if not dry_run:
            try:
                changelog_path.write_text(new_content, encoding="utf-8")
                return {"updated": True, "message": "✅ CHANGELOG.md 업데이트 완료"}
            except Exception as e:
                return {"updated": False, "message": f"❌ CHANGELOG.md 쓰기 실패: {e}"}
        else:
            return {"updated": True, "message": "📋 CHANGELOG.md 업데이트 예정"}

    def _update_todo(self, changes: dict, dry_run: bool = False) -> dict:
        """docs/todo.md 업데이트 (간단 버전)"""
        todo_path = self.project_root / "docs" / "todo.md"

        if not todo_path.exists():
            return {"updated": False, "message": "⚠️ docs/todo.md가 없습니다."}

        # TODO 파일은 복잡하므로 간단히 타임스탬프만 추가
        if not dry_run:
            try:
                content = todo_path.read_text(encoding="utf-8")
                today = datetime.now().strftime("%Y-%m-%d")
                # 맨 위에 최종 업데이트 시간 추가/업데이트
                if "최종 업데이트:" in content:
                    new_content = re.sub(
                        r'최종 업데이트: \d{4}-\d{2}-\d{2}',
                        f'최종 업데이트: {today}',
                        content
                    )
                else:
                    new_content = f"최종 업데이트: {today}\n\n{content}"

                todo_path.write_text(new_content, encoding="utf-8")
                return {"updated": True, "message": "✅ docs/todo.md 업데이트 완료"}
            except Exception as e:
                return {"updated": False, "message": f"❌ docs/todo.md 쓰기 실패: {e}"}
        else:
            return {"updated": True, "message": "📋 docs/todo.md 업데이트 예정"}

    def _format_doc_updates(self, doc_updates: dict) -> str:
        """문서 업데이트 결과 포맷팅"""
        if not doc_updates["results"]:
            return "문서 업데이트 없음\n"

        return "\n".join(doc_updates["results"]) + "\n"

    def _generate_commit_message(self, changes: dict, custom_message: str | None) -> str:
        """Conventional Commits 형식으로 커밋 메시지 생성"""
        if custom_message:
            return f"{custom_message}\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"

        # 주요 변경 유형 결정
        if changes["types"].get("feat"):
            commit_type = "feat"
            scope = self._extract_scope(changes["types"]["feat"])
        elif changes["types"].get("fix"):
            commit_type = "fix"
            scope = self._extract_scope(changes["types"]["fix"])
        elif changes["types"].get("docs"):
            commit_type = "docs"
            scope = ""
        else:
            commit_type = "chore"
            scope = ""

        # Subject 생성 (첫 번째 파일 기반)
        first_file = changes["files"][0] if changes["files"] else "update"
        subject = f"Update {Path(first_file).stem}"

        # Body 생성
        body_lines = []
        for change_type, files in changes["types"].items():
            body_lines.append(f"- {change_type}: {len(files)} files")

        body = "\n".join(body_lines)

        # 조합
        if scope:
            message = f"{commit_type}({scope}): {subject}\n\n{body}\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
        else:
            message = f"{commit_type}: {subject}\n\n{body}\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"

        return message

    def _extract_scope(self, files: list[str]) -> str:
        """파일 경로에서 scope 추출"""
        # crates/trader-xxx/src/... -> trader-xxx
        for file in files:
            if file.startswith("crates/"):
                parts = file.split("/")
                if len(parts) >= 2:
                    crate_name = parts[1].replace("trader-", "")
                    return crate_name
        return ""

    def _commit(self, commit_message: str, doc_files: list[str]) -> str:
        """커밋 실행"""
        # 업데이트된 문서 파일 추가
        if doc_files:
            for doc_file in doc_files:
                returncode, _, stderr = self.run_command(["git", "add", doc_file])
                if returncode != 0:
                    return self.format_error(
                        "Git Add Failed",
                        f"파일 추가 실패: {doc_file}\n{stderr}"
                    )

        # 커밋 실행
        self.logger.info("💾 Git commit 실행 중...")
        returncode, stdout, stderr = self.run_command([
            "git", "commit", "-m", commit_message
        ], stream_output=True)

        if returncode == 0:
            # 커밋 해시 추출
            match = re.search(r'\[.*?([a-f0-9]+)\]', stdout)
            commit_hash = match.group(1) if match else "unknown"
            return f"✅ 커밋 완료 (hash: `{commit_hash}`)\n"
        else:
            return self.format_error(
                "Commit Failed",
                f"커밋 실패:\n{stderr}"
            )

    def _push(self) -> str:
        """원격 저장소로 푸시"""
        # 현재 브랜치 확인
        _, branch_output, _ = self.run_command([
            "git", "rev-parse", "--abbrev-ref", "HEAD"
        ])
        current_branch = branch_output.strip()

        # 푸시 실행
        self.logger.info(f"⬆️ Git push 실행 중 (브랜치: {current_branch})...")
        returncode, stdout, stderr = self.run_command([
            "git", "push", "origin", current_branch
        ], stream_output=True)

        if returncode == 0:
            return f"✅ `origin/{current_branch}`로 푸시 완료\n"
        else:
            return self.format_error(
                "Push Failed",
                f"푸시 실패:\n{stderr}\n\n"
                "원격 저장소가 최신이 아닐 수 있습니다. "
                "`git pull --rebase` 후 다시 시도하세요."
            )
