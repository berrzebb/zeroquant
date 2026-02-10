<#
.SYNOPSIS
    TypeScript 파일 편집 시 금지 패턴 사전 검증 훅
.DESCRIPTION
    ESLint 제로 정책 관련 패턴을 편집 시점에 검출합니다.
    종료 코드 2 = 차단, 1 = 경고, 0 = 통과
#>

$toolInput = $env:CLAUDE_TOOL_INPUT | ConvertFrom-Json -ErrorAction SilentlyContinue

if (-not $toolInput) { exit 0 }

$newCode = if ($toolInput.new_string) { $toolInput.new_string }
           elseif ($toolInput.newString) { $toolInput.newString }
           elseif ($toolInput.content) { $toolInput.content }
           else { "" }

$filePath = if ($toolInput.file_path) { $toolInput.file_path }
            elseif ($toolInput.filePath) { $toolInput.filePath }
            else { "" }

if (-not $filePath -or -not ($filePath -match "\.(ts|tsx)$")) { exit 0 }

$warnings = @()

# 1. any 타입 사용 검출
if ($newCode -match ":\s*any\b" -or $newCode -match "<any>") {
    $warnings += "any 타입 감지 → 구체적 타입 또는 unknown 사용 필요"
}

# 2. eslint-disable 주석 검출
if ($newCode -match "eslint-disable") {
    $warnings += "eslint-disable 감지 → 코드를 직접 수정하여 해결 필요"
}

# 3. 수동 타입 정의 (generated 타입 사용 권장)
if ($newCode -match "interface\s+(Strategy|Backtest|Order|Position|Signal)\w*Response\s*\{") {
    $warnings += "수동 API 응답 타입 감지 → @/types/generated/ 자동 생성 타입 사용 권장"
}

if ($warnings.Count -gt 0) {
    Write-Host ""
    Write-Host "⚠️  [Hook] 프론트엔드 규칙 위반 감지 ($($warnings.Count)건):" -ForegroundColor Yellow
    foreach ($w in $warnings) {
        Write-Host "   • $w" -ForegroundColor Yellow
    }
    Write-Host ""
    exit 1
}

exit 0
