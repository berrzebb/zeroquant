#!/bin/bash
# =============================================================================
# Podman Compose Helper Script
# =============================================================================
#
# Usage:
#   ./podman-compose.sh up          # Start core services
#   ./podman-compose.sh up -d       # Start in background
#   ./podman-compose.sh down        # Stop all services
#   ./podman-compose.sh dev         # Start with dev tools
#   ./podman-compose.sh monitoring  # Start with monitoring
#   ./podman-compose.sh logs        # View logs
#   ./podman-compose.sh build       # Build images
#   ./podman-compose.sh ps          # List containers
#
# =============================================================================

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if podman-compose is installed
check_dependencies() {
    if ! command -v podman &> /dev/null; then
        echo -e "${RED}Error: podman is not installed${NC}"
        echo "Install with: winget install RedHat.Podman (Windows)"
        echo "           or: sudo dnf install podman (Fedora)"
        echo "           or: sudo apt install podman (Ubuntu)"
        exit 1
    fi

    if ! command -v podman-compose &> /dev/null; then
        echo -e "${YELLOW}Warning: podman-compose is not installed${NC}"
        echo "Installing via pip..."
        pip install podman-compose
    fi
}

# Main command handler
case "$1" in
    up)
        check_dependencies
        echo -e "${GREEN}Starting core services...${NC}"
        shift
        podman-compose up "$@"
        ;;

    down)
        echo -e "${YELLOW}Stopping all services...${NC}"
        podman-compose down
        ;;

    dev)
        check_dependencies
        echo -e "${GREEN}Starting with development tools...${NC}"
        shift
        podman-compose --profile dev up "$@"
        ;;

    monitoring)
        check_dependencies
        echo -e "${GREEN}Starting with monitoring stack...${NC}"
        shift
        podman-compose --profile monitoring up "$@"
        ;;

    all)
        check_dependencies
        echo -e "${GREEN}Starting all services...${NC}"
        shift
        podman-compose --profile dev --profile monitoring up "$@"
        ;;

    logs)
        shift
        podman-compose logs "$@"
        ;;

    build)
        check_dependencies
        echo -e "${GREEN}Building images with Buildah...${NC}"
        podman-compose build --no-cache
        ;;

    ps)
        podman-compose ps
        ;;

    shell)
        # Connect to a running container
        if [ -z "$2" ]; then
            echo "Usage: $0 shell <container-name>"
            echo "Available containers:"
            podman ps --format "{{.Names}}"
            exit 1
        fi
        podman exec -it "$2" /bin/sh
        ;;

    *)
        echo "Trader Bot - Podman Compose Helper"
        echo ""
        echo "Usage: $0 <command> [options]"
        echo ""
        echo "Commands:"
        echo "  up              Start core services (timescaledb, redis, trader-api)"
        echo "  up -d           Start in detached mode (background)"
        echo "  down            Stop all services"
        echo "  dev             Start with dev tools (pgadmin, redis-commander)"
        echo "  monitoring      Start with monitoring (prometheus, grafana)"
        echo "  all             Start all services including dev and monitoring"
        echo "  logs [service]  View logs (optionally for specific service)"
        echo "  build           Build/rebuild images"
        echo "  ps              List running containers"
        echo "  shell <name>    Open shell in container"
        echo ""
        echo "Examples:"
        echo "  $0 up -d                    # Start in background"
        echo "  $0 logs trader-api          # View API logs"
        echo "  $0 shell trader-timescaledb # Connect to database"
        ;;
esac
