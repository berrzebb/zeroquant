#!/bin/bash
# MCP Agent Progress Monitor
# 실시간으로 에이전트 진행 상황을 모니터링합니다.

LOG_PATH="$HOME/.claude/logs/mcp-zeroquant-agents.log"

echo "================================================"
echo "  MCP Agent Progress Monitor"
echo "================================================"
echo ""
echo "Log File: $LOG_PATH"
echo "Press Ctrl+C to exit"
echo ""

if [ ! -f "$LOG_PATH" ]; then
    echo "❌ Log file not found!"
    echo "   Make sure MCP server is running"
    exit 1
fi

# 마지막 20줄부터 시작하고 새 줄 실시간 표시
tail -n 20 -f "$LOG_PATH" | while IFS= read -r line; do
    # 색상 코딩 (ANSI escape codes)
    if echo "$line" | grep -qE "🚀|시작"; then
        echo -e "\033[0;32m$line\033[0m"  # Green
    elif echo "$line" | grep -qE "✅|완료|성공"; then
        echo -e "\033[0;32m$line\033[0m"  # Green
    elif echo "$line" | grep -qE "❌|실패|ERROR"; then
        echo -e "\033[0;31m$line\033[0m"  # Red
    elif echo "$line" | grep -qE "⚠️|WARNING"; then
        echo -e "\033[0;33m$line\033[0m"  # Yellow
    elif echo "$line" | grep -qE "\[[0-9]+/[0-9]+\]"; then
        echo -e "\033[0;36m$line\033[0m"  # Cyan
    else
        echo "$line"
    fi
done
