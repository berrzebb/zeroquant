"""MCP Agent Progress Dashboard Server

실시간으로 에이전트 진행 상황을 웹에서 모니터링합니다.

Usage:
    python server.py
    Then open http://localhost:8765
"""

import asyncio
import os
from pathlib import Path
from datetime import datetime
from typing import Set
from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.staticfiles import StaticFiles
from fastapi.responses import HTMLResponse
import uvicorn

app = FastAPI(title="MCP Agent Dashboard")

# WebSocket 연결 관리
active_connections: Set[WebSocket] = set()

# 로그 파일 경로
LOG_PATH = Path.home() / ".claude" / "logs" / "mcp-zeroquant-agents.log"


class LogWatcher:
    """로그 파일을 실시간으로 모니터링"""

    def __init__(self, log_path: Path):
        self.log_path = log_path
        self.running = False

    async def watch(self):
        """로그 파일 변경 감지 및 브로드캐스트"""
        self.running = True

        if not self.log_path.exists():
            print(f"⚠️ Log file not found: {self.log_path}")
            print("   Waiting for log file to be created...")

        last_position = 0
        if self.log_path.exists():
            # 마지막 20줄 먼저 전송
            with open(self.log_path, "r", encoding="utf-8", errors="replace") as f:
                lines = f.readlines()
                last_lines = lines[-20:] if len(lines) > 20 else lines
                for line in last_lines:
                    await self.broadcast(self.parse_log_line(line))
                last_position = f.tell()

        while self.running:
            try:
                if not self.log_path.exists():
                    await asyncio.sleep(1)
                    continue

                with open(self.log_path, "r", encoding="utf-8", errors="replace") as f:
                    f.seek(last_position)
                    new_lines = f.readlines()
                    last_position = f.tell()

                    for line in new_lines:
                        if line.strip():
                            await self.broadcast(self.parse_log_line(line))

            except Exception as e:
                print(f"Error reading log: {e}")

            await asyncio.sleep(0.5)

    def parse_log_line(self, line: str) -> dict:
        """로그 라인 파싱"""
        line = line.strip()

        # 타임스탬프 추출
        timestamp = datetime.now().strftime("%H:%M:%S")
        if "[" in line and "]" in line:
            ts_end = line.find("]")
            if ts_end > 0:
                timestamp = line[1:ts_end]
                line = line[ts_end + 1 :].strip()

        # 레벨 결정
        level = "info"
        if "🚀" in line or "시작" in line:
            level = "start"
        elif "✅" in line or "완료" in line or "성공" in line:
            level = "success"
        elif "❌" in line or "실패" in line or "ERROR" in line:
            level = "error"
        elif "⚠️" in line or "WARNING" in line:
            level = "warning"
        elif "[" in line and "/" in line and "]" in line:
            level = "progress"

        # 진행률 추출 (예: [2/5])
        progress = None
        if "[" in line and "/" in line and "]" in line:
            import re

            match = re.search(r"\[(\d+)/(\d+)\]", line)
            if match:
                current = int(match.group(1))
                total = int(match.group(2))
                progress = {"current": current, "total": total}

        return {
            "timestamp": timestamp,
            "message": line,
            "level": level,
            "progress": progress,
        }

    async def broadcast(self, data: dict):
        """모든 연결된 클라이언트에 데이터 전송"""
        if not active_connections:
            return

        disconnected = set()
        for connection in active_connections:
            try:
                await connection.send_json(data)
            except Exception:
                disconnected.add(connection)

        # 끊긴 연결 제거
        active_connections.difference_update(disconnected)


# 로그 와처 인스턴스
log_watcher = LogWatcher(LOG_PATH)


@app.on_event("startup")
async def startup_event():
    """서버 시작 시 로그 와처 실행"""
    asyncio.create_task(log_watcher.watch())
    print("Dashboard server started")
    print(f"   Log file: {LOG_PATH}")
    print("   Open: http://localhost:8765")


@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    """WebSocket 연결 처리"""
    await websocket.accept()
    active_connections.add(websocket)
    print(f"Client connected (total: {len(active_connections)})")

    try:
        # 연결 유지
        while True:
            await websocket.receive_text()
    except WebSocketDisconnect:
        active_connections.remove(websocket)
        print(f"Client disconnected (total: {len(active_connections)})")


@app.get("/")
async def get_index():
    """메인 페이지"""
    html_path = Path(__file__).parent / "static" / "index.html"
    if html_path.exists():
        return HTMLResponse(content=html_path.read_text(encoding="utf-8"))
    else:
        return HTMLResponse(content="<h1>Dashboard HTML not found</h1>", status_code=404)


@app.get("/status")
async def get_status():
    """서버 상태 확인"""
    return {
        "log_file": str(LOG_PATH),
        "log_exists": LOG_PATH.exists(),
        "active_connections": len(active_connections),
    }


# Static 파일 서빙
app.mount("/static", StaticFiles(directory=Path(__file__).parent / "static"), name="static")


if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=8766, log_level="warning")
