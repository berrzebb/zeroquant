#!/usr/bin/env python3
"""
ZeroQuant ML 모델 훈련 스크립트.

사용법:
    # 기본 훈련 (XGBoost)
    python scripts/train_ml_model.py --symbol 005930

    # LightGBM 사용
    python scripts/train_ml_model.py --symbol AAPL --model lightgbm

    # 여러 심볼로 훈련
    python scripts/train_ml_model.py --symbols 005930,000660,035420

    # 커스텀 설정
    python scripts/train_ml_model.py --symbol BTC/USDT --timeframe 1h --future-periods 10

    # API에 모델 등록
    python scripts/train_ml_model.py --symbol 005930 --register
"""

import argparse
import os
import sys
from datetime import datetime, timedelta
from pathlib import Path

# 프로젝트 루트를 path에 추가
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

import numpy as np
import requests

from scripts.ml import DataFetcher, FeatureConfig, FeatureExtractor, ModelTrainer, TrainingConfig
from scripts.ml.model_trainer import ModelType


def parse_args():
    """CLI 인자 파싱."""
    parser = argparse.ArgumentParser(
        description="ZeroQuant ML 모델 훈련",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )

    # 데이터 옵션
    data_group = parser.add_argument_group("Data Options")
    data_group.add_argument(
        "--symbol", "-s",
        type=str,
        help="훈련할 심볼 (단일)",
    )
    data_group.add_argument(
        "--symbols",
        type=str,
        help="훈련할 심볼들 (쉼표로 구분)",
    )
    data_group.add_argument(
        "--timeframe", "-t",
        type=str,
        default="1d",
        help="타임프레임 (기본: 1d)",
    )
    data_group.add_argument(
        "--start-date",
        type=str,
        help="시작 날짜 (YYYY-MM-DD)",
    )
    data_group.add_argument(
        "--end-date",
        type=str,
        help="종료 날짜 (YYYY-MM-DD)",
    )

    # 모델 옵션
    model_group = parser.add_argument_group("Model Options")
    model_group.add_argument(
        "--model", "-m",
        type=str,
        choices=["xgboost", "lightgbm", "random_forest"],
        default="xgboost",
        help="모델 타입 (기본: xgboost)",
    )
    model_group.add_argument(
        "--n-estimators",
        type=int,
        default=100,
        help="트리 수 (기본: 100)",
    )
    model_group.add_argument(
        "--max-depth",
        type=int,
        default=6,
        help="최대 트리 깊이 (기본: 6)",
    )
    model_group.add_argument(
        "--learning-rate",
        type=float,
        default=0.1,
        help="학습률 (기본: 0.1)",
    )

    # 라벨 옵션
    label_group = parser.add_argument_group("Label Options")
    label_group.add_argument(
        "--future-periods",
        type=int,
        default=5,
        help="미래 수익률 계산 기간 (기본: 5)",
    )
    label_group.add_argument(
        "--threshold",
        type=float,
        default=0.02,
        help="상승/하락 판단 임계값 (기본: 0.02 = 2%%)",
    )

    # 출력 옵션
    output_group = parser.add_argument_group("Output Options")
    output_group.add_argument(
        "--output-dir", "-o",
        type=str,
        default="models",
        help="모델 저장 디렉토리 (기본: models)",
    )
    output_group.add_argument(
        "--name",
        type=str,
        help="모델 이름 (기본: 심볼명_모델타입)",
    )

    # API 연동 옵션
    api_group = parser.add_argument_group("API Options")
    api_group.add_argument(
        "--register",
        action="store_true",
        help="훈련 완료 후 API에 모델 등록",
    )
    api_group.add_argument(
        "--api-url",
        type=str,
        default="http://localhost:3000",
        help="API 서버 URL (기본: http://localhost:3000)",
    )

    # 기타
    parser.add_argument(
        "--database-url",
        type=str,
        help="PostgreSQL 연결 문자열",
    )
    parser.add_argument(
        "--verbose", "-v",
        action="store_true",
        help="상세 출력",
    )
    parser.add_argument(
        "--list-symbols",
        action="store_true",
        help="사용 가능한 심볼 목록 출력 후 종료",
    )

    return parser.parse_args()


def list_available_symbols(fetcher: DataFetcher, timeframe: str):
    """사용 가능한 심볼 목록 출력."""
    symbols = fetcher.list_symbols(timeframe)
    print(f"\n=== Available Symbols ({timeframe}) ===")
    print(f"Total: {len(symbols)}")
    for symbol in symbols:
        start, end = fetcher.get_date_range(symbol, timeframe)
        print(f"  {symbol}: {start} ~ {end}")


def train_single_symbol(
    fetcher: DataFetcher,
    extractor: FeatureExtractor,
    trainer: ModelTrainer,
    symbol: str,
    timeframe: str,
    start_date: datetime | None,
    end_date: datetime | None,
    future_periods: int,
    threshold: float,
    verbose: bool,
) -> tuple[np.ndarray, np.ndarray, list[str]] | None:
    """단일 심볼 데이터로 Feature/라벨 추출."""
    print(f"\nLoading data for {symbol}...")

    data = fetcher.fetch_ohlcv(symbol, timeframe, start_date, end_date)

    if len(data) < extractor.config.min_klines_required() + future_periods:
        print(f"  Skipping {symbol}: insufficient data ({len(data)} candles)")
        return None

    print(f"  Loaded {len(data)} candles")

    X, y = extractor.extract_with_labels(data, future_periods, threshold)

    if verbose:
        print(f"  Features: {X.shape}")
        print(f"  Labels: 하락={sum(y==0)}, 보합={sum(y==1)}, 상승={sum(y==2)}")

    return X, y, extractor.config.feature_names()


def register_model_with_api(
    api_url: str,
    model_name: str,
    onnx_path: str,
    result: dict,
) -> bool:
    """훈련된 모델을 API에 등록."""
    print(f"\nRegistering model with API...")

    # TODO: API 엔드포인트 구현 필요
    # POST /api/v1/ml/models 엔드포인트로 모델 등록
    #
    # 현재는 구현되지 않았으므로 메시지만 출력

    print("  Warning: Model registration API not implemented yet.")
    print("  To enable this feature, implement:")
    print("    POST /api/v1/ml/models")
    print("    - model_name: str")
    print("    - model_type: str")
    print("    - onnx_path: str")
    print("    - accuracy: float")
    print("    - feature_names: list[str]")

    # 임시: 모델 경로를 특정 위치에 복사하여 MlService에서 로드할 수 있게 함
    models_dir = Path("data/ml_models")
    models_dir.mkdir(parents=True, exist_ok=True)

    import shutil
    target_path = models_dir / f"{model_name}.onnx"
    shutil.copy(onnx_path, target_path)
    print(f"  Copied ONNX model to: {target_path}")

    return True


def main():
    """메인 함수."""
    args = parse_args()

    # 데이터베이스 연결
    fetcher = DataFetcher(args.database_url)

    # 심볼 목록 조회
    if args.list_symbols:
        list_available_symbols(fetcher, args.timeframe)
        return 0

    # 심볼 결정
    if args.symbols:
        symbols = [s.strip() for s in args.symbols.split(",")]
    elif args.symbol:
        symbols = [args.symbol]
    else:
        print("Error: --symbol or --symbols required")
        return 1

    # 날짜 파싱
    start_date = None
    end_date = None
    if args.start_date:
        start_date = datetime.strptime(args.start_date, "%Y-%m-%d")
    if args.end_date:
        end_date = datetime.strptime(args.end_date, "%Y-%m-%d")

    # Feature 추출기
    extractor = FeatureExtractor(FeatureConfig())

    # 모델 타입
    model_type = ModelType(args.model)

    # 훈련 설정
    training_config = TrainingConfig(
        model_type=model_type,
        n_estimators=args.n_estimators,
        max_depth=args.max_depth,
        learning_rate=args.learning_rate,
        verbose=args.verbose,
    )

    trainer = ModelTrainer(
        config=training_config,
        output_dir=args.output_dir,
    )

    # 데이터 수집
    all_X = []
    all_y = []
    feature_names = None

    for symbol in symbols:
        result = train_single_symbol(
            fetcher=fetcher,
            extractor=extractor,
            trainer=trainer,
            symbol=symbol,
            timeframe=args.timeframe,
            start_date=start_date,
            end_date=end_date,
            future_periods=args.future_periods,
            threshold=args.threshold,
            verbose=args.verbose,
        )

        if result:
            X, y, names = result
            all_X.append(X)
            all_y.append(y)
            if feature_names is None:
                feature_names = names

    if not all_X:
        print("Error: No data collected for training")
        return 1

    # 데이터 병합
    X = np.vstack(all_X)
    y = np.concatenate(all_y)

    print(f"\n=== Training Data Summary ===")
    print(f"Total samples: {len(y)}")
    print(f"Features: {X.shape[1]}")
    print(f"Labels: 하락={sum(y==0)}, 보합={sum(y==1)}, 상승={sum(y==2)}")

    # 훈련
    result = trainer.train(X, y, feature_names)

    # 모델 저장
    model_name = args.name or f"{'_'.join(symbols)}_{args.model}"
    native_path, onnx_path = trainer.save_model(model_name)

    print(f"\n=== Model Saved ===")
    print(f"Native: {native_path}")
    if onnx_path:
        print(f"ONNX: {onnx_path}")

    # API 등록
    if args.register and onnx_path:
        register_model_with_api(
            args.api_url,
            model_name,
            onnx_path,
            {
                "accuracy": result.accuracy,
                "f1_score": result.f1_score,
                "feature_names": feature_names,
            },
        )

    # Feature 중요도 출력
    print(f"\n=== Feature Importance (Top 10) ===")
    sorted_importance = sorted(
        result.feature_importance.items(),
        key=lambda x: x[1],
        reverse=True,
    )
    for name, importance in sorted_importance[:10]:
        print(f"  {name}: {importance:.4f}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
