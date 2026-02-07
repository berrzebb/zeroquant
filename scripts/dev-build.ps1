# =============================================================================
# Development Incremental Build Script
# =============================================================================
# 코드 변경 후 빠른 재빌드를 위한 스크립트
#
# 사용법:
#   .\scripts\dev-build.ps1           # 증분 빌드 후 실행
#   .\scripts\dev-build.ps1 -Build    # 빌드만 (실행 안함)
#   .\scripts\dev-build.ps1 -Clean    # 캐시 정리 후 빌드
#
# 첫 실행: ~10분 (모든 의존성 빌드)
# 소스 변경 후: ~1-2분 (변경된 크레이트만 재빌드)
# =============================================================================

param(
    [switch]$Build,     # 빌드만 (실행 안함)
    [switch]$Clean,     # 캐시 정리
    [switch]$Release    # 릴리즈 빌드 (기본: debug)
)

$ErrorActionPreference = "Stop"
$DEV_CONTAINER = "trader-rust-dev"
$DEV_IMAGE = "trader-rust-dev:latest"

function Write-Step { param($msg) Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Success { param($msg) Write-Host "✓ $msg" -ForegroundColor Green }

# 캐시 정리
if ($Clean) {
    Write-Step "Cargo 빌드 캐시 정리..."
    docker volume rm trader_cargo_target 2>$null
    Write-Success "캐시 정리 완료"
}

# 개발용 이미지 확인/생성
$imageExists = docker images -q $DEV_IMAGE 2>$null
if (-not $imageExists) {
    Write-Step "개발용 이미지 생성 중..."
    docker build -t $DEV_IMAGE -f - . @"
FROM rust:1.93-slim-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev lld mold curl g++ libstdc++-12-dev \
    && rm -rf /var/lib/apt/lists/* \
    && cargo install sccache
ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache \
    SCCACHE_DIR=/sccache \
    CARGO_INCREMENTAL=1 \
    RUSTFLAGS="-C link-arg=-fuse-ld=mold"
WORKDIR /app
"@
    Write-Success "개발용 이미지 생성 완료"
}

# DB/Redis 서비스 실행 확인
Write-Step "의존 서비스 확인..."
docker-compose up -d timescaledb redis
Start-Sleep -Seconds 2

# 빌드 옵션
$buildMode = if ($Release) { "--release" } else { "" }
$targetDir = if ($Release) { "target/release" } else { "target/debug" }

# 증분 빌드 실행
Write-Step "증분 빌드 시작..."
$startTime = Get-Date

docker run --rm -it `
    --name $DEV_CONTAINER `
    --network trader-network `
    -v "${PWD}:/app" `
    -v "trader_cargo_registry:/usr/local/cargo/registry" `
    -v "trader_cargo_git:/usr/local/cargo/git" `
    -v "trader_cargo_target:/app/target" `
    -v "trader_sccache:/sccache" `
    -e DATABASE_URL=postgresql://trader:trader_secret@timescaledb:5432/trader `
    -e REDIS_URL=redis://redis:6379 `
    $DEV_IMAGE `
    bash -c "cargo build $buildMode -p trader-api && sccache --show-stats"

$elapsed = (Get-Date) - $startTime
Write-Success "빌드 완료 ($([math]::Round($elapsed.TotalSeconds))초)"

# 실행
if (-not $Build) {
    Write-Step "API 서버 실행..."
    docker run --rm -it `
        --name trader-api-dev `
        --network trader-network `
        -p 3000:3000 `
        -v "${PWD}:/app" `
        -v "trader_cargo_target:/app/target" `
        -e RUST_LOG=info,trader_api=debug `
        -e API_HOST=0.0.0.0 `
        -e API_PORT=3000 `
        -e DATABASE_URL=postgresql://trader:trader_secret@timescaledb:5432/trader `
        -e REDIS_URL=redis://redis:6379 `
        -e JWT_SECRET=your-super-secret-jwt-key `
        -e ENCRYPTION_MASTER_KEY=MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI= `
        $DEV_IMAGE `
        /app/$targetDir/trader-api
}
