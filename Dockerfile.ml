# =============================================================================
# ML Training Service Dockerfile
# =============================================================================
# Python 기반 ML 훈련 서비스
# XGBoost, LightGBM 훈련 및 ONNX 변환 지원
#
# 빌드: docker build -f Dockerfile.ml -t trader-ml:latest .
# 실행: docker-compose --profile ml up -d
# =============================================================================

FROM python:3.12-slim-bookworm

WORKDIR /app

# 시스템 의존성 설치
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    libpq-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Python 의존성 설치
COPY pyproject.toml ./

# pip 업그레이드 및 의존성 설치
RUN pip install --no-cache-dir --upgrade pip && \
    pip install --no-cache-dir \
    pandas>=2.0.0 \
    numpy>=1.24.0 \
    polars>=0.20.0 \
    psycopg2-binary>=2.9.0 \
    sqlalchemy>=2.0.0 \
    xgboost>=2.0.0 \
    lightgbm>=4.0.0 \
    scikit-learn>=1.4.0 \
    onnx>=1.15.0 \
    onnxmltools>=1.12.0 \
    skl2onnx>=1.16.0 \
    onnxruntime>=1.17.0 \
    click>=8.1.0 \
    rich>=13.0.0 \
    requests>=2.31.0

# 스크립트 복사
COPY scripts/ ./scripts/

# 모델 저장 디렉토리 생성
RUN mkdir -p /app/models /app/data/ml_models

# 환경변수 설정
ENV DATABASE_URL=postgresql://trader:trader_secret@timescaledb:5432/trader \
    PYTHONUNBUFFERED=1 \
    PYTHONDONTWRITEBYTECODE=1

# 헬스체크
HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
    CMD python -c "import xgboost; import lightgbm; print('OK')" || exit 1

# 기본 명령어: 심볼 목록 조회
CMD ["python", "-c", "from scripts.ml import DataFetcher; f=DataFetcher(); print('Available symbols:', f.list_symbols())"]
