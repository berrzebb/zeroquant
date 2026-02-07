# MCP Agent Progress Monitor
# 실시간으로 에이전트 진행 상황을 모니터링합니다.

$logPath = "$env:USERPROFILE\.claude\logs\mcp-zeroquant-agents.log"

Write-Host "================================================" -ForegroundColor Cyan
Write-Host "  MCP Agent Progress Monitor" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Log File: $logPath" -ForegroundColor Gray
Write-Host "Press Ctrl+C to exit" -ForegroundColor Yellow
Write-Host ""

if (-not (Test-Path $logPath)) {
    Write-Host "❌ Log file not found!" -ForegroundColor Red
    Write-Host "   Make sure MCP server is running" -ForegroundColor Yellow
    exit 1
}

# 마지막 20줄부터 시작하고 새 줄 실시간 표시
Get-Content $logPath -Wait -Tail 20 | ForEach-Object {
    $line = $_

    # 색상 코딩
    if ($line -match "🚀|시작") {
        Write-Host $line -ForegroundColor Green
    }
    elseif ($line -match "✅|완료|성공") {
        Write-Host $line -ForegroundColor Green
    }
    elseif ($line -match "❌|실패|ERROR") {
        Write-Host $line -ForegroundColor Red
    }
    elseif ($line -match "⚠️|WARNING") {
        Write-Host $line -ForegroundColor Yellow
    }
    elseif ($line -match "\[\d+/\d+\]") {
        Write-Host $line -ForegroundColor Cyan
    }
    else {
        Write-Host $line -ForegroundColor White
    }
}
