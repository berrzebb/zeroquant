#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
ZeroQuant MCP Agents 검증 스크립트

각 에이전트의 기본 동작을 빠르게 테스트합니다.
"""

import asyncio
import sys
import os
from pathlib import Path
from typing import Any

# Windows 인코딩 문제 해결
if sys.platform == "win32":
    import codecs
    sys.stdout = codecs.getwriter("utf-8")(sys.stdout.buffer, errors="replace")
    sys.stderr = codecs.getwriter("utf-8")(sys.stderr.buffer, errors="replace")

# 프로젝트 루트
PROJECT_ROOT = Path(__file__).parent.parent.parent

# 에이전트 임포트
sys.path.insert(0, str(PROJECT_ROOT / ".agents" / "zeroquant_mcp"))

from agents.build_validator import BuildValidator
from agents.code_reviewer import CodeReviewer
from agents.code_architect import CodeArchitect
from agents.code_simplifier import CodeSimplifier
from agents.ux_reviewer import UXReviewer
from agents.release_manager import ReleaseManager
from agents.security_reviewer import SecurityReviewer
from agents.test_writer import TestWriter


class AgentTester:
    """에이전트 테스터"""

    def __init__(self, project_root: Path):
        self.project_root = project_root
        self.results = []

    async def test_agent(
        self,
        agent_class: Any,
        agent_name: str,
        test_args: dict[str, Any],
        timeout: int = 60
    ) -> bool:
        """개별 에이전트 테스트"""
        print(f"\n{'='*60}")
        print(f"Testing: {agent_name}")
        print(f"Args: {test_args}")
        print(f"{'='*60}")

        try:
            agent = agent_class(self.project_root)

            # Timeout 적용
            result = await asyncio.wait_for(
                agent.execute(test_args),
                timeout=timeout
            )

            # 결과 검증
            if result and len(result) > 0:
                print(f"✅ {agent_name}: PASSED")
                print(f"   Output length: {len(result)} chars")
                print(f"   Preview: {result[:200]}...")
                self.results.append((agent_name, True, None))
                return True
            else:
                print(f"❌ {agent_name}: FAILED (Empty result)")
                self.results.append((agent_name, False, "Empty result"))
                return False

        except asyncio.TimeoutError:
            print(f"⚠️ {agent_name}: TIMEOUT ({timeout}s)")
            self.results.append((agent_name, False, f"Timeout {timeout}s"))
            return False
        except Exception as e:
            print(f"❌ {agent_name}: ERROR")
            print(f"   {type(e).__name__}: {e}")
            self.results.append((agent_name, False, str(e)))
            return False

    async def run_all_tests(self):
        """모든 에이전트 테스트 실행"""
        print(f"\n🚀 ZeroQuant MCP Agents 검증 시작")
        print(f"Project Root: {self.project_root}")
        print(f"{'='*60}\n")

        # 1. BuildValidator (빠름, 2분)
        await self.test_agent(
            BuildValidator,
            "build_validator",
            {
                "target": "workspace",
                "skip_tests": True,  # 빠른 테스트
                "skip_clippy": False
            },
            timeout=120
        )

        # 2. SecurityReviewer (최적화 후 빠름)
        await self.test_agent(
            SecurityReviewer,
            "security_reviewer",
            {
                "target": "staged",  # staged만 (빠름)
                "severity": "critical"  # critical만
            },
            timeout=30
        )

        # 3. CodeReviewer (중간)
        await self.test_agent(
            CodeReviewer,
            "code_reviewer",
            {
                "target": "staged"
            },
            timeout=60
        )

        # 4. TestWriter (빠름)
        await self.test_agent(
            TestWriter,
            "test_writer",
            {
                "target": "coverage",  # 분석만
                "mode": "analyze"
            },
            timeout=30
        )

        # 5. UXReviewer (중간)
        await self.test_agent(
            UXReviewer,
            "ux_reviewer",
            {
                "target": "api"
            },
            timeout=60
        )

        # 6. CodeSimplifier (느림, 스킵)
        print(f"\n{'='*60}")
        print(f"Skipping: code_simplifier (너무 느림, 수동 테스트 권장)")
        print(f"{'='*60}")
        self.results.append(("code_simplifier", None, "Skipped"))

        # 7. CodeArchitect (느림, 스킵)
        print(f"\n{'='*60}")
        print(f"Skipping: code_architect (수동 테스트 권장)")
        print(f"{'='*60}")
        self.results.append(("code_architect", None, "Skipped"))

        # 8. ReleaseManager (위험, 스킵)
        print(f"\n{'='*60}")
        print(f"Skipping: release_manager (실제 커밋 생성, 수동 테스트만)")
        print(f"{'='*60}")
        self.results.append(("release_manager", None, "Skipped (dangerous)"))

    def print_summary(self):
        """결과 요약"""
        print(f"\n\n{'='*60}")
        print(f"📊 테스트 결과 요약")
        print(f"{'='*60}\n")

        passed = sum(1 for _, result, _ in self.results if result is True)
        failed = sum(1 for _, result, _ in self.results if result is False)
        skipped = sum(1 for _, result, _ in self.results if result is None)

        print(f"✅ Passed:  {passed}")
        print(f"❌ Failed:  {failed}")
        print(f"⏭️ Skipped: {skipped}")
        print(f"📈 Total:   {len(self.results)}\n")

        if failed > 0:
            print("\n🔴 실패한 에이전트:\n")
            for name, result, error in self.results:
                if result is False:
                    print(f"  - {name}: {error}")

        print(f"\n{'='*60}\n")

        # Exit code
        return 0 if failed == 0 else 1


async def main():
    """메인 함수"""
    tester = AgentTester(PROJECT_ROOT)

    try:
        await tester.run_all_tests()
    except KeyboardInterrupt:
        print("\n\n⚠️ 사용자에 의해 중단됨")

    exit_code = tester.print_summary()
    sys.exit(exit_code)


if __name__ == "__main__":
    asyncio.run(main())
