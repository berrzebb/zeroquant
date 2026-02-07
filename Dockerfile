# =============================================================================
# Dockerfile for Trader API (Simplified)
# =============================================================================
#
# 3단계 멀티 스테이지 빌드:
#   1. deps    - cargo-chef로 의존성 빌드 (캐싱)
#   2. builder - 소스코드 빌드
#   3. runtime - 최소 런타임 이미지
#
# 빌드:
#   docker build -t trader-api .
#
# 실행:
#   docker run -p 3000:3000 \
#     -e DATABASE_URL=postgresql://... \
#     -e REDIS_URL=redis://... \
#     trader-api
#
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Dependencies (cargo-chef로 의존성 캐싱)
# -----------------------------------------------------------------------------
FROM rust:1.93-slim-bookworm AS deps

# 빌드 의존성 설치
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    g++ \
    && rm -rf /var/lib/apt/lists/*

# cargo-chef 설치
RUN cargo install cargo-chef

WORKDIR /app

# 레시피 생성 및 의존성 빌드
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo chef prepare --recipe-path recipe.json && \
    cargo chef cook --release --recipe-path recipe.json

# -----------------------------------------------------------------------------
# Stage 2: Builder (소스코드 빌드)
# -----------------------------------------------------------------------------
FROM deps AS builder

COPY . .
RUN cargo build --release --bin trader-api

# -----------------------------------------------------------------------------
# Stage 3: Runtime (최소 이미지)
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim

WORKDIR /app

# 런타임 의존성 설치
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libstdc++6 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r trader && useradd -r -g trader trader

# 바이너리 복사
COPY --from=builder /app/target/release/trader-api /usr/local/bin/trader-api

# 설정 파일 복사
COPY config ./config

# 소유권 설정
RUN chown -R trader:trader /app
USER trader

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

ENV RUST_LOG=info,trader_api=debug \
    API_HOST=0.0.0.0 \
    API_PORT=3000

CMD ["trader-api"]
