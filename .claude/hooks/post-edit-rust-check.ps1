<#
.SYNOPSIS
    Rust 파일 편집 후 자동 검증 훅
.DESCRIPTION
    .rs 파일이 수정되면 해당 패키지의 cargo check를 실행합니다.
    종료 코드 2 = 차단, 1 = 경고, 0 = 통과
#>

$toolInput = $env:CLAUDE_TOOL_INPUT | ConvertFrom-Json -ErrorAction SilentlyContinue

if (-not $toolInput) { exit 0 }

# 파일 경로 추출
$filePath = if ($toolInput.file_path) { $toolInput.file_path }
            elseif ($toolInput.filePath) { $toolInput.filePath }
            else { "" }

if (-not $filePath -or -not $filePath.EndsWith(".rs")) { exit 0 }

# crate 패키지명 추출 (crates/trader-xxx/... → trader-xxx)
if ($filePath -match "crates[\\/](trader-[^\\/]+)") {
    $package = $Matches[1]
    $checkResult = & cargo check -p $package --message-format=short 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host ""
        Write-Host "⚠️  [Hook] cargo check -p $package 실패:" -ForegroundColor Yellow
        $checkResult | Select-Object -Last 10 | ForEach-Object { Write-Host "  $_" }
        Write-Host ""
        exit 1  # 경고만 (차단하려면 exit 2)
    }
}

exit 0
