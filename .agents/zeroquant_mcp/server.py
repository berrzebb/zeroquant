#!/usr/bin/env python3
"""
ZeroQuant Agents MCP Server - Main Entry Point
"""

# CRITICAL: sys.path 수정을 임포트 전에 수행
import sys
from pathlib import Path

# 패키지 경로를 Python path에 추가
sys.path.insert(0, str(Path(__file__).parent.parent))

import asyncio
import logging
from typing import Any

__version__ = "1.0.0"

from mcp.server import Server
from mcp.types import Tool, TextContent, ImageContent, EmbeddedResource
from mcp.server.stdio import stdio_server

from zeroquant_mcp.agents import (
    BuildValidator,
    CodeReviewer,
    CodeArchitect,
    CodeSimplifier,
    UXReviewer,
    ReleaseManager,
    SecurityReviewer,
    TestWriter
)

# 로깅 설정 (진행 상황 출력 강화)
log_dir = Path.home() / ".claude" / "logs"
log_dir.mkdir(parents=True, exist_ok=True)

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    handlers=[
        logging.StreamHandler(),  # stderr로 출력 (Claude Code가 캡처)
        logging.FileHandler(
            log_dir / "mcp-zeroquant-agents.log",
            encoding="utf-8"
        )
    ]
)
logger = logging.getLogger(__name__)

# 서버 인스턴스
server = Server("zeroquant-agents")

# 프로젝트 루트
PROJECT_ROOT = Path(__file__).parent.parent.parent.absolute()

# Agent 인스턴스
agents = {
    "build_validator": BuildValidator(PROJECT_ROOT),
    "code_reviewer": CodeReviewer(PROJECT_ROOT),
    "code_architect": CodeArchitect(PROJECT_ROOT),
    "code_simplifier": CodeSimplifier(PROJECT_ROOT),
    "ux_reviewer": UXReviewer(PROJECT_ROOT),
    "release_manager": ReleaseManager(PROJECT_ROOT),
    "security_reviewer": SecurityReviewer(PROJECT_ROOT),
    "test_writer": TestWriter(PROJECT_ROOT),
}


@server.list_tools()
async def list_tools() -> list[Tool]:
    """사용 가능한 도구 목록"""
    return [
        Tool(
            name="build_validator",
            description=(
                "빌드 및 테스트 검증 에이전트.\n"
                "- cargo build, clippy, test, fmt 실행\n"
                "- 컴파일 에러, 경고, 테스트 실패 수집\n"
                "- 구조화된 리포트 생성"
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "빌드 대상",
                        "enum": ["workspace", "package"],
                        "default": "workspace"
                    },
                    "package_name": {
                        "type": "string",
                        "description": "특정 패키지 이름 (target=package일 때)"
                    },
                    "skip_tests": {
                        "type": "boolean",
                        "description": "테스트 생략 여부",
                        "default": False
                    },
                    "skip_clippy": {
                        "type": "boolean",
                        "description": "Clippy 생략 여부",
                        "default": False
                    }
                },
                "required": []
            }
        ),
        Tool(
            name="code_reviewer",
            description=(
                "코드 품질 리뷰 에이전트.\n"
                "- 코딩 스타일 (Decimal, unwrap, 거래소 중립성)\n"
                "- 보안 (SQL Injection, API 키)\n"
                "- 성능 (clone, 비동기)\n"
                "- 테스트 커버리지\n"
                "- 문서화 (Rustdoc)\n"
                "- Git 히스토리"
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "리뷰 대상",
                        "enum": ["staged", "commit", "pr", "files"],
                        "default": "staged"
                    },
                    "commit_hash": {
                        "type": "string",
                        "description": "커밋 해시 (target=commit일 때)"
                    },
                    "pr_number": {
                        "type": "integer",
                        "description": "PR 번호 (target=pr일 때)"
                    },
                    "files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "파일 경로 목록 (target=files일 때)"
                    }
                },
                "required": []
            }
        ),
        Tool(
            name="code_architect",
            description=(
                "아키텍처 설계 에이전트.\n"
                "- 요구사항 분석\n"
                "- 기존 코드 분석 (패턴, 의존성)\n"
                "- 컴포넌트 다이어그램\n"
                "- 파일 구조 제안\n"
                "- 구현 계획 (Phase별)\n"
                "- 트레이드오프 분석"
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "feature_name": {
                        "type": "string",
                        "description": "기능 이름"
                    },
                    "requirements": {
                        "type": "string",
                        "description": "요구사항 상세 설명"
                    },
                    "constraints": {
                        "type": "string",
                        "description": "제약사항 (성능, 동시성 등)",
                        "default": ""
                    },
                    "analyze_existing": {
                        "type": "boolean",
                        "description": "기존 코드 분석 여부",
                        "default": True
                    }
                },
                "required": ["feature_name", "requirements"]
            }
        ),
        Tool(
            name="code_simplifier",
            description=(
                "코드 단순화 분석 에이전트.\n"
                "- 중복 코드 식별 (패턴 매칭)\n"
                "- 복잡도 분석 (CC, 줄 수)\n"
                "- 레거시 코드 (주석, dead_code, TODO)\n"
                "- 타입 안전성 (String → Enum)\n"
                "- 성능 최적화 기회 (clone, String)"
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "string",
                        "description": "분석 범위",
                        "enum": ["workspace", "crate", "module", "file"],
                        "default": "workspace"
                    },
                    "crate_name": {
                        "type": "string",
                        "description": "크레이트 이름 (scope=crate일 때)"
                    },
                    "module_path": {
                        "type": "string",
                        "description": "모듈 경로 (scope=module일 때)"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "파일 경로 (scope=file일 때)"
                    },
                    "priority": {
                        "type": "string",
                        "description": "우선순위 필터",
                        "enum": ["all", "high", "medium", "low"],
                        "default": "all"
                    }
                },
                "required": []
            }
        ),
        Tool(
            name="ux_reviewer",
            description=(
                "UX 평가 에이전트.\n"
                "- API 설계 (RESTful, 응답 구조)\n"
                "- 에러 메시지 (한글, 해결 방법)\n"
                "- UI/UX (로딩, 에러, 빈 상태)\n"
                "- 접근성 (키보드, aria)\n"
                "- 성능 (로딩 시간)\n"
                "- CLI 사용성"
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "평가 대상",
                        "enum": ["api", "ui", "cli", "all"],
                        "default": "all"
                    },
                    "api_endpoints": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "API 엔드포인트 목록 (target=api일 때)"
                    },
                    "ui_components": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "UI 컴포넌트 경로 (target=ui일 때)"
                    },
                    "cli_commands": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "CLI 명령어 목록 (target=cli일 때)"
                    }
                },
                "required": []
            }
        ),
        Tool(
            name="release_manager",
            description=(
                "릴리즈 자동화 에이전트 (/ship skill).\n"
                "- 변경사항 분석 (git diff, 파일 분류)\n"
                "- 문서 자동 업데이트 (CHANGELOG, TODO, PRD)\n"
                "- 커밋 메시지 생성 (Conventional Commits)\n"
                "- 커밋 및 푸시\n"
                "- 트랜잭션 기반 (에러 시 롤백)\n"
                "- Dry-run 모드 지원"
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "mode": {
                        "type": "string",
                        "description": "실행 모드",
                        "enum": ["full", "docs-only", "preview"],
                        "default": "full"
                    },
                    "custom_message": {
                        "type": "string",
                        "description": "커스텀 커밋 메시지 (지정 시 자동 생성 무시)"
                    },
                    "skip_push": {
                        "type": "boolean",
                        "description": "푸시 생략 여부",
                        "default": False
                    }
                },
                "required": []
            }
        ),
        Tool(
            name="security_reviewer",
            description=(
                "보안 검토 에이전트 (금융 시스템 특화).\n"
                "- 하드코딩된 비밀 정보 (API Key, Password)\n"
                "- SQL Injection, Command Injection\n"
                "- 민감 데이터 로깅\n"
                "- 안전하지 않은 연산 (unwrap, unsafe)\n"
                "- 의존성 취약점 (cargo audit)\n"
                "- 설정 파일 보안 (CORS, .env)"
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "검토 대상",
                        "enum": ["staged", "commit", "workspace"],
                        "default": "staged"
                    },
                    "commit_hash": {
                        "type": "string",
                        "description": "커밋 해시 (target=commit일 때)"
                    },
                    "severity": {
                        "type": "string",
                        "description": "심각도 필터",
                        "enum": ["all", "critical", "warning"],
                        "default": "all"
                    }
                },
                "required": []
            }
        ),
        Tool(
            name="test_writer",
            description=(
                "테스트 자동 생성 에이전트.\n"
                "- 함수 시그니처 분석\n"
                "- 테스트 스켈레톤 생성 (성공/실패/Edge case)\n"
                "- Mock 데이터 제안\n"
                "- 커버리지 분석\n"
                "- 테스트 가능성 평가\n"
                "- 기존 패턴 학습"
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "대상 유형",
                        "enum": ["function", "file", "crate", "coverage"],
                        "default": "function"
                    },
                    "mode": {
                        "type": "string",
                        "description": "실행 모드",
                        "enum": ["generate", "analyze", "check-coverage"],
                        "default": "generate"
                    },
                    "function_path": {
                        "type": "string",
                        "description": "함수 경로 (target=function일 때, 형식: 'path/to/file.rs::function_name')"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "파일 경로 (target=file일 때)"
                    },
                    "crate_name": {
                        "type": "string",
                        "description": "크레이트 이름 (target=crate일 때)"
                    }
                },
                "required": []
            }
        )
    ]


@server.call_tool()
async def call_tool(name: str, arguments: dict[str, Any]) -> list[TextContent | ImageContent | EmbeddedResource]:
    """도구 실행"""
    logger.info(f"Executing tool: {name} with args: {arguments}")

    try:
        agent = agents.get(name)
        if not agent:
            return [TextContent(
                type="text",
                text=f"❌ Error: Unknown agent '{name}'"
            )]

        # 에이전트 실행
        result = await agent.execute(arguments)

        return [TextContent(
            type="text",
            text=result
        )]

    except Exception as e:
        logger.error(f"Error executing {name}: {e}", exc_info=True)
        return [TextContent(
            type="text",
            text=f"❌ Error: {str(e)}"
        )]


async def main():
    """서버 실행"""
    logger.info(f"Starting ZeroQuant Agents MCP Server v{__version__}")
    logger.info(f"Project root: {PROJECT_ROOT}")

    async with stdio_server() as (read_stream, write_stream):
        await server.run(
            read_stream,
            write_stream,
            server.create_initialization_options()
        )


if __name__ == "__main__":
    asyncio.run(main())
