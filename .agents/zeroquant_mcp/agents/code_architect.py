"""Code Architect Agent"""

from typing import Any
from .base import BaseAgent


class CodeArchitect(BaseAgent):
    """아키텍처 설계 에이전트"""

    async def execute(self, arguments: dict[str, Any]) -> str:
        """아키텍처 설계 실행"""
        self.log_progress("🏗️ 아키텍처 설계 시작")

        feature_name = arguments.get("feature_name")
        requirements = arguments.get("requirements")
        constraints = arguments.get("constraints", "")
        analyze_existing = arguments.get("analyze_existing", True)

        results = []
        results.append(f"# {feature_name} 아키텍처 설계\n\n")

        # 1. 요구사항 분석
        self.log_progress("📋 [1/5] 요구사항 분석 중")
        results.append(self.format_section(
            "📋 요구사항",
            f"**목표**: {feature_name}\n\n"
            f"{requirements}\n\n"
            + (f"**제약사항**: {constraints}\n" if constraints else "")
        ))

        # 2. 기존 코드 분석 (선택)
        if analyze_existing:
            self.log_progress("🔍 [2/5] 기존 코드 패턴 분석 중")
            analysis = self._analyze_existing_code(feature_name)
            results.append(self.format_section(
                "🔍 기존 코드 분석",
                analysis
            ))

        # 3. 설계 원칙
        self.log_progress("🎯 [3/5] 설계 원칙 정의 중")
        results.append(self.format_section(
            "🎯 설계 원칙",
            "1. **거래소 중립성**: Exchange trait 사용\n"
            "2. **도메인 중심**: Core → Strategy → Exchange 레이어\n"
            "3. **타입 안전성**: Decimal, Result, unwrap 금지\n"
            "4. **에러 처리**: 명확한 Error enum\n"
        ))

        # 4. 제안 구조
        self.log_progress("📁 [4/5] 파일 구조 생성 중")
        results.append(self.format_section(
            "📁 제안 파일 구조",
            "```\n"
            f"crates/trader-xxx/src/\n"
            f"├── {feature_name.lower()}/\n"
            f"│   ├── mod.rs\n"
            f"│   ├── core.rs\n"
            f"│   ├── types.rs\n"
            f"│   └── error.rs\n"
            "```\n"
        ))

        # 5. 구현 계획
        self.log_progress("📝 [5/5] 구현 계획 수립 중")
        results.append(self.format_section(
            "📝 구현 계획",
            "### Phase 1: 기본 구조 (예상: 4시간)\n"
            "- [ ] 타입 정의\n"
            "- [ ] 기본 로직 구현\n"
            "- [ ] 단위 테스트\n\n"
            "### Phase 2: 통합 (예상: 6시간)\n"
            "- [ ] 기존 시스템 통합\n"
            "- [ ] 통합 테스트\n"
            "- [ ] 문서화\n"
        ))

        # 6. 트레이드오프
        results.append(self.format_section(
            "⚖️ 트레이드오프 분석",
            "### Option 1: [방식 A]\n"
            "**장점**: ...\n"
            "**단점**: ...\n\n"
            "### Option 2: [방식 B] ⭐ 추천\n"
            "**장점**: ...\n"
            "**이유**: ...\n"
        ))

        self.log_progress("✅ 아키텍처 설계 완료")
        results.append(self.get_progress_section())

        return "\n".join(results)

    def _analyze_existing_code(self, feature_name: str) -> str:
        """기존 코드 패턴 분석"""
        # 관련 파일 찾기 (간단한 grep)
        _, stdout, _ = self.run_command([
            "rg",
            "-l",
            "--type", "rust",
            feature_name.lower()
        ], stream_output=True)

        if stdout.strip():
            files = stdout.strip().split('\n')[:5]  # 최대 5개
            return f"**관련 파일** ({len(files)}개 발견):\n" + "\n".join(
                f"- `{f}`" for f in files
            )
        else:
            return "관련 파일을 찾지 못했습니다. 신규 기능으로 보입니다."
