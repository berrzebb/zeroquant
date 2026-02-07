# =============================================================================
# Podman Compose Helper Script (Windows PowerShell)
# =============================================================================
#
# Usage:
#   .\podman-compose.ps1 up          # Start core services
#   .\podman-compose.ps1 up -d       # Start in background
#   .\podman-compose.ps1 down        # Stop all services
#   .\podman-compose.ps1 dev         # Start with dev tools
#   .\podman-compose.ps1 monitoring  # Start with monitoring
#   .\podman-compose.ps1 logs        # View logs
#
# =============================================================================

param(
    [Parameter(Position=0)]
    [string]$Command,

    [Parameter(Position=1, ValueFromRemainingArguments=$true)]
    [string[]]$Args
)

function Write-ColorOutput {
    param([string]$Message, [string]$Color = "White")
    Write-Host $Message -ForegroundColor $Color
}

function Test-Command {
    param([string]$Name)
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Check-Dependencies {
    if (-not (Test-Command "podman")) {
        Write-ColorOutput "Error: podman is not installed" "Red"
        Write-ColorOutput "Install with: winget install RedHat.Podman" "Yellow"
        exit 1
    }

    if (-not (Test-Command "podman-compose")) {
        Write-ColorOutput "Warning: podman-compose is not installed" "Yellow"
        Write-ColorOutput "Installing via pip..." "Yellow"
        pip install podman-compose
    }
}

switch ($Command) {
    "up" {
        Check-Dependencies
        Write-ColorOutput "Starting core services..." "Green"
        podman-compose up @Args
    }

    "down" {
        Write-ColorOutput "Stopping all services..." "Yellow"
        podman-compose down
    }

    "dev" {
        Check-Dependencies
        Write-ColorOutput "Starting with development tools..." "Green"
        podman-compose --profile dev up @Args
    }

    "monitoring" {
        Check-Dependencies
        Write-ColorOutput "Starting with monitoring stack..." "Green"
        podman-compose --profile monitoring up @Args
    }

    "all" {
        Check-Dependencies
        Write-ColorOutput "Starting all services..." "Green"
        podman-compose --profile dev --profile monitoring up @Args
    }

    "logs" {
        podman-compose logs @Args
    }

    "build" {
        Check-Dependencies
        Write-ColorOutput "Building images with Buildah..." "Green"
        podman-compose build --no-cache
    }

    "ps" {
        podman-compose ps
    }

    "shell" {
        if ($Args.Count -eq 0) {
            Write-ColorOutput "Usage: .\podman-compose.ps1 shell <container-name>" "Yellow"
            Write-ColorOutput "Available containers:" "White"
            podman ps --format "{{.Names}}"
            exit 1
        }
        podman exec -it $Args[0] /bin/sh
    }

    default {
        Write-Host @"
Trader Bot - Podman Compose Helper (Windows)

Usage: .\podman-compose.ps1 <command> [options]

Commands:
  up              Start core services (timescaledb, redis, trader-api)
  up -d           Start in detached mode (background)
  down            Stop all services
  dev             Start with dev tools (pgadmin, redis-commander)
  monitoring      Start with monitoring (prometheus, grafana)
  all             Start all services including dev and monitoring
  logs [service]  View logs (optionally for specific service)
  build           Build/rebuild images
  ps              List running containers
  shell <name>    Open shell in container

Examples:
  .\podman-compose.ps1 up -d                    # Start in background
  .\podman-compose.ps1 logs trader-api          # View API logs
  .\podman-compose.ps1 shell trader-timescaledb # Connect to database

Alternative: Rancher Desktop
  - Provides Docker/containerd + optional Kubernetes
  - Download: https://rancherdesktop.io/
"@
    }
}
