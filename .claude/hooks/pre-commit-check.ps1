<#
.SYNOPSIS
    git commit 전 자동 검증 훅
.DESCRIPTION
    커밋 전 cargo fmt/clippy 체크를 수행합니다.
    종료 코드 2 = 차단, 0 = 통과
#>

$toolInput = $env:CLAUDE_TOOL_INPUT | ConvertFrom-Json -ErrorAction SilentlyContinue

if (-not $toolInput) { exit 0 }

$command = if ($toolInput.command) { $toolInput.command } else { "" }

# git commit 명령인지 확인
if ($command -notmatch "git\s+commit") { exit 0 }

$projectDir = $env:CLAUDE_PROJECT_DIR
if (-not $projectDir) { $projectDir = "D:\Trader" }

Push-Location $projectDir

# 1. cargo fmt 체크
Write-Host "🔍 [Hook] 포맷 검사 중..." -ForegroundColor Cyan
$fmtResult = & cargo fmt --all -- --check 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "🚫 [Hook] 포맷 미적용 파일 발견. 'cargo fmt --all' 실행 필요." -ForegroundColor Red
    $fmtResult | Select-Object -Last 5 | ForEach-Object { Write-Host "  $_" }
    Pop-Location
    exit 2
}

# 2. cargo clippy 체크
Write-Host "🔍 [Hook] Clippy 검사 중..." -ForegroundColor Cyan
$clippyResult = & cargo clippy --all-targets --message-format=short 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "🚫 [Hook] Clippy 경고 발견. 수정 후 커밋하세요." -ForegroundColor Red
    $clippyResult | Where-Object { $_ -match "warning|error" } | Select-Object -Last 10 | ForEach-Object { Write-Host "  $_" }
    Pop-Location
    exit 2
}

Write-Host "✅ [Hook] 포맷 + Clippy 검사 통과" -ForegroundColor Green
Pop-Location
exit 0
