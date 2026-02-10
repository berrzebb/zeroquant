<#
.SYNOPSIS
    위험한 Bash 명령 사전 검증 훅
.DESCRIPTION
    프로덕션 DB 직접 접근, 민감 정보 노출 등을 차단합니다.
    종료 코드 2 = 차단, 0 = 통과
#>

$toolInput = $env:CLAUDE_TOOL_INPUT | ConvertFrom-Json -ErrorAction SilentlyContinue

if (-not $toolInput) { exit 0 }

$command = if ($toolInput.command) { $toolInput.command } else { "" }

if (-not $command) { exit 0 }

# 1. 호스트 직접 DB/Redis 접속 차단 (podman exec 필수)
if ($command -match "^\s*(psql|redis-cli|pg_dump|pg_restore)\s") {
    Write-Host ""
    Write-Host "🚫 [Hook] 호스트에서 직접 DB/Redis 접속이 차단되었습니다." -ForegroundColor Red
    Write-Host "   → podman exec -it trader-timescaledb psql -U trader -d trader" -ForegroundColor Cyan
    Write-Host "   → podman exec -it trader-redis redis-cli" -ForegroundColor Cyan
    Write-Host ""
    exit 2
}

# 2. API 키/시크릿 평문 노출 차단
if ($command -match "(api_key|api_secret|access_token|API_KEY|API_SECRET|ACCESS_TOKEN)\s*=\s*['""]?[A-Za-z0-9+/=]{20,}") {
    Write-Host ""
    Write-Host "🚫 [Hook] 민감 정보(API 키/시크릿)가 명령에 포함되어 차단되었습니다." -ForegroundColor Red
    Write-Host "   → 환경변수 또는 웹 UI Settings에서 설정하세요." -ForegroundColor Cyan
    Write-Host ""
    exit 2
}

# 3. 프로덕션 DB 직접 DROP/TRUNCATE 차단
if ($command -match "(?i)(DROP\s+TABLE|DROP\s+DATABASE|TRUNCATE\s+TABLE)") {
    Write-Host ""
    Write-Host "🚫 [Hook] 파괴적 SQL 명령이 차단되었습니다: $($Matches[0])" -ForegroundColor Red
    Write-Host "   → 마이그레이션 파일로 스키마 변경하세요." -ForegroundColor Cyan
    Write-Host ""
    exit 2
}

exit 0
