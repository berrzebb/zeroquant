<#
.SYNOPSIS
    Rust 파일 편집 시 금지 패턴 사전 검증 훅
.DESCRIPTION
    development_rules.md의 핵심 금지 사항을 편집 시점에 검출합니다.
    종료 코드 2 = 차단, 1 = 경고, 0 = 통과
#>

$toolInput = $env:CLAUDE_TOOL_INPUT | ConvertFrom-Json -ErrorAction SilentlyContinue

if (-not $toolInput) { exit 0 }

# 새로 작성되는 코드 추출
$newCode = if ($toolInput.new_string) { $toolInput.new_string }
           elseif ($toolInput.newString) { $toolInput.newString }
           elseif ($toolInput.content) { $toolInput.content }
           else { "" }

$filePath = if ($toolInput.file_path) { $toolInput.file_path }
            elseif ($toolInput.filePath) { $toolInput.filePath }
            else { "" }

if (-not $filePath -or -not $filePath.EndsWith(".rs")) { exit 0 }

# 테스트 파일 제외
$isTest = ($filePath -match "_test\.rs$") -or ($newCode -match "#\[cfg\(test\)\]")

$warnings = @()

# 1. unwrap() 검출 (테스트 코드 제외)
if (-not $isTest -and $newCode -match "\.unwrap\(\)") {
    $warnings += "unwrap() 사용 감지 → ? 또는 unwrap_or 사용 필요"
}

# 2. f64 금융 계산 검출 (price, amount, balance, fee, pnl, quantity 관련)
if ($newCode -match "(?i)(price|amount|balance|fee|pnl|quantity|capital)\s*:\s*f64") {
    $warnings += "f64 금융 타입 감지 → rust_decimal::Decimal 사용 필수"
}

# 3. #[allow(clippy:: 검출
if ($newCode -match "#\[allow\(clippy::") {
    $warnings += "#[allow(clippy::...)] 감지 → 코드를 직접 수정하여 해결 필요"
}

# 4. Local::now() 검출
if ($newCode -match "Local::now\(\)") {
    $warnings += "Local::now() 감지 → Utc::now() 사용 필수"
}

# 결과 출력
if ($warnings.Count -gt 0) {
    Write-Host ""
    Write-Host "⚠️  [Hook] 개발 규칙 위반 감지 ($($warnings.Count)건):" -ForegroundColor Yellow
    foreach ($w in $warnings) {
        Write-Host "   • $w" -ForegroundColor Yellow
    }
    Write-Host ""
    exit 1  # 경고 (차단하려면 exit 2)
}

exit 0
