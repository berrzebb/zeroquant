# =============================================================================
# Docker Build Script with Dependency Caching
# =============================================================================
# 사용법:
#   .\scripts\docker-build.ps1              # 일반 빌드
#   .\scripts\docker-build.ps1 -RebuildDeps # 의존성 이미지 재빌드
#   .\scripts\docker-build.ps1 -Clean       # 캐시 정리 후 빌드
# =============================================================================

param(
    [switch]$RebuildDeps,  # 의존성 이미지 재빌드
    [switch]$Clean,        # 캐시 정리
    [switch]$NoPush        # 이미지 푸시 안함
)

$ErrorActionPreference = "Stop"
$DEPS_IMAGE = "trader-deps:latest"
$API_IMAGE = "trader-api:latest"

# 색상 출력 함수
function Write-Step { param($msg) Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Success { param($msg) Write-Host "✓ $msg" -ForegroundColor Green }
function Write-Warning { param($msg) Write-Host "⚠ $msg" -ForegroundColor Yellow }

# BuildKit 활성화
$env:DOCKER_BUILDKIT = "1"
$env:COMPOSE_DOCKER_CLI_BUILD = "1"

# 캐시 정리
if ($Clean) {
    Write-Step "Docker 빌드 캐시 정리..."
    docker builder prune -f --filter type=exec.cachemount
    Write-Success "캐시 정리 완료"
}

# 의존성 이미지 존재 확인
$depsExists = docker images -q $DEPS_IMAGE 2>$null

if ($RebuildDeps -or -not $depsExists) {
    Write-Step "의존성 이미지 빌드 중... (최초 1회 또는 Cargo.toml 변경 시)"
    Write-Warning "이 작업은 10-15분 소요될 수 있습니다."

    $startTime = Get-Date
    docker build -f Dockerfile.deps -t $DEPS_IMAGE .
    $elapsed = (Get-Date) - $startTime

    Write-Success "의존성 이미지 빌드 완료 ($([math]::Round($elapsed.TotalMinutes, 1))분)"
}
else {
    Write-Success "의존성 이미지 캐시 사용: $DEPS_IMAGE"
}

# API 이미지 빌드
Write-Step "API 이미지 빌드 중..."
$startTime = Get-Date

docker-compose build trader-api

$elapsed = (Get-Date) - $startTime
Write-Success "API 이미지 빌드 완료 ($([math]::Round($elapsed.TotalMinutes, 1))분)"

# sccache 통계 확인
Write-Step "sccache 통계:"
docker run --rm $API_IMAGE sccache --show-stats 2>$null || Write-Warning "sccache 통계 조회 실패"

Write-Host "`n" -NoNewline
Write-Success "빌드 완료! 실행: docker-compose up -d trader-api"
