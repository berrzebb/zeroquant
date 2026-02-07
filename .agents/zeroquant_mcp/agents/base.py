"""Base agent class"""

import subprocess
from pathlib import Path
from typing import Any
import logging

logger = logging.getLogger(__name__)


class BaseAgent:
    """모든 Agent의 기본 클래스"""

    def __init__(self, project_root: Path):
        self.project_root = project_root
        self.logger = logger.getChild(self.__class__.__name__)
        self.progress_log = []  # 진행 상황 로그

    async def execute(self, arguments: dict[str, Any]) -> str:
        """에이전트 실행 (하위 클래스에서 구현)"""
        raise NotImplementedError

    def log_progress(self, message: str):
        """진행 상황 기록 (로그 파일 + 메모리)"""
        import datetime
        import sys
        timestamp = datetime.datetime.now().strftime("%H:%M:%S")
        log_entry = f"[{timestamp}] {message}"
        self.progress_log.append(log_entry)
        self.logger.info(message)

        # 즉시 flush하여 실시간 스트리밍 보장
        for handler in self.logger.handlers:
            handler.flush()
        sys.stderr.flush()
        sys.stdout.flush()

    def get_progress_section(self) -> str:
        """진행 로그를 마크다운 섹션으로 반환"""
        if not self.progress_log:
            return ""

        log_text = "\n".join(self.progress_log)
        return f"\n\n---\n\n## 📊 Progress Log\n\n```\n{log_text}\n```\n"

    def run_command(
        self,
        cmd: list[str],
        cwd: Path | None = None,
        timeout: int = 300,
        stream_output: bool = False
    ) -> tuple[int, str, str]:
        """명령어 실행

        Args:
            stream_output: True면 실시간 출력 (stderr로), False면 버퍼링 후 반환

        Returns:
            (return_code, stdout, stderr)
        """
        if cwd is None:
            cwd = self.project_root

        self.logger.info(f"Running: {' '.join(cmd)} in {cwd}")

        if not stream_output:
            # 기존 방식: 출력 캡처
            try:
                result = subprocess.run(
                    cmd,
                    cwd=cwd,
                    capture_output=True,
                    text=True,
                    timeout=timeout,
                    encoding='utf-8',
                    errors='replace'
                )
                return result.returncode, result.stdout, result.stderr

            except subprocess.TimeoutExpired:
                return -1, "", f"Command timed out after {timeout}s"
            except Exception as e:
                return -1, "", str(e)

        else:
            # 실시간 출력 모드 - Popen으로 라인 단위 읽기
            import sys
            import threading
            import queue

            try:
                process = subprocess.Popen(
                    cmd,
                    cwd=cwd,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                    encoding='utf-8',
                    errors='replace',
                    bufsize=1,
                    universal_newlines=True
                )

                output_lines = []

                def read_output(pipe, line_list):
                    """별도 스레드에서 출력 읽기"""
                    try:
                        for line in iter(pipe.readline, ''):
                            if line:
                                # 즉시 stderr로 출력
                                sys.stderr.write(line)
                                sys.stderr.flush()
                                line_list.append(line)
                    finally:
                        pipe.close()

                # 출력 읽기 스레드 시작
                reader_thread = threading.Thread(
                    target=read_output,
                    args=(process.stdout, output_lines),
                    daemon=True
                )
                reader_thread.start()

                # 프로세스 종료 대기
                returncode = process.wait(timeout=timeout)

                # 스레드 종료 대기 (최대 1초)
                reader_thread.join(timeout=1)

                output_text = ''.join(output_lines)
                return returncode, output_text, ""

            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
                output_text = ''.join(output_lines)
                return -1, output_text, f"Command timed out after {timeout}s"
            except Exception as e:
                if 'process' in locals():
                    process.kill()
                return -1, "", str(e)

    def format_success(self, title: str, content: str) -> str:
        """성공 메시지 포맷"""
        return f"✅ {title}\n\n{content}"

    def format_error(self, title: str, content: str) -> str:
        """에러 메시지 포맷"""
        return f"❌ {title}\n\n{content}"

    def format_warning(self, title: str, content: str) -> str:
        """경고 메시지 포맷"""
        return f"⚠️ {title}\n\n{content}"

    def format_section(self, title: str, content: str) -> str:
        """섹션 포맷"""
        return f"## {title}\n\n{content}\n"
